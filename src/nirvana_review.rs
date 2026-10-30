//! Nirvana fork, plan 33 P33b: the lesson review. After `herdr gate run` journals a pass or a send-back, one order of
//! class `lesson-review` reads what happened and may keep a lesson; gating that order applies its `lesson_ops`.
//! `herdr lessons merge` and `herdr run end` queue the same class with a merge brief when a class holds more than 8
//! live lessons. (nirvana/plans/2026-10-30-33-profiles-and-learning.md §3.3, §3.4)
//!
//! The owner, 2026-10-30, verbatim: "Pay particular atention to the self learning after each run." (L2) and "Sure"
//! to a review after every gated order on a cheap seat with a daily cap (L3).
//!
//! - **A normal herdr order.** Seated by the seat rule at Efficient (agy Flash, then Sonnet low), booked, journaled
//!   `seated` and started the way a send-back child is (`nirvana_gate::send_back`): herdr's one launcher,
//!   `nirvana_worker::launch_worker`. Nothing here seats around the table. It never takes the reviewed order's
//!   profile: its scope is the subject's journaled seating (review B1).
//! - **No orchestrator turn** (review B2). `seat exec` gates a delivered review itself, since the gate is
//!   deterministic, and neither wakes Nirvana Claude nor journals a question; its delivery is no queue line. `settle`
//!   at `herdr run end` gates any review that delivered with no `seat exec` to gate it.
//! - **Contained** (review B3). It works in a folder of its own under the run (`review_cwd`), with a copy of the
//!   stream; a claude reviewer is denied every tool that writes or runs (`DENIED`). Its gate fails it, applying
//!   nothing, when `.nirvana/lessons/` changed since it was queued in a way herdr's own writes do not explain.
//! - **herdr applies the ops, the reviewer writes nothing.** An op whose evidence is not in the reviewed worker's
//!   own stream, is shorter than `MIN_EVIDENCE`, or is text herdr wrote (the order, the brief, a lesson) is refused;
//!   so is a patch whose `base_sha` is not the lesson's body sha now (read before write), an op outside the reviewed
//!   order's class and profile, and any op this kind of review does not make. Ops apply only on a `done` report. The
//!   reviewer's own `lesson` is never kept.
//! - **A daily cap**, `lesson_reviews_per_day` per project (default 20). Over it the review is skipped, said in the
//!   `gated` event as an axis `review_skipped`. A review never reviews a review.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::nirvana_gate::{self, Axis, Mark, Outcome, Ruling, Sidecar};
use crate::nirvana_lessons::{self, Lesson};
use crate::nirvana_run::{self, now, replay, Journal, Kind, OrderState, Phase, Project, Seated};
use crate::nirvana_seat::{self, Mode, Request, SizeName, Table};

/// The class in `seats.toml`, and the sidecar's `helps` kinds of its two briefs.
pub const CLASS: &str = "lesson-review";
pub const REVIEW: &str = "lesson-review";
pub const MERGE: &str = "lesson-merge";
/// The axis a skipped review is journaled as, in the subject's `gated` event.
pub const SKIPPED: &str = "review_skipped";
/// §3.3: "default 20". A hypothesis: about one review per gated order on a busy day (the busiest day of
/// `3D-Printer Flow`'s journals gated 14 orders).
pub const PER_DAY: u32 = 20;
/// §3.4: "when a class holds more than 8 live lessons".
pub const MERGE_ABOVE: usize = 8;
/// A pending review older than this is dropped from `settle`'s list: its worker never delivered.
const PENDING_DAYS: u64 = 7;
const STATE: &str = ".lesson-reviews.json";
/// Plan 33 review B3: what a claude reviewer may not use, by Claude Code's own tool names (this machine's Claude Code
/// session lists them: `Bash`, `PowerShell` on Windows, `Edit`, `Write`, `NotebookEdit`, `Agent`, `Monitor`, ...).
/// A reviewer reads (`Read`, `Grep`, `Glob`) and answers. A deny list misses a tool Claude Code adds later; the
/// folder check in `gate_review` is the backstop.
pub const DENIED: &str =
    "Bash,PowerShell,Edit,Write,NotebookEdit,Agent,Monitor,WebFetch,WebSearch,EnterWorktree,CronCreate";

/// What the gate ruled on the reviewed order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    /// With each failing axis, `axis: why`.
    SentBack(Vec<String>),
}

// ---- the cap and the pending list ----

