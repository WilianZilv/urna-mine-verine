//! Urna eletrônica gigante: mira, carrega, dispara laser, explode blocos.
//! O escudo mágico do clube reflete os lasers.

use crate::batch::Batch;
use crate::models::rgb;
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::FRAC_PI_2;

pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub col: Color,
    pub life: f32,
    pub size: f32,
    pub gravity: bool,
}

pub struct Flash {
    pub pos: Vec3,
    pub t: f32,
    pub r: f32,
}

pub struct Beam {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub reflect: Option<(Vec3, Vec3)>,
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub flashes: Vec<Flash>,
    pub beams: Vec<Beam>,
    pub shield_flash: f32,
    pub shake: f32,
}

pub enum Shot {
    Deflected(Vec3),
    Exploded(Vec3, f32),
}

pub struct Urna {
    pub base: Vec3,
    pub pos: Vec3,
    pub yaw: f32,
    pub charging: bool,
    pub timer: f32,
    pub charge: f32,
    pub target: Vec3,
    pub barrage: u32,
    pub shots: u32,
}

const EYE: Vec3 = Vec3::new(-3.2, 0.8, 1.8);

impl Urna {
    pub fn new() -> Self {
        let base = vec3(108.0, G as f32 + 8.0, 64.5);
        Urna { base, pos: base, yaw: -FRAC_PI_2, charging: false, timer: 5.0, charge: 0.0, target: arena_center(), barrage: 0, shots: 0 }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_translation(self.pos) * Mat4::from_rotation_y(self.yaw)
    }

    pub fn eye(&self) -> Vec3 {
        self.matrix().transform_point3(EYE)
    }

    pub fn bob(&mut self, time: f32) {
        self.pos = self.base + vec3(0.0, (time * 0.9).sin() * 0.6, 0.0);
    }

    /// Retorna Some(alvo) quando dispara neste frame.
    pub fn update(&mut self, dt: f32, time: f32, mut pick: impl FnMut() -> Vec3) -> Option<Vec3> {
        self.bob(time);
        let to = self.target - self.pos;
        let want = to.x.atan2(to.z);
        self.yaw = crate::actors::angle_lerp(self.yaw, want, dt * 3.0);
        self.timer -= dt;
        if !self.charging {
            self.charge = (self.charge - dt * 3.0).max(0.0);
            if self.timer <= 0.0 {
                self.target = pick();
                self.charging = true;
                self.timer = if self.barrage > 0 { 0.3 } else { 0.8 };
            }
            return None;
        }
        self.charge = (self.charge + dt * 2.0).min(1.0);
        if self.timer > 0.0 {
            return None;
        }
        self.shots += 1;
        if self.barrage > 0 {
            self.barrage -= 1;
            self.target = pick();
            self.timer = 0.3;
        } else {
            self.charging = false;
            self.timer = gen_range(1.2, 2.8);
            if gen_range(0.0, 1.0) < 0.18 {
                self.barrage = 4;
            }
        }
        Some(self.target)
    }

