//! Plan 39 P39a's tests. Every one runs against fakes: a fake Tailscale (its status text and its serve calls), a
//! fake herdr socket API, a fake peer check and a fake probe. None runs `tailscale`, herdr or Claude (plan 39 §4).
//! A server a test starts binds 127.0.0.1 on a free port and is stopped by the test.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde_json::{json, Value};

use super::herdr::{self, Herdr, Summary};
use super::http;
use super::pcs::{self, Hello, Probe, State};
use super::peer::{self, PeerCheck};
use super::routes::{self, Site};
use super::tailscale::{self, Off, Tailscale};
use crate::nirvana_servers::Process;

const OWNER: &str = "owner@example.com";

/// A made-up tailnet: this PC, a laptop without Nirvana, a studio PC with it, a PC asleep, a phone, a PC of
/// another user and a tagged runner. No real name, address or login.
fn status_json(state: &str, certs: Value) -> String {
    json!({
        "BackendState": state,
        "CertDomains": certs,
        "Self": {"HostName": "Desk", "DNSName": "desk.example-tail.ts.net.", "OS": "windows",
                 "Online": true, "UserID": 11, "LastSeen": "0001-01-01T00:00:00Z"},
        "User": {"11": {"LoginName": OWNER, "DisplayName": "Owner"},
                 "22": {"LoginName": "friend@example.com"}},
        "Peer": {
            "k1": {"HostName": "Linux laptop", "DNSName": "laptop.example-tail.ts.net.", "OS": "linux",
                   "Online": true, "UserID": 11},
            "k2": {"HostName": "Work PC", "DNSName": "work.example-tail.ts.net.", "OS": "windows",
                   "Online": false, "UserID": 11, "LastSeen": "2026-11-04T10:00:00Z"},
            "k3": {"HostName": "Studio", "DNSName": "studio.example-tail.ts.net.", "OS": "windows",
                   "Online": true, "UserID": 11},
            "k4": {"HostName": "phone", "DNSName": "phone.example-tail.ts.net.", "OS": "iOS",
                   "Online": true, "UserID": 11},
            "k5": {"HostName": "friend-pc", "DNSName": "friend.example-tail.ts.net.", "OS": "windows",
                   "Online": true, "UserID": 22},
            "k6": {"HostName": "ci-runner", "DNSName": "ci.example-tail.ts.net.", "OS": "linux",
                   "Online": true, "UserID": 11, "Tags": ["tag:ci"]}
        }
    })
    .to_string()
}

fn ready_json() -> String {
    status_json("Running", json!(["desk.example-tail.ts.net"]))
}

struct FakeTailscale {
    status: Mutex<Result<String, String>>,
    serve_fails: Option<String>,
    calls: Mutex<Vec<String>>,
}

impl FakeTailscale {
    fn with(status: Result<String, String>) -> Self {
        Self {
            status: Mutex::new(status),
            serve_fails: None,
            calls: Mutex::default(),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Default for FakeTailscale {
    fn default() -> Self {
        Self::with(Err("unset".into()))
    }
}

impl Tailscale for FakeTailscale {
    fn status(&self) -> Result<String, String> {
        self.status.lock().unwrap().clone()
    }

    fn serve_on(&self, https: u16, local: u16) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("on {https} {local}"));
        match &self.serve_fails {
            Some(why) => Err(why.clone()),
            None => Ok(()),
        }
    }

    fn serve_off(&self, https: u16) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("off {https}"));
        Ok(())
    }
}

struct FakeHerdr {
    statuses: Vec<&'static str>,
    spaces: usize,
}

impl Herdr for FakeHerdr {
    fn call(&self, method: &str, _params: Value) -> Result<Value, String> {
        match method {
            "workspace.list" => Ok(json!({
                "type": "workspace_list",
                "workspaces": (0..self.spaces).map(|i| json!({"workspace_id": format!("w{i}")})).collect::<Vec<_>>(),
            })),
            "agent.list" => Ok(json!({
                "type": "agent_list",
                "agents": self.statuses.iter().map(|status| json!({"agent_status": status})).collect::<Vec<_>>(),
            })),
            other => Err(format!("unexpected {other}")),
        }
    }
}

fn herdr_fake() -> FakeHerdr {
    FakeHerdr {
        statuses: vec!["working", "blocked", "done", "idle"],
        spaces: 3,
    }
}

struct FakePeer(Result<(), String>);

