//! Congresso da Vila: jogadores votam leis que mudam o jogo por alguns minutos (server/congresso.js).
//! Prédio: laje comprida com torres gêmeas, cúpula e cuia no teto (oeste) e plenário aberto pra avenida
//! (leste) com 5 púlpitos (um por lei: pisa 1,5 s pra votar) e o painel holográfico virado pro leste.

use super::{Geo, Laws, Place, Screen, TW, clear_lot, fill, hash_str, text_mid, wrap};
use crate::batch::Batch;
use crate::extras::Label;
use crate::lab::panel::{CYAN, DIM, GOLD, PINK, SOFT, fit, frame, frame_at, holo_fx, text_right};
use crate::models::rgb;
use crate::player::Player;
use crate::world::{AIR, BLACK, COBBLE, G, GLASS, GRASS, STONE, WOOL, World};
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::f32::consts::{PI, TAU};

/// (id, nome, efeito, cor) na mesma ordem do servidor.
const LAWS: [(&str, &str, &str, Color); 5] = [
    ("lua", "GRAVIDADE LUNAR", "pulo de astronauta", Color::new(0.6, 0.75, 1.0, 1.0)),
    ("turbo", "TURBO NACIONAL", "todo mundo 60% mais rapido", Color::new(1.0, 0.55, 0.15, 1.0)),
    ("paz", "URNA PACIFISTA", "a urna gigante nao atira", Color::new(0.45, 1.0, 0.6, 1.0)),
    ("festa", "FESTA OBRIGATORIA", "fogos no mapa todo", Color::new(1.0, 0.4, 0.9, 1.0)),
    ("promo", "LOJA EM PROMOCAO", "loja pela metade do preco", Color::new(1.0, 0.85, 0.3, 1.0)),
];
/// Centro (bloco) de cada púlpito 3x3, em arco virado pro painel.
const PADS: [(i32, i32); 5] = [(81, 90), (80, 94), (79, 98), (80, 102), (81, 106)];
const HOLD: f32 = 1.5;
/// Pé do painel (tela virada pra +x, pra avenida).
const PX: f32 = 71.0;
const PZ: f32 = 98.5;
const PW: f32 = 20.0;
const PH: f32 = 10.0;
const PY: f32 = G as f32 + 8.5;
/// Emblema voxel de cada lei (mesma ordem de LAWS) flutuando sobre a praça: lua, raio, bandeira branca,
/// globo de espelhos, etiqueta de preço.
const ICONS: [&[&str]; 5] = [
    &["..####.", ".###...", "###....", "###....", "###....", ".###...", "..####."],
    &["....##.", "...##..", "..##...", ".#####.", "...##..", "..##...", ".##...."],
    &["#......", "#####..", "######.", "#####..", "#......", "#......", "#......"],
    &["..###..", ".#####.", "#######", "#######", "#######", ".#####.", "..###.."],
    &["..#####", ".######", "###.###", "#######", ".######", "..#####"],
];

fn geo() -> Geo {
    Geo { c: vec3(PX, PY, PZ), n: Vec3::X, w: PW, h: PH }
}

fn center() -> Vec3 {
    let r = crate::layout::CONGRESSO;
    vec3((r.0 + r.2) as f32 * 0.5, G as f32, (r.1 + r.3) as f32 * 0.5)
}

/// Pseudo-aleatório [0,1) determinístico (partícula k, semente s).
fn rnd(k: u32, s: u32) -> f32 {
    let h = (k.wrapping_mul(2654435761) ^ s.wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B);
    (h >> 8) as f32 / 16_777_216.0
}

