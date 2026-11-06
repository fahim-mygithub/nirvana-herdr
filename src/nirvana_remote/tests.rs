//! Plan 39 P39a's tests. Every one runs against fakes: a fake Tailscale (its status text, its serve config and its
//! held serves), a fake herdr socket API, a fake peer check and a fake probe. None runs `tailscale`, herdr or Claude
//! (plan 39 §4). A server a test starts binds 127.0.0.1 on a free port and is stopped by the test.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::herdr::{self, Herdr, Summary};
use super::http;
use super::pcs::{self, Hello, Probe, State};
use super::peer::{self, PeerCheck, Refusal};
use super::routes::{self, Site};
use super::tailscale::{self, Held, Mount, Off, Tailscale};
use crate::nirvana_servers::Process;

const OWNER: &str = "owner@example.com";
const HOST: &str = "desk.example-tail.ts.net:8443";
/// The owner's phone, as tailscaled names the caller in X-Forwarded-For.
const PHONE: &str = "100.101.102.103";
/// This PC's own Tailscale address.
const DESK_IP: &str = "100.64.0.1";

/// A made-up tailnet: this PC, a laptop without Nirvana, a studio PC with it, a PC asleep, a phone, a PC of
/// another user and a tagged runner. No real name, address or login.
fn status_json(state: &str, certs: Value) -> String {
    json!({
        "BackendState": state,
        "CertDomains": certs,
        "Self": {"HostName": "Desk", "DNSName": "desk.example-tail.ts.net.", "OS": "windows",
                 "Online": true, "UserID": 11, "LastSeen": "0001-01-01T00:00:00Z",
                 "TailscaleIPs": [DESK_IP, "fd7a:115c:a1e0::1"]},
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

/// Nirvana Remote's own mount on 8443, to `local`, in a foreground session or the background config.
fn ours(local: u16, foreground: bool) -> String {
    let config = json!({
        "TCP": {"8443": {"HTTPS": true}},
        "Web": {HOST: {"Handlers": {"/": {"Proxy": format!("http://127.0.0.1:{local}")}}}}
    });
    if foreground {
        json!({"Foreground": {"a1b2c3": config}}).to_string()
    } else {
        config.to_string()
    }
}

struct FakeTailscale {
    status: Mutex<Result<String, String>>,
    status_reads: AtomicUsize,
    /// The node's serve config as `serve status --json` prints it.
    serve: Arc<Mutex<String>>,
    hold_fails: Option<String>,
    /// Whether a hold makes the mount show in `serve status`.
    hold_shows: bool,
    /// Whether the last held `tailscale serve` is still running.
    alive: Arc<AtomicBool>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeTailscale {
    fn with(status: Result<String, String>) -> Self {
        Self {
            status: Mutex::new(status),
            status_reads: AtomicUsize::new(0),
            serve: Arc::new(Mutex::new("{}".into())),
            hold_fails: None,
            hold_shows: true,
            alive: Arc::new(AtomicBool::new(true)),
            calls: Arc::default(),
        }
    }

    fn serving(self, text: String) -> Self {
        *self.serve.lock().unwrap() = text;
        self
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    fn serve_text(&self) -> String {
        self.serve.lock().unwrap().clone()
    }
}

impl Default for FakeTailscale {
    fn default() -> Self {
        Self::with(Err("unset".into()))
    }
}

struct FakeHeld {
    alive: Arc<AtomicBool>,
    serve: Arc<Mutex<String>>,
    calls: Arc<Mutex<Vec<String>>>,
}

impl Held for FakeHeld {
    fn ended(&mut self) -> Option<String> {
        (!self.alive.load(Ordering::SeqCst)).then(|| "tailscale serve ended".to_string())
    }
}

impl Drop for FakeHeld {
    fn drop(&mut self) {
        // tailscaled deletes a foreground session's mount when its CLI ends.
        *self.serve.lock().unwrap() = "{}".into();
        self.calls.lock().unwrap().push("released".into());
    }
}

impl Tailscale for FakeTailscale {
    fn status(&self) -> Result<String, String> {
        self.status_reads.fetch_add(1, Ordering::SeqCst);
        self.status.lock().unwrap().clone()
    }

    fn serve_status(&self) -> Result<String, String> {
        Ok(self.serve_text())
    }

    fn hold(&self, https: u16, local: u16) -> Result<Box<dyn Held>, String> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("hold {https} {local}"));
        if let Some(why) = &self.hold_fails {
            return Err(why.clone());
        }
        self.alive.store(true, Ordering::SeqCst);
        if self.hold_shows {
            *self.serve.lock().unwrap() = ours(local, true);
        }
        Ok(Box::new(FakeHeld {
            alive: self.alive.clone(),
            serve: self.serve.clone(),
            calls: self.calls.clone(),
        }))
    }

    fn clear_background(&self, https: u16) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("clear {https}"));
        *self.serve.lock().unwrap() = "{}".into();
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
    fn check(&self, _peer: SocketAddr, _local: SocketAddr) -> Result<(), Refusal> {
        self.0.clone().map_err(|why| Refusal {
            pid: Some(4242),
            why,
        })
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

/// A site that knows the owner (and this PC's name and addresses) when `owner` is given, as after a check.
fn site(ts: Arc<FakeTailscale>, peer: Result<(), String>, owner: Option<&str>) -> Arc<Site> {
    let site = Site::new(
        ts,
        Arc::new(herdr_fake()),
        Arc::new(FakePeer(peer)),
        Arc::new(FakeProbe),
        super::HTTPS_PORT,
        fixed_now,
    );
    if let Some(owner) = owner {
        *site.owner.write().unwrap() = Some(owner.to_string());
        *site.me.write().unwrap() = Some(routes::Me {
            host: HOST.into(),
            ips: vec![
                DESK_IP.parse().unwrap(),
                "fd7a:115c:a1e0::1".parse().unwrap(),
            ],
        });
    }
    Arc::new(site)
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
    serve_with(site, http::Limits::DEFAULT)
}

fn serve_with(site: Arc<Site>, limits: http::Limits) -> Running {
    let rt = runtime();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let handler: Arc<dyn http::Handler> = site;
    rt.spawn(http::serve(listener, handler, limits, stopped));
    Running { rt, addr, stop }
}

/// The headers tailscaled sets on a request from the owner's phone, without the closing blank line.
fn proxied(method: &str, path: &str, login: Option<&str>) -> String {
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\n");
    if let Some(login) = login {
        head.push_str(&format!("Tailscale-User-Login: {login}\r\n"));
    }
    head
}

/// One raw request; the status code, the head and the body.
fn get(addr: SocketAddr, path: &str, login: Option<&str>) -> (u16, String, String) {
    raw(
        addr,
        format!("{}\r\n", proxied("GET", path, login)).as_bytes(),
    )
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
    let refused = peer::System::new().check(peer_addr, local).unwrap_err();
    assert_eq!(refused.pid, Some(std::process::id()));
    assert!(
        refused.why.contains(&std::process::id().to_string()),
        "{refused:?}"
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
        "{}Sec-Fetch-Site: same-origin\r\nOrigin: https://{HOST}\r\nContent-Length: 0\r\n\r\n",
        proxied("POST", "/api/pcs", Some(OWNER))
    );
    assert_eq!(raw(running.addr, post.as_bytes()).0, 405);
    let head = format!("{}\r\n", proxied("HEAD", "/", Some(OWNER)));
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
        confirm: Duration::from_secs(2),
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
        let local: u16 = calls[0]
            .strip_prefix("hold 8443 ")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            ts.serve_text(),
            ours(local, true),
            "a foreground mount, not --bg"
        );
        // The listener is up behind the serve, and lets the owner in now that the status named them.
        let (status, _, _) = get(
            SocketAddr::from(([127, 0, 0, 1], local)),
            "/api/hello",
            Some(OWNER),
        );
        assert_eq!(status, 200);
    });
    assert_eq!(ts.calls().last().map(String::as_str), Some("released"));
    assert_eq!(ts.serve_text(), "{}", "the mount goes with the held serve");
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
        hold_fails: Some("serve config denied".into()),
        ..FakeTailscale::with(Ok(ready_json()))
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
        vec![format!("hold 8443 {port}")],
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

/// The gates in front of the session's: a Nirvana-session server under a test's socket or config, with `off`, or in
/// a debug build without `on`, does not host it. Before 0.58.1 this used the default session, which now skips first
/// and would hide every one of these.
#[test]
fn only_the_owners_nirvana_server_hosts_nirvana_remote() {
    let nirvana = || Some(crate::nirvana_launch::SESSION.to_string());
    let none = |_: &str| None;
    assert_eq!(super::skip_reason(none, nirvana(), false), None);
    let socket = |name: &str| (name == "HERDR_SOCKET_PATH").then(|| "x".to_string());
    assert!(super::skip_reason(socket, nirvana(), false).is_some());
    let config = |name: &str| (name == "XDG_CONFIG_HOME").then(|| "x".to_string());
    assert!(super::skip_reason(config, nirvana(), false).is_some());
    let off = |name: &str| (name == "NIRVANA_REMOTE").then(|| "off".to_string());
    assert!(super::skip_reason(off, nirvana(), false).is_some());
    assert!(super::skip_reason(none, Some("work".into()), false).is_some());
    // Review round B1: a debug build (herdr-dev) runs it only when asked.
    assert!(
        super::skip_reason(none, nirvana(), true).is_some(),
        "a plain debug server"
    );
    let on = |name: &str| (name == "NIRVANA_REMOTE").then(|| "on".to_string());
    assert_eq!(super::skip_reason(on, nirvana(), true), None);
    assert!(
        super::skip_reason(on, Some("work".into()), true).is_some(),
        "on does not lift the other gates"
    );
}

/// Nirvana only (0.58.1): a plain `herdr` window started first took 8443 and the Nirvana window read "8443
/// taken". The owner, 2026-11-05: "I agree" to only the Nirvana window running the phone server. The default
/// session skips with a short reason, and `on` or a release build does not lift it.
#[test]
fn the_default_session_does_not_host_nirvana_remote() {
    let none = |_: &str| None;
    let on = |name: &str| (name == "NIRVANA_REMOTE").then(|| "on".to_string());
    assert_eq!(
        super::skip_reason(none, None, false),
        Some("not the Nirvana session")
    );
    assert_eq!(
        super::skip_reason(on, None, false),
        Some("not the Nirvana session")
    );
    assert_eq!(
        super::skip_reason(on, None, true),
        Some("not the Nirvana session")
    );
    assert!(super::skip_reason(none, None, true).is_some(), "debug too");
}

/// Session fix (0.55.1): every Nirvana window runs `--session nirvana`, and the old gate skipped every named
/// session, so the owner's server logged "a named session's server" and served nothing.
#[test]
fn the_nirvana_session_hosts_nirvana_remote_and_other_names_skip() {
    let nirvana = || Some(crate::nirvana_launch::SESSION.to_string());
    let none = |_: &str| None;
    assert_eq!(super::skip_reason(none, nirvana(), false), None);
    for name in ["HERDR_SOCKET_PATH", "XDG_CONFIG_HOME"] {
        let set = |var: &str| (var == name).then(|| "x".to_string());
        assert!(
            super::skip_reason(set, nirvana(), false).is_some(),
            "{name}"
        );
    }
    let off = |name: &str| (name == "NIRVANA_REMOTE").then(|| "off".to_string());
    assert_eq!(
        super::skip_reason(off, nirvana(), false),
        Some("NIRVANA_REMOTE=off")
    );
    assert!(
        super::skip_reason(none, nirvana(), true).is_some(),
        "a debug build still waits for NIRVANA_REMOTE=on"
    );
    let on = |name: &str| (name == "NIRVANA_REMOTE").then(|| "on".to_string());
    assert_eq!(super::skip_reason(on, nirvana(), true), None);
    for other in ["work", "nirvana-2", "Nirvana"] {
        assert_eq!(
            super::skip_reason(none, Some(other.into()), false),
            Some("a named session's server"),
            "{other}"
        );
    }
}

/// Session fix (0.55.1): two servers that both host it, each with its own state file (`session::data_dir` differs),
/// both want port 7790 and 8443 on one tailnet node. Since 0.58.1 only the Nirvana session hosts it, so this is
/// the case of a second one all the same (a pre-0.58.1 install beside a new one, a debug build with
/// `NIRVANA_REMOTE=on`). The first up keeps its mount; the second reads that foreground mount in `serve status`,
/// reports "8443 taken" in its own file and changes nothing, before and after it stops.
#[test]
fn a_second_sessions_server_finds_8443_taken_and_leaves_the_first_alone() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let first_state = temp_state("two-first");
    let second_state = temp_state("two-second");
    let _ = std::fs::remove_file(&first_state);
    let _ = std::fs::remove_file(&second_state);
    let rt = runtime();
    let config = |state: &std::path::Path| super::Config {
        port,
        https_port: 8443,
        state: state.to_path_buf(),
        every: Duration::from_secs(3600),
        confirm: Duration::from_secs(2),
    };
    let (stop_first, stopped) = tokio::sync::watch::channel(false);
    let first = rt.spawn(super::supervise(
        site(ts.clone(), Ok(()), None),
        config(&first_state),
        stopped,
    ));
    assert!(
        wait_for(&first_state, |report| report.up).up,
        "the first is up"
    );
    let mounted = ts.serve_text();
    assert_eq!(mounted, ours(port, true));
    supervise_once(ts.clone(), port, &second_state, |ts| {
        let report = read_report(&second_state);
        assert!(!report.up);
        assert_eq!(report.short, "8443 taken");
        assert!(
            report.long.contains("another running Nirvana Remote"),
            "{}",
            report.long
        );
        assert_eq!(ts.serve_text(), mounted, "the first's mount stands");
        assert!(read_report(&first_state).up, "and its row stays up");
    });
    assert_eq!(ts.serve_text(), mounted, "the second's stop leaves it too");
    assert_eq!(
        ts.calls(),
        vec![format!("hold 8443 {port}")],
        "one hold, the first's"
    );
    assert_eq!(
        get(
            SocketAddr::from(([127, 0, 0, 1], port)),
            "/api/hello",
            Some(OWNER)
        )
        .0,
        200,
        "the first still serves"
    );
    let _ = stop_first.send(true);
    rt.block_on(first).unwrap();
    assert_eq!(ts.serve_text(), "{}");
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
        confirm: Duration::from_secs(2),
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

// --- Review round: the mount (B2, S2, S3) ------------------------------------------------------------------------

#[test]
fn the_mount_on_8443_is_ours_only_when_it_is_exactly_ours() {
    let read = |text: &str| tailscale::mount(text, "desk.example-tail.ts.net", 8443, 7790).unwrap();
    assert_eq!(read("{}"), Mount::Free);
    assert_eq!(read(&ours(7790, true)), Mount::OursForeground);
    assert_eq!(read(&ours(7790, false)), Mount::OursBackground);
    assert!(
        matches!(read(&ours(7791, false)), Mount::Other(_)),
        "another port"
    );
    let extra = json!({
        "TCP": {"8443": {"HTTPS": true}},
        "Web": {HOST: {"Handlers": {"/": {"Proxy": "http://127.0.0.1:7790"},
                                    "/files": {"Path": "C:\\share"}}}}
    });
    assert!(
        matches!(read(&extra.to_string()), Mount::Other(_)),
        "a second handler"
    );
    let funnel = json!({
        "TCP": {"8443": {"HTTPS": true}},
        "Web": {HOST: {"Handlers": {"/": {"Proxy": "http://127.0.0.1:7790"}}}},
        "AllowFunnel": {HOST: true}
    });
    assert!(
        matches!(read(&funnel.to_string()), Mount::Other(_)),
        "Funnel on it"
    );
    let forward = json!({"TCP": {"8443": {"TCPForward": "127.0.0.1:22"}}});
    assert!(
        matches!(read(&forward.to_string()), Mount::Other(_)),
        "a TCP forward"
    );
    let elsewhere = json!({"Foreground": {"s": {
        "TCP": {"8443": {"HTTPS": true}},
        "Web": {"other.example-tail.ts.net:8443": {"Handlers": {"/": {"Proxy": "http://127.0.0.1:7790"}}}}
    }}});
    assert!(
        matches!(read(&elsewhere.to_string()), Mount::Other(_)),
        "another name on the port"
    );
    // Another port of the owner's is not this one's business.
    let other_port = json!({"TCP": {"443": {"HTTPS": true}},
                            "Web": {"desk.example-tail.ts.net:443": {"Handlers": {"/": {"Proxy": "http://127.0.0.1:3000"}}}}});
    assert_eq!(read(&other_port.to_string()), Mount::Free);
}

#[test]
fn someone_elses_8443_is_left_alone() {
    let theirs = json!({
        "TCP": {"8443": {"HTTPS": true}},
        "Web": {HOST: {"Handlers": {"/": {"Proxy": "http://127.0.0.1:3000"}}}}
    })
    .to_string();
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())).serving(theirs.clone()));
    let state = temp_state("theirs");
    supervise_once(ts.clone(), 0, &state, |_| {
        let report = read_report(&state);
        assert!(!report.up);
        assert_eq!(report.short, "8443 taken");
    });
    assert!(ts.calls().is_empty(), "no hold, no clear: {:?}", ts.calls());
    assert_eq!(ts.serve_text(), theirs, "their mount untouched");
}

