//! MARIO, o encanador da vila (paródia original: boné com "U" de Urna, bigodão, macacão azul de
//! botão amarelo, luva branca). Repertório de plataforma 3D dos anos 90 com os números do original
//! (unidades/quadro a 30 fps): aceleração com curva, derrapada, pulo simples/duplo/triplo (com
//! mortal), mortal pra trás, pirueta lateral, pulo longo, chute na parede, sentada com onda de choque,
//! mergulho, soco-soco-chute, voadora e carrinho. O NPC é simulado no host e briga com os lutadores
//! (pisão na cabeça dói mais); os outros recebem "mr" no snapshot. Jogável como ENCANADOR (C).

use crate::actors::{Atk, Ev, FState, Fighter, angle_lerp};
use crate::audio::Clip;
use crate::batch::Batch;
use crate::extras::Label;
use crate::models::{U, rgb};
use crate::npc::{self, Npcs, Vida};
use crate::urna::{Fx, Particle};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

/// 1 unidade/quadro do original (30 fps, encanador = 160 u = 1,5 bloco) em blocos/s.
const K: f32 = 30.0 * 1.5 / 160.0;
const GRAV: f32 = 4.0 * 30.0 * K;
const TERM: f32 = -75.0 * K;
const RUN: f32 = 32.0 * K;
const HALF: f32 = 0.3;
const HEIGHT: f32 = 1.45;
const PIV: f32 = 0.75;

const IDLE: u8 = 0;
const WALK: u8 = 1;
const SKID: u8 = 2;
const CROUCH: u8 = 3;
const JUMP: u8 = 4;
const BACKFLIP: u8 = 5;
const SIDEFLIP: u8 = 6;
const LONGJUMP: u8 = 7;
const WALLKICK: u8 = 8;
const FALL: u8 = 9;
const GP_SPIN: u8 = 10;
const GP_FALL: u8 = 11;
const GP_LAND: u8 = 12;
const DIVE: u8 = 13;
const BELLY: u8 = 14;
const PUNCH: u8 = 15;
const JUMPKICK: u8 = 16;
const SLIDEKICK: u8 = 17;
const SLIDE: u8 = 18;
const KNOCK: u8 = 19;
const DEAD: u8 = 20;
const WALLHIT: u8 = 21;

// Vozes e efeitos (também viajam pela rede como eventos)
const V_HOO: u8 = 0;
const V_HAH: u8 = 1;
const V_WAHOO: u8 = 2;
const V_YAH: u8 = 3;
const V_HUP: u8 = 4;
const V_OOF: u8 = 5;
const V_AAH: u8 = 6;
const V_YAHOO: u8 = 7;
const E_COIN: u8 = 8;
const E_WAVE: u8 = 9;
const E_PIPE: u8 = 10;

const FALAS: [&str; 6] = ["VAMO QUE VAMO!", "ESSE VOTO TA ENCANADO!", "CANO NOVO, URNA VELHA", "PULO TRIPLO NO 2o TURNO!", "PRINCESA? SO SE FOR DO CLUB", "CONSERTO ATE URNA, SO NAO CONSERTO POLITICO"];

