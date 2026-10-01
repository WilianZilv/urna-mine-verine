//! Skatista estilo Skate 3 ("flick-it"). Mouse (ou arrastar no celular) = analógico direito.
//!
//! Mecânicas modeladas a partir do design documentado pelo projeto de engenharia reversa
//! SK8-ENGINE/skate-3-rust-engine (https://github.com/SK8-ENGINE/skate-3-rust-engine):
//! - gestos: o analógico é amostrado a 60 Hz contra padrões de waypoints ordenados com raio de
//!   tolerância e orçamento de "misses"; segurar o 1º ponto não envelhece o gesto; a força do
//!   flick (ticks gastos / nº de pontos) define altura do pop e velocidade do flip;
//! - remada: ganho de velocidade por remada limitado por um blend entre limite de baixa e alta
//!   velocidade, sem passar da velocidade máxima "remável"; curva fica mais dura remando;
//! - curva: entrada amortecida, escalada por curva de velocidade, remada e manual;
//! - manual/grind: equilíbrio instável com ruído procedural que cresce com velocidade/tempo;
//! - aterrissagem: qualidade pelo desalinhamento shape × velocidade e giro (limpa / sketchy / bail).
//!
//! Aquele repositório não tem licença (todos os direitos reservados) e deriva de código da EA,
//! então nada foi copiado: tudo aqui é reimplementação própria em cima dos voxels.

use crate::batch::Batch;
use crate::models::rgb;
use crate::urna::{ik, limb};
use crate::world::World;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

const GRAV: f32 = 24.0;
const POP_MIN: f32 = 5.8;
const POP_MAX: f32 = 8.2;
const MAX_PUSH: f32 = 9.0;
const PUSH_LOW: f32 = 2.4;
const PUSH_HIGH: f32 = 0.8;
const MAX_SPEED: f32 = 16.0;
const TICK: f32 = 1.0 / 60.0;
const TOL2: f32 = 0.42 * 0.42;
const MISSES: u8 = 7;
const MOUSE_K: f32 = 0.013;

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

// ------------------------------------------------------------------ Gestos (flick-it)

const D: [f32; 2] = [0.0, -1.0];
const U: [f32; 2] = [0.0, 1.0];
const L: [f32; 2] = [-1.0, 0.0];
const R: [f32; 2] = [1.0, 0.0];
const UL: [f32; 2] = [-0.71, 0.71];
const UR: [f32; 2] = [0.71, 0.71];
const DL: [f32; 2] = [-0.71, -0.71];
const DR: [f32; 2] = [0.71, -0.71];

/// (nome, waypoints do analógico, voltas de flip, meias-voltas de shove, pontos). Versões nollie
/// saem espelhando Y.
const TAIL: [(&str, &[[f32; 2]], f32, f32, f32); 11] = [
    ("OLLIE", &[D, U], 0.0, 0.0, 50.0),
    ("KICKFLIP", &[D, UL], -1.0, 0.0, 150.0),
    ("HEELFLIP", &[D, UR], 1.0, 0.0, 150.0),
    ("POP SHOVE-IT", &[D, L], 0.0, 1.0, 100.0),
    ("FS SHOVE-IT", &[D, R], 0.0, -1.0, 100.0),
    ("360 SHOVE-IT", &[D, L, UL, U], 0.0, 2.0, 220.0),
    ("FS 360 SHOVE-IT", &[D, R, UR, U], 0.0, -2.0, 220.0),
    ("VARIAL KICKFLIP", &[D, L, UL], -1.0, 1.0, 260.0),
    ("VARIAL HEELFLIP", &[D, R, UR], 1.0, -1.0, 260.0),
    ("360 FLIP", &[L, DL, D, UR], -1.0, 2.0, 400.0),
    ("LASER FLIP", &[R, DR, D, UL], 1.0, -2.0, 420.0),
];

struct Pattern {
    name: String,
    pts: Vec<Vec2>,
    flip: f32,
    shuv: f32,
    score: f32,
    nollie: bool,
}

#[derive(Clone, Copy, Default)]
struct Node {
    active: bool,
    done: bool,
    next: usize,
    elapsed: u16,
    misses: u8,
    dist: f32,
}

impl Node {
    fn tick(&mut self, pts: &[Vec2], s: Vec2) {
        if !self.active {
            let d = pts[0].distance_squared(s);
            if d <= TOL2 {
                *self = Node { active: true, next: 1, elapsed: 1, dist: d, ..Node::default() };
            }
            return;
        }
        if self.done {
            return;
        }
        let d = pts[self.next].distance_squared(s);
        if d <= TOL2 {
            self.dist += d;
            self.elapsed += 1;
            self.misses = 0;
            self.next += 1;
            self.done = self.next == pts.len();
        } else if self.next != 1 || pts[0].distance_squared(s) > TOL2 {
            self.elapsed += 1;
            self.misses += 1;
            if self.misses > MISSES {
                *self = Node::default();
            }
        }
    }

    fn score(&self, n: usize) -> f32 {
        let n = n as f32;
        let mean = (self.dist / n).clamp(0.02, TOL2);
        n.powi(4) / (mean * self.elapsed as f32)
    }
}

struct Gestures {
    pats: Vec<Pattern>,
    nodes: Vec<Node>,
    wait: u8,
}

impl Gestures {
    fn new() -> Self {
        let mut pats = Vec::new();
        for nollie in [false, true] {
            for (name, pts, flip, shuv, score) in TAIL {
                let name = match (nollie, name) {
                    (false, _) => name.to_string(),
                    (true, "OLLIE") => "NOLLIE".to_string(),
                    (true, _) => format!("NOLLIE {name}"),
                };
                let pts = pts.iter().map(|p| vec2(p[0], if nollie { -p[1] } else { p[1] })).collect();
                pats.push(Pattern { name, pts, flip, shuv, score: score * if nollie { 1.2 } else { 1.0 }, nollie });
            }
        }
        let nodes = vec![Node::default(); pats.len()];
        Gestures { pats, nodes, wait: 0 }
    }

    fn reset(&mut self) {
        self.nodes.fill(Node::default());
        self.wait = 0;
    }

