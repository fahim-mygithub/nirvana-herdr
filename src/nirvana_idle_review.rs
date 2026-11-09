//! Nirvana fork, order LEARN (2026-11-09): a memory review in idle time, on usage that would otherwise be lost.
//! (nirvana/plans/2026-11-09-learn-idle.md)
//!
//! The owner, 2026-11-09, verbatim: "how is the memory system working is it improving the agents and storing finding
//! in memory? Can we have it review during long periods of idle if we have ree usage left?"
//!
//! - **One pure rule**, `decide`, over the facts in `Facts`, as plan 27's idle compact keeps its own. The server only
//!   starts `herdr lessons idle tick` every `TICK` (`start`); the tick gathers the facts from files (no CLI, no model)
//!   and queues at most one review. Any fact it cannot read is a reason not to run: the review spends the owner's
//!   seats, and a review that runs on a guess is the worst outcome (fail closed).
//! - **Idle** is the whole machine's, not one space's: no live lease anywhere (`leases.json`, which every seated order
//!   holds until it ends), no order's journal or stream written in any project herdr knows for `IDLE_AFTER`, and no
//!   owner key in any pane for as long (`<home>/nirvana/keys/`, which the server writes for the wake path). "Never while
//!   any order runs anywhere" is the order's rule; a space whose neighbour is busy is not idle for it either.
//! - **Spare** is the seat rule's own reading: the review is picked the way a lesson review is (`nirvana_review::pick`,
//!   Efficient, small) on the cached world (`nirvana_seat::cached_world`, the readings the window keeps fresh), and
//!   runs only when that seat's pool's 5 h window is read fresh, at least `SPARE_MIN` of it is projected unused, and it
//!   resets within `RESETS_WITHIN_S`: what it spends is usage the window would lose at its reset. A missing or stale
//!   reading is no review.
//! - **A normal order with its own record.** It opens a run of its own (`idle-<utc>`), is seated and launched by
//!   `nirvana_review::queue` as every lesson review is, and is gated by `nirvana_review::gate_review`, which hands its
//!   ops to `apply` here: new and patch ops through the same evidence checks against the stream of the order they name,
//!   merge through `nirvana_lessons::merge`, and retire through `nirvana_lessons::retire`; every one a ledger row that
//!   `herdr lessons rollback` undoes. Its run ends when it is gated, and the Learning square shows what it did.
//! - **Caps**: `PER_SPACE_PER_DAY` per project and UTC day, `PER_WINDOW` across the machine per 5 h window of the pool
//!   it runs on, and `lesson_reviews_per_day` still holds (it is a lesson review). Off with `herdr lessons idle off`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::nirvana_gate::{self, Axis, Mark};
use crate::nirvana_lessons;
use crate::nirvana_review::{self as review, Applied, Op, Scope};
use crate::nirvana_run::{self, replay, Kind, Phase, Project};
use crate::nirvana_seat::{self, Pick, Table, World};

/// No owner key and no order written anywhere for this long. A hypothesis: the owner named "near 30 min"; plan 27
/// measured Claude's prompt cache at one hour, so an owner back inside the half hour still finds their own cache warm
/// and the review's seat is not the one they would have used. The record holds the measured gaps between orders.
pub const IDLE_AFTER_S: u64 = 30 * 60;
/// How often the server starts a tick. A tick reads a few hundred file stats and no model; five minutes keeps a
/// review inside the first sixth of an idle hour. Read only by `start`, which does nothing under test.
#[cfg_attr(test, allow(dead_code))]
pub const TICK: Duration = Duration::from_secs(300);
/// The share of the cheap seat's 5 h window projected unused at its reset. A hypothesis: the owner's own example ("at
/// least half projected unused").
pub const SPARE_MIN: f32 = 0.5;
/// The window resets within this. A hypothesis: the owner's "within the next ~2 h"; any later, the spend could still be
/// wanted by the owner's own work before the reset.
pub const RESETS_WITHIN_S: u64 = 2 * 3600;
/// Idle reviews per project per UTC day. A hypothesis: a review reads every order since the last one, so a second in a
/// day reads only what ran between them.
pub const PER_SPACE_PER_DAY: u32 = 2;
/// Idle reviews across the machine per 5 h window of the pool they run on. A hypothesis: a flash-low lesson review was
/// 20k-50k tokens (live checks 2026-11-01 F8); three are a small share of a window that has half left.
pub const PER_WINDOW: u32 = 3;
/// The orders one review reads, newest first; the rest are counted in its brief.
pub const MAX_ORDERS: usize = 12;
/// The tail of each order's stream a review gets as a digest, in characters: its tool results and text, never the
/// whole transcript (median stream in `3D-Printer Flow` 2.5 MB, 2026-11-09).
pub const DIGEST_CHARS: usize = 24_000;
/// A project's first idle review reads back at most this far.
pub const LOOKBACK_S: u64 = 7 * 86_400;
/// The subject a review's sidecar names for an idle review (`nirvana_review::gate_review` hands it to `apply`). Not an
/// order id (`nirvana_run::valid_id` refuses `@`), so no order is ever mistaken for it.
pub const SUBJECT: &str = "@idle";
/// Two readings of one pool's window whose resets are this close are the same window.
const SAME_WINDOW_S: u64 = 900;
const STATE: &str = ".idle-review.json";
const GLOBAL: &str = "idle-review.json";
const LOCK: &str = "idle-review.lock";

