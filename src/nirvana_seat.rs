//! Nirvana fork, plan 12 O2: seats. `seats.toml`, the rule that seats an order, leases for orders in
//! flight, overrides, the record of finished orders, and `herdr seat list|pick|release|record`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §3, §4, §5, §16)
//!
//! The owner, 2026-10-17: "have a orchistrator agent that is aware of the 5h and 7s usage limits and can seat
//! and coordinate which agent and model can take the task to efficiently use usage credits." (plan 12 L2)
//!
//! The orchestrator decides an order's class, needs and size in its own turn; herdr decides the seat,
//! deterministically, and prints why (§5.1). `pick` is a pure function of the table, the readings, the
//! leases, the accounts and the request, so every rule below has a test on recorded readings:
//!
//! 1. **Filters**, in order, each skip with its reason: off in the table; the CLI not installed or not signed
//!    in (Accounts, 0.11.0); a need the seat lacks; the class does not take the seat; the caller excluded its
//!    family (decision 0017); a window at or under the floor; a window whose headroom is below the order's
//!    expected use while another pool could hold the whole order (Argon's review, §4.4); a `max` seat that
//!    would break the orchestrator's reserve.
//! 2. **The bar** at the mode (decision 0018): if no seat clears it, the bar drops to the best seat left, never
//!    under the class's Efficient bar (Argon's review, F4).
//! 3. **Pressure**, the least wins: `max over windows (expected_use / headroom × pace × reset)`, a week's 20%
//!    holding more than a 5 h window's 20% (§4.1). A seat with a window not read ranks below every seat
//!    whose windows are known, and above an exhausted one, which the floor already skipped (§3). Ties go to
//!    the higher score, then table order.
//! 4. **Effort** (§5.5) follows the class and its blast radius; SOTA lifts plan and load-bearing builds to
//!    xhigh; max is never chosen. Under pressure the same model on another pool comes first (equal scores),
//!    then the other family at full effort (the bar drop), and only when nothing is left one effort step down.
//!
//! Every number the rule uses lives in `seats.toml`, so the owner changes a bar or a prior with an edit and
//! the next pick reads it; the hypotheses are tagged there.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::nirvana_usage::{self, Freshness, Reading};

/// herdr's first seat table, written to `<home>/nirvana/seats.toml` when that is missing.
pub const DEFAULT: &str = include_str!("nirvana_seats.toml");
pub const DIR: &str = "nirvana";
pub const SEATS_FILE: &str = "seats.toml";
pub const LEASES_FILE: &str = "leases.json";
pub const RECORD_FILE: &str = "record.jsonl";
const LOCK_FILE: &str = "leases.lock";
/// `herdr seat pick`'s exit code when no seat is given, a refused override included.
pub const NO_SEAT_EXIT: i32 = 3;
/// Scores and bars compare with this slack: a blended score is a product of floats (decision 0018's EDGE).
const EDGE: f32 = 1e-4;
/// A lock file older than this was left by a process that died holding it.
const LOCK_STALE: Duration = Duration::from_secs(30);

// ---- the table ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    pub fn word(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }

    fn down(self) -> Option<Self> {
        match self {
            Self::Low => None,
            Self::Medium => Some(Self::Low),
            Self::High => Some(Self::Medium),
            Self::Xhigh => Some(Self::High),
            Self::Max => Some(Self::Xhigh),
        }
    }
}

/// Decision 0018's three stops; plan 12 calls the middle one Balanced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Efficient,
    Balanced,
    Sota,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Self::Efficient => "efficient",
            Self::Balanced => "balanced",
            Self::Sota => "sota",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "efficient" => Some(Self::Efficient),
            "balanced" => Some(Self::Balanced),
            "sota" => Some(Self::Sota),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SizeName {
    Small,
    Medium,
    Large,
}

