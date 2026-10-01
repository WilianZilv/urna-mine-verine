//! URNA AIRSHIP: dirigível rígido gigante (estilo 1a Guerra) dando voltas lentas no céu. Rota, bombardeios
//! e queda são funções do relógio compartilhado (iguais pra todos sem rede); o host decide impacto das
//! bombas, rajadas das metralhadoras e a explosão da queda e manda como tiros da urna (`"w"`/`shot`, `by: 10`).

use crate::actors::Ev;
use crate::audio::{Audio, Clip};
use crate::batch::Batch;
use crate::extras::Label;
use crate::models::rgb;
use crate::npc::{self, Npcs, Vida};
use crate::urna::{self, Flash, Fx, Particle, Plan};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

pub const BY: u64 = 10;
const LEN: f32 = 72.0;
const R: f32 = 6.5;
const ALT: f32 = 34.0;
/// Segundos por volta na elipse (raios RX, RZ em volta do centro do mapa).
const LOOP: f32 = 260.0;
const RX: f32 = 50.0;
const RZ: f32 = 44.0;
/// Queda até o chão; o destroço some depois de WRECK.
const FALL: f32 = 16.0;
const WRECK: f32 = 70.0;
const BOMB_EVERY: f32 = 50.0;
const BOMB_OFF: f32 = 20.0;
const BOMBS: usize = 14;
const BOMB_GAP: f32 = 0.6;
const BAY_Z: f32 = 2.0;
const GRAV: f32 = 18.0;
const GUN_RANGE: f32 = 45.0;
const SECTIONS: usize = 9;

const FALAS: [&str; 6] = ["PATRULHA AEREA DA APURACAO", "BOMBARDEIO AUDITAVEL", "VOTO IMPRESSO? NAO, VOTO LANCADO", "100% GAS NACIONAL", "CONFIRMA? CONFIRMA!", "ABAIXO A GRAVIDADE"];

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Raio do envelope ao longo do eixo (z: +nariz / -cauda): nariz rombudo, cauda afinando.
fn radius(z: f32) -> f32 {
    let u = (z / (LEN * 0.5)).clamp(-1.0, 1.0);
    let k = if u > 0.0 { 0.35 } else { 0.6 };
    (R * (1.0 - u * u).max(0.0).powf(k)).max(0.9)
}

pub fn pos(t: f32) -> Vec3 {
    let a = t * TAU / LOOP;
    vec3(64.0 + RX * a.cos(), G as f32 + ALT + 1.2 * (t * 0.21).sin(), 64.0 + RZ * a.sin())
}

fn yaw_at(t: f32) -> f32 {
    let a = t * TAU / LOOP;
    (-RX * a.sin()).atan2(RZ * a.cos())
}

fn frame(t: f32) -> Mat4 {
    Mat4::from_translation(pos(t)) * Mat4::from_rotation_y(yaw_at(t)) * Mat4::from_rotation_z(0.04) * Mat4::from_rotation_x(0.015 * (t * 0.37).sin())
}

/// Caindo em chamas: segue deslizando, mergulha de nariz e assenta no chão em FALL segundos.
fn dead_frame(world: &World, time: f32, t: f32) -> Mat4 {
    let t0 = time - t;
    let (p0, yaw) = (pos(t0), yaw_at(t0));
    let tt = t.min(FALL);
    let mut c = p0 + vec3(yaw.sin(), 0.0, yaw.cos()) * (1.4 * tt - 0.025 * tt * tt);
    c.x = c.x.clamp(6.0, WX as f32 - 6.0);
    c.z = c.z.clamp(6.0, WZ as f32 - 6.0);
    let rest = world.floor_at(c.x, WY as f32, c.z) + R * 0.55;
    let k = tt / FALL;
    c.y = p0.y + (rest - p0.y) * k * k;
    let pitch = 0.42 * smooth(t / 5.0) - 0.3 * smooth((t - (FALL - 2.5)) / 2.5);
    Mat4::from_translation(c) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_z(0.04 + 0.25 * smooth(t / 8.0)) * Mat4::from_rotation_x(pitch)
}

fn ignite(sec: usize) -> f32 {
    0.6 + (sec as f32 - 4.0).abs() * 1.4
}

fn section_z(sec: usize) -> f32 {
    LEN * 0.5 - 4.0 - sec as f32 * 8.0
}

