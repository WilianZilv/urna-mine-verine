//! Ragdoll ativo próprio (inspirado no comportamento do Euphoria, sem código dele): esqueleto de
//! partículas Verlet com restrições de osso, "músculos" puxando pra uma pose animada e comportamentos:
//! cambalear dando passos de equilíbrio com os braços em moinho, cair protegendo com as mãos e levantar.

use crate::batch::Batch;
use crate::models::rgb;
use crate::urna::limb;
use crate::world::World;
use macroquad::prelude::*;

const N: usize = 15;
const BONES: [(usize, usize); 24] = [
    (0, 1), (1, 2), (1, 3), (1, 6), (3, 6), (3, 4), (4, 5), (6, 7), (7, 8), (0, 9), (0, 12), (9, 12), (9, 10), (10, 11), (12, 13), (13, 14),
    (3, 0), (6, 0), (9, 1), (12, 1), (2, 3), (2, 6), (3, 12), (6, 9),
];

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    Stagger,
    Fall,
    GetUp,
}

pub struct Ragdoll {
    p: [Vec3; N],
    prev: [Vec3; N],
    len: Vec<(usize, usize, f32)>,
    pub mode: Mode,
    t: f32,
    still: f32,
    step_cd: f32,
    plant: [Vec3; 2],
    fall_dir: Vec3,
    yaw: f32,
    base: Vec3,
}

/// Pose animada (andar/agachar) em coordenadas de mundo. Índices: 0 pelve, 1 peito, 2 cabeça,
/// 3-5 ombro/cotovelo/mão esq, 6-8 dir, 9-11 quadril/joelho/pé esq, 12-14 dir.
fn pose(root: Vec3, yaw: f32, walk: f32, amt: f32, crouch: f32) -> [Vec3; N] {
    let r = Mat4::from_translation(root) * Mat4::from_rotation_y(yaw);
    let s = walk.sin() * amt;
    let lift = |x: f32| x.max(0.0) * 0.14 * amt;
    let (d, c) = (crouch * 0.5, crouch);
    let pl = |x: f32, y: f32, z: f32| r.transform_point3(vec3(x, y, z));
    [
        pl(0.0, 0.95 - d, c * 0.1),
        pl(0.0, 1.4 - d * 0.8, 0.05 + c * 0.3),
        pl(0.0, 1.68 - d * 0.8, 0.07 + c * 0.35),
        pl(0.22, 1.42 - d * 0.8, c * 0.3),
        pl(0.26, 1.15 - d * 0.8, -s * 0.15 + c * 0.4),
        pl(0.27, 0.9 - d * 0.6, -s * 0.3 + c * 0.5),
        pl(-0.22, 1.42 - d * 0.8, c * 0.3),
        pl(-0.26, 1.15 - d * 0.8, s * 0.15 + c * 0.4),
        pl(-0.27, 0.9 - d * 0.6, s * 0.3 + c * 0.5),
        pl(0.11, 0.92 - d, c * 0.1),
        pl(0.11, 0.5 - d * 0.4 + lift(s) * 0.5, s * 0.25 + c * 0.45),
        pl(0.11, 0.06 + lift(s), s * 0.35),
        pl(-0.11, 0.92 - d, c * 0.1),
        pl(-0.11, 0.5 - d * 0.4 + lift(-s) * 0.5, -s * 0.25 + c * 0.45),
        pl(-0.11, 0.06 + lift(-s), -s * 0.35),
    ]
}

fn flat(v: Vec3) -> Vec3 {
    vec3(v.x, 0.0, v.z)
}

impl Ragdoll {
    pub fn new(root: Vec3, yaw: f32) -> Self {
        let p = pose(root, yaw, 0.0, 0.0, 0.0);
        let len = BONES.iter().map(|&(a, b)| (a, b, p[a].distance(p[b]))).collect();
        Ragdoll { p, prev: p, len, mode: Mode::Normal, t: 0.0, still: 0.0, step_cd: 0.0, plant: [p[11], p[14]], fall_dir: Vec3::Z, yaw, base: root }
    }

