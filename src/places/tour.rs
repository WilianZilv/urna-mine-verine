//! Tour dos Poderes (server/tour.js): enquanto o jogador tá no tour, feixe de luz alto em cada lugar que
//! falta, rótulo só no mais perto e um check verde holográfico nos já carimbados. O servidor é quem carimba
//! (pela posição); aqui é só o estado {t:"pl", k:"tour", on, left, done, sec}. Quando alguém completa
//! ({fin:nome, mural}) todo cliente vê fogos sobre o Terminal; o mural dos guias vai pro painel de partidas.

use super::Place;
use crate::batch::Batch;
use crate::extras::Label;
use crate::player::Player;
use crate::world::G;
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::FRAC_PI_4;

/// Igual server/layout.js SPOTS (centro de carimbo de cada lugar).
const SPOTS: [(&str, &str, f32, f32); 5] = [("congresso", "CONGRESSO", 78.0, 98.0), ("bolsa", "BOLSA", 244.0, 86.0), ("tv", "TV URNA NEWS", 64.0, 262.0), ("banco", "BANCO CENTRAL", 190.0, 237.0), ("terminal", "TERMINAL", 202.0, 280.0)];
const BEAM_H: f32 = 70.0;
const BEAM_FAR: f32 = 320.0;
const CHECK_FAR: f32 = 120.0;
const GOLD: Color = Color::new(1.0, 0.85, 0.25, 1.0);
const GREEN: Color = Color::new(0.35, 1.0, 0.45, 1.0);

fn spot(i: usize) -> Vec3 {
    vec3(SPOTS[i].2 + 0.5, G as f32, SPOTS[i].3 + 0.5)
}

pub struct Tour {
    on: bool,
    got: [bool; 5],
    /// Segundos restantes (conta local entre mensagens do servidor).
    left: f32,
    /// ?tour=1 na URL: pede o tour sozinho ao conectar (screenshot headless).
    auto: bool,
    /// Últimos guias (quem completou), mais novo primeiro: vai pro painel do Terminal.
    pub mural: Vec<String>,
    /// Fogos sobre o Terminal por quem acabou de completar: (nome, idade).
    fin: Vec<(String, f32)>,
}

const FIN_T: f32 = 6.0;
const FIN_MAX: usize = 3;
const SKY: Vec3 = vec3(202.5, G as f32 + 34.0, 278.5);
const PAL: [Color; 5] = [Color::new(1.0, 0.85, 0.2, 1.0), Color::new(0.3, 1.0, 0.45, 1.0), Color::new(0.3, 0.8, 1.0, 1.0), Color::new(1.0, 0.3, 0.6, 1.0), Color::new(1.0, 1.0, 1.0, 1.0)];

fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

impl Tour {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        let auto = crate::web::query("tour").as_deref() == Some("1");
        #[cfg(not(target_arch = "wasm32"))]
        let auto = false;
        Tour { on: false, got: [false; 5], left: 0.0, auto, mural: Vec::new(), fin: Vec::new() }
    }

    /// Foguetes sobem, estouram em esfera e chove confete no saguão do Terminal.
    fn draw_fin(&self, trans: &mut Batch, labels: &mut Vec<Label>, eye: Vec3) {
        let d = eye.distance(SKY);
        if d > 260.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let parts = crate::quality::pick([14, 28, 48]);
        let confetti = crate::quality::pick([8, 20, 36]);
        for (k, (name, t)) in self.fin.iter().enumerate() {
            for s in 0..4usize {
                let st = t - s as f32 * 0.7;
                if st < 0.0 {
                    continue;
                }
                let c0 = SKY + vec3(((s * 7 + k * 3) % 5) as f32 * 3.0 - 6.0, (s % 2) as f32 * 4.0, ((s * 3 + k) % 5) as f32 * 3.0 - 6.0);
                let col = PAL[(s + k) % PAL.len()];
                if st < 0.6 {
                    trans.glow(&id, c0 - vec3(0.0, (1.0 - st / 0.6) * 30.0, 0.0), Vec3::splat(0.4), WHITE);
                    continue;
                }
                let e = st - 0.6;
                if e > 2.2 {
                    continue;
                }
                let f = 1.0 - e / 2.2;
                let r = 9.0 * (1.0 - (-e * 2.5).exp());
                for j in 0..parts {
                    let y = 1.0 - 2.0 * (j as f32 + 0.5) / parts as f32;
                    let (rad, th) = ((1.0 - y * y).sqrt(), j as f32 * 2.399_963);
                    let p = c0 + vec3(th.cos() * rad, y, th.sin() * rad) * r - vec3(0.0, e * e * 1.5, 0.0);
                    trans.glow(&id, p, Vec3::splat(0.1 + 0.35 * f), alpha(col, f));
                }
            }
            for j in 0..confetti {
                let y = SKY.y - 6.0 - t * 5.0 - (j % 5) as f32 * 1.5;
                if y < G as f32 {
                    continue;
                }
                let sway = (t * 3.0 + j as f32).sin() * 0.8;
                let p = vec3(SKY.x + ((j * 37) % 30) as f32 - 15.0 + sway, y, SKY.z + ((j * 53) % 24) as f32 - 12.0);
                trans.glow(&(Mat4::from_translation(p) * Mat4::from_rotation_y(t * 4.0 + j as f32)), Vec3::ZERO, vec3(0.35, 0.05, 0.2), PAL[j % PAL.len()]);
            }
            if *t < FIN_T - 1.0 && d < 140.0 {
                labels.push(Label { pos: SKY + vec3(0.0, 12.0, 0.0), text: format!("{name} VIROU GUIA DA VILA"), size: 26.0, color: GOLD });
            }
        }
    }

    fn done(&self) -> usize {
        self.got.iter().filter(|&&g| g).count()
    }

    /// Pro painel do Terminal: (carimbos, minutos restantes) se está no tour.
    pub fn board(&self) -> Option<(usize, i32)> {
        self.on.then(|| (self.done(), (self.left / 60.0).ceil() as i32))
    }
}

