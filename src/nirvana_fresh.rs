//! Nirvana fork, plan 25 §2: a fresh session after a run ends (nirvana/plans/2026-10-22-25-lean-orchestrator.md).
//!
//! The owner, 2026-10-22 (L2), picking "Fresh session + handoff": "Nirvana Claude starts fresh at each run from a
//! short note built from the journal. Largest expected saving; I measure it first." Measured in plan 25 §1: a fresh
//! session per run would have read 29.0M fewer cached tokens in WorkOutAppRefactor, 49% of its 58.9M, because a run
//! began 260k deep in one long session.
//!
//! Three processes share the work, through one file per pane, `<home>/nirvana/fresh/<pane>.json`:
//!
//! - **The Stop hook** (`herdr nirvana stop`, `on_stop_in`) records a pending fresh start when the run this pane last
//!   ended (`herdr run end`) ended since the session's last clear, and the session's context is above `fresh_above`
//!   (`seats.toml`). A `/clear` starts a new transcript, so the session's last clear is its transcript's start. When
//!   the rule no longer holds, a pending start the hook wrote is taken back.
//! - **The server** (`app/nirvana_fresh.rs`) types `/clear` into the pane with idle compact's own machinery (plan 27):
//!   the strict input line (`read_input_line`), the owner's last key (`owner_key`), Enter only after a read shows
//!   exactly `/clear`, and the paste erased if Enter never went. It also keeps the wake's rules: only into Nirvana
//!   Claude, only while it is idle, and never while an idle compact's paste is in flight on the same pane.
//! - **The SessionStart hook** (`herdr nirvana digest`, source `clear`, `on_session_start_in`) prints the handoff
//!   note, which Claude Code adds to the fresh session's context, writes the journal's `fresh` event and removes the
//!   pending file.
//!
//! The note is built from files, never from the model, and keeps to `NOTE_MAX_TOKENS` measured as characters / 4.
//! `herdr nirvana fresh` records a pending start on demand, for the owner.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::detect::AgentState;
use crate::nirvana_idle_compact::{after_paste, AfterPaste, InputLine, OwnerKey, Pending, Step};
use crate::nirvana_run::{self, Kind, Project, RunState};

/// Under `<home>/nirvana`: one pending file per pane, and the server's mark of the record it last cleared for.
pub const DIR: &str = "fresh";
/// Plan 25 §2: "2 times the measured first turn, 120k to start" (WorkOut's first turn read 60.7k). The default of
/// `seats.toml`'s `fresh_above`, for a table written before plan 25.
pub const FRESH_ABOVE: u64 = 120_000;
/// Plan 25 §2: "The handoff note, at most 2,000 tokens".
pub const NOTE_MAX_TOKENS: usize = 2_000;
/// The budget as characters: the plan measures a note as characters / 4.
pub const NOTE_MAX_CHARS: usize = NOTE_MAX_TOKENS * 4;
/// One prompt of the owner's in the note, at most: three of them leave room for the runs, the board and lessons.
const PROMPT_MAX_CHARS: usize = 1_500;
/// The owner's prompts the note quotes (§2: "the owner's last 3 prompts, verbatim").
pub const PROMPTS: usize = 3;
/// The open runs and the board sections the note names before "and n more".
const NOTE_RUNS: usize = 5;
const NOTE_SECTIONS: usize = 8;
/// What the server types.
pub const CLEAR: &str = "/clear";
/// One Backspace per character of the paste, as idle compact's erase (`ERASE_KEYS`).
pub const CLEAR_ERASE_KEYS: usize = CLEAR.len();
/// The most of a transcript read for the owner's prompts: the note waits on it inside a hook.
const PROMPT_TAIL: u64 = 2 * 1024 * 1024;

/// After an idle stretch begins, the pending file is looked for at these offsets: the Stop hook that writes it may
/// still be running when herdr reads the pane as idle (its timeout in hooks.json is 15 s).
pub const LOOKUPS: [Duration; 3] = [
    Duration::from_secs(2),
    Duration::from_secs(10),
    Duration::from_secs(30),
];
/// Then once a minute while the pane stays idle, so `herdr nirvana fresh` from another pane is seen.
pub const POLL: Duration = Duration::from_secs(60);
/// A pending start that was skipped (the owner typing, text in the box) is tried again after the owner's quiet
/// window.
pub const RETRY: Duration = Duration::from_secs(nirvana_run::OWNER_QUIET_S);
/// Never sooner than this after the last check, so a skip never makes the server loop spin.
const MIN_GAP: Duration = Duration::from_secs(1);

