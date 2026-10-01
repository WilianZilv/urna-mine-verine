//! WOLVERINE jogável (paródia original blocada: uniforme amarelo e azul, máscara de abas, garras de
//! "adamantium da urna"). Garras saem/recolhem (snikt), combo leve de 4 golpes com direção, golpe
//! pesado, gancho que lança + combo aéreo, mergulho, tornado de garras, bote com trava (monta no alvo e
//! esfaqueia no tempo certo; na urna e no godzilha escala o corpo), fúria berserker, fator de cura (o
//! esqueleto de adamantium aparece com pouca vida, explosão derruba e ele levanta), defesa que segura
//! laser (no tempo certo devolve pro atirador), escalada com as garras na parede, corrida, pulo alto e
//! rolamento. Golpes viram mensagens "a" (o host aplica) e PvP vai no "pv" do "p".

use crate::audio::Clip;
use crate::batch::Batch;
use crate::gta::Target;
use crate::models::{U, rgb};
use crate::npc;
use crate::player::Player;
use crate::urna::{Fx, Particle};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

const HALF: f32 = 0.3;
const HEIGHT: f32 = 1.8;
const GRAV: f32 = 28.0;
const WALK: f32 = 5.2;
const SPRINT: f32 = 9.5;
const JUMP: f32 = 11.0;
/// Correndo: ~3,7 blocos de altura.
const HIGH_JUMP: f32 = 14.5;
const CLIMB_V: f32 = 4.5;
const BERSERK: f32 = 12.0;

const MOVE: u8 = 0;
const AIR: u8 = 1;
const SLASH: u8 = 2;
const HEAVY: u8 = 3;
const UPPER: u8 = 4;
const AIRSLASH: u8 = 5;
const DIVE: u8 = 6;
const TORNADO: u8 = 7;
const POUNCE: u8 = 8;
const MOUNT: u8 = 9;
const GUARD: u8 = 10;
const ROLL: u8 = 11;
const CLIMB: u8 = 12;
const KNOCK: u8 = 13;
const DOWN: u8 = 14;
const ROAR: u8 = 15;
const DIVE_LAND: u8 = 16;

const S_SNIKT: u8 = 0;
const S_SHEATH: u8 = 1;
const S_WHOOSH: u8 = 2;
const S_HEAVY: u8 = 5;
const S_HIT: u8 = 6;
const S_ROAR: u8 = 7;
const S_CLANG: u8 = 8;
const S_PARRY: u8 = 9;
const S_STAB: u8 = 10;
const S_SCRAPE: u8 = 11;
const S_LAND: u8 = 12;

