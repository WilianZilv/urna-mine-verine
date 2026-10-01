//! Mods ao vivo: pacotes declarativos (JSON validado no servidor, nunca código) viram entidades no jogo.
//! Modelo = caixas por parte com pivô, animação por keyframes, comportamento = primitivas da whitelist.
//! O host simula e manda posições no snapshot ("md"); vida/morte usa o grupo `npc::MODS` (host autoritativo).
//! Zona de mods: praça ao norte do laboratório com painel, spawn pads e pedestais de itens/blocos.

use crate::audio::Clip;
use crate::batch::Batch;
use crate::club::hsv;
use crate::extras::Label;
use crate::models::rgb;
use crate::npc::{self, Npcs, Vida};
use crate::urna::beam;
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::f32::consts::TAU;
use std::sync::Arc;

pub const SITE: &str = "urna-mine-verine.wilianzilv.workers.dev";
const ZX0: i32 = 95;
const ZX1: i32 = 111;
const ZZ0: i32 = 31;
const ZZ1: i32 = 47;
const PADS: [(f32, f32); 4] = [(99.5, 37.5), (107.5, 37.5), (99.5, 43.5), (107.5, 43.5)];
const PANEL_Z: f32 = ZZ0 as f32 + 0.8;
const GRAV: f32 = 20.0;

const ANIMS: [&str; 7] = ["idle", "walk", "attack", "death", "roar", "jump", "fly"];
const SOUNDS: [&str; 7] = ["roar", "attack", "hurt", "death", "spawn", "beam", "say"];
const PALETTE: [(&str, u32); 16] = [
    ("white", 0xf2f2f2), ("light_gray", 0xa8a8a8), ("gray", 0x5c5c5c), ("black", 0x1c1c1c), ("red", 0xd83a2e), ("orange", 0xf08a24),
    ("yellow", 0xf5d63a), ("lime", 0x7ed63a), ("green", 0x3a8a2e), ("cyan", 0x2ec4c4), ("light_blue", 0x6ab4f0), ("blue", 0x2e4ad8),
    ("purple", 0x8a3ad8), ("magenta", 0xd83ab4), ("pink", 0xf0a0c0), ("brown", 0x7a5030),
];
const PATTERNS: [&str; 6] = ["solid", "checker", "stripes", "dots", "border", "bricks"];

/// Mundo gerado + praça da zona de mods (mesmo resultado em todo cliente).
pub fn generate() -> World {
    let mut w = World::generate();
    let g = G;
    for z in ZZ0..=ZZ1 {
        for x in ZX0..=ZX1 {
            for y in g..=g + 16 {
                w.set(x, y, z, AIR);
            }
            for y in g - 6..g - 1 {
                if w.get(x, y, z) == AIR {
                    w.set(x, y, z, STONE);
                }
            }
            let edge = x == ZX0 || x == ZX1 || z == ZZ0 || z == ZZ1;
            w.set(x, g - 1, z, if edge { STONE } else if (x + z) % 2 == 0 { crate::world::BLACK } else { COBBLE });
        }
    }
    for x in ZX0..=ZX1 {
        w.set(x, g - 1, ZZ1, NEON);
    }
    for (px, pz) in PADS {
        for dz in -1..=1 {
            for dx in -1..=1 {
                w.set(px as i32 + dx, g - 1, pz as i32 + dz, NEON);
            }
        }
    }
    w.guard = true;
    w
}

// ---------------------------------------------------------------- pacote -> definição
struct Cube {
    pos: Vec3,
    size: Vec3,
    col: Color,
    glow: bool,
}

struct Part {
    parent: Option<usize>,
    pivot: Vec3,
    boxes: Vec<Cube>,
}

struct Key {
    t: f32,
    /// (rotação em radianos, offset) por parte; parte ausente = pose de repouso.
    poses: Vec<(Vec3, Vec3)>,
}

struct Anim {
    dur: f32,
    looped: bool,
    keys: Vec<Key>,
}

#[derive(Default)]
struct Behav {
    wander: Option<(f32, f32)>,
    fly: Option<(f32, f32)>,
    follow: Option<(f32, f32)>,
    flee: Option<(f32, f32)>,
    melee: Option<(f32, f32, f32)>,
    beam: Option<(f32, f32, f32, Color)>,
    jump: Option<(f32, f32)>,
    roar: Option<(f32, f32)>,
    say: Option<(f32, Vec<String>)>,
    dust: Option<(f32, usize, Color, f32, f32)>,
    coins: Option<u32>,
}

struct Show {
    name: String,
    c1: Color,
    c2: Color,
    pattern: usize,
    block: bool,
}

#[derive(Default)]
struct Ent {
    pos: Vec3,
    target: Vec3,
    home: Vec3,
    yaw: f32,
    vy: f32,
    placed: bool,
    moving: bool,
    air: bool,
    goal: Option<Vec3>,
    wait: f32,
    cd_melee: f32,
    cd_beam: f32,
    t_jump: f32,
    t_roar: f32,
    t_say: f32,
    t_dust: f32,
    // contadores sincronizados (host incrementa; todo cliente reage quando muda)
    an: u32,
    bn: u32,
    rn: u32,
    sn: u32,
    si: usize,
    bt: Vec3,
    seen: [u32; 4],
    synced: bool,
    // efeitos locais
    act: u8,
    act_t: f32,
    anim_t: f32,
    hit_in: f32,
    beam_t: f32,
    say: Option<(String, f32)>,
    alive: bool,
    hp: f32,
    hurt_cd: f32,
    coins: Option<(Vec3, f32)>,
}

