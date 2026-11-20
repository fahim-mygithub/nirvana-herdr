//! Nirvana fork, plan 46 P46a: the seat table checked against real runs. `herdr seat calibrate`, the idle tick's check
//! and the Seats panel's Table page read what this module writes. (nirvana/plans/2026-11-19-46-seat-table-view.md §4)
//!
//! The owner, 2026-11-19: "Use the latest logs to update the table as refrencses. During the idle lesson learning runs
//! have it check / maintain the table using real runs as calibration / validation." (plan 46 L3 to L5)
//!
//! - **One pure function**, `calibrate(table, record, journals, now)`, recomputed whole every time: the file it feeds,
//!   `<home>/nirvana/seat-calibration.json`, holds no state of its own, so deleting it loses nothing.
//! - **Cost is applied** (plan 46 D1). Per seat and effort, the median over medium orders of what an order spent against
//!   what the table books for it, `nirvana/seat_fit.py`'s section 1 ported line for line. Where 5 or more orders put
//!   the median outside 0.5 to 2.0, `pick` books the measured cost (`Factors`, read by `populate_windows`). A project
//!   with 5 or more orders of its own for the pair uses its own figure: on 2026-11-19 Opus high read 2.30 in Megajam (n
//!   60), 1.28 in MotoProject and 0.56 in groove-coach (plan 46 §3).
//! - **Scores are findings only** (plan 46 D1). `nirvana/seat_refresh.py classes`'s rule is computed and written down,
//!   both on all refusals and without the ones whose failing axes were all the brief's or the gate's. It never changes a
//!   score: the literal rule would have moved Opus in build.load_bearing from 95 to 68 on 8 first passes of 21, where
//!   the same orders read 8 of 10 without the refusals for an unowned file or a rebase.
//! - **herdr writes this file, never the owner's table** (plan 46 D2): a table herdr edits stops matching any kept
//!   default and stops upgrading, the fault the owner's table had on 2026-11-19.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::nirvana_seat::{self, Effort, RecordLine, SizeName, Table, Tokens};

pub const FILE: &str = "seat-calibration.json";
pub const LEDGER_FILE: &str = "seat-calibration.ledger.jsonl";
/// The file's schema. A file of another version reads as none, and the next check writes this one. 2 (the P46a review,
/// finding 1): the file names the table it was measured against (`Calibration::table_fingerprint`); a version 1 file
/// has none, so it reads as no file and the next tick checks again.
pub const VERSION: u32 = 2;
/// Where a check that could not write its file says when (unix seconds), so the idle tick backs off `BACKOFF_S`
/// instead of replaying every journal at every tick (the P46a review, finding 4).
pub const FAILED_FILE: &str = "seat-calibration.failed";

// ---- seat_fit.py's numbers (nirvana/seat_fit.py, its constants of the same names) ----

/// `WINDOW`: the week, because every pool but `local` has one.
pub const WINDOW: &str = "week";
/// `BAND`: a median inside it books as the table says.
pub const BAND: (f64, f64) = (0.5, 2.0);
/// `MIN_N`: orders a pair needs before its median moves a booking.
pub const MIN_N: usize = 5;
/// Plan 46 §4: "One step never moves a cell by more than 4 times; a larger gap is flagged." Argon high measured 11.13
/// on 2026-11-19 (n 7, the seat is off): its pool was sized from two probe orders of two cents each.
pub const CLAMP: (f64, f64) = (0.25, 4.0);
/// Only record lines this recent: models and prices move (Hydrogen's price doubled in the table on 2026-11-18, Flash
/// 3.8's doubles on 2027-01-01), and a cell refit in the table is measured afresh against what came after it.
pub const KEEP_S: u64 = 60 * 86_400;

// ---- seat_refresh.py classes' rule (its docstring, written before the 2026-11-18 refresh applied it) ----

/// Under this many gated orders a group is "too few".
pub const GATED_MIN: usize = 10;
/// A first-pass share under this would send the score to the class's Efficient bar less `EFFICIENT_LESS`.
pub const FIRST_PASS_LOW: f64 = 0.40;
pub const EFFICIENT_LESS: f32 = 2.0;
/// A first-pass share at or over this, merged at or over `MERGED_HIGH`, under the Balanced bar, would send the score to
/// that bar plus `BALANCED_MORE`.
pub const FIRST_PASS_HIGH: f64 = 0.90;
pub const MERGED_HIGH: f64 = 0.70;
pub const BALANCED_MORE: f32 = 1.0;
/// seat_refresh.py's `PASS`: the first verdicts that count as a first pass.
const PASS: [&str; 2] = ["pass", "accepted"];

// ---- the page's reference rows and the idle check (plan 46 §4) ----

/// "A cell under 5 orders reads `thin`."
pub const THIN: usize = 5;
/// "A seat with none reads `no record`." Read by the Table page (P46b).
#[allow(dead_code)]
pub const NO_RECORD: &str = "no record";
/// "orders in the last 14 days".
pub const RECENT_S: u64 = 14 * 86_400;
/// A factor that moves by more than this share is a ledger row.
pub const MOVE: f64 = 0.10;
/// The idle tick checks again when the record has grown by this many lines, or after `DUE_S`.
pub const DUE_LINES: usize = 20;
pub const DUE_S: u64 = 86_400;
/// After a check that could not write its file, the idle tick waits this long before it tries again.
pub const BACKOFF_S: u64 = 3_600;

// ---- the idle check's bounds (the P46a review, finding 4: nothing bounded the tick, which holds the idle lock) ----

/// A journal larger than this is skipped, by its size on disk, before it is read. On the 2026-11-19 record the 116
/// journals it names measured 8 KB at the least, 31 KB at the median, 170 KB at p90 and 11.8 MB at the most (one
/// run): this cap skips none of them and leaves the largest room to grow to more than twice its size.
pub const JOURNAL_MAX_BYTES: u64 = 32 << 20;
/// At most this many journals per check, newest run first. The 2026-11-19 record named 116 over 60 days.
pub const JOURNAL_MAX_FILES: usize = 500;
/// The whole check's wall time, after which the journals not yet read are skipped and the check completes with what it
/// has. The check on the 2026-11-19 record took 1.2 s with 116 journals.
pub const CHECK_BUDGET: std::time::Duration = std::time::Duration::from_secs(15);