#[test]
fn a_live_foreground_mount_of_ours_is_not_taken_over() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())).serving(ours(0, true)));
    let state = temp_state("live-fg");
    supervise_once(ts.clone(), 0, &state, |_| {
        assert_eq!(read_report(&state).short, "8443 taken");
    });
    assert!(ts.calls().is_empty(), "{:?}", ts.calls());
}

#[test]
fn a_stale_background_mount_of_ours_is_cleared_before_the_port_is_bound() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())).serving(ours(port, false)));
    let state = temp_state("stale-bg");
    supervise_once(ts.clone(), port, &state, |ts| {
        assert!(read_report(&state).up);
        assert_eq!(
            ts.calls(),
            vec!["clear 8443".to_string(), format!("hold 8443 {port}")]
        );
    });
}

#[test]
fn a_stale_background_mount_is_cleared_even_when_the_port_is_taken() {
    // The dangerous case S2 names: the mount outlived its server and something else now holds the port.
    let squatter = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = squatter.local_addr().unwrap().port();
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())).serving(ours(port, false)));
    let state = temp_state("stale-squat");
    supervise_once(ts.clone(), port, &state, |_| {
        assert_eq!(read_report(&state).short, format!("port {port} busy"));
    });
    assert_eq!(ts.calls(), vec!["clear 8443".to_string()]);
    assert_eq!(ts.serve_text(), "{}");
}

