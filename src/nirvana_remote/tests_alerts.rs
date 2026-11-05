//! Plan 39 P39e's tests: alerts and the home-screen app. Everything runs against fakes: a fake push service on
//! 127.0.0.1 (the only endpoint `Hosts::Local` lets through, and that variant exists only under test), a fake herdr
//! socket API with a live `events.subscribe`, a scratch data dir under the private TEMP, and P39b's fake identity
//! and peer check. No test sends a real push, starts Tailscale, a herdr server or Claude.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ring::agreement;
use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{json, Value};

use super::alerts::{self, Alert, Alerts, Debounce, Hosts};
use super::herdr::{Events, Herdr};
use super::routes::{self, Site};
use super::tests_live::{
    from_page, get, post, proxied, raw, serve, Admit, NoProbe, NoTailscale, DESK_IP, HOST, OWNER,
    REFUSED,
};
use super::webpush::{self, Vapid};

fn b64(bytes: &[u8]) -> Vec<u8> {
    URL_SAFE_NO_PAD.decode(bytes).unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p39e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// --- The fakes -----------------------------------------------------------------------------------------------------

/// herdr with two spaces and a live `events.subscribe`.
struct FakeHerdr {
    agents: Mutex<Vec<Value>>,
    subs: Mutex<Vec<mpsc::Sender<Value>>>,
}

fn agent(pane: &str, space: &str, name: &str, status: &str) -> Value {
    json!({
        "pane_id": pane, "workspace_id": space, "name": name, "agent": "claude",
        "title": "\u{2733} Gating two branches", "agent_status": status,
        "tokens": {"nirvana_model": "Claude Opus 5.5", "nirvana_kind": "claude"},
    })
}

impl FakeHerdr {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            agents: Mutex::new(vec![
                agent("p1", "w1", "nirvana", "working"),
                agent("p2", "w2", "printer", "working"),
                agent("p3", "w2", "reel", "idle"),
            ]),
            subs: Mutex::default(),
        })
    }

    /// An agent's status changes on the PC, and herdr says so on every subscription.
    fn set(&self, pane: &str, status: &str) {
        for agent in self.agents.lock().unwrap().iter_mut() {
            if agent["pane_id"] == pane {
                agent["agent_status"] = json!(status);
            }
        }
        for tx in self.subs.lock().unwrap().iter() {
            let _ =
                tx.send(json!({"event": "pane.agent_status_changed", "data": {"pane_id": pane}}));
        }
    }

    fn subscribed(&self) -> usize {
        self.subs.lock().unwrap().len()
    }
}

struct FakeEvents {
    rx: mpsc::Receiver<Value>,
    stop: Arc<AtomicBool>,
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

impl Herdr for FakeHerdr {
    fn call(&self, method: &str, _params: Value) -> Result<Value, String> {
        match method {
            "workspace.list" => Ok(json!({"type": "workspace_list", "workspaces": [
                {"workspace_id": "w1", "label": "Nirvana-Herdr", "number": 1},
                {"workspace_id": "w2", "label": "3D-Printer Flow", "number": 2},
            ]})),
            "agent.list" => {
                Ok(json!({"type": "agent_list", "agents": *self.agents.lock().unwrap()}))
            }
            other => Err(format!("unexpected {other}")),
        }
    }

    fn subscribe(&self, _subscriptions: Value, stop: Arc<AtomicBool>) -> Result<Events, String> {
        let (tx, rx) = mpsc::channel();
        self.subs.lock().unwrap().push(tx);
        Ok(Box::new(FakeEvents { rx, stop }))
    }
}

/// One POST a push service got.
#[derive(Debug, Clone)]
struct Got {
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

/// A push service on 127.0.0.1: it records each POST and answers 410 for a path with `gone` in it, 500 for one with
/// `broken`, else 201, as Apple's does.
struct FakePush {
    port: u16,
    got: Arc<Mutex<Vec<Got>>>,
}

impl FakePush {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let got = Arc::new(Mutex::new(Vec::new()));
        let log = got.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { return };
                let log = log.clone();
                std::thread::spawn(move || answer_push(stream, &log));
            }
        });
        Self { port, got }
    }

    fn got(&self) -> Vec<Got> {
        self.got.lock().unwrap().clone()
    }

    /// The POSTs once there are `count`, or what came within `wait`.
    fn wait_for(&self, count: usize, wait: Duration) -> Vec<Got> {
        let started = Instant::now();
        while self.got().len() < count && started.elapsed() < wait {
            std::thread::sleep(Duration::from_millis(20));
        }
        self.got()
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/push/{path}", self.port)
    }
}

fn answer_push(stream: TcpStream, log: &Mutex<Vec<Got>>) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split(' ').nth(1).unwrap_or("").to_string();
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.trim_end().split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let length = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = reader.read_exact(&mut body);
    let status = if path.contains("gone") {
        "410 Gone"
    } else if path.contains("broken") {
        "500 Internal Server Error"
    } else {
        "201 Created"
    };
    log.lock().unwrap().push(Got {
        path,
        headers,
        body,
    });
    let mut stream = reader.into_inner();
    let _ = stream.write_all(
        format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes(),
    );
}

