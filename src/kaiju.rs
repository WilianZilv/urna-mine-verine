//! GODZILHA: kaiju paródia do tamanho da urna. Pisa forte, ruge, chicoteia o rabo e solta o
//! bafo atômico azul (as placas das costas acendem do rabo pra cabeça antes). Rival da urna.
//! O host simula IA e alvo e manda no snapshot ("kj"); os tiros vão como os da urna (`"w"`/`shot` com `by: 9`).

use crate::actors::{Ev, Fighter, Villager};
use crate::audio::Clip;
use crate::batch::Batch;
use crate::extras::Label;
use crate::models::rgb;
use crate::npc::{self, Npcs, Vida};
use crate::urna::{self, Fx, Particle, Plan, Urna, ik, limb};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

pub const BY: u64 = 9;

const WALK: u8 = 0;
const ROAR: u8 = 1;
const CHARGE: u8 = 2;
const BREATH: u8 = 3;
const SWIPE: u8 = 4;
const DUR: [f32; 5] = [0.0, 2.4, 2.0, 1.7, 1.3];

/// Altura do quadril; com tronco, pescoço e cabeça fica ~16 de altura, igual à urna.
const H: f32 = 6.5;
const HIP_X: f32 = 2.3;
const THIGH: f32 = 3.8;
const SHIN: f32 = 3.6;
const LEAN: f32 = 0.3;
const TAIL: usize = 10;
const SEG: f32 = 1.7;
/// Folga do escudo do clube / domo do lab (o corpo + rabo não entram).
const MARGIN: f32 = 9.0;

const FALAS: [&str; 8] = [
    "VIM DO LAGO PARANOA",
    "RADIACAO? E SO O 5G",
    "URNA, VOCE NAO ME REPRESENTA",
    "MEU BAFO E AUDITAVEL",
    "PISEI NO ORCAMENTO SECRETO",
    "ESSA PRACA E MINHA",
    "SOU O KAIJU DO CENTRAO",
    "NUCLEAR SO A EMENDA",
];

