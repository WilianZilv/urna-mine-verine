//! Arma de portal (personagem 6): portais em faces planas de bloco, cada um mostra a vista do par
//! (câmera virtual atravessando o par → render target, near plane oblíquo no portal de saída),
//! teleporte com momento preservado, cubo companheiro. Cada jogador tem seu par azul/laranja e
//! todos os portais do mapa funcionam pra qualquer um. Colocação vai pelo log de mundo ("w" k:"portal").

use crate::actors::Villager;
use crate::audio::{Audio, Clip};
use crate::batch::Batch;
use crate::models::{Look, Pose, draw_humanoid, root};
use crate::player::Player;
use crate::urna::{Fx, Particle};
use crate::world::*;
use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Comparison, Equation, PipelineParams, ShaderSource};
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

const HALF_W: f32 = 0.5;
const HALF_H: f32 = 1.0;
const BODY: Vec3 = Vec3::new(0.3, 0.9, 0.3);
const CUBE: Vec3 = Vec3::new(0.3, 0.3, 0.3);
const MAX_CUBES: usize = 3;
const SHOOT: usize = 0;
const OPEN: usize = 1;
const FIZZLE: usize = 2;
const WHOOSH: usize = 3;

const VERT: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec4 v_col;
varying vec2 v_uv;
varying vec4 v_clip;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    v_clip = gl_Position;
    v_uv = texcoord;
    v_col = color0 / 255.0;
}
"#;

/// Vista pelo portal: amostra o render target na posição de tela do fragmento (vira janela).
const VIEW_FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec4 v_col;
varying vec2 v_uv;
varying vec4 v_clip;
uniform sampler2D Texture;
uniform vec4 _Time;
void main() {
    vec2 q = v_uv * 2.0 - 1.0;
    float d = length(q);
    if (d > 1.0) discard;
    vec2 s = v_clip.xy / v_clip.w * 0.5 + 0.5;
    vec3 view = texture2D(Texture, s).rgb;
    float wave = 0.5 + 0.5 * sin(atan(q.y, q.x) * 6.0 - _Time.x * 5.0 + d * 9.0);
    float rim = smoothstep(0.8, 0.88, d);
    vec3 c = mix(view, v_col.rgb * (1.1 + 0.5 * wave), rim);
    gl_FragColor = vec4(c, 1.0 - smoothstep(0.93, 1.0, d));
}
"#;

/// Portal sem par / longe demais / dentro de outra vista: redemoinho colorido.
const SWIRL_FRAG: &str = r#"#version 100
precision mediump float;
varying lowp vec4 v_col;
varying vec2 v_uv;
varying vec4 v_clip;
uniform vec4 _Time;
void main() {
    vec2 q = v_uv * 2.0 - 1.0;
    float d = length(q);
    if (d > 1.0) discard;
    float sw = 0.5 + 0.5 * sin(atan(q.y, q.x) * 3.0 + d * 12.0 - _Time.x * 6.0);
    vec3 c = v_col.rgb * (0.2 + 0.7 * sw * (1.0 - 0.6 * d) + smoothstep(0.8, 0.88, d));
    gl_FragColor = vec4(c, 1.0 - smoothstep(0.93, 1.0, d));
}
"#;

#[derive(Clone)]
pub struct Portal {
    pub owner: u64,
    pub color: u8,
    cell: IVec3,
    ni: IVec3,
    ui: IVec3,
    pub c: Vec3,
    pub n: Vec3,
    pub u: Vec3,
    age: f32,
}

impl Portal {
    fn new(owner: u64, color: u8, cell: IVec3, ni: IVec3, ui: IVec3) -> Self {
        let (n, u) = (ni.as_vec3(), ui.as_vec3());
        let c = cell.as_vec3() + Vec3::splat(0.5) + u * 0.5 + n * 0.53;
        Portal { owner, color, cell, ni, ui, c, n, u, age: 0.0 }
    }

    fn r(&self) -> Vec3 {
        self.u.cross(self.n)
    }

    fn cells(&self) -> [IVec3; 2] {
        [self.cell, self.cell + self.ui]
    }

    fn valid(&self, w: &World) -> bool {
        self.cells().iter().all(|&k| placeable(w, k) && !solid(w, k + self.ni))
    }
}

struct Cube {
    pos: Vec3,
    vel: Vec3,
    yaw: f32,
    cd: f32,
}

struct ViewCam {
    m: Mat4,
    pass: RenderPass,
}

impl Camera for ViewCam {
    fn matrix(&self) -> Mat4 {
        self.m
    }
    fn depth_enabled(&self) -> bool {
        true
    }
    fn render_pass(&self) -> Option<RenderPass> {
        Some(self.pass.clone())
    }
    fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
        None
    }
}

pub struct Portals {
    /// Jogando com a arma de portal.
    pub active: bool,
    pub list: Vec<Portal>,
    pub outbox: Vec<Value>,
    cubes: Vec<Cube>,
    held: Option<usize>,
    remote_cubes: HashMap<u64, Vec<(Vec3, Vec3, f32)>>,
    taps: [bool; 4],
    fire_cd: f32,
    tp_cd: f32,
    fling: Option<Vec2>,
    fling_t: f32,
    sounds: Vec<(usize, Vec3, f32)>,
    clips: [Clip; 4],
    view_mat: Option<Material>,
    swirl_mat: Option<Material>,
    rts: Vec<RenderTarget>,
    rt_size: (u32, u32),
    recoil: f32,
    last_color: u8,
    my_id: u64,
}

