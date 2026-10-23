//! Nirvana fork, plan 27: an idle Claude compacts before its prompt cache goes stale.
//!
//! The owner, 2026-10-23: "Ensusre idle agents compact when cache is about to get stale. This should be a setting
//! for claude code." Claude Code writes its prompt cache with a one-hour life: every non-zero cache write in the five
//! newest transcripts on this machine was `ephemeral_1h_input_tokens` (2026-10-23). A Claude left idle past the hour
//! writes its whole context again on the owner's next turn; `/compact` sent at 50 minutes still reads that context
//! from cache and leaves a small one.
//!
//! The rule is one pure function, `decide`, over the facts in `Facts`. The server (`app/nirvana_idle_compact.rs`)
//! gathers the facts it holds itself (state, idle time, the screen, the owner's last key) and the ones outside herdr
//! (`read_outside`: the Claude process, its transcript), then pastes `/compact` only on `Decision::Send`, and presses
//! Enter only if the line then reads exactly `/compact` (`enter_gate`). Any fact it cannot read is a reason not to
//! send: this types into the owner's own panes, and typing into a pane the owner is using is the worst outcome.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::config::IdleCompactRule;
use crate::detect::{Agent, AgentState};

/// The life of Claude Code's prompt cache (measured above). Past it a compact no longer reads from cache, so the
/// stretch is not checked again.
pub const CACHE_LIFE: Duration = Duration::from_secs(60 * 60);
/// A pane checked and left alone is checked again this much later in the same stretch: the owner may clear a
/// half-typed line, or a background write may age past the limit.
pub const RECHECK: Duration = Duration::from_secs(60);
/// What is typed into the pane.
pub const COMMAND: &str = "/compact";
/// The most of a transcript read for its last turn's context (review S4: bounded work on the server loop). A last
/// assistant line further back than this from the end gives no reading, so nothing is sent.
pub const MAX_TAIL: u64 = 1024 * 1024;

/// One unbroken idle stretch of a pane: when it began, when it was last checked, whether `/compact` was pasted in it,
/// and when its Enter is due.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdleStretch {
    pub since: Option<Instant>,
    pub checked_at: Option<Instant>,
    pub sent: bool,
    /// Review S1: the paste went; at this instant the line is read again and Enter goes only if it reads `/compact`.
    pub enter_at: Option<Instant>,
}

impl IdleStretch {
    /// The stretch after the pane's state is recomputed (plan 27 §2 rules 3 and 7). `Idle` keeps the stretch it is
    /// in, or starts one now; any other state ends it, so the next `Idle` starts afresh with nothing sent.
    pub fn after(self, state: AgentState, now: Instant) -> Self {
        if state != AgentState::Idle {
            return Self::default();
        }
        if self.since.is_some() {
            return self;
        }
        Self {
            since: Some(now),
            ..Self::default()
        }
    }

    /// When the stretch is next due a check, or `None` once `/compact` went or the cache has expired. A pending
    /// Enter comes first. A check that left the pane alone waits `RECHECK`, so a skipped pane never makes the server
    /// loop spin. No `Instant` sum can overflow here: one that would is no check (review S5).
    pub fn next_check_at(self, minutes: Duration) -> Option<Instant> {
        if let Some(enter_at) = self.enter_at {
            return Some(enter_at);
        }
        let since = self.since?;
        if self.sent {
            return None;
        }
        let first = since.checked_add(minutes)?;
        let at = match self.checked_at {
            Some(checked) => first.max(checked.checked_add(RECHECK)?),
            None => first,
        };
        (at < since.checked_add(CACHE_LIFE)?).then_some(at)
    }
}

/// What the strict reading of Claude Code's input line found (review B1, B2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputLine {
    /// Provably empty: the real input box, nothing on its `❯` line but faint placeholder text, the cursor at the
    /// line's start.
    Empty,
    /// Text in the real input box, faint text left out.
    Text(String),
    /// Not provably either; why.
    Unsure(String),
}

/// The pane's cursor, in screen rows and columns from the top left (`TerminalRuntime::cursor_state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
    pub visible: bool,
}

/// Everything `decide` reads about one pane. `None` means "could not be read", unless a field says otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The setting, `None` when it is off or its numbers are out of range.
    pub rule: Option<IdleCompactRule>,
    /// The agent herdr sees in the pane.
    pub agent: Option<Agent>,
    /// The pane is a `herdr seat exec` worker's.
    pub seat_worker: bool,
    /// The Claude process's argv.
    pub argv: Option<Vec<String>>,
    pub state: AgentState,
    /// How long the pane has been `Idle` without a break.
    pub idle_for: Option<Duration>,
    /// `/compact` already went in this idle stretch.
    pub sent_in_stretch: bool,
    /// How long ago the pane's own transcript was last written; `None` when `choose_transcript` found none.
    pub transcript_age: Option<Duration>,
    /// The last assistant turn's context (`context_tokens`).
    pub context_tokens: Option<u64>,
    /// Seconds since the owner's last key in this pane (`nirvana_run::owner_key_at`); `None` when none is recorded.
    pub owner_key_secs_ago: Option<u64>,
    /// The strict reading of the input line (`read_input_line`).
    pub input_line: InputLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Send,
    Skip(String),
}

