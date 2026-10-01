//! Lugares funcionais da vila (Congresso, Bolsa, TV, Banco, Terminal). Cada um é um módulo com prédio
//! (voxel, na geração do mundo), painel holográfico/telão no mundo (render target, nunca HUD) e estado
//! que vem do servidor em {t:"pl", k:<lugar>} (server/places.js). O main só fala com `Places`.
#![allow(dead_code)]

use crate::batch::Batch;
use crate::extras::Label;
use crate::player::Player;
use crate::world::World;
use macroquad::prelude::*;
use serde_json::Value;

pub mod banco;
pub mod bolsa;
pub mod congresso;
pub mod terminal;
pub mod tv;

/// Tamanho lógico de toda tela (as funções de desenho de crate::lab::panel usam esse espaço).
pub const TW: f32 = 2048.0;
pub const TH: f32 = 1024.0;

/// Leis em vigor (Congresso) que o main aplica no jogo.
#[derive(Default, Clone, Copy)]
pub struct Laws {
    /// Gravidade lunar (player.grav).
    pub lua: bool,
    /// Turbo (player.speed_mul).
    pub turbo: bool,
    /// Urna gigante não atira.
    pub paz: bool,
    /// Fogos + céu de balada.
    pub festa: bool,
}

/// Interface de cada lugar. Tudo tem padrão vazio: o lugar só implementa o que usa.
pub trait Place {
    /// Mensagem {t:"pl", k:<chave do lugar>}.
    fn on_msg(&mut self, _m: &Value) {}
    /// Mensagem {t:"hub"} (lista de portais com presença), pra quem precisar.
    fn on_hub(&mut self, _m: &Value) {}
    /// Interação por posição (pisar em púlpito, portão...). Mensagens pro servidor vão em `out`.
    fn update(&mut self, _p: &mut Player, _dt: f32, _time: f32, _online: bool, _out: &mut Vec<Value>) {}
    /// Redesenha render targets (antes do passe 3D). Só perto e com intervalo por qualidade.
    fn render(&mut self, _time: f32, _eye: Vec3) {}
    /// Decoração em cubos + rótulos (passe 3D, antes do flush).
    fn draw(&self, _b: &mut Batch, _trans: &mut Batch, _labels: &mut Vec<Label>, _time: f32, _eye: Vec3) {}
    /// Quad da tela (depois do batch transparente, com depth test).
    fn draw_screen(&self, _time: f32, _eye: Vec3) {}
}

pub struct Places {
    pub congresso: congresso::Congresso,
    pub bolsa: bolsa::Bolsa,
    pub tv: tv::Tv,
    pub banco: banco::Banco,
    pub terminal: terminal::Terminal,
}

impl Places {
    pub fn new() -> Self {
        Places { congresso: congresso::Congresso::new(), bolsa: bolsa::Bolsa::new(), tv: tv::Tv::new(), banco: banco::Banco::new(), terminal: terminal::Terminal::new() }
    }

    fn all(&mut self) -> [&mut dyn Place; 5] {
        [&mut self.congresso, &mut self.bolsa, &mut self.tv, &mut self.banco, &mut self.terminal]
    }

    fn each(&self) -> [&dyn Place; 5] {
        [&self.congresso, &self.bolsa, &self.tv, &self.banco, &self.terminal]
    }

    pub fn on_msg(&mut self, m: &Value) {
        let k = m["k"].as_str().unwrap_or("");
        let p: &mut dyn Place = match k {
            "lei" => &mut self.congresso,
            "bolsa" => &mut self.bolsa,
            "tv" => &mut self.tv,
            "banco" => &mut self.banco,
            "term" | "go" => &mut self.terminal,
            _ => return,
        };
        p.on_msg(m);
    }

    pub fn on_hub(&mut self, m: &Value) {
        for p in self.all() {
            p.on_hub(m);
        }
    }

    /// Retorna as mensagens pro servidor (o main só manda se estiver online).
    pub fn update(&mut self, player: &mut Player, dt: f32, time: f32, online: bool) -> Vec<Value> {
        let mut out = Vec::new();
        for p in self.all() {
            p.update(player, dt, time, online, &mut out);
        }
        out
    }

    pub fn render(&mut self, time: f32, eye: Vec3) {
        for p in self.all() {
            p.render(time, eye);
        }
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        for p in self.each() {
            p.draw(b, trans, labels, time, eye);
        }
    }

    pub fn draw_screens(&self, time: f32, eye: Vec3) {
        for p in self.each() {
            p.draw_screen(time, eye);
        }
    }

    pub fn laws(&self) -> Laws {
        self.congresso.laws()
    }
}

/// Prédios na geração do mundo (World::generate, depois das ruas e árvores).
pub fn build(w: &mut World) {
    congresso::build(w);
    bolsa::build(w);
    tv::build(w);
    banco::build(w);
    terminal::build(w);
}

// ---------------------------------------------------------------- ajudas compartilhadas

/// Caixa cheia de blocos (inclusiva), via World::set (o escudo ainda está desligado na geração).
pub fn fill(w: &mut World, (x0, y0, z0): (i32, i32, i32), (x1, y1, z1): (i32, i32, i32), b: u8) {
    for y in y0.min(y1)..=y0.max(y1) {
        for z in z0.min(z1)..=z0.max(z1) {
            for x in x0.min(x1)..=x0.max(x1) {
                w.set(x, y, z, b);
            }
        }
    }
}

