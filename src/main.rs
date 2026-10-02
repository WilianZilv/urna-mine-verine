//! URNA-MINE-VERINE: clone de Minecraft em Rust com vila, clube de house,
//! briga generalizada e uma urna eletrônica gigante soltando laser.

mod actors;
mod arena_panel;
mod atlas;
#[cfg_attr(target_arch = "wasm32", path = "audio_web.rs")]
mod audio;
mod batch;
mod bomba;
mod chunks;
mod club;
mod economy;
mod eleicao;
mod extras;
mod gta;
mod gumba;
mod hub;
mod inventory;
mod items;
mod lab;
mod layout;
mod mario;
mod models;
mod mods;
mod mp;
mod npc;
#[cfg_attr(target_arch = "wasm32", path = "net_web.rs")]
mod net;
mod places;
mod player;
mod portal;
mod prof;
mod quality;
mod ragdoll;
mod shield;
mod skate;
mod steve;
mod synth;
#[cfg_attr(target_arch = "wasm32", path = "telao_web.rs")]
mod telao;
mod tnt;
mod universe;
mod urna;
mod voador;
mod kaiju;
mod zeppelin;
#[cfg(target_arch = "wasm32")]
mod web;
mod wolvie;
mod world;

use actors::{Ev, FState, Fighter, VKind, Villager};
use audio::{Audio, Clip};
use batch::Batch;
use extras::Label;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use models::{Look, Pose, draw_banker_extras, draw_flag, draw_humanoid, draw_villager, rgb, root};
use player::Player;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use telao::Telao;
use urna::{Fx, Shot, Urna};
use world::*;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: macroquad::miniquad::conf::Conf {
            window_title: "URNA-MINE-VERINE".to_owned(),
            window_width: 1600,
            window_height: 900,
            sample_count: 4,
            ..Default::default()
        },
        draw_call_vertex_capacity: 60000,
        draw_call_index_capacity: 100000,
        ..Default::default()
    }
}

struct Sfx {
    boom: Clip,
    laser: Clip,
    punch: Clip,
    slash: Clip,
    snikt: Clip,
    deflect: Clip,
    gun: Clip,
}

struct FloatText {
    pos: Vec3,
    text: String,
    color: Color,
    t: f32,
    big: bool,
}

struct Remote {
    name: String,
    pos: Vec3,
    target: Vec3,
    yaw: f32,
    walk: f32,
    look: Look,
    /// Personagem: 0 Steve, 1 skatista, 2 bandido, 3 bandido dirigindo, 4 niko, 5 portal, 6 encanador, 7 wolverine, 8 bombadinho.
    ch: u8,
    /// Skin de mod ("id@versao", carimbada pelo servidor) e velocidade estimada pros quadros do avatar.
    av: Option<String>,
    spd: f32,
    vy: f32,
}

/// Dentro do clube só se ouve o que está dentro do escudo.
fn play_at(a: &Audio, s: &Clip, pos: Vec3, listener: Vec3, base: f32, muted: bool, in_club: bool) {
    if muted || (in_club && pos.distance(shield_center()) > SHIELD_R) {
        return;
    }
    let d = pos.distance(listener);
    let volume = (base / (1.0 + d / 22.0)).clamp(0.0, 1.0);
    if volume > 0.02 {
        a.play(s, volume, false);
    }
}

