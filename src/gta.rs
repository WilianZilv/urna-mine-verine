//! Personagem "bandido" no clima do GTA 3 (tudo original, nada de código/asset do jogo): jaqueta de couro,
//! jeans, cabelo curto; câmera por cima do ombro, mira com trava no alvo, arsenal completo, procurado,
//! e o sedã da primeira missão estacionado perto da torre (fumaça quando apanha, explode quando morre).

use crate::batch::Batch;
use crate::models::rgb;
use crate::urna::limb;
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{FRAC_PI_2, PI};

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
    /// Pente (0 = sem munição: soco/taco).
    pub clip: u32,
    /// Coice na câmera (radianos).
    kick: f32,
    len: f32,
    col: Color,
}

const fn c(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

#[allow(clippy::too_many_arguments)]
const fn w(name: &'static str, kind: Kind, dmg: f32, rate: f32, pellets: u32, spread: f32, range: f32, auto: bool, clip: u32, kick: f32, len: f32, col: Color) -> Weapon {
    Weapon { name, kind, dmg, rate, pellets, spread, range, auto, clip, kick, len, col }
}

pub const ARSENAL: [Weapon; 12] = [
    w("PUNHO", Kind::Melee, 6.0, 0.4, 1, 0.0, 2.2, false, 0, 0.0, 0.0, c(0.0, 0.0, 0.0)),
    w("TACO DE BEISEBOL", Kind::Melee, 14.0, 0.6, 1, 0.0, 2.8, false, 0, 0.0, 0.9, c(0.6, 0.4, 0.2)),
    w("PISTOLA", Kind::Gun, 14.0, 0.3, 1, 0.012, 60.0, false, 12, 0.035, 0.25, c(0.15, 0.15, 0.16)),
    w("UZI", Kind::Gun, 8.0, 0.08, 1, 0.045, 45.0, true, 30, 0.012, 0.35, c(0.12, 0.12, 0.13)),
    w("ESCOPETA", Kind::Gun, 8.0, 0.9, 8, 0.09, 25.0, false, 8, 0.09, 0.8, c(0.35, 0.22, 0.12)),
    w("AK-47", Kind::Gun, 12.0, 0.11, 1, 0.025, 80.0, true, 30, 0.018, 0.85, c(0.45, 0.28, 0.12)),
    w("M16", Kind::Gun, 11.0, 0.08, 1, 0.02, 90.0, true, 30, 0.014, 0.9, c(0.1, 0.1, 0.1)),
    w("SNIPER", Kind::Gun, 90.0, 1.3, 1, 0.0, 160.0, false, 5, 0.08, 1.1, c(0.25, 0.3, 0.2)),
    w("LANCA-FOGUETE", Kind::Rocket, 0.0, 1.4, 1, 0.0, 0.0, false, 1, 0.1, 1.2, c(0.3, 0.38, 0.22)),
    w("LANCA-CHAMAS", Kind::Flame, 4.0, 0.2, 1, 0.0, 7.0, true, 0, 0.0, 0.9, c(0.6, 0.15, 0.1)),
    w("GRANADA", Kind::Grenade, 0.0, 0.9, 1, 0.0, 0.0, false, 0, 0.0, 0.0, c(0.25, 0.35, 0.2)),
    w("MOLOTOV", Kind::Molotov, 0.0, 0.9, 1, 0.0, 0.0, false, 0, 0.0, 0.0, c(0.4, 0.6, 0.3)),
];

const SKIN: Color = c(0.82, 0.64, 0.5);
const HAIR: Color = c(0.13, 0.09, 0.06);
const JACKET: Color = c(0.12, 0.1, 0.09);
const JACKET_HI: Color = c(0.2, 0.17, 0.15);
const SHIRT: Color = c(0.24, 0.27, 0.23);
const JEANS: Color = c(0.2, 0.28, 0.45);
const JEANS_DK: Color = c(0.15, 0.21, 0.35);
const SHOES: Color = c(0.06, 0.05, 0.05);

/// Alvo atingível: (centro, raio, índice, grupo `npc::*`).
pub type Target = (Vec3, f32, usize, u8);

pub enum Out {
    Hit { i: usize, g: u8, dmg: f32, dir: Vec3, at: Vec3 },
    Boom(Vec3, f32),
    Spark(Vec3),
    Bang(Vec3),
    Casing(Vec3, Vec3),
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
    flash_rot: f32,
    swing: f32,
    flame: f32,
    projs: Vec<Proj>,
    tracers: Vec<(Vec3, Vec3, f32)>,
    pub driving: bool,
    pub hit_cd: f32,
    /// Botão direito: mira por cima do ombro (trava no alvo mais perto da mira).
    pub aiming: bool,
    pub lock: Option<(u8, usize)>,
    /// Tempo desde o último tiro (corpo vira pra mira enquanto > 0).
    fired: f32,
    kick: f32,
    kick_debt: f32,
    pub body_yaw: f32,
    clip: [u32; 12],
    pub reload: f32,
    pub health: f32,
    pub cash: u32,
    /// Nível de procurado contínuo (estrelas = parte inteira, até 6).
    pub heat: f32,
    /// Segundos sem cometer crime.
    pub calm: f32,
    /// > 0 = WASTED (segundos até renascer).
    pub wasted: f32,
}

impl Bandido {
    pub fn new(yaw: f32) -> Self {
        Bandido {
            weapon: 2,
            cool: 0.0,
            flash: 0.0,
            flash_rot: 0.0,
            swing: 0.0,
            flame: 0.0,
            projs: Vec::new(),
            tracers: Vec::new(),
            driving: false,
            hit_cd: 0.0,
            aiming: false,
            lock: None,
            fired: 0.0,
            kick: 0.0,
            kick_debt: 0.0,
            body_yaw: yaw,
            clip: ARSENAL.map(|w| w.clip),
            reload: 0.0,
            health: 100.0,
            cash: 0,
            heat: 0.0,
            calm: 0.0,
            wasted: 0.0,
        }
    }

    pub fn cycle(&mut self, d: i32) {
        if d != 0 {
            self.weapon = (self.weapon as i32 + d).rem_euclid(ARSENAL.len() as i32) as usize;
            self.reload = 0.0;
            self.lock = None;
        }
    }

    pub fn stars(&self) -> u32 {
        (self.heat.floor() as u32).min(6)
    }

    /// Crime cometido: sobe o procurado e zera o tempo de esfriar.
    pub fn crime(&mut self, amount: f32) {
        self.heat = (self.heat + amount).min(6.99);
        self.calm = 0.0;
    }

    pub fn hurt(&mut self, dmg: f32) {
        if self.wasted <= 0.0 {
            self.health -= dmg;
            if self.health <= 0.0 {
                self.health = 0.0;
                self.wasted = 4.0;
                self.driving = false;
            }
        }
    }

    pub fn ammo_text(&self) -> String {
        let wpn = &ARSENAL[self.weapon];
        match wpn.kind {
            _ if self.reload > 0.0 => "RECARREGANDO".into(),
            Kind::Melee => String::new(),
            Kind::Gun | Kind::Rocket => format!("{}-{}", self.clip[self.weapon], 9999 - self.clip[self.weapon]),
            _ => "9999".into(),
        }
    }

    /// Variação de pitch da câmera neste frame: sobe no tiro e volta sozinho pra mira.
    pub fn take_kick(&mut self, dt: f32) -> f32 {
        let k = std::mem::take(&mut self.kick);
        self.kick_debt += k;
        let back = self.kick_debt * (dt * 9.0).min(1.0);
        self.kick_debt -= back;
        k - back
    }

    /// Escolhe o alvo travado: o mais perto do centro da mira, visível e no alcance.
    pub fn pick_lock(&mut self, world: &World, eye: Vec3, aim: Vec3, targets: &[Target], cone: f32) {
        let range = match ARSENAL[self.weapon].range {
            r if r > 0.0 => r.max(8.0),
            _ => 40.0,
        };
        if let Some(l) = self.lock {
            let keep = targets.iter().find(|t| (t.3, t.2) == l).is_some_and(|t| t.0.distance(eye) < range * 1.3);
            if keep {
                return;
            }
        }
        let mut best: Option<(f32, (u8, usize))> = None;
        for &(p, _, i, g) in targets {
            let d = p - eye;
            let dist = d.length();
            let cos = aim.dot(d / dist.max(0.01));
            if dist > range || cos < cone || world.raycast(eye, d / dist, dist).is_some() {
                continue;
            }
            let score = (1.0 - cos) * 40.0 + dist * 0.05;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, (g, i)));
            }
        }
        self.lock = best.map(|b| b.1);
    }

    pub fn locked_pos(&self, targets: &[Target]) -> Option<Vec3> {
        let l = self.lock?;
        targets.iter().find(|t| (t.3, t.2) == l).map(|t| t.0)
    }

    /// Dispara a arma atual. `eye`/`aim` = raio da mira (câmera); `muzzle` = cano da arma.
    #[allow(clippy::too_many_arguments)]
    pub fn fire(&mut self, world: &World, eye: Vec3, aim: Vec3, muzzle: Vec3, body: Vec3, targets: &[Target], out: &mut Vec<Out>) {
        if self.cool > 0.0 || self.reload > 0.0 || self.wasted > 0.0 {
            return;
        }
        let wpn = &ARSENAL[self.weapon];
        self.cool = wpn.rate;
        self.fired = 1.2;
        self.kick += wpn.kick;
        let flat = vec3(aim.x, 0.0, aim.z).normalize_or_zero();
        if flat != Vec3::ZERO {
            self.body_yaw = flat.x.atan2(flat.z);
        }
        if wpn.clip > 0 {
            self.clip[self.weapon] -= 1;
            if self.clip[self.weapon] == 0 {
                self.clip[self.weapon] = wpn.clip;
                self.reload = if wpn.kind == Kind::Rocket { 1.2 } else { 1.0 };
            }
        }
        // Com alvo travado, a mira vai no peito dele (igual a trava automática clássica)
        let (eye, aim) = match self.locked_pos(targets) {
            Some(p) => (muzzle, (p - muzzle).normalize_or(aim)),
            None => (eye, aim),
        };
        match wpn.kind {
            Kind::Melee => {
                self.swing = 1.0;
                for &(p, r, i, g) in targets {
                    let d = p - body;
                    if d.length() < wpn.range + r && flat.dot(d.normalize_or_zero()) > 0.5 {
                        out.push(Out::Hit { i, g, dmg: wpn.dmg, dir: flat, at: p });
                        break;
                    }
                }
            }
            Kind::Gun => {
                self.flash = 0.06;
                self.flash_rot = gen_range(0.0, PI);
                out.push(Out::Bang(muzzle));
                let right = vec3(-flat.z, 0.0, flat.x);
                out.push(Out::Casing(muzzle - aim * wpn.len * 0.5, right * gen_range(2.0, 3.5) + Vec3::Y * gen_range(2.0, 3.0)));
                for _ in 0..wpn.pellets {
                    let d = (aim + vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)) * wpn.spread).normalize();
                    let wall = world.raycast(eye, d, wpn.range).map(|h| h.2).unwrap_or(wpn.range);
                    let mut best: Option<(f32, usize, u8)> = None;
                    for &(p, r, i, g) in targets {
                        if let Some(t) = crate::urna::ray_sphere(eye, d, p, r) {
                            if t < wall && best.is_none_or(|b| t < b.0) {
                                best = Some((t, i, g));
                            }
                        }
                    }
                    let end = eye + d * best.map(|b| b.0).unwrap_or(wall);
                    self.tracers.push((muzzle, end, 0.05));
                    match best {
                        Some((_, i, g)) => out.push(Out::Hit { i, g, dmg: wpn.dmg, dir: vec3(d.x, 0.0, d.z).normalize_or_zero(), at: end }),
                        None if wall < wpn.range => out.push(Out::Spark(end)),
                        None => {}
                    }
                }
            }
            Kind::Flame => {
                self.flame = 0.25;
                for &(p, r, i, g) in targets {
                    let d = p - muzzle;
                    if d.length() < wpn.range + r && aim.dot(d.normalize_or_zero()) > 0.85 {
                        out.push(Out::Hit { i, g, dmg: wpn.dmg, dir: flat, at: p });
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

    /// `moved` = deslocamento no chão neste frame (corpo vira pra onde anda, ou pra mira).
    pub fn update(&mut self, world: &World, dt: f32, targets: &[Target], out: &mut Vec<Out>, aim: Vec3, moved: Vec3) {
        self.cool -= dt;
        self.flash -= dt;
        self.hit_cd -= dt;
        self.fired -= dt;
        self.reload = (self.reload - dt).max(0.0);
        self.swing = (self.swing - dt * 4.0).max(0.0);
        self.flame = (self.flame - dt).max(0.0);
        self.calm += dt;
        if self.calm > 12.0 {
            self.heat = (self.heat - dt * 0.08).max(0.0);
        }
        if self.wasted <= 0.0 && self.health < 100.0 && self.calm > 6.0 {
            self.health = (self.health + dt * 2.0).min(100.0);
        }
        let want = if self.aiming || self.fired > 0.0 {
            Some(aim.x.atan2(aim.z))
        } else if vec2(moved.x, moved.z).length() > 0.002 {
            Some(moved.x.atan2(moved.z))
        } else {
            None
        };
        if let Some(y) = want {
            self.body_yaw = crate::actors::angle_lerp(self.body_yaw, y, (dt * 14.0).min(1.0));
        }
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

    fn aim_pose(&self) -> bool {
        self.aiming || self.fired > 0.0 || ARSENAL[self.weapon].kind == Kind::Flame && self.flame > 0.0
    }

    /// Mão direita (em mundo) e direção da arma.
    fn hand(&self, pos: Vec3, aim: Vec3) -> (Vec3, Vec3) {
        let f = vec3(self.body_yaw.sin(), 0.0, self.body_yaw.cos());
        let right = vec3(-f.z, 0.0, f.x);
        let sh = pos + Vec3::Y * 1.42 + right * 0.24;
        if self.aim_pose() && ARSENAL[self.weapon].kind != Kind::Melee {
            (sh + aim * 0.52 - right * 0.06, aim)
        } else {
            let dir = (f * 0.55 - Vec3::Y * 0.8).normalize();
            (sh - Vec3::Y * 0.55 + f * 0.12, dir)
        }
    }

    pub fn muzzle(&self, pos: Vec3, aim: Vec3) -> Vec3 {
        let (hand, dir) = self.hand(pos, aim);
        hand + dir * (ARSENAL[self.weapon].len * 0.9 + 0.12)
    }

    /// Bandido a pé: corpo, arma na mão, projéteis, traçantes, clarão e chamas.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, pos: Vec3, aim: Vec3, walk: f32, moving: bool, time: f32) {
        let wpn = &ARSENAL[self.weapon];
        let lean = if self.wasted > 0.0 { ((4.0 - self.wasted) * 3.0).min(1.0) * FRAC_PI_2 } else { 0.0 };
        let (hand, dir) = self.hand(pos, aim);
        let throw = matches!(wpn.kind, Kind::Grenade | Kind::Molotov);
        let f = vec3(self.body_yaw.sin(), 0.0, self.body_yaw.cos());
        let (hand, dir, arm_r) = if self.swing > 0.0 && (wpn.kind == Kind::Melee || throw) {
            let a = -PI * (0.25 + 0.75 * self.swing);
            let d = (f * -a.sin() + Vec3::Y * -a.cos()) * 0.55;
            let sh = pos + Vec3::Y * 1.42 + vec3(-f.z, 0.0, f.x) * 0.24;
            (sh + d, d.normalize(), Arm::Point(d))
        } else if lean > 0.0 || (wpn.kind == Kind::Melee && wpn.len == 0.0 && !self.aim_pose()) {
            (hand, dir, Arm::Swing)
        } else {
            (hand, dir, Arm::Hand(hand))
        };
        let two_hands = wpn.len > 0.5 && wpn.kind != Kind::Melee && self.aim_pose();
        let arm_l = if two_hands { Arm::Hand(hand + dir * wpn.len * 0.45) } else { Arm::Swing };
        draw_body(b, pos, self.body_yaw, walk, if moving { 1.0 } else { 0.0 }, lean, arm_r, arm_l, 0.0);
        if lean == 0.0 {
            let tip = self.draw_weapon(b, hand, dir, !matches!(arm_r, Arm::Swing));
            if self.flash > 0.0 {
                let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, dir) * Quat::from_rotation_z(self.flash_rot), tip);
                trans.glow(&m, Vec3::ZERO, vec3(0.5, 0.08, 0.08), Color::new(1.0, 0.9, 0.4, 0.95));
                trans.glow(&m, Vec3::ZERO, vec3(0.08, 0.5, 0.08), Color::new(1.0, 0.9, 0.4, 0.95));
                trans.glow(&m, vec3(0.0, 0.0, 0.2), vec3(0.22, 0.22, 0.45), Color::new(1.0, 0.6, 0.15, 0.8));
                trans.glow(&Mat4::from_translation(tip), Vec3::ZERO, Vec3::splat(0.7), Color::new(1.0, 0.75, 0.3, 0.25));
            }
            if self.flame > 0.0 {
                for k in 0..14 {
                    let f = ((time * 6.0 + k as f32 * 0.37).fract()) * 6.5;
                    let p = tip + dir * f + vec3((k as f32 * 3.1 + time * 9.0).sin(), (k as f32 * 1.7 + time * 7.0).cos(), (k as f32 * 2.3).sin()) * f * 0.12;
                    trans.glow(&Mat4::from_translation(p), Vec3::ZERO, Vec3::splat(0.25 + f * 0.12), Color::new(1.0, 0.35 + 0.4 * (1.0 - f / 6.5), 0.05, 0.75));
                }
            }
        }
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
            let k = (t / 0.05).min(1.0);
            let head = a.lerp(c, 1.0 - k * 0.6);
            crate::urna::beam(trans, a.lerp(c, 0.15), head, 0.025, Color::new(1.0, 0.95, 0.6, k));
        }
    }

    /// Desenha a arma na mão; retorna a ponta do cano.
    fn draw_weapon(&self, b: &mut Batch, hand: Vec3, dir: Vec3, in_hand: bool) -> Vec3 {
        let wpn = &ARSENAL[self.weapon];
        let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, dir), hand);
        let dark = rgb(0.08, 0.08, 0.09);
        let wood = rgb(0.42, 0.26, 0.12);
        match wpn.kind {
            _ if !in_hand => {}
            Kind::Melee if wpn.len > 0.0 => {
                b.cube(&m, vec3(0.0, 0.0, 0.15), vec3(0.06, 0.06, 0.35), rgb(0.15, 0.1, 0.08));
                b.cube(&m, vec3(0.0, 0.0, 0.6), vec3(0.1, 0.1, 0.6), wpn.col);
            }
            Kind::Melee => {}
            Kind::Grenade => {
                b.cube(&m, Vec3::ZERO, Vec3::splat(0.14), wpn.col);
                b.cube(&m, vec3(0.0, 0.09, 0.0), vec3(0.05, 0.05, 0.08), rgb(0.6, 0.6, 0.55));
            }
            Kind::Molotov => {
                b.cube(&m, vec3(0.0, 0.0, 0.05), vec3(0.11, 0.11, 0.24), wpn.col);
                b.cube(&m, vec3(0.0, 0.0, 0.22), vec3(0.05, 0.05, 0.12), wpn.col);
                b.glow(&m, vec3(0.0, 0.0, 0.3), Vec3::splat(0.07), rgb(1.0, 0.6, 0.1));
            }
            Kind::Rocket => {
                b.cube(&m, vec3(0.0, 0.12, 0.1), vec3(0.2, 0.2, wpn.len), wpn.col);
                b.cube(&m, vec3(0.0, 0.12, 0.1 + wpn.len * 0.5), vec3(0.26, 0.26, 0.08), dark);
                b.cube(&m, vec3(0.0, 0.12, 0.1 - wpn.len * 0.5), vec3(0.24, 0.24, 0.06), dark);
                b.cube(&m, vec3(0.0, -0.05, 0.0), vec3(0.05, 0.16, 0.08), dark);
                b.cube(&m, vec3(-0.12, 0.22, 0.05), vec3(0.05, 0.08, 0.2), dark);
            }
            Kind::Flame => {
                b.cube(&m, vec3(0.0, 0.0, 0.35), vec3(0.08, 0.08, 0.8), rgb(0.4, 0.4, 0.42));
                b.cube(&m, vec3(0.0, -0.12, 0.15), vec3(0.18, 0.18, 0.35), wpn.col);
                b.cube(&m, vec3(0.0, -0.05, 0.0), vec3(0.05, 0.16, 0.08), dark);
            }
            Kind::Gun => match self.weapon {
                2 => {
                    b.cube(&m, vec3(0.0, 0.04, 0.08), vec3(0.06, 0.08, 0.26), wpn.col);
                    b.cube(&m, vec3(0.0, -0.06, -0.01), vec3(0.055, 0.14, 0.07), dark);
                }
                3 => {
                    b.cube(&m, vec3(0.0, 0.04, 0.1), vec3(0.08, 0.12, 0.32), wpn.col);
                    b.cube(&m, vec3(0.0, -0.08, 0.0), vec3(0.06, 0.18, 0.06), dark);
                    b.cube(&m, vec3(0.0, -0.12, 0.14), vec3(0.05, 0.22, 0.06), dark);
                    b.cube(&m, vec3(0.0, 0.05, 0.3), vec3(0.03, 0.03, 0.1), dark);
                }
                4 => {
                    b.cube(&m, vec3(0.0, 0.03, 0.35), vec3(0.06, 0.06, 0.75), dark);
                    b.cube(&m, vec3(0.0, -0.03, 0.3), vec3(0.08, 0.06, 0.35), wood);
                    b.cube(&m, vec3(0.0, -0.04, -0.15), vec3(0.07, 0.12, 0.3), wood);
                }
                5 => {
                    b.cube(&m, vec3(0.0, 0.03, 0.25), vec3(0.07, 0.1, 0.55), dark);
                    b.cube(&m, vec3(0.0, 0.03, 0.62), vec3(0.03, 0.03, 0.25), dark);
                    b.cube(&m, vec3(0.0, 0.0, 0.35), vec3(0.08, 0.08, 0.2), wood);
                    b.cube(&m, vec3(0.0, -0.15, 0.15), vec3(0.06, 0.2, 0.08), dark);
                    b.cube(&m, vec3(0.0, -0.02, -0.18), vec3(0.06, 0.12, 0.3), wood);
                }
                6 => {
                    b.cube(&m, vec3(0.0, 0.03, 0.25), vec3(0.07, 0.1, 0.6), wpn.col);
                    b.cube(&m, vec3(0.0, 0.03, 0.65), vec3(0.03, 0.03, 0.25), dark);
                    b.cube(&m, vec3(0.0, 0.12, 0.15), vec3(0.04, 0.06, 0.25), dark);
                    b.cube(&m, vec3(0.0, -0.13, 0.12), vec3(0.05, 0.18, 0.07), dark);
                    b.cube(&m, vec3(0.0, -0.02, -0.2), vec3(0.06, 0.12, 0.3), wpn.col);
                }
                _ => {
                    b.cube(&m, vec3(0.0, 0.03, 0.35), vec3(0.06, 0.08, 0.95), wpn.col);
                    b.cube(&m, vec3(0.0, 0.13, 0.15), vec3(0.07, 0.07, 0.35), dark);
                    b.cube(&m, vec3(0.0, -0.03, -0.18), vec3(0.06, 0.13, 0.32), wpn.col);
                }
            },
        }
        hand + dir * (wpn.len * 0.9 + 0.12)
    }
}

