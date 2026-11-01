//! Nirvana fork, plan 36: the Learning square's view of the self-learning loop, read off the draw path.
//! (nirvana/plans/2026-10-31-36-learning-square.md §1, §2)
//!
//! The owner, 2026-10-31 (plan 36 L1): "Also for the learning scheme we brought from hermes ensure we have a module
//! that is tracking this beor ethe git module and ater the preload module."
//!
//! - **What it reads** is plan 33's and 34a's own state, through their own readers: `nirvana_lessons::{read_index,
//!   ledger, live, stats_lines}`, `nirvana_knowledge::{cards, read_index, ledger, known}`, `nirvana_review::{crowded,
//!   per_day}`, and the run journals' `seated` and `gated` events of class `lesson-review`. No second definition of
//!   a lesson's liveness, a card's knownness or the cap.
//! - **It writes nothing.** The seat table is parsed from its file (or herdr's default), never through
//!   `nirvana_seat::table`, whose `read_or_create` writes the file when it is missing.
//! - **The project is the one `herdr lessons` reads** (`project_root`): the pane's git top level, or the project of a
//!   worker's worktree; found once per folder, on a thread (review S2).
//! - **A cached view, like `nirvana_git::cached`.** `cached` returns the last view and, at most every `REFRESH`,
//!   starts a thread that stats the folders the view is made from; only a changed stat (mtime or size), or a new UTC
//!   day, reads them again, and then only the journals and `knowledge/` folders that changed (`Memo`, review S4).
//!   Drawing never reads a file.
//! - **Today's count is the cap's** (`nirvana_review::used_today`); the journals say what each review came to, a
//!   reseat or send-back child counted with its parent (review S1).
//! - **Undo** (`start_undo`) runs `nirvana_lessons::rollback` or `nirvana_knowledge::rollback`, the functions behind
//!   `herdr lessons rollback` and `herdr kb rollback`, on a thread: both take a file lock a running gate may hold.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, UNIX_EPOCH};

use serde_json::Value;

use crate::nirvana_gate::{Axis, Mark};
use crate::nirvana_knowledge as knowledge;
use crate::nirvana_lessons::{self, Counters, Origin};
use crate::nirvana_profiles as profile_files;
use crate::nirvana_review as review;
use crate::nirvana_run::{Kind, Project};
use crate::nirvana_seat as seat;

/// §2: "at most every few seconds". Each check is a stat of each file the view is made from; a read follows only a
/// change. A hypothesis: 3 s is the Jobs square's idle poll (2 s running) rounded up, and a lesson op lands once per
/// gated order.
pub const REFRESH: Duration = Duration::from_secs(3);
/// A folder whose square was not drawn for this long is dropped from the cache, as the Git square's are.
const FORGET_AFTER: Duration = Duration::from_secs(60);
/// §1.2 "the last ledger rows": the newest this many, lessons and cards together.
pub const CHANGES_SHOWN: usize = 12;
/// The refusal reasons the Today section names; the rest are counted in one "other".
pub const REASONS_SHOWN: usize = 4;
/// The axes a review's gate journals when it applies nothing before trying any op (`nirvana_review::held_back`,
/// `nirvana_knowledge::held_back`): a report that is not `done`, or a folder changed outside herdr.
pub const HELD_AXES: [&str; 3] = ["status", "lessons_folder", "knowledge_folder"];
/// The axes a review's gate journals one op each under (`nirvana_review::gate_review`).
pub const OP_AXES: [&str; 2] = ["lesson_op", "card_op"];
/// The pass line a review with no op gets; it is not an op.
const NOTHING_KEPT: &str = "nothing to keep";

/// One lesson, from `index.json`, with `live` as the offer reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LessonRow {
    pub slug: String,
    pub name: String,
    pub origin: Origin,
    pub class: String,
    pub profile: Option<String>,
    pub counters: Counters,
    pub live: bool,
}

/// Which ledger a change is in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// `.nirvana/lessons/ledger.jsonl`.
    Lessons,
    /// A folder profile's `knowledge/ledger.jsonl`.
    Profile(String),
}