/// The table's `[constants] lesson_reviews_per_day`; `PER_DAY` when the table is refused.
pub fn per_day(table: Option<&Table>) -> u32 {
    table.map_or(PER_DAY, |table| table.constants.lesson_reviews_per_day)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    /// The UTC day `queued` counts, as days since 1970.
    #[serde(default)]
    day: u64,
    #[serde(default)]
    queued: u32,
    #[serde(default)]
    pending: Vec<Pending>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Pending {
    run: String,
    order: String,
    kind: String,
    subject: String,
    at: u64,
}

/// Under `runs/`, which O3's `.gitignore` ignores: the count is the machine's, never the project's history.
fn state_path(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("runs").join(STATE)
}

fn read_state(root: &Path) -> State {
    std::fs::read_to_string(state_path(root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

struct StateLock(std::fs::File);

impl Drop for StateLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn change_state<R>(root: &Path, change: impl FnOnce(&mut State) -> R) -> std::io::Result<R> {
    let path = state_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path.with_extension("lock"))?;
    file.lock()?;
    let _lock = StateLock(file);
    let mut state = read_state(root);
    let out = change(&mut state);
    let mut text = serde_json::to_string_pretty(&state)?;
    text.push('\n');
    nirvana_seat::write_atomic(&path, text.as_bytes())?;
    Ok(out)
}

/// Why no review is queued now, when the cap says so; `None` while it has room.
pub fn cap_refusal(root: &Path, cap: u32, at: u64) -> Option<String> {
    if cap == 0 {
        return Some("lesson reviews are off (lesson_reviews_per_day = 0)".into());
    }
    let state = read_state(root);
    let used = if state.day == at / 86_400 {
        state.queued
    } else {
        0
    };
    (used >= cap).then(|| {
        format!("{used} lesson reviews today reach the daily cap of {cap} (lesson_reviews_per_day)")
    })
}

/// The review's order id: the subject's, with `-lr`.
pub fn review_id(subject: &str) -> String {
    format!("{subject}-lr")
}

/// Whether the gate queues a review of `known` now. `Err(None)`: none, and nothing to say (it is queued already).
/// `Err(Some(why))`: skipped, journaled as `review_skipped`.
pub fn wanted(
    project: &Project,
    state: &nirvana_run::RunState,
    known: &OrderState,
    table: Option<&Table>,
) -> Result<(), Option<String>> {
    wanted_with(project, state, known, per_day(table), now())
}

pub fn wanted_with(
    project: &Project,
    state: &nirvana_run::RunState,
    known: &OrderState,
    cap: u32,
    at: u64,
) -> Result<(), Option<String>> {
    if known.seated.class == CLASS {
        return Err(Some("a review never reviews a review".into()));
    }
    let id = review_id(known.id());
    if state.order(&id).is_some() {
        return Err(None);
    }
    if !nirvana_run::valid_id(&id) {
        return Err(Some(format!("the review's id {id} is not an order id")));
    }
    match cap_refusal(&project.root, cap, at) {
        Some(why) => Err(Some(why)),
        None => Ok(()),
    }
}

// ---- the briefs ----

/// The rules every lesson review carries. Nirvana's own text, from Hermes' review prompts read on 2026-10-30
/// (`agent/background_review.py`: `_SKILL_REVIEW_PROMPT`, `_LESSON_LAYER_BLOCK`, `_DO_NOT_CAPTURE_BLOCK`), rewritten
/// for lessons. One departure: Hermes says "'Nothing to save.' ... should NOT be the default"; the plan (§3.3) makes
/// it the default, since a lesson costs a line in every brief of its class (decision 0023).
pub const RULES: &str = "## The rules

1. Nothing to keep is a valid answer, and the usual one. An order that went as its brief said teaches nothing new: \
answer with an empty `lesson_ops`.
2. Patch before you add. When a live lesson below covers the ground, patch it: its slug, and the base_sha shown beside \
it. A new lesson is for a kind of task no lesson covers.
3. The same lesson learned twice is one rule. Strengthen the lesson that says it; never write a second copy.
4. A step is a rule and one clause of why: what to do, then the mechanism that makes it matter. Not a story of this \
order: no order ids, dates or quoted chatter as content.
5. The strongest signal is a correction: the gate's send-back reasons below, and the owner's own words in the order. \
A lesson that would have saved a send-back is worth keeping.
6. Do not capture: a failure of this machine or its setup (a missing program, a sign-in, a path); a claim that a tool \
is broken or cannot be used; the story of one order; a failure that was never resolved, written up as a method. When \
the worker tried several things and none worked, keep nothing.
7. Each op's evidence is a quote, word for word, from the worker's own stream (named below): a command's output, a \
tool's result. herdr refuses an op whose evidence is not there; the Work Report's own words do not count.
8. Steps are at most 120 lines. Name a lesson by its kind of task, never by this order.
";

/// The merge rules: Nirvana's own text, from Hermes' curator prompt (`agent/curator.py` `CURATOR_REVIEW_PROMPT`:
/// "would a human maintainer write this as N separate skills, or as one skill with N labeled subsections?").
pub const MERGE_RULES: &str = "## The rules

1. Merge the lessons a maintainer would write as one lesson with labelled steps. Leave alone a lesson that stands \
apart; leaving all of them is a valid answer.
2. Distil, do not file: each merged step is a rule and one clause of why, the same lesson said twice becomes one rule, \
and the story of the order that taught it is dropped.
3. Name the umbrella by its kind of task, so its one line in a brief tells a worker when to open it.
4. Steps are at most 120 lines. herdr archives every merged-away lesson and can roll the merge back.
";

fn without_footer(text: &str) -> String {
    text.split("\n\n---\nFrom herdr")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_owned()
}

/// The reviewed order's copy as a review brief quotes it: no `Profile:` or `Profile sha256:` line and no agy profile
/// section, so nothing that reads a brief for a profile (`seat exec`, a reseat, the gate) finds the subject's in it
/// (review B1).
pub fn quoted_order(text: &str, profile: Option<&str>) -> String {
    let mut text = without_footer(text);
    let named = crate::nirvana_profiles::in_order(&text).and_then(Result::ok);
    for name in named.iter().map(|found| found.name.as_str()).chain(profile) {
        text = crate::nirvana_profiles::strip_sections(&text, name);
    }
    crate::nirvana_profiles::strip_profile_lines(&text)
        .trim()
        .to_owned()
}

/// The profile `order` was seated with, from its journaled `seated` event, else from its send-back chain root's.
pub fn subject_profile(state: &nirvana_run::RunState, order: &str) -> Option<String> {
    let own = state.order(order)?;
    if own.seated.class == CLASS {
        return None;
    }
    crate::nirvana_profiles::from_reason(&own.seated.reason).or_else(|| {
        let (root, _) = nirvana_gate::chain(state, order);
        state
            .order(&root)
            .and_then(|root| crate::nirvana_profiles::from_reason(&root.seated.reason))
    })
}

/// The review a reseat child carries on: up its `reseat` parents while they are reviews. The sidecar, the merge's
/// shas and the folder snapshot are the first review's.
pub fn review_root(state: &nirvana_run::RunState, order: &str) -> String {
    let mut current = order.to_owned();
    while let Some(known) = state.order(&current) {
        match (&known.seated.why, &known.seated.parent) {
            (Some(why), Some(parent))
                if why == "reseat"
                    && state
                        .order(parent)
                        .is_some_and(|parent| parent.seated.class == CLASS) =>
            {
                current = parent.clone();
            }
            _ => break,
        }
    }
    current
}

/// Review B3: a review's own folder, under its run, never the project's root where workers edit.
pub fn review_cwd(project: &Project, run: &str, id: &str) -> PathBuf {
    project.run_dir(run).join("reviews").join(id)
}

/// The live lessons an order of `class` and `profile` is offered, each with its body and body sha.
fn lessons_section(root: &Path, class: &str, profile: Option<&str>) -> String {
    let index = nirvana_lessons::read_index(root);
    let mut text = String::new();
    for (slug, entry) in &index.lessons {
        if !nirvana_lessons::live(&entry.counters) || !nirvana_lessons::fits(entry, class, profile)
        {
            continue;
        }
        let (Some(body), Some(sha)) = (
            nirvana_lessons::body(root, slug),
            nirvana_lessons::body_sha(root, slug),
        ) else {
            continue;
        };
        text.push_str(&format!(
            "### {slug} (base_sha {sha})\n\n```markdown\n{}\n```\n\n",
            body.trim_end()
        ));
    }
    if text.is_empty() {
        "None yet.\n\n".into()
    } else {
        text
    }
}

/// The subject of a review as its brief needs it.
pub struct Subject<'a> {
    pub order: &'a str,
    pub class: &'a str,
    pub profile: Option<&'a str>,
    pub order_text: &'a str,
    pub report: Option<&'a Value>,
    pub verdict: &'a Verdict,
    pub stream: &'a Path,
}

/// §3.3's brief: the order text, the Work Report, the verdict and send-back reasons, the worker's own lesson, the
/// live lessons of its class and profile with bodies and shas. Never the stream itself: its file is named.
pub fn review_brief(root: &Path, subject: &Subject) -> String {
    let mut text = format!(
        "# Lesson review of {} (plan 33 §3.3)\n\nherdr's gate ruled on the order below. Decide whether a lesson should be \
kept for later {} orders{}. Change no file: you can only read, and herdr checks each op in your Work Report and \
applies the ones that hold.\n\n{RULES}\n## The gate's verdict\n\n",
        subject.order,
        subject.class,
        subject
            .profile
            .map(|p| format!(" and orders seated with the profile {p}"))
            .unwrap_or_default()
    );
    match subject.verdict {
        Verdict::Pass => text.push_str("pass\n\n"),
        Verdict::SentBack(reasons) => {
            text.push_str("sent back, for:\n\n");
            for reason in reasons {
                text.push_str(&format!("- {reason}\n"));
            }
            text.push('\n');
        }
    }
    text.push_str(&format!(
        "## The order\n\n{}\n\n## Its Work Report\n\n",
        quoted_order(subject.order_text, subject.profile)
    ));
    let mut report = subject.report.cloned().unwrap_or(Value::Null);
    let own = report
        .as_object_mut()
        .and_then(|object| object.remove("lesson"));
    text.push_str(&format!(
        "```json\n{}\n```\n\n## The worker's own lesson\n\n",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    ));
    match own {
        Some(lesson) => text.push_str(&format!(
            "The gate checked it on its own; keep it again only through an op.\n\n```json\n{}\n```\n\n",
            serde_json::to_string_pretty(&lesson).unwrap_or_default()
        )),
        None => text.push_str("None offered.\n\n"),
    }
    text.push_str(&format!(
        "## The worker's stream\n\n{} (a copy, JSON lines; read what you need, the evidence comes from here)\n\n\
## The live lessons of {}{}\n\n{}",
        subject.stream.display(),
        subject.class,
        subject
            .profile
            .map(|p| format!(" and the profile {p}"))
            .unwrap_or_default(),
        lessons_section(root, subject.class, subject.profile)
    ));
    text.push_str(&format!(
        "## Your Work Report\n\nstatus `done`; the summary says what you kept and why, or that nothing is worth keeping; \
files, tests and gotchas empty lists; and `lesson_ops`, a list of:\n\n\
- `{{\"op\": \"new\", \"name\": \"...\", \"class\": \"{class}\", \"steps\": \"...\", \"evidence\": \"a quote from the stream\"}}`\n\
- `{{\"op\": \"patch\", \"slug\": \"...\", \"base_sha\": \"...\", \"name\": \"...\", \"class\": \"{class}\", \"steps\": \
\"the whole new steps\", \"evidence\": \"a quote from the stream\"}}`\n\n\
An empty list keeps nothing. A `lesson` of your own is ignored.\n",
        class = subject.class
    ));
    text
}

/// §3.4's brief: every live lesson of the group, each with its body and sha, and the merge op.
pub fn merge_brief(
    root: &Path,
    class: &str,
    profile: Option<&str>,
) -> (String, BTreeMap<String, String>) {
    let index = nirvana_lessons::read_index(root);
    let mut shas = BTreeMap::new();
    let mut body = String::new();
    for (slug, entry) in &index.lessons {
        if !nirvana_lessons::live(&entry.counters)
            || entry.class != class
            || entry.profile.as_deref() != profile
        {
            continue;
        }
        let (Some(text), Some(sha)) = (
            nirvana_lessons::body(root, slug),
            nirvana_lessons::body_sha(root, slug),
        ) else {
            continue;
        };
        body.push_str(&format!(
            "### {slug}\n\n```markdown\n{}\n```\n\n",
            text.trim_end()
        ));
        shas.insert(slug.clone(), sha);
    }
    let text = format!(
        "# Umbrella lessons for {class}{} (plan 33 §3.4)\n\nThe {} live lessons below share the class {class}. Each costs a \
line in every brief of the class. Merge the narrow ones into umbrella lessons. Change no file: herdr applies your ops.\n\n\
{MERGE_RULES}\n## The lessons\n\n{body}## Your Work Report\n\nstatus `done`; the summary names the umbrellas; files, \
tests and gotchas empty lists; and `lesson_ops`, a list of:\n\n\
- `{{\"op\": \"merge\", \"slugs\": [\"...\", \"...\"], \"name\": \"...\", \"steps\": \"...\"}}`\n\n\
Each slug is one of the lessons above. An empty list merges nothing.\n",
        profile
            .map(|p| format!(" (profile {p})"))
            .unwrap_or_default(),
        shas.len()
    );
    (text, shas)
}

// ---- seating, the way a send-back child is seated ----

/// herdr's one launcher. No test starts a worker: a test's `current_exe` is the test binary, and in a herdr pane it
/// would open a pane in the owner's herdr.
fn launch(project: &Project, run: &str, seated: &Seated) -> Result<String, String> {
    #[cfg(not(test))]
    {
        crate::nirvana_worker::launch_worker(project, run, seated)
    }
    #[cfg(test)]
    {
        let _ = (project, run, seated);
        Ok("not started in a test".into())
    }
}

/// The review's pick: the rule at Efficient, size small, on the live world and the live leases.
pub fn pick(
    table: &Table,
    world: &nirvana_seat::World,
    id: &str,
) -> Result<nirvana_seat::Pick, String> {
    let mut request = Request::new(CLASS);
    request.size = SizeName::Small;
    request.mode = Mode::Efficient;
    request.order = Some(id.to_owned());
    nirvana_seat::pick(table, world, &request)
        .map_err(|no_seat| format!("no seat: {}", no_seat.reason))
}

/// Seats `id` of `kind` about `subject` in its own folder under the run (review B3), and starts it. The lease is
/// booked last, after the order, the sidecar, the folder snapshot and the count are written, and released when the
/// `seated` event cannot be journaled: a review that never reaches the journal holds no seat.
#[allow(clippy::too_many_arguments)]
fn queue(
    project: &Project,
    run: &str,
    journal: &Journal,
    id: &str,
    parent: Option<&str>,
    kind: &str,
    subject: &str,
    brief: &str,
    table: &Table,
) -> Result<String, String> {
    let mut world = nirvana_seat::live_world(table);
    let leases_path = nirvana_seat::dir().join(nirvana_seat::LEASES_FILE);
    if let Ok(leases) = nirvana_seat::load_leases(&leases_path) {
        world.leases = leases.leases.clone();
        world.overrides_used = leases.overrides(run);
    }
    let pick = pick(table, &world, id)?;
    let cwd = review_cwd(project, run, id);
    std::fs::create_dir_all(&cwd).map_err(|err| err.to_string())?;
    let order_file = crate::nirvana_worker::write_order(project, &cwd, id, brief)
        .map_err(|err| err.to_string())?;
    let seated = Seated {
        order: id.to_owned(),
        seat: pick.seat.clone(),
        cli: pick.cli.clone(),
        model: pick.model.clone(),
        effort: pick.effort,
        pool: pick.pool.clone(),
        class: CLASS.into(),
        chain: Some(pick.chain.clone()),
        size: SizeName::Small,
        mode: Mode::Efficient,
        needs: Vec::new(),
        not_family: Vec::new(),
        reason: pick.reason.clone(),
        override_reason: None,
        order_file,
        cwd: cwd.display().to_string(),
        worktree: None,
        branch: None,
        parent: parent.map(str::to_owned),
        why: Some(kind.replace('-', "_")),
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
        send_ok: None,
    };
    nirvana_gate::write_sidecar(
        project,
        run,
        &Sidecar {
            order: id.to_owned(),
            helps: Some((kind.to_owned(), subject.to_owned())),
            ..Sidecar::default()
        },
    )
    .map_err(|err| err.to_string())?;
    // Review B3: the lessons folder as it stands now, which the review's gate compares against.
    std::fs::create_dir_all(nirvana_gate::gate_dir(project, run)).map_err(|e| e.to_string())?;
    std::fs::write(
        snap_path(project, run, id),
        serde_json::to_string_pretty(&nirvana_lessons::folder_snap(&project.root))
            .unwrap_or_default(),
    )
    .map_err(|err| err.to_string())?;
    let at = now();
    change_state(&project.root, |state| {
        if state.day != at / 86_400 {
            state.day = at / 86_400;
            state.queued = 0;
        }
        state.queued += 1;
        state.pending.push(Pending {
            run: run.to_owned(),
            order: id.to_owned(),
            kind: kind.to_owned(),
            subject: subject.to_owned(),
            at,
        });
    })
    .map_err(|err| err.to_string())?;
    nirvana_gate::book(table, &pick, id, run).map_err(|err| err.to_string())?;
    if let Err(err) = journal.append(now(), Kind::Seated(Box::new(seated.clone()))) {
        crate::nirvana_worker::release_lease(id);
        return Err(err.to_string());
    }
    let how = launch(project, run, &seated).unwrap_or_else(|err| err);
    Ok(format!(
        "{id} seated on {} {} ({how}): the {kind} of {subject}; herdr gates it when it delivers",
        pick.seat,
        pick.effort.word()
    ))
}

/// The gate's call after a pass or a send-back that `wanted` allowed: the next line of its ruling.
pub fn queue_review(
    project: &Project,
    run: &str,
    journal: &Journal,
    known: &OrderState,
    verdict: &Verdict,
    table: Option<&Table>,
) -> String {
    let Some(table) = table else {
        return "lesson review not queued: seats.toml is refused".into();
    };
    let order = known.id();
    let order_text = nirvana_gate::order_text(&known.seated);
    let profile = subject_profile(&replay(&journal.read().0), order);
    let id = review_id(order);
    // Review B3: the reviewer reads a copy of the stream in its own folder; its file tools stay inside that folder.
    let stream = project
        .run_dir(run)
        .join("streams")
        .join(format!("{order}.jsonl"));
    let cwd = review_cwd(project, run, &id);
    let copy = cwd.join("stream.jsonl");
    let stream = match std::fs::create_dir_all(&cwd).and_then(|()| std::fs::copy(&stream, &copy)) {
        Ok(_) => copy,
        Err(_) => stream,
    };
    let brief = review_brief(
        &project.root,
        &Subject {
            order,
            class: &known.seated.class,
            profile: profile.as_deref(),
            order_text: &order_text,
            report: known.report.as_ref(),
            verdict,
            stream: &stream,
        },
    );
    queue(
        project,
        run,
        journal,
        &id,
        Some(order),
        REVIEW,
        order,
        &brief,
        table,
    )
    .unwrap_or_else(|why| format!("lesson review not queued: {why}"))
}

// ---- the ops ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    New(Lesson),
    Patch {
        slug: String,
        base_sha: String,
        lesson: Lesson,
    },
    Merge {
        slugs: Vec<String>,
        name: String,
        steps: String,
    },
}

impl Op {
    fn word(&self) -> &'static str {
        match self {
            Self::New(_) => "new",
            Self::Patch { .. } => "patch",
            Self::Merge { .. } => "merge",
        }
    }
}

