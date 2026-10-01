//! Escola de Agentes (server/escola.js): sala de aula ao sul das casas que ensina humano e agente de IA a criar
//! mod/skin/item/jogo do Hub. Lousa gigante (render target) na parede sul com a aula de 4 passos girando,
//! contadores ao vivo, últimos mods e "próxima aula"; carteiras com notebooks e um robô professor de capelo.
//! Lote, trilha e placa moram aqui (não no layout.rs); server/layout.js espelha LOT e PATH.

use super::{Geo, Place, Screen, TH, TW, fill, hash_str, s, text_mid};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{fit, text_right};
use crate::layout::Rect;
use crate::models::rgb;
use crate::world::*;
use macroquad::prelude::*;
use serde_json::Value;

/// Lote (x0, z0, x1, z1) inclusivo, igual server/layout.js ESCOLA.
pub const LOT: Rect = (100, 266, 130, 292);
/// Trilha de cascalho: sai da rua das casas (z 242) entre duas casas e entra no lote até a porta norte.
pub const PATH: Rect = (114, 243, 116, 267);
/// Paredes da sala (x0, z0, x1, z1); porta na parede norte, x 114..116.
const HALL: Rect = (102, 268, 128, 290);
const DOOR: (i32, i32) = (114, 116);
const FL: f32 = G as f32;
const GEO: Geo = Geo { c: Vec3::new(115.5, G as f32 + 6.0, 289.94), n: Vec3::new(0.0, 0.0, -1.0), w: 18.0, h: 9.0 };
const CENTER: Vec3 = vec3(115.5, G as f32, 279.0);
const PROF: Vec3 = vec3(126.0, G as f32, 286.8);
/// Placa na rua das casas, na boca da trilha.
const SIGN: Vec3 = vec3(118.5, G as f32, 244.5);
const DESK_X: [f32; 4] = [105.5, 109.5, 121.5, 125.0];
const DESK_Z: [f32; 3] = [273.0, 277.0, 281.0];

const CHALK: Color = Color::new(0.93, 0.95, 0.88, 1.0);
const DUST: Color = Color::new(0.93, 0.95, 0.88, 0.35);
const YEL: Color = Color::new(1.0, 0.88, 0.35, 1.0);
const CYAN: Color = Color::new(0.45, 1.0, 1.0, 1.0);
const PINK: Color = Color::new(1.0, 0.55, 0.8, 1.0);

/// Segundos de cada passo destacado / de cada "próxima aula".
const STEP_S: f32 = 4.0;
const AULA_S: f32 = 12.0;
const STEPS: [(&str, &str); 4] = [
    ("COLA O LINK NO TEU AGENTE", "Cursor, Claude Code, Codex... ele le tudo sozinho"),
    ("O AGENTE VALIDA", "POST /api/mods/validate ate dar ok (sem codigo, so JSON)"),
    ("SOBE", "cria o mod com o token de criador (segredo, nunca no chat)"),
    ("ATIVA", "aparece pra vila toda na hora. parabens, virou criador"),
];
const AULAS: [(&str, &str); 4] = [
    ("NPC", "criatura com comportamento na zona de mods: anda, fala, foge, apanha"),
    ("AVATAR", "skin pro teu player que viaja entre a vila e os jogos do Hub"),
    ("ITEM", "item ficticio que os jogos dao e o player gasta com confirmacao"),
    ("JOGO DO HUB", "teu jogo web vira um portal no corredor: /hub.txt e /hub-templates/"),
];

fn step(time: f32) -> usize {
    (time / STEP_S) as usize % 4
}

fn aula(time: f32) -> usize {
    (time / AULA_S) as usize % 4
}

pub struct Escola {
    got: bool,
    site: String,
    mods: i64,
    portals: i64,
    creators: i64,
    reads: i64,
    /// Últimos mods ativados: (nome, criador).
    last: Vec<(String, String)>,
    scr: Screen,
}

impl Default for Escola {
    fn default() -> Self {
        Self::new()
    }
}

impl Escola {
    pub fn new() -> Self {
        Escola { got: false, site: "urna-mine-verine.wilianzilv.workers.dev".into(), mods: 0, portals: 0, creators: 0, reads: 0, last: vec![], scr: Screen::new() }
    }

