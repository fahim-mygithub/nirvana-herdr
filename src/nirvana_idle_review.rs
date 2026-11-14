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
//!   reading is no review. Review round S2: the week is not lost at that reset, so each week window of the seat must be
//!   read fresh and `WEEK_SPARE_MIN` projected unused. Review round S5: no review when any pool's 5 h use rose across
//!   the last `IDLE_AFTER_S` (the owner's Claude Code outside herdr), or when the seat's pool has no reading that old.
//! - **A normal order with its own record.** It opens a run of its own (`idle-<utc>`), is seated and launched by
//!   `nirvana_review::queue` as every lesson review is, and is gated by `nirvana_review::gate_review`, which hands its
//!   ops to `apply` here: new and patch ops through the same evidence checks against the stream of the order they name,
//!   merge through `nirvana_lessons::merge`, and retire through `nirvana_lessons::retire`; every one a ledger row that
//!   `herdr lessons rollback` undoes. Its run ends when it is gated, and the Learning square shows what it did.
//! - **Caps**: `PER_SPACE_PER_DAY` per project and UTC day, `PER_WINDOW` across the machine per 5 h window of the pool
//!   it runs on, and `lesson_reviews_per_day` still holds (it is a lesson review). Off with `herdr lessons idle off`.
//! - **Plan 73, Hermes's idle queue** (`agent/review_idle_queue.py`; nirvana/plans/2026-11-14-lesson-lifecycle.md).
//!   Its one slot per space is `SpaceState.last`, its newest snapshot is the orders gated since `last.through` read at
//!   queue time, and its settle is `IDLE_AFTER_S`: all here before it. What it adds is the max age. Orders that waited
//!   `MAX_WAIT_S` need only `FORCE_QUIET_S` of quiet, and every other rule still holds. The tick also runs the lessons'
//!   stale and archive sweep (`nirvana_lessons::life::sweep`), which reads files and spends no seat.

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
/// Review round S2: a review's spend counts against the pool's week too, and the week is not lost at the 5 h reset. Each
/// week window of the seat must be read fresh and at least this share of it projected unused at its own reset. A
/// hypothesis: a review is 20k-50k tokens, far under 1% of a week, so the floor is a margin against a wrong projection
/// (the pace is a straight line from the week's start), not the review's size. The Max week on 2026-10-18 (48% used at
/// 32% elapsed, projected 150%) reads 0 and refuses.
pub const WEEK_SPARE_MIN: f32 = 0.25;
/// Review round S5: the pool's 5 h `used` rose by more than this since the newest reading at least `IDLE_AFTER_S` old:
/// someone (the owner's Claude Code outside herdr, a phone session, an orchestrator mid-turn) is spending it. Claude's
/// `/usage` reads whole percents, so any rise it can show is over this.
pub const USAGE_RISE: f32 = 0.005;
/// Review round S3: after a queue that failed, this project waits this long before the next try. A hypothesis: long
/// enough that a corrupt `leases.json` or a held lock costs two tries a day at most (the day cap counts each try), short
/// enough to take the same idle stretch once it is fixed.
pub const BACKOFF_S: u64 = 3_600;
/// Review round S4/N5: a reserved review whose order never reached the journal, or was seated and never started, after
/// this long is settled as stranded. Longer than `TICK_TIMEOUT`, so a slow tick is never taken for a dead one.
pub const STRANDED_S: u64 = 15 * 60;
/// Review round N9: a tick child still running after this is killed; the next one starts on time.
#[cfg_attr(test, allow(dead_code))]
pub const TICK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Plan 73 L2, Hermes's max age. Orders gated this long ago and not yet reviewed need only `FORCE_QUIET_S` of quiet, not
/// `IDLE_AFTER_S`. Hermes forces at 30 min over a 15 s settle, about 120 settles; its numbers are not copied. Here the
/// settle is 30 min and a review also waits for a 5 h window that resets within `RESETS_WITHIN_S` with half of it
/// spare, so 6 h is more than one whole 5 h window gone by with orders waiting. A hypothesis.
pub const MAX_WAIT_S: u64 = 6 * 3_600;
/// Plan 73 L2: the quiet a forced review still needs, two ticks with no owner key and no order write. A forced review
/// skips only the long settle: the lease rule ("never while any order runs anywhere"), the spare, the week, the caps
/// and the use of the pool by anyone all hold, since they are the owner's spend. A hypothesis.
pub const FORCE_QUIET_S: u64 = 10 * 60;
/// Two readings of one pool's window whose resets are this close are the same window.
const SAME_WINDOW_S: u64 = 900;
/// The 5 h samples kept per pool for the S5 check: three idle spans, so a baseline of `IDLE_AFTER_S` ago survives a
/// missed tick or two.
const SAMPLES_KEPT_S: u64 = 3 * IDLE_AFTER_S;
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
    /// Review round S2: each week window of the seat (`week`, `week.fable`) and its projected unused share, `None` when
    /// it is not read fresh.
    pub week: Vec<(String, Option<f32>)>,
}