impl SizeName {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blast {
    Bounded,
    LoadBearing,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub version: u32,
    pub mode: Mode,
    pub use_record: bool,
    pub constants: Constants,
    pub size: Sizes,
    #[serde(rename = "pool")]
    pub pools: Vec<Pool>,
    #[serde(rename = "seat")]
    pub seats: Vec<Seat>,
    #[serde(rename = "class")]
    pub classes: Vec<Class>,
}

/// The rule's numbers (plan 12 §5.3). Each is commented with its source in the default file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constants {
    pub floor: f32,
    pub reserve: f32,
    pub reserve_pool: String,
    pub reserve_window: String,
    pub pace_ease: f32,
    pub pace_min: f32,
    pub pace_max: f32,
    pub reset_left: f32,
    pub reset_within_min: u64,
    pub reset_discount: f32,
    pub defer_min: u64,
    pub overrides_per_run: u32,
    pub record_prior: f32,
    pub record_shrink: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sizes {
    pub small: Size,
    pub medium: Size,
    pub large: Size,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Size {
    #[serde(rename = "use")]
    pub use_factor: f32,
    pub minutes: u64,
    pub lease_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pool {
    pub id: String,
    pub windows: Vec<String>,
    pub per_usd: BTreeMap<String, f64>,
    #[serde(default)]
    pub basis: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seat {
    pub id: String,
    pub cli: String,
    /// The model id; `{effort}` where the CLI takes the effort inside it (agy).
    pub model: String,
    pub family: String,
    pub pool: String,
    /// Windows this seat spends beyond its pool's: Fable's `week.fable`.
    #[serde(default)]
    pub extra_windows: Vec<String>,
    pub efforts: Vec<Effort>,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub prices: Vec<Price>,
    #[serde(rename = "use")]
    pub uses: Vec<Use>,
}

/// USD per million tokens, from a date on (plan 12 §4: "a price schedule").
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub from: String,
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    #[serde(default)]
    pub basis: String,
}

/// The expected share of each window for probe P4's order at one effort, read on `at`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Use {
    pub effort: Effort,
    pub windows: BTreeMap<String, f32>,
    pub at: String,
    /// "measured: ...", "derived: ..." or "prior: ...".
    pub basis: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Class {
    pub id: String,
    /// One of the nine chains of §5.4: the record is kept by chain, the class is a label.
    pub chain: String,
    pub blast: Blast,
    pub effort: Effort,
    #[serde(default)]
    pub sota_effort: Option<Effort>,
    #[serde(default)]
    pub needs: Vec<String>,
    pub weight: f32,
    #[serde(default)]
    pub bars: Option<Bars>,
    #[serde(default)]
    pub why: String,
    pub seats: Vec<ClassSeat>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassSeat {
    pub seat: String,
    pub score: f32,
    #[serde(default)]
    pub effort: Option<Effort>,
    #[serde(default)]
    pub only_with: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bars {
    pub efficient: f32,
    pub balanced: f32,
    pub sota: f32,
}

impl Bars {
    pub fn at(&self, mode: Mode) -> f32 {
        match mode {
            Mode::Efficient => self.efficient,
            Mode::Balanced => self.balanced,
            Mode::Sota => self.sota,
        }
    }
}

/// Decision 0018 §2.1's defaults: Efficient 70, Performant 80, SOTA 100 bounded; 85, 90, 100 load-bearing.
pub fn default_bars(blast: Blast) -> Bars {
    match blast {
        Blast::Bounded => Bars {
            efficient: 70.0,
            balanced: 80.0,
            sota: 100.0,
        },
        Blast::LoadBearing => Bars {
            efficient: 85.0,
            balanced: 90.0,
            sota: 100.0,
        },
    }
}

/// "2026-11-30" as Unix seconds at its midnight UTC.
pub fn date(text: &str) -> Option<u64> {
    nirvana_usage::rfc3339(&format!("{}T00:00:00Z", text.trim()))
}

impl Table {
    pub fn seat(&self, id: &str) -> Option<&Seat> {
        self.seats.iter().find(|seat| seat.id == id)
    }

    pub fn class(&self, id: &str) -> Option<&Class> {
        self.classes.iter().find(|class| class.id == id)
    }

    pub fn pool(&self, id: &str) -> Option<&Pool> {
        self.pools.iter().find(|pool| pool.id == id)
    }

    pub fn size(&self, size: SizeName) -> &Size {
        match size {
            SizeName::Small => &self.size.small,
            SizeName::Medium => &self.size.medium,
            SizeName::Large => &self.size.large,
        }
    }

    pub fn bars(&self, class: &Class) -> Bars {
        class.bars.unwrap_or_else(|| default_bars(class.blast))
    }

    /// The chains the classes name, in table order.
    pub fn chains(&self) -> Vec<&str> {
        let mut chains: Vec<&str> = Vec::new();
        for class in &self.classes {
            if !chains.contains(&class.chain.as_str()) {
                chains.push(&class.chain);
            }
        }
        chains
    }

    /// What is wrong with an edited table, each in a sentence. A table with problems is refused, never
    /// overwritten: it is the owner's file.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.version != 1 {
            problems.push(format!("version {} is not 1", self.version));
        }
        let c = &self.constants;
        for (name, value) in [
            ("floor", c.floor),
            ("reserve", c.reserve),
            ("reset_left", c.reset_left),
            ("reset_discount", c.reset_discount),
            ("record_prior", c.record_prior),
        ] {
            if !(0.0..=1.0).contains(&value) {
                problems.push(format!("constants.{name} {value} is not between 0 and 1"));
            }
        }
        if c.record_prior <= 0.0 {
            problems.push("constants.record_prior must be above 0".into());
        }
        if !(c.pace_min > 0.0 && c.pace_min <= c.pace_max && c.pace_ease >= 0.0) {
            problems.push("constants: pace_min must be above 0 and at most pace_max".into());
        }
        let mut seen = Vec::new();
        for pool in &self.pools {
            if seen.contains(&pool.id) {
                problems.push(format!("pool {} is listed twice", pool.id));
            }
            seen.push(pool.id.clone());
        }
        let mut seen = Vec::new();
        for seat in &self.seats {
            if seen.contains(&seat.id) {
                problems.push(format!("seat {} is listed twice", seat.id));
            }
            seen.push(seat.id.clone());
            let Some(pool) = self.pool(&seat.pool) else {
                problems.push(format!("seat {}: no pool {}", seat.id, seat.pool));
                continue;
            };
            if seat.efforts.is_empty() {
                problems.push(format!("seat {}: no efforts", seat.id));
            }
            if seat.uses.is_empty() {
                problems.push(format!("seat {}: no use cell", seat.id));
            }
            let windows = seat_windows(seat, pool);
            for cell in &seat.uses {
                if date(&cell.at).is_none() {
                    problems.push(format!(
                        "seat {}: use date {:?} is not a date",
                        seat.id, cell.at
                    ));
                }
                for window in cell.windows.keys() {
                    if !windows.contains(window) {
                        problems.push(format!(
                            "seat {}: use names window {window}, which it does not spend",
                            seat.id
                        ));
                    }
                }
            }
            for price in &seat.prices {
                if date(&price.from).is_none() {
                    problems.push(format!(
                        "seat {}: price date {:?} is not a date",
                        seat.id, price.from
                    ));
                }
            }
        }
        let mut seen = Vec::new();
        for class in &self.classes {
            if seen.contains(&class.id) {
                problems.push(format!("class {} is listed twice", class.id));
            }
            seen.push(class.id.clone());
            let bars = self.bars(class);
            if !(bars.efficient <= bars.balanced && bars.balanced <= bars.sota) {
                problems.push(format!(
                    "class {}: bars must rise from efficient to sota",
                    class.id
                ));
            }
            if class.chain.is_empty() {
                problems.push(format!("class {}: no chain", class.id));
            }
            if class.weight <= 0.0 {
                problems.push(format!("class {}: weight must be above 0", class.id));
            }
            for entry in &class.seats {
                if self.seat(&entry.seat).is_none() {
                    problems.push(format!("class {}: no seat {}", class.id, entry.seat));
                }
            }
        }
        problems
    }
}

/// The windows a seat spends: its pool's, and its own scoped week.
fn seat_windows(seat: &Seat, pool: &Pool) -> Vec<String> {
    let mut windows = pool.windows.clone();
    windows.extend(seat.extra_windows.iter().cloned());
    windows
}

impl Seat {
    /// The model id at an effort: agy takes the effort in the id (`gemini-3.8-flash-low`); claude takes
    /// `--effort` beside an unchanged id.
    pub fn model_for(&self, effort: Effort) -> String {
        self.model.replace("{effort}", effort.word())
    }

    /// The price in force on a day: the latest whose date has come, else the first.
    pub fn price_at(&self, at: u64) -> Option<&Price> {
        let mut dated: Vec<(u64, &Price)> = self
            .prices
            .iter()
            .filter_map(|price| Some((date(&price.from)?, price)))
            .collect();
        dated.sort_by_key(|(from, _)| *from);
        dated
            .iter()
            .rev()
            .find(|(from, _)| *from <= at)
            .or(dated.first())
            .map(|(_, price)| *price)
    }

    /// The use cell for an effort, else the nearest above it (dearer, so a guess errs safe), else below.
    fn use_cell(&self, effort: Effort) -> Option<&Use> {
        self.uses
            .iter()
            .find(|cell| cell.effort == effort)
            .or_else(|| {
                self.uses
                    .iter()
                    .filter(|cell| cell.effort > effort)
                    .min_by_key(|cell| cell.effort)
            })
            .or_else(|| self.uses.iter().max_by_key(|cell| cell.effort))
    }

    /// How much dearer the seat is at `to` than at `from`: Argon's schedule doubles on 2026-11-30, and a use
    /// cell read before that spends twice the share after it.
    fn price_scale(&self, from: u64, to: u64) -> f32 {
        let blend = |price: &Price| price.input + price.output;
        match (self.price_at(from), self.price_at(to)) {
            (Some(then), Some(now)) if blend(then) > 0.0 => (blend(now) / blend(then)) as f32,
            _ => 1.0,
        }
    }
}

/// An order's list-price cost from its own streamed tokens (plan 12 §4: "never from pool deltas").
pub fn cost(price: &Price, tokens: &Tokens) -> f64 {
    (tokens.input as f64 * price.input
        + tokens.output as f64 * price.output
        + tokens.cache_read as f64 * price.cache_read
        + tokens.cache_write as f64 * price.cache_write)
        / 1e6
}

/// Parses a table and refuses one with problems.
pub fn parse(text: &str) -> Result<Table, String> {
    let table: Table = toml::from_str(text).map_err(|err| err.to_string())?;
    let problems = table.problems();
    if problems.is_empty() {
        Ok(table)
    } else {
        Err(problems.join("\n"))
    }
}

/// `<home>/nirvana`, where `usage.json` lives too.
pub fn dir() -> PathBuf {
    crate::nirvana::home()
        .unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
        .join(DIR)
}

/// The table's text, written from `DEFAULT` when the file is missing, and the owner's from then on.
pub fn read_or_create(path: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(path) {
        return text;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, DEFAULT);
    DEFAULT.to_owned()
}

// ---- the world the rule reads ----

/// Everything `pick` reads besides the table and the request. Built from `usage.json`, `leases.json`, the
/// Accounts readers and `record.jsonl` by the CLI, and by hand in the tests.
#[derive(Debug, Clone, Default)]
pub struct World {
    pub now: u64,
    pub readings: Vec<Reading>,
    pub leases: Vec<Lease>,
    /// A CLI that cannot take an order, with why ("agy not signed in"). A CLI not listed is ready.
    pub clis: BTreeMap<String, String>,
    pub record: BTreeMap<RecordKey, Tally>,
    /// Overrides already accepted in the request's run.
    pub overrides_used: u32,
}

#[derive(Debug, Clone)]
pub struct Request {
    pub class: String,
    pub size: SizeName,
    pub needs: Vec<String>,
    pub not_family: Vec<String>,
    pub mode: Mode,
    /// The order being seated: its own lease does not count against it.
    pub order: Option<String>,
    /// The orchestrator's choice and its reason (§5.3 step 4).
    pub override_seat: Option<(String, String)>,
}

impl Request {
    pub fn new(class: &str) -> Self {
        Self {
            class: class.to_owned(),
            size: SizeName::Medium,
            needs: Vec::new(),
            not_family: Vec::new(),
            mode: Mode::Balanced,
            order: None,
            override_seat: None,
        }
    }
}

/// One window of a seat's pool as the rule saw it. Shares are 0 to 1.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowState {
    pub window: String,
    /// `None`: not read, read before a passed reset, or older than the staleness line. Unknown, never 0.
    #[serde(serialize_with = "six_opt")]
    pub used: Option<f32>,
    /// What live leases on the pool expect to spend.
    #[serde(serialize_with = "six")]
    pub booked: f32,
    /// `1 - used - booked`.
    #[serde(serialize_with = "six_opt")]
    pub left: Option<f32>,
    /// `left`, less the reserve on the reserve window.
    #[serde(serialize_with = "six_opt")]
    pub headroom: Option<f32>,
    pub resets_at: Option<u64>,
    #[serde(serialize_with = "six_opt")]
    pub pace: Option<f32>,
    #[serde(serialize_with = "six")]
    pub reset_factor: f32,
    /// This order's expected share of the window.
    #[serde(serialize_with = "six")]
    pub expected: f32,
}

impl WindowState {
    fn known(&self) -> bool {
        self.used.is_some()
    }

    /// Whether the order fits in what is left (after the reserve).
    fn holds(&self) -> bool {
        self.headroom
            .is_none_or(|room| self.expected <= room + EDGE)
    }

    fn pressure(&self) -> Option<f32> {
        let room = self.headroom?;
        if self.expected <= 0.0 {
            return Some(0.0);
        }
        Some(self.expected / room.max(1e-6) * self.pace.unwrap_or(1.0) * self.reset_factor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkipKind {
    Off,
    Cli,
    Need,
    Class,
    Family,
    Floor,
    Headroom,
    Reserve,
}

impl SkipKind {
    /// A skip that a cheaper effort could undo: the order's expected use decided it.
    fn priced(self) -> bool {
        matches!(self, Self::Headroom | Self::Reserve)
    }
}

#[derive(Debug, Clone)]
struct Skip {
    kind: SkipKind,
    why: String,
    /// When the window that skipped it resets.
    resets_at: Option<u64>,
}

#[derive(Debug, Clone)]
struct Candidate {
    index: usize,
    in_class: bool,
    effort: Option<Effort>,
    effort_note: Option<String>,
    score: f32,
    record_note: Option<String>,
    /// The use cell's tag: whether the expected use was measured or is a prior (§4).
    use_basis: Option<String>,
    windows: Vec<WindowState>,
    skip: Option<Skip>,
}

impl Candidate {
    fn pressure(&self) -> f32 {
        self.windows
            .iter()
            .filter_map(WindowState::pressure)
            .fold(0.0, f32::max)
    }

    fn unknown(&self) -> Vec<String> {
        self.windows
            .iter()
            .filter(|window| !window.known())
            .map(|window| window.window.clone())
            .collect()
    }
}

/// A seat that lost, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Loser {
    pub seat: String,
    pub effort: Option<Effort>,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pick {
    pub class: String,
    pub chain: String,
    pub mode: Mode,
    pub size: SizeName,
    pub seat: String,
    pub cli: String,
    pub model: String,
    pub effort: Effort,
    pub pool: String,
    pub family: String,
    #[serde(serialize_with = "six")]
    pub score: f32,
    /// The bar the seat cleared: the mode's, or the best left when none reached it.
    #[serde(serialize_with = "six")]
    pub bar: f32,
    pub bar_dropped: bool,
    #[serde(serialize_with = "six")]
    pub pressure: f32,
    /// Windows not read: the seat ranked below every seat with known headroom.
    pub unknown: Vec<String>,
    pub windows: Vec<WindowState>,
    /// Whether the expected use is measured, derived or a prior, and from what.
    pub use_basis: Option<String>,
    /// §5.3 step 5: the line the journal, `herdr seat list` and the Seats square show.
    pub reason: String,
    pub effort_note: Option<String>,
    /// The class's effort found no seat; this is one step down (§5.5).
    pub step_down: bool,
    #[serde(rename = "override")]
    pub override_reason: Option<String>,
    /// What the rule would have picked, when an override chose instead.
    pub rule_pick: Option<String>,
    pub runners_up: Vec<Loser>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoSeat {
    pub class: String,
    pub reason: String,
    /// The soonest a skipped seat's window resets: when to try again.
    pub earliest_reset: Option<u64>,
    /// The earliest reset is within `defer_min` (§5.6 step 2).
    pub defer: bool,
    pub skipped: Vec<Loser>,
}

struct Failure {
    no_seat: NoSeat,
    /// A skip the order's size decided: one effort step down may find a seat.
    priced: bool,
}

/// Decision 0029 §2.53: `min(1, ((passes + shrink × prior) / (orders + shrink)) / prior)`. A seat with no
/// record is unchanged; at or above the prior it is unchanged; below it falls as the record grows surer.
pub fn multiplier(passes: u32, orders: u32, prior: f32, shrink: f32) -> f32 {
    let shrunk = (passes as f32 + shrink * prior) / (orders as f32 + shrink);
    (shrunk / prior).min(1.0)
}

/// The effort a class gives a seat (§5.5). The class's effort and blast radius, or the seat's own departure;
/// SOTA lifts to the class's `sota_effort`; a step down under pressure; max never; then the nearest the seat
/// offers at or below it.
fn effort_for(
    class: &Class,
    entry: &ClassSeat,
    seat: &Seat,
    mode: Mode,
    step_down: bool,
) -> (Effort, Option<String>) {
    let mut want = entry.effort.unwrap_or(class.effort);
    if mode == Mode::Sota {
        if let Some(lift) = class.sota_effort {
            want = want.max(lift);
        }
    }
    if step_down {
        want = want.down().unwrap_or(want);
    }
    let mut note = None;
    if want == Effort::Max {
        want = Effort::Xhigh;
        note = Some("max is never chosen automatically".to_owned());
    }
    let offered: Vec<Effort> = seat
        .efforts
        .iter()
        .copied()
        .filter(|effort| *effort != Effort::Max)
        .collect();
    let got = offered
        .iter()
        .copied()
        .filter(|effort| *effort <= want)
        .max()
        .or_else(|| offered.iter().copied().min())
        .unwrap_or(want);
    if got != want {
        note = Some(format!(
            "{} has no {}; {}",
            seat.id,
            want.word(),
            got.word()
        ));
    }
    (got, note)
}

/// An f32 share as JSON prints it: six places, not the float's widening noise (O1's `round` in
/// nirvana_usage.rs); `+ 0.0` turns an empty sum's -0.0 into 0.
fn round6(value: f32) -> f64 {
    (f64::from(value) * 1e6).round() / 1e6 + 0.0
}

fn six<S: serde::Serializer>(value: &f32, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_f64(round6(*value))
}

fn six_opt<S: serde::Serializer>(value: &Option<f32>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.serialize_some(&round6(*value)),
        None => serializer.serialize_none(),
    }
}

fn six_map<S: serde::Serializer>(
    map: &BTreeMap<String, f32>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_map(map.iter().map(|(key, value)| (key, round6(*value))))
}

/// "41%", or "0.4%" under one percent.
fn pct(share: f32) -> String {
    let percent = share * 100.0;
    if percent > 0.0 && percent < 1.0 {
        format!("{percent:.1}%")
    } else {
        format!("{percent:.0}%")
    }
}

/// "max 5h 41% used, week 37% used": a seat's pool windows for a reason line.
fn windows_phrase(pool: &str, windows: &[WindowState]) -> String {
    let parts: Vec<String> = windows
        .iter()
        .map(|window| match window.used {
            Some(used) if window.booked > 0.0005 => format!(
                "{} {} used, {} booked",
                window.window,
                pct(used),
                pct(window.booked)
            ),
            Some(used) => format!("{} {} used", window.window, pct(used)),
            None => format!("{} not read", window.window),
        })
        .collect();
    if parts.is_empty() {
        pool.to_owned()
    } else {
        format!("{pool} {}", parts.join(", "))
    }
}

/// Seats an order: the rule of §5.3, deterministic and pure. A NoSeat names every seat the class takes and
/// why it was skipped, and the soonest reset that would free one.
pub fn pick(table: &Table, world: &World, request: &Request) -> Result<Pick, NoSeat> {
    let Some(class) = table.class(&request.class) else {
        return Err(NoSeat {
            class: request.class.clone(),
            reason: format!("no class {} in seats.toml", request.class),
            earliest_reset: None,
            defer: false,
            skipped: Vec::new(),
        });
    };
    match evaluate(table, world, request, class, false) {
        Ok(pick) => Ok(pick),
        Err(first) if first.priced && request.override_seat.is_none() => {
            match evaluate(table, world, request, class, true) {
                Ok(pick) => Ok(pick),
                Err(_) => Err(first.no_seat),
            }
        }
        Err(first) => Err(first.no_seat),
    }
}

fn evaluate(
    table: &Table,
    world: &World,
    request: &Request,
    class: &Class,
    step_down: bool,
) -> Result<Pick, Failure> {
    let constants = &table.constants;
    let size = table.size(request.size);
    let mut needs: Vec<String> = class.needs.clone();
    for need in &request.needs {
        if !needs.contains(need) {
            needs.push(need.clone());
        }
    }
    let mut candidates: Vec<Candidate> = table
        .seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            candidate(
                table, world, request, class, &needs, size, step_down, index, seat,
            )
        })
        .collect();

    // Filter 6: an order this window cannot hold, while another pool could hold it whole (§4.4). A pool
    // with a window not read is not counted as one that holds.
    let holders: Vec<(String, String)> = candidates
        .iter()
        .filter(|cand| cand.skip.is_none())
        .filter(|cand| {
            cand.windows
                .iter()
                .all(|window| window.known() && window.holds())
        })
        .map(|cand| {
            let seat = &table.seats[cand.index];
            (seat.pool.clone(), seat.id.clone())
        })
        .collect();
    for cand in candidates.iter_mut().filter(|cand| cand.skip.is_none()) {
        let seat = &table.seats[cand.index];
        let Some(short) = cand
            .windows
            .iter()
            .find(|window| window.known() && !window.holds())
        else {
            continue;
        };
        let Some((other_pool, other_seat)) = holders.iter().find(|(pool, _)| *pool != seat.pool)
        else {
            continue;
        };
        let reserve = if short.headroom != short.left {
            " after the reserve"
        } else {
            ""
        };
        cand.skip = Some(Skip {
            kind: SkipKind::Headroom,
            why: format!(
                "{} {} has {} left{reserve} for an order of {}; {other_seat} on {other_pool} can hold it",
                seat.pool,
                short.window,
                pct(short.headroom.unwrap_or(0.0)),
                pct(short.expected)
            ),
            resets_at: short.resets_at,
        });
    }

    // Filter 7: the orchestrator's reserve (§3).
    for cand in candidates.iter_mut().filter(|cand| cand.skip.is_none()) {
        let seat = &table.seats[cand.index];
        if seat.pool != constants.reserve_pool {
            continue;
        }
        let Some(window) = cand
            .windows
            .iter()
            .find(|window| window.window == constants.reserve_window)
        else {
            continue;
        };
        if let Some(left) = window.left {
            let after = left - window.expected;
            if after + EDGE < constants.reserve {
                cand.skip = Some(Skip {
                    kind: SkipKind::Reserve,
                    why: format!(
                        "would leave {} {} at {}, under the orchestrator's reserve {}",
                        seat.pool,
                        window.window,
                        pct(after.max(0.0)),
                        pct(constants.reserve)
                    ),
                    resets_at: window.resets_at,
                });
            }
        }
    }

    let skipped_list = |candidates: &[Candidate]| -> Vec<Loser> {
        candidates
            .iter()
            .filter(|cand| cand.in_class)
            .filter_map(|cand| {
                Some(Loser {
                    seat: table.seats[cand.index].id.clone(),
                    effort: cand.effort,
                    why: cand.skip.as_ref()?.why.clone(),
                })
            })
            .collect()
    };
    let earliest = |candidates: &[Candidate]| -> Option<u64> {
        candidates
            .iter()
            .filter(|cand| cand.in_class)
            .filter_map(|cand| cand.skip.as_ref()?.resets_at)
            .filter(|at| *at > world.now)
            .min()
    };
    let priced = candidates
        .iter()
        .any(|cand| cand.skip.as_ref().is_some_and(|skip| skip.kind.priced()));
    let fail = |reason: String, candidates: &[Candidate], priced: bool| -> Failure {
        let earliest_reset = earliest(candidates);
        Failure {
            no_seat: NoSeat {
                class: class.id.clone(),
                reason,
                earliest_reset,
                defer: earliest_reset
                    .is_some_and(|at| at.saturating_sub(world.now) <= constants.defer_min * 60),
                skipped: skipped_list(candidates),
            },
            priced,
        }
    };

    if let Some((seat_id, reason)) = &request.override_seat {
        if world.overrides_used >= constants.overrides_per_run {
            return Err(fail(
                format!(
                    "override {seat_id} refused: {} of {} overrides already taken in this run; more is the owner's call",
                    world.overrides_used, constants.overrides_per_run
                ),
                &candidates,
                false,
            ));
        }
        let Some(cand) = candidates
            .iter()
            .find(|cand| table.seats[cand.index].id == *seat_id)
        else {
            return Err(fail(
                format!("override {seat_id} refused: no such seat"),
                &candidates,
                false,
            ));
        };
        if let Some(skip) = &cand.skip {
            return Err(fail(
                format!("override {seat_id} refused: {}", skip.why),
                &candidates,
                false,
            ));
        }
        let rule = rank(table, &candidates, class, request.mode).ok();
        let rule_pick = rule.as_ref().map(|(order, _, _)| {
            let winner = &candidates[order[0]];
            format!(
                "{} {}",
                table.seats[winner.index].id,
                winner.effort.map_or("", Effort::word)
            )
        });
        let bar = table.bars(class).at(request.mode);
        let index = candidates
            .iter()
            .position(|cand| table.seats[cand.index].id == *seat_id)
            .expect("found above");
        let mut out = build_pick(
            table,
            &candidates,
            class,
            request,
            index,
            &[],
            bar,
            false,
            step_down,
        );
        out.override_reason = Some(reason.clone());
        out.rule_pick = rule_pick.clone();
        out.reason = format!(
            "{}; override: {reason}; the rule's pick: {}",
            out.reason,
            rule_pick.as_deref().unwrap_or("none")
        );
        return Ok(out);
    }

    if candidates.iter().all(|cand| cand.skip.is_some()) {
        let whys: Vec<String> = skipped_list(&candidates)
            .into_iter()
            .map(|loser| format!("{} ({})", loser.seat, loser.why))
            .collect();
        return Err(fail(
            format!("no seat left for {}: {}", class.id, whys.join("; ")),
            &candidates,
            priced,
        ));
    }
    match rank(table, &candidates, class, request.mode) {
        Ok((order, bar, dropped)) => Ok(build_pick(
            table,
            &candidates,
            class,
            request,
            order[0],
            &order[1..],
            bar,
            dropped,
            step_down,
        )),
        Err(reason) => Err(fail(reason, &candidates, priced)),
    }
}

/// Every filter of step 1 for one seat, its effort, its expected use and its windows.
fn candidate(
    table: &Table,
    world: &World,
    request: &Request,
    class: &Class,
    needs: &[String],
    size: &Size,
    step_down: bool,
    index: usize,
    seat: &Seat,
) -> Candidate {
    let constants = &table.constants;
    let entry = class.seats.iter().find(|entry| entry.seat == seat.id);
    let mut cand = Candidate {
        index,
        in_class: entry.is_some(),
        effort: None,
        effort_note: None,
        score: entry.map_or(0.0, |entry| entry.score),
        record_note: None,
        use_basis: None,
        windows: Vec::new(),
        skip: None,
    };
    let skip = |kind: SkipKind, why: String| {
        Some(Skip {
            kind,
            why,
            resets_at: None,
        })
    };
    if !seat.enabled {
        cand.skip = skip(SkipKind::Off, "off in seats.toml".into());
        return cand;
    }
    if let Some(why) = world.clis.get(&seat.cli) {
        cand.skip = skip(SkipKind::Cli, why.clone());
        return cand;
    }
    if let Some(need) = needs.iter().find(|need| !seat.capabilities.contains(need)) {
        cand.skip = skip(SkipKind::Need, format!("lacks {need}"));
        return cand;
    }
    let Some(entry) = entry else {
        cand.skip = skip(SkipKind::Class, format!("{} does not take it", class.id));
        return cand;
    };
    if !entry.only_with.iter().all(|need| needs.contains(need)) {
        cand.skip = skip(
            SkipKind::Class,
            format!(
                "{} takes it only for {}",
                class.id,
                entry.only_with.join(", ")
            ),
        );
        return cand;
    }
    if request.not_family.contains(&seat.family) {
        cand.skip = skip(
            SkipKind::Family,
            format!(
                "family {} excluded (another family reviews, decision 0017)",
                seat.family
            ),
        );
        return cand;
    }
    let (effort, note) = effort_for(class, entry, seat, request.mode, step_down);
    cand.effort = Some(effort);
    cand.effort_note = note;
    if table.use_record {
        let key = (seat.id.clone(), effort, class.chain.clone());
        if let Some(tally) = world.record.get(&key) {
            let factor = multiplier(
                tally.passes,
                tally.orders,
                constants.record_prior,
                constants.record_shrink,
            );
            cand.score *= factor;
            cand.record_note = Some(format!(
                "record {} of {} passed, x{factor:.3}",
                tally.passes, tally.orders
            ));
        }
    }
    let Some(pool) = table.pool(&seat.pool) else {
        return cand;
    };
    let cell = seat.use_cell(effort);
    cand.use_basis =
        cell.map(|cell| format!("{} at {}: {}", seat.id, cell.effort.word(), cell.basis));
    let scale = cell
        .and_then(|cell| date(&cell.at))
        .map_or(1.0, |at| seat.price_scale(at, world.now))
        * class.weight
        * size.use_factor;
    let fallback = cell.map_or(0.0, |cell| {
        cell.windows.values().copied().fold(0.0, f32::max)
    });
    for name in seat_windows(seat, pool) {
        let expected = cell
            .and_then(|cell| cell.windows.get(&name).copied())
            .unwrap_or(fallback)
            * scale;
        let reading = world
            .readings
            .iter()
            .find(|reading| reading.pool == seat.pool && reading.window == name)
            .filter(|reading| {
                reading.used.is_some()
                    && nirvana_usage::freshness(reading, world.now) == Freshness::Fresh
            });
        let booked: f32 = world
            .leases
            .iter()
            .filter(|lease| lease.live(world.now) && lease.pool == seat.pool)
            .filter(|lease| request.order.as_deref() != Some(lease.order.as_str()))
            .filter_map(|lease| lease.expected.get(&name))
            .fold(0.0, |sum, share| sum + share);
        let used = reading.and_then(|reading| reading.used);
        let left = used.map(|used| 1.0 - used - booked);
        let reserve = if seat.pool == constants.reserve_pool && name == constants.reserve_window {
            constants.reserve
        } else {
            0.0
        };
        let resets_at = reading.and_then(|reading| reading.resets_at);
        let pace = reading.and_then(|reading| {
            nirvana_usage::pace_with(
                reading,
                constants.pace_ease,
                constants.pace_min,
                constants.pace_max,
            )
        });
        let reset_factor = match (used, resets_at) {
            (Some(used), Some(at)) => reset_factor(
                1.0 - used,
                at.saturating_sub(world.now),
                size.minutes,
                constants,
            ),
            _ => 1.0,
        };
        cand.windows.push(WindowState {
            window: name,
            used,
            booked,
            left,
            headroom: left.map(|left| left - reserve),
            resets_at,
            pace,
            reset_factor,
            expected,
        });
    }
    // Filter 5: a window at or under the floor.
    if let Some(window) = cand.windows.iter().find(|window| {
        window
            .left
            .is_some_and(|left| left <= constants.floor + EDGE)
    }) {
        let booked = if window.booked > 0.0005 {
            format!(" and {} booked", pct(window.booked))
        } else {
            String::new()
        };
        cand.skip = Some(Skip {
            kind: SkipKind::Floor,
            why: format!(
                "{} {} {} used{booked}: at or under the floor {}",
                seat.pool,
                window.window,
                pct(window.used.unwrap_or(1.0)),
                pct(constants.floor)
            ),
            resets_at: window.resets_at,
        });
    }
    cand
}

/// §5.3: a window with more than `reset_left` left and under `reset_within_min` to its reset is discounted by
/// up to `reset_discount`, scaled by the share of the order that runs before the reset.
pub fn reset_factor(left: f32, resets_in_s: u64, order_minutes: u64, constants: &Constants) -> f32 {
    if left <= constants.reset_left || resets_in_s >= constants.reset_within_min * 60 {
        return 1.0;
    }
    let share = (resets_in_s as f32 / (order_minutes.max(1) * 60) as f32).min(1.0);
    1.0 - constants.reset_discount * share
}

/// The bar, then pressure: the survivors that clear the bar in rank order, the bar used, and whether it
/// dropped. An error says why no survivor reaches the Efficient bar.
fn rank(
    table: &Table,
    candidates: &[Candidate],
    class: &Class,
    mode: Mode,
) -> Result<(Vec<usize>, f32, bool), String> {
    let bars = table.bars(class);
    let survivors: Vec<usize> = (0..candidates.len())
        .filter(|&i| candidates[i].skip.is_none())
        .collect();
    let Some(&best) = survivors
        .iter()
        .max_by(|&&a, &&b| candidates[a].score.total_cmp(&candidates[b].score))
    else {
        return Err(format!("no seat left for {}", class.id));
    };
    let best_score = candidates[best].score;
    let wanted = bars.at(mode);
    let (bar, dropped) = if best_score + EDGE >= wanted {
        (wanted, false)
    } else if best_score + EDGE >= bars.efficient {
        // SOTA's bar is 100 so that it means the best seat left (decision 0018 §2): not a drop to report.
        (best_score, mode != Mode::Sota)
    } else {
        return Err(format!(
            "no seat left for {} reaches its Efficient bar {:.0}; best left {} at {:.1}",
            class.id, bars.efficient, table.seats[candidates[best].index].id, best_score
        ));
    };
    let mut order: Vec<usize> = survivors
        .into_iter()
        .filter(|&i| candidates[i].score + EDGE >= bar)
        .collect();
    order.sort_by(|&a, &b| {
        let (a, b) = (&candidates[a], &candidates[b]);
        (!a.unknown().is_empty())
            .cmp(&!b.unknown().is_empty())
            .then(a.pressure().total_cmp(&b.pressure()))
            .then(b.score.total_cmp(&a.score))
            .then(a.index.cmp(&b.index))
    });
    Ok((order, bar, dropped))
}

/// The pick, its reason line and every other seat with why it lost.
fn build_pick(
    table: &Table,
    candidates: &[Candidate],
    class: &Class,
    request: &Request,
    winner: usize,
    beaten: &[usize],
    bar: f32,
    dropped: bool,
    step_down: bool,
) -> Pick {
    let constants = &table.constants;
    let won = &candidates[winner];
    let seat = &table.seats[won.index];
    let effort = won.effort.unwrap_or(class.effort);
    let mode_bar = table.bars(class).at(request.mode);
    let lost_to = |cand: &Candidate| -> String {
        let (a, b) = (cand.pressure(), won.pressure());
        if !cand.unknown().is_empty() && won.unknown().is_empty() {
            format!(
                "{} not read: ranks below known headroom",
                cand.unknown().join(", ")
            )
        } else if (a - b).abs() > 1e-6 {
            format!("pressure {a:.4} against {b:.4}")
        } else if (cand.score - won.score).abs() > EDGE {
            format!(
                "same pressure, scores {:.1} against {:.1}",
                cand.score, won.score
            )
        } else {
            "same pressure and score, later in the table".to_owned()
        }
    };
    let mut runners_up: Vec<Loser> = beaten
        .iter()
        .map(|&i| {
            let cand = &candidates[i];
            Loser {
                seat: table.seats[cand.index].id.clone(),
                effort: cand.effort,
                why: lost_to(cand),
            }
        })
        .collect();
    for (i, cand) in candidates.iter().enumerate() {
        if i == winner || beaten.contains(&i) || !cand.in_class {
            continue;
        }
        let why = match &cand.skip {
            Some(skip) => skip.why.clone(),
            None => format!(
                "scores {:.1}, under the bar {bar:.0} at {}",
                cand.score,
                request.mode.word()
            ),
        };
        runners_up.push(Loser {
            seat: table.seats[cand.index].id.clone(),
            effort: cand.effort,
            why,
        });
    }
    let mut reason = format!(
        "{} \u{2192} {} {}: {}",
        class.id,
        seat.id,
        effort.word(),
        windows_phrase(&seat.pool, &won.windows)
    );
    if seat.pool == constants.reserve_pool {
        reason.push_str(&format!(", reserve {}", pct(constants.reserve)));
    }
    if dropped {
        reason.push_str(&format!(
            "; bar dropped to {bar:.0} (none left reached {mode_bar:.0})"
        ));
    }
    if step_down {
        reason.push_str("; one effort step down (the class's effort found no seat)");
    }
    if let Some(note) = &won.effort_note {
        reason.push_str(&format!("; {note}"));
    }
    if let Some(note) = &won.record_note {
        reason.push_str(&format!("; {note}"));
    }
    if let Some(&next) = beaten.first() {
        let next = &candidates[next];
        let next_seat = &table.seats[next.index];
        let phrase = windows_phrase(&next_seat.pool, &next.windows);
        if (next.score - won.score).abs() <= EDGE {
            reason.push_str(&format!("; {} scores the same ({phrase})", next_seat.id));
        } else {
            reason.push_str(&format!("; next {} ({phrase})", next_seat.id));
        }
    }
    Pick {
        class: class.id.clone(),
        chain: class.chain.clone(),
        mode: request.mode,
        size: request.size,
        seat: seat.id.clone(),
        cli: seat.cli.clone(),
        model: seat.model_for(effort),
        effort,
        pool: seat.pool.clone(),
        family: seat.family.clone(),
        score: won.score,
        bar,
        bar_dropped: dropped,
        pressure: won.pressure(),
        unknown: won.unknown(),
        windows: won.windows.clone(),
        use_basis: won.use_basis.clone(),
        reason,
        effort_note: won.effort_note.clone(),
        step_down,
        override_reason: None,
        rule_pick: None,
        runners_up,
    }
}

// ---- leases: orders in flight (§3, Argon's review F1) ----

/// One order seated and not yet finished: its pool's headroom subtracts what it expects to spend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lease {
    pub order: String,
    pub seat: String,
    pub effort: Effort,
    pub pool: String,
    pub class: String,
    #[serde(default)]
    pub run: Option<String>,
    #[serde(rename = "use", serialize_with = "six_map")]
    pub expected: BTreeMap<String, f32>,
    pub booked_at: u64,
    pub expires_at: u64,
    /// The pick as first given, so a second booking for the same order returns it unchanged.
    #[serde(default)]
    pub pick: serde_json::Value,
}

impl Lease {
    pub fn live(&self, now: u64) -> bool {
        self.expires_at > now
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Leases {
    pub version: u32,
    #[serde(default)]
    pub leases: Vec<Lease>,
    /// Per run id: the overrides accepted (§5.3 step 4).
    #[serde(default)]
    pub runs: BTreeMap<String, RunCount>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunCount {
    pub overrides: u32,
}

impl Leases {
    pub fn live(&self, order: &str, now: u64) -> Option<&Lease> {
        self.leases
            .iter()
            .find(|lease| lease.order == order && lease.live(now))
    }

    /// Books an order, idempotent per order id: a live lease for the same order is returned as it was, and
    /// nothing is written over it. Expired leases are dropped.
    pub fn book(&mut self, lease: Lease, now: u64) -> Lease {
        self.leases.retain(|known| known.live(now));
        if let Some(known) = self.leases.iter().find(|known| known.order == lease.order) {
            return known.clone();
        }
        self.leases.push(lease.clone());
        lease
    }

    /// Drops an order's lease, and every expired one; whether the order had a live lease.
    pub fn release(&mut self, order: &str, now: u64) -> bool {
        let had = self.live(order, now).is_some();
        self.leases
            .retain(|lease| lease.order != order && lease.live(now));
        had
    }

    pub fn overrides(&self, run: &str) -> u32 {
        self.runs.get(run).map_or(0, |count| count.overrides)
    }
}

/// The lease a pick books for an order.
pub fn lease_for(table: &Table, pick: &Pick, order: &str, run: Option<&str>, now: u64) -> Lease {
    Lease {
        order: order.to_owned(),
        seat: pick.seat.clone(),
        effort: pick.effort,
        pool: pick.pool.clone(),
        class: pick.class.clone(),
        run: run.map(str::to_owned),
        expected: pick
            .windows
            .iter()
            .map(|window| (window.window.clone(), window.expected))
            .collect(),
        booked_at: now,
        expires_at: now + table.size(pick.size).lease_minutes * 60,
        pick: serde_json::to_value(pick).unwrap_or_default(),
    }
}

pub fn read_leases(path: &Path) -> Leases {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// A temp file, then a rename, as `usage.json` is written (O1): a reader never sees half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map_or_else(|| "file".into(), |name| name.to_string_lossy());
    let temp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

pub fn write_leases(path: &Path, leases: &Leases) -> std::io::Result<()> {
    let mut leases = leases.clone();
    leases.version = 1;
    write_atomic(path, &serde_json::to_vec_pretty(&leases)?)
}

/// Holds `leases.lock` while it lives. A read, a pick and a write of `leases.json` happen under it, so four
/// parallel picks in five seconds see each other's bookings (Argon's review F1); the rename alone keeps the
/// file whole but would let two writers lose one lease.
pub struct Lock(PathBuf);

impl Lock {
    /// Waits up to 10 s for the lock.
    pub fn take(dir: &Path) -> std::io::Result<Self> {
        Self::try_take(dir, 200)
    }

    /// Tries `attempts` times, 50 ms apart; a lock left over `LOCK_STALE` ago is taken over.
    pub fn try_take(dir: &Path, attempts: u32) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(LOCK_FILE);
        for _ in 0..attempts {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = write!(file, "{}", std::process::id());
                    return Ok(Self(path));
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|at| SystemTime::now().duration_since(at).ok())
                        .is_some_and(|age| age > LOCK_STALE);
                    if stale {
                        let _ = std::fs::remove_file(&path);
                    } else {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                Err(err) => return Err(err),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{} is held by another herdr", path.display()),
        ))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

// ---- the record (decision 0029, plan 12 §4) ----

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub cache_write: u64,
    #[serde(default)]
    pub cache_read: u64,
    #[serde(default)]
    pub output: u64,
}

/// One finished order, one line of `record.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordLine {
    pub order: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    pub seat: String,
    #[serde(default)]
    pub model: Option<String>,
    pub effort: Effort,
    pub class: String,
    #[serde(default)]
    pub chain: Option<String>,
    /// The gate's verdict; "pass" counts as a pass in the blend, any other word as a failure.
    pub outcome: String,
    #[serde(default)]
    pub tokens: Option<Tokens>,
    /// The order's share of each window, computed from its tokens when not given.
    #[serde(default, rename = "use")]
    pub spent: Option<BTreeMap<String, f32>>,
    #[serde(default)]
    pub at: Option<u64>,
}

/// The record's key: seat, effort and chain. By seat and effort, never by provider (decision 0029: "a bad
/// row is not a bad service"); by chain in the first version, with the class kept on each line.
pub type RecordKey = (String, Effort, String);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub passes: u32,
    pub orders: u32,
}

pub fn tally(lines: &[RecordLine]) -> BTreeMap<RecordKey, Tally> {
    let mut tallies: BTreeMap<RecordKey, Tally> = BTreeMap::new();
    for line in lines {
        let Some(chain) = &line.chain else {
            continue;
        };
        let entry = tallies
            .entry((line.seat.clone(), line.effort, chain.clone()))
            .or_default();
        entry.orders += 1;
        if line.outcome == "pass" {
            entry.passes += 1;
        }
    }
    tallies
}

/// An order's share of each window of its seat, from its own tokens at the price of its day (§4, Argon's F8).
pub fn computed_use(
    table: &Table,
    seat: &Seat,
    tokens: &Tokens,
    at: u64,
) -> Option<BTreeMap<String, f32>> {
    let price = seat.price_at(at)?;
    let pool = table.pool(&seat.pool)?;
    let usd = cost(price, tokens);
    Some(
        seat_windows(seat, pool)
            .into_iter()
            .filter_map(|window| {
                let per = pool.per_usd.get(&window)?;
                Some((window, (usd * per) as f32))
            })
            .collect(),
    )
}

/// Checks a line against the table and fills what herdr knows: the chain, the model, the time and the use.
pub fn complete(table: &Table, mut line: RecordLine, now: u64) -> Result<RecordLine, String> {
    let seat = table
        .seat(&line.seat)
        .ok_or_else(|| format!("no seat {} in seats.toml", line.seat))?;
    let class = table
        .class(&line.class)
        .ok_or_else(|| format!("no class {} in seats.toml", line.class))?;
    if line.order.trim().is_empty() || line.outcome.trim().is_empty() {
        return Err("a record line names its order and its outcome".into());
    }
    let at = *line.at.get_or_insert(now);
    line.chain.get_or_insert_with(|| class.chain.clone());
    line.model
        .get_or_insert_with(|| seat.model_for(line.effort));
    if line.spent.is_none() {
        if let Some(tokens) = &line.tokens {
            line.spent = computed_use(table, seat, tokens, at);
        }
    }
    Ok(line)
}

/// Appends one line in one write.
pub fn append_record(path: &Path, line: &RecordLine) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = serde_json::to_string(line)?;
    text.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

/// The record's lines; a line that does not parse is skipped.
pub fn read_record(path: &Path) -> Vec<RecordLine> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect()
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr seat list [--json]
       herdr seat pick --class <class> [--size small|medium|large] [--needs a,b] [--not-family f]
                       [--mode efficient|balanced|sota] [--book <order>] [--run <id>]
                       [--override <seat> --reason \"...\"] [--json]
       herdr seat release <order>
       herdr seat record --json '<line>'
pick exits 0 with a seat, 3 with none (a refused override included), 2 on a usage or table error.";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn now() -> u64 {
    nirvana_usage::unix(SystemTime::now())
}

fn load_table() -> Table {
    let path = dir().join(SEATS_FILE);
    match parse(&read_or_create(&path)) {
        Ok(table) => table,
        Err(problems) => {
            eprintln!(
                "herdr seat: {} is refused (delete it to get herdr's default back):\n{problems}",
                path.display()
            );
            std::process::exit(2);
        }
    }
}

/// Why a CLI cannot take an order, from the Accounts readers; `None` when it is signed in.
fn cli_state(cli: &str) -> Option<String> {
    use crate::nirvana_accounts::{read, Cli, Signed};
    let which = match cli {
        "claude" => Cli::Claude,
        "agy" => Cli::Antigravity,
        "codex" => Cli::Codex,
        other => return Some(format!("{other} is not a CLI herdr knows")),
    };
    match read(which) {
        Signed::In(_) => None,
        Signed::NotInstalled => Some(format!("{cli} not installed")),
        Signed::Out | Signed::Reading => Some(format!("{cli} not signed in")),
    }
}

/// The world as this machine has it: usage from `usage.json` (stale readers read first, as `herdr usage`
/// does), the accounts, and the record when the table blends it.
fn live_world(table: &Table) -> World {
    let sources = nirvana_usage::current_sources(false);
    let mut clis = BTreeMap::new();
    for seat in table.seats.iter().filter(|seat| seat.enabled) {
        if !clis.contains_key(&seat.cli) {
            clis.insert(seat.cli.clone(), cli_state(&seat.cli));
        }
    }
    World {
        now: now(),
        readings: sources
            .iter()
            .flat_map(|source| source.readings.iter().cloned())
            .collect(),
        leases: Vec::new(),
        clis: clis
            .into_iter()
            .filter_map(|(cli, why)| Some((cli, why?)))
            .collect(),
        record: if table.use_record {
            tally(&read_record(&dir().join(RECORD_FILE)))
        } else {
            BTreeMap::new()
        },
        overrides_used: 0,
    }
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("list") => list_cli(&args[1..]),
        Some("pick") => pick_cli(&args[1..]),
        Some("release") => release_cli(&args[1..]),
        Some("record") => record_cli(&args[1..]),
        _ => usage_exit(),
    }
}

fn list_cli(args: &[String]) -> std::io::Result<()> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => usage_exit(),
    };
    let table = load_table();
    let mut world = live_world(&table);
    let dir = dir();
    world.leases = read_leases(&dir.join(LEASES_FILE)).leases;
    let view = list_view(&table, &world);
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        print!("{}", list_text(&table, &world, &dir));
    }
    Ok(())
}