// ---- the rule ----

/// The cheap seat's 5 h window as the seat rule read it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spare {
    pub seat: String,
    pub pool: String,
    /// `None`: no fresh reading of the pool's 5 h window (missing, old, or read before a passed reset).
    pub spare: Option<f32>,
    pub resets_at: Option<u64>,
}

/// Everything `decide` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub now: u64,
    /// `herdr lessons idle on`.
    pub on: bool,
    /// Why an order runs somewhere: a live lease.
    pub busy: Option<String>,
    /// The newest owner key in any pane; `Err` when a key file cannot be read.
    pub owner_key: Result<Option<u64>, String>,
    /// The newest journal or stream write of any order in any known project, and where.
    pub activity: Option<(u64, String)>,
    /// Orders gated in this project since its last idle review.
    pub new_orders: usize,
    /// Why `lesson_reviews_per_day` refuses a review now.
    pub review_cap: Option<String>,
    pub used_today: u32,
    pub used_window: u32,
    /// The seat a review would take; `Err` with why when none would.
    pub spare: Result<Spare, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Run,
    Skip(String),
}

fn minutes(seconds: u64) -> String {
    crate::nirvana_usage::short_duration(seconds)
}

/// The rule. Each refusal is said, so the Learning square can say why no review ran.
pub fn decide(facts: &Facts) -> Decision {
    let skip = |why: String| Decision::Skip(why);
    if !facts.on {
        return skip("idle review is off (herdr lessons idle on)".into());
    }
    if let Some(why) = &facts.busy {
        return skip(format!("an order runs: {why}"));
    }
    match &facts.owner_key {
        Err(why) => return skip(format!("the owner's last key cannot be read: {why}")),
        Ok(Some(at)) if facts.now.saturating_sub(*at) < IDLE_AFTER_S => {
            return skip(format!(
                "the owner typed {} ago; idle after {}",
                minutes(facts.now.saturating_sub(*at)),
                minutes(IDLE_AFTER_S)
            ))
        }
        Ok(_) => {}
    }
    if let Some((at, project)) = &facts.activity {
        if facts.now.saturating_sub(*at) < IDLE_AFTER_S {
            return skip(format!(
                "an order wrote in {project} {} ago; idle after {}",
                minutes(facts.now.saturating_sub(*at)),
                minutes(IDLE_AFTER_S)
            ));
        }
    }
    if facts.new_orders == 0 {
        return skip("no order gated here since the last idle review".into());
    }
    if let Some(why) = &facts.review_cap {
        return skip(why.clone());
    }
    if facts.used_today >= PER_SPACE_PER_DAY {
        return skip(format!(
            "{} idle reviews here this UTC day, the cap",
            facts.used_today
        ));
    }
    if facts.used_window >= PER_WINDOW {
        return skip(format!(
            "{} idle reviews in this 5h window, the machine's cap",
            facts.used_window
        ));
    }
    let spare = match &facts.spare {
        Ok(spare) => spare,
        Err(why) => return skip(format!("no seat for a review: {why}")),
    };
    let label = crate::nirvana_usage::label(&spare.pool);
    let Some(share) = spare.spare else {
        return skip(format!(
            "{label}'s 5h reading is missing or old: no review on a guess"
        ));
    };
    if share < SPARE_MIN {
        return skip(format!(
            "{label}'s 5h window is {:.0}% projected unused, under {:.0}%",
            share * 100.0,
            SPARE_MIN * 100.0
        ));
    }
    let Some(resets_at) = spare.resets_at else {
        return skip(format!("{label}'s 5h reset is not known"));
    };
    let left = resets_at.saturating_sub(facts.now);
    if left > RESETS_WITHIN_S {
        return skip(format!(
            "{label}'s 5h window resets in {}: its spare is not lost yet (within {})",
            minutes(left),
            minutes(RESETS_WITHIN_S)
        ));
    }
    Decision::Run
}

// ---- state ----

/// The last idle review of a project, and what it came to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Last {
    pub at: u64,
    pub run: String,
    pub order: String,
    pub seat: String,
    /// The newest gated order it read: the next review reads after it.
    pub through: u64,
    pub orders: usize,
    /// What the gate made of it; `None` until it is gated.
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub done_at: Option<u64>,
}

/// A project's idle reviews, under `runs/` (which O3's `.gitignore` ignores): the machine's count, not history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceState {
    #[serde(default)]
    pub day: u64,
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub last: Option<Last>,
    /// The last tick's answer for this project, and when.
    #[serde(default)]
    pub check: Option<(u64, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Global {
    #[serde(default)]
    pub off: bool,
    /// The pool and reset of the window `count` counts.
    #[serde(default)]
    pub window: Option<(String, u64)>,
    #[serde(default)]
    pub count: u32,
}

pub fn space_path(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("runs").join(STATE)
}

pub fn global_path() -> PathBuf {
    nirvana_seat::dir().join(GLOBAL)
}

fn read_json<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    nirvana_seat::write_atomic(path, text.as_bytes())
}