/// Each `lesson_ops` item as an op, or why it is not one.
pub fn parse_ops(report: &Value) -> Vec<Result<Op, String>> {
    let Some(items) = report.get("lesson_ops").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            };
            match item.get("op").and_then(Value::as_str) {
                Some("new") => Lesson::from_value(item).map(Op::New),
                Some("patch") => {
                    let (slug, base_sha) = (text("slug"), text("base_sha"));
                    if slug.is_empty() || base_sha.is_empty() {
                        return Err("a patch names the slug and the base_sha it read".into());
                    }
                    Lesson::from_value(item).map(|lesson| Op::Patch {
                        slug,
                        base_sha,
                        lesson,
                    })
                }
                Some("merge") => {
                    let slugs: Vec<String> = item
                        .get("slugs")
                        .and_then(Value::as_array)
                        .map(|slugs| {
                            slugs
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default();
                    Ok(Op::Merge {
                        slugs,
                        name: text("name"),
                        steps: text("steps"),
                    })
                }
                Some(other) => Err(format!("unknown op {other:?}")),
                None => Err("an op with no `op`".into()),
            }
        })
        .collect()
}

/// What one review's ops come to: for each, the line said and whether herdr applied it.
pub struct Applied {
    pub lines: Vec<(bool, String)>,
}

