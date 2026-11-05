//! herdr's socket API, as Nirvana Remote calls it.
//!
//! Nirvana Remote lives in the herdr server, so it hands its requests to the app on the same channel the socket
//! server does (`api::ApiRequestMessage`): the same methods, parameters and answers as `herdr agent list` gets,
//! without a pipe. Two methods live in the socket server itself, not the app (`src/api/server.rs`
//! `handle_connection_with_stop`): `events.subscribe` (a stream the server writes as herdr's event hub fills) and
//! `agent.wait`. For those Nirvana Remote dials this server's own socket (P39b), as `herdr` on the command line
//! does. Tests use a fake with canned answers.
//!
//! **Every socket read has an end** (review round, S3 and S4). A Windows named pipe has no read timeout
//! (`set_recv_timeout` is `Unsupported` there, which `api::client`'s `set_timeout_best_effort` swallows), so a
//! blocking read on a server that never answers would hold its blocking-pool thread, and the HTTP slot it serves,
//! for good. The socket is read the way `api::client`'s status probe reads it: peek, then read what is there, else
//! sleep `POLL` (`Polled`). `agent.wait` gives up at its own timeout plus `MARGIN`; a subscription's reader ends
//! once its `stop` is set, within one `POLL`, so the feed's old streams do not outlive it.

use std::io::{BufRead, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// A server that does not answer in this long is reported, not waited on.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(3);
/// `agent.prompt` answers once the text and its Enter are written to the pane (`queue_user_input_submission`), which
/// for a long message takes longer than a read.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(15);
/// A socket method's answer is waited on for its own `timeout_ms` (`agent.wait`'s) plus this.
pub const MARGIN: Duration = Duration::from_secs(2);
/// How often a socket with nothing to read is looked at again: a peek, not a request to herdr.
const POLL: Duration = Duration::from_millis(25);

/// The events of one `events.subscribe`, as the socket writes them, one JSON value each; the iterator ends when the
/// stream does.
pub type Events = Box<dyn Iterator<Item = Value> + Send>;

pub trait Herdr: Send + Sync {
    /// One socket API method; `Ok` holds the response's `result`.
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;

    /// `events.subscribe` with these `subscriptions`: its events, once the server said it started. The events end
    /// when the stream does, or soon after `stop` is set.
    fn subscribe(&self, _subscriptions: Value, _stop: Arc<AtomicBool>) -> Result<Events, String> {
        Err("events.subscribe is not served here".into())
    }

    /// A method the socket server answers itself (`agent.wait`), over the socket.
    fn call_socket(&self, method: &str, params: Value) -> Result<Value, String> {
        self.call(method, params)
    }
}

/// The request as the socket carries it: `{"id", "method", "params"}`.
pub fn request(
    id: u64,
    method: &str,
    params: Value,
) -> Result<crate::api::schema::Request, String> {
    serde_json::from_value(json!({
        "id": format!("nirvana-remote-{id}"),
        "method": method,
        "params": params,
    }))
    .map_err(|err| format!("{method}: {err}"))
}

/// A socket API answer read back: its `result`, or its error's message.
pub fn answer(text: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    if let Some(error) = value.get("error") {
        return Err(error["message"].as_str().unwrap_or("error").to_string());
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| "no result".to_string())
}

/// This PC's spaces and its agents at work: the PC list's own line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub spaces: usize,
    /// Agents working or waiting on the owner: the prototype's "agents at work" counts both.
    pub working: usize,
}

pub fn summary(herdr: &dyn Herdr) -> Result<Summary, String> {
    let workspaces = herdr.call("workspace.list", json!({}))?;
    let agents = herdr.call("agent.list", json!({}))?;
    let spaces = workspaces["workspaces"].as_array().map_or(0, Vec::len);
    let working = agents["agents"].as_array().map_or(0, |agents| {
        agents
            .iter()
            .filter(|agent| {
                matches!(
                    agent["agent_status"].as_str(),
                    Some("working") | Some("blocked")
                )
            })
            .count()
    });
    Ok(Summary { spaces, working })
}

/// The app behind the socket server.
pub struct App {
    pub api_tx: crate::api::ApiRequestSender,
    pub next: std::sync::atomic::AtomicU64,
}

