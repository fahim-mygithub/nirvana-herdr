//! The smallest HTTP/1.1 server Nirvana Remote needs, on the tokio the herdr server already runs.
//!
//! Its only caller is tailscaled's `serve` proxy on this PC, one request per connection: a request head parsed by
//! `httparse` (hyper's own parser), an answer with its length, and the connection closed. A body is read only when a
//! route asks for one, after every check has passed on the head (`Conn::read_body`, P39b's send and upload routes),
//! bounded by the route's own limit both before a byte is read (a `Content-Length` over it) and while it streams (a
//! chunked body that grows past it). A live view (`Conn::stream_head`, P39b's event streams) writes its head without
//! a length and its events after it. Whatever the caller sent past what was read is drained after the answer
//! (review nit N1), so the close is a FIN, not a reset.

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A request head larger than this is refused (431) rather than buffered.
pub const HEAD_LIMIT: usize = 16 * 1024;
/// A caller that has not sent its whole head in this long is dropped.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// At most this many connections are served at once; the rest wait in the accept queue.
pub const CONNECTIONS: usize = 32;
const HEADERS: usize = 48;
/// After the answer, at most this much of what the caller still sends is read and dropped, for at most
/// `DRAIN_TIME`: Windows resets a socket closed with unread bytes, and a reset can cut the answer short. 8 MiB is a
/// second of a 64 Mbit/s uplink (review round, N4): a 413 or 400 answered on the head of a large upload, its body
/// unread, is drained while the caller stops sending, so it reads the answer instead of a reset. It was 64 KiB, and a
/// caller that sent a few MiB before reading lost the 413.
pub const DRAIN_LIMIT: usize = 8 * 1024 * 1024;
pub const DRAIN_TIME: Duration = Duration::from_secs(1);
/// The slowest a body may arrive on average, past its first `READ_TIMEOUT` (P39b): a 50 MiB upload gets 10 s plus
/// 800 s. That is about 0.5 Mbit/s, under any phone's uplink, and it keeps P39a's rule that a slow caller cannot hold
/// one of the 32 slots for as long as it likes.
pub const BODY_RATE: u64 = 64 * 1024;
/// A live view's write gives up after this long: a reader that stopped reading does not keep its slot.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// A chunk-size or trailer line longer than this is not one.
const CHUNK_LINE: usize = 1024;

/// The limits `serve` runs with; a test shortens the timeout to see its 408.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub read_timeout: Duration,
    pub connections: usize,
    /// Bytes a second a body must average once its first `read_timeout` is spent.
    pub body_rate: u64,
}

impl Limits {
    pub const DEFAULT: Self = Self {
        read_timeout: READ_TIMEOUT,
        connections: CONNECTIONS,
        body_rate: BODY_RATE,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
}

impl Request {
    /// Every value of a header, named in any case.
    pub fn header(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type,
            headers: Vec::new(),
            body: body.into(),
        }
    }

    pub fn json(status: u16, value: &serde_json::Value) -> Self {
        Self::new(status, "application/json", value.to_string())
    }

    pub fn text(status: u16, text: &str) -> Self {
        Self::new(status, "text/plain; charset=utf-8", text)
    }

    /// The answer of a route that wrote its own (a live view): nothing more goes on the wire.
    pub fn sent() -> Self {
        Self::new(0, "", Vec::new())
    }

    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    /// The bytes on the wire; a HEAD answer keeps the length and drops the body.
    pub fn encode(&self, head_only: bool) -> Vec<u8> {
        if self.status == 0 {
            return Vec::new();
        }
        let mut out = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\
             Cache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\n",
            self.status,
            reason(self.status),
            self.content_type,
            self.body.len()
        );
        for (name, value) in &self.headers {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str("\r\n");
        let mut bytes = out.into_bytes();
        if !head_only {
            bytes.extend_from_slice(&self.body);
        }
        bytes
    }