impl PeerCheck for FakePeer {
    fn check(&self, _peer: SocketAddr, _local: SocketAddr) -> Result<(), String> {
        self.0.clone()
    }
}

/// Answers as Nirvana for the studio, as something else for the laptop, and not at all elsewhere.
struct FakeProbe;

impl Probe for FakeProbe {
    fn hello(&self, dns: &str, _port: u16) -> Option<Hello> {
        match dns {
            "studio.example-tail.ts.net" => Some(Hello {
                app: pcs::APP.into(),
                version: "0.54.0".into(),
                spaces: 2,
                working: 1,
            }),
            "laptop.example-tail.ts.net" => Some(Hello {
                app: "some-other-server".into(),
                version: "1".into(),
                spaces: 9,
                working: 9,
            }),
            _ => None,
        }
    }
}

fn at(hour: u8) -> time::OffsetDateTime {
    time::Date::from_calendar_date(2026, time::Month::November, 4)
        .unwrap()
        .with_hms(hour, 0, 0)
        .unwrap()
        .assume_utc()
}

fn fixed_now() -> time::OffsetDateTime {
    at(12)
}

fn site(ts: Arc<FakeTailscale>, peer: Result<(), String>, owner: Option<&str>) -> Arc<Site> {
    Arc::new(Site {
        tailscale: ts,
        herdr: Arc::new(herdr_fake()),
        peer: Arc::new(FakePeer(peer)),
        probe: Arc::new(FakeProbe),
        owner: RwLock::new(owner.map(str::to_string)),
        https_port: super::HTTPS_PORT,
        now: fixed_now,
    })
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// A server on 127.0.0.1:<free port>, stopped when the guard drops.
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
    let rt = runtime();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let handler: Arc<dyn http::Handler> = site;
    rt.spawn(http::serve(listener, handler, stopped));
    Running { rt, addr, stop }
}

/// One raw request; the status code, the head and the body.
fn get(addr: SocketAddr, path: &str, login: Option<&str>) -> (u16, String, String) {
    let mut head = format!("GET {path} HTTP/1.1\r\nHost: desk.example-tail.ts.net:8443\r\n");
    if let Some(login) = login {
        head.push_str(&format!("Tailscale-User-Login: {login}\r\n"));
    }
    head.push_str("\r\n");
    raw(addr, head.as_bytes())
}

fn raw(addr: SocketAddr, bytes: &[u8]) -> (u16, String, String) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(bytes).unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let (head, body) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    (status, head.to_string(), body.to_string())
}

// --- Tailscale's status ---------------------------------------------------------------------------------------

#[test]
fn status_names_the_owner_and_the_peers() {
    let status = tailscale::parse_status(&ready_json()).unwrap();
    assert_eq!(status.owner_login, OWNER);
    assert_eq!(status.me.host, "Desk");
    assert_eq!(status.me.dns, "desk.example-tail.ts.net");
    assert_eq!(status.me.last_seen, None, "year 1 is no time");
    assert_eq!(status.cert_domains, vec!["desk.example-tail.ts.net"]);
    assert_eq!(status.peers.len(), 6);
    let work = status
        .peers
        .iter()
        .find(|peer| peer.host == "Work PC")
        .unwrap();
    assert_eq!(work.last_seen, Some(at(10)));
    assert!(status.peers.iter().any(|peer| peer.tagged));
}

#[test]
fn a_signed_out_or_stopped_tailscale_does_not_serve() {
    let signed_out = status_json("NeedsLogin", json!(["x"]));
    assert_eq!(super::assess(Ok(signed_out)), Err(Off::SignedOut));
    let stopped = status_json("Stopped", json!(["x"]));
    assert_eq!(super::assess(Ok(stopped)), Err(Off::Stopped));
    assert_eq!(
        super::assess(Err("not installed".into())),
        Err(Off::NotInstalled)
    );
}

#[test]
fn https_off_for_the_tailnet_does_not_serve() {
    // Measured on the owner's PC 2026-11-04: CertDomains is null until HTTPS is turned on.
    assert_eq!(
        super::assess(Ok(status_json("Running", Value::Null))),
        Err(Off::NoHttps)
    );
    assert_eq!(
        super::assess(Ok(status_json("Running", json!([])))),
        Err(Off::NoHttps)
    );
    assert!(super::assess(Ok(ready_json())).is_ok());
}

// --- Who is let in ----------------------------------------------------------------------------------------------

#[test]
fn the_owners_login_is_let_in() {
    assert_eq!(routes::decide(&[OWNER], Some(OWNER)), Ok(()));
}

