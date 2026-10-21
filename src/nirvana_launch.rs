//! Nirvana fork, plan 12 O4a: Nirvana Claude's launch, the sidebar's "new" menu, and the window the Explorer verb
//! "Open Nirvana" opens.
//!
//! The owner, 2026-10-17 (plan 12 L5): "Claude will be the orchistrator model alwys when you right click on a folder
//! or start a Nirvana herder session you will create a Nirvana version of Claude with all the tool / skills ready to
//! work task in the manner we are describing." And L6: "The reason we are building this as a seperate version which
//! tbh is proably just Claude Code with these Orchistrator level skills and memories and hooks is to differentiate
//! between the two." And L7: "Sometimes I want a Orchistrator style model that tries to utilize both Claude and
//! ANtigravity and sometimes I just want Claude Code or agy traditional to tackle the project."
//!
//! So Nirvana Claude is Claude Code, unpatched, with a plugin and a launch argv (plan 12 §6):
//!
//! ```text
//! claude --plugin-dir <home>/nirvana/claude --agent nirvana --model claude-opus-5-5 --effort medium
//!        --append-system-prompt-file <home>/nirvana/claude/ROLE.md --name "Nirvana" --settings <home>/nirvana/claude/settings.json
//! ```
//!
//! with `NIRVANA_ORCHESTRATOR=1` in its environment. O4a's `agents/nirvana.md` carried the role as its prompt, and its
//! `/context` showed the cost: an agent's prompt REPLACES Claude Code's own system prompt (the sub-agents page:
//! "replaces the default Claude Code system prompt entirely, the same way `--system-prompt` does"), 1.5k tokens
//! against plain Claude Code's 2.5k. The owner's bar is parity with Claude Code (memory
//! nirvana-parity-with-claude-code), so O4b empties the agent file's body ("an agent with an empty prompt and no
//! memory leaves the session's system prompt unchanged") and appends the role from `ROLE.md`. `/context` on
//! 2026-10-18: the empty agent alone 2.5k, with the role 4k. `--agent nirvana` stays because it is what tells Nirvana
//! Claude from plain Claude Code in a pane's argv (O3's wake guard, `nirvana_run::pane_program`). The model and
//! effort are R1's, Opus 5.5 at medium, in the agent file and as flags. Each flag was checked against `claude
//! --help` of Claude Code 2.1.302; `--append-system-prompt-file` is not in its list but is named under `--bare`, a
//! missing file exits 1, and the live `/context` took it.
//!
//! Plain Claude Code and agy carry none of this: the "new" menu's other two entries type `claude` and `agy` alone,
//! which is what a herdr agent start of those kinds types today.
//!
//! Plan 13 P1 (the smart Preload, nirvana/plans/2026-10-18-13-smart-preload.md §2, §6.1): `--settings` names the
//! folder's generated file, `<home>/nirvana/preload/<folder_hash>.settings.json` (the plugin's settings plus the
//! folder's preset), when `nirvana_preset` wrote one, else the plugin's own. Nothing else reads that value: O3's wake
//! guard (`nirvana_run::pane_program`) reads `--agent` and `--plugin-dir` only, and Restart Claude keeps every flag
//! as it was, so a restart reads the regenerated file. And Nirvana Claude resumes its last session in the folder
//! (`--resume <id>`) when the session record P3's hooks write is under `resume_within_hours` old and its transcript
//! is still there; the menu's "Fresh Nirvana Claude" never resumes.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Set for Nirvana Claude only. The plugin's hooks (O4b) read it; plain Claude never has it.
pub const ORCHESTRATOR_ENV: &str = "NIRVANA_ORCHESTRATOR";
/// Set by `herdr nirvana window` for the client it supervises: this Terminal window is already Nirvana's, with this
/// folder's button and badge, so the client's own menu does not claim it again.
pub const WINDOW_FOLDER_ENV: &str = "NIRVANA_WINDOW_FOLDER";
/// The herdr session the verb opens, as Nirvana-C's verb did, so the fork never meets a server on the default one.
pub const SESSION: &str = "nirvana";
/// `command.invoke` id from the "new" menu: a new space with this kind's agent; the kind's key after it.
pub const NEW_COMMAND: &str = "nirvana.new:";
/// `command.invoke` id from the verb's window: focus Nirvana Claude on a folder, or open one; `fresh:` when this
/// window's own click started the server, then the folder.
pub const OPEN_COMMAND: &str = "nirvana.open:";
pub const OPEN_FRESH: &str = "fresh:";
pub const OPEN_NEW: &str = "new:";
/// The fork's own toast app id, registered by `install.py` and by the toast code. One id for every toast, so the
/// header always reads "Nirvana" with the mark.
pub const TOAST_APP_ID: &str = "Nirvana.Herdr";
/// The fork's one taskbar app id (plan 21 §5: "One identity for the window: app id Nirvana.Herdr.Window").
pub const WINDOW_APP_ID: &str = "Nirvana.Herdr.Window";
/// The taskbar id's prefix; each folder gets its own (P7: "Two windows with one id combine into one button showing
/// only the last badge").
pub const WINDOW_APP_ID_PREFIX: &str = "Nirvana.Herdr.";
/// What the taskbar button and the toast call the app.
pub const DISPLAY_NAME: &str = "Nirvana";

/// The agents the sidebar's "new" menu offers (L7), and plan 13 §6.1's "Fresh Nirvana Claude": Nirvana Claude
/// that never resumes the folder's last session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewKind {
    Nirvana,
    NirvanaFresh,
    ClaudeCode,
    Antigravity,
}

impl NewKind {
    /// O4a's three, above the menu's Shell.
    pub const MENU: [NewKind; 3] = [NewKind::Nirvana, NewKind::ClaudeCode, NewKind::Antigravity];
    /// Plan 13: below Shell, so O4a's entries and Shell keep their places (a keybindings test outside this package
    /// clicks Shell by its index).
    pub const AFTER_SHELL: [NewKind; 1] = [NewKind::NirvanaFresh];

