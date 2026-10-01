//! Itens do Steve: blocos (id = tipo do bloco) e ferramentas (id >= 256), com ícones pixel-art.

use crate::atlas::{self, Atlas};
use crate::batch::Batch;
use crate::world::*;
use crate::world::BLACK;
use macroquad::prelude::*;

pub type Item = u16;
pub const NONE: Item = 0;
pub const PICK: Item = 256;
pub const AXE: Item = 257;
pub const SHOVEL: Item = 258;
pub const SWORD: Item = 259;
pub const BOW: Item = 260;
pub const ARROW: Item = 261;
pub const FLINT: Item = 262;

const fn b(x: u8) -> Item {
    x as Item
}

/// Tudo que aparece no inventário do criativo.
pub const CREATIVE: [Item; 24] = [
    b(GRASS), b(DIRT), b(STONE), b(COBBLE), b(SAND), b(GRAVEL), b(PLANKS), b(LOG), b(LEAVES),
    b(GLASS), b(BRICK), b(WOOL), b(NEON), b(BLACK), b(CLUBWALL), b(BEDROCK), b(TNT), FLINT,
    PICK, AXE, SHOVEL, SWORD, BOW, ARROW,
];

pub fn is_block(it: Item) -> bool {
    it > 0 && it < 256
}

pub fn max_stack(it: Item) -> u16 {
    if is_block(it) || it == ARROW { 64 } else { 1 }
}

pub fn name(it: Item) -> &'static str {
    match it {
        PICK => "PICARETA",
        AXE => "MACHADO",
        SHOVEL => "PA",
        SWORD => "ESPADA",
        BOW => "ARCO (SEGURA E SOLTA)",
        ARROW => "FLECHAS",
        FLINT => "ISQUEIRO (ACENDE TNT / FOGO)",
        _ => match it as u8 {
            GRASS => "GRAMA",
            DIRT => "TERRA",
            STONE => "PEDRA",
            COBBLE => "PEDREGULHO",
            SAND => "AREIA",
            GRAVEL => "CASCALHO",
            PLANKS => "TABUA",
            LOG => "TRONCO",
            LEAVES => "FOLHAS",
            GLASS => "VIDRO",
            BRICK => "TIJOLO",
            WOOL => "LA",
            NEON => "NEON",
            BLACK => "BLOCO PRETO",
            CLUBWALL => "PAREDE DO CLUBE",
            BEDROCK => "BEDROCK",
            TNT => "TNT",
            _ => "",
        },
    }
}

/// Segundos pra quebrar `blk` segurando `tool` (sobrevivência). Ferramenta certa = 8x mais rápido.
pub fn break_time(blk: u8, tool: Item) -> f32 {
    let (base, best): (f32, Item) = match blk {
        GRASS | DIRT | SAND | GRAVEL => (0.9, SHOVEL),
        PLANKS | LOG => (3.0, AXE),
        STONE | COBBLE | BRICK | BLACK | CLUBWALL => (6.0, PICK),
        GLASS | NEON => (0.5, PICK),
        LEAVES => (0.35, AXE),
        WOOL => (1.2, NONE),
        TNT => (0.05, NONE),
        BEDROCK => return f32::INFINITY,
        _ => (1.0, NONE),
    };
    if tool == best { (base / 8.0).max(0.1) } else { base }
}

/// O que cai no inventário quando o bloco quebra.
pub fn drop(blk: u8) -> Option<Item> {
    match blk {
        GRASS => Some(b(DIRT)),
        STONE => Some(b(COBBLE)),
        LEAVES | GLASS | BEDROCK => None,
        x => Some(b(x)),
    }
}

/// Dano corpo a corpo em NPC (soco = 6, igual antes).
pub fn melee(it: Item) -> f32 {
    match it {
        SWORD => 16.0,
        AXE => 10.0,
        PICK => 8.0,
        SHOVEL => 7.0,
        _ => 6.0,
    }
}

const PICK_PX: [&str; 10] = ["..ssssss..", ".s..ww..s.", "s...ww...s", "....ww....", "....ww....", "....ww....", "....ww....", "....ww....", "....ww....", "....ww...."];
const AXE_PX: [&str; 10] = ["...ww.sss.", "...wwsssss", "...wwsssss", "...ww.sss.", "...ww.....", "...ww.....", "...ww.....", "...ww.....", "...ww.....", "...ww....."];
const SHOVEL_PX: [&str; 10] = ["...ssss...", "..ssssss..", "..ssssss..", "...ssss...", "....ww....", "....ww....", "....ww....", "....ww....", "...wwww...", "...w..w..."];
const SWORD_PX: [&str; 10] = ["....ss....", "...sLs....", "...sLs....", "...sLs....", "...sLs....", "...sLs....", ".yyyyyyy..", "....ww....", "....ww....", "....dd...."];
const BOW_PX: [&str; 10] = ["..wwwwq...", ".w....q...", "w.....q...", "w.....q...", "w.....q...", "w.....q...", "w.....q...", "w.....q...", ".w....q...", "..wwwwq..."];
const ARROW_PX: [&str; 10] = ["....dd....", "...dddd...", "....ww....", "....ww....", "....ww....", "....ww....", "....ww....", "...qwwq...", "...q..q...", ".........."];
const FLINT_PX: [&str; 10] = ["..sss.....", ".s...s....", ".s...s....", ".s...s....", "..s.s.....", "...s..fff.", "......ffff", ".....fffff", ".....ffff.", "......ff.."];

