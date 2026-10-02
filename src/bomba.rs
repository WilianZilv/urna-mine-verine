//! BOMBADINHO, o operário da demolição da vila (paródia original do arquétipo "bombardeiro de labirinto"):
//! capacete laranja de obra com pavio aceso no topo, óculos de solda, bigodão, colete refletivo e cinto de
//! bombinhas. Bomba com a física clássica de grade: gruda na casa do voxel, pavio de 3s, chama em cruz nos 4
//! eixos horizontais com alcance N; a chama para antes de bloco duro, quebra o PRIMEIRO bloco mole e para
//! ali (perfurante atravessa), chama em outra bomba detona na hora (cada bomba explode uma vez só, teto por
//! frame), chama fica 0,5s machucando quem estiver nela, bomba vira obstáculo depois que tu sai de cima, chute.
//! O host simula bombas e o NPC; a explosão vai como evento de mundo "w" k:"bomb" (by: BY) com os braços já
//! medidos: aplicar/replay do log só quebra blocos moles dentro dos braços, nunca explode nada de novo.
//! Jogável como BOMBADINHO (C, 8).

use crate::actors::{Atk, Ev, FState, Fighter, angle_lerp, try_move};
use crate::batch::Batch;
use crate::extras::Label;
use crate::gta::Target;
use crate::models::{U, rgb};
use crate::npc::{self, Npcs, Vida};
use crate::player::Player;
use crate::tnt::{self, Tnt};
use crate::urna::{Fx, Particle};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::FRAC_PI_2;

pub const BY: u64 = 12;
pub const FUSE: f32 = 3.0;
/// Quanto a chama fica no chão machucando.
pub const FLAME: f32 = 0.5;
/// Remota esquecida explode sozinha.
const REMOTE_MAX: f32 = 30.0;
pub const MAX_BOMBS: usize = 48;
const PER_OWNER: usize = 8;
/// Teto de explosões por frame (o resto da corrente sai no frame seguinte).
pub const BOOMS_PER_FRAME: usize = 12;
const MAX_RANGE: u8 = 8;
const KICK: f32 = 9.0;
const NPC_OWNER: u64 = u32::MAX as u64;
const NPC_MAX: usize = 3;
const NPC_RANGE: u8 = 3;
const SPEED: f32 = 4.2;
const MAX_HP: f32 = 100.0;

pub const NORMAL: u8 = 0;
pub const FOGO: u8 = 1;
pub const PERFURA: u8 = 2;
pub const REMOTA: u8 = 3;
/// Modo de colocar: solta todas as bombas que sobraram em fila pra frente (cada uma NORMAL).
pub const LINHA: u8 = 4;
pub const GOSMA: u8 = 5;
pub const NAMES: [&str; 6] = ["NORMAL", "FOGO", "PERFURANTE", "REMOTA", "LINHA", "GOSMA"];
const DIRS: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

const IDLE: u8 = 0;
const WALK: u8 = 1;
const PLACE: u8 = 2;
const HURT: u8 = 3;
const DEAD: u8 = 4;

const FALAS: [&str; 6] = ["PAVIO CURTO, MANDATO CURTO", "QUEM PLANTA BOMBA COLHE CRATERA", "OBRA PARADA? EU DESTRAVO!", "3... 2... 1... CONFIRMA!", "SAI DA LINHA QUE A CHAMA E RETA", "VOTO EXPLOSIVO, APURACAO RAPIDA"];

// ---------------------------------------------------------------- Física da explosão (pura, testada)

fn hard_block(b: u8) -> bool {
    matches!(b, BEDROCK | STONE | COBBLE | BRICK | CLUBWALL | crate::world::BLACK | NEON)
}

/// Espelho de server/layout.js (LANDMARKS + PROTECTED), [x0, x1, z0, z1]: bomba não quebra bloco ali.
const RESERVED: [(i32, i32, i32, i32); 31] = [
    (44, 76, 142, 178),
    (235, 270, 144, 177),
    (243, 263, 109, 129),
    (239, 273, 188, 209),
    (107, 111, 44, 72),
    (48, 88, 78, 118),
    (232, 272, 68, 104),
    (42, 86, 264, 294),
    (182, 216, 220, 254),
    (182, 222, 264, 296),
    (98, 132, 264, 294),
    (157, 163, 91, 138),
    (157, 163, 182, 264),
    (74, 138, 157, 163),
    (182, 244, 157, 163),
    (43, 158, 237, 243),
    (162, 228, 256, 262),
    (86, 234, 95, 101),
    (238, 242, 118, 158),
    (241, 246, 117, 121),
    (240, 243, 162, 191),
    (60, 64, 242, 266),
    (162, 184, 235, 239),
    (198, 202, 261, 266),
    (113, 117, 242, 268),
    (156, 163, 262, 269),
    (226, 274, 242, 282),
    (112, 208, 24, 92),
    (120, 200, 131, 133),
    (108, 116, 146, 154),
    (132, 140, 132, 140),
];
const HOUSES: [(i32, i32); 11] = [(50, 228), (68, 228), (86, 228), (104, 228), (122, 228), (50, 246), (68, 246), (86, 246), (104, 246), (122, 246), (166, 250)];
const BILLBOARDS: [(i32, i32); 3] = [(184, 136), (136, 184), (184, 184)];

fn reserved(x: i32, z: i32) -> bool {
    RESERVED.iter().any(|&(x0, x1, z0, z1)| x >= x0 && x <= x1 && z >= z0 && z <= z1)
        || HOUSES.iter().any(|&(hx, hz)| x >= hx - 1 && x <= hx + 7 && z >= hz - 1 && z <= hz + 7)
        || BILLBOARDS.iter().any(|&(bx, bz)| (x - bx).abs() <= 4 && (z - bz).abs() <= 4)
}

/// Escudo do clube ou domo do lab/hub: chama não entra, bomba não nasce.
fn in_shield(p: IVec3) -> bool {
    crate::shield::protected(p) || (p.as_vec3() + Vec3::splat(0.5)).distance(shield_center()) < SHIELD_R + 0.5
}

/// A chama não passa: borda do mundo, escudo, bloco duro ou bloco de área protegida.
pub fn hard(world: &World, p: IVec3) -> bool {
    if p.y <= 0 || p.y >= WY || p.x < 0 || p.z < 0 || p.x >= WX || p.z >= WZ || in_shield(p) {
        return true;
    }
    world.solid(p.x, p.y, p.z) && (hard_block(world.get(p.x, p.y, p.z)) || reserved(p.x, p.z))
}

fn soft(world: &World, p: IVec3) -> bool {
    world.solid(p.x, p.y, p.z) && !hard(world, p)
}

pub struct Blast {
    /// Casas tomadas em +x, -x, +z, -z.
    pub arms: [u8; 4],
    pub broken: Vec<IVec3>,
    pub bombs: Vec<u64>,
}

/// Mede a cruz: cada braço anda até `range`; para antes do bloco duro, para EM CIMA do primeiro bloco mole
/// (que quebra) ou da primeira bomba (que detona). Perfurante atravessa bloco mole quebrando todos.
pub fn propagate(world: &World, c: IVec3, range: u8, pierce: bool, bomb_at: impl Fn(IVec3) -> Option<u64>) -> Blast {
    let mut out = Blast { arms: [0; 4], broken: Vec::new(), bombs: Vec::new() };
    for (k, d) in DIRS.iter().enumerate() {
        for s in 1..=range.min(MAX_RANGE) as i32 {
            let p = c + *d * s;
            if hard(world, p) {
                break;
            }
            out.arms[k] = s as u8;
            if let Some(id) = bomb_at(p) {
                out.bombs.push(id);
                break;
            }
            if world.solid(p.x, p.y, p.z) {
                out.broken.push(p);
                if !pierce {
                    break;
                }
            }
        }
    }
    out
}

/// Casas da chama: centro + braços.
pub fn cells(c: IVec3, arms: [u8; 4]) -> impl Iterator<Item = IVec3> {
    std::iter::once(c).chain(DIRS.into_iter().zip(arms).flat_map(move |(d, n)| (1..=n as i32).map(move |s| c + d * s)))
}

fn parse(m: &Value) -> Option<(IVec3, [u8; 4])> {
    let c = &m["c"];
    let c = ivec3(c[0].as_i64()? as i32, c[1].as_i64()? as i32, c[2].as_i64()? as i32);
    let mut arms = [0u8; 4];
    for (k, v) in m["a"].as_array()?.iter().take(4).enumerate() {
        arms[k] = v.as_u64().unwrap_or(0).min(MAX_RANGE as u64) as u8;
    }
    Some((c, arms))
}

/// Evento "w" k:"bomb" (todos, ao vivo e no replay do log): quebra o que é mole dentro dos braços medidos pelo
/// host. Determinístico com o mundo igual; nunca acende nem explode nada.
pub fn apply(world: &mut World, m: &Value, fx: &mut Fx, avg: &[Color]) {
    let Some((c, arms)) = parse(m) else { return };
    for (d, n) in DIRS.into_iter().zip(arms) {
        for s in 1..=n as i32 {
            let p = c + d * s;
            if hard(world, p) {
                break;
            }
            if !soft(world, p) {
                continue;
            }
            let b = world.get(p.x, p.y, p.z);
            world.set(p.x, p.y, p.z, AIR);
            if gen_range(0.0, 1.0) < 0.4 {
                let bc = p.as_vec3() + Vec3::splat(0.5);
                fx.particles.push(Particle { pos: bc, vel: d.as_vec3() * gen_range(2.0, 6.0) + vec3(0.0, gen_range(3.0, 7.0), 0.0), col: avg[face_tile(b, 0)], life: gen_range(0.8, 1.6), size: gen_range(0.15, 0.3), gravity: true });
            }
        }
    }
}

