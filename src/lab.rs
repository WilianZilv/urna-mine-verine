//! Laboratório de pesquisa do cérebro: guardiã androide com cúpula de energia e painel holográfico
//! na entrada com os achados REAIS que o servidor lê (Europe PMC) e a IA resume. Fundo = moeda fictícia.

use crate::batch::Batch;
use crate::club::hsv;
use crate::extras::Label;
use crate::models::rgb;
use crate::npc::Vida;
use crate::urna::{beam, limb};
use crate::world::*;
use macroquad::prelude::*;
use serde_json::Value;
use std::f32::consts::FRAC_PI_2;

#[path = "lab_panel.rs"]
mod panel;

pub const DOME_R: f32 = 16.0;
pub const PANEL_X: f32 = 86.0;
const PANEL_Z: f32 = 64.5;

pub fn dome_center() -> Vec3 {
    vec3((LAB_X0 + LAB_X1) as f32 * 0.5 + 0.5, G as f32, (LAB_Z0 + LAB_Z1) as f32 * 0.5 + 0.5)
}

/// Guardiã: patrulha devagar a entrada (lado de dentro da cúpula). Função do tempo, igual em todos.
pub fn guard_pos(time: f32) -> Vec3 {
    vec3(LAB_X0 as f32 + 3.5, G as f32, PANEL_Z + (time * 0.25).sin() * 3.5)
}

/// Alvo de tiro/soco: (centro, raio).
pub fn guard_target(time: f32) -> (Vec3, f32) {
    (guard_pos(time) + vec3(0.0, 1.8, 0.0), 1.1)
}

struct Finding {
    title: String,
    text: String,
    src: String,
}

pub struct LabInfo {
    papers: i64,
    count: i64,
    topics: i64,
    fund: i64,
    cycles: i64,
    queue: i64,
    running: bool,
    ago: f64,
    next: f64,
    recv: f64,
    top: Vec<Finding>,
    got: bool,
    bars: Vec<(String, f32)>,
    per_cycle: Vec<f32>,
    fund_hist: Vec<f32>,
    day: f32,
    day_max: f32,
    // Animação: valores mostrados correm atrás dos reais; card novo desliza; borda pisca em dado novo.
    shown: [f32; 5],
    bars_shown: Vec<f32>,
    card_t: f32,
    flash: f32,
    rt: Option<RenderTarget>,
}

fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

fn dur(sec: f64) -> String {
    match sec as i64 {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}min", s / 60),
        s => format!("{}h{:02}", s / 3600, s % 3600 / 60),
    }
}

impl LabInfo {
    pub fn new() -> Self {
        LabInfo {
            papers: 0,
            count: 0,
            topics: 0,
            fund: 0,
            cycles: 0,
            queue: 0,
            running: false,
            ago: -1.0,
            next: 0.0,
            recv: 0.0,
            top: Vec::new(),
            got: false,
            bars: Vec::new(),
            per_cycle: Vec::new(),
            fund_hist: Vec::new(),
            day: 0.0,
            day_max: 40.0,
            shown: [0.0; 5],
            bars_shown: Vec::new(),
            card_t: 1.0,
            flash: 0.0,
            rt: None,
        }
    }

    /// Mensagem {t:"lab"} do servidor (snapshot completo, vem no join e a cada mudança).
    pub fn on_msg(&mut self, m: &Value) {
        let n = |k: &str| m[k].as_i64().unwrap_or(0);
        let old = (self.papers, self.fund, self.cycles, self.top.first().map(|f| f.title.clone()));
        let nums = |k: &str| m[k].as_array().into_iter().flatten().map(|v| v.as_f64().unwrap_or(0.0) as f32).collect::<Vec<_>>();
        self.bars = m["bars"].as_array().into_iter().flatten().map(|b| (s(&b[0]), b[1].as_f64().unwrap_or(0.0) as f32)).collect();
        self.per_cycle = nums("pc");
        self.fund_hist = nums("fh");
        self.day = m["day"].as_f64().unwrap_or(0.0) as f32;
        self.day_max = m["dmax"].as_f64().unwrap_or(40.0).max(1.0) as f32;
        (self.papers, self.count, self.topics, self.fund, self.cycles, self.queue) = (n("papers"), n("n"), n("topics"), n("fund"), n("cycles"), n("q"));
        self.running = m["run"].as_bool().unwrap_or(false);
        self.ago = m["ago"].as_f64().unwrap_or(-1.0);
        self.next = m["next"].as_f64().unwrap_or(0.0);
        self.recv = get_time();
        self.got = true;
        self.top = m["top"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|x| Finding { title: s(&x["pt"]), text: s(&x["f"]), src: format!("{} ({}) - tema: {} - {}", s(&x["j"]), s(&x["y"]), s(&x["topic"]), s(&x["u"]).replace("https://", "")) })
            .collect();
        let new_card = old.3.is_some() && old.3 != self.top.first().map(|f| f.title.clone());
        if new_card {
            self.card_t = 0.0;
        }
        if new_card || (self.got && (old.0, old.1, old.2) != (self.papers, self.fund, self.cycles)) {
            self.flash = 1.0;
        }
    }

