//! Atlas de texturas 16x16 estilo Minecraft, gerado proceduralmente (sem assets).

use macroquad::prelude::*;

pub const TILE: usize = 16;
pub const COLS: usize = 16;
pub const ROWS: usize = 2;

pub const T_GRASS_TOP: usize = 0;
pub const T_GRASS_SIDE: usize = 1;
pub const T_DIRT: usize = 2;
pub const T_STONE: usize = 3;
pub const T_SAND: usize = 4;
pub const T_PLANKS: usize = 5;
pub const T_LOG_SIDE: usize = 6;
pub const T_LOG_TOP: usize = 7;
pub const T_LEAVES: usize = 8;
pub const T_COBBLE: usize = 9;
pub const T_GLASS: usize = 10;
pub const T_CLUBWALL: usize = 11;
pub const T_BLACK: usize = 12;
pub const T_BEDROCK: usize = 13;
pub const T_BRICK: usize = 14;
pub const T_GRAVEL: usize = 15;
pub const T_WOOL: usize = 16;
pub const T_NEON: usize = 17;
pub const T_WHITE: usize = 18;
pub const T_TNT_SIDE: usize = 19;
pub const T_TNT_TOP: usize = 20;
const N_TILES: usize = 21;

pub struct Atlas {
    pub tex: Texture2D,
    /// Cor média de cada tile (partículas, hotbar).
    pub avg: Vec<Color>,
}

pub fn hash2(x: i32, y: i32, s: u32) -> f32 {
    let mut n = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ s.wrapping_mul(2_246_822_519);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0x00ff_ffff) as f32 / 16_777_216.0
}

fn mul(c: (f32, f32, f32), f: f32) -> [u8; 3] {
    [
        (c.0 * f).clamp(0.0, 255.0) as u8,
        (c.1 * f).clamp(0.0, 255.0) as u8,
        (c.2 * f).clamp(0.0, 255.0) as u8,
    ]
}

const GRASS: (f32, f32, f32) = (96.0, 160.0, 52.0);
const DIRT: (f32, f32, f32) = (134.0, 96.0, 67.0);

fn dirt(x: i32, y: i32) -> [u8; 3] {
    let n = hash2(x, y, 31);
    let f = if n < 0.1 { 0.65 } else { 0.82 + 0.3 * n };
    mul(DIRT, f)
}

