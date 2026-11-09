//! Plan 39 P39e, §2.7: alerts (prototype screen 7). When an agent turns done or needs you, a Web Push goes to each
//! phone that turned alerts on: the space's name as the title, the agent and its state as the body, and a tap opens
//! that agent's chat. Working and idle send nothing.
//!
//! **Where a change comes from.** The same one `events.subscribe` the spaces screen lives on (`spaces::Feed`): while
//! any phone has alerts on, this module is one more listener on it, so herdr is asked nothing new and nothing on a
//! timer, and the feed keeps running with no page open. Each list it sends is compared with the last (`changes`): an
//! agent that was in the list before with another state and is now done or needs you is one alert. A flap (done,
//! working, done again within `DEBOUNCE`) sends once (`Debounce`).
//!
//! **Sending** is a plain HTTPS POST with the ureq already in the tree, on this module's own thread (never on the
//! async runtime), one thread per phone so an alert takes `PUSH_TIMEOUT` at most, no redirect followed. A push
//! service that answers 404 or 410 has dropped the phone's subscription, and it is dropped here too; one that refuses
//! this PC's token twice in a row (`REJECTED_MOST`) is treated the same way.
//!
//! **Only the owner's phones, and only push services.** Subscribing is a POST behind every check in `Site::route`
//! (P39a and P39b: the Tailscale service's connection, the owner's login, another device, this PC's Host, and from
//! this page). The endpoint it names is where this PC will POST, so it must be an `https` URL on one of
//! `PUSH_HOSTS`, with no port, user or query (`endpoint`): the page can never make this PC POST to an address of its
//! choosing (SSRF). At most `MOST` phones are kept.
//!
//! **Keys and phones stay on the PC**, beside `nirvana-remote.json` in the session's data dir, outside every
//! repository: `KEY_FILE` (the VAPID private key, PKCS#8) and `PHONES_FILE` (each phone's endpoint and keys). Both
//! are created with the DACL herdr's own pipe has (`PRIVATE_SDDL`): protected from inheritance, SYSTEM and the
//! file's owner only. The key is read into `webpush::Vapid`, whose `Debug` prints the public half; no route answers
//! anything but the public key, and no log line names a key or an endpoint's path.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use super::http::{Conn, Framing, Request, Response};
use super::routes::Site;
use super::spaces::Feed;
use super::webpush::{self, Vapid};

/// The push services a phone's browser subscribes through: Apple's (Safari and every iPhone browser), Google's FCM
/// (Chrome, Edge on Android), Mozilla's autopush (Firefox). Nothing else is ever POSTed to.
pub const PUSH_HOSTS: &[&str] = &[
    "web.push.apple.com",
    "fcm.googleapis.com",
    "updates.push.services.mozilla.com",
];
/// At most this many phones get alerts. One owner; a few phones and browsers, and the dead ones a reinstall leaves
/// until their push service says 410.
pub const MOST: usize = 8;
/// A subscribe or unsubscribe body is never longer than this (an endpoint is under 1 KiB).
pub const BODY_MOST: u64 = 4 * 1024;
const ENDPOINT_MOST: usize = 2048;
/// The same agent and state again within this long is the same alert: a flap sends once.
pub const DEBOUNCE: Duration = Duration::from_secs(5);
/// One push waits at most this long for its push service. An alert's phones are sent to at once (`push_all`), so
/// this is also about the longest one alert takes.
pub const PUSH_TIMEOUT: Duration = Duration::from_secs(10);
/// A phone whose push service answers 401 or 403 this many times in a row is dropped: its subscription was made with
/// a key the push service no longer takes (a new `KEY_FILE`, or one the service refuses), and every later alert to it
/// would be refused too. Twice, not once, so one bad answer does not end a phone.
pub const REJECTED_MOST: u8 = 2;
/// How long a push service keeps an alert for a phone that is off: an alert older than this is no news.
pub const TTL_SECS: u64 = 60 * 60;
/// How often the watcher looks at `stop` and at whether any phone is left.
const WAKE: Duration = Duration::from_secs(1);
/// After the feed could not be listened to (herdr not answering), it is asked again after this.
const RETRY: Duration = Duration::from_secs(5);
pub const KEY_FILE: &str = "nirvana-remote-vapid.key";
pub const PHONES_FILE: &str = "nirvana-remote-alerts.json";
/// The DACL of herdr's own pipe (`src/platform/windows.rs`, `PrivateLocalListener::bind`): protected, so nothing
/// is inherited from the folder, and full access for SYSTEM and the file's owner only.
pub const PRIVATE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;OW)";