    pub fn label(self) -> &'static str {
        match self {
            Self::Nirvana => "Nirvana",
            Self::NirvanaFresh => "Fresh Nirvana Claude",
            Self::ClaudeCode => "Claude Code",
            Self::Antigravity => "Antigravity",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Nirvana => "nirvana",
            Self::NirvanaFresh => "nirvana-fresh",
            Self::ClaudeCode => "claude",
            Self::Antigravity => "agy",
        }
    }

    /// Nirvana Claude, resumed or fresh.
    pub fn is_nirvana(self) -> bool {
        matches!(self, Self::Nirvana | Self::NirvanaFresh)
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::MENU
            .into_iter()
            .chain(Self::AFTER_SHELL)
            .find(|kind| kind.key() == key)
    }

    /// The herdr agent kind label `agent start` takes; the program it types is herdr's own for that kind.
    pub fn agent_label(self) -> &'static str {
        match self {
            Self::Nirvana | Self::NirvanaFresh | Self::ClaudeCode => "claude",
            Self::Antigravity => "agy",
        }
    }

    /// The herdr agent name's stem; `nirvana` also marks the pane as Nirvana Claude's for the verb's second click,
    /// fresh or not.
    pub fn name_stem(self) -> &'static str {
        match self {
            Self::NirvanaFresh => Self::Nirvana.key(),
            _ => self.key(),
        }
    }

    /// What is typed after the program: Nirvana's flags, nothing for the other two.
    pub fn args(self, home: &Path) -> Vec<String> {
        match self {
            Self::Nirvana => nirvana_claude_args(home),
            Self::ClaudeCode | Self::Antigravity => Vec::new(),
            Self::NirvanaFresh => Self::Nirvana.args(home),
        }
    }

    /// The pane's extra environment.
    pub fn env(self) -> Vec<(String, String)> {
        match self {
            Self::Nirvana => vec![(ORCHESTRATOR_ENV.to_owned(), "1".to_owned())],
            Self::ClaudeCode | Self::Antigravity => Vec::new(),
            Self::NirvanaFresh => Self::Nirvana.env(),
        }
    }

    pub fn command_id(self) -> String {
        format!("{NEW_COMMAND}{}", self.key())
    }
}

/// `<home>/nirvana/claude`, where `install.py` copies the plugin.
pub fn plugin_dir(home: &Path) -> PathBuf {
    home.join("nirvana").join("claude")
}

/// The orchestrator's role, appended to Claude Code's own system prompt.
pub const ROLE_FILE: &str = "ROLE.md";
/// §12 R1: "The orchestrator is Opus 5.5 at medium."
pub const MODEL: &str = "claude-opus-5-5";
pub const EFFORT: &str = "medium";

/// The orchestrator's seat at launch (plan 20 §3.2): from the seat rule, or today's seat (opus medium)
/// when the table or the readings fail to load.
pub fn orchestrator_launch_seat(home: &Path) -> (String, String) {
    let nirvana_dir = home.join(crate::nirvana_seat::DIR);
    let seats_path = nirvana_dir.join(crate::nirvana_seat::SEATS_FILE);
    let table = match std::fs::read_to_string(&seats_path)
        .ok()
        .and_then(|text| crate::nirvana_seat::parse(&text).ok())
    {
        Some(t) => t,
        None => return (MODEL.to_owned(), EFFORT.to_owned()),
    };
    let now = crate::nirvana_usage::unix(std::time::SystemTime::now());
    let world = crate::nirvana_seat::cached_world(&nirvana_dir, now);
    let seat = crate::nirvana_seat::orchestrator_seat(&table, &world);
    (seat.model, seat.effort.word().to_owned())
}

/// The arguments after `claude` (plan 12 §6, O4b), in its order.
pub fn nirvana_claude_args(home: &Path) -> Vec<String> {
    let plugin = plugin_dir(home);
    let (model, effort) = orchestrator_launch_seat(home);
    vec![
        "--plugin-dir".into(),
        plugin.display().to_string(),
        "--agent".into(),
        "nirvana".into(),
        "--model".into(),
        model,
        "--effort".into(),
        effort,
        "--append-system-prompt-file".into(),
        plugin.join(ROLE_FILE).display().to_string(),
        "--name".into(),
        DISPLAY_NAME.into(),
        "--settings".into(),
        plugin.join("settings.json").display().to_string(),
        // The owner's own Claude runs as `claude --rc --dangerously-skip-permissions --chrome`. The owner,
        // 2026-10-19, choosing Nirvana Claude over a usage line for plain Claude: "Use Nirvana Claude instead"
        // ("Add --rc and --chrome to Nirvana Claude's launch so it can replace your plain Claude").
        "--rc".into(),
        "--chrome".into(),
    ]
}

/// Plan 13: `nirvana_claude_args` with the folder's generated settings file in place of the plugin's, when one was
/// written, and `--resume <id>` last when `resume` names a session.
pub fn launch_args(home: &Path, folder: &Path, resume: Option<&str>) -> Vec<String> {
    let mut args = nirvana_claude_args(home);
    let generated =
        crate::nirvana_preset::settings_path(&home.join(crate::nirvana_seat::DIR), folder);
    if generated.is_file() {
        if let Some(at) = args.iter().position(|arg| arg == "--settings") {
            args[at + 1] = generated.display().to_string();
        }
    }
    if let Some(id) = resume {
        args.extend(["--resume".to_owned(), id.to_owned()]);
    }
    args
}

/// The record P3's SessionStart and Stop hooks write (plan 13 §8): `<home>/nirvana/sessions/<folder_hash>.json`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    /// RFC 3339 UTC: the session's start, or its last `Stop`.
    pub at: String,
    /// The transcript's absolute path.
    pub transcript: String,
}

pub fn session_record_path(home: &Path, folder: &Path) -> PathBuf {
    home.join(crate::nirvana_seat::DIR)
        .join("sessions")
        .join(format!("{}.json", folder_hash(folder)))
}