/// What `calibrate` returns and `seat-calibration.json` holds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub version: u32,
    pub computed_at: u64,
    /// The UTC day of `computed_at`.
    pub date: String,
    /// The record it was computed from: its non-empty lines and bytes, so the idle tick sees it grow.
    pub record_lines: usize,
    pub record_bytes: u64,
    /// Lines of the last `KEEP_S` that the figures read.
    pub lines_used: usize,
    /// `fingerprint` of the table the figures were measured against (the P46a review, finding 1): a factor is a ratio
    /// to a cell as it stood, so when the table in use has another fingerprint no reader applies the file's factors
    /// and the next idle tick is due at once. Empty (a hand-made file) never fits.
    #[serde(default)]
    pub table_fingerprint: String,
    /// The table's `calibration` when this was computed: off, the file is written and no pick reads it.
    pub on: bool,
    /// Medium orders against the cell: one row per seat and effort (no project), then one per project.
    pub cost: Vec<CostRow>,
    /// The score rule's verdict per class and seat. Never applied.
    pub findings: Vec<Finding>,
    /// The page's reference rows, per class, seat and effort.
    pub cells: Vec<CellRow>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostRow {
    pub seat: String,
    pub effort: Effort,
    /// `None` for the pooled row; else the project's root as `project_of` keys it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub n: usize,
    /// seat_fit.py's "median ratio": 1.0 is the table's booking exactly. Read leniently: serde_json writes a NaN as
    /// `null`, which would otherwise make the whole file unreadable.
    #[serde(default, deserialize_with = "lenient_f64")]
    pub median: f64,
    /// What the booking is multiplied by; `None` books as the table says. Read leniently (a number, or a number as
    /// text, "NaN" included) so that one bad row is dropped by `Factors::from_calibration` instead of losing the file.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "lenient_factor"
    )]
    pub factor: Option<f64>,
    /// The median was outside `CLAMP`, so the factor is the clamp's edge: a gap larger than one step.
    #[serde(default)]
    pub clamped: bool,
    /// The table's `calibration_hold` names the pair: the figure is recorded and shown, never booked.
    #[serde(default)]
    pub held: bool,
}

/// The score rule's answer for one group.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum Rule {
    TooFew,
    Keep,
    Lower { to: f32 },
    Raise { to: f32 },
}

impl Rule {
    pub fn moves(self) -> bool {
        matches!(self, Self::Lower { .. } | Self::Raise { .. })
    }

    fn words(self, score: f32) -> String {
        match self {
            Self::TooFew => format!("too few gated to say (under {GATED_MIN})"),
            Self::Keep => format!("keeps {score:.0}"),
            Self::Lower { to } | Self::Raise { to } => format!("would go {score:.0} to {to:.0}"),
        }
    }
}

/// One class and seat: the rule both ways, with its n. A finding, never a change (plan 46 D1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub class: String,
    pub seat: String,
    pub score: f32,
    pub orders: usize,
    pub gated: usize,
    pub first_pass: usize,
    /// First verdicts whose failing axes were all the brief's or the gate's.
    pub not_worker: usize,
    /// Merged orders over the orders of runs that merged any (`None`: no such run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<f64>,
    /// first_pass / gated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<f64>,
    /// first_pass / (gated - not_worker): the share without refusals no worker could fix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_worker: Option<f64>,
    pub rule: Rule,
    pub rule_worker: Rule,
    pub text: String,
}

impl Finding {
    pub fn moves(&self) -> bool {
        self.rule.moves() || self.rule_worker.moves()
    }
}

/// One class, seat and effort, as the Table page's detail block shows it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CellRow {
    pub class: String,
    pub seat: String,
    pub effort: Option<Effort>,
    pub orders: usize,
    pub gated: usize,
    pub first_pass: usize,
    pub merged: usize,
    /// Orders of runs that merged any order: what `merged` is out of. A research run merges nothing.
    pub merge_orders: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_minutes: Option<f64>,
    /// The median of what an order spent against its booking at its own size (1.0 is the booking).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_cost: Option<f64>,
    /// Orders `median_cost` read.
    pub costed: usize,
    /// Orders of the last `RECENT_S`.
    pub recent: usize,
}

impl CellRow {
    /// "thin" under `THIN` orders, else "measured". A cell with no row is `NO_RECORD`. Read by the Table page (P46b).
    #[allow(dead_code)]
    pub fn status(&self) -> &'static str {
        if self.orders < THIN {
            "thin"
        } else {
            "measured"
        }
    }
}

/// One change of a booking factor: appeared, changed by more than `MOVE`, or gone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerRow {
    pub at: u64,
    pub seat: String,
    pub effort: Effort,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub n: usize,
}

/// What the journals of the record's runs say that a record line written before plan 46 does not: each order's first
/// verdict and its fault, and which orders merged.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Journals {
    /// (run, order) -> the first verdict and whether its failing axes were all a non-worker's.
    pub first: BTreeMap<(String, String), (String, Option<bool>)>,
    /// (run, order) of every merged order.
    pub merged: BTreeSet<(String, String)>,
    /// Runs whose journal was read and that merged at least one order.
    pub merging: BTreeSet<String>,
    /// Journal files read.
    pub files: usize,
    /// Journal files named and not read, by why (the P46a review, finding 4).
    pub skipped: Skipped,
}

/// The journals a check named and did not read.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Skipped {
    /// Not there, or its size or text did not read.
    pub missing: usize,
    /// Over `Limits::max_bytes` on disk.
    pub large: usize,
    /// Past `Limits::max_files` (the oldest runs).
    pub many: usize,
    /// Not reached within `Limits::budget`.
    pub late: usize,
}

impl Skipped {
    pub fn total(&self) -> usize {
        self.missing + self.large + self.many + self.late
    }

    /// "3 journal files skipped (1 missing, 2 past the 15 s budget)"; `None` when none was.
    pub fn words(&self, limits: &Limits) -> Option<String> {
        let parts: Vec<String> = [
            (self.missing, "missing or unreadable".to_owned()),
            (
                self.large,
                format!("over {} MB", limits.max_bytes / (1 << 20)),
            ),
            (self.many, format!("past the {}-file cap", limits.max_files)),
            (
                self.late,
                format!("past the {} s budget", limits.budget.as_secs()),
            ),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, why)| format!("{n} {why}"))
        .collect();
        (!parts.is_empty()).then(|| {
            format!(
                "{} skipped ({})",
                plural(self.total(), "journal file", "journal files"),
                parts.join(", ")
            )
        })
    }
}

/// The bounds of one check's journal replay; `Limits::default()` is the constants'.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub max_bytes: u64,
    pub max_files: usize,
    pub budget: std::time::Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: JOURNAL_MAX_BYTES,
            max_files: JOURNAL_MAX_FILES,
            budget: CHECK_BUDGET,
        }
    }
}

