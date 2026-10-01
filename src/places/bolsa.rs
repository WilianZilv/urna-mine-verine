//! Bolsa de Valores da Vila: ações fictícias que reagem à arena e à cidade (server/bolsa.js).
//! Pregão aberto a oeste com o painel holográfico virado pra avenida; salão a leste com letreiro LED
//! rolando em cima da porta (textura só repinta quando a cotação muda; o rolar é UV) e touro de neon.

use super::{Geo, Place, Screen, TH, TW, fill, hash_str, s, wrap};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{CYAN, DIM, GOLD, PINK, SOFT, fit, frame, frame_at, holo_fx, mipmaps, text_right};
use crate::world::{AIR, BLACK, COBBLE, G, GLASS, LEAVES, NEON, STONE, World};
use macroquad::prelude::*;
use serde_json::Value;

/// Painel: pé em (PX, PZ), tela virada pra -x (avenida).
const PX: f32 = 250.0;
const PZ: f32 = 83.0;
const PW: f32 = 22.0;
const PH: f32 = 11.0;
const PY: f32 = G as f32 + 9.0;
/// Letreiro LED na fachada do salão, em cima da porta (x 258, z 96..100).
const LX: f32 = 257.88;
const LY: f32 = G as f32 + 6.0;
const LZ: f32 = 98.5;
const LW: f32 = 8.0;
const LH: f32 = 1.4;
const TAPE_H: f32 = 128.0;
const SPEED: f32 = 260.0;
/// Touro em cima da coroa da fachada (x 258..261, topo em G+17), acima da porta.
const BULL: Vec3 = vec3(259.8, G as f32 + 17.0, 98.0);

const GREEN: Color = Color::new(0.35, 1.0, 0.5, 1.0);
const RED: Color = Color::new(1.0, 0.4, 0.4, 1.0);

fn geo() -> Geo {
    Geo { c: vec3(PX, PY, PZ), n: vec3(-1.0, 0.0, 0.0), w: PW, h: PH }
}

fn trend(d: f32) -> Color {
    if d > 0.0 {
        GREEN
    } else if d < 0.0 {
        RED
    } else {
        SOFT
    }
}

fn pct(d: f32) -> String {
    format!("{}{d:.1}%", if d > 0.0 { "+" } else { "" })
}

struct Tk {
    s: String,
    n: String,
    p: f32,
    d: f32,
    h: Vec<f32>,
    w: String,
    /// Criador (só ações de mods).
    c: String,
}

pub struct Bolsa {
    tk: Vec<Tk>,
    /// Ações de mods listados (página MODS do painel, alterna com a principal).
    mk: Vec<Tk>,
    top: Vec<(String, i64)>,
    hot: String,
    /// Índice da vila (média das variações, %), humor do alerta (-1 crash, 0, +1 rali) e texto.
    ix: f32,
    mood: i32,
    alert: String,
    key: u64,
    flash: f32,
    screen: Screen,
    tape: Option<RenderTarget>,
    tape_key: u64,
    tape_w: f32,
    tape_u: f32,
}

impl Bolsa {
    pub fn new() -> Self {
        Bolsa {
            tk: Vec::new(),
            mk: Vec::new(),
            top: Vec::new(),
            hot: String::new(),
            ix: 0.0,
            mood: 0,
            alert: String::new(),
            key: 0,
            flash: 0.0,
            screen: Screen::new(),
            tape: None,
            tape_key: u64::MAX,
            tape_w: 1.0,
            tape_u: 0.0,
        }
    }

    fn alert_color(&self) -> Color {
        if self.mood < 0 { RED } else { GREEN }
    }

    /// Página MODS: 10 s a cada 24 s, se tiver mod listado.
    fn mods_page(&self, time: f32) -> bool {
        !self.mk.is_empty() && time.rem_euclid(24.0) >= 14.0
    }

