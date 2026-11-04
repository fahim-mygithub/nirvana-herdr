//! What Nirvana Remote reads from and asks of this PC's Tailscale.
//!
//! The real one runs the `tailscale` CLI; every test uses a fake (plan 39 §4: "No test starts a real Tailscale").
//! `status --json` is read-only. `serve` changes the node's config, so only `start` in `nirvana_remote.rs` calls it,
//! and only in a server that hosts Nirvana Remote (`hosts_remote`), never under test.

use std::time::{Duration, Instant};

/// A CLI call that hangs (a `serve` waiting at a prompt, a stuck tailscaled) is killed after this.
const CLI_TIMEOUT: Duration = Duration::from_secs(15);

pub trait Tailscale: Send + Sync {
    /// `tailscale status --json`, as text.
    fn status(&self) -> Result<String, String>;
    /// Point `https://<this pc>:<https>/` at `http://127.0.0.1:<local>`, kept by tailscaled in the background.
    fn serve_on(&self, https: u16, local: u16) -> Result<(), String>;
    /// Remove that one HTTPS port's serve, leaving any other the owner set.
    fn serve_off(&self, https: u16) -> Result<(), String>;
}

/// One machine on the tailnet, as `status --json` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub host: String,
    /// The MagicDNS name without its trailing dot, e.g. `quadstation-w11.tailb6b323.ts.net`.
    pub dns: String,
    pub os: String,
    pub online: bool,
    /// `None` for this PC and for a peer Tailscale has no time for (it sends year 1).
    pub last_seen: Option<time::OffsetDateTime>,
    pub user_id: u64,
    pub tagged: bool,
}

/// The parts of `status --json` Nirvana Remote uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub backend_state: String,
    /// Empty while HTTPS is off for the tailnet: then no `serve --https` can work (measured 2026-11-04: `null`).
    pub cert_domains: Vec<String>,
    pub me: Node,
    /// The login of the user this PC is signed in as: the only one let in.
    pub owner_login: String,
    pub peers: Vec<Node>,
}

/// Why Nirvana Remote is not serving, short for the sidebar row and long for the log and the state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Off {
    NotInstalled,
    Stopped,
    SignedOut,
    Backend(String),
    NoHttps,
    Unreadable(String),
    PortInUse(u16),
    ServeFailed(String),
    NotWindows,
}

impl Off {
    pub fn short(&self) -> String {
        match self {
            Self::NotInstalled => "no Tailscale".into(),
            Self::Stopped => "Tailscale off".into(),
            Self::SignedOut => "signed out".into(),
            Self::Backend(state) => state.clone(),
            Self::NoHttps => "HTTPS off".into(),
            Self::Unreadable(_) => "unreadable".into(),
            Self::PortInUse(port) => format!("port {port} busy"),
            Self::ServeFailed(_) => "serve failed".into(),
            Self::NotWindows => "Windows only".into(),
        }
    }

    pub fn long(&self) -> String {
        match self {
            Self::NotInstalled => "Tailscale is not installed on this PC".into(),
            Self::Stopped => "Tailscale is turned off on this PC".into(),
            Self::SignedOut => "Tailscale is signed out on this PC".into(),
            Self::Backend(state) => format!("Tailscale is not running (state {state})"),
            Self::NoHttps => {
                "HTTPS is off for the tailnet: turn it on in the Tailscale admin console (DNS, HTTPS Certificates)"
                    .into()
            }
            Self::Unreadable(why) => format!("tailscale status could not be read: {why}"),
            Self::PortInUse(port) => {
                format!("127.0.0.1:{port} is taken; another Nirvana Remote may hold it")
            }
            Self::ServeFailed(why) => format!("tailscale serve failed: {why}"),
            Self::NotWindows => {
                "Nirvana Remote checks the caller against the Tailscale service, built for Windows only".into()
            }
        }
    }
}

