//! Placar holográfico FÍSICO ao lado da arena (borda oeste, tela virada pra dentro): quem está brigando,
//! vida, nocautes e jogadores na arena. Mesmo visual do painel do lab; a textura só é redesenhada quando
//! o placar muda (com intervalo mínimo por qualidade).

use crate::actors::{FState, Fighter};
use crate::batch::Batch;
use crate::lab::panel::{CYAN, DIM, GOLD, SOFT, fit, frame, frame_at, holo_fx, mipmaps, text_right};
use crate::layout::in_arena;
use crate::npc::{Npcs, Vida};
use crate::world::G;
use macroquad::prelude::*;
use std::f32::consts::PI;

const TW: f32 = 2048.0;
const TH: f32 = 1024.0;
/// Pé do painel (fora da areia, entre a borda oeste da arena e a margem do footprint).
const AX: f32 = 109.0;
const AZ: f32 = 58.0;
const PW: f32 = 24.0;
const PH: f32 = 12.0;
const PY: f32 = G as f32 + 9.5;

struct Row {
    name: String,
    col: Color,
    hp: f32,
    max: f32,
    kos: Option<u32>,
    st: String,
}

pub struct ArenaPanel {
    rt: Option<RenderTarget>,
    key: String,
    last: f32,
}

fn vida_row(name: &str, col: Color, v: &Vida) -> Row {
    let st = if v.alive() { "LUTANDO".into() } else { format!("VOLTA EM {:.0}s", v.down.max(0.0)) };
    Row { name: name.into(), col, hp: v.hp, max: v.max, kos: None, st }
}

fn center() -> Vec3 {
    vec3(AX, PY, AZ)
}

impl ArenaPanel {
    pub fn new() -> Self {
        ArenaPanel { rt: None, key: String::new(), last: -100.0 }
    }