fn solid(w: &World, k: IVec3) -> bool {
    w.solid(k.x, k.y, k.z)
}

fn placeable(w: &World, k: IVec3) -> bool {
    solid(w, k) && !matches!(w.get(k.x, k.y, k.z), GLASS | LEAVES | TNT)
}

fn box_hits(w: &World, c: Vec3, h: Vec3) -> bool {
    let lo = (c - h + Vec3::splat(0.001)).floor().as_ivec3();
    let hi = (c + h - Vec3::splat(0.001)).floor().as_ivec3();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                if w.solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// Cores: os meus sempre azul/laranja, os dos outros em pares próprios (caos legível).
fn color_of(owner: u64, color: u8, me: u64) -> Color {
    const PAIRS: [[(f32, f32, f32); 2]; 4] = [
        [(0.15, 0.55, 1.0), (1.0, 0.55, 0.1)],
        [(0.65, 0.3, 1.0), (1.0, 0.2, 0.2)],
        [(0.1, 0.95, 0.85), (1.0, 0.9, 0.15)],
        [(0.3, 1.0, 0.35), (1.0, 0.4, 0.8)],
    ];
    let k = if owner == me { 0 } else { 1 + (owner % 3) as usize };
    let (r, g, b) = PAIRS[k][(color & 1) as usize];
    Color::new(r, g, b, 1.0)
}

fn partner(list: &[Portal], i: usize) -> Option<usize> {
    list.iter().position(|p| p.owner == list[i].owner && p.color != list[i].color)
}

/// Rotação que leva o espaço do portal A pro B (entra pela frente de A, sai pela frente de B).
fn basis(a: &Portal, b: &Portal) -> Mat3 {
    Mat3::from_cols(-b.r(), b.u, -b.n) * Mat3::from_cols(a.r(), a.u, a.n).transpose()
}

fn xform(a: &Portal, b: &Portal) -> Mat4 {
    Mat4::from_translation(b.c) * Mat4::from_mat3(basis(a, b)) * Mat4::from_translation(-a.c)
}

/// Near plane oblíquo (Lengyel): corta tudo atrás do plano `c` (espaço de câmera).
fn oblique(proj: Mat4, c: Vec4) -> Mat4 {
    let q = proj.inverse() * vec4(c.x.signum(), c.y.signum(), 1.0, 1.0);
    let c = c * (2.0 / c.dot(q));
    let mut t = proj.transpose();
    t.z_axis = c - t.w_axis;
    t.transpose()
}

/// Corpo (centro, meia-caixa, velocidade) entrando em algum portal com par neste passo.
fn entering(list: &[Portal], c: Vec3, h: Vec3, v: Vec3, dt: f32) -> Option<(usize, usize)> {
    let tol = if h == Vec3::ZERO { 0.0 } else { 0.15 };
    for (ia, a) in list.iter().enumerate() {
        let vn = v.dot(a.n);
        if vn > -0.05 {
            continue;
        }
        let Some(ib) = partner(list, ia) else { continue };
        let (d, r) = (c - a.c, a.r());
        if d.dot(r).abs() > (HALF_W - h.dot(r.abs())).max(0.0) + tol || d.dot(a.u).abs() > (HALF_H - h.dot(a.u.abs())).max(0.0) + tol + 0.05 {
            continue;
        }
        let gap = d.dot(a.n) - h.dot(a.n.abs());
        if gap > -0.4 && gap + vn * dt < 0.04 {
            return Some((ia, ib));
        }
    }
    None
}

/// Onde o corpo sai do portal B: na frente dele, sem entrar na parede, com velocidade girada.
#[allow(clippy::too_many_arguments)]
fn exit(list: &[Portal], ia: usize, ib: usize, c: Vec3, h: Vec3, v: Vec3, w: &World, min_up: f32) -> Option<(Vec3, Vec3, Mat3)> {
    let (a, b) = (&list[ia], &list[ib]);
    let m = basis(a, b);
    let d = c - a.c;
    let (rb, ub, nb) = (b.r(), b.u, b.n);
    let lim = |axis: Vec3, half: f32| (half - h.dot(axis.abs())).max(0.0);
    let lx = (-d.dot(a.r())).clamp(-lim(rb, HALF_W), lim(rb, HALF_W));
    let ly = d.dot(a.u).clamp(-lim(ub, HALF_H), lim(ub, HALF_H));
    let mut v2 = m * v;
    let min = if nb.y > 0.5 { min_up } else { 1.0 };
    let vn = v2.dot(nb);
    if vn < min {
        v2 += nb * (min - vn);
    }
    let ext = h.dot(nb.abs());
    let lift = if nb.y.abs() < 0.5 { Vec3::Y * 0.02 } else { Vec3::ZERO };
    for (lx, ly) in [(lx, ly), (0.0, 0.0)] {
        for k in 0..8 {
            let p = b.c + rb * lx + ub * ly + nb * (ext + 0.03 + k as f32 * 0.1) + lift;
            if !box_hits(w, p, h) {
                return Some((p, v2, m));
            }
        }
    }
    None
}

/// Mira na face: (célula de baixo, normal, "cima" do portal) se tiver área plana 1x2 livre.
fn aim(w: &World, eye: Vec3, fw: Vec3) -> Result<(IVec3, IVec3, IVec3), Option<Vec3>> {
    let Some((b, prev, t)) = w.raycast(eye, fw, 80.0) else { return Err(None) };
    let hit = eye + fw * t;
    let ni = prev - b;
    if ni.abs().element_sum() != 1 {
        return Err(Some(hit));
    }
    let ui = if ni.y == 0 {
        IVec3::Y
    } else if fw.x.abs() > fw.z.abs() {
        ivec3(fw.x.signum() as i32, 0, 0)
    } else {
        ivec3(0, 0, fw.z.signum() as i32)
    };
    let f = (hit - (b.as_vec3() + Vec3::splat(0.5))).dot(ui.as_vec3());
    let tries = if f >= 0.0 { [b, b - ui] } else { [b - ui, b] };
    tries.into_iter().find(|&cell| [cell, cell + ui].iter().all(|&k| placeable(w, k) && !solid(w, k + ni))).map(|cell| (cell, ni, ui)).ok_or(Some(hit))
}

fn iv(v: &Value) -> IVec3 {
    ivec3(v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32)
}

fn synth(sr: u32, secs: f32, mut f: impl FnMut(f32, f32) -> f32) -> Clip {
    let mut s = 0x9E37_79B9u32;
    let n = (secs * sr as f32) as usize;
    Arc::new(
        (0..n)
            .map(|i| {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                f(i as f32 / sr as f32, s as f32 / u32::MAX as f32 * 2.0 - 1.0).clamp(-1.0, 1.0)
            })
            .collect(),
    )
}

fn clips(sr: u32) -> [Clip; 4] {
    let dt = 1.0 / sr as f32;
    let (mut ph, mut ph2, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let shoot = synth(sr, 0.25, |t, _| {
        ph += (300.0 + 900.0 * (-t * 14.0).exp()) * dt;
        ((TAU * ph).sin() * 0.6 + (2.0 * (ph * 2.0).fract() - 1.0) * 0.2) * (-t / 0.07).exp() * (1.0 - (-t / 0.003).exp())
    });
    let (mut ph, mut lp2) = (0.0f32, 0.0f32);
    let open = synth(sr, 0.7, |t, n| {
        let f = 110.0 + 260.0 * (1.0 - (-t * 6.0).exp());
        ph += f * dt;
        ph2 += f * 1.502 * dt;
        lp2 += (n - lp2) * 0.06;
        let env = (t / 0.02).min(1.0) * (-t / 0.3).exp();
        ((TAU * ph).sin() * 0.5 + (TAU * ph2).sin() * 0.3 + lp2 * 1.2 * (0.5 + 0.5 * (TAU * 22.0 * t).sin())) * env
    });
    let mut ph3 = 0.0f32;
    let fizzle = synth(sr, 0.35, |t, n| {
        ph3 += (420.0 - 320.0 * t / 0.35) * dt;
        let crackle = if n.abs() > 0.7 { n } else { n * 0.2 };
        (crackle * 0.6 * (-t / 0.09).exp() + (TAU * ph3).sin() * 0.3 * (-t / 0.15).exp()) * 0.8
    });
    let whoosh = synth(sr, 0.45, |t, n| {
        let k = 0.02 + 0.3 * (PI * t / 0.45).sin().powi(2);
        lp += (n - lp) * k;
        lp * 1.6 * (PI * t / 0.45).sin()
    });
    [shoot, open, fizzle, whoosh]
}

fn material(frag: &str) -> Option<Material> {
    let pipeline_params = PipelineParams {
        depth_test: Comparison::LessOrEqual,
        depth_write: true,
        color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::OneMinusValue(BlendValue::SourceAlpha))),
        ..Default::default()
    };
    load_material(ShaderSource::Glsl { vertex: VERT, fragment: frag }, MaterialParams { pipeline_params, ..Default::default() }).map_err(|e| eprintln!("portal shader: {e:?}")).ok()
}

