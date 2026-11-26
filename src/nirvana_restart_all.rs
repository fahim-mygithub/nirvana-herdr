//! Nirvana fork, plan 48: a restart of the whole Nirvana session, asked from a command, the window or the phone.
//!
//! The owner, 2026-11-25: "Is there a way to restart all of nirvana and add an option to do so. Can you restart it
//! first". Before this the only restart was the header's reload badge, shown only when a newer build was installed.
//!
//! A restart is what the badge's reload is: the server saves the session and stops, every pane's program stops with
//! it (Windows has no live handoff), a new server restores the session, and Nirvana Claude comes back in the panes
//! that ran it (`app/nirvana_relaunch.rs`). Three places ask for one:
//!
//! - **The window**: the header's version label is the badge when no newer build is installed. Two presses, as the
//!   badge's (`crate::nirvana::press_reload`); the second runs the badge's own path (`finish_reload`).
//! - **The command**: `herdr --session <name> nirvana restart` (`cli`).
//! - **The phone**: `POST /api/restart` (`nirvana_remote/live.rs`), which starts the same worker as the command.
//!
//! The command and the phone start a worker (`herdr --session <name> nirvana restart --worker`) out of the caller's
//! process tree: a short hop starts it as herdr starts its own server and leaves (`spawn_worker`), so a restart asked
//! from a pane of the session it restarts is not ended with that pane, whose whole tree herdr ends on the way down.
//! The worker sends `command.invoke` with `RESTART_COMMAND` (herdr's protocol stays upstream's, as
//! Restart Claude's does), and the server stops with `SHUTDOWN_REASON` in the `ServerShutdown` every client gets.
//!
//! **A window is kept.** A client that reads `SHUTDOWN_REASON` leaves as the badge's does: it waits until the old
//! server's sockets are gone, then exits with `RELOAD_EXIT_CODE`, and the window's `supervise_code` starts the client
//! again, which starts the new server and attaches (`finish_client_restart`). A second client gets the same message:
//! a supervised one comes back the same way (two starting servers race to bind, and herdr's `AddrInUse` leaves one),
//! an unsupervised one ends; neither is left on a dead server.
//!
//! **No window** (the server runs, the window was closed, the phone asks): the worker starts the server again with
//! no window (`comeback`). Why not open a window: a restart puts back what it found, and the owner who asks from the
//! phone is away from the PC; a window opened there would be a Windows Terminal nobody asked for. "Open Nirvana"
//! attaches a window to the running server at any time. With a window, the worker waits `WINDOW_WAIT` for the
//! window's client to bring the server back, and starts it itself only if none came.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// The `command.invoke` id the worker sends: stop the server as a restart.
pub const RESTART_COMMAND: &str = "nirvana.restart";
/// What `ServerShutdown { reason }` carries when the stop is a restart; upstream's own text otherwise.
pub const SHUTDOWN_REASON: &str = "nirvana restart";
pub const PLAIN_SHUTDOWN: &str = "server is shutting down";
/// The worker's progress and outcome, in the session's data dir, for the command that waits on it.
pub const RESULT_FILE: &str = "nirvana-restart.json";
/// The old server has this long to let its sockets go: `herdr session stop`'s own wait (session.rs
/// STOP_WAIT_TIMEOUT). Measured 2026-11-25: `herdr --session nirvana server stop` had all three processes gone
/// within 4 s.
pub const STOP_WAIT: Duration = Duration::from_secs(15);
/// With a window attached, its client has this long after the old server went to bring a new one up. A hypothesis,
/// not a measurement: the client waits on the sockets at `POLL`, starts again, and spawns the server, whose own
/// readiness wait is 15 s (autodetect SERVER_READY_TIMEOUT); past this the worker starts it, and a second start loses
/// the bind and leaves (`AddrInUse`).
pub const WINDOW_WAIT: Duration = Duration::from_secs(20);
/// A server the worker started has this long to listen.
pub const READY_WAIT: Duration = Duration::from_secs(30);
/// How often the worker and the client look at the sockets.
pub const POLL: Duration = Duration::from_millis(100);
/// How long the command waits on the worker's outcome: every wait above, and a margin for the process starts.
pub const ANSWER_WAIT: Duration = Duration::from_secs(15 + 20 + 30 + 10);

