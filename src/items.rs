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

type Sprite = [[u8; 16]; 16];

// Sprites 16x16 estilo Minecraft (cabo embaixo à esquerda, cabeça em cima à direita).
// h/H cabo, L/l/m/o cabeça brilho/clara/média/contorno (cor por ferramenta), w/W/g madeira e empunhadura
// do arco, q corda, f/F pena, d/D guarda e pomo da espada, k/K/x pederneira.
const PICK_PX: [&str; 16] = [
    "................",
    "...oooooooooo...",
    ".oolllllllllllo.",
    "olmmmmmmmmmlllo.",
    "omooooooooohllo.",
    "oo........homlo.",
    ".........hHomlo.",
    "........hH.omlo.",
    ".......hH..omlo.",
    "......hH...omlo.",
    ".....hH....omlo.",
    "....hH.....omlo.",
    "...hH......omlo.",
    "..hH.......omo..",
    ".hH.......omlo..",
    "HH........ooo...",
];
const AXE_PX: [&str; 16] = [
    "................",
    "....oooo........",
    "...oLlllo....H..",
    "..oLllllmo..hH..",
    "..oLlllmmmohH...",
    "..oLllmmmmhH....",
    "..oLllmmmhH.....",
    "..oLlmmmhH......",
    "..oLlmmhH.......",
    "...ooohH........",
    ".....hH.........",
    "....hH..........",
    "...hH...........",
    "..hH............",
    ".hH.............",
    "HH..............",
];
const SHOVEL_PX: [&str; 16] = [
    "...........oo...",
    "..........ollo..",
    ".........ollmmo.",
    "........ollmmmo.",
    "........olmmmo..",
    ".........ommo...",
    ".........hoo....",
    "........hH......",
    ".......hH.......",
    "......hH........",
    ".....hH.........",
    "....hH..........",
    "...hH...........",
    "..hH............",
    ".hH.............",
    "HH..............",
];
const SWORD_PX: [&str; 16] = [
    ".............ooo",
    "............olLo",
    "...........olmo.",
    "..........olmo..",
    ".........olmo...",
    "........olmo....",
    ".......olmo.....",
    ".Dd...olmo......",
    "..Dd.olmo.......",
    "...Ddlmo........",
    "....Dd..........",
    "....hDd.........",
    "...hH.Dd........",
    "..hH...Dd.......",
    "dD..............",
    "Dd..............",
];
/// Só a madeira: a corda (e a flecha encaixada) entra em `sprite`.
const BOW_PX: [&str; 16] = [
    "................",
    "........wwwWWW..",
    "......wwWW......",
    ".....gWW........",
    "....gW..........",
    "...gW...........",
    "..wW............",
    "..wW............",
    ".wW.............",
    ".wW.............",
    ".w..............",
    ".W..............",
    ".W..............",
    ".W..............",
    "................",
    "................",
];
const ARROW_PX: [&str; 16] = [
    "................",
    "................",
    "..........oolL..",
    "...........mll..",
    "...........hmlo.",
    "..........hH.mo.",
    ".........hH.....",
    "........hH......",
    ".......hH.......",
    "......hH........",
    ".....hH.........",
    "..f.hH..........",
    ".fFhH...........",
    "..hF............",
    ".hHf............",
    "................",
];
const FLINT_PX: [&str; 16] = [
    "................",
    "...oooo.........",
    "..ollllo........",
    ".olmoomlo.......",
    ".lmo..oml.......",
    ".lo....ol.......",
    ".lo....ol.......",
    ".mo...oom.......",
    "..mo..om..kkk...",
    "..........kxxKk.",
    ".........kxxKKKk",
    ".........kxKKKKk",
    "........kKKKKKk.",
    ".........kKKKk..",
    "..........kkk...",
    "................",
];

fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::new(r, g, b, 1.0)
}

fn head_color(it: Item) -> Color {
    match it {
        PICK => rgb(0.36, 0.9, 0.84),
        SHOVEL => rgb(0.98, 0.83, 0.28),
        ARROW | FLINT => rgb(0.76, 0.76, 0.79),
        _ => rgb(0.9, 0.9, 0.92),
    }
}

