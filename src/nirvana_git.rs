//! Nirvana fork, plan 16: the Git square (package `p16`).
//! Old Nirvana's GitHub card ported to the mod strip:
//! local git, Pull/Push/Fetch with a confirm step, CI line from gh.
//! (nirvana/plans/2026-10-19-16-git.md §2)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maximum changed files listed in a git status row / panel view.
/// Matching old Nirvana's git-widget: `FILES_SHOWN = 8`.
pub const FILES_SHOWN: usize = 8;

/// Timeout for local read-only status.
pub const STATUS_TIMEOUT: Duration = Duration::from_secs(10);

/// The parsed state of a git repository from `git status --porcelain=v2 --branch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GitStatus {
    pub repo: PathBuf,
    pub branch: String,
    pub upstream: String,
    pub ahead: usize,
    pub behind: usize,
    pub dirty: usize,
    pub files: Vec<String>,
}

/// The state of a GitHub Actions workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CiState {
    Running,
    Passed,
    Failed,
}

impl CiState {
    pub fn mark(&self) -> &'static str {
        match self {
            Self::Running => "\u{25cf}", // ●
            Self::Passed => "\u{2713}",  // ✓
            Self::Failed => "\u{2717}",  // ✗
        }
    }
}

/// A parsed GitHub Actions workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiRun {
    pub workflow_name: String,
    pub state: CiState,
    pub conclusion_word: String,
    pub created_at_secs: u64,
    pub url: String,
}

/// The cached view of a folder's git state and optional CI run.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitView {
    pub status: GitStatus,
    pub ci: Option<CiRun>,
}

/// Extracts the file path from a porcelain v2 entry line.
///
/// Format specifications (git-status(1)):
/// - `1 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
/// - `2 <xy> <sub> <mH> <mI> <mW> <hH> <hI> <X><score> <path><TAB><origPath>`
/// - `u <xy> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
/// - `? <path>`
///
/// For renames (`2`), cutting at the tab keeps `<path>` (the new path).
pub fn extract_porcelain_path(line: &str) -> String {
    let raw = if let Some(untracked) = line.strip_prefix("? ") {
        untracked
    } else if line.starts_with("1 ") {
        line.splitn(9, ' ').nth(8).unwrap_or("")
    } else if line.starts_with("2 ") {
        let field = line.splitn(10, ' ').nth(9).unwrap_or("");
        field.split('\t').next().unwrap_or(field)
    } else if line.starts_with("u ") {
        line.splitn(11, ' ').nth(10).unwrap_or("")
    } else {
        line.split('\t')
            .next()
            .and_then(|l| l.rsplit(' ').next())
            .unwrap_or(line)
    };
    raw.strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .unwrap_or(raw)
        .to_string()
}

