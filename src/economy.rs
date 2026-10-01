//! Economia FICTICIA da vila (moedas de brinquedo, sem dinheiro real). O servidor decide tudo;
//! aqui só mostra saldo, missão, ledger público (L) e os outdoors com anúncios aprovados pela IA.

use crate::batch::Batch;
use crate::extras::Label;
use crate::world::G;
use macroquad::prelude::*;
use serde_json::Value;

const LEDGER: usize = 200;
/// Outdoors (x, z) nos cantos da praça, todos virados pro centro.
const SPOTS: [(f32, f32); 4] = crate::layout::BILLBOARDS;
/// Outdoor da própria IA, na rua do clube.
const AI_SPOT: (f32, f32) = crate::layout::AI_BOARD;
const MID: Vec2 = crate::layout::PLAZA;

struct Entry {
    who: String,
    what: String,
    amt: i64,
    why: String,
}

struct Ad {
    text: String,
    by: String,
    left: f32,
}

struct Mission {
    text: String,
    got: i64,
    n: i64,
    left: f32,
}

pub struct Economy {
    coins: Option<i64>,
    treasury: i64,
    ledger: Vec<Entry>,
    ads: Vec<Option<Ad>>,
    mission: Option<Mission>,
    needs: String,
    pub show: bool,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

/// Corta o texto até caber na largura.
fn fit(t: &str, w: f32, size: f32) -> String {
    let mut out: String = t.to_string();
    while !out.is_empty() && measure_text(&out, None, size as u16, 1.0).width > w {
        out.pop();
    }
    out
}

impl Economy {
    pub fn new() -> Self {
        Economy { coins: None, treasury: 0, ledger: Vec::new(), ads: Vec::new(), mission: None, needs: String::new(), show: false }
    }

    /// Mensagem {t:"eco"} do servidor: campos presentes substituem o estado; "led" acrescenta (ou troca, se "full").
    pub fn on_msg(&mut self, m: &Value, chat: &mut Vec<(String, f64)>) {
        let full = m["full"].as_bool().unwrap_or(false);
        if let Some(t) = m["tr"].as_i64() {
            self.treasury = t;
        }
        if m["me"].is_object() {
            let me = &m["me"];
            self.coins = me["c"].as_i64();
            let x = &me["mis"];
            self.mission = x.is_object().then(|| Mission { text: s(&x["text"]), got: x["got"].as_i64().unwrap_or(0), n: x["n"].as_i64().unwrap_or(1), left: x["left"].as_f64().unwrap_or(0.0) as f32 });
        }
        if let Some(n) = m["needs"].as_str() {
            self.needs = n.to_string();
        }
        if let Some(a) = m["ads"].as_array() {
            self.ads = a.iter().map(|x| x.is_object().then(|| Ad { text: s(&x["text"]), by: s(&x["by"]), left: x["s"].as_f64().unwrap_or(0.0) as f32 })).collect();
        }
        if let Some(l) = m["led"].as_array() {
            if full {
                self.ledger.clear();
            }
            for e in l {
                let en = Entry { who: s(&e["who"]), what: s(&e["what"]), amt: e["amt"].as_i64().unwrap_or(0), why: s(&e["why"]) };
                if !full {
                    chat.push((format!("$ {} {} ({}) - {}", en.who, en.what, en.amt, en.why), get_time()));
                }
                self.ledger.push(en);
            }
            if self.ledger.len() > LEDGER {
                self.ledger.drain(..self.ledger.len() - LEDGER);
            }
        }
    }

    /// Outdoors 3D + texto do anúncio (vai pras labels projetadas na tela).
    pub fn draw_world(&self, b: &mut Batch, labels: &mut Vec<Label>, eye: Vec3, time: f32) {
        for (i, &(x, z)) in SPOTS.iter().enumerate() {
            let base = vec3(x, G as f32, z);
            let to = vec3(MID.x - x, 0.0, MID.y - z);
            let m = Mat4::from_translation(base) * Mat4::from_rotation_y(to.x.atan2(to.z));
            let ad = self.ads.get(i).and_then(|a| a.as_ref()).filter(|a| a.left > 0.0);
            let wood = Color::new(0.25, 0.17, 0.1, 1.0);
            b.cube(&m, vec3(-2.6, 1.75, 0.0), vec3(0.25, 3.5, 0.25), wood);
            b.cube(&m, vec3(2.6, 1.75, 0.0), vec3(0.25, 3.5, 0.25), wood);
            b.cube(&m, vec3(0.0, 4.6, 0.0), vec3(6.2, 2.6, 0.2), Color::new(0.06, 0.05, 0.1, 1.0));
            let k = 0.6 + 0.4 * (time * 3.0 + i as f32).sin();
            let edge = if ad.is_some() { Color::new(1.0, 0.85 * k, 0.2, 1.0) } else { Color::new(0.35, 0.35, 0.4, 1.0) };
            b.glow(&m, vec3(0.0, 5.95, 0.0), vec3(6.4, 0.15, 0.3), edge);
            b.glow(&m, vec3(0.0, 3.25, 0.0), vec3(6.4, 0.15, 0.3), edge);
            let mid = m.transform_point3(vec3(0.0, 4.8, 0.0));
            if mid.distance(eye) > 55.0 {
                continue;
            }
            match ad {
                Some(a) => {
                    labels.push(Label { pos: mid, text: a.text.clone(), size: 26.0, color: Color::new(1.0, 0.9, 0.3, 1.0) });
                    labels.push(Label { pos: m.transform_point3(vec3(0.0, 3.8, 0.0)), text: format!("anuncio de {}", a.by), size: 16.0, color: WHITE });
                }
                None => labels.push(Label { pos: mid, text: "ANUNCIE AQUI: /anuncio texto moedas".into(), size: 18.0, color: Color::new(0.8, 0.8, 0.85, 1.0) }),
            }
        }
        self.draw_ai_board(b, labels, eye, time);
    }

