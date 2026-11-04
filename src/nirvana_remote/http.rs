//! The smallest HTTP/1.1 server Nirvana Remote needs, on the tokio the herdr server already runs.
//!
//! Its only caller is tailscaled's `serve` proxy on this PC, one request per connection: a request head parsed by
//! `httparse` (hyper's own parser), an answer with its length, and the connection closed. No body is read yet:
//! P39b's upload route adds one, bounded by its `Content-Length` (plan 39 §2.3, 50 MB).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A request head larger than this is refused (431) rather than buffered.
pub const HEAD_LIMIT: usize = 16 * 1024;
/// A caller that has not sent its whole head in this long is dropped.
pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
/// At most this many connections are served at once; the rest wait in the accept queue.
pub const CONNECTIONS: usize = 32;
const HEADERS: usize = 48;

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

    pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    /// The bytes on the wire; a HEAD answer keeps the length and drops the body.
    pub fn encode(&self, head_only: bool) -> Vec<u8> {
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
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "Status",
    }
}

/// A head read so far: `Ok(None)` while it is incomplete, the status to refuse it with when it cannot be one.
pub fn parse_head(buffer: &[u8]) -> Result<Option<Request>, u16> {
    let mut headers = [httparse::EMPTY_HEADER; HEADERS];
    let mut parsed = httparse::Request::new(&mut headers);
    match parsed.parse(buffer) {
        Ok(httparse::Status::Complete(_)) => {}
        Ok(httparse::Status::Partial) if buffer.len() >= HEAD_LIMIT => return Err(431),
        Ok(httparse::Status::Partial) => return Ok(None),
        Err(httparse::Error::TooManyHeaders) => return Err(431),
        Err(_) => return Err(400),
    }
    let target = parsed.path.unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut out = Vec::with_capacity(parsed.headers.len());
    for header in parsed.headers.iter() {
        // A header that is not text is not one this server reads; refusing it keeps the identity check exact.
        let value = std::str::from_utf8(header.value).map_err(|_| 400u16)?;
        out.push((header.name.to_string(), value.to_string()));
    }
    Ok(Some(Request {
        method: parsed.method.unwrap_or("").to_string(),
        path: path.to_string(),
        query: query.to_string(),
        headers: out,
    }))
}

pub trait Handler: Send + Sync + 'static {
    /// Whether the connection may send a request at all; asked before a byte of it is read.
    fn admit(&self, peer: SocketAddr, local: SocketAddr) -> Result<(), String>;
    /// The answer to a refused connection.
    fn refused(&self, peer: SocketAddr, why: &str) -> Response;
    /// The answer to an admitted request. It may block: it runs on tokio's blocking pool.
    fn handle(&self, peer: SocketAddr, request: Request) -> Response;
}

/// Serve until `stop` turns true.
pub async fn serve(
    listener: tokio::net::TcpListener,
    handler: Arc<dyn Handler>,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let slots = Arc::new(tokio::sync::Semaphore::new(CONNECTIONS));
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
            connection(stream, peer, handler).await;
            drop(permit);
        });
    }
}

async fn connection(
    mut stream: tokio::net::TcpStream,
    peer: SocketAddr,
    handler: Arc<dyn Handler>,
) {
    let Ok(local) = stream.local_addr() else {
        return;
    };
    let admit_handler = handler.clone();
    let admitted = tokio::task::spawn_blocking(move || admit_handler.admit(peer, local))
        .await
        .unwrap_or_else(|_| Err("the check panicked".to_string()));
    if let Err(why) = admitted {
        let response = handler.refused(peer, &why);
        let _ = stream.write_all(&response.encode(false)).await;
        let _ = stream.shutdown().await;
        return;
    }
    let mut buffer = Vec::with_capacity(1024);
    let request = tokio::time::timeout(READ_TIMEOUT, async {
        let mut chunk = [0u8; 2048];
        loop {
            match parse_head(&buffer) {
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
    let request = match request {
        Ok(request) => request,
        Err(0) => return,
        Err(status) => {
            let _ = stream
                .write_all(&Response::text(status, "bad request").encode(false))
                .await;
            let _ = stream.shutdown().await;
            return;
        }
    };
    let head_only = request.method == "HEAD";
    let response = tokio::task::spawn_blocking(move || handler.handle(peer, request))
        .await
        .unwrap_or_else(|_| Response::text(500, "the handler panicked"));
    let _ = stream.write_all(&response.encode(head_only)).await;
    let _ = stream.shutdown().await;
}