/// What a review's ops may touch and what their evidence may not be: the reviewed order's class and its journaled
/// profile, and the text herdr itself wrote (the order copy, the review's brief), which a worker's stream holds when
/// the worker read its order file.
pub struct Scope<'a> {
    pub order: &'a str,
    pub class: &'a str,
    pub profile: Option<&'a str>,
    pub herdr_wrote: Vec<String>,
}

/// Review S2: why a review op's evidence does not count, when it does not.
fn evidence_refusal(root: &Path, stream: &str, scope: &Scope, evidence: &str) -> Option<String> {
    let quote = evidence.replace("\r\n", "\n");
    let quote = quote.trim();
    let length = quote.chars().count();
    if length < nirvana_lessons::MIN_EVIDENCE {
        return Some(format!(
            "the evidence is {length} characters; a quote of at least {} proves something",
            nirvana_lessons::MIN_EVIDENCE
        ));
    }
    if !nirvana_lessons::quote_in_stream(stream, quote) {
        return Some(format!("the evidence is not in {}'s stream", scope.order));
    }
    if scope
        .herdr_wrote
        .iter()
        .any(|text| text.replace("\r\n", "\n").contains(quote))
    {
        return Some(
            "the evidence is text herdr wrote (the order or this brief), not the worker's".into(),
        );
    }
    let index = nirvana_lessons::read_index(root);
    if let Some(slug) = index
        .lessons
        .keys()
        .find(|slug| nirvana_lessons::body(root, slug).is_some_and(|body| body.contains(quote)))
    {
        return Some(format!("the evidence is lesson {slug}'s own text"));
    }
    None
}

