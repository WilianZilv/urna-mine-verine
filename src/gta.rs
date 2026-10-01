//! Personagem "bandido" no estilo GTA 3 (tudo original, nada de código/asset do GTA): terceira pessoa,
//! arsenal completo e o sedã da primeira missão estacionado perto da torre.

use crate::batch::Batch;
use crate::models::{Look, rgb};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Melee,
    Gun,
    Rocket,
    Flame,
    Grenade,
    Molotov,
}

pub struct Weapon {
    pub name: &'static str,
    pub kind: Kind,
    pub dmg: f32,
    pub rate: f32,
    pub pellets: u32,
    pub spread: f32,
    pub range: f32,
    pub auto: bool,
    len: f32,
    col: Color,
}

const fn c(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

const fn w(name: &'static str, kind: Kind, dmg: f32, rate: f32, pellets: u32, spread: f32, range: f32, auto: bool, len: f32, col: Color) -> Weapon {
    Weapon { name, kind, dmg, rate, pellets, spread, range, auto, len, col }
}

pub const ARSENAL: [Weapon; 12] = [
    w("PUNHO", Kind::Melee, 5.0, 0.4, 1, 0.0, 2.2, false, 0.0, c(0.0, 0.0, 0.0)),
    w("TACO DE BEISEBOL", Kind::Melee, 11.0, 0.6, 1, 0.0, 2.8, false, 0.9, c(0.6, 0.4, 0.2)),
    w("PISTOLA", Kind::Gun, 12.0, 0.35, 1, 0.01, 60.0, false, 0.25, c(0.15, 0.15, 0.16)),
    w("UZI", Kind::Gun, 7.0, 0.08, 1, 0.045, 45.0, true, 0.35, c(0.12, 0.12, 0.13)),
    w("ESCOPETA", Kind::Gun, 7.0, 0.9, 8, 0.09, 25.0, false, 0.8, c(0.35, 0.22, 0.12)),
    w("AK-47", Kind::Gun, 11.0, 0.11, 1, 0.025, 80.0, true, 0.85, c(0.45, 0.28, 0.12)),
    w("M16", Kind::Gun, 10.0, 0.08, 1, 0.02, 90.0, true, 0.9, c(0.1, 0.1, 0.1)),
    w("SNIPER", Kind::Gun, 70.0, 1.3, 1, 0.0, 160.0, false, 1.1, c(0.25, 0.3, 0.2)),
    w("LANCA-FOGUETE", Kind::Rocket, 0.0, 1.4, 1, 0.0, 0.0, false, 1.2, c(0.3, 0.38, 0.22)),
    w("LANCA-CHAMAS", Kind::Flame, 3.0, 0.2, 1, 0.0, 7.0, true, 0.9, c(0.6, 0.15, 0.1)),
    w("GRANADA", Kind::Grenade, 0.0, 0.9, 1, 0.0, 0.0, false, 0.0, c(0.25, 0.35, 0.2)),
    w("MOLOTOV", Kind::Molotov, 0.0, 0.9, 1, 0.0, 0.0, false, 0.0, c(0.4, 0.6, 0.3)),
];

pub fn look() -> Look {
    Look { skin: rgb(0.85, 0.68, 0.55), hair: rgb(0.15, 0.1, 0.07), shirt: rgb(0.32, 0.2, 0.12), pants: rgb(0.25, 0.32, 0.18), shoes: rgb(0.08, 0.08, 0.08), beard: None, glasses: false, wolverine: false, toga: false }
}

/// Alvo atingível: (centro, raio, índice, é lutador).
pub type Target = (Vec3, f32, usize, bool);

pub enum Out {
    Hit { i: usize, fighter: bool, dmg: f32, dir: Vec3 },
    Boom(Vec3, f32),
    Spark(Vec3),
    Bang(Vec3),
}

struct Proj {
    pos: Vec3,
    vel: Vec3,
    kind: Kind,
    fuse: f32,
}

pub struct Bandido {
    pub weapon: usize,
    cool: f32,
    flash: f32,
    swing: f32,
    flame: f32,
    projs: Vec<Proj>,
    tracers: Vec<(Vec3, Vec3, f32)>,
    pub driving: bool,
    pub hit_cd: f32,
}

impl Bandido {
    pub fn new() -> Self {
        Bandido { weapon: 2, cool: 0.0, flash: 0.0, swing: 0.0, flame: 0.0, projs: Vec::new(), tracers: Vec::new(), driving: false, hit_cd: 0.0 }
    }

    pub fn cycle(&mut self, d: i32) {
        self.weapon = (self.weapon as i32 + d).rem_euclid(ARSENAL.len() as i32) as usize;
    }

    /// Dispara a arma atual. `eye`/`aim` = raio da mira (câmera); `muzzle` = cano da arma.
    pub fn fire(&mut self, world: &World, eye: Vec3, aim: Vec3, muzzle: Vec3, body: Vec3, targets: &[Target], out: &mut Vec<Out>) {
        if self.cool > 0.0 {
            return;
        }
        let wpn = &ARSENAL[self.weapon];
        self.cool = wpn.rate;
        match wpn.kind {
            Kind::Melee => {
                self.swing = 1.0;
                let flat = vec3(aim.x, 0.0, aim.z).normalize_or_zero();
                for &(p, r, i, fighter) in targets {
                    let d = p - body;
                    if d.length() < wpn.range + r && flat.dot(d.normalize_or_zero()) > 0.5 {
                        out.push(Out::Hit { i, fighter, dmg: wpn.dmg, dir: flat });
                        break;
                    }
                }
            }
            Kind::Gun => {
                self.flash = 0.06;
                out.push(Out::Bang(muzzle));
                for _ in 0..wpn.pellets {
                    let d = (aim + vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)) * wpn.spread).normalize();
                    let wall = world.raycast(eye, d, wpn.range).map(|h| h.2).unwrap_or(wpn.range);
                    let mut best: Option<(f32, usize, bool)> = None;
                    for &(p, r, i, fighter) in targets {
                        if let Some(t) = crate::urna::ray_sphere(eye, d, p, r) {
                            if t < wall && best.is_none_or(|b| t < b.0) {
                                best = Some((t, i, fighter));
                            }
                        }
                    }
                    let end = eye + d * best.map(|b| b.0).unwrap_or(wall);
                    self.tracers.push((muzzle, end, 0.06));
                    match best {
                        Some((_, i, fighter)) => out.push(Out::Hit { i, fighter, dmg: wpn.dmg, dir: vec3(d.x, 0.0, d.z).normalize_or_zero() }),
                        None if wall < wpn.range => out.push(Out::Spark(end)),
                        None => {}
                    }
                }
            }
            Kind::Flame => {
                self.flame = 0.25;
                for &(p, r, i, fighter) in targets {
                    let d = p - muzzle;
                    if d.length() < wpn.range + r && aim.dot(d.normalize_or_zero()) > 0.85 {
                        out.push(Out::Hit { i, fighter, dmg: wpn.dmg, dir: vec3(aim.x, 0.0, aim.z).normalize_or_zero() });
                    }
                }
            }
            Kind::Rocket => {
                self.flash = 0.1;
                out.push(Out::Bang(muzzle));
                self.projs.push(Proj { pos: muzzle, vel: aim * 45.0, kind: Kind::Rocket, fuse: 4.0 });
            }
            Kind::Grenade | Kind::Molotov => {
                self.swing = 1.0;
                self.projs.push(Proj { pos: muzzle, vel: aim * 17.0 + Vec3::Y * 5.0, kind: wpn.kind, fuse: 2.2 });
            }
        }
    }

    pub fn update(&mut self, world: &World, dt: f32, targets: &[Target], out: &mut Vec<Out>) {
        self.cool -= dt;
        self.flash -= dt;
        self.hit_cd -= dt;
        self.swing = (self.swing - dt * 4.0).max(0.0);
        self.flame = (self.flame - dt).max(0.0);
        for t in self.tracers.iter_mut() {
            t.2 -= dt;
        }
        self.tracers.retain(|t| t.2 > 0.0);
        for p in self.projs.iter_mut() {
            p.fuse -= dt;
            if p.kind != Kind::Rocket {
                p.vel.y -= 20.0 * dt;
            }
            let np = p.pos + p.vel * dt;
            let near = targets.iter().any(|t| t.0.distance(np) < t.1 + 0.4);
            let solid = world.solid_f(np.x, np.y, np.z);
            match p.kind {
                Kind::Rocket | Kind::Molotov if solid || near || p.fuse <= 0.0 => {
                    out.push(Out::Boom(p.pos, if p.kind == Kind::Rocket { 3.8 } else { 2.2 }));
                    p.fuse = -9.0;
                }
                Kind::Grenade if p.fuse <= 0.0 => {
                    out.push(Out::Boom(p.pos, 3.2));
                    p.fuse = -9.0;
                }
                Kind::Grenade if solid => {
                    let below = world.solid_f(p.pos.x, np.y, p.pos.z);
                    if below {
                        p.vel.y *= -0.4;
                    } else {
                        p.vel.x *= -0.5;
                        p.vel.z *= -0.5;
                    }
                    p.vel *= 0.7;
                }
                _ => p.pos = np,
            }
        }
        self.projs.retain(|p| p.fuse > -5.0);
    }

    /// Bandido a pé: corpo, arma na mão (mira), projéteis, traçantes e chamas.
    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, pos: Vec3, aim: Vec3, walk: f32, moving: bool, time: f32) {
        let flat = vec3(aim.x, 0.0, aim.z).normalize_or(Vec3::Z);
        let yaw = flat.x.atan2(flat.z);
        let wpn = &ARSENAL[self.weapon];
        let pitch = aim.y.asin();
        let arm = if self.swing > 0.0 { -std::f32::consts::PI * (0.3 + 0.6 * self.swing) } else if wpn.kind == Kind::Melee && wpn.len == 0.0 { -0.3 } else { -std::f32::consts::FRAC_PI_2 - pitch };
        let two_hands = wpn.len > 0.5 && wpn.kind != Kind::Melee;
        let pose = crate::models::Pose { walk, walk_amt: if moving { 1.0 } else { 0.0 }, arm_r: arm, arm_l: if two_hands { arm } else { -0.2 }, arm_l_out: if two_hands { -0.4 } else { 0.0 }, ..Default::default() };
        crate::models::draw_humanoid(b, &look(), &pose, &crate::models::root(pos, yaw, 0.0, 0.0));
        self.draw_weapon(b, trans, pos, aim, yaw, time);
        for p in &self.projs {
            let col = match p.kind {
                Kind::Rocket => rgb(0.3, 0.35, 0.25),
                Kind::Molotov => rgb(0.4, 0.6, 0.3),
                _ => rgb(0.2, 0.3, 0.15),
            };
            b.cube(&Mat4::from_translation(p.pos), Vec3::ZERO, Vec3::splat(0.22), col);
            if p.kind != Kind::Grenade {
                trans.glow(&Mat4::from_translation(p.pos - p.vel.normalize_or_zero() * 0.3), Vec3::ZERO, Vec3::splat(0.3 + 0.1 * (time * 40.0).sin()), Color::new(1.0, 0.6, 0.1, 0.8));
            }
        }
        for &(a, c, t) in &self.tracers {
            crate::urna::beam(trans, a, c, 0.05, Color::new(1.0, 0.9, 0.4, (t / 0.06).min(1.0)));
        }
    }

    fn draw_weapon(&self, b: &mut Batch, trans: &mut Batch, pos: Vec3, aim: Vec3, yaw: f32, time: f32) {
        let wpn = &ARSENAL[self.weapon];
        let right = vec3(-yaw.cos(), 0.0, yaw.sin());
        let hand = pos + Vec3::Y * 1.35 + right * 0.33 + aim * 0.55;
        let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, aim), hand);
        match wpn.kind {
            Kind::Melee if wpn.len > 0.0 => b.cube(&m, vec3(0.0, 0.0, 0.4), vec3(0.08, 0.08, 0.9), wpn.col),
            Kind::Melee => {}
            Kind::Grenade | Kind::Molotov => b.cube(&m, Vec3::ZERO, Vec3::splat(0.18), wpn.col),
            Kind::Rocket => {
                b.cube(&m, vec3(0.0, 0.1, 0.1), vec3(0.2, 0.2, wpn.len), wpn.col);
                b.cube(&m, vec3(0.0, 0.1, 0.1 + wpn.len * 0.5), vec3(0.24, 0.24, 0.08), rgb(0.15, 0.15, 0.15));
            }
            _ => {
                b.cube(&m, vec3(0.0, 0.0, wpn.len * 0.4), vec3(0.08, 0.12, wpn.len), wpn.col);
                b.cube(&m, vec3(0.0, -0.1, 0.05), vec3(0.06, 0.15, 0.08), rgb(0.1, 0.1, 0.1));
            }
        }
        let tip = hand + aim * (wpn.len * 0.9 + 0.1);
        if self.flash > 0.0 {
            trans.glow(&Mat4::from_translation(tip), Vec3::ZERO, Vec3::splat(0.35), Color::new(1.0, 0.85, 0.3, 0.9));
        }
        if self.flame > 0.0 {
            for k in 0..14 {
                let f = ((time * 6.0 + k as f32 * 0.37).fract()) * 6.5;
                let p = tip + aim * f + vec3((k as f32 * 3.1 + time * 9.0).sin(), (k as f32 * 1.7 + time * 7.0).cos(), (k as f32 * 2.3).sin()) * f * 0.12;
                trans.glow(&Mat4::from_translation(p), Vec3::ZERO, Vec3::splat(0.25 + f * 0.12), Color::new(1.0, 0.35 + 0.4 * (1.0 - f / 6.5), 0.05, 0.75));
            }
        }
    }

    pub fn muzzle(pos: Vec3, aim: Vec3) -> Vec3 {
        let flat = vec3(aim.x, 0.0, aim.z).normalize_or(Vec3::Z);
        let right = vec3(-flat.z, 0.0, flat.x);
        pos + Vec3::Y * 1.35 + right * 0.33 + aim * 1.2
    }
}

