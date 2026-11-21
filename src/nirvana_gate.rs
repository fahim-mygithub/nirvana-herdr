//! Nirvana fork, plan 12 O5: the completion gate. `herdr gate run|author|merge|status`, deterministic and calling no
//! model. (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §7.3; old Nirvana decisions 0017, 0019)
//!
//! The owner, 2026-10-17: "The goal is to utilize both my claude subscription and my agy gemini subsritpion to buid
//! better apps" (plan 12 L1). Better apps is what the gate holds a delivery to.
//!
//! `herdr gate run <order>` rules on a delivered order, in this order (§7.3):
//! 1. **rebase**: the order's branch onto the tip of the run's integration branch `nirvana-integrate/<run>`, so two
//!    orders that each pass alone cannot break the build together (Argon's review F6). The worker's uncommitted work
//!    is committed first, since a rebase cannot carry a dirty tree. A conflict sends it back with the files.
//! 2. **tests**: the order's `test:` command in its worktree. The output is filtered, and every `check:` the order
//!    names must be in it: a suite total cannot see a check that never ran (Nirvana CLAUDE.md rule 9).
//! 3. **report**: the Work Report against O3's schema; an empty or missing report fails, and so does `blocked`.
//! 4. **owned**: only the files the order's `owns:` lines name changed.
//! 5. **served**, for `ops.terminal` orders: each `serves:` port is listened on by a process in the worker's own pane
//!    (decision 0041 R6; old Nirvana's incident of 2026-10-08: a worker found a parallel session's listener on 8770,
//!    reported `done`, and the gate passed it).
//!
//! Then **held-out**, for a feature-size order: a check another family wrote before the build (decision 0019, and
//! plan 12 §14: Argon's review asked to cut it; not taken, since old Nirvana's M5 row passed 8 of 8 with it against
//! 4 of 8 without). The builder sees only the failing case ids. Then **review**, for a load-bearing order: a reviewer
//! of another family (decision 0017), whose verdict comes back as its Work Report.
//!
//! A failing axis is a **send-back**: a child order `<root>-s<n>` in the same worktree with the reasons, at most 3 per
//! order; the fourth failure **asks** the owner. Old Nirvana's decision 0010: a continuation is a child order. A
//! `blocked` delivery is never sent back: it asks, the block first (plan 37b).
//!
//! The order file's gate lines, each at the start of a line, anywhere in the order, any number of times:
//!
//! ```text
//! owns: src/calc.py tests/**/*.py      paths from the repository root; `*` within a name, `**` across folders,
//!                                      a trailing `/` the whole folder
//! test: python -m pytest -q -rA tests  one command, run without a shell, in the worker's folder
//! check: test_mul test_add             names that must appear in the test output
//! serves: 8770                         ops orders: ports the worker's pane must listen on
//! Profile: slicer                      the profile the order was seated with (plan 33 §3.2; P33a writes it)
//! ```
//!
//! After a pass or a send-back, one `lesson-review` order of the gated order is seated (plan 33 §3.3,
//! `nirvana_review`), and gating that order applies its lesson ops.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::nirvana_run::{self, now, replay, Journal, Kind, OrderState, Phase, Project, Seated};
use crate::nirvana_seat::{self, Blast, NoSeat, Pick, Request, SizeName, Table, World};

/// §7.3: "At most 3 send-backs per order; then the owner is asked."
pub const SEND_BACK_CAP: u32 = 3;
/// A test command running longer than this is stopped and fails the axis. A hypothesis: the largest suite herdr
/// gates today, the fork's own nextest run, takes about 9 minutes on this PC.
pub const TEST_TIMEOUT_S: u64 = 1200;
/// The run's integration branch. Not under `nirvana/`, where O3 names each order's branch: `nirvana/<order>` and
/// `nirvana/<run>/...` could not both exist when an order is named like a run.
pub const INTEGRATION_PREFIX: &str = "nirvana-integrate/";
/// The filtered output kept in an axis and in a send-back's brief.
const KEPT_LINES: usize = 40;
/// Held-out authors per order: one retry with the reason, then the build goes on without (decision 0019 §2.3).
const AUTHOR_TRIES: usize = 2;
/// `herdr gate run` exits with this while a review or a held-out check is still to come.
pub const WAIT_EXIT: i32 = 4;
/// Plan 37 §1.2 (L4): YouTube's oEmbed, asked whether each cited video exists. MotoProject, 2026-10-31: 6 of 6
/// invented ids answered 404 and 48 of 48 real ids 200. Reddit is not asked: it answered 403 and 429.
pub const OEMBED_URL: &str = "https://www.youtube.com/oembed";
/// Per call. A hypothesis: no latency of the 54 measured calls was kept.
pub const OEMBED_TIMEOUT_S: u64 = 4;
/// Calls per gate. reel-motion-gemini-r2 cited 10 videos, the most measured; a cap keeps a long list from holding the
/// gate lock for minutes.
pub const OEMBED_CALLS_PER_GATE: usize = 20;
/// All calls of one gate together (review N3): 20 slow answers inside the 4 s timeout would hold the gate lock 80 s.
/// A hypothesis, as the timeout is; a call never waits past what is left of it.
pub const OEMBED_BUDGET_S: u64 = 30;

// ---- the outcome words ----

/// The words the gate's `gated` events, the record and the tally use. O3 left the gate's words to this package
/// ("pass" is the gate's word; every other word counted as a failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The gate passed it.
    Pass,
    /// The gate found a failing axis and seated a child order with the reasons.
    SentBack,
    /// The order ended without a delivery the gate could pass: its worker failed, was cut, or it was never gated.
    Failed,
    /// It waits for the owner: the send-back cap, a worker's question, no seat left.
    Asked,
    /// Its pool ran out and a child on another seat carries the work (§5.6).
    Reseated,
    /// Plan 43 A1: refused for the brief's or the gate's fault alone; no child is seated, and the orchestrator is
    /// woken with the `herdr gate amend` line that re-gates the same branch.
    Amend,
}

impl Outcome {
    pub const ALL: [Self; 6] = [
        Self::Pass,
        Self::SentBack,
        Self::Failed,
        Self::Asked,
        Self::Reseated,
        Self::Amend,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::SentBack => "sent_back",
            Self::Failed => "failed",
            Self::Asked => "asked",
            Self::Reseated => "reseated",
            Self::Amend => "amend",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|outcome| outcome.word() == word)
    }

    /// Plan 37b review S1: the record's word for an order asked on its worker's own block. Its journal verdict stays
    /// `asked`; the record says `blocked`, which the tally does not count: a login wall is the owner's input, not the
    /// seat's failure. A worker that gave up early is not counted either: S3's hypothesis, unmeasured (nothing yet
    /// tells a real block from a give-up).
    pub const BLOCKED: &'static str = "blocked";

    /// Plan 40 addendum A: the record's word for an order the orchestrator accepted after the gate (`herdr run log`,
    /// verdict `accepted` with a reason). Its journal verdict is the same word; the tally counts it a pass.
    pub const ACCEPTED: &'static str = "accepted";

    /// Plan 43 fix round F6: the journal's word for an amend recorded before its re-gate (`gate_amend`). Its
    /// `gated` event carries the old and the new values; the re-gate's own verdict follows it. An order left here (the
    /// re-gate errored, or herdr died in it) is re-gated by `herdr gate run`. The tally does not count it.
    pub const AMENDING: &'static str = "amending";

    /// What the seat record's tally counts (decision 0029's blend): `Some(true)` a pass, `Some(false)` a failure,
    /// `None` not counted. A re-seat is not counted: the pool's window ran out, which says nothing of the seat's
    /// work, and its child's own line carries the outcome. Nor is a `blocked` one (`BLOCKED`). A word this herdr does
    /// not know (O2's "sent back", a later word) counts as a failure, as O2 counted it.
    pub fn tally(word: &str) -> Option<bool> {
        if word == Self::BLOCKED {
            return None;
        }
        // Plan 40 addendum A: the orchestrator took the delivery with a reason, which `nirvana_lessons` already counts
        // as a pass (`PASS_WORDS`). Read as a failure, 65 of 323 record lines called delivered work failed.
        if word == Self::ACCEPTED {
            return Some(true);
        }
        if word == Self::AMENDING {
            return None;
        }
        match Self::parse(word) {
            Some(Self::Pass) => Some(true),
            // Plan 43 A1: the brief's or the gate's fault says nothing of the seat; the amend's re-gate decides.
            Some(Self::Reseated | Self::Amend) => None,
            Some(Self::SentBack | Self::Failed | Self::Asked) | None => Some(false),
        }
    }
}

// ---- axes ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    Pass,
    /// Blocks: a send-back.
    Fail,
    /// Said, never blocking: an axis that does not apply, or a disclosure.
    Note,
    /// Not decided yet: a review or a held-out check to come.
    Wait,
}

/// Plan 43 A1: whose fault a failing axis is. Megajam run ia1 (2026-11-12 to 18): of 91 refusals 45 were right, 29
/// came from the orchestrator's brief and 17 from the gate itself, and the send-back child it seated for them was
/// stopped at 0 minutes 35 times. Only a worker fault seats a child; a refusal with no worker fault goes to the
/// orchestrator as a `herdr gate amend` line (`send_back`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    /// A worker can fix it: a failing test, a real conflict, a source it never read, a file another live order owns.
    Worker,
    /// The brief's: `owns:` too narrow with no live order holding the files, a `test:` that does not parse or start.
    Brief,
    /// The gate's own: herdr's files under `.nirvana/` breaking the rebase, a commit herdr could not make.
    Gate,
}

impl Fault {
    pub fn word(self) -> &'static str {
        match self {
            Self::Worker => "worker",
            Self::Brief => "brief",
            Self::Gate => "gate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Axis {
    pub axis: String,
    pub mark: Mark,
    pub why: String,
    /// The filtered output, the conflicting or unowned files: what the reader needs, never the raw output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// Plan 43 A1: a failing axis's fault, when the gate can tell; none reads as a worker's (`fault_of`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fault: Option<Fault>,
    /// Plan 43 A1: the `herdr gate amend` flags that would fix a brief fault, e.g. `--owns Docs/x/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amend: Option<String>,
}

impl Axis {
    pub(crate) fn new(axis: &str, mark: Mark, why: impl Into<String>) -> Self {
        Self {
            axis: axis.to_owned(),
            mark,
            why: why.into(),
            lines: Vec::new(),
            fault: None,
            amend: None,
        }
    }

    fn with(mut self, lines: Vec<String>) -> Self {
        self.lines = lines;
        self
    }

    fn fault(mut self, fault: Fault) -> Self {
        self.fault = Some(fault);
        self
    }
}

/// A failing axis's fault: what the gate said, else a worker's (an axis written before plan 43 carries none).
pub fn fault_of(axis: &Axis) -> Fault {
    axis.fault.unwrap_or(Fault::Worker)
}

/// Plan 43 A1: every failing axis is the brief's or the gate's, so no worker can fix the refusal.
pub fn no_worker_fault(axes: &[Axis]) -> bool {
    let failing = failing(axes);
    !failing.is_empty() && failing.iter().all(|axis| fault_of(axis) != Fault::Worker)
}

/// Plan 40 T1: the most file paths an axis keeps. The lines are printed in the ruling, journaled in the `Gated`
/// event and copied into a send-back's brief; the 2026-10 MotoProject incident (`order_changes`) listed 3,958 files,
/// about 100k tokens into one child's brief. 40 is `KEPT_LINES`, the test output's cap.
pub const FILE_LINES_MOST: usize = KEPT_LINES;

/// `paths`, at most `FILE_LINES_MOST` of them, then one `+N more` line for the rest.
pub fn capped(mut paths: Vec<String>) -> Vec<String> {
    if paths.len() > FILE_LINES_MOST {
        let more = paths.len() - FILE_LINES_MOST;
        paths.truncate(FILE_LINES_MOST);
        paths.push(format!("+{more} more"));
    }
    paths
}

pub fn failing(axes: &[Axis]) -> Vec<&Axis> {
    axes.iter().filter(|axis| axis.mark == Mark::Fail).collect()
}

// ---- the order's gate lines ----

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spec {
    pub owns: Vec<String>,
    pub test: Option<String>,
    pub checks: Vec<String>,
    pub serves: Vec<u16>,
    /// Plan 33 §3.2: `Profile: <name>`, the contract with P33a's seating; the last valid one wins.
    pub profile: Option<String>,
    /// herdr seated the order with `NO_WEB_SOURCES` (`web_asked` found no web in it): its sources axis is a note when
    /// it cites nothing and opened no page. False for an order seated before the rule, which keeps the old fail.
    pub no_web: bool,
    /// Plan 45 P45d (§4 M5): the order's own text names a `herdr look` call (`names_look`), so the gate notes a
    /// delivery whose stream shows no `herdr look` call that answered.
    pub asks_look: bool,
}

fn words(rest: &str) -> impl Iterator<Item = String> + '_ {
    rest.split([' ', ',', '\t'])
        .map(|word| word.trim().trim_matches('`'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
}

/// `line` without its leading `key`, the key's case ignored: MotoProject's reel-motion-gemini wrote `Owns:`, and the
/// gate read "the order names no owned file" (2026-10-31).
fn strip_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.get(..key.len())
        .filter(|head| head.eq_ignore_ascii_case(key))
        .map(|_| &line[key.len()..])
}

/// Review round (N1): with the case ignored (B6), an order's prose "Check: the page loads" read as three check names and
/// sent the order back. So a `check:` value is names only when it looks like names: every word (split on spaces,
/// commas and tabs, backticks trimmed) is letters, digits and `_ : . -` and does not end a sentence with `.`, and the
/// value is one word, or carries a backtick, or every word carries a digit or one of `_ : . -` as test names do
/// (`test_mul`, `gate::lock`, `c40b-a`). Otherwise the line is prose and names nothing. The cost: a list of plain
/// words with no mark (`check: alpha beta`) is prose now; write it in backticks.
fn check_names(rest: &str) -> Option<Vec<String>> {
    let names: Vec<String> = words(rest).collect();
    let name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-');
    let marked = |word: &str| {
        word.chars()
            .any(|c| c.is_ascii_digit() || matches!(c, '_' | ':' | '.' | '-'))
    };
    (!names.is_empty()
        && names
            .iter()
            .all(|word| word.chars().all(name_char) && !word.ends_with('.'))
        && (names.len() == 1 || rest.contains('`') || names.iter().all(|word| marked(word))))
    .then_some(names)
}

/// The gate lines of an order file (the syntax in the module comment). herdr's own footer and a send-back's brief
/// never start a line with one of these words, so a child order reads its root's lines unchanged.
pub fn spec(text: &str) -> Spec {
    let mut spec = Spec::default();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(rest) = strip_key(line, "owns:") {
            spec.owns
                .extend(words(rest).map(|path| path.replace('\\', "/")));
        // Plan 40 review B6: `test:`, `check:` and `serves:` are read as `owns:` is, the case ignored. A `Test:` line
        // read case-sensitively dropped the tests axis to a Note and the delivery passed with no test run.
        } else if let Some(rest) = strip_key(line, "test:") {
            let command = rest.trim().trim_matches('`').trim();
            if !command.is_empty() {
                spec.test = Some(command.to_owned());
            }
        } else if let Some(rest) = strip_key(line, "check:") {
            spec.checks.extend(check_names(rest).unwrap_or_default());
        } else if let Some(rest) = strip_key(line, "serves:") {
            spec.serves
                .extend(words(rest).filter_map(|port| port.parse::<u16>().ok()));
        } else if let Some(rest) = line.strip_prefix("Profile:") {
            let name = rest.trim().trim_matches('`');
            if crate::nirvana_lessons::valid_profile(name) {
                spec.profile = Some(name.to_owned());
            }
        } else if line.starts_with(NO_WEB_MARK) {
            spec.no_web = true;
        }
    }
    spec.asks_look = asks_look(text);
    spec
}

/// `*` and `?` within one path segment, `**` across any number of segments, `/` the only separator. A pattern ending
/// in `/` names a folder and matches everything under it, as `<folder>/**` does. 0.71.1: c0-skeleton-s1's `owns:
/// Config/` and `owns: Source/InkAether/` matched nothing, and the order was sent back for 309 files it owned. Plan 43:
/// the gate matches `owns:` with `owns_match`; this case-exact form is the tests'.
#[cfg(test)]
pub fn glob_match(pattern: &str, path: &str) -> bool {
    glob_match_case(pattern, path, false)
}

/// Plan 43 A2: whether `owns:` ignores case. Windows folders do: Megajam's h3-hand-seq (2026-11-14) owned
/// `source/inkaether/public/hands/iahandsequence.h` and was sent back twice for `Source/InkAether/Public/Hands/
/// IAHandSequence.h`, the same file.
pub const OWNS_FOLD_CASE: bool = cfg!(windows);

/// An `owns:` pattern names `path`: `glob_match`, with the case ignored where the file system ignores it.
pub fn owns_match(pattern: &str, path: &str) -> bool {
    glob_match_case(pattern, path, OWNS_FOLD_CASE)
}

/// `glob_match`, ASCII case ignored when `fold` is set.
pub fn glob_match_case(pattern: &str, path: &str, fold: bool) -> bool {
    fn segments(p: &[&str], s: &[&str], fold: bool) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(&"**"), _) => {
                segments(&p[1..], s, fold) || (!s.is_empty() && segments(p, &s[1..], fold))
            }
            (Some(pat), Some(seg)) => {
                segment(pat.as_bytes(), seg.as_bytes(), fold) && segments(&p[1..], &s[1..], fold)
            }
            _ => false,
        }
    }
    fn segment(p: &[u8], s: &[u8], fold: bool) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => {
                segment(&p[1..], s, fold) || (!s.is_empty() && segment(p, &s[1..], fold))
            }
            (Some(b'?'), Some(_)) => segment(&p[1..], &s[1..], fold),
            (Some(a), Some(b)) => {
                (a == b || (fold && a.eq_ignore_ascii_case(b))) && segment(&p[1..], &s[1..], fold)
            }
            _ => false,
        }
    }
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    let mut p: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    if pattern.ends_with('/') && !p.is_empty() {
        p.push("**");
    }
    let s: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments(&p, &s, fold)
}

/// herdr's own folder in a worktree: the order copy, offered lessons, `.gitignore`. Never the worker's change.
fn harness_path(path: &str) -> bool {
    path == nirvana_run::DIR || path.starts_with(&format!("{}/", nirvana_run::DIR))
}

/// Plan 24 §6 (ledger L3): build.*, fix.*, and plan orders must name the files they own; with no owned files,
/// the axis fails. Other classes (e.g. read-only) keep the Note from decision 0016 §8.8 point 4.
pub fn requires_owns(class: &str) -> bool {
    class.starts_with("build.") || class.starts_with("fix.") || class == "plan"
}

/// Axis 4. Plan 24 §6: build.*, fix.*, and plan orders must name the files they own. Every changed file counts as one
/// the base already had and no other order holds: `owned_axes` with nothing new and no holder. The tests' since plan 43.
#[cfg(test)]
pub fn owned_axis(class: &str, changed: &[String], owns: &[String]) -> Axis {
    owned_axes(class, changed, owns, &|_| false, &[]).0
}

/// Plan 43 A2: another live order of the run and the `owns:` it holds (its chain root's, with its amendments).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub order: String,
    pub owns: Vec<String>,
}

/// The name of an axis listing new files outside `owns:` that no live order holds (plan 43 A2).
pub const OWNED_NEW: &str = "owned_new";

/// Axis 4 with plan 43 A2 (owner ruling R4, 2026-11-18: "4. Yes"). A changed file outside `owns:` is one of three:
/// - held by another live order (`holders`): a fail, the worker's (a real collision; Megajam had 3 of 26);
/// - new, absent on the order's base (`is_new`), and held by no one: a note, in a second axis `owned_new`. Megajam:
///   165 of the 273 flagged files were new files the order made (evidence, helper scripts, new assets);
/// - on the base already and held by no one: a fail, the brief's (`owns:` too narrow), with the `--owns` flags that
///   would cover it.
/// `owns:` is matched with `owns_match` (the case ignored on Windows).
pub fn owned_axes(
    class: &str,
    changed: &[String],
    owns: &[String],
    is_new: &dyn Fn(&str) -> bool,
    holders: &[Holder],
) -> (Axis, Option<Axis>) {
    let changed: Vec<&String> = changed.iter().filter(|path| !harness_path(path)).collect();
    if owns.is_empty() {
        // Plan 40 T1: the paths shown are capped (`capped`); the count said is the whole list's.
        let total = changed.len();
        let all: Vec<String> = changed.iter().map(|path| (*path).clone()).collect();
        let shown = capped(all.clone());
        if requires_owns(class) {
            let mut axis = Axis::new(
                "owned",
                Mark::Fail,
                "unbounded: the order names no owned file",
            )
            .with(shown)
            .fault(Fault::Brief);
            axis.amend = owns_flags(&all);
            return (axis, None);
        }
        return (
            Axis::new(
                "owned",
                Mark::Note,
                format!("the order names no owned file; {total} file(s) changed, not checked"),
            )
            .with(shown),
            None,
        );
    }
    let outside: Vec<&String> = changed
        .iter()
        .filter(|path| !owns.iter().any(|pattern| owns_match(pattern, path)))
        .copied()
        .collect();
    let holder_of = |path: &str| {
        holders
            .iter()
            .find(|holder| holder.owns.iter().any(|pattern| owns_match(pattern, path)))
            .map(|holder| holder.order.clone())
    };
    let (mut held, mut narrow, mut fresh) = (Vec::new(), Vec::new(), Vec::new());
    let (mut fenced, mut fenced_paths) = (Vec::new(), Vec::new());
    for path in outside {
        match holder_of(path) {
            Some(holder) => held.push(format!("{path} (owned by {holder})")),
            None if is_new(path) => match fenced_new(path) {
                Some(why) => {
                    fenced_paths.push(path.clone());
                    fenced.push(format!("{path} (new, and {why})"))
                }
                None => fresh.push(path.clone()),
            },
            None => narrow.push(path.clone()),
        }
    }
    let note = (!fresh.is_empty()).then(|| {
        Axis::new(
            OWNED_NEW,
            Mark::Note,
            format!(
                "{} new file(s) outside `owns:` that no live order owns: a note, not a fail (plan 43 A2)",
                fresh.len()
            ),
        )
        .with(capped(fresh.clone()))
    });
    if held.is_empty() && narrow.is_empty() && fenced.is_empty() {
        let why = if fresh.is_empty() {
            format!("{} changed file(s), each owned", changed.len())
        } else {
            format!(
                "{} changed file(s): {} owned, {} new that no live order owns (noted in {OWNED_NEW})",
                changed.len(),
                changed.len() - fresh.len(),
                fresh.len()
            )
        };
        return (Axis::new("owned", Mark::Pass, why), note);
    }
    let mut why = format!(
        "{} file(s) changed that the order does not own",
        held.len() + fenced.len() + narrow.len()
    );
    if !held.is_empty() {
        why.push_str(&format!("; {} of them another live order owns", held.len()));
    }
    if !fenced.is_empty() {
        why.push_str(&format!(
            "; {} of them new and fenced (they run by themselves, configure tooling or look like a secret), so never a note",
            fenced.len()
        ));
        // Fix round 2 (H4, review (b)): a false positive looped through send-backs to the cap with no way out named.
        why.push_str(&format!(
            "; remove the fenced file(s), or, if the brief truly needs them, the orchestrator stops the child and runs \
`herdr gate amend <order> {}`",
            owns_flags(&fenced_paths).unwrap_or_default()
        ));
    }
    let lines = capped(held.iter().chain(&fenced).chain(&narrow).cloned().collect());
    let mut axis = Axis::new("owned", Mark::Fail, why).with(lines).fault(
        if held.is_empty() && fenced.is_empty() {
            Fault::Brief
        } else {
            Fault::Worker
        },
    );
    axis.amend = owns_flags(&narrow);
    (axis, note)
}

/// Fix round F2: why a new file outside `owns:` is never a note, or None for an ordinary one. Owner ruling R4 made a
/// new file no live order owns a note (Megajam: 165 of 273 flagged files were evidence, helper scripts, new assets),
/// and that stays for ordinary files, new source files (`*.rs`, `*.py`) among them: they are what the ruling covers.
/// The files below run by themselves, set up tooling, or look like a secret; a worker that writes one outside
/// `owns:` fails, its own fault. This list is the orchestrator's reading of a safety boundary (fix round, 2026-11-18),
/// not an owner ruling, and the owner may change it:
/// - a dot-prefixed component anywhere: `.env`, `.github/`, `.claude/`, `.agents/`, `.nirvana/`, `.gitattributes`,
///   `.git/hooks/`, `.husky/`, `.vscode/` (CI, agent settings, editor tasks and git attributes run or steer tools);
/// - `nirvana/controls.d/`: the controls runner discovers and executes every file there;
/// - `build.rs` (cargo runs it at every build) and `conftest.py` (pytest imports it from any folder it collects);
/// - config the order's own test reads where it runs: `pytest.ini`, `tox.ini`, `setup.cfg`, `pyproject.toml`,
///   `Makefile`, `GNUmakefile`, `justfile`, `rust-toolchain(.toml)` (fix round 2);
/// - a git hook: a hook's name in a folder named `hooks` or `githooks` (`githooks/pre-push`), or, as the whole file
///   name with no extension, one of the names that are only ever hooks (`pre-commit`, `pre-push`, `commit-msg`,
///   ...), or `lefthook.yml`;
/// - a credential-looking name: `*.pem`, `*.key`, `*.pfx`, `*.p12`, `id_rsa*` (and `id_dsa*`, `id_ecdsa*`,
///   `id_ed25519*`), and a file name, not a Markdown one, that holds `secret` or `credential`.
/// `.gitkeep` is not fenced. A fenced file is the worker's fault, and the axis says both ways on: remove it, or the
/// orchestrator, if the brief truly needs it, stops the child and runs `herdr gate amend <order> --owns <path>`.
pub fn fenced_new(path: &str) -> Option<&'static str> {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    let parts: Vec<&str> = lower.split('/').filter(|part| !part.is_empty()).collect();
    let name = parts.last().copied().unwrap_or("");
    // Fix round 2 (H4): `.gitkeep` only keeps an empty folder in git; nothing reads it.
    let dotted = |part: &&str| part.starts_with('.') && !(*part == ".gitkeep" && *part == name);
    if parts.iter().any(dotted) {
        return Some("dot-prefixed (CI, agent or tool settings, or git's own files)");
    }
    if lower.starts_with("nirvana/controls.d/") {
        return Some("under nirvana/controls.d/, which the controls runner executes");
    }
    if name == "build.rs" || name == "conftest.py" {
        return Some("a file the build or the test runner runs by itself");
    }
    // Fix round 2 (H4, review (b)): config the order's own `test:` reads by itself where it runs in the worktree:
    // pytest's `pytest.ini`, `tox.ini`, `setup.cfg` and `pyproject.toml` (an `addopts` there loads a plugin), make's and
    // just's recipes, and the toolchain cargo picks. Matched on the file name, the case folded.
    const AUTO_READ: &[&str] = &[
        "pytest.ini",
        "tox.ini",
        "setup.cfg",
        "pyproject.toml",
        "makefile",
        "gnumakefile",
        "justfile",
        "rust-toolchain",
        "rust-toolchain.toml",
    ];
    if AUTO_READ.contains(&name) {
        return Some("config the test runner or the build reads by itself");
    }
    // Every git hook's name (githooks(5)); these alone are never anything else.
    const HOOKS: &[&str] = &[
        "applypatch-msg",
        "pre-applypatch",
        "post-applypatch",
        "pre-commit",
        "pre-merge-commit",
        "prepare-commit-msg",
        "commit-msg",
        "post-commit",
        "pre-rebase",
        "post-checkout",
        "post-merge",
        "pre-push",
        "pre-receive",
        "update",
        "proc-receive",
        "post-receive",
        "post-update",
        "reference-transaction",
        "push-to-checkout",
        "pre-auto-gc",
        "post-rewrite",
        "sendemail-validate",
        "fsmonitor-watchman",
        "p4-changelist",
        "p4-prepare-changelist",
        "p4-post-changelist",
        "p4-pre-submit",
        "post-index-change",
    ];
    const ONLY_HOOKS: &[&str] = &[
        "pre-commit",
        "pre-push",
        "commit-msg",
        "prepare-commit-msg",
        "post-checkout",
        "post-merge",
        "post-commit",
        "pre-rebase",
        "post-rewrite",
        "pre-merge-commit",
        "pre-receive",
        "post-receive",
    ];
    // Fix round 2 (H4, review (b)): a hook's name counts as the whole file name with no extension, or as a file's stem
    // inside a folder named `hooks` (or `githooks`, a common `core.hooksPath`); a folder merely holding "hook" in its
    // name (`webhooks/`) and `Docs/pre-push.md` are no hooks.
    let stem = name.split('.').next().unwrap_or(name);
    let in_hooks = parts.len() >= 2 && matches!(parts[parts.len() - 2], "hooks" | "githooks");
    if (in_hooks && HOOKS.contains(&stem))
        || (!name.contains('.') && ONLY_HOOKS.contains(&name))
        || matches!(name, "lefthook.yml" | "lefthook.yaml")
    {
        return Some("a git hook, which git runs by itself");
    }
    let key_ext = [".pem", ".key", ".pfx", ".p12"]
        .iter()
        .any(|ext| name.ends_with(ext));
    let ssh_key = ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"]
        .iter()
        .any(|key| name.starts_with(key));
    // Fix round 2 (H4): `secret` and `credential` in the file name only, and never a Markdown file's
    // (`Docs/secretary.md`, `secrets/README.md` were fenced); p72's `secret.txt` stays fenced.
    let secretive = !name.ends_with(".md") && (name.contains("secret") || name.contains("credential"));
    if key_ext || ssh_key || secretive {
        return Some("a credential-looking name");
    }
    None
}

/// The `--owns` flags of `herdr gate amend` that cover `paths`: each file, folded a level up (`dir/` for two or more
/// patterns in one folder) while more than `OWNS_HINT_MOST` remain. None for no path. A hint: the orchestrator reads
/// it before it runs it.
pub fn owns_flags(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    // One level per round: a pattern goes to its folder when another pattern shares that folder.
    let fold = |patterns: &[String]| -> Vec<String> {
        let parent = |path: &str| {
            path.trim_end_matches('/')
                .rsplit_once('/')
                .map(|(dir, _)| format!("{dir}/"))
        };
        let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
        for pattern in patterns {
            if let Some(dir) = parent(pattern) {
                *counts.entry(dir).or_default() += 1;
            }
        }
        let mut next: Vec<String> = Vec::new();
        for pattern in patterns {
            let folded = parent(pattern)
                .filter(|dir| counts.get(dir).copied().unwrap_or(0) >= 2)
                .unwrap_or_else(|| pattern.clone());
            if !next.contains(&folded) {
                next.push(folded);
            }
        }
        next
    };
    // Each file as itself while few: a folder would own more than the order touched.
    let mut patterns: Vec<String> = paths.to_vec();
    while patterns.len() > OWNS_HINT_MOST {
        let next = fold(&patterns);
        if next == patterns {
            break;
        }
        patterns = next;
    }
    Some(
        patterns
            .iter()
            .map(|pattern| format!("--owns {}", shell_word(pattern)))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The most `--owns` patterns a hint names before it folds a level up. A hypothesis: four short paths fit the wake
/// beside the command. The wake keeps 90 characters of a gated reason (`nirvana_run::queue_line`), so two long paths
/// do not fit; it marks the cut with `…` and ends with `full line: herdr gate run <order>` (fix round F3).
pub const OWNS_HINT_MOST: usize = 4;

/// `word` as one shell word in Bash and PowerShell alike: bare when it is plain, else in double quotes.
fn shell_word(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        word.to_owned()
    } else {
        format!("\"{}\"", word.replace('"', "'"))
    }
}

/// Axis 3: O3's check, and a report that says it is done. A `blocked` worker's work is not passed.
pub fn report_axis(report: Option<&Value>) -> Axis {
    let Some(report) = report else {
        return Axis::new("report", Mark::Fail, "no Work Report was delivered");
    };
    if let Err(why) = crate::nirvana_worker::check_report(report) {
        return Axis::new("report", Mark::Fail, why);
    }
    match report.get("status").and_then(Value::as_str) {
        Some("done") => Axis::new("report", Mark::Pass, "well formed, status done"),
        Some(status) => Axis::new(
            "report",
            Mark::Fail,
            format!(
                "the worker reported {status}: {}",
                report.get("summary").and_then(Value::as_str).unwrap_or("")
            ),
        ),
        None => Axis::new("report", Mark::Fail, "the Work Report has no status"),
    }
}

/// A line of test output worth keeping: a named check, a failure, or a summary (Nirvana CLAUDE.md rule 9: the
/// doctrine measured a raw suite at 195,087 bytes of which 45 were information).
fn keep_line(line: &str, checks: &[String]) -> bool {
    if checks.iter().any(|check| line.contains(check.as_str())) {
        return true;
    }
    let lower = line.to_ascii_lowercase();
    [
        "fail",
        "error",
        "panicked",
        "traceback",
        "passed",
        "test result:",
        "tests run",
        "ran ",
    ]
    .iter()
    .any(|mark| lower.contains(mark))
        || line.trim_start().starts_with("OK")
}

pub fn filter(output: &str, checks: &[String]) -> Vec<String> {
    let clip = |line: &str| -> String { line.chars().take(300).collect() };
    let kept: Vec<&str> = output
        .lines()
        .map(str::trim_end)
        .filter(|line| keep_line(line, checks))
        .collect();
    if kept.len() <= KEPT_LINES {
        return kept.into_iter().map(clip).collect();
    }
    // Plan 40 review B4: the first line naming each check survives the cut, so the reader still sees it ran when 70
    // `... PASSED` lines follow it. The rest is cut from the front: the last lines hold the summary a reader reads
    // first. Review round (N2): the pinned line is the one that reports the check's result (a pass or fail word
    // besides the name), else the last line naming it: the first may be `Compiling <name>`, which says nothing ran.
    let reports = |line: &str, check: &str| {
        line.replace(check, " ")
            .to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| {
                matches!(
                    word,
                    "ok" | "pass" | "passed" | "fail" | "failed" | "error" | "panicked"
                )
            })
    };
    let mut pinned = vec![false; kept.len()];
    for check in checks {
        let naming = || {
            kept.iter()
                .enumerate()
                .filter(|(_, line)| line.contains(check.as_str()))
        };
        let result = naming()
            .find(|(_, line)| reports(line, check))
            .or_else(|| naming().next_back());
        if let Some((at, _)) = result {
            pinned[at] = true;
        }
    }
    let room = KEPT_LINES.saturating_sub(pinned.iter().filter(|pin| **pin).count());
    let others: Vec<usize> = (0..kept.len()).filter(|at| !pinned[*at]).collect();
    let tail = &others[others.len().saturating_sub(room)..];
    (0..kept.len())
        .filter(|at| pinned[*at] || tail.contains(at))
        .map(|at| clip(kept[at]))
        .collect()
}

