//! URNA-MINE-VERINE: clone de Minecraft em Rust com vila, clube de house,
//! briga generalizada e uma urna eletrônica gigante soltando laser.

mod actors;
mod atlas;
#[cfg_attr(target_arch = "wasm32", path = "audio_web.rs")]
mod audio;
mod batch;
mod club;
mod eleicao;
mod models;
mod mp;
#[cfg_attr(target_arch = "wasm32", path = "net_web.rs")]
mod net;
mod player;
mod synth;
#[cfg_attr(target_arch = "wasm32", path = "telao_web.rs")]
mod telao;
mod urna;
#[cfg(target_arch = "wasm32")]
mod web;
mod world;

use actors::{Ev, FState, Fighter, VKind, Villager};
use audio::{Audio, Clip};
use batch::Batch;
use macroquad::prelude::*;
use macroquad::rand::gen_range;
use models::{Look, Pose, draw_flag, draw_humanoid, draw_villager, rgb, root};
use player::{HOTBAR, Player};
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
}

struct FloatText {
    pos: Vec3,
    text: String,
    color: Color,
    t: f32,
    big: bool,
}

struct Label {
    pos: Vec3,
    text: String,
    size: f32,
    color: Color,
}

struct Remote {
    name: String,
    pos: Vec3,
    target: Vec3,
    yaw: f32,
    walk: f32,
    look: Look,
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

fn text_centered(s: &str, x: f32, y: f32, size: f32, color: Color, bg: bool) {
    let dim = measure_text(s, None, size as u16, 1.0);
    if bg {
        draw_rectangle(x - dim.width * 0.5 - 4.0, y - dim.height - 4.0, dim.width + 8.0, dim.height + 8.0, Color::new(0.0, 0.0, 0.0, 0.45));
    }
    draw_text(s, x - dim.width * 0.5 + 2.0, y + 2.0, size, Color::new(0.0, 0.0, 0.0, color.a * 0.7));
    draw_text(s, x - dim.width * 0.5, y, size, color);
}

fn build_all(world: &mut World, tex: &Texture2D) -> Vec<Vec<Mesh>> {
    let m = (0..CX * CZ).map(|k| world.build_chunk(k % CX, k / CX, tex)).collect();
    world.dirty.fill(false);
    m
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
        clear_background(Color::new(0.05, 0.02, 0.1, 1.0));
        let (cx, cy) = (screen_width() * 0.5, screen_height() * 0.5);
        text_centered("URNA-MINE-VERINE ONLINE", cx, cy - 90.0, 56.0, Color::new(1.0, 0.4, 0.9, 1.0), false);
        text_centered("DIGITA TEU NOME E APERTA ENTER", cx, cy - 30.0, 30.0, WHITE, false);
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
        "reset" => {
            *world = World::generate();
            None
        }
        _ => None,
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    rand::srand(macroquad::miniquad::date::now() as u64);
    let url = server_url();
    let my_name = if url.is_some() { ask_name().await } else { "JOGADOR".to_string() };
    for _ in 0..2 {
        clear_background(Color::new(0.05, 0.02, 0.1, 1.0));
        text_centered("CARREGANDO A VILA... SINTETIZANDO O HOUSE...", screen_width() * 0.5, screen_height() * 0.5, 40.0, WHITE, false);
        next_frame().await;
    }

    let atlas = atlas::build();
    let mut world = World::generate();
    let mut chunks = build_all(&mut world, &atlas.tex);

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
    };
    let house = audio.play(&Arc::new(synth::house_loop(sr)), 0.6, true);
    let mut telao = Telao::new(&audio);
    let mut tv_src = initial_source();
    telao.load(&tv_src);
    let guests = actors::spawn_guests();
    let relogio = eleicao::Relogio::new();

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
    let (mut villagers, mut fighters) = spawn_actors();
    let mut urna = Urna::new();
    let mut fx = Fx::default();
    let mut texts: Vec<FloatText> = Vec::new();
    let mut events: Vec<Ev> = Vec::new();
    let mut labels: Vec<Label> = Vec::new();
    let mut opaque = Batch::new();
    let mut trans = Batch::new();