/// A review's ops against the reviewed worker's `stream`; a merge op is refused here.
pub fn apply_review(
    root: &Path,
    ops: Vec<Result<Op, String>>,
    stream: &str,
    scope: &Scope,
    by: &str,
    run: &str,
    at: u64,
) -> Applied {
    let mut lines = Vec::new();
    for (n, op) in ops.into_iter().enumerate() {
        let n = n + 1;
        let done = op.and_then(|op| {
            let evidence = match &op {
                Op::New(lesson) | Op::Patch { lesson, .. } => &lesson.evidence,
                Op::Merge { .. } => {
                    return Err("merge is a merge order's op, not a review's".into())
                }
            };
            if let Some(why) = evidence_refusal(root, stream, scope, evidence) {
                return Err(why);
            }
            match op {
                // Review S1: a review keeps lessons for the class it reviewed, nowhere else.
                Op::New(lesson) if lesson.class != scope.class => Err(format!(
                    "a review of a {} order keeps {} lessons, not {}",
                    scope.class, scope.class, lesson.class
                )),
                Op::New(lesson) => {
                    nirvana_lessons::review_new(root, &lesson, by, run, at, scope.profile)
                        .map(|(slug, id)| format!("{id}: new {slug}"))
                }
                Op::Patch {
                    slug,
                    base_sha,
                    lesson,
                } => nirvana_lessons::review_patch(
                    root,
                    &slug,
                    &base_sha,
                    &lesson,
                    (scope.class, scope.profile),
                    by,
                    run,
                    at,
                )
                .map(|id| format!("{id}: patched {slug}")),
                Op::Merge { .. } => unreachable!("refused above"),
            }
        });
        lines.push(match done {
            Ok(line) => (true, line),
            Err(why) => (false, format!("op {n} refused: {why}")),
        });
    }
    Applied { lines }
}
/// A merge order's ops: each a merge of the group's own lessons, none changed since the brief.
#[allow(clippy::too_many_arguments)]
pub fn apply_merge(
    root: &Path,
    ops: Vec<Result<Op, String>>,
    class: &str,
    profile: Option<&str>,
    shas: &BTreeMap<String, String>,
    by: &str,
    run: &str,
    at: u64,
) -> Applied {
    let mut lines = Vec::new();
    for (n, op) in ops.into_iter().enumerate() {
        let n = n + 1;
        let done = op.and_then(|op| {
            let Op::Merge { slugs, name, steps } = op else {
                return Err(format!(
                    "{} is a review's op; a merge order merges",
                    op.word()
                ));
            };
            if let Some(slug) = slugs.iter().find(|slug| !shas.contains_key(*slug)) {
                return Err(format!(
                    "{slug} is not one of the lessons this merge was given"
                ));
            }
            let index = nirvana_lessons::read_index(root);
            if let Some(slug) = slugs.iter().find(|slug| {
                index
                    .lessons
                    .get(*slug)
                    .is_some_and(|e| e.class != class || e.profile.as_deref() != profile)
            }) {
                return Err(format!("{slug} is not of {class}"));
            }
            nirvana_lessons::merge(root, &slugs, &name, &steps, shas, by, run, at)
                .map(|(slug, id)| format!("{id}: merged {} into {slug}", slugs.join(", ")))
        });
        lines.push(match done {
            Ok(line) => (true, line),
            Err(why) => (false, format!("op {n} refused: {why}")),
        });
    }
    Applied { lines }
}