struct Def {
    id: String,
    name: String,
    version: String,
    creator: String,
    parts: Vec<Part>,
    anims: [Option<Anim>; 7],
    hp: f32,
    speed: f32,
    scale: f32,
    zone: bool,
    max: usize,
    respawn: f32,
    b: Behav,
    sounds: [Option<Clip>; 7],
    shows: Vec<Show>,
    height: f32,
    radius: f32,
    ents: Vec<Ent>,
    born: f64,
}

struct Dust {
    pos: Vec3,
    vel: Vec3,
    col: Color,
    life: f32,
    size: f32,
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn v3(v: &Value) -> Vec3 {
    vec3(f(&v[0]), f(&v[1]), f(&v[2]))
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn hex(v: &Value) -> Color {
    let h = u32::from_str_radix(v.as_str().unwrap_or("#ff00ff").trim_start_matches('#'), 16).unwrap_or(0xff00ff);
    from_u32(h)
}

fn from_u32(h: u32) -> Color {
    rgb(((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0)
}

fn pal(v: &Value) -> Color {
    from_u32(PALETTE.iter().find(|p| Some(p.0) == v.as_str()).map_or(0xffffff, |p| p.1))
}

fn hash(s: &str) -> u32 {
    s.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

/// Efeito sonoro paramétrico (mesma ideia do synth.rs: tudo gerado em código, nada de arquivo).
fn tone(sr: u32, p: &Value) -> Clip {
    let dur = f(&p["duration"]).clamp(0.05, 2.0);
    let (f0, f1) = (f(&p["freq"]).clamp(30.0, 2000.0), f(&p["freq_end"]).clamp(30.0, 2000.0));
    let (vol, att, vib, noise) = (f(&p["volume"]).clamp(0.0, 1.0), f(&p["attack"]).clamp(0.002, 0.5), f(&p["vibrato"]).clamp(0.0, 40.0), f(&p["noise"]).clamp(0.0, 1.0));
    let wave = p["wave"].as_str().unwrap_or("sine");
    let n = (dur * sr as f32) as usize;
    let (mut ph, mut lp, mut seed) = (0.0f32, 0.0f32, 0x9E37_79B9u32);
    let out = (0..n)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let k = t / dur;
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let r = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
            let fr = f0 * (f1 / f0).powf(k) * (1.0 + 0.04 * (TAU * vib * t).sin());
            ph += fr / sr as f32;
            let x = ph.fract();
            let osc = match wave {
                "square" => if x < 0.5 { 1.0 } else { -1.0 },
                "saw" => 2.0 * x - 1.0,
                "triangle" => 4.0 * (x - 0.5).abs() - 1.0,
                "noise" => r,
                _ => (TAU * x).sin(),
            };
            lp += (r - lp) * 0.2;
            let env = (t / att).min(1.0) * (1.0 - k).powf(1.5);
            ((osc * (1.0 - noise) + lp * 2.5 * noise) * env * vol * 1.4).tanh() * 0.85
        })
        .collect();
    Arc::new(out)
}

fn parse(m: &Value, sr: u32) -> Option<Def> {
    let p = &m["pkg"];
    let id = m["id"].as_str()?.to_string();
    let jparts = p["model"]["parts"].as_array()?;
    let names: Vec<&str> = jparts.iter().map(|x| x["name"].as_str().unwrap_or("")).collect();
    let parts: Vec<Part> = jparts
        .iter()
        .map(|x| Part {
            parent: x["parent"].as_str().and_then(|n| names.iter().position(|q| *q == n)),
            pivot: v3(&x["pivot"]),
            boxes: x["boxes"].as_array().into_iter().flatten().map(|b| Cube { pos: v3(&b["pos"]), size: v3(&b["size"]), col: hex(&b["color"]), glow: b["glow"].as_bool().unwrap_or(false) }).collect(),
        })
        .collect();
    let anims = ANIMS.map(|a| {
        let j = &p["animations"][a];
        let keys = j["keyframes"].as_array()?;
        Some(Anim {
            dur: f(&j["duration"]).max(0.1),
            looped: j["loop"].as_bool().unwrap_or(false),
            keys: keys
                .iter()
                .map(|k| Key {
                    t: f(&k["t"]),
                    poses: names.iter().map(|n| (v3(&k["parts"][*n]["rot"]) * (std::f32::consts::PI / 180.0), v3(&k["parts"][*n]["offset"]))).collect(),
                })
                .collect(),
        })
    });
    let bh = &p["behavior"];
    let mut b = Behav::default();
    for x in bh["primitives"].as_array().into_iter().flatten() {
        let g = |k: &str| f(&x[k]);
        match x["type"].as_str().unwrap_or("") {
            "wander" => b.wander = Some((g("radius"), g("pause"))),
            "fly" => b.fly = Some((g("height"), g("bob"))),
            "follow_player" => b.follow = Some((g("range"), g("stop"))),
            "flee" => b.flee = Some((g("range"), g("below_hp"))),
            "attack_melee" => b.melee = Some((g("damage"), g("range"), g("cooldown").max(0.5))),
            "shoot_beam" => b.beam = Some((g("damage"), g("range"), g("cooldown").max(1.5), hex(&x["color"]))),
            "jump" => b.jump = Some((g("every").max(1.0), g("height"))),
            "roar" => b.roar = Some((g("every").max(3.0), g("shake"))),
            "say" => b.say = Some((g("every").max(3.0), x["phrases"].as_array().into_iter().flatten().map(s).collect())),
            "spawn_particles" => b.dust = Some((g("every").max(0.1), (g("count") as usize).clamp(1, 30), hex(&x["color"]), g("speed"), g("size"))),
            "drop_coins" => b.coins = Some(g("amount") as u32),
            _ => {}
        }
    }
    let st = &bh["stats"];
    let scale = f(&st["scale"]).clamp(0.25, 4.0);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in parts.iter().flat_map(|p| &p.boxes) {
        lo = lo.min(c.pos - c.size * 0.5);
        hi = hi.max(c.pos + c.size * 0.5);
    }
    if lo.x > hi.x {
        return None;
    }
    let height = hi.y.max(0.5) * scale;
    let width = (hi.x - lo.x).max(hi.z - lo.z) * scale;
    let mut shows = Vec::new();
    for (k, block) in [("items", false), ("blocks", true)] {
        for x in p[k].as_array().into_iter().flatten() {
            let c1 = pal(&x["color"]);
            let c2 = if block { pal(&x["color2"]) } else { Color::new(c1.r * 0.6, c1.g * 0.6, c1.b * 0.6, 1.0) };
            shows.push(Show { name: s(&x["name"]), c1, c2, pattern: PATTERNS.iter().position(|q| Some(*q) == x["pattern"].as_str()).unwrap_or(0), block });
        }
    }
    let sp = &bh["spawn"];
    Some(Def {
        name: s(&p["manifest"]["name"]),
        version: s(&p["manifest"]["version"]),
        creator: m["creator"].as_str().map(String::from).unwrap_or_else(|| s(&p["manifest"]["author"])),
        parts,
        anims,
        hp: f(&st["hp"]).clamp(1.0, 5000.0),
        speed: f(&st["speed"]).clamp(0.0, 12.0),
        scale,
        zone: sp["where"].as_str() != Some("map"),
        max: (sp["max_instances"].as_u64().unwrap_or(1) as usize).clamp(1, 4),
        respawn: f(&sp["respawn"]).clamp(5.0, 300.0),
        b,
        sounds: SOUNDS.map(|k| p["sounds"].get(k).map(|x| tone(sr, x))),
        shows,
        height,
        radius: (0.5 * width.max(height) * 0.7).max(0.6),
        ents: Vec::new(),
        born: get_time(),
        id,
    })
}

fn home(zone: bool, slot: usize, k: usize, id: &str) -> Vec3 {
    if zone {
        let (x, z) = PADS[slot % PADS.len()];
        return vec3(x + (k / PADS.len()) as f32 * 1.5, G as f32, z);
    }
    let h = hash(id).wrapping_add(k as u32 * 7919);
    let a = (h % 360) as f32 * TAU / 360.0;
    let r = 18.0 + ((h / 360) % 22) as f32;
    vec3((64.0 + a.cos() * r).clamp(4.0, 123.0), G as f32, (64.0 + a.sin() * r).clamp(4.0, 123.0))
}

fn sample(a: &Anim, t: f32, n: usize) -> Vec<(Vec3, Vec3)> {
    let mut out = vec![(Vec3::ZERO, Vec3::ZERO); n];
    let (Some(first), Some(last)) = (a.keys.first(), a.keys.last()) else { return out };
    let t = if a.looped { t.rem_euclid(a.dur) } else { t.min(a.dur) };
    let (k0, k1, w) = if t <= first.t {
        (first, first, 0.0)
    } else if t >= last.t {
        (last, last, 0.0)
    } else {
        let i = a.keys.windows(2).position(|p| t >= p[0].t && t < p[1].t).unwrap_or(0);
        let (x, y) = (&a.keys[i], &a.keys[i + 1]);
        (x, y, (t - x.t) / (y.t - x.t).max(1e-4))
    };
    for (i, o) in out.iter_mut().enumerate() {
        let (a0, a1) = (k0.poses.get(i).copied().unwrap_or_default(), k1.poses.get(i).copied().unwrap_or_default());
        *o = (a0.0.lerp(a1.0, w), a0.1.lerp(a1.1, w));
    }
    out
}

/// Cubo 4x4x4 com padrão de paleta (itens/blocos de mod nos pedestais).
fn pattern_cube(b: &mut Batch, m: &Mat4, sz: f32, sh: &Show) {
    let q = sz / 4.0;
    for z in 0..4i32 {
        for y in 0..4i32 {
            for x in 0..4i32 {
                if (1..3).contains(&x) && (1..3).contains(&y) && (1..3).contains(&z) {
                    continue;
                }
                let alt = match sh.pattern {
                    1 => (x + y + z) % 2 == 1,
                    2 => y % 2 == 1,
                    3 => (x == 1 || x == 2) as i32 + (y == 1 || y == 2) as i32 + (z == 1 || z == 2) as i32 >= 2,
                    4 => [x, y, z].iter().filter(|v| **v == 0 || **v == 3).count() >= 2,
                    5 => y % 2 == 1 || (x + if y % 4 == 0 { 0 } else { 2 }) % 4 == 0,
                    _ => false,
                };
                let c = vec3(x as f32 - 1.5, y as f32 - 1.5, z as f32 - 1.5) * q;
                b.cube(m, c, Vec3::splat(q), if alt { sh.c2 } else { sh.c1 });
            }
        }
    }
}

// ---------------------------------------------------------------- runtime
pub struct Mods {
    defs: Vec<Def>,
    sr: u32,
    dust: Vec<Dust>,
    /// (som, posição) pro main tocar com `play_at`.
    pub sfx: Vec<(Clip, Vec3)>,
    /// Mensagens pro servidor (moedas pegas).
    pub outbox: Vec<Value>,
}

impl Mods {
    pub fn new(sr: u32) -> Self {
        Mods { defs: Vec::new(), sr, dust: Vec::new(), sfx: Vec::new(), outbox: Vec::new() }
    }

    /// {t:"mods", ids} (lista ativa, ordem de ativação) e {t:"mod", op:"set"|"del", ...}.
    pub fn on_msg(&mut self, m: &Value) {
        match (m["t"].as_str(), m["op"].as_str()) {
            (Some("mods"), _) => {
                let ids: Vec<&str> = m["ids"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
                self.defs.retain(|d| ids.contains(&d.id.as_str()));
                self.defs.sort_by_key(|d| ids.iter().position(|i| *i == d.id));
            }
            (Some("mod"), Some("set")) => {
                let Some(mut d) = parse(m, self.sr) else { return };
                let slot = self.defs.iter().position(|x| x.id == d.id).unwrap_or(self.defs.len());
                d.ents = (0..d.max)
                    .map(|k| {
                        let h = home(d.zone, slot + k, k, &d.id);
                        Ent { pos: h, target: h, home: h, alive: true, hp: d.hp, t_jump: gen_range(1.0, 5.0), t_roar: gen_range(2.0, 8.0), t_say: gen_range(1.0, 6.0), ..Default::default() }
                    })
                    .collect();
                if let Some(c) = d.sounds[4].clone() {
                    self.sfx.push((c, d.ents[0].pos));
                }
                if slot < self.defs.len() {
                    self.defs[slot] = d;
                } else {
                    self.defs.push(d);
                }
            }
            (Some("mod"), Some("del")) => self.defs.retain(|d| m["id"].as_str() != Some(d.id.as_str())),
            _ => {}
        }
    }

    /// Alvos acertáveis (mesmo formato de `npc_targets`): (centro, raio, índice, grupo).
    pub fn targets(&self, npcs: &Npcs) -> Vec<(Vec3, f32, usize, u8)> {
        let mut out = Vec::new();
        let mut k = 0;
        for d in &self.defs {
            for e in &d.ents {
                if npcs.alive(npc::MODS, k) {
                    out.push((e.pos + Vec3::Y * d.height * 0.5, d.radius, k, npc::MODS));
                }
                k += 1;
            }
        }
        out
    }

    /// Host: {id: [[x,y,z,yaw,flags,an,bn,bx,by,bz,rn,si,sn], ...]}.
    pub fn snapshot(&self) -> Value {
        let r = |x: f32| (x as f64 * 100.0).round() / 100.0;
        let mut o = serde_json::Map::new();
        for d in &self.defs {
            let list: Vec<Value> = d.ents.iter().map(|e| json!([r(e.pos.x), r(e.pos.y), r(e.pos.z), r(e.yaw), e.moving as u8 + 2 * e.air as u8, e.an, e.bn, r(e.bt.x), r(e.bt.y), r(e.bt.z), e.rn, e.si, e.sn])).collect();
            o.insert(d.id.clone(), Value::Array(list));
        }
        Value::Object(o)
    }

    pub fn apply(&mut self, m: &Value) {
        for d in &mut self.defs {
            for (e, v) in d.ents.iter_mut().zip(m[d.id.as_str()].as_array().into_iter().flatten()) {
                let u = |i: usize| v[i].as_u64().unwrap_or(0) as u32;
                e.target = vec3(f(&v[0]), f(&v[1]), f(&v[2]));
                if !e.synced || e.pos.distance(e.target) > 8.0 {
                    e.pos = e.target;
                }
                e.yaw = f(&v[3]);
                e.moving = u(4) & 1 != 0;
                e.air = u(4) & 2 != 0;
                (e.an, e.bn, e.rn, e.sn, e.si) = (u(5), u(6), u(10), u(12), u(11) as usize);
                e.bt = vec3(f(&v[7]), f(&v[8]), f(&v[9]));
                if !e.synced {
                    e.seen = [e.an, e.bn, e.rn, e.sn];
                    e.synced = true;
                }
            }
        }
    }

    /// Simula (host) / interpola (demais) e dispara efeitos locais. Retorna (dano em mim, tremida de câmera).
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, world: &World, dt: f32, time: f32, host: bool, npcs: &mut Npcs, me: Vec3, others: &[(u64, Vec3)]) -> (f32, f32) {
        let spec: Vec<(f32, f32)> = self.defs.iter().flat_map(|d| std::iter::repeat_n((d.hp, d.respawn), d.ents.len())).collect();
        npcs.sync_mods(&spec);
        let mut players = vec![me];
        players.extend(others.iter().map(|o| o.1));
        let (mut hurt, mut shake) = (0.0f32, 0.0f32);
        let mut k = 0;
        for d in &mut self.defs {
            for e in &mut d.ents {
                let life: Option<Vida> = npcs.get(npc::MODS, k).copied();
                k += 1;
                let alive = life.is_none_or(|v| v.alive());
                let hp = life.map_or(d.hp, |v| v.hp);
                if host {
                    if !e.placed || (alive && !e.alive) {
                        e.pos = vec3(e.home.x, world.floor_at(e.home.x, (WY - 1) as f32, e.home.z), e.home.z);
                        e.home.y = e.pos.y;
                        e.vy = 0.0;
                        e.placed = true;
                    }
                    if alive {
                        think(d.speed, &d.b, d.zone, d.hp, world, dt, time, e, hp, &players);
                    } else {
                        e.moving = false;
                    }
                } else {
                    e.pos = e.pos.lerp(e.target, (dt * 10.0).min(1.0));
                }
                // reações aos contadores (iguais em todos os clientes)
                let near = e.pos.distance(me);
                let pos = e.pos;
                let sounds = &d.sounds;
                let snd = |i: usize| sounds[i].clone().map(|c| (c, pos));
                if e.an != e.seen[0] {
                    e.seen[0] = e.an;
                    (e.act, e.act_t) = (1, 0.0);
                    e.hit_in = d.anims[2].as_ref().map_or(0.3, |a| (a.dur * 0.55).min(0.6));
                    self.sfx.extend(snd(1));
                }
                if e.bn != e.seen[1] {
                    e.seen[1] = e.bn;
                    (e.act, e.act_t, e.beam_t) = (1, 0.0, 0.35);
                    self.sfx.extend(snd(5));
                    if let Some((dmg, ..)) = d.b.beam {
                        if alive && (me + Vec3::Y).distance(e.bt) < 1.6 {
                            hurt += dmg;
                        }
                    }
                }
                if e.rn != e.seen[2] {
                    e.seen[2] = e.rn;
                    (e.act, e.act_t) = (2, 0.0);
                    self.sfx.extend(snd(0));
                    shake = shake.max(d.b.roar.map_or(0.0, |r| r.1) * (1.0 - near / 40.0).max(0.0));
                }
                if e.sn != e.seen[3] {
                    e.seen[3] = e.sn;
                    if let Some((_, ph)) = &d.b.say {
                        e.say = ph.get(e.si % ph.len().max(1)).map(|p| (p.clone(), 4.5));
                        self.sfx.extend(snd(6));
                    }
                }
                if e.hit_in > 0.0 {
                    e.hit_in -= dt;
                    if e.hit_in <= 0.0 && alive {
                        if let Some((dmg, range, _)) = d.b.melee {
                            let dh = vec2(me.x - e.pos.x, me.z - e.pos.z).length();
                            if dh < range + 0.6 && me.y > e.pos.y - 1.5 && me.y < e.pos.y + d.height {
                                hurt += dmg;
                                shake = shake.max(0.3);
                            }
                        }
                    }
                }
                if alive && hp < e.hp && e.hurt_cd <= 0.0 {
                    self.sfx.extend(snd(2));
                    e.hurt_cd = 0.3;
                }
                if e.alive && !alive {
                    self.sfx.extend(snd(3));
                    if let Some(n) = d.b.coins {
                        e.coins = Some((e.pos, 30.0));
                        e.say = Some((format!("+{n} MOEDAS NO CHAO!"), 3.0));
                    }
                }
                e.hurt_cd -= dt;
                (e.alive, e.hp) = (alive, hp);
                e.act_t += dt;
                e.anim_t += dt;
                e.beam_t -= dt;
                if let Some(sy) = e.say.as_mut() {
                    sy.1 -= dt;
                }
                if e.say.as_ref().is_some_and(|sy| sy.1 <= 0.0) {
                    e.say = None;
                }
                if let Some((p, t)) = e.coins.as_mut() {
                    *t -= dt;
                    if vec2(me.x - p.x, me.z - p.z).length() < 2.0 && (me.y - p.y).abs() < 2.5 {
                        self.outbox.push(json!({"t": "mk", "m": d.id}));
                        for _ in 0..16 {
                            self.dust.push(Dust { pos: *p + Vec3::Y * 0.5, vel: vec3(gen_range(-2.0, 2.0), gen_range(2.0, 5.0), gen_range(-2.0, 2.0)), col: Color::new(1.0, 0.85, 0.2, 1.0), life: 0.8, size: 0.12 });
                        }
                        *t = 0.0;
                    }
                }
                if e.coins.is_some_and(|c| c.1 <= 0.0) {
                    e.coins = None;
                }
                if let Some((every, count, col, speed, size)) = d.b.dust {
                    e.t_dust -= dt;
                    if e.t_dust <= 0.0 && alive && near < 60.0 {
                        e.t_dust = every;
                        for _ in 0..count {
                            let o = vec3(gen_range(-1.0, 1.0), gen_range(0.0, 1.0), gen_range(-1.0, 1.0)) * vec3(d.radius, d.height, d.radius);
                            self.dust.push(Dust { pos: e.pos + o, vel: vec3(gen_range(-1.0, 1.0), gen_range(0.3, 1.5), gen_range(-1.0, 1.0)) * speed, col, life: gen_range(0.6, 1.2), size });
                        }
                    }
                }
            }
        }
        for p in &mut self.dust {
            p.vel.y -= 3.0 * dt;
            p.pos += p.vel * dt;
            p.life -= dt;
        }
        self.dust.retain(|p| p.life > 0.0);
        if self.dust.len() > 600 {
            self.dust.drain(..self.dust.len() - 600);
        }
        (hurt, shake)
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3, npcs: &Npcs) {
        self.draw_zone(b, trans, labels, time, eye);
        let mut k = 0;
        for d in &self.defs {
            for e in &d.ents {
                let life = npcs.get(npc::MODS, k).copied();
                k += 1;
                if e.pos.distance(eye) > 90.0 {
                    continue;
                }
                draw_ent(b, trans, d, e, life.as_ref(), time);
                let top = e.pos + Vec3::Y * (d.height + 0.7);
                if let Some((p, _)) = e.coins {
                    let m = Mat4::from_translation(p + Vec3::Y * (0.4 + (time * 3.0).sin() * 0.1)) * Mat4::from_rotation_y(time * 3.0);
                    for i in 0..5 {
                        b.glow(&m, vec3((i as f32 - 2.0) * 0.25, (i % 2) as f32 * 0.15, 0.0), vec3(0.22, 0.06, 0.22), Color::new(1.0, 0.82, 0.2, 1.0));
                    }
                    if p.distance(eye) < 30.0 {
                        labels.push(Label { pos: p + Vec3::Y * 1.2, text: format!("+{} MOEDAS - PEGA!", d.b.coins.unwrap_or(0)), size: 16.0, color: Color::new(1.0, 0.85, 0.3, 1.0) });
                    }
                }
                if let Some((say, _)) = &e.say {
                    if e.pos.distance(eye) < 45.0 {
                        labels.push(Label { pos: top + Vec3::Y * 0.6, text: format!("\"{say}\""), size: 18.0, color: rgb(1.0, 0.95, 0.6) });
                    }
                }
                if e.pos.distance(eye) < 35.0 {
                    let tag = match life {
                        Some(v) if !v.alive() => format!("{} - VOLTA EM {:.0}s", d.name.to_uppercase(), v.down.max(0.0)),
                        _ => format!("{} (MOD v{} por {})", d.name.to_uppercase(), d.version, d.creator),
                    };
                    labels.push(Label { pos: top, text: tag, size: 18.0, color: rgb(0.75, 0.6, 1.0) });
                }
            }
        }
        let id = Mat4::IDENTITY;
        for p in &self.dust {
            trans.glow(&id, p.pos, Vec3::splat(p.size), Color::new(p.col.r, p.col.g, p.col.b, (p.life * 1.5).min(1.0)));
        }
    }

    fn draw_zone(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        let c = vec3((ZX0 + ZX1) as f32 * 0.5 + 0.5, G as f32, (ZZ0 + ZZ1) as f32 * 0.5 + 0.5);
        if eye.distance(c) > 110.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let g = G as f32;
        let dark = rgb(0.08, 0.06, 0.12);
        let k = 0.7 + 0.3 * (time * 2.0).sin();
        // Painel gigante na borda norte, virado pro sul (pra quem vem do lab)
        let (w, h, y0) = (15.0f32, 8.0f32, g + 7.5);
        let px = c.x;
        for side in [-1.0f32, 1.0] {
            b.cube(&id, vec3(px + side * (w * 0.5 + 0.3), g + 5.75, PANEL_Z), vec3(0.5, 11.5, 0.5), rgb(0.25, 0.27, 0.3));
        }
        b.cube(&id, vec3(px, y0, PANEL_Z - 0.1), vec3(w, h, 0.25), dark);
        for (cc, sz) in [(vec3(0.0, h * 0.5, 0.0), vec3(w + 0.4, 0.18, 0.35)), (vec3(0.0, -h * 0.5, 0.0), vec3(w + 0.4, 0.18, 0.35)), (vec3(0.0, 2.75, 0.0), vec3(w, 0.08, 0.3))] {
            b.glow(&id, vec3(px, y0, PANEL_Z) + cc, sz, hsv(0.8 + (time * 0.1).sin() * 0.05, 0.7, k));
        }
        for (x, z) in PADS {
            let p = vec3(x, g, z);
            trans.glow(&id, p + Vec3::Y * 0.05, vec3(3.0, 0.1, 3.0), Color::new(0.7, 0.4, 1.0, 0.25 + 0.2 * k));
            let r = (time * 0.8 + x * 0.1).fract();
            trans.glow(&id, p + Vec3::Y * (r * 4.0), vec3(2.6, 0.06, 2.6), Color::new(0.8, 0.5, 1.0, 0.5 * (1.0 - r)));
        }
        // Pedestais com itens/blocos dos mods (lado leste)
        let shows: Vec<(&Show, &str)> = self.defs.iter().flat_map(|d| d.shows.iter().map(move |s| (s, d.name.as_str()))).take(6).collect();
        for (i, (sh, from)) in shows.iter().enumerate() {
            let p = vec3(ZX1 as f32 - 0.5, g, ZZ0 as f32 + 4.5 + i as f32 * 2.3);
            b.cube(&id, p + Vec3::Y * 0.5, vec3(0.9, 1.0, 0.9), rgb(0.3, 0.3, 0.34));
            b.glow(&id, p + Vec3::Y * 1.02, vec3(0.95, 0.04, 0.95), rgb(0.7, 0.5, 1.0));
            let sz = if sh.block { 0.8 } else { 0.45 };
            let m = Mat4::from_translation(p + Vec3::Y * (1.7 + (time * 1.5 + i as f32).sin() * 0.1)) * Mat4::from_rotation_y(time * 0.8 + i as f32);
            pattern_cube(b, &m, sz, sh);
            if p.distance(eye) < 14.0 {
                labels.push(Label { pos: p + Vec3::Y * 2.6, text: format!("{} ({}: {})", sh.name, if sh.block { "bloco" } else { "item" }, from), size: 14.0, color: WHITE });
            }
        }
        if eye.z < PANEL_Z || eye.distance(vec3(px, y0, PANEL_Z)) > 75.0 {
            return;
        }
        let mut put = |y: f32, text: String, size: f32, color: Color| labels.push(Label { pos: vec3(px, y0 + y, PANEL_Z + 0.2), text, size, color });
        put(3.35, "MODDING AO VIVO - ZONA DE MODS".into(), 30.0, rgb(1.0, 0.45, 0.95));
        put(2.3, "MODDING: mande teu agent ler".into(), 20.0, WHITE);
        put(1.5, format!("{SITE}/modding.txt"), 24.0, rgb(0.5, 1.0, 1.0));
        put(0.8, "(Claude, Cursor, GPT... qualquer IA com HTTP. mod = so dados, nunca codigo)".into(), 14.0, rgb(0.75, 0.75, 0.85));
        put(0.05, format!("MODS ATIVOS ({}):", self.defs.len()), 18.0, rgb(1.0, 0.85, 0.3));
        if self.defs.is_empty() {
            put(-0.7, "nenhum mod ativo - teu agent pode ser o primeiro".into(), 16.0, WHITE);
        }
        for (i, d) in self.defs.iter().take(8).enumerate() {
            let alive = d.ents.iter().filter(|e| e.alive).count();
            put(-0.7 - i as f32 * 0.55, format!("{} v{} - por {}  [{}/{} vivos]", d.name, d.version, d.creator, alive, d.ents.len()), 16.0, if get_time() - d.born < 20.0 { rgb(0.5, 1.0, 0.5) } else { WHITE });
        }
    }
}

/// Decisão do host pra uma entidade viva: movimento, pulo, ataques, rugido, falas.
#[allow(clippy::too_many_arguments)]
fn think(speed: f32, b: &Behav, zone: bool, max_hp: f32, world: &World, dt: f32, time: f32, e: &mut Ent, hp: f32, players: &[Vec3]) {
    let flat = |v: Vec3| vec2(v.x, v.z);
    let (target, dist) = players.iter().map(|p| (*p, flat(*p - e.pos).length())).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((e.pos, f32::MAX));
    let leash = if zone { b.wander.map_or(6.0, |w| w.0).max(8.0) + 4.0 } else { 30.0 };
    let mut dir = Vec2::ZERO;
    let mut face: Option<Vec2> = None;
    let mut spd = speed;
    if b.flee.is_some_and(|f| hp / max_hp < f.1 && dist < f.0) {
        dir = flat(e.pos - target).normalize_or_zero();
        spd *= 1.3;
    } else if let Some((_, stop)) = b.follow.filter(|f| dist < f.0) {
        let to = flat(target - e.pos);
        face = Some(to);
        if dist > stop && flat(e.pos - e.home).length() < leash {
            dir = to.normalize_or_zero();
        }
    } else if let Some((radius, pause)) = b.wander {
        if e.wait > 0.0 {
            e.wait -= dt;
        } else {
            let goal = *e.goal.get_or_insert_with(|| e.home + vec3(gen_range(-radius, radius), 0.0, gen_range(-radius, radius)));
            if flat(goal - e.pos).length() < 0.6 {
                e.goal = None;
                e.wait = pause;
            } else {
                dir = flat(goal - e.pos).normalize_or_zero();
            }
        }
    }
    if flat(e.pos - e.home).length() > leash + 4.0 {
        dir = flat(e.home - e.pos).normalize_or_zero();
    }
    let fly = b.fly;
    let ground = world.floor_at(e.pos.x, e.pos.y + 0.6, e.pos.z);
    let on_ground = fly.is_none() && e.pos.y <= ground + 0.01 && e.vy <= 0.0;
    if dir != Vec2::ZERO && spd > 0.0 {
        let step = dir * spd * dt;
        let (nx, nz) = (e.pos.x + step.x, e.pos.z + step.y);
        if fly.is_some() || !world.solid_f(nx, e.pos.y + 0.5, nz) {
            e.pos.x = nx.clamp(1.0, WX as f32 - 1.0);
            e.pos.z = nz.clamp(1.0, WZ as f32 - 1.0);
        } else if on_ground {
            e.vy = 7.0;
            e.goal = None;
        }
    }
    e.moving = dir != Vec2::ZERO && spd > 0.0;
    if let Some(f) = face.or((dir != Vec2::ZERO).then_some(dir)) {
        let want = f.x.atan2(f.y);
        let mut dy = (want - e.yaw).rem_euclid(TAU);
        if dy > std::f32::consts::PI {
            dy -= TAU;
        }
        e.yaw += dy * (dt * 6.0).min(1.0);
    }
    if let Some((height, bob)) = fly {
        let top = world.floor_at(e.pos.x, (WY - 1) as f32, e.pos.z);
        let want = top + height + (time * 1.3).sin() * bob;
        e.pos.y += (want - e.pos.y) * (dt * 2.0).min(1.0);
        e.air = true;
    } else {
        e.t_jump -= dt;
        if let Some((every, h)) = b.jump {
            if e.t_jump <= 0.0 && on_ground {
                e.vy = (2.0 * GRAV * h).sqrt();
                e.t_jump = every;
            }
        }
        e.vy -= GRAV * dt;
        e.pos.y += e.vy * dt;
        let gnd = world.floor_at(e.pos.x, e.pos.y + 0.6, e.pos.z);
        if e.pos.y <= gnd {
            e.pos.y = gnd;
            e.vy = 0.0;
        }
        e.air = e.pos.y > gnd + 0.05;
    }
    e.cd_melee -= dt;
    e.cd_beam -= dt;
    if let Some((_, range, cd)) = b.melee {
        if dist < range && e.cd_melee <= 0.0 {
            e.an += 1;
            e.cd_melee = cd;
        }
    }
    if let Some((_, range, cd, _)) = b.beam {
        if dist < range && dist > 1.5 && e.cd_beam <= 0.0 {
            e.bn += 1;
            e.bt = target + Vec3::Y;
            e.cd_beam = cd;
        }
    }
    if let Some((every, _)) = b.roar {
        e.t_roar -= dt;
        if e.t_roar <= 0.0 {
            e.rn += 1;
            e.t_roar = every;
        }
    }
    if let Some((every, ph)) = &b.say {
        e.t_say -= dt;
        if e.t_say <= 0.0 && !ph.is_empty() {
            e.si = gen_range(0, ph.len());
            e.sn += 1;
            e.t_say = *every;
        }
    }
}

fn draw_ent(b: &mut Batch, trans: &mut Batch, d: &Def, e: &Ent, life: Option<&Vida>, time: f32) {
    let dead = life.filter(|v| !v.alive());
    let flash = life.map_or(0.0, |v| v.flash);
    let pick = |i: usize| d.anims[i].as_ref();
    let act_dur = |i: usize| pick(i).map_or(0.6, |a| a.dur);
    let (anim, t) = if let Some(v) = dead {
        (pick(3), v.t)
    } else if e.act == 1 && e.act_t < act_dur(2) {
        (pick(2), e.act_t)
    } else if e.act == 2 && e.act_t < act_dur(4) && pick(4).is_some() {
        (pick(4), e.act_t)
    } else if e.air && d.b.fly.is_some() && pick(6).is_some() {
        (pick(6), e.anim_t)
    } else if e.air && pick(5).is_some() {
        (pick(5), e.anim_t)
    } else if e.moving {
        (pick(1), e.anim_t)
    } else {
        (pick(0), e.anim_t)
    };
    let n = d.parts.len();
    let poses = anim.map_or_else(|| vec![(Vec3::ZERO, Vec3::ZERO); n], |a| sample(a, t, n));
    let tip = if dead.is_some() && pick(3).is_none() { dead.map_or(0.0, |v| v.lean()) } else { 0.0 };
    let root = Mat4::from_translation(e.pos) * Mat4::from_rotation_y(e.yaw) * Mat4::from_rotation_x(-tip) * Mat4::from_scale(Vec3::splat(d.scale));
    let mut mats: Vec<Mat4> = Vec::with_capacity(n);
    for (i, p) in d.parts.iter().enumerate() {
        let (rot, off) = poses[i];
        let parent = p.parent.and_then(|j| mats.get(j).copied()).unwrap_or(root);
        let r = Mat4::from_rotation_y(rot.y) * Mat4::from_rotation_x(rot.x) * Mat4::from_rotation_z(rot.z);
        let m = parent * Mat4::from_translation(p.pivot + off) * r * Mat4::from_translation(-p.pivot);
        for c in &p.boxes {
            let col = Color::new(c.col.r + (1.0 - c.col.r) * flash, c.col.g + (1.0 - c.col.g) * flash, c.col.b + (1.0 - c.col.b) * flash, 1.0);
            if c.glow && dead.is_none() {
                b.glow(&m, c.pos, c.size, col);
            } else {
                b.cube(&m, c.pos, c.size, col);
            }
        }
        mats.push(m);
    }
    if e.beam_t > 0.0 && dead.is_none() {
        if let Some((.., col)) = d.b.beam {
            let from = e.pos + Vec3::Y * d.height * 0.8;
            let k = 0.6 + 0.4 * (time * 40.0).sin();
            beam(trans, from, e.bt, 0.35 * k, Color::new(col.r, col.g, col.b, 0.5));
            beam(trans, from, e.bt, 0.1, Color::new(1.0, 1.0, 1.0, 0.9));
            trans.glow(&Mat4::from_translation(e.bt), Vec3::ZERO, Vec3::splat(0.9 * k), Color::new(col.r, col.g, col.b, 0.6));
        }
    }
}
