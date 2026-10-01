//! Sintetizador: loop de house 124 BPM e efeitos sonoros, gerados em código na taxa do dispositivo.

use std::f32::consts::TAU;

pub const BPM: f32 = 124.0;

struct Rng(u32);
impl Rng {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn saw(x: f32) -> f32 {
    2.0 * (x - (x + 0.5).floor())
}

fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// 8 compassos de deep house: kick 4x4, clap 2/4, hats no contratempo,
/// baixo offbeat, stabs de acorde (Am7 - Fmaj7 - Dm7 - Em7), pad e sidechain.
pub fn house_loop(sr: u32) -> Vec<f32> {
    let spb = 60.0 / BPM;
    let step = spb / 4.0;
    let bars = 8usize;
    let n = (bars as f32 * 4.0 * spb * sr as f32) as usize;
    let chords: [[f32; 4]; 4] = [
        [220.00, 261.63, 329.63, 392.00],
        [174.61, 220.00, 261.63, 329.63],
        [146.83, 174.61, 220.00, 261.63],
        [164.81, 196.00, 246.94, 293.66],
    ];
    let roots = [110.0, 87.31, 73.42, 82.41];
    let stabs = [3usize, 6, 11, 14];
    let chord_len = 2.0 * 4.0 * spb;

    let mut rng = Rng(0x1234_5678);
    let (mut c1, mut c2, mut hlp, mut slp, mut blp) = (0f32, 0f32, 0f32, 0f32, 0f32);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let bi = (t / spb) as usize;
        let tb = t - bi as f32 * spb;
        let si = (t / step) as usize;
        let ts = t - si as f32 * step;
        let p16 = si % 16;
        let bar = bi / 4;
        let ch = (bar / 2) % 4;
        let noise = rng.f();

        // Kick
        let kph = TAU * (48.0 * tb + 130.0 * 0.03 * (1.0 - (-tb / 0.03).exp()));
        let kick = kph.sin() * (-tb / 0.24).exp() + noise * 0.15 * (-tb / 0.003).exp();

        // Clap (2 e 4)
        c1 += (noise - c1) * 0.45;
        c2 += (c1 - c2) * 0.06;
        let bp = c1 - c2;
        let clap = if bi % 2 == 1 {
            let mut e = 0.0;
            for k in 0..3 {
                let o = k as f32 * 0.011;
                if tb >= o {
                    e += (-(tb - o) / 0.005).exp() * 0.6;
                }
            }
            if tb > 0.022 {
                e += (-(tb - 0.022) / 0.13).exp() * 0.8;
            }
            bp * e
        } else {
            0.0
        };

        // Hats
        hlp += (noise - hlp) * 0.6;
        let hp = noise - hlp;
        let chat = if p16 % 4 != 0 {
            hp * (-ts / 0.018).exp() * if p16 % 2 == 1 { 0.6 } else { 1.0 }
        } else {
            0.0
        };
        let ohat = if p16 % 4 == 2 { hp * (-ts / 0.09).exp() } else { 0.0 };

        // Baixo no contratempo
        let bt = match p16 % 4 {
            2 => Some(ts),
            3 => Some(ts + step),
            _ => None,
        };
        let bass = if let Some(bt) = bt {
            let env = (1.0 - (-bt / 0.004).exp()) * (-bt / 0.16).exp();
            let ph = TAU * roots[ch] * bt;
            (ph.sin() * 2.2 + (2.0 * ph).sin() * 0.4).tanh() * env
        } else {
            0.0
        };
        blp += (bass - blp) * 0.25;

        // Stabs de acorde
        let mut last = None;
        for &s in &stabs {
            if s <= p16 {
                last = Some(s);
            }
        }
        let st = match last {
            Some(s) => (p16 - s) as f32 * step + ts,
            None => (p16 + 16 - stabs[3]) as f32 * step + ts,
        };
        let senv = (1.0 - (-st / 0.003).exp()) * (-st / 0.15).exp();
        let mut raw = 0.0;
        for &f in &chords[ch] {
            raw += saw(f * t) + saw(f * 1.006 * t) * 0.7;
        }
        raw /= 6.0;
        let sweep = 0.75 + 0.25 * (TAU * t / (bars as f32 * 4.0 * spb)).sin();
        slp += (raw - slp) * (0.04 + 0.35 * senv) * sweep;
        let stab = slp * senv;

        // Pad
        let tc = t % chord_len;
        let pad_env = (tc / 0.08).min(1.0) * ((chord_len - tc) / 0.08).min(1.0);
        let mut pad = 0.0;
        for &f in &chords[ch] {
            pad += (TAU * f * 0.5 * t).sin() + 0.3 * (TAU * f * 1.003 * t).sin();
        }
        pad *= pad_env / 4.0;

        let duck = 0.3 + 0.7 * smooth(tb / 0.22);
        let mix = 0.95 * kick + 0.45 * clap + 0.10 * chat + 0.20 * ohat + duck * (0.55 * blp + 0.5 * stab + 0.10 * pad);
        out.push((mix * 0.95).tanh() * 0.9);
    }
    out
}

