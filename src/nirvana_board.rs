//! Nirvana fork, feature 8: the shared board. One file per workspace that every agent there can read and
//! write, and that a running Claude sees on its next turn after another agent changes it.
//!
//! The owner, 2026-10-16: "So the third  box  I also want to borrow from Pi Durable. They seem to have a
//! shared file among all the agents that when you update with one agent the other agents pick up on. Kind of
//! like a shared state file". Then: "Keep a board per workspace." and "flag this feature only if it opens a
//! project with a github". (nirvana/plans/2026-10-16-08-memory-square-and-board.md)
//!
//! **What it copies.** durable-pi's App State (disler/durable-pi 37165c6, `shared-state.ts`): one shared
//! document, a revision raised by every write, a write that names the revision it read and is refused when
//! another write came first, and agents told to read before they write. durable-pi's agents pull; nothing is
//! pushed to them.
//!
//! **What it adds.** A running Claude is told what changed. Measured 2026-10-16 on Claude Code 2.1.301: an
//! edit to CLAUDE.md or a file it imports never reaches a running session, a `FileChanged` hook's text never
//! reaches Claude, and a `UserPromptSubmit` hook's output reaches it on the next turn (about 45 tokens for a
//! one-line change). So `herdr board notice`, run by that hook, prints only the sections that changed since
//! that session last saw the board, and nothing when none did.
//!
//! **Any provider writes it** through this binary, `herdr board read` and `herdr board write`, so an agent
//! needs only a shell.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Map, Value};

/// The board's folder and file at the workspace's root.
pub const BOARD_DIR: &str = ".nirvana";
pub const BOARD_FILE: &str = "board.md";
const LOCK_FILE: &str = "board.lock";
/// How long a write waits for another writer's lock, and how old a lock is before it is a dead writer's.
const LOCK_WAIT: Duration = Duration::from_secs(3);
const LOCK_STALE: Duration = Duration::from_secs(15);
/// What marks the hook herdr adds: the command ends with it.
pub const NOTICE_COMMAND: &str = "board notice";
/// A write whose revision is stale exits with this, as durable-pi answers 409.
pub const EXIT_STALE: i32 = 3;
/// The window reads a folder's board again after this long.
const VIEW_TTL: Duration = Duration::from_secs(2);

/// One topic on the board.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub text: String,
    /// Who wrote it last, and when (RFC 3339, UTC); empty after an edit by hand.
    pub by: String,
    pub at: String,
}

/// The board: a revision and its sections, in the file's order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    pub revision: u64,
    /// Any text above the first section, kept as it is.
    pub preamble: String,
    pub sections: Vec<Section>,
}

fn by_line(line: &str) -> Option<(String, String)> {
    let inner = line
        .trim()
        .strip_prefix("<!-- by ")?
        .strip_suffix("-->")?
        .trim();
    let (by, at) = inner.split_once(" at ").unwrap_or((inner, ""));
    Some((by.trim().to_owned(), at.trim().to_owned()))
}

fn trim_blank_lines(lines: &[&str]) -> String {
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

impl Board {
    /// Reads `board.md`. Anything it does not know is kept: a hand edit is never lost by the next write.
    pub fn parse(text: &str) -> Board {
        let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let mut board = Board::default();
        let mut body = text.as_str();
        if let Some(rest) = body.strip_prefix("---\n") {
            if let Some(end) = rest.find("\n---") {
                for line in rest[..end].lines() {
                    if let Some(value) = line.trim().strip_prefix("revision:") {
                        board.revision = value.trim().parse().unwrap_or(0);
                    }
                }
                body = rest[end + 4..]
                    .strip_prefix('\n')
                    .unwrap_or(&rest[end + 4..]);
            }
        }
        let mut preamble: Vec<&str> = Vec::new();
        let mut current: Option<(String, Vec<&str>, String, String)> = None;
        let finish = |current: Option<(String, Vec<&str>, String, String)>, board: &mut Board| {
            if let Some((name, lines, by, at)) = current {
                board.sections.push(Section {
                    name,
                    text: trim_blank_lines(&lines),
                    by,
                    at,
                });
            }
        };
        for line in body.lines() {
            if let Some(name) = line.strip_prefix("## ") {
                finish(current.take(), &mut board);
                current = Some((
                    name.trim().to_owned(),
                    Vec::new(),
                    String::new(),
                    String::new(),
                ));
            } else if let Some((_, lines, by, at)) = current.as_mut() {
                match by_line(line) {
                    Some((who, when)) => {
                        *by = who;
                        *at = when;
                    }
                    None => lines.push(line),
                }
            } else {
                preamble.push(line);
            }
        }
        finish(current.take(), &mut board);
        board.preamble = trim_blank_lines(&preamble);
        board
    }

    pub fn render(&self) -> String {
        let mut out = format!("---\nrevision: {}\n---\n", self.revision);
        if !self.preamble.is_empty() {
            out.push('\n');
            out.push_str(&self.preamble);
            out.push('\n');
        }
        for section in &self.sections {
            out.push_str(&format!("\n## {}\n", section.name));
            if !section.text.is_empty() {
                out.push_str(&section.text);
                out.push('\n');
            }
            if !section.by.is_empty() {
                out.push_str(&format!("<!-- by {} at {} -->\n", section.by, section.at));
            }
        }
        out
    }

    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections
            .iter()
            .find(|section| section.name.eq_ignore_ascii_case(name))
    }

    /// Sets one section's text, or removes the section when the text is empty. Every other section stays.
    fn set(&mut self, name: &str, text: &str, by: &str, at: &str) {
        let text = text.trim().to_owned();
        let index = self
            .sections
            .iter()
            .position(|section| section.name.eq_ignore_ascii_case(name));
        match (index, text.is_empty()) {
            (Some(index), true) => {
                self.sections.remove(index);
            }
            (Some(index), false) => {
                let section = &mut self.sections[index];
                section.text = text;
                section.by = by.to_owned();
                section.at = at.to_owned();
            }
            (None, true) => {}
            (None, false) => self.sections.push(Section {
                name: name.trim().to_owned(),
                text,
                by: by.to_owned(),
                at: at.to_owned(),
            }),
        }
    }
}

