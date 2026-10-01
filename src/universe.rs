//! Universo: guarda-roupa (skins = mods kind "avatar"), avatar desenhado nos jogadores remotos e itens de
//! outros mundos (mods kind "item" ganhos nos jogos do hub). O servidor (server/universe.js) carimba a skin
//! de cada jogador no "p" e manda os pacotes sob demanda ({t:"uv_get"} -> {t:"uv_pkg"}), que ficam em cache.

use crate::batch::Batch;
use crate::models::rgb;
use macroquad::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
mod js {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        pub fn urna_uv_ctok(ptr: *const u8, len: usize);
    }
}

struct Part {
    parent: Option<usize>,
    pivot: Vec3,
    boxes: Vec<(Vec3, Vec3, Color, bool)>,
}

struct Anim {
    dur: f32,
    looped: bool,
    /// (t, [(rotação rad, offset)] por parte)
    keys: Vec<(f32, Vec<(Vec3, Vec3)>)>,
}

struct Avatar {
    parts: Vec<Part>,
    anims: HashMap<String, Anim>,
    scale: f32,
}

pub struct Entry {
    pub id: String,
    pub name: String,
    pub creator: String,
}

pub struct BagItem {
    pub name: String,
    pub n: u64,
    pub c1: Color,
    pub c2: Color,
}

const PALETTE: [(&str, u32); 16] = [
    ("white", 0xf2f2f2), ("light_gray", 0xa8a8a8), ("gray", 0x5c5c5c), ("black", 0x1c1c1c), ("red", 0xd83a2e), ("orange", 0xf08a24),
    ("yellow", 0xf5d63a), ("lime", 0x7ed63a), ("green", 0x3a8a2e), ("cyan", 0x2ec4c4), ("light_blue", 0x6ab4f0), ("blue", 0x2e4ad8),
    ("purple", 0x8a3ad8), ("magenta", 0xd83ab4), ("pink", 0xf0a0c0), ("brown", 0x7a5030),
];

fn col(h: u32) -> Color {
    rgb(((h >> 16) & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, (h & 255) as f32 / 255.0)
}

fn hex(v: &Value) -> Color {
    col(u32::from_str_radix(v.as_str().unwrap_or("#ff00ff").trim_start_matches('#'), 16).unwrap_or(0xff00ff))
}

fn pal(v: &Value) -> Color {
    col(PALETTE.iter().find(|p| Some(p.0) == v.as_str()).map_or(0xffffff, |p| p.1))
}

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn v3(v: &Value) -> Vec3 {
    vec3(f(&v[0]), f(&v[1]), f(&v[2]))
}

fn parse(pkg: &Value) -> Option<Avatar> {
    let jp = pkg["model"]["parts"].as_array()?;
    let names: Vec<&str> = jp.iter().map(|p| p["name"].as_str().unwrap_or("")).collect();
    let parts = jp
        .iter()
        .map(|p| Part {
            parent: p["parent"].as_str().and_then(|n| names.iter().position(|q| *q == n)),
            pivot: v3(&p["pivot"]),
            boxes: p["boxes"].as_array().into_iter().flatten().map(|b| (v3(&b["pos"]), v3(&b["size"]), hex(&b["color"]), b["glow"].as_bool().unwrap_or(false))).collect(),
        })
        .collect();
    let anims = pkg["animations"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, a)| {
            let keys = a["keyframes"].as_array().into_iter().flatten().map(|kf| (f(&kf["t"]), names.iter().map(|n| (v3(&kf["parts"][*n]["rot"]) * (std::f32::consts::PI / 180.0), v3(&kf["parts"][*n]["offset"]))).collect())).collect();
            (k.clone(), Anim { dur: f(&a["duration"]).max(0.1), looped: a["loop"].as_bool().unwrap_or(true), keys })
        })
        .collect();
    Some(Avatar { parts, anims, scale: pkg["avatar"]["scale"].as_f64().map_or(1.0, |s| s as f32).clamp(0.3, 2.0) })
}

fn sample(a: &Anim, t: f32, n: usize) -> Vec<(Vec3, Vec3)> {
    let mut out = vec![(Vec3::ZERO, Vec3::ZERO); n];
    let (Some(first), Some(last)) = (a.keys.first(), a.keys.last()) else { return out };
    let t = if a.looped { t.rem_euclid(a.dur) } else { t.min(a.dur) };
    let (k0, k1, w) = if t <= first.0 {
        (first, first, 0.0)
    } else if t >= last.0 {
        (last, last, 0.0)
    } else {
        let i = a.keys.windows(2).position(|p| t >= p[0].0 && t < p[1].0).unwrap_or(0);
        (&a.keys[i], &a.keys[i + 1], (t - a.keys[i].0) / (a.keys[i + 1].0 - a.keys[i].0).max(1e-4))
    };
    for (i, o) in out.iter_mut().enumerate() {
        let (p0, p1) = (k0.1.get(i).copied().unwrap_or_default(), k1.1.get(i).copied().unwrap_or_default());
        *o = (p0.0.lerp(p1.0, w), p0.1.lerp(p1.1, w));
    }
    out
}