/// Braço: solto balançando, mão num ponto do mundo, ou apontando numa direção (relativa ao ombro).
#[derive(Clone, Copy)]
pub enum Arm {
    Swing,
    Hand(Vec3),
    Point(Vec3),
}

/// Corpo do bandido (proporção mais "gente" que o Steve): jaqueta de couro aberta, camisa,
/// jeans, botina, cabelo curto e cara fechada. `lean` = caído de costas.
#[allow(clippy::too_many_arguments)]
pub fn draw_body(b: &mut Batch, pos: Vec3, yaw: f32, walk: f32, walk_amt: f32, lean: f32, arm_r: Arm, arm_l: Arm, flash: f32) {
    let tint = |c: Color| Color::new(c.r + (1.0 - c.r) * flash * 0.7, c.g * (1.0 - flash * 0.3), c.b * (1.0 - flash * 0.3), 1.0);
    let root = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-lean);
    let bob = (walk * 2.0).sin().abs() * 0.03 * walk_amt;
    let body = root * Mat4::from_translation(vec3(0.0, bob, 0.0));
    // Pernas: coxa + canela com joelho dobrando na volta do passo
    for (side, ph) in [(1.0f32, 0.0f32), (-1.0, PI)] {
        let s = (walk + ph).sin() * walk_amt;
        let knee = ((walk + ph).cos() * walk_amt).max(0.0) * 0.9;
        let hip = body * Mat4::from_translation(vec3(side * 0.1, 0.92, 0.0)) * Mat4::from_rotation_x(s * 0.6);
        b.cube(&hip, vec3(0.0, -0.22, 0.0), vec3(0.17, 0.46, 0.19), tint(JEANS));
        let shin = hip * Mat4::from_translation(vec3(0.0, -0.44, 0.0)) * Mat4::from_rotation_x(knee);
        b.cube(&shin, vec3(0.0, -0.2, 0.0), vec3(0.16, 0.42, 0.17), tint(JEANS_DK));
        b.cube(&shin, vec3(0.0, -0.42, 0.04), vec3(0.17, 0.09, 0.27), SHOES);
    }
    // Cinto, camisa, jaqueta aberta com gola
    b.cube(&body, vec3(0.0, 0.95, 0.0), vec3(0.4, 0.07, 0.22), rgb(0.07, 0.06, 0.05));
    b.cube(&body, vec3(0.0, 0.95, 0.115), vec3(0.07, 0.05, 0.01), rgb(0.7, 0.65, 0.4));
    b.cube(&body, vec3(0.0, 1.2, 0.0), vec3(0.36, 0.48, 0.2), tint(SHIRT));
    for side in [1.0f32, -1.0] {
        b.cube(&body, vec3(side * 0.13, 1.18, 0.0), vec3(0.16, 0.56, 0.24), tint(JACKET));
        b.cube(&body, vec3(side * 0.09, 1.38, 0.115), vec3(0.08, 0.16, 0.02), tint(JACKET_HI));
        b.cube(&body, vec3(side * 0.12, 1.46, -0.02), vec3(0.12, 0.08, 0.24), tint(JACKET_HI));
    }
    b.cube(&body, vec3(0.0, 1.18, -0.06), vec3(0.42, 0.56, 0.13), tint(JACKET));
    // Braços
    let right = body.transform_vector3(Vec3::X);
    for (side, arm) in [(-1.0f32, arm_r), (1.0, arm_l)] {
        let sh = body.transform_point3(vec3(side * 0.25, 1.42, 0.0));
        let tip = match arm {
            Arm::Hand(p) => sh + (p - sh).clamp_length_max(0.6),
            Arm::Point(d) => sh + d,
            Arm::Swing => {
                let s = (walk + if side < 0.0 { PI } else { 0.0 }).sin() * walk_amt * 0.5;
                body.transform_point3(vec3(side * 0.29, 1.42 - 0.56 * s.cos(), 0.56 * s.sin()))
            }
        };
        let elbow = sh.lerp(tip, 0.5) - Vec3::Y * 0.04 + right * side * 0.03;
        limb(b, sh, elbow, 0.13, tint(JACKET));
        limb(b, elbow, tip, 0.12, tint(JACKET));
        b.cube(&Mat4::from_translation(tip), Vec3::ZERO, Vec3::splat(0.1), tint(SKIN));
    }
    // Pescoço e cabeça
    b.cube(&body, vec3(0.0, 1.5, 0.0), vec3(0.11, 0.08, 0.11), tint(SKIN));
    let head = body * Mat4::from_translation(vec3(0.0, 1.66, 0.0));
    b.cube(&head, Vec3::ZERO, vec3(0.22, 0.26, 0.24), tint(SKIN));
    b.cube(&head, vec3(0.0, -0.08, 0.02), vec3(0.225, 0.1, 0.23), tint(c(0.68, 0.53, 0.42)));
    b.cube(&head, vec3(0.0, 0.125, -0.01), vec3(0.235, 0.05, 0.25), HAIR);
    b.cube(&head, vec3(0.0, 0.05, -0.11), vec3(0.235, 0.16, 0.04), HAIR);
    for side in [1.0f32, -1.0] {
        b.cube(&head, vec3(side * 0.112, 0.06, -0.03), vec3(0.02, 0.12, 0.16), HAIR);
        b.cube(&head, vec3(side * 0.115, 0.0, -0.01), vec3(0.03, 0.06, 0.05), tint(SKIN));
        b.cube(&head, vec3(side * 0.05, 0.045, 0.121), vec3(0.07, 0.018, 0.01), HAIR);
        b.glow(&head, vec3(side * 0.05, 0.018, 0.121), vec3(0.04, 0.025, 0.01), rgb(0.1, 0.08, 0.06));
    }
    b.cube(&head, vec3(0.0, -0.01, 0.13), vec3(0.04, 0.06, 0.03), tint(c(0.76, 0.58, 0.45)));
    b.cube(&head, vec3(0.0, -0.07, 0.122), vec3(0.07, 0.012, 0.01), rgb(0.35, 0.18, 0.15));
}