/// The workspace a folder belongs to: the nearest folder at or above it that holds `.git`. Every agent in
/// that repository shares one board, whichever subfolder it started in.
pub fn root(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

pub fn board_path(root: &Path) -> PathBuf {
    root.join(BOARD_DIR).join(BOARD_FILE)
}

/// The repository's git config: `.git/config`, or for a worktree, the config of the repository it belongs
/// to (`.git` is then a file naming the worktree's git folder, whose `commondir` names the shared one).
fn git_config(root: &Path) -> Option<PathBuf> {
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git.join("config"));
    }
    let text = std::fs::read_to_string(&dot_git).ok()?;
    let git_dir = text.trim().strip_prefix("gitdir:")?.trim();
    let git_dir = root.join(git_dir);
    let common = std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|common| git_dir.join(common.trim()))
        .unwrap_or(git_dir);
    Some(common.join("config"))
}

/// Whether the repository has a GitHub remote, read from its git config, never by running `git`: the window
/// asks this while it draws.
pub fn has_github_remote(root: &Path) -> bool {
    git_config(root)
        .and_then(|config| std::fs::read_to_string(config).ok())
        .is_some_and(|config| {
            config.lines().any(|line| {
                let line = line.trim();
                line.starts_with("url") && line.contains("github.com")
            })
        })
}

pub fn read(root: &Path) -> Option<Board> {
    std::fs::read_to_string(board_path(root))
        .ok()
        .map(|text| Board::parse(&text))
}