    /// Cúpula de energia + tela do painel (modo imediato, depois do batch transparente, igual ao escudo do club).
    pub fn draw_dome(&self, time: f32, hit: f32, eye: Vec3) {
        let c = dome_center();
        let k = 0.03 * (time * 1.7).sin();
        draw_sphere(c, DOME_R, None, Color::new(0.3, 1.0, 0.7, 0.07 + k + 0.2 * hit));
        draw_sphere_wires(c, DOME_R + 0.05, None, Color::new(0.4, 1.0, 0.8, 0.1 + 0.3 * hit));
        self.draw_screen(time, eye);
    }
}

/// Guardiã (androide cientista de jaleco, cérebro num domo de vidro na cabeça) + painel da entrada.
pub fn draw(b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3, info: &LabInfo, life: Option<&Vida>) {
    draw_guard(b, trans, labels, time, eye, life);
    panel::draw_frame(b, trans, time, info.flash);
}

fn draw_guard(b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3, life: Option<&Vida>) {
    let p = guard_pos(time);
    let dead = life.filter(|v| !v.alive());
    let flash = life.map_or(0.0, |v| v.flash);
    let to = eye - p;
    let yaw = to.x.atan2(to.z);
    let hover = if dead.is_some() { 0.0 } else { 0.35 + (time * 2.0).sin() * 0.12 };
    let lean = dead.map_or(0.0, |v| (v.t * 2.5).min(1.0) * FRAC_PI_2);
    let m = Mat4::from_translation(p + vec3(0.0, hover, 0.0)) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-lean);
    let tint = |c: Color| Color::new(c.r + (1.0 - c.r) * flash, c.g + (1.0 - c.g) * flash, c.b + (1.0 - c.b) * flash, c.a);
    let coat = tint(rgb(0.93, 0.94, 0.96));
    let chrome = tint(rgb(0.62, 0.66, 0.72));
    let dark = rgb(0.12, 0.13, 0.16);
    let cyan = Color::new(0.3, 1.0, 0.85, 1.0);

    // Base antigravidade
    b.cube(&m, vec3(0.0, 0.25, 0.0), vec3(1.2, 0.3, 1.2), dark);
    if dead.is_none() {
        b.glow(&m, vec3(0.0, 0.08, 0.0), vec3(1.0, 0.06, 1.0), cyan);
        trans.glow(&m, vec3(0.0, -0.15, 0.0), vec3(1.3, 0.3, 1.3), Color::new(0.3, 1.0, 0.85, 0.25));
    }
    // Jaleco longo aberto na frente, camisa escura, bolso com canetas e crachá
    b.cube(&m, vec3(0.0, 1.55, 0.0), vec3(1.4, 2.1, 0.85), coat);
    b.cube(&m, vec3(0.0, 1.8, 0.43), vec3(0.4, 1.5, 0.02), dark);
    b.glow(&m, vec3(0.0, 1.9, 0.45), vec3(0.06, 1.2, 0.02), cyan);
    for (k, c) in [rgb(0.9, 0.2, 0.2), rgb(0.2, 0.4, 0.95), rgb(0.1, 0.1, 0.1)].into_iter().enumerate() {
        b.cube(&m, vec3(-0.45 + k as f32 * 0.09, 2.25, 0.45), vec3(0.05, 0.25, 0.04), c);
    }
    b.glow(&m, vec3(0.42, 2.15, 0.44), vec3(0.28, 0.2, 0.02), rgb(0.95, 0.85, 0.3));
    // Pescoço + cabeça cromada com visor e domo de vidro com cérebro rosa pulsando
    b.cube(&m, vec3(0.0, 2.75, 0.0), vec3(0.3, 0.3, 0.3), chrome);
    let h = m * Mat4::from_translation(vec3(0.0, 3.2, 0.0)) * Mat4::from_rotation_z((time * 1.3).sin() * 0.06);
    b.cube(&h, Vec3::ZERO, vec3(0.9, 0.7, 0.85), chrome);
    let visor = if dead.is_some() { rgb(0.25, 0.05, 0.05) } else { hsv(0.47 + (time * 0.7).sin() * 0.03, 0.8, 1.0) };
    b.glow(&h, vec3(0.0, 0.08, 0.43), vec3(0.75, 0.18, 0.02), visor);
    if dead.is_none() && (time * 3.0).fract() < 0.5 {
        b.glow(&h, vec3(-0.15 + (time * 4.0).sin() * 0.2, 0.08, 0.45), vec3(0.12, 0.12, 0.02), WHITE);
    }
    b.cube(&h, vec3(0.0, -0.22, 0.43), vec3(0.4, 0.06, 0.02), dark);
    let pulse = ((time * 2.4).sin() * 0.5 + 0.5).powi(2);
    for (q, sz) in [(vec3(-0.17, 0.55, 0.0), vec3(0.32, 0.3, 0.6)), (vec3(0.17, 0.55, 0.0), vec3(0.32, 0.3, 0.6))] {
        b.glow(&h, q, sz, Color::new(1.0, 0.45 + 0.3 * pulse, 0.7, 1.0));
    }
    trans.glow(&h, vec3(0.0, 0.58, 0.0), vec3(0.85, 0.5, 0.8), Color::new(0.7, 1.0, 1.0, 0.25));
    // Braço esquerdo com prancheta; direito erguido com emissor ligado no topo da cúpula
    let sh_l = m.transform_point3(vec3(-0.8, 2.4, 0.0));
    let sh_r = m.transform_point3(vec3(0.8, 2.4, 0.0));
    let clip = m.transform_point3(vec3(-0.45, 1.6, 0.6));
    limb(b, sh_l, clip, 0.22, coat);
    let cm = Mat4::from_translation(clip) * Mat4::from_rotation_y(yaw) * Mat4::from_rotation_x(-0.5);
    b.cube(&cm, Vec3::ZERO, vec3(0.6, 0.75, 0.05), rgb(0.45, 0.3, 0.18));
    b.glow(&cm, vec3(0.0, 0.0, 0.03), vec3(0.5, 0.6, 0.02), WHITE);
    let hand = if dead.is_some() { m.transform_point3(vec3(1.2, 1.4, 0.4)) } else { m.transform_point3(vec3(1.0, 3.9 + (time * 1.5).sin() * 0.15, 0.3)) };
    limb(b, sh_r, hand, 0.22, coat);
    b.glow(&Mat4::from_translation(hand), Vec3::ZERO, Vec3::splat(0.3), cyan);

    let top = dome_center() + vec3(0.0, DOME_R, 0.0);
    if let Some(v) = dead {
        if (time * 9.0).sin() > 0.4 {
            trans.glow(&h, vec3(0.3, 0.3, 0.0), Vec3::splat(0.4), Color::new(1.0, 0.8, 0.3, 0.9));
        }
        if p.distance(eye) < 45.0 {
            labels.push(Label { pos: p + vec3(0.0, 2.5, 0.0), text: format!("DRA. SINAPSE-9 REINICIANDO ({:.0}s) - A CUPULA FICA", v.down.max(0.0)), size: 18.0, color: rgb(1.0, 0.5, 0.4) });
        }
        return;
    }
    let k = 0.6 + 0.4 * (time * 7.0).sin();
    beam(trans, hand, top, 0.35 + 0.25 * k, Color::new(0.3, 1.0, 0.8, 0.35));
    beam(trans, hand, top, 0.1, Color::new(1.0, 1.0, 1.0, 0.8));
    trans.glow(&Mat4::from_translation(top), Vec3::ZERO, Vec3::splat(1.2 + k), Color::new(0.3, 1.0, 0.8, 0.5));
    if p.distance(eye) < 14.0 {
        const FALAS: [&str; 5] = ["SO LEIO ARTIGO PUBLICADO. NAO INVENTO.", "URNA NAO ENTRA NA CUPULA", "CORRELACAO NAO E CAUSALIDADE", "ESTUDO EM RATO NAO E CURA", "/lab PRO ULTIMO ACHADO"];
        labels.push(Label { pos: p + vec3(0.0, 4.6, 0.0), text: format!("\"{}\"", FALAS[(time / 6.0) as usize % FALAS.len()]), size: 18.0, color: rgb(0.6, 1.0, 0.9) });
        labels.push(Label { pos: p + vec3(0.0, 4.1, 0.0), text: "DRA. SINAPSE-9, GUARDIA DO LAB".into(), size: 18.0, color: rgb(0.95, 0.95, 1.0) });
    }
}