/// Encodes a project folder path the way Claude Code encodes it for ~/.claude/projects/<enc>.
/// Each character that is not an ASCII letter or digit becomes `-`.
pub fn encode_project_folder(folder: &Path) -> String {
    let s = folder.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_end_matches('/');
    let s = if trimmed.is_empty() { &s[..] } else { trimmed };
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// A session id as Claude Code writes one: it matches ^[A-Za-z0-9][A-Za-z0-9-]{0,63}$.
fn plain_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// §6.1: the session to resume, when the folder's record is under `within_hours` old at `now` (Unix seconds) and
/// its transcript is still there; `None` otherwise, and the launch starts fresh.
pub fn resume_session_at(
    home: &Path,
    folder: &Path,
    now: u64,
    within_hours: f64,
) -> Option<String> {
    let text = std::fs::read_to_string(session_record_path(home, folder)).ok()?;
    let record: SessionRecord = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let at = crate::nirvana_usage::rfc3339(&record.at)?;
    if at > now + 300 {
        return None;
    }
    let age = now.saturating_sub(at);
    let transcript = Path::new(&record.transcript);
    let expected_folder_enc = encode_project_folder(folder);
    let stem_matches = transcript.file_stem().and_then(|s| s.to_str()) == Some(&record.session_id);
    let parent_matches = transcript
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .is_some_and(|parent| {
            if cfg!(windows) {
                parent.eq_ignore_ascii_case(&expected_folder_enc)
            } else {
                parent == expected_folder_enc
            }
        });
    if !plain_session_id(&record.session_id)
        || (age as f64) >= within_hours * 3600.0
        || !Path::new(&record.transcript).is_file()
        || !stem_matches
        || !parent_matches
    {
        return None;
    }
    Some(record.session_id)
}

/// `resume_session_at` now, with this machine's `resume_within_hours`.
pub fn resume_session(home: &Path, folder: &Path) -> Option<String> {
    let hours = crate::nirvana_preset::resume_within_hours(&home.join(crate::nirvana_seat::DIR));
    resume_session_at(home, folder, crate::nirvana_run::now(), hours)
}

/// Whether an agent name is one `NewKind::Nirvana` gives: `nirvana` or `nirvana-<n>`.
pub fn is_nirvana_agent_name(name: &str) -> bool {
    name == NewKind::Nirvana.name_stem()
        || name
            .strip_prefix("nirvana-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The first of `stem`, `stem-2`, `stem-3`, ... that `taken` does not hold.
pub fn free_agent_name(stem: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(stem) {
        return stem.to_owned();
    }
    (2u32..)
        .map(|n| format!("{stem}-{n}"))
        .find(|name| !taken(name))
        .expect("an unbounded range always has a free name")
}

/// The folder as the taskbar id hashes it: separators as `/`, no trailing separator, lower case. Windows paths are
/// case-insensitive, and Explorer hands `%V` in whatever case the folder was typed, so `C:\Work` and `c:/work/` must
/// be one button.
pub fn folder_key(folder: &Path) -> String {
    let text = folder.display().to_string().replace('\\', "/");
    let trimmed = text.trim_end_matches('/');
    let trimmed = if trimmed.is_empty() { "/" } else { trimmed };
    trimmed.to_lowercase()
}

/// 16 hex digits of the folder key's sha256: short enough for an AppUserModelID (at most 128 characters, no
/// spaces), long enough that two of the owner's folders never meet.
pub fn folder_hash(folder: &Path) -> String {
    let digest = Sha256::digest(folder_key(folder).as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn window_app_id(folder: &Path) -> String {
    format!("{WINDOW_APP_ID_PREFIX}{}", folder_hash(folder))
}

/// The folder's name as the toast shows it: its last part, or the whole path for a drive root.
pub fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| folder.display().to_string())
}

/// The badge's letter: the first letter or digit of the folder's name, upper-cased when that is one character
/// (`ß` stays `ß`, not `SS`). `.config` gives `C`; a name with neither, or none, gives `N` for Nirvana.
pub fn folder_letter(folder: &Path) -> char {
    letter_of(&folder_name(folder))
}

pub fn letter_of(name: &str) -> char {
    let Some(first) = name.chars().find(|c| c.is_alphanumeric()) else {
        return 'N';
    };
    let mut upper = first.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(single), None) => single,
        _ => first,
    }
}

/// "Nirvana · <folder>", the toast's title (plan 12 §8).
pub fn toast_title(folder: &Path) -> String {
    format!("{DISPLAY_NAME} \u{b7} {}", folder_name(folder))
}

/// The folder Explorer hands the verb. A drive root arrives as `C:"` because `"%V"` with `%V = C:\` puts a
/// backslash before the closing quote, which the argument rules read as an escaped quote; the quote is dropped and
/// the root's backslash put back.
pub fn verb_folder(arg: &str) -> PathBuf {
    let trimmed = arg.trim().trim_end_matches('"').trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        return PathBuf::from(format!("{trimmed}\\"));
    }
    PathBuf::from(trimmed)
}

/// The `command.invoke` id the verb's window sends.
pub fn open_command_id(folder: &Path, fresh: bool, new_space: bool) -> String {
    format!(
        "{OPEN_COMMAND}{}{}{}",
        if new_space { OPEN_NEW } else { "" },
        if fresh { OPEN_FRESH } else { "" },
        folder.display()
    )
}

/// The folder, the fresh flag, and the new-space flag back from an `OPEN_COMMAND` id's tail.
pub fn parse_open_command(mut rest: &str) -> (PathBuf, bool, bool) {
    let mut fresh = false;
    let mut new_space = false;
    loop {
        if let Some(tail) = rest.strip_prefix(OPEN_FRESH) {
            fresh = true;
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix(OPEN_NEW) {
            new_space = true;
            rest = tail;
        } else {
            break;
        }
    }
    (PathBuf::from(rest), fresh, new_space)
}

/// A mark written by a live Nirvana window at `<home>/nirvana/window.json` (plan 21 §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WindowMark {
    pub pid: u32,
    pub hwnd: usize,
    pub at: u64,
}

// Plan 21 §3.1: window mark and liveness facts, wired into window and open in step 2.
#[allow(dead_code)]
pub const WINDOW_MARK_FILE: &str = "window.json";

/// `<home>/nirvana/window.json`
#[allow(dead_code)]
pub fn window_mark_path(home: &Path) -> PathBuf {
    home.join("nirvana").join(WINDOW_MARK_FILE)
}

/// Writes `mark` to `<home>/nirvana/window.json` with write_bytes semantics (single file write).
#[allow(dead_code)]
pub fn write_window_mark(home: &Path, mark: &WindowMark) -> std::io::Result<()> {
    let path = window_mark_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(mark)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::write(path, data)
}