// ---------------------------------------------------------------- Bombas (host simula; clientes seguem o snapshot)

/// Corpo que leva gosma ou segura bomba chutada: (0 lutador [índice] | 1 jogador [id] | 2 BOMBADINHO, chave).
type Key = (u8, u64);

pub struct Bomb {
    pub id: u64,
    pub owner: u64,
    pub kind: u8,
    pub range: u8,
    pub cell: IVec3,
    pub pos: Vec3,
    pub t: f32,
    np: Vec3,
    slide: IVec3,
    stuck: Option<Key>,
    age: f32,
}

pub struct Flame {
    pub cells: Vec<IVec3>,
    pub t: f32,
    owner: u64,
    hit: Vec<(u8, usize)>,
    me: bool,
    fresh: bool,
}

#[derive(Default)]
pub struct Bombs {
    pub list: Vec<Bomb>,
    pub flames: Vec<Flame>,
    /// Host: ids que já explodiram (nunca voltam).
    done: HashSet<u64>,
    /// Ids de explosão já aplicados (eco repetido não aplica de novo).
    heard: HashSet<u64>,
    /// Host: corrente pendente (detona no próximo passo).
    fire: VecDeque<u64>,
    pub booms: usize,
}

fn center(c: IVec3) -> Vec3 {
    c.as_vec3() + vec3(0.5, 0.0, 0.5)
}

fn cell_of(p: Vec3) -> IVec3 {
    ivec3(p.x.floor() as i32, (p.y + 0.05).floor() as i32, p.z.floor() as i32)
}

impl Bombs {
    pub fn at(&self, p: IVec3) -> Option<u64> {
        self.list.iter().find(|b| b.cell == p).map(|b| b.id)
    }

    /// Host: põe bomba validada (casa livre, fora de escudo, tetos). FOGO ganha +3 de alcance.
    pub fn place(&mut self, world: &World, id: u64, owner: u64, kind: u8, range: u8, cell: IVec3) -> bool {
        let busy = self.list.iter().any(|b| b.id == id || b.cell == cell);
        let mine = self.list.iter().filter(|b| b.owner == owner).count();
        if kind > GOSMA || kind == LINHA || busy || mine >= PER_OWNER || self.list.len() >= MAX_BOMBS || self.done.contains(&id) || hard(world, cell) || world.solid(cell.x, cell.y, cell.z) {
            return false;
        }
        let range = (range.clamp(1, MAX_RANGE) + if kind == FOGO { 3 } else { 0 }).min(MAX_RANGE);
        let pos = center(cell);
        self.list.push(Bomb { id, owner, kind, range, cell, pos, t: if kind == REMOTA { REMOTE_MAX } else { FUSE }, np: pos, slide: IVec3::ZERO, stuck: None, age: 0.0 });
        true
    }

    pub fn trigger(&mut self, id: u64) {
        if !self.fire.contains(&id) {
            self.fire.push_back(id);
        }
    }

    pub fn detonate_remote(&mut self, owner: u64) {
        let ids: Vec<u64> = self.list.iter().filter(|b| b.owner == owner && b.kind == REMOTA).map(|b| b.id).collect();
        for id in ids {
            self.trigger(id);
        }
    }

    /// Chute: desliza na grade até bater em bloco, bomba ou alguém.
    pub fn kick(&mut self, id: u64, d: IVec3) -> bool {
        let Some(b) = self.list.iter_mut().find(|b| b.id == id) else { return false };
        if b.stuck.is_some() || b.slide != IVec3::ZERO || d.y != 0 || d.x.abs() + d.z.abs() != 1 {
            return false;
        }
        b.slide = d;
        true
    }

    /// Explosão esférica (urna, TNT, carro) acende as bombas no raio.
    pub fn blast(&mut self, c: Vec3, r: f32) {
        for b in self.list.iter_mut() {
            if (b.pos + Vec3::Y * 0.4).distance(c) < r + 0.5 {
                b.t = b.t.min(0.15);
            }
        }
    }

    /// Host: pavio, chute, gosma, queda e corrente. Devolve até BOOMS_PER_FRAME eventos de explosão.
    pub fn update(&mut self, world: &World, dt: f32, bodies: &[(Key, Vec3)]) -> Vec<Value> {
        let occupied: Vec<(u64, IVec3)> = self.list.iter().map(|b| (b.id, b.cell)).collect();
        for b in self.list.iter_mut() {
            b.age += dt;
            b.t -= dt;
            let owner_key = if b.owner == NPC_OWNER { (2, 0) } else { (1, b.owner) };
            if let Some(k) = b.stuck {
                match bodies.iter().find(|x| x.0 == k) {
                    Some(&(_, p)) => {
                        b.pos = p + Vec3::Y * 1.1;
                        b.cell = cell_of(p);
                    }
                    None => b.stuck = None,
                }
                b.np = b.pos;
                continue;
            }
            if b.slide != IVec3::ZERO {
                let d = b.slide.as_vec3();
                b.pos += d * KICK * dt;
                let cur = ivec3(b.pos.x.floor() as i32, b.cell.y, b.pos.z.floor() as i32);
                if cur != b.cell && !hard(world, cur) && !world.solid(cur.x, cur.y, cur.z) {
                    b.cell = cur;
                }
                if (b.pos - center(b.cell)).dot(d) >= 0.0 {
                    let next = b.cell + b.slide;
                    let who = bodies.iter().find(|x| x.0 != owner_key && vec2(x.1.x - next.x as f32 - 0.5, x.1.z - next.z as f32 - 0.5).length() < 0.7 && (x.1.y - next.y as f32).abs() < 1.5);
                    if hard(world, next) || world.solid(next.x, next.y, next.z) || occupied.iter().any(|o| o.0 != b.id && o.1 == next) || who.is_some() {
                        b.pos = center(b.cell);
                        b.slide = IVec3::ZERO;
                        if let (GOSMA, Some(w)) = (b.kind, who) {
                            b.stuck = Some(w.0);
                        }
                    }
                }
            } else {
                if !world.solid(b.cell.x, b.cell.y - 1, b.cell.z) && b.cell.y > 1 {
                    b.cell.y -= 1;
                } else if world.solid(b.cell.x, b.cell.y, b.cell.z) && b.cell.y < WY - 2 {
                    b.cell.y += 1;
                }
                let c = center(b.cell);
                b.pos = vec3(c.x, b.pos.y + (c.y - b.pos.y).clamp(-12.0 * dt, 12.0 * dt), c.z);
                if b.kind == GOSMA && b.age > 0.6 {
                    if let Some(w) = bodies.iter().find(|x| x.0 != owner_key && vec2(x.1.x - b.pos.x, x.1.z - b.pos.z).length() < 0.75 && (x.1.y - b.pos.y).abs() < 1.5) {
                        b.stuck = Some(w.0);
                    }
                }
            }
            b.np = b.pos;
        }
        for b in &self.list {
            if b.t <= 0.0 && !self.fire.contains(&b.id) {
                self.fire.push_back(b.id);
            }
        }
        let mut out = Vec::new();
        while out.len() < BOOMS_PER_FRAME {
            let Some(id) = self.fire.pop_front() else { break };
            let Some(k) = self.list.iter().position(|b| b.id == id) else { continue };
            let b = self.list.swap_remove(k);
            if !self.done.insert(b.id) {
                continue;
            }
            let blast = propagate(world, b.cell, b.range, b.kind == PERFURA, |p| self.list.iter().find(|o| o.cell == p).map(|o| o.id));
            for id in blast.bombs {
                self.trigger(id);
            }
            out.push(json!({"t": "w", "k": "bomb", "id": b.id, "c": [b.cell.x, b.cell.y, b.cell.z], "a": blast.arms, "ty": b.kind, "o": b.owner, "by": BY}));
        }
        if self.done.len() > 20000 {
            self.done.clear();
        }
        out
    }

    /// Antes de aplicar um "w" (todos): explosão repetida = false; acende TNT nos braços, tira a bomba e põe a
    /// chama. Reset limpa tudo.
    pub fn before_world(&mut self, world: &World, m: &Value, tnt: &mut Tnt) -> bool {
        match m["k"].as_str() {
            Some("bomb") => {}
            Some("reset") => {
                self.list.clear();
                self.flames.clear();
                self.fire.clear();
                return true;
            }
            _ => return true,
        }
        let id = m["id"].as_u64().unwrap_or(0);
        let Some((c, arms)) = parse(m).filter(|_| self.heard.insert(id)) else { return false };
        if self.heard.len() > 20000 {
            self.heard.clear();
        }
        self.list.retain(|b| b.id != id);
        let mut cs = vec![c];
        for (d, n) in DIRS.into_iter().zip(arms) {
            for s in 1..=n as i32 {
                let p = c + d * s;
                if hard(world, p) {
                    break;
                }
                if world.get(p.x, p.y, p.z) == TNT {
                    tnt.prime(world, p, tnt::id_at(id, p), 0.3 + 0.1 * s as f32);
                }
                cs.push(p);
            }
        }
        self.flames.push(Flame { cells: cs, t: FLAME, owner: m["o"].as_u64().unwrap_or(0), hit: Vec::new(), me: false, fresh: true });
        self.booms += 1;
        true
    }

    pub fn tick_flames(&mut self, dt: f32) {
        for f in self.flames.iter_mut() {
            f.t -= dt;
        }
        self.flames.retain(|f| f.t > 0.0);
    }
}