fn pixels(it: Item) -> Option<&'static [&'static str; 10]> {
    Some(match it {
        PICK => &PICK_PX,
        AXE => &AXE_PX,
        SHOVEL => &SHOVEL_PX,
        SWORD => &SWORD_PX,
        BOW => &BOW_PX,
        ARROW => &ARROW_PX,
        FLINT => &FLINT_PX,
        _ => return None,
    })
}

fn palette(c: u8) -> Color {
    match c {
        b'w' => Color::new(0.55, 0.38, 0.2, 1.0),
        b's' => Color::new(0.78, 0.8, 0.84, 1.0),
        b'L' => Color::new(0.95, 0.97, 1.0, 1.0),
        b'd' => Color::new(0.3, 0.3, 0.34, 1.0),
        b'y' => Color::new(0.85, 0.7, 0.2, 1.0),
        b'q' => Color::new(0.95, 0.95, 0.9, 1.0),
        b'f' => Color::new(0.18, 0.18, 0.2, 1.0),
        _ => BLANK,
    }
}

/// Ícone 2D do item dentro do quadrado (x, y, s).
pub fn draw_icon(atlas: &Atlas, it: Item, x: f32, y: f32, s: f32) {
    if is_block(it) {
        let t = face_tile(it as u8, 0);
        let pad = s * 0.14;
        draw_texture_ex(
            &atlas.tex,
            x + pad,
            y + pad,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(s - pad * 2.0, s - pad * 2.0)),
                source: Some(Rect::new(((t % atlas::COLS) * atlas::TILE) as f32, ((t / atlas::COLS) * atlas::TILE) as f32, atlas::TILE as f32, atlas::TILE as f32)),
                ..Default::default()
            },
        );
    } else if let Some(px) = pixels(it) {
        let p = s * 0.072;
        let (ox, oy) = (x + (s - p * 10.0) * 0.5, y + (s - p * 10.0) * 0.5);
        for (r, row) in px.iter().enumerate() {
            for (c, ch) in row.bytes().enumerate() {
                if ch != b'.' {
                    draw_rectangle(ox + c as f32 * p, oy + r as f32 * p, p + 0.5, p + 0.5, palette(ch));
                }
            }
        }
    }
}

/// Item na mão em 3D (matriz da mão; o cabo aponta pra +Y local).
pub fn draw_held(bt: &mut Batch, m: &Mat4, it: Item, avg: &[Color]) {
    let wood = palette(b'w');
    let steel = palette(b's');
    match it {
        NONE => {}
        _ if is_block(it) => bt.cube(m, vec3(0.0, 0.12, 0.0), Vec3::splat(0.24), avg[face_tile(it as u8, 0)]),
        PICK => {
            bt.cube(m, vec3(0.0, 0.2, 0.0), vec3(0.04, 0.45, 0.04), wood);
            bt.cube(m, vec3(0.0, 0.42, 0.0), vec3(0.34, 0.05, 0.05), steel);
        }
        AXE => {
            bt.cube(m, vec3(0.0, 0.2, 0.0), vec3(0.04, 0.45, 0.04), wood);
            bt.cube(m, vec3(0.07, 0.36, 0.0), vec3(0.12, 0.14, 0.03), steel);
        }
        SHOVEL => {
            bt.cube(m, vec3(0.0, 0.18, 0.0), vec3(0.04, 0.4, 0.04), wood);
            bt.cube(m, vec3(0.0, 0.42, 0.0), vec3(0.12, 0.14, 0.02), steel);
        }
        SWORD => {
            bt.cube(m, vec3(0.0, 0.05, 0.0), vec3(0.04, 0.12, 0.04), wood);
            bt.cube(m, vec3(0.0, 0.12, 0.0), vec3(0.16, 0.03, 0.04), palette(b'y'));
            bt.cube(m, vec3(0.0, 0.34, 0.0), vec3(0.05, 0.42, 0.015), steel);
        }
        BOW => {
            bt.cube(m, vec3(0.0, 0.2, -0.06), vec3(0.03, 0.42, 0.03), wood);
            bt.cube(m, vec3(0.0, 0.2, 0.02), vec3(0.01, 0.4, 0.01), palette(b'q'));
        }
        ARROW => bt.cube(m, vec3(0.0, 0.2, 0.0), vec3(0.025, 0.4, 0.025), wood),
        _ => {
            bt.cube(m, vec3(0.0, 0.08, 0.0), vec3(0.1, 0.1, 0.03), steel);
            bt.cube(m, vec3(0.05, 0.16, 0.0), vec3(0.08, 0.08, 0.04), palette(b'f'));
        }
    }
}
