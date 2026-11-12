//! Plan 39 P39b, §2.1: every space with its agents, live (prototype screen 2).
//!
//! The list is herdr's own `workspace.list` and `agent.list`, cut to what the phone draws. It is kept live by one
//! `events.subscribe` per Nirvana Remote, not by asking herdr on a timer: the space events (created, closed,
//! renamed, reordered), the pane events that change who the agents are (created, closed, exited, agent detected),
//! and `pane.agent_status_changed` for each agent's pane. That last one names one pane (`Subscription` in
//! `src/api/schema/events.rs` has no all-panes form), so when the agents change the feed subscribes again with the
//! new panes. Each event (a burst counts as one, `SETTLE`) rebuilds the list, and a list that changed goes to every
//! page that listens (`listen`), one channel each.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tracing::warn;

use super::herdr::Herdr;

/// Events closer together than this are one change: an agent going from working to done moves several at once.
pub const SETTLE: Duration = Duration::from_millis(150);
/// A burst is cut here, so a stream that never pauses still shows its changes.
const SETTLE_MOST: Duration = Duration::from_secs(1);
/// Before each subscription after the first the feed waits this, doubling to `RETRY_MOST` (`Feed::watch`).
const RETRY_FIRST: Duration = Duration::from_millis(500);
const RETRY_MOST: Duration = Duration::from_secs(10);
/// Alpha 0.70.0: a background-wait reading changes with no herdr event (`crate::nirvana_bgwait`: running turns quiet,
/// a task is lost, the server's worker finishes its first read a moment after the event that asked for it). So the
/// feed builds the list once more this long after each event, longer than the reader's `REFRESH`, and every
/// `BACKGROUND_EVERY` while any agent shows a reading. Neither runs while nothing waits.
pub const FOLLOW_UP: Duration = Duration::from_secs(4);
pub const BACKGROUND_EVERY: Duration = Duration::from_secs(15);

/// herdr's status as the phone says it (the prototype's chips): working, needs you, done, idle.
pub fn status_word(status: &str) -> &'static str {
    match status {
        "working" => "working",
        "blocked" => "needs",
        "done" => "done",
        _ => "idle",
    }
}

/// What the space shows when its agents differ: one that needs you first, then one at work, then one done.
fn space_status(agents: &[Value]) -> &'static str {
    let has = |word: &str| agents.iter().any(|agent| agent["status"] == word);
    ["needs", "working", "done"]
        .into_iter()
        .find(|word| has(word))
        .unwrap_or("idle")
}

fn token<'a>(agent: &'a Value, name: &str) -> Option<&'a str> {
    agent["tokens"][name]
        .as_str()
        .filter(|value| !value.is_empty())
}

/// One agent as the phone draws it. `id` is herdr's public pane id: the only handle the page ever sends back.
pub fn agent_row(agent: &Value) -> Value {
    agent_row_at(agent, crate::nirvana_bgwait::now_secs())
}

/// The background-wait reading in herdr's row (`crate::nirvana_bgwait::TOKEN`).
pub fn background(agent: &Value) -> Option<crate::nirvana_bgwait::View> {
    agent["tokens"][crate::nirvana_bgwait::TOKEN]
        .as_str()
        .and_then(crate::nirvana_bgwait::View::from_token)
}

/// `agent_row` at `now`. Alpha 0.70.0: an agent whose turn ended and that waits on background work reads `bg`, with
/// its reading in words under `bg` (`{"state": "waiting" | "quiet" | "lost", "line"}`), not `working`; one whose tasks
/// are all lost reads herdr's state, except that a Working from the screen reads idle. A blocked agent is the owner's
/// to answer and keeps `needs`. Display only: the routes that act on an agent read herdr's `agent_status`.
pub fn agent_row_at(agent: &Value, now: u64) -> Value {
    let name = agent["name"]
        .as_str()
        .or_else(|| agent["display_agent"].as_str())
        .or_else(|| agent["agent"].as_str())
        .unwrap_or("agent");
    let mut row = json!({
        "id": agent["pane_id"],
        "name": name,
        "model": token(agent, crate::nirvana_spaces::MODEL_TOKEN).unwrap_or(""),
        "kind": token(agent, crate::nirvana_spaces::KIND_TOKEN).unwrap_or(""),
        "line": crate::nirvana_spaces::task_title(agent["title"].as_str()),
        "status": status_word(agent["agent_status"].as_str().unwrap_or("")),
    });
    let herdr_status = agent["agent_status"].as_str().unwrap_or("");
    if let Some(view) = background(agent).filter(|_| herdr_status != "blocked") {
        let shown = view.shown();
        row["status"] = json!(match shown {
            crate::nirvana_bgwait::Shown::Waiting | crate::nirvana_bgwait::Shown::Quiet => "bg",
            crate::nirvana_bgwait::Shown::Lost if herdr_status == "working" => "idle",
            crate::nirvana_bgwait::Shown::Lost => status_word(herdr_status),
        });
        row["bg"] = json!({ "state": shown.phone(), "line": view.sentence(now) });
    }
    row
}

