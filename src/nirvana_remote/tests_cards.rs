//! Plan 39 P39d's tests: the new-space picker and the cards. Everything runs against fakes: a fake herdr socket API
//! (its lists, `command.invoke`'s answer, and a log of every call), a stand-in for the verb's preset, a scratch
//! folder under the private TEMP standing in for `Desktop\Pojects`, a Tailscale that is never asked and a peer check
//! that admits. No test opens a real space, starts a herdr server or Claude, or reads the owner's profile folders.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::cards;
use super::herdr::Herdr;
use super::http;
use super::pcs::{Hello, Probe};
use super::peer::{PeerCheck, Refusal};
use super::pick::{self, Local};
use super::routes::{self, Site};
use super::tailscale::{Held, Tailscale};
use crate::nirvana_running::{Activity, Tool};
use crate::nirvana_usage::{Cache, Reading, Source};

const OWNER: &str = "owner@example.com";
const HOST: &str = "desk.example-tail.ts.net:8443";
const PHONE: &str = "100.101.102.103";
const DESK_IP: &str = "100.64.0.1";
const REFUSED: &str = "Only the Tailscale account this PC is signed in with can connect.";

// --- Fakes ------------------------------------------------------------------------------------------------------

struct NoTailscale;

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

struct Admit;

impl PeerCheck for Admit {
    fn check(&self, _peer: SocketAddr, _local: SocketAddr) -> Result<(), Refusal> {
        Ok(())
    }
}

struct NoProbe;

impl Probe for NoProbe {
    fn hello(&self, _dns: &str, _port: u16) -> Option<Hello> {
        None
    }
}

/// herdr as P39d's routes see it. `log` is shared with the preset stand-in, so the order of the two is seen.
struct FakeHerdr {
    panes: Mutex<Vec<Value>>,
    log: Arc<Mutex<Vec<String>>>,
    calls: Mutex<Vec<(String, Value)>>,
}