/// Axis 2, from a finished run of the order's test command.
pub fn tests_axis(test: Option<&str>, run: Option<&CommandRun>, checks: &[String]) -> Axis {
    let (Some(test), Some(run)) = (test, run) else {
        if checks.is_empty() {
            return Axis::new("tests", Mark::Note, "the order names no test command");
        }
        // Plan 43 A1: the brief's to fix, as a `test:` it never wrote.
        let mut axis = Axis::new(
            "tests",
            Mark::Fail,
            "the order names checks but no `test:` command to run them",
        )
        .fault(Fault::Brief);
        axis.amend = Some("--test \"<a command that prints the checks>\"".into());
        return axis;
    };
    let lines = filter(&run.output, checks);
    if let Some(why) = &run.error {
        let mut axis = Axis::new("tests", Mark::Fail, format!("`{test}` did not run: {why}"))
            .with(lines)
            .fault(Fault::Brief);
        axis.amend = Some("--test \"<the fixed command>\"".into());
        return axis;
    }
    // Plan 43 A1: Megajam's r1-tripo-v3, a3-megascans-fit and v12-valise-wording were sent back for their own `test:`,
    // which did not parse; a worker cannot fix the orchestrator's command.
    if run.exit != Some(0) && !run.timed_out {
        if let Some(why) = test_unparsed(test, &run.output) {
            let mut axis = Axis::new(
                "tests",
                Mark::Fail,
                format!("`{test}` did not parse: {why}"),
            )
            .with(lines)
            .fault(Fault::Brief);
            axis.amend = Some("--test \"<the fixed command>\"".into());
            return axis;
        }
    }
    if run.timed_out {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!("`{test}` ran past {TEST_TIMEOUT_S} s and was stopped"),
        )
        .with(lines);
    }
    // Plan 40 review B4: the named checks are looked for in the WHOLE output. Read from the filtered lines, which are
    // cut to the last `KEPT_LINES`, a check that passed early was cut by 70 later `PASSED` lines and a correct order
    // was sent back. A check absent from the whole output still fails (Nirvana CLAUDE.md rule 9).
    let absent: Vec<&String> = checks
        .iter()
        .filter(|check| !run.output.lines().any(|line| line.contains(check.as_str())))
        .collect();
    if run.exit != Some(0) {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!(
                "`{test}` exited {}",
                run.exit
                    .map_or("without a code".to_owned(), |c| c.to_string())
            ),
        )
        .with(lines);
    }
    if !absent.is_empty() {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!(
                "the named check(s) {} never ran: a suite total cannot see a check that never ran",
                absent
                    .iter()
                    .map(|check| check.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with(lines);
    }
    Axis::new(
        "tests",
        Mark::Pass,
        if checks.is_empty() {
            format!("`{test}` exited 0 (the order names no check)")
        } else {
            format!("`{test}` exited 0 and printed {}", checks.join(", "))
        },
    )
    .with(lines)
}

/// `text` without its ANSI escapes (pwsh colours `ParserError:`).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// Plan 43 A1: why a `test:` command's own text did not parse or start, read from its output; None when the command
/// ran, its test passing or not. Only the command's own text counts, never a file it runs: python's `File
/// "<string>"` is the `-c` code; a pwsh `ParserError` that names no `.ps1` is the `-Command` text; bash's `-c:` line
/// is the `-c` text; cmd's "is not recognized" names the program itself. Megajam's r1-tripo-v3 printed `SyntaxError:
/// unterminated string literal` under `File "<string>"`; a3-megascans-fit and v12-valise-wording a bare
/// `ParserError:`. A worker's own broken `.py` or `.ps1` names its file, and stays the worker's.
///
/// Fix round F1: `python -c "import brokenmod"` on a module holding `x = (` prints the `-c` frame `File "<string>",
/// line 1, in <module>`, then `File "...brokenmod.py", line 1`, then `SyntaxError`: the worker's module, not the
/// brief's text. So the error is the brief's only when the frame right above it is `File "<string>"` and that frame
/// opens no traceback: a compile error of the `-c` text itself prints that one frame and no `Traceback (most recent
/// call last):` line (`exec()` of a string read from the worker's file prints the traceback first, and stays the
/// worker's). bash: only a line that opens with the shell's own name and `-c:` (`bash: -c: line 1: syntax error`); a
/// script the `-c` text sources names its file (`./bad.sh: line 1: ...`). A worker's script that itself runs `bash
/// -c` or `pwsh -Command` on broken text still reads as the brief's: an open hole, rare, said in the fix round's
/// report.
///
/// Fix round 2 (H2): that hole is closed by reading the command itself. Its own text is only ever the inline code of
/// an interpreter run directly: `python -c`, `bash -c` / `sh -c`, `pwsh -Command` (and Windows PowerShell's
/// positional command). A test whose program is a script, a test runner or anything else (`bash run.sh`, `pytest`,
/// `cargo test`, `pwsh -File x.ps1`) has no text of the brief's to misparse, so a parse error anywhere in its output
/// is the worker's (`inline_code`). And a pwsh `ParserError:` that names no file is still the worker's when a later
/// line shows it came from a loaded file: `pwsh -Command "Import-Module ./bad.psm1; 'x'"` on a module holding
/// `function f { (` printed (pwsh 7, 2026-11-18) `ParserError: An expression was expected after '('.` and then
/// `Import-Module: The specified module './bad.psm1' was not loaded ...`; Windows PowerShell 5 printed the module's
/// `At C:\...\bad.psm1:1 char:15`, wrapped across two lines, before the `Import-Module :` error's `At line:1`.
/// Output a pipe of which was never read (`UNREAD_OUTPUT`) is not the whole story, so it is never the brief's.
pub fn test_unparsed(test: &str, output: &str) -> Option<String> {
    // Fix round 2 (H2): `run_command` reads both pipes whole, so the front is never cut; a pipe still held open after
    // the test ended is dropped whole and marked with this line. A dropped stderr loses a `Traceback` header too.
    if output.contains(UNREAD_OUTPUT) {
        return None;
    }
    let plain = strip_ansi(output);
    let lines: Vec<&str> = plain.lines().map(str::trim).collect();
    let argv = split_command(test);
    let program = argv.first().cloned().unwrap_or_default();
    let base = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let base = base.strip_suffix(".exe").unwrap_or(&base);
    let inline = inline_code(base, argv.get(1..).unwrap_or(&[]));
    if inline && is_python(base) && plain.contains("File \"<string>\"") {
        for (at, line) in lines.iter().enumerate() {
            if !["SyntaxError", "IndentationError", "TabError"]
                .iter()
                .any(|kind| line.starts_with(kind))
            {
                continue;
            }
            // The frame the error is in: the nearest `File "` line above it.
            let Some(frame) = lines[..at]
                .iter()
                .rposition(|line| line.starts_with("File \""))
            else {
                continue;
            };
            if !lines[frame].starts_with("File \"<string>\"") {
                continue;
            }
            // A traceback over the frame: the `-c` text ran, and imported or exec'd the broken code. Its frames and
            // source lines are indented; the first line above them that is not is its header, or other output.
            let raw: Vec<&str> = plain.lines().collect();
            let traced = raw[..frame]
                .iter()
                .rev()
                .find(|line| !line.starts_with([' ', '\t']))
                .is_some_and(|line| line.trim().starts_with("Traceback (most recent call last)"));
            if !traced {
                return Some((*line).to_owned());
            }
        }
    }
    if inline && matches!(base, "pwsh" | "powershell") {
        let script = |text: &str| {
            let lower = text.to_ascii_lowercase();
            lower.contains(".ps1") || lower.contains(".psm1")
        };
        // pwsh 7 echoes the failing source under the error (`Line |`, `   1 | <text>`, `     | ~~~`): the brief's own
        // text, which may name a `.ps1` itself.
        let echo = |line: &str| {
            line.split_once('|').is_some_and(|(head, _)| {
                let head = head.trim();
                head.is_empty() || head == "Line" || head.chars().all(|c| c.is_ascii_digit())
            })
        };
        // Fix round 2 (H2): a later line that shows a file was loaded: the module import's own error, a script's
        // path, or Windows PowerShell's `At <path>:<line>` (its `At line:` is the command's own text).
        let loaded = |line: &str| {
            line.starts_with("Import-Module:")
                || line.starts_with("Import-Module :")
                || (line.starts_with("At ") && !line.starts_with("At line:"))
                || (!echo(line) && script(line))
        };
        for (at, line) in lines.iter().enumerate() {
            if let Some(rest) = line.strip_prefix("ParserError:") {
                if script(rest) {
                    continue;
                }
                if lines[at + 1..].iter().any(|line| loaded(line)) {
                    continue;
                }
                // pwsh 7 prints the message last, on a `|` line of its own.
                let message = lines[at + 1..]
                    .iter()
                    .take(8)
                    .filter_map(|line| line.strip_prefix('|'))
                    .map(str::trim)
                    .rfind(|line| !line.is_empty() && !line.chars().all(|c| c == '~' || c == ' '));
                return Some(match message {
                    Some(message) => format!("ParserError: {message}"),
                    None => "ParserError".to_owned(),
                });
            }
        }
        // Windows PowerShell 5: `At line:1 char:5` for the command's text, `At C:\x.ps1:3 char:5` for a script's.
        if plain.contains("At line:")
            && lines
                .iter()
                .any(|line| line.contains("CategoryInfo") && line.contains("ParserError"))
            && !lines.iter().any(|line| loaded(line))
        {
            return Some("ParserError".to_owned());
        }
    }
    if inline && matches!(base, "bash" | "sh") {
        // Fix round F1: the shell's own `-c:` line opens with its name as run (`bash: -c:`, `/usr/bin/bash: -c:`); a
        // line that only holds `-c:` somewhere is another program's.
        let own = |line: &str| {
            line.split_once(": -c:").is_some_and(|(head, _)| {
                let name = head
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                name.strip_suffix(".exe").unwrap_or(&name) == base
            })
        };
        if let Some(line) = lines.iter().find(|line| {
            own(line)
                && (line.contains("syntax error")
                    || line.contains("unexpected EOF while looking for matching"))
        }) {
            return Some((*line).to_owned());
        }
    }
    if !program.is_empty() {
        let needle = format!("'{program}' is not recognized as an internal or external command");
        if plain.contains(&needle) {
            return Some(needle);
        }
    }
    None
}

/// The line `run_command` adds when a pipe was still open after the test ended and its text was dropped.
pub const UNREAD_OUTPUT: &str =
    "herdr gate: the output pipes were still open after the test ended; the rest was not read";

/// A python launcher by its base name: `python`, `python3`, `python3.13`, `pythonw`, `py`.
fn is_python(base: &str) -> bool {
    base == "py" || base.starts_with("python")
}

/// Fix round 2 (H2): the test command is an interpreter run directly on inline code, the only text of the brief's
/// that can fail to parse: `python [-I ...] -c`, `bash|sh [-e ...] -c` (a short-flag cluster holding `c`, as `-lc`,
/// before the first operand), `pwsh -Command` (`-c`, `-com...`, `-EncodedCommand`) before any `-File`, and Windows
/// PowerShell with no `-File` (its first operand is a command). `args` are the words after the program.
fn inline_code(base: &str, args: &[String]) -> bool {
    let lower: Vec<String> = args.iter().map(|arg| arg.to_ascii_lowercase()).collect();
    let short_c = |arg: &str| {
        arg.len() >= 2
            && arg.starts_with('-')
            && !arg.starts_with("--")
            && arg[1..].chars().all(|c| c.is_ascii_alphabetic())
            && arg[1..].contains('c')
    };
    if is_python(base) {
        // Python's options before `-c`; `-m` or a script operand ends them.
        for arg in &lower {
            if arg == "-c" {
                return true;
            }
            if arg == "-m" || !arg.starts_with('-') {
                return false;
            }
        }
        return false;
    }
    if matches!(base, "bash" | "sh") {
        return lower
            .iter()
            .take_while(|arg| arg.starts_with('-'))
            .any(|arg| short_c(arg));
    }
    if matches!(base, "pwsh" | "powershell") {
        let command = |arg: &str| {
            matches!(arg, "-c" | "-e" | "-ec" | "-encodedcommand")
                || (arg.len() >= 4 && "-command".starts_with(arg))
        };
        let file = |arg: &str| arg.len() >= 2 && "-file".starts_with(arg);
        let first_command = lower.iter().position(|arg| command(arg));
        let first_file = lower.iter().position(|arg| file(arg));
        return match (first_command, first_file) {
            (Some(c), Some(f)) => c < f,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => base == "powershell",
        };
    }
    false
}

/// Axis 5. `listened` is every port listened on inside the worker's pane's process tree.
pub fn served_axis(class: &str, wanted: &[u16], listened: Result<Vec<u16>, String>) -> Axis {
    if class != "ops.terminal" {
        return Axis::new("served", Mark::Note, "not an ops order");
    }
    if wanted.is_empty() {
        return Axis::new("served", Mark::Fail, "an ops order names no `serves:` port");
    }
    let listened = match listened {
        Ok(ports) => ports,
        Err(why) => {
            return Axis::new(
                "served",
                Mark::Fail,
                format!("the worker's pane could not be read: {why}"),
            )
        }
    };
    let missing: Vec<String> = wanted
        .iter()
        .filter(|port| !listened.contains(port))
        .map(u16::to_string)
        .collect();
    if missing.is_empty() {
        Axis::new(
            "served",
            Mark::Pass,
            format!("the worker's pane listens on {}", join_ports(wanted)),
        )
    } else {
        Axis::new(
            "served",
            Mark::Fail,
            format!(
                "no process in the worker's own pane listens on {} (it listens on {})",
                missing.join(", "),
                if listened.is_empty() {
                    "nothing".to_owned()
                } else {
                    join_ports(&listened)
                }
            ),
        )
    }
}

fn join_ports(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Trims fragment and trailing slash from a URL, lowercases scheme and host, drops www., and treats http and https as equal.
pub fn normalize_url(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url).trim();
    if without_fragment.is_empty() {
        return String::new();
    }
    let (scheme, rest) = if let Some(idx) = without_fragment.find("://") {
        let rest = &without_fragment[idx + 3..];
        ("http://", rest)
    } else {
        ("", without_fragment)
    };
    let (host, path_and_query) = match rest.find(['/', '?']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let mut host = host.to_ascii_lowercase();
    if host.starts_with("www.") {
        host = host[4..].to_string();
    }
    let (path, query) = match path_and_query.find('?') {
        Some(q_idx) => (&path_and_query[..q_idx], &path_and_query[q_idx..]),
        None => (path_and_query, ""),
    };
    let trimmed_path = path.trim_end_matches('/');
    format!("{scheme}{host}{trimmed_path}{query}")
}

fn collect_json_strings(val: &Value, out: &mut Vec<String>) {
    match val {
        Value::String(s) => out.push(s.clone()),
        Value::Array(arr) => {
            for v in arr {
                collect_json_strings(v, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_json_strings(v, out);
            }
        }
        _ => {}
    }
}

pub fn extract_url_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let slice = &text[cursor..];
        let next_http = slice.find("http://");
        let next_https = slice.find("https://");
        let next_pos = match (next_http, next_https) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some(pos) = next_pos else {
            break;
        };
        let start = cursor + pos;
        let url_part = &text[start..];
        let len = url_part
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>'))
            .unwrap_or(url_part.len());
        let token = trim_url_tail(&url_part[..len]);
        tokens.push(token.to_string());
        cursor = start + len;
    }
    tokens
}

/// A URL in a shell line or in prose often ends at `;`, `,`, `.` or a closing `)`; a `)` stays when the URL opened
/// one, as in Wikipedia's `Foo_(bar)`.
fn trim_url_tail(token: &str) -> &str {
    let mut t = token;
    while let Some(last) = t.chars().last() {
        let opened = last == ')' && t.matches('(').count() >= t.matches(')').count();
        if matches!(last, ';' | ',' | '.' | ')' | '|' | '&') && !opened {
            t = &t[..t.len() - 1];
        } else {
            break;
        }
    }
    t
}

/// The URLs a shell command fetches. A command is cut at `;`, `|`, `&&` and newlines outside quotes (`segments`), and
/// a segment's URLs count only when the segment itself runs a fetch tool, looked for outside its URLs: `curl a; echo
/// <url>` and `echo https://curl.se/` fetch nothing. A segment that runs `gh api` or `gh repo view` also reads the
/// repository it names (`gh_read_urls`).
pub fn shell_fetch_urls(cmd: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for segment in segments(cmd) {
        let tokens = extract_url_tokens(segment);
        let mut rest = segment.to_owned();
        for token in &tokens {
            rest = rest.replace(token.as_str(), " ");
        }
        if command_is_shell_fetch(&rest) {
            urls.extend(tokens);
        }
        urls.extend(gh_read_urls(segment));
    }
    urls
}

pub const SHELL_FETCH_WORDS: &[&str] = &[
    "agent-browser",
    "bladebro",
    "curl",
    "wget",
    "Invoke-WebRequest",
    "iwr",
];

/// Plan 35 P35a: agy reads pages from Python and PowerShell (370 `python urllib` commands in 78 agy streams, plan 35
/// §1.2), and none of these was a fetch word, so the sources check sent back reads it could not see. Still per
/// segment: in the 8 agy send-backs the scripts named their URL on a line of its own, and those stay uncounted.
pub const SCRIPT_FETCH_WORDS: &[&str] = &[
    "urlopen(",
    "urllib.request",
    "requests.get",
    "requests.post",
    "requests.head",
    "requests.Session",
    "httpx.get",
    "httpx.Client",
    "Invoke-RestMethod",
];

/// MotoProject's reel-motion-gemini-r2 read YouTube through `yt_dlp` (16 of its 56 commands), in a script or as
/// `python -m yt_dlp`; YouTube watches, Shorts and searches all go through it.
pub const VIDEO_FETCH_WORDS: &[&str] = &["yt_dlp", "yt-dlp", "YoutubeDL"];

/// Fetch words that count only as a whole word: `irm` is inside "confirm". `iwr` stays a substring in
/// `SHELL_FETCH_WORDS`, as before plan 35.
pub const FETCH_TOKENS: &[&str] = &["irm"];

pub fn command_is_shell_fetch(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    SHELL_FETCH_WORDS
        .iter()
        .chain(SCRIPT_FETCH_WORDS)
        .chain(VIDEO_FETCH_WORDS)
        .any(|word| lower.contains(&word.to_ascii_lowercase()))
        || FETCH_TOKENS.iter().any(|word| has_token(&lower, word))
}

/// `word` stands alone in `text`: no letter, digit, `_`, `-` or `.` touches it, and no `$` before it (a variable).
fn has_token(text: &str, word: &str) -> bool {
    let joins = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(|c| joins(c) || c == '$') && !after.is_some_and(joins)
    })
}

/// Plan 35 P35a review round (S2): one segment's code, as long as the segment so offsets agree. Its URLs, its quoted
/// strings and a `#` comment are blanked: `echo "curl"`, `# requests.get` and `x = "yt-dlp notes"` run nothing. The
/// quote that opens a shell's `-c`/`-Command`/`/c`/`-e` argument holds the code itself, so it reads as `(`.
fn segment_code(segment: &str) -> String {
    let mut text = segment.to_owned();
    for token in extract_url_tokens(segment) {
        text = text.replace(token.as_str(), &" ".repeat(token.len()));
    }
    let mut bytes = text.into_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte == b'#' && (at == 0 || bytes[at - 1].is_ascii_whitespace()) {
            bytes[at..].fill(b' ');
            break;
        }
        if byte != b'"' && byte != b'\'' {
            at += 1;
            continue;
        }
        if opens_code(&String::from_utf8_lossy(&bytes[..at])) {
            bytes[at] = b'(';
            at += 1;
            continue;
        }
        // Plan 47 P47d: where the quote closes is `quote_end`'s, as `segments` reads it.
        match quote_end(&bytes, at).map(|end| end - at - 1) {
            Some(len) => {
                bytes[at..at + len + 2].fill(b' ');
                at += len + 2;
            }
            None => {
                bytes[at] = b' ';
                at += 1;
            }
        }
    }
    // Only whole characters between ASCII quotes, or from a `#` on, were blanked.
    String::from_utf8(bytes).unwrap_or_default()
}

/// The quote that follows `head` opens a shell's code: `head` ends in whitespace after `-c`, `-Command`, `/c` or `-e`.
/// That quote holds commands, not a string, so `segment_code` reads it as `(` and `segments` cuts inside it.
fn opens_code(head: &str) -> bool {
    let head = head.to_ascii_lowercase();
    let flag = head.split_whitespace().next_back().unwrap_or("");
    head.ends_with(char::is_whitespace) && matches!(flag, "-c" | "-command" | "/c" | "-e")
}

/// Plan 47 P47d: where the quote at `at` (`'` or `"`) closes, on its own line, or `None` when it does not. In `"..."`
/// a backslash or a PowerShell backtick escapes the next character; `'...'` has no escape (PowerShell's `''` reads as
/// two strings side by side, which hold the same characters). A quote that does not close on its line is no quote:
/// an apostrophe in `echo it's` must not join the rest of a long command into one segment.
fn quote_end(bytes: &[u8], at: usize) -> Option<usize> {
    let quote = bytes[at];
    let mut end = at + 1;
    while end < bytes.len() {
        match bytes[end] {
            b'\n' => return None,
            b'\\' | b'`' if quote == b'"' => {
                if bytes.get(end + 1) == Some(&b'\n') {
                    return None;
                }
                end += 2;
            }
            byte if byte == quote => return Some(end),
            _ => end += 1,
        }
    }
    None
}

/// The word at `at` of a segment's code is a command it runs: it comes first, or after `(`, `{`, `[`, `&`, `!`, `=`,
/// `@`, `$(` or a backtick, or after `try`, `do`, `then`, `else`, `sudo`, `time`, `exec`, a shell's `-c`/`-Command`/
/// `/c`/`-e`, or python's `-m`. A `$` right before it makes it a variable (`$irm`).
fn command_at(code: &str, at: usize) -> bool {
    let before = code[..at].trim_end();
    if code[..at].ends_with('$') {
        return false;
    }
    let rest = before.trim_end_matches(|c: char| c.is_whitespace() || "({[&!=@$`".contains(c));
    if rest.is_empty() || rest.len() < before.len() {
        return true;
    }
    let prev = rest.rsplit(char::is_whitespace).next().unwrap_or("");
    [
        "try", "do", "then", "else", "sudo", "time", "exec", "-m", "-c", "-command", "/c", "-e",
    ]
    .contains(&prev.to_ascii_lowercase().as_str())
}

/// Plan 35 P35a review round (S2): the commands that fetch, each counted only as a command word (`command_at`), so a
/// file named `curl.txt`, a variable `curl_notes` or an argument `echo curl` fetches nothing. `yt_dlp` is python's
/// `-m yt_dlp`.
pub const FETCH_COMMANDS: &[&str] = &[
    "curl",
    "wget",
    "iwr",
    "irm",
    "invoke-webrequest",
    "invoke-restmethod",
    "yt-dlp",
    "yt_dlp",
    "agent-browser",
    "bladebro",
];

/// The script calls that fetch, counted in a segment's code (outside its strings and comment) when no letter, digit
/// or `_` comes before them: `urlopen(` but not `myurlopen(`.
pub const FETCH_CALLS: &[&str] = &[
    "urlopen(",
    "urllib.request.",
    "requests.get(",
    "requests.post(",
    "requests.head(",
    "requests.session(",
    "httpx.get(",
    "httpx.client(",
    "yt_dlp.",
    "youtubedl(",
];

/// A segment led by one of these prints or reads its arguments: `echo urlopen(x)` calls nothing.
const ARG_COMMANDS: &[&str] = &[
    "echo",
    "printf",
    "write-output",
    "write-host",
    "cat",
    "type",
    "get-content",
    "gc",
];

/// Each place `word` stands in `code` as a command word, `.exe` allowed after it.
fn command_words<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word).filter_map(move |(at, _)| {
        let after = &code[at + word.len()..];
        let after = after.strip_prefix(".exe").unwrap_or(after);
        let joined = after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        (!joined && command_at(code, at)).then_some(at)
    })
}

/// A segment runs a fetch: a fetch command is one of its command words, or a fetch call stands in its code, or a
/// python `subprocess` or `os.system` call runs a fetch command (reel-motion-gemini-r2 read Reddit through
/// `subprocess.check_output(['curl.exe', ...])`).
fn segment_runs_fetch(segment: &str) -> bool {
    let code = segment_code(segment).to_ascii_lowercase();
    if FETCH_COMMANDS
        .iter()
        .any(|word| command_words(&code, word).next().is_some())
    {
        return true;
    }
    let head = code
        .trim_start_matches(|c: char| c.is_whitespace() || "({[&!@$`".contains(c))
        .split_whitespace()
        .next()
        .unwrap_or("");
    let calls = |code: &str, call: &str| {
        code.match_indices(call).any(|(at, _)| {
            !code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        })
    };
    !ARG_COMMANDS.contains(&head)
        && (FETCH_CALLS.iter().any(|call| calls(&code, call)) || runs_fetch_child(segment, &code))
}

/// A `subprocess.<fn>(` or `os.system(`/`os.popen(` call in the code whose first argument, or the first item of its
/// argument list, is a string led by a fetch command: `['curl.exe', '-s', u]`, `"yt-dlp -j " + u`.
fn runs_fetch_child(segment: &str, code: &str) -> bool {
    let raw = segment.to_ascii_lowercase();
    ["subprocess.", "os.system(", "os.popen("]
        .iter()
        .any(|call| {
            code.match_indices(call).any(|(at, _)| {
                let rest = &raw[at + call.len()..];
                let rest = if *call == "subprocess." {
                    let Some(open) = rest.find('(') else {
                        return false;
                    };
                    if !rest[..open]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    {
                        return false;
                    }
                    &rest[open + 1..]
                } else {
                    rest
                };
                let rest = rest
                    .trim_start()
                    .strip_prefix(['[', '('])
                    .unwrap_or(rest.trim_start());
                let rest = rest.trim_start();
                let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                    return false;
                };
                let word = rest[1..]
                    .split([quote, ' '])
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(".exe");
                FETCH_COMMANDS.contains(&word)
            })
        })
}

/// Plan 35 P35a: `gh api repos/<o>/<n>[/...]` and `gh repo view <o>/<n>` read a repository with no URL in the command
/// (the ref-dance-apps send-back: 8 of 8 cited read so). Each reads `https://github.com/<o>/<n>` and
/// `https://api.github.com/repos/<o>/<n>`; `gh api` also reads its own endpoint (asl-report cited `.../license`).
/// Review round (S1): only a `gh` that is the segment's command word, so `echo gh api repos/o/n`, a `# gh repo view`
/// comment or the phrase in a string reads nothing.
fn gh_read_urls(segment: &str) -> Vec<String> {
    let code = segment_code(segment).to_ascii_lowercase();
    let repo = |o: &str, n: &str| {
        vec![
            format!("https://github.com/{o}/{n}"),
            format!("https://api.github.com/repos/{o}/{n}"),
        ]
    };
    let mut urls = Vec::new();
    for at in command_words(&code, "gh") {
        let words: Vec<&str> = segment[at..]
            .split_whitespace()
            .map(|w| w.trim_matches(|c| matches!(c, '"' | '\'' | '(' | ')')))
            .collect();
        let Some(&sub) = words.get(1) else {
            continue;
        };
        match (sub, words.get(2).copied()) {
            ("api", _) => {
                let Some(endpoint) = gh_api_endpoint(&words[2..]) else {
                    continue;
                };
                let Some(path) = endpoint.trim_start_matches('/').strip_prefix("repos/") else {
                    continue;
                };
                let Some((owner, rest)) = path.split_once('/') else {
                    continue;
                };
                let name_end = rest.find(['/', '?']).unwrap_or(rest.len());
                let (name, tail) = rest.split_at(name_end);
                if gh_name(owner) && gh_name(name) {
                    urls.extend(repo(owner, name));
                    if !tail.is_empty() {
                        urls.push(format!("https://api.github.com/repos/{owner}/{name}{tail}"));
                    }
                }
            }
            ("repo", Some("view")) => {
                let Some(arg) = words.get(3) else {
                    continue;
                };
                let arg = arg
                    .strip_prefix("https://")
                    .or_else(|| arg.strip_prefix("http://"))
                    .map_or(*arg, |rest| {
                        let rest = rest.strip_prefix("www.").unwrap_or(rest);
                        rest.strip_prefix("github.com/").unwrap_or("")
                    });
                let arg = arg.trim_end_matches('/');
                let arg = arg.strip_suffix(".git").unwrap_or(arg);
                if let Some((owner, name)) = arg.split_once('/') {
                    if gh_name(owner) && gh_name(name) {
                        urls.extend(repo(owner, name));
                    }
                }
            }
            _ => {}
        }
    }
    urls
}

/// The endpoint of `gh api`: its first word that is neither a flag nor the value of `-X`/`--method`/`-H`/`--header`/
/// `--hostname`.
fn gh_api_endpoint<'a>(args: &[&'a str]) -> Option<&'a str> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if matches!(*arg, "-X" | "--method" | "-H" | "--header" | "--hostname") {
            args.next();
        } else if !arg.starts_with('-') {
            return Some(arg);
        }
    }
    None
}

/// A GitHub owner or repository name: no `{owner}` placeholder, no shell or path punctuation.
fn gh_name(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// An agy tool step that reads a page: its URL readers and browser tools, or an MCP call to a browser server.
fn agy_fetch_tool(tool_name: &str, params: &Value) -> bool {
    let is_browser_mcp = tool_name == "call_mcp_tool" && {
        let mut p_strings = Vec::new();
        collect_json_strings(params, &mut p_strings);
        p_strings.iter().any(|s| {
            let s_lower = s.to_ascii_lowercase();
            s_lower.contains("browser")
                || s_lower.contains("bladebro")
                || s_lower.contains("chrome")
                || s_lower.contains("playwright")
        })
    };
    tool_name == "read_url_content"
        || tool_name == "open_browser_url"
        || tool_name == "read_browser_page"
        || tool_name.starts_with("browser_")
        || is_browser_mcp
}

/// Checks if a normalized URL was fetched, read or browsed in the worker stream.
fn url_in_stream(norm_url: &str, stream: &str) -> bool {
    if norm_url.is_empty() {
        return false;
    }
    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        // Claude: message.content[] items with type == "tool_use"
        if let Some(content) = val.pointer("/message/content").and_then(Value::as_array) {
            for block in content {
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                    let is_fetch = name == "WebFetch"
                        || (name.starts_with("mcp__")
                            && (name.contains("browser")
                                || name.contains("bladebro")
                                || name.contains("chrome")
                                || name.contains("playwright")));
                    if is_fetch {
                        if let Some(input) = block.get("input") {
                            let mut strings = Vec::new();
                            // Plan 47 P47d (measurement §5 item 3): WebFetch fetched its `url`; its `prompt` is the
                            // worker's own text, as `stream_reads` already read it. A browser call keeps its input.
                            if name == "WebFetch" {
                                strings.extend(input.get("url").and_then(Value::as_str).map(str::to_owned));
                            } else {
                                collect_json_strings(input, &mut strings);
                            }
                            for s in strings {
                                for tok in extract_url_tokens(&s) {
                                    if normalize_url(&tok) == norm_url {
                                        return true;
                                    }
                                }
                            }
                        }
                    } else if matches!(name, "Bash" | "PowerShell") {
                        // The command only: a `description` naming curl and a URL fetches nothing. Plan 47 P47d
                        // (measurement §5 item 4): Claude's PowerShell tool runs a command as Bash does, and the
                        // stable-id rule (`stream_reads`) already read both.
                        let command = block
                            .pointer("/input/command")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if shell_fetch_urls(command)
                            .iter()
                            .any(|tok| normalize_url(tok) == norm_url)
                        {
                            return true;
                        }
                    }
                }
            }
        }

        // agy: step_update tool steps
        let step = val.get("step_update").unwrap_or(&val);
        if step.get("step_type").and_then(Value::as_str) == Some("tool") {
            let tool_name = step.get("tool_name").and_then(Value::as_str).unwrap_or("");
            let params = &step["tool_info"]["parameters"];
            if agy_fetch_tool(tool_name, params) {
                let mut strings = Vec::new();
                collect_json_strings(params, &mut strings);
                for s in strings {
                    for tok in extract_url_tokens(&s) {
                        if normalize_url(&tok) == norm_url {
                            return true;
                        }
                    }
                }
            } else if tool_name == "run_command" {
                // agy's command is `CommandLine` (the recorded fixtures); its other parameters are not the command.
                let command = params
                    .get("CommandLine")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if shell_fetch_urls(command)
                    .iter()
                    .any(|tok| normalize_url(tok) == norm_url)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// A cited source's stable id: what stays the same between the URL a worker cites and the one it fetched
/// (MotoProject's reel-motion-gemini-r2 cited `reddit.com/r/x/comments/<id>/<slug>/` and fetched `.../<slug>/.rss`, and
/// read YouTube pages through `yt_dlp` with the ids in a Python list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StableId {
    /// A YouTube video id, 11 characters.
    Video(String),
    /// A Reddit post id, lowercased.
    Post(String),
    /// A GitHub repository as `owner/name`, lowercased; only a repository's own page or API URL.
    Repo(String),
    /// A local file, lowercased, `/` separators.
    Path(String),
}

/// The stable id of a cited source, or `None` for any other URL (which keeps the per-segment rule alone).
pub fn stable_id(url: &str) -> Option<StableId> {
    let url = url.trim();
    if let Some(path) = url.strip_prefix("file://") {
        // `file:///C:/x` names `C:/x`, `file:///home/x` names `/home/x`.
        let path = percent_decoded(path);
        let b = path.as_bytes();
        let drive = b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':';
        return Some(StableId::Path(local_path(if drive {
            &path[1..]
        } else {
            &path
        })));
    }
    let bytes = url.as_bytes();
    if bytes.len() > 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        return Some(StableId::Path(local_path(url)));
    }
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let rest = rest.split('#').next().unwrap_or(rest);
    let (host, path_query) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = host.to_ascii_lowercase();
    let host = ["www.", "m.", "old.", "music.", "embed."]
        .iter()
        .find_map(|sub| host.strip_prefix(sub))
        .unwrap_or(&host);
    let (path, query) = path_query.split_once('?').unwrap_or((path_query, ""));
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match host {
        "youtube.com" => match parts.as_slice() {
            ["watch"] => query
                .split('&')
                .find_map(|pair| pair.strip_prefix("v="))
                .and_then(video_id),
            ["shorts" | "embed" | "live", id, ..] => video_id(id),
            _ => None,
        },
        "youtu.be" => parts.first().and_then(|id| video_id(id)),
        "redd.it" => parts.first().and_then(|id| post_id(id)),
        "reddit.com" => parts
            .iter()
            .position(|p| *p == "comments")
            .and_then(|at| parts.get(at + 1))
            .and_then(|id| post_id(id)),
        "github.com" => match parts.as_slice() {
            [owner, name] => repo_id(owner, name),
            _ => None,
        },
        "api.github.com" => match parts.as_slice() {
            ["repos", owner, name] => repo_id(owner, name),
            _ => None,
        },
        _ => None,
    }
}

/// `%XX` escapes decoded, as a `file://` URL writes a space or a non-ASCII name.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn id_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
}

