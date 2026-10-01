//! Personagem skatista no estilo Skate 3 ("flick-it"): o mouse (ou arrastar no celular) faz o papel
//! do analógico direito. Puxa pra baixo e dá flick pra cima = ollie; diagonais = flips; lado = shove-it;
//! pra cima e flick pra baixo = nollie. Remada, carve, powerslide, manual, grind em quina e bail.
//! Física própria em cima dos voxels (não usa código nem assets do Skate 3).

use crate::batch::Batch;
use crate::models::rgb;
use crate::urna::{ik, limb};
use crate::world::World;
use macroquad::prelude::*;
use std::f32::consts::{PI, TAU};

const GRAV: f32 = 24.0;
const POP: f32 = 7.4;
const MAX_PUSH: f32 = 9.5;
const MAX_SPEED: f32 = 15.0;

/// Entrada já traduzida do teclado/mouse/toque. `steer` > 0 = esquerda.
#[derive(Default)]
pub struct Input {
    pub push: bool,
    pub brake: bool,
    pub steer: f32,
    pub mouse: Vec2,
    pub touch: Option<Vec2>,
    pub grab: bool,
    pub manual: bool,
    pub ollie: bool,
}

#[derive(Clone, Copy)]
enum Flick {
    Idle,
    Tail(f32),
    Nose(f32),
}

struct Trick {
    flip: f32,
    shuv: f32,
    t: f32,
    dur: f32,
}

impl Trick {
    fn angles(&self) -> (f32, f32) {
        let k = (self.t / self.dur).min(1.0);
        (self.flip * TAU * k, self.shuv * PI * k)
    }
}

struct Grind {
    axis: Vec3,
    y: f32,
}

pub struct Skater {
    pub pos: Vec3,
    pub heading: f32,
    speed: f32,
    vel: Vec3,
    on_ground: bool,
    rs: Vec2,
    flick: Flick,
    trick: Option<Trick>,
    crouch: f32,
    push_t: f32,
    slide: f32,
    manual: f32,
    grab: f32,
    spin: f32,
    air_t: f32,
    grind: Option<Grind>,
    bail: f32,
    bail_dir: Vec3,
    cam: f32,
    combo: Vec<(String, f32)>,
    land_t: f32,
    pub score: u32,
    pub popup: Option<(String, f32)>,
    /// Eventos do frame pro main (faíscas/som).
    pub sparks: bool,
    pub landed: bool,
    pub bailed: bool,
}

fn dir(h: f32) -> Vec3 {
    vec3(h.sin(), 0.0, h.cos())
}

fn angle_diff(a: f32, b: f32) -> f32 {
    (a - b + PI).rem_euclid(TAU) - PI
}