/// A pending fresh start, as the Stop hook or `herdr nirvana fresh` writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The run whose end asked for it; `None` for the owner's own start with no ended run.
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub ended_at: Option<u64>,
    /// The session's context when the record was written: the journal's "context size before the clear".
    #[serde(default)]
    pub context: Option<u64>,
    /// The session's transcript then, which the note reads the owner's prompts from.
    #[serde(default)]
    pub transcript: Option<String>,
    pub project: String,
    /// Unix seconds; the server's mark names it, so one record is cleared for at most once.
    pub at: u64,
    /// "stop" (the Stop hook) or "owner" (`herdr nirvana fresh`).
    pub by: String,
}

fn safe(pane: &str) -> String {
    pane.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// `<home>/nirvana/fresh/<pane>.json`.
pub fn record_path(home: &Path, pane: &str) -> PathBuf {
    home.join(crate::nirvana_seat::DIR)
        .join(DIR)
        .join(format!("{}.json", safe(pane)))
}

/// `<home>/nirvana/fresh/<pane>.sent`: the `at` of the record the server last pressed Enter for.
pub fn sent_path(home: &Path, pane: &str) -> PathBuf {
    home.join(crate::nirvana_seat::DIR)
        .join(DIR)
        .join(format!("{}.sent", safe(pane)))
}

/// The pane's pending record: `Ok(None)` when there is none, `Err` when one is there but cannot be read (a doubt:
/// the server types nothing).
pub fn read_record(home: &Path, pane: &str) -> Result<Option<Record>, String> {
    let path = record_path(home, pane);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("{} could not be read: {err}", path.display())),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|err| format!("{} is not a record: {err}", path.display()))
}

pub fn write_record(home: &Path, pane: &str, record: &Record) -> std::io::Result<()> {
    let path = record_path(home, pane);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::nirvana_seat::write_atomic(&path, serde_json::to_string(record)?.as_bytes())
}

/// The `at` the server last cleared this pane for; `None` when it never did or the mark cannot be read.
pub fn read_sent(home: &Path, pane: &str) -> Option<u64> {
    std::fs::read_to_string(sent_path(home, pane))
        .ok()?
        .trim()
        .parse()
        .ok()
}

pub fn write_sent(home: &Path, pane: &str, at: u64) -> std::io::Result<()> {
    let path = sent_path(home, pane);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, at.to_string())
}

/// Removes the pane's record and the server's mark: the note was delivered, or the session is already fresh.
pub fn forget(home: &Path, pane: &str) {
    let _ = std::fs::remove_file(record_path(home, pane));
    let _ = std::fs::remove_file(sent_path(home, pane));
}

// ---- the trigger (the Stop hook) ----

/// The run that `pane` last ended, and when: the ended runs whose `opened` names the pane as the orchestrator's
/// (`herdr seat run` records its `HERDR_PANE_ID`), the latest end first.
pub fn last_end_of<'a>(pane: &str, runs: &'a [(String, RunState)]) -> Option<(&'a str, u64)> {
    runs.iter()
        .filter(|(_, state)| {
            state
                .opened
                .as_ref()
                .and_then(|opened| opened.orchestrator_pane.as_deref())
                == Some(pane)
        })
        .filter_map(|(run, state)| Some((run.as_str(), state.ended.as_ref()?.0)))
        .max_by_key(|(_, at)| *at)
}

/// Plan 25 §2 "When": both must hold, the run this session last ended ended since the session's last clear, and
/// the context is above `fresh_above`. `Ok` names the run; `Err` says which part failed. Pure.
pub fn trigger(
    last_end: Option<(&str, u64)>,
    session_began: Option<u64>,
    context: Option<u64>,
    fresh_above: u64,
) -> Result<String, String> {
    let Some((run, ended_at)) = last_end else {
        return Err("no run of this pane has ended".into());
    };
    let Some(began) = session_began else {
        return Err("the session's start is not known".into());
    };
    if ended_at < began {
        return Err(format!("run {run} ended before the session's last clear"));
    }
    let Some(context) = context else {
        return Err("the session's context could not be read".into());
    };
    if context <= fresh_above {
        return Err(format!("context {context} is not above {fresh_above}"));
    }
    Ok(run.to_owned())
}

/// When the session's conversation began: its transcript's creation. A `/clear` starts a new transcript, so this is
/// also the session's last clear; a `--resume` keeps the file, and so the start it had.
pub fn session_began(transcript: &Path) -> Option<u64> {
    let created = std::fs::metadata(transcript).ok()?.created().ok()?;
    Some(crate::nirvana_usage::unix(created))
}