/// One ledger row, lessons or cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub source: Source,
    pub id: String,
    pub op: String,
    /// Every slug the op touched, in the ledger's order.
    pub slugs: Vec<String>,
    /// The order whose Work Report carried the op, or `herdr`.
    pub by: String,
    pub at: u64,
    /// A later rollback names this op (`of`): the rollbacks refuse it.
    pub undone: bool,
}

impl Change {
    /// Whether `herdr lessons rollback` / `herdr kb rollback` would try it: a rollback row and an op already rolled
    /// back are refused there with these words.
    pub fn refusal(&self) -> Option<String> {
        if self.op == "rollback" {
            Some(format!(
                "{} is a rollback, which herdr does not undo",
                self.id
            ))
        } else if self.undone {
            Some(format!("{} is rolled back already", self.id))
        } else {
            None
        }
    }
}

/// One profile in `.nirvana/profiles/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileRow {
    pub name: String,
    /// `None` for a profile that is one file, which keeps no cards.
    pub knowledge: Option<PathBuf>,
    pub cards: usize,
    /// Cards that parse but herdr does not know as they stand (`herdr profile check <name> --adopt`).
    pub unknown: usize,
    /// Cards whose verdict is `reject` or `dead-end`.
    pub warnings: usize,
    /// Cards the card parser refuses.
    pub refused: usize,
}

/// How a review's gate ruled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ruled {
    /// Seated, not gated yet.
    Waiting,
    Done,
    /// Nothing tried: the report was not `done`, or a folder changed outside herdr.
    HeldBack,
    Failed,
}

/// One lesson review or merge seated today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub order: String,
    pub ruled: Ruled,
    pub gated_at: Option<u64>,
    pub applied: usize,
    pub refused: usize,
}

/// §1.2 "Today".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Today {
    /// The UTC day, as days since 1970, which the cap counts (`nirvana_review::cap_refusal`).
    pub day: u64,
    /// The reviews the cap counted today (`nirvana_review::used_today`): the square's number, held against `cap`.
    pub used: u32,
    /// The reviews seated today, from the journals: what the panel says each came to.
    pub reviews: Vec<Review>,
    /// `lesson_reviews_per_day`.
    pub cap: u32,
    /// Each refusal reason with its count, most first.
    pub reasons: Vec<(String, usize)>,
}

impl Today {
    pub fn count(&self, ruled: Ruled) -> usize {
        self.reviews.iter().filter(|r| r.ruled == ruled).count()
    }

    pub fn gated(&self) -> usize {
        self.reviews.len() - self.count(Ruled::Waiting)
    }

    pub fn applied(&self) -> usize {
        self.reviews.iter().map(|r| r.applied).sum()
    }

    pub fn refused(&self) -> usize {
        self.reviews.iter().map(|r| r.refused).sum()
    }

    /// §1.1's mark: the last review gated today was held back, failed, or had every op refused.
    pub fn last_went_wrong(&self) -> bool {
        self.reviews
            .iter()
            .filter(|r| r.gated_at.is_some())
            .max_by_key(|r| r.gated_at)
            .is_some_and(|r| {
                matches!(r.ruled, Ruled::HeldBack | Ruled::Failed)
                    || (r.refused > 0 && r.applied == 0)
            })
    }
}

/// Everything the square and the panel show for one project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LearningView {
    pub root: PathBuf,
    /// Live first, then by slug.
    pub lessons: Vec<LessonRow>,
    pub live: usize,
    /// `herdr lessons stats`, line for line.
    pub stats: Vec<String>,
    pub today: Today,
    /// Newest first, at most `CHANGES_SHOWN`.
    pub changes: Vec<Change>,
    pub profiles: Vec<ProfileRow>,
    /// `nirvana_review::crowded`: `(class, profile, live count)` a merge at `herdr run end` takes.
    pub waiting: Vec<(String, Option<String>, usize)>,
}

impl LearningView {
    /// The file Enter opens for a change: the first of its slugs whose file is there, as the view was read.
    pub fn change_file(&self, change: &Change) -> Option<PathBuf> {
        let dir = match &change.source {
            Source::Lessons => nirvana_lessons::dir(&self.root),
            Source::Profile(name) => self
                .profiles
                .iter()
                .find(|p| &p.name == name)?
                .knowledge
                .clone()?,
        };
        change
            .slugs
            .iter()
            .map(|slug| dir.join(format!("{slug}.md")))
            .find(|file| file.is_file())
    }