    /// Redesenha a tela na render target (antes do passe 3D). Só perto, do lado da arena, e se mudou.
    #[allow(clippy::too_many_arguments)]
    pub fn render<'a>(&mut self, time: f32, eye: Vec3, fighters: &[Fighter], npcs: &Npcs, mario: Option<Vec3>, bomba: Option<Vec3>, urna: Vec3, players: impl Iterator<Item = (&'a str, Vec3)>) {
        if eye.distance(center()) > 100.0 || eye.x < AX + 1.0 {
            return;
        }
        let (every, res) = crate::quality::pick([(1.0, 0.5), (0.25, 1.0), (0.1, 1.0)]);
        let fresh = self.rt.as_ref().is_some_and(|rt| rt.texture.width() == TW * res);
        if fresh && time - self.last < every && time >= self.last {
            return;
        }
        let mut rows: Vec<Row> = fighters
            .iter()
            .filter(|f| f.spawned)
            .map(|f| {
                let st = match f.state {
                    FState::Ko(_) => "NOCAUTE",
                    FState::Entering => "ENTRANDO",
                    _ => "LUTANDO",
                };
                Row { name: f.name.into(), col: f.flag.0, hp: f.hp.max(0.0), max: f.max_hp, kos: Some(f.kos), st: st.into() }
            })
            .collect();
        if let (Some(p), Some(v)) = (mario, npcs.get(crate::npc::MARIO, 0)) {
            if in_arena(p.xz(), 0.0) {
                rows.push(vida_row("MARIO", Color::new(1.0, 0.25, 0.2, 1.0), v));
            }
        }
        if let (Some(p), Some(v)) = (bomba, npcs.get(crate::npc::BOMBA, 0)) {
            if in_arena(p.xz(), 0.0) {
                rows.push(vida_row("BOMBADINHO", Color::new(1.0, 0.55, 0.15, 1.0), v));
            }
        }
        rows.push(vida_row("GODZILHA", Color::new(0.45, 0.85, 1.0, 1.0), npcs.kaiju()));
        if in_arena(urna.xz(), 0.0) {
            rows.push(vida_row("URNA GIGANTE", Color::new(0.9, 0.9, 0.95, 1.0), npcs.urna()));
        }
        rows.sort_by(|a, b| b.kos.unwrap_or(0).cmp(&a.kos.unwrap_or(0)).then((b.hp / b.max).total_cmp(&(a.hp / a.max))));
        let mut names: Vec<&str> = players.filter(|(_, p)| in_arena(p.xz(), 0.0)).map(|(n, _)| n).collect();
        names.sort_unstable();
        let mut key = names.join(",");
        for r in &rows {
            key += &format!("|{}:{:.0}:{:?}:{}", r.name, r.hp, r.kos, r.st);
        }
        if fresh && key == self.key {
            return;
        }
        self.last = time;
        if !fresh {
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
        paint(time, &rows, &names);
        set_default_camera();
        mipmaps(&rt);
        self.key = key;
    }

    /// Quad da tela no mundo (depth test do passe 3D), virado pra +x (pra dentro da arena).
    pub fn draw_screen(&self, time: f32, eye: Vec3) {
        let Some(rt) = &self.rt else { return };
        if eye.x < AX + 0.05 || eye.distance(center()) > 140.0 {
            return;
        }
        let glitch = (time * 11.0).sin() * (time * 6.1).sin() > 0.95;
        let a = (0.9 + 0.05 * (time * 29.0).sin()) * if glitch { 0.65 } else { 1.0 };
        let col = Color::new(1.0, 1.0, 1.0, a);
        let x = AX + 0.05;
        let (z0, z1, y0, y1) = (AZ - PW * 0.5, AZ + PW * 0.5, PY - PH * 0.5, PY + PH * 0.5);
        draw_mesh(&Mesh {
            vertices: vec![
                Vertex::new(x, y1, z1, 0.0, 1.0, col),
                Vertex::new(x, y1, z0, 1.0, 1.0, col),
                Vertex::new(x, y0, z0, 1.0, 0.0, col),
                Vertex::new(x, y0, z1, 0.0, 0.0, col),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(rt.texture.clone()),
        });
    }
}

fn paint(time: f32, rows: &[Row], names: &[&str]) {
    draw_text("ARENA DOS GIGANTES  -  PLACAR AO VIVO", 40.0, 84.0, 76.0, CYAN);
    text_right(&format!("{} NA ARENA", rows.len() + names.len()), TW - 40.0, 84.0, 52.0, GOLD);
    draw_rectangle(40.0, 120.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

    frame(40.0, 140.0, TW - 80.0, 680.0, "");
    let (xr, xn, xh, xk, xs) = (70.0, 160.0, 820.0, 1500.0, TW - 70.0);
    for (t, x) in [("#", xr), ("LUTADOR", xn), ("VIDA", xh), ("NOCAUTES", xk - 90.0)] {
        draw_text(t, x, 184.0, 30.0, SOFT);
    }
    text_right("STATUS", xs, 184.0, 30.0, SOFT);
    let rh = (620.0 / rows.len().max(6) as f32).min(100.0);
    for (i, r) in rows.iter().enumerate() {
        let y = 200.0 + i as f32 * rh;
        let mid = y + rh * 0.5;
        let lead = i == 0 && r.kos.unwrap_or(0) > 0;
        draw_rectangle(56.0, y + 6.0, TW - 112.0, rh - 12.0, Color::new(0.08, 0.3, 0.42, if lead { 0.45 } else { 0.25 }));
        draw_rectangle(56.0, y + 6.0, 10.0, rh - 12.0, r.col);
        draw_text(&format!("{}", i + 1), xr + 10.0, mid + 20.0, 60.0, if lead { GOLD } else { WHITE });
        draw_text(&fit(&r.name, xh - xn - 40.0, 56.0), xn, mid + 20.0, 56.0, if lead { GOLD } else { WHITE });
        let hk = (r.hp / r.max).clamp(0.0, 1.0);
        let bw = 560.0;
        draw_rectangle(xh, mid - 22.0, bw, 44.0, Color::new(0.45, 1.0, 1.0, 0.1));
        draw_rectangle(xh, mid - 22.0, bw * hk, 44.0, Color::new(1.0 - hk, 0.3 + 0.7 * hk, 0.6 * hk + 0.2, 0.9));
        draw_rectangle_lines(xh, mid - 22.0, bw, 44.0, 2.0, DIM);
        draw_text(&format!("{:.0}/{:.0}", r.hp, r.max), xh + 16.0, mid + 13.0, 36.0, WHITE);
        let k = r.kos.map_or("-".to_string(), |k| k.to_string());
        text_right(&k, xk + 40.0, mid + 24.0, 72.0, if lead { GOLD } else { WHITE });
        let stc = if r.st == "LUTANDO" { CYAN } else { Color::new(1.0, 0.5, 0.4, 1.0) };
        text_right(&r.st, xs, mid + 14.0, 40.0, stc);
    }

    frame(40.0, 840.0, TW - 80.0, 140.0, &format!("JOGADORES NA ARENA ({})", names.len()));
    let who = if names.is_empty() { "ninguem ainda - entre pela rua norte da praca".to_string() } else { names.join("   -   ") };
    draw_text(&fit(&who, TW - 140.0, 44.0), 70.0, 950.0, 44.0, if names.is_empty() { SOFT } else { WHITE });
    holo_fx(time, 0.0);
}

/// Pilares, emissor e moldura (mesma estrutura do painel do lab, girada pra olhar pra arena).
pub fn draw_frame(b: &mut Batch, trans: &mut Batch, time: f32, eye: Vec3) {
    if eye.distance(center()) > 160.0 {
        return;
    }
    let m = Mat4::from_translation(vec3(AX, 0.0, AZ)) * Mat4::from_rotation_y(PI);
    frame_at(b, trans, &m, (PW, PH, PY), time, 0.0);
}