/// Which endpoints may be POSTed to. A build has the push services only; a test may name its own fake push service on
/// 127.0.0.1, and that variant does not exist outside tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hosts {
    PushServices,
    #[cfg(test)]
    Local(u16),
}

/// A phone's endpoint, if this PC may POST to it: `https`, a host in `PUSH_HOSTS` exactly, the default port, no
/// user, password, query or fragment. The URL is returned as parsed, and that is what is stored and POSTed to, so
/// the check and the send read the same URL.
pub fn endpoint(text: &str, hosts: Hosts) -> Result<url::Url, &'static str> {
    if text.len() > ENDPOINT_MOST {
        return Err("an endpoint that long is not a push service's");
    }
    let url = url::Url::parse(text).map_err(|_| "the endpoint is not a URL")?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("an endpoint has no user in it");
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("an endpoint has no query");
    }
    let host = url.host_str().unwrap_or("");
    let known = match hosts {
        Hosts::PushServices => {
            url.scheme() == "https" && url.port().is_none() && PUSH_HOSTS.contains(&host)
        }
        #[cfg(test)]
        Hosts::Local(port) => {
            url.scheme() == "http" && host == "127.0.0.1" && url.port() == Some(port)
        }
    };
    if !known {
        return Err("the endpoint is not a known push service");
    }
    if url.path().len() <= 1 {
        return Err("the endpoint has no subscription in it");
    }
    Ok(url)
}

fn unpad(text: &str) -> Result<Vec<u8>, ()> {
    URL_SAFE_NO_PAD
        .decode(text.trim().trim_end_matches('='))
        .map_err(|_| ())
}

/// A phone's keys from its subscription, checked: `p256dh` a P-256 point (65 octets, uncompressed, on the curve),
/// `auth` 16 octets.
pub fn keys(p256dh: &str, auth: &str) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    let public = unpad(p256dh).map_err(|_| "p256dh is not base64url")?;
    let secret = unpad(auth).map_err(|_| "auth is not base64url")?;
    if public.len() != 65 || public[0] != 4 {
        return Err("p256dh is not an uncompressed P-256 key");
    }
    if secret.len() != 16 {
        return Err("auth is not 16 bytes");
    }
    // A point off the curve fails ECDH; finding out now keeps it out of the store.
    let rng = ring::rand::SystemRandom::new();
    let ours = ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &rng)
        .map_err(|_| "no key to check p256dh with")?;
    let theirs = ring::agreement::UnparsedPublicKey::new(&ring::agreement::ECDH_P256, &public);
    ring::agreement::agree_ephemeral(ours, &theirs, |_| ())
        .map_err(|_| "p256dh is not a P-256 point")?;
    Ok((public, secret))
}

/// One phone that has alerts on, as `PHONES_FILE` keeps it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phone {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    /// Unix seconds it was added.
    pub added: u64,
}

impl Phone {
    /// The same subscription: endpoint and keys (when it was added aside).
    pub fn same(&self, other: &Phone) -> bool {
        self.endpoint == other.endpoint && self.p256dh == other.p256dh && self.auth == other.auth
    }
}

#[derive(Default, Serialize, Deserialize)]
struct PhonesFile {
    phones: Vec<Phone>,
}

/// One alert: an agent that turned done or needs you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub space: String,
    pub agent: String,
    pub name: String,
    /// `done` or `needs`, as the page's chips say.
    pub state: &'static str,
}

/// The alerts between two lists from `spaces::build`: each agent that was in `before` with another state and is now
/// done or needs you. An agent seen for the first time is not a change (its first state is where it starts).
pub fn changes(before: &Value, after: &Value) -> Vec<Alert> {
    let mut was: HashMap<String, String> = HashMap::new();
    for space in before["spaces"].as_array().into_iter().flatten() {
        for agent in space["agents"].as_array().into_iter().flatten() {
            if let (Some(id), Some(status)) = (agent["id"].as_str(), agent["status"].as_str()) {
                was.insert(id.to_string(), status.to_string());
            }
        }
    }
    let mut out = Vec::new();
    for space in after["spaces"].as_array().into_iter().flatten() {
        for agent in space["agents"].as_array().into_iter().flatten() {
            let (Some(id), Some(status)) = (agent["id"].as_str(), agent["status"].as_str()) else {
                continue;
            };
            let state = match status {
                "done" => "done",
                "needs" => "needs",
                _ => continue,
            };
            if was.get(id).is_some_and(|old| old != status) {
                out.push(Alert {
                    space: space["name"].as_str().unwrap_or("space").to_string(),
                    agent: id.to_string(),
                    name: agent["name"].as_str().unwrap_or("agent").to_string(),
                    state,
                });
            }
        }
    }
    out
}

