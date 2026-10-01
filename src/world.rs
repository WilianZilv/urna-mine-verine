//! Mundo voxel: geração da vila, clube de house, meshing por chunk com AO, raycast.

use crate::atlas::{self, *};
use macroquad::prelude::*;

pub const WX: i32 = 128;
pub const WY: i32 = 48;
pub const WZ: i32 = 128;
/// Altura do chão da vila (y dos pés).
pub const G: i32 = 20;
pub const CHUNK: i32 = 16;
pub const CX: i32 = WX / CHUNK;
pub const CZ: i32 = WZ / CHUNK;

pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const SAND: u8 = 4;
pub const PLANKS: u8 = 5;
pub const LOG: u8 = 6;
pub const LEAVES: u8 = 7;
pub const COBBLE: u8 = 8;
pub const GLASS: u8 = 9;
pub const CLUBWALL: u8 = 10;
pub const BLACK: u8 = 11;
pub const BEDROCK: u8 = 12;
pub const BRICK: u8 = 13;
pub const GRAVEL: u8 = 14;
pub const WOOL: u8 = 15;
pub const NEON: u8 = 16;
pub const TNT: u8 = 17;
/// Fogo do isqueiro: não colide, não para o raycast nem vira malha (desenhado à parte).
pub const FIRE: u8 = 18;

// Clube de house (lado oeste)
pub const CLUB_X0: i32 = 8;
pub const CLUB_X1: i32 = 36;
pub const CLUB_Z0: i32 = 48;
pub const CLUB_Z1: i32 = 80;
pub const FLOOR_X0: i32 = 16;
pub const FLOOR_X1: i32 = 32;
pub const FLOOR_Z0: i32 = 54;
pub const FLOOR_Z1: i32 = 74;
pub const SHIELD_R: f32 = 22.0;
pub const LAB_X0: i32 = 94;
pub const LAB_X1: i32 = 112;
pub const LAB_Z0: i32 = 52;
pub const LAB_Z1: i32 = 76;

pub fn shield_center() -> Vec3 {
    vec3(22.0, G as f32 + 2.0, 64.0)
}
pub fn arena_center() -> Vec3 {
    vec3(64.0, G as f32, 64.0)
}

pub fn face_tile(b: u8, face: usize) -> usize {
    match b {
        GRASS => match face {
            2 => T_GRASS_TOP,
            3 => T_DIRT,
            _ => T_GRASS_SIDE,
        },
        LOG => {
            if face == 2 || face == 3 { T_LOG_TOP } else { T_LOG_SIDE }
        }
        DIRT => T_DIRT,
        STONE => T_STONE,
        SAND => T_SAND,
        PLANKS => T_PLANKS,
        LEAVES => T_LEAVES,
        COBBLE => T_COBBLE,
        GLASS => T_GLASS,
        CLUBWALL => T_CLUBWALL,
        BLACK => T_BLACK,
        BEDROCK => T_BEDROCK,
        BRICK => T_BRICK,
        GRAVEL => T_GRAVEL,
        WOOL => T_WOOL,
        NEON => T_NEON,
        TNT => {
            if face == 2 || face == 3 { T_TNT_TOP } else { T_TNT_SIDE }
        }
        _ => T_WHITE,
    }
}

const DIRS: [(i32, i32, i32); 6] = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)];
const FACE_SHADE: [f32; 6] = [0.82, 0.82, 1.0, 0.5, 0.68, 0.68];
const AO_CURVE: [f32; 4] = [0.45, 0.65, 0.82, 1.0];

pub struct World {
    pub blocks: Vec<u8>,
    pub dirty: Vec<bool>,
}