pub const NOT_STOPPED: &str = "the old server did not stop within 15 s; nothing was started";
pub const NOT_LISTENING: &str =
    "a new server was started but did not listen within 30 s (see herdr-server.log)";
/// Fix round E (review finding 6): what upstream's `command_not_found` means for the restart's id.
pub const OLD_SERVER: &str = "the running Nirvana is an older build that has no restart; reload it once with the \
     header's badge, or close the window and open Nirvana again";

pub const USAGE: &str = "usage: herdr --session <name> nirvana restart
  Restarts the session: the server saves it and stops, every program in its panes stops (a working agent's turn
  and a seated worker's order stop with their pane), a new server restores it, and Nirvana Claude comes back in
  the panes that ran it. With the Nirvana window open the window stays; with none the server comes back with no
  window. It answers once the new server listens, or says what did not come back, with exit 1.
  Run from a pane of the session it restarts, it ends with that pane; the restart goes on by itself and writes its
  outcome to nirvana-restart.json in the session's folder. With no --session it restarts the session this
  terminal's herdr socket names.
  A running Nirvana from an older build has no restart: reload it once with the header's badge, or close the
  window and open Nirvana again.";

// ---- The server's mark: which server answers ----

/// A process's mark: its pid and its start in unix milliseconds. Fix round A (review finding 1): the mark was the pid
/// alone, and Windows reuses pids, so a new server with the old one's pid left the phone on "Restarting Nirvana" for
/// the page's whole wait (app.js RESTART_MOST_MS, 180 s). With the start beside it, a new process has a new mark.
pub fn mark_of(pid: u32, started_unix_ms: u128) -> String {
    format!("{pid}-{started_unix_ms}")
}

static SERVER_MARK: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// This process's mark, taken once. `/api/hello` and the 202 of `POST /api/restart` both name it (`nirvana_remote`
/// routes.rs `with_server`, live.rs `restart`), and the page waits to see it change. The phone server takes it as it
/// starts (`nirvana_remote::start`), so its time is the server's start to within the server's own startup; a later
/// process with the same pid takes a later time.
pub fn server_mark() -> &'static str {
    SERVER_MARK.get_or_init(|| mark_of(std::process::id(), unix_ms_now()))
}

fn unix_ms_now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or(0)
}

// ---- The server: the stop is a restart ----

/// The reason the server's `ServerShutdown` carries: `SHUTDOWN_REASON` when the stop was asked as a restart.
pub fn shutdown_message(restarting: bool) -> String {
    if restarting {
        SHUTDOWN_REASON.to_owned()
    } else {
        PLAIN_SHUTDOWN.to_owned()
    }
}

// ---- The client: leave as the badge's reload does ----

static CLIENT_RESTART: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the server's shutdown reason is a restart's.
pub fn is_restart(reason: Option<&str>) -> bool {
    reason == Some(SHUTDOWN_REASON)
}

