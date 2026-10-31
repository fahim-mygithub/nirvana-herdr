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
//! - **A cached view, like `nirvana_git::cached`.** `cached` returns the last view and, at most every `REFRESH`,
//!   starts a thread that stats the folders the view is made from; only a changed stat (mtime or size), or a new UTC
//!   day, reads them again. Drawing never reads a file.
//! - **Undo** (`start_undo`) runs `nirvana_lessons::rollback` or `nirvana_knowledge::rollback`, the functions behind
//!   `herdr lessons rollback` and `herdr kb rollback`, on a thread: both take a file lock a running gate may hold.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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

/// §1.2 "Today", from the journals of the runs written today. A review is counted on the day it was seated, which is
/// the day the cap counts it (`nirvana_review::queue` counts it just before the `seated` event).
pub fn today(project: &Project, cap: u32, now: u64) -> Today {
    let day = day_of(now);
    let mut reviews: Vec<Review> = Vec::new();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for run in project.runs() {
        let journal = project.journal(&run);
        // A journal last written before today holds no review seated today.
        let written = std::fs::metadata(&journal.path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .map_or(u64::MAX, |d| d.as_secs());
        if day_of(written) < day {
            continue;
        }
        let (events, _) = journal.read();
        let mut seated: Vec<String> = Vec::new();
        let mut gated: HashMap<String, (u64, String, Vec<Value>)> = HashMap::new();
        for event in &events {
            match &event.kind {
                Kind::Seated(s) if s.class == review::CLASS && day_of(event.at) == day => {
                    seated.push(s.order.clone());
                }
                Kind::Gated {
                    order,
                    verdict,
                    axes,
                    ..
                } => {
                    gated.insert(order.clone(), (event.at, verdict.clone(), axes.clone()));
                }
                _ => {}
            }
        }
        for order in seated {
            let review = match gated.get(&order) {
                None => Review {
                    order,
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
                        order,
                        ruled,
                        gated_at: Some(*at),
                        applied,
                        refused,
                    }
                }
            };
            reviews.push(review);
        }
    }
    let mut reasons: Vec<(String, usize)> = reasons.into_iter().collect();
    reasons.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    Today {
        day,
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

fn profiles(root: &Path) -> Vec<ProfileRow> {
    profile_files::list(root)
        .into_iter()
        .map(|(name, _, _)| {
            let Some(dir) = profile_files::knowledge_dir(root, &name) else {
                return ProfileRow {
                    name,
                    ..ProfileRow::default()
                };
            };
            let (index, rows) = (knowledge::read_index(&dir), knowledge::ledger(&dir));
            let mut row = ProfileRow {
                name,
                knowledge: Some(dir.clone()),
                ..ProfileRow::default()
            };
            for (slug, file, parsed) in knowledge::cards(&dir) {
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
        })
        .collect()
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

/// The whole view of `root`, read now. Off the draw path only: `cached` calls it on a thread of its own.
pub fn load(root: &Path, now: u64) -> LearningView {
    #[cfg(test)]
    READS.with(|reads| reads.set(reads.get() + 1));
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
    let profiles = profiles(root);
    LearningView {
        root: root.to_path_buf(),
        live: lessons.iter().filter(|l| l.live).count(),
        stats: if index.lessons.is_empty() {
            Vec::new()
        } else {
            nirvana_lessons::stats_lines(&index)
        },
        lessons,
        today: today(&project, cap(), now),
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

// ---- the cache ----

#[derive(Default)]
struct Entry {
    view: Option<LearningView>,
    stamp: Option<Stamp>,
    reading: bool,
    checked_at: Option<Instant>,
    asked_at: Option<Instant>,
}

static ENTRIES: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);
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

/// Reads `folder` again on the next `cached`, whatever its stamp: after an undo.
pub fn invalidate(folder: &Path) {
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.as_mut().and_then(|map| map.get_mut(folder)) {
        entry.stamp = None;
        entry.checked_at = None;
    }
}

/// The last view of `folder`, `None` before its first read. At most every `REFRESH` it starts a thread that stats the
/// folder and reads it again when the stamp changed. Called while drawing: it reads no file.
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
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    map.retain(|_, entry| entry.asked_at.is_some_and(|at| at.elapsed() < FORGET_AFTER));
    let entry = map.entry(folder.to_path_buf()).or_default();
    entry.asked_at = Some(Instant::now());
    let due = entry.checked_at.is_none_or(|at| at.elapsed() >= REFRESH);
    if due && !entry.reading {
        entry.reading = true;
        let (folder, known) = (folder.to_path_buf(), entry.stamp.clone());
        std::thread::spawn(move || refresh(&folder, known));
    }
    entry.view.clone()
}

fn refresh(folder: &Path, known: Option<Stamp>) {
    let now = crate::nirvana_run::now();
    let stamp = stamp(folder, now);
    let view = (known.as_ref() != Some(&stamp)).then(|| load(folder, now));
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(entry) = guard.as_mut().and_then(|map| map.get_mut(folder)) {
        entry.reading = false;
        entry.checked_at = Some(Instant::now());
        if let Some(view) = view {
            entry.stamp = Some(stamp);
            if entry.view.as_ref() != Some(&view) {
                entry.view = Some(view);
                CHANGED.store(true, Ordering::Release);
            }
        }
    }
}

// ---- undo (§1.3) ----

/// What an undo's thread hands back, per folder, until the panel takes it.
static UNDONE: Mutex<Option<HashMap<PathBuf, Result<String, String>>>> = Mutex::new(None);

/// The rollback `herdr lessons rollback <id>` or `herdr kb rollback --profile <name> <id>` runs, for `change`.
pub fn undo(root: &Path, change: &Change, at: u64) -> Result<String, String> {
    match &change.source {
        Source::Lessons => nirvana_lessons::rollback(root, &change.id, at),
        Source::Profile(name) => knowledge::rollback(root, name, &change.id, at),
    }
}

/// Runs `undo` on its own thread and returns at once; `take_undo` hands its words back.
pub fn start_undo(root: PathBuf, change: Change) {
    std::thread::spawn(move || {
        let done = undo(&root, &change, crate::nirvana_run::now());
        invalidate(&root);
        UNDONE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .insert(root, done);
        CHANGED.store(true, Ordering::Release);
    });
}

/// A finished undo's words for `root`: a map read, no I/O.
pub fn take_undo(root: &Path) -> Option<Result<String, String>> {
    UNDONE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_mut()
        .and_then(|map| map.remove(root))
}

#[cfg(test)]
pub(crate) mod tests;
