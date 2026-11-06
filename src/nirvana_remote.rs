//! Nirvana fork, plan 39 P39a: Nirvana Remote, the phone's way into this PC's live session.
//!
//! The owner, 2026-10-21: "how do I expose Nirvana to other devices to controll. Meaning have different computers
//! login to the active pc session working right now." and, for the login, "Tailscale account"
//! (nirvana/plans/2026-11-04-39-nirvana-mobile.md, built from 2026-10-22-remote-access-design.md).
//!
//! One per PC, inside the Nirvana session's herdr server (or the default one), started and stopped with it. It
//! listens on 127.0.0.1 only; `tailscale serve` puts it at `https://<pc>.<tailnet>.ts.net:8443/` for the owner's
//! devices and names each caller in a header (`routes.rs`), trusted only from the Tailscale service's own connection
//! (`peer.rs`). Tailscale off or signed out, HTTPS off for the tailnet, the port taken, or anything not its own on
//! 8443: it does not serve, and the sidebar's servers block says why from the state file this writes (`foot_row`). No
//! fallback to the home network.
//! The mount is a foreground serve held by a child `tailscale serve` (tailscale.rs): it ends with this server, a
//! hard kill included, and is checked again every `CHECK_EVERY`.

pub mod chat;
pub mod herdr;
pub mod http;
pub mod live;
pub mod pcs;
pub mod peer;
/// P39c: the prompt sheet and Esc.
pub mod prompt;
pub mod routes;
pub mod send;
pub mod spaces;
pub mod stream;
pub mod tailscale;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_live;
#[cfg(test)]
mod tests_prompt;
// Plan 39 P39d: the new-space picker and the cards.
pub mod cards;
pub mod pick;
#[cfg(test)]
mod tests_cards;
// Plan 39 P39e: alerts, Web Push to the owner's phones, and the home-screen app's files.
pub mod alerts;
#[cfg(test)]
mod tests_alerts;
pub mod webpush;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use tailscale::{Held, Mount, Off, Tailscale};

/// Where Nirvana Remote listens, 127.0.0.1 only. Fixed, so the one serve config stays right across restarts, and so
/// a second herdr server on this PC finds it taken instead of serving twice.
pub const LOCAL_PORT: u16 = 7790;
/// The tailnet's HTTPS port for it. Not 443, which the owner may serve something else on; 8443 is one Funnel
/// also accepts, should a later plan want it.
pub const HTTPS_PORT: u16 = 8443;
/// How often Tailscale is read again: a sign-in, HTTPS turned on, or Tailscale stopping shows within this.
pub const CHECK_EVERY: Duration = Duration::from_secs(60);
/// The state file is rewritten each check; older than this, the window reads Nirvana Remote as gone (a pid file is
/// not liveness: memory `pid-file-is-not-liveness`).
pub const STALE_AFTER: Duration = Duration::from_secs(150);
const STATE_FILE: &str = "nirvana-remote.json";
/// The window reads the state file at most this often.
#[cfg(not(test))]
const FOOT_EVERY: Duration = Duration::from_secs(3);

/// Why this server does not host Nirvana Remote, or `None` when it does. Only the default server or the Nirvana
/// session's server of the owner's own config does: the integration tests start real `herdr server` processes with
/// `HERDR_SOCKET_PATH` and `XDG_CONFIG_HOME` set (36 of them, tests/), and none of those may run a real `tailscale`
/// (plan 39 §4). Any other named session skips, so a second session on this PC never contends for the port. A debug
/// build (`herdr-dev`, which has its own config dir and sets neither) hosts it only when `NIRVANA_REMOTE=on` asks:
/// a dev server started first would otherwise take the port and the mount from the installed one (review round, B1).
pub fn skip_reason(
    var: impl Fn(&str) -> Option<String>,
    session: Option<String>,
    debug_build: bool,
) -> Option<&'static str> {
    let asked = var("NIRVANA_REMOTE");
    if asked
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("off"))
    {
        return Some("NIRVANA_REMOTE=off");
    }
    if var(crate::api::SOCKET_PATH_ENV_VAR).is_some() {
        return Some("a server on its own socket (HERDR_SOCKET_PATH)");
    }
    if var("XDG_CONFIG_HOME").is_some() {
        return Some("a server on its own config (XDG_CONFIG_HOME)");
    }
    // The Nirvana session hosts it like the default one. Every Nirvana window runs `--session nirvana`
    // (`nirvana_launch::SESSION`), so skipping every named session skipped the only server it exists for: on
    // 2026-11-04, after installing 0.55.0, the owner's server logged `nirvana remote not started why="a named
    // session's server"` and `tailscale serve status --json` read `{}` (record p39a, "Session fix (0.55.1)").
    if session.is_some_and(|name| name != crate::nirvana_launch::SESSION) {
        return Some("a named session's server");
    }
    if debug_build && !asked.is_some_and(|value| value.eq_ignore_ascii_case("on")) {
        return Some("a debug build (NIRVANA_REMOTE=on runs it)");
    }
    None
}

