//! WebSocket do navegador (web/urna.js). Mesma API do net.rs.

use crate::web::*;

pub const CONNECTING: i32 = 0;
pub const OPEN: i32 = 1;

pub struct Net;

impl Net {
    /// url vazio = mesmo host da página (wss://host/ws).
    pub fn connect(url: &str) -> Net {
        unsafe { urna_ws_connect(url.as_ptr(), url.len()) };
        Net
    }

    pub fn state(&self) -> i32 {
        unsafe { urna_ws_state() }
    }

    pub fn send(&self, s: String) {
        unsafe { urna_ws_send(s.as_ptr(), s.len()) }
    }

    pub fn poll(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(m) = read_string(|p, c| unsafe { urna_ws_recv(p, c) }) {
            out.push(m);
        }
        out
    }
}