/// The client read a restart's shutdown: `main` finishes it (`finish_client_restart`).
pub fn client_saw_restart() {
    CLIENT_RESTART.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn client_restart_requested() -> bool {
    CLIENT_RESTART.load(std::sync::atomic::Ordering::Relaxed)
}

/// The client's exit code once it waited on the old server: `RELOAD_EXIT_CODE` (the window starts it again) only
/// when the old sockets are gone, so the new client never starts a server while the old one holds them.
pub fn client_exit(old_gone: bool) -> i32 {
    if old_gone {
        crate::nirvana::RELOAD_EXIT_CODE
    } else {
        1
    }
}

/// Waits up to `limit` for `gone`, looking each `step`.
pub fn wait_until(
    limit: Duration,
    step: Duration,
    mut gone: impl FnMut() -> bool,
    mut sleep: impl FnMut(Duration),
) -> bool {
    let mut waited = Duration::ZERO;
    loop {
        if gone() {
            return true;
        }
        if waited >= limit {
            return false;
        }
        sleep(step);
        waited += step;
    }
}

/// What the window shows while its client waits for the old server (fix round, review finding 9): before it, only
/// upstream's `herdr: server shut down: nirvana restart`, and then up to `STOP_WAIT` of nothing.
pub const RESTARTING_LINE: &str =
    "nirvana: Nirvana is restarting; this window comes back by itself in a few seconds.";

/// The client's wait after a restart's shutdown, told on `out` first: wait for the old server's sockets to go, and
/// the exit code (`client_exit`).
pub fn client_restart_wait(
    out: &mut dyn std::io::Write,
    gone: impl FnMut() -> bool,
    sleep: impl FnMut(Duration),
) -> i32 {
    let _ = writeln!(out, "{RESTARTING_LINE}");
    let _ = out.flush();
    let gone = wait_until(STOP_WAIT, POLL, gone, sleep);
    if !gone {
        let _ = writeln!(out, "nirvana: {NOT_STOPPED}");
    }
    client_exit(gone)
}

/// After the client left on a restart's shutdown: wait for the old server's sockets to go, then leave with the code
/// `supervise_code` starts the client again on.
pub fn finish_client_restart() -> ! {
    std::process::exit(client_restart_wait(
        &mut std::io::stderr(),
        old_sockets_gone,
        std::thread::sleep,
    ));
}

fn reachable(path: &Path) -> bool {
    crate::ipc::connect_local_stream(path).is_ok()
}

/// Neither of this session's sockets answers.
fn old_sockets_gone() -> bool {
    !reachable(&crate::session::active_api_socket_path())
        && !reachable(&crate::server::socket_paths::client_socket_path())
}

// ---- What happens, decided ----

/// What brings the server back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comeback {
    /// The Nirvana window's client, which starts the new server and attaches (the worker waits `WINDOW_WAIT`).
    Window,
    /// The worker, with no window.
    ServerAlone,
}

/// A live window mark is the Nirvana window's, so it counts only for the session that window runs.
pub fn comeback(window_live: bool, session: Option<&str>) -> Comeback {
    if window_live && session == Some(crate::nirvana_launch::SESSION) {
        Comeback::Window
    } else {
        Comeback::ServerAlone
    }
}

/// What the worker has seen so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    pub window: bool,
    /// Both old sockets went (kept once seen: a socket that answers after it is the new server's).
    pub old_gone: bool,
    /// A server listens on the session's client socket, after the old one went.
    pub listening: bool,
    pub since_stop: Duration,
    pub since_gone: Duration,
    /// Since the worker started a server itself.
    pub since_start: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Wait,
    StartServer,
    Done,
    Fail(&'static str),
}

/// The worker's next step over what it has seen.
pub fn next_step(facts: &Facts) -> Step {
    if !facts.old_gone {
        return if facts.since_stop >= STOP_WAIT {
            Step::Fail(NOT_STOPPED)
        } else {
            Step::Wait
        };
    }
    if facts.listening {
        return Step::Done;
    }
    match facts.since_start {
        Some(waited) if waited >= READY_WAIT => Step::Fail(NOT_LISTENING),
        Some(_) => Step::Wait,
        None if facts.window && facts.since_gone < WINDOW_WAIT => Step::Wait,
        None => Step::StartServer,
    }
}

/// Whether the command runs in a pane of the session it restarts: its pane's herdr socket is the session's.
pub fn in_own_pane(pane_socket: Option<&Path>, session_socket: &Path) -> bool {
    pane_socket.is_some_and(|socket| socket == session_socket)
}

const WINDOW_STAYS: &str =
    "nirvana: the Nirvana window stays: its client comes back on the new server.";
const NO_WINDOW: &str =
    "nirvana: no Nirvana window is open: the server comes back with no window (Open Nirvana opens one).";