#[test]
fn a_mount_that_never_shows_is_a_failed_serve() {
    let ts = Arc::new(FakeTailscale {
        hold_shows: false,
        ..FakeTailscale::with(Ok(ready_json()))
    });
    let state = temp_state("never-shows");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    supervise_once(ts.clone(), port, &state, |ts| {
        let report = read_report(&state);
        assert_eq!(report.short, "serve failed", "{report:?}");
        assert_eq!(
            ts.calls(),
            vec![format!("hold 8443 {port}"), "released".to_string()]
        );
        assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
    });
}

/// Poll the state file until `want` holds or 5 s pass.
fn wait_for(state: &std::path::Path, want: impl Fn(&super::Report) -> bool) -> super::Report {
    let started = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(state) {
            if let Ok(report) = serde_json::from_str::<super::Report>(&text) {
                if want(&report) || started.elapsed() > Duration::from_secs(5) {
                    return report;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_mount_that_goes_away_turns_the_row_off_and_is_set_again() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let state = temp_state("gone");
    let _ = std::fs::remove_file(&state);
    let rt = runtime();
    let config = super::Config {
        port: 0,
        https_port: 8443,
        state: state.clone(),
        every: Duration::from_millis(150),
        confirm: Duration::from_secs(2),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = rt.spawn(super::supervise(
        site(ts.clone(), Ok(()), None),
        config,
        stopped,
    ));
    assert!(wait_for(&state, |report| report.up).up);
    // `tailscale serve reset`, or the owner's edit: the mount is gone while the CLI still runs.
    *ts.serve.lock().unwrap() = "{}".into();
    let gone = wait_for(&state, |report| !report.up);
    assert_eq!(gone.short, "serve gone", "{gone:?}");
    assert!(wait_for(&state, |report| report.up).up, "set again");
    // The CLI holding it ends (tailscaled restarted).
    ts.alive.store(false, Ordering::SeqCst);
    let ended = wait_for(&state, |report| !report.up);
    assert_eq!(ended.short, "serve gone", "{ended:?}");
    assert!(ended.long.contains("tailscale serve ended"), "{ended:?}");
    let _ = stop.send(true);
    rt.block_on(task).unwrap();
    let holds = ts
        .calls()
        .iter()
        .filter(|call| call.starts_with("hold"))
        .count();
    assert!(holds >= 2, "{:?}", ts.calls());
}

/// P39b: a live view holds a connection to the listener, and the serve's end closes it (`Serving::end`), so neither
/// a phone nor the herdr server's shutdown waits on it.
#[test]
fn stopping_nirvana_remote_ends_the_pages_live_views() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let state = temp_state("views");
    let _ = std::fs::remove_file(&state);
    let rt = runtime();
    let config = super::Config {
        port: 0,
        https_port: 8443,
        state: state.clone(),
        every: Duration::from_secs(3600),
        confirm: Duration::from_secs(2),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = rt.spawn(super::supervise(
        site(ts.clone(), Ok(()), None),
        config,
        stopped,
    ));
    assert!(wait_for(&state, |report| report.up).up);
    let local: u16 = ts
        .calls()
        .iter()
        .find_map(|call| call.strip_prefix("hold 8443 ")?.parse().ok())
        .unwrap();
    let mut view = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], local))).unwrap();
    view.write_all(format!("{}\r\n", proxied("GET", "/api/events", Some(OWNER))).as_bytes())
        .unwrap();
    view.set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut seen = String::new();
    let mut chunk = [0u8; 4096];
    let started = Instant::now();
    while !seen.contains("event: spaces") && started.elapsed() < Duration::from_secs(5) {
        if let Ok(read) = view.read(&mut chunk) {
            seen.push_str(&String::from_utf8_lossy(&chunk[..read]));
        }
    }
    assert!(seen.contains("event: spaces"), "{seen}");
    let _ = stop.send(true);
    rt.block_on(task).unwrap();
    let started = Instant::now();
    let mut closed = false;
    while started.elapsed() < Duration::from_secs(3) {
        match view.read(&mut chunk) {
            Ok(0) => {
                closed = true;
                break;
            }
            Ok(_) => {}
            Err(err)
                if err.kind() == std::io::ErrorKind::TimedOut
                    || err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                closed = true;
                break;
            }
        }
    }
    drop(view);
    // A view left open would hold the runtime's drop for its 15 minutes.
    rt.shutdown_timeout(Duration::from_secs(1));
    assert!(closed, "the live view outlived Nirvana Remote");
}

