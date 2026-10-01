//! Bolsonaro Voador (homenagem ao jogo mobile de 2016): o Jair de um universo paralelo que voa
//! e atira laser pelos olhos. A rota é função do tempo sincronizado (igual pra todos sem rede);
//! o host escolhe os alvos e manda os tiros como os da urna (`"w"`/`shot` com `by: 3`).

use crate::actors::{Ev, Fighter, Villager};
use crate::batch::Batch;
use crate::extras::Label;
use crate::models::{Look, Pose, U, draw_humanoid, rgb};
use crate::npc::{self, Npcs, Vida};
use crate::urna::{self, Beam, Fx, Plan};
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

pub const BY: u64 = 3;
const S: f32 = 3.0;
const NOD: f32 = -1.1;

const FALAS: [&str; 12] = [
    "TALKEY?",
    "BRASIL ACIMA DE TUDO. EU ACIMA DO BRASIL",
    "E DAI? EU VOO",
    "ESSA URNA AI NAO E AUDITAVEL!",
    "QUERO VOTO IMPRESSO... A LASER",
    "IMBROCHAVEL E IMPOUSAVEL",
    "ESSE CLUB DO HOUSE E COMUNISTA",
    "MOTOCIATA? EU FACO AVIOCIATA",
    "BOLSONARO VOADOR, DESDE 2016",
    "LASER NOS OLHOS, TALKEY?",
    "NAO SOU PILOTO, SOU MITO",
    "UNIVERSO PARALELO, PO! AQUI EU VOO",
];
const GRITOS: [&str; 5] = ["PEW PEW, TALKEY?", "TOMA LASER!", "E DAI?", "AVIOCIATA!", "CONFIRMA ISSO AI!"];

/// Rota no céu: dois senos por eixo (vagueia pela vila toda) e um mergulho a cada 26s.
pub fn pos(t: f32) -> Vec3 {
    let x = 64.0 + 40.0 * (t * 0.11).sin() + 12.0 * (t * 0.27 + 1.0).sin();
    let z = 64.0 + 38.0 * (t * 0.083 + 2.0).cos() + 12.0 * (t * 0.23).sin();
    let ph = t.rem_euclid(26.0);
    let dive = if ph < 5.0 { (ph / 5.0 * PI).sin().powi(2) } else { 0.0 };
    vec3(x, G as f32 + 20.0 + 2.5 * (t * 0.6).sin() - 13.0 * dive, z)
}

fn yaw_at(t: f32) -> f32 {
    let d = pos(t + 0.05) - pos(t);
    d.x.atan2(d.z)
}

/// Matriz do corpo (deitado no ar, cabeça pra frente, inclinando na curva) e da cabeça.
fn frame(t: f32, at: Vec3) -> (Mat4, Mat4) {
    let v = (pos(t + 0.05) - pos(t)) / 0.05;
    let yaw = v.x.atan2(v.z);
    let turn = (yaw_at(t + 0.2) - yaw + PI).rem_euclid(TAU) - PI;
    let bank = (-turn * 3.0).clamp(-0.9, 0.9);
    let dive = (-v.y).atan2(vec2(v.x, v.z).length()).clamp(-0.6, 1.0);
    let body = Mat4::from_translation(at)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_rotation_z(bank)
        * Mat4::from_rotation_x(FRAC_PI_2 + dive)
        * Mat4::from_scale(Vec3::splat(S))
        * Mat4::from_translation(vec3(0.0, -16.0 * U, 0.0));
    let head = body * Mat4::from_translation(vec3(0.0, 24.0 * U, 0.0)) * Mat4::from_rotation_x(NOD);
    (body, head)
}

pub fn eyes(t: f32) -> [Vec3; 2] {
    let (_, h) = frame(t, pos(t));
    [1.0f32, -1.0].map(|s| h.transform_point3(vec3(s * 2.0 * U, 4.0 * U, 4.4 * U)))
}

fn ground(world: &World, p: Vec3) -> f32 {
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    (1..WY).rev().find(|&y| world.get(x, y, z) != AIR).map(|y| y as f32 + 1.0).unwrap_or(G as f32)
}

pub struct Voador {
    cd: f32,
    glow: f32,
}

impl Voador {
    pub fn new() -> Self {
        Voador { cd: 8.0, glow: 0.0 }
    }