    pub fn lesson_file(&self, lesson: &LessonRow) -> PathBuf {
        nirvana_lessons::dir(&self.root).join(format!("{}.md", lesson.slug))
    }
}

fn day_of(at: u64) -> u64 {
    at / 86_400
}

/// A refusal reason with its numbers read as `N`, so "the evidence is 5 characters" and "... 7 characters" count as
/// one reason.
pub fn reason_key(why: &str) -> String {
    let mut out = String::new();
    let mut in_digits = false;
    for c in why.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('N');
            }
            in_digits = true;
        } else {
            out.push(c);
            in_digits = false;
        }
    }
    out
}

/// What one gated event of a review says.
fn ruling(verdict: &str, axes: &[Value]) -> (Ruled, usize, usize, Vec<String>) {
    let axes: Vec<Axis> = axes
        .iter()
        .filter_map(|axis| serde_json::from_value(axis.clone()).ok())
        .collect();
    let held = axes
        .iter()
        .any(|a| a.mark == Mark::Fail && HELD_AXES.contains(&a.axis.as_str()));
    let ruled = if nirvana_lessons::PASS_WORDS.contains(&verdict) {
        Ruled::Done
    } else if held {
        Ruled::HeldBack
    } else {
        Ruled::Failed
    };
    let (mut applied, mut reasons) = (0, Vec::new());
    for axis in axes.iter().filter(|a| OP_AXES.contains(&a.axis.as_str())) {
        match axis.mark {
            Mark::Pass if axis.why != NOTHING_KEPT => applied += 1,
            Mark::Note => reasons.push(
                axis.why
                    .split_once("refused: ")
                    .map_or(axis.why.as_str(), |(_, why)| why)
                    .to_owned(),
            ),
            _ => {}
        }
    }
    (ruled, applied, reasons.len(), reasons)
}

/// What one journal says about lesson reviews. Review S4: kept in `Memo` by the journal's stat, so the journal a
/// running order appends to is the only one parsed again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Digest {
    /// `(order, at, parent)` of each `seated` event of class `lesson-review`; `parent` only for a reseat or
    /// send-back child, which carries its parent's review on and is not a review of its own.
    seated: Vec<(String, u64, Option<String>)>,
    /// `(order, at, verdict, axes)` of each `gated` event of an order seated above it.
    gated: Vec<(String, u64, String, Vec<Value>)>,
}

/// The `why` of a child that goes on with its parent's order (`nirvana_worker`'s reseat, `nirvana_gate::send_back`).
const CARRIES_ON: [&str; 2] = ["reseat", "send_back"];

fn digest(path: &Path) -> Digest {
    let (events, _) = crate::nirvana_run::Journal::new(path.to_path_buf()).read();
    let mut out = Digest::default();
    for event in &events {
        match &event.kind {
            Kind::Seated(s) if s.class == review::CLASS => {
                let parent = s.parent.clone().filter(|_| {
                    s.why
                        .as_deref()
                        .is_some_and(|why| CARRIES_ON.contains(&why))
                });
                out.seated.push((s.order.clone(), event.at, parent));
            }
            Kind::Gated {
                order,
                verdict,
                axes,
                ..
            } if out.seated.iter().any(|(o, _, _)| o == order) => {
                out.gated
                    .push((order.clone(), event.at, verdict.clone(), axes.clone()));
            }
            _ => {}
        }
    }
    out
}

/// `(path, modified ns, size)`: one file's stat, as `Stamp` holds it.
type FileStat = (PathBuf, u128, u64);

/// What the cache thread keeps between reads of one project, by each file's stat (review S4).
#[derive(Debug, Default)]
pub struct Memo {
    journals: HashMap<PathBuf, ((u128, u64), Digest)>,
    /// Each profile's `knowledge/`, by the stat of every file in it, with the row read from it.
    profiles: HashMap<PathBuf, (Vec<FileStat>, ProfileRow)>,
    /// Journals parsed and `knowledge/` folders read, for the tests.
    pub reads: usize,
}

