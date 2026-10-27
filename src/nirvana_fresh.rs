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
/// Review B1 and S1: a run's end asks for a fresh start only this long after it, and a Stop's record lives no
/// longer. A hypothesis, not a measurement: long enough for the turn that ended the run and herdr's lookups, short
/// enough that a session left idle long after a run is idle compact's business (plan 27), not this one's.
pub const FRESH_WITHIN: Duration = Duration::from_secs(10 * 60);
/// Review B1: the most of a transcript read for an owner prompt after the run's end. The Stop hook reads it on each
/// turn inside `FRESH_WITHIN` of an end; the transcripts on this machine reach 126 MB, so it is never read whole.
const OWNER_TAIL: u64 = 8 * 1024 * 1024;
/// Review round 2, S-C: the owner's own record lives no longer than this. A hypothesis, not a measurement: the owner
/// asks for a fresh start to have one soon, and an hour on the conversation they asked it for may not be the one
/// they are in.
pub const OWNER_WITHIN: Duration = Duration::from_secs(60 * 60);

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

/// Round 3's nit: the server's mark names the record `at` and was written at most `FRESH_WITHIN` before `now`. A mark
/// whose time cannot be read is a doubt, so not lately.
pub fn sent_lately(home: &Path, pane: &str, at: u64, now: u64) -> bool {
    let written = std::fs::metadata(sent_path(home, pane))
        .and_then(|meta| meta.modified())
        .map(crate::nirvana_usage::unix);
    read_sent(home, pane) == Some(at)
        && written.is_ok_and(|written| now.saturating_sub(written) <= FRESH_WITHIN.as_secs())
}

pub fn write_sent(home: &Path, pane: &str, at: u64) -> std::io::Result<()> {
    let path = sent_path(home, pane);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::nirvana_seat::write_atomic(&path, at.to_string().as_bytes())
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

/// A run `pane` opened that has not ended (review B1: a fresh start never clears in the middle of a run).
pub fn open_of<'a>(pane: &str, runs: &'a [(String, RunState)]) -> Option<&'a str> {
    runs.iter()
        .filter(|(_, state)| state.ended.is_none())
        .find(|(_, state)| {
            state
                .opened
                .as_ref()
                .and_then(|opened| opened.orchestrator_pane.as_deref())
                == Some(pane)
        })
        .map(|(run, _)| run.as_str())
}

/// Review round 2 nit: the one line said when an open run holds off the fresh start a run's end would ask for, naming
/// the run. No age caps an open run: one left open by mistake only fails safe. Pure.
pub fn open_blocks(last_end: Option<(&str, u64)>, open_here: Option<&str>) -> Option<String> {
    let (ended, _) = last_end?;
    Some(format!(
        "no fresh start after run {ended}: run {} of this pane is open",
        open_here?
    ))
}

/// Plan 25 §2 "When", with the review round's B1: the run this session last ended ended since the session's last
/// clear and at most `FRESH_WITHIN` ago, no run of this pane is open, the context is above `fresh_above`, and the
/// owner has typed nothing since the end (`owner_since`, given the end; read last, since it reads the transcript).
/// `Ok` names the run; `Err` says which part failed. Pure over its arguments.
pub fn trigger(
    last_end: Option<(&str, u64)>,
    open_here: Option<&str>,
    session_began: Option<u64>,
    context: Option<u64>,
    fresh_above: u64,
    now: u64,
    owner_since: impl FnOnce(u64) -> Result<(), String>,
) -> Result<String, String> {
    let Some((run, ended_at)) = last_end else {
        return Err("no run of this pane has ended".into());
    };
    if let Some(open) = open_here {
        return Err(format!("run {open} of this pane is open"));
    }
    if now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {
        return Err(format!(
            "run {run} ended more than {} s ago",
            FRESH_WITHIN.as_secs()
        ));
    }
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
    owner_since(ended_at)?;
    Ok(run.to_owned())
}

/// Review B1: `Err` when the owner typed a prompt at or after `ended_at`, the conversation they are still in, or
/// when that cannot be ruled out. `text` is the transcript's tail. Every doubt is an owner prompt: a line that does
/// not parse, an owner prompt whose time cannot be read, and a tail with no line timed before the end, which may
/// have cut the prompts after the end off. The owner's prompts are `owner_prompts`' own. Pure.
pub fn owner_prompt_since(text: &str, ended_at: u64) -> Result<(), String> {
    typed_since(text, ended_at, owner_typed_in)
}