/// Bandido de outro jogador (pistola na mão, mira pra onde olha).
pub fn draw_remote(b: &mut Batch, pos: Vec3, yaw: f32, walk: f32, moving: bool) {
    let f = vec3(yaw.sin(), 0.0, yaw.cos());
    let hand = pos + Vec3::Y * 1.42 + vec3(-f.z, 0.0, f.x) * 0.2 + f * 0.5;
    draw_body(b, pos, yaw, walk, if moving { 1.0 } else { 0.0 }, 0.0, Arm::Hand(hand), Arm::Swing, 0.0);
    let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, f), hand);
    b.cube(&m, vec3(0.0, 0.04, 0.08), vec3(0.06, 0.08, 0.26), rgb(0.15, 0.15, 0.16));
}

/// Ícone da arma no HUD (silhueta em retângulos, caixa de `s` px).
pub fn draw_icon(weapon: usize, x: f32, y: f32, s: f32) {
    let col = Color::new(0.95, 0.95, 0.9, 1.0);
    let r = |a: f32, b: f32, w: f32, h: f32| draw_rectangle(x + a * s, y + b * s, w * s, h * s, col);
    match ARSENAL[weapon].kind {
        Kind::Melee if weapon == 0 => {
            r(0.3, 0.35, 0.4, 0.3);
            r(0.3, 0.3, 0.1, 0.1);
            r(0.42, 0.28, 0.1, 0.1);
            r(0.54, 0.3, 0.1, 0.1);
            r(0.25, 0.55, 0.3, 0.12);
        }
        Kind::Melee => {
            for k in 0..6 {
                r(0.18 + k as f32 * 0.11, 0.7 - k as f32 * 0.09, 0.14, 0.1 + k as f32 * 0.01);
            }
        }
        Kind::Grenade => {
            draw_circle(x + s * 0.5, y + s * 0.55, s * 0.2, col);
            r(0.45, 0.25, 0.1, 0.12);
        }
        Kind::Molotov => {
            r(0.42, 0.4, 0.16, 0.42);
            r(0.46, 0.22, 0.08, 0.2);
            draw_circle(x + s * 0.5, y + s * 0.17, s * 0.06, ORANGE);
        }
        Kind::Rocket => {
            r(0.1, 0.4, 0.8, 0.14);
            r(0.05, 0.37, 0.1, 0.2);
            r(0.4, 0.54, 0.06, 0.12);
        }
        Kind::Flame => {
            r(0.15, 0.42, 0.55, 0.08);
            r(0.3, 0.5, 0.22, 0.16);
            draw_circle(x + s * 0.8, y + s * 0.45, s * 0.08, ORANGE);
        }
        Kind::Gun => {
            let len = ARSENAL[weapon].len.min(1.0);
            let w = 0.25 + len * 0.55;
            r(0.5 - w * 0.5, 0.4, w, 0.11);
            r(0.5 - w * 0.5 + w * 0.25, 0.5, 0.07, 0.16);
            if len > 0.5 {
                r(0.5 - w * 0.5, 0.45, 0.14, 0.13);
            }
            if weapon == 3 || weapon == 5 || weapon == 6 {
                r(0.5, 0.5, 0.06, 0.18);
            }
            if weapon == 7 {
                r(0.4, 0.33, 0.25, 0.06);
            }
        }
    }
}