    /// Uma amostra a 60 Hz. Retorna (padrão, força 0..1). Se um padrão mais longo que compartilha
    /// o prefixo ainda está vivo, espera alguns ticks antes de decidir (shove-it × 360 shove-it).
    fn sample(&mut self, s: Vec2) -> Option<(usize, f32)> {
        for (n, p) in self.nodes.iter_mut().zip(&self.pats) {
            n.tick(&p.pts, s);
        }
        let mut best: Option<usize> = None;
        for (i, n) in self.nodes.iter().enumerate() {
            if n.done && best.is_none_or(|b| n.score(self.pats[i].pts.len()) > self.nodes[b].score(self.pats[b].pts.len())) {
                best = Some(i);
            }
        }
        let best = best?;
        let len = self.pats[best].pts.len();
        let longer = self.nodes.iter().zip(&self.pats).any(|(n, p)| n.active && !n.done && p.pts.len() > len && n.next >= len);
        if longer && s.length() > 0.6 && self.wait < 8 {
            self.wait += 1;
            return None;
        }
        let ratio = self.nodes[best].elapsed as f32 / len as f32;
        let strength = ((6.0 - ratio) / 4.0).clamp(0.0, 1.0);
        self.reset();
        Some((best, strength))
    }
}

// ------------------------------------------------------------------ Estado

struct Trick {
    flip: f32,
    shuv: f32,
    t: f32,
    dur: f32,
}

impl Trick {
    fn k(&self) -> f32 {
        (self.t / self.dur).min(1.0)
    }
    fn angles(&self) -> (f32, f32) {
        let e = 1.0 - (1.0 - self.k()).powi(2);
        (self.flip * TAU * e, self.shuv * PI * e)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum GrindKind {
    FiftyFifty,
    FiveO,
    Nose,
    Board,
}

impl GrindKind {
    fn name(self) -> &'static str {
        match self {
            GrindKind::FiftyFifty => "50-50 GRIND",
            GrindKind::FiveO => "5-0 GRIND",
            GrindKind::Nose => "NOSEGRIND",
            GrindKind::Board => "BOARDSLIDE",
        }
    }
}

struct Grind {
    axis: Vec3,
    along_x: bool,
    line: f32,
    y: f32,
    kind: GrindKind,
    bal: f32,
    bal_v: f32,
    t: f32,
}

struct Manual {
    nose: bool,
    bal: f32,
    bal_v: f32,
}

pub struct Skater {
    pub pos: Vec3,
    /// Direção de deslocamento no chão.
    pub heading: f32,
    /// Direção do nose do shape (heading + PI quando fakie; gira livre no ar).
    yaw: f32,
    fakie: bool,
    speed: f32,
    vel: Vec3,
    on_ground: bool,
    rs: Vec2,
    rs_idle: f32,
    gest: Gestures,
    tick_acc: f32,
    trick: Option<Trick>,
    pop_t: f32,
    pop_sign: f32,
    turn: f32,
    lean: f32,
    push_p: f32,
    push_dur: f32,
    push_dv: f32,
    push_scalar: f32,
    idle_t: f32,
    slide: f32,
    brake_amt: f32,
    manual: Option<Manual>,
    manual_lock: bool,
    held_manual: bool,
    noise: f32,
    noise_goal: f32,
    noise_t: f32,
    grab: f32,
    grab_name: &'static str,
    spin: f32,
    spin_v: f32,
    air_t: f32,
    grind: Option<Grind>,
    bail: f32,
    bail_dir: Vec3,
    bail_spd: f32,
    board_pos: Vec3,
    board_vel: Vec3,
    board_rot: Quat,
    board_spin: Vec3,
    sketch: f32,
    crouch: f32,
    crouch_v: f32,
    pitch: f32,
    arms: f32,
    cam_yaw: f32,
    cam_y: f32,
    cam_len: f32,
    cam_eye: Vec3,
    cam_at: Vec3,
    cam_init: bool,
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

/// Linha de grind sob `p` ao longo de X (ou Z): topo de muro de 1 bloco (centro) ou quina de
/// plataforma (borda). Retorna a coordenada perpendicular da linha.
fn grind_line(w: &World, p: Vec3, along_x: bool) -> Option<f32> {
    let (x, y, z) = (p.x.floor() as i32, p.y.round() as i32, p.z.floor() as i32);
    if !w.solid(x, y - 1, z) || w.solid(x, y, z) || w.solid(x, y + 1, z) {
        return None;
    }
    let (dx, dz) = if along_x { (0, 1) } else { (1, 0) };
    let plus = !w.solid(x + dx, y - 1, z + dz);
    let minus = !w.solid(x - dx, y - 1, z - dz);
    let base = if along_x { z as f32 } else { x as f32 };
    let line = match (plus, minus) {
        (true, true) => base + 0.5,
        (true, false) => base + 1.0,
        (false, true) => base,
        _ => return None,
    };
    let perp = if along_x { p.z } else { p.x };
    ((perp - line).abs() < 0.5).then_some(line)
}

/// Matriz com eixo Z = `fwd`, Y ≈ `up`.
fn frame(fwd: Vec3, up: Vec3, pos: Vec3) -> Mat4 {
    let z = fwd.normalize_or(Vec3::Z);
    let x = up.cross(z).normalize_or(Vec3::X);
    let y = z.cross(x);
    Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), pos.extend(1.0))
}