/// What the state file holds, for the sidebar row and for anyone reading why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub up: bool,
    pub short: String,
    pub long: String,
    pub url: Option<String>,
    /// Unix seconds of the check that wrote it.
    pub at: u64,
    /// The herdr server that wrote it: only the one holding the port overwrites another's fresh report.
    #[serde(default)]
    pub pid: u32,
}

impl Report {
    fn up(url: String, at: u64) -> Self {
        Self {
            up: true,
            short: format!(":{HTTPS_PORT}"),
            // Not "to the owner's devices": a process on this PC may reach the mount too (record p39a, review
            // round S1), so the line names who the check admits, not where they sit.
            long: format!("serving {url} to this PC's Tailscale login only"),
            url: Some(url),
            at,
            pid: std::process::id(),
        }
    }

    fn off(off: &Off, at: u64) -> Self {
        Self {
            up: false,
            short: off.short(),
            long: off.long(),
            url: None,
            at,
            pid: std::process::id(),
        }
    }
}

pub fn state_path() -> PathBuf {
    crate::session::data_dir().join(STATE_FILE)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn read_state(path: &Path) -> Option<Report> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// One owner of the state file (review round, S7): the server holding the port always writes it; another writes
/// only into a file that is missing, stale or its own, so a second server's "port busy" never covers the first's
/// "up" and the row does not flicker.
fn may_write(path: &Path, holding: bool, now: u64) -> bool {
    holding
        || read_state(path).is_none_or(|report| {
            report.pid == std::process::id()
                || now.saturating_sub(report.at) > STALE_AFTER.as_secs()
        })
}

fn write_report(path: &Path, report: &Report) {
    let Ok(text) = serde_json::to_string(report) else {
        return;
    };
    // Written beside and renamed, so the window never reads half a file.
    let partial = path.with_extension("json.partial");
    if std::fs::write(&partial, text).is_ok() {
        let _ = std::fs::rename(&partial, path);
    }
}

/// The settings `supervise` runs with; a test gives port 0, a temp state file and its own interval.
pub struct Config {
    pub port: u16,
    pub https_port: u16,
    pub state: PathBuf,
    pub every: Duration,
    /// How long a new mount has to show in `serve status` before the serve counts as failed.
    pub confirm: Duration,
}

/// `status --json` read, or why not. A missing CLI is its own reason.
pub fn assess(answer: Result<String, String>) -> Result<tailscale::Status, Off> {
    let text = answer.map_err(|error| {
        if error == "not installed" {
            Off::NotInstalled
        } else {
            Off::Unreadable(error)
        }
    })?;
    let status = tailscale::parse_status(&text)?;
    tailscale::ready(&status)?;
    if !cfg!(windows) {
        return Err(Off::NotWindows);
    }
    Ok(status)
}

/// Which mount is Nirvana Remote's: this PC's name, the HTTPS port, the local port.
#[derive(Clone)]
struct Key {
    dns: String,
    https: u16,
    local: u16,
}

impl Key {
    fn read(&self, ts: &dyn Tailscale) -> Result<Mount, String> {
        tailscale::mount(&ts.serve_status()?, &self.dns, self.https, self.local)
    }
}

/// After the hold ends, how long the listener waits for tailscaled to delete the mount.
const GONE_WAIT: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(100);

struct Serving {
    task: tokio::task::JoinHandle<()>,
    stop: tokio::sync::watch::Sender<bool>,
    /// The page's live views (P39b): ended with the serve, since each holds a connection to this listener.
    streams: Arc<stream::Streams>,
    held: Option<Box<dyn Held>>,
    key: Key,
    url: String,
}

impl Serving {
    /// The mount first, then the listener: tailscaled deletes a foreground mount when its `tailscale serve`
    /// ends, and the port stays held until it has, so the mount never points at a port nobody holds (S2).
    async fn end(mut self, ts: Arc<dyn Tailscale>) {
        let held = self.held.take();
        let key = self.key.clone();
        let _ = tokio::task::spawn_blocking(move || {
            drop(held);
            let started = std::time::Instant::now();
            while started.elapsed() < GONE_WAIT
                && key.read(ts.as_ref()) == Ok(Mount::OursForeground)
            {
                std::thread::sleep(POLL);
            }
        })
        .await;
        self.streams.close_all();
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }

    /// Whether the mount is still there and still ours (review round, S3): the CLI holding it alive, and
    /// `serve status` showing it.
    async fn check(&mut self, ts: Arc<dyn Tailscale>) -> Result<(), String> {
        if let Some(why) = self.held.as_mut().and_then(|held| held.ended()) {
            return Err(why);
        }
        let key = self.key.clone();
        match tokio::task::spawn_blocking(move || key.read(ts.as_ref())).await {
            Ok(Ok(Mount::OursForeground)) => Ok(()),
            Ok(Ok(Mount::Free)) => Err("serve status no longer has it".into()),
            Ok(Ok(Mount::Other(why))) => Err(why),
            Ok(Ok(Mount::OursBackground)) => Err("it was replaced by a background mount".into()),
            Ok(Err(error)) => Err(format!("serve status could not be read: {error}")),
            Err(_) => Err("the serve status call panicked".into()),
        }
    }
}

/// Look at what serves on the HTTPS port, clear a stale mount of ours, bind the listener, and hold the mount; the
/// URL the phone opens, or why not. Nothing that is not Nirvana Remote's own mount is ever changed (B2).
async fn bring_up(
    site: &Arc<routes::Site>,
    config: &Config,
    status: &tailscale::Status,
) -> Result<Serving, Off> {
    let ts = site.tailscale.clone();
    let before = Key {
        dns: status.me.dns.clone(),
        https: config.https_port,
        local: config.port,
    };
    let look = {
        let (ts, key) = (ts.clone(), before.clone());
        move || key.read(ts.as_ref())
    };
    let found = tokio::task::spawn_blocking(look.clone())
        .await
        .unwrap_or_else(|_| Err("the serve status call panicked".into()))
        .map_err(Off::Unreadable)?;
    match found {
        Mount::Free => {}
        Mount::Other(why) => return Err(Off::ServeTaken(why)),
        Mount::OursForeground => {
            return Err(Off::ServeTaken(format!(
                "{} is held by another running Nirvana Remote",
                config.https_port
            )))
        }
        Mount::OursBackground => {
            // Left by a build that used `--bg`, or typed by hand: it points a trusted ts.net origin at port 7790
            // whether or not anything of ours holds it (S2), so it goes before the port is bound.
            let (clear, https) = (ts.clone(), config.https_port);
            tokio::task::spawn_blocking(move || clear.clear_background(https))
                .await
                .unwrap_or_else(|_| Err("the serve call panicked".into()))
                .map_err(Off::ServeFailed)?;
            warn!(
                https,
                "nirvana remote removed a stale background mount of its own"
            );
            let after = tokio::task::spawn_blocking(look)
                .await
                .unwrap_or_else(|_| Err("the serve status call panicked".into()))
                .map_err(Off::Unreadable)?;
            if after != Mount::Free {
                return Err(Off::ServeTaken(format!(
                    "the stale mount on {} did not clear",
                    config.https_port
                )));
            }
        }
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", config.port))
        .map_err(|_| Off::PortInUse(config.port))?;
    let local = listener
        .local_addr()
        .map_err(|err| Off::Unreadable(err.to_string()))?
        .port();
    listener
        .set_nonblocking(true)
        .map_err(|err| Off::Unreadable(err.to_string()))?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .map_err(|err| Off::Unreadable(err.to_string()))?;
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let handler: Arc<dyn http::Handler> = site.clone();
    let task = tokio::spawn(http::serve(
        listener,
        handler,
        http::Limits::DEFAULT,
        stopped,
    ));
    let key = Key { local, ..before };
    let url = format!("https://{}:{}/", key.dns, key.https);
    let mut serving = Serving {
        task,
        stop,
        streams: site.streams.clone(),
        held: None,
        key: key.clone(),
        url,
    };
    let (hold, https, wait) = (ts.clone(), config.https_port, config.confirm);
    let held = tokio::task::spawn_blocking(move || {
        let mut held = hold.hold(https, local)?;
        // The mount counts once tailscaled shows it: the CLI ends with tailscaled's own error when the port is
        // taken after the look above ("listener already exists for port"), and never overwrites it.
        let started = std::time::Instant::now();
        loop {
            if let Some(why) = held.ended() {
                return Err(why);
            }
            if key.read(hold.as_ref()) == Ok(Mount::OursForeground) {
                return Ok(held);
            }
            if started.elapsed() >= wait {
                drop(held);
                return Err("the mount did not show in serve status".into());
            }
            std::thread::sleep(POLL);
        }
    })
    .await
    .unwrap_or_else(|_| Err("the serve call panicked".into()));
    match held {
        Ok(held) => {
            serving.held = Some(held);
            Ok(serving)
        }
        Err(error) => {
            serving.end(ts).await;
            Err(Off::ServeFailed(error))
        }
    }
}

/// Read Tailscale, serve when it lets us, write the state file, and again each `every`, until `stop`.
pub async fn supervise(
    site: Arc<routes::Site>,
    config: Config,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let mut serving: Option<Serving> = None;
    loop {
        let ts = site.tailscale.clone();
        let answer = tokio::task::spawn_blocking(move || ts.status())
            .await
            .unwrap_or_else(|_| Err("the status call panicked".into()));
        let report = match assess(answer) {
            Ok(status) => {
                if let Ok(mut owner) = site.owner.write() {
                    *owner = Some(status.owner_login.clone());
                }
                if let Ok(mut me) = site.me.write() {
                    *me = Some(routes::Me {
                        host: format!("{}:{}", status.me.dns, config.https_port),
                        ips: status.me.ips.clone(),
                    });
                }
                let checked = match serving.as_mut() {
                    Some(up) => Some(up.check(site.tailscale.clone()).await),
                    None => None,
                };
                match checked {
                    Some(Ok(())) => Report::up(
                        serving
                            .as_ref()
                            .map_or_else(String::new, |up| up.url.clone()),
                        unix_now(),
                    ),
                    Some(Err(why)) => {
                        if let Some(gone) = serving.take() {
                            gone.end(site.tailscale.clone()).await;
                        }
                        Report::off(&Off::MountGone(why), unix_now())
                    }
                    None => match bring_up(&site, &config, &status).await {
                        Ok(up) => {
                            info!(url = %up.url, "nirvana remote serving");
                            let report = Report::up(up.url.clone(), unix_now());
                            serving = Some(up);
                            report
                        }
                        Err(off) => Report::off(&off, unix_now()),
                    },
                }
            }
            Err(off) => {
                // Nobody is let in while Tailscale cannot say who the owner is.
                if let Ok(mut owner) = site.owner.write() {
                    *owner = None;
                }
                if let Ok(mut me) = site.me.write() {
                    *me = None;
                }
                Report::off(&off, unix_now())
            }
        };
        if !report.up {
            warn!(why = %report.long, "nirvana remote not serving");
        }
        if may_write(&config.state, serving.is_some(), report.at) {
            write_report(&config.state, &report);
        }
        tokio::select! {
            _ = tokio::time::sleep(config.every) => {}
            _ = stop.wait_for(|stopped| *stopped) => break,
        }
    }
    if let Some(up) = serving {
        up.end(site.tailscale.clone()).await;
    }
    // Only this server's own report goes with it.
    if read_state(&config.state).is_some_and(|report| report.pid == std::process::id()) {
        let _ = std::fs::remove_file(&config.state);
    }
}

/// The running Nirvana Remote, stopped when the herdr server stops.
pub struct Remote {
    stop: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
    /// P39e: told to stop with the rest; its thread ends within a second, or once a push in flight is answered.
    alerts: Option<Arc<alerts::Alerts>>,
}

impl Remote {
    pub async fn stop(self) {
        if let Some(alerts) = &self.alerts {
            alerts.stop();
        }
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

/// Start Nirvana Remote on the herdr server's runtime, when this server is the one to host it.
pub fn start(api_tx: crate::api::ApiRequestSender) -> Option<Remote> {
    if let Some(why) = skip_reason(
        |name| std::env::var(name).ok(),
        crate::session::active_name(),
        cfg!(debug_assertions),
    ) {
        info!(why, "nirvana remote not started");
        return None;
    }
    let mut site = routes::Site::new(
        Arc::new(tailscale::Cli),
        Arc::new(herdr::App {
            api_tx,
            next: std::sync::atomic::AtomicU64::new(1),
        }),
        Arc::new(peer::System::new()),
        Arc::new(pcs::Https),
        HTTPS_PORT,
        time::OffsetDateTime::now_utc,
    );
    // P39b: where the chat's transcripts may be read from, and the folder session records.
    site.claude_dir = crate::nirvana::claude_config_dir();
    site.nirvana_home = crate::nirvana::home();
    // P39d: the picker's root under the profile, the verb's preset, the servers block's ports.
    site.local = pick::Local::live();
    // P39e: the push key and the phones, in this session's data dir beside the state file. Alerts that cannot be set
    // up leave the rest of Nirvana Remote serving; the routes then say so.
    site.alerts = match alerts::Alerts::live() {
        Ok(alerts) => Some(alerts),
        Err(why) => {
            warn!(why, "nirvana remote alerts are off");
            None
        }
    };
    let site = Arc::new(site);
    if let Some(alerts) = &site.alerts {
        alerts.attach(site.feed.clone());
    }
    let config = Config {
        port: LOCAL_PORT,
        https_port: HTTPS_PORT,
        state: state_path(),
        every: CHECK_EVERY,
        confirm: Duration::from_secs(10),
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let alerts = site.alerts.clone();
    let task = tokio::spawn(supervise(site, config, stopped));
    Some(Remote { stop, task, alerts })
}

/// The sidebar's row: up with its port, or off with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootRow {
    pub up: bool,
    pub text: String,
}

/// The row from a state file's text, `None` when there is none or it is stale.
pub fn foot_row_from(text: &str, now: u64) -> Option<FootRow> {
    let report: Report = serde_json::from_str(text).ok()?;
    if now.saturating_sub(report.at) > STALE_AFTER.as_secs() {
        return None;
    }
    Some(FootRow {
        up: report.up,
        text: report.short,
    })
}

#[cfg(test)]
thread_local! {
    static TEST_ROW: std::cell::RefCell<Option<FootRow>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_foot_row_for_test(row: Option<FootRow>) {
    TEST_ROW.with_borrow_mut(|slot| *slot = row);
}

/// The row as last read; the file is read at most every `FOOT_EVERY`. Under test, the row a test set.
pub fn foot_row() -> Option<FootRow> {
    #[cfg(test)]
    {
        TEST_ROW.with_borrow(Clone::clone)
    }
    #[cfg(not(test))]
    {
        use std::sync::{Mutex, OnceLock};
        use std::time::Instant;
        type Cached = Option<(Instant, Option<FootRow>)>;
        static CACHE: OnceLock<Mutex<Cached>> = OnceLock::new();
        let cache = CACHE.get_or_init(|| Mutex::new(None));
        let Ok(mut cache) = cache.lock() else {
            return None;
        };
        if let Some((read_at, row)) = cache.as_ref() {
            if read_at.elapsed() < FOOT_EVERY {
                return row.clone();
            }
        }
        let row = std::fs::read_to_string(state_path())
            .ok()
            .and_then(|text| foot_row_from(&text, unix_now()));
        *cache = Some((Instant::now(), row.clone()));
        row
    }
}