fn render(sr: u32, secs: f32, mut f: impl FnMut(f32, &mut Rng) -> f32) -> Vec<f32> {
    let n = (secs * sr as f32) as usize;
    let mut rng = Rng(0xBEEF_1234);
    (0..n).map(|i| f(i as f32 / sr as f32, &mut rng)).collect()
}

pub fn boom(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    render(sr, 1.6, move |t, r| {
        lp += (r.f() - lp) * 0.08;
        let thump = (TAU * (35.0 + 60.0 * (-t / 0.08).exp()) * t).sin() * (-t / 0.3).exp();
        ((lp * 3.0 * (-t / 0.45).exp() + thump) * 1.5).tanh() * 0.9
    })
}

pub fn gun(sr: u32) -> Vec<f32> {
    let mut lp = 0.0;
    render(sr, 0.35, move |t, r| {
        let n = r.f();
        lp += (n - lp) * 0.25;
        let crack = (n - lp * 0.5) * (-t / 0.012).exp();
        let body = lp * 2.5 * (-t / 0.09).exp();
        let thump = (TAU * (90.0 + 120.0 * (-t / 0.01).exp()) * t).sin() * (-t / 0.04).exp();
        ((crack + body + thump) * 1.4).tanh() * 0.8
    })
}

/// Loop de motor (4 cilindros) com ciclo inteiro pra emendar sem clique. Vários tons cruzados = rotação.
pub fn engine(sr: u32, hz: f32) -> Vec<f32> {
    let cycles = (hz * 0.5).round().max(1.0);
    let n = (cycles / hz * sr as f32) as usize;
    let hz = cycles * sr as f32 / n as f32;
    let mut lp = 0.0;
    let out: Vec<f32> = (0..2 * n)
        .map(|i| {
            let ph = (hz * (i % n) as f32 / sr as f32).fract();
            let pulse = (-ph * 9.0).exp() * 2.0 - 0.35;
            let rumble = (TAU * ph).sin() * 0.5 + (TAU * 2.0 * ph).sin() * 0.3;
            lp += (pulse + rumble - lp) * 0.3;
            (lp * 1.3).tanh() * 0.6
        })
        .collect();
    out[n..].to_vec()
}

pub fn laser(sr: u32) -> Vec<f32> {
    let mut ph = 0.0;
    render(sr, 0.45, move |t, _| {
        let f = 200.0 + 1600.0 * (-t * 7.0).exp() + (t * 90.0).sin() * 40.0;
        ph += f / sr as f32;
        let sq = if (ph * 0.5).fract() < 0.5 { 1.0 } else { -1.0 };
        (saw(ph) * 0.6 + sq * 0.25) * (-t / 0.18).exp() * (1.0 - (-t / 0.005).exp()) * 0.7
    })
}

pub fn punch(sr: u32) -> Vec<f32> {
    let (mut lp, mut lp2) = (0.0, 0.0);
    render(sr, 0.32, move |t, r| {
        lp += (r.f() - lp) * 0.06;
        lp2 += (lp - lp2) * 0.06;
        let thud = (TAU * (42.0 + 70.0 * (-t / 0.03).exp()) * t).sin() * (-t / 0.12).exp();
        let sub = (TAU * 32.0 * t).sin() * (-t / 0.18).exp();
        (lp2 * 6.0 * (-t / 0.04).exp() + thud * 1.3 + sub * 0.6).tanh() * 0.95
    })
}

pub fn slash(sr: u32) -> Vec<f32> {
    let (mut lp, mut lp2) = (0.0, 0.0);
    render(sr, 0.32, move |t, r| {
        let n = r.f();
        lp += (n - lp) * 0.12;
        lp2 += (lp - lp2) * 0.12;
        let swoosh = lp2 * 4.0 * (-t / 0.06).exp();
        let ring = ((TAU * 620.0 * t).sin() + (TAU * 930.0 * t).sin() * 0.5) * (-t / 0.07).exp();
        let thud = (TAU * (48.0 + 90.0 * (-t / 0.025).exp()) * t).sin() * (-t / 0.1).exp();
        (swoosh + ring * 0.15 + thud * 1.2).tanh() * 0.9
    })
}

pub fn snikt(sr: u32) -> Vec<f32> {
    render(sr, 0.55, |t, r| {
        let mut s = 0.0;
        for &o in &[0.0f32, 0.09] {
            if t >= o {
                let tt = t - o;
                let ring = (TAU * 2900.0 * tt).sin() + (TAU * 4300.0 * tt).sin() * 0.8 + (TAU * 5800.0 * tt).sin() * 0.6 + (TAU * 7700.0 * tt).sin() * 0.4;
                s += ring * (-tt / 0.12).exp() * 0.25 + r.f() * (-tt / 0.02).exp() * 0.4;
            }
        }
        s.clamp(-1.0, 1.0)
    })
}

pub fn deflect(sr: u32) -> Vec<f32> {
    render(sr, 0.7, |t, r| {
        let trem = 0.7 + 0.3 * (TAU * 18.0 * t).sin();
        let s = (TAU * 880.0 * t).sin() + (TAU * 1320.0 * t).sin() * 0.6 + (TAU * 1760.0 * t).sin() * 0.4;
        (s * 0.3 * trem * (-t / 0.25).exp() + r.f() * 0.15 * (-t / 0.05).exp()).clamp(-1.0, 1.0)
    })
}
