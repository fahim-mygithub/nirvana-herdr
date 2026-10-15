//! The Nirvana fork's own identity: its version, the installed build it can reload onto, and the
//! supervisor that relaunches it.
//!
//! The owner, 2026-10-15: "For our first implementation similar to Nirvana hot realoding the newest
//! changes and keeping track of the project have this element for the Herdr instance" (the Nirvana
//! sidebar header: mark, name, "alpha X.Y.Z"). Nirvana draws one version per merge beside its name and
//! offers a Reload once the code moved past the window (Nirvana `ui/components/shell.js`). Here:
//!
//! - `VERSION` is the fork's number from `nirvana/VERSION`, separate from herdr's Cargo version, which
//!   gates herdr's own update manifest and protocol checks.
//! - `nirvana/install.py` installs a build to `<home>/bin/herdr.exe` with a stamp beside it. The
//!   server's background version check, which upstream points at herdr's releases, reads that stamp
//!   instead and raises herdr's existing `UpdateReady` when it is newer than the running build.
//! - A confirmed click on the header's reload badge detaches with `request_reload()` set; `main`
//!   then stops the server and exits with `RELOAD_EXIT_CODE`, and `supervise` starts the newly
//!   installed build, which starts a new server that restores the saved session.
//!
//! Kept in one file with small hooks in upstream files, so `git merge upstream/master` stays easy.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

pub const VERSION: &str = env!("NIRVANA_VERSION");
/// The exit code that asks `supervise` to start the installed build again (EX_TEMPFAIL).
pub const RELOAD_EXIT_CODE: i32 = 75;
pub const STAMP_FILE: &str = "nirvana-version.txt";
pub const HOME_ENV: &str = "NIRVANA_HERDR_HOME";
/// How often the server reads the stamp. One small file read; the owner wants a new build seen soon.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(5);
/// What herdr's update toast says to do, in place of `herdr update`.
pub const RELOAD_COMMAND: &str = "nirvana reload";

static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);
static RELOAD_ARMED: AtomicBool = AtomicBool::new(false);
static LAST_ANNOUNCED: Mutex<Option<String>> = Mutex::new(None);

pub fn label(version: &str) -> String {
    format!("alpha {version}")
}

/// The fork's install root: `$NIRVANA_HERDR_HOME`, else `%LOCALAPPDATA%\Programs\Nirvana-Herdr`.
pub fn home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(home));
    }
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map(|base| PathBuf::from(base).join("Programs").join("Nirvana-Herdr"))
}

pub fn installed_exe_in(home: &Path) -> PathBuf {
    home.join("bin")
        .join(if cfg!(windows) { "herdr.exe" } else { "herdr" })
}

pub fn read_stamp(home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(home.join("bin").join(STAMP_FILE)).ok()?;
    let version = text.trim();
    parse(version).map(|_| version.to_string())
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split('.');
    let triple = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(triple)
}

/// Whether `installed` is a later build than `running`. An unreadable number is never newer.
pub fn is_newer(running: &str, installed: &str) -> bool {
    match (parse(running), parse(installed)) {
        (Some(running), Some(installed)) => installed > running,
        _ => false,
    }
}

/// The installed build's version when it is newer than this one.
pub fn newer_installed_in(home: &Path, running: &str) -> Option<String> {
    read_stamp(home).filter(|installed| is_newer(running, installed))
}

/// The fork's background version check (replaces upstream's release-manifest check). Announces
/// each newer build once, so the 5 s cadence never repeats a toast.
pub fn check_installed(events: &tokio::sync::mpsc::Sender<crate::events::AppEvent>) {
    let Some(version) = home().and_then(|home| newer_installed_in(&home, VERSION)) else {
        return;
    };
    let mut last = LAST_ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(version.as_str()) {
        return;
    }
    *last = Some(version.clone());
    tracing::info!(version, "nirvana: a newer build is installed");
    let _ = events.blocking_send(crate::events::AppEvent::UpdateReady {
        version,
        install_command: RELOAD_COMMAND.to_string(),
    });
}

