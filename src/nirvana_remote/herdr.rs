//! herdr's socket API, as Nirvana Remote calls it.
//!
//! Nirvana Remote lives in the herdr server, so it hands its requests to the app on the same channel the socket
//! server does (`api::ApiRequestMessage`): the same methods, parameters and answers as `herdr agent list` gets,
//! without a pipe. Tests use a fake with canned answers.

use std::time::Duration;

use serde_json::{json, Value};

/// A server that does not answer in this long is reported, not waited on.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(3);

pub trait Herdr: Send + Sync {
    /// One socket API method; `Ok` holds the response's `result`.
    fn call(&self, method: &str, params: Value) -> Result<Value, String>;
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
        let text = answered
            .recv_timeout(ANSWER_TIMEOUT)
            .map_err(|_| format!("{method}: no answer in {} s", ANSWER_TIMEOUT.as_secs()))?;
        answer(&text)
    }
}