/// A number, a number written as text ("NaN", "1e9"), or nothing: anything else reads as nothing.
fn lenient_number(value: Option<serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn lenient_factor<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Option<f64>, D::Error> {
    Ok(lenient_number(Option::<serde_json::Value>::deserialize(
        de,
    )?))
}

fn lenient_f64<'de, D: serde::Deserializer<'de>>(de: D) -> Result<f64, D::Error> {
    Ok(lenient_number(Option::<serde_json::Value>::deserialize(de)?).unwrap_or(f64::NAN))
}

/// The P46a review, finding 1: a fingerprint of everything `unit` reads from the table, so a factor is applied only
/// against the cells it was measured on. Hashed: each seat's id and pool; each of its use cells (effort, `at`, and every
/// window's share: a factor scales every window of the cell, so an edit of any of them restates the cell); each of its
/// price rows (date and the four prices); each pool's `per_usd`; each class's weight; and `size.medium.use`. Not
/// hashed, so an edit there keeps the factors: scores, bars, `overrides_per_run` and every other constant, `mode`,
/// `calibration`, `calibration_hold`, comments and every `basis` text. Sorted by id, so moving a block is no change.
/// Floats as Rust's shortest round-trip text; sha256, its first 16 hex digits.
pub fn fingerprint(table: &Table) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let mut text = String::new();
    let _ = writeln!(text, "size.medium.use {:?}", table.size.medium.use_factor);
    let mut classes: Vec<_> = table.classes.iter().collect();
    classes.sort_by(|a, b| a.id.cmp(&b.id));
    for class in classes {
        let _ = writeln!(text, "class {} weight {:?}", class.id, class.weight);
    }
    let mut pools: Vec<_> = table.pools.iter().collect();
    pools.sort_by(|a, b| a.id.cmp(&b.id));
    for pool in pools {
        let _ = writeln!(text, "pool {} per_usd {:?}", pool.id, pool.per_usd);
    }
    let mut seats: Vec<_> = table.seats.iter().collect();
    seats.sort_by(|a, b| a.id.cmp(&b.id));
    for seat in seats {
        let _ = writeln!(text, "seat {} pool {}", seat.id, seat.pool);
        let mut uses: Vec<_> = seat.uses.iter().collect();
        uses.sort_by_key(|cell| cell.effort);
        for cell in uses {
            let _ = writeln!(
                text,
                "  use {} at {} {:?}",
                cell.effort.word(),
                cell.at,
                cell.windows
            );
        }
        let mut prices: Vec<_> = seat.prices.iter().collect();
        prices.sort_by(|a, b| a.from.cmp(&b.from));
        for price in prices {
            let _ = writeln!(
                text,
                "  price {} {:?} {:?} {:?} {:?}",
                price.from, price.input, price.output, price.cache_read, price.cache_write
            );
        }
    }
    Sha256::digest(text.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Calibration {
    /// The figures were measured against this table's cells (`fingerprint`).
    pub fn fits(&self, table: &Table) -> bool {
        self.table_fingerprint == fingerprint(table)
    }
}

// ---- the arithmetic ----

/// A line's time: when it was written, else when its order ended or was seated.
fn line_at(line: &RecordLine) -> Option<u64> {
    line.at.or(line.ended_at).or(line.seated_at)
}

/// Python's `statistics.median`: the middle value, or the mean of the two middle ones.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    })
}

/// seat_fit.py's `fit` for one line: the order in P4 units before its size, `actual / (cell[week] x class weight)`,
/// with `actual` its tokens at the seat's price row on the cell's own `at` date times the pool's `per_usd[week]`
/// ("Pricing at the cell's own date keeps a price change out of the fit"). `None` wherever the script skips the line.
/// One departure: a line whose tokens are all zero is skipped, where the script keeps a non-empty dict of zeros; serde
/// reads `{}` and `{"input": 0}` alike. The script's subagent skip is the caller's: `calibrate` passes only the
/// deduplicated orders, which leave subagents out (one guard, so its control can go red; with a second one here the
/// fix round's `p97_fit_keeps_subagents` could not).
pub fn unit(table: &Table, line: &RecordLine) -> Option<f64> {
    let seat = table.seat(&line.seat)?;
    line.size?;
    let tokens = line.tokens.filter(|tokens| *tokens != Tokens::default())?;
    let cell = seat.uses.iter().find(|cell| cell.effort == line.effort)?;
    let per = table
        .pool(&seat.pool)?
        .per_usd
        .get(WINDOW)
        .copied()
        .filter(|per| *per != 0.0)?;
    let price = seat.price_at(nirvana_seat::date(&cell.at)?)?;
    let share = f64::from(
        cell.windows
            .get(WINDOW)
            .copied()
            .filter(|share| *share != 0.0)?,
    );
    let weight = f64::from(table.class(&line.class)?.weight);
    // A class weight of 0 divides by zero: an infinite ratio is no measurement (the script stops there with an error).
    Some(nirvana_seat::cost(price, &tokens) * per / (share * weight))
        .filter(|unit| unit.is_finite())
}

/// The booking rule: a factor at `MIN_N` or more orders whose median is outside `BAND`, clamped to `CLAMP`.
pub fn factor(n: usize, median: f64) -> (Option<f64>, bool) {
    if n < MIN_N || (BAND.0..=BAND.1).contains(&median) {
        return (None, false);
    }
    let clamped = median.clamp(CLAMP.0, CLAMP.1);
    (Some(clamped), clamped != median)
}

/// A path as a key: forward slashes, no trailing slash, and lower case on Windows, whose paths ignore case.
fn normalise(path: &str) -> String {
    let unified = path.replace('\\', "/");
    let trimmed = unified.trim_end_matches('/');
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed.to_owned()
    }
}

/// The project a record line's `journal` (`<project>/.nirvana/runs/<run>/journal.jsonl#<seq>`, `journal_ref`) names: the
/// path before its `.nirvana` folder. `None` for a line with no such path.
pub fn project_of(journal: &str) -> Option<String> {
    let path = journal.rsplit_once('#').map_or(journal, |(path, _)| path);
    let unified = path.replace('\\', "/");
    let (root, _) = unified.split_once("/.nirvana/")?;
    Some(normalise(root))
}

/// A project root as `project_of` keys it; a worktree under `.nirvana/worktrees/` is its project's.
pub fn root_key(root: &Path) -> String {
    let text = root.to_string_lossy().replace('\\', "/");
    let root = match text.split_once("/.nirvana/") {
        Some((root, _)) => root.to_owned(),
        None => text.strip_suffix("/.nirvana").unwrap_or(&text).to_owned(),
    };
    normalise(&root)
}

/// One deduplicated order the scores and the reference rows read.
struct Order<'a> {
    line: &'a RecordLine,
    first: Option<String>,
    not_worker: bool,
    /// `Some` only for an order of a run that merged any order.
    merged: Option<bool>,
}

