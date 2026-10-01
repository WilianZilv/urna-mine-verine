//! Villagers (curtindo o house / se rendendo) e lutadores (IA de combate).

use crate::layout;
use crate::models::*;
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{FRAC_PI_2, PI};

#[derive(Clone)]
pub enum Ev {
    Hit { pos: Vec3, claws: bool },
    Snikt(Vec3),
    Text { pos: Vec3, text: String, color: Color, big: bool },
    Shake(f32),
    Banner(String),
}

fn text(ev: &mut Vec<Ev>, pos: Vec3, s: &str, color: Color, big: bool) {
    ev.push(Ev::Text { pos, text: s.to_string(), color, big });
}

pub fn angle_lerp(a: f32, b: f32, k: f32) -> f32 {
    let mut d = (b - a) % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    }
    if d < -PI {
        d += 2.0 * PI;
    }
    a + d * k.clamp(0.0, 1.0)
}

fn fwd(yaw: f32) -> Vec3 {
    vec3(yaw.sin(), 0.0, yaw.cos())
}

/// Move no plano com step-up de 1 bloco. Retorna false se bateu em parede.
pub fn try_move(world: &World, pos: &mut Vec3, d: Vec3) -> bool {
    let nx = (pos.x + d.x).clamp(1.0, WX as f32 - 1.0);
    let nz = (pos.z + d.z).clamp(1.0, WZ as f32 - 1.0);
    let ny = world.floor_at(nx, pos.y + 1.05, nz);
    if ny - pos.y > 1.05 || world.solid_f(nx, ny + 0.1, nz) || world.solid_f(nx, ny + 1.1, nz) {
        return false;
    }
    pos.x = nx;
    pos.z = nz;
    if ny > pos.y {
        pos.y = ny;
    }
    true
}

// ---------------------------------------------------------------- Villagers

pub const FLAG_TEXTS: [&str; 6] = ["EU ME RENDO", "QUE SE FODA", "SO CURTINDO", "TO NEM AI", "PAZ E HOUSE", "EU ME RENDO"];

#[derive(Clone, Copy, PartialEq)]
pub enum VKind {
    Dancer,
    Wanderer,
    Dj,
}

pub struct Villager {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub kind: VKind,
    pub look: VLook,
    pub flag: usize,
    pub flag_col: Color,
    pub phase: f32,
    pub arms_up: bool,
    pub home: Vec3,
    pub target: Vec3,
    pub timer: f32,
    pub airborne: bool,
    pub spin: f32,
    pub walk: f32,
}

fn villager_look(i: usize) -> VLook {
    let robes = [rgb(0.45, 0.3, 0.18), rgb(0.45, 0.2, 0.55), rgb(0.88, 0.88, 0.82), rgb(0.25, 0.45, 0.2), rgb(0.2, 0.3, 0.6), rgb(0.6, 0.15, 0.15)];
    let hat = if i % 4 == 1 { Some(rgb(0.85, 0.75, 0.35)) } else { None };
    VLook { robe: robes[i % robes.len()], skin: rgb(0.76, 0.56, 0.42), hat }
}

fn party_color(i: usize) -> Color {
    [rgb(1.0, 0.3, 0.75), rgb(0.2, 0.9, 1.0), rgb(1.0, 0.9, 0.2), rgb(0.5, 1.0, 0.4), rgb(1.0, 0.55, 0.15)][i % 5]
}

pub fn spawn_villagers() -> Vec<Villager> {
    let mut vs = Vec::new();
    let g = G as f32;
    let mk = |pos: Vec3, kind: VKind, i: usize, flag: usize| {
        let flag_col = if FLAG_TEXTS[flag] == "EU ME RENDO" { WHITE } else { party_color(i) };
        Villager {
            pos,
            vel: Vec3::ZERO,
            yaw: -FRAC_PI_2,
            kind,
            look: villager_look(i),
            flag,
            flag_col,
            phase: gen_range(0.0, 6.28),
            arms_up: gen_range(0.0, 1.0) < 0.7,
            home: pos,
            target: pos,
            timer: 0.0,
            airborne: false,
            spin: 0.0,
            walk: 0.0,
        }
    };
    let mut dj = mk(layout::club(vec3(12.3, g + 1.0, 64.5)), VKind::Dj, 2, 2);
    dj.yaw = FRAC_PI_2;
    dj.arms_up = false;
    vs.push(dj);
    for i in 0..28 {
        let pos = vec3(gen_range(FLOOR_X0 as f32 + 0.8, FLOOR_X1 as f32 - 0.8), g, gen_range(FLOOR_Z0 as f32 + 0.8, FLOOR_Z1 as f32 - 0.8));
        vs.push(mk(pos, VKind::Dancer, i, i % FLAG_TEXTS.len()));
    }
    for i in 0..12 {
        let a: f32 = gen_range(0.0, 6.28);
        let r = gen_range(16.0, 32.0);
        let pos = layout::plaza_center() + vec3(a.cos() * r, 0.0, a.sin() * r);
        let flag = if i % 3 == 0 { 1 } else { 0 };
        vs.push(mk(pos, VKind::Wanderer, i + 3, flag));
    }
    vs
}

