//! Modelos blocados: humanoide (estilo Steve), villager, bandeira.

use crate::batch::Batch;
use macroquad::prelude::*;
use std::f32::consts::PI;

/// 1 "pixel" de skin em unidades de mundo (32px de altura ≈ 1.8 bloco).
pub const U: f32 = 0.056;

#[derive(Clone, Copy)]
pub struct Look {
    pub skin: Color,
    pub hair: Color,
    pub shirt: Color,
    pub pants: Color,
    pub shoes: Color,
    pub beard: Option<Color>,
    pub glasses: bool,
    pub wolverine: bool,
    pub toga: bool,
}

#[derive(Clone, Copy, Default)]
pub struct Pose {
    pub walk: f32,
    pub walk_amt: f32,
    /// Ângulo do braço (0 = abaixado, -PI/2 = pra frente, -PI = pra cima).
    pub arm_l: f32,
    pub arm_r: f32,
    /// Abertura lateral do braço.
    pub arm_l_out: f32,
    pub arm_r_out: f32,
    /// Queda pra trás (PI/2 = deitado).
    pub lean: f32,
    pub bounce: f32,
    pub nod: f32,
    pub flash: f32,
    pub berserk: bool,
}

pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

fn tint(c: Color, p: &Pose) -> Color {
    let mut c = c;
    if p.berserk {
        c = Color::new((c.r * 0.8 + 0.3).min(1.0), c.g * 0.55, c.b * 0.55, c.a);
    }
    if p.flash > 0.0 {
        let f = p.flash * 0.75;
        c = Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f * 0.6, c.b + (1.0 - c.b) * f * 0.6, c.a);
    }
    c
}

pub fn root(pos: Vec3, yaw: f32, lean: f32, bounce: f32) -> Mat4 {
    Mat4::from_translation(pos + vec3(0.0, bounce, 0.0)) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-lean)
}

fn t(x: f32, y: f32, z: f32) -> Mat4 {
    Mat4::from_translation(vec3(x, y, z))
}

