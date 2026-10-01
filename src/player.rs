//! Jogador primeira pessoa: física AABB contra voxels, voo, knockback.

use crate::world::*;
use macroquad::prelude::*;
use std::f32::consts::FRAC_PI_2;

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
}

impl Player {
    pub fn new() -> Self {
        Player { pos: Self::spawn(), vel: Vec3::ZERO, knock: Vec3::ZERO, yaw: -FRAC_PI_2, pitch: -0.3, fly: false, on_ground: false, sel: 0, stick: Vec2::ZERO, jump_held: false, can_fly: true }
    }

    pub fn spawn() -> Vec3 {
        crate::layout::SPAWN
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + vec3(0.0, 1.62, 0.0)
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

    fn move_axis(&mut self, w: &World, axis: usize, d: f32) {
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
        }
    }
}