/// Plan 27 §2: send `/compact` only when every rule holds. The first rule that fails is the reason.
pub fn decide(facts: &Facts) -> Decision {
    let skip = |reason: String| Decision::Skip(reason);
    // 1. The setting is on.
    let Some(rule) = facts.rule else {
        return skip("idle compact is off".into());
    };
    // 2. Claude, and not a worker.
    if facts.agent != Some(Agent::Claude) {
        return skip("the pane is not Claude".into());
    }
    if facts.seat_worker {
        return skip("the pane is a seat worker's".into());
    }
    let Some(argv) = facts.argv.as_deref() else {
        return skip("Claude's process could not be read".into());
    };
    if is_headless(argv) {
        return skip("Claude runs headless (-p)".into());
    }
    // 3. Idle without a break for at least `minutes`.
    if facts.state != AgentState::Idle {
        return skip(format!("the pane is {:?}, not idle", facts.state));
    }
    let minutes = rule.minutes.as_secs() / 60;
    if facts.idle_for.is_none_or(|idle| idle < rule.minutes) {
        return skip(format!("idle under {minutes} min"));
    }
    // 7. Once per idle stretch.
    if facts.sent_in_stretch {
        return skip("already compacted in this idle stretch".into());
    }
    // 4. The pane's own transcript, last written at least `minutes` ago.
    let Some(age) = facts.transcript_age else {
        return skip("the pane's own transcript was not found".into());
    };
    if age < rule.minutes {
        return skip(format!(
            "the transcript was written under {minutes} min ago"
        ));
    }
    // 5. A context worth compacting.
    let Some(tokens) = facts.context_tokens else {
        return skip("the transcript holds no context reading".into());
    };
    if tokens < rule.min_tokens {
        return skip(format!("context {tokens} is under {}", rule.min_tokens));
    }
    // 6. The owner is not at the pane (review S1), and nothing is on the input line.
    if let Some(why) = owner_quiet_breach(facts.owner_key_secs_ago) {
        return skip(why);
    }
    match &facts.input_line {
        InputLine::Empty => Decision::Send,
        InputLine::Text(_) => skip("the input line holds text".into()),
        InputLine::Unsure(why) => skip(format!("the input line is not provably empty: {why}")),
    }
}

/// Why the owner's last key is too recent, as the orchestrator's wake path reads it (`nirvana_run`, `OWNER_QUIET_S`).
fn owner_quiet_breach(owner_key_secs_ago: Option<u64>) -> Option<String> {
    let ago = owner_key_secs_ago?;
    (ago < crate::nirvana_run::OWNER_QUIET_S)
        .then(|| format!("the owner typed in the pane {ago} s ago"))
}

/// Review S1: the Enter after the paste goes only when the line reads exactly `/compact` and the owner has not
/// typed since. `Err` is why it was left unsent.
pub fn enter_gate(line: &InputLine, owner_key_secs_ago: Option<u64>) -> Result<(), String> {
    if let Some(why) = owner_quiet_breach(owner_key_secs_ago) {
        return Err(why);
    }
    match line {
        InputLine::Text(text) if text == COMMAND => Ok(()),
        other => Err(format!("the input line reads {other:?}, not {COMMAND}")),
    }
}

/// A worker's Claude runs `-p`/`--print` and ends when done; it is never compacted.
pub fn is_headless(argv: &[String]) -> bool {
    argv.iter().any(|arg| arg == "-p" || arg == "--print")
}

// ---- the input line (review B1, B2) ----

/// Screens on which Enter would answer something other than the input line, lower case: the markers of the blocked
/// and transcript-viewer rules in `detect/manifests/claude.toml` (`transcript_viewer`, `live_blocked_form`,
/// `dynamic_workflow_prompt`, `mcp_elicitation_prompt`, `model_picker_menu`, `bash_permission_prompt`,
/// `generic_permission_prompt`, `legacy_no_prompt_blocker`), then vim mode's footer. The legacy rule's loose
/// phrases ("do you want to", "would you like to") are kept too, though Claude's own replies use them: a doubt sends
/// nothing.
const NOT_THE_INPUT: &[&str] = &[
    "showing detailed transcript",
    "esc to cancel",
    "enter to confirm",
    "enter to select",
    "arrow keys to navigate",
    "arrows to navigate",
    "↑/↓ to navigate",
    "↑↓ to navigate",
    "↑↓ scroll",
    "run a dynamic workflow?",
    "requests your input",
    "select model",
    "enter to set as default",
    "do you want to proceed?",
    "waiting for permission",
    "do you want to allow this connection?",
    "tab to amend",
    "ctrl+e to explain",
    "review your answers",
    "skip interview and plan immediately",
    "do you want to",
    "would you like to",
    "-- normal --",
    "-- insert --",
];

/// The most non-empty lines Claude draws under its input box (its footer, a short command menu). More, and the box
/// is not the live bottom of the screen: a viewport off the bottom shows an older frame's box with newer lines below.
const FOOTER_LINES: usize = 12;

/// One cell of a screen line: its character and whether it is drawn faint or inverse.
#[derive(Debug, Clone, Copy)]
struct Cell {
    ch: char,
    faint: bool,
    inverse: bool,
}