/// What the command says before it starts: what stops, and what comes back.
pub fn notice(
    session: &str,
    working: Option<usize>,
    comeback: Comeback,
    own_pane: bool,
) -> Vec<String> {
    let at_work = match working {
        Some(0) => "no agent is working now".to_owned(),
        Some(1) => "1 agent is working now; its turn stops".to_owned(),
        Some(count) => format!("{count} agents are working now; their turns stop"),
        None => "the agents could not be counted".to_owned(),
    };
    let mut lines = vec![
        format!(
            "nirvana: restarting session {session}: {at_work}. Every program in its panes stops; Nirvana Claude comes \
             back in the panes that ran it."
        ),
        match comeback {
            Comeback::Window => WINDOW_STAYS.to_owned(),
            Comeback::ServerAlone => NO_WINDOW.to_owned(),
        },
    ];
    if own_pane {
        lines.push(format!(
            "nirvana: this terminal is a pane of session {session}, so it stops with it; the restart goes on by \
             itself and writes its outcome to {RESULT_FILE}."
        ));
    }
    lines
}

// ---- The worker ----

/// What the worker acts on; the tests replace it.
pub trait World {
    /// `command.invoke` with `RESTART_COMMAND`.
    fn ask_restart(&self) -> Result<(), String>;
    fn old_gone(&self) -> bool;
    fn listening(&self) -> bool;
    fn start_server(&self) -> Result<(), String>;
    fn now(&self) -> Instant;
    fn sleep(&self, step: Duration);
    /// The progress line the command reads: `stopping`, `stopped`, `starting`, `done` or `failed`.
    fn report(&self, state: &str, why: &str);
}

/// One restart: ask, wait for the old server to go, then for the new one, starting it when nothing else does.
/// `Ok` holds the time it took.
pub fn run(world: &dyn World, comeback: Comeback) -> Result<Duration, String> {
    let asked = world.now();
    world.report("stopping", "");
    if let Err(why) = world.ask_restart() {
        let why = format!("the server did not take the restart: {why}");
        world.report("failed", &why);
        return Err(why);
    }
    let mut gone_at: Option<Instant> = None;
    let mut started_at: Option<Instant> = None;
    loop {
        let now = world.now();
        if gone_at.is_none() && world.old_gone() {
            gone_at = Some(now);
            world.report("stopped", "");
        }
        let facts = Facts {
            window: comeback == Comeback::Window,
            old_gone: gone_at.is_some(),
            listening: gone_at.is_some() && world.listening(),
            since_stop: now.saturating_duration_since(asked),
            since_gone: gone_at.map_or(Duration::ZERO, |at| now.saturating_duration_since(at)),
            since_start: started_at.map(|at| now.saturating_duration_since(at)),
        };
        match next_step(&facts) {
            Step::Wait => world.sleep(POLL),
            Step::StartServer => {
                world.report("starting", "");
                if let Err(why) = world.start_server() {
                    let why = format!("the new server could not be started: {why}");
                    world.report("failed", &why);
                    return Err(why);
                }
                started_at = Some(now);
            }
            Step::Done => {
                let took = now.saturating_duration_since(asked);
                world.report("done", "");
                return Ok(took);
            }
            Step::Fail(why) => {
                world.report("failed", why);
                return Err(why.to_owned());
            }
        }
    }
}

/// What the worker says when the server refused `RESTART_COMMAND`. Upstream's `command_not_found` (`app/
/// custom_commands.rs`, "custom command manifest is stale; reload configuration") is what a server from a build before
/// plan 48 answers, which has no restart: `OLD_SERVER` says that, and how to get past it. Any other refusal keeps the
/// server's own words.
pub fn refusal(code: Option<&str>, message: &str) -> String {
    if code == Some("command_not_found") {
        OLD_SERVER.to_owned()
    } else {
        message.to_owned()
    }
}

/// The worker on this PC: herdr's own socket, server start and clock; its progress in `<data dir>/RESULT_FILE`.
struct Live {
    run: String,
    result: PathBuf,
}

impl World for Live {
    fn ask_restart(&self) -> Result<(), String> {
        let request = crate::api::schema::Request {
            id: "nirvana:restart".into(),
            method: crate::api::schema::Method::CommandInvoke(
                crate::api::schema::CommandInvokeParams {
                    command_id: RESTART_COMMAND.into(),
                    workspace_id: None,
                    tab_id: None,
                    pane_id: None,
                    selection: None,
                },
            ),
        };
        let response = crate::api::client::ApiClient::local()
            .request_value(&request)
            .map_err(|err| err.to_string())?;
        match response.get("error") {
            Some(error) => Err(refusal(
                error["code"].as_str(),
                error["message"].as_str().unwrap_or("error"),
            )),
            None => Ok(()),
        }
    }