pub fn draw_humanoid(b: &mut Batch, look: &Look, p: &Pose, root: &Mat4) {
    let c = |col: Color| tint(col, p);
    let blue = rgb(0.12, 0.25, 0.75);
    let dark = rgb(0.08, 0.06, 0.06);

    // Pernas
    for (side, sw) in [(1.0, p.walk.sin()), (-1.0, -p.walk.sin())] {
        let m = *root * t(side * 2.0 * U, 12.0 * U, 0.0) * Mat4::from_rotation_x(sw * 0.8 * p.walk_amt);
        b.cube(&m, vec3(0.0, -4.5 * U, 0.0), vec3(4.0 * U, 9.0 * U, 4.0 * U), c(look.pants));
        b.cube(&m, vec3(0.0, -10.5 * U, 0.0), vec3(4.1 * U, 3.0 * U, 4.1 * U), c(look.shoes));
    }

    // Tronco
    b.cube(root, vec3(0.0, 18.0 * U, 0.0), vec3(8.0 * U, 12.0 * U, 4.0 * U), c(look.shirt));
    if look.toga {
        b.cube(root, vec3(0.0, 7.5 * U, 0.0), vec3(8.8 * U, 9.0 * U, 4.8 * U), c(look.shirt));
        b.cube(root, vec3(0.0, 23.2 * U, 2.1 * U), vec3(3.0 * U, 1.6 * U, 0.3 * U), WHITE);
    }
    if look.wolverine {
        for side in [1.0f32, -1.0] {
            b.cube(root, vec3(side * 3.6 * U, 19.0 * U, 0.0), vec3(1.0 * U, 9.0 * U, 4.2 * U), c(blue));
        }
        b.cube(root, vec3(0.0, 23.0 * U, 0.0), vec3(8.2 * U, 2.0 * U, 4.2 * U), c(blue));
        b.cube(root, vec3(0.0, 12.8 * U, 0.0), vec3(8.2 * U, 1.6 * U, 4.2 * U), c(rgb(0.55, 0.12, 0.08)));
        b.cube(root, vec3(0.0, 12.8 * U, 2.15 * U), vec3(2.0 * U, 1.6 * U, 0.3 * U), c(rgb(0.9, 0.85, 0.2)));
    }

    // Braços (+X = esquerda do personagem)
    for (side, ang, out) in [(1.0f32, p.arm_l, p.arm_l_out), (-1.0, p.arm_r, p.arm_r_out)] {
        let m = *root * t(side * 6.0 * U, 22.0 * U, 0.0) * Mat4::from_rotation_x(ang) * Mat4::from_rotation_z(side * out);
        if look.wolverine {
            b.cube(&m, vec3(0.0, -1.0 * U, 0.0), vec3(4.1 * U, 6.0 * U, 4.1 * U), c(blue));
            b.cube(&m, vec3(0.0, -6.0 * U, 0.0), vec3(4.0 * U, 4.0 * U, 4.0 * U), c(look.shirt));
            b.cube(&m, vec3(0.0, -9.0 * U, 0.0), vec3(4.15 * U, 2.0 * U, 4.15 * U), c(blue));
            let steel = rgb(0.82, 0.86, 0.92);
            for k in [-1.2f32, 0.0, 1.2] {
                b.cube(&m, vec3(k * U, -13.5 * U, 0.6 * U), vec3(0.35 * U, 7.0 * U, 0.35 * U), steel);
            }
        } else {
            b.cube(&m, vec3(0.0, -0.5 * U, 0.0), vec3(4.15 * U, 5.0 * U, 4.15 * U), c(look.shirt));
            b.cube(&m, vec3(0.0, -6.5 * U, 0.0), vec3(4.0 * U, 7.0 * U, 4.0 * U), c(look.skin));
        }
    }

    // Cabeça
    let m = *root * t(0.0, 24.0 * U, 0.0) * Mat4::from_rotation_x(p.nod);
    let head_col = if look.wolverine { look.shirt } else { look.skin };
    b.cube(&m, vec3(0.0, 4.0 * U, 0.0), vec3(8.0 * U, 8.0 * U, 8.0 * U), c(head_col));
    let fz = 4.0 * U;
    if look.wolverine {
        b.cube(&m, vec3(0.0, 1.6 * U, 0.0), vec3(8.1 * U, 3.2 * U, 8.1 * U), c(look.skin));
        b.cube(&m, vec3(0.0, 4.4 * U, fz + 0.05 * U), vec3(7.0 * U, 2.4 * U, 0.2 * U), dark);
        for side in [1.0f32, -1.0] {
            b.glow(&m, vec3(side * 1.9 * U, 4.3 * U, fz + 0.2 * U), vec3(2.2 * U, 0.8 * U, 0.2 * U), WHITE);
            let fin = m * t(side * 3.4 * U, 6.5 * U, 0.5 * U) * Mat4::from_rotation_z(-side * 0.45);
            b.cube(&fin, vec3(0.0, 2.5 * U, 0.0), vec3(1.0 * U, 5.0 * U, 3.0 * U), dark);
            b.cube(&m, vec3(side * 4.1 * U, 2.0 * U, 1.0 * U), vec3(0.4 * U, 3.5 * U, 3.0 * U), c(look.hair));
        }
        b.cube(&m, vec3(0.0, 1.2 * U, fz + 0.1 * U), vec3(3.0 * U, 0.6 * U, 0.2 * U), rgb(0.35, 0.1, 0.1));
        return;
    }

    // Cabelo
    b.cube(&m, vec3(0.0, 7.6 * U, 0.0), vec3(8.4 * U, 1.4 * U, 8.4 * U), c(look.hair));
    b.cube(&m, vec3(0.0, 5.0 * U, -3.9 * U), vec3(8.4 * U, 6.0 * U, 0.8 * U), c(look.hair));
    for side in [1.0f32, -1.0] {
        b.cube(&m, vec3(side * 4.05 * U, 6.0 * U, -1.0 * U), vec3(0.5 * U, 3.0 * U, 6.0 * U), c(look.hair));
    }
    // Barba
    if let Some(bc) = look.beard {
        b.cube(&m, vec3(0.0, 1.5 * U, fz + 0.2 * U), vec3(8.2 * U, 3.0 * U, 0.4 * U), c(bc));
        for side in [1.0f32, -1.0] {
            b.cube(&m, vec3(side * 4.1 * U, 2.2 * U, 1.5 * U), vec3(0.4 * U, 4.0 * U, 5.0 * U), c(bc));
        }
    }
    // Olhos / óculos / boca
    for side in [1.0f32, -1.0] {
        if look.glasses {
            b.cube(&m, vec3(side * 2.0 * U, 4.0 * U, fz + 0.1 * U), vec3(2.8 * U, 1.8 * U, 0.1 * U), dark);
        }
        b.glow(&m, vec3(side * 2.0 * U, 4.0 * U, fz + 0.2 * U), vec3(2.0 * U, 1.0 * U, 0.15 * U), WHITE);
        b.glow(&m, vec3(side * 1.5 * U, 4.0 * U, fz + 0.3 * U), vec3(1.0 * U, 1.0 * U, 0.15 * U), rgb(0.2, 0.15, 0.4));
    }
    if look.glasses {
        b.cube(&m, vec3(0.0, 4.3 * U, fz + 0.1 * U), vec3(1.4 * U, 0.4 * U, 0.1 * U), dark);
    }
    b.cube(&m, vec3(0.0, 1.8 * U, fz + 0.45 * U), vec3(3.0 * U, 0.6 * U, 0.2 * U), rgb(0.45, 0.15, 0.15));
}