/// The Work Report's own check, with `lesson_ops` aside: the worker's schema learns the key from P33b's hunk.
fn report_axis(report: Option<&Value>) -> Axis {
    let Some(report) = report else {
        return Axis::new("report", Mark::Fail, "no Work Report was delivered");
    };
    let mut plain = report.clone();
    let ops = plain
        .as_object_mut()
        .and_then(|object| object.remove("lesson_ops"));
    if ops.as_ref().is_some_and(|ops| {
        !ops.as_array()
            .is_some_and(|items| items.iter().all(Value::is_object))
    }) {
        return Axis::new("report", Mark::Fail, "lesson_ops is not a list of objects");
    }
    nirvana_gate::report_or_verdict(Some(&plain))
}

fn shas_path(project: &Project, run: &str, id: &str) -> PathBuf {
    nirvana_gate::gate_dir(project, run).join(format!("{id}.shas.json"))
}

/// Review B3: the lessons folder as it stood when the review was queued.
fn snap_path(project: &Project, run: &str, id: &str) -> PathBuf {
    nirvana_gate::gate_dir(project, run).join(format!("{id}.lessons.json"))
}

/// A merge order's subject: `<class>`, or `<class>@<profile>`.
fn group(subject: &str) -> (&str, Option<&str>) {
    match subject.split_once('@') {
        Some((class, profile)) => (class, Some(profile)),
        None => (subject, None),
    }
}

