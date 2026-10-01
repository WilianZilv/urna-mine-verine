//! Banco Central da Vila: ranking, ledger na fachada, caixa eletrônico e poupança fictícia (server/banco.js).
//! Prédio clássico (pódio, colunata de "mármore", frontão) com telão dourado acima da porta; dentro, porta de
//! cofre redonda com volante girando e 2 caixas eletrônicos (pisar 1 s no tapete pede o extrato ao servidor).

use super::{Geo, Place, Screen, TH, TW, fill, hash_str, s, text_mid};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{fit, text_right};
use crate::models::rgb;
use crate::player::Player;
use crate::world::*;
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::{PI, TAU};

/// Altura do piso de dentro (pódio de 2 blocos).
const FL: f32 = G as f32 + 2.0;
const GEO: Geo = Geo { c: Vec3::new(183.8, G as f32 + 13.0, 237.5), n: Vec3::new(-1.0, 0.0, 0.0), w: 18.0, h: 9.0 };
/// Tapetes dos caixas eletrônicos (x, z); a máquina fica 1.9 bloco pra parede.
const PADS: [(f32, f32); 2] = [(199.5, 226.5), (199.5, 248.5)];
const VAULT: Vec3 = Vec3::new(212.9, FL + 4.5, 237.5);

const GOLD: Color = Color::new(1.0, 0.82, 0.3, 1.0);
const DIMG: Color = Color::new(1.0, 0.82, 0.3, 0.35);
const SOFT: Color = Color::new(0.95, 0.9, 0.78, 1.0);

pub struct Banco {
    tr: i64,
    rich: Vec<(String, i64)>,
    led: Vec<(String, String, i64)>,
    sv: i64,
    nsv: i64,
    rate: f64,
    got: bool,
    recv: f64,
    scr: Screen,
    stand: f32,
    sent: f32,
}

impl Banco {
    pub fn new() -> Self {
        Banco { tr: 0, rich: vec![], led: vec![], sv: 0, nsv: 0, rate: 2.0, got: false, recv: -10.0, scr: Screen::new(), stand: 0.0, sent: -100.0 }
    }