    pub fn controlled(&self) -> bool {
        self.mode == Mode::Normal
    }

    pub fn pelvis(&self) -> Vec3 {
        self.p[0]
    }

    /// Impulso (m/s) — explosão, tiro, tropeço. Forte derruba; fraco faz cambalear.
    pub fn hit(&mut self, imp: Vec3, dt: f32) {
        let h = dt * 0.5;
        for i in 0..N {
            let w = if i <= 8 { 1.0 } else { 0.55 };
            self.prev[i] -= imp * w * h;
        }
        self.fall_dir = flat(imp).normalize_or(Vec3::Z);
        let strong = imp.length() > 9.0 || (self.mode == Mode::Stagger && imp.length() > 5.0);
        if strong {
            self.mode = Mode::Fall;
        } else if self.mode == Mode::Normal {
            self.mode = Mode::Stagger;
            self.plant = [self.p[11], self.p[14]];
        }
        self.t = 0.0;
        self.still = 0.0;
    }

    /// Retorna Some(chão sob a pelve) quando devolve o controle ao jogador.
    pub fn update(&mut self, w: &World, dt: f32, root: Vec3, yaw: f32, walk: f32, amt: f32, time: f32) -> Option<Vec3> {
        let dt = dt.clamp(1e-4, 0.05);
        let h = dt * 0.5;
        self.t += dt;
        self.step_cd -= dt;
        let ground = |p: Vec3| w.floor_at(p.x, p.y + 0.3, p.z);
        let com = self.p.iter().copied().sum::<Vec3>() / N as f32;
        let mut target = pose(root, yaw, walk, amt, 0.0);
        let mut muscle: [f32; N];
        let mut give_back = None;
        match self.mode {
            Mode::Normal => {
                self.yaw = yaw;
                for i in 0..N {
                    let old = self.p[i];
                    self.p[i] = target[i];
                    self.prev[i] = old;
                }
                return None;
            }
            Mode::Stagger => {
                let pv = self.p[0];
                let base = vec3(pv.x, ground(pv), pv.z);
                target = pose(base, self.yaw, 0.0, 0.0, 0.25);
                let mid = (self.plant[0] + self.plant[1]) * 0.5;
                let off = flat(com - mid);
                let vel = flat(self.p[0] - self.prev[0]) / h;
                if off.length() > 0.2 && self.step_cd <= 0.0 {
                    let i = if (self.plant[0] - mid).dot(off) < (self.plant[1] - mid).dot(off) { 0 } else { 1 };
                    let mut f = mid + off * 2.2 + vel * 0.15;
                    f.y = ground(f);
                    self.plant[i] = f;
                    self.step_cd = 0.16;
                }
                target[11] = self.plant[0] + Vec3::Y * 0.06;
                target[14] = self.plant[1] + Vec3::Y * 0.06;
                let side = vec3(self.yaw.cos(), 0.0, -self.yaw.sin());
                let fwd = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
                for (k, (sh, hand, s)) in [(3usize, 5usize, 1.0f32), (6, 8, -1.0)].into_iter().enumerate() {
                    let a = time * 14.0 + k as f32 * 3.1;
                    target[hand] = self.p[sh] + side * s * 0.45 + Vec3::Y * (0.35 * a.sin()) + fwd * (0.35 * a.cos());
                }
                muscle = [0.35; N];
                muscle[11] = 0.7;
                muscle[14] = 0.7;
                let tilt = (self.p[1] - self.p[0]).normalize_or(Vec3::Y).y;
                if self.p[0].y - base.y < 0.55 || tilt < 0.55 {
                    self.mode = Mode::Fall;
                    self.t = 0.0;
                } else if self.t > 1.2 && off.length() < 0.15 {
                    self.mode = Mode::Normal;
                    give_back = Some(base);
                }
            }
            Mode::Fall => {
                muscle = [0.03; N];
                let mut land = flat(self.p[1]) + self.fall_dir * 0.5;
                land.y = ground(self.p[1]) + 0.05;
                let side = vec3(self.fall_dir.z, 0.0, -self.fall_dir.x);
                target[5] = land + side * 0.25;
                target[8] = land - side * 0.25;
                muscle[5] = 0.22;
                muscle[8] = 0.22;
                let speed = (0..N).map(|i| (self.p[i] - self.prev[i]).length()).sum::<f32>() / N as f32 / h;
                self.still = if speed < 0.5 { self.still + dt } else { 0.0 };
                if self.still > 0.9 {
                    self.mode = Mode::GetUp;
                    self.t = 0.0;
                    let pv = self.p[0];
                    self.base = vec3(pv.x, ground(pv), pv.z);
                    let d = flat(self.p[2] - self.p[0]);
                    if d.length() > 0.1 {
                        self.yaw = d.x.atan2(d.z);
                    }
                }
            }
            Mode::GetUp => {
                let k = (self.t / 1.6).min(1.0);
                let k = k * k * (3.0 - 2.0 * k);
                target = pose(self.base, self.yaw, 0.0, 0.0, 1.0 - k);
                muscle = [0.08 + 0.7 * k; N];
                if self.t > 1.6 {
                    self.mode = Mode::Normal;
                    give_back = Some(self.base);
                }
            }
        }

        for _ in 0..2 {
            for i in 0..N {
                let v = (self.p[i] - self.prev[i]) * 0.985;
                self.prev[i] = self.p[i];
                self.p[i] += v + Vec3::Y * (-22.0 * h * h * (1.0 - muscle[i]));
                self.p[i] += (target[i] - self.p[i]) * (muscle[i] * 0.3);
            }
            for _ in 0..5 {
                for &(a, b, l) in &self.len {
                    let d = self.p[b] - self.p[a];
                    let dl = d.length().max(1e-4);
                    let c = d * ((dl - l) / dl * 0.5);
                    self.p[a] += c;
                    self.p[b] -= c;
                }
            }
            for i in 0..N {
                let q = self.p[i];
                if q.y < 0.0 || w.solid_f(q.x, q.y, q.z) {
                    let top = q.y.floor() + 1.0;
                    if self.prev[i].y >= top - 0.1 || q.y < 0.0 {
                        self.p[i].y = top.max(0.0) + 0.001;
                        let d = self.p[i] - self.prev[i];
                        self.prev[i].x = self.p[i].x - d.x * 0.4;
                        self.prev[i].z = self.p[i].z - d.z * 0.4;
                    } else {
                        self.p[i].x = self.prev[i].x;
                        self.p[i].z = self.prev[i].z;
                    }
                }
            }
        }
        give_back
    }

