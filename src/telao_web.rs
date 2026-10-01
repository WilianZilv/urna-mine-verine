//! Telão no navegador: iframe do YouTube projetado por cima do canvas (web/urna.js).

use crate::audio::Audio;
use crate::web::*;
use macroquad::prelude::*;

pub const DEFAULT_URL: &str = "https://www.youtube.com/watch?v=dPaWD5E7xMM";

pub struct Telao {
    pub tex: Texture2D,
    pub status: String,
    pub title: String,
    pub live: bool,
    vol: f32,
}

impl Telao {
    pub fn new(_audio: &Audio) -> Self {
        let tex = Texture2D::from_rgba8(1, 1, &[5, 5, 8, 255]);
        Telao { tex, status: "CLIQUE NA TELA PRA LIGAR O TELAO".into(), title: String::new(), live: false, vol: 1.0 }
    }

    pub fn load(&mut self, src: &str) {
        unsafe { urna_yt_load(src.as_ptr(), src.len()) };
        self.live = false;
    }

    /// Linha do tempo compartilhada: o vídeo começou `elapsed` segundos atrás no servidor.
    pub fn sync(&mut self, elapsed: f64) {
        unsafe { urna_yt_sync(elapsed) };
    }

    pub fn set_volume(&mut self, v: f32) {
        self.vol = v;
    }

    pub fn update(&mut self) {
        self.live = unsafe { urna_yt_state() } == 1;
        self.status = if self.live { String::new() } else { "CLIQUE NA TELA PRA LIGAR O TELAO".into() };
        if let Some(t) = read_string(|p, c| unsafe { urna_yt_title(p, c) }) {
            self.title = t;
        }
    }

    /// Amostra 8x8 das cores do vídeo (iframe não deixa ler pixel: o JS mistura as miniaturas do YouTube pela posição do vídeo).
    pub fn sample(&mut self) -> Option<Vec<[u8; 3]>> {
        let mut buf = [0u8; 192];
        let n = unsafe { urna_yt_colors(buf.as_mut_ptr(), buf.len()) };
        (n == 192).then(|| buf.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
    }

    /// Cantos do telão na tela (topo-esq, topo-dir, baixo-dir, baixo-esq).
    pub fn place(&self, corners: Option<[Vec2; 4]>) {
        let c = corners.unwrap_or([Vec2::ZERO; 4]);
        unsafe { urna_yt_place(c[0].x, c[0].y, c[1].x, c[1].y, c[2].x, c[2].y, c[3].x, c[3].y, corners.is_some() as i32, self.vol) };
    }
}