/// The push body the phone's service worker shows (`page/sw.js`). Only what the notification needs: the space's
/// name, the agent's name, its state, and its pane id for the tap. It leaves the tailnet, encrypted, through the push
/// service; nothing from the agent's screen or transcript is in it.
pub fn payload(alert: &Alert) -> Vec<u8> {
    let said = if alert.state == "needs" {
        "needs you"
    } else {
        "is done"
    };
    json!({
        "title": alert.space,
        "body": format!("{} {said}", alert.name),
        "agent": alert.agent,
        "name": alert.name,
        "space": alert.space,
        "state": alert.state,
    })
    .to_string()
    .into_bytes()
}

/// The one push sent when a phone turns alerts on, so the owner sees at once that the whole way works.
pub fn welcome() -> Vec<u8> {
    json!({ "title": "Nirvana", "body": "Alerts are on for this phone.", "agent": "" })
        .to_string()
        .into_bytes()
}

/// One send per agent and state per `window`.
pub struct Debounce {
    window: Duration,
    last: HashMap<(String, &'static str), Instant>,
}

impl Debounce {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            last: HashMap::new(),
        }
    }

    /// Whether `alert` may be sent at `now`; when it may, `now` is its last send.
    pub fn allow(&mut self, alert: &Alert, now: Instant) -> bool {
        let key = (alert.agent.clone(), alert.state);
        if self
            .last
            .get(&key)
            .is_some_and(|at| now.saturating_duration_since(*at) < self.window)
        {
            return false;
        }
        if self.last.len() > 256 {
            let window = self.window;
            self.last
                .retain(|_, at| now.saturating_duration_since(*at) < window);
        }
        self.last.insert(key, now);
        true
    }
}

/// What a push service said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    /// 404 or 410: the subscription is gone (RFC 8030 §7.3), and so is the phone here.
    Gone,
    /// 401 or 403: the push service refused the token (RFC 8292 §4.2); `REJECTED_MOST` in a row drops the phone.
    Rejected(u16),
    Failed(String),
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// A file only SYSTEM and its owner can open (`PRIVATE_SDDL`), made new: it is never an older file reused, whose
/// DACL might be wider.
#[cfg(windows)]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    use interprocess::os::windows::security_descriptor::{
        AsSecurityDescriptorExt, SecurityDescriptor,
    };
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL};

    let sddl = widestring::U16CString::from_str(PRIVATE_SDDL)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let descriptor = SecurityDescriptor::deserialize(&sddl)?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(u32::MAX),
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 0,
    };
    descriptor.write_to_security_attributes(&mut attributes);
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: `wide` is a NUL-terminated path and `attributes` points at `descriptor`, both alive for the call.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a handle CreateFileW just opened, owned by the File from here.
    Ok(unsafe { std::fs::File::from_raw_handle(handle as _) })
}

#[cfg(not(windows))]
fn create_private(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

/// `bytes` into `path`, whole or not at all: written to a new private file beside it, then renamed over it. The
/// renamed file keeps its own DACL.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension(format!("partial-{}", std::process::id()));
    let _ = std::fs::remove_file(&partial);
    let written = create_private(&partial).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&partial, path));
    if renamed.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    renamed
}

/// Whether a file is there, as `Path::try_exists` says; a test hands in one that fails.
pub type Exists = fn(&Path) -> std::io::Result<bool>;

/// This PC's VAPID key from `dir`, made and kept there the first time; `true` with it when it was made just now.
///
/// A new key ends every stored phone's subscription: each was made with the old key as its `applicationServerKey`,
/// and the push service refuses a token signed with another (403). So a key is made only when the file is known to be
/// missing: a file that could not even be looked at (`exists` failed) turns alerts off, as a corrupt one does,
/// rather than a new key being renamed over one that may be fine.
fn load_key(dir: &Path, exists: Exists) -> Result<(Vapid, bool), String> {
    let path = dir.join(KEY_FILE);
    let there = exists(&path)
        .map_err(|err| format!("the alerts key could not be looked at: {}", err.kind()))?;
    if !there {
        let pkcs8 = Vapid::generate()?;
        let text = format!("{}\n", URL_SAFE_NO_PAD.encode(&pkcs8));
        write_private(&path, text.as_bytes())
            .map_err(|err| format!("the alerts key could not be written: {}", err.kind()))?;
        info!("nirvana remote made its alerts key");
    }
    // What the file holds is never put in an error or a log line.
    let text = std::fs::read_to_string(&path)
        .map_err(|err| format!("the alerts key could not be read: {}", err.kind()))?;
    let pkcs8 = unpad(&text).map_err(|()| "the alerts key file is not base64url".to_string())?;
    Vapid::from_pkcs8(&pkcs8).map(|vapid| (vapid, !there))
}