pub fn read_space(root: &Path) -> SpaceState {
    read_json(&space_path(root))
}

pub fn read_global() -> Global {
    read_json(&global_path())
}

pub fn used_today(state: &SpaceState, now: u64) -> u32 {
    if state.day == now / 86_400 {
        state.count
    } else {
        0
    }
}

pub fn used_window(global: &Global, pool: &str, resets_at: Option<u64>) -> u32 {
    match (&global.window, resets_at) {
        (Some((own, at)), Some(resets)) if own == pool && at.abs_diff(resets) <= SAME_WINDOW_S => {
            global.count
        }
        _ => 0,
    }
}

/// `herdr lessons idle on|off`.
pub fn set_on(on: bool) -> std::io::Result<()> {
    let mut global = read_global();
    global.off = !on;
    write_json(&global_path(), &global)
}

// ---- the facts, from files ----

/// The projects herdr knows: each that an ended order's `record.jsonl` line names.
pub fn known_projects() -> Vec<PathBuf> {
    let text =
        std::fs::read_to_string(nirvana_seat::dir().join("record.jsonl")).unwrap_or_default();
    let mut out: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let Some(journal) = value.get("journal").and_then(Value::as_str) else {
            continue;
        };
        let Some((root, _)) = journal.split_once(nirvana_run::DIR) else {
            continue;
        };
        let root = PathBuf::from(root.trim_end_matches(['\\', '/']));
        if !root.as_os_str().is_empty() && !out.contains(&root) {
            out.push(root);
        }
    }
    out.sort();
    out
}

fn modified(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// The newest write of any order in `root`: each run's journal and every stream file.
pub fn activity(root: &Path) -> Option<u64> {
    let runs = Project::at(root).dir().join("runs");
    let mut newest = None;
    for run in std::fs::read_dir(runs).into_iter().flatten().flatten() {
        let path = run.path();
        if !path.is_dir() {
            continue;
        }
        newest = newest.max(modified(&path.join(nirvana_run::JOURNAL)));
        for stream in std::fs::read_dir(path.join("streams"))
            .into_iter()
            .flatten()
            .flatten()
        {
            newest = newest.max(modified(&stream.path()));
        }
    }
    newest
}

/// The newest owner key in any pane (`nirvana_run::note_owner_key`). No folder is no key; a file that cannot be read
/// is a doubt, not quiet.
pub fn owner_key(dir: &Path) -> Result<Option<u64>, String> {
    let entries = match std::fs::read_dir(dir.join("keys")) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("{}: {err}", dir.join("keys").display())),
    };
    let mut newest = None;
    for entry in entries {
        let path = entry.map_err(|err| err.to_string())?.path();
        let text =
            std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        let at: u64 = text
            .trim()
            .parse()
            .map_err(|_| format!("{} holds {:?}, not a second", path.display(), text.trim()))?;
        newest = newest.max(Some(at));
    }
    Ok(newest)
}

/// A live lease, said as `<order> on <seat>`.
pub fn busy(world: &World) -> Option<String> {
    world
        .leases
        .iter()
        .find(|lease| lease.live(world.now))
        .map(|lease| format!("{} on {}", lease.order, lease.seat))
}

/// The seat a review would take and its pool's 5 h window, as the seat rule reads them.
pub fn spare(table: &Table, world: &World, id: &str) -> Result<(Pick, Spare), String> {
    let pick = review::pick(table, world, id)?;
    let window = pick.windows.iter().find(|window| window.window == "5h");
    let spare = Spare {
        seat: pick.seat.clone(),
        pool: pick.pool.clone(),
        spare: window.and_then(|window| window.spare),
        resets_at: window.and_then(|window| window.resets_at),
    };
    Ok((pick, spare))
}

/// One gated order an idle review reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Reviewed {
    pub run: String,
    pub order: String,
    pub class: String,
    pub profile: Option<String>,
    pub verdict: String,
    pub reason: Option<String>,
    pub report: Option<Value>,
    pub at: u64,
}

/// The orders gated in `project` after `since`, newest first, every class but the reviews' own. A journal written
/// before `since` is not read.
pub fn gated_since(project: &Project, since: u64) -> Vec<Reviewed> {
    let mut out: Vec<Reviewed> = Vec::new();
    for run in project.runs() {
        let journal = project.journal(&run);
        if modified(&journal.path).is_some_and(|at| at <= since) {
            continue;
        }
        let events = journal.read().0;
        let state = replay(&events);
        let mut gated: BTreeMap<String, (u64, String, Option<String>)> = BTreeMap::new();
        for event in &events {
            if let Kind::Gated {
                order,
                verdict,
                reason,
                ..
            } = &event.kind
            {
                if event.at > since {
                    gated.insert(order.clone(), (event.at, verdict.clone(), reason.clone()));
                }
            }
        }
        for (order, (at, verdict, reason)) in gated {
            let Some(known) = state.order(&order) else {
                continue;
            };
            let class = known.seated.class.clone();
            if class == review::CLASS || class == crate::nirvana_knowledge::TEACH_CLASS {
                continue;
            }
            out.push(Reviewed {
                profile: review::subject_profile(&state, &order),
                run: run.clone(),
                order,
                class,
                verdict,
                reason,
                report: known.report.clone(),
                at,
            });
        }
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.order.cmp(&b.order)));
    out
}