    /// The head of a live view: no length, since the body ends with the connection; the same safety headers.
    /// It is `encode`'s head less its length line, so the safety headers are written in one place.
    pub fn encode_stream_head(&self) -> Vec<u8> {
        let empty = Self {
            body: Vec::new(),
            ..self.clone()
        };
        String::from_utf8_lossy(&empty.encode(true))
            .replacen("Content-Length: 0\r\n", "", 1)
            .into_bytes()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// A head read so far: `Ok(None)` while it is incomplete, the status to refuse it with when it cannot be one.
#[cfg(test)]
pub fn parse_head(buffer: &[u8]) -> Result<Option<Request>, u16> {
    parse(buffer).map(|parsed| parsed.map(|(request, _)| request))
}

/// `parse_head`, with where the head ends: what follows is the body's first bytes.
fn parse(buffer: &[u8]) -> Result<Option<(Request, usize)>, u16> {
    let mut headers = [httparse::EMPTY_HEADER; HEADERS];
    let mut parsed = httparse::Request::new(&mut headers);
    let length = match parsed.parse(buffer) {
        Ok(httparse::Status::Complete(length)) => length,
        Ok(httparse::Status::Partial) if buffer.len() >= HEAD_LIMIT => return Err(431),
        Ok(httparse::Status::Partial) => return Ok(None),
        Err(httparse::Error::TooManyHeaders) => return Err(431),
        Err(_) => return Err(400),
    };
    let target = parsed.path.unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut out = Vec::with_capacity(parsed.headers.len());
    for header in parsed.headers.iter() {
        // A header that is not text is not one this server reads; refusing it keeps the identity check exact.
        let value = std::str::from_utf8(header.value).map_err(|_| 400u16)?;
        out.push((header.name.to_string(), value.to_string()));
    }
    Ok(Some((
        Request {
            method: parsed.method.unwrap_or("").to_string(),
            path: path.to_string(),
            query: query.to_string(),
            headers: out,
        },
        length,
    )))
}

/// How a request's body is framed, read from its head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    None,
    Length(u64),
    Chunked,
    /// Two lengths, a length and a coding at once (the request-smuggling shape), a length that is not plain digits,
    /// or a coding this server does not decode.
    Bad,
}

pub fn framing(request: &Request) -> Framing {
    let lengths = request.header("Content-Length");
    let codings = request.header("Transfer-Encoding");
    match (lengths.as_slice(), codings.as_slice()) {
        ([], []) => Framing::None,
        ([length], []) => {
            let length = length.trim();
            if length.is_empty() || !length.bytes().all(|b| b.is_ascii_digit()) {
                return Framing::Bad;
            }
            length.parse().map_or(Framing::Bad, Framing::Length)
        }
        ([], [coding]) if coding.trim().eq_ignore_ascii_case("chunked") => Framing::Chunked,
        _ => Framing::Bad,
    }
}

/// Why a body was not read whole; each has its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyError {
    /// Over the route's limit: said by its `Content-Length` before a byte is read, or found while it streams.
    TooLarge,
    /// No byte for `read_timeout`, or under `body_rate` on average.
    Timeout,
    /// A chunk line that is not one, a framing this server does not read, or a close before the end.
    Bad(&'static str),
    /// Where the body was going failed (the disk).
    Write(String),
}

impl BodyError {
    pub fn status(&self) -> u16 {
        match self {
            Self::TooLarge => 413,
            Self::Timeout => 408,
            Self::Bad(_) => 400,
            Self::Write(_) => 500,
        }
    }
}

/// The connection as a route sees it, on the blocking pool: the body to read when it wants one, and the socket to
/// write a live view to.
pub struct Conn {
    stream: std::net::TcpStream,
    buffer: Vec<u8>,
    at: usize,
    framing: Framing,
    limits: Limits,
    started: Instant,
}

impl Conn {
    pub fn new(
        stream: std::net::TcpStream,
        first: Vec<u8>,
        framing: Framing,
        limits: Limits,
    ) -> Self {
        Self {
            stream,
            buffer: first,
            at: 0,
            framing,
            limits,
            started: Instant::now(),
        }
    }

    pub fn framing(&self) -> Framing {
        self.framing
    }

    fn into_stream(self) -> std::net::TcpStream {
        self.stream
    }

