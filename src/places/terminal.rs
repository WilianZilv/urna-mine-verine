//! Terminal Interdimensional: painel de partidas (vila + jogos do Hub) e portões de viagem (server/terminal.js).
//! Andar no véu de um portão teleporta na hora (local) e avisa o servidor (contagem do dia); "/viajar destino"
//! cobra 5 moedas fictícias no servidor, que responde {t:"pl", k:"go", d} só pra quem pediu.

use super::{Geo, Place, Screen, TW, hash_str, text_mid};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{CYAN, DIM, GOLD, SOFT, fit, frame, holo_fx, text_right};
use crate::models::rgb;
use crate::player::Player;
use crate::world::{AIR, BLACK, G, GLASS, NEON, SAND, STONE, WOOL, World};
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::{FRAC_PI_2, PI};

/// Destino: id do servidor, nome no painel, chegada (x, z) com pé no chão, yaw olhando pro lugar, cor.
struct Dest {
    id: &'static str,
    name: &'static str,
    p: (f32, f32),
    yaw: f32,
    col: Color,
}

const fn d(id: &'static str, name: &'static str, p: (f32, f32), yaw: f32, col: (f32, f32, f32)) -> Dest {
    Dest { id, name, p, yaw, col: Color::new(col.0, col.1, col.2, 1.0) }
}

const DESTS: [Dest; 8] = [
    d("praca", "PRACA CENTRAL", (160.5, 181.5), -FRAC_PI_2, (0.4, 1.0, 0.5)),
    d("arena", "ARENA DOS GIGANTES", (160.5, 103.5), -FRAC_PI_2, (1.0, 0.35, 0.25)),
    d("club", "CLUB DO HOUSE", (92.5, 160.5), PI, (0.75, 0.35, 1.0)),
    d("lab", "LAB DO CEREBRO", (230.5, 160.5), 0.0, (0.35, 0.95, 1.0)),
    d("hub", "GAME HUB", (crate::hub::HX0 as f32 + 1.0, crate::layout::HUB_PATH_Z0 as f32 + 5.5), FRAC_PI_2, (1.0, 0.4, 0.95)),
    d("congresso", "CONGRESSO DA VILA", (90.5, 98.5), PI, (0.3, 0.9, 0.35)),
    d("bolsa", "BOLSA DE VALORES", (230.5, 98.5), 0.0, (1.0, 0.85, 0.25)),
    d("tv", "TV URNA NEWS", (62.5, 262.5), FRAC_PI_2, (1.0, 0.25, 0.35)),
];

/// Carimbos da vila (server/terminal.js VILLAGE): os 8 destinos + banco + o próprio terminal.
const STAMPS: [(&str, Color); 10] = [
    (DESTS[0].id, DESTS[0].col),
    (DESTS[1].id, DESTS[1].col),
    (DESTS[2].id, DESTS[2].col),
    (DESTS[3].id, DESTS[3].col),
    (DESTS[4].id, DESTS[4].col),
    (DESTS[5].id, DESTS[5].col),
    (DESTS[6].id, DESTS[6].col),
    (DESTS[7].id, DESTS[7].col),
    ("banco", Color::new(0.3, 1.0, 0.6, 1.0)),
    ("terminal", Color::new(0.3, 1.0, 1.0, 1.0)),
];
const NS: usize = STAMPS.len();

const HUB_GATE: usize = 4;
/// Plano dos portões (encostados na parede sul, véu virado pro saguão).
const GZ: f32 = 291.0;
const BOARD: Geo = Geo { c: vec3(200.5, G as f32 + 10.5, 265.8), n: vec3(0.0, 0.0, -1.0), w: 22.0, h: 11.0 };
const CENTER: Vec3 = vec3(202.5, G as f32, 280.5);
const CHEGADAS: Vec3 = vec3(193.5, G as f32, 269.5);
const TOWER: Vec3 = vec3(218.0, G as f32 + 25.0, 269.0);

fn gate(i: usize) -> Vec3 {
    vec3(202.5 + (i as f32 - 3.5) * 4.2, G as f32, GZ)
}

fn arrive(p: &mut Player, i: usize) {
    let d = &DESTS[i];
    p.pos = vec3(d.p.0, G as f32 + 0.2, d.p.1);
    p.vel = Vec3::ZERO;
    p.yaw = d.yaw;
}