// ---- the brief ----

/// The tail of a stream's own strings (`nirvana_lessons::stream_strings`): what a review may quote. A quote inside one
/// string is in the stream, which is what the gate checks.
pub fn digest(stream: &str) -> String {
    let text = nirvana_lessons::stream_strings(stream).join("\n\n---\n\n");
    let skip = text.chars().count().saturating_sub(DIGEST_CHARS);
    text.chars().skip(skip).collect()
}

fn digest_name(reviewed: &Reviewed) -> String {
    format!("{}--{}.txt", reviewed.run, reviewed.order)
}

fn clipped(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(most).collect();
    out.push_str(" ...");
    out
}

/// The idle review's own rules, after `nirvana_review::RULES`.
pub const IDLE_RULES: &str = "## The idle review's own rules

1. herdr was idle with usage to spare, so you read every order gated since the last idle review, not one. Keep only \
what the next order of the same class would use.
2. Each new or patch op names `from`: the run/order below whose stream digest holds its evidence. The lesson's class \
is that order's; herdr sets it.
3. `retire` takes a lesson listed under \"Never helped\": offered at least six times and never followed by a pass \
after a worker opened it. Retire one only when it is wrong or no longer fits; a lesson that is merely narrow is merged.
4. `merge` takes lessons of one class and one profile from the live lessons below, by slug.
";

/// The brief of an idle review of `subjects` (newest first), with `total` gated since `since`.
pub fn idle_brief(
    root: &Path,
    subjects: &[Reviewed],
    total: usize,
    since: u64,
    digests: &Path,
) -> (String, BTreeMap<String, String>) {
    let mut text = format!(
        "# Idle memory review of {} (order LEARN)\n\nherdr has been idle and the cheap seat has usage it would lose at its \
reset. Read what the orders gated since {} did, and keep, fix, merge or retire lessons. Change no file: you can only \
read, and herdr checks each op in your Work Report and applies the ones that hold.\n\n{}\n{IDLE_RULES}\n## The orders \
({} of {total}, newest first)\n\n",
        root.display(),
        nirvana_run::utc(since),
        review::RULES,
        subjects.len()
    );
    for reviewed in subjects {
        text.push_str(&format!(
            "### {}/{} · class {}{} · {}\n\n",
            reviewed.run,
            reviewed.order,
            reviewed.class,
            reviewed
                .profile
                .as_deref()
                .map(|p| format!(" · profile {p}"))
                .unwrap_or_default(),
            reviewed.verdict
        ));
        if let Some(reason) = &reviewed.reason {
            text.push_str(&format!("Why: {}\n\n", clipped(reason, 600)));
        }
        let report = reviewed
            .report
            .as_ref()
            .map(|report| serde_json::to_string_pretty(report).unwrap_or_default())
            .unwrap_or_else(|| "no Work Report".into());
        text.push_str(&format!(
            "Work Report:\n\n```json\n{}\n```\n\nStream digest (the last {DIGEST_CHARS} characters of its tool results \
and text; quote from here): {}\n\n",
            clipped(&report, 4_000),
            digests.join(digest_name(reviewed)).display()
        ));
    }
    let index = nirvana_lessons::read_index(root);
    let mut shas = BTreeMap::new();
    let mut live = String::new();
    let mut never = String::new();
    for (slug, entry) in &index.lessons {
        if nirvana_lessons::live(&entry.counters) {
            let (Some(body), Some(sha)) = (
                nirvana_lessons::body(root, slug),
                nirvana_lessons::body_sha(root, slug),
            ) else {
                continue;
            };
            live.push_str(&format!(
                "### {slug} · class {}{} (base_sha {sha})\n\n```markdown\n{}\n```\n\n",
                entry.class,
                entry
                    .profile
                    .as_deref()
                    .map(|p| format!(" · profile {p}"))
                    .unwrap_or_default(),
                body.trim_end()
            ));
            shas.insert(slug.clone(), sha);
        } else if nirvana_lessons::retirable(&entry.counters) {
            never.push_str(&format!(
                "- {slug} · class {} · offered {} · opened {} · passed after opened 0: {}\n",
                entry.class, entry.counters.offered, entry.counters.opened, entry.description
            ));
        }
    }
    text.push_str("## The live lessons\n\n");
    text.push_str(if live.is_empty() {
        "None yet.\n\n"
    } else {
        &live
    });
    text.push_str("## Never helped\n\n");
    text.push_str(if never.is_empty() {
        "None.\n\n"
    } else {
        &never
    });
    if !never.is_empty() {
        text.push('\n');
    }
    text.push_str(
        "## Your Work Report\n\nstatus `done`; the summary says what you kept, fixed, merged or retired and why, or that \
nothing is worth it; files, tests and gotchas empty lists; and `lesson_ops`, a list of:\n\n\
- `{\"op\": \"new\", \"from\": \"<run>/<order>\", \"name\": \"...\", \"class\": \"<that order's class>\", \"steps\": \"...\", \
\"evidence\": \"a quote from that order's digest\"}`\n\
- `{\"op\": \"patch\", \"from\": \"<run>/<order>\", \"slug\": \"...\", \"base_sha\": \"...\", \"name\": \"...\", \"class\": \
\"<the lesson's class>\", \"steps\": \"the whole new steps\", \"evidence\": \"a quote from that order's digest\"}`\n\
- `{\"op\": \"merge\", \"slugs\": [\"...\", \"...\"], \"name\": \"...\", \"steps\": \"...\"}`\n\
- `{\"op\": \"retire\", \"slug\": \"...\"}`\n\n\
An empty list keeps nothing. A `lesson` of your own is ignored.\n",
    );
    (text, shas)
}