    pub fn draw(&self, b: &mut Batch, time: f32) {
        let m = self.matrix();
        let beige = rgb(0.87, 0.84, 0.77);
        let panel = rgb(0.78, 0.75, 0.68);
        let dark = rgb(0.12, 0.12, 0.13);
        // Corpo
        b.cube(&m, vec3(0.0, 0.0, 0.0), vec3(14.0, 9.0, 3.0), beige);
        b.cube(&m, vec3(0.0, -0.3, 1.45), vec3(13.4, 7.6, 0.2), panel);
        // Faixa "JUSTIÇA ELEITORAL"
        b.cube(&m, vec3(0.0, 3.95, 1.55), vec3(13.6, 0.7, 0.1), rgb(0.25, 0.27, 0.3));
        b.cube(&m, vec3(-6.0, 3.95, 1.62), vec3(0.6, 0.5, 0.05), rgb(0.1, 0.55, 0.2));
        b.cube(&m, vec3(-6.0, 3.95, 1.66), vec3(0.3, 0.3, 0.05), rgb(1.0, 0.85, 0.1));
        // Tela (lado esquerdo do eleitor = -X local)
        b.cube(&m, vec3(-3.2, 0.8, 1.6), vec3(6.0, 4.6, 0.12), dark);
        let pulse = 0.85 + 0.15 * (time * 8.0).sin();
        let screen = if self.charging { Color::new(1.0, 0.25 + 0.2 * pulse, 0.25, 1.0) } else { rgb(0.82 * pulse, 0.92 * pulse, 1.0) };
        b.glow(&m, vec3(-3.2, 0.8, 1.68), vec3(5.2, 3.8, 0.05), screen);
        // "foto do candidato" e texto
        b.glow(&m, vec3(-4.6, 1.3, 1.72), vec3(1.4, 1.8, 0.04), rgb(0.35, 0.35, 0.4));
        for k in 0..3 {
            b.glow(&m, vec3(-2.4, 2.0 - k as f32 * 0.6, 1.72), vec3(2.6, 0.25, 0.04), rgb(0.15, 0.15, 0.2));
        }
        // Teclado (lado direito = +X local)
        b.cube(&m, vec3(3.6, -0.2, 1.6), vec3(5.2, 6.4, 0.12), rgb(0.2, 0.2, 0.22));
        for row in 0..4 {
            for col in 0..3 {
                if row == 3 && col != 1 {
                    continue;
                }
                let x = 2.2 + col as f32 * 1.4;
                let y = 2.0 - row as f32 * 1.1;
                b.cube(&m, vec3(x, y, 1.72), vec3(1.1, 0.8, 0.2), dark);
                b.glow(&m, vec3(x, y, 1.83), vec3(0.3, 0.4, 0.02), WHITE);
            }
        }
        b.cube(&m, vec3(2.1, -2.6, 1.72), vec3(1.3, 0.8, 0.2), rgb(0.95, 0.95, 0.95));
        b.cube(&m, vec3(3.6, -2.6, 1.72), vec3(1.3, 0.8, 0.2), rgb(0.95, 0.45, 0.1));
        b.cube(&m, vec3(5.2, -2.6, 1.72), vec3(1.6, 1.0, 0.2), rgb(0.15, 0.7, 0.25));
        // Propulsores mágicos
        for &x in &[-5.0f32, 0.0, 5.0] {
            let f = 0.7 + 0.3 * (time * 30.0 + x).sin();
            b.cube(&m, vec3(x, -4.8, 0.0), vec3(1.6, 0.6, 1.6), dark);
            b.glow(&m, vec3(x, -5.4 - f * 0.5, 0.0), vec3(1.0 * f, 0.8 + f, 1.0 * f), Color::new(1.0, 0.55 * f, 0.1, 1.0));
        }
        // Olho do laser carregando
        if self.charge > 0.0 {
            let s = 0.3 + self.charge * 1.4 + (time * 40.0).sin() * 0.1;
            b.glow(&m, EYE, vec3(s, s, s), Color::new(1.0, 0.2, 0.2, 1.0));
        }
    }
}

pub fn ray_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = o - c;
    let b = oc.dot(d);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t > 0.0 { Some(t) } else { None }
}

/// Tiro decidido pelo host; `apply` é determinístico e roda igual em todos os clientes.
pub struct Plan {
    pub o: Vec3,
    pub hit: Vec3,
    pub r: f32,
    pub deflect: bool,
}

pub fn plan(world: &World, o: Vec3, target: Vec3, big_chance: f32) -> Plan {
    let d = (target - o).normalize_or_zero();
    let max = (target - o).length() + 6.0;
    let wt = world.raycast(o, d, max).map(|h| h.2);
    let r = if gen_range(0.0, 1.0) < big_chance { 5.5 } else { gen_range(2.5, 4.0) };
    if let Some(s) = ray_sphere(o, d, shield_center(), SHIELD_R) {
        if wt.is_none_or(|w| s < w) {
            return Plan { o, hit: o + d * s, r, deflect: true };
        }
    }
    Plan { o, hit: wt.map(|w| o + d * w).unwrap_or(target), r, deflect: false }
}

pub fn apply(world: &mut World, plan: &Plan, fx: &mut Fx, avg: &[Color]) -> Shot {
    let (o, sc) = (plan.o, shield_center());
    let d = (plan.hit - o).normalize_or_zero();
    {
        if plan.deflect {
            let p = plan.hit;
            let n = (p - sc).normalize();
            let refl = d - 2.0 * d.dot(n) * n;
            fx.beams.push(Beam { from: o, to: p, t: 0.35, reflect: Some((p, p + refl * 70.0)) });
            fx.shield_flash = 1.0;
            for _ in 0..30 {
                fx.particles.push(Particle {
                    pos: p,
                    vel: (n * gen_range(2.0, 8.0) + vec3(gen_range(-4.0, 4.0), gen_range(-4.0, 4.0), gen_range(-4.0, 4.0))),
                    col: Color::new(0.6, 0.9, 1.0, 1.0),
                    life: gen_range(0.3, 0.8),
                    size: gen_range(0.08, 0.2),
                    gravity: false,
                });
            }
            return Shot::Deflected(p);
        }
    }
    let (impact, r) = (plan.hit, plan.r);
    fx.beams.push(Beam { from: o, to: impact, t: 0.3, reflect: None });
    explode(world, impact, r, fx, avg);
    Shot::Exploded(impact, r)
}

