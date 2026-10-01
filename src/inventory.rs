//! Inventário do Steve: 9 slots de hotbar + 27 de mochila, hotbar estilo Minecraft e tela do E/I.

use crate::atlas::Atlas;
use crate::items::{self, *};
use crate::world::*;
use macroquad::prelude::*;

pub const SLOTS: usize = 36;

pub struct Inv {
    pub slots: [(Item, u16); SLOTS],
    /// Sobrevivência: slot pego na tela do inventário (o próximo clique troca).
    picked: Option<usize>,
}

impl Inv {
    fn from(hot: [(Item, u16); 9]) -> Self {
        let mut slots = [(NONE, 0); SLOTS];
        slots[..9].copy_from_slice(&hot);
        Inv { slots, picked: None }
    }

    pub fn survival() -> Self {
        Self::from([(PICK, 1), (AXE, 1), (SHOVEL, 1), (SWORD, 1), (BOW, 1), (ARROW, 32), (TNT as Item, 8), (FLINT, 1), (NONE, 0)])
    }

    pub fn creative() -> Self {
        Self::from([GRASS, STONE, PLANKS, LOG, GLASS, BRICK, NEON, TNT, 0].map(|b| (b as Item, 64)))
            .with(8, FLINT, 1)
    }

    fn with(mut self, k: usize, it: Item, n: u16) -> Self {
        self.slots[k] = (it, n);
        self
    }

    /// Junta na pilha existente, senão no primeiro slot vazio (hotbar primeiro).
    pub fn add(&mut self, it: Item, n: u16) -> bool {
        if let Some(s) = self.slots.iter_mut().find(|s| s.0 == it && s.1 + n <= max_stack(it)) {
            s.1 += n;
            return true;
        }
        if let Some(s) = self.slots.iter_mut().find(|s| s.0 == NONE) {
            *s = (it, n);
            return true;
        }
        false
    }

    /// Gasta 1 do slot `k`.
    pub fn use_slot(&mut self, k: usize) {
        let s = &mut self.slots[k];
        s.1 = s.1.saturating_sub(1);
        if s.1 == 0 {
            *s = (NONE, 0);
        }
    }

    /// Gasta 1 do item onde estiver (flechas).
    pub fn take(&mut self, it: Item) -> bool {
        match self.slots.iter().position(|s| s.0 == it) {
            Some(k) => {
                self.use_slot(k);
                true
            }
            None => false,
        }
    }

    pub fn count(&self, it: Item) -> u32 {
        self.slots.iter().filter(|s| s.0 == it).map(|s| s.1 as u32).sum()
    }
}

fn cell_bg(x: f32, y: f32, s: f32, on: bool) {
    draw_rectangle(x, y, s, s, Color::new(0.0, 0.0, 0.0, 0.5));
    draw_rectangle_lines(x, y, s, s, if on { 4.0 } else { 2.0 }, if on { WHITE } else { Color::new(0.35, 0.35, 0.35, 1.0) });
}