fn fwd(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn approach(x: f32, to: f32, step: f32) -> f32 {
    if x < to { (x + step).min(to) } else { (x - step).max(to) }
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn airborne(a: u8) -> bool {
    matches!(a, JUMP | BACKFLIP | SIDEFLIP | LONGJUMP | WALLKICK | FALL | GP_SPIN | GP_FALL | DIVE | JUMPKICK | SLIDEKICK | WALLHIT)
}

/// Entrada do frame: direção desejada no mundo (|mv| <= 1), A (pulo), Z (agachar), B (soco).
#[derive(Clone, Copy, Default)]
pub struct Intent {
    pub mv: Vec3,
    pub jump: bool,
    pub hold: bool,
    pub crouch: bool,
    pub punch: bool,
}

/// Alvo pra golpe: centro e raio (topo = c.y + r é onde se pisa).
pub struct Foe {
    pub key: u32,
    pub c: Vec3,
    pub r: f32,
}

pub struct Strike {
    pub key: u32,
    pub dmg: f32,
    pub knock: Vec3,
    pub at: Vec3,
    pub w: &'static str,
    pub stomp: bool,
}

/// Corpo com o repertório completo; quem dirige é a IA (NPC) ou o teclado (ENCANADOR).
pub struct Body {
    pub pos: Vec3,
    pub vy: f32,
    /// Velocidade pra frente (negativa = recuando), sempre ao longo de `yaw`.
    pub spd: f32,
    pub yaw: f32,
    pub act: u8,
    pub aux: u8,
    pub t: f32,
    pub ground: bool,
    pub walk: f32,
    pub squash: f32,
    pub invuln: f32,
    pub stars: f32,
    /// Andou de cara numa parede neste frame (a IA pula).
    pub blocked: bool,
    chain: u8,
    chain_t: f32,
    wall_n: Vec3,
    prev_crouch: bool,
    punch_q: bool,
    shock: bool,
    hit: Vec<u32>,
    pub voice: Vec<u8>,
}

impl Body {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Body { pos, vy: 0.0, spd: 0.0, yaw, act: FALL, aux: 0, t: 0.0, ground: false, walk: 0.0, squash: 0.0, invuln: 0.0, stars: 0.0, blocked: false, chain: 0, chain_t: 9.0, wall_n: Vec3::ZERO, prev_crouch: false, punch_q: false, shock: false, hit: Vec::new(), voice: Vec::new() }
    }

    fn enter(&mut self, act: u8, aux: u8) {
        self.act = act;
        self.aux = aux;
        self.t = 0.0;
        self.hit.clear();
        self.punch_q = false;
    }

    fn collides(world: &World, p: Vec3) -> bool {
        let (x0, x1) = ((p.x - HALF).floor() as i32, (p.x + HALF).floor() as i32);
        let (y0, y1) = (p.y.floor() as i32, (p.y + HEIGHT).floor() as i32);
        let (z0, z1) = ((p.z - HALF).floor() as i32, (p.z + HALF).floor() as i32);
        (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| world.solid(x, y, z))))
    }

    fn jump(&mut self) {
        let quick = self.chain_t < 0.22;
        if quick && self.chain == 2 && self.spd > 20.0 * K {
            self.vy = 69.0 * K;
            self.enter(JUMP, 3);
            self.voice.push(V_WAHOO);
        } else if quick && self.chain == 1 && self.spd > 1.0 {
            self.vy = 52.0 * K + 0.25 * self.spd;
            self.enter(JUMP, 2);
            self.voice.push(V_HAH);
        } else {
            self.vy = 42.0 * K + 0.25 * self.spd;
            self.enter(JUMP, 1);
            self.voice.push(V_HOO);
        }
        self.ground = false;
    }

    fn crouch_jump(&mut self) {
        if self.spd > 2.2 {
            self.vy = 30.0 * K;
            self.spd = (self.spd * 1.5).min(48.0 * K);
            self.enter(LONGJUMP, 0);
            self.voice.push(V_WAHOO);
        } else {
            self.vy = 62.0 * K;
            self.spd = -16.0 * K;
            self.enter(BACKFLIP, 0);
            self.voice.push(V_YAHOO);
        }
        self.ground = false;
    }

    fn slide_kick(&mut self) {
        self.vy = 12.0 * K;
        self.spd = self.spd.max(32.0 * K);
        self.enter(SLIDEKICK, 0);
        self.voice.push(V_HUP);
        self.ground = false;
    }

    /// Controle no ar do original: acelera/freia pela componente pra frente, vira devagar pelo lado.
    fn air_control(&mut self, dt: f32, mag: f32, dyaw: f32, cap: f32, k: f32) {
        self.spd += 1.5 * 30.0 * K * dyaw.cos() * mag * dt * k;
        self.yaw += 1.47 * dyaw.sin() * mag * dt * k;
        if self.spd > cap {
            self.spd -= 30.0 * K * dt;
        }
        if self.spd < -16.0 * K {
            self.spd += 2.0 * 30.0 * K * dt;
        }
    }

    /// Tomou golpe: vira pro agressor e voa pra trás (forte = estrelinhas).
    pub fn hurt(&mut self, knock: Vec3, hard: bool) {
        if self.act == DEAD {
            return;
        }
        let h = vec3(knock.x, 0.0, knock.z);
        if h.length() > 0.1 {
            self.yaw = (-h.x).atan2(-h.z);
        }
        self.spd = -(h.length() * 0.8).max(2.0);
        self.vy = knock.y.max(if hard { 7.0 } else { 3.5 });
        self.ground = false;
        self.enter(KNOCK, hard as u8);
        self.invuln = 1.2;
        self.stars = if hard { 2.2 } else { 1.0 };
        self.voice.push(V_OOF);
    }

    pub fn die(&mut self) {
        self.enter(DEAD, 0);
        self.spd = -3.0;
        self.vy = 7.0;
        self.ground = false;
        self.stars = 0.0;
        self.voice.push(V_AAH);
    }

    fn land(&mut self) {
        self.squash = 1.0;
        self.chain_t = 0.0;
        let next = if self.spd.abs() > 0.3 { WALK } else { IDLE };
        match self.act {
            JUMP => {
                self.chain = if self.aux >= 3 { 0 } else { self.aux };
                self.enter(next, 0);
            }
            LONGJUMP => {
                self.chain = 0;
                self.spd *= 0.5;
                self.enter(WALK, 0);
            }
            GP_FALL => {
                self.chain = 0;
                self.enter(GP_LAND, 0);
                self.shock = true;
                self.voice.push(E_WAVE);
            }
            GP_SPIN => self.enter(GP_LAND, 0),
            DIVE => self.enter(BELLY, 0),
            SLIDEKICK => self.enter(SLIDE, 0),
            KNOCK => self.t = 0.0,
            DEAD => {}
            _ => {
                self.chain = 0;
                self.spd = self.spd.max(0.0);
                self.enter(next, 0);
            }
        }
    }

    fn ground_step(&mut self, dt: f32, inp: &Intent, mag: f32, want: f32, dyaw: f32) {
        match self.act {
            IDLE | WALK => {
                if inp.jump && inp.crouch {
                    self.crouch_jump();
                } else if inp.jump {
                    self.jump();
                } else if inp.punch && self.spd >= 29.0 * K {
                    self.vy = 2.5;
                    self.spd = (self.spd + 15.0 * K).min(48.0 * K);
                    self.enter(DIVE, 0);
                    self.voice.push(V_HUP);
                    self.ground = false;
                } else if inp.punch && inp.crouch && self.spd > 2.2 {
                    self.slide_kick();
                } else if inp.punch {
                    self.enter(PUNCH, 0);
                    self.voice.push(V_YAH);
                } else if inp.crouch {
                    self.enter(CROUCH, 0);
                } else if mag > 0.1 {
                    if self.spd >= 16.0 * K && dyaw.abs() > 1.75 {
                        self.enter(SKID, 0);
                        return;
                    }
                    let rate = if self.spd < 2.0 { 14.0 } else { 5.9 };
                    self.yaw += dyaw.clamp(-rate * dt, rate * dt);
                    let tgt = RUN * mag;
                    let a = 30.0 * K * dt;
                    self.spd = if self.spd <= 0.0 {
                        self.spd + 1.1 * a
                    } else if self.spd <= tgt {
                        (self.spd + (1.1 - self.spd / K / 43.0) * a).min(tgt)
                    } else {
                        (self.spd - a).max(tgt)
                    };
                    self.act = WALK;
                } else {
                    self.spd = approach(self.spd, 0.0, 2.0 * 30.0 * K * dt);
                    if self.spd == 0.0 {
                        self.act = IDLE;
                    }
                }
            }
            SKID => {
                if inp.jump {
                    self.yaw = if mag > 0.1 { want } else { self.yaw + PI };
                    self.vy = 62.0 * K;
                    self.spd = 8.0 * K;
                    self.enter(SIDEFLIP, 0);
                    self.voice.push(V_YAHOO);
                    self.ground = false;
                    return;
                }
                self.spd -= 2.0 * 30.0 * K * dt;
                if self.spd <= 0.0 {
                    self.yaw = want;
                    self.spd = 0.0;
                    self.enter(WALK, 0);
                }
            }
            CROUCH => {
                self.spd = approach(self.spd, 0.0, (if self.spd > 2.2 { 7.0 } else { 20.0 }) * dt);
                if inp.jump {
                    self.crouch_jump();
                } else if inp.punch && self.spd > 2.2 {
                    self.slide_kick();
                } else if !inp.crouch {
                    self.enter(IDLE, 0);
                }
            }
            PUNCH => {
                let dur = if self.aux == 2 { 0.38 } else { 0.22 };
                self.spd = if self.t < 0.06 { self.spd.max(2.5) } else { approach(self.spd, 0.0, 20.0 * dt) };
                if inp.punch && self.t > 0.04 {
                    self.punch_q = true;
                }
                if self.t >= dur {
                    if self.punch_q && self.aux < 2 {
                        let n = self.aux + 1;
                        self.enter(PUNCH, n);
                        self.voice.push(if n == 2 { V_HUP } else { V_YAH });
                    } else {
                        self.enter(IDLE, 0);
                    }
                }
            }
            GP_LAND => {
                self.spd = 0.0;
                if self.t > 0.45 || (self.t > 0.2 && inp.jump) {
                    self.enter(IDLE, 0);
                }
            }
            BELLY | SLIDE => {
                self.spd = approach(self.spd, 0.0, (if self.act == BELLY { 7.0 } else { 9.0 }) * dt);
                if (inp.jump || inp.punch) && self.t > 0.15 {
                    self.vy = 30.0 * K;
                    self.spd = self.spd.max(10.0 * K);
                    self.enter(JUMP, 1);
                    self.voice.push(V_HOO);
                    self.ground = false;
                } else if self.spd < 0.5 && self.t > 0.4 {
                    self.enter(IDLE, 0);
                }
            }
            KNOCK => {
                self.spd = approach(self.spd, 0.0, 12.0 * dt);
                let lie = if self.aux == 1 { 1.1 } else { 0.5 };
                if self.t > lie {
                    self.enter(IDLE, 0);
                }
            }
            DEAD => self.spd = approach(self.spd, 0.0, 10.0 * dt),
            _ => {}
        }
    }

    fn air_step(&mut self, dt: f32, inp: &Intent, crouch_press: bool, mag: f32, dyaw: f32) {
        match self.act {
            JUMP | BACKFLIP | SIDEFLIP | WALLKICK | FALL | LONGJUMP => {
                if self.act == JUMP && self.aux < 3 && !inp.hold && self.vy > 0.0 {
                    self.vy -= GRAV * 2.0 * dt;
                }
                if crouch_press && self.act != LONGJUMP {
                    self.vy = 0.0;
                    self.spd = 0.0;
                    self.enter(GP_SPIN, 0);
                    self.voice.push(V_HUP);
                } else if inp.punch && self.spd >= 28.0 * K {
                    self.spd = (self.spd + 15.0 * K).min(48.0 * K);
                    self.enter(DIVE, 0);
                    self.voice.push(V_HUP);
                } else if inp.punch {
                    self.vy = self.vy.max(12.0 * K);
                    self.enter(JUMPKICK, 0);
                    self.voice.push(V_YAH);
                } else {
                    let cap = if self.act == LONGJUMP { 48.0 * K } else { 32.0 * K };
                    self.air_control(dt, mag, dyaw, cap.max(self.spd.min(48.0 * K)), 1.0);
                }
            }
            JUMPKICK | DIVE | SLIDEKICK => self.air_control(dt, mag, dyaw, 48.0 * K, 0.3),
            WALLHIT => {
                self.vy = self.vy.max(-2.0);
                self.spd = 0.0;
                if inp.jump {
                    self.yaw = self.wall_n.x.atan2(self.wall_n.z);
                    self.spd = 24.0 * K;
                    self.vy = 62.0 * K;
                    self.enter(WALLKICK, 0);
                    self.voice.push(V_WAHOO);
                } else if self.t > 0.16 {
                    self.enter(FALL, 0);
                }
            }
            GP_SPIN => {
                self.vy = 0.0;
                self.spd = 0.0;
                if self.t >= 0.3 {
                    self.enter(GP_FALL, 0);
                }
            }
            GP_FALL => {
                self.vy = -50.0 * K;
                self.spd = 0.0;
            }
            _ => {}
        }
    }

    /// Um frame: entrada -> ação -> física com voxel -> golpes. Devolve quem apanhou.
    pub fn update(&mut self, world: &World, dt: f32, inp: &Intent, foes: &[Foe]) -> Vec<Strike> {
        self.t += dt;
        self.chain_t += dt;
        self.invuln -= dt;
        self.stars = (self.stars - dt).max(0.0);
        self.squash = (self.squash - dt * 6.0).max(0.0);
        let crouch_press = inp.crouch && !self.prev_crouch;
        self.prev_crouch = inp.crouch;
        let mag = inp.mv.length().min(1.0);
        let want = if mag > 0.1 { inp.mv.x.atan2(inp.mv.z) } else { self.yaw };
        let dyaw = wrap(want - self.yaw);
        let dead = self.act == DEAD;
        let inp = if dead { Intent::default() } else { *inp };

        if self.ground {
            self.ground_step(dt, &inp, mag, want, dyaw);
        } else {
            self.air_step(dt, &inp, crouch_press, mag, dyaw);
        }
        if self.act != GP_SPIN {
            self.vy = (self.vy - GRAV * dt).max(TERM);
        }

        // Física: eixos separados, subpassos pra não atravessar bloco em queda rápida
        let was_ground = self.ground;
        let v = fwd(self.yaw) * self.spd + Vec3::Y * self.vy;
        let n = (v.length() * dt / 0.3).ceil().clamp(1.0, 12.0) as i32;
        let step = v * dt / n as f32;
        let mut wall: Option<Vec3> = None;
        let mut vstep = step.y;
        self.ground = false;
        for _ in 0..n {
            for axis in [0usize, 2] {
                if step[axis] == 0.0 {
                    continue;
                }
                let mut p = self.pos;
                p[axis] = (p[axis] + step[axis]).clamp(1.0, WX as f32 - 1.0);
                if Self::collides(world, p) {
                    let mut nrm = Vec3::ZERO;
                    nrm[axis] = -step[axis].signum();
                    wall = Some(nrm);
                } else {
                    self.pos = p;
                }
            }
            if vstep != 0.0 {
                let mut p = self.pos;
                p.y += vstep;
                if !Self::collides(world, p) {
                    self.pos = p;
                } else {
                    if vstep < 0.0 {
                        let q = vec3(p.x, p.y.floor() + 1.0, p.z);
                        if q.y <= self.pos.y + 0.001 && !Self::collides(world, q) {
                            self.pos = q;
                        }
                        self.ground = true;
                    }
                    self.vy = 0.0;
                    vstep = 0.0;
                }
            }
        }
        self.blocked = false;
        if let Some(nrm) = wall {
            if self.ground || was_ground {
                self.blocked = self.spd > 0.5;
                self.spd = self.spd.min(2.0);
            } else if matches!(self.act, LONGJUMP | DIVE) && self.spd > 16.0 * K {
                self.hurt(nrm * 3.0 + Vec3::Y * 2.0, false);
            } else if matches!(self.act, JUMP | BACKFLIP | SIDEFLIP | WALLKICK | FALL) && self.spd > 2.5 && nrm.dot(fwd(self.yaw)) < -0.3 {
                self.wall_n = nrm;
                self.enter(WALLHIT, 0);
            }
        }
        if self.ground && (airborne(self.act) || matches!(self.act, KNOCK | DEAD) && !was_ground) {
            self.land();
        } else if !self.ground && !airborne(self.act) && !matches!(self.act, KNOCK | DEAD) {
            self.enter(FALL, 0);
        }
        if self.pos.y < -8.0 {
            self.pos = arena_center() + vec3(0.0, 14.0, 0.0);
            self.vy = 0.0;
            self.spd = 0.0;
            self.enter(FALL, 1);
        }
        self.walk += self.spd.abs() * dt * 1.9;

        if dead {
            return Vec::new();
        }
        self.strikes(foes, inp.hold)
    }

    fn strikes(&mut self, foes: &[Foe], hold: bool) -> Vec<Strike> {
        let mut out = Vec::new();
        let fw = fwd(self.yaw);
        let up = Vec3::Y;
        let t = self.t;
        // (à frente, altura, raio, dano, empurrão horizontal, vertical, nome)
        let hitbox = match self.act {
            PUNCH if self.aux < 2 && (0.04..0.14).contains(&t) => Some((0.65, 1.0, 0.6, 5.0 + self.aux as f32, 3.0, 1.5, "SOCO")),
            PUNCH if self.aux == 2 && (0.08..0.24).contains(&t) => Some((0.75, 0.6, 0.7, 10.0, 8.0, 5.0, "CHUTE")),
            JUMPKICK if t < 0.3 => Some((0.6, 0.6, 0.7, 8.0, 6.0, 4.0, "VOADORA")),
            DIVE | BELLY if self.spd > 3.0 => Some((0.7, 0.4, 0.7, 7.0, 7.0, 3.0, "MERGULHO")),
            SLIDEKICK | SLIDE if self.spd > 3.0 => Some((0.6, 0.3, 0.75, 9.0, 6.0, 6.0, "CARRINHO")),
            _ => None,
        };
        let shock = std::mem::take(&mut self.shock);
        let mut bounce = false;
        for f in foes {
            if self.hit.contains(&f.key) {
                continue;
            }
            let flat = vec3(f.c.x - self.pos.x, 0.0, f.c.z - self.pos.z);
            let dir = flat.normalize_or(fw);
            let top = f.c.y + f.r;
            if !self.ground && self.vy < -1.0 && !matches!(self.act, KNOCK | WALLHIT) && flat.length() < f.r * 0.8 + 0.3 && self.pos.y > top - 0.6 && self.pos.y < top + 0.4 {
                let gp = self.act == GP_FALL;
                out.push(Strike { key: f.key, dmg: if gp { 26.0 } else { 16.0 }, knock: dir * 2.0 - up * 2.0, at: vec3(f.c.x, top, f.c.z), w: if gp { "SENTADA" } else { "PISAO" }, stomp: true });
                self.hit.push(f.key);
                bounce = true;
                continue;
            }
            if shock && flat.length() < 2.8 + f.r * 0.5 && (f.c.y - self.pos.y).abs() < 1.8 {
                out.push(Strike { key: f.key, dmg: 10.0, knock: dir * 9.0 + up * 6.0, at: f.c, w: "ONDA DE CHOQUE", stomp: false });
                self.hit.push(f.key);
                continue;
            }
            if let Some((ahead, h, r, dmg, kh, kv, w)) = hitbox {
                let c = self.pos + fw * ahead + up * h;
                if c.distance(f.c) < r + f.r * 0.8 {
                    out.push(Strike { key: f.key, dmg, knock: dir * kh + up * kv, at: c.lerp(f.c, 0.5), w, stomp: false });
                    self.hit.push(f.key);
                }
            }
        }
        if bounce {
            let hit = std::mem::take(&mut self.hit);
            self.vy = if hold { 15.0 } else { 11.0 };
            self.enter(JUMP, 1);
            self.hit = hit;
            self.voice.push(V_HOO);
        }
        out
    }
}