/// Everything `decide` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub now: u64,
    /// `herdr lessons idle on`.
    pub on: bool,
    /// Review round S1: an input the busy check needs that cannot be read (`leases.json`, a project's runs folder,
    /// `record.jsonl`): no review, since what it would have said is unknown.
    pub doubt: Option<String>,
    /// Why an order runs somewhere: a live lease.
    pub busy: Option<String>,
    /// The newest owner key in any pane; `Err` when a key file cannot be read.
    pub owner_key: Result<Option<u64>, String>,
    /// The newest journal or stream write of any order in any known project, and where.
    pub activity: Option<(u64, String)>,
    /// Review round S5: a pool whose 5 h `used` rose across the last `IDLE_AFTER_S`, and by how much; `Err` when the
    /// seat's pool has no reading from that long ago to compare with.
    pub usage_moved: Result<Option<(String, f32)>, String>,
    /// Review round S3: a queue failed here; no try before this.
    pub backoff_until: Option<u64>,
    /// Orders gated in this project since its last idle review; `None` until the tick counts them, which it does only
    /// when every other fact allows a review (review round N2: the count replays journals).
    pub new_orders: Option<usize>,
    /// Plan 73: when the oldest of those orders was gated; counted with them. Once it is `MAX_WAIT_S` old, the quiet
    /// needed is `FORCE_QUIET_S`.
    pub pending_since: Option<u64>,
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

/// The rule. Each refusal is said, so the Learning square can say why no review ran. The order count comes last: the
/// tick counts orders only when `ready` passes (review round N2).
///
/// Plan 73: once the orders waited `MAX_WAIT_S`, the quiet is `FORCE_QUIET_S`, and a refusal names the rule that holds
/// the review then, not the long quiet it no longer needs.
pub fn decide(facts: &Facts) -> Decision {
    let quiet = if waited_out(facts) {
        FORCE_QUIET_S
    } else {
        IDLE_AFTER_S
    };
    if let Err(why) = ready_within(facts, quiet) {
        return Decision::Skip(why);
    }
    match facts.new_orders {
        Some(n) if n > 0 => Decision::Run,
        _ => Decision::Skip("no order gated here since the last idle review".into()),
    }
}

/// Plan 73: the oldest order waiting for a review was gated `MAX_WAIT_S` ago or more.
pub fn waited_out(facts: &Facts) -> bool {
    facts
        .pending_since
        .is_some_and(|at| facts.now.saturating_sub(at) >= MAX_WAIT_S)
}

/// Every fact but the order count, cheapest first.
pub fn ready(facts: &Facts) -> Result<(), String> {
    ready_within(facts, IDLE_AFTER_S)
}