fn write_file(path: &Path, text: &str) -> std::io::Result<()> {
    let dir = path.parent().expect("a parent");
    std::fs::create_dir_all(dir)?;
    let temp = dir.join(format!(".{BOARD_FILE}.{}.tmp", std::process::id()));
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// The writers' lock: a file created only if absent. A lock older than `LOCK_STALE` is a writer that died.
struct Lock(PathBuf);

impl Lock {
    fn take(root: &Path) -> std::io::Result<Lock> {
        Lock::take_at(&root.join(BOARD_DIR).join(LOCK_FILE))
    }

    fn take_at(path: &Path) -> std::io::Result<Lock> {
        let path = path.to_path_buf();
        std::fs::create_dir_all(path.parent().expect("a parent"))?;
        let started = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Lock(path)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let old = std::fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                        .is_some_and(|age| age > LOCK_STALE);
                    if old {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if started.elapsed() > LOCK_WAIT {
                        return Err(std::io::Error::other(format!(
                            "another writer holds {}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                // Plan 40, measured in `a_file_lock_keeps_both_writers_lines` (2 runs in 12 failed before this): on
                // Windows a lock file the last writer just removed is "delete pending" for a moment, and creating it
                // then is Access denied, not AlreadyExists. That is a lock being let go: wait for it like a held one.
                Err(err)
                    if err.kind() == std::io::ErrorKind::PermissionDenied
                        && started.elapsed() <= LOCK_WAIT =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => return Err(err),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum WriteError {
    /// Another write came first: the board is at this revision.
    Stale(u64),
    Io(String),
}

fn now() -> String {
    time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .ok()
        .and_then(|now| {
            now.format(&time::format_description::well_known::Rfc3339)
                .ok()
        })
        .unwrap_or_default()
}

/// Writes one section if the board is still at `expect`, and returns the new revision. Empty text removes the
/// section.
pub fn write(
    root: &Path,
    name: &str,
    text: &str,
    expect: u64,
    by: &str,
) -> Result<u64, WriteError> {
    if name.trim().is_empty() {
        return Err(WriteError::Io("a section needs a name".into()));
    }
    let _lock = Lock::take(root).map_err(|err| WriteError::Io(err.to_string()))?;
    let mut board = read(root).unwrap_or_default();
    if board.revision != expect {
        return Err(WriteError::Stale(board.revision));
    }
    board.set(name, text, by, &now());
    board.revision += 1;
    write_file(&board_path(root), &board.render())
        .map_err(|err| WriteError::Io(err.to_string()))?;
    Ok(board.revision)
}

/// Creates an empty board when there is none, for "edit": the editor opens only a file that exists.
pub fn ensure(root: &Path) -> std::io::Result<PathBuf> {
    let path = board_path(root);
    if !path.is_file() {
        write_file(&path, &Board::default().render())?;
    }
    Ok(path)
}

/// The sections whose text differs from what a session last saw, in the board's order.
pub fn unseen(board: &Board, seen: Option<&Board>) -> Vec<String> {
    board
        .sections
        .iter()
        .filter(|section| {
            seen.and_then(|seen| seen.section(&section.name))
                .is_none_or(|old| old.text != section.text)
        })
        .map(|section| section.name.clone())
        .collect()
}

/// The command a hook or an agent runs: this binary, quoted.
fn exe_line(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

fn footer(exe: &Path) -> String {
    let exe = exe_line(exe);
    format!(
        "Shared board of this workspace, for every agent here. Read it: {exe} board read. Write one section: \
         {exe} board write <section> --expect <revision> --by <your name> -- <text> (empty text removes it). \
         A write with a stale revision is refused: read again first."
    )
}

fn section_text(section: &Section) -> String {
    let by = if section.by.is_empty() {
        String::new()
    } else {
        format!(" (by {})", section.by)
    };
    format!("## {}{by}\n{}", section.name, section.text)
}

/// The most of the board a session's first notice prints whole. Plan 40 T1: the first prompt of every session (and
/// of every `/clear`) printed the whole board, uncapped, so a 20 KB board cost about 5k tokens per session per
/// agent. Over this, the first notice is each section's name and first line, the size, and the command that reads
/// the rest; the fresh note caps its board heads the same way (`nirvana_fresh`). A hypothesis: about 400 tokens.
pub const FIRST_NOTICE_WHOLE_CHARS: usize = 1_200;
/// A section's first line in a capped notice is cut to this.
const FIRST_LINE_CHARS: usize = 160;

fn first_line(text: &str) -> String {
    let line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.chars().count() > FIRST_LINE_CHARS {
        let cut: String = line.chars().take(FIRST_LINE_CHARS).collect();
        format!("{cut}...")
    } else {
        line.to_owned()
    }
}

/// What a session is told on its next prompt. `seen` is what it saw last; `None` is its first prompt. `None`
/// back means print nothing: the board has not changed for it.
pub fn notice(board: &Board, seen: Option<&Board>, exe: &Path) -> Option<String> {
    let Some(seen) = seen else {
        let mut out = if board.sections.is_empty() {
            "The shared board is empty so far.".to_owned()
        } else {
            let sections: Vec<_> = board.sections.iter().map(section_text).collect();
            let whole = sections.join("\n");
            let size = whole.chars().count();
            if size <= FIRST_NOTICE_WHOLE_CHARS {
                format!("Shared board, revision {}:\n{whole}", board.revision)
            } else {
                let heads: Vec<String> = board
                    .sections
                    .iter()
                    .map(|section| {
                        let by = if section.by.is_empty() {
                            String::new()
                        } else {
                            format!(" (by {})", section.by)
                        };
                        format!("## {}{by}: {}", section.name, first_line(&section.text))
                    })
                    .collect();
                format!(
                    "Shared board, revision {}: {} sections, {size} characters. Each section's first line is below; \
                     {} board read prints all of it.\n{}",
                    board.revision,
                    board.sections.len(),
                    exe_line(exe),
                    heads.join("\n")
                )
            }
        };
        out.push('\n');
        out.push_str(&footer(exe));
        return Some(out);
    };
    let changed = unseen(board, Some(seen));
    let removed: Vec<_> = seen
        .sections
        .iter()
        .filter(|old| board.section(&old.name).is_none())
        .map(|old| old.name.clone())
        .collect();
    if changed.is_empty() && removed.is_empty() {
        return None;
    }
    let mut out = format!(
        "The shared board changed since your last turn (now revision {}):",
        board.revision
    );
    for name in &changed {
        if let Some(section) = board.section(name) {
            out.push('\n');
            out.push_str(&section_text(section));
        }
    }
    if !removed.is_empty() {
        out.push_str(&format!("\nRemoved: {}", removed.join(", ")));
    }
    // The full how-to came with the first prompt. The live run of 2026-10-16 measured a change notice at
    // 666 characters, most of it that how-to with the program's path twice; one short line keeps the path.
    out.push_str(&format!(
        "\n(Board: {} board read | board write <section> --expect {} --by <you> -- <text>)",
        exe_line(exe),
        board.revision
    ));
    Some(out)
}

/// Where the board a session last saw is kept: under the Nirvana-Herdr home, never in the project.
fn seen_path(session: &str) -> Option<PathBuf> {
    let session: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if session.is_empty() {
        return None;
    }
    let home = crate::nirvana::home().unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"));
    Some(home.join("board-seen").join(format!("{session}.md")))
}

pub fn seen(session: &str) -> Option<Board> {
    let text = std::fs::read_to_string(seen_path(session)?).ok()?;
    Some(Board::parse(&text))
}

fn remember(session: &str, board: &Board) {
    if let Some(path) = seen_path(session) {
        let _ = write_file(&path, &board.render());
    }
}

/// The session a transcript belongs to: its file name, as Claude Code names it.
pub fn session_of(transcript: &Path) -> Option<String> {
    transcript
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
}

/// The binary hooks and agents run: the installed one when there is one, so a reload never leaves a hook
/// pointing at a build that is gone.
pub fn exe() -> PathBuf {
    crate::nirvana::home()
        .map(|home| crate::nirvana::installed_exe_in(&home))
        .filter(|exe| exe.is_file())
        .or_else(|| std::env::current_exe().ok())
        .unwrap_or_else(|| PathBuf::from("herdr"))
}

fn hook_entries(local: &Value) -> impl Iterator<Item = &Value> {
    local
        .pointer("/hooks/UserPromptSubmit")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
}

fn is_ours(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.trim_end().ends_with(NOTICE_COMMAND))
}

/// Whether the folder's local settings run the board's notice hook.
pub fn is_shared(local: &Value) -> bool {
    hook_entries(local).any(is_ours)
}

/// Adds or removes the board's notice hook. Every other setting and hook stays as it was.
pub fn set_shared(local: &mut Value, exe: &Path, on: bool) {
    if !local.is_object() {
        *local = Value::Object(Map::new());
    }
    let root = local.as_object_mut().expect("an object");
    if on {
        if is_shared(&Value::Object(root.clone())) {
            return;
        }
        let hooks = root
            .entry("hooks")
            .or_insert_with(|| Value::Object(Map::new()));
        if !hooks.is_object() {
            *hooks = Value::Object(Map::new());
        }
        let list = hooks
            .as_object_mut()
            .expect("an object")
            .entry("UserPromptSubmit")
            .or_insert_with(|| Value::Array(Vec::new()));
        if !list.is_array() {
            *list = Value::Array(Vec::new());
        }
        list.as_array_mut().expect("an array").push(json!({
            "hooks": [{"type": "command", "command": format!("{} {NOTICE_COMMAND}", exe_line(exe))}]
        }));
        return;
    }
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(list) = hooks
        .get_mut("UserPromptSubmit")
        .and_then(Value::as_array_mut)
    {
        for group in list.iter_mut() {
            if let Some(inner) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                inner.retain(|hook| !is_ours(hook));
            }
        }
        list.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|inner| !inner.is_empty())
        });
        if list.is_empty() {
            hooks.remove("UserPromptSubmit");
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
}

/// Switches the hook in a folder's local settings: the file Preload's switches use.
pub fn share(cwd: &Path, on: bool) -> Result<(), String> {
    // Plan 40 B6: through the one read-check-write, so a grant Claude Code wrote meanwhile is not lost.
    let exe = exe();
    crate::nirvana_preload::update_local(cwd, &|local| set_shared(local, &exe, on))?;
    forget(cwd);
    Ok(())
}

/// What the window shows for a folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BoardView {
    pub root: PathBuf,
    pub board: Option<Board>,
    /// The folder's local settings run the notice hook.
    pub shared: bool,
}

/// Each folder's board as the window last read it, and when.
type Views = Mutex<HashMap<PathBuf, (Instant, Option<BoardView>)>>;

fn views() -> &'static Views {
    static VIEWS: OnceLock<Views> = OnceLock::new();
    VIEWS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn load(cwd: &Path) -> Option<BoardView> {
    let root = root(cwd)?;
    if !has_github_remote(&root) {
        return None;
    }
    let shared = crate::nirvana_preload::read_local(cwd)
        .map(|local| is_shared(&local))
        .unwrap_or(false);
    Some(BoardView {
        board: read(&root),
        root,
        shared,
    })
}

/// The board for the window, read again at most every `VIEW_TTL`. `None`: no Memory square, because the
/// folder is not in a repository with a GitHub remote.
pub fn view(cwd: &Path) -> Option<BoardView> {
    if cfg!(test) {
        return load(cwd);
    }
    let mut views = views().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, view)) = views.get(cwd) {
        if at.elapsed() < VIEW_TTL {
            return view.clone();
        }
    }
    let view = load(cwd);
    views.insert(cwd.to_path_buf(), (Instant::now(), view.clone()));
    view
}

/// Drops the window's copy, after a switch or an edit.
pub fn forget(cwd: &Path) {
    views()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(cwd);
}

/// The strip's value for the Memory square, and whether it asks for attention.
pub fn square_value(view: &BoardView, seen: Option<&Board>) -> (String, bool) {
    let board = view
        .board
        .as_ref()
        .filter(|board| !board.sections.is_empty());
    let mut value = match board {
        Some(board) => format!("board r{}", board.revision),
        None => "board empty".to_owned(),
    };
    if !view.shared {
        value.push_str(" \u{b7} not shared");
        return (value, false);
    }
    let new = board.map_or(0, |board| unseen(board, seen).len());
    if new > 0 {
        value.push_str(&format!(" \u{b7} {new} new"));
    }
    (value, new > 0)
}

const USAGE: &str = "usage: herdr board read [<section>]\n       herdr board write <section> --expect <revision> [--by <name>] [--cwd <folder>] -- <text>\n       herdr board notice   (a Claude Code UserPromptSubmit hook; reads the hook's JSON on stdin)";

fn fail(message: &str, code: i32) -> ! {
    eprintln!("{message}");
    std::process::exit(code)
}

/// `herdr board ...`.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let mut cwd = std::env::current_dir()?;
    let mut expect = None;
    let mut by = String::from("agent");
    let mut words = Vec::new();
    let mut rest = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--" => {
                rest.extend(iter.by_ref().cloned());
                break;
            }
            "--expect" => expect = iter.next().and_then(|n| n.parse::<u64>().ok()),
            "--by" => by = iter.next().cloned().unwrap_or(by),
            "--cwd" => cwd = iter.next().map(PathBuf::from).unwrap_or(cwd),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => words.push(arg.clone()),
        }
    }
    match words.first().map(String::as_str) {
        Some("notice") => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            // Plan 12 O4b: the Nirvana Claude plugin runs this hook with `--plugin`. A folder whose own settings run
            // it as well gets the notice once, from its own hook; two copies raced to print the same notice.
            if words.iter().any(|word| word == "--plugin")
                && crate::nirvana_claude::folder_runs(&input, "UserPromptSubmit", NOTICE_COMMAND)
            {
                return Ok(());
            }
            run_notice(&input);
            Ok(())
        }
        Some("read") => {
            let Some(root) = root(&cwd) else {
                fail("not in a git repository: the board lives at its root", 2)
            };
            let board = read(&root).unwrap_or_default();
            let out = match words.get(1) {
                Some(name) => match board.section(name) {
                    Some(section) => {
                        format!("revision: {}\n{}\n", board.revision, section_text(section))
                    }
                    None => format!("revision: {}\n(no section {name})\n", board.revision),
                },
                None => board.render(),
            };
            std::io::stdout().write_all(out.as_bytes())
        }
        Some("write") => {
            let Some(name) = words.get(1) else {
                fail(USAGE, 2)
            };
            let Some(expect) = expect else {
                fail("write needs --expect <revision>: read the board first", 2)
            };
            let Some(root) = root(&cwd) else {
                fail("not in a git repository: the board lives at its root", 2)
            };
            let mut text = rest.join(" ");
            if text.is_empty() {
                text = words[2..].join(" ");
            }
            match write(&root, name, &text, expect, &by) {
                Ok(revision) => {
                    println!("revision: {revision}");
                    Ok(())
                }
                Err(WriteError::Stale(revision)) => fail(
                    &format!("board is at revision {revision}, not {expect}; read it again"),
                    EXIT_STALE,
                ),
                Err(WriteError::Io(err)) => fail(&err, 1),
            }
        }
        _ => fail(USAGE, 2),
    }
}

