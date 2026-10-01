//! Ponte com o JavaScript do navegador (web/urna.js). Só compila em wasm32.

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    pub fn urna_ws_connect(ptr: *const u8, len: usize);
    pub fn urna_ws_state() -> i32;
    pub fn urna_ws_send(ptr: *const u8, len: usize);
    pub fn urna_ws_recv(ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_audio_rate() -> u32;
    pub fn urna_audio_clip(ptr: *const f32, len: usize, rate: u32) -> u32;
    pub fn urna_audio_play(clip: u32, vol: f32, looped: i32) -> u32;
    pub fn urna_audio_volume(voice: u32, vol: f32);
    pub fn urna_yt_load(ptr: *const u8, len: usize);
    pub fn urna_yt_state() -> i32;
    pub fn urna_yt_title(ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_yt_colors(ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_yt_place(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, visible: i32, vol: f32);
    pub fn urna_prompt(msg: *const u8, msg_len: usize, ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_query_name(ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_is_touch() -> i32;
    pub fn urna_open_url(ptr: *const u8, len: usize) -> i32;
    pub fn urna_now() -> f64;
    pub fn urna_prof(ptr: *const u8, len: usize);
    pub fn urna_query(key: *const u8, key_len: usize, ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_store_get(key: *const u8, key_len: usize, ptr: *mut u8, cap: usize) -> i32;
    pub fn urna_store_set(key: *const u8, key_len: usize, val: *const u8, val_len: usize);
    pub fn urna_clip_save();
    pub fn urna_clip_poll(tier: i32, ptr: *mut u8, cap: usize) -> i32;
}

/// Salva os últimos ~10s de jogo como vídeo (download local; ver web/urna.js).
pub fn clip_save() {
    unsafe { urna_clip_save() }
}

/// Informa a qualidade atual (BAIXA = grava só sob demanda) e devolve aviso do clipe pro chat.
pub fn clip_poll(tier: u8) -> Option<String> {
    read_string(|p, c| unsafe { urna_clip_poll(tier as i32, p, c) })
}

/// Parâmetro da URL (?chave=valor).
pub fn query(key: &str) -> Option<String> {
    read_string(|p, c| unsafe { urna_query(key.as_ptr(), key.len(), p, c) })
}

/// localStorage do navegador.
pub fn store_get(key: &str) -> Option<String> {
    read_string(|p, c| unsafe { urna_store_get(key.as_ptr(), key.len(), p, c) })
}

pub fn store_set(key: &str, val: &str) {
    unsafe { urna_store_set(key.as_ptr(), key.len(), val.as_ptr(), val.len()) }
}

/// Abre o link numa aba nova. false = popup bloqueado (o JS copia o link pro clipboard).
pub fn open_url(url: &str) -> bool {
    unsafe { urna_open_url(url.as_ptr(), url.len()) != 0 }
}

/// Lê uma string do JS: f escreve em (ptr, cap) e retorna o tamanho, -tamanho se não coube, 0 se vazio.
pub fn read_string(f: impl Fn(*mut u8, usize) -> i32) -> Option<String> {
    let mut buf = vec![0u8; 4096];
    loop {
        let n = f(buf.as_mut_ptr(), buf.len());
        if n == 0 {
            return None;
        }
        if n < 0 {
            buf.resize((-n) as usize, 0);
            continue;
        }
        buf.truncate(n as usize);
        return String::from_utf8(buf).ok();
    }
}

pub fn prompt(msg: &str) -> Option<String> {
    read_string(|p, c| unsafe { urna_prompt(msg.as_ptr(), msg.len(), p, c) })
}
