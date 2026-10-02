//! Escudos de energia do lab e do Game Hub: nenhum bloco some nem aparece dentro deles (World::set
//! recusa) e tiro/laser para na superfície. Ficam ligados mesmo com a guardiã caída. O lote do ringue
//! também é protegido (só blocos, sem domo).

use crate::hub::{HX0, HX1, HZ0, HZ1};
use crate::world::*;
use macroquad::prelude::*;

/// Domo do hub: elipsoide (semi-eixos) sobre o corredor; as quinas do piso entram pela caixa.
pub const HUB_R: Vec3 = vec3(17.0, 11.0, 10.0);

pub fn hub_center() -> Vec3 {
    vec3((HX0 + HX1) as f32 * 0.5 + 0.5, G as f32, (HZ0 + HZ1) as f32 * 0.5 + 0.5)
}

/// (centro no chão, semi-eixos): lab, hub.
pub fn domes() -> [(Vec3, Vec3); 2] {
    [(crate::lab::dome_center(), Vec3::splat(crate::lab::DOME_R)), (hub_center(), HUB_R)]
}

/// Bloco dentro de um escudo (domo + cilindro até o fundo, ou piso do corredor do hub).
pub fn protected(p: IVec3) -> bool {
    if (HX0..=HX1).contains(&p.x) && (HZ0..=HZ1).contains(&p.z) || crate::places::ringue::in_lot(p.x, p.z) {
        return true;
    }
    let bc = p.as_vec3() + Vec3::splat(0.5);
    domes().iter().any(|(c, r)| {
        let q = (bc - *c) / *r;
        vec3(q.x, q.y.max(0.0), q.z).length_squared() < 1.0
    })
}

/// Primeira entrada do raio (d normalizado) num domo: (distância, normal, índice do domo).
pub fn ray(o: Vec3, d: Vec3) -> Option<(f32, Vec3, usize)> {
    let mut best: Option<(f32, Vec3, usize)> = None;
    for (i, (c, r)) in domes().iter().enumerate() {
        let (oo, dd) = ((o - *c) / *r, d / *r);
        let (a, b, cc) = (dd.dot(dd), oo.dot(dd), oo.dot(oo) - 1.0);
        let disc = b * b - a * cc;
        if cc <= 0.0 || disc < 0.0 {
            continue;
        }
        let t = (-b - disc.sqrt()) / a;
        if t > 0.0 && best.is_none_or(|h| t < h.0) {
            let p = o + d * t;
            best = Some((t, ((p - *c) / (*r * *r)).normalize_or_zero(), i));
        }
    }
    best
}

/// Domo do hub (modo imediato, depois do batch transparente, igual ao do lab): ciano com veios magenta.
pub fn draw_hub(time: f32, hit: f32) {
    let k = 0.03 * (time * 1.9).sin();
    let gl = unsafe { get_internal_gl() }.quad_gl;
    gl.push_model_matrix(Mat4::from_translation(hub_center()) * Mat4::from_scale(HUB_R));
    draw_sphere(Vec3::ZERO, 1.0, None, Color::new(0.35, 0.9, 1.0, 0.07 + k + 0.2 * hit));
    draw_sphere_wires(Vec3::ZERO, 1.004, None, Color::new(1.0, 0.35, 0.95, 0.12 + 0.3 * hit));
    let gl = unsafe { get_internal_gl() }.quad_gl;
    gl.pop_model_matrix();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::urna::{self, Fx, Plan, Shot};

    fn count(w: &World, lo: IVec3, hi: IVec3) -> usize {
        let mut n = 0;
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    n += (w.get(x, y, z) != AIR) as usize;
                }
            }
        }
        n
    }

    #[test]
    fn shields_hold() {
        let mut w = crate::mods::generate();
        let avg = vec![WHITE; 256];
        let mut fx = Fx::default();
        let (lab_lo, lab_hi) = (ivec3(LAB_X0, G - 3, LAB_Z0), ivec3(LAB_X1, G + 5, LAB_Z1));
        let (hub_lo, hub_hi) = (ivec3(HX0, G - 3, HZ0), ivec3(HX1, G + 5, HZ1));
        let (lab0, hub0) = (count(&w, lab_lo, lab_hi), count(&w, hub_lo, hub_hi));
        let (lab, hub) = (crate::layout::lab, crate::layout::hub);
        let (lz, hz) = (crate::layout::LAB_D.y, crate::layout::HUB_D.y);
        for c in [lab(vec3(103.0, G as f32, 64.0)), vec3(LAB_X0 as f32, G as f32 + 1.0, 60.0 + lz as f32), hub(vec3(106.0, G as f32, 88.0)), vec3(HX1 as f32, G as f32, 90.0 + hz as f32)] {
            urna::explode(&mut w, c, 5.5, &mut fx, &avg);
        }
        for x in LAB_X0..=LAB_X1 {
            w.set(x, G - 1, 60 + lz, AIR);
            w.set(x, G + 2, 70 + lz, crate::world::TNT);
        }
        w.set(100 + crate::layout::HUB_D.x, G - 1, 88 + hz, AIR);
        let o = lab(vec3(60.0, G as f32 + 30.0, 64.0));
        let s = urna::apply(&mut w, &Plan { o, hit: lab(vec3(103.0, G as f32, 64.0)), r: 5.5, deflect: false }, &mut fx, &avg);
        assert!(matches!(s, Shot::Deflected(p) if p.distance(crate::lab::dome_center()) > crate::lab::DOME_R - 0.5));
        let s = urna::apply(&mut w, &Plan { o, hit: hub(vec3(106.0, G as f32, 88.0)), r: 5.5, deflect: false }, &mut fx, &avg);
        assert!(matches!(s, Shot::Deflected(_)));
        assert_eq!(count(&w, lab_lo, lab_hi), lab0);
        assert_eq!(count(&w, hub_lo, hub_hi), hub0);
        let c = crate::layout::plaza_center().as_ivec3() + ivec3(10, 0, 0);
        assert!(!protected(c));
        urna::explode(&mut w, vec3(c.x as f32, G as f32 - 1.0, c.z as f32), 3.0, &mut fx, &avg);
        assert_eq!(w.get(c.x, G - 1, c.z), AIR);
    }
}