/// §1.2 "Today", from the journals of the runs written today. A review is counted on the day it was seated, which is
/// the day the cap counts it (`nirvana_review::queue` counts it just before the `seated` event); a reseat or
/// send-back child is the same review, which the latest `gated` event of its chain rules (review S1).
pub fn today(project: &Project, cap: u32, now: u64, memo: &mut Memo) -> Today {
    let day = day_of(now);
    let mut reviews: Vec<Review> = Vec::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    let mut kept = HashMap::new();
    for run in project.runs() {
        let path = project.journal(&run).path;
        let Some((_, modified, size)) = stat(&path) else {
            continue;
        };
        // A journal last written before today holds no review seated today.
        if modified != 0 && day_of((modified / 1_000_000_000) as u64) < day {
            continue;
        }
        let digest = match memo.journals.remove(&path) {
            Some((key, digest)) if key == (modified, size) => digest,
            _ => {
                memo.reads += 1;
                digest(&path)
            }
        };
        let parents: HashMap<&str, &str> = digest
            .seated
            .iter()
            .filter_map(|(order, _, parent)| Some((order.as_str(), parent.as_deref()?)))
            .collect();
        let root_of = |order: &str| -> String {
            let mut current = order;
            // A journal is the run's own record, but a loop in it must not hang the thread.
            for _ in 0..=parents.len() {
                match parents.get(current) {
                    Some(parent) => current = parent,
                    None => break,
                }
            }
            current.to_owned()
        };
        let mut last: HashMap<String, (u64, &str, &[Value])> = HashMap::new();
        for (order, at, verdict, axes) in &digest.gated {
            let root = root_of(order);
            if last.get(&root).is_none_or(|(seen, _, _)| *at >= *seen) {
                last.insert(root, (*at, verdict.as_str(), axes.as_slice()));
            }
        }
        for (order, at, parent) in &digest.seated {
            if parent.is_some() || day_of(*at) != day {
                continue;
            }
            let review = match last.get(order) {
                None => Review {
                    order: order.clone(),
                    ruled: Ruled::Waiting,
                    gated_at: None,
                    applied: 0,
                    refused: 0,
                },
                Some((at, verdict, axes)) => {
                    let (ruled, applied, refused, whys) = ruling(verdict, axes);
                    for why in whys {
                        *reasons.entry(reason_key(&why)).or_default() += 1;
                    }
                    Review {
                        order: order.clone(),
                        ruled,
                        gated_at: Some(*at),
                        applied,
                        refused,
                    }
                }
            };
            reviews.push(review);
        }
        kept.insert(path, ((modified, size), digest));
    }
    // Journals of runs gone, or last written before today, leave the memo.
    memo.journals = kept;
    let mut reasons: Vec<(String, usize)> = reasons.into_iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Today {
        day,
        used: review::used_today(&project.root, now),
        reviews,
        cap,
        reasons,
    }
}

/// The seat table's `lesson_reviews_per_day`, read without writing: the machine's `seats.toml` when it is there,
/// else herdr's default; `nirvana_review::per_day` decides, so a refused table reads `PER_DAY` as the gate's does.
pub fn cap() -> u32 {
    let text = std::fs::read_to_string(seat::dir().join(seat::SEATS_FILE))
        .unwrap_or_else(|_| seat::DEFAULT.to_owned());
    review::per_day(seat::parse(&text).ok().as_ref())
}

/// Every ledger row of the lessons and of each folder profile's cards, newest first, at most `CHANGES_SHOWN`.
fn changes(root: &Path, profiles: &[ProfileRow]) -> Vec<Change> {
    let mut all = Vec::new();
    let lessons = nirvana_lessons::ledger(root);
    for row in &lessons {
        let mut slugs: Vec<String> = Vec::new();
        for snap in row.after.iter().chain(&row.before) {
            if !slugs.contains(&snap.slug) {
                slugs.push(snap.slug.clone());
            }
        }
        all.push(Change {
            source: Source::Lessons,
            id: row.id.clone(),
            op: row.op.clone(),
            slugs,
            by: row.by.clone(),
            at: row.at,
            undone: lessons.iter().any(|r| r.of.as_deref() == Some(&row.id)),
        });
    }
    for profile in profiles {
        let Some(dir) = &profile.knowledge else {
            continue;
        };
        let rows = knowledge::ledger(dir);
        for row in &rows {
            all.push(Change {
                source: Source::Profile(profile.name.clone()),
                id: row.id.clone(),
                op: row.op.clone(),
                slugs: vec![row.slug.clone()],
                by: row.by.clone(),
                at: row.at,
                undone: rows.iter().any(|r| r.of.as_deref() == Some(&row.id)),
            });
        }
    }
    let number = |id: &str| {
        id.strip_prefix("op-")
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };
    all.sort_by(|a, b| {
        b.at.cmp(&a.at)
            .then_with(|| number(&b.id).cmp(&number(&a.id)))
            .then_with(|| a.source.cmp(&b.source))
    });
    all.truncate(CHANGES_SHOWN);
    all
}

