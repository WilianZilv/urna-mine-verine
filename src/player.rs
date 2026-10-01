//! Jogador primeira pessoa: física AABB contra voxels, voo, knockback.

use crate::world::*;
use macroquad::prelude::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// Câmera inicial voando pra screenshot/debug: ?cam=x,y,z,yaw,pitch (nativo: URNA_CAM).
fn cam() -> Option<Vec<f32>> {
    #[cfg(target_arch = "wasm32")]
    let s = crate::web::query("cam");
    #[cfg(not(target_arch = "wasm32"))]
    let s = std::env::var("URNA_CAM").ok();
    let v: Vec<f32> = s?.split(',').filter_map(|t| t.parse().ok()).collect();
    (v.len() == 5).then_some(v)
}

pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub knock: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fly: bool,
    pub on_ground: bool,
    pub sel: usize,
    /// Joystick virtual (x = direita, y = frente) e botão de pulo do celular.
    pub stick: Vec2,
    pub jump_held: bool,
    pub can_fly: bool,
    /// Física e controles do Minecraft Java (Steve); os outros personagens usam o movimento antigo.
    pub mc: bool,
    /// Botão AGACHA do celular.
    pub sneak_held: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    hit_wall: bool,
    last_w: f64,
    last_space: f64,
    /// 0..1 suavizado: câmera desce agachado.
    crouch: f32,
    /// Multiplicador do FOV (correndo abre), suavizado.
    fov: f32,
    /// View bobbing do Minecraft: distância andada (x0,6) e amplitude (<= 0,1).
    walk_dist: f32,
    bob: f32,
}

// Minecraft Java em blocos/s (20 ticks/s). Atrito por tick vira decaimento contínuo: chão 0,6*0,91,
// ar 0,91, voo vertical 0,6; gravidade 0,08/tick com arrasto 0,98 (terminal 78,4 b/s).
const WALK: f32 = 4.317;
const SPRINT: f32 = 5.612;
const SNEAK: f32 = 1.31;
const FLY: f32 = 10.9;
const FLY_SPRINT: f32 = 21.6;
const FLY_VERT: f32 = 7.5;
const GROUND_RATE: f32 = 12.10;
const AIR_RATE: f32 = 1.886;
const FLY_VERT_RATE: f32 = 10.22;
const DRAG: f32 = 0.404;
const TERMINAL: f32 = -78.4;
/// Sobe ~1,25 bloco (pulo de 0,42 b/tick do Minecraft com esta gravidade contínua).
const JUMP: f32 = 9.25;
/// Empurrão pra frente no pulo correndo (MC: +0,2 b/tick).
const SPRINT_JUMP: f32 = 2.2;
/// Dois toques (W corre, espaço voa): 7 ticks.
const DOUBLE_TAP: f64 = 0.35;

/// Entrada de um frame do Steve (teclado/celular já juntos).
#[derive(Default)]
struct McInput {
    /// x = direita, y = frente.
    mv: Vec2,
    sprint: bool,
    sneak: bool,
    jump: bool,
    w_tap: bool,
    space_tap: bool,
    fly_tap: bool,
    now: f64,
}

/// Velocidade e deslocamento exatos de dv/dt = rate * (alvo - v) num passo dt.
fn approach(v: f32, target: f32, rate: f32, dt: f32) -> (f32, f32) {
    let k = (-rate * dt).exp();
    (target + (v - target) * k, target * dt + (v - target) * (1.0 - k) / rate)
}

impl Player {
    pub fn new() -> Self {
        let mut p = Player {
            pos: Self::spawn(),
            vel: Vec3::ZERO,
            knock: Vec3::ZERO,
            yaw: -FRAC_PI_2,
            pitch: -0.3,
            fly: false,
            on_ground: false,
            sel: 0,
            stick: Vec2::ZERO,
            jump_held: false,
            can_fly: true,
            mc: false,
            sneak_held: false,
            sprinting: false,
            sneaking: false,
            hit_wall: false,
            last_w: -1.0,
            last_space: -1.0,
            crouch: 0.0,
            fov: 1.0,
            walk_dist: 0.0,
            bob: 0.0,
        };
        if let Some(c) = cam() {
            (p.pos, p.yaw, p.pitch, p.fly) = (vec3(c[0], c[1], c[2]), c[3], c[4], true);
        }
        p
    }