/// Bombas soltas nos últimos `win` segundos: (instante, índice na fileira).
fn bombs(time: f32, win: f32) -> impl Iterator<Item = (f32, usize)> {
    let n0 = ((time - win - BOMB_OFF) / BOMB_EVERY).floor() as i32;
    let n1 = ((time - BOMB_OFF) / BOMB_EVERY).floor() as i32;
    (n0..=n1).flat_map(|n| (0..BOMBS).map(move |k| (n as f32 * BOMB_EVERY + BOMB_OFF + k as f32 * BOMB_GAP, k))).filter(move |&(tr, _)| tr <= time && tr > time - win)
}

fn release(tr: f32, k: usize) -> Vec3 {
    let s = if k.is_multiple_of(2) { -1.2 } else { 1.2 };
    frame(tr).transform_point3(vec3(s, -radius(BAY_Z) - 0.3, BAY_Z))
}

/// Queda livre até o chão, o escudo do clube ou um domo.
fn drop_dist(world: &World, s: Vec3) -> f32 {
    let down = -Vec3::Y;
    let mut d = world.raycast(s, down, 90.0).map(|h| h.2).unwrap_or(s.y);
    if let Some(t) = urna::ray_sphere(s, down, shield_center(), SHIELD_R) {
        d = d.min(t);
    }
    if let Some((t, _, _)) = crate::shield::ray(s, down) {
        d = d.min(t);
    }
    d
}

/// Torres: duas em cima do casco e uma no nariz da gôndola de comando.
fn turret_local(i: usize) -> Vec3 {
    match i {
        0 => vec3(0.0, radius(24.0) + 0.3, 24.0),
        1 => vec3(0.0, radius(-4.0) + 0.3, -4.0),
        _ => vec3(0.0, -radius(21.0) - 3.4, 25.2),
    }
}

/// Esferas de acerto ao longo do casco (só vivo).
pub fn targets(time: f32, npcs: &Npcs) -> Vec<crate::gta::Target> {
    if !npcs.zeppelin().alive() {
        return Vec::new();
    }
    let m = frame(time);
    (0..7).map(|k| {
        let z = -27.0 + k as f32 * 9.0;
        (m.transform_point3(vec3(0.0, 0.0, z)), radius(z) * 0.95, 0, npc::ZEPPELIN)
    }).collect()
}

/// 2 perto, 1 meio, 0 longe (casco simplificado); o nível de qualidade encurta as faixas.
fn detail(d: f32) -> u8 {
    let k = crate::quality::pick([0.55, 0.75, 1.0]);
    if d < 110.0 * k {
        2
    } else if d < 230.0 * k {
        1
    } else {
        0
    }
}

struct Clips {
    drone: Clip,
    gun: Clip,
}

pub struct Zeppelin {
    host_alive: bool,
    last_t: f32,
    crash_prev: f32,
    gun_cd: f32,
    burst: u32,
    alive: bool,
    prev_t: f32,
    /// Traçantes (de, até, idade) e mira das torres (boca, alvo, segundos).
    tracers: Vec<(Vec3, Vec3, f32)>,
    aim: Vec<(Vec3, Vec3, f32)>,
    drone: Option<u64>,
    clips: Clips,
    sfx: Vec<(Clip, Vec3, f32)>,
}

impl Zeppelin {
    pub fn new(sr: u32) -> Self {
        Zeppelin {
            host_alive: true,
            last_t: -1.0,
            crash_prev: 0.0,
            gun_cd: 3.0,
            burst: 5,
            alive: true,
            prev_t: 0.0,
            tracers: Vec::new(),
            aim: Vec::new(),
            drone: None,
            clips: Clips { drone: Arc::new(drone(sr)), gun: Arc::new(gun(sr)) },
            sfx: Vec::new(),
        }
    }