fn fwd(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

fn yaw_to(d: Vec3) -> f32 {
    d.x.atan2(d.z)
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn approach(x: f32, to: f32, step: f32) -> f32 {
    if x < to { (x + step).min(to) } else { (x - step).max(to) }
}

fn find(targets: &[Target], l: (u8, usize)) -> Option<Target> {
    targets.iter().find(|t| t.3 == l.0 && t.2 == l.1).copied()
}

fn metal(g: u8) -> bool {
    matches!(g, npc::ROBOT | npc::URNA | npc::EU | npc::GUARD | npc::ZEPPELIN)
}

/// (duração, início e fim da janela de acerto) do golpe leve `n` da sequência (3 = finalizador).
fn slash_timing(n: u8) -> (f32, f32, f32) {
    if n >= 3 { (0.5, 0.14, 0.28) } else { (0.32, 0.07, 0.17) }
}

/// Melhor alvo no cone da câmera (trava / bote automático).
fn pick(targets: &[Target], eye: Vec3, cam: Vec3, max: f32) -> Option<(u8, usize)> {
    let mut best: Option<(f32, (u8, usize))> = None;
    for &(c, r, i, g) in targets {
        let d = c - eye;
        let dist = (d.length() - r * 0.5).max(0.1);
        let cos = d.normalize_or_zero().dot(cam);
        if dist > max || cos < 0.55 {
            continue;
        }
        let s = cos * 2.0 - dist / max;
        if best.is_none_or(|b| s > b.0) {
            best = Some((s, (g, i)));
        }
    }
    best.map(|b| b.1)
}

/// Entrada do frame (teclado, mouse e botões do celular já juntos). `stick`: x = direita, y = frente.
#[derive(Clone, Copy, Default)]
pub struct Intent {
    pub stick: Vec2,
    pub jump: bool,
    pub jump_hold: bool,
    pub light: bool,
    pub light_hold: bool,
    pub heavy: bool,
    pub guard: bool,
    pub pounce: bool,
    pub lock: bool,
    pub rage: bool,
    pub sprint: bool,
    pub roll: bool,
    pub claws: bool,
}

impl Intent {
    /// `tap`/`held`: botões 0-3 do celular (PULA, GARRA, BOTE, DEFESA/FURIA).
    pub fn read(active: bool, mouse: bool, stick: Vec2, tap: [bool; 4], held: [bool; 4], rage_full: bool) -> Intent {
        let key = |k| active && is_key_down(k);
        let press = |k| active && is_key_pressed(k);
        let kb = vec2((key(KeyCode::D) as i32 - key(KeyCode::A) as i32) as f32, (key(KeyCode::W) as i32 - key(KeyCode::S) as i32) as f32);
        let stick = if !active {
            Vec2::ZERO
        } else if stick.length() > 0.1 {
            stick
        } else {
            kb.clamp_length_max(1.0)
        };
        Intent {
            stick,
            jump: tap[0] || press(KeyCode::Space),
            jump_hold: held[0] || key(KeyCode::Space),
            light: tap[1] || (mouse && is_mouse_button_pressed(MouseButton::Left)),
            light_hold: held[1] || (mouse && is_mouse_button_down(MouseButton::Left)),
            heavy: press(KeyCode::F),
            guard: (mouse && is_mouse_button_down(MouseButton::Right)) || (held[3] && !rage_full),
            pounce: tap[2] || press(KeyCode::E),
            lock: press(KeyCode::Tab) || (mouse && is_mouse_button_pressed(MouseButton::Middle)),
            rage: (tap[3] && rage_full) || press(KeyCode::R),
            sprint: key(KeyCode::LeftShift) || key(KeyCode::RightShift) || stick.length() > 0.95,
            roll: press(KeyCode::LeftControl) || press(KeyCode::V),
            claws: press(KeyCode::X),
        }
    }
}

struct Mount {
    g: u8,
    i: usize,
    /// Posição na superfície do alvo: ângulo em volta do centro e altura relativa.
    ang: f32,
    h: f32,
    big: bool,
    cd: f32,
    /// 0..1 do anel do tempo certo (crítico perto do fim).
    ring: f32,
}

struct Strike {
    g: u8,
    i: usize,
    pvp: Option<u64>,
    dmg: f32,
    dir: Vec3,
    at: Vec3,
    w: &'static str,
}

/// Estado de animação (local ou de jogador remoto).
#[derive(Clone, Copy, Default)]
struct Anim {
    act: u8,
    aux: u8,
    t: f32,
    walk: f32,
    run: f32,
    vy: f32,
    ground: bool,
    claws: f32,
    berserk: bool,
    stage: u8,
    stab: f32,
    squat: f32,
}

pub struct Body {
    pub pos: Vec3,
    vel: Vec3,
    pub yaw: f32,
    act: u8,
    /// Golpe leve: bits 0-1 = posição na sequência, 2-3 = variante (1 dir, 2 esq, 3 estocada).
    aux: u8,
    t: f32,
    ground: bool,
    walk: f32,
    hp: f32,
    rage: f32,
    berserk: f32,
    claws: f32,
    claws_out: bool,
    /// Segundos sem dar nem tomar dano (cura rápida depois de 2,5 s).
    calm: f32,
    /// Segundos sem atacar (garras recolhem).
    idle: f32,
    queued: u8,
    hit: Vec<(u8, usize)>,
    hit_p: Vec<u64>,
    wall: Vec3,
    guard_t: f32,
    invuln: f32,
    /// Congelada de impacto.
    stop: f32,
    charge: f32,
    charged: bool,
    air_n: u8,
    combo_n: u32,
    combo_t: f32,
    tick: i32,
    mount: Option<Mount>,
    lock: Option<(u8, usize)>,
    ptarget: Option<(u8, usize)>,
    pspd: f32,
    roll_dir: Vec3,
    stab_t: f32,
    flash: f32,
    squat: f32,
    reflect: Option<Vec3>,
    snd: Vec<(u8, f32)>,
    strikes: Vec<Strike>,
    pops: Vec<(Vec3, String, Color)>,
}

impl Body {
    pub fn new(pos: Vec3, yaw: f32) -> Self {
        Body {
            pos,
            vel: Vec3::ZERO,
            yaw,
            act: AIR,
            aux: 0,
            t: 0.0,
            ground: false,
            walk: 0.0,
            hp: 100.0,
            rage: 0.0,
            berserk: 0.0,
            claws: 0.0,
            claws_out: false,
            calm: 9.0,
            idle: 0.0,
            queued: 0,
            hit: Vec::new(),
            hit_p: Vec::new(),
            wall: Vec3::ZERO,
            guard_t: 9.0,
            invuln: 0.0,
            stop: 0.0,
            charge: 0.0,
            charged: false,
            air_n: 0,
            combo_n: 0,
            combo_t: 9.0,
            tick: 0,
            mount: None,
            lock: None,
            ptarget: None,
            pspd: 0.0,
            roll_dir: Vec3::Z,
            stab_t: 9.0,
            flash: 0.0,
            squat: 0.0,
            reflect: None,
            snd: Vec::new(),
            strikes: Vec::new(),
            pops: Vec::new(),
        }
    }

    fn stage(&self) -> u8 {
        if self.hp < 33.0 {
            2
        } else if self.hp < 66.0 {
            1
        } else {
            0
        }
    }

    fn anim(&self) -> Anim {
        let h = vec2(self.vel.x, self.vel.z).length();
        Anim { act: self.act, aux: self.aux, t: self.t, walk: self.walk, run: h / SPRINT, vy: self.vel.y, ground: self.ground, claws: self.claws, berserk: self.berserk > 0.0, stage: self.stage(), stab: self.stab_t, squat: self.squat }
    }

    fn start(&mut self, act: u8, aux: u8) {
        self.act = act;
        self.aux = aux;
        self.t = 0.0;
        self.queued = 0;
        self.hit.clear();
        self.hit_p.clear();
        match act {
            SLASH | AIRSLASH => {
                self.snd.push((S_WHOOSH + (aux & 3) % 3, 0.9));
                self.pop_claws();
            }
            HEAVY | UPPER | DIVE | TORNADO | POUNCE => {
                self.snd.push((S_HEAVY, 1.0));
                self.pop_claws();
            }
            _ => {}
        }
    }

    fn pop_claws(&mut self) {
        self.idle = 0.0;
        if !self.claws_out {
            self.claws_out = true;
            self.snd.push((S_SNIKT, 1.2));
        }
    }

    fn window(&self) -> (f32, f32) {
        match self.act {
            SLASH => {
                let (_, a, b) = slash_timing(self.aux & 3);
                (a, b)
            }
            HEAVY => (0.26, 0.4),
            UPPER => (0.08, 0.24),
            AIRSLASH => (0.06, 0.17),
            _ => (0.0, 0.0),
        }
    }

    fn striking(&self) -> bool {
        let (a, b) = self.window();
        matches!(self.act, TORNADO | DIVE | POUNCE | ROAR) || (b > 0.0 && self.t >= a && self.t < b + 0.04)
    }

    /// Vira pro alvo travado, ou pro mais perto na frente da câmera, ou pra onde a câmera olha.
    fn face(&mut self, cf: Vec3, targets: &[Target]) {
        let near = || {
            targets
                .iter()
                .filter(|t| {
                    let d = vec3(t.0.x - self.pos.x, 0.0, t.0.z - self.pos.z);
                    d.length() - t.1 * 0.8 < 3.5 && d.normalize_or_zero().dot(cf) > 0.2 && (t.0.y - self.pos.y).abs() < t.1 + 2.0
                })
                .min_by(|a, b| a.0.distance_squared(self.pos).total_cmp(&b.0.distance_squared(self.pos)))
                .copied()
        };
        let t = self.lock.and_then(|l| find(targets, l)).filter(|t| t.0.distance(self.pos) < 8.0 + t.1).or_else(near);
        self.yaw = match t {
            Some(t) => yaw_to(t.0 - self.pos),
            None => yaw_to(cf),
        };
    }

    /// Acerta quem estiver no arco à frente (arc = cosseno mínimo; -1 = em volta toda).
    #[allow(clippy::too_many_arguments)]
    fn sweep(&mut self, reach: f32, arc: f32, dmg: f32, kh: f32, kv: f32, w: &'static str, targets: &[Target], others: &[(u64, Vec3)], fx: &mut Fx) -> bool {
        let up = Vec3::Y;
        let fw = fwd(self.yaw);
        let base = self.pos + up * 1.0;
        let dmg = dmg * if self.berserk > 0.0 { 1.6 } else { 1.0 };
        let mut hits: Vec<(Vec3, Vec3, bool)> = Vec::new();
        let foes = targets.iter().map(|&(c, r, i, g)| (c, r, Ok((g, i)))).chain(others.iter().map(|&(id, p)| (p + up * 0.9, 0.45, Err(id))));
        for (c, r, who) in foes {
            let done = match who {
                Ok(k) => self.hit.contains(&k),
                Err(id) => self.hit_p.contains(&id),
            };
            let d = c - base;
            let flat = vec3(d.x, 0.0, d.z);
            let gap = flat.length() - r * 0.8;
            if done || gap > reach || d.y.abs() > r + 1.4 || (arc > -1.0 && gap > 0.3 && flat.normalize_or_zero().dot(fw) < arc) {
                continue;
            }
            let dir = flat.normalize_or(fw);
            let at = base + dir * gap.max(0.3) + up * d.y.clamp(-0.5, 0.8);
            let push = dir * (kh / 7.0) + up * (kv / 7.0);
            match who {
                Ok((g, i)) => {
                    self.hit.push((g, i));
                    self.strikes.push(Strike { g, i, pvp: None, dmg, dir: push, at, w });
                    hits.push((at, dir, metal(g)));
                }
                Err(id) => {
                    self.hit_p.push(id);
                    self.strikes.push(Strike { g: 0, i: 0, pvp: Some(id), dmg, dir, at, w });
                    hits.push((at, dir, false));
                }
            }
        }
        if hits.is_empty() {
            return false;
        }
        for &(at, dir, steel) in &hits {
            Self::gore(fx, at, dir, steel, 8);
        }
        self.combo_n += hits.len() as u32;
        self.combo_t = 0.0;
        self.calm = 0.0;
        if self.berserk <= 0.0 {
            self.rage = (self.rage + dmg * 0.6 * hits.len() as f32).min(100.0);
        }
        self.stop = if dmg >= 14.0 { 0.09 } else { 0.05 };
        self.snd.push((S_HIT, 1.0));
        if dmg >= 14.0 {
            self.pops.push((hits[0].0 + up * 0.8, w.to_string(), Color::new(1.0, 0.9, 0.4, 1.0)));
        }
        true
    }

    fn gore(fx: &mut Fx, at: Vec3, dir: Vec3, steel: bool, n: usize) {
        for _ in 0..n {
            let col = if steel { Color::new(1.0, 0.85, 0.4, 1.0) } else { Color::new(gen_range(0.45, 0.75), 0.02, 0.02, 1.0) };
            fx.particles.push(Particle { pos: at, vel: dir * gen_range(1.0, 4.5) + vec3(gen_range(-1.5, 1.5), gen_range(0.5, 3.0), gen_range(-1.5, 1.5)), col, life: gen_range(0.3, 0.7), size: gen_range(0.05, 0.12), gravity: !steel });
        }
    }

    /// Dano em mim. `guard`: direção pro agressor quando dá pra defender (ZERO = qualquer lado).
    fn hurt(&mut self, dmg: f32, knock: Vec3, guard: Option<Vec3>) {
        if self.invuln > 0.0 || matches!(self.act, DOWN | ROAR) || dmg <= 0.0 {
            return;
        }
        let (mut dmg, mut knock) = (dmg, knock);
        if guard.is_some_and(|to| self.act == GUARD && (to == Vec3::ZERO || fwd(self.yaw).dot(to) > 0.0)) {
            if self.guard_t < 0.25 {
                self.snd.push((S_PARRY, 1.2));
                self.pops.push((self.pos + Vec3::Y * 2.2, "APARADO!".into(), SKYBLUE));
                self.rage = (self.rage + 10.0).min(100.0);
                return;
            }
            dmg *= 0.15;
            knock *= 0.2;
            self.snd.push((S_CLANG, 1.0));
        }
        if self.berserk > 0.0 {
            dmg *= 0.6;
        }
        self.hp -= dmg;
        self.calm = 0.0;
        self.flash = 1.0;
        if self.berserk <= 0.0 {
            self.rage = (self.rage + dmg * 0.8).min(100.0);
        }
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.start(DOWN, 0);
            self.mount = None;
            self.vel = knock * 0.5 + Vec3::Y * 3.0;
            self.ground = false;
            self.pops.push((self.pos + Vec3::Y * 2.2, "O ADAMANTIUM SEGUROU...".into(), Color::new(0.8, 0.85, 0.95, 1.0)));
        } else if knock.length() > 6.0 && self.act != MOUNT {
            self.start(KNOCK, 0);
            self.vel = knock;
            self.ground = false;
        } else {
            self.vel += knock * 0.4;
        }
    }

    fn roar(&mut self) {
        self.start(ROAR, 0);
        self.berserk = BERSERK;
        self.rage = 100.0;
        self.invuln = 0.8;
        self.mount = None;
        self.pop_claws();
        self.snd.push((S_ROAR, 1.6));
        self.pops.push((self.pos + Vec3::Y * 2.4, "BERSERKER!!!".into(), RED));
    }

    fn pounce(&mut self, cf: Vec3, cam: Vec3, targets: &[Target]) {
        let up = Vec3::Y;
        let tg = self.lock.and_then(|l| find(targets, l)).or_else(|| pick(targets, self.pos + up * 1.6, cam, 22.0).and_then(|l| find(targets, l)));
        self.start(POUNCE, 0);
        self.mount = None;
        match tg {
            Some(t) => {
                self.ptarget = Some((t.3, t.2));
                let to = t.0 - (self.pos + up * 0.9);
                let flat = vec3(to.x, 0.0, to.z);
                let h = flat.length();
                let tt = (h / 18.0).clamp(0.3, 0.9);
                self.pspd = h / tt;
                self.vel = flat.normalize_or(cf) * self.pspd;
                self.vel.y = (to.y / tt + 0.5 * GRAV * tt).clamp(4.0, 24.0);
                self.yaw = yaw_to(flat.normalize_or(cf));
            }
            None => {
                self.ptarget = None;
                self.vel = cf * 13.0 + up * 8.0;
                self.yaw = yaw_to(cf);
            }
        }
        self.ground = false;
    }

    fn mount_on(&mut self, t: Target) {
        let rel = self.pos + Vec3::Y * 0.9 - t.0;
        let big = t.1 >= 1.5;
        self.mount = Some(Mount { g: t.3, i: t.2, ang: rel.x.atan2(rel.z), h: if big { rel.y.clamp(-t.1 * 0.5, t.1 * 0.8) } else { 0.35 }, big, cd: 0.15, ring: 0.0 });
        self.start(MOUNT, 0);
        self.vel = Vec3::ZERO;
        self.snd.push((S_STAB, 1.2));
        self.stop = 0.08;
        let dmg = 10.0 * if self.berserk > 0.0 { 1.6 } else { 1.0 };
        self.strikes.push(Strike { g: t.3, i: t.2, pvp: None, dmg, dir: -rel.normalize_or_zero() * 0.4, at: t.0 + rel.normalize_or_zero() * t.1 * 0.8, w: "BOTE" });
        self.pops.push((self.pos + Vec3::Y * 2.0, if big { "ESCALANDO!".into() } else { "MONTOU!".into() }, Color::new(1.0, 0.85, 0.3, 1.0)));
        self.calm = 0.0;
        self.combo_n += 1;
        self.combo_t = 0.0;
    }

    fn locomote(&mut self, dt: f32, mv: Vec3, mag: f32, top: f32, acc: f32, turn: bool) {
        let h = vec3(self.vel.x, 0.0, self.vel.z);
        let nh = h + (mv * top - h).clamp_length_max(acc * dt);
        self.vel.x = nh.x;
        self.vel.z = nh.z;
        if turn && mag > 0.1 {
            self.yaw += wrap(yaw_to(mv) - self.yaw).clamp(-14.0 * dt, 14.0 * dt);
        }
    }

    fn lunge(&mut self, dt: f32, speed: f32) {
        if speed > 0.0 {
            let f = fwd(self.yaw) * speed;
            self.vel.x = f.x;
            self.vel.z = f.z;
        } else {
            let k = (-12.0 * dt).exp();
            self.vel.x *= k;
            self.vel.z *= k;
        }
    }

    fn side(inp: &Intent, n: u8) -> u8 {
        if inp.stick.x > 0.5 {
            1
        } else if inp.stick.x < -0.5 {
            2
        } else if inp.stick.y > 0.5 && n == 2 {
            3
        } else {
            0
        }
    }

    fn ground_attack(&mut self, inp: &Intent, heavy: bool, sprint: bool, cf: Vec3, targets: &[Target]) -> bool {
        if heavy {
            if sprint {
                self.start(TORNADO, 0);
                self.tick = 0;
            } else if inp.stick.y < -0.5 {
                self.start(UPPER, 0);
            } else {
                self.start(HEAVY, 0);
            }
        } else if inp.light {
            self.start(SLASH, Self::side(inp, 0) << 2);
        } else {
            return false;
        }
        self.face(cf, targets);
        true
    }

    fn collides(world: &World, p: Vec3) -> bool {
        let (x0, x1) = ((p.x - HALF).floor() as i32, (p.x + HALF).floor() as i32);
        let (y0, y1) = (p.y.floor() as i32, (p.y + HEIGHT).floor() as i32);
        let (z0, z1) = ((p.z - HALF).floor() as i32, (p.z + HALF).floor() as i32);
        (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| world.solid(x, y, z))))
    }

    /// Eixos separados com subpassos e degrau de 1 bloco. Retorna a normal da parede que bateu.
    fn physics(&mut self, world: &World, dt: f32) -> Option<Vec3> {
        let v = self.vel;
        let n = (v.length() * dt / 0.3).ceil().clamp(1.0, 12.0) as i32;
        let step = v * dt / n as f32;
        let was = self.ground;
        let mut wall = None;
        let mut vstep = step.y;
        self.ground = false;
        for _ in 0..n {
            for axis in [0usize, 2] {
                if step[axis] == 0.0 {
                    continue;
                }
                let lim = if axis == 0 { WX } else { WZ } as f32 - 1.0;
                let mut p = self.pos;
                p[axis] = (p[axis] + step[axis]).clamp(1.0, lim);
                if !Self::collides(world, p) {
                    self.pos = p;
                    continue;
                }
                let up = p + Vec3::Y * 1.01;
                if was && self.act != CLIMB && !Self::collides(world, up) {
                    self.pos = up;
                    continue;
                }
                let mut nrm = Vec3::ZERO;
                nrm[axis] = -step[axis].signum();
                wall = Some(nrm);
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
                    self.vel.y = 0.0;
                    vstep = 0.0;
                }
            }
        }
        if !self.ground && self.vel.y <= 0.0 && Self::collides(world, self.pos - Vec3::Y * 0.06) {
            self.ground = true;
        }
        if let Some(nrm) = wall {
            if nrm.x != 0.0 {
                self.vel.x = 0.0;
            } else {
                self.vel.z = 0.0;
            }
        }
        wall
    }

    #[allow(clippy::too_many_arguments)]
    fn step(&mut self, world: &World, dt: f32, inp: &Intent, cam: Vec3, targets: &[Target], others: &[(u64, Vec3)], fx: &mut Fx) {
        let up = Vec3::Y;
        let bz = self.berserk > 0.0;
        let mul = if bz { 1.3 } else { 1.0 };
        self.berserk = (self.berserk - dt).max(0.0);
        if bz {
            self.rage = self.berserk / BERSERK * 100.0;
        }
        self.calm += dt;
        self.idle += dt;
        self.invuln -= dt;
        self.combo_t += dt;
        self.guard_t += dt;
        self.stab_t += dt;
        self.flash = (self.flash - dt * 4.0).max(0.0);
        self.squat = (self.squat - dt * 3.0).max(0.0);
        if self.combo_t > 2.5 {
            self.combo_n = 0;
        }
        let adt = if self.stop > 0.0 {
            self.stop -= dt;
            dt * 0.1
        } else {
            dt * mul
        };
        self.t += adt;

        // Fator de cura: rápido fora de combate, sempre um pouco
        if self.act != DOWN && self.hp < 100.0 {
            let rate = if bz { 9.0 } else if self.calm > 2.5 { 16.0 } else { 2.5 };
            self.hp = (self.hp + rate * dt).min(100.0);
            if self.calm > 2.5 && gen_range(0.0, 1.0) < dt * 14.0 {
                let p = self.pos + vec3(gen_range(-0.3, 0.3), gen_range(0.3, 1.7), gen_range(-0.3, 0.3));
                fx.particles.push(Particle { pos: p, vel: up * gen_range(0.5, 1.2), col: Color::new(1.0, 0.6, 0.65, 1.0), life: 0.5, size: 0.05, gravity: false });
            }
        }
        if inp.claws {
            if self.claws_out {
                self.claws_out = false;
                self.snd.push((S_SHEATH, 0.8));
            } else {
                self.pop_claws();
            }
        }
        if self.claws_out && self.idle > 8.0 && matches!(self.act, MOVE | AIR) {
            self.claws_out = false;
            self.snd.push((S_SHEATH, 0.8));
        }
        self.claws = approach(self.claws, if self.claws_out { 1.0 } else { 0.0 }, dt * 14.0);
        // Segurar o leve carrega o pesado (celular não tem F)
        let mut heavy = inp.heavy;
        if inp.light_hold && self.act != MOUNT {
            self.charge += dt;
            if self.charge > 0.45 && !self.charged {
                self.charged = true;
                heavy = true;
            }
        } else {
            self.charge = 0.0;
            self.charged = false;
        }

        let cf = vec3(cam.x, 0.0, cam.z).normalize_or(fwd(self.yaw));
        let cr = vec3(-cf.z, 0.0, cf.x);
        let mv = (cf * inp.stick.y + cr * inp.stick.x).clamp_length_max(1.0);
        let mag = mv.length();
        let sprint = inp.sprint && mag > 0.5;
        let lockp = self.lock.and_then(|l| find(targets, l));
        if self.lock.is_some() && lockp.is_none_or(|t| t.0.distance(self.pos) > 32.0 + t.1) {
            self.lock = None;
        }

        let free = matches!(self.act, MOVE | AIR | GUARD | CLIMB) || (matches!(self.act, SLASH | HEAVY | AIRSLASH) && self.t > self.window().1);
        if inp.rage && self.rage >= 100.0 && !bz && !matches!(self.act, DOWN | MOUNT) {
            self.roar();
        } else if free && inp.pounce {
            self.pounce(cf, cam, targets);
        } else if free && self.ground && (inp.roll || (self.act == GUARD && inp.jump)) {
            self.start(ROLL, 0);
            self.roll_dir = if mag > 0.1 { mv.normalize() } else { -fwd(self.yaw) };
            if mag > 0.1 {
                self.yaw = yaw_to(mv);
            }
            self.invuln = 0.35;
        }

        let vy0 = self.vel.y;
        match self.act {
            MOVE => {
                if inp.guard && self.ground {
                    self.start(GUARD, 0);
                    self.guard_t = 0.0;
                    self.pop_claws();
                } else if !self.ground_attack(inp, heavy, sprint, cf, targets) {
                    if inp.jump && self.ground {
                        self.vel.y = if sprint { HIGH_JUMP } else { JUMP } + if bz { 2.0 } else { 0.0 };
                        self.ground = false;
                        self.act = AIR;
                        self.t = 0.0;
                    }
                    let strafe = lockp.filter(|_| !sprint);
                    self.locomote(dt, mv, mag, if sprint { SPRINT } else { WALK } * mul, 50.0, strafe.is_none());
                    if let Some(t) = strafe {
                        self.yaw += wrap(yaw_to(t.0 - self.pos) - self.yaw).clamp(-10.0 * dt, 10.0 * dt);
                    }
                }
            }
            AIR => {
                if inp.light && self.air_n < 3 {
                    let n = self.air_n;
                    self.start(AIRSLASH, n);
                    self.air_n += 1;
                    self.vel.y = self.vel.y.max(3.5);
                    self.face(cf, targets);
                } else if heavy {
                    self.start(DIVE, 0);
                    self.face(cf, targets);
                    let f = fwd(self.yaw) * 6.0;
                    self.vel = vec3(f.x, 4.0, f.z);
                } else {
                    self.locomote(dt, mv, mag, if sprint { SPRINT } else { WALK } * mul, 14.0, true);
                }
            }
            SLASH => {
                let n = self.aux & 3;
                let (dur, w0, w1) = slash_timing(n);
                let far = lockp.is_some_and(|t| vec2(t.0.x - self.pos.x, t.0.z - self.pos.z).length() - t.1 * 0.8 > 1.6);
                self.lunge(dt, if self.t < 0.1 { if far { 6.0 } else { 2.5 } } else { 0.0 });
                if self.t >= w0 && self.t < w1 {
                    let (reach, arc, dmg, kh, kv, w) = match (n, self.aux >> 2) {
                        (3, _) => (1.9, 0.0, 14.0, 8.0, 4.0, "TRITURADOR"),
                        (_, 3) => (2.3, 0.75, 10.0, 5.0, 1.0, "ESTOCADA"),
                        (2, _) => (1.7, 0.2, 9.0, 4.0, 2.0, "X DE GARRA"),
                        _ => (1.6, 0.1, 7.0, 3.0, 1.5, "GARRADA"),
                    };
                    self.sweep(reach, arc, dmg, kh, kv, w, targets, others, fx);
                }
                if self.t > 0.05 {
                    if inp.light {
                        self.queued = 1;
                    } else if heavy {
                        self.queued = 2;
                    }
                }
                if self.t >= w1 + 0.02 && self.queued != 0 {
                    if self.queued == 2 {
                        self.ground_attack(&Intent { light: false, ..*inp }, true, sprint, cf, targets);
                    } else {
                        let next = if n < 3 { n + 1 } else { 0 };
                        self.start(SLASH, next | Self::side(inp, next) << 2);
                        self.face(cf, targets);
                    }
                } else if self.t >= dur {
                    self.act = MOVE;
                    self.t = 0.0;
                }
            }
            HEAVY => {
                self.lunge(dt, if self.t >= 0.2 && self.t < 0.34 { 8.0 } else { 0.0 });
                if self.t >= 0.26 && self.t < 0.4 {
                    self.sweep(2.3, -0.1, 22.0, 10.0, 4.0, "GARRA PESADA", targets, others, fx);
                }
                if self.t >= 0.62 {
                    self.act = MOVE;
                    self.t = 0.0;
                }
            }
            UPPER => {
                self.lunge(dt, 0.0);
                if self.t >= 0.08 && self.t < 0.24 {
                    self.sweep(1.8, 0.2, 12.0, 0.5, 11.0, "GANCHO", targets, others, fx);
                }
                if self.t >= 0.12 && self.aux == 0 {
                    self.aux = 1;
                    self.vel.y = 13.0;
                    self.ground = false;
                    self.air_n = 0;
                }
                if self.t >= 0.45 {
                    self.act = AIR;
                    self.t = 0.0;
                }
            }
            AIRSLASH => {
                let n = self.aux & 3;
                let f = fwd(self.yaw) * 2.0;
                self.vel.x = f.x;
                self.vel.z = f.z;
                if self.t >= 0.06 && self.t < 0.17 {
                    let (dmg, kh, kv, w) = if n == 2 { (12.0, 6.0, -4.0, "MORTAL DE GARRA") } else { (8.0, 2.0, 5.0, "GARRA AEREA") };
                    if self.sweep(1.9, 0.0, dmg, kh, kv, w, targets, others, fx) {
                        self.vel.y = self.vel.y.max(2.5);
                    }
                }
                if self.t > 0.05 && inp.light {
                    self.queued = 1;
                }
                if self.t >= 0.19 && self.queued == 1 && self.air_n < 3 {
                    let n = self.air_n;
                    self.start(AIRSLASH, n);
                    self.air_n += 1;
                    self.vel.y = self.vel.y.max(3.5);
                    self.face(cf, targets);
                } else if self.t >= 0.12 && heavy {
                    self.start(DIVE, 0);
                    self.vel.y = 4.0;
                } else if self.t >= 0.3 {
                    self.act = AIR;
                    self.t = 0.0;
                }
            }
            DIVE => {
                if self.t >= 0.1 {
                    self.vel.y = -30.0;
                }
                self.sweep(1.2, -1.0, 10.0, 3.0, -2.0, "MERGULHO", targets, others, fx);
            }
            DIVE_LAND => {
                self.lunge(dt, 0.0);
                if self.aux == 0 {
                    self.aux = 1;
                    self.hit.clear();
                    self.sweep(3.2, -1.0, 20.0, 9.0, 7.0, "IMPACTO", targets, others, fx);
                    fx.shake = fx.shake.max(0.35);
                    self.snd.push((S_LAND, 1.4));
                    for i in 0..24 {
                        let a = i as f32 / 24.0 * TAU;
                        let d = vec3(a.cos(), 0.0, a.sin());
                        fx.particles.push(Particle { pos: self.pos + d * 0.4 + up * 0.1, vel: d * 8.0 + up * 0.8, col: Color::new(0.75, 0.7, 0.62, 1.0), life: 0.35, size: 0.2, gravity: false });
                    }
                }
                if self.t >= 0.4 {
                    self.act = MOVE;
                    self.t = 0.0;
                }
            }
            TORNADO => {
                let dur = if bz { 1.8 } else { 1.2 };
                if mag > 0.1 {
                    self.yaw += wrap(yaw_to(mv) - self.yaw).clamp(-4.0 * dt, 4.0 * dt);
                }
                let f = fwd(self.yaw) * 7.5 * mul;
                self.vel.x = f.x;
                self.vel.z = f.z;
                let tick = (self.t / 0.15) as i32;
                if tick != self.tick {
                    self.tick = tick;
                    self.hit.clear();
                    self.hit_p.clear();
                    if tick % 2 == 0 {
                        self.snd.push((S_WHOOSH + (tick / 2 % 3) as u8, 0.8));
                    }
                }
                self.sweep(2.2, -1.0, 5.0, 4.0, 2.5, "TORNADO DE GARRAS", targets, others, fx);
                if self.t >= dur {
                    self.act = MOVE;
                    self.t = 0.0;
                }
            }
            GUARD => {
                if !inp.guard {
                    self.act = MOVE;
                    self.t = 0.0;
                } else if inp.light {
                    self.start(SLASH, 0);
                    self.face(cf, targets);
                } else {
                    self.locomote(dt, mv, mag, 2.0, 30.0, false);
                    let look = lockp.map(|t| yaw_to(t.0 - self.pos)).unwrap_or(yaw_to(cf));
                    self.yaw += wrap(look - self.yaw).clamp(-10.0 * dt, 10.0 * dt);
                    self.idle = 0.0;
                }
            }
            ROLL => {
                let v = self.roll_dir * 11.0 * (1.0 - self.t / 0.45 * 0.5);
                self.vel.x = v.x;
                self.vel.z = v.z;
                if self.t >= 0.45 {
                    self.act = MOVE;
                    self.t = 0.0;
                }
            }
            CLIMB => {
                let n = self.wall;
                self.vel.x = -n.x * 1.5;
                self.vel.z = -n.z * 1.5;
                self.yaw = yaw_to(-n);
                let low = Self::collides(world, self.pos - n * 0.15);
                let high = Self::collides(world, self.pos - n * 0.15 + up * 0.9);
                if !low || inp.light {
                    self.act = AIR;
                    self.t = 0.0;
                } else if !high {
                    self.vel = -n * 4.5 + up * 7.0;
                    self.act = AIR;
                    self.t = 0.0;
                } else if inp.jump && inp.stick.y < -0.5 {
                    self.vel = n * 7.0 + up * 10.0;
                    self.yaw = yaw_to(n);
                    self.act = AIR;
                    self.t = 0.0;
                } else {
                    if inp.jump_hold {
                        self.vel.y = CLIMB_V * mul;
                        self.walk += dt * 6.0;
                    } else {
                        self.vel.y = (self.vel.y - GRAV * dt).max(-3.0);
                    }
                    let tick = (self.t / 0.3) as i32;
                    if tick != self.tick {
                        self.tick = tick;
                        self.snd.push((S_SCRAPE, 0.7));
                    }
                    if gen_range(0.0, 1.0) < dt * 20.0 {
                        let side = vec3(-n.z, 0.0, n.x) * gen_range(-0.3, 0.3);
                        let col = if inp.jump_hold { Color::new(0.5, 0.48, 0.45, 1.0) } else { Color::new(1.0, 0.85, 0.4, 1.0) };
                        fx.particles.push(Particle { pos: self.pos - n * 0.32 + side + up * gen_range(1.0, 1.6), vel: n * 1.5 + vec3(0.0, gen_range(-1.0, 1.0), 0.0), col, life: 0.4, size: 0.06, gravity: inp.jump_hold });
                    }
                    self.idle = 0.0;
                }
            }
            POUNCE => {
                match self.ptarget.and_then(|l| find(targets, l)) {
                    Some(t) => {
                        let to = t.0 - (self.pos + up * 0.9);
                        let flat = vec3(to.x, 0.0, to.z);
                        let hv = flat.normalize_or_zero() * self.pspd;
                        self.vel.x = hv.x;
                        self.vel.z = hv.z;
                        if flat.length() > 0.2 {
                            self.yaw = yaw_to(flat);
                        }
                        if to.length() < t.1 * 0.85 + 1.0 {
                            self.mount_on(t);
                        }
                    }
                    None => {
                        self.sweep(1.2, 0.3, 8.0, 5.0, 3.0, "BOTE", targets, others, fx);
                    }
                }
                if self.act == POUNCE && self.t > 1.5 {
                    self.act = AIR;
                    self.t = 0.0;
                }
            }
            MOUNT => self.ride(dt, inp, heavy, targets, fx),
            KNOCK => {
                if self.ground {
                    self.lunge(dt, 0.0);
                    if self.t > 0.9 {
                        self.act = MOVE;
                        self.t = 0.0;
                        self.squat = 0.6;
                    }
                }
            }
            DOWN => {
                if self.ground {
                    self.lunge(dt, 0.0);
                }
                if gen_range(0.0, 1.0) < dt * 30.0 {
                    let p = self.pos + vec3(gen_range(-0.9, 0.9), gen_range(0.1, 0.4), gen_range(-0.9, 0.9));
                    fx.particles.push(Particle { pos: p, vel: up * gen_range(0.6, 1.4), col: Color::new(1.0, 0.55, 0.6, 1.0), life: 0.6, size: 0.06, gravity: false });
                }
                if self.t > 3.5 {
                    self.hp = 40.0;
                    self.act = MOVE;
                    self.t = 0.0;
                    self.invuln = 1.5;
                    self.squat = 1.0;
                    self.pops.push((self.pos + Vec3::Y * 2.2, "AINDA NAO, BUB.".into(), Color::new(1.0, 0.85, 0.3, 1.0)));
                }
            }
            ROAR => {
                self.lunge(dt, 0.0);
                if self.aux == 0 && self.t > 0.15 {
                    self.aux = 1;
                    self.sweep(3.5, -1.0, 8.0, 9.0, 5.0, "RUGIDO", targets, others, fx);
                    fx.shake = fx.shake.max(0.5);
                    for _ in 0..30 {
                        let d = vec3(gen_range(-1.0, 1.0), gen_range(0.0, 1.0), gen_range(-1.0, 1.0)).normalize_or_zero();
                        fx.particles.push(Particle { pos: self.pos + up * 1.2, vel: d * gen_range(4.0, 9.0), col: Color::new(1.0, gen_range(0.05, 0.3), 0.05, 1.0), life: gen_range(0.3, 0.6), size: 0.12, gravity: false });
                    }
                }
                if self.t >= 0.75 {
                    self.act = if self.ground { MOVE } else { AIR };
                    self.t = 0.0;
                }
            }
            _ => {}
        }

        let g = match self.act {
            CLIMB | MOUNT => 0.0,
            AIRSLASH => GRAV * 0.2,
            UPPER => GRAV * 0.8,
            DIVE if self.t >= 0.1 => 0.0,
            _ => GRAV,
        };
        self.vel.y = (self.vel.y - g * dt).max(-55.0);
        if self.act == MOUNT {
            return;
        }
        let wall = self.physics(world, dt);
        self.walk += vec2(self.vel.x, self.vel.z).length() * dt * 1.6;
        if self.pos.y < -8.0 {
            self.pos = Player::spawn();
            self.vel = Vec3::ZERO;
        }

        if self.ground {
            match self.act {
                AIR | AIRSLASH => {
                    self.act = MOVE;
                    self.t = 0.0;
                    if vy0 < -16.0 {
                        self.squat = 1.0;
                        self.snd.push((S_LAND, 0.8));
                    }
                }
                UPPER if self.aux == 1 && self.t > 0.2 => {
                    self.act = MOVE;
                    self.t = 0.0;
                }
                POUNCE if self.t > 0.12 => {
                    self.act = MOVE;
                    self.t = 0.0;
                }
                DIVE if self.t >= 0.1 => self.start(DIVE_LAND, 0),
                CLIMB if !inp.jump_hold => {
                    self.act = MOVE;
                    self.t = 0.0;
                }
                _ => {}
            }
            if self.act == MOVE {
                self.air_n = 0;
            }
        } else if self.act == MOVE {
            self.act = AIR;
            self.t = 0.0;
        }
        if let Some(n) = wall {
            let into = mv.dot(-n) > 0.3 && inp.jump_hold && matches!(self.act, MOVE | AIR);
            if into || self.act == POUNCE {
                self.start(CLIMB, 0);
                self.wall = n;
                self.yaw = yaw_to(-n);
                self.pop_claws();
                self.snd.push((S_SCRAPE, 0.9));
            }
        }
    }

    /// Montado: segue o alvo, esfaqueia no clique (no anel = crítico), escala os gigantes.
    fn ride(&mut self, dt: f32, inp: &Intent, heavy: bool, targets: &[Target], fx: &mut Fx) {
        let up = Vec3::Y;
        let Some(mut m) = self.mount.take() else {
            self.act = AIR;
            return;
        };
        let Some((c, r, _, _)) = find(targets, (m.g, m.i)) else {
            self.vel = up * 7.0 - fwd(self.yaw) * 3.0;
            self.act = AIR;
            self.t = 0.0;
            return;
        };
        if m.big {
            m.ang -= inp.stick.x * 2.2 / r * dt;
            m.h = (m.h + inp.stick.y * 2.5 * dt).clamp(-r * 0.6, r * 0.95);
        }
        let rr = if m.big { r * 0.82 + 0.4 } else { r * 0.6 + 0.25 };
        let off = vec3(m.ang.sin() * rr, m.h, m.ang.cos() * rr);
        self.pos = c + off - up * 0.9;
        self.vel = Vec3::ZERO;
        self.yaw = yaw_to(-off);
        self.ground = false;
        self.walk += inp.stick.length() * dt * 5.0;
        m.ring = (m.ring + dt / 0.7).fract();
        m.cd -= dt;
        let mul = if self.berserk > 0.0 { 1.6 } else { 1.0 };
        let inward = -vec3(off.x, 0.0, off.z).normalize_or_zero();
        let at = c + off * 0.75;
        if inp.light && m.cd <= 0.0 {
            let crit = (0.75..0.95).contains(&m.ring);
            let dmg = if crit { 18.0 } else { 6.0 } * mul;
            self.strikes.push(Strike { g: m.g, i: m.i, pvp: None, dmg, dir: inward * 0.3, at, w: if crit { "ESTOCADA CRITICA" } else { "ESTOCADA" } });
            m.cd = 0.11;
            self.stab_t = 0.0;
            self.snd.push((if crit { S_HIT } else { S_STAB }, 1.0));
            Self::gore(fx, at, -inward, metal(m.g), if crit { 14 } else { 5 });
            if crit {
                self.stop = 0.07;
                self.pops.push((at + up * 0.6, "NO TEMPO!".into(), Color::new(0.4, 1.0, 0.5, 1.0)));
            }
            self.after_hit(dmg);
        } else if heavy && m.cd <= 0.0 {
            let dmg = 28.0 * mul;
            self.strikes.push(Strike { g: m.g, i: m.i, pvp: None, dmg, dir: inward * 0.5, at, w: "GARRA ATE O CABO" });
            m.cd = 0.9;
            self.stab_t = 0.0;
            self.stop = 0.12;
            self.snd.push((S_HIT, 1.3));
            Self::gore(fx, at, -inward, metal(m.g), 20);
            self.pops.push((at + up * 0.6, "ATE O CABO!".into(), Color::new(1.0, 0.85, 0.3, 1.0)));
            self.after_hit(dmg);
        }
        if inp.jump || (!m.big && self.t > 2.4) {
            if !m.big {
                self.strikes.push(Strike { g: m.g, i: m.i, pvp: None, dmg: 12.0 * mul, dir: inward * 1.5 + up * 0.8, at, w: "CHUTE DE SAIDA" });
            }
            self.vel = -inward * 7.0 + up * 8.0;
            self.act = AIR;
            self.t = 0.0;
            self.air_n = 0;
            return;
        }
        self.mount = Some(m);
    }

    fn after_hit(&mut self, dmg: f32) {
        self.calm = 0.0;
        self.combo_n += 1;
        self.combo_t = 0.0;
        if self.berserk <= 0.0 {
            self.rage = (self.rage + dmg * 0.6).min(100.0);
        }
    }
}