/// Why no op of this review is applied, before any is tried: its report is not `done` (review B2), or the lessons
/// folder changed since it was queued in a way herdr's own writes do not explain (review B3).
fn held_back(project: &Project, run: &str, root: &str, report: &Value) -> Option<Axis> {
    let status = report.get("status").and_then(Value::as_str).unwrap_or("");
    if status != "done" {
        return Some(Axis::new(
            "status",
            Mark::Fail,
            format!("the review's status is {status}: no op is applied"),
        ));
    }
    let snap: nirvana_lessons::FolderSnap = std::fs::read_to_string(snap_path(project, run, root))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())?;
    let changed = nirvana_lessons::unexplained(&project.root, &snap);
    (!changed.is_empty()).then(|| {
        Axis::new(
            "lessons_folder",
            Mark::Fail,
            format!(
                "{} changed in .nirvana/lessons since the review was queued, and herdr did not write it: no op is \
                 applied",
                changed.join(", ")
            ),
        )
    })
}

/// `herdr gate run <review>`: the report well formed, then each op applied or refused, said one axis each.
pub fn gate_review(
    project: &Project,
    run: &str,
    journal: &Journal,
    known: &OrderState,
    kind: &str,
    subject: &str,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    if let Some(verdict) = &known.verdict {
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: verdict.clone(),
            axes: Vec::new(),
            next: Vec::new(),
        });
    }
    if !matches!(known.phase, Phase::Delivered | Phase::Asked) {
        return Err(format!(
            "the {kind} {order} is {}: the gate rules on a delivered order",
            known.phase.word()
        ));
    }
    let state = replay(&journal.read().0);
    let root = review_root(&state, &order);
    let report = known.report.clone();
    let mut axes = vec![report_axis(report.as_ref())];
    let mut passed = axes[0].mark != Mark::Fail;
    if passed {
        let report = report.unwrap_or(Value::Null);
        let held = held_back(project, run, &root, &report);
        let ops = parse_ops(&report);
        let at = now();
        let applied = if let Some(axis) = held {
            passed = false;
            axes.push(axis);
            Applied { lines: Vec::new() }
        } else if ops.is_empty() {
            Applied {
                lines: vec![(true, "nothing to keep".into())],
            }
        } else if kind == MERGE {
            let shas: BTreeMap<String, String> =
                std::fs::read_to_string(shas_path(project, run, &root))
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
                    .unwrap_or_default();
            let (class, profile) = group(subject);
            apply_merge(&project.root, ops, class, profile, &shas, &order, run, at)
        } else {
            let stream = nirvana_gate::stream_text(project, run, subject);
            let reviewed = state.order(subject);
            let class = reviewed.map(|s| s.seated.class.clone()).unwrap_or_default();
            let profile = subject_profile(&state, subject);
            // Review S2: the text herdr wrote, which the worker's stream holds when it read its order: the subject's
            // copy, and this review's brief (and its first review's, for a reseat child).
            let mut herdr_wrote: Vec<String> = reviewed
                .map(|s| nirvana_gate::order_text(&s.seated))
                .into_iter()
                .collect();
            for id in [order.as_str(), root.as_str()] {
                if let Some(review) = state.order(id) {
                    herdr_wrote.push(nirvana_gate::order_text(&review.seated));
                }
            }
            herdr_wrote.retain(|text| !text.trim().is_empty());
            apply_review(
                &project.root,
                ops,
                &stream,
                &Scope {
                    order: subject,
                    class: &class,
                    profile: profile.as_deref(),
                    herdr_wrote,
                },
                &order,
                run,
                at,
            )
        };
        for (kept, line) in applied.lines {
            axes.push(Axis::new(
                "lesson_op",
                if kept { Mark::Pass } else { Mark::Note },
                line,
            ));
        }
        if report.get("lesson").is_some() {
            axes.push(Axis::new(
                "lesson",
                Mark::Note,
                "the reviewer's own lesson is not kept (plan 33 §3.3)",
            ));
        }
        if let Some(home) = crate::nirvana::home() {
            let _ = nirvana_lessons::write_index_line(&home, &project.root);
        }
    }
    let verdict = if passed {
        Outcome::Pass
    } else {
        Outcome::Failed
    };
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.clone(),
                verdict: verdict.word().into(),
                reason: (!passed).then(|| nirvana_gate::reason_line(&axes).unwrap_or_default()),
                axes: nirvana_gate::axes_values(&axes),
            },
        )
        .map_err(|err| err.to_string())?;
    let _ = change_state(&project.root, |state| {
        state
            .pending
            .retain(|p| !(p.run == run && (p.order == order || p.order == root)));
    });
    Ok(Ruling {
        order,
        run: run.to_owned(),
        verdict: verdict.word().into(),
        axes,
        next: Vec::new(),
    })
}
// ---- merging (§3.4) ----

/// The groups over `MERGE_ABOVE` live lessons: `(class, profile, count)`.
pub fn crowded(root: &Path) -> Vec<(String, Option<String>, usize)> {
    let index = nirvana_lessons::read_index(root);
    let mut groups: BTreeMap<(String, Option<String>), usize> = BTreeMap::new();
    for entry in index.lessons.values() {
        if nirvana_lessons::live(&entry.counters) {
            *groups
                .entry((entry.class.clone(), entry.profile.clone()))
                .or_default() += 1;
        }
    }
    groups
        .into_iter()
        .filter(|(_, count)| *count > MERGE_ABOVE)
        .map(|((class, profile), count)| (class, profile, count))
        .collect()
}