/// What the idle review's gate needs: the orders it read, and the live lessons' shas as its brief showed them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Given {
    orders: Vec<(String, String, String, Option<String>)>,
    shas: BTreeMap<String, String>,
}

fn given_path(project: &Project, run: &str, id: &str) -> PathBuf {
    nirvana_gate::gate_dir(project, run).join(format!("{id}.idle.json"))
}

// ---- queueing ----

/// A fresh run id for an idle review at `now`: `idle-YYYYMMDD-HHMM`, with `-2`, `-3` when taken.
fn run_id(project: &Project, now: u64) -> String {
    let utc = nirvana_run::utc(now);
    let digits: String = utc.chars().filter(char::is_ascii_digit).collect();
    let base = format!(
        "idle-{}-{}",
        digits.get(..8).unwrap_or("0"),
        digits.get(8..12).unwrap_or("0")
    );
    (1..)
        .map(|n| {
            if n == 1 {
                base.clone()
            } else {
                format!("{base}-{n}")
            }
        })
        .find(|id| !project.run_dir(id).exists())
        .unwrap_or(base)
}

fn end_run(project: &Project, run: &str, outcome: &str, why: String) {
    let journal = project.journal(run);
    if replay(&journal.read().0).ended.is_some() {
        return;
    }
    let _ = journal.append(
        nirvana_run::now(),
        Kind::Ended {
            order: None,
            outcome: outcome.into(),
            why: Some(why),
            exit: None,
            pools: Vec::new(),
            pools_basis: None,
        },
    );
}

/// Queues one idle review of `subjects` in `project`: the reclass first (`nirvana_lessons::reclass`, a ledger op of its
/// own), then a run of its own, the digests, the brief, and the order through `nirvana_review::queue`.
#[allow(clippy::too_many_arguments)]
fn queue(
    project: &Project,
    table: &Table,
    world: &World,
    subjects: &[Reviewed],
    total: usize,
    since: u64,
    now: u64,
) -> Result<(String, String, String), String> {
    let root = &project.root;
    let mut said = Vec::new();
    match nirvana_lessons::reclass(root, &nirvana_lessons::router_classes(), false, now) {
        Ok(done) if !done.moved.is_empty() => said.push(format!(
            "reclassed {} lesson(s) as {}",
            done.moved.len(),
            done.op.unwrap_or_default()
        )),
        Ok(_) => {}
        Err(why) => said.push(format!("reclass refused: {why}")),
    }
    let run = run_id(project, now);
    let id = format!("{run}-review");
    let journal = project.journal(&run);
    journal
        .append(
            now,
            Kind::Opened {
                project: root.display().to_string(),
                orchestrator_pane: None,
                auto: true,
                pools: Vec::new(),
                pools_basis: Some("an idle review (order LEARN): no orchestrator".into()),
            },
        )
        .map_err(|err| err.to_string())?;
    let cwd = review::review_cwd(project, &run, &id);
    let digests = cwd.join("digests");
    let made = (|| {
        std::fs::create_dir_all(&digests).map_err(|err| err.to_string())?;
        for reviewed in subjects {
            let stream = nirvana_gate::stream_text(project, &reviewed.run, &reviewed.order);
            std::fs::write(digests.join(digest_name(reviewed)), digest(&stream))
                .map_err(|err| err.to_string())?;
        }
        let (brief, shas) = idle_brief(root, subjects, total, since, &digests);
        std::fs::create_dir_all(nirvana_gate::gate_dir(project, &run))
            .map_err(|err| err.to_string())?;
        write_json(
            &given_path(project, &run, &id),
            &Given {
                orders: subjects
                    .iter()
                    .map(|r| {
                        (
                            r.run.clone(),
                            r.order.clone(),
                            r.class.clone(),
                            r.profile.clone(),
                        )
                    })
                    .collect(),
                shas,
            },
        )
        .map_err(|err| err.to_string())?;
        review::queue_idle(project, &run, &journal, &id, &brief, table, world)
    })();
    match made {
        Ok(line) => {
            said.push(line);
            Ok((run, id, said.join("; ")))
        }
        Err(why) => {
            end_run(project, &run, "failed", format!("not queued: {why}"));
            Err(why)
        }
    }
}

// ---- the tick ----