    fn paint_mods(&self, time: f32) {
        let (cw, ch) = (446.0, 300.0);
        for (i, t) in self.mk.iter().take(6).enumerate() {
            card(40.0 + (i % 3) as f32 * (cw + 20.0), 140.0 + (i / 3) as f32 * (ch + 20.0), cw, ch, t, time);
        }
        let (rx, rw) = (1440.0, TW - 1480.0);
        frame(rx, 140.0, rw, 620.0, "COMO VIRA ACAO");
        let how = "todo mod npc ativo concorre. os 6 mais populares (porrada, abates, tempo no ar e compras) entram no pregao a 50.00. desativou? o cofre paga o ultimo preco pros donos.";
        for (j, l) in wrap(how, rw - 48.0, 36.0, 11).iter().enumerate() {
            draw_text(l, rx + 24.0, 220.0 + j as f32 * 46.0, 36.0, if j == 0 { GOLD } else { WHITE });
        }
        draw_text("/bolsa mods", rx + 24.0, 740.0, 34.0, CYAN);
    }

    fn paint(&self, time: f32) {
        let mods = self.mods_page(time);
        draw_text(if mods { "BOLSA DE VALORES DA VILA  -  ACOES DE MODS" } else { "BOLSA DE VALORES DA VILA  -  ACOES FICTICIAS" }, 40.0, 84.0, 70.0, CYAN);
        let blink = (time * 1.5).fract() < 0.7;
        let (st, stc) = if self.tk.is_empty() {
            ("CONECTANDO...".to_string(), GOLD)
        } else if self.mood != 0 {
            (if (time * 3.0).fract() < 0.6 { if self.mood < 0 { "CIRCUIT BREAKER" } else { "DISPAROU" } } else { "" }.to_string(), self.alert_color())
        } else {
            (if blink { format!("INDICE {}", pct(self.ix)) } else { "PREGAO ABERTO".into() }, if blink { trend(self.ix) } else { GOLD })
        };
        text_right(&st, TW - 40.0, 80.0, 44.0, stc);
        draw_rectangle(40.0, 120.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

        if mods {
            self.paint_mods(time);
        } else {
            let (cw, ch) = (330.0, 300.0);
            for (i, t) in self.tk.iter().take(8).enumerate() {
                card(40.0 + (i % 4) as f32 * (cw + 20.0), 140.0 + (i / 4) as f32 * (ch + 20.0), cw, ch, t, time);
            }

            let (rx, rw) = (1440.0, TW - 1480.0);
            frame(rx, 140.0, rw, 620.0, "MAIORES INVESTIDORES");
            for (i, (n, v)) in self.top.iter().enumerate() {
                let y = 250.0 + i as f32 * 96.0;
                let c = if i == 0 { GOLD } else { WHITE };
                draw_text(&format!("{}", i + 1), rx + 24.0, y, 56.0, c);
                draw_text(&fit(n, rw - 280.0, 46.0), rx + 80.0, y, 46.0, c);
                text_right(&format!("{v}"), rx + rw - 24.0, y, 50.0, GOLD);
            }
            if self.top.is_empty() {
                for (j, l) in wrap("ninguem investiu ainda. seja o primeiro trouxa: /investir LULA 5", rw - 48.0, 38.0, 4).iter().enumerate() {
                    draw_text(l, rx + 24.0, 250.0 + j as f32 * 46.0, 38.0, SOFT);
                }
            }
            draw_text("valor em acoes (moedas ficticias)", rx + 24.0, 740.0, 28.0, DIM);
        }

        frame(40.0, 780.0, TW - 80.0, 150.0, "ANALISTA DA VILA");
        let hot = self.hot.trim_start_matches("ANALISTA: ");
        for (j, l) in wrap(hot, TW - 140.0, 44.0, 2).iter().enumerate() {
            draw_text(l, 70.0, 866.0 + j as f32 * 48.0, 44.0, if j == 0 { PINK } else { WHITE });
        }
        let sym = if mods { self.mk[0].s.as_str() } else { "LULA" };
        draw_text(&format!("/investir {sym} 5   /vender {sym} 5   /carteira   -   moeda ficticia"), 40.0, 990.0, 38.0, SOFT);
        text_right("cotacao a cada 20s  -  taxa 1% vai pro cofre da IA", TW - 40.0, 990.0, 30.0, DIM);
        holo_fx(time, self.flash);
        if self.mood != 0 {
            let c = self.alert_color();
            let a = if (time * 4.0).fract() < 0.5 { 1.0 } else { 0.25 };
            draw_rectangle_lines(10.0, 10.0, TW - 20.0, TH - 20.0, 16.0, Color::new(c.r, c.g, c.b, a));
            draw_rectangle(40.0, 780.0, TW - 80.0, 150.0, Color::new(0.03, 0.02, 0.03, 0.92));
            draw_rectangle_lines(40.0, 780.0, TW - 80.0, 150.0, 6.0, c);
            for (j, l) in wrap(&self.alert, TW - 140.0, 48.0, 2).iter().enumerate() {
                draw_text(l, 70.0, 846.0 + j as f32 * 54.0, 48.0, if j == 0 { c } else { WHITE });
            }
        }
    }

    /// Fita do letreiro: todas as cotações numa faixa só (largura lógica = largura do texto).
    fn render_tape(&mut self, eye: Vec3) {
        if eye.distance(vec3(LX, LY, LZ)) > 110.0 || eye.x > LX {
            return;
        }
        let (pw, ph) = crate::quality::pick([(1024u32, 32u32), (2048, 64), (2048, 64)]);
        let fresh = self.tape.as_ref().is_some_and(|rt| rt.texture.width() as u32 == pw);
        if fresh && self.tape_key == self.key {
            return;
        }
        let size = 92.0;
        let sep = ("   *   ".to_string(), DIM);
        let mut segs = vec![("BOLSA DE VALORES DA VILA".to_string(), GOLD), sep.clone()];
        if self.mood != 0 {
            segs.push((self.alert.clone(), self.alert_color()));
            segs.push(sep.clone());
        }
        for t in self.tk.iter().chain(&self.mk) {
            segs.push((format!("{} ", t.s), GOLD));
            segs.push((format!("{:.2} ", t.p), WHITE));
            segs.push((pct(t.d), trend(t.d)));
            segs.push(sep.clone());
        }
        if self.tk.is_empty() {
            segs.push(("PREGAO ABRINDO...".into(), SOFT));
            segs.push(sep);
        }
        let ws: Vec<f32> = segs.iter().map(|(t, _)| measure_text(t, None, size as u16, 1.0).width).collect();
        let lw = ws.iter().sum::<f32>().max(1.0);
        if !fresh {
            self.tape = None;
        }
        let rt = self
            .tape
            .get_or_insert_with(|| {
                let rt = render_target(pw, ph);
                rt.texture.set_filter(FilterMode::Linear);
                rt
            })
            .clone();
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, lw, TAPE_H));
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
        let mut x = 0.0;
        for ((t, c), w) in segs.iter().zip(&ws) {
            draw_text(t, x, 98.0, size, *c);
            x += w;
        }
        let mut y = 0.0;
        while y < TAPE_H {
            draw_rectangle(0.0, y, lw, 3.0, Color::new(0.0, 0.0, 0.0, 0.4));
            y += 8.0;
        }
        set_default_camera();
        mipmaps(&rt);
        self.tape_key = self.key;
        self.tape_w = lw;
    }

    /// Janela da fita rolando por UV (dois quads quando dá a volta).
    fn draw_tape(&self, eye: Vec3) {
        let Some(rt) = &self.tape else { return };
        if eye.x > LX - 0.02 || eye.distance(vec3(LX, LY, LZ)) > 140.0 {
            return;
        }
        let f = (LW / LH * TAPE_H / self.tape_w).min(1.0);
        let u0 = self.tape_u;
        let (y0, y1, z0) = (LY - LH * 0.5, LY + LH * 0.5, LZ - LW * 0.5);
        let quad = |za: f32, zb: f32, ua: f32, ub: f32| {
            draw_mesh(&Mesh {
                vertices: vec![
                    Vertex::new(LX, y1, za, ua, 1.0, WHITE),
                    Vertex::new(LX, y1, zb, ub, 1.0, WHITE),
                    Vertex::new(LX, y0, zb, ub, 0.0, WHITE),
                    Vertex::new(LX, y0, za, ua, 0.0, WHITE),
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                texture: Some(rt.texture.clone()),
            })
        };
        if u0 + f <= 1.0 {
            quad(z0, z0 + LW, u0, u0 + f);
        } else {
            let zm = z0 + LW * (1.0 - u0) / f;
            quad(z0, zm, u0, 1.0);
            quad(zm, z0 + LW, 0.0, u0 + f - 1.0);
        }
    }
}