/// A click on the reload badge: the first arms it, the second confirms. Reload stops the server, so
/// the programs in its panes stop too (Windows has no live handoff); one click must not do that.
pub fn press_reload() -> bool {
    if RELOAD_ARMED.swap(false, Ordering::Relaxed) {
        RELOAD_REQUESTED.store(true, Ordering::Relaxed);
        true
    } else {
        RELOAD_ARMED.store(true, Ordering::Relaxed);
        false
    }
}

pub fn disarm_reload() -> bool {
    RELOAD_ARMED.swap(false, Ordering::Relaxed)
}

pub fn reload_armed() -> bool {
    RELOAD_ARMED.load(Ordering::Relaxed)
}

pub fn reload_requested() -> bool {
    RELOAD_REQUESTED.load(Ordering::Relaxed)
}

/// After the client detached for a reload: stop this session's server (it saves the session on the
/// way down) and leave with the code `supervise` waits for.
pub fn finish_reload() -> ! {
    if let Err(err) = crate::session::stop_active_server() {
        eprintln!("nirvana: the server did not stop: {err}");
        std::process::exit(1);
    }
    std::process::exit(RELOAD_EXIT_CODE);
}

/// `herdr nirvana-shell [args]`: run the installed build with `args` until it exits with anything
/// but `RELOAD_EXIT_CODE`. Reads no input, so the client it runs owns the terminal alone.
pub fn supervise(args: &[String]) -> std::io::Result<()> {
    loop {
        let exe = home()
            .map(|home| installed_exe_in(&home))
            .filter(|exe| exe.exists())
            .map_or_else(std::env::current_exe, Ok)?;
        let status = std::process::Command::new(&exe).args(args).status()?;
        match status.code() {
            Some(RELOAD_EXIT_CODE) => continue,
            Some(code) => std::process::exit(code),
            None => std::process::exit(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nirvana-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        dir
    }

    #[test]
    fn version_comes_from_the_version_file() {
        let file = include_str!("../nirvana/VERSION");
        assert_eq!(VERSION, file.trim());
        assert!(parse(VERSION).is_some(), "{VERSION} is not X.Y.Z");
        assert_eq!(label("0.1.0"), "alpha 0.1.0");
    }

    #[test]
    fn only_a_later_build_is_newer() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.9", "0.2.0"));
        assert!(is_newer("0.9.0", "0.10.0"));
        assert!(!is_newer("0.1.1", "0.1.1"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("0.1.0", "garbage"));
        assert!(!is_newer("0.1.0", "0.1.1.1"));
    }

    #[test]
    fn the_stamp_beside_the_installed_build_is_read() {
        let home = scratch("stamp");
        assert_eq!(newer_installed_in(&home, "0.1.0"), None);
        std::fs::write(home.join("bin").join(STAMP_FILE), "0.1.1\r\n").unwrap();
        assert_eq!(newer_installed_in(&home, "0.1.0").as_deref(), Some("0.1.1"));
        assert_eq!(newer_installed_in(&home, "0.1.1"), None);
        std::fs::write(home.join("bin").join(STAMP_FILE), "not a version").unwrap();
        assert_eq!(newer_installed_in(&home, "0.1.0"), None);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn reload_takes_two_presses() {
        RELOAD_REQUESTED.store(false, Ordering::Relaxed);
        RELOAD_ARMED.store(false, Ordering::Relaxed);
        assert!(!press_reload());
        assert!(reload_armed() && !reload_requested());
        assert!(press_reload());
        assert!(reload_requested() && !reload_armed());
        RELOAD_REQUESTED.store(false, Ordering::Relaxed);
        assert!(!press_reload());
        assert!(disarm_reload());
        assert!(!press_reload());
        RELOAD_ARMED.store(false, Ordering::Relaxed);
    }
}