fn in_flame(cells: &[IVec3], c: Vec3, r: f32) -> bool {
    let h = r.min(0.6);
    cells.iter().any(|p| {
        let (x, y, z) = (p.x as f32, p.y as f32, p.z as f32);
        c.x > x - h && c.x < x + 1.0 + h && c.z > z - h && c.z < z + 1.0 + h && c.y - r < y + 1.6 && c.y + r > y - 0.2
    })
}

// ---------------------------------------------------------------- NPC + jogável

pub struct Npc {
    pub pos: Vec3,
    pub yaw: f32,
    pub spawned: bool,
    act: u8,
    t: f32,
    vy: f32,
    walk: f32,
    moving: bool,
    invuln: f32,
    target: Option<usize>,
    retarget: f32,
    cd: f32,
    path: Vec<IVec3>,
    replan: f32,
    last_hp: f32,
    net: Option<Vec3>,
}

/// Estado do jogador local quando ele é o BOMBADINHO.
pub struct Me {
    pub kind: u8,
    pub max: usize,
    pub range: u8,
    pub hp: f32,
    dead: f32,
    kick_cd: f32,
    flash: f32,
}

#[derive(Clone, Copy, Default)]
pub struct Intent {
    pub place: bool,
    pub cycle: i32,
    pub detonate: bool,
}

pub struct Bomba {
    pub bombs: Bombs,
    pub npc: Npc,
    pub me: Option<Me>,
    /// Bombas que o jogador local pôs e o host ainda não confirmou (previsão).
    pending: Vec<Bomb>,
    /// Bombas embaixo do jogador local (ainda dá pra sair de cima).
    on: Vec<u64>,
    seq: u64,
    aggro: Vec<f32>,
    swings: Vec<(usize, f32, f32)>,
    pub msg: Option<String>,
    /// Centros de explosão novos (o main toca o som).
    pub sounds: Vec<Vec3>,
}

impl Bomba {
    pub fn new() -> Self {
        let npc = Npc { pos: arena_center() + Vec3::Y * 14.0, yaw: 0.0, spawned: false, act: IDLE, t: 0.0, vy: 0.0, walk: 0.0, moving: false, invuln: 0.0, target: None, retarget: 0.0, cd: 2.0, path: Vec::new(), replan: 0.0, last_hp: 0.0, net: None };
        Bomba { bombs: Bombs::default(), npc, me: None, pending: Vec::new(), on: Vec::new(), seq: 0, aggro: Vec::new(), swings: Vec::new(), msg: None, sounds: Vec::new() }
    }

    pub fn start_me(&mut self) {
        self.me = Some(Me { kind: NORMAL, max: 3, range: 3, hp: MAX_HP, dead: 0.0, kick_cd: 0.0, flash: 0.0 });
    }

    pub fn visible(&self) -> bool {
        self.npc.spawned || self.npc.net.is_some()
    }

    /// Alvo pra jogador/bandido acertar (barrinha sobre a cabeça vem daqui).
    pub fn target(&self, npcs: &Npcs) -> Option<Target> {
        (self.visible() && npcs.alive(npc::BOMBA, 0)).then(|| (self.npc.pos + Vec3::Y * 0.8, 0.7, 0, npc::BOMBA))
    }

    /// Linha do ?debug=1 (sem aspas: vai cru no JSON do profiler).
    pub fn debug(&self) -> String {
        let n = &self.npc;
        format!("BOMBA npc={} act={} x={:.1} y={:.2} z={:.1} vy={:.1} tgt={:?} path={} cd={:.1} bombs={} flames={} booms={} me={} kind={}", self.visible() as u8, n.act, n.pos.x, n.pos.y, n.pos.z, n.vy, n.target, n.path.len(), n.cd, self.bombs.list.len(), self.bombs.flames.len(), self.bombs.booms, self.me.is_some() as u8, self.me.as_ref().map_or("-", |m| NAMES[m.kind as usize]))
    }

    /// Antes de aplicar um "w" (todos).
    pub fn before_world(&mut self, world: &World, m: &Value, tnt: &mut Tnt) -> bool {
        let fresh = self.bombs.before_world(world, m, tnt);
        if fresh && m["k"] == "bomb" {
            if let Some((c, _)) = parse(m) {
                self.sounds.push(center(c) + Vec3::Y * 0.5);
            }
        }
        fresh
    }

    /// Host: mensagens "a" de bomba dos jogadores.
    pub fn on_action(&mut self, world: &World, owner: u64, m: &Value) {
        match m["k"].as_str().unwrap_or("") {
            "bomb" => {
                let p = &m["p"];
                let cell = ivec3(p[0].as_i64().unwrap_or(-1) as i32, p[1].as_i64().unwrap_or(-1) as i32, p[2].as_i64().unwrap_or(-1) as i32);
                let (kind, range) = (m["ty"].as_u64().unwrap_or(0).min(255) as u8, m["n"].as_u64().unwrap_or(3).min(255) as u8);
                self.bombs.place(world, m["bid"].as_u64().unwrap_or(0), owner, kind, range, cell);
            }
            "bombx" => self.bombs.detonate_remote(owner),
            "kick" => {
                let d = &m["d"];
                self.bombs.kick(m["b"].as_u64().unwrap_or(0), ivec3(d[0].as_i64().unwrap_or(0) as i32, 0, d[1].as_i64().unwrap_or(0) as i32));
            }
            _ => {}
        }
    }

    fn fightable(&self) -> bool {
        self.npc.spawned && self.npc.act != DEAD && self.npc.invuln <= 0.0
    }

    fn spawn(&mut self, events: &mut Vec<Ev>, first: bool) {
        let p = crate::layout::arena_point(10.0);
        let n = &mut self.npc;
        (n.pos, n.vy, n.act, n.t, n.spawned, n.invuln, n.target, n.path, n.cd) = (vec3(p.x.floor() + 0.5, G as f32 + 14.0, p.y.floor() + 0.5), 0.0, IDLE, 0.0, true, 2.0, None, Vec::new(), 2.0);
        events.push(Ev::Banner(if first { "O BOMBADINHO CHEGOU NA ARENA! SAI DA LINHA DA CHAMA".into() } else { "O BOMBADINHO VOLTOU COM PAVIO NOVO!".into() }));
    }

    fn die(&mut self) {
        (self.npc.act, self.npc.t, self.npc.path) = (DEAD, 0.0, Vec::new());
    }

    /// Host: dano no BOMBADINHO pelos lutadores (morre pelo sistema de vida dos NPCs).
    fn take_hit(&mut self, npcs: &mut Npcs, dmg: f32, who: &str, kos: &mut u32, events: &mut Vec<Ev>) {
        if !self.fightable() {
            return;
        }
        let p = self.npc.pos;
        events.push(Ev::Hit { pos: p + Vec3::Y * 0.9, claws: false });
        events.push(Ev::Text { pos: p + Vec3::Y * 2.0, text: format!("-{}", dmg.round() as i32), color: WHITE, big: false });
        if npcs.hit(npc::BOMBA, 0, dmg) == Some(true) {
            *kos += 1;
            self.die();
            events.push(Ev::Banner(format!("{who} DESARMOU O BOMBADINHO! VOLTA EM 40s")));
        } else {
            (self.npc.act, self.npc.t) = (HURT, 0.0);
        }
        self.npc.last_hp = npcs.get(npc::BOMBA, 0).map_or(0.0, |v| v.hp);
    }

    /// Casas (xz) que vão pegar fogo: braços de toda bomba viva (sem corrente) + chamas no chão.
    fn danger(&self, world: &World, extra: &[(IVec3, u8, bool)]) -> HashSet<IVec2> {
        let mut out = HashSet::new();
        let live = self.bombs.list.iter().map(|b| (b.cell, b.range, b.kind == PERFURA));
        for (c, r, pierce) in live.chain(extra.iter().copied()) {
            for p in cells(c, propagate(world, c, r, pierce, |_| None).arms) {
                out.insert(ivec2(p.x, p.z));
            }
        }
        for f in &self.bombs.flames {
            out.extend(f.cells.iter().map(|p| ivec2(p.x, p.z)));
        }
        out
    }

    /// Vizinho andável (sobe 1, desce até 3, cabe a cabeça, sem bomba, dentro da arena).
    fn step(&self, world: &World, c: IVec3, d: IVec3) -> Option<IVec3> {
        let (nx, nz) = (c.x + d.x, c.z + d.z);
        if !crate::layout::in_arena(vec2(nx as f32 + 0.5, nz as f32 + 0.5), 1.0) {
            return None;
        }
        let ny = world.floor_at(nx as f32 + 0.5, c.y as f32 + 1.05, nz as f32 + 0.5) as i32;
        let n = ivec3(nx, ny, nz);
        let ok = ny - c.y <= 1 && c.y - ny <= 3 && !world.solid(nx, ny, nz) && !world.solid(nx, ny + 1, nz) && !self.bombs.list.iter().any(|b| b.cell.x == nx && b.cell.z == nz);
        ok.then_some(n)
    }

    /// Fuga: BFS até a casa segura mais perto (caminho sem a casa de partida).
    fn flee(&self, world: &World, from: IVec3, danger: &HashSet<IVec2>, max: usize) -> Option<Vec<IVec3>> {
        let mut prev: HashMap<IVec2, IVec3> = HashMap::new();
        let mut seen = HashSet::from([ivec2(from.x, from.z)]);
        let mut q = VecDeque::from([(from, 0usize)]);
        while let Some((c, n)) = q.pop_front() {
            if !danger.contains(&ivec2(c.x, c.z)) {
                let mut path = vec![c];
                while let Some(&p) = prev.get(&ivec2(path[path.len() - 1].x, path[path.len() - 1].z)).filter(|p| **p != from) {
                    path.push(p);
                }
                path.reverse();
                return Some(path);
            }
            if n >= max || seen.len() > 400 {
                continue;
            }
            for d in DIRS {
                if let Some(nc) = self.step(world, c, d).filter(|nc| seen.insert(ivec2(nc.x, nc.z))) {
                    prev.insert(ivec2(nc.x, nc.z), c);
                    q.push_back((nc, n + 1));
                }
            }
        }
        None
    }

