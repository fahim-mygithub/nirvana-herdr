//! Plan 39 P39b's tests: the spaces, the chat, sending and uploads. Every one runs against fakes: a fake herdr socket
//! API (its lists, its `events.subscribe` streams fed by the test, and a log of every call), a Tailscale that is never
//! asked, and a peer check that admits. The identity is the fake header tailscaled would set. A server a test starts
//! binds 127.0.0.1 on a free port and is stopped by the test; files go under the private TEMP the gates set.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::herdr::{Events, Herdr};
use super::http;
use super::pcs::{Hello, Probe};
use super::peer::{PeerCheck, Refusal};
use super::routes::{self, Site};
use super::tailscale::{Held, Tailscale};
use super::{chat, send, spaces, stream};

pub(super) const OWNER: &str = "owner@example.com";
pub(super) const HOST: &str = "desk.example-tail.ts.net:8443";
pub(super) const PHONE: &str = "100.101.102.103";
pub(super) const DESK_IP: &str = "100.64.0.1";
const SESSION: &str = "6f1c2a3b-0000-4000-8000-000000000001";
pub(super) const REFUSED: &str =
    "Only the Tailscale account this PC is signed in with can connect.";

// --- Fakes ------------------------------------------------------------------------------------------------------

pub(super) struct NoTailscale;

impl Tailscale for NoTailscale {
    fn status(&self) -> Result<String, String> {
        Err("not asked".into())
    }
    fn serve_status(&self) -> Result<String, String> {
        Err("not asked".into())
    }
    fn hold(&self, _https: u16, _local: u16) -> Result<Box<dyn Held>, String> {
        Err("not asked".into())
    }
    fn clear_background(&self, _https: u16) -> Result<(), String> {
        Err("not asked".into())
    }
}

pub(super) struct Admit;

impl PeerCheck for Admit {
    fn check(&self, _peer: SocketAddr, _local: SocketAddr) -> Result<(), Refusal> {
        Ok(())
    }
}

pub(super) struct NoProbe;

impl Probe for NoProbe {
    fn hello(&self, _dns: &str, _port: u16) -> Option<Hello> {
        None
    }
}

/// herdr as the routes see it: two spaces and a stray agent in none, a screen, and every call logged.
struct FakeHerdr {
    workspaces: Mutex<Vec<Value>>,
    agents: Mutex<Vec<Value>>,
    calls: Mutex<Vec<(String, Value)>>,
    subs: Mutex<Vec<(Value, mpsc::Sender<Value>)>>,
    screen: String,
    /// Subscription readers not yet dropped (review round, S3: an old stream must end).
    readers: Arc<AtomicUsize>,
    /// A status set as `events.subscribe` is answered, with no event: a change between snapshot and subscription.
    on_subscribe: Mutex<Option<(String, String)>>,
    /// Every stream ends at once, after its ack.
    ends_at_once: AtomicBool,
}

/// A subscription's events as the fake carries them: they end when the test drops the sender, or once `stop` is set
/// (as herdr.rs's `Polled` does), and the reader count goes down when the stream is dropped.
struct FakeEvents {
    rx: mpsc::Receiver<Value>,
    stop: Arc<AtomicBool>,
    readers: Arc<AtomicUsize>,
}

impl Iterator for FakeEvents {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return None;
            }
            match self.rx.recv_timeout(Duration::from_millis(20)) {
                Ok(event) => return Some(event),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }
}

impl Drop for FakeEvents {
    fn drop(&mut self) {
        self.readers.fetch_sub(1, Ordering::SeqCst);
    }
}

impl FakeHerdr {
    fn new(folder: &Path, transcript: Option<&Path>) -> Arc<Self> {
        let mut nirvana = agent("p1", "w1", "nirvana", "working", folder);
        if let Some(path) = transcript {
            nirvana["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN] =
                json!(path.display().to_string());
        }
        Arc::new(Self {
            workspaces: Mutex::new(vec![
                json!({"workspace_id": "w1", "label": "Nirvana-Herdr", "number": 1}),
                json!({"workspace_id": "w2", "label": "3D-Printer Flow", "number": 2}),
            ]),
            agents: Mutex::new(vec![
                nirvana,
                agent("p2", "w2", "flash", "idle", folder),
                agent("p3", "w2", "printer", "blocked", folder),
                agent("p4", "w2", "reel", "done", folder),
                agent("p9", "w-gone", "stray", "working", folder),
            ]),
            calls: Mutex::default(),
            subs: Mutex::default(),
            screen: "\u{273b} Gating\u{2026} (12s \u{b7} esc to interrupt)\n\n\n> \n".into(),
            readers: Arc::default(),
            on_subscribe: Mutex::default(),
            ends_at_once: AtomicBool::new(false),
        })
    }

    fn calls(&self, method: &str) -> Vec<Value> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params.clone())
            .collect()
    }

    fn methods(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }

    fn set_status(&self, pane: &str, status: &str) {
        for agent in self.agents.lock().unwrap().iter_mut() {
            if agent["pane_id"] == pane {
                agent["agent_status"] = json!(status);
            }
        }
    }

    /// One event on the newest subscription.
    fn emit(&self, event: Value) {
        if let Some((_, tx)) = self.subs.lock().unwrap().last() {
            let _ = tx.send(event);
        }
    }

    fn subscribed(&self) -> Vec<Value> {
        self.subs
            .lock()
            .unwrap()
            .iter()
            .map(|(params, _)| params.clone())
            .collect()
    }
}

fn agent(pane: &str, space: &str, name: &str, status: &str, folder: &Path) -> Value {
    json!({
        "pane_id": pane, "workspace_id": space, "name": name, "agent": "claude",
        "title": "\u{2733} Gating two branches", "agent_status": status,
        "tokens": {"nirvana_model": "Claude Opus 5.5", "nirvana_kind": "claude"},
        "cwd": folder.display().to_string(),
    })
}

impl Herdr for FakeHerdr {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        match method {
            "workspace.list" => Ok(
                json!({"type": "workspace_list", "workspaces": *self.workspaces.lock().unwrap()}),
            ),
            "agent.list" => {
                Ok(json!({"type": "agent_list", "agents": *self.agents.lock().unwrap()}))
            }
            "agent.read" => Ok(json!({"type": "pane_read", "read": {"text": self.screen}})),
            "agent.prompt" | "agent.send_keys" => Ok(json!({"type": "ok"})),
            other => Err(format!("unexpected {other}")),
        }
    }

    fn subscribe(&self, subscriptions: Value, stop: Arc<AtomicBool>) -> Result<Events, String> {
        let (tx, rx) = mpsc::channel();
        self.subs.lock().unwrap().push((subscriptions, tx));
        if let Some((pane, status)) = self.on_subscribe.lock().unwrap().take() {
            self.set_status(&pane, &status);
        }
        if self.ends_at_once.load(Ordering::SeqCst) {
            return Ok(Box::new(std::iter::empty()));
        }
        self.readers.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(FakeEvents {
            rx,
            stop,
            readers: self.readers.clone(),
        }))
    }

    fn call_socket(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params));
        Ok(json!({"type": "agent_info"}))
    }
}

// --- The site and the wire ---------------------------------------------------------------------------------------

pub(super) fn fixed_now() -> time::OffsetDateTime {
    time::Date::from_calendar_date(2026, time::Month::November, 4)
        .unwrap()
        .with_hms(12, 0, 0)
        .unwrap()
        .assume_utc()
}

fn site_with(herdr: Arc<FakeHerdr>, claude: Option<PathBuf>, home: Option<PathBuf>) -> Arc<Site> {
    let mut site = Site::new(
        Arc::new(NoTailscale),
        herdr,
        Arc::new(Admit),
        Arc::new(NoProbe),
        super::HTTPS_PORT,
        fixed_now,
    );
    *site.owner.write().unwrap() = Some(OWNER.into());
    *site.me.write().unwrap() = Some(routes::Me {
        host: HOST.into(),
        ips: vec![DESK_IP.parse().unwrap()],
    });
    site.claude_dir = claude;
    site.nirvana_home = home;
    Arc::new(site)
}

pub(super) struct Running {
    rt: tokio::runtime::Runtime,
    pub(super) addr: SocketAddr,
    stop: tokio::sync::watch::Sender<bool>,
    site: Arc<Site>,
}

impl Drop for Running {
    fn drop(&mut self) {
        // As the supervisor does: a view left open would hold the runtime's drop until its next beat.
        self.site.streams.close_all();
        let _ = self.stop.send(true);
        self.rt
            .block_on(async { tokio::time::sleep(Duration::from_millis(20)).await });
    }
}

pub(super) fn serve_with(site: Arc<Site>, limits: http::Limits) -> Running {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let handler: Arc<dyn http::Handler> = site.clone();
    rt.spawn(http::serve(listener, handler, limits, stopped));
    Running {
        rt,
        addr,
        stop,
        site,
    }
}

pub(super) fn serve(site: Arc<Site>) -> Running {
    serve_with(site, http::Limits::DEFAULT)
}