fn first_of<'a>(line: &'a RecordLine, journals: &'a Journals) -> (Option<String>, bool) {
    if let Some(first) = &line.first_verdict {
        return (
            Some(first.clone()),
            line.first_no_worker_fault == Some(true) || first == "amend",
        );
    }
    let key = (line.run.clone().unwrap_or_default(), line.order.clone());
    match journals.first.get(&key) {
        Some((first, fault)) => (Some(first.clone()), *fault == Some(true)),
        None => (None, false),
    }
}

fn share(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

/// seat_refresh.py's rule for one group: too few under `GATED_MIN` gated; a first-pass share under `FIRST_PASS_LOW` to
/// the Efficient bar less 2 (when that is a fall); at or over `FIRST_PASS_HIGH`, merged at or over `MERGED_HIGH` and
/// under the Balanced bar, to that bar plus 1; else it keeps its score.
fn rule(
    table: &Table,
    class: &nirvana_seat::Class,
    score: f32,
    gated: usize,
    first_pass: usize,
    merged: Option<f64>,
) -> Rule {
    if gated < GATED_MIN {
        return Rule::TooFew;
    }
    let share = first_pass as f64 / gated as f64;
    let bars = table.bars(class);
    if share < FIRST_PASS_LOW {
        let to = bars.efficient - EFFICIENT_LESS;
        return if score > to {
            Rule::Lower { to }
        } else {
            Rule::Keep
        };
    }
    if share >= FIRST_PASS_HIGH
        && merged.is_some_and(|merged| merged >= MERGED_HIGH)
        && score < bars.balanced
    {
        return Rule::Raise {
            to: bars.balanced + BALANCED_MORE,
        };
    }
    Rule::Keep
}

/// The calibration from the record, pure: no file, no clock but `now`. `journals` fills what a line written before
/// plan 46 does not carry; the line's own fields win.
pub fn calibrate(
    table: &Table,
    lines: &[RecordLine],
    journals: &Journals,
    now: u64,
) -> Calibration {
    let kept: Vec<&RecordLine> = lines
        .iter()
        .filter(|line| line_at(line).is_some_and(|at| at.saturating_add(KEEP_S) >= now))
        .collect();
    let medium_use = f64::from(table.size.medium.use_factor);

    // One line per order and run, the last (as `nirvana_seat::tally`), subagents left out: the cost, the scores and the
    // reference rows all read these. The P46a review, finding 6: cost read every line, so a corrected line repeating
    // its order's tokens counted that order twice toward the n of 5. The record of 2026-11-19 held no such pair (2457
    // lines, no order and run with two), so no figure moved there. seat_fit.py dedupes the same way.
    let mut last: BTreeMap<(Option<&str>, &str), &RecordLine> = BTreeMap::new();
    for line in &kept {
        if line.chain.as_deref() == Some(crate::nirvana_claude::SUBAGENT_CHAIN) {
            continue;
        }
        last.insert((line.run.as_deref(), line.order.as_str()), line);
    }

    // Cost: the medium orders, as seat_fit.py reads them.
    let mut pooled: BTreeMap<(String, Effort), Vec<f64>> = BTreeMap::new();
    let mut own: BTreeMap<(String, String, Effort), Vec<f64>> = BTreeMap::new();
    for line in last.values() {
        if line.size != Some(SizeName::Medium) {
            continue;
        }
        let Some(unit) = unit(table, line) else {
            continue;
        };
        let ratio = unit / medium_use;
        pooled
            .entry((line.seat.clone(), line.effort))
            .or_default()
            .push(ratio);
        if let Some(project) = line.journal.as_deref().and_then(project_of) {
            own.entry((project, line.seat.clone(), line.effort))
                .or_default()
                .push(ratio);
        }
    }
    let row = |seat: &str, effort: Effort, project: Option<&str>, values: &[f64]| {
        let median = median(values).unwrap_or(0.0);
        let (factor, clamped) = factor(values.len(), median);
        CostRow {
            seat: seat.to_owned(),
            effort,
            project: project.map(str::to_owned),
            n: values.len(),
            median,
            factor,
            clamped,
            held: table.held(seat, effort),
        }
    };
    let mut cost: Vec<CostRow> = pooled
        .iter()
        .map(|((seat, effort), values)| row(seat, *effort, None, values))
        .collect();
    cost.extend(
        own.iter()
            .map(|((project, seat, effort), values)| row(seat, *effort, Some(project), values)),
    );

    // Scores and reference rows: the same deduplicated orders.
    let orders: Vec<Order> = last
        .values()
        .map(|line| {
            let (first, not_worker) = first_of(line, journals);
            let run = line.run.clone().unwrap_or_default();
            let merged = journals
                .merging
                .contains(&run)
                .then(|| journals.merged.contains(&(run, line.order.clone())));
            Order {
                line,
                first,
                not_worker,
                merged,
            }
        })
        .collect();

    let mut cells: BTreeMap<(String, String, Option<Effort>), Vec<&Order>> = BTreeMap::new();
    let mut groups: BTreeMap<(String, String), Vec<&Order>> = BTreeMap::new();
    for order in &orders {
        let line = order.line;
        cells
            .entry((line.class.clone(), line.seat.clone(), Some(line.effort)))
            .or_default()
            .push(order);
        groups
            .entry((line.class.clone(), line.seat.clone()))
            .or_default()
            .push(order);
    }
    let counts = |orders: &[&Order]| {
        let gated: Vec<&&Order> = orders.iter().filter(|o| o.first.is_some()).collect();
        let first_pass = gated
            .iter()
            .filter(|o| o.first.as_deref().is_some_and(|word| PASS.contains(&word)))
            .count();
        let not_worker = gated.iter().filter(|o| o.not_worker).count();
        let merge_orders = orders.iter().filter(|o| o.merged.is_some()).count();
        let merged = orders.iter().filter(|o| o.merged == Some(true)).count();
        (gated.len(), first_pass, not_worker, merged, merge_orders)
    };
    let cell_rows: Vec<CellRow> = cells
        .iter()
        .map(|((class, seat, effort), orders)| {
            let (gated, first_pass, _, merged, merge_orders) = counts(orders);
            let minutes: Vec<f64> = orders
                .iter()
                .filter_map(|o| {
                    let (seated, ended) = (o.line.seated_at?, o.line.ended_at?);
                    Some(ended.saturating_sub(seated) as f64 / 60.0)
                })
                .collect();
            let costs: Vec<f64> = orders
                .iter()
                .filter_map(|o| {
                    let size = table.size(o.line.size?).use_factor;
                    Some(unit(table, o.line)? / f64::from(size))
                })
                .collect();
            CellRow {
                class: class.clone(),
                seat: seat.clone(),
                effort: *effort,
                orders: orders.len(),
                gated,
                first_pass,
                merged,
                merge_orders,
                median_minutes: median(&minutes),
                median_cost: median(&costs),
                costed: costs.len(),
                recent: orders
                    .iter()
                    .filter(|o| {
                        line_at(o.line).is_some_and(|at| at.saturating_add(RECENT_S) >= now)
                    })
                    .count(),
            }
        })
        .collect();

    let mut findings = Vec::new();
    for ((class_id, seat), orders) in &groups {
        let Some(class) = table.class(class_id) else {
            continue;
        };
        let Some(entry) = class.seats.iter().find(|entry| entry.seat == *seat) else {
            continue;
        };
        let (gated, first_pass, not_worker, merged, merge_orders) = counts(orders);
        let merged_share = share(merged, merge_orders);
        let worker_gated = gated - not_worker;
        let rule_all = rule(table, class, entry.score, gated, first_pass, merged_share);
        let rule_worker = rule(
            table,
            class,
            entry.score,
            worker_gated,
            first_pass,
            merged_share,
        );
        let mut text = format!(
            "{seat} in {class_id}: first pass {first_pass} of {gated}{}, {}",
            share(first_pass, gated).map_or(String::new(), |s| format!(" ({s:.2})")),
            rule_all.words(entry.score)
        );
        if not_worker > 0 {
            text.push_str(&format!(
                "; without {not_worker} refusal{} no worker could fix, {first_pass} of {worker_gated}{}, {}",
                if not_worker == 1 { "" } else { "s" },
                share(first_pass, worker_gated).map_or(String::new(), |s| format!(" ({s:.2})")),
                rule_worker.words(entry.score)
            ));
        }
        findings.push(Finding {
            class: class_id.clone(),
            seat: seat.clone(),
            score: entry.score,
            orders: orders.len(),
            gated,
            first_pass,
            not_worker,
            merged: merged_share,
            share: share(first_pass, gated),
            share_worker: share(first_pass, worker_gated),
            rule: rule_all,
            rule_worker,
            text,
        });
    }

    let mut calibration = Calibration {
        version: VERSION,
        computed_at: now,
        date: crate::nirvana_run::utc(now).chars().take(10).collect(),
        record_lines: lines.len(),
        record_bytes: 0,
        lines_used: kept.len(),
        table_fingerprint: fingerprint(table),
        on: table.calibration,
        cost,
        findings,
        cells: cell_rows,
        summary: String::new(),
    };
    calibration.summary = summary(&calibration);
    calibration
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "1 cost booked as measured, 2 score findings": what the Learning panel's line and the Table page say.
pub fn summary(calibration: &Calibration) -> String {
    let pooled = calibration
        .cost
        .iter()
        .filter(|row| row.project.is_none() && row.factor.is_some() && !row.held)
        .count();
    let projects = calibration
        .cost
        .iter()
        .filter(|row| row.project.is_some() && row.factor.is_some() && !row.held)
        .count();
    let held = calibration
        .cost
        .iter()
        .filter(|row| row.factor.is_some() && row.held)
        .count();
    let flagged = calibration.cost.iter().filter(|row| row.clamped).count();
    let findings = calibration.findings.iter().filter(|f| f.moves()).count();
    let mut out = format!("{} booked as measured", plural(pooled, "cost", "costs"));
    if projects > 0 {
        out.push_str(&format!(", {projects} more by a project's own orders"));
    }
    if held > 0 {
        out.push_str(&format!(", {held} held by calibration_hold"));
    }
    if flagged > 0 {
        out.push_str(&format!(
            ", {} more than one step off",
            plural(flagged, "gap", "gaps")
        ));
    }
    out.push_str(&format!(
        ", {}",
        plural(findings, "score finding", "score findings")
    ));
    if !calibration.on {
        out.push_str(" (calibration is off in seats.toml)");
    }
    out
}

/// The ledger rows between two calibrations: a factor that appeared, moved by more than `MOVE`, or went.
pub fn ledger(before: Option<&Calibration>, after: &Calibration, now: u64) -> Vec<LedgerRow> {
    type Key = (String, Effort, Option<String>);
    let factors = |cal: &Calibration| -> BTreeMap<Key, (f64, usize)> {
        cal.cost
            .iter()
            .filter_map(|row| {
                Some((
                    (row.seat.clone(), row.effort, row.project.clone()),
                    (row.factor?, row.n),
                ))
            })
            .collect()
    };
    let old = before.map(factors).unwrap_or_default();
    let new = factors(after);
    let n_now = |key: &Key| {
        after
            .cost
            .iter()
            .find(|row| (&row.seat, row.effort, &row.project) == (&key.0, key.1, &key.2))
            .map_or(0, |row| row.n)
    };
    let keys: BTreeSet<&Key> = old.keys().chain(new.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let (was, is) = (old.get(key).map(|f| f.0), new.get(key).map(|f| f.0));
            let moved = match (was, is) {
                (Some(a), Some(b)) => (b / a - 1.0).abs() > MOVE,
                (None, None) => false,
                _ => true,
            };
            moved.then(|| LedgerRow {
                at: now,
                seat: key.0.clone(),
                effort: key.1,
                project: key.2.clone(),
                before: was,
                after: is,
                n: new.get(key).map_or_else(|| n_now(key), |f| f.1),
            })
        })
        .collect()
}

// ---- the factors a pick reads ----

/// A factor the check could have written: finite, positive and inside `CLAMP` (the check clamps every one).
fn factor_in_bounds(factor: f64) -> bool {
    factor.is_finite() && (CLAMP.0..=CLAMP.1).contains(&factor)
}

/// The rows `Factors::from_calibration` drops for a factor no check writes (not finite, not positive, outside `CLAMP`):
/// a hand edit or a bad write. Rows under `MIN_N` orders are not counted; the check writes those itself.
pub fn dropped_rows(calibration: &Calibration) -> usize {
    calibration
        .cost
        .iter()
        .filter(|row| row.n >= MIN_N && row.factor.is_some_and(|factor| !factor_in_bounds(factor)))
        .count()
}

/// One booking factor as a pick applies it.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub factor: f64,
    pub n: usize,
    pub median: f64,
    pub clamped: bool,
    /// Whose orders measured it: `None` for every project's.
    pub project: Option<String>,
}