fn video_id(id: &str) -> Option<StableId> {
    (id.len() == 11 && id.bytes().all(id_byte)).then(|| StableId::Video(id.to_owned()))
}

fn post_id(id: &str) -> Option<StableId> {
    (id.len() >= 4 && id.bytes().all(|b| b.is_ascii_alphanumeric()))
        .then(|| StableId::Post(id.to_ascii_lowercase()))
}

fn repo_id(owner: &str, name: &str) -> Option<StableId> {
    let name = name.strip_suffix(".git").unwrap_or(name);
    (gh_name(owner) && gh_name(name))
        .then(|| StableId::Repo(format!("{owner}/{name}").to_ascii_lowercase()))
}

fn local_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

/// `id` stands in `text` as itself: for a video id no id character touches it; a post id follows `comments/` in a
/// reddit.com address, or `redd.it/` (review nit: a GitHub `.../comments/<id>` is not a Reddit post); a repository is
/// not part of a longer name.
fn id_in(text: &str, id: &StableId) -> bool {
    let free = |text: &str, at: usize, len: usize, joins: &dyn Fn(char) -> bool| {
        !text[..at].chars().next_back().is_some_and(joins)
            && !text[at + len..].chars().next().is_some_and(joins)
    };
    match id {
        StableId::Video(v) => text
            .match_indices(v.as_str())
            .any(|(at, _)| free(text, at, v.len(), &|c| c.is_ascii() && id_byte(c as u8))),
        StableId::Post(p) => {
            let lower = text.to_ascii_lowercase();
            ["comments/", "redd.it/"].iter().any(|lead| {
                let needle = format!("{lead}{p}");
                lower.match_indices(&needle).any(|(at, _)| {
                    let token = lower[..at]
                        .rsplit(|c: char| {
                            c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | ',')
                        })
                        .next()
                        .unwrap_or("");
                    (*lead == "redd.it/" || token.contains("reddit.com/"))
                        && !lower[at + needle.len()..]
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphanumeric())
                })
            })
        }
        StableId::Repo(r) => {
            let lower = text.to_ascii_lowercase();
            let joins = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.');
            lower.match_indices(r.as_str()).any(|(at, _)| {
                let tail = &lower[at + r.len()..];
                let tail = tail.strip_prefix(".git").unwrap_or(tail);
                !lower[..at].chars().next_back().is_some_and(joins)
                    && !tail.chars().next().is_some_and(joins)
            })
        }
        StableId::Path(_) => false,
    }
}

/// What a stream shows the worker read, beyond the per-segment rule: the text of each command that runs a fetch
/// (`segment_runs_fetch` in any of its segments) and each fetch tool's call, the output of each that vouches
/// (`output_vouches`), and every path a file-reading tool opened. Only a stable id counts here; a plain URL keeps the
/// per-segment rule (the owner, 2026-10-31: "plain URLs keep the per-segment rule"), with one exception since plan
/// 47 P47d: an agy status line (`status`).
#[derive(Debug, Default)]
pub struct Reads {
    texts: Vec<String>,
    outputs: Vec<String>,
    paths: Vec<String>,
    /// Plan 47 P47d: each URL (normalized) on a status line of its own (`status_urls`) in the output of an agy
    /// `run_command` that fetches (`command_fetches`, on the part of a cut command agy shows), that agy answered
    /// `DONE` and that exited 0. This relaxes review round B1's "a plain URL never counts from an output" for this
    /// one shape, the owner told the trade (fewer wrong send-backs; a status line faked beside a real fetch would
    /// pass): agy cuts a command at 512 characters (all 786 cut lines in Megajam were 513), and in the 9 failed
    /// sources checks of 2026-11-01 to 11-20, 56 of the 75 URLs sent back as unopened had a `200 <url>` line in a
    /// `run_command` output. 21 of them count by this rule; the rest came from a command whose fetch lay past the cut
    /// (33) or from a script run by name (2), which show no fetch word. Claude's stream does not get it: its command
    /// is whole (454 Bash commands in 32 Claude research orders, 131 over 512 characters, none cut), so the
    /// per-segment rule already sees its URLs.
    status: Vec<String>,
    /// Addendum B (plan 40), for `media_read` only: each URL (normalized) a call that fetches or opens media named
    /// (`command_media`). The report's own calls (`finish`, `StructuredOutput`) and writes are not here: a cite names
    /// itself there.
    media_urls: Vec<String>,
    /// Each media file (`media_file`) a call opened and that was answered without an error: a `Read`/`read_file`/
    /// `view_file` result, or an ffprobe or ffmpeg input whose command exited 0.
    media_files: Vec<String>,
    /// Plan 43 A3: each local path a call read, as written (`local_path` form): a `Read`/`read_file`/`view_file`
    /// path, a `Grep` or `Glob` folder, and the paths a shell command that reads local files names
    /// (`shell_local_reads`). A relative one is resolved against the order's folders when a source is checked.
    local: Vec<LocalRead>,
}

/// One local path a call read. `search`: a search over a folder (`Grep`, `rg`, `grep`, `findstr`, `Select-String`),
/// which reads every file under it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalRead {
    path: String,
    search: bool,
    /// Fix round F4: the call that read it (a Claude `tool_use` id, or an agy step's tool and parameters), so an
    /// answer that is an error takes its reads back. Empty until `stream_reads` tags it.
    call: String,
}

/// Addendum B (plan 40): a `media.*` order's cited source counts as read when a call that fetches or opens media names
/// it (`yt-dlp <url>`, `ffprobe <url>`, `ffmpeg -i <url>`), or, for a YouTube URL, when its video id stands in a media
/// file the worker opened without an error (`...\youtube\8aQ8SoQl5EY\proxy.mp4`). Four of six `media.video` orders on
/// argon were sent back on 2026-10-31; `yt-breakdown` and `yt-breakdown-illit` cited the video whose downloaded copy
/// they had watched, which no fetch in their streams named. Review round (S1): the first cut counted any command or
/// `Read` path that named the URL or the id, so `echo <url>` or a `Read` of a missing `...\<id>\x` passed. `research.*`
/// keeps the stricter rules above. The oEmbed check of a cited video (`invented_videos`) still runs. A cited local
/// file keeps plan 35a's rule (`stable_read`, any class).
pub fn media_read(url: &str, reads: &Reads) -> bool {
    let norm = normalize_url(url);
    if !norm.is_empty() && reads.media_urls.contains(&norm) {
        return true;
    }
    match stable_id(url) {
        Some(id @ StableId::Video(_)) => reads.media_files.iter().any(|file| id_in(file, &id)),
        _ => false,
    }
}

/// Review round (S1): the files that hold a video's media: what a download leaves (video, audio, yt-dlp's
/// `.info.json`), the frames a worker cuts from it, and its captions. A plain `.json`, `.md` or `.txt` is not here:
/// the worker's own notes in the id's folder (yt-breakdown's `ingest.json`) are no sign it watched anything.
const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "webm", "mkv", "mov", "m4v", "avi", "m4a", "mp3", "wav", "opus", "ogg", "flac", "aac",
    "jpg", "jpeg", "png", "webp", "gif", "vtt", "srt", "ttml", "sbv", "json3",
];

fn media_file(path: &str) -> bool {
    let lower = path
        .trim_matches(|c: char| matches!(c, '"' | '\'' | '`' | ',' | '(' | ')' | '[' | ']'))
        .to_ascii_lowercase();
    lower.ends_with(".info.json")
        || lower.rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty() && !stem.ends_with(['/', '\\']) && MEDIA_EXTENSIONS.contains(&ext)
        })
}

/// Review round (S1): what one shell command shows of media, for `media_read`. URLs: every URL of a command that
/// fetches (`command_fetches`, the filter `research.*` uses), and the URLs a segment hands to ffprobe or to ffmpeg as
/// its `-i` input. Files: the media files (`media_file`) a segment hands to ffprobe, or to ffmpeg as `-i`; they count
/// only once the call is answered without an error, which the caller judges. ffmpeg's output file is not an input: a
/// command that writes `<id>\x.mp4` opened nothing of that video. ffprobe or ffmpeg inside a script's string
/// (`subprocess.run(["ffmpeg", ...])`) is not a command word (`segment_code` blanks strings); those workers' own
/// `read_file` of the media still counts.
fn command_media(cmd: &str) -> (Vec<String>, Vec<String>) {
    let mut urls = Vec::new();
    if command_fetches(cmd) {
        urls.extend(
            extract_url_tokens(cmd)
                .iter()
                .map(|token| normalize_url(token)),
        );
    }
    let mut files = Vec::new();
    for segment in segments(cmd) {
        let code = segment_code(segment).to_ascii_lowercase();
        let probe = command_words(&code, "ffprobe").next().is_some();
        let mpeg = command_words(&code, "ffmpeg").next().is_some();
        if !probe && !mpeg {
            continue;
        }
        let words: Vec<&str> = segment
            .split(|c: char| c.is_whitespace() || matches!(c, '"' | '\''))
            .filter(|word| !word.is_empty())
            .collect();
        for (at, word) in words.iter().enumerate() {
            let input = probe || (at > 0 && words[at - 1] == "-i");
            if !input {
                continue;
            }
            urls.extend(
                extract_url_tokens(word)
                    .iter()
                    .map(|token| normalize_url(token)),
            );
            if media_file(word) {
                files.push((*word).to_owned());
            }
        }
    }
    (urls, files)
}

/// agy's `run_command` output leads with `Command exited with code N.` when N is not 0 (`view::agy_result`).
fn agy_command_ok(output: &str) -> bool {
    output
        .strip_prefix("Command exited with code ")
        .and_then(|rest| rest.split('.').next())
        .and_then(|code| code.trim().parse::<i64>().ok())
        .is_none_or(|code| code == 0)
}

/// A command's segments: cut at every newline, and at `;`, `|` (so `||`) and `&&` outside a quote that closes on its
/// line (`quote_end`). A shell's quoted code (`opens_code`) is cut inside, as its shell would run it. Plan 47 P47d: the
/// cut was blind to quotes, so option3-amazon-bom's `bladebro act eval "a?.innerText || b?.innerText" --url "<url>"`
/// put its `--url` in a segment with no fetch word (3 URLs it had opened, sent back). A single `&` still cuts nothing:
/// PowerShell's `& "C:\x\curl.exe"` is a call.
fn segments(cmd: &str) -> impl Iterator<Item = &str> {
    let bytes = cmd.as_bytes();
    let mut out = Vec::new();
    let (mut start, mut at) = (0, 0);
    while at < bytes.len() {
        match bytes[at] {
            b'"' | b'\'' if !opens_code(&cmd[start..at]) => {
                if let Some(end) = quote_end(bytes, at) {
                    at = end;
                }
            }
            b'\n' | b';' | b'|' => {
                out.push(&cmd[start..at]);
                start = at + 1;
            }
            b'&' if bytes.get(at + 1) == Some(&b'&') => {
                out.push(&cmd[start..at]);
                start = at + 2;
                at += 1;
            }
            _ => {}
        }
        at += 1;
    }
    out.push(&cmd[start..]);
    out.into_iter()
}

/// agy's stream cuts a long `CommandLine` to its first 512 characters and this mark (66 of reel-motion-gemini-r2's 112
/// commands), so the call may lie past the cut, unseen.
const CUT_MARK: char = '\u{2026}';

/// The python modules that fetch: in a cut command, importing one counts as its fetch.
const FETCH_IMPORTS: &[&str] = &["yt_dlp", "urllib.request", "requests", "httpx"];

/// A whole command runs a fetch: one of its segments does, or runs a `gh` read, or the command is cut (`CUT_MARK`) and
/// imports a fetch module. A command that is not cut and only imports one fetches nothing.
fn command_fetches(cmd: &str) -> bool {
    segments(cmd).any(|segment| segment_runs_fetch(segment) || !gh_read_urls(segment).is_empty())
        || (cmd.trim_end().ends_with(CUT_MARK) && segments(cmd).any(imports_fetch))
}

/// A python `import` or `from ... import` line naming a fetch module.
fn imports_fetch(segment: &str) -> bool {
    let code = segment_code(segment);
    let line = code.trim();
    let names = if let Some(names) = line.strip_prefix("import ") {
        names
    } else if let Some(from) = line.strip_prefix("from ") {
        from.split(" import ").next().unwrap_or("")
    } else {
        return false;
    };
    names
        .split(',')
        .any(|name| FETCH_IMPORTS.contains(&name.split(" as ").next().unwrap_or("").trim()))
}

/// Plan 35 P35a review round (S2): the commands whose output is the worker's own files, not a page: they read or list
/// local files. `cat notes.md; curl -sI x` prints the notes as if they came back.
const LOCAL_READERS: &[&str] = &[
    "cat",
    "type",
    "get-content",
    "gc",
    "head",
    "tail",
    "more",
    "less",
    "sed",
    "awk",
    "grep",
    "rg",
    "findstr",
    "select-string",
    "sls",
    "import-csv",
    "ls",
    "dir",
    "get-childitem",
    "gci",
    "find",
    "tree",
];

/// The script calls that read a local file; `open(` only with no letter, digit, `_` or `.` before it (`urlopen(` and
/// `Image.open(` are not it).
const LOCAL_READ_CALLS: &[&str] = &[
    ".read_text(",
    ".read_bytes(",
    "readalltext(",
    "readalllines(",
];

/// Plan 35 P35a review round (S2): fetches whose output names ids they never opened: a YouTube search, playlist or
/// channel, a Reddit search or subreddit listing, the GitHub search API, a web search. A YouTube page fetched over
/// HTTP is one too (`listing`): the page holds its related videos.
pub const LISTING_MARKS: &[&str] = &[
    "ytsearch",
    "--flat-playlist",
    "extract_flat",
    "youtube.com/results",
    "search_query=",
    "youtube.com/playlist",
    "list=",
    "youtube.com/@",
    "youtube.com/channel/",
    "youtube.com/c/",
    "youtube.com/user/",
    "youtube.com/feed",
    "youtubei/",
    "reddit.com/search",
    "search.json",
    "search.rss",
    "/hot.json",
    "/new.json",
    "/top.json",
    "/rising.json",
    // and so `api.github.com/search`
    "github.com/search",
    "github.com/topics",
    "gh search",
    "google.com/search",
    "bing.com/search",
    "duckduckgo.com",
];

/// A segment reads a local file: a reader is one of its command words, or a file read stands in its code.
fn segment_reads_local(segment: &str) -> bool {
    let code = segment_code(segment).to_ascii_lowercase();
    LOCAL_READERS.iter().any(|word| {
        command_words(&code, word).any(|at| {
            code[at + word.len()..]
                .chars()
                .next()
                .is_none_or(char::is_whitespace)
        })
    }) || LOCAL_READ_CALLS.iter().any(|call| code.contains(call))
        || code.match_indices("open(").any(|(at, _)| {
            !code[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.'))
        })
}

/// A fetch lists what it did not open: it names a `LISTING_MARKS` page, or a YouTube page with no yt-dlp in the
/// command (yt-dlp reads the one video; the page's HTML holds its related videos too).
fn listing(lower: &str) -> bool {
    LISTING_MARKS.iter().any(|mark| lower.contains(mark))
        || (["youtube.com/", "youtu.be/"]
            .iter()
            .any(|host| lower.contains(host))
            && !["yt_dlp", "yt-dlp", "youtubedl"]
                .iter()
                .any(|word| lower.contains(word)))
}

/// A fetch's output vouches for the ids in it: the fetch is no listing and names no `file://`, and no pipeline of the
/// command starts by reading a local file. A reader later in a pipeline reads the fetch (`curl -s u | grep title`).
fn output_vouches(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    !lower.contains("file://")
        && !listing(&lower)
        && !text
            .split(['\n', ';'])
            .flat_map(|s| s.split("&&"))
            .filter_map(|pipeline| pipeline.split('|').next())
            .any(segment_reads_local)
}

fn json_text(val: &Value) -> String {
    let mut strings = Vec::new();
    collect_json_strings(val, &mut strings);
    strings.join("\n")
}

// ---- plan 43 A3: local sources ----

/// A command without the bodies of its heredocs (`<<EOF`, `<<'EOF'`, `<<-EOF`) and PowerShell here-strings
/// (`@'...'@`, `@"..."@`): text a command writes, never a path it reads. Megajam's s2-sound-audit-s1 wrote its whole
/// sources list into a report with `cat >> Docs/Audio/SOUND_AUDIT.md <<'EOF'`.
///
/// Fix round F4: a body fed to a shell (`bash <<EOF`) is shell text and stays in the first value; a body fed to a
/// script interpreter (`python - <<'EOF'`) is returned whole in the second, one script each, since its lines are no
/// shell segments: a path set on one line (`p=r"C:\x.csv"`) is opened on another (`open(p)`).
fn without_heredocs(cmd: &str) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut scripts = Vec::new();
    let mut lines = cmd.lines();
    while let Some(line) = lines.next() {
        // A here-string opens at a line's end and closes at a line's start.
        if let Some(at) = line.find("@'").or_else(|| line.find("@\"")) {
            if line[at + 2..].trim().is_empty() {
                let close = if line[at..].starts_with("@'") {
                    "'@"
                } else {
                    "\"@"
                };
                out.push_str(&line[..at]);
                out.push('\n');
                for body in lines.by_ref() {
                    if let Some(rest) = body.trim_start().strip_prefix(close) {
                        out.push_str(rest);
                        out.push('\n');
                        break;
                    }
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
        let Some(at) = line.find("<<") else {
            continue;
        };
        let tag: String = line[at + 2..]
            .trim_start_matches('-')
            .trim_start()
            .trim_start_matches(['\'', '"'])
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if tag.is_empty() {
            continue;
        }
        // A script fed to an interpreter (`python - <<'EOF'`) is code that may read; it is kept. Megajam's
        // w7-size-audit read its build's container_Win64.csv from such a body.
        let fed = split_command(&line[..at]).iter().find_map(|word| {
            let base = word
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let base = base.strip_suffix(".exe").unwrap_or(&base).to_owned();
            HEREDOC_READERS.contains(&base.as_str()).then_some(base)
        });
        let shell = fed
            .as_deref()
            .is_some_and(|base| matches!(base, "bash" | "sh" | "pwsh" | "powershell"));
        let mut script = String::new();
        for body in lines.by_ref() {
            if body.trim() == tag {
                break;
            }
            if shell {
                out.push_str(body);
                out.push('\n');
            } else if fed.is_some() {
                script.push_str(body);
                script.push('\n');
            }
        }
        if !script.is_empty() {
            scripts.push(script);
        }
    }
    (out, scripts)
}

/// The interpreters whose heredoc body is a script, not text written to a file.
const HEREDOC_READERS: &[&str] = &[
    "python",
    "python3",
    "py",
    "node",
    "bash",
    "sh",
    "pwsh",
    "powershell",
    "ruby",
    "perl",
];

/// A command cut at `;`, `|`, `&&`, `||` and newlines outside quotes (`segments` cuts inside them too: s2-sound-audit-s1's
/// `grep -n -i "sound\|A_" "Docs/FRONT_ASSETS.md"` lost its path). A quote after a backslash is no quote.
fn shell_segments(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c == '\\' && matches!(next, Some('"' | '\'')) {
            current.push(c);
            current.extend(next);
            at += 2;
            continue;
        }
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                current.push(c);
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    current.push(c);
                }
                '\n' | ';' | '|' => out.push(std::mem::take(&mut current)),
                '&' if next == Some('&') => {
                    out.push(std::mem::take(&mut current));
                    at += 1;
                }
                _ => current.push(c),
            },
        }
        at += 1;
    }
    out.push(current);
    out.retain(|segment| !segment.trim().is_empty());
    out
}

/// The contents of each quoted string in `segment`, `'...'` or `"..."`, a backslash before a quote escaping it.
fn quoted_parts(segment: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let bytes = segment.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let q = bytes[at];
        if q != b'"' && q != b'\'' {
            at += 1;
            continue;
        }
        let start = at + 1;
        let mut end = start;
        while end < bytes.len() && !(bytes[end] == q && bytes[end - 1] != b'\\') {
            end += 1;
        }
        if end >= bytes.len() {
            break;
        }
        parts.push(&segment[start..end]);
        at = end + 1;
    }
    parts
}

/// The folder a `cd`, `pushd`, `Set-Location` or `sl` segment moves to, as written.
fn moves_to(words: &[String]) -> Option<&str> {
    let first = words.first()?.to_ascii_lowercase();
    if !matches!(
        first.as_str(),
        "cd" | "pushd" | "set-location" | "sl" | "chdir"
    ) {
        return None;
    }
    words[1..]
        .iter()
        .find(|word| !word.starts_with('-'))
        .map(String::as_str)
}

/// Commands that read a file's bytes beside `LOCAL_READERS`: they count lines, hash or query it.
const LOCAL_COUNTERS: &[&str] = &["wc", "jq", "md5sum", "sha1sum", "sha256sum", "get-filehash"];

/// Commands that search every file under a folder.
const LOCAL_SEARCHERS: &[&str] = &["rg", "grep", "findstr", "select-string", "sls"];

/// Commands whose path arguments are written, not read.
const LOCAL_WRITERS: &[&str] = &[
    "tee",
    "out-file",
    "set-content",
    "add-content",
    "touch",
    "mkdir",
];

/// A word of a command that names a local path: a `/` or `\` in it or a file name's extension, no URL, flag or
/// variable.
fn path_word(word: &str) -> Option<String> {
    let word = word
        .trim_matches(|c: char| matches!(c, '`' | ',' | ':' | ';'))
        .trim_end_matches(')');
    if word.len() < 3
        || word.contains("://")
        || word.starts_with(['-', '$', '%', '@', '&', '|'])
        || word.contains(['*', '>', '<'])
    {
        return None;
    }
    let named = word.contains(['/', '\\'])
        || word.rsplit_once('.').is_some_and(|(stem, ext)| {
            !stem.is_empty()
                && (1..=8).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        });
    named.then(|| local_path(word.trim_start_matches("./")))
}

/// Plan 43 A3: the local paths a shell command reads. Megajam's a1-asset-wiring-audit-s1 read with `ls Docs/AssetGen/`
/// and `wc -l Docs/AssetGen/L02B_FIT/DECISIONS.md`, s2-sound-audit-s1 with `head -n 4 "Docs/Pipeline/CORE_SPEC.md"`, and
/// w7-size-audit with `cd ".../jobs"; head -5 container_Win64.csv`.
///
/// Fix round F4 (review finding 4): the first cut let a command that read anything vouch for every path word in it,
/// so `cat a.md; echo "Docs/B.md"` vouched for `Docs/B.md`. Now a path is read only when it is an argument of a
/// reading command (`segment_reads_local`, `LOCAL_COUNTERS`) in its own segment (`shell_segments`: cut at `;`, `&&`,
/// `||`, an unquoted `|` and newlines), and never a writer's (`LOCAL_WRITERS`) or a redirect's target. Heredoc bodies
/// a command writes are gone (`without_heredocs`). A script body fed to an interpreter is one unit: when a line of it
/// reads (`open(`, `.read_text(`, ...), each path in its quoted strings counts, a path being set on one line and
/// opened on the next. A relative path after a `cd` in the same command is under that folder, and a reader with no
/// path reads that folder (`cd x && ls -la`). A variable set in an earlier segment (`S="C:/x"; ls "$S/fab"`, pwsh's
/// `$S = "C:/x"`) is read as its value: Megajam's a1-asset-wiring-audit-s1 listed its sourced assets so. Not covered,
/// so stricter than a worker's real reads: a `for f in a b; do cat "$f"` loop vouches for neither file.
fn shell_local_reads(cmd: &str) -> Vec<LocalRead> {
    let (text, scripts) = without_heredocs(cmd);
    let segs = shell_segments(&text);
    let runs = |segment: &str, words: &[&str]| {
        let code = segment_code(segment).to_ascii_lowercase();
        words
            .iter()
            .any(|word| command_words(&code, word).next().is_some())
    };
    let mut reads = Vec::new();
    let mut folder: Option<String> = None;
    let push = |reads: &mut Vec<LocalRead>, folder: &Option<String>, path: String, search: bool| {
        // After a `cd`, a relative path is under its folder; it is kept as written too, since an order's `cd` is most
        // often into its own folder, which the check resolves against anyway.
        if let Some(dir) = folder.as_ref().filter(|_| !absolute(&path)) {
            reads.push(LocalRead {
                path: local_path(&format!("{dir}/{path}")),
                search,
                call: String::new(),
            });
        }
        reads.push(LocalRead {
            path,
            search,
            call: String::new(),
        });
    };
    let mut vars: Vec<(String, String)> = Vec::new();
    // Fix round 2 (H6 c, d): the words of each open `for NAME in <words>` loop, and the variables and folder outside
    // each open `( ... )` subshell, put back when it closes.
    let mut loops: Vec<(String, Vec<String>)> = Vec::new();
    let mut scopes: Vec<(Vec<(String, String)>, Option<String>)> = Vec::new();
    let mut closing = false;
    for raw in &segs {
        if std::mem::take(&mut closing) {
            if let Some((outer_vars, outer_folder)) = scopes.pop() {
                vars = outer_vars;
                folder = outer_folder;
            }
        }
        let mut owned = raw.trim().to_owned();
        if let Some(inner) = owned.strip_prefix('(') {
            scopes.push((vars.clone(), folder.clone()));
            owned = inner.trim_start().to_owned();
        }
        if owned.ends_with(')') && owned.matches(')').count() > owned.matches('(').count() {
            owned.pop();
            closing = true;
        }
        let owned = match owned.split_once(char::is_whitespace) {
            Some((head, rest)) if matches!(head, "do" | "then" | "else") => rest.trim().to_owned(),
            _ => owned,
        };
        let segment = &owned;
        let words = split_command(segment);
        if let Some(to) = moves_to(&words) {
            let to = match &folder {
                Some(from) if !absolute(to) => format!("{from}/{to}"),
                _ => to.to_owned(),
            };
            folder = Some(to);
            continue;
        }
        if let Some((name, value)) = assignment(&words) {
            vars.retain(|(known, _)| *known != name);
            vars.push((name, value));
            continue;
        }
        if words.first().map(String::as_str) == Some("for")
            && words.len() > 3
            && words[2] == "in"
        {
            loops.push((words[1].clone(), words[3..].to_vec()));
            continue;
        }
        if words.first().map(String::as_str) == Some("done") {
            loops.pop();
            continue;
        }
        // Fix round 2 (H6 b): `git show <rev>:<path>` and `git cat-file -p <rev>:<path>` read `<path>` at `<rev>`.
        if words.first().is_some_and(|word| word.eq_ignore_ascii_case("git"))
            && words
                .get(1)
                .is_some_and(|sub| matches!(sub.as_str(), "show" | "cat-file"))
        {
            for word in &words[2..] {
                let Some((rev, path)) = word.split_once(':') else {
                    continue;
                };
                if rev.is_empty() || rev.contains(['/', '\\']) || path.is_empty() {
                    continue;
                }
                if let Some(path) = path_word(path) {
                    push(&mut reads, &folder, path, false);
                }
            }
            continue;
        }
        let reads_here = segment_reads_local(segment) || runs(segment, LOCAL_COUNTERS);
        if !reads_here || runs(segment, LOCAL_WRITERS) {
            continue;
        }
        let search = runs(segment, LOCAL_SEARCHERS);
        let expanded = expand_vars(segment, &vars);
        // A loop's variable read as each of its words.
        let mut variants = vec![expanded];
        for (name, values) in &loops {
            let marks = [format!("${name}"), format!("${{{name}}}")];
            variants = variants
                .into_iter()
                .flat_map(|variant| {
                    if marks.iter().any(|mark| variant.contains(mark.as_str())) {
                        values
                            .iter()
                            .map(|value| {
                                marks
                                    .iter()
                                    .fold(variant.clone(), |text, mark| text.replace(mark.as_str(), value))
                            })
                            .collect::<Vec<_>>()
                    } else {
                        vec![variant]
                    }
                })
                .collect();
        }
        for segment in &variants {
        let words = split_command(segment);
        let patterns = pattern_words(&words);
        let mut named = Vec::new();
        // A script's quoted strings, read whole from the raw text: `r"C:\x\a.csv"` loses its quotes to the words.
        for part in quoted_parts(segment) {
            if patterns.iter().any(|at| words[*at] == part) {
                continue;
            }
            if let Some(path) = path_word(part.trim()) {
                named.push(path);
            }
        }
        for (at, word) in words.iter().enumerate() {
            let after_redirect = at > 0 && words[at - 1].ends_with('>');
            if after_redirect || word.starts_with('>') || word.contains(">>") || patterns.contains(&at) {
                continue;
            }
            let whole = !word.contains(['(', ')', '\'', '"', '=', ';', '[', ']', '{', '}']);
            let pieces: Vec<&str> = if whole {
                vec![word.as_str()]
            } else {
                word.split(['(', ')', '\'', '"', '=', ';', '[', ']', '{', '}', ','])
                    .collect()
            };
            for piece in pieces {
                if let Some(path) = path_word(piece.trim()) {
                    named.push(path);
                }
            }
        }
        // A reader with no path reads its folder: a1-asset-wiring-audit's `cd ".../assets/sourced" && ls -la`.
        if named.is_empty() {
            if let Some(dir) = &folder {
                reads.push(LocalRead {
                    path: local_path(dir),
                    search,
                    call: String::new(),
                });
            }
        }
        for path in named {
            push(&mut reads, &folder, path, search);
        }
        }
    }
    for script in &scripts {
        if !script.lines().any(segment_reads_local) {
            continue;
        }
        for line in script.lines() {
            for part in quoted_parts(line) {
                if let Some(path) = path_word(part.trim()) {
                    push(&mut reads, &folder, path, false);
                }
            }
        }
    }
    reads
}

/// Fix round 2 (H6 a, review): the indices of `words` that are a search's pattern, never a path it reads: `grep`'s and
/// `rg`'s first operand (or each `-e`/`--regexp` value, after which every operand is a path), `findstr`'s first
/// operand and its `/c:` string, `Select-String`'s `-Pattern` value (or its first operand), and the value of `find`'s
/// `-name`, `-iname`, `-path`, `-ipath`, `-wholename` and `-regex`. `grep -rn "Docs/B.md" .` vouched for `Docs/B.md`.
fn pattern_words(words: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    let base = |word: &str| {
        let lower = word.rsplit(['/', '\\']).next().unwrap_or(word).to_ascii_lowercase();
        lower.strip_suffix(".exe").unwrap_or(&lower).to_owned()
    };
    let mut at = 0;
    while at < words.len() {
        let command = base(&words[at]);
        let rest = at + 1;
        match command.as_str() {
            "grep" | "rg" | "egrep" | "fgrep" => {
                let mut explicit = false;
                let mut first = None;
                let mut i = rest;
                while i < words.len() && !matches!(words[i].as_str(), "|" | ";") {
                    let word = words[i].as_str();
                    if matches!(word, "-e" | "--regexp" | "-f" | "--file") {
                        explicit = true;
                        out.push(i + 1);
                        i += 2;
                        continue;
                    }
                    // A flag whose value is the next word: no operand.
                    if matches!(
                        word,
                        "-A" | "-B" | "-C" | "-m" | "-g" | "-t" | "-T" | "-d" | "-D" | "--glob" | "--type"
                            | "--type-not" | "--max-count" | "--include" | "--exclude" | "--exclude-dir"
                            | "--context" | "--after-context" | "--before-context"
                    ) {
                        i += 2;
                        continue;
                    }
                    if word.starts_with("--regexp=") {
                        explicit = true;
                        out.push(i);
                    } else if !word.starts_with('-') && first.is_none() {
                        first = Some(i);
                    }
                    i += 1;
                }
                if let (false, Some(first)) = (explicit, first) {
                    out.push(first);
                }
            }
            "findstr" => {
                let mut first = None;
                for (i, word) in words.iter().enumerate().skip(rest) {
                    let lower = word.to_ascii_lowercase();
                    if lower.starts_with("/c:") || lower.starts_with("/g:") {
                        out.push(i);
                        first = Some(usize::MAX);
                    } else if !word.starts_with('/') && first.is_none() {
                        first = Some(i);
                    }
                }
                if let Some(first) = first.filter(|first| *first != usize::MAX) {
                    out.push(first);
                }
            }
            "select-string" | "sls" => {
                let mut named = false;
                let mut first = None;
                let mut i = rest;
                while i < words.len() {
                    let lower = words[i].to_ascii_lowercase();
                    if lower.starts_with('-') {
                        if "-pattern".starts_with(lower.as_str()) && lower.len() >= 2 {
                            named = true;
                            out.push(i + 1);
                        }
                        i += 2;
                        continue;
                    }
                    if first.is_none() {
                        first = Some(i);
                    }
                    i += 1;
                }
                if let (false, Some(first)) = (named, first) {
                    out.push(first);
                }
            }
            "find" => {
                for i in rest..words.len() {
                    if matches!(
                        words[i].as_str(),
                        "-name" | "-iname" | "-path" | "-ipath" | "-wholename" | "-regex" | "-iregex"
                    ) {
                        out.push(i + 1);
                    }
                }
            }
            _ => {}
        }
        at += 1;
    }
    out.retain(|i| *i < words.len());
    out
}