/// Reads the mark at `<home>/nirvana/window.json`; empty or torn files read as no mark.
#[allow(dead_code)]
pub fn read_window_mark(home: &Path) -> Option<WindowMark> {
    let path = window_mark_path(home);
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// Removes the mark at `<home>/nirvana/window.json` only when its pid matches `pid`.
#[allow(dead_code)]
pub fn remove_window_mark(home: &Path, pid: u32) -> std::io::Result<bool> {
    let path = window_mark_path(home);
    let Some(mark) = read_window_mark(home) else {
        return Ok(false);
    };
    if mark.pid == pid {
        std::fs::remove_file(path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Pure liveness check over the three facts (plan 21 §3.1):
/// process at pid runs and is herdr.exe, IsWindow(hwnd) is true, server is listening.
#[allow(dead_code)]
pub fn is_mark_live(
    process_runs_herdr: bool,
    window_is_window: bool,
    server_listening: bool,
) -> bool {
    process_runs_herdr && window_is_window && server_listening
}

/// Pure helper parameterized with predicates for unit testing without a real window.
#[allow(dead_code)]
pub fn check_mark_live_with<F1, F2, F3>(
    mark: &WindowMark,
    mut process_runs_herdr: F1,
    mut window_is_window: F2,
    mut server_listening: F3,
) -> bool
where
    F1: FnMut(u32) -> bool,
    F2: FnMut(usize) -> bool,
    F3: FnMut() -> bool,
{
    is_mark_live(
        process_runs_herdr(mark.pid),
        window_is_window(mark.hwnd),
        server_listening(),
    )
}

#[cfg(windows)]
#[allow(dead_code)]
pub fn is_herdr_process(pid: u32) -> bool {
    if pid <= 4 {
        return false;
    }
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    const STILL_ACTIVE: u32 = 259;
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut exit_code: u32 = 0;
    let ok = unsafe { GetExitCodeProcess(handle, &mut exit_code) } != 0;
    if !ok || exit_code != STILL_ACTIVE {
        unsafe { CloseHandle(handle) };
        return false;
    }
    let mut path = vec![0_u16; 1024];
    let mut len = path.len() as u32;
    let success =
        unsafe { QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut len) } != 0;
    unsafe { CloseHandle(handle) };
    if !success || len == 0 {
        return false;
    }
    let full_path = String::from_utf16_lossy(&path[..len as usize]);
    let exe_name = Path::new(&full_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    exe_name.eq_ignore_ascii_case("herdr.exe")
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn is_herdr_process(_pid: u32) -> bool {
    false
}

#[cfg(windows)]
#[allow(dead_code)]
pub fn is_valid_window(hwnd: usize) -> bool {
    if hwnd == 0 {
        return false;
    }
    use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
    unsafe { IsWindow(hwnd as _) != 0 }
}

#[cfg(not(windows))]
#[allow(dead_code)]
pub fn is_valid_window(_hwnd: usize) -> bool {
    false
}

#[allow(dead_code)]
pub fn is_server_listening_now() -> bool {
    crate::server::autodetect::is_server_listening().unwrap_or(false)
}

/// Checks if a mark is live against the real system facts.
#[allow(dead_code)]
pub fn is_live_window_mark(mark: &WindowMark) -> bool {
    check_mark_live_with(
        mark,
        is_herdr_process,
        is_valid_window,
        is_server_listening_now,
    )
}

/// The route taken by `herdr nirvana open [<folder>]` (plan 21 §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenRoute {
    FocusOrOpenFolder { folder: PathBuf, hwnd: usize },
    FlashWindow { hwnd: usize },
    StartWindowFolder { folder: PathBuf },
    StartWindowNoFolder,
}

/// Pure routing logic over live mark and folder inputs (plan 21 §3.2).
pub fn route_open(live_mark: Option<&WindowMark>, folder: Option<&Path>) -> OpenRoute {
    match (live_mark, folder) {
        (Some(mark), Some(folder)) => OpenRoute::FocusOrOpenFolder {
            folder: folder.to_path_buf(),
            hwnd: mark.hwnd,
        },
        (Some(mark), None) => OpenRoute::FlashWindow { hwnd: mark.hwnd },
        (None, Some(folder)) => OpenRoute::StartWindowFolder {
            folder: folder.to_path_buf(),
        },
        (None, None) => OpenRoute::StartWindowNoFolder,
    }
}

pub const OPEN_LOG_FILE: &str = "open.log";

/// `<home>/nirvana/open.log`
pub fn open_log_path(home: &Path) -> PathBuf {
    home.join("nirvana").join(OPEN_LOG_FILE)
}

/// Appends a one-line failure description to `<home>/nirvana/open.log` (plan 21 §3.2).
pub fn log_open_error(home: &Path, error: &str) {
    let path = open_log_path(home);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "[{now}] {error}");
    }
}

/// Spawns a new Windows Terminal window running herdr nirvana window (plan 21 §3.2).
pub fn spawn_window(folder: Option<&Path>) -> std::io::Result<()> {
    let herdr = crate::nirvana_taskbar::herdr_exe();
    let mut cmd = std::process::Command::new("wt.exe");
    cmd.args(["-w", "new", "-p", DISPLAY_NAME]);
    if let Some(folder) = folder {
        cmd.args(["-d", &folder.to_string_lossy()]);
        cmd.arg(&herdr);
        cmd.args([
            "--session",
            SESSION,
            "nirvana",
            "window",
            &folder.to_string_lossy(),
        ]);
    } else {
        cmd.arg(&herdr);
        cmd.args(["--session", SESSION, "nirvana", "window"]);
    }
    cmd.spawn()?;
    Ok(())
}

/// `herdr nirvana open [<folder>]` (plan 21 §3.2).
pub fn open(folder: Option<&Path>) -> std::io::Result<i32> {
    let home = match crate::nirvana::home() {
        Some(h) => h,
        None => {
            eprintln!("nirvana: neither NIRVANA_HERDR_HOME nor LOCALAPPDATA is set");
            return Ok(1);
        }
    };
    let mark = read_window_mark(&home);
    let live_mark = mark.as_ref().filter(|m| is_live_window_mark(m));
    let route = route_open(live_mark, folder);

    match route {
        OpenRoute::FocusOrOpenFolder { folder, hwnd } => {
            let res = (|| -> std::io::Result<()> {
                let fresh = !crate::server::autodetect::is_server_listening().unwrap_or(false);
                let preset = crate::nirvana_preset::at_open(&folder);
                invoke_open(&folder, fresh, true)?;
                crate::nirvana_taskbar::flash_window(hwnd)?;
                crate::nirvana_taskbar::toast_open_with(&folder, preset.as_deref());
                Ok(())
            })();
            if let Err(err) = res {
                log_open_error(&home, &format!("failed to open into live window: {err}"));
                if let Err(spawn_err) = spawn_window(Some(&folder)) {
                    log_open_error(&home, &format!("failed to spawn window: {spawn_err}"));
                }
            }
        }
        OpenRoute::FlashWindow { hwnd } => {
            if let Err(err) = crate::nirvana_taskbar::flash_window(hwnd) {
                log_open_error(&home, &format!("failed to flash window: {err}"));
            }
        }
        OpenRoute::StartWindowFolder { folder } => {
            if let Err(err) = spawn_window(Some(&folder)) {
                log_open_error(&home, &format!("failed to spawn window: {err}"));
            }
        }
        OpenRoute::StartWindowNoFolder => {
            if let Err(err) = spawn_window(None) {
                log_open_error(&home, &format!("failed to spawn window: {err}"));
            }
        }
    }
    Ok(0)
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

const USAGE: &str = "usage: herdr --session nirvana nirvana window [<folder>]
  Opens Nirvana Claude on <folder> in this terminal: starts the session's server if it is not running, focuses the
  Nirvana Claude already on the folder or opens a space with one, marks this Windows Terminal window with the
  Nirvana button and the folder's letter, and runs the herdr client until it exits. Explorer's \"Open Nirvana\"
  runs it.
       herdr nirvana open [<folder>]
  Routes to the live Nirvana window if one is open (focusing or opening the space and flashing the window),
  otherwise starts a window.
       herdr nirvana digest|guard-spawn|subagent-stop|stop|statusline
  The Nirvana Claude plugin's hooks and statusline (plan 12 O4b); each reads Claude Code's JSON on stdin.
  Before Claude starts, the window applies the folder's preset (plan 13; `herdr preload why <folder>`).";

/// `herdr nirvana <subcommand>`.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("window") if args.len() == 2 => {
            let code = window(Some(&verb_folder(&args[1])))?;
            std::process::exit(code);
        }
        Some("window") if args.len() == 1 => {
            let code = window(None)?;
            std::process::exit(code);
        }
        Some("open") if args.len() == 2 => {
            let code = open(Some(&verb_folder(&args[1])))?;
            std::process::exit(code);
        }
        Some("open") if args.len() == 1 => {
            let code = open(None)?;
            std::process::exit(code);
        }
        // Plan 12 O4b: the plugin's hooks and statusline.
        Some(name) if args.len() == 1 && crate::nirvana_claude::SUBCOMMANDS.contains(&name) => {
            crate::nirvana_claude::cli(name).expect("a listed subcommand")
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// The verb's window: the server, the space, the button and badge, the toast, then the client until it exits.
fn window(folder: Option<&Path>) -> std::io::Result<i32> {
    if let Some(folder) = folder {
        if !folder.is_dir() {
            eprintln!("nirvana: {} is not a folder", folder.display());
            hold_window();
            return Ok(2);
        }
    }
    let home = crate::nirvana::home();
    let current_pid = std::process::id();

    // Check if another window is already live (plan 21 §3.2)
    if let Some(home) = &home {
        if let Some(mark) = read_window_mark(home) {
            if is_live_window_mark(&mark) && mark.pid != current_pid {
                if let Some(folder) = folder {
                    let folder =
                        std::path::absolute(folder).unwrap_or_else(|_| folder.to_path_buf());
                    let fresh = !crate::server::autodetect::is_server_listening().unwrap_or(false);
                    let preset = crate::nirvana_preset::at_open(&folder);
                    if let Err(err) = invoke_open(&folder, fresh, true) {
                        eprintln!("nirvana: Nirvana Claude did not start: {err}");
                    }
                    let _ = crate::nirvana_taskbar::flash_window(mark.hwnd);
                    crate::nirvana_taskbar::toast_open_with(&folder, preset.as_deref());
                    return Ok(0);
                } else {
                    let _ = crate::nirvana_taskbar::flash_window(mark.hwnd);
                    return Ok(0);
                }
            }
        }
    }

    let folder_abs = folder.map(|f| std::path::absolute(f).unwrap_or_else(|_| f.to_path_buf()));
    let fresh = !crate::server::autodetect::is_server_listening().unwrap_or(false);
    if fresh {
        let start_dir = folder_abs
            .clone()
            .or_else(user_home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        std::env::set_current_dir(&start_dir)?;
        crate::server::autodetect::spawn_server_daemon()?;
        crate::server::autodetect::wait_for_server_socket(
            &crate::server::socket_paths::client_socket_path(),
            std::time::Duration::from_secs(15),
        )?;
    }

    let preset = folder_abs
        .as_ref()
        .and_then(|f| crate::nirvana_preset::at_open(f));
    if let Some(folder) = &folder_abs {
        if let Err(err) = invoke_open(folder, fresh, false) {
            eprintln!("nirvana: Nirvana Claude did not start: {err}");
        }
    }

    let claimed = crate::nirvana_taskbar::claim_window(folder_abs.as_deref());
    if let Err(err) = &claimed {
        eprintln!("nirvana: the taskbar button was not set: {err}");
    }
    if let Some(folder) = &folder_abs {
        crate::nirvana_taskbar::toast_open_with(folder, preset.as_deref());
        std::env::set_var(WINDOW_FOLDER_ENV, folder);
    }

    // Write the mark now that the window is claimed
    if let Some(home) = &home {
        let hwnd = crate::nirvana_taskbar::terminal_window().unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mark = WindowMark {
            pid: current_pid,
            hwnd,
            at: now,
        };
        let _ = write_window_mark(home, &mark);
    }

    let code = crate::nirvana::supervise_code(&[]);

    // On exit: remove the mark only when mark's pid is its own
    if let Some(home) = &home {
        let _ = remove_window_mark(home, current_pid);
    }
    if claimed.is_ok() {
        crate::nirvana_taskbar::release_window();
    }
    code
}

fn invoke_open(folder: &Path, fresh: bool, new_space: bool) -> std::io::Result<()> {
    let request = crate::api::schema::Request {
        id: "nirvana:open".into(),
        method: crate::api::schema::Method::CommandInvoke(
            crate::api::schema::CommandInvokeParams {
                command_id: open_command_id(folder, fresh, new_space),
                workspace_id: None,
                tab_id: None,
                pane_id: None,
                selection: None,
            },
        ),
    };
    let response = crate::api::client::ApiClient::local()
        .request_value(&request)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    match response.get("error") {
        Some(error) => Err(std::io::Error::other(error.to_string())),
        None => Ok(()),
    }
}

/// A window that closes at once never shows why; wait for Enter when there is someone to press it.
fn hold_window() {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        eprintln!("Press Enter to close.");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAGS: [&str; 7] = [
        "--plugin-dir",
        "--model",
        "--effort",
        "--append-system-prompt-file",
        "--agent",
        "--name",
        "--settings",
    ];

    #[test]
    fn nirvana_claude_takes_the_plugin_agent_name_and_settings_from_home() {
        let home = Path::new("C:/Users/o/AppData/Local/Programs/Nirvana-Herdr");
        let plugin = home.join("nirvana").join("claude");
        assert_eq!(
            NewKind::Nirvana.args(home),
            vec![
                "--plugin-dir".to_string(),
                plugin.display().to_string(),
                "--agent".into(),
                "nirvana".into(),
                "--model".into(),
                "claude-opus-5-5".into(),
                "--effort".into(),
                "medium".into(),
                "--append-system-prompt-file".into(),
                plugin.join("ROLE.md").display().to_string(),
                "--name".into(),
                "Nirvana".into(),
                "--settings".into(),
                plugin.join("settings.json").display().to_string(),
                "--rc".into(),
                "--chrome".into(),
            ]
        );
        assert_eq!(
            NewKind::Nirvana.env(),
            vec![("NIRVANA_ORCHESTRATOR".to_string(), "1".to_string())]
        );
        assert_eq!(NewKind::Nirvana.agent_label(), "claude");
        // O4b: the role is appended to Claude Code's own prompt; the agent's own file has no prompt to replace it.
        let agent = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("nirvana/claude/agents/nirvana.md"),
        )
        .expect("the agent file");
        let body = agent.splitn(3, "---").nth(2).expect("front matter");
        assert!(
            body.trim().is_empty(),
            "the agent's prompt replaces Claude Code's: {body:?}"
        );
    }

    #[test]
    fn plain_claude_code_and_agy_carry_no_nirvana_flag_and_no_nirvana_env() {
        // L6: "a test that plain Claude's argv carries no Nirvana flag".
        let home = Path::new("/home/o/.nirvana-herdr");
        for kind in [NewKind::ClaudeCode, NewKind::Antigravity] {
            let args = kind.args(home);
            for flag in FLAGS {
                assert!(!args.iter().any(|a| a == flag), "{kind:?} carries {flag}");
            }
            assert!(
                !args.iter().any(|a| a.contains("nirvana")),
                "{kind:?}: {args:?}"
            );
            assert!(kind.env().is_empty(), "{kind:?} sets an env");
            assert!(!is_nirvana_agent_name(kind.name_stem()));
        }
        assert_eq!(NewKind::ClaudeCode.agent_label(), "claude");
        assert_eq!(NewKind::Antigravity.agent_label(), "agy");
        assert_eq!(
            crate::detect::parse_agent_label(NewKind::Antigravity.agent_label()),
            Some(crate::detect::Agent::Antigravity)
        );
    }

    #[test]
    fn the_menu_offers_nirvana_claude_code_and_antigravity_in_that_order() {
        let labels: Vec<_> = NewKind::MENU.iter().map(|kind| kind.label()).collect();
        assert_eq!(labels, ["Nirvana", "Claude Code", "Antigravity"]);
        assert_eq!(NewKind::AFTER_SHELL, [NewKind::NirvanaFresh]);
        assert_eq!(NewKind::NirvanaFresh.label(), "Fresh Nirvana Claude");
        for kind in NewKind::MENU.into_iter().chain(NewKind::AFTER_SHELL) {
            assert_eq!(NewKind::from_key(kind.key()), Some(kind));
            assert_eq!(kind.command_id(), format!("nirvana.new:{}", kind.key()));
        }
        assert_eq!(NewKind::from_key("codex"), None);
        // Plan 13 §6.1: the fresh one is Nirvana Claude in every way but the resume, so the verb's second click
        // finds it by its name.
        let home = Path::new("/home/o/.nirvana-herdr");
        assert_eq!(
            NewKind::NirvanaFresh.args(home),
            NewKind::Nirvana.args(home)
        );
        assert_eq!(NewKind::NirvanaFresh.env(), NewKind::Nirvana.env());
        assert_eq!(NewKind::NirvanaFresh.name_stem(), "nirvana");
        assert!(NewKind::NirvanaFresh.is_nirvana() && NewKind::Nirvana.is_nirvana());
        assert!(!NewKind::ClaudeCode.is_nirvana() && !NewKind::Antigravity.is_nirvana());
    }

    fn scratch_home(name: &str) -> PathBuf {
        let home =
            std::env::temp_dir().join(format!("nirvana-p13a-launch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("nirvana")).expect("home");
        home
    }

    #[test]
    fn the_launch_names_the_folders_generated_settings_when_one_was_written() {
        let home = scratch_home("settings");
        let folder = Path::new(r"C:\work\print");
        // None written: the plugin's own file, exactly O4b's argv.
        assert_eq!(launch_args(&home, folder, None), nirvana_claude_args(&home));
        let generated = crate::nirvana_preset::settings_path(&home.join("nirvana"), folder);
        std::fs::create_dir_all(generated.parent().unwrap()).unwrap();
        std::fs::write(&generated, "{}").unwrap();
        let args = launch_args(&home, folder, None);
        let at = args.iter().position(|a| a == "--settings").unwrap();
        assert_eq!(args[at + 1], generated.display().to_string());
        assert_eq!(args.iter().filter(|a| *a == "--settings").count(), 1);
        // Everything else is O4b's, so the wake guard still knows it (`--agent`, `--plugin-dir`).
        let mut base = nirvana_claude_args(&home);
        base[at + 1] = generated.display().to_string();
        assert_eq!(args, base);
        // Another folder has its own file, or none.
        assert_eq!(
            launch_args(&home, Path::new(r"C:\work\other"), None),
            nirvana_claude_args(&home)
        );
        let resumed = launch_args(&home, folder, Some("abc-123"));
        assert_eq!(resumed[resumed.len() - 2..], ["--resume", "abc-123"]);
        let _ = std::fs::remove_dir_all(&home);
    }

    fn record(home: &Path, folder: &Path, id: &str, at: &str, transcript: &Path) {
        let path = session_record_path(home, folder);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            serde_json::json!({"session_id": id, "at": at, "transcript": transcript}).to_string(),
        )
        .unwrap();
    }

    #[test]
    fn a_recent_session_with_its_transcript_resumes_and_anything_else_starts_fresh() {
        let home = scratch_home("resume");
        let folder = Path::new(r"C:\work\print");
        let enc = encode_project_folder(folder);
        let transcript_dir = home.join(&enc);
        std::fs::create_dir_all(&transcript_dir).unwrap();
        let session_id = "6f1c2a3b-0000-4000-8000-000000000001";
        let transcript = transcript_dir.join(format!("{session_id}.jsonl"));
        std::fs::write(&transcript, "{}\n").unwrap();
        let at = "2026-10-18T10:00:00Z";
        let then = crate::nirvana_usage::rfc3339(at).unwrap();
        assert_eq!(
            resume_session_at(&home, folder, then, 24.0),
            None,
            "no record"
        );
        record(&home, folder, session_id, at, &transcript);
        assert_eq!(
            resume_session_at(&home, folder, then + 3600, 24.0).as_deref(),
            Some(session_id)
        );
        // The record's folder is the folder's hash: the same folder in another case still resumes.
        assert!(resume_session_at(&home, Path::new("c:/WORK/print/"), then + 60, 24.0).is_some());
        // Too old.
        assert_eq!(
            resume_session_at(&home, folder, then + 24 * 3600, 24.0),
            None
        );
        assert_eq!(resume_session_at(&home, folder, then + 3 * 3600, 2.0), None);
        // §6.1 "If the transcript file is gone, the launch drops --resume and starts fresh."
        std::fs::remove_file(&transcript).unwrap();
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        std::fs::write(&transcript, "{}\n").unwrap();
        // Finding 7: transcript file stem must equal session id.
        let wrong_stem = transcript_dir.join("other-id.jsonl");
        std::fs::write(&wrong_stem, "{}\n").unwrap();
        record(&home, folder, session_id, at, &wrong_stem);
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        // Finding 7: transcript parent folder must equal Claude Code's encoding of folder.
        let wrong_parent_dir = home.join("C--other-folder");
        std::fs::create_dir_all(&wrong_parent_dir).unwrap();
        let wrong_parent = wrong_parent_dir.join(format!("{session_id}.jsonl"));
        std::fs::write(&wrong_parent, "{}\n").unwrap();
        record(&home, folder, session_id, at, &wrong_parent);
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        // Finding 7: leading hyphen in session id is rejected.
        let hyphen_transcript = transcript_dir.join("-leading-hyphen.jsonl");
        std::fs::write(&hyphen_transcript, "{}\n").unwrap();
        record(&home, folder, "-leading-hyphen", at, &hyphen_transcript);
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        // Finding 7: at cannot be more than 5 minutes (300 s) in the future.
        record(&home, folder, session_id, at, &transcript);
        assert_eq!(resume_session_at(&home, folder, then - 301, 24.0), None);
        assert!(resume_session_at(&home, folder, then - 60, 24.0).is_some());
        // An id that is not one Claude Code writes is never typed into a shell.
        record(&home, folder, "x; rm -rf /", at, &transcript);
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        // A broken record is no record.
        std::fs::write(session_record_path(&home, folder), "{not json").unwrap();
        assert_eq!(resume_session_at(&home, folder, then + 60, 24.0), None);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn nirvana_agent_names_are_recognised_and_numbered() {
        assert!(is_nirvana_agent_name("nirvana"));
        assert!(is_nirvana_agent_name("nirvana-12"));
        assert!(!is_nirvana_agent_name("nirvana-"));
        assert!(!is_nirvana_agent_name("nirvana-x"));
        assert!(!is_nirvana_agent_name("claude"));
        let taken = ["nirvana", "nirvana-2"];
        assert_eq!(
            free_agent_name("nirvana", |n| taken.contains(&n)),
            "nirvana-3"
        );
        assert_eq!(free_agent_name("agy", |_| false), "agy");
    }

    #[test]
    fn one_folder_one_id_whatever_its_case_or_separators() {
        let a = Path::new(r"C:\Users\o\Desktop\Pojects\MotoProject");
        let b = Path::new("c:/users/o/desktop/pojects/motoproject/");
        assert_eq!(folder_hash(a), folder_hash(b));
        assert_eq!(folder_hash(a).len(), 16);
        assert!(folder_hash(a).bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(
            folder_hash(a),
            folder_hash(Path::new(r"C:\Users\o\Desktop\Pojects\Nirvana"))
        );
        let id = window_app_id(a);
        assert!(id.starts_with("Nirvana.Herdr.") && id.len() <= 128 && !id.contains(' '));
        // The toast's id is the fork's own and never a folder's.
        assert_ne!(TOAST_APP_ID, id);
        // A known value pins the hash, so a change of rule is a visible change of every button.
        assert_eq!(folder_key(a), "c:/users/o/desktop/pojects/motoproject");
    }

    #[test]
    fn the_letter_is_the_first_letter_or_digit_of_the_folder() {
        assert_eq!(folder_letter(Path::new("C:/work/pojects")), 'P');
        assert_eq!(folder_letter(Path::new("C:/work/2026-plans")), '2');
        assert_eq!(folder_letter(Path::new("C:/work/.config")), 'C');
        assert_eq!(folder_letter(Path::new("C:/work/über")), 'Ü');
        assert_eq!(folder_letter(Path::new("C:/work/日本")), '日');
        assert_eq!(letter_of("ßeta"), 'ß');
        assert_eq!(letter_of(""), 'N');
        assert_eq!(letter_of("---"), 'N');
        assert_eq!(folder_letter(Path::new("")), 'N');
        assert_eq!(toast_title(Path::new("C:/x/Pojects")), "Nirvana · Pojects");
    }

    #[test]
    fn the_verb_folder_survives_explorers_quoting_of_a_drive_root() {
        assert_eq!(verb_folder("C:\""), PathBuf::from("C:\\"));
        assert_eq!(verb_folder("D:"), PathBuf::from("D:\\"));
        assert_eq!(
            verb_folder(r"C:\Users\o\My Work"),
            PathBuf::from(r"C:\Users\o\My Work")
        );
    }

    #[test]
    fn the_open_command_round_trips_folder_and_fresh() {
        let folder = Path::new(r"C:\Users\o\My Work");
        for fresh in [false, true] {
            for new_space in [false, true] {
                let id = open_command_id(folder, fresh, new_space);
                let rest = id.strip_prefix(OPEN_COMMAND).unwrap();
                assert_eq!(
                    parse_open_command(rest),
                    (folder.to_path_buf(), fresh, new_space)
                );
            }
        }
    }

    #[test]
    fn launch_args_carries_rule_model_and_effort_in_efficient_mode() {
        let home = scratch_home("efficient");
        let folder = Path::new(r"C:\work\print");
        std::fs::write(
            home.join("nirvana").join("seats.toml"),
            crate::nirvana_seat::DEFAULT.replace("mode = \"balanced\"", "mode = \"efficient\""),
        )
        .unwrap();
        let args = launch_args(&home, folder, None);
        let m_idx = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[m_idx + 1], "claude-sonnet-5-5");
        let e_idx = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[e_idx + 1], "medium");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn launch_args_carries_rule_model_and_effort_in_sota_mode() {
        let home = scratch_home("sota");
        let folder = Path::new(r"C:\work\print");
        std::fs::write(
            home.join("nirvana").join("seats.toml"),
            crate::nirvana_seat::DEFAULT.replace("mode = \"balanced\"", "mode = \"sota\""),
        )
        .unwrap();
        let args = launch_args(&home, folder, None);
        let m_idx = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[m_idx + 1], "claude-opus-5-5");
        let e_idx = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[e_idx + 1], "high");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn launch_args_falls_back_to_opus_medium_when_table_fails_to_parse() {
        let home = scratch_home("broken-table");
        let folder = Path::new(r"C:\work\print");
        std::fs::write(
            home.join("nirvana").join("seats.toml"),
            "not valid toml { [ {",
        )
        .unwrap();
        let args = launch_args(&home, folder, None);
        let m_idx = args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(args[m_idx + 1], "claude-opus-5-5");
        let e_idx = args.iter().position(|a| a == "--effort").unwrap();
        assert_eq!(args[e_idx + 1], "medium");
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn liveness_check_requires_all_three_facts() {
        // Plan 21 §3.1 and §7: each of the three facts false alone gives not live; all true gives live.
        let mark = WindowMark {
            pid: 1234,
            hwnd: 5678,
            at: 1729500000,
        };
        assert!(is_mark_live(true, true, true));
        assert!(!is_mark_live(false, true, true));
        assert!(!is_mark_live(true, false, true));
        assert!(!is_mark_live(true, true, false));
        assert!(!is_mark_live(false, false, false));

        assert!(check_mark_live_with(&mark, |_| true, |_| true, || true));
        assert!(!check_mark_live_with(&mark, |_| false, |_| true, || true));
        assert!(!check_mark_live_with(&mark, |_| true, |_| false, || true));
        assert!(!check_mark_live_with(&mark, |_| true, |_| true, || false));
    }

    #[test]
    fn window_mark_write_read_and_remove_by_own_pid() {
        // Plan 21 §3.1 and §7: write, read back, remove only by its own pid.
        let home = scratch_home("mark");
        assert_eq!(read_window_mark(&home), None);

        let mark = WindowMark {
            pid: 4321,
            hwnd: 8765,
            at: 1729512345,
        };
        write_window_mark(&home, &mark).unwrap();
        assert_eq!(read_window_mark(&home), Some(mark));

        // Removing with a different pid fails and keeps the mark
        assert_eq!(remove_window_mark(&home, 9999).unwrap(), false);
        assert_eq!(read_window_mark(&home), Some(mark));

        // Removing with the mark's pid succeeds and deletes the file
        assert_eq!(remove_window_mark(&home, 4321).unwrap(), true);
        assert_eq!(read_window_mark(&home), None);
        assert!(!window_mark_path(&home).exists());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn torn_or_empty_window_mark_reads_as_no_mark() {
        // Plan 21 §3.1 and §7: a torn or empty file reads as no mark.
        let home = scratch_home("torn-mark");
        let path = window_mark_path(&home);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();

        std::fs::write(&path, b"").unwrap();
        assert_eq!(read_window_mark(&home), None);

        std::fs::write(&path, b"{\"pid\": 1234, \"hwnd\": ").unwrap();
        assert_eq!(read_window_mark(&home), None);

        std::fs::write(&path, b"not json at all").unwrap();
        assert_eq!(read_window_mark(&home), None);

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn route_table_covers_all_four_rows_as_data() {
        // Plan 21 §3.2 and §7: all four rows from fixed liveness and folder inputs as data
        let mark = WindowMark {
            pid: 1234,
            hwnd: 5678,
            at: 1729500000,
        };
        let folder = Path::new(r"C:\work\print");

        // Row 1: live mark = yes, folder = yes
        assert_eq!(
            route_open(Some(&mark), Some(folder)),
            OpenRoute::FocusOrOpenFolder {
                folder: folder.to_path_buf(),
                hwnd: 5678,
            }
        );

        // Row 2: live mark = yes, folder = no
        assert_eq!(
            route_open(Some(&mark), None),
            OpenRoute::FlashWindow { hwnd: 5678 }
        );

        // Row 3: live mark = no, folder = yes
        assert_eq!(
            route_open(None, Some(folder)),
            OpenRoute::StartWindowFolder {
                folder: folder.to_path_buf(),
            }
        );

        // Row 4: live mark = no, folder = no
        assert_eq!(route_open(None, None), OpenRoute::StartWindowNoFolder);
    }

    #[test]
    fn open_log_appends_errors() {
        // Plan 21 §3.2: open.log records failures with one line per error
        let home = scratch_home("open-log");
        let log = open_log_path(&home);
        assert!(!log.exists());

        log_open_error(&home, "first error message");
        assert!(log.is_file());
        let text1 = std::fs::read_to_string(&log).unwrap();
        assert!(text1.contains("first error message"));

        log_open_error(&home, "second error message");
        let text2 = std::fs::read_to_string(&log).unwrap();
        assert!(text2.contains("first error message"));
        assert!(text2.contains("second error message"));

        let _ = std::fs::remove_dir_all(&home);
    }
}
