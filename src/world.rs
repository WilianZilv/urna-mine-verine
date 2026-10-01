//! Mundo voxel: geração da vila, clube de house, meshing por chunk com AO, raycast.

use crate::atlas::{self, *};
use crate::layout::{self, CLUB_D, LAB_D};
use macroquad::prelude::*;

pub const WX: i32 = crate::layout::SIZE;
pub const WY: i32 = 48;
pub const WZ: i32 = crate::layout::SIZE;
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
pub const CLUB_X0: i32 = 8 + CLUB_D.x;
pub const CLUB_X1: i32 = 36 + CLUB_D.x;
pub const CLUB_Z0: i32 = 48 + CLUB_D.y;
pub const CLUB_Z1: i32 = 80 + CLUB_D.y;
pub const FLOOR_X0: i32 = 16 + CLUB_D.x;
pub const FLOOR_X1: i32 = 32 + CLUB_D.x;
pub const FLOOR_Z0: i32 = 54 + CLUB_D.y;
pub const FLOOR_Z1: i32 = 74 + CLUB_D.y;
pub const SHIELD_R: f32 = 22.0;
pub const LAB_X0: i32 = 94 + LAB_D.x;
pub const LAB_X1: i32 = 112 + LAB_D.x;
pub const LAB_Z0: i32 = 52 + LAB_D.y;
pub const LAB_Z1: i32 = 76 + LAB_D.y;