/// A shell variable set by a segment of its own: bash's `NAME=value` (one word once quotes are gone) or pwsh's
/// `$NAME = value`. The name and the value as written.
fn assignment(words: &[String]) -> Option<(String, String)> {
    let ident = |name: &str| {
        !name.is_empty()
            && !name.starts_with(|c: char| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    match words {
        [one] => one
            .split_once('=')
            .filter(|(name, value)| ident(name) && !value.is_empty())
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
        [name, eq, value] if eq == "=" => name
            .strip_prefix('$')
            .filter(|name| ident(name))
            .map(|name| (name.to_owned(), value.clone())),
        _ => None,
    }
}

/// `segment` with each `$NAME` and `${NAME}` of `vars` replaced by its value; any other `$` is left.
fn expand_vars(segment: &str, vars: &[(String, String)]) -> String {
    if vars.is_empty() || !segment.contains('$') {
        return segment.to_owned();
    }
    let mut out = String::with_capacity(segment.len());
    let mut rest = segment;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let (name, used) = match after.strip_prefix('{') {
            Some(inner) => match inner.find('}') {
                Some(end) => (&inner[..end], end + 2),
                None => ("", 0),
            },
            None => {
                let end = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(after.len());
                (&after[..end], end)
            }
        };
        match vars.iter().find(|(known, _)| known == name) {
            Some((_, value)) if !name.is_empty() => {
                out.push_str(value);
                rest = &after[used..];
            }
            _ => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A cited source that is no web page: a path, absolute or relative to the repository, or a `file://` URL. A bare
/// domain (`docs.rs/serde`) and `www.` are the web.
///
/// Fix round F5 (review finding 5): the first cut took anything with no `://`, `www.` or `host.tld/` as local, so
/// `Smith et al 2023`, `RFC 9110` or a bare `example.com` was a local entry, which research.read skips, and its axis
/// passed with "0 source(s) cited, each verified". Now an entry is local only when it holds a `/` or `\`, or one of
/// its pieces (`local_pieces`, a parenthesised note dropped) ends in a known file extension (a list fix round 2 removed);
/// anything else is checked as a web source and fails unfetched, as before plan 43. Megajam's a1-asset-wiring-audit
/// cited `git ls-tree -r --name-only HEAD (assets/, Content/, Source/)`: a slash, so local, and not checked.
///
/// Fix round 2 (H7, review): a slash alone made `N/A`, `and/or` and `TCP/IP` local, a known extension made `docs.rs`
/// and `tokio.rs` local, and `Makefile`, `LICENSE`, `.gitignore` or an `.hlsl` on disk read as the web. Now a piece of
/// the entry (`local_pieces`) is local when it resolves to a file or folder that exists, absolute or under one of
/// `roots` (the worktree, then the project), or when it holds a path separator and its last component has a file
/// extension (`Docs/x.md`, `Shaders/ink.hlsl`); `file://` entries are read the same way.
pub fn local_entry(url: &str, roots: &[&Path]) -> bool {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    // A `host.tld/` anywhere in it, not only first: `see example.com/x` holds a slash and is the web.
    let web = !lower.starts_with("file://")
        && (lower.contains("://") || lower.starts_with("www.") || bare_domain(url).is_some());
    if web {
        return false;
    }
    let on_disk = |piece: &str| {
        if absolute(piece) {
            Path::new(piece).exists()
        } else {
            roots.iter().any(|root| root.join(piece).exists())
        }
    };
    // Megajam's a1-asset-wiring-audit cited `git ls-tree -r --name-only HEAD (assets/, Content/, Source/)`: a command
    // over folders named in its note, which exist; such an entry is local when a folder or file its note names exists.
    let mut noted = String::new();
    let mut depth = 0usize;
    for c in url.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                noted.push(',');
            }
            _ if depth > 0 => noted.push(c),
            _ => {}
        }
    }
    if local_pieces(&noted).iter().any(|piece| on_disk(piece)) {
        return true;
    }
    local_pieces(url).iter().any(|piece| {
        let on_disk = on_disk(piece);
        let last = piece
            .replace('\\', "/")
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_owned();
        let shaped = piece.contains(['/', '\\'])
            && last.rsplit_once('.').is_some_and(|(stem, ext)| {
                !stem.is_empty()
                    && (1..=8).contains(&ext.len())
                    && ext.chars().all(|c| c.is_ascii_alphanumeric())
                    && ext.chars().any(|c| c.is_ascii_alphabetic())
            });
        // A folder written as one (`Docs/AssetGen/`, `.nirvana/lessons/`) is a path too; `N/A` and `TCP/IP` are not.
        let folder = piece.len() > 1 && piece.ends_with(['/', '\\']);
        on_disk || shaped || folder
    })
}

/// The paths one cited local entry names, as written. Megajam's research.read orders cited `Config/DefaultEngine.ini,
/// Config/DefaultGame.ini`, `<folder>/ (folder listing)` and `Front/InkTitleRoot.cpp, InkTitleGameMode.cpp` (the
/// second in the first's folder). Parenthesised notes, `:line` and `#L` tails and a trailing `/*` are dropped.
pub fn local_pieces(entry: &str) -> Vec<String> {
    let mut text = String::new();
    let mut depth = 0usize;
    for c in entry.trim().chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => text.push(c),
            _ => {}
        }
    }
    let text = match text.trim().strip_prefix("file://") {
        Some(rest) => {
            let rest = percent_decoded(rest);
            let b = rest.as_bytes();
            if b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
                rest[1..].to_owned()
            } else {
                rest
            }
        }
        None => text.trim().to_owned(),
    };
    let mut pieces = Vec::new();
    let mut folder: Option<String> = None;
    for piece in text
        .split([',', ';', '+'])
        .flat_map(|part| part.split(" and "))
        .map(|piece| piece.trim().trim_matches(['`', '"', '\'']).trim())
    {
        let mut piece = piece.split('#').next().unwrap_or("").trim().to_owned();
        // `x.rs:120` or `x.rs:12-40`, never a drive's `C:`.
        if let Some((head, tail)) = piece.rsplit_once(':') {
            if head.len() > 1
                && !tail.is_empty()
                && tail.chars().all(|c| c.is_ascii_digit() || c == '-')
            {
                piece = head.to_owned();
            }
        }
        let piece = piece
            .trim_end_matches("/*")
            .trim_end_matches("\\*")
            .trim_end_matches("/**")
            .to_owned();
        if piece.is_empty() || (piece.contains(char::is_whitespace) && !piece.contains(['/', '\\']))
        {
            continue;
        }
        let piece = match &folder {
            Some(dir) if !piece.contains(['/', '\\']) => format!("{dir}/{piece}"),
            _ => piece,
        };
        folder = piece
            .replace('\\', "/")
            .trim_end_matches('/')
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_owned());
        pieces.push(piece);
    }
    pieces
}

/// A path absolute on this machine: a drive (`C:/`, `C:\`), a root `/`, or a share `\\`.
fn absolute(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'/' | b'\\'))
        || path.starts_with('/')
        || path.starts_with("\\\\")
}

/// `path` in `local_path` form with `.` segments and trailing separators gone, and Git Bash's `/c/x` read as `c:/x`.
fn clean_path(path: &str) -> String {
    let mut path = local_path(path);
    let b = path.as_bytes();
    if b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/' {
        path = format!("{}:{}", &path[1..2], &path[2..]);
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "." => {}
            ".." if parts
                .last()
                .is_some_and(|last| !last.is_empty() && *last != "..") =>
            {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let joined = parts.join("/");
    let trimmed = joined.trim_end_matches('/');
    if trimmed.is_empty() {
        joined
    } else {
        trimmed.to_owned()
    }
}

/// Where a path written in a stream or a Work Report may be: itself when absolute, else under each of `roots`.
fn resolved(path: &str, roots: &[&Path]) -> Vec<String> {
    if absolute(path) {
        return vec![clean_path(path)];
    }
    let mut places: Vec<String> = roots
        .iter()
        .map(|root| clean_path(&format!("{}/{path}", root.display())))
        .collect();
    places.push(clean_path(path));
    places
}

/// Plan 43 A3: a cited local entry the stream shows read: each of its pieces (`local_pieces`) is a path a call read,
/// resolved against the order's folders (`roots`: its worktree, then the project), or lies under a folder a search
/// read. A search of a whole root vouches for no file in it.
pub fn local_read(entry: &str, reads: &Reads, roots: &[&Path]) -> bool {
    let pieces = local_pieces(entry);
    if pieces.is_empty() {
        return false;
    }
    let tops: Vec<String> = roots
        .iter()
        .map(|root| clean_path(&root.display().to_string()))
        .collect();
    pieces.iter().all(|piece| {
        let cited = resolved(piece, roots);
        reads.local.iter().any(|read| {
            resolved(&read.path, roots).iter().any(|place| {
                cited.contains(place)
                    || (read.search
                        && !tops.contains(place)
                        && !tops.iter().any(|top| top.starts_with(&format!("{place}/")))
                        && cited.iter().any(|c| c.starts_with(&format!("{place}/"))))
            })
        })
    })
}

pub fn stream_reads(stream: &str) -> Reads {
    let mut reads = Reads::default();
    // Fix round F4: each local read and file path with the call that made it; an answer that is an error takes that
    // call's back (`Read` of a missing file, a shell read whose command failed: Claude's `is_error`, agy's `ERROR` or
    // its `Command exited with code N`). A call with no answer in the stream keeps its reads, as `media_files` does
    // not: an excerpt or a killed worker leaves calls unanswered, and an unanswered read is no error.
    let mut paths: Vec<(String, String)> = Vec::new();
    let tag = |mut found: Vec<LocalRead>, call: &str| {
        for read in &mut found {
            read.call = call.to_owned();
        }
        found
    };
    let mut fetch_ids: Vec<String> = Vec::new();
    // Review round (S1): a Claude call's media files wait here for its `tool_result`; only one without `is_error`
    // counts them. A `Read` of a missing file or a failed ffprobe answers with `is_error: true`.
    let mut media_pending: Vec<(String, Vec<String>)> = Vec::new();
    for line in stream.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if let Some(content) = val.pointer("/message/content").and_then(Value::as_array) {
            for block in content {
                let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        let input = block.get("input").unwrap_or(&Value::Null);
                        let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                        // Review round (S2): WebFetch's `prompt` is the worker's own text; only its `url` was read.
                        if let Some(command) = input
                            .get("command")
                            .and_then(Value::as_str)
                            .filter(|_| matches!(name, "Bash" | "PowerShell"))
                        {
                            let (urls, files) = command_media(command);
                            reads.media_urls.extend(urls);
                            media_pending.push((id.to_owned(), files));
                            reads.local.extend(tag(shell_local_reads(command), id));
                        }
                        // Plan 43 A3: a search names its folder; with none it searched the worker's folder, which
                        // vouches for no one file.
                        if let Some(path) = input
                            .get("path")
                            .and_then(Value::as_str)
                            .filter(|_| matches!(name, "Grep" | "Glob"))
                        {
                            reads.local.push(LocalRead {
                                path: local_path(path),
                                search: name == "Grep",
                                call: id.to_owned(),
                            });
                        }
                        let text = match name {
                            "Bash" | "PowerShell" => input
                                .get("command")
                                .and_then(Value::as_str)
                                .filter(|c| command_fetches(c))
                                .map(str::to_owned),
                            "WebFetch" => {
                                input.get("url").and_then(Value::as_str).map(str::to_owned)
                            }
                            _ if name.starts_with("mcp__")
                                && ["browser", "bladebro", "chrome", "playwright"]
                                    .iter()
                                    .any(|w| name.contains(w)) =>
                            {
                                Some(json_text(input))
                            }
                            "Read" => {
                                if let Some(path) = input.get("file_path").and_then(Value::as_str) {
                                    paths.push((id.to_owned(), local_path(path)));
                                    reads.local.push(LocalRead {
                                        path: local_path(path),
                                        search: false,
                                        call: id.to_owned(),
                                    });
                                    if media_file(path) {
                                        media_pending.push((id.to_owned(), vec![path.to_owned()]));
                                    }
                                }
                                None
                            }
                            _ => None,
                        };
                        if let Some(text) = text {
                            if output_vouches(&text) {
                                fetch_ids.push(id.to_owned());
                            }
                            reads.texts.push(text);
                        }
                    }
                    Some("tool_result") => {
                        let id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if !id.is_empty() && fetch_ids.iter().any(|f| f == id) {
                            reads
                                .outputs
                                .push(json_text(block.get("content").unwrap_or(&Value::Null)));
                        }
                        if let Some(at) = media_pending.iter().position(|(call, _)| call == id) {
                            let (_, files) = media_pending.swap_remove(at);
                            if block.get("is_error").and_then(Value::as_bool) != Some(true) {
                                reads.media_files.extend(files);
                            }
                        }
                        let errored = block.get("is_error").and_then(Value::as_bool) == Some(true);
                        if errored && !id.is_empty() {
                            reads.local.retain(|read| read.call != id);
                            paths.retain(|(call, _)| call != id);
                        }
                    }
                    _ => {}
                }
            }
        }
        let step = val.get("step_update").unwrap_or(&val);
        if step.get("step_type").and_then(Value::as_str) != Some("tool") {
            continue;
        }
        let tool = step.get("tool_name").and_then(Value::as_str).unwrap_or("");
        let params = &step["tool_info"]["parameters"];
        let output = step["tool_info"]["output"].as_str().unwrap_or("");
        // Review round (S1): agy answers a step on a later line with the same step, `DONE` or `ERROR` (an error under
        // `tool_info.error`); the `ACTIVE` line is the call alone.
        let answered = step.get("state").and_then(Value::as_str) == Some("DONE")
            && step["tool_info"].get("error").is_none_or(Value::is_null);
        let state = step.get("state").and_then(Value::as_str);
        // Fix round F4: an agy step's call is its tool and parameters, the same on its `ACTIVE` and its answer's line.
        // An answer that failed takes the step's reads back and adds none.
        let call = format!("agy:{tool}:{params}");
        let failed = state == Some("ERROR")
            || !step["tool_info"].get("error").is_none_or(Value::is_null)
            || (tool == "run_command" && state == Some("DONE") && !agy_command_ok(output));
        if failed {
            reads.local.retain(|read| read.call != call);
            paths.retain(|(at, _)| *at != call);
        }
        let text = match tool {
            "run_command" => {
                let command = params["CommandLine"].as_str().unwrap_or("");
                let (urls, files) = command_media(command);
                reads.media_urls.extend(urls);
                // Plan 43 A3: agy repeats a step on its `DONE` line; a path read twice is still one read.
                if !failed {
                    reads.local.extend(tag(shell_local_reads(command), &call));
                }
                if answered && agy_command_ok(output) {
                    reads.media_files.extend(files);
                }
                // Plan 47 P47d: a `file://` fetch prints a file, not a page (the B1 review's notes fetched as a file).
                let fetches = command_fetches(command);
                if fetches
                    && answered
                    && agy_command_ok(output)
                    && !command.to_ascii_lowercase().contains("file://")
                {
                    reads.status.extend(status_urls(output));
                }
                command_fetches(command).then(|| command.to_owned())
            }
            "read_file" | "view_file" => {
                for key in ["Path", "AbsolutePath", "file_path"] {
                    if let Some(path) = params.get(key).and_then(Value::as_str).filter(|_| !failed)
                    {
                        paths.push((call.clone(), local_path(path)));
                        reads.local.push(LocalRead {
                            path: local_path(path),
                            search: false,
                            call: call.clone(),
                        });
                        if answered && media_file(path) {
                            reads.media_files.push(path.to_owned());
                        }
                    }
                }
                None
            }
            _ if agy_fetch_tool(tool, params) => Some(json_text(params)),
            _ => None,
        };
        if let Some(text) = text {
            if output_vouches(&text) {
                reads.outputs.push(output.to_owned());
            }
            reads.texts.push(text);
        }
    }
    reads.paths = paths.into_iter().map(|(_, path)| path).collect();
    reads
}

/// A cited source the stream shows read beyond the per-segment rule (`stream_reads`): its stable id in a fetch's text
/// or in an output that vouches, or its path opened by a file tool. Any other URL: never here.
pub fn stable_read(url: &str, reads: &Reads) -> bool {
    match stable_id(url) {
        Some(StableId::Path(path)) => reads.paths.contains(&path),
        Some(id) => reads
            .texts
            .iter()
            .chain(&reads.outputs)
            .any(|text| id_in(text, &id)),
        None => false,
    }
}

/// Plan 47 P47d: the URLs (normalized) an output names on status lines: a line that is a 2xx or 3xx code and one
/// http(s) URL and nothing else but whitespace, as `curl -w "%{http_code} %{url_effective}\n"` prints it. A 4xx, a 5xx
/// or curl's `000` (no answer) opened nothing, and a line with other text on it is no status line.
fn status_urls(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let (code, url) = (words.next()?, words.next()?);
            let status = code.len() == 3
                && code.bytes().all(|b| b.is_ascii_digit())
                && matches!(code.as_bytes()[0], b'2' | b'3');
            (status
                && words.next().is_none()
                && (url.starts_with("https://") || url.starts_with("http://")))
            .then(|| normalize_url(url))
        })
        .collect()
}

/// Plan 47 P47d: a cited URL an agy fetch printed on a status line of its own (`Reads::status`), compared as the axis
/// compares any URL (`normalize_url`).
pub fn status_read(url: &str, reads: &Reads) -> bool {
    let norm = normalize_url(url);
    !norm.is_empty() && reads.status.contains(&norm)
}

// ---- live checks 2026-11-01 F2: sources only when the order asks for the web ----

/// The class that is the web (ROLE.md: "Web research, shopping and reading pages are `research.web`").
pub const WEB_CLASS: &str = "research.web";

/// The needs that put the web in an order: the two that grant bladebro (`nirvana_bladebro::GRANTING`), and `web`.
const WEB_NEEDS: [&str; 3] = ["browser", "bladebro", "web"];

/// Words that name the web outright, lowercase: they ask for it wherever they stand. `online` and `google` alone are
/// left out, since a research order names "online docs" or a Google API as often as it asks for a page.
const WEB_NAMES: &[&str] = &[
    "www.",
    "the web",
    "web search",
    "web page",
    "webpage",
    "on the internet",
    "search the internet",
    "find online",
    "search online",
    "look online",
    "youtube",
    "youtu.be",
    "tiktok",
    "instagram",
    "reddit",
    "wikipedia",
];

/// Words that ask for a lookup (gsrc review S3): the web, unless their sentence points at the project
/// (`LOCAL_MARKS`), as "look up the commit rule in CLAUDE.md" or "transcribe the podcast at E:/pod.mp3" do. Review
/// S5: a bare `website` or `internet` matched orders about a site's own code, so only these forms of them count.
const LOOKUP_WORDS: &[&str] = &[
    "look up",
    "search for",
    "latest price",
    "current price",
    "reviews of",
    "latest episode",
    "latest video",
    "'s channel",
    "podcast",
    "transcript of",
    "transcribe the",
    "official website",
    "'s website",
    "website of",
    "on their website",
    "on the website",
];

/// What in a sentence points at the project or the machine rather than the web, lowercase.
const LOCAL_MARKS: &[&str] = &[
    ":/", ":\\", "src/", "./", "~/", ".md", ".rs", ".py", ".toml", ".json", ".txt", ".html",
    ".pdf", ".mp4", ".mp3", ".wav", ".mov", ".mkv", ".m4a", " file", " folder", " repo",
    "codebase", "the code",
];

/// A host that is this machine (review S5): `http://localhost:8770/api` in a research order is not the web.
fn local_host(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority
        .rsplit('@')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or("").to_owned(),
        None => host.split(':').next().unwrap_or("").to_owned(),
    };
    host == "localhost"
        || host.ends_with(".localhost")
        || host.starts_with("127.")
        || host == "0.0.0.0"
        || host == "::1"
        || host.is_empty()
}

/// A bare domain with a path at the start of a word, `name.tld/` (review S3): `github.com/org/repo`, `docs.rs/serde`.
/// A path segment (`nirvana/controls.d/x`), a dotfile (`./x`, `.claude/`) and a version (`v0.52/`) are not.
fn bare_domain(written: &str) -> Option<&str> {
    written.split_whitespace().find_map(|word| {
        let word = word.trim_start_matches(['(', '"', '\'', '`', '<', '[']);
        let (host, _) = word.split_once('/')?;
        let labels: Vec<&str> = host.split('.').collect();
        let tld = labels.last()?;
        let named = labels.len() >= 2
            && labels.iter().all(|label| {
                !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
            && tld.len() >= 2
            && tld.chars().all(|c| c.is_ascii_alphabetic());
        named.then_some(host)
    })
}

/// `word` in `text` as a word, an `s` after it allowed: no letter or digit runs on into an end of it that is one
/// (`the web` is not in `the webhook`; `podcasts` holds `podcast`).
fn has_word(text: &str, word: &str) -> bool {
    let joins = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    let (first, last) = (word.chars().next(), word.chars().next_back());
    text.match_indices(word).any(|(at, _)| {
        let after = &text[at + word.len()..];
        let after = after.strip_prefix('s').unwrap_or(after);
        (!joins(first) || !joins(text[..at].chars().next_back()))
            && (!joins(last) || !joins(after.chars().next()))
    })
}

/// The order's sentences, cut at line ends and at `. `, `? ` and `! ` (so `CLAUDE.md` stays whole).
fn sentences(lower: &str) -> impl Iterator<Item = &str> {
    lower
        .split('\n')
        .flat_map(|line| line.split(". "))
        .flat_map(|part| part.split("? "))
        .flat_map(|part| part.split("! "))
}

/// herdr's sources line for an order that asks for the web: the Work Report cites what it read (plan 24 §5).
pub const WEB_SOURCES: &str =
    "Your Work Report must include the `sources` array of `{url, used_for}` for each source consulted.";

/// The start of `NO_WEB_SOURCES`, short so a re-wrapped order copy still carries it whole on its first line (review
/// N3); `spec` reads a line starting with it as `no_web`.
pub const NO_WEB_MARK: &str = "herdr found no web page asked for in this order";

/// herdr's sources line for a research or media order that does not ask for the web. lc4-s1 ran `curl
/// https://example.com` "as required by the order's sources specification", which was `WEB_SOURCES` in an order that
/// quoted rules from memory (live checks 2026-11-01, F2). Review S2: an order `web_asked` missed may still need a page,
/// so the line never talks the worker out of a fetch the answer needs, nor into one it does not.
pub const NO_WEB_SOURCES: &str = "herdr found no web page asked for in this order, so the gate asks you for no source. \
If answering needs a page you do not have, open it and cite it in the Work Report's `sources` array (`{url, \
used_for}`); never open one only to have a source.";

/// Why an order asks for the web, read at seating from its class, its needs and its own text (`seat run` passes
/// `nirvana_profile_detect::own_text`, which cuts herdr's lessons, cards and lines: they name pages of their own, and
/// `NO_WEB_SOURCES` says `web page`); `None` when it does not. Live checks 2026-11-01 F2: lc3 (an
/// MCP call) and lc4 (rules from memory) were sent back "no sources", and both redos fetched a page unrelated to the
/// order to have one to cite.
pub fn web_asked(class: &str, needs: &[String], written: &str) -> Option<String> {
    if class == WEB_CLASS {
        return Some(format!("the class is {WEB_CLASS}"));
    }
    if let Some(need) = needs.iter().find(|need| WEB_NEEDS.contains(&need.as_str())) {
        return Some(format!("the order needs {need}"));
    }
    if let Some(url) = extract_url_tokens(written)
        .into_iter()
        .find(|url| !local_host(url))
    {
        return Some(format!("the order names {url}"));
    }
    if let Some(host) = bare_domain(written) {
        return Some(format!("the order names {host}/"));
    }
    let lower = written.to_lowercase();
    if let Some(word) = WEB_NAMES.iter().find(|word| has_word(&lower, word)) {
        return Some(format!("the order says `{}`", word.trim_end_matches('.')));
    }
    let lookup = sentences(&lower)
        .filter(|sentence| !LOCAL_MARKS.iter().any(|mark| sentence.contains(mark)))
        .find_map(|sentence| LOOKUP_WORDS.iter().find(|word| has_word(sentence, word)));
    lookup.map(|word| format!("the order says `{word}`"))
}

/// The sources line herdr puts in a research or media order's brief; `None` for any other class.
pub fn sources_brief_line(class: &str, needs: &[String], written: &str) -> Option<&'static str> {
    if !class.starts_with("research.") && !class.starts_with("media.") {
        return None;
    }
    Some(match web_asked(class, needs, written) {
        Some(_) => WEB_SOURCES,
        None => NO_WEB_SOURCES,
    })
}

/// What the gate reads of `web_asked` from the root order: the class again, then herdr's line. An order with
/// neither line (seated before the rule) asks for the web, as every research order did.
pub fn web_of(class: &str, spec: &Spec) -> Option<&'static str> {
    if class == WEB_CLASS {
        Some("the class is research.web")
    } else if spec.no_web {
        None
    } else {
        Some("herdr's brief asked it for its sources")
    }
}

/// The page reads of `stream_reads` (fetches, URL reads, browser calls), less those whose every URL is this machine
/// (`curl http://localhost:8770/api`): what a worker opened of the web (gsrc review S4).
fn page_reads(reads: &Reads) -> Vec<&str> {
    reads
        .texts
        .iter()
        .filter(|text| {
            let urls = extract_url_tokens(text);
            urls.is_empty() || urls.iter().any(|url| !local_host(url))
        })
        .map(String::as_str)
        .collect()
}

/// Plan 24 §5: gate axis for `research.*` and `media.*` classes.
/// Fails for each source URL that does not appear in the order's stream as a fetch, read or browser call. URLs are
/// compared after trimming a trailing `/` and the fragment. Citing nothing fails an order that asks for the web
/// (`web`, from `web_of`) and an order whose worker opened pages; for any other it is a note (live checks 2026-11-01,
/// F2, and review S4: what the worker did decides first). Every source cited is checked. With no folder to resolve a
/// relative path against: the tests' since plan 43, whose gate calls `sources_axis_at`.
#[cfg(test)]
pub fn sources_axis(
    class: &str,
    web: Option<&str>,
    report: Option<&Value>,
    stream: &str,
) -> Option<Axis> {
    sources_axis_at(class, web, report, stream, &[])
}

/// The class whose local entries the sources axis does not check (plan 43 A3): it reads the project, and Megajam's
/// research.read passed its first gate 0 times of 6 on local paths it had read.
pub const READ_CLASS: &str = "research.read";

/// `sources_axis` with the order's folders (`roots`: its worktree, then the project), against which a cited local
/// path relative to the repository is resolved (plan 43 A3). A local entry counts as read when a `Read`, `Grep`,
/// `Glob`, agy `read_file`/`view_file` or a shell read names it (`local_read`); a research.read order's local entries
/// are not checked at all. A web URL cited and never fetched still fails. A media order that cites nothing, opened no
/// page and read local files gets a note, whatever its brief said of the web: Megajam's r15-meadow-rescore-o and
/// r16-meadow3-rescore-o judged local stills, were refused "no sources", and were merged by hand.
pub fn sources_axis_at(
    class: &str,
    web: Option<&str>,
    report: Option<&Value>,
    stream: &str,
    roots: &[&Path],
) -> Option<Axis> {
    if !class.starts_with("research.") && !class.starts_with("media.") {
        return None;
    }
    let reads = stream_reads(stream);
    let opened = page_reads(&reads);
    // Fix round F4 (review finding 4): media judged, not any read. Every worker reads its own order file, so "any
    // local read" downgraded a media order that asked for the web, cited nothing and fetched nothing to a note.
    let judged_local = !reads.media_files.is_empty()
        || reads
            .local
            .iter()
            .any(|read| !read.search && media_file(&read.path));
    let none_cited = |why: &str| {
        match web {
        Some(asked) if class.starts_with("media.") && opened.is_empty() && judged_local => Axis::new(
            "sources",
            Mark::Note,
            format!(
                "no sources: {why}; the worker opened no web page and judged local files, so nothing is cited to \
check ({asked})"
            ),
        ),
        Some(asked) => Axis::new(
            "sources",
            Mark::Fail,
            format!("no sources: {why} ({asked})"),
        ),
        None if !opened.is_empty() => Axis::new(
            "sources",
            Mark::Fail,
            format!(
                "no sources: {why}, and the worker opened {} page(s): cite what it used",
                opened.len()
            ),
        )
        .with(
            opened
                .iter()
                .take(5)
                .map(|text| text.chars().take(200).collect())
                .collect(),
        ),
        None => Axis::new(
            "sources",
            Mark::Note,
            format!("no web asked; none cited and no page opened ({why})"),
        ),
    }
    };
    let Some(report) = report else {
        return Some(none_cited("no Work Report was delivered"));
    };
    let sources = match report.get("sources").and_then(Value::as_array) {
        Some(s) if !s.is_empty() => s,
        _ => return Some(none_cited("the Work Report names no sources")),
    };
    let mut missing = Vec::new();
    let mut skipped = 0usize;
    for item in sources {
        let Some(url) = item.get("url").and_then(Value::as_str) else {
            missing.push("item without url".to_owned());
            continue;
        };
        let norm = normalize_url(url);
        if stable_read(url, &reads) {
            continue;
        }
        if status_read(url, &reads) {
            continue;
        }
        if local_entry(url, roots) {
            if class == READ_CLASS {
                skipped += 1;
            } else if !local_read(url, &reads, roots) {
                missing.push(url.to_owned());
            }
            continue;
        }
        if class.starts_with("media.") && media_read(url, &reads) {
            continue;
        }
        if !url_in_stream(&norm, stream) {
            missing.push(url.to_owned());
        }
    }
    let unchecked = if skipped > 0 {
        format!("; {skipped} local entr(y/ies) not checked ({READ_CLASS} reads the project)")
    } else {
        String::new()
    };
    // Fix round F5: with every entry skipped, nothing was verified, and the axis must not say so.
    if missing.is_empty() && skipped == sources.len() {
        Some(Axis::new(
            "sources",
            Mark::Pass,
            format!(
                "{skipped} local entr(y/ies) cited and none checked ({READ_CLASS} reads the project); no web \
source cited"
            ),
        ))
    } else if missing.is_empty() {
        Some(Axis::new(
            "sources",
            Mark::Pass,
            format!(
                "{} source(s) cited, each verified in stream{unchecked}",
                sources.len() - skipped
            ),
        ))
    } else {
        Some(
            Axis::new(
                "sources",
                Mark::Fail,
                format!(
                    "{} source(s) cited that were not fetched, read or browsed{unchecked}",
                    missing.len()
                ),
            )
            .with(missing),
        )
    }
}

// ---- plan 37 §1.2: do the cited videos exist ----

/// What YouTube's oEmbed said of one video id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Video {
    Exists,
    /// 404: the only answer that fails (L4, "a 404 fails").
    Missing,
    /// Any other status: a note (an embedding-disabled or private video is not an invented one).
    Other(u16),
    /// A timeout or a network error: a note, and the calls left are not made, since the next would wait as long.
    Unreachable(String),
}

/// The line a missing video adds to the sources axis; `invented_videos` reads it back for the send-back.
pub const NO_SUCH_VIDEO: &str = "no such video on YouTube (oEmbed 404): ";

/// One oEmbed call. No redirect is followed: a 3xx is an answer of its own, a note.
pub fn oembed(base: &str, id: &str, timeout: Duration) -> Video {
    let agent = ureq::AgentBuilder::new()
        .timeout(timeout)
        .redirects(0)
        .build();
    let watch = format!("https://www.youtube.com/watch?v={id}");
    match agent
        .get(base)
        .query("url", &watch)
        .query("format", "json")
        .call()
    {
        Ok(response) if response.status() == 200 => Video::Exists,
        Ok(response) => Video::Other(response.status()),
        Err(ureq::Error::Status(404, _)) => Video::Missing,
        Err(ureq::Error::Status(status, _)) => Video::Other(status),
        Err(ureq::Error::Transport(transport)) => Video::Unreachable(transport.kind().to_string()),
    }
}