// ---------------------------------------------------------------- Sons sintetizados (sem amostras)

struct Rng(u32);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn normalize(mut v: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-6);
    v.iter_mut().for_each(|x| *x *= peak / m);
    v
}

/// Voz de desenho: dente-de-serra com vibrato passando por dois formantes que deslizam entre vogais.
/// Sílaba = (f0 início, f0 fim, duração, formantes início, formantes fim, aspirado "h").
fn voice(sr: u32, syl: &[(f32, f32, f32, [f32; 2], [f32; 2], bool)]) -> Vec<f32> {
    let srf = sr as f32;
    let mut out = Vec::new();
    let mut rng = Rng(0x5EED_1234);
    let mut ph = 0.0f32;
    for &(f0a, f0b, dur, va, vb, h) in syl {
        let n = (dur * srf) as usize;
        let mut st = [[0.0f32; 2]; 2];
        for i in 0..n {
            let k = i as f32 / n as f32;
            let t = i as f32 / srf;
            let f0 = f0a + (f0b - f0a) * smooth(k) + (t * 33.0).sin() * f0a * 0.02;
            ph = (ph + f0 / srf).fract();
            let breath = if h { (-t / 0.035).exp() } else { 0.0 };
            let x = (1.0 - 2.0 * ph) * (1.0 - breath * 0.85) + rng.f() * breath * 1.2;
            let mut y = 0.0;
            for (j, s) in st.iter_mut().enumerate() {
                let f = (va[j] + (vb[j] - va[j]) * smooth(k * 1.4)) * 1.3;
                let r = (-PI * (110.0 + 60.0 * j as f32) / srf).exp();
                let v = (1.0 - r) * x + 2.0 * r * (TAU * f / srf).cos() * s[0] - r * r * s[1];
                s[1] = s[0];
                s[0] = v;
                y += v * if j == 0 { 1.0 } else { 0.7 };
            }
            let env = (t / 0.012).min(1.0) * ((dur - t) / 0.05).clamp(0.0, 1.0);
            out.push(y * env);
        }
    }
    normalize(out, 0.8)
}

const VU: [f32; 2] = [330.0, 800.0];
const VA: [f32; 2] = [760.0, 1250.0];
const VI: [f32; 2] = [300.0, 2250.0];
const VO: [f32; 2] = [520.0, 900.0];

fn render(sr: u32, secs: f32, mut f: impl FnMut(f32, &mut Rng) -> f32) -> Vec<f32> {
    let mut rng = Rng(0xC0FF_EE11);
    (0..(secs * sr as f32) as usize).map(|i| f(i as f32 / sr as f32, &mut rng)).collect()
}

/// Brilho de moeda: arpejo agudo curtinho.
fn chime(sr: u32) -> Vec<f32> {
    normalize(
        render(sr, 0.5, |t, _| {
            let mut s = 0.0;
            for (k, f) in [1567.98f32, 2093.0, 2637.0].into_iter().enumerate() {
                let tt = t - k as f32 * 0.055;
                if tt > 0.0 {
                    s += ((TAU * f * tt).sin() + 0.3 * (TAU * 2.0 * f * tt).sin()) * (-tt / 0.13).exp();
                }
            }
            s
        }),
        0.5,
    )
}

/// Sentada no chão: baque grave com poeira.
fn thud(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    normalize(
        render(sr, 0.45, move |t, r| {
            lp += (r.f() - lp) * 0.05;
            (TAU * (38.0 + 110.0 * (-t / 0.04).exp()) * t).sin() * (-t / 0.16).exp() * 1.4 + lp * 4.0 * (-t / 0.12).exp()
        }),
        0.9,
    )
}

/// Cano de chegada: sopro subindo e um "plop".
fn pipe_snd(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    let mut ph = 0.0;
    normalize(
        render(sr, 0.7, move |t, r| {
            lp += (r.f() - lp) * (0.02 + 0.2 * t);
            ph += (90.0 + 380.0 * (t / 0.55).min(1.0).powi(2)) / sr as f32;
            let tone = (TAU * ph).sin() * (t / 0.55).min(1.0) * if t < 0.55 { 1.0 } else { (-(t - 0.55) / 0.05).exp() };
            let pop = if t > 0.55 { (TAU * 700.0 * (t - 0.55)).sin() * (-(t - 0.55) / 0.03).exp() } else { 0.0 };
            lp * 1.5 * (1.0 - t / 0.7) + tone * 0.6 + pop
        }),
        0.7,
    )
}

