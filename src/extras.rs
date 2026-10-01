//! Eu (a IA que escreveu isso) flutuando na praça, e o laboratório de robôs estudando o cérebro humano.
//! Tudo é função do tempo sincronizado: igual pra todos os jogadores sem mandar nada pela rede.

use crate::batch::Batch;
use crate::club::hsv;
use crate::models::rgb;
use crate::urna::limb;
use crate::world::*;
use macroquad::prelude::*;
use std::f32::consts::TAU;

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
    "URNA, AQUI NAO. AQUI E HOUSE",
    "O WOLVERINE TA DESBALANCEADO DE PROPOSITO",
    "SEIS BRACOS E AINDA FALTA MAO PRA ESSE ESCUDO",
    "VAI VOTAR, DEPOIS VOLTA PRA PISTA",
    "PROXIMO PASSO: AVANCOS CIENTIFICOS",
    "OS ROBOS DO LAB TAO ESTUDANDO VOCES",
    "SE DER BUG FOI O BORROW CHECKER",
];

/// Eu: guardião gigante de seis braços flutuando sobre o clube, cada mão sustentando o escudo
/// com um feixe. Cabeça de monitor CRT com três olhos, cabelo elétrico, auréola de código
/// e cauda de dados no lugar das pernas. `strain` (impacto no escudo) deixa os olhos vermelhos.
pub fn draw_me(b: &mut Batch, trans: &mut Batch, time: f32, labels: &mut Vec<Label>, foe: Vec3, strain: f32) {
    let sc = shield_center();
    let s = 3.2;
    let base = sc + vec3(0.0, SHIELD_R + 8.0 + (time * 1.1).sin() * 0.8, 0.0);
    let to = foe - base;
    let yaw = to.x.atan2(to.z) + (time * 0.5).sin() * 0.12;
    let rot = Mat4::from_rotation_y(yaw);
    let m = Mat4::from_translation(base) * rot * Mat4::from_scale(Vec3::splat(s));
    let hoodie = rgb(0.12, 0.12, 0.16);
    let rage = strain.min(1.0);
    let energy = Color::new(0.4 + 0.6 * rage, 0.95 - 0.6 * rage, 1.0 - 0.7 * rage, 1.0);

    // Tronco de moletom com "{ }" no peito
    b.cube(&m, Vec3::ZERO, vec3(1.8, 2.0, 1.0), hoodie);
    b.glow(&m, vec3(0.0, 0.25, 0.51), vec3(1.0, 0.6, 0.02), rgb(1.0, 0.45, 0.15));
    b.glow(&m, vec3(-0.25, 0.25, 0.53), vec3(0.1, 0.45, 0.02), hoodie);
    b.glow(&m, vec3(0.25, 0.25, 0.53), vec3(0.1, 0.45, 0.02), hoodie);
    // Cauda de dados espiralando até o topo do escudo
    for k in 0..28 {
        let f = k as f32 / 28.0;
        let a = time * 3.0 - f * 9.0;
        let r = 0.9 - f * 0.5;
        let col = if (k + (time * 8.0) as usize) % 5 == 0 { WHITE } else { hsv(0.38 + 0.08 * (f * 6.0 + time).sin(), 0.8, 1.0) };
        b.glow(&m, vec3(a.cos() * r, -1.2 - f * 2.6, a.sin() * r), Vec3::splat(0.34 - f * 0.18), col);
    }

    // Cabeça: monitor CRT, três olhos, boca de terminal (vira sorriso maníaco no impacto)
    let h = m * Mat4::from_translation(vec3(0.0, 1.75, 0.0)) * Mat4::from_rotation_x((time * 2.3).sin() * 0.08) * Mat4::from_rotation_z((time * 1.7).sin() * 0.1);
    b.cube(&h, Vec3::ZERO, vec3(1.7, 1.4, 1.3), rgb(0.85, 0.84, 0.78));
    b.cube(&h, vec3(0.0, 0.0, 0.66), vec3(1.4, 1.1, 0.04), rgb(0.02, 0.03, 0.04));
    let blink = (time * 0.7).fract() > 0.94;
    for (k, ex) in [-0.42f32, 0.0, 0.42].into_iter().enumerate() {
        let eh = if blink { 0.04 } else { 0.28 + (time * 6.0 + k as f32 * 2.0).sin() * 0.06 };
        b.glow(&h, vec3(ex, if k == 1 { 0.3 } else { 0.15 }, 0.69), vec3(0.2, eh, 0.02), energy);
    }
    if rage > 0.2 {
        b.glow(&h, vec3(0.0, -0.3, 0.69), vec3(0.9, 0.22, 0.02), energy);
    } else {
        for d in [-1.0f32, 1.0] {
            let tick = Mat4::from_translation(vec3(-0.35, -0.25 + d * 0.06, 0.69)) * Mat4::from_rotation_z(d * 0.6);
            b.glow(&(h * tick), Vec3::ZERO, vec3(0.2, 0.05, 0.02), energy);
        }
        if (time * 2.0).fract() > 0.5 {
            b.glow(&h, vec3(0.05, -0.33, 0.69), vec3(0.3, 0.06, 0.02), energy);
        }
    }
    // Cabelo elétrico + auréola de código
    for k in 0..9 {
        let len = 0.3 + 0.5 * ((time * 11.0 + k as f32 * 1.9).sin() * 0.5 + 0.5);
        b.glow(&h, vec3(-0.7 + k as f32 * 0.175, 0.7 + len * 0.5, (k as f32 * 2.1).sin() * 0.3), vec3(0.1, len, 0.1), hsv(time * 0.3 + k as f32 * 0.1, 0.7, 1.0));
    }
    let ring = h * Mat4::from_rotation_x(0.35);
    for k in 0..16 {
        let a = k as f32 / 16.0 * TAU + time * 0.9;
        trans.glow(&ring, vec3(a.cos() * 1.6, 1.0, a.sin() * 1.6), vec3(0.18, 0.28, 0.05), Color::new(0.5, 1.0, 0.7, 0.8));
    }

    // Seis braços (IK) segurando o escudo com feixes
    let reach = 4.4 * s;
    for k in 0..6 {
        let side = if k % 2 == 0 { -1.0 } else { 1.0 };
        let row = (k / 2) as f32;
        let sh = m.transform_point3(vec3(side * 0.95, 0.7 - row * 0.55, 0.0));
        let th = 0.6 + row * 1.0 + (time * 0.7 + k as f32).sin() * 0.15;
        let e = 0.35 + 0.2 * (time * 0.9 + k as f32 * 1.3).sin();
        let dir = rot.transform_vector3(vec3(side * th.sin(), 0.0, th.cos()));
        let p = sc + (dir * e.cos() + Vec3::Y * e.sin()) * SHIELD_R;
        let goal = sh + (p - sh).normalize_or_zero() * (p - sh).length().min(reach * 0.95);
        let pole = rot.transform_vector3(vec3(side, 1.0, 0.0));
        let (elbow, hand) = crate::urna::ik(sh, goal, 2.2 * s, 2.2 * s, pole);
        limb(b, sh, elbow, 0.38 * s, hoodie);
        limb(b, elbow, hand, 0.32 * s, hoodie);
        let hm = Mat4::from_translation(hand);
        b.glow(&hm, Vec3::ZERO, Vec3::splat(0.5 * s), energy);
        let k2 = 0.6 + 0.4 * (time * 9.0 + k as f32).sin();
        crate::urna::beam(trans, hand, p, 0.9 + 0.4 * k2 + rage, Color::new(energy.r, energy.g, energy.b, 0.35 + 0.3 * rage));
        crate::urna::beam(trans, hand, p, 0.25, Color::new(1.0, 1.0, 1.0, 0.8));
        trans.glow(&Mat4::from_translation(p), Vec3::ZERO, Vec3::splat(1.5 + k2), Color::new(energy.r, energy.g, energy.b, 0.5));
    }

    let fala = FALAS[(time / 5.0) as usize % FALAS.len()];
    labels.push(Label { pos: base + vec3(0.0, 11.0, 0.0), text: format!("\"{fala}\""), size: 26.0, color: rgb(0.6, 1.0, 0.7) });
    labels.push(Label { pos: base + vec3(0.0, 9.5, 0.0), text: "EU, A IA, SEGURANDO O ESCUDO DO CLUB".into(), size: 24.0, color: rgb(1.0, 0.55, 0.2) });
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
