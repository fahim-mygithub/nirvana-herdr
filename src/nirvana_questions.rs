//! Nirvana fork, plan 42 P42a: the Questions record, what the agents need from the owner, and `herdr ask
//! add|answer|withdraw|list`. (nirvana/plans/2026-11-14-42-questions-square.md §5.1, §5.2, §6)
//!
//! The owner, 2026-11-14: "Sometimes the agent has a list of questions that I end up keep having to ask over an dover
//! becuases it gets buried." Run `ia1` measured it: about 15 questions asked in more than one turn, one scope cut asked
//! in 10 turns over 14.2 hours and never answered, a merge the owner said yes to 4 times and that stayed listed as
//! open because "an answer has no place to be recorded" (plan §3).
//!
//! - **The record** is `<project>/.nirvana/questions.jsonl`: append-only, one item a line, the latest line for an id
//!   winning. Append-only like the run journal, so two writers never lose each other's line and the history of an
//!   item (asked, asked again, answered) stays readable. Every write takes `questions.lock`, an OS file lock the OS
//!   lets go when its holder dies, as `journal.lock` does.
//! - **No second item for one question.** The same text (case and spacing aside), or `--same q7`, raises `asked_count`
//!   on the item that exists. A question asked again after it was answered prints the answer: run `ia1`'s merge was
//!   answered 4 times because nothing showed the orchestrator it already had been.
//! - **Who answered** is kept apart (§5.3): `owner` only from the window or the phone (P42b, P42c call
//!   [`answer_as_owner`]); `orchestrator` from `herdr ask answer`, which is the orchestrator's reading of a typed
//!   prompt; `journal` when a worker's question was answered in its run by a later order. The CLI has no way to write
//!   `owner`: an agent can fake typing (the `feat/owner-override` finding), so herdr marks only its own key press or
//!   phone tap as the owner's.
//! - **Workers' questions are mirrored** from each run's `asked` events, on read (see [`mirror`] for why there).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::nirvana_run::{self, Kind, Phase, Project};

pub const FILE: &str = "questions.jsonl";
const LOCK: &str = "questions.lock";
/// Which journal bytes the mirror has read, per run: a cache only. Losing it costs one full re-read and makes no
/// second item, because each item keeps the `run#seq` of the events it came from.
const CURSOR: &str = "questions.cursor.json";
/// §5.1: "`text`: one line, at most 200 chars". A question that needs more is two questions or a doc.
pub const TEXT_MAX: usize = 200;
/// The `source` of an item the orchestrator filed; any other source is a worker's order id.
pub const ORCHESTRATOR: &str = "orchestrator";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Open,
    Answered,
    Withdrawn,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Answered => "answered",
            Self::Withdrawn => "withdrawn",
        }
    }
}

/// Who recorded an answer (§5.3). Only [`answer_as_owner`] makes `Owner`; nothing the CLI parses reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// A key press in the window or a tap on the phone: herdr's own input, which an agent cannot fake.
    Owner,
    /// `herdr ask answer`: the orchestrator's reading of what the owner typed to it.
    Orchestrator,
    /// The run's journal shows the asking order went on, or a later order answered it (`nirvana_dead::answered`).
    Journal,
}

/// One question (§5.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// `orchestrator`, or the order id of the worker that asked.
    pub source: String,
    pub text: String,
    /// A worker's question in full when `text` had to be cut to one line of `TEXT_MAX`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    /// One of `options`; questions that carry one got one-word answers in run `ia1` ("Go with recommended", 5 times).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<String>,
    /// The orders or steps that wait on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<String>,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered_by: Option<By>,
    pub asked_at: u64,
    #[serde(default)]
    pub last_asked_at: u64,
    #[serde(default = "one")]
    pub asked_count: u32,
    /// `run#seq` of each journal `asked` event mirrored into this item: the mirror's dedupe key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal: Vec<String>,
}

fn one() -> u32 {
    1
}

/// The record as read: one item per id in the order the ids first appeared, and the lines that did not parse
/// (kept in the file, never rewritten).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Questions {
    pub items: Vec<Item>,
    pub bad_lines: usize,
}