/// Whether any agent in a list from `build` shows a background-wait reading.
pub fn any_background(snapshot: &Value) -> bool {
    snapshot["spaces"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|space| space["agents"].as_array().cloned().unwrap_or_default())
        .any(|agent| agent.get("bg").is_some())
}

/// The list the phone draws, from `workspace.list` and `agent.list` answers. An agent whose space is not in this
/// herdr's list is left out.
pub fn build(workspaces: &Value, agents: &Value) -> Value {
    let agents = agents["agents"].as_array().cloned().unwrap_or_default();
    let spaces: Vec<Value> = workspaces["workspaces"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|space| {
            let rows: Vec<Value> = agents
                .iter()
                .filter(|agent| agent["workspace_id"] == space["workspace_id"])
                .map(agent_row)
                .collect();
            json!({
                "id": space["workspace_id"],
                "name": space["label"],
                "status": space_status(&rows),
                "agents": rows,
            })
        })
        .collect();
    json!({ "version": crate::nirvana::VERSION, "spaces": spaces })
}

pub fn snapshot(herdr: &dyn Herdr) -> Result<Value, String> {
    let workspaces = herdr.call("workspace.list", json!({}))?;
    let agents = herdr.call("agent.list", json!({}))?;
    Ok(build(&workspaces, &agents))
}