fn draw_quad(p: &Portal, col: Color, tex: Option<&Texture2D>) {
    let s = (p.age / 0.25).min(1.0);
    let s = s * s * (3.0 - 2.0 * s);
    let (r, u) = (p.r() * HALF_W * 1.15 * s, p.u * HALF_H * 1.06 * s);
    let v = |q: Vec3, uv: Vec2| Vertex::new2(q, uv, col);
    draw_mesh(&Mesh {
        vertices: vec![v(p.c - r - u, vec2(0.0, 0.0)), v(p.c + r - u, vec2(1.0, 0.0)), v(p.c + r + u, vec2(1.0, 1.0)), v(p.c - r + u, vec2(0.0, 1.0))],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: tex.cloned(),
    });
}

/// Fração da tela coberta (aprox.) ou None se fora do frustum.
fn on_screen(vp: &Mat4, p: &Portal) -> Option<f32> {
    let (r, u) = (p.r() * HALF_W, p.u * HALF_H);
    let (mut lo, mut hi, mut behind, mut front) = (Vec2::splat(9.0), Vec2::splat(-9.0), false, false);
    for q in [p.c - r - u, p.c + r - u, p.c + r + u, p.c - r + u] {
        let h = *vp * q.extend(1.0);
        if h.w <= 0.01 {
            behind = true;
            continue;
        }
        front = true;
        let n = h.truncate().truncate() / h.w;
        lo = lo.min(n);
        hi = hi.max(n);
    }
    match (front, behind) {
        (false, _) => None,
        (true, true) => Some(4.0),
        _ if hi.x < -1.0 || lo.x > 1.0 || hi.y < -1.0 || lo.y > 1.0 => None,
        _ => Some((hi.x - lo.x) * (hi.y - lo.y)),
    }
}