pub struct Universe {
    pub list: Vec<Entry>,
    /// Minha skin confirmada pelo servidor ("id@versao").
    pub me: Option<String>,
    cache: HashMap<String, Avatar>,
    asked: HashMap<String, f64>,
    pub bag: Vec<BagItem>,
    /// Mensagens pro servidor.
    pub outbox: Vec<Value>,
}

impl Universe {
    pub fn new() -> Self {
        Universe { list: Vec::new(), me: None, cache: HashMap::new(), asked: HashMap::new(), bag: Vec::new(), outbox: Vec::new() }
    }

    /// uv_list / uv_me / uv_pkg / uv_bag / uv_ctok.
    pub fn on_msg(&mut self, m: &Value) {
        match m["t"].as_str().unwrap_or("") {
            "uv_list" => {
                let s = |v: &Value| v.as_str().unwrap_or("").to_string();
                self.list = m["av"].as_array().into_iter().flatten().map(|a| Entry { id: s(&a["id"]), name: s(&a["name"]), creator: s(&a["creator"]) }).collect();
                self.me = m["me"].as_str().map(String::from);
            }
            "uv_me" => self.me = m["me"].as_str().map(String::from),
            "uv_pkg" => {
                if let (Some(id), Some(v), Some(a)) = (m["id"].as_str(), m["v"].as_str(), parse(&m["pkg"])) {
                    if self.cache.len() > 32 {
                        self.cache.clear();
                    }
                    self.cache.insert(format!("{id}@{v}"), a);
                }
            }
            "uv_bag" => {
                self.bag = m["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|x| BagItem { name: x["name"].as_str().unwrap_or("?").to_string(), n: x["n"].as_u64().unwrap_or(0), c1: pal(&x["color"]), c2: pal(&x["color2"]) })
                    .collect();
            }
            "uv_ctok" => {
                #[cfg(target_arch = "wasm32")]
                {
                    let s = m.to_string();
                    unsafe { js::urna_uv_ctok(s.as_ptr(), s.len()) };
                }
            }
            _ => {}
        }
    }

    /// Desenha o avatar `r` ("id@v"); false = ainda sem pacote (pede ao servidor e o chamador usa o boneco padrão).
    pub fn draw(&mut self, b: &mut Batch, r: &str, pos: Vec3, yaw: f32, speed: f32, air: bool, t: f32) -> bool {
        let Some(av) = self.cache.get(r) else {
            let id = r.split('@').next().unwrap_or("").to_string();
            let now = get_time();
            if self.asked.get(&id).is_none_or(|t0| now - t0 > 10.0) {
                self.asked.insert(id.clone(), now);
                self.outbox.push(json!({"t": "uv_get", "mod": id}));
            }
            return false;
        };
        let name = if air && av.anims.contains_key("jump") {
            "jump"
        } else if speed > 4.5 && av.anims.contains_key("run") {
            "run"
        } else if speed > 0.3 {
            "walk"
        } else {
            "idle"
        };
        let n = av.parts.len();
        let poses = av.anims.get(name).map_or_else(|| vec![(Vec3::ZERO, Vec3::ZERO); n], |a| sample(a, t, n));
        let root = Mat4::from_translation(pos) * Mat4::from_rotation_y(yaw) * Mat4::from_scale(Vec3::splat(av.scale));
        let mut mats: Vec<Mat4> = Vec::with_capacity(n);
        for (i, p) in av.parts.iter().enumerate() {
            let (rot, off) = poses[i];
            let parent = p.parent.and_then(|j| mats.get(j).copied()).unwrap_or(root);
            let m = parent * Mat4::from_translation(p.pivot + off) * Mat4::from_rotation_y(rot.y) * Mat4::from_rotation_x(rot.x) * Mat4::from_rotation_z(rot.z) * Mat4::from_translation(-p.pivot);
            for (c, s, color, glow) in &p.boxes {
                if *glow {
                    b.glow(&m, *c, *s, *color);
                } else {
                    b.cube(&m, *c, *s, *color);
                }
            }
            mats.push(m);
        }
        true
    }

    /// Cartões da fileira SKIN em cima dos personagens (o primeiro = boneco padrão).
    fn cards(&self, sw: f32, sh: f32) -> Vec<Rect> {
        let n = (self.list.len() + 1).min(8) as f32;
        let (gap, h) = (8.0, 48.0);
        let w = ((sw - gap * (n + 1.0)) / n).min(170.0);
        let y = sh * 0.5 - (sh * 0.36).min(200.0) * 0.5 - 108.0;
        let x0 = sw * 0.5 - (w * n + gap * (n - 1.0)) * 0.5;
        (0..n as usize).map(|i| Rect::new(x0 + i as f32 * (w + gap), y, w, h)).collect()
    }

    /// Clique na fileira SKIN: pede a troca ao servidor e devolve o texto do banner.
    pub fn click(&mut self, p: Vec2, sw: f32, sh: f32) -> Option<String> {
        let i = self.cards(sw, sh).iter().position(|r| r.contains(p))?;
        let (id, name) = if i == 0 { (String::new(), "PADRAO".to_string()) } else { (self.list[i - 1].id.clone(), self.list[i - 1].name.to_uppercase()) };
        self.outbox.push(json!({"t": "uv_set", "mod": id}));
        Some(format!("SKIN: {name}"))
    }

    pub fn draw_wardrobe(&self, sw: f32, sh: f32, text: impl Fn(&str, f32, f32, f32, Color)) {
        let cards = self.cards(sw, sh);
        let mine = self.me.as_deref().and_then(|r| r.split('@').next()).unwrap_or("");
        text(if self.list.is_empty() { "SKIN: nenhuma skin de mod ativa - crie uma (modding.txt, kind avatar)" } else { "SKIN (mods de avatar, todo mundo ve e vale nos jogos do hub)" }, sw * 0.5, cards[0].y - 10.0, 18.0, Color::new(0.75, 0.6, 1.0, 1.0));
        for (i, r) in cards.iter().enumerate() {
            let (label, by, on) = if i == 0 { ("PADRAO", "", mine.is_empty()) } else { let e = &self.list[i - 1]; (e.name.as_str(), e.creator.as_str(), e.id == mine) };
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.12, 0.06, 0.2, 0.92));
            draw_rectangle_lines(r.x, r.y, r.w, r.h, if on { 3.0 } else { 1.5 }, if on { Color::new(1.0, 0.85, 0.3, 1.0) } else { Color::new(0.5, 0.4, 0.7, 1.0) });
            text(label, r.x + r.w * 0.5, r.y + r.h * 0.45, (r.w / 9.0).clamp(11.0, 18.0), WHITE);
            if !by.is_empty() {
                text(&format!("por {by}"), r.x + r.w * 0.5, r.y + r.h * 0.85, 12.0, Color::new(0.75, 0.75, 0.85, 1.0));
            }
        }
    }

    /// Faixa "ITENS DE OUTROS MUNDOS" em cima da tela do inventário.
    pub fn draw_bag(&self, sw: f32, text: impl Fn(&str, f32, f32, f32, Color)) {
        let s = 34.0;
        let n = self.bag.len().max(1) as f32;
        let w = (n * (s + 110.0)).min(sw - 20.0);
        let (x0, y0) = (sw * 0.5 - w * 0.5, 14.0);
        draw_rectangle(x0 - 8.0, y0 - 6.0, w + 16.0, s + 34.0, Color::new(0.1, 0.05, 0.18, 0.88));
        text("ITENS DE OUTROS MUNDOS (ganhos nos jogos do GAME HUB)", sw * 0.5, y0 + 10.0, 15.0, Color::new(0.8, 0.65, 1.0, 1.0));
        if self.bag.is_empty() {
            text("nenhum ainda - jogue nos portais do hub", sw * 0.5, y0 + 36.0, 14.0, Color::new(0.7, 0.7, 0.8, 1.0));
            return;
        }
        for (i, it) in self.bag.iter().enumerate() {
            let x = x0 + i as f32 * (s + 110.0);
            let y = y0 + 18.0;
            if x + s > sw {
                break;
            }
            draw_rectangle(x, y, s, s, it.c2);
            draw_rectangle(x + s * 0.2, y + s * 0.2, s * 0.6, s * 0.6, it.c1);
            draw_rectangle_lines(x, y, s, s, 2.0, WHITE);
            text(&format!("{} x{}", it.name, it.n), x + s + 52.0, y + s * 0.62, 14.0, WHITE);
        }
    }
}
