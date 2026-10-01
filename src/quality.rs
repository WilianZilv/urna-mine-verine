//! Qualidade gráfica BAIXA/MEDIA/ALTA. AUTO: celular começa em BAIXA, PC em ALTA e cai um nível se ficar
//! 3s abaixo de 25 FPS. Manual: ?q=low|med|high, F4 ou botão no menu de personagens (salvo no navegador).

use std::sync::atomic::{AtomicU8, Ordering::Relaxed};

pub const LOW: u8 = 0;
pub const HIGH: u8 = 2;
static TIER: AtomicU8 = AtomicU8::new(HIGH);

pub fn tier() -> u8 {
    TIER.load(Relaxed)
}

/// Escolhe por nível: [baixa, média, alta].
pub fn pick<T: Copy>(v: [T; 3]) -> T {
    v[tier() as usize]
}

const KEYS: [&str; 3] = ["low", "med", "high"];
const NAMES: [&str; 3] = ["BAIXA", "MEDIA", "ALTA"];

pub struct Quality {
    pub auto: bool,
    slow: f32,
    warmup: f32,
    ema: f32,
}

fn saved() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::query("q").or_else(|| crate::web::store_get("urna_q"));
    #[cfg(not(target_arch = "wasm32"))]
    return std::env::var("URNA_Q").ok();
}

impl Quality {
    pub fn new(mobile: bool) -> Quality {
        let fixed = saved().and_then(|s| KEYS.iter().position(|k| *k == s));
        TIER.store(fixed.map(|t| t as u8).unwrap_or(if mobile { LOW } else { HIGH }), Relaxed);
        Quality { auto: fixed.is_none(), slow: 0.0, warmup: 5.0, ema: 1.0 / 60.0 }
    }

    pub fn label(&self) -> String {
        format!("QUALIDADE: {}{}", NAMES[tier() as usize], if self.auto { " (AUTO)" } else { "" })
    }

    /// AUTO: 3s seguidos abaixo de 25 FPS derruba um nível. Retorna aviso quando muda.
    pub fn update(&mut self, dt: f32) -> Option<String> {
        if !self.auto || tier() == LOW {
            return None;
        }
        if self.warmup > 0.0 {
            self.warmup -= dt;
            return None;
        }
        self.ema += (dt - self.ema) * 0.1;
        self.slow = if self.ema > 1.0 / 25.0 { self.slow + dt } else { 0.0 };
        if self.slow < 3.0 {
            return None;
        }
        self.slow = 0.0;
        self.ema = 1.0 / 60.0;
        TIER.store(tier() - 1, Relaxed);
        Some(format!("{} - FPS BAIXO", self.label()))
    }

    /// AUTO -> BAIXA -> MEDIA -> ALTA -> AUTO.
    pub fn cycle(&mut self, mobile: bool) -> String {
        let key = if self.auto {
            self.auto = false;
            TIER.store(LOW, Relaxed);
            KEYS[0]
        } else if tier() < HIGH {
            TIER.store(tier() + 1, Relaxed);
            KEYS[tier() as usize]
        } else {
            self.auto = true;
            self.warmup = 3.0;
            TIER.store(if mobile { LOW } else { HIGH }, Relaxed);
            "auto"
        };
        #[cfg(target_arch = "wasm32")]
        crate::web::store_set("urna_q", key);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = key;
        self.label()
    }
}