fn profiles(root: &Path, memo: &mut Memo) -> Vec<ProfileRow> {
    let mut kept = HashMap::new();
    let rows = profile_files::list(root)
        .into_iter()
        .map(|(name, _, _)| {
            let Some(dir) = profile_files::knowledge_dir(root, &name) else {
                return ProfileRow {
                    name,
                    ..ProfileRow::default()
                };
            };
            // Review S4: the cards, their shas, the index and the ledger are read again only when a file in
            // `knowledge/` changed, was added or went.
            let key: Vec<(PathBuf, u128, u64)> =
                listed(&dir).iter().filter_map(|path| stat(path)).collect();
            let row = match memo.profiles.remove(&dir) {
                Some((seen, row)) if seen == key && row.name == name => row,
                _ => {
                    memo.reads += 1;
                    profile_row(name, &dir)
                }
            };
            kept.insert(dir, (key, row.clone()));
            row
        })
        .collect();
    memo.profiles = kept;
    rows
}

fn profile_row(name: String, dir: &Path) -> ProfileRow {
    let (index, rows) = (knowledge::read_index(dir), knowledge::ledger(dir));
    let mut row = ProfileRow {
        name,
        knowledge: Some(dir.to_path_buf()),
        ..ProfileRow::default()
    };
    for (slug, file, parsed) in knowledge::cards(dir) {
        row.cards += 1;
        match parsed {
            Err(_) => row.refused += 1,
            Ok(card) => {
                let sha = knowledge::card_sha(&file).unwrap_or_default();
                if !knowledge::known(&index, &rows, &slug, &sha) {
                    row.unknown += 1;
                }
                if card.verdict.warns() {
                    row.warnings += 1;
                }
            }
        }
    }
    row
}

#[cfg(test)]
thread_local! {
    /// Views built on this thread; the draw-path test reads it on the thread that composes.
    static READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub fn reads_on_this_thread() -> usize {
    READS.with(std::cell::Cell::get)
}

#[cfg(test)]
static PANIC_LOADING: Mutex<Option<PathBuf>> = Mutex::new(None);
#[cfg(test)]
static PANIC_RESOLVING: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Review N5's test: the cache thread's read of `root` (or the project lookup of `folder`) panics while this is set.
#[cfg(test)]
pub fn panic_for_test(loading: Option<PathBuf>, resolving: Option<PathBuf>) {
    *PANIC_LOADING.lock().unwrap_or_else(|e| e.into_inner()) = loading;
    *PANIC_RESOLVING.lock().unwrap_or_else(|e| e.into_inner()) = resolving;
}

/// The whole view of `root`, read now with nothing remembered: the tests' reader.
#[cfg(test)]
pub fn load(root: &Path, now: u64) -> LearningView {
    load_with(root, now, &mut Memo::default())
}

/// The whole view of `root`, read now, parsing again only the journals and `knowledge/` folders whose stat changed
/// since `memo` saw them. Off the draw path only: `cached` calls it on a thread of its own.
pub fn load_with(root: &Path, now: u64, memo: &mut Memo) -> LearningView {
    #[cfg(test)]
    READS.with(|reads| reads.set(reads.get() + 1));
    #[cfg(test)]
    if PANIC_LOADING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_deref()
        == Some(root)
    {
        panic!("p36 test: loading {} panics", root.display());
    }
    let project = Project::at(root);
    let index = nirvana_lessons::read_index(root);
    let mut lessons: Vec<LessonRow> = index
        .lessons
        .iter()
        .map(|(slug, entry)| LessonRow {
            slug: slug.clone(),
            name: entry.name.clone(),
            origin: entry.origin,
            class: entry.class.clone(),
            profile: entry.profile.clone(),
            counters: entry.counters.clone(),
            live: nirvana_lessons::live(&entry.counters),
        })
        .collect();
    lessons.sort_by(|a, b| b.live.cmp(&a.live).then_with(|| a.slug.cmp(&b.slug)));
    let profiles = profiles(root, memo);
    LearningView {
        root: root.to_path_buf(),
        live: lessons.iter().filter(|l| l.live).count(),
        // Review N7: `herdr lessons stats` prints its lines for an empty index too.
        stats: nirvana_lessons::stats_lines(&index),
        lessons,
        today: today(&project, cap(), now, memo),
        changes: changes(root, &profiles),
        waiting: review::crowded(root),
        profiles,
    }
}

// ---- the stat that says whether to read again (§2) ----

/// `(path, modified ns, size)` of each file and folder the view is made from, and the UTC day.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamp {
    day: u64,
    files: Vec<(PathBuf, u128, u64)>,
}