impl Applied {
    /// `Pick.use_basis`'s words: "measured x0.37 over 7 orders".
    pub fn words(&self) -> String {
        let mut out = format!("measured x{:.2} over {} orders", self.factor, self.n);
        if self.project.is_some() {
            out.push_str(" of this project");
        }
        if self.clamped {
            out.push_str(&format!(
                " (the record reads x{:.2}: a gap larger than one step, flagged)",
                self.median
            ));
        }
        out
    }
}

/// The factors from `seat-calibration.json`, for one project. Empty: every booking is the table's.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Factors {
    pooled: BTreeMap<(String, Effort), Applied>,
    /// Per project, the pairs with `MIN_N` or more of its own orders: its own factor, or `None` (inside the band).
    own: BTreeMap<String, BTreeMap<(String, Effort), Option<Applied>>>,
    /// The project whose orders this world seats; `None` reads the pooled factors only.
    pub project: Option<String>,
}

impl Factors {
    /// The rows as a pick applies them. The P46a review, finding 3: the file is checked here, where it is read, as well
    /// as where it is written: a factor that is not finite and positive drops its row (a project's then falls back to
    /// the pooled figure), and a row under `MIN_N` orders is dropped. A hand edit or a bad write of 0 booked a seat as
    /// free. The P46a second review, F6: the check always clamps what it writes (`factor`), so a factor outside `CLAMP`
    /// in the file is no measurement but corruption: its row is dropped too (it books as the table says) rather than
    /// booked at the clamp's edge, and `dropped_rows` counts them for the summary. Whether the file fits the table is
    /// the reader's question (`Calibration::fits`), asked by `from_file`, `for_this_project` and the Seats square.
    pub fn from_calibration(calibration: &Calibration, project: Option<String>) -> Self {
        let mut factors = Self {
            project,
            ..Self::default()
        };
        for row in &calibration.cost {
            if row.n < MIN_N {
                continue;
            }
            let applied = match row.factor {
                Some(raw) if !factor_in_bounds(raw) => continue,
                Some(factor) => Some(Applied {
                    factor,
                    n: row.n,
                    median: row.median,
                    clamped: row.clamped,
                    project: row.project.clone(),
                }),
                None => None,
            };
            match &row.project {
                None => {
                    if let Some(applied) = applied {
                        factors
                            .pooled
                            .insert((row.seat.clone(), row.effort), applied);
                    }
                }
                Some(project) => {
                    factors
                        .own
                        .entry(project.clone())
                        .or_default()
                        .insert((row.seat.clone(), row.effort), applied);
                }
            }
        }
        factors
    }