    fn move_to(&mut self, world: &World, to: Vec3, speed: f32, dt: f32) -> bool {
        let n = &self.npc;
        let d = vec3(to.x - n.pos.x, 0.0, to.z - n.pos.z);
        let len = d.length();
        if len < 0.05 {
            return true;
        }
        let mut p = n.pos;
        if !try_move(world, &mut p, d / len * (speed * dt).min(len)) {
            return false;
        }
        let (old, nc) = (cell_of(n.pos), cell_of(p));
        if (old.x, old.z) != (nc.x, nc.z) && self.bombs.list.iter().any(|b| b.cell.x == nc.x && b.cell.z == nc.z) {
            return false;
        }
        let n = &mut self.npc;
        n.pos = p;
        n.yaw = angle_lerp(n.yaw, d.x.atan2(d.z), dt * 12.0);
        n.walk += speed * dt * 2.6;
        n.moving = true;
        true
    }

    fn new_id(&mut self, owner: u64) -> u64 {
        self.seq += 1;
        if owner == NPC_OWNER { (1u64 << 52) + self.seq } else { ((owner & 0xF_FFFF) << 32) | (self.seq & 0xFFFF_FFFF) }
    }

    /// IA: mira em lutador alinhado, põe bomba só se tiver rota de fuga, foge da linha da chama, chuta bomba
    /// na direção do alvo e detona a remota quando alguém entra na cruz.
    fn ai(&mut self, world: &World, dt: f32, fighters: &[Fighter]) {
        let me = cell_of(self.npc.pos);
        let danger = self.danger(world, &[]);
        let n = &mut self.npc;
        n.cd -= dt;
        n.retarget -= dt;
        n.replan -= dt;
        let valid = n.target.is_some_and(|j| fighters.get(j).is_some_and(|f| f.active()));
        if !valid || n.retarget <= 0.0 {
            n.retarget = gen_range(3.0, 6.0);
            n.target = fighters.iter().enumerate().filter(|(_, f)| f.active()).map(|(j, f)| (j, f.pos.distance(n.pos) * gen_range(0.7, 1.4))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
        }
        let hot = danger.contains(&ivec2(me.x, me.z));
        if hot && (self.npc.path.is_empty() || self.npc.replan <= 0.0 || self.npc.path.last().is_some_and(|g| danger.contains(&ivec2(g.x, g.z)))) {
            self.npc.path = self.flee(world, me, &danger, 10).unwrap_or_default();
            self.npc.replan = 0.4;
        }
        let own: Vec<(u64, IVec3, u8)> = self.bombs.list.iter().filter(|b| b.owner == NPC_OWNER && b.kind == REMOTA).map(|b| (b.id, b.cell, b.range)).collect();
        for (id, c, r) in own {
            let cross: HashSet<IVec2> = cells(c, propagate(world, c, r, false, |_| None).arms).map(|p| ivec2(p.x, p.z)).collect();
            let caught = fighters.iter().any(|f| f.active() && cross.contains(&ivec2(f.pos.x.floor() as i32, f.pos.z.floor() as i32)));
            if caught && !cross.contains(&ivec2(me.x, me.z)) {
                self.bombs.trigger(id);
            }
        }
        let Some(j) = self.npc.target.filter(|_| !hot && self.npc.path.is_empty()) else { return };
        let tp = fighters[j].pos;
        let tc = cell_of(tp);
        let (dx, dz) = (tc.x - me.x, tc.z - me.z);
        let dist = vec2(tp.x - self.npc.pos.x, tp.z - self.npc.pos.z).length();
        let aligned = (dx == 0 || dz == 0) && (dx.abs() + dz.abs()) as u8 <= NPC_RANGE + 1 && (tc.y - me.y).abs() <= 1;
        let axis = if dx.abs() >= dz.abs() { ivec3(dx.signum(), 0, 0) } else { ivec3(0, 0, dz.signum()) };
        let mine = self.bombs.list.iter().filter(|b| b.owner == NPC_OWNER).count();
        if self.npc.cd <= 0.0 && (dx == 0 || dz == 0) && dx.abs() + dz.abs() >= 3 {
            if let Some(id) = self.bombs.at(me + axis) {
                if self.bombs.kick(id, axis) {
                    self.npc.cd = gen_range(0.8, 1.4);
                    (self.npc.act, self.npc.t) = (PLACE, 0.0);
                    return;
                }
            }
        }
        if self.npc.cd <= 0.0 && mine < NPC_MAX && (aligned || dist < 2.2) && self.bombs.at(me).is_none() && world.solid(me.x, me.y - 1, me.z) {
            let r = gen_range(0.0, 1.0);
            let kind = if dist < 1.6 && r < 0.25 {
                GOSMA
            } else if aligned && dx.abs() + dz.abs() >= 3 && mine == 0 && r < 0.45 {
                LINHA
            } else if r < 0.6 {
                FOGO
            } else if r < 0.72 {
                PERFURA
            } else if r < 0.86 {
                REMOTA
            } else {
                NORMAL
            };
            let mut plan = vec![me];
            if kind == LINHA {
                for k in 1..(NPC_MAX - mine) as i32 {
                    let p = me + axis * k;
                    if world.solid(p.x, p.y, p.z) || !world.solid(p.x, p.y - 1, p.z) || self.bombs.at(p).is_some() {
                        break;
                    }
                    plan.push(p);
                }
            }
            let (bk, range) = if kind == LINHA { (NORMAL, NPC_RANGE) } else { (kind, NPC_RANGE + if kind == FOGO { 3 } else { 0 }) };
            let extra: Vec<(IVec3, u8, bool)> = plan.iter().map(|&p| (p, range, bk == PERFURA)).collect();
            let danger2 = self.danger(world, &extra);
            if let Some(path) = self.flee(world, me, &danger2, 7).filter(|p| p.len() <= 7) {
                for p in plan {
                    let id = self.new_id(NPC_OWNER);
                    self.bombs.place(world, id, NPC_OWNER, bk, NPC_RANGE, p);
                }
                self.npc.path = path;
                self.npc.cd = gen_range(1.4, 2.6);
                (self.npc.act, self.npc.t) = (PLACE, 0.0);
                return;
            }
            self.npc.cd = 0.3;
        }
        if dist > 1.6 {
            let to = if aligned {
                tp - (tp - self.npc.pos).normalize_or_zero() * 2.0
            } else if dx != 0 && dz != 0 {
                center(ivec3(if dx.abs() < dz.abs() { tc.x } else { me.x }, me.y, if dx.abs() < dz.abs() { me.z } else { tc.z }))
            } else {
                center(me + axis * 2)
            };
            let ahead = cell_of(self.npc.pos + (to - self.npc.pos).normalize_or_zero() * 0.7);
            if !danger.contains(&ivec2(ahead.x, ahead.z)) && !self.move_to(world, to, SPEED, dt) && self.npc.vy == 0.0 {
                self.npc.vy = 7.5;
            }
        }
    }

    /// Lutadores com raiva do BOMBADINHO vão pra cima dele (o resto da IA deles segue igual).
    fn brawl(&mut self, dt: f32, fighters: &mut [Fighter], npcs: &mut Npcs, events: &mut Vec<Ev>) {
        self.aggro.resize(fighters.len(), 0.0);
        let me = self.npc.pos;
        let on = self.npc.spawned && self.npc.act != DEAD;
        for (j, f) in fighters.iter_mut().enumerate() {
            self.aggro[j] -= dt;
            let flat = vec3(me.x - f.pos.x, 0.0, me.z - f.pos.z);
            let d = flat.length();
            if on && f.active() && d < 3.0 && (me.y - f.pos.y).abs() < 2.0 {
                self.aggro[j] = self.aggro[j].max(1.5);
            }
            if !on || !f.active() || self.aggro[j] <= 0.0 || d > 14.0 || f.state != FState::Idle {
                continue;
            }
            let dir = flat.normalize_or(Vec3::X);
            f.yaw = angle_lerp(f.yaw, dir.x.atan2(dir.z), dt * 12.0);
            if d > 1.6 {
                f.state = FState::Dodge(0.1, dir * f.speed);
                f.walk += dt * f.speed * 2.8;
                f.walk_amt = (f.walk_amt + dt * 5.0).min(1.0);
            } else if f.atk_cd <= 0.0 && self.npc.invuln <= 0.0 {
                f.state = FState::Attack { kind: if gen_range(0, 2) == 0 { Atk::Jab } else { Atk::Cross }, t: 0.0, done: true };
                f.target = None;
                self.swings.push((j, 0.15, f.dmg * if f.berserk > 0.0 { 1.4 } else { 1.0 }));
            }
        }
        for s in self.swings.iter_mut() {
            s.1 -= dt;
        }
        let ready: Vec<(usize, f32, f32)> = self.swings.iter().copied().filter(|s| s.1 <= 0.0).collect();
        self.swings.retain(|s| s.1 > 0.0);
        for (j, _, dmg) in ready {
            let f = &fighters[j];
            if f.active() && vec2(me.x - f.pos.x, me.z - f.pos.z).length() < 2.2 && (me.y - f.pos.y).abs() < 1.6 {
                let mut kos = f.kos;
                let name = f.name;
                self.take_hit(npcs, dmg, name, &mut kos, events);
                fighters[j].kos = kos;
            }
        }
    }

    /// Host: vida, física e IA do NPC; revide dos lutadores.
    pub fn think(&mut self, world: &World, dt: f32, time: f32, fighters: &mut [Fighter], npcs: &mut Npcs, events: &mut Vec<Ev>) {
        let Some(life) = npcs.get(npc::BOMBA, 0).copied() else { return };
        if !self.npc.spawned {
            if time > 10.0 {
                self.spawn(events, true);
                self.npc.last_hp = life.hp;
            }
            return;
        }
        if life.alive() && self.npc.act == DEAD {
            self.spawn(events, false);
        } else if !life.alive() && self.npc.act != DEAD {
            self.die();
            events.push(Ev::Banner("DESARMARAM O BOMBADINHO! VOLTA EM 40s".into()));
        } else if life.hp < self.npc.last_hp - 0.01 && self.npc.act != DEAD {
            (self.npc.act, self.npc.t) = (HURT, 0.0);
        }
        self.npc.last_hp = life.hp;
        let n = &mut self.npc;
        n.t += dt;
        n.invuln -= dt;
        n.moving = false;
        let ground = world.floor_at(n.pos.x, n.pos.y + 0.05, n.pos.z);
        if n.pos.y > ground + 0.01 || n.vy > 0.0 {
            n.vy -= 24.0 * dt;
            n.pos.y = (n.pos.y + n.vy * dt).max(ground);
            if n.pos.y <= ground {
                n.vy = 0.0;
            }
        } else {
            (n.pos.y, n.vy) = (ground, 0.0);
        }
        if n.pos.y < 1.5 {
            let p = crate::layout::arena_point(10.0);
            n.pos = vec3(p.x, G as f32 + 6.0, p.y);
        }
        match n.act {
            DEAD => return,
            HURT if n.t < 0.35 => {}
            PLACE if n.t < 0.25 => {}
            _ => n.act = IDLE,
        }
        if self.npc.act == IDLE && self.npc.vy == 0.0 {
            self.ai(world, dt, fighters);
        }
        if let Some(&next) = self.npc.path.first() {
            let c = center(next);
            if vec2(c.x - self.npc.pos.x, c.z - self.npc.pos.z).length() < 0.15 {
                self.npc.path.remove(0);
            } else if !self.move_to(world, c, SPEED * 1.15, dt) {
                if self.npc.vy == 0.0 && next.y > cell_of(self.npc.pos).y {
                    self.npc.vy = 7.5;
                }
                self.npc.replan = 0.0;
                self.npc.path.clear();
            }
        }
        if self.npc.moving && self.npc.act == IDLE {
            self.npc.act = WALK;
        }
        self.brawl(dt, fighters, npcs, events);
    }

    /// Todos: chamas, previsões, interpolação; host devolve as explosões pra mandar.
    pub fn update(&mut self, world: &World, dt: f32, is_host: bool, fighters: &[Fighter], players: &[(u64, Vec3)], fx: &mut Fx) -> Vec<Value> {
        self.bombs.tick_flames(dt);
        let (per, cap) = crate::quality::pick([(0, 12), (1, 32), (2, 64)]);
        for f in self.bombs.flames.iter_mut().filter(|f| f.fresh) {
            f.fresh = false;
            for (k, p) in f.cells.iter().enumerate().take(cap) {
                let c = p.as_vec3() + vec3(0.5, 0.4, 0.5);
                for _ in 0..per + (k == 0) as usize * 4 {
                    let hot = gen_range(0.0, 1.0);
                    fx.particles.push(Particle { pos: c, vel: vec3(gen_range(-2.0, 2.0), gen_range(1.5, 5.0), gen_range(-2.0, 2.0)), col: Color::new(1.0, 0.45 + 0.5 * hot, 0.1 * hot, 1.0), life: gen_range(0.25, 0.6), size: gen_range(0.18, 0.4), gravity: false });
                }
            }
        }
        let list = &self.bombs.list;
        self.pending.retain_mut(|b| {
            b.age += dt;
            b.t -= dt;
            b.age < 1.5 && !list.iter().any(|l| l.id == b.id)
        });
        if !is_host {
            for b in self.bombs.list.iter_mut() {
                b.t -= dt;
                b.pos = if b.pos.distance(b.np) > 4.0 { b.np } else { b.pos.lerp(b.np, (dt * 14.0).min(1.0)) };
            }
            let n = &mut self.npc;
            if let Some(p) = n.net {
                let before = n.pos;
                n.pos = if n.pos.distance(p) > 8.0 { p } else { n.pos.lerp(p, (dt * 12.0).min(1.0)) };
                n.t += dt;
                n.walk += before.distance(n.pos).min(1.0) * 2.6;
            }
            return Vec::new();
        }
        let mut bodies: Vec<(Key, Vec3)> = fighters.iter().enumerate().filter(|(_, f)| f.active()).map(|(j, f)| ((0, j as u64), f.pos)).collect();
        bodies.extend(players.iter().map(|&(id, p)| ((1, id), p)));
        if self.npc.spawned && self.npc.act != DEAD {
            bodies.push(((2, 0), self.npc.pos));
        }
        self.bombs.update(world, dt, &bodies)
    }

    /// Host: quem está na chama (uma vez por explosão). Lutadores apanham aqui; o resto volta pro main (npc_hit).
    pub fn burn(&mut self, targets: &[Target], fighters: &mut [Fighter], time: f32, events: &mut Vec<Ev>) -> Vec<(u8, usize, f32, Vec3, Vec3)> {
        let mut out = Vec::new();
        self.aggro.resize(fighters.len(), 0.0);
        for f in self.bombs.flames.iter_mut() {
            let c0 = center(f.cells[0]);
            for &(c, r, i, g) in targets {
                if f.hit.contains(&(g, i)) || !in_flame(&f.cells, c, r) {
                    continue;
                }
                f.hit.push((g, i));
                let dir = vec3(c.x - c0.x, 0.0, c.z - c0.z).normalize_or(Vec3::X);
                if g != npc::FIGHTER {
                    out.push((g, i, if g >= npc::URNA { 60.0 } else { 40.0 }, dir, c));
                    continue;
                }
                let Some(fi) = fighters.get_mut(i).filter(|x| x.active()) else { continue };
                fi.hp -= 34.0;
                fi.vel += dir * 6.0 + Vec3::Y * 5.0;
                fi.flash = 1.0;
                fi.last_hit = time;
                events.push(Ev::Text { pos: fi.pos + Vec3::Y * 2.2, text: "-34 KABUM!".into(), color: rgb(1.0, 0.55, 0.1), big: false });
                if f.owner == NPC_OWNER {
                    self.aggro[i] = 6.0;
                }
                if fi.hp <= 0.0 {
                    fi.hp = 0.0;
                    fi.state = FState::Ko(if fi.wolverine { 3.5 } else { 5.0 });
                    fi.ko_t = 0.0;
                    events.push(Ev::Text { pos: fi.pos + Vec3::Y * 2.8, text: "K.O.!".into(), color: rgb(1.0, 0.3, 0.2), big: true });
                    if f.owner == NPC_OWNER {
                        events.push(Ev::Banner(format!("BOMBADINHO EXPLODIU {}", fi.name)));
                    }
                } else {
                    fi.state = FState::Stun(0.5);
                }
            }
        }
        out
    }

    /// Jogador local na chama (uma vez por explosão): direção pra empurrar. BOMBADINHO jogável perde vida.
    pub fn burn_me(&mut self, pos: Vec3) -> Option<Vec3> {
        let c = pos + Vec3::Y * 0.9;
        let f = self.bombs.flames.iter_mut().find(|f| !f.me && in_flame(&f.cells, c, 0.4))?;
        f.me = true;
        let c0 = center(f.cells[0]);
        if let Some(m) = self.me.as_mut().filter(|m| m.dead <= 0.0) {
            m.hp -= 34.0;
            m.flash = 1.0;
        }
        Some(vec3(pos.x - c0.x, 0.0, pos.z - c0.z).normalize_or(Vec3::X))
    }

    /// Explosão de fora (urna, mods) no BOMBADINHO jogável.
    pub fn hurt_me(&mut self, dmg: f32) {
        if let Some(m) = self.me.as_mut().filter(|m| m.dead <= 0.0 && dmg > 0.0) {
            m.hp -= dmg;
            m.flash = 1.0;
        }
    }

    /// Bomba é obstáculo pro jogador local depois que ele sai de cima. Devolve (bomba, eixo) se trombou.
    pub fn block(&mut self, player: &mut Player, before: Vec3) -> Option<(u64, IVec3)> {
        let all: Vec<(u64, IVec3, bool)> = self.bombs.list.iter().map(|b| (b.id, b.cell, true)).chain(self.pending.iter().map(|b| (b.id, b.cell, false))).collect();
        let now = player.pos;
        self.on.retain(|id| all.iter().any(|b| b.0 == *id && overlaps(now, b.1)));
        let mut bump = None;
        for &(id, c, real) in &all {
            if self.on.contains(&id) || !overlaps(player.pos, c) {
                continue;
            }
            if overlaps(before, c) {
                self.on.push(id);
                continue;
            }
            let p = player.pos;
            let (xo, zo) = (vec3(p.x, p.y, before.z), vec3(before.x, p.y, p.z));
            player.pos = if !overlaps(xo, c) { xo } else if !overlaps(zo, c) { zo } else { vec3(before.x, p.y, before.z) };
            let to = center(c) - before;
            let axis = if to.x.abs() > to.z.abs() { ivec3(to.x.signum() as i32, 0, 0) } else { ivec3(0, 0, to.z.signum() as i32) };
            if real {
                bump = Some((id, axis));
            }
        }
        bump
    }

    /// BOMBADINHO jogável (depois do movimento do Player): obstáculo/chute, troca de tipo, bomba, detonar.
    pub fn play(&mut self, world: &World, dt: f32, inp: &Intent, player: &mut Player, before: Vec3, my_id: u64) -> Vec<Value> {
        let mut out = Vec::new();
        let bump = self.block(player, before);
        let Some(m) = self.me.as_mut() else { return out };
        m.flash = (m.flash - dt * 3.0).max(0.0);
        m.kick_cd -= dt;
        if m.dead > 0.0 {
            m.dead -= dt;
            player.pos = before;
            if m.dead <= 0.0 {
                m.hp = MAX_HP;
                player.pos = Player::spawn();
                player.vel = Vec3::ZERO;
            }
            return out;
        }
        if m.hp <= 0.0 {
            m.dead = 3.0;
            self.msg = Some("BOMBADO! VOLTA NA TORRE EM 3s".into());
            return out;
        }
        if let Some((id, d)) = bump.filter(|_| m.kick_cd <= 0.0) {
            m.kick_cd = 0.4;
            out.push(json!({"t": "a", "k": "kick", "b": id, "d": [d.x, d.z]}));
        }
        m.kind = (m.kind as i32 + inp.cycle).rem_euclid(NAMES.len() as i32) as u8;
        if inp.detonate {
            out.push(json!({"t": "a", "k": "bombx"}));
        }
        if !inp.place {
            return out;
        }
        let (kind, max, range) = (m.kind, m.max, m.range);
        let cell = cell_of(player.pos);
        let mine = self.bombs.list.iter().chain(&self.pending).filter(|b| b.owner == my_id).count();
        let fw = player.forward();
        let axis = if fw.x.abs() > fw.z.abs() { ivec3(fw.x.signum() as i32, 0, 0) } else { ivec3(0, 0, fw.z.signum() as i32) };
        let taken = |p: IVec3, s: &Self| world.solid(p.x, p.y, p.z) || s.bombs.list.iter().chain(&s.pending).any(|b| b.cell == p);
        let mut plan = Vec::new();
        for k in 0..if kind == LINHA { max.saturating_sub(mine) } else { (mine < max) as usize } {
            let p = cell + axis * k as i32;
            if taken(p, self) || in_shield(p) {
                break;
            }
            plan.push(p);
        }
        if plan.is_empty() {
            self.msg = Some(if mine >= max { format!("SEM BOMBA: {mine}/{max} NO CHAO") } else { "NAO CABE BOMBA AQUI".into() });
        }
        let bk = if kind == LINHA { NORMAL } else { kind };
        for p in plan {
            let id = self.new_id(my_id);
            out.push(json!({"t": "a", "k": "bomb", "p": [p.x, p.y, p.z], "ty": bk, "n": range, "bid": id}));
            let pos = center(p);
            self.pending.push(Bomb { id, owner: my_id, kind: bk, range, cell: p, pos, t: if bk == REMOTA { REMOTE_MAX } else { FUSE }, np: pos, slide: IVec3::ZERO, stuck: None, age: 0.0 });
            if overlaps(player.pos, p) {
                self.on.push(id);
            }
        }
        out
    }

    pub fn snapshot(&self) -> Value {
        let r = |x: f32| (x as f64 * 100.0).round() / 100.0;
        let n = &self.npc;
        json!({
            "n": [r(n.pos.x), r(n.pos.y), r(n.pos.z), r(n.yaw), n.act, r(n.t), n.spawned],
            "b": self.bombs.list.iter().map(|b| json!([b.id, b.cell.x, b.cell.y, b.cell.z, r(b.pos.x), r(b.pos.y), r(b.pos.z), b.kind, r(b.t), b.owner])).collect::<Vec<_>>(),
        })
    }

    /// Cliente: NPC e bombas do host (bomba que eu já vi explodir não volta).
    pub fn apply(&mut self, v: &Value) {
        if let Some(a) = v["n"].as_array().filter(|a| a.len() >= 7 && a[6].as_bool().unwrap_or(false)) {
            let f = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
            let n = &mut self.npc;
            let p = vec3(f(0), f(1), f(2));
            if n.net.is_none() {
                n.pos = p;
            }
            n.net = Some(p);
            n.yaw = f(3);
            let act = a[4].as_u64().unwrap_or(0) as u8;
            if act != n.act {
                n.t = f(5);
            }
            n.act = act;
        }
        let Some(list) = v["b"].as_array() else { return };
        let old = std::mem::take(&mut self.bombs.list);
        for e in list {
            let id = e[0].as_u64().unwrap_or(0);
            if self.bombs.heard.contains(&id) {
                continue;
            }
            let g = |i: usize| e[i].as_f64().unwrap_or(0.0) as f32;
            let np = vec3(g(4), g(5), g(6));
            let pos = old.iter().find(|b| b.id == id).map_or(np, |b| b.pos);
            let cell = ivec3(e[1].as_i64().unwrap_or(0) as i32, e[2].as_i64().unwrap_or(0) as i32, e[3].as_i64().unwrap_or(0) as i32);
            self.bombs.list.push(Bomb { id, owner: e[9].as_u64().unwrap_or(0), kind: e[7].as_u64().unwrap_or(0).min(GOSMA as u64) as u8, range: 1, cell, pos, t: g(8), np, slide: IVec3::ZERO, stuck: None, age: 1.0 });
        }
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, life: Option<&Vida>, me: Option<(Vec3, f32, f32, bool)>) {
        let id = Mat4::IDENTITY;
        for bm in self.bombs.list.iter().chain(&self.pending) {
            draw_bomb(b, trans, bm.pos, bm.kind, bm.t, time, bm.id);
        }
        let low = crate::quality::tier() == crate::quality::LOW;
        for f in &self.bombs.flames {
            let k = (f.t / FLAME).clamp(0.0, 1.0);
            let s = ((1.0 - k) * 12.0).min(1.0) * (k * 3.0).min(1.0);
            for (n, p) in f.cells.iter().enumerate() {
                let c = p.as_vec3() + vec3(0.5, 0.45, 0.5);
                let w = 0.05 * (time * 40.0 + n as f32 * 1.7).sin();
                b.glow(&id, c, Vec3::splat(0.5 * s + w), Color::new(1.0, 0.95, 0.65, 1.0));
                if !low {
                    trans.glow(&id, c, vec3(0.98, 0.9, 0.98) * s, Color::new(1.0, 0.42, 0.05, 0.25 + 0.45 * k));
                }
            }
        }
        if let (Some((pos, yaw, walk, moving)), Some(m)) = (me, &self.me) {
            if m.dead <= 0.0 {
                let p = pose(if moving { WALK } else { IDLE }, 0.0, walk, true);
                draw_body(b, trans, pos, yaw, &p, m.flash, time);
            }
        }
        if !self.visible() {
            return;
        }
        let n = &self.npc;
        let p = pose(n.act, n.t, n.walk, n.vy == 0.0 || n.net.is_some());
        let head = draw_body(b, trans, n.pos, n.yaw, &p, life.map_or(0.0, |l| l.flash), time);
        let tag = head + Vec3::Y * 0.6;
        if n.act == DEAD {
            if n.t < 6.0 {
                labels.push(Label { pos: tag, text: "\"FALHOU O PAVIO...\"".into(), size: 22.0, color: rgb(1.0, 0.5, 0.2) });
            }
            return;
        }
        labels.push(Label { pos: tag, text: "BOMBADINHO".into(), size: 24.0, color: rgb(1.0, 0.55, 0.1) });
        if time.rem_euclid(13.0) < 4.0 {
            labels.push(Label { pos: tag + Vec3::Y * 0.7, text: format!("\"{}\"", FALAS[(time / 13.0) as usize % FALAS.len()]), size: 20.0, color: WHITE });
        }
    }

    /// HUD do BOMBADINHO jogável (rodapé, igual a arma do bandido).
    pub fn hud(&self, sw: f32, sh: f32, my_id: u64, text: impl Fn(&str, f32, f32, f32, Color)) {
        let Some(m) = &self.me else { return };
        if m.dead > 0.0 {
            text("BOMBADO!", sw * 0.5, sh * 0.5, (sh * 0.1).max(40.0), Color::new(1.0, 0.45, 0.1, 1.0));
            return;
        }
        let mine = self.bombs.list.iter().chain(&self.pending).filter(|b| b.owner == my_id).count();
        let s = format!("< {} >   BOMBAS {}/{}   ALCANCE {}   VIDA {:.0}", NAMES[m.kind as usize], m.max.saturating_sub(mine), m.max, m.range + if m.kind == FOGO { 3 } else { 0 }, m.hp.max(0.0));
        text(&s, sw * 0.5, sh - 24.0, 26.0, Color::new(1.0, 0.6 + 0.4 * (1.0 - m.flash), 0.2 + 0.6 * (1.0 - m.flash), 1.0));
    }
}

fn overlaps(p: Vec3, c: IVec3) -> bool {
    let (x, y, z) = (c.x as f32, c.y as f32, c.z as f32);
    p.x + 0.3 > x && p.x - 0.3 < x + 1.0 && p.z + 0.3 > z && p.z - 0.3 < z + 1.0 && p.y < y + 1.0 && p.y + 1.79 > y
}

/// Jogador remoto de BOMBADINHO: só a pose de andar a partir da posição.
pub fn draw_remote(b: &mut Batch, trans: &mut Batch, pos: Vec3, yaw: f32, walk: f32, moving: bool, time: f32) {
    draw_body(b, trans, pos, yaw, &pose(if moving { WALK } else { IDLE }, 0.0, walk, true), 0.0, time);
}

#[derive(Default)]
struct BPose {
    leg: f32,
    arm_l: f32,
    arm_r: f32,
    lean: f32,
    crouch: f32,
    bounce: f32,
    nod: f32,
}

fn pose(act: u8, t: f32, walk: f32, ground: bool) -> BPose {
    let mut p = BPose { arm_l: -0.1, arm_r: -0.1, ..Default::default() };
    match act {
        _ if !ground => {
            p.arm_l = -2.6;
            p.arm_r = -2.6;
            p.leg = 0.4;
        }
        WALK => {
            let s = walk.sin();
            p.leg = s * 0.8;
            p.arm_l = -s * 0.7;
            p.arm_r = s * 0.7;
            p.bounce = walk.sin().abs() * 0.05;
            p.lean = 0.12;
        }
        PLACE => {
            let k = (1.0 - (t / 0.25 - 0.5).abs() * 2.0).max(0.0);
            p.crouch = k;
            p.arm_l = -0.9 * k;
            p.arm_r = -0.9 * k;
            p.lean = 0.35 * k;
        }
        HURT => {
            p.lean = -0.35;
            p.arm_l = -2.2;
            p.arm_r = -2.0;
            p.nod = -0.3;
        }
        DEAD => {
            p.lean = -(t * 3.0).min(1.0) * FRAC_PI_2;
            p.arm_l = -2.8;
            p.arm_r = -2.8;
        }
        _ => {
            let s = (walk * 0.3 + t * 2.0).sin() * 0.05;
            p.arm_l += s;
            p.arm_r -= s;
        }
    }
    p
}

/// Modelo blocado original; retorna a posição do topo da cabeça.
fn draw_body(b: &mut Batch, trans: &mut Batch, pos: Vec3, yaw: f32, p: &BPose, flash: f32, time: f32) -> Vec3 {
    let tint = |c: Color| {
        let f = flash * 0.75;
        Color::new(c.r + (1.0 - c.r) * f, c.g + (1.0 - c.g) * f * 0.6, c.b + (1.0 - c.b) * f * 0.6, 1.0)
    };
    let navy = tint(rgb(0.13, 0.17, 0.36));
    let vest = tint(rgb(0.78, 0.92, 0.16));
    let skin = tint(rgb(0.93, 0.72, 0.55));
    let boot = tint(rgb(0.1, 0.08, 0.07));
    let glove = tint(rgb(0.92, 0.52, 0.12));
    let hat = tint(rgb(1.0, 0.5, 0.08));
    let hat_d = tint(rgb(0.82, 0.36, 0.04));
    let stache = tint(rgb(0.1, 0.07, 0.05));
    let black = rgb(0.05, 0.05, 0.06);
    let stripe = Color::new(0.88, 0.9, 0.92, 1.0);
    let t = |x: f32, y: f32, z: f32| Mat4::from_translation(vec3(x, y, z));
    let base = t(pos.x, pos.y + p.bounce, pos.z) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(p.lean);
    let m = base * Mat4::from_scale(vec3(1.0 + 0.12 * p.crouch, 1.0 - 0.25 * p.crouch, 1.0 + 0.12 * p.crouch));
    // Pernas: macacão e botina
    for (side, ang) in [(1.0f32, p.leg), (-1.0, -p.leg)] {
        let l = m * t(side * 2.3 * U, 8.0 * U, 0.0) * Mat4::from_rotation_x(ang);
        b.cube(&l, vec3(0.0, -3.0 * U, 0.0), vec3(4.0 * U, 6.0 * U, 4.0 * U), navy);
        b.cube(&l, vec3(0.0, -6.8 * U, 0.6 * U), vec3(4.6 * U, 2.6 * U, 5.6 * U), boot);
    }
    // Tronco: colete refletivo e cinto de bombinhas com pavio
    b.cube(&m, vec3(0.0, 12.2 * U, 0.0), vec3(9.0 * U, 7.6 * U, 5.6 * U), navy);
    b.cube(&m, vec3(0.0, 13.2 * U, 0.0), vec3(9.6 * U, 5.6 * U, 6.2 * U), vest);
    for y in [11.6f32, 14.4] {
        b.glow(&m, vec3(0.0, y * U, 0.0), vec3(9.8 * U, 0.8 * U, 6.4 * U), stripe);
    }
    b.cube(&m, vec3(0.0, 9.0 * U, 0.0), vec3(9.4 * U, 1.2 * U, 6.0 * U), tint(rgb(0.3, 0.18, 0.08)));
    for k in [-1.0f32, 0.0, 1.0] {
        b.cube(&m, vec3(k * 2.8 * U, 8.8 * U, 3.3 * U), vec3(1.8 * U, 1.8 * U, 1.8 * U), black);
        b.glow(&m, vec3(k * 2.8 * U, 10.0 * U, 3.3 * U), vec3(0.4 * U, 0.7 * U, 0.4 * U), Color::new(1.0, 0.75, 0.3, 1.0));
    }
    // Braços: manga do macacão e luvona laranja
    for (side, ang) in [(1.0f32, p.arm_l), (-1.0, p.arm_r)] {
        let a = m * t(side * 5.6 * U, 15.6 * U, 0.0) * Mat4::from_rotation_x(ang) * Mat4::from_rotation_z(side * 0.12);
        b.cube(&a, vec3(0.0, -2.8 * U, 0.0), vec3(3.0 * U, 5.8 * U, 3.0 * U), navy);
        b.cube(&a, vec3(0.0, -6.6 * U, 0.0), vec3(3.6 * U, 2.8 * U, 3.6 * U), glove);
    }
    // Cabeça: óculos de solda, nariz, bigodão; capacete de obra com pavio aceso no topo
    let h = m * t(0.0, 16.2 * U, 0.0) * Mat4::from_rotation_x(p.nod);
    b.cube(&h, vec3(0.0, 4.0 * U, 0.0), vec3(8.4 * U, 8.0 * U, 8.0 * U), skin);
    b.cube(&h, vec3(0.0, 3.4 * U, 4.4 * U), vec3(1.8 * U, 1.8 * U, 1.2 * U), tint(rgb(0.95, 0.62, 0.48)));
    b.cube(&h, vec3(0.0, 2.0 * U, 4.3 * U), vec3(6.4 * U, 1.6 * U, 1.0 * U), stache);
    b.cube(&h, vec3(0.0, 5.6 * U, 0.0), vec3(8.8 * U, 1.4 * U, 8.8 * U), black);
    for side in [1.0f32, -1.0] {
        b.glow(&h, vec3(side * 1.9 * U, 5.6 * U, 4.45 * U), vec3(2.6 * U, 2.0 * U, 0.4 * U), Color::new(0.35, 0.9, 1.0, 1.0));
    }
    b.cube(&h, vec3(0.0, 8.8 * U, 0.0), vec3(9.2 * U, 2.6 * U, 9.2 * U), hat);
    b.cube(&h, vec3(0.0, 10.4 * U, 0.0), vec3(7.0 * U, 1.4 * U, 7.0 * U), hat);
    b.cube(&h, vec3(0.0, 7.6 * U, 1.2 * U), vec3(10.6 * U, 0.6 * U, 11.2 * U), hat_d);
    b.cube(&h, vec3(0.0, 10.2 * U, 0.0), vec3(1.2 * U, 2.8 * U, 9.4 * U), hat_d);
    let wick = h * t(0.0, 11.0 * U, -1.0 * U) * Mat4::from_rotation_x(-0.5);
    b.cube(&wick, vec3(0.0, 1.6 * U, 0.0), vec3(0.7 * U, 3.2 * U, 0.7 * U), rgb(0.35, 0.3, 0.25));
    let spark = wick.transform_point3(vec3(0.0, 3.4 * U, 0.0));
    let fl = 0.6 + 0.4 * (time * 31.0).sin().abs();
    let sm = Mat4::from_translation(spark) * Mat4::from_rotation_y(time * 9.0) * Mat4::from_rotation_x(time * 7.0);
    trans.glow(&sm, Vec3::ZERO, Vec3::splat(1.8 * U * fl), Color::new(1.0, 0.85, 0.25, 0.85));
    b.glow(&sm, Vec3::ZERO, Vec3::splat(0.8 * U), WHITE);
    h.transform_point3(vec3(0.0, 9.0 * U, 0.0))
}

/// Bomba de pavio (redonda em blocos), faixa na cor do tipo; pulsa mais rápido no fim do pavio.
fn draw_bomb(b: &mut Batch, trans: &mut Batch, pos: Vec3, kind: u8, t: f32, time: f32, id: u64) {
    let urgent = if kind == REMOTA { 0.0 } else { (1.0 - t / FUSE).clamp(0.0, 1.0) };
    let s = 1.0 + 0.07 * (time * (5.0 + 16.0 * urgent)).sin() * (0.4 + urgent);
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y((id % 628) as f32 * 0.01) * Mat4::from_scale(Vec3::splat(s));
    let black = Color::new(0.06, 0.06, 0.08, 1.0);
    let band = [rgb(0.6, 0.6, 0.66), rgb(1.0, 0.35, 0.08), rgb(0.25, 0.55, 1.0), rgb(0.95, 0.15, 0.15), WHITE, rgb(0.35, 0.9, 0.2)][kind.min(GOSMA) as usize];
    let c = vec3(0.0, 0.36, 0.0);
    b.cube(&m, c, Vec3::splat(0.56), black);
    for sz in [vec3(0.68, 0.4, 0.4), vec3(0.4, 0.4, 0.68), vec3(0.4, 0.68, 0.4)] {
        b.cube(&m, c, sz, black);
    }
    b.glow(&m, c, vec3(0.6, 0.1, 0.6), band);
    b.glow(&m, vec3(-0.14, 0.52, 0.29), vec3(0.1, 0.1, 0.02), Color::new(0.75, 0.75, 0.85, 1.0));
    b.cube(&m, vec3(0.0, 0.72, 0.0), vec3(0.2, 0.08, 0.2), rgb(0.5, 0.5, 0.55));
    match kind {
        PERFURA => {
            for d in DIRS {
                b.cube(&m, c + d.as_vec3() * 0.38, vec3(0.1, 0.1, 0.1) + d.abs().as_vec3() * 0.1, band);
            }
        }
        REMOTA => {
            b.cube(&m, vec3(0.12, 0.92, 0.0), vec3(0.04, 0.34, 0.04), rgb(0.7, 0.7, 0.75));
            if (time * 3.0).fract() < 0.5 {
                b.glow(&m, vec3(0.12, 1.1, 0.0), Vec3::splat(0.09), Color::new(1.0, 0.1, 0.1, 1.0));
            }
        }
        GOSMA => {
            for (x, z, h) in [(0.22f32, 0.18f32, 0.26f32), (-0.2, -0.16, 0.2), (0.05, -0.27, 0.16)] {
                b.glow(&m, vec3(x, 0.2, z), vec3(0.16, h, 0.16), Color::new(0.4, 0.95, 0.25, 1.0));
            }
        }
        _ => {}
    }
    if kind != REMOTA {
        let len = 0.18 * (t / FUSE).clamp(0.2, 1.0);
        b.cube(&m, vec3(0.0, 0.76 + len * 0.5, 0.0), vec3(0.05, len, 0.05), rgb(0.45, 0.35, 0.2));
        let sp = m.transform_point3(vec3(0.0, 0.78 + len, 0.0));
        let f = 0.5 + 0.5 * (time * 37.0 + id as f32).sin().abs();
        trans.glow(&(Mat4::from_translation(sp) * Mat4::from_rotation_y(time * 11.0)), Vec3::ZERO, Vec3::splat(0.12 * f + 0.04), Color::new(1.0, 0.8, 0.2, 0.9));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chão limpo 17x17 na praça (fora de escudo e de área protegida).
    fn open(w: &mut World) -> IVec3 {
        let c = crate::layout::plaza_center().as_ivec3();
        let o = ivec3(c.x + 12, G, c.z);
        for z in -8..=8 {
            for x in -8..=8 {
                w.set(o.x + x, G - 1, o.z + z, GRASS);
                for y in 0..4 {
                    w.set(o.x + x, G + y, o.z + z, AIR);
                }
            }
        }
        assert!(!reserved(o.x, o.z) && !in_shield(o));
        o
    }

    fn msg(id: u64, c: IVec3, b: &Blast) -> Value {
        json!({"t": "w", "k": "bomb", "id": id, "c": [c.x, c.y, c.z], "a": b.arms, "ty": 0, "o": 0, "by": BY})
    }

    fn boom(w: &mut World, o: IVec3, range: u8, pierce: bool) -> Blast {
        let b = propagate(w, o, range, pierce, |_| None);
        apply(w, &msg(1, o, &b), &mut Fx::default(), &vec![WHITE; 256]);
        b
    }

    #[test]
    fn open_air_full_range() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        let b = boom(&mut w, o, 3, false);
        assert_eq!(b.arms, [3; 4]);
        assert!(b.broken.is_empty());
        assert_eq!(cells(o, b.arms).count(), 13);
        assert_eq!(w.get(o.x, G - 1, o.z), GRASS, "chão embaixo fica");
    }

    #[test]
    fn stops_before_hard_block() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        w.set(o.x + 2, G, o.z, STONE);
        let b = boom(&mut w, o, 4, true);
        assert_eq!(b.arms[0], 1);
        assert_eq!(w.get(o.x + 2, G, o.z), STONE);
    }

