//! Painel holográfico FÍSICO na entrada do lab: o conteúdo (números, gráficos, cards) é desenhado numa
//! render target e aplicado num quad 3D com teste de profundidade, preso entre pilares e um emissor.

use super::*;
use macroquad::miniquad::{FilterMode as MqFilter, MipmapFilterMode};
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

static LAST_PAINT: AtomicU32 = AtomicU32::new(0);

const TW: f32 = 2048.0;
const TH: f32 = 1024.0;
/// Tela no mundo: largura em z, altura em y, centro.
const PW: f32 = 20.0;
const PH: f32 = 10.0;
const PY: f32 = G as f32 + 8.5;

pub(crate) const CYAN: Color = Color::new(0.45, 1.0, 1.0, 1.0);
pub(crate) const DIM: Color = Color::new(0.45, 1.0, 1.0, 0.35);
pub(crate) const PINK: Color = Color::new(1.0, 0.55, 0.85, 1.0);
pub(crate) const GOLD: Color = Color::new(1.0, 0.85, 0.3, 1.0);
pub(crate) const SOFT: Color = Color::new(0.75, 0.9, 0.95, 1.0);

pub(crate) fn fit(t: &str, w: f32, size: f32) -> String {
    if measure_text(t, None, size as u16, 1.0).width <= w {
        return t.to_string();
    }
    let mut out: String = t.to_string();
    while !out.is_empty() && measure_text(&format!("{out}..."), None, size as u16, 1.0).width > w {
        out.pop();
    }
    format!("{}...", out.trim_end())
}

/// Quebra por largura em pixels, até `max` linhas (a última ganha "..." se sobrar).
fn wrap_px(t: &str, w: f32, size: f32, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in t.split_whitespace() {
        let cur = lines.last().unwrap();
        let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if measure_text(&next, None, size as u16, 1.0).width > w && !cur.is_empty() {
            if lines.len() == max {
                let last = lines.pop().unwrap();
                lines.push(fit(&format!("{last} {word}..."), w, size));
                return lines;
            }
            lines.push(word.to_string());
        } else {
            *lines.last_mut().unwrap() = next;
        }
    }
    lines
}

pub(crate) fn text_right(t: &str, x: f32, y: f32, size: f32, col: Color) {
    let w = measure_text(t, None, size as u16, 1.0).width;
    draw_text(t, x - w, y, size, col);
}

fn text_mid(t: &str, x: f32, y: f32, size: f32, col: Color) {
    let w = measure_text(t, None, size as u16, 1.0).width;
    draw_text(t, x - w * 0.5, y, size, col);
}

/// Caixa de gráfico com título.
pub(crate) fn frame(x: f32, y: f32, w: f32, h: f32, title: &str) {
    draw_rectangle(x, y, w, h, Color::new(0.05, 0.35, 0.45, 0.22));
    draw_rectangle_lines(x, y, w, h, 3.0, DIM);
    draw_rectangle(x, y, 26.0, 4.0, CYAN);
    if !title.is_empty() {
        draw_text(title, x + 16.0, y + 36.0, 30.0, CYAN);
    }
}

/// Anel (donut) de a0 até a1 radianos, começando no topo.
fn ring(cx: f32, cy: f32, r0: f32, r1: f32, a0: f32, a1: f32, col: Color) {
    let n = ((a1 - a0).abs() / 0.08).ceil().max(1.0) as i32;
    for i in 0..n {
        let (t0, t1) = (a0 + (a1 - a0) * i as f32 / n as f32, a0 + (a1 - a0) * (i + 1) as f32 / n as f32);
        let p = |a: f32, r: f32| vec2(cx + a.sin() * r, cy - a.cos() * r);
        draw_triangle(p(t0, r0), p(t0, r1), p(t1, r1), col);
        draw_triangle(p(t0, r0), p(t1, r1), p(t1, r0), col);
    }
}

/// Efeito holograma: linhas de varredura, faixa correndo e borda que pisca com dado novo.
pub(crate) fn holo_fx(time: f32, flash: f32) {
    let mut y = 0.0;
    while y < TH {
        draw_rectangle(0.0, y, TW, 2.0, Color::new(0.0, 0.0, 0.0, 0.07));
        y += 8.0;
    }
    let band = (time * 140.0) % (TH + 200.0) - 100.0;
    draw_rectangle(0.0, band, TW, 60.0, Color::new(0.45, 1.0, 1.0, 0.05));
    draw_rectangle_lines(4.0, 4.0, TW - 8.0, TH - 8.0, 6.0, Color::new(0.45, 1.0, 1.0, 0.4 + 0.6 * flash));
}