    /// Host: escolhe alvo (jogador, NPC, urna ou chão) e devolve o tiro pra mandar.
    #[allow(clippy::too_many_arguments)]
    pub fn think(&mut self, world: &World, dt: f32, time: f32, players: &[Vec3], fighters: &[Fighter], villagers: &[Villager], npcs: &Npcs, urna_pos: Vec3, events: &mut Vec<Ev>) -> Option<Plan> {
        self.cd -= dt;
        if self.cd > 0.0 || !npcs.voador().alive() {
            return None;
        }
        self.cd = gen_range(2.5, 4.5);
        let me = pos(time);
        let near = |p: &Vec3| (8.0..60.0).contains(&p.distance(me));
        let ps: Vec<Vec3> = players.iter().map(|p| *p + vec3(0.0, 0.9, 0.0)).filter(near).collect();
        let mut ns: Vec<Vec3> = fighters.iter().filter(|f| f.active()).map(|f| f.pos + vec3(0.0, 0.9, 0.0)).filter(near).collect();
        ns.extend(villagers.iter().enumerate().filter(|(i, _)| npcs.alive(npc::VILLAGER, *i)).map(|(_, v)| v.pos + vec3(0.0, 0.9, 0.0)).filter(near));
        let roll = gen_range(0.0, 1.0);
        let (target, grito) = if roll < 0.2 && npcs.urna().alive() && near(&urna_pos) {
            (urna_pos, "ESSA URNA NAO E AUDITAVEL!")
        } else if roll < 0.5 && time > 20.0 && !ps.is_empty() {
            (ps[gen_range(0, ps.len())], GRITOS[gen_range(0, GRITOS.len())])
        } else if !ns.is_empty() {
            (ns[gen_range(0, ns.len())], GRITOS[gen_range(0, GRITOS.len())])
        } else {
            let p = me + vec3(gen_range(-15.0, 15.0), 0.0, gen_range(-15.0, 15.0));
            (vec3(p.x, ground(world, p), p.z), "TALKEY?")
        };
        if gen_range(0.0, 1.0) < 0.35 {
            events.push(Ev::Text { pos: me + vec3(0.0, 4.0, 0.0), text: grito.into(), color: rgb(1.0, 0.9, 0.2), big: false });
        }
        let mut plan = urna::plan(world, eyes(time)[0], target, 0.0);
        plan.r = gen_range(1.6, 2.4);
        Some(plan)
    }