/// The leads of the lines Claude Code records for a `!` command the owner runs in the pane: the command, then its
/// output.
pub const BANG_LEADS: [&str; 3] = ["<bash-input>", "<bash-stdout>", "<bash-stderr>"];

/// A `!` command's line (`BANG_LEADS`), as `owner_prompt_in` reads it.
fn bang_line(value: &Value) -> bool {
    owner_prompt_in(value).is_some_and(|text| BANG_LEADS.iter().any(|lead| text.starts_with(lead)))
}

/// Round 3, R2: `owner_prompt_since` for the owner's own record, read from its second. The owner's
/// `!herdr nirvana fresh` is recorded as a `<bash-input>` line and a `<bash-stdout>` line in that second or just
/// after, so counting them forgot the record the owner had just asked for. A `!` command's lines are not the owner
/// typing here; after a run's end they still are (`owner_prompt_since`), where they are a doubt. Pure.
pub fn owner_typed_since(text: &str, at: u64) -> Result<(), String> {
    typed_since(text, at, |value| owner_typed_in(value) && !bang_line(value))
}

/// `owner_prompt_since` with the owner's lines told by `owners`.
fn typed_since(text: &str, ended_at: u64, owners: impl Fn(&Value) -> bool) -> Result<(), String> {
    let mut reaches_the_end = false;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return Err("a transcript line does not parse, so it may be the owner's".into());
        };
        let at = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(crate::nirvana_usage::rfc3339);
        if owners(&value) {
            match at {
                Some(at) if at < ended_at => {}
                Some(at) => {
                    return Err(format!(
                        "the owner typed at {}, after the run's end",
                        nirvana_run::utc(at)
                    ))
                }
                None => return Err("an owner prompt's time cannot be read".into()),
            }
        }
        reaches_the_end |= at.is_some_and(|at| at < ended_at);
    }
    if !reaches_the_end {
        return Err("the transcript's tail has no line from before the run's end".into());
    }
    Ok(())
}

/// The transcript's tail, read for `owner_prompt_since`; an unread transcript is a doubt.
fn owner_since(transcript: &Path, ended_at: u64) -> Result<(), String> {
    let text = crate::nirvana_subagents::read_file_tail(transcript, OWNER_TAIL)
        .map_err(|err| format!("the transcript could not be read: {err}"))?;
    owner_prompt_since(&text, ended_at)
}

/// When the session's conversation began: its transcript's creation. A `/clear` starts a new transcript, so this is
/// also the session's last clear; a `--resume` keeps the file, and so the start it had.
pub fn session_began(transcript: &Path) -> Option<u64> {
    let created = std::fs::metadata(transcript).ok()?.created().ok()?;
    Some(crate::nirvana_usage::unix(created))
}

/// Every run of the project with its replayed state, for the note and the owner's record, where a run not read only
/// leaves a line out. The fresh start's rules read `read_run_states`.
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

/// Round 3, R1: every run of the project with its replayed state, or why not all of them could be read.
/// `run_states` reads a runs folder it cannot list, or a journal it cannot read, as no run, and `open_of` then finds
/// no open run in it: a doubt read as a pass. Here a folder or entry not listed, a journal not read and a journal line
/// that does not parse are each `Err`. A project with no runs folder has no run, and a folder with no journal is no
/// run, as `Project::runs` reads them.
pub fn read_run_states(project: &Project) -> Result<Vec<(String, RunState)>, String> {
    let dir = project.dir().join("runs");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(format!("{} could not be listed: {err}", dir.display())),
    };
    let mut runs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("{} could not be listed: {err}", dir.display()))?;
        let kind = entry
            .file_type()
            .map_err(|err| format!("{} could not be read: {err}", entry.path().display()))?;
        if !kind.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|name| format!("a run folder's name is not text: {name:?}"))?;
        let path = entry.path().join(nirvana_run::JOURNAL);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("{} could not be read: {err}", path.display())),
        };
        let (events, bad) = nirvana_run::parse_lines(&text);
        if bad > 0 {
            return Err(format!(
                "{} has {bad} line(s) that do not parse",
                path.display()
            ));
        }
        runs.push((name, nirvana_run::replay(&events)));
    }
    runs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(runs)
}

/// Round 3, R1: a Stop's record is sent for only while the replay finds its own run, and finds it ended. Pure.
pub fn own_run_ended(record: &Record, runs: &[(String, RunState)]) -> Result<(), String> {
    let Some(run) = record.run.as_deref() else {
        return Err("the Stop's record names no run".into());
    };
    match runs.iter().find(|(id, _)| id == run) {
        None => Err(format!("run {run} is not among the project's runs")),
        Some((_, state)) if state.ended.is_none() => Err(format!("run {run} has not ended")),
        Some(_) => Ok(()),
    }
}