pub(crate) fn check_seat_notice(session: &str, root: Option<&Path>) -> Option<String> {
    let home = crate::nirvana::home().unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"));
    let mut record = crate::nirvana_claude::read_session_seat(&home, session)?;
    let table = match root {
        Some(r) => crate::nirvana_seat::table_for(r).ok()?,
        None => crate::nirvana_seat::table().ok()?,
    };
    // The cache alone: a stale reading must not run `claude /usage` inside the prompt hook (cached_world).
    let now = crate::nirvana_usage::unix(std::time::SystemTime::now());
    let world = crate::nirvana_seat::cached_world(&home.join(crate::nirvana_seat::DIR), now);
    let rule = crate::nirvana_seat::orchestrator_seat(&table, &world);
    let last = match (&record.last_notified_seat, &record.last_notified_effort) {
        (Some(s), Some(e)) => Some((s.as_str(), e.as_str())),
        _ => None,
    };
    let (line, notified) = crate::nirvana_claude::seat_notice(
        &record.seat,
        &record.effort,
        &rule.seat,
        rule.effort.word(),
        &rule.reason,
        last,
    );
    if let Some((s, e)) = notified {
        if record.last_notified_seat.as_deref() != Some(&s)
            || record.last_notified_effort.as_deref() != Some(&e)
        {
            record.last_notified_seat = Some(s);
            record.last_notified_effort = Some(e);
            let _ = crate::nirvana_claude::write_session_seat(&home, &record);
        }
    }
    line
}