    /// `from_calibration` on the file in `dir`; empty when there is none or it was measured against another table
    /// (the P46a review, finding 1).
    pub fn from_file(dir: &Path, table: &Table, project: Option<String>) -> Self {
        read(dir)
            .filter(|cal| cal.fits(table))
            .map_or_else(Self::default, |cal| Self::from_calibration(&cal, project))
    }

    /// The factors for the project this process runs in (`Project::from_env`, which may ask `git`): asked only when the
    /// file holds a project's own figure. Empty when the file was measured against another table.
    pub fn for_this_project(dir: &Path, table: &Table) -> Self {
        let Some(calibration) = read(dir).filter(|cal| cal.fits(table)) else {
            return Self::default();
        };
        let has_own = calibration
            .cost
            .iter()
            .any(|row| row.project.is_some() && row.n >= MIN_N);
        let project = has_own.then(|| root_key(&crate::nirvana_run::Project::from_env().root));
        Self::from_calibration(&calibration, project)
    }

    /// The factor for a seat and effort: the project's own verdict when it has `MIN_N` orders, else the pooled one.
    pub fn for_cell(&self, seat: &str, effort: Effort) -> Option<&Applied> {
        let key = (seat.to_owned(), effort);
        if let Some(own) = self
            .project
            .as_ref()
            .and_then(|project| self.own.get(project))
            .and_then(|own| own.get(&key))
        {
            return own.as_ref();
        }
        self.pooled.get(&key)
    }
}

// ---- the files ----

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE)
}

pub fn ledger_path(dir: &Path) -> PathBuf {
    dir.join(LEDGER_FILE)
}

/// The calibration in `dir`; `None` when it is missing, does not parse, or is another version.
pub fn read(dir: &Path) -> Option<Calibration> {
    let text = std::fs::read_to_string(path(dir)).ok()?;
    serde_json::from_str::<Calibration>(&text)
        .ok()
        .filter(|cal| cal.version == VERSION)
}

/// The record as read: its lines that parse, and its size.
pub struct Record {
    pub lines: Vec<RecordLine>,
    pub count: usize,
    pub bytes: u64,
}

/// The installed record, or why it cannot be read. A record that is not there is an error here: there is nothing to
/// check, and the tick writes nothing.
pub fn read_record(path: &Path) -> Result<Record, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let raw: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    Ok(Record {
        lines: raw
            .iter()
            .filter_map(|line| serde_json::from_str(line.trim()).ok())
            .collect(),
        count: raw.len(),
        bytes: text.len() as u64,
    })
}

/// The non-empty lines of the record, without parsing them: the tick's cheap look.
fn record_count(path: &Path) -> Option<usize> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.lines().filter(|line| !line.trim().is_empty()).count())
}

/// Replays each journal the record's recent lines name, once: the first verdicts and the merges a line written before
/// plan 46 does not carry. A journal that is skipped reads as none: its orders read as ungated.
///
/// Bounded (the P46a review, finding 4: the tick holds the idle lock, and the paths can be anywhere): newest run first;
/// a journal that is missing or over `limits.max_bytes` by its metadata is skipped before it is read; at most
/// `limits.max_files` are read; once `started` is `limits.budget` old the rest are skipped, and the check completes
/// with what it has. What this cannot do: a read, or the metadata call before it, that blocks (a dead network drive)
/// blocks the tick until the operating system gives up, since a blocking read cannot be cancelled; the budget is
/// checked between files only.
pub fn read_journals(
    lines: &[RecordLine],
    now: u64,
    limits: &Limits,
    started: std::time::Instant,
) -> Journals {
    // run -> (journal path, the run's newest line time)
    let mut runs: BTreeMap<String, (String, u64)> = BTreeMap::new();
    for line in lines.iter() {
        let Some(at) = line_at(line).filter(|at| at.saturating_add(KEEP_S) >= now) else {
            continue;
        };
        if let (Some(run), Some(journal)) = (&line.run, &line.journal) {
            let path = journal
                .rsplit_once('#')
                .map_or(journal.as_str(), |(path, _)| path);
            let entry = runs
                .entry(run.clone())
                .or_insert_with(|| (path.to_owned(), at));
            entry.1 = entry.1.max(at);
        }
    }
    let mut order: Vec<(String, String, u64)> = runs
        .into_iter()
        .map(|(run, (path, at))| (run, path, at))
        .collect();
    order.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut journals = Journals::default();
    for (run, path, _) in order {
        if started.elapsed() >= limits.budget {
            journals.skipped.late += 1;
            continue;
        }
        if journals.files >= limits.max_files {
            journals.skipped.many += 1;
            continue;
        }
        match std::fs::metadata(&path) {
            Err(_) => {
                journals.skipped.missing += 1;
                continue;
            }
            Ok(meta) if meta.len() > limits.max_bytes => {
                journals.skipped.large += 1;
                continue;
            }
            Ok(_) => {}
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            journals.skipped.missing += 1;
            continue;
        };
        journals.files += 1;
        let state = crate::nirvana_run::replay(&crate::nirvana_run::parse_lines(&text).0);
        for order in &state.orders {
            let key = (run.clone(), order.id().to_owned());
            if let Some(first) = &order.first_verdict {
                journals
                    .first
                    .insert(key.clone(), (first.clone(), order.first_no_worker_fault));
            }
            if order.merged_sha.is_some() || order.phase == crate::nirvana_run::Phase::Merged {
                journals.merged.insert(key);
                journals.merging.insert(run.clone());
            }
        }
    }
    journals
}