/// Each seat with its pool's windows as a pick at medium would see them (no order: nothing expected).
fn seat_states(table: &Table, world: &World) -> Vec<(usize, Option<String>, Vec<WindowState>)> {
    let constants = &table.constants;
    table
        .seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            let why = if !seat.enabled {
                Some("off in seats.toml".to_owned())
            } else {
                world.clis.get(&seat.cli).cloned()
            };
            let windows = table
                .pool(&seat.pool)
                .map(|pool| seat_windows(seat, pool))
                .unwrap_or_default()
                .into_iter()
                .map(|name| {
                    let reading = world
                        .readings
                        .iter()
                        .find(|r| r.pool == seat.pool && r.window == name)
                        .filter(|r| {
                            r.used.is_some()
                                && nirvana_usage::freshness(r, world.now) == Freshness::Fresh
                        });
                    let booked: f32 = world
                        .leases
                        .iter()
                        .filter(|lease| lease.live(world.now) && lease.pool == seat.pool)
                        .filter_map(|lease| lease.expected.get(&name))
                        .fold(0.0, |sum, share| sum + share);
                    let used = reading.and_then(|r| r.used);
                    let left = used.map(|used| 1.0 - used - booked);
                    let reserve = if seat.pool == constants.reserve_pool
                        && name == constants.reserve_window
                    {
                        constants.reserve
                    } else {
                        0.0
                    };
                    WindowState {
                        window: name,
                        used,
                        booked,
                        left,
                        headroom: left.map(|left| left - reserve),
                        resets_at: reading.and_then(|r| r.resets_at),
                        pace: reading.and_then(|r| {
                            nirvana_usage::pace_with(
                                r,
                                constants.pace_ease,
                                constants.pace_min,
                                constants.pace_max,
                            )
                        }),
                        reset_factor: 1.0,
                        expected: 0.0,
                    }
                })
                .collect();
            (index, why, windows)
        })
        .collect()
}