#[test]
fn another_user_or_no_user_is_refused() {
    assert!(routes::decide(&["friend@example.com"], Some(OWNER)).is_err());
    assert!(routes::decide(&[], Some(OWNER)).is_err());
    assert!(routes::decide(&[OWNER, OWNER], Some(OWNER)).is_err());
    assert!(routes::decide(&[OWNER], None).is_err());
    // Case is not folded: Tailscale sends the login as it stores it.
    assert!(routes::decide(&["Owner@example.com"], Some(OWNER)).is_err());
}

#[test]
fn over_http_the_owner_gets_the_page_and_others_get_403() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let (status, head, body) = get(running.addr, "/", Some(OWNER));
    assert_eq!(status, 200, "{head}");
    assert!(body.contains("/pcs.js"));
    assert_eq!(get(running.addr, "/", Some("friend@example.com")).0, 403);
    assert_eq!(get(running.addr, "/", None).0, 403);
    assert_eq!(get(running.addr, "/api/pcs", None).0, 403);
}

#[test]
fn a_connection_not_from_tailscale_is_refused_before_its_request() {
    let running = serve(site(
        Arc::default(),
        Err("pid 4242 (chrome.exe), not the Tailscale service".into()),
        Some(OWNER),
    ));
    let (status, _, body) = get(running.addr, "/", Some(OWNER));
    assert_eq!(status, 403);
    assert!(!body.contains("<html"));
}

#[test]
fn nobody_is_let_in_while_the_owner_is_unknown() {
    let running = serve(site(Arc::default(), Ok(()), None));
    assert_eq!(get(running.addr, "/", Some(OWNER)).0, 403);
}

#[test]
fn the_peer_check_trusts_only_the_tailscale_service_tree() {
    let processes = vec![
        Process {
            pid: 6716,
            parent_pid: 700,
            name: "tailscaled.exe".into(),
        },
        Process {
            pid: 8672,
            parent_pid: 6716,
            name: "tailscaled.exe".into(),
        },
        Process {
            pid: 8800,
            parent_pid: 6716,
            name: "cmd.exe".into(),
        },
        Process {
            pid: 9000,
            parent_pid: 1234,
            name: "tailscaled.exe".into(),
        },
    ];
    assert!(peer::is_tailscale(6716, Some(6716), &processes));
    assert!(peer::is_tailscale(8672, Some(6716), &processes));
    assert!(
        !peer::is_tailscale(8800, Some(6716), &processes),
        "another child of the service"
    );
    assert!(
        !peer::is_tailscale(9000, Some(6716), &processes),
        "a tailscaled.exe the owner could start"
    );
    assert!(
        !peer::is_tailscale(6716, None, &processes),
        "no service, nobody"
    );
    assert!(!peer::is_tailscale(0, Some(0), &processes));
}

#[cfg(windows)]
#[test]
fn a_connection_owner_is_the_process_that_opened_it() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let local = listener.local_addr().unwrap();
    let client = TcpStream::connect(local).unwrap();
    let (_accepted, peer_addr) = listener.accept().unwrap();
    assert_eq!(peer_addr, client.local_addr().unwrap());
    assert_eq!(
        peer::read::connection_owner(peer_addr, local),
        Some(std::process::id())
    );
    // The same caller through the real check: this test process is not the Tailscale service.
    let refused = peer::System.check(peer_addr, local).unwrap_err();
    assert!(
        refused.contains(&std::process::id().to_string()),
        "{refused}"
    );
}

// --- The PC list ------------------------------------------------------------------------------------------------

#[test]
fn the_pc_list_puts_this_pc_first_then_nirvana_online_and_asleep() {
    let status = tailscale::parse_status(&ready_json()).unwrap();
    let me = Summary {
        spaces: 3,
        working: 2,
    };
    let list = pcs::list(&status, me, 8443, &FakeProbe, fixed_now());
    let names = list.iter().map(|pc| pc.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, vec!["Desk", "Studio", "Linux laptop", "Work PC"]);
    assert!(list[0].is_self);
    assert_eq!(list[0].line, "3 spaces, 2 agents at work");
    assert_eq!(list[0].url.as_deref(), Some("/"));
    assert_eq!(list[1].state, State::Nirvana);
    assert_eq!(list[1].line, "2 spaces, 1 agent at work");
    assert_eq!(
        list[1].url.as_deref(),
        Some("https://studio.example-tail.ts.net:8443/")
    );
    assert_eq!(list[3].state, State::Asleep);
    assert_eq!(list[3].line, "Asleep, last seen 2 h ago");
    assert_eq!(list[3].url, None);
}

