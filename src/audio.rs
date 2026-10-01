//! Mixer próprio sobre cpal: vozes (efeitos/house sintetizado) + stream PCM do telão.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub type Clip = Arc<Vec<f32>>;

struct Voice {
    id: u64,
    data: Clip,
    pos: usize,
    vol: f32,
    looped: bool,
}

#[derive(Default)]
struct Mix {
    voices: Vec<Voice>,
    next_id: u64,
}

/// PCM intercalado no formato do dispositivo, alimentado por ffmpeg.
pub struct StreamBuf {
    pub buf: Mutex<VecDeque<f32>>,
    vol: AtomicU32,
    /// Frames de áudio já tocados (relógio para sincronizar o vídeo).
    pub played: AtomicU64,
    /// Incrementado a cada troca de fonte; threads antigas param.
    pub generation: AtomicU64,
}

impl StreamBuf {
    pub fn set_volume(&self, v: f32) {
        self.vol.store(v.to_bits(), Ordering::Relaxed);
    }
}

pub struct Audio {
    _stream: Option<cpal::Stream>,
    pub rate: u32,
    pub channels: usize,
    mix: Arc<Mutex<Mix>>,
    pub stream: Arc<StreamBuf>,
}

fn render(out: &mut [f32], ch: usize, mix: &Mutex<Mix>, sb: &StreamBuf) {
    let mut m = mix.lock().unwrap();
    let mut buf = sb.buf.lock().unwrap();
    let svol = f32::from_bits(sb.vol.load(Ordering::Relaxed));
    let mut played = 0u64;
    for frame in out.chunks_mut(ch) {
        let mut s = 0.0;
        for v in m.voices.iter_mut() {
            if v.pos >= v.data.len() {
                if !v.looped {
                    continue;
                }
                v.pos = 0;
            }
            s += v.data[v.pos] * v.vol;
            v.pos += 1;
        }
        let have = buf.len() >= ch;
        if have {
            played += 1;
        }
        for o in frame.iter_mut() {
            let st = if have { buf.pop_front().unwrap_or(0.0) * svol } else { 0.0 };
            *o = (s + st).clamp(-1.0, 1.0);
        }
    }
    m.voices.retain(|v| v.looped || v.pos < v.data.len());
    sb.played.fetch_add(played, Ordering::Relaxed);
}

impl Audio {
    pub fn new() -> Self {
        let mix = Arc::new(Mutex::new(Mix::default()));
        let stream = Arc::new(StreamBuf {
            buf: Mutex::new(VecDeque::new()),
            vol: AtomicU32::new(1.0f32.to_bits()),
            played: AtomicU64::new(0),
            generation: AtomicU64::new(0),
        });
        let mut a = Audio { _stream: None, rate: 48000, channels: 2, mix: mix.clone(), stream: stream.clone() };
        let Some(dev) = cpal::default_host().default_output_device() else {
            eprintln!("audio: sem dispositivo de saida");
            return a;
        };
        let Ok(sup) = dev.default_output_config() else {
            eprintln!("audio: sem config padrao");
            return a;
        };
        let cfg = sup.config();
        let ch = cfg.channels as usize;
        a.rate = cfg.sample_rate;
        a.channels = ch;
        let err = |e| eprintln!("audio: {e}");
        let built = match sup.sample_format() {
            cpal::SampleFormat::I16 => {
                let mut tmp = Vec::new();
                dev.build_output_stream::<i16, _, _>(
                    cfg,
                    move |out, _| {
                        tmp.resize(out.len(), 0.0);
                        render(&mut tmp, ch, &mix, &stream);
                        for (o, s) in out.iter_mut().zip(&tmp) {
                            *o = (s * 32767.0) as i16;
                        }
                    },
                    err,
                    None,
                )
            }
            _ => dev.build_output_stream::<f32, _, _>(cfg, move |out, _| render(out, ch, &mix, &stream), err, None),
        };
        match built {
            Ok(s) => {
                if let Err(e) = s.play() {
                    eprintln!("audio: {e}");
                }
                a._stream = Some(s);
            }
            Err(e) => eprintln!("audio: {e}"),
        }
        a
    }

    pub fn play(&self, data: &Clip, vol: f32, looped: bool) -> u64 {
        let mut m = self.mix.lock().unwrap();
        m.next_id += 1;
        let id = m.next_id;
        m.voices.push(Voice { id, data: data.clone(), pos: 0, vol, looped });
        id
    }

    pub fn set_volume(&self, id: u64, vol: f32) {
        if let Some(v) = self.mix.lock().unwrap().voices.iter_mut().find(|v| v.id == id) {
            v.vol = vol;
        }
    }
}