fn stat(path: &Path) -> Option<(PathBuf, u128, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let at = meta
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Some((path.to_path_buf(), at, meta.len()))
}

fn listed(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    paths
}

/// The lessons folder and its index and ledger, the profiles folder, each profile's `knowledge/` and every file in
/// it, the runs folder and each run's journal, and the seat table. A lesson's body is not in the view, so it is not
/// stamped.
pub fn stamp(root: &Path, now: u64) -> Stamp {
    let project = Project::at(root);
    let lessons = nirvana_lessons::dir(root);
    let mut paths = vec![
        lessons.clone(),
        lessons.join(nirvana_lessons::INDEX),
        lessons.join(nirvana_lessons::LEDGER),
        profile_files::folder(root),
        // Also the folder the cap's count is renamed into (`nirvana_review::queue`), so a queued review reads again.
        project.dir().join("runs"),
        seat::dir().join(seat::SEATS_FILE),
    ];
    for entry in listed(&profile_files::folder(root)) {
        paths.push(entry.clone());
        let knowledge = entry.join(profile_files::KNOWLEDGE);
        paths.push(knowledge.clone());
        paths.extend(listed(&knowledge));
    }
    for run in listed(&project.dir().join("runs")) {
        paths.push(run.join(crate::nirvana_run::JOURNAL));
    }
    Stamp {
        day: day_of(now),
        files: paths.iter().filter_map(|path| stat(path)).collect(),
    }
}

// ---- the project a pane is in (review S2) ----

/// A wait for `git rev-parse`, which the Git square's reads bound the same way.
const GIT_WAIT: Duration = Duration::from_secs(5);

/// The project `herdr lessons` reads for a pane whose folder is `folder`: `$NIRVANA_PROJECT`, else the git top level,
/// else the folder (`nirvana_run::Project::from_env`). The client cannot read a pane's environment, so the one case
/// where herdr sets `$NIRVANA_PROJECT` is read from the path: a worker pane's (`nirvana_worker::pane_env`), whose
/// folder is its worktree, `<project>/.nirvana/worktrees/<order>` (`Project::worktrees_dir`). Any other worktree is its
/// own top level, as `herdr lessons` run there reads it. One `git` per folder, on the cache thread.
pub fn project_root(folder: &Path) -> PathBuf {
    #[cfg(test)]
    if PANIC_RESOLVING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_deref()
        == Some(folder)
    {
        panic!("p36 test: resolving {} panics", folder.display());
    }
    let out = crate::nirvana_git::run_cmd(
        folder,
        "git",
        &["rev-parse", "--show-toplevel"],
        GIT_WAIT,
        &[],
    );
    // `run_cmd` puts stderr first; the top level is stdout's one line.
    let top = (out.code == 0)
        .then(|| out.text.lines().rev().find(|l| !l.trim().is_empty()))
        .flatten()
        .map(|top| {
            let top = top.trim();
            // git prints `C:/Users/...` on Windows, as `from_env` turns it.
            PathBuf::from(if cfg!(windows) {
                top.replace('/', "\\")
            } else {
                top.to_owned()
            })
        });
    let Some(top) = top else {
        return folder.to_path_buf();
    };
    let worker = top
        .parent()
        .filter(|dir| dir.file_name().is_some_and(|n| n == "worktrees"))
        .and_then(Path::parent)
        .filter(|dir| {
            dir.file_name()
                .is_some_and(|n| n == crate::nirvana_run::DIR)
        })
        .and_then(Path::parent);
    worker.map_or(top.clone(), Path::to_path_buf)
}