    /// The bytes waiting, reading more when none are: at most `read_timeout` per read, never past `deadline`.
    fn more(&mut self, deadline: Instant) -> Result<&[u8], BodyError> {
        if self.at < self.buffer.len() {
            return Ok(&self.buffer[self.at..]);
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(BodyError::Timeout);
        }
        let wait = self
            .limits
            .read_timeout
            .min(deadline - now)
            .max(Duration::from_millis(1));
        let _ = self.stream.set_read_timeout(Some(wait));
        self.buffer.resize(64 * 1024, 0);
        self.at = 0;
        match self.stream.read(&mut self.buffer) {
            Ok(0) => {
                self.buffer.clear();
                Err(BodyError::Bad("the body ended early"))
            }
            Ok(read) => {
                self.buffer.truncate(read);
                Ok(&self.buffer[..])
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                self.buffer.clear();
                Err(BodyError::Timeout)
            }
            Err(_) => {
                self.buffer.clear();
                Err(BodyError::Bad("the connection failed"))
            }
        }
    }

    fn byte(&mut self, deadline: Instant) -> Result<u8, BodyError> {
        let byte = self.more(deadline)?[0];
        self.at += 1;
        Ok(byte)
    }

    /// Copy `count` bytes of the body into `sink`.
    fn copy(
        &mut self,
        mut count: u64,
        sink: &mut dyn Write,
        deadline: Instant,
    ) -> Result<(), BodyError> {
        while count > 0 {
            let waiting = self.more(deadline)?;
            let take = waiting
                .len()
                .min(usize::try_from(count).unwrap_or(usize::MAX));
            sink.write_all(&waiting[..take])
                .map_err(|err| BodyError::Write(err.to_string()))?;
            self.at += take;
            count -= take as u64;
        }
        Ok(())
    }

    fn line(&mut self, deadline: Instant) -> Result<String, BodyError> {
        let mut line = Vec::new();
        loop {
            match self.byte(deadline)? {
                b'\n' => break,
                byte => line.push(byte),
            }
            if line.len() > CHUNK_LINE {
                return Err(BodyError::Bad("a chunk line too long"));
            }
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        String::from_utf8(line).map_err(|_| BodyError::Bad("a chunk line that is not text"))
    }

    /// Read the whole body into `sink`, at most `limit` bytes; how many were read. A `Content-Length` over `limit`
    /// is refused before a byte is read; a chunked body is counted as it streams and refused the moment it passes
    /// `limit`. The deadline is `read_timeout` plus the body's size at `body_rate`.
    pub fn read_body(&mut self, limit: u64, sink: &mut dyn Write) -> Result<u64, BodyError> {
        let size = match self.framing {
            Framing::None => return Ok(0),
            Framing::Bad => return Err(BodyError::Bad("a body framing this server does not read")),
            Framing::Length(length) if length > limit => return Err(BodyError::TooLarge),
            Framing::Length(length) => length,
            Framing::Chunked => limit,
        };
        let rate = self.limits.body_rate.max(1);
        let deadline = self.started + self.limits.read_timeout + Duration::from_secs(size / rate);
        if let Framing::Length(length) = self.framing {
            self.copy(length, sink, deadline)?;
            self.framing = Framing::None;
            return Ok(length);
        }
        let mut total: u64 = 0;
        loop {
            let line = self.line(deadline)?;
            // RFC 9112 §7.1: `1*HEXDIG`, then optional whitespace before an extension. `from_str_radix` alone
            // would take a leading `+` or space (review round, N5).
            let hex = line
                .split(';')
                .next()
                .unwrap_or("")
                .trim_end_matches([' ', '\t']);
            if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(BodyError::Bad("a chunk size that is not hex"));
            }
            let chunk = u64::from_str_radix(hex, 16)
                .map_err(|_| BodyError::Bad("a chunk size that is not hex"))?;
            if chunk == 0 {
                // The trailer lines, up to the blank one.
                while !self.line(deadline)?.is_empty() {}
                self.framing = Framing::None;
                return Ok(total);
            }
            total = total.saturating_add(chunk);
            if total > limit {
                return Err(BodyError::TooLarge);
            }
            self.copy(chunk, sink, deadline)?;
            if !self.line(deadline)?.is_empty() {
                return Err(BodyError::Bad("a chunk longer than its size"));
            }
        }
    }

    /// Start a live view with `head` (`Response::encode_stream_head`); then `write` each part.
    pub fn stream_head(&mut self, head: &Response) -> std::io::Result<()> {
        let _ = self.stream.set_write_timeout(Some(WRITE_TIMEOUT));
        self.write(&head.encode_stream_head())
    }

    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.stream.write_all(bytes)?;
        self.stream.flush()
    }
}