/// Every run of the project with its replayed state.
fn run_states(project: &Project) -> Vec<(String, RunState)> {
    project
        .runs()
        .into_iter()
        .map(|run| {
            let state = nirvana_run::replay(&project.journal(&run).read().0);
            (run, state)
        })
        .collect()
}

/// The Stop hook's half: writes the pane's pending record when `trigger` holds, and takes back a record an earlier
/// Stop wrote when it no longer does (the owner's own `herdr nirvana fresh` stays). Returns what it did.
pub fn on_stop_in(
    home: &Path,
    pane: &str,
    project: &Project,
    transcript: &Path,
    fresh_above: u64,
    now: u64,
) -> std::io::Result<Result<String, String>> {
    let runs = run_states(project);
    let last = last_end_of(pane, &runs);
    let context = crate::nirvana_idle_compact::read_context_tokens(transcript);
    match trigger(last, session_began(transcript), context, fresh_above) {
        Ok(run) => {
            // A record this hook wrote for the same run keeps its second: the server's mark names it, so a turn
            // after the clear that was never confirmed is not a second record to clear for.
            let at = read_record(home, pane)
                .ok()
                .flatten()
                .filter(|record| record.by == "stop" && record.run.as_deref() == Some(run.as_str()))
                .map_or(now, |record| record.at);
            let record = Record {
                ended_at: last.map(|(_, at)| at),
                run: Some(run.clone()),
                context,
                transcript: Some(transcript.display().to_string()),
                project: project.root.display().to_string(),
                at,
                by: "stop".into(),
            };
            write_record(home, pane, &record)?;
            Ok(Ok(run))
        }
        Err(why) => {
            if read_record(home, pane)
                .ok()
                .flatten()
                .is_some_and(|record| record.by == "stop")
            {
                forget(home, pane);
            }
            Ok(Err(why))
        }
    }
}

// ---- the note (the SessionStart hook) ----

/// What the note is built from, all read from files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoteInput {
    pub run: Option<String>,
    /// The ended run's receipt line.
    pub ended: Option<String>,
    /// Each open run, with its orders' ids and phases.
    pub open: Vec<(String, Vec<(String, String)>)>,
    /// The board's sections: name and first line.
    pub board: Vec<(String, String)>,
    /// The owner's last prompts, oldest first, verbatim.
    pub prompts: Vec<String>,
    /// The project's `.nirvana/lessons` index line.
    pub lessons: Option<String>,
}

/// The note's last line (§2).
pub fn closing(run: Option<&str>) -> String {
    match run {
        Some(run) => format!(
            "This is a fresh session after run {run}. Ask the owner before assuming anything the note does not say."
        ),
        None => "This is a fresh session the owner asked for. Ask the owner before assuming anything the note does \
                 not say."
            .into(),
    }
}

fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let kept: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

