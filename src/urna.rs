//! Urna eletrônica gigante: mira, carrega, dispara laser, explode blocos.
//! O escudo mágico do clube reflete os lasers.

use crate::batch::Batch;
use crate::models::rgb;
use crate::world::*;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use std::f32::consts::FRAC_PI_2;

pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub col: Color,
    pub life: f32,
    pub size: f32,
    pub gravity: bool,
}

pub struct Flash {
    pub pos: Vec3,
    pub t: f32,
    pub r: f32,
}

pub struct Beam {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub reflect: Option<(Vec3, Vec3)>,
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<Particle>,
    pub flashes: Vec<Flash>,
    pub beams: Vec<Beam>,
    pub shield_flash: f32,
    /// Impacto nos domos de energia: [lab, hub].
    pub dome_flash: [f32; 2],
    pub shake: f32,
}

pub enum Shot {
    Deflected(Vec3),
    Exploded(Vec3, f32),
}

pub struct Foot {
    pub pos: Vec3,
    from: Vec3,
    to: Vec3,
    t: f32,
}

/// Urna caricata de pernas e braços. `root` (chão sob o corpo) e mira vêm da IA do host;
/// pés, corpo e mãos são física procedural local (`animate`), igual em todos os clientes.
pub struct Urna {
    pub root: Vec3,
    pub pos: Vec3,
    vel: Vec3,
    pub yaw: f32,
    tilt: Vec2,
    pub charging: bool,
    pub timer: f32,
    pub charge: f32,
    pub target: Vec3,
    pub barrage: u32,
    pub shots: u32,
    goal: Vec3,
    feet: [Foot; 2],
    hands: [Vec3; 2],
    hand_vel: [Vec3; 2],
    last_root: Vec3,
    root_vel: Vec3,
    /// > 0 no frame em que um pé bate no chão (tremor/som no main).
    pub stomp: f32,
    /// Derrubada: cai de costas, para de andar e atirar.
    pub dead: bool,
}

/// Folga da urna (meia largura + braços) em volta de obstáculos.
const MARGIN: f32 = 6.5;

fn panel_box() -> (Vec2, Vec2) {
    let (lo, hi) = crate::eleicao::PAINEL;
    (vec2(lo.x, lo.z) - Vec2::splat(MARGIN), vec2(hi.x, hi.z) + Vec2::splat(MARGIN))
}

fn in_box(p: Vec2, (lo, hi): (Vec2, Vec2)) -> bool {
    p.x > lo.x && p.x < hi.x && p.y > lo.y && p.y < hi.y
}

