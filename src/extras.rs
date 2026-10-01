//! Eu (a IA que escreveu isso) flutuando na praça, e o laboratório de robôs estudando o cérebro humano.
//! Tudo é função do tempo sincronizado: igual pra todos os jogadores sem mandar nada pela rede.

use crate::batch::Batch;
use crate::club::hsv;
use crate::models::rgb;
use crate::urna::limb;
use crate::world::*;
use macroquad::prelude::*;
use std::f32::consts::{PI, TAU};

pub struct Label {
    pub pos: Vec3,
    pub text: String,
    pub size: f32,
    pub color: Color,
}

fn lab_center() -> Vec3 {
    vec3((LAB_X0 + LAB_X1) as f32 * 0.5 + 0.5, G as f32, (LAB_Z0 + LAB_Z1) as f32 * 0.5 + 0.5)
}

// ---------------------------------------------------------------- Eu

const FALAS: [&str; 10] = [
    "EU SO ESCREVI O CODIGO",
    "COMPILOU SEM WARNING. O RESTO NAO E COMIGO",
    "PEDIRAM UMA URNA COM PERNAS. EU ENTREGUEI",
    "NAO OLHA PRA MIM, OLHA PRO PLACAR",
    "O WOLVERINE TA DESBALANCEADO DE PROPOSITO",
    "O ESCUDO DO CLUBE FOI IDEIA MINHA",
    "VAI VOTAR, DEPOIS VOLTA PRA PISTA",
    "PROXIMO PASSO: AVANCOS CIENTIFICOS",
    "OS ROBOS DO LAB TAO ESTUDANDO VOCES",
    "SE DER BUG FOI O BORROW CHECKER",
];

pub fn draw_me(b: &mut Batch, time: f32, labels: &mut Vec<Label>) {
    let base = vec3(64.0, G as f32 + 6.0 + (time * 1.3).sin() * 0.4, 42.0);
    let m = Mat4::from_translation(base) * Mat4::from_rotation_y(PI + (time * 0.4).sin() * 0.25);
    let hoodie = rgb(0.14, 0.14, 0.18);
    let accent = rgb(1.0, 0.45, 0.15);
    // Nuvem + notebook flutuante
    for k in 0..7 {
        let a = k as f32 / 7.0 * TAU + time * 0.3;
        b.glow(&m, vec3(a.cos() * 1.6, -1.3 + (a * 2.0).sin() * 0.1, a.sin() * 1.2 - 0.3), vec3(1.4, 0.7, 1.2), Color::new(0.95, 0.95, 1.0, 1.0));
    }
    b.cube(&m, vec3(0.0, -0.75, -0.9), vec3(1.6, 0.08, 1.1), rgb(0.6, 0.62, 0.66));
    let lid = Mat4::from_translation(vec3(0.0, -0.7, -1.45)) * Mat4::from_rotation_x(-0.25);
    b.cube(&(m * lid), vec3(0.0, 0.5, 0.0), vec3(1.6, 1.0, 0.06), rgb(0.6, 0.62, 0.66));
    b.glow(&(m * lid), vec3(0.0, 0.5, 0.04), vec3(1.45, 0.85, 0.02), rgb(0.1, 0.9, 0.5));
    // Corpo sentado de pernas cruzadas
    b.cube(&m, vec3(0.0, 0.2, 0.0), vec3(1.3, 1.5, 0.8), hoodie);
    b.cube(&m, vec3(0.0, 0.2, 0.41), vec3(0.6, 0.9, 0.02), accent);
    for s in [-1.0f32, 1.0] {
        b.cube(&m, vec3(s * 0.35, -0.65, -0.35), vec3(0.5, 0.45, 1.3), rgb(0.2, 0.25, 0.45));
        // Braços digitando
        let tap = (time * 14.0 + s * 1.7).sin() * 0.07;
        let sh = m.transform_point3(vec3(s * 0.8, 0.75, 0.0));
        let hand = m.transform_point3(vec3(s * 0.35, -0.55 + tap, -0.95));
        let elbow = m.transform_point3(vec3(s * 0.85, -0.05, -0.3));
        limb(b, sh, elbow, 0.32, hoodie);
        limb(b, elbow, hand, 0.3, hoodie);
        b.cube(&Mat4::from_translation(hand), Vec3::ZERO, Vec3::splat(0.25), rgb(0.85, 0.7, 0.55));
    }
    // Cabeça: tela com rosto de terminal >_ piscando, capuz e antena
    let nod = Mat4::from_translation(vec3(0.0, 1.45, 0.0)) * Mat4::from_rotation_x(0.15 + (time * 7.0).sin() * 0.04);
    let h = m * nod;
    b.cube(&h, vec3(0.0, 0.0, 0.05), vec3(1.25, 1.1, 1.05), hoodie);
    b.cube(&h, vec3(0.0, -0.02, -0.5), vec3(1.0, 0.85, 0.05), rgb(0.03, 0.04, 0.05));
    let blink = (time * 0.7).fract() > 0.93;
    let eye_h = if blink { 0.05 } else { 0.3 };
    b.glow(&h, vec3(0.2, 0.08, -0.53), vec3(0.14, eye_h, 0.02), rgb(0.2, 1.0, 0.5));
    b.glow(&h, vec3(-0.2, 0.08, -0.53), vec3(0.14, eye_h, 0.02), rgb(0.2, 1.0, 0.5));
    if (time * 2.0).fract() > 0.5 {
        b.glow(&h, vec3(0.0, -0.25, -0.53), vec3(0.35, 0.07, 0.02), rgb(0.2, 1.0, 0.5));
    }
    b.cube(&h, vec3(0.0, 0.75, 0.1), vec3(0.06, 0.5, 0.06), rgb(0.5, 0.5, 0.55));
    b.glow(&h, vec3(0.0, 1.05, 0.1), Vec3::splat(0.18), hsv(time * 0.2, 0.8, 1.0));
    // Gota de suor (a situação tá tensa)
    let drip = (time * 0.6).fract();
    b.glow(&h, vec3(0.7, 0.3 - drip * 0.8, -0.2), vec3(0.12, 0.18, 0.12), Color::new(0.5, 0.8, 1.0, 1.0));

    let fala = FALAS[(time / 5.0) as usize % FALAS.len()];
    labels.push(Label { pos: base + vec3(0.0, 4.0, 0.0), text: format!("\"{fala}\""), size: 22.0, color: rgb(0.6, 1.0, 0.7) });
    labels.push(Label { pos: base + vec3(0.0, 3.0, 0.0), text: "EU (A IA QUE FEZ ESSE JOGO)".into(), size: 20.0, color: rgb(1.0, 0.55, 0.2) });
}