    fn paint(&self, time: f32) {
        draw_rectangle(0.0, 0.0, TW, TH, Color::new(0.08, 0.2, 0.13, 1.0));
        // Manchas de giz apagado
        for i in 0..9 {
            let (x, y) = ((i * 547 % 1900) as f32 + 40.0, (i * 331 % 860) as f32 + 90.0);
            draw_rectangle(x, y, 260.0, 26.0, Color::new(1.0, 1.0, 1.0, 0.025));
        }
        draw_text("ESCOLA DE AGENTES", 40.0, 92.0, 84.0, CHALK);
        let st = if !self.got { "CONECTANDO..." } else if (time * 1.5).fract() < 0.65 { "AULA AO VIVO" } else { "" };
        text_right(st, TW - 40.0, 84.0, 44.0, PINK);
        draw_rectangle(40.0, 112.0, TW - 80.0, 4.0, DUST);

        // Aula: 4 passos, um destacado por vez
        let (x0, w0) = (40.0, 1260.0);
        draw_text("COMO CRIAR UM MOD EM 4 PASSOS", x0, 180.0, 56.0, YEL);
        let cur = step(time);
        for (i, (t, sub)) in STEPS.iter().enumerate() {
            let y = 210.0 + i as f32 * 150.0;
            let on = i == cur;
            if on {
                draw_rectangle(x0, y, w0, 136.0, Color::new(1.0, 1.0, 1.0, 0.07));
                draw_rectangle(x0, y, 8.0, 136.0, YEL);
            }
            draw_circle(x0 + 70.0, y + 68.0, 46.0, if on { YEL } else { DUST });
            text_mid(&(i + 1).to_string(), x0 + 70.0, y + 90.0, 66.0, Color::new(0.08, 0.2, 0.13, 1.0));
            draw_text(t, x0 + 140.0, y + 62.0, 54.0, if on { CHALK } else { DUST });
            let sub = if i == 0 { format!("{}/skill.md", self.site) } else { sub.to_string() };
            draw_text(&fit(&sub, w0 - 160.0, 36.0), x0 + 140.0, y + 112.0, 36.0, if on { CYAN } else { DUST });
            if i < 3 {
                draw_text("v", x0 + 60.0, y + 150.0, 30.0, DUST);
            }
        }
        if cur == 0 {
            draw_text(STEPS[0].1, x0 + 140.0, 840.0, 32.0, DUST);
        }

        // Ao vivo
        let (x1, w1) = (1340.0, TW - 1380.0);
        let box_ = |x: f32, y: f32, n: i64, what: &str, col: Color| {
            draw_rectangle_lines(x, y, w1 * 0.5 - 10.0, 150.0, 3.0, DUST);
            text_mid(&n.to_string(), x + w1 * 0.25 - 5.0, y + 92.0, 86.0, col);
            text_mid(what, x + w1 * 0.25 - 5.0, y + 134.0, 28.0, CHALK);
        };
        box_(x1, 140.0, self.mods, "MODS ATIVOS", YEL);
        box_(x1 + w1 * 0.5 + 10.0, 140.0, self.portals, "PORTAIS NO AR", CYAN);
        box_(x1, 305.0, self.creators, "CRIADORES", PINK);
        box_(x1 + w1 * 0.5 + 10.0, 305.0, self.reads, "/skill.md HOJE", CHALK);

        draw_text("ULTIMOS MODS", x1, 520.0, 40.0, YEL);
        for (i, (n, c)) in self.last.iter().take(3).enumerate() {
            let y = 570.0 + i as f32 * 62.0;
            draw_text(&fit(n, w1 * 0.55, 40.0), x1, y, 40.0, CHALK);
            text_right(&fit(&format!("por {c}"), w1 * 0.42, 30.0), x1 + w1, y, 30.0, DUST);
        }
        if self.last.is_empty() {
            draw_text("nenhum ainda. o primeiro pode ser o teu", x1, 570.0, 32.0, DUST);
        }

        let (t, sub) = AULAS[aula(time)];
        let y = 770.0;
        draw_rectangle(x1, y, w1, 150.0, Color::new(1.0, 0.88, 0.35, 0.08));
        draw_rectangle_lines(x1, y, w1, 150.0, 3.0, YEL);
        draw_text("PROXIMA AULA:", x1 + 20.0, y + 52.0, 38.0, YEL);
        draw_text(t, x1 + 300.0, y + 52.0, 46.0, CHALK);
        for (j, l) in super::wrap(sub, w1 - 40.0, 30.0, 2).iter().enumerate() {
            draw_text(l, x1 + 20.0, y + 96.0 + j as f32 * 34.0, 30.0, DUST);
        }
        for k in 0..4 {
            draw_circle(x1 + w1 - 100.0 + k as f32 * 24.0, y + 40.0, 7.0, if k == aula(time) { YEL } else { DUST });
        }

        draw_rectangle(40.0, 950.0, TW - 80.0, 3.0, DUST);
        text_mid("/escola   -   /skill.md   /modding.txt   /hub.txt   /hub-templates/", TW * 0.5, 1005.0, 44.0, CHALK);
    }