/// A phone's side of a subscription: its P-256 key (good for one message, as ring's keys are) and auth secret.
struct Ua {
    private: agreement::EphemeralPrivateKey,
    public: Vec<u8>,
    auth: [u8; 16],
}

impl Ua {
    fn new() -> Self {
        let rng = SystemRandom::new();
        let private =
            agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng).unwrap();
        let public = private.compute_public_key().unwrap().as_ref().to_vec();
        let mut auth = [0u8; 16];
        rng.fill(&mut auth).unwrap();
        Self {
            private,
            public,
            auth,
        }
    }

    fn subscription(&self, endpoint: &str) -> Value {
        json!({
            "endpoint": endpoint,
            "expirationTime": null,
            "keys": {"p256dh": URL_SAFE_NO_PAD.encode(&self.public), "auth": URL_SAFE_NO_PAD.encode(self.auth)},
        })
    }

    /// What the phone's browser does with a push body: the header read, the shared secret from the key it names,
    /// the record opened and its 0x02 delimiter checked.
    fn open(self, body: &[u8]) -> Vec<u8> {
        let salt = &body[..16];
        let record_size = u32::from_be_bytes(body[16..20].try_into().unwrap());
        assert_eq!(record_size, webpush::RECORD_SIZE);
        let id_length = body[20] as usize;
        assert_eq!(
            id_length, 65,
            "the key id is the sender's 65-octet public key"
        );
        let as_public = &body[21..21 + id_length];
        let ecdh = agreement::agree_ephemeral(
            self.private,
            &agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, as_public),
            <[u8]>::to_vec,
        )
        .unwrap();
        let (cek, nonce) =
            webpush::derive(&ecdh, &self.auth, &self.public, as_public, salt).unwrap();
        let key = ring::aead::LessSafeKey::new(
            ring::aead::UnboundKey::new(&ring::aead::AES_128_GCM, &cek).unwrap(),
        );
        let mut record = body[21 + id_length..].to_vec();
        let plain = key
            .open_in_place(
                ring::aead::Nonce::assume_unique_for_key(nonce),
                ring::aead::Aad::empty(),
                &mut record,
            )
            .unwrap()
            .to_vec();
        assert_eq!(plain.last(), Some(&2), "the last record's delimiter");
        plain[..plain.len() - 1].to_vec()
    }
}

// --- The site ------------------------------------------------------------------------------------------------------

struct World {
    dir: PathBuf,
    herdr: Arc<FakeHerdr>,
    push: FakePush,
    site: Arc<Site>,
    alerts: Arc<Alerts>,
}

fn world(name: &str, welcome: bool) -> World {
    let dir = scratch(name);
    let herdr = FakeHerdr::new();
    let push = FakePush::start();
    let mut opened = Alerts::open(&dir, Hosts::Local(push.port), Duration::from_secs(3)).unwrap();
    opened.welcome = welcome;
    let alerts = Arc::new(opened);
    let site = site_with(herdr.clone(), Some(alerts.clone()));
    World {
        dir,
        herdr,
        push,
        site,
        alerts,
    }
}

fn site_with(herdr: Arc<FakeHerdr>, alerts: Option<Arc<Alerts>>) -> Arc<Site> {
    let mut site = Site::new(
        Arc::new(NoTailscale),
        herdr,
        Arc::new(Admit),
        Arc::new(NoProbe),
        super::HTTPS_PORT,
        super::tests_live::fixed_now,
    );
    *site.owner.write().unwrap() = Some(OWNER.into());
    *site.me.write().unwrap() = Some(routes::Me {
        host: HOST.into(),
        ips: vec![DESK_IP.parse().unwrap()],
    });
    site.alerts = alerts;
    let site = Arc::new(site);
    if let Some(alerts) = &site.alerts {
        alerts.attach(site.feed.clone());
    }
    site
}

