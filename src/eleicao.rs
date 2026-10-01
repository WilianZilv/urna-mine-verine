//! Contagem regressiva gigante pro 1º turno (04/10/2026 08:00 Brasília) e evento na abertura.

use crate::batch::Batch;
use crate::world::G;
use macroquad::prelude::*;

/// 2026-10-04 11:00 UTC = 08:00 em Brasília (abertura das urnas).
const ABERTURA: f64 = 1_791_111_600.0;
/// Urnas abertas das 08:00 às 17:00.
const ABERTO: f64 = 9.0 * 3600.0;
/// Duração do evento (fogos + urna enlouquecida).
pub const EVENTO: f64 = 120.0;

/// Segundos até a abertura (negativo = já abriu). ELEICAO_EM_SEGUNDOS antecipa pra teste.
pub struct Relogio {
    alvo: f64,
}

impl Relogio {
    pub fn new() -> Self {
        let teste = std::env::var("ELEICAO_EM_SEGUNDOS").ok().and_then(|v| v.parse::<f64>().ok());
        Relogio { alvo: teste.map(|s| now() + s).unwrap_or(ABERTURA) }
    }

    pub fn faltam(&self) -> f64 {
        self.alvo - now()
    }

    pub fn evento(&self) -> bool {
        (-EVENTO..=0.0).contains(&self.faltam())
    }

    fn linhas(&self) -> [String; 3] {
        let f = self.faltam();
        if f > 0.0 {
            let s = f as u64;
            let d = s / 86400;
            ["FALTAM".into(), format!("{d} {} {:02}:{:02}:{:02}", if d == 1 { "DIA" } else { "DIAS" }, s / 3600 % 24, s / 60 % 60, s % 60), "PRAS ELEICOES 2026".into()]
        } else if f + ABERTO > 0.0 {
            let s = (f + ABERTO) as u64;
            ["URNAS ABERTAS!".into(), format!("FECHA EM {:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60), "VAI VOTAR!".into()]
        } else {
            ["URNAS FECHADAS".into(), "APURACAO".into(), "ELEICOES 2026".into()]
        }
    }

    /// Placar flutuando no norte da praça, virado pro sul (spawn).
    pub fn draw(&self, b: &mut Batch, time: f32) {
        let id = Mat4::IDENTITY;
        let (cx, z, top) = (64.0, 36.0, G as f32 + 27.0);
        let px = 0.8;
        let linhas = self.linhas();
        let scale = |l: usize, t: &str| if l == 1 && t.len() <= 16 { 1.5 } else { 1.0 };
        let largura = linhas.iter().enumerate().map(|(l, t)| t.len() as f32 * 4.0 * px * scale(l, t)).fold(0.0, f32::max);
        b.cube(&id, vec3(cx, top - 10.0 * px, z - 0.4), vec3(largura + 3.0, 22.0 * px, 0.4), Color::new(0.04, 0.04, 0.06, 1.0));
        let pulse = 0.75 + 0.25 * (time * 4.0).sin();
        let cores = [Color::new(1.0, 1.0, 1.0, 1.0), Color::new(1.0, 0.25 * pulse, 0.2, 1.0), Color::new(0.2, 1.0, 0.35, 1.0)];
        for (l, (txt, col)) in linhas.iter().zip(cores).enumerate() {
            let p = px * scale(l, txt);
            let w = txt.len() as f32 * 4.0 * p;
            let y0 = top - [0.0, 6.0 * px, 15.0 * px][l] - if l == 1 && p == px { 1.0 * px } else { 0.0 };
            for (k, ch) in txt.chars().enumerate() {
                let g = glyph(ch);
                for row in 0..5 {
                    for colx in 0..3 {
                        if g.as_bytes()[row * 4 + colx] == b'#' {
                            let x = cx - w * 0.5 + (k as f32 * 4.0 + colx as f32 + 0.5) * p;
                            let y = y0 - (row as f32 + 0.5) * p;
                            b.glow(&id, vec3(x, y, z), vec3(p * 0.92, p * 0.92, 0.3), col);
                        }
                    }
                }
            }
        }
    }
}

fn now() -> f64 {
    macroquad::miniquad::date::now()
}

/// Fonte 3x5: 5 linhas de 3 colunas separadas por espaço.
fn glyph(c: char) -> &'static str {
    match c {
        '0' | 'O' => "### #.# #.# #.# ###",
        '1' => ".#. ##. .#. .#. ###",
        '2' => "### ..# ### #.. ###",
        '3' => "### ..# ### ..# ###",
        '4' => "#.# #.# ### ..# ..#",
        '5' | 'S' => "### #.. ### ..# ###",
        '6' => "### #.. ### #.# ###",
        '7' => "### ..# ..# ..# ..#",
        '8' => "### #.# ### #.# ###",
        '9' => "### #.# ### ..# ###",
        'A' => ".#. #.# ### #.# #.#",
        'B' => "##. #.# ##. #.# ##.",
        'C' => "### #.. #.. #.. ###",
        'D' => "##. #.# #.# #.# ##.",
        'E' => "### #.. ##. #.. ###",
        'F' => "### #.. ##. #.. #..",
        'G' => "### #.. #.# #.# ###",
        'H' => "#.# #.# ### #.# #.#",
        'I' => "### .#. .#. .#. ###",
        'J' => "..# ..# ..# #.# ###",
        'K' => "#.# #.# ##. #.# #.#",
        'L' => "#.. #.. #.. #.. ###",
        'M' => "#.# ### ### #.# #.#",
        'N' => "##. #.# #.# #.# #.#",
        'P' => "### #.# ### #.. #..",
        'Q' => "### #.# #.# ### ..#",
        'R' => "##. #.# ##. #.# #.#",
        'T' => "### .#. .#. .#. .#.",
        'U' => "#.# #.# #.# #.# ###",
        'V' => "#.# #.# #.# #.# .#.",
        'W' => "#.# #.# ### ### #.#",
        'X' => "#.# #.# .#. #.# #.#",
        'Y' => "#.# #.# .#. .#. .#.",
        'Z' => "### ..# .#. #.. ###",
        ':' => "... .#. ... .#. ...",
        '!' => ".#. .#. .#. ... .#.",
        '-' => "... ... ### ... ...",
        _ => "... ... ... ... ...",
    }
}