pub trait Handler: Send + Sync + 'static {
    /// Whether the connection may send a request at all; asked before a byte of it is read.
    fn admit(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String>;
    /// The answer to a refused connection.
    fn refused(&self, peer: SocketAddr, why: &str) -> Response;
    /// The answer to an admitted request. It may block: it runs on tokio's blocking pool. It reads a body from
    /// `conn` only once its checks have passed; a live view writes to `conn` and answers `Response::sent()`.
    fn handle(&self, peer: SocketAddr, request: Request, conn: &mut Conn) -> Response;
}

/// Serve until `stop` turns true.
pub async fn serve(
    listener: tokio::net::TcpListener,
    handler: Arc<dyn Handler>,
    limits: Limits,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let slots = Arc::new(tokio::sync::Semaphore::new(limits.connections));
    loop {
        let permit = tokio::select! {
            permit = slots.clone().acquire_owned() => match permit {
                Ok(permit) => permit,
                Err(_) => return,
            },
            _ = stop.wait_for(|stopped| *stopped) => return,
        };
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            _ = stop.wait_for(|stopped| *stopped) => return,
        };
        let Ok((stream, peer)) = accepted else {
            continue;
        };
        let handler = handler.clone();
        tokio::spawn(async move {
            connection(stream, peer, handler, limits).await;
            drop(permit);
        });
    }
}

async fn connection(
    mut stream: tokio::net::TcpStream,
    peer: SocketAddr,
    handler: Arc<dyn Handler>,
    limits: Limits,
) {
    let read_timeout = limits.read_timeout;
    let Ok(local) = stream.local_addr() else {
        return;
    };
    let admit_handler = handler.clone();
    let admitted = tokio::task::spawn_blocking(move || admit_handler.admit(peer, local))
        .await
        .unwrap_or_else(|_| Err("the check panicked".to_string()));
    if let Err(why) = admitted {
        let response = handler.refused(peer, &why);
        finish(&mut stream, &response.encode(false)).await;
        return;
    }
    let mut buffer = Vec::with_capacity(1024);
    let request = tokio::time::timeout(read_timeout, async {
        let mut chunk = [0u8; 2048];
        loop {
            match parse(&buffer) {
                Ok(Some(request)) => return Ok(request),
                Err(status) => return Err(status),
                Ok(None) => {}
            }
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return Err(0),
                Ok(read) => buffer.extend_from_slice(&chunk[..read]),
            }
        }
    })
    .await
    .unwrap_or(Err(408));
    let (request, head_length) = match request {
        Ok(request) => request,
        Err(0) => return,
        Err(status) => {
            finish(
                &mut stream,
                &Response::text(status, "bad request").encode(false),
            )
            .await;
            return;
        }
    };
    let head_only = request.method == "HEAD";
    // The route runs on the blocking pool with the connection as a plain socket, so it can read a body or write a
    // live view at its own pace; the socket comes back for the answer and the drain.
    let first = buffer.split_off(head_length.min(buffer.len()));
    let Ok(plain) = stream.into_std() else {
        return;
    };
    if plain.set_nonblocking(false).is_err() {
        return;
    }
    let framing = framing(&request);
    let ran = tokio::task::spawn_blocking(move || {
        let mut conn = Conn::new(plain, first, framing, limits);
        let response = handler.handle(peer, request, &mut conn);
        (conn.into_stream(), response)
    })
    .await;
    let Ok((plain, response)) = ran else {
        return;
    };
    if plain.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(mut stream) = tokio::net::TcpStream::from_std(plain) else {
        return;
    };
    finish(&mut stream, &response.encode(head_only)).await;
}

/// Send the answer, close the sending half, then read and drop what the caller still sends until it closes, up to
/// `DRAIN_LIMIT` or `DRAIN_TIME`.
async fn finish(stream: &mut tokio::net::TcpStream, answer: &[u8]) {
    let _ = stream.write_all(answer).await;
    let _ = stream.shutdown().await;
    let _ = tokio::time::timeout(DRAIN_TIME, async {
        let mut chunk = [0u8; 16 * 1024];
        let mut left = DRAIN_LIMIT;
        while left > 0 {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => left = left.saturating_sub(read),
            }
        }
    })
    .await;
}