fn post_json(addr: std::net::SocketAddr, path: &str, body: &Value) -> (u16, Value) {
    let (status, _, text) = post(
        addr,
        path,
        "application/json",
        body.to_string().as_bytes(),
        OWNER,
    );
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn phones_on_disk(dir: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(dir.join(alerts::PHONES_FILE)).unwrap_or_default();
    serde_json::from_str::<Value>(&text).unwrap_or(Value::Null)["phones"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|phone| phone["endpoint"].as_str().unwrap_or("").to_string())
        .collect()
}

/// Until the watcher has subscribed to herdr's events through the feed.
fn until_subscribed(herdr: &FakeHerdr) {
    let started = Instant::now();
    while herdr.subscribed() == 0 && started.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(herdr.subscribed() > 0, "the alerts never listened to herdr");
    // The feed takes its second snapshot after subscribing; a change before it is a change it already saw.
    std::thread::sleep(Duration::from_millis(300));
}

// --- RFC 8291 and RFC 8292 -----------------------------------------------------------------------------------------

#[test]
fn rfc_8291_appendix_a_is_reproduced() {
    // RFC 8291, Appendix A: the inputs and the intermediate values, base64url.
    let ecdh = b64(b"kyrL1jIIOHEzg3sM2ZWRHDRB62YACZhhSlknJ672kSs");
    let auth = b64(b"BTBZMqHH6r4Tts7J_aSIgg");
    let ua_public = b64(
        b"BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
    );
    let as_public = b64(
        b"BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8",
    );
    let salt: [u8; 16] = b64(b"DGv6ra1nlYgDCS1FRnbzlw").try_into().unwrap();
    let (cek, nonce) = webpush::derive(&ecdh, &auth, &ua_public, &as_public, &salt).unwrap();
    assert_eq!(cek.to_vec(), b64(b"oIhVW04MRdy2XN9CiKLxTg"), "CEK");
    assert_eq!(nonce.to_vec(), b64(b"4h_95klXJ5E_qnoN"), "NONCE");
    let head = webpush::header(&salt, webpush::RECORD_SIZE, &as_public);
    assert_eq!(
        head,
        b64(b"DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8"),
        "the 86-octet header"
    );
    let record = webpush::seal(&cek, nonce, b"When I grow up, I want to be a watermelon").unwrap();
    assert_eq!(
        record,
        b64(b"8pfeW0KbunFT06SuDKoJH9Ql87S1QUrdirN6GcG7sFz1y1sqLgVi1VhjVkHsUoEsbI_0LpXMuGvnzQ"),
        "the ciphertext"
    );
}

#[test]
fn a_push_body_opens_with_the_phones_key_only() {
    let ua = Ua::new();
    let body = webpush::encrypt(&ua.public, &ua.auth, b"hello phone").unwrap();
    // Each message has its own key and salt.
    let again = webpush::encrypt(&ua.public, &ua.auth, b"hello phone").unwrap();
    assert_ne!(body[..16], again[..16], "a fresh salt");
    assert_ne!(body[21..86], again[21..86], "a fresh sender key");
    assert_eq!(ua.open(&body), b"hello phone");
    // Another phone's key cannot open it.
    let other = Ua::new();
    let opened = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| other.open(&body)));
    assert!(opened.is_err(), "another phone opened the push");
    assert!(
        webpush::encrypt(&[4u8; 65], &[0u8; 16], b"x").is_err(),
        "a point off the curve"
    );
    assert!(
        webpush::encrypt(&ua_public_of(), &[0u8; 16], &[b'x'; 3994]).is_err(),
        "over 3993 bytes"
    );
}

fn ua_public_of() -> Vec<u8> {
    Ua::new().public
}

#[test]
fn the_vapid_header_is_an_es256_token_for_the_push_services_origin() {
    let vapid = Vapid::from_pkcs8(&Vapid::generate().unwrap()).unwrap();
    let now = 1_800_000_000;
    let header = vapid
        .authorization("https://web.push.apple.com", now)
        .unwrap();
    let rest = header.strip_prefix("vapid t=").expect("the vapid scheme");
    let (token, key) = rest.split_once(", k=").unwrap();
    assert_eq!(key, vapid.public_b64());
    assert_eq!(b64(key.as_bytes()).len(), 65);
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    let head: Value = serde_json::from_slice(&b64(parts[0].as_bytes())).unwrap();
    assert_eq!(head, json!({"typ": "JWT", "alg": "ES256"}));
    let claims: Value = serde_json::from_slice(&b64(parts[1].as_bytes())).unwrap();
    assert_eq!(claims["aud"], "https://web.push.apple.com");
    assert_eq!(claims["exp"], now + webpush::TOKEN_FOR);
    assert_eq!(claims["sub"], webpush::SUBJECT);
    let signature = b64(parts[2].as_bytes());
    assert_eq!(signature.len(), 64, "ES256 is r || s, 64 octets");
    ring::signature::UnparsedPublicKey::new(
        &ring::signature::ECDSA_P256_SHA256_FIXED,
        vapid.public_key(),
    )
    .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
    .expect("the token verifies with the served key");
    assert!(!format!("{vapid:?}").contains("PRIVATE"));
}

// --- Which endpoints, which keys -----------------------------------------------------------------------------------