fn card(x: f32, y: f32, w: f32, h: f32, t: &Tk, time: f32) {
    let col = trend(t.d);
    frame(x, y, w, h, "");
    draw_rectangle(x, y + h - 6.0, w, 6.0, Color::new(col.r, col.g, col.b, 0.6));
    draw_text(&t.s, x + 18.0, y + 64.0, 62.0, GOLD);
    text_right(&fit(&t.n, w - 190.0, 24.0), x + w - 16.0, y + 40.0, 24.0, SOFT);
    if !t.c.is_empty() {
        text_right(&fit(&format!("por {}", t.c), w - 190.0, 22.0), x + w - 16.0, y + 68.0, 22.0, DIM);
    }
    draw_text(&format!("{:.2}", t.p), x + 18.0, y + 134.0, 64.0, WHITE);
    text_right(&pct(t.d), x + w - 16.0, y + 132.0, 42.0, col);
    spark(x + 18.0, y + 156.0, w - 36.0, 92.0, &t.h, col, time);
    draw_text(&fit(&t.w, w - 36.0, 24.0), x + 18.0, y + h - 20.0, 24.0, DIM);
}

/// Mini gráfico do histórico (linha + área).
fn spark(x: f32, y: f32, w: f32, h: f32, v: &[f32], col: Color, time: f32) {
    draw_rectangle(x, y, w, h, Color::new(0.45, 1.0, 1.0, 0.06));
    if v.len() < 2 {
        return;
    }
    let (lo, hi) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
    let span = (hi - lo).max(0.5);
    let pt = |i: usize| vec2(x + w * i as f32 / (v.len() - 1) as f32, y + h - h * (v[i] - lo) / span);
    let fillc = Color::new(col.r, col.g, col.b, 0.15);
    for i in 1..v.len() {
        let (a, b) = (pt(i - 1), pt(i));
        draw_triangle(a, b, vec2(b.x, y + h), fillc);
        draw_triangle(a, vec2(b.x, y + h), vec2(a.x, y + h), fillc);
        draw_line(a.x, a.y, b.x, b.y, 4.0, col);
    }
    let e = pt(v.len() - 1);
    draw_circle(e.x, e.y, 6.0 + 3.0 * (time * 4.0).sin().abs(), WHITE);
}

