//! Telão do clube: player de YouTube (ou arquivo local) via yt-dlp + ffmpeg.
//! Áudio vai em streaming pro mixer; vídeo vira textura sincronizada pelo relógio do áudio.

use crate::audio::{Audio, StreamBuf};
use macroquad::prelude::*;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::Duration;

pub const DEFAULT_URL: &str = "https://www.youtube.com/watch?v=dPaWD5E7xMM";
const VW: usize = 640;
const VH: usize = 360;
const FPS: f64 = 24.0;

enum Msg {
    Status(String),
    Title(String),
    Ended(u64),
}

pub struct Telao {
    pub tex: Texture2D,
    pub status: String,
    pub title: String,
    pub live: bool,
    source: String,
    frames: Option<Receiver<Vec<u8>>>,
    msg_rx: Receiver<Msg>,
    msg_tx: Sender<Msg>,
    shown: u64,
    sb: Arc<StreamBuf>,
    rate: u32,
    channels: usize,
    small: Option<Vec<[u8; 3]>>,
}

fn color_bars() -> Vec<u8> {
    let bars: [[u8; 3]; 7] = [[192, 192, 192], [192, 192, 0], [0, 192, 192], [0, 192, 0], [192, 0, 192], [192, 0, 0], [0, 0, 192]];
    let mut v = vec![255u8; VW * VH * 4];
    for y in 0..VH {
        for x in 0..VW {
            let c = bars[x * 7 / VW];
            let i = (y * VW + x) * 4;
            v[i..i + 3].copy_from_slice(&c);
        }
    }
    v
}