/// The agents' panes in a list from `build`.
pub fn panes(snapshot: &Value) -> BTreeSet<String> {
    snapshot["spaces"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .flat_map(|space| space["agents"].as_array().cloned().unwrap_or_default())
        .filter_map(|agent| agent["id"].as_str().map(str::to_string))
        .collect()
}

/// What the feed subscribes to for these agent panes.
pub fn subscriptions(panes: &BTreeSet<String>) -> Value {
    let mut list: Vec<Value> = [
        "workspace.created",
        "workspace.closed",
        "workspace.renamed",
        "workspace.reordered",
        "pane.created",
        "pane.closed",
        "pane.exited",
        "pane.agent_detected",
    ]
    .into_iter()
    .map(|kind| json!({ "type": kind }))
    .collect();
    list.extend(
        panes
            .iter()
            .map(|pane| json!({ "type": "pane.agent_status_changed", "pane_id": pane })),
    );
    Value::Array(list)
}

/// How long the feed waits for an event before it builds the list anyway (alpha 0.70.0): `FOLLOW_UP` after an event,
/// so a reading the event's own build asked for shows; `BACKGROUND_EVERY` while a reading shows; else only on events.
pub fn next_build(after_event: bool, background: bool) -> Option<Duration> {
    if after_event {
        Some(FOLLOW_UP)
    } else if background {
        Some(BACKGROUND_EVERY)
    } else {
        None
    }
}

/// Sets its flag when dropped: the subscription it guards is told to end.
struct Stop(Arc<AtomicBool>);

impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct State {
    listeners: Vec<Sender<String>>,
    last: Option<String>,
    watching: bool,
}

/// The one subscription to herdr, shared by every page that listens.
pub struct Feed {
    herdr: Arc<dyn Herdr>,
    state: Mutex<State>,
}

impl Feed {
    pub fn new(herdr: Arc<dyn Herdr>) -> Arc<Self> {
        Arc::new(Self {
            herdr,
            state: Mutex::new(State::default()),
        })
    }

    /// A channel of lists, the current one first; the feed starts watching herdr when it is not already.
    pub fn listen(self: &Arc<Self>) -> Result<Receiver<String>, String> {
        let now = snapshot(self.herdr.as_ref())?.to_string();
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(now.clone());
        let start = {
            let mut state = self.state.lock().map_err(|_| "the feed is broken")?;
            state.listeners.push(tx);
            state.last = Some(now);
            !std::mem::replace(&mut state.watching, true)
        };
        if start {
            let feed = self.clone();
            if let Err(err) = std::thread::Builder::new()
                .name("nirvana-remote-feed".into())
                .spawn(move || feed.watch())
            {
                if let Ok(mut state) = self.state.lock() {
                    state.watching = false;
                }
                return Err(format!("the feed could not start: {err}"));
            }
        }
        Ok(rx)
    }

    /// How many pages listen now.
    #[cfg(test)]
    pub fn listening(&self) -> usize {
        self.state.lock().map_or(0, |state| state.listeners.len())
    }

    /// Send `list` to every listener when it differs from the last one; drop listeners that went away. False once
    /// nobody listens, which also marks the feed stopped (under the same lock `listen` takes, so a new listener
    /// either sees it stopped and starts it again, or is seen here).
    fn send(&self, list: Option<String>) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if let Some(list) = list {
            if state.last.as_deref() != Some(list.as_str()) {
                state
                    .listeners
                    .retain(|listener| listener.send(list.clone()).is_ok());
                state.last = Some(list);
            }
        }
        if state.listeners.is_empty() {
            state.watching = false;
            return false;
        }
        true
    }

    /// The feed's loop (review round, S3): snapshot, subscribe, snapshot again (a change that came between the two
    /// is seen now, not at the next event), then rebuild on each burst of events. Before every subscription after
    /// the first it waits `retry`, which doubles each time and goes back to `RETRY_FIRST` only once a stream has
    /// carried an event: a server that acks and closes at once, or keeps failing, is asked at most every
    /// `RETRY_MOST`, not in a tight loop. A stream left behind is told to `stop`, so its reader thread ends within
    /// herdr.rs's `POLL`.
    fn watch(self: Arc<Self>) {
        let mut retry = RETRY_FIRST;
        let mut first = true;
        loop {
            if !std::mem::replace(&mut first, false) {
                if !self.send(None) {
                    return;
                }
                std::thread::sleep(retry);
                retry = (retry * 2).min(RETRY_MOST);
            }
            let list = match snapshot(self.herdr.as_ref()) {
                Ok(list) => list,
                Err(error) => {
                    warn!(%error, "nirvana remote could not read the spaces");
                    continue;
                }
            };
            let watched = panes(&list);
            if !self.send(Some(list.to_string())) {
                return;
            }
            let stop = Arc::new(AtomicBool::new(false));
            let events = match self.herdr.subscribe(subscriptions(&watched), stop.clone()) {
                Ok(events) => events,
                Err(error) => {
                    warn!(%error, "nirvana remote could not subscribe to herdr's events");
                    continue;
                }
            };
            // The stream ends when this subscription is left, however it is left.
            let _ends = Stop(stop);
            // Alpha 0.70.0: when the next list is built with no event (`next_build`).
            let mut timed;
            match snapshot(self.herdr.as_ref()) {
                Ok(again) => {
                    let moved = panes(&again) != watched;
                    timed = next_build(false, any_background(&again));
                    if !self.send(Some(again.to_string())) {
                        return;
                    }
                    if moved {
                        continue;
                    }
                }
                Err(_) => continue,
            }
            // The stream is read on its own thread so a burst can be waited out here.
            let (tx, rx) = mpsc::channel::<Value>();
            let reader = std::thread::Builder::new()
                .name("nirvana-remote-events".into())
                .spawn(move || {
                    for event in events {
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                });
            if reader.is_err() {
                continue;
            }
            let mut heard = false;
            loop {
                let event = match timed {
                    Some(wait) => match rx.recv_timeout(wait) {
                        Ok(_) => true,
                        Err(RecvTimeoutError::Timeout) => false,
                        // The stream ended (herdr fell behind, or its server is going): subscribe again.
                        Err(RecvTimeoutError::Disconnected) => break,
                    },
                    None => match rx.recv() {
                        Ok(_) => true,
                        Err(_) => break,
                    },
                };
                if event {
                    if !std::mem::replace(&mut heard, true) {
                        retry = RETRY_FIRST;
                    }
                    let started = Instant::now();
                    loop {
                        let left = SETTLE_MOST.saturating_sub(started.elapsed());
                        match rx.recv_timeout(SETTLE.min(left)) {
                            Ok(_) if !left.is_zero() => {}
                            Ok(_) | Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => break,
                        }
                    }
                }
                let Ok(list) = snapshot(self.herdr.as_ref()) else {
                    break;
                };
                let changed_agents = panes(&list) != watched;
                timed = next_build(event, any_background(&list));
                if !self.send(Some(list.to_string())) {
                    return;
                }
                if changed_agents {
                    break;
                }
            }
        }
    }
}