/// The hook: prints what changed for this session, then remembers what it saw. Never fails the prompt: any
/// problem prints nothing.
fn run_notice(input: &str) {
    let hook: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let session = hook
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cwd = hook
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let root = cwd.as_deref().and_then(root);
    if !session.is_empty() {
        if let Some(line) = check_seat_notice(session, root.as_deref()) {
            println!("{line}");
        }
    }
    let Some(root) = root else {
        return;
    };
    let board = read(&root).unwrap_or_default();
    let seen = seen(session);
    if let Some(text) = notice(&board, seen.as_ref(), &exe()) {
        println!("{text}");
    }
    remember(session, &board);
    // Plan 40 D1: one file per session, never removed. A new session is the time to sweep, once.
    if seen.is_none() {
        if let Some(dir) = seen_path(session).and_then(|p| p.parent().map(Path::to_path_buf)) {
            prune_dir(&dir, 0, PRUNE_AFTER, SystemTime::now());
        }
    }
}

// ---- pruning the files that only grow (plan 40 D1) ---------------------------------------------------

/// Per-session files and log lines older than this are removed when their module next writes there.
pub const PRUNE_AFTER: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Deletes the files of `oldest_first` older than `max_age`, but never the newest `keep_newest` of them. A file
/// whose time cannot be read is kept. Returns how many went.
pub fn prune_old_files(
    oldest_first: Vec<PathBuf>,
    keep_newest: usize,
    max_age: Duration,
    now: SystemTime,
) -> usize {
    let candidates = oldest_first.len().saturating_sub(keep_newest);
    let mut gone = 0;
    for path in oldest_first.into_iter().take(candidates) {
        let old = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > max_age);
        if old && std::fs::remove_file(&path).is_ok() {
            gone += 1;
        }
    }
    gone
}

/// `prune_old_files` over the plain files of one folder, oldest first by their write time.
pub fn prune_dir(dir: &Path, keep_newest: usize, max_age: Duration, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let modified = entry.metadata().and_then(|meta| meta.modified()).ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    files.sort();
    prune_old_files(
        files.into_iter().map(|(_, path)| path).collect(),
        keep_newest,
        max_age,
        now,
    )
}

/// A log is rewritten only once its first line is this much past `max_age`, so a log in steady use is rewritten
/// about once a day, not on every write after its first line ages out.
const PRUNE_SLACK: Duration = Duration::from_secs(24 * 60 * 60);