/// One tick over `projects`: settle each finished idle review, then decide for each project in turn and queue at
/// most one review. Returns a line per project.
pub fn tick_with(projects: &[PathBuf], table: &Table, world: &World) -> Vec<String> {
    let now = world.now;
    let mut lines = Vec::new();
    for root in projects {
        if let Some(line) = settle(&Project::at(root)) {
            lines.push(line);
        }
    }
    let mut global = read_global();
    let busy = busy(world);
    let owner = owner_key(&nirvana_seat::dir());
    let activity = projects
        .iter()
        .filter_map(|root| activity(root).map(|at| (at, root.display().to_string())))
        .max();
    let cap = review::per_day(Some(table));
    let mut queued = false;
    for root in projects {
        let project = Project::at(root);
        let mut state = read_space(root);
        let since = state
            .last
            .as_ref()
            .map_or(now.saturating_sub(LOOKBACK_S), |last| last.through);
        let gated = if !queued && !global.off {
            gated_since(&project, since)
        } else {
            Vec::new()
        };
        let id = format!("{}-review", run_id(&project, now));
        let picked = spare(table, world, &id);
        let facts = Facts {
            now,
            on: !global.off,
            busy: if queued {
                Some("an idle review was queued in this tick".into())
            } else {
                busy.clone()
            },
            owner_key: owner.clone(),
            activity: activity.clone(),
            new_orders: gated.len(),
            review_cap: review::cap_refusal(root, cap, now),
            used_today: used_today(&state, now),
            used_window: picked.as_ref().map_or(0, |(_, spare)| {
                used_window(&global, &spare.pool, spare.resets_at)
            }),
            spare: picked
                .as_ref()
                .map(|(_, spare)| spare.clone())
                .map_err(Clone::clone),
        };
        let decision = decide(&facts);
        let said = match (&decision, &picked) {
            (Decision::Run, Ok((pick, spare))) => {
                let subjects: Vec<Reviewed> = gated.iter().take(MAX_ORDERS).cloned().collect();
                let through = gated.iter().map(|r| r.at).max().unwrap_or(now);
                match queue(&project, table, world, &subjects, gated.len(), since, now) {
                    Ok((run, order, line)) => {
                        queued = true;
                        state.day = now / 86_400;
                        state.count = facts.used_today + 1;
                        state.last = Some(Last {
                            at: now,
                            run,
                            order,
                            seat: format!("{} {}", pick.seat, pick.effort.word()),
                            through,
                            orders: subjects.len(),
                            result: None,
                            done_at: None,
                        });
                        let window = (spare.pool.clone(), spare.resets_at.unwrap_or(now));
                        global.count = facts.used_window + 1;
                        global.window = Some(window);
                        let _ = write_json(&global_path(), &global);
                        format!("queued: {line}")
                    }
                    Err(why) => format!("not queued: {why}"),
                }
            }
            (Decision::Run, Err(why)) => format!("no seat: {why}"),
            (Decision::Skip(why), _) => format!("skipped: {why}"),
        };
        if state.check.as_ref().map(|(_, was)| was) != Some(&said)
            || matches!(decision, Decision::Run)
        {
            state.check = Some((now, said.clone()));
            let _ = write_json(&space_path(root), &state);
        }
        lines.push(format!("{}: {said}", root.display()));
    }
    lines
}

/// A tick on this machine: the known projects, the seat table read without writing, the cached world. One tick at a
/// time (`LOCK`): a second one finds the lock taken and does nothing.
pub fn tick() -> Vec<String> {
    let dir = nirvana_seat::dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return vec!["no nirvana folder".into()];
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(LOCK))
    else {
        return vec!["the idle review lock cannot be opened".into()];
    };
    if file.try_lock().is_err() {
        return vec!["another idle review tick is running".into()];
    }
    let text = std::fs::read_to_string(dir.join(nirvana_seat::SEATS_FILE))
        .unwrap_or_else(|_| nirvana_seat::DEFAULT.to_owned());
    let table = match nirvana_seat::parse(&text) {
        Ok(table) => table,
        Err(why) => return vec![format!("seats.toml is refused: {why}")],
    };
    let world = nirvana_seat::cached_world(&dir, nirvana_run::now());
    let lines = tick_with(&known_projects(), &table, &world);
    let _ = file.unlock();
    lines
}

// ---- settling, and the gate's half ----

/// The project's last idle review, when it is not settled yet: gated (its result read from the gate's axes), delivered
/// (gated now, which records the result through `finished`), or ended with no report.
pub fn settle(project: &Project) -> Option<String> {
    let mut state = read_space(&project.root);
    let last = state.last.clone().filter(|last| last.result.is_none())?;
    let run_state = replay(&project.journal(&last.run).read().0);
    let known = run_state.order(&last.order)?;
    if known.verdict.is_none() && known.phase == Phase::Delivered {
        return Some(match nirvana_gate::gate(project, &last.run, &last.order) {
            Ok(ruling) => format!("{}: gated {}", last.order, ruling.verdict),
            Err(why) => format!("{}: {why}", last.order),
        });
    }
    if known.verdict.is_none() && matches!(known.phase, Phase::Failed | Phase::Interrupted) {
        let why = format!(
            "ended with no report ({})",
            known.outcome.as_deref().unwrap_or(known.phase.word())
        );
        end_run(project, &last.run, "failed", why.clone());
        if let Some(last) = state.last.as_mut() {
            last.result = Some(why.clone());
            last.done_at = Some(nirvana_run::now());
        }
        let _ = write_json(&space_path(&project.root), &state);
        return Some(format!("{}: {why}", last.order));
    }
    None
}

