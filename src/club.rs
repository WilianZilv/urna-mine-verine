//! Clube de house: pista LED sincronizada no beat, caixas pulsando, toca-discos, globo e canhões de luz.

use crate::batch::Batch;
use crate::layout::CLUB_D;
use crate::models::rgb;
use crate::world::*;
use macroquad::prelude::*;

pub fn hsv(h: f32, s: f32, v: f32) -> Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, 1.0)
}

pub fn draw(b: &mut Batch, tb: &mut Batch, time: f32, beat: f32) {
    let g = G as f32;
    let bi = beat.floor() as i32;
    let bf = beat.fract();
    let kick = (-bf * 6.0).exp();
    let id = Mat4::IDENTITY;
    let o = vec3(CLUB_D.x as f32, 0.0, CLUB_D.y as f32);
    let at = Mat4::from_translation(o);

    // Pista LED
    for z in FLOOR_Z0..FLOOR_Z1 {
        for x in FLOOR_X0..FLOOR_X1 {
            let on = ((x / 2 + z / 2 + bi) % 3) == 0;
            let hue = (x + z) as f32 * 0.04 + bi as f32 * 0.13;
            let v = if on { 0.6 + 0.4 * kick } else { 0.12 };
            b.glow(&id, vec3(x as f32 + 0.5, g + 0.01, z as f32 + 0.5), vec3(0.94, 0.04, 0.94), hsv(hue, 0.85, v));
        }
    }

    // Cones das caixas de som pulsando
    for &z in &[51.5f32, 77.5] {
        for &(y, s) in &[(g + 1.2, 1.4f32), (g + 3.4, 0.9)] {
            let k = s * (1.0 + 0.12 * kick);
            b.cube(&at, vec3(12.05, y, z), vec3(0.1, k, k), rgb(0.25, 0.25, 0.28));
            b.cube(&at, vec3(12.1 + 0.08 * kick, y, z), vec3(0.1, k * 0.45, k * 0.45), rgb(0.08, 0.08, 0.1));
        }
    }

    // Toca-discos + mixer na cabine
    let top = g + 2.0;
    for &z in &[61.8f32, 66.2] {
        b.cube(&at, vec3(13.9, top + 0.05, z), vec3(1.3, 0.1, 1.3), rgb(0.15, 0.15, 0.17));
        b.cube(&at, vec3(13.9, top + 0.12, z), vec3(1.0, 0.04, 1.0), rgb(0.05, 0.05, 0.05));
        let a = time * 3.5;
        b.glow(&at, vec3(13.9 + a.cos() * 0.35, top + 0.16, z + a.sin() * 0.35), vec3(0.12, 0.03, 0.12), WHITE);
    }
    b.cube(&at, vec3(13.9, top + 0.08, 64.0), vec3(0.8, 0.16, 1.4), rgb(0.2, 0.2, 0.22));
    for k in 0..4 {
        b.glow(&at, vec3(13.9, top + 0.18, 63.5 + k as f32 * 0.33), vec3(0.12, 0.05, 0.12), hsv(k as f32 * 0.25 + time * 0.2, 0.9, 1.0));
    }

    // Globo espelhado
    let ball = vec3(24.0, g + 8.5, 64.5) + o;
    let rot = Mat4::from_translation(ball) * Mat4::from_rotation_y(time * 1.2);
    b.cube(&rot, Vec3::ZERO, Vec3::splat(1.3), rgb(0.75, 0.78, 0.82));
    for k in 0..12 {
        let a = k as f32 * 0.523;
        let y = ((k * 7) % 5) as f32 * 0.25 - 0.5;
        let on = (k + bi) % 3 == 0;
        b.glow(&rot, vec3(a.cos() * 0.66, y, a.sin() * 0.66), Vec3::splat(0.18), if on { WHITE } else { rgb(0.6, 0.65, 0.7) });
    }

    // Canhões de luz (transparentes)
    let beam = |tb: &mut Batch, from: Vec3, dir: Vec3, len: f32, w: f32, col: Color| {
        let q = Quat::from_rotation_arc(Vec3::Y, dir.normalize());
        let m = Mat4::from_rotation_translation(q, from + dir.normalize() * len * 0.5);
        tb.glow(&m, Vec3::ZERO, vec3(w, len, w), col);
    };
    for k in 0..5 {
        let kf = k as f32;
        let from = vec3(8.6, g + 6.6, 52.0 + kf * 6.0) + o;
        let a = time * 0.9 + kf * 1.3;
        let dir = vec3(1.0, -0.35 + 0.3 * (a * 0.7).sin(), a.sin() * 0.8);
        let mut col = hsv(kf * 0.2 + time * 0.1, 0.8, 1.0);
        col.a = 0.18 + 0.12 * kick;
        beam(tb, from, dir, 26.0, 0.3, col);
    }
    for k in 0..4 {
        let kf = k as f32;
        let from = vec3(36.0, g + 7.0, 50.0 + kf * 9.5) + o;
        let a = time * 0.6 + kf * 1.7;
        let dir = vec3(a.cos() * 0.5, 1.0, a.sin() * 0.5);
        let mut col = hsv(0.55 + kf * 0.12, 0.7, 1.0);
        col.a = 0.16;
        beam(tb, from, dir, 60.0, 0.6, col);
    }
}