/// Touro de neon dourado investindo pra oeste (cabeça em -x), e a seta do humor do mercado em cima.
fn bull(b: &mut Batch, trans: &mut Batch, time: f32, mood: f32, hi: bool) {
    let m = Mat4::from_translation(BULL);
    let gold = Color::new(1.0, 0.74, 0.16, 1.0);
    let deep = Color::new(0.72, 0.48, 0.08, 1.0);
    let k = 0.75 + 0.25 * (time * 2.2).sin();
    let neon = Color::new(1.0, 0.85 * k + 0.1, 0.35 * k, 1.0);
    for (lx, lz) in [(-1.2, -0.5), (-1.2, 0.5), (1.2, -0.5), (1.2, 0.5)] {
        b.cube(&m, vec3(lx, 0.55, lz), vec3(0.45, 1.1, 0.45), deep);
    }
    b.cube(&m, vec3(0.2, 1.7, 0.0), vec3(3.0, 1.3, 1.5), gold);
    b.cube(&m, vec3(-0.8, 2.25, 0.0), vec3(1.4, 0.6, 1.6), gold);
    b.cube(&m, vec3(-1.25, 1.5, 0.0), vec3(0.8, 1.2, 1.4), gold);
    b.cube(&m, vec3(-2.0, 1.5, 0.0), vec3(0.9, 0.9, 1.0), gold);
    b.cube(&m, vec3(-2.55, 1.25, 0.0), vec3(0.4, 0.5, 0.8), deep);
    b.glow(&m, vec3(-2.0, 2.02, 0.0), vec3(0.3, 0.22, 1.8), neon);
    for z in [-1.0f32, 1.0] {
        b.glow(&m, vec3(-2.3, 2.2, z * 0.95), vec3(0.6, 0.2, 0.2), neon);
        b.glow(&m, vec3(-2.6, 2.5, z * 1.0), vec3(0.2, 0.5, 0.2), neon);
        b.glow(&m, vec3(-2.46, 1.72, z * 0.3), vec3(0.05, 0.14, 0.16), RED);
    }
    b.cube(&m, vec3(1.78, 1.95, 0.0), vec3(0.16, 0.6, 0.16), deep);
    b.glow(&m, vec3(1.85, 2.3, 0.0), vec3(0.3, 0.3, 0.3), neon);
    if hi {
        for (y, z) in [(2.36, -0.76), (2.36, 0.76), (1.04, -0.76), (1.04, 0.76)] {
            b.glow(&m, vec3(0.2, y, z), vec3(3.0, 0.06, 0.06), neon);
        }
        trans.glow(&m, vec3(0.0, 0.03, 0.0), vec3(4.0, 0.04, 3.2), Color::new(1.0, 0.8, 0.3, 0.15 + 0.1 * k));
    }
    arrow(b, time, 4.4, mood);
}