/// The handoff note. Pure, and never over `NOTE_MAX_CHARS`: each prompt is cut at `PROMPT_MAX_CHARS`, and a body
/// still too long is cut before the closing line, which is always whole.
pub fn note(input: &NoteInput) -> String {
    let mut body = String::from(
        "herdr's handoff note (plan 25): the previous session was cleared after a run ended.\n",
    );
    if let Some(ended) = &input.ended {
        body.push_str(&format!("\nThe run that ended: {ended}\n"));
    }
    if input.open.is_empty() {
        body.push_str("\nNo run is open.\n");
    } else {
        body.push_str("\nRuns still open:\n");
        for (run, orders) in input.open.iter().take(NOTE_RUNS) {
            let orders = if orders.is_empty() {
                "no orders".to_owned()
            } else {
                orders
                    .iter()
                    .map(|(order, phase)| format!("{order} {phase}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            body.push_str(&format!("- {run}: {orders}\n"));
        }
        if input.open.len() > NOTE_RUNS {
            body.push_str(&format!("- and {} more\n", input.open.len() - NOTE_RUNS));
        }
    }
    if !input.board.is_empty() {
        body.push_str("\nThe board's open items:\n");
        for (name, first) in input.board.iter().take(NOTE_SECTIONS) {
            body.push_str(&format!("- {name}: {}\n", cut(first, 160)));
        }
        if input.board.len() > NOTE_SECTIONS {
            body.push_str(&format!(
                "- and {} more\n",
                input.board.len() - NOTE_SECTIONS
            ));
        }
    }
    if !input.prompts.is_empty() {
        body.push_str("\nThe owner's last prompts, verbatim, oldest first:\n");
        for prompt in &input.prompts {
            body.push_str(&format!(
                "> {}\n",
                cut(prompt, PROMPT_MAX_CHARS).replace('\n', "\n> ")
            ));
        }
    }
    if let Some(lessons) = &input.lessons {
        body.push_str(&format!("\n{lessons}\n"));
    }
    let closing = closing(input.run.as_deref());
    let room = NOTE_MAX_CHARS - closing.chars().count() - 1;
    let mut text = cut(&body, room);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&closing);
    text
}

/// The note's size in the plan's measure, characters / 4, rounded up.
pub fn note_tokens(note: &str) -> u64 {
    note.chars().count().div_ceil(4) as u64
}

/// A prompt the owner typed, from one transcript line; `None` for anything else: herdr's own wake digests, Claude
/// Code's meta lines and command echoes, tool results, a subagent's lines.
fn owner_prompt(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("user")
        || value.get("isMeta").and_then(Value::as_bool) == Some(true)
        || value.get("isSidechain").and_then(Value::as_bool) == Some(true)
        || value.get("isCompactSummary").and_then(Value::as_bool) == Some(true)
    {
        return None;
    }
    let content = value.pointer("/message/content")?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => {
            if blocks
                .iter()
                .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
            {
                return None;
            }
            blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => return None,
    };
    let text = text.trim();
    let not_owner = [
        "<command-",
        "<local-command-",
        "<system-reminder>",
        "Caveat:",
        "herdr run ",
    ];
    if text.is_empty() || not_owner.iter().any(|lead| text.starts_with(lead)) {
        return None;
    }
    Some(text.to_owned())
}

/// The owner's last `n` prompts in a transcript's text, oldest first. Pure.
pub fn owner_prompts(text: &str, n: usize) -> Vec<String> {
    let mut prompts: Vec<String> = text
        .lines()
        .rev()
        .filter_map(owner_prompt)
        .take(n)
        .collect();
    prompts.reverse();
    prompts
}

/// The note's input, read from the project's journals, board and lessons, and the transcript the record names.
pub fn note_input(record: &Record) -> NoteInput {
    let project = Project::at(&record.project);
    let runs = run_states(&project);
    let ended = record.run.as_ref().and_then(|run| {
        let (_, state) = runs.iter().find(|(id, _)| id == run)?;
        let (at, _, _) = state.ended.as_ref()?;
        let orders = state
            .orders
            .iter()
            .map(|order| format!("{} {}", order.id(), order.phase.word()))
            .collect::<Vec<_>>()
            .join(", ");
        let receipt = project.journal(run).dir().join("receipt.md");
        Some(format!(
            "{run} ended at {}; orders: {}; receipt {}",
            nirvana_run::utc(*at),
            if orders.is_empty() {
                "none".into()
            } else {
                orders
            },
            receipt.display()
        ))
    });
    let open = runs
        .iter()
        .filter(|(_, state)| state.ended.is_none())
        .map(|(run, state)| {
            (
                run.clone(),
                state
                    .orders
                    .iter()
                    .map(|order| (order.id().to_owned(), order.phase.word().to_owned()))
                    .collect(),
            )
        })
        .collect();
    let board = crate::nirvana_board::read(&project.root)
        .map(|board| {
            board
                .sections
                .iter()
                .map(|section| {
                    let first = section
                        .text
                        .lines()
                        .map(str::trim)
                        .find(|line| !line.is_empty())
                        .unwrap_or("")
                        .to_owned();
                    (section.name.clone(), first)
                })
                .collect()
        })
        .unwrap_or_default();
    let prompts = record
        .transcript
        .as_deref()
        .and_then(|path| {
            crate::nirvana_subagents::read_file_tail(Path::new(path), PROMPT_TAIL).ok()
        })
        .map(|text| owner_prompts(&text, PROMPTS))
        .unwrap_or_default();
    let index = crate::nirvana_lessons::read_index(&project.root);
    let live = index
        .lessons
        .values()
        .filter(|entry| crate::nirvana_lessons::live(&entry.counters))
        .count();
    NoteInput {
        run: record.run.clone(),
        ended,
        open,
        board,
        prompts,
        lessons: Some(crate::nirvana_lessons::index_line_text(&project.root, live)),
    }
}

/// The SessionStart hook's half. On `clear`, with a record for the pane: the note to print, after the journal's
/// `fresh` event is written and the record removed. On `startup`, a session that is already fresh: the record is
/// removed and nothing printed. Any other source leaves it.
pub fn on_session_start_in(home: &Path, pane: &str, source: &str, now: u64) -> Option<String> {
    match source {
        "clear" => {}
        "startup" => {
            forget(home, pane);
            return None;
        }
        _ => return None,
    }
    let record = read_record(home, pane).ok().flatten()?;
    let text = note(&note_input(&record));
    if let Some(run) = &record.run {
        let journal = Project::at(&record.project).journal(run);
        if journal.path.is_file() {
            if let Err(err) = journal.append(
                now,
                Kind::Fresh {
                    pane: pane.to_owned(),
                    by: record.by.clone(),
                    context: record.context,
                    note_tokens: note_tokens(&text),
                },
            ) {
                eprintln!("herdr nirvana digest: the fresh event was not journaled: {err}");
            }
        }
    }
    forget(home, pane);
    Some(text)
}

// ---- the server's rule ----

/// One idle stretch's fresh-start state (plan 25 §2), kept on the stretch (`IdleStretch::fresh`) so that any state
/// but `Idle` ends it with the stretch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FreshStretch {
    /// Checks made in this stretch.
    pub checks: usize,
    pub checked_at: Option<Instant>,
    /// A pending record was seen in this stretch, and left for now.
    pub found: bool,
    /// `/clear` went in this stretch.
    pub sent: bool,
    /// The paste went; the reads after it are due (idle compact's `Pending`).
    pub pending: Option<Pending>,
    /// The `at` of the record the paste is for, which the server's mark takes once Enter went.
    pub record_at: Option<u64>,
}

impl FreshStretch {
    /// A `/clear` paste is in flight: idle compact keeps off the pane until it is done.
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }

    /// When the stretch that began at `since` is next due a check: a read after the paste first, then the lookups,
    /// then `RETRY` for a record left for now or `POLL` for none, never sooner than `MIN_GAP` after the last check.
    pub fn next_check_at(&self, since: Option<Instant>) -> Option<Instant> {
        if let Some(pending) = &self.pending {
            return pending.due();
        }
        if self.sent {
            return None;
        }
        let since = since?;
        let at = match (LOOKUPS.get(self.checks), self.checked_at) {
            (Some(after), _) => since.checked_add(*after)?,
            (None, Some(checked)) => checked.checked_add(if self.found { RETRY } else { POLL })?,
            (None, None) => since,
        };
        match self.checked_at {
            Some(checked) => Some(at.max(checked.checked_add(MIN_GAP)?)),
            None => Some(at),
        }
    }
}