fn load_phones(path: &Path) -> Vec<Phone> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<PhonesFile>(&text).ok())
        .map(|file| file.phones)
        .unwrap_or_default()
}

pub struct Alerts {
    dir: PathBuf,
    vapid: Vapid,
    phones: Mutex<Vec<Phone>>,
    hosts: Hosts,
    agent: ureq::Agent,
    /// Whether the watcher thread runs. Set and cleared only under `phones`' lock, so a phone added as the last one
    /// goes is never left without a watcher.
    running: AtomicBool,
    stop: AtomicBool,
    feed: Mutex<Option<Arc<Feed>>>,
    /// Each phone's 401/403 answers in a row (`REJECTED_MOST`); in memory only, so a restart starts each at none.
    strikes: Mutex<HashMap<String, u8>>,
    /// Whether a phone that turns alerts on gets `welcome`. Off in tests that count pushes.
    pub welcome: bool,
    pub debounce: Duration,
}

impl Alerts {
    /// The alerts kept in `dir` (the session's data dir), its key made the first time.
    pub fn open(dir: &Path, hosts: Hosts, timeout: Duration) -> Result<Self, String> {
        Self::open_with(dir, hosts, timeout, Path::try_exists)
    }

    /// `open`, with how the key file is looked for handed in (a test makes it fail).
    pub fn open_with(
        dir: &Path,
        hosts: Hosts,
        timeout: Duration,
        exists: Exists,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(dir)
            .map_err(|err| format!("the data dir could not be made: {}", err.kind()))?;
        let (vapid, made) = load_key(dir, exists)?;
        let mut phones = load_phones(&dir.join(PHONES_FILE));
        if made && !phones.is_empty() {
            // Each was subscribed with the key that is gone: the push service would refuse every alert to them, and
            // they would hold places under `MOST`. The page says to turn alerts on again (alerts.js).
            warn!(
                phones = phones.len(),
                "nirvana remote made a new alerts key; the phones stored were subscribed with the old one and are dropped"
            );
            phones.clear();
            write_private(&dir.join(PHONES_FILE), br#"{"phones":[]}"#)
                .map_err(|err| format!("the phones file could not be written: {}", err.kind()))?;
        }
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            // A push service that redirects is not followed: the host allow-list is for the URL that is POSTed to.
            .redirects(0)
            .build();
        Ok(Self {
            dir: dir.to_path_buf(),
            vapid,
            phones: Mutex::new(phones),
            hosts,
            agent,
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            feed: Mutex::new(None),
            strikes: Mutex::default(),
            welcome: true,
            debounce: DEBOUNCE,
        })
    }

    /// The alerts of the PC's own session, as `start` makes them.
    pub fn live() -> Result<Arc<Self>, String> {
        Self::open(
            &crate::session::data_dir(),
            Hosts::PushServices,
            PUSH_TIMEOUT,
        )
        .map(Arc::new)
    }

    pub fn public_key(&self) -> String {
        self.vapid.public_b64()
    }

    pub fn count(&self) -> usize {
        self.phones.lock().map_or(0, |phones| phones.len())
    }

    fn save(&self, phones: &[Phone]) -> Result<(), String> {
        let text = serde_json::to_string(&PhonesFile {
            phones: phones.to_vec(),
        })
        .map_err(|err| err.to_string())?;
        write_private(&self.dir.join(PHONES_FILE), text.as_bytes())
            .map_err(|err| format!("the phones file could not be written: {}", err.kind()))
    }

    /// Listen on `feed` from now on, watching at once when a phone already has alerts on.
    pub fn attach(self: &Arc<Self>, feed: Arc<Feed>) {
        if let Ok(mut slot) = self.feed.lock() {
            *slot = Some(feed);
        }
        if let Ok(phones) = self.phones.lock() {
            if !phones.is_empty() {
                self.start_watching(&phones);
            }
        }
    }

    /// Ends the watcher within `WAKE`, or, when an alert is being sent, once its pushes in flight are answered or time
    /// out (`PUSH_TIMEOUT`, since they run at once); no phone and no alert is started after it. Called when Nirvana
    /// Remote stops, which does not wait for the watcher.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Start the watcher thread unless it runs. The caller holds `phones`' lock.
    fn start_watching(self: &Arc<Self>, _held: &[Phone]) {
        let Some(feed) = self.feed.lock().ok().and_then(|feed| feed.clone()) else {
            return;
        };
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let alerts = self.clone();
        if std::thread::Builder::new()
            .name("nirvana-remote-alerts".into())
            .spawn(move || alerts.watch(feed))
            .is_err()
        {
            self.running.store(false, Ordering::SeqCst);
        }
    }

    /// Whether the watcher goes on; when no phone is left it marks itself stopped under the lock `subscribe` takes.
    fn go_on(&self) -> bool {
        if self.stop.load(Ordering::SeqCst) {
            self.running.store(false, Ordering::SeqCst);
            return false;
        }
        let Ok(phones) = self.phones.lock() else {
            self.running.store(false, Ordering::SeqCst);
            return false;
        };
        if phones.is_empty() {
            self.running.store(false, Ordering::SeqCst);
            return false;
        }
        true
    }

    /// The watcher: every list the feed sends, compared with the last; each change that passes `Debounce` is pushed.
    fn watch(self: Arc<Self>, feed: Arc<Feed>) {
        let _cleared = Unwinding(&self.running);
        let mut before: Option<Value> = None;
        let mut debounce = Debounce::new(self.debounce);
        'listen: while self.go_on() {
            let lists = match feed.listen() {
                Ok(lists) => lists,
                Err(error) => {
                    warn!(%error, "nirvana remote alerts could not listen to the spaces");
                    let started = Instant::now();
                    while started.elapsed() < RETRY {
                        if !self.go_on() {
                            break 'listen;
                        }
                        std::thread::sleep(WAKE.min(RETRY));
                    }
                    continue;
                }
            };
            loop {
                if !self.go_on() {
                    break 'listen;
                }
                let text = match lists.recv_timeout(WAKE) {
                    Ok(text) => text,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => continue 'listen,
                };
                let Ok(now) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(before) = before.as_ref() {
                    for alert in changes(before, &now) {
                        if debounce.allow(&alert, Instant::now()) {
                            self.push_all(&alert);
                        }
                    }
                }
                before = Some(now);
            }
        }
    }