/// Claude Code's input line, read strictly from the pane's screen with its colours (`visible_ansi`) and its cursor.
///
/// Claude Code 2.1.302, recorded in a herdr pane on 2026-10-18 (`nirvana_run/fixtures/claude-2.1.302-*.ansi`): the
/// input box is the body between the screen's last two horizontal rules, one line `❯`, a no-break space, then the
/// text; an empty line shows a placeholder drawn faint (`ESC[2m`). So: empty only when that box is the last pair of
/// rules near the bottom, its one line starts `❯`, everything after the mark is faint (SGR 2; a grey colour does not
/// count), no `[` chip is on it, no cursor is drawn past its start, and the terminal's cursor, when shown, sits at
/// its start. The line is never looked for anywhere else (no fallback to a `❯` or `>` line in a menu or a picker),
/// and a screen with a dialog, the transcript viewer or vim mode is not read at all. `nirvana_run::prompt_line_text`
/// stays as the wake path's own reading.
pub fn read_input_line(screen: &str, cursor: Option<Cursor>, scrolled: bool) -> InputLine {
    let unsure = |why: &str| InputLine::Unsure(why.to_owned());
    if scrolled {
        return unsure("the pane is scrolled back");
    }
    let lines: Vec<Vec<Cell>> = screen.lines().map(cells).collect();
    let plain: Vec<String> = lines
        .iter()
        .map(|cells| cells.iter().map(|cell| cell.ch).collect())
        .collect();
    let lower = plain.join("\n").to_lowercase();
    if let Some(marker) = NOT_THE_INPUT.iter().find(|marker| lower.contains(**marker)) {
        return InputLine::Unsure(format!("the screen shows {marker:?}"));
    }
    let rules: Vec<usize> = plain
        .iter()
        .enumerate()
        .filter(|(_, line)| is_rule(line))
        .map(|(index, _)| index)
        .collect();
    let [.., top, bottom] = rules[..] else {
        return unsure("no input box on the screen");
    };
    let below = plain[bottom + 1..]
        .iter()
        .filter(|line| !line.trim().is_empty())
        .count();
    if below > FOOTER_LINES {
        return unsure("the input box is not at the bottom of the screen");
    }
    if bottom != top + 2 {
        return unsure("the input box holds more than one line");
    }
    let row = top + 1;
    let line = &lines[row];
    let marked = line.first().is_some_and(|cell| cell.ch == '❯')
        && line
            .get(1)
            .is_some_and(|cell| matches!(cell.ch, ' ' | '\u{a0}'));
    if !marked {
        return unsure("the input box's line does not start with ❯");
    }
    let rest = &line[2..];
    let owned: String = rest
        .iter()
        .filter(|cell| !cell.faint)
        .map(|cell| cell.ch)
        .collect();
    let owned = owned.trim();
    if !owned.is_empty() {
        return InputLine::Text(owned.to_owned());
    }
    if rest.iter().any(|cell| cell.ch == '[') {
        return unsure("a [ chip is on the input line");
    }
    if rest.iter().skip(1).any(|cell| cell.inverse) {
        return unsure("a cursor is drawn past the input line's start");
    }
    match cursor {
        None => unsure("the cursor could not be read"),
        Some(cursor) if cursor.visible && (cursor.row != row || cursor.col != 2) => {
            unsure("the cursor is not at the input line's start")
        }
        Some(_) => InputLine::Empty,
    }
}

/// A line of `─` only, the input box's border.
fn is_rule(line: &str) -> bool {
    let line = line.trim();
    line.chars().count() >= 10 && line.chars().all(|c| c == '─')
}

/// The cells of a line with SGR escapes, each with the faint and inverse it is drawn with. Other escapes are dropped.
fn cells(line: &str) -> Vec<Cell> {
    let mut cells = Vec::new();
    let (mut faint, mut inverse) = (false, false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            cells.push(Cell {
                ch: c,
                faint,
                inverse,
            });
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut end = None;
        for c in chars.by_ref() {
            if c.is_ascii_alphabetic() || c == '~' {
                end = Some(c);
                break;
            }
            params.push(c);
        }
        if end == Some('m') {
            apply_sgr(&params, &mut faint, &mut inverse);
        }
    }
    cells
}