/// `ready` with `quiet` as the time since the owner's last key and the last order write. Plan 73: `FORCE_QUIET_S` for
/// orders that waited `MAX_WAIT_S`; the use of a pool is still read over `IDLE_AFTER_S` (`usage_moved`).
pub fn ready_within(facts: &Facts, quiet: u64) -> Result<(), String> {
    let skip = |why: String| Err(why);
    if !facts.on {
        return skip("idle review is off (herdr lessons idle on)".into());
    }
    if let Some(why) = &facts.doubt {
        return skip(format!("cannot tell whether an order runs: {why}"));
    }
    if let Some(why) = &facts.busy {
        return skip(format!("an order runs: {why}"));
    }
    match &facts.owner_key {
        Err(why) => return skip(format!("the owner's last key cannot be read: {why}")),
        Ok(Some(at)) if facts.now.saturating_sub(*at) < quiet => {
            return skip(format!(
                "the owner typed {} ago; idle after {}",
                minutes(facts.now.saturating_sub(*at)),
                minutes(quiet)
            ))
        }
        Ok(_) => {}
    }
    if let Some((at, project)) = &facts.activity {
        if facts.now.saturating_sub(*at) < quiet {
            return skip(format!(
                "an order wrote in {project} {} ago; idle after {}",
                minutes(facts.now.saturating_sub(*at)),
                minutes(quiet)
            ));
        }
    }
    match &facts.usage_moved {
        Err(why) => return skip(format!("no usage history to rule out other use: {why}")),
        Ok(Some((pool, rise))) => {
            return skip(format!(
                "{}'s 5h use rose {:.0} points in the last {}: someone is using it",
                crate::nirvana_usage::label(pool),
                rise * 100.0,
                minutes(IDLE_AFTER_S)
            ))
        }
        Ok(None) => {}
    }
    if let Some(until) = facts.backoff_until.filter(|until| *until > facts.now) {
        return skip(format!(
            "the last try here failed; next try in {}",
            minutes(until - facts.now)
        ));
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
    // S2: what is lost at the 5 h reset is the 5 h window's spare only; the week keeps counting the spend.
    if spare.week.is_empty() {
        return skip(format!(
            "{label} has no week reading: the week's room is unknown"
        ));
    }
    for (window, share) in &spare.week {
        let Some(share) = share else {
            return skip(format!(
                "{label}'s {window} reading is missing or old: no review on a guess"
            ));
        };
        if *share < WEEK_SPARE_MIN {
            return skip(format!(
                "{label}'s {window} is {:.0}% projected unused, under {:.0}%: the week has no room",
                share * 100.0,
                WEEK_SPARE_MIN * 100.0
            ));
        }
    }
    Ok(())
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
    /// Review round S3: a queue failed; no try before this.
    #[serde(default)]
    pub backoff_until: Option<u64>,
}

/// One 5 h reading of a pool as a tick saw it (review round S5).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub read_at: u64,
    pub used: f32,
    #[serde(default)]
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Global {
    #[serde(default)]
    pub off: bool,
    /// Review round N3: per pool, the reset of the window counted and the reviews in it. One pool's count no longer
    /// resets another's.
    #[serde(default)]
    pub windows: BTreeMap<String, (u64, u32)>,
    /// Review round S5: per pool, the 5 h readings of the last `SAMPLES_KEPT_S`.
    #[serde(default)]
    pub samples: BTreeMap<String, Vec<Sample>>,
}

pub fn space_path(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("runs").join(STATE)
}

pub fn global_path() -> PathBuf {
    nirvana_seat::dir().join(GLOBAL)
}

fn read_json<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    read_checked(path).unwrap_or_default()
}

/// Review B5: a state file as it is, its default only when it is not there. Any other read error (an editor or backup
/// tool holding it, a sharing violation on Windows) and any text that does not parse is `Err`: the module's rule is
/// "any fact it cannot read is a reason not to run", and a damaged `idle-review.json` read as its default switched
/// idle review back on (`off` defaults to false) and reset its per-window caps.
fn read_checked<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|err| format!("{} does not parse: {err}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(format!("{} cannot be read: {err}", path.display())),
    }
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

/// Review round N4: the one writer of a state file. Read, change and write under `<file>.lock`, held only for that, so
/// the gate's `finished` and a tick never put back a copy read before the other wrote. Not the tick's own `LOCK`: the
/// tick's settle gates a review, whose `finished` comes here in the same process.
fn change_json<T, R>(path: &Path, change: impl FnOnce(&mut T) -> R) -> std::io::Result<R>
where
    T: Default + Serialize + for<'de> Deserialize<'de>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(PathBuf::from(format!("{}.lock", path.display())))?;
    file.lock()?;
    // Review B5: a file there that does not read is never written over with a default. It stays as it is (a copy goes
    // to `<file>.bad` for the owner) and the change is refused, so every tick reads a doubt until it is fixed or
    // deleted.
    let mut value: T = match read_checked(path) {
        Ok(value) => value,
        Err(why) => {
            if path.exists() {
                let _ = std::fs::copy(path, PathBuf::from(format!("{}.bad", path.display())));
            }
            return Err(std::io::Error::other(format!(
                "{why}; fix it or delete it, herdr will not write over it"
            )));
        }
    };
    let out = change(&mut value);
    write_json(path, &value)?;
    drop(file);
    Ok(out)
}