    /// Host: bombas que chegaram ao chão, rajadas nos jogadores voando perto e a explosão da queda.
    pub fn think(&mut self, world: &World, dt: f32, time: f32, npcs: &Npcs, players: &[Vec3], events: &mut Vec<Ev>) -> Vec<Plan> {
        let mut out = Vec::new();
        let life = *npcs.zeppelin();
        let last = std::mem::replace(&mut self.last_t, time);
        if life.alive() != self.host_alive {
            self.host_alive = life.alive();
            events.push(Ev::Banner(if life.alive() { "O URNA AIRSHIP VOLTOU A PATRULHAR O CEU!".into() } else { "ZEPELIM EM CHAMAS! VOLTA EM 3 MINUTOS".into() }));
        }
        if !life.alive() {
            if self.crash_prev < FALL && life.t >= FALL {
                let m = dead_frame(world, time, FALL);
                for z in [-24.0, -8.0, 8.0, 24.0] {
                    let p = m.transform_point3(vec3(0.0, 0.0, z));
                    let g = world.floor_at(p.x, WY as f32, p.z);
                    let mut plan = urna::plan(world, vec3(p.x, g + 14.0, p.z), vec3(p.x, g - 1.0, p.z), 0.0);
                    plan.r = 5.0;
                    out.push(plan);
                }
            }
            self.crash_prev = life.t;
            return out;
        }
        self.crash_prev = 0.0;

        if last >= 0.0 && time - last < 1.0 {
            for (tr, k) in bombs(time, 6.0) {
                if k == 0 && tr > last {
                    events.push(Ev::Text { pos: release(tr, 0) - Vec3::Y * 2.0, text: "BOMBARDEIO AUDITAVEL!".into(), color: rgb(1.0, 0.6, 0.2), big: true });
                }
                let s = release(tr, k);
                let d = drop_dist(world, s);
                let ti = tr + (2.0 * d / GRAV).sqrt();
                if last < ti && ti <= time {
                    let mut plan = urna::plan(world, s, s - Vec3::Y * (d + 2.0), 0.0);
                    plan.r = gen_range(2.6, 3.4);
                    out.push(plan);
                }
            }
        }

        self.gun_cd -= dt;
        if self.gun_cd <= 0.0 {
            let m = frame(time);
            let guns = [0, 1, 2].map(|i| m.transform_point3(turret_local(i)));
            let best = players
                .iter()
                .filter(|p| p.y > G as f32 + 12.0)
                .filter_map(|p| guns.iter().map(|g| (*g, g.distance(*p))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(g, d)| (g, *p, d)))
                .filter(|x| x.2 < GUN_RANGE)
                .min_by(|a, b| a.2.total_cmp(&b.2));
            match best {
                Some((g, p, _)) => {
                    let spread = vec3(gen_range(-1.0, 1.0), gen_range(-0.6, 0.6), gen_range(-1.0, 1.0)) * 1.3;
                    let mut plan = urna::plan(world, g, p + Vec3::Y * 0.9 + spread, 0.0);
                    plan.r = 0.8;
                    out.push(plan);
                    self.burst -= 1;
                    self.gun_cd = 0.16;
                    if self.burst == 0 {
                        self.burst = 5;
                        self.gun_cd = 3.5;
                    }
                }
                None => self.gun_cd = 0.5,
            }
        }
        out
    }

    /// Todos os clientes, ao receber um tiro `by: 10`: bomba não tem feixe; bala vira traçante.
    pub fn on_shot(&mut self, plan: &Plan, fx: &mut Fx) {
        if let Some(i) = fx.beams.iter().rposition(|b| b.from == plan.o) {
            fx.beams.remove(i);
        }
        if plan.r < 1.5 {
            self.tracers.push((plan.o, plan.hit, 0.0));
            self.sfx.push((self.clips.gun.clone(), plan.o, 0.5));
            self.aim.retain(|a| a.0.distance(plan.o) > 3.0);
            self.aim.push((plan.o, plan.hit, 1.2));
        }
    }

    /// Todos os clientes: ronco dos motores, sons, fumaça/fogo da queda, estrondo no chão.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, world: &World, audio: &Audio, dt: f32, time: f32, life: &Vida, eye: Vec3, muted: bool, in_club: bool, fx: &mut Fx) {
        let alive = life.alive();
        let t = life.t;
        let m = if alive { frame(time) } else { dead_frame(world, time, t) };
        let c = m.transform_point3(Vec3::ZERO);
        let d = c.distance(eye);

        let id = *self.drone.get_or_insert_with(|| audio.play(&self.clips.drone, 0.0, true));
        let k = if alive { 1.0 } else { (1.0 - t / FALL).max(0.0) };
        let vol = if muted || in_club { 0.0 } else { 0.22 * k / (1.0 + (d - 25.0).max(0.0) / 30.0) };
        audio.set_volume(id, vol);
        for (clip, p, base) in self.sfx.drain(..) {
            let v = (base / (1.0 + p.distance(eye) / 22.0)).clamp(0.0, 1.0);
            if !muted && !in_club && v > 0.02 {
                audio.play(&clip, v, false);
            }
        }
        for tr in self.tracers.iter_mut() {
            tr.2 += dt;
        }
        self.tracers.retain(|tr| tr.2 < 0.25);
        for a in self.aim.iter_mut() {
            a.2 -= dt;
        }
        self.aim.retain(|a| a.2 > 0.0);