// ---------------------------------------------------------------- Sons sintetizados (graves, sem amostras)

struct Rng(u32);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn render(sr: u32, secs: f32, mut f: impl FnMut(f32, &mut Rng) -> f32) -> Vec<f32> {
    let mut rng = Rng(0x0BAD_C1A0);
    (0..(secs * sr as f32) as usize).map(|i| f(i as f32 / sr as f32, &mut rng)).collect()
}

fn norm(mut v: Vec<f32>, peak: f32) -> Vec<f32> {
    let m = v.iter().fold(0.0f32, |m, x| m.max(x.abs())).max(1e-6);
    v.iter_mut().for_each(|x| *x *= peak / m);
    v
}

/// Três lâminas saindo em sequência: estalo metálico + raspada + baque do punho.
fn snikt(sr: u32) -> Vec<f32> {
    let s = render(sr, 0.5, |t, r| {
        let mut s = (TAU * (140.0 + 200.0 * (-t / 0.01).exp()) * t).sin() * (-t / 0.035).exp() * 0.8;
        for (k, o) in [0.0f32, 0.035, 0.07].into_iter().enumerate() {
            let tt = t - o;
            if tt < 0.0 {
                continue;
            }
            let f = 1700.0 + k as f32 * 230.0;
            let ring = (TAU * f * tt).sin() + 0.6 * (TAU * f * 1.51 * tt).sin() + 0.35 * (TAU * f * 2.37 * tt).sin();
            s += ring * (-tt / 0.11).exp() * 0.35 + r.f() * (-tt / 0.012).exp() * 0.45;
        }
        s
    });
    norm(s, 0.75)
}