/// The video ids a Work Report cites, each once, in the order cited.
pub fn cited_videos(report: Option<&Value>) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    let sources = report
        .and_then(|report| report.get("sources"))
        .and_then(Value::as_array);
    for url in sources
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("url").and_then(Value::as_str))
    {
        if let Some(StableId::Video(id)) = stable_id(url) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// The sources axis with each cited video asked about, at most `cap` calls inside `budget` in all: a missing one
/// fails the axis naming its id; anything the check could not settle is the second value, a `videos` note, never a
/// fail. `ask` is given the time left, which bounds its own call.
pub fn check_videos(
    mut axis: Axis,
    report: Option<&Value>,
    ask: &mut dyn FnMut(&str, Duration) -> Video,
    cap: usize,
    budget: Duration,
) -> (Axis, Option<Axis>) {
    let started = Instant::now();
    let ids = cited_videos(report);
    let (mut found, mut missing, mut unsettled) = (0, Vec::new(), Vec::new());
    let mut unreachable = false;
    for (n, id) in ids.iter().enumerate() {
        if n >= cap {
            unsettled.push(format!(
                "{} cited video(s) over the cap of {cap} calls were not checked",
                ids.len() - cap
            ));
            break;
        }
        if unreachable {
            unsettled.push(format!("{id}: not checked, oEmbed was unreachable"));
            continue;
        }
        let left = budget.saturating_sub(started.elapsed());
        if left.is_zero() {
            unsettled.push(format!(
                "{id}: not checked, the gate's {} s for oEmbed were spent",
                budget.as_secs()
            ));
            continue;
        }
        match ask(id, left) {
            Video::Exists => found += 1,
            Video::Missing => missing.push(id.clone()),
            Video::Other(status) => unsettled.push(format!("{id}: oEmbed answered {status}")),
            Video::Unreachable(why) => {
                unreachable = true;
                unsettled.push(format!("{id}: oEmbed unreachable: {why}"));
            }
        }
    }
    if found > 0 {
        axis.why = format!("{}; {found} cited video(s) exist on YouTube", axis.why);
    }
    if !missing.is_empty() {
        let said = format!("{} cited video(s) do not exist on YouTube", missing.len());
        axis.why = if axis.mark == Mark::Fail {
            format!("{}; {said}", axis.why)
        } else {
            said
        };
        axis.mark = Mark::Fail;
        axis.lines
            .extend(missing.iter().map(|id| format!("{NO_SUCH_VIDEO}{id}")));
    }
    let note = (!unsettled.is_empty()).then(|| {
        Axis::new(
            "videos",
            Mark::Note,
            format!(
                "the existence of {} cited video(s) was not settled; that never fails",
                unsettled.len()
            ),
        )
        .with(unsettled)
    });
    (axis, note)
}

/// The ids a failing sources axis found missing: plan 37 §1.3's step-up reads them.
pub fn invented_videos(axes: &[Axis]) -> Vec<String> {
    failing(axes)
        .iter()
        .filter(|axis| axis.axis == "sources")
        .flat_map(|axis| &axis.lines)
        .filter_map(|line| line.strip_prefix(NO_SUCH_VIDEO))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in oEmbed server (127.0.0.1); with none set, a test build asks nothing.
    static TEST_OEMBED: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_oembed(base: Option<String>) {
    TEST_OEMBED.with_borrow_mut(|slot| *slot = base);
}

/// Where the gate asks. A test build never reaches the network: only a stand-in it was given.
fn oembed_base() -> Option<String> {
    #[cfg(test)]
    return TEST_OEMBED.with_borrow(Clone::clone);
    #[cfg(not(test))]
    Some(OEMBED_URL.to_owned())
}

/// The ports listened on in the tree under the pane's shell, from one process table and one listener table:
/// `nirvana_servers`'s own walk (feature 4), so a server counts here exactly when the sidebar shows it.
pub fn pane_ports(
    shell_pid: u32,
    processes: &[crate::nirvana_servers::Process],
    listeners: &[crate::nirvana_servers::Listener],
) -> Vec<u16> {
    crate::nirvana_servers::space_ports(&[shell_pid], processes, listeners)
}

// ---- held-out cases ----

/// One case of a check's output: its id and whether it passed. pytest `-rA` (`PASSED id`, `FAILED id - why`),
/// cargo test (`test id ... ok`), unittest `-v` (`name (id) ... ok`).
pub fn cases(output: &str) -> Vec<(String, bool)> {
    let mut found: Vec<(String, bool)> = Vec::new();
    let mut push = |id: &str, pass: bool| {
        let id = id.trim().to_owned();
        if id.is_empty() {
            return;
        }
        match found.iter_mut().find(|(known, _)| *known == id) {
            Some(entry) => entry.1 &= pass,
            None => found.push((id, pass)),
        }
    };
    for line in output.lines().map(str::trim) {
        let mut parts = line.split_whitespace();
        let first = parts.next().unwrap_or("");
        // pytest -v: `tests/test_x.py::test_a PASSED [ 50%]`.
        if first.contains("::") {
            match parts.clone().next() {
                Some("PASSED" | "XFAIL") => push(first, true),
                Some("FAILED" | "ERROR" | "XPASS") => push(first, false),
                _ => {}
            }
        }
        match first {
            "PASSED" | "XFAIL" => push(parts.next().unwrap_or(""), true),
            // unittest ends `FAILED (errors=2)`: a count, not a case (the scripted run of 2026-10-18 read it as a
            // fourth case of a three-case check).
            "FAILED" | "ERROR" | "XPASS" => {
                let id = parts.next().unwrap_or("");
                if !id.starts_with('(') {
                    push(id, false);
                }
            }
            "test" => {
                if let Some((id, result)) = line[4..].split_once(" ... ") {
                    match result.trim() {
                        "ok" => push(id, true),
                        "FAILED" => push(id, false),
                        _ => {}
                    }
                }
            }
            _ => {
                if let Some((head, result)) = line.split_once(" ... ") {
                    if let (Some(open), true) = (head.find(" ("), head.ends_with(')')) {
                        let id = &head[open + 2..head.len() - 1];
                        match result.trim() {
                            "ok" => push(id, true),
                            "FAIL" | "ERROR" => push(id, false),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    found
}

/// Decision 0019 §2.3: a check is valid at the base when at least one case ran and at least one fails there; a
/// check that passes at the base tests nothing.
pub fn heldout_valid(base: &[(String, bool)]) -> Result<(), String> {
    if base.is_empty() {
        return Err("no case of the check ran at the base".into());
    }
    if base.iter().all(|(_, pass)| *pass) {
        return Err(format!(
            "all {} case(s) pass at the base: the check tests nothing",
            base.len()
        ));
    }
    Ok(())
}

/// The held-out axis at the order's head. The ids are the only thing the builder will see (§7.3: "the builder sees
/// only the failing test ids"), so the axis holds them and nothing of the check's output.
pub fn heldout_axis(run: &CommandRun) -> Axis {
    let found = cases(&run.output);
    let failing: Vec<String> = found
        .iter()
        .filter(|(_, pass)| !pass)
        .map(|(id, _)| id.clone())
        .collect();
    if found.is_empty() {
        return Axis::new("heldout", Mark::Fail, "the held-out check printed no case");
    }
    match (failing.is_empty(), run.exit) {
        (true, Some(0)) => Axis::new(
            "heldout",
            Mark::Pass,
            format!("{} held-out case(s) pass", found.len()),
        ),
        (true, exit) => Axis::new(
            "heldout",
            Mark::Fail,
            format!(
                "the held-out check exited {exit:?} with its {} case(s) passing",
                found.len()
            ),
        ),
        (false, _) => Axis::new(
            "heldout",
            Mark::Fail,
            format!("{} held-out case(s) fail", failing.len()),
        )
        .with(failing),
    }
}

// ---- running a command without a shell ----

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRun {
    pub exit: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    /// It did not start.
    pub error: Option<String>,
}

/// Words of a command line, with `"` and `'` quoting. No shell: Git Bash rewrote `/usage` into a path twice on
/// 2026-10-17 (plan 12 §3), and a gate line is the orchestrator's text.
pub fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            None => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

/// Runs `line` in `cwd`, stdout and stderr together, stopped after `timeout`. On Windows a program that is a
/// `.cmd` (npm) is not found by name, so a not-found start is retried through `cmd /C`.
pub fn run_command(cwd: &Path, line: &str, timeout: Duration) -> CommandRun {
    let argv = split_command(line);
    let Some((program, args)) = argv.split_first() else {
        return CommandRun {
            error: Some("an empty command".into()),
            ..CommandRun::default()
        };
    };
    // Plan 40 review B2: the test runs in a job object, so a timeout ends its whole tree. `child.kill()` ended only
    // `cmd` or `cargo`; a hung test binary kept the pipes open, the readers never reached EOF, and the gate held the
    // project's gate lock with them.
    let spawn = |program: &str, args: &[String]| {
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        crate::nirvana_worker::job::spawn(&mut command)
    };
    let child = match spawn(program, args) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && cfg!(windows) => {
            let mut through = vec!["/C".to_owned()];
            through.extend(argv.iter().cloned());
            spawn("cmd", &through)
        }
        other => other,
    };
    let (job, mut child) = match child {
        Ok(started) => started,
        Err(err) => {
            return CommandRun {
                error: Some(err.to_string()),
                ..CommandRun::default()
            }
        }
    };
    let pipe = |reader: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut reader) = reader {
                let _ = reader.read_to_end(&mut text);
            }
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let out = pipe(
        child
            .stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let err = pipe(
        child
            .stderr
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() > timeout => {
                timed_out = true;
                job.kill();
                break child.wait().ok();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break None,
        }
    };
    // The job closes here, which ends anything of the test's still running (kill on close). The readers then reach
    // EOF; a deadline keeps a pipe some process outside the job still holds from holding the gate.
    drop(job);
    let read_for = Duration::from_secs(if timed_out { 5 } else { 30 });
    let output = crate::nirvana_worker::job::join_within(out, read_for);
    let stderr = crate::nirvana_worker::job::join_within(err, read_for);
    let unread = output.is_none() || stderr.is_none();
    let mut output = output.unwrap_or_default();
    let stderr = stderr.unwrap_or_default();
    if unread {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(UNREAD_OUTPUT);
        output.push('\n');
    }
    if !stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&stderr);
    }
    CommandRun {
        exit: status.and_then(|status| status.code()),
        output,
        timed_out,
        error: None,
    }
}

// ---- git ----

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn rev(cwd: &Path, name: &str) -> Option<String> {
    git(
        cwd,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{commit}}"),
        ],
    )
    .ok()
    .map(|sha| sha.trim().to_owned())
    .filter(|sha| !sha.is_empty())
}

pub fn integration_branch(run: &str) -> String {
    format!("{INTEGRATION_PREFIX}{run}")
}

/// The run's integration branch, made from the project's HEAD the first time an order of the run is gated.
pub fn ensure_integration(root: &Path, run: &str) -> Result<String, String> {
    let branch = integration_branch(run);
    if let Some(sha) = rev(root, &format!("refs/heads/{branch}")) {
        return Ok(sha);
    }
    let head = rev(root, "HEAD")
        .ok_or("the project has no commit to start the integration branch from")?;
    git(root, &["branch", &branch, &head])?;
    Ok(head)
}

/// `-c user.name=... -c user.email=...` when the repository has no identity: a gate commit must not fail on a
/// machine where git was never told who commits.
fn identity(cwd: &Path) -> Vec<String> {
    let set = |key: &str| git(cwd, &["config", key]).is_ok_and(|value| !value.trim().is_empty());
    if set("user.email") && set("user.name") {
        Vec::new()
    } else {
        [
            "-c",
            "user.name=herdr gate",
            "-c",
            "user.email=herdr-gate@localhost",
        ]
        .map(str::to_owned)
        .to_vec()
    }
}

/// Every path, outside herdr's own folder, with a change the worker has not committed. `-z` gives each path as it is:
/// the quoted form git prints for a non-ASCII name matched no file, so the owned axis dropped it (P35a review S3).
fn uncommitted(cwd: &Path) -> Result<Vec<String>, String> {
    let status = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "-uall",
            "--",
            ".",
            ":(exclude).nirvana",
        ],
    )?;
    let mut paths = Vec::new();
    let mut entries = status.split('\0');
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        // A rename or copy is `XY <to>`, then its source as an entry of its own.
        if matches!(entry.as_bytes()[0], b'R' | b'C') {
            entries.next();
        }
        if !harness_path(&entry[3..]) {
            paths.push(entry[3..].to_owned());
        }
    }
    Ok(paths)
}

/// A file's size and the later of its creation and modification times, or `None` when it is gone. The creation time
/// counts too: yt-dlp sets a download's modification time to the video's upload date, and `curl -R` and `cp -p` keep
/// the source's (P35a review S3).
fn fingerprint(path: &Path) -> Option<(u64, u128)> {
    let meta = std::fs::metadata(path)
        .or_else(|_| std::fs::symlink_metadata(path))
        .ok()?;
    let nanos = |time: std::io::Result<std::time::SystemTime>| {
        time.ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos())
    };
    Some((
        meta.len(),
        nanos(meta.modified()).max(nanos(meta.created())),
    ))
}

fn seat_snapshot_path(project: &Project, run: &str, order: &str) -> PathBuf {
    gate_dir(project, run).join(format!("{order}.seat-dirty.json"))
}

/// Plan 35 P35a, MotoProject round: the project folder's uncommitted files when an order is seated there, each with
/// its size and time, so the owned axis counts only what the order changed. reel-motion-gemini-r2 ran in the project
/// folder and was sent back for "3958 file(s) changed that the order does not own": the whole dirty tree, none of
/// it its own. `seat run` calls this for an order with no worktree, before the worker starts.
pub fn write_seat_snapshot(
    project: &Project,
    run: &str,
    order: &str,
    cwd: &Path,
) -> Result<usize, String> {
    let dirty = uncommitted(cwd)?;
    let snapshot: serde_json::Map<String, Value> = dirty
        .iter()
        .map(|path| {
            let print = fingerprint(&cwd.join(path)).map_or(Value::Null, |(len, nanos)| {
                serde_json::json!([len, nanos.to_string()])
            });
            (path.clone(), print)
        })
        .collect();
    let file = seat_snapshot_path(project, run, order);
    std::fs::create_dir_all(gate_dir(project, run)).map_err(|err| err.to_string())?;
    std::fs::write(&file, Value::Object(snapshot).to_string())
        .map_err(|err| format!("{}: {err}", file.display()))?;
    Ok(dirty.len())
}

/// Said in the owned axis when the gate had no seat snapshot: until `seat run` writes one, every project-folder
/// verdict says so, so a missing snapshot cannot read as a pass (CLAUDE.md rule 8).
pub const NO_SNAPSHOT: &str =
    "no seat snapshot: counted the files created, changed or deleted since seating";

/// The uncommitted paths an order in the project folder changed, against its chain root's seat snapshot: a path
/// dirty at seating counts only when its size or time moved since. With no snapshot (an order seated before plan 35),
/// a path counts when it was created or modified at or after the root was seated, or is gone: a deletion counts, as
/// on master, since the gate cannot tell when it happened (review S3). With neither a snapshot nor a seating in the
/// journal, the whole dirty tree. The note is `NO_SNAPSHOT` when the times decided.
fn order_changes(subject: &Subject) -> Result<(Vec<String>, Option<&'static str>), String> {
    let dirty = uncommitted(subject.cwd)?;
    let project = Project::at(subject.root);
    let state = replay(&project.journal(subject.run).read().0);
    let (root, _) = chain(&state, subject.order);
    let snapshot = std::fs::read_to_string(seat_snapshot_path(&project, subject.run, &root))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Map<String, Value>>(&text).ok());
    if let Some(snapshot) = snapshot {
        let changed = dirty
            .into_iter()
            .filter(|path| {
                let now = fingerprint(&subject.cwd.join(path))
                    .map_or(Value::Null, |(len, nanos)| {
                        serde_json::json!([len, nanos.to_string()])
                    });
                snapshot.get(path) != Some(&now)
            })
            .collect();
        return Ok((changed, None));
    }
    let Some(seated_at) = state.order(&root).map(|known| known.seated_at) else {
        return Ok((dirty, None));
    };
    let changed = dirty
        .into_iter()
        .filter(|path| {
            fingerprint(&subject.cwd.join(path))
                .is_none_or(|(_, nanos)| nanos / 1_000_000_000 >= u128::from(seated_at))
        })
        .collect();
    Ok((changed, Some(NO_SNAPSHOT)))
}

/// Commits the worker's work on its own branch, herdr's folder left out: every change to a tracked file, and each
/// new file an `owns:` pattern names. A new file no pattern names stays out of the commit and is returned: the live
/// run of 2026-10-18 committed the worker's `__pycache__/*.pyc`, which its own test run had made in a project with
/// no `.gitignore`, and then failed the order on the owned axis for the gate's own commit. With no `owns:` line every
/// new file is the worker's, as before.
pub fn commit_work(cwd: &Path, order: &str, owns: &[String]) -> Result<Vec<String>, String> {
    let status = git(
        cwd,
        &[
            "status",
            "-z",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).nirvana",
        ],
    )?;
    // Plan 40 review B5: `-z`, parsed as `uncommitted()` parses it. Without it git quotes a non-ASCII name
    // (`"caf\303\251.md"`), `git add` of the quoted spelling matched nothing, and the gate failed the order on
    // "the worker's work could not be committed".
    let (mut tracked, mut new, mut left_out) = (false, Vec::new(), Vec::new());
    let mut entries = status.split('\0');
    while let Some(line) = entries.next() {
        if line.len() < 4 {
            continue;
        }
        // A rename or copy is `XY <to>`, then its source as an entry of its own.
        if matches!(line.as_bytes()[0], b'R' | b'C') {
            entries.next();
        }
        let path = line[3..].to_owned();
        if !line.starts_with("??") {
            tracked = true;
        } else if owns.is_empty() || owns.iter().any(|pattern| owns_match(pattern, &path)) {
            new.push(path);
        } else {
            left_out.push(path);
        }
    }
    if !tracked && new.is_empty() {
        return Ok(left_out);
    }
    if tracked {
        git(cwd, &["add", "-u", "--", ".", ":(exclude).nirvana"])?;
    }
    for chunk in new.chunks(64) {
        let mut args = vec!["add", "--"];
        args.extend(chunk.iter().map(String::as_str));
        git(cwd, &args)?;
    }
    let mut args = identity(cwd);
    args.extend(
        [
            "commit",
            "-q",
            "-m",
            &format!("herdr gate: the work of {order}\n\nNirvana-Order: {order}"),
        ]
        .map(str::to_owned),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git(cwd, &args)?;
    Ok(left_out)
}

/// Why a rebase failed: the conflicting files, or, when git stopped before any conflict, its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseFail {
    pub files: Vec<String>,
    pub why: String,
}

/// Rebases the worktree's branch onto `onto`. A conflict aborts the rebase and names the files.
pub fn rebase(cwd: &Path, onto: &str) -> Result<(), RebaseFail> {
    let mut args = identity(cwd);
    args.extend(["rebase", "-q", onto].map(str::to_owned));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match git(cwd, &args) {
        Ok(_) => Ok(()),
        Err(why) => {
            let files: Vec<String> = git(cwd, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect();
            let _ = git(cwd, &["rebase", "--abort"]);
            Err(RebaseFail { files, why })
        }
    }
}

/// The paths a git refusal lists, one per tab-indented line ("would be overwritten by checkout:\n\t.nirvana/x").
fn listed_paths(why: &str) -> Vec<String> {
    why.lines()
        .filter(|line| line.starts_with('\t'))
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Plan 43 A1: a failed rebase is the gate's own fault when only herdr's files stood in its way: its conflicting or
/// listed paths are all under `.nirvana/`, or git refused the worker's tree as dirty when every change outside
/// `.nirvana/` is committed (the gate's commit leaves herdr's folder out). Megajam: c0-skeleton and c1-rules ("cannot
/// rebase: You have unstaged changes"), l04-build-s2-r1 (".nirvana/lessons/index.json ... would be overwritten").
pub fn rebase_fault(cwd: &Path, fail: &RebaseFail) -> Fault {
    let listed = if fail.files.is_empty() {
        listed_paths(&fail.why)
    } else {
        fail.files.clone()
    };
    if !listed.is_empty() {
        return if listed.iter().all(|path| harness_path(path)) {
            Fault::Gate
        } else {
            Fault::Worker
        };
    }
    let dirty_refusal =
        fail.why.contains("unstaged changes") || fail.why.contains("uncommitted changes");
    if dirty_refusal && uncommitted(cwd).is_ok_and(|paths| paths.is_empty()) {
        Fault::Gate
    } else {
        Fault::Worker
    }
}

/// Plan 43 A1, the root of the three rebases above: herdr's own files under `.nirvana/` that would stop a rebase onto
/// `onto`, set aside under `aside` first. They are every tracked one with a change (git refuses a dirty tree, and the
/// gate's commit leaves `.nirvana/` out) and every untracked one `onto` tracks (git will not overwrite it; the project
/// tracks `.nirvana/lessons/`, and a worktree holds offered copies). Returns the paths set aside.
///
/// Fix round G1 (review (b)): the first cut could leave the worktree changed. A `set_aside_harness` that failed
/// midway returned before any restore; a tracked file with edits came back as `HEAD` had it; a death between set-aside
/// and restore left it so for good; and the aside copy was never deleted (one folder per gated order, on a disk 99%
/// full). Now every file is copied, and `aside/manifest.json` written, before anything in the worktree changes; a
/// failure after that restores from the manifest before it returns; `restore_harness` puts each file back as it was,
/// and deletes the copy; and the next gate of the order restores from a manifest a dead gate left before it does
/// anything else (`deterministic`).
pub fn set_aside_harness(cwd: &Path, onto: &str, aside: &Path) -> Result<Vec<String>, String> {
    let status = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "-uall",
            "--",
            nirvana_run::DIR,
        ],
    )?;
    let mut tracked = Vec::new();
    let mut untracked = Vec::new();
    let mut entries = status.split('\0');
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        if matches!(entry.as_bytes()[0], b'R' | b'C') {
            entries.next();
        }
        let path = entry[3..].to_owned();
        if !harness_path(&path) {
            continue;
        }
        if entry.starts_with("??") {
            untracked.push(path);
        } else {
            tracked.push(path);
        }
    }
    if !untracked.is_empty() {
        let theirs: HashSet<String> = git(
            cwd,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                "-z",
                onto,
                "--",
                nirvana_run::DIR,
            ],
        )?
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();
        untracked.retain(|path| theirs.contains(path));
    }
    let moved: Vec<String> = tracked.iter().chain(&untracked).cloned().collect();
    if moved.is_empty() {
        return Ok(moved);
    }
    // Every copy and the manifest first: nothing in the worktree has changed if this fails.
    let mut manifest = AsideManifest::default();
    // Fix round 2 (H5 d): the copies this call wrote, removed again when a copy or the manifest fails, so a full disk
    // leaves no half set-aside behind (the review found them kept until a later gate succeeded).
    let mut written: Vec<PathBuf> = Vec::new();
    let copied_all = (|| -> Result<(), String> {
        for (path, was_tracked) in tracked
            .iter()
            .map(|path| (path, true))
            .chain(untracked.iter().map(|path| (path, false)))
        {
            let from = cwd.join(path);
            let copied = from.is_file();
            if copied {
                #[cfg(test)]
                if !written.is_empty() && TEST_ASIDE_COPY_FAILS.replace(false) {
                    return Err("a test's copy failure".into());
                }
                let to = aside.join(path);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                std::fs::copy(&from, &to).map_err(|err| format!("{}: {err}", from.display()))?;
                written.push(to);
            }
            manifest.entries.push(AsideEntry {
                path: path.clone(),
                tracked: was_tracked,
                copied,
            });
        }
        std::fs::create_dir_all(aside).map_err(|err| err.to_string())?;
        let text = serde_json::to_string_pretty(&manifest).map_err(|err| err.to_string())?;
        nirvana_seat::write_atomic(&aside.join(ASIDE_MANIFEST), text.as_bytes())
            .map_err(|err| err.to_string())
    })();
    if let Err(why) = copied_all {
        for copy in &written {
            let _ = std::fs::remove_file(copy);
        }
        let _ = std::fs::remove_file(aside.join(ASIDE_MANIFEST));
        // Only folders left empty: copies an earlier restore kept on purpose stay.
        let _ = remove_empty_dirs(aside);
        return Err(why);
    }
    let cleared = (|| -> Result<(), String> {
        for chunk in tracked.chunks(64) {
            let mut args = vec!["reset", "-q", "--"];
            args.extend(chunk.iter().map(String::as_str));
            git(cwd, &args)?;
            let in_head: Vec<&str> = chunk
                .iter()
                .map(String::as_str)
                .filter(|path| git(cwd, &["cat-file", "-e", &format!("HEAD:{path}")]).is_ok())
                .collect();
            if !in_head.is_empty() {
                let mut args = vec!["checkout", "-q", "--"];
                args.extend(in_head.iter().copied());
                git(cwd, &args)?;
            }
            for path in chunk
                .iter()
                .filter(|path| !in_head.contains(&path.as_str()))
            {
                let _ = std::fs::remove_file(cwd.join(path));
            }
        }
        #[cfg(test)]
        if TEST_ASIDE_FAILS.replace(false) {
            return Err("a test's set-aside failure, midway".into());
        }
        for path in &untracked {
            let _ = std::fs::remove_file(cwd.join(path));
        }
        Ok(())
    })();
    if let Err(why) = cleared {
        let restored = restore_harness(cwd, aside)
            .map(|_| "each file was put back".to_owned())
            .unwrap_or_else(|err| format!("and the restore failed too: {err}"));
        return Err(format!("{why}; {restored}"));
    }
    Ok(moved)
}

/// Removes `dir` and the folders under it that hold no file, deepest first; a folder with a file stays.
fn remove_empty_dirs(dir: &Path) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)?.flatten() {
        if entry.path().is_dir() {
            let _ = remove_empty_dirs(&entry.path());
        }
    }
    std::fs::remove_dir(dir)
}

#[cfg(test)]
thread_local! {
    /// Fix round G1's hook: the next set-aside on this thread fails once, after it reset the tracked files.
    pub(crate) static TEST_ASIDE_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Fix round 2 (H5 d): the next set-aside's second copy fails once, the first written.
    pub(crate) static TEST_ASIDE_COPY_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Fix round 2 (H5 c): the next restore's removal of its aside folder fails once (a Windows lock).
    pub(crate) static TEST_ASIDE_REMOVE_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Fix round 2 (H8): the next `gate_amend` on this thread errs once, after its amend is journaled.
    pub(crate) static TEST_REGATE_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The folder a worktree's set-aside lives in, under the run's gate folder. Fix round 2 (H5 a): keyed by the worktree,
/// not the order: a send-back child shares its parent's worktree, and the parent's leftover belongs to that worktree;
/// two gates of one worktree are one at a time (`gate_lock`).
pub fn aside_dir(project: &Project, run: &str, worktree: &Path) -> PathBuf {
    let name = worktree
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect();
    gate_dir(project, run).join(format!("{name}-aside"))
}

/// The record of a set-aside, `<aside>/manifest.json`: written before the worktree changes, deleted once each file is
/// back. A gate that finds one restores from it first (fix round G1).
pub const ASIDE_MANIFEST: &str = "manifest.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsideManifest {
    pub entries: Vec<AsideEntry>,
}

/// One file set aside: tracked or untracked before the rebase, and whether it existed (a tracked file the worker
/// deleted has no copy, and is deleted again).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsideEntry {
    pub path: String,
    pub tracked: bool,
    pub copied: bool,
}

/// Puts back every file `set_aside_harness` moved, from its manifest, and deletes the aside copies. A tracked file
/// comes back as it was, the worker's edits and all, over whatever the rebase left (it is herdr's own state, which the
/// gate's commit leaves out); one the worker had deleted is deleted again. An untracked one comes back where the
/// rebase left it absent; where the rebase brought a tracked copy of it, that copy stays, and the old one, when it
/// differs, is kept in `aside` and named in the result. With no manifest, nothing to do. An error leaves the manifest,
/// so the next gate tries again.
///
/// Fix round 2 (H5): an entry whose aside copy is gone is skipped and named (`Restored::missing`), not an error that
/// failed every later gate of the order; and the manifest goes before the folder, so a folder Windows will not let go
/// of (`Restored::left`) never makes the next gate restore again over newer edits.
pub fn restore_harness(cwd: &Path, aside: &Path) -> Result<Restored, String> {
    restore_from(cwd, aside, false)
}

/// Fix round 2 (H5 a): what a dead gate left, restored by the next gate of the worktree (`deterministic`). That gate
/// may be a send-back child's, which shares the worktree and may have changed the file since: a tracked file the
/// dead gate left as `HEAD` has it (or absent) is put back, but one whose content differs from `HEAD`'s and whose
/// modification time is after the manifest's was changed since by someone else, and the live file stays
/// (`Restored::live`). The time alone is not enough: the dead gate's own `git checkout` wrote every file it set aside
/// after its manifest.
pub fn restore_leftover(cwd: &Path, aside: &Path) -> Result<Restored, String> {
    restore_from(cwd, aside, true)
}

/// What a restore did beside putting files back. Each list holds paths under `.nirvana/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Restored {
    /// Untracked files the rebase brought a different tracked copy of; the old copy stays in the aside folder.
    pub kept: Vec<String>,
    /// Entries whose aside copy was gone: skipped.
    pub missing: Vec<String>,
    /// Leftover entries whose live file is newer than the manifest and not as the dead gate left it: kept live.
    pub live: Vec<String>,
    /// The aside folder could not be removed after the manifest was (a Windows lock): why.
    pub left: Option<String>,
}

impl Restored {
    /// The note a gate adds for anything beside a plain put-back, or None.
    pub fn note(&self, aside: &Path) -> Option<String> {
        let mut parts = Vec::new();
        if !self.missing.is_empty() {
            parts.push(format!(
                "{} set-aside file(s) had no copy left and were skipped: {}",
                self.missing.len(),
                self.missing.join(", ")
            ));
        }
        if !self.live.is_empty() {
            parts.push(format!(
                "{} file(s) changed after the set-aside were kept as they are now, not restored: {}",
                self.live.len(),
                self.live.join(", ")
            ));
        }
        if let Some(why) = &self.left {
            parts.push(format!(
                "the aside folder {} could not be removed ({why}); its manifest is gone, so nothing restores from it",
                aside.display()
            ));
        }
        (!parts.is_empty()).then(|| parts.join("; "))
    }
}

fn restore_from(cwd: &Path, aside: &Path, leftover: bool) -> Result<Restored, String> {
    let manifest_path = aside.join(ASIDE_MANIFEST);
    let Ok(text) = std::fs::read_to_string(&manifest_path) else {
        return Ok(Restored::default());
    };
    let manifest: AsideManifest = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let written_at = std::fs::metadata(&manifest_path)
        .and_then(|meta| meta.modified())
        .ok();
    // A leftover's tracked file someone changed after the dead gate: newer than the manifest, and not what the gate's
    // checkout (or its rebase) left, which is `HEAD`'s blob or no file.
    let changed_since = |path: &str, to: &Path| -> bool {
        if !leftover || !to.is_file() {
            return false;
        }
        let newer = match (written_at, std::fs::metadata(to).and_then(|meta| meta.modified())) {
            (Some(manifest), Ok(live)) => live > manifest,
            _ => false,
        };
        if !newer {
            return false;
        }
        let normal = |text: &str| text.replace("\r\n", "\n");
        let live = std::fs::read(to)
            .map(|bytes| normal(&String::from_utf8_lossy(&bytes)))
            .unwrap_or_default();
        match git(cwd, &["show", &format!("HEAD:{path}")]) {
            Ok(head) => normal(&head) != live,
            Err(_) => true,
        }
    };
    let mut done = Restored::default();
    for entry in &manifest.entries {
        let to = cwd.join(&entry.path);
        let from = aside.join(&entry.path);
        if entry.tracked && changed_since(&entry.path, &to) {
            done.live.push(entry.path.clone());
            continue;
        }
        if !entry.copied {
            if entry.tracked && to.exists() {
                std::fs::remove_file(&to).map_err(|err| format!("{}: {err}", to.display()))?;
            }
            continue;
        }
        if !from.is_file() {
            done.missing.push(entry.path.clone());
            continue;
        }
        let put_back = entry.tracked || !to.exists();
        if put_back {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            std::fs::copy(&from, &to).map_err(|err| format!("{}: {err}", to.display()))?;
        } else if std::fs::read(&to).ok() != std::fs::read(&from).ok() {
            done.kept.push(entry.path.clone());
        }
    }
    // Fix round 2 (H5 c): the manifest first. Kept with the folder, a lock on the folder left it, and the next gate
    // restored it again over the edits made since.
    if let Err(err) = std::fs::remove_file(&manifest_path) {
        // Marked instead: a manifest by another name restores nothing.
        std::fs::rename(&manifest_path, aside.join(format!("{ASIDE_MANIFEST}.done")))
            .map_err(|rename| format!("{}: {err}; {rename}", manifest_path.display()))?;
    }
    if done.kept.is_empty() {
        #[cfg(test)]
        if TEST_ASIDE_REMOVE_FAILS.replace(false) {
            done.left = Some("a test's lock".into());
            return Ok(done);
        }
        if let Err(err) = std::fs::remove_dir_all(aside) {
            done.left = Some(err.to_string());
        }
    } else {
        for entry in manifest
            .entries
            .iter()
            .filter(|entry| !done.kept.contains(&entry.path))
        {
            let _ = std::fs::remove_file(aside.join(&entry.path));
        }
        let _ = std::fs::remove_file(aside.join(format!("{ASIDE_MANIFEST}.done")));
    }
    Ok(done)
}

/// Fix round G1: the restore runs on every way out of the rebase, a panic among them.
struct AsideGuard<'a> {
    cwd: &'a Path,
    aside: &'a Path,
    armed: bool,
}

impl AsideGuard<'_> {
    fn restore(mut self) -> Result<Restored, String> {
        self.armed = false;
        restore_harness(self.cwd, self.aside)
    }
}

impl Drop for AsideGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = restore_harness(self.cwd, self.aside);
        }
    }
}

/// The paths the order's commits change against `base`.
pub fn changed_files(cwd: &Path, base: &str) -> Result<Vec<String>, String> {
    Ok(
        git(cwd, &["diff", "--name-only", &format!("{base}...HEAD")])?
            .lines()
            .map(str::to_owned)
            .filter(|path| !path.is_empty())
            .collect(),
    )
}

/// Where `nirvana_worker::add_worktree` records the commit a worktree began at: beside the worktree, never in it.
pub fn start_file(worktree: &Path) -> PathBuf {
    let name = worktree
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    worktree.with_file_name(format!("{name}.start"))
}