#[test]
fn the_held_serve_is_foreground_and_asks_nothing() {
    assert_eq!(
        tailscale::hold_args(8443, 7790),
        vec!["serve", "--https=8443", "http://127.0.0.1:7790"]
    );
}

#[test]
fn the_installed_cli_is_preferred_over_path() {
    let installed = std::path::Path::new("C:\\Program Files")
        .join("Tailscale")
        .join("tailscale.exe");
    assert_eq!(
        tailscale::program(Some("C:\\Program Files".into()), |path| path == installed),
        installed
    );
    assert_eq!(
        tailscale::program(Some("C:\\Program Files".into()), |_| false),
        std::path::PathBuf::from("tailscale")
    );
    assert_eq!(
        tailscale::program(None, |_| true),
        std::path::PathBuf::from("tailscale")
    );
}

// --- Review round: who is let in (S1, S5) --------------------------------------------------------------------------

#[test]
fn a_request_from_this_pc_itself_is_refused() {
    let own: Vec<IpAddr> = vec![
        DESK_IP.parse().unwrap(),
        "fd7a:115c:a1e0::1".parse().unwrap(),
    ];
    assert_eq!(routes::from_another_device(&[PHONE], &own), Ok(()));
    assert_eq!(
        routes::from_another_device(&["fd7a:115c:a1e0::9"], &own),
        Ok(())
    );
    assert!(routes::from_another_device(&[DESK_IP], &own).is_err());
    assert!(routes::from_another_device(&["fd7a:115c:a1e0::1"], &own).is_err());
    assert!(
        routes::from_another_device(&["192.168.1.20"], &own).is_err(),
        "not a tailnet source"
    );
    assert!(routes::from_another_device(&[], &own).is_err());
    assert!(routes::from_another_device(&[PHONE, PHONE], &own).is_err());
    assert!(routes::from_another_device(&["nope"], &own).is_err());
    // Over HTTP: the same owner login from this PC's own address is refused.
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let from_desk = format!(
        "GET /api/hello HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {DESK_IP}\r\nTailscale-User-Login: {OWNER}\r\n\r\n"
    );
    assert_eq!(raw(running.addr, from_desk.as_bytes()).0, 403);
    assert_eq!(get(running.addr, "/api/hello", Some(OWNER)).0, 200);
}