/// Parses the output of `git status --porcelain=v2 --branch`.
pub fn parse_porcelain_v2(text: &str, repo: PathBuf) -> GitStatus {
    let mut status = GitStatus {
        repo,
        branch: String::new(),
        upstream: String::new(),
        ahead: 0,
        behind: 0,
        dirty: 0,
        files: Vec::new(),
    };

    for line in text.lines() {
        let line = line.trim_end();
        if let Some(head) = line.strip_prefix("# branch.head ") {
            status.branch = head.trim().to_string();
        } else if let Some(up) = line.strip_prefix("# branch.upstream ") {
            status.upstream = up.trim().to_string();
        } else if let Some(ab) = line.strip_prefix("# branch.ab ") {
            let mut parts = ab.split_whitespace();
            if let Some(a) = parts.next() {
                status.ahead = a.trim_start_matches('+').parse().unwrap_or(0);
            }
            if let Some(b) = parts.next() {
                status.behind = b.trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if !line.is_empty() && !line.starts_with('#') {
            status.dirty += 1;
            if status.files.len() < FILES_SHOWN {
                let path = extract_porcelain_path(line);
                if !path.is_empty() {
                    status.files.push(path);
                }
            }
        }
    }

    status
}

/// Computes the square label for the strip.
///
/// Rules from §2:
/// - Reads, for example, `main ↑1 ↓2 · 3 changed`.
/// - A count that is 0 is left out, so a clean, level branch reads `main`.
/// - A CI state from §4 adds one mark at the end: `✓` passed, `✗` failed, `●` running.
/// - Narrow, it drops the word "changed" and keeps the numbers (`main ↑1 ↓2 · 3`).
pub fn git_label(status: &GitStatus, ci: Option<&CiRun>, narrow: bool) -> String {
    let mut label = if status.branch.is_empty() {
        "(detached)".to_string()
    } else {
        status.branch.clone()
    };

    let mut arrows = Vec::new();
    if status.ahead > 0 {
        arrows.push(format!("\u{2191}{}", status.ahead));
    }
    if status.behind > 0 {
        arrows.push(format!("\u{2193}{}", status.behind));
    }
    if !arrows.is_empty() {
        label.push(' ');
        label.push_str(&arrows.join(" "));
    }

    if status.dirty > 0 {
        label.push_str(" \u{b7} ");
        if narrow {
            label.push_str(&format!("{}", status.dirty));
        } else {
            label.push_str(&format!("{} changed", status.dirty));
        }
    }

    if let Some(ci_run) = ci {
        label.push(' ');
        label.push_str(ci_run.state.mark());
    }

    label
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmdResult {
    pub code: i32,
    pub text: String,
}

/// Executes a subprocess with a timeout and `CREATE_NO_WINDOW` on Windows.
/// Stderr is ordered before stdout to match `plugin.py`, presenting human-facing
/// error sentences first.
pub fn run_cmd(
    cwd: &Path,
    prog: &str,
    args: &[&str],
    timeout: Duration,
    env_vars: &[(&str, &str)],
) -> CmdResult {
    let mut command = std::process::Command::new(prog);
    command.args(args);
    command.current_dir(cwd);
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    for (k, v) in env_vars {
        command.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return CmdResult {
                code: 127,
                text: format!("{prog} is not on PATH"),
            };
        }
        Err(e) => {
            return CmdResult {
                code: 1,
                text: format!("failed to spawn {prog}: {e}"),
            };
        }
    };

    let (tx, rx) = std::sync::mpsc::channel();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();

    let t_stdout = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut out) = stdout.take() {
            use std::io::Read;
            let _ = out.read_to_end(&mut buf);
        }
        buf
    });

    let t_stderr = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut err) = stderr.take() {
            use std::io::Read;
            let _ = err.read_to_end(&mut buf);
        }
        buf
    });

    let t_wait = std::thread::spawn(move || {
        let res = child.wait();
        let _ = tx.send(res);
        child
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(status)) => {
            let out_bytes = t_stdout.join().unwrap_or_default();
            let err_bytes = t_stderr.join().unwrap_or_default();
            let _ = t_wait.join();
            let out_str = String::from_utf8_lossy(&out_bytes);
            let err_str = String::from_utf8_lossy(&err_bytes);
            let code = status.code().unwrap_or(1);
            let text = [err_str.trim(), out_str.trim()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            CmdResult { code, text }
        }
        Ok(Err(e)) => {
            let _ = t_wait.join();
            CmdResult {
                code: 1,
                text: format!("wait failed: {e}"),
            }
        }
        Err(_) => {
            if let Ok(mut child) = t_wait.join() {
                let _ = child.kill();
                let _ = child.wait();
            }
            CmdResult {
                code: 124,
                text: format!(
                    "{prog} {} exceeded {} s in {}",
                    args.first().unwrap_or(&""),
                    timeout.as_secs(),
                    cwd.display()
                ),
            }
        }
    }
}

/// Reads git status for a repository folder using `git status --porcelain=v2 --branch`.
pub fn read_git_status(path: &Path) -> Result<GitStatus, String> {
    if !path.is_dir() {
        return Err(format!("{}: not a directory", path.display()));
    }
    let res = run_cmd(
        path,
        "git",
        &["status", "--porcelain=v2", "--branch"],
        STATUS_TIMEOUT,
        &[("GIT_TERMINAL_PROMPT", "0")],
    );
    if res.code == 127 {
        return Err("git is not on PATH".into());
    }
    if res.code == 124 {
        return Err(res.text);
    }
    if res.code != 0 {
        return Err(res.text);
    }
    Ok(parse_porcelain_v2(&res.text, path.to_path_buf()))
}