fn draw_cube(b: &mut Batch, pos: Vec3, yaw: f32) {
    let m = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw);
    b.cube(&m, Vec3::ZERO, Vec3::splat(0.56), Color::new(0.72, 0.73, 0.75, 1.0));
    for k in 0..8 {
        let s = vec3(if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 });
        b.cube(&m, s * 0.2, Vec3::splat(0.2), Color::new(0.42, 0.43, 0.46, 1.0));
    }
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        for s in [-1.0, 1.0] {
            let size = Vec3::splat(0.2) - axis * 0.18;
            b.glow(&m, axis * 0.285 * s, size, Color::new(1.0, 0.45, 0.72, 1.0));
        }
    }
}

impl Portals {
    pub fn new(sample_rate: u32) -> Self {
        Portals {
            active: false,
            list: Vec::new(),
            outbox: Vec::new(),
            cubes: Vec::new(),
            held: None,
            remote_cubes: HashMap::new(),
            taps: [false; 4],
            fire_cd: 0.0,
            tp_cd: 0.0,
            fling: None,
            fling_t: 0.0,
            sounds: Vec::new(),
            clips: clips(sample_rate),
            view_mat: material(VIEW_FRAG),
            swirl_mat: material(SWIRL_FRAG),
            rts: Vec::new(),
            rt_size: (0, 0),
            recoil: 0.0,
            last_color: 0,
            my_id: 0,
        }
    }

    /// Botões do celular (1 azul, 2 laranja, 3 cubo).
    pub fn tap(&mut self, b: usize) {
        if let Some(t) = self.taps.get_mut(b) {
            *t = true;
        }
    }

    // ------------------------------------------------------------ rede

    pub fn on_world(&mut self, m: &Value) {
        match m["k"].as_str() {
            Some("reset") => self.list.clear(),
            Some("portal") => {
                let owner = m["id"].as_u64().unwrap_or(0);
                let color = m["c"].as_u64().unwrap_or(0).min(1) as u8;
                let (ni, ui) = (iv(&m["n"]), iv(&m["u"]));
                if ni.abs().element_sum() != 1 || ui.abs().element_sum() != 1 || ni.dot(ui) != 0 {
                    return;
                }
                let p = Portal::new(owner, color, iv(&m["b"]), ni, ui);
                let cells = p.cells();
                self.list.retain(|q| !(q.owner == owner && q.color == color) && !(q.ni == p.ni && q.cells().iter().any(|k| cells.contains(k))));
                self.sounds.push((OPEN, p.c, 1.0));
                self.list.push(p);
            }
            _ => {}
        }
    }

    /// Depois do log do welcome: só ficam portais de quem está online.
    pub fn joined(&mut self, online: impl Fn(u64) -> bool) {
        self.list.retain(|p| online(p.owner));
        self.remote_cubes.clear();
        self.sounds.clear();
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.remote_cubes.clear();
    }

    pub fn remove_owner(&mut self, id: u64) {
        self.list.retain(|p| p.owner != id);
        self.remote_cubes.remove(&id);
    }

    pub fn fill_p(&self, pm: &mut Value) {
        if !self.cubes.is_empty() {
            let r = |x: f32| (x * 100.0).round() / 100.0;
            pm["pc"] = Value::Array(self.cubes.iter().map(|c| json!([r(c.pos.x), r(c.pos.y), r(c.pos.z), r(c.yaw)])).collect());
        }
    }

    pub fn on_p(&mut self, id: u64, m: &Value) {
        let Some(arr) = m["pc"].as_array() else {
            self.remote_cubes.remove(&id);
            return;
        };
        let list = self.remote_cubes.entry(id).or_default();
        list.resize(arr.len(), (Vec3::NAN, Vec3::ZERO, 0.0));
        for (c, a) in list.iter_mut().zip(arr) {
            let f = |k: usize| a[k].as_f64().unwrap_or(0.0) as f32;
            c.1 = vec3(f(0), f(1), f(2));
            c.2 = f(3);
            if c.0.is_nan() || c.0.distance(c.1) > 6.0 {
                c.0 = c.1;
            }
        }
    }

    /// Jogador remoto pulou de um portal pro par: teleporta em vez de deslizar pela parede.
    pub fn jumped(&self, from: Vec3, to: Vec3) -> bool {
        (0..self.list.len()).any(|i| partner(&self.list, i).is_some_and(|j| from.distance(self.list[i].c) < 2.5 && to.distance(self.list[j].c) < 2.5))
    }

    // ------------------------------------------------------------ simulação

