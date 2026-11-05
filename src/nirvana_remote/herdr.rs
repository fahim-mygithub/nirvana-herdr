//! herdr's socket API, as Nirvana Remote calls it.
//!
//! Nirvana Remote lives in the herdr server, so it hands its requests to the app on the same channel the socket
//! server does (`api::ApiRequestMessage`): the same methods, parameters and answers as `herdr agent list` gets,
//! without a pipe. Two methods live in the socket server itself, not the app (`src/api/server.rs`
//! `handle_connection_with_stop`): `events.subscribe` (a stream the server writes as herdr's event hub fills) and
//! `agent.wait`. For those Nirvana Remote dials this server's own socket (P39b), as `herdr` on the command line
//! does. Tests use a fake with canned answers.

use std::time::Duration;

use serde_json::{json, Value};

/// A server that does not answer in this long is reported, not waited on.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(3);
/// `agent.prompt` answers once the text and its Enter are written to the pane (`queue_user_input_submission`), which
/// for a long message takes longer than a read.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(15);

/// The events of one `events.subscribe`, as the socket writes them, one JSON value each; the iterator ends when the
/// stream does.
pub type Events = Box<dyn Iterator<Item = Value> + Send>;

pub trait Herdr: Send + Sync {
    /// One socket API method; `Ok` holds the response's `result`.
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;

    /// `events.subscribe` with these `subscriptions`: its events, once the server said it started.
    fn subscribe(&self, _subscriptions: Value) -> Result<Events, String> {
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

    fn subscribe(&self, subscriptions: Value) -> Result<Events, String> {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut lines = socket_request(
            id,
            "events.subscribe",
            json!({ "subscriptions": subscriptions }),
        )?;
        // The first line says whether it started; each line after it is one event.
        let first = lines.next().ok_or("events.subscribe: no answer")?;
        answer(&first)?;
        Ok(Box::new(
            lines.filter_map(|line| serde_json::from_str(&line).ok()),
        ))
    }

    fn call_socket(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut lines = socket_request(id, method, params)?;
        answer(&lines.next().ok_or_else(|| format!("{method}: no answer"))?)
    }
}

/// One request on this server's own API socket (`api::socket_path`, the same session as this server, so the same
/// pipe the socket server listens on); the answer's lines, read as they come.
fn socket_request(
    id: u64,
    method: &str,
    params: Value,
) -> Result<Box<dyn Iterator<Item = String> + Send>, String> {
    use std::io::{BufRead, Write};
    let line =
        serde_json::to_string(&request(id, method, params)?).map_err(|err| err.to_string())?;
    let mut stream = crate::ipc::connect_local_stream(&crate::api::socket_path())
        .map_err(|err| format!("{method}: herdr's socket: {err}"))?;
    stream
        .write_all(format!("{line}\n").as_bytes())
        .map_err(|err| format!("{method}: {err}"))?;
    let reader = std::io::BufReader::new(stream);
    Ok(Box::new(reader.lines().map_while(Result::ok)))
}