    pub fn spawn() -> Vec3 {
        crate::layout::SPAWN
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + vec3(0.0, 1.62 - 0.3 * self.crouch, 0.0)
    }

    /// Altura de queda que dá a velocidade vertical `vy` (gravidade com arrasto do Minecraft).
    pub fn fall_height(vy: f32) -> f32 {
        let v = (-vy).clamp(0.0, -TERMINAL * 0.999);
        -TERMINAL / DRAG * -(1.0 + v / TERMINAL).ln() - v / DRAG
    }

    pub fn fov_mul(&self) -> f32 {
        self.fov
    }

    /// Transformação do view bobbing no espaço da câmera (GameRenderer.bobView do Minecraft).
    pub fn bob_matrix(&self) -> Mat4 {
        let (g, h) = (-self.walk_dist * PI, self.bob);
        Mat4::from_translation(vec3(g.sin() * h * 0.5, -(g.cos() * h).abs(), 0.0))
            * Mat4::from_rotation_z((g.sin() * h * 3.0).to_radians())
            * Mat4::from_rotation_x(((g - 0.2).cos() * h).abs() * 5.0f32.to_radians())
    }

    /// Câmera (olho, frente, cima) com o bobbing aplicado.
    pub fn bob_view(&self, eye: Vec3, fw: Vec3) -> (Vec3, Vec3, Vec3) {
        if self.bob <= 0.0 {
            return (eye, fw, Vec3::Y);
        }
        let cam = Mat4::look_at_rh(eye, eye + fw, Vec3::Y).inverse() * self.bob_matrix().inverse();
        (cam.transform_point3(Vec3::ZERO), cam.transform_vector3(Vec3::NEG_Z), cam.transform_vector3(Vec3::Y))
    }

    pub fn forward(&self) -> Vec3 {
        vec3(self.yaw.cos() * self.pitch.cos(), self.pitch.sin(), self.yaw.sin() * self.pitch.cos())
    }

    pub fn look(&mut self, md: Vec2) {
        self.yaw += md.x * 0.0025;
        self.pitch = (self.pitch - md.y * 0.0025).clamp(-1.55, 1.55);
    }