#[test]
fn only_a_push_services_https_endpoint_is_taken() {
    let known = Hosts::PushServices;
    for good in [
        "https://web.push.apple.com/QGuQyavXutnMB-0\u{2014}x".replace('\u{2014}', "z"),
        "https://WEB.PUSH.APPLE.COM/abc".to_string(),
        "https://fcm.googleapis.com/fcm/send/abc:def".to_string(),
        "https://updates.push.services.mozilla.com/wpush/v2/gAAAA".to_string(),
        "https://web.push.apple.com:443/abc".to_string(),
    ] {
        let url = alerts::endpoint(&good, known).unwrap_or_else(|why| panic!("{good}: {why}"));
        assert_eq!(url.scheme(), "https");
    }
    for bad in [
        "http://web.push.apple.com/abc",
        "https://web.push.apple.com.evil.example/abc",
        "https://evil.example/web.push.apple.com/abc",
        "https://web.push.apple.com@evil.example/abc",
        "https://user@web.push.apple.com/abc",
        "https://user:pw@web.push.apple.com/abc",
        "https://web.push.apple.com:8443/abc",
        "https://web.push.apple.com/abc?to=http://127.0.0.1",
        "https://web.push.apple.com/abc#x",
        "https://web.push.apple.com/",
        "https://web.push.apple.com",
        "https://127.0.0.1/abc",
        "https://[::1]/abc",
        "https://localhost/abc",
        "http://127.0.0.1:7790/api/send",
        "file:///C:/Windows/win.ini",
        "web.push.apple.com/abc",
        "",
    ] {
        assert!(alerts::endpoint(bad, known).is_err(), "{bad} was taken");
    }
    let long = format!("https://web.push.apple.com/{}", "a".repeat(3000));
    assert!(alerts::endpoint(&long, known).is_err());
    // A test's own push service only through the test variant, and only its port.
    assert!(alerts::endpoint("http://127.0.0.1:5555/push/a", known).is_err());
    assert!(alerts::endpoint("http://127.0.0.1:5555/push/a", Hosts::Local(5555)).is_ok());
    assert!(alerts::endpoint("http://127.0.0.1:5556/push/a", Hosts::Local(5555)).is_err());
}

#[test]
fn a_phones_keys_are_checked() {
    let ua = Ua::new();
    let p256dh = URL_SAFE_NO_PAD.encode(&ua.public);
    let auth = URL_SAFE_NO_PAD.encode(ua.auth);
    assert!(alerts::keys(&p256dh, &auth).is_ok());
    assert!(
        alerts::keys(&format!("{p256dh}="), &format!("{auth}==")).is_ok(),
        "padding is fine"
    );
    let mut off = ua.public.clone();
    off[64] ^= 1;
    assert!(
        alerts::keys(&URL_SAFE_NO_PAD.encode(&off), &auth).is_err(),
        "off the curve"
    );
    let mut compressed = ua.public.clone();
    compressed[0] = 2;
    assert!(
        alerts::keys(&URL_SAFE_NO_PAD.encode(&compressed), &auth).is_err(),
        "not 0x04"
    );
    assert!(alerts::keys(&URL_SAFE_NO_PAD.encode(&ua.public[..64]), &auth).is_err());
    assert!(alerts::keys(&p256dh, &URL_SAFE_NO_PAD.encode([0u8; 15])).is_err());
    assert!(alerts::keys("not base64!", &auth).is_err());
}

// --- What is a change ----------------------------------------------------------------------------------------------

fn list(states: &[(&str, &str)]) -> Value {
    let agents: Vec<Value> = states
        .iter()
        .map(|(id, status)| json!({"id": id, "name": format!("agent {id}"), "status": status}))
        .collect();
    json!({"spaces": [{"id": "w1", "name": "Nirvana-Herdr", "agents": agents}]})
}

#[test]
fn done_and_needs_you_are_alerts_and_working_and_idle_are_not() {
    let before = list(&[
        ("a", "working"),
        ("b", "working"),
        ("c", "idle"),
        ("d", "done"),
        ("e", "needs"),
        ("f", "working"),
    ]);
    let after = list(&[
        ("a", "done"),
        ("b", "needs"),
        ("c", "working"),
        ("d", "idle"),
        ("e", "needs"),
        ("f", "working"),
        ("g", "needs"),
    ]);
    let alerts = alerts::changes(&before, &after);
    assert_eq!(
        alerts,
        vec![
            Alert {
                space: "Nirvana-Herdr".into(),
                agent: "a".into(),
                name: "agent a".into(),
                state: "done"
            },
            Alert {
                space: "Nirvana-Herdr".into(),
                agent: "b".into(),
                name: "agent b".into(),
                state: "needs"
            },
        ],
        "a new agent (g) and an unchanged one (e) are not changes"
    );
    let body: Value = serde_json::from_slice(&alerts::payload(&alerts[1])).unwrap();
    assert_eq!(
        body,
        json!({"title": "Nirvana-Herdr", "body": "agent b needs you", "agent": "b", "name": "agent b",
               "space": "Nirvana-Herdr", "state": "needs"}),
        "the body holds the space, the agent and its state, and nothing else"
    );
    let done: Value = serde_json::from_slice(&alerts::payload(&alerts[0])).unwrap();
    assert_eq!(done["body"], "agent a is done");
}

#[test]
fn a_flap_within_the_window_sends_once() {
    let mut debounce = Debounce::new(Duration::from_secs(5));
    let done = Alert {
        space: "s".into(),
        agent: "p1".into(),
        name: "n".into(),
        state: "done",
    };
    let needs = Alert {
        state: "needs",
        ..done.clone()
    };
    let t0 = Instant::now();
    assert!(debounce.allow(&done, t0));
    assert!(
        !debounce.allow(&done, t0 + Duration::from_secs(2)),
        "done again 2 s later"
    );
    assert!(
        debounce.allow(&needs, t0 + Duration::from_secs(2)),
        "another state is news"
    );
    assert!(
        debounce.allow(&done, t0 + Duration::from_secs(6)),
        "after the window"
    );
    let other = Alert {
        agent: "p2".into(),
        ..done.clone()
    };
    assert!(
        debounce.allow(&other, t0 + Duration::from_secs(6)),
        "another agent"
    );
}

