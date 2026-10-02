//! RINGUE DA VILA (server/ring.js): boxe PvP entre jogadores no noroeste, acima do Congresso. Ringue elevado
//! (lona, postes, cordas, degraus), arquibancadas e placar holográfico ao lado (render target, nunca HUD).
//! O servidor manda em tudo (fila, rounds, vida, nocaute); aqui só desenha, teleporta/empurra quando ele manda
//! e envia o jab de quem joga com personagem sem PvP próprio. O lote inteiro é escudo (shield::protected).
//! Lote, lona, trilha e placa moram aqui; server/layout.js espelha RINGUE.

use super::{Geo, Place, Screen, TW, fill, hash_str, s, text_mid};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{CYAN, DIM, GOLD, SOFT, fit, frame, frame_at, holo_fx, text_right};
use crate::layout::Rect;
use crate::models::rgb;
use crate::world::*;
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::FRAC_PI_2;

/// Lote (x0, z0, x1, z1) inclusivo, igual server/layout.js RINGUE.
pub const LOT: Rect = (56, 38, 92, 70);
/// Lona (blocos) elevada 2 blocos: pisa em y = G + 2.
pub const LONA: Rect = (68, 48, 79, 59);
/// Trilha de cascalho: sobe da Avenida dos Poderes (z 96) até o lote.
pub const PATH: Rect = (90, 71, 92, 95);
const FL: f32 = G as f32 + 2.0;
const CENTER: Vec3 = vec3(74.0, FL, 54.0);
/// Placar ao norte do ringue, virado pro sul (pra lona e pra quem chega pela trilha).
const GEO: Geo = Geo { c: Vec3::new(74.0, G as f32 + 10.0, 42.0), n: Vec3::new(0.0, 0.0, 1.0), w: 20.0, h: 10.0 };
const SIGN: Vec3 = vec3(93.5, G as f32, 94.5);

const RED: Color = Color::new(1.0, 0.3, 0.25, 1.0);
const BLUE: Color = Color::new(0.3, 0.55, 1.0, 1.0);

pub fn in_lot(x: i32, z: i32) -> bool {
    (LOT.0..=LOT.2).contains(&x) && (LOT.1..=LOT.3).contains(&z)
}

/// Pé dentro das cordas e em cima da lona (mesma regra do inLona do servidor).
fn on_lona(p: Vec3) -> bool {
    p.x >= LONA.0 as f32 && p.x < LONA.2 as f32 + 1.0 && p.z >= LONA.1 as f32 && p.z < LONA.3 as f32 + 1.0 && p.y >= FL - 0.5
}

pub struct Ringue {
    got: bool,
    ph: String,
    round: i64,
    /// Segundos até o fim da fase quando o snapshot chegou, e o relógio do jogo nessa hora.
    left: f32,
    at: f32,
    now: f32,
    /// Lutadores: (nome, vida, rounds ganhos).
    f: Vec<(String, f32, i64)>,
    q: Vec<String>,
    king: Option<(String, i64)>,
    /// Maiores sequências: (nome, vitórias, melhor sequência).
    top: Vec<(String, i64, i64)>,
    /// Últimas lutas: (vencedor, perdedor, placar).
    last: Vec<(String, String, String)>,
    tp: Option<(Vec3, bool)>,
    knock: Option<Vec2>,
    /// Faixa do round (só chega pros 2 lutadores); o main mostra no banner.
    pub msg: Option<String>,
    /// Servidor curou (começo de round): o main enche a vida do Steve.
    pub heal: bool,
    /// Clique/toque de ataque neste frame (o main preenche antes do update).
    pub jab: bool,
    scr: Screen,
}

impl Default for Ringue {
    fn default() -> Self {
        Self::new()
    }
}

