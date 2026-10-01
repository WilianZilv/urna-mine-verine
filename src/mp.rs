//! Protocolo multiplayer (JSON). O host simula lutadores/villagers/urna e manda snapshots;
//! mudanças no mundo (blocos, tiros, reset) passam pelo servidor pra todos aplicarem na mesma ordem.

use crate::actors::{Atk, Ev, FState, Fighter, Villager};
use crate::urna::{Plan, Urna};
use macroquad::prelude::*;
use serde_json::{Value, json};

fn r2(x: f32) -> f64 {
    (x as f64 * 100.0).round() / 100.0
}

pub fn v3(v: Vec3) -> Value {
    json!([r2(v.x), r2(v.y), r2(v.z)])
}

pub fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

pub fn get_v3(v: &Value) -> Vec3 {
    vec3(f(&v[0]), f(&v[1]), f(&v[2]))
}

pub fn shot(p: &Plan) -> Value {
    json!({"t": "w", "k": "shot", "o": v3(p.o), "h": v3(p.hit), "r": r2(p.r), "d": p.deflect})
}

pub fn get_shot(m: &Value) -> Plan {
    Plan { o: get_v3(&m["o"]), hit: get_v3(&m["h"]), r: f(&m["r"]), deflect: m["d"].as_bool().unwrap_or(false) }
}

const ATKS: [Atk; 5] = [Atk::Jab, Atk::Cross, Atk::Combo3, Atk::Finisher, Atk::Special];

fn fighter(x: &Fighter) -> Value {
    let (code, a, b) = match x.state {
        FState::Idle => (0, 0.0, 0.0),
        FState::Attack { kind, t, .. } => (1, kind as usize as f32, t),
        FState::Stun(t) => (2, t, 0.0),
        FState::Dodge(t, _) => (3, t, 0.0),
        FState::Ko(t) => (4, t, 0.0),
        FState::Entering => (5, 0.0, 0.0),
    };
    json!([r2(x.pos.x), r2(x.pos.y), r2(x.pos.z), r2(x.yaw), r2(x.hp), code, r2(a), r2(b), r2(x.walk), r2(x.walk_amt), r2(x.flash), r2(x.berserk), r2(x.ko_t), x.kos, x.spawned])
}

/// Aplica estado do host; retorna posição alvo (interpolada no main).
fn apply_fighter(x: &mut Fighter, v: &Value) -> Vec3 {
    x.yaw = f(&v[3]);
    x.hp = f(&v[4]);
    let (a, b) = (f(&v[6]), f(&v[7]));
    x.state = match v[5].as_i64().unwrap_or(0) {
        1 => FState::Attack { kind: ATKS[(a as usize).min(4)], t: b, done: true },
        2 => FState::Stun(a),
        3 => FState::Dodge(a, Vec3::ZERO),
        4 => FState::Ko(a),
        5 => FState::Entering,
        _ => FState::Idle,
    };
    x.walk = f(&v[8]);
    x.walk_amt = f(&v[9]);
    x.flash = f(&v[10]);
    x.berserk = f(&v[11]);
    x.ko_t = f(&v[12]);
    x.kos = v[13].as_u64().unwrap_or(0) as u32;
    let was = x.spawned;
    x.spawned = v[14].as_bool().unwrap_or(false);
    let p = vec3(f(&v[0]), f(&v[1]), f(&v[2]));
    if !was {
        x.pos = p;
    }
    p
}

fn villager(x: &Villager) -> Value {
    json!([r2(x.pos.x), r2(x.pos.y), r2(x.pos.z), r2(x.yaw), r2(x.spin), r2(x.walk)])
}

fn ev(e: &Ev) -> Value {
    match e {
        Ev::Hit { pos, claws } => json!({"k": "h", "p": v3(*pos), "c": claws}),
        Ev::Snikt(p) => json!({"k": "s", "p": v3(*p)}),
        Ev::Text { pos, text, color, big } => json!({"k": "t", "p": v3(*pos), "m": text, "c": [r2(color.r), r2(color.g), r2(color.b)], "b": big}),
        Ev::Shake(a) => json!({"k": "k", "a": r2(*a)}),
        Ev::Banner(s) => json!({"k": "b", "m": s}),
    }
}

fn get_ev(v: &Value) -> Option<Ev> {
    let p = get_v3(&v["p"]);
    let s = || v["m"].as_str().unwrap_or("").to_string();
    Some(match v["k"].as_str()? {
        "h" => Ev::Hit { pos: p, claws: v["c"].as_bool().unwrap_or(false) },
        "s" => Ev::Snikt(p),
        "t" => Ev::Text { pos: p, text: s(), color: Color::new(f(&v["c"][0]), f(&v["c"][1]), f(&v["c"][2]), 1.0), big: v["b"].as_bool().unwrap_or(false) },
        "k" => Ev::Shake(f(&v["a"])),
        "b" => Ev::Banner(s()),
        _ => return None,
    })
}

pub fn snapshot(time: f32, urna: &Urna, fs: &[Fighter], vs: &[Villager], evs: &[Ev]) -> Value {
    json!({
        "t": "s",
        "time": time,
        "u": [r2(urna.yaw), urna.charging, r2(urna.charge), urna.shots, r2(urna.root.x), r2(urna.root.z), v3(urna.target)],
        "f": fs.iter().map(fighter).collect::<Vec<_>>(),
        "v": vs.iter().map(villager).collect::<Vec<_>>(),
        "e": evs.iter().map(ev).collect::<Vec<_>>(),
    })
}

/// Posições alvo dos lutadores e villagers ficam em `fpos`/`vpos` pra interpolar.
pub fn apply_snapshot(m: &Value, urna: &mut Urna, fs: &mut [Fighter], vs: &mut [Villager], fpos: &mut Vec<Vec3>, vpos: &mut Vec<Vec3>, evs: &mut Vec<Ev>) {
    let u = &m["u"];
    urna.yaw = f(&u[0]);
    urna.charging = u[1].as_bool().unwrap_or(false);
    urna.charge = f(&u[2]);
    urna.shots = u[3].as_u64().unwrap_or(0) as u32;
    if u[4].is_number() {
        urna.root.x = f(&u[4]);
        urna.root.z = f(&u[5]);
        urna.target = get_v3(&u[6]);
    }
    fpos.resize(fs.len(), Vec3::ZERO);
    for (i, v) in m["f"].as_array().into_iter().flatten().enumerate().take(fs.len()) {
        fpos[i] = apply_fighter(&mut fs[i], v);
    }
    vpos.resize(vs.len(), Vec3::ZERO);
    for (i, v) in m["v"].as_array().into_iter().flatten().enumerate().take(vs.len()) {
        let x = &mut vs[i];
        vpos[i] = vec3(f(&v[0]), f(&v[1]), f(&v[2]));
        x.yaw = f(&v[3]);
        x.spin = f(&v[4]);
        x.walk = f(&v[5]);
    }
    evs.extend(m["e"].as_array().into_iter().flatten().filter_map(get_ev));
}