    /// Robô professor de capelo: balança, vira a cabeça pra turma e aponta a lousa com a varinha.
    fn prof(&self, b: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let id = Mat4::IDENTITY;
        let (steel, dark, blue) = (rgb(0.7, 0.72, 0.76), rgb(0.28, 0.3, 0.34), rgb(0.25, 0.45, 0.85));
        let bob = (time * 2.2).sin() * 0.05;
        let m = Mat4::from_translation(PROF + vec3(0.0, bob, 0.0));
        for x in [-0.25, 0.25] {
            b.cube(&m, vec3(x, 0.45 - bob, 0.0), vec3(0.3, 0.9, 0.36), dark);
        }
        b.cube(&m, vec3(0.0, 1.35, 0.0), vec3(1.0, 1.0, 0.62), blue);
        let k = 0.6 + 0.4 * (time * 3.0).sin();
        b.glow(&m, vec3(0.0, 1.45, -0.32), vec3(0.6, 0.36, 0.02), Color::new(0.2 * k, k, 0.5 * k, 1.0));
        // Braço esquerdo solto, direito apontando a lousa (atrás dele, +z)
        let ml = m * Mat4::from_translation(vec3(-0.65, 1.75, 0.0)) * Mat4::from_rotation_x((time * 1.3).sin() * 0.15);
        b.cube(&ml, vec3(0.0, -0.4, 0.0), vec3(0.24, 0.85, 0.24), steel);
        let point = 1.9 + (time * 0.9).sin() * 0.35;
        let mr = m * Mat4::from_translation(vec3(0.65, 1.75, 0.0)) * Mat4::from_rotation_x(point) * Mat4::from_rotation_z(0.35);
        b.cube(&mr, vec3(0.0, -0.4, 0.0), vec3(0.24, 0.85, 0.24), steel);
        b.cube(&mr, vec3(0.0, -1.1, 0.0), vec3(0.06, 0.9, 0.06), rgb(0.5, 0.35, 0.2));
        b.glow(&mr, vec3(0.0, -1.58, 0.0), vec3(0.1, 0.1, 0.1), Color::new(1.0, 0.95, 0.6, 1.0));
        // Cabeça: olha a turma (-z) e às vezes a lousa
        let look = (time * 0.5).sin() * 0.6;
        let mh = m * Mat4::from_translation(vec3(0.0, 2.15, 0.0)) * Mat4::from_rotation_y(look);
        b.cube(&mh, vec3(0.0, 0.0, 0.0), vec3(0.82, 0.62, 0.62), steel);
        let blink = if (time * 0.7).fract() < 0.06 { 0.02 } else { 0.12 };
        for x in [-0.18, 0.18] {
            b.glow(&mh, vec3(x, 0.05, -0.32), vec3(0.16, blink, 0.02), CYAN);
        }
        b.glow(&mh, vec3(0.0, -0.15, -0.32), vec3(0.3, 0.04, 0.02), CYAN);
        // Capelo + borla
        b.cube(&mh, vec3(0.0, 0.36, 0.0), vec3(0.6, 0.12, 0.6), rgb(0.08, 0.08, 0.1));
        b.cube(&mh, vec3(0.0, 0.45, 0.0), vec3(1.0, 0.05, 1.0), rgb(0.08, 0.08, 0.1));
        let sway = (time * 1.7).sin() * 0.08;
        b.glow(&mh, vec3(0.48, 0.25 + sway, -0.48), vec3(0.06, 0.4, 0.06), YEL);
        b.glow(&id, PROF + vec3(0.0, 0.03, 0.0), vec3(1.6, 0.04, 1.6), Color::new(0.2, 0.6, 1.0, 0.6 + 0.2 * k));
        if eye.distance(PROF) < 30.0 {
            labels.push(Label { pos: PROF + vec3(0.0, 3.4, 0.0), text: "PROF. ROBO-CAPELO".into(), size: 20.0, color: YEL });
            labels.push(Label { pos: PROF + vec3(0.0, 2.95, 0.0), text: "\"o dever de casa e colar o link\"".into(), size: 15.0, color: CHALK });
        }
    }

