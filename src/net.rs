//! WebSocket nativo (thread + tungstenite). Mesma API do net_web.rs.

use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Error, Message};

pub const CONNECTING: i32 = 0;
pub const OPEN: i32 = 1;
pub const CLOSED: i32 = 2;

pub struct Net {
    tx: Sender<String>,
    rx: Receiver<String>,
    state: Arc<AtomicI32>,
}

fn tcp(s: &MaybeTlsStream<TcpStream>) -> Option<&TcpStream> {
    match s {
        MaybeTlsStream::Plain(t) => Some(t),
        MaybeTlsStream::NativeTls(t) => Some(t.get_ref()),
        _ => None,
    }
}

impl Net {
    /// url: wss://.../ws (vazio = offline).
    pub fn connect(url: &str) -> Net {
        let (tx, out_rx) = mpsc::channel::<String>();
        let (in_tx, rx) = mpsc::channel::<String>();
        let state = Arc::new(AtomicI32::new(if url.is_empty() { CLOSED } else { CONNECTING }));
        if !url.is_empty() {
            let (url, st) = (url.to_string(), state.clone());
            thread::spawn(move || {
                let Ok((mut ws, _)) = tungstenite::connect(url.as_str()) else {
                    st.store(CLOSED, Ordering::Relaxed);
                    return;
                };
                if let Some(t) = tcp(ws.get_ref()) {
                    let _ = t.set_read_timeout(Some(Duration::from_millis(5)));
                }
                st.store(OPEN, Ordering::Relaxed);
                'outer: loop {
                    while let Ok(m) = out_rx.try_recv() {
                        if ws.send(Message::text(m)).is_err() {
                            break 'outer;
                        }
                    }
                    match ws.read() {
                        Ok(Message::Text(t)) => {
                            if in_tx.send(t.to_string()).is_err() {
                                break;
                            }
                        }
                        Ok(_) => {}
                        Err(Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                        Err(_) => break,
                    }
                }
                st.store(CLOSED, Ordering::Relaxed);
            });
        }
        Net { tx, rx, state }
    }

    pub fn state(&self) -> i32 {
        self.state.load(Ordering::Relaxed)
    }

    pub fn send(&self, s: String) {
        if self.state() == OPEN {
            let _ = self.tx.send(s);
        }
    }

    pub fn poll(&mut self) -> Vec<String> {
        self.rx.try_iter().collect()
    }
}