    /// `alert` to every phone, all at once on one thread each (at most `MOST`), so a push service that hangs holds the
    /// alert up for `PUSH_TIMEOUT` at most, not that times each phone. Nothing starts once `stop` is set. The phones
    /// whose push service says they are gone, or refused their token `REJECTED_MOST` times in a row, are dropped.
    pub fn push_all(&self, alert: &Alert) {
        let phones = self.phones.lock().map(|p| p.clone()).unwrap_or_default();
        let body = payload(alert);
        let urgency = if alert.state == "needs" {
            "high"
        } else {
            "normal"
        };
        let outcomes: Vec<(&Phone, Outcome)> = std::thread::scope(|scope| {
            let mut sending = Vec::new();
            for phone in &phones {
                if self.stop.load(Ordering::SeqCst) {
                    break;
                }
                let body = &body;
                sending.push((phone, scope.spawn(move || self.push(phone, body, urgency))));
            }
            sending
                .into_iter()
                .map(|(phone, sent)| {
                    let outcome = sent
                        .join()
                        .unwrap_or_else(|_| Outcome::Failed("the send panicked".into()));
                    (phone, outcome)
                })
                .collect()
        });
        let mut gone = Vec::new();
        let (mut sent, mut failed) = (0, 0);
        if let Ok(mut strikes) = self.strikes.lock() {
            for (phone, outcome) in outcomes {
                match outcome {
                    Outcome::Sent => {
                        sent += 1;
                        strikes.remove(&phone.endpoint);
                    }
                    Outcome::Gone => gone.push(phone.endpoint.clone()),
                    Outcome::Rejected(code) => {
                        failed += 1;
                        let count = strikes.entry(phone.endpoint.clone()).or_insert(0);
                        *count += 1;
                        warn!(host = %host_of(&phone.endpoint), status = code, strikes = *count,
                            "nirvana remote alert not sent: the push service refused this PC's token");
                        if *count >= REJECTED_MOST {
                            strikes.remove(&phone.endpoint);
                            gone.push(phone.endpoint.clone());
                        }
                    }
                    Outcome::Failed(why) => {
                        failed += 1;
                        warn!(host = %host_of(&phone.endpoint), why, "nirvana remote alert not sent");
                    }
                }
            }
        }
        if !gone.is_empty() {
            if let Ok(mut phones) = self.phones.lock() {
                phones.retain(|phone| !gone.contains(&phone.endpoint));
                if let Err(why) = self.save(&phones) {
                    warn!(why, "nirvana remote could not drop a gone phone");
                }
            }
        }
        info!(
            agent = %alert.agent,
            state = alert.state,
            sent,
            gone = gone.len(),
            failed,
            "nirvana remote alert"
        );
    }