    /// Carteiras com cadeira e notebook (tela piscando o "agente trabalhando").
    fn desks(&self, b: &mut Batch, time: f32) {
        let id = Mat4::IDENTITY;
        let (wood, leg, seat) = (rgb(0.62, 0.45, 0.26), rgb(0.25, 0.25, 0.28), rgb(0.2, 0.42, 0.7));
        for (i, &x) in DESK_X.iter().enumerate() {
            for (j, &z) in DESK_Z.iter().enumerate() {
                b.cube(&id, vec3(x, FL + 0.8, z), vec3(1.7, 0.1, 0.9), wood);
                for (dx, dz) in [(-0.75, -0.35), (0.75, -0.35), (-0.75, 0.35), (0.75, 0.35)] {
                    b.cube(&id, vec3(x + dx, FL + 0.38, z + dz), vec3(0.08, 0.76, 0.08), leg);
                }
                b.cube(&id, vec3(x, FL + 0.45, z - 1.0), vec3(0.8, 0.08, 0.7), seat);
                b.cube(&id, vec3(x, FL + 0.85, z - 1.33), vec3(0.8, 0.8, 0.08), seat);
                b.cube(&id, vec3(x, FL + 0.22, z - 1.0), vec3(0.08, 0.44, 0.08), leg);
                b.cube(&id, vec3(x, FL + 0.87, z - 0.1), vec3(0.7, 0.04, 0.45), rgb(0.15, 0.15, 0.18));
                b.cube(&id, vec3(x, FL + 1.1, z + 0.14), vec3(0.7, 0.45, 0.04), rgb(0.15, 0.15, 0.18));
                let ph = (time * 2.0 + (i * 3 + j) as f32 * 1.3).sin();
                let col = if ph > 0.6 { Color::new(0.4, 1.0, 0.5, 1.0) } else { Color::new(0.3, 0.7, 1.0, 1.0) };
                b.glow(&id, vec3(x, FL + 1.12, z + 0.11), vec3(0.6, 0.36, 0.02), col);
            }
        }
        // Mesa do professor
        b.cube(&id, vec3(121.0, FL + 0.9, 286.3), vec3(3.0, 0.12, 1.2), wood);
        b.cube(&id, vec3(121.0, FL + 0.45, 286.3), vec3(2.8, 0.9, 1.0), rgb(0.45, 0.3, 0.16));
        b.glow(&id, vec3(120.2, FL + 1.06, 286.3), vec3(0.25, 0.2, 0.25), Color::new(0.9, 0.15, 0.15, 1.0));
    }
}