    fn old_gone(&self) -> bool {
        old_sockets_gone()
    }

    fn listening(&self) -> bool {
        crate::server::autodetect::is_server_listening().unwrap_or(false)
    }

    fn start_server(&self) -> Result<(), String> {
        // As the window with no folder starts it: from the user's home, so the server's startup folder is not
        // wherever the worker was started.
        if let Some(home) = user_home() {
            let _ = std::env::set_current_dir(home);
        }
        crate::server::autodetect::spawn_server_daemon()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, step: Duration) {
        std::thread::sleep(step);
    }

    fn report(&self, state: &str, why: &str) {
        write_result(&self.result, &self.run, state, why);
    }
}

fn user_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn result_path() -> PathBuf {
    crate::session::data_dir().join(RESULT_FILE)
}

fn write_result(path: &Path, run: &str, state: &str, why: &str) {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let line = json!({"run": run, "state": state, "why": why, "at": at});
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, line.to_string());
}

/// The worker's state for `run` in the result file, when it is that run's: `(state, why)`.
pub fn read_result(text: &str, run: &str) -> Option<(String, String)> {
    let value: Value = serde_json::from_str(text).ok()?;
    (value["run"] == run).then(|| {
        (
            value["state"].as_str().unwrap_or("").to_owned(),
            value["why"].as_str().unwrap_or("").to_owned(),
        )
    })
}

/// The build a restart comes back on: the installed one, as `supervise_code` picks it, else this one.
fn restart_exe() -> std::io::Result<PathBuf> {
    crate::nirvana::home()
        .map(|home| crate::nirvana::installed_exe_in(&home))
        .filter(|exe| exe.exists())
        .map_or_else(std::env::current_exe, Ok)
}

/// The session's name as `--session` takes it.
fn session_arg(name: Option<&str>) -> String {
    name.unwrap_or(crate::session::DEFAULT_SESSION_NAME)
        .to_owned()
}

/// The worker's argv after the program.
pub fn worker_args(session: &str, run: &str, comeback: Comeback) -> Vec<String> {
    vec![
        "--session".into(),
        session.into(),
        "nirvana".into(),
        "restart".into(),
        "--worker".into(),
        run.into(),
        match comeback {
            Comeback::Window => "window".into(),
            Comeback::ServerAlone => "alone".into(),
        },
    ]
}

/// The hop's argv: the worker's, with `--hop` for `--worker`.
pub fn hop_args(session: &str, run: &str, comeback: Comeback) -> Vec<String> {
    worker_args(session, run, comeback)
        .into_iter()
        .map(|arg| {
            if arg == "--worker" {
                "--hop".to_owned()
            } else {
                arg
            }
        })
        .collect()
}

/// How long the caller waits for the hop to start the worker and leave.
const HOP_WAIT: Duration = Duration::from_secs(20);

/// Starts the worker out of the caller's process tree. herdr ends each pane's programs by walking the pane's process
/// tree by parent pid (`pane.rs` shutdown_pane_processes, `platform::session_processes`), so a worker that a command
/// in a pane started would end with that pane. A hop starts the worker and leaves at once, and the caller waits for
/// it: by the time the worker asks the server to stop, its parent is gone and no pane's tree reaches it. The worker
/// is started as herdr starts its own server (`launch_server_daemon_command`: detached, and through WMI when the
/// hop's job kills its processes on close). `run` names the worker's lines in the result file.
fn spawn_worker(session: &str, run: &str, comeback: Comeback) -> Result<(), String> {
    let exe = restart_exe().map_err(|err| err.to_string())?;
    let mut hop = std::process::Command::new(exe);
    hop.args(hop_args(session, run, comeback))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(home) = user_home() {
        hop.current_dir(home);
    }
    crate::platform::configure_background_command(&mut hop);
    let mut child = hop.spawn().map_err(|err| err.to_string())?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("the worker was not started ({status})")),
            Ok(None) if started.elapsed() < HOP_WAIT => std::thread::sleep(POLL),
            Ok(None) => {
                let _ = child.kill();
                return Err("the worker's start did not end".to_owned());
            }
            Err(err) => return Err(err.to_string()),
        }
    }
}

