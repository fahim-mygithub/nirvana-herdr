//! Nirvana fork, plan 31 review W: the server's half of a queued wake's retry.
//!
//! Measured 2026-10-29 (records/p31a.md §2.3): four deliveries had one wake attempt each, which queued, and nothing
//! tried again, so they waited 1,132 s to 25,798 s for the owner's next prompt. A wake that queues puts its run on a
//! list under the Nirvana home (`nirvana_run::schedule_retry`); this pass runs `herdr run wake <run>` for each run due,
//! which applies `wake_decision`'s rules again, and the wake takes the run off the list once it delivers or nothing
//! waits. The `UserPromptSubmit` notice takes it off too, and the wake reads the journal again under its lock before
//! it types, so a queue is delivered once.
//!
//! The wake runs as a child process, as `herdr run tick` would: it asks this server about the pane through the
//! socket, which this loop must stay free to answer.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::App;

/// How often the pass reads the list. The first retry waits 30 s, so a pass every 10 s is late by at most a third.
const PASS_EVERY: Duration = Duration::from_secs(10);

static NEXT_PASS: Mutex<Option<Instant>> = Mutex::new(None);

impl App {
    /// Starts a wake for each run whose retry is due. Never changes what is drawn.
    pub(crate) fn nirvana_wake_retry(&mut self, now: Instant) -> bool {
        {
            let mut next = NEXT_PASS.lock().unwrap_or_else(|e| e.into_inner());
            if next.is_some_and(|due| now < due) {
                return false;
            }
            *next = Some(now + PASS_EVERY);
        }
        let Some(home) = crate::nirvana::home() else {
            return false;
        };
        for retry in crate::nirvana_run::take_due_retries(&home, crate::nirvana_run::now()) {
            spawn_wake(&retry);
        }
        false
    }
}

/// `herdr run wake <run>` in the run's project, against this server. Its answer goes to the run's journal
/// (`wake_queued`, `woken`), so nothing here waits on it; a thread reaps the child.
fn spawn_wake(retry: &crate::nirvana_run::WakeRetry) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = std::process::Command::new(exe);
    command
        .args(["run", "wake", &retry.run])
        .env(crate::nirvana_run::PROJECT_ENV, &retry.root)
        .env(crate::api::SOCKET_PATH_ENV_VAR, crate::api::socket_path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    match command.spawn() {
        Ok(mut child) => {
            let _ = std::thread::Builder::new()
                .name("nirvana-wake-retry".into())
                .spawn(move || {
                    let _ = child.wait();
                });
        }
        Err(err) => {
            tracing::info!(run = %retry.run, %err, "nirvana: the wake retry could not start");
        }
    }
}