fn group_subject(class: &str, profile: Option<&str>) -> String {
    profile.map_or_else(|| class.to_owned(), |p| format!("{class}@{p}"))
}

/// Queues one merge order for `class` and `profile` in `run`, unless one is pending or the cap is spent.
pub fn queue_merge(
    project: &Project,
    run: &str,
    class: &str,
    profile: Option<&str>,
    table: &Table,
    cap: u32,
) -> Result<String, String> {
    let subject = group_subject(class, profile);
    // Review S6: the pending check, the cap and the count under the gate's lock, as a review's are inside `gate`, so
    // two merges at once cannot both pass the cap.
    let _lock = nirvana_gate::gate_lock(project).map_err(|err| err.to_string())?;
    if read_state(&project.root)
        .pending
        .iter()
        .any(|p| p.kind == MERGE && p.subject == subject)
    {
        return Err(format!("a merge of {subject} is pending already"));
    }
    if let Some(why) = cap_refusal(&project.root, cap, now()) {
        return Err(why);
    }
    let journal = project.journal(run);
    if !journal.path.is_file() {
        return Err(format!("no run {run} in {}", project.dir().display()));
    }
    let state = replay(&journal.read().0);
    let base = format!("lm-{}", nirvana_lessons::slug(&subject));
    let id = (1..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| state.order(id).is_none())
        .unwrap_or(base);
    let (brief, shas) = merge_brief(&project.root, class, profile);
    if shas.len() < 2 {
        return Err(format!("{subject} holds fewer than two live lessons"));
    }
    std::fs::create_dir_all(nirvana_gate::gate_dir(project, run)).map_err(|e| e.to_string())?;
    std::fs::write(
        shas_path(project, run, &id),
        serde_json::to_string_pretty(&shas).unwrap_or_default(),
    )
    .map_err(|err| err.to_string())?;
    queue(
        project, run, &journal, &id, None, MERGE, &subject, &brief, table,
    )
}

/// `herdr run end`'s step: a merge order for each crowded group. Reads the seat table only when one is crowded.
pub fn merge_crowded(project: &Project, run: &str) -> Vec<String> {
    let groups = crowded(&project.root);
    if groups.is_empty() {
        return Vec::new();
    }
    let table = match nirvana_seat::table_for(&project.root) {
        Ok(table) => table,
        Err(why) => return vec![format!("lessons merge: seats.toml is refused: {why}")],
    };
    let cap = per_day(Some(&table));
    groups
        .into_iter()
        .map(|(class, profile, count)| {
            match queue_merge(project, run, &class, profile.as_deref(), &table, cap) {
                Ok(line) => format!("lessons merge ({count} live): {line}"),
                Err(why) => format!("lessons merge of {class} ({count} live) not queued: {why}"),
            }
        })
        .collect()
}

/// Gates each pending review that has delivered, and drops one that ended without a delivery or is older than
/// `PENDING_DAYS`. `herdr run end` runs it: a review delivered after its run ended has no orchestrator to wake.
pub fn settle(project: &Project) -> Vec<String> {
    let pending = read_state(&project.root).pending;
    let mut lines = Vec::new();
    let mut drop = Vec::new();
    for p in &pending {
        let state = replay(&project.journal(&p.run).read().0);
        match state.order(&p.order) {
            Some(known) if known.verdict.is_some() => drop.push(p.clone()),
            Some(known) if known.phase == Phase::Delivered => {
                match nirvana_gate::gate(project, &p.run, &p.order) {
                    Ok(ruling) => lines.push(format!("{}: {}", ruling.order, ruling.verdict)),
                    Err(why) => lines.push(format!("{}: {why}", p.order)),
                }
            }
            Some(known) if known.phase.terminal() => drop.push(p.clone()),
            None => drop.push(p.clone()),
            Some(_) if now().saturating_sub(p.at) > PENDING_DAYS * 86_400 => drop.push(p.clone()),
            Some(_) => {}
        }
    }
    if !drop.is_empty() {
        let _ = change_state(&project.root, |state| {
            state.pending.retain(|p| !drop.contains(p));
        });
    }
    lines
}

const MERGE_USAGE: &str =
    "usage: herdr lessons merge [--class <class>] [--profile <name>] [--run <run>]   (--run defaults to $NIRVANA_RUN)";

/// `herdr lessons merge`: with `--class`, that group when it holds two live lessons or more; else every group over 8.
pub fn merge_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let (mut class, mut profile, mut run) = (None, None, std::env::var("NIRVANA_RUN").ok());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--class" => class = iter.next().cloned(),
            "--profile" => profile = iter.next().cloned(),
            "--run" => run = iter.next().cloned(),
            _ => {
                eprintln!("{MERGE_USAGE}");
                std::process::exit(2);
            }
        }
    }
    let Some(run) = run.filter(|run| nirvana_run::valid_id(run)) else {
        eprintln!("{MERGE_USAGE}");
        std::process::exit(2);
    };
    let lines = match class {
        Some(class) => {
            let table = nirvana_seat::table_for(&project.root).map_err(std::io::Error::other)?;
            vec![queue_merge(
                project,
                &run,
                &class,
                profile.as_deref(),
                &table,
                per_day(Some(&table)),
            )
            .unwrap_or_else(|why| format!("not queued: {why}"))]
        }
        None => merge_crowded(project, &run),
    };
    if lines.is_empty() {
        println!("no class holds more than {MERGE_ABOVE} live lessons");
    }
    for line in lines {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests;
