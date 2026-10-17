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

// ---- Feature 2: Restart Claude from the tab menu ----
//
// The owner, 2026-10-15: "If i right click this tab and I have an option to restart the claude code
// insstance with that exact directory opened and that session resumed." The tab menu sends herdr's
// existing `command.invoke` with this id (no new API method, so herdr's protocol stays upstream's);
// `app/nirvana_restart.rs` stops the pane's Claude and types the command below into its shell.

/// The `command.invoke` id the tab menu's "Restart Claude" sends.
pub const RESTART_CLAUDE_COMMAND: &str = "nirvana.restart_claude";
/// The `command.invoke` id the Preload panel's "edit" sends, the file's path after it (feature 5).
pub const EDIT_FILE_COMMAND: &str = "nirvana.edit_file:";
/// The `command.invoke` id the Accounts panel's sign-in sends, the CLI's key after it (feature 11).
pub const ACCOUNT_LOGIN_COMMAND: &str = "nirvana.account_login:";

/// The file the Preload panel may open in the editor: an absolute path to an existing markdown file.
/// Everything Preload offers to edit is one (CLAUDE.md, MEMORY.md, SKILL.md, an agent's file); the check
/// keeps the command from opening anything else a client names.
pub fn editable_file(file: &str) -> Option<PathBuf> {
    let path = PathBuf::from(file);
    let markdown = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    (path.is_absolute() && markdown && path.is_file()).then_some(path)
}

/// The terminal editors "edit" tries, first found, each with the arguments that give it the mouse.
/// Microsoft Edit is part of Windows 11 (`edit`, 1.2.1 on this machine on 2026-10-16) and takes clicks,
/// selection, the wheel and its menus by itself; elsewhere it installs as `msedit`, because Debian's
/// `edit` is run-mailcap's. `$VISUAL` and `$EDITOR` are not tried: either may name a window editor, and the
/// owner asked for a terminal one that takes the mouse ("not open through notepad").
#[cfg(windows)]
const TERMINAL_EDITORS: [(&str, &[&str]); 3] =
    [("edit", &[]), ("micro", &[]), ("nano", &["--mouse"])];
#[cfg(not(windows))]
const TERMINAL_EDITORS: [(&str, &[&str]); 3] =
    [("msedit", &[]), ("micro", &[]), ("nano", &["--mouse"])];

/// The argv that opens `path` in a terminal editor: `NIRVANA_EDITOR` (one program, run with the file) when
/// set, else the first of `TERMINAL_EDITORS` on the PATH. `None` when there is none.
pub fn terminal_editor_argv(path: &Path) -> Option<Vec<String>> {
    terminal_editor_argv_with(path, std::env::var("NIRVANA_EDITOR").ok(), find_program)
}

fn terminal_editor_argv_with(
    path: &Path,
    chosen: Option<String>,
    find: impl Fn(&str) -> Option<PathBuf>,
) -> Option<Vec<String>> {
    let file = path.display().to_string();
    if let Some(chosen) = chosen.filter(|chosen| !chosen.trim().is_empty()) {
        return Some(vec![chosen.trim().to_owned(), file]);
    }
    TERMINAL_EDITORS.iter().find_map(|(program, args)| {
        let found = find(program)?;
        let mut argv = vec![found.display().to_string()];
        argv.extend(args.iter().map(|arg| (*arg).to_owned()));
        argv.push(file.clone());
        Some(argv)
    })
}

/// A program on the PATH. On Windows only `.exe` and `.com`: a pane runs its argv directly, and a batch
/// file needs `cmd`.
fn find_program(name: &str) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        vec![format!("{name}.exe"), format!("{name}.com")]
    } else {
        vec![name.to_owned()]
    };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

/// How long the restart waits for the pane's shell to be free after Claude is stopped.
pub const RESTART_SHELL_WAIT: Duration = Duration::from_secs(3);

/// Claude Code's config folder: `CLAUDE_CONFIG_DIR`, else `~/.claude` (Claude Code's own rule).
pub fn claude_config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()))
        .map(|home| PathBuf::from(home).join(".claude"))
}