pub fn shield_center() -> Vec3 {
    layout::club(vec3(22.0, G as f32 + 2.0, 64.0))
}
pub fn arena_center() -> Vec3 {
    layout::arena_center()
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
    /// Liga depois da geração: `set` ignora blocos dentro dos escudos (crate::shield).
    pub guard: bool,
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
        if x < 0 || z < 0 || y < 0 || x >= WX || z >= WZ || y >= WY || (self.guard && crate::shield::protected(ivec3(x, y, z))) {
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
            guard: false,
        };

        // Terreno: cidade plana no meio, morros com floresta nas bordas
        let mut flat = vec![0f32; (WX * WZ) as usize];
        let (m0, m1) = (22.0, WX as f32 - 22.0);
        for z in 0..WZ {
            for x in 0..WX {
                let (xf, zf) = (x as f32, z as f32);
                let bx = (m0 - xf).max(0.0).max(xf - m1);
                let bz = (m0 - zf).max(0.0).max(zf - m1);
                let t = smoothstep(0.0, 16.0, (bx * bx + bz * bz).sqrt());
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
        w.build_roads();
        w.build_plaza();
        w.build_arena();
        w.build_skate();

        for &(x0, z0, south) in layout::HOUSES.iter().chain([&layout::TOWER_HOUSE]) {
            w.build_house(x0, z0, south);
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
        w.fill(lx0, g, 62 + LAB_D.y, lx0, g + 3, 66 + LAB_D.y, AIR);
        for &(x, z) in &[(lx0, lz0), (lx1, lz0), (lx0, lz1), (lx1, lz1), (lx0, 61 + LAB_D.y), (lx0, 67 + LAB_D.y)] {
            w.fill(x, g, z, x, g + 4, z, STONE);
        }
        w.fill(lx0, g + 4, lz0, lx1, g + 4, lz0, NEON);
        w.fill(lx0, g + 4, lz1, lx1, g + 4, lz1, NEON);

        // Torre de observação (fim da rua sul)
        let t = layout::tower(vec3(62.0, 0.0, 104.0)).as_ivec3();
        let (tx, tz) = (t.x, t.z);
        w.fill(tx, g, tz, tx + 3, g + 9, tz + 3, COBBLE);
        for &(x, z) in &[(tx, tz), (tx + 3, tz), (tx, tz + 3), (tx + 3, tz + 3)] {
            w.fill(x, g, z, x, g + 9, z, LOG);
        }
        w.fill(tx, g + 9, tz, tx + 3, g + 9, tz + 3, PLANKS);

        // Árvores: floresta nos morros e bosques soltos entre os distritos
        for z in 3..WZ - 3 {
            for x in 3..WX - 3 {
                let t = flat[(z * WX + x) as usize];
                let forest = t >= 0.7 && hash2(x, z, 777) < 0.014;
                let grove = t < 0.3 && hash2(x, z, 777) < 0.006 && layout::free(x, z, 4);
                if !forest && !grove {
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
                            if r == 2 && dx.abs() == 2 && dz.abs() == 2 && hash2(x + dx, z + dz, 779u32.wrapping_add(dy as u32)) < 0.6 {
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
        crate::hub::build(&mut w);
        w
    }

    fn build_house(&mut self, x0: i32, z0: i32, door_south: bool) {
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
        let dz = if door_south { z1 } else { z0 };
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
        let (dx, dz) = (CLUB_D.x, CLUB_D.y);
        self.fill(9 + dx, g, 56 + dz, 14 + dx, g, 72 + dz, PLANKS);
        self.fill(13 + dx, g + 1, 60 + dz, 14 + dx, g + 1, 68 + dz, BLACK);
        // Caixas de som
        self.fill(9 + dx, g, 50 + dz, 11 + dx, g + 4, 52 + dz, BLACK);
        self.fill(9 + dx, g, 76 + dz, 11 + dx, g + 4, 78 + dz, BLACK);
    }

    fn lamp(&mut self, x: i32, z: i32) {
        self.fill(x, G, z, x, G + 2, z, BLACK);
        self.put(x, G + 3, z, NEON);
    }

    fn grass_free(&self, x: i32, z: i32) -> bool {
        self.get(x, G - 1, z) == GRASS && self.get(x, G, z) == AIR
    }

    /// Anel em volta da praça + ruas retas (cascalho com meio-fio de pedra), trilhas, postes e placas.
    fn build_roads(&mut self) {
        let g = G;
        let (pc, r) = (layout::PLAZA, layout::RING_R);
        for z in 0..WZ {
            for x in 0..WX {
                let e = (vec2(x as f32 + 0.5, z as f32 + 0.5).distance(pc) - r).abs();
                if e < 2.5 {
                    self.put(x, g - 1, z, if e < 1.5 { GRAVEL } else { STONE });
                    self.fill(x, g, z, x, g + 4, z, AIR);
                }
            }
        }
        for &(x0, z0, x1, z1) in &layout::ROADS {
            let along_z = z1 - z0 > x1 - x0;
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let curb = if along_z { x == x0 || x == x1 } else { z == z0 || z == z1 };
                    if !curb || self.get(x, g - 1, z) != GRAVEL {
                        self.put(x, g - 1, z, if curb { STONE } else { GRAVEL });
                    }
                    self.fill(x, g, z, x, g + 4, z, AIR);
                }
            }
        }
        for &(x0, z0, x1, z1) in &layout::PATHS {
            self.fill(x0, g - 1, z0, x1, g - 1, z1, GRAVEL);
        }
        // Postes a cada 14 blocos, alternando o lado da rua
        for &(x0, z0, x1, z1) in &layout::ROADS {
            let along_z = z1 - z0 > x1 - x0;
            let (a0, a1) = if along_z { (z0, z1) } else { (x0, x1) };
            for (k, a) in (a0 + 7..a1).step_by(14).enumerate() {
                let side = if k % 2 == 0 { -1 } else { 1 };
                let (x, z) = if along_z { (if side < 0 { x0 - 1 } else { x1 + 1 }, a) } else { (a, if side < 0 { z0 - 1 } else { z1 + 1 }) };
                if layout::free(x, z, 0) && self.grass_free(x, z) {
                    self.lamp(x, z);
                }
            }
        }
        for k in 0..20 {
            let a = k as f32 * std::f32::consts::TAU / 20.0 + 0.16;
            let (x, z) = ((pc.x + a.cos() * (r + 3.5)) as i32, (pc.y + a.sin() * (r + 3.5)) as i32);
            if layout::free(x, z, 0) && self.grass_free(x, z) {
                self.lamp(x, z);
            }
        }
        for &(x, z, _) in &layout::SIGNS {
            let (x, z) = (x as i32, z as i32);
            self.fill(x, g, z, x, g + 1, z, LOG);
            self.put(x, g + 2, z, PLANKS);
        }
    }

    /// Praça central: piso de pedregulho com anel de tijolo, fonte no meio, bancos e postes.
    fn build_plaza(&mut self) {
        let g = G;
        let (pc, pr) = (layout::PLAZA, layout::PLAZA_R);
        let (cx, cz) = (pc.x as i32, pc.y as i32);
        let n = pr as i32 + 1;
        for z in cz - n..=cz + n {
            for x in cx - n..=cx + n {
                let d = vec2(x as f32 + 0.5, z as f32 + 0.5).distance(pc);
                if d >= pr {
                    continue;
                }
                let b = if d > pr - 1.0 || d < 4.5 { STONE } else if (d - 11.0).abs() < 0.5 { BRICK } else { COBBLE };
                self.put(x, g - 1, z, b);
                self.fill(x, g, z, x, g + 6, z, AIR);
                if d < 3.5 {
                    self.put(x, g - 1, z, GLASS);
                } else if d < 4.5 {
                    self.put(x, g, z, STONE);
                }
                if d < 1.0 {
                    self.fill(x, g - 1, z, x, g + 2, z, STONE);
                    self.put(x, g + 3, z, NEON);
                }
            }
        }
        let tau = std::f32::consts::TAU;
        for k in 0..8 {
            let a = (k as f32 + 0.5) * tau / 8.0;
            let (dir, tan) = (vec2(a.cos(), a.sin()), vec2(-a.sin(), a.cos()));
            for s in -1..=1 {
                let p = pc + dir * 15.0 + tan * s as f32;
                self.put(p.x.floor() as i32, g, p.y.floor() as i32, PLANKS);
            }
            let p = pc + dir * (pr - 3.0);
            self.lamp(p.x.floor() as i32, p.y.floor() as i32);
        }
    }

    /// Arena aberta pros gigantes: grama com manchas de terra e borda de areia.
    fn build_arena(&mut self) {
        let g = G;
        let (c, h) = (layout::ARENA, layout::ARENA_HALF);
        let (x0, x1, z0, z1) = ((c.x - h.x) as i32, (c.x + h.x) as i32, (c.y - h.y) as i32, (c.y + h.y) as i32);
        for z in z0..=z1 {
            for x in x0..=x1 {
                let edge = x - x0 < 2 || x1 - x < 2 || z - z0 < 2 || z1 - z < 2;
                let b = if edge { SAND } else if vnoise(x as f32 / 9.0, z as f32 / 9.0, 41) > 0.68 { DIRT } else { GRASS };
                self.fill(x, g - 4, z, x, g - 2, z, DIRT);
                self.put(x, g - 1, z, b);
                self.fill(x, g, z, x, WY - 1, z, AIR);
            }
        }
        for &(x, z) in &[(x0, z0), (x1, z0), (x0, z1), (x1, z1)] {
            self.lamp(x, z);
        }
    }

    /// Pista de skate: piso de pedra, quarter pipes lisos norte/sul com deck, funbox com kickers,
    /// bank a leste, ledge e corrimão. As transições são superfícies analíticas (crate::skate):
    /// aqui só entra o volume de blocos inteiros por baixo delas e os decks.
    fn build_skate(&mut self) {
        let g = G;
        let (x0, z0, x1, z1) = layout::SKATE;
        for z in z0..=z1 {
            for x in x0..=x1 {
                self.put(x, g - 1, z, STONE);
                self.fill(x, g, z, x, g + 6, z, AIR);
                let k = crate::skate::park_fill(x, z);
                if k > 0 {
                    self.fill(x, g, z, x, g + k - 1, z, STONE);
                }
            }
        }
        self.fill(x0 + 1, g, z0, x1 - 1, g + 2, z0 + 1, STONE);
        self.fill(x0 + 1, g, z1 - 1, x1 - 1, g + 2, z1, STONE);
        self.fill(x1, g, z0 + 7, x1, g + 1, z0 + 29, STONE);
        self.fill(x0 + 6, g, z0 + 13, x0 + 17, g, z0 + 21, PLANKS);
        self.fill(x0 + 30, g, z0 + 10, x0 + 30, g, z0 + 26, COBBLE);
        self.fill(x0 + 37, g, z0 + 10, x0 + 37, g, z0 + 26, BLACK);
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
            // normal = retângulo UV do tile (o shader do chunk prende o UV nele)
            verts.push(Vertex {
                normal: vec4(u0, v0, u1, v1),
                ..Vertex::new(x as f32 + fx, y as f32 + fy, z as f32 + fz, u0 + (u1 - u0) * tu, v0 + (v1 - v0) * tv, Color::new(l, l, l, 1.0))
            });
        }
        if (aos[0] as u16 + aos[2] as u16) >= (aos[1] as u16 + aos[3] as u16) {
            idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            idx.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        }
    }
}