    let mut grabbed = false;
    let mut last_mouse: Vec2 = mouse_position().into();
    let mut show_help = true;
    let mut muted = false;
    let mut banner: Option<(String, f32)> = Some(("BEM-VINDO A VILA. O HOUSE TA TOCANDO.".into(), 4.0));
    let mut wolverine_called = false;
    let mut was_event = false;
    let mut fireworks_t = 0.0f32;
    let start = get_time();
    let mut time_offset = 0.0f64;
    let up = vec3(0.0, 1.0, 0.0);

    loop {
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
                            remotes.insert(pid, Remote { name: p[1].as_str().unwrap_or("?").into(), pos: Vec3::ZERO, target: Vec3::ZERO, yaw: 0.0, walk: 0.0, look: remote_look(pid) });
                        }
                    }
                    world = World::generate();
                    let mut scratch = Fx::default();
                    for w in m["log"].as_array().into_iter().flatten() {
                        apply_world(&mut world, w, &mut scratch, &atlas.avg);
                    }
                    chunks = build_all(&mut world, &atlas.tex);
                    if let Some(u) = m["tv"].as_str().filter(|u| *u != tv_src) {
                        tv_src = u.to_string();
                        telao.load(&tv_src);
                    }
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
                    remotes.insert(id, Remote { name: n, pos: Vec3::ZERO, target: Vec3::ZERO, yaw: 0.0, walk: 0.0, look: remote_look(id) });
                }
                "leave" => {
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
                        r.target = p;
                        r.yaw = mp::f(&m["y"]);
                    }
                }
                "chat" => {
                    let who = who(id, online, my_id, &my_name, &remotes);
                    chat.push((format!("{who}: {}", m["m"].as_str().unwrap_or("")), get_time()));
                }
                "tv" => {
                    tv_src = m["u"].as_str().unwrap_or(telao::DEFAULT_URL).to_string();
                    telao.load(&tv_src);
                    banner = Some((format!("{} TROCOU O TELAO", who(id, online, my_id, &my_name, &remotes)), 2.5));
                }
                "w" => match apply_world(&mut world, &m, &mut fx, &atlas.avg) {
                    Some((plan, shot)) => {
                        let eye = player.eye();
                        play_at(&audio, &sfx.laser, plan.o, eye, 1.0, muted, in_club);
                        match shot {
                            Shot::Deflected(p) => {
                                play_at(&audio, &sfx.deflect, p, eye, 1.2, muted, in_club);
                                texts.push(FloatText { pos: p, text: "ESCUDO DO HOUSE!".into(), color: SKYBLUE, t: 0.0, big: false });
                            }
                            Shot::Exploded(p, r) => {
                                play_at(&audio, &sfx.boom, p, eye, 1.5, muted, in_club);
                                if !in_club {
                                    fx.shake = fx.shake.max((1.0 - p.distance(eye) / 70.0).max(0.0) * 1.2);
                                }
                                if is_host {
                                    actors::blast_fighters(&mut fighters, p, r, time, &mut events);
                                    actors::blast_villagers(&mut villagers, p, r);
                                }
                                let d = (player.pos + up).distance(p);
                                if d < r * 2.2 {
                                    let k = 1.0 - d / (r * 2.2);
                                    let dir = (player.pos - p).normalize_or_zero();
                                    player.knock += vec3(dir.x, 0.0, dir.z) * 14.0 * k + up * 9.0 * k;
                                }
                                if r > 5.0 {
                                    texts.push(FloatText { pos: p + up * 3.0, text: "CONFIRMA!!!".into(), color: rgb_green(), t: 0.0, big: true });
                                }
                            }
                        }
                    }
                    None => {
                        if m["k"] == "reset" {
                            chunks = build_all(&mut world, &atlas.tex);
                            let w_on = fighters[3].spawned;
                            (villagers, fighters) = spawn_actors();
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
                    match m["k"].as_str().unwrap_or("") {
                        "pf" if i < fighters.len() => {
                            let f = &mut fighters[i];
                            if f.active() {
                                f.hp -= 6.0;
                                f.vel += fwh * 7.0 + up * 3.0;
                                f.flash = 1.0;
                                if f.hp <= 0.0 {
                                    f.hp = 0.0;
                                    f.state = FState::Ko(5.0);
                                    f.ko_t = 0.0;
                                } else {
                                    f.state = FState::Stun(0.3);
                                }
                                events.push(Ev::Text { pos: f.pos + up * 2.2, text: format!("SOCO DE {}!", who(id, online, my_id, &my_name, &remotes)), color: YELLOW, big: false });
                                events.push(Ev::Hit { pos: f.pos + up, claws: false });
                            }
                        }
                        "pv" if i < villagers.len() => {
                            let v = &mut villagers[i];
                            v.airborne = true;
                            v.vel = fwh * 8.0 + up * 7.0;
                            events.push(Ev::Text { pos: v.pos + up * 2.2, text: "HMMM!".into(), color: WHITE, big: false });
                            events.push(Ev::Hit { pos: v.pos + up, claws: false });
                        }
                        "wolv" if !fighters[3].spawned => {
                            fighters[3].spawn_wolverine();
                            wolverine_called = true;
                        }
                        _ => {}
                    }
                }
                "s" if !is_host => {
                    let target = mp::f(&m["time"]) as f64 - (get_time() - start);
                    time_offset = if (target - time_offset).abs() > 1.0 { target } else { time_offset + (target - time_offset) * 0.1 };
                    mp::apply_snapshot(&m, &mut urna, &mut fighters, &mut villagers, &mut fpos, &mut vpos, &mut events);
                }
                _ => {}
            }
        }

        // ------------------------------------------------ Input
        let mut just_grabbed = false;
        if !grabbed && is_mouse_button_pressed(MouseButton::Left) {
            grabbed = true;
            just_grabbed = true;
            set_cursor_grab(true);
            show_mouse(false);
        }
        if grabbed && typing.is_none() && (is_key_pressed(KeyCode::Tab) || is_key_pressed(KeyCode::Escape)) {
            grabbed = false;
            set_cursor_grab(false);
            show_mouse(true);
        }
        let mouse: Vec2 = mouse_position().into();
        let md = mouse - last_mouse;
        last_mouse = mouse;
        if grabbed && !just_grabbed {
            player.look(md);
        }
        if let Some(msg) = typing.as_mut() {
            if type_into(msg, 120) {
                let m = msg.trim().to_string();
                if !m.is_empty() {
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
            if is_key_pressed(KeyCode::M) {
                muted = !muted;
            }
            if is_key_pressed(KeyCode::Y) {
                #[cfg(target_arch = "wasm32")]
                let src = web::prompt();
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
            if is_key_pressed(KeyCode::R) {
                send(json!({"t": "w", "k": "reset"}), &mut loopback);
            }
            for (k, key) in [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9].iter().enumerate() {
                if is_key_pressed(*key) {
                    player.sel = k;
                }
            }
        }
        if is_host && !wolverine_called && time > 20.0 {
            fighters[3].spawn_wolverine();
            wolverine_called = true;
            events.push(Ev::Banner("TEM ALGO CAINDO DO CEU...".into()));
        }
        let wheel = mouse_wheel().1;
        if wheel > 0.0 {
            player.sel = (player.sel + 8) % 9;
        } else if wheel < 0.0 {
            player.sel = (player.sel + 1) % 9;
        }

        player.update(&world, dt, grabbed && typing.is_none());
        let eye = player.eye();
        let fw = player.forward();
        let pick = world.raycast(eye, fw, 6.0);

        if grabbed && !just_grabbed && is_mouse_button_pressed(MouseButton::Left) {
            // Soco em lutador/villager tem prioridade sobre quebrar bloco
            let mut best: Option<(f32, usize, bool)> = None;
            for (i, f) in fighters.iter().enumerate() {
                if let Some(t) = f.spawned.then(|| urna::ray_sphere(eye, fw, f.pos + up, 0.75)).flatten() {
                    if t < 4.5 && best.is_none_or(|b| t < b.0) {
                        best = Some((t, i, true));
                    }
                }
            }
            for (i, v) in villagers.iter().enumerate() {
                if let Some(t) = urna::ray_sphere(eye, fw, v.pos + up, 0.7) {
                    if t < 4.5 && best.is_none_or(|b| t < b.0) {
                        best = Some((t, i, false));
                    }
                }
            }
            let fwh = vec3(fw.x, 0.0, fw.z).normalize_or_zero();
            if let Some((_, i, is_f)) = best {
                send(json!({"t": "a", "k": if is_f { "pf" } else { "pv" }, "i": i, "d": mp::v3(fwh)}), &mut loopback);
            } else if let Some((p, _, _)) = pick {
                if p.y > 0 {
                    let b = world.get(p.x, p.y, p.z);
                    send(json!({"t": "w", "k": "set", "p": [p.x, p.y, p.z], "b": AIR}), &mut loopback);
                    for _ in 0..8 {
                        fx.particles.push(urna::Particle {
                            pos: p.as_vec3() + Vec3::splat(0.5),
                            vel: vec3(gen_range(-2.0, 2.0), gen_range(1.0, 4.0), gen_range(-2.0, 2.0)),
                            col: atlas.avg[face_tile(b, 0)],
                            life: gen_range(0.4, 0.9),
                            size: 0.12,
                            gravity: true,
                        });
                    }
                }
            }
        }
        if grabbed && is_mouse_button_pressed(MouseButton::Right) {
            if let Some((_, prev, _)) = pick {
                if world.get(prev.x, prev.y, prev.z) == AIR {
                    world.set(prev.x, prev.y, prev.z, HOTBAR[player.sel]);
                    let blocked = player.collides_at(&world, player.pos);
                    world.set(prev.x, prev.y, prev.z, AIR);
                    if !blocked {
                        send(json!({"t": "w", "k": "set", "p": [prev.x, prev.y, prev.z], "b": HOTBAR[player.sel]}), &mut loopback);
                    }
                }
            }
        }

        // ------------------------------------------------ Simulação (host) / interpolação (demais)
        let evento = relogio.evento();
        if evento && !was_event {
            banner = Some(("AS ELEICOES COMECARAM!!! URNAS ABERTAS!".into(), 6.0));
            play_at(&audio, &sfx.boom, eye, eye, 1.0, muted, false);
        }
        was_event = evento;
        if is_host {
            actors::update_villagers(&mut villagers, &world, dt, time);
            actors::update_fighters(&mut fighters, &world, dt, time, &mut events);
            if evento && !urna.charging {
                urna.timer = urna.timer.min(0.35);
            }
            let mut targets: Vec<Vec3> = vec![player.pos];
            targets.extend(remotes.values().map(|r| r.pos));
            let shot_target = {
                let fs = &fighters;
                let vs = &villagers;
                urna.update(dt, time, || {
                    let roll = gen_range(0.0, 1.0);
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
            if let Some(target) = shot_target {
                let plan = urna::plan(&world, urna.eye(), target, if evento { 0.5 } else { 0.15 });
                send(mp::shot(&plan), &mut loopback);
            }
        } else {
            urna.bob(time);
            let k = (dt * 12.0).min(1.0);
            for (f, &p) in fighters.iter_mut().zip(&fpos) {
                f.pos = if f.pos.distance(p) > 6.0 { p } else { f.pos.lerp(p, k) };
            }
            for (v, &p) in villagers.iter_mut().zip(&vpos) {
                v.pos = if v.pos.distance(p) > 6.0 { p } else { v.pos.lerp(p, k) };
            }
        }
        for r in remotes.values_mut() {
            let moved = r.pos.distance(r.target);
            r.walk += moved * 3.0;
            r.pos = if moved > 8.0 { r.target } else { r.pos.lerp(r.target, (dt * 12.0).min(1.0)) };
        }

        // Fogos na abertura das urnas
        if evento {
            fireworks_t -= dt;
            if fireworks_t <= 0.0 {
                fireworks_t = gen_range(0.15, 0.4);
                let c = vec3(gen_range(30.0, 100.0), G as f32 + gen_range(22.0, 38.0), gen_range(30.0, 80.0));
                let hue = gen_range(0.0, 1.0);
                for _ in 0..40 {
                    let d = vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)).normalize_or_zero();
                    fx.particles.push(urna::Particle { pos: c, vel: d * gen_range(8.0, 14.0), col: club::hsv(hue + gen_range(-0.05, 0.05), 0.8, 1.0), life: gen_range(0.8, 1.5), size: 0.3, gravity: false });
                }
                play_at(&audio, &sfx.punch, c, eye, 0.8, muted, in_club);
            }
        }

        fx.update(&world, dt);
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
            net.send(json!({"t": "p", "p": mp::v3(player.pos), "y": fw.x.atan2(fw.z)}).to_string());
            if is_host {
                net.send(mp::snapshot(time, &urna, &fighters, &villagers, &ev_out).to_string());
                ev_out.clear();
            }
        }

        for t in texts.iter_mut() {
            t.t += dt;
            t.pos.y += dt * 0.8;
        }
        texts.retain(|t| t.t < 1.6);

        let mut rebuilt = 0;
        for k in 0..(CX * CZ) as usize {
            if world.dirty[k] && rebuilt < 8 {
                chunks[k] = world.build_chunk(k as i32 % CX, k as i32 / CX, &atlas.tex);
                world.dirty[k] = false;
                rebuilt += 1;
            }
        }

        // Volume da música pela distância do clube
        telao.update();
        let target_vol = if muted { 0.0 } else { 0.25 + 0.75 * (1.0 - (eye.distance(shield_center()) - 10.0) / 90.0).clamp(0.0, 1.0) };
        audio.set_volume(house, if telao.live { 0.0 } else { target_vol * 0.6 });
        telao.set_volume(target_vol);

        // ------------------------------------------------ Render 3D
        clear_background(Color::new(0.53, 0.75, 1.0, 1.0));
        let shake = vec3(gen_range(-1.0, 1.0), gen_range(-1.0, 1.0), gen_range(-1.0, 1.0)) * fx.shake * 0.35;
        let cam = Camera3D {
            position: eye + shake,
            target: eye + shake + fw,
            up,
            fovy: 70f32.to_radians(),
            ..Default::default()
        };
        set_camera(&cam);
        let vp = cam.matrix();

        // Céu: sol e nuvens
        let id = Mat4::IDENTITY;
        let sun_dir = vec3(0.4, 0.75, 0.3).normalize();
        opaque.glow(&id, eye + sun_dir * 160.0, Vec3::splat(16.0), Color::new(1.0, 0.95, 0.6, 1.0));
        for k in 0..22 {
            let kx = atlas::hash2(k, 1, 900) * 260.0 - 66.0;
            let kz = atlas::hash2(k, 2, 900) * 260.0 - 66.0;
            let x = (kx + time * 1.5).rem_euclid(260.0) - 66.0;
            let s = vec3(8.0 + atlas::hash2(k, 3, 900) * 12.0, 1.5, 6.0 + atlas::hash2(k, 4, 900) * 10.0);
            opaque.glow(&id, vec3(x, 62.0, kz), s, Color::new(1.0, 1.0, 1.0, 1.0));
        }

        for list in &chunks {
            for m in list {
                draw_mesh(m);
            }
        }

        labels.clear();
        club::draw(&mut opaque, &mut trans, time, beat);
        relogio.draw(&mut opaque, time);
        let bf = beat.fract();
        for (i, v) in villagers.iter().enumerate() {
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
        for g in &guests {
            let pose = actors::guest_pose(g, beat);
            let m = root(g.pos, g.yaw, 0.0, pose.bounce);
            draw_humanoid(&mut opaque, &g.look, &pose, &m);
            if g.pos.distance(eye) < 45.0 {
                labels.push(Label { pos: g.pos + up * 2.2, text: g.name.to_string(), size: 18.0, color: Color::new(1.0, 0.85, 0.3, 1.0) });
            }
        }
        for r in remotes.values() {
            let moving = (r.pos - r.target).length() > 0.05;
            let pose = Pose { walk: r.walk, walk_amt: if moving { 1.0 } else { 0.0 }, arm_l: -0.2, arm_r: -0.2, ..Default::default() };
            draw_humanoid(&mut opaque, &r.look, &pose, &root(r.pos, r.yaw, 0.0, 0.0));
            labels.push(Label { pos: r.pos + up * 2.2, text: r.name.clone(), size: 22.0, color: Color::new(0.5, 1.0, 0.6, 1.0) });
        }
        // Telão: moldura presa na parede oeste do clube, virado pro leste
        let (tx, tz0, tz1, ty0, ty1) = (9.21, 56.5, 72.5, G as f32 + 4.5, G as f32 + 13.5);
        opaque.cube(&id, vec3(9.1, (ty0 + ty1) * 0.5, (tz0 + tz1) * 0.5), vec3(0.2, ty1 - ty0 + 0.4, tz1 - tz0 + 0.4), Color::new(0.05, 0.05, 0.06, 1.0));
        urna.draw(&mut opaque, time);
        fx.draw_opaque(&mut opaque);
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

        fx.draw_transparent(&mut trans, time);
        trans.flush(&atlas.tex);
        fx.draw_flashes();
        let sc = shield_center();
        let sf = fx.shield_flash;
        let shimmer = 0.03 * (time * 2.0).sin();
        draw_sphere(sc, SHIELD_R, None, Color::new(0.45, 0.75 + 0.2 * sf, 1.0, 0.09 + shimmer + 0.25 * sf));
        draw_sphere_wires(sc, SHIELD_R + 0.05, None, Color::new(0.6, 0.9, 1.0, 0.12 + 0.4 * sf));

        // ------------------------------------------------ Render 2D
        set_default_camera();
        labels.push(Label { pos: vec3(36.5, G as f32 + 8.2, 64.5), text: "CLUB DO HOUSE - SO CURTINDO".into(), size: 30.0, color: Color::new(1.0, 0.4, 0.9, 1.0) });
        labels.push(Label { pos: urna.matrix().transform_point3(vec3(0.0, 6.0, 0.0)), text: "URNA ELETRONICA".into(), size: 32.0, color: Color::new(1.0, 0.85, 0.3, 1.0) });
        labels.push(Label { pos: urna.matrix().transform_point3(vec3(5.2, -3.6, 1.8)), text: "CONFIRMA".into(), size: 20.0, color: GREEN });
        for l in &labels {
            if let Some(s) = project(&vp, l.pos) {
                let d = l.pos.distance(eye);
                let size = (l.size * (14.0 / d.max(6.0)).clamp(0.55, 1.6)).max(13.0);
                text_centered(&l.text, s.x, s.y, size, l.color, true);
            }
        }
        for f in &fighters {
            if !f.spawned {
                continue;
            }
            if let Some(s) = project(&vp, f.pos + up * 2.05) {
                let w = 60.0;
                draw_rectangle(s.x - w * 0.5, s.y, w, 6.0, Color::new(0.0, 0.0, 0.0, 0.6));
                let k = f.hp / f.max_hp;
                draw_rectangle(s.x - w * 0.5, s.y, w * k, 6.0, Color::new(1.0 - k, k, 0.1, 1.0));
            }
        }
        for t in &texts {
            if let Some(s) = project(&vp, t.pos) {
                let a = (1.0 - t.t / 1.6).clamp(0.0, 1.0);
                let mut c = t.color;
                c.a = a;
                text_centered(&t.text, s.x, s.y, if t.big { 46.0 } else { 24.0 }, c, false);
            }
        }

        // HUD: placar
        let sw = screen_width();
        let sh = screen_height();
        let panel_w = 230.0;
        let n = fighters.len() as f32;
        let x0 = sw * 0.5 - (panel_w * n + 10.0 * (n - 1.0)) * 0.5;
        for (i, f) in fighters.iter().enumerate() {
            let x = x0 + i as f32 * (panel_w + 10.0);
            draw_rectangle(x, 10.0, panel_w, 52.0, Color::new(0.0, 0.0, 0.0, 0.55));
            draw_rectangle(x, 10.0, 6.0, 52.0, f.flag.0);
            draw_text(f.name, x + 12.0, 30.0, 20.0, WHITE);
            if f.spawned {
                let k = f.hp / f.max_hp;
                draw_rectangle(x + 12.0, 38.0, panel_w - 70.0, 12.0, Color::new(0.2, 0.2, 0.2, 1.0));
                draw_rectangle(x + 12.0, 38.0, (panel_w - 70.0) * k, 12.0, if f.berserk > 0.0 { RED } else { Color::new(1.0 - k, k, 0.15, 1.0) });
                draw_text(&format!("KO {}", f.kos), x + panel_w - 52.0, 50.0, 20.0, YELLOW);
                if matches!(f.state, FState::Ko(_)) {
                    draw_text("NOCAUTE", x + 14.0, 49.0, 16.0, WHITE);
                }
            } else {
                let left = (20.0 - time).max(0.0);
                draw_text(&format!("chegando em {:.0}s (K)", left), x + 12.0, 50.0, 18.0, GRAY);
            }
        }

        // Status da rede + jogadores online
        let status = match (url.is_some(), online, net.state()) {
            (false, _, _) => "OFFLINE (sem servidor configurado)".to_string(),
            (_, true, _) => format!("ONLINE {} jogador(es){}", remotes.len() + 1, if is_host { " | voce e o host" } else { "" }),
            (_, false, net::CONNECTING) => "CONECTANDO...".to_string(),
            _ => "OFFLINE (servidor fora do ar)".to_string(),
        };
        let dim = measure_text(&status, None, 20, 1.0);
        draw_text(&status, sw - dim.width - 12.0, 84.0, 20.0, if online { Color::new(0.5, 1.0, 0.6, 1.0) } else { GRAY });
        if online {
            let mut names: Vec<&str> = remotes.values().map(|r| r.name.as_str()).collect();
            names.insert(0, my_name.as_str());
            for (i, nm) in names.iter().take(12).enumerate() {
                let d = measure_text(nm, None, 18, 1.0);
                draw_text(nm, sw - d.width - 12.0, 106.0 + i as f32 * 20.0, 18.0, WHITE);
            }
        }

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

        // Mira
        draw_line(sw * 0.5 - 9.0, sh * 0.5, sw * 0.5 + 9.0, sh * 0.5, 2.0, WHITE);
        draw_line(sw * 0.5, sh * 0.5 - 9.0, sw * 0.5, sh * 0.5 + 9.0, 2.0, WHITE);

        // Hotbar
        let slot = 48.0;
        let hx = sw * 0.5 - slot * 4.5;
        let hy = sh - slot - 14.0;
        for (k, &b) in HOTBAR.iter().enumerate() {
            let x = hx + k as f32 * slot;
            draw_rectangle(x, hy, slot, slot, Color::new(0.0, 0.0, 0.0, 0.5));
            let t = face_tile(b, 0);
            draw_texture_ex(
                &atlas.tex,
                x + 6.0,
                hy + 6.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(slot - 12.0, slot - 12.0)),
                    source: Some(Rect::new(((t % atlas::COLS) * atlas::TILE) as f32, ((t / atlas::COLS) * atlas::TILE) as f32, atlas::TILE as f32, atlas::TILE as f32)),
                    ..Default::default()
                },
            );
            let border = if k == player.sel { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) };
            draw_rectangle_lines(x, hy, slot, slot, if k == player.sel { 4.0 } else { 2.0 }, border);
        }

        draw_text(&format!("URNA-MINE-VERINE  |  {} FPS  |  disparos da urna: {}", get_fps(), urna.shots), 12.0, sh - 80.0, 20.0, WHITE);
        let now_playing = if telao.live { format!("TELAO: {}", telao.title) } else { "house sintetizado 124 BPM (telao carregando...)".to_string() };
        draw_text(&format!("TOCANDO: {}{}", now_playing, if muted { " [MUDO]" } else { "" }), 12.0, sh - 58.0, 20.0, Color::new(1.0, 0.5, 0.9, 1.0));
        if show_help {
            let lines = [
                "WASD andar | ESPACO pular | SHIFT correr | F voar (CTRL desce)",
                "MOUSE olhar | ESQ quebrar/socar | DIR colocar | 1-9/RODA bloco",
                "K chama Wolverine | R reseta mundo | M muta | TAB solta mouse | H ajuda",
                "Y troca o video do telao (link do YouTube) | T ou ENTER chat",
            ];
            for (i, l) in lines.iter().enumerate() {
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
        if !grabbed {
            text_centered("CLIQUE PRA ENTRAR NA VILA", sw * 0.5, sh * 0.5 + 60.0, 36.0, WHITE, true);
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