/// Everything `decide` reads about one pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// Idle compact's `/compact` is in flight on this pane.
    pub compact_in_flight: bool,
    pub record: Result<Option<Record>, String>,
    /// The `at` the server last cleared this pane for.
    pub sent_for: Option<u64>,
    /// The pane runs Nirvana Claude (`nirvana_run::is_nirvana_claude_argv`); `None` when its process was not read.
    pub nirvana: Option<bool>,
    pub state: AgentState,
    pub owner_key: OwnerKey,
    pub input_line: InputLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Paste `/clear`, for the record written at this second.
    Send(u64),
    /// A record waits, but not now; why.
    Skip(String),
    /// Nothing waits.
    Nothing,
}

/// Plan 25 §2 "How", with the wake's rules: only into Nirvana Claude, only while it is idle, only when the owner has
/// been quiet `OWNER_QUIET_S` and the input line is provably empty, and never while idle compact's paste is in
/// flight. The first rule that fails is the reason. Pure.
pub fn decide(facts: &Facts) -> Decision {
    let skip = |why: String| Decision::Skip(why);
    if facts.compact_in_flight {
        return skip("an idle compact's /compact is in flight".into());
    }
    let record = match &facts.record {
        Err(why) => return skip(why.clone()),
        Ok(None) => return Decision::Nothing,
        Ok(Some(record)) => record,
    };
    if facts.sent_for == Some(record.at) {
        return Decision::Nothing;
    }
    match facts.nirvana {
        Some(true) => {}
        Some(false) => return skip("the pane does not run Nirvana Claude".into()),
        None => return skip("the pane's process could not be read".into()),
    }
    if facts.state != AgentState::Idle {
        return skip(format!("the pane is {:?}, not idle", facts.state));
    }
    if let Some(why) = crate::nirvana_idle_compact::owner_quiet_breach(&facts.owner_key) {
        return skip(why);
    }
    match &facts.input_line {
        InputLine::Empty => Decision::Send(record.at),
        InputLine::Text(_) => skip("the input line holds text".into()),
        InputLine::Unsure(why) => skip(format!("the input line is not provably empty: {why}")),
    }
}