/// `herdr seat list --json`.
pub fn list_view(table: &Table, world: &World) -> serde_json::Value {
    use serde_json::json;
    let seats: Vec<serde_json::Value> = seat_states(table, world)
        .into_iter()
        .map(|(index, why, windows)| {
            let seat = &table.seats[index];
            json!({
                "seat": seat.id,
                "cli": seat.cli,
                "model": seat.model,
                "family": seat.family,
                "pool": seat.pool,
                "efforts": seat.efforts,
                "capabilities": seat.capabilities,
                "enabled": seat.enabled,
                "prices": seat.prices.iter().map(|price| json!({"from": price.from, "input": price.input, "output": price.output, "cache_read": price.cache_read, "cache_write": price.cache_write, "basis": price.basis})).collect::<Vec<_>>(),
                "use": seat.uses.iter().map(|cell| json!({"effort": cell.effort, "windows": cell.windows, "at": cell.at, "basis": cell.basis})).collect::<Vec<_>>(),
                "ready": why.is_none(),
                "why_not": why,
                "windows": windows,
            })
        })
        .collect();
    let pools: Vec<serde_json::Value> = table
        .pools
        .iter()
        .map(|pool| json!({"pool": pool.id, "windows": pool.windows, "per_usd": pool.per_usd, "basis": pool.basis}))
        .collect();
    let classes: Vec<serde_json::Value> = table
        .classes
        .iter()
        .map(|class| {
            json!({
                "class": class.id,
                "chain": class.chain,
                "effort": class.effort,
                "bars": table.bars(class),
                "needs": class.needs,
                "seats": class.seats.iter().map(|entry| json!({"seat": entry.seat, "score": entry.score})).collect::<Vec<_>>(),
                "why": class.why,
            })
        })
        .collect();
    json!({
        "now": world.now,
        "mode": table.mode,
        "use_record": table.use_record,
        "chains": table.chains(),
        "pools": pools,
        "classes": classes,
        "seats": seats,
        "leases": world.leases.iter().filter(|lease| lease.live(world.now)).collect::<Vec<_>>(),
    })
}