#[test]
fn a_request_to_another_name_is_misdirected() {
    assert_eq!(routes::for_this_pc(&[HOST], Some(HOST)), Ok(()));
    assert_eq!(
        routes::for_this_pc(&["DESK.example-tail.ts.net:8443"], Some(HOST)),
        Ok(())
    );
    assert!(routes::for_this_pc(&["evil.example:8443"], Some(HOST)).is_err());
    assert!(routes::for_this_pc(&["desk.example-tail.ts.net"], Some(HOST)).is_err());
    assert!(routes::for_this_pc(&[], Some(HOST)).is_err());
    assert!(routes::for_this_pc(&[HOST], None).is_err());
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let rebound = format!(
        "GET / HTTP/1.1\r\nHost: evil.example:8443\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: {OWNER}\r\n\r\n"
    );
    assert_eq!(raw(running.addr, rebound.as_bytes()).0, 421);
}

#[test]
fn a_cross_site_request_that_may_change_something_is_refused() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let post = |extra: &str| {
        let request = format!(
            "{}{extra}Content-Length: 0\r\n\r\n",
            proxied("POST", "/api/pcs", Some(OWNER))
        );
        raw(running.addr, request.as_bytes()).0
    };
    assert_eq!(post(""), 403, "neither header");
    assert_eq!(post("Sec-Fetch-Site: cross-site\r\n"), 403);
    assert_eq!(post("Sec-Fetch-Site: same-site\r\n"), 403);
    assert_eq!(post("Origin: https://evil.example\r\n"), 403);
    assert_eq!(
        post("Sec-Fetch-Site: same-origin\r\nOrigin: https://evil.example\r\n"),
        403
    );
    // From the page itself the request passes the check and reaches the method rule.
    assert_eq!(
        post(&format!(
            "Sec-Fetch-Site: same-origin\r\nOrigin: https://{HOST}\r\n"
        )),
        405
    );
    assert_eq!(post(&format!("Origin: https://{HOST}\r\n")), 405);
    // A GET is a read, and needs neither.
    assert_eq!(get(running.addr, "/api/hello", Some(OWNER)).0, 200);
}