fn sheath(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    let s = render(sr, 0.25, move |t, r| {
        lp += (r.f() - lp) * 0.3;
        let ring = (TAU * (1500.0 - 2400.0 * t) * t).sin() * (-t / 0.06).exp();
        lp * (1.0 - t / 0.25) * 0.8 + ring * 0.4 + (TAU * 110.0 * t).sin() * (-t / 0.03).exp() * 0.6
    });
    norm(s, 0.5)
}

/// Golpe no ar: ruído filtrado grave com o filtro varrendo (lp menor = mais grave).
fn whoosh(sr: u32, lp_k: f32, dur: f32, thump: f32) -> Vec<f32> {
    let (mut a, mut b) = (0.0, 0.0);
    let s = render(sr, dur, move |t, r| {
        let k = lp_k * (0.6 + 0.8 * (t / dur * PI).sin());
        a += (r.f() - a) * k;
        b += (a - b) * k;
        let env = (t / (dur * 0.25)).min(1.0) * (1.0 - t / dur).max(0.0).powf(1.5);
        b * env * 6.0 + (TAU * thump * t).sin() * env * 0.2
    });
    norm(s, 0.6)
}

/// Garra entrando: baque grave + rasgo + clique.
fn hit(sr: u32) -> Vec<f32> {
    let (mut a, mut c) = (0.0, 0.0);
    let s = render(sr, 0.3, move |t, r| {
        a += (r.f() - a) * 0.12;
        c += (a - c) * 0.12;
        let thud = (TAU * (50.0 + 90.0 * (-t / 0.02).exp()) * t).sin() * (-t / 0.09).exp();
        let rip = c * 5.0 * (-t / 0.05).exp();
        let tick = r.f() * (-t / 0.004).exp() * 0.4;
        ((thud * 1.4 + rip + tick) * 1.5).tanh()
    });
    norm(s, 0.9)
}