fn list_text(table: &Table, world: &World, dir: &Path) -> String {
    let mut out = format!(
        "mode {} \u{b7} record {} \u{b7} {}\n",
        table.mode.word(),
        if table.use_record {
            "blended"
        } else {
            "logged, not blended"
        },
        dir.join(SEATS_FILE).display()
    );
    out.push_str(&format!(
        "{:<9} {:<6} {:<26} {:<9} {:<34} {}\n",
        "seat", "cli", "model", "pool", "left (5h, week)", "state"
    ));
    for (index, why, windows) in seat_states(table, world) {
        let seat = &table.seats[index];
        let left: Vec<String> = windows
            .iter()
            .map(|window| match window.headroom {
                Some(room) if Some(room) != window.left => format!(
                    "{} {} ({} after reserve)",
                    window.window,
                    pct(window.left.unwrap_or(0.0)),
                    pct(room)
                ),
                Some(room) => format!("{} {}", window.window, pct(room)),
                None => format!("{} not read", window.window),
            })
            .collect();
        out.push_str(&format!(
            "{:<9} {:<6} {:<26} {:<9} {:<34} {}\n",
            seat.id,
            seat.cli,
            seat.model,
            seat.pool,
            left.join(", "),
            why.as_deref().unwrap_or("ready")
        ));
    }
    let live: Vec<&Lease> = world
        .leases
        .iter()
        .filter(|lease| lease.live(world.now))
        .collect();
    if !live.is_empty() {
        out.push_str("orders in flight:\n");
        for lease in live {
            out.push_str(&format!(
                "  {} {} {} on {} ({}), expires in {}\n",
                lease.order,
                lease.seat,
                lease.effort.word(),
                lease.pool,
                lease.class,
                nirvana_usage::short_duration(lease.expires_at.saturating_sub(world.now))
            ));
        }
    }
    out
}

fn list_arg(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect()
}

fn pick_cli(args: &[String]) -> std::io::Result<()> {
    let table = load_table();
    let mut request = Request::new("");
    request.mode = table.mode;
    let mut json = false;
    let mut book: Option<String> = None;
    let mut run: Option<String> = None;
    let mut over: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--class" => request.class = value(),
            "--size" => request.size = SizeName::parse(&value()).unwrap_or_else(|| usage_exit()),
            "--needs" => request.needs = list_arg(&value()),
            "--not-family" => request.not_family = list_arg(&value()),
            "--mode" => request.mode = Mode::parse(&value()).unwrap_or_else(|| usage_exit()),
            "--book" => book = Some(value()),
            "--run" => run = Some(value()),
            "--override" => over = Some(value()),
            "--reason" => reason = Some(value()),
            "--json" => json = true,
            _ => usage_exit(),
        }
    }
    if table.class(&request.class).is_none() {
        let known: Vec<&str> = table
            .classes
            .iter()
            .map(|class| class.id.as_str())
            .collect();
        eprintln!(
            "herdr seat pick: no class {:?}; the classes: {}",
            request.class,
            known.join(", ")
        );
        std::process::exit(2);
    }
    if let Some(seat) = &over {
        let (Some(reason), Some(_)) = (&reason, &run) else {
            eprintln!(
                "herdr seat pick: --override needs --reason and --run (at most {} per run)",
                table.constants.overrides_per_run
            );
            std::process::exit(2);
        };
        request.override_seat = Some((seat.clone(), reason.clone()));
    }
    request.order = book.clone();
    let mut world = live_world(&table);
    let dir = dir();
    let leases_path = dir.join(LEASES_FILE);
    let lock = Lock::take(&dir)?;
    let mut leases = read_leases(&leases_path);
    world.now = now();
    if let Some(order) = &book {
        if let Some(known) = leases.live(order, world.now) {
            let mut out = known.pick.clone();
            out["booked"] = "existing".into();
            out["lease"] = lease_view(known)?;
            drop(lock);
            return print_pick(&out, json);
        }
    }
    world.leases = leases.leases.clone();
    world.overrides_used = run.as_deref().map_or(0, |run| leases.overrides(run));
    let result = pick(&table, &world, &request);
    let out = match result {
        Ok(chosen) => {
            let mut out = serde_json::to_value(&chosen)?;
            let mut changed = false;
            if chosen.override_reason.is_some() {
                if let Some(run) = &run {
                    leases.runs.entry(run.clone()).or_default().overrides += 1;
                    changed = true;
                }
            }
            if let Some(order) = &book {
                let lease = lease_for(&table, &chosen, order, run.as_deref(), world.now);
                let booked = leases.book(lease, world.now);
                out["booked"] = "new".into();
                out["lease"] = lease_view(&booked)?;
                changed = true;
            }
            if changed {
                write_leases(&leases_path, &leases)?;
            }
            out
        }
        Err(no_seat) => {
            drop(lock);
            let mut out = serde_json::to_value(&no_seat)?;
            out["no_seat"] = true.into();
            if json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!("no seat: {}", no_seat.reason);
                if let Some(at) = no_seat.earliest_reset {
                    println!(
                        "earliest reset in {}{}",
                        nirvana_usage::short_duration(at.saturating_sub(world.now)),
                        if no_seat.defer { ": defer" } else { "" }
                    );
                }
                for loser in &no_seat.skipped {
                    println!("  {}: {}", loser.seat, loser.why);
                }
            }
            std::io::stdout().flush()?;
            std::process::exit(NO_SEAT_EXIT);
        }
    };
    drop(lock);
    print_pick(&out, json)
}