fn color(it: Item, c: u8) -> Color {
    let h = head_color(it);
    let head = |f: f32| rgb(h.r * f, h.g * f, h.b * f);
    match c {
        b'L' => rgb(h.r * 0.4 + 0.6, h.g * 0.4 + 0.6, h.b * 0.4 + 0.6),
        b'l' => head(1.0),
        b'm' => head(0.76),
        b'o' => head(0.42),
        b'h' | b'w' => rgb(0.6, 0.42, 0.22),
        b'H' => rgb(0.35, 0.24, 0.11),
        b'W' => rgb(0.38, 0.25, 0.12),
        b'g' => rgb(0.27, 0.18, 0.09),
        b'q' => rgb(0.86, 0.86, 0.84),
        b'f' => rgb(0.96, 0.96, 0.96),
        b'F' => rgb(0.67, 0.67, 0.67),
        b'd' => rgb(0.3, 0.22, 0.14),
        b'D' => rgb(0.18, 0.13, 0.08),
        b'k' => rgb(0.3, 0.3, 0.32),
        b'K' => rgb(0.17, 0.17, 0.19),
        b'x' => rgb(0.52, 0.52, 0.55),
        _ => BLANK,
    }
}

fn line(s: &mut Sprite, a: IVec2, b: IVec2, ch: u8) {
    let n = (b - a).abs().max_element();
    for k in 0..=n {
        let p = a.as_vec2() + (b - a).as_vec2() * (k as f32 / n.max(1) as f32);
        let (x, y) = (p.x.round() as usize, p.y.round() as usize);
        if x < 16 && y < 16 && s[y][x] == b'.' {
            s[y][x] = ch;
        }
    }
}

/// Pixel-art do item. `pull` 1..=3: arco puxado (corda em V e flecha encaixada apontando pra cima/esquerda).
fn sprite(it: Item, pull: u8) -> Option<Sprite> {
    let rows = match it {
        PICK => &PICK_PX,
        AXE => &AXE_PX,
        SHOVEL => &SHOVEL_PX,
        SWORD => &SWORD_PX,
        BOW => &BOW_PX,
        ARROW => &ARROW_PX,
        FLINT => &FLINT_PX,
        _ => return None,
    };
    let mut s = [[b'.'; 16]; 16];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.bytes().take(16).enumerate() {
            s[r][c] = ch;
        }
    }
    if it == BOW {
        let (a, b) = (ivec2(12, 1), ivec2(1, 12));
        if pull == 0 {
            line(&mut s, a, b, b'q');
        } else {
            let n = 6 + pull.min(3) as usize;
            line(&mut s, a, ivec2(n as i32, n as i32), b'q');
            line(&mut s, ivec2(n as i32, n as i32), b, b'q');
            for k in 0..n {
                s[n - k][n - k] = if k == 0 { b'f' } else { b'h' };
            }
            s[n][n + 1] = b'f';
            s[n + 1][n] = b'f';
            s[1][1] = b'l';
            s[2][2] = b'm';
            s[1][2] = b'o';
            s[2][1] = b'o';
        }
    }
    Some(s)
}

/// Faixas horizontais da mesma cor: (linha, coluna, largura, cor).
fn runs(s: &Sprite, mut f: impl FnMut(usize, usize, usize, u8)) {
    for (r, row) in s.iter().enumerate() {
        let mut c = 0;
        while c < 16 {
            let ch = row[c];
            let mut n = 1;
            while c + n < 16 && row[c + n] == ch {
                n += 1;
            }
            if ch != b'.' {
                f(r, c, n, ch);
            }
            c += n;
        }
    }
}

/// Ícone 2D do item dentro do quadrado (x, y, s), em escala inteira de pixel.
pub fn draw_icon(atlas: &Atlas, it: Item, x: f32, y: f32, s: f32) {
    let p = (s * 0.72 / 16.0).floor().max(1.0);
    let d = p * 16.0;
    let (ox, oy) = ((x + (s - d) * 0.5).round(), (y + (s - d) * 0.5).round());
    if is_block(it) {
        // Cubo isométrico como no inventário do Minecraft: topo claro, lado esquerdo médio, direito escuro
        let (w, cx) = (d * 0.9, ox + d * 0.5);
        let (e, q, sh) = (w * 0.5, w * 0.25, w * 0.58);
        let top = oy + (d - (2.0 * q + sh)) * 0.5;
        let (t, l, r, c) = (vec2(cx, top), vec2(cx - e, top + q), vec2(cx + e, top + q), vec2(cx, top + 2.0 * q));
        let down = vec2(0.0, sh);
        let mut vertices = Vec::with_capacity(12);
        for (face, quad, shade) in [(2, [t, r, c, l], 1.0), (4, [l, c, c + down, l + down], 0.8), (0, [c, r, r + down, c + down], 0.6)] {
            let (u0, v0, u1, v1) = atlas::uv(face_tile(it as u8, face));
            let col = Color::new(shade, shade, shade, 1.0);
            for (p, (u, v)) in quad.iter().zip([(u0, v0), (u1, v0), (u1, v1), (u0, v1)]) {
                vertices.push(Vertex::new(p.x, p.y, 0.0, u, v, col));
            }
        }
        let indices = (0..3u16).flat_map(|f| [0, 1, 2, 0, 2, 3].map(|k| f * 4 + k)).collect();
        draw_mesh(&Mesh { vertices, indices, texture: Some(atlas.tex.clone()) });
    } else if let Some(sp) = sprite(it, 0) {
        runs(&sp, |r, c, n, ch| draw_rectangle(ox + c as f32 * p, oy + r as f32 * p, n as f32 * p, p, color(it, ch)));
    }
}