        let shake = |fx: &mut Fx, a: f32, range: f32| {
            if !in_club {
                fx.shake = fx.shake.max((1.0 - d / range).max(0.0) * a);
            }
        };
        if !alive && self.alive {
            for _ in 0..60 {
                let f = gen_range(0.0, 1.0);
                let p = m.transform_point3(vec3(gen_range(-4.0, 4.0), gen_range(-3.0, 5.0), gen_range(-10.0, 10.0)));
                fx.particles.push(Particle { pos: p, vel: vec3(gen_range(-1.0, 1.0), gen_range(0.0, 1.2), gen_range(-1.0, 1.0)) * gen_range(4.0, 12.0), col: Color::new(1.0, 0.35 + 0.5 * f, 0.1 * f, 1.0), life: gen_range(0.6, 1.5), size: gen_range(0.4, 1.0), gravity: false });
            }
            fx.flashes.push(Flash { pos: c, t: 0.0, r: 9.0 });
            shake(fx, 0.8, 120.0);
        }
        if !alive && self.prev_t < FALL && t >= FALL {
            for z in [-24.0, -8.0, 8.0, 24.0] {
                let p = m.transform_point3(vec3(0.0, -2.0, z));
                fx.flashes.push(Flash { pos: p, t: 0.0, r: 8.0 });
                for _ in 0..25 {
                    let a = gen_range(0.0, TAU);
                    let f = gen_range(0.0, 1.0);
                    fx.particles.push(Particle { pos: p, vel: vec3(a.cos() * gen_range(4.0, 14.0), gen_range(3.0, 12.0), a.sin() * gen_range(4.0, 14.0)), col: Color::new(1.0, 0.3 + 0.5 * f, 0.05, 1.0), life: gen_range(0.6, 1.6), size: gen_range(0.4, 1.0), gravity: true });
                }
            }
            shake(fx, 1.6, 130.0);
        }
        self.alive = alive;
        self.prev_t = if alive { 0.0 } else { t };