/// `dead(i)`: villager morto só cai com a física e fica onde parou.
pub fn update_villagers(vs: &mut [Villager], world: &World, dt: f32, time: f32, dead: impl Fn(usize) -> bool) {
    let dj = layout::club(vec3(12.3, G as f32, 64.5));
    for (i, v) in vs.iter_mut().enumerate() {
        let dead = dead(i);
        if v.airborne {
            v.vel.y -= 25.0 * dt;
            v.pos += v.vel * dt;
            v.pos.x = v.pos.x.clamp(1.0, WX as f32 - 1.0);
            v.pos.z = v.pos.z.clamp(1.0, WZ as f32 - 1.0);
            v.spin += dt * 9.0;
            let gnd = world.floor_at(v.pos.x, v.pos.y + 0.5, v.pos.z);
            if v.pos.y <= gnd && v.vel.y < 0.0 {
                v.pos.y = gnd;
                v.airborne = false;
                v.vel = Vec3::ZERO;
                v.spin = 0.0;
                if v.kind != VKind::Wanderer && !dead {
                    v.pos = v.home;
                }
            }
            continue;
        }
        if dead {
            continue;
        }
        match v.kind {
            VKind::Dj => {}
            VKind::Dancer => {
                v.timer -= dt;
                if v.timer < 0.0 {
                    v.target = v.home + vec3(gen_range(-1.5, 1.5), 0.0, gen_range(-1.5, 1.5));
                    v.timer = gen_range(2.0, 5.0);
                }
                let to = v.target - v.pos;
                let to = vec3(to.x, 0.0, to.z);
                if to.length() > 0.2 {
                    try_move(world, &mut v.pos, to.normalize() * 0.6 * dt);
                }
                let d = dj - v.pos;
                let want = d.x.atan2(d.z) + (time * 0.5 + v.phase).sin() * 0.7;
                v.yaw = angle_lerp(v.yaw, want, dt * 2.0);
                v.pos.y = world.floor_at(v.pos.x, v.pos.y + 1.05, v.pos.z);
            }
            VKind::Wanderer => {
                v.timer -= dt;
                let to = v.target - v.pos;
                let to = vec3(to.x, 0.0, to.z);
                if to.length() < 0.6 || v.timer < 0.0 {
                    let a: f32 = gen_range(0.0, 6.28);
                    let r = gen_range(15.0, 70.0);
                    v.target = layout::plaza_center() + vec3(a.cos() * r, 0.0, a.sin() * r);
                    v.timer = 10.0;
                } else {
                    let dir = to.normalize();
                    if !try_move(world, &mut v.pos, dir * 1.4 * dt) {
                        v.timer = 0.0;
                    }
                    v.walk += dt * 6.0;
                    v.yaw = angle_lerp(v.yaw, dir.x.atan2(dir.z), dt * 6.0);
                }
                let gnd = world.floor_at(v.pos.x, v.pos.y + 0.05, v.pos.z);
                if gnd < v.pos.y {
                    v.pos.y = (v.pos.y - 8.0 * dt).max(gnd);
                }
            }
        }
    }
}

pub fn blast_villagers(vs: &mut [Villager], c: Vec3, r: f32) {
    for v in vs.iter_mut() {
        let d = (v.pos + vec3(0.0, 1.0, 0.0)).distance(c);
        if d < r * 1.6 {
            let k = 1.0 - d / (r * 1.6);
            let mut dir = v.pos - c;
            dir.y = 0.0;
            let dir = dir.normalize_or(vec3(1.0, 0.0, 0.0));
            v.vel = dir * 14.0 * k + vec3(0.0, 9.0 * k + 4.0, 0.0);
            v.airborne = true;
        }
    }
}

// ---------------------------------------------------------------- VIPs do clube

pub struct Guest {
    pub name: &'static str,
    pub look: Look,
    pub pos: Vec3,
    pub yaw: f32,
    pub phase: f32,
}

/// Pose de quem morreu: braços abertos, estirado (o `lean` vem do `root`).
pub fn dead_pose() -> Pose {
    Pose { arm_l: -0.3, arm_r: -0.3, arm_l_out: 1.4, arm_r_out: 1.3, ..Default::default() }
}