/// Faint (2 on, 22 or 0 off) and inverse (7 on, 27 or 0 off) after one SGR. The arguments of an extended colour
/// (`38;5;n`, `38;2;r;g;b`, and the same for 48 and 58) are skipped, so the `2` in `48;2;0;0;0` is not read as faint;
/// a colour written with colons is one group and skipped whole.
fn apply_sgr(params: &str, faint: &mut bool, inverse: &mut bool) {
    if params.is_empty() {
        *faint = false;
        *inverse = false;
        return;
    }
    let mut groups = params.split(';');
    while let Some(group) = groups.next() {
        if group.contains(':') {
            continue;
        }
        match group.parse::<u32>().unwrap_or(u32::MAX) {
            0 => {
                *faint = false;
                *inverse = false;
            }
            2 => *faint = true,
            22 => *faint = false,
            7 => *inverse = true,
            27 => *inverse = false,
            38 | 48 | 58 => match groups.next() {
                Some("5") => {
                    groups.next();
                }
                Some("2") => {
                    groups.nth(2);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

// ---- the transcript (review S3, S4) ----

/// The context a turn read: `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`. Unlike
/// `nirvana_subagents::parse_claude_usage` (a turn's whole traffic, for the agents panel), `output_tokens` is left
/// out: it is what the turn wrote, not what the next one re-reads. `None` when none of the three is there.
pub fn context_tokens(usage: &Value) -> Option<u64> {
    let fields = [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ];
    let read: Vec<u64> = fields
        .iter()
        .filter_map(|field| usage.get(field).and_then(Value::as_u64))
        .collect();
    (!read.is_empty()).then(|| read.iter().sum())
}

/// The context of the last assistant line in a transcript's text.
pub fn last_context_tokens(text: &str) -> Option<u64> {
    text.lines().rev().find_map(|line| {
        let value: Value = serde_json::from_str(line.trim()).ok()?;
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            return None;
        }
        Some(context_tokens(value.pointer("/message/usage")?))
    })?
}

/// The last assistant turn's context, read from the file's tail, widened from 64 KB up to `MAX_TAIL` (the agents
/// panel widens to 8 MB, `nirvana_subagents::read_transcript_tail_with_expansion`; this runs on the server loop).
pub fn read_context_tokens(path: &Path) -> Option<u64> {
    use crate::nirvana_subagents::{read_file_tail, INITIAL_TAIL_WINDOW};
    let len = std::fs::metadata(path).ok()?.len();
    let mut window = INITIAL_TAIL_WINDOW.min(MAX_TAIL);
    loop {
        let text = read_file_tail(path, window).ok()?;
        let found = last_context_tokens(&text);
        if found.is_some() || window >= MAX_TAIL || len <= window {
            return found;
        }
        window = (window * 4).min(MAX_TAIL);
    }
}

/// How long ago the file was last written. `None` when it cannot be read or its time is ahead of the clock.
pub fn file_age(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified).ok()
}

/// A file's last write, in Unix seconds; `None` when it is not a file.
fn written_secs(path: &Path) -> Option<u64> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    Some(crate::nirvana_usage::unix(meta.modified().ok()?))
}

/// Claude's own record of its conversation, `<config>/sessions/<pid>.json` (`nirvana::read_claude_session`), and
/// when that file was last written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionsRecord {
    pub session_id: String,
    pub written_secs: Option<u64>,
}

/// Review S3: the pane's own transcript, or why there is none. Pure over its arguments; `written` gives a file's
/// last write in Unix seconds (`None`: no such file).
///
/// - The pane's foreground must be Claude: after the owner quits Claude the pane is a shell, and nothing is sent.
/// - An id counts only when it is tied to this Claude process by its start time: Claude's sessions record when it
///   was written since the process started (a record left by an earlier process with a reused pid is older), else
///   the hook's id when its transcript was written since then. The record comes first: the hook's id can be stale
///   after `/clear`. Without the start time (`process_creation_secs` is known on Windows only) nothing is tied, so
///   nothing is sent.
/// - No id: nothing. The folder's newest transcript is never taken here (the agents panel still takes it,
///   `nirvana_subagents::resolve_claude_session`), because it may be another conversation's.
/// - Another Claude pane in the same folder with the same id, or with none, means the two cannot be told apart.
#[allow(clippy::too_many_arguments)]
pub fn choose_transcript(
    foreground: Option<Agent>,
    record: Option<&SessionsRecord>,
    start_secs: Option<u64>,
    hook_session: Option<&str>,
    others: &[(&str, &Path, Option<&str>)],
    folder: &Path,
    claude_dir: &Path,
    written: impl Fn(&Path) -> Option<u64>,
) -> Result<PathBuf, String> {
    if foreground != Some(Agent::Claude) {
        return Err("the pane's foreground is not Claude".into());
    }
    let Some(start) = start_secs else {
        return Err("the Claude process's start time is not known".into());
    };
    let encoded = crate::nirvana_subagents::encode_project_folder(folder);
    let from_record = record
        .filter(|record| record.written_secs.is_some_and(|at| at >= start))
        .map(|record| record.session_id.as_str());
    let (id, from_hook) = match (from_record, hook_session) {
        (Some(id), _) => (id, false),
        (None, Some(id)) => (id, true),
        (None, None) => return Err("no session id names this Claude's conversation".into()),
    };
    if others.iter().any(|(_, other_folder, other_id)| {
        crate::nirvana_subagents::encode_project_folder(other_folder) == encoded
            && other_id.is_none_or(|other| other == id)
    }) {
        return Err("another Claude pane in the folder cannot be told apart".into());
    }
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid {
        return Err(format!("the session id {id:?} is not an id"));
    }
    let path = claude_dir
        .join("projects")
        .join(&encoded)
        .join(format!("{id}.jsonl"));
    match written(&path) {
        None => Err(format!("no transcript for session {id}")),
        Some(at) if from_hook && at < start => Err(format!(
            "the hook's session {id} was last written before this Claude started"
        )),
        Some(_) => Ok(path),
    }
}

/// What the server knows about a Claude pane for the reads outside herdr.
#[derive(Debug, Clone, Default)]
pub struct PaneQuery {
    pub pane_id: String,
    pub shell_pid: Option<u32>,
    pub folder: Option<PathBuf>,
    /// The conversation id Claude's hooks reported to herdr.
    pub hook_session: Option<String>,
    /// The other Claude panes: id, folder, hook session.
    pub others: Vec<(String, PathBuf, Option<String>)>,
}

/// The facts that live outside herdr.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outside {
    pub argv: Option<Vec<String>>,
    pub transcript_age: Option<Duration>,
    pub context_tokens: Option<u64>,
}