    #[test]
    fn breaks_first_soft_block_and_stops() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        w.set(o.x, G, o.z + 2, PLANKS);
        w.set(o.x, G, o.z + 3, PLANKS);
        let b = boom(&mut w, o, 5, false);
        assert_eq!(b.arms[2], 2);
        assert_eq!(b.broken, vec![o + ivec3(0, 0, 2)]);
        assert_eq!(w.get(o.x, G, o.z + 2), AIR);
        assert_eq!(w.get(o.x, G, o.z + 3), PLANKS);
    }

    #[test]
    fn pierce_goes_through_soft_until_hard() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        for k in 1..=2 {
            w.set(o.x - k, G, o.z, DIRT);
        }
        w.set(o.x - 4, G, o.z, COBBLE);
        let b = boom(&mut w, o, 6, true);
        assert_eq!(b.arms[1], 3);
        assert_eq!(b.broken.len(), 2);
        assert_eq!((w.get(o.x - 1, G, o.z), w.get(o.x - 2, G, o.z), w.get(o.x - 4, G, o.z)), (AIR, AIR, COBBLE));
    }

    #[test]
    fn range_and_fogo_bonus() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        let mut bs = Bombs::default();
        assert!(bs.place(&w, 1, 7, FOGO, 2, o));
        assert_eq!(bs.list[0].range, 5);
        assert!(!bs.place(&w, 2, 7, NORMAL, 2, o), "casa ocupada");
        assert!(!bs.place(&w, 3, 7, LINHA, 2, o + IVec3::X), "linha é modo, não bomba");
        bs.trigger(1);
        let ev = bs.update(&w, 0.016, &[]);
        assert_eq!(ev.len(), 1);
        assert_eq!(parse(&ev[0]).unwrap().1, [5; 4]);
    }

    #[test]
    fn chain_reaction_each_bomb_once() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        let mut bs = Bombs::default();
        let mut tnt = Tnt::default();
        let mut id = 0;
        for z in 0..6 {
            for x in 0..6 {
                id += 1;
                assert!(bs.place(&w, id, id % 5, NORMAL, 2, o + ivec3(x * 2 - 6, 0, z * 2 - 6)));
            }
        }
        bs.trigger(1);
        let (mut seen, mut frames) = (HashSet::new(), 0);
        while !bs.list.is_empty() {
            frames += 1;
            assert!(frames < 50, "corrente não terminou");
            let ev = bs.update(&w, 0.016, &[]);
            assert!(ev.len() <= BOOMS_PER_FRAME);
            for m in ev {
                assert!(bs.before_world(&w, &m, &mut tnt));
                assert!(!bs.before_world(&w, &m, &mut tnt), "eco repetido não explode de novo");
                assert!(seen.insert(m["id"].as_u64().unwrap()), "cada bomba explode uma vez");
            }
        }
        assert_eq!(seen.len(), 36);
        assert!(frames >= 3, "teto por frame segura a corrente");
        assert!(!bs.place(&w, 1, 0, NORMAL, 2, o), "id que explodiu não volta");
        for _ in 0..40 {
            bs.tick_flames(0.016);
        }
        assert!(bs.flames.is_empty(), "chama some depois de 0,5s");
    }

    #[test]
    fn kick_slides_until_blocked() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        w.set(o.x + 5, G, o.z, STONE);
        let mut bs = Bombs::default();
        assert!(bs.place(&w, 9, 1, REMOTA, 2, o));
        assert!(bs.kick(9, IVec3::X));
        for _ in 0..120 {
            assert!(bs.update(&w, 0.016, &[]).is_empty());
        }
        assert_eq!(bs.list[0].cell, o + ivec3(4, 0, 0));
        bs.detonate_remote(1);
        assert_eq!(bs.update(&w, 0.016, &[]).len(), 1);
    }

    #[test]
    fn shields_and_protected_areas_hold() {
        let mut w = crate::mods::generate();
        let c = crate::lab::dome_center().as_ivec3() + ivec3(0, 1, 0);
        assert!(hard(&w, c));
        let mut bs = Bombs::default();
        assert!(!bs.place(&w, 1, 0, NORMAL, 3, c), "bomba não nasce no domo");
        let a = arena_center().as_ivec3() + ivec3(3, 0, 0);
        w.set(a.x + 1, a.y, a.z, PLANKS);
        let b = boom(&mut w, a, 3, true);
        assert_eq!(b.arms[0], 0);
        assert_eq!(w.get(a.x + 1, a.y, a.z), PLANKS, "arena é área protegida do servidor");
        let forged = json!({"k": "bomb", "id": 5, "c": [a.x, a.y, a.z], "a": [8, 8, 8, 8]});
        apply(&mut w, &forged, &mut Fx::default(), &vec![WHITE; 256]);
        assert_eq!(w.get(a.x + 1, a.y, a.z), PLANKS, "evento forjado não quebra área protegida");
    }

    #[test]
    fn replay_is_idempotent() {
        let mut w = crate::mods::generate();
        let o = open(&mut w);
        w.set(o.x + 1, G, o.z, TNT);
        w.set(o.x + 2, G, o.z, WOOL);
        let b = propagate(&w, o, 4, false, |_| None);
        let m = msg(3, o, &b);
        let mut tnt = Tnt::default();
        let mut bs = Bombs::default();
        assert!(bs.before_world(&w, &m, &mut tnt));
        assert_eq!(tnt.primed.len(), 1, "chama acende TNT");
        apply(&mut w, &m, &mut Fx::default(), &vec![WHITE; 256]);
        assert_eq!(w.get(o.x + 1, G, o.z), AIR);
        assert_eq!(w.get(o.x + 2, G, o.z), WOOL);
        let w2 = w.blocks.clone();
        apply(&mut w, &m, &mut Fx::default(), &vec![WHITE; 256]);
        assert!(w.blocks == w2, "aplicar de novo não muda nada");
        assert!(!bs.before_world(&w, &m, &mut tnt));
        assert_eq!(tnt.primed.len(), 1);
    }
}