impl Questions {
    pub fn get(&self, id: &str) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    pub fn open(&self) -> Vec<&Item> {
        self.items
            .iter()
            .filter(|item| item.state == State::Open)
            .collect()
    }

    /// What the square shows (§5.5): the open count and the oldest open item's age.
    pub fn summary(&self, now: u64) -> Summary {
        let open = self.open();
        Summary {
            open: open.len(),
            oldest_age_s: open
                .iter()
                .map(|item| now.saturating_sub(item.asked_at))
                .max(),
        }
    }

    fn next_id(&self) -> String {
        format!("q{}", top_number(&self.items) + 1)
    }

    /// The item a new ask of `text` from `source` repeats: same source, same text once case and spacing are set
    /// aside, not withdrawn. A withdrawn question asked again is a new one: it was taken back.
    fn same_text(&self, source: &str, text: &str) -> Option<&Item> {
        let key = normal(text);
        self.items.iter().rev().find(|item| {
            item.state != State::Withdrawn && item.source == source && normal(&item.text) == key
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub open: usize,
    pub oldest_age_s: Option<u64>,
}

/// The highest `q<n>` in use: ids are never reused, a withdrawn one included.
fn top_number(items: &[Item]) -> u64 {
    items
        .iter()
        .filter_map(|item| item.id.strip_prefix('q')?.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}

/// Case and runs of whitespace set aside, and a trailing `?` or `.`: "Merge ia1 to main?" and "merge  ia1 to main"
/// are one question.
pub fn normal(text: &str) -> String {
    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
    words
        .join(" ")
        .trim_end_matches(['?', '.', '!'])
        .trim_end()
        .to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskError {
    /// Bad input: exit 2.
    Invalid(String),
    NotFound(String),
    /// The item is answered or withdrawn: exit 1.
    NotOpen {
        id: String,
        state: State,
    },
    Io(String),
}

impl std::fmt::Display for AskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(why) => write!(f, "{why}"),
            Self::NotFound(id) => write!(f, "no question {id}: `herdr ask list --all` lists them"),
            Self::NotOpen { id, state } => write!(f, "{id} is {}, not open", state.word()),
            Self::Io(why) => write!(f, "{why}"),
        }
    }
}

impl From<std::io::Error> for AskError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl AskError {
    pub fn code(&self) -> i32 {
        match self {
            Self::Invalid(_) => 2,
            Self::NotFound(_) | Self::NotOpen { .. } | Self::Io(_) => 1,
        }
    }
}

// ---- the file ----

pub fn path(project: &Project) -> PathBuf {
    project.dir().join(FILE)
}

pub fn parse(text: &str) -> Questions {
    let mut items: Vec<Item> = Vec::new();
    let mut bad_lines = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match serde_json::from_str::<Item>(line.trim()) {
            // The latest line for an id wins, at the place the id first appeared, so the list keeps asking order.
            Ok(item) => match items.iter_mut().find(|known| known.id == item.id) {
                Some(known) => *known = item,
                None => items.push(item),
            },
            Err(_) => bad_lines += 1,
        }
    }
    Questions { items, bad_lines }
}

/// The record as it is on disk, with no mirroring: what a reader that must not write calls.
pub fn load(project: &Project) -> Questions {
    parse(&std::fs::read_to_string(path(project)).unwrap_or_default())
}

/// The record after the runs' worker questions are mirrored into it: what `herdr ask list` and the square read.
pub fn refresh(project: &Project, now: u64) -> Result<Questions, AskError> {
    mirror(project, now)?;
    Ok(load(project))
}

struct Lock(std::fs::File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn lock(project: &Project) -> std::io::Result<Lock> {
    nirvana_run::ensure_dir(&project.dir())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(project.dir().join(LOCK))?;
    file.lock()?;
    Ok(Lock(file))
}

/// Reads the record and appends what `decide` returns, under the lock, so a check (is this text asked already?)
/// and the line it allows are one step.
fn transact<R>(
    project: &Project,
    decide: impl FnOnce(&Questions) -> Result<(Vec<Item>, R), AskError>,
) -> Result<R, AskError> {
    let _lock = lock(project)?;
    let questions = load(project);
    let (lines, out) = decide(&questions)?;
    if !lines.is_empty() {
        let mut bytes = String::new();
        for item in &lines {
            bytes.push_str(&serde_json::to_string(item).map_err(|e| AskError::Io(e.to_string()))?);
            bytes.push('\n');
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path(project))?
            .write_all(bytes.as_bytes())?;
    }
    Ok(out)
}

// ---- adding ----

/// What `herdr ask add` asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ask {
    pub text: String,
    pub options: Vec<String>,
    pub recommended: Option<String>,
    pub blocks: Vec<String>,
    pub same: Option<String>,
    pub run: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Added {
    New(Item),
    /// Asked before and still open: its count was raised.
    Again(Item),
    /// Asked before and answered: the answer, so the orchestrator stops asking.
    Answered(Item),
}

impl Added {
    pub fn item(&self) -> &Item {
        match self {
            Self::New(item) | Self::Again(item) | Self::Answered(item) => item,
        }
    }
}

fn one_line(what: &str, text: &str) -> Result<(), AskError> {
    if text.trim().is_empty() {
        return Err(AskError::Invalid(format!("{what} is empty")));
    }
    if text.contains(['\n', '\r']) {
        return Err(AskError::Invalid(format!("{what} must be one line")));
    }
    Ok(())
}

/// Checks an ask before any write: one line of at most `TEXT_MAX`, options one line and distinct, the recommended
/// one among them, blocks ids.
pub fn check(ask: &Ask) -> Result<(), AskError> {
    one_line("the question", &ask.text)?;
    let chars = ask.text.trim().chars().count();
    if chars > TEXT_MAX {
        return Err(AskError::Invalid(format!(
            "the question is {chars} chars; at most {TEXT_MAX}: ask the one decision, put the rest in a doc"
        )));
    }
    for option in &ask.options {
        one_line("an option", option)?;
    }
    for (i, option) in ask.options.iter().enumerate() {
        if ask.options[..i]
            .iter()
            .any(|other| other.trim() == option.trim())
        {
            return Err(AskError::Invalid(format!(
                "option {option:?} is given twice"
            )));
        }
    }
    if let Some(recommended) = &ask.recommended {
        if !ask
            .options
            .iter()
            .any(|option| option.trim() == recommended.trim())
        {
            return Err(AskError::Invalid(format!(
                "--recommended {recommended:?} is not one of the --option values"
            )));
        }
    }
    for block in &ask.blocks {
        if !nirvana_run::valid_id(block) {
            return Err(AskError::Invalid(format!(
                "--blocks {block:?} is not an order or step id ([A-Za-z0-9._-], 1 to 64)"
            )));
        }
    }
    if let Some(same) = &ask.same {
        if !nirvana_run::valid_id(same) {
            return Err(AskError::Invalid(format!(
                "--same {same:?} is not a question id"
            )));
        }
    }
    Ok(())
}

fn union(into: &mut Vec<String>, more: &[String]) {
    for value in more {
        if !into.contains(value) {
            into.push(value.clone());
        }
    }
}

/// `herdr ask add`: the orchestrator files a question.
pub fn add(project: &Project, ask: &Ask, now: u64) -> Result<Added, AskError> {
    check(ask)?;
    transact(project, |questions| {
        let known = match &ask.same {
            Some(id) => Some(
                questions
                    .get(id)
                    .ok_or_else(|| AskError::NotFound(id.clone()))?,
            ),
            None => questions.same_text(ORCHESTRATOR, &ask.text),
        };
        if let Some(known) = known {
            if known.state == State::Withdrawn {
                return Err(AskError::NotOpen {
                    id: known.id.clone(),
                    state: known.state,
                });
            }
            let mut item = known.clone();
            item.asked_count += 1;
            item.last_asked_at = now;
            union(&mut item.blocks, &ask.blocks);
            if item.state == State::Open && !ask.options.is_empty() {
                item.options = ask.options.iter().map(|o| o.trim().to_owned()).collect();
                item.recommended = ask.recommended.as_ref().map(|r| r.trim().to_owned());
            }
            let added = if item.state == State::Answered {
                Added::Answered(item.clone())
            } else {
                Added::Again(item.clone())
            };
            return Ok((vec![item], added));
        }
        let item = Item {
            id: questions.next_id(),
            run: ask.run.clone(),
            source: ORCHESTRATOR.into(),
            text: ask.text.trim().to_owned(),
            detail: None,
            options: ask.options.iter().map(|o| o.trim().to_owned()).collect(),
            recommended: ask.recommended.as_ref().map(|r| r.trim().to_owned()),
            blocks: ask.blocks.clone(),
            state: State::Open,
            answer: None,
            answered_at: None,
            answered_by: None,
            asked_at: now,
            last_asked_at: now,
            asked_count: 1,
            journal: Vec::new(),
        };
        Ok((vec![item.clone()], Added::New(item)))
    })
}

// ---- answering ----

fn settle(
    project: &Project,
    id: &str,
    change: impl FnOnce(&Item) -> Result<Item, AskError>,
) -> Result<Item, AskError> {
    transact(project, |questions| {
        let known = questions
            .get(id)
            .ok_or_else(|| AskError::NotFound(id.to_owned()))?;
        let item = change(known)?;
        Ok((vec![item.clone()], item))
    })
}

fn answer_with(
    project: &Project,
    id: &str,
    answer: &str,
    by: By,
    now: u64,
) -> Result<Item, AskError> {
    one_line("the answer", answer)?;
    settle(project, id, |known| {
        // The owner may correct the orchestrator's reading or the journal's guess; nobody else re-answers.
        let may = known.state == State::Open
            || (by == By::Owner
                && known.state == State::Answered
                && known.answered_by != Some(By::Owner));
        if !may {
            return Err(AskError::NotOpen {
                id: known.id.clone(),
                state: known.state,
            });
        }
        let mut item = known.clone();
        item.state = State::Answered;
        item.answer = Some(answer.trim().to_owned());
        item.answered_by = Some(by);
        item.answered_at = Some(now);
        Ok(item)
    })
}

/// `herdr ask answer`: the orchestrator records its reading of what the owner typed. Never `By::Owner`.
pub fn answer(project: &Project, id: &str, answer: &str, now: u64) -> Result<Item, AskError> {
    answer_with(project, id, answer, By::Orchestrator, now)
}

/// The window's key press or the phone's tap (P42b, P42c). Not reachable from `herdr ask`: §5.3, only herdr's own
/// input counts as the owner.
#[allow(dead_code)] // P42b and P42c call it.
pub fn answer_as_owner(
    project: &Project,
    id: &str,
    answer: &str,
    now: u64,
) -> Result<Item, AskError> {
    answer_with(project, id, answer, By::Owner, now)
}

/// `herdr ask withdraw`: the asker takes an open question back.
pub fn withdraw(project: &Project, id: &str, now: u64) -> Result<Item, AskError> {
    settle(project, id, |known| {
        if known.state != State::Open {
            return Err(AskError::NotOpen {
                id: known.id.clone(),
                state: known.state,
            });
        }
        let mut item = known.clone();
        item.state = State::Withdrawn;
        item.answered_at = Some(now);
        Ok(item)
    })
}

// ---- workers' questions ----

/// `text` cut to one line of `TEXT_MAX` chars, and the full question when it had to be cut.
fn line_of(question: &str) -> (String, Option<String>) {
    let flat = question.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= TEXT_MAX && !question.trim().contains('\n') {
        return (flat, None);
    }
    let cut: String = flat.chars().take(TEXT_MAX - 1).collect();
    (
        format!("{}…", cut.trim_end()),
        Some(question.trim().to_owned()),
    )
}

/// Whether the run still holds `order`'s question open: the Blocked row's own rule (`nirvana_dead::order_shown`).
/// It is open while the order sits at Asked, no later order answers it and no child continues it.
fn still_asked(state: &nirvana_run::RunState, order: &str) -> Result<(), String> {
    let Some(known) = state.order(order) else {
        return Ok(());
    };
    if let Some(answerer) = state.orders.iter().find(|other| {
        other.answers.as_deref() == Some(order) || other.seated.parent.as_deref() == Some(order)
    }) {
        return Err(format!(
            "answered in the run: order {} took it up",
            answerer.id()
        ));
    }
    if known.phase != Phase::Asked {
        return Err(format!(
            "answered in the run: the order went on ({})",
            known.phase.word()
        ));
    }
    Ok(())
}

/// Mirrors every worker `asked` event of the project's runs into the record (§5.2 item 2), and settles a mirrored
/// item the run shows answered.
///
/// **Why on read, not where the journal is written.** An `asked` event is written from several places (the seat
/// supervisor, the gate's `asked` verdict, the wake cap) and by whatever herdr build wrote that journal; a hook in
/// each writer would miss the next one added and every journal written before this package. Reading covers them all
/// in one place. It stays cheap: a cursor keeps each journal's length, so an unchanged journal costs one `stat`; a
/// changed one with no `"asked"` in it is skipped before it is parsed; only a changed journal that asked is replayed
/// (run `ia1`'s 4.6 MB journal, the largest on this machine, is measured in the package report).
///
/// The run's own question with no order (the wake cap's) is not mirrored: it is herdr's, and P42d answers it on the
/// orchestrator's prompt. Review orders are skipped, as the run's replay skips them: their question is about another
/// order.
pub fn mirror(project: &Project, now: u64) -> Result<usize, AskError> {
    let runs = project.runs();
    if runs.is_empty() {
        return Ok(0);
    }
    let cursor_path = project.dir().join(CURSOR);
    let mut cursor: BTreeMap<String, u64> = std::fs::read_to_string(&cursor_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let mut changed = Vec::new();
    for run in runs {
        let journal = project.journal(&run);
        let len = std::fs::metadata(&journal.path)
            .map(|m| m.len())
            .unwrap_or(0);
        if cursor.get(&run) == Some(&len) {
            continue;
        }
        changed.push((run, journal, len));
    }
    if changed.is_empty() {
        return Ok(0);
    }
    let written = transact(project, |questions| {
        let mut items = questions.items.clone();
        let mut out: Vec<usize> = Vec::new();
        for (run, journal, _) in &changed {
            let text = std::fs::read_to_string(&journal.path).unwrap_or_default();
            if !text.contains("\"asked\"") {
                continue;
            }
            let (events, _) = nirvana_run::parse_lines(&text);
            let state = nirvana_run::replay(&events);
            for event in &events {
                let Kind::Asked {
                    order: Some(order),
                    question,
                } = &event.kind
                else {
                    continue;
                };
                if state
                    .order(order)
                    .is_some_and(|known| nirvana_run::reviews(&known.seated.class))
                {
                    continue;
                }
                let key = format!("{run}#{}", event.seq);
                if items.iter().any(|item| item.journal.contains(&key)) {
                    continue;
                }
                let (line, detail) = line_of(question);
                let repeat = items.iter().rposition(|item| {
                    item.state != State::Withdrawn
                        && item.source == *order
                        && normal(&item.text) == normal(&line)
                });
                match repeat {
                    Some(i) => {
                        items[i].asked_count += 1;
                        items[i].last_asked_at = event.at;
                        items[i].journal.push(key);
                        out.push(i);
                    }
                    None => {
                        items.push(Item {
                            id: format!("q{}", top_number(&items) + 1),
                            run: Some(run.clone()),
                            source: order.clone(),
                            text: line,
                            detail,
                            options: Vec::new(),
                            recommended: None,
                            blocks: vec![order.clone()],
                            state: State::Open,
                            answer: None,
                            answered_at: None,
                            answered_by: None,
                            asked_at: event.at,
                            last_asked_at: event.at,
                            asked_count: 1,
                            journal: vec![key],
                        });
                        out.push(items.len() - 1);
                    }
                }
            }
            // Settle the open worker items of this run the journal shows answered, so a question answered by a
            // continuation order does not sit open on the square after its Blocked row is gone.
            for (i, item) in items.iter_mut().enumerate() {
                if item.state != State::Open
                    || item.source == ORCHESTRATOR
                    || item.run.as_deref() != Some(run.as_str())
                {
                    continue;
                }
                if let Err(why) = still_asked(&state, &item.source) {
                    item.state = State::Answered;
                    item.answer = Some(why);
                    item.answered_by = Some(By::Journal);
                    item.answered_at = Some(now);
                    out.push(i);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        let lines: Vec<Item> = out.iter().map(|&i| items[i].clone()).collect();
        let count = lines.len();
        Ok((lines, count))
    })?;
    // The cursor moves only after the items are on disk: a crash between the two re-reads a journal, which the
    // `run#seq` keys make harmless.
    for (run, _, len) in changed {
        cursor.insert(run, len);
    }
    let _ = std::fs::write(
        &cursor_path,
        serde_json::to_string(&cursor).unwrap_or_default(),
    );
    Ok(written)
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr ask add \"<question>\" [--option <a> --option <b> [--recommended <a>]] [--blocks <id>]... [--same <q>] [--run <run>]\n       herdr ask answer <q> \"<answer>\"\n       herdr ask withdraw <q>\n       herdr ask list [--all] [--json]\n\nOne line per question, at most 200 chars. Asking the same text again, or with --same, raises its count and makes no\nsecond item. `ask answer` records your reading of what the owner typed; only the window and the phone record the owner.";

/// What one `herdr ask` call prints and exits with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Outcome {
    fn ok(out: String) -> Self {
        Self {
            code: 0,
            out,
            err: String::new(),
        }
    }

    fn fail(err: &AskError) -> Self {
        Self {
            code: err.code(),
            out: String::new(),
            err: format!("herdr ask: {err}\n"),
        }
    }

    fn usage(why: &str) -> Self {
        Self {
            code: 2,
            out: String::new(),
            err: format!("herdr ask: {why}\n{USAGE}\n"),
        }
    }
}

/// How long ago, in one short word: `40s`, `12m`, `3h`, `2d`.
pub fn age_word(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

fn item_line(item: &Item, now: u64) -> String {
    let mut line = format!(
        "{}  {}  [{}] {}",
        item.id,
        item.state.word(),
        item.source,
        item.text
    );
    if !item.options.is_empty() {
        let options: Vec<String> = item
            .options
            .iter()
            .map(|option| {
                if item.recommended.as_deref() == Some(option.as_str()) {
                    format!("{option} (recommended)")
                } else {
                    option.clone()
                }
            })
            .collect();
        line.push_str(&format!("  options: {}", options.join(" | ")));
    }
    if !item.blocks.is_empty() {
        line.push_str(&format!("  blocks: {}", item.blocks.join(", ")));
    }
    line.push_str(&format!(
        "  asked {}x, {} ago",
        item.asked_count,
        age_word(now.saturating_sub(item.asked_at))
    ));
    if let (Some(answer), Some(by)) = (&item.answer, item.answered_by) {
        let by = match by {
            By::Owner => "owner",
            By::Orchestrator => "orchestrator",
            By::Journal => "journal",
        };
        line.push_str(&format!("  -> {answer} (by {by})"));
    }
    line
}

/// Takes `--flag <value>`; a missing value is a usage error, never a silent default.
fn value<'a>(iter: &mut std::slice::Iter<'a, String>, flag: &str) -> Result<&'a String, Outcome> {
    iter.next()
        .filter(|v| !v.starts_with("--"))
        .ok_or_else(|| Outcome::usage(&format!("{flag} needs a value")))
}

/// `herdr ask ...` against `project` at `now`, as a value: the tests drive the CLI's parsing through this.
pub fn execute(project: &Project, args: &[String], now: u64) -> Outcome {
    match run_args(project, args, now) {
        Ok(outcome) | Err(outcome) => outcome,
    }
}

fn run_args(project: &Project, args: &[String], now: u64) -> Result<Outcome, Outcome> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(Outcome::usage("name a verb"));
    };
    let mut words: Vec<&String> = Vec::new();
    let mut ask = Ask::default();
    let mut all = false;
    let mut json = false;
    let mut iter = rest.iter();
    while let Some(arg) = iter.next() {
        // Every flag is listed: an unknown one (a `--by owner`, say) is refused, never taken as the question's text.
        match arg.as_str() {
            "--option" if verb == "add" => ask.options.push(value(&mut iter, arg)?.clone()),
            "--recommended" if verb == "add" => {
                ask.recommended = Some(value(&mut iter, arg)?.clone())
            }
            "--blocks" if verb == "add" => ask.blocks.push(value(&mut iter, arg)?.clone()),
            "--same" if verb == "add" => ask.same = Some(value(&mut iter, arg)?.clone()),
            "--run" if verb == "add" => ask.run = Some(value(&mut iter, arg)?.clone()),
            "--all" if verb == "list" => all = true,
            "--json" if verb == "list" => json = true,
            "-h" | "--help" => return Ok(Outcome::ok(format!("{USAGE}\n"))),
            flag if flag.starts_with("--") => {
                return Err(Outcome::usage(&format!("{verb} takes no {flag}")))
            }
            _ => words.push(arg),
        }
    }
    let fail = |err: AskError| Outcome::fail(&err);
    match verb.as_str() {
        "add" => {
            let [text] = words.as_slice() else {
                return Err(Outcome::usage("add takes one quoted question"));
            };
            ask.text = (*text).clone();
            if ask
                .run
                .as_deref()
                .is_some_and(|run| !nirvana_run::valid_id(run))
            {
                return Err(fail(AskError::Invalid("--run is not a run id".into())));
            }
            // The run the question belongs to: the one named, else the project's latest, as `herdr run` picks.
            if ask.run.is_none() {
                ask.run = project.runs().pop();
            }
            let added = add(project, &ask, now).map_err(fail)?;
            let item = added.item();
            Ok(Outcome::ok(match &added {
                Added::New(_) => format!("{} open\n", item.id),
                Added::Again(_) => format!(
                    "{} open, asked {} times: do not repeat it in prose, the square shows it\n",
                    item.id, item.asked_count
                ),
                Added::Answered(_) => format!(
                    "{} was answered: {}\n",
                    item.id,
                    item.answer.as_deref().unwrap_or("")
                ),
            }))
        }
        "answer" => {
            let [id, text] = words.as_slice() else {
                return Err(Outcome::usage(
                    "answer takes a question id and one quoted answer",
                ));
            };
            let item = answer(project, id, text, now).map_err(fail)?;
            Ok(Outcome::ok(format!("{} answered\n", item.id)))
        }
        "withdraw" => {
            let [id] = words.as_slice() else {
                return Err(Outcome::usage("withdraw takes a question id"));
            };
            let item = withdraw(project, id, now).map_err(fail)?;
            Ok(Outcome::ok(format!("{} withdrawn\n", item.id)))
        }
        "list" => {
            if !words.is_empty() {
                return Err(Outcome::usage("list takes no words"));
            }
            let questions = refresh(project, now).map_err(fail)?;
            let shown: Vec<&Item> = questions
                .items
                .iter()
                .filter(|item| all || item.state == State::Open)
                .collect();
            if json {
                let text = serde_json::to_string_pretty(&shown)
                    .map_err(|e| fail(AskError::Io(e.to_string())))?;
                return Ok(Outcome::ok(format!("{text}\n")));
            }
            let mut out = String::new();
            for item in &shown {
                out.push_str(&item_line(item, now));
                out.push('\n');
            }
            let summary = questions.summary(now);
            out.push_str(&match summary.oldest_age_s {
                Some(age) => format!("{} open, the oldest {} old\n", summary.open, age_word(age)),
                None => "no open questions\n".to_owned(),
            });
            if questions.bad_lines > 0 {
                out.push_str(&format!(
                    "{} unreadable lines in {FILE}, kept\n",
                    questions.bad_lines
                ));
            }
            Ok(Outcome::ok(out))
        }
        other => Err(Outcome::usage(&format!("unknown verb {other:?}"))),
    }
}

/// `herdr ask ...`.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let outcome = execute(&Project::from_env(), args, nirvana_run::now());
    std::io::stdout().write_all(outcome.out.as_bytes())?;
    std::io::stderr().write_all(outcome.err.as_bytes())?;
    if outcome.code != 0 {
        std::process::exit(outcome.code);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