#[test]
fn a_peer_answering_as_something_else_reads_as_not_running() {
    let status = tailscale::parse_status(&ready_json()).unwrap();
    let list = pcs::list(&status, Summary::default(), 8443, &FakeProbe, fixed_now());
    let laptop = list.iter().find(|pc| pc.name == "Linux laptop").unwrap();
    assert_eq!(laptop.state, State::Online);
    assert_eq!(laptop.line, "Online, Nirvana not running");
    assert_eq!(laptop.url, None);
}

#[test]
fn phones_tagged_and_other_users_machines_are_not_listed() {
    let status = tailscale::parse_status(&ready_json()).unwrap();
    let list = pcs::list(&status, Summary::default(), 8443, &FakeProbe, fixed_now());
    for name in ["phone", "friend-pc", "ci-runner"] {
        assert!(list.iter().all(|pc| pc.name != name), "{name} listed");
    }
}

#[test]
fn an_asleep_pc_says_when_it_was_last_seen() {
    let now = fixed_now();
    let ago = |minutes: i64| Some(now - time::Duration::minutes(minutes));
    assert_eq!(pcs::asleep_line(None, now), "Asleep");
    assert_eq!(pcs::asleep_line(ago(0), now), "Asleep, last seen just now");
    assert_eq!(pcs::asleep_line(ago(5), now), "Asleep, last seen 5 min ago");
    assert_eq!(pcs::asleep_line(ago(130), now), "Asleep, last seen 2 h ago");
    assert_eq!(
        pcs::asleep_line(ago(3 * 1440), now),
        "Asleep, last seen 3 d ago"
    );
}

#[test]
fn the_nirvana_line_counts_spaces_and_agents_at_work() {
    assert_eq!(pcs::nirvana_line(3, 2), "3 spaces, 2 agents at work");
    assert_eq!(pcs::nirvana_line(1, 1), "1 space, 1 agent at work");
    assert_eq!(pcs::nirvana_line(2, 0), "2 spaces, no agents at work");
}

#[test]
fn the_summary_counts_working_and_blocked_agents() {
    assert_eq!(
        herdr::summary(&herdr_fake()).unwrap(),
        Summary {
            spaces: 3,
            working: 2
        }
    );
}

#[test]
fn a_request_is_the_socket_apis_shape() {
    for method in ["workspace.list", "agent.list"] {
        let request = herdr::request(7, method, json!({})).unwrap();
        assert_eq!(request.id, "nirvana-remote-7");
        assert_eq!(request.method.operation().name, method);
    }
    assert_eq!(
        herdr::answer(r#"{"id":"x","result":{"type":"agent_list","agents":[]}}"#).unwrap()["type"],
        "agent_list"
    );
    assert_eq!(
        herdr::answer(r#"{"id":"x","error":{"code":"c","message":"no such pane"}}"#),
        Err("no such pane".to_string())
    );
}

// --- Routes ----------------------------------------------------------------------------------------------------

#[test]
fn api_pcs_answers_the_list_as_json() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let running = serve(site(ts, Ok(()), Some(OWNER)));
    let (status, head, body) = get(running.addr, "/api/pcs", Some(OWNER));
    assert_eq!(status, 200, "{head}");
    assert!(head.contains("Content-Type: application/json"));
    let value: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(value["pcs"][0]["name"], "Desk");
    assert_eq!(value["pcs"][0]["self"], true);
    assert_eq!(value["pcs"][0]["state"], "nirvana");
    assert_eq!(value["pcs"][0]["line"], "3 spaces, 2 agents at work");
    assert_eq!(value["pcs"][3]["state"], "asleep");
}

#[test]
fn api_pcs_says_why_when_tailscale_cannot_be_read() {
    let ts = Arc::new(FakeTailscale::with(Ok(status_json(
        "NeedsLogin",
        json!(["x"]),
    ))));
    let running = serve(site(ts, Ok(()), Some(OWNER)));
    let (status, _, body) = get(running.addr, "/api/pcs", Some(OWNER));
    assert_eq!(status, 502);
    assert!(body.contains("signed out"), "{body}");
}

#[test]
fn api_hello_names_nirvana_remote() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let (status, _, body) = get(running.addr, "/api/hello", Some(OWNER));
    assert_eq!(status, 200);
    let hello: Hello = serde_json::from_str(&body).unwrap();
    assert_eq!(hello.app, pcs::APP);
    assert_eq!(hello.version, crate::nirvana::VERSION);
    assert_eq!((hello.spaces, hello.working), (3, 2));
}