fn fwd(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

fn side(yaw: f32) -> Vec3 {
    vec3(yaw.cos(), 0.0, -yaw.sin())
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn floor(world: &World, p: Vec3) -> f32 {
    world.floor_at(p.x, G as f32 + 14.0, p.z)
}

fn xz(p: Vec3) -> Vec2 {
    vec2(p.x, p.z)
}

/// Zonas proibidas (centro, raio): escudo do clube e domo do lab.
fn keep_out() -> [(Vec2, f32); 2] {
    [(xz(shield_center()), SHIELD_R + MARGIN), (xz(crate::lab::dome_center()), crate::lab::DOME_R + MARGIN)]
}

/// Área do GODZILHA: a arena do norte, fora do clube e do lab.
fn zone_ok(p: Vec2) -> bool {
    crate::layout::in_arena(p, 0.0) && keep_out().iter().all(|(c, r)| p.distance(*c) > *r)
}

fn wander_point() -> Vec3 {
    loop {
        let p = crate::layout::arena_point(4.0);
        if zone_ok(p) {
            return vec3(p.x, G as f32, p.y);
        }
    }
}

struct Foot {
    pos: Vec3,
    from: Vec3,
    to: Vec3,
    t: f32,
}

struct Clips {
    roar: Clip,
    stomp: Clip,
    charge: Clip,
    breath: Clip,
    swipe: Clip,
}

pub struct Kaiju {
    pub root: Vec3,
    pos: Vec3,
    vel: Vec3,
    pub yaw: f32,
    st: u8,
    st_t: f32,
    pub target: Vec3,
    goal: Vec3,
    /// 0 urna, 1 jogador, 2 voador, 3 NPC (ponto fixo), 4 nada.
    focus: u8,
    focus_i: usize,
    focus_p: Vec3,
    focus_t: f32,
    cd: f32,
    shot_t: f32,
    smash_cd: f32,
    feet: [Foot; 2],
    tail: [Vec3; TAIL + 1],
    last_root: Vec3,
    root_vel: Vec3,
    net: Option<(Vec3, f32)>,
    alive: bool,
    host_alive: bool,
    crashed: bool,
    jaw: f32,
    glow: f32,
    roll: f32,
    beams: Vec<(Vec3, Vec3, f32)>,
    clips: Clips,
    /// Sons pra tocar no main (clipe, posição, volume base).
    pub sfx: Vec<(Clip, Vec3, f32)>,
}

impl Kaiju {
    pub fn new(sr: u32) -> Self {
        let root = crate::layout::arena_center() + vec3(24.0, 0.0, 16.0);
        let yaw = PI;
        let foot = |s: f32| {
            let p = root + side(yaw) * s * HIP_X;
            Foot { pos: p, from: p, to: p, t: 1.0 }
        };
        let tail = std::array::from_fn(|k| root + vec3(0.0, H, 0.0) - fwd(yaw) * (2.4 + k as f32 * SEG));
        Kaiju {
            root,
            pos: root + vec3(0.0, H, 0.0),
            vel: Vec3::ZERO,
            yaw,
            st: WALK,
            st_t: 0.0,
            target: arena_center(),
            goal: wander_point(),
            focus: 4,
            focus_i: 0,
            focus_p: Vec3::ZERO,
            focus_t: 3.0,
            cd: 6.0,
            shot_t: 0.0,
            smash_cd: 0.0,
            feet: [foot(-1.0), foot(1.0)],
            tail,
            last_root: root,
            root_vel: Vec3::ZERO,
            net: None,
            alive: true,
            host_alive: true,
            crashed: false,
            jaw: 0.0,
            glow: 0.0,
            roll: 0.0,
            beams: Vec::new(),
            clips: Clips {
                roar: Arc::new(roar(sr)),
                stomp: Arc::new(stomp(sr)),
                charge: Arc::new(charge(sr)),
                breath: Arc::new(breath(sr)),
                swipe: Arc::new(swipe(sr)),
            },
            sfx: Vec::new(),
        }
    }

    /// Centro do corpo pra mira/dano.
    pub fn center(&self) -> Vec3 {
        self.pos + vec3(0.0, 3.5, 0.0)
    }

    pub fn targets(&self, npcs: &Npcs) -> Option<crate::gta::Target> {
        npcs.kaiju().alive().then(|| (self.center(), 4.5, 0, npc::KAIJU))
    }

    /// Pra urna revidar: centro do GODZILHA se vivo e perto.
    pub fn lure(&self, from: Vec3, npcs: &Npcs) -> Option<Vec3> {
        (npcs.kaiju().alive() && xz(from).distance(xz(self.root)) < 70.0).then(|| self.center())
    }

    fn twist(&self) -> f32 {
        if self.st == SWIPE { -0.5 * (smooth(self.st_t / DUR[4]) * PI).sin() } else { 0.0 }
    }

    fn body(&self) -> Mat4 {
        Mat4::from_translation(self.pos) * Mat4::from_rotation_y(self.yaw + self.twist()) * Mat4::from_rotation_z(self.roll)
    }

    fn upper(&self) -> Mat4 {
        let lean = match self.st {
            ROAR => LEAN - 0.25 * (self.st_t / 0.4).min(1.0),
            CHARGE | BREATH => LEAN + 0.12,
            _ => LEAN,
        };
        self.body() * Mat4::from_rotation_x(lean)
    }

    fn head(&self, time: f32) -> Mat4 {
        let u = self.upper();
        let neck = u.transform_point3(vec3(0.0, 8.0, 2.2));
        let pitch = match self.st {
            ROAR => -0.7 * (self.st_t / 0.4).min(1.0),
            CHARGE | BREATH => {
                let d = self.target - neck;
                (-d.y).atan2(xz(d).length().max(0.1)).clamp(-0.6, 0.9) - LEAN - 0.12
            }
            _ => 0.08 * (time * 1.3).sin(),
        };
        u * Mat4::from_translation(vec3(0.0, 8.0, 2.2)) * Mat4::from_rotation_x(pitch)
    }

    fn mouth(&self, time: f32) -> Vec3 {
        self.head(time).transform_point3(vec3(0.0, 0.0, 3.6))
    }

    /// Ponto do bafo varrendo o alvo de um lado pro outro.
    fn aim(&self) -> Vec3 {
        self.target + side(self.yaw) * (self.st_t * 3.0).sin() * 3.5
    }

    fn enter(&mut self, st: u8) {
        self.st = st;
        self.st_t = 0.0;
        let head = self.pos + vec3(0.0, 8.0, 0.0);
        match st {
            ROAR => self.sfx.push((self.clips.roar.clone(), head, 2.0)),
            CHARGE => self.sfx.push((self.clips.charge.clone(), head, 1.4)),
            BREATH => self.sfx.push((self.clips.breath.clone(), head, 1.8)),
            SWIPE => self.sfx.push((self.clips.swipe.clone(), self.pos, 1.4)),
            _ => {}
        }
    }

    pub fn snapshot(&self) -> Value {
        let r = |x: f32| (x as f64 * 100.0).round() / 100.0;
        json!([r(self.root.x), r(self.root.z), r(self.yaw), self.st, r(self.st_t), r(self.target.x), r(self.target.y), r(self.target.z)])
    }

    pub fn apply(&mut self, v: &Value) {
        let Some(a) = v.as_array().filter(|a| a.len() >= 8) else { return };
        let f = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
        self.net = Some((vec3(f(0), G as f32, f(1)), f(2)));
        let st = a[3].as_u64().unwrap_or(0) as u8;
        if st != self.st {
            self.enter(st);
        }
        if (self.st_t - f(4)).abs() > 0.4 {
            self.st_t = f(4);
        }
        self.target = vec3(f(5), f(6), f(7));
    }

    fn focus_pos(&self, time: f32, urna: &Urna, npcs: &Npcs, players: &[Vec3]) -> Option<Vec3> {
        match self.focus {
            0 if npcs.urna().alive() => Some(urna.pos),
            1 => players.get(self.focus_i).map(|p| *p + vec3(0.0, 0.9, 0.0)),
            2 if npcs.voador().alive() => Some(crate::voador::pos(time)),
            3 => Some(self.focus_p),
            _ => None,
        }
    }

    /// Corpo (com root em `root`) atravessando bloco sólido? Retorna o ponto batido.
    fn blocked(&self, world: &World, root: Vec3) -> Option<Vec3> {
        let base = floor(world, root);
        let (s, f) = (side(self.yaw), fwd(self.yaw));
        for x in [-2.6, 0.0, 2.6] {
            for z in [-1.5, 2.5] {
                for y in [H - 1.0, H + 2.5, H + 6.0] {
                    let p = root + s * x + f * z + vec3(0.0, base - root.y + y, 0.0);
                    if world.solid_f(p.x, p.y, p.z) {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// IA do host: persegue a urna (ou outro alvo), anda, ataca. Devolve os tiros pra mandar.
    #[allow(clippy::too_many_arguments)]
    pub fn think(&mut self, world: &World, dt: f32, time: f32, urna: &Urna, npcs: &Npcs, players: &[Vec3], fighters: &[Fighter], villagers: &[Villager], events: &mut Vec<Ev>) -> Vec<Plan> {
        let mut out = Vec::new();
        let alive = npcs.kaiju().alive();
        if alive != self.host_alive {
            self.host_alive = alive;
            if alive {
                self.root = wander_point();
                self.goal = self.root;
                self.enter(ROAR);
                events.push(Ev::Banner("O GODZILHA EMERGIU DO LAGO PARANOA!".into()));
            } else {
                events.push(Ev::Banner("O GODZILHA CAIU! VOLTA NO PROXIMO FILME (90s)".into()));
                events.push(Ev::Shake(1.0));
            }
        }
        if !alive {
            self.st = WALK;
            return out;
        }
        self.cd -= dt;
        self.smash_cd -= dt;
        self.focus_t -= dt;
        if self.focus_t <= 0.0 {
            self.focus_t = gen_range(7.0, 12.0);
            let roll = gen_range(0.0, 1.0);
            let near = |p: Vec3| xz(p).distance(xz(self.root)) < 60.0;
            let ns: Vec<Vec3> = fighters.iter().filter(|f| f.active()).map(|f| f.pos + vec3(0.0, 0.9, 0.0)).chain(villagers.iter().enumerate().filter(|(i, _)| npcs.alive(npc::VILLAGER, *i)).map(|(_, v)| v.pos + vec3(0.0, 0.9, 0.0))).filter(|p| near(*p) && zone_ok(xz(*p))).collect();
            self.focus = if roll < 0.6 && npcs.urna().alive() {
                0
            } else if roll < 0.72 && npcs.voador().alive() && near(crate::voador::pos(time)) {
                2
            } else if roll < 0.86 && time > 40.0 && !players.is_empty() {
                self.focus_i = gen_range(0, players.len());
                1
            } else if !ns.is_empty() {
                self.focus_p = ns[gen_range(0, ns.len())];
                3
            } else {
                4
            };
        }
        let fp = self.focus_pos(time, urna, npcs, players);

        // Andar: chega a uma distância de briga do alvo; sem alvo, vagueia
        let keep = if self.focus == 0 { 15.0 } else { 13.0 };
        let mut want = match fp {
            Some(p) => {
                let d = xz(p) - xz(self.root);
                if d.length() > keep + 3.0 {
                    p - vec3(d.x, 0.0, d.y).normalize_or_zero() * keep
                } else {
                    self.root
                }
            }
            None => {
                if xz(self.goal).distance(xz(self.root)) < 1.5 {
                    self.goal = wander_point();
                }
                self.goal
            }
        };
        if !zone_ok(xz(want)) {
            if !zone_ok(xz(self.goal)) || xz(self.goal).distance(xz(self.root)) < 1.5 {
                self.goal = wander_point();
            }
            want = self.goal;
        }
        let speed = if self.st == WALK { 2.0 } else { 0.0 };
        let mut dir = (xz(want) - xz(self.root)).normalize_or_zero();
        for (c, r) in keep_out() {
            let to = xz(self.root) - c;
            let n = to.normalize_or_zero();
            if to.length() < r + 6.0 && dir.dot(n) < 0.0 {
                dir = (dir - n * dir.dot(n)).normalize_or_zero();
            }
        }
        let step = (speed * dt).min(xz(want).distance(xz(self.root)));
        let mut next = self.root + vec3(dir.x, 0.0, dir.y) * step;
        for (c, r) in keep_out() {
            let to = xz(next) - c;
            if to.length() < r {
                let p = c + to.normalize_or(Vec2::X) * r;
                next = vec3(p.x, next.y, p.y);
            }
        }
        if npcs.urna().alive() {
            let to = xz(next) - xz(urna.root);
            if to.length() < 15.0 {
                let p = xz(urna.root) + to.normalize_or(Vec2::X) * 15.0;
                next = vec3(p.x, next.y, p.y);
            }
        }
        let (lo, hi) = (crate::layout::ARENA - crate::layout::ARENA_HALF, crate::layout::ARENA + crate::layout::ARENA_HALF);
        next.x = next.x.clamp(lo.x, hi.x);
        next.z = next.z.clamp(lo.y, hi.y);
        match self.blocked(world, next).filter(|_| self.blocked(world, self.root).is_none()) {
            Some(hit) => {
                // Prédio no caminho: derruba na porrada
                if self.smash_cd <= 0.0 {
                    self.smash_cd = 1.5;
                    out.push(Plan { o: hit, hit, r: 3.2, deflect: false });
                } else {
                    self.goal = wander_point();
                }
            }
            None => self.root = next,
        }

        let look = match (self.st, fp) {
            (CHARGE | BREATH, _) => self.target,
            (_, Some(p)) if step < 0.01 => p,
            _ => self.root + vec3(dir.x, 0.0, dir.y) * 5.0,
        };
        let to = look - self.root;
        if xz(to).length() > 0.5 {
            self.yaw = crate::actors::angle_lerp(self.yaw, to.x.atan2(to.z), dt * 1.8);
        }

        // Ataques
        let mouth = self.mouth(time);
        match self.st {
            WALK if self.cd <= 0.0 => {
                let roll = gen_range(0.0, 1.0);
                match fp {
                    Some(p) => {
                        self.target = p;
                        let d = xz(p).distance(xz(self.root));
                        if d < 18.0 && roll < 0.45 {
                            self.enter(SWIPE);
                        } else if d < 75.0 && roll < 0.85 {
                            self.enter(CHARGE);
                        } else {
                            self.enter(ROAR);
                        }
                    }
                    None if roll < 0.4 => self.enter(ROAR),
                    None => {}
                }
                self.cd = gen_range(4.5, 8.0);
                if self.st == ROAR {
                    events.push(Ev::Text { pos: mouth + vec3(0.0, 3.0, 0.0), text: "GRRROOOOAAAANN!!!".into(), color: rgb(0.5, 0.85, 1.0), big: true });
                }
            }
            CHARGE => {
                if let Some(p) = fp {
                    self.target = self.target.lerp(p, (dt * 2.0).min(1.0));
                }
                if self.st_t >= DUR[2] {
                    self.enter(BREATH);
                    self.shot_t = 0.0;
                    events.push(Ev::Text { pos: mouth + vec3(0.0, 2.0, 0.0), text: "BAFO ATOMICO!".into(), color: rgb(0.4, 0.8, 1.0), big: true });
                }
            }
            BREATH => {
                if let Some(p) = fp {
                    self.target = self.target.lerp(p, (dt * 1.2).min(1.0));
                }
                self.shot_t -= dt;
                if self.shot_t <= 0.0 {
                    self.shot_t = 0.35;
                    let mut plan = urna::plan(world, mouth, self.aim(), 0.0);
                    plan.r = gen_range(2.6, 3.4);
                    out.push(plan);
                }
                if self.st_t >= DUR[3] {
                    self.enter(WALK);
                }
            }
            SWIPE => {
                if self.st_t >= 0.55 && self.st_t - dt < 0.55 {
                    let tip = self.tail[TAIL];
                    let p = fp.filter(|p| xz(*p).distance(xz(self.root)) < 19.0).unwrap_or(tip);
                    out.push(Plan { o: p, hit: p, r: 3.0, deflect: false });
                    events.push(Ev::Text { pos: p + vec3(0.0, 3.0, 0.0), text: "RABADA!".into(), color: rgb(1.0, 0.8, 0.3), big: false });
                }
                if self.st_t >= DUR[4] {
                    self.enter(WALK);
                }
            }
            ROAR if self.st_t >= DUR[1] => self.enter(WALK),
            _ => {}
        }
        out
    }

    /// Todos os clientes, ao receber um tiro `by: 9`: troca o feixe vermelho da urna por azul.
    pub fn on_shot(&mut self, plan: &Plan, fx: &mut Fx) {
        if let Some(i) = fx.beams.iter().rposition(|b| b.from == plan.o) {
            let b = fx.beams.remove(i);
            if b.from.distance(b.to) > 0.5 {
                self.beams.push((b.from, b.to, 0.3));
            } else {
                self.sfx.push((self.clips.stomp.clone(), plan.hit, 1.2));
            }
        }
    }

    /// Física procedural (todos os clientes): passos, mola do corpo, rabo, queda na morte.
    #[allow(clippy::too_many_arguments)]
    pub fn animate(&mut self, world: &World, dt: f32, time: f32, life: &Vida, eye: Vec3, in_club: bool, fx: &mut Fx) {
        let dt = dt.max(1e-4);
        if let Some((r, y)) = self.net {
            self.root = if xz(r).distance(xz(self.root)) > 20.0 { r } else { self.root.lerp(r, (dt * 6.0).min(1.0)) };
            self.yaw = crate::actors::angle_lerp(self.yaw, y, (dt * 6.0).min(1.0));
        }
        self.st_t += dt;
        for b in self.beams.iter_mut() {
            b.2 -= dt;
        }
        self.beams.retain(|b| b.2 > 0.0);
        let shake = |fx: &mut Fx, p: Vec3, a: f32, range: f32| {
            if !in_club {
                fx.shake = fx.shake.max((1.0 - p.distance(eye) / range).max(0.0) * a);
            }
        };

        let alive = life.alive();
        if alive && !self.alive {
            self.pos = self.root + vec3(0.0, H + 25.0, 0.0);
            self.vel = Vec3::ZERO;
            self.roll = 0.0;
            for (i, f) in self.feet.iter_mut().enumerate() {
                let p = self.root + side(self.yaw) * (i as f32 * 2.0 - 1.0) * HIP_X;
                *f = Foot { pos: p, from: p, to: p, t: 1.0 };
            }
            self.last_root = self.root;
        }
        if !alive && self.alive {
            self.crashed = false;
            let c = self.center();
            for _ in 0..80 {
                let k = gen_range(0.0, 1.0);
                fx.particles.push(Particle {
                    pos: c + vec3(gen_range(-2.0, 2.0), gen_range(-2.0, 3.0), gen_range(-2.0, 2.0)),
                    vel: vec3(gen_range(-1.0, 1.0), gen_range(0.2, 1.4), gen_range(-1.0, 1.0)) * gen_range(4.0, 14.0),
                    col: Color::new(1.0, 0.35 + 0.5 * k, 0.1 * k, 1.0),
                    life: gen_range(0.5, 1.4),
                    size: gen_range(0.3, 0.8),
                    gravity: false,
                });
            }
            fx.flashes.push(urna::Flash { pos: c, t: 0.0, r: 7.0 });
            self.sfx.push((self.clips.roar.clone(), c, 1.5));
            shake(fx, c, 1.2, 90.0);
        }
        self.alive = alive;

        let rv = (self.root - self.last_root) / dt;
        self.last_root = self.root;
        self.root_vel = self.root_vel.lerp(if rv.length() < 20.0 { rv } else { Vec3::ZERO }, (dt * 5.0).min(1.0));
        let sd = side(self.yaw);

        if !alive {
            // Tomba de lado com estrondo
            let t = life.t;
            let ground = floor(world, self.root);
            let target = vec3(self.root.x, ground + 2.4, self.root.z) + sd * 3.0;
            self.vel += ((target - self.pos) * 6.0 - self.vel * 3.0) * dt;
            self.pos += self.vel * dt;
            self.roll = -smooth(t / 1.3) * FRAC_PI_2;
            if t >= 1.3 && !self.crashed {
                self.crashed = true;
                let c = vec3(self.root.x, ground, self.root.z) + sd * 6.0;
                for _ in 0..70 {
                    let a = gen_range(0.0, TAU);
                    fx.particles.push(Particle {
                        pos: c + vec3(a.cos() * 3.0, 0.5, a.sin() * 3.0),
                        vel: vec3(a.cos() * gen_range(5.0, 12.0), gen_range(1.0, 5.0), a.sin() * gen_range(5.0, 12.0)),
                        col: Color::new(0.55, 0.5, 0.42, 1.0),
                        life: gen_range(0.6, 1.5),
                        size: gen_range(0.3, 0.7),
                        gravity: true,
                    });
                }
                fx.flashes.push(urna::Flash { pos: c, t: 0.0, r: 5.0 });
                self.sfx.push((self.clips.stomp.clone(), c, 2.0));
                shake(fx, c, 1.4, 90.0);
            }
            self.tail_update(world, dt, time, 0.0);
            self.jaw = (self.jaw + dt).min(0.6);
            self.glow = 0.0;
            return;
        }

        let mut lift = 0.0;
        let mut landed = Vec::new();
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let mut want = self.root + sd * s * HIP_X + self.root_vel * 0.5;
            want.y = floor(world, want);
            let other_planted = self.feet[1 - i].t >= 1.0;
            let rvel = self.root_vel;
            let f = &mut self.feet[i];
            if f.t >= 1.0 {
                let off = xz(f.pos).distance(xz(want));
                if other_planted && (off > 2.0 || (f.pos.y - want.y).abs() > 1.5) {
                    f.from = f.pos;
                    f.to = want + rvel * 0.3;
                    f.to.y = floor(world, f.to);
                    f.t = 0.0;
                }
            } else {
                f.t = (f.t + dt / 0.7).min(1.0);
                f.pos = f.from.lerp(f.to, smooth(f.t)) + Vec3::Y * (f.t * PI).sin() * 1.8;
                lift = (f.t * PI).sin();
                if f.t >= 1.0 {
                    f.pos = f.to;
                    landed.push(f.pos);
                }
            }
        }
        for p in landed {
            shake(fx, p, 0.7, 55.0);
            self.sfx.push((self.clips.stomp.clone(), p, 0.9));
            for _ in 0..10 {
                let a = gen_range(0.0, TAU);
                fx.particles.push(Particle { pos: p + vec3(0.0, 0.3, 0.0), vel: vec3(a.cos() * 4.0, gen_range(1.0, 2.5), a.sin() * 4.0), col: Color::new(0.55, 0.5, 0.42, 1.0), life: gen_range(0.4, 0.8), size: gen_range(0.2, 0.4), gravity: true });
            }
        }
        if self.st == ROAR && self.st_t > 0.3 && self.st_t < 1.9 {
            shake(fx, self.pos, 0.35, 50.0);
        }

        let ground = (self.feet[0].pos.y + self.feet[1].pos.y) * 0.5;
        let target = vec3(self.root.x, ground + H + lift * 0.4, self.root.z);
        self.vel += ((target - self.pos) * 40.0 - self.vel * 8.0) * dt;
        self.pos += self.vel * dt;
        self.roll = 0.0;

        let swing = if self.st == SWIPE { 2.4 * (smooth(self.st_t / DUR[4]) * PI).sin() } else { 0.0 };
        self.tail_update(world, dt, time, swing);
        let jaw = match self.st {
            ROAR if self.st_t > 0.2 && self.st_t < 2.0 => 1.0,
            BREATH => 0.8,
            CHARGE => 0.25 * (self.st_t / DUR[2]),
            _ => 0.0,
        };
        self.jaw += (jaw - self.jaw) * (dt * 10.0).min(1.0);
        let glow = match self.st {
            CHARGE => self.st_t / DUR[2],
            BREATH => 1.0,
            _ => 0.0,
        };
        self.glow = if glow > self.glow { glow } else { (self.glow - dt * 1.5).max(glow) };
    }

    /// Rabo procedural: segmentos pra trás descendo até o chão, balançando; `swing` chicoteia.
    fn tail_update(&mut self, world: &World, dt: f32, time: f32, swing: f32) {
        let m = self.body();
        let base = m.transform_point3(vec3(0.0, 0.3, -2.4));
        let mut prev = base;
        let k_lerp = (dt * 12.0).min(1.0);
        self.tail[0] = base;
        let dead = !self.alive;
        for k in 1..=TAIL {
            let f = k as f32 / TAIL as f32;
            let sway = if dead { 0.0 } else { (time * 1.6 - k as f32 * 0.5).sin() * 0.15 * f };
            let a = self.yaw + self.twist() + PI + swing * f.powf(0.7) + sway;
            let mut p = prev + vec3(a.sin(), 0.0, a.cos()) * SEG;
            let gy = floor(world, p) + if dead { 0.5 } else { 0.9 + 0.5 * f };
            p.y = if dead { gy } else { base.y + (gy - base.y) * (f / 0.6).min(1.0) };
            self.tail[k] = self.tail[k].lerp(p, k_lerp);
            prev = self.tail[k];
        }
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, world: &World, labels: &mut Vec<Label>, time: f32, life: &Vida) {
        if !life.alive() && life.t > 30.0 {
            return;
        }
        let skin = rgb(0.2, 0.25, 0.21);
        let dark = rgb(0.13, 0.16, 0.14);
        let belly = rgb(0.38, 0.4, 0.33);
        let bone = rgb(0.78, 0.78, 0.7);
        let claw = rgb(0.92, 0.9, 0.82);
        let blue = rgb(0.35, 0.75, 1.0);
        let flash = life.flash;
        let tint = |c: Color| if flash > 0.0 { Color::new(c.r + flash * 0.6, c.g + flash * 0.3, c.b + flash * 0.3, 1.0) } else { c };
        let (m, u, h) = (self.body(), self.upper(), self.head(time));
        let fw = fwd(self.yaw + self.twist());
        let up = Vec3::Y;

        // Pernas grossas (joelho pra frente) e pés com garras
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let hip = m.transform_point3(vec3(HIP_X * s, 0.4, 0.0));
            let ankle = self.feet[i].pos + up * 0.9;
            let (knee, end) = ik(hip, ankle, THIGH, SHIN, fw + up * 0.2);
            limb(b, hip, knee, 2.3, tint(skin));
            limb(b, knee, end, 1.8, tint(skin));
            b.cube(&Mat4::from_translation(knee), Vec3::ZERO, Vec3::splat(2.0), tint(dark));
            let foot = Mat4::from_translation(end - up * 0.5) * Mat4::from_rotation_y(self.yaw);
            b.cube(&foot, vec3(0.0, -0.1, 0.5), vec3(2.4, 1.1, 3.2), tint(skin));
            for t in [-0.8f32, 0.0, 0.8] {
                b.cube(&foot, vec3(t, -0.3, 2.3), vec3(0.45, 0.45, 0.7), claw);
            }
        }

        // Tronco em pera, barriga clara com sulcos, escamas
        b.cube(&u, vec3(0.0, 2.6, 0.2), vec3(5.4, 5.2, 4.6), tint(skin));
        b.cube(&u, vec3(0.0, 5.8, 0.6), vec3(4.6, 2.8, 4.0), tint(skin));
        b.cube(&u, vec3(0.0, 7.4, 1.5), vec3(3.0, 2.2, 3.0), tint(skin));
        b.cube(&u, vec3(0.0, 3.4, 2.55), vec3(3.6, 5.8, 0.3), tint(belly));
        for k in 0..5 {
            b.cube(&u, vec3(0.0, 1.0 + k as f32 * 1.1, 2.72), vec3(3.4, 0.12, 0.08), dark);
        }
        for k in 0..14 {
            let hx = crate::atlas::hash2(k, 3, 77) - 0.5;
            let hy = crate::atlas::hash2(k, 5, 91);
            let s = if k % 2 == 0 { -1.0 } else { 1.0 };
            b.cube(&u, vec3(s * 2.72, 0.8 + hy * 6.0, hx * 3.6), vec3(0.15, 0.6, 0.6), dark);
        }

        // Bracinhos
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let sh = u.transform_point3(vec3(2.5 * s, 5.5, 1.4));
            let local = match self.st {
                ROAR => vec3(3.4 * s, 6.0, 2.4),
                SWIPE => vec3(1.8 * s, 4.5 + (self.st_t * 9.0 + i as f32 * PI).sin(), 3.8),
                CHARGE | BREATH => vec3(2.2 * s, 4.0, 2.6),
                _ => vec3(1.9 * s, 3.9 + 0.2 * (time * 2.0 + i as f32).sin(), 3.3),
            };
            let (elbow, hand) = ik(sh, u.transform_point3(local), 1.8, 1.7, -up + fw * 0.3);
            limb(b, sh, elbow, 1.0, tint(skin));
            limb(b, elbow, hand, 0.85, tint(skin));
            b.cube(&Mat4::from_translation(hand), Vec3::ZERO, Vec3::splat(0.9), tint(dark));
            for t in [-0.3f32, 0.3] {
                b.cube(&Mat4::from_translation(hand + fw * 0.5 + vec3(t, -0.2, 0.0)), Vec3::ZERO, vec3(0.2, 0.2, 0.5), claw);
            }
        }

        // Cabeça: crânio, testa, focinho, mandíbula que abre, dentes, olhos
        b.cube(&h, vec3(0.0, 0.6, 0.6), vec3(2.6, 2.0, 3.0), tint(skin));
        b.cube(&h, vec3(0.0, 1.5, 1.5), vec3(2.8, 0.5, 1.2), tint(dark));
        b.cube(&h, vec3(0.0, 0.4, 2.7), vec3(2.0, 1.3, 1.6), tint(skin));
        for s in [-0.5f32, 0.5] {
            b.cube(&h, vec3(s, 0.9, 3.5), vec3(0.2, 0.15, 0.1), dark);
        }
        for k in 0..5 {
            let x = -0.8 + k as f32 * 0.4;
            b.cube(&h, vec3(x, -0.3, 3.1), vec3(0.18, 0.3, 0.18), claw);
        }
        let j = h * Mat4::from_translation(vec3(0.0, -0.2, 0.4)) * Mat4::from_rotation_x(self.jaw * 0.7);
        b.cube(&j, vec3(0.0, -0.35, 1.4), vec3(1.9, 0.6, 3.0), tint(skin));
        b.cube(&j, vec3(0.0, -0.05, 1.4), vec3(1.6, 0.1, 2.6), rgb(0.5, 0.15, 0.15));
        for k in 0..4 {
            b.cube(&j, vec3(-0.6 + k as f32 * 0.4, 0.1, 2.6), vec3(0.16, 0.25, 0.16), claw);
        }
        let eye_col = if life.alive() { rgb(1.0, 0.75, 0.2) } else { rgb(0.25, 0.25, 0.25) };
        for s in [-1.0f32, 1.0] {
            b.glow(&h, vec3(s * 1.31, 1.0, 1.6), vec3(0.06, 0.35, 0.55), eye_col);
            b.glow(&h, vec3(s * 1.34, 1.0, 1.65), vec3(0.04, 0.3, 0.15), dark);
        }

        // Placas dorsais (costas + rabo); acendem do rabo pra cabeça carregando o bafo
        let back: [(f32, f32); 6] = [(0.8, 1.6), (2.2, 2.2), (3.6, 2.6), (5.0, 2.4), (6.4, 1.8), (7.6, 1.2)];
        let total = (TAIL - 2 + back.len()) as f32;
        let lit = |idx: usize| ((self.glow * 1.25 - idx as f32 / total) * 6.0).clamp(0.0, 1.0);
        let plate = |b: &mut Batch, trans: &mut Batch, pm: Mat4, size: f32, idx: usize| {
            let l = lit(idx);
            let col = Color::new(bone.r + (blue.r - bone.r) * l, bone.g + (blue.g - bone.g) * l, bone.b + (blue.b - bone.b) * l, 1.0);
            for (x, k) in [(0.0f32, 1.0f32), (-0.45, 0.65), (0.45, 0.65)] {
                let c = vec3(x, size * k * 0.45, 0.0);
                let s = vec3(0.3, size * k, size * k * 0.6);
                if l > 0.0 {
                    b.glow(&pm, c, s, col);
                } else {
                    b.cube(&pm, c, s, col);
                }
            }
            if l > 0.0 {
                let pulse = 0.8 + 0.2 * (time * 25.0 + idx as f32).sin();
                trans.glow(&pm, vec3(0.0, size * 0.45, 0.0), vec3(0.9, size * 1.4, size), Color::new(0.3, 0.7, 1.0, 0.35 * l * pulse));
            }
        };
        for k in 0..TAIL - 2 {
            let (a, c) = (self.tail[k], self.tail[k + 1]);
            let d = c - a;
            let yaw = (-d.x).atan2(-d.z);
            let pm = Mat4::from_translation((a + c) * 0.5 + up * (1.0 - k as f32 * 0.07)) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_z(self.roll);
            plate(b, trans, pm, 1.4 - k as f32 * 0.13, TAIL - 3 - k);
        }
        for (i, &(y, s)) in back.iter().enumerate() {
            let pm = u * Mat4::from_translation(vec3(0.0, y, -2.3)) * Mat4::from_rotation_x(-0.35);
            plate(b, trans, pm, s, TAIL - 2 + i);
        }

        // Rabo
        for k in 0..TAIL {
            let w = 2.6 - k as f32 * 0.21;
            limb(b, self.tail[k], self.tail[k + 1], w, tint(if k % 2 == 0 { skin } else { dark }));
        }

        // Bafo atômico: feixe azul contínuo + flashes dos tiros
        let wob = 1.0 + 0.25 * (time * 50.0).sin();
        let mouth = self.mouth(time);
        if self.alive && self.st == BREATH {
            let d = (self.aim() - mouth).normalize_or_zero();
            let max = self.aim().distance(mouth) + 6.0;
            let mut len = world.raycast(mouth, d, max).map(|h| h.2).unwrap_or(max - 6.0);
            if let Some(s) = urna::ray_sphere(mouth, d, shield_center(), SHIELD_R) {
                len = len.min(s);
            }
            let end = mouth + d * len;
            urna::beam(trans, mouth, end, 2.2 * wob, Color::new(0.2, 0.5, 1.0, 0.35));
            urna::beam(trans, mouth, end, 0.7, Color::new(0.8, 0.95, 1.0, 0.95));
            trans.glow(&Mat4::from_translation(end), Vec3::ZERO, Vec3::splat(3.0 * wob), Color::new(0.4, 0.8, 1.0, 0.5));
        }
        if self.alive && self.glow > 0.0 {
            trans.glow(&Mat4::from_translation(mouth), Vec3::ZERO, Vec3::splat(0.6 + self.glow * 1.4 * wob), Color::new(0.4, 0.8, 1.0, 0.6 * self.glow));
        }
        for &(a, c, t) in &self.beams {
            let k = (t / 0.3).min(1.0);
            urna::beam(trans, a, c, 1.4 * k, Color::new(0.2, 0.5, 1.0, 0.35 * k));
            urna::beam(trans, a, c, 0.45 * k, Color::new(0.85, 0.95, 1.0, 0.9 * k));
        }

        let tag = h.transform_point3(vec3(0.0, 3.2, 0.0));
        if !life.alive() {
            if life.t < 10.0 {
                labels.push(Label { pos: self.center() + up * 4.0, text: "\"VOLTO NO PROXIMO FILME...\"".into(), size: 26.0, color: rgb(0.5, 0.85, 1.0) });
            }
            return;
        }
        labels.push(Label { pos: tag, text: "GODZILHA".into(), size: 32.0, color: rgb(0.45, 0.85, 1.0) });
        if self.st == ROAR {
            labels.push(Label { pos: tag + up * 2.0, text: "\"GRRROOOOAAAANN!!!\"".into(), size: 26.0, color: rgb(0.6, 0.9, 1.0) });
        } else if time.rem_euclid(11.0) < 5.0 {
            let fala = FALAS[(time / 11.0) as usize % FALAS.len()];
            labels.push(Label { pos: tag + up * 2.0, text: format!("\"{fala}\""), size: 24.0, color: rgb(0.6, 1.0, 0.6) });
        }
    }
}

// ------------------------------------------------ Sons sintetizados

struct Rng(u32);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn saw(x: f32) -> f32 {
    2.0 * (x - (x + 0.5).floor())
}

fn render(sr: u32, secs: f32, mut f: impl FnMut(f32, f32) -> f32) -> Vec<f32> {
    let mut rng = Rng(0x6D2B_79F5);
    (0..(secs * sr as f32) as usize).map(|i| f(i as f32 / sr as f32, rng.f())).collect()
}

/// Rugido: guincho rasgado que sobe e desce, com corpo grave e ruído.
fn roar(sr: u32) -> Vec<f32> {
    let (mut ph, mut ph2, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let dt = 1.0 / sr as f32;
    render(sr, 2.4, move |t, n| {
        let env = (t / 0.15).min(1.0) * (1.0 - smooth((t - 1.5) / 0.9));
        let f = 160.0 + 280.0 * smooth(t / 0.5) - 120.0 * (t / 2.4) + (t * 33.0).sin() * 18.0;
        ph += f * dt;
        ph2 += f * 0.25 * dt;
        lp += (n - lp) * 0.25;
        let tone = saw(ph) * 0.6 + saw(ph * 1.51) * 0.3 + (TAU * ph2).sin() * 0.6;
        ((tone + lp * 1.3) * 2.5).tanh() * env * 0.8
    })
}

fn stomp(sr: u32) -> Vec<f32> {
    let mut lp = 0.0f32;
    render(sr, 0.9, move |t, n| {
        lp += (n - lp) * 0.06;
        let thump = (TAU * (28.0 + 55.0 * (-t / 0.05).exp()) * t).sin() * (-t / 0.35).exp();
        ((thump * 1.6 + lp * 2.5 * (-t / 0.12).exp()) * 1.4).tanh() * 0.95
    })
}

fn charge(sr: u32) -> Vec<f32> {
    let mut ph = 0.0f32;
    let dt = 1.0 / sr as f32;
    render(sr, 2.0, move |t, n| {
        let k = t / 2.0;
        ph += (70.0 + 420.0 * k * k) * dt;
        let crackle = if n > 0.97 - 0.1 * k { n } else { 0.0 };
        ((TAU * ph).sin() * 0.5 + saw(ph * 2.0) * 0.2 + crackle * 0.6) * (0.2 + 0.8 * k) * 0.7
    })
}

fn breath(sr: u32) -> Vec<f32> {
    let (mut lp, mut ph) = (0.0f32, 0.0f32);
    let dt = 1.0 / sr as f32;
    render(sr, 1.7, move |t, n| {
        lp += (n - lp) * 0.35;
        ph += (520.0 + 60.0 * (t * 9.0).sin()) * dt;
        let trem = 0.75 + 0.25 * (TAU * 23.0 * t).sin();
        let env = (t / 0.05).min(1.0) * (1.0 - smooth((t - 1.3) / 0.4));
        ((lp * 0.9 + saw(ph) * 0.35 + (TAU * ph * 0.5).sin() * 0.3) * trem * env * 1.6).tanh() * 0.8
    })
}

fn swipe(sr: u32) -> Vec<f32> {
    let (mut a, mut b) = (0.0f32, 0.0f32);
    render(sr, 0.7, move |t, n| {
        a += (n - a) * 0.3;
        b += (a - b) * 0.08;
        (a - b) * (t / 0.7 * PI).sin().powi(2) * 1.6
    })
}