    /// Antes do Player::update: se o passo previsto entra num portal, sai pelo par.
    pub fn pre_move(&mut self, w: &World, pl: &mut Player, dt: f32, input: bool) {
        self.tp_cd -= dt;
        if let Some(f) = self.fling {
            self.fling_t += dt;
            if (pl.on_ground && self.fling_t > 0.1) || pl.fly || self.fling_t > 6.0 {
                self.fling = None;
            } else {
                pl.knock.x = f.x;
                pl.knock.z = f.y;
                self.fling = Some(f * (1.0 - 0.15 * dt));
            }
        }
        if self.tp_cd > 0.0 || self.list.len() < 2 {
            return;
        }
        let key = |k: KeyCode| input && is_key_down(k);
        let f = vec3(pl.yaw.cos(), 0.0, pl.yaw.sin());
        let r = vec3(-f.z, 0.0, f.x);
        let stick = if input { pl.stick } else { Vec2::ZERO };
        let wish = if stick.length() > 0.1 {
            f * stick.y + r * stick.x
        } else {
            (f * (key(KeyCode::W) as i32 - key(KeyCode::S) as i32) as f32 + r * (key(KeyCode::D) as i32 - key(KeyCode::A) as i32) as f32).normalize_or_zero()
        };
        let sprint = key(KeyCode::LeftShift) || stick.length() > 0.95;
        let speed = match (pl.fly, sprint) {
            (true, true) => 25.0,
            (true, false) => 12.0,
            (false, true) => 6.5,
            _ => 4.3,
        };
        let vy = if pl.fly { ((key(KeyCode::Space) || pl.jump_held) as i32 - key(KeyCode::LeftControl) as i32) as f32 * speed } else { pl.vel.y - 28.0 * dt } + pl.knock.y;
        let v = vec3(wish.x * speed + pl.knock.x, vy, wish.z * speed + pl.knock.z);
        let c = pl.pos + Vec3::Y * 0.9;
        let Some((ia, ib)) = entering(&self.list, c, BODY, v, dt) else { return };
        let Some((c2, v2, m)) = exit(&self.list, ia, ib, c, BODY, v, w, 6.0) else { return };
        self.sounds.push((WHOOSH, self.list[ia].c, 0.8));
        self.sounds.push((WHOOSH, c2, 0.8));
        pl.pos = c2 - Vec3::Y * 0.9;
        pl.vel = vec3(0.0, v2.y, 0.0);
        pl.on_ground = false;
        let walk_out = m * (wish * speed);
        let extra = vec2(v2.x - walk_out.x, v2.z - walk_out.z);
        pl.knock = Vec3::ZERO;
        self.fling = (extra.length() > 0.5).then_some(extra);
        self.fling_t = 0.0;
        if let Some(e) = self.fling {
            pl.knock.x = e.x;
            pl.knock.z = e.y;
        }
        let f3 = m * pl.forward();
        let g = m * f;
        let h = [vec2(f3.x, f3.z), vec2(g.x, g.z)].into_iter().find(|h| h.length() > 0.2);
        if let Some(h) = h {
            pl.yaw = h.y.atan2(h.x);
        }
        pl.pitch = f3.y.clamp(-1.0, 1.0).asin().clamp(-1.55, 1.55);
        self.tp_cd = 0.12;
        if let Some(c) = self.held.and_then(|i| self.cubes.get_mut(i)) {
            c.pos = pl.eye();
            c.vel = v2;
            c.cd = 0.3;
        }
    }

