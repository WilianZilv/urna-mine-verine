//! Vida de todo NPC que não é lutador: villagers, VIPs do clube, robôs do lab, a urna e eu (a IA).
//! O host aplica dano e decide mortes; o estado vai compacto no snapshot ("n") pros outros clientes.

use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::FRAC_PI_2;

/// Grupos de alvo (lutadores têm vida própria em `actors::Fighter`).
pub const FIGHTER: u8 = 0;
pub const VILLAGER: u8 = 1;
pub const GUEST: u8 = 2;
pub const ROBOT: u8 = 3;
pub const URNA: u8 = 4;
pub const EU: u8 = 5;
pub const GUARD: u8 = 6;
pub const VOADOR: u8 = 7;
/// Entidades de mods (lista dinâmica, uma vida por instância, na ordem de `mods::Mods`).
pub const MODS: u8 = 8;
pub const KAIJU: u8 = 9;
pub const ZEPPELIN: u8 = 10;
pub const MARIO: u8 = 11;

#[derive(Clone, Copy)]
pub struct Vida {
    pub hp: f32,
    pub max: f32,
    /// > 0 = morto; segundos até renascer.
    pub down: f32,
    /// Segundos desde a morte (animação de queda).
    pub t: f32,
    pub flash: f32,
    /// Segundos que a barrinha de vida ainda fica visível sobre a cabeça.
    pub bar: f32,
    respawn: f32,
}

/// Quanto tempo a barrinha de vida fica na tela depois de tomar dano.
pub const BAR_SECS: f32 = 4.0;

impl Vida {
    fn new(max: f32, respawn: f32) -> Self {
        Vida { hp: max, max, down: 0.0, t: 0.0, flash: 0.0, bar: 0.0, respawn }
    }

    pub fn alive(&self) -> bool {
        self.down <= 0.0
    }

    /// Retorna true se morreu com esse golpe.
    fn hit(&mut self, dmg: f32) -> bool {
        if !self.alive() {
            return false;
        }
        self.hp -= dmg;
        self.flash = 1.0;
        self.bar = BAR_SECS;
        if self.hp > 0.0 {
            return false;
        }
        self.hp = 0.0;
        self.down = self.respawn;
        self.t = 0.0;
        true
    }

    /// Ângulo de queda pra trás (0 vivo, PI/2 estirado no chão).
    pub fn lean(&self) -> f32 {
        if self.alive() { 0.0 } else { (self.t * 3.5).min(1.0) * FRAC_PI_2 }
    }
}

pub struct Npcs {
    pub groups: [Vec<Vida>; 12],
}

pub struct Death {
    pub g: u8,
    pub i: usize,
}

impl Npcs {
    pub fn new(villagers: usize, guests: usize, robots: usize) -> Self {
        Npcs {
            groups: [
                Vec::new(),
                vec![Vida::new(30.0, 15.0); villagers],
                vec![Vida::new(45.0, 20.0); guests],
                vec![Vida::new(70.0, 20.0); robots],
                vec![Vida::new(1200.0, 45.0)],
                vec![Vida::new(500.0, 40.0)],
                vec![Vida::new(400.0, 45.0)],
                vec![Vida::new(900.0, 60.0)],
                Vec::new(),
                vec![Vida::new(1500.0, 90.0)],
                vec![Vida::new(3000.0, 180.0)],
                vec![Vida::new(260.0, 45.0)],
            ],
        }
    }

    /// (hp máximo, segundos pra renascer) de cada entidade de mod; recria a lista se mudou.
    pub fn sync_mods(&mut self, spec: &[(f32, f32)]) {
        let l = &mut self.groups[MODS as usize];
        if l.len() != spec.len() || l.iter().zip(spec).any(|(v, s)| v.max != s.0 || v.respawn != s.1) {
            *l = spec.iter().map(|&(hp, r)| Vida::new(hp, r)).collect();
        }
    }

    pub fn get(&self, g: u8, i: usize) -> Option<&Vida> {
        self.groups.get(g as usize)?.get(i)
    }

    pub fn alive(&self, g: u8, i: usize) -> bool {
        self.get(g, i).is_none_or(|v| v.alive())
    }

    pub fn urna(&self) -> &Vida {
        &self.groups[URNA as usize][0]
    }

    pub fn eu(&self) -> &Vida {
        &self.groups[EU as usize][0]
    }

    pub fn voador(&self) -> &Vida {
        &self.groups[VOADOR as usize][0]
    }

    pub fn kaiju(&self) -> &Vida {
        &self.groups[KAIJU as usize][0]
    }

    pub fn zeppelin(&self) -> &Vida {
        &self.groups[ZEPPELIN as usize][0]
    }

    pub fn guard(&self) -> &Vida {
        &self.groups[GUARD as usize][0]
    }

    /// Host: aplica dano. Some(true) = morreu agora.
    pub fn hit(&mut self, g: u8, i: usize, dmg: f32) -> Option<bool> {
        Some(self.groups.get_mut(g as usize)?.get_mut(i)?.hit(dmg))
    }

    /// Todos os clientes: anima; o host também decide quem renasce.
    pub fn tick(&mut self, dt: f32, host: bool) -> Vec<Death> {
        let mut back = Vec::new();
        for (g, list) in self.groups.iter_mut().enumerate() {
            for (i, v) in list.iter_mut().enumerate() {
                v.flash = (v.flash - dt * 4.0).max(0.0);
                v.bar = (v.bar - dt).max(0.0);
                if v.alive() {
                    continue;
                }
                v.t += dt;
                if host {
                    v.down -= dt;
                    if v.down <= 0.0 {
                        v.down = 0.0;
                        v.hp = v.max;
                        back.push(Death { g: g as u8, i });
                    }
                }
            }
        }
        back
    }

    /// Só quem não está com vida cheia: [grupo, índice, hp, segundos pra renascer, tempo morto].
    pub fn snapshot(&self) -> Value {
        let mut out = Vec::new();
        for (g, list) in self.groups.iter().enumerate() {
            for (i, v) in list.iter().enumerate().filter(|(_, v)| v.hp < v.max) {
                out.push(json!([g, i, v.hp.round(), (v.down * 10.0).round() / 10.0, (v.t * 10.0).round() / 10.0]));
            }
        }
        Value::Array(out)
    }

    pub fn apply(&mut self, m: &Value) {
        let mut seen: Vec<Vec<bool>> = self.groups.iter().map(|l| vec![false; l.len()]).collect();
        for e in m.as_array().into_iter().flatten() {
            let (g, i) = (e[0].as_u64().unwrap_or(99) as usize, e[1].as_u64().unwrap_or(0) as usize);
            let Some(v) = self.groups.get_mut(g).and_then(|l| l.get_mut(i)) else { continue };
            let hp = e[2].as_f64().unwrap_or(0.0) as f32;
            if hp < v.hp {
                v.flash = 1.0;
                v.bar = BAR_SECS;
            }
            v.hp = hp;
            let down = e[3].as_f64().unwrap_or(0.0) as f32;
            if down > 0.0 && v.alive() {
                v.t = e[4].as_f64().unwrap_or(0.0) as f32;
            }
            v.down = down;
            seen[g][i] = true;
        }
        for (g, list) in self.groups.iter_mut().enumerate() {
            for (i, v) in list.iter_mut().enumerate() {
                if !seen[g][i] {
                    v.hp = v.max;
                    v.down = 0.0;
                }
            }
        }
    }
}
