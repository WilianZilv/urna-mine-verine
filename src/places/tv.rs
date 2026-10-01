//! TV URNA NEWS: âncora IA resume o mundo num telão de fachada (server/tv.js). Pisar no AO VIVO do
//! estúdio 1,5 s põe o jogador no ar (lower third no telão + pergunta).

use super::{Geo, Place, Screen, TW, fill, hash_str, s, text_mid, wrap};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{CYAN, GOLD, SOFT, fit, frame, holo_fx, text_right};
use crate::models::rgb;
use crate::player::Player;
use crate::world::{AIR, BLACK, G, GLASS, GRASS, NEON, PLANKS, World};
use macroquad::prelude::*;
use serde_json::Value;
use std::f32::consts::{FRAC_PI_4, PI};

const RED: Color = Color::new(1.0, 0.18, 0.2, 1.0);
/// Centro do pad AO VIVO (x, z), 3x3 blocos de neon no piso.
const PAD: Vec2 = vec2(64.5, 281.5);
/// Centro do estúdio (culling dos cubos internos).
const STUDIO: Vec3 = vec3(64.5, G as f32 + 3.0, 280.0);

fn geo() -> Geo {
    Geo { c: vec3(64.5, G as f32 + 10.5, 265.8), n: vec3(0.0, 0.0, -1.0), w: 22.0, h: 11.0 }
}

pub struct Tv {
    scr: Screen,
    got: bool,
    h: Vec<String>,
    tk: String,
    a: String,
    n: i64,
    ago: f64,
    recv: f64,
    /// Quando a fala da âncora mudou (boca mexe enquanto é nova) e quando chegou boletim novo (pisca).
    a_t: f64,
    fl_t: f64,
    /// No ar: (nome, pergunta, fim em get_time, início).
    air: Option<(String, String, f64, f64)>,
    /// Patrocínios: (texto, quem, fim em get_time).
    sp: Vec<(String, String, f64)>,
    /// PLANTAO URGENTE: (texto, fim em get_time, início).
    urg: Option<(String, f64, f64)>,
    /// MOMENTO DO DIA: itens (categoria, valor, nome), fim e início em get_time.
    mom: Option<(Vec<(String, String, String)>, f64, f64)>,
    /// AUDIENCIA (do servidor), recorde e dia dele; `watching` = este cliente olhando o telão de perto.
    aud: i64,
    rec: i64,
    recd: String,
    watching: bool,
    pad: f32,
    cool: f32,
}

impl Tv {
    pub fn new() -> Self {
        Tv { scr: Screen::new(), got: false, h: vec![], tk: String::new(), a: String::new(), n: 0, ago: -1.0, recv: 0.0, a_t: -100.0, fl_t: -100.0, air: None, sp: vec![], urg: None, mom: None, aud: 0, rec: 0, recd: String::new(), watching: false, pad: 0.0, cool: 0.0 }
    }