// --- Review round: the HTTP limits (S6, N1, N7) --------------------------------------------------------------------

#[test]
fn a_head_that_never_finishes_gets_408() {
    let running = serve_with(
        site(Arc::default(), Ok(()), Some(OWNER)),
        http::Limits {
            read_timeout: Duration::from_millis(300),
            connections: http::CONNECTIONS,
            body_rate: http::BODY_RATE,
            body_most: http::BODY_MOST,
        },
    );
    let started = Instant::now();
    let (status, _, _) = raw(running.addr, b"GET / HTTP/1.1\r\nHost: x\r\n");
    assert_eq!(status, 408);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_33rd_connection_waits_for_a_slot() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let stalled = (0..http::CONNECTIONS)
        .map(|_| TcpStream::connect(running.addr).unwrap())
        .collect::<Vec<_>>();
    std::thread::sleep(Duration::from_millis(300));
    let mut late = TcpStream::connect(running.addr).unwrap();
    late.write_all(format!("{}\r\n", proxied("GET", "/api/hello", Some(OWNER))).as_bytes())
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
}

#[test]
fn every_answer_carries_the_safety_headers() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let refused = serve(site(
        Arc::default(),
        Err("pid 4242 (x.exe)".into()),
        Some(OWNER),
    ));
    let answers = [
        get(running.addr, "/", Some(OWNER)),
        get(running.addr, "/app.js", Some(OWNER)),
        get(running.addr, "/api/hello", Some(OWNER)),
        get(running.addr, "/nope", Some(OWNER)),
        get(running.addr, "/", Some("friend@example.com")),
        get(refused.addr, "/", Some(OWNER)),
    ];
    assert_eq!(
        answers
            .iter()
            .map(|(status, _, _)| *status)
            .collect::<Vec<_>>(),
        vec![200, 200, 200, 404, 403, 403]
    );
    for (status, head, _) in &answers {
        for header in [
            "Cache-Control: no-store",
            "X-Content-Type-Options: nosniff",
            "Referrer-Policy: no-referrer",
            "Connection: close",
            "Content-Security-Policy: default-src 'self'",
        ] {
            assert!(head.contains(header), "{status} lacks {header}: {head}");
        }
    }
}