/// Segmento a→b cruza a caixa (teste de slabs 2D)?
fn seg_hits(a: Vec2, b: Vec2, (lo, hi): (Vec2, Vec2)) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..2 {
        if d[k].abs() < 1e-6 {
            if a[k] <= lo[k] || a[k] >= hi[k] {
                return false;
            }
            continue;
        }
        let (mut ta, mut tb) = ((lo[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        t0 = t0.max(ta);
        t1 = t1.min(tb);
        if t0 >= t1 {
            return false;
        }
    }
    true
}

/// Próximo ponto pra contornar o placar: vai até a ponta mais perto e depois cruza pro lado do objetivo.
fn detour(root: Vec2, goal: Vec2) -> Vec2 {
    let bx = panel_box();
    if !seg_hits(root, goal, bx) {
        return goal;
    }
    let (lo, hi) = bx;
    let zc = (lo.y + hi.y) * 0.5;
    let side = |z: f32| if z > zc { hi.y + 0.5 } else { lo.y - 0.5 };
    if root.x <= lo.x || root.x >= hi.x {
        vec2(root.x, side(goal.y))
    } else {
        let ex = if root.x - lo.x < hi.x - root.x { lo.x - 0.5 } else { hi.x + 0.5 };
        vec2(ex, side(root.y))
    }
}

const EYE: Vec3 = Vec3::new(-2.2, 1.1, 2.2);
const HALF: Vec3 = Vec3::new(5.0, 3.5, 2.0);
const HIP: Vec3 = Vec3::new(2.4, -3.3, 0.0);
const SHOULDER: Vec3 = Vec3::new(5.5, 1.6, 0.0);
const THIGH: f32 = 4.6;
const SHIN: f32 = 4.6;
const UPPER: f32 = 4.3;
const FORE: f32 = 4.3;
const STAND: f32 = 8.4 + 3.3;

/// Ponto aleatório da arena pra urna ir quebrar, longe do escudo.
fn wander_point() -> Vec3 {
    loop {
        let a = crate::layout::arena_point(8.0);
        let p = vec3(a.x, G as f32, a.y);
        if p.distance(shield_center()) > SHIELD_R + 10.0 && p.distance(crate::lab::dome_center()) > crate::lab::DOME_R + 8.0 && !in_box(vec2(p.x, p.z), panel_box()) {
            return p;
        }
    }
}

/// IK de dois ossos: retorna (joelho/cotovelo, ponta alcançável).
pub fn ik(a: Vec3, t: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let d = t - a;
    let dist = d.length().clamp(0.05, l1 + l2 - 0.01);
    let dir = d.normalize_or(Vec3::NEG_Y);
    let x = (l1 * l1 - l2 * l2 + dist * dist) / (2.0 * dist);
    let h = (l1 * l1 - x * x).max(0.0).sqrt();
    let bend = (pole - dir * pole.dot(dir)).normalize_or(Vec3::Y);
    (a + dir * x + bend * h, a + dir * dist)
}

/// Caixa esticada entre dois pontos.
pub fn limb(b: &mut Batch, a: Vec3, c: Vec3, w: f32, col: Color) {
    let d = c - a;
    let len = d.length();
    if len < 1e-3 {
        return;
    }
    let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, d / len), (a + c) * 0.5);
    b.cube(&m, Vec3::ZERO, vec3(w, len, w), col);
}

impl Urna {
    pub fn new() -> Self {
        let root = crate::layout::URNA_SPAWN;
        let side = vec3(0.0, 0.0, 1.0);
        let foot = |s: f32| Foot { pos: root + side * s * HIP.x, from: root, to: root, t: 1.0 };
        Urna {
            root,
            pos: root + vec3(0.0, STAND, 0.0),
            vel: Vec3::ZERO,
            yaw: -FRAC_PI_2,
            tilt: Vec2::ZERO,
            charging: false,
            timer: 5.0,
            charge: 0.0,
            target: arena_center(),
            barrage: 0,
            shots: 0,
            goal: root,
            feet: [foot(-1.0), foot(1.0)],
            hands: [root; 2],
            hand_vel: [Vec3::ZERO; 2],
            last_root: root,
            root_vel: Vec3::ZERO,
            stomp: 0.0,
            dead: false,
        }
    }