impl Skater {
    pub fn new(pos: Vec3, heading: f32) -> Self {
        Skater {
            pos,
            heading,
            yaw: heading,
            fakie: false,
            speed: 0.0,
            vel: Vec3::ZERO,
            on_ground: false,
            rs: Vec2::ZERO,
            rs_idle: 0.0,
            gest: Gestures::new(),
            tick_acc: 0.0,
            trick: None,
            pop_t: 10.0,
            pop_sign: 1.0,
            turn: 0.0,
            lean: 0.0,
            push_p: 0.0,
            push_dur: 0.6,
            push_dv: 0.0,
            push_scalar: 1.0,
            idle_t: 0.0,
            slide: 0.0,
            brake_amt: 0.0,
            manual: None,
            manual_lock: false,
            held_manual: false,
            noise: 0.0,
            noise_goal: 0.0,
            noise_t: 0.0,
            grab: 0.0,
            grab_name: "INDY",
            spin: 0.0,
            spin_v: 0.0,
            air_t: 0.0,
            grind: None,
            bail: 0.0,
            bail_dir: Vec3::X,
            bail_spd: 0.0,
            board_pos: pos,
            board_vel: Vec3::ZERO,
            board_rot: Quat::IDENTITY,
            board_spin: Vec3::ZERO,
            sketch: 0.0,
            crouch: 0.18,
            crouch_v: 0.0,
            pitch: 0.0,
            arms: 0.25,
            cam_yaw: heading,
            cam_y: pos.y,
            cam_len: 3.5,
            cam_eye: pos + Vec3::Y * 1.4 - dir(heading) * 3.5,
            cam_at: pos + Vec3::Y * 0.8 + dir(heading) * 2.0,
            cam_init: false,
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
        (self.cam_eye, self.cam_at)
    }

    /// Combo em andamento ("KICKFLIP + BS 180 + 50-50 GRIND").
    pub fn combo_text(&self) -> String {
        self.combo.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(" + ")
    }

    fn nose_yaw(&self) -> f32 {
        self.heading + if self.fakie { PI } else { 0.0 }
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
        let hv = if self.on_ground || self.grind.is_some() {
            dir(self.heading) * self.speed
        } else {
            vec3(self.vel.x, 0.0, self.vel.z)
        };
        self.bail_dir = hv.normalize_or(dir(self.heading));
        self.bail_spd = hv.length();
        self.board_pos = self.pos + Vec3::Y * 0.1;
        self.board_vel = hv * 1.1 + vec3(gen_range(-1.5, 1.5), gen_range(2.5, 4.5), gen_range(-1.5, 1.5));
        self.board_rot = Quat::from_rotation_y(self.yaw);
        self.board_spin = vec3(gen_range(-14.0, 14.0), gen_range(-8.0, 8.0), gen_range(-14.0, 14.0));
        self.vel = hv * 0.7 + Vec3::Y * self.vel.y.max(1.5);
        self.popup = Some((format!("BAIL! {why}"), 2.0));
        self.combo.clear();
        self.trick = None;
        self.grind = None;
        self.manual = None;
        self.grab = 0.0;
        self.slide = 0.0;
        self.push_p = 0.0;
        self.on_ground = false;
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
        let dt = dt.min(0.1);
        if let Some((_, t)) = self.popup.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.popup = None;
            }
        }
        self.held_manual = inp.manual;
        // Analógico direito virtual: toque é absoluto; mouse acumula e volta ao centro parado.
        match inp.touch {
            Some(t) => {
                self.rs = t.clamp_length_max(1.0);
                self.rs_idle = 0.0;
            }
            None => {
                let m = vec2(inp.mouse.x, -inp.mouse.y);
                if m.length_squared() > 0.25 {
                    self.rs = (self.rs + m * MOUSE_K).clamp_length_max(1.0);
                    self.rs_idle = 0.0;
                } else {
                    self.rs_idle += dt;
                    if self.rs_idle > 0.1 {
                        self.rs *= (-dt * 6.0).exp();
                    }
                }
            }
        }
        self.noise_t -= dt;
        if self.noise_t <= 0.0 {
            self.noise_t = gen_range(0.25, 0.6);
            self.noise_goal = gen_range(-1.0, 1.0);
        }
        self.noise += (self.noise_goal - self.noise) * (dt * 3.0).min(1.0);
        if self.pos.y < -20.0 {
            self.pos = crate::player::Player::spawn();
            self.vel = Vec3::ZERO;
            self.speed = 0.0;
            self.grind = None;
            self.on_ground = false;
        }

        if self.bail > 0.0 {
            self.update_bail(w, dt);
            self.update_camera(w, dt);
            return;
        }

        let can_pop = self.on_ground || self.grind.is_some();
        let mut pop: Option<(usize, f32)> = None;
        self.tick_acc += dt;
        while self.tick_acc >= TICK {
            self.tick_acc -= TICK;
            if let Some(r) = self.gest.sample(self.rs) {
                if can_pop && pop.is_none() {
                    pop = Some(r);
                }
            }
        }
        if let Some((i, s)) = pop {
            let p = &self.gest.pats[i];
            let (name, flip, shuv, score, nollie) = (p.name.clone(), p.flip, p.shuv, p.score, p.nollie);
            self.pop(name, flip, shuv, score, s, nollie);
        } else if inp.ollie && can_pop {
            self.pop("OLLIE".into(), 0.0, 0.0, 50.0, 1.0, false);
        }