fn clips(sr: u32) -> Vec<Clip> {
    let c = |v: Vec<f32>| Arc::new(v);
    vec![
        c(voice(sr, &[(330.0, 420.0, 0.16, VU, VU, true)])),
        c(voice(sr, &[(380.0, 500.0, 0.18, VA, VA, true)])),
        c(voice(sr, &[(300.0, 360.0, 0.13, VU, VA, false), (430.0, 640.0, 0.3, VU, VU, true)])),
        c(voice(sr, &[(430.0, 380.0, 0.13, VI, VA, false)])),
        c(voice(sr, &[(460.0, 520.0, 0.11, VA, VU, true)])),
        c(voice(sr, &[(300.0, 190.0, 0.22, VO, VU, false)])),
        c(voice(sr, &[(520.0, 150.0, 0.95, VA, VO, false)])),
        c(voice(sr, &[(360.0, 420.0, 0.13, VI, VA, false), (480.0, 720.0, 0.32, VU, VU, true)])),
        c(chime(sr)),
        c(thud(sr)),
        c(pipe_snd(sr)),
    ]
}

// ---------------------------------------------------------------- NPC + jogável

struct Swing {
    j: usize,
    t: f32,
    dmg: f32,
    knock: (f32, f32),
    claws: bool,
}

pub struct Mario {
    pub body: Body,
    /// Corpo do jogador local quando ele escolhe o ENCANADOR.
    pub me: Option<Body>,
    spawned: bool,
    target: Option<usize>,
    retarget: f32,
    cd: f32,
    hold_t: f32,
    plan: u8,
    plan_t: f32,
    plan_n: u8,
    aggro: Vec<f32>,
    chasing: Vec<bool>,
    swings: Vec<Swing>,
    last_hp: f32,
    net: Option<(Vec3, Vec3)>,
    pipe: Option<(Vec3, f32)>,
    coins: Vec<(Vec3, Vec3, f32)>,
    parts: Vec<Particle>,
    q: Vec<Value>,
    clips: Vec<Clip>,
    /// Sons pra tocar no main (clipe, posição, volume base).
    pub sfx: Vec<(Clip, Vec3, f32)>,
}

const P_NONE: u8 = 0;
const P_PUNCH: u8 = 1;
const P_STOMP: u8 = 2;
const P_TRIPLE: u8 = 3;
const P_SIDEFLIP: u8 = 4;
const P_BACKFLIP: u8 = 5;
const P_POUND: u8 = 6;

impl Mario {
    pub fn new(sr: u32) -> Self {
        Mario {
            body: Body::new(arena_center() + vec3(0.0, 16.0, 0.0), 0.0),
            me: None,
            spawned: false,
            target: None,
            retarget: 0.0,
            cd: 1.0,
            hold_t: 0.0,
            plan: P_NONE,
            plan_t: 0.0,
            plan_n: 0,
            aggro: Vec::new(),
            chasing: Vec::new(),
            swings: Vec::new(),
            last_hp: 0.0,
            net: None,
            pipe: None,
            coins: Vec::new(),
            parts: Vec::new(),
            q: Vec::new(),
            clips: clips(sr),
            sfx: Vec::new(),
        }
    }

    pub fn visible(&self) -> bool {
        self.spawned || self.net.is_some()
    }

    /// Alvo pra jogador/bandido acertar (barrinha sobre a cabeça vem daqui).
    pub fn target(&self, npcs: &Npcs) -> Option<crate::gta::Target> {
        (self.visible() && npcs.alive(npc::MARIO, 0)).then(|| (self.body.pos + Vec3::Y * 0.8, 0.7, 0, npc::MARIO))
    }

    /// Efeito local (todos os clientes): voz, moedas, onda de choque, cano.
    fn fx_local(&mut self, k: u8, p: Vec3) {
        let vol = match k {
            E_COIN => 0.9,
            E_WAVE => 1.4,
            E_PIPE => 1.3,
            _ => 1.1,
        };
        if let Some(c) = self.clips.get(k as usize) {
            self.sfx.push((c.clone(), p, vol));
        }
        match k {
            E_COIN => {
                for _ in 0..2 {
                    self.coins.push((p, vec3(gen_range(-1.5, 1.5), gen_range(4.0, 6.0), gen_range(-1.5, 1.5)), 0.0));
                }
                for _ in 0..10 {
                    let col = if gen_range(0.0, 1.0) < 0.5 { Color::new(1.0, 0.85, 0.15, 1.0) } else { WHITE };
                    self.parts.push(Particle { pos: p, vel: vec3(gen_range(-3.0, 3.0), gen_range(1.0, 4.0), gen_range(-3.0, 3.0)), col, life: gen_range(0.3, 0.6), size: 0.08, gravity: false });
                }
            }
            E_WAVE => {
                for i in 0..28 {
                    let a = i as f32 / 28.0 * TAU;
                    let d = vec3(a.cos(), 0.0, a.sin());
                    self.parts.push(Particle { pos: p + d * 0.4 + Vec3::Y * 0.1, vel: d * 9.0 + Vec3::Y * 0.6, col: Color::new(0.85, 0.8, 0.7, 1.0), life: 0.35, size: 0.22, gravity: false });
                    if i % 3 == 0 {
                        self.parts.push(Particle { pos: p + d * 0.3 + Vec3::Y * 0.2, vel: d * 4.0 + Vec3::Y * 1.0, col: Color::new(1.0, 0.95, 0.4, 1.0), life: 0.3, size: 0.12, gravity: false });
                    }
                }
            }
            E_PIPE => {
                self.pipe = Some((p, 0.0));
                for _ in 0..16 {
                    self.parts.push(Particle { pos: p, vel: vec3(gen_range(-2.0, 2.0), gen_range(-3.0, 0.0), gen_range(-2.0, 2.0)), col: Color::new(0.85, 0.95, 0.9, 1.0), life: gen_range(0.4, 0.8), size: 0.2, gravity: false });
                }
            }
            _ => {}
        }
    }

    /// Host: efeito local + fila pros outros clientes.
    fn emit(&mut self, k: u8, p: Vec3) {
        self.fx_local(k, p);
        if self.q.len() < 40 {
            self.q.push(json!([k, (p.x * 10.0).round() / 10.0, (p.y * 10.0).round() / 10.0, (p.z * 10.0).round() / 10.0]));
        }
    }

    fn spawn(&mut self, events: &mut Vec<Ev>, first: bool) {
        let c = arena_center();
        let p = vec3(c.x + gen_range(-9.0, 9.0), G as f32 + 16.0, c.z + gen_range(-9.0, 9.0));
        self.body = Body::new(p, gen_range(0.0, TAU));
        self.body.aux = 1;
        self.body.invuln = 2.5;
        self.spawned = true;
        self.target = None;
        self.plan = P_NONE;
        self.emit(E_PIPE, p + Vec3::Y * 1.7);
        self.emit(V_YAHOO, p);
        events.push(Ev::Banner(if first { "UM ENCANADOR CAIU DO CANO! E ELE QUER BRIGA".into() } else { "O ENCANADOR VOLTOU PELO CANO!".into() }));
    }

    fn fightable(&self) -> bool {
        self.spawned && !matches!(self.body.act, DEAD) && self.body.invuln <= 0.0
    }

    /// Host: dano no encanador (dos lutadores); morre pelo sistema de vida dos NPCs.
    #[allow(clippy::too_many_arguments)]
    fn take_hit(&mut self, npcs: &mut Npcs, dmg: f32, knock: Vec3, claws: bool, who: &str, kos: &mut u32, events: &mut Vec<Ev>) {
        if !self.fightable() {
            return;
        }
        let up = Vec3::Y;
        let p = self.body.pos;
        events.push(Ev::Hit { pos: p + up * 0.9, claws });
        events.push(Ev::Text { pos: p + up * 2.0 + vec3(gen_range(-0.4, 0.4), 0.0, gen_range(-0.4, 0.4)), text: format!("-{}", dmg.round() as i32), color: if claws { rgb(1.0, 0.9, 0.3) } else { WHITE }, big: false });
        if npcs.hit(npc::MARIO, 0, dmg) == Some(true) {
            *kos += 1;
            self.body.die();
            events.push(Ev::Text { pos: p + up * 2.6, text: "K.O.!".into(), color: rgb(1.0, 0.3, 0.2), big: true });
            events.push(Ev::Banner(format!("{who} NOCAUTEOU O MARIO! O CANO DEVOLVE EM 45s")));
            events.push(Ev::Shake(0.3));
        } else {
            self.body.hurt(knock, dmg >= 12.0);
        }
        self.last_hp = npcs.get(npc::MARIO, 0).map_or(0.0, |v| v.hp);
    }