        // Fumaça das seções em chamas (rastro na queda, coluna no destroço)
        if !alive && t < WRECK - 5.0 {
            let rate = if d > 150.0 || crate::quality::tier() == crate::quality::LOW { 1.5 } else { 4.0 };
            for sec in 0..SECTIONS {
                if t < ignite(sec) || gen_range(0.0, 1.0) > rate * dt {
                    continue;
                }
                let z = section_z(sec);
                let p = m.transform_point3(vec3(gen_range(-1.5, 1.5), radius(z) * 0.8, z + gen_range(-3.0, 3.0)));
                let g = gen_range(0.12, 0.3);
                fx.particles.push(Particle { pos: p, vel: vec3(gen_range(-0.6, 0.6), gen_range(2.5, 5.0), gen_range(-0.6, 0.6)), col: Color::new(g, g, g, 1.0), life: gen_range(2.0, 3.5), size: gen_range(0.9, 1.6), gravity: false });
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, world: &World, labels: &mut Vec<Label>, time: f32, eye: Vec3, life: &Vida) {
        let alive = life.alive();
        let t = life.t;
        if !alive && t > WRECK {
            return;
        }
        let m = if alive { frame(time) } else { dead_frame(world, time, t) };
        let c = m.transform_point3(Vec3::ZERO);
        let lod = detail(c.distance(eye));
        let flash = life.flash;
        let tint = |col: Color| if flash > 0.0 { Color::new(col.r + flash * 0.5, col.g + flash * 0.25, col.b + flash * 0.2, 1.0) } else { col };
        let fabric = [rgb(0.63, 0.64, 0.61), rgb(0.57, 0.58, 0.56)];
        let rib = rgb(0.38, 0.39, 0.38);
        let metal = rgb(0.24, 0.26, 0.24);
        let char = rgb(0.16, 0.12, 0.1);
        let sec_of = |z: f32| (((LEN * 0.5 - z) / 8.0) as usize).min(SECTIONS - 1);
        // 0 inteiro, (0,1) queimando (escurece), >= 1 só o esqueleto
        let burn = |z: f32| if alive { 0.0 } else { ((t - ignite(sec_of(z))) / 5.0).max(0.0) };

        // Envelope: seções octogonais (duas caixas cruzadas) com nervuras escuras entre os painéis
        let n = [6usize, 9, 18][lod as usize];
        let seg = LEN / n as f32;
        for i in 0..n {
            let z = LEN * 0.5 - seg * (i as f32 + 0.5);
            let r = radius(z);
            let k = burn(z);
            if k >= 1.0 {
                let h = r * 0.85;
                b.cube(&m, vec3(0.0, h, z), vec3(2.0 * h, 0.25, 0.3), char);
                b.cube(&m, vec3(0.0, -h, z), vec3(2.0 * h, 0.25, 0.3), char);
                b.cube(&m, vec3(h, 0.0, z), vec3(0.25, 2.0 * h, 0.3), char);
                b.cube(&m, vec3(-h, 0.0, z), vec3(0.25, 2.0 * h, 0.3), char);
                if lod == 2 {
                    for (x, y) in [(0.0, h), (0.0, -h), (h, 0.0), (-h, 0.0)] {
                        b.cube(&m, vec3(x, y, z), vec3(0.2, 0.2, seg), char);
                    }
                }
                continue;
            }
            let base = fabric[i % 2];
            let col = tint(Color::new(base.r + (char.r - base.r) * k, base.g + (char.g - base.g) * k, base.b + (char.b - base.b) * k, 1.0));
            if lod == 0 {
                b.cube(&m, vec3(0.0, 0.0, z), vec3(2.0 * r, 1.7 * r, seg + 0.02), col);
                continue;
            }
            b.cube(&m, vec3(0.0, 0.0, z), vec3(2.0 * r, 1.42 * r, seg + 0.02), col);
            b.cube(&m, vec3(0.0, 0.0, z), vec3(1.42 * r, 2.0 * r, seg + 0.02), col);
            if lod == 2 && i > 0 {
                let zb = z + seg * 0.5;
                let rb = radius(zb).max(r);
                b.cube(&m, vec3(0.0, 0.0, zb), vec3(2.0 * rb + 0.2, 1.42 * rb + 0.2, 0.3), rib);
                b.cube(&m, vec3(0.0, 0.0, zb), vec3(1.42 * rb + 0.2, 2.0 * rb + 0.2, 0.3), rib);
            }
        }
        if lod == 2 {
            b.cube(&m, vec3(0.0, 0.0, LEN * 0.5), vec3(1.6, 1.6, 0.6), rib);
        }

        // Empenagem cruciforme; leme listrado com as teclas da urna (BRANCO/CORRIGE/CONFIRMA)
        let zf = -30.5;
        let rf = radius(zf);
        if lod == 0 {
            b.cube(&m, vec3(0.0, 0.3, zf), vec3(0.45, 2.0 * rf + 11.0, 8.0), metal);
            b.cube(&m, vec3(0.0, 0.0, zf), vec3(2.0 * rf + 12.0, 0.45, 8.0), metal);
        } else {
            let fin = tint(rgb(0.5, 0.51, 0.49));
            b.cube(&m, vec3(0.0, rf + 3.2, zf), vec3(0.45, 6.4, 8.0), fin);
            b.cube(&m, vec3(0.0, -rf - 2.6, zf), vec3(0.45, 5.2, 8.0), fin);
            for s in [-1.0f32, 1.0] {
                b.cube(&m, vec3(s * (rf + 3.2), 0.0, zf), vec3(6.4, 0.45, 8.0), fin);
            }
            let h = 2.0 * rf + 11.6;
            if lod == 2 {
                for (j, col) in [WHITE, rgb(1.0, 0.5, 0.1), rgb(0.1, 0.75, 0.25)].into_iter().enumerate() {
                    b.cube(&m, vec3(0.0, -rf - 5.2 + h * (j as f32 + 0.5) / 3.0, -35.4), vec3(0.35, h / 3.0, 2.2), col);
                }
                b.cube(&m, vec3(0.0, 0.0, -35.4), vec3(2.0 * rf + 12.6, 0.35, 2.2), metal);
            } else {
                b.cube(&m, vec3(0.0, 0.4, -35.4), vec3(0.35, h, 2.2), rgb(0.1, 0.75, 0.25));
            }
        }

        // Gôndola de comando (janelas acesas) e escoras
        let zc = 21.0;
        let yc = -radius(zc) - 2.4;
        b.cube(&m, vec3(0.0, yc, zc), vec3(2.8, 2.4, 7.5), tint(metal));
        if lod >= 1 {
            b.glow(&m, vec3(0.0, yc + 0.4, zc + 0.5), vec3(2.85, 0.7, 5.5), if alive { rgb(1.0, 0.9, 0.55) } else { rgb(0.2, 0.15, 0.1) });
        }
        if lod == 2 {
            b.cube(&m, vec3(0.0, yc - 0.1, zc + 4.3), vec3(2.2, 1.8, 1.2), metal);
            for dz in [-2.5, 2.5] {
                b.cube(&m, vec3(0.0, yc + 1.8, zc + dz), vec3(0.25, 1.4, 0.25), rib);
            }
        }

        // Gôndolas de motor com hélices empurrando (giram enquanto vivo / caindo)
        let spin = if alive || t < FALL { time * 22.0 } else { 0.0 };
        for (x, z) in [(5.2f32, 4.0f32), (-5.2, 4.0), (4.6, -12.0), (-4.6, -12.0), (0.0, -20.0)] {
            if lod == 0 {
                continue;
            }
            let r = radius(z);
            let y = if x == 0.0 { -r - 1.6 } else { -0.71 * r - 1.6 };
            b.cube(&m, vec3(x, y, z), vec3(1.6, 1.7, 4.0), tint(metal));
            if lod < 2 {
                continue;
            }
            b.cube(&m, vec3(x, y + 1.25, z), vec3(0.25, 1.0, 0.25), rib);
            let hub = m * Mat4::from_translation(vec3(x, y, z - 2.3));
            b.cube(&hub, Vec3::ZERO, Vec3::splat(0.45), rgb(0.15, 0.15, 0.15));
            for j in 0..2 {
                let bl = hub * Mat4::from_rotation_z(spin + j as f32 * PI * 0.5 + x);
                b.cube(&bl, Vec3::ZERO, vec3(0.28, 3.4, 0.12), rgb(0.35, 0.24, 0.14));
            }
        }

        // Torres de metralhadora (cano mira no último alvo)
        if lod == 2 {
            for i in 0..3 {
                let lp = turret_local(i);
                let wp = m.transform_point3(lp);
                let aim = self.aim.iter().find(|a| a.0.distance(wp) < 3.0).map(|a| a.1);
                let dir = match aim {
                    Some(p) => (p - wp).normalize_or(Vec3::Z),
                    None => m.transform_vector3(vec3((time * 0.3 + i as f32).sin(), if i == 2 { -0.3 } else { 0.15 }, (time * 0.3 + i as f32).cos())).normalize_or(Vec3::Z),
                };
                let ring = if i == 2 { vec3(0.9, 0.6, 0.9) } else { vec3(1.3, 0.6, 1.3) };
                b.cube(&m, lp, ring, metal);
                let gm = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Z, dir), wp + dir * 0.8);
                b.cube(&gm, Vec3::ZERO, vec3(0.18, 0.18, 1.6), rgb(0.1, 0.1, 0.1));
            }
        }