/// Stops processes now. herdr's Windows `signal_processes` opens each process with query rights only,
/// so its `TerminateProcess` is refused and the process lives on (live run 2026-10-15: Claude pid 53612
/// survived a restart). Here the handle asks for `PROCESS_TERMINATE`.
pub fn terminate_processes(pids: &[u32]) {
    #[cfg(windows)]
    for &pid in pids {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, TerminateProcess, PROCESS_TERMINATE,
        };
        // SAFETY: OpenProcess returns null on failure, which is checked; the handle is closed once.
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !handle.is_null() {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
    #[cfg(not(windows))]
    crate::platform::signal_processes(pids, crate::platform::Signal::Kill);
}

/// The bytes typed into the pane's shell: the command and a carriage return, nothing else. herdr's
/// `encode_api_submission` follows the terminal's modes, and a stopped Claude leaves bracketed paste and
/// its keyboard protocol on: the live run 2026-10-15 got `[f((Get-Command claude ...}[` at the prompt,
/// never submitted. herdr's own restore path types a plain line into a fresh shell the same way.
pub fn shell_submission(command: &str) -> Vec<u8> {
    let mut bytes = command.as_bytes().to_vec();
    bytes.push(b'\r');
    bytes
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeSession {
    pub session_id: String,
    pub cwd: Option<PathBuf>,
}

/// The conversation a running Claude holds. Claude Code writes `<config>/sessions/<pid>.json` for each
/// interactive Claude with its `sessionId` and `cwd` (read on this machine 2026-10-15, Claude Code
/// 2.1.300). A file whose `pid` is another process is a stale one left by a reused pid and is ignored.
pub fn read_claude_session(config_dir: &Path, pid: u32) -> Option<ClaudeSession> {
    let text =
        std::fs::read_to_string(config_dir.join("sessions").join(format!("{pid}.json"))).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    if value.get("pid").and_then(serde_json::Value::as_u64) != Some(u64::from(pid)) {
        return None;
    }
    let session_id = value.get("sessionId")?.as_str()?.trim();
    let id_ok = !session_id.is_empty()
        && session_id.len() <= 128
        && session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !id_ok {
        return None;
    }
    Some(ClaudeSession {
        session_id: session_id.to_string(),
        cwd: value
            .get("cwd")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
    })
}

/// The arguments for the restarted Claude: the old Claude's own arguments (the Nirvana theme's
/// `--settings`, a model, an effort) without any earlier resume, then `--resume <id>`, or
/// `--continue` (the newest conversation in the folder) when no id was found.
pub fn restart_args(old_args: &[String], session_id: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    let mut rest = old_args.iter().peekable();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--continue" | "-c" | "--fork-session" => {}
            "--resume" | "-r" | "--session-id" => {
                // `-r` alone opens Claude's picker; a following value is the session it named.
                if rest.peek().is_some_and(|next| !next.starts_with('-')) {
                    rest.next();
                }
            }
            other if other.starts_with("--resume=") || other.starts_with("--session-id=") => {}
            _ => args.push(arg.clone()),
        }
    }
    match session_id {
        Some(id) => args.extend(["--resume".to_string(), id.to_string()]),
        None => args.push("--continue".to_string()),
    }
    args
}

/// The arguments for restarting a Claude whose conversation was never saved: one that has had no
/// prompt yet. Claude Code writes the transcript at the first prompt, and `--resume <id>` then fails
/// with "No conversation found with session ID" and leaves the shell (live 2026-10-16, Preload's `r`
/// after switching the Jev guide on in a fresh Claude). `--continue` would open another conversation,
/// so the restart starts fresh, with the same flags.
pub fn restart_args_unsaved(old_args: &[String]) -> Vec<String> {
    let mut args = restart_args(old_args, None);
    args.pop(); // the `--continue` fallback
    args
}