        let n = (dt / TICK).ceil().max(1.0) as usize;
        let h = dt / n as f32;
        for _ in 0..n {
            if self.bail > 0.0 {
                break;
            }
            if self.grind.is_some() {
                self.update_grind(w, h, inp);
            } else if self.on_ground {
                self.update_ground(w, h, inp);
            } else {
                self.update_air(w, h, inp);
            }
        }
        self.animate(dt, inp);
        self.update_camera(w, dt);
    }

    fn pop(&mut self, name: String, flip: f32, shuv: f32, score: f32, strength: f32, nollie: bool) {
        let (hv, mult) = match self.grind.take() {
            Some(g) => (g.axis * self.speed, 0.85),
            None => (dir(self.heading) * self.speed, if self.manual.is_some() { 1.05 } else { 1.0 }),
        };
        self.yaw = self.nose_yaw();
        self.vel = hv + Vec3::Y * (POP_MIN + (POP_MAX - POP_MIN) * strength) * mult;
        self.on_ground = false;
        self.air_t = 0.0;
        self.spin = 0.0;
        self.spin_v = self.turn * 3.5;
        self.manual = None;
        self.slide = 0.0;
        self.push_p = 0.0;
        self.pop_t = 0.0;
        self.pop_sign = if nollie { -1.0 } else { 1.0 };
        self.trick = (flip != 0.0 || shuv != 0.0).then(|| Trick {
            flip,
            shuv,
            t: 0.0,
            dur: (0.36 + 0.05 * (flip.abs() + shuv.abs())) / (0.8 + 0.3 * strength),
        });
        let name = if self.fakie && !nollie { format!("FAKIE {name}") } else { name };
        self.add(&name, score * (0.7 + 0.3 * strength));
        self.crouch_v -= 7.0;
        self.gest.reset();
    }

    fn update_ground(&mut self, w: &World, dt: f32, inp: &Input) {
        // Manual: equilíbrio instável; W/S ou analógico pra cima/baixo compensam.
        let want_manual = inp.manual && self.speed > 1.0;
        if !inp.manual {
            self.manual_lock = false;
        }
        if want_manual && self.manual.is_none() && !self.manual_lock {
            self.manual = Some(Manual { nose: self.rs.y > 0.3, bal: gen_range(-0.1, 0.1), bal_v: 0.0 });
            self.push_p = 0.0;
        }
        if !want_manual {
            self.manual = None;
        }
        if let Some(mut m) = self.manual.take() {
            let ctrl = (inp.push as i32 - inp.brake as i32) as f32 + self.rs.y * 1.5;
            let ctrl = if m.nose { -ctrl } else { ctrl };
            let acc = 2.2 * m.bal + self.noise * (1.2 + self.speed * 0.1) - ctrl * 4.0 - 1.8 * m.bal_v;
            m.bal_v += acc * dt;
            m.bal += m.bal_v * dt;
            if m.bal.abs() > 1.0 {
                self.manual_lock = true;
                self.speed *= 0.85;
            } else {
                let name = if m.nose { "NOSE MANUAL" } else { "MANUAL" };
                self.manual = Some(m);
                self.add(name, dt * 120.0);
            }
        }
        let manual = self.manual.is_some();

        // Remada: ganho aplicado na fase em que o pé empurra o chão.
        if self.push_p > 0.0 {
            let p0 = self.push_p;
            self.push_p += dt / self.push_dur;
            let overlap = (self.push_p.min(0.7) - p0.max(0.25)).max(0.0);
            self.speed += self.push_dv * overlap / 0.45;
            if self.push_p >= 1.0 {
                self.push_p = 0.0;
            }
        }
        let pushing = self.push_p > 0.25 && self.push_p < 0.7;
        if inp.push && !manual && self.push_p == 0.0 && !inp.brake {
            let frac = (self.speed / MAX_PUSH).clamp(0.0, 1.0);
            self.push_dv = (PUSH_LOW + (PUSH_HIGH - PUSH_LOW) * frac).min((MAX_PUSH - self.speed).max(0.0));
            self.push_dur = 0.62 - 0.17 * frac;
            self.push_p = 1e-4;
        }

        // Modelo de velocidade: rolamento + arrasto, atrito extra sem input / em manual, freio.
        let any_input = inp.push || inp.brake || inp.steer != 0.0 || manual;
        self.idle_t = if any_input { 0.0 } else { self.idle_t + dt };
        let mut decel = 0.2 + 0.004 * self.speed * self.speed;
        if self.idle_t > 2.0 {
            decel += 0.35;
        }
        if manual {
            decel += 0.3;
        }
        let sliding = inp.brake && !manual && self.speed > 1.5 && (self.speed > 5.0 || self.slide.abs() > 0.3);
        let side = if self.slide.abs() > 0.05 {
            self.slide.signum()
        } else if inp.steer != 0.0 {
            inp.steer.signum()
        } else {
            1.0
        };
        self.slide += ((if sliding { 1.35 * side } else { 0.0 }) - self.slide) * (dt * 8.0).min(1.0);
        if sliding {
            decel += 7.0;
            self.add("POWERSLIDE", dt * 30.0);
        } else if inp.brake && !manual {
            decel += 4.5;
        }
        self.speed = (self.speed - decel * dt).clamp(0.0, MAX_SPEED);

        // Curva: entrada amortecida × curva de velocidade × remada × manual.
        self.turn += (inp.steer - self.turn) * (1.0 - (-dt * 9.0).exp());
        self.push_scalar = if pushing { (self.push_scalar - dt * 3.0).max(0.45) } else { (self.push_scalar + dt * 2.0).min(1.0) };
        let v = self.speed;
        let rate = if v < 0.4 { 1.6 } else { (v / 1.3).clamp(1.2, 2.7) * (1.0 - 0.4 * (v / MAX_SPEED).min(1.0)) };
        let scal = self.push_scalar * if manual { 0.6 } else { 1.0 } * if sliding { 0.3 } else { 1.0 };
        self.heading += self.turn * rate * scal * dt;

        let fwd = dir(self.heading);
        let mut vel = fwd * self.speed;
        let lost = self.step(w, &mut vel, dt);
        if lost > 7.5 {
            self.bail_now("BATEU");
            return;
        }
        self.speed = vel.dot(fwd).max(0.0);
        self.yaw = self.nose_yaw();
        let f = w.floor_at(self.pos.x, self.pos.y + 0.3, self.pos.z);
        if f < self.pos.y - 0.05 {
            self.on_ground = false;
            self.vel = fwd * self.speed;
            self.air_t = 0.0;
            self.spin = 0.0;
            self.spin_v = 0.0;
            self.manual = None;
            self.push_p = 0.0;
            self.pop_t = 10.0;
            return;
        }
        self.pos.y = f;
        if !manual && !self.combo.is_empty() {
            self.land_t += dt;
            if self.land_t > 0.9 {
                self.bank();
            }
        }
    }

    fn update_air(&mut self, w: &World, dt: f32, inp: &Input) {
        self.air_t += dt;
        self.pop_t += dt;
        self.vel.y -= GRAV * dt;
        self.spin_v += (inp.steer * 7.5 - self.spin_v) * (dt * 4.0).min(1.0);
        self.yaw += self.spin_v * dt;
        self.spin += self.spin_v * dt;
        let flipping = self.trick.as_ref().is_some_and(|t| t.k() < 1.0);
        let grabbing = inp.grab && self.air_t > 0.12 && !flipping;
        if grabbing && self.grab < 0.05 {
            let r = self.rs;
            self.grab_name = if r.length() < 0.35 {
                "INDY"
            } else if r.y.abs() > r.x.abs() {
                if r.y > 0.0 { "NOSEGRAB" } else { "TAILGRAB" }
            } else if r.x < 0.0 {
                "MELON"
            } else {
                "STALEFISH"
            };
        }
        self.grab += ((grabbing as i32 as f32) - self.grab) * (dt * 10.0).min(1.0);
        if grabbing {
            let name = self.grab_name;
            self.add(name, dt * 200.0);
        }
        if let Some(t) = self.trick.as_mut() {
            t.t += dt;
        }
        let mut hv = vec3(self.vel.x, 0.0, self.vel.z);
        if self.step(w, &mut hv, dt) > 9.0 {
            self.bail_now("BATEU NO AR");
            return;
        }
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

    fn spin_points(&mut self) {
        let n = (self.spin.abs() / PI).round() as i32;
        if n > 0 {
            let name = format!("{} {}", if self.spin > 0.0 { "BS" } else { "FS" }, n * 180);
            self.add(&name, n as f32 * 100.0);
        }
        self.spin = 0.0;
        self.spin_v = 0.0;
    }

    fn land(&mut self, w: &World) {
        let hv = vec3(self.vel.x, 0.0, self.vel.z);
        let spd = hv.length();
        let vy = self.vel.y;
        self.pop_t = 10.0;
        if vy < -21.0 {
            return self.bail_now("CAIU DE MUITO ALTO");
        }
        if self.trick.as_ref().is_some_and(|t| t.k() < 0.8) {
            return self.bail_now("NAO PEGOU O SHAPE");
        }
        if self.grab > 0.75 {
            return self.bail_now("NAO SOLTOU O GRAB");
        }
        self.trick = None;
        self.grab = 0.0;
        self.gest.reset();
        if spd > 2.0 && self.try_grind(w, hv) {
            self.spin_points();
            return;
        }
        // Qualidade: desalinhamento do shape com a velocidade (vale qualquer ponta = fakie).
        let vh = if spd > 1.5 { hv.x.atan2(hv.z) } else { self.yaw };
        let d = angle_diff(self.yaw, vh);
        let (fakie, mis) = if d.abs() <= FRAC_PI_2 { (false, d.abs()) } else { (true, PI - d.abs()) };
        if mis > 0.75 {
            return self.bail_now("CAIU DE LADO");
        }
        self.spin_points();
        self.fakie = if spd > 1.5 { fakie } else { false };
        self.heading = if spd > 1.5 { vh } else { self.yaw };
        self.speed = spd;
        if mis > 0.3 {
            self.sketch = 1.0;
            self.speed *= 0.75;
            if let Some(last) = self.combo.last_mut() {
                last.1 *= 0.7;
            }
        }
        self.crouch_v += (-vy / 14.0).clamp(0.2, 1.0) * 7.0;
        self.vel = Vec3::ZERO;
        self.on_ground = true;
        self.landed = true;
        self.land_t = 0.0;
        self.yaw = self.nose_yaw();
    }

    fn try_grind(&mut self, w: &World, hv: Vec3) -> bool {
        let along_x = hv.x.abs() > hv.z.abs();
        let axis = if along_x { vec3(hv.x.signum(), 0.0, 0.0) } else { vec3(0.0, 0.0, hv.z.signum()) };
        if hv.normalize().dot(axis) < 0.75 {
            return false;
        }
        let Some(line) = grind_line(w, self.pos, along_x) else { return false };
        let ah = axis.x.atan2(axis.z);
        let d = angle_diff(self.yaw, ah);
        let m = d.abs().min(PI - d.abs());
        let kind = if m > 0.8 {
            GrindKind::Board
        } else if self.held_manual {
            if self.rs.y > 0.3 { GrindKind::Nose } else { GrindKind::FiveO }
        } else {
            GrindKind::FiftyFifty
        };
        self.heading = ah;
        if kind != GrindKind::Board {
            self.fakie = d.abs() > FRAC_PI_2;
        }
        self.yaw = if kind == GrindKind::Board { ah + FRAC_PI_2 * d.signum() } else { self.nose_yaw() };
        self.speed = hv.length();
        let y = self.pos.y.round();
        if along_x {
            self.pos.z = line;
        } else {
            self.pos.x = line;
        }
        self.pos.y = y;
        self.grind = Some(Grind { axis, along_x, line, y, kind, bal: gen_range(-0.15, 0.15), bal_v: 0.0, t: 0.0 });
        self.add(kind.name(), 50.0);
        self.vel = Vec3::ZERO;
        self.on_ground = false;
        self.landed = true;
        self.crouch_v += 3.0;
        true
    }

    fn update_grind(&mut self, w: &World, dt: f32, inp: &Input) {
        let Some(mut g) = self.grind.take() else { return };
        g.t += dt;
        self.speed = (self.speed - if g.kind == GrindKind::Board { 2.2 } else { 1.1 } * dt).max(0.0);
        let acc = 1.6 * g.bal + self.noise * (0.9 + g.t * 0.2) + 3.5 * inp.steer - 1.5 * g.bal_v;
        g.bal_v += acc * dt;
        g.bal += g.bal_v * dt;
        if g.bal.abs() > 1.0 {
            return self.bail_now("PERDEU O EQUILIBRIO");
        }
        if self.speed < 1.0 {
            return self.exit_grind(&g, 0.5);
        }
        self.add(g.kind.name(), dt * if g.kind == GrindKind::Board { 170.0 } else { 150.0 });
        self.sparks = true;
        let mut np = self.pos + g.axis * self.speed * dt;
        np.y = g.y;
        match grind_line(w, np, g.along_x) {
            Some(line) if !blocked(w, np) => {
                g.line += (line - g.line) * (dt * 10.0).min(1.0);
                if g.along_x {
                    np.z = g.line;
                } else {
                    np.x = g.line;
                }
                self.pos = np;
                self.grind = Some(g);
            }
            _ if blocked(w, np) => {
                if self.speed > 5.0 {
                    self.bail_now("BATEU");
                } else {
                    self.speed = 0.0;
                    self.exit_grind(&g, 0.0);
                }
            }
            _ => self.exit_grind(&g, 1.5),
        }
    }

    fn exit_grind(&mut self, g: &Grind, up: f32) {
        self.grind = None;
        self.vel = g.axis * self.speed + Vec3::Y * up;
        self.on_ground = false;
        self.air_t = 0.0;
        self.spin = 0.0;
        self.spin_v = 0.0;
        self.pop_t = 10.0;
        self.trick = None;
        self.yaw = self.nose_yaw();
    }

    fn update_bail(&mut self, w: &World, dt: f32) {
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

        self.board_vel.y -= GRAV * dt;
        let mut np = self.board_pos + self.board_vel * dt;
        if w.solid_f(np.x, self.board_pos.y + 0.1, np.z) {
            self.board_vel.x *= -0.3;
            self.board_vel.z *= -0.3;
            np = vec3(self.board_pos.x, np.y, self.board_pos.z);
        }
        let bf = w.floor_at(np.x, self.board_pos.y + 0.3, np.z);
        if np.y < bf {
            np.y = bf;
            self.board_vel.y = -self.board_vel.y * 0.35;
            self.board_vel.x *= 0.7;
            self.board_vel.z *= 0.7;
            self.board_spin *= 0.6;
        }
        self.board_pos = np;
        self.board_rot = (Quat::from_scaled_axis(self.board_spin * dt) * self.board_rot).normalize();

        if self.bail <= 0.0 {
            self.bail = 0.0;
            self.vel = Vec3::ZERO;
            self.speed = 0.0;
            self.spin = 0.0;
            self.spin_v = 0.0;
            self.fakie = false;
            self.heading = self.bail_dir.x.atan2(self.bail_dir.z);
            self.yaw = self.heading;
            self.on_ground = ny <= f + 0.01;
            self.crouch = 0.6;
            self.gest.reset();
        }
    }

    fn animate(&mut self, dt: f32, inp: &Input) {
        let airborne = !self.on_ground && self.grind.is_none();
        let rolling = self.on_ground || self.grind.is_some();
        let preload = rolling && self.rs.length() > 0.55;
        let target = if airborne {
            if self.trick.is_some() || self.grab > 0.1 {
                0.8
            } else if self.pop_t < 0.12 {
                -0.1
            } else {
                0.45
            }
        } else if preload {
            0.85
        } else if let Some(g) = &self.grind {
            0.45 + g.bal.abs() * 0.2
        } else if self.manual.is_some() {
            0.3
        } else if self.push_p > 0.0 {
            0.4
        } else if inp.brake {
            0.45
        } else {
            0.18 + self.turn.abs() * 0.15
        };
        self.crouch_v += ((target - self.crouch) * 140.0 - self.crouch_v * 18.0) * dt;
        self.crouch = (self.crouch + self.crouch_v * dt).clamp(-0.15, 1.05);

        let pitch = if airborne && self.pop_t < 0.45 {
            let p = self.pop_t;
            0.5 * self.pop_sign * (p / 0.05).min(1.0) * (1.0 - ((p - 0.08) / 0.3).clamp(0.0, 1.0))
        } else if let Some(m) = &self.manual {
            if m.nose { -0.3 + m.bal * 0.1 } else { 0.3 + m.bal * 0.1 }
        } else if let Some(g) = &self.grind {
            match g.kind {
                GrindKind::FiveO => 0.22,
                GrindKind::Nose => -0.22,
                _ => 0.0,
            }
        } else {
            0.0
        };
        self.pitch += (pitch - self.pitch) * (dt * 25.0).min(1.0);

        let lean = if self.on_ground { self.turn * (self.speed / 7.0).min(1.0) * 0.22 } else { 0.0 };
        self.lean += (lean - self.lean) * (dt * 10.0).min(1.0);
        let arms = if airborne {
            0.8
        } else if self.grind.is_some() || self.sketch > 0.0 {
            1.0
        } else if self.manual.is_some() {
            0.9
        } else {
            0.25 + self.turn.abs() * 0.3
        };
        self.arms += (arms - self.arms) * (dt * 6.0).min(1.0);
        let braking = inp.brake && self.on_ground && self.slide.abs() < 0.3 && self.manual.is_none() && self.speed > 0.2;
        self.brake_amt += ((braking as i32 as f32) - self.brake_amt) * (dt * 10.0).min(1.0);
        self.sketch = (self.sketch - dt * 1.5).max(0.0);
    }

    /// Câmera estilo Skate 3: baixa, atrás, segue a direção de deslocamento com atraso e
    /// acompanha a altura com preguiça no ar. Encurta quando tem parede atrás.
    fn update_camera(&mut self, w: &World, dt: f32) {
        let hv = vec2(self.vel.x, self.vel.z);
        let (travel, rate) = if self.bail > 0.0 {
            (self.bail_dir.x.atan2(self.bail_dir.z), 1.0)
        } else if let Some(g) = &self.grind {
            (g.axis.x.atan2(g.axis.z), 4.0)
        } else if self.on_ground {
            (self.heading, if self.speed < 0.3 { 1.5 } else { 3.5 })
        } else if hv.length() > 1.0 {
            (hv.x.atan2(hv.y), 1.6)
        } else {
            (self.cam_yaw, 1.0)
        };
        if !self.cam_init {
            self.cam_yaw = travel;
            self.cam_y = self.pos.y;
        }
        self.cam_yaw += angle_diff(travel, self.cam_yaw) * (dt * rate).min(1.0);
        let airborne = !self.on_ground && self.grind.is_none() && self.bail <= 0.0;
        self.cam_y += (self.pos.y - self.cam_y) * (dt * if airborne { 2.0 } else { 7.0 }).min(1.0);
        self.cam_y = self.cam_y.clamp(self.pos.y - 2.0, self.pos.y + 2.0);
        let d = dir(self.cam_yaw);
        let focus = vec3(self.pos.x, self.cam_y + 0.95, self.pos.z);
        let back = -d * (3.3 + self.speed.min(14.0) * 0.07) + Vec3::Y * 0.45;
        let want = back.length();
        let bn = back / want;
        let fit = w.raycast(focus, bn, want).map(|h| (h.2 - 0.25).max(0.35)).unwrap_or(want);
        self.cam_len = if !self.cam_init || fit < self.cam_len { fit } else { self.cam_len + (fit - self.cam_len) * (dt * 3.0).min(1.0) };
        self.cam_eye = focus + bn * self.cam_len;
        self.cam_at = focus + d * 2.0 + Vec3::Y * ((self.pos.y - self.cam_y) * 0.7 - 0.15);
        self.cam_init = true;
    }

    // ------------------------------------------------------------------ Desenho

    pub fn draw(&self, b: &mut Batch, shirt: Color, time: f32) {
        if self.bail > 0.0 {
            return self.draw_bail(b, shirt, time);
        }
        let up = Vec3::Y;
        let airborne = !self.on_ground && self.grind.is_none();
        let (flip, shuv, k) = self.trick.as_ref().map(|t| {
            let (a, s) = t.angles();
            (a, s, t.k())
        }).unwrap_or((0.0, 0.0, 1.0));
        let flipping = k < 1.0;
        let lift = if airborne { self.crouch.max(0.0) * 0.28 + if flipping { 0.08 } else { 0.0 } } else { 0.0 };
        let base = self.pos + up * lift;
        let ry = Quat::from_rotation_y(self.yaw + self.slide);
        let travel_right = {
            let t = dir(self.heading);
            vec3(-t.z, 0.0, t.x)
        };
        let wobble = (time * 17.0).sin() * 0.035 * ((self.speed - 11.0) / 5.0).clamp(0.0, 1.0) + (time * 21.0).sin() * 0.06 * self.sketch;
        let roll = -self.lean * if self.fakie { -1.0 } else { 1.0 } + wobble;
        let pivot = if !airborne && self.pitch.abs() > 0.01 { vec3(0.0, 0.0, -0.21 * self.pitch.signum()) } else { Vec3::ZERO };
        let bm = Mat4::from_translation(base)
            * Mat4::from_quat(ry)
            * Mat4::from_translation(pivot)
            * Mat4::from_quat(Quat::from_rotation_y(shuv) * Quat::from_rotation_x(-self.pitch) * Quat::from_rotation_z(roll + flip))
            * Mat4::from_translation(-pivot);
        draw_board(b, &bm);

        // Base do corpo: fb = eixo do shape, chest = pra onde o peito aponta (regular).
        let fb = dir(self.yaw + self.slide * 0.35);
        let sb = vec3(-fb.z, 0.0, fb.x);
        let tw = if self.push_p > 0.0 && self.on_ground { (self.push_p * PI).sin() * 0.55 } else { 0.0 };
        let chest = (sb * (1.0 - tw) + fb * tw).normalize();
        let bal = self.grind.as_ref().map(|g| g.bal).unwrap_or(0.0);
        let u = (up - travel_right * (self.lean * 1.6 + bal * 0.25)).normalize();

        let rm = Mat4::from_translation(base) * Mat4::from_quat(ry);
        let slide_ft = if airborne { ((self.pop_t / 0.15).clamp(0.0, 1.0) * (1.0 - ((self.pop_t - 0.25) / 0.2).clamp(0.0, 1.0))) * 0.12 } else { 0.0 };
        let (fz, bz) = if self.pop_sign > 0.0 { (0.16 + slide_ft, -0.3) } else { (0.3, -0.16 - slide_ft) };
        let (front, mut back) = if flipping {
            let h = (k * PI).sin();
            (rm.transform_point3(vec3(-0.02, 0.22 + 0.18 * h, 0.2)), rm.transform_point3(vec3(-0.02, 0.22 + 0.14 * h, -0.24)))
        } else {
            (bm.transform_point3(vec3(-0.02, 0.18, fz)), bm.transform_point3(vec3(-0.02, 0.2, bz)))
        };
        let mut back_dir = (chest - fb * 0.15).normalize();
        if self.push_p > 0.0 && self.on_ground {
            let p = self.push_p;
            let ground = |q: f32| self.pos - sb * 0.2 + fb * (0.25 - 0.75 * q) + up * 0.04;
            back = if p < 0.25 {
                let q = p / 0.25;
                back.lerp(ground(0.0), q) + up * (q * PI).sin() * 0.12
            } else if p < 0.7 {
                ground((p - 0.25) / 0.45)
            } else {
                let q = (p - 0.7) / 0.3;
                ground(1.0).lerp(back, q) + up * (q * PI).sin() * 0.15
            };
            back_dir = fb;
        }
        if self.brake_amt > 0.01 {
            back = back.lerp(self.pos - sb * 0.15 - fb * 0.4 + up * 0.04, self.brake_amt);
        }
        let travel = if airborne && vec2(self.vel.x, self.vel.z).length() > 1.0 { vec3(self.vel.x, 0.0, self.vel.z).normalize() } else { dir(self.heading) };
        let grab_side = |x: f32, z: f32| bm.transform_point3(vec3(x, 0.15, z));
        let grab = (self.grab > 0.3).then(|| match self.grab_name {
            "MELON" => (0, grab_side(0.12, 0.05)),
            "NOSEGRAB" => (0, grab_side(0.0, 0.36)),
            "TAILGRAB" => (1, grab_side(0.0, -0.36)),
            "STALEFISH" => (1, grab_side(0.12, -0.08)),
            _ => (1, grab_side(-0.12, -0.05)),
        });
        let pose = Pose {
            front,
            back,
            fdir: [(chest * 0.8 + fb * (0.55 + tw)).normalize(), back_dir],
            fb,
            chest,
            u,
            crouch: self.crouch,
            hip_shift: -fb * (self.pitch * 0.3) - travel_right * bal * 0.12 + sb * (time * 14.0).sin() * 0.04 * self.sketch,
            spine_tilt: -fb * (self.pitch * 0.6),
            arms: self.arms,
            swing: if self.push_p > 0.0 { (self.push_p * TAU).sin() * 0.25 } else { 0.0 },
            grab,
            look: (chest * 0.4 + travel).normalize_or(chest),
        };
        draw_body(b, &pose, shirt, time);
    }

    fn draw_bail(&self, b: &mut Batch, shirt: Color, time: f32) {
        let up = Vec3::Y;
        draw_board(b, &Mat4::from_rotation_translation(self.board_rot, self.board_pos));
        let t = 2.0 - self.bail;
        let fall = (t / 0.4).min(1.0) * FRAC_PI_2;
        let rt = (t - 0.4).max(0.0);
        let roll = self.bail_spd * (1.0 - (-1.5 * rt).exp()) / 1.5 * 1.2;
        let fwd = self.bail_dir;
        let q_fall = Quat::from_axis_angle(vec3(fwd.z, 0.0, -fwd.x), fall);
        let lu = q_fall * up;
        let q = Quat::from_axis_angle(lu, roll) * q_fall;
        let f0 = dir(self.yaw);
        let s0 = vec3(-f0.z, 0.0, f0.x);
        let (fb, chest, u) = (q * f0, q * s0, q * up);
        let o = self.pos + up * (0.08 + 0.1 * fall / FRAC_PI_2);
        let flail = (time * 12.0).sin() * (1.0 - fall / FRAC_PI_2 * 0.7);
        let pose = Pose {
            front: o + fb * 0.22 + u * 0.05,
            back: o - fb * 0.25 + u * 0.12 + chest * 0.1,
            fdir: [chest, chest],
            fb,
            chest,
            u,
            crouch: 0.35,
            hip_shift: Vec3::ZERO,
            spine_tilt: -chest * 0.2,
            arms: 1.0,
            swing: flail * 0.4,
            grab: None,
            look: chest,
        };
        draw_body(b, &pose, shirt, time);
    }
}

