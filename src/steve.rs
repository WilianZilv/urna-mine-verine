//! Steve jogável. SOBREVIVÊNCIA: vida, dano de queda/explosão/fogo/PvP, ferramentas com tempo de
//! quebra, arco com flecha balística, TNT com pavio e isqueiro. CRIATIVO: voo, quebra instantânea,
//! blocos infinitos e sem dano. Mudanças de mundo vão pelos eventos "w" (host e quem entra depois
//! veem igual); flechas e dano PvP pegam carona na mensagem "p".

use crate::atlas::Atlas;
use crate::batch::Batch;
use crate::gta::Target;
use crate::inventory::{self, Inv};
use crate::items::{self, *};
use crate::models::{U, root};
use crate::mp;
use crate::npc;
use crate::player::Player;
use crate::urna::{self, Fx, Particle};
use crate::world::*;
use macroquad::miniquad::PassAction;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

pub const MAX_HP: f32 = 20.0;
/// Valor de "by" no tiro da explosão de TNT.
pub const BY: u64 = 11;
const FUSE: f32 = 4.0;
const FIRE_LIFE: f32 = 6.0;
const REACH: f32 = 4.5;
/// Ângulo do braço direito dos Steves remotos (Pose.arm_r no main): o item segue esse braço.
const REMOTE_ARM: f32 = -0.2;

/// Entrada do frame (mouse + botões do celular).
pub struct Input {
    pub attack: bool,
    pub mine: bool,
    pub use_press: bool,
    pub use_hold: bool,
}

struct Arrow {
    pos: Vec3,
    vel: Vec3,
    /// > 0: cravada num bloco (segundos até sumir).
    stuck: f32,
    dmg: f32,
    mine: bool,
}

struct Fuse {
    p: IVec3,
    t: f32,
    owner: u64,
}

pub struct Steve {
    pub creative: bool,
    pub inv: Inv,
    pub inv_open: bool,
    pub hp: f32,
    active: bool,
    dead: f32,
    death_pos: Option<Vec3>,
    calm: f32,
    hurt_flash: f32,
    last_vy: f32,
    was_ground: bool,
    fire_cd: f32,
    mine: Option<(IVec3, f32)>,
    mine_cd: f32,
    charge: f32,
    was_use: bool,
    swing: f32,
    arrows: Vec<Arrow>,
    fuses: Vec<Fuse>,
    fires: Vec<(IVec3, f32)>,
    pending: HashSet<IVec3>,
    /// Mensagens pro main mandar (eventos de mundo / golpes em NPC).
    pub outbox: Vec<Value>,
    shots_out: Vec<Value>,
    pvp_out: Vec<Value>,
    held: HashMap<u64, Item>,
    cracks: Vec<Texture2D>,
    toast: Option<(String, f32)>,
    last_sel: usize,
    /// 1 -> 0 ao trocar de item (a mão desce e sobe).
    equip: f32,
    /// Atlas próprio pros cubos texturizados (TNT acesa, bloco na mão dos outros).
    tex: Texture2D,
    tm: TexMesh,
}

fn crack_textures() -> Vec<Texture2D> {
    // Rachaduras: caminhadas aleatórias saindo do centro; cada estágio mostra mais passos
    let mut order = [[u8::MAX; 16]; 16];
    for w in 0..7 {
        let a0 = w as f32 / 7.0 * std::f32::consts::TAU + crate::atlas::hash2(w, 0, 51) * 0.6;
        let (mut x, mut y) = (7.5f32, 7.5f32);
        for s in 0..12u8 {
            let a = a0 + (crate::atlas::hash2(w, s as i32, 52) - 0.5) * 1.4;
            x += a.cos();
            y += a.sin();
            let (xi, yi) = (x as i32, y as i32);
            if (0..16).contains(&xi) && (0..16).contains(&yi) {
                let o = &mut order[yi as usize][xi as usize];
                *o = (*o).min(s);
            }
        }
    }
    (0..8)
        .map(|stage| {
            let lim = (stage as u8 + 1) * 12 / 8;
            let mut bytes = vec![0u8; 16 * 16 * 4];
            for y in 0..16 {
                for x in 0..16 {
                    if order[y][x] <= lim {
                        bytes[(y * 16 + x) * 4 + 3] = 210;
                    }
                }
            }
            let t = Texture2D::from_rgba8(16, 16, &bytes);
            t.set_filter(FilterMode::Nearest);
            t
        })
        .collect()
}

fn ip(p: IVec3) -> Value {
    json!([p.x, p.y, p.z])
}

fn set_msg(p: IVec3, b: u8) -> Value {
    json!({"t": "w", "k": "set", "p": ip(p), "b": b})
}

fn lit_msg(p: IVec3, t: f32) -> Value {
    json!({"t": "w", "k": "set", "p": ip(p), "b": AIR, "lit": t})
}