// -----------------------------------------------------------------------------
// Cache and Background Polling
// -----------------------------------------------------------------------------

#[derive(Default)]
struct CacheEntry {
    view: Option<GitView>,
    reading: bool,
    read_at: Option<Instant>,
    asked_at: Option<Instant>,
    panel_open: bool,
}

static ENTRIES: Mutex<Option<HashMap<PathBuf, CacheEntry>>> = Mutex::new(None);
static CHANGED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static TEST_VIEW: std::cell::RefCell<Option<GitView>> = const { std::cell::RefCell::new(None) };
}

pub fn set_view_for_test(view: Option<GitView>) {
    TEST_VIEW.with_borrow_mut(|v| *v = view);
}

/// Loads a fresh GitView synchronously for `folder`.
pub fn load_git_view(folder: &Path) -> Option<GitView> {
    match read_git_status(folder) {
        Ok(status) => Some(GitView { status, ci: None }),
        Err(_) => None,
    }
}

const FORGET_AFTER: Duration = Duration::from_secs(60);

fn poll() {
    let due: Vec<(PathBuf, bool)> = {
        let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
        let Some(map) = guard.as_mut() else {
            return;
        };
        map.retain(|_, entry| entry.asked_at.is_some_and(|at| at.elapsed() < FORGET_AFTER));
        let mut to_read = Vec::new();
        for (folder, entry) in map.iter_mut() {
            if entry.reading {
                continue;
            }
            let reread_interval = if entry.panel_open {
                Duration::from_secs(5)
            } else {
                Duration::from_secs(15)
            };
            if entry
                .read_at
                .is_none_or(|at| at.elapsed() >= reread_interval)
            {
                entry.reading = true;
                to_read.push((folder.clone(), entry.panel_open));
            }
        }
        to_read
    };

    for (folder, _panel_open) in due {
        std::thread::spawn(move || {
            let new_view = load_git_view(&folder);
            let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(map) = guard.as_mut() {
                if let Some(entry) = map.get_mut(&folder) {
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    if entry.view != new_view {
                        entry.view = new_view;
                        CHANGED.store(true, Ordering::Release);
                    }
                }
            }
        });
    }
}

pub fn take_changed() -> bool {
    poll();
    CHANGED.swap(false, Ordering::AcqRel)
}

pub fn set_panel_open(folder: &Path, open: bool) {
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(map) = guard.as_mut() {
        if let Some(entry) = map.get_mut(folder) {
            entry.panel_open = open;
        }
    }
}

/// Returns the cached view for a folder, or spawns a background read if missing.
pub fn cached(folder: &Path) -> Option<GitView> {
    #[cfg(test)]
    {
        if let Some(view) = TEST_VIEW.with_borrow(Clone::clone) {
            return Some(view);
        }
    }

    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(folder.to_path_buf()).or_default();
    entry.asked_at = Some(Instant::now());

    if let Some(view) = &entry.view {
        return Some(view.clone());
    }

    if !entry.reading {
        entry.reading = true;
        let folder_clone = folder.to_path_buf();
        std::thread::spawn(move || {
            let view = load_git_view(&folder_clone);
            let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(map) = guard.as_mut() {
                if let Some(entry) = map.get_mut(&folder_clone) {
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    if entry.view != view {
                        entry.view = view;
                        CHANGED.store(true, Ordering::Release);
                    }
                }
            }
        });
    }

    None
}

// -----------------------------------------------------------------------------
// Unit Tests (§5)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_v2_clean_tree() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head main
# branch.upstream origin/main
# branch.ab +0 -0
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.branch, "main");
        assert_eq!(status.upstream, "origin/main");
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.dirty, 0);
        assert!(status.files.is_empty());
    }

    #[test]
    fn porcelain_v2_dirty_files() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head feat/square
# branch.upstream origin/feat/square
# branch.ab +2 -1
1 .M N... 100644 100644 100644 1234567 1234567 src/main.rs
1 M. N... 100644 100644 100644 2345678 2345678 Cargo.toml
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.branch, "feat/square");
        assert_eq!(status.upstream, "origin/feat/square");
        assert_eq!(status.ahead, 2);
        assert_eq!(status.behind, 1);
        assert_eq!(status.dirty, 2);
        assert_eq!(status.files, vec!["src/main.rs", "Cargo.toml"]);
    }

    #[test]
    fn porcelain_v2_rename_shows_new_path() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head main