    /// Corpo da urna (com o root em `root`) atravessando bloco sólido?
    fn body_blocked(&self, world: &World, root: Vec3) -> bool {
        let base = world.floor_at(root.x, G as f32 + 14.0, root.z) + STAND;
        let rot = Mat4::from_rotation_y(self.yaw);
        for x in [-HALF.x, 0.0, HALF.x] {
            for y in [-HALF.y + 0.5, 0.0, HALF.y] {
                for z in [-HALF.z, HALF.z] {
                    let p = root + rot.transform_vector3(vec3(x, 0.0, z));
                    if world.solid_f(p.x, base + y, p.z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_translation(self.pos) * Mat4::from_rotation_y(self.yaw) * Mat4::from_rotation_x(self.tilt.x) * Mat4::from_rotation_z(self.tilt.y)
    }

    pub fn eye(&self) -> Vec3 {
        self.matrix().transform_point3(EYE)
    }

    fn side(&self) -> Vec3 {
        vec3(self.yaw.cos(), 0.0, -self.yaw.sin())
    }

    fn fwd(&self) -> Vec3 {
        vec3(self.yaw.sin(), 0.0, self.yaw.cos())
    }

    /// Coice do disparo (todos os clientes, ao aplicar o tiro).
    pub fn recoil(&mut self) {
        self.vel += -self.fwd() * 7.0 + vec3(0.0, 3.0, 0.0);
    }

    /// IA do host: anda pelo mapa inteiro destruindo tudo e mira. Retorna Some(alvo) quando dispara.
    pub fn update(&mut self, world: &World, dt: f32, mut pick: impl FnMut() -> Vec3) -> Option<Vec3> {
        if self.dead {
            self.charging = false;
            self.charge = 0.0;
            self.barrage = 0;
            self.timer = 3.0;
            return None;
        }
        let r2 = vec2(self.root.x, self.root.z);
        if in_box(r2, panel_box()) {
            let (lo, hi) = panel_box();
            self.root.z = if r2.y > (lo.y + hi.y) * 0.5 { hi.y + 0.1 } else { lo.y - 0.1 };
        }
        if (self.goal - self.root).length() < 1.0 {
            self.goal = wander_point();
        }
        let way = detour(vec2(self.root.x, self.root.z), vec2(self.goal.x, self.goal.z));
        let to_goal = vec3(way.x, self.root.y, way.y) - self.root;
        let speed = if self.charging { 0.6 } else { 2.4 };
        let next = self.root + to_goal.normalize_or_zero() * (speed * dt).min(to_goal.length());
        if self.body_blocked(world, next) && !self.body_blocked(world, self.root) {
            self.goal = wander_point();
        } else {
            self.root = next;
        }
        let look = if self.charging || self.charge > 0.2 { self.target } else { vec3(way.x, self.root.y, way.y) };
        let to = look - self.root;
        self.yaw = crate::actors::angle_lerp(self.yaw, to.x.atan2(to.z), dt * 2.5);
        self.timer -= dt;
        if !self.charging {
            self.charge = (self.charge - dt * 3.0).max(0.0);
            if self.timer <= 0.0 {
                self.target = pick();
                self.charging = true;
                self.timer = if self.barrage > 0 { 0.3 } else { 0.9 };
            }
            return None;
        }
        self.charge = (self.charge + dt * 2.0).min(1.0);
        if self.timer > 0.0 {
            return None;
        }
        self.shots += 1;
        if self.barrage > 0 {
            self.barrage -= 1;
            self.target = pick();
            self.timer = 0.3;
        } else {
            self.charging = false;
            self.timer = gen_range(1.2, 2.8);
            if gen_range(0.0, 1.0) < 0.18 {
                self.barrage = 4;
            }
        }
        Some(self.target)
    }

    /// Física procedural: passos com pé plantado no terreno, corpo em mola, braços socando/mirando.
    pub fn animate(&mut self, world: &World, dt: f32, time: f32) {
        let dt = dt.max(1e-4);
        let rv = (self.root - self.last_root) / dt;
        self.last_root = self.root;
        self.root_vel = self.root_vel.lerp(if rv.length() < 20.0 { rv } else { Vec3::ZERO }, (dt * 5.0).min(1.0));
        let (side, fwd, up) = (self.side(), self.fwd(), Vec3::Y);
        self.stomp = 0.0;

        if self.dead {
            // Tomba de costas: corpo deitado no chão, pernas e braços largados
            let ground = world.floor_at(self.root.x, G as f32 + 14.0, self.root.z);
            let target = vec3(self.root.x, ground + HALF.z + 0.3, self.root.z) - fwd * 4.0;
            let acc = (target - self.pos) * 12.0 - self.vel * 5.0;
            self.vel += acc * dt;
            self.pos += self.vel * dt;
            if self.pos.y < target.y {
                self.pos.y = target.y;
                self.vel.y = self.vel.y.max(0.0);
            }
            self.tilt = self.tilt.lerp(vec2(-FRAC_PI_2, 0.15), (dt * 2.5).min(1.0));
            let m = self.matrix();
            for i in 0..2 {
                let s = if i == 0 { -1.0 } else { 1.0 };
                let sh = m.transform_point3(vec3(SHOULDER.x * s, SHOULDER.y, 0.0));
                let mut goal = sh + side * s * 4.0 + fwd * 3.0;
                goal.y = ground + 0.9;
                self.hand_vel[i] = (goal - self.hands[i]) * 4.0;
                self.hands[i] += self.hand_vel[i] * dt;
            }
            return;
        }

        let mut lift = 0.0;
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let mut want = self.root + side * s * HIP.x + self.root_vel * 0.45;
            want.y = world.floor_at(want.x, G as f32 + 14.0, want.z);
            let other_planted = self.feet[1 - i].t >= 1.0;
            let f = &mut self.feet[i];
            if f.t >= 1.0 {
                let off = vec2(f.pos.x - want.x, f.pos.z - want.z).length();
                if other_planted && (off > 1.7 || (f.pos.y - want.y).abs() > 1.5) {
                    f.from = f.pos;
                    f.to = want + self.root_vel * 0.25;
                    f.to.y = world.floor_at(f.to.x, G as f32 + 14.0, f.to.z);
                    f.t = 0.0;
                }
            } else {
                f.t = (f.t + dt / 0.5).min(1.0);
                let e = f.t * f.t * (3.0 - 2.0 * f.t);
                f.pos = f.from.lerp(f.to, e) + up * (f.t * std::f32::consts::PI).sin() * 1.5;
                lift = (f.t * std::f32::consts::PI).sin();
                if f.t >= 1.0 {
                    f.pos = f.to;
                    self.stomp = 1.0;
                }
            }
        }

        // Corpo: mola amortecida (peso) acima dos pés
        let ground = (self.feet[0].pos.y + self.feet[1].pos.y) * 0.5;
        let target = vec3(self.root.x, ground + STAND + lift * 0.5, self.root.z);
        let acc = (target - self.pos) * 45.0 - self.vel * 8.0;
        self.vel += acc * dt;
        self.pos += self.vel * dt;
        let off = self.pos - target;
        let sway = (time * 3.2).sin() * 0.03 * self.root_vel.length().min(2.0);
        self.tilt = self.tilt.lerp(vec2(off.dot(fwd) * -0.06 + self.vel.dot(fwd) * 0.02, off.dot(side) * 0.06 + sway), (dt * 8.0).min(1.0));

        // Mãos: carregando = apontando pro alvo; senão socando o ar alternado
        let m = self.matrix();
        let reach = UPPER + FORE - 0.3;
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let sh = m.transform_point3(vec3(SHOULDER.x * s, SHOULDER.y, 0.0));
            let mut goal = if self.charging {
                sh + (self.target - sh).normalize_or(fwd) * reach + up * (time * 30.0 + i as f32).sin() * 0.2
            } else {
                let punch = (time * 4.0 + i as f32 * std::f32::consts::PI).sin().max(0.0);
                m.transform_point3(vec3(s * 3.5, 1.5 + punch, 2.5 + punch * 5.0))
            };
            if goal.distance(sh) > reach {
                goal = sh + (goal - sh).normalize() * reach;
            }
            let a = (goal - self.hands[i]) * 30.0 - self.hand_vel[i] * 6.0;
            self.hand_vel[i] += a * dt;
            self.hands[i] += self.hand_vel[i] * dt;
        }
    }

    pub fn draw(&self, b: &mut Batch, trans: &mut Batch, time: f32) {
        let m = self.matrix();
        let beige = rgb(0.9, 0.87, 0.78);
        let panel = rgb(0.8, 0.77, 0.69);
        let dark = rgb(0.12, 0.12, 0.13);
        let (side, fwd, up) = (self.side(), self.fwd(), Vec3::Y);

        // Corpo
        b.cube(&m, Vec3::ZERO, HALF * 2.0, beige);
        b.cube(&m, vec3(0.0, -0.3, HALF.z), vec3(9.6, 6.2, 0.15), panel);
        // Faixa "JUSTIÇA ELEITORAL" com brasão
        b.cube(&m, vec3(0.4, 3.05, HALF.z + 0.05), vec3(8.4, 0.55, 0.1), rgb(0.25, 0.27, 0.3));
        b.cube(&m, vec3(-4.3, 3.05, HALF.z + 0.08), vec3(0.6, 0.6, 0.05), rgb(0.1, 0.55, 0.2));
        b.glow(&m, vec3(-4.3, 3.05, HALF.z + 0.12), vec3(0.3, 0.3, 0.04), rgb(1.0, 0.85, 0.1));

        // Tela = rosto
        b.cube(&m, vec3(-2.2, 0.7, HALF.z + 0.06), vec3(4.6, 3.8, 0.12), dark);
        let pulse = 0.85 + 0.15 * (time * 8.0).sin();
        let fz = HALF.z + 0.17;
        if self.dead {
            let flicker = if (time * 13.0).sin() > 0.7 { 0.35 } else { 0.18 };
            b.glow(&m, vec3(-2.2, 0.7, HALF.z + 0.13), vec3(4.1, 3.3, 0.04), rgb(flicker, flicker, flicker * 1.2));
            for &ex in &[-3.2f32, -1.2] {
                for r in [0.8f32, -0.8] {
                    let x = Mat4::from_translation(vec3(ex, 1.2, fz)) * Mat4::from_rotation_z(r);
                    b.glow(&(m * x), Vec3::ZERO, vec3(1.4, 0.3, 0.04), rgb(0.9, 0.1, 0.1));
                }
            }
            b.glow(&m, vec3(-2.2, -0.45, fz), vec3(2.4, 0.25, 0.04), dark);
            trans.glow(&m, vec3((time * 1.7).sin() * 2.0, 4.5 + (time * 2.0).fract() * 4.0, 0.0), Vec3::splat(2.0 + (time * 2.0).fract() * 2.0), Color::new(0.2, 0.2, 0.2, 0.5 * (1.0 - (time * 2.0).fract())));
        } else {
            let screen = if self.charging { Color::new(1.0, 0.3 + 0.2 * pulse, 0.3, 1.0) } else { rgb(0.8 * pulse, 0.92 * pulse, 1.0) };
            b.glow(&m, vec3(-2.2, 0.7, HALF.z + 0.13), vec3(4.1, 3.3, 0.04), screen);
            let look = m.inverse().transform_vector3(self.target - self.eye()).normalize_or_zero();
            for &ex in &[-3.2f32, -1.2] {
                b.glow(&m, vec3(ex, 1.2, fz), vec3(1.15, 1.35, 0.04), WHITE);
                b.glow(&m, vec3(ex + look.x * 0.28, 1.15 + look.y * 0.3, fz + 0.02), vec3(0.5, 0.6, 0.04), dark);
                let brow = Mat4::from_translation(vec3(ex, 2.1, fz)) * Mat4::from_rotation_z(if ex < -2.0 { -0.35 } else { 0.35 } * if self.charging { 1.4 } else { 0.6 });
                b.glow(&(m * brow), Vec3::ZERO, vec3(1.3, 0.25, 0.04), dark);
            }
        }
        if self.charging {
            b.glow(&m, vec3(-2.2, -0.35, fz), vec3(1.8, 0.9 * (0.6 + 0.4 * pulse), 0.04), rgb(0.6, 0.05, 0.05));
        } else if !self.dead {
            b.glow(&m, vec3(-2.2, -0.35, fz), vec3(2.2, 0.28, 0.04), dark);
            b.glow(&m, vec3(-3.35, -0.2, fz), vec3(0.3, 0.3, 0.04), dark);
            b.glow(&m, vec3(-1.05, -0.2, fz), vec3(0.3, 0.3, 0.04), dark);
        }

        // Teclado numérico + BRANCO / CORRIGE / CONFIRMA
        b.cube(&m, vec3(2.6, -0.3, HALF.z + 0.06), vec3(3.8, 5.6, 0.12), rgb(0.2, 0.2, 0.22));
        for row in 0..4 {
            for col in 0..3 {
                if row == 3 && col != 1 {
                    continue;
                }
                let (x, y) = (1.6 + col as f32 * 1.0, 1.7 - row as f32 * 0.85);
                b.cube(&m, vec3(x, y, HALF.z + 0.2), vec3(0.8, 0.6, 0.2), dark);
                b.glow(&m, vec3(x, y, HALF.z + 0.31), vec3(0.2, 0.3, 0.02), WHITE);
            }
        }
        let press = if self.charging { 0.1 } else { 0.0 };
        b.cube(&m, vec3(1.5, -2.2, HALF.z + 0.2), vec3(0.95, 0.65, 0.2), rgb(0.97, 0.97, 0.97));
        b.cube(&m, vec3(2.55, -2.2, HALF.z + 0.2), vec3(0.95, 0.65, 0.2), rgb(0.95, 0.45, 0.1));
        b.cube(&m, vec3(3.75, -2.2, HALF.z + 0.2 - press), vec3(1.25, 0.8, 0.2), rgb(0.15, 0.7, 0.25));

        // Pernas (IK) e tênis
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let hip = m.transform_point3(vec3(HIP.x * s, HIP.y, 0.0));
            let ankle = self.feet[i].pos + up * 0.7;
            let (knee, end) = ik(hip, ankle, THIGH, SHIN, fwd + up * 0.2);
            limb(b, hip, knee, 1.3, rgb(0.25, 0.25, 0.28));
            limb(b, knee, end, 1.1, rgb(0.25, 0.25, 0.28));
            b.cube(&id_at(knee), Vec3::ZERO, Vec3::splat(1.4), rgb(0.2, 0.2, 0.22));
            let shoe = Mat4::from_translation(end - up * 0.35) * Mat4::from_rotation_y(self.yaw);
            b.cube(&shoe, vec3(0.0, 0.0, 0.5), vec3(1.8, 1.0, 3.0), rgb(0.95, 0.95, 0.95));
            b.cube(&shoe, vec3(0.0, -0.4, 0.5), vec3(1.9, 0.25, 3.1), rgb(0.85, 0.15, 0.15));
        }

        // Braços (IK) com luvas de boxe; carregando, as luvas ficam em brasa
        for i in 0..2 {
            let s = if i == 0 { -1.0 } else { 1.0 };
            let sh = m.transform_point3(vec3(SHOULDER.x * s, SHOULDER.y, 0.0));
            let (elbow, hand) = ik(sh, self.hands[i], UPPER, FORE, -up + side * s * 0.8 - fwd * 0.3);
            b.cube(&id_at(sh), Vec3::ZERO, Vec3::splat(1.5), beige);
            limb(b, sh, elbow, 1.0, beige);
            limb(b, elbow, hand, 0.9, beige);
            b.cube(&id_at(hand), Vec3::ZERO, Vec3::splat(1.9), rgb(0.85, 0.1, 0.1));
            if self.charge > 0.0 {
                let k = self.charge * (0.7 + 0.3 * (time * 20.0 + i as f32).sin());
                trans.glow(&id_at(hand), Vec3::ZERO, Vec3::splat(2.6 + k), Color::new(1.0, 0.3, 0.1, 0.4 * k));
            }
        }

        // Olhos do laser carregando
        if self.charge > 0.0 {
            let s = 0.3 + self.charge * 1.2 + (time * 40.0).sin() * 0.1;
            b.glow(&m, EYE + vec3(0.0, 0.0, 0.2), vec3(s, s, s), Color::new(1.0, 0.2, 0.2, 1.0));
        }
    }
}

fn id_at(p: Vec3) -> Mat4 {
    Mat4::from_translation(p)
}

pub fn beam(b: &mut Batch, a: Vec3, c: Vec3, w: f32, col: Color) {
    let d = c - a;
    let len = d.length();
    if len < 1e-3 {
        return;
    }
    let m = Mat4::from_rotation_translation(Quat::from_rotation_arc(Vec3::Y, d / len), (a + c) * 0.5);
    b.glow(&m, Vec3::ZERO, vec3(w, len, w), col);
}

pub fn ray_sphere(o: Vec3, d: Vec3, c: Vec3, r: f32) -> Option<f32> {
    let oc = o - c;
    let b = oc.dot(d);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let t = -b - disc.sqrt();
    if t > 0.0 { Some(t) } else { None }
}

/// Tiro decidido pelo host; `apply` é determinístico e roda igual em todos os clientes.
pub struct Plan {
    pub o: Vec3,
    pub hit: Vec3,
    pub r: f32,
    pub deflect: bool,
}

pub fn plan(world: &World, o: Vec3, target: Vec3, big_chance: f32) -> Plan {
    let d = (target - o).normalize_or_zero();
    let max = (target - o).length() + 6.0;
    let wt = world.raycast(o, d, max).map(|h| h.2);
    let r = if gen_range(0.0, 1.0) < big_chance { 5.5 } else { gen_range(2.5, 4.0) };
    if let Some(s) = ray_sphere(o, d, shield_center(), SHIELD_R) {
        if wt.is_none_or(|w| s < w) {
            return Plan { o, hit: o + d * s, r, deflect: true };
        }
    }
    Plan { o, hit: wt.map(|w| o + d * w).unwrap_or(target), r, deflect: false }
}

pub fn apply(world: &mut World, plan: &Plan, fx: &mut Fx, avg: &[Color]) -> Shot {
    let (o, sc) = (plan.o, shield_center());
    let d = (plan.hit - o).normalize_or_zero();
    let dome = if plan.deflect { None } else { crate::shield::ray(o, d).filter(|h| h.0 < o.distance(plan.hit)) };
    {
        if plan.deflect || dome.is_some() {
            let (p, n) = match dome {
                Some((t, n, i)) => {
                    fx.dome_flash[i] = 1.0;
                    (o + d * t, n)
                }
                None => {
                    fx.shield_flash = 1.0;
                    (plan.hit, (plan.hit - sc).normalize())
                }
            };
            let refl = d - 2.0 * d.dot(n) * n;
            fx.beams.push(Beam { from: o, to: p, t: 0.35, reflect: Some((p, p + refl * 70.0)) });
            for _ in 0..30 {
                fx.particles.push(Particle {
                    pos: p,
                    vel: (n * gen_range(2.0, 8.0) + vec3(gen_range(-4.0, 4.0), gen_range(-4.0, 4.0), gen_range(-4.0, 4.0))),
                    col: Color::new(0.6, 0.9, 1.0, 1.0),
                    life: gen_range(0.3, 0.8),
                    size: gen_range(0.08, 0.2),
                    gravity: false,
                });
            }
            return Shot::Deflected(p);
        }
    }
    let (impact, r) = (plan.hit, plan.r);
    fx.beams.push(Beam { from: o, to: impact, t: 0.3, reflect: None });
    explode(world, impact, r, fx, avg);
    Shot::Exploded(impact, r)
}

pub fn explode(world: &mut World, c: Vec3, r: f32, fx: &mut Fx, avg: &[Color]) {
    let ri = r.ceil() as i32 + 1;
    let ci = c.floor().as_ivec3();
    let sc = shield_center();
    for dy in -ri..=ri {
        for dz in -ri..=ri {
            for dx in -ri..=ri {
                let p = ci + ivec3(dx, dy, dz);
                let bc = p.as_vec3() + Vec3::splat(0.5);
                let jitter = crate::atlas::hash2(p.x * 7 + p.y, p.z, 4242) * 0.8;
                if bc.distance(c) > r + jitter - 0.4 || p.y <= 0 || bc.distance(sc) < SHIELD_R + 0.5 || crate::shield::protected(p) {
                    continue;
                }
                let b = world.get(p.x, p.y, p.z);
                if b == AIR {
                    continue;
                }
                world.set(p.x, p.y, p.z, AIR);
                if gen_range(0.0, 1.0) < 0.3 {
                    fx.particles.push(Particle {
                        pos: bc,
                        vel: (bc - c).normalize_or_zero() * gen_range(4.0, 12.0) + vec3(0.0, gen_range(4.0, 10.0), 0.0),
                        col: avg[face_tile(b, 0)],
                        life: gen_range(1.0, 2.2),
                        size: gen_range(0.15, 0.35),
                        gravity: true,
                    });
                }
            }
        }
    }
    for _ in 0..25 {
        let f = gen_range(0.0, 1.0);
        fx.particles.push(Particle {
            pos: c,
            vel: vec3(gen_range(-1.0, 1.0), gen_range(0.0, 1.2), gen_range(-1.0, 1.0)) * gen_range(3.0, 9.0),
            col: Color::new(1.0, 0.4 + 0.5 * f, 0.1 * f, 1.0),
            life: gen_range(0.3, 0.9),
            size: gen_range(0.2, 0.5),
            gravity: false,
        });
    }
    fx.flashes.push(Flash { pos: c, t: 0.0, r });
}

impl Fx {
    pub fn update(&mut self, world: &World, dt: f32) {
        for p in self.particles.iter_mut() {
            if p.gravity {
                p.vel.y -= 20.0 * dt;
            } else {
                p.vel *= (1.0 - 2.5 * dt).max(0.0);
            }
            let np = p.pos + p.vel * dt;
            if p.gravity && world.solid_f(np.x, np.y, np.z) {
                p.vel *= -0.3;
            } else {
                p.pos = np;
            }
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
        for f in self.flashes.iter_mut() {
            f.t += dt;
        }
        self.flashes.retain(|f| f.t < 0.5);
        for b in self.beams.iter_mut() {
            b.t -= dt;
        }
        self.beams.retain(|b| b.t > 0.0);
        self.shield_flash = (self.shield_flash - dt * 2.0).max(0.0);
        for f in &mut self.dome_flash {
            *f = (*f - dt * 2.0).max(0.0);
        }
        self.shake = (self.shake - dt * 1.5).max(0.0);
    }

    pub fn draw_opaque(&self, b: &mut Batch) {
        for p in &self.particles {
            let s = p.size * (p.life / 0.4).min(1.0);
            if p.gravity {
                b.cube(&Mat4::IDENTITY, p.pos, Vec3::splat(s), p.col);
            } else {
                b.glow(&Mat4::IDENTITY, p.pos, Vec3::splat(s), p.col);
            }
        }
    }

    pub fn draw_transparent(&self, b: &mut Batch, time: f32) {
        let seg = |b: &mut Batch, a: Vec3, c: Vec3, w: f32, col: Color| {
            let d = c - a;
            let len = d.length();
            if len < 1e-3 {
                return;
            }
            let q = Quat::from_rotation_arc(Vec3::Y, d / len);
            let m = Mat4::from_rotation_translation(q, (a + c) * 0.5);
            b.glow(&m, Vec3::ZERO, vec3(w, len, w), col);
        };
        for beam in &self.beams {
            let k = (beam.t / 0.3).min(1.0);
            let wob = 1.0 + 0.2 * (time * 60.0).sin();
            seg(b, beam.from, beam.to, 1.1 * k * wob, Color::new(1.0, 0.1, 0.15, 0.35 * k));
            seg(b, beam.from, beam.to, 0.35 * k, Color::new(1.0, 0.85, 0.9, 0.95 * k));
            if let Some((a, c)) = beam.reflect {
                seg(b, a, c, 0.8 * k, Color::new(0.4, 0.8, 1.0, 0.4 * k));
                seg(b, a, c, 0.25 * k, Color::new(0.9, 1.0, 1.0, 0.9 * k));
            }
        }
    }

    pub fn draw_flashes(&self) {
        for f in &self.flashes {
            let k = 1.0 - f.t / 0.5;
            draw_sphere(f.pos, f.r * (0.6 + f.t * 2.5), None, Color::new(1.0, 0.6 * k + 0.2, 0.1, 0.6 * k));
        }
    }
}