impl FakeHerdr {
    fn new(log: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
        Arc::new(Self {
            panes: Mutex::default(),
            log,
            calls: Mutex::default(),
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
}

fn agent(pane: &str, space: &str, name: &str, status: &str) -> Value {
    json!({
        "pane_id": pane, "workspace_id": space, "name": name, "agent": "claude",
        "title": "\u{2733} Gating two branches", "agent_status": status,
        "tokens": {"nirvana_model": "Claude Opus 5.5", "nirvana_kind": "claude"},
    })
}

fn workspaces() -> Value {
    json!({"type": "workspace_list", "workspaces": [
        {"workspace_id": "w1", "label": "Nirvana-Herdr", "number": 1},
        {"workspace_id": "w2", "label": "3D-Printer Flow", "number": 2},
    ]})
}

fn agents() -> Value {
    json!({"type": "agent_list", "agents": [
        agent("p1", "w1", "nirvana", "working"),
        agent("p2", "w2", "flash", "idle"),
        agent("p3", "w2", "printer", "blocked"),
        agent("p4", "w2", "reel", "done"),
        agent("p5", "w1", "flash", "working"),
        agent("p9", "w-gone", "stray", "working"),
    ]})
}

impl Herdr for FakeHerdr {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        self.log.lock().unwrap().push(format!(
            "{method} {}",
            params["command_id"].as_str().unwrap_or("")
        ));
        match method {
            "workspace.list" => Ok(workspaces()),
            "agent.list" => Ok(agents()),
            "pane.list" => Ok(json!({"type": "pane_list", "panes": *self.panes.lock().unwrap()})),
            "command.invoke" => Ok(json!({
                "type": "agent_started",
                "agent": {"pane_id": "p20", "workspace_id": "w9", "name": "nirvana-2"},
                "argv": ["claude", "--plugin-dir", "C:\\secret-looking\\plugin"],
            })),
            other => Err(format!("unexpected {other}")),
        }
    }
}

// --- The world ---------------------------------------------------------------------------------------------------

fn fixed_now() -> time::OffsetDateTime {
    time::Date::from_calendar_date(2026, time::Month::November, 4)
        .unwrap()
        .with_hms(12, 0, 0)
        .unwrap()
        .assume_utc()
}

fn now_unix() -> u64 {
    fixed_now().unix_timestamp() as u64
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p39d-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A junction at `link` to `target`, as `mklink /J` makes one (no privilege needed, unlike a symbolic link).
#[cfg(windows)]
fn junction(link: &Path, target: &Path) {
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(made.status.success(), "mklink /J: {made:?}");
}

struct World {
    /// Stands in for `Desktop\Pojects`.
    root: PathBuf,
    /// A folder outside the root, which a junction in it points to.
    outside: PathBuf,
    home: PathBuf,
    herdr: Arc<FakeHerdr>,
    log: Arc<Mutex<Vec<String>>>,
}

fn world(name: &str) -> World {
    let base = scratch(name);
    let root = base.join("Pojects");
    for folder in [
        "groove-coach",
        "moto-reel-mcp",
        "3D-Printer Flow",
        ".nirvana",
    ] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
    }
    std::fs::write(root.join("notes.txt"), "a file's contents").unwrap();
    std::fs::write(
        root.join("groove-coach").join("secret.txt"),
        "inside a folder",
    )
    .unwrap();
    let outside = base.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    #[cfg(windows)]
    junction(&root.join("out"), &outside);
    let home = base.join("home");
    let log = Arc::new(Mutex::new(Vec::new()));
    World {
        root,
        outside,
        home,
        herdr: FakeHerdr::new(log.clone()),
        log,
    }
}

impl World {
    fn site(&self) -> Arc<Site> {
        let mut site = Site::new(
            Arc::new(NoTailscale),
            self.herdr.clone(),
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
        site.nirvana_home = Some(self.home.clone());
        let log = self.log.clone();
        site.local = Local::with(
            Some(self.root.clone()),
            Arc::new(move |folder: &Path| {
                log.lock()
                    .unwrap()
                    .push(format!("preset {}", folder.display()));
                Some("preset Lean".into())
            }),
            Arc::new(|space: &str| match space {
                "w1" => vec![5173, 8080],
                "w2" => vec![8770],
                _ => Vec::new(),
            }),
        );
        Arc::new(site)
    }

    fn presets(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|line| line.starts_with("preset "))
            .cloned()
            .collect()
    }
}

struct Running {
    rt: tokio::runtime::Runtime,
    addr: SocketAddr,
    stop: tokio::sync::watch::Sender<bool>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.rt
            .block_on(async { tokio::time::sleep(Duration::from_millis(20)).await });
    }
}

fn serve(site: Arc<Site>) -> Running {
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
    let handler: Arc<dyn http::Handler> = site;
    rt.spawn(http::serve(
        listener,
        handler,
        http::Limits::DEFAULT,
        stopped,
    ));
    Running { rt, addr, stop }
}

/// What tailscaled sends for a request from the owner's phone (or `login`), without the closing blank line.
fn proxied(method: &str, path: &str, login: &str) -> String {
    format!("{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: {login}\r\n")
}

fn from_page() -> String {
    format!("Sec-Fetch-Site: same-origin\r\nOrigin: https://{HOST}\r\n")
}

fn raw(addr: SocketAddr, bytes: &[u8]) -> (u16, String, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(bytes).unwrap();
    let mut answer = Vec::new();
    let _ = stream.read_to_end(&mut answer);
    let answer = String::from_utf8_lossy(&answer).to_string();
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    (status, head.to_string(), body.to_string())
}

fn get(addr: SocketAddr, path: &str) -> (u16, String, String) {
    raw(
        addr,
        format!("{}\r\n", proxied("GET", path, OWNER)).as_bytes(),
    )
}

fn get_json(addr: SocketAddr, path: &str) -> (u16, Value) {
    let (status, _, body) = get(addr, path);
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

fn open(addr: SocketAddr, body: &Value) -> (u16, Value) {
    let body = body.to_string();
    let request = format!(
        "{}{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        proxied("POST", "/api/folders/open", OWNER),
        from_page(),
        body.len()
    );
    let (status, _, text) = raw(addr, request.as_bytes());
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn names(folders: &Value) -> Vec<String> {
    let mut names: Vec<String> = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|folder| folder["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

// --- The picker ---------------------------------------------------------------------------------------------------

#[test]
fn the_picker_root_is_desktop_pojects_under_the_users_profile() {
    let profile = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap();
    assert_eq!(pick::root(), Some(profile.join("Desktop").join("Pojects")));
    assert_eq!(pick::root_label(), "Desktop\\Pojects");
}

#[test]
fn the_picker_lists_the_roots_plain_folders_and_nothing_inside_them() {
    let w = world("list");
    let running = serve(w.site());
    let (status, folders) = get_json(running.addr, "/api/folders");
    assert_eq!(status, 200, "{folders}");
    // Not the file, not the hidden folder, not the junction that points out of the root.
    assert_eq!(
        names(&folders),
        vec!["3D-Printer Flow", "groove-coach", "moto-reel-mcp"]
    );
    assert_eq!(folders["root"], "Desktop\\Pojects");
    let text = folders.to_string();
    for leak in [
        "notes.txt",
        "secret.txt",
        "inside a folder",
        "a file's contents",
        "elsewhere",
    ] {
        assert!(!text.contains(leak), "{leak} in {text}");
    }
    assert!(
        !text.contains(&w.root.display().to_string().replace('\\', "\\\\")),
        "the PC's path is not sent: {text}"
    );
}

#[test]
fn a_name_with_a_path_in_it_is_refused_before_the_disk_is_asked() {
    for name in [
        "",
        ".",
        "..",
        "..\\elsewhere",
        "../elsewhere",
        "groove-coach\\..\\..",
        "C:",
        "D:\\work",
        "C:\\Windows",
        "\\\\?\\C:\\Windows",
        "\\\\server\\share",
        "groove-coach:stream",
        "groove-coach::$DATA",
        "CON",
        "con.txt",
        "NUL",
        "com1",
        "LPT9.log",
        "COM\u{b9}",
        "trailing.",
        "trailing ",
        "tab\tname",
    ] {
        assert!(pick::name_ok(name).is_err(), "{name:?} was taken");
    }
    for name in [
        "groove-coach",
        "3D-Printer Flow",
        "Nirvana-Herdr-p39d",
        "console",
        "com10",
        "a.b",
    ] {
        assert_eq!(pick::name_ok(name), Ok(()), "{name:?}");
    }
}

#[test]
fn a_folder_outside_the_root_is_never_opened() {
    let w = world("outside");
    let running = serve(w.site());
    for (name, status) in [
        ("..", 400),
        ("..\\elsewhere", 400),
        ("C:\\Windows", 400),
        ("\\\\?\\C:\\Windows", 400),
        ("\\\\localhost\\C$", 400),
        ("groove-coach:x", 400),
        ("CON", 400),
        ("not-there", 404),
        ("notes.txt", 404),
        (".nirvana", 404),
    ] {
        let (got, answer) = open(running.addr, &json!({ "name": name }));
        assert_eq!(got, status, "{name}: {answer}");
    }
    // The junction to a folder outside: not listed, so not one of the names the server opens.
    #[cfg(windows)]
    {
        let (got, answer) = open(running.addr, &json!({ "name": "out" }));
        assert_eq!(got, 404, "the junction: {answer}");
    }
    assert!(w.herdr.methods().is_empty(), "{:?}", w.herdr.methods());
    assert!(w.presets().is_empty());
    let _ = &w.outside;
}

#[cfg(windows)]
#[test]
fn a_junction_is_not_a_plain_folder_and_resolves_outside_the_root() {
    let w = world("junction");
    let link = w.root.join("out");
    assert!(link.is_dir(), "a junction reads as a folder to is_dir");
    assert!(!pick::plain_dir(&link));
    assert!(pick::plain_dir(&w.root.join("groove-coach")));
    assert!(!pick::inside(&w.root, &link), "its target is outside");
    assert!(pick::inside(&w.root, &w.root.join("groove-coach")));
    assert!(!pick::inside(&w.root, &w.root));
    assert!(!pick::inside(
        &w.root,
        &w.root.join("groove-coach").join("..").join("..")
    ));
}

#[test]
fn opening_a_folder_starts_it_as_open_nirvana_here_does() {
    let w = world("open");
    *w.herdr.panes.lock().unwrap() = vec![
        json!({"pane_id": "p1", "workspace_id": "w1", "cwd": w.root.join("moto-reel-mcp").display().to_string()}),
    ];
    let running = serve(w.site());
    let (status, answer) = open(running.addr, &json!({"name": "groove-coach"}));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["space"], "w9");
    assert_eq!(answer["existing"], false);
    assert_eq!(answer["preset"], "preset Lean");
    assert!(
        !answer.to_string().contains("plugin"),
        "herdr's argv stays on the PC: {answer}"
    );
    let folder = w.root.join("groove-coach");
    let invoked = w.herdr.calls("command.invoke");
    assert_eq!(
        invoked,
        vec![json!({ "command_id": crate::nirvana_launch::open_command_id(&folder, false, true) })]
    );
    assert_eq!(
        invoked[0]["command_id"],
        format!("nirvana.open:new:{}", folder.display())
    );
    // The verb's order: the folder's preset, then the open.
    let log = w.log.lock().unwrap().clone();
    let preset = log
        .iter()
        .position(|line| *line == format!("preset {}", folder.display()))
        .expect("the preset was applied");
    let start = log
        .iter()
        .position(|line| line.starts_with("command.invoke "))
        .unwrap();
    assert!(preset < start, "{log:?}");
}

#[test]
fn a_folder_already_open_names_its_space_and_starts_nothing() {
    let w = world("existing");
    // As herdr reports a pane: its own case and a trailing separator, which `folder_key` reads as the same folder.
    let cwd = format!("{}\\", w.root.join("GROOVE-COACH").display());
    *w.herdr.panes.lock().unwrap() =
        vec![json!({"pane_id": "p7", "workspace_id": "w2", "cwd": cwd})];
    let running = serve(w.site());
    let (_, folders) = get_json(running.addr, "/api/folders");
    let groove = folders["folders"]
        .as_array()
        .unwrap()
        .iter()
        .find(|folder| folder["name"] == "groove-coach")
        .unwrap()
        .clone();
    assert_eq!(groove["open"], true);
    assert_eq!(groove["space"], "w2");
    let (status, answer) = open(running.addr, &json!({"name": "groove-coach"}));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer, json!({"space": "w2", "existing": true}));
    assert!(w.herdr.calls("command.invoke").is_empty());
    assert!(
        w.presets().is_empty(),
        "no preset for a folder already open"
    );
}

#[test]
fn the_open_is_a_post_of_json_from_this_page() {
    let w = world("post");
    let running = serve(w.site());
    let (status, _, _) = get(running.addr, "/api/folders/open");
    assert_eq!(status, 405);
    let body = r#"{"name":"groove-coach"}"#;
    let plain = format!(
        "{}{}Content-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
        proxied("POST", "/api/folders/open", OWNER),
        from_page(),
        body.len()
    );
    assert_eq!(raw(running.addr, plain.as_bytes()).0, 415);
    let (status, _) = open(running.addr, &json!({"path": "C:\\Windows"}));
    assert_eq!(status, 400, "a path field is not a name");
    let (status, _, _) = raw(
        running.addr,
        format!("{}\r\n", proxied("POST", "/api/cards/usage", OWNER)).as_bytes(),
    );
    assert_eq!(
        status, 403,
        "no Sec-Fetch-Site or Origin: refused before the route"
    );
    let posted = format!(
        "{}{}Content-Length: 0\r\n\r\n",
        proxied("POST", "/api/cards/usage", OWNER),
        from_page()
    );
    assert_eq!(
        raw(running.addr, posted.as_bytes()).0,
        405,
        "the cards are read-only"
    );
    assert!(w.herdr.calls("command.invoke").is_empty());
}

#[test]
fn a_refused_caller_gets_nothing_from_the_picker_or_the_cards() {
    let w = world("refused");
    let running = serve(w.site());
    let body = r#"{"name":"groove-coach"}"#;
    let reads = [
        "/api/folders",
        "/api/cards/usage",
        "/api/cards/servers",
        "/api/cards/running",
    ];
    for login in ["friend@example.com", ""] {
        for path in reads {
            let (status, _, text) = raw(
                running.addr,
                format!("{}\r\n", proxied("GET", path, login)).as_bytes(),
            );
            assert_eq!(
                (status, text.as_str()),
                (403, REFUSED),
                "{path} for {login:?}"
            );
        }
        let post = format!(
            "{}{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            proxied("POST", "/api/folders/open", login),
            from_page(),
            body.len()
        );
        let (status, _, text) = raw(running.addr, post.as_bytes());
        assert_eq!(
            (status, text.as_str()),
            (403, REFUSED),
            "open for {login:?}"
        );
    }
    // The owner's login from this PC's own address.
    let local = format!(
        "GET /api/folders HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {DESK_IP}\r\nTailscale-User-Login: {OWNER}\r\n\r\n"
    );
    assert_eq!(raw(running.addr, local.as_bytes()).0, 403);
    // Another name than this PC's.
    let rebound = format!(
        "GET /api/cards/running HTTP/1.1\r\nHost: evil.example:8443\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: {OWNER}\r\n\r\n"
    );
    assert_eq!(raw(running.addr, rebound.as_bytes()).0, 421);
    // Another site's page on the owner's phone: a cross-site open, an open with neither header, a cross-site read.
    let cross = format!(
        "{}Sec-Fetch-Site: cross-site\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        proxied("POST", "/api/folders/open", OWNER),
        body.len()
    );
    assert_eq!(raw(running.addr, cross.as_bytes()).0, 403);
    let bare = format!(
        "{}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        proxied("POST", "/api/folders/open", OWNER),
        body.len()
    );
    assert_eq!(
        raw(running.addr, bare.as_bytes()).0,
        403,
        "no Sec-Fetch-Site, no Origin"
    );
    let other_origin = format!(
        "{}Origin: https://evil.example\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        proxied("POST", "/api/folders/open", OWNER),
        body.len()
    );
    assert_eq!(raw(running.addr, other_origin.as_bytes()).0, 403);
    let cross_read = format!(
        "{}Sec-Fetch-Site: cross-site\r\n\r\n",
        proxied("GET", "/api/folders", OWNER)
    );
    assert_eq!(raw(running.addr, cross_read.as_bytes()).0, 403);
    assert!(
        w.herdr.methods().is_empty(),
        "herdr was not asked a thing: {:?}",
        w.herdr.methods()
    );
    assert!(w.presets().is_empty());
}

// --- The cards ----------------------------------------------------------------------------------------------------

fn reading(pool: &str, window: &str, used: f32, read_at: u64) -> Reading {
    Reading {
        pool: pool.into(),
        window: window.into(),
        used: Some(used),
        resets_at: Some(read_at + 3_600),
        resets_text: Some("Nov 4, 1:00pm (America/New_York)".into()),
        read_at,
        source: "claude /usage".into(),
    }
}

/// The cache file as the window's readers write it: Claude's two windows, Gemini's and Argon's (a hidden pool).
fn write_cache(home: &Path) {
    let at = now_unix() - 60;
    let mut cache = Cache {
        version: 1,
        ..Cache::default()
    };
    cache.sources.insert(
        "claude".into(),
        Source {
            read_at: Some(at),
            readings: vec![
                reading("max", "5h", 0.42, at),
                reading("max", "week", 0.65, at),
            ],
            ..Source::default()
        },
    );
    cache.sources.insert(
        "antigravity".into(),
        Source {
            read_at: Some(at),
            readings: vec![
                reading("gemini", "5h", 0.18, at),
                reading("gemini", "week", 0.02, at),
                reading("argon", "week", 0.5, at),
            ],
            ..Source::default()
        },
    );
    let dir = home.join(crate::nirvana_usage::CACHE_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(crate::nirvana_usage::CACHE_FILE),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();
}

#[test]
fn the_usage_card_is_the_foots_rows_from_the_shared_cache() {
    let w = world("usage");
    write_cache(&w.home);
    let running = serve(w.site());
    let (status, card) = get_json(running.addr, "/api/cards/usage");
    assert_eq!(status, 200, "{card}");
    let rows = card["rows"].as_array().unwrap();
    let labels: Vec<&str> = rows
        .iter()
        .map(|row| row["label"].as_str().unwrap())
        .collect();
    // The foot's pools in its order, Argon and Google's Claude pool left out (`FOOT_HIDDEN`).
    assert_eq!(labels, vec!["Claude", "Gemini", "Codex", "Local"]);
    let claude = &rows[0];
    assert_eq!(claude["provider"], "claude");
    let windows = claude["windows"].as_array().unwrap();
    assert_eq!(windows[0]["window"], "5h");
    assert!((windows[0]["used"].as_f64().unwrap() - 0.42).abs() < 1e-6);
    assert_eq!(windows[1]["window"], "week");
    assert_eq!(windows[0]["resets_in"], 3_540);
    assert_eq!(windows[0]["stale"], false);
    assert!(
        (claude["fill"].as_f64().unwrap() - 0.65).abs() < 1e-6,
        "the tightest window fills the bar"
    );
    assert!((rows[1]["windows"][0]["used"].as_f64().unwrap() - 0.18).abs() < 1e-6);
    // Codex and Local were never read: the foot's own words.
    assert_eq!(rows[2]["word"], "reading");
    let text = card.to_string();
    for kept in ["claude /usage", "America/New_York", "argon"] {
        assert!(!text.contains(kept), "{kept} left the PC: {text}");
    }
    // The same rows the PC's foot draws from the same sources.
    let cache = crate::nirvana_usage::read_cache(
        &w.home
            .join(crate::nirvana_usage::CACHE_DIR)
            .join(crate::nirvana_usage::CACHE_FILE),
    );
    let foot: Vec<String> = crate::nirvana_usage::foot_rows(&cards::sources(&cache), now_unix())
        .into_iter()
        .filter(|row| !crate::nirvana_usage::FOOT_HIDDEN.contains(&row.pool.as_str()))
        .map(|row| row.label().to_string())
        .collect();
    assert_eq!(foot, labels);
}

#[test]
fn the_usage_card_with_no_cache_says_reading() {
    let w = world("usage-none");
    let running = serve(w.site());
    let (status, card) = get_json(running.addr, "/api/cards/usage");
    assert_eq!(status, 200);
    assert!(card["rows"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["word"] == "reading"));
}

#[test]
fn the_servers_card_leads_with_remote_then_each_spaces_ports() {
    let w = world("servers");
    let running = serve(w.site());
    let (status, card) = get_json(running.addr, "/api/cards/servers");
    assert_eq!(status, 200, "{card}");
    assert_eq!(
        card,
        json!({
            "remote": {"name": "remote", "port": super::HTTPS_PORT},
            "up": 3,
            "servers": [
                {"space": "Nirvana-Herdr", "port": 5173},
                {"space": "Nirvana-Herdr", "port": 8080},
                {"space": "3D-Printer Flow", "port": 8770},
            ],
        })
    );
}

#[test]
fn the_running_card_counts_agents_at_work_in_the_squares_words() {
    let activity = |agent: &Value| {
        (agent["pane_id"] == "p1").then(|| Activity {
            now: Some(Tool {
                name: "Bash".into(),
                detail: "cargo nextest run --secret-flag".into(),
                since: None,
            }),
            ..Activity::default()
        })
    };
    let card = cards::running_card(&workspaces(), &agents(), &activity);
    assert_eq!(card["working"], 2, "{card}");
    assert_eq!(card["needs"], 1);
    assert_eq!(
        card["agents"],
        json!([
            {"id": "p1", "name": "nirvana", "space": "Nirvana-Herdr", "status": "working", "words": "working \u{b7} Bash"},
            {"id": "p3", "name": "printer", "space": "3D-Printer Flow", "status": "needs", "words": "waits on you"},
            {"id": "p5", "name": "flash", "space": "Nirvana-Herdr", "status": "working", "words": "working"},
        ])
    );
    assert!(
        !card.to_string().contains("secret-flag"),
        "a tool's name only"
    );
    assert_eq!(
        cards::running_words("working", None).as_deref(),
        Some("working")
    );
    assert_eq!(cards::running_words("idle", None), None);
    assert_eq!(cards::running_words("done", None), None);
}

#[test]
fn the_running_card_reads_only_a_transcript_under_claudes_folder() {
    let base = scratch("running-token");
    let claude = base.join("claude");
    let inside = claude.join("projects").join("C--work");
    std::fs::create_dir_all(&inside).unwrap();
    let own = inside.join("6f1c2a3b-0000-4000-8000-000000000001.jsonl");
    std::fs::write(&own, "").unwrap();
    let elsewhere = base.join("elsewhere.jsonl");
    std::fs::write(&elsewhere, "").unwrap();
    let read: Mutex<Vec<PathBuf>> = Mutex::default();
    let reader = |path: &Path| {
        read.lock().unwrap().push(path.to_path_buf());
        None
    };
    for path in [&own, &elsewhere] {
        let mut agent = agent("p1", "w1", "nirvana", "working");
        agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN] =
            json!(path.display().to_string());
        let _ = cards::activity_of(&agent, Some(&claude), &reader);
    }
    let _ = cards::activity_of(&agent("p2", "w1", "x", "working"), Some(&claude), &reader);
    assert_eq!(*read.lock().unwrap(), vec![own]);
}

#[test]
fn the_running_card_route_answers_from_herdrs_lists() {
    let w = world("running");
    let running = serve(w.site());
    let (status, card) = get_json(running.addr, "/api/cards/running");
    assert_eq!(status, 200, "{card}");
    assert_eq!(
        (card["working"].clone(), card["needs"].clone()),
        (json!(2), json!(1))
    );
    assert_eq!(
        w.herdr.methods(),
        vec!["workspace.list".to_string(), "agent.list".to_string()]
    );
}

// --- The page -----------------------------------------------------------------------------------------------------

#[test]
fn the_page_has_the_cards_and_the_new_space_sheet() {
    let w = world("page");
    let running = serve(w.site());
    let (_, _, index) = get(running.addr, "/");
    for script in ["/cards.js", "/newspace.js"] {
        assert!(
            index.contains(&format!(r#"<script src="{script}"></script>"#)),
            "{script}"
        );
        let (status, head, _) = get(running.addr, script);
        assert_eq!(status, 200, "{script}");
        assert!(head.contains("text/javascript"), "{head}");
    }
    let (_, _, cards) = get(running.addr, "/cards.js");
    let (_, _, newspace) = get(running.addr, "/newspace.js");
    let (_, _, spaces) = get(running.addr, "/spaces.js");
    for route in [
        "/api/cards/usage",
        "/api/cards/servers",
        "/api/cards/running",
    ] {
        assert!(cards.contains(&format!("'{route}'")), "{route}");
    }
    assert!(
        newspace.contains("nv.postJson('/api/folders/open', { name: folder.name })"),
        "the folder's listed name, never a path"
    );
    assert!(newspace.contains("nv.getJson('/api/folders')"));
    assert!(spaces.contains("window.NirvanaCards.mount(nv)"));
    assert!(
        spaces.contains("cards.refresh()"),
        "a changed list reads the cards again"
    );
    assert!(spaces.contains("pickers.open(nv,"));
    assert!(spaces.contains("'aria-label': 'Open a new space'"));
    for (name, text) in [
        ("cards", &cards),
        ("newspace", &newspace),
        ("spaces", &spaces),
    ] {
        for markup in [".innerHTML", ".outerHTML", "insertAdjacentHTML", "style="] {
            assert!(!text.contains(markup), "{name} has {markup}");
        }
    }
    let (_, _, css) = get(running.addr, "/app.css");
    for rule in [
        ".cards {",
        ".card-box {",
        ".bar-fill {",
        ".folder {",
        "@keyframes grow",
    ] {
        assert!(css.contains(rule), "{rule}");
    }
}
