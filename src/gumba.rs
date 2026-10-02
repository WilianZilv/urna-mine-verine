//! COGUMAU, o cogumelo invocado da arena (paródia original do "bichinho que anda e morre no pisão"):
//! chapéu roxo de pintas laranja, pé creme, monocelha brava, um dentão e botinha verde. Patrulha num
//! vai-e-vem, vira em parede/beirada, morde quem encosta de lado; pisão por cima (caindo, pés acima da
//! cabeça) amassa feito panqueca e quica quem pisou. O host anda com eles ("gb" no snapshot); a vida e o
//! renascer são do grupo `npc::GUMBA`, então tiro, bomba e explosão também matam.

use crate::batch::Batch;
use crate::gta::Target;
use crate::models::rgb;
use crate::npc::{self, Npcs};
use crate::world::{World, arena_center};
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::{PI, TAU};

pub const N: usize = 6;
const R: f32 = 0.42;
const H: f32 = 0.85;
const SPD: f32 = 1.5;
/// Até onde vai de cada lado da casa antes de dar meia-volta.
const SPAN: f32 = 4.5;
const PANCAKE: f32 = 0.5;
const STOMP_DMG: f32 = 50.0;

/// Pisão: caindo, pés na faixa do topo da cabeça e dentro do raio.
pub fn stomps(feet: Vec3, vy: f32, top: Vec3) -> bool {
    vy < -1.0 && feet.y > top.y - 0.35 && feet.y < top.y + 0.7 && vec2(feet.x - top.x, feet.z - top.z).length() < R + 0.3
}

/// Encostou de lado: pés abaixo do topo e corpo cruzando a altura dele.
pub fn bumps(feet: Vec3, base: Vec3) -> bool {
    feet.y < base.y + H - 0.35 && feet.y + 1.6 > base.y && vec2(feet.x - base.x, feet.z - base.z).length() < R + 0.3
}

/// Casa e direção da patrulha (mesma em todo cliente).
fn home(k: usize) -> (Vec3, f32) {
    let a = k as f32 * TAU / N as f32 + 0.4;
    let r = 8.0 + (k % 3) as f32 * 3.5;
    (arena_center() + vec3(a.cos() * r, 0.0, a.sin() * r), a + PI * 0.5)
}

struct Gumba {
    pos: Vec3,
    yaw: f32,
    vy: f32,
    walk: f32,
    net: Vec3,
    /// Pisão previsto aqui antes do host confirmar (1 -> 0).
    pred: f32,
}

impl Gumba {
    fn new(k: usize) -> Self {
        let (pos, yaw) = home(k);
        Gumba { pos, yaw, vy: 0.0, walk: 0.0, net: pos, pred: 0.0 }
    }
}

/// O que o jogador local fez/sofreu com eles neste frame.
#[derive(Default)]
pub struct Touch {
    pub hurt: f32,
    pub dir: Vec3,
    pub bounce: bool,
    pub out: Vec<Value>,
}

pub struct Gumbas {
    list: Vec<Gumba>,
    was: Vec<bool>,
    hurt_cd: f32,
}

impl Gumbas {
    pub fn new() -> Self {
        Gumbas { list: (0..N).map(Gumba::new).collect(), was: vec![true; N], hurt_cd: 0.0 }
    }

    /// Host: anda, cai, vira em parede/beirada/fim da ronda; quem renasceu volta pra casa.
    pub fn think(&mut self, world: &World, dt: f32, npcs: &Npcs) {
        for (k, g) in self.list.iter_mut().enumerate() {
            let alive = npcs.alive(npc::GUMBA, k);
            if alive && !self.was[k] {
                *g = Gumba::new(k);
                g.pos.y += 3.0;
            }
            self.was[k] = alive;
            if !alive {
                continue;
            }
            g.vy = (g.vy - 25.0 * dt).max(-20.0);
            g.pos.y += g.vy * dt;
            let floor = world.floor_at(g.pos.x, g.pos.y + 0.5, g.pos.z);
            let ground = g.pos.y <= floor;
            if ground {
                g.pos.y = floor;
                g.vy = 0.0;
            }
            let f = vec3(g.yaw.sin(), 0.0, g.yaw.cos());
            let probe = g.pos + f * (R + 0.15);
            let wall = world.solid_f(probe.x, g.pos.y + 0.3, probe.z);
            let edge = ground && !world.solid_f(probe.x, g.pos.y - 0.5, probe.z) && !world.solid_f(probe.x, g.pos.y - 1.5, probe.z);
            let far = (g.pos - home(k).0).dot(f) > SPAN || !crate::layout::in_arena(vec2(probe.x, probe.z), 3.0);
            if wall || edge || far {
                g.yaw = (g.yaw + PI).rem_euclid(TAU);
            } else {
                g.pos += f * SPD * dt;
                g.walk += SPD * dt * 7.0;
            }
        }
    }