pub fn failed_path(dir: &Path) -> PathBuf {
    dir.join(FAILED_FILE)
}

/// A write failed under `BACKOFF_S` ago (`FAILED_FILE`). The P46a second review: a note dated after `now` (a clock
/// moved back, a note copied from another machine) held every check off until that time; it is ignored.
fn backing_off(dir: &Path, now: u64) -> bool {
    std::fs::read_to_string(failed_path(dir))
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
        .is_some_and(|at| at <= now && now < at.saturating_add(BACKOFF_S))
}

/// Whether the idle tick should check again: the record grew by `DUE_LINES` or shrank, `DUE_S` passed, there is no
/// readable calibration, or the one there was measured against another table (the P46a review, finding 1: at once).
/// No record: never. A write that failed under `BACKOFF_S` ago: not yet, whatever else holds.
pub fn due(dir: &Path, table: &Table, now: u64) -> bool {
    if backing_off(dir, now) {
        return false;
    }
    let Some(lines) = record_count(&dir.join(nirvana_seat::RECORD_FILE)) else {
        return false;
    };
    match read(dir) {
        None => true,
        Some(cal) => {
            !cal.fits(table)
                || now >= cal.computed_at.saturating_add(DUE_S)
                || lines >= cal.record_lines + DUE_LINES
                || lines < cal.record_lines
        }
    }
}

/// One check: what it computed, the ledger rows it found, and whether it wrote them.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub calibration: Calibration,
    pub ledger: Vec<LedgerRow>,
    pub written: bool,
    /// Journal files the check replayed, and its wall time with them.
    pub journals: usize,
    pub millis: u128,
    /// Journal files it named and did not read (finding 4), and the words for them, already in the summary.
    pub skipped: Skipped,
}

/// Computes from the record in `dir` and, when `write`, writes the file (atomic) and appends the ledger. A record that
/// does not read, or a file that does not write, is an `Err`. `compute` and `save` are its two halves.
pub fn check(dir: &Path, table: &Table, now: u64, write: bool) -> Result<Checked, String> {
    let mut checked = compute(dir, table, now, &Limits::default())?;
    if write {
        save(dir, &mut checked)?;
    }
    Ok(checked)
}

/// The figures from the record in `dir`, against `table`, with `limits` on the journals; nothing written.
pub fn compute(dir: &Path, table: &Table, now: u64, limits: &Limits) -> Result<Checked, String> {
    let started = std::time::Instant::now();
    let record = read_record(&dir.join(nirvana_seat::RECORD_FILE))?;
    let journals = read_journals(&record.lines, now, limits, started);
    let mut calibration = calibrate(table, &record.lines, &journals, now);
    calibration.record_lines = record.count;
    calibration.record_bytes = record.bytes;
    let before = read(dir);
    let ledger = ledger(before.as_ref(), &calibration, now);
    if !ledger.is_empty() {
        calibration.summary.push_str(&format!(
            "; {} since the last check",
            plural(ledger.len(), "booking changed", "bookings changed")
        ));
    }
    if let Some(words) = journals.skipped.words(limits) {
        calibration.summary.push_str(&format!("; {words}"));
    }
    Ok(Checked {
        calibration,
        ledger,
        written: false,
        journals: journals.files,
        millis: started.elapsed().as_millis(),
        skipped: journals.skipped,
    })
}

/// Writes `checked`'s file (atomic) and appends its ledger rows; marks it written.
pub fn save(dir: &Path, checked: &mut Checked) -> Result<(), String> {
    let mut text =
        serde_json::to_string_pretty(&checked.calibration).map_err(|err| err.to_string())?;
    text.push('\n');
    nirvana_seat::write_atomic(&path(dir), text.as_bytes())
        .map_err(|err| format!("{}: {err}", path(dir).display()))?;
    if !checked.ledger.is_empty() {
        let mut rows = String::new();
        for row in &checked.ledger {
            rows.push_str(&serde_json::to_string(row).map_err(|err| err.to_string())?);
            rows.push('\n');
        }
        use std::io::Write;
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ledger_path(dir))
            .and_then(|mut file| file.write_all(rows.as_bytes()))
            .map_err(|err| format!("{}: {err}", ledger_path(dir).display()))?;
    }
    checked.written = true;
    Ok(())
}