struct Portal {
    name: String,
    by: String,
    n: i64,
    c: Color,
}

/// Chegada de alguém (broadcast {t:"pl",k:"term",arr:{d,n}}): feixe ~2 s, rótulo 3 s.
struct Arrival {
    i: usize,
    name: String,
    age: f32,
}

const FX_MAX: usize = 8;
const BEAM_T: f32 = 2.0;
const LABEL_T: f32 = 3.0;
const WARP_T: f32 = 1.2;
const BEAM_CYAN: Color = Color::new(0.3, 1.0, 1.0, 1.0);
const BEAM_MAG: Color = Color::new(1.0, 0.3, 1.0, 1.0);

fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

pub struct Terminal {
    screen: Screen,
    trips: [i64; 8],
    total: i64,
    got: bool,
    portals: Vec<Portal>,
    go: Option<usize>,
    cool: f32,
    arrivals: Vec<Arrival>,
    /// Redemoinho de partida: (onde, idade).
    warps: Vec<(Vec3, f32)>,
    /// Carimbos da vila (ordem de STAMPS): do servidor quando online, senão só desta sessão.
    stamps: [bool; NS],
    /// Carimbos de portais do Hub ("hub:<id>") e se já veio a lista do servidor.
    hubs: usize,
    srv: bool,
    /// Tour dos Poderes (copiado de places::tour todo frame): (carimbos, minutos restantes).
    pub tour: Option<(usize, i32)>,
    /// Mural dos guias (quem completou o tour), mais novo primeiro.
    pub mural: Vec<String>,
}

impl Terminal {
    pub fn new() -> Self {
        Terminal { screen: Screen::new(), trips: [0; 8], total: 0, got: false, portals: Vec::new(), go: None, cool: 0.0, arrivals: Vec::new(), warps: Vec::new(), stamps: [false; NS], hubs: 0, srv: false, tour: None, mural: Vec::new() }
    }

    fn warp(&mut self, at: Vec3) {
        if self.warps.len() >= FX_MAX {
            self.warps.remove(0);
        }
        self.warps.push((at, 0.0));
    }

    fn arrival(&mut self, i: usize, name: &str) {
        if self.arrivals.len() >= FX_MAX {
            self.arrivals.remove(0);
        }
        let name: String = name.to_uppercase().chars().take(20).collect();
        self.arrivals.push(Arrival { i, name: if name.is_empty() { "ALGUEM".into() } else { name }, age: 0.0 });
        self.warp(gate(i));
    }

    fn travel(&mut self, p: &mut Player, i: usize) {
        arrive(p, i);
        if !self.srv {
            self.stamps[i] = true;
        }
        self.cool = 2.0;
    }

    fn draw_fx(&self, trans: &mut Batch, labels: &mut Vec<Label>, eye: Vec3) {
        let id = Mat4::IDENTITY;
        let parts = crate::quality::pick([6, 12, 20]);
        for a in &self.arrivals {
            let d = &DESTS[a.i];
            let p = vec3(d.p.0, G as f32, d.p.1);
            let dist = eye.distance(p);
            let t = a.age;
            if t < BEAM_T && dist < 180.0 {
                let f = (1.0 - t / BEAM_T) * (t / 0.15).min(1.0);
                let h = 40.0;
                trans.glow(&id, p + vec3(0.0, h * 0.5, 0.0), vec3(1.6, h, 1.6), alpha(BEAM_CYAN, 0.3 * f));
                trans.glow(&id, p + vec3(0.0, h * 0.5, 0.0), vec3(0.5, h, 0.5), alpha(BEAM_MAG, 0.75 * f));
                let r = 1.0 + t * 3.0;
                trans.glow(&id, p + vec3(0.0, 0.05, 0.0), vec3(r * 2.0, 0.05, r * 2.0), alpha(BEAM_CYAN, 0.25 * f));
                for j in 0..parts {
                    let ang = j as f32 / parts as f32 * std::f32::consts::TAU + t * 3.0;
                    let rad = 0.8 + t * 2.5;
                    let y = 0.3 + t * 5.0 + (j % 3) as f32 * 0.6;
                    let c = if j % 2 == 0 { BEAM_CYAN } else { BEAM_MAG };
                    trans.glow(&id, p + vec3(ang.cos() * rad, y, ang.sin() * rad), Vec3::splat(0.2), alpha(c, 0.9 * f));
                }
            }
            if t < LABEL_T && dist < 40.0 {
                let c = if (t * 4.0).fract() < 0.5 { BEAM_CYAN } else { BEAM_MAG };
                labels.push(Label { pos: p + vec3(0.0, 3.4, 0.0), text: format!("{} CHEGOU DO TERMINAL", a.name), size: 20.0, color: c });
            }
        }
        let n = crate::quality::pick([8, 14, 24]);
        for &(w, t) in &self.warps {
            if eye.distance(w) > 80.0 {
                continue;
            }
            let f = 1.0 - t / WARP_T;
            trans.glow(&id, w + vec3(0.0, 1.6, 0.0), vec3(0.4 + t, 3.2, 0.4 + t), alpha(BEAM_MAG, 0.35 * f));
            for j in 0..n {
                let k = j as f32 / n as f32;
                let ang = k * std::f32::consts::TAU * 2.0 + t * 9.0;
                let rad = (1.4 - k * 0.7) * (0.3 + f * 0.7);
                let c = if j % 2 == 0 { BEAM_CYAN } else { BEAM_MAG };
                trans.glow(&id, w + vec3(ang.cos() * rad, 0.2 + k * 3.0 + t * 1.5, ang.sin() * rad), Vec3::splat(0.15), alpha(c, 0.9 * f));
            }
        }
    }

