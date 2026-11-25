//! A shared club's connection: a WebSocket to the club's room (multiplayer/worker/src/club.js), run on its own thread so
//! the game never waits on the network. Messages go out as [`ToServer`] JSON and come back as [`ToGame`], read on the
//! thread (a save can be large).
//!
//! Desktop and Android: tungstenite over rustls (the same TLS as the downloads, net.rs). The browser: the page's own
//! WebSocket (web/opensgolf.js sg_ws_*), polled each frame.

use super::session::{ToGame, ToServer};

/// What the connection reports.
#[derive(Debug)]
pub enum Event {
    Open,
    Msg(ToGame),
    /// The connection ended (or never opened), and why.
    Closed(String),
}

pub struct Conn {
    #[cfg(not(target_arch = "wasm32"))]
    tx: std::sync::mpsc::Sender<String>,
    #[cfg(not(target_arch = "wasm32"))]
    rx: std::sync::mpsc::Receiver<Event>,
    #[cfg(target_arch = "wasm32")]
    id: i32,
    #[cfg(target_arch = "wasm32")]
    opened: bool,
    #[cfg(target_arch = "wasm32")]
    closed: bool,
}

impl std::fmt::Debug for Conn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Conn")
    }
}

/// The club's socket address: the board's address with ws(s) and the club's path.
pub fn club_url(base: &str, id: &str) -> String {
    let base = base.trim_end_matches('/');
    let base = if let Some(r) = base.strip_prefix("https://") {
        format!("wss://{r}")
    } else if let Some(r) = base.strip_prefix("http://") {
        format!("ws://{r}")
    } else {
        base.to_string()
    };
    format!("{base}/clubs/{id}/ws")
}

#[cfg(not(target_arch = "wasm32"))]
impl Conn {
    /// Starts connecting to `url` (ws:// or wss://).
    pub fn open(url: &str) -> Conn {
        let (tx, out_rx) = std::sync::mpsc::channel::<String>();
        let (in_tx, rx) = std::sync::mpsc::channel::<Event>();
        let url = url.to_string();
        std::thread::spawn(move || {
            let why = run(&url, &out_rx, &in_tx).err().unwrap_or_else(|| "closed".into());
            let _ = in_tx.send(Event::Closed(why));
        });
        Conn { tx, rx }
    }

    pub fn send(&self, m: &ToServer) {
        if let Ok(t) = serde_json::to_string(m) {
            let _ = self.tx.send(t);
        }
    }

    /// Everything that has come in since the last call.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut v = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => v.push(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // (the thread said Closed before it ended)
                    break;
                }
            }
        }
        v
    }
}

/// The connection's thread: connects, then sends what the game queues and hands on what comes in, until either side
/// closes (the game drops its [`Conn`]).
#[cfg(not(target_arch = "wasm32"))]
fn run(url: &str, out: &std::sync::mpsc::Receiver<String>, inc: &std::sync::mpsc::Sender<Event>) -> Result<(), String> {
    use std::sync::mpsc::TryRecvError;
    use std::time::Duration;
    use tungstenite::{stream::MaybeTlsStream, Message};
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("could not connect: {e}"))?;
    // a short read wait, so what the game sends goes out within it
    let tcp = match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s,
        MaybeTlsStream::Rustls(s) => s.get_mut(),
        _ => return Err("unknown stream".into()),
    };
    tcp.set_read_timeout(Some(Duration::from_millis(15))).map_err(|e| e.to_string())?;
    let _ = tcp.set_nodelay(true);
    if inc.send(Event::Open).is_err() {
        return Ok(());
    }
    loop {
        loop {
            match out.try_recv() {
                Ok(t) => ws.send(Message::text(t)).map_err(|e| format!("send: {e}"))?,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    let _ = ws.close(None);
                    let _ = ws.flush();
                    return Ok(());
                }
            }
        }
        match ws.read() {
            Ok(Message::Text(t)) => match serde_json::from_str::<ToGame>(t.as_str()) {
                Ok(m) => {
                    if inc.send(Event::Msg(m)).is_err() {
                        return Ok(());
                    }
                }
                Err(e) => return Err(format!("a message this game can't read ({e}): update the game")),
            },
            Ok(Message::Close(f)) => {
                return Err(f.map(|f| f.reason.to_string()).filter(|r| !r.is_empty()).unwrap_or_else(|| "closed".into()))
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                // nothing came in: a write of the queued messages may still be pending
                let _ = ws.flush();
            }
            Err(e) => return Err(format!("connection lost: {e}")),
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn sg_ws_open(url: *const u8, len: usize) -> i32;
    fn sg_ws_send(id: i32, text: *const u8, len: usize);
    /// 0 connecting, 1 open, 2 closed.
    fn sg_ws_state(id: i32) -> i32;
    /// The next message's length in bytes, or -1 when none has come in.
    fn sg_ws_next(id: i32) -> i32;
    fn sg_ws_take(id: i32, dst: *mut u8);
    fn sg_ws_reason(id: i32, dst: *mut u8, cap: usize) -> i32;
    fn sg_ws_close(id: i32);
}

#[cfg(target_arch = "wasm32")]
impl Conn {
    /// Starts connecting to `url` (ws:// or wss://) through the page.
    pub fn open(url: &str) -> Conn {
        // SAFETY: the page copies the address out of the wasm memory before returning.
        let id = unsafe { sg_ws_open(url.as_ptr(), url.len()) };
        Conn { id, opened: false, closed: false }
    }

    /// Queues a message (the page sends what was queued before the socket opened once it opens).
    pub fn send(&self, m: &ToServer) {
        if self.closed {
            return;
        }
        if let Ok(t) = serde_json::to_string(m) {
            // SAFETY: as open
            unsafe { sg_ws_send(self.id, t.as_ptr(), t.len()) };
        }
    }

    /// Everything that has come in since the last call.
    pub fn poll(&mut self) -> Vec<Event> {
        let mut v = Vec::new();
        if self.closed {
            return v;
        }
        // SAFETY: each take copies into a buffer of the length the page gave.
        let state = unsafe { sg_ws_state(self.id) };
        if state >= 1 && !self.opened {
            self.opened = true;
            if state == 1 {
                v.push(Event::Open);
            }
        }
        loop {
            let n = unsafe { sg_ws_next(self.id) };
            if n < 0 {
                break;
            }
            let mut buf = vec![0u8; n as usize];
            unsafe { sg_ws_take(self.id, buf.as_mut_ptr()) };
            match serde_json::from_slice::<ToGame>(&buf) {
                Ok(m) => v.push(Event::Msg(m)),
                Err(e) => {
                    self.closed = true;
                    unsafe { sg_ws_close(self.id) };
                    v.push(Event::Closed(format!("a message this game can't read ({e}): update the game")));
                    return v;
                }
            }
        }
        if state == 2 {
            self.closed = true;
            let mut buf = vec![0u8; 256];
            let k = unsafe { sg_ws_reason(self.id, buf.as_mut_ptr(), buf.len()) }.clamp(0, 256) as usize;
            let why = String::from_utf8_lossy(&buf[..k]).into_owned();
            v.push(Event::Closed(if why.is_empty() { "closed".into() } else { why }));
        }
        v
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for Conn {
    fn drop(&mut self) {
        // SAFETY: the page forgets the socket (closing it if it is still open)
        unsafe { sg_ws_close(self.id) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_club_address() {
        assert_eq!(club_url("https://b.example.dev/", "ab"), "wss://b.example.dev/clubs/ab/ws");
        assert_eq!(club_url("http://127.0.0.1:8787", "ab"), "ws://127.0.0.1:8787/clubs/ab/ws");
    }
}