    /// One push to one phone: encrypted to its keys, signed for its push service's origin.
    pub fn push(&self, phone: &Phone, plaintext: &[u8], urgency: &str) -> Outcome {
        let Ok(url) = endpoint(&phone.endpoint, self.hosts) else {
            return Outcome::Failed("an endpoint that is not a push service's".into());
        };
        let (public, secret) = match keys(&phone.p256dh, &phone.auth) {
            Ok(keys) => keys,
            Err(why) => return Outcome::Failed(why.into()),
        };
        let body = match webpush::encrypt(&public, &secret, plaintext) {
            Ok(body) => body,
            Err(why) => return Outcome::Failed(why),
        };
        let audience = url.origin().ascii_serialization();
        let authorization = match self.vapid.authorization(&audience, unix_now()) {
            Ok(value) => value,
            Err(why) => return Outcome::Failed(why),
        };
        let answer = self
            .agent
            .post(url.as_str())
            .set("TTL", &TTL_SECS.to_string())
            .set("Urgency", urgency)
            .set("Content-Encoding", "aes128gcm")
            .set("Content-Type", "application/octet-stream")
            .set("Authorization", &authorization)
            .send_bytes(&body);
        match answer {
            Ok(response) if (200..300).contains(&response.status()) => Outcome::Sent,
            Ok(response) => Outcome::Failed(format!("status {}", response.status())),
            Err(ureq::Error::Status(404 | 410, _)) => Outcome::Gone,
            Err(ureq::Error::Status(code @ (401 | 403), _)) => Outcome::Rejected(code),
            Err(ureq::Error::Status(code, _)) => Outcome::Failed(format!("status {code}")),
            // The kind only: ureq's own text names the URL, and an endpoint's path is the phone's capability.
            Err(ureq::Error::Transport(transport)) => Outcome::Failed(transport.kind().to_string()),
        }
    }

    /// Add or update one phone; `Ok(true)` when it is new.
    fn add(self: &Arc<Self>, phone: Phone) -> Result<bool, (u16, String)> {
        let mut phones = self
            .phones
            .lock()
            .map_err(|_| (500, "the alerts are broken".to_string()))?;
        if phones.iter().any(|known| known.same(&phone)) {
            // Nothing changed: the file is not rewritten (the page tells the PC only when it differs, but an old page
            // may still tell it each time it opens).
            self.start_watching(&phones);
            return Ok(false);
        }
        let mut next = phones.clone();
        let count = next.len();
        let new = match next.iter_mut().find(|known| known.endpoint == phone.endpoint) {
            Some(known) => {
                known.p256dh = phone.p256dh;
                known.auth = phone.auth;
                false
            }
            None if count >= MOST => {
                return Err((
                    409,
                    format!(
                        "{MOST} phones already get alerts from this PC: turn alerts off on one of them first"
                    ),
                ))
            }
            None => {
                next.push(phone);
                true
            }
        };
        self.save(&next).map_err(|why| (500, why))?;
        *phones = next;
        self.start_watching(&phones);
        Ok(new)
    }

    /// Whether this PC keeps a phone at `phone`'s endpoint, and whether with the same keys.
    fn known(&self, phone: &Phone) -> (bool, bool) {
        self.phones.lock().map_or((false, false), |phones| {
            let at = phones.iter().find(|known| known.endpoint == phone.endpoint);
            (at.is_some(), at.is_some_and(|known| known.same(phone)))
        })
    }

    fn remove(&self, endpoint: &str) -> Result<bool, (u16, String)> {
        let mut phones = self
            .phones
            .lock()
            .map_err(|_| (500, "the alerts are broken".to_string()))?;
        let next: Vec<Phone> = phones
            .iter()
            .filter(|phone| phone.endpoint != endpoint)
            .cloned()
            .collect();
        if next.len() == phones.len() {
            return Ok(false);
        }
        self.save(&next).map_err(|why| (500, why))?;
        *phones = next;
        Ok(true)
    }
}