/// What a gated idle review did, in one line: `pass · 2 applied (op-7: new x; ...) · 1 refused`.
pub fn result_line(verdict: &str, axes: &[Axis]) -> String {
    let applied: Vec<&str> = axes
        .iter()
        .filter(|axis| axis.axis == "lesson_op" && axis.mark == Mark::Pass)
        .map(|axis| axis.why.as_str())
        .filter(|line| *line != "nothing to keep")
        .collect();
    let refused = axes
        .iter()
        .filter(|axis| axis.axis == "lesson_op" && axis.mark == Mark::Note)
        .count();
    let mut line = format!("{verdict} · {} applied", applied.len());
    if !applied.is_empty() {
        line.push_str(&format!(" ({})", clipped(&applied.join("; "), 240)));
    }
    if refused > 0 {
        line.push_str(&format!(" · {refused} refused"));
    }
    if let Some(held) = axes
        .iter()
        .find(|axis| axis.mark == Mark::Fail)
        .map(|axis| clipped(&axis.why, 160))
    {
        line.push_str(&format!(" · {held}"));
    }
    line
}

/// The gate's call after it journals an idle review's verdict: the result into the project's state, and the run ended.
pub fn finished(project: &Project, run: &str, order: &str, verdict: &str, axes: &[Axis]) {
    let line = result_line(verdict, axes);
    let mut state = read_space(&project.root);
    if let Some(last) = state.last.as_mut().filter(|last| last.run == run) {
        last.result = Some(line.clone());
        last.done_at = Some(nirvana_run::now());
        let _ = write_json(&space_path(&project.root), &state);
    }
    end_run(project, run, "done", format!("{order} gated: {line}"));
}

/// Each `lesson_ops` item with its `from`, as `nirvana_review::parse_ops` reads one, or `retire`.
enum IdleOp {
    Retire(String),
    Op(Option<String>, Result<Op, String>),
}

fn parse(report: &Value) -> Vec<IdleOp> {
    let Some(items) = report.get("lesson_ops").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
            };
            if item.get("op").and_then(Value::as_str) == Some("retire") {
                return IdleOp::Retire(text("slug").unwrap_or_default());
            }
            let one = serde_json::json!({ "lesson_ops": [item] });
            let op = review::parse_ops(&one)
                .into_iter()
                .next()
                .unwrap_or_else(|| Err("an empty op".into()));
            IdleOp::Op(text("from"), op)
        })
        .collect()
}

/// An idle review's ops, as `nirvana_review::gate_review` hands them over: `root_id` is the review whose brief and
/// given orders count (a reseat child carries its first review's), `by` the order that delivered them.
pub fn apply(
    project: &Project,
    run: &str,
    root_id: &str,
    report: &Value,
    by: &str,
    at: u64,
) -> Applied {
    let given: Given = read_json(&given_path(project, run, root_id));
    let root = &project.root;
    let state = replay(&project.journal(run).read().0);
    let briefs: Vec<String> = [by, root_id]
        .iter()
        .filter_map(|id| state.order(id))
        .map(|known| nirvana_gate::order_text(&known.seated))
        .filter(|text| !text.trim().is_empty())
        .collect();
    let mut streams: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut lines = Vec::new();
    for (n, op) in parse(report).into_iter().enumerate() {
        let n = n + 1;
        let done: Result<String, String> = match op {
            IdleOp::Retire(slug) if slug.is_empty() => Err("a retire names its slug".into()),
            IdleOp::Retire(slug) => nirvana_lessons::retire(root, &slug, by, run, at)
                .map(|id| format!("{id}: retired {slug}")),
            IdleOp::Op(_, Err(why)) => Err(why),
            IdleOp::Op(_, Ok(Op::Merge { slugs, name, steps })) => {
                match slugs.iter().find(|slug| !given.shas.contains_key(*slug)) {
                    Some(slug) => Err(format!("{slug} is not a live lesson this review was given")),
                    None => nirvana_lessons::merge(
                        root,
                        &slugs,
                        &name,
                        &steps,
                        &given.shas,
                        by,
                        run,
                        at,
                    )
                    .map(|(slug, id)| format!("{id}: merged {} into {slug}", slugs.join(", "))),
                }
            }
            IdleOp::Op(from, Ok(op)) => {
                let evidence = match &op {
                    Op::New(lesson) | Op::Patch { lesson, .. } => lesson.evidence.clone(),
                    Op::Merge { .. } => unreachable!("matched above"),
                };
                // The order the op names, else the first given order whose stream holds the quote.
                let named = from.as_deref().and_then(|from| {
                    given
                        .orders
                        .iter()
                        .find(|(r, o, _, _)| format!("{r}/{o}") == from || *o == from)
                });
                let found = named.or_else(|| {
                    given.orders.iter().find(|(r, o, _, _)| {
                        let stream = streams
                            .entry((r.clone(), o.clone()))
                            .or_insert_with(|| nirvana_gate::stream_text(project, r, o));
                        nirvana_lessons::quote_in_stream(stream, &evidence)
                    })
                });
                match (from, found) {
                    (Some(from), None) => Err(format!("{from} is not an order this review read")),
                    (None, None) => {
                        Err("the evidence is in none of the given orders' streams".into())
                    }
                    (_, Some((r, o, class, profile))) => {
                        let stream = streams
                            .entry((r.clone(), o.clone()))
                            .or_insert_with(|| nirvana_gate::stream_text(project, r, o))
                            .clone();
                        let subject = replay(&project.journal(r).read().0);
                        let mut herdr_wrote: Vec<String> = subject
                            .order(o)
                            .map(|known| nirvana_gate::order_text(&known.seated))
                            .into_iter()
                            .collect();
                        herdr_wrote.extend(briefs.iter().cloned());
                        herdr_wrote.retain(|text| !text.trim().is_empty());
                        // The class is the order's (rule 2 of `IDLE_RULES`), whatever the reviewer wrote.
                        let op = match op {
                            Op::New(lesson) => Op::New(nirvana_lessons::Lesson {
                                class: class.clone(),
                                ..lesson
                            }),
                            other => other,
                        };
                        review::apply_review(
                            root,
                            vec![Ok(op)],
                            &stream,
                            &Scope {
                                order: o,
                                class,
                                profile: profile.as_deref(),
                                herdr_wrote,
                            },
                            by,
                            run,
                            at,
                        )
                        .lines
                        .into_iter()
                        .next()
                        .map_or(Err("no line".into()), |(kept, line)| {
                            let line = line
                                .strip_prefix("op 1 refused: ")
                                .map(str::to_owned)
                                .unwrap_or(line);
                            if kept {
                                Ok(format!("{line} (from {r}/{o})"))
                            } else {
                                Err(line)
                            }
                        })
                    }
                }
            }
        };
        lines.push(match done {
            Ok(line) => (true, line),
            Err(why) => (false, format!("op {n} refused: {why}")),
        });
    }
    Applied { lines }
}