pub fn spawn_guests() -> Vec<Guest> {
    let skin = rgb(0.86, 0.66, 0.52);
    let black = rgb(0.06, 0.06, 0.07);
    let toga = |hair: Color, beard: Option<Color>, glasses: bool| Look {
        skin,
        hair,
        shirt: black,
        pants: black,
        shoes: black,
        beard,
        glasses,
        wolverine: false,
        toga: true,
    };
    let gray = rgb(0.7, 0.7, 0.7);
    let white = rgb(0.9, 0.9, 0.9);
    let dark = rgb(0.15, 0.12, 0.1);
    let stf = [
        ("FACHIN", toga(gray, Some(gray), true)),
        ("GILMAR", toga(white, None, true)),
        ("CARMEN LUCIA", toga(rgb(0.8, 0.75, 0.6), None, true)),
        ("TOFFOLI", toga(rgb(0.35, 0.33, 0.32), None, false)),
        ("FUX", toga(rgb(0.3, 0.25, 0.22), None, false)),
        ("MORAES", toga(skin, None, false)),
        ("NUNES MARQUES", toga(dark, None, false)),
        ("MENDONCA", toga(dark, None, true)),
        ("ZANIN", toga(dark, None, false)),
        ("DINO", toga(rgb(0.4, 0.38, 0.36), None, false)),
    ];
    let g = G as f32;
    let mut out: Vec<Guest> = stf
        .into_iter()
        .enumerate()
        .map(|(i, (name, look))| Guest { name, look, pos: layout::club(vec3(17.2, g, 57.5 + i as f32 * 1.6)), yaw: FRAC_PI_2, phase: i as f32 * 1.7 })
        .collect();
    let casual = |shirt: Color, pants: Color, hair: Color| Look { skin, hair, shirt, pants, shoes: black, beard: None, glasses: false, wolverine: false, toga: false };
    out.push(Guest { name: "VORCARO", look: casual(rgb(0.1, 0.13, 0.3), black, dark), pos: layout::club(vec3(11.6, g + 1.0, 61.3)), yaw: FRAC_PI_2, phase: 0.4 });
    out.push(Guest { name: "LULINHA", look: casual(rgb(0.55, 0.75, 0.95), rgb(0.85, 0.8, 0.7), dark), pos: layout::club(vec3(11.6, g + 1.0, 67.7)), yaw: FRAC_PI_2, phase: 2.2 });
    out
}

pub fn guest_pose(g: &Guest, beat: f32) -> Pose {
    let s = (beat * PI + g.phase).sin();
    let mut p = Pose { bounce: (beat * PI).sin().abs() * 0.15, nod: (beat * 2.0 * PI).sin() * 0.2, arm_l_out: 0.3, arm_r_out: 0.3, ..Default::default() };
    if (g.phase as i32) % 2 == 0 {
        p.arm_l = -2.6 + s * 0.4;
        p.arm_r = -2.6 - s * 0.4;
    } else {
        p.arm_r = -2.8 + (beat * 2.0 * PI).sin().abs() * 0.6;
        p.arm_l = -0.5;
        p.arm_r_out = 0.1;
    }
    p
}

// ---------------------------------------------------------------- Lutadores

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Atk {
    Jab,
    Cross,
    Combo3,
    Finisher,
    Special,
}