fn draw_board(b: &mut Batch, m: &Mat4) {
    let grip = rgb(0.06, 0.06, 0.07);
    let wood = rgb(0.78, 0.58, 0.32);
    let art = rgb(0.85, 0.2, 0.15);
    let metal = rgb(0.72, 0.72, 0.76);
    let wheel = rgb(0.96, 0.94, 0.86);
    b.cube(m, vec3(0.0, 0.125, 0.0), vec3(0.21, 0.025, 0.56), wood);
    b.cube(m, vec3(0.0, 0.139, 0.0), vec3(0.205, 0.004, 0.56), grip);
    b.cube(m, vec3(0.0, 0.111, 0.0), vec3(0.17, 0.004, 0.4), art);
    b.cube(m, vec3(0.0, 0.110, 0.0), vec3(0.06, 0.004, 0.42), rgb(1.0, 0.85, 0.1));
    for e in [-1.0f32, 1.0] {
        let km = *m * Mat4::from_translation(vec3(0.0, 0.125, e * 0.28)) * Mat4::from_rotation_x(-e * 0.35);
        b.cube(&km, vec3(0.0, 0.0, e * 0.065), vec3(0.205, 0.025, 0.13), wood);
        b.cube(&km, vec3(0.0, 0.0135, e * 0.065), vec3(0.2, 0.004, 0.13), grip);
        b.cube(m, vec3(0.0, 0.098, e * 0.2), vec3(0.07, 0.02, 0.09), rgb(0.3, 0.3, 0.33));
        b.cube(m, vec3(0.0, 0.07, e * 0.2), vec3(0.2, 0.03, 0.04), metal);
        for sx in [-0.105f32, 0.105] {
            b.cube(m, vec3(sx, 0.03, e * 0.2), vec3(0.04, 0.056, 0.056), wheel);
        }
    }
}