    /// Todos os clientes, ao receber o tiro: segundo feixe (olho direito) e olhos acesos.
    pub fn on_shot(&mut self, plan: &Plan, time: f32, fx: &mut Fx) {
        self.glow = 0.5;
        fx.beams.push(Beam { from: eyes(time)[1], to: plan.hit, t: 0.3, reflect: None });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, b: &mut Batch, trans: &mut Batch, world: &World, time: f32, dt: f32, labels: &mut Vec<Label>, life: &Vida) {
        self.glow = (self.glow - dt).max(0.0);
        let mut at = pos(time);
        let mut tumble = Mat4::IDENTITY;
        let dead = (!life.alive()).then_some(life.t);
        if let Some(t) = dead {
            if t > 12.0 {
                return;
            }
            let p0 = pos(time - t);
            let floor = ground(world, p0) + 0.4;
            let fall = (p0.y - 9.0 * t * t).max(floor);
            at = vec3(p0.x, fall, p0.z);
            let spin = if fall > floor { t * 7.0 } else { 0.0 };
            tumble = Mat4::from_rotation_y(spin);
            if fall > floor {
                for k in 0..3 {
                    let p = at + vec3((t * 13.0 + k as f32).sin(), 1.5 + k as f32, (t * 11.0 + k as f32).cos());
                    trans.glow(&Mat4::from_translation(p), Vec3::ZERO, Vec3::splat(1.2 + k as f32 * 0.6), Color::new(1.0, 0.4, 0.1, 0.5));
                }
            }
        }
        let (body, head) = if dead.is_some() {
            let m = Mat4::from_translation(at) * tumble * Mat4::from_rotation_x(-FRAC_PI_2) * Mat4::from_scale(Vec3::splat(S)) * Mat4::from_translation(vec3(0.0, -16.0 * U, 0.0));
            (m, m * Mat4::from_translation(vec3(0.0, 24.0 * U, 0.0)))
        } else {
            frame(time, at)
        };

        let suit = rgb(0.1, 0.12, 0.2);
        let skin = rgb(0.93, 0.76, 0.66);
        let hair = rgb(0.68, 0.68, 0.66);
        let green = rgb(0.0, 0.6, 0.25);
        let yellow = rgb(1.0, 0.85, 0.1);
        let look = Look { skin, hair, shirt: suit, pants: suit, shoes: rgb(0.05, 0.05, 0.05), beard: None, glasses: false, wolverine: false, toga: false };
        let (arm_r, arm_l) = if dead.is_some() { (-2.6, -2.6) } else { (-PI, -0.15) };
        let pose = Pose { walk: time * 6.0, walk_amt: if dead.is_some() { 0.0 } else { 0.12 }, arm_l, arm_r, nod: if dead.is_some() { 0.0 } else { NOD }, flash: life.flash, ..Default::default() };
        draw_humanoid(b, &look, &pose, &body);

        // Camisa, gravata e faixa presidencial verde-amarela
        b.cube(&body, vec3(0.0, 22.6 * U, 2.05 * U), vec3(3.0 * U, 2.6 * U, 0.2 * U), WHITE);
        b.cube(&body, vec3(0.0, 20.0 * U, 2.15 * U), vec3(1.0 * U, 5.0 * U, 0.2 * U), rgb(0.15, 0.25, 0.6));
        let sash = body * Mat4::from_translation(vec3(0.0, 18.0 * U, 2.3 * U)) * Mat4::from_rotation_z(0.6);
        b.cube(&sash, Vec3::ZERO, vec3(2.2 * U, 13.0 * U, 0.3 * U), green);
        for s in [-1.0f32, 1.0] {
            b.cube(&sash, vec3(s * 1.2 * U, 0.0, 0.0), vec3(0.5 * U, 13.0 * U, 0.35 * U), yellow);
        }
        b.glow(&sash, vec3(0.0, -4.0 * U, 0.2 * U), vec3(1.6 * U, 1.6 * U, 0.2 * U), yellow);

        // Cabelo grisalho repartido de lado + topete
        b.cube(&head, vec3(1.6 * U, 8.32 * U, 0.5 * U), vec3(0.45 * U, 0.1 * U, 7.0 * U), skin);
        b.cube(&head, vec3(-0.9 * U, 7.2 * U, 4.05 * U), vec3(6.4 * U, 1.4 * U, 0.6 * U), hair);
        b.cube(&head, vec3(-1.5 * U, 8.5 * U, 1.0 * U), vec3(4.6 * U, 0.6 * U, 6.0 * U), hair);

        // Capa verde com barra amarela, tremulando
        for k in 0..6 {
            let f = k as f32;
            let wave = if dead.is_some() { 0.0 } else { (time * 9.0 - f * 0.9).sin() * (0.3 + f * 0.25) };
            let col = if k == 5 { yellow } else { green };
            b.cube(&body, vec3(0.0, 22.5 * U - f * 4.0 * U, -2.4 * U - wave * U - f * 0.4 * U), vec3(9.0 * U, 4.2 * U, 0.4 * U), col);
        }
        b.glow(&body, vec3(0.0, 14.0 * U, -3.5 * U - (time * 9.0 - 2.0).sin() * U), vec3(3.0 * U, 3.0 * U, 0.2 * U), rgb(0.1, 0.25, 0.8));

        // Olhos de laser
        let g = self.glow * 2.0;
        if dead.is_none() {
            for s in [1.0f32, -1.0] {
                let e = vec3(s * 2.0 * U, 4.0 * U, 4.4 * U);
                b.glow(&head, e, vec3(2.2 * U, 1.2 * U, 0.3 * U), Color::new(1.0, 0.15 + 0.3 * (1.0 - g.min(1.0)), 0.1, 1.0));
                if g > 0.0 {
                    trans.glow(&head, e + vec3(0.0, 0.0, 0.6 * U), Vec3::splat(3.5 * U * g.min(1.0)), Color::new(1.0, 0.1, 0.1, 0.6));
                }
            }
        }

        let tag = at + vec3(0.0, 4.5, 0.0);
        if let Some(t) = dead {
            if t < 8.0 {
                labels.push(Label { pos: tag, text: "\"CAI... MAS VOU RECORRER!\"".into(), size: 24.0, color: rgb(1.0, 0.4, 0.3) });
            }
            return;
        }
        labels.push(Label { pos: tag, text: "BOLSONARO VOADOR".into(), size: 26.0, color: yellow });
        if time.rem_euclid(9.0) < 5.0 {
            let fala = FALAS[(time / 9.0) as usize % FALAS.len()];
            labels.push(Label { pos: tag + vec3(0.0, 2.0, 0.0), text: format!("\"{fala}\""), size: 24.0, color: rgb(0.5, 1.0, 0.5) });
        }
    }
}
