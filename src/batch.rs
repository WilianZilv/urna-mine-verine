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

/// Descarta cubos fora da câmera, além de `max_dist` ou menores que `min_ratio` (raio / distância, ~fração de pixel).
#[derive(Clone, Copy)]
pub struct Cull {
    pub planes: [Vec4; 6],
    pub eye: Vec3,
    pub min_ratio: f32,
    pub max_dist: f32,
}

pub struct Batch {
    v: Vec<Vertex>,
    i: Vec<u16>,
    done: Vec<(Vec<Vertex>, Vec<u16>)>,
    pool: Vec<(Vec<Vertex>, Vec<u16>)>,
    uv: Vec2,
    light: Vec3,
    pub cull: Option<Cull>,
}

impl Batch {
    pub fn new() -> Self {
        let (u0, v0, u1, v1) = atlas::uv(T_WHITE);
        Self {
            v: Vec::new(),
            i: Vec::new(),
            done: Vec::new(),
            pool: Vec::new(),
            uv: vec2((u0 + u1) * 0.5, (v0 + v1) * 0.5),
            light: vec3(0.4, 1.0, 0.3).normalize(),
            cull: None,
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
        let h = s * 0.5;
        let wc = m.transform_point3(c);
        let axes = [m.x_axis.truncate(), m.y_axis.truncate(), m.z_axis.truncate()];
        if let Some(cull) = &self.cull {
            let r = h.length() * axes.iter().map(|a| a.length_squared()).fold(0.0, f32::max).sqrt();
            let d = wc.distance(cull.eye);
            if r < d * cull.min_ratio || d - r > cull.max_dist || !crate::chunks::sphere_visible(&cull.planes, wc, r) {
                return;
            }
        }
        crate::prof::add(&crate::prof::CUBES, 1);
        if self.v.len() > 15000 {
            let (v, i) = self.pool.pop().unwrap_or_default();
            self.done.push((std::mem::replace(&mut self.v, v), std::mem::replace(&mut self.i, i)));
        }
        let (ex, ey, ez) = (axes[0] * h.x, axes[1] * h.y, axes[2] * h.z);
        let mut p = [Vec3::ZERO; 8];
        for (k, pk) in p.iter_mut().enumerate() {
            let sx = if k & 1 != 0 { ex } else { -ex };
            let sy = if k & 2 != 0 { ey } else { -ey };
            let sz = if k & 4 != 0 { ez } else { -ez };
            *pk = wc + sx + sy + sz;
        }
        let units = axes.map(|a| a.normalize_or_zero());
        for (f, (q, _)) in FACES.iter().enumerate() {
            let shade = if lit {
                let nw = if f % 2 == 0 { units[f / 2] } else { -units[f / 2] };
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

    /// Desenha de novo sem esvaziar (vista dos portais reaproveita a cena).
    pub fn redraw(&self, tex: &Texture2D) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        gl.texture(Some(tex));
        gl.draw_mode(DrawMode::Triangles);
        for (v, i) in self.done.iter().map(|(v, i)| (v.as_slice(), i.as_slice())).chain(std::iter::once((self.v.as_slice(), self.i.as_slice()))) {
            if !i.is_empty() {
                crate::prof::add(&crate::prof::CALLS, 1);
                crate::prof::add(&crate::prof::VERTS, v.len());
                gl.geometry(v, i);
            }
        }
    }

    /// Desenha e esvazia, guardando os buffers pro próximo frame (sem realocar).
    pub fn flush(&mut self, tex: &Texture2D) {
        self.redraw(tex);
        self.v.clear();
        self.i.clear();
        for (mut v, mut i) in self.done.drain(..) {
            v.clear();
            i.clear();
            self.pool.push((v, i));
        }
    }
}