    fn paint(&self, time: f32) {
        draw_text("TERMINAL INTERDIMENSIONAL  -  PARTIDAS", 40.0, 84.0, 72.0, CYAN);
        let today: i64 = self.trips.iter().sum();
        let st = if self.got { format!("HOJE {today} VIAGENS  -  {} DESDE A INAUGURACAO", self.total) } else { "CONECTANDO A TORRE...".into() };
        text_right(&st, TW - 40.0, 80.0, 34.0, GOLD);
        draw_rectangle(40.0, 120.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

        // Esquerda: voos da vila (letreiro de palhetas: célula escura com risco no meio, status piscando)
        frame(40.0, 140.0, 1000.0, 770.0, "VOOS DA VILA");
        for (t, x) in [("DESTINO", 70.0), ("PORTAO", 600.0), ("HOJE", 760.0)] {
            draw_text(t, x, 214.0, 30.0, SOFT);
        }
        text_right("STATUS", 1010.0, 214.0, 30.0, SOFT);
        let flap = |x: f32, y: f32, w: f32| {
            draw_rectangle(x, y, w, 64.0, Color::new(0.02, 0.04, 0.06, 0.9));
            draw_rectangle(x, y + 31.0, w, 2.0, Color::new(0.0, 0.0, 0.0, 0.8));
        };
        for (i, d) in DESTS.iter().enumerate() {
            let y = 232.0 + i as f32 * 84.0;
            flap(60.0, y, 520.0);
            flap(590.0, y, 140.0);
            flap(750.0, y, 110.0);
            flap(870.0, y, 150.0);
            draw_rectangle(60.0, y, 8.0, 64.0, d.col);
            draw_text(&fit(d.name, 490.0, 46.0), 80.0, y + 48.0, 46.0, WHITE);
            text_mid(&format!("{:02}", i + 1), 660.0, y + 50.0, 52.0, GOLD);
            text_mid(&self.trips[i].to_string(), 805.0, y + 50.0, 48.0, WHITE);
            let on = (time * 1.3 + i as f32 * 0.37).fract() < 0.8;
            text_mid(if on { "EMBARQUE" } else { "- - - -" }, 945.0, y + 44.0, 30.0, if on { Color::new(0.4, 1.0, 0.5, 1.0) } else { DIM });
        }

        // Direita: jogos do Hub com presença ao vivo
        let (x1, w1) = (1060.0, TW - 1100.0);
        frame(x1, 140.0, w1, 770.0, "CONEXOES INTERDIMENSIONAIS (GAME HUB)");
        for (i, p) in self.portals.iter().enumerate() {
            let y = 196.0 + i as f32 * 80.0;
            draw_rectangle(x1 + 20.0, y, w1 - 40.0, 70.0, Color::new(0.08, 0.3, 0.42, 0.25));
            draw_rectangle(x1 + 20.0, y, 8.0, 70.0, p.c);
            let nc = Color::new(p.c.r.max(0.5), p.c.g.max(0.5), p.c.b.max(0.5), 1.0);
            draw_text(&fit(&p.name.to_uppercase(), w1 - 330.0, 40.0), x1 + 44.0, y + 38.0, 40.0, nc);
            draw_text(&fit(&format!("por {}", p.by), w1 - 330.0, 24.0), x1 + 44.0, y + 64.0, 24.0, SOFT);
            let n = if p.n == 1 { "1 jogando".to_string() } else { format!("{} jogando", p.n) };
            text_right(&n, x1 + w1 - 40.0, y + 46.0, 36.0, if p.n > 0 { GOLD } else { SOFT });
        }
        if self.portals.is_empty() {
            draw_text("nenhum jogo no ar agora", x1 + 30.0, 260.0, 44.0, SOFT);
            draw_text("plugue o teu: /hub.txt", x1 + 30.0, 320.0, 34.0, DIM);
        }
        let n = self.stamps.iter().filter(|&&s| s).count();
        let hubs = if self.hubs > 0 { format!(" +{} PORTAIS", self.hubs) } else { String::new() };
        draw_text(&format!("TEU PASSAPORTE: {n}/{NS}{hubs}"), x1 + 30.0, 862.0, 34.0, if n == NS { GOLD } else { CYAN });
        for (i, s) in STAMPS.iter().enumerate() {
            let x = x1 + w1 - 40.0 - (NS - i) as f32 * 32.0;
            if self.stamps[i] {
                draw_rectangle(x, 838.0, 26.0, 26.0, s.1);
            } else {
                draw_rectangle_lines(x, 838.0, 26.0, 26.0, 3.0, DIM);
            }
        }
        draw_text(&format!("embarque no portao GAME HUB ({:02})", HUB_GATE + 1), x1 + 30.0, 898.0, 30.0, GOLD);
        match self.tour {
            Some((n, min)) => text_right(&format!("TEU TOUR DOS PODERES: {n}/5 - {min} MIN"), x1 + w1 - 40.0, 898.0, 30.0, GOLD),
            None => text_right("TOUR DOS PODERES: /tour", x1 + w1 - 40.0, 898.0, 30.0, CYAN),
        }

        text_mid("/viajar destino (5 moedas)  -  ou anda no portao", TW * 0.5, 970.0, 52.0, GOLD);
        if !self.mural.is_empty() && (time / 5.0) as i32 % 2 == 1 {
            text_mid(&fit(&format!("MURAL DOS GUIAS DA VILA:  {}", self.mural.join("  -  ")), TW - 80.0, 30.0), TW * 0.5, 1010.0, 30.0, GOLD);
        } else {
            text_mid("/destinos lista tudo  -  moedas ficticias, passagem vai pro cofre da IA", TW * 0.5, 1008.0, 26.0, SOFT);
        }
        holo_fx(time, 0.0);
    }
}

impl Place for Terminal {
    fn on_msg(&mut self, m: &Value) {
        if m["k"] == "go" {
            self.go = DESTS.iter().position(|d| m["d"] == d.id);
            return;
        }
        if let Some(a) = m.get("arr") {
            if let Some(i) = DESTS.iter().position(|d| a["d"] == d.id) {
                self.arrival(i, a["n"].as_str().unwrap_or(""));
            }
            return;
        }
        if let Some(list) = m["stamps"].as_array() {
            self.srv = true;
            for (i, s) in STAMPS.iter().enumerate() {
                self.stamps[i] = list.iter().any(|v| v == s.0);
            }
            self.hubs = list.iter().filter(|v| v.as_str().is_some_and(|s| s.starts_with("hub:"))).count();
            return;
        }
        for (i, d) in DESTS.iter().enumerate() {
            self.trips[i] = m["trips"][d.id].as_i64().unwrap_or(0);
        }
        self.total = m["total"].as_i64().unwrap_or(0);
        self.got = true;
    }