/// The hop's one step: the worker, detached as herdr's server is.
fn start_worker(session: &str, run: &str, comeback: Comeback) -> Result<u32, String> {
    let exe = restart_exe().map_err(|err| err.to_string())?;
    let mut command = std::process::Command::new(exe);
    command
        .args(worker_args(session, run, comeback))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(home) = user_home() {
        command.current_dir(home);
    }
    crate::platform::detach_server_daemon_command(&mut command);
    crate::platform::launch_server_daemon_command(&mut command).map_err(|err| err.to_string())
}

fn new_run() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

/// Whether the Nirvana window's mark is live now (read while the old server still listens: liveness asks it).
fn window_live_now() -> bool {
    crate::nirvana::home()
        .and_then(|home| crate::nirvana_launch::read_window_mark(&home))
        .is_some_and(|mark| crate::nirvana_launch::is_live_window_mark(&mark))
}

/// The phone's restart (`POST /api/restart`), from inside the server: the worker for this session, not waited on.
pub fn restart_from_phone() -> Result<(), String> {
    let name = crate::session::active_name();
    let comeback = comeback(window_live_now(), name.as_deref());
    spawn_worker(&session_arg(name.as_deref()), &new_run(), comeback)
}

fn comeback_arg(how: &str) -> Comeback {
    if how == "window" {
        Comeback::Window
    } else {
        Comeback::ServerAlone
    }
}

/// `herdr agent list`'s working count, as the phone's line counts it.
fn working_now() -> Option<usize> {
    let request: crate::api::schema::Request = serde_json::from_value(json!({
        "id": "nirvana:restart:agents", "method": "agent.list", "params": {}
    }))
    .ok()?;
    let response = crate::api::client::ApiClient::local()
        .request_value(&request)
        .ok()?;
    Some(crate::nirvana_remote::herdr::working(&response["result"]))
}

/// What `herdr nirvana restart <args>` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verb {
    Command,
    Help,
    Hop { run: String, how: String },
    Worker { run: String, how: String },
    Bad,
}

/// Fix round E: the usage the window's usage text points at (`herdr nirvana restart help`) is an answer, exit 0.
pub fn verb(args: &[String]) -> Verb {
    match args {
        [] => Verb::Command,
        [word] if matches!(word.as_str(), "help" | "--help" | "-h") => Verb::Help,
        [flag, run, how] if flag == "--hop" => Verb::Hop {
            run: run.clone(),
            how: how.clone(),
        },
        [flag, run, how] if flag == "--worker" => Verb::Worker {
            run: run.clone(),
            how: how.clone(),
        },
        _ => Verb::Bad,
    }
}

/// The session whose own API socket `socket` is (`session::api_socket_path_for`'s layout): `Some(None)` for the
/// default session's `<config>/herdr.sock`, `Some(Some(name))` for `<config>/sessions/<name>/herdr.sock`, `None` for
/// any other path. `sessions/default` is no session's: `--session default` is the config folder's own socket.
pub fn session_of_socket(socket: &Path, config_dir: &Path) -> Option<Option<String>> {
    if socket.file_name()? != "herdr.sock" {
        return None;
    }
    let folder = socket.parent()?;
    if folder == config_dir {
        return Some(None);
    }
    if folder.parent()? != config_dir.join("sessions") {
        return None;
    }
    let name = folder.file_name()?.to_str()?;
    if name == crate::session::DEFAULT_SESSION_NAME {
        return None;
    }
    crate::session::validate_name(name).ok()?;
    Some(Some(name.to_owned()))
}