pub fn change_space<R>(
    root: &Path,
    change: impl FnOnce(&mut SpaceState) -> R,
) -> std::io::Result<R> {
    change_json(&space_path(root), change)
}

pub fn change_global<R>(change: impl FnOnce(&mut Global) -> R) -> std::io::Result<R> {
    change_json(&global_path(), change)
}

pub fn used_today(state: &SpaceState, now: u64) -> u32 {
    if state.day == now / 86_400 {
        state.count
    } else {
        0
    }
}

pub fn used_window(global: &Global, pool: &str, resets_at: Option<u64>) -> u32 {
    match (global.windows.get(pool), resets_at) {
        (Some((at, count)), Some(resets)) if at.abs_diff(resets) <= SAME_WINDOW_S => *count,
        _ => 0,
    }
}

/// One more review in `pool`'s window that resets at `resets_at`.
fn count_window(global: &mut Global, pool: &str, resets_at: u64) {
    let count = used_window(global, pool, Some(resets_at)) + 1;
    global.windows.insert(pool.to_owned(), (resets_at, count));
}

/// `herdr lessons idle on|off`.
pub fn set_on(on: bool) -> std::io::Result<()> {
    change_global(|global| global.off = !on)
}

/// Review round S5: each pool's fresh 5 h reading into the global samples, the old ones dropped.
pub fn record_samples(global: &mut Global, world: &World) {
    for reading in &world.readings {
        let Some(used) = reading.used else { continue };
        if reading.window != "5h"
            || crate::nirvana_usage::freshness(reading, world.now)
                != crate::nirvana_usage::Freshness::Fresh
        {
            continue;
        }
        let samples = global.samples.entry(reading.pool.clone()).or_default();
        if samples
            .last()
            .is_none_or(|last| last.read_at < reading.read_at)
        {
            samples.push(Sample {
                read_at: reading.read_at,
                used,
                resets_at: reading.resets_at,
            });
        }
    }
    let keep_from = world.now.saturating_sub(SAMPLES_KEPT_S);
    for samples in global.samples.values_mut() {
        samples.retain(|sample| sample.read_at >= keep_from);
    }
    global.samples.retain(|_, samples| !samples.is_empty());
}

/// Review round S5: whether a pool's 5 h use rose across the last `IDLE_AFTER_S`. The seat's own pool must have a
/// sample at least that old in the same window (else `Err`: no history is no review); any other pool with one counts
/// too, since its rise says the owner is at work.
pub fn usage_moved(
    global: &Global,
    world: &World,
    pool: &str,
) -> Result<Option<(String, f32)>, String> {
    let before = world.now.saturating_sub(IDLE_AFTER_S);
    let rise = |pool: &str| -> Option<f32> {
        let now = world.readings.iter().find(|reading| {
            reading.pool == pool
                && reading.window == "5h"
                && reading.used.is_some()
                && crate::nirvana_usage::freshness(reading, world.now)
                    == crate::nirvana_usage::Freshness::Fresh
        })?;
        let then = global
            .samples
            .get(pool)?
            .iter()
            .filter(|sample| sample.read_at <= before)
            .filter(|sample| match (sample.resets_at, now.resets_at) {
                (Some(a), Some(b)) => a.abs_diff(b) <= SAME_WINDOW_S,
                _ => false,
            })
            .max_by_key(|sample| sample.read_at)?;
        Some(now.used.unwrap_or(0.0) - then.used)
    };
    let Some(own) = rise(pool) else {
        return Err(format!(
            "no reading of {}'s 5h window from {} ago in this window yet",
            crate::nirvana_usage::label(pool),
            minutes(IDLE_AFTER_S)
        ));
    };
    if own > USAGE_RISE {
        return Ok(Some((pool.to_owned(), own)));
    }
    for other in global.samples.keys().filter(|other| *other != pool) {
        if let Some(up) = rise(other).filter(|up| *up > USAGE_RISE) {
            return Ok(Some((other.clone(), up)));
        }
    }
    Ok(None)
}

