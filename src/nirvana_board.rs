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
        let path = root.join(BOARD_DIR).join(LOCK_FILE);
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

/// What a session is told on its next prompt. `seen` is what it saw last; `None` is its first prompt. `None`
/// back means print nothing: the board has not changed for it.
pub fn notice(board: &Board, seen: Option<&Board>, exe: &Path) -> Option<String> {
    let Some(seen) = seen else {
        let mut out = if board.sections.is_empty() {
            "The shared board is empty so far.".to_owned()
        } else {
            let sections: Vec<_> = board.sections.iter().map(section_text).collect();
            format!(
                "Shared board, revision {}:\n{}",
                board.revision,
                sections.join("\n")
            )
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
    let mut local = crate::nirvana_preload::read_local(cwd)?;
    set_shared(&mut local, &exe(), on);
    crate::nirvana_preload::write_local(cwd, &local)?;
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
    let Some(root) = cwd.as_deref().and_then(root) else {
        return;
    };
    let board = read(&root).unwrap_or_default();
    let seen = seen(session);
    if let Some(text) = notice(&board, seen.as_ref(), &exe()) {
        println!("{text}");
    }
    remember(session, &board);
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
}
