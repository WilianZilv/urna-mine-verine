//! Profiler de frame (F3 ou ?debug=1): ms por seção, draw calls, vértices, cubos e textos.

use macroquad::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub static CALLS: AtomicUsize = AtomicUsize::new(0);
pub static VERTS: AtomicUsize = AtomicUsize::new(0);
pub static CUBES: AtomicUsize = AtomicUsize::new(0);
pub static CHUNKS: AtomicUsize = AtomicUsize::new(0);
pub static TEXTS: AtomicUsize = AtomicUsize::new(0);
const COUNTERS: [&AtomicUsize; 5] = [&CALLS, &VERTS, &CUBES, &CHUNKS, &TEXTS];
#[cfg(target_arch = "wasm32")]
const COUNTER_NAMES: [&str; 5] = ["calls", "verts", "cubes", "chunks", "texts"];

pub const NET: usize = 0;
pub const INPUT: usize = 1;
pub const PLAYER: usize = 2;
pub const NPC: usize = 3;
pub const MISC: usize = 4;
pub const LAB_RT: usize = 5;
pub const WORLD: usize = 6;
pub const ACTORS: usize = 7;
pub const PORTALS: usize = 8;
pub const FLUSH: usize = 9;
pub const LABELS: usize = 10;
pub const UI: usize = 11;
pub const PRESENT: usize = 12;
const N: usize = 13;
const NAMES: [&str; N] = ["net+json", "input", "player", "npc sims", "fx/remesh", "lab rt", "world", "actors", "portals", "flush", "labels", "ui", "present"];

pub fn add(c: &AtomicUsize, n: usize) {
    c.fetch_add(n, Relaxed);
}

fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    return unsafe { crate::web::urna_now() };
    #[cfg(not(target_arch = "wasm32"))]
    return macroquad::miniquad::date::now() * 1000.0;
}

#[derive(Default)]
pub struct Prof {
    pub on: bool,
    last: f64,
    acc: [f64; N],
    cnt: [usize; 5],
    frames: u32,
    window: f64,
    avg: [f32; N],
    avg_cnt: [usize; 5],
    fps: f32,
}

impl Prof {
    pub fn toggle(&mut self) {
        self.on = !self.on;
        self.window = 0.0;
    }

    pub fn mark(&mut self, s: usize) {
        if self.on {
            let t = now_ms();
            self.acc[s] += t - self.last;
            self.last = t;
        }
    }

    /// Fecha o frame (chamar no topo do loop). A cada 1s calcula médias.
    pub fn frame(&mut self, extra: &str) {
        let c = COUNTERS.map(|c| c.swap(0, Relaxed));
        if !self.on {
            return;
        }
        self.mark(PRESENT);
        if self.window == 0.0 {
            self.window = self.last;
            self.acc = [0.0; N];
            self.cnt = [0; 5];
            return;
        }
        self.frames += 1;
        for (a, v) in self.cnt.iter_mut().zip(c) {
            *a += v;
        }
        let span = self.last - self.window;
        if span < 1000.0 {
            return;
        }
        let f = self.frames as f64;
        self.fps = (f * 1000.0 / span) as f32;
        for k in 0..N {
            self.avg[k] = (self.acc[k] / f) as f32;
        }
        for k in 0..5 {
            self.avg_cnt[k] = self.cnt[k] / self.frames as usize;
        }
        self.acc = [0.0; N];
        self.cnt = [0; 5];
        self.frames = 0;
        self.window = self.last;
        #[cfg(target_arch = "wasm32")]
        {
            let secs: Vec<String> = NAMES.iter().zip(self.avg).map(|(n, v)| format!("\"{n}\":{v:.2}")).collect();
            let cnts: Vec<String> = COUNTER_NAMES.iter().zip(self.avg_cnt).map(|(n, v)| format!("\"{n}\":{v}")).collect();
            let s = format!("{{\"fps\":{:.1},{},\"w\":{},\"h\":{},\"extra\":\"{extra}\",\"ms\":{{{}}}}}", self.fps, cnts.join(","), screen_width(), screen_height(), secs.join(","));
            unsafe { crate::web::urna_prof(s.as_ptr(), s.len()) };
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = extra;
    }

    pub fn draw(&self, extra: &str) {
        if !self.on {
            return;
        }
        let lines = N + 3;
        draw_rectangle(4.0, 4.0, 240.0, 16.0 * lines as f32 + 8.0, Color::new(0.0, 0.0, 0.0, 0.6));
        let mut y = 18.0;
        let mut line = |s: &str| {
            draw_text(s, 10.0, y, 16.0, WHITE);
            y += 16.0;
        };
        let c = &self.avg_cnt;
        line(&format!("{:.1} FPS  {extra}", self.fps));
        line(&format!("calls {}  chunks {}  texts {}", c[0], c[3], c[4]));
        line(&format!("verts {}k  cubes {}", c[1] / 1000, c[2]));
        for (n, v) in NAMES.iter().zip(self.avg) {
            line(&format!("{n:>10} {v:6.2} ms"));
        }
    }
}