/// Reads the pane's foreground job and the Claude process in it, then its own transcript (`choose_transcript`).
/// Stops at the first read that fails, leaving the rest `None`.
pub fn read_outside(query: &PaneQuery, claude_dir: Option<&Path>, now: SystemTime) -> Outside {
    let Some(job) = query.shell_pid.and_then(crate::detect::foreground_job) else {
        return Outside::default();
    };
    let found = crate::detect::identify_agent_process_in_job(&job);
    let foreground = found.as_ref().map(|(agent, _, _)| *agent);
    let Some((Agent::Claude, _, claude_pid)) = found else {
        return Outside::default();
    };
    let argv = job
        .processes
        .iter()
        .find(|process| process.pid == claude_pid)
        .and_then(|process| process.argv.clone());
    let mut outside = Outside {
        argv,
        ..Outside::default()
    };
    let (Some(claude_dir), Some(folder)) = (claude_dir, query.folder.as_deref()) else {
        return outside;
    };
    let record =
        crate::nirvana::read_claude_session(claude_dir, claude_pid).map(|session| SessionsRecord {
            session_id: session.session_id,
            written_secs: written_secs(
                &claude_dir
                    .join("sessions")
                    .join(format!("{claude_pid}.json")),
            ),
        });
    let others: Vec<(&str, &Path, Option<&str>)> = query
        .others
        .iter()
        .map(|(id, folder, session)| (id.as_str(), folder.as_path(), session.as_deref()))
        .collect();
    let transcript = match choose_transcript(
        foreground,
        record.as_ref(),
        crate::nirvana_subagents::process_creation_secs(claude_pid),
        query.hook_session.as_deref(),
        &others,
        folder,
        claude_dir,
        written_secs,
    ) {
        Ok(transcript) => transcript,
        Err(why) => {
            tracing::debug!(pane = %query.pane_id, why, "nirvana: idle compact found no transcript");
            return outside;
        }
    };
    outside.transcript_age = file_age(&transcript, now);
    outside.context_tokens = read_context_tokens(&transcript);
    outside
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    fn rule() -> IdleCompactRule {
        IdleCompactRule {
            minutes: 50 * MIN,
            min_tokens: 50_000,
        }
    }

    /// Every rule holds: the one case that sends. Each test below breaks one fact of it.
    fn sending() -> Facts {
        Facts {
            rule: Some(rule()),
            agent: Some(Agent::Claude),
            seat_worker: false,
            argv: Some(vec!["claude".into(), "--model".into(), "opus".into()]),
            state: AgentState::Idle,
            idle_for: Some(51 * MIN),
            sent_in_stretch: false,
            transcript_age: Some(51 * MIN),
            context_tokens: Some(80_000),
            owner_key_secs_ago: None,
            input_line: InputLine::Empty,
        }
    }

    fn skipped(facts: &Facts, why: &str) {
        match decide(facts) {
            Decision::Skip(reason) => assert!(reason.contains(why), "{reason:?} lacks {why:?}"),
            Decision::Send => panic!("sent, expected a skip for {why:?}: {facts:?}"),
        }
    }

    #[test]
    fn idle_compact_sends_when_every_rule_holds() {
        assert_eq!(decide(&sending()), Decision::Send);
        // At exactly the limits it still sends: "at least" in §2 rules 3 to 5, and a key just outside the quiet.
        let facts = Facts {
            idle_for: Some(50 * MIN),
            transcript_age: Some(50 * MIN),
            context_tokens: Some(50_000),
            owner_key_secs_ago: Some(crate::nirvana_run::OWNER_QUIET_S),
            ..sending()
        };
        assert_eq!(decide(&facts), Decision::Send);
    }

    #[test]
    fn idle_compact_rule1_setting_off_sends_nothing() {
        skipped(
            &Facts {
                rule: None,
                ..sending()
            },
            "off",
        );
    }

    #[test]
    fn idle_compact_rule2_only_claude() {
        for agent in [None, Some(Agent::Codex), Some(Agent::Antigravity)] {
            skipped(&Facts { agent, ..sending() }, "not Claude");
        }
    }

    #[test]
    fn idle_compact_rule2_seat_worker_pane_left_alone() {
        skipped(
            &Facts {
                seat_worker: true,
                ..sending()
            },
            "seat worker",
        );
    }

    #[test]
    fn idle_compact_rule2_unread_process_sends_nothing() {
        skipped(
            &Facts {
                argv: None,
                ..sending()
            },
            "could not be read",
        );
    }

    #[test]
    fn idle_compact_rule2_headless_claude_left_alone() {
        for flag in ["-p", "--print"] {
            let argv = vec!["claude".into(), flag.into(), "do it".into()];
            skipped(
                &Facts {
                    argv: Some(argv),
                    ..sending()
                },
                "headless",
            );
        }
    }

    #[test]
    fn idle_compact_rule3_only_an_idle_pane() {
        for state in [
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Unknown,
        ] {
            skipped(&Facts { state, ..sending() }, "not idle");
        }
    }

    #[test]
    fn idle_compact_rule3_idle_under_minutes_waits() {
        for idle_for in [None, Some(49 * MIN)] {
            skipped(
                &Facts {
                    idle_for,
                    ..sending()
                },
                "idle under 50 min",
            );
        }
    }

    #[test]
    fn idle_compact_rule4_transcript_must_resolve() {
        skipped(
            &Facts {
                transcript_age: None,
                ..sending()
            },
            "own transcript was not found",
        );
    }

    #[test]
    fn idle_compact_rule4_recent_transcript_waits() {
        skipped(
            &Facts {
                transcript_age: Some(10 * MIN),
                ..sending()
            },
            "written under 50 min",
        );
    }

    #[test]
    fn idle_compact_rule5_context_must_be_read() {
        skipped(
            &Facts {
                context_tokens: None,
                ..sending()
            },
            "no context",
        );
    }

    #[test]
    fn idle_compact_rule5_small_context_left_alone() {
        skipped(
            &Facts {
                context_tokens: Some(49_999),
                ..sending()
            },
            "under 50000",
        );
    }

    #[test]
    fn idle_compact_rule6_no_input_line_sends_nothing() {
        skipped(
            &Facts {
                input_line: InputLine::Unsure("no input box on the screen".into()),
                ..sending()
            },
            "not provably empty",
        );
    }

    #[test]
    fn idle_compact_rule6_typed_text_left_alone() {
        skipped(
            &Facts {
                input_line: InputLine::Text("half a thought".into()),
                ..sending()
            },
            "holds text",
        );
    }

    /// Review S1: a key inside `OWNER_QUIET_S` means the owner is at the pane.
    #[test]
    fn idle_compact_rule6_owner_keys_inside_the_quiet_window_skip() {
        for ago in [0, crate::nirvana_run::OWNER_QUIET_S - 1] {
            skipped(
                &Facts {
                    owner_key_secs_ago: Some(ago),
                    ..sending()
                },
                "the owner typed",
            );
        }
    }

    #[test]
    fn idle_compact_rule7_once_per_stretch() {
        skipped(
            &Facts {
                sent_in_stretch: true,
                ..sending()
            },
            "already compacted",
        );
    }

    /// Review S1: Enter goes only on a line that reads exactly `/compact`, with the owner away.
    #[test]
    fn idle_compact_enter_only_on_exactly_compact() {
        assert_eq!(
            enter_gate(&InputLine::Text("/compact".into()), None),
            Ok(())
        );
        for line in [
            InputLine::Text("/compact hello".into()),
            InputLine::Text("/compac".into()),
            InputLine::Text("hello/compact".into()),
            InputLine::Empty,
            InputLine::Unsure("no input box on the screen".into()),
        ] {
            assert!(enter_gate(&line, None).is_err(), "{line:?}");
        }
        assert!(enter_gate(&InputLine::Text("/compact".into()), Some(0))
            .is_err_and(|why| why.contains("the owner typed")));
    }

    // ---- the strict input line, on the recorded screens and ones built from them ----

    const RECORDED_EMPTY: &str = include_str!("nirvana_run/fixtures/claude-2.1.302-empty.ansi");
    const RECORDED_TYPED: &str = include_str!("nirvana_run/fixtures/claude-2.1.302-typed.ansi");

    /// The cursor where an empty line has it: on the `❯` row, just after the mark.
    fn at_start(screen: &str) -> Option<Cursor> {
        let row = screen.lines().position(|line| line.starts_with('❯'))?;
        Some(Cursor {
            row,
            col: 2,
            visible: true,
        })
    }

    fn read(screen: &str) -> InputLine {
        read_input_line(screen, at_start(screen), false)
    }

    fn not_empty(screen: &str, why: &str) {
        match read(screen) {
            InputLine::Empty => panic!("read as empty, expected {why:?}"),
            InputLine::Text(text) => {
                assert!(why.is_empty(), "read text {text:?}, expected {why:?}")
            }
            InputLine::Unsure(reason) => assert!(reason.contains(why), "{reason:?} lacks {why:?}"),
        }
    }

    #[test]
    fn idle_compact_input_line_recorded_empty_reads_empty() {
        assert_eq!(read(RECORDED_EMPTY), InputLine::Empty);
        // The terminal's cursor hidden: the drawn line alone decides.
        let hidden = Cursor {
            row: 0,
            col: 0,
            visible: false,
        };
        assert_eq!(
            read_input_line(RECORDED_EMPTY, Some(hidden), false),
            InputLine::Empty
        );
    }

    #[test]
    fn idle_compact_input_line_recorded_typed_reads_its_text() {
        assert_eq!(
            read(RECORDED_TYPED),
            InputLine::Text("hello owner draft".into())
        );
    }

    /// B1: a faint pasted-text chip is not a placeholder.
    #[test]
    fn idle_compact_input_line_pasted_chip_is_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/pasted-chip.ansi"),
            "chip",
        );
    }

    /// B1: a faint image chip is not a placeholder.
    #[test]
    fn idle_compact_input_line_image_chip_is_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/image-chip.ansi"),
            "chip",
        );
    }

    /// B1: grey text is the owner's; only faint text is a placeholder.
    #[test]
    fn idle_compact_input_line_grey_typed_text_is_not_empty() {
        assert_eq!(
            read(include_str!(
                "nirvana_idle_compact/fixtures/grey-typed.ansi"
            )),
            InputLine::Text("hello owner draft".into())
        );
        // The `2` of a background colour (`48;2;r;g;b`) is not faint.
        let screen = RECORDED_TYPED.replace(
            "❯\u{a0}hello owner draft",
            "❯\u{a0}\x1b[48;2;30;30;30mhello owner draft",
        );
        assert_eq!(read(&screen), InputLine::Text("hello owner draft".into()));
    }

    /// B1: a draft of two lines, whether its text is on the first or the second.
    #[test]
    fn idle_compact_input_line_multiline_draft_is_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/multiline-last-empty.ansi"),
            "more than one line",
        );
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/multiline-first-empty.ansi"),
            "more than one line",
        );
    }

    /// B1: a cursor drawn (inverse) past the line's start, or the terminal's cursor elsewhere, is not empty.
    #[test]
    fn idle_compact_input_line_cursor_away_from_the_start_is_not_empty() {
        let drawn = RECORDED_EMPTY.replace(
            "\x1b[2mTry \"fix lint errors\"",
            "\x1b[2mTry \"fix\x1b[7m \x1b[27mlint errors\"",
        );
        not_empty(&drawn, "cursor is drawn");
        let mut cursor = at_start(RECORDED_EMPTY).unwrap();
        cursor.col = 9;
        assert!(matches!(
            read_input_line(RECORDED_EMPTY, Some(cursor), false),
            InputLine::Unsure(why) if why.contains("not at the input line's start")
        ));
        cursor.col = 2;
        cursor.row += 3;
        assert!(matches!(
            read_input_line(RECORDED_EMPTY, Some(cursor), false),
            InputLine::Unsure(why) if why.contains("not at the input line's start")
        ));
        assert!(matches!(
            read_input_line(RECORDED_EMPTY, None, false),
            InputLine::Unsure(why) if why.contains("could not be read")
        ));
    }

    /// B2: a menu in place of the box.
    #[test]
    fn idle_compact_input_line_menu_is_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/menu.ansi"),
            "the screen shows",
        );
    }

    /// B2: a `❯` line with no box around it is never read as the input line.
    #[test]
    fn idle_compact_input_line_needs_the_box() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/no-box.ansi"),
            "no input box",
        );
        // The box's line must start with `❯`: an older layout's `>` is not read.
        let older = RECORDED_EMPTY.replace("❯\u{a0}", ">\u{a0}");
        let cursor = at_start(RECORDED_EMPTY);
        assert!(matches!(
            read_input_line(&older, cursor, false),
            InputLine::Unsure(why) if why.contains("does not start with ❯")
        ));
    }

    /// B2: a screen whose box is not at the bottom, and a pane scrolled back.
    #[test]
    fn idle_compact_input_line_scrolled_screen_is_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/scrolled.ansi"),
            "not at the bottom",
        );
        assert!(matches!(
            read_input_line(RECORDED_EMPTY, at_start(RECORDED_EMPTY), true),
            InputLine::Unsure(why) if why.contains("scrolled back")
        ));
    }

    /// B2: vim mode, and a dialog's marker over an empty-looking box.
    #[test]
    fn idle_compact_input_line_vim_and_dialog_markers_are_not_empty() {
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/vim-insert.ansi"),
            "-- insert --",
        );
        not_empty(
            include_str!("nirvana_idle_compact/fixtures/dialog-marker.ansi"),
            "esc to cancel",
        );
    }

    #[test]
    fn idle_compact_input_line_reads_compact_for_the_enter() {
        let screen = RECORDED_TYPED.replace("hello owner draft", "/compact");
        assert_eq!(read(&screen), InputLine::Text("/compact".into()));
        // A faint hint after it is Claude's, not the owner's.
        let hinted =
            RECORDED_TYPED.replace("hello owner draft", "/compact\x1b[2m [instructions]\x1b[0m");
        assert_eq!(read(&hinted), InputLine::Text("/compact".into()));
    }

    // ---- the stretch ----

    #[test]
    fn idle_stretch_starts_on_idle_and_keeps_its_start() {
        let t0 = Instant::now();
        let stretch = IdleStretch::default().after(AgentState::Idle, t0);
        assert_eq!(stretch.since, Some(t0));
        // A later recompute that is still idle keeps the first instant, so the idle time is unbroken.
        let later = stretch.after(AgentState::Idle, t0 + 10 * MIN);
        assert_eq!(later.since, Some(t0));
    }

    #[test]
    fn idle_stretch_ends_on_any_other_state() {
        let t0 = Instant::now();
        let sent = IdleStretch {
            since: Some(t0),
            checked_at: Some(t0 + 50 * MIN),
            sent: true,
            enter_at: Some(t0 + 50 * MIN),
        };
        for state in [
            AgentState::Working,
            AgentState::Blocked,
            AgentState::Unknown,
        ] {
            assert_eq!(sent.after(state, t0 + 51 * MIN), IdleStretch::default());
        }
        // The next idle is a fresh stretch with nothing sent.
        let fresh = sent
            .after(AgentState::Working, t0 + 51 * MIN)
            .after(AgentState::Idle, t0 + 52 * MIN);
        assert_eq!(fresh.since, Some(t0 + 52 * MIN));
        assert!(!fresh.sent);
    }

    #[test]
    fn idle_stretch_is_due_at_minutes_then_rechecks_until_the_cache_expires() {
        let t0 = Instant::now();
        let stretch = IdleStretch {
            since: Some(t0),
            ..IdleStretch::default()
        };
        assert_eq!(stretch.next_check_at(50 * MIN), Some(t0 + 50 * MIN));
        // Checked and left alone: a minute later, not at once (no spinning).
        let checked = IdleStretch {
            checked_at: Some(t0 + 50 * MIN),
            ..stretch
        };
        assert_eq!(checked.next_check_at(50 * MIN), Some(t0 + 51 * MIN));
        // A check due at or past the cache's hour never comes.
        let late = IdleStretch {
            checked_at: Some(t0 + 59 * MIN),
            ..stretch
        };
        assert_eq!(late.next_check_at(50 * MIN), None);
        // Once sent, never again in the stretch.
        let sent = IdleStretch {
            sent: true,
            ..stretch
        };
        assert_eq!(sent.next_check_at(50 * MIN), None);
        // Not idle: never due.
        assert_eq!(IdleStretch::default().next_check_at(50 * MIN), None);
    }

    /// Review S1: a pasted `/compact` is due its Enter, sent or not.
    #[test]
    fn idle_stretch_is_due_its_enter_after_the_paste() {
        let t0 = Instant::now();
        let pasted = IdleStretch {
            since: Some(t0),
            checked_at: Some(t0 + 50 * MIN),
            sent: true,
            enter_at: Some(t0 + 50 * MIN + Duration::from_millis(300)),
        };
        assert_eq!(
            pasted.next_check_at(50 * MIN),
            Some(t0 + 50 * MIN + Duration::from_millis(300))
        );
    }

    // ---- the transcript ----

    #[test]
    fn context_tokens_leave_out_output() {
        let usage = serde_json::json!({
            "input_tokens": 10,
            "cache_read_input_tokens": 60_000,
            "cache_creation_input_tokens": 500,
            "output_tokens": 9_000,
        });
        assert_eq!(context_tokens(&usage), Some(60_510));
        assert_eq!(
            context_tokens(&serde_json::json!({"output_tokens": 5})),
            None
        );
    }

    #[test]
    fn last_context_tokens_reads_the_last_assistant_line() {
        let text = concat!(
            r#"{"type":"assistant","message":{"usage":{"input_tokens":1,"cache_read_input_tokens":100}}}"#,
            "\n",
            r#"{"type":"assistant","message":{"usage":{"input_tokens":2,"cache_read_input_tokens":70000}}}"#,
            "\n",
            r#"{"type":"user","message":{"content":"next"}}"#,
            "\n",
            // A later line of another type with a usage is not a turn's context.
            r#"{"type":"summary","message":{"usage":{"input_tokens":5}}}"#,
            "\n",
        );
        assert_eq!(last_context_tokens(text), Some(70_002));
        assert_eq!(last_context_tokens(r#"{"type":"user","message":{}}"#), None);
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nirvana-idle-compact-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A transcript whose last assistant line sits before `filler` bytes of other lines.
    fn transcript_with_filler(path: &Path, filler: usize) {
        let mut text = String::from(
            "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":60000}}}\n",
        );
        let line =
            "{\"type\":\"user\",\"message\":{\"content\":\"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"}}\n";
        let end = text.len() + filler;
        while text.len() < end {
            text.push_str(line);
        }
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn transcript_facts_come_from_a_temporary_file() {
        let dir = temp_dir("facts");
        let path = dir.join("session.jsonl");
        std::fs::write(
            &path,
            "{\"type\":\"assistant\",\"message\":{\"usage\":{\"input_tokens\":3,\"cache_creation_input_tokens\":51000,\"output_tokens\":7}}}\n",
        )
        .unwrap();
        assert_eq!(read_context_tokens(&path), Some(51_003));
        let written = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(file_age(&path, written + 55 * MIN), Some(55 * MIN));
        // A file time ahead of the clock is not an age.
        assert_eq!(file_age(&path, written.checked_sub(MIN).unwrap()), None);
        assert_eq!(read_context_tokens(&dir.join("missing.jsonl")), None);
        // The last assistant line sits before 100 KB of other lines: the read widens past its first 64 KB.
        let long = dir.join("long.jsonl");
        transcript_with_filler(&long, 100 * 1024);
        assert_eq!(read_context_tokens(&long), Some(60_000));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Review S4: the read stops at 1 MB from the end; a last turn further back is no reading.
    #[test]
    fn transcript_read_stops_at_one_megabyte() {
        let dir = temp_dir("bound");
        let near = dir.join("near.jsonl");
        transcript_with_filler(&near, 900 * 1024);
        assert_eq!(read_context_tokens(&near), Some(60_000));
        let far = dir.join("far.jsonl");
        transcript_with_filler(&far, 1100 * 1024);
        assert_eq!(read_context_tokens(&far), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    // ---- choosing the pane's own transcript (review S3) ----

    const START: u64 = 1_000;
    const ID_RECORD: &str = "record-0000";
    const ID_HOOK: &str = "hook-1111";

    fn claude_dir() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            r"C:\claude-config"
        } else {
            "/claude-config"
        })
    }

    fn folder() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            r"C:\work\project"
        } else {
            "/work/project"
        })
    }

    fn transcript(id: &str) -> PathBuf {
        claude_dir()
            .join("projects")
            .join(crate::nirvana_subagents::encode_project_folder(&folder()))
            .join(format!("{id}.jsonl"))
    }

    fn record(written: u64) -> SessionsRecord {
        SessionsRecord {
            session_id: ID_RECORD.into(),
            written_secs: Some(written),
        }
    }

    /// Both transcripts exist, written after the start.
    fn written(path: &Path) -> Option<u64> {
        (path == transcript(ID_RECORD) || path == transcript(ID_HOOK)).then_some(START + 60)
    }

    fn choose(
        foreground: Option<Agent>,
        record: Option<&SessionsRecord>,
        start: Option<u64>,
        hook: Option<&str>,
        others: &[(&str, &Path, Option<&str>)],
    ) -> Result<PathBuf, String> {
        choose_transcript(
            foreground,
            record,
            start,
            hook,
            others,
            &folder(),
            &claude_dir(),
            written,
        )
    }

    #[test]
    fn choose_transcript_takes_the_sessions_record_over_the_hook() {
        let fresh = record(START + 5);
        assert_eq!(
            choose(
                Some(Agent::Claude),
                Some(&fresh),
                Some(START),
                Some(ID_HOOK),
                &[]
            ),
            Ok(transcript(ID_RECORD))
        );
        // A record older than the process is a reused pid's: the hook's id is taken.
        let stale = record(START - 5);
        assert_eq!(
            choose(
                Some(Agent::Claude),
                Some(&stale),
                Some(START),
                Some(ID_HOOK),
                &[]
            ),
            Ok(transcript(ID_HOOK))
        );
    }

    #[test]
    fn choose_transcript_needs_claude_in_the_foreground() {
        let fresh = record(START + 5);
        for foreground in [None, Some(Agent::Codex)] {
            assert_eq!(
                choose(foreground, Some(&fresh), Some(START), Some(ID_HOOK), &[]),
                Err("the pane's foreground is not Claude".into())
            );
        }
    }

    #[test]
    fn choose_transcript_with_no_id_takes_nothing() {
        assert!(choose(Some(Agent::Claude), None, Some(START), None, &[])
            .is_err_and(|why| why.contains("no session id")));
        // Without the start time no id is tied to this Claude, so none is taken.
        let fresh = record(START + 5);
        assert!(
            choose(Some(Agent::Claude), Some(&fresh), None, Some(ID_HOOK), &[])
                .is_err_and(|why| why.contains("start time"))
        );
        // A hook id whose transcript was last written before this Claude started is an earlier conversation's.
        let before = |path: &Path| (path == transcript(ID_HOOK)).then_some(START - 60);
        assert!(choose_transcript(
            Some(Agent::Claude),
            None,
            Some(START),
            Some(ID_HOOK),
            &[],
            &folder(),
            &claude_dir(),
            before,
        )
        .is_err_and(|why| why.contains("before this Claude started")));
        // An id with no transcript yet (a fresh conversation), and a hook value that is not an id.
        assert!(choose_transcript(
            Some(Agent::Claude),
            Some(&fresh),
            Some(START),
            None,
            &[],
            &folder(),
            &claude_dir(),
            |_: &Path| None,
        )
        .is_err_and(|why| why.contains("no transcript")));
        assert!(choose(
            Some(Agent::Claude),
            None,
            Some(START),
            Some("../hook-1111"),
            &[]
        )
        .is_err_and(|why| why.contains("not an id")));
    }

    #[test]
    fn choose_transcript_two_panes_in_one_folder() {
        let fresh = record(START + 5);
        let here = folder();
        // The other pane names no session, or the same one: the two cannot be told apart.
        for other in [None, Some(ID_RECORD)] {
            assert!(choose(
                Some(Agent::Claude),
                Some(&fresh),
                Some(START),
                None,
                &[("w1:p2", here.as_path(), other)],
            )
            .is_err_and(|why| why.contains("cannot be told apart")));
        }
        // Another session in the folder, or another folder: this pane's own.
        let elsewhere = here.join("other");
        for (folder, other) in [(here.as_path(), Some(ID_HOOK)), (elsewhere.as_path(), None)] {
            assert_eq!(
                choose(
                    Some(Agent::Claude),
                    Some(&fresh),
                    Some(START),
                    None,
                    &[("w1:p2", folder, other)],
                ),
                Ok(transcript(ID_RECORD))
            );
        }
    }
}