/// Rugido: dente-de-serra grave com rosnado (AM) por três formantes de "A" aberto + bafo.
fn roar(sr: u32) -> Vec<f32> {
    let srf = sr as f32;
    let (mut ph, mut lp) = (0.0f32, 0.0f32);
    let mut st = [[0.0f32; 2]; 3];
    let s = render(sr, 1.5, move |t, r| {
        let f0 = 82.0 + 30.0 * (t / 0.25).min(1.0) - 25.0 * (t / 1.5) + (t * 31.0).sin() * 4.0;
        ph = (ph + f0 / srf).fract();
        let growl = 1.0 + 0.6 * (TAU * 27.0 * t).sin();
        lp += (r.f() - lp) * 0.3;
        let x = (1.0 - 2.0 * ph) * growl + lp * 0.8;
        let mut y = 0.0;
        for (s, (f, bw, g)) in st.iter_mut().zip([(650.0f32, 120.0f32, 1.0f32), (1080.0, 150.0, 0.7), (2400.0, 250.0, 0.25)]) {
            let rr = (-PI * bw / srf).exp();
            let v = (1.0 - rr) * x + 2.0 * rr * (TAU * f / srf).cos() * s[0] - rr * rr * s[1];
            s[1] = s[0];
            s[0] = v;
            y += v * g;
        }
        let env = (t / 0.08).min(1.0) * ((1.5 - t) / 0.5).clamp(0.0, 1.0);
        (y * env * 3.0).tanh() + (TAU * f0 * 0.5 * t).sin() * env * 0.25
    });
    norm(s, 0.9)
}

/// Garra contra laser/lâmina; `bright` > 1 = aparada perfeita.
fn clang(sr: u32, bright: f32) -> Vec<f32> {
    let s = render(sr, 0.6, move |t, r| {
        let mut s = 0.0;
        for (f, d, g) in [(1180.0f32, 0.25f32, 1.0f32), (1790.0, 0.18, 0.7), (2630.0, 0.12, 0.5), (3870.0, 0.07, 0.35)] {
            s += (TAU * f * bright * t).sin() * (-t / d).exp() * g;
        }
        s * 0.4 + r.f() * (-t / 0.006).exp() * 0.6 + (TAU * 180.0 * t).sin() * (-t / 0.03).exp() * 0.5
    });
    norm(s, 0.7)
}

fn stab(sr: u32) -> Vec<f32> {
    let mut a = 0.0;
    let s = render(sr, 0.16, move |t, r| {
        a += (r.f() - a) * 0.2;
        (TAU * (70.0 + 60.0 * (-t / 0.015).exp()) * t).sin() * (-t / 0.05).exp() * 1.2 + a * 3.0 * (-t / 0.025).exp()
    });
    norm(s, 0.8)
}

fn scrape(sr: u32) -> Vec<f32> {
    let (mut a, mut b) = (0.0, 0.0);
    let s = render(sr, 0.3, move |t, r| {
        a += (r.f() - a) * 0.5;
        b += (a - b) * 0.08;
        (a - b) * (t / 0.03).min(1.0) * (1.0 - t / 0.3) * (1.0 + 0.5 * (TAU * 60.0 * t).sin())
    });
    norm(s, 0.35)
}

fn land(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    let s = render(sr, 0.4, move |t, r| {
        lp += (r.f() - lp) * 0.05;
        (TAU * (40.0 + 100.0 * (-t / 0.04).exp()) * t).sin() * (-t / 0.15).exp() * 1.4 + lp * 4.0 * (-t / 0.1).exp()
    });
    norm(s, 0.9)
}

fn clips(sr: u32) -> Vec<Clip> {
    let c = |v: Vec<f32>| Arc::new(v);
    vec![
        c(snikt(sr)),
        c(sheath(sr)),
        c(whoosh(sr, 0.07, 0.26, 70.0)),
        c(whoosh(sr, 0.055, 0.28, 60.0)),
        c(whoosh(sr, 0.085, 0.24, 80.0)),
        c(whoosh(sr, 0.04, 0.42, 45.0)),
        c(hit(sr)),
        c(roar(sr)),
        c(clang(sr, 1.0)),
        c(clang(sr, 1.45)),
        c(stab(sr)),
        c(scrape(sr)),
        c(land(sr)),
    ]
}

// ---------------------------------------------------------------- Pose e modelo

#[derive(Default)]
struct P {
    lean: f32,
    pitch: f32,
    spin: f32,
    bounce: f32,
    crouch: f32,
    twist: f32,
    arm_l: f32,
    arm_r: f32,
    out_l: f32,
    out_r: f32,
    leg_l: f32,
    leg_r: f32,
    nod: f32,
    roar: bool,
}

fn pose(a: &Anim, time: f32) -> P {
    let t = a.t;
    let mut p = P { arm_l: -0.15, arm_r: -0.15, out_l: 0.12, out_r: 0.12, ..Default::default() };
    let lerp = |x: f32, y: f32, k: f32| x + (y - x) * smooth(k);
    match a.act {
        MOVE if a.run > 0.05 => {
            let s = a.walk.sin();
            let amp = 0.4 + 0.6 * a.run.min(1.0);
            p.leg_l = s * 0.9 * amp;
            p.leg_r = -s * 0.9 * amp;
            if a.run > 0.8 {
                p.arm_l = 0.7 + s * 0.5;
                p.arm_r = 0.7 - s * 0.5;
                p.out_l = 0.25;
                p.out_r = 0.25;
                p.lean = 0.45;
                p.nod = -0.3;
            } else {
                p.arm_l = -s * 0.8 * amp - 0.3;
                p.arm_r = s * 0.8 * amp - 0.3;
                p.lean = 0.12;
            }
            p.bounce = s.abs() * 0.06 * a.run;
        }
        MOVE => {
            p.arm_l = -0.45;
            p.arm_r = -0.45;
            p.out_l = 0.35;
            p.out_r = 0.35;
            p.crouch = 0.15;
            p.bounce = (time * 2.5).sin() * 0.015;
        }
        AIR if a.vy > 0.0 => {
            p.arm_l = -2.6;
            p.arm_r = -2.6;
            p.out_l = 0.3;
            p.out_r = 0.3;
            p.leg_l = -0.8;
            p.leg_r = 0.3;
        }
        AIR => {
            p.arm_l = -1.2;
            p.arm_r = -1.2;
            p.out_l = 1.0;
            p.out_r = 1.0;
            p.leg_l = -0.4;
            p.leg_r = 0.2;
        }
        SLASH => {
            let (n, side) = (a.aux & 3, a.aux >> 2);
            let (_, w0, w1) = slash_timing(n);
            let k = (t - w0 * 0.5) / (w1 - w0 * 0.5);
            p.leg_l = 0.3;
            p.leg_r = -0.35;
            match (n, side) {
                (3, _) => {
                    p.arm_l = lerp(0.4, -2.8, k);
                    p.arm_r = p.arm_l;
                    p.out_l = 0.5;
                    p.out_r = 0.5;
                    p.crouch = (1.0 - k.clamp(0.0, 1.0)) * 0.4;
                    p.bounce = smooth(k) * 0.15;
                }
                (_, 3) => {
                    p.arm_r = lerp(-1.2, -1.65, k);
                    p.arm_l = -0.5;
                    p.lean = 0.25 * smooth(k);
                    p.twist = lerp(-0.4, 0.3, k);
                }
                (2, 0) => {
                    p.arm_l = lerp(-2.9, -0.7, k);
                    p.arm_r = p.arm_l;
                    p.out_l = lerp(0.6, -0.15, k);
                    p.out_r = p.out_l;
                }
                (n, side) if side == 1 || (side == 0 && n == 0) => {
                    p.arm_r = -1.5;
                    p.out_r = lerp(1.3, -0.2, k);
                    p.twist = lerp(-0.9, 0.9, k);
                    p.arm_l = -0.6;
                }
                _ => {
                    p.arm_l = -1.5;
                    p.out_l = lerp(1.3, -0.2, k);
                    p.twist = lerp(0.9, -0.9, k);
                    p.arm_r = -0.6;
                }
            }
        }
        HEAVY => {
            if t < 0.26 {
                let k = t / 0.26;
                p.twist = -1.1 * smooth(k);
                p.arm_r = 0.6 * smooth(k);
                p.arm_l = -1.0;
                p.crouch = 0.3 * smooth(k);
            } else {
                let k = (t - 0.26) / 0.14;
                p.twist = lerp(-1.1, 1.1, k);
                p.arm_l = -1.5;
                p.arm_r = -1.5;
                p.out_r = lerp(1.2, -0.4, k);
                p.out_l = lerp(-0.4, 1.2, k);
                p.crouch = 0.3;
                p.lean = 0.2;
            }
            p.leg_l = 0.4;
            p.leg_r = -0.5;
        }
        UPPER => {
            let k = t / 0.2;
            p.arm_r = lerp(0.4, -3.0, k);
            p.arm_l = -0.3;
            p.crouch = (1.0 - k.clamp(0.0, 1.0)) * 0.5;
            p.lean = -0.15 * smooth(k);
            p.leg_r = -0.8 * smooth(k);
        }
        AIRSLASH => {
            let k = (t - 0.03) / 0.14;
            match a.aux & 3 {
                0 => {
                    p.arm_r = lerp(-2.8, -0.4, k);
                    p.twist = lerp(-0.5, 0.5, k);
                    p.arm_l = -1.0;
                }
                1 => {
                    p.arm_l = lerp(-2.8, -0.4, k);
                    p.twist = lerp(0.5, -0.5, k);
                    p.arm_r = -1.0;
                }
                _ => {
                    p.pitch = TAU * smooth(t / 0.3);
                    p.arm_l = -2.5;
                    p.arm_r = -2.5;
                    p.out_l = 0.8;
                    p.out_r = 0.8;
                    p.leg_l = -1.2;
                    p.leg_r = -1.2;
                }
            }
            p.leg_l = p.leg_l.min(-0.5);
            p.leg_r = p.leg_r.max(0.3);
        }
        DIVE => {
            p.lean = 1.2;
            p.arm_l = -2.9;
            p.arm_r = -2.9;
            p.out_l = 0.25;
            p.out_r = 0.25;
            p.leg_l = 0.4;
            p.leg_r = 0.4;
        }
        DIVE_LAND => {
            p.crouch = 0.8;
            p.arm_l = -0.8;
            p.arm_r = 0.6;
            p.out_r = 0.6;
            p.nod = 0.2;
        }
        TORNADO => {
            p.spin = t * 22.0;
            p.arm_l = -0.2;
            p.arm_r = -0.2;
            p.out_l = 1.45;
            p.out_r = 1.45;
            p.lean = 0.15;
        }
        POUNCE => {
            p.lean = 1.15;
            p.arm_l = -2.9;
            p.arm_r = -2.9;
            p.out_l = 0.35;
            p.out_r = 0.35;
            p.leg_l = 0.6;
            p.leg_r = 0.3;
            p.nod = -0.4;
        }
        MOUNT => {
            p.crouch = 0.5;
            p.lean = 0.4;
            p.arm_l = -1.9;
            p.out_l = 0.3;
            p.arm_r = if a.stab < 0.06 { -2.9 + 1.9 * (a.stab / 0.06) } else { -1.0 - 1.9 * ((a.stab - 0.06) / 0.2).min(1.0) };
            let s = a.walk.sin();
            p.leg_l = -0.6 + s * 0.3;
            p.leg_r = -0.6 - s * 0.3;
        }
        GUARD => {
            p.arm_l = -1.45;
            p.arm_r = -1.45;
            p.out_l = -0.55;
            p.out_r = -0.55;
            p.crouch = 0.3;
            p.lean = 0.1;
            p.nod = 0.15;
        }
        ROLL => {
            p.pitch = TAU * smooth(t / 0.45);
            p.arm_l = -1.2;
            p.arm_r = -1.2;
            p.leg_l = -1.4;
            p.leg_r = -1.4;
            p.crouch = 0.3;
        }
        CLIMB => {
            let s = (a.walk * 1.5).sin();
            p.arm_l = -2.9 + s * 0.35;
            p.arm_r = -2.9 - s * 0.35;
            p.leg_l = s * 0.5 - 0.3;
            p.leg_r = -s * 0.5 - 0.3;
            p.lean = -0.05;
        }
        KNOCK => {
            p.lean = if a.ground { -1.4 } else { -0.6 };
            p.bounce = if a.ground { 0.15 } else { 0.0 };
            p.arm_l = -2.4;
            p.arm_r = -2.4;
            p.out_l = 1.0;
            p.out_r = 1.0;
        }
        DOWN => {
            p.lean = -FRAC_PI_2 * smooth(t * 3.0);
            p.bounce = 0.12 * smooth(t * 3.0);
            p.out_l = 1.3;
            p.out_r = 1.3;
            p.arm_l = -0.2;
            p.arm_r = -0.2;
            p.nod = (t * 3.0).sin() * 0.05;
        }
        ROAR => {
            p.arm_l = 0.2;
            p.arm_r = 0.2;
            p.out_l = 1.0;
            p.out_r = 1.0;
            p.crouch = 0.35;
            p.nod = -0.5;
            p.roar = true;
            p.bounce = (time * 60.0).sin() * 0.02;
        }
        _ => {}
    }
    p.crouch = p.crouch.max(a.squat);
    p
}