/// The idle tick's step: a check when `due`, its summary for the Learning panel's line. Model-free and seat-free, so it
/// sits behind none of the review's gates. `None` when not due or when the record failed to read (nothing written).
/// A file that does not write is said, and the tick backs off `BACKOFF_S` (`FAILED_FILE`) instead of replaying every
/// journal at the next tick; if that note cannot be written either, nothing backs off.
pub fn tick(dir: &Path, table: &Table, now: u64) -> Option<String> {
    if !due(dir, table, now) {
        return None;
    }
    let mut checked = compute(dir, table, now, &Limits::default()).ok()?;
    match save(dir, &mut checked) {
        Ok(()) => {
            let _ = std::fs::remove_file(failed_path(dir));
            Some(checked.calibration.summary)
        }
        Err(why) => {
            let _ = std::fs::write(failed_path(dir), format!("{now}\n"));
            Some(format!(
                "the check could not be written ({why}); the next try is in {} min",
                BACKOFF_S / 60
            ))
        }
    }
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr seat calibrate [--json] [--dry-run] [--picks]
The record's medium orders against each seat's booked cost (nirvana/seat_fit.py's rule: a factor at 5 or more orders
outside 0.5-2.0, clamped to 0.25-4.0), and the score rule's findings (never applied). Writes
<home>/nirvana/seat-calibration.json and a ledger row per booking that changed; --dry-run prints what would change.
--picks also prints every class and mode whose pick on a cool world differs with calibration off and on, as the table
holds pairs and with no hold, pooled and for each project with its own figure.";

fn ledger_words(row: &LedgerRow) -> String {
    let what = |f: Option<f64>| f.map_or("the table's".to_owned(), |f| format!("x{f:.2}"));
    format!(
        "{} {}{}: {} to {} (n {})",
        row.seat,
        row.effort.word(),
        row.project
            .as_deref()
            .map_or(String::new(), |p| format!(" in {p}")),
        what(row.before),
        what(row.after),
        row.n
    )
}

/// The text `herdr seat calibrate` prints.
pub fn text(checked: &Checked, record: &Path) -> String {
    let cal = &checked.calibration;
    let mut out = format!(
        "seat calibration from {} ({} lines, {} in the last {} days){}\n",
        record.display(),
        cal.record_lines,
        cal.lines_used,
        KEEP_S / 86_400,
        if checked.written {
            ""
        } else {
            "; dry run, nothing written"
        }
    );
    out.push_str(&format!(
        "cost, medium orders against the booked cell ({WINDOW} window; booked as measured at n >= {MIN_N} outside {}-{}):\n",
        BAND.0, BAND.1
    ));
    for row in cal.cost.iter().filter(|row| row.project.is_none()) {
        out.push_str(&format!(
            "  {} {}: median {:.2}, n {}, {}\n",
            row.seat,
            row.effort.word(),
            row.median,
            row.n,
            match row.factor {
                Some(f) if row.held => format!("held at the table's booking (measured x{f:.2})"),
                Some(f) if row.clamped =>
                    format!("booked x{f:.2}, clamped: a gap larger than one step"),
                Some(f) => format!("booked x{f:.2}"),
                None => "as the table".to_owned(),
            }
        ));
    }
    for row in cal
        .cost
        .iter()
        .filter(|row| row.project.is_some() && row.n >= MIN_N)
    {
        out.push_str(&format!(
            "  {} {} in {}: median {:.2}, n {}, {}\n",
            row.seat,
            row.effort.word(),
            row.project.as_deref().unwrap_or(""),
            row.median,
            row.n,
            match row.factor {
                Some(f) if row.held => format!("held at the table's booking (measured x{f:.2})"),
                Some(f) => format!("booked x{f:.2}"),
                None => "as the table".to_owned(),
            }
        ));
    }
    let verb = if checked.written {
        "changed"
    } else {
        "would change"
    };
    for row in &checked.ledger {
        out.push_str(&format!("{verb}: {}\n", ledger_words(row)));
    }
    let moving: Vec<&Finding> = cal.findings.iter().filter(|f| f.moves()).collect();
    out.push_str(&format!(
        "score findings (seat_refresh.py's rule; never applied): {} of {} groups\n",
        moving.len(),
        cal.findings.len()
    ));
    for finding in moving {
        out.push_str(&format!("  {}\n", finding.text));
    }
    out.push_str(&format!("{}\n", cal.summary));
    out.push_str(&format!(
        "checked in {} ms, {} journal files read{}\n",
        checked.millis,
        checked.journals,
        if checked.skipped.total() > 0 {
            format!(", {} skipped", checked.skipped.total())
        } else {
            String::new()
        }
    ));
    out
}

/// Plan 46: each class and mode whose pick on a cool world (every window read now at 0%) differs between calibration
/// off and on, `table` as given, the factors `calibration`'s for `project`: "balanced plan: opus high -> fable high".
pub fn moved_picks(
    table: &Table,
    calibration: &Calibration,
    project: Option<String>,
    now: u64,
) -> Vec<String> {
    let mut world = nirvana_seat::cool_world(
        table,
        &nirvana_seat::World {
            now,
            ..nirvana_seat::World::default()
        },
    );
    world.calibration = Factors::from_calibration(calibration, project);
    let mut on = table.clone();
    on.calibration = true;
    let mut off = table.clone();
    off.calibration = false;
    let mut moved = Vec::new();
    for class in &table.classes {
        for mode in [
            nirvana_seat::Mode::Efficient,
            nirvana_seat::Mode::Balanced,
            nirvana_seat::Mode::Sota,
        ] {
            let mut request = nirvana_seat::Request::new(&class.id);
            request.mode = mode;
            let seat = |table: &Table| match nirvana_seat::pick(table, &world, &request) {
                Ok(pick) => format!("{} {}", pick.seat, pick.effort.word()),
                Err(_) => "no seat".to_owned(),
            };
            let (a, b) = (seat(&off), seat(&on));
            if a != b {
                moved.push(format!("{} {}: {a} -> {b}", mode.word(), class.id));
            }
        }
    }
    moved
}

/// `--picks`' text: the moves as the table holds pairs and with no hold, pooled and per project.
fn picks_text(table: &Table, calibration: &Calibration, now: u64) -> String {
    let mut unheld = table.clone();
    unheld.calibration_hold.clear();
    let mut projects: Vec<Option<String>> = vec![None];
    for row in &calibration.cost {
        if row.n >= MIN_N && row.factor.is_some() && !projects.contains(&row.project) {
            projects.push(row.project.clone());
        }
    }
    let mut out = String::new();
    for (label, table) in [("as held", table), ("with no hold", &unheld)] {
        for project in &projects {
            let moved = moved_picks(table, calibration, project.clone(), now);
            out.push_str(&format!(
                "picks that move, {label}, {}: {}\n",
                project.as_deref().unwrap_or("pooled"),
                moved.len()
            ));
            for line in moved {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }
    out
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let (mut json, mut dry, mut picks) = (false, false, false);
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            "--dry-run" => dry = true,
            "--picks" => picks = true,
            _ => {
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        }
    }
    let table = match nirvana_seat::table() {
        Ok(table) => table,
        Err(err) => {
            eprintln!("herdr seat calibrate: {err}");
            std::process::exit(2);
        }
    };
    let dir = nirvana_seat::dir();
    let checked = match check(&dir, &table, crate::nirvana_run::now(), !dry) {
        Ok(checked) => checked,
        Err(err) => {
            eprintln!("herdr seat calibrate: {err}; nothing written");
            std::process::exit(1);
        }
    };
    if json {
        let out = serde_json::json!({
            "calibration": checked.calibration,
            "ledger": checked.ledger,
            "written": checked.written,
            "journals": checked.journals,
            "millis": checked.millis,
            "journals_skipped": checked.skipped.total(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        print!("{}", text(&checked, &dir.join(nirvana_seat::RECORD_FILE)));
    }
    if picks {
        print!(
            "{}",
            picks_text(&table, &checked.calibration, crate::nirvana_run::now())
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
