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
/// Timeout for network actions (Pull, Push, Fetch).
pub const NET_TIMEOUT: Duration = Duration::from_secs(60);

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

    // The child stays on this thread and is polled, so the timeout can kill it. Review of plan 16 (2026-10-19): the
    // build moved the child into a thread blocked in `wait()` and joined that thread at the timeout, which waited for
    // the hung child instead of killing it, so a push stuck on a credential helper never came back.
    let started = Instant::now();
    let waited = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() >= timeout => break Err(None),
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(e) => break Err(Some(e)),
        }
    };

    match waited {
        Ok(status) => {
            let out_bytes = t_stdout.join().unwrap_or_default();
            let err_bytes = t_stderr.join().unwrap_or_default();
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
        Err(Some(e)) => CmdResult {
            code: 1,
            text: format!("wait failed: {e}"),
        },
        Err(None) => {
            // The reader threads are left to end when the pipes close; joining them could wait on a grandchild
            // (git-remote-https, a credential helper) that still holds a pipe.
            let _ = child.kill();
            let _ = child.wait();
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
// Step 2: The Actions (Pull, Push, Fetch)
// -----------------------------------------------------------------------------

/// What Pull, Push and Fetch share before acting:
/// 1. Re-read status from the repository.
/// 2. Hold the caller to the branch it displayed (`expect_branch`).
/// 3. Verify that the branch has an upstream remote configured.
/// 4. Resolve the remote and upstream branch from `# branch.upstream`.
///
/// Nothing acts yet when this returns an Err refusal.
pub fn stage_action(
    path: &Path,
    expect_branch: &str,
    verb: &str,
) -> Result<(GitStatus, String, String), String> {
    let status = read_git_status(path)
        .map_err(|e| format!("git {verb}: refused. Failed to read status: {e}"))?;

    if expect_branch.is_empty() {
        return Err(format!(
            "git {verb}: refused. This command acts on the working tree, so the caller must name the branch it was looking at as expect_branch; the repository is on {}.",
            if status.branch.is_empty() { "(detached)" } else { &status.branch }
        ));
    }

    if expect_branch != status.branch {
        return Err(format!(
            "git {verb}: refused. The panel offered {:?} and {} is on {:?}. Nothing ran.",
            expect_branch,
            path.display(),
            if status.branch.is_empty() {
                "(detached)"
            } else {
                &status.branch
            }
        ));
    }

    if status.upstream.is_empty() {
        return Err(format!(
            "git {verb}: refused. {} has no upstream, so there is no remote to {verb} {}. git branch --set-upstream-to sets one.",
            status.branch,
            if verb == "pull" { "from" } else { "to" }
        ));
    }

    let (remote, upstream_branch) = match status.upstream.split_once('/') {
        Some((r, b)) if !r.is_empty() && !b.is_empty() => (r.to_string(), b.to_string()),
        _ => {
            return Err(format!(
                "git {verb}: refused. The upstream {:?} does not read as <remote>/<branch>, and this command will not guess one.",
                status.upstream
            ));
        }
    };

    if verb == "push" && status.ahead == 0 {
        return Err("git push: refused. Nothing ahead to push.".to_string());
    }

    Ok((status, remote, upstream_branch))
}

/// Executes `git pull --ff-only <remote> <branch>`.
pub fn pull_action(path: &Path, expect_branch: &str) -> Result<String, String> {
    let (_status, remote, branch) = stage_action(path, expect_branch, "pull")?;
    let res = run_cmd(
        path,
        "git",
        &["pull", "--ff-only", &remote, &branch],
        NET_TIMEOUT,
        &[("GIT_TERMINAL_PROMPT", "0")],
    );
    refresh_folder(path);
    if res.code != 0 {
        Err(format!(
            "git pull: {}",
            if res.text.is_empty() {
                "git failed with no message"
            } else {
                &res.text
            }
        ))
    } else {
        Ok(if res.text.is_empty() {
            "git pull: nothing to report".into()
        } else {
            res.text
        })
    }
}

/// Executes `git push <remote> <branch>` (no force, no lease, no delete, no refspec).
pub fn push_action(path: &Path, expect_branch: &str) -> Result<String, String> {
    let (_status, remote, branch) = stage_action(path, expect_branch, "push")?;
    let res = run_cmd(
        path,
        "git",
        &["push", &remote, &branch],
        NET_TIMEOUT,
        &[("GIT_TERMINAL_PROMPT", "0")],
    );
    refresh_folder(path);
    if res.code != 0 {
        Err(format!(
            "git push: {}",
            if res.text.is_empty() {
                "git failed with no message"
            } else {
                &res.text
            }
        ))
    } else {
        Ok(if res.text.is_empty() {
            "git push: nothing to report".into()
        } else {
            res.text
        })
    }
}

/// Executes `git fetch <remote>`.
pub fn fetch_action(path: &Path, expect_branch: &str) -> Result<String, String> {
    let (_status, remote, _branch) = stage_action(path, expect_branch, "fetch")?;
    let res = run_cmd(
        path,
        "git",
        &["fetch", &remote],
        NET_TIMEOUT,
        &[("GIT_TERMINAL_PROMPT", "0")],
    );
    refresh_folder(path);
    if res.code != 0 {
        Err(format!(
            "git fetch: {}",
            if res.text.is_empty() {
                "git failed with no message"
            } else {
                &res.text
            }
        ))
    } else {
        Ok(if res.text.is_empty() {
            "git fetch: nothing to report".into()
        } else {
            res.text
        })
    }
}

/// Refreshes the cached entry for a folder immediately after an action.
pub fn refresh_folder(folder: &Path) {
    let status = read_git_status(folder).ok();
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(folder.to_path_buf()).or_default();
    let ci = entry.view.as_ref().and_then(|v| v.ci.clone());
    entry.view = status.map(|s| GitView { status: s, ci });
    entry.read_at = Some(Instant::now());
    CHANGED.store(true, Ordering::Release);
}

/// What an action's worker thread hands back, per folder, until the panel takes it.
static ACTION_DONE: Mutex<Option<HashMap<PathBuf, Result<String, String>>>> = Mutex::new(None);

/// Runs a Pull, Push or Fetch on its own thread and returns at once. Plan 16 §3: the action runs off the draw path
/// and the button reads `Pulling…` meanwhile. Review of plan 16 (2026-10-19): the build called the action inline
/// from the key and mouse handlers, so the whole window froze for as long as git talked to the remote.
pub fn start_action(
    folder: PathBuf,
    expect_branch: String,
    action: fn(&Path, &str) -> Result<String, String>,
) {
    std::thread::spawn(move || {
        let result = action(&folder, &expect_branch);
        let mut guard = ACTION_DONE.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .get_or_insert_with(HashMap::new)
            .insert(folder, result);
        CHANGED.store(true, Ordering::Release);
    });
}

/// The finished action's result for this folder, once; `None` while it still runs. A map read, no I/O, so the
/// panel may call it while it draws.
pub fn take_action_result(folder: &Path) -> Option<Result<String, String>> {
    let mut guard = ACTION_DONE.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().and_then(|map| map.remove(folder))
}

// -----------------------------------------------------------------------------
// Step 3: The CI Reader
// -----------------------------------------------------------------------------

/// Checks whether a remote URL points to github.com.
pub fn is_github_remote_url(url: &str) -> bool {
    url.contains("github.com")
}

/// Retrieves the configured URL for a named remote.
pub fn get_remote_url(repo: &Path, remote: &str) -> Option<String> {
    let res = run_cmd(
        repo,
        "git",
        &["remote", "get-url", remote],
        STATUS_TIMEOUT,
        &[("GIT_TERMINAL_PROMPT", "0")],
    );
    if res.code == 0 && !res.text.trim().is_empty() {
        Some(res.text.trim().to_string())
    } else {
        None
    }
}

/// Builds the argv for listing workflow runs via `gh run list`.
pub fn gh_run_list_argv(upstream_branch: &str) -> Vec<String> {
    vec![
        "run".into(),
        "list".into(),
        "--branch".into(),
        upstream_branch.into(),
        "--limit".into(),
        "1".into(),
        "--json".into(),
        "status,conclusion,workflowName,createdAt,url".into(),
    ]
}

/// Stock program that opens nothing, preventing sign-in popups (CLAUDE.md rule 15).
pub fn gh_no_browser_program() -> &'static str {
    if cfg!(windows) {
        r"C:\Windows\System32\where.exe"
    } else {
        "/bin/true"
    }
}

/// Environment variables for gh invocations that make browser launch and prompt impossible.
pub fn gh_env() -> Vec<(&'static str, &'static str)> {
    let no_browser = gh_no_browser_program();
    vec![
        ("BROWSER", no_browser),
        ("GH_BROWSER", no_browser),
        ("GH_PROMPT_DISABLED", "1"),
    ]
}

/// Parses the JSON output of `gh run list`.
pub fn parse_ci_json(json_str: &str, _now_secs: u64) -> Option<CiRun> {
    #[derive(Deserialize)]
    struct GhRun {
        status: String,
        conclusion: Option<String>,
        #[serde(rename = "workflowName")]
        workflow_name: String,
        #[serde(rename = "createdAt")]
        created_at: String,
        #[serde(default)]
        url: String,
    }

    let runs: Vec<GhRun> = serde_json::from_str(json_str).ok()?;
    let first = runs.into_iter().next()?;

    let (state, conclusion_word) = if first.status != "completed" {
        (CiState::Running, "running".to_string())
    } else if first.conclusion.as_deref() == Some("success") {
        (CiState::Passed, "passed".to_string())
    } else {
        let word = first.conclusion.unwrap_or_else(|| "failed".to_string());
        (CiState::Failed, word)
    };

    let created_at_secs = crate::nirvana_usage::rfc3339(&first.created_at).unwrap_or(0);

    Some(CiRun {
        workflow_name: first.workflow_name,
        state,
        conclusion_word,
        created_at_secs,
        url: first.url,
    })
}

/// Formats the panel CI row text: `CI  <workflow>  passed  12m ago`.
pub fn ci_row_text(ci: &CiRun, now_secs: u64) -> String {
    let elapsed = now_secs.saturating_sub(ci.created_at_secs);
    let ago = crate::nirvana_usage::short_duration(elapsed);
    format!(
        "CI  {}  {}  {} ago",
        ci.workflow_name, ci.conclusion_word, ago
    )
}

/// Reads the CI status using `gh` if on PATH, remote is github.com, and upstream is set.
pub fn read_ci_status(repo: &Path, status: &GitStatus) -> Option<CiRun> {
    if cfg!(test) {
        // Safety guardrail: unit and integration tests never execute real gh
        return None;
    }
    if status.upstream.is_empty() {
        return None;
    }
    let (remote, upstream_branch) = status.upstream.split_once('/')?;
    let remote_url = get_remote_url(repo, remote)?;
    if !is_github_remote_url(&remote_url) {
        return None;
    }

    let argv_strings = gh_run_list_argv(upstream_branch);
    let argv_refs: Vec<&str> = argv_strings.iter().map(String::as_str).collect();
    let envs = gh_env();

    let res = run_cmd(repo, "gh", &argv_refs, STATUS_TIMEOUT, &envs);
    if res.code != 0 {
        return None;
    }
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    parse_ci_json(&res.text, now_secs)
}

// -----------------------------------------------------------------------------
// Cache and Background Polling
// -----------------------------------------------------------------------------

#[derive(Default)]
struct CacheEntry {
    view: Option<GitView>,
    reading: bool,
    read_at: Option<Instant>,
    ci_read_at: Option<Instant>,
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
    let status = read_git_status(folder).ok()?;
    let ci = read_ci_status(folder, &status);
    Some(GitView { status, ci })
}

const FORGET_AFTER: Duration = Duration::from_secs(60);

fn poll() {
    let due: Vec<(PathBuf, bool, bool)> = {
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
            let status_due = entry
                .read_at
                .is_none_or(|at| at.elapsed() >= reread_interval);
            let ci_due = entry
                .ci_read_at
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(60));
            if status_due || ci_due {
                entry.reading = true;
                to_read.push((folder.clone(), entry.panel_open, ci_due));
            }
        }
        to_read
    };

    for (folder, _panel_open, read_ci) in due {
        std::thread::spawn(move || {
            let status_opt = read_git_status(&folder).ok();
            let ci_opt = if read_ci {
                if let Some(status) = &status_opt {
                    read_ci_status(&folder, status)
                } else {
                    None
                }
            } else {
                let guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
                guard
                    .as_ref()
                    .and_then(|m| m.get(&folder))
                    .and_then(|e| e.view.as_ref())
                    .and_then(|v| v.ci.clone())
            };
            let new_view = status_opt.map(|status| GitView { status, ci: ci_opt });
            let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(map) = guard.as_mut() {
                if let Some(entry) = map.get_mut(&folder) {
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    if read_ci {
                        entry.ci_read_at = Some(Instant::now());
                    }
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
            let status_opt = read_git_status(&folder_clone).ok();
            let ci_opt = if let Some(status) = &status_opt {
                read_ci_status(&folder_clone, status)
            } else {
                None
            };
            let view = status_opt.map(|status| GitView { status, ci: ci_opt });
            let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(map) = guard.as_mut() {
                if let Some(entry) = map.get_mut(&folder_clone) {
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    entry.ci_read_at = Some(Instant::now());
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

    #[test]
    fn expect_branch_refusal() {
        let tmp =
            std::env::temp_dir().join(format!("nirvana-git-test-refuse-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Initialize a real repo
        let _ = std::process::Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&tmp)
            .output();

        // Mismatched branch
        let err = pull_action(&tmp, "other-branch").unwrap_err();
        assert!(err.contains("refused"), "expected refusal: {err}");
        assert!(err.contains("other-branch"));

        // No upstream
        let err2 = pull_action(&tmp, "main").unwrap_err();
        assert!(err2.contains("refused"), "expected refusal: {err2}");
        assert!(err2.contains("has no upstream"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn real_git_push_fetch_pull_and_diverged() {
        let base_tmp = std::env::temp_dir().join(format!(
            "nirvana-git-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&base_tmp);
        std::fs::create_dir_all(&base_tmp).unwrap();

        let bare = base_tmp.join("bare.git");
        let repo1 = base_tmp.join("repo1");
        let repo2 = base_tmp.join("repo2");

        // 1. Bare remote
        let res = std::process::Command::new("git")
            .args(["init", "--bare"])
            .arg(&bare)
            .output()
            .unwrap();
        assert!(res.status.success());

        // 2. Clone repo1
        let res = std::process::Command::new("git")
            .args(["clone"])
            .arg(&bare)
            .arg(&repo1)
            .output()
            .unwrap();
        assert!(res.status.success());

        // Configure user locally in repo1
        let _ = std::process::Command::new("git")
            .args(["config", "user.name", "P16 Tester"])
            .current_dir(&repo1)
            .output();
        let _ = std::process::Command::new("git")
            .args(["config", "user.email", "tester@example.com"])
            .current_dir(&repo1)
            .output();

        // Initial commit on main
        let _ = std::process::Command::new("git")
            .args(["checkout", "-b", "main"])
            .current_dir(&repo1)
            .output();
        std::fs::write(repo1.join("file.txt"), "init\n").unwrap();
        let _ = std::process::Command::new("git")
            .args(["add", "file.txt"])
            .current_dir(&repo1)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&repo1)
            .output();
        let _ = std::process::Command::new("git")
            .args(["push", "-u", "origin", "main"])
            .current_dir(&repo1)
            .output();

        // 3. Clone repo2
        let res = std::process::Command::new("git")
            .args(["clone", "-b", "main"])
            .arg(&bare)
            .arg(&repo2)
            .output()
            .unwrap();
        assert!(res.status.success());
        let _ = std::process::Command::new("git")
            .args(["config", "user.name", "P16 Tester 2"])
            .current_dir(&repo2)
            .output();
        let _ = std::process::Command::new("git")
            .args(["config", "user.email", "tester2@example.com"])
            .current_dir(&repo2)
            .output();

        // Case 1: Push from repo1
        std::fs::write(repo1.join("file.txt"), "change 1\n").unwrap();
        let _ = std::process::Command::new("git")
            .args(["commit", "-am", "change 1"])
            .current_dir(&repo1)
            .output();
        let st = read_git_status(&repo1).unwrap();
        assert_eq!(st.ahead, 1);

        let push_msg = push_action(&repo1, "main").expect("push succeeds");
        assert!(!push_msg.is_empty());
        let st_after_push = read_git_status(&repo1).unwrap();
        assert_eq!(st_after_push.ahead, 0);

        // Case 2: repo2 pushes a commit; repo1 fetches then pulls
        let _ = std::process::Command::new("git")
            .args(["pull"])
            .current_dir(&repo2)
            .output();
        std::fs::write(repo2.join("file2.txt"), "repo2 commit\n").unwrap();
        let _ = std::process::Command::new("git")
            .args(["add", "file2.txt"])
            .current_dir(&repo2)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "repo2 commit"])
            .current_dir(&repo2)
            .output();
        let push_res = std::process::Command::new("git")
            .args(["push", "origin", "main"])
            .current_dir(&repo2)
            .output()
            .unwrap();
        assert!(
            push_res.status.success(),
            "repo2 push must succeed: {}",
            String::from_utf8_lossy(&push_res.stderr)
        );

        // Before fetch in repo1, behind is 0
        let st1_pre_fetch = read_git_status(&repo1).unwrap();
        assert_eq!(st1_pre_fetch.behind, 0);

        // Fetch in repo1
        fetch_action(&repo1, "main").expect("fetch succeeds");
        let st1_post_fetch = read_git_status(&repo1).unwrap();
        assert_eq!(st1_post_fetch.behind, 1);

        // Pull in repo1 fast-forwards
        pull_action(&repo1, "main").expect("pull succeeds");
        let st1_post_pull = read_git_status(&repo1).unwrap();
        assert_eq!(st1_post_pull.behind, 0);
        assert!(repo1.join("file2.txt").exists());

        // Case 3: Diverged branch: Pull is refused by git, and local commit is kept
        // repo2 makes a commit and pushes
        std::fs::write(repo2.join("diverge2.txt"), "repo2 diverge\n").unwrap();
        let _ = std::process::Command::new("git")
            .args(["add", "diverge2.txt"])
            .current_dir(&repo2)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "repo2 diverge"])
            .current_dir(&repo2)
            .output();
        let _ = std::process::Command::new("git")
            .args(["push", "origin", "main"])
            .current_dir(&repo2)
            .output();

        // repo1 makes a different local commit
        std::fs::write(repo1.join("diverge1.txt"), "repo1 diverge\n").unwrap();
        let _ = std::process::Command::new("git")
            .args(["add", "diverge1.txt"])
            .current_dir(&repo1)
            .output();
        let _ = std::process::Command::new("git")
            .args(["commit", "-m", "local commit Y"])
            .current_dir(&repo1)
            .output();

        // Fetch in repo1 shows 1 ahead and 1 behind
        fetch_action(&repo1, "main").expect("fetch succeeds");
        let st_diverged = read_git_status(&repo1).unwrap();
        assert_eq!(st_diverged.ahead, 1);
        assert_eq!(st_diverged.behind, 1);

        // Pull in repo1 is refused by git (--ff-only)
        let pull_err =
            pull_action(&repo1, "main").expect_err("ff-only pull must fail on divergence");
        assert!(pull_err.contains("git pull:"));

        // Verify local commit is kept
        let log = std::process::Command::new("git")
            .args(["log", "-1", "--format=%s"])
            .current_dir(&repo1)
            .output()
            .unwrap();
        let commit_subject = String::from_utf8_lossy(&log.stdout);
        assert_eq!(commit_subject.trim(), "local commit Y");
        assert!(repo1.join("diverge1.txt").exists());

        let _ = std::fs::remove_dir_all(&base_tmp);
    }

    #[test]
    fn ci_parse_cases() {
        let now = 1_700_000_000;

        // 1. Running (status != "completed")
        let running_json = r#"[
            {
                "status": "in_progress",
                "conclusion": null,
                "workflowName": "Build & Test",
                "createdAt": "2025-10-09T08:00:00Z",
                "url": "https://github.com/org/repo/actions/runs/1"
            }
        ]"#;
        let ci = parse_ci_json(running_json, now).expect("parsed running");
        assert_eq!(ci.state, CiState::Running);
        assert_eq!(ci.conclusion_word, "running");
        assert_eq!(ci.workflow_name, "Build & Test");

        // 2. Passed (status == "completed", conclusion == "success")
        let passed_json = r#"[
            {
                "status": "completed",
                "conclusion": "success",
                "workflowName": "CI",
                "createdAt": "2025-10-09T08:00:00Z",
                "url": "https://github.com/org/repo/actions/runs/2"
            }
        ]"#;
        let ci = parse_ci_json(passed_json, now).expect("parsed passed");
        assert_eq!(ci.state, CiState::Passed);
        assert_eq!(ci.conclusion_word, "passed");

        // 3. Failed
        let failed_json = r#"[
            {
                "status": "completed",
                "conclusion": "failure",
                "workflowName": "Lint",
                "createdAt": "2025-10-09T08:00:00Z",
                "url": "https://github.com/org/repo/actions/runs/3"
            }
        ]"#;
        let ci = parse_ci_json(failed_json, now).expect("parsed failed");
        assert_eq!(ci.state, CiState::Failed);
        assert_eq!(ci.conclusion_word, "failure");

        // 4. Empty list
        assert!(parse_ci_json("[]", now).is_none());

        // 5. Invalid JSON
        assert!(parse_ci_json("not valid json", now).is_none());
    }

    #[test]
    fn ci_probe_rules() {
        // gh argv
        let argv = gh_run_list_argv("main");
        assert_eq!(
            argv,
            vec![
                "run",
                "list",
                "--branch",
                "main",
                "--limit",
                "1",
                "--json",
                "status,conclusion,workflowName,createdAt,url"
            ]
        );

        // gh env prohibits browser launches and disables prompts
        let envs = gh_env();
        let env_map: HashMap<_, _> = envs.into_iter().collect();
        assert_eq!(env_map.get("GH_PROMPT_DISABLED"), Some(&"1"));
        assert!(env_map.contains_key("BROWSER"));
        assert!(env_map.contains_key("GH_BROWSER"));

        // GitHub remote check
        assert!(is_github_remote_url(
            "https://github.com/herdrdev/herdr.git"
        ));
        assert!(is_github_remote_url("git@github.com:herdrdev/herdr.git"));
        assert!(!is_github_remote_url("https://gitlab.com/org/repo.git"));
        assert!(!is_github_remote_url("file:///tmp/repo.git"));
    }

    #[test]
    fn ci_row_text_formatting() {
        let ci = CiRun {
            workflow_name: "CI".into(),
            state: CiState::Passed,
            conclusion_word: "passed".into(),
            created_at_secs: 1_000,
            url: "https://github.com/org/repo/actions/runs/1".into(),
        };
        // 1000 + 12 * 60 = 1720
        let row = ci_row_text(&ci, 1_720);
        assert_eq!(row, "CI  CI  passed  12m ago");
    }

    // Review of plan 16: a child past its timeout is killed and `run_cmd` comes back, rather than waiting it out.
    #[test]
    fn run_cmd_kills_a_child_past_its_timeout() {
        let dir = std::env::temp_dir();
        let started = Instant::now();
        let res = if cfg!(windows) {
            run_cmd(
                &dir,
                "ping",
                &["-n", "30", "127.0.0.1"],
                Duration::from_millis(500),
                &[],
            )
        } else {
            run_cmd(&dir, "sleep", &["30"], Duration::from_millis(500), &[])
        };
        assert_eq!(res.code, 124, "timed out: {}", res.text);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "run_cmd waited {:?} for a child it should have killed at 500 ms",
            started.elapsed()
        );
    }

    fn slow_action(_folder: &Path, branch: &str) -> Result<String, String> {
        std::thread::sleep(Duration::from_millis(400));
        Ok(format!("done on {branch}"))
    }

    // Review of plan 16: an action returns at once and hands its result back later, once.
    #[test]
    fn start_action_returns_before_the_action_ends() {
        let folder =
            std::env::temp_dir().join(format!("nirvana-git-action-{}", std::process::id()));
        let started = Instant::now();
        start_action(folder.clone(), "main".into(), slow_action);
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "start_action blocked for {:?}",
            started.elapsed()
        );
        assert_eq!(
            take_action_result(&folder),
            None,
            "no result before the action ends"
        );
        let mut got = None;
        while started.elapsed() < Duration::from_secs(5) && got.is_none() {
            got = take_action_result(&folder);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(got, Some(Ok("done on main".to_string())));
        assert_eq!(take_action_result(&folder), None, "a result is taken once");
    }
}