fn changed_between(cwd: &Path, from: &str, to: &str) -> Result<HashSet<String>, String> {
    Ok(
        git(cwd, &["diff", "--name-only", "--no-renames", from, to])?
            .lines()
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// Of `changed` (against `base`, after the rebase onto it), the paths that came with the commit `start` the worktree
/// began at and that the worker left as they were. 0.71.1: am1-assetmap's worktree began at the owner's commit
/// 7dbefa0 on main, a minute after it; the integration branch lacked it, so the rebase carried it as a copy and the
/// owned axis sent the order back for its 35 files.
///
/// A changed path is the owner's only when both hold, so no edit of the worker's is ever dropped:
/// - HEAD has it exactly as `start` had it: the worker's net change to it is nothing.
/// - `base` has not changed it since `fork`, where `start` and `base` meet: the branch's version is not a revert of
///   another order's merged work.
///
/// HEAD differs from `base` there, so `start` differs from `fork`: the change came with the worktree. `start` is
/// herdr's record, written when the worktree was made, and must name a commit.
pub fn begun_with(
    cwd: &Path,
    base: &str,
    start: &str,
    changed: &[String],
) -> Result<Vec<String>, String> {
    let start = git(
        cwd,
        &["rev-parse", "--verify", &format!("{start}^{{commit}}")],
    )?
    .trim()
    .to_owned();
    // Review of 0.71.1, finding 1: the record sits beside the worktree, where a worker's shell can write it. A worker
    // that commits an unowned edit and writes its own HEAD there would empty `worker` below and pass every path. The
    // commit a worktree began at is on the branch it was made from (the owner's main); a worker's own commit is only
    // on its order's branch. So the record counts only when a branch outside `nirvana/` holds it; else nothing is
    // left out and the gate counts as before 0.71.1.
    let holders = git(
        cwd,
        &[
            "for-each-ref",
            "--contains",
            &start,
            "--format=%(refname)",
            "refs/heads/",
        ],
    )?;
    if !holders
        .lines()
        .any(|name| !name.is_empty() && !name.starts_with("refs/heads/nirvana/"))
    {
        return Ok(Vec::new());
    }
    let fork = git(cwd, &["merge-base", &start, base])?.trim().to_owned();
    let integrated = changed_between(cwd, &fork, base)?;
    let worker = changed_between(cwd, &start, "HEAD")?;
    Ok(changed
        .iter()
        .filter(|path| !worker.contains(*path) && !integrated.contains(*path))
        .cloned()
        .collect())
}

/// `changed` without the paths `begun_with` gives, and a note saying how many were left out. With no start record
/// (a worktree made before 0.71.1) or a git error, every path is kept, as before.
fn without_begun_with(
    cwd: &Path,
    base: &str,
    changed: Vec<String>,
) -> (Vec<String>, Option<String>) {
    let Some(start) = std::fs::read_to_string(start_file(cwd))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
    else {
        return (changed, None);
    };
    match begun_with(cwd, base, &start, &changed) {
        Ok(theirs) if !theirs.is_empty() => {
            let note = format!(
                "{} file(s) the worktree began with at {} and the worker left alone are not counted",
                theirs.len(),
                short_sha(&start)
            );
            let kept = changed
                .into_iter()
                .filter(|path| !theirs.contains(path))
                .collect();
            (kept, Some(note))
        }
        _ => (changed, None),
    }
}

/// Merges the gated head into the integration branch with no checkout: the order's branch is rebased onto the
/// branch's tip, so the merge's tree is the head's tree. `update-ref` takes the old value, so a branch that moved
/// since the gate is refused, not overwritten. The served checkout is never touched (memory
/// served-checkout-is-a-shared-writer: a landing step ran `reset` in the served `main` on 2026-09-20).
pub fn merge(
    root: &Path,
    run: &str,
    order: &str,
    base: &str,
    head: &str,
) -> Result<String, String> {
    let branch = integration_branch(run);
    let tip = rev(root, &format!("refs/heads/{branch}")).ok_or(format!("no branch {branch}"))?;
    if tip != base {
        return Err(format!(
            "{branch} moved since the gate ({} then, {} now): gate {order} again",
            short_sha(base),
            short_sha(&tip)
        ));
    }
    let mut args = identity(root);
    args.extend(
        [
            "commit-tree",
            &format!("{head}^{{tree}}"),
            "-p",
            base,
            "-p",
            head,
            "-m",
            &format!("herdr gate merge: {order}\n\nNirvana-Order: {order}\nNirvana-Run: {run}"),
        ]
        .map(str::to_owned),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let sha = git(root, &args)?.trim().to_owned();
    git(
        root,
        &["update-ref", &format!("refs/heads/{branch}"), &sha, base],
    )?;
    Ok(sha)
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

// ---- the deterministic half ----

/// What the five axes saw: the integration tip the branch was rebased onto, and the head they ran at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Det {
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    pub axes: Vec<Axis>,
}

/// The order as the five axes need it.
pub struct Subject<'a> {
    pub root: &'a Path,
    pub cwd: &'a Path,
    pub run: &'a str,
    pub order: &'a str,
    /// The order's branch, when it ran in a worktree.
    pub branch: Option<&'a str>,
    pub class: &'a str,
    pub report: Option<&'a Value>,
    pub spec: &'a Spec,
    /// The ports listened on in the worker's pane, read only for an ops order.
    pub ports: &'a dyn Fn() -> Result<Vec<u16>, String>,
}

/// Axes 1 to 5. The test output goes to `log` whole; the axes keep it filtered.
pub fn deterministic(subject: &Subject, log: Option<&Path>) -> Det {
    let mut axes = Vec::new();
    let mut det = Det::default();
    let (base, worktree) = match subject.branch {
        Some(_) => {
            // Fix round G1: a gate that died between set-aside and restore left a manifest; its files go back first.
            let aside = aside_dir(&Project::at(subject.root), subject.run, subject.cwd);
            if aside.join(ASIDE_MANIFEST).is_file() {
                match restore_leftover(subject.cwd, &aside) {
                    Ok(restored) => axes.push(Axis::new(
                        "aside",
                        Mark::Note,
                        format!(
                            "an earlier gate of this worktree stopped between setting herdr's own files aside and \
putting them back; they were put back from {} first{}",
                            aside.display(),
                            restored
                                .note(&aside)
                                .map(|note| format!("; {note}"))
                                .unwrap_or_default()
                        ),
                    )),
                    Err(why) => {
                        axes.push(
                            Axis::new(
                                "rebase",
                                Mark::Fail,
                                format!(
                                    "herdr's own files an earlier gate set aside could not be put back from {}: {why}",
                                    aside.display()
                                ),
                            )
                            .fault(Fault::Gate),
                        );
                        det.axes = axes;
                        return det;
                    }
                }
            }
            let base = match ensure_integration(subject.root, subject.run) {
                Ok(base) => base,
                Err(why) => {
                    axes.push(Axis::new("rebase", Mark::Fail, why).fault(Fault::Gate));
                    det.axes = axes;
                    return det;
                }
            };
            match commit_work(subject.cwd, subject.order, &subject.spec.owns) {
                Ok(left_out) if !left_out.is_empty() => axes.push(
                    Axis::new(
                        "commit",
                        Mark::Note,
                        format!(
                            "{} new file(s) that no `owns:` pattern names were left out of the commit",
                            left_out.len()
                        ),
                    )
                    // Plan 40 T1: a file list, capped like the owned axis's.
                    .with(capped(left_out)),
                ),
                Ok(_) => {}
                Err(why) => {
                    // Plan 43 A1: herdr's commit, which no worker can make for it.
                    axes.push(
                        Axis::new(
                            "rebase",
                            Mark::Fail,
                            format!("the worker's work could not be committed: {why}"),
                        )
                        .fault(Fault::Gate),
                    );
                    det.axes = axes;
                    return det;
                }
            }
            let integration = integration_branch(subject.run);
            // Plan 43 A1: herdr's own files out of the rebase's way first (`set_aside_harness`).
            let moved = match set_aside_harness(subject.cwd, &integration, &aside) {
                Ok(moved) => moved,
                Err(why) => {
                    axes.push(
                        Axis::new(
                            "rebase",
                            Mark::Fail,
                            format!(
                                "herdr's own files under .nirvana/ could not be set aside: {why}"
                            ),
                        )
                        .fault(Fault::Gate),
                    );
                    det.base = Some(base);
                    det.axes = axes;
                    return det;
                }
            };
            let guard = AsideGuard {
                cwd: subject.cwd,
                aside: &aside,
                armed: !moved.is_empty(),
            };
            let rebased = rebase(subject.cwd, &integration);
            let restored = guard.restore();
            let aside_note = match (&restored, moved.is_empty()) {
                (_, true) => String::new(),
                (Ok(done), false) if done.kept.is_empty() => format!(
                    "; {} of herdr's own file(s) under .nirvana/ set aside for it and put back{}",
                    moved.len(),
                    done.note(&aside).map(|note| format!("; {note}")).unwrap_or_default()
                ),
                (Ok(done), false) => format!(
                    "; {} of herdr's own file(s) under .nirvana/ set aside for it and put back; the rebase brought {} \
of them, whose old copies are kept in {}{}",
                    moved.len(),
                    done.kept.len(),
                    aside.display(),
                    done.note(&aside).map(|note| format!("; {note}")).unwrap_or_default()
                ),
                (Err(why), false) => format!(
                    "; {} of herdr's own file(s) under .nirvana/ set aside for it and NOT put back ({why}): the next \
gate restores them from {}",
                    moved.len(),
                    aside.display()
                ),
            };
            match rebased {
                Ok(()) => axes.push(Axis::new(
                    "rebase",
                    Mark::Pass,
                    format!("onto {integration} at {}{aside_note}", short_sha(&base)),
                )),
                Err(fail) => {
                    let fault = rebase_fault(subject.cwd, &fail);
                    let (why, lines) = if fail.files.is_empty() {
                        (
                            format!("the branch does not rebase onto {integration}: git stopped before any conflict"),
                            fail.why.lines().map(str::to_owned).collect(),
                        )
                    } else {
                        (
                            format!(
                                "the branch does not rebase onto {integration}: {} file(s) conflict",
                                fail.files.len()
                            ),
                            fail.files.clone(),
                        )
                    };
                    axes.push(
                        Axis::new("rebase", Mark::Fail, format!("{why}{aside_note}"))
                            .with(capped(lines))
                            .fault(fault),
                    );
                    det.base = Some(base);
                    det.axes = axes;
                    return det;
                }
            }
            (Some(base), true)
        }
        None => {
            axes.push(Axis::new(
                "rebase",
                Mark::Note,
                "the order ran in the project's folder, not a worktree: no branch to rebase",
            ));
            (None, false)
        }
    };
    det.head = rev(subject.cwd, "HEAD");
    det.base = base.clone();
    // Plan 37b review N4: a blocked worker's tree is unfinished, and its gate asks whatever the tests say; running a
    // long `test:` there only delays the wake (a broken environment may be the block itself).
    let blocked = subject
        .report
        .and_then(|report| report.get("status"))
        .and_then(Value::as_str)
        == Some("blocked");
    let test = subject.spec.test.as_deref().filter(|_| !blocked);
    let run = test.map(|test| {
        let run = run_command(subject.cwd, test, Duration::from_secs(TEST_TIMEOUT_S));
        if let Some(log) = log {
            if let Some(parent) = log.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(log, &run.output);
        }
        run
    });
    axes.push(match subject.spec.test.as_deref() {
        Some(test) if blocked => Axis::new(
            "tests",
            Mark::Note,
            format!("`{test}` not run: the worker reported blocked"),
        ),
        _ => tests_axis(
            subject.spec.test.as_deref(),
            run.as_ref(),
            &subject.spec.checks,
        ),
    });
    axes.push(report_axis(subject.report));
    let changed = match (&base, worktree) {
        (Some(base), true) => changed_files(subject.cwd, base).map(|changed| (changed, None)),
        // Only an order with no branch comes here: a branch gives a base or returns above.
        _ => order_changes(subject),
    };
    // 0.71.1: a worktree's paths that came with the owner's commits it began at are not the order's.
    let changed = changed.map(|(changed, note)| match (&base, worktree) {
        (Some(base), true) => without_begun_with(subject.cwd, base, changed),
        _ => (changed, note.map(str::to_owned)),
    });
    let mut owned_new = None;
    axes.push(match changed {
        Ok((changed, note)) => {
            // Plan 43 A2: new against the base the axes ran on (the integration tip, or the project's HEAD for an
            // order in the project's folder), and held by the run's other live orders.
            let tree = base.clone().unwrap_or_else(|| "HEAD".to_owned());
            let git_dir = if worktree { subject.cwd } else { subject.root };
            let known = tracked_in(git_dir, &tree, &changed);
            let is_new = |path: &str| known.as_ref().is_ok_and(|known| !known.contains(path));
            let holders = live_holders(&Project::at(subject.root), subject.run, subject.order);
            let (mut owned, new_note) = owned_axes(
                subject.class,
                &changed,
                &subject.spec.owns,
                &is_new,
                &holders,
            );
            if let Some(note) = note {
                owned.why = format!("{} ({note})", owned.why);
            }
            if let Err(why) = &known {
                owned.why = format!("{} (no file was read as new: {why})", owned.why);
            }
            owned_new = new_note;
            owned
        }
        Err(why) => Axis::new("owned", Mark::Fail, why),
    });
    axes.extend(owned_new);
    let listened = if subject.class == "ops.terminal" {
        (subject.ports)()
    } else {
        Ok(Vec::new())
    };
    axes.push(served_axis(subject.class, &subject.spec.serves, listened));
    let stream = stream_text(&Project::at(subject.root), subject.run, subject.order);
    let web = web_of(subject.class, subject.spec);
    // Plan 43 A3: a cited path relative to the repository is looked for in the worker's folder, then the project's.
    let roots = [subject.cwd, subject.root];
    if let Some(axis) = sources_axis_at(subject.class, web, subject.report, &stream, &roots) {
        // Plan 37 §1.2: reel-motion-gemini's six YouTube ids were invented, and only a reader caught it, 8 days late.
        let (axis, note) = match oembed_base() {
            Some(base) => check_videos(
                axis,
                subject.report,
                &mut |id, left| oembed(&base, id, left.min(Duration::from_secs(OEMBED_TIMEOUT_S))),
                OEMBED_CALLS_PER_GATE,
                Duration::from_secs(OEMBED_BUDGET_S),
            ),
            None => (axis, None),
        };
        axes.push(axis);
        axes.extend(note);
    }
    axes.extend(look_axis(subject.spec.asks_look, &stream));
    det.axes = axes;
    det
}

// ---- plan 45 P45d (§4 M5): a look the order asked for that did not run ----

/// Whether `text` names a `herdr look` call: the word `herdr`, a `.exe` and a closing quote allowed after it (`&
/// "C:\...\herdr.exe" look`), then spaces, then `look`, then spaces and an argument a real call has there
/// (`look_argument`). How a brief asks for a look: herdr had no word list for it before P45d, and a word list
/// ("judge", "frame") would take every build that mentions a picture. A worker's stream is read by `runs_look`.
///
/// P45d fix round (review finding 3): `look` alone was enough, so "keep the herdr look and feel" and "make herdr look
/// right" asked for a look, and orders in this repository are about herdr. A negation is not read: "do not run `herdr
/// look a.png`" still asks, and the note it may bring is only a note.
pub fn names_look(text: &str) -> bool {
    text.match_indices("herdr").any(|(at, _)| {
        let rest = &text[at + "herdr".len()..];
        let rest = rest.strip_prefix(".exe").unwrap_or(rest);
        let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
        let spaced = rest.trim_start_matches([' ', '\t']);
        spaced.len() < rest.len() && spaced.strip_prefix("look").is_some_and(look_argument)
    })
}

/// What follows `look` in a real call, by `nirvana_look::USAGE` (`herdr look <file> --ask "<question>" [--seat <id>]
/// ...`): spaces, then a token that opens with a quote, a flag, a placeholder or a variable (`"<path>"`, `--ask`,
/// `<file>`, `$frame`, `%f%`, `{frame}`, `~/a.png`), or a path-like token (a `/` or `\` in it, or a `.` before a
/// letter or digit: `frame_012.png`). ROLE.md and the look and media skills teach the first three forms.
fn look_argument(after: &str) -> bool {
    let arg = after.trim_start_matches([' ', '\t']);
    if arg.len() == after.len() {
        return false;
    }
    let token = arg.split([' ', '\t', '\n', '\r']).next().unwrap_or("");
    let path = token.contains(['/', '\\']);
    let extension = token
        .char_indices()
        .any(|(at, c)| c == '.' && token[at + 1..].starts_with(|next: char| next.is_alphanumeric()));
    token.starts_with(['"', '\'', '-'])
        || token.starts_with(['<', '$', '%', '{', '~'])
        || path
        || extension
}

/// Whether a worker's shell line runs `herdr look`: in one of its commands (`shell_segments`: cut at `;`, `|`, `&&`,
/// `||` and line ends outside quotes), after PowerShell's call operator `&` and any `NAME=value` words, the command
/// word (`split_command`, quotes taken off) is `herdr` or a path to `herdr` or `herdr.exe`, and the next word is `look`.
///
/// P45d fix round (review finding 4): `names_look` over the whole line counted `grep -rn "herdr look" docs` as a look
/// that ran, which hid a missing look. Not seen: a call through a PowerShell variable (`$h = '...\herdr.exe'; & $h
/// look a.png`), one inside another command's quoted argument (`bash -c 'herdr look a.png'`) or a `$(...)`, and one in
/// a script; the note says the stream shows no call, not that none ran.
pub fn runs_look(line: &str) -> bool {
    shell_segments(line).iter().any(|segment| {
        let words = split_command(segment);
        let mut words = words
            .iter()
            .skip_while(|word| *word == "&" || env_assignment(word));
        words.next().is_some_and(|first| herdr_word(first))
            && words.next().is_some_and(|second| second == "look")
    })
}

/// `NAME=value` before a command word (`FRAME=a.png herdr look "$FRAME"`).
fn env_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// `herdr`, `herdr.exe`, or a path ending in either.
fn herdr_word(word: &str) -> bool {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    name.eq_ignore_ascii_case("herdr") || name.eq_ignore_ascii_case("herdr.exe")
}

/// The order asks for a look when its own text (herdr's lines, lessons and cards cut, `nirvana_profile_detect::
/// own_text`) names `herdr look`: a lesson or card line naming it is herdr's, not the orchestrator's ask.
pub fn asks_look(text: &str) -> bool {
    names_look(&crate::nirvana_profile_detect::own_text(text))
}

/// The `herdr look` calls in a worker's stream: how many were made, and how many answered without an error (a Claude
/// `tool_result` without `is_error`, an agy `run_command` step `DONE` with exit 0). A look that found no seat exits
/// `NO_SEAT_EXIT`, which both CLIs report as an error. A call counts by `runs_look`: herdr as the command word, so a
/// search for the words is no look; a look run inside a script or through a shell variable is not seen.
pub fn looks_in(stream: &str) -> (usize, usize) {
    let (mut made, mut ran) = (0usize, 0usize);
    let mut pending: Vec<String> = Vec::new();
    let mut agy_seen: Vec<String> = Vec::new();
    for line in stream.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if let Some(content) = val.pointer("/message/content").and_then(Value::as_array) {
            for block in content {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_use") => {
                        let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                        let input = block.get("input").unwrap_or(&Value::Null);
                        let command = input.get("command").and_then(Value::as_str).unwrap_or("");
                        if matches!(name, "Bash" | "PowerShell") && runs_look(command) {
                            made += 1;
                            let id = block.get("id").and_then(Value::as_str).unwrap_or("");
                            pending.push(id.to_owned());
                        }
                    }
                    Some("tool_result") => {
                        let id = block
                            .get("tool_use_id")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if let Some(at) = pending.iter().position(|call| call == id) {
                            pending.swap_remove(at);
                            let failed = block.get("is_error").and_then(Value::as_bool) == Some(true);
                            ran += usize::from(!failed);
                        }
                    }
                    _ => {}
                }
            }
        }
        let step = val.get("step_update").unwrap_or(&val);
        if step.get("step_type").and_then(Value::as_str) != Some("tool")
            || step.get("tool_name").and_then(Value::as_str) != Some("run_command")
        {
            continue;
        }
        let params = &step["tool_info"]["parameters"];
        let command = params["CommandLine"].as_str().unwrap_or("");
        if !runs_look(command) {
            continue;
        }
        // agy repeats a step on its answer's line: one call per tool and parameters.
        let call = params.to_string();
        if !agy_seen.contains(&call) {
            agy_seen.push(call);
            made += 1;
        }
        let answer = step["tool_info"]["output"].as_str().unwrap_or("");
        if step.get("state").and_then(Value::as_str) == Some("DONE")
            && step["tool_info"].get("error").is_none_or(Value::is_null)
            && agy_command_ok(answer)
        {
            ran += 1;
        }
    }
    (made, ran.min(made))
}

/// Plan 45 §4 M5: "`herdr look` had no seat in 35 delivered orders, 18 of them on one day. Nearly all reported
/// `done` on the worker's own reading. The gate does not check it." A note, never a fail (§5: "The gate notes a brief
/// that asked for a look when no look ran"): the worker may have judged rightly, and a look run from a script is not
/// seen. `None` when the order asks for none, or a look answered.
pub fn look_axis(asked: bool, stream: &str) -> Option<Axis> {
    if !asked {
        return None;
    }
    let (made, ran) = looks_in(stream);
    (ran == 0).then(|| {
        Axis::new(
            "look",
            Mark::Note,
            if made == 0 {
                "the order asks for `herdr look` and its stream shows no `herdr look` call (one through a shell \
                 variable or in a script is not seen), so what it says of the pictures may be the worker's own \
                 reading"
                    .to_owned()
            } else {
                format!(
                    "the order asks for `herdr look` and none of its {made} look call(s) answered (no seat, or an \
                     error), so what it says of the pictures is the worker's own reading"
                )
            },
        )
    })
}

// ---- the gate's own record, beside the journal ----

/// `<run>/gate/<order>.json`: what the journal's events do not carry. The relation of an order to its reviewers and
/// held-out authors, the check as captured, and the last deterministic result with the shas it ran at, so a
/// returning review does not run the tests again on the same trees.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub order: String,
    #[serde(default)]
    pub gates: u32,
    #[serde(default)]
    pub last: Option<Det>,
    #[serde(default)]
    pub reviews: Vec<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub heldout: Option<Heldout>,
    /// Why the order has no held-out check, said in every gate of it (decision 0019 §2.3: "never silently").
    #[serde(default)]
    pub heldout_skipped: Option<String>,
    /// The subject when this order is a reviewer or an author: `(kind, subject)`.
    #[serde(default)]
    pub helps: Option<(String, String)>,
    /// Plan 43 A1: `herdr gate amend`'s edits of this chain root's gate lines, oldest first, each with what it
    /// replaced. The order file stays as the worker read it; `amended_spec` lays these over it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub amends: Vec<Amend>,
}

/// One `herdr gate amend` (plan 43 A1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amend {
    pub at: u64,
    /// The order the amend was typed for; the edit is its chain root's.
    pub order: String,
    /// Patterns added to `owns:`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owns_added: Vec<String>,
    /// `owns:` as it stood before.
    #[serde(default)]
    pub owns_before: Vec<String>,
    /// The new `test:` command, and the one it replaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_before: Option<String>,
}

/// The chain root's gate lines with its amends laid over them: owns added, the test replaced.
pub fn amend_spec(mut spec: Spec, amends: &[Amend]) -> Spec {
    for amend in amends {
        for pattern in &amend.owns_added {
            if !spec.owns.contains(pattern) {
                spec.owns.push(pattern.clone());
            }
        }
        if let Some(test) = &amend.test {
            spec.test = Some(test.clone());
        }
    }
    spec
}

/// The gate lines of `root`'s order as the gate holds it to them: its order file and its amends.
fn amended_spec(project: &Project, run: &str, root: &Seated) -> Spec {
    amend_spec(
        spec(&order_text(root)),
        &read_sidecar(project, run, &root.order).amends,
    )
}

/// Of `paths`, those `tree` holds. `--literal-pathspecs`: a path is a name, never a pattern.
fn tracked_in(cwd: &Path, tree: &str, paths: &[String]) -> Result<HashSet<String>, String> {
    let mut found = HashSet::new();
    for chunk in paths.chunks(64) {
        let mut args = vec![
            "--literal-pathspecs",
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            tree,
            "--",
        ];
        args.extend(chunk.iter().map(String::as_str));
        found.extend(
            git(cwd, &args)?
                .split('\0')
                .filter(|path| !path.is_empty())
                .map(str::to_owned),
        );
    }
    Ok(found)
}

/// Plan 43 A2: the classes that own no file: reviewers, lesson reviews, held-out authors and teachers.
fn owns_nothing(class: &str) -> bool {
    class == crate::nirvana_review::CLASS
        || class == "review.adversary"
        || class == "acceptance.author"
        || class == crate::nirvana_knowledge::TEACH_CLASS
}

/// Plan 43 A2: the run's other live orders and the `owns:` each holds, `order`'s own chain left out. Live is seated,
/// running, deferred, delivered or asked, or gated and not yet merged as a pass, an amend or an accept: work that may
/// still land. A send-back is left out (its child holds the same lines), as are merged, failed, re-seated and
/// interrupted orders. Each holds its chain root's lines, amended.
pub fn live_holders(project: &Project, run: &str, order: &str) -> Vec<Holder> {
    let state = replay(&project.journal(run).read().0);
    let (own_root, _) = chain(&state, order);
    let mut holders: Vec<Holder> = Vec::new();
    for known in &state.orders {
        if owns_nothing(&known.seated.class) {
            continue;
        }
        let live = match known.phase {
            Phase::Seated | Phase::Running | Phase::Deferred | Phase::Delivered | Phase::Asked => {
                true
            }
            Phase::Gated => matches!(
                known.verdict.as_deref(),
                Some(verdict) if verdict == Outcome::Pass.word()
                    || verdict == Outcome::Amend.word()
                    || verdict == Outcome::AMENDING
                    || verdict == Outcome::ACCEPTED
            ),
            Phase::Merged | Phase::Failed | Phase::Reseated | Phase::Interrupted => false,
        };
        if !live {
            continue;
        }
        let (root, _) = chain(&state, known.id());
        if root == own_root || holders.iter().any(|holder| holder.order == root) {
            continue;
        }
        let Some(root_seated) = state.order(&root).map(|root| &root.seated) else {
            continue;
        };
        let owns = amended_spec(project, run, root_seated).owns;
        if !owns.is_empty() {
            holders.push(Holder { order: root, owns });
        }
    }
    holders
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heldout {
    pub author: String,
    /// The captured folder, beside the journal; the builder's worktree never holds it.
    pub dir: String,
    pub command: String,
    pub base_cases: Vec<(String, bool)>,
}

pub(crate) fn gate_dir(project: &Project, run: &str) -> PathBuf {
    project.run_dir(run).join("gate")
}

pub fn read_sidecar(project: &Project, run: &str, order: &str) -> Sidecar {
    std::fs::read_to_string(gate_dir(project, run).join(format!("{order}.json")))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| Sidecar {
            order: order.to_owned(),
            ..Sidecar::default()
        })
}

pub(crate) fn write_sidecar(
    project: &Project,
    run: &str,
    sidecar: &Sidecar,
) -> std::io::Result<()> {
    let dir = gate_dir(project, run);
    std::fs::create_dir_all(&dir)?;
    let mut text = serde_json::to_string_pretty(sidecar)?;
    text.push('\n');
    nirvana_seat::write_atomic(
        &dir.join(format!("{}.json", sidecar.order)),
        text.as_bytes(),
    )
}

/// One gate at a time per project (§7.2: "the gate's builds run one at a time"; old Nirvana's three gates inside
/// four minutes collided on ports, `WinError 10048`). An OS lock under `runs/`, which git ignores.
pub(crate) struct GateLock(std::fs::File);

impl Drop for GateLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(crate) fn gate_lock(project: &Project) -> std::io::Result<GateLock> {
    let path = project.dir().join("runs").join(".gate.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(GateLock(file))
}

// ---- chains, sizes and families ----

/// The order a chain of send-backs and reseats started from, and how many send-backs it has had, this order's own
/// included. Plan 40 review B1: a rate-limit reseat (`why = "reseat"`) is followed to the root too, as `helps_of`,
/// `nirvana_learning` and `nirvana_review` follow it. Stopping at it lost the root's held-out check and seat snapshot,
/// and restarted the send-back cap after each reseat. A reseat is not a send-back, so it adds nothing to the depth.
pub fn chain(state: &nirvana_run::RunState, order: &str) -> (String, u32) {
    let mut current = order.to_owned();
    let mut depth = 0;
    // A journal is append-only and a child names an earlier parent, so a loop cannot happen; the bound is for a
    // hand-edited journal.
    for _ in 0..1000 {
        let Some(known) = state.order(&current) else {
            break;
        };
        match (known.seated.why.as_deref(), &known.seated.parent) {
            (Some(why @ ("send_back" | "reseat")), Some(parent)) => {
                if why == "send_back" {
                    depth += 1;
                }
                current = parent.clone();
            }
            _ => break,
        }
    }
    (current, depth)
}

/// What the gate does with a failing order (§7.3): a child, or the owner once the cap is spent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendBack {
    Child { id: String, n: u32 },
    Ask,
}

pub fn plan_send_back(state: &nirvana_run::RunState, order: &str) -> SendBack {
    let (root, depth) = chain(state, order);
    if depth >= SEND_BACK_CAP {
        return SendBack::Ask;
    }
    let n = depth + 1;
    let base: String = root.chars().take(56).collect();
    let id = (0..)
        .map(|extra| {
            if extra == 0 {
                format!("{base}-s{n}")
            } else {
                format!("{base}-s{n}-{extra}")
            }
        })
        .find(|id| state.order(id).is_none())
        .unwrap_or_else(|| format!("{base}-s{n}"));
    SendBack::Child { id, n }
}

/// A load-bearing order gets a reviewer of another family: the class's blast in the table (plan, build.load_bearing).
pub fn load_bearing(table: Option<&Table>, class: &str) -> bool {
    match table.and_then(|table| table.class(class)) {
        Some(class) => class.blast == Blast::LoadBearing,
        None => matches!(class, "plan" | "build.load_bearing"),
    }
}

/// A feature-size order gets a held-out check: a build or a debug fix that is not small. §7.3 names "feature-size"
/// without a rule; this reads it off what the orchestrator already says of an order, its class and its size.
pub fn feature_size(class: &str, size: SizeName) -> bool {
    (class.starts_with("build.") || class == "fix.debug") && size != SizeName::Small
}

/// Decision 0017 through O2's `pick`, whose fix round (B1) holds the fallback: another family first; only when no
/// other family has a seat, the builder's own in a fresh session, `same_family_fallback` set and the reason ending
/// "(decision 0017)". The gate puts its mark on top, at the start of the reason, so the orchestrator reads which
/// second opinion it is starting before it reads why.
pub fn pick_other_family(
    table: &Table,
    world: &World,
    class: &str,
    family: &str,
    mode: nirvana_seat::Mode,
    order: &str,
    mark: &str,
) -> Result<(Pick, bool), NoSeat> {
    let mut request = Request::new(class);
    request.mode = mode;
    request.size = SizeName::Small;
    request.order = Some(order.to_owned());
    request.not_family = vec![family.to_owned()];
    let mut pick = nirvana_seat::pick(table, world, &request)?;
    let same = pick.same_family_fallback;
    if same {
        pick.reason = format!("{mark}: {}", pick.reason);
    }
    Ok((pick, same))
}

fn family_of(table: Option<&Table>, seat: &str) -> String {
    table
        .and_then(|table| table.seat(seat))
        .map_or_else(|| "unknown".to_owned(), |seat| seat.family.clone())
}

// ---- briefs ----

fn original(text: &str) -> String {
    text.split("\n\n---\nFrom herdr")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_owned()
}

/// A send-back child's brief: the reasons, then the root order. A held-out failure gives the case ids only.
/// A sources fail means the gate saw no read of the cited sources, and reel-motion-gemini's twelve were invented: its
/// send-back said "do not start over" and kept them (MotoProject, 2026-10-31). Nothing cited is trusted.
pub const SOURCES_REDO: &str = "none of the sources you cited is trusted, because the gate saw no read of them. Open \
each one again in this session, with a command or tool of its own, before you cite it; drop every source you cannot \
open, and redo each claim that rests on a dropped source.";
/// Plan 37 §1.3 (L5): a cited video that does not exist was invented, so the first try's notes and claims are no
/// better than its sources. reel-motion-gemini's send-back kept its file and was stopped as stale (2026-10-31).
pub const INVENTED_REDO: &str = "the gate found cited videos that do not exist, so nothing from the first try is \
trusted: not its sources, its notes or its claims. Do the order again from the start, and cite only what you opened \
in this session.";
/// Review N4: the same finding when other axes failed too. Plan 35's S4 holds (the other axes' instruction stays, the
/// sources text is added): a rebase or a held-out case only makes sense on the work as it stands, so the order is not
/// started over, but nothing the first try took from its sources stands.
pub const INVENTED_SOURCES: &str = "the gate found cited videos that do not exist, so nothing of the first try's \
sources is trusted: drop every source it cited and every claim that rests on one, and find them again, citing only \
what you opened in this session.";

/// The send-back line for an order seated with `NO_WEB_SOURCES`, whatever axis failed. Review S2: it names the fetch
/// the fix needs as well as the one it does not.
pub const NO_WEB_REDO: &str = "herdr found no web page asked for in this order: the gate wants a source from you \
only for a page you open. If the fix needs a page you do not have, open it and cite it; never open one only to have a \
source.";

pub fn send_back_brief(
    order: &str,
    root: &str,
    n: u32,
    axes: &[Axis],
    root_text: &str,
    integration: &str,
) -> String {
    // Review round (S4): the sources text only when there are cited sources to open again (not for "no Work Report"
    // or "names no sources"), and on a mixed fail after the other axes' instruction, not instead of it.
    let redo = failing(axes)
        .iter()
        .any(|axis| axis.axis == "sources" && !axis.lines.is_empty());
    let others = failing(axes).iter().any(|axis| axis.axis != "sources");
    let fix = "fix what the gate found, do not start over.";
    let invented = !invented_videos(axes).is_empty();
    let opening = match (redo, others) {
        // Plan 37 §1.3: an invented video starts the order over; on a mixed fail it is the sources' text (review N4).
        _ if invented && !others => INVENTED_REDO.to_owned(),
        _ if invented => format!("{fix} For the sources: {INVENTED_SOURCES}"),
        (true, false) => SOURCES_REDO.to_owned(),
        (true, true) => format!("{fix} For the sources: {SOURCES_REDO}"),
        (false, _) => fix.to_owned(),
    };
    // Live checks 2026-11-01 F2: lc4-s1's brief said "no sources" over an order line asking for them, and its worker
    // fetched example.com to have one. An order that asks for no web is told the gate wants no source from it.
    let no_web = if spec(root_text).no_web {
        format!(" {NO_WEB_REDO}")
    } else {
        String::new()
    };
    let mut brief = format!(
        "# Send-back {n} of {SEND_BACK_CAP}: {root}\n\nherdr's gate sent back {order}. Its work is in this folder, committed \
on this branch. You are a new session: {opening}{no_web}\n\n## What the gate found\n\n"
    );
    for axis in failing(axes) {
        brief.push_str(&format!("- **{}**: {}\n", axis.axis, axis.why));
        match axis.axis.as_str() {
            "heldout" => {
                brief.push_str("  A check you cannot see fails these cases; make the work meet the order so they pass:\n");
                for id in &axis.lines {
                    brief.push_str(&format!("  - `{id}`\n"));
                }
            }
            "rebase" => {
                brief.push_str(&format!(
                    "  Run `git rebase {integration}` here and resolve, keeping both changes:\n"
                ));
                for line in &axis.lines {
                    brief.push_str(&format!("  - `{line}`\n"));
                }
            }
            _ => {
                for line in &axis.lines {
                    brief.push_str(&format!("  > {line}\n"));
                }
            }
        }
    }
    brief.push_str("\n## The original order\n\n");
    brief.push_str(&original(root_text));
    brief.push('\n');
    brief
}

fn review_brief(
    order: &str,
    root_text: &str,
    worktree: &str,
    diff: &str,
    same_family: bool,
) -> String {
    format!(
        "# Review of {order} (decision 0017)\n\nAnother agent did the order below. Judge its change as a stranger's work{}. \
Read the diff and the files; change nothing, run what you need to.\n\n- the diff: {diff}\n- its folder: {worktree}\n\n\
Your Work Report is the verdict:\n- status `done`: the change does what the order asks, and nothing it must not.\n\
- status `blocked`: it must go back; put each defect in `gotchas`, one line each, with the file and what is wrong.\n\
- status `needs_answer`: you cannot judge it without the owner; the question in `question`.\n\n## The order\n\n{}\n",
        if same_family { ", though a model of its own family wrote it" } else { "" },
        quoted(root_text)
    )
}

/// The subject's order as a gate review or a held-out author's brief quotes it: herdr's footer cut, and its `Profile:`
/// and `Profile sha256:` lines and any agy profile section taken out, as a lesson review's brief takes them out
/// (`nirvana_review::quoted_order`). Plan 45 §4 M6 and P45d: the review briefs quoted the subject's `Profile:` line,
/// `seat run` refused every review of a profiled order ("the order says `Profile: ...` at line start"), and the
/// orchestrator stripped the line by hand. A reviewer judges the change as a stranger's work, so it takes none of the
/// subject's profile either: no body, no cards, no servers.
fn quoted(text: &str) -> String {
    crate::nirvana_review::quoted_order(text, None)
}

fn author_brief(order: &str, request: &str, retry: Option<&str>) -> String {
    format!(
        "# Held-out check for {order} (decision 0019)\n\nAnother agent will do the order below after you. You write the \
check it will be held to; you do not build anything.{}\n\n## What to write\n\n\
- A folder `.nirvana/heldout/` in this folder, holding the check: one test file or a few, written from the order \
alone. Name every case. Import the code under test inside each case.\n\
- `.nirvana/heldout/COMMAND`: one line that runs the check from this folder, with `{{dir}}` where the check folder's \
path goes, e.g. `python -m pytest -q -rA {{dir}}`. It must print one line per case with its result (pytest `-rA`, \
`cargo test`, unittest `-v`).\n\
- The check must FAIL here as the folder is now: at least one case fails before the build.\n\
- Change no other file. Read no network.\n\nEnd with the Work Report: status done, the cases named in the summary.\n\n\
## The order\n\n{}\n",
        retry.map(|why| format!("\n\nYour first try was refused: {why}. Write it again.")).unwrap_or_default(),
        quoted(request)
    )
}

// ---- the gate ----

/// What `herdr gate run` decided.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ruling {
    pub order: String,
    pub run: String,
    /// pass, sent_back, asked, or wait.
    pub verdict: String,
    pub axes: Vec<Axis>,
    /// What comes next: the child, the review or author order to start, or the merge.
    pub next: Vec<String>,
}

impl Ruling {
    pub fn exit(&self) -> i32 {
        match self.verdict.as_str() {
            "pass" => 0,
            "wait" => WAIT_EXIT,
            _ => 1,
        }
    }
}

pub(crate) fn order_text(seated: &Seated) -> String {
    std::fs::read_to_string(&seated.order_file).unwrap_or_default()
}