fn vnoise(x: f32, z: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let zi = z.floor() as i32;
    let fx = x - xi as f32;
    let fz = z - zi as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sz = fz * fz * (3.0 - 2.0 * fz);
    let a = hash2(xi, zi, seed);
    let b = hash2(xi + 1, zi, seed);
    let c = hash2(xi, zi + 1, seed);
    let d = hash2(xi + 1, zi + 1, seed);
    let ab = a + (b - a) * sx;
    let cd = c + (d - c) * sx;
    ab + (cd - ab) * sz
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl World {
    #[inline]
    fn idx(x: i32, y: i32, z: i32) -> usize {
        ((y * WZ + z) * WX + x) as usize
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 {
            return BEDROCK;
        }
        if x < 0 || z < 0 || x >= WX || z >= WZ || y >= WY {
            return AIR;
        }
        self.blocks[Self::idx(x, y, z)]
    }

    #[inline]
    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        !matches!(self.get(x, y, z), AIR | FIRE)
    }

    pub fn solid_f(&self, x: f32, y: f32, z: f32) -> bool {
        self.solid(x.floor() as i32, y.floor() as i32, z.floor() as i32)
    }

    fn put(&mut self, x: i32, y: i32, z: i32, b: u8) {
        if x < 0 || z < 0 || y < 0 || x >= WX || z >= WZ || y >= WY {
            return;
        }
        self.blocks[Self::idx(x, y, z)] = b;
    }

    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, b: u8) {
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    self.put(x, y, z, b);
                }
            }
        }
    }

    /// Altera bloco e marca chunks afetados para remesh.
    pub fn set(&mut self, x: i32, y: i32, z: i32, b: u8) {
        if x < 0 || z < 0 || y < 0 || x >= WX || z >= WZ || y >= WY {
            return;
        }
        self.blocks[Self::idx(x, y, z)] = b;
        let cx = x / CHUNK;
        let cz = z / CHUNK;
        let mut mark = |cx: i32, cz: i32| {
            if cx >= 0 && cz >= 0 && cx < CX && cz < CZ {
                self.dirty[(cz * CX + cx) as usize] = true;
            }
        };
        mark(cx, cz);
        if x % CHUNK == 0 {
            mark(cx - 1, cz);
        }
        if x % CHUNK == CHUNK - 1 {
            mark(cx + 1, cz);
        }
        if z % CHUNK == 0 {
            mark(cx, cz - 1);
        }
        if z % CHUNK == CHUNK - 1 {
            mark(cx, cz + 1);
        }
    }

    /// Topo do bloco sólido mais alto em (x,z) com índice <= floor(y).
    pub fn floor_at(&self, x: f32, y: f32, z: f32) -> f32 {
        let xi = x.floor() as i32;
        let zi = z.floor() as i32;
        let mut yi = (y.floor() as i32).min(WY - 1);
        while yi >= 0 {
            if self.solid(xi, yi, zi) {
                return yi as f32 + 1.0;
            }
            yi -= 1;
        }
        0.0
    }

    /// DDA voxel. Retorna (bloco atingido, célula anterior, distância).
    pub fn raycast(&self, o: Vec3, d: Vec3, max: f32) -> Option<(IVec3, IVec3, f32)> {
        let mut p = o.floor().as_ivec3();
        let step = ivec3(d.x.signum() as i32, d.y.signum() as i32, d.z.signum() as i32);
        let inv = |v: f32| if v.abs() < 1e-9 { f32::INFINITY } else { 1.0 / v.abs() };
        let t_delta = vec3(inv(d.x), inv(d.y), inv(d.z));
        let first = |o: f32, p: i32, d: f32| {
            if d > 0.0 {
                (p as f32 + 1.0 - o) / d
            } else if d < 0.0 {
                (o - p as f32) / -d
            } else {
                f32::INFINITY
            }
        };
        let mut t_max = vec3(first(o.x, p.x, d.x), first(o.y, p.y, d.y), first(o.z, p.z, d.z));
        let mut prev = p;
        let mut t = 0.0;
        while t <= max {
            if self.solid(p.x, p.y, p.z) {
                return Some((p, prev, t));
            }
            prev = p;
            if t_max.x < t_max.y && t_max.x < t_max.z {
                p.x += step.x;
                t = t_max.x;
                t_max.x += t_delta.x;
            } else if t_max.y < t_max.z {
                p.y += step.y;
                t = t_max.y;
                t_max.y += t_delta.y;
            } else {
                p.z += step.z;
                t = t_max.z;
                t_max.z += t_delta.z;
            }
        }
        None
    }

    pub fn generate() -> World {
        let mut w = World {
            blocks: vec![AIR; (WX * WY * WZ) as usize],
            dirty: vec![true; (CX * CZ) as usize],
        };

        // Terreno: vila plana no centro e no clube, colinas em volta
        let mut flat = vec![0f32; (WX * WZ) as usize];
        for z in 0..WZ {
            for x in 0..WX {
                let (xf, zf) = (x as f32, z as f32);
                let dc = ((xf - 64.0).powi(2) + (zf - 64.0).powi(2)).sqrt();
                let bx = (4.0 - xf).max(0.0).max(xf - 40.0);
                let bz = (44.0 - zf).max(0.0).max(zf - 84.0);
                let db = (bx * bx + bz * bz).sqrt();
                let t = smoothstep(48.0, 62.0, dc).min(smoothstep(0.0, 10.0, db));
                flat[(z * WX + x) as usize] = t;
                let n = 0.6 * vnoise(xf / 22.0, zf / 22.0, 1)
                    + 0.3 * vnoise(xf / 11.0, zf / 11.0, 2)
                    + 0.1 * vnoise(xf / 5.0, zf / 5.0, 3);
                let hill = (G - 3) as f32 + n * 14.0;
                let h = (G as f32 + (hill - G as f32) * t).round() as i32;
                for y in 0..h {
                    let b = if y == 0 {
                        BEDROCK
                    } else if y < h - 4 {
                        STONE
                    } else if y < h - 1 {
                        DIRT
                    } else {
                        GRASS
                    };
                    w.put(x, y, z, b);
                }
            }
        }

        let g = G;
        // Praça (arena da briga)
        for z in 0..WZ {
            for x in 0..WX {
                let d = (((x - 64) * (x - 64) + (z - 64) * (z - 64)) as f32).sqrt();
                if d < 13.0 {
                    w.put(x, g - 1, z, if d > 12.0 { STONE } else { COBBLE });
                }
            }
        }
        // Caminhos de cascalho
        w.fill(36, g - 1, 62, 52, g - 1, 65, GRAVEL);
        w.fill(76, g - 1, 62, 100, g - 1, 65, GRAVEL);
        w.fill(62, g - 1, 20, 65, g - 1, 52, GRAVEL);
        w.fill(62, g - 1, 76, 65, g - 1, 108, GRAVEL);

        // Casas
        for &(x0, z0) in &[(50, 30), (70, 30), (50, 90), (70, 90), (84, 44), (84, 78), (38, 28), (38, 92)] {
            w.build_house(x0, z0);
        }

        w.build_club();

        // Laboratório dos robôs cientistas (leste): piso, pilares, paredes de vidro, entrada a oeste
        let (lx0, lx1, lz0, lz1) = (LAB_X0, LAB_X1, LAB_Z0, LAB_Z1);
        w.fill(lx0, g, lz0, lx1, g + 8, lz1, AIR);
        w.fill(lx0, g - 1, lz0, lx1, g - 1, lz1, STONE);
        w.fill(lx0 + 1, g - 1, lz0 + 1, lx1 - 1, g - 1, lz1 - 1, BLACK);
        w.fill(lx0, g, lz0, lx1, g + 3, lz0, GLASS);
        w.fill(lx0, g, lz1, lx1, g + 3, lz1, GLASS);
        w.fill(lx1, g, lz0, lx1, g + 3, lz1, GLASS);
        w.fill(lx0, g, lz0, lx0, g + 3, lz1, GLASS);
        w.fill(lx0, g, 62, lx0, g + 3, 66, AIR);
        for &(x, z) in &[(lx0, lz0), (lx1, lz0), (lx0, lz1), (lx1, lz1), (lx0, 61), (lx0, 67)] {
            w.fill(x, g, z, x, g + 4, z, STONE);
        }
        w.fill(lx0, g + 4, lz0, lx1, g + 4, lz0, NEON);
        w.fill(lx0, g + 4, lz1, lx1, g + 4, lz1, NEON);

        // Torre de observação (spawn do jogador)
        w.fill(62, g, 104, 65, g + 9, 107, COBBLE);
        for &(x, z) in &[(62, 104), (65, 104), (62, 107), (65, 107)] {
            w.fill(x, g, z, x, g + 9, z, LOG);
        }
        w.fill(62, g + 9, 104, 65, g + 9, 107, PLANKS);

        // Árvores fora da vila
        for z in 3..WZ - 3 {
            for x in 3..WX - 3 {
                if flat[(z * WX + x) as usize] < 0.7 || hash2(x, z, 777) > 0.014 {
                    continue;
                }
                let top = w.floor_at(x as f32 + 0.5, WY as f32 - 1.0, z as f32 + 0.5) as i32;
                if w.get(x, top - 1, z) != GRASS {
                    continue;
                }
                let th = 4 + (hash2(x, z, 778) * 2.0) as i32;
                for dy in -2..=1 {
                    let r: i32 = if dy == 1 { 1 } else { 2 };
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if r == 2 && dx.abs() == 2 && dz.abs() == 2 && hash2(x + dx, z + dz, 779 + dy as u32) < 0.6 {
                                continue;
                            }
                            let (lx, ly, lz) = (x + dx, top + th + dy, z + dz);
                            if w.get(lx, ly, lz) == AIR {
                                w.put(lx, ly, lz, LEAVES);
                            }
                        }
                    }
                }
                w.fill(x, top, z, x, top + th - 1, z, LOG);
            }
        }
        w
    }

    fn build_house(&mut self, x0: i32, z0: i32) {
        let g = G;
        let (x1, z1) = (x0 + 6, z0 + 6);
        self.fill(x0, g - 1, z0, x1, g - 1, z1, COBBLE);
        self.fill(x0, g, z0, x1, g + 3, z1, PLANKS);
        self.fill(x0 + 1, g, z0 + 1, x1 - 1, g + 3, z1 - 1, AIR);
        for &(x, z) in &[(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
            self.fill(x, g, z, x, g + 3, z, LOG);
        }
        // Janelas
        for &i in &[2, 4] {
            for y in g + 1..=g + 2 {
                self.put(x0 + i, y, z0, GLASS);
                self.put(x0 + i, y, z1, GLASS);
                self.put(x0, y, z0 + i, GLASS);
                self.put(x1, y, z0 + i, GLASS);
            }
        }
        // Porta virada pro centro
        let dz = if z0 + 3 < 64 { z1 } else { z0 };
        self.put(x0 + 3, g, dz, AIR);
        self.put(x0 + 3, g + 1, dz, AIR);
        // Telhado escalonado
        for k in 0..4 {
            self.fill(x0 - 1 + k, g + 4 + k, z0 - 1 + k, x1 + 1 - k, g + 4 + k, z1 + 1 - k, BRICK);
        }
    }

    fn build_club(&mut self) {
        let g = G;
        // Piso preto
        self.fill(CLUB_X0, g - 1, CLUB_Z0, CLUB_X1, g - 1, CLUB_Z1, BLACK);
        self.fill(CLUB_X0, g, CLUB_Z0, CLUB_X1, g + 12, CLUB_Z1, AIR);
        // Paredes (oeste, norte, sul) + faixa neon
        self.fill(CLUB_X0, g, CLUB_Z0, CLUB_X0, g + 5, CLUB_Z1, CLUBWALL);
        self.fill(CLUB_X0, g, CLUB_Z0, CLUB_X1, g + 5, CLUB_Z0, CLUBWALL);
        self.fill(CLUB_X0, g, CLUB_Z1, CLUB_X1, g + 5, CLUB_Z1, CLUBWALL);
        self.fill(CLUB_X0, g + 6, CLUB_Z0, CLUB_X0, g + 6, CLUB_Z1, NEON);
        self.fill(CLUB_X0, g + 6, CLUB_Z0, CLUB_X1, g + 6, CLUB_Z0, NEON);
        self.fill(CLUB_X0, g + 6, CLUB_Z1, CLUB_X1, g + 6, CLUB_Z1, NEON);
        // Entrada leste: pilares + viga com neon
        self.fill(CLUB_X1, g, CLUB_Z0, CLUB_X1, g + 6, CLUB_Z0 + 2, CLUBWALL);
        self.fill(CLUB_X1, g, CLUB_Z1 - 2, CLUB_X1, g + 6, CLUB_Z1, CLUBWALL);
        self.fill(CLUB_X1, g + 5, CLUB_Z0, CLUB_X1, g + 5, CLUB_Z1, CLUBWALL);
        self.fill(CLUB_X1, g + 6, CLUB_Z0, CLUB_X1, g + 6, CLUB_Z1, NEON);
        // Palco + cabine do DJ
        self.fill(9, g, 56, 14, g, 72, PLANKS);
        self.fill(13, g + 1, 60, 14, g + 1, 68, BLACK);
        // Caixas de som
        self.fill(9, g, 50, 11, g + 4, 52, BLACK);
        self.fill(9, g, 76, 11, g + 4, 78, BLACK);
    }

    /// Gera meshes de um chunk (face culling + ambient occlusion por vértice).
    pub fn build_chunk(&self, cx: i32, cz: i32, tex: &Texture2D) -> Vec<Mesh> {
        let mut meshes = Vec::new();
        let mut verts: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u16> = Vec::new();
        for y in 0..WY {
            for z in cz * CHUNK..(cz + 1) * CHUNK {
                for x in cx * CHUNK..(cx + 1) * CHUNK {
                    let b = self.get(x, y, z);
                    if b == AIR || b == FIRE {
                        continue;
                    }
                    for (d, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                        if self.solid(x + dx, y + dy, z + dz) {
                            continue;
                        }
                        self.emit_face(&mut verts, &mut idx, x, y, z, b, d, [dx, dy, dz]);
                        if verts.len() >= 16000 {
                            meshes.push(Mesh {
                                vertices: std::mem::take(&mut verts),
                                indices: std::mem::take(&mut idx),
                                texture: Some(tex.clone()),
                            });
                        }
                    }
                }
            }
        }
        if !verts.is_empty() {
            meshes.push(Mesh { vertices: verts, indices: idx, texture: Some(tex.clone()) });
        }
        meshes
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_face(&self, verts: &mut Vec<Vertex>, idx: &mut Vec<u16>, x: i32, y: i32, z: i32, b: u8, d: usize, n: [i32; 3]) {
        let a = d / 2;
        let s = if d % 2 == 0 { 1 } else { 0 };
        let ua = (a + 1) % 3;
        let va = (a + 2) % 3;
        let (u0, v0, u1, v1) = atlas::uv(face_tile(b, d));
        let shade = FACE_SHADE[d];
        let base = verts.len() as u16;
        let nb = [x + n[0], y + n[1], z + n[2]];
        let mut aos = [0u8; 4];
        for (k, &(cu, cv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
            let mut p = [0i32; 3];
            p[a] = s;
            p[ua] = cu;
            p[va] = cv;
            let mut eu = [0i32; 3];
            eu[ua] = if cu == 1 { 1 } else { -1 };
            let mut ev = [0i32; 3];
            ev[va] = if cv == 1 { 1 } else { -1 };
            let s1 = self.solid(nb[0] + eu[0], nb[1] + eu[1], nb[2] + eu[2]);
            let s2 = self.solid(nb[0] + ev[0], nb[1] + ev[1], nb[2] + ev[2]);
            let c = self.solid(nb[0] + eu[0] + ev[0], nb[1] + eu[1] + ev[1], nb[2] + eu[2] + ev[2]);
            let ao = if s1 && s2 { 0 } else { 3 - (s1 as u8 + s2 as u8 + c as u8) };
            aos[k] = ao;
            let l = shade * AO_CURVE[ao as usize];
            let (fx, fy, fz) = (p[0] as f32, p[1] as f32, p[2] as f32);
            let (tu, tv) = match a {
                1 => (fx, fz),
                0 => (fz, 1.0 - fy),
                _ => (fx, 1.0 - fy),
            };
            verts.push(Vertex::new(
                x as f32 + fx,
                y as f32 + fy,
                z as f32 + fz,
                u0 + (u1 - u0) * tu,
                v0 + (v1 - v0) * tv,
                Color::new(l, l, l, 1.0),
            ));
        }
        if (aos[0] as u16 + aos[2] as u16) >= (aos[1] as u16 + aos[3] as u16) {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        }
    }
}