/// Drops the lines of a JSON-lines log whose `ts` (RFC 3339) is older than `max_age`. It reads only the first line
/// unless that one is older than `max_age` and a day, so a log with nothing to drop costs one short read per call. A
/// line with no readable `ts` is kept. The rewrite goes through a temporary file and a rename; an append from another
/// process between the read and the rename is lost, which the daily rewrite makes rare. Returns how many lines went.
pub fn prune_log(path: &Path, max_age: Duration, now: SystemTime) -> usize {
    let (Some(cutoff), Some(trigger)) = (
        now.checked_sub(max_age),
        now.checked_sub(max_age + PRUNE_SLACK),
    ) else {
        return 0;
    };
    let older_than = |line: &str, when: SystemTime| -> bool {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| value.get("ts").and_then(Value::as_str).map(str::to_owned))
            .and_then(|ts| {
                time::OffsetDateTime::parse(&ts, &time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .is_some_and(|at| SystemTime::from(at) < when)
    };
    let first = std::fs::File::open(path).ok().and_then(|file| {
        use std::io::BufRead;
        std::io::BufReader::new(file).lines().next()?.ok()
    });
    if !first.as_deref().is_some_and(|line| older_than(line, trigger)) {
        return 0;
    }
    let old = |line: &str| older_than(line, cutoff);
    prune_lines(path, &old)
}

fn prune_lines(path: &Path, old: &dyn Fn(&str) -> bool) -> usize {
    let Ok(text) = std::fs::read_to_string(path) else {
        return 0;
    };
    let mut gone = 0;
    let mut kept = String::with_capacity(text.len());
    for line in text.lines() {
        if old(line) {
            gone += 1;
        } else if !line.trim().is_empty() {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    let temp = crate::nirvana_session_record::temp_sibling(path);
    if std::fs::write(&temp, kept).is_err() || std::fs::rename(&temp, path).is_err() {
        let _ = std::fs::remove_file(&temp);
        return 0;
    }
    gone
}

// ---- a lock beside any file (plan 40 B5) -------------------------------------------------------------

/// Holds `<file>.lock` while a read-modify-write of `file` runs; the same rules as the board's own lock.
pub struct FileLock {
    _lock: Lock,
}

impl FileLock {
    pub fn take(file: &Path) -> std::io::Result<FileLock> {
        let mut name = file.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        Lock::take_at(&file.with_file_name(name)).map(|lock| FileLock { _lock: lock })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nirvana-board-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git")).expect("temp repo");
        dir
    }

    const BOARD: &str = "---\nrevision: 3\n---\n\nKept above the sections.\n\n## Server\nport 5173\n<!-- by codex at 2026-10-16T14:02:11Z -->\n\n## Ownership\nsrc/api/** is claude's\nsrc/ui/** is codex's\n";

    #[test]
    fn the_board_reads_and_writes_back_as_it_was() {
        let board = Board::parse(BOARD);
        assert_eq!(board.revision, 3);
        assert_eq!(board.preamble, "Kept above the sections.");
        assert_eq!(board.sections.len(), 2);
        assert_eq!(board.sections[0].text, "port 5173");
        assert_eq!(board.sections[0].by, "codex");
        assert_eq!(board.sections[0].at, "2026-10-16T14:02:11Z");
        assert_eq!(
            board.sections[1].text,
            "src/api/** is claude's\nsrc/ui/** is codex's"
        );
        assert_eq!(board.sections[1].by, "");
        assert_eq!(Board::parse(&board.render()), board);
        // CRLF from an editor on Windows reads the same.
        assert_eq!(Board::parse(&BOARD.replace('\n', "\r\n")), board);
    }

    #[test]
    fn a_write_names_the_revision_it_read_and_a_stale_one_is_refused() {
        let root = scratch("write");
        assert_eq!(write(&root, "Server", "port 5173", 0, "codex"), Ok(1));
        assert_eq!(
            write(&root, "Decision", "Stripe test mode", 1, "claude"),
            Ok(2)
        );
        // Another writer came first: refused, nothing written.
        assert_eq!(
            write(&root, "Server", "port 9999", 1, "claude"),
            Err(WriteError::Stale(2))
        );
        let board = read(&root).expect("a board");
        assert_eq!(board.revision, 2);
        assert_eq!(
            board.section("server").map(|s| s.text.as_str()),
            Some("port 5173")
        );
        assert_eq!(
            board.section("Decision").map(|s| s.by.as_str()),
            Some("claude")
        );
        // Empty text removes the section; the others stay.
        assert_eq!(write(&root, "Server", "", 2, "you"), Ok(3));
        let board = read(&root).expect("a board");
        assert!(board.section("Server").is_none());
        assert!(board.section("Decision").is_some());
        // The lock is gone after each write.
        assert!(!root.join(BOARD_DIR).join(LOCK_FILE).exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hand_edit_survives_the_next_write() {
        let root = scratch("hand");
        std::fs::create_dir_all(root.join(BOARD_DIR)).unwrap();
        std::fs::write(board_path(&root), BOARD).unwrap();
        assert_eq!(write(&root, "Server", "port 5174", 3, "codex"), Ok(4));
        let board = read(&root).expect("a board");
        assert_eq!(board.preamble, "Kept above the sections.");
        assert_eq!(
            board.section("Ownership").map(|s| s.text.as_str()),
            Some("src/api/** is claude's\nsrc/ui/** is codex's")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_dead_writers_lock_is_taken_over_and_a_live_one_waits() {
        let root = scratch("lock");
        let held = Lock::take(&root).expect("first lock");
        let started = Instant::now();
        assert!(matches!(
            write(&root, "Server", "x", 0, "codex"),
            Err(WriteError::Io(_))
        ));
        assert!(started.elapsed() >= LOCK_WAIT);
        drop(held);
        assert_eq!(write(&root, "Server", "x", 0, "codex"), Ok(1));
        // A writer that died left its lock: older than LOCK_STALE, it is taken over at once.
        let lock = root.join(BOARD_DIR).join(LOCK_FILE);
        let file = std::fs::File::create(&lock).expect("a dead writer's lock");
        file.set_modified(SystemTime::now() - LOCK_STALE - Duration::from_secs(5))
            .expect("aged");
        drop(file);
        let started = Instant::now();
        assert_eq!(write(&root, "Server", "y", 1, "codex"), Ok(2));
        assert!(started.elapsed() < LOCK_WAIT);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_folder_in_a_repository_shares_its_board() {
        let root = scratch("root");
        let deep = root.join("src").join("api");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(root_of(&deep), Some(root.clone()));
        assert_eq!(root_of(&root), Some(root.clone()));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn root_of(dir: &Path) -> Option<PathBuf> {
        root(dir)
    }

    #[test]
    fn the_feature_is_on_only_for_a_repository_with_a_github_remote() {
        let root = scratch("github");
        std::fs::write(
            root.join(".git").join("config"),
            "[remote \"origin\"]\n\turl = https://gitlab.com/x/y.git\n",
        )
        .unwrap();
        assert!(!has_github_remote(&root));
        std::fs::write(
            root.join(".git").join("config"),
            "[core]\n\tbare = false\n[remote \"origin\"]\n\turl = git@github.com:x/y.git\n",
        )
        .unwrap();
        assert!(has_github_remote(&root));
        // A worktree: .git is a file naming its git folder, whose commondir names the shared one.
        let tree = std::env::temp_dir().join(format!("nirvana-board-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tree);
        std::fs::create_dir_all(&tree).unwrap();
        let wt_git = root.join(".git").join("worktrees").join("tree");
        std::fs::create_dir_all(&wt_git).unwrap();
        std::fs::write(wt_git.join("commondir"), "../..\n").unwrap();
        std::fs::write(tree.join(".git"), format!("gitdir: {}\n", wt_git.display())).unwrap();
        assert!(has_github_remote(&tree));
        // No repository at all: no board.
        let bare = std::env::temp_dir().join(format!("nirvana-board-none-{}", std::process::id()));
        std::fs::create_dir_all(&bare).unwrap();
        assert!(view(&bare).is_none());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&tree);
        let _ = std::fs::remove_dir_all(&bare);
    }

    #[test]
    fn a_session_is_told_only_what_changed_since_its_last_turn() {
        let exe = Path::new(r"C:\bin\herdr.exe");
        let board = Board::parse(BOARD);
        // First prompt: the whole board, and how to read and write it.
        let first = notice(&board, None, exe).expect("a first notice");
        assert!(first.contains("revision 3"), "{first}");
        assert!(first.contains("## Server (by codex)\nport 5173"), "{first}");
        assert!(
            first.contains("\"C:\\bin\\herdr.exe\" board write <section> --expect"),
            "{first}"
        );
        // Nothing changed: nothing printed, so the turn costs nothing.
        assert_eq!(notice(&board, Some(&board), exe), None);
        // One section changed and one removed: only those.
        let mut next = board.clone();
        next.set("Server", "port 5174", "codex", "t");
        next.sections.retain(|section| section.name != "Ownership");
        next.revision = 4;
        let changed = notice(&next, Some(&board), exe).expect("a change notice");
        assert!(changed.contains("now revision 4"), "{changed}");
        assert!(changed.contains("port 5174"), "{changed}");
        assert!(!changed.contains("src/api"), "{changed}");
        assert!(changed.contains("Removed: Ownership"), "{changed}");
        // Later notices carry a one-line how-to, with the revision to expect, not the first one's paragraph.
        assert!(
            changed.contains("board write <section> --expect 4"),
            "{changed}"
        );
        assert!(
            !changed.contains("Shared board of this workspace"),
            "{changed}"
        );
        assert!(changed.len() < first.len(), "{changed}");
        // An empty board on the first prompt still says how to write.
        let empty = notice(&Board::default(), None, exe).expect("a first notice");
        assert!(
            empty.contains("empty so far") && empty.contains("board write"),
            "{empty}"
        );
        assert_eq!(unseen(&next, Some(&board)), vec!["Server".to_owned()]);
    }

    /// Plan 40 T1: a big board's first notice is capped to each section's first line, its size and the read
    /// command; a small board still comes whole; later notices are as they were.
    #[test]
    fn a_big_boards_first_notice_is_capped() {
        let exe = Path::new(r"C:\bin\herdr.exe");
        let mut board = Board::parse(BOARD);
        for n in 0..12 {
            let body = format!("first line of topic {n}\n{}", "a long note line\n".repeat(80));
            board.set(&format!("Topic {n}"), &body, "claude", "t");
        }
        board.revision = 9;
        // The notice as it was before the cap: the whole board, then the how-to.
        let sections: Vec<_> = board.sections.iter().map(section_text).collect();
        let whole = format!(
            "Shared board, revision {}:\n{}\n{}",
            board.revision,
            sections.join("\n"),
            footer(exe)
        )
        .chars()
        .count();
        let first = notice(&board, None, exe).expect("a first notice");
        println!(
            "plan 40 T1: first board notice, before {whole} chars, after {}",
            first.chars().count()
        );
        assert!(first.chars().count() * 4 < whole, "{first}");
        assert!(first.contains("14 sections"), "{first}");
        assert!(first.contains("## Topic 3 (by claude): first line of topic 3"), "{first}");
        assert!(!first.contains("a long note line"), "{first}");
        assert!(first.contains("\"C:\\bin\\herdr.exe\" board read prints all"), "{first}");
        assert!(first.contains("board write <section> --expect"), "{first}");
        // A small board is whole, as before.
        let small = notice(&Board::parse(BOARD), None, exe).unwrap();
        assert!(small.contains("## Server (by codex)\nport 5173"), "{small}");
        // A later notice still carries a changed section whole.
        let mut next = board.clone();
        next.set("Topic 3", "first line of topic 3\nnew second line", "codex", "t");
        let changed = notice(&next, Some(&board), exe).unwrap();
        assert!(changed.contains("new second line"), "{changed}");
    }

    /// Plan 40 D1: files and log lines past the age go; the newest `keep` files and fresh lines stay.
    #[test]
    fn old_files_and_log_lines_are_pruned() {
        let dir = scratch("prune");
        let now = SystemTime::now();
        let old = now - Duration::from_secs(40 * 24 * 60 * 60);
        let mut paths = Vec::new();
        for n in 0..4 {
            let path = dir.join(format!("s{n}.md"));
            std::fs::write(&path, "x").unwrap();
            if n < 3 {
                std::fs::File::options()
                    .write(true)
                    .open(&path)
                    .unwrap()
                    .set_modified(old - Duration::from_secs(n))
                    .unwrap();
            }
            paths.push(path);
        }
        // s3 is new; keep 1 keeps it, and the three old files go.
        assert_eq!(prune_dir(&dir, 1, PRUNE_AFTER, now), 3);
        assert_eq!(
            paths.iter().map(|p| p.exists()).collect::<Vec<_>>(),
            vec![false, false, false, true]
        );
        let log = dir.join("calls.jsonl");
        let ts = |t: SystemTime| {
            time::OffsetDateTime::from(t)
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap()
        };
        let text = format!(
            "{}\n{}\nnot json\n{}\n",
            json!({"ts": ts(old), "n": 1}),
            json!({"ts": ts(old), "n": 2}),
            json!({"ts": ts(now), "n": 3})
        );
        std::fs::write(&log, &text).unwrap();
        assert_eq!(prune_log(&log, PRUNE_AFTER, now), 2);
        let left = std::fs::read_to_string(&log).unwrap();
        assert!(!left.contains("\"n\":1") && left.contains("not json") && left.contains("\"n\":3"));
        // A log whose first line is fresh is not read past that line.
        assert_eq!(prune_log(&log, PRUNE_AFTER, now), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Plan 40 B5: the lock beside a file makes two read-modify-write loops lose nothing.
    #[test]
    fn a_file_lock_keeps_both_writers_lines() {
        let dir = scratch("file-lock");
        let file = dir.join("NIRVANA.md");
        std::fs::write(&file, "head\n").unwrap();
        let writers: Vec<_> = ["a", "b"]
            .into_iter()
            .map(|who| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for n in 0..100 {
                        {
                            let _lock = FileLock::take(&file).expect("lock");
                            let mut text = std::fs::read_to_string(&file).unwrap();
                            text.push_str(&format!("{who}{n}\n"));
                            crate::nirvana_seat::write_atomic(&file, text.as_bytes()).unwrap();
                        }
                        // Out of the lock a moment, as real writers are, so the other's 50 ms poll finds it free.
                        std::thread::sleep(Duration::from_millis(2));
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let text = std::fs::read_to_string(&file).unwrap();
        for who in ["a", "b"] {
            for n in 0..100 {
                assert!(text.lines().any(|l| l == format!("{who}{n}")), "{who}{n} lost");
            }
        }
        assert!(!dir.join("NIRVANA.md.lock").exists(), "the lock goes with the writer");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sharing_adds_one_hook_and_unsharing_leaves_the_file_as_it_was() {
        let exe = Path::new(r"C:\bin\herdr.exe");
        let before = json!({
            "permissions": {"deny": ["WebFetch"]},
            "hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command", "command": "mine.ps1"}]}]}
        });
        let mut local = before.clone();
        assert!(!is_shared(&local));
        set_shared(&mut local, exe, true);
        assert!(is_shared(&local));
        set_shared(&mut local, exe, true);
        assert_eq!(
            local["hooks"]["UserPromptSubmit"].as_array().map(Vec::len),
            Some(2),
            "added once"
        );
        assert_eq!(
            local["hooks"]["UserPromptSubmit"][1]["hooks"][0]["command"],
            json!("\"C:\\bin\\herdr.exe\" board notice")
        );
        set_shared(&mut local, exe, false);
        assert_eq!(local, before);
        let mut bare = json!({});
        set_shared(&mut bare, exe, true);
        set_shared(&mut bare, exe, false);
        assert_eq!(bare, json!({}));
    }

    #[test]
    fn the_square_says_what_is_new_for_this_claude() {
        let board = Board::parse(BOARD);
        let shared = BoardView {
            root: PathBuf::new(),
            board: Some(board.clone()),
            shared: true,
        };
        assert_eq!(
            square_value(&shared, Some(&board)),
            ("board r3".into(), false)
        );
        assert_eq!(
            square_value(&shared, None),
            ("board r3 \u{b7} 2 new".into(), true)
        );
        let unshared = BoardView {
            shared: false,
            ..shared.clone()
        };
        assert_eq!(
            square_value(&unshared, None),
            ("board r3 \u{b7} not shared".into(), false)
        );
        let empty = BoardView {
            board: None,
            ..shared
        };
        assert_eq!(square_value(&empty, None), ("board empty".into(), false));
    }

    #[test]
    fn restart_notice_for_project_a_does_not_fire_when_project_b_mode_changes() {
        let guard = crate::nirvana::with_test_home("p23-notice-proj");
        let home_seats = guard
            .path
            .join("nirvana")
            .join(crate::nirvana_seat::SEATS_FILE);
        std::fs::create_dir_all(home_seats.parent().unwrap()).unwrap();
        std::fs::write(&home_seats, crate::nirvana_seat::DEFAULT).unwrap();

        let dir = std::env::temp_dir().join(format!("nirvana-p23-notice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let proj_a = dir.join("proj_a");
        let proj_b = dir.join("proj_b");
        std::fs::create_dir_all(proj_a.join(".nirvana")).unwrap();
        std::fs::create_dir_all(proj_b.join(".nirvana")).unwrap();

        std::fs::write(
            proj_a
                .join(".nirvana")
                .join(crate::nirvana_seat::SEATS_FILE),
            "mode = \"sota\"\n",
        )
        .unwrap();
        std::fs::write(
            proj_b
                .join(".nirvana")
                .join(crate::nirvana_seat::SEATS_FILE),
            "mode = \"efficient\"\n",
        )
        .unwrap();

        let _ = crate::nirvana_claude::record_session_seat(
            &guard.path,
            "sess-a",
            "claude-opus-5-5",
            "high",
        );

        assert_eq!(
            check_seat_notice("sess-a", Some(&proj_a)),
            None,
            "project A running on SOTA does not notice when in SOTA mode"
        );

        // Change project B's mode
        std::fs::write(
            proj_b
                .join(".nirvana")
                .join(crate::nirvana_seat::SEATS_FILE),
            "mode = \"balanced\"\n",
        )
        .unwrap();

        assert_eq!(
            check_seat_notice("sess-a", Some(&proj_a)),
            None,
            "project A notice must not fire when project B changes mode"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