pub fn explode(world: &mut World, c: Vec3, r: f32, fx: &mut Fx, avg: &[Color]) {
    let ri = r.ceil() as i32 + 1;
    let ci = c.floor().as_ivec3();
    let sc = shield_center();
    for dy in -ri..=ri {
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                let p = ci + ivec3(dx, dy, dz);
                let bc = p.as_vec3() + Vec3::splat(0.5);
                let jitter = crate::atlas::hash2(p.x * 7 + p.y, p.z, 4242) * 0.8;
                if bc.distance(c) > r + jitter - 0.4 || p.y <= 0 || bc.distance(sc) < SHIELD_R + 0.5 {
                    continue;
                }
                let b = world.get(p.x, p.y, p.z);
                if b == AIR {
                    continue;
                }
                world.set(p.x, p.y, p.z, AIR);
                if gen_range(0.0, 1.0) < 0.3 {
                    fx.particles.push(Particle {
                        pos: bc,
                        vel: (bc - c).normalize_or_zero() * gen_range(4.0, 12.0) + vec3(0.0, gen_range(4.0, 10.0), 0.0),
                        col: avg[face_tile(b, 0)],
                        life: gen_range(1.0, 2.2),
                        size: gen_range(0.15, 0.35),
                        gravity: true,
                    });
                }
            }
        }
    }
    for _ in 0..25 {
        let f = gen_range(0.0, 1.0);
        fx.particles.push(Particle {
            pos: c,
            vel: vec3(gen_range(-1.0, 1.0), gen_range(0.0, 1.2), gen_range(-1.0, 1.0)) * gen_range(3.0, 9.0),
            col: Color::new(1.0, 0.4 + 0.5 * f, 0.1 * f, 1.0),
            life: gen_range(0.3, 0.9),
            size: gen_range(0.2, 0.5),
            gravity: false,
        });
    }
    fx.flashes.push(Flash { pos: c, t: 0.0, r });
}

impl Fx {
    pub fn update(&mut self, world: &World, dt: f32) {
        for p in self.particles.iter_mut() {
            if p.gravity {
                p.vel.y -= 20.0 * dt;
            } else {
                p.vel *= (1.0 - 2.5 * dt).max(0.0);
            }
            let np = p.pos + p.vel * dt;
            if p.gravity && world.solid_f(np.x, np.y, np.z) {
                p.vel *= -0.3;
            } else {
                p.pos = np;
            }
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for f in self.flashes.iter_mut() {
            f.t += dt;
        }
        self.flashes.retain(|f| f.t < 0.5);
        for b in self.beams.iter_mut() {
            b.t -= dt;
        }
        self.beams.retain(|b| b.t > 0.0);
        self.shield_flash = (self.shield_flash - dt * 2.0).max(0.0);
        self.shake = (self.shake - dt * 1.5).max(0.0);
    }

    pub fn draw_opaque(&self, b: &mut Batch) {
        for p in &self.particles {
            let s = p.size * (p.life / 0.4).min(1.0);
            if p.gravity {
                b.cube(&Mat4::IDENTITY, p.pos, Vec3::splat(s), p.col);
            } else {
                b.glow(&Mat4::IDENTITY, p.pos, Vec3::splat(s), p.col);
            }
        }
    }

    pub fn draw_transparent(&self, b: &mut Batch, time: f32) {
        let seg = |b: &mut Batch, a: Vec3, c: Vec3, w: f32, col: Color| {
            let d = c - a;
            let len = d.length();
            if len < 1e-3 {
                return;
            }
            let q = Quat::from_rotation_arc(Vec3::Y, d / len);
            let m = Mat4::from_rotation_translation(q, (a + c) * 0.5);
            b.glow(&m, Vec3::ZERO, vec3(w, len, w), col);
        };
        for beam in &self.beams {
            let k = (beam.t / 0.3).min(1.0);
            let wob = 1.0 + 0.2 * (time * 60.0).sin();
            seg(b, beam.from, beam.to, 1.1 * k * wob, Color::new(1.0, 0.1, 0.15, 0.35 * k));
            seg(b, beam.from, beam.to, 0.35 * k, Color::new(1.0, 0.85, 0.9, 0.95 * k));
            if let Some((a, c)) = beam.reflect {
                seg(b, a, c, 0.8 * k, Color::new(0.4, 0.8, 1.0, 0.4 * k));
                seg(b, a, c, 0.25 * k, Color::new(0.9, 1.0, 1.0, 0.9 * k));
            }
        }
    }

    pub fn draw_flashes(&self) {
        for f in &self.flashes {
            let k = 1.0 - f.t / 0.5;
            draw_sphere(f.pos, f.r * (0.6 + f.t * 2.5), None, Color::new(1.0, 0.6 * k + 0.2, 0.1, 0.6 * k));
        }
    }
}