/// Item em 3D centrado na origem de `m` (1 unidade = 16 px): sprite extrudado em faixas de 1 px de
/// espessura; bloco vira cubo texturizado em `tm`. `pull`: estágio do arco (0 = solto).
pub fn draw_model(bt: &mut Batch, tm: &mut TexMesh, m: &Mat4, it: Item, pull: u8) {
    if is_block(it) {
        tm.block(m, it as u8, WHITE);
        return;
    }
    let Some(sp) = sprite(it, pull) else { return };
    let px = 1.0 / 16.0;
    runs(&sp, |r, c, n, ch| {
        let w = n as f32;
        bt.cube(m, vec3((c as f32 + w * 0.5 - 8.0) * px, (7.5 - r as f32) * px, 0.0), vec3(w * px, px, px), color(it, ch));
    });
}

/// Cubos com textura do atlas (bloco na mão, TNT acesa). Desenha com `flush` depois do batch.
pub struct TexMesh {
    v: Vec<Vertex>,
    i: Vec<u16>,
}

impl TexMesh {
    pub fn new() -> Self {
        TexMesh { v: Vec::new(), i: Vec::new() }
    }

    /// Cubo unitário centrado na origem de `m`, com as faces do bloco `blk`.
    pub fn block(&mut self, m: &Mat4, blk: u8, tint: Color) {
        self.cube(m, |f| face_tile(blk, f), tint);
    }

    pub fn cube(&mut self, m: &Mat4, tile: impl Fn(usize) -> usize, tint: Color) {
        const FACES: [([usize; 4], Vec3); 6] = [
            ([1, 3, 7, 5], Vec3::X),
            ([0, 4, 6, 2], Vec3::NEG_X),
            ([2, 6, 7, 3], Vec3::Y),
            ([0, 1, 5, 4], Vec3::NEG_Y),
            ([4, 5, 7, 6], Vec3::Z),
            ([0, 2, 3, 1], Vec3::NEG_Z),
        ];
        let light = vec3(0.4, 1.0, 0.3).normalize();
        let corner = |k: usize| vec3((k & 1) as f32, ((k >> 1) & 1) as f32, ((k >> 2) & 1) as f32);
        for (f, (q, n)) in FACES.iter().enumerate() {
            let (u0, v0, u1, v1) = atlas::uv(tile(f));
            let nw = m.transform_vector3(*n).normalize_or_zero();
            let shade = 0.5 + 0.45 * nw.dot(light).max(0.0) + 0.12 * nw.y.max(0.0);
            let col = Color::new(tint.r * shade, tint.g * shade, tint.b * shade, tint.a);
            let base = self.v.len() as u16;
            for &k in q {
                let s = corner(k);
                let (u, v) = match f {
                    0 => (1.0 - s.z, 1.0 - s.y),
                    1 => (s.z, 1.0 - s.y),
                    4 => (s.x, 1.0 - s.y),
                    5 => (1.0 - s.x, 1.0 - s.y),
                    _ => (s.x, s.z),
                };
                self.v.push(Vertex::new2(m.transform_point3(s - Vec3::splat(0.5)), vec2(u0 + (u1 - u0) * u, v0 + (v1 - v0) * v), col));
            }
            self.i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    pub fn flush(&mut self, tex: &Texture2D) {
        if !self.i.is_empty() {
            let gl = unsafe { get_internal_gl() }.quad_gl;
            gl.texture(Some(tex));
            gl.draw_mode(DrawMode::Triangles);
            gl.geometry(&self.v, &self.i);
        }
        self.v.clear();
        self.i.clear();
    }
}