// ---- the cache ----

#[derive(Default)]
struct Entry {
    view: Option<LearningView>,
    stamp: Option<Stamp>,
    reading: bool,
    checked_at: Option<Instant>,
    asked_at: Option<Instant>,
    /// Taken by the reading thread and handed back with its view.
    memo: Option<Memo>,
}

#[derive(Default)]
struct Cache {
    /// Each pane folder asked about: the project it is in (`None` while its thread looks), and when it was asked.
    roots: HashMap<PathBuf, (Option<PathBuf>, Instant)>,
    /// Each project's view, by its root.
    entries: HashMap<PathBuf, Entry>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static CHANGED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static TEST_VIEW: std::cell::RefCell<Option<LearningView>> = const { std::cell::RefCell::new(None) };
    /// Tests that do not set a view and do not ask for the cache get no square, so the strip's other tests read the
    /// same strip whatever a background read finds in their temp folders.
    static TEST_LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub fn set_view_for_test(view: Option<LearningView>) {
    TEST_VIEW.with_borrow_mut(|v| *v = view);
}

#[cfg(test)]
pub fn set_live_for_test(live: bool) {
    TEST_LIVE.with(|l| l.set(live));
}

/// Whether a view changed since the last call: `src/client/mod.rs`'s repaint chain asks every tick.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
}

/// Reads the project `root` again on the next `cached`, whatever its stamp: after an undo.
pub fn invalidate(root: &Path) {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.as_mut().and_then(|cache| cache.entries.get_mut(root)) {
        entry.stamp = None;
        entry.checked_at = None;
    }
}

/// What the strip draws for a pane in `folder`: `Some(None)` before the first read, which draws a placeholder
/// (review N4). Tests that set no view and did not ask for the live cache get `None`, no square.
pub fn square(folder: &Path) -> Option<Option<LearningView>> {
    #[cfg(test)]
    if TEST_VIEW.with_borrow(Option::is_none) && !TEST_LIVE.with(std::cell::Cell::get) {
        return None;
    }
    Some(cached(folder))
}

/// The last view of the project a pane in `folder` is in, `None` before its first read. The first call starts a
/// thread that finds the project (`project_root`); after it, at most every `REFRESH`, a thread stats the project and
/// reads it again when the stamp changed. Called while drawing: it reads no file.
pub fn cached(folder: &Path) -> Option<LearningView> {
    #[cfg(test)]
    {
        if let Some(view) = TEST_VIEW.with_borrow(Clone::clone) {
            return Some(view);
        }
        if !TEST_LIVE.with(std::cell::Cell::get) {
            return None;
        }
    }
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(Cache::default);
    let now = Instant::now();
    cache
        .roots
        .retain(|_, (_, at)| now.duration_since(*at) < FORGET_AFTER);
    cache
        .entries
        .retain(|_, entry| entry.asked_at.is_some_and(|at| at.elapsed() < FORGET_AFTER));
    let root = match cache.roots.get_mut(folder) {
        Some((root, at)) => {
            *at = now;
            root.clone()?
        }
        None => {
            cache.roots.insert(folder.to_path_buf(), (None, now));
            let folder = folder.to_path_buf();
            std::thread::spawn(move || resolve(folder));
            return None;
        }
    };
    let entry = cache.entries.entry(root.clone()).or_default();
    entry.asked_at = Some(now);
    let due = entry.checked_at.is_none_or(|at| at.elapsed() >= REFRESH);
    if due && !entry.reading {
        entry.reading = true;
        let (known, memo) = (entry.stamp.clone(), entry.memo.take().unwrap_or_default());
        std::thread::spawn(move || refresh(root, known, memo));
    }
    entry.view.clone()
}

/// Hands a project lookup's answer back, or, when the thread panicked first, forgets the folder so the next draw
/// looks again (review N5).
struct Resolved {
    folder: PathBuf,
    root: Option<PathBuf>,
}