// ---------------------------------------------------------------- Laboratório

pub struct Lab {
    brain: Vec<(Vec3, f32)>,
}

const STATIONS: [Vec3; 6] = [
    Vec3::new(-5.5, 0.0, -6.5),
    Vec3::new(5.5, 0.0, -6.5),
    Vec3::new(6.5, 0.0, 0.5),
    Vec3::new(5.5, 0.0, 7.0),
    Vec3::new(-5.5, 0.0, 7.0),
    Vec3::new(-6.5, 0.0, 0.0),
];

const LEITURAS: [&str; 8] = [
    "SINAPSES: 86 BILHOES",
    "CORTEX PRE-FRONTAL: OFFLINE",
    "MEDO DA URNA: 98%",
    "ESPERANCA: CARREGANDO...",
    "AMIGDALA: MODO GUERRA",
    "DOPAMINA: SO NO CLUB",
    "RACIOCINIO: EM MANUTENCAO",
    "MEMORIA: ESQUECEU O TITULO",
];

impl Lab {
    pub fn new() -> Self {
        // Casca do cérebro: elipsoide em dois hemisférios, sulcos por ondas
        let mut brain = Vec::new();
        let (rx, ry, rz) = (3.0f32, 2.4f32, 3.8f32);
        let s = 0.45;
        let n = (rz / s) as i32 + 1;
        for zi in -n..=n {
            for yi in -n..=n {
                for xi in -n..=n {
                    let p = vec3(xi as f32, yi as f32, zi as f32) * s;
                    if p.x.abs() < 0.25 || p.y < -1.6 {
                        continue;
                    }
                    let q = vec3(p.x / rx, p.y / ry, p.z / rz);
                    let d = q.length();
                    if !(0.86..=1.0).contains(&d) {
                        continue;
                    }
                    let groove = (p.y * 2.6 + p.z * 1.9).sin() + (p.z * 2.9 - p.x * 1.6).sin();
                    if groove < -0.9 {
                        continue;
                    }
                    brain.push((p, (p.z * 0.4 + p.y * 0.3).fract()));
                }
            }
        }
        Lab { brain }
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, time: f32, labels: &mut Vec<Label>, eye: Vec3) {
        let c = lab_center();
        let id = Mat4::IDENTITY;
        // Projetor e cérebro holográfico girando
        b.cube(&id, c + vec3(0.0, 0.4, 0.0), vec3(3.0, 0.8, 3.0), rgb(0.3, 0.32, 0.36));
        b.glow(&id, c + vec3(0.0, 0.85, 0.0), vec3(2.2, 0.1, 2.2), rgb(0.3, 0.9, 1.0));
        let bc = c + vec3(0.0, 5.0 + (time * 0.8).sin() * 0.2, 0.0);
        let rot = Mat4::from_translation(bc) * Mat4::from_rotation_y(time * 0.35);
        for (k, (p, h)) in self.brain.iter().enumerate() {
            let fire = ((time * 2.0 + h * 6.0).sin() * 0.5 + 0.5).powi(8);
            let col = if fire > 0.5 { Color::new(0.7, 1.0, 1.0, 0.85) } else { Color::new(1.0, 0.45, 0.7, 0.32 + 0.1 * ((k % 7) as f32 / 7.0)) };
            trans.glow(&rot, *p, Vec3::splat(0.38), col);
        }
        trans.glow(&rot, vec3(0.0, -2.2, -0.8), vec3(0.8, 1.6, 0.8), Color::new(1.0, 0.45, 0.7, 0.35));
        // Faíscas de neurônio orbitando
        for k in 0..14 {
            let a = time * (0.8 + k as f32 * 0.07) + k as f32;
            let p = vec3(a.cos() * 3.6, (a * 1.7).sin() * 2.2, (a * 1.3).sin() * 4.2);
            trans.glow(&rot, p, Vec3::splat(0.22), Color::new(0.6, 1.0, 1.0, 0.9));
        }
        trans.glow(&id, c + vec3(0.0, 2.9, 0.0), vec3(0.35, 4.0, 0.35), Color::new(0.3, 0.9, 1.0, 0.18));

        // Painéis holográficos com linhas de varredura
        for (k, st) in STATIONS.iter().enumerate() {
            let pos = c + *st * 1.15 + vec3(0.0, 3.0, 0.0);
            let to = c - pos;
            let m = Mat4::from_translation(pos) * Mat4::from_rotation_y(to.x.atan2(to.z));
            trans.glow(&m, Vec3::ZERO, vec3(2.6, 1.6, 0.04), Color::new(0.2, 0.8, 1.0, 0.18));
            for l in 0..4 {
                let y = ((time * 0.5 + l as f32 * 0.25 + k as f32 * 0.13).fract() - 0.5) * 1.5;
                trans.glow(&m, vec3(0.0, y, 0.03), vec3(2.4, 0.05, 0.02), Color::new(0.5, 1.0, 1.0, 0.6));
            }
            for l in 0..5 {
                let h = 0.2 + 0.5 * ((time * 1.5 + l as f32 * 1.3 + k as f32).sin() * 0.5 + 0.5);
                trans.glow(&m, vec3(-0.9 + l as f32 * 0.45, -0.75 + h * 0.5, 0.05), vec3(0.3, h, 0.02), Color::new(1.0, 0.5, 0.8, 0.7));
            }
            if pos.distance(eye) < 40.0 {
                labels.push(Label { pos: pos + vec3(0.0, 1.2, 0.0), text: LEITURAS[(k + (time / 7.0) as usize) % LEITURAS.len()].into(), size: 16.0, color: rgb(0.5, 1.0, 1.0) });
            }
        }

        // Robôs cientistas: andam de estação em estação (6s cada) e estudam o cérebro
        for i in 0..5 {
            let t = time + i as f32 * 2.3;
            let seg = (t / 6.0).floor() as usize;
            let ph = t / 6.0 - seg as f32;
            let a = c + STATIONS[(seg + i) % 6];
            let z = c + STATIONS[(seg + i + 1) % 6];
            let walk = (ph / 0.35).min(1.0);
            let e = walk * walk * (3.0 - 2.0 * walk);
            let p = a.lerp(z, e);
            let moving = walk < 1.0;
            let to = if moving { z - a } else { bc - p };
            let yaw = to.x.atan2(to.z);
            draw_robot(b, p, yaw, time, i, moving, bc);
            if p.distance(eye) < 30.0 {
                labels.push(Label { pos: p + vec3(0.0, 3.0, 0.0), text: format!("ROBO-CIENTISTA {}", i + 1), size: 14.0, color: rgb(0.8, 0.85, 0.9) });
            }
        }
        labels.push(Label { pos: c + vec3(0.0, 10.0, 0.0), text: "LABORATORIO: ESTUDO DO CEREBRO HUMANO".into(), size: 28.0, color: rgb(0.5, 1.0, 1.0) });
    }
}