/// Mipmaps: texto continua legível (sem serrilhado) de 15-25 blocos.
pub(crate) fn mipmaps(rt: &RenderTarget) {
    let mut gl = unsafe { get_internal_gl() };
    gl.flush();
    let id = rt.texture.raw_miniquad_id();
    gl.quad_context.texture_set_min_filter(id, MqFilter::Linear, MipmapFilterMode::Linear);
    gl.quad_context.texture_generate_mipmaps(id);
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl LabInfo {
    /// Redesenha a tela na render target (antes do passe 3D). Só perto do painel.
    pub fn render(&mut self, time: f32, eye: Vec3, guard: &Vida) {
        if eye.distance(vec3(PANEL_X, PY, PANEL_Z)) > 90.0 || eye.x > PANEL_X + 2.0 {
            return;
        }
        let dt = get_frame_time().min(0.1);
        let k = 1.0 - (-dt * 2.5).exp();
        let target = [self.papers, self.count, self.topics, self.cycles, self.fund].map(|v| v as f32);
        for (s, t) in self.shown.iter_mut().zip(target) {
            *s += (t - *s) * k;
            if (t - *s).abs() < 0.02 {
                *s = t;
            }
        }
        self.bars_shown.resize(self.bars.len(), 0.0);
        for (s, b) in self.bars_shown.iter_mut().zip(&self.bars) {
            *s += (b.1 - *s) * k;
        }
        self.card_t = (self.card_t + dt * 1.2).min(1.0);
        self.flash = (self.flash - dt * 0.8).max(0.0);

        // 2048x1024 + mipmaps todo frame pesa no celular: qualidade baixa/média redesenha menos vezes e menor
        let (every, res) = crate::quality::pick([(0.25, 0.5), (1.0 / 15.0, 1.0), (0.0, 1.0)]);
        let last = f32::from_bits(LAST_PAINT.load(Relaxed));
        if self.rt.as_ref().is_some_and(|rt| rt.texture.width() == TW * res) && time - last < every && time >= last {
            return;
        }
        LAST_PAINT.store(time.to_bits(), Relaxed);
        if self.rt.as_ref().is_some_and(|rt| rt.texture.width() != TW * res) {
            self.rt = None;
        }
        let rt = self
            .rt
            .get_or_insert_with(|| {
                let rt = render_target((TW * res) as u32, (TH * res) as u32);
                rt.texture.set_filter(FilterMode::Linear);
                rt
            })
            .clone();
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, TW, TH));
        cam.render_target = Some(rt.clone());
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.05, 0.09, 0.85));
        self.paint(time, guard);
        set_default_camera();
        mipmaps(&rt);
    }

    fn paint(&self, time: f32, guard: &Vida) {
        let since = get_time() - self.recv;
        // Cabeçalho
        draw_text("LABORATORIO DE PESQUISA DO CEREBRO", 40.0, 84.0, 76.0, CYAN);
        let status = if !self.got {
            "CONECTANDO...".to_string()
        } else if self.running {
            if (time * 3.0).fract() < 0.6 { "LENDO ARTIGOS AGORA".into() } else { String::new() }
        } else {
            format!("proximo ciclo em {}", dur((self.next - since).max(0.0)))
        };
        text_right(&status, TW - 40.0, 58.0, 40.0, if self.running { GOLD } else { CYAN });
        let ago = if self.ago < 0.0 { "nunca".into() } else { format!("ha {}", dur(self.ago + since)) };
        text_right(&format!("atualizado {ago}  -  artigos REAIS (Europe PMC)  -  fila {}", self.queue), TW - 40.0, 102.0, 30.0, SOFT);
        draw_rectangle(40.0, 120.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

        // Números grandes
        let labels = ["ARTIGOS LIDOS", "ACHADOS", "TEMAS", "CICLOS", "FUNDO (moedas ficticias)"];
        let tw = (TW - 80.0 - 4.0 * 20.0) / 5.0;
        for (i, l) in labels.iter().enumerate() {
            let x = 40.0 + i as f32 * (tw + 20.0);
            frame(x, 140.0, tw, 122.0, "");
            draw_text(l, x + 18.0, 178.0, 30.0, SOFT);
            draw_text(&format!("{:.0}", self.shown[i]), x + 18.0, 250.0, 76.0, if i == 4 { GOLD } else { WHITE });
        }

        // Barras: achados por tema
        let (x0, w0) = (40.0, 940.0);
        frame(x0, 282.0, w0, 320.0, "ACHADOS POR TEMA");
        let max = self.bars.iter().map(|b| b.1).fold(1.0f32, f32::max);
        for (i, ((name, _), shown)) in self.bars.iter().zip(&self.bars_shown).take(5).enumerate() {
            let y = 334.0 + i as f32 * 52.0;
            text_right(&fit(name, 240.0, 38.0), x0 + 270.0, y + 32.0, 38.0, SOFT);
            let full = w0 - 370.0;
            draw_rectangle(x0 + 285.0, y + 4.0, full, 36.0, Color::new(0.45, 1.0, 1.0, 0.08));
            let w = full * shown / max;
            draw_rectangle(x0 + 285.0, y + 4.0, w, 36.0, Color::new(0.3, 0.9, 1.0, 0.8));
            draw_rectangle(x0 + 285.0 + w - 4.0, y + 4.0, 4.0, 36.0, WHITE);
            draw_text(&format!("{:.0}", shown), x0 + 297.0 + w, y + 34.0, 38.0, WHITE);
        }
        if self.bars.is_empty() {
            draw_text("sem achados ainda", x0 + 20.0, 400.0, 30.0, SOFT);
        }

        // Artigos novos por ciclo (colunas) e saldo do fundo (linha com área)
        frame(x0, 620.0, 455.0, 185.0, "ARTIGOS POR CICLO");
        let (gx, gy, gw, gh) = (x0 + 20.0, 790.0, 415.0, 110.0);
        let pc_max = self.per_cycle.iter().copied().fold(1.0f32, f32::max);
        let n = self.per_cycle.len().max(1) as f32;
        for (i, v) in self.per_cycle.iter().enumerate() {
            let h = gh * v / pc_max;
            let bw = gw / n;
            let last = i + 1 == self.per_cycle.len();
            draw_rectangle(gx + i as f32 * bw + 2.0, gy - h.max(2.0), (bw - 4.0).max(2.0), h.max(2.0), if last { PINK } else { Color::new(0.3, 0.9, 1.0, 0.7) });
        }
        text_right(&format!("max {pc_max:.0}"), x0 + 440.0, 656.0, 26.0, SOFT);

        frame(x0 + 485.0, 620.0, 455.0, 185.0, "FUNDO DO LAB");
        let (fx, fw) = (x0 + 505.0, 415.0);
        let fh = &self.fund_hist;
        if fh.len() >= 2 {
            let (lo, hi) = fh.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
            let span = (hi - lo).max(1.0);
            let pt = |i: usize| vec2(fx + fw * i as f32 / (fh.len() - 1) as f32, gy - gh * (fh[i] - lo) / span);
            for i in 1..fh.len() {
                let (a, b) = (pt(i - 1), pt(i));
                draw_triangle(a, b, vec2(b.x, gy), Color::new(1.0, 0.85, 0.3, 0.15));
                draw_triangle(a, vec2(b.x, gy), vec2(a.x, gy), Color::new(1.0, 0.85, 0.3, 0.15));
                draw_line(a.x, a.y, b.x, b.y, 4.0, GOLD);
            }
            let end = pt(fh.len() - 1);
            draw_circle(end.x, end.y, 7.0 + 3.0 * (time * 4.0).sin().abs(), WHITE);
        } else {
            draw_text("sem historico ainda", fx, gy - 40.0, 28.0, SOFT);
        }
        text_right(&format!("{:.0}", self.shown[4]), x0 + 920.0, 656.0, 26.0, GOLD);

        // Orçamento diário (donut) + vida da guardiã
        frame(x0, 822.0, w0, 140.0, "");
        let (cx, cy) = (x0 + 80.0, 892.0);
        let frac = (self.day / self.day_max).clamp(0.0, 1.0);
        ring(cx, cy, 44.0, 60.0, 0.0, TAU, Color::new(0.45, 1.0, 1.0, 0.15));
        ring(cx, cy, 44.0, 60.0, 0.0, TAU * frac, CYAN);
        text_mid(&format!("{:.0}/{:.0}", self.day, self.day_max), cx, cy + 10.0, 30.0, WHITE);
        draw_text("CICLOS HOJE", x0 + 155.0, 870.0, 30.0, SOFT);
        draw_text("(limite diario)", x0 + 155.0, 905.0, 24.0, DIM);
        let hx = x0 + 400.0;
        draw_text("GUARDIA SINAPSE-9", hx, 862.0, 30.0, SOFT);
        let hk = (guard.hp / guard.max).clamp(0.0, 1.0);
        draw_rectangle(hx, 876.0, 510.0, 30.0, Color::new(0.45, 1.0, 1.0, 0.1));
        let hcol = if guard.flash > 0.0 { WHITE } else { Color::new(1.0 - hk, 0.3 + 0.7 * hk, 0.6 * hk + 0.2, 0.9) };
        draw_rectangle(hx, 876.0, 510.0 * hk, 30.0, hcol);
        draw_rectangle_lines(hx, 876.0, 510.0, 30.0, 2.0, DIM);
        let st = if guard.alive() { format!("{:.0}/{:.0} HP  -  CUPULA ATIVA", guard.hp, guard.max) } else { format!("REINICIANDO EM {:.0}s  -  CUPULA SEGUE LIGADA", guard.down.max(0.0)) };
        draw_text(&st, hx, 944.0, 28.0, if guard.alive() { WHITE } else { Color::new(1.0, 0.5, 0.4, 1.0) });
        draw_text("/lab   /pesquisa tema   /doarlab n   -   moedas ficticias", x0, 1004.0, 30.0, SOFT);

        // Últimos 3 achados (o novo desliza da direita)
        let (x1, w1) = (1020.0, TW - 1060.0);
        draw_text("ULTIMOS ACHADOS  -  a IA so resume o abstract publicado", x1, 306.0, 30.0, CYAN);
        for (i, f) in self.top.iter().enumerate() {
            let y = 322.0 + i as f32 * 212.0;
            let (dx, a) = if i == 0 { ((1.0 - ease(self.card_t)) * 320.0, ease(self.card_t)) } else { (0.0, 1.0) };
            let x = x1 + dx;
            let fade = |c: Color| Color::new(c.r, c.g, c.b, c.a * a);
            draw_rectangle(x, y, w1, 198.0, fade(Color::new(0.08, 0.3, 0.42, 0.35)));
            draw_rectangle(x, y, 8.0, 198.0, fade(PINK));
            if i == 0 && self.card_t < 1.0 {
                draw_rectangle_lines(x, y, w1, 198.0, 4.0, Color::new(1.0, 1.0, 1.0, 1.0 - self.card_t));
            }
            draw_text(&fit(&f.title, w1 - 40.0, 44.0), x + 24.0, y + 44.0, 44.0, fade(PINK));
            let body = if f.text.is_empty() { vec!["(sem resumo da IA agora: so titulo e revista)".to_string()] } else { wrap_px(&f.text, w1 - 40.0, 34.0, 3) };
            for (j, l) in body.iter().enumerate() {
                draw_text(l, x + 24.0, y + 86.0 + j as f32 * 35.0, 34.0, fade(WHITE));
            }
            draw_text(&fit(&f.src, w1 - 40.0, 26.0), x + 24.0, y + 190.0, 26.0, fade(SOFT));
        }
        if self.top.is_empty() {
            draw_text("primeiro ciclo de leitura em andamento...", x1, 400.0, 34.0, SOFT);
        }
        draw_text("DOE DIRETO PRA QUEM PESQUISA:  bbrfoundation.org/donate  |  idor.org", x1, 1004.0, 32.0, GOLD);

        holo_fx(time, self.flash);
    }

    /// Quad da tela no mundo (depth test do passe 3D: paredes e gente na frente tampam).
    pub(super) fn draw_screen(&self, time: f32, eye: Vec3) {
        let Some(rt) = &self.rt else { return };
        if eye.x > PANEL_X - 0.05 {
            return;
        }
        let glitch = (time * 13.0).sin() * (time * 7.3).sin() > 0.95;
        let a = (0.9 + 0.05 * (time * 31.0).sin()) * if glitch { 0.65 } else { 1.0 };
        let col = Color::new(1.0, 1.0, 1.0, a);
        let x = PANEL_X - 0.05;
        let (z0, z1, y0, y1) = (PANEL_Z - PW * 0.5, PANEL_Z + PW * 0.5, PY - PH * 0.5, PY + PH * 0.5);
        draw_mesh(&Mesh {
            vertices: vec![
                Vertex::new(x, y1, z0, 0.0, 1.0, col),
                Vertex::new(x, y1, z1, 1.0, 1.0, col),
                Vertex::new(x, y0, z1, 1.0, 0.0, col),
                Vertex::new(x, y0, z0, 0.0, 0.0, col),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(rt.texture.clone()),
        });
    }
}

pub fn draw_frame(b: &mut Batch, trans: &mut Batch, time: f32, flash: f32) {
    frame_at(b, trans, &Mat4::from_translation(vec3(PANEL_X, 0.0, PANEL_Z)), (PW, PH, PY), time, flash);
}

/// Estrutura física: pilares com faixas de luz, emissor em cima, barra de base, moldura e halo.
/// `m` põe a origem no pé do painel (tela virada pra -x local); tamanho = (largura, altura, centro y).
pub(crate) fn frame_at(b: &mut Batch, trans: &mut Batch, m: &Mat4, (pw, ph, py): (f32, f32, f32), time: f32, flash: f32) {
    let id = *m;
    let metal = rgb(0.2, 0.22, 0.26);
    let dark = rgb(0.1, 0.11, 0.13);
    let k = 0.7 + 0.3 * (time * 2.0).sin() + 0.5 * flash;
    let glow = Color::new(0.3 * k, 0.95 * k.min(1.0), 1.0, 1.0);
    let top = py + ph * 0.5;
    let bot = py - ph * 0.5;
    let g = G as f32;
    for side in [-1.0f32, 1.0] {
        let z = side * (pw * 0.5 + 0.7);
        let h = top + 1.2 - g;
        b.cube(&id, vec3(0.0, g + 0.3, z), vec3(2.2, 0.6, 2.2), dark);
        b.glow(&id, vec3(0.0, g + 0.62, z), vec3(1.6, 0.06, 1.6), glow);
        b.cube(&id, vec3(0.0, g + h * 0.5, z), vec3(1.0, h, 1.0), metal);
        b.glow(&id, vec3(-0.52, g + h * 0.5, z), vec3(0.04, h - 1.0, 0.25), glow);
        for i in 0..6 {
            let y = g + 1.0 + ((time * 0.6 + i as f32 / 6.0).fract()) * (h - 1.5);
            b.glow(&id, vec3(-0.53, y, z), vec3(0.04, 0.3, 0.4), WHITE);
        }
        b.cube(&id, vec3(0.0, g + h + 0.25, z), vec3(1.3, 0.5, 1.3), dark);
    }
    // Emissor em cima com lentes, barra de base flutuando acima da cabeça
    let span = pw + 2.4;
    b.cube(&id, vec3(0.0, top + 0.75, 0.0), vec3(1.4, 0.9, span), metal);
    b.glow(&id, vec3(-0.2, top + 0.27, 0.0), vec3(0.8, 0.06, span - 1.0), glow);
    for i in 0..9 {
        let z = -pw * 0.5 + 1.25 + i as f32 * (pw - 2.5) / 8.0;
        let p = 0.5 + 0.5 * (time * 3.0 + i as f32 * 0.7).sin();
        b.glow(&id, vec3(-0.72, top + 0.75, z), vec3(0.05, 0.4, 0.4), Color::new(0.5 + 0.5 * p, 1.0, 1.0, 1.0));
    }
    b.cube(&id, vec3(0.0, bot - 0.35, 0.0), vec3(0.7, 0.35, span), metal);
    b.glow(&id, vec3(-0.36, bot - 0.35, 0.0), vec3(0.04, 0.12, span - 1.0), glow);
    // Moldura de luz e halo atrás da tela
    let edge = Color::new(0.45, 1.0, 1.0, 0.55 + 0.45 * flash);
    for (c, s) in [
        (vec3(-0.04, top, 0.0), vec3(0.06, 0.12, pw + 0.12)),
        (vec3(-0.04, bot, 0.0), vec3(0.06, 0.12, pw + 0.12)),
        (vec3(-0.04, py, -pw * 0.5), vec3(0.06, ph, 0.12)),
        (vec3(-0.04, py, pw * 0.5), vec3(0.06, ph, 0.12)),
    ] {
        trans.glow(&id, c, s, edge);
    }
    trans.glow(&id, vec3(0.2, py, 0.0), vec3(0.05, ph + 1.2, pw + 1.2), Color::new(0.2, 0.8, 1.0, 0.1 + 0.1 * flash));
    trans.glow(&id, vec3(-0.5, top + 0.1, 0.0), vec3(0.6, 0.3, pw), Color::new(0.4, 1.0, 1.0, 0.18));
}