fn blocked(w: &World, p: Vec3) -> bool {
    let (x0, x1) = ((p.x - 0.3).floor() as i32, (p.x + 0.3).floor() as i32);
    let (z0, z1) = ((p.z - 0.3).floor() as i32, (p.z + 0.3).floor() as i32);
    for y in (p.y + 0.05).floor() as i32..=(p.y + 1.6).floor() as i32 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                if w.solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// Topo de bloco com 1 de largura (muro, corrimão): dá pra grindar ao longo de `along_x`.
fn ledge(w: &World, x: i32, y: i32, z: i32, along_x: bool) -> bool {
    let (dx, dz) = if along_x { (0, 1) } else { (1, 0) };
    w.solid(x, y - 1, z) && !w.solid(x, y, z) && !w.solid(x, y + 1, z) && !w.solid(x + dx, y - 1, z + dz) && !w.solid(x - dx, y - 1, z - dz)
}

impl Skater {
    pub fn new(pos: Vec3, heading: f32) -> Self {
        Skater {
            pos,
            heading,
            speed: 0.0,
            vel: Vec3::ZERO,
            on_ground: false,
            rs: Vec2::ZERO,
            flick: Flick::Idle,
            trick: None,
            crouch: 0.15,
            push_t: 0.0,
            slide: 0.0,
            manual: 0.0,
            grab: 0.0,
            spin: 0.0,
            air_t: 0.0,
            grind: None,
            bail: 0.0,
            bail_dir: Vec3::X,
            cam: heading,
            combo: Vec::new(),
            land_t: 0.0,
            score: 0,
            popup: None,
            sparks: false,
            landed: false,
            bailed: false,
        }
    }

    pub fn camera(&self) -> (Vec3, Vec3) {
        let d = dir(self.cam);
        (self.pos + Vec3::Y * 2.4 - d * 4.6, self.pos + Vec3::Y * 1.0 + d * 2.0)
    }

    /// Combo em andamento ("KICKFLIP + BS 180 + 50-50 GRIND").
    pub fn combo_text(&self) -> String {
        self.combo.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(" + ")
    }

    fn add(&mut self, name: &str, pts: f32) {
        match self.combo.last_mut() {
            Some(last) if last.0 == name && pts < 10.0 => last.1 += pts,
            _ => self.combo.push((name.to_string(), pts)),
        }
        self.land_t = 0.0;
    }

    fn bank(&mut self) {
        if self.combo.is_empty() {
            return;
        }
        let n = self.combo.len();
        let pts = (self.combo.iter().map(|c| c.1).sum::<f32>() * n as f32) as u32;
        self.score += pts;
        self.popup = Some((format!("{}  x{n}  = {pts}", self.combo_text()), 2.5));
        self.combo.clear();
    }

    pub fn bail_now(&mut self, why: &str) {
        if self.bail > 0.0 {
            return;
        }
        self.bail = 2.0;
        self.bailed = true;
        let hv = if self.on_ground { dir(self.heading) * self.speed } else { vec3(self.vel.x, 0.0, self.vel.z) };
        self.bail_dir = hv.normalize_or(dir(self.heading));
        self.vel = hv * 0.6 + Vec3::Y * self.vel.y.max(0.0);
        self.popup = Some((format!("BAIL! {why}"), 2.0));
        self.combo.clear();
        self.trick = None;
        self.grind = None;
        self.manual = 0.0;
        self.grab = 0.0;
        self.slide = 0.0;
    }

    /// Move no plano com colisão por eixo; zera o componente bloqueado e retorna a velocidade perdida.
    fn step(&mut self, w: &World, v: &mut Vec3, dt: f32) -> f32 {
        let mut lost = 0.0;
        for axis in [0, 2] {
            let mut p = self.pos;
            p[axis] += v[axis] * dt;
            if blocked(w, p) {
                lost += v[axis].abs();
                v[axis] = 0.0;
            } else {
                self.pos = p;
            }
        }
        lost
    }

    pub fn update(&mut self, w: &World, dt: f32, inp: &Input) {
        self.sparks = false;
        self.landed = false;
        self.bailed = false;
        if let Some((_, t)) = self.popup.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.popup = None;
            }
        }
        self.rs = match inp.touch {
            Some(t) => t,
            None => (self.rs * (-dt * 5.0).exp() + inp.mouse * 0.012).clamp_length_max(1.0),
        };
        if self.pos.y < -20.0 {
            self.pos = crate::player::Player::spawn();
            self.vel = Vec3::ZERO;
            self.speed = 0.0;
        }

        if self.bail > 0.0 {
            self.bail -= dt;
            self.vel.y -= GRAV * dt;
            let damp = (1.0 - 2.5 * dt).max(0.0);
            let mut hv = vec3(self.vel.x * damp, 0.0, self.vel.z * damp);
            self.step(w, &mut hv, dt);
            self.vel = vec3(hv.x, self.vel.y, hv.z);
            let f = w.floor_at(self.pos.x, self.pos.y + 0.3, self.pos.z);
            let ny = self.pos.y + self.vel.y * dt;
            if ny <= f {
                self.pos.y = f;
                self.vel.y = 0.0;
            } else {
                self.pos.y = ny;
            }
            if self.bail <= 0.0 {
                self.bail = 0.0;
                self.vel = Vec3::ZERO;
                self.speed = 0.0;
                self.flick = Flick::Idle;
                self.spin = 0.0;
                self.on_ground = ny <= f + 0.01;
            }
            return;
        }

        // Flick-it
        let fwd = dir(self.heading);
        let mut pop: Option<(&str, f32, f32, f32)> = None;
        let (x, y, len) = (self.rs.x, self.rs.y, self.rs.length());
        match self.flick {
            Flick::Idle => {
                if y < -0.6 {
                    self.flick = Flick::Tail(0.0);
                } else if y > 0.6 {
                    self.flick = Flick::Nose(0.0);
                }
            }
            Flick::Tail(t) => {
                if y > 0.45 {
                    pop = Some(if x < -0.4 { ("KICKFLIP", -1.0, 0.0, 0.5) } else if x > 0.4 { ("HEELFLIP", 1.0, 0.0, 0.5) } else { ("OLLIE", 0.0, 0.0, 0.0) });
                } else if x.abs() > 0.7 && y > -0.45 {
                    pop = Some(if x < 0.0 { ("POP SHOVE-IT", 0.0, -1.0, 0.45) } else { ("FS SHOVE-IT", 0.0, 1.0, 0.45) });
                } else if len < 0.2 && t > 0.3 {
                    self.flick = Flick::Idle;
                } else {
                    self.flick = Flick::Tail(t + dt);
                }
            }
            Flick::Nose(t) => {
                if y < -0.45 {
                    pop = Some(if x < -0.4 { ("NOLLIE FLIP", -1.0, 0.0, 0.5) } else if x > 0.4 { ("NOLLIE HEELFLIP", 1.0, 0.0, 0.5) } else { ("NOLLIE", 0.0, 0.0, 0.0) });
                } else if len < 0.2 && t > 0.3 {
                    self.flick = Flick::Idle;
                } else {
                    self.flick = Flick::Nose(t + dt);
                }
            }
        }
        if inp.ollie {
            pop = Some(("OLLIE", 0.0, 0.0, 0.0));
        }
        let (loaded, charge) = match self.flick {
            Flick::Tail(t) | Flick::Nose(t) => (true, t.min(0.6)),
            Flick::Idle => (false, 0.0),
        };
        let airborne = !self.on_ground && self.grind.is_none();
        let want = if loaded { 1.0 } else if airborne { 0.45 + 0.4 * self.grab } else { 0.15 };
        self.crouch += (want - self.crouch) * (dt * 12.0).min(1.0);
        if let Some((name, flip, shuv, dur)) = pop {
            self.flick = Flick::Idle;
            self.rs = Vec2::ZERO;
            if !airborne {
                let hv = match self.grind.take() {
                    Some(g) => g.axis * self.speed,
                    None => fwd * self.speed,
                };
                self.vel = hv + Vec3::Y * (POP + charge * 2.2);
                self.on_ground = false;
                self.air_t = 0.0;
                self.spin = 0.0;
                self.manual = 0.0;
                self.slide = 0.0;
                self.trick = (dur > 0.0).then_some(Trick { flip, shuv, t: 0.0, dur });
                self.add(name, if flip != 0.0 { 150.0 } else if shuv != 0.0 { 100.0 } else { 50.0 });
            }
        }

        if self.grind.is_some() {
            self.update_grind(w, dt);
        } else if self.on_ground {
            self.update_ground(w, dt, inp);
        } else {
            self.update_air(w, dt, inp);
        }

        let hv = vec3(self.vel.x, 0.0, self.vel.z);
        let travel = if self.on_ground || self.grind.is_some() || hv.length() < 1.0 { self.heading - self.spin } else { hv.x.atan2(hv.z) };
        self.cam += angle_diff(travel, self.cam) * (dt * 4.0).min(1.0);
    }

    fn update_ground(&mut self, w: &World, dt: f32, inp: &Input) {
        self.push_t = (self.push_t - dt).max(0.0);
        if inp.push && self.push_t <= 0.0 && self.manual < 0.5 {
            self.push_t = 0.6;
            if self.speed < MAX_PUSH {
                self.speed = (self.speed + 2.6).min(MAX_PUSH);
            }
        }
        let sliding = inp.brake && self.speed > 5.0;
        self.slide += ((if sliding { 1.25 } else { 0.0 }) - self.slide) * (dt * 8.0).min(1.0);
        let decel = if sliding { 8.0 } else if inp.brake { 5.0 } else { 0.3 };
        self.speed = (self.speed - decel * dt).clamp(0.0, MAX_SPEED);
        if sliding {
            self.add("POWERSLIDE", 0.0);
        }
        self.heading += inp.steer * 2.4 / (1.0 + self.speed * 0.06) * dt;
        let manual = inp.manual && self.speed > 1.0;
        self.manual += ((manual as i32 as f32) - self.manual) * (dt * 10.0).min(1.0);
        if manual {
            self.add("MANUAL", dt * 120.0);
        } else if !self.combo.is_empty() {
            self.land_t += dt;
            if self.land_t > 0.9 {
                self.bank();
            }
        }
        let fwd = dir(self.heading);
        let mut v = fwd * self.speed;
        let lost = self.step(w, &mut v, dt);
        if lost > 7.0 {
            self.bail_now("BATEU");
            return;
        }
        self.speed = v.dot(fwd).max(0.0);
        let f = w.floor_at(self.pos.x, self.pos.y + 0.3, self.pos.z);
        if f < self.pos.y - 0.05 {
            self.on_ground = false;
            self.vel = fwd * self.speed;
            self.air_t = 0.0;
            self.spin = 0.0;
        } else {
            self.pos.y = f;
        }
    }

    fn update_air(&mut self, w: &World, dt: f32, inp: &Input) {
        self.air_t += dt;
        self.vel.y -= GRAV * dt;
        let spin_v = inp.steer * 7.0;
        self.heading += spin_v * dt;
        self.spin += spin_v * dt;
        let grabbing = inp.grab && self.air_t > 0.12;
        self.grab += ((grabbing as i32 as f32) - self.grab) * (dt * 10.0).min(1.0);
        if grabbing {
            self.add("INDY GRAB", dt * 200.0);
        }
        if let Some(t) = self.trick.as_mut() {
            t.t += dt;
        }
        let mut hv = vec3(self.vel.x, 0.0, self.vel.z);
        self.step(w, &mut hv, dt);
        self.vel = vec3(hv.x, self.vel.y, hv.z);
        let ny = self.pos.y + self.vel.y * dt;
        if self.vel.y > 0.0 {
            if blocked(w, vec3(self.pos.x, ny, self.pos.z)) {
                self.vel.y = 0.0;
            } else {
                self.pos.y = ny;
            }
        } else {
            let f = w.floor_at(self.pos.x, self.pos.y + 0.3, self.pos.z);
            if ny <= f {
                self.pos.y = f;
                self.land(w);
            } else {
                self.pos.y = ny;
            }
        }
    }

    fn land(&mut self, w: &World) {
        let hv = vec3(self.vel.x, 0.0, self.vel.z);
        let spd = hv.length();
        if self.trick.as_ref().is_some_and(|t| t.t < t.dur * 0.85) {
            return self.bail_now("NAO COMPLETOU O TRUQUE");
        }
        self.trick = None;
        let turns = (self.spin / PI).round();
        if (self.spin - turns * PI).abs() > 0.7 {
            return self.bail_now("CAIU DE LADO");
        }
        let n = turns.abs() as i32;
        if n > 0 {
            self.add(&format!("{} {}", if self.spin > 0.0 { "BS" } else { "FS" }, n * 180), n as f32 * 100.0);
        }
        self.spin = 0.0;
        if spd > 1.0 {
            let vh = hv.x.atan2(hv.z);
            let mut d = angle_diff(self.heading, vh);
            if d.abs() > PI / 2.0 {
                d = angle_diff(self.heading + PI, vh);
            }
            if d.abs() > 0.65 {
                return self.bail_now("SHAPE TORTO");
            }
            self.heading = vh;
        }
        self.grab = 0.0;
        self.speed = spd;
        self.vel = Vec3::ZERO;
        self.landed = true;
        self.land_t = 0.0;
        let (xi, yi, zi) = (self.pos.x.floor() as i32, self.pos.y.round() as i32, self.pos.z.floor() as i32);
        if spd > 2.0 {
            let ax = if hv.x.abs() > hv.z.abs() { vec3(hv.x.signum(), 0.0, 0.0) } else { vec3(0.0, 0.0, hv.z.signum()) };
            if hv.normalize().dot(ax) > 0.8 && ledge(w, xi, yi, zi, ax.x != 0.0) {
                self.grind = Some(Grind { axis: ax, y: yi as f32 });
                self.heading = ax.x.atan2(ax.z);
                self.add("50-50 GRIND", 50.0);
                return;
            }
        }
        self.on_ground = true;
    }

    fn update_grind(&mut self, w: &World, dt: f32) {
        let Some(g) = self.grind.as_ref() else { return };
        let (axis, gy) = (g.axis, g.y);
        self.speed = (self.speed - dt).max(2.0);
        self.add("50-50 GRIND", dt * 150.0);
        self.sparks = true;
        let np = self.pos + axis * self.speed * dt;
        let c = if axis.x != 0.0 { vec3(np.x, gy, np.z.floor() + 0.5) } else { vec3(np.x.floor() + 0.5, gy, np.z) };
        if ledge(w, c.x.floor() as i32, gy as i32, c.z.floor() as i32, axis.x != 0.0) && !blocked(w, c) {
            self.pos = c;
        } else {
            self.grind = None;
            self.on_ground = false;
            self.vel = axis * self.speed + Vec3::Y * 2.0;
            self.air_t = 0.0;
            self.spin = 0.0;
        }
    }

    pub fn draw(&self, b: &mut Batch, shirt: Color, time: f32) {
        let up = Vec3::Y;
        let f = dir(self.heading);
        let s = vec3(-f.z, 0.0, f.x);
        let (flip, shuv) = self.trick.as_ref().map(|t| t.angles()).unwrap_or((0.0, 0.0));
        let flipping = self.trick.as_ref().map(|t| (t.t / t.dur).min(1.0)).unwrap_or(1.0) < 1.0;
        let lift = if flipping { 0.2 } else { 0.0 };

        // Shape (no bail sai voando e girando)
        let (bpos, brot) = if self.bail > 0.0 {
            let k = (2.0 - self.bail).min(0.8);
            (self.pos + self.bail_dir * k * 3.0 + up * (k * (0.8 - k) * 4.0), Quat::from_rotation_y(self.heading) * Quat::from_rotation_z(time * 9.0))
        } else {
            (self.pos + up * (0.1 + lift), Quat::from_rotation_y(self.heading + shuv + self.slide) * Quat::from_rotation_x(-self.manual * 0.3) * Quat::from_rotation_z(flip))
        };
        let bm = Mat4::from_rotation_translation(brot, bpos);
        let black = rgb(0.07, 0.07, 0.08);
        b.cube(&bm, vec3(0.0, 0.05, 0.0), vec3(0.24, 0.04, 0.62), black);
        b.cube(&bm, vec3(0.0, 0.028, 0.0), vec3(0.236, 0.02, 0.6), rgb(0.1, 0.6, 0.25));
        b.cube(&bm, vec3(0.0, 0.022, 0.0), vec3(0.12, 0.02, 0.28), rgb(1.0, 0.85, 0.1));
        for e in [-1.0f32, 1.0] {
            let tm = bm * Mat4::from_translation(vec3(0.0, 0.05, e * 0.31)) * Mat4::from_rotation_x(-e * 0.4);
            b.cube(&tm, vec3(0.0, 0.0, e * 0.06), vec3(0.24, 0.04, 0.13), black);
            b.cube(&bm, vec3(0.0, 0.0, e * 0.21), vec3(0.18, 0.04, 0.05), rgb(0.7, 0.7, 0.75));
            for sx in [-0.1f32, 0.1] {
                b.cube(&bm, vec3(sx, -0.04, e * 0.21), vec3(0.05, 0.065, 0.065), rgb(0.95, 0.95, 0.9));
            }
        }

        // Corpo: base (f ao longo do shape, s = peito, u = cima). No bail tomba pro lado.
        let (s, u) = if self.bail > 0.0 {
            let k = ((2.0 - self.bail) / 0.35).min(1.0) * PI * 0.5;
            (s * k.cos() + up * k.sin(), up * k.cos() - s * k.sin())
        } else {
            (s, up)
        };
        let f = if self.slide > 0.01 { dir(self.heading + self.slide * 0.8) } else { f };
        let o = self.pos;
        let jeans = rgb(0.2, 0.3, 0.55);
        let skin = rgb(0.85, 0.65, 0.5);
        let c = self.crouch;
        let feet_h = 0.14 + lift + if flipping { 0.12 } else { 0.0 };
        let front = o + f * 0.24 + u * feet_h;
        let mut back = o - f * 0.22 + u * feet_h;
        if self.push_t > 0.0 && self.on_ground {
            let ph = 1.0 - self.push_t / 0.6;
            back = back.lerp(o + s * 0.22 - f * (0.15 + 0.45 * ph) + u * 0.03, (ph * PI).sin());
        }
        let hip = o + u * (0.98 - c * 0.33 - self.manual * 0.05) - s * (0.05 * c) - f * (self.manual * 0.12);
        for (h, foot) in [(hip + f * 0.1, front), (hip - f * 0.1, back)] {
            let (knee, end) = ik(h, foot, 0.5, 0.5, s + u * 0.2);
            limb(b, h, knee, 0.15, jeans);
            limb(b, knee, end, 0.13, jeans);
            b.cube(&Mat4::from_translation(end), Vec3::ZERO, vec3(0.17, 0.09, 0.17), rgb(0.95, 0.95, 0.95));
        }
        let chest = hip + u * 0.55 + s * (0.16 * c);
        limb(b, hip, chest, 0.32, shirt);
        let head = chest + u * 0.26 + s * 0.04;
        b.cube(&Mat4::from_translation(head), Vec3::ZERO, Vec3::splat(0.27), skin);
        b.cube(&Mat4::from_translation(head + u * 0.13), Vec3::ZERO, vec3(0.29, 0.12, 0.29), rgb(0.85, 0.15, 0.15));
        for e in [-1.0f32, 1.0] {
            b.cube(&Mat4::from_translation(head + s * 0.14 + f * (e * 0.06) + u * 0.02), Vec3::ZERO, Vec3::splat(0.04), black);
        }
        let grab_pt = bpos + s * 0.13 + u * 0.05;
        for (i, e) in [1.0f32, -1.0].into_iter().enumerate() {
            let sh = chest + f * (e * 0.18) - u * 0.04;
            let sway = (time * 2.0 + i as f32).sin() * 0.08;
            let hand = if self.grab > 0.3 && i == 0 { grab_pt } else { sh + f * (e * 0.42) - u * (0.2 - sway) + s * 0.05 };
            let (el, hd) = ik(sh, hand, 0.32, 0.32, -u + f * e * 0.3);
            limb(b, sh, el, 0.1, shirt);
            limb(b, el, hd, 0.09, skin);
        }
    }
}