fn mmss(s: f32) -> String {
    let s = s.ceil().max(0.0) as i32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Mod no concurso: nome, criador, votos.
#[derive(Default, Clone, PartialEq)]
struct Entry {
    n: String,
    c: String,
    vo: u32,
}

impl Entry {
    fn from(v: &Value) -> Self {
        Entry { n: super::s(&v["n"]), c: super::s(&v["c"]), vo: v["vo"].as_u64().unwrap_or(0) as u32 }
    }
}

/// Concurso semanal de mods (mds no snapshot "lei").
#[derive(Default)]
struct Contest {
    week: String,
    /// Segundos até a virada, na hora do snapshot.
    left: f32,
    n: u32,
    tot: u32,
    top: Vec<Entry>,
    winner: Option<(String, Entry)>,
    hist: Vec<(String, Entry)>,
}

#[derive(Default, Clone, Copy)]
struct Vote {
    v: u32,
    /// Segundos em vigor / até acabar o recesso, na hora que chegou o snapshot.
    on: f32,
    cd: f32,
}

pub struct Congresso {
    st: [Vote; 5],
    q: u32,
    passed: u32,
    last: String,
    got: bool,
    live: bool,
    recv: f64,
    flash_at: f64,
    pad: Option<usize>,
    hold: f32,
    sent: Option<usize>,
    cool: f32,
    screen: Screen,
    /// get_time() em que cada lei entrou em vigor (pulso do emblema).
    pulse: [f64; 5],
    /// Confete sobre o plenário quando uma lei passa.
    burst: f64,
    burst_col: Color,
    mds: Contest,
}

impl Congresso {
    pub fn new() -> Self {
        Congresso { st: [Vote::default(); 5], q: 1, passed: 0, last: String::new(), got: false, live: false, recv: 0.0, flash_at: -100.0, pad: None, hold: 0.0, sent: None, cool: 0.0, screen: Screen::new(), pulse: [-100.0; 5], burst: -100.0, burst_col: WHITE, mds: Contest::default() }
    }

    /// Página do painel: 0 = leis, 1 = concurso de mods (alterna a cada 12 s).
    fn page(&self, time: f32) -> u32 {
        if self.got && self.live { (time / 12.0) as u32 % 2 } else { 0 }
    }

    fn paint_mods(&self, time: f32) {
        let m = &self.mds;
        draw_text("CONCURSO DE MODS", 40.0, 92.0, 84.0, GOLD);
        let left = (m.left - (get_time() - self.recv) as f32).max(0.0);
        let s = left as i32;
        let when = if s >= 86400 { format!("FALTAM {} DIA(S) {}H", s / 86400, s % 86400 / 3600) } else { format!("FALTAM {}H{:02}", s / 3600, s % 3600 / 60) };
        text_right(&format!("SEMANA {}  -  {when}", m.week), TW - 40.0, 70.0, 44.0, CYAN);
        text_right(&format!("{} mod(s) na disputa  -  {} voto(s)", m.n, m.tot), TW - 40.0, 108.0, 30.0, SOFT);
        draw_rectangle(40.0, 126.0, TW - 80.0, 4.0, Color::new(1.0, 0.85, 0.3, 0.7));

        frame(40.0, 146.0, TW - 80.0, 430.0, "");
        if m.top.is_empty() {
            let blink = (time * 2.0).fract() < 0.7;
            text_mid("NENHUM MOD ATIVO NA VILA", TW * 0.5, 330.0, 72.0, if blink { PINK } else { SOFT });
            text_mid("sem candidato, sem concurso. sobe um mod (/modding.txt)", TW * 0.5, 410.0, 40.0, SOFT);
        }
        let (xb, bw) = (980.0, 700.0);
        let max = m.top.iter().map(|e| e.vo).max().unwrap_or(0).max(1);
        let medal = [GOLD, Color::new(0.8, 0.85, 0.9, 1.0), Color::new(0.85, 0.5, 0.25, 1.0)];
        for (i, e) in m.top.iter().enumerate() {
            let y = 160.0 + i as f32 * 136.0;
            draw_rectangle(56.0, y + 4.0, TW - 112.0, 126.0, Color::new(0.3, 0.22, 0.05, if i == 0 { 0.45 } else { 0.22 }));
            draw_rectangle(56.0, y + 4.0, 12.0, 126.0, medal[i]);
            draw_text(&format!("{}.", i + 1), 90.0, y + 84.0, 80.0, medal[i]);
            draw_text(&fit(&e.n.to_uppercase(), xb - 260.0, 60.0), 190.0, y + 64.0, 60.0, if i == 0 { GOLD } else { WHITE });
            draw_text(&fit(&format!("de {}", e.c), xb - 260.0, 34.0), 190.0, y + 108.0, 34.0, SOFT);
            let k = e.vo as f32 / max as f32;
            draw_rectangle(xb, y + 40.0, bw, 50.0, Color::new(1.0, 0.85, 0.3, 0.1));
            draw_rectangle(xb, y + 40.0, bw * k, 50.0, Color::new(medal[i].r, medal[i].g, medal[i].b, 0.85));
            draw_rectangle_lines(xb, y + 40.0, bw, 50.0, 2.0, DIM);
            text_right(&format!("{} voto(s)", e.vo), TW - 70.0, y + 82.0, 46.0, WHITE);
        }

        frame(40.0, 592.0, TW - 80.0, 350.0, "HALL DA FAMA");
        match &m.winner {
            Some((wk, w)) => {
                draw_text(&fit(&format!("MOD DA SEMANA ({wk}): {} de {}", w.n.to_uppercase(), w.c), TW - 140.0, 56.0), 70.0, 680.0, 56.0, GOLD);
                for (j, (wk, h)) in m.hist.iter().skip(1).take(4).enumerate() {
                    draw_text(&fit(&format!("{wk}: {} de {} ({} voto(s))", h.n, h.c, h.vo), TW - 160.0, 38.0), 90.0, 752.0 + j as f32 * 46.0, 38.0, SOFT);
                }
            }
            None => {
                draw_text("nenhum campeao ainda. o primeiro MOD DA SEMANA sai na virada de segunda (UTC)", 70.0, 690.0, 40.0, SOFT);
            }
        }
        draw_text("/concurso   /votarmod numero ou nome", 40.0, 1000.0, 40.0, GOLD);
        text_right("1 voto por semana, pode trocar", TW - 40.0, 1000.0, 30.0, SOFT);
        holo_fx(time, self.flash());
    }

    /// (em vigor, recesso) restantes agora, descontando o tempo desde o snapshot.
    fn left(&self, i: usize) -> (f32, f32) {
        let e = (get_time() - self.recv) as f32;
        let v = &self.st[i];
        let on = (v.on - e).max(0.0);
        (on, if on > 0.0 { 0.0 } else { (v.cd - e).max(0.0) })
    }

    pub fn laws(&self) -> Laws {
        if !(self.got && self.live) {
            return Laws::default();
        }
        let on = |i: usize| self.left(i).0 > 0.0;
        Laws { lua: on(0), turbo: on(1), paz: on(2), festa: on(3) }
    }

    fn status(&self, i: usize) -> (String, Color) {
        if !(self.got && self.live) {
            return ("CONECTANDO...".into(), SOFT);
        }
        match self.left(i) {
            (on, _) if on > 0.0 => (format!("EM VIGOR {}", mmss(on)), GOLD),
            (_, cd) if cd > 0.0 => (format!("RECESSO {}", mmss(cd)), PINK),
            _ => ("EM VOTACAO".into(), CYAN),
        }
    }

    fn flash(&self) -> f32 {
        (1.0 - (get_time() - self.flash_at) as f32 / 2.5).max(0.0)
    }

    fn paint(&self, time: f32) {
        let live = self.got && self.live;
        draw_text("CONGRESSO DA VILA", 40.0, 92.0, 84.0, CYAN);
        let info = if live { format!("QUORUM {} VOTO(S)  -  {} LEI(S) APROVADA(S)", self.q, self.passed) } else { "CONECTANDO...".into() };
        text_right(&info, TW - 40.0, 70.0, 44.0, GOLD);
        text_right("a vila vota, o jogo obedece (por 5 minutos)", TW - 40.0, 108.0, 30.0, SOFT);
        draw_rectangle(40.0, 126.0, TW - 80.0, 4.0, Color::new(0.45, 1.0, 1.0, 0.7));

        frame(40.0, 146.0, TW - 80.0, 604.0, "");
        let (xb, bw) = (1010.0, 480.0);
        draw_text("LEI", 90.0, 186.0, 30.0, SOFT);
        draw_text("VOTOS / QUORUM", xb, 186.0, 30.0, SOFT);
        text_right("SITUACAO", TW - 70.0, 186.0, 30.0, SOFT);
        for (i, &(id, n, d, col)) in LAWS.iter().enumerate() {
            let y = 200.0 + i as f32 * 108.0;
            let on = live && self.left(i).0 > 0.0;
            draw_rectangle(56.0, y + 4.0, TW - 112.0, 98.0, Color::new(0.08, 0.3, 0.42, if on { 0.5 } else { 0.22 }));
            draw_rectangle(56.0, y + 4.0, 12.0, 98.0, col);
            draw_text(n, 90.0, y + 54.0, 56.0, if on { GOLD } else { WHITE });
            draw_text(&fit(&format!("/lei {id}  -  {d}"), xb - 130.0, 30.0), 90.0, y + 90.0, 30.0, SOFT);
            let v = self.st[i].v;
            let k = if live { (v as f32 / self.q.max(1) as f32).min(1.0) } else { 0.0 };
            draw_rectangle(xb, y + 30.0, bw, 44.0, Color::new(0.45, 1.0, 1.0, 0.1));
            draw_rectangle(xb, y + 30.0, bw * k, 44.0, Color::new(col.r, col.g, col.b, 0.85));
            draw_rectangle_lines(xb, y + 30.0, bw, 44.0, 2.0, DIM);
            draw_text(&if live { format!("{v}/{}", self.q) } else { "-".into() }, xb + bw + 24.0, y + 68.0, 48.0, WHITE);
            let (st, sc) = self.status(i);
            text_right(&st, TW - 70.0, y + 66.0, 46.0, sc);
        }

        frame(40.0, 766.0, TW - 80.0, 176.0, "RELATOR IA");
        let (body, bc) = if !live {
            (if (time * 2.0).fract() < 0.6 { "CONECTANDO AO PLENARIO..." } else { "" }.to_string(), SOFT)
        } else if self.last.is_empty() {
            ("nenhuma lei aprovada ainda. a urna ta de olho.".into(), SOFT)
        } else {
            (format!("\"{}\"", self.last), WHITE)
        };
        for (j, l) in wrap(&body, TW - 160.0, 44.0, 2).iter().enumerate() {
            draw_text(l, 70.0, 856.0 + j as f32 * 50.0, 44.0, bc);
        }
        draw_text("/lei id   /leis   -   ou pisa no pulpito", 40.0, 1000.0, 40.0, CYAN);
        text_right("max 2 leis juntas  -  depois 10 min de recesso", TW - 40.0, 1000.0, 30.0, SOFT);
        holo_fx(time, self.flash());
    }

    /// Emblema gigante girando sobre a praça pra cada lei em vigor (visível do mapa todo).
    fn emblems(&self, b: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        if !(self.got && self.live) {
            return;
        }
        let act: Vec<(usize, f32)> = (0..5).map(|i| (i, self.left(i).0)).filter(|x| x.1 > 0.0).collect();
        let base = vec3(crate::layout::PLAZA.x, G as f32 + 38.0, crate::layout::PLAZA.y);
        let ring = crate::quality::pick([0, 8, 14]);
        for (j, &(i, on)) in act.iter().enumerate() {
            let (_, n, _, col) = LAWS[i];
            let k = (1.0 - (get_time() - self.pulse[i]) as f32 / 2.0).max(0.0);
            let s = 1.0 + 0.5 * k * (k * 12.0).sin().abs();
            let lit = |c: Color| Color::new(c.r + (1.0 - c.r) * k * 0.7, c.g + (1.0 - c.g) * k * 0.7, c.b + (1.0 - c.b) * k * 0.7, 1.0);
            let c = base + vec3((j as f32 - (act.len() - 1) as f32 * 0.5) * 26.0, (time * 1.2 + i as f32).sin() * 0.8, 0.0);
            let m = Mat4::from_translation(c) * Mat4::from_rotation_y(time * 0.7 + i as f32 * 1.3) * Mat4::from_scale(Vec3::splat(s));
            let rows = ICONS[i];
            let (w, h) = (rows[0].len() as f32, rows.len() as f32);
            for (y, r) in rows.iter().enumerate() {
                for (x, ch) in r.bytes().enumerate() {
                    if ch != b'#' {
                        continue;
                    }
                    let cc = match i {
                        2 if x > 0 => WHITE,
                        3 if (x + y + (time * 4.0) as usize) % 2 == 0 => WHITE,
                        _ => col,
                    };
                    let p = vec3((x as f32 - (w - 1.0) * 0.5) * 2.0, ((h - 1.0) * 0.5 - y as f32) * 2.0, 0.0);
                    b.glow(&m, p, vec3(1.85, 1.85, 1.2), lit(cc));
                }
            }
            if i == 3 {
                b.glow(&m, vec3(0.0, 10.0, 0.0), vec3(0.2, 6.0, 0.2), WHITE);
            }
            for r in 0..ring {
                let a = r as f32 / ring as f32 * TAU + time * 1.5;
                b.glow(&Mat4::IDENTITY, c + vec3(a.cos() * 10.5 * s, -8.5 + (a * 3.0 + time * 2.0).sin() * 0.4, a.sin() * 10.5 * s), Vec3::splat(0.9), lit(col));
            }
            let top = c + vec3(0.0, 10.5 + j as f32 * 4.0, 0.0);
            let d = eye.distance(top);
            if d < 200.0 {
                // O main descarta rótulo além de 45/80 blocos (qualidade): puxa pro raio de visão, mesma posição na tela
                let pos = eye + (top - eye) * (40.0 / d).min(1.0);
                labels.push(Label { pos, text: format!("LEI EM VIGOR: {n} {}", mmss(on)), size: 30.0, color: col });
            }
        }
    }

    /// Confete/fogos sobre o plenário por alguns segundos depois que uma lei passa.
    fn confetti(&self, b: &mut Batch, eye: Vec3) {
        let t = (get_time() - self.burst) as f32;
        let o = vec3(79.0, G as f32 + 9.0, 98.5);
        if !(0.0..3.5).contains(&t) || eye.distance(o) > 220.0 {
            return;
        }
        let size = 0.55 * (1.0 - t / 3.5);
        for k in 0..crate::quality::pick([14u32, 28, 44]) {
            let (a, up, sp) = (rnd(k, 1) * TAU, 0.3 + rnd(k, 2) * 0.9, 6.0 + rnd(k, 3) * 6.0);
            let p = o + vec3(a.cos(), up, a.sin()) * sp * t - vec3(0.0, 3.0 * t * t, 0.0);
            let col = if k % 3 == 0 { self.burst_col } else { LAWS[k as usize % 5].3 };
            b.glow(&Mat4::IDENTITY, p, Vec3::splat(size), col);
        }
    }
}

impl Place for Congresso {
    fn on_msg(&mut self, m: &Value) {
        let Some(a) = m["laws"].as_array() else { return };
        for l in a {
            let Some(i) = LAWS.iter().position(|x| Some(x.0) == l["id"].as_str()) else { continue };
            let on = l["on"].as_f64().unwrap_or(0.0) as f32;
            if on > 0.0 && self.st[i].on <= 0.0 && self.sent == Some(i) {
                self.sent = None;
            }
            if on > 0.0 && self.got && self.left(i).0 <= 0.0 {
                self.pulse[i] = get_time();
                (self.burst, self.burst_col) = (get_time(), LAWS[i].3);
            }
            self.st[i] = Vote { v: l["v"].as_u64().unwrap_or(0) as u32, on, cd: l["cd"].as_f64().unwrap_or(0.0) as f32 };
        }
        let passed = m["passed"].as_u64().unwrap_or(0) as u32;
        if self.got && passed > self.passed {
            self.flash_at = get_time();
        }
        (self.got, self.recv, self.passed) = (true, get_time(), passed);
        self.q = m["q"].as_u64().unwrap_or(1) as u32;
        self.last = super::s(&m["last"]);
        let d = &m["mds"];
        let arr = |k: &str| d[k].as_array().cloned().unwrap_or_default();
        self.mds = Contest {
            week: super::s(&d["week"]),
            left: d["left"].as_f64().unwrap_or(0.0) as f32,
            n: d["n"].as_u64().unwrap_or(0) as u32,
            tot: d["tot"].as_u64().unwrap_or(0) as u32,
            top: arr("top").iter().map(Entry::from).collect(),
            winner: d["winner"].is_object().then(|| (super::s(&d["winner"]["week"]), Entry::from(&d["winner"]))),
            hist: arr("hist").iter().map(|h| (super::s(&h["week"]), Entry::from(h))).collect(),
        };
    }

    fn update(&mut self, p: &mut Player, dt: f32, _time: f32, online: bool, out: &mut Vec<Value>) {
        self.live = online;
        self.cool = (self.cool - dt).max(0.0);
        let at = PADS.iter().position(|&(x, z)| p.on_ground && (p.pos.y - G as f32).abs() < 1.0 && (p.pos.x - x as f32 - 0.5).abs() < 1.5 && (p.pos.z - z as f32 - 0.5).abs() < 1.5);
        if at != self.pad {
            (self.pad, self.hold) = (at, 0.0);
        }
        let Some(i) = at else { return };
        if !(online && self.got) || self.sent == Some(i) {
            return;
        }
        self.hold = (self.hold + dt).min(HOLD);
        if self.hold >= HOLD && self.cool <= 0.0 {
            out.push(json!({"t": "pl", "k": "lei_voto", "lei": LAWS[i].0}));
            (self.sent, self.cool) = (Some(i), 3.0);
        }
    }

    fn render(&mut self, time: f32, eye: Vec3) {
        let live = self.got && self.live;
        let page = self.page(time);
        let m = &self.mds;
        let mins = if page == 1 { ((m.left - (get_time() - self.recv) as f32) / 60.0) as i32 } else { 0 };
        let mut key = format!("{live}|{}|{}|{}|{page}|{}|{}|{}|{mins}", self.q, self.passed, self.last, m.week, m.tot, m.n);
        for e in m.top.iter().chain(m.winner.iter().map(|w| &w.1)) {
            key += &format!("|{}:{}:{}", e.n, e.c, e.vo);
        }
        let mut animated = self.flash() > 0.0;
        for i in 0..5 {
            let (on, cd) = self.left(i);
            animated |= live && (on > 0.0 || cd > 0.0);
            key += &format!("|{}:{}:{}", self.st[i].v, on > 0.0, cd > 0.0);
        }
        if self.screen.begin(time, eye, &geo(), hash_str(&key), animated || !live || (page == 1 && m.top.is_empty())) {
            if page == 1 {
                self.paint_mods(time);
            } else {
                self.paint(time);
            }
            self.screen.end();
        }
    }

    fn draw(&self, b: &mut Batch, trans: &mut Batch, labels: &mut Vec<Label>, time: f32, eye: Vec3) {
        self.emblems(b, labels, time, eye);
        self.confetti(b, eye);
        if eye.distance(center()) > 130.0 {
            return;
        }
        let id = Mat4::IDENTITY;
        let g = G as f32;
        let live = self.got && self.live;
        frame_at(b, trans, &(Mat4::from_translation(vec3(PX, 0.0, PZ)) * Mat4::from_rotation_y(PI)), (PW, PH, PY), time, self.flash());
        let beams = crate::quality::pick([false, true, true]);
        for (i, &(x, z)) in PADS.iter().enumerate() {
            let (_, n, _, col) = LAWS[i];
            let c = vec3(x as f32 + 0.5, g, z as f32 + 0.5);
            let (on, cd) = if live { self.left(i) } else { (0.0, 0.0) };
            let pulse = 0.75 + 0.25 * (time * 3.0 + i as f32).sin();
            let k = if on > 0.0 { 1.0 } else if cd > 0.0 { 0.3 } else { pulse };
            b.glow(&id, c + vec3(0.0, 0.03, 0.0), vec3(2.8, 0.06, 2.8), Color::new(col.r * k, col.g * k, col.b * k, 1.0));
            let mine = self.sent == Some(i);
            if self.pad == Some(i) && self.hold > 0.0 && !mine {
                let s = 2.6 * self.hold / HOLD;
                b.glow(&id, c + vec3(0.0, 0.07, 0.0), vec3(s, 0.04, s), WHITE);
            }
            // Púlpito na borda oeste (de frente pro painel)
            b.cube(&id, c + vec3(-1.75, 0.6, 0.0), vec3(0.5, 1.2, 1.4), rgb(0.12, 0.13, 0.16));
            b.glow(&id, c + vec3(-1.75, 1.23, 0.0), vec3(0.6, 0.06, 1.5), if mine { WHITE } else { col });
            let v = self.st[i].v;
            if beams && live && (v > 0 || on > 0.0) {
                let h = if on > 0.0 { 6.0 } else { 1.0 + 4.0 * (v as f32 / self.q.max(1) as f32).min(1.0) };
                trans.glow(&id, c + vec3(0.0, h * 0.5, 0.0), vec3(0.35, h, 0.35), Color::new(col.r, col.g, col.b, 0.25 + 0.1 * pulse));
            }
            if eye.distance(c) < 35.0 {
                let text = if !live {
                    format!("LEI: {n} - CONECTANDO...")
                } else if self.pad == Some(i) && !mine && on <= 0.0 && cd <= 0.0 {
                    format!("LEI: {n} - VOTANDO... {:.0}%", 100.0 * self.hold / HOLD)
                } else if on > 0.0 {
                    format!("LEI: {n} - EM VIGOR {}", mmss(on))
                } else if cd > 0.0 {
                    format!("LEI: {n} - RECESSO {} - {v}/{} VOTOS", mmss(cd), self.q)
                } else if mine {
                    format!("LEI: {n} - SEU VOTO - {v}/{} VOTOS", self.q)
                } else {
                    format!("LEI: {n} - {v}/{} VOTOS - PISA PRA VOTAR", self.q)
                };
                labels.push(Label { pos: c + vec3(0.0, 2.2 + (i % 2) as f32 * 0.9, 0.0), text, size: 20.0, color: if on > 0.0 { GOLD } else { col } });
            }
        }
        // Mastros com bandeira da vila balançando na entrada
        for z in [91.5f32, 105.5] {
            let base = vec3(84.5, g, z);
            b.cube(&id, base + vec3(0.0, 4.5, 0.0), vec3(0.2, 9.0, 0.2), rgb(0.8, 0.8, 0.82));
            let n = crate::quality::pick([0, 4, 8]);
            for s in 0..n {
                let t = (s as f32 + 0.5) / n as f32;
                let wave = (time * 4.0 - t * 6.0).sin() * 0.3 * t;
                let p = base + vec3(wave, 8.0, 0.1 + t * 3.2);
                b.cube(&id, p, vec3(0.08, 1.6, 3.25 / n as f32), rgb(0.1, 0.6, 0.25));
                b.glow(&id, p, vec3(0.1, 0.4, 3.25 / n as f32), GOLD);
            }
        }
        if eye.distance(center()) < 80.0 {
            labels.push(Label { pos: vec3(86.5, g + 7.6, PZ), text: "CONGRESSO DA VILA".into(), size: 30.0, color: CYAN });
        }
    }

    fn draw_screen(&self, time: f32, eye: Vec3) {
        self.screen.draw(&geo(), time, eye, 140.0);
    }
}

pub fn build(w: &mut World) {
    let r = crate::layout::CONGRESSO;
    let g = G;
    clear_lot(w, r, 26, STONE);
    // Gramado nas bordas
    fill(w, (r.0, g - 1, r.1), (r.2, g - 1, r.1 + 1), GRASS);
    fill(w, (r.0, g - 1, r.3 - 1), (r.2, g - 1, r.3), GRASS);
    fill(w, (r.0, g - 1, r.1), (r.0 + 1, g - 1, r.3), GRASS);

    // Laje comprida (oeste): paredes brancas, faixa de vidro, porta leste
    fill(w, (52, g, 84), (66, g + 4, 112), WOOL);
    fill(w, (53, g, 85), (65, g + 3, 111), AIR);
    fill(w, (52, g + 1, 85), (52, g + 2, 111), GLASS);
    fill(w, (66, g + 1, 85), (66, g + 2, 111), GLASS);
    fill(w, (53, g + 1, 84), (65, g + 2, 84), GLASS);
    fill(w, (53, g + 1, 112), (65, g + 2, 112), GLASS);
    fill(w, (66, g, 96), (66, g + 2, 100), AIR);
    // Marquise com colunas finas
    fill(w, (67, g + 4, 84), (68, g + 4, 112), WOOL);
    for z in (84..=112).step_by(4) {
        fill(w, (68, g, z), (68, g + 3, z), WOOL);
    }

    // Torres gêmeas finas (face larga pro leste) com ponte de vidro
    for z0 in [93, 100] {
        fill(w, (54, g, z0), (55, g + 22, z0 + 3), WOOL);
        for y in (g + 5..=g + 21).step_by(2) {
            fill(w, (54, y, z0 + 1), (55, y, z0 + 2), GLASS);
        }
    }
    fill(w, (54, g + 17, 97), (55, g + 18, 99), GLASS);
    fill(w, (54, g + 19, 97), (55, g + 19, 99), WOOL);

    // Cúpula (norte) e cuia virada pra cima (sul) no teto
    let y0 = g + 5;
    let (cx, cz) = (59.5f32, 89.5f32);
    for y in y0..y0 + 5 {
        for z in 84..96 {
            for x in 53..66 {
                let d = vec3(x as f32 + 0.5 - cx, (y - y0) as f32 + 0.5, z as f32 + 0.5 - cz).length();
                if d <= 4.6 && d > 3.4 {
                    w.set(x, y, z, WOOL);
                }
            }
        }
    }
    let (cx, cz) = (59.5f32, 107.5f32);
    for k in 0..4 {
        let rr = 2.2 + k as f32 * 1.15;
        for z in 100..115 {
            for x in 53..66 {
                let d = vec2(x as f32 + 0.5 - cx, z as f32 + 0.5 - cz).length();
                if d <= rr && (k == 0 || d > rr - 1.2) {
                    w.set(x, y0 + k, z, WOOL);
                }
            }
        }
    }

    // Plenário aberto (leste): passarela, arquibancadas baixas, púlpitos
    fill(w, (72, g - 1, 97), (r.2, g - 1, 99), COBBLE);
    for (z, s) in [(82, 1), (83, 0), (113, 0), (114, 1)] {
        fill(w, (74, g, z), (84, g + s, z), WOOL);
    }
    for (x, z) in PADS {
        fill(w, (x - 1, g - 1, z - 1), (x + 1, g - 1, z + 1), BLACK);
    }
    // Pórtico da entrada
    for z in [94, 102] {
        fill(w, (r.2, g, z), (r.2, g + 5, z), WOOL);
    }
    fill(w, (r.2, g + 6, 94), (r.2, g + 6, 102), WOOL);
}