/// Takes back a pending start an earlier Stop wrote; the owner's own is left.
fn take_back(home: &Path, pane: &str) {
    if read_record(home, pane)
        .ok()
        .flatten()
        .is_some_and(|record| record.by == "stop")
    {
        forget(home, pane);
    }
}

/// The Stop hook's half: writes the pane's pending record when `trigger` holds, and takes back a record an earlier
/// Stop wrote when it no longer does. The owner's own `herdr nirvana fresh` is never taken back nor overwritten.
/// Returns what it did.
pub fn on_stop_in(
    home: &Path,
    pane: &str,
    project: &Project,
    transcript: &Path,
    fresh_above: u64,
    now: u64,
) -> std::io::Result<Result<String, String>> {
    // Round 3, R1: a run list not read would read as no open run; that is a doubt, so no record.
    let runs = match read_run_states(project) {
        Ok(runs) => runs,
        Err(why) => {
            eprintln!("herdr nirvana stop: no fresh start: {why}");
            take_back(home, pane);
            return Ok(Err(why));
        }
    };
    let last = last_end_of(pane, &runs);
    let open = open_of(pane, &runs);
    if let Some(line) = open_blocks(last, open) {
        eprintln!("herdr nirvana stop: {line}");
    }
    let context = crate::nirvana_idle_compact::read_context_tokens(transcript);
    let held = trigger(
        last,
        open,
        session_began(transcript),
        context,
        fresh_above,
        now,
        |ended_at| owner_since(transcript, ended_at),
    );
    match held {
        Ok(run) => {
            let existing = read_record(home, pane).ok().flatten();
            // Review nit: the owner asked for this pane's fresh start themselves; the hook leaves it as it is.
            if existing.as_ref().is_some_and(|record| record.by == "owner") {
                return Ok(Err("the owner's own fresh start waits".into()));
            }
            // A record this hook wrote for the same run keeps its second: the server's mark names it, so a turn
            // after the clear that was never confirmed is not a second record to clear for.
            let at = existing
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
            take_back(home, pane);
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
    owner_prompt_in(&serde_json::from_str(line.trim()).ok()?)
}

/// `owner_prompt` over a parsed line, which the trigger's check reads too (review B1: one filter for both).
fn owner_prompt_in(value: &Value) -> Option<String> {
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
        // Review round 2: Claude Code's own news of a background task, a `user` line without `isMeta`.
        "<task-notification>",
    ];
    if text.is_empty() || not_owner.iter().any(|lead| text.starts_with(lead)) {
        return None;
    }
    Some(text.to_owned())
}

/// Review round 2: the owner is in the conversation for B1's rule when a line is their prompt, or an image they pasted
/// with no words, which `owner_prompt_in` leaves out (the note has nothing of it to quote). The image line is read as
/// the same line with each image as a word, so `owner_prompt_in`'s header, tool results and leads hold for it
/// unchanged; a line with words of its own was already read by `owner_prompt_in`.
fn owner_typed_in(value: &Value) -> bool {
    if owner_prompt_in(value).is_some() {
        return true;
    }
    let Some(Value::Array(blocks)) = value.pointer("/message/content") else {
        return false;
    };
    let kind = |block: &Value| block.get("type").and_then(Value::as_str) == Some("image");
    let worded = blocks.iter().any(|block| {
        block
            .get("text")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    });
    if worded || !blocks.iter().any(kind) {
        return false;
    }
    let mut shown = value.clone();
    if let Some(Value::Array(blocks)) = shown.pointer_mut("/message/content") {
        for block in blocks.iter_mut().filter(|block| kind(block)) {
            *block = serde_json::json!({"type": "text", "text": "[image]"});
        }
    }
    owner_prompt_in(&shown).is_some()
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

/// The note's input, read from the project's journals, board and lessons, and the owner's prompts from
/// `transcript`.
pub fn note_input(record: &Record, transcript: Option<&Path>) -> NoteInput {
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
    let prompts = transcript
        .and_then(|path| crate::nirvana_subagents::read_file_tail(path, PROMPT_TAIL).ok())
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

/// The SessionStart hook's half. On `clear`, with a record for the pane of the hook's own `project`: the note to
/// print, after the journal's `fresh` event is written and the record removed; a record of another project is
/// forgotten and nothing printed (review S1: a reused pane id). On `startup`, a session that is already fresh: the
/// record is removed and nothing printed. On `resume`, a Stop's record is removed (review S1: it was for the
/// session before the restart); the owner's stays. Any other source leaves it. Round 2's nit: on `clear`, a stale
/// Stop's record (`stale`) the server did not clear for is forgotten and nothing printed.
///
/// The owner's prompts come from the record's transcript for a Stop's record, and from the transcript the hook
/// reports, `hook_transcript`, for the owner's (review S2: the owner's record names none).
pub fn on_session_start_in(
    home: &Path,
    pane: &str,
    source: &str,
    project: Option<&Path>,
    hook_transcript: Option<&Path>,
    now: u64,
) -> Option<String> {
    match source {
        "clear" => {}
        "startup" => {
            forget(home, pane);
            return None;
        }
        "resume" => {
            if read_record(home, pane)
                .ok()
                .flatten()
                .is_some_and(|record| record.by == "stop")
            {
                forget(home, pane);
            }
            return None;
        }
        _ => return None,
    }
    let record = read_record(home, pane).ok().flatten()?;
    if !project.is_some_and(|project| {
        crate::nirvana_spaces::same_folder(project, Path::new(&record.project))
    }) {
        forget(home, pane);
        return None;
    }
    // Review round 2 nit: a stale Stop's record is no note for a `/clear` the owner typed. The one the server cleared
    // for (its mark names it) is not stale here: the server sent it inside `FRESH_WITHIN`, and this hook runs after.
    // Round 3's nit: only a mark written inside `FRESH_WITHIN` of now (`sent_lately`), so an old mark left beside an
    // old record never prints its note at a `/clear` the owner typed.
    if record.by != "owner"
        && !sent_lately(home, pane, record.at, now)
        && stale(&record, now).is_some()
    {
        forget(home, pane);
        return None;
    }
    let transcript = if record.by == "owner" {
        hook_transcript.map(Path::to_path_buf)
    } else {
        record.transcript.as_deref().map(PathBuf::from)
    };
    let text = note(&note_input(&record, transcript.as_deref()));
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
    /// A `/clear` paste is in flight, or its Enter went in this stretch: idle compact keeps off the pane (review S3:
    /// a compact pass after the Enter could still read the old transcript and type `/compact` into the new
    /// session). The stretch ends on any state but `Idle`, so this never outlives it.
    pub fn busy(&self) -> bool {
        self.pending.is_some() || self.sent
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
    /// Idle compact pasted `/compact` in this stretch (review S3: the reverse of `FreshStretch::busy`).
    pub compact_sent: bool,
    pub record: Result<Option<Record>, String>,
    /// Unix seconds now, for the record's age (review S1).
    pub now: u64,
    /// The `at` the server last cleared this pane for.
    pub sent_for: Option<u64>,
    /// The pane runs Nirvana Claude (`nirvana_run::is_nirvana_claude_argv`); `None` when its process was not read.
    pub nirvana: Option<bool>,
    /// The pane's own transcript now, as idle compact chooses it (`choose_transcript`); `None` when it could not be
    /// told (review S1).
    pub transcript: Option<PathBuf>,
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
    /// The record is stale: remove it and its mark; why (review S1).
    Forget(String),
    /// Nothing waits.
    Nothing,
}

/// Whether two transcript paths name one file: separators, a `\\?\` prefix and case aside (Windows), as the hook and
/// `choose_transcript` may spell the Claude folder differently.
pub fn same_transcript(left: &Path, right: &Path) -> bool {
    crate::nirvana_spaces::same_folder(left, right)
}

/// Why a record is too old to clear for, or `None`. A Stop's record: its own second or its run's end more than
/// `FRESH_WITHIN` ago (review S1, and round 2's S-B: a record kept from an earlier turn of the same run keeps its
/// second, but the end is what B1 ages); one naming no end is a doubt. The owner's: its second more than
/// `OWNER_WITHIN` ago (round 2's S-C). Pure.
pub fn stale(record: &Record, now: u64) -> Option<String> {
    if record.by == "owner" {
        return (now.saturating_sub(record.at) > OWNER_WITHIN.as_secs()).then(|| {
            format!(
                "the owner's record is more than {} s old",
                OWNER_WITHIN.as_secs()
            )
        });
    }
    let Some(ended_at) = record.ended_at else {
        return Some("the Stop's record names no end".into());
    };
    if now.saturating_sub(record.at) > FRESH_WITHIN.as_secs() {
        return Some(format!(
            "the Stop's record is more than {} s old",
            FRESH_WITHIN.as_secs()
        ));
    }
    if now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {
        return Some(format!(
            "its run ended more than {} s ago",
            FRESH_WITHIN.as_secs()
        ));
    }
    None
}

/// Review round 2, S-A and S-C: the rules the record was written under, again at send time and in one place, since
/// the Stop hook that would take a record back may be late or killed (its timeout in hooks.json is 15 s). A Stop's
/// record: not stale (`stale`, the end within `FRESH_WITHIN`), no open run of the pane (`open_here`), and no owner
/// prompt at or after the end in the pane's transcript now. The owner's: not stale, and no owner prompt at or after
/// the record's own second. `tail` is that transcript's tail; one not read is a doubt. `Err` says why. Pure.
pub fn recheck(
    record: &Record,
    now: u64,
    open_here: Option<&str>,
    tail: Result<&str, String>,
) -> Result<(), String> {
    if let Some(why) = stale(record, now) {
        return Err(why);
    }
    let since = if record.by == "owner" {
        record.at
    } else {
        if let Some(open) = open_here {
            return Err(format!("run {open} of this pane is open"));
        }
        record
            .ended_at
            .ok_or_else(|| "the Stop's record names no end".to_owned())?
    };
    let tail = tail?;
    // Round 3, R2: the owner's own `!herdr nirvana fresh` is not the owner typing after their record.
    if record.by == "owner" {
        owner_typed_since(tail, since)
    } else {
        owner_prompt_since(tail, since)
    }
}

/// `recheck` over the files: the runs of the record's project (a Stop's record only), and the tail of `transcript`,
/// the pane's own as the server read it (`read_outside_and_transcript`). Round 3, R1: runs not all read are a doubt,
/// and a Stop's record is sent for only while its own run is among them, ended (`own_run_ended`).
pub fn recheck_in(
    record: &Record,
    pane: &str,
    transcript: Option<&Path>,
    now: u64,
) -> Result<(), String> {
    let runs = if record.by == "owner" {
        Vec::new()
    } else {
        let runs = read_run_states(&Project::at(&record.project))?;
        own_run_ended(record, &runs)?;
        runs
    };
    let tail = transcript
        .ok_or_else(|| "the pane's transcript could not be told".to_owned())
        .and_then(|path| {
            crate::nirvana_subagents::read_file_tail(path, OWNER_TAIL)
                .map_err(|err| format!("the transcript could not be read: {err}"))
        });
    recheck(
        record,
        now,
        open_of(pane, &runs),
        tail.as_deref().map_err(String::clone),
    )
}

/// Plan 25 §2 "How", with the wake's rules: only into Nirvana Claude, only while it is idle, only when the owner has
/// been quiet `OWNER_QUIET_S` and the input line is provably empty, and never while idle compact's paste is in
/// flight or after it went in the stretch. The review round's S1: a stale record is forgotten (`stale`), and a
/// Stop's is sent for only while the pane's transcript is still the record's (the owner's record names none, so it
/// is not compared). Round 2's S-A: when every other rule holds, `recheck` (B1's rules again, given the record) is
/// asked last, since it reads files; its failure forgets the record. The first rule that fails is the reason.
/// Pure over its arguments.
pub fn decide(facts: &Facts, recheck: impl FnOnce(&Record) -> Result<(), String>) -> Decision {
    let skip = |why: String| Decision::Skip(why);
    if facts.compact_in_flight {
        return skip("an idle compact's /compact is in flight".into());
    }
    if facts.compact_sent {
        return skip("an idle compact went in this stretch".into());
    }
    let record = match &facts.record {
        Err(why) => return skip(why.clone()),
        Ok(None) => return Decision::Nothing,
        Ok(Some(record)) => record,
    };
    if let Some(why) = stale(record, facts.now) {
        return Decision::Forget(why);
    }
    if facts.sent_for == Some(record.at) {
        return Decision::Nothing;
    }
    match facts.nirvana {
        Some(true) => {}
        Some(false) => return skip("the pane does not run Nirvana Claude".into()),
        None => return skip("the pane's process could not be read".into()),
    }
    if record.by != "owner" {
        match (record.transcript.as_deref(), facts.transcript.as_deref()) {
            (Some(want), Some(have)) if same_transcript(Path::new(want), have) => {}
            (None, _) => return skip("the record names no transcript".into()),
            (_, None) => return skip("the pane's transcript could not be told".into()),
            (Some(_), Some(_)) => return skip("the pane's transcript is not the record's".into()),
        }
    }
    if facts.state != AgentState::Idle {
        return skip(format!("the pane is {:?}, not idle", facts.state));
    }
    if let Some(why) = crate::nirvana_idle_compact::owner_quiet_breach(&facts.owner_key) {
        return skip(why);
    }
    match &facts.input_line {
        InputLine::Empty => match recheck(record) {
            Ok(()) => Decision::Send(record.at),
            Err(why) => Decision::Forget(why),
        },
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

/// `herdr nirvana digest`'s call: the note to print after the digest, on a `/clear` with a record for the pane of
/// the hook's own project.
pub fn session_start_hook(hook: &Value, project: Option<&Project>) -> Option<String> {
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return None;
    }
    let source = hook.get("source").and_then(Value::as_str)?;
    let (home, pane) = (crate::nirvana::home()?, pane_env()?);
    let transcript = hook
        .get("transcript_path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .map(Path::new);
    on_session_start_in(
        &home,
        &pane,
        source,
        project.map(|project| project.root.as_path()),
        transcript,
        nirvana_run::now(),
    )
}

/// Review S2: a named pane is the caller's to clear only when it works in the caller's project; `target` is that
/// pane's project, `None` when it could not be read (a doubt: refused). Pure.
pub fn same_project(target: Option<&Path>, caller: &Path) -> Result<(), String> {
    match target {
        None => Err("its folder could not be read".into()),
        Some(target) if crate::nirvana_spaces::same_folder(target, caller) => Ok(()),
        Some(target) => Err(format!(
            "it works in {}, not in this project {}",
            target.display(),
            caller.display()
        )),
    }
}

/// A pane's project, from the server (`pane.get`): its foreground's folder, else its own, taken up to the git top
/// level as `herdr nirvana stop` takes the hook's.
fn pane_project(pane: &str) -> Option<PathBuf> {
    let request = crate::api::schema::Request {
        id: "nirvana:fresh".into(),
        method: crate::api::schema::Method::PaneGet(crate::api::schema::PaneTarget {
            pane_id: pane.to_owned(),
        }),
    };
    let value = crate::api::client::ApiClient::local()
        .request_value_with_timeout(&request, Duration::from_secs(5))
        .ok()?;
    let crate::api::schema::ResponseResult::PaneInfo { pane } =
        crate::api::client::parse_response_value(value).ok()?.result
    else {
        return None;
    };
    let folder = PathBuf::from(pane.foreground_cwd.or(pane.cwd)?);
    Some(crate::nirvana_board::root(&folder).unwrap_or(folder))
}

/// The owner's record for `pane`: the project's last run the pane ended, and no transcript nor context (review S2:
/// the folder's session record names whichever of its Nirvana panes wrote last, so neither is guessed from it).
pub fn owner_record(
    pane: &str,
    project: &Project,
    runs: &[(String, RunState)],
    now: u64,
) -> Record {
    let last = last_end_of(pane, runs);
    Record {
        run: last.map(|(run, _)| run.to_owned()),
        ended_at: last.map(|(_, at)| at),
        context: None,
        transcript: None,
        project: project.root.display().to_string(),
        at: now,
        by: "owner".into(),
    }
}

/// `herdr nirvana fresh [<pane>]` (§2: "does the same on demand, for the owner"): a pending record for the pane, its
/// own `HERDR_PANE_ID` when none is named. The server types `/clear` once the pane is idle and the owner quiet.
///
/// Review S2: the record names no transcript (`owner_record`), so the note's prompts come from the transcript the
/// SessionStart hook reports. A named pane in another project is refused.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let named = args.first().cloned();
    let Some(pane) = named.clone().or_else(pane_env) else {
        eprintln!("usage: herdr nirvana fresh [<pane>]  (inside a pane, its own HERDR_PANE_ID)");
        std::process::exit(2);
    };
    let Some(home) = crate::nirvana::home() else {
        eprintln!("herdr nirvana fresh: no Nirvana home");
        std::process::exit(2);
    };
    let project = Project::from_env();
    if named.is_some() {
        if let Err(why) = same_project(pane_project(&pane).as_deref(), &project.root) {
            eprintln!("herdr nirvana fresh: not pane {pane}: {why}");
            std::process::exit(2);
        }
    }
    let record = owner_record(&pane, &project, &run_states(&project), nirvana_run::now());
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