/// One read after the `/clear` paste: idle compact's rule (`after_paste`), read for `/clear`. The line is shown to
/// it as its own command exactly when it reads exactly `/clear`, and a line reading `/compact` is a doubt here, so
/// every outcome of that rule (the tries, the erase, the owner's key ending them) holds for this paste unchanged.
pub fn after_clear_paste(step: Step, line: &InputLine, owner_key: &OwnerKey) -> AfterPaste {
    let seen = match line {
        InputLine::Text(text) if text == CLEAR => {
            InputLine::Text(crate::nirvana_idle_compact::COMMAND.to_owned())
        }
        InputLine::Text(text) if text == crate::nirvana_idle_compact::COMMAND => {
            InputLine::Unsure(format!("the input line reads {text}, not {CLEAR}"))
        }
        other => other.clone(),
    };
    after_paste(step, &seen, owner_key)
}

// ---- the CLI ----

fn pane_env() -> Option<String> {
    std::env::var("HERDR_PANE_ID")
        .ok()
        .filter(|pane| !pane.is_empty())
}

/// `herdr nirvana stop`'s call: never fails the turn, and says nothing on stdout (the Stop hook's stdout is its
/// decision).
pub fn stop_hook(hook: &Value, project: &Project) {
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return;
    }
    let (Some(home), Some(pane)) = (crate::nirvana::home(), pane_env()) else {
        return;
    };
    let Some(transcript) = hook.get("transcript_path").and_then(Value::as_str) else {
        return;
    };
    let fresh_above = crate::nirvana_seat::parse(&crate::nirvana_seat::read_or_create(
        &crate::nirvana_seat::dir().join(crate::nirvana_seat::SEATS_FILE),
    ))
    .map_or(FRESH_ABOVE, |table| table.constants.fresh_above);
    if let Err(err) = on_stop_in(
        &home,
        &pane,
        project,
        Path::new(transcript),
        fresh_above,
        nirvana_run::now(),
    ) {
        eprintln!("herdr nirvana stop: the fresh start was not recorded: {err}");
    }
}

/// `herdr nirvana digest`'s call: the note to print after the digest, on a `/clear` with a record for the pane.
pub fn session_start_hook(hook: &Value) -> Option<String> {
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return None;
    }
    let source = hook.get("source").and_then(Value::as_str)?;
    let (home, pane) = (crate::nirvana::home()?, pane_env()?);
    on_session_start_in(&home, &pane, source, nirvana_run::now())
}

/// `herdr nirvana fresh [<pane>]` (§2: "does the same on demand, for the owner"): a pending record for the pane, its
/// own `HERDR_PANE_ID` when none is named. The server types `/clear` once the pane is idle and the owner quiet.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let Some(pane) = args.first().cloned().or_else(pane_env) else {
        eprintln!("usage: herdr nirvana fresh [<pane>]  (inside a pane, its own HERDR_PANE_ID)");
        std::process::exit(2);
    };
    let Some(home) = crate::nirvana::home() else {
        eprintln!("herdr nirvana fresh: no Nirvana home");
        std::process::exit(2);
    };
    let project = Project::from_env();
    let runs = run_states(&project);
    let last = last_end_of(&pane, &runs);
    let session = crate::nirvana_launch::session_record_path(&home, &project.root);
    let transcript = std::fs::read_to_string(session)
        .ok()
        .and_then(|text| {
            serde_json::from_str::<crate::nirvana_launch::SessionRecord>(
                text.trim_start_matches('\u{feff}'),
            )
            .ok()
        })
        .map(|record| record.transcript);
    let record = Record {
        run: last.map(|(run, _)| run.to_owned()),
        ended_at: last.map(|(_, at)| at),
        context: transcript
            .as_deref()
            .and_then(|path| crate::nirvana_idle_compact::read_context_tokens(Path::new(path))),
        transcript,
        project: project.root.display().to_string(),
        at: nirvana_run::now(),
        by: "owner".into(),
    };
    write_record(&home, &pane, &record)?;
    println!(
        "herdr nirvana fresh: pending for pane {pane}; herdr types /clear once Nirvana Claude is idle, its input line \
         is empty and you have been quiet {} s",
        nirvana_run::OWNER_QUIET_S
    );
    Ok(())
}

#[cfg(test)]
mod tests;