#[test]
fn the_page_carries_its_screens_and_the_reduced_motion_rule() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let (status, head, body) = get(running.addr, "/", Some(OWNER));
    assert_eq!(status, 200);
    assert!(head.contains("Content-Security-Policy: default-src 'self'"));
    assert!(body.contains(r#"<script src="/app.js"></script>"#));
    assert!(body.contains(r#"<script src="/pcs.js"></script>"#));
    assert!(
        !body.contains("<script>"),
        "the CSP allows no inline script"
    );
    let (_, _, css) = get(running.addr, "/app.css", Some(OWNER));
    assert!(css.contains("@media (prefers-reduced-motion: reduce)"));
    assert!(css.contains("max-width: 430px"));
    let (_, _, pcs) = get(running.addr, "/pcs.js", Some(OWNER));
    assert!(pcs.contains("nv.screen('pcs'"));
    assert!(pcs.contains("Signed in through Tailscale"));
    assert!(!pcs.contains("innerHTML"));
    assert_eq!(get(running.addr, "/nope", Some(OWNER)).0, 404);
}

#[test]
fn only_get_and_head_are_answered() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let post = format!(
        "POST /api/pcs HTTP/1.1\r\nTailscale-User-Login: {OWNER}\r\nContent-Length: 0\r\n\r\n"
    );
    assert_eq!(raw(running.addr, post.as_bytes()).0, 405);
    let head = format!("HEAD / HTTP/1.1\r\nTailscale-User-Login: {OWNER}\r\n\r\n");
    let (status, head, body) = raw(running.addr, head.as_bytes());
    assert_eq!(status, 200, "{head}");
    assert!(body.is_empty());
}

#[test]
fn an_oversized_or_broken_head_is_refused() {
    let mut big = b"GET / HTTP/1.1\r\nX: ".to_vec();
    big.extend(std::iter::repeat_n(b'a', http::HEAD_LIMIT));
    assert_eq!(http::parse_head(&big), Err(431));
    assert_eq!(http::parse_head(b"NOT A REQUEST\r\n\r\n"), Err(400));
    assert_eq!(http::parse_head(b"GET / HTTP/1.1\r\nHost: x\r\n"), Ok(None));
    let request = http::parse_head(b"GET /api/pcs?x=1 HTTP/1.1\r\nHost: x\r\n\r\n")
        .unwrap()
        .unwrap();
    assert_eq!(
        (request.path.as_str(), request.query.as_str()),
        ("/api/pcs", "x=1")
    );
}

// --- The supervisor -------------------------------------------------------------------------------------------

fn temp_state(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("p39a-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("nirvana-remote.json")
}

fn read_report(path: &std::path::Path) -> super::Report {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Run `supervise` for one check, hand its state to `look`, then stop it.
fn supervise_once(
    ts: Arc<FakeTailscale>,
    port: u16,
    state: &std::path::Path,
    look: impl FnOnce(&Arc<FakeTailscale>),
) {
    let rt = runtime();
    let site = site(ts.clone(), Ok(()), None);
    let config = super::Config {
        port,
        https_port: 8443,
        state: state.to_path_buf(),
        every: Duration::from_secs(3600),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = rt.spawn(super::supervise(site, config, stopped));
    for _ in 0..200 {
        if state.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    look(&ts);
    let _ = stop.send(true);
    rt.block_on(task).unwrap();
}

#[test]
fn supervise_serves_when_tailscale_is_ready_and_removes_its_serve_on_stop() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let state = temp_state("ready");
    supervise_once(ts.clone(), 0, &state, |ts| {
        let report = read_report(&state);
        assert!(report.up, "{report:?}");
        assert_eq!(report.short, ":8443");
        assert_eq!(
            report.url.as_deref(),
            Some("https://desk.example-tail.ts.net:8443/")
        );
        let calls = ts.calls();
        assert_eq!(calls.len(), 1, "{calls:?}");
        let local: u16 = calls[0].strip_prefix("on 8443 ").unwrap().parse().unwrap();
        // The listener is up behind the serve, and lets the owner in now that the status named them.
        let (status, _, _) = get(
            SocketAddr::from(([127, 0, 0, 1], local)),
            "/api/hello",
            Some(OWNER),
        );
        assert_eq!(status, 200);
    });
    assert_eq!(ts.calls().last().map(String::as_str), Some("off 8443"));
    assert!(!state.exists(), "the state file goes with the server");
}

#[test]
fn supervise_does_not_serve_when_signed_out_and_says_why() {
    let ts = Arc::new(FakeTailscale::with(Ok(status_json(
        "NeedsLogin",
        json!(["x"]),
    ))));
    let state = temp_state("signed-out");
    supervise_once(ts.clone(), 0, &state, |_| {
        let report = read_report(&state);
        assert!(!report.up);
        assert_eq!(report.short, "signed out");
    });
    assert!(ts.calls().is_empty(), "no serve without Tailscale");
}

#[test]
fn supervise_does_not_serve_with_https_off() {
    let ts = Arc::new(FakeTailscale::with(Ok(status_json("Running", Value::Null))));
    let state = temp_state("https-off");
    supervise_once(ts.clone(), 0, &state, |_| {
        assert_eq!(read_report(&state).short, "HTTPS off");
    });
    assert!(ts.calls().is_empty());
}

#[test]
fn a_failed_serve_closes_the_listener_and_says_why() {
    let ts = Arc::new(FakeTailscale {
        status: Mutex::new(Ok(ready_json())),
        serve_fails: Some("serve config denied".into()),
        calls: Mutex::default(),
    });
    let state = temp_state("serve-fails");
    // A known free port, so the test can see it closed again.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    supervise_once(ts.clone(), port, &state, |_| {
        let report = read_report(&state);
        assert!(!report.up);
        assert_eq!(report.short, "serve failed");
        assert!(report.long.contains("serve config denied"));
        assert!(
            TcpStream::connect(("127.0.0.1", port)).is_err(),
            "the listener closed"
        );
    });
    assert_eq!(
        ts.calls(),
        vec![format!("on 8443 {port}")],
        "nothing to turn off"
    );
}

#[test]
fn a_taken_port_says_why_and_serves_nothing() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let state = temp_state("port-taken");
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    supervise_once(ts.clone(), port, &state, |_| {
        assert_eq!(read_report(&state).short, format!("port {port} busy"));
    });
    assert!(ts.calls().is_empty());
}

#[test]
fn only_the_owners_default_server_hosts_nirvana_remote() {
    let none = |_: &str| None;
    assert_eq!(super::skip_reason(none, None), None);
    let socket = |name: &str| (name == "HERDR_SOCKET_PATH").then(|| "x".to_string());
    assert!(super::skip_reason(socket, None).is_some());
    let config = |name: &str| (name == "XDG_CONFIG_HOME").then(|| "x".to_string());
    assert!(super::skip_reason(config, None).is_some());
    let off = |name: &str| (name == "NIRVANA_REMOTE").then(|| "off".to_string());
    assert!(super::skip_reason(off, None).is_some());
    assert!(super::skip_reason(none, Some("work".into())).is_some());
}

// --- The sidebar row -------------------------------------------------------------------------------------------

#[test]
fn the_foot_row_reads_up_off_and_stale() {
    let up = super::Report::up("https://desk/".into(), 1000);
    let text = serde_json::to_string(&up).unwrap();
    assert_eq!(
        super::foot_row_from(&text, 1010),
        Some(super::FootRow {
            up: true,
            text: ":8443".into()
        })
    );
    assert_eq!(super::foot_row_from(&text, 1000 + 151), None, "stale");
    let off = serde_json::to_string(&super::Report::off(&Off::NoHttps, 1000)).unwrap();
    assert_eq!(
        super::foot_row_from(&off, 1000),
        Some(super::FootRow {
            up: false,
            text: "HTTPS off".into()
        })
    );
    assert_eq!(super::foot_row_from("not json", 0), None);
}

#[test]
fn a_signed_out_tailscale_lets_nobody_in() {
    // The owner was known from an earlier check; Tailscale then signs out.
    let ts = Arc::new(FakeTailscale::with(Ok(status_json(
        "NeedsLogin",
        json!(["x"]),
    ))));
    let site = site(ts, Ok(()), Some(OWNER));
    let state = temp_state("owner-cleared");
    let rt = runtime();
    let config = super::Config {
        port: 0,
        https_port: 8443,
        state: state.clone(),
        every: Duration::from_secs(3600),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = rt.spawn(super::supervise(site.clone(), config, stopped));
    for _ in 0..200 {
        if state.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(*site.owner.read().unwrap(), None);
    let _ = stop.send(true);
    rt.block_on(task).unwrap();
}