// ---- the facts, from files ----

/// The projects herdr knows: each that an ended order's `record.jsonl` line names.
pub fn known_projects() -> Result<Vec<PathBuf>, String> {
    known_projects_in(
        &nirvana_seat::dir().join("record.jsonl"),
        &std::env::temp_dir(),
    )
}

/// `known_projects` from `record`. Review round N3: a root under `temp` (a check's fixture that reached the record) is
/// never a project to spend a review on, nor a root with no `.nirvana` folder now; one root spelled in two cases is one
/// on Windows. Review round S1: a record that exists and cannot be read is `Err`, never no projects.
pub fn known_projects_in(record: &Path, temp: &Path) -> Result<Vec<PathBuf>, String> {
    let text = match std::fs::read_to_string(record) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(format!("{}: {err}", record.display())),
    };
    let fold = |path: &Path| {
        let text = path.display().to_string().replace('/', "\\");
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    let temp = fold(temp);
    let mut out: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
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
        let key = fold(&root);
        if root.as_os_str().is_empty()
            || (!temp.is_empty() && key.starts_with(&temp))
            || !root.join(nirvana_run::DIR).is_dir()
            || !seen.insert(key)
        {
            continue;
        }
        out.push(root);
    }
    out.sort();
    Ok(out)
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

/// The newest write of any order in `root`: each run's journal and every stream file. Review round S1: a runs or
/// streams folder that exists and cannot be listed is `Err` (a doubt), never quiet; a folder that is not there is.
pub fn activity(root: &Path) -> Result<Option<u64>, String> {
    let list = |dir: &Path| -> Result<Vec<PathBuf>, String> {
        match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .map(|entry| entry.path())
                        .map_err(|err| format!("{}: {err}", dir.display()))
                })
                .collect(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(err) => Err(format!("{}: {err}", dir.display())),
        }
    };
    let mut newest = None;
    for path in list(&Project::at(root).dir().join("runs"))? {
        if !path.is_dir() {
            continue;
        }
        newest = newest.max(modified(&path.join(nirvana_run::JOURNAL)));
        for stream in list(&path.join("streams"))? {
            newest = newest.max(modified(&stream));
        }
    }
    Ok(newest)
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
        // `used` is `None` unless the reading is fresh (`nirvana_seat` reads only fresh ones), so is `spare`.
        week: pick
            .windows
            .iter()
            .filter(|window| window.window.split('.').next() == Some("week"))
            .map(|window| (window.window.clone(), window.used.and(window.spare)))
            .collect(),
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

/// Queues one idle review of `subjects` in `project` as order `id` of run `run` (reserved by the tick before this): the
/// reclass first (`nirvana_lessons::reclass`, a ledger op of its own, only lessons herdr wrote last), then the run, the
/// digests, the brief, and the order through `nirvana_review::queue_idle`.
#[allow(clippy::too_many_arguments)]
fn queue(
    project: &Project,
    table: &Table,
    world: &World,
    run: &str,
    id: &str,
    subjects: &[Reviewed],
    total: usize,
    since: u64,
    now: u64,
) -> Result<String, String> {
    let root = &project.root;
    let mut said = Vec::new();
    match nirvana_lessons::reclass(root, &nirvana_lessons::router_classes(), false, false, now) {
        Ok(done) => {
            if !done.moved.is_empty() {
                said.push(format!(
                    "reclassed {} lesson(s) as {}",
                    done.moved.len(),
                    done.op.unwrap_or_default()
                ));
            }
            // Review round S6: a lesson the owner edited is left; the line says so, so the square shows it.
            let edited = done
                .left
                .iter()
                .filter(|(_, _, why)| why.starts_with(nirvana_lessons::EDITED))
                .count();
            if edited > 0 {
                said.push(format!(
                    "{edited} lesson(s) left in their class: {} (herdr lessons reclass --dry-run lists them)",
                    nirvana_lessons::EDITED
                ));
            }
        }
        Err(why) => said.push(format!("reclass refused: {why}")),
    }
    let (run, id) = (run.to_owned(), id.to_owned());
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
            Ok(said.join("; "))
        }
        Err(why) => {
            end_run(project, &run, "failed", format!("not queued: {why}"));
            Err(why)
        }
    }
}