fn cobble(x: i32, y: i32) -> [u8; 3] {
    // Voronoi simples em grade 4x4 (tileável)
    let (cx, cy) = (x / 4, y / 4);
    let mut d1 = f32::MAX;
    let mut d2 = f32::MAX;
    let mut id = 0u32;
    for oy in -1..=1 {
        for ox in -1..=1 {
            let gx = cx + ox;
            let gy = cy + oy;
            let wx = gx.rem_euclid(4);
            let wy = gy.rem_euclid(4);
            let px = gx as f32 * 4.0 + 0.5 + hash2(wx, wy, 5) * 3.0;
            let py = gy as f32 * 4.0 + 0.5 + hash2(wx, wy, 6) * 3.0;
            let d = ((x as f32 - px).powi(2) + (y as f32 - py).powi(2)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                id = (wx + wy * 4) as u32;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    if d2 - d1 < 0.9 {
        mul((70.0, 70.0, 70.0), 0.9 + 0.2 * hash2(x, y, 9))
    } else {
        let g = 115.0 + hash2(id as i32, 0, 11) * 40.0;
        mul((g, g, g), 0.9 + 0.15 * hash2(x, y, 12))
    }
}

fn pixel(tile: usize, x: i32, y: i32) -> [u8; 3] {
    let n = hash2(x, y, tile as u32 * 7 + 1);
    match tile {
        T_GRASS_TOP => mul(GRASS, 0.8 + 0.32 * n),
        T_GRASS_SIDE => {
            let edge = 3 + (hash2(x, 0, 99) * 2.5) as i32;
            if y < edge { mul(GRASS, 0.8 + 0.3 * n) } else { dirt(x, y) }
        }
        T_DIRT => dirt(x, y),
        T_STONE => {
            let blob = hash2(x / 3, y / 2, 44);
            mul((125.0, 125.0, 125.0), 0.82 + 0.22 * n - if blob > 0.8 { 0.12 } else { 0.0 })
        }
        T_SAND => mul((219.0, 207.0, 163.0), 0.92 + 0.12 * n),
        T_PLANKS => {
            let row = y / 4;
            let seam = (row * 5 + 3).rem_euclid(16);
            let f = if y % 4 == 3 {
                0.62
            } else if x == seam {
                0.72
            } else {
                0.88 + 0.16 * hash2(x / 3, row, 17) + 0.05 * n
            };
            mul((162.0, 130.0, 78.0), f)
        }
        T_LOG_SIDE => {
            let f = if x % 4 == 0 { 0.65 } else { 0.78 + 0.3 * hash2(x, y / 3, 21) };
            mul((102.0, 81.0, 51.0), f)
        }
        T_LOG_TOP => {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d > 6.6 {
                mul((102.0, 81.0, 51.0), 0.8 + 0.2 * n)
            } else if (d as i32) % 2 == 0 {
                mul((150.0, 118.0, 72.0), 0.9 + 0.1 * n)
            } else {
                mul((176.0, 142.0, 92.0), 0.92 + 0.1 * n)
            }
        }
        T_LEAVES => {
            if n > 0.86 {
                mul((30.0, 70.0, 20.0), 1.0)
            } else {
                mul((62.0, 132.0, 40.0), 0.65 + 0.5 * n)
            }
        }
        T_COBBLE => cobble(x, y),
        T_GLASS => {
            if x == 0 || y == 0 || x == 15 || y == 15 {
                [235, 248, 255]
            } else if (x - y == 3 || x - y == 4) && x < 10 || (x - y == -5 && x > 6) {
                [250, 252, 255]
            } else {
                mul((170.0, 205.0, 225.0), 0.95 + 0.05 * n)
            }
        }
        T_CLUBWALL => {
            let off = if (y / 4) % 2 == 0 { 0 } else { 4 };
            if y % 4 == 0 || (x + off) % 8 == 0 {
                [22, 8, 36]
            } else {
                mul((70.0, 28.0, 105.0), 0.85 + 0.25 * n)
            }
        }
        T_BLACK => mul((26.0, 26.0, 32.0), 0.8 + 0.4 * n),
        T_BEDROCK => {
            let g = 40.0 + 80.0 * hash2(x / 2, y / 2, 3);
            mul((g, g, g), 0.9 + 0.2 * n)
        }
        T_BRICK => {
            let off = if (y / 4) % 2 == 0 { 0 } else { 4 };
            if y % 4 == 3 || (x + off) % 8 == 0 {
                [185, 175, 165]
            } else {
                mul((150.0, 62.0, 50.0), 0.85 + 0.25 * n)
            }
        }
        T_GRAVEL => {
            let k = hash2(x, y, 77);
            if k < 0.25 {
                mul((95.0, 90.0, 88.0), 1.0)
            } else if k > 0.85 {
                mul((170.0, 160.0, 150.0), 1.0)
            } else {
                mul((132.0, 126.0, 120.0), 0.9 + 0.2 * n)
            }
        }
        T_WOOL => mul((233.0, 236.0, 236.0), 0.9 + 0.1 * n - if y % 3 == 0 { 0.04 } else { 0.0 }),
        T_NEON => {
            let border = x == 0 || y == 0 || x == 15 || y == 15;
            if border { [170, 20, 140] } else { mul((255.0, 70.0, 210.0), 0.92 + 0.1 * n) }
        }
        T_TNT_SIDE => {
            // Faixa branca com "TNT" em pixel no meio
            const TXT: [&str; 5] = ["###.#..#.###", ".#..##.#..#.", ".#..#.##..#.", ".#..#..#..#.", ".#..#..#..#."];
            if (5..11).contains(&y) {
                let (tx, ty) = (x - 2, y - 6);
                let ink = (0..5).contains(&ty) && (0..12).contains(&tx) && TXT[ty as usize].as_bytes()[tx as usize] == b'#';
                if ink { [30, 30, 30] } else { mul((235.0, 235.0, 230.0), 0.95 + 0.05 * n) }
            } else if x % 4 == 0 {
                mul((150.0, 30.0, 25.0), 0.9 + 0.1 * n)
            } else {
                mul((205.0, 45.0, 35.0), 0.9 + 0.15 * n)
            }
        }
        T_TNT_TOP => {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 2.0 { [60, 60, 60] } else { mul((190.0, 45.0, 35.0), 0.85 + 0.2 * n) }
        }
        _ => [255, 255, 255],
    }
}

pub fn build() -> Atlas {
    let w = TILE * COLS;
    let h = TILE * ROWS;
    let mut bytes = vec![255u8; w * h * 4];
    let mut avg = Vec::with_capacity(N_TILES);
    for t in 0..N_TILES {
        let ox = (t % COLS) * TILE;
        let oy = (t / COLS) * TILE;
        let mut sum = [0f32; 3];
        for y in 0..TILE {
            for x in 0..TILE {
                let p = pixel(t, x as i32, y as i32);
                let i = ((oy + y) * w + ox + x) * 4;
                bytes[i] = p[0];
                bytes[i + 1] = p[1];
                bytes[i + 2] = p[2];
                bytes[i + 3] = 255;
                for k in 0..3 {
                    sum[k] += p[k] as f32;
                }
            }
        }
        let n = (TILE * TILE) as f32 * 255.0;
        avg.push(Color::new(sum[0] / n, sum[1] / n, sum[2] / n, 1.0));
    }
    let tex = Texture2D::from_rgba8(w as u16, h as u16, &bytes);
    tex.set_filter(FilterMode::Nearest);
    Atlas { tex, avg }
}

/// Retângulo UV (u0, v0, u1, v1) do tile, com pequeno inset contra bleeding.
pub fn uv(tile: usize) -> (f32, f32, f32, f32) {
    let tw = 1.0 / COLS as f32;
    let th = 1.0 / ROWS as f32;
    let tx = (tile % COLS) as f32;
    let ty = (tile / COLS) as f32;
    let e = 0.0005;
    (tx * tw + e, ty * th + e, (tx + 1.0) * tw - e, (ty + 1.0) * th - e)
}