impl Place for Escola {
    fn on_msg(&mut self, m: &Value) {
        let i = |v: &Value| v.as_f64().unwrap_or(0.0) as i64;
        let site = s(&m["site"]);
        if !site.is_empty() {
            self.site = site.trim_start_matches("https://").to_string();
        }
        self.mods = i(&m["mods"]);
        self.portals = i(&m["portals"]);
        self.creators = i(&m["creators"]);
        self.reads = i(&m["reads"]);
        self.last = m["last"].as_array().map(|a| a.iter().map(|r| (s(&r[0]), s(&r[1]))).collect()).unwrap_or_default();
        self.got = true;
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        let key = || hash_str(&format!("{}|{}|{}|{}|{}|{:?}|{}|{}|{}", self.site, self.mods, self.portals, self.creators, self.reads, self.last, self.got, step(time), aula(time)));
        if self.scr.begin_with(time, eye, &GEO, false, key) {
            self.paint(time);
            self.scr.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let id = Mat4::IDENTITY;
        if eye.distance(SIGN) < 40.0 {
            b.cube(&id, SIGN + vec3(0.0, 1.2, 0.0), vec3(0.15, 2.4, 0.15), rgb(0.4, 0.3, 0.2));
            b.cube(&id, SIGN + vec3(0.0, 2.6, 0.0), vec3(2.6, 0.7, 0.1), rgb(0.08, 0.2, 0.13));
            labels.push(Label { pos: SIGN + vec3(0.0, 3.6, 0.0), text: "v ESCOLA DE AGENTES".into(), size: 18.0, color: Color::new(1.0, 0.95, 0.75, 1.0) });
        }
        if eye.distance(CENTER) > 130.0 {
            return;
        }
        let k = 0.75 + 0.25 * (time * 2.0).sin();
        let door = vec3(115.5, FL + 5.0, 267.9);
        b.glow(&id, door, vec3(7.0, 0.12, 0.08), Color::new(k, 0.85 * k, 0.3 * k, 1.0));
        if eye.distance(door) < 60.0 {
            labels.push(Label { pos: door + vec3(0.0, 1.4, 0.0), text: "ESCOLA DE AGENTES".into(), size: 28.0, color: YEL });
            labels.push(Label { pos: door + vec3(0.0, 0.7, 0.0), text: "aprende a criar mod colando 1 link no teu agente - /escola".into(), size: 15.0, color: CHALK });
        }
        // Sino da torre (balança devagar)
        let bell = Mat4::from_translation(vec3(115.5, FL + 16.6, 269.0)) * Mat4::from_rotation_x((time * 1.5).sin() * 0.25);
        b.cube(&bell, vec3(0.0, -0.5, 0.0), vec3(0.9, 0.9, 0.9), rgb(0.85, 0.65, 0.2));
        b.glow(&bell, vec3(0.0, -1.0, 0.0), vec3(0.3, 0.2, 0.3), Color::new(k, 0.8 * k, 0.3, 1.0));

        if eye.distance(CENTER) > crate::quality::pick([45.0, 70.0, 70.0]) {
            return;
        }
        // Moldura de madeira da lousa + bandeja de giz
        let (c, w, h) = (GEO.c, GEO.w, GEO.h);
        let fz = c.z + 0.02;
        let wood = rgb(0.5, 0.33, 0.18);
        for (p, sz) in [
            (vec3(c.x, c.y + h * 0.5, fz), vec3(w + 0.5, 0.25, 0.14)),
            (vec3(c.x, c.y - h * 0.5, fz), vec3(w + 0.5, 0.25, 0.14)),
            (vec3(c.x - w * 0.5, c.y, fz), vec3(0.25, h + 0.5, 0.14)),
            (vec3(c.x + w * 0.5, c.y, fz), vec3(0.25, h + 0.5, 0.14)),
        ] {
            b.cube(&id, p, sz, wood);
        }
        b.cube(&id, vec3(c.x, c.y - h * 0.5 - 0.2, c.z - 0.2), vec3(w, 0.1, 0.4), wood);
        for i in 0..4 {
            b.cube(&id, vec3(c.x - 4.0 + i as f32 * 0.6, c.y - h * 0.5 - 0.1, c.z - 0.25), vec3(0.35, 0.08, 0.08), [CHALK, YEL, PINK, CYAN][i]);
        }
        self.desks(b, time);
        self.prof(b, labels, time, eye);
        if crate::quality::tier() != crate::quality::LOW {
            for lx in [108.5, 122.5] {
                b.glow(&id, vec3(lx, FL + 11.7, 279.0), vec3(6.0, 0.15, 0.6), Color::new(1.0, 0.97, 0.88, 1.0));
                trans.glow(&id, vec3(lx, FL + 11.2, 279.0), vec3(7.0, 0.8, 1.6), Color::new(1.0, 0.95, 0.8, 0.08));
            }
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.scr.draw(&GEO, time, eye, 130.0);
    }
}

pub fn build(w: &mut World) {
    let g = G;
    super::clear_lot(w, LOT, 22, GRASS);
    // Trilha da rua das casas até a porta (sem árvore no caminho)
    fill(w, (PATH.0, g, PATH.1), (PATH.2, g + 9, PATH.3), AIR);
    fill(w, (PATH.0, g - 1, PATH.1), (PATH.2, g - 1, PATH.3), GRAVEL);
    // Sala: base de tijolo, paredes brancas, quinas de tronco, teto de pedra
    let (x0, z0, x1, z1) = HALL;
    fill(w, (x0, g - 1, z0), (x1, g - 1, z1), PLANKS);
    fill(w, (x0, g, z0), (x1, g + 11, z1), WOOL);
    fill(w, (x0, g, z0), (x1, g + 1, z1), BRICK);
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        fill(w, (x, g, z), (x, g + 12, z), LOG);
    }
    fill(w, (x0 + 1, g, z0 + 1), (x1 - 1, g + 11, z1 - 1), AIR);
    fill(w, (x0, g + 12, z0), (x1, g + 12, z1), STONE);
    fill(w, (x0, g + 13, z0), (x1, g + 13, z1), BRICK);
    fill(w, (x0 + 1, g + 13, z0 + 1), (x1 - 1, g + 13, z1 - 1), AIR);
    // Janelas: oeste e norte (dos lados da porta); leste só no alto
    for z in [271, 275, 279, 283, 287] {
        fill(w, (x0, g + 3, z), (x0, g + 8, z + 1), GLASS);
    }
    for x in [104, 108, 120, 124] {
        fill(w, (x, g + 3, z0), (x + 1, g + 8, z0), GLASS);
    }
    for z in [271, 276, 281, 286] {
        fill(w, (x1, g + 8, z), (x1, g + 10, z + 1), GLASS);
    }
    // Porta norte com verga de areia e degrau
    fill(w, (DOOR.0, g, z0), (DOOR.1, g + 3, z0), AIR);
    fill(w, (DOOR.0 - 1, g + 4, z0), (DOOR.1 + 1, g + 4, z0), SAND);
    // Parede da lousa (sul): fundo preto
    fill(w, (106, g + 1, z1), (125, g + 11, z1), crate::world::BLACK);
    // Torre do sino sobre a porta
    fill(w, (113, g + 13, z0), (118, g + 18, z0 + 2), BRICK);
    fill(w, (114, g + 14, z0), (117, g + 17, z0 + 2), AIR);
    fill(w, (113, g + 19, z0), (118, g + 19, z0 + 2), SAND);
    fill(w, (115, g + 20, z0 + 1), (116, g + 20, z0 + 1), SAND);
    // Jardim: canteiros de areia com árvore baixa nas quinas do pátio
    for (x, z) in [(101, 267), (129, 267)] {
        w.set(x, g - 1, z, SAND);
        fill(w, (x, g, z), (x, g + 2, z), LOG);
        fill(w, (x, g + 3, z), (x, g + 3, z), LEAVES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout;

    fn hit((ax0, az0, ax1, az1): Rect, (bx0, bz0, bx1, bz1): Rect) -> bool {
        ax0 <= bx1 && ax1 >= bx0 && az0 <= bz1 && az1 >= bz0
    }

    #[test]
    fn escola_lot_free() {
        for x in LOT.0..=LOT.2 {
            for z in LOT.1..=LOT.3 {
                assert!(layout::free(x, z, 0), "lote ocupado em {x},{z}");
            }
        }
        for r in layout::FOOTPRINTS.iter().chain(&layout::ROADS).chain(&layout::PATHS) {
            assert!(!hit(LOT, *r), "lote cruza {r:?}");
        }
        for &(hx, hz, _) in layout::HOUSES.iter().chain([&layout::TOWER_HOUSE]) {
            assert!(!hit(PATH, (hx - 1, hz - 1, hx + 7, hz + 7)), "trilha cruza casa {hx},{hz}");
        }
        for r in &layout::ROADS {
            assert!(!hit(PATH, *r), "trilha em cima da rua {r:?}");
        }
        assert!(hit((PATH.0, PATH.1 - 1, PATH.2, PATH.1 - 1), layout::ROADS[4]), "trilha nao encosta na rua das casas");
    }

    #[test]
    fn escola_door_reachable() {
        let w = World::generate();
        let x = 115;
        assert!(w.solid(x, G - 1, PATH.1 - 2), "rua das casas sem chao");
        for z in PATH.1 - 2..=280 {
            assert!(!w.solid(x, G, z) && !w.solid(x, G + 1, z), "caminho bloqueado em {x},{z}");
            assert!(w.solid(x, G - 1, z), "sem chao em {x},{z}");
        }
        for (dx, dz) in [(-1, 0), (1, 0)] {
            assert!(!w.solid(x + dx, G, HALL.1 + dz), "porta estreita");
        }
        assert!(w.solid(GEO.c.x as i32, GEO.c.y as i32, 290), "lousa sem parede");
        assert!(!w.solid(PROF.x as i32, G, PROF.z as i32), "professor dentro da parede");
    }
}