// ---- the tick ----

/// One tick over `projects`: settle each finished idle review, then decide for each project in turn and queue at
/// most one review. `doubt` is an input the tick could not read (`leases.json`: review round S1). Returns a line per
/// project.
pub fn tick_with(
    projects: &[PathBuf],
    table: &Table,
    world: &World,
    doubt: Option<String>,
) -> Vec<String> {
    let now = world.now;
    let mut lines = Vec::new();
    for root in projects {
        if let Some(line) = settle(&Project::at(root), world, doubt.is_none()) {
            lines.push(line);
        }
        // Plan 73 L3: stale and archive by time. Files only, no seat, so it runs whatever the idle facts say; a line
        // only when it archived or failed, so a quiet tick says nothing new.
        let swept = nirvana_lessons::life::sweep(root, now, false);
        if !swept.archived.is_empty() || !swept.failed.is_empty() {
            lines.push(format!(
                "{}: {}",
                root.display(),
                nirvana_lessons::life::sweep_report(&swept, false)
                    .trim_end()
                    .replace('\n', " ")
            ));
        }
    }
    // S5: this tick's readings join the history before it is read.
    let mut doubt = doubt;
    // Review B5: a global file that does not read is a doubt, never its default (`off` false, caps zero).
    let global = match change_global(|global| {
        record_samples(global, world);
        global.clone()
    }) {
        Ok(global) => global,
        Err(err) => {
            doubt = doubt.or(Some(err.to_string()));
            Global::default()
        }
    };
    let busy = busy(world);
    let owner = owner_key(&nirvana_seat::dir());
    let mut activity = None;
    for root in projects {
        match self::activity(root) {
            Ok(at) => activity = activity.max(at.map(|at| (at, root.display().to_string()))),
            Err(why) => doubt = doubt.or(Some(why)),
        }
    }
    // N2: one pick for every project (the world is the same; no lease is the review's yet), and the S5 history for its
    // pool.
    let picked = spare(table, world, "idle-review");
    let moved = match &picked {
        Ok((_, spare)) => usage_moved(&global, world, &spare.pool),
        Err(why) => Err(why.clone()),
    };
    let cap = review::per_day(Some(table));
    let mut queued = false;
    for root in projects {
        let project = Project::at(root);
        // Review B5: the space's own file likewise (its day count and backoff).
        let (state, space_doubt) = match read_checked::<SpaceState>(&space_path(root)) {
            Ok(state) => (state, None),
            Err(why) => (SpaceState::default(), Some(why)),
        };
        let since = state
            .last
            .as_ref()
            .map_or(now.saturating_sub(LOOKBACK_S), |last| last.through);
        let mut facts = Facts {
            now,
            on: !global.off,
            doubt: doubt.clone().or(space_doubt),
            busy: if queued {
                Some("an idle review was queued in this tick".into())
            } else {
                busy.clone()
            },
            owner_key: owner.clone(),
            activity: activity.clone(),
            usage_moved: moved.clone(),
            backoff_until: state.backoff_until,
            new_orders: None,
            pending_since: None,
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
        // N2: the journals are replayed only when every cheaper fact allows a review. Plan 73: or would with the short
        // quiet of a forced review. That adds at most the four ticks of a quiet spell that fall between `FORCE_QUIET_S`
        // and `IDLE_AFTER_S`, and only while the rest of `ready` holds.
        let full = ready(&facts).is_ok();
        let gated = if full || ready_within(&facts, FORCE_QUIET_S).is_ok() {
            gated_since(&project, since)
        } else {
            Vec::new()
        };
        facts.new_orders = Some(gated.len());
        facts.pending_since = gated.iter().map(|r| r.at).min();
        let decision = decide(&facts);
        let waited = (!full)
            .then(|| {
                facts
                    .pending_since
                    .map(|at| minutes(now.saturating_sub(at)))
            })
            .flatten();
        let said = match (&decision, &picked) {
            (Decision::Run, Ok((pick, spare))) => {
                queued = true;
                let subjects: Vec<Reviewed> = gated.iter().take(MAX_ORDERS).cloned().collect();
                let through = gated.iter().map(|r| r.at).max().unwrap_or(now);
                let run = run_id(&project, now);
                let id = format!("{run}-review");
                // S3/S4: the reservation is written before anything is queued: the day's count, the window's count and
                // `last` with the run. A tick that dies after this has spent its count and moved `through`, so the
                // next tick cannot run the same orders again; `settle` ends a review that never reached the journal.
                let reserved = change_space(root, |state| {
                    if state.day != now / 86_400 {
                        state.day = now / 86_400;
                        state.count = 0;
                    }
                    state.count += 1;
                    state.last = Some(Last {
                        at: now,
                        run: run.clone(),
                        order: id.clone(),
                        seat: format!("{} {}", pick.seat, pick.effort.word()),
                        through,
                        orders: subjects.len(),
                        result: None,
                        done_at: None,
                    });
                })
                .and_then(|()| {
                    change_global(|global| {
                        count_window(global, &spare.pool, spare.resets_at.unwrap_or(now))
                    })
                });
                // A test stands in for a tick that dies here (S4).
                #[cfg(test)]
                if reserved.is_ok() && DIE_AFTER_RESERVE.get() {
                    return lines;
                }
                match reserved {
                    Err(err) => format!("not queued: the reservation cannot be written: {err}"),
                    Ok(()) => match queue(
                        &project,
                        table,
                        world,
                        &run,
                        &id,
                        &subjects,
                        gated.len(),
                        since,
                        now,
                    ) {
                        Ok(line) => match &waited {
                            Some(waited) => format!(
                                "queued after orders waited {waited}, on {} of quiet: {line}",
                                minutes(FORCE_QUIET_S)
                            ),
                            None => format!("queued: {line}"),
                        },
                        Err(why) => {
                            // S3: the try stays counted; its orders are read again after the back-off.
                            let _ = change_space(root, |state| {
                                if let Some(last) = state.last.as_mut().filter(|l| l.run == run) {
                                    last.result = Some(format!("not queued: {why}"));
                                    last.done_at = Some(now);
                                    last.through = since;
                                }
                                state.backoff_until = Some(now + BACKOFF_S);
                            });
                            format!("not queued: {why}; next try in {}", minutes(BACKOFF_S))
                        }
                    },
                }
            }
            (Decision::Run, Err(why)) => format!("no seat: {why}"),
            (Decision::Skip(why), _) => format!("skipped: {why}"),
        };
        let run_now = matches!(decision, Decision::Run);
        let _ = change_space(root, |state| {
            if state.check.as_ref().map(|(_, was)| was) != Some(&said) || run_now {
                state.check = Some((now, said.clone()));
            }
        });
        lines.push(format!("{}: {said}", root.display()));
    }
    lines
}

/// A tick on this machine: the known projects, the seat table read without writing, the cached world. One tick at a
/// time (`LOCK`): a second one finds the lock taken and does nothing.
pub fn tick() -> Vec<String> {
    match known_projects() {
        Ok(projects) => tick_in(&nirvana_seat::dir(), &projects),
        Err(why) => vec![format!(
            "the project list cannot be read ({why}): no review"
        )],
    }
}

/// `tick` over `projects`, with the nirvana folder `dir`.
pub fn tick_in(dir: &Path, projects: &[PathBuf]) -> Vec<String> {
    if std::fs::create_dir_all(dir).is_err() {
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
    // S1: `cached_world` reads an unreadable `leases.json` as no leases, which is "nothing runs". The tick reads it
    // itself and fails closed; `cached_world` is shared, so it is left as it is.
    let mut world = nirvana_seat::cached_world(dir, nirvana_run::now());
    let doubt = match nirvana_seat::load_leases(&dir.join(nirvana_seat::LEASES_FILE)) {
        Ok(leases) => {
            world.leases = leases.leases;
            None
        }
        Err(why) => Some(why),
    };
    let lines = tick_with(projects, &table, &world, doubt);
    let _ = file.unlock();
    lines
}

// ---- settling, and the gate's half ----

/// The project's last idle review, when it is not settled yet: gated (its result read from the gate's axes), delivered
/// (gated now, which records the result through `finished`), ended with no report, or stranded (review round S4/N5:
/// reserved and never journaled, or seated and never started, `STRANDED_S` on with no live lease). `leases_known`:
/// `world.leases` was read; without it a seated review is never called stranded.
pub fn settle(project: &Project, world: &World, leases_known: bool) -> Option<String> {
    let state = read_space(&project.root);
    let last = state.last.clone().filter(|last| last.result.is_none())?;
    let run_state = replay(&project.journal(&last.run).read().0);
    let stale = world.now.saturating_sub(last.at) > STRANDED_S;
    let Some(known) = run_state.order(&last.order) else {
        if !stale {
            return None;
        }
        let why = "never seated: the tick that reserved it stopped first".to_owned();
        if project.journal(&last.run).path.is_file() {
            end_run(project, &last.run, "failed", why.clone());
        }
        record(project, &last.run, &why, world.now);
        return Some(format!("{}: {why}", last.order));
    };
    if known.verdict.is_none() && known.phase == Phase::Delivered {
        return Some(match nirvana_gate::gate(project, &last.run, &last.order) {
            Ok(ruling) => format!("{}: gated {}", last.order, ruling.verdict),
            Err(why) => format!("{}: {why}", last.order),
        });
    }
    let never_started = known.phase == Phase::Seated
        && stale
        && leases_known
        && !world
            .leases
            .iter()
            .any(|lease| lease.order == last.order && lease.live(world.now));
    if known.verdict.is_none()
        && (matches!(known.phase, Phase::Failed | Phase::Interrupted) || never_started)
    {
        let why = if never_started {
            "seated and never started, its lease ended".to_owned()
        } else {
            format!(
                "ended with no report ({})",
                known.outcome.as_deref().unwrap_or(known.phase.word())
            )
        };
        end_run(project, &last.run, "failed", why.clone());
        record(project, &last.run, &why, world.now);
        return Some(format!("{}: {why}", last.order));
    }
    None
}

/// `last.result` for the review of `run`, under the state file's lock (N4).
fn record(project: &Project, run: &str, line: &str, at: u64) {
    let _ = change_space(&project.root, |state| {
        if let Some(last) = state.last.as_mut().filter(|last| last.run == run) {
            last.result = Some(line.to_owned());
            last.done_at = Some(at);
        }
    });
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
    record(project, run, &line, nirvana_run::now());
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
                // The order the op names, else the first given order whose stream holds the quote. Review round N8: a
                // bare order id that two given runs share names neither.
                let matching: Vec<_> = from
                    .as_deref()
                    .map(|from| {
                        given
                            .orders
                            .iter()
                            .filter(|(r, o, _, _)| format!("{r}/{o}") == from || *o == from)
                            .collect()
                    })
                    .unwrap_or_default();
                if matching.len() > 1 {
                    lines.push((
                        false,
                        format!(
                            "op {n} refused: {} names {} orders this review read; name it <run>/<order>",
                            from.as_deref().unwrap_or_default(),
                            matching.len()
                        ),
                    ));
                    continue;
                }
                let named = matching.first().copied();
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
            "on: after {} idle, on a seat whose 5h window is {:.0}%+ projected unused and resets within {}, \
             and whose week is {:.0}%+ \u{b7} herdr lessons idle off",
            minutes(IDLE_AFTER_S),
            SPARE_MIN * 100.0,
            minutes(RESETS_WITHIN_S),
            WEEK_SPARE_MIN * 100.0
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
            for root in known_projects().map_err(std::io::Error::other)? {
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
                // N9: a hung tick (a stuck gate in `settle`) is killed at `TICK_TIMEOUT`; it is this thread's own
                // child, nothing else.
                if let Ok(mut child) = command.spawn() {
                    let started = std::time::Instant::now();
                    loop {
                        match child.try_wait() {
                            Ok(None) if started.elapsed() < TICK_TIMEOUT => {
                                std::thread::sleep(Duration::from_secs(1));
                            }
                            Ok(None) => {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                            _ => break,
                        }
                    }
                }
            });
    }
}

#[cfg(test)]
thread_local! {
    /// Review round S4: a test makes the tick stop right after its reservation, as a killed tick would.
    static DIE_AFTER_RESERVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
mod tests;
