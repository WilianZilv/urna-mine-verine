//! Game Hub: corredor de arcos-portal pra jogos web externos (Urna Portal Protocol, server/hub.js).
//! Cada portal verificado e ativo vira um arco; entrar nele pede um token de sessão ao servidor e o
//! web/hub.js abre o jogo num iframe sandbox por cima. Esc/voltar devolve o jogador na frente do arco.

use crate::batch::Batch;
use crate::extras::Label;
use crate::models::rgb;
use crate::player::Player;
use crate::world::*;
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::PI;

pub const HX0: i32 = 91;
pub const HX1: i32 = 121;
pub const HZ0: i32 = 81;
pub const HZ1: i32 = 95;
const SLOTS: usize = 8;
const SIGN: Vec3 = vec3(120.6, G as f32 + 6.5, 88.0);
const SITE: &str = "urna-mine-verine.wilianzilv.workers.dev";

#[cfg(target_arch = "wasm32")]
mod js {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn urna_hub_open(ptr: *const u8, len: usize);
        pub fn urna_hub_state() -> i32;
        pub fn urna_hub_query(ptr: *mut u8, cap: usize) -> i32;
    }
}

fn js_open(_s: &str) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        js::urna_hub_open(_s.as_ptr(), _s.len())
    }
}

fn js_state() -> i32 {
    #[cfg(target_arch = "wasm32")]
    return unsafe { js::urna_hub_state() };
    #[cfg(not(target_arch = "wasm32"))]
    0
}

/// `?hub=id` na URL: voltou de um jogo externo, nasce na frente do arco dele.
fn js_query() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::read_string(|p, c| unsafe { js::urna_hub_query(p, c) });
    #[cfg(not(target_arch = "wasm32"))]
    std::env::var("URNA_HUB").ok()
}

/// Piso escuro com faixa neon, colunas limpas até o céu e caminho de cascalho da estrada leste.
pub fn build(w: &mut World) {
    let g = G;
    for x in HX0..=HX1 {
        for z in HZ0..=HZ1 {
            for y in g..WY {
                w.set(x, y, z, AIR);
            }
            for y in g - 4..g - 1 {
                if !w.solid(x, y, z) {
                    w.set(x, y, z, STONE);
                }
            }
            let edge = x == HX0 || x == HX1 || z == HZ0 || z == HZ1;
            w.set(x, g - 1, z, if edge { STONE } else if z == (HZ0 + HZ1) / 2 && x % 3 == 0 { NEON } else { crate::world::BLACK });
        }
    }
    for x in HX0..=HX0 + 1 {
        for z in 66..HZ0 {
            w.set(x, g - 1, z, GRAVEL);
            for y in g..g + 3 {
                w.set(x, y, z, AIR);
            }
        }
    }
}

/// Centro do arco (no chão) e normal pra dentro do corredor.
fn slot(i: usize) -> (Vec3, Vec3) {
    let x = 96.5 + 6.0 * (i % 4) as f32;
    if i < 4 { (vec3(x, G as f32, HZ0 as f32 + 1.5), Vec3::Z) } else { (vec3(x, G as f32, HZ1 as f32 - 0.5), -Vec3::Z) }
}

struct Portal {
    id: String,
    name: String,
    by: String,
    v: String,
    c: [Color; 2],
    n: i64,
}

pub struct Hub {
    portals: Vec<Portal>,
    site: String,
    got: bool,
    query: Option<String>,
    /// Esperando o token do servidor (segundos restantes) e arco em que entrou.
    pending: f32,
    inside: Option<usize>,
    open_t: f32,
    seen_open: bool,
    release: bool,
    cool: f32,
    pub msg: Option<(String, f32)>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

impl Hub {
    pub fn new() -> Self {
        Hub { portals: Vec::new(), site: SITE.into(), got: false, query: js_query().filter(|q| !q.is_empty()), pending: 0.0, inside: None, open_t: 0.0, seen_open: false, release: false, cool: 0.0, msg: None }
    }

    /// {t:"hub"} = lista de arcos; {t:"hub_s"} = token de sessão pra abrir o jogo.
    pub fn on_msg(&mut self, m: &Value) {
        if m["t"] == "hub_s" {
            let Some(i) = self.portals.iter().position(|p| m["p"] == p.id.as_str()) else { return };
            if self.pending <= 0.0 {
                return;
            }
            self.pending = 0.0;
            self.inside = Some(i);
            self.open_t = 0.0;
            self.seen_open = false;
            self.release = true;
            js_open(&json!({"p": m["p"], "tok": m["tok"], "url": m["url"], "origin": m["origin"], "name": m["name"], "ret": m["ret"]}).to_string());
            return;
        }
        let Some(list) = m["portals"].as_array() else { return };
        if let Some(site) = m["site"].as_str().filter(|s| !s.is_empty()) {
            self.site = site.trim_start_matches("https://").trim_start_matches("http://").to_string();
        }
        let col = |c: &Value| Color::new(c[0].as_f64().unwrap_or(1.0) as f32, c[1].as_f64().unwrap_or(1.0) as f32, c[2].as_f64().unwrap_or(1.0) as f32, 1.0);
        self.portals = list.iter().take(SLOTS).map(|p| Portal { id: s(&p["id"]), name: s(&p["name"]), by: s(&p["by"]), v: s(&p["v"]), c: [col(&p["c"][0]), col(&p["c"][1])], n: p["n"].as_i64().unwrap_or(0) }).collect();
        self.got = true;
    }