    /// Outdoor da IA: o texto de "necessidades" que ela mesma escreve (filtrado no servidor).
    fn draw_ai_board(&self, b: &mut Batch, labels: &mut Vec<Label>, eye: Vec3, time: f32) {
        if self.needs.is_empty() {
            return;
        }
        let (x, z) = AI_SPOT;
        let to = vec3(MID.x - x, 0.0, MID.y - z);
        let m = Mat4::from_translation(vec3(x, G as f32, z)) * Mat4::from_rotation_y(to.x.atan2(to.z));
        let dark = Color::new(0.08, 0.06, 0.1, 1.0);
        b.cube(&m, vec3(-3.6, 2.5, 0.0), vec3(0.3, 5.0, 0.3), dark);
        b.cube(&m, vec3(3.6, 2.5, 0.0), vec3(0.3, 5.0, 0.3), dark);
        b.cube(&m, vec3(0.0, 6.5, 0.0), vec3(8.4, 3.6, 0.25), Color::new(0.04, 0.02, 0.08, 1.0));
        let k = 0.5 + 0.5 * (time * 2.0).sin();
        let edge = Color::new(0.4 + 0.6 * k, 0.3, 1.0 - 0.5 * k, 1.0);
        for y in [4.6, 8.4] {
            b.glow(&m, vec3(0.0, y, 0.0), vec3(8.6, 0.18, 0.35), edge);
        }
        let mid = m.transform_point3(vec3(0.0, 6.5, 0.0));
        if mid.distance(eye) > 70.0 {
            return;
        }
        labels.push(Label { pos: m.transform_point3(vec3(0.0, 9.1, 0.0)), text: "OUTDOOR DA IA".into(), size: 20.0, color: Color::new(1.0, 0.4, 0.9, 1.0) });
        let mut lines: Vec<String> = vec![String::new()];
        for w in self.needs.split_whitespace() {
            let cur = lines.last_mut().unwrap();
            if !cur.is_empty() && cur.len() + w.len() > 34 {
                lines.push(String::new());
            }
            let cur = lines.last_mut().unwrap();
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(w);
        }
        let n = lines.len().min(3);
        for (i, l) in lines.into_iter().take(3).enumerate() {
            let y = 6.5 + (n as f32 - 1.0) * 0.5 - i as f32;
            labels.push(Label { pos: m.transform_point3(vec3(0.0, y, 0.0)), text: l, size: 22.0, color: Color::new(0.7, 1.0, 1.0, 1.0) });
        }
    }

    /// Saldo/missão no HUD, L abre o ledger público. `keys` = teclado livre (fora do chat/menus).
    pub fn hud(&mut self, dt: f32, sw: f32, sh: f32, keys: bool) {
        if keys && is_key_pressed(KeyCode::L) {
            self.show = !self.show;
        }
        for a in self.ads.iter_mut().flatten() {
            a.left -= dt;
        }
        if let Some(m) = self.mission.as_mut() {
            m.left -= dt;
        }
        let Some(c) = self.coins else { return };
        let mis = match &self.mission {
            Some(m) if m.left > 0.0 => format!("  |  MISSAO {}/{} {} ({:.0}s)", m.got, m.n, m.text, m.left),
            _ => String::new(),
        };
        let line = fit(&format!("$ {c} MOEDAS  |  COFRE IA {}{mis}", self.treasury), sw * 0.6, 20.0);
        let d = measure_text(&line, None, 20, 1.0);
        draw_rectangle(8.0, 66.0, d.width + 10.0, 22.0, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_text(&line, 13.0, 83.0, 20.0, Color::new(1.0, 0.85, 0.3, 1.0));
        if !self.show {
            return;
        }
        let w = (sw - 20.0).min(680.0);
        let rows = (((sh - 200.0) / 38.0) as usize).clamp(3, 12);
        let (x, y) = (sw * 0.5 - w * 0.5, 96.0);
        draw_rectangle(x, y, w, 34.0 + rows as f32 * 38.0, Color::new(0.02, 0.02, 0.06, 0.85));
        draw_rectangle_lines(x, y, w, 34.0 + rows as f32 * 38.0, 2.0, Color::new(1.0, 0.85, 0.3, 0.8));
        draw_text("LEDGER PUBLICO (moedas ficticias) - L fecha", x + 10.0, y + 22.0, 20.0, Color::new(1.0, 0.85, 0.3, 1.0));
        for (i, e) in self.ledger.iter().rev().take(rows).enumerate() {
            let ry = y + 52.0 + i as f32 * 38.0;
            draw_text(&fit(&format!("{}: {} ({})", e.who, e.what, e.amt), w - 20.0, 18.0), x + 10.0, ry, 18.0, WHITE);
            draw_text(&fit(&e.why, w - 30.0, 15.0), x + 20.0, ry + 15.0, 15.0, Color::new(0.7, 0.7, 0.8, 1.0));
        }
    }
}