    pub fn draw(&self, b: &mut Batch) {
        let p = &self.p;
        let jacket = rgb(0.36, 0.33, 0.29);
        let pants = rgb(0.14, 0.14, 0.17);
        let skin = rgb(0.86, 0.7, 0.58);
        limb(b, p[0], p[1], 0.34, jacket);
        limb(b, p[3], p[6], 0.2, jacket);
        limb(b, p[9], p[12], 0.22, pants);
        let up = (p[2] - p[1]).normalize_or(Vec3::Y);
        limb(b, p[1] + up * 0.05, p[2] - up * 0.08, 0.1, skin);
        limb(b, p[2] - up * 0.12, p[2] + up * 0.14, 0.25, skin);
        limb(b, p[2] + up * 0.1, p[2] + up * 0.16, 0.27, rgb(0.12, 0.09, 0.07));
        for (sh, el, ha) in [(3, 4, 5), (6, 7, 8)] {
            limb(b, p[sh], p[el], 0.12, jacket);
            limb(b, p[el], p[ha], 0.11, jacket);
            b.cube(&Mat4::from_translation(p[ha]), Vec3::ZERO, Vec3::splat(0.1), skin);
        }
        for (hip, kn, ft) in [(9, 10, 11), (12, 13, 14)] {
            limb(b, p[hip], p[kn], 0.15, pants);
            limb(b, p[kn], p[ft], 0.14, pants);
            b.cube(&Mat4::from_translation(p[ft]), Vec3::ZERO, vec3(0.14, 0.1, 0.14), rgb(0.08, 0.06, 0.05));
        }
    }
}
