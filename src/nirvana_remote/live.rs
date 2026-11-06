//! Plan 39 P39b's routes: the spaces, an agent's chat, sending to it, and uploads.
//!
//! Every one of them runs after `Site::route`'s checks (the Tailscale service's connection, the owner's login,
//! another device, this PC's Host, and for a POST this page's `Sec-Fetch-Site`/`Origin`), so a caller refused there
//! gets no list, no transcript, no send and no file. An agent is named by its herdr pane id only, and must be one
//! of this herdr's agents in one of its spaces (`found`); every path is chosen here from herdr's answer for it.
//!
//! - `GET /api/spaces`: the list (`spaces::build`). `GET /api/events`: the same, live (`spaces::Feed`).
//! - `GET /api/chat?agent=<id>`: the agent, its bubbles and its live screen. `GET /api/chat/events?agent=<id>`:
//!   the same, then what changes: the agent's state from the feed, new or changed bubbles when the transcript grows
//!   (its length and time read once each `stream::WAKE`; a file, not herdr), and the screen again on a state change.
//! - `POST /api/send`: `{"agent", "text", "files", "mode"}` (`send.rs`).
//! - `POST /api/upload?agent=<id>`: the body is the file; its type names the extension.

use std::cell::Cell;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::chat;
use super::http::{Conn, Framing, Request, Response};
use super::routes::{Site, CSP};
use super::send;
use super::spaces;
use super::stream;

/// One of this herdr's agents, with its space's name, and herdr's other agents (whose folders it may share).
pub struct Found {
    pub agent: Value,
    pub space: String,
    pub others: Vec<Value>,
}

impl Found {
    pub fn id(&self) -> &str {
        self.agent["pane_id"].as_str().unwrap_or("")
    }

    /// The row the page draws, with the space's name.
    fn row(&self) -> Value {
        let mut row = spaces::agent_row(&self.agent);
        row["space"] = json!(self.space);
        row
    }
}

/// `key`'s value in a query string, percent-decoded.
pub fn query_value(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (name == key).then(|| decode(value))?
    })
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Each bubble's text by its id: what a view has sent.
fn keyed(messages: &[Value]) -> HashMap<String, String> {
    messages
        .iter()
        .map(|bubble| {
            (
                bubble["id"].as_str().unwrap_or("").to_string(),
                bubble.to_string(),
            )
        })
        .collect()
}

fn error(status: u16, why: impl Into<String>) -> Response {
    Response::json(status, &json!({ "error": why.into() }))
}

fn event_stream_head() -> Response {
    Response::new(200, "text/event-stream; charset=utf-8", "").with("Content-Security-Policy", CSP)
}

#[derive(Deserialize)]
struct SendBody {
    agent: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    mode: Option<String>,
}