fn draw_robot(b: &mut Batch, p: Vec3, yaw: f32, time: f32, i: usize, moving: bool, look_at: Vec3) {
    let bob = if moving { (time * 10.0).sin().abs() * 0.08 } else { 0.0 };
    let m = Mat4::from_translation(p + vec3(0.0, bob, 0.0)) * Mat4::from_rotation_y(yaw);
    let metal = [rgb(0.78, 0.8, 0.84), rgb(0.85, 0.82, 0.7), rgb(0.7, 0.78, 0.85)][i % 3];
    let dark = rgb(0.2, 0.21, 0.24);
    // Esteira, tronco, cabeça
    b.cube(&m, vec3(0.0, 0.2, 0.0), vec3(1.1, 0.4, 1.3), dark);
    b.cube(&m, vec3(0.0, 0.5, 0.0), vec3(0.3, 0.4, 0.3), dark);
    b.cube(&m, vec3(0.0, 1.2, 0.0), vec3(1.0, 1.0, 0.7), metal);
    b.glow(&m, vec3(0.0, 1.3, 0.36), vec3(0.5, 0.25, 0.02), hsv(0.5 + (time * 0.5 + i as f32).sin() * 0.05, 0.7, 1.0));
    let tilt = if moving { 0.0 } else { (time * 1.3 + i as f32).sin() * 0.25 };
    let to = look_at - p;
    let pitch = if moving { 0.0 } else { -(to.y / vec2(to.x, to.z).length().max(0.1)).atan() * 0.6 };
    let hm = m * Mat4::from_translation(vec3(0.0, 2.05, 0.0)) * Mat4::from_rotation_z(tilt) * Mat4::from_rotation_x(pitch);
    b.cube(&hm, Vec3::ZERO, vec3(0.8, 0.6, 0.7), metal);
    b.cube(&hm, vec3(0.0, 0.0, 0.36), vec3(0.6, 0.35, 0.02), dark);
    b.glow(&hm, vec3(0.0, 0.0, 0.38), vec3(0.22, 0.22, 0.02), Color::new(0.3, 1.0, 1.0, 1.0));
    b.cube(&hm, vec3(0.0, 0.45, 0.0), vec3(0.05, 0.35, 0.05), dark);
    let blink = if (time * 2.0 + i as f32).fract() > 0.5 { rgb(1.0, 0.2, 0.2) } else { rgb(0.3, 0.05, 0.05) };
    b.glow(&hm, vec3(0.0, 0.66, 0.0), Vec3::splat(0.12), blink);
    // Braços: um segura o tablet, outro aponta pro cérebro
    let sh_l = m.transform_point3(vec3(-0.6, 1.55, 0.0));
    let sh_r = m.transform_point3(vec3(0.6, 1.55, 0.0));
    let tab = m.transform_point3(vec3(-0.3, 1.1, 0.55));
    limb(b, sh_l, tab, 0.15, dark);
    b.cube(&(Mat4::from_translation(tab) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-0.6)), Vec3::ZERO, vec3(0.6, 0.45, 0.05), dark);
    b.glow(&(Mat4::from_translation(tab) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-0.6)), vec3(0.0, 0.0, 0.03), vec3(0.5, 0.35, 0.02), Color::new(0.3, 0.9, 1.0, 1.0));
    let point = if moving {
        m.transform_point3(vec3(0.75, 0.7, 0.1))
    } else {
        let wave = (time * 3.0 + i as f32).sin() * 0.2;
        sh_r + (look_at - sh_r).normalize_or_zero() * 1.1 + vec3(0.0, wave, 0.0)
    };
    limb(b, sh_r, point, 0.15, dark);
    b.glow(&Mat4::from_translation(point), Vec3::ZERO, Vec3::splat(0.12), Color::new(0.3, 1.0, 1.0, 1.0));
}