fn draw_slot(atlas: &Atlas, (it, n): (Item, u16), x: f32, y: f32, s: f32, on: bool, infinite: bool) {
    cell_bg(x, y, s, on);
    items::draw_icon(atlas, it, x, y, s);
    if n > 1 && !infinite {
        let t = n.to_string();
        let fs = (s * 0.38).max(13.0);
        let d = measure_text(&t, None, fs as u16, 1.0);
        draw_text(&t, x + s - d.width - 3.0, y + s - 4.0, fs, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text(&t, x + s - d.width - 4.0, y + s - 5.0, fs, WHITE);
    }
}

pub fn hotbar_origin(sw: f32, sh: f32, slot: f32) -> Vec2 {
    vec2(sw * 0.5 - slot * 4.5, sh - slot - 14.0)
}

pub fn draw_hotbar(inv: &Inv, atlas: &Atlas, sel: usize, sw: f32, sh: f32, slot: f32, creative: bool, mobile: bool) {
    let o = hotbar_origin(sw, sh, slot);
    for k in 0..9 {
        draw_slot(atlas, inv.slots[k], o.x + k as f32 * slot, o.y, slot, k == sel, creative);
    }
    if mobile {
        let x = o.x - slot - 6.0;
        cell_bg(x, o.y, slot, false);
        for d in [-1.0, 0.0, 1.0] {
            draw_circle(x + slot * 0.5 + d * slot * 0.22, o.y + slot * 0.5, slot * 0.07, WHITE);
        }
    }
}

/// Toque na hotbar: 0-8 = slot, 9 = botão "..." à esquerda que abre o inventário (só celular).
pub fn hotbar_hit(p: Vec2, sw: f32, sh: f32, slot: f32, mobile: bool) -> Option<usize> {
    let o = hotbar_origin(sw, sh, slot);
    if p.y < o.y || p.y > o.y + slot {
        return None;
    }
    if mobile && p.x >= o.x - slot - 6.0 && p.x < o.x - 6.0 {
        return Some(9);
    }
    let k = (p.x - o.x) / slot;
    (0.0..9.0).contains(&k).then_some(k as usize)
}

/// Grade da tela do inventário: (origem, tamanho da célula, linhas de cima).
fn panel(sw: f32, sh: f32, creative: bool) -> (Vec2, f32, usize) {
    let rows = if creative { CREATIVE.len().div_ceil(9) } else { 3 };
    let cell = (sw * 0.9 / 9.0).min((sh * 0.7) / (rows as f32 + 2.0)).min(60.0);
    let h = cell * (rows as f32 + 1.0) + cell * 0.4;
    (vec2(sw * 0.5 - cell * 4.5, sh * 0.5 - h * 0.5), cell, rows)
}

/// Célula clicada: Ok(k) = linha de cima (criativo: índice em CREATIVE, sobrevivência: slot 9+k);
/// Err(k) = hotbar k. None = fora.
fn cell_at(p: Vec2, sw: f32, sh: f32, creative: bool) -> Option<Result<usize, usize>> {
    let (o, cell, rows) = panel(sw, sh, creative);
    let col = ((p.x - o.x) / cell).floor();
    if !(0.0..9.0).contains(&col) || p.y < o.y {
        return None;
    }
    let col = col as usize;
    let row = ((p.y - o.y) / cell) as usize;
    if row < rows {
        return Some(Ok(row * 9 + col));
    }
    let hy = o.y + cell * (rows as f32 + 0.4);
    (p.y >= hy && p.y < hy + cell).then_some(Err(col))
}

pub fn draw_screen(inv: &Inv, atlas: &Atlas, sel: usize, sw: f32, sh: f32, creative: bool, mobile: bool) {
    let (o, cell, rows) = panel(sw, sh, creative);
    draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
    draw_rectangle(o.x - 10.0, o.y - 44.0, cell * 9.0 + 20.0, cell * (rows as f32 + 1.4) + 64.0, Color::new(0.55, 0.55, 0.58, 0.95));
    let title = if creative { "CRIATIVO: CLICA NUM BLOCO PRA POR NO SLOT DA HOTBAR" } else { "INVENTARIO: CLICA EM DOIS SLOTS PRA TROCAR" };
    let fs = (cell * 0.36).clamp(13.0, 20.0);
    draw_text(title, o.x, o.y - 16.0, fs, Color::new(0.1, 0.1, 0.12, 1.0));
    for r in 0..rows {
        for c in 0..9 {
            let k = r * 9 + c;
            let (x, y) = (o.x + c as f32 * cell, o.y + r as f32 * cell);
            if creative {
                match CREATIVE.get(k) {
                    Some(&it) => draw_slot(atlas, (it, 1), x, y, cell, false, true),
                    None => cell_bg(x, y, cell, false),
                }
            } else {
                draw_slot(atlas, inv.slots[9 + k], x, y, cell, inv.picked == Some(9 + k), false);
            }
        }
    }
    let hy = o.y + cell * (rows as f32 + 0.4);
    for c in 0..9 {
        let on = if creative { c == sel } else { inv.picked == Some(c) };
        draw_slot(atlas, inv.slots[c], o.x + c as f32 * cell, hy, cell, on, creative);
    }
    let hint = if mobile { "TOCA FORA PRA FECHAR" } else { "E / I / ESC FECHA" };
    draw_text(hint, o.x, hy + cell + 16.0, fs, Color::new(0.1, 0.1, 0.12, 1.0));
}

/// Clique/toque na tela do inventário. Retorna false se foi fora (fecha).
pub fn click(inv: &mut Inv, sel: &mut usize, p: Vec2, sw: f32, sh: f32, creative: bool) -> bool {
    let Some(hit) = cell_at(p, sw, sh, creative) else {
        inv.picked = None;
        return false;
    };
    if creative {
        match hit {
            Ok(k) => {
                if let Some(&it) = CREATIVE.get(k) {
                    inv.slots[*sel] = (it, max_stack(it));
                }
            }
            Err(c) => *sel = c,
        }
        return true;
    }
    let k = match hit {
        Ok(k) => 9 + k,
        Err(c) => c,
    };
    match inv.picked.take() {
        Some(a) if a != k => inv.slots.swap(a, k),
        Some(_) => {}
        None => inv.picked = Some(k),
    }
    true
}