/// Urso de neon vermelho (mercado em baixa): corcunda alta, cabeça baixa e pata da frente dando a patada.
fn bear(b: &mut Batch, trans: &mut Batch, time: f32, mood: f32, hi: bool) {
    let m = Mat4::from_translation(BULL);
    let fur = Color::new(0.78, 0.1, 0.12, 1.0);
    let deep = Color::new(0.45, 0.05, 0.07, 1.0);
    let k = 0.75 + 0.25 * (time * 3.1).sin();
    let neon = Color::new(1.0, 0.12 + 0.1 * k, 0.18 * k, 1.0);
    for (lx, lz) in [(-1.1, 0.55), (1.1, -0.55), (1.1, 0.55)] {
        b.cube(&m, vec3(lx, 0.5, lz), vec3(0.65, 1.0, 0.65), deep);
    }
    let swipe = 0.3 * (time * 2.5).sin();
    b.cube(&m, vec3(-1.6, 1.9 + swipe, -0.75), vec3(0.6, 1.3, 0.55), deep);
    for i in 0..3 {
        b.glow(&m, vec3(-1.95, 1.35 + swipe, -0.95 + i as f32 * 0.2), vec3(0.35, 0.08, 0.08), neon);
    }
    b.cube(&m, vec3(0.1, 1.75, 0.0), vec3(3.0, 1.5, 1.8), fur);
    b.cube(&m, vec3(-0.6, 2.65, 0.0), vec3(1.3, 0.5, 1.6), fur);
    b.cube(&m, vec3(-1.85, 1.75, 0.0), vec3(1.0, 0.95, 1.05), fur);
    b.cube(&m, vec3(-2.5, 1.6, 0.0), vec3(0.45, 0.45, 0.6), deep);
    b.cube(&m, vec3(1.7, 2.1, 0.0), vec3(0.3, 0.3, 0.3), deep);
    for z in [-0.35f32, 0.35] {
        b.cube(&m, vec3(-1.75, 2.35, z), vec3(0.25, 0.3, 0.25), deep);
        b.glow(&m, vec3(-2.36, 1.95, z * 0.7), vec3(0.05, 0.12, 0.14), Color::new(1.0, 0.9, 0.3, 1.0));
    }
    if hi {
        b.glow(&m, vec3(-0.3, 2.92, 0.0), vec3(2.6, 0.06, 0.08), neon);
        for z in [-0.92f32, 0.92] {
            b.glow(&m, vec3(0.1, 1.05, z), vec3(3.0, 0.06, 0.06), neon);
        }
        trans.glow(&m, vec3(0.0, 0.03, 0.0), vec3(4.0, 0.04, 3.2), Color::new(1.0, 0.15, 0.15, 0.15 + 0.1 * k));
    }
    arrow(b, time, 4.0, mood);
}