/// Modelo blocado original. Retorna (cabeça, pontas das garras esquerda/direita).
fn draw_model(b: &mut Batch, pos: Vec3, yaw: f32, a: &Anim, flash: f32, time: f32) -> (Vec3, [Vec3; 2]) {
    let p = pose(a, time);
    let tint = |c: Color| {
        let mut c = c;
        if a.berserk {
            c = Color::new((c.r * 0.8 + 0.3).min(1.0), c.g * 0.55, c.b * 0.55, 1.0);
        }
        let f = flash * 0.75;
        Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f * 0.6, c.b + (1.0 - c.b) * f * 0.6, 1.0)
    };
    let yellow = tint(rgb(0.98, 0.8, 0.1));
    let blue = tint(rgb(0.12, 0.25, 0.75));
    let blue_d = tint(rgb(0.07, 0.15, 0.5));
    let black = tint(rgb(0.06, 0.06, 0.08));
    let skin = tint(rgb(0.86, 0.66, 0.52));
    let hair = tint(rgb(0.12, 0.08, 0.05));
    let belt = tint(rgb(0.55, 0.12, 0.08));
    let flesh = rgb(0.7, 0.08, 0.08);
    let silver = rgb(0.8, 0.84, 0.9);
    let steel = if a.berserk { rgb(1.0, 0.72, 0.7) } else { rgb(0.86, 0.9, 0.97) };
    let t = |x: f32, y: f32, z: f32| Mat4::from_translation(vec3(x, y, z));
    let cr = p.crouch;
    let base = t(pos.x, pos.y + p.bounce - 0.22 * cr, pos.z)
        * Mat4::from_rotation_y(yaw + p.spin)
        * Mat4::from_rotation_x(p.lean + cr * 0.25)
        * t(0.0, 0.9, 0.0)
        * Mat4::from_rotation_x(p.pitch)
        * t(0.0, -0.9, 0.0);

    // Pernas: amarelo com bota azul de cano alto
    for (side, ang) in [(1.0f32, p.leg_l - cr * 0.8), (-1.0, p.leg_r - cr * 0.8)] {
        let l = base * t(side * 2.0 * U, 12.0 * U, 0.0) * Mat4::from_rotation_x(ang);
        b.cube(&l, vec3(0.0, -4.0 * U, 0.0), vec3(4.0 * U, 8.0 * U, 4.0 * U), yellow);
        b.cube(&l, vec3(0.0, -9.5 * U, 0.2 * U), vec3(4.3 * U, 5.0 * U, 4.6 * U), blue);
        b.cube(&l, vec3(0.0, -7.2 * U, 0.0), vec3(4.5 * U, 0.8 * U, 4.5 * U), blue_d);
    }
    // Sunga azul e cinto com fivela
    b.cube(&base, vec3(0.0, 12.6 * U, 0.0), vec3(8.2 * U, 3.2 * U, 4.2 * U), blue);
    b.cube(&base, vec3(0.0, 14.6 * U, 0.0), vec3(8.3 * U, 1.2 * U, 4.3 * U), belt);
    b.cube(&base, vec3(0.0, 14.6 * U, 2.2 * U), vec3(2.0 * U, 1.2 * U, 0.3 * U), yellow);

    // Tronco (gira no golpe): amarelo, laterais azuis listradas, ombreira azul
    let u = base * t(0.0, 13.0 * U, 0.0) * Mat4::from_rotation_y(p.twist) * t(0.0, -13.0 * U, 0.0);
    b.cube(&u, vec3(0.0, 19.6 * U, 0.0), vec3(8.0 * U, 9.2 * U, 4.0 * U), yellow);
    for side in [1.0f32, -1.0] {
        b.cube(&u, vec3(side * 3.55 * U, 19.5 * U, 0.0), vec3(1.0 * U, 9.0 * U, 4.2 * U), blue);
        for k in 0..3 {
            for z in [2.12f32, -2.12] {
                let s = u * t(side * 3.0 * U, (17.4 + k as f32 * 2.3) * U, z * U) * Mat4::from_rotation_z(side * 0.5);
                b.cube(&s, Vec3::ZERO, vec3(2.2 * U, 0.6 * U, 0.15 * U), black);
            }
        }
    }
    b.cube(&u, vec3(0.0, 23.6 * U, 0.0), vec3(8.4 * U, 1.6 * U, 4.4 * U), blue);
    // Dano: uniforme rasgado; com pouca vida aparece a costela de adamantium
    if a.stage >= 1 {
        b.cube(&u, vec3(2.0 * U, 20.5 * U, 2.06 * U), vec3(2.4 * U, 1.8 * U, 0.12 * U), skin);
        b.cube(&u, vec3(-1.6 * U, 17.2 * U, 2.06 * U), vec3(1.8 * U, 2.4 * U, 0.12 * U), skin);
        for k in 0..3 {
            let s = u * t((-2.5 + k as f32) * U, 21.0 * U, 2.1 * U) * Mat4::from_rotation_z(0.6);
            b.cube(&s, Vec3::ZERO, vec3(0.35 * U, 3.2 * U, 0.1 * U), flesh);
        }
    }
    if a.stage >= 2 {
        b.cube(&u, vec3(0.6 * U, 19.8 * U, 2.07 * U), vec3(5.4 * U, 5.6 * U, 0.12 * U), flesh);
        for k in 0..4 {
            b.glow(&u, vec3(0.6 * U, (17.6 + k as f32 * 1.4) * U, 2.14 * U), vec3(4.6 * U, 0.55 * U, 0.12 * U), silver);
        }
        b.glow(&u, vec3(0.6 * U, 19.6 * U, 2.16 * U), vec3(0.8 * U, 5.4 * U, 0.1 * U), silver);
    }

    // Braços: ombro azul, antebraço amarelo, luva azul e as três garras
    let mut tips = [Vec3::ZERO; 2];
    for (k, (side, ang, out)) in [(1.0f32, p.arm_l, p.out_l), (-1.0, p.arm_r, p.out_r)].into_iter().enumerate() {
        let m = u * t(side * 6.0 * U, 22.5 * U, 0.0) * Mat4::from_rotation_x(ang) * Mat4::from_rotation_z(side * out);
        b.cube(&m, vec3(0.0, -1.0 * U, 0.0), vec3(4.2 * U, 5.0 * U, 4.2 * U), blue);
        b.cube(&m, vec3(0.0, -5.8 * U, 0.0), vec3(4.0 * U, 4.6 * U, 4.0 * U), yellow);
        b.cube(&m, vec3(0.0, -9.2 * U, 0.0), vec3(4.3 * U, 2.2 * U, 4.3 * U), blue);
        b.cube(&m, vec3(0.0, -11.0 * U, 0.0), vec3(3.8 * U, 1.8 * U, 3.8 * U), blue_d);
        if a.stage >= 2 {
            b.glow(&m, vec3(0.0, -6.0 * U, 2.06 * U), vec3(1.3 * U, 4.4 * U, 0.14 * U), silver);
        }
        let len = 8.0 * U * a.claws;
        if len > 0.02 {
            for x in [-1.2f32, 0.0, 1.2] {
                b.glow(&m, vec3(x * U, -11.8 * U - len * 0.5, 0.9 * U), vec3(0.4 * U, len, 0.5 * U), steel);
            }
        }
        tips[k] = m.transform_point3(vec3(0.0, -11.8 * U - len, 0.9 * U));
    }

    // Cabeça: máscara amarela de abas pretas, queixo e costeleta à mostra
    let h = u * t(0.0, 24.2 * U, 0.0) * Mat4::from_rotation_x(p.nod);
    let fz = 4.0 * U;
    b.cube(&h, vec3(0.0, 4.0 * U, 0.0), vec3(8.0 * U, 8.0 * U, 8.0 * U), yellow);
    b.cube(&h, vec3(0.0, 1.6 * U, 0.0), vec3(8.1 * U, 3.2 * U, 8.1 * U), skin);
    b.cube(&h, vec3(0.0, 4.4 * U, fz + 0.05 * U), vec3(7.0 * U, 2.4 * U, 0.2 * U), black);
    let eye = if a.berserk { rgb(1.0, 0.15, 0.1) } else { WHITE };
    for side in [1.0f32, -1.0] {
        b.glow(&h, vec3(side * 1.9 * U, 4.3 * U, fz + 0.2 * U), vec3(2.2 * U, 0.8 * U, 0.2 * U), eye);
        let tall = if a.stage >= 2 && side > 0.0 { 2.5 } else { 5.0 };
        let fin = h * t(side * 3.4 * U, 6.5 * U, 0.5 * U) * Mat4::from_rotation_z(-side * 0.45);
        b.cube(&fin, vec3(0.0, tall * 0.5 * U, 0.0), vec3(1.0 * U, tall * U, 3.0 * U), black);
        b.cube(&h, vec3(side * 4.1 * U, 2.0 * U, 1.0 * U), vec3(0.4 * U, 3.5 * U, 3.0 * U), hair);
    }
    if p.roar {
        b.cube(&h, vec3(0.0, 1.0 * U, fz + 0.1 * U), vec3(3.4 * U, 1.6 * U, 0.2 * U), rgb(0.3, 0.02, 0.02));
        b.glow(&h, vec3(0.0, 1.7 * U, fz + 0.15 * U), vec3(3.0 * U, 0.3 * U, 0.15 * U), WHITE);
    } else {
        b.cube(&h, vec3(0.0, 1.2 * U, fz + 0.1 * U), vec3(3.0 * U, 0.6 * U, 0.2 * U), rgb(0.35, 0.1, 0.1));
    }
    if a.stage >= 1 {
        b.cube(&h, vec3(2.2 * U, 6.0 * U, fz + 0.06 * U), vec3(2.0 * U, 1.6 * U, 0.12 * U), skin);
    }
    if a.stage >= 2 {
        b.glow(&h, vec3(-2.0 * U, 3.0 * U, fz + 0.12 * U), vec3(3.8 * U, 4.6 * U, 0.14 * U), silver);
        b.glow(&h, vec3(-2.0 * U, 4.3 * U, fz + 0.22 * U), vec3(1.3 * U, 1.0 * U, 0.1 * U), rgb(1.0, 0.3, 0.2));
    }
    (h.transform_point3(vec3(0.0, 8.0 * U, 0.0)), tips)
}