/// A lease as the CLI prints it: its stored pick is the answer itself, so it is left out.
fn lease_view(lease: &Lease) -> serde_json::Result<serde_json::Value> {
    let mut lease = lease.clone();
    lease.pick = serde_json::Value::Null;
    let mut value = serde_json::to_value(&lease)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("pick");
    }
    Ok(value)
}

fn print_pick(out: &serde_json::Value, json: bool) -> std::io::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(out)?);
    } else {
        let text = |key: &str| {
            out.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        };
        println!("{}", text("reason"));
        println!(
            "seat {} \u{b7} {} {} \u{b7} effort {} \u{b7} pool {}{}",
            text("seat"),
            text("cli"),
            text("model"),
            text("effort"),
            text("pool"),
            match out.get("booked").and_then(serde_json::Value::as_str) {
                Some(booked) => format!(" \u{b7} booked ({booked})"),
                None => String::new(),
            }
        );
        for loser in out
            .get("runners_up")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let field = |key: &str| {
                loser
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
            };
            println!(
                "  {}: {}",
                format!("{} {}", field("seat"), field("effort")).trim_end(),
                field("why")
            );
        }
    }
    std::io::stdout().flush()
}

fn release_cli(args: &[String]) -> std::io::Result<()> {
    let [order] = args else { usage_exit() };
    let dir = dir();
    let path = dir.join(LEASES_FILE);
    let _lock = Lock::take(&dir)?;
    let mut leases = read_leases(&path);
    if leases.release(order, now()) {
        write_leases(&path, &leases)?;
        println!("released {order}");
    } else {
        println!("no live lease for {order}");
    }
    Ok(())
}