impl Ringue {
    pub fn new() -> Self {
        Ringue { got: false, ph: "idle".into(), round: 0, left: 0.0, at: 0.0, now: 0.0, f: vec![], q: vec![], king: None, top: vec![], last: vec![], tp: None, knock: None, msg: None, heal: false, jab: false, scr: Screen::new() }
    }

    fn remaining(&self) -> f32 {
        (self.left - (self.now - self.at)).max(0.0)
    }

    fn status(&self) -> String {
        let n = self.remaining().ceil() as i64;
        match self.ph.as_str() {
            "count" => format!("ROUND {} EM {n}", self.round),
            "fight" => "LUTA!".into(),
            "break" => "INTERVALO".into(),
            "end" => "FIM DE LUTA".into(),
            _ => match (&self.king, self.q.len()) {
                (Some((k, _)), _) => format!("{} ESPERA DESAFIANTE", k.to_uppercase()),
                (None, 1) => "FALTA 1 LUTADOR".into(),
                _ => "PISA NA LONA OU /ringue".into(),
            },
        }
    }

    fn paint(&self, time: f32) {
        draw_text("RINGUE DA VILA", 40.0, 90.0, 84.0, CYAN);
        text_right(&self.status(), TW - 40.0, 84.0, 56.0, GOLD);
        draw_rectangle(40.0, 120.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

        let king = self.king.as_ref().map(|k| k.0.as_str());
        for (i, (x, col, corner)) in [(40.0, RED, "CORNER VERMELHO"), (1148.0, BLUE, "CORNER AZUL")].into_iter().enumerate() {
            let w = 860.0;
            frame(x, 150.0, w, 470.0, "");
            draw_rectangle(x, 150.0, w, 12.0, col);
            draw_text(corner, x + 30.0, 210.0, 34.0, col);
            let Some((name, hp, rw)) = self.f.get(i) else {
                draw_text("VAGA", x + 30.0, 320.0, 96.0, DIM);
                draw_text("pisa na lona ou manda /ringue", x + 30.0, 400.0, 40.0, SOFT);
                continue;
            };
            if king == Some(name.as_str()) {
                text_right(&format!("REI x{}", self.king.as_ref().map_or(0, |k| k.1)), x + w - 30.0, 210.0, 40.0, GOLD);
            }
            draw_text(&fit(&name.to_uppercase(), w - 60.0, 96.0), x + 30.0, 320.0, 96.0, WHITE);
            let k = (hp / 100.0).clamp(0.0, 1.0);
            let bw = w - 60.0;
            draw_rectangle(x + 30.0, 360.0, bw, 70.0, Color::new(0.45, 1.0, 1.0, 0.1));
            draw_rectangle(x + 30.0, 360.0, bw * k, 70.0, Color::new(1.0 - k, 0.3 + 0.7 * k, 0.6 * k + 0.2, 0.9));
            draw_rectangle_lines(x + 30.0, 360.0, bw, 70.0, 2.0, DIM);
            draw_text(&format!("{:.0} / 100", hp), x + 50.0, 410.0, 48.0, WHITE);
            draw_text("ROUNDS", x + 30.0, 530.0, 40.0, SOFT);
            for r in 0..2 {
                let on = (r as i64) < *rw;
                draw_circle(x + 230.0 + r as f32 * 80.0, 518.0, 28.0, if on { GOLD } else { Color::new(0.45, 1.0, 1.0, 0.15) });
            }
        }
        text_mid("ROUND", TW * 0.5, 220.0, 44.0, SOFT);
        text_mid(&if self.round > 0 { self.round.to_string() } else { "-".into() }, TW * 0.5, 340.0, 130.0, WHITE);
        text_mid("de 3", TW * 0.5, 390.0, 34.0, SOFT);
        let n = self.remaining().ceil() as i64;
        let (clock, col) = match self.ph.as_str() {
            "fight" => (format!("{}:{:02}", n / 60, n % 60), if n <= 10 { RED } else { WHITE }),
            "count" => (n.to_string(), GOLD),
            _ => ("--".into(), DIM),
        };
        text_mid(&clock, TW * 0.5, 500.0, 92.0, col);
        text_mid("VS", TW * 0.5, 590.0, 50.0, GOLD);

        frame(40.0, 650.0, 620.0, 340.0, &format!("FILA ({})", self.q.len()));
        if self.q.is_empty() {
            draw_text("vazia - pisa na lona ou /ringue", 70.0, 740.0, 36.0, SOFT);
        }
        for (i, n) in self.q.iter().take(5).enumerate() {
            draw_text(&fit(&format!("{}. {n}", i + 1), 560.0, 44.0), 70.0, 740.0 + i as f32 * 50.0, 44.0, if i == 0 { GOLD } else { WHITE });
        }

        frame(680.0, 650.0, 660.0, 340.0, "REI DO RINGUE");
        match &self.king {
            Some((k, st)) => {
                draw_text(&fit(&k.to_uppercase(), 600.0, 64.0), 710.0, 750.0, 64.0, GOLD);
                draw_text(&format!("sequencia: {st}"), 710.0, 795.0, 36.0, SOFT);
            }
            None => {
                draw_text("trono vazio", 710.0, 750.0, 52.0, DIM);
            }
        }
        for (i, (w, l, sc)) in self.last.iter().take(3).enumerate() {
            draw_text(&fit(&format!("{w} venceu {l}  {sc}"), 600.0, 32.0), 710.0, 860.0 + i as f32 * 42.0, 32.0, WHITE);
        }

        frame(1360.0, 650.0, TW - 1400.0, 340.0, "MAIORES SEQUENCIAS");
        if self.top.is_empty() {
            draw_text("ninguem ainda", 1390.0, 740.0, 36.0, SOFT);
        }
        for (i, (n, w, b)) in self.top.iter().take(5).enumerate() {
            let y = 740.0 + i as f32 * 50.0;
            draw_text(&fit(&format!("{}. {n}", i + 1), 380.0, 42.0), 1390.0, y, 42.0, if i == 0 { GOLD } else { WHITE });
            text_right(&format!("x{b}  ({w} vit)"), TW - 70.0, y, 36.0, SOFT);
        }
        holo_fx(time, 0.0);
    }

    fn sign(&self, b: &mut Batch, labels: &mut Vec<Label>, eye: Vec3) {
        if eye.distance(SIGN) < 40.0 {
            let id = Mat4::IDENTITY;
            b.cube(&id, SIGN + vec3(0.0, 1.2, 0.0), vec3(0.15, 2.4, 0.15), rgb(0.4, 0.3, 0.2));
            b.cube(&id, SIGN + vec3(0.0, 2.6, 0.0), vec3(2.6, 0.7, 0.1), rgb(0.35, 0.08, 0.08));
            labels.push(Label { pos: SIGN + vec3(0.0, 3.6, 0.0), text: "^ RINGUE DA VILA (PvP)".into(), size: 18.0, color: Color::new(1.0, 0.95, 0.75, 1.0) });
        }
    }
}

impl Place for Ringue {
    fn on_msg(&mut self, m: &Value) {
        match m["a"].as_str() {
            Some("tp") => {
                let p = &m["p"];
                let v = vec3(p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32, p[2].as_f64().unwrap_or(0.0) as f32);
                self.tp = Some((v, m["heal"].as_i64() == Some(1)));
            }
            Some("msg") => self.msg = Some(s(&m["m"])),
            Some("hit") => self.knock = Some(vec2(m["d"][0].as_f64().unwrap_or(0.0) as f32, m["d"][1].as_f64().unwrap_or(0.0) as f32)),
            Some(_) => {}
            None => {
                let i = |v: &Value| v.as_i64().unwrap_or(0);
                self.ph = s(&m["ph"]);
                self.round = i(&m["round"]);
                self.left = m["left"].as_f64().unwrap_or(0.0) as f32 / 1000.0;
                self.at = self.now;
                self.f = m["f"].as_array().map(|a| a.iter().map(|r| (s(&r[1]), r[2].as_f64().unwrap_or(0.0) as f32, i(&r[3]))).collect()).unwrap_or_default();
                self.q = m["q"].as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default();
                self.king = m["king"].as_array().map(|k| (s(&k[0]), i(&k[1])));
                self.top = m["top"].as_array().map(|a| a.iter().map(|r| (s(&r[0]), i(&r[1]), i(&r[2]))).collect()).unwrap_or_default();
                self.last = m["last"].as_array().map(|a| a.iter().map(|r| (s(&r[0]), s(&r[1]), s(&r[2]))).collect()).unwrap_or_default();
                self.got = true;
            }
        }
    }