impl Steve {
    pub fn new() -> Self {
        Steve {
            creative: true,
            inv: Inv::creative(),
            inv_open: false,
            hp: MAX_HP,
            active: true,
            dead: 0.0,
            death_pos: None,
            calm: 0.0,
            hurt_flash: 0.0,
            last_vy: 0.0,
            was_ground: true,
            fire_cd: 0.0,
            mine: None,
            mine_cd: 0.0,
            charge: 0.0,
            was_use: false,
            swing: 0.0,
            arrows: Vec::new(),
            fuses: Vec::new(),
            fires: Vec::new(),
            pending: HashSet::new(),
            outbox: Vec::new(),
            shots_out: Vec::new(),
            pvp_out: Vec::new(),
            held: HashMap::new(),
            cracks: crack_textures(),
            toast: None,
            last_sel: 0,
            equip: 0.0,
            tex: crate::atlas::build().tex,
            tm: TexMesh::new(),
        }
    }

    pub fn set_mode(&mut self, creative: bool, player: &mut Player) {
        self.creative = creative;
        self.inv = if creative { Inv::creative() } else { Inv::survival() };
        self.hp = MAX_HP;
        self.dead = 0.0;
        self.mine = None;
        self.inv_open = false;
        player.sel = 0;
        if !creative {
            player.fly = false;
        }
    }

    fn say(&mut self, s: &str) {
        self.toast = Some((s.to_string(), 2.0));
    }