impl Herdr for App {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let request = request(id, method, params)?;
        let (respond_to, answered) = std::sync::mpsc::channel();
        self.api_tx
            .send(crate::api::ApiRequestMessage {
                request,
                respond_to,
                response_write_complete: None,
            })
            .map_err(|_| "the herdr server is stopping".to_string())?;
        let timeout = if method == "agent.prompt" {
            PROMPT_TIMEOUT
        } else {
            ANSWER_TIMEOUT
        };
        let text = answered
            .recv_timeout(timeout)
            .map_err(|_| format!("{method}: no answer in {} s", timeout.as_secs()))?;
        answer(&text)
    }

    fn subscribe(&self, subscriptions: Value, stop: Arc<AtomicBool>) -> Result<Events, String> {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        subscribe_at(&crate::api::socket_path(), id, subscriptions, stop)
    }

    fn call_socket(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        call_socket_at(&crate::api::socket_path(), id, method, params)
    }
}

/// How long a socket method's answer is waited on: its own `timeout_ms` plus `MARGIN`, else `ANSWER_TIMEOUT`.
pub fn socket_wait(params: &Value) -> Duration {
    params["timeout_ms"]
        .as_u64()
        .map_or(ANSWER_TIMEOUT, |ms| Duration::from_millis(ms) + MARGIN)
}

/// `call_socket` on the socket at `path` (`api::socket_path`: the same session as this server, so the same pipe
/// its socket server listens on). Its answer, or an error once `socket_wait` has passed.
pub fn call_socket_at(path: &Path, id: u64, method: &str, params: Value) -> Result<Value, String> {
    let wait = socket_wait(&params);
    let mut reader = socket_request(path, id, method, params, None)?;
    reader.get_mut().deadline = Some(Instant::now() + wait);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(read) if read > 0 => answer(&line),
        Ok(_) => Err(format!("{method}: no answer")),
        Err(err) => Err(format!(
            "{method}: no answer in {} s: {err}",
            wait.as_secs()
        )),
    }
}

/// `subscribe` on the socket at `path`.
pub fn subscribe_at(
    path: &Path,
    id: u64,
    subscriptions: Value,
    stop: Arc<AtomicBool>,
) -> Result<Events, String> {
    let mut reader = socket_request(
        path,
        id,
        "events.subscribe",
        json!({ "subscriptions": subscriptions }),
        Some(stop),
    )?;
    // The first line says whether it started; each line after it is one event, for as long as they come.
    reader.get_mut().deadline = Some(Instant::now() + ANSWER_TIMEOUT);
    let mut first = String::new();
    match reader.read_line(&mut first) {
        Ok(read) if read > 0 => {}
        _ => return Err("events.subscribe: no answer".into()),
    }
    answer(&first)?;
    reader.get_mut().deadline = None;
    Ok(Box::new(
        reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok()),
    ))
}

/// One request on the socket at `path`; its answer is read through `Polled`.
fn socket_request(
    path: &Path,
    id: u64,
    method: &str,
    params: Value,
    stop: Option<Arc<AtomicBool>>,
) -> Result<std::io::BufReader<Polled>, String> {
    let line =
        serde_json::to_string(&request(id, method, params)?).map_err(|err| err.to_string())?;
    let mut stream = crate::ipc::connect_local_stream(path)
        .map_err(|err| format!("{method}: herdr's socket: {err}"))?;
    stream
        .write_all(format!("{line}\n").as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|err| format!("{method}: {err}"))?;
    crate::ipc::set_local_stream_polling(&mut stream, true)
        .map_err(|err| format!("{method}: {err}"))?;
    Ok(std::io::BufReader::new(Polled {
        stream,
        deadline: None,
        stop,
    }))
}

/// A socket read that cannot block for good: past `deadline` it is an error, and once `stop` is set it is the end
/// of the stream.
pub struct Polled {
    stream: crate::ipc::LocalStream,
    deadline: Option<Instant>,
    stop: Option<Arc<AtomicBool>>,
}

impl Read for Polled {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            if self
                .stop
                .as_ref()
                .is_some_and(|stop| stop.load(Ordering::SeqCst))
            {
                return Ok(0);
            }
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "herdr's socket did not answer",
                ));
            }
            match crate::ipc::poll_local_stream_read_count(&mut self.stream, buffer)? {
                crate::ipc::LocalStreamReadCount::Data(count) => return Ok(count),
                crate::ipc::LocalStreamReadCount::Closed => return Ok(0),
                crate::ipc::LocalStreamReadCount::Pending => std::thread::sleep(POLL),
            }
        }
    }
}