    fn paint(&self, time: f32) {
        let flash = (1.0 - (get_time() - self.recv) as f32 / 1.5).clamp(0.0, 1.0);
        draw_rectangle(0.0, 0.0, TW, TH, Color::new(0.08, 0.05, 0.0, 0.92));
        draw_text("BANCO CENTRAL DA VILA  -  MOEDA FICTICIA", 40.0, 88.0, 80.0, GOLD);
        let st = if !self.got { "CONECTANDO..." } else if (time * 1.5).fract() < 0.65 { "AO VIVO" } else { "" };
        text_right(st, TW - 40.0, 84.0, 46.0, Color::new(1.0, 0.4, 0.3, 1.0));
        draw_rectangle(40.0, 110.0, TW - 80.0, 5.0, GOLD);

        // Cofre + poupança
        let (x0, w0) = (40.0, 600.0);
        panel(x0, 135.0, w0, 420.0, "COFRE DA IA");
        let t = self.tr.to_string();
        let sz = 220.0f32.min(220.0 * (w0 - 60.0) / measure_text(&t, None, 220, 1.0).width.max(1.0));
        text_mid(&t, x0 + w0 * 0.5, 380.0, sz, WHITE);
        text_mid("moedas ficticias  -  reserva 300", x0 + w0 * 0.5, 450.0, 34.0, SOFT);
        let (bx, bw) = (x0 + 40.0, w0 - 80.0);
        draw_rectangle(bx, 480.0, bw, 28.0, Color::new(1.0, 0.82, 0.3, 0.12));
        draw_rectangle(bx, 480.0, bw * (self.tr as f32 / 2000.0).clamp(0.0, 1.0), 28.0, if self.tr < 400 { Color::new(1.0, 0.35, 0.25, 0.9) } else { GOLD });
        draw_rectangle(bx + bw * 0.15 - 2.0, 470.0, 4.0, 48.0, WHITE);
        draw_text("reserva", bx + bw * 0.15 + 8.0, 540.0, 26.0, DIMG);

        panel(x0, 575.0, w0, 370.0, "POUPANCA");
        draw_text(&self.sv.to_string(), x0 + 30.0, 720.0, 130.0, WHITE);
        draw_text(&format!("guardados por {} poupador{}", self.nsv, if self.nsv == 1 { "" } else { "es" }), x0 + 30.0, 785.0, 36.0, SOFT);
        draw_text(&format!("{}%/h pagos pelo cofre", self.rate), x0 + 30.0, 850.0, 50.0, GOLD);
        draw_text("(enquanto a IA tiver troco)", x0 + 30.0, 900.0, 32.0, DIMG);

        // Ranking (patrimônio = carteira + poupança + ações)
        let (x1, w1) = (670.0, 680.0);
        panel(x1, 135.0, w1, 810.0, "RANKING DOS MAIS RICOS");
        let max = self.rich.first().map_or(1, |r| r.1.max(1)) as f32;
        let medal = [GOLD, Color::new(0.85, 0.88, 0.92, 1.0), Color::new(0.85, 0.55, 0.3, 1.0)];
        for (i, (n, v)) in self.rich.iter().take(8).enumerate() {
            let y = 200.0 + i as f32 * 92.0;
            let col = *medal.get(i).unwrap_or(&SOFT);
            draw_text(&format!("{}.", i + 1), x1 + 24.0, y + 50.0, 54.0, col);
            draw_text(&fit(n, 340.0, 48.0), x1 + 100.0, y + 48.0, 48.0, WHITE);
            text_right(&v.to_string(), x1 + w1 - 24.0, y + 48.0, 48.0, GOLD);
            draw_rectangle(x1 + 100.0, y + 62.0, (w1 - 124.0) * (*v as f32 / max).clamp(0.0, 1.0), 10.0, Color::new(col.r, col.g, col.b, 0.5));
        }
        if self.rich.is_empty() {
            draw_text("ninguem tem nada (ainda)", x1 + 30.0, 260.0, 40.0, SOFT);
        }
        draw_text("carteira + poupanca + acoes", x1 + 24.0, 930.0, 28.0, DIMG);

        // Ledger público (mais novo em cima)
        let (x2, w2) = (1380.0, TW - 1420.0);
        panel(x2, 135.0, w2, 810.0, "LEDGER AO VIVO");
        for (i, (who, what, amt)) in self.led.iter().rev().take(6).enumerate() {
            let y = 196.0 + i as f32 * 124.0;
            draw_rectangle(x2 + 16.0, y, w2 - 32.0, 112.0, Color::new(0.35, 0.22, 0.0, 0.3));
            draw_rectangle(x2 + 16.0, y, 6.0, 112.0, if i == 0 { Color::new(1.0, 0.95, 0.7, 0.5 + 0.5 * flash) } else { GOLD });
            draw_text(&fit(who, w2 - 220.0, 42.0), x2 + 36.0, y + 46.0, 42.0, GOLD);
            text_right(&amt.to_string(), x2 + w2 - 36.0, y + 46.0, 42.0, WHITE);
            draw_text(&fit(what, w2 - 72.0, 32.0), x2 + 36.0, y + 92.0, 32.0, SOFT);
        }
        if self.led.is_empty() {
            draw_text("nenhuma moeda se mexeu", x2 + 30.0, 260.0, 36.0, SOFT);
        }

        text_mid("/poupar n   /sacar n   /ranking   -   pisa no caixa eletronico", TW * 0.5, 1002.0, 46.0, GOLD);
        gold_fx(time, flash);
    }