    /// Dano no jogador local (só vale no Steve sobrevivência).
    pub fn hurt(&mut self, dmg: f32) {
        if !self.active || self.creative || self.dead > 0.0 || dmg <= 0.0 {
            return;
        }
        self.hp -= dmg;
        self.hurt_flash = 1.0;
        self.calm = 0.0;
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.dead = 3.0;
            self.mine = None;
            self.inv_open = false;
        }
    }

    /// Antes de aplicar um evento "w": pavio aceso, fogo novo, reação em cadeia (host), reset.
    pub fn before_world(&mut self, world: &World, m: &Value, is_host: bool, my_id: u64) {
        match m["k"].as_str() {
            Some("set") => {
                let p = &m["p"];
                let p = ivec3(p[0].as_i64().unwrap_or(0) as i32, p[1].as_i64().unwrap_or(0) as i32, p[2].as_i64().unwrap_or(0) as i32);
                if let Some(t) = m["lit"].as_f64() {
                    self.pending.remove(&p);
                    if world.get(p.x, p.y, p.z) == TNT {
                        self.fuses.push(Fuse { p, t: t as f32, owner: m["id"].as_u64().unwrap_or(0) });
                    }
                }
                if m["b"].as_u64() == Some(FIRE as u64) {
                    self.fires.push((p, FIRE_LIFE));
                }
            }
            Some("shot") if is_host && !m["d"].as_bool().unwrap_or(false) => {
                // TNT no raio da explosão acende com pavio curto (só o host conta, depois manda a explosão)
                let plan = mp::get_shot(m);
                let (c, r) = (plan.hit, plan.r + 1.0);
                let ri = r.ceil() as i32;
                let ci = c.floor().as_ivec3();
                for dy in -ri..=ri {
                    for dz in -ri..=ri {
                        for dx in -ri..=ri {
                            let p = ci + ivec3(dx, dy, dz);
                            if world.get(p.x, p.y, p.z) == TNT && (p.as_vec3() + Vec3::splat(0.5)).distance(c) <= r {
                                self.fuses.push(Fuse { p, t: gen_range(0.5, 1.3), owner: my_id });
                            }
                        }
                    }
                }
            }
            Some("reset") => {
                self.fuses.clear();
                self.fires.clear();
                self.pending.clear();
                self.arrows.clear();
            }
            _ => {}
        }
    }

    /// Depois de recarregar o mundo pelo log (entrada): fogo que ficou aceso volta a contar.
    pub fn scan(&mut self, world: &World) {
        self.fires.clear();
        self.fuses.clear();
        for y in 1..WY {
            for z in 0..WZ {
                for x in 0..WX {
                    if world.get(x, y, z) == FIRE {
                        self.fires.push((ivec3(x, y, z), FIRE_LIFE));
                    }
                }
            }
        }
    }

    /// Mensagem "p" de outro jogador: item na mão, flechas que ele atirou, dano PvP em mim.
    /// Retorna (dano, direção) se me acertaram.
    pub fn on_p(&mut self, id: u64, m: &Value, my_id: u64) -> Option<(f32, Vec3)> {
        match m["h"].as_u64() {
            Some(h) => self.held.insert(id, h as Item),
            None => self.held.remove(&id),
        };
        for a in m["ar"].as_array().into_iter().flatten() {
            self.arrows.push(Arrow { pos: mp::get_v3(&a[0]), vel: mp::get_v3(&a[1]), stuck: 0.0, dmg: 0.0, mine: false });
        }
        let (mut dmg, mut dir) = (0.0, Vec3::ZERO);
        for e in m["pv"].as_array().into_iter().flatten() {
            if e[0].as_u64() == Some(my_id) {
                dmg += mp::f(&e[1]);
                dir = mp::get_v3(&e[2]);
            }
        }
        if dmg <= 0.0 {
            return None;
        }
        self.hurt(dmg);
        Some((dmg, dir))
    }

    /// Completa a mensagem "p" que vai sair.
    pub fn fill_p(&mut self, v: &mut Value, ch: u8, sel: usize) {
        if ch == 0 {
            v["h"] = json!(self.inv.slots[sel].0);
        }
        if !self.shots_out.is_empty() {
            v["ar"] = Value::Array(std::mem::take(&mut self.shots_out));
        }
        if !self.pvp_out.is_empty() {
            v["pv"] = Value::Array(std::mem::take(&mut self.pvp_out));
        }
    }

    fn break_block(&mut self, p: IVec3, blk: u8, fx: &mut Fx, avg: &[Color]) {
        self.outbox.push(set_msg(p, AIR));
        for _ in 0..10 {
            fx.particles.push(Particle {
                pos: p.as_vec3() + Vec3::splat(0.5),
                vel: vec3(gen_range(-2.0, 2.0), gen_range(1.0, 4.0), gen_range(-2.0, 2.0)),
                col: avg[face_tile(blk, 0)],
                life: gen_range(0.4, 0.9),
                size: 0.12,
                gravity: true,
            });
        }
        if !self.creative {
            if let Some(d) = items::drop(blk) {
                if !self.inv.add(d, 1) {
                    self.say("INVENTARIO CHEIO");
                }
            }
        }
    }

    /// Ações do Steve controlado (só quando ele é o personagem atual).
    #[allow(clippy::too_many_arguments)]
    pub fn act(&mut self, world: &World, player: &Player, eye: Vec3, fw: Vec3, pick: Option<(IVec3, IVec3, f32)>, targets: &[Target], others: &[(u64, Vec3)], inp: &Input, dt: f32, fx: &mut Fx, avg: &[Color]) {
        if self.dead > 0.0 || self.inv_open {
            self.mine = None;
            self.charge = 0.0;
            self.was_use = false;
            return;
        }
        let sel = player.sel;
        let it = self.inv.slots[sel].0;
        self.mine_cd -= dt;
        let fwh = vec3(fw.x, 0.0, fw.z).normalize_or_zero();
        let wall = pick.map(|p| p.2).unwrap_or(f32::MAX);

        // Golpe: NPC ou jogador na mira tem prioridade sobre o bloco
        let mut hit_something = false;
        if inp.attack {
            self.swing = 1.0;
            let mut best: Option<(f32, Result<(usize, u8), u64>)> = None;
            for &(c, r, i, g) in targets {
                if let Some(t) = urna::ray_sphere(eye, fw, c, r) {
                    if t < REACH + r - 0.75 && t < wall + 0.3 && best.as_ref().is_none_or(|b| t < b.0) {
                        best = Some((t, Ok((i, g))));
                    }
                }
            }
            for &(id, p) in others {
                if let Some(t) = urna::ray_sphere(eye, fw, p + Vec3::Y * 0.9, 0.65) {
                    if t < REACH && t < wall + 0.3 && best.as_ref().is_none_or(|b| t < b.0) {
                        best = Some((t, Err(id)));
                    }
                }
            }
            let dmg = items::melee(it);
            match best {
                Some((t, Ok((i, g)))) => {
                    let k = match g {
                        npc::FIGHTER => "pf",
                        npc::VILLAGER if it != SWORD => "pv",
                        _ => "hit",
                    };
                    let w = if it == SWORD { "ESPADADA" } else { "SOCO" };
                    self.outbox.push(json!({"t": "a", "k": k, "g": g, "i": i, "d": mp::v3(fwh), "p": mp::v3(eye + fw * t), "dmg": dmg, "w": w}));
                    hit_something = true;
                }
                Some((_, Err(id))) => {
                    self.pvp_out.push(json!([id, (dmg * 0.45 * 10.0).round() / 10.0, mp::v3(fwh)]));
                    hit_something = true;
                }
                None => {}
            }
        }

        // Quebrar: segura pra minerar (rachadura), criativo é instantâneo
        match pick {
            Some((p, _, _)) if inp.mine && !hit_something && crate::shield::protected(p) => {
                self.mine = None;
                if inp.attack {
                    self.say("ESCUDO DE ENERGIA: AQUI NADA QUEBRA");
                }
            }
            Some((p, _, _)) if inp.mine && !hit_something && p.y > 0 => {
                let blk = world.get(p.x, p.y, p.z);
                if self.creative {
                    if inp.attack || self.mine_cd <= 0.0 {
                        self.mine_cd = 0.25;
                        self.break_block(p, blk, fx, avg);
                    }
                    self.mine = None;
                } else {
                    let bt = items::break_time(blk, it);
                    let prog = match self.mine {
                        Some((q, k)) if q == p => k,
                        _ => 0.0,
                    } + dt / bt;
                    if gen_range(0.0, 1.0) < dt * 12.0 {
                        fx.particles.push(Particle { pos: p.as_vec3() + Vec3::splat(0.5) - fw * 0.55, vel: vec3(gen_range(-1.5, 1.5), gen_range(0.5, 2.5), gen_range(-1.5, 1.5)), col: avg[face_tile(blk, 0)], life: 0.4, size: 0.07, gravity: true });
                    }
                    if prog >= 1.0 {
                        self.break_block(p, blk, fx, avg);
                        self.mine = None;
                    } else {
                        self.mine = Some((p, prog));
                    }
                }
            }
            _ => self.mine = None,
        }
        if inp.mine && pick.is_some() && self.swing <= 0.0 {
            self.swing = 1.0;
        }

        // Usar: põe bloco, acende TNT/fogo, puxa o arco
        if inp.use_press {
            if let Some((p, prev, _)) = pick {
                let at = world.get(prev.x, prev.y, prev.z);
                if (is_block(it) || it == FLINT) && (crate::shield::protected(prev) || (it == FLINT && crate::shield::protected(p))) {
                    self.say("ESCUDO DE ENERGIA: AQUI NADA MUDA");
                } else if is_block(it) && matches!(at, AIR | FIRE) {
                    let (lo, hi) = (prev.as_vec3(), prev.as_vec3() + Vec3::ONE);
                    let pp = player.pos;
                    let inside = pp.x + 0.3 > lo.x && pp.x - 0.3 < hi.x && pp.y + 1.79 > lo.y && pp.y < hi.y && pp.z + 0.3 > lo.z && pp.z - 0.3 < hi.z;
                    if !inside {
                        self.outbox.push(set_msg(prev, it as u8));
                        if !self.creative {
                            self.inv.use_slot(sel);
                        }
                    }
                } else if it == FLINT {
                    if world.get(p.x, p.y, p.z) == TNT {
                        self.outbox.push(lit_msg(p, FUSE));
                    } else if at == AIR {
                        self.outbox.push(set_msg(prev, FIRE));
                    }
                }
            }
        }
        if it == BOW && inp.use_hold {
            self.charge = (self.charge + dt).min(1.0);
        } else {
            if it == BOW && self.was_use && self.charge > 0.12 {
                if self.creative || self.inv.take(ARROW) {
                    let o = eye + fw * 0.4;
                    let v = fw * (14.0 + 40.0 * self.charge);
                    self.arrows.push(Arrow { pos: o, vel: v, stuck: 0.0, dmg: 4.0 + 16.0 * self.charge, mine: true });
                    self.shots_out.push(json!([mp::v3(o), mp::v3(v)]));
                } else {
                    self.say("SEM FLECHA");
                }
            }
            self.charge = 0.0;
        }
        self.was_use = inp.use_hold;
    }

    /// Todo frame (mesmo sem ser o Steve): vida, pavios, fogo, flechas.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, world: &World, player: &mut Player, active: bool, is_host: bool, my_id: u64, targets: &[Target], others: &[(u64, Vec3)], dt: f32, fx: &mut Fx) {
        self.active = active;
        self.swing = (self.swing - dt * 4.0).max(0.0);
        self.equip = (self.equip - dt * 6.0).max(0.0);
        self.hurt_flash = (self.hurt_flash - dt * 2.0).max(0.0);
        if let Some((_, t)) = self.toast.as_mut() {
            *t -= dt;
        }
        self.toast = self.toast.take().filter(|t| t.1 > 0.0);
        if !active {
            self.inv_open = false;
            self.mine = None;
        }
        if active && player.sel != self.last_sel {
            self.last_sel = player.sel;
            self.equip = 1.0;
            let n = items::name(self.inv.slots[player.sel].0);
            if !n.is_empty() {
                self.toast = Some((n.to_string(), 1.5));
            }
        }

        // Sobrevivência: queda, fogo, regeneração, morte
        if active && !self.creative {
            player.fly = false;
            if self.dead > 0.0 {
                self.dead -= dt;
                player.pos = *self.death_pos.get_or_insert(player.pos);
                player.vel = Vec3::ZERO;
                if self.dead <= 0.0 {
                    self.death_pos = None;
                    player.pos = Player::spawn();
                    self.set_mode(false, player);
                }
            } else {
                if player.on_ground && !self.was_ground {
                    let h = self.last_vy * self.last_vy / 56.0;
                    if self.last_vy < 0.0 && h > 3.5 {
                        self.hurt(h - 3.5);
                    }
                }
                self.fire_cd -= dt;
                let feet = player.pos.floor().as_ivec3();
                if self.fire_cd <= 0.0 && (world.get(feet.x, feet.y, feet.z) == FIRE || world.get(feet.x, feet.y + 1, feet.z) == FIRE) {
                    self.fire_cd = 0.5;
                    self.hurt(1.0);
                }
                self.calm += dt;
                if self.calm > 4.0 && self.hp < MAX_HP {
                    self.hp = (self.hp + dt * 0.5).min(MAX_HP);
                }
            }
        }
        self.was_ground = player.on_ground;
        self.last_vy = player.vel.y;

        // Pavios: quem acendeu manda a explosão (se saiu, o host assume)
        for f in self.fuses.iter_mut() {
            f.t -= dt;
            if f.t <= 0.0 && (f.owner == my_id || (is_host && !others.iter().any(|o| o.0 == f.owner))) {
                let c = f.p.as_vec3() + Vec3::splat(0.5);
                let mut v = mp::shot(&urna::Plan { o: c, hit: c, r: 3.5, deflect: false });
                v["by"] = json!(BY);
                self.outbox.push(v);
            }
        }
        self.fuses.retain(|f| f.t > 0.0);

        // Fogo: chama, acende TNT vizinha e apaga sozinho (host decide)
        for (p, t) in self.fires.iter_mut() {
            *t -= dt;
            if gen_range(0.0, 1.0) < dt * 8.0 {
                fx.particles.push(Particle { pos: p.as_vec3() + vec3(gen_range(0.2, 0.8), 0.3, gen_range(0.2, 0.8)), vel: vec3(0.0, gen_range(1.0, 2.0), 0.0), col: Color::new(0.4, 0.4, 0.4, 1.0), life: 0.8, size: 0.15, gravity: false });
            }
            if !is_host {
                continue;
            }
            for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                let q = *p + d;
                if world.get(q.x, q.y, q.z) == TNT && !self.pending.contains(&q) && !self.fuses.iter().any(|f| f.p == q) {
                    self.pending.insert(q);
                    self.outbox.push(lit_msg(q, FUSE));
                }
            }
            if *t <= 0.0 && world.get(p.x, p.y, p.z) == FIRE {
                self.outbox.push(set_msg(*p, AIR));
            }
        }
        self.fires.retain(|(p, t)| *t > 0.0 && world.get(p.x, p.y, p.z) == FIRE);

        // Flechas: gravidade, cravam no bloco; as minhas acertam NPC e jogador
        let mut hits = Vec::new();
        for a in self.arrows.iter_mut() {
            if a.stuck > 0.0 {
                a.stuck -= dt;
                continue;
            }
            a.vel.y -= 20.0 * dt;
            let step = a.vel * dt;
            let len = step.length();
            let dir = step / len.max(1e-5);
            if a.mine {
                let mut best: Option<(f32, Result<(usize, u8), u64>)> = None;
                for &(c, r, i, g) in targets {
                    if let Some(t) = urna::ray_sphere(a.pos, dir, c, r) {
                        if t < len && best.as_ref().is_none_or(|b| t < b.0) {
                            best = Some((t, Ok((i, g))));
                        }
                    }
                }
                for &(id, p) in others {
                    if let Some(t) = urna::ray_sphere(a.pos, dir, p + Vec3::Y * 0.9, 0.6) {
                        if t < len && best.as_ref().is_none_or(|b| t < b.0) {
                            best = Some((t, Err(id)));
                        }
                    }
                }
                if let Some((t, who)) = best {
                    hits.push((who, a.pos + dir * t, vec3(dir.x, 0.0, dir.z).normalize_or_zero(), a.dmg));
                    a.stuck = -1.0;
                    continue;
                }
            }
            if let Some((_, _, t)) = world.raycast(a.pos, dir, len) {
                a.pos += dir * t;
                a.stuck = 15.0;
                a.vel = dir;
            } else {
                a.pos += step;
                if a.pos.y < -10.0 {
                    a.stuck = -1.0;
                }
            }
        }
        self.arrows.retain(|a| a.stuck >= 0.0);
        for (who, at, dir, dmg) in hits {
            match who {
                Ok((i, g)) => self.outbox.push(json!({"t": "a", "k": "hit", "g": g, "i": i, "d": mp::v3(dir), "p": mp::v3(at), "dmg": dmg, "w": "FLECHA"})),
                Err(id) => self.pvp_out.push(json!([id, (dmg * 0.45 * 10.0).round() / 10.0, mp::v3(dir)])),
            }
            for _ in 0..5 {
                fx.particles.push(Particle { pos: at, vel: dir * 2.0 + vec3(gen_range(-1.5, 1.5), gen_range(0.5, 2.5), gen_range(-1.5, 1.5)), col: Color::new(0.6, 0.05, 0.05, 1.0), life: 0.5, size: 0.08, gravity: true });
            }
        }
    }

    /// Flechas, TNT acesa, fogo e item na mão dos outros Steves (o meu vai no `draw_hud`, em viewmodel).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_world(&mut self, b: &mut Batch, time: f32, _eye: Vec3, _fw: Vec3, _me_steve: bool, _sel: usize, remotes: impl Iterator<Item = (u64, Vec3, f32, u8)>, _avg: &[Color]) {
        // Flecha = sprite cruzado em X, alinhado com a velocidade (cravada guarda a direção do impacto)
        for a in &self.arrows {
            let d = a.vel.normalize_or(Vec3::NEG_Y);
            let tip = if a.stuck > 0.0 { a.pos + d * 0.12 } else { a.pos };
            let base = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::X, d), tip - d * 0.27) * Mat4::from_scale(Vec3::splat(0.5));
            for roll in [0.0, FRAC_PI_2] {
                items::draw_model(b, &mut self.tm, &(base * Mat4::from_rotation_x(roll) * Mat4::from_rotation_z(-FRAC_PI_4)), ARROW, 0);
            }
        }
        // TNT acesa: pisca branco a cada 0,25 s e incha no fim do pavio
        for f in &self.fuses {
            let k = (1.0 - f.t / 0.5).clamp(0.0, 1.0).powi(4);
            let m = Mat4::from_scale_rotation_translation(Vec3::splat(1.0 + k * 0.3), Quat::IDENTITY, f.p.as_vec3() + Vec3::splat(0.5));
            self.tm.block(&m, TNT, WHITE);
            if (f.t * 4.0) as i32 % 2 == 0 {
                self.tm.cube(&(m * Mat4::from_scale(Vec3::splat(1.004))), |_| crate::atlas::T_WHITE, Color::new(1.0, 1.0, 1.0, 0.7));
            }
        }
        // Fogo: 4 planos cruzados de línguas de chama que tremulam
        for (p, _) in &self.fires {
            let base = p.as_vec3() + vec3(0.5, 0.0, 0.5);
            for pl in 0..4 {
                let m = Mat4::from_translation(base) * Mat4::from_rotation_y(pl as f32 * FRAC_PI_4);
                for c in 0..5 {
                    let h0 = crate::atlas::hash2(p.x * 7 + pl * 5 + c, p.z * 3 + p.y, 61);
                    let fl = 0.5 + 0.5 * (time * (8.0 + h0 * 7.0) + h0 * 40.0).sin();
                    let h = (0.4 + 0.55 * fl) * (1.0 - (c as f32 - 2.0).abs() * 0.2);
                    let x = (c as f32 - 2.0) * 0.18 + (time * 5.0 + h0 * 9.0).sin() * 0.015;
                    for (lo, hi, col) in [(0.0, 0.45, Color::new(1.0, 0.92, 0.35, 1.0)), (0.45, 0.8, Color::new(1.0, 0.55, 0.08, 1.0)), (0.8, 1.0, Color::new(0.85, 0.2, 0.04, 1.0))] {
                        b.glow(&m, vec3(x, h * (lo + hi) * 0.5, 0.0), vec3(0.17, h * (hi - lo), 0.02), col);
                    }
                }
            }
        }
        // Item na mão direita dos outros Steves, preso no braço (mesmo ângulo do braço do draw_humanoid)
        for (id, pos, yaw, ch) in remotes {
            let Some(&it) = self.held.get(&id).filter(|_| ch == 0) else { continue };
            // Igual ao ItemInHandLayer do Minecraft: eixos do item (x = direita, y = frente, z = cima) no ombro
            // girado do braço, 1 unidade = 16 px do Steve, depois o display "thirdperson_righthand" do item.
            let shoulder = root(pos, yaw, 0.0, 0.0) * Mat4::from_translation(vec3(-6.0 * U, 22.0 * U, 0.0)) * Mat4::from_rotation_x(REMOTE_ARM);
            let axes = Mat4::from_cols(Vec4::NEG_X, Vec4::Z, Vec4::Y, Vec4::W);
            let base = shoulder * Mat4::from_scale(Vec3::splat(16.0 * U)) * axes * Mat4::from_translation(vec3(1.0, 2.0, -10.0) / 16.0);
            let deg = f32::to_radians;
            let (tr, rot, s) = match it {
                _ if is_block(it) => (vec3(0.0, 2.5, 0.0), vec3(75.0, 45.0, 0.0), 0.375),
                BOW => (vec3(-1.0, -2.0, 2.5), vec3(-80.0, 260.0, -40.0), 0.9),
                ARROW | FLINT => (vec3(0.0, 3.0, 1.0), Vec3::ZERO, 0.55),
                _ => (vec3(0.0, 4.0, 0.5), vec3(0.0, -90.0, 55.0), 0.85),
            };
            let display = Mat4::from_translation(tr / 16.0) * Mat4::from_euler(EulerRot::XYZ, deg(rot.x), deg(rot.y), deg(rot.z)) * Mat4::from_scale(Vec3::splat(s));
            items::draw_model(b, &mut self.tm, &(base * display), it, 0);
        }
    }

    /// Depois do flush do batch: cubos texturizados (TNT acesa, blocos na mão) e rachadura do bloco minerado.
    pub fn draw_crack(&mut self) {
        self.tm.flush(&self.tex);
        if let Some((p, k)) = self.mine {
            let stage = ((k * 8.0) as usize).min(7);
            draw_cube(p.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.004), Some(&self.cracks[stage]), WHITE);
        }
    }

    /// Item na mão em primeira pessoa: câmera própria na origem, depth limpo (não entra na parede nem
    /// pega neblina/portal). Transformações do Minecraft (braço, golpe, arco puxado, display do item).
    fn draw_viewmodel(&self, atlas: &Atlas, it: Item) {
        if self.dead > 0.0 || self.inv_open {
            return;
        }
        // Cena girada pra luz fixa do Batch vir de trás/esquerda da câmera (face do item iluminada)
        let w = Mat4::from_quat(Quat::from_rotation_arc(vec3(-0.7, 0.3, 0.65).normalize(), vec3(0.4, 1.0, 0.3).normalize()));
        set_camera(&Camera3D { position: Vec3::ZERO, target: w.transform_vector3(Vec3::NEG_Z), up: w.transform_vector3(Vec3::Y), fovy: 70f32.to_radians(), ..Default::default() });
        let gl = unsafe { get_internal_gl() };
        gl.quad_context.begin_default_pass(PassAction::Clear { color: None, depth: Some(1.0), stencil: None });
        gl.quad_context.end_render_pass();
        let t = Mat4::from_translation;
        let (rx, ry, rz) = (|d: f32| Mat4::from_rotation_x(d.to_radians()), |d: f32| Mat4::from_rotation_y(d.to_radians()), |d: f32| Mat4::from_rotation_z(d.to_radians()));
        let s = if self.swing > 0.0 { 1.0 - self.swing } else { 0.0 };
        let sq = s.sqrt();
        let swing = w * t(vec3(-0.4 * (sq * PI).sin(), 0.2 * (sq * TAU).sin(), -0.2 * (s * PI).sin()));
        let arm = t(vec3(0.56, -0.52 - 0.6 * self.equip, -0.72));
        let attack = ry(45.0 - (s * s * PI).sin() * 20.0) * rz(-(sq * PI).sin() * 20.0) * rx(-(sq * PI).sin() * 80.0) * ry(-45.0);
        let (mut b, mut tm) = (Batch::new(), TexMesh::new());
        let drawing = it == BOW && self.charge > 0.0;
        let m = if drawing {
            let f = (self.charge * self.charge + self.charge * 2.0) / 3.0;
            let shake = if f > 0.9 { (get_time() as f32 * 40.0).sin() * 0.004 } else { 0.0 };
            w * arm * t(vec3(-0.2785682, 0.18344387 + shake, 0.15731531)) * rx(-13.935) * ry(35.3) * rz(-9.785) * t(vec3(0.0, 0.0, f * 0.04)) * Mat4::from_scale(vec3(1.0, 1.0, 1.0 + f * 0.2)) * ry(-45.0)
        } else {
            swing * arm * attack
        };
        match it {
            NONE => {
                // Mão vazia: braço do Steve saindo do canto
                let m = swing * t(vec3(0.38, -0.32 - 0.6 * self.equip, -0.7)) * rx(-(sq * PI).sin() * 30.0) * Mat4::from_quat(Quat::from_rotation_arc(Vec3::NEG_Y, vec3(0.35, -0.5, 0.8).normalize()));
                b.cube(&m, vec3(0.0, -0.375, 0.0), vec3(0.25, 0.75, 0.25), Color::new(0.86, 0.66, 0.52, 1.0));
            }
            _ if is_block(it) => tm.block(&(m * ry(45.0) * Mat4::from_scale(Vec3::splat(0.4))), it as u8, WHITE),
            _ => {
                let pull = if drawing { 1 + (self.charge * 2.99) as u8 } else { 0 };
                let m = m * t(vec3(1.13, 3.2, 1.13) / 16.0) * ry(-90.0) * rz(25.0) * Mat4::from_scale(Vec3::splat(0.68));
                items::draw_model(&mut b, &mut tm, &m, it, pull);
            }
        }
        b.flush(&atlas.tex);
        tm.flush(&atlas.tex);
        set_default_camera();
    }

    /// HUD: hotbar, corações, carga do arco, nome do item.
    pub fn draw_hud(&self, atlas: &Atlas, sel: usize, sw: f32, sh: f32, slot: f32, mobile: bool) {
        self.draw_viewmodel(atlas, self.inv.slots[sel].0);
        if self.hurt_flash > 0.0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.8, 0.0, 0.0, 0.25 * self.hurt_flash));
        }
        inventory::draw_hotbar(&self.inv, atlas, sel, sw, sh, slot, self.creative, mobile);
        let o = inventory::hotbar_origin(sw, sh, slot);
        if !self.creative {
            const HEART: [&str; 6] = [".##.##.", "#######", "#######", ".#####.", "..###..", "...#..."];
            let px = (slot * 0.055).max(2.0);
            for k in 0..10 {
                let x0 = o.x + k as f32 * px * 8.0;
                let y0 = o.y - px * 8.0;
                let fill = (self.hp - k as f32 * 2.0).clamp(0.0, 2.0);
                let shake = if self.hp <= 4.0 { ((get_time() * 30.0 + k as f64).sin() as f32) * px * 0.4 } else { 0.0 };
                for (r, row) in HEART.iter().enumerate() {
                    for (c, ch) in row.bytes().enumerate() {
                        if ch == b'#' {
                            let red = fill >= 2.0 || (fill >= 1.0 && c < 4);
                            let col = if red { Color::new(0.9, 0.1, 0.12, 1.0) } else { Color::new(0.15, 0.15, 0.15, 0.85) };
                            draw_rectangle(x0 + c as f32 * px, y0 + r as f32 * px + shake, px + 0.4, px + 0.4, col);
                        }
                    }
                }
            }
            let arrows = self.inv.count(ARROW);
            if self.inv.slots[sel].0 == BOW {
                let t = format!("FLECHAS: {arrows}");
                draw_text(&t, o.x + slot * 9.0 - measure_text(&t, None, 18, 1.0).width, o.y - 8.0, 18.0, WHITE);
            }
        } else {
            draw_text("CRIATIVO", o.x, o.y - 8.0, 18.0, Color::new(0.6, 0.9, 1.0, 1.0));
        }
        if self.charge > 0.0 {
            let w = slot * 3.0;
            draw_rectangle(sw * 0.5 - w * 0.5, sh * 0.5 + 24.0, w, 6.0, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_rectangle(sw * 0.5 - w * 0.5, sh * 0.5 + 24.0, w * self.charge, 6.0, if self.charge >= 1.0 { YELLOW } else { WHITE });
        }
        if let Some((s, t)) = &self.toast {
            let a = t.min(1.0);
            let d = measure_text(s, None, 22, 1.0);
            draw_text(s, sw * 0.5 - d.width * 0.5 + 1.5, o.y - slot * 0.75 + 1.5, 22.0, Color::new(0.0, 0.0, 0.0, 0.7 * a));
            draw_text(s, sw * 0.5 - d.width * 0.5, o.y - slot * 0.75, 22.0, Color::new(1.0, 1.0, 1.0, a));
        }
    }

    /// Por cima de tudo: tela do inventário e tela de morte.
    pub fn draw_overlay(&self, atlas: &Atlas, sel: usize, sw: f32, sh: f32, mobile: bool) {
        if self.inv_open {
            inventory::draw_screen(&self.inv, atlas, sel, sw, sh, self.creative, mobile);
        }
        if self.dead > 0.0 {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.45, 0.0, 0.0, 0.5));
            let s = (sh * 0.1).max(36.0);
            let d = measure_text("VOCE MORREU!", None, s as u16, 1.0);
            draw_text("VOCE MORREU!", sw * 0.5 - d.width * 0.5, sh * 0.45, s, WHITE);
            let t = format!("RENASCENDO EM {:.0}...", self.dead.ceil());
            let d = measure_text(&t, None, 24, 1.0);
            draw_text(&t, sw * 0.5 - d.width * 0.5, sh * 0.45 + 40.0, 24.0, WHITE);
        }
    }

    /// Clique/toque com o inventário aberto; fora da janela fecha.
    pub fn inv_click(&mut self, sel: &mut usize, p: Vec2, sw: f32, sh: f32) {
        if !inventory::click(&mut self.inv, sel, p, sw, sh, self.creative) {
            self.inv_open = false;
        }
    }
}