/// What tailscaled sends for a request from the owner's phone (or `login`), without the closing blank line.
pub(super) fn proxied(method: &str, path: &str, login: &str) -> String {
    format!("{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: {login}\r\n")
}

/// The headers a POST from the page itself carries.
pub(super) fn from_page() -> String {
    format!("Sec-Fetch-Site: same-origin\r\nOrigin: https://{HOST}\r\n")
}

fn split(answer: &str) -> (u16, String, String) {
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((answer, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    (status, head.to_string(), body.to_string())
}

/// One request and its whole answer. A read that fails, or a body shorter or longer than its `Content-Length`, fails
/// the test here with what was read, instead of handing a cut body to the test's own assertions (P39c item 6: the
/// helper dropped the read's error, so a flake read as a missing line of the page).
pub(super) fn raw(addr: SocketAddr, bytes: &[u8]) -> (u16, String, String) {
    let mut stream =
        TcpStream::connect(addr).unwrap_or_else(|err| panic!("connect to {addr}: {err}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(bytes).unwrap();
    let mut answer = Vec::new();
    if let Err(err) = stream.read_to_end(&mut answer) {
        panic!(
            "the answer could not be read whole: {err} ({:?}), after {} bytes: {:?}",
            err.kind(),
            answer.len(),
            String::from_utf8_lossy(&answer[..answer.len().min(300)])
        );
    }
    whole(&answer, bytes.starts_with(b"HEAD "));
    split(&String::from_utf8_lossy(&answer))
}

/// An answer's body is exactly its `Content-Length`, when it has one (a live view has none); a HEAD answer keeps
/// the length and has no body.
pub(super) fn whole(answer: &[u8], head_request: bool) {
    // The four bytes CR LF CR LF. The first version had the escapes collapsed to two raw LFs by the file write, so
    // `windows(4)` never matched and nothing was checked (review S1; control `p39c_whole_checks`).
    let Some(end) = answer.windows(4).position(|w| w == b"\r\n\r\n") else {
        return;
    };
    let head = String::from_utf8_lossy(&answer[..end]);
    let length = head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("Content-Length")
            .then(|| value.trim().parse::<usize>().ok())?
    });
    if let Some(length) = length {
        let body = answer.len() - end - 4;
        assert!(
            body == if head_request { 0 } else { length },
            "a body of {body} bytes under Content-Length {length}: {head}"
        );
    }
}

pub(super) fn get(addr: SocketAddr, path: &str, login: &str) -> (u16, String, String) {
    raw(
        addr,
        format!("{}\r\n", proxied("GET", path, login)).as_bytes(),
    )
}

pub(super) fn post(
    addr: SocketAddr,
    path: &str,
    kind: &str,
    body: &[u8],
    login: &str,
) -> (u16, String, String) {
    let mut bytes = format!(
        "{}{}Content-Type: {kind}\r\nContent-Length: {}\r\n\r\n",
        proxied("POST", path, login),
        from_page(),
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    raw(addr, &bytes)
}

fn send_json(addr: SocketAddr, body: Value) -> (u16, Value) {
    let (status, _, text) = post(
        addr,
        "/api/send",
        "application/json",
        body.to_string().as_bytes(),
        OWNER,
    );
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// A live view read as it comes.
struct View {
    stream: TcpStream,
    buffer: String,
}

impl View {
    fn open(addr: SocketAddr, path: &str) -> Self {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(format!("{}\r\n", proxied("GET", path, OWNER)).as_bytes())
            .unwrap();
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        Self {
            stream,
            buffer: String::new(),
        }
    }

    /// Read for up to `within`; false once the server closed.
    fn pump(&mut self, within: Duration) -> bool {
        let started = Instant::now();
        let mut chunk = [0u8; 8192];
        while started.elapsed() < within {
            match self.stream.read(&mut chunk) {
                Ok(0) => return false,
                Ok(read) => self
                    .buffer
                    .push_str(&String::from_utf8_lossy(&chunk[..read])),
                Err(err)
                    if matches!(
                        err.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => return false,
            }
            if self.buffer.contains("\n\n") {
                return true;
            }
        }
        true
    }

    /// The next `name` event's data, within `within`; events of other names are passed over.
    fn next(&mut self, name: &str, within: Duration) -> Option<Value> {
        let started = Instant::now();
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let block: String = self.buffer.drain(..end + 2).collect();
                let mut kind = "";
                let mut data = "";
                for line in block.lines() {
                    if let Some(rest) = line.strip_prefix("event: ") {
                        kind = rest;
                    }
                    if let Some(rest) = line.strip_prefix("data: ") {
                        data = rest;
                    }
                }
                if kind == name {
                    return serde_json::from_str(data).ok();
                }
            }
            if started.elapsed() >= within || !self.pump(Duration::from_millis(300)) {
                return None;
            }
        }
    }

    /// Whether the server closes the view within `within`.
    fn closes(&mut self, within: Duration) -> bool {
        let started = Instant::now();
        while started.elapsed() < within {
            if !self.pump(Duration::from_millis(200)) {
                return true;
            }
            self.buffer.clear();
        }
        false
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p39b-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A Claude transcript as Claude Code writes one, in `<claude>/projects/<slug>/<session>.jsonl`.
fn transcript(claude: &Path, slug: &str) -> PathBuf {
    let dir = claude.join("projects").join(slug);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{SESSION}.jsonl"));
    std::fs::write(&path, TRANSCRIPT).unwrap();
    path
}

const TRANSCRIPT: &str = concat!(
    r#"{"type":"user","uuid":"u1","message":{"role":"user","content":"Gate G and L together"}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"Running the gates."},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"bash gates.sh","description":"Run the gates"}}]}}"#,
    "\n",
    r#"{"type":"user","uuid":"u2","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"4631/4632\nfmt clean","is_error":false}]}}"#,
    "\n",
    r#"{"type":"user","uuid":"u3","isMeta":true,"message":{"role":"user","content":"Caveat: a note Claude Code adds"}}"#,
    "\n",
    r#"{"type":"user","uuid":"u4","message":{"role":"user","content":"<command-name>/model</command-name>"}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a2","isSidechain":true,"message":{"content":[{"type":"text","text":"a subagent's line"}]}}"#,
    "\n",
    r#"{"type":"assistant","uuid":"a3","message":{"content":[{"type":"text","text":"All gates are green."}]}}"#,
    "\n",
);

/// A folder, a Claude config with the transcript for it, and herdr with the Nirvana agent pointing at it.
pub(super) struct World {
    root: PathBuf,
    pub(super) folder: PathBuf,
    claude: PathBuf,
    path: PathBuf,
    herdr: Arc<FakeHerdr>,
}

pub(super) fn world(name: &str) -> World {
    let root = scratch(name);
    let folder = root.join("space");
    std::fs::create_dir_all(&folder).unwrap();
    let claude = root.join("claude");
    let path = transcript(
        &claude,
        &crate::nirvana_launch::encode_project_folder(&folder),
    );
    let herdr = FakeHerdr::new(&folder, Some(&path));
    World {
        root,
        folder,
        claude,
        path,
        herdr,
    }
}

impl World {
    pub(super) fn site(&self) -> Arc<Site> {
        site_with(self.herdr.clone(), Some(self.claude.clone()), None)
    }

    pub(super) fn uploads(&self) -> PathBuf {
        self.folder.join(".nirvana").join("uploads")
    }

    /// The files in the uploads folder, `.gitignore` left out.
    pub(super) fn uploaded(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.uploads())
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != ".gitignore")
            .collect();
        names.sort();
        names
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// --- §2.1 spaces ------------------------------------------------------------------------------------------------

#[test]
fn the_spaces_list_has_every_space_its_agents_and_their_states() {
    let w = world("list");
    let list = spaces::snapshot(w.herdr.as_ref()).unwrap();
    let spaces = list["spaces"].as_array().unwrap();
    assert_eq!(spaces.len(), 2);
    assert_eq!(spaces[0]["name"], "Nirvana-Herdr");
    assert_eq!(spaces[0]["status"], "working");
    assert_eq!(spaces[0]["agents"][0]["id"], "p1");
    assert_eq!(spaces[0]["agents"][0]["model"], "Claude Opus 5.5");
    assert_eq!(
        spaces[0]["agents"][0]["line"], "Gating two branches",
        "the state mark is not the task"
    );
    let words: Vec<&str> = spaces[1]["agents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| agent["status"].as_str().unwrap())
        .collect();
    assert_eq!(words, ["idle", "needs", "done"]);
    assert_eq!(
        spaces[1]["status"], "needs",
        "needs you outranks done and idle"
    );
    assert!(
        !list.to_string().contains("stray"),
        "an agent in no space of this herdr is not listed"
    );
}

#[test]
fn api_spaces_answers_the_list_and_a_refused_caller_gets_none() {
    let w = world("api-spaces");
    let running = serve(w.site());
    let (status, _, body) = get(running.addr, "/api/spaces", OWNER);
    assert_eq!(status, 200);
    assert!(body.contains("Nirvana-Herdr"), "{body}");
    let (status, _, body) = get(running.addr, "/api/spaces", "friend@example.com");
    assert_eq!(status, 403);
    assert_eq!(body, REFUSED);
}

#[test]
fn the_live_view_pushes_a_change_that_events_subscribe_reports() {
    let w = world("view");
    let running = serve(w.site());
    let mut view = View::open(running.addr, "/api/events");
    let first = view
        .next("spaces", Duration::from_secs(5))
        .expect("the whole list first");
    assert_eq!(first["spaces"][1]["agents"][0]["status"], "idle");
    // herdr reports p2 at work; the page hears it without asking.
    let started = Instant::now();
    while w.herdr.subscribed().is_empty() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    w.herdr.set_status("p2", "working");
    w.herdr
        .emit(json!({"event": "pane.agent_status_changed", "data": {"pane_id": "p2"}}));
    let next = view
        .next("spaces", Duration::from_secs(5))
        .expect("the change, pushed");
    assert_eq!(next["spaces"][1]["agents"][0]["status"], "working");
}

#[test]
fn the_feed_subscribes_to_each_agents_pane_and_again_when_agents_change() {
    let w = world("subs");
    let feed = spaces::Feed::new(w.herdr.clone());
    let lists = feed.listen().unwrap();
    assert!(lists.recv_timeout(Duration::from_secs(2)).is_ok());
    let started = Instant::now();
    while w.herdr.subscribed().is_empty() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let first = w.herdr.subscribed()[0].clone();
    let watched = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .filter(|sub| sub["type"] == "pane.agent_status_changed")
            .map(|sub| sub["pane_id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        watched(&first),
        ["p1", "p2", "p3", "p4"],
        "each agent's pane, not the stray's"
    );
    let kinds: Vec<&str> = first
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|sub| sub["type"].as_str())
        .collect();
    assert!(
        kinds.contains(&"workspace.created") && kinds.contains(&"pane.agent_detected"),
        "{kinds:?}"
    );
    // A new agent: the feed subscribes again, now with its pane.
    w.herdr
        .agents
        .lock()
        .unwrap()
        .push(agent("p5", "w1", "worker", "working", &w.folder));
    w.herdr.emit(json!({"event": "pane_agent_detected"}));
    let started = Instant::now();
    while w.herdr.subscribed().len() < 2 && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let again = w.herdr.subscribed();
    assert_eq!(again.len(), 2, "subscribed again");
    assert!(watched(&again[1]).contains(&"p5".to_string()));
    assert_eq!(feed.listening(), 1);
}

#[test]
fn a_herdr_stream_that_ends_is_subscribed_again() {
    let w = world("resub");
    let running = serve(w.site());
    let mut view = View::open(running.addr, "/api/events");
    assert!(view.next("spaces", Duration::from_secs(5)).is_some());
    let started = Instant::now();
    while w.herdr.subscribed().is_empty() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    // The stream ends (herdr's server stopped writing it): every sender goes.
    w.herdr.subs.lock().unwrap().iter_mut().for_each(|(_, tx)| {
        let (dead, _) = mpsc::channel();
        *tx = dead;
    });
    let started = Instant::now();
    while w.herdr.subscribed().len() < 2 && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(w.herdr.subscribed().len() >= 2, "subscribed again");
    // And the page's view still carries changes.
    w.herdr.set_status("p4", "working");
    w.herdr
        .emit(json!({"event": "pane.agent_status_changed", "data": {"pane_id": "p4"}}));
    let next = view
        .next("spaces", Duration::from_secs(5))
        .expect("still live");
    assert_eq!(next["spaces"][1]["agents"][2]["status"], "working");
}

#[test]
fn a_live_view_ends_when_nirvana_remote_stops_and_says_when_to_retry() {
    let w = world("stop");
    let site = w.site();
    let running = serve(site.clone());
    let mut view = View::open(running.addr, "/api/events");
    assert!(view.next("spaces", Duration::from_secs(5)).is_some());
    let mut head = View::open(running.addr, "/api/events");
    head.pump(Duration::from_secs(3));
    assert!(head.buffer.starts_with("HTTP/1.1 200"), "{}", head.buffer);
    assert!(head.buffer.contains("Content-Type: text/event-stream"));
    assert!(
        !head.buffer.contains("Content-Length"),
        "a live view has no length"
    );
    assert!(head
        .buffer
        .contains("Content-Security-Policy: default-src 'self'"));
    assert!(head
        .buffer
        .contains(&format!("retry: {}", stream::RETRY_MS)));
    site.streams.close_all();
    assert!(
        view.closes(Duration::from_secs(4)),
        "ended with Nirvana Remote"
    );
    assert!(head.closes(Duration::from_secs(4)));
}

#[test]
fn at_most_six_live_views_and_a_new_one_ends_the_oldest() {
    let w = world("most");
    let site = w.site();
    let running = serve(site.clone());
    let mut views: Vec<View> = (0..stream::MOST)
        .map(|_| {
            let mut view = View::open(running.addr, "/api/events");
            assert!(view.next("spaces", Duration::from_secs(5)).is_some());
            view
        })
        .collect();
    assert_eq!(site.streams.count(), stream::MOST);
    let mut newest = View::open(running.addr, "/api/events");
    assert!(newest.next("spaces", Duration::from_secs(5)).is_some());
    assert!(
        views[0].closes(Duration::from_secs(4)),
        "the oldest gave way"
    );
    assert!(
        !views[1].closes(Duration::from_millis(1500)),
        "the rest stay"
    );
    assert_eq!(site.streams.count(), stream::MOST);
}

// --- §2.2 chat --------------------------------------------------------------------------------------------------

#[test]
fn a_transcript_reads_as_bubbles_with_each_tool_call_folded() {
    let bubbles = chat::bubbles(TRANSCRIPT);
    let kinds: Vec<&str> = bubbles
        .iter()
        .map(|b| b["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["user", "agent", "tool", "agent"], "{bubbles:?}");
    assert_eq!(bubbles[0]["text"], "Gate G and L together");
    assert_eq!(bubbles[2]["id"], "toolu_1");
    assert_eq!(bubbles[2]["tool"], "Bash");
    assert_eq!(bubbles[2]["arg"], "Run the gates");
    assert_eq!(
        bubbles[2]["result"], "4631/4632",
        "the result's first line on the folded line"
    );
    assert_eq!(bubbles[2]["ok"], true);
    assert!(bubbles[2]["detail"]
        .as_str()
        .unwrap()
        .contains("bash gates.sh"));
    assert!(bubbles[2]["detail"].as_str().unwrap().contains("fmt clean"));
    assert_eq!(bubbles[3]["text"], "All gates are green.");
    let all = serde_json::to_string(&bubbles).unwrap();
    assert!(
        !all.contains("Caveat"),
        "a note Claude Code adds is not a bubble"
    );
    assert!(
        !all.contains("command-name"),
        "a slash command's echo is not a bubble"
    );
    assert!(
        !all.contains("subagent"),
        "a subagent's line is not a bubble"
    );
    // A failed call reads as one.
    let failed = format!(
        "{TRANSCRIPT}{}\n",
        r#"{"type":"user","uuid":"u9","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":[{"type":"text","text":"exit 1"}],"is_error":true}]}}"#
    );
    let again = chat::bubbles(&failed);
    assert_eq!(again[2]["ok"], false);
    assert_eq!(again[2]["result"], "exit 1");
}

#[test]
fn the_transcript_is_the_agents_own_never_a_path_from_the_request() {
    let w = world("own");
    let with = |token: &Path| {
        let mut agent = agent("p1", "w1", "nirvana", "idle", &w.folder);
        agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN] =
            json!(token.display().to_string());
        chat::transcript_for(&agent, &[], Some(&w.claude), None)
    };
    assert_eq!(with(&w.path), Some(w.path.clone()));
    // Anything that is not a transcript of this Claude config is refused, even when herdr's token names it.
    let elsewhere = w.root.join("elsewhere");
    std::fs::create_dir_all(elsewhere.join("x")).unwrap();
    std::fs::write(
        elsewhere.join("x").join(format!("{SESSION}.jsonl")),
        TRANSCRIPT,
    )
    .unwrap();
    assert_eq!(
        with(&elsewhere.join("x").join(format!("{SESSION}.jsonl"))),
        None
    );
    let odd = w.path.with_file_name("-leading.jsonl");
    std::fs::write(&odd, TRANSCRIPT).unwrap();
    assert_eq!(with(&odd), None, "not a plain session id");
    let text = w.path.with_extension("txt");
    std::fs::write(&text, TRANSCRIPT).unwrap();
    assert_eq!(with(&text), None);
    assert_eq!(
        with(&w.path.with_file_name("0000.jsonl")),
        None,
        "not there"
    );
    // Over HTTP the page names an agent, not a file.
    let running = serve(w.site());
    assert_eq!(get(running.addr, "/api/chat", OWNER).0, 400);
    assert_eq!(
        get(running.addr, "/api/chat?agent=p9", OWNER).0,
        404,
        "the stray agent is in no space"
    );
    assert_eq!(
        get(running.addr, "/api/chat?agent=..%2F..%2Fwin.ini", OWNER).0,
        404
    );
    let (status, _, body) = get(
        running.addr,
        &format!("/api/chat?agent=p1&path={}", elsewhere.display()),
        OWNER,
    );
    assert_eq!(status, 200);
    assert!(body.contains("All gates are green."), "{body}");
}

#[test]
fn the_folder_session_record_is_the_fallback_with_its_checks() {
    let w = world("record");
    let home = w.root.join("home");
    let mut agent = agent("p1", "w1", "nirvana", "idle", &w.folder);
    assert_eq!(
        chat::transcript_for(&agent, &[], Some(&w.claude), Some(&home)),
        None,
        "no token, no record"
    );
    let record = crate::nirvana_launch::session_record_path(&home, &w.folder);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    let write = |id: &str, path: &Path| {
        std::fs::write(
            &record,
            json!({"session_id": id, "at": "2026-11-04T10:00:00Z", "transcript": path}).to_string(),
        )
        .unwrap();
    };
    write(SESSION, &w.path);
    assert_eq!(
        chat::transcript_for(&agent, &[], Some(&w.claude), Some(&home)),
        Some(w.path.clone())
    );
    // The record's checks: the file's stem is its id.
    write("0c0ffee0-1111-4222-8333-444455556666", &w.path);
    assert_eq!(
        chat::transcript_for(&agent, &[], Some(&w.claude), Some(&home)),
        None
    );
    // A token, when herdr has one, wins over the record.
    write(SESSION, &w.path);
    agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN] = json!("C:\\elsewhere\\x.jsonl");
    assert_eq!(
        chat::transcript_for(&agent, &[], Some(&w.claude), Some(&home)),
        None
    );
}

#[test]
fn api_chat_answers_the_bubbles_and_the_live_screen() {
    let w = world("chat");
    let running = serve(w.site());
    let (status, _, body) = get(running.addr, "/api/chat?agent=p1", OWNER);
    assert_eq!(status, 200, "{body}");
    let chat: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(chat["agent"]["space"], "Nirvana-Herdr");
    assert_eq!(chat["agent"]["status"], "working");
    assert_eq!(chat["transcript"], true);
    assert_eq!(chat["messages"].as_array().unwrap().len(), 4);
    assert_eq!(
        chat["live"],
        "\u{273b} Gating\u{2026} (12s \u{b7} esc to interrupt)\n\n>"
    );
    let read = w.herdr.calls("agent.read");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0]["target"], "p1");
    // An agent that is not Claude: no transcript, the screen only.
    let (_, _, body) = get(running.addr, "/api/chat?agent=p2", OWNER);
    let other: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(other["transcript"], false);
    assert_eq!(other["messages"], json!([]));
}

#[test]
fn the_chat_view_pushes_new_bubbles_as_the_transcript_grows() {
    let w = world("grow");
    let running = serve(w.site());
    let mut view = View::open(running.addr, "/api/chat/events?agent=p1");
    let first = view
        .next("chat", Duration::from_secs(5))
        .expect("the whole chat first");
    assert_eq!(first["messages"].as_array().unwrap().len(), 4);
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&w.path)
        .unwrap();
    file.write_all(concat!(r#"{"type":"assistant","uuid":"a4","message":{"content":[{"type":"tool_use","id":"toolu_2","name":"Read","input":{"file_path":"C:\\x\\notes.md"}}]}}"#, "\n").as_bytes())
        .unwrap();
    drop(file);
    let grown = view
        .next("messages", Duration::from_secs(5))
        .expect("the new bubble");
    assert_eq!(
        grown["messages"],
        json!([chat::bubbles(&std::fs::read_to_string(&w.path).unwrap())[4]])
    );
    // Its result comes later and changes that bubble only.
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&w.path)
        .unwrap();
    file.write_all(concat!(r#"{"type":"user","uuid":"u5","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_2","content":"12 lines"}]}}"#, "\n").as_bytes())
        .unwrap();
    drop(file);
    let result = view
        .next("messages", Duration::from_secs(5))
        .expect("the result");
    let changed = result["messages"].as_array().unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["id"], "toolu_2");
    assert_eq!(changed[0]["result"], "12 lines");
    // A state change brings the agent and the screen again.
    let started = Instant::now();
    while w.herdr.subscribed().is_empty() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    w.herdr.set_status("p1", "done");
    w.herdr
        .emit(json!({"event": "pane.agent_status_changed", "data": {"pane_id": "p1"}}));
    let state = view
        .next("agent", Duration::from_secs(5))
        .expect("the state");
    assert_eq!(state["status"], "done");
    assert!(view.next("live", Duration::from_secs(5)).is_some());
}

// --- §2.3 send --------------------------------------------------------------------------------------------------

#[test]
fn a_message_is_typed_through_agent_prompt() {
    let w = world("prompt");
    let running = serve(w.site());
    let (status, answer) = send_json(
        running.addr,
        json!({"agent": "p2", "text": "  hello there  "}),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(
        w.herdr.calls("agent.prompt"),
        [json!({"target": "p2", "text": "hello there"})]
    );
    assert!(w.herdr.calls("agent.send_keys").is_empty());
    // Not JSON, no length, nothing to send, no such agent: nothing is typed.
    assert_eq!(
        post(running.addr, "/api/send", "text/plain", b"hi", OWNER).0,
        415
    );
    assert_eq!(
        send_json(running.addr, json!({"agent": "p2", "text": "  "})).0,
        400
    );
    assert_eq!(
        send_json(running.addr, json!({"agent": "p9", "text": "x"})).0,
        404
    );
    // A send over its 64 KiB is refused on its head, before its body comes.
    let started = Instant::now();
    let big = format!(
        "{}{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        proxied("POST", "/api/send", OWNER),
        from_page(),
        send::SEND_MOST + 1
    );
    assert_eq!(raw(running.addr, big.as_bytes()).0, 413);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(w.herdr.calls("agent.prompt").len(), 1);
}

#[test]
fn a_working_agent_asks_queue_or_send_now() {
    let w = world("choose");
    let running = serve(w.site());
    let (status, answer) = send_json(running.addr, json!({"agent": "p1", "text": "also this"}));
    assert_eq!(status, 409);
    assert_eq!(answer["choose"], true);
    assert!(
        w.herdr.calls("agent.prompt").is_empty(),
        "nothing typed before the owner picks"
    );
    let (status, _) = send_json(
        running.addr,
        json!({"agent": "p1", "text": "also this", "mode": "queue"}),
    );
    assert_eq!(status, 200);
    assert_eq!(w.herdr.calls("agent.prompt").len(), 1);
    assert!(
        w.herdr.calls("agent.send_keys").is_empty(),
        "queue does not stop the turn"
    );
    assert_eq!(
        send_json(
            running.addr,
            json!({"agent": "p1", "text": "x", "mode": "later"})
        )
        .0,
        400
    );
}

#[test]
fn send_now_presses_esc_and_waits_before_typing() {
    let w = world("now");
    let running = serve(w.site());
    let (status, answer) = send_json(
        running.addr,
        json!({"agent": "p1", "text": "stop and do this", "mode": "now"}),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["stopped"], true);
    let order: Vec<String> = w
        .herdr
        .methods()
        .into_iter()
        .filter(|method| {
            ["agent.send_keys", "agent.wait", "agent.prompt"].contains(&method.as_str())
        })
        .collect();
    assert_eq!(order, ["agent.send_keys", "agent.wait", "agent.prompt"]);
    assert_eq!(
        w.herdr.calls("agent.send_keys"),
        [json!({"target": "p1", "keys": ["esc"]})]
    );
    let wait = &w.herdr.calls("agent.wait")[0];
    assert_eq!(wait["target"], "p1");
    assert!(!wait["until"].to_string().contains("working"));
}

#[test]
fn a_blocked_agent_is_not_typed_into() {
    let w = world("blocked");
    let running = serve(w.site());
    for mode in [json!(null), json!("queue"), json!("now")] {
        let (status, answer) = send_json(
            running.addr,
            json!({"agent": "p3", "text": "yes", "mode": mode}),
        );
        assert_eq!(status, 409, "{answer}");
    }
    assert!(w.herdr.calls("agent.prompt").is_empty());
    assert!(w.herdr.calls("agent.send_keys").is_empty());
}

#[test]
fn two_sends_at_once_both_land() {
    let w = world("both");
    let running = serve(w.site());
    let addr = running.addr;
    let sends: Vec<_> = ["from the phone", "from the laptop"]
        .into_iter()
        .map(|text| {
            std::thread::spawn(move || send_json(addr, json!({"agent": "p2", "text": text})).0)
        })
        .collect();
    for send in sends {
        assert_eq!(send.join().unwrap(), 200);
    }
    let mut texts: Vec<String> = w
        .herdr
        .calls("agent.prompt")
        .iter()
        .map(|params| params["text"].as_str().unwrap().to_string())
        .collect();
    texts.sort();
    assert_eq!(texts, ["from the laptop", "from the phone"]);
}

// --- Who gets nothing ---------------------------------------------------------------------------------------------

#[test]
fn a_refused_caller_gets_nothing_from_the_new_routes() {
    let w = world("refused");
    let running = serve(w.site());
    let send = json!({"agent": "p2", "text": "hi"}).to_string();
    for login in ["friend@example.com", ""] {
        for path in [
            "/api/spaces",
            "/api/events",
            "/api/chat?agent=p1",
            "/api/chat/events?agent=p1",
        ] {
            let (status, _, body) = get(running.addr, path, login);
            assert_eq!(
                (status, body.as_str()),
                (403, REFUSED),
                "{path} for {login:?}"
            );
        }
        let (status, _, body) = post(
            running.addr,
            "/api/send",
            "application/json",
            send.as_bytes(),
            login,
        );
        assert_eq!((status, body.as_str()), (403, REFUSED));
        let (status, _, _) = post(
            running.addr,
            "/api/upload?agent=p2",
            "image/png",
            b"png bytes",
            login,
        );
        assert_eq!(status, 403);
    }
    // The owner's login from this PC itself, and the owner's from another site's page.
    let mut local = format!("POST /api/send HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {DESK_IP}\r\nTailscale-User-Login: {OWNER}\r\n");
    local.push_str(&format!(
        "{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{send}",
        from_page(),
        send.len()
    ));
    assert_eq!(raw(running.addr, local.as_bytes()).0, 403);
    let cross = format!(
        "{}Sec-Fetch-Site: cross-site\r\nContent-Type: image/png\r\nContent-Length: 3\r\n\r\npng",
        proxied("POST", "/api/upload?agent=p2", OWNER)
    );
    assert_eq!(raw(running.addr, cross.as_bytes()).0, 403);
    let bare = format!(
        "{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{send}",
        proxied("POST", "/api/send", OWNER),
        send.len()
    );
    assert_eq!(
        raw(running.addr, bare.as_bytes()).0,
        403,
        "no Sec-Fetch-Site, no Origin"
    );
    assert!(
        w.herdr.methods().is_empty(),
        "herdr was not asked a thing: {:?}",
        w.herdr.methods()
    );
    assert!(!w.folder.join(".nirvana").exists(), "no folder, no file");
}

// --- Uploads ------------------------------------------------------------------------------------------------------

#[test]
fn an_upload_lands_in_the_spaces_uploads_folder_under_the_servers_name() {
    let w = world("upload");
    let running = serve(w.site());
    let head = format!(
        "{}{}X-File-Name: ..\\..\\CON.png\r\nContent-Disposition: attachment; filename=\"../../evil.png\"\r\n\
         Content-Type: image/png\r\nContent-Length: 9\r\n\r\npng bytes",
        proxied("POST", "/api/upload?agent=p2&name=..%2F..%2Fevil.png", OWNER),
        from_page()
    );
    let (status, _, body) = raw(running.addr, head.as_bytes());
    assert_eq!(status, 200, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    let name = answer["file"].as_str().unwrap();
    assert!(
        send::is_upload_name(name) && name.ends_with(".png"),
        "{name}"
    );
    assert_eq!(
        w.uploaded(),
        [name.to_string()],
        "one file, the server's name, nothing else"
    );
    assert_eq!(std::fs::read(w.uploads().join(name)).unwrap(), b"png bytes");
    assert_eq!(answer["path"], w.uploads().join(name).display().to_string());
    assert_eq!(
        std::fs::read_to_string(w.uploads().join(".gitignore")).unwrap(),
        "*\n"
    );
    assert!(!w.root.join("evil.png").exists() && !w.folder.join("evil.png").exists());
    // A type the server does not know is `.bin`; no length is 411.
    let (_, _, body) = post(
        running.addr,
        "/api/upload?agent=p2",
        "application/x-thing",
        b"x",
        OWNER,
    );
    assert!(body.contains(".bin\""), "{body}");
    let none = format!(
        "{}{}Content-Type: image/png\r\n\r\n",
        proxied("POST", "/api/upload?agent=p2", OWNER),
        from_page()
    );
    assert_eq!(raw(running.addr, none.as_bytes()).0, 411);
}

/// Send `head`, then `body` from another thread; the answer.
pub(super) fn upload_streamed(
    addr: SocketAddr,
    head: String,
    body: impl FnOnce(&mut TcpStream) + Send + 'static,
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let sender = std::thread::spawn(move || body(&mut writer));
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    let _ = sender.join();
    let (status, _, body) = split(&String::from_utf8_lossy(&answer));
    (status, body)
}

/// `upload_streamed`, with how long the answer's first byte took: after a cut the server drains what the caller
/// still sends (P39u review round, S2), so the exchange ends later than the answer arrives.
pub(super) fn upload_timed(
    addr: SocketAddr,
    head: String,
    body: impl FnOnce(&mut TcpStream) + Send + 'static,
) -> (u16, String, Duration) {
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    let mut writer = stream.try_clone().unwrap();
    let sender = std::thread::spawn(move || body(&mut writer));
    let mut answer = vec![0u8; 1];
    let first = match stream.read(&mut answer) {
        Ok(1) => started.elapsed(),
        _ => {
            answer.clear();
            started.elapsed()
        }
    };
    let _ = stream.read_to_end(&mut answer);
    let _ = sender.join();
    let (status, _, body) = split(&String::from_utf8_lossy(&answer));
    (status, body, first)
}

pub(super) fn upload_head(length: Option<u64>) -> String {
    let framing = length.map_or_else(
        || "Transfer-Encoding: chunked\r\n".to_string(),
        |n| format!("Content-Length: {n}\r\n"),
    );
    format!(
        "{}{}Content-Type: image/png\r\n{framing}\r\n",
        proxied("POST", "/api/upload?agent=p2", OWNER),
        from_page()
    )
}

#[test]
fn an_upload_over_50_mb_is_refused_before_its_body_is_read() {
    let w = world("too-big");
    let running = serve(w.site());
    let started = Instant::now();
    // The head alone: the server answers without waiting for a byte of the body.
    let (status, _, body) = raw(
        running.addr,
        upload_head(Some(send::UPLOAD_MOST + 1)).as_bytes(),
    );
    assert_eq!(status, 413, "{body}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(w.uploaded().is_empty());
    assert!(
        w.herdr.methods().is_empty(),
        "refused on the head, before the agent was looked up"
    );
}

#[test]
fn a_chunked_upload_is_refused_the_moment_it_passes_50_mb() {
    let w = world("chunked");
    let running = serve(w.site());
    let (status, body) = upload_streamed(running.addr, upload_head(None), |out| {
        let mib = vec![b'x'; 1024 * 1024];
        for _ in 0..50 {
            if out.write_all(b"100000\r\n").is_err()
                || out.write_all(&mib).is_err()
                || out.write_all(b"\r\n").is_err()
            {
                return;
            }
        }
        // One byte past 50 MiB.
        let _ = out.write_all(b"1\r\nx\r\n0\r\n\r\n");
    });
    assert_eq!(status, 413, "{body}");
    assert!(
        w.uploaded().is_empty(),
        "no file and no .partial: {:?}",
        w.uploaded()
    );
    // Exactly 50 MiB, chunked, is taken.
    let (status, body) = upload_streamed(running.addr, upload_head(None), |out| {
        let mib = vec![b'y'; 1024 * 1024];
        for _ in 0..50 {
            let _ = out.write_all(b"100000\r\n");
            let _ = out.write_all(&mib);
            let _ = out.write_all(b"\r\n");
        }
        let _ = out.write_all(b"0\r\n\r\n");
    });
    assert_eq!(status, 200, "{body}");
    assert_eq!(w.uploaded().len(), 1);
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        answer["size"],
        send::UPLOAD_MOST,
        "the chunks, without their framing"
    );
}

#[test]
fn a_50_mb_upload_is_taken_whole() {
    let w = world("whole");
    let running = serve(w.site());
    let (status, body) =
        upload_streamed(running.addr, upload_head(Some(send::UPLOAD_MOST)), |out| {
            let mib = vec![b'z'; 1024 * 1024];
            for _ in 0..50 {
                let _ = out.write_all(&mib);
            }
        });
    assert_eq!(status, 200, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["size"], send::UPLOAD_MOST);
    let name = answer["file"].as_str().unwrap();
    assert_eq!(
        std::fs::metadata(w.uploads().join(name)).unwrap().len(),
        send::UPLOAD_MOST
    );
}

#[test]
fn a_slow_upload_is_cut_and_leaves_no_file() {
    let w = world("slow");
    let running = serve_with(
        w.site(),
        http::Limits {
            read_timeout: Duration::from_millis(300),
            ..http::Limits::DEFAULT
        },
    );
    let started = Instant::now();
    let (status, body) = upload_streamed(running.addr, upload_head(Some(1000)), |out| {
        let _ = out.write_all(b"only some of it");
        std::thread::sleep(Duration::from_secs(3));
    });
    assert_eq!(
        status, 400,
        "a body that stops: not 408, which a browser re-sends"
    );
    // The 400 is the timeout's, not a malformed body's (P39u moved the timeout off 408 to 400).
    assert!(body.contains("the upload was too slow"), "{body}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(w.uploaded().is_empty(), "{:?}", w.uploaded());
    // The average-rate deadline: a body that keeps trickling is cut too.
    let running = serve_with(
        w.site(),
        http::Limits {
            read_timeout: Duration::from_millis(500),
            connections: http::CONNECTIONS,
            body_rate: 1000,
            body_most: http::BODY_MOST,
        },
    );
    let (status, _, answered) = upload_timed(running.addr, upload_head(Some(1500)), |out| {
        // Two bytes each 100 ms for 8 s: never idle for the 0.5 s, always under the rate.
        for _ in 0..80 {
            if out.write_all(b"xx").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    assert_eq!(status, 400, "1500 bytes at 1000/s get 0.5 s plus 1 s");
    // The answer's time, not the exchange's: since P39u's review round the server drains what the sender still
    // sends after the cut.
    assert!(
        answered < Duration::from_millis(4500),
        "cut at the deadline, not when the sender stopped: {answered:?}"
    );
    assert!(w.uploaded().is_empty());
}

#[test]
fn the_33rd_connection_waits_while_a_50_mb_upload_holds_a_slot() {
    let w = world("cap");
    let running = serve(w.site());
    let addr = running.addr;
    let (go, wait) = mpsc::channel::<()>();
    let upload = std::thread::spawn(move || {
        upload_streamed(addr, upload_head(Some(send::UPLOAD_MOST)), move |out| {
            let mib = vec![b'c'; 1024 * 1024];
            let _ = out.write_all(&mib);
            let _ = wait.recv();
            for _ in 1..50 {
                let _ = out.write_all(&mib);
            }
        })
    });
    std::thread::sleep(Duration::from_millis(300));
    let stalled: Vec<TcpStream> = (0..http::CONNECTIONS - 1)
        .map(|_| TcpStream::connect(addr).unwrap())
        .collect();
    std::thread::sleep(Duration::from_millis(300));
    let mut late = TcpStream::connect(addr).unwrap();
    late.write_all(format!("{}\r\n", proxied("GET", "/api/hello", OWNER)).as_bytes())
        .unwrap();
    late.set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut first = [0u8; 1];
    assert!(
        late.read(&mut first).is_err(),
        "answered while every slot was taken"
    );
    drop(stalled);
    late.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut answer = String::new();
    late.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    go.send(()).unwrap();
    let (status, body) = upload.join().unwrap();
    assert_eq!(status, 200, "{body}");
}

#[test]
fn a_send_adds_the_uploaded_files_paths_and_refuses_any_other_name() {
    let w = world("paths");
    let running = serve(w.site());
    let (_, _, body) = post(
        running.addr,
        "/api/upload?agent=p2",
        "image/jpeg",
        b"jpeg",
        OWNER,
    );
    let name = serde_json::from_str::<Value>(&body).unwrap()["file"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, answer) = send_json(
        running.addr,
        json!({"agent": "p2", "text": "look", "files": [name]}),
    );
    assert_eq!(status, 200, "{answer}");
    let typed = w.herdr.calls("agent.prompt")[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    // Plan 40: an image's path is followed by its note (send::note_for); the path still comes first.
    let path = w.uploads().join(&name).display().to_string();
    assert_eq!(
        typed,
        format!(
            "look {path} (image, 4 bytes; herdr look \"{path}\" --ask \"<your question>\" reads it and answers in text)"
        ),
        "after a space, not a newline: an Enter in a pane without bracketed paste"
    );
    for bad in [
        "..\\..\\Windows\\win.ini",
        "../x.png",
        "phone-20261104-120000-deadbeef.png",
        &format!("{name}:hidden"),
        "CON.png",
        ".gitignore",
        "..\\uploads\\.gitignore",
    ] {
        let (status, _) = send_json(
            running.addr,
            json!({"agent": "p2", "text": "x", "files": [bad]}),
        );
        assert_eq!(status, 400, "{bad}");
    }
    assert_eq!(w.herdr.calls("agent.prompt").len(), 1);
}

#[test]
fn upload_names_are_the_servers_own() {
    let name = send::name_for("image/png; charset=binary", fixed_now());
    assert!(
        name.starts_with("phone-20261104-120000-") && name.ends_with(".png"),
        "{name}"
    );
    assert!(send::is_upload_name(&name));
    assert_ne!(
        send::name_for("image/png", fixed_now()),
        send::name_for("image/png", fixed_now())
    );
    assert!(
        send::name_for("text/html", fixed_now()).ends_with(".bin"),
        "a type it does not know"
    );
    for bad in [
        "CON.png",
        "phone-20261104-120000-deadbeef.png:stream",
        "phone-20261104-120000-deadbeef.exe",
        "phone-20261104-120000-deadbee.png",
        "phone-2026110-120000-deadbeef.png",
        "../phone-20261104-120000-deadbeef.png",
        "phone-20261104-120000-deadbeef.png.partial",
    ] {
        assert!(!send::is_upload_name(bad), "{bad}");
    }
}

#[test]
fn the_body_framing_is_read_from_the_head() {
    let head = |extra: &str| {
        http::parse_head(format!("POST / HTTP/1.1\r\nHost: x\r\n{extra}\r\n").as_bytes())
            .unwrap()
            .unwrap()
    };
    assert_eq!(http::framing(&head("")), http::Framing::None);
    assert_eq!(
        http::framing(&head("Content-Length: 12\r\n")),
        http::Framing::Length(12)
    );
    assert_eq!(
        http::framing(&head("Transfer-Encoding: chunked\r\n")),
        http::Framing::Chunked
    );
    for bad in [
        "Content-Length: 12\r\nTransfer-Encoding: chunked\r\n",
        "Content-Length: 12\r\nContent-Length: 12\r\n",
        "Content-Length: +12\r\n",
        "Content-Length: 1 2\r\n",
        "Transfer-Encoding: gzip, chunked\r\n",
    ] {
        assert_eq!(http::framing(&head(bad)), http::Framing::Bad, "{bad}");
    }
}

// --- The page -----------------------------------------------------------------------------------------------------

#[test]
fn the_page_has_the_spaces_and_chat_screens_and_no_inline_script() {
    let w = world("page");
    let running = serve(w.site());
    let (_, head, index) = get(running.addr, "/", OWNER);
    assert!(head.contains(&format!("Content-Security-Policy: {}", routes::CSP)));
    assert!(!routes::CSP.contains("unsafe-inline") && routes::CSP.contains("default-src 'self'"));
    for script in ["/app.js", "/pcs.js", "/spaces.js", "/chat.js"] {
        assert!(
            index.contains(&format!(r#"<script src="{script}"></script>"#)),
            "{script}"
        );
    }
    assert!(!index.contains("<script>"));
    let (_, _, spaces) = get(running.addr, "/spaces.js", OWNER);
    assert!(spaces.contains("nv.screen('spaces'") && spaces.contains("nv.stream('/api/events'"));
    let (_, _, chat) = get(running.addr, "/chat.js", OWNER);
    assert!(chat.contains("nv.screen('chat'"));
    assert!(
        chat.contains("accept: 'image/*'"),
        "the photo library on iPhone"
    );
    assert!(
        chat.contains("addEventListener('paste'"),
        "a pasted screenshot"
    );
    assert!(
        chat.contains("MOST = 50 * 1024 * 1024"),
        "refused before sending"
    );
    let (_, _, app) = get(running.addr, "/app.js", OWNER);
    assert!(
        // The live view's own listener: P39o's error screen has another, which a bare substring also matched.
        app.contains("    document.addEventListener('visibilitychange', shown);\n")
            && app.contains("EventSource.CLOSED"),
        "back from the background, and a restart"
    );
    for (name, text) in [("spaces", &spaces), ("chat", &chat), ("app", &app)] {
        for markup in [".innerHTML", ".outerHTML", "insertAdjacentHTML"] {
            assert!(!text.contains(markup), "{name} parses markup with {markup}");
        }
        assert!(
            !text.contains("style="),
            "{name}: the CSP has no inline style"
        );
    }
    let (_, _, css) = get(running.addr, "/app.css", OWNER);
    assert!(css.contains("@media (prefers-reduced-motion: reduce)"));
    assert!(css.contains(":where(.nv) button"));
}

// --- Review round (record p39b, "Review round") ------------------------------------------------------------------

/// The folder's "Open Nirvana here" session record, naming `w.path`; its home.
fn with_record(w: &World) -> PathBuf {
    let home = w.root.join("home");
    let record = crate::nirvana_launch::session_record_path(&home, &w.folder);
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(
        &record,
        json!({"session_id": SESSION, "at": "2026-11-04T10:00:00Z", "transcript": w.path})
            .to_string(),
    )
    .unwrap();
    home
}

/// `agent` with herdr's label `label` and the kind token `token` (none: removed).
fn kind(mut agent: Value, label: &str, token: Option<&str>) -> Value {
    agent["agent"] = json!(label);
    match token {
        Some(token) => agent["tokens"][crate::nirvana_spaces::KIND_TOKEN] = json!(token),
        None => {
            agent["tokens"]
                .as_object_mut()
                .unwrap()
                .remove(crate::nirvana_spaces::KIND_TOKEN);
        }
    }
    agent
}

#[test]
fn the_folders_record_is_a_lone_claudes_only() {
    let w = world("whose");
    let home = with_record(&w);
    let claude = agent("p1", "w1", "nirvana", "idle", &w.folder);
    let read = |agent: &Value, others: &[Value]| {
        chat::transcript_for(agent, others, Some(&w.claude), Some(&home))
    };
    assert_eq!(read(&claude, &[]), Some(w.path.clone()), "a lone Claude");
    assert_eq!(
        read(&kind(claude.clone(), "claude", None), &[]),
        Some(w.path.clone()),
        "a Claude before its probe: herdr's label"
    );
    for (label, token) in [
        ("codex", Some("codex")),
        ("antigravity", Some("antigravity")),
        ("codex", None),
        ("claude", Some("codex")),
    ] {
        assert_eq!(
            read(&kind(claude.clone(), label, token), &[]),
            None,
            "{label} {token:?} is not a Claude"
        );
    }
    // A second Claude in the folder: the record could be either one's.
    let twin = agent("p7", "w1", "second", "idle", &w.folder);
    assert_eq!(
        read(&claude, std::slice::from_ref(&twin)),
        None,
        "never the other's"
    );
    // A Codex beside it, or a Claude in another folder, takes nothing from it.
    let codex = kind(
        agent("p8", "w1", "codex", "idle", &w.folder),
        "codex",
        Some("codex"),
    );
    let elsewhere = w.root.join("other");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let far = agent("p6", "w1", "far", "idle", &elsewhere);
    assert_eq!(read(&claude, &[codex, far]), Some(w.path.clone()));
    // Over HTTP: a Codex agent in that folder gets the screen and no bubbles.
    w.herdr.agents.lock().unwrap()[1] = kind(
        agent("p2", "w2", "flash", "idle", &w.folder),
        "codex",
        Some("codex"),
    );
    w.herdr.agents.lock().unwrap()[0]["tokens"]
        .as_object_mut()
        .unwrap()
        .remove(crate::nirvana_spaces::TRANSCRIPT_TOKEN);
    let running = serve(site_with(
        w.herdr.clone(),
        Some(w.claude.clone()),
        Some(home.clone()),
    ));
    let (_, _, body) = get(running.addr, "/api/chat?agent=p2", OWNER);
    let codex: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(codex["transcript"], false, "{body}");
    assert_eq!(codex["messages"], json!([]));
    assert!(
        codex["live"].as_str().unwrap().contains("Gating"),
        "the live screen"
    );
    // p1, a Claude with no token now, shares the folder with the Claudes p3 and p4: no record for it either.
    let (_, _, body) = get(running.addr, "/api/chat?agent=p1", OWNER);
    let claude: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(claude["messages"], json!([]), "{body}");
}

#[test]
fn phone_text_reaches_agent_prompt_without_controls() {
    let w = world("controls");
    let running = serve(w.site());
    let (status, answer) = send_json(
        running.addr,
        json!({"agent": "p2", "text": "look\u{1b}[201~\rrm -rf x\u{1b} and\u{7f}\u{9b}2J\u{7}\ttab\nnext"}),
    );
    assert_eq!(status, 200, "{answer}");
    let typed = w.herdr.calls("agent.prompt")[0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(typed, "look[201~rm -rf x and2J\ttab\nnext");
    assert!(
        !typed
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t'),
        "{typed:?}"
    );
    // A lone ESC is nothing to send.
    assert_eq!(
        send_json(running.addr, json!({"agent": "p2", "text": "\u{1b}"})).0,
        400
    );
    assert_eq!(w.herdr.calls("agent.prompt").len(), 1);
    // A file's path follows after a space, quoted when the folder has one.
    assert_eq!(
        send::message(
            "see",
            &[
                PathBuf::from("C:\\3D-Printer Flow\\.nirvana\\uploads\\phone-1.png"),
                PathBuf::from("C:\\x\\phone-2.png")
            ]
        ),
        "see \"C:\\3D-Printer Flow\\.nirvana\\uploads\\phone-1.png\" C:\\x\\phone-2.png"
    );
}

#[test]
fn a_change_between_the_snapshot_and_the_subscription_is_seen() {
    let w = world("between");
    *w.herdr.on_subscribe.lock().unwrap() = Some(("p2".into(), "working".into()));
    let feed = spaces::Feed::new(w.herdr.clone());
    let lists = feed.listen().unwrap();
    let first: Value =
        serde_json::from_str(&lists.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
    assert_eq!(first["spaces"][1]["agents"][0]["status"], "idle");
    // No event is ever emitted: the snapshot taken after the subscription carries it.
    let next: Value = serde_json::from_str(
        &lists
            .recv_timeout(Duration::from_secs(3))
            .expect("the change"),
    )
    .unwrap();
    assert_eq!(next["spaces"][1]["agents"][0]["status"], "working");
}

#[test]
fn a_stream_that_ends_at_once_is_not_subscribed_in_a_loop() {
    let w = world("spin");
    w.herdr.ends_at_once.store(true, Ordering::SeqCst);
    let feed = spaces::Feed::new(w.herdr.clone());
    let _lists = feed.listen().unwrap();
    std::thread::sleep(Duration::from_millis(3200));
    // Waits of 0.5, 1 and 2 s: subscriptions at 0, 0.5 and 1.5 s. A wait reset on the ack makes seven; none, hundreds.
    let count = w.herdr.subscribed().len();
    assert!((2..=4).contains(&count), "{count} subscriptions in 3.2 s");
}

#[test]
fn an_old_stream_ends_when_the_feed_subscribes_again() {
    let w = world("old-streams");
    let feed = spaces::Feed::new(w.herdr.clone());
    let _lists = feed.listen().unwrap();
    for (round, pane) in ["p11", "p12", "p13"].into_iter().enumerate() {
        let started = Instant::now();
        while w.herdr.subscribed().len() < round + 1 && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        w.herdr
            .agents
            .lock()
            .unwrap()
            .push(agent(pane, "w1", "worker", "working", &w.folder));
        w.herdr.emit(json!({"event": "pane.agent_detected"}));
    }
    let started = Instant::now();
    while w.herdr.subscribed().len() < 4 && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(w.herdr.subscribed().len(), 4);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        w.herdr.readers.load(Ordering::SeqCst),
        1,
        "only the newest stream is still read"
    );
}

/// A socket server at a temp path that reads one request line and then, if `ack`, says it started; it never says
/// more, and holds the connection until `hold` is dropped.
fn silent_socket(
    name: &str,
    ack: bool,
) -> (PathBuf, mpsc::Sender<()>, std::thread::JoinHandle<()>) {
    #[cfg(unix)]
    use interprocess::local_socket::traits::Listener as _;
    use std::io::BufRead;
    let path = std::env::temp_dir().join(format!("p39b-{name}-{}.sock", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let listener = crate::ipc::bind_private_local_listener(&path).unwrap();
    let (hold, held) = mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let stream = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if ack {
            let id = serde_json::from_str::<Value>(&line).unwrap()["id"].clone();
            let ack = json!({"id": id, "result": {"type": "subscription_started"}});
            reader
                .get_mut()
                .write_all(format!("{ack}\n").as_bytes())
                .unwrap();
            reader.get_mut().flush().unwrap();
        }
        let _ = held.recv();
    });
    (path, hold, server)
}

#[test]
fn a_socket_that_never_answers_is_given_up_on() {
    let (path, hold, server) = silent_socket("silent", false);
    let started = Instant::now();
    let answer = super::herdr::call_socket_at(
        &path,
        1,
        "agent.wait",
        json!({"target": "p1", "until": ["idle"], "timeout_ms": 200}),
    );
    let took = started.elapsed();
    assert!(answer.is_err(), "{answer:?}");
    let wait = Duration::from_millis(200) + super::herdr::MARGIN;
    assert!(
        took + Duration::from_millis(50) >= wait && took < wait + Duration::from_secs(2),
        "the wait plus its margin, not for good: {took:?}"
    );
    drop(hold);
    server.join().unwrap();
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_subscription_told_to_stop_ends_its_reader() {
    let (path, hold, server) = silent_socket("stop", true);
    let stop = Arc::new(AtomicBool::new(false));
    let mut events = super::herdr::subscribe_at(
        &path,
        1,
        json!([{"type": "workspace.created"}]),
        stop.clone(),
    )
    .expect("the ack");
    let (done, ended) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let _ = done.send(events.next().is_none());
    });
    assert!(
        ended.recv_timeout(Duration::from_millis(300)).is_err(),
        "still reading a quiet stream"
    );
    stop.store(true, Ordering::SeqCst);
    assert_eq!(
        ended.recv_timeout(Duration::from_secs(1)),
        Ok(true),
        "ended once told to stop"
    );
    reader.join().unwrap();
    drop(hold);
    server.join().unwrap();
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_live_view_ends_when_the_owner_or_this_pc_changes() {
    let w = world("recheck");
    let site = w.site();
    let running = serve(site.clone());
    let mut chat = View::open(running.addr, "/api/chat/events?agent=p1");
    assert!(chat.next("chat", Duration::from_secs(5)).is_some());
    let mut spaces = View::open(running.addr, "/api/events");
    assert!(spaces.next("spaces", Duration::from_secs(5)).is_some());
    assert!(
        !chat.closes(Duration::from_millis(1500)),
        "open while nothing changed"
    );
    *site.owner.write().unwrap() = Some("someone@example.com".into());
    assert!(chat.closes(Duration::from_secs(3)), "another login");
    assert!(spaces.closes(Duration::from_secs(3)));
    *site.owner.write().unwrap() = Some(OWNER.into());
    let mut again = View::open(running.addr, "/api/events");
    assert!(again.next("spaces", Duration::from_secs(5)).is_some());
    *site.me.write().unwrap() = None;
    assert!(
        again.closes(Duration::from_secs(3)),
        "Tailscale no longer says who this PC is"
    );
}

#[test]
fn a_cross_site_read_of_the_api_gets_nothing() {
    let w = world("cross-read");
    let site = w.site();
    let running = serve(site.clone());
    let read = |path: &str, extra: &str| {
        raw(
            running.addr,
            format!("{}{extra}\r\n", proxied("GET", path, OWNER)).as_bytes(),
        )
    };
    for path in [
        "/api/spaces",
        "/api/events",
        "/api/chat?agent=p1",
        "/api/chat/events?agent=p1",
        "/api/hello",
    ] {
        for extra in [
            "Sec-Fetch-Site: cross-site\r\n",
            "Sec-Fetch-Site: same-site\r\n",
            "Origin: https://evil.example\r\n",
        ] {
            let (status, _, body) = read(path, extra);
            assert_eq!((status, body.as_str()), (403, REFUSED), "{path} {extra:?}");
        }
    }
    assert!(w.herdr.methods().is_empty(), "{:?}", w.herdr.methods());
    assert_eq!(site.streams.count(), 0, "no slot taken");
    // The page's own reads: same-origin, a typed one, a matching Origin, and a browser that sends neither.
    let matching = format!("Sec-Fetch-Site: same-origin\r\nOrigin: https://{HOST}\r\n");
    for extra in [
        "Sec-Fetch-Site: same-origin\r\n",
        "Sec-Fetch-Site: none\r\n",
        matching.as_str(),
        "",
    ] {
        assert_eq!(read("/api/spaces", extra).0, 200, "{extra:?}");
    }
    // The page itself opens from a link on another site.
    assert_eq!(read("/", "Sec-Fetch-Site: cross-site\r\n").0, 200);
}

#[test]
fn a_413_on_the_head_reaches_a_caller_that_sends_its_body_first() {
    let w = world("early-413");
    let running = serve(w.site());
    let mut stream = TcpStream::connect(running.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(upload_head(Some(send::UPLOAD_MOST + 1)).as_bytes())
        .unwrap();
    // As a client that writes before it reads: 4 MiB of the body, then it stops.
    let mib = vec![b'q'; 1024 * 1024];
    for _ in 0..4 {
        if stream.write_all(&mib).is_err() {
            break;
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut answer = Vec::new();
    let read = stream.read_to_end(&mut answer);
    let answer = String::from_utf8_lossy(&answer);
    assert!(read.is_ok(), "{read:?} after {answer:?}");
    assert!(answer.starts_with("HTTP/1.1 413"), "{answer}");
    assert!(w.uploaded().is_empty());
}

#[test]
fn a_chunk_size_is_plain_hex() {
    let w = world("chunk-hex");
    let running = serve(w.site());
    for (size, status) in [("+1", 400), (" 1", 400), ("1 ;name=v", 200), ("1", 200)] {
        let body = format!("{size}\r\nx\r\n0\r\n\r\n");
        let (got, text) = upload_streamed(running.addr, upload_head(None), move |out| {
            let _ = out.write_all(body.as_bytes());
        });
        assert_eq!(got, status, "{size:?}: {text}");
    }
    assert_eq!(
        w.uploaded().len(),
        2,
        "only the two plain sizes left a file"
    );
}

#[test]
fn the_page_ends_a_gone_view_and_waits_out_composition() {
    let w = world("page-round");
    let running = serve(w.site());
    let (_, _, app) = get(running.addr, "/app.js", OWNER);
    let (_, _, chat) = get(running.addr, "/chat.js", OWNER);
    assert!(
        app.contains("if (!over) on[name](data, close);"),
        "a handler can end its view"
    );
    assert!(
        chat.contains(
            "gone: (_, close) => { close(); note.textContent = 'This agent has closed on the PC.'; },"
        ),
        "a closed agent's view ends"
    );
    assert!(
        app.contains("Date.now() - opened >= 1000"),
        "one reopen on a wake"
    );
    assert!(
        chat.contains("event.isComposing || event.keyCode === 229"),
        "IME Enter"
    );
}

/// The owner, 2026-11-09: after leaving the screen the one-line box showed only the start of the draft and did not grow.
/// The box is a textarea that fits its text on every change and when the screen comes back, and a return to it puts
/// the caret at the end. The layout itself was measured in headless Chromium and WebKit only (record p39g).
#[test]
fn the_message_box_grows_with_its_text_and_comes_back_at_the_end() {
    let w = world("grow");
    let running = serve(w.site());
    let (_, _, chat) = get(running.addr, "/chat.js", OWNER);
    let (_, _, css) = get(running.addr, "/app.css", OWNER);
    assert!(chat.contains("h('textarea', { id: 'nv-draft'"), "a box that wraps");
    assert!(
        chat.contains("input.style.height = Math.min(input.scrollHeight, DRAFT_MOST) + 'px';"),
        "the box fits its text"
    );
    assert!(
        chat.contains("input.addEventListener('input', () => { fitDraft(); drawPending(); });"),
        "fit on typing"
    );
    assert!(
        chat.contains("document.addEventListener('visibilitychange', refit);"),
        "fit on coming back"
    );
    assert!(
        chat.contains("input.addEventListener('focus', () => requestAnimationFrame(toEnd));"),
        "the caret goes to the end"
    );
    assert!(css.contains("resize: none; overflow-y: hidden;"), "no grab handle");
}

#[test]
fn a_chat_view_takes_the_transcript_that_comes_with_the_first_prompt() {
    let w = world("first-prompt");
    let token = crate::nirvana_spaces::TRANSCRIPT_TOKEN;
    w.herdr.agents.lock().unwrap()[0]["tokens"]
        .as_object_mut()
        .unwrap()
        .remove(token);
    let running = serve(w.site());
    let mut view = View::open(running.addr, "/api/chat/events?agent=p1");
    let first = view.next("chat", Duration::from_secs(5)).expect("the chat");
    assert_eq!(first["transcript"], false);
    let started = Instant::now();
    while w.herdr.subscribed().is_empty() && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    // The probe sets the token as the agent's state moves.
    w.herdr.agents.lock().unwrap()[0]["tokens"][token] = json!(w.path.display().to_string());
    w.herdr.set_status("p1", "done");
    w.herdr
        .emit(json!({"event": "pane.agent_status_changed", "data": {"pane_id": "p1"}}));
    let whole = view
        .next("chat", Duration::from_secs(5))
        .expect("the whole chat again, from its file");
    assert_eq!(whole["transcript"], true);
    assert_eq!(whole["messages"].as_array().unwrap().len(), 4);
    assert_eq!(whole["agent"]["status"], "done");
}

/// P39c item 6. The page's scripts read in parallel against one server, as a phone's first load and a test run under
/// a parallel build both do: every answer is read whole (`raw` fails on a read error or a short body) and is the
/// asset byte for byte. 384 connections, so the run does not press on this PC's ephemeral ports (a probe of 30 000
/// in a few seconds had `connect` fail with WSAENOBUFS, os error 10055; record p39c).
#[test]
fn asset_reads_in_parallel_arrive_whole() {
    let w = world("assets-parallel");
    let running = serve(w.site());
    let addr = running.addr;
    let asset = |path: &str| {
        routes::ASSETS
            .iter()
            .find(|(at, _, _)| *at == path)
            .map(|(_, _, body)| (*body).to_string())
            .unwrap()
    };
    let (app, chat) = (asset("/app.js"), asset("/chat.js"));
    let threads: Vec<_> = (0..16)
        .map(|thread| {
            let (app, chat) = (app.clone(), chat.clone());
            std::thread::spawn(move || {
                for i in 0..24 {
                    let (path, want) = if (thread + i) % 2 == 0 {
                        ("/app.js", &app)
                    } else {
                        ("/chat.js", &chat)
                    };
                    let (status, _, body) = get(addr, path, OWNER);
                    assert_eq!(status, 200, "{path}");
                    assert!(
                        body == *want,
                        "{path}: {} of {} bytes",
                        body.len(),
                        want.len()
                    );
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("every read whole");
    }
}