// --- The whole way, against the fakes ------------------------------------------------------------------------------

#[test]
fn an_agent_turning_done_or_needing_you_pushes_each_phone_and_working_does_not() {
    let w = world("push", false);
    let running = serve(w.site.clone());
    let ua = Ua::new();
    let (status, answer) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &ua.subscription(&w.push.url("phone-1")),
    );
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["new"], true);
    until_subscribed(&w.herdr);
    assert!(w.push.got().is_empty(), "nothing before a change");

    w.herdr.set("p1", "done");
    let got = w.push.wait_for(1, Duration::from_secs(5));
    assert_eq!(got.len(), 1, "one push on done: {got:?}");
    let push = &got[0];
    assert_eq!(push.path, "/push/phone-1");
    assert_eq!(push.headers["content-encoding"], "aes128gcm");
    assert_eq!(push.headers["ttl"], alerts::TTL_SECS.to_string());
    assert_eq!(push.headers["urgency"], "normal");
    let authorization = &push.headers["authorization"];
    assert!(authorization.starts_with("vapid t="), "{authorization}");
    assert!(authorization.ends_with(&format!(", k={}", w.alerts.public_key())));
    let token = authorization["vapid t=".len()..].split(',').next().unwrap();
    let claims: Value =
        serde_json::from_slice(&b64(token.split('.').nth(1).unwrap().as_bytes())).unwrap();
    assert_eq!(
        claims["aud"],
        format!("http://127.0.0.1:{}", w.push.port),
        "the push service's origin"
    );
    let plain: Value = serde_json::from_slice(&ua.open(&push.body)).unwrap();
    assert_eq!(plain["title"], "Nirvana-Herdr");
    assert_eq!(plain["body"], "nirvana is done");
    assert_eq!(plain["agent"], "p1");
    assert_eq!(plain["state"], "done");

    w.herdr.set("p3", "working");
    w.herdr.set("p2", "idle");
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(w.push.got().len(), 1, "working and idle send nothing");

    w.herdr.set("p2", "blocked");
    let got = w.push.wait_for(2, Duration::from_secs(5));
    assert_eq!(got.len(), 2, "one push on needs you");
    assert_eq!(got[1].headers["urgency"], "high");
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_flap_sends_one_push() {
    let w = world("flap", false);
    let running = serve(w.site.clone());
    let ua = Ua::new();
    let (status, _) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &ua.subscription(&w.push.url("phone-1")),
    );
    assert_eq!(status, 200);
    until_subscribed(&w.herdr);
    w.herdr.set("p1", "done");
    assert_eq!(w.push.wait_for(1, Duration::from_secs(5)).len(), 1);
    std::thread::sleep(Duration::from_millis(400));
    w.herdr.set("p1", "working");
    std::thread::sleep(Duration::from_millis(400));
    w.herdr.set("p1", "done");
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        w.push.got().len(),
        1,
        "done, working, done within the window is one push"
    );
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_phone_its_push_service_calls_gone_is_dropped_and_the_others_kept() {
    let w = world("gone", false);
    let running = serve(w.site.clone());
    for name in ["phone-ok", "phone-gone", "phone-broken"] {
        let (status, answer) = post_json(
            running.addr,
            "/api/alerts/subscribe",
            &Ua::new().subscription(&w.push.url(name)),
        );
        assert_eq!(status, 200, "{answer}");
    }
    assert_eq!(phones_on_disk(&w.dir).len(), 3);
    until_subscribed(&w.herdr);
    w.herdr.set("p1", "done");
    assert_eq!(
        w.push.wait_for(3, Duration::from_secs(5)).len(),
        3,
        "one push to each phone"
    );
    let started = Instant::now();
    while w.alerts.count() != 2 && started.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(w.alerts.count(), 2, "410 drops the phone; a 500 does not");
    let kept = phones_on_disk(&w.dir);
    assert!(kept.iter().any(|e| e.ends_with("phone-ok")), "{kept:?}");
    assert!(kept.iter().any(|e| e.ends_with("phone-broken")), "{kept:?}");
    assert!(!kept.iter().any(|e| e.ends_with("phone-gone")), "{kept:?}");
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_new_phone_gets_one_welcome_push() {
    let w = world("welcome", true);
    let running = serve(w.site.clone());
    let ua = Ua::new();
    let subscription = ua.subscription(&w.push.url("phone-1"));
    let (status, answer) = post_json(running.addr, "/api/alerts/subscribe", &subscription);
    assert_eq!((status, answer["new"].clone()), (200, json!(true)));
    let got = w.push.wait_for(1, Duration::from_secs(5));
    assert_eq!(got.len(), 1, "the welcome push");
    let plain: Value = serde_json::from_slice(&ua.open(&got[0].body)).unwrap();
    assert_eq!(plain["title"], "Nirvana");
    assert_eq!(plain["agent"], "");
    // The same phone again (the page tells the PC each time it opens) is not new and gets nothing.
    let (status, answer) = post_json(running.addr, "/api/alerts/subscribe", &subscription);
    assert_eq!((status, answer["new"].clone()), (200, json!(false)));
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(w.push.got().len(), 1);
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_stopped_nirvana_remote_sends_no_more_alerts() {
    let w = world("stop", false);
    let running = serve(w.site.clone());
    let (status, _) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &Ua::new().subscription(&w.push.url("phone-1")),
    );
    assert_eq!(status, 200);
    until_subscribed(&w.herdr);
    w.alerts.stop();
    std::thread::sleep(Duration::from_millis(1300));
    w.herdr.set("p1", "done");
    std::thread::sleep(Duration::from_millis(1200));
    assert!(w.push.got().is_empty(), "a push after stop");
    drop(running);
}

// --- The routes ------------------------------------------------------------------------------------------------------

#[test]
fn subscribing_takes_a_push_services_endpoint_only_and_at_most_eight_phones() {
    let w = world("routes", false);
    let running = serve(w.site.clone());
    let ua = Ua::new();
    for bad in [
        "https://evil.example/push".to_string(),
        "http://127.0.0.1:7790/api/send".to_string(),
        format!("http://127.0.0.1:{}/", w.push.port),
        format!("http://localhost:{}/push/x", w.push.port),
        format!("http://127.0.0.1:{}/push/x?y", w.push.port),
        "https://web.push.apple.com/abc".to_string(),
    ] {
        let (status, answer) = post_json(
            running.addr,
            "/api/alerts/subscribe",
            &ua.subscription(&bad),
        );
        assert_eq!(status, 400, "{bad}: {answer}");
    }
    let mut keys_bad = ua.subscription(&w.push.url("phone-x"));
    keys_bad["keys"]["auth"] = json!("AAAA");
    assert_eq!(
        post_json(running.addr, "/api/alerts/subscribe", &keys_bad).0,
        400
    );
    assert_eq!(w.alerts.count(), 0);
    assert!(phones_on_disk(&w.dir).is_empty());

    for n in 0..alerts::MOST {
        let (status, answer) = post_json(
            running.addr,
            "/api/alerts/subscribe",
            &Ua::new().subscription(&w.push.url(&format!("phone-{n}"))),
        );
        assert_eq!(status, 200, "{answer}");
        assert_eq!(answer["phones"], n + 1);
    }
    let (status, answer) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &Ua::new().subscription(&w.push.url("phone-9")),
    );
    assert_eq!(status, 409, "the ninth: {answer}");
    // A phone already there may update its keys at the cap.
    let (status, answer) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &Ua::new().subscription(&w.push.url("phone-0")),
    );
    assert_eq!((status, answer["new"].clone()), (200, json!(false)));
    assert_eq!(phones_on_disk(&w.dir).len(), alerts::MOST);

    let (status, answer) = post_json(
        running.addr,
        "/api/alerts/unsubscribe",
        &json!({"endpoint": w.push.url("phone-3")}),
    );
    assert_eq!((status, answer["removed"].clone()), (200, json!(true)));
    assert_eq!(phones_on_disk(&w.dir).len(), alerts::MOST - 1);
    let (_, answer) = post_json(
        running.addr,
        "/api/alerts/unsubscribe",
        &json!({"endpoint": w.push.url("phone-3")}),
    );
    assert_eq!(answer["removed"], false);

    let (status, _, _) = post(
        running.addr,
        "/api/alerts/subscribe",
        "text/plain",
        b"{}",
        OWNER,
    );
    assert_eq!(status, 415);
    let (status, _, _) = get(running.addr, "/api/alerts/subscribe", OWNER);
    assert_eq!(status, 405);
    assert!(
        w.push.got().is_empty(),
        "no push was sent by subscribing (welcome off)"
    );
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_refused_caller_gets_nothing_from_the_alerts_routes() {
    let w = world("refused", true);
    let running = serve(w.site.clone());
    let body = Ua::new().subscription(&w.push.url("phone-1")).to_string();
    let length = body.len();
    let other_host = format!(
        "POST /api/alerts/subscribe HTTP/1.1\r\nHost: evil.example:8443\r\nX-Forwarded-For: {}\r\n\
         Tailscale-User-Login: {OWNER}\r\n{}Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
        super::tests_live::PHONE,
        from_page(),
    );
    let tries = [
        format!(
            "{}{}Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
            proxied("POST", "/api/alerts/subscribe", "someone@else.example"),
            from_page()
        ),
        format!(
            "POST /api/alerts/subscribe HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {}\r\n{}\
             Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
            super::tests_live::PHONE,
            from_page()
        ),
        format!(
            "POST /api/alerts/subscribe HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {DESK_IP}\r\n\
             Tailscale-User-Login: {OWNER}\r\n{}Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
            from_page()
        ),
        format!(
            "{}Sec-Fetch-Site: cross-site\r\nOrigin: https://evil.example\r\nContent-Type: application/json\r\n\
             Content-Length: {length}\r\n\r\n{body}",
            proxied("POST", "/api/alerts/subscribe", OWNER)
        ),
        format!(
            "{}Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{body}",
            proxied("POST", "/api/alerts/subscribe", OWNER)
        ),
        other_host,
        format!(
            "{}Sec-Fetch-Site: cross-site\r\n\r\n",
            proxied("GET", "/api/alerts", OWNER)
        ),
        format!("{}\r\n", proxied("GET", "/api/alerts", "someone@else.example")),
        format!("{}\r\n", proxied("GET", "/sw.js", "someone@else.example")),
    ];
    for request in tries {
        let (status, _, text) = raw(running.addr, request.as_bytes());
        assert!(status == 403 || status == 421, "{status} for {request}");
        assert_eq!(text, REFUSED, "{request}");
        assert!(!text.contains(&w.alerts.public_key()));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(w.alerts.count(), 0, "no phone was added");
    assert!(phones_on_disk(&w.dir).is_empty());
    assert!(w.push.got().is_empty(), "nothing was pushed");
    drop(running);
    w.alerts.stop();
}

#[test]
fn the_key_is_made_once_kept_private_and_only_its_public_half_is_served() {
    let w = world("key", false);
    let first = w.alerts.public_key();
    let again = Alerts::open(&w.dir, Hosts::Local(w.push.port), Duration::from_secs(1)).unwrap();
    assert_eq!(again.public_key(), first, "the same key after a restart");
    let file = std::fs::read_to_string(w.dir.join(alerts::KEY_FILE)).unwrap();
    let running = serve(w.site.clone());
    let (status, _, text) = get(running.addr, "/api/alerts", OWNER);
    assert_eq!(status, 200);
    let answer: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(answer["key"], first);
    assert_eq!(answer["most"], alerts::MOST);
    assert!(!text.contains(file.trim()), "the private key in an answer");
    for path in ["/sw.js", "/alerts.js", "/manifest.webmanifest", "/"] {
        let (_, _, text) = get(running.addr, path, OWNER);
        assert!(!text.contains(file.trim()), "the private key in {path}");
    }
    // Phones are kept across a restart too.
    let (status, _) = post_json(
        running.addr,
        "/api/alerts/subscribe",
        &Ua::new().subscription(&w.push.url("phone-1")),
    );
    assert_eq!(status, 200);
    let reopened = Alerts::open(&w.dir, Hosts::Local(w.push.port), Duration::from_secs(1)).unwrap();
    assert_eq!(reopened.count(), 1);
    #[cfg(windows)]
    for name in [alerts::KEY_FILE, alerts::PHONES_FILE] {
        let dacl = dacl_of(&w.dir.join(name));
        assert_eq!(
            dacl, "D:P(A;;FA;;;SY)(A;;FA;;;OW)",
            "{name}: protected, SYSTEM and the owner only"
        );
    }
    // Without alerts set up, the routes say so and serve no key.
    let bare = site_with(FakeHerdr::new(), None);
    let bare_running = serve(bare);
    let (status, _, _) = get(bare_running.addr, "/api/alerts", OWNER);
    assert_eq!(status, 409);
    drop(running);
    w.alerts.stop();
}

/// A file's DACL as SDDL, read back from the file system.
#[cfg(windows)]
fn dacl_of(path: &Path) -> String {
    use interprocess::os::windows::security_descriptor::{
        AsSecurityDescriptorExt, BorrowedSecurityDescriptor,
    };
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::{GetFileSecurityW, DACL_SECURITY_INFORMATION};
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut needed = 0u32;
    // SAFETY: a NUL-terminated path; the first call asks for the size only.
    unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    assert!(needed > 0, "no security descriptor for {}", path.display());
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    // SAFETY: the buffer is `needed` bytes or more, aligned for a descriptor.
    let ok = unsafe {
        GetFileSecurityW(
            wide.as_ptr(),
            DACL_SECURITY_INFORMATION,
            buffer.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    assert_ne!(ok, 0, "GetFileSecurityW");
    // SAFETY: GetFileSecurityW wrote a self-relative descriptor into `buffer`, alive for this borrow.
    let descriptor = unsafe { BorrowedSecurityDescriptor::from_ptr(buffer.as_ptr().cast()) };
    descriptor
        .serialize(DACL_SECURITY_INFORMATION, |sddl| sddl.to_string_lossy())
        .unwrap()
}

// --- The home-screen app ---------------------------------------------------------------------------------------------

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "a PNG");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

/// A GET's head and its body as bytes.
fn get_bytes(addr: std::net::SocketAddr, path: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(format!("{}\r\n", proxied("GET", path, OWNER)).as_bytes())
        .unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).unwrap();
    super::tests_live::whole(&answer, false);
    let end = answer.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&answer[..end]).to_string(),
        answer[end + 4..].to_vec(),
    )
}