/// Lote limpo: ar do chão até `h` acima, piso de `floor` e fundação de pedra.
pub fn clear_lot(w: &mut World, r: crate::layout::Rect, h: i32, floor: u8) {
    let g = crate::world::G;
    fill(w, (r.0, g, r.1), (r.2, g + h, r.3), crate::world::AIR);
    fill(w, (r.0, g - 3, r.1), (r.2, g - 2, r.3), crate::world::STONE);
    fill(w, (r.0, g - 1, r.1), (r.2, g - 1, r.3), floor);
}

/// Onde fica uma tela no mundo: centro, normal (horizontal, pra onde a tela olha), largura, altura.
#[derive(Clone, Copy)]
pub struct Geo {
    pub c: Vec3,
    pub n: Vec3,
    pub w: f32,
    pub h: f32,
}

impl Geo {
    /// Direita de quem olha a tela de frente.
    pub fn right(&self) -> Vec3 {
        (-self.n).cross(Vec3::Y).normalize_or_zero()
    }

    pub fn facing(&self, eye: Vec3) -> bool {
        (eye - self.c).dot(self.n) > 0.05
    }
}

/// Render target de uma tela com redesenho limitado por qualidade, distância e mudança de conteúdo.
/// Resolução física: 1024x512 (média/alta) ou 512x256 (baixa) pro espaço lógico TW x TH.
pub struct Screen {
    rt: Option<RenderTarget>,
    last: f32,
    key: u64,
}

impl Default for Screen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen {
    pub fn new() -> Self {
        Screen { rt: None, last: -100.0, key: u64::MAX }
    }

    /// Prepara a câmera na render target se precisar repintar. `key` = hash do conteúdo; `animated` repinta
    /// mesmo sem mudar (no intervalo da qualidade). Retorna true: pinte em (0,0)-(TW,TH) e chame `end`.
    pub fn begin(&mut self, time: f32, eye: Vec3, g: &Geo, key: u64, animated: bool) -> bool {
        let d = eye.distance(g.c);
        if d > 220.0 {
            self.rt = None;
            return false;
        }
        if d > 110.0 || !g.facing(eye) {
            return false;
        }
        let (every, res) = crate::quality::pick([(1.0, 0.25), (0.25, 0.5), (0.1, 0.5)]);
        let fresh = self.rt.as_ref().is_some_and(|rt| rt.texture.width() == TW * res);
        if fresh && ((time - self.last < every && time >= self.last) || (!animated && key == self.key)) {
            return false;
        }
        if !fresh {
            self.rt = None;
        }
        self.last = time;
        self.key = key;
        let rt = self
            .rt
            .get_or_insert_with(|| {
                let rt = render_target((TW * res) as u32, (TH * res) as u32);
                rt.texture.set_filter(FilterMode::Linear);
                rt
            })
            .clone();
        let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, TW, TH));
        cam.render_target = Some(rt);
        set_camera(&cam);
        clear_background(Color::new(0.0, 0.05, 0.09, 0.85));
        true
    }

    pub fn end(&self) {
        set_default_camera();
        if let Some(rt) = &self.rt {
            crate::lab::panel::mipmaps(rt);
        }
    }

    /// Quad da tela no mundo (só de frente e até `far`).
    pub fn draw(&self, g: &Geo, time: f32, eye: Vec3, far: f32) {
        let Some(rt) = &self.rt else { return };
        if !g.facing(eye) || eye.distance(g.c) > far {
            return;
        }
        let glitch = (time * 12.0 + g.c.x).sin() * (time * 6.7).sin() > 0.96;
        let col = Color::new(1.0, 1.0, 1.0, if glitch { 0.7 } else { 0.95 });
        let c = g.c + g.n * 0.05;
        let (r, u) = (g.right() * g.w * 0.5, Vec3::Y * g.h * 0.5);
        draw_mesh(&Mesh {
            vertices: vec![
                Vertex::new2(c - r + u, vec2(0.0, 1.0), col),
                Vertex::new2(c + r + u, vec2(1.0, 1.0), col),
                Vertex::new2(c + r - u, vec2(1.0, 0.0), col),
                Vertex::new2(c - r - u, vec2(0.0, 0.0), col),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(rt.texture.clone()),
        });
    }
}

/// Hash barato pra chave de conteúdo da tela.
pub fn hash_str(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

pub fn text_mid(t: &str, x: f32, y: f32, size: f32, col: Color) {
    let w = measure_text(t, None, size as u16, 1.0).width;
    draw_text(t, x - w * 0.5, y, size, col);
}

/// Quebra por largura em pixels, até `max` linhas.
pub fn wrap(t: &str, w: f32, size: f32, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new()];
    for word in t.split_whitespace() {
        let cur = lines.last().unwrap();
        let next = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if measure_text(&next, None, size as u16, 1.0).width > w && !cur.is_empty() {
            if lines.len() == max {
                let last = lines.pop().unwrap();
                lines.push(crate::lab::panel::fit(&format!("{last} {word}"), w, size));
                return lines;
            }
            lines.push(word.to_string());
        } else {
            *lines.last_mut().unwrap() = next;
        }
    }
    lines
}

pub fn s(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}