    /// Canto do telão: AO VIVO pulsando + AUDIENCIA e recorde. (x, y) = canto superior esquerdo, 470x100.
    fn badge(&self, time: f32, x: f32, y: f32) {
        let p = 0.5 + 0.5 * (time * 4.0).sin();
        draw_rectangle(x, y, 470.0, 100.0, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_rectangle_lines(x, y, 470.0, 100.0, 4.0, Color::new(1.0, 0.2, 0.2, 0.5 + 0.5 * p));
        draw_circle(x + 30.0, y + 30.0, 10.0 + 5.0 * p, Color::new(1.0, 0.1 + 0.2 * p, 0.1, 1.0));
        draw_text("AO VIVO", x + 54.0, y + 44.0, 44.0, Color::new(1.0, 0.25 + 0.5 * p, 0.25 + 0.5 * p, 1.0));
        let day = self.recd.get(5..).map(|d| d.split('-').rev().collect::<Vec<_>>().join("/")).unwrap_or_default();
        text_right(&format!("REC {}{}", self.rec, if day.is_empty() { String::new() } else { format!(" ({day})") }), x + 455.0, y + 40.0, 30.0, GOLD);
        let c = if self.watching { GOLD } else { WHITE };
        draw_text(&format!("AUDIENCIA: {}", self.aud), x + 18.0, y + 88.0, 44.0, c);
        if self.watching {
            text_right("(TU)", x + 455.0, y + 88.0, 30.0, GOLD);
        }
    }

    fn mom_now(&self) -> Option<&(Vec<(String, String, String)>, f64, f64)> {
        self.mom.as_ref().filter(|m| m.1 > get_time() && !m.0.is_empty())
    }

    /// Item do MOMENTO DO DIA na tela agora: (índice, fração do tempo dele já passada).
    fn mom_item(n: usize, start: f64, end: f64, now: f64) -> (usize, f32) {
        let per = ((end - start) / n as f64).max(1.0);
        let k = ((now - start).max(0.0) / per) as usize;
        (k.min(n - 1), (((now - start).max(0.0) % per) / per) as f32)
    }

    /// Tela dourada do MOMENTO DO DIA: categoria, valor gigante e nome, um recorde por vez.
    fn paint_mom(&self, time: f32, items: &[(String, String, String)], end: f64, start: f64, now: f64) {
        let (i, f) = Self::mom_item(items.len(), start, end, now);
        let (cat, v, name) = &items[i];
        let dark = Color::new(0.07, 0.045, 0.0, 1.0);
        draw_rectangle(0.0, 0.0, TW, 1024.0, dark);
        for k in 0..14 {
            let a = time * 0.25 + k as f32 * PI / 7.0;
            let c = vec2(TW * 0.5, 600.0);
            draw_triangle(c, c + vec2(a.cos(), a.sin()) * 1500.0, c + vec2((a + 0.12).cos(), (a + 0.12).sin()) * 1500.0, Color::new(1.0, 0.8, 0.25, 0.06));
        }
        let pulse = 0.75 + 0.25 * (time * 3.0).sin();
        let gold = Color::new(GOLD.r * pulse, GOLD.g * pulse, GOLD.b * pulse + 0.05, 1.0);
        for (x, y, w, h) in [(0.0, 0.0, TW, 26.0), (0.0, 998.0, TW, 26.0), (0.0, 0.0, 26.0, 1024.0), (TW - 26.0, 0.0, 26.0, 1024.0)] {
            draw_rectangle(x, y, w, h, gold);
        }
        draw_rectangle_lines(44.0, 44.0, TW - 88.0, 936.0, 6.0, Color::new(1.0, 0.85, 0.3, 0.6));
        for (x, y) in [(44.0, 44.0), (TW - 44.0, 44.0), (44.0, 980.0), (TW - 44.0, 980.0)] {
            draw_poly(x, y, 4, 34.0, time * 40.0, GOLD);
        }
        draw_rectangle(110.0, 70.0, 1360.0, 170.0, Color::new(0.35, 0.22, 0.0, 0.9));
        draw_rectangle(110.0, 232.0, 1360.0, 8.0, GOLD);
        text_mid("MOMENTO DO DIA", 790.0, 200.0, 150.0, gold);
        self.badge(time, TW - 560.0, 100.0);
        // Entrada do item: desliza da direita e acende nos primeiros instantes
        let e = (f * 8.0).clamp(0.0, 1.0);
        let dx = (1.0 - e) * (1.0 - e) * 900.0;
        let a = e;
        text_mid(&fit(cat, TW - 240.0, 100.0), TW * 0.5 + dx, 380.0, 100.0, Color::new(1.0, 1.0, 1.0, a));
        text_mid(&fit(v, TW - 240.0, 260.0), TW * 0.5 + dx, 650.0, 260.0, Color::new(GOLD.r, GOLD.g, GOLD.b, a));
        draw_rectangle(TW * 0.5 - 600.0 + dx, 720.0, 1200.0, 140.0, Color::new(0.5, 0.03, 0.06, 0.9 * a));
        text_mid(&fit(name, 1150.0, 110.0), TW * 0.5 + dx, 830.0, 110.0, Color::new(1.0, 1.0, 1.0, a));
        let n = items.len() as f32;
        for k in 0..items.len() {
            let x = TW * 0.5 + (k as f32 - (n - 1.0) * 0.5) * 70.0;
            if k == i {
                draw_circle(x, 925.0, 20.0, GOLD);
            } else {
                draw_circle_lines(x, 925.0, 18.0, 4.0, GOLD);
            }
        }
        text_right(&format!("{:.0}s", (end - now).max(0.0)), TW - 80.0, 950.0, 56.0, SOFT);
        draw_text("TV URNA - recordes da vila de hoje", 80.0, 950.0, 40.0, SOFT);
        holo_fx(time, (1.0 - f * 10.0).clamp(0.0, 1.0) * 0.4);
    }

    fn air_now(&self) -> Option<&(String, String, f64, f64)> {
        self.air.as_ref().filter(|a| a.2 > get_time())
    }

    fn urg_now(&self) -> Option<&(String, f64, f64)> {
        self.urg.as_ref().filter(|u| u.1 > get_time())
    }

    /// Tela cheia vermelha do PLANTAO URGENTE.
    fn paint_urg(&self, time: f32, text: &str, end: f64, now: f64) {
        let blink = (time * 2.5).fract() < 0.5;
        draw_rectangle(0.0, 0.0, TW, 1024.0, Color::new(0.35, 0.0, 0.02, 1.0));
        for i in 0..8 {
            let x = ((time * 300.0 + i as f32 * 320.0) % (TW + 640.0)) - 320.0;
            draw_triangle(vec2(x, 250.0), vec2(x + 160.0, 250.0), vec2(x - 80.0, 1024.0), Color::new(0.6, 0.0, 0.05, 0.35));
        }
        draw_rectangle(0.0, 0.0, TW, 230.0, if blink { RED } else { Color::new(0.75, 0.05, 0.08, 1.0) });
        draw_rectangle(0.0, 230.0, TW, 12.0, WHITE);
        text_mid("PLANTAO URGENTE", TW * 0.5, 180.0, 170.0, WHITE);
        for (j, l) in wrap(text, TW - 160.0, 110.0, 4).iter().enumerate() {
            text_mid(l, TW * 0.5, 410.0 + j as f32 * 130.0, 110.0, WHITE);
        }
        draw_rectangle(0.0, 900.0, TW, 124.0, Color::new(0.02, 0.02, 0.05, 0.95));
        draw_rectangle(0.0, 900.0, 420.0, 124.0, RED);
        text_mid("TV URNA", 210.0, 984.0, 80.0, WHITE);
        draw_text(&fit("INTERROMPEMOS A PROGRAMACAO PRA ISSO AI", TW - 1010.0, 48.0), 460.0, 978.0, 48.0, SOFT);
        text_right(&format!("{:.0}s", (end - now).max(0.0)), TW - 40.0, 90.0, 80.0, WHITE);
        self.badge(time, TW - 510.0, 912.0);
        holo_fx(time, if blink { 0.3 } else { 0.0 });
    }

    fn paint(&self, time: f32) {
        let now = get_time();
        if let Some((text, end, _)) = self.urg_now() {
            return self.paint_urg(time, text, *end, now);
        }
        if let Some((items, end, start)) = self.mom_now() {
            return self.paint_mom(time, items, *end, *start, now);
        }
        let flash = (1.0 - (now - self.fl_t) as f32 / 2.0).clamp(0.0, 1.0);
        // Cabeçalho
        draw_rectangle(0.0, 0.0, TW, 120.0, Color::new(0.5, 0.03, 0.06, 0.95));
        draw_rectangle(0.0, 116.0, TW, 6.0, RED);
        draw_text("URNA NEWS", 40.0, 92.0, 90.0, WHITE);
        let ago = if self.ago < 0.0 { "sintonizando".to_string() } else { format!("ha {} min", ((self.ago + now - self.recv) / 60.0).floor() as i64) };
        text_right(&format!("BOLETIM #{:04}  -  {ago}", self.n), TW - 520.0, 78.0, 48.0, GOLD);
        self.badge(time, TW - 490.0, 10.0);

        self.anchor(time, now);

        // Manchetes
        let (cx, cw) = (680.0, TW - 720.0);
        for i in 0..3 {
            let y = 140.0 + i as f32 * 164.0;
            draw_rectangle(cx, y, cw, 150.0, Color::new(0.08, 0.3, 0.42, 0.35));
            let k = if i == 0 { 0.6 + 0.4 * (time * 3.0).sin() } else { 1.0 };
            draw_rectangle(cx, y, 12.0, 150.0, Color::new(RED.r, RED.g, RED.b, k));
            draw_text(&format!("{}", i + 1), cx + 36.0, y + 106.0, 100.0, GOLD);
            let t = self.h.get(i).map(String::as_str).unwrap_or(if self.got { "..." } else { "sintonizando o satelite da vila..." });
            for (j, l) in wrap(t, cw - 170.0, 52.0, 2).iter().enumerate() {
                draw_text(l, cx + 130.0, y + 64.0 + j as f32 * 56.0, 52.0, WHITE);
            }
        }
        // Fala da âncora
        frame(cx, 636.0, cw, 150.0, "");
        draw_text("ANCORA URNA-BOT:", cx + 20.0, 676.0, 32.0, CYAN);
        let quote = if self.a.is_empty() { "...".to_string() } else { format!("\"{}\"", self.a) };
        for (j, l) in wrap(&quote, cw - 40.0, 44.0, 2).iter().enumerate() {
            draw_text(l, cx + 20.0, 724.0 + j as f32 * 46.0, 44.0, SOFT);
        }
        let next = if self.ago < 0.0 { "proximo boletim: ja ja".to_string() } else { format!("proximo boletim em ~{} min", ((360.0 - (self.ago + now - self.recv)) / 60.0).ceil().max(0.0) as i64) };
        draw_text(&format!("{next}   |   {} patrocinio(s) no letreiro", self.sp.iter().filter(|x| x.2 > now).count()), cx, 840.0, 34.0, SOFT);

        // Lower third de quem está no ar
        if let Some((name, q, end, start)) = self.air_now() {
            let e = ((now - start) as f32 * 3.0).clamp(0.0, 1.0);
            let x = 40.0 - (1.0 - e) * 1200.0;
            draw_rectangle(x, 640.0, TW - 80.0, 96.0, RED);
            draw_text(&fit(&format!("ENTREVISTADO AO VIVO: {}", name.to_uppercase()), TW - 380.0, 72.0), x + 30.0, 712.0, 72.0, WHITE);
            text_right(&format!("{:.0}s", (end - now).max(0.0)), x + TW - 120.0, 714.0, 80.0, GOLD);
            draw_rectangle(x, 736.0, TW - 80.0, 130.0, Color::new(0.95, 0.95, 0.97, 0.97));
            for (j, l) in wrap(q, TW - 160.0, 48.0, 2).iter().enumerate() {
                draw_text(l, x + 30.0, 790.0 + j as f32 * 52.0, 48.0, Color::new(0.08, 0.08, 0.12, 1.0));
            }
        }

        // Letreiro: manchetes + ticker + patrocinados
        let mut crawl: Vec<String> = self.h.clone();
        if !self.tk.is_empty() {
            crawl.push(self.tk.clone());
        }
        crawl.extend(self.sp.iter().filter(|x| x.2 > now).map(|(t, by, _)| format!("PATROCINADO por {by}: {t}")));
        if crawl.is_empty() {
            crawl.push("TV URNA NEWS - a unica emissora aprovada pela urna".into());
        }
        let line = crawl.join("   ///   ");
        draw_rectangle(0.0, 880.0, TW, 84.0, Color::new(0.02, 0.02, 0.05, 0.95));
        let lw = measure_text(&line, None, 54, 1.0).width;
        let x = TW - (time * 260.0) % (lw + TW - 300.0);
        draw_text(&line, x, 940.0, 54.0, WHITE);
        draw_rectangle(0.0, 880.0, 300.0, 84.0, RED);
        text_mid("PLANTAO", 150.0, 940.0, 56.0, WHITE);
        draw_rectangle(0.0, 964.0, TW, 60.0, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text("satira - noticias do jogo   |   /tv   /noticia (5 moedas)   /manchete texto n   |   pise no AO VIVO do estudio", 40.0, 1004.0, 30.0, SOFT);
        holo_fx(time, flash);
    }

    /// Âncora robô vetorial (original): boca mexe enquanto a fala é nova ou durante a entrevista.
    fn anchor(&self, time: f32, now: f64) {
        let talk = now - self.a_t < 10.0 || self.air_now().is_some_and(|a| now - a.3 < 8.0);
        draw_rectangle(40.0, 140.0, 600.0, 646.0, Color::new(0.04, 0.12, 0.22, 0.6));
        for i in 0..6 {
            let y = 180.0 + i as f32 * 70.0;
            draw_rectangle(60.0, y, 560.0, 2.0, Color::new(0.45, 1.0, 1.0, 0.08));
        }
        let (ax, bob) = (340.0, (time * 1.7).sin() * 4.0);
        let metal = Color::new(0.7, 0.75, 0.82, 1.0);
        // Antena
        draw_line(ax, 240.0 + bob, ax, 186.0 + bob, 6.0, metal);
        draw_circle(ax, 180.0 + bob, 13.0, if (time * 2.0).fract() < 0.5 { RED } else { Color::new(0.4, 0.05, 0.05, 1.0) });
        // Cabeça + rosto de tela
        draw_rectangle(212.0, 240.0 + bob, 256.0, 196.0, metal);
        draw_rectangle(196.0, 300.0 + bob, 16.0, 70.0, GOLD);
        draw_rectangle(468.0, 300.0 + bob, 16.0, 70.0, GOLD);
        draw_rectangle(232.0, 258.0 + bob, 216.0, 160.0, Color::new(0.03, 0.06, 0.1, 1.0));
        if (time * 0.6).fract() < 0.04 {
            draw_rectangle(270.0, 318.0 + bob, 50.0, 6.0, CYAN);
            draw_rectangle(360.0, 318.0 + bob, 50.0, 6.0, CYAN);
        } else {
            draw_circle(295.0, 320.0 + bob, 22.0, CYAN);
            draw_circle(385.0, 320.0 + bob, 22.0, CYAN);
            draw_circle(300.0, 314.0 + bob, 7.0, WHITE);
            draw_circle(390.0, 314.0 + bob, 7.0, WHITE);
        }
        let mh = if talk { 8.0 + ((time * 13.0).sin().abs() * 0.7 + (time * 21.0).sin().abs() * 0.3) * 34.0 } else { 6.0 };
        draw_rectangle(ax - 45.0, 382.0 + bob - mh * 0.5, 90.0, mh, CYAN);
        // Pescoço, terno, gravata
        draw_rectangle(ax - 25.0, 436.0, 50.0, 22.0, metal);
        draw_rectangle(190.0, 456.0, 300.0, 200.0, Color::new(0.08, 0.1, 0.2, 1.0));
        draw_triangle(vec2(ax - 55.0, 456.0), vec2(ax + 55.0, 456.0), vec2(ax, 560.0), WHITE);
        draw_triangle(vec2(ax - 14.0, 462.0), vec2(ax + 14.0, 462.0), vec2(ax, 590.0), RED);
        draw_triangle(vec2(ax - 55.0, 456.0), vec2(ax - 10.0, 560.0), vec2(ax - 95.0, 500.0), Color::new(0.14, 0.17, 0.3, 1.0));
        draw_triangle(vec2(ax + 55.0, 456.0), vec2(ax + 10.0, 560.0), vec2(ax + 95.0, 500.0), Color::new(0.14, 0.17, 0.3, 1.0));
        // Bancada com papéis
        draw_rectangle(60.0, 620.0, 560.0, 150.0, Color::new(0.1, 0.1, 0.14, 1.0));
        draw_rectangle(60.0, 620.0, 560.0, 10.0, RED);
        draw_rectangle(230.0, 604.0, 120.0, 18.0, WHITE);
        draw_rectangle(200.0, 600.0, 60.0, 24.0, metal);
        draw_rectangle(420.0, 600.0, 60.0, 24.0, metal);
        text_mid("URNA NEWS", ax, 714.0, 70.0, WHITE);
        text_mid("a noticia que a urna aprova", ax, 752.0, 28.0, SOFT);
    }

    /// Sirene girando no topo da fachada, halo vermelho pulsando em volta do telão e chamariz de longe.
    fn siren(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let id = Mat4::IDENTITY;
        let g = G as f32;
        let p = (time * 6.0).sin() * 0.5 + 0.5;
        let top = vec3(64.5, g + 18.0, 266.5);
        b.cube(&id, top + vec3(0.0, 0.25, 0.0), vec3(1.6, 0.5, 1.6), rgb(0.12, 0.12, 0.15));
        b.glow(&id, top + vec3(0.0, 1.0, 0.0), vec3(1.0, 1.0, 1.0), Color::new(1.0, 0.1 + 0.3 * p, 0.1, 1.0));
        let beams = crate::quality::pick([1, 2, 2]);
        for i in 0..beams {
            let m = Mat4::from_translation(top + vec3(0.0, 1.0, 0.0)) * Mat4::from_rotation_y(time * 5.0 + i as f32 * PI);
            trans.glow(&m, vec3(0.0, 0.0, 3.0), vec3(0.45, 0.45, 6.0), Color::new(1.0, 0.12, 0.08, 0.55));
        }
        let gs = geo();
        let (x0, x1, y0, y1) = (gs.c.x - gs.w * 0.5, gs.c.x + gs.w * 0.5, gs.c.y - gs.h * 0.5, gs.c.y + gs.h * 0.5);
        let (z, d) = (265.5, 0.9);
        let c = Color::new(1.0, 0.08, 0.1, 0.3 + 0.5 * p);
        trans.glow(&id, vec3(gs.c.x, y1 + d, z), vec3(gs.w + d * 4.0, d, 0.1), c);
        trans.glow(&id, vec3(gs.c.x, y0 - d, z), vec3(gs.w + d * 4.0, d, 0.1), c);
        trans.glow(&id, vec3(x0 - d, gs.c.y, z), vec3(d, gs.h, 0.1), c);
        trans.glow(&id, vec3(x1 + d, gs.c.y, z), vec3(d, gs.h, 0.1), c);
        if eye.distance(top) < 150.0 {
            labels.push(Label { pos: top + vec3(0.0, 3.0, 0.0), text: "PLANTAO NA TV URNA".into(), size: 30.0, color: Color::new(1.0, 0.3, 0.3, 1.0) });
        }
    }
}

impl Place for Tv {
    fn on_msg(&mut self, m: &Value) {
        let now = get_time();
        let a = s(&m["a"]);
        if a != self.a {
            self.a_t = now;
        }
        let n = m["n"].as_i64().unwrap_or(0);
        if self.got && n != self.n {
            self.fl_t = now;
        }
        self.h = m["h"].as_array().map(|v| v.iter().map(s).collect()).unwrap_or_default();
        self.tk = s(&m["tk"]);
        self.a = a;
        self.n = n;
        self.ago = m["ago"].as_f64().unwrap_or(-1.0);
        self.recv = now;
        self.got = true;
        let air = &m["air"];
        self.air = air.is_object().then(|| {
            let start = match &self.air {
                Some((name, _, _, st)) if *name == s(&air["name"]) => *st,
                _ => now,
            };
            (s(&air["name"]), s(&air["q"]), now + air["left"].as_f64().unwrap_or(0.0), start)
        });
        let u = &m["urg"];
        self.urg = u.is_object().then(|| {
            let start = match &self.urg {
                Some((t, _, st)) if *t == s(&u["text"]) => *st,
                _ => now,
            };
            (s(&u["text"]), now + u["left"].as_f64().unwrap_or(0.0), start)
        });
        self.aud = m["aud"].as_i64().unwrap_or(0);
        self.rec = m["rec"].as_i64().unwrap_or(0);
        self.recd = s(&m["recd"]);
        let mo = &m["mom"];
        self.mom = mo.is_object().then(|| {
            let items: Vec<(String, String, String)> = mo["items"].as_array().map(|v| v.iter().map(|x| (s(&x["cat"]), s(&x["v"]), s(&x["n"]))).collect()).unwrap_or_default();
            let left = mo["left"].as_f64().unwrap_or(0.0);
            let start = match &self.mom {
                Some((old, _, st)) if *old == items => *st,
                _ => now + left - 30.0,
            };
            (items, now + left, start)
        });
        self.sp = m["sp"].as_array().map(|v| v.iter().map(|x| (s(&x["text"]), s(&x["by"]), now + x["left"].as_f64().unwrap_or(0.0))).collect()).unwrap_or_default();
    }

    fn update(&mut self, p: &mut Player, dt: f32, time: f32, online: bool, out: &mut Vec<Value>) {
        let (gs, eye) = (geo(), p.eye());
        let to = gs.c - eye;
        self.watching = to.length() < 60.0 && gs.facing(eye) && p.forward().dot(to.normalize_or_zero()) > 0.7;
        let on = (p.pos.x - PAD.x).abs() < 1.5 && (p.pos.z - PAD.y).abs() < 1.5 && (p.pos.y - G as f32).abs() < 1.2;
        if !on || !online || time < self.cool {
            self.pad = 0.0;
            return;
        }
        self.pad += dt;
        if self.pad >= 1.5 {
            self.pad = 0.0;
            self.cool = time + 10.0;
            out.push(serde_json::json!({ "t": "pl", "k": "tv_aovivo" }));
        }
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        let now = get_time();
        let air = self.air.as_ref().filter(|a| a.2 > now).map_or("", |a| a.0.as_str());
        let urg = self.urg.as_ref().filter(|u| u.1 > now).map_or("", |u| u.0.as_str());
        let mom = self.mom.as_ref().filter(|m| m.1 > now).map_or(0, |m| m.0.len());
        let key = || hash_str(&format!("{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}", self.h.join("|"), self.tk, self.a, self.n, air, self.sp.len(), urg, mom, self.aud, self.rec, self.watching));
        if self.scr.begin_with(time, eye, &geo(), true, key) {
            self.paint(time);
            self.scr.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let id = Mat4::IDENTITY;
        let g = G as f32;
        if self.urg_now().is_some() && eye.distance(STUDIO) < 160.0 {
            self.siren(b, trans, labels, time, eye);
        }
        if eye.distance(STUDIO) > 140.0 {
            return;
        }
        let hi = crate::quality::tier() > crate::quality::LOW;
        let air = self.air_now().is_some();
        let metal = rgb(0.12, 0.12, 0.15);
        let blink = (time * 2.0).fract() < 0.5;

        // Moldura do telão: aro de metal, borda de luz pulsando, lâmpada AO VIVO e luzes correndo
        let gs = geo();
        let (x0, x1, y0, y1) = (gs.c.x - gs.w * 0.5, gs.c.x + gs.w * 0.5, gs.c.y - gs.h * 0.5, gs.c.y + gs.h * 0.5);
        let z = 265.85;
        b.cube(&id, vec3(gs.c.x, y1 + 0.3, z), vec3(gs.w + 1.2, 0.6, 0.3), metal);
        b.cube(&id, vec3(gs.c.x, y0 - 0.3, z), vec3(gs.w + 1.2, 0.6, 0.3), metal);
        b.cube(&id, vec3(x0 - 0.3, gs.c.y, z), vec3(0.6, gs.h, 0.3), metal);
        b.cube(&id, vec3(x1 + 0.3, gs.c.y, z), vec3(0.6, gs.h, 0.3), metal);
        let k = 0.65 + 0.35 * (time * 2.5).sin();
        let edge = if air { Color::new(1.0, 0.2, 0.2, 0.6 + 0.4 * k) } else { Color::new(1.0, 0.35 * k + 0.3, 0.35 * k + 0.3, 0.75) };
        let zf = 265.66;
        for (c, sz) in [
            (vec3(gs.c.x, y1 + 0.06, zf), vec3(gs.w + 0.24, 0.12, 0.06)),
            (vec3(gs.c.x, y0 - 0.06, zf), vec3(gs.w + 0.24, 0.12, 0.06)),
            (vec3(x0 - 0.06, gs.c.y, zf), vec3(0.12, gs.h, 0.06)),
            (vec3(x1 + 0.06, gs.c.y, zf), vec3(0.12, gs.h, 0.06)),
        ] {
            trans.glow(&id, c, sz, edge);
        }
        b.glow(&id, vec3(x0 + 1.4, y1 + 0.95, 265.7), vec3(2.2, 0.6, 0.2), if blink || air { RED } else { Color::new(0.35, 0.05, 0.05, 1.0) });
        if hi {
            for i in 0..12 {
                let on = ((time * 6.0) as i32 + i) % 3 == 0;
                let x = x0 + 3.5 + i as f32 * (gs.w - 4.5) / 11.0;
                b.glow(&id, vec3(x, y1 + 0.3, z - 0.17), vec3(0.3, 0.3, 0.04), if on { WHITE } else { Color::new(0.6, 0.1, 0.1, 1.0) });
            }
        }

        // Antena na fachada (luz piscando) e parabólica girando no teto
        let (mx, mz) = (79.5, 267.0);
        b.cube(&id, vec3(mx, g + 22.0, mz), vec3(0.3, 8.0, 0.3), metal);
        for h in [19.5, 22.0, 24.5] {
            b.cube(&id, vec3(mx, g + h, mz), vec3(1.6 - h * 0.04, 0.12, 0.12), metal);
        }
        b.glow(&id, vec3(mx, g + 26.2, mz), vec3(0.45, 0.45, 0.45), if blink { RED } else { Color::new(0.3, 0.05, 0.05, 1.0) });
        let dish = rgb(0.85, 0.86, 0.9);
        b.cube(&id, vec3(54.5, g + 13.5, 283.5), vec3(1.2, 1.0, 1.2), metal);
        b.cube(&id, vec3(54.5, g + 14.8, 283.5), vec3(0.3, 1.6, 0.3), metal);
        let m = Mat4::from_translation(vec3(54.5, g + 15.8, 283.5)) * Mat4::from_rotation_y(time * 0.25) * Mat4::from_rotation_x(-0.7);
        b.cube(&m, Vec3::ZERO, vec3(3.2, 0.15, 3.2), dish);
        if hi {
            b.cube(&(m * Mat4::from_rotation_y(FRAC_PI_4)), Vec3::ZERO, vec3(3.2, 0.15, 3.2), dish);
        }
        b.cube(&m, vec3(0.0, 0.9, 0.0), vec3(0.12, 1.8, 0.12), metal);
        b.glow(&m, vec3(0.0, 1.85, 0.0), vec3(0.3, 0.3, 0.3), Color::new(1.0, 0.8, 0.3, 1.0));

        // Estúdio (só de perto)
        if eye.distance(STUDIO) > 60.0 {
            return;
        }
        let pad = vec3(PAD.x, g, PAD.y);
        let rigs: &[(f32, f32)] = if hi { &[(59.5, 275.5), (69.5, 275.5)] } else { &[(64.5, 275.0)] };
        for &(cx, cz) in rigs {
            b.cube(&id, vec3(cx, g + 0.7, cz), vec3(0.15, 1.4, 0.15), metal);
            b.cube(&id, vec3(cx, g + 0.04, cz), vec3(1.3, 0.08, 0.15), metal);
            b.cube(&id, vec3(cx, g + 0.04, cz), vec3(0.15, 0.08, 1.3), metal);
            let d = pad - vec3(cx, g, cz);
            let cm = Mat4::from_translation(vec3(cx, g + 1.65, cz)) * Mat4::from_rotation_y(d.x.atan2(d.z));
            b.cube(&cm, Vec3::ZERO, vec3(0.6, 0.55, 1.0), rgb(0.2, 0.2, 0.23));
            b.cube(&cm, vec3(0.0, 0.0, 0.62), vec3(0.42, 0.42, 0.3), rgb(0.05, 0.05, 0.06));
            b.glow(&cm, vec3(0.0, 0.33, 0.25), vec3(0.14, 0.1, 0.14), if air { RED } else { Color::new(0.3, 0.08, 0.08, 1.0) });
        }
        // Âncora robô atrás da bancada
        let talk = get_time() - self.a_t < 10.0 || air;
        let bob = (time * 1.7).sin() * 0.04;
        let (rx, rz) = (64.5, 286.7);
        b.cube(&id, vec3(rx, g + 1.3, rz), vec3(1.1, 1.0, 0.6), rgb(0.08, 0.1, 0.2));
        b.glow(&id, vec3(rx, g + 1.45, rz - 0.31), vec3(0.12, 0.6, 0.02), RED);
        let hm = Mat4::from_translation(vec3(rx, g + 2.2 + bob, rz)) * Mat4::from_rotation_y(PI);
        b.cube(&hm, Vec3::ZERO, vec3(0.85, 0.7, 0.7), rgb(0.7, 0.75, 0.82));
        for ex in [-0.18, 0.18] {
            b.glow(&hm, vec3(ex, 0.08, 0.36), vec3(0.14, 0.12, 0.02), CYAN);
        }
        let mh = if talk { 0.05 + (time * 13.0).sin().abs() * 0.12 } else { 0.04 };
        b.glow(&hm, vec3(0.0, -0.15, 0.36), vec3(0.3, mh, 0.02), CYAN);
        b.cube(&hm, vec3(0.0, 0.55, 0.0), vec3(0.06, 0.4, 0.06), metal);
        b.glow(&hm, vec3(0.0, 0.78, 0.0), vec3(0.14, 0.14, 0.14), if blink { RED } else { Color::new(0.3, 0.05, 0.05, 1.0) });
        b.glow(&id, vec3(64.5, g + 0.75, 283.97), vec3(13.0, 0.12, 0.05), Color::new(1.0, 0.25 + 0.2 * k, 0.25, 1.0));
        // Pad AO VIVO: placa pulsando, coluna de carga e feixe quando tem gente no ar
        let p = (time * 3.0).sin() * 0.5 + 0.5;
        trans.glow(&id, vec3(PAD.x, g + 0.03, PAD.y), vec3(3.0, 0.04, 3.0), Color::new(1.0, 0.15, 0.2, 0.25 + 0.25 * p));
        if self.pad > 0.0 {
            let h = 2.6 * self.pad / 1.5;
            trans.glow(&id, vec3(PAD.x, g + h * 0.5, PAD.y), vec3(3.05, h, 3.05), Color::new(1.0, 0.3, 0.3, 0.18));
        }
        if air && hi {
            trans.glow(&id, vec3(PAD.x, g + 3.0, PAD.y), vec3(2.6, 6.0, 2.6), Color::new(1.0, 0.2, 0.25, 0.08 + 0.05 * p));
        }
        let lp = vec3(PAD.x, g + 2.8, PAD.y);
        if lp.distance(eye) < 35.0 {
            let text = match self.air_now() {
                Some((name, _, end, _)) => format!("NO AR: {} ({:.0}s)", name, (end - get_time()).max(0.0)),
                None if self.pad > 0.0 => format!("SEGURA... {:.0}%", self.pad / 1.5 * 100.0),
                None if time < self.cool => "AGUARDANDO A PRODUCAO...".into(),
                None => "AO VIVO - fica 1,5s aqui e vira noticia".into(),
            };
            labels.push(Label { pos: lp, text, size: 20.0, color: Color::new(1.0, 0.45, 0.45, 1.0) });
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.scr.draw(&geo(), time, eye, 140.0);
    }
}

pub fn build(w: &mut World) {
    super::clear_lot(w, crate::layout::TV, 24, GRASS);
    let (x0, z0, x1, z1) = (46, 266, 82, 290);
    let g = G;
    // Casca preta: salão 12 de altura, fachada norte sobe até G+17 (telão)
    fill(w, (x0, g - 1, z0), (x1, g - 1, z1), PLANKS);
    fill(w, (x0, g, z0), (x1, g + 11, z1), BLACK);
    fill(w, (x0 + 1, g, z0 + 1), (x1 - 1, g + 11, z1 - 1), AIR);
    fill(w, (x0, g + 12, z0), (x1, g + 12, z1), BLACK);
    fill(w, (x0, g + 12, z0), (x1, g + 17, z0 + 1), BLACK);
    for x in [x0, x1] {
        fill(w, (x, g, z0), (x, g + 17, z0), NEON);
    }
    fill(w, (x0, g + 17, z0), (x1, g + 17, z0), NEON);
    fill(w, (x0 + 1, g + 4, z0), (x1 - 1, g + 4, z0), NEON);
    // Porta (trilha chega em x 61..63)
    fill(w, (60, g, z0), (64, g + 3, z0 + 1), AIR);
    // Janelas laterais e luz no teto
    for z in (z0 + 4..z1 - 2).step_by(4) {
        fill(w, (x0, g + 3, z), (x0, g + 6, z + 1), GLASS);
        fill(w, (x1, g + 3, z), (x1, g + 6, z + 1), GLASS);
    }
    for x in (x0 + 4..x1).step_by(6) {
        for z in (z0 + 4..z1).step_by(6) {
            w.set(x, g + 12, z, NEON);
        }
    }
    // Estúdio: piso preto, bancada, pad AO VIVO, fundo de vidro com faixa de neon
    fill(w, (x0 + 1, g - 1, 278), (x1 - 1, g - 1, z1 - 1), BLACK);
    fill(w, (58, g, 284), (70, g, 285), BLACK);
    fill(w, (63, g - 1, 280), (65, g - 1, 282), NEON);
    fill(w, (52, g + 2, z1), (76, g + 7, z1), GLASS);
    fill(w, (52, g + 8, z1 - 1), (76, g + 8, z1 - 1), NEON);
}