/// Seta girando em cima da estátua: índice da vila (verde pra cima, vermelho pra baixo).
fn arrow(b: &mut Batch, time: f32, h: f32, mood: f32) {
    let (col, up) = if mood >= 0.0 { (GREEN, 1.0) } else { (RED, -1.0) };
    let y = h + 0.25 * (time * 1.8).sin();
    let spin = Mat4::from_translation(BULL + vec3(0.0, y, 0.0)) * Mat4::from_rotation_y(time * 1.2);
    b.glow(&spin, vec3(0.0, -0.3 * up, 0.0), vec3(0.3, 0.9, 0.3), col);
    b.glow(&spin, vec3(0.0, 0.3 * up, 0.0), vec3(1.0, 0.25, 0.3), col);
    b.glow(&spin, vec3(0.0, 0.5 * up, 0.0), vec3(0.5, 0.2, 0.3), col);
}

/// Giroflex no teto do salão durante o alerta: cúpula piscando e dois fachos (vermelho e verde) girando.
fn sirens(b: &mut Batch, time: f32, n: usize) {
    let y = G as f32 + 12.0;
    for (i, (x, z)) in [(260.5, 74.5), (268.5, 92.5), (268.5, 74.5), (260.5, 92.5)].into_iter().take(n).enumerate() {
        let m = Mat4::from_translation(vec3(x, y, z));
        b.cube(&m, vec3(0.0, 0.15, 0.0), vec3(0.7, 0.3, 0.7), Color::new(0.1, 0.1, 0.12, 1.0));
        let p = (time * 8.0 + i as f32).sin() > 0.0;
        b.glow(&m, vec3(0.0, 0.5, 0.0), vec3(0.45, 0.4, 0.45), if p { RED } else { GREEN });
        let spin = m * Mat4::from_rotation_y(time * 6.0 + i as f32 * 1.6);
        b.glow(&spin, vec3(1.1, 0.5, 0.0), vec3(1.8, 0.12, 0.12), RED);
        b.glow(&spin, vec3(-1.1, 0.5, 0.0), vec3(1.8, 0.12, 0.12), GREEN);
    }
}