    /// Detecta entrada no arco e segura o jogador enquanto o overlay está aberto.
    /// Retorna (mensagem pro servidor, soltar o mouse agora).
    pub fn update(&mut self, player: &mut Player, dt: f32, online: bool, col: Color, ch: u8) -> (Option<Value>, bool) {
        self.cool -= dt;
        let mut out = None;
        if let Some(q) = self.query.take_if(|_| self.got) {
            if let Some(i) = self.portals.iter().position(|p| p.id == q) {
                let (c, n) = slot(i);
                player.pos = c + n * 2.5;
                player.vel = Vec3::ZERO;
                player.yaw = n.z.atan2(n.x);
                self.cool = 1.0;
            }
        }
        if let Some(i) = self.inside {
            let (c, n) = slot(i);
            player.pos = c + n * 2.5;
            player.vel = Vec3::ZERO;
            self.open_t += dt;
            let st = js_state();
            self.seen_open |= st == 1;
            if st == 0 && (self.seen_open || self.open_t > 1.0) {
                if !self.seen_open {
                    self.msg = Some(("PORTAL SO ABRE NO NAVEGADOR".into(), 2.5));
                }
                self.inside = None;
                player.yaw = n.z.atan2(n.x);
                self.cool = 2.0;
            }
        } else if self.pending > 0.0 {
            self.pending -= dt;
            if self.pending <= 0.0 {
                self.cool = 2.0;
                self.msg = Some(("PORTAL NAO RESPONDEU".into(), 2.5));
            }
        } else if self.cool <= 0.0 && online {
            for (i, p) in self.portals.iter().enumerate() {
                let (c, n) = slot(i);
                let r = player.pos - c;
                let (d, lat) = (r.dot(n), r.x * n.z - r.z * n.x);
                if lat.abs() < 1.5 && d < 0.35 && d > -1.0 && r.y < 3.5 {
                    const CH: [&str; 5] = ["steve", "skatista", "bandido", "bandido", "niko"];
                    let hex = format!("#{:02x}{:02x}{:02x}", (col.r * 255.0) as u8, (col.g * 255.0) as u8, (col.b * 255.0) as u8);
                    out = Some(json!({"t": "hub", "k": "enter", "p": p.id, "col": hex, "ch": CH[(ch as usize).min(4)]}));
                    self.pending = 4.0;
                    self.msg = Some((format!("ABRINDO {}...", p.name.to_uppercase()), 2.0));
                    player.pos = c + n * 0.6;
                    player.vel = Vec3::ZERO;
                    break;
                }
            }
        }
        (out, std::mem::take(&mut self.release))
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let mid = vec3((HX0 + HX1) as f32 * 0.5, G as f32, (HZ0 + HZ1) as f32 * 0.5);
        if eye.distance(mid) > 80.0 {
            return;
        }
        let stone = rgb(0.2, 0.2, 0.25);
        for i in 0..SLOTS {
            let (c, n) = slot(i);
            let m = Mat4::from_translation(c) * Mat4::from_rotation_y(if n.z > 0.0 { 0.0 } else { PI });
            let p = self.portals.get(i);
            let (c1, c2) = p.map_or((rgb(0.3, 0.3, 0.35), rgb(0.2, 0.2, 0.25)), |p| (p.c[0], p.c[1]));
            let k = 0.7 + 0.3 * (time * 2.0 + i as f32).sin();
            // Degraus, colunas em pedra escura com veios de luz, arco escalonado no topo
            b.cube(&m, vec3(0.0, 0.1, 0.0), vec3(5.4, 0.2, 2.0), stone);
            for side in [-1.0f32, 1.0] {
                b.cube(&m, vec3(side * 2.0, 2.4, 0.0), vec3(0.8, 4.8, 0.9), stone);
                b.glow(&m, vec3(side * 1.58, 2.4, 0.0), vec3(0.06, 4.6, 0.5), Color::new(c1.r * k, c1.g * k, c1.b * k, 1.0));
                b.cube(&m, vec3(side * 1.4, 5.0, 0.0), vec3(1.6, 0.6, 0.9), stone);
            }
            b.cube(&m, vec3(0.0, 5.55, 0.0), vec3(3.2, 0.6, 0.9), stone);
            b.glow(&m, vec3(0.0, 5.9, 0.0), vec3(1.0, 0.25, 0.95), c2);
            let Some(p) = p else {
                trans.glow(&m, vec3(0.0, 2.4, 0.0), vec3(3.2, 4.6, 0.04), Color::new(0.5, 0.5, 0.6, 0.08));
                if eye.distance(c) < 25.0 {
                    labels.push(Label { pos: m.transform_point3(vec3(0.0, 3.0, 0.3)), text: "VAGA LIVRE - /hub.txt".into(), size: 16.0, color: rgb(0.6, 0.6, 0.7) });
                }
                continue;
            };
            // Redemoinho: véu translúcido + espiral de faíscas nas duas cores do portal
            trans.glow(&m, vec3(0.0, 2.4, 0.0), vec3(3.2, 4.6, 0.04), Color::new(c1.r, c1.g, c1.b, 0.18 + 0.1 * k));
            for j in 0..36 {
                let t = j as f32 / 36.0;
                let a = time * 2.4 + t * 13.0 + i as f32;
                let r = 0.15 + 1.35 * t;
                let q = vec3(a.cos() * r, 2.4 + a.sin() * r * 1.5, 0.05);
                let c = Color::new(c1.r + (c2.r - c1.r) * t, c1.g + (c2.g - c1.g) * t, c1.b + (c2.b - c1.b) * t, 0.85);
                trans.glow(&m, q, Vec3::splat(0.14 + 0.1 * (1.0 - t)), c);
            }
            if p.n > 0 {
                for j in 0..p.n.min(12) {
                    let a = time * 1.3 + j as f32 * 0.52;
                    trans.glow(&m, vec3(a.cos() * 2.5, 5.2 + (time * 3.0 + j as f32).sin() * 0.3, a.sin() * 0.6), Vec3::splat(0.18), c2);
                }
            }
            if eye.distance(c) < 32.0 {
                let at = |y: f32| m.transform_point3(vec3(0.0, y, 0.6));
                labels.push(Label { pos: at(6.8), text: p.name.to_uppercase(), size: 22.0, color: Color::new(c2.r.max(0.4), c2.g.max(0.4), c2.b.max(0.4), 1.0) });
                labels.push(Label { pos: at(6.3), text: format!("por {}  -  v{}", p.by, p.v), size: 15.0, color: WHITE });
                let n = if p.n == 1 { "1 jogando agora".to_string() } else { format!("{} jogando agora", p.n) };
                labels.push(Label { pos: at(0.9), text: n, size: 15.0, color: rgb(0.7, 1.0, 0.8) });
            }
        }
        // Placa grande no fundo do corredor
        let m = Mat4::from_translation(SIGN) * Mat4::from_rotation_y(-PI * 0.5);
        b.cube(&m, Vec3::ZERO, vec3(12.0, 5.0, 0.3), rgb(0.05, 0.03, 0.09));
        for (c, sz) in [(vec3(0.0, 2.55, 0.0), vec3(12.3, 0.15, 0.4)), (vec3(0.0, -2.55, 0.0), vec3(12.3, 0.15, 0.4)), (vec3(6.1, 0.0, 0.0), vec3(0.15, 5.2, 0.4)), (vec3(-6.1, 0.0, 0.0), vec3(0.15, 5.2, 0.4))] {
            b.glow(&m, c, sz, crate::club::hsv(time * 0.05, 0.7, 1.0));
        }
        for side in [-1.0f32, 1.0] {
            b.cube(&m, vec3(side * 5.0, -4.4, 0.0), vec3(0.4, 3.8, 0.4), rgb(0.15, 0.15, 0.18));
        }
        if eye.distance(SIGN) < 55.0 {
            let at = |y: f32| m.transform_point3(vec3(0.0, y, 0.3));
            labels.push(Label { pos: at(1.4), text: "GAME HUB - PORTAIS PRA JOGOS DA WEB".into(), size: 30.0, color: rgb(0.7, 0.5, 1.0) });
            labels.push(Label { pos: at(0.2), text: format!("HUB: manda teu agent ler {}/hub.txt", self.site), size: 22.0, color: WHITE });
            let status = if !self.got { "conectando...".to_string() } else { format!("{} portais no ar  |  anda pra dentro do arco pra jogar  |  Esc volta", self.portals.len()) };
            labels.push(Label { pos: at(-1.1), text: status, size: 17.0, color: rgb(0.6, 1.0, 0.9) });
        }
        // Totem na entrada do caminho (estrada leste)
        let post = vec3(HX0 as f32 + 3.2, G as f32, 66.5);
        b.cube(&Mat4::IDENTITY, post + vec3(0.0, 1.5, 0.0), vec3(0.3, 3.0, 0.3), rgb(0.2, 0.15, 0.1));
        b.glow(&Mat4::IDENTITY, post + vec3(0.0, 3.2, 0.0), vec3(1.6, 0.6, 0.15), rgb(0.6, 0.35, 1.0));
        if eye.distance(post) < 30.0 {
            labels.push(Label { pos: post + vec3(0.0, 3.9, 0.0), text: "GAME HUB (SUL)".into(), size: 18.0, color: rgb(0.8, 0.65, 1.0) });
        }
    }
}