/// Clears `Alerts::running` if the watcher thread panics, so the next subscribe starts a new one instead of trusting a
/// watcher that is gone. Only on a panic: a watcher that ends normally clears it in `go_on`, under the phones lock,
/// and clearing it again here could clear a newer watcher's flag.
struct Unwinding<'a>(&'a AtomicBool);

impl Drop for Unwinding<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.store(false, Ordering::SeqCst);
        }
    }
}

/// An endpoint's host, the only part of it a log line names.
fn host_of(endpoint: &str) -> String {
    url::Url::parse(endpoint)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_default()
}

/// The home-screen app's files (plan 39 P39e): the service worker, the manifest, the alerts row, and the icon in the
/// sizes iOS and the manifest ask for, drawn from `nirvana/assets/nirvana.svg` full-bleed (iOS rounds the corners
/// itself, and fills a transparent corner with black).
pub const FILES: &[(&str, &str, &[u8])] = &[
    (
        "/sw.js",
        "text/javascript; charset=utf-8",
        include_bytes!("page/sw.js"),
    ),
    (
        "/alerts.js",
        "text/javascript; charset=utf-8",
        include_bytes!("page/alerts.js"),
    ),
    (
        "/manifest.webmanifest",
        "application/manifest+json",
        include_bytes!("page/manifest.webmanifest"),
    ),
    (
        "/apple-touch-icon.png",
        "image/png",
        include_bytes!("page/icon-180.png"),
    ),
    (
        "/icon-192.png",
        "image/png",
        include_bytes!("page/icon-192.png"),
    ),
    (
        "/icon-512.png",
        "image/png",
        include_bytes!("page/icon-512.png"),
    ),
];

fn error(status: u16, why: impl Into<String>) -> Response {
    Response::json(status, &json!({ "error": why.into() }))
}

#[derive(Deserialize)]
struct SubscriptionKeys {
    p256dh: String,
    auth: String,
}

/// `PushSubscription.toJSON()`, as the page posts it; `expirationTime` is ignored.
#[derive(Deserialize)]
struct Subscribe {
    endpoint: String,
    keys: SubscriptionKeys,
}

#[derive(Deserialize)]
struct Unsubscribe {
    endpoint: String,
}

/// A JSON body of at most `BODY_MOST`, or the answer that refuses it.
fn json_body<T: serde::de::DeserializeOwned>(
    request: &Request,
    conn: &mut Conn,
) -> Result<T, Response> {
    let json = request.header("Content-Type").first().is_some_and(|kind| {
        kind.trim()
            .to_ascii_lowercase()
            .starts_with("application/json")
    });
    if !json {
        return Err(error(415, "alerts take JSON"));
    }
    if conn.framing() == Framing::None {
        return Err(error(411, "a body has a length"));
    }
    let mut body = Vec::new();
    if let Err(why) = conn.read_body(BODY_MOST, &mut body) {
        return Err(error(why.status(), "the body could not be read"));
    }
    serde_json::from_slice(&body).map_err(|_| error(400, "not a push subscription"))
}

impl Site {
    /// P39e's routes and files. Called after every check in `Site::route`, like P39d's.
    pub fn alerts_routes(&self, request: &Request, conn: &mut Conn) -> Option<Response> {
        let reads = request.method == "GET" || request.method == "HEAD";
        if let Some((path, kind, body)) = FILES.iter().find(|(path, _, _)| *path == request.path) {
            return Some(if reads {
                // P39o: the worker is served with the build tag and the shell's paths written in (shell.rs).
                Response::new(
                    200,
                    kind,
                    if *path == "/sw.js" {
                        super::shell::worker().as_bytes()
                    } else {
                        *body
                    },
                )
            } else {
                Response::text(405, "GET only").with("Allow", "GET, HEAD")
            });
        }
        let answer = match (request.path.as_str(), request.method.as_str()) {
            ("/api/alerts", "GET" | "HEAD") => self.alerts_key(),
            ("/api/alerts/subscribe", "POST") => self.alerts_subscribe(request, conn),
            ("/api/alerts/check", "POST") => self.alerts_check(request, conn),
            ("/api/alerts/unsubscribe", "POST") => self.alerts_unsubscribe(request, conn),
            ("/api/alerts", _) => Response::text(405, "GET only").with("Allow", "GET, HEAD"),
            ("/api/alerts/subscribe" | "/api/alerts/check" | "/api/alerts/unsubscribe", _) => {
                Response::text(405, "POST only").with("Allow", "POST")
            }
            _ => return None,
        };
        Some(answer)
    }