impl Place for Bolsa {
    fn on_msg(&mut self, m: &Value) {
        let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
        let arr = |v: &Value| v.as_array().cloned().unwrap_or_default();
        let tks = |v: &Value| -> Vec<Tk> {
            arr(v)
                .iter()
                .map(|t| Tk { s: s(&t["s"]), n: s(&t["n"]), p: f(&t["p"]), d: f(&t["d"]), h: arr(&t["h"]).iter().map(f).collect(), w: s(&t["w"]), c: s(&t["c"]) })
                .collect()
        };
        self.tk = tks(&m["tk"]);
        self.mk = tks(&m["mk"]);
        self.top = arr(&m["top"]).iter().map(|e| (s(&e[0]), e[1].as_i64().unwrap_or(0))).collect();
        self.hot = s(&m["hot"]);
        self.ix = f(&m["ix"]);
        self.mood = m["mood"].as_i64().unwrap_or(0).signum() as i32;
        self.alert = s(&m["alert"]);
        self.key = hash_str(&m.to_string());
        self.flash = 1.0;
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        let dt = get_frame_time().min(0.1);
        self.flash = (self.flash - dt * 0.8).max(0.0);
        let speed = if self.mood != 0 { SPEED * 3.0 } else { SPEED };
        self.tape_u = (self.tape_u + dt * speed / self.tape_w).fract();
        let g = geo();
        if self.screen.begin(time, eye, &g, self.key, true) {
            self.paint(time);
            self.screen.end();
        }
        self.render_tape(eye);
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        if eye.distance(vec3(252.0, G as f32, 86.0)) > 130.0 {
            return;
        }
        let hi = crate::quality::pick([false, true, true]);
        if self.ix < 0.0 {
            bear(b, trans, time, self.ix, hi);
        } else {
            bull(b, trans, time, self.ix, hi);
        }
        if self.mood != 0 {
            sirens(b, time, crate::quality::pick([2, 4, 4]));
            let on = (time * 4.0).fract() < 0.5;
            frame_at(b, trans, &Mat4::from_translation(vec3(PX, 0.0, PZ)), (PW, PH, PY), time, if on { 1.0 } else { 0.0 });
            if on {
                let c = self.alert_color();
                let id = Mat4::IDENTITY;
                for y in [PY - PH * 0.5 - 0.25, PY + PH * 0.5 + 0.25] {
                    b.glow(&id, vec3(PX - 0.1, y, PZ), vec3(0.1, 0.2, PW + 0.7), c);
                }
                for z in [PZ - PW * 0.5 - 0.25, PZ + PW * 0.5 + 0.25] {
                    b.glow(&id, vec3(PX - 0.1, PY, z), vec3(0.1, PH + 0.7, 0.2), c);
                }
            }
        } else {
            frame_at(b, trans, &Mat4::from_translation(vec3(PX, 0.0, PZ)), (PW, PH, PY), time, self.flash);
        }
        // Monitores do pregão (verde/vermelho piscando) em cima das mesas
        if hi {
            let id = Mat4::IDENTITY;
            for (i, (x, z)) in [242, 245].iter().flat_map(|&x| (75..=79).chain(87..=91).map(move |z| (x, z))).enumerate() {
                let c = vec3(x as f32 + 0.5, G as f32 + 1.35, z as f32 + 0.5);
                b.cube(&id, c + vec3(0.12, 0.0, 0.0), vec3(0.1, 0.55, 0.8), Color::new(0.1, 0.1, 0.12, 1.0));
                let up = (time * 0.7 + i as f32 * 1.37).sin() > 0.0;
                b.glow(&id, c, vec3(0.06, 0.45, 0.7), if up { GREEN } else { RED });
            }
        }
        // Moldura do letreiro
        let k = 0.7 + 0.3 * (time * 3.0).sin();
        let edge = Color::new(1.0, 0.8 * k, 0.25, 1.0);
        for y in [LY - LH * 0.5 - 0.08, LY + LH * 0.5 + 0.08] {
            b.glow(&Mat4::IDENTITY, vec3(LX - 0.02, y, LZ), vec3(0.06, 0.1, LW + 0.2), edge);
        }
        if eye.distance(vec3(LX, G as f32 + 10.0, LZ)) < 70.0 {
            labels.push(Label { pos: vec3(LX - 0.5, G as f32 + 10.5, LZ), text: "BOLSA DE VALORES DA VILA".into(), size: 24.0, color: GOLD });
        }
        if eye.distance(BULL) < 60.0 {
            let (text, color) = if self.ix < 0.0 {
                ("URSO DA BAIXA (hiberna no prejuizo)", Color::new(1.0, 0.75, 0.75, 1.0))
            } else {
                ("TOURO DA ALTA (so sobe na fe)", Color::new(1.0, 0.95, 0.75, 1.0))
            };
            labels.push(Label { pos: BULL + vec3(0.0, 6.0, 0.0), text: text.into(), size: 16.0, color });
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.screen.draw(&geo(), time, eye, 140.0);
        self.draw_tape(eye);
    }
}

pub fn build(w: &mut World) {
    let g = G;
    super::clear_lot(w, crate::layout::BOLSA, 20, STONE);
    // Calçada que continua a avenida até a porta; pregão de piso preto com friso de neon
    fill(w, (234, g - 1, 96), (257, g - 1, 100), COBBLE);
    fill(w, (236, g - 1, 72), (248, g - 1, 94), NEON);
    fill(w, (237, g - 1, 73), (247, g - 1, 93), BLACK);
    fill(w, (234, g, 70), (247, g, 70), LEAVES);
    // Mesas do pregão
    for x in [242, 245] {
        fill(w, (x, g, 75), (x, g, 79), BLACK);
        fill(w, (x, g, 87), (x, g, 91), BLACK);
    }

    // Salão: caixa de pedra oca, piso preto, claraboia de vidro
    let (x0, x1, z0, z1, top) = (258, 270, 72, 102, g + 11);
    fill(w, (x0, g, z0), (x1, top, z1), STONE);
    fill(w, (x0 + 1, g, z0 + 1), (x1 - 1, top - 1, z1 - 1), AIR);
    fill(w, (x0 + 1, g - 1, z0 + 1), (x1 - 1, g - 1, z1 - 1), BLACK);
    fill(w, (x0 + 1, g - 1, 98), (x1 - 1, g - 1, 98), NEON);
    fill(w, (x0 + 3, top, z0 + 4), (x1 - 3, top, z1 - 4), GLASS);
    // Fachada oeste: colunas a cada 4 com vidro entre, frisos de neon
    for z in z0 + 1..94 {
        if (z - z0) % 4 != 0 {
            fill(w, (x0, g + 1, z), (x0, g + 8, z), GLASS);
        }
    }
    fill(w, (x0, g + 9, z0), (x0, g + 9, z1), NEON);
    fill(w, (x0, top + 1, z0), (x0, top + 1, z1), NEON);
    // Laterais e fundo com faixas de vidro
    for x in x0 + 1..x1 {
        if x % 3 != 0 {
            fill(w, (x, g + 2, z0), (x, g + 7, z0), GLASS);
            fill(w, (x, g + 2, z1), (x, g + 7, z1), GLASS);
        }
    }
    for z in z0 + 1..z1 {
        if z % 3 != 0 {
            fill(w, (x1, g + 2, z), (x1, g + 7, z), GLASS);
        }
    }
    // Portal: porta alta com moldura de neon, faixa preta do letreiro LED e coroa escalonada
    fill(w, (x0, g, 96), (x0, g + 4, 100), AIR);
    fill(w, (x0, g, 95), (x0, g + 4, 95), NEON);
    fill(w, (x0, g, 101), (x0, g + 4, 101), NEON);
    fill(w, (x0, g + 5, 94), (x0, g + 6, z1), BLACK);
    fill(w, (x0, g + 7, 94), (x0, g + 8, z1), STONE);
    fill(w, (x0, top + 1, 94), (x0 + 3, g + 15, z1 - 1), STONE);
    fill(w, (x0, g + 13, 96), (x0, g + 14, 100), GLASS);
    fill(w, (x0, g + 16, 94), (x0 + 3, g + 16, z1 - 1), NEON);
    fill(w, (x0 + 1, g + 16, 95), (x0 + 2, g + 16, z1 - 2), STONE);
}
