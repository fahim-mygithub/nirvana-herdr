//! What Nirvana Remote reads from and asks of this PC's Tailscale.
//!
//! The real one runs the `tailscale` CLI; every test uses a fake (plan 39 §4: "No test starts a real Tailscale").
//! `status --json` and `serve status --json` are read-only. `serve` changes the node's config, so only `start` in
//! `nirvana_remote.rs` calls it, and only in a server that hosts Nirvana Remote (`skip_reason`), never under test.
//!
//! The mount is a foreground `serve` (no `--bg`), held by a `tailscale serve` child for as long as Nirvana Remote
//! serves (review round, B2 and S2). Read in Tailscale 1.102.2's source (`ipn/serve.go` `Foreground`,
//! `ipn/ipnlocal/serve.go`): a foreground config lives for the CLI's IPN bus session, is deleted when that session
//! ends (`defer b.DeleteForegroundSession(sessionID)`), is dropped on reload from disk when no session holds it (a
//! tailscaled restart or a reboot), and "cannot override existing listeners--neither existing foreground listeners
//! nor existing background listeners". So it never overwrites the owner's own mount, and it does not outlive the
//! child; the child is in a kill-on-close job, so it does not outlive herdr either.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A CLI call that hangs (a `serve` waiting at a prompt, a stuck tailscaled) is killed after this.
const CLI_TIMEOUT: Duration = Duration::from_secs(15);

pub trait Tailscale: Send + Sync {
    /// `tailscale status --json`, as text.
    fn status(&self) -> Result<String, String>;
    /// `tailscale serve status --json`, as text: the node's whole serve config, foreground sessions included.
    fn serve_status(&self) -> Result<String, String>;
    /// Hold a foreground serve of `https://<this pc>:<https>/` to `http://127.0.0.1:<local>`; it lasts until the
    /// returned `Held` is dropped or its process ends.
    fn hold(&self, https: u16, local: u16) -> Result<Box<dyn Held>, String>;
    /// Remove the background mount on `https` that `mount` read as a stale one of Nirvana Remote's.
    fn clear_background(&self, https: u16) -> Result<(), String>;
}

/// A held foreground serve. Dropping it ends the `tailscale serve` process, and tailscaled removes the mount.
pub trait Held: Send {
    /// `Some(why)` once the process holding the mount has ended.
    fn ended(&mut self) -> Option<String>;
}

/// What sits on the HTTPS port in the node's serve config, as Nirvana Remote reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mount {
    /// Nothing serves on that port.
    Free,
    /// Nirvana Remote's mount, held by a live `tailscale serve`: one "/" handler proxying to
    /// `http://127.0.0.1:<local>`, HTTPS, no Funnel, in a foreground session.
    OursForeground,
    /// The same mount in the background config. Nirvana Remote never makes one (it only holds foreground serves),
    /// so this is left by the P39a build or typed by hand, and it outlives whatever held port 7790: the stale mount
    /// the review's S2 names.
    OursBackground,
    /// Anything else on that port: the owner's own, or Funnel. Never touched.
    Other(String),
}

/// The serve config's mount on `https`, for this PC's `dns` name and Nirvana Remote's `local` port.
pub fn mount(text: &str, dns: &str, https: u16, local: u16) -> Result<Mount, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let mut found = Vec::new();
    look(&value, dns, https, local, false, &mut found);
    if let Some(sessions) = value["Foreground"].as_object() {
        for session in sessions.values() {
            look(session, dns, https, local, true, &mut found);
        }
    }
    if let Some(other) = found.iter().find_map(|found| match found {
        Mount::Other(why) => Some(why.clone()),
        _ => None,
    }) {
        return Ok(Mount::Other(other));
    }
    Ok(if found.contains(&Mount::OursForeground) {
        Mount::OursForeground
    } else if found.contains(&Mount::OursBackground) {
        Mount::OursBackground
    } else {
        Mount::Free
    })
}