// ---------------------------------------------------------------- Jogável + remotos

struct RState {
    act: u8,
    aux: u8,
    t: f32,
    at: f64,
    claws: f32,
    bz: bool,
    stage: u8,
}

/// Roteiro de teste (?wolvie=combo|pounce|rage; nativo URNA_WOLVIE e URNA_WOLVIE_SHOT=pasta pros PNGs).
struct Demo {
    kind: u8,
    t: f32,
    placed: bool,
    picked: bool,
    shots: Option<String>,
    n: usize,
}

const DEMO_SHOTS: [[f32; 3]; 3] = [[0.95, 2.62, 3.3], [1.3, 2.5, 3.8], [0.5, 1.7, 3.4]];

impl Demo {
    fn from_env() -> Option<Demo> {
        #[cfg(target_arch = "wasm32")]
        let (k, shots) = (crate::web::query("wolvie"), None);
        #[cfg(not(target_arch = "wasm32"))]
        let (k, shots) = (std::env::var("URNA_WOLVIE").ok(), std::env::var("URNA_WOLVIE_SHOT").ok());
        let kind = match k?.as_str() {
            "combo" => 0,
            "pounce" => 1,
            "rage" => 2,
            _ => return None,
        };
        if let Some(d) = &shots {
            let _ = std::fs::create_dir_all(d);
        }
        Some(Demo { kind, t: 0.0, placed: false, picked: false, shots, n: 0 })
    }

    fn drive(&mut self, inp: &mut Intent, player: &mut Player, b: &mut Body, world: &World, targets: &[Target], dt: f32) {
        let t0 = self.t;
        self.t += dt;
        let t = self.t;
        *inp = Intent::default();
        if !self.placed {
            let want = if self.kind == 1 { npc::URNA } else { npc::FIGHTER };
            let Some(tg) = targets.iter().find(|x| x.3 == want).copied() else { return };
            self.placed = true;
            let p = tg.0 + if self.kind == 1 { vec3(12.0, 0.0, 6.0) } else { vec3(2.0, 0.0, 1.0) };
            b.pos = vec3(p.x, world.floor_at(p.x, G as f32 + 10.0, p.z), p.z);
            b.lock = Some((tg.3, tg.2));
        }
        if let Some(tg) = b.lock.and_then(|l| find(targets, l)) {
            let d = tg.0 - b.pos;
            player.yaw = d.z.atan2(d.x);
            player.pitch = if self.kind == 1 { 0.05 } else { -0.3 };
        }
        let (k0, k) = (t0 % 5.0, t % 5.0);
        let at = |x: f32| k0 < x && k >= x;
        match self.kind {
            0 => {
                inp.light = [0.3, 0.55, 0.8, 1.05, 2.95, 3.2, 3.45].into_iter().any(at);
                inp.heavy = at(1.9) || at(2.5) || at(3.75);
                if (2.45..2.55).contains(&k) {
                    inp.stick.y = -1.0;
                }
            }
            1 => {
                inp.pounce = t > 0.8 && !matches!(b.act, MOUNT | POUNCE) && t as i32 != t0 as i32;
                inp.light = b.act == MOUNT && (t * 8.0) as i32 != (t0 * 8.0) as i32;
                inp.stick.y = if b.act == MOUNT { 0.6 } else { 0.0 };
            }
            _ => {
                if b.berserk <= 0.0 && b.act != ROAR {
                    b.rage = 100.0;
                    inp.rage = true;
                }
                if at(1.2) || b.act == TORNADO {
                    inp.sprint = true;
                    inp.stick.y = 1.0;
                    inp.heavy = at(1.2);
                }
                inp.light = [3.0, 3.25, 3.5, 3.75].into_iter().any(at);
            }
        }
    }
}

pub struct Wolvie {
    /// Corpo do jogador local quando ele escolhe o WOLVERINE.
    pub me: Option<Body>,
    clips: Vec<Clip>,
    /// Sons pra tocar no main (clipe, posição, volume base).
    pub sfx: Vec<(Clip, Vec3, f32)>,
    remote: HashMap<u64, RState>,
    pvp_out: Vec<Value>,
    cam: Option<Vec3>,
    pops: Vec<(Vec3, String, Color, f32)>,
    trail: Vec<([Vec3; 2], f32)>,
    beam: Option<(Vec3, Vec3, f32)>,
    flash: f32,
    demo: Option<Demo>,
}

impl Wolvie {
    pub fn new(sr: u32) -> Self {
        Wolvie { me: None, clips: clips(sr), sfx: Vec::new(), remote: HashMap::new(), pvp_out: Vec::new(), cam: None, pops: Vec::new(), trail: Vec::new(), beam: None, flash: 0.0, demo: Demo::from_env() }
    }

    pub fn yaw(&self) -> f32 {
        self.me.as_ref().map_or(0.0, |b| b.yaw)
    }

    pub fn rage_full(&self) -> bool {
        self.me.as_ref().is_some_and(|b| b.rage >= 100.0 && b.berserk <= 0.0)
    }

    pub fn demo_on(&self) -> bool {
        self.demo.is_some()
    }

    /// Roteiro de teste: escolhe o personagem no 1º frame.
    pub fn demo_pick(&mut self) -> bool {
        self.demo.as_mut().is_some_and(|d| !std::mem::replace(&mut d.picked, true))
    }

    /// Depois de desenhar: PNGs do roteiro nativo. true = acabou (o main fecha).
    pub fn after_frame(&mut self) -> bool {
        let Some(d) = self.demo.as_mut() else { return false };
        let Some(dir) = d.shots.as_ref() else { return false };        match DEMO_SHOTS[d.kind as usize].get(d.n) {
            Some(&ts) if d.t >= ts => {
                get_screen_data().export_png(&format!("{dir}/wolvie_{}_{}.png", ["combo", "pounce", "rage"][d.kind as usize], d.n));
                d.n += 1;
                false
            }
            Some(_) => false,
            None => true,
        }
    }

    /// Explosão/laser em mim: multiplicador do empurrão (defesa segura, no tempo certo devolve).
    pub fn blast(&mut self, from: Vec3) -> f32 {
        let Some(b) = self.me.as_mut() else { return 1.0 };
        if b.invuln > 0.0 || b.act == ROAR {
            return 0.0;
        }
        let to = from - b.pos;
        if b.act != GUARD || fwd(b.yaw).dot(vec3(to.x, 0.0, to.z).normalize_or_zero()) < 0.0 {
            return 1.0;
        }
        b.idle = 0.0;
        if b.guard_t < 0.3 {
            b.reflect = Some(from);
            b.snd.push((S_PARRY, 1.4));
            b.rage = (b.rage + 15.0).min(100.0);
            b.pops.push((b.pos + Vec3::Y * 2.3, "LASER DEVOLVIDO!".into(), SKYBLUE));
            self.flash = 1.0;
            return 0.0;
        }
        b.snd.push((S_CLANG, 1.1));
        b.pops.push((b.pos + Vec3::Y * 2.3, "DEFENDEU".into(), Color::new(0.8, 0.85, 0.95, 1.0)));
        0.12
    }

    /// Dano de PvP/mod em mim. Retorna o multiplicador do empurrão do main (0 = eu cuido).
    pub fn damage(&mut self, dmg: f32, dir: Vec3) -> f32 {
        let Some(b) = self.me.as_mut() else { return 1.0 };
        let knock = if dir == Vec3::ZERO { Vec3::ZERO } else { dir * 4.0 + Vec3::Y * 2.0 };
        b.hurt(dmg, knock, Some(-dir));
        0.0
    }

    /// WOLVERINE jogável: corpo próprio, golpes viram mensagens "a" (o host aplica) e PvP.
    #[allow(clippy::too_many_arguments)]
    pub fn play(&mut self, world: &World, dt: f32, inp: &Intent, player: &mut Player, targets: &[Target], others: &[(u64, Vec3)], fx: &mut Fx) -> Vec<Value> {
        let Some(mut b) = self.me.take() else { return Vec::new() };
        let mut inp = *inp;
        if let Some(d) = self.demo.as_mut() {
            d.drive(&mut inp, player, &mut b, world, targets, dt);
        }
        let knock = std::mem::take(&mut player.knock);
        if knock.length() > 2.0 {
            b.hurt(knock.length() * 4.0, knock, None);
        }
        let cam = player.forward();
        if inp.lock {
            b.lock = if b.lock.is_some() { None } else { pick(targets, b.pos + Vec3::Y * 1.6, cam, 30.0) };
        }
        b.step(world, dt, &inp, cam, targets, others, fx);
        if let Some(o) = b.reflect.take() {
            let from = b.pos + Vec3::Y * 1.2;
            self.beam = Some((from, o, 0.4));
            if let Some(t) = targets.iter().filter(|t| t.0.distance(o) < t.1 + 8.0).min_by(|a, c| a.0.distance_squared(o).total_cmp(&c.0.distance_squared(o))) {
                b.strikes.push(Strike { g: t.3, i: t.2, pvp: None, dmg: 45.0, dir: (t.0 - from).normalize_or_zero(), at: t.0, w: "LASER DEVOLVIDO" });
                b.after_hit(45.0);
            }
        }
        if let Some(t) = b.lock.and_then(|l| find(targets, l)).filter(|_| b.act != MOUNT) {
            let d = t.0 - b.pos;
            player.yaw += wrap(d.z.atan2(d.x) - player.yaw) * (dt * 5.0).min(1.0);
        }
        player.pos = b.pos;
        player.vel = Vec3::ZERO;
        let r = |x: f32| (x * 10.0).round() / 10.0;
        let mut out = Vec::new();
        for s in b.strikes.drain(..) {
            match s.pvp {
                Some(id) => self.pvp_out.push(json!([id, r(s.dmg * 0.2), crate::mp::v3(s.dir)])),
                None => out.push(json!({"t": "a", "k": "hit", "g": s.g, "i": s.i, "d": crate::mp::v3(s.dir), "p": crate::mp::v3(s.at), "dmg": r(s.dmg), "w": s.w})),
            }
        }
        for (k, v) in b.snd.drain(..) {
            self.sfx.push((self.clips[k as usize].clone(), b.pos, v));
        }
        for (p, s, c) in b.pops.drain(..) {
            self.pops.push((p, s, c, 0.0));
        }
        self.me = Some(b);
        out
    }