/// `status --json` read into what Nirvana Remote needs, or why it cannot serve.
pub fn parse_status(text: &str) -> Result<Status, Off> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| Off::Unreadable(err.to_string()))?;
    let backend_state = value["BackendState"].as_str().unwrap_or("").to_string();
    match backend_state.as_str() {
        "Running" => {}
        "Stopped" => return Err(Off::Stopped),
        "NeedsLogin" | "NeedsMachineAuth" => return Err(Off::SignedOut),
        other => return Err(Off::Backend(other.to_string())),
    }
    let me = node(&value["Self"]).ok_or_else(|| Off::Unreadable("no Self".into()))?;
    let owner_login = value["User"][me.user_id.to_string()]["LoginName"]
        .as_str()
        .filter(|login| !login.is_empty())
        .ok_or_else(|| Off::Unreadable("no login for this PC's user".into()))?
        .to_string();
    let cert_domains = value["CertDomains"]
        .as_array()
        .map(|domains| {
            domains
                .iter()
                .filter_map(|domain| domain.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut peers = value["Peer"]
        .as_object()
        .map(|peers| peers.values().filter_map(node).collect::<Vec<_>>())
        .unwrap_or_default();
    peers.sort_by_key(|peer| peer.host.to_lowercase());
    Ok(Status {
        backend_state,
        cert_domains,
        me,
        owner_login,
        peers,
    })
}

/// Whether the status lets Nirvana Remote serve: running, and HTTPS on for the tailnet.
pub fn ready(status: &Status) -> Result<(), Off> {
    if status.cert_domains.is_empty() {
        return Err(Off::NoHttps);
    }
    Ok(())
}

fn node(value: &serde_json::Value) -> Option<Node> {
    let host = value["HostName"].as_str()?.to_string();
    let last_seen = value["LastSeen"].as_str().and_then(|text| {
        time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
            .ok()
            // Tailscale sends 0001-01-01 for "no time".
            .filter(|seen| seen.year() > 2000)
    });
    Some(Node {
        host,
        dns: value["DNSName"]
            .as_str()
            .unwrap_or("")
            .trim_end_matches('.')
            .to_string(),
        os: value["OS"].as_str().unwrap_or("").to_string(),
        online: value["Online"].as_bool().unwrap_or(false),
        last_seen,
        user_id: value["UserID"].as_u64().unwrap_or(0),
        tagged: value["Tags"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty()),
    })
}

/// The `tailscale` CLI.
pub struct Cli;

impl Tailscale for Cli {
    fn status(&self) -> Result<String, String> {
        run(&["status", "--json"])
    }

    fn serve_on(&self, https: u16, local: u16) -> Result<(), String> {
        run(&[
            "serve",
            "--bg",
            "--yes",
            &format!("--https={https}"),
            &format!("http://127.0.0.1:{local}"),
        ])
        .map(|_| ())
    }

    fn serve_off(&self, https: u16) -> Result<(), String> {
        run(&["serve", "--yes", &format!("--https={https}"), "off"]).map(|_| ())
    }
}

/// Run `tailscale` with no stdin and no console, killed at `CLI_TIMEOUT`. The child is polled on this thread
/// so the timeout can kill it (plan 16's review: a child waited on from another thread was never killed).
/// `BROWSER` points at nothing so no call can open the owner's signed-in browser (CLAUDE.md rule 15).
fn run(args: &[&str]) -> Result<String, String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut command = crate::noninteractive_process::command("tailscale");
    command
        .args(args)
        .env("BROWSER", if cfg!(windows) { "rem" } else { "true" })
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            "not installed".to_string()
        } else {
            err.to_string()
        }
    })?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });
    let err = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= CLI_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "tailscale {} took over {} s",
                    args.first().unwrap_or(&""),
                    CLI_TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(error) => return Err(error.to_string()),
        }
    };
    let out = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    let err = String::from_utf8_lossy(&err.join().unwrap_or_default()).into_owned();
    if status.success() {
        Ok(out)
    } else {
        let line = err
            .lines()
            .chain(out.lines())
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("no output")
            .to_string();
        Err(line)
    }
}