    /// Todos: cliente segue o host; previsão de pisão expira.
    pub fn animate(&mut self, dt: f32, is_host: bool) {
        self.hurt_cd -= dt;
        for g in &mut self.list {
            g.pred = (g.pred - dt).max(0.0);
            if is_host {
                continue;
            }
            let d = g.net - g.pos;
            if d.length() > 6.0 {
                g.pos = g.net;
            } else {
                let s = d * (dt * 10.0).min(1.0);
                g.walk += vec2(s.x, s.z).length() * 7.0;
                g.pos += s;
            }
        }
    }

    pub fn snapshot(&self) -> Value {
        let r = |x: f32| (x as f64 * 100.0).round() / 100.0;
        Value::Array(self.list.iter().map(|g| json!([r(g.pos.x), r(g.pos.y), r(g.pos.z), r(g.yaw)])).collect())
    }

    pub fn apply(&mut self, v: &Value) {
        for (g, a) in self.list.iter_mut().zip(v.as_array().into_iter().flatten()) {
            let f = |i: usize| a[i].as_f64().unwrap_or(0.0) as f32;
            (g.net, g.yaw) = (vec3(f(0), f(1), f(2)), f(3));
        }
    }

    /// Vivos pra acertar (tiro, soco, explosão, pisão do ENCANADOR): topo = centro + raio.
    pub fn targets(&self, npcs: &Npcs) -> Vec<Target> {
        self.list.iter().enumerate().filter(|(k, g)| npcs.alive(npc::GUMBA, *k) && g.pred <= 0.0).map(|(k, g)| (g.pos + Vec3::Y * 0.4, 0.45, k, npc::GUMBA)).collect()
    }

    /// Jogador local: pisão (manda o golpe pro host e já amassa aqui) ou mordida de lado.
    pub fn touch(&mut self, npcs: &Npcs, feet: Vec3, vy: f32, can_stomp: bool) -> Touch {
        let mut t = Touch::default();
        for (k, g) in self.list.iter_mut().enumerate() {
            if !npcs.alive(npc::GUMBA, k) || g.pred > 0.0 {
                continue;
            }
            let top = g.pos + Vec3::Y * H;
            if can_stomp && stomps(feet, vy, top) {
                g.pred = 1.0;
                t.bounce = true;
                t.out.push(json!({"t": "a", "k": "hit", "g": npc::GUMBA, "i": k, "d": crate::mp::v3(-Vec3::Y), "p": crate::mp::v3(top), "dmg": STOMP_DMG, "w": "PISAO"}));
            } else if self.hurt_cd <= 0.0 && bumps(feet, g.pos) {
                self.hurt_cd = 1.0;
                t.hurt = 1.0;
                t.dir = vec3(feet.x - g.pos.x, 0.0, feet.z - g.pos.z).normalize_or(Vec3::X);
            }
        }
        t
    }

    pub fn draw(&self, b: &mut Batch, npcs: &Npcs, eye: Vec3) {
        let far = crate::quality::pick([35.0, 60.0, 100.0]);
        let low = crate::quality::tier() == crate::quality::LOW;
        for (k, g) in self.list.iter().enumerate() {
            let Some(v) = npcs.get(npc::GUMBA, k) else { continue };
            if g.pos.distance(eye) > far {
                continue;
            }
            let squash = if g.pred > 0.0 { Some(1.0 - g.pred) } else { (!v.alive()).then_some(v.t) };
            if squash.is_some_and(|t| t >= PANCAKE) {
                continue;
            }
            draw_one(b, g, squash.is_some(), v.flash, low);
        }
    }
}