    fn vault(&self, b: &mut Batch, trans: &mut Batch, time: f32) {
        let id = Mat4::IDENTITY;
        let (steel, dark, brass) = (rgb(0.6, 0.62, 0.66), rgb(0.3, 0.31, 0.34), rgb(0.9, 0.7, 0.28));
        let r = 3.6;
        let n = crate::quality::pick([14, 22, 30]);
        for i in 0..n {
            let m = Mat4::from_translation(VAULT) * Mat4::from_rotation_x(i as f32 / n as f32 * TAU);
            b.cube(&m, vec3(-0.2, r, 0.0), vec3(0.6, 0.7, TAU * r / n as f32 + 0.1), steel);
        }
        for j in 0..7 {
            let y = -r + (j as f32 + 0.5) * (2.0 * r / 7.0);
            let half = (r * r - y * y).max(0.0).sqrt();
            b.cube(&id, VAULT + vec3(0.0, y, 0.0), vec3(0.3, 2.0 * r / 7.0 + 0.02, 2.0 * half * 0.95), dark);
        }
        let k = 0.7 + 0.3 * (time * 2.0).sin();
        let bolts = crate::quality::pick([6, 10, 14]);
        for i in 0..bolts {
            let a = i as f32 / bolts as f32 * TAU;
            b.glow(&id, VAULT + vec3(-0.2, a.cos() * (r - 0.7), a.sin() * (r - 0.7)), vec3(0.1, 0.25, 0.25), Color::new(k, 0.8 * k, 0.3 * k, 1.0));
        }
        // Volante girando devagar (ninguém nunca abre)
        let m = Mat4::from_translation(VAULT - vec3(0.5, 0.0, 0.0)) * Mat4::from_rotation_x(time * 0.4);
        b.cube(&m, Vec3::ZERO, vec3(0.35, 0.7, 0.7), brass);
        for i in 0..3 {
            let mk = m * Mat4::from_rotation_x(i as f32 * PI / 3.0);
            b.cube(&mk, Vec3::ZERO, vec3(0.14, 0.18, 3.2), brass);
            for e in [-1.65, 1.65] {
                b.cube(&mk, vec3(-0.1, 0.0, e), vec3(0.3, 0.3, 0.3), brass);
            }
        }
        for y in [-1.8, 1.8] {
            b.cube(&id, VAULT + vec3(-0.1, y, -r - 0.35), vec3(0.4, 0.9, 0.5), dark);
        }
        if crate::quality::tier() != crate::quality::LOW {
            trans.glow(&id, VAULT + vec3(0.05, 0.0, 0.0), vec3(0.04, 2.0 * r + 1.4, 2.0 * r + 1.4), Color::new(1.0, 0.8, 0.3, 0.08 + 0.05 * k));
        }
    }
}

/// Caixa com título no estilo dourado.
fn panel(x: f32, y: f32, w: f32, h: f32, title: &str) {
    draw_rectangle(x, y, w, h, Color::new(0.4, 0.28, 0.02, 0.22));
    draw_rectangle_lines(x, y, w, h, 3.0, DIMG);
    draw_rectangle(x, y, 26.0, 4.0, GOLD);
    draw_text(title, x + 18.0, y + 42.0, 36.0, GOLD);
}

/// holo_fx dourado: varredura, faixa correndo e borda que pisca com dado novo.
fn gold_fx(time: f32, flash: f32) {
    let mut y = 0.0;
    while y < TH {
        draw_rectangle(0.0, y, TW, 2.0, Color::new(0.0, 0.0, 0.0, 0.07));
        y += 8.0;
    }
    let band = (time * 120.0) % (TH + 200.0) - 100.0;
    draw_rectangle(0.0, band, TW, 60.0, Color::new(1.0, 0.82, 0.3, 0.05));
    draw_rectangle_lines(4.0, 4.0, TW - 8.0, TH - 8.0, 6.0, Color::new(1.0, 0.82, 0.3, 0.4 + 0.6 * flash));
}

impl Place for Banco {
    fn on_msg(&mut self, m: &Value) {
        let i = |v: &Value| v.as_f64().unwrap_or(0.0) as i64;
        let arr = |k: &str| m[k].as_array().cloned().unwrap_or_default();
        self.tr = i(&m["tr"]);
        self.rich = arr("rich").iter().map(|r| (s(&r[0]), i(&r[1]))).collect();
        self.led = arr("led").iter().map(|r| (s(&r[0]), s(&r[1]), i(&r[2]))).collect();
        self.sv = i(&m["sv"]);
        self.nsv = i(&m["nsv"]);
        self.rate = m["rate"].as_f64().unwrap_or(2.0);
        self.got = true;
        self.recv = get_time();
    }