impl Place for Tour {
    fn on_msg(&mut self, m: &Value) {
        if let Some(list) = m["mural"].as_array() {
            self.mural = list.iter().filter_map(|v| v.as_str()).take(5).map(|s| s.to_uppercase()).collect();
        }
        if let Some(n) = m["fin"].as_str() {
            if self.fin.len() >= FIN_MAX {
                self.fin.remove(0);
            }
            self.fin.push((n.to_uppercase().chars().take(16).collect(), 0.0));
        }
        let Some(on) = m["on"].as_bool() else { return };
        self.on = on;
        self.left = m["sec"].as_f64().unwrap_or(0.0) as f32;
        let done = m["done"].as_array();
        for (i, s) in SPOTS.iter().enumerate() {
            self.got[i] = done.is_some_and(|d| d.iter().any(|v| v == s.0));
        }
    }

    fn update(&mut self, _p: &mut Player, dt: f32, _time: f32, online: bool, out: &mut Vec<Value>) {
        self.left = (self.left - dt).max(0.0);
        for f in &mut self.fin {
            f.1 += dt;
        }
        self.fin.retain(|f| f.1 < FIN_T);
        if self.auto && online {
            self.auto = false;
            out.push(json!({"t": "chat", "m": "/tour"}));
        }
    }

    fn draw(&self, _b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        self.draw_fin(trans, labels, eye);
        if !self.on {
            return;
        }
        let id = Mat4::IDENTITY;
        let sparks = crate::quality::pick([0, 4, 8]);
        let mut near: Option<(usize, f32)> = None;
        for i in 0..SPOTS.len() {
            let p = spot(i);
            let d = vec2(eye.x - p.x, eye.z - p.z).length();
            if self.got[i] {
                if d < CHECK_FAR {
                    let m = Mat4::from_translation(p + vec3(0.0, 7.0 + (time * 2.0 + i as f32).sin() * 0.3, 0.0)) * Mat4::from_rotation_y(time * 1.5) * Mat4::from_scale(Vec3::splat(1.4));
                    trans.glow(&(m * Mat4::from_translation(vec3(-0.25, -0.25, 0.0)) * Mat4::from_rotation_z(-FRAC_PI_4)), Vec3::ZERO, vec3(0.75, 0.24, 0.24), GREEN);
                    trans.glow(&(m * Mat4::from_translation(vec3(0.5, 0.0, 0.0)) * Mat4::from_rotation_z(FRAC_PI_4)), Vec3::ZERO, vec3(1.45, 0.24, 0.24), GREEN);
                }
                continue;
            }
            if near.is_none_or(|(_, nd)| d < nd) {
                near = Some((i, d));
            }
            if d > BEAM_FAR {
                continue;
            }
            let k = 0.75 + 0.25 * (time * 3.0 + i as f32).sin();
            let mid = p + vec3(0.0, BEAM_H * 0.5, 0.0);
            trans.glow(&id, mid, vec3(2.2, BEAM_H, 2.2), Color::new(1.0, 0.55, 0.05, 0.35 * k));
            trans.glow(&id, mid, vec3(0.7, BEAM_H, 0.7), Color::new(1.0, 0.8, 0.15, 0.95 * k));
            let r = 1.5 + (time * 1.2 + i as f32 * 0.3).fract() * 4.0;
            trans.glow(&id, p + vec3(0.0, 0.06, 0.0), vec3(r * 2.0, 0.05, r * 2.0), Color::new(1.0, 0.8, 0.2, 0.3 * (1.0 - (r - 1.5) / 4.0)));
            if d < 100.0 {
                for j in 0..sparks {
                    let t = (time * 0.4 + j as f32 / sparks as f32).fract();
                    let a = j as f32 * 2.4 + time;
                    trans.glow(&id, p + vec3(a.cos() * 1.3, t * 25.0, a.sin() * 1.3), Vec3::splat(0.25), Color::new(1.0, 0.9, 0.4, 0.9 * (1.0 - t)));
                }
            }
        }
        if let Some((i, _)) = near {
            labels.push(Label { pos: spot(i) + vec3(0.0, 12.0, 0.0), text: format!("TOUR: {} ({}/5)", SPOTS[i].1, self.done() + 1), size: 22.0, color: GOLD });
        }
    }
}