pub(crate) fn stream_text(project: &Project, run: &str, order: &str) -> String {
    std::fs::read_to_string(
        project
            .run_dir(run)
            .join("streams")
            .join(format!("{order}.jsonl")),
    )
    .unwrap_or_default()
}

pub(crate) fn axes_values(axes: &[Axis]) -> Vec<Value> {
    axes.iter()
        .filter_map(|axis| serde_json::to_value(axis).ok())
        .collect()
}

/// The reason a `gated` event carries: the first failing axis, with no order id in it, so the retro can match the
/// same gap across runs.
pub fn reason_line(axes: &[Axis]) -> Option<String> {
    failing(axes)
        .first()
        .map(|axis| format!("{}: {}", axis.axis, axis.why))
}

fn seat_run_line(
    pick: &Pick,
    file: &Path,
    id: &str,
    run: &str,
    class: &str,
    not_family: Option<&str>,
) -> String {
    format!(
        "herdr seat run {} --order \"{}\" --order-id {id} --run {run} --class {class}{}",
        pick.seat,
        file.display(),
        not_family
            .map(|family| format!(" --not-family {family}"))
            .unwrap_or_default()
    )
}

/// Books `id` on `pick` so the `herdr seat run` the orchestrator types gets the same seat (O3 reads a live booking
/// first).
pub(crate) fn book(table: &Table, pick: &Pick, id: &str, run: &str) -> std::io::Result<()> {
    let dir = nirvana_seat::dir();
    let _lock = nirvana_seat::Lock::take(&dir)?;
    let path = dir.join(nirvana_seat::LEASES_FILE);
    // `load_leases` and `book_pick`, as O3's `seat run` books since its merge: a corrupt file is an error, never read
    // as empty and written back over every run's override count (O2 fix round).
    let mut leases = nirvana_seat::load_leases(&path).map_err(std::io::Error::other)?;
    nirvana_seat::book_pick(&mut leases, table, pick, Some(id), Some(run), now());
    nirvana_seat::write_leases(&path, &leases)
}

fn write_order_file(project: &Project, id: &str, text: &str) -> std::io::Result<PathBuf> {
    project.ensure()?;
    std::fs::create_dir_all(project.orders_dir())?;
    let path = project.orders_dir().join(format!("{id}.md"));
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The worker's pane's listening ports, read live: `herdr pane process-info` for the shell, then the tables.
fn live_ports(pane: Option<&str>) -> Result<Vec<u16>, String> {
    let pane = pane.ok_or("the order has no pane")?;
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let out = std::process::Command::new(exe)
        .args(["pane", "process-info", pane])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| "pane process-info gave no JSON".to_owned())?;
    let shell = [
        "/result/process_info/shell_pid",
        "/process_info/shell_pid",
        "/shell_pid",
    ]
    .iter()
    .find_map(|pointer| value.pointer(pointer).and_then(Value::as_u64))
    .ok_or("pane process-info named no shell pid")? as u32;
    // The Spaces square's own reader (`nirvana_servers::read`), so the gate and the square see one table.
    let processes = crate::nirvana_servers::read::processes();
    let listeners = crate::nirvana_servers::read::listeners();
    Ok(pane_ports(shell, &processes, &listeners))
}

#[cfg(test)]
thread_local! {
    /// Review N1's hook: the next gate on this thread panics once it holds the lock.
    pub(crate) static TEST_GATE_PANICS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Gates one order. The caller holds nothing; this takes the project's gate lock.
pub fn gate(project: &Project, run: &str, order: &str) -> Result<Ruling, String> {
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    #[cfg(test)]
    if TEST_GATE_PANICS.replace(false) {
        panic!("a test's gate panic");
    }
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in run {run}"))?
        .clone();
    let sidecar = read_sidecar(project, run, order);
    let table = nirvana_seat::table_for(&project.root).ok();
    // Plan 33 review: a lesson review re-seated when its pool ran out carries its first review's sidecar, so its child
    // is gated as a review and never as a plain order.
    let helps = helps_of(project, run, &state, order).or_else(|| {
        (known.seated.class == crate::nirvana_review::CLASS)
            .then(|| {
                read_sidecar(
                    project,
                    run,
                    &crate::nirvana_review::review_root(&state, order),
                )
                .helps
            })
            .flatten()
    });
    // Plan 34 §2.5: a teaching order's card ops are applied by herdr; it is never gated as a plain order.
    if known.seated.class == crate::nirvana_knowledge::TEACH_CLASS {
        return crate::nirvana_knowledge::gate_teach(project, run, &journal, &known);
    }
    match helps {
        Some((kind, subject)) if kind == "author" => {
            return gate_author(project, run, &journal, &known, &subject, table.as_ref())
        }
        Some((kind, subject))
            if kind == crate::nirvana_review::REVIEW || kind == crate::nirvana_review::MERGE =>
        {
            return crate::nirvana_review::gate_review(
                project, run, &journal, &known, &kind, &subject,
            )
        }
        Some((kind, subject)) if kind == "review" => {
            let own = gate_reviewer(&journal, &known, run)?;
            if own.verdict != "pass" {
                return Ok(own);
            }
            drop(_lock);
            return gate(project, run, &subject);
        }
        _ => {}
    }
    match known.phase {
        Phase::Delivered => {}
        // A passed order whose branch or integration tip moved since (another order merged first) is gated again,
        // onto the new tip: `gate merge` refuses the old trees.
        Phase::Gated
            if known.verdict.as_deref() == Some("pass")
                && !trees_moved(project, run, &known, &sidecar) =>
        {
            return Ok(Ruling {
                order: order.to_owned(),
                run: run.to_owned(),
                verdict: "pass".into(),
                axes: sidecar.last.map(|det| det.axes).unwrap_or_default(),
                next: vec![format!("herdr gate merge {order} --run {run}")],
            })
        }
        Phase::Gated if known.verdict.as_deref() == Some("pass") => {}
        // Plan 43 fix round F6: an amend journaled whose re-gate never gave its verdict.
        Phase::Gated if known.verdict.as_deref() == Some(Outcome::AMENDING) => {}
        // Review S2: an order gated sent_back or asked is ruled on already; a gate by hand (the wake that carried the
        // verdict was missed, or the orchestrator wants it again) gives the stored verdict, with the child or the
        // question, and journals nothing.
        // Plan 43 A1: an amend's ruling too, its `herdr gate amend` line again.
        Phase::Gated
            if matches!(
                known.verdict.as_deref(),
                Some(verdict) if verdict == Outcome::SentBack.word()
                    || verdict == Outcome::Asked.word()
                    || verdict == Outcome::Amend.word()
            ) =>
        {
            return Ok(stored_ruling(&state, &known, run, sidecar))
        }
        phase => {
            return Err(format!(
                "{order} is {}{}: the gate rules on a delivered order",
                phase.word(),
                known
                    .verdict
                    .as_deref()
                    .map(|v| format!(" ({v})"))
                    .unwrap_or_default()
            ))
        }
    }
    gate_subject(
        project,
        run,
        &journal,
        &state,
        &known,
        sidecar,
        table.as_ref(),
        Route::ByFault,
        Vec::new(),
    )
}

/// Plan 43 A1: where a refusal goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// A child for a worker fault; the orchestrator's `herdr gate amend` line when no failing axis is the worker's.
    ByFault,
    /// A child whatever the fault: `herdr gate amend --send-back`, the orchestrator's choice.
    Child,
}

/// Whether the trees a passed order's gate ran on are gone: its branch's head or the integration tip moved.
fn trees_moved(project: &Project, run: &str, known: &OrderState, sidecar: &Sidecar) -> bool {
    let Some(last) = &sidecar.last else {
        return false;
    };
    if known.seated.branch.is_none() {
        return false;
    }
    let head = rev(Path::new(&known.seated.cwd), "HEAD");
    let tip = rev(
        &project.root,
        &format!("refs/heads/{}", integration_branch(run)),
    );
    last.head != head || last.base != tip
}

/// The ruling a sent_back or asked order already has, from the journal: a send-back's latest child and where it
/// stands, or the question asked.
fn stored_ruling(
    state: &nirvana_run::RunState,
    known: &OrderState,
    run: &str,
    sidecar: Sidecar,
) -> Ruling {
    let order = known.id();
    let verdict = known.verdict.clone().unwrap_or_default();
    let next = if verdict == Outcome::SentBack.word() {
        let child = state.orders.iter().rev().find(|child| {
            child.seated.parent.as_deref() == Some(order)
                && child.seated.why.as_deref() == Some("send_back")
        });
        match child {
            Some(child) => format!(
                "{} seated on {} {} ({}): the send-back of {order}, already made",
                child.id(),
                child.seated.seat,
                child.seated.effort.word(),
                child.phase.word()
            ),
            None => format!("{order} was sent back and the journal holds no child of it"),
        }
    } else {
        known.why.clone().unwrap_or_default()
    };
    Ruling {
        order: order.to_owned(),
        run: run.to_owned(),
        verdict,
        axes: sidecar.last.map(|det| det.axes).unwrap_or_default(),
        next: vec![next],
    }
}

#[allow(clippy::too_many_arguments)]
fn gate_subject(
    project: &Project,
    run: &str,
    journal: &Journal,
    state: &nirvana_run::RunState,
    known: &OrderState,
    mut sidecar: Sidecar,
    table: Option<&Table>,
    route: Route,
    notes: Vec<Axis>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    let seated = &known.seated;
    let (root, _) = chain(state, &order);
    let root_seated = state
        .order(&root)
        .map_or_else(|| seated.clone(), |root| root.seated.clone());
    let root_text = order_text(&root_seated);
    // Plan 43 A1: the chain root's `herdr gate amend` edits, laid over its order file.
    let spec = amend_spec(spec(&root_text), &read_sidecar(project, run, &root).amends);
    let cwd = PathBuf::from(&seated.cwd);
    let pane = known.pane.clone();
    let ports = move || live_ports(pane.as_deref());
    let head_now = rev(&cwd, "HEAD");
    let base_now = rev(
        &project.root,
        &format!("refs/heads/{}", integration_branch(run)),
    );
    let dirty = uncommitted(&cwd)
        .map(|paths| !paths.is_empty())
        .unwrap_or(true);
    // A returning review reruns nothing when the trees are the ones the axes ran on.
    let reuse = sidecar.last.as_ref().is_some_and(|last| {
        !dirty
            && last.head.is_some()
            && last.head == head_now
            && last.base == base_now
            && failing(&last.axes).is_empty()
    });
    sidecar.gates += 1;
    let det = if reuse {
        sidecar.last.clone().unwrap_or_default()
    } else {
        let log = gate_dir(project, run).join(format!("{order}-g{}.log", sidecar.gates));
        deterministic(
            &Subject {
                root: &project.root,
                cwd: &cwd,
                run,
                order: &order,
                branch: seated.branch.as_deref(),
                class: &seated.class,
                report: known.report.as_ref(),
                spec: &spec,
                ports: &ports,
            },
            Some(&log),
        )
    };
    sidecar.last = Some(det.clone());
    let mut axes = det.axes.clone();
    // Plan 43 A1: what `herdr gate amend` changed, journaled with the verdict it led to.
    axes.extend(notes);
    // Held-out: the chain root's check, run at this order's head.
    let root_sidecar = if root == order {
        sidecar.clone()
    } else {
        read_sidecar(project, run, &root)
    };
    if failing(&axes).is_empty() {
        if let Some(heldout) = &root_sidecar.heldout {
            let line = heldout.command.replace("{dir}", &heldout.dir);
            let result = run_command(&cwd, &line, Duration::from_secs(TEST_TIMEOUT_S));
            let _ = std::fs::write(
                gate_dir(project, run).join(format!("{order}-g{}-heldout.log", sidecar.gates)),
                &result.output,
            );
            axes.push(heldout_axis(&result));
        } else if let Some(author) = root_sidecar
            .authors
            .last()
            .filter(|_| root_sidecar.heldout_skipped.is_none())
        {
            axes.push(Axis::new(
                "heldout",
                Mark::Wait,
                format!("{author} wrote the held-out check and it is not judged yet: `herdr gate run {author}`"),
            ));
        } else if feature_size(&root_seated.class, root_seated.size)
            || root_sidecar.heldout_skipped.is_some()
        {
            axes.push(Axis::new(
                "heldout",
                Mark::Note,
                root_sidecar.heldout_skipped.clone().unwrap_or_else(|| {
                    "no held-out check was written before the build (decision 0019: said, never silent)".into()
                }),
            ));
        }
    }
    // The review, last: a reviewer reads work that already passes the rest.
    let mut next = Vec::new();
    if failing(&axes).is_empty()
        && !axes.iter().any(|axis| axis.mark == Mark::Wait)
        && load_bearing(table, &seated.class)
    {
        match review_state(state, &sidecar) {
            ReviewState::None => {
                let line =
                    request_review(project, run, known, &root_text, &det, &mut sidecar, table)?;
                axes.push(Axis::new(
                    "review",
                    Mark::Wait,
                    "a reviewer of another family is booked; start it",
                ));
                next.push(line);
            }
            ReviewState::Pending(id) => {
                axes.push(Axis::new(
                    "review",
                    Mark::Wait,
                    format!("{id} has not delivered its verdict"),
                ));
                // Plan 37 §1.1: `seat exec` gates a delivery itself, so the line that starts a booked reviewer was
                // printed to no one; a gate by hand gives it again while the reviewer is not seated.
                if state.order(&id).is_none() {
                    next.push(booked_review_line(project, run, known, &id, table));
                }
            }
            ReviewState::Verdict(id, report) => {
                // The reviewer's own gate, when it was not gated by name: its report is well formed.
                if let Some(review) = state.order(&id).filter(|review| review.verdict.is_none()) {
                    let _ = gate_reviewer(journal, review, run);
                }
                axes.push(review_axis(&id, &report));
            }
        }
    }
    write_sidecar(project, run, &sidecar).map_err(|err| err.to_string())?;
    if axes.iter().any(|axis| axis.mark == Mark::Wait) {
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: "wait".into(),
            axes,
            next,
        });
    }
    let stream = stream_text(project, run, &order);
    // Plan 34 §2.3: the cards' counters are the lessons' own, counted at the same verdicts.
    let card_profile = order_profile(seated, &spec);
    if failing(&axes).is_empty() {
        // Lessons: the order passed; the quote must be in its own stream (§7.4.3).
        if let Some(lesson) = known
            .report
            .as_ref()
            .and_then(|report| report.get("lesson"))
        {
            // Plan 33 §3.1: any project, remote or none; §3.2: the order's own copy names its profile.
            let profile = order_profile(seated, &spec);
            let (verdict, ledger_note) = crate::nirvana_lessons::verify_and_write_noted(
                &project.root,
                lesson,
                true,
                &stream,
                &order,
                run,
                now(),
                crate::nirvana_lessons::Meta {
                    profile: profile.as_deref(),
                    origin: crate::nirvana_lessons::Origin::Worker,
                },
            );
            axes.push(match verdict {
                crate::nirvana_lessons::Verdict::Written(path) => {
                    if let Some(home) = crate::nirvana::home() {
                        let _ = crate::nirvana_lessons::write_index_line(&home, &project.root);
                    }
                    Axis::new(
                        "lesson",
                        Mark::Pass,
                        match &ledger_note {
                            Some(note) => format!("written to {}; {note}", path.display()),
                            None => format!("written to {}", path.display()),
                        },
                    )
                }
                crate::nirvana_lessons::Verdict::Refused(why) => {
                    Axis::new("lesson", Mark::Note, format!("not written: {why}"))
                }
            });
        }
        let _ = crate::nirvana_lessons::note_gated(&project.root, &stream, &order, true);
        if let Some(profile) = &card_profile {
            let _ =
                crate::nirvana_knowledge::note_gated(&project.root, profile, &stream, &order, true);
        }
        // Plan 33 §3.3: the cap's refusal is journaled with the verdict, as an axis named `review_skipped`; so is a
        // clean first pass, which has nothing to learn (review-trim-agy-browser 2026-11-02, live checks F8).
        let review = crate::nirvana_review::wanted_for(
            project,
            state,
            known,
            &crate::nirvana_review::Verdict::Pass,
            table,
        );
        if let Err(Some(why)) = &review {
            axes.push(Axis::new(
                crate::nirvana_review::SKIPPED,
                Mark::Note,
                why.clone(),
            ));
        }
        journal
            .append(
                now(),
                Kind::Gated {
                    order: order.clone(),
                    verdict: Outcome::Pass.word().into(),
                    reason: None,
                    axes: axes_values(&axes),
                },
            )
            .map_err(|err| err.to_string())?;
        record(project, run, &order);
        next.push(format!("herdr gate merge {order} --run {run}"));
        if review.is_ok() {
            next.push(crate::nirvana_review::queue_review(
                project,
                run,
                journal,
                known,
                &crate::nirvana_review::Verdict::Pass,
                table,
            ));
        }
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: Outcome::Pass.word().into(),
            axes,
            next,
        });
    }
    // Plan 37b review S1: a block is the owner's or the orchestrator's to answer, so the lessons and cards the worker
    // opened are not counted as failing after it (a give-up counts nothing too: S3's hypothesis, unmeasured).
    let blocked = block_reason(known).is_some();
    // Plan 43 A1: no failing axis is the worker's, so no child: the orchestrator amends the brief and re-gates the same
    // branch. A block still wins (`send_back`), and the worker's lessons are not counted against a fault not its own.
    if !blocked && route == Route::ByFault && no_worker_fault(&axes) {
        return amend_refusal(project, run, journal, &order, axes);
    }
    if !blocked {
        let _ = crate::nirvana_lessons::note_gated(&project.root, &stream, &order, false);
    }
    if let Some(profile) = card_profile.as_ref().filter(|_| !blocked) {
        let _ =
            crate::nirvana_knowledge::note_gated(&project.root, profile, &stream, &order, false);
    }
    let reasons: Vec<String> = failing(&axes)
        .iter()
        .map(|axis| format!("{}: {}", axis.axis, axis.why))
        .collect();
    let review = crate::nirvana_review::wanted_for(
        project,
        state,
        known,
        &crate::nirvana_review::Verdict::SentBack(reasons.clone()),
        table,
    );
    if let Err(Some(why)) = &review {
        axes.push(Axis::new(
            crate::nirvana_review::SKIPPED,
            Mark::Note,
            why.clone(),
        ));
    }
    let mut ruling = send_back(
        project, run, journal, state, known, &root, &root_text, axes, table,
    )?;
    // §3.3: "after `herdr gate run` journals `Gated` (pass) or a send-back"; an `asked` (the cap spent) is neither.
    if review.is_ok() && ruling.verdict == Outcome::SentBack.word() {
        ruling.next.push(crate::nirvana_review::queue_review(
            project,
            run,
            journal,
            known,
            &crate::nirvana_review::Verdict::SentBack(reasons),
            table,
        ));
    }
    Ok(ruling)
}

/// The profile an order was seated with: its own copy's `Profile:` line, else its chain root's. None for a lesson
/// review, whose brief quotes the order it reviews (plan 33 review B1).
fn order_profile(seated: &Seated, root_spec: &Spec) -> Option<String> {
    if seated.class == crate::nirvana_review::CLASS {
        return None;
    }
    spec(&order_text(seated))
        .profile
        .or_else(|| root_spec.profile.clone())
}

/// The record line, once, from the journal as it now stands.
pub(crate) fn record(project: &Project, run: &str, order: &str) {
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    if let Some(known) = state.order(order) {
        let _ = nirvana_run::write_record(run, known, nirvana_run::journal_ref(&journal, &state));
    }
}

/// Plan 37b: the block a `blocked` Work Report gives, its summary and its question when it has one; None for any other
/// status. A lesson review and a helper never reach here: `gate` routes them before `gate_subject`.
fn block_reason(known: &OrderState) -> Option<String> {
    let report = known.report.as_ref()?;
    if report.get("status").and_then(Value::as_str) != Some("blocked") {
        return None;
    }
    let summary = report
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let question = report
        .get("question")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|question| !question.is_empty() && *question != summary);
    // Review N2: an empty block never reads "blocked: ." (`check_report` asks for a summary, so this is unlikely).
    Some(match question {
        Some(question) if summary.is_empty() => question.to_owned(),
        Some(question) => format!("{summary} ({question})"),
        None if summary.is_empty() => "no reason given".to_owned(),
        None => summary.to_owned(),
    })
}

/// The question a gated blocked delivery asks: the block first (the wake's line keeps 120 characters), then what the
/// gate found in what the worker did deliver.
fn blocked_question(order: &str, block: &str, axes: &[Axis]) -> String {
    let found: Vec<String> = failing(axes)
        .iter()
        .filter(|axis| axis.axis != "report")
        .map(|axis| format!("{}: {}", axis.axis, axis.why))
        .collect();
    let invented = invented_videos(axes).len();
    let gate = match (found.is_empty(), invented) {
        (true, _) => "the gate finds nothing else wrong in what it delivered".to_owned(),
        (false, 0) => format!("the gate also finds {}", found.join("; ")),
        (false, n) => format!(
            "the gate also finds {} ({n} cited video(s) do not exist)",
            found.join("; ")
        ),
    };
    format!(
        "{order} is blocked: {block}. {gate}. herdr redoes no blocked order: answer the block, change the order, or drop it?"
    )
}

/// A child's brief section that states the amended gate lines (plan 43 A1).
fn amended_section(spec: &Spec) -> String {
    let mut text = String::from(
        "\n## The gate lines as amended by the orchestrator (`herdr gate amend`)\n\nThese replace the original's \
`owns:` and `test:` lines.\n\n",
    );
    for pattern in &spec.owns {
        text.push_str(&format!("- `owns: {pattern}`\n"));
    }
    if let Some(test) = &spec.test {
        text.push_str(&format!("- `test: {test}`\n"));
    }
    text
}

/// Plan 43 A1: the `herdr gate amend` line a refusal with no worker fault names: the order, then each failing axis's
/// flags (`Axis::amend`), each once. A gate fault has no flag: the same line re-gates the branch as it is.
pub fn amend_line(order: &str, axes: &[Axis]) -> String {
    let mut line = format!("herdr gate amend {order}");
    let mut seen: Vec<&str> = Vec::new();
    for flags in failing(axes)
        .iter()
        .filter_map(|axis| axis.amend.as_deref())
    {
        if !seen.contains(&flags) {
            seen.push(flags);
            line.push(' ');
            line.push_str(flags);
        }
    }
    line
}