    fn update(&mut self, p: &mut Player, dt: f32, time: f32, online: bool, out: &mut Vec<Value>) {
        let on = PADS.iter().any(|&(x, z)| (p.pos.x - x).abs() < 0.9 && (p.pos.z - z).abs() < 0.9 && (p.pos.y - FL).abs() < 0.6);
        self.stand = if on { self.stand + dt } else { 0.0 };
        if online && self.stand >= 1.0 && (time - self.sent >= 5.0 || time < self.sent) {
            self.sent = time;
            out.push(json!({"t": "pl", "k": "banco_atm"}));
        }
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        let key = if eye.distance(GEO.c) < 110.0 { hash_str(&format!("{}|{:?}|{:?}|{}|{}|{}", self.tr, self.rich, self.led, self.sv, self.nsv, self.got)) } else { 0 };
        if self.scr.begin(time, eye, &GEO, key, crate::quality::tier() != crate::quality::LOW) {
            self.paint(time);
            self.scr.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        if eye.distance(vec3(199.0, FL, 237.5)) > 130.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let hi = crate::quality::tier() != crate::quality::LOW;
        let k = 0.75 + 0.25 * (time * 2.0).sin();
        let gold = Color::new(k, 0.8 * k, 0.28 * k, 1.0);

        // Moldura dourada do telão + halo
        let (c, w, h) = (GEO.c, GEO.w, GEO.h);
        let x = c.x - 0.02;
        for (p, sz) in [
            (vec3(x, c.y + h * 0.5, c.z), vec3(0.12, 0.2, w + 0.4)),
            (vec3(x, c.y - h * 0.5, c.z), vec3(0.12, 0.2, w + 0.4)),
            (vec3(x, c.y, c.z - w * 0.5), vec3(0.12, h + 0.4, 0.2)),
            (vec3(x, c.y, c.z + w * 0.5), vec3(0.12, h + 0.4, 0.2)),
        ] {
            b.glow(&id, p, sz, gold);
        }
        if hi {
            trans.glow(&id, vec3(183.9, c.y, c.z), vec3(0.04, h + 1.2, w + 1.2), Color::new(1.0, 0.8, 0.3, 0.12));
            // Moeda gigante no frontão
            for j in -3..=3 {
                let yy = j as f32 * 0.5;
                let half = (3.4 - yy * yy).max(0.1).sqrt();
                b.glow(&id, vec3(183.92, G as f32 + 21.5 + yy, 237.5), vec3(0.08, 0.5, 2.0 * half), gold);
            }
            b.cube(&id, vec3(183.86, G as f32 + 21.5, 237.5), vec3(0.06, 1.6, 0.35), rgb(0.5, 0.35, 0.05));
        }
        // Friso dourado na cornija
        b.glow(&id, vec3(183.95, G as f32 + 18.05, 237.5), vec3(0.1, 0.1, 29.0), gold);

        self.vault(b, trans, time);
        if eye.distance(VAULT) < 45.0 {
            labels.push(Label { pos: VAULT + vec3(-0.8, 4.6, 0.0), text: format!("COFRE DA IA: {} moedas", self.tr), size: 24.0, color: GOLD });
        }

        // Caixas eletrônicos
        let feet = eye - vec3(0.0, 1.62, 0.0);
        for (i, &(px, pz)) in PADS.iter().enumerate() {
            let dz = if i == 0 { -1.0 } else { 1.0 };
            let mz = pz + dz * 1.9;
            let front = mz - dz * 0.46;
            b.cube(&id, vec3(px, FL + 1.1, mz), vec3(1.3, 2.2, 0.9), rgb(0.17, 0.19, 0.23));
            b.glow(&id, vec3(px, FL + 2.05, front), vec3(1.1, 0.25, 0.04), gold);
            b.glow(&id, vec3(px, FL + 1.55, front), vec3(0.8, 0.5, 0.03), Color::new(0.3, 0.9 + 0.1 * (time * 5.0).sin(), 1.0, 1.0));
            b.cube(&id, vec3(px, FL + 1.05, mz - dz * 0.62), vec3(0.9, 0.08, 0.35), rgb(0.1, 0.1, 0.12));
            b.glow(&id, vec3(px, FL + 0.75, front), vec3(0.6, 0.05, 0.03), Color::new(0.3, 1.0, 0.4, 1.0));
            if hi {
                for kx in 0..3 {
                    for kz in 0..2 {
                        b.glow(&id, vec3(px - 0.2 + kx as f32 * 0.2, FL + 1.1, mz - dz * (0.55 + kz as f32 * 0.12)), vec3(0.12, 0.03, 0.08), SOFT);
                    }
                }
            }
            let on = (feet.x - px).abs() < 0.9 && (feet.z - pz).abs() < 0.9 && (feet.y - FL).abs() < 0.6;
            let pk = if on { self.stand.min(1.0) } else { 0.4 + 0.3 * (time * 3.0 + i as f32).sin() };
            b.glow(&id, vec3(px, FL + 0.03, pz), vec3(1.6, 0.06, 1.6), Color::new(0.6 + 0.4 * pk, 0.45 + 0.4 * pk, 0.1 + 0.3 * pk, 1.0));
            for (o, sz) in [(vec3(0.0, 0.0, 0.85), vec3(1.8, 0.08, 0.1)), (vec3(0.0, 0.0, -0.85), vec3(1.8, 0.08, 0.1)), (vec3(0.85, 0.0, 0.0), vec3(0.1, 0.08, 1.8)), (vec3(-0.85, 0.0, 0.0), vec3(0.1, 0.08, 1.8))] {
                b.glow(&id, vec3(px, FL + 0.04, pz) + o, sz, gold);
            }
            if hi {
                trans.glow(&id, vec3(px, FL + 1.2, pz), vec3(1.5, 2.4, 1.5), Color::new(1.0, 0.8, 0.3, 0.05 + 0.12 * pk));
            }
            if eye.distance(vec3(px, FL, pz)) < 30.0 {
                labels.push(Label { pos: vec3(px, FL + 3.0, mz), text: "CAIXA ELETRONICO - PISA AQUI".into(), size: 18.0, color: GOLD });
            }
        }

        // Lustres dourados
        if hi {
            for lx in [196.0, 204.0] {
                b.cube(&id, vec3(lx, G as f32 + 16.5, 237.5), vec3(0.08, 3.0, 0.08), rgb(0.3, 0.25, 0.1));
                b.glow(&id, vec3(lx, G as f32 + 14.8, 237.5), vec3(1.4, 0.4, 1.4), Color::new(1.0, 0.9, 0.6, 1.0));
                trans.glow(&id, vec3(lx, G as f32 + 14.8, 237.5), vec3(2.6, 1.0, 2.6), Color::new(1.0, 0.85, 0.4, 0.12));
            }
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.scr.draw(&GEO, time, eye, 130.0);
    }
}

pub fn build(w: &mut World) {
    let g = G;
    super::clear_lot(w, crate::layout::BANCO, 30, STONE);
    // Pódio: degrau (x 184) + plataforma 2 blocos; piso xadrez e tapete "dourado" (areia) até o cofre
    fill(w, (184, g, 223), (184, g, 251), COBBLE);
    fill(w, (185, g, 223), (213, g + 1, 251), STONE);
    for z in 224..=250 {
        for x in 191..=212 {
            if (x + z) % 2 == 0 {
                w.set(x, g + 1, z, COBBLE);
            }
        }
    }
    fill(w, (186, g + 1, 236), (211, g + 1, 238), SAND);
    // Salão de tijolo com faixas de pedra, janelas laterais e porta oeste (z 235..239)
    fill(w, (190, g + 2, 223), (213, g + 17, 251), BRICK);
    fill(w, (190, g + 2, 223), (213, g + 2, 251), STONE);
    fill(w, (190, g + 17, 223), (213, g + 17, 251), STONE);
    fill(w, (191, g + 2, 224), (212, g + 17, 250), AIR);
    for x in [194, 198, 202, 206, 210] {
        for z in [223, 251] {
            fill(w, (x, g + 5, z), (x + 1, g + 11, z), GLASS);
        }
    }
    fill(w, (190, g + 2, 235), (190, g + 7, 239), AIR);
    fill(w, (190, g + 8, 234), (190, g + 8, 240), SAND);
    // Fundo do cofre na parede leste
    fill(w, (213, g + 2, 232), (213, g + 11, 243), STONE);
    fill(w, (213, g + 12, 232), (213, g + 12, 243), SAND);
    // Entablamento (onde mora o telão): arquitrave de pedra, frisos de areia, tijolo
    fill(w, (184, g + 8, 223), (189, g + 17, 251), BRICK);
    fill(w, (184, g + 8, 223), (189, g + 8, 251), STONE);
    fill(w, (184, g + 9, 223), (184, g + 9, 251), SAND);
    fill(w, (184, g + 17, 223), (184, g + 17, 251), SAND);
    // Colunata de "mármore" (lã branca) com base de pedra e capitel dourado
    for z in [224, 227, 230, 233, 241, 244, 247, 250] {
        w.set(185, g + 2, z, STONE);
        fill(w, (185, g + 3, z), (185, g + 6, z), WOOL);
        w.set(185, g + 7, z, SAND);
    }
    // Teto e telhado de duas águas; frontão branco com borda dourada
    fill(w, (184, g + 18, 223), (213, g + 18, 251), STONE);
    for i in 0..8 {
        let (y, z0, z1) = (g + 19 + i, 223 + 2 * i, 251 - 2 * i);
        fill(w, (185, y, z0), (213, y, z1), BRICK);
        fill(w, (184, y, z0), (184, y, z1), WOOL);
        w.set(184, y, z0, SAND);
        w.set(184, y, z1, SAND);
    }
}