fn ffmpeg(input: &str, args: &[&str]) -> std::io::Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-loglevel", "error", "-nostdin"]);
    if input.starts_with("http") {
        cmd.args(["-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "5"]);
    }
    cmd.args(["-i", input]).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
}

/// (título, url do vídeo, url do áudio)
fn resolve(url: &str) -> Option<(String, String, String)> {
    let args = ["--no-playlist", "-f", "bv*[height<=360]+ba/b[height<=360]/b", "--print", "%(title)s", "-g", url];
    let out = Command::new("yt-dlp")
        .args(args)
        .stderr(Stdio::null())
        .output()
        .or_else(|_| Command::new("uvx").arg("yt-dlp").args(args).stderr(Stdio::null()).output())
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    match lines.as_slice() {
        [title, v, a, ..] => Some((title.to_string(), v.to_string(), a.to_string())),
        [title, va] => Some((title.to_string(), va.to_string(), va.to_string())),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn run(src: String, generation: u64, sb: Arc<StreamBuf>, ftx: SyncSender<Vec<u8>>, mtx: Sender<Msg>, rate: u32, ch: usize) {
    let alive = move |sb: &StreamBuf| sb.generation.load(Ordering::Relaxed) == generation;
    let status = |s: &str| {
        let _ = mtx.send(Msg::Status(s.to_string()));
    };
    let (video, audio) = if src.starts_with("http") {
        status("RESOLVENDO LINK DO YOUTUBE...");
        let Some((title, v, a)) = resolve(&src) else {
            status("FALHOU: instale yt-dlp (uv tool install yt-dlp)");
            return;
        };
        let _ = mtx.send(Msg::Title(title));
        (v, a)
    } else {
        let name = std::path::Path::new(&src).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let _ = mtx.send(Msg::Title(name));
        (src.clone(), src.clone())
    };
    if !alive(&sb) {
        return;
    }
    status("CARREGANDO...");

    let sbv = sb.clone();
    thread::spawn(move || {
        let vf = format!("fps={FPS},scale={VW}:{VH}:force_original_aspect_ratio=decrease,pad={VW}:{VH}:(ow-iw)/2:(oh-ih)/2");
        let Ok(mut child) = ffmpeg(&video, &["-an", "-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgba", "-"]) else {
            return;
        };
        let mut out = child.stdout.take().unwrap();
        let mut frame = vec![0u8; VW * VH * 4];
        while alive(&sbv) && out.read_exact(&mut frame).is_ok() {
            if ftx.send(frame.clone()).is_err() {
                break;
            }
        }
        let _ = child.kill();
    });

    let (chs, rates) = (ch.to_string(), rate.to_string());
    let Ok(mut child) = ffmpeg(&audio, &["-vn", "-ac", &chs, "-ar", &rates, "-f", "f32le", "-"]) else {
        status("FALHOU: ffmpeg nao encontrado no PATH");
        return;
    };
    let mut out = child.stdout.take().unwrap();
    let mut bytes = vec![0u8; 16384];
    let max = rate as usize * ch * 3;
    let mut started = false;
    let mut ended = false;
    while alive(&sb) {
        if sb.buf.lock().unwrap().len() > max {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        if out.read_exact(&mut bytes).is_err() {
            ended = true;
            break;
        }
        let mut buf = sb.buf.lock().unwrap();
        buf.extend(bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
        drop(buf);
        if !started {
            started = true;
            status("");
        }
    }
    let _ = child.kill();
    if ended && alive(&sb) {
        let _ = mtx.send(Msg::Ended(generation));
    }
}

impl Telao {
    pub fn new(audio: &Audio) -> Self {
        let (sb, rate, channels) = (audio.stream.clone(), audio.rate, audio.channels);
        let tex = Texture2D::from_rgba8(VW as u16, VH as u16, &color_bars());
        tex.set_filter(FilterMode::Linear);
        let (msg_tx, msg_rx) = mpsc::channel();
        Telao { tex, status: "SEM SINAL".into(), title: String::new(), live: false, source: String::new(), frames: None, msg_rx, msg_tx, shown: 0, sb, rate, channels, small: None }
    }

    /// Troca a fonte: link do YouTube (ou qualquer URL suportada pelo yt-dlp) ou caminho de arquivo.
    pub fn load(&mut self, src: &str) {
        let generation = self.sb.generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.sb.buf.lock().unwrap().clear();
        self.sb.played.store(0, Ordering::Relaxed);
        self.shown = 0;
        self.live = false;
        self.source = src.to_string();
        self.status = "CONECTANDO...".into();
        let (ftx, frx) = mpsc::sync_channel(48);
        self.frames = Some(frx);
        let (src, sb, mtx, rate, ch) = (src.to_string(), self.sb.clone(), self.msg_tx.clone(), self.rate, self.channels);
        thread::spawn(move || run(src, generation, sb, ftx, mtx, rate, ch));
    }

    /// Só sincroniza no navegador; no nativo o ffmpeg toca do começo.
    pub fn sync(&mut self, _elapsed: f64) {}

    pub fn set_volume(&mut self, v: f32) {
        self.sb.set_volume(v);
    }

    /// Só usado no navegador (iframe); no nativo o vídeo é textura.
    pub fn place(&self, _corners: Option<[Vec2; 4]>) {}

    pub fn update(&mut self) {
        while let Ok(m) = self.msg_rx.try_recv() {
            match m {
                Msg::Status(s) => self.status = s,
                Msg::Title(t) => self.title = t,
                Msg::Ended(g) if g == self.sb.generation.load(Ordering::Relaxed) => {
                    let src = self.source.clone();
                    self.load(&src);
                }
                Msg::Ended(_) => {}
            }
        }
        let played = self.sb.played.load(Ordering::Relaxed);
        self.live = played > 0;
        let target = (played as f64 / self.rate as f64 * FPS) as u64;
        let mut latest = None;
        if let Some(rx) = &self.frames {
            while self.shown < target {
                match rx.try_recv() {
                    Ok(f) => {
                        latest = Some(f);
                        self.shown += 1;
                    }
                    Err(_) => break,
                }
            }
        }
        if let Some(f) = latest {
            self.tex.update_from_bytes(VW as u32, VH as u32, &f);
            let px = (0..64).map(|k| {
                let i = ((VH / 16 + (k / 8) * VH / 8) * VW + VW / 16 + (k % 8) * VW / 8) * 4;
                [f[i], f[i + 1], f[i + 2]]
            });
            self.small = Some(px.collect());
        }
    }

    /// Amostra 8x8 do último quadro novo (pras luzes do clube).
    pub fn sample(&mut self) -> Option<Vec<[u8; 3]>> {
        self.small.take()
    }
}