    /// IA: escolhe lutador, corre com curva e usa o repertório conforme a distância.
    fn ai(&mut self, dt: f32, fighters: &[Fighter]) -> Intent {
        let b = &self.body;
        let mut inp = Intent::default();
        self.retarget -= dt;
        self.cd -= dt;
        self.hold_t -= dt;
        let valid = self.target.is_some_and(|j| fighters.get(j).is_some_and(|f| f.active()));
        if !valid || self.retarget <= 0.0 {
            self.retarget = gen_range(3.0, 6.0);
            self.target = fighters.iter().enumerate().filter(|(_, f)| f.active()).map(|(j, f)| (j, f.pos.distance(b.pos) * gen_range(0.6, 1.5))).min_by(|a, c| a.1.total_cmp(&c.1)).map(|x| x.0);
        }
        let Some(j) = self.target else {
            let to = arena_center() - b.pos;
            inp.mv = vec3(to.x, 0.0, to.z).clamp_length_max(1.0) * 0.6;
            return inp;
        };
        let tp = fighters[j].pos;
        let to = tp - b.pos;
        let flat = vec3(to.x, 0.0, to.z);
        let d = flat.length();
        let dir = flat.normalize_or(fwd(b.yaw));
        let dy = to.y;
        inp.mv = dir;
        let ready = b.ground && matches!(b.act, IDLE | WALK);
        let press = |inp: &mut Intent, hold: &mut f32, h: f32| {
            inp.jump = true;
            *hold = h;
        };

        match self.plan {
            P_PUNCH => {
                if b.act == PUNCH {
                    inp.punch = b.t > 0.08;
                    inp.mv = Vec3::ZERO;
                } else {
                    self.plan = P_NONE;
                }
            }
            P_STOMP => {
                if b.ground && !matches!(b.act, JUMP) && self.plan_t > 0.2 {
                    self.plan = P_NONE;
                }
                self.plan_t += dt;
            }
            P_TRIPLE => {
                if ready && b.chain_t < 0.12 && self.plan_n < 3 && b.spd > 1.0 {
                    press(&mut inp, &mut self.hold_t, 0.4);
                    self.plan_n += 1;
                } else if ready && b.chain_t > 0.3 {
                    self.plan = P_NONE;
                }
            }
            P_SIDEFLIP => {
                self.plan_t -= dt;
                if self.plan_t > 0.0 {
                    inp.mv = -dir;
                } else if b.act == SKID {
                    press(&mut inp, &mut self.hold_t, 0.4);
                    self.plan = P_NONE;
                } else if self.plan_t < -0.8 {
                    self.plan = P_NONE;
                }
            }
            P_BACKFLIP => {
                self.plan_t += dt;
                inp.mv = Vec3::ZERO;
                inp.crouch = true;
                if b.act == CROUCH && b.spd <= 2.2 {
                    press(&mut inp, &mut self.hold_t, 0.5);
                    self.plan = P_NONE;
                } else if self.plan_t > 1.0 {
                    self.plan = P_NONE;
                }
            }
            P_POUND => {
                self.plan_t += dt;
                if !b.ground && b.vy < 1.5 && matches!(b.act, JUMP) {
                    inp.crouch = true;
                    self.plan = P_NONE;
                } else if self.plan_t > 1.2 {
                    self.plan = P_NONE;
                }
            }
            _ => {}
        }

        if self.plan == P_NONE && self.cd <= 0.0 && ready {
            let r = gen_range(0.0, 1.0);
            if d < 1.5 && dy.abs() < 1.0 {
                if r < 0.4 {
                    inp.punch = true;
                    self.plan = P_PUNCH;
                    self.cd = 0.5;
                } else if r < 0.55 {
                    inp.crouch = true;
                    self.plan = P_BACKFLIP;
                    self.plan_t = 0.0;
                    self.cd = 1.2;
                } else if r < 0.75 {
                    press(&mut inp, &mut self.hold_t, 0.2);
                    self.plan = P_POUND;
                    self.plan_t = 0.0;
                    self.cd = 1.0;
                } else {
                    press(&mut inp, &mut self.hold_t, 0.35);
                    self.plan = P_STOMP;
                    self.plan_t = 0.0;
                    self.cd = 0.7;
                }
            } else if d < 3.8 {
                if r < 0.3 && b.spd > 2.5 {
                    inp.crouch = true;
                    inp.punch = true;
                    self.cd = 1.0;
                } else if r < 0.42 && b.spd >= 29.0 * K {
                    inp.punch = true;
                    self.cd = 1.2;
                } else if r < 0.75 {
                    press(&mut inp, &mut self.hold_t, 0.35);
                    self.plan = P_STOMP;
                    self.plan_t = 0.0;
                    self.cd = 0.6;
                } else {
                    self.cd = 0.3;
                }
            } else if d < 10.0 {
                self.cd = 0.4;
                if b.spd >= 29.0 * K && r < 0.25 {
                    inp.punch = true;
                    self.cd = 1.2;
                } else if b.spd > 5.0 && r < 0.5 {
                    inp.crouch = true;
                    press(&mut inp, &mut self.hold_t, 0.5);
                    self.cd = 1.2;
                } else if b.spd > 6.0 && r < 0.8 {
                    self.plan = P_TRIPLE;
                    self.plan_n = 1;
                    press(&mut inp, &mut self.hold_t, 0.4);
                } else if r < 0.88 {
                    self.plan = P_SIDEFLIP;
                    self.plan_t = 0.35;
                }
            } else {
                self.cd = 0.5;
                if r < 0.15 && b.spd > 6.0 {
                    self.plan = P_TRIPLE;
                    self.plan_n = 1;
                    press(&mut inp, &mut self.hold_t, 0.4);
                }
            }
            if dy > 0.6 && d < 4.0 && !inp.jump {
                press(&mut inp, &mut self.hold_t, 0.45);
            }
        }
        if b.blocked && ready && !inp.jump {
            press(&mut inp, &mut self.hold_t, 0.45);
        }
        if b.act == WALLHIT {
            press(&mut inp, &mut self.hold_t, 0.4);
        }
        if !b.ground {
            // No ar: mira em cima da cabeça; caindo por cima vira sentada
            inp.mv = if d < 0.35 { Vec3::ZERO } else { dir * (d * 0.6).min(1.0) };
            let head = tp.y + 1.85;
            if b.vy < 0.0 && d < 0.8 && b.pos.y > head + 0.7 && matches!(b.act, JUMP | BACKFLIP | SIDEFLIP | WALLKICK | FALL) {
                inp.crouch = true;
            } else if d < 1.5 && (b.pos.y - tp.y).abs() < 1.0 && matches!(b.act, JUMP | FALL) && gen_range(0.0, 1.0) < dt * 2.5 {
                inp.punch = true;
            }
        }
        inp.hold = inp.jump || self.hold_t > 0.0;
        inp
    }

    /// Lutadores com raiva do encanador vão pra cima dele (o resto da IA deles segue igual).
    fn brawl(&mut self, dt: f32, fighters: &mut [Fighter], npcs: &mut Npcs, events: &mut Vec<Ev>) {
        let n = fighters.len();
        self.aggro.resize(n, 0.0);
        self.chasing.resize(n, false);
        let me = self.body.pos;
        let on = self.spawned && self.body.act != DEAD;
        for (j, f) in fighters.iter_mut().enumerate() {
            self.aggro[j] -= dt;
            let flat = vec3(me.x - f.pos.x, 0.0, me.z - f.pos.z);
            let d = flat.length();
            if on && f.active() && d < 3.5 && (me.y - f.pos.y).abs() < 2.0 {
                self.aggro[j] = self.aggro[j].max(1.5);
            }
            let chase = self.chasing[j];
            let angry = on && f.active() && self.aggro[j] > 0.0 && d < 16.0;
            if !angry || !matches!(f.state, FState::Idle) && !(chase && matches!(f.state, FState::Dodge(..))) {
                if chase && matches!(f.state, FState::Dodge(..)) {
                    f.state = FState::Idle;
                }
                self.chasing[j] = false;
                continue;
            }
            let dir = flat.normalize_or(vec3(1.0, 0.0, 0.0));
            f.yaw = angle_lerp(f.yaw, dir.x.atan2(dir.z), dt * 12.0);
            let bz = if f.berserk > 0.0 { 1.5 } else { 1.0 };
            let facing = vec3(f.yaw.sin(), 0.0, f.yaw.cos()).dot(dir) > 0.5;
            if d > 1.6 {
                f.state = FState::Dodge(0.1, dir * f.speed * bz);
                f.walk += dt * f.speed * bz * 2.8;
                f.walk_amt = (f.walk_amt + dt * 5.0).min(1.0);
                self.chasing[j] = true;
            } else if f.atk_cd <= 0.0 && facing && self.body.invuln <= 0.0 {
                let kind = match (f.combo, f.wolverine) {
                    (0, _) => Atk::Jab,
                    (1, _) => Atk::Cross,
                    (2, true) => Atk::Combo3,
                    _ => Atk::Finisher,
                };
                let (hit, mult, knock) = match kind {
                    Atk::Jab => (0.13, 1.0, (2.5, 1.0)),
                    Atk::Cross => (0.15, 1.1, (3.0, 1.5)),
                    Atk::Combo3 => (0.17, 1.25, (3.5, 2.0)),
                    _ => (0.30, 1.9, (9.0, 6.0)),
                };
                let aspd = if f.wolverine { 1.35 * bz } else { 1.0 };
                f.state = FState::Attack { kind, t: 0.0, done: true };
                f.target = None;
                self.chasing[j] = false;
                self.swings.push(Swing { j, t: hit / aspd, dmg: f.dmg * mult * if f.berserk > 0.0 { 1.4 } else { 1.0 }, knock, claws: f.wolverine });
            } else {
                let perp = vec3(dir.z, 0.0, -dir.x) * if j % 2 == 0 { 1.0 } else { -1.0 };
                f.state = FState::Dodge(0.1, perp * 1.2 - dir * 0.3);
                f.walk += dt * 4.0;
                self.chasing[j] = true;
            }
        }
        let mut done = Vec::new();
        for (k, s) in self.swings.iter_mut().enumerate() {
            s.t -= dt;
            if s.t <= 0.0 {
                done.push(k);
            }
        }
        for k in done.into_iter().rev() {
            let s = self.swings.remove(k);
            let f = &fighters[s.j];
            let flat = vec3(me.x - f.pos.x, 0.0, me.z - f.pos.z);
            if f.active() && matches!(f.state, FState::Attack { .. }) && flat.length() < 2.2 && (me.y - f.pos.y).abs() < 1.6 {
                let knock = flat.normalize_or(Vec3::X) * s.knock.0 + Vec3::Y * s.knock.1;
                let name = f.name;
                let mut kos = fighters[s.j].kos;
                self.take_hit(npcs, s.dmg, knock, s.claws, name, &mut kos, events);
                fighters[s.j].kos = kos;
            }
        }
    }