/// Plan 43 A1: a refusal no worker can fix. Journaled `gated amend` with every axis and its fault, and no child; the
/// reason opens with the exact `herdr gate amend` line, so the wake (`<order> gated amend: <reason>`) carries it. Megajam
/// run ia1: the orchestrator stopped the send-back child 26 times and merged the parent by hand 11 times.
fn amend_refusal(
    project: &Project,
    run: &str,
    journal: &Journal,
    order: &str,
    axes: Vec<Axis>,
) -> Result<Ruling, String> {
    let line = amend_line(order, &axes);
    let faults: Vec<String> = failing(&axes)
        .iter()
        .map(|axis| format!("{} ({})", axis.axis, fault_of(axis).word()))
        .collect();
    let reason = format!("{line}; {}", reason_line(&axes).unwrap_or_default());
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.to_owned(),
                verdict: Outcome::Amend.word().into(),
                reason: Some(reason),
                axes: axes_values(&axes),
            },
        )
        .map_err(|err| err.to_string())?;
    record(project, run, order);
    Ok(Ruling {
        order: order.to_owned(),
        run: run.to_owned(),
        verdict: Outcome::Amend.word().into(),
        axes,
        next: vec![
            format!(
                "{line}: no worker was seated, since every failing axis is the brief's or the gate's: {}",
                faults.join(", ")
            ),
            format!("or seat a worker for it anyway: herdr gate amend {order} --send-back"),
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn send_back(
    project: &Project,
    run: &str,
    journal: &Journal,
    state: &nirvana_run::RunState,
    known: &OrderState,
    root: &str,
    root_text: &str,
    axes: Vec<Axis>,
    table: Option<&Table>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    let block = block_reason(known);
    // Plan 37b: a blocked order's gated line names its block, whichever axis failed first. Review N3: as `blocked:`
    // and not the report axis's `report: the worker reported blocked: `, whose 37 characters left the block 53 of the
    // line's 90.
    let reason = block
        .as_ref()
        .map(|block| format!("blocked: {block}"))
        .or_else(|| reason_line(&axes));
    let ask = |question: String, axes: Vec<Axis>| -> Result<Ruling, String> {
        journal
            .transact(now(), |_| {
                Ok((
                    vec![
                        Kind::Gated {
                            order: order.clone(),
                            verdict: Outcome::Asked.word().into(),
                            reason: reason.clone(),
                            axes: axes_values(&axes),
                        },
                        Kind::Asked {
                            order: Some(order.clone()),
                            question: question.clone(),
                        },
                    ],
                    (),
                ))
            })
            .map_err(|err| err.to_string())?;
        record(project, run, &order);
        Ok(Ruling {
            order: order.clone(),
            run: run.to_owned(),
            verdict: Outcome::Asked.word().into(),
            axes,
            next: vec![question],
        })
    };
    // Plan 37b (the owner, 2026-11-01: "send it blocked as well for validationg"): a blocked delivery is gated for
    // what it did deliver, and then asked, never sent back. A send-back is the same order again, on the rule's seat or
    // one up, with "fix what the gate found": it pays a redo of a worker that said it cannot go on, and the block's
    // question goes unanswered. When the gate also fails another axis (invented sources, an unowned file) the block
    // still wins, and the question names what the gate found beside it.
    if let Some(block) = &block {
        return ask(blocked_question(&order, block, &axes), axes);
    }
    let (id, n) = match plan_send_back(state, &order) {
        SendBack::Ask => {
            return ask(
                format!(
                    "{order}: {SEND_BACK_CAP} send-backs are spent and the gate still finds {}; take it over, change the order, or drop it?",
                    reason.clone().unwrap_or_default()
                ),
                axes,
            )
        }
        SendBack::Child { id, n } => (id, n),
    };
    let Some(table) = table else {
        return ask(
            format!("{order} must go back, but seats.toml is refused"),
            axes,
        );
    };
    let seated = &known.seated;
    let mut brief = send_back_brief(&order, root, n, &axes, root_text, &integration_branch(run));
    // Plan 43 A1: the gate holds the child to the amended lines, so its worker reads them too.
    let amends = read_sidecar(project, run, root).amends;
    if !amends.is_empty() {
        brief.push_str(&amended_section(&amend_spec(spec(root_text), &amends)));
    }
    // Plan 31 review S1: the child never inherits the approval; it keeps the device line, and hears the parent may
    // have sent.
    let brief = crate::nirvana_gui_tools::child_brief_lines(&brief, known.seated.send_ok.is_some());
    let mut request = Request::new(&seated.class);
    request.size = seated.size;
    // Plan 29 §5.2: an order seated for a project MCP server goes back on a claude seat or not at all.
    let needs = match crate::nirvana_gui_tools::seat_needs(table, &project.root, &seated.needs) {
        Ok(needs) => needs,
        Err(why) => return ask(format!("{order} must go back, but {why}"), axes),
    };
    (request.needs, request.claude_only) = (needs.rest, needs.claude_only);
    request.not_family = seated.not_family.clone();
    request.mode = seated.mode;
    request.order = Some(id.clone());
    // Live checks 2026-11-01 F3: the redo is picked at the effort its parent ran at, the caller's `--effort` included,
    // so the rule prices its windows and its pressure check at that effort (review S6).
    request.effort = Some(seated.effort);
    // Plan 33 §2.2: "A send-back child and a reseat keep the parent's profile copy", and so its narrowing.
    if let Err(why) =
        crate::nirvana_profiles::narrow_for_order(&mut request, &project.run_dir(run), root_text)
    {
        return ask(format!("{order} must go back, but {why}"), axes);
    }
    if let Some(reason) = &seated.override_reason {
        request.override_seat = Some((
            format!("{}:{}", seated.seat, seated.effort.word()),
            reason.clone(),
        ));
        request.inherited_override = true;
    }
    // Plan 37 §1.3 (L5): a cited video that does not exist steps the redo up one seat. herdr's re-pick put
    // reel-motion-gemini's redo on the same flash-low seat that invented its sources (MotoProject, 2026-10-31). The
    // step is herdr's, so it counts against no run's overrides, and a later send-back of the child keeps it.
    let invented = invented_videos(&axes);
    let step_up = (!invented.is_empty())
        .then(|| nirvana_seat::step_up(table, &seated.class, &seated.seat, now()))
        .flatten();
    let step_reason = step_up.as_ref().map(|up| {
        format!(
            "step up from {} to {up}: {} cited video(s) do not exist (plan 37 §1.3)",
            seated.seat,
            invented.len()
        )
    });
    if let (Some(up), Some(reason)) = (&step_up, &step_reason) {
        request.override_seat = Some((up.clone(), reason.clone()));
        request.inherited_override = true;
    }
    // Review S1: a delivery with invented sources is never redone on the seat that invented them, nor on the rule's
    // pick (for research.web the cheap seat that invented reel-motion-gemini's 12). With no seat above it, or the one
    // above refused, the owner is asked: keeping the order's own seat would hand the redo to the inventor.
    let invented_ask = |why: String| {
        format!(
            "{order} must go back: {} cited video(s) do not exist, and {why}; herdr redoes invented sources neither on \
the seat that invented them nor on the rule's pick. Take it over, wait for a stronger seat, or seat the redo by hand \
with `herdr seat run`?",
            invented.len()
        )
    };
    if !invented.is_empty() && step_up.is_none() {
        return ask(
            invented_ask(format!(
                "no seat of {} stands above {}",
                seated.class, seated.seat
            )),
            axes,
        );
    }
    let inherited_reason = step_reason.clone().or(seated.override_reason.clone());
    let mut world = nirvana_seat::live_world(table);
    let leases_path = nirvana_seat::dir().join(nirvana_seat::LEASES_FILE);
    if let Ok(leases) = nirvana_seat::load_leases(&leases_path) {
        world.leases = leases.leases.clone();
        world.overrides_used = leases.overrides(run);
    }
    let (pick, why_seated, child_override_reason) =
        match nirvana_seat::pick(table, &world, &request) {
            Ok(pick) => (pick, None, inherited_reason),
            Err(no_seat) if !invented.is_empty() => {
                return ask(
                    invented_ask(format!(
                        "the seat above it, {}, is refused: {}",
                        step_up.as_deref().unwrap_or_default(),
                        no_seat.reason
                    )),
                    axes,
                )
            }
            Err(no_seat) if request.inherited_override => {
                let mut fallback_request = request.clone();
                fallback_request.override_seat = None;
                fallback_request.inherited_override = false;
                match nirvana_seat::pick(table, &world, &fallback_request) {
                    Ok(pick) => {
                        let seat_name = &seated.seat;
                        let why = format!(
                            "inherited override {} refused: {}; the rule picked",
                            seat_name, no_seat.reason
                        );
                        (pick, Some(why), None)
                    }
                    Err(err) => {
                        return ask(
                            format!("{order} must go back, and no seat is left: {}", err.reason),
                            axes,
                        );
                    }
                }
            }
            Err(no_seat) => {
                return ask(
                    format!(
                        "{order} must go back, and no seat is left: {}",
                        no_seat.reason
                    ),
                    axes,
                )
            }
        };
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    let cwd = PathBuf::from(&seated.cwd);
    let order_file = crate::nirvana_worker::write_order(project, &cwd, &id, &brief)
        .map_err(|err| err.to_string())?;
    let child = Seated {
        order: id.clone(),
        seat: pick.seat.clone(),
        cli: pick.cli.clone(),
        model: pick.model.clone(),
        effort: pick.effort,
        pool: pick.pool.clone(),
        class: seated.class.clone(),
        chain: Some(pick.chain.clone()),
        size: seated.size,
        mode: seated.mode,
        needs: seated.needs.clone(),
        not_family: seated.not_family.clone(),
        reason: why_seated.unwrap_or_else(|| pick.reason.clone()),
        override_reason: child_override_reason,
        order_file,
        cwd: seated.cwd.clone(),
        worktree: seated.worktree.clone(),
        branch: seated.branch.clone(),
        parent: Some(order.clone()),
        why: Some("send_back".into()),
        rule_pick: None,
        bar_dropped: Some(pick.bar_dropped),
        step_down: Some(pick.step_down),
        use_basis: pick.use_basis.clone(),
        expected: pick
            .windows
            .iter()
            .map(|w| (w.window.clone(), w.expected))
            .collect(),
        jev_pick: None,
        // Plan 31 review S1: a new Send needs a new `--send-ok` with a new quote.
        send_ok: None,
        // Plan 45 P45d: a child is seated with its parent's profile copy (plan 33 §2.2), and no fitting-profile line
        // is shown for it, so `profile_fit` is empty; both stay unknown when the parent's line predates the key.
        profile: seated.profile.clone(),
        profile_fit: seated.profile.as_ref().map(|_| Vec::new()),
    };
    journal
        .transact(now(), |_| {
            Ok((
                vec![
                    Kind::Gated {
                        order: order.clone(),
                        verdict: Outcome::SentBack.word().into(),
                        reason: reason.clone(),
                        axes: axes_values(&axes),
                    },
                    Kind::Seated(Box::new(child.clone())),
                ],
                (),
            ))
        })
        .map_err(|err| err.to_string())?;
    record(project, run, &order);
    let how = crate::nirvana_worker::launch_worker(project, run, &child).unwrap_or_else(|err| err);
    Ok(Ruling {
        order: order.clone(),
        run: run.to_owned(),
        verdict: Outcome::SentBack.word().into(),
        axes,
        next: vec![format!(
            "{id} seated on {} {} ({how}): send-back {n} of {SEND_BACK_CAP}",
            pick.seat,
            pick.effort.word()
        )],
    })
}

enum ReviewState {
    None,
    Pending(String),
    Verdict(String, Value),
}

fn review_state(state: &nirvana_run::RunState, sidecar: &Sidecar) -> ReviewState {
    let Some(id) = sidecar.reviews.last() else {
        return ReviewState::None;
    };
    match state.order(id) {
        Some(review) => match (&review.report, review.phase) {
            (Some(report), Phase::Delivered | Phase::Gated | Phase::Asked) => {
                ReviewState::Verdict(id.clone(), report.clone())
            }
            (_, Phase::Failed | Phase::Interrupted | Phase::Reseated) => ReviewState::None,
            _ => ReviewState::Pending(id.clone()),
        },
        None => ReviewState::Pending(id.clone()),
    }
}

/// The reviewer's Work Report as an axis: `done` approves, `blocked` sends back with its gotchas, `needs_answer`
/// waits for the owner (a judgment herdr does not make).
pub fn review_axis(id: &str, report: &Value) -> Axis {
    let summary = report
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    match report.get("status").and_then(Value::as_str) {
        Some("done") => Axis::new("review", Mark::Pass, format!("{id} approves: {summary}")),
        Some("needs_answer") => Axis::new(
            "review",
            Mark::Wait,
            format!(
                "{id} cannot judge it without the owner: {}",
                report
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or(&summary)
            ),
        ),
        _ => Axis::new(
            "review",
            Mark::Fail,
            format!("{id} sends it back: {summary}"),
        )
        .with(
            report
                .get("gotchas")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn request_review(
    project: &Project,
    run: &str,
    known: &OrderState,
    root_text: &str,
    det: &Det,
    sidecar: &mut Sidecar,
    table: Option<&Table>,
) -> Result<String, String> {
    let table = table.ok_or("seats.toml is refused: no reviewer can be picked")?;
    let order = known.id();
    let family = family_of(Some(table), &known.seated.seat);
    let id = format!("{order}-rev{}", sidecar.reviews.len() + 1);
    let world = nirvana_seat::live_world(table);
    let (pick, same) = pick_other_family(
        table,
        &world,
        "review.adversary",
        &family,
        known.seated.mode,
        &id,
        "SAME-FAMILY REVIEW",
    )
    .map_err(|no_seat| format!("no reviewer: {}", no_seat.reason))?;
    let diff_path = gate_dir(project, run).join(format!("{id}.diff"));
    let diff = match (&det.base, &det.head) {
        (Some(base), Some(head)) => {
            git(&project.root, &["diff", "--no-color", base, head]).unwrap_or_default()
        }
        _ => git(
            Path::new(&known.seated.cwd),
            &["diff", "--no-color", "HEAD"],
        )
        .unwrap_or_default(),
    };
    std::fs::create_dir_all(gate_dir(project, run)).map_err(|err| err.to_string())?;
    std::fs::write(&diff_path, diff).map_err(|err| err.to_string())?;
    let text = review_brief(
        order,
        root_text,
        &known.seated.cwd,
        &diff_path.display().to_string(),
        same,
    );
    let file = write_order_file(project, &id, &text).map_err(|err| err.to_string())?;
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    sidecar.reviews.push(id.clone());
    write_sidecar(
        project,
        run,
        &Sidecar {
            order: id.clone(),
            helps: Some(("review".into(), order.to_owned())),
            ..Sidecar::default()
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(seat_run_line(
        &pick,
        &file,
        &id,
        run,
        "review.adversary",
        (!same).then_some(family.as_str()),
    ))
}

/// The `herdr seat run` line of a reviewer booked and never seated, from its booking; without a live booking, how to
/// seat it by hand.
fn booked_review_line(
    project: &Project,
    run: &str,
    known: &OrderState,
    id: &str,
    table: Option<&Table>,
) -> String {
    let file = project.orders_dir().join(format!("{id}.md"));
    let leases = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE));
    let Some(lease) = leases
        .ok()
        .and_then(|leases| leases.live(id, now()).cloned())
    else {
        return format!(
            "{id}'s booking is gone: `herdr seat pick --class review.adversary --not-family {} --book {id} --run {run}`, \
then `herdr seat run <seat> --order \"{}\" --order-id {id} --run {run} --class review.adversary`",
            family_of(table, &known.seated.seat),
            file.display()
        );
    };
    let family = family_of(table, &known.seated.seat);
    let same = family_of(table, &lease.seat) == family;
    format!(
        "herdr seat run {} --order \"{}\" --order-id {id} --run {run} --class review.adversary{}",
        lease.seat,
        file.display(),
        if same {
            String::new()
        } else {
            format!(" --not-family {family}")
        }
    )
}

/// Plan 37 §1.1 (L3): which deliveries `seat exec` gates itself, so that gating costs Nirvana Claude no turn. Every
/// `done` delivery but a helper's (a reviewer of another family, a held-out author): gating one of those starts its
/// subject's next step, which the orchestrator sequences. A lesson review is gated whatever it reported (plan 33
/// review B2). MotoProject, 2026-10-23 to 31: 1 of 26 deliveries was gated, and an invented source went unread.
/// Plan 37b: a `blocked` delivery too (the owner, 2026-11-01: "send it blocked as well for validationg"); its gate
/// asks and never sends back (`send_back`). A `needs_answer` one is `asked` already, which the gate does not rule on.
pub fn gates_on_delivery(
    project: &Project,
    run: &str,
    order: &str,
    class: &str,
    status: &str,
) -> bool {
    class == crate::nirvana_review::CLASS
        || (matches!(status, "done" | "blocked")
            && helps_of(project, run, &replay(&project.journal(run).read().0), order).is_none())
}

/// Whom an order helps, `(kind, subject)`: its own sidecar's, else the parent's it was re-seated or sent back from
/// (review N7: a reviewer re-seated when its pool ran out has no sidecar of its own, and was gated as a plain order).
pub fn helps_of(
    project: &Project,
    run: &str,
    state: &nirvana_run::RunState,
    order: &str,
) -> Option<(String, String)> {
    let mut current = order.to_owned();
    // A chain is finite, but a journal written by hand could loop; no real chain is near this long.
    for _ in 0..64 {
        if let Some(helps) = read_sidecar(project, run, &current).helps {
            return Some(helps);
        }
        let seated = &state.order(&current)?.seated;
        match (&seated.parent, seated.why.as_deref()) {
            (Some(parent), Some("send_back" | "reseat")) => current = parent.clone(),
            _ => return None,
        }
    }
    None
}

/// A reviewer's own gate: its report is well formed. Its verdict is its subject's review axis.
fn gate_reviewer(journal: &Journal, known: &OrderState, run: &str) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    if known.verdict.is_some() {
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: "pass".into(),
            axes: Vec::new(),
            next: Vec::new(),
        });
    }
    // A reviewer that needs the owner is `asked` by O3, with its report kept: its subject's review axis waits.
    if !matches!(known.phase, Phase::Delivered | Phase::Asked) {
        return Err(format!("the reviewer {order} is {}", known.phase.word()));
    }
    let axis = report_or_verdict(known.report.as_ref());
    let verdict = if axis.mark == Mark::Fail {
        Outcome::Failed
    } else {
        Outcome::Pass
    };
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.clone(),
                verdict: verdict.word().into(),
                reason: (axis.mark == Mark::Fail).then(|| format!("report: {}", axis.why)),
                axes: axes_values(std::slice::from_ref(&axis)),
            },
        )
        .map_err(|err| err.to_string())?;
    Ok(Ruling {
        order,
        run: run.to_owned(),
        verdict: verdict.word().into(),
        axes: vec![axis],
        next: Vec::new(),
    })
}

/// A reviewer's or author's report need only be well formed: `blocked` is a review's verdict, not a failure.
pub(crate) fn report_or_verdict(report: Option<&Value>) -> Axis {
    match report.map(crate::nirvana_worker::check_report) {
        Some(Ok(())) => Axis::new("report", Mark::Pass, "well formed"),
        Some(Err(why)) => Axis::new("report", Mark::Fail, why),
        None => Axis::new("report", Mark::Fail, "no Work Report was delivered"),
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    let mut copied = 0;
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copied += copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// Gating a held-out author's order: capture its check beside the journal, take it out of the author's folder,
/// and judge it at the base (decision 0019 §2.3). Invalid: one retry with the reason; invalid twice: the build goes
/// on without one, said in every gate of it.
fn gate_author(
    project: &Project,
    run: &str,
    journal: &Journal,
    known: &OrderState,
    subject: &str,
    table: Option<&Table>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    if known.phase != Phase::Delivered {
        return Err(format!(
            "the held-out author {order} is {}",
            known.phase.word()
        ));
    }
    let cwd = PathBuf::from(&known.seated.cwd);
    let found = cwd.join(nirvana_run::DIR).join("heldout");
    let captured = project.run_dir(run).join("heldout").join(subject);
    let mut axes = vec![report_or_verdict(known.report.as_ref())];
    let judged: Result<Heldout, String> = (|| {
        if !found.is_dir() {
            return Err("the author wrote no .nirvana/heldout/ folder".to_owned());
        }
        let _ = std::fs::remove_dir_all(&captured);
        copy_dir(&found, &captured).map_err(|err| err.to_string())?;
        std::fs::remove_dir_all(&found).map_err(|err| err.to_string())?;
        let changed = uncommitted(&cwd)?;
        if !changed.is_empty() {
            return Err(format!(
                "the author changed repository files: {}",
                changed.join(", ")
            ));
        }
        let command = std::fs::read_to_string(captured.join("COMMAND"))
            .map_err(|_| "the check has no COMMAND file".to_owned())?
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .to_owned();
        if !command.contains("{dir}") {
            return Err("COMMAND does not name {dir}".to_owned());
        }
        let line = command.replace("{dir}", &captured.display().to_string());
        let result = run_command(&cwd, &line, Duration::from_secs(TEST_TIMEOUT_S));
        let base_cases = cases(&result.output);
        heldout_valid(&base_cases)?;
        Ok(Heldout {
            author: order.clone(),
            dir: captured.display().to_string(),
            command,
            base_cases,
        })
    })();
    let mut subject_card = read_sidecar(project, run, subject);
    let mut next = Vec::new();
    let verdict = match judged {
        Ok(heldout) => {
            let failing = heldout.base_cases.iter().filter(|(_, pass)| !pass).count();
            axes.push(Axis::new(
                "heldout",
                Mark::Pass,
                format!(
                    "{} case(s), {failing} failing at the base: valid",
                    heldout.base_cases.len()
                ),
            ));
            subject_card.heldout = Some(heldout);
            next.push(format!(
                "the build may start: herdr seat run ... --order-id {subject} --run {run}"
            ));
            Outcome::Pass
        }
        Err(why) => {
            axes.push(Axis::new("heldout", Mark::Fail, why.clone()));
            if subject_card.authors.len() < AUTHOR_TRIES {
                // `author` writes the subject's card itself (the new author's id); read it back after.
                let retried = author(project, run, subject, &why, table);
                subject_card = read_sidecar(project, run, subject);
                match retried {
                    Ok(line) => next.push(line),
                    Err(err) => {
                        subject_card.heldout_skipped = Some(format!(
                            "no held-out check: {why}; the retry could not be booked: {err}"
                        ));
                    }
                }
            } else {
                subject_card.heldout_skipped = Some(format!(
                    "no held-out check: refused twice, last because {why}"
                ));
                next.push(format!("the build goes on without a held-out check: {why}"));
            }
            Outcome::Failed
        }
    };
    write_sidecar(project, run, &subject_card).map_err(|err| err.to_string())?;
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.clone(),
                verdict: verdict.word().into(),
                reason: reason_line(&axes),
                axes: axes_values(&axes),
            },
        )
        .map_err(|err| err.to_string())?;
    record(project, run, &order);
    let _ = crate::nirvana_worker::cli(&["cleanup".to_owned(), order.clone()]);
    Ok(Ruling {
        order,
        run: run.to_owned(),
        verdict: verdict.word().into(),
        axes,
        next,
    })
}

/// `herdr gate author <order>`: book and write the held-out author's order for a builder that is booked and not
/// started (`herdr seat pick --book <order>` first: the author's family must differ from the builder's seat).
fn author(
    project: &Project,
    run: &str,
    subject: &str,
    retry: &str,
    table: Option<&Table>,
) -> Result<String, String> {
    let table = table.ok_or("seats.toml is refused")?;
    let mut card = read_sidecar(project, run, subject);
    let request = card_request(project, subject)?;
    let lease = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))?
        .live(subject, now())
        .cloned()
        .ok_or(format!("{subject} has no booking: book the builder first, `herdr seat pick --class <c> --book {subject} --run {run}`"))?;
    let family = lease
        .pick
        .get("family")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| family_of(Some(table), &lease.seat));
    let id = format!("{subject}-acc{}", card.authors.len() + 1);
    let world = nirvana_seat::live_world(table);
    let (pick, same) = pick_other_family(
        table,
        &world,
        "acceptance.author",
        &family,
        table.mode,
        &id,
        "SAME-FAMILY AUTHOR",
    )
    .map_err(|no_seat| format!("no author: {}", no_seat.reason))?;
    let text = author_brief(subject, &request, (!retry.is_empty()).then_some(retry));
    let file = write_order_file(project, &id, &text).map_err(|err| err.to_string())?;
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    card.authors.push(id.clone());
    write_sidecar(project, run, &card).map_err(|err| err.to_string())?;
    write_sidecar(
        project,
        run,
        &Sidecar {
            order: id.clone(),
            helps: Some(("author".into(), subject.to_owned())),
            ..Sidecar::default()
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(format!(
        "{} --worktree",
        seat_run_line(
            &pick,
            &file,
            &id,
            run,
            "acceptance.author",
            (!same).then_some(family.as_str())
        )
    ))
}

/// The builder's order text for its author: the file `gate author --order` was given, kept beside the sidecars.
fn card_request(project: &Project, subject: &str) -> Result<String, String> {
    let path = project.orders_dir().join(format!("{subject}.request.md"));
    std::fs::read_to_string(&path)
        .map_err(|_| format!("no request for {subject}: pass --order <file>"))
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr gate run <order> [--run r] [--json]       rule on a delivered order (exit 0 pass, 1 sent back
                                                         or asked, 4 waiting for a review or a held-out check)
       herdr gate author <order> --order <file> [--run r]  book the held-out author for a booked builder
       herdr gate merge <order> [--run r]                 merge a passed order into nirvana-integrate/<run>
       herdr gate merge <order> --by-hand <reason> [--sha s] [--run r]
                                                         record a merge the orchestrator made itself
       herdr gate amend <order> [--owns <glob>]... [--test <cmd>] [--send-back] [--run r] [--json]
                                                         amend the brief's owns/test and re-gate the same branch
       herdr gate status [<order>] [--run r] [--json]";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

#[derive(Default)]
struct Args {
    positional: Option<String>,
    run: Option<String>,
    order_file: Option<PathBuf>,
    json: bool,
    /// Plan 43: `gate amend`'s flags.
    owns: Vec<String>,
    test: Option<String>,
    send_back: bool,
    /// Plan 43: `gate merge --by-hand <reason> [--sha <sha>]`.
    by_hand: Option<String>,
    sha: Option<String>,
}

fn parse_args(args: &[String]) -> Args {
    let mut parsed = Args::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--run" => parsed.run = Some(value()),
            "--order" => parsed.order_file = Some(PathBuf::from(value())),
            "--owns" => parsed.owns.push(value()),
            "--test" => parsed.test = Some(value()),
            "--send-back" => parsed.send_back = true,
            "--by-hand" => parsed.by_hand = Some(value()),
            "--sha" => parsed.sha = Some(value()),
            "--json" => parsed.json = true,
            other if !other.starts_with("--") && parsed.positional.is_none() => {
                parsed.positional = Some(other.to_owned())
            }
            _ => usage_exit(),
        }
    }
    parsed
}

fn run_for(project: &Project, order: &str, run: Option<String>) -> String {
    if !nirvana_run::valid_id(order) {
        eprintln!("herdr gate: {order:?} is not an order id");
        std::process::exit(2);
    }
    match run.or_else(|| project.run_of(order)) {
        Some(run) if nirvana_run::valid_id(&run) => run,
        _ => {
            eprintln!("herdr gate: no run seats {order}");
            std::process::exit(1);
        }
    }
}

fn print_ruling(ruling: &Ruling, json: bool) -> std::io::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(ruling)?);
        return Ok(());
    }
    println!("{} · {}", ruling.order, ruling.verdict);
    for axis in &ruling.axes {
        let mark = match axis.mark {
            Mark::Pass => "pass",
            Mark::Fail => "FAIL",
            Mark::Note => "note",
            Mark::Wait => "wait",
        };
        println!("  {mark} {}: {}", axis.axis, axis.why);
        for line in axis.lines.iter().take(12) {
            println!("       {line}");
        }
    }
    for line in &ruling.next {
        println!("next: {line}");
    }
    Ok(())
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let rest = args.get(1..).unwrap_or(&[]);
    let parsed = parse_args(rest);
    match args.first().map(String::as_str) {
        Some("run") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let run = run_for(&project, &order, parsed.run);
            match gate(&project, &run, &order) {
                Ok(ruling) => {
                    print_ruling(&ruling, parsed.json)?;
                    std::process::exit(ruling.exit());
                }
                Err(why) => {
                    eprintln!("herdr gate run: {why}");
                    std::process::exit(2);
                }
            }
        }
        Some("author") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let Some(file) = parsed.order_file else {
                usage_exit()
            };
            let run = parsed
                .run
                .or_else(|| std::env::var("NIRVANA_RUN").ok())
                .unwrap_or_else(|| usage_exit());
            if !nirvana_run::valid_id(&order) || !nirvana_run::valid_id(&run) {
                usage_exit();
            }
            let text = std::fs::read_to_string(&file)?;
            project.ensure()?;
            std::fs::create_dir_all(project.orders_dir())?;
            std::fs::write(
                project.orders_dir().join(format!("{order}.request.md")),
                &text,
            )?;
            let table = nirvana_seat::table_for(&project.root).ok();
            match author(&project, &run, &order, "", table.as_ref()) {
                Ok(line) => {
                    println!("{line}");
                    println!(
                        "then, when it has delivered: herdr gate run {order}-acc1 --run {run}"
                    );
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr gate author: {why}");
                    std::process::exit(3);
                }
            }
        }
        Some("amend") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            if parsed.by_hand.is_some() || parsed.sha.is_some() {
                usage_exit();
            }
            let run = run_for(&project, &order, parsed.run);
            match gate_amend(
                &project,
                &run,
                &order,
                &parsed.owns,
                parsed.test.as_deref(),
                parsed.send_back,
            ) {
                Ok(ruling) => {
                    print_ruling(&ruling, parsed.json)?;
                    std::process::exit(ruling.exit());
                }
                Err(why) => {
                    eprintln!("herdr gate amend: {why}");
                    std::process::exit(REFUSED_EXIT);
                }
            }
        }
        // Fix round G2: `--by-hand` takes none of `gate amend`'s flags, as `gate merge` takes none, and it exits 2 on a
        // refusal as `gate amend` does (a blank reason is a usage error, exit 2 too).
        Some("merge") if parsed.by_hand.is_some() => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let reason = parsed.by_hand.clone().unwrap_or_default();
            if !merge_flags_ok(&parsed) {
                usage_exit();
            }
            let run = run_for(&project, &order, parsed.run);
            match merge_by_hand(&project, &run, &order, &reason, parsed.sha.as_deref()) {
                Ok(line) => {
                    println!("{line}");
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr gate merge --by-hand: {why}");
                    std::process::exit(REFUSED_EXIT);
                }
            }
        }
        Some("merge") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            if !merge_flags_ok(&parsed) {
                usage_exit();
            }
            let run = run_for(&project, &order, parsed.run);
            match merge_cli(&project, &run, &order) {
                Ok(line) => {
                    println!("{line}");
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr gate merge: {why}");
                    std::process::exit(1);
                }
            }
        }
        Some("status") => status_cli(&project, parsed),
        _ => usage_exit(),
    }
}

/// Fix round G2: the exit of `gate amend` and `gate merge --by-hand` when herdr refuses the call: 2, the usage
/// errors' code too.
const REFUSED_EXIT: i32 = 2;

/// Fix round G2: `gate merge` takes none of `gate amend`'s flags (`--owns`, `--test`, `--send-back`), with or without
/// `--by-hand`; `--sha` only with `--by-hand`; and `--by-hand` needs a reason that is not blank.
fn merge_flags_ok(parsed: &Args) -> bool {
    let amend_flags = !parsed.owns.is_empty() || parsed.test.is_some() || parsed.send_back;
    match &parsed.by_hand {
        Some(reason) => !amend_flags && !reason.trim().is_empty(),
        None => !amend_flags && parsed.sha.is_none(),
    }
}

fn merge_cli(project: &Project, run: &str, order: &str) -> Result<String, String> {
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in {run}"))?;
    if known.phase == Phase::Merged {
        return Ok(format!(
            "{order} is merged already ({})",
            known.merged_sha.clone().unwrap_or_default()
        ));
    }
    if known.verdict.as_deref() != Some(Outcome::Pass.word()) {
        return Err(format!(
            "{order}'s gate verdict is {}: only a passed order merges",
            known.verdict.as_deref().unwrap_or("not given")
        ));
    }
    if known.seated.branch.is_none() {
        return Err(format!(
            "{order} ran in the project's folder: its work is there already, nothing to merge"
        ));
    }
    let det = read_sidecar(project, run, order)
        .last
        .ok_or("the gate kept no record of its trees")?;
    let (Some(base), Some(head)) = (det.base, det.head) else {
        return Err("the gate kept no base or head".into());
    };
    let cwd = PathBuf::from(&known.seated.cwd);
    if rev(&cwd, "HEAD").as_deref() != Some(head.as_str()) {
        return Err(format!(
            "{order}'s branch moved since the gate: gate it again"
        ));
    }
    let sha = merge(&project.root, run, order, &base, &head)?;
    journal
        .append(
            now(),
            Kind::Merged {
                order: order.to_owned(),
                sha: Some(sha.clone()),
                by_hand: None,
            },
        )
        .map_err(|err| err.to_string())?;
    drop(_lock);
    // O3's cleanup: the pane, then the worktree, then the booking; the chain's earlier orders shared the worktree.
    let (root, _) = chain(&state, order);
    let mut chain_orders = vec![order.to_owned()];
    let mut current = order.to_owned();
    while current != root {
        let Some(parent) = state
            .order(&current)
            .and_then(|known| known.seated.parent.clone())
        else {
            break;
        };
        chain_orders.push(parent.clone());
        current = parent;
    }
    for id in &chain_orders {
        let _ = crate::nirvana_worker::cli(&["cleanup".to_owned(), id.clone()]);
    }
    Ok(format!(
        "{order} merged into {} as {}; a merge to main is the owner's (plan 12 §6)",
        integration_branch(run),
        short_sha(&sha)
    ))
}

/// Plan 43 A1: `herdr gate amend <order> [--owns <glob>]... [--test <cmd>] [--send-back]`. Adds `owns:` patterns and
/// replaces the `test:` of the order's chain root, journaled with the old and new values as `gated amending` in the
/// transaction that writes the sidecar's `amends` (fix round F6), then re-gates the same delivered branch with no new
/// worker; the re-gate's `gated` event carries an `amend` note axis too. `--send-back` seats a child for the refusal
/// even when no axis is the worker's. With no flag it re-gates as is (a gate fault). It refuses an order gated asked
/// (the owner's answer is pending), one delivered and not gated yet, and one whose child is live. The send-back cap is
/// untouched: an amend is no send-back.
pub fn gate_amend(
    project: &Project,
    run: &str,
    order: &str,
    owns: &[String],
    test: Option<&str>,
    send_back: bool,
) -> Result<Ruling, String> {
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in run {run}"))?
        .clone();
    if owns_nothing(&known.seated.class) || helps_of(project, run, &state, order).is_some() {
        return Err(format!(
            "{order} is a helper or a review ({}): its gate has no `owns:` or `test:` to amend",
            known.seated.class
        ));
    }
    let verdict = known.verdict.as_deref().unwrap_or("");
    // Fix round F6 (review finding 6): an order asked waits on the owner; an amend would re-gate, and could pass, it
    // past the owner's answer.
    if known.phase == Phase::Gated && verdict == Outcome::Asked.word() {
        return Err(format!(
            "{order} is gated asked: the owner's answer is pending, and an amend would re-gate it past that answer"
        ));
    }
    // Fix round F6: a delivered order not gated yet is gated first (`herdr gate run`): the amend's own `gated
    // amending` event would be its first verdict, which takes the wake that its real verdict is owed.
    if known.phase == Phase::Delivered {
        return Err(format!(
            "{order} is delivered and not gated yet: gate it first with `herdr gate run {order}`"
        ));
    }
    let amendable = known.phase == Phase::Gated
        && (verdict == Outcome::AMENDING
            || [Outcome::Amend, Outcome::SentBack]
                .iter()
                .any(|outcome| outcome.word() == verdict));
    if !amendable {
        return Err(format!(
            "{order} is {}{}: an amend re-gates a delivered order the gate refused",
            known.phase.word(),
            known
                .verdict
                .as_deref()
                .map(|v| format!(" ({v})"))
                .unwrap_or_default()
        ));
    }
    // The re-gate runs in the worktree a live child works in: the child is stopped first.
    if let Some(child) = state
        .orders
        .iter()
        .find(|child| child.seated.parent.as_deref() == Some(order) && child.phase.wants_worker())
    {
        return Err(format!(
            "{} works on {order}'s branch now ({}): stop it first with `herdr seat stop {}`",
            child.id(),
            child.phase.word(),
            child.id()
        ));
    }
    // Fix round 2 (H8, review): a child that went on past its worker (delivered, gated, merged, asked or reseated) has
    // put its own work on this branch; re-gating the parent would rule on the child's work under the parent's name.
    // Only a child that ended with nothing usable (interrupted or failed, as `herdr seat stop` leaves it) lets the
    // parent be amended. A helper (a lesson review, `<order>-lr`) writes nothing on the branch and never counts.
    if let Some(child) = state.orders.iter().find(|child| {
        child.seated.parent.as_deref() == Some(order)
            && !owns_nothing(&child.seated.class)
            && !matches!(child.phase, Phase::Interrupted | Phase::Failed)
    }) {
        return Err(format!(
            "{} is {} already and its work is on {order}'s branch: amend {} instead",
            child.id(),
            child.phase.word(),
            child.id()
        ));
    }
    let (root, _) = chain(&state, order);
    let root_seated = state
        .order(&root)
        .map_or_else(|| known.seated.clone(), |root| root.seated.clone());
    let mut root_card = read_sidecar(project, run, &root);
    let before = amend_spec(spec(&order_text(&root_seated)), &root_card.amends);
    let mut added: Vec<String> = Vec::new();
    for pattern in owns.iter().map(|pattern| pattern.trim().replace('\\', "/")) {
        if !pattern.is_empty() && !before.owns.contains(&pattern) && !added.contains(&pattern) {
            added.push(pattern);
        }
    }
    let test = test
        .map(|test| test.trim().trim_matches('`').trim().to_owned())
        .filter(|test| !test.is_empty() && before.test.as_ref() != Some(test));
    let mut said = Vec::new();
    if !added.is_empty() {
        said.push(format!(
            "owns + {} (was {})",
            added.join(" "),
            if before.owns.is_empty() {
                "none".to_owned()
            } else {
                before.owns.join(" ")
            }
        ));
    }
    if let Some(test) = &test {
        said.push(format!(
            "test `{test}` (was {})",
            before
                .test
                .as_deref()
                .map_or("none".to_owned(), |old| format!("`{old}`"))
        ));
    }
    let note = Axis::new(
        "amend",
        Mark::Note,
        if said.is_empty() {
            format!("re-gated as it was, by `herdr gate amend {order}`")
        } else {
            format!("by `herdr gate amend {order}`: {}", said.join("; "))
        },
    );
    if !said.is_empty() {
        let old_card = root_card.clone();
        root_card.amends.push(Amend {
            at: now(),
            order: order.to_owned(),
            owns_added: added,
            owns_before: before.owns.clone(),
            test_before: test.as_ref().and(before.test.clone()),
            test,
        });
        // Fix round F6 (review finding 6): the first cut wrote the sidecar and then re-gated, so a re-gate that erred
        // (or a herdr that died in a long `test:`) left `owns:`/`test:` changed with no journal event. Now the amend is
        // a `gated amending` event, carrying the old and the new values, written in one journal transaction with the
        // sidecar: the sidecar is written inside it, and put back when the event is not written. The re-gate's verdict
        // follows; `herdr gate run` finishes an order left amending.
        let reason = format!("herdr gate amend {order}: {}", said.join("; "));
        let written = journal.transact(now(), |_| {
            write_sidecar(project, run, &root_card)?;
            Ok((
                vec![Kind::Gated {
                    order: order.to_owned(),
                    verdict: Outcome::AMENDING.into(),
                    reason: Some(reason.clone()),
                    axes: axes_values(std::slice::from_ref(&note)),
                }],
                (),
            ))
        });
        if let Err(err) = written {
            let _ = write_sidecar(project, run, &old_card);
            return Err(format!(
                "the amend was not journaled, and nothing changed: {err}"
            ));
        }
    }
    let mut notes = vec![note];
    if send_back {
        notes.push(Axis::new(
            "amend",
            Mark::Note,
            "--send-back: a refusal seats a worker whatever its fault",
        ));
    }
    let sidecar = read_sidecar(project, run, order);
    // Fix round 2 (H8): the re-gate erring (a herdr that dies in a long `test:`, a journal it cannot write), after the
    // amend was journaled.
    #[cfg(test)]
    if TEST_REGATE_FAILS.replace(false) {
        return Err("a test's re-gate failure".into());
    }
    let table = nirvana_seat::table_for(&project.root).ok();
    let state = replay(&journal.read().0);
    let known = state.order(order).cloned().unwrap_or(known);
    gate_subject(
        project,
        run,
        &journal,
        &state,
        &known,
        sidecar,
        table.as_ref(),
        if send_back {
            Route::Child
        } else {
            Route::ByFault
        },
        notes,
    )
}

/// Plan 43 A7 (C10): `herdr gate merge <order> --by-hand <reason> [--sha <sha>]`, the record of a merge the
/// orchestrator made itself. Megajam run ia1: 43 orders landed on its integration branch with no `merged` event, and 9
/// `merged` events had no sha. The commit is `--sha` when given, which must be on the integration branch; else the
/// first commit there with the order's branch head as a parent (the head itself when it is the tip). Journals a
/// `gated accepted` event whose reason is `by hand: <reason>` and whose `by_hand` axis names the sha, then `merged`
/// with the sha and the reason in its `by_hand` field. Nothing is cleaned up: the orchestrator's own merge may still
/// use the worktree.
pub fn merge_by_hand(
    project: &Project,
    run: &str,
    order: &str,
    reason: &str,
    sha: Option<&str>,
) -> Result<String, String> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err("--by-hand needs the reason the orchestrator merged it itself".into());
    }
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in {run}"))?
        .clone();
    if known.phase == Phase::Merged {
        return Ok(format!(
            "{order} is merged already ({})",
            known.merged_sha.clone().unwrap_or_default()
        ));
    }
    if known.phase.wants_worker() {
        return Err(format!(
            "{order} is {}: its worker may still change the work; stop it first",
            known.phase.word()
        ));
    }
    let branch = integration_branch(run);
    let tip = rev(&project.root, &format!("refs/heads/{branch}"))
        .ok_or(format!("no branch {branch}: nothing was merged into it"))?;
    let on_branch =
        |sha: &str| git(&project.root, &["merge-base", "--is-ancestor", sha, &tip]).is_ok();
    let landed = match sha {
        Some(sha) => {
            let full = rev(&project.root, sha).ok_or(format!("{sha} names no commit"))?;
            if !on_branch(&full) {
                return Err(format!("{} is not on {branch}", short_sha(&full)));
            }
            full
        }
        None => {
            let head = known
                .seated
                .branch
                .as_deref()
                .and_then(|name| {
                    rev(&project.root, &format!("refs/heads/{name}"))
                        .or_else(|| rev(Path::new(&known.seated.cwd), "HEAD"))
                })
                .ok_or(format!(
                    "{order} has no branch of its own: name the commit that landed it with --sha"
                ))?;
            if !on_branch(&head) {
                return Err(format!(
                    "{order}'s branch head {} is not on {branch} (a cherry-pick?): name the commit that landed it \
with --sha",
                    short_sha(&head)
                ));
            }
            landing_commit(&project.root, &head, &tip)
        }
    };
    let axis = Axis::new(
        "by_hand",
        Mark::Note,
        format!(
            "merged into {branch} by the orchestrator at {}: {reason}",
            short_sha(&landed)
        ),
    )
    .with(vec![landed.clone()]);
    journal
        .transact(now(), |_| {
            Ok((
                vec![
                    Kind::Gated {
                        order: order.to_owned(),
                        verdict: Outcome::ACCEPTED.into(),
                        reason: Some(format!("by hand: {reason}")),
                        axes: axes_values(std::slice::from_ref(&axis)),
                    },
                    Kind::Merged {
                        order: order.to_owned(),
                        sha: Some(landed.clone()),
                        by_hand: Some(reason.to_owned()),
                    },
                ],
                (),
            ))
        })
        .map_err(|err| err.to_string())?;
    record(project, run, order);
    Ok(format!(
        "{order} recorded as merged by hand into {branch} at {}: {reason}",
        short_sha(&landed)
    ))
}

/// The commit on `tip`'s history that took `head` in: the oldest with `head` as a parent; `head` itself when it is
/// the tip or no commit names it as a parent.
fn landing_commit(root: &Path, head: &str, tip: &str) -> String {
    git(
        root,
        &[
            "rev-list",
            "--ancestry-path",
            "--reverse",
            "--parents",
            &format!("{head}..{tip}"),
        ],
    )
    .ok()
    .and_then(|list| {
        list.lines().find_map(|line| {
            let mut words = line.split_whitespace();
            let commit = words.next()?;
            words
                .any(|parent| parent == head)
                .then(|| commit.to_owned())
        })
    })
    .unwrap_or_else(|| head.to_owned())
}

fn status_cli(project: &Project, args: Args) -> std::io::Result<()> {
    let runs = match (&args.run, &args.positional) {
        (Some(run), _) => vec![run.clone()],
        (None, Some(order)) => project.run_of(order).into_iter().collect(),
        (None, None) => project.runs(),
    };
    let mut rows = Vec::new();
    for run in &runs {
        let state = replay(&project.journal(run).read().0);
        for known in &state.orders {
            if args
                .positional
                .as_deref()
                .is_some_and(|order| order != known.id())
            {
                continue;
            }
            let card = read_sidecar(project, run, known.id());
            let (root, depth) = chain(&state, known.id());
            rows.push(serde_json::json!({
                "run": run,
                "order": known.id(),
                "phase": known.phase.word(),
                "verdict": known.verdict,
                "send_backs": depth,
                "root": root,
                "gates": card.gates,
                "reviews": card.reviews,
                "authors": card.authors,
                "heldout": card.heldout.as_ref().map(|h| h.base_cases.len()),
                "heldout_skipped": card.heldout_skipped,
                "helps": card.helps,
                "failing": card.last.as_ref().map(|det| failing(&det.axes).iter().map(|a| a.axis.clone()).collect::<Vec<_>>()),
            }));
        }
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if rows.is_empty() {
        println!("no order to show");
    }
    for row in rows {
        let text = |key: &str| row[key].as_str().unwrap_or("").to_owned();
        println!(
            "{} {} {} · gate {} · send-backs {} of {SEND_BACK_CAP}{}{}{}",
            text("run"),
            text("order"),
            text("phase"),
            row["verdict"].as_str().unwrap_or("not run"),
            row["send_backs"],
            row["reviews"]
                .as_array()
                .filter(|r| !r.is_empty())
                .map(|r| format!(
                    " · reviews {}",
                    r.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .unwrap_or_default(),
            row["heldout"]
                .as_u64()
                .map(|n| format!(" · held-out {n} case(s)"))
                .unwrap_or_default(),
            row["helps"]
                .as_array()
                .map(|h| format!(
                    " · {} for {}",
                    h[0].as_str().unwrap_or(""),
                    h[1].as_str().unwrap_or("")
                ))
                .unwrap_or_default(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