    fn update(&mut self, p: &mut crate::player::Player, _dt: f32, time: f32, online: bool, out: &mut Vec<Value>) {
        self.now = time;
        if let Some((to, heal)) = self.tp.take() {
            p.pos = to;
            p.vel = Vec3::ZERO;
            self.heal |= heal;
        }
        if let Some(d) = self.knock.take() {
            p.knock += vec3(d.x, 0.0, d.y) * 6.0 + Vec3::Y * 3.0;
        }
        if std::mem::take(&mut self.jab) && online && self.ph == "fight" && on_lona(p.pos) {
            out.push(json!({"t": "pl", "k": "ringue", "a": "soco"}));
        }
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        self.now = time;
        let rem = self.remaining().ceil();
        let key = || hash_str(&format!("{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{rem}", self.got, self.ph, self.round, self.f, self.q, self.king, self.top, self.last));
        if self.scr.begin_with(time, eye, &GEO, false, key) {
            self.paint(time);
            self.scr.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        self.sign(b, labels, eye);
        if eye.distance(CENTER) > 160.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let fight = self.ph == "fight";
        let flash = if self.ph == "count" { 0.5 + 0.5 * (time * 8.0).sin() } else { 0.0 };
        frame_at(b, trans, &(Mat4::from_translation(vec3(GEO.c.x, 0.0, GEO.c.z)) * Mat4::from_rotation_y(FRAC_PI_2)), (GEO.w, GEO.h, GEO.c.y), time, flash);
        if eye.distance(CENTER) > crate::quality::pick([60.0, 100.0, 100.0]) {
            return;
        }
        // Cordas (3 alturas) por dentro dos postes; protetores dos postes: vermelho, azul e 2 neutros
        let (x0, z0, x1, z1) = (LONA.0 as f32 + 0.5, LONA.1 as f32 + 0.5, LONA.2 as f32 + 0.5, LONA.3 as f32 + 0.5);
        let (cx, cz, wx, wz) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5, x1 - x0, z1 - z0);
        for (k, col) in [(0.55, RED), (1.05, WHITE), (1.55, BLUE)] {
            let y = FL + k;
            for (c, sz) in [(vec3(cx, y, z0), vec3(wx, 0.08, 0.08)), (vec3(cx, y, z1), vec3(wx, 0.08, 0.08)), (vec3(x0, y, cz), vec3(0.08, 0.08, wz)), (vec3(x1, y, cz), vec3(0.08, 0.08, wz))] {
                b.cube(&id, c, sz, col);
            }
        }
        for (x, z, col) in [(x0, z0, RED), (x1, z1, BLUE), (x1, z0, WHITE), (x0, z1, WHITE)] {
            b.cube(&id, vec3(x, FL + 1.05, z), vec3(0.75, 1.4, 0.75), col);
            b.glow(&id, vec3(x, FL + 2.05, z), vec3(0.5, 0.12, 0.5), col);
        }
        // Logo na lona
        let k = if fight { 0.8 + 0.2 * (time * 6.0).sin() } else { 0.55 };
        b.glow(&id, vec3(cx, FL + 0.02, cz), vec3(4.0, 0.02, 4.0), Color::new(0.75 * k, 0.12 * k, 0.12 * k, 1.0));
        b.glow(&id, vec3(cx, FL + 0.03, cz), vec3(3.2, 0.02, 3.2), Color::new(0.95, 0.95, 0.9, 1.0));
        // Holofotes em cima (luz em volume só em média/alta)
        for (dx, dz) in [(-3.0, -3.0), (3.0, -3.0), (-3.0, 3.0), (3.0, 3.0)] {
            b.cube(&id, vec3(cx + dx, FL + 8.6, cz + dz), vec3(0.8, 0.5, 0.8), rgb(0.15, 0.15, 0.18));
            b.glow(&id, vec3(cx + dx, FL + 8.32, cz + dz), vec3(0.6, 0.06, 0.6), Color::new(1.0, 0.97, 0.85, 1.0));
        }
        if crate::quality::tier() != crate::quality::LOW {
            trans.glow(&id, vec3(cx, FL + 4.2, cz), vec3(wx - 1.0, 8.0, wz - 1.0), Color::new(1.0, 0.95, 0.8, if fight { 0.07 } else { 0.04 }));
        }
        if eye.distance(CENTER) < 60.0 {
            labels.push(Label { pos: CENTER + vec3(0.0, 4.6, 0.0), text: "RINGUE DA VILA".into(), size: 30.0, color: GOLD });
            let sub = if self.f.len() == 2 { format!("{}  x  {}", self.f[0].0, self.f[1].0) } else { "pisa na lona ou /ringue - melhor de 3, quem vence fica".into() };
            labels.push(Label { pos: CENTER + vec3(0.0, 3.9, 0.0), text: sub, size: 16.0, color: SOFT });
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.scr.draw(&GEO, time, eye, 140.0);
    }
}

pub fn build(w: &mut World) {
    let g = G;
    super::clear_lot(w, LOT, 16, GRASS);
    // Trilha da avenida até o lote e calçada até os degraus do sul
    fill(w, (PATH.0, g, PATH.1), (PATH.2, g + 9, PATH.3), AIR);
    fill(w, (PATH.0, g - 1, PATH.1), (PATH.2, g - 1, PATH.3), GRAVEL);
    fill(w, (PATH.0, g - 1, 62), (PATH.2, g - 1, LOT.3), GRAVEL);
    // Piso de pedregulho em volta do ringue
    fill(w, (64, g - 1, 44), (83, g - 1, 64), COBBLE);
    fill(w, (72, g - 1, 62), (PATH.2, g - 1, 64), GRAVEL);
    // Ringue: saia preta, lona de lã, postes nas quinas, degraus no sul e no norte
    let (x0, z0, x1, z1) = LONA;
    fill(w, (x0, g, z0), (x1, g, z1), crate::world::BLACK);
    fill(w, (x0, g + 1, z0), (x1, g + 1, z1), WOOL);
    for (x, z) in [(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
        fill(w, (x, g + 2, z), (x, g + 3, z), LOG);
    }
    fill(w, (72, g, z1 + 1), (75, g, z1 + 1), PLANKS);
    fill(w, (72, g, z0 - 1), (75, g, z0 - 1), PLANKS);
    // Arquibancadas (4 degraus) a oeste e a leste, viradas pro ringue
    for i in 0..4 {
        fill(w, (63 - i, g, 46), (63 - i, g + i, 61), PLANKS);
        fill(w, (84 + i, g, 46), (84 + i, g + i, 61), PLANKS);
        fill(w, (63 - i, g + i, 46), (63 - i, g + i, 46), BRICK);
        fill(w, (84 + i, g + i, 61), (84 + i, g + i, 61), BRICK);
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
    fn ringue_lot_free() {
        for x in LOT.0..=LOT.2 {
            for z in LOT.1..=LOT.3 {
                assert!(layout::free(x, z, 0), "lote ocupado em {x},{z}");
            }
        }
        for r in layout::FOOTPRINTS.iter().chain(&layout::ROADS).chain(&layout::PATHS).chain([&super::super::escola::LOT]) {
            assert!(!hit(LOT, *r), "lote cruza {r:?}");
            assert!(!hit(PATH, *r), "trilha cruza {r:?}");
        }
        assert!(!hit(PATH, layout::CONGRESSO) && PATH.0 > layout::CONGRESSO.2 + 2, "trilha encosta no congresso");
        assert!(hit((PATH.0, PATH.3 + 1, PATH.2, PATH.3 + 1), layout::ROADS[6]), "trilha nao encosta na avenida");
    }

    #[test]
    fn ringue_reachable_and_shielded() {
        let w = World::generate();
        let x = 91;
        for z in PATH.1 - 2..=PATH.3 + 2 {
            assert!(!w.solid(x, G, z) && !w.solid(x, G + 1, z), "trilha bloqueada em {x},{z}");
            assert!(w.solid(x, G - 1, z), "sem chao em {x},{z}");
        }
        for x in 72..=x {
            assert!(!w.solid(x, G, 63) && !w.solid(x, G + 1, 63), "calcada bloqueada em {x}");
        }
        // degrau (1) e lona (2) sem nada em cima até a altura da cabeça
        assert!(w.solid(73, G, LONA.3 + 1) && !w.solid(73, G + 1, LONA.3 + 1));
        assert!(w.solid(73, G + 1, 54) && !w.solid(73, G + 2, 54) && !w.solid(73, G + 3, 54));
        assert!(on_lona(CENTER) && !on_lona(vec3(74.0, G as f32, 63.0)));
        assert!(crate::shield::protected(ivec3(73, G + 1, 54)) && crate::shield::protected(ivec3(LOT.0, G, LOT.3)));
        assert!(!crate::shield::protected(ivec3(LOT.0 - 1, G, 54)));
        let mut w = w;
        w.guard = true;
        w.set(73, G + 1, 54, AIR);
        w.set(60, G, 66, STONE);
        assert!(w.solid(73, G + 1, 54) && !w.solid(60, G, 66), "escudo do ringue furou");
    }

    #[test]
    fn ringue_snapshot_and_orders() {
        let mut r = Ringue::new();
        r.now = 10.0;
        r.on_msg(&json!({"k": "ringue", "ph": "fight", "round": 2, "left": 42500, "f": [[1, "Ana", 70, 1], [2, "Bia", 100, 0]], "q": ["Caio"], "king": ["Ana", 3], "top": [["Ana", 5, 3]], "last": [["Ana", "Rui", "2-0"]]}));
        assert_eq!(r.f[0], ("Ana".into(), 70.0, 1));
        assert_eq!(r.king, Some(("Ana".into(), 3)));
        r.now = 12.0;
        assert!((r.remaining() - 40.5).abs() < 1e-3);
        r.on_msg(&json!({"k": "ringue", "a": "tp", "p": [69.5, 22, 49.5], "heal": 1}));
        r.on_msg(&json!({"k": "ringue", "a": "msg", "m": "ROUND 2: LUTA!"}));
        let mut p = crate::player::Player::new();
        let mut out = vec![];
        r.jab = true;
        r.update(&mut p, 0.016, 12.0, true, &mut out);
        assert_eq!(p.pos, vec3(69.5, 22.0, 49.5));
        assert!(r.heal && r.msg.as_deref() == Some("ROUND 2: LUTA!"));
        assert_eq!(out, vec![json!({"t": "pl", "k": "ringue", "a": "soco"})]);
        assert!(!r.jab);
        p.pos = vec3(74.0, G as f32, 63.0);
        r.jab = true;
        r.update(&mut p, 0.016, 12.1, true, &mut out);
        assert_eq!(out.len(), 1, "fora da lona nao soca");
    }
}