impl Site {
    /// The agent with pane id `id`, when it is one of this herdr's agents in one of its spaces.
    pub fn found(&self, id: &str) -> Result<Found, Response> {
        let workspaces = self
            .herdr
            .call("workspace.list", json!({}))
            .map_err(|why| error(502, why))?;
        let agents = self
            .herdr
            .call("agent.list", json!({}))
            .map_err(|why| error(502, why))?;
        let space = |workspace: &Value| {
            workspaces["workspaces"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .find(|space| space["workspace_id"] == *workspace)
                .map(|space| space["label"].as_str().unwrap_or("").to_string())
        };
        let all = agents["agents"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        all.iter()
            .filter(|agent| !id.is_empty() && agent["pane_id"] == id)
            .find_map(|agent| {
                Some(Found {
                    agent: agent.clone(),
                    space: space(&agent["workspace_id"])?,
                    others: all
                        .iter()
                        .filter(|other| other["pane_id"] != id)
                        .cloned()
                        .collect(),
                })
            })
            .ok_or_else(|| error(404, "no such agent in this PC's spaces"))
    }

    /// Who the checks let in now: the owner's login and this PC, as the supervisor last read them from Tailscale
    /// (review round, S6). A live view takes it when it opens and ends at its next `stream::WAKE` once it differs:
    /// the owner signed this PC in to another account, or Tailscale stopped saying who the owner is (both are
    /// `None` then, and every new request is refused too).
    fn caller(&self) -> (Option<String>, Option<super::routes::Me>) {
        (
            self.owner.read().ok().and_then(|owner| owner.clone()),
            self.me.read().ok().and_then(|me| me.clone()),
        )
    }

    fn found_in(&self, request: &Request) -> Result<Found, Response> {
        let id = query_value(&request.query, "agent").ok_or_else(|| error(400, "which agent?"))?;
        self.found(&id)
    }

    fn transcript(&self, found: &Found) -> Option<PathBuf> {
        chat::transcript_for(
            &found.agent,
            &found.others,
            self.claude_dir.as_deref(),
            self.nirvana_home.as_deref(),
        )
    }

    /// The bottom of the agent's screen, from `agent.read`; empty when it cannot be read.
    fn live(&self, id: &str) -> String {
        self.herdr
            .call(
                "agent.read",
                json!({"target": id, "source": "recent", "lines": 40, "format": "text", "strip_ansi": true}),
            )
            .map(|read| chat::live(&read))
            .unwrap_or_default()
    }

    fn chat_payload(&self, found: &Found, messages: &[Value], transcript: bool) -> Value {
        json!({
            "agent": found.row(),
            "messages": messages,
            "transcript": transcript,
            "live": self.live(found.id()),
        })
    }

    pub(super) fn spaces(&self) -> Response {
        match spaces::snapshot(self.herdr.as_ref()) {
            Ok(list) => Response::json(200, &list),
            Err(why) => error(502, why),
        }
    }

    pub(super) fn events(&self, request: &Request, conn: &mut Conn) -> Response {
        if request.method == "HEAD" {
            return Response::new(200, "text/event-stream; charset=utf-8", "");
        }
        let lists = match self.feed.listen() {
            Ok(lists) => lists,
            Err(why) => return error(502, why),
        };
        let ticket = self.streams.open();
        if stream::begin(conn, event_stream_head()).is_err() {
            return Response::sent();
        }
        let caller = self.caller();
        stream::run(
            conn,
            &ticket,
            || self.caller() == caller,
            |wait| match lists.recv_timeout(wait) {
                Ok(list) => Some(vec![stream::event("spaces", &list)]),
                Err(RecvTimeoutError::Timeout) => Some(Vec::new()),
                Err(RecvTimeoutError::Disconnected) => None,
            },
        );
        Response::sent()
    }

    pub(super) fn chat(&self, request: &Request) -> Response {
        let found = match self.found_in(request) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        let path = self.transcript(&found);
        let messages = chat::read(path.as_deref());
        Response::json(200, &self.chat_payload(&found, &messages, path.is_some()))
    }

    pub(super) fn chat_events(&self, request: &Request, conn: &mut Conn) -> Response {
        let found = match self.found_in(request) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        if request.method == "HEAD" {
            return Response::new(200, "text/event-stream; charset=utf-8", "");
        }
        let lists = match self.feed.listen() {
            Ok(lists) => lists,
            Err(why) => return error(502, why),
        };
        let ticket = self.streams.open();
        if stream::begin(conn, event_stream_head()).is_err() {
            return Response::sent();
        }
        let id = found.id().to_string();
        let mut path = self.transcript(&found);
        let mut stamp = chat::stamp(path.as_deref());
        let messages = chat::read(path.as_deref());
        let mut sent = keyed(&messages);
        let first = self.chat_payload(&found, &messages, path.is_some());
        let mut row = first["agent"].clone();
        if conn
            .write(stream::event("chat", &first.to_string()).as_bytes())
            .is_err()
        {
            return Response::sent();
        }
        let mut gone = false;
        let caller = self.caller();
        stream::run(
            conn,
            &ticket,
            || self.caller() == caller,
            |wait| {
                if gone {
                    return None;
                }
                let mut parts = Vec::new();
                match lists.recv_timeout(wait) {
                    Ok(list) => {
                        let list: Value = serde_json::from_str(&list).unwrap_or_default();
                        let now = list["spaces"]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_default()
                            .iter()
                            .flat_map(|space| {
                                space["agents"].as_array().cloned().unwrap_or_default()
                            })
                            .find(|agent| agent["id"] == id.as_str());
                        match now {
                            None => {
                                gone = true;
                                return Some(vec![stream::event("gone", "{}")]);
                            }
                            Some(now) => {
                                let mut now = now;
                                now["space"] = row["space"].clone();
                                if now != row {
                                    row = now;
                                    // Its transcript may have changed with it: a Claude's token comes with its
                                    // first prompt and outranks the folder's record, and a second Claude in the
                                    // folder takes the record away (chat.rs, "Whose record"). Then the whole chat
                                    // again, from the new file.
                                    let found = self.found(&id).ok();
                                    let fresh =
                                        found.as_ref().and_then(|found| self.transcript(found));
                                    match found {
                                        Some(found) if fresh != path => {
                                            path = fresh;
                                            stamp = chat::stamp(path.as_deref());
                                            let messages = chat::read(path.as_deref());
                                            sent = keyed(&messages);
                                            let whole = self.chat_payload(
                                                &found,
                                                &messages,
                                                path.is_some(),
                                            );
                                            row = whole["agent"].clone();
                                            parts.push(stream::event("chat", &whole.to_string()));
                                        }
                                        _ => {
                                            parts.push(stream::event("agent", &row.to_string()));
                                            parts.push(stream::event(
                                                "live",
                                                &json!({ "live": self.live(&id) }).to_string(),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return None,
                }
                let fresh = chat::stamp(path.as_deref());
                if fresh != stamp {
                    stamp = fresh;
                    let changed: Vec<Value> = chat::read(path.as_deref())
                        .into_iter()
                        .filter(|bubble| {
                            let key = bubble["id"].as_str().unwrap_or("").to_string();
                            let text = bubble.to_string();
                            if sent.get(&key) == Some(&text) {
                                return false;
                            }
                            sent.insert(key, text);
                            true
                        })
                        .collect();
                    if !changed.is_empty() {
                        parts.push(stream::event(
                            "messages",
                            &json!({ "messages": changed }).to_string(),
                        ));
                    }
                }
                Some(parts)
            },
        );
        Response::sent()
    }

    pub(super) fn send(&self, request: &Request, conn: &mut Conn) -> Response {
        let json_body = request.header("Content-Type").first().is_some_and(|kind| {
            kind.trim()
                .to_ascii_lowercase()
                .starts_with("application/json")
        });
        if !json_body {
            return error(415, "a send is JSON");
        }
        if conn.framing() == Framing::None {
            return error(411, "a send has a length");
        }
        let mut body = Vec::new();
        if let Err(why) = conn.read_body(send::SEND_MOST, &mut body) {
            return error(why.status(), "the send could not be read");
        }
        let Ok(body) = serde_json::from_slice::<SendBody>(&body) else {
            return error(400, "a send is {agent, text, files, mode}");
        };
        let found = match self.found(&body.agent) {
            Ok(found) => found,
            Err(refused) => return refused,
        };
        let mut files = Vec::new();
        if !body.files.is_empty() {
            let dir = match send::folder_of(&found.agent) {
                Ok(folder) => folder.join(crate::nirvana_run::DIR).join("uploads"),
                Err(why) => return error(409, why),
            };
            for name in &body.files {
                let path = dir.join(name);
                if !send::is_upload_name(name) || !path.is_file() {
                    return error(400, "a file that is not one of this space's uploads");
                }
                files.push(path);
            }
        }
        let message = send::message(&body.text, &files);
        if message.is_empty() {
            return error(400, "nothing to send");
        }
        let id = found.id().to_string();
        let mut stopped = None;
        match found.agent["agent_status"].as_str().unwrap_or("") {
            "blocked" => {
                return error(
                    409,
                    "the agent needs you on the PC: a question or a permission is on its screen",
                )
            }
            "working" => match body.mode.as_deref() {
                None => {
                    return Response::json(
                        409,
                        &json!({"choose": true, "error": "the agent is working: queue it, or send it now"}),
                    )
                }
                Some("queue") => {}
                Some("now") => {
                    if let Err(why) = self
                        .herdr
                        .call("agent.send_keys", json!({"target": id, "keys": ["esc"]}))
                    {
                        return error(502, why);
                    }
                    stopped = Some(
                        self.herdr
                            .call_socket(
                                "agent.wait",
                                json!({
                                    "target": id,
                                    "until": ["idle", "done", "blocked"],
                                    "timeout_ms": send::STOP_WAIT.as_millis() as u64,
                                }),
                            )
                            .is_ok(),
                    );
                }
                Some(_) => return error(400, "mode is queue or now"),
            },
            _ => {}
        }
        match self
            .herdr
            .call("agent.prompt", json!({"target": id, "text": message}))
        {
            Ok(_) => Response::json(
                200,
                &json!({"sent": true, "mode": body.mode, "stopped": stopped}),
            ),
            Err(why) => error(502, why),
        }
    }

    /// `POST /api/upload`. Every answer but a saved file logs one line (P39u: on 2026-11-06 two phone uploads failed
    /// and herdr-server.log said nothing): the route, the size the head declared, the bytes read, the time since the
    /// head, the status and why; never a byte of the file. The lines go through the refusals' throttle, one per kind
    /// of failure a minute. A saved file logs its size and time at info, which is the phone's uplink, measured. The
    /// answer to a failure carries the same figures, so the page can say on the chip how far it got.
    pub(super) fn upload(&self, request: &Request, conn: &mut Conn) -> Response {
        let size = match conn.framing() {
            Framing::Length(length) => length.to_string(),
            Framing::Chunked => "chunked".to_string(),
            Framing::None => "none".to_string(),
            Framing::Bad => "bad framing".to_string(),
        };
        let read = Cell::new(0u64);
        let taken = self.take_upload(request, conn, &read);
        let elapsed = conn.elapsed();
        match taken {
            Ok(answer) => {
                info!(
                    route = "/api/upload",
                    size = %size,
                    read = read.get(),
                    elapsed_ms = elapsed.as_millis() as u64,
                    "nirvana remote upload saved"
                );
                Response::json(200, &answer)
            }
            Err((status, why)) => {
                if let Some(held_back) = self
                    .throttle
                    .allow(&format!("upload {status} {why}"), Instant::now())
                {
                    warn!(
                        route = "/api/upload",
                        size = %size,
                        read = read.get(),
                        elapsed_ms = elapsed.as_millis() as u64,
                        status,
                        why = %why,
                        held_back,
                        "nirvana remote upload failed"
                    );
                }
                Response::json(
                    status,
                    &json!({
                        "error": why,
                        "read": read.get(),
                        "seconds": elapsed.as_secs(),
                    }),
                )
            }
        }
    }

    /// The upload itself: the answer to send, or the status and why; `read` counts the body's bytes as they come.
    fn take_upload(
        &self,
        request: &Request,
        conn: &mut Conn,
        read: &Cell<u64>,
    ) -> Result<Value, (u16, String)> {
        match conn.framing() {
            Framing::None => return Err((411, "an upload has a length".into())),
            Framing::Bad => return Err((400, "a body framing this server does not read".into())),
            // Refused on the head: not a byte of the body is read.
            Framing::Length(length) if length > send::UPLOAD_MOST => {
                return Err((413, "over 50 MB".into()))
            }
            _ => {}
        }
        let found = self.found_in(request).map_err(|refused| {
            let why = serde_json::from_slice::<Value>(&refused.body)
                .ok()
                .and_then(|body| body["error"].as_str().map(str::to_string))
                .unwrap_or_default();
            (refused.status, why)
        })?;
        let dir = send::folder_of(&found.agent)
            .and_then(|folder| send::uploads_dir(&folder))
            .map_err(|why| (409, why))?;
        let kind = request
            .header("Content-Type")
            .first()
            .copied()
            .unwrap_or("");
        let name = send::name_for(kind, (self.now)());
        let (path, size) = send::save(&dir, &name, |sink| {
            conn.read_body(send::UPLOAD_MOST, &mut Counted { sink, read })
        })?;
        Ok(json!({"file": name, "path": path.display().to_string(), "size": size}))
    }
}

/// A sink that counts what goes through it: the bytes of the body read so far, for the log line.
struct Counted<'a> {
    sink: &'a mut dyn Write,
    read: &'a Cell<u64>,
}

impl Write for Counted<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let wrote = self.sink.write(bytes)?;
        self.read.set(self.read.get() + wrote as u64);
        Ok(wrote)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}