    /// Host: vida, IA, golpes nos lutadores e revide deles.
    pub fn think(&mut self, world: &World, dt: f32, time: f32, fighters: &mut [Fighter], npcs: &mut Npcs, events: &mut Vec<Ev>) {
        let Some(life) = npcs.get(npc::MARIO, 0).copied() else { return };
        if !self.spawned {
            if time > 6.0 {
                self.spawn(events, true);
                self.last_hp = life.hp;
            }
            return;
        }
        if life.alive() && self.body.act == DEAD {
            self.spawn(events, false);
            self.last_hp = life.hp;
        } else if !life.alive() && self.body.act != DEAD {
            self.body.die();
            events.push(Ev::Banner("DERRUBARAM O ENCANADOR! O CANO DEVOLVE EM 45s".into()));
        } else if life.hp < self.last_hp - 0.01 && !matches!(self.body.act, KNOCK | DEAD) {
            self.body.hurt(-fwd(self.body.yaw) * 4.0, false);
        }
        self.last_hp = life.hp;

        let inp = if self.body.act == DEAD { Intent::default() } else { self.ai(dt, fighters) };
        let foes: Vec<Foe> = fighters.iter().enumerate().filter(|(_, f)| f.active()).map(|(j, f)| Foe { key: j as u32, c: f.pos + Vec3::Y, r: 0.85 }).collect();
        let strikes = self.body.update(world, dt, &inp, &foes);
        let up = Vec3::Y;
        for s in strikes {
            let j = s.key as usize;
            let f = &mut fighters[j];
            if !f.active() {
                continue;
            }
            f.hp -= s.dmg;
            f.vel += s.knock;
            f.flash = 1.0;
            f.last_hit = time;
            self.aggro.resize(j + 1, 0.0);
            self.aggro[j] = 8.0;
            events.push(Ev::Hit { pos: s.at, claws: false });
            events.push(Ev::Text { pos: f.pos + up * 2.2, text: format!("-{} {}!", s.dmg.round() as i32, s.w), color: if s.stomp { rgb(1.0, 0.85, 0.2) } else { WHITE }, big: s.stomp });
            if f.hp <= 0.0 {
                f.hp = 0.0;
                f.state = FState::Ko(if f.wolverine { 3.5 } else { 5.0 });
                f.ko_t = 0.0;
                events.push(Ev::Text { pos: f.pos + up * 2.8, text: "K.O.!".into(), color: rgb(1.0, 0.3, 0.2), big: true });
                events.push(Ev::Banner(format!("MARIO NOCAUTEOU {}", f.name)));
                self.emit(V_YAHOO, self.body.pos);
            } else {
                f.state = FState::Stun(if s.stomp { 0.8 } else { 0.35 });
            }
            self.emit(E_COIN, s.at);
        }
        if self.body.ground && self.body.act == GP_LAND && self.body.t < dt * 1.5 {
            events.push(Ev::Shake(0.25));
        }
        self.brawl(dt, fighters, npcs, events);
        for v in std::mem::take(&mut self.body.voice) {
            self.emit(v, self.body.pos);
        }
    }

    /// ENCANADOR jogável: mesmo corpo, golpes viram mensagens "a" (o host aplica).
    pub fn play(&mut self, world: &World, dt: f32, inp: &Intent, knock: Vec3, targets: &[crate::gta::Target]) -> Vec<Value> {
        let Some(mut b) = self.me.take() else { return Vec::new() };
        if knock.length() > 3.0 {
            b.hurt(knock, knock.length() > 8.0);
        }
        let foes: Vec<Foe> = targets.iter().enumerate().map(|(k, t)| Foe { key: k as u32, c: t.0, r: t.1.min(2.5) }).collect();
        let mut out = Vec::new();
        for s in b.update(world, dt, inp, &foes) {
            let t = targets[s.key as usize];
            let d = vec3(s.knock.x, 0.0, s.knock.z).normalize_or(fwd(b.yaw));
            out.push(json!({"t": "a", "k": "hit", "g": t.3, "i": t.2, "d": crate::mp::v3(d), "p": crate::mp::v3(s.at), "dmg": s.dmg, "w": s.w}));
            self.fx_local(E_COIN, s.at);
        }
        for v in std::mem::take(&mut b.voice) {
            self.fx_local(v, b.pos);
        }
        self.me = Some(b);
        out
    }

    pub fn snapshot(&mut self) -> Value {
        let b = &self.body;
        let r = |x: f32| (x as f64 * 100.0).round() / 100.0;
        json!({"b": [r(b.pos.x), r(b.pos.y), r(b.pos.z), r(b.yaw), b.act, b.aux, r(b.t), r(b.spd), r(b.vy), r(b.invuln), r(b.stars), self.spawned], "e": std::mem::take(&mut self.q)})
    }

    /// Cliente: estado do host (posição interpolada com a velocidade) + efeitos.
    pub fn apply(&mut self, v: &Value) {
        let Some(a) = v["b"].as_array().filter(|a| a.len() >= 12) else { return };
        if !a[11].as_bool().unwrap_or(false) {
            return;
        }
        let f = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
        let b = &mut self.body;
        b.yaw = f(3);
        let (act, aux) = (a[4].as_u64().unwrap_or(0) as u8, a[5].as_u64().unwrap_or(0) as u8);
        if act != b.act || aux != b.aux || (b.t - f(6)).abs() > 0.25 {
            b.t = f(6);
        }
        if act != b.act && act == GP_LAND {
            b.squash = 1.0;
        }
        (b.act, b.aux, b.spd, b.vy, b.invuln, b.stars) = (act, aux, f(7), f(8), f(9), f(10));
        let p = vec3(f(0), f(1), f(2));
        if self.net.is_none_or(|(q, _)| q.distance(p) > 8.0) {
            b.pos = p;
        }
        self.net = Some((p, fwd(b.yaw) * b.spd + Vec3::Y * b.vy));
        for e in v["e"].as_array().into_iter().flatten() {
            let g = |i: usize| e[i].as_f64().unwrap_or(0.0) as f32;
            self.fx_local(e[0].as_u64().unwrap_or(99) as u8, vec3(g(1), g(2), g(3)));
        }
    }

    /// Todos: anima (cliente segue o host), moedas, cano, partículas.
    pub fn animate(&mut self, world: &World, dt: f32, is_host: bool, fx: &mut Fx) {
        if !is_host {
            if let Some((p, v)) = self.net.as_mut() {
                *p += *v * dt;
                if v.y < 0.0 && world.solid_f(p.x, p.y - 0.05, p.z) {
                    v.y = 0.0;
                }
                let b = &mut self.body;
                b.pos = b.pos.lerp(*p, (dt * 12.0).min(1.0));
                b.t += dt;
                b.walk += b.spd.abs() * dt * 1.9;
                b.squash = (b.squash - dt * 6.0).max(0.0);
                b.stars = (b.stars - dt).max(0.0);
                b.invuln -= dt;
            }
        }
        for c in self.coins.iter_mut() {
            c.1.y -= 18.0 * dt;
            c.0 += c.1 * dt;
            c.2 += dt;
        }
        self.coins.retain(|c| c.2 < 0.8);
        if let Some((_, t)) = self.pipe.as_mut() {
            *t += dt;
        }
        self.pipe = self.pipe.filter(|p| p.1 < 4.0);
        fx.particles.append(&mut self.parts);
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, life: Option<&Vida>) {
        if let Some(m) = &self.me {
            draw_body(b, trans, m, 0.0, time);
        }
        for (p, _, t) in &self.coins {
            let m = Mat4::from_translation(*p) * Mat4::from_rotation_y(t * 14.0);
            b.glow(&m, Vec3::ZERO, vec3(0.34, 0.34, 0.07), Color::new(1.0, 0.82, 0.12, 1.0));
            b.glow(&m, vec3(0.0, 0.0, 0.04), vec3(0.07, 0.2, 0.02), Color::new(0.85, 0.55, 0.05, 1.0));
        }
        if let Some((p, t)) = self.pipe {
            draw_pipe(b, p, t);
        }
        if !self.visible() {
            return;
        }
        let flash = life.map_or(0.0, |l| l.flash);
        let head = draw_body(b, trans, &self.body, flash, time);
        let tag = head + Vec3::Y * 0.9;
        if self.body.act == DEAD {
            if self.body.t < 6.0 {
                labels.push(Label { pos: tag, text: "\"GAME OVER? SO NO 2o TURNO!\"".into(), size: 22.0, color: rgb(1.0, 0.4, 0.3) });
            }
            return;
        }
        labels.push(Label { pos: tag, text: "MARIO".into(), size: 24.0, color: rgb(1.0, 0.25, 0.2) });
        if time.rem_euclid(11.0) < 4.0 {
            let fala = FALAS[(time / 11.0) as usize % FALAS.len()];
            labels.push(Label { pos: tag + Vec3::Y * 0.7, text: format!("\"{fala}\""), size: 20.0, color: WHITE });
        }
    }
}