struct Pose {
    front: Vec3,
    back: Vec3,
    fdir: [Vec3; 2],
    fb: Vec3,
    chest: Vec3,
    u: Vec3,
    crouch: f32,
    hip_shift: Vec3,
    spine_tilt: Vec3,
    arms: f32,
    swing: f32,
    /// (0 = mão da frente, 1 = de trás, ponto no shape)
    grab: Option<(usize, Vec3)>,
    look: Vec3,
}

fn draw_body(b: &mut Batch, p: &Pose, shirt: Color, time: f32) {
    let pants = rgb(0.16, 0.2, 0.3);
    let skin = rgb(0.85, 0.65, 0.5);
    let shoe = rgb(0.92, 0.92, 0.9);
    let sole = rgb(0.2, 0.2, 0.22);
    let cap = rgb(0.85, 0.15, 0.15);
    let black = rgb(0.05, 0.05, 0.06);
    let (u, chest, fb) = (p.u, p.chest, p.fb);

    for (i, foot) in [p.front, p.back].into_iter().enumerate() {
        let m = frame(p.fdir[i], u, foot);
        b.cube(&m, vec3(0.0, 0.0, 0.03), vec3(0.11, 0.075, 0.28), shoe);
        b.cube(&m, vec3(0.0, -0.035, 0.03), vec3(0.115, 0.02, 0.29), sole);
    }
    let hip = (p.front + p.back) * 0.5 + u * (0.86 - p.crouch * 0.42) - chest * (0.03 + 0.07 * p.crouch) + p.hip_shift;
    for (i, foot) in [p.front, p.back].into_iter().enumerate() {
        let e = if i == 0 { 1.0 } else { -1.0 };
        let h = hip + fb * (0.1 * e);
        let (knee, end) = ik(h, foot + u * 0.05, 0.47, 0.45, chest + fb * (0.35 * e) + u * 0.1);
        limb(b, h, knee, 0.15, pants);
        limb(b, knee, end, 0.12, pants);
    }
    limb(b, hip - fb * 0.13, hip + fb * 0.13, 0.2, pants);
    let spine = (u + chest * (0.15 + 0.5 * p.crouch.max(0.0)) + p.spine_tilt).normalize();
    let neck = hip + spine * 0.52;
    limb(b, hip, neck, 0.32, shirt);
    let sax = u.cross(chest).normalize_or(fb);

    let hc = neck + spine * 0.17 + chest * 0.03;
    let hm = frame(p.look, spine, hc);
    b.cube(&hm, Vec3::ZERO, vec3(0.25, 0.27, 0.25), skin);
    b.cube(&hm, vec3(0.0, 0.12, 0.0), vec3(0.27, 0.09, 0.27), cap);
    b.cube(&hm, vec3(0.0, 0.09, 0.17), vec3(0.24, 0.025, 0.12), cap);
    for e in [-1.0f32, 1.0] {
        b.cube(&hm, vec3(e * 0.06, 0.02, 0.126), Vec3::splat(0.04), black);
    }

    for (i, e) in [(0usize, 1.0f32), (1, -1.0)] {
        let sh = neck - spine * 0.06 + sax * (0.2 * e);
        let hang = sh - u * 0.52 + chest * 0.06 + sax * (0.07 * e);
        let out = sh + sax * (0.5 * e) - u * 0.12 + chest * 0.08;
        let sway = u * (0.05 * (time * 2.0 + i as f32).sin());
        let mut hand = hang.lerp(out, p.arms) + sway + chest * (p.swing * e);
        if let Some((gi, gp)) = p.grab {
            if gi == i {
                hand = gp;
            }
        }
        let (el, hd) = ik(sh, hand, 0.3, 0.3, -u - chest * 0.3 + sax * (0.4 * e));
        limb(b, sh, el, 0.11, shirt);
        limb(b, el, hd, 0.1, shirt);
        b.cube(&Mat4::from_translation(hd), Vec3::ZERO, Vec3::splat(0.08), skin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(path: &[[f32; 2]]) -> Option<(String, f32)> {
        let mut g = Gestures::new();
        let mut samples: Vec<Vec2> = vec![Vec2::ZERO];
        samples.extend(path.iter().map(|p| vec2(p[0], p[1])));
        samples.extend([Vec2::ZERO; 12]);
        samples.into_iter().find_map(|s| g.sample(s)).map(|(i, s)| (g.pats[i].name.clone(), s))
    }

    #[test]
    fn flicks() {
        assert_eq!(run(&[D, D, D, D, [0.0, 0.0], U]).unwrap().0, "OLLIE");
        assert_eq!(run(&[D, UL]).unwrap().0, "KICKFLIP");
        assert_eq!(run(&[D, DL, L]).unwrap().0, "POP SHOVE-IT");
        assert_eq!(run(&[D, DL, L, UL, U]).unwrap().0, "360 SHOVE-IT");
        assert_eq!(run(&[D, DL, L, UL]).unwrap().0, "VARIAL KICKFLIP");
        assert_eq!(run(&[D, DR, R]).unwrap().0, "FS SHOVE-IT");
        assert_eq!(run(&[L, DL, D, DR, UR]).unwrap().0, "360 FLIP");
        assert_eq!(run(&[U, D]).unwrap().0, "NOLLIE");
        let fast = run(&[D, U]).unwrap().1;
        let slow = run(&[D, [0.0, -0.5], [0.0, -0.2], [0.0, 0.0], [0.0, 0.2], [0.0, 0.4], U]).unwrap().1;
        assert!(fast > slow, "{fast} {slow}");
    }
}
