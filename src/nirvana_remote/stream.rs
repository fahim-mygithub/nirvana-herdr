//! Plan 39 P39b: the page's live views, as Server-Sent Events.
//!
//! Why SSE, and not a WebSocket or a long poll:
//! - It is a plain GET with a streamed answer, so it goes through `tailscale serve` unchanged. tailscaled's proxy is
//!   Go's `httputil.ReverseProxy` (record p39a read `ipn/ipnlocal/serve.go`), which flushes each write at once for
//!   `text/event-stream` (its `flushInterval`); a WebSocket needs the upgrade carried, and this server would need a
//!   frame codec. *Not measured on the owner's tailnet: the live check's step for it is in the record.*
//! - Safari has had `EventSource` since 5, and it reconnects by itself after a drop, waiting the `retry` this sends.
//!   When iPhone Safari puts the page in the background it drops the connection; the page opens a new one when it is
//!   shown again (`app.js`, `visibilitychange`), and the first event of every stream is the whole state, so nothing
//!   is missed while it was away. When the PC's herdr restarts, the proxy answers 502 until Nirvana Remote is back,
//!   which closes an `EventSource` for good, so the page opens it again itself with a growing wait.
//! - A long poll would cost a request per change and a gap between them.
//!
//! Each view holds one of the 32 connection slots for as long as it is open, so at most `MOST` are open at once: a
//! new one ends the oldest (a phone that came back from the background opens a new view before the old one's
//! connection is found dead). A view also ends after `LIFETIME`, and the page reconnects, and within a `WAKE` of
//! the owner's login or this PC's Tailscale name changing under it (`run`'s `still`). A comment every `BEAT`
//! finds a reader that went away (the write fails). Stopping Nirvana Remote ends them all (`close_all`).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::http::{Conn, Response};

/// Live views open at once.
pub const MOST: usize = 6;
/// A view ends after this long; the page opens a new one.
pub const LIFETIME: Duration = Duration::from_secs(15 * 60);
/// A comment line this often, so a reader that went away is found.
pub const BEAT: Duration = Duration::from_secs(20);
/// How often a view wakes to see whether it was told to end.
pub const WAKE: Duration = Duration::from_secs(1);
/// What `EventSource` waits before it reconnects by itself, in ms.
pub const RETRY_MS: u64 = 2000;

#[derive(Default)]
pub struct Streams {
    next: AtomicU64,
    open: Mutex<Vec<(u64, Arc<AtomicBool>)>>,
}

/// One open view; it leaves the list when dropped.
pub struct Ticket {
    id: u64,
    ended: Arc<AtomicBool>,
    streams: Arc<Streams>,
    opened: Instant,
}

impl Ticket {
    /// Told to end, or past its lifetime.
    pub fn over(&self) -> bool {
        self.ended.load(Ordering::SeqCst) || self.opened.elapsed() >= LIFETIME
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if let Ok(mut open) = self.streams.open.lock() {
            open.retain(|(id, _)| *id != self.id);
        }
    }
}

impl Streams {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// A ticket for a new view; when `MOST` are open, the oldest is told to end.
    pub fn open(self: &Arc<Self>) -> Ticket {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let ended = Arc::new(AtomicBool::new(false));
        if let Ok(mut open) = self.open.lock() {
            while open.len() >= MOST {
                let (_, oldest) = open.remove(0);
                oldest.store(true, Ordering::SeqCst);
            }
            open.push((id, ended.clone()));
        }
        Ticket {
            id,
            ended,
            streams: self.clone(),
            opened: Instant::now(),
        }
    }

    /// End every view: Nirvana Remote is stopping or its mount went away.
    pub fn close_all(&self) {
        if let Ok(mut open) = self.open.lock() {
            for (_, ended) in open.drain(..) {
                ended.store(true, Ordering::SeqCst);
            }
        }
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.open.lock().map_or(0, |open| open.len())
    }
}

/// One event as the wire carries it: `event:` and one `data:` line (JSON has no raw newline).
pub fn event(name: &str, data: &str) -> String {
    format!("event: {name}\ndata: {data}\n\n")
}

/// The live view's head; `head` carries the route's headers (the CSP).
pub fn begin(conn: &mut Conn, head: Response) -> std::io::Result<()> {
    conn.stream_head(&head)?;
    conn.write(format!("retry: {RETRY_MS}\n\n").as_bytes())
}

/// Run a view until it is over, its reader is gone, or `still` says the caller would no longer be let in (review
/// round, S6: the checks run once on the request, so a view asks again each `WAKE`): `next` is called at least each
/// `WAKE` and returns what to send (nothing, or events), or `None` to end the view.
pub fn run(
    conn: &mut Conn,
    ticket: &Ticket,
    still: impl Fn() -> bool,
    mut next: impl FnMut(Duration) -> Option<Vec<String>>,
) {
    let mut wrote = Instant::now();
    while !ticket.over() && still() {
        let Some(parts) = next(WAKE) else {
            return;
        };
        for part in parts {
            if conn.write(part.as_bytes()).is_err() {
                return;
            }
            wrote = Instant::now();
        }
        if wrote.elapsed() >= BEAT {
            if conn.write(b": beat\n\n").is_err() {
                return;
            }
            wrote = Instant::now();
        }
    }
}
