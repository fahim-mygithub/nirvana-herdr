//! Nirvana fork, plan 39 P39a: Nirvana Remote, the phone's way into this PC's live session.
//!
//! The owner, 2026-10-21: "how do I expose Nirvana to other devices to controll. Meaning have different computers
//! login to the active pc session working right now." and, for the login, "Tailscale account"
//! (nirvana/plans/2026-11-04-39-nirvana-mobile.md, built from 2026-10-22-remote-access-design.md).
//!
//! One per PC, inside the default herdr server, started and stopped with it. It listens on 127.0.0.1 only;
//! `tailscale serve` puts it at `https://<pc>.<tailnet>.ts.net:8443/` for the owner's devices and names each caller
//! in a header (`routes.rs`), trusted only from the Tailscale service's own connection (`peer.rs`). Tailscale off or
//! signed out, HTTPS off for the tailnet, or the port taken: it does not serve, and the sidebar's servers block says
//! why from the state file this writes (`foot_row`). No fallback to the home network.

pub mod herdr;
pub mod http;
pub mod pcs;
pub mod peer;
pub mod routes;
pub mod tailscale;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use tailscale::Off;

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

/// Why this server does not host Nirvana Remote, or `None` when it does. Only the default server of the owner's
/// own config does: the integration tests start real `herdr server` processes with `HERDR_SOCKET_PATH` and
/// `XDG_CONFIG_HOME` set (36 of them, tests/), and none of those may run a real `tailscale` (plan 39 §4).
pub fn skip_reason(
    var: impl Fn(&str) -> Option<String>,
    session: Option<String>,
) -> Option<&'static str> {
    if var("NIRVANA_REMOTE").is_some_and(|value| value.eq_ignore_ascii_case("off")) {
        return Some("NIRVANA_REMOTE=off");
    }
    if var(crate::api::SOCKET_PATH_ENV_VAR).is_some() {
        return Some("a server on its own socket (HERDR_SOCKET_PATH)");
    }
    if var("XDG_CONFIG_HOME").is_some() {
        return Some("a server on its own config (XDG_CONFIG_HOME)");
    }
    if session.is_some() {
        return Some("a named session's server");
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
}

impl Report {
    fn up(url: String, at: u64) -> Self {
        Self {
            up: true,
            short: format!(":{HTTPS_PORT}"),
            long: format!("serving {url} to this Tailscale account's devices"),
            url: Some(url),
            at,
        }
    }

    fn off(off: &Off, at: u64) -> Self {
        Self {
            up: false,
            short: off.short(),
            long: off.long(),
            url: None,
            at,
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

struct Serving {
    task: tokio::task::JoinHandle<()>,
    stop: tokio::sync::watch::Sender<bool>,
}

impl Serving {
    async fn end(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

/// Bind the listener and point `tailscale serve` at it; the URL the phone opens, or why not.
async fn bring_up(
    site: &Arc<routes::Site>,
    config: &Config,
    status: &tailscale::Status,
) -> Result<(Serving, String), Off> {
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
    let task = tokio::spawn(http::serve(listener, handler, stopped));
    let serving = Serving { task, stop };
    let ts = site.tailscale.clone();
    let https = config.https_port;
    let served = tokio::task::spawn_blocking(move || ts.serve_on(https, local))
        .await
        .unwrap_or_else(|_| Err("the serve call panicked".into()));
    if let Err(error) = served {
        serving.end().await;
        return Err(Off::ServeFailed(error));
    }
    Ok((serving, format!("https://{}:{}/", status.me.dns, https)))
}

/// Read Tailscale, serve when it lets us, write the state file, and again each `every`, until `stop`.
pub async fn supervise(
    site: Arc<routes::Site>,
    config: Config,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let mut serving: Option<(Serving, String)> = None;
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
                match &serving {
                    Some((_, url)) => Report::up(url.clone(), unix_now()),
                    None => match bring_up(&site, &config, &status).await {
                        Ok((up, url)) => {
                            info!(%url, "nirvana remote serving");
                            serving = Some((up, url.clone()));
                            Report::up(url, unix_now())
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
                Report::off(&off, unix_now())
            }
        };
        if !report.up {
            warn!(why = %report.long, "nirvana remote not serving");
        }
        write_report(&config.state, &report);
        tokio::select! {
            _ = tokio::time::sleep(config.every) => {}
            _ = stop.wait_for(|stopped| *stopped) => break,
        }
    }
    if let Some((up, _)) = serving {
        up.end().await;
        let ts = site.tailscale.clone();
        let https = config.https_port;
        let off = tokio::task::spawn_blocking(move || ts.serve_off(https)).await;
        if let Ok(Err(error)) = off {
            warn!(%error, "nirvana remote could not remove its tailscale serve");
        }
    }
    let _ = std::fs::remove_file(&config.state);
}

/// The running Nirvana Remote, stopped when the herdr server stops.
pub struct Remote {
    stop: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}

impl Remote {
    pub async fn stop(self) {
        let _ = self.stop.send(true);
        let _ = self.task.await;
    }
}

/// Start Nirvana Remote on the herdr server's runtime, when this server is the one to host it.
pub fn start(api_tx: crate::api::ApiRequestSender) -> Option<Remote> {
    if let Some(why) = skip_reason(
        |name| std::env::var(name).ok(),
        crate::session::active_name(),
    ) {
        info!(why, "nirvana remote not started");
        return None;
    }
    let site = Arc::new(routes::Site {
        tailscale: Arc::new(tailscale::Cli),
        herdr: Arc::new(herdr::App {
            api_tx,
            next: std::sync::atomic::AtomicU64::new(1),
        }),
        peer: Arc::new(peer::System),
        probe: Arc::new(pcs::Https),
        owner: RwLock::new(None),
        https_port: HTTPS_PORT,
        now: time::OffsetDateTime::now_utc,
    });
    let config = Config {
        port: LOCAL_PORT,
        https_port: HTTPS_PORT,
        state: state_path(),
        every: CHECK_EVERY,
    };
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(supervise(site, config, stopped));
    Some(Remote { stop, task })
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
