//! TNT acesa como no Minecraft: ao acender o bloco some e vira entidade (pulinho pra cima, gravidade,
//! colisão, pisca), explode uma vez só. Explosão acende as TNTs que ela arranca com pavio curto.
//! Todo mundo simula igual (ids determinísticos); só o host manda a explosão ("shot" com "tnt": id).

use crate::urna;
use crate::world::*;
use macroquad::prelude::*;
use std::collections::HashSet;

/// Pavio de quem acende (80 ticks).
pub const FUSE: f32 = 4.0;
pub const R: f32 = 3.5;
/// Teto de TNT acesa ao mesmo tempo e de explosões mandadas por frame (o resto espera).
pub const MAX_PRIMED: usize = 128;
pub const BOOMS_PER_FRAME: usize = 6;
const H: f32 = 0.49;
const GRAVITY: f32 = 16.0;

pub struct Primed {
    pub id: u64,
    /// Centro da base.
    pub pos: Vec3,
    pub vel: Vec3,
    pub t: f32,
}

#[derive(Default)]
pub struct Tnt {
    pub primed: Vec<Primed>,
    /// Ids que já explodiram (nunca reacende).
    done: HashSet<u64>,
    /// Ids de explosão já aplicados (eco repetido não explode de novo).
    heard: HashSet<u64>,
}

fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) & ((1 << 53) - 1)
}

/// Id da TNT no bloco `p` acesa pela explosão `seed` (cabe num número do JS).
pub fn id_at(seed: u64, p: IVec3) -> u64 {
    mix(seed ^ mix(((p.x as u64) << 40) ^ ((p.y as u64) << 20) ^ p.z as u64))
}

/// Semente de uma explosão sem id (laser da urna, etc.).
pub fn seed_of(c: Vec3, r: f32) -> u64 {
    mix(((c.x.to_bits() as u64) << 32) ^ ((c.y.to_bits() as u64) << 16) ^ c.z.to_bits() as u64 ^ r.to_bits() as u64)
}

fn unit(id: u64, k: u64) -> f32 {
    (mix(id ^ k) % 10_000) as f32 / 10_000.0
}