/// Sedã da primeira missão: arcade com aderência lateral, freio de mão (drift) e sobe degrau de 1 bloco.
/// Apanha (batida/explosão), solta fumaça e explode; depois de um tempo outro aparece na vaga.
pub struct Car {
    pub pos: Vec3,
    pub yaw: f32,
    pub vel: Vec3,
    vy: f32,
    steer: f32,
    wheel: f32,
    pub hp: f32,
    /// > 0 = carcaça queimada (segundos até aparecer outro).
    pub wreck: f32,
    /// Animação da porta do motorista (1 = abrindo agora).
    pub door: f32,
    smoke_t: f32,
}

impl Car {
    pub fn new(world: &World) -> Self {
        let (x, z) = (70.5, 99.5);
        Car { pos: vec3(x, world.floor_at(x, G as f32 + 3.0, z), z), yaw: -FRAC_PI_2, vel: Vec3::ZERO, vy: 0.0, steer: 0.0, wheel: 0.0, hp: 100.0, wreck: 0.0, door: 0.0, smoke_t: 0.0 }
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

    pub fn damage(&mut self, dmg: f32) {
        if self.wreck <= 0.0 {
            self.hp = (self.hp - dmg).max(0.0);
        }
    }

    /// Retorna true no frame em que o carro explode.
    pub fn tick(&mut self, world: &World, dt: f32) -> bool {
        self.door = (self.door - dt * 2.0).max(0.0);
        self.smoke_t -= dt;
        if self.wreck > 0.0 {
            self.wreck -= dt;
            self.vel *= (-dt * 3.0).exp();
            if self.wreck <= 0.0 {
                *self = Car::new(world);
            }
            return false;
        }
        if self.hp <= 0.0 {
            self.wreck = 20.0;
            self.vy = 9.0;
            return true;
        }
        false
    }

    /// Fumaça do capô conforme o dano: (posição, cor) quando é hora de soltar um puff.
    pub fn smoke(&mut self) -> Option<(Vec3, Color, bool)> {
        if self.smoke_t > 0.0 || (self.hp > 50.0 && self.wreck <= 0.0) {
            return None;
        }
        self.smoke_t = if self.wreck > 0.0 || self.hp < 20.0 { 0.04 } else { 0.12 };
        let fire = self.wreck > 0.0 || self.hp < 20.0;
        let g = if self.hp < 25.0 || self.wreck > 0.0 { 0.12 } else { 0.55 };
        let p = self.pos + self.fwd() * if self.wreck > 0.0 { gen_range(-1.5, 1.5) } else { 1.5 } + Vec3::Y * 1.0;
        Some((p, Color::new(g, g, g, 1.0), fire))
    }

    /// Retorna a velocidade da batida (0 se não bateu). `steer` > 0 = esquerda.
    pub fn update(&mut self, world: &World, dt: f32, throttle: f32, steer: f32, handbrake: bool) -> f32 {
        let (throttle, steer) = if self.wreck > 0.0 { (0.0, 0.0) } else { (throttle, steer) };
        let f = self.fwd();
        let l = vec3(f.z, 0.0, -f.x);
        let (mut vf, mut vs) = (self.vel.dot(f), self.vel.dot(l));
        let top = if self.hp < 30.0 { 16.0 } else { 28.0 };
        if throttle > 0.0 {
            if vf < top {
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
            if crash > 6.0 {
                self.damage((crash - 6.0) * 2.5);
            }
        } else {
            self.pos.x = np.x;
            self.pos.z = np.z;
        }
        let ground = world.floor_at(self.pos.x, self.pos.y + 1.2, self.pos.z);
        if ground >= self.pos.y - 0.05 && self.vy <= 0.0 {
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
        let door = (self.door * PI).sin() * 1.1;
        draw_car_ex(b, self.pos, self.yaw, self.steer, self.wheel, driver, door, self.hp, self.wreck > 0.0);
    }
}

pub fn draw_car(b: &mut Batch, pos: Vec3, yaw: f32, steer: f32, wheel: f32, driver: bool) {
    draw_car_ex(b, pos, yaw, steer, wheel, driver, 0.0, 100.0, false);
}

/// Sedã quatro portas de linhas retas (original): capô longo em cunha, coluna C inclinada,
/// grade cromada, faróis retangulares, frisos, retrovisores e rodas com calota.
#[allow(clippy::too_many_arguments)]
fn draw_car_ex(b: &mut Batch, pos: Vec3, yaw: f32, steer: f32, wheel: f32, driver: bool, door: f32, hp: f32, wreck: bool) {
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
    let dent = (1.0 - hp / 100.0).clamp(0.0, 1.0);
    let paint = if wreck { rgb(0.1, 0.09, 0.08) } else { rgb(0.42 - dent * 0.1, 0.5 - dent * 0.12, 0.47 - dent * 0.1) };
    let low = Color::new(paint.r * 0.75, paint.g * 0.75, paint.b * 0.75, 1.0);
    let glass = if wreck { rgb(0.05, 0.05, 0.05) } else { rgb(0.1, 0.15, 0.2) };
    let dark = rgb(0.06, 0.06, 0.07);
    let chrome = if wreck { rgb(0.2, 0.18, 0.16) } else { rgb(0.75, 0.76, 0.78) };
    // Chassi: saia, carroceria, capô e porta-malas
    b.cube(&m, vec3(0.0, 0.42, 0.0), vec3(1.86, 0.22, 4.5), low);
    b.cube(&m, vec3(0.0, 0.72, 0.0), vec3(1.9, 0.42, 4.4), paint);
    let hood = m * Mat4::from_translation(vec3(0.0, 0.95, 1.35)) * Mat4::from_rotation_x(0.05 + dent * 0.08);
    b.cube(&hood, Vec3::ZERO, vec3(1.8, 0.06, 1.7), paint);
    b.cube(&m, vec3(0.0, 0.97, -1.75), vec3(1.8, 0.08, 0.95), paint);
    // Cabine: teto + colunas + vidros
    b.cube(&m, vec3(0.0, 1.5, -0.35), vec3(1.6, 0.07, 1.75), paint);
    let ws = m * Mat4::from_translation(vec3(0.0, 1.24, 0.72)) * Mat4::from_rotation_x(-0.55);
    b.cube(&ws, Vec3::ZERO, vec3(1.55, 0.6, 0.04), glass);
    let rw = m * Mat4::from_translation(vec3(0.0, 1.24, -1.38)) * Mat4::from_rotation_x(0.6);
    b.cube(&rw, Vec3::ZERO, vec3(1.55, 0.6, 0.04), glass);
    for s in [-1.0f32, 1.0] {
        b.cube(&m, vec3(s * 0.79, 1.23, -0.35), vec3(0.04, 0.48, 1.65), glass);
        for z in [0.6f32, -0.35, -1.25] {
            b.cube(&m, vec3(s * 0.8, 1.23, z), vec3(0.06, 0.5, 0.1), paint);
        }
        // Friso, maçanetas, retrovisor
        b.cube(&m, vec3(s * 0.96, 0.78, 0.0), vec3(0.02, 0.05, 3.6), chrome);
        for z in [0.25f32, -0.85] {
            b.cube(&m, vec3(s * 0.96, 0.88, z), vec3(0.03, 0.04, 0.16), chrome);
        }
        b.cube(&m, vec3(s * 1.0, 1.08, 0.75), vec3(0.14, 0.1, 0.06), dark);
        // Faróis e lanternas
        if !wreck {
            b.glow(&m, vec3(s * 0.62, 0.78, 2.21), vec3(0.42, 0.16, 0.03), rgb(1.0, 0.96, 0.8));
            b.glow(&m, vec3(s * 0.66, 0.8, -2.21), vec3(0.38, 0.18, 0.03), rgb(0.85, 0.05, 0.05));
            b.glow(&m, vec3(s * 0.38, 0.8, -2.21), vec3(0.14, 0.18, 0.03), rgb(1.0, 0.55, 0.1));
        }
        for z in [1.42f32, -1.42] {
            let wm = m * Mat4::from_translation(vec3(s * 0.88, 0.36, z)) * Mat4::from_rotation_y(if z > 0.0 { steer * 0.5 } else { 0.0 }) * Mat4::from_rotation_x(wheel);
            b.cube(&wm, Vec3::ZERO, vec3(0.26, 0.72, 0.72), dark);
            b.cube(&wm, Vec3::ZERO, vec3(0.26, 0.5, 0.5), dark);
            b.cube(&wm, vec3(s * 0.12, 0.0, 0.0), vec3(0.04, 0.36, 0.36), chrome);
            b.cube(&wm, vec3(s * 0.14, 0.0, 0.0), vec3(0.03, 0.12, 0.42), rgb(0.4, 0.4, 0.42));
        }
    }
    // Porta do motorista (lado esquerdo do carro = +X local) abrindo
    if door > 0.01 {
        let dm = m * Mat4::from_translation(vec3(0.95, 0.0, 0.62)) * Mat4::from_rotation_y(-door);
        b.cube(&dm, vec3(0.0, 0.75, -0.55), vec3(0.08, 0.5, 1.1), paint);
        b.cube(&dm, vec3(0.0, 1.23, -0.55), vec3(0.04, 0.45, 1.0), glass);
    }
    // Para-choques, grade, placa
    b.cube(&m, vec3(0.0, 0.45, 2.25), vec3(1.94, 0.18, 0.12), chrome);
    b.cube(&m, vec3(0.0, 0.45, -2.25), vec3(1.94, 0.18, 0.12), chrome);
    b.cube(&m, vec3(0.0, 0.75, 2.215), vec3(0.7, 0.2, 0.03), dark);
    for k in 0..4 {
        b.cube(&m, vec3(-0.27 + k as f32 * 0.18, 0.75, 2.225), vec3(0.03, 0.18, 0.02), chrome);
    }
    b.cube(&m, vec3(0.0, 0.62, -2.235), vec3(0.5, 0.16, 0.02), rgb(0.88, 0.88, 0.85));
    b.cube(&m, vec3(0.0, 0.62, -2.24), vec3(0.4, 0.05, 0.02), rgb(0.1, 0.2, 0.6));
    if driver {
        let seat = m * Mat4::from_translation(vec3(0.4, 0.15, -0.1));
        let head = seat.transform_point3(vec3(0.0, 1.05, 0.0));
        b.cube(&Mat4::from_translation(head), Vec3::ZERO, vec3(0.22, 0.26, 0.24), SKIN);
        b.cube(&Mat4::from_translation(head + Vec3::Y * 0.125), Vec3::ZERO, vec3(0.235, 0.05, 0.25), HAIR);
        b.cube(&seat, vec3(0.0, 0.7, 0.0), vec3(0.4, 0.5, 0.25), JACKET);
        for s in [-1.0f32, 1.0] {
            limb(b, seat.transform_point3(vec3(s * 0.22, 0.88, 0.0)), seat.transform_point3(vec3(s * 0.18, 0.75, 0.5)), 0.12, JACKET);
        }
        b.cube(&seat, vec3(0.0, 0.78, 0.62), vec3(0.4, 0.4, 0.05), dark);
    }
}