fn first_line(env: &str, file: &str) -> Option<String> {
    std::env::var(env)
        .ok()
        .or_else(|| std::fs::read_to_string(file).ok().and_then(|s| s.lines().next().map(str::to_string)))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn initial_source() -> String {
    first_line("TELAO_URL", "assets/telao.txt").unwrap_or_else(|| telao::DEFAULT_URL.to_string())
}

/// URL do servidor: MP_URL ou assets/server.txt (nativo). No navegador: mesmo host da página.
fn server_url() -> Option<String> {
    if cfg!(target_arch = "wasm32") {
        return Some(String::new());
    }
    let u = first_line("MP_URL", "assets/server.txt")?;
    let u = u.replacen("https://", "wss://", 1).replacen("http://", "ws://", 1);
    let u = u.trim_end_matches('/');
    Some(if u.ends_with("/ws") { u.to_string() } else { format!("{u}/ws") })
}

fn project(vp: &Mat4, p: Vec3) -> Option<Vec2> {
    let s = project_any(vp, p)?;
    let (w, h) = (screen_width(), screen_height());
    if s.x < -0.05 * w || s.x > 1.05 * w || s.y < -0.05 * h || s.y > 1.05 * h {
        return None;
    }
    Some(s)
}

fn project_any(vp: &Mat4, p: Vec3) -> Option<Vec2> {
    let c = *vp * p.extend(1.0);
    if c.w <= 0.05 {
        return None;
    }
    let n = c.truncate() / c.w;
    Some(vec2((n.x + 1.0) * 0.5 * screen_width(), (1.0 - n.y) * 0.5 * screen_height()))
}

/// Barrinha de vida do villager, usada por todo mundo; `r` é o raio do alvo (gigantes ganham barra um pouco maior).
fn overhead_bar(vp: &Mat4, top: Vec3, r: f32, k: f32) {
    let Some(s) = project(vp, top + Vec3::Y * 0.5) else { return };
    let z = (r / 0.7).sqrt().clamp(1.0, 2.2);
    let (w, h) = (44.0 * z, 5.0 * z.sqrt());
    let k = k.clamp(0.0, 1.0);
    draw_rectangle(s.x - w * 0.5, s.y, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
    draw_rectangle(s.x - w * 0.5, s.y, w * k, h, Color::new(1.0 - k, k, 0.1, 1.0));
}

/// Tamanhos contínuos rasterizam glifos novos no atlas da fonte (reenvia a textura inteira):
/// rasteriza em poucos tamanhos fixos e escala.
fn text_centered(s: &str, x: f32, y: f32, size: f32, color: Color, bg: bool) {
    let base = [12u16, 16, 20, 24, 32, 48, 64].into_iter().find(|b| *b as f32 >= size * 0.97).unwrap_or(64);
    let scale = size / base as f32;
    let dim = measure_text(s, None, base, scale);
    if bg {
        draw_rectangle(x - dim.width * 0.5 - 4.0, y - dim.height - 4.0, dim.width + 8.0, dim.height + 8.0, Color::new(0.0, 0.0, 0.0, 0.45));
    }
    prof::add(&prof::TEXTS, 1);
    let p = |color| TextParams { font_size: base, font_scale: scale, color, ..Default::default() };
    // Com fundo escuro a sombra quase não aparece: qualidade baixa pula (metade dos glifos)
    if !(bg && quality::tier() == quality::LOW) {
        draw_text_ex(s, x - dim.width * 0.5 + 2.0, y + 2.0, p(Color::new(0.0, 0.0, 0.0, color.a * 0.7)));
    }
    draw_text_ex(s, x - dim.width * 0.5, y, p(color));
}

/// Mesma semente em todos os clientes: dançarinos e lutadores nascem iguais.
fn spawn_actors() -> (Vec<Villager>, Vec<Fighter>) {
    rand::srand(2026);
    let out = (actors::spawn_villagers(), actors::spawn_fighters());
    rand::srand(macroquad::miniquad::date::now() as u64);
    out
}

fn remote_look(id: u64) -> Look {
    let shirts = [rgb(0.2, 0.6, 0.9), rgb(0.9, 0.5, 0.1), rgb(0.6, 0.2, 0.7), rgb(0.1, 0.7, 0.6), rgb(0.85, 0.85, 0.2), rgb(0.9, 0.3, 0.4)];
    let k = id as usize;
    Look {
        skin: [rgb(0.86, 0.66, 0.52), rgb(0.6, 0.42, 0.3), rgb(0.42, 0.28, 0.2)][k % 3],
        hair: [rgb(0.1, 0.08, 0.06), rgb(0.45, 0.3, 0.15), rgb(0.85, 0.7, 0.3)][k / 3 % 3],
        shirt: shirts[k % shirts.len()],
        pants: rgb(0.2, 0.25, 0.5),
        shoes: rgb(0.1, 0.1, 0.1),
        beard: None,
        glasses: k % 4 == 1,
        wolverine: false,
        toga: false,
    }
}

/// Caixa de texto do navegador (celular não tem teclado no jogo).
#[allow(unused_variables)]
fn ask_text(msg: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return web::prompt(msg).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    #[cfg(not(target_arch = "wasm32"))]
    None
}

fn tapped() -> bool {
    touches().iter().any(|t| t.phase == TouchPhase::Started)
}

/// Botões do celular: (centro, raio, rótulo).
/// Botões do celular; rótulos mudam com o personagem (0 Steve, 1 skatista, 2 bandido, 3 dirigindo).
fn touch_buttons(sw: f32, sh: f32, ch: u8) -> [(Vec2, f32, &'static str); 9] {
    let r = (sw.min(sh) * 0.085).max(26.0);
    let (x, y) = (sw - r * 1.4, sh - r * 1.4);
    let l = match ch {
        1 => ["OLLIE", "GRAB F", "MANUAL", "GRAB T"],
        4 => ["PULA", "SE JOGA", "-", "-"],
        2 => ["PULA", "ATIRA", "ARMA", "CARRO"],
        3 => ["FREIO", "-", "-", "SAIR"],
        5 => ["PULA", "AZUL", "LARANJA", "CUBO"],
        6 => ["PULA", "SOCO", "AGACHA", "-"],
        7 => ["PULA", "GARRA", "BOTE", "DEFESA"],
        8 => ["PULA", "BOMBA", "TIPO", "DETONA"],
        _ => ["PULA", "BATE", "POE", "VOA"],
    };
    [
        (vec2(x, y), r * 1.15, l[0]),
        (vec2(x - r * 2.5, y + r * 0.2), r, l[1]),
        (vec2(x - r * 0.4, y - r * 2.4), r, l[2]),
        (vec2(x - r * 2.7, y - r * 2.0), r * 0.75, l[3]),
        (vec2(r * 0.9 + 8.0, sh * 0.3), r * 0.7, "CHAT"),
        (vec2(r * 2.6 + 8.0, sh * 0.3), r * 0.7, "TELAO"),
        (vec2(r * 4.3 + 8.0, sh * 0.3), r * 0.7, "PERS"),
        (vec2(r * 6.0 + 8.0, sh * 0.3), r * 0.7, "BANCO"),
        (vec2(r * 7.7 + 8.0, sh * 0.3), r * 0.7, "CLIPE"),
    ]
}

/// Cards do menu: (nome, descrição, código do personagem escolhido).
const CHARS: [(&str, &str, u8); 8] = [
    ("STEVE SURVIVAL", "vida, ferramentas, arco, TNT", 0),
    ("STEVE CRIATIVO", "voa, blocos infinitos, sem dano", 1),
    ("SKATISTA", "SKATE 3: flick-it, grind, manual", 2),
    ("BANDIDO", "GTA 3: arsenal completo + carro", 3),
    ("ARMA DE PORTAL", "portais azul/laranja + cubo", 5),
    ("ENCANADOR", "pulo triplo, mortal, sentada", 6),
    ("WOLVERINE", "garras, bote, furia, fator de cura", 7),
    ("BOMBADINHO", "bombas: fogo, perfura, remota, linha", 8),
];

fn char_cards(sw: f32, sh: f32) -> [Rect; CHARS.len()] {
    let (gap, n) = (10.0, CHARS.len() as f32);
    let w = ((sw - gap * (n + 1.0)) / n).min(210.0);
    let h = (sh * 0.36).min(200.0);
    let x0 = sw * 0.5 - (w * n + gap * (n - 1.0)) * 0.5;
    std::array::from_fn(|i| Rect::new(x0 + i as f32 * (w + gap), sh * 0.5 - h * 0.5, w, h))
}

/// Botão de qualidade gráfica embaixo dos cards de personagem.
fn quality_button(sw: f32, sh: f32) -> Rect {
    let w = (sw * 0.8).min(320.0);
    Rect::new(sw * 0.5 - w * 0.5, sh * 0.5 + (sh * 0.36).min(200.0) * 0.5 + 48.0, w, 38.0)
}

/// Lê texto digitado (nome / chat). Retorna true no Enter.
fn type_into(s: &mut String, max: usize) -> bool {
    while let Some(c) = get_char_pressed() {
        if !c.is_control() && s.chars().count() < max {
            s.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        s.pop();
    }
    is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter)
}

async fn ask_name() -> String {
    #[cfg(target_arch = "wasm32")]
    let preset = web::read_string(|p, c| unsafe { web::urna_query_name(p, c) });
    #[cfg(not(target_arch = "wasm32"))]
    let preset = first_line("URNA_NOME", "assets/nome.txt");
    if let Some(n) = preset {
        return n.chars().take(16).collect();
    }
    let mut name = String::new();
    loop {
        if type_into(&mut name, 16) && !name.trim().is_empty() {
            return name.trim().to_string();
        }
        if tapped() {
            if let Some(n) = ask_text("Teu nome:") {
                return n.chars().take(16).collect();
            }
        }
        clear_background(Color::new(0.05, 0.02, 0.1, 1.0));
        let (cx, cy) = (screen_width() * 0.5, screen_height() * 0.5);
        text_centered("URNA-MINE-VERINE ONLINE", cx, cy - 90.0, 56.0, Color::new(1.0, 0.4, 0.9, 1.0), false);
        text_centered("DIGITA TEU NOME E APERTA ENTER (CELULAR: TOCA NA TELA)", cx, cy - 30.0, 26.0, WHITE, false);
        let cursor = if (get_time() * 2.0) as i32 % 2 == 0 { "_" } else { " " };
        text_centered(&format!("{name}{cursor}"), cx, cy + 40.0, 48.0, YELLOW, true);
        next_frame().await;
    }
}

/// Aplica evento de mundo (bloco, tiro, reset). Retorna o tiro pra efeitos.
fn apply_world(world: &mut World, m: &Value, fx: &mut Fx, avg: &[Color]) -> Option<(urna::Plan, Shot)> {
    match m["k"].as_str()? {
        "set" => {
            let p = &m["p"];
            world.set(p[0].as_i64()? as i32, p[1].as_i64()? as i32, p[2].as_i64()? as i32, m["b"].as_u64()? as u8);
            None
        }
        "shot" => {
            let plan = mp::get_shot(m);
            let s = urna::apply(world, &plan, fx, avg);
            Some((plan, s))
        }
        "bomb" => {
            bomba::apply(world, m, fx, avg);
            None
        }
        "reset" => {
            *world = mods::generate();
            None
        }
        "ai" => {
            let p3 = |v: &Value| ivec3(v[0].as_i64().unwrap_or(0) as i32, v[1].as_i64().unwrap_or(0) as i32, v[2].as_i64().unwrap_or(0) as i32);
            for op in m["ops"].as_array()? {
                let k = op["k"].as_u64().unwrap_or(0) as u8;
                let hollow = op["h"].as_bool().unwrap_or(false);
                match op["op"].as_str().unwrap_or("") {
                    "box" => {
                        let (a, b) = (p3(&op["a"]), p3(&op["b"]));
                        let (lo, hi) = (a.min(b), a.max(b));
                        for y in lo.y..=hi.y {
                            for z in lo.z..=hi.z {
                                for x in lo.x..=hi.x {
                                    let edge = x == lo.x || x == hi.x || z == lo.z || z == hi.z || y == lo.y || y == hi.y;
                                    if !hollow || edge {
                                        world.set(x, y, z, k);
                                    }
                                }
                            }
                        }
                    }
                    "ball" => {
                        let (c, r) = (p3(&op["c"]), op["r"].as_i64().unwrap_or(1) as i32);
                        for y in -r..=r {
                            for z in -r..=r {
                                for x in -r..=r {
                                    let d = ((x * x + y * y + z * z) as f32).sqrt();
                                    if d <= r as f32 + 0.3 && (!hollow || d > r as f32 - 1.0) {
                                        world.set(c.x + x, c.y + y, c.z + z, k);
                                    }
                                }
                            }
                        }
                    }
                    "boom" => urna::explode(world, p3(&op["c"]).as_vec3() + Vec3::splat(0.5), op["r"].as_f64().unwrap_or(3.0) as f32, fx, avg),
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);
    simulate_mouse_with_touch(false);
    let url = server_url();
    let my_name = if url.is_some() { ask_name().await } else { "JOGADOR".to_string() };
    for _ in 0..2 {
        clear_background(Color::new(0.05, 0.02, 0.1, 1.0));
        text_centered("CARREGANDO A VILA... SINTETIZANDO O HOUSE...", screen_width() * 0.5, screen_height() * 0.5, 40.0, WHITE, false);
        next_frame().await;
    }

    let atlas = atlas::build();
    let mut world = mods::generate();
    let mut chunks = chunks::Chunks::new(&mut world, &atlas.tex);

    // Áudio: house sintetizado de fallback até o telão (YouTube) começar a tocar
    let audio = Audio::new();
    let sr = audio.rate;
    let bpm: f32 = std::env::var("HOUSE_BPM").ok().and_then(|v| v.parse().ok()).unwrap_or(synth::BPM);
    let sfx = Sfx {
        boom: Arc::new(synth::boom(sr)),
        laser: Arc::new(synth::laser(sr)),
        punch: Arc::new(synth::punch(sr)),
        slash: Arc::new(synth::slash(sr)),
        snikt: Arc::new(synth::snikt(sr)),
        deflect: Arc::new(synth::deflect(sr)),
        gun: Arc::new(synth::gun(sr)),
    };
    let house = audio.play(&Arc::new(synth::house_loop(sr)), 0.6, true);
    let engine: Vec<u64> = [38.0, 58.0, 88.0, 130.0].iter().map(|hz| audio.play(&Arc::new(synth::engine(sr, *hz)), 0.0, true)).collect();
    let mut telao = Telao::new(&audio);
    let mut tv_src = initial_source();
    telao.load(&tv_src);
    let guests = actors::spawn_guests();
    let relogio = eleicao::Relogio::new();
    let lab = extras::Lab::new();
    let mut lab_info = lab::LabInfo::new();
    let mut arena_board = arena_panel::ArenaPanel::new();
    let mut places = places::Places::new();
    let mut hub = hub::Hub::new();
    let mut club_k = 0.0f32;
    let mut vibe = club::Vibe::new();
    let mut vibe_t = 0.0f32;
    // Celular
    #[cfg(target_arch = "wasm32")]
    let mut mobile = unsafe { web::urna_is_touch() } != 0;
    #[cfg(not(target_arch = "wasm32"))]
    let mut mobile = false;
    let mut quality = quality::Quality::new(mobile);
    let mut stick: Option<(u64, Vec2)> = None;
    let mut look_touch: Option<(u64, Vec2)> = None;
    let mut held: HashMap<u64, usize> = HashMap::new();
    let mut look_origin = Vec2::ZERO;
    let mut skate_touch: Option<Vec2> = None;
    // Personagens
    let mut chars_open = false;
    let mut sk_test = skate::Test::from_env();
    let mut skater: Option<skate::Skater> = sk_test.as_ref().map(|t| t.spawn());
    let mut bandido: Option<gta::Bandido> = None;
    let mut car = gta::Car::new(&world);
    let mut niko: Option<ragdoll::Ragdoll> = None;
    let niko_look = Look { skin: rgb(0.86, 0.7, 0.58), hair: rgb(0.12, 0.09, 0.07), shirt: rgb(0.36, 0.33, 0.29), pants: rgb(0.14, 0.14, 0.17), shoes: rgb(0.08, 0.06, 0.05), beard: None, glasses: false, wolverine: false, toga: false };
    let mut gta_walk = 0.0f32;
    // Efeitos pedidos pro agente IA
    let mut ai_say: Option<(String, f32)> = None;
    let mut ai_fireworks = 0.0f32;
    let mut ai_rage = 0.0f32;
    let mut ai_sky: Option<(Color, f32)> = None;
    let mut eco = economy::Economy::new();
    let mut mods = mods::Mods::new(sr);
    let mut uni = universe::Universe::new();

    // Rede
    let mut net = net::Net::connect(url.as_deref().unwrap_or(""));
    let mut hello_sent = false;
    let mut reconnect_t = 5.0f32;
    let mut welcomed = false;
    let mut my_id: u64 = 0;
    let mut host_id: u64 = 0;
    let mut remotes: HashMap<u64, Remote> = HashMap::new();
    let mut loopback: Vec<Value> = Vec::new();
    let mut ev_out: Vec<Ev> = Vec::new();
    let mut fpos: Vec<Vec3> = Vec::new();
    let mut vpos: Vec<Vec3> = Vec::new();
    let mut net_t = 0.0f32;
    let mut chat: Vec<(String, f64)> = Vec::new();
    let mut typing: Option<String> = None;
    let mut was_online = false;

    let mut player = Player::new();
    let mut steve = steve::Steve::new();
    let (mut villagers, mut fighters) = spawn_actors();
    let mut npcs = npc::Npcs::new(villagers.len(), guests.len(), extras::ROBOTS);
    let mut voador = voador::Voador::new();
    let mut kaiju = kaiju::Kaiju::new(audio.rate);
    let mut zeppelin = zeppelin::Zeppelin::new(audio.rate);
    let mut mario = mario::Mario::new(audio.rate);
    let mut gumbas = gumba::Gumbas::new();
    let mut bomba = bomba::Bomba::new();
    let mut wolvie = wolvie::Wolvie::new(audio.rate);
    let mut portals = portal::Portals::new(audio.rate);
    let mut cam_smooth: Option<Vec3> = None;
    let mut recent_hits: Vec<(u8, usize, f32)> = Vec::new();
    let mut urna = Urna::new();
    let mut fx = Fx::default();
    let mut texts: Vec<FloatText> = Vec::new();
    let mut events: Vec<Ev> = Vec::new();
    let mut labels: Vec<Label> = Vec::new();
    let mut opaque = Batch::new();
    let mut trans = Batch::new();

    let mut grabbed = false;
    let mut last_mouse: Vec2 = mouse_position().into();
    let mut show_help = !mobile && sk_test.is_none();
    let mut muted = wolvie.demo_on();
    let mut banner: Option<(String, f32)> = Some(("BEM-VINDO A VILA. O HOUSE TA TOCANDO.".into(), 4.0));
    let mut wolverine_called = false;
    // (hp do último frame, segundos de barrinha) de cada lutador; funciona igual no host e nos clientes.
    let mut fighter_bars: Vec<(f32, f32)> = Vec::new();
    let mut was_event = false;
    let mut fireworks_t = 0.0f32;
    let start = get_time();
    let mut time_offset = 0.0f64;
    let up = vec3(0.0, 1.0, 0.0);
    #[cfg(target_arch = "wasm32")]
    let debug = web::query("debug").is_some_and(|v| v == "1");
    #[cfg(not(target_arch = "wasm32"))]
    let debug = std::env::var("URNA_DEBUG").is_ok_and(|v| v == "1");
    let mut prof = prof::Prof::default();
    prof.on = debug;

    loop {
        prof.frame(&if prof.on { format!("{} {} grab={}", quality.label(), bomba.debug(), grabbed as u8) } else { quality.label() });
        let dt = get_frame_time().min(0.05);
        let online = welcomed && net.state() == net::OPEN;
        if was_online && !online {
            banner = Some(("CONEXAO CAIU - MODO OFFLINE".into(), 3.0));
            remotes.clear();
        }
        was_online = online;
        let is_host = !online || my_id == host_id;
        let time = (get_time() - start + time_offset) as f32;
        let beat = time * bpm / 60.0;
        let eye0 = player.eye();
        let in_club = eye0.distance(shield_center()) < SHIELD_R
            || (eye0.x > CLUB_X0 as f32 && eye0.x < CLUB_X1 as f32 + 1.0 && eye0.z > CLUB_Z0 as f32 && eye0.z < CLUB_Z1 as f32 + 1.0);

        // ------------------------------------------------ Rede: mensagens recebidas
        reconnect_t -= dt;
        if url.is_some() && net.state() != net::OPEN && net.state() != net::CONNECTING && reconnect_t <= 0.0 {
            reconnect_t = 5.0;
            net = net::Net::connect(url.as_deref().unwrap_or(""));
            hello_sent = false;
            welcomed = false;
        }
        if !hello_sent && net.state() == net::OPEN {
            net.send(json!({"t": "hello", "n": my_name}).to_string());
            hello_sent = true;
        }
        prof.mark(prof::MISC);
        let mut inbox: Vec<Value> = net.poll().iter().filter_map(|s| serde_json::from_str(s).ok()).collect();
        inbox.append(&mut loopback);

        // Online: servidor ecoa pra todos (ordem única). Offline: loopback local.
        let send = |m: Value, loopback: &mut Vec<Value>| {
            if online {
                net.send(m.to_string());
            } else {
                loopback.push(m);
            }
        };

        let mut is_host = is_host;
        for m in inbox {
            let id = m["id"].as_u64().unwrap_or(0);
            match m["t"].as_str().unwrap_or("") {
                "welcome" => {
                    welcomed = true;
                    my_id = id;
                    host_id = m["host"].as_u64().unwrap_or(0);
                    is_host = my_id == host_id;
                    for p in m["players"].as_array().into_iter().flatten() {
                        let pid = p[0].as_u64().unwrap_or(0);
                        if pid != my_id {
                            remotes.insert(pid, Remote { name: p[1].as_str().unwrap_or("?").into(), pos: Vec3::ZERO, target: Vec3::ZERO, yaw: 0.0, walk: 0.0, look: remote_look(pid), ch: 0, av: None, spd: 0.0, vy: 0.0 });
                        }
                    }
                    world = mods::generate();
                    let mut scratch = Fx::default();
                    portals.clear();
                    for w in m["log"].as_array().into_iter().flatten() {
                        apply_world(&mut world, w, &mut scratch, &atlas.avg);
                        portals.on_world(w);
                    }
                    portals.joined(|o| o == my_id || remotes.contains_key(&o));
                    steve.scan(&world);
                    chunks.build_all(&mut world);
                    if let Some(u) = m["tv"].as_str().filter(|u| *u != tv_src) {
                        tv_src = u.to_string();
                        telao.load(&tv_src);
                    }
                    telao.sync(m["tvEl"].as_f64().unwrap_or(0.0) / 1000.0);
                    chat.push((format!("* conectado como {my_name} ({} online)", remotes.len() + 1), get_time()));
                    banner = Some(("ONLINE! T ABRE O CHAT".into(), 3.0));
                }
                "host" => {
                    host_id = id;
                    is_host = my_id == host_id;
                    if is_host {
                        wolverine_called = fighters[3].spawned;
                    }
                }
                "join" => {
                    let n = m["n"].as_str().unwrap_or("?").to_string();
                    chat.push((format!("* {n} entrou na vila"), get_time()));
                    remotes.insert(id, Remote { name: n, pos: Vec3::ZERO, target: Vec3::ZERO, yaw: 0.0, walk: 0.0, look: remote_look(id), ch: 0, av: None, spd: 0.0, vy: 0.0 });
                }
                "leave" => {
                    portals.remove_owner(id);
                    if let Some(r) = remotes.remove(&id) {
                        chat.push((format!("* {} saiu", r.name), get_time()));
                    }
                }
                "p" => {
                    if let Some(r) = remotes.get_mut(&id) {
                        let p = mp::get_v3(&m["p"]);
                        if r.pos == Vec3::ZERO {
                            r.pos = p;
                        }
                        let d = p - r.target;
                        (r.spd, r.vy) = (vec2(d.x, d.z).length() / 0.15, d.y / 0.15);
                        r.target = p;
                        r.yaw = mp::f(&m["y"]);
                        r.ch = m["c"].as_u64().unwrap_or(0) as u8;
                        r.av = m["av"].as_str().map(String::from);
                    }
                    portals.on_p(id, &m);
                    wolvie.on_p(id, &m);
                    if let Some((dmg, dir)) = steve.on_p(id, &m, my_id) {
                        player.knock += (dir * 6.0 + up * 3.0) * wolvie.damage(dmg * 5.0, dir);
                        if let Some(b) = bandido.as_mut().filter(|b| !b.driving) {
                            b.hurt(dmg * 5.0);
                        }
                    }
                }
                "chat" => {
                    let who = m["n"].as_str().map(String::from).unwrap_or_else(|| who(id, online, my_id, &my_name, &remotes));
                    chat.push((format!("{who}: {}", m["m"].as_str().unwrap_or("")), get_time()));
                }
                "tv" => {
                    tv_src = m["u"].as_str().unwrap_or(telao::DEFAULT_URL).to_string();
                    telao.load(&tv_src);
                    telao.sync(0.0);
                    banner = Some((format!("{} TROCOU O TELAO", m["n"].as_str().map(String::from).unwrap_or_else(|| who(id, online, my_id, &my_name, &remotes))), 2.5));
                }
                "w" => match {
                    let fresh = steve.before_world(&world, &m) && bomba.before_world(&world, &m, &mut steve.tnt);
                    portals.on_world(&m);
                    if fresh { apply_world(&mut world, &m, &mut fx, &atlas.avg) } else { None }
                } {
                    Some((plan, shot)) => {
                        let eye = player.eye();
                        if m["by"].is_null() {
                            urna.recoil();
                            play_at(&audio, &sfx.laser, plan.o, eye, 1.0, muted, in_club);
                        } else if m["by"] == voador::BY {
                            voador.on_shot(&plan, time, &mut fx);
                            play_at(&audio, &sfx.laser, plan.o, eye, 1.0, muted, in_club);
                        } else if m["by"] == kaiju::BY {
                            kaiju.on_shot(&plan, &mut fx);
                        } else if m["by"] == zeppelin::BY {
                            zeppelin.on_shot(&plan, &mut fx);
                        }
                        match shot {
                            Shot::Deflected(p) => {
                                play_at(&audio, &sfx.deflect, p, eye, 1.2, muted, in_club);
                                let club = p.distance(shield_center()) < SHIELD_R + 1.0;
                                texts.push(FloatText { pos: p, text: if club { "ESCUDO DO HOUSE!" } else { "CUPULA DE ENERGIA!" }.into(), color: SKYBLUE, t: 0.0, big: false });
                            }
                            Shot::Exploded(p, r) => {
                                play_at(&audio, &sfx.boom, p, eye, 1.5, muted, in_club);
                                if !in_club {
                                    fx.shake = fx.shake.max((1.0 - p.distance(eye) / 70.0).max(0.0) * 1.2);
                                }
                                if is_host {
                                    bomba.bombs.blast(p, r);
                                    actors::blast_fighters(&mut fighters, p, r, time, &mut events);
                                    actors::blast_villagers(&mut villagers, p, r);
                                    let by_player = !m["by"].is_null();
                                    for (c, rad, i, g) in npc_targets(&fighters, &villagers, &guests, &npcs, &urna, &mods, &kaiju, time).into_iter().chain(gumbas.targets(&npcs)) {
                                        let reach = r * 1.8 + rad * 0.5;
                                        let d = c.distance(p);
                                        if g == npc::FIGHTER || d >= reach || (g >= npc::URNA && !by_player && g != npc::KAIJU) || (g == npc::KAIJU && m["by"] == kaiju::BY) || (g == npc::ZEPPELIN && m["by"] == zeppelin::BY) {
                                            continue;
                                        }
                                        let k = 1.0 - d / reach;
                                        let dmg = if g >= npc::URNA { 150.0 } else { 70.0 } * k;
                                        if npc_hit(&mut npcs, &mut villagers, g, i, dmg, (c - p).normalize_or_zero(), c, &mut events) {
                                            let mut v = mp::shot(&urna::Plan { o: urna.root + up * 1.5, hit: urna.root + up * 1.5, r: 6.0, deflect: false });
                                            v["by"] = json!(2);
                                            send(v, &mut loopback);
                                        }
                                    }
                                }
                                let d = (player.pos + up).distance(p);
                                if d < r * 2.2 {
                                    let k = (1.0 - d / (r * 2.2)) * wolvie.blast(plan.o);
                                    let dir = (player.pos - p).normalize_or_zero();
                                    player.knock += vec3(dir.x, 0.0, dir.z) * 14.0 * k + up * 9.0 * k;
                                    steve.hurt(24.0 * k);
                                    if let Some(b) = bandido.as_mut().filter(|b| !b.driving) {
                                        b.hurt(90.0 * k);
                                    }
                                    bomba.hurt_me(90.0 * k);
                                }
                                let dc = car.pos.distance(p);
                                if dc < r * 2.2 {
                                    car.damage(130.0 * (1.0 - dc / (r * 2.2)));
                                }
                                if r > 5.0 {
                                    texts.push(FloatText { pos: p + up * 3.0, text: "CONFIRMA!!!".into(), color: rgb_green(), t: 0.0, big: true });
                                }
                            }
                        }
                    }
                    None => {
                        if m["k"] == "ai" {
                            let say = m["say"].as_str().unwrap_or("");
                            chat.push((format!("IA (pra {}): {say}", m["n"].as_str().unwrap_or("?")), get_time()));
                            ai_say = Some((say.to_string(), 10.0));
                            for op in m["ops"].as_array().into_iter().flatten() {
                                let s = op["s"].as_f64().unwrap_or(5.0) as f32;
                                match op["op"].as_str().unwrap_or("") {
                                    "banner" => banner = Some((op["text"].as_str().unwrap_or("").to_string(), 4.0)),
                                    "fireworks" => ai_fireworks = s,
                                    "rage" => ai_rage = s,
                                    "sky" => ai_sky = Some((Color::new(op["c"][0].as_f64().unwrap_or(1.0) as f32, op["c"][1].as_f64().unwrap_or(0.0) as f32, op["c"][2].as_f64().unwrap_or(0.0) as f32, 1.0), s)),
                                    "wolverine" if is_host && !fighters[3].spawned => {
                                        fighters[3].spawn_wolverine();
                                        wolverine_called = true;
                                    }
                                    "tp" if op["n"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&my_name)) => {
                                        let to = mp::get_v3(&op["to"]) + vec3(0.5, 0.0, 0.5);
                                        player.pos = to;
                                        player.vel = Vec3::ZERO;
                                        if let Some(s) = skater.as_mut() {
                                            *s = skate::Skater::new(to, s.heading);
                                        }
                                        if let Some(b) = bandido.as_mut() {
                                            b.driving = false;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if m["k"] == "reset" {
                            chunks.build_all(&mut world);
                            let w_on = fighters[3].spawned;
                            (villagers, fighters) = spawn_actors();
                            npcs = npc::Npcs::new(villagers.len(), guests.len(), extras::ROBOTS);
                            if w_on {
                                fighters[3].spawn_wolverine();
                            }
                            banner = Some(("MUNDO RESETADO".into(), 2.0));
                        }
                    }
                },
                "a" if is_host => {
                    let i = m["i"].as_u64().unwrap_or(0) as usize;
                    let fwh = mp::get_v3(&m["d"]);
                    let g = m["g"].as_u64().unwrap_or(0) as u8;
                    let k = match m["k"].as_str().unwrap_or("") {
                        "hit" if g == npc::FIGHTER => "pf",
                        "hit" => "npc",
                        k => k,
                    };
                    match k {
                        "npc" => {
                            let dmg = m["dmg"].as_f64().unwrap_or(6.0) as f32;
                            if npc_hit(&mut npcs, &mut villagers, g, i, dmg, fwh, mp::get_v3(&m["p"]), &mut events) {
                                let mut v = mp::shot(&urna::Plan { o: urna.root + up * 1.5, hit: urna.root + up * 1.5, r: 6.0, deflect: false });
                                v["by"] = json!(2);
                                send(v, &mut loopback);
                            }
                        }
                        "pf" if i < fighters.len() => {
                            let f = &mut fighters[i];
                            if f.active() {
                                f.hp -= m["dmg"].as_f64().unwrap_or(6.0) as f32;
                                f.vel += fwh * 7.0 + up * 3.0;
                                f.flash = 1.0;
                                if f.hp <= 0.0 {
                                    f.hp = 0.0;
                                    f.state = FState::Ko(5.0);
                                    f.ko_t = 0.0;
                                } else {
                                    f.state = FState::Stun(0.3);
                                }
                                events.push(Ev::Text { pos: f.pos + up * 2.2, text: format!("{} DE {}!", m["w"].as_str().unwrap_or("SOCO"), who(id, online, my_id, &my_name, &remotes)), color: YELLOW, big: false });
                                events.push(Ev::Hit { pos: f.pos + up, claws: false });
                            }
                        }
                        "pv" if i < villagers.len() => {
                            let v = &mut villagers[i];
                            v.airborne = true;
                            v.vel = fwh * 8.0 + up * 7.0;
                            let at = v.pos + up;
                            events.push(Ev::Text { pos: v.pos + up * 2.2, text: "HMMM!".into(), color: WHITE, big: false });
                            npc_hit(&mut npcs, &mut villagers, npc::VILLAGER, i, 10.0, fwh, at, &mut events);
                        }
                        "wolv" if !fighters[3].spawned => {
                            fighters[3].spawn_wolverine();
                            wolverine_called = true;
                        }
                        "bomb" | "bombx" | "kick" => bomba.on_action(&world, id, &m),
                        _ => {}
                    }
                }
                "s" if !is_host => {
                    let target = mp::f(&m["time"]) as f64 - (get_time() - start);
                    time_offset = if (target - time_offset).abs() > 1.0 { target } else { time_offset + (target - time_offset) * 0.1 };
                    mp::apply_snapshot(&m, &mut urna, &mut fighters, &mut villagers, &mut fpos, &mut vpos, &mut events);
                    npcs.apply(&m["n"]);
                    mods.apply(&m["md"]);
                    kaiju.apply(&m["kj"]);
                    mario.apply(&m["mr"]);
                    gumbas.apply(&m["gb"]);
                    bomba.apply(&m["bb"]);
                }
                "eco" => eco.on_msg(&m, &mut chat),
                "lab" => lab_info.on_msg(&m),
                "mod" | "mods" => mods.on_msg(&m),
                "hub" | "hub_s" => {
                    places.on_hub(&m);
                    hub.on_msg(&m);
                }
                "pl" => places.on_msg(&m),
                "uv_list" | "uv_me" | "uv_pkg" | "uv_bag" | "uv_ctok" => uni.on_msg(&m),
                _ => {}
            }
        }

        prof.mark(prof::NET);
        // ------------------------------------------------ Toque (celular)
        let (sw, sh) = (screen_width(), screen_height());
        let ch: u8 = match (&skater, &bandido) {
            (Some(_), _) => 1,
            (_, Some(b)) => 2 + b.driving as u8,
            _ if niko.is_some() => 4,
            _ if portals.active => 5,
            _ if mario.me.is_some() => 6,
            _ if wolvie.me.is_some() => 7,
            _ if bomba.me.is_some() => 8,
            _ => 0,
        };
        let mut buttons = touch_buttons(sw, sh, ch);
        if ch == 0 && !steve.creative {
            buttons[3].2 = "AGACHA";
        }
        if ch == 7 && wolvie.rage_full() {
            buttons[3].2 = "FURIA";
        }
        let (mut tap_hit, mut tap_place) = (false, false);
        let (mut pick_char, mut car_toggle, mut sk_ollie, mut cycle_weapon): (Option<usize>, bool, bool, i32) = (None, false, false, 0);
        let mut niko_dive = false;
        let mut clip_tap = false;
        let mut m_tap = [false; 3];
        let mut w_tap = [false; 4];
        let mut b_tap = [false; 4];
        let ts = touches();
        if !ts.is_empty() && !mobile {
            mobile = true;
            show_help = false;
            if quality.auto {
                quality = quality::Quality::new(true);
            }
        }
        if let Some(b) = quality.update(dt) {
            banner = Some((b, 2.5));
        }
        let slot = if mobile { (sw / 13.0).min(48.0) } else { 48.0 };
        for t in &ts {
            let p = t.position;
            match t.phase {
                TouchPhase::Started if chars_open && quality_button(sw, sh).contains(p) => banner = Some((quality.cycle(mobile), 2.0)),
                TouchPhase::Started if chars_open => {
                    if let Some(b) = uni.click(p, sw, sh) {
                        banner = Some((b, 2.0));
                        chars_open = false;
                        continue;
                    }
                    pick_char = char_cards(sw, sh).iter().position(|r| r.contains(p));
                    if pick_char.is_none() {
                        chars_open = false;
                    }
                }
                TouchPhase::Started if steve.inv_open => steve.inv_click(&mut player.sel, p, sw, sh),
                TouchPhase::Started => {
                    let hot = if ch == 0 { inventory::hotbar_hit(p, sw, sh, slot, mobile) } else { None };
                    let (lc, lr) = hub::link_button(sw, sh);
                    if let Some(k) = hot {
                        if k < 9 {
                            player.sel = k;
                        } else {
                            steve.inv_open = true;
                        }
                    } else if let Some(u) = hub.aim_url().filter(|_| p.distance(lc) < lr) {
                        chat.push((hub::open_url(&u), get_time()));
                    } else if let Some(b) = buttons.iter().position(|(c, r, _)| p.distance(*c) < *r) {
                        held.insert(t.id, b);
                        match (b, ch) {
                            (0, 1) => sk_ollie = true,
                            (1..=3, 5) => portals.tap(b),
                            (0..=2, 6) => m_tap[b] = true,
                            (0..=3, 7) => w_tap[b] = true,
                            (1..=3, 8) => b_tap[b] = true,
                            (1, 0) => tap_hit = true,
                            (2, 0) => tap_place = true,
                            (2, 2) => cycle_weapon = 1,
                            (3, 2) | (3, 3) => car_toggle = true,
                            (3, 0) if player.can_fly => {
                                player.fly = !player.fly;
                                player.vel.y = 0.0;
                            }
                            (6, _) => chars_open = true,
                            (7, _) => eco.show = !eco.show,
                            (8, _) => clip_tap = true,
                            (1, 4) => niko_dive = true,
                            (4, _) => {
                                if let Some(m) = ask_text("Mensagem pro chat:") {
                                    match hub.link(&m) {
                                        Some(u) => chat.push((hub::open_url(&u), get_time())),
                                        None => send(json!({"t": "chat", "m": m}), &mut loopback),
                                    }
                                }
                            }
                            (5, _) => {
                                if let Some(u) = ask_text("Cola o link do YouTube pro telao:") {
                                    send(json!({"t": "tv", "u": u}), &mut loopback);
                                }
                            }
                            _ => {}
                        }
                    } else if p.x < sw * 0.45 && stick.is_none() {
                        stick = Some((t.id, p));
                    } else if look_touch.is_none() {
                        look_touch = Some((t.id, p));
                        look_origin = p;
                    }
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if let Some((id, last)) = look_touch.filter(|l| l.0 == t.id) {
                        if ch == 1 {
                            skate_touch = Some(((p - look_origin) / (sh * 0.12)).clamp_length_max(1.0) * vec2(1.0, -1.0));
                        } else {
                            player.look((p - last) * 2.2);
                        }
                        look_touch = Some((id, p));
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    held.remove(&t.id);
                    if stick.is_some_and(|s| s.0 == t.id) {
                        stick = None;
                    }
                    if look_touch.is_some_and(|l| l.0 == t.id) {
                        look_touch = None;
                        skate_touch = None;
                    }
                }
            }
        }
        let stick_r = sh.min(sw) * 0.13;
        player.stick = match stick {
            Some((id, o)) => ts.iter().find(|t| t.id == id).map(|t| ((t.position - o) / stick_r).clamp_length_max(1.0) * vec2(1.0, -1.0)).unwrap_or(Vec2::ZERO),
            None => Vec2::ZERO,
        };
        player.jump_held = held.values().any(|b| *b == 0);
        player.sneak_held = ch == 0 && !steve.creative && held.values().any(|b| *b == 3);

        // ------------------------------------------------ Input
        let mut just_grabbed = false;
        if steve.inv_open && is_mouse_button_pressed(MouseButton::Left) {
            steve.inv_click(&mut player.sel, mouse_position().into(), sw, sh);
        } else if !mobile && !grabbed && !steve.inv_open && is_mouse_button_pressed(MouseButton::Left) {
            grabbed = true;
            just_grabbed = true;
            set_cursor_grab(true);
            show_mouse(false);
        }
        if grabbed && typing.is_none() && ((is_key_pressed(KeyCode::Tab) && ch != 7) || is_key_pressed(KeyCode::Escape)) {
            grabbed = false;
            set_cursor_grab(false);
            show_mouse(true);
        }
        let mouse: Vec2 = mouse_position().into();
        let md = mouse - last_mouse;
        last_mouse = mouse;
        if grabbed && !just_grabbed && ch != 1 && !chars_open && !steve.inv_open {
            player.look(md);
        }
        if chars_open && is_mouse_button_pressed(MouseButton::Left) {
            if quality_button(sw, sh).contains(mouse) {
                banner = Some((quality.cycle(mobile), 2.0));
            } else if let Some(b) = uni.click(mouse, sw, sh) {
                banner = Some((b, 2.0));
                chars_open = false;
            } else {
                pick_char = char_cards(sw, sh).iter().position(|r| r.contains(mouse));
            }
        }
        if let Some(msg) = typing.as_mut() {
            if type_into(msg, 120) {
                let m = msg.trim().to_string();
                if let Some(u) = hub.link(&m) {
                    chat.push((hub::open_url(&u), get_time()));
                    grabbed = false;
                    set_cursor_grab(false);
                    show_mouse(true);
                } else if !m.is_empty() {
                    send(json!({"t": "chat", "m": m}), &mut loopback);
                }
                typing = None;
            } else if is_key_pressed(KeyCode::Escape) {
                typing = None;
            }
        } else {
            if is_key_pressed(KeyCode::T) || is_key_pressed(KeyCode::Enter) {
                while get_char_pressed().is_some() {}
                typing = Some(String::new());
            }
            if is_key_pressed(KeyCode::H) {
                show_help = !show_help;
            }
            if is_key_pressed(KeyCode::F3) {
                prof.toggle();
            }
            if is_key_pressed(KeyCode::F4) {
                banner = Some((quality.cycle(mobile), 2.0));
            }
            if is_key_pressed(KeyCode::M) {
                muted = !muted;
            }
            clip_tap |= is_key_pressed(KeyCode::B);
            if is_key_pressed(KeyCode::Y) {
                #[cfg(target_arch = "wasm32")]
                let src = web::prompt("Cola o link do YouTube pro telao:");
                #[cfg(not(target_arch = "wasm32"))]
                let src = macroquad::miniquad::window::clipboard_get();
                match src.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                    Some(src) => send(json!({"t": "tv", "u": src}), &mut loopback),
                    None => banner = Some(("COPIE UM LINK DO YOUTUBE E APERTE Y".into(), 2.5)),
                }
            }
            if is_key_pressed(KeyCode::K) && !fighters[3].spawned {
                send(json!({"t": "a", "k": "wolv"}), &mut loopback);
            }
            if is_key_pressed(KeyCode::R) && ch != 7 {
                send(json!({"t": "w", "k": "reset"}), &mut loopback);
            }
            if is_key_pressed(KeyCode::C) {
                chars_open = !chars_open;
                if chars_open && grabbed {
                    grabbed = false;
                    set_cursor_grab(false);
                    show_mouse(true);
                }
            }
            let sign = hub.aim_url().filter(|_| grabbed && !chars_open && !steve.inv_open && (is_key_pressed(KeyCode::E) || (is_mouse_button_pressed(MouseButton::Left) && !just_grabbed)));
            if let Some(u) = &sign {
                chat.push((hub::open_url(u), get_time()));
                grabbed = false;
                set_cursor_grab(false);
                show_mouse(true);
            }
            let inv_key = sign.is_none() && (is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::I) || (steve.inv_open && is_key_pressed(KeyCode::Escape)));
            if ch == 0 && !chars_open && inv_key {
                steve.inv_open = !steve.inv_open;
                grabbed = !steve.inv_open && !mobile;
                set_cursor_grab(grabbed);
                show_mouse(!grabbed);
            }
            if ch >= 2 && is_key_pressed(KeyCode::F) {
                car_toggle = true;
            }
            if ch == 2 {
                cycle_weapon += is_key_pressed(KeyCode::E) as i32 - is_key_pressed(KeyCode::Q) as i32;
            }
            if ch == 4 && is_key_pressed(KeyCode::G) {
                niko_dive = true;
            }
            for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9].iter().enumerate() {
                if is_key_pressed(*key) {
                    if chars_open {
                        pick_char = (k < CHARS.len()).then_some(k);
                    } else if let Some(b) = bandido.as_mut() {
                        b.weapon = k;
                    } else {
                        player.sel = k;
                    }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            if clip_tap {
                web::clip_save();
            }
            if let Some(m) = web::clip_poll(quality::tier()) {
                chat.push((m, get_time()));
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        if clip_tap {
            banner = Some(("CLIPE SO NO NAVEGADOR".into(), 2.0));
        }
        if wolvie.demo_pick() {
            pick_char = CHARS.iter().position(|c| c.2 == 7);
        }
        if let Some(m) = pick_char {
            let c = CHARS[m].2;
            chars_open = false;
            if let Some(s) = skater.take() {
                player.pos = s.pos;
                player.yaw = (s.heading.cos()).atan2(s.heading.sin());
            }
            if bandido.take().is_some_and(|b| b.driving) {
                player.pos = car.pos + vec3(0.0, 2.0, 0.0);
            }
            niko = None;
            mario.me = None;
            wolvie.me = None;
            bomba.me = None;
            portals.active = c == 5;
            player.vel = Vec3::ZERO;
            player.fly = false;
            let fw = player.forward();
            match c {
                2 => skater = Some(skate::Skater::new(player.pos, fw.x.atan2(fw.z))),
                3 => bandido = Some(gta::Bandido::new(fw.x.atan2(fw.z))),
                4 => niko = Some(ragdoll::Ragdoll::new(player.pos, fw.x.atan2(fw.z))),
                5 => {}
                6 => mario.me = Some(mario::Body::new(player.pos, fw.x.atan2(fw.z))),
                7 => wolvie.me = Some(wolvie::Body::new(player.pos, fw.x.atan2(fw.z))),
                8 => bomba.start_me(),
                _ => steve.set_mode(c == 1, &mut player),
            }
            banner = Some((format!("PERSONAGEM: {}", CHARS[m].0), 2.0));
        }
        if car_toggle {
            if let Some(b) = bandido.as_mut() {
                if b.driving {
                    b.driving = false;
                    car.door = 1.0;
                    let l = vec3(car.fwd().z, 0.0, -car.fwd().x);
                    player.pos = [car.pos + l * 1.8, car.pos - l * 1.8].into_iter().find(|p| !player.collides_at(&world, *p)).unwrap_or(car.pos + up * 2.0);
                    player.vel = Vec3::ZERO;
                    b.body_yaw = car.yaw;
                } else if player.pos.distance(car.pos) < 4.0 && car.wreck > 0.0 {
                    banner = Some(("ESSE AI JA ERA. ESPERA OUTRO APARECER NA VAGA".into(), 2.0));
                } else if player.pos.distance(car.pos) < 4.0 {
                    b.driving = true;
                    b.aiming = false;
                    car.door = 1.0;
                } else {
                    banner = Some(("CHEGA PERTO DO CARRO (PERTO DA TORRE)".into(), 2.0));
                }
            }
        }
        if let Some(b) = bandido.as_mut() {
            b.cycle(cycle_weapon);
        }
        if is_host && !wolverine_called && time > 20.0 {
            fighters[3].spawn_wolverine();
            wolverine_called = true;
            events.push(Ev::Banner("TEM ALGO CAINDO DO CEU...".into()));
        }
        let wheel = mouse_wheel().1;
        if let Some(b) = bandido.as_mut() {
            b.cycle(-(wheel.signum() as i32) * (wheel != 0.0) as i32);
        } else if wheel > 0.0 && ch != 8 {
            player.sel = (player.sel + 8) % 9;
        } else if wheel < 0.0 && ch != 8 {
            player.sel = (player.sel + 1) % 9;
        }

        prof.mark(prof::INPUT);
        let active = (grabbed || mobile) && typing.is_none() && !chars_open && !steve.inv_open;
        let key = |k: KeyCode| active && is_key_down(k);
        let btn = |i: usize| held.values().any(|b| *b == i);
        let steer = ((key(KeyCode::A) as i32 - key(KeyCode::D) as i32) as f32 - player.stick.x).clamp(-1.0, 1.0);
        let pos_before = player.pos;
        let wasted = bandido.as_ref().is_some_and(|b| b.wasted > 0.0);
        let active = active && !wasted;
        let mut moving = false;
        if let Some(sk) = skater.as_mut() {
            let inp = skate::Input {
                push: key(KeyCode::W) || player.stick.y > 0.5,
                brake: key(KeyCode::S) || player.stick.y < -0.5,
                steer,
                mouse: if active && grabbed && !just_grabbed { md } else { Vec2::ZERO },
                touch: skate_touch,
                grab_front: key(KeyCode::Q) || (active && grabbed && is_mouse_button_down(MouseButton::Left)) || btn(1),
                grab_back: key(KeyCode::E) || (active && grabbed && is_mouse_button_down(MouseButton::Right)) || btn(3),
                manual: key(KeyCode::LeftShift) || btn(2),
                ollie: sk_ollie || (active && is_key_pressed(KeyCode::Space)),
            };
            match sk_test.as_mut() {
                Some(t) => {
                    let inp = t.input(sk);
                    sk.update(&world, t.dt(), &inp);
                }
                None => sk.update(&world, dt, &inp),
            }
            if player.knock.length() > 3.0 {
                sk.bail_now("EXPLOSAO");
            }
            player.knock = Vec3::ZERO;
            player.pos = sk.pos;
            if sk.landed || sk.bailed {
                play_at(&audio, &sfx.punch, sk.pos, sk.pos, if sk.bailed { 1.0 } else { 0.4 }, muted, false);
            }
            if sk.sparks {
                fx.particles.push(urna::Particle { pos: sk.pos + up * 0.05, vel: vec3(gen_range(-2.0, 2.0), gen_range(1.0, 3.0), gen_range(-2.0, 2.0)), col: Color::new(1.0, 0.8, 0.3, 1.0), life: 0.3, size: 0.06, gravity: false });
            }
        } else if bandido.as_ref().is_some_and(|b| b.driving) {
            let throttle = (key(KeyCode::W) as i32 - key(KeyCode::S) as i32) as f32 + player.stick.y;
            let crash = car.update(&world, dt, throttle.clamp(-1.0, 1.0), steer, key(KeyCode::Space) || btn(0));
            if crash > 5.0 {
                play_at(&audio, &sfx.punch, car.pos, car.pos, (crash / 15.0).min(1.5), muted, false);
                fx.shake = fx.shake.max((crash / 30.0).min(0.6));
            }
            player.pos = car.pos;
            player.knock = Vec3::ZERO;
        } else if mario.me.is_some() {
            let f = vec3(player.yaw.cos(), 0.0, player.yaw.sin());
            let r = vec3(-f.z, 0.0, f.x);
            let keys = f * (key(KeyCode::W) as i32 - key(KeyCode::S) as i32) as f32 + r * (key(KeyCode::D) as i32 - key(KeyCode::A) as i32) as f32;
            let stick = if active { player.stick } else { Vec2::ZERO };
            let inp = mario::Intent {
                mv: if stick.length() > 0.1 { f * stick.y + r * stick.x } else { keys.normalize_or_zero() },
                jump: m_tap[0] || (active && is_key_pressed(KeyCode::Space)),
                hold: key(KeyCode::Space) || btn(0),
                crouch: key(KeyCode::LeftShift) || btn(2),
                punch: m_tap[1] || (active && is_key_pressed(KeyCode::E)),
            };
            let mut tg = npc_targets(&fighters, &villagers, &guests, &npcs, &urna, &mods, &kaiju, time);
            tg.extend(mario.target(&npcs));
            tg.extend(gumbas.targets(&npcs));
            tg.extend(bomba.target(&npcs));
            let before = player.pos;
            for v in mario.play(&world, dt, &inp, std::mem::take(&mut player.knock), &tg) {
                send(v, &mut loopback);
            }
            player.pos = mario.me.as_ref().map_or(player.pos, |b| b.pos);
            player.vel = Vec3::ZERO;
            let d = vec2(player.pos.x - before.x, player.pos.z - before.z).length();
            moving = d > 0.001;
            gta_walk += d * 3.0;
        } else if wolvie.me.is_some() {
            let inp = wolvie::Intent::read(active, active && grabbed && !just_grabbed, player.stick, w_tap, [btn(0), btn(1), btn(2), btn(3)], wolvie.rage_full());
            let mut tg = npc_targets(&fighters, &villagers, &guests, &npcs, &urna, &mods, &kaiju, time);
            tg.extend(mario.target(&npcs));
            tg.extend(gumbas.targets(&npcs));
            tg.extend(bomba.target(&npcs));
            let others: Vec<(u64, Vec3)> = remotes.iter().map(|(id, r)| (*id, r.pos)).collect();
            let before = player.pos;
            for v in wolvie.play(&world, dt, &inp, &mut player, &tg, &others, &mut fx) {
                send(v, &mut loopback);
            }
            let d = vec2(player.pos.x - before.x, player.pos.z - before.z).length();
            moving = d > 0.001;
            gta_walk += d * 3.0;
        } else if niko.as_ref().is_none_or(|n| n.controlled()) {
            player.can_fly = bandido.is_none() && niko.is_none() && steve.creative && !portals.active && bomba.me.is_none();
            player.mc = ch == 0;
            let (before, vy) = (player.pos, player.vel.y);
            if let Some(n) = niko.as_mut() {
                let imp = std::mem::take(&mut player.knock);
                if imp.length() > 2.0 {
                    n.hit(imp * 1.2, dt);
                    play_at(&audio, &sfx.punch, player.pos, player.pos, 0.8, muted, false);
                }
            }
            portals.pre_move(&world, &mut player, dt, active && niko.as_ref().is_none_or(|n| n.controlled()));
            player.update(&world, dt, active && niko.as_ref().is_none_or(|n| n.controlled()));
            let d = vec2(player.pos.x - before.x, player.pos.z - before.z).length();
            moving = d > 0.001;
            gta_walk += d * 3.0;
            if let Some(n) = niko.as_mut() {
                let fwh = vec3(player.forward().x, 0.0, player.forward().z).normalize_or_zero();
                if niko_dive {
                    n.hit(fwh * 7.0 + up * 2.5, dt);
                } else if vy < -14.0 && player.on_ground {
                    n.hit(fwh * 4.0 + up * 10.0, dt);
                }
            }
        }
        if let Some(n) = niko.as_mut() {
            let fw = player.forward();
            if let Some(r) = n.update(&world, dt, player.pos, fw.x.atan2(fw.z), gta_walk, moving as i32 as f32, time) {
                player.pos = r;
                player.vel = Vec3::ZERO;
            }
            if !n.controlled() {
                let pv = n.pelvis();
                player.pos = vec3(pv.x, world.floor_at(pv.x, pv.y, pv.z), pv.z);
                player.knock = Vec3::ZERO;
            }
        }
        if ch == 8 {
            let mouse_ok = grabbed && !just_grabbed && active;
            let keys = if active { is_key_pressed(KeyCode::E) as i32 - is_key_pressed(KeyCode::Q) as i32 - wheel.signum() as i32 * (wheel != 0.0) as i32 } else { 0 };
            let inp = bomba::Intent {
                place: b_tap[1] || (active && is_key_pressed(KeyCode::F)) || (mouse_ok && is_mouse_button_pressed(MouseButton::Left)),
                cycle: b_tap[2] as i32 + keys,
                detonate: b_tap[3] || (active && is_key_pressed(KeyCode::G)) || (mouse_ok && is_mouse_button_pressed(MouseButton::Right)),
            };
            for v in bomba.play(&world, dt, &inp, &mut player, pos_before, my_id) {
                send(v, &mut loopback);
            }
            if let Some(s) = bomba.msg.take() {
                banner = Some((s, 2.0));
            }
        } else if matches!(ch, 0 | 5) {
            bomba.block(&mut player, pos_before);
        }
        // COGUMAU: pisão (quem anda pelo player; o ENCANADOR pisa pelos próprios golpes) ou mordida de lado
        let feet = match ch {
            0 | 2 | 4 | 5 => Some((player.pos, player.vel.y, true)),
            6 => mario.me.as_ref().map(|b| (b.pos, 0.0, false)),
            7 => Some((player.pos, 0.0, false)),
            _ => None,
        };
        if let Some((p, vy, can_stomp)) = feet {
            let t = gumbas.touch(&npcs, p, vy, can_stomp);
            if t.bounce {
                player.vel.y = 9.0;
                play_at(&audio, &sfx.punch, p, p, 0.7, muted, false);
            }
            if t.hurt > 0.0 {
                player.knock += (t.dir * 5.0 + up * 3.0) * wolvie.damage(10.0, t.dir);
                steve.hurt(3.0);
                if let Some(b) = bandido.as_mut().filter(|b| !b.driving) {
                    b.hurt(15.0);
                }
            }
            for v in t.out {
                send(v, &mut loopback);
            }
        }
        let (hub_msg, hub_release) = hub.update(&mut player, dt, online, remote_look(my_id).shirt, ch);
        if let Some(m) = hub_msg {
            send(m, &mut loopback);
        }
        if hub_release {
            grabbed = false;
            set_cursor_grab(false);
            show_mouse(true);
        }
        if let Some(b) = hub.msg.take() {
            banner = Some(b);
        }
        places.ringue.jab = (grabbed && !just_grabbed && active && is_mouse_button_pressed(MouseButton::Left)) || ts.iter().any(|t| t.phase == TouchPhase::Started && buttons.get(1).is_some_and(|b| t.position.distance(b.0) < b.1));
        for m in places.update(&mut player, dt, time, online) {
            if online {
                net.send(m.to_string());
            }
        }
        if let Some(b) = places.ringue.msg.take() {
            banner = Some((b, 2.0));
        }
        if std::mem::take(&mut places.ringue.heal) {
            steve.hp = steve::MAX_HP;
        }
        let laws = places.laws();
        (player.grav, player.speed_mul) = (if laws.lua { 0.35 } else { 1.0 }, if laws.turbo { 1.6 } else { 1.0 });
        if laws.festa {
            ai_fireworks = ai_fireworks.max(0.2);
        }
        // Carro: física mesmo estacionado, dano, fumaça, explosão e ronco do motor
        let driving_now = bandido.as_ref().is_some_and(|b| b.driving);
        if !driving_now {
            car.update(&world, dt, 0.0, 0.0, true);
        }
        if car.tick(&world, dt) {
            let mut v = mp::shot(&urna::Plan { o: car.pos + up, hit: car.pos + up, r: 3.5, deflect: false });
            v["by"] = json!(1);
            send(v, &mut loopback);
            if let Some(b) = bandido.as_mut() {
                b.crime(1.0);
                if b.driving {
                    b.driving = false;
                    player.pos = car.pos + up * 2.5;
                    player.vel = Vec3::ZERO;
                }
            }
        }
        if let Some((p, col, fire)) = car.smoke() {
            let jitter = vec3(gen_range(-0.3, 0.3), 0.0, gen_range(-0.3, 0.3));
            fx.particles.push(urna::Particle { pos: p + jitter, vel: vec3(gen_range(-0.5, 0.5), gen_range(2.0, 3.5), gen_range(-0.5, 0.5)), col, life: gen_range(1.0, 1.8), size: gen_range(0.4, 0.8), gravity: false });
            if fire {
                fx.particles.push(urna::Particle { pos: p + jitter * 2.0, vel: vec3(0.0, gen_range(1.0, 2.5), 0.0), col: Color::new(1.0, gen_range(0.3, 0.7), 0.05, 1.0), life: 0.4, size: gen_range(0.3, 0.6), gravity: false });
            }
        }
        let rev = (car.speed().abs() / 28.0 * 3.0 + if driving_now && (key(KeyCode::W) || key(KeyCode::S) || player.stick.y.abs() > 0.3) { 0.6 } else { 0.0 }).min(3.0);
        for (k, id) in engine.iter().enumerate() {
            let v = if driving_now && !muted && car.wreck <= 0.0 { (1.0 - (rev - k as f32).abs()).max(0.0) * 0.3 } else { 0.0 };
            audio.set_volume(*id, v);
        }
        let (eye, fw) = if let Some(sk) = &skater {
            let (e, t) = sk.camera();
            (e, (t - e).normalize())
        } else if let Some(b) = bandido.as_mut() {
            // Câmera solta atrás (a pé), por cima do ombro mirando, mira telescópica na sniper/bazuca,
            // e no carro atrasada na velocidade (drift aparece). Posição suavizada, direção não (mira precisa).
            b.aiming = !b.driving && b.wasted <= 0.0 && grabbed && active && is_mouse_button_down(MouseButton::Right);
            let (want_e, look) = if b.driving {
                let f = car.fwd();
                let back = -(car.vel.normalize_or(f) * 0.35 + f * 0.65).normalize_or(f);
                let e = car.pos + up * 3.0 + back * 7.5;
                (e, Some(car.pos + up * 1.2 + f * 4.0))
            } else {
                let fw = player.forward();
                let right = vec3(-fw.z, 0.0, fw.x).normalize_or_zero();
                let head = player.pos + up * 1.65;
                let want = if b.aiming && matches!(b.weapon, 7 | 8) {
                    fw * 0.35 + right * 0.12
                } else if b.aiming {
                    -fw * 1.9 + right * 0.75 + up * 0.1
                } else {
                    -fw * 3.6 + right * 0.45 + up * 0.35
                };
                let dist = world.raycast(head, want.normalize(), want.length()).map(|h| (h.2 - 0.3).max(0.2)).unwrap_or(want.length());
                (head + want.normalize() * dist, None)
            };
            let k = 1.0 - (-dt * if b.driving { 6.0 } else if b.aiming { 25.0 } else { 14.0 }).exp();
            let e = match cam_smooth {
                Some(c) if c.distance(want_e) < 15.0 => c.lerp(want_e, k),
                _ => want_e,
            };
            cam_smooth = Some(e);
            (e, look.map(|l| (l - e).normalize()).unwrap_or(player.forward()))
        } else if let Some(c) = wolvie.camera(&world, &player, dt) {
            c
        } else if niko.is_some() || mario.me.is_some() || bomba.me.is_some() {
            let fw = player.forward();
            let right = vec3(-fw.z, 0.0, fw.x).normalize_or_zero();
            let head = player.pos + up * 1.7;
            let want = -fw * 3.2 + right * 0.6;
            let dist = world.raycast(head, want.normalize(), want.length()).map(|h| (h.2 - 0.3).max(0.2)).unwrap_or(want.length());
            (head + want.normalize() * dist, fw)
        } else {
            (player.eye(), player.forward())
        };
        let pick = if ch == 0 { world.raycast(eye, fw, 6.0) } else { None };
        hub.look(eye, fw);

        let mut targets = npc_targets(&fighters, &villagers, &guests, &npcs, &urna, &mods, &kaiju, time);
        targets.extend(mario.target(&npcs));
        targets.extend(gumbas.targets(&npcs));
        targets.extend(bomba.target(&npcs));
        if let Some(b) = bandido.as_mut() {
            let mut outs = Vec::new();
            if b.driving {
                let spd = car.speed().abs();
                if spd > 4.0 && b.hit_cd <= 0.0 {
                    for t in &targets {
                        if vec2(t.0.x - car.pos.x, t.0.z - car.pos.z).length() < 2.4 && (t.0.y - car.pos.y).abs() < 2.5 {
                            outs.push(gta::Out::Hit { i: t.2, g: t.3, dmg: spd * 1.5, dir: car.fwd(), at: t.0 });
                            b.hit_cd = 0.3;
                            b.crime(0.25);
                            car.damage(spd * 0.15);
                        }
                    }
                }
            } else if b.wasted <= 0.0 {
                let wpn = &gta::ARSENAL[b.weapon];
                let mouse_fire = grabbed && !just_grabbed && active && if wpn.auto { is_mouse_button_down(MouseButton::Left) } else { is_mouse_button_pressed(MouseButton::Left) };
                // Mira travada: botão direito (menos sniper/bazuca, que é na mão) ou automática no celular
                if b.aiming && !matches!(b.weapon, 7 | 8) {
                    b.pick_lock(&world, eye, fw, &targets, 0.95);
                } else if btn(1) {
                    b.pick_lock(&world, eye, fw, &targets, 0.8);
                } else if !b.aiming {
                    b.lock = None;
                }
                if mouse_fire || btn(1) {
                    let muzzle = b.muzzle(player.pos, fw);
                    b.fire(&world, eye, fw, muzzle, player.pos + up, &targets, &mut outs);
                }
                let k = b.take_kick(dt);
                if k != 0.0 {
                    player.pitch = (player.pitch + k).clamp(-1.55, 1.55);
                    player.yaw += gen_range(-0.3, 0.3) * k.max(0.0);
                }
            }
            b.update(&world, dt, &targets, &mut outs, fw, player.pos - pos_before);
            if b.wasted > 0.0 {
                b.wasted -= dt;
                if b.wasted <= 0.0 {
                    let cash = b.cash - b.cash / 10;
                    *b = gta::Bandido::new(fw.x.atan2(fw.z));
                    b.cash = cash;
                    player.pos = player::Player::spawn();
                    player.vel = Vec3::ZERO;
                    banner = Some(("HOSPITAL DA VILA: -10% DA GRANA".into(), 3.0));
                }
            }
            let wname = if b.driving { "ATROPELADO" } else { gta::ARSENAL[b.weapon].name };
            for o in outs {
                match o {
                    gta::Out::Hit { i, g, dmg, dir, at } => {
                        send(json!({"t": "a", "k": "hit", "g": g, "i": i, "d": mp::v3(dir), "p": mp::v3(at), "dmg": dmg, "w": wname}), &mut loopback);
                        b.crime(if g == npc::FIGHTER { 0.04 } else { 0.08 });
                        b.cash += dmg as u32;
                        if !recent_hits.iter().any(|h| (h.0, h.1) == (g, i)) {
                            recent_hits.push((g, i, time));
                        }
                        let metal = matches!(g, npc::ROBOT | npc::URNA | npc::EU | npc::GUARD);
                        for _ in 0..if metal { 5 } else { 8 } {
                            let col = if metal { Color::new(1.0, 0.85, 0.4, 1.0) } else { Color::new(gen_range(0.5, 0.75), 0.02, 0.02, 1.0) };
                            fx.particles.push(urna::Particle { pos: at, vel: dir * gen_range(1.0, 4.0) + vec3(gen_range(-1.5, 1.5), gen_range(0.5, 3.0), gen_range(-1.5, 1.5)), col, life: gen_range(0.3, 0.7), size: gen_range(0.05, 0.12), gravity: !metal });
                        }
                    }
                    gta::Out::Boom(p, r) => {
                        let mut v = mp::shot(&urna::Plan { o: p, hit: p, r, deflect: false });
                        v["by"] = json!(1);
                        send(v, &mut loopback);
                    }
                    gta::Out::Spark(p) => {
                        for _ in 0..4 {
                            fx.particles.push(urna::Particle { pos: p, vel: vec3(gen_range(-3.0, 3.0), gen_range(0.0, 3.0), gen_range(-3.0, 3.0)), col: Color::new(1.0, 0.85, 0.4, 1.0), life: 0.25, size: 0.05, gravity: false });
                        }
                        for _ in 0..3 {
                            fx.particles.push(urna::Particle { pos: p, vel: vec3(gen_range(-1.5, 1.5), gen_range(1.0, 3.0), gen_range(-1.5, 1.5)), col: Color::new(0.45, 0.42, 0.38, 1.0), life: gen_range(0.3, 0.6), size: 0.08, gravity: true });
                        }
                    }
                    gta::Out::Bang(p) => play_at(&audio, &sfx.gun, p, eye, 0.8, muted, in_club),
                    gta::Out::Casing(p, v) => fx.particles.push(urna::Particle { pos: p, vel: v, col: Color::new(0.85, 0.65, 0.25, 1.0), life: 0.8, size: 0.04, gravity: true }),
                }
            }
            // Quem eu acertei e morreu em seguida conta como abate: mais procurado, mais grana
            recent_hits.retain(|&(g, i, t)| {
                let dead = if g == npc::FIGHTER { !fighters[i].active() } else { !npcs.alive(g, i) };
                if dead {
                    b.crime(if g >= npc::URNA { 3.0 } else { 0.7 });
                    let reward = match g {
                        npc::URNA => 5000,
                        npc::EU => 2000,
                        npc::VOADOR => 2500,
                        npc::KAIJU => 4000,
                        npc::ZEPPELIN => 6000,
                        npc::GUARD => 1000,
                        npc::FIGHTER => 500,
                        _ => 100,
                    };
                    b.cash += reward;
                    texts.push(FloatText { pos: player.pos + up * 2.4, text: format!("+${reward}"), color: Color::new(0.4, 0.9, 0.4, 1.0), t: 0.0, big: g >= npc::URNA });
                }
                !dead && time - t < 3.0
            });
        }

        // Steve: golpe/mineração (segura), usar item; pavios, fogo e flechas rodam sempre
        let others: Vec<(u64, Vec3)> = remotes.iter().map(|(id, r)| (*id, r.pos)).collect();
        if ch == 0 {
            let mouse_ok = grabbed && !just_grabbed && active;
            let inp = steve::Input {
                attack: tap_hit || (mouse_ok && is_mouse_button_pressed(MouseButton::Left)),
                mine: tap_hit || (active && btn(1)) || (mouse_ok && is_mouse_button_down(MouseButton::Left)),
                use_press: tap_place || (mouse_ok && is_mouse_button_pressed(MouseButton::Right)),
                use_hold: (active && btn(2)) || (mouse_ok && is_mouse_button_down(MouseButton::Right)),
            };
            steve.act(&world, &player, eye, fw, pick, &targets, &others, &inp, dt, &mut fx, &atlas.avg);
        }
        steve.tick(&world, &mut player, ch == 0, is_host, &targets, &others, dt, &mut fx);
        for v in steve.outbox.drain(..) {
            send(v, &mut loopback);
        }
        portals.update(&world, &player, eye, fw, grabbed && !just_grabbed && active, active, my_id, is_host, &mut villagers, &mut fx, dt);
        for v in portals.outbox.drain(..) {
            send(v, &mut loopback);
        }
        portals.sounds(&audio, eye, muted);

        prof.mark(prof::PLAYER);
        // ------------------------------------------------ Simulação (host) / interpolação (demais)
        let evento = relogio.evento();
        if evento && !was_event {
            banner = Some(("AS ELEICOES COMECARAM!!! URNAS ABERTAS!".into(), 6.0));
            play_at(&audio, &sfx.boom, eye, eye, 1.0, muted, false);
        }
        was_event = evento;
        for d in npcs.tick(dt, is_host) {
            match d.g {
                npc::VILLAGER => {
                    if let Some(v) = villagers.get_mut(d.i) {
                        v.pos = v.home;
                        v.vel = Vec3::ZERO;
                        v.airborne = false;
                        v.spin = 0.0;
                    }
                }
                npc::URNA => {
                    urna.pos.y += 30.0;
                    events.push(Ev::Banner("A URNA VOLTOU. 2o TURNO!".into()));
                }
                npc::EU => events.push(Ev::Banner("RECOMPILEI. VOLTEI.".into())),
                npc::VOADOR => events.push(Ev::Banner("O BOLSONARO VOADOR VOLTOU A VOAR, TALKEY?".into())),
                npc::GUARD => events.push(Ev::Banner("SINAPSE-9 REINICIOU. O LAB SEGUE LENDO.".into())),
                _ => {}
            }
        }
        let (mod_hurt, mod_shake) = mods.update(&world, dt, time, is_host, &mut npcs, player.pos, &others);
        if mod_hurt > 0.0 {
            steve.hurt(mod_hurt);
            if let Some(b) = bandido.as_mut().filter(|b| !b.driving) {
                b.hurt(mod_hurt * 3.0);
            }
            wolvie.damage(mod_hurt * 3.0, Vec3::ZERO);
            bomba.hurt_me(mod_hurt * 3.0);
        }
        fx.shake = fx.shake.max(mod_shake);
        for (c, p) in mods.sfx.drain(..) {
            play_at(&audio, &c, p, eye, 1.0, muted, in_club);
        }
        for v in mods.outbox.drain(..) {
            send(v, &mut loopback);
        }
        for v in uni.outbox.drain(..) {
            if online {
                send(v, &mut loopback);
            }
        }
        if urna.dead && npcs.urna().alive() && !is_host {
            urna.pos.y += 30.0;
        }
        urna.dead = !npcs.urna().alive();
        if is_host {
            actors::update_villagers(&mut villagers, &world, dt, time, |i| !npcs.alive(npc::VILLAGER, i));
            actors::update_fighters(&mut fighters, &world, dt, time, &mut events);
            gumbas.think(&world, dt, &npcs);
            mario.think(&world, dt, time, &mut fighters, &gumbas.targets(&npcs), &mut npcs, &mut events);
            bomba.think(&world, dt, time, &mut fighters, &mut npcs, &mut events);
            if (evento || ai_rage > 0.0) && !urna.charging {
                urna.timer = urna.timer.min(0.35);
            }
            let mut targets: Vec<Vec3> = vec![player.pos];
            targets.extend(remotes.values().map(|r| r.pos));
            let shot_target = {
                let fs = &fighters;
                let vs = &villagers;
                let rival = kaiju.lure(urna.root, &npcs);
                urna.update(&world, dt, || {
                    let roll = gen_range(0.0, 1.0);
                    if let Some(k) = rival.filter(|_| roll < 0.5) {
                        return k;
                    }
                    let active: Vec<Vec3> = fs.iter().filter(|f| f.active()).map(|f| f.pos + vec3(0.0, 0.9, 0.0)).collect();
                    if roll < 0.35 && !active.is_empty() {
                        active[gen_range(0, active.len())]
                    } else if roll < 0.6 {
                        let c = arena_center() + vec3(gen_range(-40.0, 40.0), 0.0, gen_range(-40.0, 40.0));
                        vec3(c.x, G as f32, c.z)
                    } else if roll < 0.8 {
                        shield_center() + vec3(gen_range(-6.0, 6.0), gen_range(0.0, 4.0), gen_range(-8.0, 8.0))
                    } else if roll < 0.85 && time > 30.0 {
                        targets[gen_range(0, targets.len())]
                    } else {
                        let w: Vec<Vec3> = vs.iter().filter(|v| v.kind == VKind::Wanderer).map(|v| v.pos).collect();
                        if w.is_empty() { arena_center() } else { w[gen_range(0, w.len())] }
                    }
                })
            };
            if let Some(target) = shot_target.filter(|_| !laws.paz) {
                let plan = urna::plan(&world, urna.eye(), target, if evento { 0.5 } else { 0.15 });
                send(mp::shot(&plan), &mut loopback);
            }
            if let Some(plan) = voador.think(&world, dt, time, &targets, &fighters, &villagers, &npcs, urna.pos, &mut events) {
                let mut v = mp::shot(&plan);
                v["by"] = json!(voador::BY);
                send(v, &mut loopback);
            }
            for plan in kaiju.think(&world, dt, time, &urna, &npcs, &targets, &fighters, &villagers, &mut events) {
                let mut v = mp::shot(&plan);
                v["by"] = json!(kaiju::BY);
                send(v, &mut loopback);
            }
            for plan in zeppelin.think(&world, dt, time, &npcs, &targets, &mut events) {
                let mut v = mp::shot(&plan);
                v["by"] = json!(zeppelin::BY);
                send(v, &mut loopback);
            }
        } else {
            let k = (dt * 12.0).min(1.0);
            for (f, &p) in fighters.iter_mut().zip(&fpos) {
                f.pos = if f.pos.distance(p) > 6.0 { p } else { f.pos.lerp(p, k) };
            }
            for (v, &p) in villagers.iter_mut().zip(&vpos) {
                v.pos = if v.pos.distance(p) > 6.0 { p } else { v.pos.lerp(p, k) };
            }
        }
        urna.animate(&world, dt, time);
        if urna.stomp > 0.0 {
            let d = urna.root.distance(eye);
            if !in_club {
                fx.shake = fx.shake.max((1.0 - d / 45.0).max(0.0) * 0.5);
            }
            play_at(&audio, &sfx.boom, urna.root, eye, 0.35, muted, in_club);
        }
        kaiju.animate(&world, dt, time, npcs.kaiju(), eye, in_club, &mut fx);
        for (c, p, v) in kaiju.sfx.drain(..) {
            play_at(&audio, &c, p, eye, v, muted, in_club);
        }
        zeppelin.tick(&world, &audio, dt, time, npcs.zeppelin(), eye, muted, in_club, &mut fx);
        mario.animate(&world, dt, is_host, &mut fx);
        gumbas.animate(dt, is_host);
        let mut bodies: Vec<(u64, Vec3)> = remotes.iter().map(|(id, r)| (*id, r.pos)).collect();
        bodies.push((my_id, player.pos));
        for v in bomba.update(&world, dt, is_host, &fighters, &bodies, &mut fx) {
            send(v, &mut loopback);
        }
        if is_host {
            for (g, i, dmg, dir, at) in bomba.burn(&targets, &mut fighters, time, &mut events) {
                if npc_hit(&mut npcs, &mut villagers, g, i, dmg, dir, at, &mut events) {
                    let mut v = mp::shot(&urna::Plan { o: urna.root + up * 1.5, hit: urna.root + up * 1.5, r: 6.0, deflect: false });
                    v["by"] = json!(2);
                    send(v, &mut loopback);
                }
            }
        }
        if let Some(dir) = bomba.burn_me(player.pos) {
            player.knock += (dir * 5.0 + up * 6.0) * wolvie.damage(40.0, dir);
            steve.hurt(10.0);
            if let Some(b) = bandido.as_mut().filter(|b| !b.driving) {
                b.hurt(60.0);
            }
        }
        for p in bomba.sounds.drain(..) {
            play_at(&audio, &sfx.boom, p, eye, 1.1, muted, in_club);
            if !in_club {
                fx.shake = fx.shake.max((1.0 - p.distance(eye) / 40.0).max(0.0) * 0.6);
            }
        }
        for (c, p, v) in mario.sfx.drain(..) {
            play_at(&audio, &c, p, eye, v, muted, in_club);
        }
        for (c, p, v) in wolvie.sfx.drain(..) {
            play_at(&audio, &c, p, eye, v, muted, in_club);
        }
        for r in remotes.values_mut() {
            let moved = r.pos.distance(r.target);
            r.walk += moved * 3.0;
            r.pos = if moved > 8.0 || portals.jumped(r.pos, r.target) { r.target } else { r.pos.lerp(r.target, (dt * 12.0).min(1.0)) };
        }

        prof.mark(prof::NPC);
        // Fogos na abertura das urnas (ou quando pedem pra IA)
        ai_fireworks -= dt;
        ai_rage -= dt;
        if let Some((_, t)) = ai_say.as_mut() {
            *t -= dt;
        }
        ai_say = ai_say.filter(|s| s.1 > 0.0);
        if let Some((_, t)) = ai_sky.as_mut() {
            *t -= dt;
        }
        ai_sky = ai_sky.filter(|s| s.1 > 0.0);
        if evento || ai_fireworks > 0.0 {
            fireworks_t -= dt;
            if fireworks_t <= 0.0 {
                fireworks_t = gen_range(0.15, 0.4);
                let c = layout::plaza_center() + vec3(gen_range(-50.0, 50.0), gen_range(22.0, 38.0), gen_range(-50.0, 30.0));
                let hue = gen_range(0.0, 1.0);
                for _ in 0..40 {
                    let d = vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)).normalize_or_zero();
                    fx.particles.push(urna::Particle { pos: c, vel: d * gen_range(8.0, 14.0), col: club::hsv(hue + gen_range(-0.05, 0.05), 0.8, 1.0), life: gen_range(0.8, 1.5), size: 0.3, gravity: false });
                }
                play_at(&audio, &sfx.punch, c, eye, 0.8, muted, in_club);
            }
        }

        fx.update(&world, dt);
        let cap = quality::pick([300, 800, 4000]);
        if fx.particles.len() > cap {
            fx.particles.drain(..fx.particles.len() - cap);
        }
        if is_host && online {
            ev_out.extend(events.iter().cloned());
        }
        for e in events.drain(..) {
            match e {
                Ev::Hit { pos, claws } => {
                    play_at(&audio, if claws { &sfx.slash } else { &sfx.punch }, pos, eye, 0.9, muted, in_club);
                    for _ in 0..6 {
                        fx.particles.push(urna::Particle {
                            pos,
                            vel: vec3(gen_range(-3.0, 3.0), gen_range(0.0, 3.0), gen_range(-3.0, 3.0)),
                            col: if claws { Color::new(0.9, 0.95, 1.0, 1.0) } else { Color::new(1.0, 0.9, 0.5, 1.0) },
                            life: 0.3,
                            size: 0.1,
                            gravity: false,
                        });
                    }
                }
                Ev::Snikt(p) => play_at(&audio, &sfx.snikt, p, eye, 1.2, muted, in_club),
                Ev::Text { pos, text, color, big } => texts.push(FloatText { pos, text, color, t: 0.0, big }),
                Ev::Shake(a) => {
                    if !in_club {
                        fx.shake = fx.shake.max(a)
                    }
                }
                Ev::Banner(s) => banner = Some((s, 3.0)),
            }
        }

        // Rede: posição e snapshot do host ~7x/s (cota grátis do Cloudflare conta mensagens recebidas)
        net_t -= dt;
        if online && net_t <= 0.0 {
            net_t = 0.15;
            let yaw = match (&skater, &bandido) {
                (Some(s), _) => s.heading,
                (_, Some(b)) if b.driving => car.yaw,
                _ if mario.me.is_some() => mario.me.as_ref().map_or(0.0, |b| b.yaw),
                _ if wolvie.me.is_some() => wolvie.yaw(),
                _ => fw.x.atan2(fw.z),
            };
            let mut pm = json!({"t": "p", "p": mp::v3(player.pos), "y": yaw, "c": ch});
            steve.fill_p(&mut pm, ch, player.sel);
            portals.fill_p(&mut pm);
            wolvie.fill_p(&mut pm);
            net.send(pm.to_string());
            if is_host {
                let mut s = mp::snapshot(time, &urna, &fighters, &villagers, &ev_out);
                s["n"] = npcs.snapshot();
                s["md"] = mods.snapshot();
                s["kj"] = kaiju.snapshot();
                s["mr"] = mario.snapshot();
                s["gb"] = gumbas.snapshot();
                s["bb"] = bomba.snapshot();
                net.send(s.to_string());
                ev_out.clear();
            }
        }

        for t in texts.iter_mut() {
            t.t += dt;
            t.pos.y += dt * 0.8;
        }
        texts.retain(|t| t.t < 1.6);

        chunks.update(&mut world, 8);

        // Volume da música pela distância do clube
        telao.update();
        vibe_t -= dt;
        if vibe_t <= 0.0 {
            vibe_t = quality::pick([0.25, 0.1, 0.1]);
            if let Some(px) = telao.sample() {
                vibe.sample(&px);
            }
        }
        vibe.update(telao.live, dt);
        let target_vol = if muted { 0.0 } else { 0.25 + 0.75 * (1.0 - (eye.distance(shield_center()) - 10.0) / 90.0).clamp(0.0, 1.0) };
        audio.set_volume(house, if telao.live { 0.0 } else { target_vol * 0.6 });
        telao.set_volume(target_vol);

        // ------------------------------------------------ Render 3D
        club_k += ((in_club as i32 as f32) - club_k) * (dt * 2.0).min(1.0);
        let day = 1.0 - club_k * 0.9;
        let (va, vk) = (vibe.pal[3], vibe.k);
        let night = vec3(0.06 + (va.r * 0.2 - 0.06) * vk, va.g * 0.2 * vk, 0.1 + (va.b * 0.2 - 0.1) * vk) * club_k;
        let sky = Color::new(0.53 * day + night.x, 0.75 * day + night.y, 1.0 * day + night.z, 1.0);
        let sky = match ai_sky {
            Some((c, t)) => {
                let k = t.min(1.0) * 0.8;
                Color::new(sky.r + (c.r * day - sky.r) * k, sky.g + (c.g * day - sky.g) * k, sky.b + (c.b * day - sky.b) * k, 1.0)
            }
            None => sky,
        };
        prof.mark(prof::MISC);
        lab_info.render(time, eye, npcs.guard());
        let in_ring = remotes.values().map(|r| (r.name.as_str(), r.pos)).chain([(my_name.as_str(), player.pos)]);
        arena_board.render(time, eye, &fighters, &npcs, mario.visible().then_some(mario.body.pos), bomba.visible().then_some(bomba.npc.pos), urna.pos, in_ring);
        places.render(time, eye);
        prof.mark(prof::LAB_RT);
        clear_background(sky);
        let shake = vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)) * fx.shake * 0.35;
        let (cam_eye, cam_fw, cam_up) = player.bob_view(eye + shake, fw);
        let cam = Camera3D {
            position: cam_eye,
            target: cam_eye + cam_fw,
            up: cam_up,
            fovy: match bandido.as_ref().filter(|b| b.aiming) {
                Some(b) if b.weapon == 7 => 18f32,
                Some(b) if b.weapon == 8 => 45f32,
                Some(_) => 58f32,
                None => skater.as_ref().map_or(70f32 * player.fov_mul(), |s| s.fov()),
            }
            .to_radians(),
            ..Default::default()
        };
        set_camera(&cam);
        let vp = cam.matrix();
        // Vista dos portais redesenha o batch de outro ângulo: sem culling enquanto houver portal visível
        let low = quality::tier() == quality::LOW;
        let view_dist = quality::pick([72.0, 128.0, f32::INFINITY]);
        let cull = (portals.list.is_empty() || low).then(|| batch::Cull { planes: chunks::frustum(&vp), eye: cam.position, min_ratio: quality::pick([1.5, 1.0, 0.6]) * 2.0 * (cam.fovy * 0.5).tan() / sh, max_dist: view_dist + 8.0 });
        opaque.cull = cull.map(|c| batch::Cull { max_dist: f32::INFINITY, ..c });
        trans.cull = cull;

        // Céu: sol e nuvens
        let id = Mat4::IDENTITY;
        let sun_dir = vec3(0.4, 0.75, 0.3).normalize();
        opaque.glow(&id, eye + sun_dir * 160.0, Vec3::splat(16.0), Color::new(day, 0.95 * day, 0.6 * day, 1.0));
        let span = world::WX as f32 + 132.0;
        for k in 0..60 {
            let kx = atlas::hash2(k, 1, 900) * span - 66.0;
            let kz = atlas::hash2(k, 2, 900) * span - 66.0;
            let x = (kx + time * 1.5).rem_euclid(span) - 66.0;
            let s = vec3(8.0 + atlas::hash2(k, 3, 900) * 12.0, 1.5, 6.0 + atlas::hash2(k, 4, 900) * 10.0);
            opaque.glow(&id, vec3(x, 62.0, kz), s, Color::new(day, day, day, 1.0));
        }
        opaque.cull = cull;

        chunks.draw(&vp, None, eye, view_dist, sky);
        prof.mark(prof::WORLD);

        labels.clear();
        club::draw(&mut opaque, &mut trans, time, beat, &vibe);
        relogio.draw(&mut opaque, time);
        let bf = beat.fract();
        for (i, v) in villagers.iter().enumerate() {
            if let Some(d) = npcs.get(npc::VILLAGER, i).filter(|d| !d.alive()) {
                let m = root(v.pos, v.yaw, d.lean(), 0.0) * Mat4::from_rotation_x(v.spin);
                draw_villager(&mut opaque, &v.look, &m, 0.0, 0.0, 0.0, 0.0, 0.0);
                continue;
            }
            let m = root(v.pos, v.yaw, 0.0, 0.0) * Mat4::from_rotation_x(v.spin);
            let (bounce, nod, arms, phase, walk_amt) = match v.kind {
                VKind::Dancer => ((beat * std::f32::consts::PI).sin().abs() * 0.18, (beat * std::f32::consts::TAU).sin() * 0.25, if v.arms_up { 1.0 } else { 0.0 }, beat * std::f32::consts::PI + v.phase, 0.25),
                VKind::Dj => (0.0, (beat * std::f32::consts::TAU).sin() * 0.4, if (beat as i32 / 8) % 2 == 0 { 1.0 } else { 0.0 }, beat * 2.0, 0.0),
                VKind::Wanderer => (0.0, 0.0, 0.0, 0.0, 1.0),
            };
            let m = if bounce > 0.0 { Mat4::from_translation(vec3(0.0, bounce, 0.0)) * m } else { m };
            draw_villager(&mut opaque, &v.look, &m, arms, phase, nod, if v.kind == VKind::Dancer { beat * 3.14 } else { v.walk }, walk_amt);
            let sway = (time * 2.0 + v.phase).sin() * 0.15 + if v.kind == VKind::Dancer { (1.0 - bf) * 0.1 } else { 0.0 };
            let top = draw_flag(&mut opaque, &m, vec3(-0.45, 0.9, -0.1), 2.0, v.flag_col, None, time + i as f32, sway, -1.0);
            if top.distance(eye) < 38.0 {
                labels.push(Label { pos: top, text: actors::FLAG_TEXTS[v.flag].to_string(), size: 18.0, color: WHITE });
            }
        }
        for f in &fighters {
            if !f.spawned {
                continue;
            }
            let pose = actors::fighter_pose(f, time);
            let m = root(f.pos, f.yaw, pose.lean, pose.bounce);
            draw_humanoid(&mut opaque, &f.look, &pose, &m);
            let top = draw_flag(&mut opaque, &m, vec3(0.0, 0.75, -0.2), 2.3, f.flag.0, f.flag.1, time, (time * 3.0).sin() * 0.1, 1.0);
            labels.push(Label { pos: top, text: f.flag_text.to_string(), size: 22.0, color: f.flag.0 });
            labels.push(Label { pos: f.pos + up * 2.3, text: f.name.to_string(), size: 24.0, color: if f.berserk > 0.0 { RED } else { WHITE } });
        }
        for (i, g) in guests.iter().enumerate() {
            let life = npcs.get(npc::GUEST, i).copied();
            if let Some(d) = life.filter(|d| !d.alive()) {
                let m = root(g.pos, g.yaw, d.lean(), 0.0);
                draw_humanoid(&mut opaque, &g.look, &actors::dead_pose(), &m);
                if g.name == actors::VORCARO {
                    draw_banker_extras(&mut opaque, &g.look, &actors::dead_pose(), &m);
                }
                continue;
            }
            let mut pose = actors::guest_pose(g, beat);
            pose.flash = life.map(|d| d.flash).unwrap_or(0.0);
            let vip = g.name == actors::VORCARO;
            let yaw = if vip { g.yaw + (beat * std::f32::consts::FRAC_PI_4).sin() * 0.7 } else { g.yaw };
            let m = root(g.pos, yaw, pose.lean, pose.bounce);
            let m = if vip { m * Mat4::from_scale(Vec3::splat(1.15)) } else { m };
            draw_humanoid(&mut opaque, &g.look, &pose, &m);
            if vip {
                draw_banker_extras(&mut opaque, &g.look, &pose, &m);
                let v = vibe.light(Color::new(1.0, 0.9, 0.6, 1.0), 0);
                let mut spot = Color::new(0.5 + v.r * 0.5, 0.5 + v.g * 0.5, 0.5 + v.b * 0.5, 1.0);
                opaque.glow(&id, vec3(g.pos.x, G as f32 + 0.03, g.pos.z), vec3(2.2, 0.02, 2.2), spot);
                spot.a = 0.14 + 0.08 * (1.0 - bf).powi(2);
                trans.glow(&id, vec3(g.pos.x, G as f32 + 4.0, g.pos.z), vec3(1.6, 8.0, 1.6), spot);
            }
            if g.pos.distance(eye) < 45.0 {
                let (size, h) = if vip { (24.0, 2.5) } else { (18.0, 2.2) };
                labels.push(Label { pos: g.pos + up * h, text: g.name.to_string(), size, color: Color::new(1.0, 0.85, 0.3, 1.0) });
                if vip && time % 9.0 < 4.5 {
                    let fala = actors::VORCARO_FALAS[(time / 9.0) as usize % actors::VORCARO_FALAS.len()];
                    labels.push(Label { pos: g.pos + up * 3.1, text: format!("\"{fala}\""), size: 22.0, color: WHITE });
                }
            }
        }
        for (rid, r) in remotes.iter() {
            let moving = (r.pos - r.target).length() > 0.05;
            match r.ch {
                1 => skate::Skater::new(r.pos, r.yaw).draw(&mut opaque, r.look.shirt, time),
                3 => gta::draw_car(&mut opaque, r.pos, r.yaw, 0.0, r.walk, true),
                2 => gta::draw_remote(&mut opaque, r.pos, r.yaw, r.walk, moving),
                6 => mario::draw_remote(&mut opaque, &mut trans, r.pos, r.yaw, r.walk, moving, r.vy, time),
                7 => wolvie.draw_remote(&mut opaque, *rid, r.pos, r.yaw, r.walk, if moving { r.spd } else { 0.0 }, r.vy, time),
                8 => bomba::draw_remote(&mut opaque, &mut trans, r.pos, r.yaw, r.walk, moving, time),
                0 | 5 if r.av.as_deref().is_some_and(|a| uni.draw(&mut opaque, a, r.pos, r.yaw, if moving { r.spd } else { 0.0 }, r.vy.abs() > 1.5, time)) => {}
                _ => {
                    let pose = Pose { walk: r.walk, walk_amt: if moving { 1.0 } else { 0.0 }, arm_l: -0.2, arm_r: -0.2, ..Default::default() };
                    let look = if r.ch == 4 { &niko_look } else { &r.look };
                    draw_humanoid(&mut opaque, look, &pose, &steve.remote_root(*rid, r.pos, r.yaw));
                }
            }
            labels.push(Label { pos: r.pos + up * 2.2, text: r.name.clone(), size: 22.0, color: Color::new(0.5, 1.0, 0.6, 1.0) });
        }
        // Telão: moldura presa na parede oeste do clube, virado pro leste
        let (cdx, cdz) = (layout::CLUB_D.x as f32, layout::CLUB_D.y as f32);
        let (tx, tz0, tz1, ty0, ty1) = (9.21 + cdx, 56.5 + cdz, 72.5 + cdz, G as f32 + 4.5, G as f32 + 13.5);
        opaque.cube(&id, vec3(9.1 + cdx, (ty0 + ty1) * 0.5, (tz0 + tz1) * 0.5), vec3(0.2, ty1 - ty0 + 0.4, tz1 - tz0 + 0.4), Color::new(0.05, 0.05, 0.06, 1.0));
        let driving = bandido.as_ref().is_some_and(|b| b.driving);
        car.draw(&mut opaque, driving);
        if !driving && car.pos.distance(eye) < 30.0 {
            labels.push(Label { pos: car.pos + up * 2.4, text: if ch == 2 { "SEDA DA 1a MISSAO - F PRA ENTRAR".into() } else { "SEDA DA 1a MISSAO (VIRA BANDIDO NO C)".into() }, size: 18.0, color: Color::new(1.0, 0.85, 0.3, 1.0) });
        }
        skate::draw_park(&mut opaque, eye);
        if let Some(sk) = &skater {
            sk.draw(&mut opaque, rgb(0.45, 0.47, 0.5), time);
        }
        if let Some(n) = &niko {
            n.draw(&mut opaque);
        }
        if let Some(b) = bandido.as_ref().filter(|b| !b.driving && !(b.aiming && matches!(b.weapon, 7 | 8))) {
            b.draw(&mut opaque, &mut trans, player.pos, fw, gta_walk, moving, time);
        }
        urna.draw(&mut opaque, &mut trans, time);
        let eu_dead = (!npcs.eu().alive()).then(|| npcs.eu().t);
        extras::draw_me(&mut opaque, &mut trans, time, &mut labels, urna.pos, fx.shield_flash, ai_say.as_ref().map(|s| s.0.as_str()), eu_dead);
        voador.draw(&mut opaque, &mut trans, &world, time, dt, &mut labels, npcs.voador());
        kaiju.draw(&mut opaque, &mut trans, &world, &mut labels, time, npcs.kaiju());
        zeppelin.draw(&mut opaque, &mut trans, &world, &mut labels, time, eye, npcs.zeppelin());
        mario.draw(&mut opaque, &mut trans, &mut labels, time, npcs.get(npc::MARIO, 0));
        gumbas.draw(&mut opaque, &npcs, eye);
        bomba.draw(&mut opaque, &mut trans, &mut labels, time, npcs.get(npc::BOMBA, 0), (ch == 8).then(|| (player.pos, fw.x.atan2(fw.z), gta_walk, moving)));
        wolvie.draw(&mut opaque, &mut trans, time);
        let robots_dead: Vec<Option<f32>> = (0..extras::ROBOTS).map(|i| npcs.get(npc::ROBOT, i).filter(|d| !d.alive()).map(|d| d.t)).collect();
        lab.draw(&mut opaque, &mut trans, time, &mut labels, eye, &robots_dead);
        lab::draw(&mut opaque, &mut trans, &mut labels, time, eye, &lab_info, Some(npcs.guard()));
        arena_panel::draw_frame(&mut opaque, &mut trans, time, eye);
        mods.draw(&mut opaque, &mut trans, &mut labels, time, eye, &npcs);
        hub.draw(&mut opaque, &mut trans, &mut labels, time, eye);
        places.draw(&mut opaque, &mut trans, &mut labels, time, eye);
        eco.draw_world(&mut opaque, &mut labels, eye, time);
        steve.draw_world(&mut opaque, time, eye, fw, ch == 0, player.sel, remotes.iter().map(|(id, r)| (*id, r.pos, r.yaw, r.ch)), &atlas.avg);
        fx.draw_opaque(&mut opaque);
        let body = matches!(ch, 0 | 5).then(|| (player.pos, fw.x.atan2(fw.z), remote_look(my_id), gta_walk, moving));
        prof.mark(prof::ACTORS);
        portals.render(&mut opaque, &chunks, &atlas.tex, &cam, sky, body, mobile);
        prof.mark(prof::PORTALS);
        opaque.flush(&atlas.tex);
        draw_mesh(&Mesh {
            vertices: vec![
                Vertex::new(tx, ty1, tz1, 0.0, 0.0, WHITE),
                Vertex::new(tx, ty1, tz0, 1.0, 0.0, WHITE),
                Vertex::new(tx, ty0, tz0, 1.0, 1.0, WHITE),
                Vertex::new(tx, ty0, tz1, 0.0, 1.0, WHITE),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(telao.tex.clone()),
        });
        let corners = [vec3(tx, ty1, tz1), vec3(tx, ty1, tz0), vec3(tx, ty0, tz0), vec3(tx, ty0, tz1)];
        let on_screen = (in_club && eye.x > tx).then(|| corners.map(|c| project_any(&vp, c))).and_then(|c| Some([c[0]?, c[1]?, c[2]?, c[3]?]));
        telao.place(on_screen);
        let screen_mid = vec3(tx + 0.3, (ty0 + ty1) * 0.5, (tz0 + tz1) * 0.5);
        if !telao.status.is_empty() {
            labels.push(Label { pos: screen_mid, text: telao.status.clone(), size: 30.0, color: WHITE });
        }
        if !telao.title.is_empty() {
            labels.push(Label { pos: vec3(tx + 0.3, ty1 + 0.8, screen_mid.z), text: telao.title.clone(), size: 22.0, color: Color::new(1.0, 0.4, 0.9, 1.0) });
        }

        if let Some((p, _, _)) = pick {
            draw_cube_wires(p.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.004), Color::new(0.0, 0.0, 0.0, 0.8));
        }
        steve.draw_crack();

        fx.draw_transparent(&mut trans, time);
        trans.flush(&atlas.tex);
        fx.draw_flashes();
        let sc = shield_center();
        let sf = fx.shield_flash;
        let shimmer = 0.03 * (time * 2.0).sin();
        if chunks::sphere_visible(&chunks::frustum(&vp), sc, SHIELD_R) {
            let rings = quality::pick([8, 16, 16]);
            draw_sphere_ex(sc, SHIELD_R, None, Color::new(0.45, 0.75 + 0.2 * sf, 1.0, 0.09 + shimmer + 0.25 * sf), DrawSphereParams { rings, slices: rings + 4 * (rings < 16) as usize, ..Default::default() });
            if !low {
                draw_sphere_wires(sc, SHIELD_R + 0.05, None, Color::new(0.6, 0.9, 1.0, 0.12 + 0.4 * sf));
            }
        }
        lab_info.draw_dome(time, npcs.guard().flash.max(fx.dome_flash[0]), eye);
        arena_board.draw_screen(time, eye);
        places.draw_screens(time, eye);
        shield::draw_hub(time, fx.dome_flash[1]);

        prof.mark(prof::FLUSH);
        // ------------------------------------------------ Render 2D
        set_default_camera();
        if club_k > 0.01 {
            let (w, h) = (screen_width(), screen_height());
            let pulse = (1.0 - beat.fract()).powi(3);
            let c = vibe.light(club::hsv(beat * 0.125, 0.85, 1.0), beat.floor().max(0.0) as usize);
            draw_rectangle(0.0, 0.0, w, h, Color::new(night.x, night.y, night.z + 0.02 * club_k, 0.5 * club_k));
            draw_rectangle(0.0, 0.0, w, h, Color::new(c.r, c.g, c.b, (0.12 + 0.06 * vibe.k) * pulse * club_k));
        }
        labels.push(Label { pos: layout::club(vec3(36.5, G as f32 + 8.2, 64.5)), text: "CLUB DO HOUSE - SO CURTINDO".into(), size: 30.0, color: Color::new(1.0, 0.4, 0.9, 1.0) });
        layout::labels(&mut labels, eye);
        labels.push(Label { pos: urna.matrix().transform_point3(vec3(0.0, 6.0, 0.0)), text: "URNA ELETRONICA".into(), size: 32.0, color: Color::new(1.0, 0.85, 0.3, 1.0) });
        labels.push(Label { pos: urna.matrix().transform_point3(vec3(5.2, -3.6, 1.8)), text: "CONFIRMA".into(), size: 20.0, color: GREEN });
        let (label_max, label_far) = quality::pick([(12, 45.0), (24, 80.0), (usize::MAX, f32::MAX)]);
        labels.retain(|l| l.pos.distance(eye) < label_far);
        if labels.len() > label_max {
            labels.sort_by(|a, b| a.pos.distance_squared(eye).total_cmp(&b.pos.distance_squared(eye)));
            labels.truncate(label_max);
        }
        for l in &labels {
            if let Some(s) = project(&vp, l.pos) {
                let d = l.pos.distance(eye);
                let size = (l.size * (14.0 / d.max(6.0)).clamp(0.55, 1.6)).max(13.0);
                text_centered(&l.text, s.x, s.y, size, l.color, true);
            }
        }
        // Barrinhas de vida só sobre a cabeça, por alguns segundos depois de apanhar
        fighter_bars.resize(fighters.len(), (0.0, 0.0));
        for (f, (last, left)) in fighters.iter().zip(fighter_bars.iter_mut()) {
            *left = if f.hp < *last { npc::BAR_SECS } else { (*left - dt).max(0.0) };
            *last = f.hp;
            if f.spawned && f.hp < f.max_hp && *left > 0.0 {
                overhead_bar(&vp, f.pos + up * 1.55, 0.8, f.hp / f.max_hp);
            }
        }
        let mut tops: Vec<(u8, usize, Vec3, f32)> = Vec::new();
        for &(c, r, i, g) in targets.iter().filter(|t| t.3 != npc::FIGHTER) {
            let top = c + up * if g == npc::KAIJU { 8.0 } else { r };
            match tops.iter_mut().find(|t| t.0 == g && t.1 == i) {
                Some(t) if top.y > t.2.y => (t.2, t.3) = (top, t.3.max(r)),
                Some(t) => t.3 = t.3.max(r),
                None => tops.push((g, i, top, r)),
            }
        }
        for (g, i, top, r) in tops {
            let Some(d) = npcs.get(g, i).filter(|d| d.hp < d.max && d.bar > 0.0) else { continue };
            overhead_bar(&vp, top, r, d.hp / d.max);
        }
        for t in &texts {
            if let Some(s) = project(&vp, t.pos) {
                let a = (1.0 - t.t / 1.6).clamp(0.0, 1.0);
                let mut c = t.color;
                c.a = a;
                text_centered(&t.text, s.x, s.y, if t.big { 46.0 } else { 24.0 }, c, false);
            }
        }

        prof.mark(prof::LABELS);
        let sw = screen_width();
        let sh = screen_height();

        // Status da rede + jogadores online
        let status = match (url.is_some(), online, net.state()) {
            (false, _, _) => "OFFLINE (sem servidor configurado)".to_string(),
            (_, true, _) => format!("ONLINE {} jogador(es){}", remotes.len() + 1, if is_host { " | voce e o host" } else { "" }),
            (_, false, net::CONNECTING) => "CONECTANDO...".to_string(),
            _ => "OFFLINE (servidor fora do ar)".to_string(),
        };
        let hud_y = if let Some(b) = &bandido {
            draw_gta_hud(b, sw, time);
            135.0
        } else {
            0.0
        };
        let dim = measure_text(&status, None, 20, 1.0);
        draw_text(&status, sw - dim.width - 12.0, 84.0 + hud_y, 20.0, if online { Color::new(0.5, 1.0, 0.6, 1.0) } else { GRAY });
        if online {
            let mut names: Vec<&str> = remotes.values().map(|r| r.name.as_str()).collect();
            names.insert(0, my_name.as_str());
            for (i, nm) in names.iter().take(12).enumerate() {
                let d = measure_text(nm, None, 18, 1.0);
                draw_text(nm, sw - d.width - 12.0, 106.0 + hud_y + i as f32 * 20.0, 18.0, WHITE);
            }
        }
        eco.hud(dt, sw, sh, typing.is_none() && !chars_open);

        // Chat
        let now = get_time();
        let shown: Vec<&(String, f64)> = chat.iter().rev().filter(|c| typing.is_some() || now - c.1 < 12.0).take(8).collect();
        for (i, (line, _)) in shown.iter().enumerate() {
            let y = sh - 130.0 - i as f32 * 22.0;
            let d = measure_text(line, None, 20, 1.0);
            draw_rectangle(8.0, y - 17.0, d.width + 10.0, 22.0, Color::new(0.0, 0.0, 0.0, 0.45));
            draw_text(line, 13.0, y, 20.0, WHITE);
        }
        if let Some(msg) = &typing {
            draw_rectangle(8.0, sh - 118.0, sw * 0.5, 26.0, Color::new(0.0, 0.0, 0.0, 0.7));
            draw_text(&format!("> {msg}_"), 13.0, sh - 99.0, 22.0, YELLOW);
        }
        if chat.len() > 100 {
            chat.drain(..chat.len() - 100);
        }

        // Mira (bandido: alvo travado com marcador na cor da vida dele, luneta na sniper/bazuca)
        match bandido.as_ref() {
            Some(b) if b.driving => {}
            Some(b) => {
                let lock = b.lock.and_then(|(g, i)| {
                    let k = if g == npc::FIGHTER { fighters.get(i).map(|f| f.hp / f.max_hp) } else { npcs.get(g, i).map(|d| d.hp / d.max) };
                    Some((b.locked_pos(&targets)?, k.unwrap_or(1.0)))
                });
                if let Some(s) = lock.and_then(|(p, k)| project(&vp, p).map(|s| (s, k))) {
                    let (s, k) = s;
                    let col = Color::new((2.0 - 2.0 * k).min(1.0), (2.0 * k).min(1.0), 0.1, 0.95);
                    let rot = time * 3.0;
                    for q in 0..4 {
                        let a = rot + q as f32 * std::f32::consts::FRAC_PI_2;
                        let (dx, dy) = (a.cos(), a.sin());
                        let tip = vec2(s.x + dx * 14.0, s.y + dy * 14.0);
                        let base = vec2(s.x + dx * 26.0, s.y + dy * 26.0);
                        let perp = vec2(-dy, dx) * 6.0;
                        draw_triangle(tip, base + perp, base - perp, col);
                    }
                } else if b.aiming && matches!(b.weapon, 7 | 8) {
                    let r = sh * 0.42;
                    let (cx, cy) = (sw * 0.5, sh * 0.5);
                    let ink = Color::new(0.0, 0.0, 0.0, 1.0);
                    draw_rectangle(0.0, 0.0, cx - r, sh, ink);
                    draw_rectangle(cx + r, 0.0, sw - cx - r, sh, ink);
                    draw_circle_lines(cx, cy, r + sh * 0.3, sh * 0.6, ink);
                    draw_line(cx - r, cy, cx + r, cy, 1.5, ink);
                    draw_line(cx, cy - r, cx, cy + r, 1.5, ink);
                    draw_circle(cx, cy, 2.5, RED);
                } else {
                    let gap = if b.aiming { 4.0 } else { 7.0 };
                    let col = if b.aiming { Color::new(1.0, 0.3, 0.3, 0.95) } else { Color::new(1.0, 1.0, 1.0, 0.8) };
                    for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        draw_line(sw * 0.5 + dx * gap, sh * 0.5 + dy * gap, sw * 0.5 + dx * (gap + 8.0), sh * 0.5 + dy * (gap + 8.0), 2.0, col);
                    }
                }
            }
            None => {
                draw_line(sw * 0.5 - 9.0, sh * 0.5, sw * 0.5 + 9.0, sh * 0.5, 2.0, WHITE);
                draw_line(sw * 0.5, sh * 0.5 - 9.0, sw * 0.5, sh * 0.5 + 9.0, 2.0, WHITE);
            }
        }

        // Skate: combo e pontos / Bandido: arma atual
        if let Some(sk) = &skater {
            let combo = sk.combo_text();
            if !combo.is_empty() {
                text_centered(&combo, sw * 0.5, sh * 0.2, 26.0, Color::new(0.6, 1.0, 0.7, 1.0), true);
            }
            if let Some((p, t)) = &sk.popup {
                text_centered(p, sw * 0.5, sh * 0.2 + 34.0, 30.0, Color::new(1.0, 0.85, 0.3, t.min(1.0)), true);
            }
            text_centered(&format!("PONTOS SKATE: {}", sk.score), sw * 0.5, sh - 24.0, 24.0, WHITE, true);
        }
        if let Some(b) = &bandido {
            let txt = if b.driving { "DIRIGINDO - F SAI | ESPACO FREIO DE MAO".to_string() } else { format!("< {}/{}  {} >", b.weapon + 1, gta::ARSENAL.len(), gta::ARSENAL[b.weapon].name) };
            text_centered(&txt, sw * 0.5, sh - 24.0, 26.0, Color::new(1.0, 0.85, 0.3, 1.0), true);
            if b.wasted > 0.0 {
                let a = ((4.0 - b.wasted) * 1.5).min(1.0);
                draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.35, 0.0, 0.0, 0.45 * a));
                text_centered("WASTED", sw * 0.5, sh * 0.5, (sh * 0.14).max(48.0), Color::new(0.85, 0.1, 0.1, a), false);
            }
        }

        // Hotbar, vida e carga do arco do Steve
        if ch == 0 {
            steve.draw_hud(&atlas, player.sel, sw, sh, slot, mobile, &player.bob_matrix());
        }
        portals.hud(sw, sh);
        wolvie.hud(dt, sw, sh, &targets, |p| project(&vp, p), |s, x, y, z, c| text_centered(s, x, y, z, c, true), mobile);
        bomba.hud(sw, sh, my_id, |s, x, y, z, c| text_centered(s, x, y, z, c, true));

        draw_text(&format!("URNA-MINE-VERINE  |  {} FPS  |  disparos da urna: {}", get_fps(), urna.shots), 12.0, sh - 80.0, 20.0, WHITE);
        let now_playing = if telao.live { format!("TELAO: {}", telao.title) } else { "house sintetizado 124 BPM (telao carregando...)".to_string() };
        draw_text(&format!("TOCANDO: {}{}", now_playing, if muted { " [MUDO]" } else { "" }), 12.0, sh - 58.0, 20.0, Color::new(1.0, 0.5, 0.9, 1.0));
        if show_help {
            let lines = match ch {
                1 => [
                    "SKATE: W rema (toca no ritmo) / bombeia na rampa | S freia, POWERSLIDE, logo apos pousar = REVERT | A/D carve, no ar gira",
                    "MOUSE flick (rapido = pop alto): baixo>cima OLLIE | baixo>cima-esq/dir KICK/HEEL | baixo>lado SHOVE | no ar: flick = LATE FLIP",
                    "baixo>lado>cima 360 SHOVE | baixo>lado>diag VARIAL | esq>baixo>cima-dir 360 FLIP | inverta = NOLLIE | sobe o QP = VERT AIR",
                    "Q/BOTAO ESQ grab mao da frente, E/BOTAO DIR mao de tras (+mouse escolhe) | SHIFT MANUAL / STALL na coping | quina = GRIND",
                ],
                5 => [
                    "ARMA DE PORTAL: WASD anda | ESPACO pula | ESQ portal AZUL | DIR portal LARANJA",
                    "portal so em face plana 1x2 (parede, chao, teto) | velocidade entra = velocidade sai",
                    "Q cria cubo companheiro | E pega/solta | todo portal de todo jogador funciona pra todos",
                    "C troca personagem | T chat | M muta | H ajuda",
                ],
                4 => [
                    "NIKO: WASD anda | MOUSE olha | ESPACO pula | SHIFT corre",
                    "G = SE JOGA (tropeca/cai) | explosao derruba | queda alta derruba",
                    "Ragdoll ativo: cambaleia dando passos, cai protegendo com as maos e levanta sozinho",
                    "C troca personagem | T chat | /comando fala com a IA | H ajuda",
                ],
                2 | 3 => [
                    "BANDIDO: WASD anda | ESQ atira | DIR segura = mira no ombro + TRAVA no alvo (sniper/bazuca: luneta)",
                    "RODA / Q / E / 1-9 troca arma | F carro | todo NPC morre (ate a urna) e o procurado sobe",
                    "CARRO: W acelera | S re/freio | A/D vira | ESPACO freio de mao (drift)",
                    "C troca personagem | T chat | H ajuda",
                ],
                7 => [
                    "WOLVERINE: WASD | SHIFT corre | ESPACO pula / segura na parede = escala | CTRL ou V rola | X garras",
                    "ESQ combo (direcao varia o golpe) | segura ESQ ou F pesado | S+F gancho (lanca) | F no ar mergulho | SHIFT+F tornado",
                    "E BOTE (trava: TAB/meio) | montado: ESQ no tempo do anel, F estocada, ESPACO solta | DIR defende (na hora reflete laser)",
                    "R FURIA com a barra cheia | fator de cura sozinho | C troca personagem | T chat | H ajuda",
                ],
                8 => [
                    "BOMBADINHO: WASD anda | ESPACO pula | F ou ESQ solta bomba (gruda no grid, pavio 3s, explode em cruz)",
                    "Q/E ou RODA troca: NORMAL, FOGO (+alcance), PERFURANTE (fura bloco), REMOTA, LINHA (fileira), GOSMA (gruda)",
                    "G ou DIR detona as REMOTAS | anda contra a bomba = CHUTA | chama na bomba = reacao em cadeia",
                    "chama para no bloco duro e quebra so o 1o bloco fraco | C troca personagem | T chat | H ajuda",
                ],
                _ if steve.creative => [
                    "CRIATIVO: WASD andar | ESPACO pular | 2x W ou CTRL corre | SHIFT agacha | 2x ESPACO voa (SHIFT desce)",
                    "ESQ quebra na hora/soca | DIR poe | 1-9/RODA hotbar | E ou I inventario (todos os blocos)",
                    "K chama Wolverine | R reseta mundo | M muta | TAB solta mouse | H ajuda",
                    "Y troca o video do telao | T ou ENTER chat | C PERSONAGENS",
                ],
                _ => [
                    "SURVIVAL: WASD andar | ESPACO pular | 2x W ou CTRL corre | SHIFT agacha | queda machuca",
                    "ESQ segura = minera (ferramenta certa e mais rapida) / bate | DIR poe bloco",
                    "ARCO: segura DIR e solta | ISQUEIRO: DIR na TNT (4s) ou poe fogo | 1-9/RODA | E/I inventario",
                    "Y telao | T chat | C PERSONAGENS | K Wolverine | R reseta | H ajuda",
                ],
            };
            for (i, l) in lines.iter().chain(["B = CLIPE: salva os ultimos 10s em video (download local)"].iter()).enumerate() {
                draw_text(l, 12.0, 90.0 + i as f32 * 22.0, 20.0, Color::new(1.0, 1.0, 1.0, 0.85));
            }
        }
        if let Some((s, t)) = banner.as_mut() {
            *t -= dt;
            let a = t.clamp(0.0, 1.0);
            text_centered(s, sw * 0.5, sh * 0.28, 44.0, Color::new(1.0, 0.95, 0.4, a), true);
            if *t <= 0.0 {
                banner = None;
            }
        }
        if mobile {
            if let Some((_, o)) = stick {
                draw_circle(o.x, o.y, stick_r, Color::new(1.0, 1.0, 1.0, 0.12));
                draw_circle_lines(o.x, o.y, stick_r, 2.0, Color::new(1.0, 1.0, 1.0, 0.4));
                let k = o + player.stick * vec2(1.0, -1.0) * stick_r;
                draw_circle(k.x, k.y, stick_r * 0.42, Color::new(1.0, 1.0, 1.0, 0.45));
            } else {
                text_centered("ARRASTA AQUI PRA ANDAR", sw * 0.22, sh * 0.62, 16.0, Color::new(1.0, 1.0, 1.0, 0.5), false);
            }
            for (i, (c, r, label)) in buttons.iter().enumerate().filter(|b| b.1.2 != "-") {
                let on = held.values().any(|b| *b == i) || (i == 3 && player.fly);
                draw_circle(c.x, c.y, *r, Color::new(0.0, 0.0, 0.0, if on { 0.55 } else { 0.32 }));
                draw_circle_lines(c.x, c.y, *r, 2.0, Color::new(1.0, 1.0, 1.0, 0.55));
                text_centered(label, c.x, c.y + 6.0, (*r * 0.5).max(13.0), WHITE, false);
            }
        } else if !grabbed && !chars_open && !steve.inv_open && sk_test.is_none() {
            text_centered("CLIQUE PRA ENTRAR NA VILA", sw * 0.5, sh * 0.5 + 60.0, 36.0, WHITE, true);
        }
        if ch == 0 {
            steve.draw_overlay(&atlas, player.sel, sw, sh, mobile);
            if steve.inv_open {
                uni.draw_bag(sw, |s, x, y, z, c| text_centered(s, x, y, z, c, false));
            }
        }
        if typing.is_none() && !chars_open && !steve.inv_open {
            hub.draw_prompt(sw, sh, mobile);
        }
        if chars_open {
            draw_rectangle(0.0, 0.0, sw, sh, Color::new(0.0, 0.0, 0.0, 0.55));
            text_centered("ESCOLHE O PERSONAGEM", sw * 0.5, sh * 0.5 - (sh * 0.36).min(200.0) * 0.5 - 24.0, 34.0, WHITE, true);
            uni.draw_wardrobe(sw, sh, |s, x, y, z, c| text_centered(s, x, y, z, c, true));
            for (i, r) in char_cards(sw, sh).iter().enumerate() {
                let code = match ch {
                    0 => steve.creative as u8,
                    1 => 2,
                    2 | 3 => 3,
                    c => c,
                };
                let sel = CHARS[i].2 == code;
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.1, 0.1, 0.15, 0.9));
                draw_rectangle_lines(r.x, r.y, r.w, r.h, if sel { 4.0 } else { 2.0 }, if sel { Color::new(1.0, 0.85, 0.3, 1.0) } else { GRAY });
                text_centered(&format!("{}", i + 1), r.x + r.w * 0.5, r.y + r.h * 0.25, (r.w * 0.3).min(44.0), Color::new(1.0, 0.85, 0.3, 1.0), false);
                let name_size = (r.w / 7.0).clamp(12.0, 26.0);
                for (k, word) in CHARS[i].0.split(' ').enumerate() {
                    text_centered(word, r.x + r.w * 0.5, r.y + r.h * 0.5 + k as f32 * (name_size + 4.0), name_size, WHITE, true);
                }
                if r.w > 120.0 {
                    text_centered(CHARS[i].1, r.x + r.w * 0.5, r.y + r.h * 0.86, (r.w / 14.0).min(15.0), Color::new(0.8, 0.8, 0.85, 1.0), false);
                }
            }
            text_centered(if mobile { "TOCA NUM PERSONAGEM" } else { "CLICA OU APERTA 1-8 | C FECHA" }, sw * 0.5, sh * 0.5 + (sh * 0.36).min(200.0) * 0.5 + 34.0, 20.0, WHITE, false);
            let qb = quality_button(sw, sh);
            draw_rectangle(qb.x, qb.y, qb.w, qb.h, Color::new(0.1, 0.1, 0.15, 0.9));
            draw_rectangle_lines(qb.x, qb.y, qb.w, qb.h, 2.0, Color::new(0.45, 1.0, 1.0, 1.0));
            text_centered(&format!("{}{}", quality.label(), if mobile { "" } else { " (F4)" }), sw * 0.5, qb.y + qb.h * 0.5 + 7.0, 20.0, WHITE, false);
        }
        prof.draw(&quality.label());
        prof.mark(prof::UI);

        if let (Some(t), Some(sk)) = (sk_test.as_mut(), skater.as_mut()) {
            if t.after_frame(sk) {
                std::process::exit(0);
            }
        }
        if wolvie.after_frame() {
            std::process::exit(0);
        }
        next_frame().await;
    }
}

fn who(id: u64, online: bool, my_id: u64, my_name: &str, remotes: &HashMap<u64, Remote>) -> String {
    if !online || id == my_id {
        return my_name.to_string();
    }
    remotes.get(&id).map(|r| r.name.clone()).unwrap_or_else(|| "???".into())
}

fn rgb_green() -> Color {
    Color::new(0.2, 1.0, 0.35, 1.0)
}

/// Todo NPC vivo que dá pra acertar: (centro, raio, índice, grupo).
#[allow(clippy::too_many_arguments)]
fn npc_targets(fighters: &[Fighter], villagers: &[Villager], guests: &[actors::Guest], npcs: &npc::Npcs, urna: &Urna, mods: &mods::Mods, kaiju: &kaiju::Kaiju, time: f32) -> Vec<gta::Target> {
    let up = Vec3::Y;
    let mut t: Vec<gta::Target> = fighters.iter().enumerate().filter(|(_, f)| f.active()).map(|(i, f)| (f.pos + up, 0.8, i, npc::FIGHTER)).collect();
    t.extend(villagers.iter().enumerate().filter(|(i, _)| npcs.alive(npc::VILLAGER, *i)).map(|(i, v)| (v.pos + up * 0.9, 0.7, i, npc::VILLAGER)));
    t.extend(guests.iter().enumerate().filter(|(i, _)| npcs.alive(npc::GUEST, *i)).map(|(i, g)| (g.pos + up, 0.7, i, npc::GUEST)));
    t.extend((0..extras::ROBOTS).filter(|i| npcs.alive(npc::ROBOT, *i)).map(|i| (extras::Lab::robot_at(i, time).0 + up * 1.2, 0.9, i, npc::ROBOT)));
    if npcs.urna().alive() {
        t.push((urna.pos, 4.6, 0, npc::URNA));
    }
    if npcs.eu().alive() {
        t.push((extras::eu_base(time), 4.0, 0, npc::EU));
    }
    if npcs.voador().alive() {
        t.push((voador::pos(time), 2.6, 0, npc::VOADOR));
    }
    if npcs.guard().alive() {
        let (c, r) = lab::guard_target(time);
        t.push((c, r, 0, npc::GUARD));
    }
    t.extend(mods.targets(npcs));
    t.extend(kaiju.targets(npcs));
    t.extend(zeppelin::targets(time, npcs));
    t
}

/// Host: dano num NPC (não lutador). Retorna true se foi a urna que morreu (o main manda a explosão).
#[allow(clippy::too_many_arguments)]
fn npc_hit(npcs: &mut npc::Npcs, villagers: &mut [Villager], g: u8, i: usize, dmg: f32, dir: Vec3, at: Vec3, events: &mut Vec<Ev>) -> bool {
    let Some(died) = npcs.hit(g, i, dmg) else { return false };
    let up = Vec3::Y;
    if g == npc::VILLAGER && (died || dmg >= 10.0) {
        if let Some(v) = villagers.get_mut(i) {
            v.airborne = true;
            v.vel = dir * (4.0 + dmg * 0.15).min(12.0) + up * (3.0 + dmg * 0.1).min(9.0);
        }
    }
    events.push(Ev::Hit { pos: at, claws: false });
    if !died {
        return false;
    }
    let (txt, col) = match g {
        npc::VILLAGER => ("MORREU!", RED),
        npc::GUEST => ("FOI DE BASE!", RED),
        npc::ROBOT => ("CURTO-CIRCUITO!", YELLOW),
        npc::URNA => ("URNA DESTRUIDA!!!", ORANGE),
        npc::VOADOR => ("MITO ABATIDO!!!", ORANGE),
        npc::GUARD => ("SINAPSE-9 DESLIGOU!", ORANGE),
        npc::MODS => ("MOD ABATIDO!", ORANGE),
        npc::KAIJU => ("GODZILHA ABATIDO!!!", ORANGE),
        npc::ZEPPELIN => ("ZEPELIM ABATIDO!!!", ORANGE),
        npc::MARIO => ("ENCANADOR ABATIDO!", ORANGE),
        npc::GUMBA => ("COGUMAU AMASSADO!", YELLOW),
        npc::BOMBA => ("BOMBADINHO DESARMADO!", ORANGE),
        _ => ("A IA CAIU!!!", ORANGE),
    };
    events.push(Ev::Text { pos: at + up * 1.2, text: txt.into(), color: col, big: g >= npc::URNA && g != npc::GUMBA });
    match g {
        npc::URNA => {
            events.push(Ev::Banner("A URNA CAIU! APURACAO SUSPENSA".into()));
            events.push(Ev::Shake(1.0));
        }
        npc::EU => events.push(Ev::Banner("DERRUBARAM A IA! (O ESCUDO FICA, JA TAVA COMPILADO)".into())),
        npc::VOADOR => events.push(Ev::Banner("DERRUBARAM O BOLSONARO VOADOR! ELE VAI RECORRER...".into())),
        _ => {}
    }
    g == npc::URNA
}

fn draw_star(x: f32, y: f32, r: f32, col: Color) {
    let p = |k: usize, rr: f32| {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        vec2(x + a.cos() * rr, y + a.sin() * rr)
    };
    for k in 0..5 {
        draw_triangle(vec2(x, y), p(2 * k, r), p(2 * k + 1, r * 0.45), col);
        draw_triangle(vec2(x, y), p(2 * k + 1, r * 0.45), p(2 * k + 2, r), col);
    }
}

/// HUD no clima GTA 3, canto superior direito: arma + munição, relógio, grana, vida, estrelas.
fn draw_gta_hud(b: &gta::Bandido, sw: f32, time: f32) {
    let x_r = sw - 12.0;
    let y = 72.0;
    let s = 64.0;
    draw_rectangle(x_r - s, y, s, s, Color::new(0.0, 0.0, 0.0, 0.45));
    draw_rectangle_lines(x_r - s, y, s, s, 2.0, Color::new(1.0, 1.0, 1.0, 0.6));
    gta::draw_icon(b.weapon, x_r - s, y, s);
    let ammo = b.ammo_text();
    let d = measure_text(&ammo, None, 16, 1.0);
    draw_text(&ammo, x_r - s * 0.5 - d.width * 0.5, y + s + 15.0, 16.0, WHITE);
    let tx = x_r - s - 12.0;
    let right = |txt: &str, yy: f32, size: f32, col: Color| {
        let d = measure_text(txt, None, size as u16, 1.0);
        draw_text(txt, tx - d.width + 2.0, yy + 2.0, size, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text(txt, tx - d.width, yy, size, col);
    };
    let mins = (time * 2.0) as u32;
    right(&format!("{:02}:{:02}", 8 + mins / 60 % 24, mins % 60), y + 20.0, 26.0, Color::new(0.85, 0.85, 0.8, 1.0));
    right(&format!("${:08}", b.cash), y + 50.0, 34.0, Color::new(0.38, 0.55, 0.85, 1.0));
    let hp_col = Color::new(1.0, 0.45, 0.55, 1.0);
    let hp = format!("{:03}", b.health.ceil() as i32);
    right(&hp, y + 80.0, 30.0, hp_col);
    let hd = measure_text(&hp, None, 30, 1.0);
    let hx = tx - hd.width - 18.0;
    draw_circle(hx - 5.0, y + 66.0, 6.0, hp_col);
    draw_circle(hx + 5.0, y + 66.0, 6.0, hp_col);
    draw_triangle(vec2(hx - 11.0, y + 68.0), vec2(hx + 11.0, y + 68.0), vec2(hx, y + 80.0), hp_col);
    for k in 0..6u32 {
        let cx = x_r - 12.0 - k as f32 * 26.0;
        let on = k < b.stars();
        let blink = on && b.calm < 3.0 && (time * 6.0).sin() > 0.0;
        draw_star(cx + 1.5, y + s + 40.0 + 1.5, 11.0, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_star(cx, y + s + 40.0, 11.0, if blink { WHITE } else if on { Color::new(1.0, 0.8, 0.15, 1.0) } else { Color::new(0.3, 0.3, 0.3, 0.5) });
    }
}