impl Drop for Resolved {
    fn drop(&mut self) {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(cache) = guard.as_mut() else {
            return;
        };
        match self.root.take() {
            Some(root) => {
                if let Some((slot, _)) = cache.roots.get_mut(&self.folder) {
                    *slot = Some(root);
                }
                // The next draw starts the first read.
                CHANGED.store(true, Ordering::Release);
            }
            None => {
                cache.roots.remove(&self.folder);
            }
        }
    }
}

fn resolve(folder: PathBuf) {
    let mut done = Resolved { folder, root: None };
    done.root = Some(project_root(&done.folder));
}

/// Hands a read back to its entry, and clears `reading` whatever happened, a panic included (review N5): a thread
/// that never came back left the square frozen on its last view.
struct Read {
    root: PathBuf,
    stamp: Option<Stamp>,
    view: Option<LearningView>,
    memo: Option<Memo>,
}

impl Drop for Read {
    fn drop(&mut self) {
        let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = guard
            .as_mut()
            .and_then(|cache| cache.entries.get_mut(&self.root))
        else {
            return;
        };
        entry.reading = false;
        entry.checked_at = Some(Instant::now());
        entry.memo = self.memo.take();
        if let Some(view) = self.view.take() {
            entry.stamp = self.stamp.take();
            if entry.view.as_ref() != Some(&view) {
                entry.view = Some(view);
                CHANGED.store(true, Ordering::Release);
            }
        }
    }
}

fn refresh(root: PathBuf, known: Option<Stamp>, mut memo: Memo) {
    let mut done = Read {
        root,
        stamp: None,
        view: None,
        memo: None,
    };
    let now = crate::nirvana_run::now();
    let stamp = stamp(&done.root, now);
    if known.as_ref() != Some(&stamp) {
        done.view = Some(load_with(&done.root, now, &mut memo));
        done.stamp = Some(stamp);
    }
    done.memo = Some(memo);
}

// ---- undo (§1.3) ----

/// What each undo's thread hands back, by its ticket, until the panel that started it takes it.
static UNDONE: Mutex<BTreeMap<u64, Result<String, String>>> = Mutex::new(BTreeMap::new());
static TICKETS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
static HELD: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

/// Review S3's test: an undo of `(root, id)` waits while it is held.
#[cfg(test)]
pub fn hold_undo_for_test(root: &Path, id: &str, held: bool) {
    let mut list = HELD.lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|(r, i)| !(r == root && i == id));
    if held {
        list.push((root.to_path_buf(), id.to_owned()));
    }
}

/// The rollback `herdr lessons rollback <id>` or `herdr kb rollback --profile <name> <id>` runs, for `change`.
pub fn undo(root: &Path, change: &Change, at: u64) -> Result<String, String> {
    match &change.source {
        Source::Lessons => nirvana_lessons::rollback(root, &change.id, at),
        Source::Profile(name) => knowledge::rollback(root, name, &change.id, at),
    }
}

/// Runs `undo` on its own thread and returns at once with its ticket; `take_undo(ticket)` hands its words back, so an
/// undo a closed panel started never answers the next panel's (review S3).
pub fn start_undo(root: PathBuf, change: Change) -> u64 {
    let ticket = TICKETS.fetch_add(1, Ordering::AcqRel) + 1;
    std::thread::spawn(move || {
        #[cfg(test)]
        while HELD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|(r, i)| r == &root && i == &change.id)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            undo(&root, &change, crate::nirvana_run::now())
        }))
        .unwrap_or_else(|_| {
            Err("the undo stopped before it finished; read the ledger before trying again".into())
        });
        invalidate(&root);
        UNDONE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(ticket, done);
        CHANGED.store(true, Ordering::Release);
    });
    ticket
}

/// The words of the undo with `ticket`, once it finished: a map read, no I/O. Older tickets' words, which no panel
/// waits on any more, go with it.
pub fn take_undo(ticket: u64) -> Option<Result<String, String>> {
    let mut map = UNDONE.lock().unwrap_or_else(|e| e.into_inner());
    let done = map.remove(&ticket)?;
    map.retain(|seen, _| *seen > ticket);
    Some(done)
}

#[cfg(test)]
pub(crate) mod tests;