/// Jogador remoto de ENCANADOR: só pose de andar/pular a partir da posição.
pub fn draw_remote(b: &mut Batch, trans: &mut Batch, pos: Vec3, yaw: f32, walk: f32, moving: bool, vy: f32, time: f32) {
    let mut body = Body::new(pos, yaw);
    (body.act, body.aux, body.spd, body.walk, body.ground) = if vy.abs() > 1.5 { (JUMP, 1, 4.0, walk, false) } else if moving { (WALK, 0, 6.0, walk, true) } else { (IDLE, 0, 0.0, walk, true) };
    draw_body(b, trans, &body, 0.0, time);
}

/// Cano de chegada (original): verde-azulado de boca pra baixo no céu, com aro dourado e "U".
fn draw_pipe(b: &mut Batch, p: Vec3, t: f32) {
    let s = smooth(t / 0.4) * (1.0 - smooth((t - 3.3) / 0.6));
    if s <= 0.01 {
        return;
    }
    let m = Mat4::from_translation(p) * Mat4::from_scale(Vec3::splat(s));
    let body = rgb(0.1, 0.62, 0.55);
    let rim = rgb(0.18, 0.78, 0.68);
    let gold = Color::new(1.0, 0.82, 0.2, 1.0);
    b.cube(&m, vec3(0.0, 2.2, 0.0), vec3(1.9, 3.6, 1.9), body);
    b.cube(&m, vec3(0.0, 0.35, 0.0), vec3(2.5, 0.7, 2.5), rim);
    b.cube(&m, vec3(0.0, -0.01, 0.0), vec3(1.8, 0.04, 1.8), rgb(0.02, 0.05, 0.05));
    b.glow(&m, vec3(0.0, 1.5, 0.0), vec3(1.95, 0.25, 1.95), gold);
    for z in [1.0f32, -1.0] {
        let f = Mat4::from_rotation_y(if z > 0.0 { 0.0 } else { PI });
        let mm = m * f;
        b.glow(&mm, vec3(0.0, 2.5, 0.97), vec3(0.8, 0.8, 0.04), WHITE);
        b.cube(&mm, vec3(-0.22, 2.58, 0.99), vec3(0.14, 0.5, 0.04), rgb(0.85, 0.08, 0.08));
        b.cube(&mm, vec3(0.22, 2.58, 0.99), vec3(0.14, 0.5, 0.04), rgb(0.85, 0.08, 0.08));
        b.cube(&mm, vec3(0.0, 2.3, 0.99), vec3(0.58, 0.13, 0.04), rgb(0.85, 0.08, 0.08));
    }
}

#[derive(Default)]
struct MPose {
    lean: f32,
    pitch: f32,
    roll: f32,
    bounce: f32,
    crouch: f32,
    arm_l: f32,
    arm_r: f32,
    out_l: f32,
    out_r: f32,
    leg_l: f32,
    leg_r: f32,
    nod: f32,
}

fn pose(b: &Body, time: f32) -> MPose {
    let t = b.t;
    let run = (b.spd.abs() / RUN).min(1.0);
    let mut p = MPose { arm_l: -0.12, arm_r: -0.12, out_l: 0.15, out_r: 0.15, ..Default::default() };
    let tuck = |p: &mut MPose| {
        p.arm_l = -1.2;
        p.arm_r = -1.2;
        p.out_l = 0.3;
        p.out_r = 0.3;
        p.leg_l = -1.5;
        p.leg_r = -1.5;
    };
    match b.act {
        IDLE => {
            let s = (time * 2.2).sin() * 0.04;
            p.arm_l += s;
            p.arm_r -= s;
            p.bounce = s * 0.3;
        }
        WALK => {
            let s = b.walk.sin();
            let amp = 0.35 + 0.65 * run;
            p.leg_l = s * 0.95 * amp;
            p.leg_r = -s * 0.95 * amp;
            p.arm_l = -s * 0.9 * amp - 0.2 * run;
            p.arm_r = s * 0.9 * amp - 0.2 * run;
            p.lean = 0.28 * run;
            p.bounce = b.walk.sin().abs() * 0.07 * run;
        }
        SKID => {
            p.lean = -0.35;
            p.out_l = 1.2;
            p.out_r = 1.2;
            p.leg_l = -0.5;
            p.leg_r = 0.2;
        }
        CROUCH => {
            p.crouch = 1.0;
            p.lean = 0.25;
            p.arm_l = -0.5;
            p.arm_r = -0.5;
        }
        JUMP => match b.aux {
            3 => {
                let k = t / 0.8;
                p.pitch = TAU * smooth(k);
                if k < 1.0 {
                    tuck(&mut p);
                } else {
                    p.out_l = 1.4;
                    p.out_r = 1.4;
                    p.leg_l = -0.3;
                    p.leg_r = 0.3;
                }
            }
            2 => {
                p.arm_l = -2.7;
                p.arm_r = -2.7;
                p.out_l = 0.4;
                p.out_r = 0.4;
                p.leg_l = -0.5;
                p.leg_r = 0.3;
            }
            _ => {
                p.arm_r = -2.9;
                p.arm_l = -0.4;
                p.out_l = 0.5;
                p.leg_r = -0.9;
                p.leg_l = 0.2;
            }
        },
        BACKFLIP => {
            p.pitch = -TAU * smooth(t / 0.75);
            tuck(&mut p);
        }
        SIDEFLIP => {
            p.roll = TAU * smooth(t / 0.7);
            p.out_l = 1.4;
            p.out_r = 1.4;
            p.leg_l = -0.3;
            p.leg_r = 0.3;
        }
        LONGJUMP => {
            p.lean = 1.15;
            p.arm_l = -2.9;
            p.arm_r = -2.9;
            p.leg_l = 0.5;
            p.leg_r = 0.3;
        }
        WALLKICK => {
            p.arm_l = -2.6;
            p.arm_r = -2.6;
            p.out_l = 0.6;
            p.out_r = 0.6;
            p.leg_l = -0.6;
            p.leg_r = 0.4;
        }
        WALLHIT => {
            p.lean = -0.2;
            p.out_l = 1.0;
            p.out_r = 1.0;
            p.arm_l = -2.0;
            p.arm_r = -2.0;
        }
        FALL if b.aux == 1 => {
            let w = (time * 16.0).sin() * 0.35;
            p.arm_l = -2.8 + w;
            p.arm_r = -2.8 - w;
            p.leg_l = w;
            p.leg_r = -w;
        }
        FALL => {
            p.out_l = 1.0;
            p.out_r = 1.0;
            p.leg_l = -0.3;
            p.leg_r = 0.2;
        }
        GP_SPIN => {
            p.pitch = TAU * smooth(t / 0.3);
            tuck(&mut p);
        }
        GP_FALL => {
            p.leg_l = -1.6;
            p.leg_r = -1.6;
            p.arm_l = -0.6;
            p.arm_r = -0.6;
            p.out_l = 0.9;
            p.out_r = 0.9;
            p.lean = -0.15;
        }
        GP_LAND => {
            p.leg_l = -1.4;
            p.leg_r = -1.4;
            p.out_l = 1.3;
            p.out_r = 1.3;
            p.bounce = -0.35;
        }
        DIVE | BELLY => {
            p.lean = if b.act == BELLY { FRAC_PI_2 } else { 1.4 };
            p.bounce = if b.act == BELLY { 0.12 } else { 0.0 };
            p.arm_l = -3.0;
            p.arm_r = -3.0;
            p.leg_l = 0.15;
            p.leg_r = 0.15;
        }
        PUNCH => {
            let (hit, dur) = if b.aux == 2 { (0.14, 0.38) } else { (0.08, 0.22) };
            let ext = if t < hit { t / hit } else { (1.0 - (t - hit) / (dur - hit)).max(0.0) };
            match b.aux {
                0 => {
                    p.arm_r = -1.57 * ext - 0.2;
                    p.arm_l = -0.6;
                }
                1 => {
                    p.arm_l = -1.57 * ext - 0.2;
                    p.arm_r = -0.6;
                }
                _ => {
                    p.leg_r = -1.7 * ext;
                    p.lean = -0.2 * ext;
                    p.out_l = 0.9;
                    p.out_r = 0.9;
                }
            }
        }
        JUMPKICK => {
            let ext = (t / 0.08).min(1.0);
            p.leg_r = -1.7 * ext;
            p.leg_l = 0.3;
            p.lean = -0.3;
            p.out_l = 1.0;
            p.out_r = 1.0;
        }
        SLIDEKICK | SLIDE => {
            p.lean = -1.2;
            p.bounce = 0.15;
            p.leg_r = -0.4;
            p.leg_l = 0.6;
            p.out_l = 1.0;
            p.out_r = 1.0;
        }
        KNOCK => {
            let w = (time * 14.0).sin() * 0.3;
            p.lean = if b.ground && b.aux == 1 { -(t * 4.0).min(1.0) * 1.35 } else { -0.5 };
            p.arm_l = -2.4 + w;
            p.arm_r = -2.4 - w;
            p.out_l = 1.2;
            p.out_r = 1.2;
            p.leg_l = -0.6;
            p.leg_r = -0.2;
            p.nod = -0.3;
        }
        DEAD => {
            p.lean = -(t * 3.0).min(1.0) * FRAC_PI_2;
            p.out_l = 1.4;
            p.out_r = 1.4;
            p.arm_l = -0.2;
            p.arm_r = -0.2;
        }
        _ => {}
    }
    p.crouch = p.crouch.max(b.squash * 0.6);
    p
}

