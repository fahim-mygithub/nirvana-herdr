//! Nirvana fork, dead panes (nirvana/plans/2026-11-08-dead-panes.md): the server's half of the sweep.
//!
//! Every `PASS_EVERY` this starts `herdr run sweep` as a child against this server, as the wake retry starts `herdr
//! run wake` (`nirvana_wake_retry`): the sweep asks the server about panes through the socket, which this loop must
//! stay free to answer. The rules are `nirvana_dead::decide`'s. The first pass waits `FIRST_PASS_AFTER` from the
//! server's start, so a restored session's panes have their shells before their processes are read; the backlog (116
//! panes on 2026-11-08) drains `nirvana_dead::MAX_CLOSES` a pass. One child at a time, and only in the Nirvana
//! session's server (review S6, `nirvana_dead::sweeps_in`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::App;

/// Between passes. A finished pane's grace is `nirvana_dead::CLOSE_GRACE_S` (2 min), so it closes 2 to 3 min after its
/// worker ended.
const PASS_EVERY: Duration = Duration::from_secs(60);
/// From the first loop pass of the server to its first sweep.
const FIRST_PASS_AFTER: Duration = Duration::from_secs(60);
/// A sweep child still running after this is killed (review S1): a pass is at most 2 lists, a `process-info` per
/// candidate and 25 closes, seconds in all; each call gives up after `nirvana_dead::CALL_LIMIT`.
const SWEEP_LIMIT: Duration = Duration::from_secs(300);

static NEXT_PASS: Mutex<Option<Instant>> = Mutex::new(None);
static SWEEPING: AtomicBool = AtomicBool::new(false);
/// The server's start in unix seconds, as its first loop pass saw it: a stopped order's pane waits an hour from it
/// (`nirvana_dead::STOPPED_GRACE_S`).
static SERVER_STARTED: OnceLock<u64> = OnceLock::new();

impl App {
    /// The instant of the next sweep, for the loop's deadline, so an idle server still sweeps.
    pub(crate) fn dead_panes_deadline(&self) -> Option<Instant> {
        *NEXT_PASS.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a sweep when one is due. `watched` names the tabs the server's clients have focused, asked only when a
    /// pass starts. Never changes what is drawn: the closes reach the clients as the API's own.
    pub(crate) fn nirvana_dead_panes(
        &mut self,
        now: Instant,
        watched: impl FnOnce() -> Vec<String>,
    ) -> bool {
        // A unit test's App never sweeps a session: the sweep would reach the server the test process runs in.
        if cfg!(test) {
            return false;
        }
        if !crate::nirvana_dead::sweeps_in(crate::session::active_name().as_deref()) {
            return false;
        }
        let server_started = *SERVER_STARTED.get_or_init(crate::nirvana_run::now);
        {
            let mut next = NEXT_PASS.lock().unwrap_or_else(|e| e.into_inner());
            match *next {
                None => {
                    *next = Some(now + FIRST_PASS_AFTER);
                    return false;
                }
                Some(due) if now < due => return false,
                Some(_) => *next = Some(now + PASS_EVERY),
            }
        }
        if crate::nirvana::home().is_none() || SWEEPING.swap(true, Ordering::SeqCst) {
            return false;
        }
        if !spawn_sweep(server_started, &watched()) {
            SWEEPING.store(false, Ordering::SeqCst);
        }
        false
    }
}

/// `herdr run sweep` against this server; a thread reaps the child and frees the next pass. The child writes its own
/// log (`nirvana_dead::LOG_FILE`).
fn spawn_sweep(server_started: u64, watched: &[String]) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut command = std::process::Command::new(exe);
    command
        .args(["run", "sweep", "--server-started"])
        .arg(server_started.to_string());
    for tab in watched {
        command.args(["--watched", tab.as_str()]);
    }
    command
        .env(crate::api::SOCKET_PATH_ENV_VAR, crate::api::socket_path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    match command.spawn() {
        Ok(child) => std::thread::Builder::new()
            .name("nirvana-dead-panes".into())
            .spawn(move || reap(child, SWEEP_LIMIT, crate::nirvana::home().as_deref()))
            .is_ok(),
        Err(err) => {
            tracing::info!(%err, "nirvana: the dead-pane sweep could not start");
            false
        }
    }
}

/// Waits for the sweep child up to `limit`, kills it past that with a line in the sweep's log, and frees the next
/// pass whatever happened (review S1: a hung child held `SWEEPING` for ever, and no later pass ran).
fn reap(mut child: std::process::Child, limit: Duration, home: Option<&std::path::Path>) {
    let failed = match crate::nirvana_dead::wait_within(&mut child, limit) {
        Ok(Some(_)) => None,
        Ok(None) => Some(format!(
            "the sweep ran past {} s and was killed",
            limit.as_secs_f32()
        )),
        Err(err) => Some(format!("the sweep could not be waited on: {err}")),
    };
    if let Some(line) = failed {
        tracing::info!(line, "nirvana: dead-pane sweep");
        if let Some(home) = home {
            crate::nirvana_dead::log_lines(home, &[line]);
        }
    }
    SWEEPING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A program that runs for half a minute and prints nothing: the hung sweep.
    fn hanging() -> std::process::Command {
        let mut command = if cfg!(windows) {
            let mut command = std::process::Command::new("ping");
            command.args(["-n", "30", "127.0.0.1"]);
            command
        } else {
            let mut command = std::process::Command::new("sleep");
            command.arg("30");
            command
        };
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command
    }

    #[test]
    fn a_hung_sweep_is_killed_logged_and_frees_the_next_pass() {
        let home = std::env::temp_dir().join(format!(
            "nirvana_dead_reap_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&home).unwrap();
        SWEEPING.store(true, Ordering::SeqCst);
        let child = hanging().spawn().unwrap();
        let started = Instant::now();
        reap(child, Duration::from_millis(300), Some(&home));
        let took = started.elapsed();
        assert!(
            took < Duration::from_secs(10),
            "the reaper waited {took:?}, not the limit"
        );
        assert!(
            !SWEEPING.load(Ordering::SeqCst),
            "the next pass is free again"
        );
        let log = std::fs::read_to_string(
            home.join(crate::nirvana_seat::DIR)
                .join(crate::nirvana_dead::LOG_FILE),
        )
        .unwrap_or_default();
        let _ = std::fs::remove_dir_all(&home);
        assert!(log.contains("was killed"), "the log says so: {log:?}");
    }
}