        // Emblema satírico nas laterais: placa com uma urna (tela + teclas)
        if lod >= 1 {
            let ze = 10.0;
            let re = radius(ze);
            for s in [-1.0f32, 1.0] {
                if burn(ze) >= 1.0 {
                    break;
                }
                b.cube(&m, vec3(s * (re + 0.04), 0.0, ze), vec3(0.08, 3.6, 7.0), rgb(0.92, 0.9, 0.82));
                if lod == 2 {
                    b.cube(&m, vec3(s * (re + 0.09), 0.5, ze + 1.4 * s), vec3(0.08, 1.8, 2.6), rgb(0.12, 0.14, 0.18));
                    for (j, col) in [WHITE, rgb(1.0, 0.5, 0.1), rgb(0.1, 0.8, 0.25)].into_iter().enumerate() {
                        b.glow(&m, vec3(s * (re + 0.09), -0.9, ze - s * (0.6 + j as f32 * 1.0)), vec3(0.08, 0.6, 0.8), col);
                    }
                }
            }
        }

        // Fogo nas seções que pegaram
        if !alive {
            for sec in 0..SECTIONS {
                let ig = ignite(sec);
                if t < ig || t > WRECK - 4.0 {
                    continue;
                }
                let z = section_z(sec);
                let r = radius(z);
                let f = 0.8 + 0.25 * (time * 17.0 + sec as f32 * 2.1).sin();
                let grow = ((t - ig) / 1.5).min(1.0) * f;
                trans.glow(&m, vec3(0.0, r * 0.6 + 1.0, z), vec3(r * 1.5, 3.0 * grow + 0.5, 6.5) * grow.max(0.2), Color::new(1.0, 0.3, 0.0, 0.75));
                trans.glow(&m, vec3(0.0, r * 0.6 + 0.6, z), vec3(r * 0.9, 2.0 * grow, 4.5) * grow.max(0.2), Color::new(1.0, 0.7, 0.05, 0.9));
            }
        }