#[test]
fn the_home_screen_app_is_served_with_its_manifest_icons_and_worker() {
    let w = world("app", false);
    let running = serve(w.site.clone());
    let (head, body) = get_bytes(running.addr, "/manifest.webmanifest");
    assert!(
        head.contains("200 OK") && head.contains("application/manifest+json"),
        "{head}"
    );
    assert!(
        head.contains("Content-Security-Policy: default-src 'self'"),
        "{head}"
    );
    let manifest: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(manifest["name"], "Nirvana");
    assert_eq!(manifest["display"], "standalone");
    assert_eq!(manifest["start_url"], "/");
    assert_eq!(manifest["scope"], "/");
    assert_eq!(manifest["background_color"], "#121416");
    assert_eq!(manifest["theme_color"], "#121416");
    for icon in manifest["icons"].as_array().unwrap() {
        let (head, png) = get_bytes(running.addr, icon["src"].as_str().unwrap());
        assert!(head.contains("image/png"), "{head}");
        let (width, height) = png_size(&png);
        assert_eq!(
            format!("{width}x{height}"),
            icon["sizes"],
            "{}",
            icon["src"]
        );
    }
    let (_, touch) = get_bytes(running.addr, "/apple-touch-icon.png");
    assert_eq!(png_size(&touch), (180, 180), "iOS's home-screen size");
    // Full-bleed (no alpha): iOS fills a transparent corner with black. Colour type 2 is RGB.
    assert_eq!(touch[25], 2, "an RGB PNG, no alpha");

    let (status, _, page) = get(running.addr, "/", OWNER);
    assert_eq!(status, 200);
    for tag in [
        r#"<link rel="manifest" href="/manifest.webmanifest">"#,
        r#"<link rel="apple-touch-icon" href="/apple-touch-icon.png">"#,
        r#"<meta name="apple-mobile-web-app-capable" content="yes">"#,
        r#"<meta name="apple-mobile-web-app-title" content="Nirvana">"#,
        r#"<script src="/alerts.js"></script>"#,
    ] {
        assert!(page.contains(tag), "{tag}");
    }

    let (head, sw) = get_bytes(running.addr, "/sw.js");
    assert!(head.contains("text/javascript"), "{head}");
    let sw = String::from_utf8(sw).unwrap();
    assert!(
        !sw.contains("'fetch'") && !sw.contains("\"fetch\""),
        "no fetch handler: it caches nothing"
    );
    assert!(
        !sw.contains("caches.") && !sw.contains("cache.put"),
        "no Cache Storage"
    );
    assert!(sw.contains("showNotification"));
    assert!(
        sw.contains("'/#chat?' + new URLSearchParams(open)"),
        "a tap opens the agent's chat"
    );
    let (status, _, _) = raw(
        running.addr,
        format!(
            "{}{}Content-Length: 0\r\n\r\n",
            proxied("POST", "/sw.js", OWNER),
            from_page()
        )
        .as_bytes(),
    );
    assert_eq!(status, 405);

    let (_, script) = get_bytes(running.addr, "/alerts.js");
    let script = String::from_utf8(script).unwrap();
    assert!(script.contains("alerts work only from the home-screen app (iOS 16.4 or later)"));
    assert!(script.contains("register('/sw.js', { scope: '/' })"));
    assert!(script.contains("userVisibleOnly: true, applicationServerKey: bytes(pc)"));
    assert!(script.contains("postJson('/api/alerts/subscribe', subscription.toJSON())"));
    assert!(
        script.contains("postJson('/api/alerts/unsubscribe', { endpoint: subscription.endpoint })")
    );
    // The permission prompt is asked for in the tap itself, before anything is awaited (iOS).
    let click = script
        .split("toggle.addEventListener('click'")
        .nth(1)
        .unwrap();
    let click = &click[..click.find("});").unwrap()];
    let asked = click
        .find("Notification.requestPermission()")
        .expect("asked in the tap");
    let before: Vec<&str> = click[..asked]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    assert!(
        !before.iter().any(|line| line.contains("await")),
        "an await before the prompt: {click}"
    );
    let (_, app) = get_bytes(running.addr, "/app.js");
    let app = String::from_utf8(app).unwrap();
    assert!(app.contains("if (name !== 'chat' || !query || !has('chat')) return null;"));
    drop(running);
    w.alerts.stop();
}

#[test]
fn a_tapped_alert_links_to_its_agents_chat() {
    // The link the worker opens is the one the page boots into: `#chat?agent=..&name=..&space=..`.
    let sw = include_str!("page/sw.js");
    assert!(sw.contains("const open = data.agent ? { agent: data.agent, name: data.name || '', space: data.space || '' } : null;"));
    let app = include_str!("page/app.js");
    assert!(app.contains("const agent = asked.get('agent') || '';"));
    assert!(app.contains("history.pushState({ name: 'chat', params: chat }, '', '#chat');"));
    // What the worker reads is what payload writes.
    let alert = Alert {
        space: "S".into(),
        agent: "p7".into(),
        name: "N".into(),
        state: "done",
    };
    let body: Value = serde_json::from_slice(&alerts::payload(&alert)).unwrap();
    for field in ["title", "body", "agent", "name", "space"] {
        assert!(body.get(field).is_some(), "{field}");
        assert!(
            sw.contains(&format!("data.{field}")),
            "the worker reads data.{field}"
        );
    }
}