fn record_cli(args: &[String]) -> std::io::Result<()> {
    let [flag, text] = args else { usage_exit() };
    if flag != "--json" {
        usage_exit();
    }
    let table = load_table();
    let line: RecordLine = match serde_json::from_str(text) {
        Ok(line) => line,
        Err(err) => {
            eprintln!("herdr seat record: not a record line: {err}");
            std::process::exit(2);
        }
    };
    let line = match complete(&table, line, now()) {
        Ok(line) => line,
        Err(err) => {
            eprintln!("herdr seat record: {err}");
            std::process::exit(2);
        }
    };
    let dir = dir();
    append_record(&dir.join(RECORD_FILE), &line)?;
    // A finished order's booking ends with it: its real use now shows in the pool's readings.
    let _lock = Lock::take(&dir)?;
    let path = dir.join(LEASES_FILE);
    let mut leases = read_leases(&path);
    if leases.release(&line.order, now()) {
        write_leases(&path, &leases)?;
    }
    println!("{}", serde_json::to_string(&line)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-17T21:51:12Z, when O1's live readings were taken (nirvana_usage's `READ_1017`).
    const NOW: u64 = 1_792_273_872;

    fn at(text: &str) -> u64 {
        nirvana_usage::rfc3339(text).expect("a timestamp")
    }

    fn reading(pool: &str, window: &str, used: f32, resets: u64, read_at: u64) -> Reading {
        Reading {
            pool: pool.into(),
            window: window.into(),
            used: Some(used),
            resets_at: Some(resets),
            resets_text: None,
            read_at,
            source: "test".into(),
        }
    }

    /// The readings of 2026-10-17 21:51Z (plan 12 §15, `herdr usage --refresh`, and agy's answer in
    /// `nirvana_usage`'s `AGY_STREAM_1017`), read 30 s before `NOW`.
    fn recorded() -> Vec<Reading> {
        let read = NOW - 30;
        vec![
            reading("max", "5h", 0.41, at("2026-10-18T00:50:00Z"), read),
            reading("max", "week", 0.37, at("2026-10-23T16:00:00Z"), read),
            reading("max", "week.fable", 0.0, at("2026-10-23T16:00:00Z"), read),
            reading("gemini", "5h", 0.0196, at("2026-10-18T02:35:36Z"), read),
            reading("gemini", "week", 0.0168, at("2026-10-24T15:48:02Z"), read),
            reading("google3p", "5h", 0.0331, at("2026-10-18T02:40:32Z"), read),
            reading("google3p", "week", 0.0402, at("2026-10-19T10:54:24Z"), read),
            reading("argon", "week", 0.0212, at("2026-10-24T16:03:32Z"), read),
        ]
    }

    fn table() -> Table {
        parse(DEFAULT).expect("the default table parses")
    }

    fn world() -> World {
        World {
            now: NOW,
            readings: recorded(),
            ..World::default()
        }
    }

    fn set(world: &mut World, pool: &str, window: &str, used: f32) {
        let reading = world
            .readings
            .iter_mut()
            .find(|r| r.pool == pool && r.window == window)
            .expect("a recorded window");
        reading.used = Some(used);
    }

    fn agy_out(world: &mut World) {
        world.clis.insert("agy".into(), "agy not signed in".into());
    }

    fn request(class: &str) -> Request {
        Request::new(class)
    }

    fn chosen(table: &Table, world: &World, request: &Request) -> Pick {
        pick(table, world, request).unwrap_or_else(|no| panic!("no seat: {no:?}"))
    }

    fn why<'a>(losers: &'a [Loser], seat: &str) -> &'a str {
        losers
            .iter()
            .find(|loser| loser.seat == seat)
            .map(|loser| loser.why.as_str())
            .unwrap_or_else(|| panic!("{seat} not among {losers:?}"))
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nirvana-seat-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn the_default_table_is_written_once_parses_and_holds_the_plans_seats_and_classes() {
        let dir = temp_dir("default");
        let path = dir.join(SEATS_FILE);
        assert_eq!(read_or_create(&path), DEFAULT);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), DEFAULT);
        let table = parse(&read_or_create(&path)).expect("parses");
        assert!(table.problems().is_empty(), "{:?}", table.problems());
        // Plan 12 §4: seven seats in the first version; fable, gpt-oss and local present but off.
        let on: Vec<&str> = table
            .seats
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(
            on,
            ["opus", "sonnet", "haiku", "g-opus", "g-sonnet", "argon", "flash"]
        );
        let off: Vec<&str> = table
            .seats
            .iter()
            .filter(|s| !s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(off, ["fable", "gpt-oss", "local"]);
        // §5.2's nineteen classes, in the nine chains of §5.4.
        let classes: Vec<&str> = table.classes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            classes,
            [
                "plan",
                "build.load_bearing",
                "build.bounded",
                "build.frontend",
                "judge.visual",
                "fix.debug",
                "ops.terminal",
                "review.adversary",
                "acceptance.author",
                "research.read",
                "research.web",
                "read.huge",
                "write.docs",
                "media.video",
                "media.audio",
                "sweep.mechanical",
                "chore.memory",
                "chore.script",
                "chore.github"
            ]
        );
        assert_eq!(
            table.chains(),
            [
                "plan",
                "build.load_bearing",
                "build.bounded",
                "build.visual",
                "fix.debug",
                "review",
                "research",
                "media",
                "chore"
            ]
        );
        // §5.3's hypotheses, and pace's numbers are the foot's.
        let c = &table.constants;
        assert!(close(c.floor, 0.05) && close(c.reserve, 0.15));
        assert!(close(c.pace_ease, nirvana_usage::PACE_EASE));
        assert!(close(c.pace_min, nirvana_usage::PACE_MIN));
        assert!(close(c.pace_max, nirvana_usage::PACE_MAX));
        assert!(close(c.reset_left, 0.30) && c.reset_within_min == 60);
        assert!(close(c.reset_discount, 0.5) && c.defer_min == 30);
        assert_eq!(c.overrides_per_run, 3);
        assert!(!table.use_record, "the blend ships off");
        // Every expected-use cell says whether it was measured.
        for seat in &table.seats {
            for cell in &seat.uses {
                assert!(
                    ["measured:", "derived:", "prior:", "free:"]
                        .iter()
                        .any(|tag| cell.basis.starts_with(tag)),
                    "{} {:?}: {}",
                    seat.id,
                    cell.effort,
                    cell.basis
                );
            }
        }
        // The owner's file is kept, and a bad one is refused rather than overwritten.
        std::fs::write(&path, "version = 2\n").unwrap();
        assert_eq!(read_or_create(&path), "version = 2\n");
        assert!(parse(&DEFAULT.replace("version = 1", "version = 2"))
            .unwrap_err()
            .contains("version 2 is not 1"));
        assert!(parse(&DEFAULT.replace(
            "bars = { efficient = 70, balanced = 85, sota = 100 }\nwhy = \"P4",
            "bars = { efficient = 90, balanced = 85, sota = 100 }\nwhy = \"P4"
        ))
        .unwrap_err()
        .contains("class build.bounded: bars must rise"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bounded_build_at_balanced_goes_to_sonnet_medium_with_its_reason_line() {
        let table = table();
        let got = chosen(&table, &world(), &request("build.bounded"));
        assert_eq!(
            (got.seat.as_str(), got.effort, got.cli.as_str()),
            ("sonnet", Effort::Medium, "claude")
        );
        assert_eq!(got.model, "claude-sonnet-5-5");
        assert_eq!(
            (got.pool.as_str(), got.chain.as_str()),
            ("max", "build.bounded")
        );
        // §5.3 step 5's form.
        assert_eq!(
            got.reason,
            "build.bounded \u{2192} sonnet medium: max 5h 41% used, week 37% used, reserve 15%; \
             g-sonnet scores the same (google3p 5h 3% used, week 4% used)"
        );
        assert!(why(&got.runners_up, "g-sonnet").starts_with("pressure "));
        assert_eq!(
            why(&got.runners_up, "argon"),
            "scores 82.0, under the bar 85 at balanced"
        );
        // Expected use: P4's order (0.088% of max 5 h) x weight 1 x medium size 5.
        let five = &got.windows[0];
        assert_eq!(five.window, "5h");
        assert!(close(five.expected, 0.0044), "{five:?}");
        assert!(close(five.headroom.unwrap(), 0.44), "{five:?}");
        assert!(got
            .use_basis
            .as_deref()
            .unwrap()
            .contains("derived: P4 sonnet medium"));
    }

    #[test]
    fn a_seat_that_is_off_or_whose_cli_is_not_ready_is_skipped_with_why() {
        let table = table();
        // Fable is off: at SOTA it would be the best plan seat (97).
        let mut sota = request("plan");
        sota.mode = Mode::Sota;
        let got = chosen(&table, &world(), &sota);
        assert_eq!(got.seat, "opus");
        assert_eq!(why(&got.runners_up, "fable"), "off in seats.toml");
        // agy signed out: every agy seat says so.
        let mut world = world();
        agy_out(&mut world);
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "sonnet");
        for seat in ["g-sonnet", "argon", "flash"] {
            assert_eq!(why(&got.runners_up, seat), "agy not signed in");
        }
    }

    #[test]
    fn a_need_the_seat_lacks_and_a_class_that_does_not_take_it_skip_it() {
        let table = table();
        let mut web = request("research.web");
        web.needs = vec!["video".into()];
        let got = chosen(&table, &world(), &web);
        assert_eq!(why(&got.runners_up, "sonnet"), "lacks video");
        assert_eq!(why(&got.runners_up, "g-sonnet"), "lacks video");
        assert!(got.seat == "flash" || got.seat == "argon");
        // Argon takes visual work only when it moves.
        let got = chosen(&table, &world(), &request("build.frontend"));
        assert_eq!(got.seat, "opus");
        assert_eq!(
            why(&got.runners_up, "argon"),
            "build.frontend takes it only for video"
        );
        let mut moving = request("build.frontend");
        moving.needs = vec!["video".into()];
        let got = chosen(&table, &world(), &moving);
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::High));
        assert_eq!(why(&got.runners_up, "opus"), "lacks video");
        // A seat the class does not list is never a candidate.
        let class = table.class("plan").unwrap();
        let haiku = table.seat("haiku").unwrap();
        let size = table.size(SizeName::Medium);
        let cand = candidate(
            &table,
            &world(),
            &request("plan"),
            class,
            &[],
            size,
            false,
            2,
            haiku,
        );
        assert_eq!(
            cand.skip.map(|s| s.why).as_deref(),
            Some("plan does not take it")
        );
    }

    #[test]
    fn the_caller_can_exclude_a_family_so_another_family_reviews() {
        let table = table();
        let mut review = request("review.adversary");
        review.not_family = vec!["anthropic".into()];
        let got = chosen(&table, &world(), &review);
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::High));
        assert_eq!(
            why(&got.runners_up, "opus"),
            "family anthropic excluded (another family reviews, decision 0017)"
        );
        review.not_family = vec!["google".into()];
        let got = chosen(&table, &world(), &review);
        assert_eq!((got.seat.as_str(), got.effort), ("opus", Effort::High));
    }

    #[test]
    fn a_window_at_or_under_the_floor_skips_its_seats_booked_use_included() {
        let table = table();
        let mut world = world();
        set(&mut world, "gemini", "5h", 0.96);
        let got = chosen(&table, &world, &request("research.web"));
        assert_eq!(got.seat, "argon");
        assert_eq!(
            why(&got.runners_up, "flash"),
            "gemini 5h 96% used: at or under the floor 5%"
        );
        // 90% used and 6% booked by an order in flight leaves 4%.
        let mut world = self::world();
        set(&mut world, "gemini", "5h", 0.90);
        world
            .leases
            .push(lease("o-1", "flash", "gemini", &[("5h", 0.06)], NOW + 600));
        let got = chosen(&table, &world, &request("research.web"));
        assert_eq!(
            why(&got.runners_up, "flash"),
            "gemini 5h 90% used and 6% booked: at or under the floor 5%"
        );
    }

    #[test]
    fn an_order_a_window_cannot_hold_goes_to_a_pool_that_can() {
        let table = table();
        let mut world = world();
        // 83% of max's 5 h used: 2% left after the reserve, for an order expected at 3.4%.
        set(&mut world, "max", "5h", 0.83);
        let got = chosen(&table, &world, &request("build.load_bearing"));
        assert_eq!(
            why(&got.runners_up, "opus"),
            "max 5h has 2% left after the reserve for an order of 3%; argon on argon can hold it"
        );
        // g-opus's pool is small: 127% of its 5 h window for this order.
        assert!(why(&got.runners_up, "g-opus").ends_with("argon on argon can hold it"));
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::High));
        assert!(got.bar_dropped);
    }

    #[test]
    fn a_max_seat_that_would_break_the_orchestrators_reserve_is_skipped() {
        let table = table();
        let mut world = world();
        agy_out(&mut world);
        set(&mut world, "max", "5h", 0.86);
        let no = pick(&table, &world, &request("build.bounded")).unwrap_err();
        assert_eq!(
            why(&no.skipped, "sonnet"),
            "would leave max 5h at 14%, under the orchestrator's reserve 15%"
        );
    }

    #[test]
    fn the_bar_drops_to_the_best_seat_left_and_never_under_the_efficient_bar() {
        let mut table = table();
        let mut world = world();
        set(&mut world, "max", "5h", 0.86);
        set(&mut world, "google3p", "5h", 0.96);
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::Low));
        assert!(got.bar_dropped && close(got.bar, 82.0));
        assert!(
            got.reason
                .contains("; bar dropped to 82 (none left reached 85)"),
            "{}",
            got.reason
        );
        assert_eq!(
            why(&got.runners_up, "flash"),
            "scores 78.0, under the bar 82 at balanced"
        );
        // With the Efficient bar at 83, argon (82) is under it: no seat.
        let class = table
            .classes
            .iter_mut()
            .find(|c| c.id == "build.bounded")
            .unwrap();
        class.bars = Some(Bars {
            efficient: 83.0,
            balanced: 85.0,
            sota: 100.0,
        });
        let no = pick(&table, &world, &request("build.bounded")).unwrap_err();
        assert_eq!(
            no.reason,
            "no seat left for build.bounded reaches its Efficient bar 83; best left argon at 82.0"
        );
    }

    #[test]
    fn pressure_is_the_worst_window_and_a_weeks_share_is_not_a_5h_windows() {
        let table = table();
        let mut world = world();
        // Gemini's week 90% used: flash's binding window is the week, not the 5 h one.
        set(&mut world, "gemini", "week", 0.90);
        let got = chosen(&table, &world, &request("research.web"));
        assert_eq!(got.seat, "flash");
        let term =
            |w: &WindowState| w.expected / w.headroom.unwrap() * w.pace.unwrap() * w.reset_factor;
        let five = got.windows.iter().find(|w| w.window == "5h").unwrap();
        let week = got.windows.iter().find(|w| w.window == "week").unwrap();
        assert!(term(week) > term(five), "{five:?} {week:?}");
        assert!(
            close(got.pressure, term(week)),
            "{} {}",
            got.pressure,
            term(week)
        );
        // The same order is twice the share of gemini's 5 h window that it is of the week, so at 20%
        // headroom each the 5 h window is the dearer.
        let flash = table.seat("flash").unwrap();
        let cell = flash.use_cell(Effort::Low).unwrap();
        let window = |name: &str| WindowState {
            window: name.into(),
            used: Some(0.8),
            booked: 0.0,
            left: Some(0.2),
            headroom: Some(0.2),
            resets_at: None,
            pace: Some(1.0),
            reset_factor: 1.0,
            expected: cell.windows[name],
        };
        assert!(window("5h").pressure().unwrap() > window("week").pressure().unwrap());
    }

    #[test]
    fn quota_about_to_reset_is_discounted_by_the_share_of_the_order_before_it() {
        let c = table().constants;
        // Half left, 30 min to the reset: a 15 min order runs wholly before it, half price.
        assert!(close(reset_factor(0.5, 1800, 15, &c), 0.5));
        // A 2 h order: a quarter runs before it.
        assert!(close(reset_factor(0.5, 1800, 120, &c), 0.875));
        // 20% left is not "more than 30%"; 61 min is not "under 60".
        assert!(close(reset_factor(0.2, 1800, 15, &c), 1.0));
        assert!(close(reset_factor(0.5, 61 * 60, 15, &c), 1.0));
        // In a got: gemini's 5 h window resets in 20 min, a medium order is 45 min.
        let mut world = world();
        world
            .readings
            .iter_mut()
            .find(|r| r.pool == "gemini" && r.window == "5h")
            .unwrap()
            .resets_at = Some(NOW + 1200);
        let got = chosen(&table(), &world, &request("research.web"));
        let five = got.windows.iter().find(|w| w.window == "5h").unwrap();
        assert!(
            close(five.reset_factor, 1.0 - 0.5 * (1200.0 / 2700.0)),
            "{five:?}"
        );
    }

    #[test]
    fn ties_go_to_the_higher_score_then_to_table_order() {
        let mut table = table();
        for seat in table
            .seats
            .iter_mut()
            .filter(|s| s.id == "flash" || s.id == "argon")
        {
            for cell in &mut seat.uses {
                cell.windows.clear();
            }
        }
        // Both 84, both no expected use: argon is earlier in the table.
        let got = chosen(&table, &world(), &request("research.web"));
        assert_eq!(got.seat, "argon");
        assert_eq!(
            why(&got.runners_up, "flash"),
            "same pressure and score, later in the table"
        );
        let class = table
            .classes
            .iter_mut()
            .find(|c| c.id == "research.web")
            .unwrap();
        class
            .seats
            .iter_mut()
            .find(|e| e.seat == "argon")
            .unwrap()
            .score = 83.0;
        let got = chosen(&table, &world(), &request("research.web"));
        assert_eq!(got.seat, "flash");
        assert_eq!(
            why(&got.runners_up, "argon"),
            "same pressure, scores 83.0 against 84.0"
        );
    }

    #[test]
    fn a_window_not_read_ranks_below_known_headroom_and_above_an_exhausted_one() {
        let table = table();
        let mut world = world();
        world.readings.retain(|r| r.pool != "max");
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "g-sonnet");
        assert_eq!(
            why(&got.runners_up, "sonnet"),
            "5h, week not read: ranks below known headroom"
        );
        // Google's Claude pool exhausted: the unread pool is better than none.
        set(&mut world, "google3p", "5h", 0.97);
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "sonnet");
        assert_eq!(got.unknown, ["5h", "week"]);
        // A reading older than the staleness line is not read.
        let mut world = self::world();
        for r in world.readings.iter_mut().filter(|r| r.pool == "max") {
            r.read_at = NOW - nirvana_usage::STALE_AFTER - 100;
        }
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "g-sonnet"
        );
        // An unread pool never counts as one that can hold the order.
        let mut table = table;
        for seat in table
            .seats
            .iter_mut()
            .filter(|s| s.id == "flash" || s.id == "argon")
        {
            seat.enabled = false;
        }
        let mut world = self::world();
        world.readings.retain(|r| r.pool != "max");
        set(&mut world, "google3p", "5h", 0.90);
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "g-sonnet"
        );
    }

    fn lease(order: &str, seat: &str, pool: &str, uses: &[(&str, f32)], expires_at: u64) -> Lease {
        Lease {
            order: order.into(),
            seat: seat.into(),
            effort: Effort::Medium,
            pool: pool.into(),
            class: "build.bounded".into(),
            run: None,
            expected: uses.iter().map(|(w, u)| ((*w).to_owned(), *u)).collect(),
            booked_at: NOW - 60,
            expires_at,
            pick: serde_json::Value::Null,
        }
    }

    #[test]
    fn live_leases_take_headroom_expired_ones_and_the_orders_own_do_not() {
        let table = table();
        let mut world = world();
        agy_out(&mut world);
        set(&mut world, "max", "5h", 0.80);
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "sonnet"
        );
        // Another order in flight books 6% of the 5 h window: 14% left, under the reserve.
        world
            .leases
            .push(lease("o-other", "opus", "max", &[("5h", 0.06)], NOW + 600));
        let no = pick(&table, &world, &request("build.bounded")).unwrap_err();
        assert!(
            why(&no.skipped, "sonnet").starts_with("would leave max 5h at 14%"),
            "{no:?}"
        );
        // The same order seated again does not count its own lease.
        let mut again = request("build.bounded");
        again.order = Some("o-other".into());
        assert_eq!(chosen(&table, &world, &again).seat, "sonnet");
        // An expired lease holds nothing.
        world.leases[0].expires_at = NOW - 1;
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "sonnet"
        );
    }

    #[test]
    fn a_booking_is_idempotent_per_order_and_released_by_id() {
        let table = table();
        let got = chosen(&table, &world(), &request("build.bounded"));
        let first = lease_for(&table, &got, "o-1", Some("r-1"), NOW);
        assert_eq!(first.expires_at, NOW + 180 * 60, "a medium order's lease");
        assert!(close(first.expected["5h"], got.windows[0].expected));
        let mut leases = Leases::default();
        assert_eq!(leases.book(first.clone(), NOW), first);
        let mut second = first.clone();
        second.seat = "g-sonnet".into();
        assert_eq!(
            leases.book(second, NOW + 5),
            first,
            "the first booking stands"
        );
        assert_eq!(leases.leases.len(), 1);
        // On disk, written whole.
        let dir = temp_dir("leases");
        let path = dir.join(LEASES_FILE);
        write_leases(&path, &leases).unwrap();
        assert_eq!(read_leases(&path).leases, leases.leases);
        assert!(leases.release("o-1", NOW + 10));
        assert!(!leases.release("o-1", NOW + 10));
        assert!(leases.leases.is_empty());
        // An expired booking is not live, and a new one for the same order replaces it.
        let mut old = first.clone();
        old.expires_at = NOW - 1;
        let mut leases = Leases {
            leases: vec![old],
            ..Leases::default()
        };
        assert!(leases.live("o-1", NOW).is_none());
        assert_eq!(leases.book(first.clone(), NOW), first);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_lease_lock_is_exclusive_and_a_stale_one_is_taken_over() {
        let dir = temp_dir("lock");
        let held = Lock::try_take(&dir, 2).expect("free");
        assert!(dir.join(LOCK_FILE).is_file());
        assert!(Lock::try_take(&dir, 2).is_err(), "held by the first");
        drop(held);
        assert!(!dir.join(LOCK_FILE).exists());
        // A lock its process left behind a minute ago.
        std::fs::write(dir.join(LOCK_FILE), "1").unwrap();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(dir.join(LOCK_FILE))
            .unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
        drop(file);
        assert!(Lock::try_take(&dir, 2).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_override_passes_every_filter_and_three_are_taken_per_run() {
        let table = table();
        let mut over = request("build.bounded");
        over.override_seat = Some(("argon".into(), "wants a Gemini build".into()));
        let got = chosen(&table, &world(), &over);
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::Low));
        assert_eq!(got.rule_pick.as_deref(), Some("sonnet medium"));
        assert!(
            got.reason
                .ends_with("; override: wants a Gemini build; the rule's pick: sonnet medium"),
            "{}",
            got.reason
        );
        let mut world = world();
        world.overrides_used = 3;
        assert_eq!(
            pick_err(&table, &world, &over),
            "override argon refused: 3 of 3 overrides already taken in this run; more is the owner's call"
        );
        // An override cannot dodge a filter: the class, a need, the reserve.
        over.override_seat = Some(("opus".into(), "trickier".into()));
        assert_eq!(
            pick_err(&table, &self::world(), &over),
            "override opus refused: build.bounded does not take it"
        );
        let mut video = over.clone();
        video.needs = vec!["video".into()];
        video.override_seat = Some(("g-sonnet".into(), "x".into()));
        assert_eq!(
            pick_err(&table, &self::world(), &video),
            "override g-sonnet refused: lacks video"
        );
        let mut world = self::world();
        agy_out(&mut world);
        set(&mut world, "max", "5h", 0.86);
        over.override_seat = Some(("sonnet".into(), "x".into()));
        assert_eq!(
            pick_err(&table, &world, &over),
            "override sonnet refused: would leave max 5h at 14%, under the orchestrator's reserve 15%"
        );
    }

    fn pick_err(table: &Table, world: &World, request: &Request) -> String {
        pick(table, world, request).unwrap_err().reason
    }

    #[test]
    fn effort_follows_the_class_sota_lifts_plans_and_max_is_never_chosen() {
        let table = table();
        let world = world();
        let effort = |class: &str, mode: Mode| {
            let mut r = request(class);
            r.mode = mode;
            let p = chosen(&table, &world, &r);
            (p.seat, p.effort)
        };
        assert_eq!(
            effort("build.bounded", Mode::Balanced),
            ("sonnet".into(), Effort::Medium)
        );
        assert_eq!(
            effort("build.bounded", Mode::Sota),
            ("sonnet".into(), Effort::Medium)
        );
        assert_eq!(
            effort("fix.debug", Mode::Balanced),
            ("opus".into(), Effort::Medium)
        );
        assert_eq!(
            effort("plan", Mode::Balanced),
            ("opus".into(), Effort::High)
        );
        assert_eq!(effort("plan", Mode::Sota), ("opus".into(), Effort::Xhigh));
        // g-opus offers no xhigh: high, said so.
        let mut world = self::world();
        world
            .clis
            .insert("claude".into(), "claude not signed in".into());
        let mut sota = request("plan");
        sota.mode = Mode::Sota;
        let got = chosen(&table, &world, &sota);
        assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));
        assert_eq!(
            got.effort_note.as_deref(),
            Some("g-opus has no xhigh; high")
        );
        // A class asking max gets xhigh, even from a seat that lists max.
        let mut table = table;
        table
            .classes
            .iter_mut()
            .find(|c| c.id == "plan")
            .unwrap()
            .effort = Effort::Max;
        table
            .seats
            .iter_mut()
            .find(|s| s.id == "opus")
            .unwrap()
            .efforts
            .push(Effort::Max);
        let got = chosen(&table, &self::world(), &request("plan"));
        assert_eq!((got.seat.as_str(), got.effort), ("opus", Effort::Xhigh));
        assert_eq!(
            got.effort_note.as_deref(),
            Some("max is never chosen automatically")
        );
    }

    #[test]
    fn under_pressure_with_nothing_left_one_effort_step_down() {
        let table = table();
        let mut world = world();
        agy_out(&mut world);
        // 16.8% left: opus high (2.2%) breaks the 15% reserve, opus medium (1.4%) does not.
        set(&mut world, "max", "5h", 0.832);
        let got = chosen(&table, &world, &request("plan"));
        assert_eq!((got.seat.as_str(), got.effort), ("opus", Effort::Medium));
        assert!(got.step_down);
        assert!(
            got.reason
                .contains("; one effort step down (the class's effort found no seat)"),
            "{}",
            got.reason
        );
    }

    #[test]
    fn no_seat_names_every_skip_and_the_earliest_reset() {
        let table = table();
        let mut world = world();
        set(&mut world, "max", "5h", 0.86);
        set(&mut world, "google3p", "5h", 0.97);
        set(&mut world, "argon", "week", 0.97);
        set(&mut world, "gemini", "5h", 0.97);
        let no = pick(&table, &world, &request("build.bounded")).unwrap_err();
        assert!(
            no.reason.starts_with(
                "no seat left for build.bounded: sonnet (would leave max 5h at 14%, under the orchestrator's reserve 15%); \
                 g-sonnet (google3p 5h 97% used: at or under the floor 5%)"
            ),
            "{}",
            no.reason
        );
        assert_eq!(no.skipped.len(), 4);
        // Max's 5 h window resets first, about three hours on: no deferring.
        assert_eq!(no.earliest_reset, Some(at("2026-10-18T00:50:00Z")));
        assert!(!no.defer);
        world
            .readings
            .iter_mut()
            .find(|r| r.pool == "max" && r.window == "5h")
            .unwrap()
            .resets_at = Some(NOW + 20 * 60);
        let no = pick(&table, &world, &request("build.bounded")).unwrap_err();
        assert_eq!(no.earliest_reset, Some(NOW + 20 * 60));
        assert!(no.defer, "within 30 minutes");
    }

    /// Plan 12 L9's verifier: "`seat pick --needs video` never returns a Claude seat".
    #[test]
    fn an_order_that_needs_video_never_gets_a_claude_seat() {
        let table = table();
        let mut seated = 0;
        for world in [world(), {
            let mut w = world();
            set(&mut w, "argon", "week", 0.97);
            w
        }] {
            for class in &table.classes {
                for mode in [Mode::Efficient, Mode::Balanced, Mode::Sota] {
                    for size in [SizeName::Small, SizeName::Medium, SizeName::Large] {
                        let mut r = request(&class.id);
                        r.needs = vec!["video".into()];
                        r.mode = mode;
                        r.size = size;
                        if let Ok(p) = pick(&table, &world, &r) {
                            assert_ne!(
                                p.family, "anthropic",
                                "{} {mode:?}: {}",
                                class.id, p.reason
                            );
                            seated += 1;
                        }
                        for seat in ["opus", "sonnet", "haiku", "g-opus", "g-sonnet"] {
                            r.override_seat = Some((seat.into(), "x".into()));
                            assert!(pick(&table, &world, &r).is_err(), "{seat} for {}", class.id);
                        }
                    }
                }
            }
        }
        assert!(seated > 0, "media.video seats argon");
        let mut video = request("media.video");
        video.needs = vec!["video".into()];
        assert_eq!(chosen(&table, &world(), &video).seat, "argon");
    }

    #[test]
    fn the_record_blend_is_decision_0029s_and_ships_off() {
        // Decision 0029's own table: 9 of 12 is 0.956, 0 of 3 and 1 of 5 are 0.625, 0 of 1 is 0.833.
        assert!((multiplier(9, 12, 0.8, 5.0) - 0.956).abs() < 0.001);
        assert!(close(multiplier(0, 3, 0.8, 5.0), 0.625));
        assert!(close(multiplier(1, 5, 0.8, 5.0), 0.625));
        assert!((multiplier(0, 1, 0.8, 5.0) - 0.833).abs() < 0.001);
        // No record, or one at or above the prior, changes nothing.
        assert!(close(multiplier(0, 0, 0.8, 5.0), 1.0));
        assert!(close(multiplier(4, 4, 0.8, 5.0), 1.0));
        let mut table = table();
        let mut world = world();
        world.record.insert(
            ("sonnet".into(), Effort::Medium, "build.bounded".into()),
            Tally {
                passes: 0,
                orders: 3,
            },
        );
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "sonnet"
        );
        table.use_record = true;
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "g-sonnet");
        assert_eq!(
            why(&got.runners_up, "sonnet"),
            "scores 55.0, under the bar 85 at balanced"
        );
    }

    #[test]
    fn a_finished_order_is_recorded_with_its_use_from_its_own_tokens() {
        let table = table();
        let line: RecordLine = serde_json::from_str(
            r#"{"order":"o-1","run":"r-1","seat":"g-sonnet","effort":"medium","class":"build.bounded","outcome":"pass","tokens":{"cache_read":45863,"output":1287}}"#,
        )
        .unwrap();
        let line = complete(&table, line, NOW).unwrap();
        assert_eq!(line.chain.as_deref(), Some("build.bounded"));
        assert_eq!(line.model.as_deref(), Some("claude-sonnet-5-5-medium"));
        assert_eq!(line.at, Some(NOW));
        // P4's own order: the factor was fit on it, so its tokens give back about 3.31% of the 5 h window.
        let spent = line.spent.clone().unwrap();
        assert!((spent["5h"] - 0.0331).abs() < 0.0005, "{spent:?}");
        assert!((spent["week"] - 0.0166).abs() < 0.0005, "{spent:?}");
        let dir = temp_dir("record");
        let path = dir.join(RECORD_FILE);
        append_record(&path, &line).unwrap();
        let mut fail = line.clone();
        fail.outcome = "sent back".into();
        append_record(&path, &fail).unwrap();
        let lines = read_record(&path);
        assert_eq!(lines, vec![line, fail]);
        let tallies = tally(&lines);
        assert_eq!(
            tallies[&("g-sonnet".into(), Effort::Medium, "build.bounded".into())],
            Tally {
                passes: 1,
                orders: 2
            }
        );
        assert!(complete(
            &table,
            serde_json::from_str(
                r#"{"order":"o","seat":"nope","effort":"low","class":"plan","outcome":"pass"}"#
            )
            .unwrap(),
            NOW
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn argons_price_doubles_on_2026_11_30_and_its_expected_use_with_it() {
        let table = table();
        let argon = table.seat("argon").unwrap();
        assert!(close(
            argon.price_at(date("2026-11-29").unwrap()).unwrap().input as f32,
            2.0
        ));
        assert!(close(
            argon.price_at(date("2026-11-30").unwrap()).unwrap().input as f32,
            4.0
        ));
        let tokens = Tokens {
            cache_read: 55_053,
            output: 1_180,
            ..Tokens::default()
        };
        let nov = computed_use(&table, argon, &tokens, date("2026-11-01").unwrap()).unwrap();
        let dec = computed_use(&table, argon, &tokens, date("2026-12-01").unwrap()).unwrap();
        assert!(close(dec["week"], 2.0 * nov["week"]), "{nov:?} {dec:?}");
        // A got in December expects twice the share the October cell holds.
        let december = date("2026-12-01").unwrap() + 43_200;
        let world = World {
            now: december,
            readings: vec![reading(
                "argon",
                "week",
                0.1,
                december + 5 * 86_400,
                december - 30,
            )],
            ..World::default()
        };
        let mut video = request("media.video");
        video.needs = vec!["video".into()];
        let got = chosen(&table, &world, &video);
        assert_eq!(got.seat, "argon");
        // 1.01% x 2 (price) x 2 (weight) x 5 (medium).
        assert!(
            close(got.windows[0].expected, 0.0101 * 2.0 * 2.0 * 5.0),
            "{:?}",
            got.windows
        );
    }

    #[test]
    fn seat_list_shows_each_seat_its_headroom_and_why_not() {
        let table = table();
        let mut world = world();
        agy_out(&mut world);
        world
            .leases
            .push(lease("o-1", "sonnet", "max", &[("5h", 0.02)], NOW + 600));
        let view = list_view(&table, &world);
        let seat = |id: &str| {
            view["seats"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["seat"] == id)
                .unwrap()
                .clone()
        };
        assert_eq!(seat("fable")["why_not"], "off in seats.toml");
        assert_eq!(seat("g-opus")["why_not"], "agy not signed in");
        assert!(seat("opus")["why_not"].is_null());
        let five = &seat("opus")["windows"][0];
        assert!(
            (five["headroom"].as_f64().unwrap() - 0.42).abs() < 1e-4,
            "{five}"
        );
        assert_eq!(view["leases"].as_array().unwrap().len(), 1);
        let text = list_text(&table, &world, Path::new("home"));
        assert!(text.contains("5h 57% (42% after reserve)"), "{text}");
        assert!(text.contains("o-1 sonnet medium on max"), "{text}");
    }
}