/// Banqueiro da balada: terno aberto sem gravata, cordão e relógio de ouro, óculos escuros, gel no cabelo.
pub fn draw_banker_extras(b: &mut Batch, look: &Look, p: &Pose, root: &Mat4) {
    let c = |col: Color| tint(col, p);
    let gold = rgb(1.0, 0.8, 0.2);
    let lapel = rgb(0.03, 0.05, 0.14);
    b.cube(root, vec3(0.0, 20.5 * U, 2.05 * U), vec3(2.6 * U, 5.0 * U, 0.15 * U), c(WHITE));
    b.cube(root, vec3(0.0, 22.6 * U, 2.12 * U), vec3(1.4 * U, 1.6 * U, 0.15 * U), c(look.skin));
    for side in [1.0f32, -1.0] {
        b.cube(root, vec3(side * 1.75 * U, 19.5 * U, 2.1 * U), vec3(0.9 * U, 7.0 * U, 0.15 * U), c(lapel));
    }
    b.glow(root, vec3(0.0, 21.9 * U, 2.2 * U), vec3(2.2 * U, 0.45 * U, 0.1 * U), gold);
    b.glow(root, vec3(0.0, 21.1 * U, 2.22 * U), vec3(0.9 * U, 0.9 * U, 0.1 * U), gold);
    let arm = *root * t(6.0 * U, 22.0 * U, 0.0) * Mat4::from_rotation_x(p.arm_l) * Mat4::from_rotation_z(p.arm_l_out);
    b.glow(&arm, vec3(0.0, -8.6 * U, 0.0), vec3(4.35 * U, 1.1 * U, 4.35 * U), gold);
    let m = *root * t(0.0, 24.0 * U, 0.0) * Mat4::from_rotation_x(p.nod);
    let fz = 4.0 * U;
    b.cube(&m, vec3(0.0, 4.0 * U, fz + 0.4 * U), vec3(7.4 * U, 2.2 * U, 0.2 * U), rgb(0.02, 0.02, 0.03));
    b.glow(&m, vec3(2.6 * U, 4.6 * U, fz + 0.52 * U), vec3(0.8 * U, 0.4 * U, 0.05 * U), gold);
    b.cube(&m, vec3(0.0, 8.38 * U, 0.6 * U), vec3(6.0 * U, 0.2 * U, 6.6 * U), c(rgb(0.2, 0.2, 0.25)));
}

#[derive(Clone, Copy)]
pub struct VLook {
    pub robe: Color,
    pub skin: Color,
    pub hat: Option<Color>,
}

