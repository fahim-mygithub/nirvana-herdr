//! Nirvana fork, dead panes (nirvana/plans/2026-11-08-dead-panes.md): the server's half of the sweep.
//!
//! Every `PASS_EVERY` this starts `herdr run sweep` as a child against this server, as the wake retry starts `herdr
//! run wake` (`nirvana_wake_retry`): the sweep asks the server about panes through the socket, which this loop must
//! stay free to answer. The rules are `nirvana_dead::decide`'s. The first pass waits `FIRST_PASS_AFTER` from the
//! server's start, so a restored session's panes have their shells before their processes are read; that pass is the
//! one that takes the backlog (116 panes on 2026-11-08). One child at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::App;

/// Between passes. A finished pane's grace is `nirvana_dead::CLOSE_GRACE_S` (2 min), so it closes 2 to 3 min after its
/// worker ended.
const PASS_EVERY: Duration = Duration::from_secs(60);
/// From the first loop pass of the server to its first sweep.
const FIRST_PASS_AFTER: Duration = Duration::from_secs(60);

static NEXT_PASS: Mutex<Option<Instant>> = Mutex::new(None);
static SWEEPING: AtomicBool = AtomicBool::new(false);

impl App {
    /// The instant of the next sweep, for the loop's deadline, so an idle server still sweeps.
    pub(crate) fn dead_panes_deadline(&self) -> Option<Instant> {
        *NEXT_PASS.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts a sweep when one is due. Never changes what is drawn: the closes reach the clients as the API's own.
    pub(crate) fn nirvana_dead_panes(&mut self, now: Instant) -> bool {
        // A unit test's App never sweeps a session: the sweep would reach the server the test process runs in.
        if cfg!(test) {
            return false;
        }
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
        if !spawn_sweep() {
            SWEEPING.store(false, Ordering::SeqCst);
        }
        false
    }
}

/// `herdr run sweep` against this server; a thread reaps the child and frees the next pass. The child writes its own
/// log (`nirvana_dead::LOG_FILE`).
fn spawn_sweep() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut command = std::process::Command::new(exe);
    command
        .args(["run", "sweep"])
        .env(crate::api::SOCKET_PATH_ENV_VAR, crate::api::socket_path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    match command.spawn() {
        Ok(mut child) => std::thread::Builder::new()
            .name("nirvana-dead-panes".into())
            .spawn(move || {
                let _ = child.wait();
                SWEEPING.store(false, Ordering::SeqCst);
            })
            .is_ok(),
        Err(err) => {
            tracing::info!(%err, "nirvana: the dead-pane sweep could not start");
            false
        }
    }
}