/// Whether Claude Code saved the conversation: `<config>/projects/<folder>/<id>.jsonl`, in any folder.
pub fn conversation_saved(config_dir: &Path, session_id: &str) -> bool {
    let file = format!("{session_id}.jsonl");
    std::fs::read_dir(config_dir.join("projects"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| entry.path().join(&file).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_existing_absolute_markdown_file_is_editable() {
        let dir = std::env::temp_dir().join(format!("nirvana-editable-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let markdown = dir.join("CLAUDE.md");
        let text = dir.join("notes.txt");
        std::fs::write(&markdown, "x").expect("md");
        std::fs::write(&text, "x").expect("txt");
        assert_eq!(
            editable_file(&markdown.display().to_string()),
            Some(markdown.clone())
        );
        assert_eq!(editable_file(&text.display().to_string()), None);
        assert_eq!(editable_file("CLAUDE.md"), None);
        assert_eq!(
            editable_file(&dir.join("missing.md").display().to_string()),
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_opens_a_terminal_editor_that_takes_the_mouse_never_notepad() {
        let file = Path::new("/work/proj/CLAUDE.md");
        let shown = file.display().to_string();
        let on_path = |names: &'static [&'static str]| {
            move |name: &str| {
                names
                    .contains(&name)
                    .then(|| PathBuf::from(format!("/bin/{name}")))
            }
        };
        let first = TERMINAL_EDITORS[0].0;
        // The first editor found wins; nano only with its mouse switch.
        assert_eq!(
            terminal_editor_argv_with(file, None, on_path(&["edit", "msedit", "micro", "nano"])),
            Some(vec![format!("/bin/{first}"), shown.clone()])
        );
        assert_eq!(
            terminal_editor_argv_with(file, None, on_path(&["nano"])),
            Some(vec!["/bin/nano".into(), "--mouse".into(), shown.clone()])
        );
        // NIRVANA_EDITOR wins over every editor found.
        assert_eq!(
            terminal_editor_argv_with(file, Some(" hx ".into()), on_path(&["micro"])),
            Some(vec!["hx".into(), shown.clone()])
        );
        // Nothing found: no editor, so the panel says so; Notepad is never the answer.
        assert_eq!(
            terminal_editor_argv_with(file, None, on_path(&["notepad"])),
            None
        );
        assert_eq!(
            terminal_editor_argv_with(file, Some("  ".into()), on_path(&[])),
            None
        );
    }

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

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn the_session_file_names_the_running_claudes_conversation() {
        let dir = scratch("claude-session");
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        std::fs::write(
            dir.join("sessions").join("4242.json"),
            r#"{"pid":4242,"sessionId":"287150ad-a664-40ae-bcff-1976a2fcd814","cwd":"C:/work/moto","kind":"interactive"}"#,
        )
        .unwrap();
        let session = read_claude_session(&dir, 4242).unwrap();
        assert_eq!(session.session_id, "287150ad-a664-40ae-bcff-1976a2fcd814");
        assert_eq!(session.cwd, Some(PathBuf::from("C:/work/moto")));
        // No file for that pid: no id, so the restart falls back to --continue.
        assert_eq!(read_claude_session(&dir, 4243), None);
    }

    #[test]
    fn a_session_file_left_by_another_process_is_not_used() {
        let dir = scratch("claude-stale");
        std::fs::create_dir_all(dir.join("sessions")).unwrap();
        std::fs::write(
            dir.join("sessions").join("77.json"),
            r#"{"pid":78,"sessionId":"aaaa-bbbb"}"#,
        )
        .unwrap();
        assert_eq!(read_claude_session(&dir, 77), None);
        std::fs::write(
            dir.join("sessions").join("77.json"),
            r#"{"pid":77,"sessionId":"x; rm -rf /"}"#,
        )
        .unwrap();
        assert_eq!(read_claude_session(&dir, 77), None);
    }

    #[test]
    fn restart_keeps_the_flags_and_resumes_the_same_conversation() {
        let old = strings(&[
            "--settings",
            r#"{"theme":"custom:nirvana-dark"}"#,
            "--resume",
            "old-id",
            "--model",
            "opus",
            "-c",
            "--fork-session",
        ]);
        assert_eq!(
            restart_args(&old, Some("new-id")),
            strings(&[
                "--settings",
                r#"{"theme":"custom:nirvana-dark"}"#,
                "--model",
                "opus",
                "--resume",
                "new-id",
            ])
        );
        assert_eq!(
            restart_args(&strings(&["-r", "--verbose", "--session-id=abc"]), None),
            strings(&["--verbose", "--continue"])
        );
    }

    #[test]
    fn a_claude_with_no_saved_conversation_restarts_fresh() {
        let dir = std::env::temp_dir().join(format!("nirvana-unsaved-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("projects").join("C--work-proj")).expect("projects");
        assert!(!conversation_saved(&dir, "abc"));
        std::fs::write(
            dir.join("projects").join("C--work-proj").join("abc.jsonl"),
            "{}\n",
        )
        .expect("transcript");
        assert!(conversation_saved(&dir, "abc"));
        assert_eq!(
            restart_args_unsaved(&strings(&["--model", "sonnet", "--resume", "abc"])),
            strings(&["--model", "sonnet"])
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn terminate_really_stops_a_running_process() {
        // The live run's bug: a query-only handle left Claude running. A real child must be gone.
        let mut child = if cfg!(windows) {
            std::process::Command::new("ping")
                .args(["-n", "30", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .spawn()
        } else {
            std::process::Command::new("sleep").arg("30").spawn()
        }
        .unwrap();
        terminate_processes(&[child.id()]);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut exited = false;
        while std::time::Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // Reap it either way; a survivor is killed through std so the test leaves nothing behind.
        let _ = child.kill();
        let _ = child.wait();
        assert!(exited, "the process outlived terminate_processes");
    }

    #[test]
    fn the_restart_command_is_typed_as_a_plain_line() {
        let bytes = shell_submission("& claude '--resume' abc");
        assert_eq!(bytes, b"& claude '--resume' abc\r");
        assert!(
            !bytes.contains(&0x1b),
            "no escape sequence reaches the shell"
        );
    }
}