        // Bombas caindo
        if alive {
            for (tr, k) in bombs(time, 4.0) {
                let s = release(tr, k);
                let fall = 0.5 * GRAV * (time - tr) * (time - tr);
                if fall < drop_dist(world, s) {
                    let bm = Mat4::from_translation(s - Vec3::Y * fall);
                    b.cube(&bm, Vec3::ZERO, vec3(0.5, 1.1, 0.5), rgb(0.15, 0.16, 0.14));
                    b.cube(&bm, vec3(0.0, 0.6, 0.0), vec3(0.8, 0.2, 0.8), rgb(0.3, 0.3, 0.28));
                }
            }
        }
        for &(a, c, age) in &self.tracers {
            let k = age / 0.15;
            if k < 1.0 {
                urna::beam(trans, a.lerp(c, (k - 0.3).max(0.0)), a.lerp(c, k), 0.12, Color::new(1.0, 0.85, 0.3, 0.9));
            }
            if age < 0.06 {
                trans.glow(&Mat4::from_translation(a), Vec3::ZERO, Vec3::splat(0.7), Color::new(1.0, 0.8, 0.3, 0.7));
            }
        }

        // Sombra no chão
        if alive && lod >= 1 {
            let g = world.floor_at(c.x, WY as f32, c.z) + 0.06;
            let sm = Mat4::from_translation(vec3(c.x, g, c.z)) * Mat4::from_rotation_y(yaw_at(time));
            trans.glow(&sm, Vec3::ZERO, vec3(2.0 * R * 0.9, 0.05, LEN * 0.9), Color::new(0.0, 0.0, 0.0, 0.16));
        }

        let tag = m.transform_point3(vec3(0.0, R + 3.0, 6.0));
        if !alive {
            if t < 10.0 {
                labels.push(Label { pos: tag, text: "\"OH, A HUMANIDADE... E O FUNDO PARTIDARIO!\"".into(), size: 26.0, color: rgb(1.0, 0.5, 0.3) });
            }
            return;
        }
        if lod >= 1 {
            labels.push(Label { pos: tag, text: "URNA AIRSHIP".into(), size: 32.0, color: rgb(0.95, 0.9, 0.75) });
            if time.rem_euclid(13.0) < 5.0 {
                let fala = FALAS[(time / 13.0) as usize % FALAS.len()];
                labels.push(Label { pos: tag + Vec3::Y * 2.5, text: format!("\"{fala}\""), size: 24.0, color: rgb(0.6, 1.0, 0.6) });
            }
        }
    }
}

// ------------------------------------------------ Sons sintetizados

/// Ronco dos motores em loop de 2s: harmônicos de 55 Hz com pulsar das hélices (fecha o ciclo sem estalo).
fn drone(sr: u32) -> Vec<f32> {
    let n = (2.0 * sr as f32) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let tone: f32 = (1..=6).map(|h| (TAU * 55.0 * h as f32 * t).sin() / h as f32).sum::<f32>() + 0.6 * (TAU * 27.5 * t).sin();
            let pulse = 0.6 + 0.25 * (PI * 9.0 * t).sin().powi(2) + 0.15 * (PI * 6.5 * t).sin().powi(2);
            (tone * pulse * 0.7).tanh() * 0.8
        })
        .collect()
}

/// Rajada curta de metralhadora: três estalos com corpo grave.
fn gun(sr: u32) -> Vec<f32> {
    let mut s = 0x9E37_79B9u32;
    (0..(0.36 * sr as f32) as usize)
        .map(|i| {
            let t = i as f32 / sr as f32;
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let n = (s as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let tt = t % 0.11;
            (n * (-tt / 0.018).exp() + (TAU * 95.0 * tt).sin() * (-tt / 0.04).exp()) * 0.8
        })
        .collect()
}