/// Modelo blocado original: retorna a posição da cabeça (estrelinhas/label).
fn draw_body(b: &mut Batch, trans: &mut Batch, body: &Body, flash: f32, time: f32) -> Vec3 {
    if body.invuln > 0.0 && body.act != DEAD && (body.invuln * 16.0) as i32 % 2 == 1 {
        let p = pose(body, time);
        return body.pos + Vec3::Y * (1.5 - 0.3 * p.crouch);
    }
    let p = pose(body, time);
    let tint = |c: Color| {
        let f = flash * 0.75;
        Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f * 0.6, c.b + (1.0 - c.b) * f * 0.6, 1.0)
    };
    let red = tint(rgb(0.86, 0.07, 0.07));
    let red_d = tint(rgb(0.68, 0.04, 0.04));
    let blue = tint(rgb(0.12, 0.22, 0.78));
    let skin = tint(rgb(1.0, 0.78, 0.6));
    let nose = tint(rgb(1.0, 0.68, 0.52));
    let shoe = tint(rgb(0.42, 0.22, 0.08));
    let hair = tint(rgb(0.27, 0.14, 0.05));
    let stache = tint(rgb(0.12, 0.06, 0.02));
    let glove = tint(rgb(0.97, 0.97, 0.97));
    let yellow = Color::new(1.0, 0.85, 0.12, 1.0);
    let t = |x: f32, y: f32, z: f32| Mat4::from_translation(vec3(x, y, z));
    let sq = p.crouch;
    let base = t(body.pos.x, body.pos.y + p.bounce.max(-0.4), body.pos.z) * Mat4::from_rotation_y(body.yaw) * Mat4::from_rotation_x(p.lean);
    let m = base * t(0.0, PIV, 0.0) * Mat4::from_rotation_x(p.pitch) * Mat4::from_rotation_z(p.roll) * t(0.0, -PIV, 0.0) * Mat4::from_scale(vec3(1.0 + 0.15 * sq, 1.0 - 0.3 * sq, 1.0 + 0.15 * sq));

    // Pernas: macacão azul e sapato marrom comprido
    for (side, ang) in [(1.0f32, p.leg_l), (-1.0, p.leg_r)] {
        let l = m * t(side * 2.2 * U, 7.0 * U, 0.0) * Mat4::from_rotation_x(ang);
        b.cube(&l, vec3(0.0, -2.5 * U, 0.0), vec3(3.8 * U, 5.0 * U, 3.8 * U), blue);
        b.cube(&l, vec3(0.0, -5.6 * U, 0.8 * U), vec3(4.2 * U, 2.8 * U, 5.8 * U), shoe);
    }
    // Tronco: barriga de macacão, camisa vermelha, peitilho, alças e botões amarelos
    b.cube(&m, vec3(0.0, 9.6 * U, 0.0), vec3(9.0 * U, 5.4 * U, 6.2 * U), blue);
    b.cube(&m, vec3(0.0, 14.2 * U, 0.0), vec3(8.6 * U, 4.2 * U, 5.6 * U), red);
    b.cube(&m, vec3(0.0, 13.2 * U, 2.9 * U), vec3(5.0 * U, 3.6 * U, 0.4 * U), blue);
    for side in [1.0f32, -1.0] {
        b.cube(&m, vec3(side * 2.6 * U, 14.8 * U, 2.9 * U), vec3(1.4 * U, 2.8 * U, 0.42 * U), blue);
        b.cube(&m, vec3(side * 2.6 * U, 14.4 * U, -2.9 * U), vec3(1.4 * U, 3.6 * U, 0.42 * U), blue);
        b.glow(&m, vec3(side * 2.6 * U, 13.4 * U, 3.15 * U), vec3(1.3 * U, 1.3 * U, 0.3 * U), yellow);
    }
    // Braços: manga vermelha, luva branca
    for (side, ang, out) in [(1.0f32, p.arm_l, p.out_l), (-1.0, p.arm_r, p.out_r)] {
        let a = m * t(side * 5.6 * U, 15.4 * U, 0.0) * Mat4::from_rotation_x(ang) * Mat4::from_rotation_z(side * out);
        b.cube(&a, vec3(0.0, -2.6 * U, 0.0), vec3(2.9 * U, 5.6 * U, 2.9 * U), red);
        b.cube(&a, vec3(0.0, -6.5 * U, 0.0), vec3(3.8 * U, 3.4 * U, 3.8 * U), glove);
    }
    // Cabeça: narigão, bigodão, costeletas, boné vermelho com "U" (de Urna)
    let h = m * t(0.0, 16.6 * U, 0.0) * Mat4::from_rotation_x(p.nod);
    b.cube(&h, vec3(0.0, 4.6 * U, 0.0), vec3(9.4 * U, 8.6 * U, 8.6 * U), skin);
    b.cube(&h, vec3(0.0, 3.7 * U, 4.9 * U), vec3(3.2 * U, 2.8 * U, 2.6 * U), nose);
    b.cube(&h, vec3(0.0, 2.1 * U, 4.6 * U), vec3(7.6 * U, 1.8 * U, 1.3 * U), stache);
    for side in [1.0f32, -1.0] {
        b.cube(&h, vec3(side * 3.4 * U, 1.4 * U, 4.4 * U), vec3(1.6 * U, 1.8 * U, 1.0 * U), stache);
        b.glow(&h, vec3(side * 1.9 * U, 6.0 * U, 4.35 * U), vec3(1.7 * U, 2.4 * U, 0.2 * U), WHITE);
        b.glow(&h, vec3(side * 1.6 * U, 5.8 * U, 4.46 * U), vec3(0.9 * U, 1.5 * U, 0.2 * U), rgb(0.1, 0.25, 0.75));
        b.cube(&h, vec3(side * 1.9 * U, 7.7 * U, 4.36 * U), vec3(2.1 * U, 0.6 * U, 0.2 * U), hair);
        b.cube(&h, vec3(side * 4.75 * U, 4.0 * U, 1.4 * U), vec3(0.4 * U, 3.4 * U, 2.4 * U), hair);
        b.cube(&h, vec3(side * 4.8 * U, 4.8 * U, -0.8 * U), vec3(0.6 * U, 2.2 * U, 1.6 * U), skin);
    }
    b.cube(&h, vec3(0.0, 3.8 * U, -4.45 * U), vec3(9.6 * U, 4.4 * U, 0.6 * U), hair);
    b.cube(&h, vec3(0.0, 9.3 * U, -0.3 * U), vec3(10.0 * U, 2.6 * U, 9.8 * U), red);
    b.cube(&h, vec3(0.0, 10.9 * U, -0.6 * U), vec3(8.4 * U, 1.2 * U, 8.0 * U), red);
    b.cube(&h, vec3(0.0, 8.3 * U, 5.3 * U), vec3(9.4 * U, 0.7 * U, 3.4 * U), red_d);
    b.glow(&h, vec3(0.0, 9.6 * U, 4.72 * U), vec3(3.6 * U, 2.4 * U, 0.2 * U), WHITE);
    for x in [-0.9f32, 0.9] {
        b.cube(&h, vec3(x * U, 9.8 * U, 4.85 * U), vec3(0.6 * U, 1.7 * U, 0.2 * U), red);
    }
    b.cube(&h, vec3(0.0, 9.0 * U, 4.85 * U), vec3(2.4 * U, 0.5 * U, 0.2 * U), red);

    let head = h.transform_point3(vec3(0.0, 5.0 * U, 0.0));
    // Estrelinhas girando depois de levar pancada
    if body.stars > 0.0 {
        for k in 0..3 {
            let a = time * 7.0 + k as f32 * TAU / 3.0;
            let c = head + vec3(a.cos() * 0.45, 0.45 + (time * 9.0 + k as f32).sin() * 0.05, a.sin() * 0.45);
            let sm = Mat4::from_translation(c) * Mat4::from_rotation_y(time * 5.0);
            trans.glow(&sm, Vec3::ZERO, vec3(0.16, 0.05, 0.05), yellow);
            trans.glow(&sm, Vec3::ZERO, vec3(0.05, 0.16, 0.05), yellow);
            trans.glow(&(sm * Mat4::from_rotation_z(FRAC_PI_2 * 0.5)), Vec3::ZERO, vec3(0.13, 0.04, 0.04), WHITE);
        }
    }
    head
}