# branch.upstream origin/main
# branch.ab +0 -0
2 R. N... 100644 100644 100644 1234567 1234567 R100 src/new_name.rs\tsrc/old_name.rs
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.dirty, 1);
        assert_eq!(status.files, vec!["src/new_name.rs"]);
    }

    #[test]
    fn porcelain_v2_untracked_files() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head main
# branch.upstream origin/main
# branch.ab +0 -0
? untracked_file.txt
? another_file.rs
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.dirty, 2);
        assert_eq!(status.files, vec!["untracked_file.txt", "another_file.rs"]);
    }

    #[test]
    fn porcelain_v2_detached_head() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head (detached)
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.branch, "(detached)");
        assert_eq!(status.upstream, "");
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
        assert_eq!(status.dirty, 0);
    }

    #[test]
    fn porcelain_v2_no_upstream() {
        let text = "\
# branch.oid 1234567890abcdef1234567890abcdef12345678
# branch.head local_branch
";
        let status = parse_porcelain_v2(text, PathBuf::from("test-repo"));
        assert_eq!(status.branch, "local_branch");
        assert_eq!(status.upstream, "");
        assert_eq!(status.ahead, 0);
        assert_eq!(status.behind, 0);
    }

    #[test]
    fn git_label_wide_and_narrow_formats() {
        let mut status = GitStatus {
            repo: PathBuf::from("test"),
            branch: "main".into(),
            upstream: "origin/main".into(),
            ahead: 1,
            behind: 2,
            dirty: 3,
            files: vec!["a.txt".into(), "b.txt".into(), "c.txt".into()],
        };

        // Wide format: "main ↑1 ↓2 · 3 changed"
        assert_eq!(
            git_label(&status, None, false),
            "main \u{2191}1 \u{2193}2 \u{b7} 3 changed"
        );
        // Narrow format: "main ↑1 ↓2 · 3"
        assert_eq!(
            git_label(&status, None, true),
            "main \u{2191}1 \u{2193}2 \u{b7} 3"
        );

        // Clean, level branch: "main"
        status.ahead = 0;
        status.behind = 0;
        status.dirty = 0;
        assert_eq!(git_label(&status, None, false), "main");
        assert_eq!(git_label(&status, None, true), "main");

        // Ahead only: "main ↑1"
        status.ahead = 1;
        assert_eq!(git_label(&status, None, false), "main \u{2191}1");

        // Behind only: "main ↓2"
        status.ahead = 0;
        status.behind = 2;
        assert_eq!(git_label(&status, None, false), "main \u{2193}2");

        // Dirty only: "main · 4 changed" / "main · 4"
        status.behind = 0;
        status.dirty = 4;
        assert_eq!(git_label(&status, None, false), "main \u{b7} 4 changed");
        assert_eq!(git_label(&status, None, true), "main \u{b7} 4");

        // CI mark appended
        let ci_passed = CiRun {
            workflow_name: "CI".into(),
            state: CiState::Passed,
            conclusion_word: "passed".into(),
            created_at_secs: 100,
            url: "https://github.com".into(),
        };
        assert_eq!(
            git_label(&status, Some(&ci_passed), false),
            "main \u{b7} 4 changed \u{2713}"
        );

        let ci_running = CiRun {
            workflow_name: "CI".into(),
            state: CiState::Running,
            conclusion_word: "running".into(),
            created_at_secs: 100,
            url: "https://github.com".into(),
        };
        assert_eq!(
            git_label(&status, Some(&ci_running), true),
            "main \u{b7} 4 \u{25cf}"
        );

        let ci_failed = CiRun {
            workflow_name: "CI".into(),
            state: CiState::Failed,
            conclusion_word: "failure".into(),
            created_at_secs: 100,
            url: "https://github.com".into(),
        };
        assert_eq!(
            git_label(&status, Some(&ci_failed), false),
            "main \u{b7} 4 changed \u{2717}"
        );
    }
}