fn hits(world: &World, pos: Vec3) -> bool {
    let (lo, hi) = (pos - vec3(H, 0.0, H), pos + vec3(H, 2.0 * H, H));
    for y in lo.y.floor() as i32..=hi.y.floor() as i32 {
        for z in lo.z.floor() as i32..=hi.z.floor() as i32 {
            for x in lo.x.floor() as i32..=hi.x.floor() as i32 {
                if world.solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

impl Tnt {
    pub fn clear(&mut self) {
        self.primed.clear();
    }

    /// Acende a TNT do bloco `p` (antes do evento tirar o bloco). Bloco que não pode sumir não acende.
    pub fn prime(&mut self, world: &World, p: IVec3, id: u64, fuse: f32) -> bool {
        if world.get(p.x, p.y, p.z) != TNT || (world.guard && crate::shield::protected(p)) || self.primed.len() >= MAX_PRIMED || self.done.contains(&id) || self.primed.iter().any(|e| e.id == id) {
            return false;
        }
        let a = unit(id, 1) * std::f32::consts::TAU;
        self.primed.push(Primed { id, pos: p.as_vec3() + vec3(0.5, 0.0, 0.5), vel: vec3(-a.sin() * 0.4, 4.0, -a.cos() * 0.4), t: fuse });
        true
    }

    /// Evento de explosão (antes de aplicar). `false` = eco repetido, não aplicar. Tira a TNT que explodiu,
    /// empurra as acesas e acende com pavio curto (10-30 ticks) as que a explosão vai arrancar.
    pub fn on_blast(&mut self, world: &World, c: Vec3, r: f32, id: Option<u64>) -> bool {
        if let Some(id) = id {
            if !self.heard.insert(id) {
                return false;
            }
            self.done.insert(id);
            self.primed.retain(|e| e.id != id);
        }
        for e in self.primed.iter_mut() {
            let d = (e.pos + Vec3::Y * H).distance(c);
            if d < r * 2.0 {
                e.vel += (e.pos + Vec3::Y * H - c).normalize_or(Vec3::Y) * (1.0 - d / (r * 2.0)) * 8.0;
            }
        }
        let seed = id.unwrap_or_else(|| seed_of(c, r));
        let ri = r.ceil() as i32 + 1;
        let ci = c.floor().as_ivec3();
        for dy in -ri..=ri {
            for dz in -ri..=ri {
                for dx in -ri..=ri {
                    let p = ci + ivec3(dx, dy, dz);
                    if world.get(p.x, p.y, p.z) == TNT && urna::blast_hits(c, r, p) {
                        let id = id_at(seed, p);
                        self.prime(world, p, id, 0.5 + unit(id, 2));
                    }
                }
            }
        }
        true
    }

    /// Física e pavio. Host: devolve até BOOMS_PER_FRAME explosões (id, centro) pra mandar.
    pub fn update(&mut self, world: &World, dt: f32, is_host: bool) -> Vec<(u64, Vec3)> {
        let drag = 0.98f32.powf(dt * 20.0);
        for e in self.primed.iter_mut() {
            e.t -= dt;
            e.vel.y -= GRAVITY * dt;
            e.vel *= drag;
            for axis in 0..3 {
                let mut p = e.pos;
                p[axis] += e.vel[axis] * dt;
                if hits(world, p) {
                    if axis == 1 && e.vel.y < 0.0 {
                        let f = 0.7f32.powf(dt * 20.0);
                        (e.vel.x, e.vel.z) = (e.vel.x * f, e.vel.z * f);
                    }
                    e.vel[axis] = 0.0;
                } else {
                    e.pos = p;
                }
            }
        }
        let mut out = Vec::new();
        if is_host {
            self.primed.retain(|e| {
                if e.t <= 0.0 && out.len() < BOOMS_PER_FRAME {
                    out.push((e.id, e.pos + Vec3::Y * H));
                    return false;
                }
                true
            });
            self.done.extend(out.iter().map(|o| o.0));
        } else {
            // Cliente espera o "shot" do host; se não vier, some
            self.primed.retain(|e| e.t > -2.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::urna::Fx;

    /// Host offline: explosão mandada volta pelo loopback e passa por on_blast + explode.
    fn run(w: &mut World, tnt: &mut Tnt, max_frames: usize) -> Vec<u64> {
        let avg = vec![WHITE; 256];
        let mut fx = Fx::default();
        let mut booms = Vec::new();
        for _ in 0..max_frames {
            for (id, c) in tnt.update(w, 0.05, true) {
                assert!(tnt.on_blast(w, c, R, Some(id)));
                assert!(!tnt.on_blast(w, c, R, Some(id)), "eco repetido não pode explodir de novo");
                urna::explode(w, c, R, &mut fx, &avg);
                booms.push(id);
            }
            assert!(tnt.primed.len() <= MAX_PRIMED);
            if tnt.primed.is_empty() {
                return booms;
            }
        }
        panic!("TNT não terminou: {} acesas", tnt.primed.len());
    }

    fn open_spot() -> IVec3 {
        let c = crate::layout::plaza_center().as_ivec3();
        ivec3(c.x + 12, 0, c.z)
    }

    fn ignite(w: &mut World, tnt: &mut Tnt, p: IVec3) {
        assert!(tnt.prime(w, p, 1, FUSE));
        w.set(p.x, p.y, p.z, AIR);
    }

    #[test]
    fn chain_of_50_terminates() {
        let mut w = crate::mods::generate();
        let o = open_spot();
        let line: Vec<IVec3> = (0..50).map(|i| ivec3(o.x + i - 25, G, o.z)).collect();
        for p in &line {
            assert!(!crate::shield::protected(*p));
            w.set(p.x, p.y, p.z, TNT);
        }
        let mut tnt = Tnt::default();
        ignite(&mut w, &mut tnt, line[0]);
        let booms = run(&mut w, &mut tnt, 4000);
        assert_eq!(booms.len(), 50);
        assert_eq!(booms.iter().collect::<HashSet<_>>().len(), 50, "cada TNT explode uma vez");
        assert!(line.iter().all(|p| w.get(p.x, p.y, p.z) != TNT));
    }

    #[test]
    fn cube_5x5x5_explodes_once_each() {
        let mut w = crate::mods::generate();
        let o = open_spot() + ivec3(0, G, 0);
        for y in 0..5 {
            for z in 0..5 {
                for x in 0..5 {
                    w.set(o.x + x, o.y + y, o.z + z, TNT);
                }
            }
        }
        let mut tnt = Tnt::default();
        ignite(&mut w, &mut tnt, o);
        let booms = run(&mut w, &mut tnt, 4000);
        assert_eq!(booms.len(), 125);
        assert_eq!(booms.iter().collect::<HashSet<_>>().len(), 125);
    }

    #[test]
    fn tnt_in_shield_never_loops() {
        let mut w = crate::mods::generate();
        let c = crate::lab::dome_center().as_ivec3() + ivec3(0, 1, 0);
        assert!(crate::shield::protected(c));
        w.guard = false;
        w.set(c.x, c.y, c.z, TNT);
        w.set(c.x + 1, c.y, c.z, TNT);
        w.guard = true;
        let mut tnt = Tnt::default();
        assert!(!tnt.prime(&w, c, 1, FUSE), "TNT que não pode sumir não acende");
        let avg = vec![WHITE; 256];
        let mut fx = Fx::default();
        let at = c.as_vec3() + Vec3::splat(0.5);
        for i in 0..50 {
            assert!(tnt.on_blast(&w, at, R, Some(100 + i)));
            urna::explode(&mut w, at, R, &mut fx, &avg);
        }
        assert!(tnt.primed.is_empty());
        assert!(run(&mut w, &mut tnt, 10).is_empty());
        assert_eq!(w.get(c.x, c.y, c.z), TNT);
    }
}