impl Atk {
    fn dur(self) -> f32 {
        match self {
            Atk::Jab => 0.30,
            Atk::Cross => 0.34,
            Atk::Combo3 => 0.38,
            Atk::Finisher => 0.55,
            Atk::Special => 0.8,
        }
    }
    fn hit(self) -> f32 {
        match self {
            Atk::Jab => 0.13,
            Atk::Cross => 0.15,
            Atk::Combo3 => 0.17,
            Atk::Finisher => 0.30,
            Atk::Special => 0.12,
        }
    }
    fn mult(self) -> f32 {
        match self {
            Atk::Jab => 1.0,
            Atk::Cross => 1.1,
            Atk::Combo3 => 1.25,
            Atk::Finisher => 1.9,
            Atk::Special => 2.2,
        }
    }
    fn knock(self) -> (f32, f32) {
        match self {
            Atk::Jab => (2.5, 1.0),
            Atk::Cross => (3.0, 1.5),
            Atk::Combo3 => (3.5, 2.0),
            Atk::Finisher => (9.0, 6.0),
            Atk::Special => (11.0, 7.0),
        }
    }
    fn stun(self) -> f32 {
        match self {
            Atk::Jab => 0.18,
            Atk::Cross => 0.22,
            Atk::Combo3 => 0.26,
            Atk::Finisher => 0.55,
            Atk::Special => 0.6,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum FState {
    Idle,
    Attack { kind: Atk, t: f32, done: bool },
    Stun(f32),
    Dodge(f32, Vec3),
    Ko(f32),
    Entering,
}

pub struct Fighter {
    pub name: &'static str,
    pub look: Look,
    pub flag: (Color, Option<Color>),
    pub flag_text: &'static str,
    pub wolverine: bool,
    pub max_hp: f32,
    pub hp: f32,
    pub speed: f32,
    pub dmg: f32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub state: FState,
    pub combo: u32,
    pub combo_t: f32,
    pub atk_cd: f32,
    pub special_cd: f32,
    pub target: Option<usize>,
    pub retarget: f32,
    pub walk: f32,
    pub walk_amt: f32,
    pub kos: u32,
    pub flash: f32,
    pub last_hit: f32,
    pub berserk: f32,
    pub berserk_cd: f32,
    pub spawned: bool,
    pub home: Vec3,
    pub ko_t: f32,
    pub airborne: bool,
}

impl Fighter {
    #[allow(clippy::too_many_arguments)]
    fn new(name: &'static str, look: Look, flag: (Color, Option<Color>), flag_text: &'static str, home: Vec3, wolverine: bool) -> Self {
        let (hp, speed, dmg) = if wolverine { (160.0, 5.2, 8.0) } else { (100.0, 3.4, 9.0) };
        Fighter {
            name,
            look,
            flag,
            flag_text,
            wolverine,
            max_hp: hp,
            hp,
            speed,
            dmg,
            pos: home,
            vel: Vec3::ZERO,
            yaw: 0.0,
            state: FState::Idle,
            combo: 0,
            combo_t: 0.0,
            atk_cd: gen_range(0.5, 1.5),
            special_cd: gen_range(4.0, 8.0),
            target: None,
            retarget: 0.0,
            walk: 0.0,
            walk_amt: 0.0,
            kos: 0,
            flash: 0.0,
            last_hit: -10.0,
            berserk: 0.0,
            berserk_cd: 0.0,
            spawned: !wolverine,
            home,
            ko_t: 0.0,
            airborne: false,
        }
    }

    pub fn active(&self) -> bool {
        self.spawned && !matches!(self.state, FState::Ko(_) | FState::Entering)
    }

    fn max_combo(&self) -> u32 {
        if self.wolverine { 4 } else { 3 }
    }

    fn combo_kind(&self) -> Atk {
        match (self.combo, self.wolverine) {
            (0, _) => Atk::Jab,
            (1, _) => Atk::Cross,
            (2, true) => Atk::Combo3,
            _ => Atk::Finisher,
        }
    }

    pub fn spawn_wolverine(&mut self) {
        self.spawned = true;
        self.hp = self.max_hp;
        self.pos = arena_center() + vec3(0.0, 28.0, 0.0);
        self.vel = Vec3::ZERO;
        self.state = FState::Entering;
    }

    fn respawn(&mut self) {
        self.hp = self.max_hp;
        self.pos = self.home + vec3(0.0, 6.0, 0.0);
        self.vel = Vec3::ZERO;
        self.state = FState::Idle;
        self.combo = 0;
        self.berserk = 0.0;
    }
}

pub fn spawn_fighters() -> Vec<Fighter> {
    let skin = rgb(0.86, 0.66, 0.52);
    let c = arena_center();
    vec![
        Fighter::new(
            "LULA",
            Look { skin, hair: rgb(0.85, 0.85, 0.85), shirt: rgb(0.8, 0.1, 0.1), pants: rgb(0.2, 0.2, 0.25), shoes: rgb(0.1, 0.1, 0.1), beard: Some(rgb(0.8, 0.8, 0.8)), glasses: false, wolverine: false, toga: false },
            (rgb(0.85, 0.1, 0.1), Some(WHITE)),
            "LULA",
            c + vec3(-5.0, 0.0, -4.0),
            false,
        ),
        Fighter::new(
            "FLAVIO BOLSONARO",
            Look { skin, hair: rgb(0.25, 0.17, 0.1), shirt: rgb(0.1, 0.55, 0.25), pants: rgb(0.15, 0.2, 0.45), shoes: rgb(0.3, 0.2, 0.1), beard: None, glasses: false, wolverine: false, toga: false },
            (rgb(0.1, 0.6, 0.25), Some(rgb(1.0, 0.85, 0.1))),
            "FLAVIO",
            c + vec3(5.0, 0.0, -4.0),
            false,
        ),
        Fighter::new(
            "RENAN SANTOS",
            Look { skin, hair: rgb(0.1, 0.08, 0.06), shirt: rgb(0.12, 0.12, 0.14), pants: rgb(0.25, 0.3, 0.45), shoes: rgb(0.1, 0.1, 0.1), beard: None, glasses: true, wolverine: false, toga: false },
            (rgb(0.1, 0.2, 0.6), Some(rgb(1.0, 0.55, 0.1))),
            "RENAN",
            c + vec3(0.0, 0.0, 5.0),
            false,
        ),
        Fighter::new(
            "WOLVERINE",
            Look { skin, hair: rgb(0.12, 0.08, 0.05), shirt: rgb(0.98, 0.82, 0.1), pants: rgb(0.98, 0.82, 0.1), shoes: rgb(0.12, 0.25, 0.75), beard: None, glasses: false, wolverine: true, toga: false },
            (rgb(0.98, 0.82, 0.1), Some(rgb(0.12, 0.25, 0.75))),
            "SNIKT",
            c + vec3(0.0, 0.0, -6.0),
            true,
        ),
    ]
}

fn physics(f: &mut Fighter, world: &World, dt: f32) -> bool {
    let hv = vec3(f.vel.x, 0.0, f.vel.z);
    if hv.length_squared() > 1e-4 && !try_move(world, &mut f.pos, hv * dt) {
        f.vel.x *= -0.3;
        f.vel.z *= -0.3;
    }
    let ground = world.floor_at(f.pos.x, f.pos.y + 0.05, f.pos.z);
    if f.pos.y > ground + 0.01 || f.vel.y > 0.0 {
        f.vel.y -= 24.0 * dt;
        f.pos.y += f.vel.y * dt;
        if f.pos.y <= ground {
            f.pos.y = ground;
            f.vel.y = 0.0;
            return true;
        }
        false
    } else {
        f.pos.y = ground;
        let damp = (1.0 - 8.0 * dt).max(0.0);
        f.vel.x *= damp;
        f.vel.z *= damp;
        true
    }
}

struct Hit {
    target: usize,
    from: usize,
    dmg: f32,
    knock: Vec3,
    kind: Atk,
    claws: bool,
}

pub fn update_fighters(fs: &mut [Fighter], world: &World, dt: f32, time: f32, ev: &mut Vec<Ev>) {
    let n = fs.len();
    let snap: Vec<(Vec3, bool)> = fs.iter().map(|f| (f.pos, f.active())).collect();
    let mut hits: Vec<Hit> = Vec::new();
    let mut threats: Vec<(usize, usize)> = Vec::new();
    let up = vec3(0.0, 1.0, 0.0);

    for i in 0..n {
        let f = &mut fs[i];
        if !f.spawned {
            continue;
        }
        f.flash = (f.flash - dt * 4.0).max(0.0);
        f.atk_cd -= dt;
        f.special_cd -= dt;
        f.combo_t -= dt;
        f.berserk = (f.berserk - dt).max(0.0);
        f.berserk_cd -= dt;
        if f.combo_t <= 0.0 && !matches!(f.state, FState::Attack { .. }) {
            f.combo = 0;
        }

        // Fator de cura + berserker do Wolverine
        if f.wolverine && f.active() {
            let regen = 3.0 + if time - f.last_hit > 2.5 { 10.0 } else { 0.0 };
            f.hp = (f.hp + regen * dt).min(f.max_hp);
            if f.hp < f.max_hp * 0.35 && f.berserk_cd <= 0.0 {
                f.berserk = 6.0;
                f.berserk_cd = 18.0;
                text(ev, f.pos + up * 2.6, "BERSERKER!!!", RED, true);
                ev.push(Ev::Snikt(f.pos));
                ev.push(Ev::Shake(0.3));
            }
        }
        let bz = if f.berserk > 0.0 { 1.5 } else { 1.0 };
        let aspd = if f.wolverine { 1.35 * bz } else { 1.0 };

        let grounded = physics(f, world, dt);
        let landed = grounded && f.airborne;
        f.airborne = !grounded;
        if f.pos.y < 1.5 {
            f.respawn();
        }

        let fw = fwd(f.yaw);
        match f.state {
            FState::Entering => {
                if grounded {
                    f.state = FState::Idle;
                    ev.push(Ev::Snikt(f.pos));
                    ev.push(Ev::Shake(0.8));
                    ev.push(Ev::Banner("WOLVERINE ENTROU NA BRIGA! SNIKT!".into()));
                    text(ev, f.pos + up * 2.6, "SNIKT!", rgb(1.0, 0.9, 0.2), true);
                    for j in 0..n {
                        if j != i && snap[j].1 && snap[j].0.distance(f.pos) < 5.0 {
                            let dir = (snap[j].0 - f.pos).normalize_or(vec3(1.0, 0.0, 0.0));
                            hits.push(Hit { target: j, from: i, dmg: 10.0, knock: vec3(dir.x, 0.0, dir.z) * 10.0 + up * 6.0, kind: Atk::Finisher, claws: true });
                        }
                    }
                }
            }
            FState::Ko(t) => {
                f.ko_t += dt;
                let t = t - dt;
                if t <= 0.0 {
                    f.respawn();
                    text(ev, f.pos + up * 2.0, "VOLTOU!", WHITE, false);
                } else {
                    f.state = FState::Ko(t);
                }
            }
            FState::Stun(t) => {
                f.walk_amt *= 0.9;
                f.state = if t - dt <= 0.0 { FState::Idle } else { FState::Stun(t - dt) };
            }
            FState::Dodge(t, dir) => {
                try_move(world, &mut f.pos, dir * dt);
                f.state = if t - dt <= 0.0 { FState::Idle } else { FState::Dodge(t - dt, dir) };
            }
            FState::Attack { kind, t, done } => {
                let t = t + dt * aspd;
                let mut done = done;
                if let Some(j) = f.target {
                    let to = snap[j].0 - f.pos;
                    f.yaw = angle_lerp(f.yaw, to.x.atan2(to.z), dt * 12.0);
                    let dist = vec3(to.x, 0.0, to.z).length();
                    let dmg = f.dmg * kind.mult() * if f.berserk > 0.0 { 1.4 } else { 1.0 };
                    let (kh, kv) = kind.knock();
                    let dir = vec3(to.x, 0.0, to.z).normalize_or(fw);
                    if kind == Atk::Special {
                        if !done && t > kind.hit() && dist < 1.9 && snap[j].1 {
                            done = true;
                            hits.push(Hit { target: j, from: i, dmg, knock: dir * kh + up * kv, kind, claws: f.wolverine });
                        }
                    } else if !done && t >= kind.hit() {
                        done = true;
                        let facing = fw.dot(dir) > 0.3;
                        if dist <= 2.1 && facing && snap[j].1 {
                            hits.push(Hit { target: j, from: i, dmg, knock: dir * kh + up * kv, kind, claws: f.wolverine });
                        }
                    }
                }
                // Mergulho do Wolverine: dano em área ao aterrissar
                if kind == Atk::Special && f.wolverine && landed && t > 0.1 {
                    ev.push(Ev::Shake(0.35));
                    text(ev, f.pos + up * 2.2, "SNIKT!!", rgb(1.0, 0.9, 0.2), true);
                    for j in 0..n {
                        if j != i && snap[j].1 && snap[j].0.distance(f.pos) < 2.8 {
                            let dir = (snap[j].0 - f.pos).normalize_or(fw);
                            hits.push(Hit { target: j, from: i, dmg: 10.0, knock: vec3(dir.x, 0.0, dir.z) * 7.0 + up * 5.0, kind: Atk::Combo3, claws: true });
                        }
                    }
                }
                if t >= kind.dur() && (kind != Atk::Special || grounded) {
                    f.state = FState::Idle;
                    let last = kind == Atk::Special || f.combo + 1 >= f.max_combo();
                    f.combo = if last { 0 } else { f.combo + 1 };
                    f.combo_t = 1.2;
                    f.atk_cd = if last { gen_range(0.6, 1.0) / bz } else { 0.06 };
                } else {
                    f.state = FState::Attack { kind, t, done };
                }
            }
            FState::Idle => {
                f.retarget -= dt;
                let valid = f.target.is_some_and(|j| snap[j].1);
                if f.retarget <= 0.0 || !valid {
                    f.retarget = gen_range(2.0, 4.5);
                    let mut best = None;
                    let mut bd = f32::MAX;
                    for (j, s) in snap.iter().enumerate() {
                        if j == i || !s.1 {
                            continue;
                        }
                        let d = s.0.distance(f.pos) * gen_range(0.6, 1.5);
                        if d < bd {
                            bd = d;
                            best = Some(j);
                        }
                    }
                    f.target = best;
                }
                let speed = f.speed * bz;
                if let Some(j) = f.target {
                    let to = snap[j].0 - f.pos;
                    let flat = vec3(to.x, 0.0, to.z);
                    let dist = flat.length();
                    let dir = flat.normalize_or(fw);
                    let want = dir.x.atan2(dir.z);
                    f.yaw = angle_lerp(f.yaw, want, dt * 10.0);
                    let facing = fw.dot(dir) > 0.8;

                    if f.special_cd <= 0.0 && grounded && dist > 3.0 && dist < 8.0 && facing {
                        f.special_cd = if f.wolverine { gen_range(4.0, 6.0) } else { gen_range(6.0, 10.0) };
                        f.vel = dir * if f.wolverine { 14.0 } else { 11.0 } + up * if f.wolverine { 7.5 } else { 6.0 };
                        f.state = FState::Attack { kind: Atk::Special, t: 0.0, done: false };
                        text(ev, f.pos + up * 2.4, if f.wolverine { "BOTE!" } else { "VOADORA!" }, rgb(1.0, 0.8, 0.3), false);
                        threats.push((j, i));
                    } else if dist > 1.5 {
                        let mut step = dir * speed;
                        for (k, s) in snap.iter().enumerate() {
                            if k != i && s.1 {
                                let away = f.pos - s.0;
                                let d = away.length();
                                if d < 1.0 && d > 1e-3 {
                                    step += away / d * 2.0;
                                }
                            }
                        }
                        try_move(world, &mut f.pos, step * dt);
                        f.walk += dt * speed * 2.8;
                        f.walk_amt = (f.walk_amt + dt * 5.0).min(1.0);
                    } else if f.atk_cd <= 0.0 && fw.dot(dir) > 0.5 {
                        f.state = FState::Attack { kind: f.combo_kind(), t: 0.0, done: false };
                        threats.push((j, i));
                    } else {
                        let perp = vec3(dir.z, 0.0, -dir.x) * (time * 1.3 + i as f32 * 2.0).sin();
                        try_move(world, &mut f.pos, (perp * 1.2 - dir * 0.3) * dt);
                        f.walk += dt * 4.0;
                        f.walk_amt = (f.walk_amt - dt * 3.0).max(0.3);
                    }
                } else {
                    let to = f.home - f.pos;
                    if vec3(to.x, 0.0, to.z).length() > 1.0 {
                        try_move(world, &mut f.pos, vec3(to.x, 0.0, to.z).normalize() * speed * 0.5 * dt);
                    }
                    f.walk_amt = (f.walk_amt - dt * 3.0).max(0.0);
                }
            }
        }
    }

    // Esquivas (o alvo percebe o golpe vindo)
    for (j, a) in threats {
        let (apos, t) = (fs[a].pos, &mut fs[j]);
        if t.state == FState::Idle && t.active() {
            let chance = if t.wolverine { 0.4 } else { 0.12 };
            if gen_range(0.0, 1.0) < chance {
                let away = vec3(t.pos.x - apos.x, 0.0, t.pos.z - apos.z).normalize_or(vec3(1.0, 0.0, 0.0));
                let side = vec3(away.z, 0.0, -away.x) * if gen_range(0.0, 1.0) < 0.5 { 1.0 } else { -1.0 };
                t.state = FState::Dodge(0.28, side * 7.0 + away * 2.5);
                text(ev, t.pos + up * 2.2, "ESQUIVA", rgb(0.6, 0.9, 1.0), false);
            }
        }
    }

    // Aplicação dos golpes
    for h in hits {
        let apos = fs[h.from].pos;
        let mut ko = false;
        {
            let t = &mut fs[h.target];
            if !t.active() {
                continue;
            }
            if let FState::Dodge(..) = t.state {
                continue;
            }
            let mut dmg = h.dmg;
            let mut knock = h.knock;
            let to_att = vec3(apos.x - t.pos.x, 0.0, apos.z - t.pos.z).normalize_or(fwd(t.yaw));
            let block_chance = if t.wolverine { 0.25 } else { 0.15 };
            let blocked = t.state == FState::Idle && fwd(t.yaw).dot(to_att) > 0.5 && gen_range(0.0, 1.0) < block_chance;
            if blocked {
                dmg *= 0.25;
                knock *= 0.3;
                text(ev, t.pos + up * 2.2, "DEFENDEU", LIGHTGRAY, false);
            }
            t.hp -= dmg;
            t.vel += knock;
            t.flash = 1.0;
            t.last_hit = time;
            ev.push(Ev::Hit { pos: t.pos + up, claws: h.claws });
            let col = if h.claws { rgb(1.0, 0.9, 0.3) } else { WHITE };
            text(ev, t.pos + up * 2.0 + vec3(gen_range(-0.4, 0.4), 0.0, gen_range(-0.4, 0.4)), &format!("-{}", dmg.round() as i32), col, false);
            if t.hp <= 0.0 {
                t.hp = 0.0;
                t.state = FState::Ko(if t.wolverine { 3.5 } else { 5.0 });
                t.ko_t = 0.0;
                ko = true;
                text(ev, t.pos + up * 2.8, "K.O.!", rgb(1.0, 0.3, 0.2), true);
                ev.push(Ev::Shake(0.35));
            } else if !blocked {
                t.state = FState::Stun(h.kind.stun());
            }
        }
        if ko {
            fs[h.from].kos += 1;
            let name = fs[h.from].name;
            let victim = fs[h.target].name;
            ev.push(Ev::Banner(format!("{name} NOCAUTEOU {victim}")));
        }
    }
}

/// Explosão da urna atingindo lutadores.
pub fn blast_fighters(fs: &mut [Fighter], c: Vec3, r: f32, time: f32, ev: &mut Vec<Ev>) {
    let up = vec3(0.0, 1.0, 0.0);
    for f in fs.iter_mut() {
        if !f.spawned {
            continue;
        }
        let d = (f.pos + up * 0.9).distance(c);
        let rr = r * 1.8;
        if d >= rr {
            continue;
        }
        let k = 1.0 - d / rr;
        let mut dir = f.pos - c;
        dir.y = 0.0;
        let dir = dir.normalize_or(vec3(1.0, 0.0, 0.0));
        f.vel += dir * 16.0 * k + up * (8.0 * k + 3.0);
        if f.active() {
            let dmg = 45.0 * k * if f.wolverine { 0.6 } else { 1.0 };
            f.hp -= dmg;
            f.flash = 1.0;
            f.last_hit = time;
            text(ev, f.pos + up * 2.2, &format!("-{} URNA!", dmg.round() as i32), rgb(1.0, 0.5, 0.1), false);
            if f.hp <= 0.0 {
                f.hp = 0.0;
                f.state = FState::Ko(if f.wolverine { 3.5 } else { 5.0 });
                f.ko_t = 0.0;
                text(ev, f.pos + up * 2.8, "K.O. PELA URNA!", rgb(1.0, 0.3, 0.2), true);
            } else {
                f.state = FState::Stun(0.6);
            }
        }
    }
}

/// Pose de animação derivada do estado de combate.
pub fn fighter_pose(f: &Fighter, time: f32) -> Pose {
    let mut p = Pose {
        walk: f.walk,
        walk_amt: f.walk_amt,
        arm_l: -0.9,
        arm_r: -0.7,
        arm_l_out: 0.15,
        arm_r_out: 0.15,
        flash: f.flash,
        berserk: f.berserk > 0.0,
        bounce: (time * 6.0 + f.pos.x).sin().abs() * 0.03,
        ..Default::default()
    };
    match f.state {
        FState::Attack { kind, t, .. } => {
            let h = kind.hit();
            let ext = if t < h { t / h } else { (1.0 - (t - h) / (kind.dur() - h)).max(0.0) };
            match kind {
                Atk::Jab => {
                    p.arm_r = -0.7 - 0.95 * ext;
                    if f.wolverine {
                        p.arm_r_out = 0.9 - 1.1 * ext;
                    }
                }
                Atk::Cross => {
                    p.arm_l = -0.9 - 0.75 * ext;
                    if f.wolverine {
                        p.arm_l_out = 0.9 - 1.1 * ext;
                    }
                }
                Atk::Combo3 => {
                    p.arm_l = -0.8 - 0.8 * ext;
                    p.arm_r = -0.8 - 0.8 * ext;
                    p.arm_l_out = 1.0 - 1.1 * ext;
                    p.arm_r_out = 1.0 - 1.1 * ext;
                }
                Atk::Finisher => {
                    p.arm_l = -0.3 - 2.5 * ext;
                    p.arm_r = -0.3 - 2.5 * ext;
                    p.lean = -0.15 * ext;
                }
                Atk::Special => {
                    p.arm_l = -1.6;
                    p.arm_r = -1.6;
                    p.lean = -0.5;
                    p.walk_amt = 0.0;
                }
            }
        }
        FState::Stun(_) => {
            p.arm_l = -0.2;
            p.arm_r = -0.3;
            p.arm_l_out = 0.6;
            p.arm_r_out = 0.6;
            p.lean = 0.3;
        }
        FState::Dodge(..) => {
            p.lean = -0.25;
        }
        FState::Ko(_) => {
            p.lean = (f.ko_t * 4.0).min(1.0) * FRAC_PI_2;
            p.arm_l_out = 1.3;
            p.arm_r_out = 1.3;
            p.arm_l = 0.0;
            p.arm_r = 0.0;
            p.walk_amt = 0.0;
            p.bounce = 0.0;
        }
        FState::Entering => {
            p.arm_l = -2.8;
            p.arm_r = -2.8;
            p.arm_l_out = 0.5;
            p.arm_r_out = 0.5;
        }
        FState::Idle => {}
    }
    p
}