    fn on_hub(&mut self, m: &Value) {
        if m["t"] != "hub" {
            return;
        }
        let Some(list) = m["portals"].as_array() else { return };
        let f = |v: &Value| v.as_f64().unwrap_or(1.0) as f32;
        let s = super::s;
        self.portals = list.iter().take(8).map(|p| Portal { name: s(&p["name"]), by: s(&p["by"]), n: p["n"].as_i64().unwrap_or(0), c: Color::new(f(&p["c"][0][0]), f(&p["c"][0][1]), f(&p["c"][0][2]), 1.0) }).collect();
    }

    fn update(&mut self, p: &mut Player, dt: f32, _time: f32, online: bool, out: &mut Vec<Value>) {
        self.cool -= dt;
        for a in &mut self.arrivals {
            a.age += dt;
        }
        self.arrivals.retain(|a| a.age < LABEL_T);
        for w in &mut self.warps {
            w.1 += dt;
        }
        self.warps.retain(|w| w.1 < WARP_T);
        if let Some(i) = self.go.take() {
            self.warp(p.pos);
            self.travel(p, i);
            return;
        }
        if !self.srv && p.pos.distance(CENTER) < 8.0 {
            self.stamps[NS - 1] = true;
        }
        if self.cool > 0.0 || p.pos.distance(CENTER) > 30.0 {
            return;
        }
        for i in 0..DESTS.len() {
            let r = p.pos - gate(i);
            if r.x.abs() < 1.1 && r.z > -0.35 && r.z < 1.0 && r.y > -0.5 && r.y < 3.5 {
                self.travel(p, i);
                if online {
                    out.push(json!({"t": "pl", "k": "term_trip", "d": DESTS[i].id}));
                } else {
                    self.arrival(i, "voce");
                }
                return;
            }
        }
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        if eye.distance(BOARD.c) > 110.0 {
            return;
        }
        let mut key = format!("{}|{}|{:?}|{:?}|{}|{:?}", self.got, self.total, self.trips, self.stamps, self.hubs, self.tour);
        key += &self.mural.join(",");
        for p in &self.portals {
            key += &format!("|{}:{}:{}", p.name, p.by, p.n);
        }
        if self.screen.begin(time, eye, &BOARD, hash_str(&key), true) {
            self.paint(time);
            self.screen.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        self.draw_fx(trans, labels, eye);
        if eye.distance(CENTER) > 130.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let k = 0.7 + 0.3 * (time * 2.0).sin();
        let white = rgb(0.9, 0.92, 0.95);
        let dark = rgb(0.1, 0.11, 0.13);

        // Moldura de luz do painel de partidas + marquise de metal em cima
        let (c, hw, hh) = (BOARD.c - vec3(0.0, 0.0, 0.08), BOARD.w * 0.5, BOARD.h * 0.5);
        let edge = Color::new(0.3 * k, 0.95, 1.0, 1.0);
        for (o, s) in [(vec3(0.0, hh, 0.0), vec3(BOARD.w + 0.5, 0.25, 0.15)), (vec3(0.0, -hh, 0.0), vec3(BOARD.w + 0.5, 0.25, 0.15)), (vec3(-hw, 0.0, 0.0), vec3(0.25, BOARD.h, 0.15)), (vec3(hw, 0.0, 0.0), vec3(0.25, BOARD.h, 0.15))] {
            b.glow(&id, c + o, s, edge);
        }
        b.cube(&id, c + vec3(0.0, hh + 0.6, -0.3), vec3(BOARD.w + 1.2, 0.6, 0.8), dark);
        b.glow(&id, c + vec3(0.0, hh + 0.28, -0.6), vec3(BOARD.w, 0.06, 0.2), edge);

        // Farol da torre de controle (gira e pisca)
        let blink = if (time * 1.5).fract() < 0.5 { rgb(1.0, 0.2, 0.2) } else { rgb(0.4, 0.05, 0.05) };
        b.glow(&(Mat4::from_translation(TOWER) * Mat4::from_rotation_y(time * 2.5)), Vec3::ZERO, vec3(1.2, 0.4, 0.4), blink);
        b.cube(&id, TOWER - vec3(0.0, 0.5, 0.0), vec3(0.3, 0.8, 0.3), dark);

        // Portões: pórtico branco, letreiro com número, véu brilhando na cor do destino
        let sparks = crate::quality::pick([0, 5, 10]);
        for (i, d) in DESTS.iter().enumerate() {
            let g = gate(i);
            let m = Mat4::from_translation(g) * Mat4::from_rotation_y(PI);
            let kk = 0.75 + 0.25 * (time * 2.2 + i as f32).sin();
            let col = Color::new(d.col.r * kk, d.col.g * kk, d.col.b * kk, 1.0);
            for side in [-1.0f32, 1.0] {
                b.cube(&m, vec3(side * 1.55, 2.1, 0.0), vec3(0.5, 4.2, 1.2), white);
                b.glow(&m, vec3(side * 1.28, 2.1, 0.0), vec3(0.05, 3.9, 0.6), col);
            }
            b.cube(&m, vec3(0.0, 4.55, 0.0), vec3(3.6, 0.7, 1.3), white);
            b.cube(&m, vec3(0.0, 5.4, 0.1), vec3(3.0, 1.0, 0.3), dark);
            b.glow(&m, vec3(0.0, 4.95, 0.62), vec3(3.2, 0.08, 0.06), col);
            b.glow(&m, vec3(-1.15, 5.4, 0.28), vec3(0.5, 0.7, 0.06), GOLD);
            trans.glow(&m, vec3(0.0, 2.0, 0.0), vec3(2.6, 3.9, 0.05), Color::new(d.col.r, d.col.g, d.col.b, 0.16 + 0.12 * kk));
            if sparks > 0 {
                trans.glow(&m, vec3(0.0, 0.03, 0.6), vec3(2.6, 0.04, 1.4), Color::new(d.col.r, d.col.g, d.col.b, 0.25));
            }
            for j in 0..sparks {
                let t = (time * 0.5 + j as f32 / sparks as f32).fract();
                let x = ((j * 37 + i * 11) % 20) as f32 / 10.0 - 1.0;
                trans.glow(&m, vec3(x, 0.2 + t * 3.6, 0.05), Vec3::splat(0.1), Color::new(1.0, 1.0, 1.0, 0.8 * (1.0 - t)));
            }
            if eye.distance(g) < 35.0 {
                let at = |y: f32| m.transform_point3(vec3(0.0, y, 0.5));
                labels.push(Label { pos: at(6.3), text: format!("PORTAO {:02}", i + 1), size: 16.0, color: GOLD });
                labels.push(Label { pos: at(5.4), text: d.name.into(), size: 18.0, color: Color::new(d.col.r.max(0.5), d.col.g.max(0.5), d.col.b.max(0.5), 1.0) });
                let n = self.trips[i];
                labels.push(Label { pos: at(0.7), text: if n == 1 { "1 viagem hoje".into() } else { format!("{n} viagens hoje") }, size: 14.0, color: SOFT });
            }
        }

        // Chegadas: totem perto da porta pra quem volta andando
        b.cube(&id, CHEGADAS + vec3(0.0, 1.4, 0.0), vec3(0.3, 2.8, 0.3), dark);
        b.glow(&id, CHEGADAS + vec3(0.0, 3.0, 0.0), vec3(2.4, 0.6, 0.15), rgb(0.4, 1.0, 0.5));
        if eye.distance(CHEGADAS) < 30.0 {
            labels.push(Label { pos: CHEGADAS + vec3(0.0, 3.8, 0.0), text: "CHEGADAS".into(), size: 20.0, color: rgb(0.5, 1.0, 0.6) });
            labels.push(Label { pos: CHEGADAS + vec3(0.0, 3.4, 0.0), text: "bem-vindo de volta, sem bagagem extraviada (eu acho)".into(), size: 13.0, color: SOFT });
        }
        if eye.distance(BOARD.c) < 90.0 {
            labels.push(Label { pos: BOARD.c + vec3(0.0, 7.6, -0.5), text: "TERMINAL INTERDIMENSIONAL".into(), size: 28.0, color: CYAN });
        }
        let check = vec3(189.5, G as f32 + 1.8, 277.5);
        if eye.distance(check) < 25.0 {
            labels.push(Label { pos: check, text: "CHECK-IN: nao precisa, ninguem confere nada".into(), size: 14.0, color: SOFT });
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.screen.draw(&BOARD, time, eye, 130.0);
    }
}

/// Saguão de vidro e pedra branca, pista neon no chão, painel na fachada norte e torre de controle no canto NE.
pub fn build(w: &mut World) {
    let r = crate::layout::TERMINAL;
    let (x0, z0, x1, z1) = r;
    let g = G;
    super::clear_lot(w, r, 26, STONE);
    // Piso: pista central preta com tracejado e luzes, faixa de embarque amarela, fundo dos portões escuro
    for z in z0 + 1..z1 {
        for x in x0 + 1..x1 {
            let b = if z >= 290 {
                BLACK
            } else if z == 289 {
                SAND
            } else if x == 200 {
                if z % 3 == 0 { BLACK } else { WOOL }
            } else if x == 197 || x == 203 {
                if z % 4 == 0 { NEON } else { WOOL }
            } else if (198..=202).contains(&x) {
                BLACK
            } else {
                STONE
            };
            w.set(x, g - 1, z, b);
        }
    }
    super::fill(w, (192, g - 1, 268), (194, g - 1, 270), NEON);
    // Paredes: pilares brancos a cada 4, vidro entre; parede sul inteira branca com faixa neon
    for y in g..=g + 11 {
        for x in x0..=x1 {
            for z in [z0, z1] {
                let pier = (x - x0) % 4 == 0 || x == x1;
                let b = if z == z1 { if y == g + 6 { NEON } else { WOOL } } else if y == g || y >= g + 10 || pier { WOOL } else { GLASS };
                w.set(x, y, z, b);
            }
        }
        for z in z0..=z1 {
            for x in [x0, x1] {
                let pier = (z - z0) % 4 == 0 || z == z1;
                w.set(x, y, z, if y == g || y >= g + 10 || pier { WOOL } else { GLASS });
            }
        }
    }
    // Teto branco com claraboia
    for z in z0..=z1 {
        for x in x0..=x1 {
            let sky = (190..=214).contains(&x) && (271..=289).contains(&z) && (z - 271) % 4 != 0;
            w.set(x, g + 12, z, if sky { GLASS } else { WOOL });
        }
    }
    // Fachada norte alta: fundo preto atrás do painel, cornija neon, porta
    super::fill(w, (x0, g + 12, z0), (214, g + 18, z0), WOOL);
    super::fill(w, (188, g + 4, z0), (212, g + 16, z0), BLACK);
    super::fill(w, (x0, g + 19, z0), (214, g + 19, z0), NEON);
    super::fill(w, (198, g, z0), (202, g + 3, z0), AIR);
    super::fill(w, (197, g + 4, z0), (203, g + 4, z0), NEON);
    // Balcões de check-in
    super::fill(w, (187, g, 277), (192, g, 278), WOOL);
    super::fill(w, (212, g, 277), (217, g, 278), WOOL);
    // Torre de controle: fuste branco com faixas, cabine de vidro, antena
    super::fill(w, (216, g, 267), (219, g + 20, 270), WOOL);
    for y in (g + 14..=g + 19).step_by(5) {
        super::fill(w, (216, y, 267), (219, y, 270), NEON);
    }
    super::fill(w, (215, g + 20, 266), (220, g + 20, 271), WOOL);
    super::fill(w, (215, g + 21, 266), (220, g + 23, 271), GLASS);
    for (x, z) in [(215, 266), (220, 266), (215, 271), (220, 271)] {
        super::fill(w, (x, g + 21, z), (x, g + 23, z), WOOL);
    }
    super::fill(w, (215, g + 24, 266), (220, g + 24, 271), WOOL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_arrivals_free() {
        let w = World::generate();
        for d in &DESTS {
            for (ox, oz) in [(0.0, 0.0), (-0.3, -0.3), (0.3, -0.3), (-0.3, 0.3), (0.3, 0.3)] {
                let (x, z) = ((d.p.0 + ox).floor() as i32, (d.p.1 + oz).floor() as i32);
                for y in [G, G + 1] {
                    assert!(!w.solid(x, y, z), "{} preso em {x},{y},{z}", d.id);
                }
                assert!(w.solid(x, G - 1, z), "{} sem chao em {x},{z}", d.id);
            }
        }
    }

    #[test]
    fn terminal_gates_reachable() {
        let w = World::generate();
        for i in 0..DESTS.len() {
            let g = gate(i);
            for z in [g.z - 3.0, g.z, g.z + 0.9] {
                assert!(!w.solid_f(g.x, G as f32 + 0.5, z) && !w.solid_f(g.x, G as f32 + 1.5, z), "portao {i} bloqueado");
            }
        }
        assert!(!w.solid(200, G, 266) && !w.solid(200, G + 1, 266), "porta fechada");
    }
}