/// Sedã da primeira missão: arcade com aderência lateral, freio de mão (drift) e sobe degrau de 1 bloco.
pub struct Car {
    pub pos: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    vy: f32,
    steer: f32,
    wheel: f32,
}

impl Car {
    pub fn new(world: &World) -> Self {
        let (x, z) = (70.5, 99.5);
        Car { pos: vec3(x, world.floor_at(x, G as f32 + 3.0, z), z), yaw: -std::f32::consts::FRAC_PI_2, vel: Vec3::ZERO, vy: 0.0, steer: 0.0, wheel: 0.0 }
    }

    pub fn fwd(&self) -> Vec3 {
        vec3(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    pub fn speed(&self) -> f32 {
        self.vel.dot(self.fwd())
    }

    fn hits(&self, world: &World, p: Vec3, base: f32) -> bool {
        let f = vec3(self.yaw.sin(), 0.0, self.yaw.cos());
        let l = vec3(f.z, 0.0, -f.x);
        for (a, s) in [(2.1, 0.85), (2.1, -0.85), (-2.1, 0.85), (-2.1, -0.85), (0.0, 0.85), (0.0, -0.85), (2.1, 0.0)] {
            let q = p + f * a + l * s;
            for h in [0.15, 1.0, 1.5] {
                if world.solid_f(q.x, base + h, q.z) {
                    return true;
                }
            }
        }
        false
    }

    /// Retorna a velocidade da batida (0 se não bateu). `steer` > 0 = esquerda.
    pub fn update(&mut self, world: &World, dt: f32, throttle: f32, steer: f32, handbrake: bool) -> f32 {
        let f = self.fwd();
        let l = vec3(f.z, 0.0, -f.x);
        let (mut vf, mut vs) = (self.vel.dot(f), self.vel.dot(l));
        if throttle > 0.0 {
            if vf < 28.0 {
                vf += throttle * if vf < 0.0 { 22.0 } else { 14.0 } * dt;
            }
        } else if throttle < 0.0 {
            vf = if vf > 0.5 { vf + throttle * 22.0 * dt } else { (vf + throttle * 8.0 * dt).max(-9.0) };
        }
        vf -= vf * 0.25 * dt + vf.signum() * dt * (vf.abs() > 0.2) as i32 as f32;
        vs *= (-dt * if handbrake { 1.5 } else { 9.0 }).exp();
        if handbrake {
            vf *= (-dt * 1.2).exp();
        }
        self.steer += (steer - self.steer) * (dt * 8.0).min(1.0);
        self.yaw += self.steer * 2.0 * (vf / 8.0).clamp(-1.0, 1.0) * dt * if handbrake { 1.5 } else { 1.0 };
        let f = self.fwd();
        let l = vec3(f.z, 0.0, -f.x);
        self.vel = f * vf + l * vs;
        self.wheel += vf * dt * 2.8;

        let np = self.pos + self.vel * dt;
        let ground = world.floor_at(np.x, self.pos.y + 1.2, np.z);
        let base = ground.max(self.pos.y);
        let mut crash = 0.0;
        if self.hits(world, np, base) {
            crash = self.vel.length();
            self.vel *= -0.3;
        } else {
            self.pos.x = np.x;
            self.pos.z = np.z;
        }
        let ground = world.floor_at(self.pos.x, self.pos.y + 1.2, self.pos.z);
        if ground >= self.pos.y - 0.05 {
            self.pos.y = ground;
            self.vy = 0.0;
        } else {
            self.vy -= 25.0 * dt;
            self.pos.y = (self.pos.y + self.vy * dt).max(ground);
        }
        if self.pos.y < -20.0 {
            *self = Car::new(world);
        }
        crash
    }

    pub fn draw(&self, b: &mut Batch, driver: bool) {
        draw_car(b, self.pos, self.yaw, self.steer, self.wheel, driver);
    }
}

pub fn draw_car(b: &mut Batch, pos: Vec3, yaw: f32, steer: f32, wheel: f32, driver: bool) {
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
    let paint = rgb(0.55, 0.6, 0.66);
    let glass = rgb(0.08, 0.12, 0.16);
    let dark = rgb(0.07, 0.07, 0.08);
    b.cube(&m, vec3(0.0, 0.6, 0.0), vec3(1.9, 0.6, 4.4), paint);
    b.cube(&m, vec3(0.0, 1.15, -0.25), vec3(1.7, 0.55, 2.2), paint);
    b.cube(&m, vec3(0.0, 1.15, 0.86), vec3(1.5, 0.45, 0.05), glass);
    b.cube(&m, vec3(0.0, 1.15, -1.36), vec3(1.5, 0.45, 0.05), glass);
    for s in [-1.0f32, 1.0] {
        b.cube(&m, vec3(s * 0.86, 1.15, -0.25), vec3(0.03, 0.42, 1.95), glass);
        b.glow(&m, vec3(s * 0.65, 0.65, 2.21), vec3(0.35, 0.15, 0.03), rgb(1.0, 0.95, 0.75));
        b.glow(&m, vec3(s * 0.7, 0.68, -2.21), vec3(0.3, 0.15, 0.03), rgb(0.9, 0.05, 0.05));
        for z in [1.4f32, -1.4] {
            let wm = m * Mat4::from_translation(vec3(s * 0.92, 0.36, z)) * Mat4::from_rotation_y(if z > 0.0 { steer * 0.5 } else { 0.0 }) * Mat4::from_rotation_x(wheel);
            b.cube(&wm, Vec3::ZERO, vec3(0.28, 0.7, 0.7), dark);
            b.cube(&wm, vec3(s * 0.15, 0.0, 0.0), vec3(0.03, 0.3, 0.3), rgb(0.7, 0.7, 0.72));
        }
    }
    b.cube(&m, vec3(0.0, 0.42, 2.23), vec3(1.92, 0.22, 0.1), dark);
    b.cube(&m, vec3(0.0, 0.42, -2.23), vec3(1.92, 0.22, 0.1), dark);
    b.cube(&m, vec3(0.0, 0.62, 2.235), vec3(0.5, 0.18, 0.02), rgb(0.85, 0.85, 0.85));
    if driver {
        b.cube(&m, vec3(0.42, 1.2, 0.0), vec3(0.32, 0.32, 0.32), rgb(0.85, 0.68, 0.55));
        b.cube(&m, vec3(0.42, 1.38, 0.0), vec3(0.34, 0.08, 0.34), rgb(0.15, 0.1, 0.07));
    }
}