/// One serve config (the background one or a foreground session's) on `https`.
fn look(
    config: &serde_json::Value,
    dns: &str,
    https: u16,
    local: u16,
    foreground: bool,
    found: &mut Vec<Mount>,
) {
    let port = https.to_string();
    let tcp = &config["TCP"][&port];
    let suffix = format!(":{https}");
    let webs = config["Web"]
        .as_object()
        .map(|webs| {
            webs.iter()
                .filter(|(host_port, _)| host_port.ends_with(&suffix))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if tcp.is_null() && webs.is_empty() {
        return;
    }
    if tcp["TCPForward"].as_str().is_some_and(|to| !to.is_empty()) {
        found.push(Mount::Other(format!("a TCP forward on {https}")));
        return;
    }
    let ours = format!("http://127.0.0.1:{local}");
    let mine = format!("{dns}:{https}");
    for (host_port, web) in &webs {
        if config["AllowFunnel"][host_port.as_str()].as_bool() == Some(true) {
            found.push(Mount::Other(format!("Funnel on {https}")));
            return;
        }
        let handlers = web["Handlers"].as_object();
        let only_root_to_us = host_port.as_str() == mine
            && handlers.is_some_and(|handlers| {
                handlers.len() == 1
                    && handlers.get("/").is_some_and(|root| {
                        root["Proxy"]
                            .as_str()
                            .is_some_and(|to| to.trim_end_matches('/') == ours)
                    })
            });
        if !only_root_to_us {
            found.push(Mount::Other(format!(
                "{https} already serves something else ({host_port})"
            )));
            return;
        }
    }
    found.push(if foreground {
        Mount::OursForeground
    } else {
        Mount::OursBackground
    });
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
    /// Its tailnet addresses (`TailscaleIPs`).
    pub ips: Vec<std::net::IpAddr>,
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
    /// Something that is not Nirvana Remote's own mount sits on the HTTPS port; nothing was changed.
    ServeTaken(String),
    /// The mount Nirvana Remote held is gone (a `serve reset`, tailscaled restarting, the CLI ending).
    MountGone(String),
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
            Self::ServeTaken(_) => "8443 taken".into(),
            Self::MountGone(_) => "serve gone".into(),
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
            Self::ServeTaken(why) => {
                format!("not serving, and nothing changed: {why}")
            }
            Self::MountGone(why) => {
                format!("the serve mount went away ({why}); it is set again at the next check")
            }
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
        ips: value["TailscaleIPs"]
            .as_array()
            .map(|ips| {
                ips.iter()
                    .filter_map(|ip| ip.as_str()?.parse().ok())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The `tailscale` CLI.
pub struct Cli;

impl Tailscale for Cli {
    fn status(&self) -> Result<String, String> {
        run(&["status", "--json"])
    }

    fn serve_status(&self) -> Result<String, String> {
        run(&["serve", "status", "--json"])
    }

    fn hold(&self, https: u16, local: u16) -> Result<Box<dyn Held>, String> {
        let args = hold_args(https, local);
        CliHold::start(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .map(|held| Box::new(held) as Box<dyn Held>)
    }

    fn clear_background(&self, https: u16) -> Result<(), String> {
        // No `--yes`: the mount was read as one "/" handler, so the CLI has nothing to ask (it prompts only before
        // deleting more than one handler under a port).
        run(&["serve", &format!("--https={https}"), "off"]).map(|_| ())
    }
}

/// The held serve's arguments: no `--bg`, so the mount is a foreground session's and ends with the process, and no
/// `--yes`, which only skips the prompt before deleting handlers (B2: never a blind `--yes`).
pub fn hold_args(https: u16, local: u16) -> Vec<String> {
    vec![
        "serve".into(),
        format!("--https={https}"),
        format!("http://127.0.0.1:{local}"),
    ]
}

/// The CLI to run: the one Tailscale's installer puts under Program Files when it is there, else whatever PATH
/// finds (review nit N3). Program Files is writable by administrators only; a PATH entry may not be.
pub fn program(program_files: Option<String>, exists: impl Fn(&Path) -> bool) -> PathBuf {
    if let Some(root) = program_files.filter(|root| !root.is_empty()) {
        let installed = Path::new(&root).join("Tailscale").join("tailscale.exe");
        if exists(&installed) {
            return installed;
        }
    }
    PathBuf::from("tailscale")
}

fn command() -> std::process::Command {
    let program = program(std::env::var("ProgramFiles").ok(), Path::is_file);
    let mut command = crate::noninteractive_process::command(program);
    // `BROWSER` points at nothing so no call can open the owner's signed-in browser (CLAUDE.md rule 15).
    command.env("BROWSER", if cfg!(windows) { "rem" } else { "true" });
    command
}

/// A foreground `tailscale serve`, in a job that kills it when the last handle closes: when this is dropped, and
/// when herdr ends any other way, a hard kill included.
struct CliHold {
    child: std::process::Child,
    /// Its first stderr line, read on a thread so a full pipe never stalls it.
    error: std::sync::Arc<std::sync::Mutex<String>>,
    #[cfg(windows)]
    _job: job::Job,
}

impl CliHold {
    fn start(args: &[&str]) -> Result<Self, String> {
        use std::io::BufRead;
        use std::process::Stdio;
        let mut child = command()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| err.to_string())?;
        #[cfg(windows)]
        let job = match job::Job::kill_on_close(&child) {
            Ok(job) => job,
            Err(why) => {
                // A mount that could outlive herdr is the stale mount S2 forbids: no job, no serve.
                let _ = child.kill();
                let _ = child.wait();
                return Err(why);
            }
        };
        let error = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        if let Some(stderr) = child.stderr.take() {
            let error = error.clone();
            std::thread::spawn(move || {
                for line in std::io::BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                {
                    let line = line.trim().to_string();
                    if let Ok(mut first) = error.lock() {
                        if first.is_empty() && !line.is_empty() {
                            *first = line;
                        }
                    }
                }
            });
        }
        Ok(Self {
            child,
            error,
            #[cfg(windows)]
            _job: job,
        })
    }
}

impl Held for CliHold {
    fn ended(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(Some(status)) => {
                let line = self
                    .error
                    .lock()
                    .map(|line| line.clone())
                    .unwrap_or_default();
                Some(if line.is_empty() {
                    format!("tailscale serve ended ({status})")
                } else {
                    line
                })
            }
            Err(error) => Some(error.to_string()),
        }
    }
}

impl Drop for CliHold {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// An unnamed job, so no other process can open it; its one handle is this, closed by Windows when herdr
    /// ends. The same limits as `nirvana_job.rs`, with every failure reported instead of ignored.
    pub struct Job(HANDLE);

    // The handle is only closed, from whichever thread drops it.
    unsafe impl Send for Job {}

    impl Job {
        pub fn kill_on_close(child: &std::process::Child) -> Result<Self, String> {
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err("could not create a job for tailscale serve".into());
            }
            let job = Job(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let set = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if set == 0 {
                return Err("could not set the job's kill-on-close limit".into());
            }
            if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle().cast()) } == 0 {
                return Err("could not put tailscale serve in herdr's job".into());
            }
            Ok(job)
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

/// Run `tailscale` with no stdin and no console, killed at `CLI_TIMEOUT`. The child is polled on this thread
/// so the timeout can kill it (plan 16's review: a child waited on from another thread was never killed).
fn run(args: &[&str]) -> Result<String, String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut command = command();
    command
        .args(args)
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