    pub fn collides_at(&self, w: &World, p: Vec3) -> bool {
        let (x0, x1) = ((p.x - 0.3).floor() as i32, (p.x + 0.3).floor() as i32);
        let (y0, y1) = (p.y.floor() as i32, (p.y + 1.79).floor() as i32);
        let (z0, z1) = ((p.z - 0.3).floor() as i32, (p.z + 0.3).floor() as i32);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if w.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn update(&mut self, w: &World, dt: f32, input: bool) {
        if self.mc {
            self.update_mc(w, dt, input);
            return;
        }
        (self.sprinting, self.sneaking, self.crouch, self.fov, self.bob) = (false, false, 0.0, 1.0, 0.0);
        let f = vec3(self.yaw.cos(), 0.0, self.yaw.sin());
        let r = vec3(-f.z, 0.0, f.x);
        let mut wish = Vec3::ZERO;
        if input {
            if is_key_down(KeyCode::W) {
                wish += f;
            }
            if is_key_down(KeyCode::S) {
                wish -= f;
            }
            if is_key_down(KeyCode::D) {
                wish += r;
            }
            if is_key_down(KeyCode::A) {
                wish -= r;
            }
            if is_key_pressed(KeyCode::F) && self.can_fly {
                self.fly = !self.fly;
                self.vel.y = 0.0;
            }
        }
        let stick = if input { self.stick } else { Vec2::ZERO };
        let wish = if stick.length() > 0.1 { f * stick.y + r * stick.x } else { wish.normalize_or_zero() };
        let jump = input && (is_key_down(KeyCode::Space) || self.jump_held);
        let sprint = input && (is_key_down(KeyCode::LeftShift) || stick.length() > 0.95);
        let speed = if self.fly {
            if sprint { 25.0 } else { 12.0 }
        } else if sprint {
            6.5
        } else {
            4.3
        };
        self.vel.x = wish.x * speed + self.knock.x;
        self.vel.z = wish.z * speed + self.knock.z;
        if self.fly {
            let up = jump;
            let down = input && is_key_down(KeyCode::LeftControl);
            self.vel.y = (up as i32 - down as i32) as f32 * speed + self.knock.y;
        } else {
            self.vel.y -= 28.0 * dt;
            if self.on_ground && jump {
                self.vel.y = 8.4;
            }
            self.vel.y += self.knock.y;
        }
        self.knock.y = 0.0;
        self.knock *= (1.0 - 3.0 * dt).max(0.0);

        self.move_axis(w, 0, self.vel.x * dt);
        self.move_axis(w, 1, self.vel.y * dt);
        self.move_axis(w, 2, self.vel.z * dt);

        if self.pos.y < -20.0 {
            self.pos = Self::spawn();
            self.vel = Vec3::ZERO;
        }
    }

    /// Steve do Minecraft: WASD/joystick, sprint (2x W, Ctrl ou joystick no talo), Shift agacha (não cai da
    /// beirada), pulo de 1,25 bloco, voo do criativo (2x espaço; espaço sobe, Shift desce), sem auto-degrau.
    fn update_mc(&mut self, w: &World, dt: f32, input: bool) {
        let key = |k: KeyCode| input && is_key_down(k);
        let tap = |k: KeyCode| input && is_key_pressed(k);
        let stick = if input { self.stick } else { Vec2::ZERO };
        let mv = if stick.length() > 0.1 {
            stick
        } else {
            vec2((key(KeyCode::D) as i32 - key(KeyCode::A) as i32) as f32, (key(KeyCode::W) as i32 - key(KeyCode::S) as i32) as f32)
        };
        let inp = McInput {
            mv,
            sprint: key(KeyCode::LeftControl) || stick.length() > 0.95,
            sneak: key(KeyCode::LeftShift) || key(KeyCode::RightShift) || (input && self.sneak_held),
            jump: key(KeyCode::Space) || (input && self.jump_held),
            w_tap: tap(KeyCode::W),
            space_tap: tap(KeyCode::Space),
            fly_tap: tap(KeyCode::F),
            now: get_time(),
        };
        self.step_mc(w, dt, &inp);
    }

    fn step_mc(&mut self, w: &World, dt: f32, inp: &McInput) {
        let now = inp.now;
        let f = vec3(self.yaw.cos(), 0.0, self.yaw.sin());
        let r = vec3(-f.z, 0.0, f.x);
        let mv = inp.mv.clamp_length_max(1.0);
        let forward = mv.y > 0.8;

        if inp.w_tap {
            if now - self.last_w < DOUBLE_TAP {
                self.sprinting = true;
            }
            self.last_w = now;
        }
        self.sneaking = inp.sneak;
        if forward && inp.sprint {
            self.sprinting = true;
        }
        if !forward || self.hit_wall || (self.sneaking && !self.fly) {
            self.sprinting = false;
        }
        if self.can_fly {
            let mut toggle = inp.fly_tap;
            if inp.space_tap {
                toggle |= now - self.last_space < DOUBLE_TAP;
                self.last_space = if toggle { -1.0 } else { now };
            }
            if toggle {
                self.fly = !self.fly;
                self.vel.y = 0.0;
                self.on_ground = false;
            }
        }

        let jump = inp.jump;
        let mut h = vec2(self.vel.x, self.vel.z);
        let wish = vec2(f.x, f.z) * mv.y + vec2(r.x, r.z) * mv.x;
        let (speed, rate) = match (self.fly, self.sprinting) {
            (true, s) => (if s { FLY_SPRINT } else { FLY }, AIR_RATE),
            _ if self.sneaking => (SNEAK, if self.on_ground { GROUND_RATE } else { AIR_RATE }),
            (_, s) => (if s { SPRINT } else { WALK }, if self.on_ground { GROUND_RATE } else { AIR_RATE }),
        };
        self.vel.y += self.knock.y;
        let dy = if self.fly {
            let (vy, dy) = approach(self.vel.y, (jump as i32 - self.sneaking as i32) as f32 * FLY_VERT, FLY_VERT_RATE, dt);
            self.vel.y = vy;
            dy
        } else {
            if self.on_ground && jump {
                self.vel.y = JUMP;
                if self.sprinting {
                    h += vec2(f.x, f.z) * SPRINT_JUMP;
                }
            }
            let (vy, dy) = approach(self.vel.y, TERMINAL, DRAG, dt);
            self.vel.y = vy;
            dy
        };
        let (vx, dx) = approach(h.x, wish.x * speed, rate, dt);
        let (vz, dz) = approach(h.y, wish.y * speed, rate, dt);
        (self.vel.x, self.vel.z) = (vx, vz);
        let (kx, kz) = (self.knock.x * dt, self.knock.z * dt);
        self.knock.y = 0.0;
        self.knock *= (1.0 - 3.0 * dt).max(0.0);

        let before = self.pos;
        self.hit_wall = false;
        self.move_axis(w, 1, dy);
        if self.fly && self.on_ground {
            self.fly = false;
        }
        self.move_axis(w, 0, dx + kx);
        self.move_axis(w, 2, dz + kz);

        // Câmera: agachar, FOV correndo e balanço da caminhada (bob += (min(0,1, vel/tick) - bob) * 0,4 por tick)
        let moved = vec2(self.pos.x - before.x, self.pos.z - before.z).length();
        let ease = |rate: f32| 1.0 - (-rate * dt).exp();
        self.crouch += ((self.sneaking && !self.fly) as i32 as f32 - self.crouch) * ease(15.0);
        self.fov += (if self.sprinting { 1.1 } else { 1.0 } - self.fov) * ease(10.0);
        self.walk_dist += moved * 0.6;
        let bob = if self.on_ground && !self.fly && dt > 0.0 { (moved / dt / 20.0).min(0.1) } else { 0.0 };
        self.bob += (bob - self.bob) * ease(10.2);

        if self.pos.y < -20.0 {
            self.pos = Self::spawn();
            self.vel = Vec3::ZERO;
        }
    }

    /// Agachado no chão: corta o passo que deixaria o pé sem bloco embaixo (de 0,05 em 0,05, como no MC).
    fn edge_guard(&self, w: &World, axis: usize, mut d: f32) -> f32 {
        if !(self.mc && self.sneaking && self.on_ground && !self.fly) || axis == 1 {
            return d;
        }
        let supported = |d: f32| {
            let mut p = self.pos;
            p[axis] += d;
            self.collides_at(w, p - vec3(0.0, 0.6, 0.0))
        };
        while d != 0.0 && !supported(d) {
            d = if d.abs() < 0.05 { 0.0 } else { d - 0.05 * d.signum() };
        }
        d
    }

    fn move_axis(&mut self, w: &World, axis: usize, d: f32) {
        let d = self.edge_guard(w, axis, d);
        if d == 0.0 {
            return;
        }
        let mut p = self.pos;
        p[axis] += d;
        if !self.collides_at(w, p) {
            self.pos = p;
            if axis == 1 {
                self.on_ground = false;
            }
            return;
        }
        if axis == 1 {
            if d < 0.0 {
                let mut q = self.pos;
                q.y = p.y.floor() + 1.0;
                if q.y <= self.pos.y + 0.001 && !self.collides_at(w, q) {
                    self.pos = q;
                }
                self.on_ground = true;
            }
            self.vel.y = 0.0;
        } else {
            self.vel[axis] = 0.0;
            self.hit_wall = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    /// Mundo vazio com chão de pedra (topo em y = 10) até x < `edge`.
    fn world(edge: i32) -> World {
        let mut w = World { blocks: vec![AIR; (WX * WY * WZ) as usize], dirty: vec![false; (CX * CZ) as usize], guard: false };
        for z in 0..WZ {
            for x in 0..edge.min(WX) {
                w.set(x, 9, z, STONE);
            }
        }
        w
    }

    fn steve(x: f32) -> Player {
        let mut p = Player::new();
        (p.pos, p.yaw, p.mc, p.can_fly, p.fly, p.on_ground) = (vec3(x, 10.0, 100.5), 0.0, true, false, false, true);
        p
    }

    /// Roda `secs` segundos e devolve a velocidade horizontal média do último segundo.
    fn run(p: &mut Player, w: &World, t: &mut f64, secs: f32, inp: impl Fn(f64) -> McInput) -> f32 {
        let n = (secs / DT).round() as usize;
        let mut mark = p.pos;
        for i in 0..n {
            if i + 60 == n {
                mark = p.pos;
            }
            *t += DT as f64;
            p.step_mc(w, DT, &McInput { now: *t, ..inp(*t) });
        }
        vec2(p.pos.x - mark.x, p.pos.z - mark.z).length()
    }

    fn fwd() -> McInput {
        McInput { mv: vec2(0.0, 1.0), ..Default::default() }
    }

    #[test]
    fn minecraft_speeds() {
        let w = world(WX);
        let mut t = 0.0;
        let mut p = steve(20.5);
        let walk = run(&mut p, &w, &mut t, 3.0, |_| fwd());
        let mut p = steve(20.5);
        let sprint = run(&mut p, &w, &mut t, 3.0, |_| McInput { sprint: true, ..fwd() });
        let mut p = steve(20.5);
        let sneak = run(&mut p, &w, &mut t, 3.0, |_| McInput { sneak: true, ..fwd() });
        // Dois toques no W (com 0,1 s de intervalo) liga o sprint; soltar o W desliga
        let mut p = steve(20.5);
        let t0 = t;
        let tapped = run(&mut p, &w, &mut t, 3.0, |now| McInput { w_tap: now - t0 < DT as f64 * 1.5 || (now - t0 - 0.1).abs() < DT as f64 * 0.5, ..fwd() });
        assert!(p.sprinting);
        run(&mut p, &w, &mut t, 0.1, |_| McInput::default());
        assert!(!p.sprinting);
        println!("walk {walk:.3}  sprint {sprint:.3}  sneak {sneak:.3}  double-tap {tapped:.3} b/s");
        assert!((walk - WALK).abs() < 0.02 && (sprint - SPRINT).abs() < 0.02 && (sneak - SNEAK).abs() < 0.02 && (tapped - SPRINT).abs() < 0.02);
    }

    #[test]
    fn jump_and_sprint_jump() {
        let w = world(WX);
        let mut t = 0.0;
        let mut p = steve(20.5);
        let (y0, mut top, mut air) = (p.pos.y, 0.0f32, 0.0);
        let mut i = 0;
        while i == 0 || !p.on_ground {
            t += DT as f64;
            p.step_mc(&w, DT, &McInput { jump: i == 0, now: t, ..Default::default() });
            top = top.max(p.pos.y - y0);
            air += DT;
            i += 1;
        }
        let mut p = steve(20.5);
        run(&mut p, &w, &mut t, 1.0, |_| McInput { sprint: true, ..fwd() });
        let x0 = p.pos.x;
        run(&mut p, &w, &mut t, 6.0, |_| McInput { sprint: true, jump: true, ..fwd() });
        let sj = (p.pos.x - x0) / 6.0;
        println!("jump height {top:.3} b, airtime {air:.2} s, sprint-jump {sj:.3} b/s");
        assert!((top - 1.252).abs() < 0.03 && (air - 0.6).abs() < 0.08 && (6.6..7.3).contains(&sj));
    }

    #[test]
    fn sneak_stops_at_edge_and_sprint_stops_at_wall() {
        let w = world(30);
        let mut t = 0.0;
        let mut p = steve(25.5);
        run(&mut p, &w, &mut t, 6.0, |_| McInput { sneak: true, ..fwd() });
        println!("sneak edge x = {:.3} (edge 30), y = {:.2}", p.pos.x, p.pos.y);
        assert!(p.pos.y == 10.0 && p.pos.x > 29.6 && p.pos.x < 30.3 + 0.01);
        let mut w = world(WX);
        for y in 10..13 {
            for z in 0..WZ {
                w.set(28, y, z, STONE);
            }
        }
        let mut p = steve(20.5);
        run(&mut p, &w, &mut t, 3.0, |_| McInput { sprint: true, ..fwd() });
        assert!(!p.sprinting && p.pos.x < 27.7);
    }

    #[test]
    fn creative_flight() {
        let w = world(WX);
        let mut t = 0.0;
        let mut p = steve(20.5);
        p.can_fly = true;
        let t0 = t;
        // Dois toques no espaço: pula e liga o voo
        run(&mut p, &w, &mut t, 0.5, |now| {
            let k = ((now - t0) / DT as f64).round() as i32;
            McInput { space_tap: k == 1 || k == 8, jump: k <= 1 || k == 8, ..Default::default() }
        });
        assert!(p.fly);
        let fly = run(&mut p, &w, &mut t, 4.0, |_| fwd());
        let fast = run(&mut p, &w, &mut t, 4.0, |_| McInput { sprint: true, ..fwd() });
        let y = p.pos.y;
        run(&mut p, &w, &mut t, 1.0, |_| McInput { jump: true, ..Default::default() });
        let up = p.pos.y - y;
        run(&mut p, &w, &mut t, 4.0, |_| McInput { sneak: true, ..Default::default() });
        println!("fly {fly:.2}  sprint-fly {fast:.2} b/s, up ~{up:.2} b/s, landed fly={}", p.fly);
        assert!((fly - FLY).abs() < 0.15 && (fast - FLY_SPRINT).abs() < 0.4 && !p.fly && p.on_ground);
    }
}