fn draw_one(b: &mut Batch, g: &Gumba, flat: bool, flash: f32, low: bool) {
    let tint = |c: Color| Color::new(c.r + (1.0 - c.r) * flash * 0.7, c.g + (1.0 - c.g) * flash * 0.4, c.b + (1.0 - c.b) * flash * 0.4, 1.0);
    let hop = g.walk.sin().abs() * 0.06;
    let s = if flat { vec3(1.45, 0.18, 1.45) } else { vec3(1.0 + hop * 0.5, 1.0 - hop, 1.0 + hop * 0.5) };
    let m = Mat4::from_translation(g.pos) * Mat4::from_rotation_y(g.yaw) * Mat4::from_scale(s);
    let cap = tint(rgb(0.46, 0.12, 0.52));
    let stem = tint(rgb(0.96, 0.88, 0.7));
    let boot = tint(rgb(0.12, 0.38, 0.18));
    let ink = rgb(0.08, 0.04, 0.04);
    let swing = if flat { 0.0 } else { g.walk.sin() * 0.12 };
    for side in [1.0f32, -1.0] {
        b.cube(&m, vec3(side * 0.15, 0.07, 0.04 + swing * side), vec3(0.16, 0.14, 0.26), boot);
    }
    b.cube(&m, vec3(0.0, 0.3, 0.0), vec3(0.5, 0.34, 0.46), stem);
    b.cube(&m, vec3(0.0, 0.6, 0.0), vec3(0.86, 0.26, 0.86), cap);
    b.cube(&m, vec3(0.0, 0.78, 0.0), vec3(0.56, 0.14, 0.56), cap);
    // Cara: monocelha em V, olhos com pupila pro meio, boca reta e um dentão
    for side in [1.0f32, -1.0] {
        let brow = m * Mat4::from_translation(vec3(side * 0.09, 0.42, 0.236)) * Mat4::from_rotation_z(side * 0.4);
        b.cube(&brow, Vec3::ZERO, vec3(0.19, 0.05, 0.02), ink);
        b.glow(&m, vec3(side * 0.1, 0.33, 0.235), vec3(0.1, 0.11, 0.02), WHITE);
        b.cube(&m, vec3(side * 0.075, 0.32, 0.246), vec3(0.045, 0.07, 0.02), ink);
    }
    b.cube(&m, vec3(0.0, 0.235, 0.236), vec3(0.18, 0.025, 0.02), ink);
    b.glow(&m, vec3(0.03, 0.2, 0.236), vec3(0.07, 0.06, 0.03), WHITE);
    if low {
        return;
    }
    let spot = Color::new(1.0, 0.58, 0.12, 1.0);
    for (c, sz) in [
        (vec3(0.12, 0.855, 0.1), vec3(0.13, 0.02, 0.13)),
        (vec3(-0.14, 0.855, -0.1), vec3(0.11, 0.02, 0.11)),
        (vec3(0.2, 0.62, 0.435), vec3(0.13, 0.1, 0.02)),
        (vec3(-0.22, 0.6, -0.435), vec3(0.13, 0.1, 0.02)),
        (vec3(0.435, 0.62, -0.12), vec3(0.02, 0.1, 0.13)),
        (vec3(-0.435, 0.6, 0.15), vec3(0.02, 0.1, 0.13)),
    ] {
        b.glow(&m, c, sz, spot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::G;

    const TOP: Vec3 = vec3(10.0, 20.85, 10.0);

    #[test]
    fn pisao_de_cima_amassa() {
        assert!(stomps(vec3(10.1, 21.0, 10.0), -6.0, TOP));
        assert!(stomps(vec3(10.4, 20.6, 9.8), -12.0, TOP), "pés um pouco abaixo do topo, na borda, ainda vale");
    }

    #[test]
    fn subindo_parado_ou_de_lado_nao_amassa() {
        assert!(!stomps(vec3(10.0, 21.0, 10.0), 4.0, TOP), "subindo");
        assert!(!stomps(vec3(10.0, 21.0, 10.0), -0.5, TOP), "parado em cima");
        assert!(!stomps(vec3(10.0, 20.0, 10.0), -6.0, TOP), "pés abaixo da cabeça = trombada de lado");
        assert!(!stomps(vec3(11.0, 21.0, 10.0), -6.0, TOP), "longe pro lado");
        assert!(!stomps(vec3(10.0, 22.0, 10.0), -6.0, TOP), "ainda alto demais");
    }

    #[test]
    fn patrulha_fica_na_ronda_e_no_chao() {
        let world = World::generate();
        let npcs = Npcs::new(0, 0, 0);
        let mut g = Gumbas::new();
        let start: Vec<Vec3> = g.list.iter().map(|x| x.pos).collect();
        let mut turned = 0;
        for _ in 0..1200 {
            let before: Vec<f32> = g.list.iter().map(|x| x.yaw).collect();
            g.think(&world, 1.0 / 60.0, &npcs);
            turned += g.list.iter().zip(&before).filter(|(x, y)| x.yaw != **y).count();
        }
        assert!(turned >= N, "todo mundo deu meia-volta pelo menos uma vez");
        for (k, x) in g.list.iter().enumerate() {
            assert!((x.pos.y - G as f32).abs() < 0.01, "no chão da arena");
            assert!(vec2(x.pos.x - start[k].x, x.pos.z - start[k].z).length() <= SPAN + 0.1, "não fugiu da ronda");
        }
    }

    #[test]
    fn trombada_de_lado_morde() {
        let base = TOP - Vec3::Y * H;
        assert!(bumps(vec3(10.5, 20.0, 10.0), base));
        assert!(!bumps(vec3(10.0, 20.9, 10.0), base), "em cima não é de lado");
        assert!(!bumps(vec3(11.5, 20.0, 10.0), base), "longe");
    }
}