    fn alerts_or_why(&self) -> Result<&Arc<Alerts>, Response> {
        self.alerts.as_ref().ok_or_else(|| {
            error(
                409,
                "alerts are not set up on this PC (herdr-server.log says why)",
            )
        })
    }

    /// `GET /api/alerts`: the public key a phone subscribes with, and how many phones have alerts on.
    fn alerts_key(&self) -> Response {
        match self.alerts_or_why() {
            Ok(alerts) => Response::json(
                200,
                &json!({ "key": alerts.public_key(), "phones": alerts.count(), "most": MOST }),
            ),
            Err(answer) => answer,
        }
    }

    fn alerts_subscribe(&self, request: &Request, conn: &mut Conn) -> Response {
        let alerts = match self.alerts_or_why() {
            Ok(alerts) => alerts.clone(),
            Err(answer) => return answer,
        };
        let body: Subscribe = match json_body(request, conn) {
            Ok(body) => body,
            Err(answer) => return answer,
        };
        let url = match endpoint(&body.endpoint, alerts.hosts) {
            Ok(url) => url,
            Err(why) => return error(400, why),
        };
        if let Err(why) = keys(&body.keys.p256dh, &body.keys.auth) {
            return error(400, why);
        }
        let phone = Phone {
            endpoint: url.to_string(),
            p256dh: body.keys.p256dh.trim().trim_end_matches('=').to_string(),
            auth: body.keys.auth.trim().trim_end_matches('=').to_string(),
            added: unix_now(),
        };
        let new = match alerts.add(phone.clone()) {
            Ok(new) => new,
            Err((status, why)) => return error(status, why),
        };
        info!(host = %host_of(&phone.endpoint), new, "nirvana remote alerts on for a phone");
        if new && alerts.welcome {
            // Off the route's thread: the answer does not wait on Apple.
            let sender = alerts.clone();
            let _ = std::thread::Builder::new()
                .name("nirvana-remote-welcome".into())
                .spawn(move || {
                    let why = match sender.push(&phone, &welcome(), "normal") {
                        Outcome::Sent | Outcome::Gone => return,
                        Outcome::Rejected(code) => format!("status {code}, the token refused"),
                        Outcome::Failed(why) => why,
                    };
                    warn!(host = %host_of(&phone.endpoint), why, "nirvana remote welcome alert not sent");
                });
        }
        Response::json(
            200,
            &json!({ "on": true, "new": new, "phones": alerts.count() }),
        )
    }

    /// `POST /api/alerts/check`: whether this PC keeps the posted subscription (`known`) and with the same keys
    /// (`same`), so the page re-posts only one that differs and says so when the PC no longer knows it. A POST, not a
    /// GET with a query: the endpoint is the phone's capability and stays out of URLs. Nothing is written.
    fn alerts_check(&self, request: &Request, conn: &mut Conn) -> Response {
        let alerts = match self.alerts_or_why() {
            Ok(alerts) => alerts.clone(),
            Err(answer) => return answer,
        };
        let body: Subscribe = match json_body(request, conn) {
            Ok(body) => body,
            Err(answer) => return answer,
        };
        let phone = Phone {
            endpoint: url::Url::parse(&body.endpoint)
                .map_or(body.endpoint.clone(), |url| url.to_string()),
            p256dh: body.keys.p256dh.trim().trim_end_matches('=').to_string(),
            auth: body.keys.auth.trim().trim_end_matches('=').to_string(),
            added: 0,
        };
        let (known, same) = alerts.known(&phone);
        Response::json(200, &json!({ "known": known, "same": same }))
    }

    fn alerts_unsubscribe(&self, request: &Request, conn: &mut Conn) -> Response {
        let alerts = match self.alerts_or_why() {
            Ok(alerts) => alerts.clone(),
            Err(answer) => return answer,
        };
        let body: Unsubscribe = match json_body(request, conn) {
            Ok(body) => body,
            Err(answer) => return answer,
        };
        // Stored endpoints are as parsed; one the page sends is compared the same way.
        let key =
            url::Url::parse(&body.endpoint).map_or(body.endpoint.clone(), |url| url.to_string());
        match alerts.remove(&key) {
            Ok(removed) => Response::json(
                200,
                &json!({ "removed": removed, "phones": alerts.count() }),
            ),
            Err((status, why)) => error(status, why),
        }
    }
}
