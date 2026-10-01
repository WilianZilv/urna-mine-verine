//! Batch de cubos transformados na CPU (personagens, urna, partículas, efeitos).

use crate::atlas::{self, T_WHITE};
use macroquad::prelude::*;

const FACES: [([usize; 4], [f32; 3]); 6] = [
    ([1, 3, 7, 5], [1.0, 0.0, 0.0]),
    ([0, 4, 6, 2], [-1.0, 0.0, 0.0]),
    ([2, 6, 7, 3], [0.0, 1.0, 0.0]),
    ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
    ([4, 5, 7, 6], [0.0, 0.0, 1.0]),
    ([0, 2, 3, 1], [0.0, 0.0, -1.0]),
];

pub struct Batch {
    v: Vec<Vertex>,
    i: Vec<u16>,
    done: Vec<(Vec<Vertex>, Vec<u16>)>,
    uv: Vec2,
    light: Vec3,
}

impl Batch {
    pub fn new() -> Self {
        let (u0, v0, u1, v1) = atlas::uv(T_WHITE);
        Self {
            v: Vec::new(),
            i: Vec::new(),
            done: Vec::new(),
            uv: vec2((u0 + u1) * 0.5, (v0 + v1) * 0.5),
            light: vec3(0.4, 1.0, 0.3).normalize(),
        }
    }

    /// Cubo com iluminação direcional.
    pub fn cube(&mut self, m: &Mat4, c: Vec3, s: Vec3, col: Color) {
        self.cube_ex(m, c, s, col, true);
    }

    /// Cubo emissivo (sem sombreamento).
    pub fn glow(&mut self, m: &Mat4, c: Vec3, s: Vec3, col: Color) {
        self.cube_ex(m, c, s, col, false);
    }

    pub fn cube_ex(&mut self, m: &Mat4, c: Vec3, s: Vec3, col: Color, lit: bool) {
        if self.v.len() > 15000 {
            self.done.push((std::mem::take(&mut self.v), std::mem::take(&mut self.i)));
        }
        let h = s * 0.5;
        let mut p = [Vec3::ZERO; 8];
        for (k, pk) in p.iter_mut().enumerate() {
            let o = vec3(
                if k & 1 != 0 { h.x } else { -h.x },
                if k & 2 != 0 { h.y } else { -h.y },
                if k & 4 != 0 { h.z } else { -h.z },
            );
            *pk = m.transform_point3(c + o);
        }
        for (q, n) in FACES.iter() {
            let shade = if lit {
                let nw = m.transform_vector3(Vec3::from_array(*n)).normalize_or_zero();
                0.5 + 0.45 * nw.dot(self.light).max(0.0) + 0.12 * nw.y.max(0.0)
            } else {
                1.0
            };
            let cc = Color::new(col.r * shade, col.g * shade, col.b * shade, col.a);
            let base = self.v.len() as u16;
            for &k in q {
                self.v.push(Vertex::new2(p[k], self.uv, cc));
            }
            self.i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub fn flush(&mut self, tex: &Texture2D) {
        if !self.v.is_empty() {
            self.done.push((std::mem::take(&mut self.v), std::mem::take(&mut self.i)));
        }
        for (v, i) in self.done.drain(..) {
            draw_mesh(&Mesh { vertices: v, indices: i, texture: Some(tex.clone()) });
        }
    }
}