/// Which session the plain `herdr nirvana restart` restarts, in `session::active_api_socket_path`'s order: `--session`
/// (`explicit`), else the terminal's `HERDR_SOCKET_PATH` (`socket`: every pane has its session's), else
/// `HERDR_SESSION` (`name`). Fix round G (review section 6): a socket that was not `name`'s told the owner to run
/// `herdr --session default ...`, which is wrong advice when the socket is another session's. The socket's own folder
/// names its session now; a socket that is no session's own names none.
pub fn target_session(
    explicit: bool,
    name: Option<&str>,
    socket: Option<&Path>,
    config_dir: &Path,
) -> Result<Option<String>, String> {
    match socket {
        Some(socket) if !explicit => session_of_socket(socket, config_dir).ok_or_else(|| {
            format!(
                "nirvana: this terminal's herdr socket ({}) is not a session's own; name the session: `herdr \
                 --session <name> nirvana restart`",
                socket.display()
            )
        }),
        _ => Ok(name.map(str::to_owned)),
    }
}

/// `herdr nirvana restart`; `--hop` and `--worker` (`<run> window|alone`) are its own steps.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    match verb(args) {
        Verb::Command => std::process::exit(command()),
        Verb::Help => {
            println!("{USAGE}");
            std::process::exit(0);
        }
        Verb::Hop { run: id, how } => {
            let session = session_arg(crate::session::active_name().as_deref());
            let started = start_worker(&session, &id, comeback_arg(&how));
            if let Err(why) = &started {
                write_result(&result_path(), &id, "failed", why);
            }
            std::process::exit(if started.is_ok() { 0 } else { 1 });
        }
        Verb::Worker { run: id, how } => {
            let world = Live {
                run: id,
                result: result_path(),
            };
            std::process::exit(if run(&world, comeback_arg(&how)).is_ok() {
                0
            } else {
                1
            });
        }
        Verb::Bad => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// The command: say what stops and what comes back, start the worker, wait on its outcome.
fn command() -> i32 {
    let pane_socket = std::env::var_os(crate::api::SOCKET_PATH_ENV_VAR).map(PathBuf::from);
    let name = match target_session(
        crate::session::explicit_session_requested(),
        crate::session::active_name().as_deref(),
        pane_socket.as_deref(),
        &crate::config::config_dir(),
    ) {
        Ok(name) => name,
        Err(why) => {
            eprintln!("{why}");
            return 2;
        }
    };
    let session = session_arg(name.as_deref());
    let session_socket = crate::session::api_socket_path_for(name.as_deref());
    if !reachable(&session_socket) {
        eprintln!("nirvana: session {session} is not running: there is nothing to restart");
        return 1;
    }
    let comeback = comeback(window_live_now(), name.as_deref());
    let own_pane = in_own_pane(pane_socket.as_deref(), &session_socket);
    // `ApiClient::local` asks `session::active_api_socket_path`, which `target_session` follows: the session's socket.
    for line in notice(&session, working_now(), comeback, own_pane) {
        println!("{line}");
    }
    let run = new_run();
    // The worker, started with `--session <session>`, writes here: that session's folder.
    let result = crate::session::data_dir_for(name.as_deref()).join(RESULT_FILE);
    if let Err(why) = spawn_worker(&session, &run, comeback) {
        eprintln!("nirvana: the restart did not start: {why}");
        return 1;
    }
    let started = Instant::now();
    let mut said_stopped = false;
    while started.elapsed() < ANSWER_WAIT {
        let seen = std::fs::read_to_string(&result)
            .ok()
            .and_then(|text| read_result(&text, &run));
        match seen
            .as_ref()
            .map(|(state, why)| (state.as_str(), why.as_str()))
        {
            Some(("done", _)) => {
                println!(
                    "nirvana: restarted in {:.1} s: the new server listens; Nirvana Claude comes back in its panes \
                     within about 30 s.",
                    started.elapsed().as_secs_f64()
                );
                return 0;
            }
            Some(("failed", why)) => {
                eprintln!("nirvana: the restart did not finish: {why}");
                return 1;
            }
            Some(("stopped", _)) if !said_stopped => {
                said_stopped = true;
                println!("nirvana: the old server stopped.");
            }
            _ => {}
        }
        std::thread::sleep(POLL);
    }
    eprintln!(
        "nirvana: no outcome from the restart in {} s; see {}",
        ANSWER_WAIT.as_secs(),
        result.display()
    );
    1
}

#[cfg(test)]
mod tests;