    /// Câmera por cima do ombro (igual ao bandido), mais longe montado num gigante.
    pub fn camera(&mut self, world: &World, player: &Player, dt: f32) -> Option<(Vec3, Vec3)> {
        let b = self.me.as_ref()?;
        let up = Vec3::Y;
        let fw = player.forward();
        let right = vec3(-fw.z, 0.0, fw.x).normalize_or_zero();
        let head = b.pos + up * 1.6;
        let back = match b.act {
            MOUNT if b.mount.as_ref().is_some_and(|m| m.big) => 6.5,
            TORNADO | ROAR | DOWN => 4.4,
            _ => 3.4,
        };
        let want = -fw * back + right * 0.6 + up * 0.45;
        let dist = world.raycast(head, want.normalize(), want.length()).map(|h| (h.2 - 0.3).max(0.2)).unwrap_or(want.length());
        let e = head + want.normalize() * dist;
        let k = 1.0 - (-dt * 14.0).exp();
        let e = match self.cam {
            Some(c) if c.distance(e) < 15.0 => c.lerp(e, k),
            _ => e,
        };
        self.cam = Some(e);
        Some((e, fw))
    }

    /// Completa o "p" que vai sair: pose pros outros verem e dano PvP (junto com o do Steve).
    pub fn fill_p(&mut self, v: &mut Value) {
        if let Some(b) = &self.me {
            let r = |x: f32| (x * 100.0).round() / 100.0;
            v["wv"] = json!([b.act, b.aux, r(b.t), r(b.claws), (b.berserk > 0.0) as u8, b.stage()]);
        }
        if !self.pvp_out.is_empty() {
            let mut add = std::mem::take(&mut self.pvp_out);
            match v["pv"].as_array_mut() {
                Some(a) => a.append(&mut add),
                None => v["pv"] = Value::Array(add),
            }
        }
    }

    /// "p" de outro jogador: pose do Wolverine dele (e os sons de golpe/garra/rugido).
    pub fn on_p(&mut self, id: u64, m: &Value) {
        let Some(a) = m["wv"].as_array().filter(|a| a.len() >= 6) else {
            self.remote.remove(&id);
            return;
        };
        let g = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
        let r = RState { act: g(0) as u8, aux: g(1) as u8, t: g(2), at: get_time(), claws: g(3), bz: g(4) > 0.5, stage: g(5) as u8 };
        let pos = crate::mp::get_v3(&m["p"]);
        if let Some(old) = self.remote.get(&id) {
            let s = match r.act {
                _ if r.act == old.act => None,
                SLASH | AIRSLASH => Some((S_WHOOSH, 0.8)),
                HEAVY | UPPER | TORNADO | DIVE | POUNCE => Some((S_HEAVY, 0.9)),
                ROAR => Some((S_ROAR, 1.4)),
                _ => None,
            };
            if let Some((k, v)) = s {
                self.sfx.push((self.clips[k as usize].clone(), pos, v));
            }
            if r.claws > 0.5 && old.claws <= 0.5 {
                self.sfx.push((self.clips[S_SNIKT as usize].clone(), pos, 1.0));
            }
        }
        self.remote.insert(id, r);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_remote(&self, b: &mut Batch, id: u64, pos: Vec3, yaw: f32, walk: f32, spd: f32, vy: f32, time: f32) {
        let mut a = Anim { act: MOVE, walk: walk * 0.55, run: (spd / SPRINT).min(1.0), vy, ground: vy.abs() < 1.5, ..Default::default() };
        if let Some(r) = self.remote.get(&id) {
            (a.act, a.aux, a.t, a.claws, a.berserk, a.stage) = (r.act, r.aux, r.t + (get_time() - r.at).min(3.0) as f32, r.claws, r.bz, r.stage);
            a.stab = a.t % 0.25;
        }
        if a.act == MOVE && !a.ground {
            a.act = AIR;
        }
        draw_model(b, pos, yaw, &a, 0.0, time);
    }

    /// Eu (se for o Wolverine), rastro das garras e laser devolvido.
    pub fn draw(&mut self, b: &mut Batch, trans: &mut Batch, time: f32) {
        let dt = get_frame_time().min(0.05);
        for tr in self.trail.iter_mut() {
            tr.1 += dt;
        }
        self.trail.retain(|t| t.1 < 0.16);
        if let Some((a, c, t)) = self.beam.as_mut() {
            *t -= dt;
            crate::urna::beam(trans, *a, *c, 0.3 * *t / 0.4 + 0.05, Color::new(1.0, 0.35, 0.6, (*t / 0.4).min(1.0) * 0.8));
        }
        self.beam = self.beam.filter(|b| b.2 > 0.0);
        let Some(body) = self.me.as_ref() else { return };
        let (_, tips) = draw_model(b, body.pos, body.yaw, &body.anim(), body.flash, time);
        if body.claws > 0.5 && body.striking() {
            self.trail.push((tips, 0.0));
        }
        let col = if body.berserk > 0.0 { (1.0, 0.3, 0.25) } else { (0.75, 0.9, 1.0) };
        for w in self.trail.windows(2) {
            let alpha = (1.0 - w[1].1 / 0.16) * 0.6;
            for k in 0..2 {
                crate::urna::beam(trans, w[0].0[k], w[1].0[k], 0.07, Color::new(col.0, col.1, col.2, alpha));
            }
        }
    }

    /// HUD: vida/fúria, combo, trava, anel do tempo certo montado, tela vermelha no berserker.
    #[allow(clippy::too_many_arguments)]
    pub fn hud(&mut self, dt: f32, sw: f32, sh: f32, targets: &[Target], proj: impl Fn(Vec3) -> Option<Vec2>, text: impl Fn(&str, f32, f32, f32, Color), mobile: bool) {
        let time = get_time() as f32;
        for p in self.pops.iter_mut() {
            p.3 += dt;
            p.0.y += dt * 0.9;
        }
        self.pops.retain(|p| p.3 < 1.4);
        self.flash = (self.flash - dt * 3.0).max(0.0);
        for (pos, s, c, t) in &self.pops {
            if let Some(q) = proj(*pos) {
                text(s, q.x, q.y, 26.0, Color::new(c.r, c.g, c.b, (1.0 - t / 1.4).clamp(0.0, 1.0)));
            }
        }
        let Some(b) = self.me.as_ref() else { return };
        if b.berserk > 0.0 {
            let pulse = 0.5 + 0.5 * (time * 8.0).sin();
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.6, 0.0, 0.0, 0.12 + 0.05 * pulse));
            let e = Color::new(0.7, 0.0, 0.0, 0.25 + 0.1 * pulse);
            draw_rectangle(0.0, 0.0, sw, 40.0, e);
            draw_rectangle(0.0, sh - 40.0, sw, 40.0, e);
            draw_rectangle(0.0, 0.0, 40.0, sh, e);
            draw_rectangle(sw - 40.0, 0.0, 40.0, sh, e);
        }
        if self.flash > 0.0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(1.0, 1.0, 1.0, self.flash * 0.35));
        }
        if b.act == DOWN {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.1, 0.0, 0.0, 0.35));
            text(&format!("FATOR DE CURA... {:.0}", (3.5 - b.t).max(0.0).ceil()), sw * 0.5, sh * 0.45, 40.0, Color::new(1.0, 0.6, 0.65, 1.0));
        } else if b.hp < 33.0 {
            let e = Color::new(0.75, 0.8, 0.88, 0.18);
            draw_rectangle(0.0, 0.0, 30.0, sh, e);
            draw_rectangle(sw - 30.0, 0.0, 30.0, sh, e);
        }
        // Barras: vida (cura) e fúria
        let w = (sw * 0.36).min(380.0);
        let (x, y) = (sw * 0.5 - w * 0.5, sh - 66.0);
        let k = b.hp / 100.0;
        draw_rectangle(x - 2.0, y - 2.0, w + 4.0, 14.0, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(x, y, w * k, 10.0, Color::new(1.0, 0.25 + 0.6 * k, 0.1, 1.0));
        draw_rectangle(x - 2.0, y + 14.0, w + 4.0, 10.0, Color::new(0.0, 0.0, 0.0, 0.6));
        let full = b.rage >= 100.0 && b.berserk <= 0.0;
        let blink = full && (time * 6.0).sin() > 0.0;
        draw_rectangle(x, y + 16.0, w * b.rage / 100.0, 6.0, if blink { Color::new(1.0, 0.85, 0.3, 1.0) } else { Color::new(0.9, 0.1, 0.08, 1.0) });
        let label = if b.berserk > 0.0 {
            format!("BERSERKER {:.0}s", b.berserk.ceil())
        } else if full {
            if mobile { "FURIA CHEIA: TOCA FURIA".into() } else { "FURIA CHEIA: R".into() }
        } else if b.calm > 2.5 && b.hp < 100.0 {
            "REGENERANDO".into()
        } else {
            "WOLVERINE".into()
        };
        text(&label, sw * 0.5, y - 8.0, 18.0, if b.berserk > 0.0 || full { Color::new(1.0, 0.3, 0.2, 1.0) } else { Color::new(1.0, 0.85, 0.3, 1.0) });
        if b.combo_n >= 2 {
            text(&format!("{} GARRADAS", b.combo_n), sw * 0.82, sh * 0.42, 28.0 + (b.combo_n.min(20) as f32), Color::new(1.0, 0.85, 0.3, (1.0 - b.combo_t / 2.5).clamp(0.0, 1.0)));
        }
        // Trava: losango girando no alvo
        if let Some(q) = b.lock.and_then(|l| find(targets, l)).and_then(|t| proj(t.0)) {
            let rot = time * 3.0;
            for i in 0..4 {
                let a = rot + i as f32 * FRAC_PI_2;
                let (dx, dy) = (a.cos(), a.sin());
                let tip = vec2(q.x + dx * 14.0, q.y + dy * 14.0);
                let base = vec2(q.x + dx * 26.0, q.y + dy * 26.0);
                let perp = vec2(-dy, dx) * 6.0;
                draw_triangle(tip, base + perp, base - perp, Color::new(1.0, 0.25, 0.15, 0.95));
            }
        }
        if let (MOUNT, Some(m)) = (b.act, b.mount.as_ref()) {
            let (cx, cy) = (sw * 0.5, sh * 0.62);
            draw_circle_lines(cx, cy, 33.0, 17.0, Color::new(0.3, 1.0, 0.4, 0.35));
            draw_circle_lines(cx, cy, 20.0 + 90.0 * (1.0 - m.ring), 3.0, WHITE);
            let atk = if mobile { "GARRA" } else { "ESQ" };
            text(&format!("{atk} ESFAQUEIA - NO ANEL VERDE = CRITICO"), cx, cy + 80.0, 20.0, WHITE);
            let rest = match (m.big, mobile) {
                (true, false) => "WASD ESCALA | F CRAVA FUNDO | ESPACO PULA FORA",
                (true, true) => "ANALOGICO ESCALA | PULA SAI",
                (false, false) => "ESPACO PULA FORA",
                (false, true) => "PULA SAI",
            };
            text(rest, cx, cy + 104.0, 18.0, Color::new(0.85, 0.85, 0.9, 1.0));
        }
        if b.act == GUARD {
            text("DEFESA", sw * 0.5, sh * 0.5 + 60.0, 22.0, if b.guard_t < 0.3 { SKYBLUE } else { Color::new(0.8, 0.85, 0.95, 1.0) });
        }
    }
}