#[test]
fn head_on_an_asset_has_its_length_and_no_body() {
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    for path in ["/app.css", "/app.js", "/pcs.js"] {
        let (_, _, body) = get(running.addr, path, Some(OWNER));
        let (status, head, empty) = raw(
            running.addr,
            format!("{}\r\n", proxied("HEAD", path, Some(OWNER))).as_bytes(),
        );
        assert_eq!(status, 200, "{path}");
        assert!(
            head.contains(&format!("Content-Length: {}", body.len())),
            "{path}: {head}"
        );
        assert!(empty.is_empty(), "{path}");
    }
}

#[test]
fn a_request_body_is_drained_not_reset() {
    // The head first; the body only once the answer is on its way, as a slow upload sends it. A server that closed
    // without draining has its socket gone by then, so the body draws a reset, and Windows drops the answer the
    // client had not read yet.
    let running = serve(site(Arc::default(), Ok(()), Some(OWNER)));
    let body = vec![b'x'; 48 * 1024];
    let head = format!(
        "{}Content-Length: {}

",
        proxied("POST", "/api/pcs", Some(OWNER)),
        body.len()
    );
    let mut stream = TcpStream::connect(running.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let _ = stream.write_all(&body);
    std::thread::sleep(Duration::from_millis(200));
    let mut answer = String::new();
    let read = stream.read_to_string(&mut answer);
    assert!(read.is_ok(), "{read:?} after {answer:?}");
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
    assert!(answer.ends_with("Only the Tailscale account this PC is signed in with can connect."));
}

// --- Review round: the log and the process snapshot (S4) ---------------------------------------------------------

#[test]
fn a_refusal_is_logged_once_a_minute_per_caller() {
    let throttle = routes::Throttle::new(Duration::from_secs(60));
    let t0 = Instant::now();
    assert_eq!(throttle.allow("pid 1", t0), Some(0));
    assert_eq!(throttle.allow("pid 1", t0 + Duration::from_secs(1)), None);
    assert_eq!(throttle.allow("pid 1", t0 + Duration::from_secs(30)), None);
    assert_eq!(
        throttle.allow("pid 2", t0 + Duration::from_secs(1)),
        Some(0),
        "another caller"
    );
    assert_eq!(
        throttle.allow("pid 1", t0 + Duration::from_secs(61)),
        Some(2),
        "the next line counts what was held back"
    );
    // Cycling keys cannot grow the map without end.
    for key in 0..5000 {
        let _ = throttle.allow(&format!("k{key}"), t0 + Duration::from_secs(62));
    }
    assert_eq!(
        throttle.allow("pid 9", t0 + Duration::from_secs(62)),
        None,
        "full"
    );
    assert_eq!(
        throttle.allow("pid 9", t0 + Duration::from_secs(200)),
        Some(0),
        "old keys dropped"
    );
}

#[test]
fn the_process_snapshot_refuses_from_cache_and_trusts_only_fresh() {
    let reads = AtomicUsize::new(0);
    let child = std::cell::Cell::new(true);
    let processes = || {
        reads.fetch_add(1, Ordering::SeqCst);
        if child.get() {
            vec![Process {
                pid: 8672,
                parent_pid: 6716,
                name: "tailscaled.exe".into(),
            }]
        } else {
            Vec::new()
        }
    };
    let snapshot = peer::Snapshot::new(Duration::from_secs(1));
    let t0 = Instant::now();
    // A connect loop from pid 4242: one snapshot answers every refusal within the second.
    for step in 0..10 {
        let (trusted, _) = snapshot.trusted(
            4242,
            Some(6716),
            &processes,
            t0 + Duration::from_millis(step * 50),
        );
        assert!(!trusted);
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    // The service's child is trusted only on a fresh read.
    let (trusted, _) = snapshot.trusted(
        8672,
        Some(6716),
        &processes,
        t0 + Duration::from_millis(600),
    );
    assert!(trusted);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "read again before trusting"
    );
    // The child is gone and its pid reused within the second: the cache still names it, the fresh read does not.
    child.set(false);
    let (trusted, _) = snapshot.trusted(
        8672,
        Some(6716),
        &processes,
        t0 + Duration::from_millis(700),
    );
    assert!(!trusted, "old data never lets a caller in");
    // A second later the cache is read again.
    let _ = snapshot.trusted(4242, Some(6716), &processes, t0 + Duration::from_secs(3));
    assert_eq!(reads.load(Ordering::SeqCst), 4);
}

// --- Review round: one state file owner (S7) and the status cache (N2) -------------------------------------------

#[test]
fn a_second_server_leaves_the_holders_report_alone() {
    let state = temp_state("second");
    let holder = super::Report {
        up: true,
        short: ":8443".into(),
        long: "serving".into(),
        url: Some("https://desk.example-tail.ts.net:8443/".into()),
        at: super::unix_now(),
        pid: 1,
    };
    std::fs::write(&state, serde_json::to_string(&holder).unwrap()).unwrap();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let rt = runtime();
    let config = super::Config {
        port,
        https_port: 8443,
        state: state.clone(),
        every: Duration::from_secs(3600),
        confirm: Duration::from_secs(2),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = rt.spawn(super::supervise(
        site(ts.clone(), Ok(()), None),
        config,
        stopped,
    ));
    // The check has run once the owner is known.
    for _ in 0..200 {
        if ts.status_reads.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(read_report(&state), holder, "the holder's report stands");
    let _ = stop.send(true);
    rt.block_on(task).unwrap();
    assert_eq!(read_report(&state), holder, "and is not removed at stop");
    // A stale one is anyone's.
    let stale = super::Report { at: 1, ..holder };
    std::fs::write(&state, serde_json::to_string(&stale).unwrap()).unwrap();
    supervise_once(ts, port, &state, |_| {
        let report = wait_for(&state, |report| report.pid == std::process::id());
        assert_eq!(report.short, format!("port {port} busy"));
    });
}

#[test]
fn api_pcs_reads_tailscale_once_in_five_seconds() {
    let ts = Arc::new(FakeTailscale::with(Ok(ready_json())));
    let running = serve(site(ts.clone(), Ok(()), Some(OWNER)));
    for _ in 0..3 {
        assert_eq!(get(running.addr, "/api/pcs", Some(OWNER)).0, 200);
    }
    assert_eq!(ts.status_reads.load(Ordering::SeqCst), 1);
}