    /// Tiro dos portais, cubos, partículas e villagers voando passando pelos portais.
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, w: &World, pl: &Player, eye: Vec3, fw: Vec3, mouse_ok: bool, active: bool, my_id: u64, is_host: bool, villagers: &mut [Villager], fx: &mut Fx, dt: f32) {
        self.my_id = my_id;
        self.fire_cd -= dt;
        self.recoil = (self.recoil - dt * 5.0).max(0.0);
        let gone: Vec<Vec3> = self.list.iter().filter(|p| !p.valid(w)).map(|p| p.c).collect();
        if !gone.is_empty() {
            self.list.retain(|p| p.valid(w));
            self.sounds.extend(gone.into_iter().map(|c| (FIZZLE, c, 0.8)));
        }
        for p in self.list.iter_mut() {
            p.age += dt;
        }
        let taps = std::mem::take(&mut self.taps);
        if !self.active {
            self.held = None;
        } else {
            let fire = [taps[1] || (mouse_ok && is_mouse_button_pressed(MouseButton::Left)), taps[2] || (mouse_ok && is_mouse_button_pressed(MouseButton::Right))];
            for (color, go) in fire.into_iter().enumerate() {
                if go && self.fire_cd <= 0.0 {
                    self.fire(w, eye, fw, color as u8, fx);
                }
            }
            let grab = taps[3] || (active && is_key_pressed(KeyCode::E));
            if active && is_key_pressed(KeyCode::Q) {
                self.spawn_cube(w, eye, fw, pl.yaw);
            } else if grab {
                if let Some(i) = self.held.take() {
                    let c = &mut self.cubes[i];
                    c.vel = c.vel.clamp_length_max(8.0);
                } else if let Some(i) = self.cubes.iter().enumerate().filter(|(_, c)| c.pos.distance(eye) < 2.8 && (c.pos - eye).normalize_or_zero().dot(fw) > 0.6).min_by(|a, b| a.1.pos.distance(eye).total_cmp(&b.1.pos.distance(eye))).map(|(i, _)| i) {
                    self.held = Some(i);
                } else if taps[3] {
                    self.spawn_cube(w, eye, fw, pl.yaw);
                }
            }
        }

        // Cubos
        for (i, c) in self.cubes.iter_mut().enumerate() {
            c.cd -= dt;
            if self.held == Some(i) {
                c.vel = ((eye + fw * 1.7 - c.pos) * 14.0).clamp_length_max(20.0);
                if c.pos.distance(eye) > 4.5 {
                    self.held = None;
                }
            } else {
                c.vel.y -= 25.0 * dt;
            }
            if c.cd <= 0.0 {
                if let Some((ia, ib)) = entering(&self.list, c.pos, CUBE, c.vel, dt) {
                    if let Some((p, v, m)) = exit(&self.list, ia, ib, c.pos, CUBE, c.vel, w, 3.0) {
                        self.sounds.push((WHOOSH, p, 0.5));
                        let d = m * vec3(c.yaw.cos(), 0.0, c.yaw.sin());
                        c.yaw = d.z.atan2(d.x);
                        c.pos = p;
                        c.vel = v;
                        c.cd = 0.12;
                        if self.held == Some(i) {
                            self.held = None;
                        }
                    }
                }
            }
            let steps = ((c.vel.length() * dt) / 0.25).ceil().max(1.0) as i32;
            let mut ground = false;
            for _ in 0..steps {
                for a in 0..3 {
                    let mut p = c.pos;
                    p[a] += c.vel[a] * dt / steps as f32;
                    if box_hits(w, p, CUBE) {
                        ground |= a == 1 && c.vel.y < 0.0;
                        c.vel[a] = 0.0;
                    } else {
                        c.pos = p;
                    }
                }
            }
            if ground {
                let k = (1.0 - 8.0 * dt).max(0.0);
                c.vel.x *= k;
                c.vel.z *= k;
            }
        }
        if let Some(i) = self.cubes.iter().position(|c| c.pos.y < -10.0) {
            self.cubes.remove(i);
            self.held = None;
        }
        for list in self.remote_cubes.values_mut() {
            for c in list.iter_mut() {
                c.0 = c.0.lerp(c.1, (dt * 10.0).min(1.0));
            }
        }

        // Partículas (estilhaços, fumaça, flechas de fogo...) e villagers arremessados
        for p in fx.particles.iter_mut() {
            if let Some((ia, ib)) = entering(&self.list, p.pos, Vec3::ZERO, p.vel, dt) {
                let (a, b) = (&self.list[ia], &self.list[ib]);
                let m = basis(a, b);
                p.pos = b.c + m * (p.pos - a.c);
                let lz = (p.pos - b.c).dot(b.n);
                p.pos += b.n * (0.05 - lz).max(0.0);
                p.vel = m * p.vel;
            }
        }
        if is_host {
            for v in villagers.iter_mut().filter(|v| v.airborne) {
                let c = v.pos + Vec3::Y * 0.9;
                if let Some((ia, ib)) = entering(&self.list, c, BODY, v.vel, dt) {
                    if let Some((p, vel, _)) = exit(&self.list, ia, ib, c, BODY, v.vel, w, 5.0) {
                        v.pos = p - Vec3::Y * 0.9;
                        v.vel = vel;
                    }
                }
            }
        }

        // Brilho na borda
        for p in &self.list {
            if gen_range(0.0, 1.0) < 0.25 {
                let a = gen_range(0.0, TAU);
                let o = p.r() * a.cos() * HALF_W * 1.05 + p.u * a.sin() * HALF_H * 1.02;
                fx.particles.push(Particle { pos: p.c + o, vel: o * 0.3 + p.n * 0.4, col: color_of(p.owner, p.color, my_id), life: 0.5, size: 0.05, gravity: false });
            }
        }
    }

    fn fire(&mut self, w: &World, eye: Vec3, fw: Vec3, color: u8, fx: &mut Fx) {
        self.fire_cd = 0.2;
        self.recoil = 1.0;
        self.last_color = color;
        let col = color_of(self.my_id, color, self.my_id);
        self.sounds.push((SHOOT, eye, 0.7));
        let target = match aim(w, eye, fw) {
            Ok((cell, ni, ui)) => {
                let p = Portal::new(self.my_id, color, cell, ni, ui);
                let cells = p.cells();
                let blocked = self.list.iter().any(|q| !(q.owner == self.my_id && q.color == color) && q.ni == ni && q.cells().iter().any(|k| cells.contains(k)));
                if blocked {
                    self.sounds.push((FIZZLE, p.c, 1.0));
                } else {
                    self.outbox.push(json!({"t": "w", "k": "portal", "c": color, "b": [cell.x, cell.y, cell.z], "n": [ni.x, ni.y, ni.z], "u": [ui.x, ui.y, ui.z]}));
                }
                Some(p.c)
            }
            Err(hit) => {
                if let Some(h) = hit {
                    self.sounds.push((FIZZLE, h, 1.0));
                    for _ in 0..10 {
                        fx.particles.push(Particle { pos: h, vel: vec3(gen_range(-2.0, 2.0), gen_range(0.0, 3.0), gen_range(-2.0, 2.0)), col, life: 0.4, size: 0.06, gravity: true });
                    }
                }
                hit
            }
        };
        let to = target.unwrap_or(eye + fw * 30.0);
        let from = eye + fw * 0.6;
        let n = (from.distance(to) * 2.0).min(80.0) as i32;
        for k in 0..n {
            let p = from.lerp(to, k as f32 / n.max(1) as f32);
            fx.particles.push(Particle { pos: p, vel: vec3(gen_range(-0.2, 0.2), gen_range(-0.2, 0.2), gen_range(-0.2, 0.2)), col, life: 0.25, size: 0.06, gravity: false });
        }
    }

    fn spawn_cube(&mut self, w: &World, eye: Vec3, fw: Vec3, yaw: f32) {
        let p = eye + fw * 1.4;
        let pos = if box_hits(w, p, CUBE) { eye } else { p };
        if self.cubes.len() >= MAX_CUBES {
            self.cubes.remove(0);
            self.held = None;
        }
        self.cubes.push(Cube { pos, vel: Vec3::ZERO, yaw, cd: 0.0 });
        self.sounds.push((OPEN, pos, 0.5));
    }

    pub fn sounds(&mut self, audio: &Audio, listener: Vec3, muted: bool) {
        for (k, pos, base) in self.sounds.drain(..) {
            let vol = (base / (1.0 + pos.distance(listener) / 22.0)).clamp(0.0, 1.0);
            if !muted && vol > 0.02 {
                audio.play(&self.clips[k], vol, false);
            }
        }
    }

    // ------------------------------------------------------------ render

    /// Antes do opaque.flush: cubos, vistas pelos portais (render targets), quads dos portais e arma.
    /// `body` = (pos, yaw, look, walk, andando) do jogador local pra ele se ver pelo portal.
    #[allow(clippy::too_many_arguments)]
    pub fn render(&mut self, opaque: &mut Batch, chunks: &crate::chunks::Chunks, tex: &Texture2D, cam: &Camera3D, sky: Color, body: Option<(Vec3, f32, Look, f32, bool)>, mobile: bool) {
        for c in &self.cubes {
            draw_cube(opaque, c.pos, c.yaw);
        }
        for list in self.remote_cubes.values() {
            for c in list {
                draw_cube(opaque, c.0, c.2);
            }
        }
        if !self.list.is_empty() {
            self.render_portals(opaque, chunks, tex, cam, sky, body, mobile);
        }
        if self.active {
            let fw = (cam.target - cam.position).normalize_or(Vec3::X);
            self.draw_gun(opaque, cam.position, fw);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_portals(&mut self, opaque: &Batch, chunks: &crate::chunks::Chunks, tex: &Texture2D, cam: &Camera3D, sky: Color, body: Option<(Vec3, f32, Look, f32, bool)>, mobile: bool) {
        let (Some(view_mat), Some(swirl_mat)) = (self.view_mat.clone(), self.swirl_mat.clone()) else { return };
        let (sw, sh) = (screen_width(), screen_height());
        let vp = cam.matrix();
        let me = self.my_id;
        let mut cands: Vec<(usize, usize, f32)> = (0..self.list.len())
            .filter_map(|i| {
                let a = &self.list[i];
                let j = partner(&self.list, i)?;
                ((cam.position - a.c).dot(a.n) > 0.02).then_some(())?;
                on_screen(&vp, a).filter(|s| *s > 0.0004)?;
                Some((i, j, a.c.distance(cam.position)))
            })
            .collect();
        cands.sort_by(|a, b| a.2.total_cmp(&b.2));
        // Qualidade baixa: sem vista (só o redemoinho)
        cands.truncate(crate::quality::pick([0, 1, if mobile { 1 } else { 2 }]));

        let scale = if mobile || crate::quality::tier() < crate::quality::HIGH { 0.4 } else { 0.5 };
        let size = (((sw * scale) as u32).clamp(64, 900), ((sh * scale) as u32).clamp(64, 900));
        if size != self.rt_size {
            self.rts.clear();
            self.rt_size = size;
        }
        while self.rts.len() < cands.len() {
            let rt = render_target_ex(size.0, size.1, RenderTargetParams { sample_count: 1, depth: true });
            rt.texture.set_filter(FilterMode::Linear);
            self.rts.push(rt);
        }

        let mut bodies = Batch::new();
        if let Some((pos, yaw, look, walk, moving)) = body {
            let pose = Pose { walk, walk_amt: if moving { 1.0 } else { 0.0 }, arm_l: -0.2, arm_r: -1.2, ..Default::default() };
            draw_humanoid(&mut bodies, &look, &pose, &root(pos, yaw, 0.0, 0.0));
        }
        let view = Mat4::look_at_rh(cam.position, cam.target, cam.up);
        let proj = Mat4::perspective_rh_gl(cam.fovy, sw / sh, 0.05, 400.0);
        for (k, &(i, j, _)) in cands.iter().enumerate() {
            let (a, b) = (&self.list[i], &self.list[j]);
            let v2 = view * xform(a, b).inverse();
            let plane = b.n.extend(-b.n.dot(b.c - b.n * 0.025));
            let pv = v2.inverse().transpose() * plane;
            let p2 = if pv.w < 0.0 { oblique(proj, pv) } else { proj };
            set_camera(&ViewCam { m: p2 * v2, pass: self.rts[k].render_pass.clone() });
            clear_background(sky);
            chunks.draw(&(p2 * v2), Some(&self.rts[k].render_pass), cam.position, f32::MAX);
            opaque.redraw(tex);
            bodies.redraw(tex);
            gl_use_material(&swirl_mat);
            for (_, p) in self.list.iter().enumerate().filter(|(q, _)| *q != j) {
                draw_quad(p, color_of(p.owner, p.color, me), None);
            }
            gl_use_default_material();
        }
        set_camera(cam);
        for (i, p) in self.list.iter().enumerate() {
            if (cam.position - p.c).dot(p.n) <= 0.0 {
                continue;
            }
            let col = color_of(p.owner, p.color, me);
            match cands.iter().position(|c| c.0 == i) {
                Some(k) => {
                    gl_use_material(&view_mat);
                    draw_quad(p, col, Some(&self.rts[k].texture));
                }
                None => {
                    gl_use_material(&swirl_mat);
                    draw_quad(p, col, None);
                }
            }
        }
        gl_use_default_material();
    }

    fn draw_gun(&self, b: &mut Batch, eye: Vec3, fw: Vec3) {
        let right = fw.cross(Vec3::Y).normalize_or(Vec3::X);
        let up = right.cross(fw);
        let pos = eye + right * 0.26 - up * 0.24 + fw * (0.5 - self.recoil * 0.06);
        let m = Mat4::from_cols(right.extend(0.0), up.extend(0.0), (-fw).extend(0.0), pos.extend(1.0));
        let col = color_of(self.my_id, self.last_color, self.my_id);
        let white = Color::new(0.92, 0.92, 0.9, 1.0);
        let dark = Color::new(0.12, 0.12, 0.14, 1.0);
        b.cube(&m, vec3(0.0, 0.0, 0.06), vec3(0.13, 0.13, 0.34), white);
        b.cube(&m, vec3(0.0, -0.11, 0.14), vec3(0.07, 0.14, 0.08), dark);
        b.cube(&m, vec3(0.0, 0.0, -0.15), vec3(0.09, 0.09, 0.12), dark);
        b.glow(&m, vec3(0.0, 0.07, 0.02), vec3(0.05, 0.02, 0.22), col);
        for k in 0..3 {
            let a = k as f32 * TAU / 3.0 + PI * 0.5;
            b.cube(&m, vec3(a.cos() * 0.07, a.sin() * 0.07, -0.25), vec3(0.025, 0.025, 0.12), Color::new(0.7, 0.7, 0.72, 1.0));
        }
        b.glow(&m, vec3(0.0, 0.0, -0.215), vec3(0.06, 0.06, 0.02), col);
    }

    pub fn hud(&self, sw: f32, sh: f32) {
        if !self.active {
            return;
        }
        let (cx, cy) = (sw * 0.5, sh * 0.5);
        for (color, dx) in [(0u8, -16.0), (1, 16.0)] {
            let col = color_of(self.my_id, color, self.my_id);
            draw_circle_lines(cx + dx, cy, 6.0, 2.0, col);
            if self.list.iter().any(|p| p.owner == self.my_id && p.color == color) {
                draw_circle(cx + dx, cy, 4.0, col);
            }
        }
        let txt = format!("ARMA DE PORTAL | portais no mapa: {} | cubos: {}/{}", self.list.len(), self.cubes.len(), MAX_CUBES);
        let d = measure_text(&txt, None, 22, 1.0);
        draw_rectangle(cx - d.width * 0.5 - 6.0, sh - 46.0, d.width + 12.0, 28.0, Color::new(0.0, 0.0, 0.0, 0.45));
        draw_text(&txt, cx - d.width * 0.5, sh - 26.0, 22.0, WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(w: &World) -> Vec<Portal> {
        let a = Portal::new(0, 0, ivec3(63, G, 104), ivec3(0, 0, -1), IVec3::Y);
        let b = Portal::new(0, 1, ivec3(73, G, 96), ivec3(0, 0, 1), IVec3::Y);
        assert!(a.valid(w) && b.valid(w));
        vec![a, b]
    }

    #[test]
    fn walk_through_wall_pair() {
        let w = World::generate();
        let list = pair(&w);
        let c = list[0].c + vec3(0.0, -0.1, -0.3);
        let v = vec3(0.0, 0.0, 4.3);
        let (ia, ib) = entering(&list, c, BODY, v, 1.0 / 60.0).expect("entra no azul");
        let (p, v2, _) = exit(&list, ia, ib, c, BODY, v, &w, 6.0).expect("sai no laranja");
        assert!((p - list[1].c).dot(list[1].n) > 0.3 && !box_hits(&w, p, BODY));
        assert!((v2.z - 4.3).abs() < 0.01, "{v2}");
        assert!(entering(&list, c, BODY, -v, 1.0 / 60.0).is_none());
    }

    #[test]
    fn falling_into_floor_flings_out_of_wall() {
        let w = World::generate();
        let mut list = pair(&w);
        list[0] = Portal::new(0, 0, ivec3(63, G - 1, 99), IVec3::Y, ivec3(0, 0, -1));
        assert!(list[0].valid(&w));
        let c = list[0].c + vec3(0.0, 0.92, 0.0);
        let v = vec3(0.0, -20.0, 0.0);
        let (ia, ib) = entering(&list, c, BODY, v, 1.0 / 60.0).expect("cai no chão");
        let (p, v2, _) = exit(&list, ia, ib, c, BODY, v, &w, 6.0).expect("sai na parede");
        assert!(v2.z > 19.9 && v2.y.abs() < 0.01, "{v2}");
        assert!(!box_hits(&w, p, BODY));
    }

    #[test]
    fn aim_snaps_to_flat_area() {
        let w = World::generate();
        let (cell, ni, ui) = aim(&w, vec3(63.5, G as f32 + 1.62, 101.0), vec3(0.0, -0.2, 1.0).normalize()).expect("parede da torre");
        assert_eq!((ni, ui), (ivec3(0, 0, -1), IVec3::Y));
        assert_eq!((cell.x, cell.z), (63, 104));
        assert!(aim(&w, vec3(63.5, G as f32 + 1.62, 101.0), vec3(0.0, 1.0, 0.0)).is_err());
    }

    #[test]
    fn oblique_keeps_far_side() {
        let proj = Mat4::perspective_rh_gl(1.2, 1.6, 0.05, 400.0);
        let m = oblique(proj, vec4(0.0, 0.0, -1.0, -5.0));
        let clip = |z: f32| {
            let h = m * vec4(0.0, 0.0, z, 1.0);
            h.z / h.w
        };
        assert!(clip(-4.0) < -1.0 && (-1.0..=1.0).contains(&clip(-10.0)));
    }
}