/// Villager clássico: cabeção, narigão, monocelha. `arms_up` > 0.5 = dançando de braço pra cima.
#[allow(clippy::too_many_arguments)]
pub fn draw_villager(b: &mut Batch, vl: &VLook, root: &Mat4, arms_up: f32, arm_phase: f32, nod: f32, walk: f32, walk_amt: f32) {
    let legs = Color::new(vl.robe.r * 0.6, vl.robe.g * 0.6, vl.robe.b * 0.6, 1.0);
    for (side, sw) in [(1.0f32, walk.sin()), (-1.0, -walk.sin())] {
        let m = *root * t(side * 2.0 * U, 6.0 * U, 0.0) * Mat4::from_rotation_x(sw * 0.6 * walk_amt);
        b.cube(&m, vec3(0.0, -3.0 * U, 0.0), vec3(4.0 * U, 6.0 * U, 4.0 * U), legs);
    }
    b.cube(root, vec3(0.0, 9.0 * U, 0.0), vec3(8.4 * U, 6.0 * U, 6.4 * U), vl.robe);
    b.cube(root, vec3(0.0, 18.0 * U, 0.0), vec3(8.0 * U, 12.0 * U, 6.0 * U), vl.robe);
    b.cube(root, vec3(0.0, 6.5 * U, 0.0), vec3(8.6 * U, 1.0 * U, 6.6 * U), legs);

    if arms_up > 0.5 {
        for side in [1.0f32, -1.0] {
            let ang = -2.7 + (arm_phase + if side > 0.0 { 0.0 } else { PI }).sin() * 0.35;
            let m = *root * t(side * 5.5 * U, 22.0 * U, 0.0) * Mat4::from_rotation_x(ang) * Mat4::from_rotation_z(side * 0.3);
            b.cube(&m, vec3(0.0, -4.0 * U, 0.0), vec3(3.5 * U, 8.0 * U, 3.5 * U), vl.robe);
            b.cube(&m, vec3(0.0, -9.0 * U, 0.0), vec3(3.2 * U, 2.5 * U, 3.2 * U), vl.skin);
        }
    } else {
        b.cube(root, vec3(0.0, 19.0 * U, 4.0 * U), vec3(8.0 * U, 4.0 * U, 4.0 * U), vl.robe);
        for side in [1.0f32, -1.0] {
            b.cube(root, vec3(side * 2.0 * U, 19.0 * U, 6.1 * U), vec3(3.0 * U, 3.0 * U, 0.4 * U), vl.skin);
        }
    }

    let m = *root * t(0.0, 24.0 * U, 0.0) * Mat4::from_rotation_x(nod);
    b.cube(&m, vec3(0.0, 5.0 * U, 0.0), vec3(8.0 * U, 10.0 * U, 8.0 * U), vl.skin);
    let nose = Color::new(vl.skin.r * 0.9, vl.skin.g * 0.82, vl.skin.b * 0.78, 1.0);
    b.cube(&m, vec3(0.0, 3.0 * U, 5.0 * U), vec3(2.0 * U, 4.0 * U, 2.0 * U), nose);
    b.cube(&m, vec3(0.0, 6.3 * U, 4.05 * U), vec3(6.0 * U, 1.0 * U, 0.2 * U), rgb(0.25, 0.15, 0.08));
    for side in [1.0f32, -1.0] {
        b.glow(&m, vec3(side * 2.0 * U, 5.0 * U, 4.05 * U), vec3(2.0 * U, 1.0 * U, 0.2 * U), WHITE);
        b.glow(&m, vec3(side * 1.5 * U, 5.0 * U, 4.12 * U), vec3(1.0 * U, 1.0 * U, 0.2 * U), rgb(0.1, 0.55, 0.2));
    }
    if let Some(hc) = vl.hat {
        b.cube(&m, vec3(0.0, 10.5 * U, 0.0), vec3(11.0 * U, 1.0 * U, 11.0 * U), hc);
        b.cube(&m, vec3(0.0, 11.8 * U, 0.0), vec3(8.4 * U, 2.0 * U, 8.4 * U), hc);
    }
}

/// Bandeira com mastro. `dir` define pra que lado (X local) o pano estende.
/// Retorna a posição em mundo do topo da bandeira (pra label).
#[allow(clippy::too_many_arguments)]
pub fn draw_flag(b: &mut Batch, root: &Mat4, base: Vec3, height: f32, cloth: Color, stripe: Option<Color>, time: f32, sway: f32, dir: f32) -> Vec3 {
    let m = *root * Mat4::from_translation(base) * Mat4::from_rotation_z(sway);
    b.cube(&m, vec3(0.0, height * 0.5, 0.0), vec3(0.06, height, 0.06), rgb(0.45, 0.32, 0.18));
    let segs = 6;
    let w = 0.95 / segs as f32;
    for k in 0..segs {
        let kf = k as f32;
        let x = dir * (0.03 + w * (kf + 0.5));
        let wave = (time * 7.0 - kf * 0.9).sin() * 0.05 * (kf + 1.0) / segs as f32 * 2.0;
        b.cube(&m, vec3(x, height - 0.3, wave), vec3(w + 0.01, 0.58, 0.03), cloth);
        if let Some(sc) = stripe {
            b.cube(&m, vec3(x, height - 0.3, wave), vec3(w + 0.012, 0.18, 0.04), sc);
        }
    }
    m.transform_point3(vec3(dir * 0.48, height + 0.25, 0.0))
}