// ---- the Learning square's line, and the CLI ----

/// What the Learning square shows of idle review for one project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdleView {
    pub on: bool,
    pub last: Option<Last>,
    pub check: Option<(u64, String)>,
}

pub fn view(root: &Path) -> IdleView {
    let state = read_space(root);
    IdleView {
        on: !read_global().off,
        last: state.last,
        check: state.check,
    }
}

/// The panel's lines: on or off, the last review and what it did, the last check.
pub fn lines(view: &IdleView, now: u64) -> Vec<String> {
    let ago = |at: u64| {
        let seconds = now.saturating_sub(at);
        if seconds < 60 {
            "just now".to_owned()
        } else {
            format!("{} ago", minutes(seconds))
        }
    };
    let mut out = vec![if view.on {
        format!(
            "on: after {} idle, on a seat whose 5h window is {:.0}%+ projected unused and resets within {} \u{b7} \
             herdr lessons idle off",
            minutes(IDLE_AFTER_S),
            SPARE_MIN * 100.0,
            minutes(RESETS_WITHIN_S)
        )
    } else {
        "off \u{b7} herdr lessons idle on".to_owned()
    }];
    match &view.last {
        None => out.push("no idle review yet".into()),
        Some(last) => {
            out.push(format!(
                "last: {} on {} ({}), read {} order{}",
                ago(last.at),
                last.seat,
                last.run,
                last.orders,
                if last.orders == 1 { "" } else { "s" }
            ));
            out.push(match &last.result {
                Some(result) => format!("did: {result}"),
                None => "did: not gated yet".into(),
            });
        }
    }
    if let Some((at, said)) = &view.check {
        out.push(format!("checked {}: {said}", ago(*at)));
    }
    out
}

const USAGE: &str = "usage: herdr lessons idle [status|on|off|tick]
  status  on or off, and each project's last idle review and last check
  on|off  the switch (default on), in <home>/nirvana/idle-review.json
  tick    one check now; the server runs it every 5 minutes";

pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        None | Some("status") => {
            let now = nirvana_run::now();
            println!(
                "idle review is {}",
                if read_global().off { "off" } else { "on" }
            );
            for root in known_projects() {
                println!("{}", root.display());
                for line in lines(&view(&root), now) {
                    println!("  {line}");
                }
            }
            Ok(())
        }
        Some(word @ ("on" | "off")) => {
            set_on(word == "on")?;
            println!("idle review is {word}");
            Ok(())
        }
        Some("tick") => {
            for line in tick() {
                println!("{line}");
            }
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

/// The server's half: every `TICK`, `herdr lessons idle tick` as a child with no window, so a tick never holds the
/// server loop or its locks. Never in a test: a test's `current_exe` is the test binary.
pub fn start() {
    #[cfg(not(test))]
    {
        if crate::nirvana::home().is_none() {
            return;
        }
        let _ = std::thread::Builder::new()
            .name("nirvana-idle-review".into())
            .spawn(|| loop {
                std::thread::sleep(TICK);
                let Ok(exe) = std::env::current_exe() else {
                    return;
                };
                let mut command = std::process::Command::new(exe);
                command
                    .args(["lessons", "idle", "tick"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    command.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
                }
                if let Ok(mut child) = command.spawn() {
                    let _ = child.wait();
                }
            });
    }
}

#[cfg(test)]
mod tests;
