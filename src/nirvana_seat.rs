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
//!    family (decision 0017, a preference: when no other family has a seat and no override was given, the
//!    class is seated again on its own family, in a fresh session, and the pick says so); a window at or
//!    under the floor; a window whose headroom is below the order's expected use while another pool could
//!    hold the whole order (Argon's review, §4.4); a `max` seat that would break the orchestrator's reserve.
//! 2. **The bar** at the mode (decision 0018): if no seat clears it, the bar drops to the best seat left, never
//!    under the class's Efficient bar (Argon's review, F4).
//! 3. **Pressure**, the least wins: `max over windows (expected_use / headroom × pace × reset)`, a week's 20%
//!    holding more than a 5 h window's 20% (§4.1). Before pressure, three tiers (§3): known headroom that
//!    holds the order; then a seat with a window not read, or a pool with no windows at all; then a known
//!    window too small for the order, which is nearer exhausted than free. Ties go to the higher score, then
//!    to the class's own seat order, which is §5.4's row.
//! 4. **Effort** (§5.5) follows the class and its blast radius; SOTA lifts to the class's `sota_effort`, and the
//!    rule never chooses above high (plan 19's ruling "no automatic effort above high"): `xhigh` comes only from an
//!    override or a parent's effort, and max never. Under pressure the same model on another pool comes first
//!    (equal scores), then the other family at full effort (the bar drop), and only when nothing is left one effort
//!    step down.
//! 5. **A seat with its own window fails closed** (plan 40a, Fable 5.2): every window it spends must have a fresh
//!    reading and its own window must keep `fable_reserve` after the order, in every mode and under `--override`.
//!    Claude Code's `-p` "never asks for consent. When a Fable request there would bill to usage credits, Claude
//!    Code bills it without asking" (code.claude.com model-config, read 2026-11-09).
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
/// The defaults `seats.toml` has shipped with in past releases (from 0.13.0 on).
pub const HISTORICAL_DEFAULTS: &[&str] = &[
    include_str!("nirvana_seats_v1.toml"),
    include_str!("nirvana_seats_v2.toml"),
    include_str!("nirvana_seats_v3.toml"),
    include_str!("nirvana_seats_v4.toml"),
    include_str!("nirvana_seats_v5.toml"),
    include_str!("nirvana_seats_v6.toml"),
    // v7 is plan 34a's (0.46.0), which plan 35c's `why` line left without a copy here: an install holding it never
    // upgraded (the owner's did not, read 2026-11-02). v8 is plan 35c's, before plan 35 R2 added `browser` to agy.
    include_str!("nirvana_seats_v7.toml"),
    include_str!("nirvana_seats_v8.toml"),
    // Three defaults that shipped in a commit and had no copy here (review-trim review nit), each byte for byte from
    // git: v9 plan 33b's (ad601b8d), v10 plan 33's join (c2ad3b39), v11 plan 34a's first commit (b5ab56a3).
    include_str!("nirvana_seats_v9.toml"),
    include_str!("nirvana_seats_v10.toml"),
    include_str!("nirvana_seats_v11.toml"),
    // Plan 40a: the default before Fable 5.2 took its seat (86b37c7e), copied before the edit so an installed copy
    // upgrades.
    include_str!("nirvana_seats_v12.toml"),
    // Plan 70: the default before Gemini 4 Carbon took Argon's seat (d5834ff5), copied before the edit so an
    // installed copy upgrades.
    include_str!("nirvana_seats_v13.toml"),
    // Plan 71: the default before Gemini 4 Hydrogen took Flash 3.8's seat (20b42c2c), copied before the edit so an
    // installed copy upgrades.
    include_str!("nirvana_seats_v14.toml"),
];
pub const DIR: &str = "nirvana";
pub const SEATS_FILE: &str = "seats.toml";
pub const LEASES_FILE: &str = "leases.json";
pub const RECORD_FILE: &str = "record.jsonl";
const LOCK_FILE: &str = "leases.lock";
/// `herdr seat pick`'s exit code when no seat is given, a refused override included.
pub const NO_SEAT_EXIT: i32 = 3;
/// `herdr seat pick` and `release` exit with this when another herdr held the lease lock for all of `Lock::take`.
pub const LOCK_EXIT: i32 = 4;
/// Scores, bars and shares compare with this slack: a blended score is a product of f32s. Decision 0018 §2.2
/// keeps an EDGE of 1e-9 for its f64 composites; f32 needs a coarser one, so this is not that constant.
const EDGE: f32 = 1e-4;
/// A run whose last override is older than this is dropped from `leases.json`: a run does not outlive a week's
/// window, and the file would otherwise grow by one entry per run forever.
pub const RUN_KEEP_S: u64 = 8 * 86_400;
/// How long a pick waits for one CLI's sign-in to be read (`claude auth status`), so a hang there cannot hang
/// `seat pick`.
const ACCOUNTS_TIMEOUT: Duration = Duration::from_secs(15);

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

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" => Some(Self::Xhigh),
            "max" => Some(Self::Max),
            _ => None,
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

    /// Plan 12 O8a: the word the Seats square's header shows (§8: "Efficient, Balanced, SOTA").
    pub fn title(self) -> &'static str {
        match self {
            Self::Efficient => "Efficient",
            Self::Balanced => "Balanced",
            Self::Sota => "SOTA",
        }
    }

    /// Plan 12 O8a: the mode the Seats square's one key moves to, round the three stops.
    pub fn next(self) -> Self {
        match self {
            Self::Efficient => Self::Balanced,
            Self::Balanced => Self::Sota,
            Self::Sota => Self::Efficient,
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
    /// Plan 40a (review M5): Nirvana Claude's own seat by mode. A table without the section seats it as before.
    #[serde(default)]
    pub orchestrator: OrchestratorTable,
}

/// One seat and effort for Nirvana Claude.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorChoice {
    pub seat: String,
    pub effort: Effort,
}

impl OrchestratorChoice {
    fn new(seat: &str, effort: Effort) -> Self {
        Self {
            seat: seat.to_owned(),
            effort,
        }
    }
}

/// Plan 40a: `[orchestrator]`. Each field defaults to the seat `orchestrator_seat` hard-coded before it, so an owner's
/// edited table without the section (`maybe_upgrade` never touches an edited file) behaves exactly as before.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct OrchestratorTable {
    pub efficient: OrchestratorChoice,
    /// Balanced while the climb is under t = 0.5.
    pub balanced: OrchestratorChoice,
    /// Balanced at t >= 0.5.
    pub balanced_hot: OrchestratorChoice,
    pub sota: OrchestratorChoice,
    /// SOTA when `sota` cannot be taken: off, or a seat with its own window not read or under
    /// `fable_orchestrator_left`.
    pub sota_fallback: OrchestratorChoice,
    /// Balanced with the pool's readings missing or stale.
    pub unread: OrchestratorChoice,
}

impl Default for OrchestratorTable {
    fn default() -> Self {
        Self {
            efficient: OrchestratorChoice::new("sonnet", Effort::Medium),
            balanced: OrchestratorChoice::new("opus", Effort::Medium),
            balanced_hot: OrchestratorChoice::new("sonnet", Effort::Medium),
            sota: OrchestratorChoice::new("opus", Effort::High),
            sota_fallback: OrchestratorChoice::new("opus", Effort::High),
            unread: OrchestratorChoice::new("opus", Effort::Medium),
        }
    }
}

impl OrchestratorTable {
    fn named(&self) -> [(&'static str, &OrchestratorChoice); 6] {
        [
            ("efficient", &self.efficient),
            ("balanced", &self.balanced),
            ("balanced_hot", &self.balanced_hot),
            ("sota", &self.sota),
            ("sota_fallback", &self.sota_fallback),
            ("unread", &self.unread),
        ]
    }
}

/// Plan 40a's `fable_reserve` when a table written before it leaves the line out.
fn default_fable_reserve() -> f32 {
    0.10
}

/// Plan 40a's `fable_orchestrator_left` when a table written before it leaves the line out.
fn default_fable_orchestrator_left() -> f32 {
    0.25
}

/// Plan 40a's `subagent_default` when a table written before it leaves the line out.
fn default_subagent_default() -> String {
    "sonnet".to_owned()
}

/// Plan 40a's `autocompact_tokens` when a table written before it leaves the line out: 0, no flag. The review round
/// measured that Claude Code already compacts far below a 400k cap (the toml's comment), so the flag changed nothing.
fn default_autocompact_tokens() -> u64 {
    0
}

/// Plan 14 §2.2's `spare_weight` when a table written before it leaves the line out.
fn default_spare_weight() -> f32 {
    0.05
}

/// Plan 19 §6.2's `climb_start` when a table written before it leaves the line out.
fn default_climb_start() -> f32 {
    0.70
}

/// Plan 19 §6.2's `climb_full` when a table written before it leaves the line out.
fn default_climb_full() -> f32 {
    1.00
}

/// Plan 25 §2's `fresh_above` when a table written before it leaves the line out.
fn default_fresh_above() -> u64 {
    crate::nirvana_fresh::FRESH_ABOVE
}

/// Plan 33 §3.3's `lesson_reviews_per_day` when a table written before it leaves the line out.
fn default_lesson_reviews_per_day() -> u32 {
    crate::nirvana_review::PER_DAY
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
    /// Plan 14. Optional, so a seats.toml written before 0.22.0 still parses: the upgrade to 0.22.0 refused the
    /// owner's unedited table ("TOML parse error at line 17") and every `seat pick` failed until it was deleted.
    #[serde(default = "default_spare_weight")]
    pub spare_weight: f32,
    pub reset_left: f32,
    pub reset_within_min: u64,
    pub reset_discount: f32,
    pub defer_min: u64,
    pub overrides_per_run: u32,
    pub record_prior: f32,
    pub record_shrink: f32,
    /// Plan 19 §6.2: Balanced mode climb thresholds.
    #[serde(default = "default_climb_start")]
    pub climb_start: f32,
    #[serde(default = "default_climb_full")]
    pub climb_full: f32,
    /// Plan 25 §2: a fresh session after a run ends only above this context (`nirvana_fresh::trigger`).
    #[serde(default = "default_fresh_above")]
    pub fresh_above: u64,
    /// Plan 33 §3.3: lesson reviews a project may queue per UTC day; 0 turns them off.
    #[serde(default = "default_lesson_reviews_per_day")]
    pub lesson_reviews_per_day: u32,
    /// Plan 40a: a seat with its own window is skipped when the order would leave less than this of it.
    #[serde(default = "default_fable_reserve")]
    pub fable_reserve: f32,
    /// Plan 40a: SOTA seats Nirvana Claude on a seat with its own window only while that window has this left.
    #[serde(default = "default_fable_orchestrator_left")]
    pub fable_orchestrator_left: f32,
    /// Plan 40a: `CLAUDE_CODE_SUBAGENT_MODEL` for Nirvana Claude; empty sets nothing.
    #[serde(default = "default_subagent_default")]
    pub subagent_default: String,
    /// Plan 40a: Nirvana Claude's `--autocompact <tokens>`; 0 passes no flag.
    #[serde(default = "default_autocompact_tokens")]
    pub autocompact_tokens: u64,
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
    /// Plan 19 §6.4: Fallback seats rank after every survivor without fallback in all modes.
    #[serde(default)]
    pub fallback: bool,
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
    /// Plan 19 §5: Effort for Efficient mode.
    #[serde(default)]
    pub efficient_effort: Option<Effort>,
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
            ("spare_weight", c.spare_weight),
            ("reset_left", c.reset_left),
            ("reset_discount", c.reset_discount),
            ("record_prior", c.record_prior),
            ("climb_start", c.climb_start),
            ("climb_full", c.climb_full),
            ("fable_reserve", c.fable_reserve),
            ("fable_orchestrator_left", c.fable_orchestrator_left),
        ] {
            if !(0.0..=1.0).contains(&value) {
                problems.push(format!("constants.{name} {value} is not between 0 and 1"));
            }
        }
        // Claude Code takes "auto, or 100k-1M tokens" (`claude --help`, 2.1.321); 0 is herdr's "pass no flag".
        if c.autocompact_tokens != 0 && !(100_000..=1_000_000).contains(&c.autocompact_tokens) {
            problems.push(format!(
                "constants.autocompact_tokens {} is not 0 or between 100000 and 1000000",
                c.autocompact_tokens
            ));
        }
        for (name, choice) in self.orchestrator.named() {
            match self.seat(&choice.seat) {
                None => problems.push(format!("orchestrator.{name}: no seat {}", choice.seat)),
                Some(seat) if seat.cli != "claude" => problems.push(format!(
                    "orchestrator.{name}: {} is not a claude seat; Nirvana Claude is Claude Code",
                    choice.seat
                )),
                Some(_) => {}
            }
        }
        if c.climb_start >= c.climb_full {
            problems.push("constants: climb_start must be less than climb_full".into());
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
            for (at, entry) in class.seats.iter().enumerate() {
                if self.seat(&entry.seat).is_none() {
                    problems.push(format!("class {}: no seat {}", class.id, entry.seat));
                }
                // The class's order breaks the last tie, so a seat listed twice would have two places in it.
                if class.seats[..at].iter().any(|seen| seen.seat == entry.seat) {
                    problems.push(format!(
                        "class {}: seat {} is listed twice",
                        class.id, entry.seat
                    ));
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

    /// The use cell for an effort, else the nearest above it (dearer, so a guess errs safe), else below. Public so
    /// the spawn guard prices a subagent the way the rule does (review D6: `nirvana_claude::expected_share` wrote this
    /// lookup a second time).
    pub fn use_cell(&self, effort: Effort) -> Option<&Use> {
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
/// An installed file that equals an old default upgrades itself after being copied to `<home>/nirvana/backup/`.
pub fn read_or_create(path: &Path) -> String {
    let _ = maybe_upgrade(path);
    if let Ok(text) = std::fs::read_to_string(path) {
        return text;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, DEFAULT);
    DEFAULT.to_owned()
}

fn extract_mode_line(text: &str) -> Option<&str> {
    let mut in_table = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_table = true;
        }
        if !in_table && trimmed.starts_with("mode") {
            let rest = trimmed["mode".len()..].trim_start();
            if rest.starts_with('=') {
                return Some(line);
            }
        }
    }
    None
}

fn replace_mode_line(text: &str, new_mode_line: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut in_table = false;
    let mut replaced = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            in_table = true;
        }
        if !in_table && !replaced && trimmed.starts_with("mode") {
            let rest = trimmed["mode".len()..].trim_start();
            if rest.starts_with('=') {
                out.push_str(new_mode_line);
                out.push('\n');
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn maybe_upgrade(path: &Path) -> Option<()> {
    let text = std::fs::read_to_string(path).ok()?;
    let normalized = text.replace("\r\n", "\n");
    let mode_line = extract_mode_line(&normalized)?;
    let canonical = replace_mode_line(&normalized, "mode = \"balanced\"");
    let current_canonical = DEFAULT.replace("\r\n", "\n");
    if canonical == current_canonical {
        return None;
    }
    let is_historical = HISTORICAL_DEFAULTS
        .iter()
        .any(|hist| canonical == hist.replace("\r\n", "\n"));
    if !is_historical {
        return None;
    }
    let dir = path.parent()?;
    let backup_dir = dir.join("backup");
    let _ = std::fs::create_dir_all(&backup_dir);
    let _ = std::fs::copy(path, backup_dir.join(SEATS_FILE));

    let upgraded = replace_mode_line(&current_canonical, mode_line);
    let _ = write_atomic(path, upgraded.as_bytes());
    Some(())
}

/// The text as `maybe_upgrade` compares it: LF line ends and the top-level mode line fixed.
fn canonical(text: &str) -> String {
    replace_mode_line(&text.replace("\r\n", "\n"), "mode = \"balanced\"")
}

/// Review M2: the owner's file is neither herdr's default nor an old default the next read upgrades, so a changed
/// default never reaches it (`maybe_upgrade` leaves an edited file alone, and should). `seat list` and the Seats square
/// say so in one line, and `herdr seat diff` shows the difference.
pub fn differs_from_default(text: &str) -> bool {
    let text = canonical(text);
    text != canonical(DEFAULT) && !HISTORICAL_DEFAULTS.iter().any(|old| text == canonical(old))
}

/// The line `seat list` and the Seats square show when `differs_from_default`.
pub const DIFFERS_LINE: &str =
    "seats.toml differs from herdr's default. herdr seat diff shows how.";

/// `herdr seat diff`: a unified difference from the owner's `seats.toml` to herdr's default with the owner's mode line,
/// three lines of context, so the mode they chose never shows as a change.
pub fn diff_to_default(installed: &str, label: &str) -> String {
    let installed = installed.replace("\r\n", "\n");
    let default = match extract_mode_line(&installed) {
        Some(mode) => replace_mode_line(&DEFAULT.replace("\r\n", "\n"), mode),
        None => DEFAULT.replace("\r\n", "\n"),
    };
    let body = unified(&installed, &default, 3);
    if body.is_empty() {
        return "seats.toml is herdr's default.\n".to_owned();
    }
    format!("--- {label}\n+++ herdr's default\n{body}")
}

/// A line diff by longest common subsequence: `seats.toml` is under a thousand lines, so the table is small.
fn unified(old: &str, new: &str, context: usize) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if a[i] == b[j] {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    // The edit script: (' ', '-', '+'), each with its line and its numbers in old and new.
    let mut ops: Vec<(char, &str, usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            ops.push((' ', a[i], i, j));
            i += 1;
            j += 1;
        } else if i < n && (j == m || lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
            // A removal before an addition, as `diff -u` prints a changed line.
            ops.push(('-', a[i], i, j));
            i += 1;
        } else {
            ops.push(('+', b[j], i, j));
            j += 1;
        }
    }
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k].0 != ' ').collect();
    let mut out = String::new();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(context);
        let mut end = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= end + 2 * context + 1 {
            k += 1;
            end = changed[k];
        }
        let end = (end + context + 1).min(ops.len());
        let hunk = &ops[start..end];
        let old_len = hunk.iter().filter(|op| op.0 != '+').count();
        let new_len = hunk.iter().filter(|op| op.0 != '-').count();
        out.push_str(&format!(
            "@@ -{},{old_len} +{},{new_len} @@\n",
            hunk[0].2 + 1,
            hunk[0].3 + 1
        ));
        for (kind, line, _, _) in hunk {
            out.push(*kind);
            out.push_str(line);
            out.push('\n');
        }
        k += 1;
    }
    out
}

/// Plan 12 O8a: the table's text with its top-level `mode` set, every other byte as it was. The Seats square's
/// key writes the mode here, where `pick` reads its default (§8), and not in a file of its own: two places for
/// one setting would let a hand edit of `mode =` silently lose to the other. The owner's comments and order
/// stay; the result must parse to the new mode, or nothing is written.
pub fn with_mode(text: &str, mode: Mode) -> Result<String, String> {
    let mut out = String::with_capacity(text.len() + 8);
    let mut done = false;
    let mut in_table = false;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let trimmed = body.trim_start();
        in_table |= trimmed.starts_with('[');
        let is_mode = !in_table
            && trimmed
                .strip_prefix("mode")
                .is_some_and(|rest| rest.trim_start().starts_with('='));
        if is_mode && !done {
            let indent = &body[..body.len() - trimmed.len()];
            out.push_str(&format!("{indent}mode = \"{}\"", mode.word()));
            out.push_str(&line[body.len()..]);
            done = true;
        } else {
            out.push_str(line);
        }
    }
    if !done {
        return Err("seats.toml has no top-level `mode =` line".into());
    }
    match parse(&out) {
        Ok(table) if table.mode == mode => Ok(out),
        Ok(table) => Err(format!(
            "seats.toml still reads mode {} after the edit",
            table.mode.word()
        )),
        Err(problems) => Err(problems),
    }
}

/// Plan 12 O8a: sets the mode of `seats.toml` in `dir` (herdr's is `dir()`); the mode it had. A table that is
/// refused is not touched: the owner's own edit is never overwritten (O2's rule for this file).
pub fn set_mode_in(dir: &Path, mode: Mode) -> Result<Mode, String> {
    let path = dir.join(SEATS_FILE);
    let text = read_or_create(&path);
    let previous = parse(&text)
        .map_err(|problems| format!("{} is refused, fix it first:\n{problems}", path.display()))?
        .mode;
    if previous != mode {
        write_atomic(&path, with_mode(&text, mode)?.as_bytes())
            .map_err(|err| format!("{}: {err}", path.display()))?;
    }
    Ok(previous)
}

/// `seat pick`'s request before its flags are read: the table's mode stands when `--mode` is not given (§5.1).
pub fn default_request(table: &Table) -> Request {
    let mut request = Request::new("");
    request.mode = table.mode;
    request
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
    /// Plan 24 §4: an inherited override on retry/send-back does not count against overrides_per_run.
    pub inherited_override: bool,
    /// Plan 29 §5.2: why only a claude seat may take the order (it needs a project MCP server); `None` for any seat.
    pub claude_only: Option<String>,
    /// Plan 33 §2.2: the profile the order is seated with; `None` for none.
    pub profile: Option<Narrow>,
    /// A child's re-pick (a send-back, its step-up, a rate-limit reseat): the effort its parent ran at, the caller's
    /// `--effort` included. It replaces the class's and the profile's effort as the base; the step down under pressure
    /// still applies to it, read now (live checks 2026-11-01, F3).
    pub effort: Option<Effort>,
}

/// Plan 33 §2.2: what a profile asks of the seat rule. "`model` and `family` narrow the candidate seats; they never
/// skip usage floors, reserves or modes", and "`effort` replaces the class effort".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Narrow {
    pub name: String,
    pub model: Option<String>,
    pub family: Option<String>,
    pub effort: Option<Effort>,
}

impl Narrow {
    /// Why the profile does not take this seat; `None` when it does. A model matches the seat's id, the id at any of
    /// its efforts, or an agy id without its effort (`gemini-3.8-flash` for `gemini-3.8-flash-{effort}`).
    pub fn refuses(&self, seat: &Seat) -> Option<String> {
        if let Some(model) = &self.model {
            let fits = seat.model == *model
                || seat.model.strip_suffix("-{effort}") == Some(model.as_str())
                || seat
                    .efforts
                    .iter()
                    .any(|effort| seat.model_for(*effort) == *model);
            if !fits {
                return Some(format!(
                    "profile {} asks for model {model}; this seat is {}",
                    self.name, seat.model
                ));
            }
        }
        if let Some(family) = self
            .family
            .as_ref()
            .filter(|family| **family != seat.family)
        {
            return Some(format!(
                "profile {} asks for family {family}; this seat is {}",
                self.name, seat.family
            ));
        }
        None
    }
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
            inherited_override: false,
            claude_only: None,
            profile: None,
            effort: None,
        }
    }
}

/// Plan 14 §2.1: `(elapsed, projected, spare)` from a fresh reading and the pool's live booked share.
/// `projected = used / max(elapsed, pace_ease)` is not clamped to `pace_min` and does not add `pace_ease` to
/// `used`; `spare = max(0, 1 - projected - booked)`.
pub fn projected_spare(
    reading: &Reading,
    booked: f32,
    pace_ease: f32,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    let Some(used) = reading.used else {
        return (None, None, None);
    };
    let Some(elapsed) = nirvana_usage::elapsed(reading) else {
        return (None, None, None);
    };
    let projected = used / elapsed.max(pace_ease).max(1e-6);
    let spare = (1.0 - projected - booked).max(0.0);
    (Some(elapsed), Some(projected), Some(spare))
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
    pub elapsed: Option<f32>,
    #[serde(serialize_with = "six_opt")]
    pub projected: Option<f32>,
    #[serde(serialize_with = "six_opt")]
    pub spare: Option<f32>,
    #[serde(serialize_with = "six_opt")]
    pub pace: Option<f32>,
    #[serde(serialize_with = "six")]
    pub reset_factor: f32,
    #[serde(serialize_with = "six")]
    pub spare_weight: f32,
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
        let full =
            Some(self.expected / room.max(1e-6) * self.pace.unwrap_or(1.0) * self.reset_factor)?;
        let spare = self.spare.unwrap_or(0.0).clamp(0.0, self.expected);
        let rest = (self.expected - spare).max(0.0);
        Some(spare * self.spare_weight + full * (rest / self.expected))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkipKind {
    Off,
    Cli,
    Need,
    Class,
    Family,
    /// Plan 33 §2.2: the order's profile asks for another model or family.
    Profile,
    Floor,
    Headroom,
    Reserve,
    /// Plan 40a: a seat with its own window, one of whose windows has no fresh reading. Not priced: a cheaper effort
    /// reads nothing.
    Unread,
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
    /// The step-down pass gave this seat a lower effort than the class's; false for a seat already at its lowest.
    stepped: bool,
}

impl Candidate {
    fn pressure(&self) -> f32 {
        self.windows
            .iter()
            .filter_map(WindowState::pressure)
            .fold(0.0, f32::max)
    }

    /// A window not read, or a pool with no windows at all (`local`): either way its headroom is not known, and
    /// a pool with nothing to read is not free (review nit: `local` would have won every chore at pressure 0).
    fn unread(&self) -> bool {
        self.windows.is_empty() || self.windows.iter().any(|window| !window.known())
    }

    fn unknown(&self) -> Vec<String> {
        if self.windows.is_empty() {
            return vec!["no windows".to_owned()];
        }
        self.windows
            .iter()
            .filter(|window| !window.known())
            .map(|window| window.window.clone())
            .collect()
    }

    /// The first known window too small for this order.
    fn short(&self) -> Option<&WindowState> {
        self.windows
            .iter()
            .find(|window| window.known() && !window.holds())
    }

    /// §3's order before pressure: 0 known headroom that holds the order, 1 a window not read, 2 a known window
    /// that cannot hold it. A reader that fails ranks "below known headroom and above exhausted", and a window
    /// too small for the order is nearer exhausted than free: seated there, a mid-order rate limit is near
    /// certain (review S2). A seat with both an unread and a short window is short.
    fn tier(&self) -> u8 {
        if self.short().is_some() {
            2
        } else if self.unread() {
            1
        } else {
            0
        }
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
    /// Windows not read ("no windows" for a pool without any): the seat ranked below every seat with known
    /// headroom.
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
    /// Plan 24 §4: an inherited override on retry/send-back.
    #[serde(default)]
    pub inherited_override: bool,
    /// What the rule would have picked, when an override chose instead.
    pub rule_pick: Option<String>,
    /// `--not-family` left no seat, so the class was seated on the excluded family, in a fresh session
    /// (decision 0017's fallback). The journal counts these.
    pub same_family_fallback: bool,
    pub runners_up: Vec<Loser>,
    /// Plan 12 O8b: the seats that cleared the bar, the pick first, when more than one did: what Jev's seating
    /// advice chooses among (§9 role 3). Empty when one seat was left, and on an override.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contenders: Vec<Contender>,
    /// Plan 12 O8b: what Jev advised for this order, set by `nirvana_jev_roles` after the rule picked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jev_pick: Option<crate::nirvana_jev_roles::JevPick>,
    /// Plan 33 §2.2: the profile the pick was narrowed by, kept in the lease so `seat run` seats with the same one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// The needs the booking was made with, as given (bladebro and the project's servers included): `seat run` with
    /// no `--needs` reuses the booking, and its sources line reads them (gsrc review S1). Empty from `pick` itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub needs: Vec<String>,
}

/// One seat that cleared the bar: the facts Jev weighs (O8b).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contender {
    pub seat: String,
    pub effort: Effort,
    pub model: String,
    pub family: String,
    pub pool: String,
    #[serde(serialize_with = "six")]
    pub score: f32,
    #[serde(serialize_with = "six")]
    pub pressure: f32,
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
#[allow(clippy::too_many_arguments)]
fn effort_for(
    class: &Class,
    entry: &ClassSeat,
    seat: &Seat,
    mode: Mode,
    step_down: bool,
    is_override: bool,
    override_effort: Option<Effort>,
    profile_effort: Option<Effort>,
    parent_effort: Option<Effort>,
) -> (Effort, Option<String>) {
    let mut note = None;
    // Live checks 2026-11-01 F3: lc4 ran research.read at `--effort low` and its send-back lc4-s1 was seated on flash
    // medium, the class's effort. A child's base is the parent's effort; an override naming an effort (an inherited
    // `seat:effort`) still wins, and plan 37's step-up names a seat only, so it keeps the parent's too.
    let parent = parent_effort.filter(|_| override_effort.is_none());
    let mut want = if let Some(eff) = override_effort {
        eff
    } else if let Some(eff) = parent {
        eff
    } else if let Some(eff) = profile_effort {
        // Plan 33 §2.2: "`effort` replaces the class effort"; the mode's lift, the step down and the cap still apply.
        eff
    } else {
        entry.effort.unwrap_or_else(|| match mode {
            Mode::Efficient => class.efficient_effort.unwrap_or(class.effort),
            Mode::Balanced => class.effort,
            Mode::Sota => class.effort,
        })
    };
    // The parent's effort already had the mode's lift when it was picked; a caller's lower `--effort` is kept.
    if override_effort.is_none() && parent.is_none() && mode == Mode::Sota {
        if let Some(lift) = class.sota_effort {
            want = want.max(lift);
        }
    }
    if step_down {
        want = want.down().unwrap_or(want);
    }
    // A parent's effort above high came from an override or the caller: a child keeps it uncapped.
    if let Some(eff) = override_effort {
        if eff > Effort::High {
            note = Some(format!("effort overridden to {}", eff.word()));
        }
    } else if want > Effort::High && parent.is_none() {
        want = Effort::High;
        note = Some("capped at high".to_owned());
    }
    let offered: Vec<Effort> = seat
        .efforts
        .iter()
        .copied()
        .filter(|effort| is_override || parent == Some(Effort::Max) || *effort != Effort::Max)
        .collect();
    let got = offered
        .iter()
        .copied()
        .filter(|effort| *effort <= want)
        .max()
        .or_else(|| offered.iter().copied().min())
        .unwrap_or(want);
    if let Some(eff) = parent {
        note = Some(if got == eff {
            format!("effort {} kept from the parent", eff.word())
        } else {
            format!("the parent's effort {}", eff.word())
        });
    }
    if got != want {
        let seat_note = format!("{} has no {}; {}", seat.id, want.word(), got.word());
        note = Some(match note {
            Some(n) => format!("{n}; {seat_note}"),
            None => seat_note,
        });
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

/// Plan 14 §2.2: "fits G·Claude's spare (week 96% projected unused, resets in 11h)" when every window of the
/// seat holds the order inside its projected spare.
fn spare_phrase(pool: &str, windows: &[WindowState], now: u64) -> Option<String> {
    if windows.is_empty()
        || !windows.iter().all(|window| {
            window.expected > 0.0
                && window
                    .spare
                    .is_some_and(|spare| spare > 0.0 && window.expected <= spare + EDGE)
        })
    {
        return None;
    }
    let window = windows.iter().max_by(|a, b| {
        a.elapsed
            .unwrap_or(0.0)
            .total_cmp(&b.elapsed.unwrap_or(0.0))
    })?;
    let spare = window.spare?;
    let resets = window
        .resets_at
        .map(|at| {
            format!(
                ", resets in {}",
                nirvana_usage::short_duration(at.saturating_sub(now))
            )
        })
        .unwrap_or_default();
    Some(format!(
        "fits {}'s spare ({} {} projected unused{resets})",
        nirvana_usage::label(pool),
        window.window,
        pct(spare),
    ))
}

/// Seats an order: the rule of §5.3, deterministic and pure. A NoSeat names every seat the class takes and
/// why it was skipped, and the soonest reset that would free one.
/// Plan 19 §6.2: climb over windows of a pool, and the window that yielded the max climb.
pub fn windows_climb(windows: &[WindowState], pace_ease: f32) -> Option<(f32, String)> {
    let mut best: Option<(f32, String)> = None;
    for w in windows {
        if let Some(u) = w.used {
            let el = w.elapsed.unwrap_or(0.0).max(pace_ease);
            let climb = (u + w.booked) / el;
            match &best {
                None => best = Some((climb, w.window.clone())),
                Some((cur, _)) if climb > *cur => best = Some((climb, w.window.clone())),
                _ => {}
            }
        }
    }
    best
}

/// Plan 19 §6.2: climb t factor clamped to [0, 1].
pub fn climb_t(climb: f32, climb_start: f32, climb_full: f32) -> f32 {
    if climb <= climb_start || climb_full <= climb_start {
        0.0
    } else {
        ((climb - climb_start) / (climb_full - climb_start)).clamp(0.0, 1.0)
    }
}

pub fn pick(table: &Table, world: &World, request: &Request) -> Result<Pick, NoSeat> {
    // Plan 33 §2.2: "When no candidate fits, the pick says so, naming the profile."
    pick_unnamed(table, world, request).map_err(|mut no_seat| {
        if let Some(profile) = &request.profile {
            no_seat.reason = format!("with profile {}: {}", profile.name, no_seat.reason);
        }
        no_seat
    })
}

fn pick_unnamed(table: &Table, world: &World, request: &Request) -> Result<Pick, NoSeat> {
    let Some(class) = table.class(&request.class) else {
        return Err(NoSeat {
            class: request.class.clone(),
            reason: format!("no class {} in seats.toml", request.class),
            earliest_reset: None,
            defer: false,
            skipped: Vec::new(),
        });
    };
    match seat_once(table, world, request, class) {
        // Decision 0017: another family is a preference, not a wall. With no other family left and no
        // override, the class is seated again on its own family, in a fresh session (a new order, never the
        // builder's conversation), and the pick says so. A wall turned a family's outage into a deferred
        // review, up to a week under §5.6 step 4 (review B1).
        Err(other) if !request.not_family.is_empty() && request.override_seat.is_none() => {
            let mut own = request.clone();
            own.not_family.clear();
            match seat_once(table, world, &own, class) {
                Ok(mut got) => {
                    // Each other-family seat and what stopped it, as 0017's `same_family_reason` names them.
                    let stopped: Vec<String> = other
                        .skipped
                        .iter()
                        .filter(|loser| {
                            table
                                .seat(&loser.seat)
                                .is_some_and(|seat| !request.not_family.contains(&seat.family))
                        })
                        .map(|loser| format!("{} ({})", loser.seat, loser.why))
                        .collect();
                    let why = if stopped.is_empty() {
                        other.reason
                    } else {
                        stopped.join("; ")
                    };
                    got.same_family_fallback = true;
                    got.reason.push_str(&format!(
                        "; same family ({}), in a fresh session: no other family had a seat: {why} (decision 0017)",
                        got.family
                    ));
                    Ok(got)
                }
                // Its own family's skips, with their resets: the earliest that frees any seat at all.
                Err(mut none) => {
                    none.reason
                        .push_str("; its own family was tried too (decision 0017)");
                    Err(none)
                }
            }
        }
        other => other,
    }
}

/// Plan 37 §1.3 (L5, "Step up one seat"): the seat a redo takes after its sources were invented, the next stronger
/// seat of the same class: the class's own score first, then the seat's output price at `now`, so a tie in the
/// score goes to the dearer model (research.web scores flash and argon 84 each; flash low invented
/// reel-motion-gemini's sources and argon medium's redo was real, MotoProject 2026-10-23 and 31). `None` when no
/// enabled seat of the class stands above `seat`: the gate then asks, never the rule's pick (review S1).
pub fn step_up(table: &Table, class: &str, seat: &str, now: u64) -> Option<String> {
    let class = table.class(class)?;
    let strength = |entry: &ClassSeat| -> Option<(f32, f64)> {
        let seat = table.seat(&entry.seat).filter(|seat| seat.enabled)?;
        let price = seat.price_at(now).map_or(0.0, |price| price.output);
        Some((entry.score, price))
    };
    let here = class
        .seats
        .iter()
        .find(|entry| entry.seat == seat)
        .and_then(strength)?;
    let above = |(score, price): (f32, f64)| {
        score > here.0 + EDGE || ((score - here.0).abs() <= EDGE && price > here.1)
    };
    class
        .seats
        .iter()
        .filter_map(|entry| Some((entry, strength(entry)?)))
        .filter(|(entry, at)| entry.seat != seat && above(*at))
        .min_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map(|(entry, _)| entry.seat.clone())
}

/// One pass of the rule at the class's effort and, when a skip was priced, one effort step down (§5.5).
fn seat_once(
    table: &Table,
    world: &World,
    request: &Request,
    class: &Class,
) -> Result<Pick, NoSeat> {
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
    // with a window not read is not counted as one that holds. SOTA skips headroom filter (filter 6).
    if request.mode != Mode::Sota {
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
            let Some((other_pool, other_seat)) =
                holders.iter().find(|(pool, _)| *pool != seat.pool)
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

    if let Some((override_target, reason)) = &request.override_seat {
        // The effort is applied in `candidate`; here it is only checked (review D3: it was parsed and dropped).
        let seat_id = match override_target.split_once(':') {
            Some((id, eff_str)) => {
                if Effort::parse(eff_str).is_none() {
                    return Err(fail(
                        format!("override {override_target} refused: unknown effort {eff_str}"),
                        &candidates,
                        false,
                    ));
                }
                id.trim()
            }
            None => override_target.trim(),
        };
        // Review a, N9: one condition, no longer nested under a clippy allow that only kept an old control's find text.
        if !request.inherited_override && world.overrides_used >= constants.overrides_per_run {
            return Err(fail(
                format!(
                    "override {override_target} refused: {} of {} overrides already taken in this run; more is the owner's call",
                    world.overrides_used, constants.overrides_per_run
                ),
                &candidates,
                false,
            ));
        }
        let Some(cand) = candidates
            .iter()
            .find(|cand| table.seats[cand.index].id == seat_id)
        else {
            return Err(fail(
                format!("override {override_target} refused: no such seat"),
                &candidates,
                false,
            ));
        };
        if let Some(skip) = &cand.skip {
            return Err(fail(
                format!("override {override_target} refused: {}", skip.why),
                &candidates,
                false,
            ));
        }
        let mode_bar = table.bars(class).at(request.mode);
        let rule = rank(table, &candidates, class, request.mode, mode_bar).ok();
        let rule_pick = rule.as_ref().map(|(order, _, _)| {
            let winner = &candidates[order[0]];
            format!(
                "{} {}",
                table.seats[winner.index].id,
                winner.effort.map_or("", Effort::word)
            )
        });
        let index = candidates
            .iter()
            .position(|cand| table.seats[cand.index].id == seat_id)
            .expect("found above");
        // The bar the seat stands at: an override can take a seat under the mode's bar, and the pick says
        // so instead of reporting a bar it did not clear (review nit).
        let score = candidates[index].score;
        let under = score + EDGE < mode_bar;
        let bar = if under { score } else { mode_bar };
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
            world.now,
            None,
        );
        out.override_reason = Some(reason.clone());
        out.inherited_override = request.inherited_override;
        out.rule_pick = rule_pick.clone();
        let under = if under {
            format!(
                "; {override_target} scores {score:.0}, under the bar {mode_bar:.0} at {}",
                request.mode.word()
            )
        } else {
            String::new()
        };
        out.reason = format!(
            "{}{under}; override: {reason}; the rule's pick: {}",
            out.reason,
            rule_pick.as_deref().unwrap_or("none")
        );
        // The seats the rule would have ranked lost to the override, not to a bar or a price.
        for loser in &mut out.runners_up {
            let ranked = candidates.iter().any(|cand| {
                table.seats[cand.index].id == loser.seat && cand.in_class && cand.skip.is_none()
            });
            if ranked {
                loser.why = format!("the override took {seat_id}");
            }
        }
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
    let mut climb_info: Option<f32> = None;
    let wanted_bar = if request.mode == Mode::Balanced {
        let primary_survivors: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.in_class && c.skip.is_none() && !table.seats[c.index].fallback)
            .map(|(i, _)| i)
            .collect();
        let best_idx = primary_survivors
            .iter()
            .copied()
            .max_by(|&a, &b| candidates[a].score.total_cmp(&candidates[b].score))
            .or_else(|| {
                candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.in_class && c.skip.is_none())
                    .map(|(i, _)| i)
                    .max_by(|&a, &b| candidates[a].score.total_cmp(&candidates[b].score))
            });
        if let Some(best) = best_idx {
            let best_cand = &candidates[best];
            let hot_pool = table.seats[best_cand.index].pool.clone();
            let (_climb, t) =
                if let Some((climb, _)) = windows_climb(&best_cand.windows, constants.pace_ease) {
                    climb_info = Some(climb);
                    (
                        climb,
                        climb_t(climb, constants.climb_start, constants.climb_full),
                    )
                } else {
                    climb_info = Some(0.0);
                    (0.0, 0.0)
                };
            let bars = table.bars(class);
            let b_bar = bars.balanced - t * (bars.balanced - bars.efficient);
            if t >= 0.5 {
                for cand in candidates.iter_mut() {
                    let seat = &table.seats[cand.index];
                    if cand.in_class && cand.skip.is_none() && seat.pool == hot_pool {
                        let entry = class.seats.iter().find(|e| e.seat == seat.id);
                        // Plan 33 §2.2: a profile's effort is the base the mode steps down from; a child's base
                        // is its parent's effort (F3), so a caller's high steps to medium, never past it.
                        let base_effort = request
                            .effort
                            .or_else(|| request.profile.as_ref().and_then(|profile| profile.effort))
                            .or_else(|| entry.and_then(|e| e.effort))
                            .unwrap_or(class.effort);
                        let min_effort = class.efficient_effort.unwrap_or(class.effort);
                        let stepped_effort =
                            base_effort.down().unwrap_or(base_effort).max(min_effort);
                        if cand.effort.is_some_and(|eff| eff > stepped_effort) {
                            cand.effort = Some(stepped_effort);
                            cand.stepped = true;
                            populate_windows(
                                table,
                                world,
                                request,
                                class,
                                size,
                                seat,
                                cand,
                                stepped_effort,
                            );
                        }
                    }
                }
            }
            b_bar
        } else {
            table.bars(class).balanced
        }
    } else {
        table.bars(class).at(request.mode)
    };
    match rank(table, &candidates, class, request.mode, wanted_bar) {
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
            world.now,
            climb_info,
        )),
        Err(reason) => Err(fail(reason, &candidates, priced)),
    }
}

fn populate_windows(
    table: &Table,
    world: &World,
    request: &Request,
    class: &Class,
    size: &Size,
    seat: &Seat,
    cand: &mut Candidate,
    effort: Effort,
) {
    let constants = &table.constants;
    let Some(pool) = table.pool(&seat.pool) else {
        return;
    };
    cand.windows.clear();
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
        let headroom = left.map(|left| left - reserve);
        let resets_at = reading.and_then(|reading| reading.resets_at);
        let (elapsed, projected, spare) = reading
            .map(|reading| projected_spare(reading, booked, constants.pace_ease))
            .unwrap_or((None, None, None));
        let pace = reading.and_then(|reading| {
            nirvana_usage::pace_with(
                reading,
                constants.pace_ease,
                constants.pace_min,
                constants.pace_max,
            )
        });
        // "More than 30% left" is what this order could still use: after booked orders and the reserve. At
        // max 5 h with 35% unused, 20% is usable, and that is not quota about to be lost (review nit).
        let reset_factor = match (headroom, resets_at) {
            (Some(room), Some(at)) if request.mode != Mode::Sota => {
                reset_factor(room, at.saturating_sub(world.now), size.minutes, constants)
            }
            _ => 1.0,
        };
        cand.windows.push(WindowState {
            window: name,
            used,
            booked,
            left,
            headroom,
            resets_at,
            elapsed,
            projected,
            spare,
            pace,
            reset_factor,
            spare_weight: constants.spare_weight,
            expected,
        });
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
        stepped: false,
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
    if let Some(why) = request
        .claude_only
        .as_ref()
        .filter(|_| seat.cli != "claude")
    {
        cand.skip = skip(SkipKind::Need, why.clone());
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
    };
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
    if let Some(why) = request
        .profile
        .as_ref()
        .and_then(|profile| profile.refuses(seat))
    {
        cand.skip = skip(SkipKind::Profile, why);
        return cand;
    }
    let profile_effort = request.profile.as_ref().and_then(|profile| profile.effort);
    let (is_override, override_effort) = match &request.override_seat {
        Some((target, _)) => {
            let (target_id, target_eff) = match target.split_once(':') {
                Some((id, eff_str)) => (id.trim(), Effort::parse(eff_str)),
                None => (target.trim(), None),
            };
            if target_id == seat.id {
                (true, target_eff)
            } else {
                (false, None)
            }
        }
        None => (false, None),
    };
    let (effort, note) = effort_for(
        class,
        entry,
        seat,
        request.mode,
        step_down,
        is_override,
        override_effort,
        profile_effort,
        request.effort,
    );
    cand.effort = Some(effort);
    cand.effort_note = note;
    // A seat already at its lowest effort (argon low, flash low in build.bounded) is not stepped down.
    cand.stepped = step_down
        && effort
            < effort_for(
                class,
                entry,
                seat,
                request.mode,
                false,
                is_override,
                override_effort,
                profile_effort,
                request.effort,
            )
            .0;
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
    populate_windows(table, world, request, class, size, seat, &mut cand, effort);
    // Filter 5: a window at or under the floor.
    let floored: Vec<&WindowState> = cand
        .windows
        .iter()
        .filter(|window| {
            window
                .left
                .is_some_and(|left| left <= constants.floor + EDGE)
        })
        .collect();
    if let Some(window) = floored.first() {
        // The seat is free only when every floored window has reset: the latest of them, and unknown when one
        // has no reset time. The first one's reset sent a NoSeat to defer 20 minutes for a 5 h window while
        // the week stayed floored for days (review S4).
        let free_at = floored
            .iter()
            .map(|window| window.resets_at)
            .collect::<Option<Vec<u64>>>()
            .and_then(|ats| ats.into_iter().max());
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
            resets_at: free_at,
        });
    }
    if cand.skip.is_none() {
        cand.skip = scoped_skip(table, world, seat, &cand.windows);
    }
    cand
}

/// "Fable week" for `week.fable`, "max 5h" for a pool window: what a skip or the orchestrator's reason names.
fn window_label(pool: &str, window: &str) -> String {
    match window.split_once('.') {
        Some((base, scope)) => format!("{} {base}", capitalized(scope)),
        None => format!("{pool} {window}"),
    }
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Why a window of `seat` has no fresh reading: never read, or read but stale. `None` when it is fresh.
fn unread_why(world: &World, pool: &str, window: &str) -> Option<String> {
    let label = window_label(pool, window);
    let any = world
        .readings
        .iter()
        .find(|reading| reading.pool == pool && reading.window == window && reading.used.is_some());
    match any {
        None => Some(format!("{label} not read")),
        Some(reading) if nirvana_usage::freshness(reading, world.now) != Freshness::Fresh => {
            Some(format!("{label} reading is stale"))
        }
        Some(_) => None,
    }
}

/// Plan 40a, the fail-closed rule for a seat with its own window (Fable's `week.fable`): skipped unless every window it
/// spends, its pool's and its own, has a fresh reading, and the order leaves `fable_reserve` of each of its own. It
/// runs in `candidate`, so it holds in every mode, SOTA's included (SOTA ignores pressure, not filters), and the
/// override path refuses a skipped seat. The reason is the owner's: `claude -p` bills a Fable request past the week to
/// usage credits without asking (code.claude.com model-config).
fn scoped_skip(table: &Table, world: &World, seat: &Seat, windows: &[WindowState]) -> Option<Skip> {
    if seat.extra_windows.is_empty() {
        return None;
    }
    let model = seat
        .extra_windows
        .first()
        .and_then(|window| window.split_once('.'))
        .map_or_else(|| seat.id.clone(), |(_, scope)| capitalized(scope));
    let cost = format!("a headless {model} run past its week bills usage credits without asking");
    // Its own windows first: "Fable week not read" is the reason that names the money.
    let own = |window: &&WindowState| seat.extra_windows.contains(&window.window);
    for window in windows
        .iter()
        .filter(own)
        .chain(windows.iter().filter(|w| !own(w)))
    {
        if let Some(why) = window
            .used
            .is_none()
            .then(|| unread_why(world, &seat.pool, &window.window))
            .flatten()
        {
            return Some(Skip {
                kind: SkipKind::Unread,
                why: format!("{why}; {cost}"),
                resets_at: None,
            });
        }
    }
    let reserve = table.constants.fable_reserve;
    for window in windows
        .iter()
        .filter(|window| seat.extra_windows.contains(&window.window))
    {
        let left = window.left.unwrap_or(0.0);
        let after = left - window.expected;
        if after + EDGE < reserve {
            return Some(Skip {
                kind: SkipKind::Reserve,
                why: format!(
                    "would leave {} at {}, under the reserve {}; {cost}",
                    window_label(&seat.pool, &window.window),
                    pct(after.max(0.0)),
                    pct(reserve)
                ),
                resets_at: window.resets_at,
            });
        }
    }
    None
}

/// Plan 40a fix S1: may `seat` spend its own scoped windows right now, for `order` at `effort`? `scoped_skip` ran only
/// in `candidate`, so only on a fresh pick, and a live lease was reused by `pick --book` and `seat run` without it: a
/// Fable booking made at 40% used was started at 95%, or after its reading went stale, and `claude -p` bills past the
/// week without asking (package review a, S1). This asks the same question on `world` as it is now, with the order's
/// own booking taken out of `booked`. A seat with no scoped window is never refused here, and its usage is not read.
/// A seat turned off since its booking is refused too: the owner took it away.
pub fn may_spend_now(
    table: &Table,
    world: &World,
    seat_id: &str,
    effort: Effort,
    class_id: &str,
    size: SizeName,
    order: &str,
) -> Result<(), String> {
    let Some(seat) = table.seat(seat_id) else {
        return Ok(());
    };
    if seat.extra_windows.is_empty() {
        return Ok(());
    }
    if !seat.enabled {
        return Err(format!("{}: off in seats.toml", seat.id));
    }
    let Some(class) = table.class(class_id) else {
        return Err(format!("{}: no class {class_id} in seats.toml", seat.id));
    };
    let mut request = Request::new(class_id);
    request.size = size;
    request.order = Some(order.to_owned());
    let mut cand = Candidate {
        index: 0,
        in_class: true,
        effort: Some(effort),
        effort_note: None,
        score: 0.0,
        record_note: None,
        use_basis: None,
        windows: Vec::new(),
        skip: None,
        stepped: false,
    };
    populate_windows(
        table,
        world,
        &request,
        class,
        table.size(size),
        seat,
        &mut cand,
        effort,
    );
    match scoped_skip(table, world, seat, &cand.windows) {
        Some(skip) => Err(format!("{}: {}", seat.id, skip.why)),
        None => Ok(()),
    }
}

/// A booking's size from its stored pick. A pick that does not say reads as large: the largest expected use, so the
/// reserve check errs toward refusing.
pub fn lease_size(lease: &Lease) -> SizeName {
    lease
        .pick
        .get("size")
        .and_then(|size| serde_json::from_value(size.clone()).ok())
        .unwrap_or(SizeName::Large)
}

/// What a refused spend says: what to do first, then why (the RULES.md style).
pub fn spend_refused(order: &str, why: &str) -> String {
    format!(
        "Book {order} again with `herdr seat pick --book {order}`. Its booking was released and nothing was started. \
         {why}"
    )
}

/// `may_spend_now` at the moment a worker starts, from the live world and the live bookings. On a refusal the order's
/// lease is released under `leases.lock`, and the message is `spend_refused`. A `leases.json` that does not parse
/// refuses a scoped seat: what others booked is not known, so its reserve cannot be checked.
// Called from `nirvana_worker::exec` by the S1 hunk this package hands to the worker builder; until it lands nothing
// in this crate calls it. Drop the allow with that hunk.
pub fn check_before_spend(
    root: &Path,
    seat_id: &str,
    effort: Effort,
    class_id: &str,
    size: SizeName,
    order: &str,
) -> Result<(), String> {
    let table = table_for(root)?;
    if table
        .seat(seat_id)
        .is_none_or(|seat| seat.extra_windows.is_empty())
    {
        return Ok(());
    }
    let dir = dir();
    let path = dir.join(LEASES_FILE);
    let mut world = live_world(&table);
    world.leases = load_leases(&path)?.leases;
    world.now = now();
    let Err(why) = may_spend_now(&table, &world, seat_id, effort, class_id, size, order) else {
        return Ok(());
    };
    let released = Lock::take(&dir).is_ok_and(|_lock| {
        load_leases(&path).is_ok_and(|mut leases| {
            !leases.release(order, now()) || write_leases(&path, &leases).is_ok()
        })
    });
    if released {
        Err(spend_refused(order, &why))
    } else {
        Err(format!(
            "Release {order} with `herdr seat release {order}`, then book it again. Nothing was started. {why}"
        ))
    }
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
    wanted: f32,
) -> Result<(Vec<usize>, f32, bool), String> {
    let bars = table.bars(class);
    let survivors: Vec<usize> = (0..candidates.len())
        .filter(|&i| candidates[i].skip.is_none())
        .collect();
    let primary_survivors: Vec<usize> = survivors
        .iter()
        .copied()
        .filter(|&i| !table.seats[candidates[i].index].fallback)
        .collect();
    let effective_survivors = if !primary_survivors.is_empty() {
        &primary_survivors
    } else {
        &survivors
    };
    let Some(&best) = effective_survivors
        .iter()
        .max_by(|&&a, &&b| candidates[a].score.total_cmp(&candidates[b].score))
    else {
        return Err(format!("no seat left for {}", class.id));
    };
    let best_score = candidates[best].score;
    let (bar, dropped) = if mode == Mode::Sota {
        (0.0, false)
    } else if best_score + EDGE >= wanted {
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
    // Ties go to the higher score, then to the class's own order, §5.4's row: with the agy reader stale,
    // flash and argon in research.web are both unread at 84, and table order gave argon (review S8). Every
    // survivor is in the class and a class lists a seat once, so no tie is left after it.
    let place = |cand: &Candidate| {
        class
            .seats
            .iter()
            .position(|entry| entry.seat == table.seats[cand.index].id)
    };
    order.sort_by(|&a, &b| {
        let (cand_a, cand_b) = (&candidates[a], &candidates[b]);
        let fallback_a = table.seats[cand_a.index].fallback;
        let fallback_b = table.seats[cand_b.index].fallback;
        if fallback_a != fallback_b {
            return fallback_a.cmp(&fallback_b);
        }
        if mode == Mode::Sota {
            return cand_b
                .score
                .total_cmp(&cand_a.score)
                .then(place(cand_a).cmp(&place(cand_b)));
        }
        let (a, b) = (cand_a, cand_b);
        a.tier()
            .cmp(&b.tier())
            .then(a.pressure().total_cmp(&b.pressure()))
            .then(b.score.total_cmp(&a.score))
            .then(place(a).cmp(&place(b)))
    });
    Ok((order, bar, dropped))
}

/// The pick, its reason line and every other seat with why it lost.
#[allow(clippy::too_many_arguments)]
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
    now: u64,
    _climb: Option<f32>,
) -> Pick {
    let constants = &table.constants;
    let won = &candidates[winner];
    let seat = &table.seats[won.index];
    let effort = won.effort.unwrap_or(class.effort);
    let mode_bar = table.bars(class).at(request.mode);
    let lost_to = |cand: &Candidate| -> String {
        let cand_seat = &table.seats[cand.index];
        let won_seat = &table.seats[won.index];
        if !won_seat.fallback && cand_seat.fallback {
            return format!("fallback seat; {} is not", won_seat.id);
        }
        if request.mode == Mode::Sota {
            if (cand.score - won.score).abs() > EDGE {
                return format!("scores {:.1}, under {:.1}", cand.score, won.score);
            }
            return "same score, later in the class's order".to_owned();
        }
        let (a, b) = (cand.pressure(), won.pressure());
        let pool = &table.seats[cand.index].pool;
        match (cand.tier(), won.tier(), cand.short()) {
            (2, below, Some(short)) if below < 2 => format!(
                "{pool} {} has {} left for an order of {}: ranks below {}",
                short.window,
                pct(short.headroom.unwrap_or(0.0).max(0.0)),
                pct(short.expected),
                if below == 0 {
                    "known headroom"
                } else {
                    "an unread pool"
                }
            ),
            (1, 0, _) if cand.windows.is_empty() => {
                format!("{pool} has no windows to read: ranks below known headroom")
            }
            (1, 0, _) => format!(
                "{} not read: ranks below known headroom",
                cand.unknown().join(", ")
            ),
            _ if (a - b).abs() > 1e-6 => format!("pressure {a:.4} against {b:.4}"),
            _ if (cand.score - won.score).abs() > EDGE => format!(
                "same pressure, scores {:.1} against {:.1}",
                cand.score, won.score
            ),
            _ => "same pressure and score, later in the class's order".to_owned(),
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
        spare_phrase(&seat.pool, &won.windows, now)
            .unwrap_or_else(|| windows_phrase(&seat.pool, &won.windows))
    );
    if seat.pool == constants.reserve_pool {
        // An unread reserve window leaves the reserve unknown: the seat ranked with the unread, and the line
        // says the reserve was not checked rather than that it held (review S2).
        let unread = won
            .windows
            .iter()
            .any(|window| window.window == constants.reserve_window && !window.known());
        if unread {
            reason.push_str(&format!(
                ", reserve {} not checked ({} not read)",
                pct(constants.reserve),
                constants.reserve_window
            ));
        } else {
            reason.push_str(&format!(", reserve {}", pct(constants.reserve)));
        }
    }
    if seat.fallback {
        let skipped: Vec<String> = candidates
            .iter()
            .filter(|c| c.in_class && !table.seats[c.index].fallback)
            .map(|c| {
                let s = &table.seats[c.index];
                let why = c
                    .skip
                    .as_ref()
                    .map(|sk| sk.why.as_str())
                    .unwrap_or("skipped");
                format!("{} ({})", s.id, why)
            })
            .collect();
        if !skipped.is_empty() {
            reason.push_str(&format!("; fallback: {}", skipped.join("; ")));
        }
    }
    if dropped {
        reason.push_str(&format!(
            "; bar dropped to {bar:.0} (none left reached {mode_bar:.0})"
        ));
    }
    let stepped = step_down && won.stepped;
    if stepped {
        reason.push_str("; one effort step down (the class's effort found no seat)");
    }
    if let Some(note) = &won.effort_note {
        reason.push_str(&format!("; {note}"));
    }
    if let Some(note) = &won.record_note {
        reason.push_str(&format!("; {note}"));
    }
    if let Some(profile) = &request.profile {
        reason.push_str(&format!("; profile {}", profile.name));
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
        step_down: stepped,
        override_reason: None,
        inherited_override: request.inherited_override,
        rule_pick: None,
        same_family_fallback: false,
        runners_up,
        contenders: if beaten.is_empty() {
            Vec::new()
        } else {
            std::iter::once(winner)
                .chain(beaten.iter().copied())
                .map(|i| {
                    let cand = &candidates[i];
                    let seat = &table.seats[cand.index];
                    let effort = cand.effort.unwrap_or(class.effort);
                    Contender {
                        seat: seat.id.clone(),
                        effort,
                        model: seat.model_for(effort),
                        family: seat.family.clone(),
                        pool: seat.pool.clone(),
                        score: cand.score,
                        pressure: cand.pressure(),
                    }
                })
                .collect()
        },
        jev_pick: None,
        profile: request.profile.as_ref().map(|profile| profile.name.clone()),
        needs: Vec::new(),
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
    /// When the run's last override was accepted; a run older than `RUN_KEEP_S` is pruned.
    #[serde(default)]
    pub at: u64,
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
            .map(|window| (window.window.clone(), round6(window.expected) as f32))
            .collect(),
        booked_at: now,
        expires_at: now + table.size(pick.size).lease_minutes * 60,
        pick: serde_json::to_value(pick).unwrap_or_default(),
    }
}

/// `pick --book` for an order that already has a live lease: that lease, whose stored pick is the answer, or a
/// refusal when the repeat names an override the booking was not. The old pick came back with only
/// `"booked": "existing"` to show that the new `--override` was ignored (review nit).
pub fn existing_booking<'a>(
    leases: &'a Leases,
    order: &str,
    override_seat: Option<&str>,
    now: u64,
) -> Option<Result<&'a Lease, String>> {
    let known = leases.live(order, now)?;
    let overrode = known
        .pick
        .get("override")
        .is_some_and(|value| !value.is_null());
    Some(match override_seat {
        Some(seat) if !(overrode && (known.seat == seat || seat.starts_with(&format!("{}:", known.seat)))) => Err(format!(
            "override {seat} refused: {order} is already booked on {} {} ({}); release it first (herdr seat release {order})",
            known.seat,
            known.effort.word(),
            if overrode {
                "an override"
            } else {
                "the rule's pick"
            }
        )),
        _ => Ok(known),
    })
}

/// What `pick` writes under the lock for a pick it gives: an override counted against its run, the order
/// booked when `order` is given, and runs whose last override is over `RUN_KEEP_S` old dropped. The lease
/// booked, and whether `leases` changed.
pub fn book_pick(
    leases: &mut Leases,
    table: &Table,
    pick: &Pick,
    order: Option<&str>,
    run: Option<&str>,
    now: u64,
) -> (Option<Lease>, bool) {
    let runs = leases.runs.len();
    leases
        .runs
        .retain(|_, count| now.saturating_sub(count.at) <= RUN_KEEP_S);
    let mut changed = leases.runs.len() != runs;
    if let (Some(_), Some(run)) = (&pick.override_reason, run) {
        if !pick.inherited_override {
            let count = leases.runs.entry(run.to_owned()).or_default();
            count.overrides += 1;
            count.at = now;
            changed = true;
        }
    }
    let lease = order.map(|order| leases.book(lease_for(table, pick, order, run, now), now));
    (lease.clone(), changed || lease.is_some())
}

/// `leases.json`, or why it cannot be used. A missing file is empty; a file that does not parse is an error,
/// never read as empty, because the next write would wipe every run's override count (review nit).
pub fn load_leases(path: &Path) -> Result<Leases, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|err| {
            format!(
                "{} does not parse ({err}); it holds each run's override count, so it is not read as empty: fix or delete it",
                path.display()
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Leases::default()),
        Err(err) => Err(format!("{} cannot be read: {err}", path.display())),
    }
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

/// Holds the OS's lock on `leases.lock` while it lives. A read, a pick and a write of `leases.json` happen
/// under it, so four parallel picks in five seconds see each other's bookings (Argon's review F1); the rename
/// alone keeps the file whole but would let two writers lose one lease.
///
/// The lock is `File::try_lock`, which the OS lets go when the handle closes, its process dying included. So
/// a crash leaves nothing to time out, no stale rule is needed, and the file itself stays: nothing deletes it.
/// The first build created the file to lock and deleted it on drop; two takers of a stale file could both
/// get in, and a holder's drop deleted whoever's lock was there (review S3).
pub struct Lock {
    _file: std::fs::File,
}

impl Lock {
    /// Waits up to 10 s for the lock.
    pub fn take(dir: &Path) -> std::io::Result<Self> {
        Self::try_take(dir, 200)
    }

    /// Tries `attempts` times, 50 ms apart; `TimedOut` when another handle held it for all of them.
    pub fn try_take(dir: &Path, attempts: u32) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(LOCK_FILE);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        for attempt in 0..attempts {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if attempt + 1 < attempts {
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
                Err(std::fs::TryLockError::Error(err)) => return Err(err),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{} is held by another herdr", path.display()),
        ))
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
    /// One of O5's words, `nirvana_gate::Outcome`: pass, sent_back, failed, asked, reseated, and the worker's newer
    /// ones. What each counts as in the blend is `tally_word`.
    pub outcome: String,
    #[serde(default)]
    pub tokens: Option<Tokens>,
    /// The order's share of each window, computed from its tokens when not given.
    #[serde(default, rename = "use")]
    pub spent: Option<BTreeMap<String, f32>>,
    #[serde(default)]
    pub at: Option<u64>,
    // Plan 12 O3: what the run's journal knows of the order, so the record answers "how did it run" offline
    // (§7.4.1). All optional, so every line written before O3 still parses, and absent fields are not written.
    /// The line's schema version: 1 from O3 on; absent on O2's lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    /// The pick's reason line (§5.3 step 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, rename = "override", skip_serializing_if = "Option::is_none")]
    pub override_reason: Option<String>,
    /// Unix seconds of the order's `seated` event, and of its last event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seated_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_backs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reseats: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferrals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_stops: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stalls: Option<u32>,
    // The pick as it was booked (O2 review S5): expected use against real use is what offline tuning reads, and
    // the lease that held it is released when the order ends.
    /// What the rule would have picked, when an override chose instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_pick: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<SizeName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar_dropped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_down: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_basis: Option<String>,
    /// The order's expected share of each window, as booked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<BTreeMap<String, f32>>,
    /// The herdr build that wrote the line (`nirvana/VERSION`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub herdr: Option<String>,
    /// The run's journal and its last event when the line was written: `<path>#<seq>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal: Option<String>,
    /// Keys this herdr does not know, kept on a round trip (O2 review S5: a later field must not be dropped).
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// The record's key: seat, effort and chain. By seat and effort, never by provider (decision 0029: "a bad
/// row is not a bad service"); by chain in the first version, with the class kept on each line.
pub type RecordKey = (String, Effort, String);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub passes: u32,
    pub orders: u32,
}

/// The review round (Part 2 item 6): what a record word counts as, for old and new words alike. The worker builder is
/// changing the word for an accepted order (`accepted`, where the record said `failed`: the usage audit's finding 2,
/// 65 lines) and for a subagent order, so the tally reads both. `Some(true)` a pass, `Some(false)` an order that did
/// not pass, `None` neither. A word this herdr does not know is neither: O2 counted it a failure, which is how 65
/// accepted orders read as failures the day a new word came in.
pub fn tally_word(word: &str) -> Option<bool> {
    match word {
        "accepted" => Some(true),
        // O5's words keep the gate's own answer: pass a pass, failed and sent_back failures, reseated not counted, and
        // `blocked` not counted: plan 37b review S1 ruled a login wall the owner's input, not the seat's failure
        // (`nirvana_gate::Outcome::BLOCKED`). The work order listed blocked as a failure; that is reported, not made.
        "pass" | "failed" | "sent_back" | "blocked" | "reseated" => {
            crate::nirvana_gate::Outcome::tally(word)
        }
        _ => None,
    }
}

pub fn tally(lines: &[RecordLine]) -> BTreeMap<RecordKey, Tally> {
    let mut tallies: BTreeMap<RecordKey, Tally> = BTreeMap::new();
    // Several lines for one order and run: the last one decides (a later line is a later verdict on the same work).
    let mut last: BTreeMap<(&str, Option<&str>), &RecordLine> = BTreeMap::new();
    for line in lines {
        last.insert((line.order.as_str(), line.run.as_deref()), line);
    }
    let decided: Vec<&RecordLine> = lines
        .iter()
        .filter(|line| {
            last.get(&(line.order.as_str(), line.run.as_deref()))
                .is_some_and(|kept| std::ptr::eq(*kept, *line))
        })
        .collect();
    for line in decided {
        let Some(chain) = &line.chain else {
            continue;
        };
        // O4b's worker subagents (chain `subagent`, outcome `subagent-<status>`): the orchestrator judged them, not
        // the gate, so they count toward no seat's tally, and no "subagent" row of failures builds up either.
        if chain == crate::nirvana_claude::SUBAGENT_CHAIN {
            continue;
        }
        let Some(passed) = tally_word(&line.outcome) else {
            continue;
        };
        let entry = tallies
            .entry((line.seat.clone(), line.effort, chain.clone()))
            .or_default();
        entry.orders += 1;
        if passed {
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

/// A line for the same order and run is already in the record.
pub fn recorded(lines: &[RecordLine], order: &str, run: Option<&str>) -> bool {
    lines
        .iter()
        .any(|line| line.order == order && line.run.as_deref() == run)
}

/// Fills what the line does not say from the order's booking: the pick's pool, reason, override, rule pick,
/// mode, size, bar drop, step down, use basis and expected use, and `seated_at` from `booked_at`.
pub fn fill_from_lease(line: &mut RecordLine, lease: &Lease) {
    let pick = &lease.pick;
    let text = |key: &str| {
        pick.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let flag = |key: &str| pick.get(key).and_then(serde_json::Value::as_bool);
    line.pool.get_or_insert_with(|| lease.pool.clone());
    if line.reason.is_none() {
        line.reason = text("reason");
    }
    if line.override_reason.is_none() {
        line.override_reason = text("override");
    }
    if line.rule_pick.is_none() {
        line.rule_pick = text("rule_pick");
    }
    if line.mode.is_none() {
        line.mode = pick
            .get("mode")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
    }
    if line.size.is_none() {
        line.size = pick
            .get("size")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
    }
    if line.bar_dropped.is_none() {
        line.bar_dropped = flag("bar_dropped");
    }
    if line.step_down.is_none() {
        line.step_down = flag("step_down");
    }
    if line.use_basis.is_none() {
        line.use_basis = text("use_basis");
    }
    if line.expected.is_none() && !lease.expected.is_empty() {
        line.expected = Some(lease.expected.clone());
    }
    line.seated_at.get_or_insert(lease.booked_at);
}

/// Records an order once (O2 review S5: a retry appended twice). Under `leases.lock`: refuse an order and run
/// already recorded, fill the line from its live booking, append it, then release the booking when `release`.
/// `Ok(false)` is the refusal.
pub fn record_once(
    dir: &Path,
    mut line: RecordLine,
    release: bool,
    now: u64,
) -> std::io::Result<(bool, RecordLine)> {
    let _lock = Lock::take(dir)?;
    let record = dir.join(RECORD_FILE);
    if recorded(&read_record(&record), &line.order, line.run.as_deref()) {
        return Ok((false, line));
    }
    let leases_path = dir.join(LEASES_FILE);
    // A leases.json that does not parse is left as it is (the O2 fix round: it holds each run's override
    // count), so its booking is neither read nor released; the order's outcome is still recorded.
    let mut leases = match load_leases(&leases_path) {
        Ok(leases) => Some(leases),
        Err(why) => {
            eprintln!("herdr seat record: {why}; the booking is not released");
            None
        }
    };
    if let Some(lease) = leases
        .as_ref()
        .and_then(|leases| leases.live(&line.order, now).cloned())
    {
        fill_from_lease(&mut line, &lease);
    }
    line.v.get_or_insert(1);
    line.herdr
        .get_or_insert_with(|| crate::nirvana::VERSION.to_owned());
    append_record(&record, &line)?;
    // A finished order's booking ends with it: its real use now shows in the pool's readings.
    if let Some(leases) = leases.as_mut().filter(|_| release) {
        if leases.release(&line.order, now) {
            write_leases(&leases_path, leases)?;
        }
    }
    Ok((true, line))
}

/// The record's lines; a line that does not parse is skipped.
pub fn read_record(path: &Path) -> Vec<RecordLine> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect()
}

/// Sum of expected use of argon orders booked or recorded in the last 7 days (plan 19 §8).
pub fn argon_record_used(record_lines: &[RecordLine], leases: &[Lease], now: u64) -> f32 {
    let cutoff = now.saturating_sub(7 * 86_400);
    let mut sum = 0.0_f32;
    let mut seen_orders = std::collections::HashSet::new();
    for line in record_lines {
        let is_argon = line.seat == "argon" || line.pool.as_deref() == Some("argon");
        let at = line.at.or(line.ended_at).or(line.seated_at).unwrap_or(0);
        if is_argon && at >= cutoff {
            seen_orders.insert(&line.order);
            let expected = line
                .expected
                .as_ref()
                .and_then(|exp| exp.get("week").copied())
                .or_else(|| line.spent.as_ref().and_then(|s| s.get("week").copied()))
                .unwrap_or(0.0);
            sum += expected;
        }
    }
    for lease in leases {
        let is_argon = lease.pool == "argon" || lease.seat == "argon";
        let at = lease.booked_at;
        if is_argon && at >= cutoff && !seen_orders.contains(&lease.order) {
            let expected = lease.expected.get("week").copied().unwrap_or(0.0);
            sum += expected;
        }
    }
    sum
}

/// Read the expected argon week use from herdr's record.jsonl and leases.json in dir() (plan 19 §8).
pub fn read_argon_from_herdr(now: u64) -> f32 {
    let dir = dir();
    let record_lines = read_record(&dir.join(RECORD_FILE));
    let leases = load_leases(&dir.join(LEASES_FILE)).unwrap_or_default();
    argon_record_used(&record_lines, &leases.leases, now)
}

/// Plan 20 §3.1: the orchestrator's seat, following the table's mode and climb.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrchestratorSeat {
    pub seat: String,
    pub effort: Effort,
    pub mode: Mode,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub climb: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    pub reason: String,
    pub line: String,
}

impl OrchestratorSeat {
    /// Format for the Seats square row: "Balanced, climb 1.18", "Efficient", "SOTA, Fable week 82% used".
    pub fn square_summary(&self) -> String {
        match (self.mode, self.climb) {
            (Mode::Balanced, Some(climb)) => format!("Balanced, climb {climb:.2}"),
            _ => self.reason.clone(),
        }
    }
}

/// What Nirvana Claude runs on when the table names nothing it can take: R1's Opus 5.5 at medium, the last resort
/// only (a table whose every claude seat is off). Every other answer comes from `[orchestrator]`.
const LAST_RESORT: (&str, Effort, &str) = ("opus", Effort::Medium, "claude-opus-5-5");

/// A choice as Nirvana Claude can take it: the seat on, at the effort it offers nearest at or below the one named
/// (else its lowest). Why not, in words, when the seat is missing or off.
fn take_choice<'a>(
    table: &'a Table,
    choice: &OrchestratorChoice,
) -> Result<(&'a Seat, Effort), String> {
    let seat = table
        .seat(&choice.seat)
        .ok_or_else(|| format!("no seat {}", choice.seat))?;
    if !seat.enabled {
        return Err(format!("{} off in seats.toml", seat.id));
    }
    let effort = seat
        .efforts
        .iter()
        .copied()
        .filter(|effort| *effort <= choice.effort)
        .max()
        .or_else(|| seat.efforts.iter().copied().min())
        .ok_or_else(|| format!("{} has no efforts", seat.id))?;
    Ok((seat, effort))
}

/// Plan 40a: a seat with its own window (Fable) seats Nirvana Claude only when every window it spends was read fresh
/// and its own keeps `fable_orchestrator_left` after what is booked. The reason when not: "Fable week 82% used",
/// "Fable week not read". A seat without its own window passes: plan 20's rule never asked that of Opus or Sonnet.
fn scoped_orchestrator_ok(table: &Table, world: &World, seat: &Seat) -> Result<(), String> {
    if seat.extra_windows.is_empty() {
        return Ok(());
    }
    let Some(pool) = table.pool(&seat.pool) else {
        return Err(format!("no pool {}", seat.pool));
    };
    let windows = seat_windows(seat, pool);
    for window in windows
        .iter()
        .filter(|w| seat.extra_windows.contains(w))
        .chain(windows.iter().filter(|w| !seat.extra_windows.contains(w)))
    {
        if let Some(why) = unread_why(world, &seat.pool, window) {
            return Err(why);
        }
    }
    for window in &seat.extra_windows {
        let used = world
            .readings
            .iter()
            .find(|r| r.pool == seat.pool && &r.window == window)
            .and_then(|r| r.used)
            .unwrap_or(1.0);
        let booked: f32 = world
            .leases
            .iter()
            .filter(|lease| lease.live(world.now) && lease.pool == seat.pool)
            .filter_map(|lease| lease.expected.get(window))
            .sum();
        if 1.0 - used - booked + EDGE < table.constants.fable_orchestrator_left {
            let booked = if booked > 0.0005 {
                format!(", {} booked", pct(booked))
            } else {
                String::new()
            };
            return Err(format!(
                "{} {} used{booked}",
                window_label(&seat.pool, window),
                pct(used)
            ));
        }
    }
    Ok(())
}

/// One constructor for every branch (review D6: five copies of the same block). `wanted` is the mode's choice, then
/// the choices it falls back to in order; the first Nirvana Claude can take wins, and each one passed over adds its
/// reason to `reason` ("SOTA, Fable week 82% used").
fn orchestrator_from(
    table: &Table,
    world: &World,
    mode: Mode,
    reason: String,
    wanted: &[&OrchestratorChoice],
    climb: Option<(f32, f32, String)>,
) -> OrchestratorSeat {
    let mut reason = reason;
    let mut passed: Vec<String> = Vec::new();
    let mut taken = None;
    for choice in wanted {
        match take_choice(table, choice).and_then(|(seat, effort)| {
            scoped_orchestrator_ok(table, world, seat).map(|()| (seat, effort))
        }) {
            Ok(got) => {
                taken = Some(got);
                break;
            }
            Err(why) => {
                if !passed.contains(&why) {
                    passed.push(why);
                }
            }
        }
    }
    // A table whose every named choice is off: any claude seat that is on and has no window of its own, else R1's.
    // Review a, N4: the reason says which of the two, so an owner who turned sonnet or opus off reads why the
    // orchestrator sits where it does.
    if taken.is_none() {
        let any_on = table
            .seats
            .iter()
            .any(|seat| seat.enabled && seat.cli == "claude" && seat.extra_windows.is_empty());
        passed.push(if any_on {
            "no named seat can be taken, so the first claude seat on".to_owned()
        } else {
            format!(
                "every claude seat is off, so {} {} as the last resort",
                LAST_RESORT.0,
                LAST_RESORT.1.word()
            )
        });
    }
    let (seat, effort, model) = match taken.or_else(|| {
        table
            .seats
            .iter()
            .filter(|seat| seat.enabled && seat.cli == "claude" && seat.extra_windows.is_empty())
            .find_map(|seat| {
                take_choice(table, &OrchestratorChoice::new(&seat.id, Effort::Medium)).ok()
            })
    }) {
        Some((seat, effort)) => (seat.id.clone(), effort, seat.model_for(effort)),
        None => (
            LAST_RESORT.0.to_owned(),
            LAST_RESORT.1,
            LAST_RESORT.2.to_owned(),
        ),
    };
    if !passed.is_empty() {
        reason = format!("{reason}, {}", passed.join(", "));
    }
    let line = format!("nirvana claude -> {seat} {}: {reason}", effort.word());
    let (climb, t, window) = match climb {
        Some((climb, t, window)) => (Some(climb), Some(t), Some(window)),
        None => (None, None, None),
    };
    OrchestratorSeat {
        seat,
        effort,
        mode,
        model,
        climb,
        t,
        window,
        reason,
        line,
    }
}

/// Plan 20 §3.1 and plan 40a: Nirvana Claude's seat from the table's mode, its `[orchestrator]` choices and the
/// readings. Balanced climbs on the pool of its `balanced` seat.
pub fn orchestrator_seat(table: &Table, world: &World) -> OrchestratorSeat {
    let o = &table.orchestrator;
    let mode = table.mode;
    match mode {
        Mode::Efficient => orchestrator_from(
            table,
            world,
            mode,
            "Efficient".into(),
            &[&o.efficient, &o.unread],
            None,
        ),
        Mode::Sota => orchestrator_from(
            table,
            world,
            mode,
            "SOTA".into(),
            &[&o.sota, &o.sota_fallback, &o.unread],
            None,
        ),
        Mode::Balanced => {
            let pool_name = table
                .seat(&o.balanced.seat)
                .map_or("max", |seat| seat.pool.as_str());
            let window_names: Vec<String> = table
                .pool(pool_name)
                .map(|p| p.windows.clone())
                .unwrap_or_else(|| vec!["5h".into(), "week".into()]);
            let pool_readings: Vec<&Reading> = world
                .readings
                .iter()
                .filter(|r| r.pool == pool_name && r.used.is_some())
                .collect();
            let fresh_readings: Vec<&Reading> = pool_readings
                .iter()
                .copied()
                .filter(|r| nirvana_usage::freshness(r, world.now) == Freshness::Fresh)
                .collect();
            let unread = |why: &str| {
                orchestrator_from(
                    table,
                    world,
                    mode,
                    format!("Balanced, readings {why}"),
                    &[&o.unread],
                    None,
                )
            };
            if pool_readings.is_empty() {
                return unread("missing");
            }
            if fresh_readings.is_empty() {
                return unread("stale");
            }
            let windows: Vec<WindowState> = window_names
                .iter()
                .map(|win_name| {
                    let fresh = fresh_readings.iter().find(|r| &r.window == win_name);
                    let booked: f32 = world
                        .leases
                        .iter()
                        .filter(|lease| lease.live(world.now) && lease.pool == pool_name)
                        .filter_map(|lease| lease.expected.get(win_name))
                        .fold(0.0, |sum, share| sum + share);
                    let used = fresh.and_then(|r| r.used);
                    WindowState {
                        window: win_name.clone(),
                        used,
                        booked,
                        left: used.map(|u| 1.0 - u - booked),
                        headroom: None,
                        resets_at: fresh.and_then(|r| r.resets_at),
                        elapsed: fresh.and_then(|r| nirvana_usage::elapsed(r)),
                        projected: None,
                        spare: None,
                        pace: None,
                        reset_factor: 1.0,
                        spare_weight: table.constants.spare_weight,
                        expected: 0.0,
                    }
                })
                .collect();
            let Some((climb, best_window)) = windows_climb(&windows, table.constants.pace_ease)
            else {
                return unread("missing");
            };
            let t = climb_t(
                climb,
                table.constants.climb_start,
                table.constants.climb_full,
            );
            let choice = if t < 0.5 {
                &o.balanced
            } else {
                &o.balanced_hot
            };
            orchestrator_from(
                table,
                world,
                mode,
                format!("Balanced, {pool_name} {best_window} climb {climb:.2}"),
                &[choice, &o.unread],
                Some((climb, t, best_window)),
            )
        }
    }
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr seat list [--json]
       herdr seat pick --class <class> [--size small|medium|large] [--needs a,b] [--not-family f]
                       [--mode efficient|balanced|sota] [--book <order>] [--run <id>] [--profile <name>]
                       [--override <seat> --reason \"...\"] [--json]
       herdr seat release <order>
       herdr seat record --json '<line>'
       herdr seat orchestrator [--json]
       herdr seat diff   (how your seats.toml differs from herdr's default)
       herdr seat mode [efficient|balanced|sota]   (prints the mode, or sets seats.toml's; plan 12 O8a)
       herdr seat run|exec|status|read|stop|cleanup ...   (plan 12 O3; `herdr seat run` alone prints its usage)
--size defaults to medium (seats.toml's [size.medium]); --mode to seats.toml's mode.
pick exits 0 with a seat, 3 with none (a refused override included), 2 on a usage or table error, a --needs
or --not-family word no seat has, or a leases.json that does not parse. pick and release exit 4 when another
herdr held the seat lock for 10 s.";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn now() -> u64 {
    nirvana_usage::unix(SystemTime::now())
}

/// The owner's table, or why it is refused. O3's runner reads it so; the CLI exits on a refusal.
pub fn table() -> Result<Table, String> {
    let path = dir().join(SEATS_FILE);
    parse(&read_or_create(&path)).map_err(|problems| {
        format!(
            "{} is refused (delete it to get herdr's default back):
{problems}",
            path.display()
        )
    })
}

/// The mode source: whether it came from the project's file or the machine's seats.toml (plan 23 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ModeSource {
    Project,
    Machine,
}

impl ModeSource {
    pub fn word(&self) -> &'static str {
        match self {
            Self::Project => "this project",
            Self::Machine => "machine",
        }
    }
}

/// Where a project's own seats.toml lives: <project>/.nirvana/seats.toml (plan 23 §2).
pub fn project_seats_path(root: &Path) -> PathBuf {
    root.join(crate::nirvana_run::DIR).join(SEATS_FILE)
}

/// Parses a project's <project>/.nirvana/seats.toml file (plan 23 §2).
/// Content: one key, `mode = "efficient" | "balanced" | "sota"`, and comments.
/// Any other key, or a bad value, refuses the file, naming the path and line. A file with no `mode` line (empty,
/// or its line commented out) gives `None`: the machine's mode, as with no file. Refusing it would stop every seat
/// pick in the project over a file that breaks neither rule (plan 23 review S2).
pub fn parse_project_seats(path: &Path, text: &str) -> Result<Option<Mode>, String> {
    // An editor's byte-order mark would read as part of the key.
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut found_mode: Option<(Mode, usize)> = None;
    for (idx, line) in text.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.starts_with('[') {
            return Err(format!(
                "{}: line {line_no}: tables not allowed in project seats.toml",
                path.display()
            ));
        }
        let Some((key_raw, val_raw)) = trimmed.split_once('=') else {
            return Err(format!(
                "{}: line {line_no}: expected `key = value`",
                path.display()
            ));
        };
        let key = key_raw.trim();
        if key != "mode" {
            return Err(format!(
                "{}: line {line_no}: unknown key `{key}` (only `mode` is allowed)",
                path.display()
            ));
        }
        if found_mode.is_some() {
            return Err(format!(
                "{}: line {line_no}: duplicate `mode` key",
                path.display()
            ));
        }
        let val_trimmed = val_raw.trim();
        let mode_str = if let Some(after_quote) = val_trimmed.strip_prefix('"') {
            let Some(end_quote) = after_quote.find('"') else {
                return Err(format!(
                    "{}: line {line_no}: unclosed string",
                    path.display()
                ));
            };
            let rest = after_quote[end_quote + 1..].trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return Err(format!(
                    "{}: line {line_no}: unexpected tokens after value",
                    path.display()
                ));
            }
            &after_quote[..end_quote]
        } else if let Some(after_quote) = val_trimmed.strip_prefix('\'') {
            let Some(end_quote) = after_quote.find('\'') else {
                return Err(format!(
                    "{}: line {line_no}: unclosed string",
                    path.display()
                ));
            };
            let rest = after_quote[end_quote + 1..].trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return Err(format!(
                    "{}: line {line_no}: unexpected tokens after value",
                    path.display()
                ));
            }
            &after_quote[..end_quote]
        } else {
            let unquoted = val_trimmed.split('#').next().unwrap_or("").trim();
            unquoted
        };
        let Some(mode) = Mode::parse(mode_str) else {
            return Err(format!(
                "{}: line {line_no}: bad mode value `{mode_str}` (expected \"efficient\", \"balanced\", or \"sota\")",
                path.display()
            ));
        };
        found_mode = Some((mode, line_no));
    }
    Ok(found_mode.map(|(mode, _)| mode))
}

/// Updates or creates the `mode = ...` line in a project seats file, preserving comments and formatting.
pub fn with_project_mode(text: &str, mode: Mode) -> Result<String, String> {
    let mut out = String::with_capacity(text.len() + 16);
    let mut done = false;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let trimmed = body.trim_start();
        let is_mode = trimmed
            .strip_prefix("mode")
            .is_some_and(|rest| rest.trim_start().starts_with('='));
        if is_mode && !done {
            let indent = &body[..body.len() - trimmed.len()];
            let after_val = if let Some((_, val_part)) = trimmed.split_once('=') {
                if let Some(hash_pos) = val_part.find('#') {
                    format!(" {}", &val_part[hash_pos..].trim_end())
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            out.push_str(&format!("{indent}mode = \"{}\"{after_val}", mode.word()));
            let ending = &line[body.len()..];
            out.push_str(ending);
            done = true;
        } else {
            out.push_str(line);
        }
    }
    if !done {
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("mode = \"{}\"\n", mode.word()));
    }
    Ok(out)
}

/// Sets the mode in <project>/.nirvana/seats.toml (plan 23 §2), keeping the file's comments, or creating it.
/// A refused file is never rewritten: it is the owner's to fix, and `herdr seat mode --clear` removes it. The caller
/// already knows the mode before (`project_mode`), so nothing is read here that could fail after the write.
pub fn set_project_mode(root: &Path, mode: Mode) -> Result<(), String> {
    let path = project_seats_path(root);
    let text = if path.exists() {
        let text =
            std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        parse_project_seats(&path, &text).map_err(|why| format!("{why}; {CLEAR_HINT}"))?;
        text
    } else {
        String::new()
    };
    let updated = with_project_mode(&text, mode)?;
    if updated == text {
        return Ok(());
    }
    parse_project_seats(&path, &updated)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    write_atomic(&path, updated.as_bytes()).map_err(|err| format!("{}: {err}", path.display()))
}

/// What to do about a refused project file.
pub const CLEAR_HINT: &str = "fix it, or `herdr seat mode --clear` removes it";

/// Determines the mode for a project (plan 23 §3):
/// 1. Project's file `<project>/.nirvana/seats.toml` if it exists;
/// 2. Else global `table.mode`.
pub fn project_mode(root: &Path, table: &Table) -> Result<(Mode, ModeSource), String> {
    let path = project_seats_path(root);
    if !path.exists() {
        return Ok((table.mode, ModeSource::Machine));
    }
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    match parse_project_seats(&path, &text)? {
        Some(mode) => Ok((mode, ModeSource::Project)),
        // Empty, or comments only: the machine's mode, as with no file.
        None => Ok((table.mode, ModeSource::Machine)),
    }
}

/// Returns the global table with its mode replaced by the project's mode (plan 23 §3).
pub fn table_for(root: &Path) -> Result<Table, String> {
    let mut table = table()?;
    let (mode, _source) = project_mode(root, &table)?;
    table.mode = mode;
    Ok(table)
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

#[cfg(test)]
thread_local! {
    static TEST_CLIS: std::cell::RefCell<Option<BTreeMap<String, String>>> =
        const { std::cell::RefCell::new(None) };
}

/// A test's stand-in for `live_world`: no usage, and these CLIs refused with their reasons, every other one signed
/// in. Plan 29's review round drives `book`, the reseat and the send-back, and the live reads run `claude auth status`
/// and `claude -p /usage`.
#[cfg(test)]
pub fn set_test_world(clis: Option<BTreeMap<String, String>>) {
    TEST_CLIS.with_borrow_mut(|slot| *slot = clis);
}

/// Why a CLI cannot take an order, from the Accounts readers; `None` when it is signed in. agy's is read from its newest
/// log only (review B2: the gate is a spend decision and fails closed), and each answer is kept `SIGNIN_KEEP_S`
/// (review T2: `claude auth status` cost a process per pick).
fn cli_state(cli: &str) -> Option<String> {
    use crate::nirvana_accounts::{read_for_gate, Cli};
    let which = match cli {
        "claude" => Cli::Claude,
        "agy" => Cli::Antigravity,
        "codex" => Cli::Codex,
        other => return Some(format!("{other} is not a CLI herdr knows")),
    };
    cached_cli_state(&dir(), cli, now(), || {
        signed_within(cli, ACCOUNTS_TIMEOUT, move || read_for_gate(which))
    })
}

/// Review T2: how long a sign-in answer is kept. A sign-in does not change in a minute, and a logout shows within one.
pub const SIGNIN_KEEP_S: u64 = 60;
/// `<home>/nirvana/signin.json`: each CLI's last gate answer and when it was read.
pub const SIGNIN_FILE: &str = "signin.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SigninCache {
    #[serde(default)]
    clis: BTreeMap<String, SigninEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SigninEntry {
    /// `None`: signed in.
    why: Option<String>,
    at: u64,
}

/// `read`'s answer for `cli`, or the one kept in `<dir>/signin.json` when it is under `SIGNIN_KEEP_S` old. A file that
/// does not parse is read as empty: it holds nothing but answers that can be read again.
fn cached_cli_state(
    dir: &Path,
    cli: &str,
    now: u64,
    read: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let path = dir.join(SIGNIN_FILE);
    let mut cache: SigninCache = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    if let Some(entry) = cache
        .clis
        .get(cli)
        .filter(|entry| entry.at <= now && now - entry.at < SIGNIN_KEEP_S)
    {
        return entry.why.clone();
    }
    let why = read();
    cache.clis.insert(
        cli.to_owned(),
        SigninEntry {
            why: why.clone(),
            at: now,
        },
    );
    if let Ok(bytes) = serde_json::to_vec_pretty(&cache) {
        let _ = write_atomic(&path, &bytes);
    }
    why
}

/// A sign-in read with a deadline. `claude auth status` runs with no deadline of its own at every pick
/// (`nirvana_accounts::output`), and a hang there hung `seat pick` (review S7). A read past the deadline is
/// left to finish on its own thread: herdr does not kill a process it cannot name from here.
fn signed_within(
    cli: &str,
    timeout: Duration,
    read: impl FnOnce() -> crate::nirvana_accounts::Signed + Send + 'static,
) -> Option<String> {
    use crate::nirvana_accounts::Signed;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(read());
    });
    match rx.recv_timeout(timeout) {
        Ok(Signed::In(_)) => None,
        Ok(Signed::NotInstalled) => Some(format!("{cli} not installed")),
        Ok(Signed::Out | Signed::Reading) => Some(format!("{cli} not signed in")),
        Err(_) => Some(format!("{cli} sign-in not read in {timeout:?}")),
    }
}

/// The `--needs` and `--not-family` words no seat in the table has, each in a sentence. A typo excluded
/// nothing (`--not-family antropic`) or skipped every seat (`--needs vidoe`), silently (review nit).
pub fn unknown_words(table: &Table, needs: &[String], families: &[String]) -> Vec<String> {
    let mut capabilities: Vec<&str> = table
        .seats
        .iter()
        .flat_map(|seat| seat.capabilities.iter().map(String::as_str))
        .collect();
    capabilities.sort_unstable();
    capabilities.dedup();
    let mut known: Vec<&str> = table
        .seats
        .iter()
        .map(|seat| seat.family.as_str())
        .collect();
    known.sort_unstable();
    known.dedup();
    let mut words = Vec::new();
    for need in needs
        .iter()
        .filter(|need| !capabilities.contains(&need.as_str()))
    {
        words.push(format!(
            "--needs {need}: no seat has it; the capabilities: {}",
            capabilities.join(", ")
        ));
    }
    for family in families
        .iter()
        .filter(|family| !known.contains(&family.as_str()))
    {
        words.push(format!(
            "--not-family {family}: no seat is of it; the families: {}",
            known.join(", ")
        ));
    }
    words
}

/// `Lock::take` for the CLI: another herdr holding the lock all the while is its own exit, `LOCK_EXIT`.
fn take_lock(dir: &Path) -> std::io::Result<Lock> {
    match Lock::take(dir) {
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {
            eprintln!("herdr seat: {err}; nothing was changed, try again");
            std::process::exit(LOCK_EXIT);
        }
        other => other,
    }
}

/// `load_leases` for the CLI: a file that does not parse is a table-class error, exit 2, never read as empty.
fn leases_or_exit(path: &Path) -> Leases {
    load_leases(path).unwrap_or_else(|err| {
        eprintln!("herdr seat: {err}");
        std::process::exit(2);
    })
}

/// The world from `<dir>/usage.json` and `<dir>/leases.json` alone, for the orchestrator's seat at launch, in the
/// prompt notice and in the Seats square (plan 20 §3). `live_world` re-reads a stale source by running
/// `claude /usage` and `agy /usage`: in a `UserPromptSubmit` hook that stalls every prompt after an idle gap, and a
/// test with no cache ran the real CLIs on the owner's sign-in (review of the p20 build, 2026-10-20). A stale
/// reading here is stale, which the rule turns into opus medium.
pub fn cached_world(dir: &Path, now: u64) -> World {
    let cache = nirvana_usage::read_cache(&dir.join(nirvana_usage::CACHE_FILE));
    World {
        now,
        readings: cache
            .sources
            .values()
            .flat_map(|source| source.readings.iter().cloned())
            .collect(),
        leases: load_leases(&dir.join(LEASES_FILE))
            .unwrap_or_default()
            .leases,
        ..World::default()
    }
}

/// The world as this machine has it: usage from `usage.json` (stale readers read first, as `herdr usage`
/// does), the accounts, and the record when the table blends it.
pub fn live_world(table: &Table) -> World {
    #[cfg(test)]
    if let Some(clis) = TEST_CLIS.with_borrow(Clone::clone) {
        return World {
            now: now(),
            clis,
            ..World::default()
        };
    }
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
        Some("orchestrator") => orchestrator_cli(&args[1..]),
        Some("diff") => diff_cli(&args[1..]),
        // Plan 12 O8a: the mode the Seats square shows and changes.
        Some("mode") => crate::nirvana_seats_square::mode_cli(&args[1..]),
        // Plan 12 O3: the runner's verbs live in `nirvana_worker`.
        Some("run" | "exec" | "status" | "read" | "stop" | "cleanup") => {
            crate::nirvana_worker::cli(args)
        }
        _ => usage_exit(),
    }
}

fn orchestrator_cli(args: &[String]) -> std::io::Result<()> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => usage_exit(),
    };
    let project = crate::nirvana_run::Project::from_env();
    let table = match table_for(&project.root) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("herdr seat orchestrator: {err}");
            std::process::exit(2);
        }
    };
    let mut world = live_world(&table);
    let dir = dir();
    world.leases = leases_or_exit(&dir.join(LEASES_FILE)).leases;
    let seat = orchestrator_seat(&table, &world);
    if json {
        println!("{}", serde_json::to_string_pretty(&seat)?);
    } else {
        println!("{}", seat.line);
    }
    Ok(())
}

/// Review M2: `herdr seat diff` prints how the owner's `seats.toml` differs from herdr's default.
fn diff_cli(args: &[String]) -> std::io::Result<()> {
    if !args.is_empty() {
        usage_exit();
    }
    let path = dir().join(SEATS_FILE);
    let text = read_or_create(&path);
    print!("{}", diff_to_default(&text, &path.display().to_string()));
    std::io::stdout().flush()
}

fn list_cli(args: &[String]) -> std::io::Result<()> {
    let json = match args {
        [] => false,
        [flag] if flag == "--json" => true,
        _ => usage_exit(),
    };
    let project = crate::nirvana_run::Project::from_env();
    let table = match table_for(&project.root) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("herdr seat list: {err}");
            std::process::exit(2);
        }
    };
    let mut world = live_world(&table);
    let dir = dir();
    world.leases = leases_or_exit(&dir.join(LEASES_FILE)).leases;
    let mut view = list_view(&table, &world);
    let differs =
        std::fs::read_to_string(dir.join(SEATS_FILE)).is_ok_and(|text| differs_from_default(&text));
    view["differs_from_default"] = differs.into();
    if json {
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        let mut text = list_text(&table, &world, &dir);
        if differs {
            text.push_str(DIFFERS_LINE);
            text.push('\n');
        }
        print!("{text}");
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
                    let (elapsed, projected, spare) = reading
                        .map(|r| projected_spare(r, booked, constants.pace_ease))
                        .unwrap_or((None, None, None));
                    WindowState {
                        window: name,
                        used,
                        booked,
                        left,
                        headroom: left.map(|left| left - reserve),
                        resets_at: reading.and_then(|r| r.resets_at),
                        elapsed,
                        projected,
                        spare,
                        pace: reading.and_then(|r| {
                            nirvana_usage::pace_with(
                                r,
                                constants.pace_ease,
                                constants.pace_min,
                                constants.pace_max,
                            )
                        }),
                        reset_factor: 1.0,
                        spare_weight: constants.spare_weight,
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

/// `herdr seat pick`'s flags, read over the project's table.
#[derive(Debug)]
pub(crate) struct PickArgs {
    pub request: Request,
    /// `--needs` as given, the profile's joined, before plan 29 routes the servers and bladebro out of `request`:
    /// what the booking keeps for `seat run`'s sources line (gsrc review S1).
    pub needs: Vec<String>,
    pub json: bool,
    pub book: Option<String>,
    pub run: Option<String>,
    pub over: Option<String>,
    pub reason: Option<String>,
}

/// The table `herdr seat pick` reads in the project at `root`, and its flags over it: the project's mode stands
/// unless `--mode` is given (plan 23 §3).
pub(crate) fn pick_args_for(root: &Path, args: &[String]) -> Result<(Table, PickArgs), String> {
    let table = table_for(root)?;
    let mut picked = PickArgs {
        request: default_request(&table),
        needs: Vec::new(),
        json: false,
        book: None,
        run: None,
        over: None,
        reason: None,
    };
    let request = &mut picked.request;
    let mut profile: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--profile" => profile = Some(value()),
            "--class" => request.class = value(),
            "--size" => request.size = SizeName::parse(&value()).unwrap_or_else(|| usage_exit()),
            "--needs" => request.needs = list_arg(&value()),
            "--not-family" => request.not_family = list_arg(&value()),
            "--mode" => request.mode = Mode::parse(&value()).unwrap_or_else(|| usage_exit()),
            "--book" => picked.book = Some(value()),
            "--run" => picked.run = Some(value()),
            "--override" => picked.over = Some(value()),
            "--reason" => picked.reason = Some(value()),
            "--json" => picked.json = true,
            _ => usage_exit(),
        }
    }
    // Plan 33 §2.2: the profile's class when the flags name none, and its needs joined before plan 29's routing.
    let profile = profile
        .map(|name| crate::nirvana_profiles::load(root, &name))
        .transpose()?;
    if let Some(loaded) = &profile {
        if picked.request.class.is_empty() {
            picked.request.class = loaded.profile.class.clone().unwrap_or_default();
        }
        picked.request.needs =
            crate::nirvana_profiles::joined_needs(&picked.request.needs, &loaded.profile);
    }
    picked.needs = picked.request.needs.clone();
    // Plan 29 §5.1: a need naming one of the project's MCP servers is no seat capability; the order takes a claude seat.
    let needs = crate::nirvana_gui_tools::seat_needs(&table, root, &picked.request.needs)?;
    (picked.request.needs, picked.request.claude_only) = (needs.rest, needs.claude_only);
    if let Some(loaded) = &profile {
        crate::nirvana_profiles::narrow(&mut picked.request, &loaded.profile);
    }
    Ok((table, picked))
}

fn pick_cli(args: &[String]) -> std::io::Result<()> {
    let project = crate::nirvana_run::Project::from_env();
    let (
        table,
        PickArgs {
            mut request,
            needs,
            json,
            book,
            run,
            over,
            reason,
        },
    ) = match pick_args_for(&project.root, args) {
        Ok(picked) => picked,
        Err(err) => {
            eprintln!("herdr seat pick: {err}");
            std::process::exit(2);
        }
    };
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
    let words = unknown_words(&table, &request.needs, &request.not_family);
    if !words.is_empty() {
        for word in words {
            eprintln!("herdr seat pick: {word}");
        }
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
    let world = live_world(&table);
    let dir = dir();
    // Plan 12 O8b (§9 role 3): Jev's advice over the seats left, in shadow unless `jev.toml` says on.
    let client = crate::nirvana_jev::live_client().map(|(client, _)| client);
    let setting = crate::nirvana_jev_roles::settings_in(&dir).seating;
    let outcome = pick_and_book(
        &dir,
        &table,
        world,
        &request,
        book.as_deref(),
        run.as_deref(),
        needs,
        |world, chosen| {
            crate::nirvana_jev_roles::advise_seating(
                client.as_ref(),
                &dir,
                setting,
                &table,
                world,
                &request,
                chosen,
                true,
            )
        },
    );
    match outcome {
        Ok(Booking::Seat(out)) => print_pick(&out, json),
        Ok(Booking::None(no_seat, now)) => no_seat_exit(&no_seat, now, json),
        Err(BookError::Lock(err)) => {
            eprintln!("herdr seat: {err}; nothing was changed, try again");
            std::process::exit(LOCK_EXIT);
        }
        Err(BookError::Leases(err)) => {
            eprintln!("herdr seat: {err}");
            std::process::exit(2);
        }
        Err(BookError::Io(err)) => Err(err),
    }
}

/// What `pick_and_book` gave: the pick to print (the booking in it), or no seat at that time.
pub(crate) enum Booking {
    Seat(serde_json::Value),
    None(NoSeat, u64),
}

pub(crate) enum BookError {
    /// Another herdr held `leases.lock` for all of `Lock::take`.
    Lock(std::io::Error),
    /// `leases.json` does not parse: a table-class error, never read as empty.
    Leases(String),
    Io(std::io::Error),
}

impl From<std::io::Error> for BookError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for BookError {
    fn from(err: serde_json::Error) -> Self {
        Self::Io(err.into())
    }
}

/// Whether two picks were made among the same seats: the same seat and effort won, out of the same contenders.
fn same_contest(a: &Pick, b: &Pick) -> bool {
    let line = |pick: &Pick| -> Vec<(String, Effort)> {
        pick.contenders
            .iter()
            .map(|c| (c.seat.clone(), c.effort))
            .collect()
    };
    a.seat == b.seat && a.effort == b.effort && line(a) == line(b)
}

/// Review B1: `seat pick` held `leases.lock` across Jev's network call (20 s and a retry), so three parallel picks and
/// a worker's `seat record` timed out at the lock's 10 s. Now Jev is asked first, on a pick from the bookings read
/// without the lock; under the lock the rule picks again on the bookings as they are, and the advice is kept only when
/// that pick was made among the same seats. Otherwise the rule's new pick stands and says why. The lock is held for the
/// read, the pick and the write alone.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pick_and_book(
    dir: &Path,
    table: &Table,
    mut world: World,
    request: &Request,
    book: Option<&str>,
    run: Option<&str>,
    needs: Vec<String>,
    advise: impl FnOnce(&World, Pick) -> Pick,
) -> Result<Booking, BookError> {
    let leases_path = dir.join(LEASES_FILE);
    let over = request
        .override_seat
        .as_ref()
        .map(|(seat, _)| seat.as_str());
    // Before the lock: the bookings as they stand, read only, for Jev's question.
    let before = load_leases(&leases_path).map_err(BookError::Leases)?;
    world.now = now();
    let booked_already =
        book.is_some_and(|order| existing_booking(&before, order, over, world.now).is_some());
    let advised = if booked_already {
        None
    } else {
        world.leases = before.leases.clone();
        world.overrides_used = run.map_or(0, |run| before.overrides(run));
        pick(table, &world, request)
            .ok()
            .map(|rule| (rule.clone(), advise(&world, rule)))
    };
    let lock = Lock::take(dir).map_err(|err| match err.kind() {
        std::io::ErrorKind::TimedOut => BookError::Lock(err),
        _ => BookError::Io(err),
    })?;
    let mut leases = load_leases(&leases_path).map_err(BookError::Leases)?;
    world.now = now();
    if let Some(order) = book {
        match existing_booking(&leases, order, over, world.now).map(|found| found.cloned()) {
            Some(Ok(known)) => {
                // Plan 40a fix S1: the stored pick is answered again only while its seat may still spend. A Fable
                // booking whose week went stale or ran down since is released, under this lock, and refused.
                world.leases = leases.leases.clone();
                if let Err(why) = may_spend_now(
                    table,
                    &world,
                    &known.seat,
                    known.effort,
                    &known.class,
                    lease_size(&known),
                    order,
                ) {
                    leases.release(order, world.now);
                    write_leases(&leases_path, &leases)?;
                    drop(lock);
                    let refused = NoSeat {
                        class: request.class.clone(),
                        reason: spend_refused(order, &why),
                        earliest_reset: None,
                        defer: false,
                        skipped: Vec::new(),
                    };
                    return Ok(Booking::None(refused, world.now));
                }
                let known = &known;
                let mut out = known.pick.clone();
                out["booked"] = "existing".into();
                out["lease"] = lease_view(known)?;
                drop(lock);
                return Ok(Booking::Seat(out));
            }
            Some(Err(reason)) => {
                drop(lock);
                let refused = NoSeat {
                    class: request.class.clone(),
                    reason,
                    earliest_reset: None,
                    defer: false,
                    skipped: Vec::new(),
                };
                return Ok(Booking::None(refused, world.now));
            }
            None => {}
        }
    }
    world.leases = leases.leases.clone();
    world.overrides_used = run.map_or(0, |run| leases.overrides(run));
    match pick(table, &world, request) {
        Ok(mut chosen) => {
            chosen = match advised {
                Some((rule, advised)) if advised.jev_pick.is_some() => {
                    if same_contest(&rule, &chosen) {
                        advised
                    } else {
                        chosen.reason.push_str(
                            "; Jev's advice dropped: the bookings moved while Jev was asked",
                        );
                        chosen
                    }
                }
                _ => chosen,
            };
            let chosen = Pick { needs, ..chosen };
            let mut out = serde_json::to_value(&chosen)?;
            let (booked, changed) = book_pick(&mut leases, table, &chosen, book, run, world.now);
            if let Some(booked) = &booked {
                out["booked"] = "new".into();
                out["lease"] = lease_view(booked)?;
            }
            if changed {
                write_leases(&leases_path, &leases)?;
            }
            drop(lock);
            Ok(Booking::Seat(out))
        }
        Err(no_seat) => {
            drop(lock);
            Ok(Booking::None(no_seat, world.now))
        }
    }
}

/// Prints a NoSeat and exits `NO_SEAT_EXIT`.
fn no_seat_exit(no_seat: &NoSeat, now: u64, json: bool) -> std::io::Result<()> {
    let mut out = serde_json::to_value(no_seat)?;
    out["no_seat"] = true.into();
    if json {
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("no seat: {}", no_seat.reason);
        if let Some(at) = no_seat.earliest_reset {
            println!(
                "earliest reset in {}{}",
                nirvana_usage::short_duration(at.saturating_sub(now)),
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
    let _lock = take_lock(&dir)?;
    let mut leases = leases_or_exit(&path);
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
    let (written, line) = record_once(&dir(), line, true, now())?;
    if !written {
        eprintln!(
            "herdr seat record: {} is already recorded{}",
            line.order,
            line.run
                .as_deref()
                .map(|run| format!(" for run {run}"))
                .unwrap_or_default()
        );
        std::process::exit(2);
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
        // Plan 12 §4: seven seats in the first version; plan 40a turned Fable 5.2 on; plan 70 put Carbon in Argon's
        // place and turned Argon off; plan 71 put Hydrogen in Flash's place and turned Flash off; gpt-oss and local
        // stay off.
        let on: Vec<&str> = table
            .seats
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(
            on,
            ["opus", "sonnet", "haiku", "fable", "g-opus", "g-sonnet", "carbon", "hydrogen"]
        );
        let off: Vec<&str> = table
            .seats
            .iter()
            .filter(|s| !s.enabled)
            .map(|s| s.id.as_str())
            .collect();
        assert_eq!(off, ["argon", "flash", "gpt-oss", "local"]);
        // Plan 40a: Fable 5.2's seat, as the Claude docs gave it on 2026-11-09.
        let fable = table.seat("fable").unwrap();
        assert_eq!(fable.model, "claude-fable-5-2");
        assert_eq!(
            fable.efforts,
            [Effort::Low, Effort::Medium, Effort::High, Effort::Xhigh]
        );
        assert_eq!(fable.extra_windows, ["week.fable"]);
        assert!(fable.capabilities.contains(&"browser".to_owned()));
        let price = fable.price_at(date("2026-11-09").unwrap()).unwrap();
        assert_eq!(
            (
                price.input,
                price.output,
                price.cache_read,
                price.cache_write
            ),
            (8.0, 40.0, 0.20, 10.0)
        );
        assert_eq!(fable.prices.len(), 2, "the old row is kept");
        let sonnet = table.seat("sonnet").unwrap();
        assert!(
            (sonnet
                .price_at(date("2026-11-09").unwrap())
                .unwrap()
                .cache_read
                - 0.10)
                .abs()
                < 1e-9
        );
        assert!(
            (sonnet
                .price_at(date("2026-10-20").unwrap())
                .unwrap()
                .cache_read
                - 0.20)
                .abs()
                < 1e-9
        );
        // The review round: Fable's week and the 5 h window each measured once on 2026-11-09 (the pool's basis).
        assert!((table.pool("max").unwrap().per_usd["week.fable"] - 0.005).abs() < 1e-9);
        for cell in &fable.uses {
            assert_eq!(
                cell.windows.len(),
                3,
                "{:?}: 5h, week, week.fable",
                cell.effort
            );
        }
        let c = &table.constants;
        assert!(close(c.fable_reserve, 0.10) && close(c.fable_orchestrator_left, 0.25));
        assert_eq!(
            (c.subagent_default.as_str(), c.autocompact_tokens),
            ("sonnet", 0)
        );
        assert_eq!(
            table.orchestrator.sota,
            OrchestratorChoice::new("fable", Effort::High)
        );
        assert_eq!(
            table.orchestrator.sota_fallback,
            OrchestratorChoice::new("opus", Effort::High)
        );
        // A table without the section, or the constants, seats Nirvana Claude as before plan 40a.
        let before = parse(include_str!("nirvana_seats_v12.toml")).unwrap();
        assert_eq!(before.orchestrator, OrchestratorTable::default());
        assert_eq!(
            before.orchestrator.sota,
            OrchestratorChoice::new("opus", Effort::High)
        );
        assert_eq!(before.constants.autocompact_tokens, 0);
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
                // Plan 40c: `herdr look`'s class, in the media chain.
                "look.image",
                "sweep.mechanical",
                "chore.memory",
                "chore.script",
                "chore.github",
                "lesson-review",
                "profile-teach"
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
        assert!(close(c.spare_weight, 0.05));
        assert!(close(c.reset_left, 0.30) && c.reset_within_min == 60);
        assert!(close(c.reset_discount, 0.5) && c.defer_min == 30);
        assert_eq!(c.overrides_per_run, 3);
        assert!(!table.use_record, "the blend ships off");
        let max_pool = table.pool("max").unwrap();
        assert!((max_pool.per_usd["week"] - 0.00078).abs() < 1e-7);
        assert!(max_pool
            .basis
            .starts_with("derived: week of 2026-10-16, $796 at list price for 62%"));
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
        // A table from before plan 14 has no spare_weight line: it parses, with the default.
        let before_14 = parse(&DEFAULT.replace("spare_weight = 0.05\n", "")).unwrap();
        assert!(close(before_14.constants.spare_weight, 0.05));
        assert!(
            parse(&DEFAULT.replace("spare_weight = 0.05", "spare_weight = 1.5"))
                .unwrap_err()
                .contains("constants.spare_weight 1.5 is not between 0 and 1")
        );
        assert!(parse(&DEFAULT.replace(
            "bars = { efficient = 70, balanced = 85, sota = 100 }\nwhy = \"plan 19 §4",
            "bars = { efficient = 90, balanced = 85, sota = 100 }\nwhy = \"plan 19 §4"
        ))
        .unwrap_err()
        .contains("class build.bounded: bars must rise"));
        assert!(parse(&DEFAULT.replace(
            "  { seat = \"sonnet\", score = 85 },\n  { seat = \"opus\", score = 88 },",
            "  { seat = \"sonnet\", score = 85 },\n  { seat = \"sonnet\", score = 80 },\n  { seat = \"opus\", score = 88 },"
        ))
        .unwrap_err()
        .contains("class research.read: seat sonnet is listed twice"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unedited_seats_toml_upgrades_itself_preserving_mode_and_backs_up_old_default() {
        let dir = temp_dir("upgrade-seats");
        let path = dir.join(SEATS_FILE);
        let backup = dir.join("backup").join(SEATS_FILE);

        // 1. Install historical default v1 (from 0.13.0, 80ec1044), with CRLF and mode = "efficient"
        let v1_with_mode = replace_mode_line(HISTORICAL_DEFAULTS[0], "mode = \"efficient\"");
        let v1_crlf = v1_with_mode.replace('\n', "\r\n");
        std::fs::write(&path, &v1_crlf).unwrap();

        // 2. read_or_create upgrades to current DEFAULT, putting mode = "efficient" back, and backs up v1
        let text = read_or_create(&path);
        let parsed = parse(&text).expect("upgraded seats.toml parses");
        assert_eq!(
            parsed.mode,
            Mode::Efficient,
            "user's mode setting is preserved"
        );
        assert!(
            text.contains("spare_weight = 0.05"),
            "upgraded seats.toml contains new plan 14 settings"
        );
        let current_efficient = replace_mode_line(DEFAULT, "mode = \"efficient\"");
        assert_eq!(
            text, current_efficient,
            "matches current DEFAULT with mode put back"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            current_efficient,
            "on-disk file is upgraded"
        );
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            v1_crlf,
            "backup holds the exact old installed file"
        );

        // 3. Second read_or_create is a no-op
        assert_eq!(read_or_create(&path), current_efficient);

        // 4. An edited file (e.g. customized floor) is kept as it is, no backup overwritten
        let edited = current_efficient.replace("floor = 0.05", "floor = 0.08");
        std::fs::write(&path, &edited).unwrap();
        assert_eq!(read_or_create(&path), edited);

        // 5. Unparseable file is kept as it is
        std::fs::write(&path, "invalid = toml ][\n").unwrap();
        assert_eq!(read_or_create(&path), "invalid = toml ][\n");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn l7_google_family_enabled_seats_are_flash_and_argon_only() {
        let table = table();
        let mut google_enabled: Vec<&str> = table
            .seats
            .iter()
            .filter(|s| s.enabled && s.family == "google")
            .map(|s| s.id.as_str())
            .collect();
        google_enabled.sort();
        // Plan 70: Carbon took Argon's place. Plan 71: Hydrogen took Flash's.
        assert_eq!(google_enabled, ["carbon", "hydrogen"]);
    }

    #[test]
    fn section2_shipped_table_per_usd_and_use_cells_match_plan19() {
        let table = table();
        let gemini = table.pool("gemini").unwrap();
        assert!(close(gemini.per_usd["week"] as f32, 0.00107));
        assert!(close(gemini.per_usd["5h"] as f32, 0.0021));

        let google3p = table.pool("google3p").unwrap();
        assert!(close(google3p.per_usd["week"] as f32, 0.166));
        assert!(close(google3p.per_usd["5h"] as f32, 0.31));

        let max = table.pool("max").unwrap();
        assert!(close(max.per_usd["week"] as f32, 0.00078));
        // Plan 40a's review round measured the 5 h factor once (0.0073, the pool's basis); plan 19's was a prior.
        assert!(close(max.per_usd["5h"] as f32, 0.0073));

        let flash = table.seat("flash").unwrap();
        let cell_low = flash.uses.iter().find(|u| u.effort == Effort::Low).unwrap();
        // The review round refit low and medium to the record (`nirvana/seat_fit.py`: x8.97 n 12, x2.73 n 17).
        assert!(close(cell_low.windows["5h"], 0.0001563));
        assert!(close(cell_low.windows["week"], 7.815e-05));
        let cell_med = flash
            .uses
            .iter()
            .find(|u| u.effort == Effort::Medium)
            .unwrap();
        assert!(close(cell_med.windows["5h"], 0.0004541));
        assert!(close(cell_med.windows["week"], 0.000227));
        let cell_high = flash
            .uses
            .iter()
            .find(|u| u.effort == Effort::High)
            .unwrap();
        assert!(close(cell_high.windows["5h"], 0.00026928));
        assert!(close(cell_high.windows["week"], 0.00013464));

        let g_opus = table.seat("g-opus").unwrap();
        assert!(g_opus.fallback);
        let cell_low = g_opus
            .uses
            .iter()
            .find(|u| u.effort == Effort::Low)
            .unwrap();
        assert!(close(cell_low.windows["5h"], 0.006624));
        assert!(close(cell_low.windows["week"], 0.003536));
        let cell_med = g_opus
            .uses
            .iter()
            .find(|u| u.effort == Effort::Medium)
            .unwrap();
        assert!(close(cell_med.windows["5h"], 0.010971));
        assert!(close(cell_med.windows["week"], 0.0058565));
        let cell_high = g_opus
            .uses
            .iter()
            .find(|u| u.effort == Effort::High)
            .unwrap();
        assert!(close(cell_high.windows["5h"], 0.017595));
        assert!(close(cell_high.windows["week"], 0.009282));

        let g_sonnet = table.seat("g-sonnet").unwrap();
        assert!(g_sonnet.fallback);
        let cell_low = g_sonnet
            .uses
            .iter()
            .find(|u| u.effort == Effort::Low)
            .unwrap();
        assert!(close(cell_low.windows["5h"], 0.0041193));
        assert!(close(cell_low.windows["week"], 0.00221));
        let cell_med = g_sonnet
            .uses
            .iter()
            .find(|u| u.effort == Effort::Medium)
            .unwrap();
        assert!(close(cell_med.windows["5h"], 0.0068517));
        assert!(close(cell_med.windows["week"], 0.0036686));
        let cell_high = g_sonnet
            .uses
            .iter()
            .find(|u| u.effort == Effort::High)
            .unwrap();
        assert!(close(cell_high.windows["5h"], 0.010971));
        assert!(close(cell_high.windows["week"], 0.0058786));

        assert!(close(table.constants.climb_start, 0.70));
        assert!(close(table.constants.climb_full, 1.00));
    }

    #[test]
    fn section10_v3_upgrade_keeps_efficient_mode_and_edited_does_not() {
        let dir = temp_dir("upgrade-v3");
        let path = dir.join(SEATS_FILE);
        let backup = dir.join("backup").join(SEATS_FILE);

        let v3_with_mode = replace_mode_line(HISTORICAL_DEFAULTS[2], "mode = \"efficient\"");
        std::fs::write(&path, &v3_with_mode).unwrap();

        let text = read_or_create(&path);
        let parsed = parse(&text).expect("upgraded seats.toml parses");
        assert_eq!(parsed.mode, Mode::Efficient, "mode = efficient preserved");
        let current_efficient = replace_mode_line(DEFAULT, "mode = \"efficient\"");
        assert_eq!(text, current_efficient);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), current_efficient);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), v3_with_mode);

        // An edited file is never upgraded
        let edited = current_efficient.replace("floor = 0.05", "floor = 0.08");
        std::fs::write(&path, &edited).unwrap();
        assert_eq!(read_or_create(&path), edited);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bounded_build_at_balanced_goes_to_sonnet_medium_with_its_reason_line() {
        // Plan 14 §2.3 & §3 table fixture: g-sonnet scores 88 (same as sonnet) and fits in G·Claude's spare while Max is
        // projected past 100%. Plan 14 balanced mode did not drop the bar via climb.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        let got = chosen(&table, &world(), &request("build.bounded"));
        assert_eq!(
            (got.seat.as_str(), got.effort, got.cli.as_str()),
            ("g-sonnet", Effort::Medium, "agy")
        );
        assert_eq!(got.model, "claude-sonnet-5-5-medium");
        assert_eq!(
            (got.pool.as_str(), got.chain.as_str()),
            ("google3p", "build.bounded")
        );
        assert_eq!(
            got.reason,
            "build.bounded \u{2192} g-sonnet medium: fits G\u{b7}Claude's spare (week 95% projected unused, resets in 1d13h); \
             sonnet scores the same (max 5h 41% used, week 37% used)"
        );
        assert_eq!(
            why(&got.runners_up, "argon"),
            "scores 82.0, under the bar 85 at balanced"
        );
        // Once G·Claude's 5 h spare is spent, sonnet on Max takes build.bounded with §5.3 step 5's form.
        let mut busy_g = world();
        set(&mut busy_g, "google3p", "5h", 0.20);
        let on_max = chosen(&table, &busy_g, &request("build.bounded"));
        assert_eq!(
            (on_max.seat.as_str(), on_max.effort, on_max.cli.as_str()),
            ("sonnet", Effort::Medium, "claude")
        );
        assert_eq!(
            on_max.reason,
            "build.bounded \u{2192} sonnet medium: max 5h 41% used, week 37% used, reserve 15%; \
             g-sonnet scores the same (google3p 5h 20% used, week 4% used)"
        );
        // A same-model twin on another pool is named when it scores the same.
        assert_eq!(
            chosen(&table, &world(), &request("plan")).reason,
            "plan \u{2192} opus high: max 5h 41% used, week 37% used, reserve 15%; \
             g-opus scores the same (google3p 5h 3% used, week 4% used)"
        );
        // Expected use on Max: P4's order (0.088% of max 5 h) x weight 1 x medium size 5.
        let five = &on_max.windows[0];
        assert_eq!(five.window, "5h");
        assert!(close(five.expected, 0.0044), "{five:?}");
        assert!(close(five.headroom.unwrap(), 0.44), "{five:?}");
        assert!(on_max
            .use_basis
            .as_deref()
            .unwrap()
            .contains("derived: P4 sonnet medium"));
    }

    #[test]
    fn a_seat_that_is_off_or_whose_cli_is_not_ready_is_skipped_with_why() {
        // Plan 14 table: fable is an off seat.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        // Plan 14 table: build.frontend restricted argon to only_with = ["video"].
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        // Plan 14 table: g-opus is an unreserved anthropic twin.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let mut review = request("review.adversary");
        review.not_family = vec!["anthropic".into()];
        let got = chosen(&table, &world(), &review);
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::High));
        assert_eq!(
            why(&got.runners_up, "opus"),
            "family anthropic excluded (another family reviews, decision 0017)"
        );
        assert!(!got.same_family_fallback);
        review.not_family = vec!["google".into()];
        let got = chosen(&table, &world(), &review);
        assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));
        assert!(!got.same_family_fallback);
    }

    /// Decision 0017: "A review prefers another family, falls back to its own in a fresh session".
    #[test]
    fn a_review_with_no_other_family_left_falls_back_to_its_own_in_a_fresh_session() {
        // Plan 14 table: review.adversary has only anthropic and argon.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let mut world = world();
        set(&mut world, "argon", "week", 0.97);
        let mut review = request("review.adversary");
        review.not_family = vec!["anthropic".into()];
        let got = chosen(&table, &world, &review);
        assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));
        assert!(got.same_family_fallback);
        assert!(
            got.reason.ends_with(
                "; same family (anthropic), in a fresh session: no other family had a seat: \
                 argon (argon week 97% used: at or under the floor 5%) (decision 0017)"
            ),
            "{}",
            got.reason
        );
        // Decision 0019's acceptance author falls back the same way.
        let mut author = request("acceptance.author");
        author.not_family = vec!["anthropic".into()];
        let got = chosen(&table, &world, &author);
        assert_eq!(got.seat, "sonnet");
        assert!(got.same_family_fallback);
        // An override is the orchestrator's own choice: it is never relaxed.
        let mut over = review.clone();
        over.override_seat = Some(("opus".into(), "x".into()));
        assert_eq!(
            pick_err(&table, &world, &over),
            "override opus refused: family anthropic excluded (another family reviews, decision 0017)"
        );
        // Neither family: the NoSeat is the second pass's, so its earliest reset is the soonest seat of
        // either family, max's 5 h window, not argon's week.
        set(&mut world, "max", "5h", 0.97);
        set(&mut world, "google3p", "5h", 0.97);
        let no = pick(&table, &world, &review).unwrap_err();
        assert!(
            no.reason
                .ends_with("; its own family was tried too (decision 0017)"),
            "{}",
            no.reason
        );
        assert_eq!(no.earliest_reset, Some(at("2026-10-18T00:50:00Z")));
    }

    #[test]
    fn a_window_at_or_under_the_floor_skips_its_seats_booked_use_included() {
        let table = table();
        let mut world = world();
        set(&mut world, "gemini", "5h", 0.96);
        let got = chosen(&table, &world, &request("research.web"));
        // Plan 70: carbon is in hydrogen's pool (plan 71: hydrogen took flash's place), so the one floor skips both,
        // with the same why.
        assert!(
            !["carbon", "hydrogen"].contains(&got.seat.as_str()),
            "{}",
            got.seat
        );
        for seat in ["hydrogen", "carbon"] {
            assert_eq!(
                why(&got.runners_up, seat),
                "gemini 5h 96% used: at or under the floor 5%"
            );
        }
        // 90% used and 6% booked by an order in flight leaves 4%.
        let mut world = self::world();
        set(&mut world, "gemini", "5h", 0.90);
        world.leases.push(lease(
            "o-1",
            "hydrogen",
            "gemini",
            &[("5h", 0.06)],
            NOW + 600,
        ));
        let got = chosen(&table, &world, &request("research.web"));
        assert_eq!(
            why(&got.runners_up, "hydrogen"),
            "gemini 5h 90% used and 6% booked: at or under the floor 5%"
        );
    }

    #[test]
    fn an_order_a_window_cannot_hold_goes_to_a_pool_that_can() {
        // Plan 14 table: g-opus on google3p per_usd factor and build.load_bearing weight.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        // Plan 14 table: build.bounded had argon at 82, flash at 78, bar at 85, efficient at 80.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        // Gemini's and Argon's weeks 90% used: neither has spare, and hydrogen's binding window is the week (plan 71:
        // it took flash's place). Anthropic's seats are left out: since the review round's refit (flash low x8.97)
        // sonnet wins this world, and the test is hydrogen's pressure, not who wins.
        set(&mut world, "gemini", "week", 0.90);
        set(&mut world, "argon", "week", 0.90);
        let mut req = request("research.web");
        req.not_family = vec!["anthropic".into()];
        let got = chosen(&table, &world, &req);
        assert_eq!(got.seat, "hydrogen");
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
        // A candidate's pressure is its worst window's, wherever that sits in the pool's order.
        let window = |name: &str, expected: f32| WindowState {
            window: name.into(),
            used: Some(0.8),
            booked: 0.0,
            left: Some(0.2),
            headroom: Some(0.2),
            resets_at: None,
            elapsed: None,
            projected: None,
            spare: None,
            pace: Some(1.0),
            reset_factor: 1.0,
            spare_weight: 0.05,
            expected,
        };
        for windows in [
            vec![window("5h", 0.01), window("week", 0.04)],
            vec![window("week", 0.04), window("5h", 0.01)],
        ] {
            let cand = Candidate {
                index: 0,
                in_class: true,
                effort: None,
                effort_note: None,
                score: 0.0,
                record_note: None,
                use_basis: None,
                windows,
                skip: None,
                stepped: false,
            };
            assert!(close(cand.pressure(), 0.2), "{}", cand.pressure());
        }
    }

    #[test]
    fn quota_about_to_reset_is_discounted_by_the_share_of_the_order_before_it() {
        // Plan 14 table: g-sonnet on ops.terminal.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let c = &table.constants;
        // Half left, 30 min to the reset: a 15 min order runs wholly before it, half price.
        assert!(close(reset_factor(0.5, 1800, 15, c), 0.5));
        // A 2 h order: a quarter runs before it.
        assert!(close(reset_factor(0.5, 1800, 120, c), 0.875));
        // 20% left is not "more than 30%"; 61 min is not "under 60".
        assert!(close(reset_factor(0.2, 1800, 15, c), 1.0));
        assert!(close(reset_factor(0.5, 61 * 60, 15, c), 1.0));
        // In a got: google3p's 5 h window has 32% left (> 30%) and resets in 59 min (elapsed 0.803, so
        // projected is 84.6% and spare is 15.4%); g-sonnet medium on ops.terminal expects 16.55%, spilling
        // 1.2% past the spare into rest, where the reset discount prices it.
        let mut world = world();
        let g5 = world
            .readings
            .iter_mut()
            .find(|r| r.pool == "google3p" && r.window == "5h")
            .unwrap();
        g5.used = Some(0.68);
        g5.resets_at = Some(NOW + 3540);
        world.readings.retain(|r| r.pool != "max");
        let got = chosen(&table, &world, &request("ops.terminal"));
        assert_eq!(got.seat, "g-sonnet");
        let five = got.windows.iter().find(|w| w.window == "5h").unwrap();
        assert!(close(five.reset_factor, 0.5), "{five:?}");
        // The discount reaches the price: the 5 h window binds, and its non-spare rest carries the factor.
        assert!(five.reset_factor < 1.0);
        let spare = five.spare.unwrap();
        assert!(spare < five.expected, "{five:?}");
        let rest = five.expected - spare;
        let term = spare * five.spare_weight
            + rest / five.headroom.unwrap() * five.pace.unwrap() * five.reset_factor;
        assert!(close(got.pressure, term), "{} {term}", got.pressure);
        // "More than 30% left" is what the order could use: max's 5 h window 65% used is 35% unused but 20%
        // after the reserve, so its reset 20 minutes off is no discount.
        let mut world = self::world();
        set(&mut world, "max", "5h", 0.65);
        set(&mut world, "max", "week", 0.05);
        set(&mut world, "google3p", "5h", 0.90);
        world
            .readings
            .iter_mut()
            .find(|r| r.pool == "max" && r.window == "5h")
            .unwrap()
            .resets_at = Some(NOW + 1200);
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "sonnet");
        assert!(
            close(got.windows[0].reset_factor, 1.0),
            "{:?}",
            got.windows[0]
        );
    }

    #[test]
    fn ties_go_to_the_higher_score_then_to_the_classs_own_order() {
        let mut table = table();
        for seat in table
            .seats
            .iter_mut()
            .filter(|s| s.id == "hydrogen" || s.id == "carbon")
        {
            for cell in &mut seat.uses {
                cell.windows.clear();
            }
        }
        // Both 84, both no expected use: hydrogen heads research.web's row (§5.4), though carbon is earlier
        // in the table (plan 70: carbon took argon's place; plan 71: hydrogen took flash's).
        let got = chosen(&table, &world(), &request("research.web"));
        assert_eq!(got.seat, "hydrogen");
        assert_eq!(
            why(&got.runners_up, "carbon"),
            "same pressure and score, later in the class's order"
        );
        // The score comes before the class's order.
        let class = table
            .classes
            .iter_mut()
            .find(|c| c.id == "research.web")
            .unwrap();
        class
            .seats
            .iter_mut()
            .find(|e| e.seat == "hydrogen")
            .unwrap()
            .score = 83.0;
        let got = chosen(&table, &world(), &request("research.web"));
        assert_eq!(got.seat, "carbon");
        assert_eq!(
            why(&got.runners_up, "hydrogen"),
            "same pressure, scores 83.0 against 84.0"
        );
    }

    #[test]
    fn a_window_not_read_ranks_below_known_headroom_and_above_a_window_too_small() {
        // Plan 14 §5.4 table: sonnet and its same-model twin g-sonnet both 88 (sonnet, g-sonnet, flash).
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let mut world = world();
        world.readings.retain(|r| r.pool != "max");
        let got = chosen(&table, &world, &request("ops.terminal"));
        assert_eq!(got.seat, "g-sonnet");
        assert_eq!(
            why(&got.runners_up, "sonnet"),
            "5h, week not read: ranks below known headroom"
        );
        // Google's Claude pool exhausted: the unread pool is better than none.
        set(&mut world, "google3p", "5h", 0.97);
        let got = chosen(&table, &world, &request("ops.terminal"));
        assert_eq!(got.seat, "sonnet");
        assert_eq!(got.unknown, ["5h", "week"]);
        // A reading older than the staleness line is not read.
        let mut world = self::world();
        for r in world.readings.iter_mut().filter(|r| r.pool == "max") {
            r.read_at = NOW - nirvana_usage::STALE_AFTER - 100;
        }
        assert_eq!(
            chosen(&table, &world, &request("ops.terminal")).seat,
            "g-sonnet"
        );
        // A known window that cannot hold the order is nearer exhausted than free: google3p's 5 h window has
        // 10% left for an order expected at 17%, so the unread max ranks above it (review S2). An unread pool
        // never counts as one that can hold the order, so g-sonnet is ranked, not skipped.
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
        let got = chosen(&table, &world, &request("ops.terminal"));
        assert_eq!(got.seat, "sonnet");
        assert_eq!(
            why(&got.runners_up, "g-sonnet"),
            "google3p 5h has 10% left for an order of 17%: ranks below an unread pool"
        );
        // Max's 5 h window unread: the reserve could not be checked, and the line says so.
        assert_eq!(
            got.reason,
            "ops.terminal \u{2192} sonnet medium: max 5h not read, week not read, reserve 15% not checked (5h not read); \
             g-sonnet scores the same (google3p 5h 90% used, week 4% used)"
        );
        // A pool with no windows to read is unread, not free: local would have won every chore at pressure 0.
        // Plan 14 table: chore.script included local.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        table
            .seats
            .iter_mut()
            .find(|s| s.id == "local")
            .unwrap()
            .enabled = true;
        let got = chosen(&table, &self::world(), &request("chore.script"));
        assert_eq!(got.seat, "flash");
        assert_eq!(
            why(&got.runners_up, "local"),
            "local has no windows to read: ranks below known headroom"
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
        // Plan 14 table: build.bounded had sonnet without haiku.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        second.seat = "sonnet".into();
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
        assert_eq!(load_leases(&path).unwrap().leases, leases.leases);
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
    fn the_lease_lock_is_exclusive_and_a_file_left_behind_does_not_hold_it() {
        let dir = temp_dir("lock");
        let held = Lock::try_take(&dir, 2).expect("free");
        let err = Lock::try_take(&dir, 2).err().expect("held by the first");
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
        drop(held);
        // A drop closes the handle and the OS lets go; nothing deletes the file, so no holder can delete
        // another's lock.
        assert!(dir.join(LOCK_FILE).is_file());
        let again = Lock::try_take(&dir, 2).expect("free once the holder is gone");
        drop(again);
        // A file a dead process left, its pid in it as the first build wrote: no lock is on it, so it is
        // taken at the first try, with no stale rule.
        std::fs::write(dir.join(LOCK_FILE), "4242").unwrap();
        assert!(Lock::try_take(&dir, 1).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_override_passes_every_filter_and_three_are_taken_per_run() {
        // Plan 14 table: build.bounded rule pick was g-sonnet medium.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let mut over = request("build.bounded");
        over.override_seat = Some(("argon".into(), "wants a Gemini build".into()));
        let got = chosen(&table, &world(), &over);
        assert_eq!((got.seat.as_str(), got.effort), ("argon", Effort::Low));
        assert_eq!(got.rule_pick.as_deref(), Some("g-sonnet medium"));
        assert!(
            got.reason.ends_with(
                "; argon scores 82, under the bar 85 at balanced; override: wants a Gemini build; \
                 the rule's pick: g-sonnet medium"
            ),
            "{}",
            got.reason
        );
        assert!(close(got.bar, 82.0) && !got.bar_dropped, "{}", got.bar);
        assert_eq!(why(&got.runners_up, "g-sonnet"), "the override took argon");
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
            ("hydrogen".into(), Effort::Medium)
        );
        assert_eq!(
            effort("build.bounded", Mode::Sota),
            ("opus".into(), Effort::High)
        );
        assert_eq!(
            effort("fix.debug", Mode::Balanced),
            ("hydrogen".into(), Effort::Medium)
        );
        assert_eq!(
            effort("plan", Mode::Balanced),
            ("hydrogen".into(), Effort::High)
        );
        // Plan 40a: Fable 97 heads plan at SOTA while its week was read fresh (this world's 0%).
        assert_eq!(effort("plan", Mode::Sota), ("fable".into(), Effort::High));
        // An override asking xhigh on g-opus gets high because g-opus offers no xhigh.
        let mut over = request("plan");
        over.override_seat = Some(("g-opus:xhigh".into(), "trickier".into()));
        let got = chosen(&table, &world, &over);
        assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));
        assert_eq!(
            got.effort_note.as_deref(),
            Some("effort overridden to xhigh; g-opus has no xhigh; high")
        );
        // A class asking max gets high, even from a seat that lists max.
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
            .find(|s| s.id == "hydrogen")
            .unwrap()
            .efforts
            .push(Effort::Max);
        table
            .seats
            .iter_mut()
            .find(|s| s.id == "opus")
            .unwrap()
            .efforts
            .push(Effort::Max);
        let got = chosen(&table, &self::world(), &request("plan"));
        assert_eq!((got.seat.as_str(), got.effort), ("hydrogen", Effort::High));
        assert_eq!(got.effort_note.as_deref(), Some("capped at high"));
        // An explicit override can still choose max.
        let mut over_max = request("plan");
        over_max.override_seat = Some(("opus:max".into(), "explicit max".into()));
        let got_max = chosen(&table, &self::world(), &over_max);
        assert_eq!(
            (got_max.seat.as_str(), got_max.effort),
            ("opus", Effort::Max)
        );
        assert_eq!(
            got_max.effort_note.as_deref(),
            Some("effort overridden to max")
        );
    }

    #[test]
    fn l6_no_class_in_any_mode_chooses_an_effort_above_high_and_override_works() {
        let table = table();
        let world = world();
        // 1. Every class across every mode picks an effort <= High
        for class in &table.classes {
            for mode in [Mode::Efficient, Mode::Balanced, Mode::Sota] {
                let mut req = request(&class.id);
                req.mode = mode;
                let pick = chosen(&table, &world, &req);
                assert!(
                    pick.effort <= Effort::High,
                    "class {} in mode {:?} picked effort {:?} > High",
                    class.id,
                    mode,
                    pick.effort
                );
            }
        }

        // 2. When a class or seat wanted Xhigh or Max, note is "capped at high"
        let mut modified_table = table.clone();
        modified_table
            .classes
            .iter_mut()
            .find(|c| c.id == "plan")
            .unwrap()
            .effort = Effort::Xhigh;
        let pick = chosen(&modified_table, &world, &request("plan"));
        assert_eq!(pick.effort, Effort::High);
        assert_eq!(pick.effort_note.as_deref(), Some("capped at high"));

        // 3. An override to xhigh still works and says so in effort_note
        let mut req_xhigh = request("plan");
        req_xhigh.override_seat = Some(("opus:xhigh".into(), "override reason".into()));
        let pick_xhigh = chosen(&table, &world, &req_xhigh);
        assert_eq!(pick_xhigh.seat, "opus");
        assert_eq!(pick_xhigh.effort, Effort::Xhigh);
        assert_eq!(
            pick_xhigh.effort_note.as_deref(),
            Some("effort overridden to xhigh")
        );
        assert!(pick_xhigh.reason.contains("effort overridden to xhigh"));
    }

    #[test]
    fn under_pressure_with_nothing_left_one_effort_step_down() {
        // Plan 14 table: plan seats were g-opus, opus, argon (sonnet was not in plan).
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
    fn a_seat_already_at_its_lowest_effort_is_not_said_to_step_down() {
        let table = table();
        let class = table.class("media.audio").unwrap();
        let got = match evaluate(&table, &world(), &request("media.audio"), class, true) {
            Ok(got) => got,
            Err(_) => panic!("no seat"),
        };
        assert_eq!((got.seat.as_str(), got.effort), ("hydrogen", Effort::Low));
        assert!(!got.step_down);
        assert!(!got.reason.contains("step down"), "{}", got.reason);
    }

    /// Live checks 2026-11-01 F3 and gsrc review S6: a child is picked at its parent's effort, so its windows and use
    /// basis are that effort's, not patched after the pick. Pressure is read now: a parent's effort that no longer fits
    /// steps the child down from it (§5.5), and a parent's own step down is kept, its effort the base and never raised
    /// back, since a raise would spend more on the pool that was pressed.
    #[test]
    fn gsrc_a_child_is_picked_at_the_parents_effort_and_pressure_now_steps_it_down() {
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        let mut world = world();
        agy_out(&mut world);
        let five = |pick: &Pick| {
            pick.windows
                .iter()
                .find(|window| window.window == "5h")
                .map(|window| window.expected)
        };
        // No pressure: the class gives opus high; a parent at medium passes medium, priced at medium.
        let rule = chosen(&table, &world, &request("plan"));
        assert_eq!(
            (rule.seat.as_str(), rule.effort),
            ("opus", Effort::High),
            "{}",
            rule.reason
        );
        let mut child = request("plan");
        child.effort = Some(Effort::Medium);
        let got = chosen(&table, &world, &child);
        assert_eq!(
            (got.seat.as_str(), got.effort),
            ("opus", Effort::Medium),
            "{}",
            got.reason
        );
        assert!(!got.step_down);
        assert!(
            got.reason.contains("effort medium kept from the parent"),
            "{}",
            got.reason
        );
        assert!(
            got.use_basis
                .as_deref()
                .is_some_and(|basis| basis.starts_with("opus at medium")),
            "{:?}",
            got.use_basis
        );
        assert!(
            five(&got) < five(&rule),
            "{:?} {:?}",
            five(&got),
            five(&rule)
        );
        // Pressure now: 16.8% left, opus high breaks the reserve; a parent at high is stepped down to medium.
        set(&mut world, "max", "5h", 0.832);
        child.effort = Some(Effort::High);
        let got = chosen(&table, &world, &child);
        assert_eq!(
            (got.seat.as_str(), got.effort),
            ("opus", Effort::Medium),
            "{}",
            got.reason
        );
        assert!(got.step_down);
        assert!(
            got.reason.contains("the parent's effort high"),
            "{}",
            got.reason
        );
    }

    /// F3: a parent's effort is not lifted by SOTA nor capped at high (a caller's or an override's effort), and the
    /// mode's climb steps a child down one from the parent's effort, never from the class's.
    #[test]
    fn gsrc_a_childs_base_is_the_parents_effort_for_the_mode_the_cap_and_the_climb() {
        let table = table();
        // Opus's effort, whether it won or ran up: the default world seats plan on Gemini's spare.
        let opus = |pick: &Pick| {
            if pick.seat == "opus" {
                Some(pick.effort)
            } else {
                pick.runners_up
                    .iter()
                    .find(|loser| loser.seat == "opus")
                    .and_then(|loser| loser.effort)
            }
        };
        let mut sota = request("plan");
        sota.mode = Mode::Sota;
        sota.effort = Some(Effort::Low);
        assert_eq!(chosen(&table, &world(), &sota).effort, Effort::Low);
        let el_week = nirvana_usage::elapsed(
            world()
                .readings
                .iter()
                .find(|r| r.pool == "max" && r.window == "week")
                .unwrap(),
        )
        .unwrap();
        let at = |climb: f32| {
            let mut world = world();
            set(&mut world, "max", "5h", 0.05);
            set(&mut world, "max", "week", climb * el_week);
            world
        };
        // A cool world (l3's 0.5, no step): a parent at xhigh is not capped at high.
        let mut xhigh = request("plan");
        xhigh.effort = Some(Effort::Xhigh);
        let got = chosen(&table, &at(0.5), &xhigh);
        assert_eq!(
            opus(&got),
            Some(Effort::Xhigh),
            "{} {:?}",
            got.reason,
            got.runners_up
        );
        // The climb at 0.85 steps plan's high to medium (l3); a parent at xhigh steps to high, one step from its own.
        let world_85 = at(0.85);
        let got = chosen(&table, &world_85, &xhigh);
        assert_eq!(opus(&got), Some(Effort::High), "{}", got.reason);
    }

    #[test]
    fn a_booking_counts_its_runs_overrides_and_a_repeat_returns_or_refuses() {
        let table = table();
        let mut over = request("build.bounded");
        over.override_seat = Some(("carbon".into(), "wants a Gemini build".into()));
        let mut leases = Leases::default();
        for n in 1..=3 {
            let mut world = world();
            world.overrides_used = leases.overrides("r-1");
            let got = chosen(&table, &world, &over);
            let order = format!("o-{n}");
            let (lease, changed) =
                book_pick(&mut leases, &table, &got, Some(&order), Some("r-1"), NOW);
            assert!(changed && lease.is_some_and(|lease| lease.order == order));
            assert_eq!(leases.overrides("r-1"), n);
        }
        // The fourth in the run is refused, by the count `book_pick` kept.
        let mut world = world();
        world.overrides_used = leases.overrides("r-1");
        assert!(pick_err(&table, &world, &over).starts_with("override carbon refused: 3 of 3"));
        // A rule pick counts nothing against its run.
        let rule = chosen(&table, &self::world(), &request("build.bounded"));
        book_pick(&mut leases, &table, &rule, Some("o-9"), Some("r-2"), NOW);
        assert_eq!(leases.overrides("r-2"), 0);
        // A repeat --book gives the first pick back, or refuses when it names another override.
        let known = existing_booking(&leases, "o-1", None, NOW + 5)
            .unwrap()
            .unwrap();
        assert_eq!(known.seat, "carbon");
        assert_eq!(known.pick["override"], "wants a Gemini build");
        assert!(existing_booking(&leases, "o-1", Some("carbon"), NOW + 5)
            .unwrap()
            .is_ok());
        assert_eq!(
            existing_booking(&leases, "o-9", Some("carbon"), NOW + 5)
                .unwrap()
                .unwrap_err(),
            "override carbon refused: o-9 is already booked on hydrogen medium (the rule's pick); \
             release it first (herdr seat release o-9)"
        );
        assert!(existing_booking(&leases, "o-404", Some("carbon"), NOW).is_none());
        // A run whose last override is over eight days old is dropped at the next write.
        let (_, changed) = book_pick(&mut leases, &table, &rule, None, None, NOW + RUN_KEEP_S + 1);
        assert!(changed);
        assert!(leases.runs.is_empty(), "{:?}", leases.runs);
    }

    #[test]
    fn a_leases_file_that_does_not_parse_is_an_error_never_empty() {
        let dir = temp_dir("corrupt");
        let path = dir.join(LEASES_FILE);
        assert_eq!(
            load_leases(&path),
            Ok(Leases::default()),
            "missing is empty"
        );
        std::fs::write(
            &path,
            "{\"version\": 1, \"runs\": {\"r-1\": {\"overrides\": 3",
        )
        .unwrap();
        let err = load_leases(&path).unwrap_err();
        assert!(err.contains("does not parse"), "{err}");
        assert!(err.contains("fix or delete it"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sign_in_read_that_hangs_is_given_up_at_its_deadline() {
        use crate::nirvana_accounts::Signed;
        let started = std::time::Instant::now();
        let why = signed_within("claude", Duration::from_millis(200), || {
            std::thread::sleep(Duration::from_secs(3));
            Signed::In("a@b".into())
        });
        assert_eq!(why.as_deref(), Some("claude sign-in not read in 200ms"));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert_eq!(
            signed_within("agy", Duration::from_secs(5), || Signed::Out).as_deref(),
            Some("agy not signed in")
        );
        assert_eq!(
            signed_within("claude", Duration::from_secs(5), || Signed::In(
                "a@b".into()
            )),
            None
        );
        assert_eq!(ACCOUNTS_TIMEOUT, Duration::from_secs(15));
    }

    #[test]
    fn a_needs_or_family_word_no_seat_has_is_named() {
        let table = table();
        let words = unknown_words(&table, &["vidoe".into()], &["antropic".into()]);
        assert_eq!(words.len(), 2, "{words:?}");
        assert!(words[0].starts_with("--needs vidoe: no seat has it; the capabilities: "));
        assert!(words[0].contains("video"));
        assert!(words[1].starts_with("--not-family antropic: no seat is of it; the families: "));
        assert!(words[1].contains("anthropic, google"));
        assert!(unknown_words(&table, &["video".into()], &["anthropic".into()]).is_empty());
    }

    /// Plan 12 §5.4's rows, updated for Plan 14 spend-down seating: on `world()` (`2026-10-17T21:51Z`), Max is
    /// projected past 100% (0% spare) while G·Claude and Gemini have projected spare, so equal-scoring seats on
    /// spare pools lead at `Small` and then walk the row as each pool is spent.
    #[test]
    fn every_class_at_balanced_seats_its_5_4_head_then_walks_its_row() {
        const ROWS: &[(&str, &[&str], &[&str])] = &[
            ("plan", &[], &["g-opus", "opus", "argon"]),
            ("build.load_bearing", &[], &["g-opus", "opus", "argon"]),
            (
                "build.bounded",
                &[],
                &["g-sonnet", "sonnet", "argon", "flash"],
            ),
            ("build.frontend", &[], &["g-opus", "opus"]),
            ("judge.visual", &[], &["g-opus", "opus"]),
            ("fix.debug", &[], &["g-opus", "opus", "argon"]),
            ("ops.terminal", &[], &["g-sonnet", "sonnet", "flash"]),
            (
                "review.adversary",
                &["anthropic"],
                &["argon", "g-opus", "opus"],
            ),
            (
                "review.adversary",
                &["google"],
                &["g-opus", "opus", "argon"],
            ),
            ("acceptance.author", &["anthropic"], &["argon", "sonnet"]),
            ("acceptance.author", &["google"], &["sonnet", "argon"]),
            ("research.read", &[], &["sonnet", "flash"]),
            (
                "research.web",
                &[],
                &["flash", "argon", "g-sonnet", "sonnet"],
            ),
            ("read.huge", &[], &["flash", "argon"]),
            ("write.docs", &[], &["argon", "g-sonnet", "sonnet"]),
            ("media.video", &[], &["argon", "flash"]),
            ("media.audio", &[], &["flash", "argon"]),
            ("sweep.mechanical", &[], &["flash", "haiku"]),
            ("chore.memory", &[], &["flash", "haiku"]),
            ("chore.script", &[], &["flash", "haiku"]),
            ("chore.github", &[], &["flash", "haiku"]),
        ];
        // Plan 14 §5.4 table and rows: Plan 14 balanced mode did not have climb bar drop.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        let mut covered: Vec<&str> = ROWS.iter().map(|(class, _, _)| *class).collect();
        covered.dedup();
        let classes: Vec<&str> = table.classes.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(covered, classes, "every class has its row");
        let ask = |class: &str, not_family: &[&str], size: SizeName| {
            let mut r = request(class);
            r.not_family = not_family.iter().map(|f| (*f).to_owned()).collect();
            r.needs = table.class(class).unwrap().needs.clone();
            r.size = size;
            r
        };
        for (class, not_family, row) in ROWS {
            // Walk the row at Small: spend every pool before the next seat.
            let mut world = world();
            for (at, seat) in row.iter().enumerate() {
                let r = ask(class, not_family, SizeName::Small);
                let got = chosen(&table, &world, &r);
                assert_eq!(got.seat, *seat, "{class} step {at}: {}", got.reason);
                assert_eq!(
                    got.same_family_fallback,
                    not_family.contains(&got.family.as_str()),
                    "{class} step {at}: {}",
                    got.reason
                );
                let pool = table.seat(seat).unwrap().pool.clone();
                for reading in world.readings.iter_mut().filter(|r| r.pool == pool) {
                    reading.used = Some(0.97);
                }
            }
        }
    }

    #[test]
    fn no_seat_names_every_skip_and_the_earliest_reset() {
        // Plan 14 table: build.bounded candidates and reasons.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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

    #[test]
    fn a_seat_floored_on_two_windows_is_free_only_when_both_have_reset() {
        let table = table();
        let mut world = world();
        let floor = |world: &mut World, pool: &str, window: &str, resets: Option<u64>| {
            let r = world
                .readings
                .iter_mut()
                .find(|r| r.pool == pool && r.window == window)
                .unwrap();
            r.used = Some(0.97);
            r.resets_at = resets;
        };
        // Haiku is floored on max's 5 h window for 20 minutes and on its week for five days; flash on
        // gemini's 5 h window for three hours. The first window's reset said "defer 20 minutes".
        floor(&mut world, "max", "5h", Some(NOW + 20 * 60));
        floor(&mut world, "max", "week", Some(NOW + 5 * 86_400));
        floor(&mut world, "gemini", "5h", Some(NOW + 3 * 3600));
        let no = pick(&table, &world, &request("chore.script")).unwrap_err();
        assert_eq!(no.earliest_reset, Some(NOW + 3 * 3600));
        assert!(!no.defer);
        // A floored window with no reset time leaves its seat's reset unknown, so it frees no seat.
        floor(&mut world, "gemini", "5h", None);
        floor(&mut world, "gemini", "week", Some(NOW + 3600));
        let no = pick(&table, &world, &request("chore.script")).unwrap_err();
        assert_eq!(no.earliest_reset, Some(NOW + 5 * 86_400));
    }

    /// Plan 12 L9's verifier: "`seat pick --needs video` never returns a Claude seat".
    #[test]
    fn an_order_that_needs_video_never_gets_a_claude_seat() {
        let table = table();
        let mut seated = 0;
        for world in [world(), {
            let mut w = world();
            set(&mut w, "gemini", "week", 0.97);
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
        assert!(seated > 0, "media.video seats carbon");
        let mut video = request("media.video");
        video.needs = vec!["video".into()];
        assert_eq!(chosen(&table, &world(), &video).seat, "carbon");
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
        // Plan 14 table: build.bounded pick was g-sonnet without use_record; Plan 14 balanced mode did not drop the bar via climb.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        let mut world = world();
        world.record.insert(
            ("g-sonnet".into(), Effort::Medium, "build.bounded".into()),
            Tally {
                passes: 0,
                orders: 3,
            },
        );
        assert_eq!(
            chosen(&table, &world, &request("build.bounded")).seat,
            "g-sonnet"
        );
        table.use_record = true;
        let got = chosen(&table, &world, &request("build.bounded"));
        assert_eq!(got.seat, "sonnet");
        assert_eq!(
            why(&got.runners_up, "g-sonnet"),
            "scores 55.0, under the bar 85 at balanced"
        );
    }

    #[test]
    fn a_finished_order_is_recorded_with_its_use_from_its_own_tokens() {
        // Plan 14 table: factors fit on P4's order under Plan 14 per_usd factors.
        let table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
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
        // Another order: since the review round one order and run is decided by its last line (`tally`).
        let mut fail = line.clone();
        fail.order = "o-2".into();
        fail.outcome = "sent_back".into();
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
        // A pick in December expects twice the share the November cell holds. Plan 70: argon is off, so the pick is
        // carbon, whose price doubles on the same day; its cells are on gemini's two windows.
        let carbon = table.seat("carbon").unwrap();
        assert!(close(
            carbon.price_at(date("2026-11-30").unwrap()).unwrap().input as f32,
            2.0 * carbon.price_at(date("2026-11-29").unwrap()).unwrap().input as f32
        ));
        let december = date("2026-12-01").unwrap() + 43_200;
        let world = World {
            now: december,
            readings: ["5h", "week"]
                .into_iter()
                .map(|window| reading("gemini", window, 0.1, december + 4 * 3_600, december - 30))
                .collect(),
            ..World::default()
        };
        let mut video = request("media.video");
        video.needs = vec!["video".into()];
        let got = chosen(&table, &world, &video);
        assert_eq!((got.seat.as_str(), got.effort), ("carbon", Effort::Medium));
        let week = got.windows.iter().find(|w| w.window == "week").unwrap();
        // carbon medium's 0.1803% x 2 (price) x 2 (weight) x 5 (medium).
        assert!(
            close(week.expected, 0.001803 * 2.0 * 2.0 * 5.0),
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
        assert_eq!(seat("gpt-oss")["why_not"], "off in seats.toml");
        // Plan 40a: Fable is on, and its own week is one of its windows.
        assert!(seat("fable")["why_not"].is_null());
        assert_eq!(seat("fable")["windows"][2]["window"], "week.fable");
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

    /// Plan 14 §2.1: `projected = used / max(elapsed, ease)` without `pace_min` clamping or adding `ease` to
    /// `used`, and `spare = max(0, 1 - projected - booked)`.
    #[test]
    fn projected_spare_uses_elapsed_with_ease_floor_and_subtracts_booked_leases() {
        let ease = 0.10;
        // G·Claude week on 2026-10-18: 4% used with 11 h of 168 h left (elapsed ~0.9345) -> projected ~4.3%,
        // spare ~95.7%.
        let g_week = reading("google3p", "week", 0.04, NOW + 11 * 3600, NOW);
        let (elapsed, projected, spare) = projected_spare(&g_week, 0.0, ease);
        assert!(close(elapsed.unwrap(), (604_800.0 - 39_600.0) / 604_800.0));
        assert!(close(projected.unwrap(), 0.04 / elapsed.unwrap()));
        assert!(close(spare.unwrap(), 1.0 - projected.unwrap()));
        assert!((spare.unwrap() - 0.957).abs() < 0.001, "{spare:?}");
        // Max week on 2026-10-18: 48% used with 32% of the week elapsed -> projected 150%, spare 0%.
        let max_week = reading("max", "week", 0.48, NOW + (604_800.0 * 0.68) as u64, NOW);
        let (elapsed, projected, spare) = projected_spare(&max_week, 0.0, ease);
        assert!(close(elapsed.unwrap(), 0.32));
        assert!(close(projected.unwrap(), 1.50));
        assert!(close(spare.unwrap(), 0.0));
        // Fresh window (elapsed < ease) divides by ease, never by zero, and never clamps 0% used up to pace_min.
        let fresh_zero = reading("google3p", "5h", 0.0, NOW + 17_800, NOW);
        let (_, projected, spare) = projected_spare(&fresh_zero, 0.0, ease);
        assert!(close(projected.unwrap(), 0.0));
        assert!(close(spare.unwrap(), 1.0));
        let fresh_used = reading("google3p", "5h", 0.03, NOW + 17_800, NOW);
        let (elapsed, projected, spare) = projected_spare(&fresh_used, 0.0, ease);
        assert!(elapsed.unwrap() < ease);
        assert!(close(projected.unwrap(), 0.03 / ease));
        assert!(close(spare.unwrap(), 0.70));
        // Booked leases reduce spare directly.
        let (_, _, booked_spare) = projected_spare(&fresh_used, 0.25, ease);
        assert!(close(booked_spare.unwrap(), 0.45));
        let (_, _, clamped_spare) = projected_spare(&fresh_used, 0.80, ease);
        assert!(close(clamped_spare.unwrap(), 0.0));
        // Unknown reset time leaves elapsed, projected and spare None.
        let mut no_reset = g_week;
        no_reset.resets_at = None;
        assert_eq!(projected_spare(&no_reset, 0.0, ease), (None, None, None));
    }

    /// Plan 14 §2.2: `spare_part = min(e, spare)` is weighted by `spare_weight` while `rest = max(0, e - spare)`
    /// pays `rest / headroom * pace * reset_factor`; the worst window still binds and tier 2 still guards a
    /// window too small for the order.
    #[test]
    fn cost_splits_spare_and_rest_across_fresh_resetting_racing_and_booked_windows() {
        let w = |spare: Option<f32>, headroom: f32, pace: f32, reset_factor: f32, expected: f32| {
            WindowState {
                window: "5h".into(),
                used: Some(1.0 - headroom),
                booked: 0.0,
                left: Some(headroom),
                headroom: Some(headroom),
                resets_at: Some(NOW + 3600),
                elapsed: Some(0.8),
                projected: spare.map(|s| 1.0 - s),
                spare,
                pace: Some(pace),
                reset_factor,
                spare_weight: 0.05,
                expected,
            }
        };
        // Wholly inside spare: `expected * spare_weight`.
        let inside = w(Some(0.60), 0.70, 0.5, 1.0, 0.20);
        assert!(close(inside.pressure().unwrap(), 0.20 * 0.05));
        // Racing window (spare = 0): full price `expected / headroom * pace * reset_factor`.
        let racing = w(Some(0.0), 0.50, 1.6, 1.0, 0.20);
        assert!(close(racing.pressure().unwrap(), 0.20 / 0.50 * 1.6));
        // Straddling spare and rest near reset: `spare * spare_weight + rest / headroom * pace * reset_factor`.
        let straddle = w(Some(0.15), 0.40, 0.9, 0.5, 0.25);
        let want = 0.15 * 0.05 + 0.10 / 0.40 * 0.9 * 0.5;
        assert!(close(straddle.pressure().unwrap(), want));
        // A booked lease on a pool reduces its spare so an order that fitted before now spills into rest.
        // Plan 14 table: g-opus is an unreserved twin candidate; Plan 14 balanced mode did not drop the bar.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        let mut world = world();
        world.leases.push(lease(
            "o-booked",
            "g-opus",
            "google3p",
            &[("5h", 0.60)],
            NOW + 600,
        ));
        let mut small_plan = request("plan");
        small_plan.size = SizeName::Small;
        let got = chosen(&table, &world, &small_plan);
        // With 60% of google3p 5h booked, its spare drops from 66.9% to 6.9%, so g-opus high (17%) spills 10.1%
        // into non-spare rest and opus on max wins instead.
        assert_eq!(got.seat, "opus");
        // When g-opus wins with one window in spare (week: 42% <= 94.8%) and another spilling (5h: 85% > 66.9%),
        // the reason line does not claim the order fits in spare.
        let mut only_agy = self::world();
        only_agy
            .clis
            .insert("claude".into(), "claude not signed in".into());
        let got = chosen(&table, &only_agy, &request("plan"));
        assert_eq!(got.seat, "g-opus");
        assert!(
            got.reason
                .starts_with("plan \u{2192} g-opus high: google3p 5h 3% used, week 4% used"),
            "{}",
            got.reason
        );
    }

    /// Plan 14 §3: fixture test on the 2026-10-18 readings, and when Max has spare too.
    #[test]
    fn spend_down_on_2026_10_18_readings_and_when_max_has_spare_too() {
        // Plan 14 balanced mode did not drop the bar via climb.
        let mut table = parse(HISTORICAL_DEFAULTS[2]).unwrap();
        table.constants.climb_start = 99.0;
        table.constants.climb_full = 100.0;
        // 2026-10-18T21:48:31Z: Max 5h 24% used (resets 02:00Z), Max week 48% used (pace 1.38, resets
        // 2026-10-23T16:00Z); G·Claude 5h 0% used, G·Claude week 4.02% used resetting in 11h; Gemini 5h 0%
        // used, week 2.52% used; Argon week 2.12% used.
        let now_1018 = at("2026-10-18T21:48:31Z");
        let read = now_1018 - 30;
        let world_1018 = World {
            now: now_1018,
            readings: vec![
                reading("max", "5h", 0.24, at("2026-10-19T02:00:00Z"), read),
                reading("max", "week", 0.48, at("2026-10-23T16:00:00Z"), read),
                reading("max", "week.fable", 0.0, at("2026-10-23T16:00:00Z"), read),
                reading("gemini", "5h", 0.0, at("2026-10-19T02:58:37Z"), read),
                reading("gemini", "week", 0.0252, at("2026-10-24T15:48:02Z"), read),
                reading("google3p", "5h", 0.0, at("2026-10-19T02:58:37Z"), read),
                reading("google3p", "week", 0.0402, now_1018 + 11 * 3600, read),
                reading("argon", "week", 0.0212, at("2026-10-24T16:03:32Z"), read),
            ],
            ..World::default()
        };
        for size in [SizeName::Small, SizeName::Medium] {
            let mut plan = request("plan");
            plan.size = size;
            let got = chosen(&table, &world_1018, &plan);
            assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));
            assert!(
                got.reason.contains(
                    "g-opus high: fits G\u{b7}Claude's spare (week 96% projected unused, resets in 11h)"
                ),
                "{}",
                got.reason
            );

            let mut fix = request("fix.debug");
            fix.size = size;
            let got = chosen(&table, &world_1018, &fix);
            assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::Medium));

            let mut review = request("review.adversary");
            review.size = size;
            review.not_family = vec!["google".into()];
            let got = chosen(&table, &world_1018, &review);
            assert_eq!((got.seat.as_str(), got.effort), ("g-opus", Effort::High));

            let mut bounded = request("build.bounded");
            bounded.size = size;
            let got = chosen(&table, &world_1018, &bounded);
            assert_eq!(
                (got.seat.as_str(), got.effort),
                ("g-sonnet", Effort::Medium)
            );
            assert!(
                got.reason.contains(
                    "g-sonnet medium: fits G\u{b7}Claude's spare (week 96% projected unused, resets in 11h)"
                ),
                "{}",
                got.reason
            );
        }

        // build.load_bearing at medium (128% of G·Claude's 5 h window) and large (510%) goes to opus on Max,
        // and when Max is unread so Filter 6 does not skip g-opus, g-opus is tier 2 because its 5 h window
        // cannot hold the order.
        for size in [SizeName::Medium, SizeName::Large] {
            let mut lb = request("build.load_bearing");
            lb.size = size;
            let got = chosen(&table, &world_1018, &lb);
            assert_eq!((got.seat.as_str(), got.effort), ("opus", Effort::High));
        }
        let mut unread_other = world_1018.clone();
        unread_other.readings.retain(|r| r.pool == "google3p");
        let got = chosen(&table, &unread_other, &request("build.load_bearing"));
        assert_eq!(got.seat, "opus");
        assert_eq!(
            why(&got.runners_up, "g-opus"),
            "google3p 5h has 100% left for an order of 128%: ranks below an unread pool"
        );

        // read.huge and sweep.mechanical -> flash; write.docs -> argon.
        assert_eq!(
            chosen(&table, &world_1018, &request("read.huge")).seat,
            "flash"
        );
        assert_eq!(
            chosen(&table, &world_1018, &request("sweep.mechanical")).seat,
            "flash"
        );
        assert_eq!(
            chosen(&table, &world_1018, &request("write.docs")).seat,
            "argon"
        );

        // With Max also having spare (5% used with 90% of the window left, pace ~0.5), the old picks return.
        let mut max_spare = world_1018;
        for r in max_spare.readings.iter_mut().filter(|r| r.pool == "max") {
            r.used = Some(0.05);
            r.resets_at = Some(if r.window == "5h" {
                now_1018 + (18_000.0 * 0.90) as u64
            } else {
                now_1018 + (604_800.0 * 0.90) as u64
            });
        }
        assert_eq!(chosen(&table, &max_spare, &request("plan")).seat, "opus");
        assert_eq!(
            chosen(&table, &max_spare, &request("fix.debug")).seat,
            "opus"
        );
        let mut review = request("review.adversary");
        review.not_family = vec!["google".into()];
        assert_eq!(chosen(&table, &max_spare, &review).seat, "opus");
        assert_eq!(
            chosen(&table, &max_spare, &request("build.bounded")).seat,
            "sonnet"
        );
        assert_eq!(
            chosen(&table, &max_spare, &request("build.load_bearing")).seat,
            "opus"
        );
    }

    #[test]
    fn l2_efficient_for_every_class_seats_lowest_pressure_at_efficient_effort() {
        let table = table();
        let world = world();
        for class in &table.classes {
            let mut req = request(&class.id);
            req.mode = Mode::Efficient;
            let pick = chosen(&table, &world, &req);
            let expected_effort = class.efficient_effort.unwrap_or(class.effort);
            assert_eq!(
                pick.effort, expected_effort,
                "class {} in Efficient mode got effort {:?}, expected {:?}",
                class.id, pick.effort, expected_effort
            );
            let winner_seat = table.seats.iter().find(|s| s.id == pick.seat).unwrap();
            assert!(
                !winner_seat.fallback,
                "winner {} should not be fallback",
                winner_seat.id
            );
        }
    }

    #[test]
    fn l3_balanced_on_cool_world_seats_opus_and_sonnet_and_climbs() {
        let table = table();
        let mut world_cool = world();
        let el_week = nirvana_usage::elapsed(
            world_cool
                .readings
                .iter()
                .find(|r| r.pool == "max" && r.window == "week")
                .unwrap(),
        )
        .unwrap();
        // climb 0.5: week used = 0.5 * el_week, 5h used = 0.05
        set(&mut world_cool, "max", "5h", 0.05);
        set(&mut world_cool, "max", "week", 0.50 * el_week);

        let mut req_plan = request("plan");
        req_plan.mode = Mode::Balanced;
        let pick_plan = chosen(&table, &world_cool, &req_plan);
        assert_eq!(pick_plan.seat, "opus");
        assert_eq!(pick_plan.effort, Effort::High);

        let mut req_load = request("build.load_bearing");
        req_load.mode = Mode::Balanced;
        let pick_load = chosen(&table, &world_cool, &req_load);
        assert_eq!(pick_load.seat, "opus");
        assert_eq!(pick_load.effort, Effort::High);

        let mut req_bounded = request("build.bounded");
        req_bounded.mode = Mode::Balanced;
        let pick_bounded = chosen(&table, &world_cool, &req_bounded);
        assert_eq!(pick_bounded.seat, "sonnet");
        assert_eq!(pick_bounded.effort, Effort::Medium);

        // at 0.85: bar is half-way (80) and Max's effort is down one (High -> Medium)
        let mut world_85 = world();
        set(&mut world_85, "max", "5h", 0.05);
        set(&mut world_85, "max", "week", 0.85 * el_week);
        let pick_85 = chosen(&table, &world_85, &req_plan);
        assert!(close(pick_85.bar, 80.0));
        let opus_loser = pick_85.runners_up.iter().find(|l| l.seat == "opus");
        if pick_85.seat == "opus" {
            assert_eq!(pick_85.effort, Effort::Medium);
        } else if let Some(loser) = opus_loser {
            assert_eq!(loser.effort, Some(Effort::Medium));
        }

        // at 1.2: argon or hydrogen wins plan
        let mut world_12 = world();
        set(&mut world_12, "max", "5h", 0.41);
        set(&mut world_12, "max", "week", 1.20 * el_week);
        let pick_12 = chosen(&table, &world_12, &req_plan);
        assert!(
            pick_12.seat == "argon" || pick_12.seat == "hydrogen",
            "at climb 1.2, expected argon or hydrogen to win plan, got {}",
            pick_12.seat
        );
    }

    #[test]
    fn l5_sota_pick_is_the_same_with_no_use_and_with_non_floored_pool_at_95_pct() {
        let table = table();
        let mut world_zero = world();
        for r in &mut world_zero.readings {
            r.used = Some(0.0);
        }

        // World with non-floored pools:
        // Gemini (hydrogen's and carbon's pool) and Argon at 94% (so left = 6% > floor 5%).
        // Max: week at 94%, 5h at 75% (so left = 25% > reserve 20% + expected use).
        // Plan 40a: Fable's own week stays at 0% here. Its fail-closed reserve is the one usage rule SOTA keeps per
        // seat (`scoped_skip`), tested on its own below.
        let mut world_94 = world();
        for r in &mut world_94.readings {
            if r.pool == "max" && r.window == "5h" {
                r.used = Some(0.75);
            } else if r.window == "week.fable" {
                r.used = Some(0.0);
            } else {
                r.used = Some(0.94);
            }
        }

        for class in &table.classes {
            let mut req = request(&class.id);
            req.mode = Mode::Sota;
            let pick_zero = chosen(&table, &world_zero, &req);
            let pick_94 = chosen(&table, &world_94, &req);
            if class.id == "plan" {
                assert_eq!(pick_zero.seat, "fable");
            }
            assert_eq!(
                pick_zero.seat, pick_94.seat,
                "class {} had seat {:?} at 0%, but {:?} at 94%",
                class.id, pick_zero.seat, pick_94.seat
            );
            assert_eq!(
                pick_zero.effort, pick_94.effort,
                "class {} had effort {:?} at 0%, but {:?} at 94%",
                class.id, pick_zero.effort, pick_94.effort
            );
        }

        // Floor and reserve still skip:
        // 1. Reserve skip on max 5h:
        let mut world_reserve = world();
        set(&mut world_reserve, "max", "5h", 0.85); // left = 15% <= 20% reserve
        let mut req_plan = request("plan");
        req_plan.mode = Mode::Sota;
        let pick_res = chosen(&table, &world_reserve, &req_plan);
        assert_eq!(pick_res.seat, "carbon");
        assert!(why(&pick_res.runners_up, "opus").contains("under the orchestrator's reserve"));

        // 2. Floor skip on carbon: plan 70 put carbon in flash's pool, plan 71 hydrogen in flash's place, so gemini's
        // floor skips both.
        let mut world_floor = world_reserve;
        set(&mut world_floor, "gemini", "week", 0.96); // left = 4% <= 5% floor
        let pick_floor = chosen(&table, &world_floor, &req_plan);
        assert!(
            !["carbon", "hydrogen"].contains(&pick_floor.seat.as_str()),
            "{}",
            pick_floor.seat
        );
        assert!(why(&pick_floor.runners_up, "carbon").contains("floor"));
        assert!(why(&pick_floor.runners_up, "hydrogen").contains("floor"));

        // 3. Filter 6 (headroom) is bypassed in SOTA:
        let mut world_f6 = world_zero.clone();
        // Left 30%: Fable's 5 h share since the review round's refit (about 6% at the 2026-10-17 price) keeps the 15%.
        set(&mut world_f6, "max", "5h", 0.70);
        let pick_f6 = chosen(&table, &world_f6, &req_plan);
        assert_eq!(
            pick_f6.seat, "fable",
            "{}: {:?}",
            pick_f6.reason, pick_f6.runners_up
        );
    }

    #[test]
    fn l8_g_opus_wins_plan_only_when_all_non_fallback_seats_are_skipped() {
        // Plan 70: carbon shares hydrogen's pool, so no floor skips it alone; it is turned off here instead.
        let mut table = table();
        table
            .seats
            .iter_mut()
            .find(|s| s.id == "carbon")
            .unwrap()
            .enabled = false;
        let mut world_test = world();
        let mut req_plan = request("plan");
        req_plan.mode = Mode::Sota;

        // Skip opus, carbon, sonnet (argon is off in the shipped table):
        set(&mut world_test, "max", "5h", 0.85); // skips opus and sonnet by reserve
        let pick_flash = chosen(&table, &world_test, &req_plan);
        assert_eq!(pick_flash.seat, "hydrogen");
        assert!(why(&pick_flash.runners_up, "g-opus").contains("fallback seat; hydrogen is not"));

        // Now also skip hydrogen (plan 71: it took flash's place):
        set(&mut world_test, "gemini", "5h", 0.96); // skips hydrogen by floor
        let pick_g_opus = chosen(&table, &world_test, &req_plan);
        assert_eq!(pick_g_opus.seat, "g-opus");
        assert!(pick_g_opus.reason.contains("; fallback:"));
        assert!(pick_g_opus.reason.contains("opus"));
        assert!(pick_g_opus.reason.contains("sonnet"));
        assert!(pick_g_opus.reason.contains("hydrogen"));
    }

    #[test]
    fn section8_argon_record_used_sums_last_7_days_of_booked_and_recorded() {
        let now = 1_000_000;
        let day = 86_400;
        let rec1: RecordLine = serde_json::from_str(&format!(
            r#"{{"order":"o-old","seat":"argon","effort":"medium","class":"plan","outcome":"pass","at":{},"expected":{{"week":0.05}}}}"#,
            now - 8 * day
        )).unwrap();
        let rec2: RecordLine = serde_json::from_str(&format!(
            r#"{{"order":"o-recent","seat":"argon","effort":"medium","class":"plan","outcome":"pass","at":{},"expected":{{"week":0.03}}}}"#,
            now - 2 * day
        )).unwrap();

        let lease1 = Lease {
            order: "o-booked".into(),
            seat: "argon".into(),
            effort: Effort::Medium,
            pool: "argon".into(),
            class: "plan".into(),
            run: None,
            expected: BTreeMap::from([("week".into(), 0.02)]),
            booked_at: now - 3600,
            expires_at: now + 3600,
            pick: serde_json::Value::Null,
        };

        // Booked order already in record should not double count
        let mut lease_dup = lease1.clone();
        lease_dup.order = "o-recent".into();

        let sum = argon_record_used(&[rec1, rec2], &[lease1, lease_dup], now);
        assert!(close(sum, 0.05)); // 0.03 from rec2 + 0.02 from lease1
    }

    #[test]
    fn orchestrator_seat_efficient_mode() {
        let mut table = table();
        table.mode = Mode::Efficient;
        let world = world();
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "sonnet");
        assert_eq!(seat.effort, Effort::Medium);
        assert_eq!(seat.model, "claude-sonnet-5-5");
        assert_eq!(seat.line, "nirvana claude -> sonnet medium: Efficient");
        assert_eq!(seat.square_summary(), "Efficient");
        // A table without `[orchestrator]` (an owner's edited one) seats Efficient as before plan 40a: Sonnet medium.
        let mut old = parse(include_str!("nirvana_seats_v12.toml")).unwrap();
        old.mode = Mode::Efficient;
        assert_eq!(
            orchestrator_seat(&old, &world).line,
            "nirvana claude -> sonnet medium: Efficient"
        );
    }

    #[test]
    fn orchestrator_seat_sota_mode() {
        // Plan 40a: the shipped table seats SOTA on Fable high while the Fable week was read fresh (0% here).
        let mut table = table();
        table.mode = Mode::Sota;
        let world = world();
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "fable");
        assert_eq!(seat.effort, Effort::High);
        assert_eq!(seat.model, "claude-fable-5-2");
        assert_eq!(seat.line, "nirvana claude -> fable high: SOTA");
        assert_eq!(seat.square_summary(), "SOTA");
        // A table without `[orchestrator]` (an owner's edited one) seats as before: Opus high.
        let mut old = parse(include_str!("nirvana_seats_v12.toml")).unwrap();
        old.mode = Mode::Sota;
        let seat = orchestrator_seat(&old, &world);
        assert_eq!(seat.line, "nirvana claude -> opus high: SOTA");
    }

    /// Plan 40a: SOTA takes Fable only when the Fable seat is on, every window it spends was read fresh, and the Fable
    /// week keeps `fable_orchestrator_left`; otherwise `sota_fallback`, and the reason says why.
    #[test]
    fn c40a_sota_orchestrator_falls_back_to_opus_with_why() {
        let mut table = table();
        table.mode = Mode::Sota;
        let line = |table: &Table, world: &World| {
            let seat = orchestrator_seat(table, world);
            (seat.line.clone(), seat.square_summary())
        };
        // The Fable week at 82%: 18% left, under 25%.
        let mut hot = world();
        set(&mut hot, "max", "week.fable", 0.82);
        assert_eq!(
            line(&table, &hot),
            (
                "nirvana claude -> opus high: SOTA, Fable week 82% used".into(),
                "SOTA, Fable week 82% used".into()
            )
        );
        // At 74%: 26% left, Fable stays.
        let mut warm = world();
        set(&mut warm, "max", "week.fable", 0.74);
        assert_eq!(line(&table, &warm).0, "nirvana claude -> fable high: SOTA");
        // Booked Fable orders count against what is left.
        warm.leases.push(lease(
            "o-f",
            "fable",
            "max",
            &[("week.fable", 0.02)],
            NOW + 600,
        ));
        assert_eq!(
            line(&table, &warm).0,
            "nirvana claude -> opus high: SOTA, Fable week 74% used, 2% booked"
        );
        // Not read at all.
        let mut unread = world();
        unread.readings.retain(|r| r.window != "week.fable");
        assert_eq!(line(&table, &unread).1, "SOTA, Fable week not read");
        // Read, but stale.
        let mut stale = world();
        stale.now = NOW + 10_000;
        assert_eq!(
            line(&table, &stale).0,
            "nirvana claude -> opus high: SOTA, Fable week reading is stale"
        );
        // The pool's own windows too: the 5 h not read.
        let mut no_5h = world();
        no_5h
            .readings
            .retain(|r| !(r.pool == "max" && r.window == "5h"));
        assert_eq!(line(&table, &no_5h).1, "SOTA, max 5h not read");
        // The Fable seat off.
        let mut off = table.clone();
        off.seats
            .iter_mut()
            .find(|s| s.id == "fable")
            .unwrap()
            .enabled = false;
        assert_eq!(
            line(&off, &world()).0,
            "nirvana claude -> opus high: SOTA, fable off in seats.toml"
        );
        // An effort the seat does not offer takes the nearest below it.
        let mut capped = table.clone();
        capped.orchestrator.sota.effort = Effort::Max;
        assert_eq!(orchestrator_seat(&capped, &world()).effort, Effort::Xhigh);
        // Every choice off: any claude seat that is on and has no window of its own.
        let mut bare = table.clone();
        for seat in bare
            .seats
            .iter_mut()
            .filter(|s| s.id == "opus" || s.id == "fable")
        {
            seat.enabled = false;
        }
        let seat = orchestrator_seat(&bare, &world());
        assert_eq!(
            (seat.seat.as_str(), seat.effort),
            ("sonnet", Effort::Medium)
        );
    }

    #[test]
    fn orchestrator_seat_balanced_at_t_0_3() {
        let mut table = table();
        table.mode = Mode::Balanced;
        let mut world = world();
        let el_week = nirvana_usage::elapsed(
            world
                .readings
                .iter()
                .find(|r| r.pool == "max" && r.window == "week")
                .unwrap(),
        )
        .unwrap();
        // climb = 0.70 + 0.3 * (1.00 - 0.70) = 0.79 -> t = 0.30
        set(&mut world, "max", "5h", 0.05);
        set(&mut world, "max", "week", 0.79 * el_week);
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "opus");
        assert_eq!(seat.effort, Effort::Medium);
        assert!(close(seat.t.unwrap(), 0.30));
        assert!(close(seat.climb.unwrap(), 0.79));
        assert_eq!(
            seat.line,
            "nirvana claude -> opus medium: Balanced, max week climb 0.79"
        );
    }

    #[test]
    fn orchestrator_seat_balanced_at_t_0_5() {
        let mut table = table();
        table.mode = Mode::Balanced;
        let mut world = world();
        let el_week = nirvana_usage::elapsed(
            world
                .readings
                .iter()
                .find(|r| r.pool == "max" && r.window == "week")
                .unwrap(),
        )
        .unwrap();
        // climb = 0.70 + 0.5 * (1.00 - 0.70) = 0.85 -> t = 0.50
        set(&mut world, "max", "5h", 0.05);
        set(&mut world, "max", "week", 0.85 * el_week);
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "sonnet");
        assert_eq!(seat.effort, Effort::Medium);
        assert!(close(seat.t.unwrap(), 0.50));
        assert!(close(seat.climb.unwrap(), 0.85));
        assert_eq!(
            seat.line,
            "nirvana claude -> sonnet medium: Balanced, max week climb 0.85"
        );
    }

    #[test]
    fn orchestrator_seat_balanced_at_t_1_0() {
        let mut table = table();
        table.mode = Mode::Balanced;
        let mut world = world();
        let el_week = nirvana_usage::elapsed(
            world
                .readings
                .iter()
                .find(|r| r.pool == "max" && r.window == "week")
                .unwrap(),
        )
        .unwrap();
        // climb = 1.18 -> t = 1.00
        set(&mut world, "max", "5h", 0.05);
        set(&mut world, "max", "week", 1.18 * el_week);
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "sonnet");
        assert_eq!(seat.effort, Effort::Medium);
        assert!(close(seat.t.unwrap(), 1.00));
        assert!(close(seat.climb.unwrap(), 1.18));
        assert_eq!(
            seat.line,
            "nirvana claude -> sonnet medium: Balanced, max week climb 1.18"
        );
        assert_eq!(seat.square_summary(), "Balanced, climb 1.18");
    }

    #[test]
    fn orchestrator_seat_balanced_stale_readings() {
        let mut table = table();
        table.mode = Mode::Balanced;
        let mut world = world();
        world.now = NOW + 10_000; // Far in future: all readings stale
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "opus");
        assert_eq!(seat.effort, Effort::Medium);
        assert_eq!(
            seat.line,
            "nirvana claude -> opus medium: Balanced, readings stale"
        );
        assert_eq!(seat.square_summary(), "Balanced, readings stale");
    }

    #[test]
    fn orchestrator_seat_balanced_missing_readings() {
        let mut table = table();
        table.mode = Mode::Balanced;
        let mut world = world();
        world.readings.retain(|r| r.pool != "max");
        let seat = orchestrator_seat(&table, &world);
        assert_eq!(seat.seat, "opus");
        assert_eq!(seat.effort, Effort::Medium);
        assert_eq!(
            seat.line,
            "nirvana claude -> opus medium: Balanced, readings missing"
        );
    }

    fn temp_project(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nirvana-p23-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_p23_project_mode_no_file_gives_machine() {
        let _guard = crate::nirvana::with_test_home("p23-no-file");
        let proj = temp_project("no-file");
        let table = parse(DEFAULT).unwrap();
        let (mode, source) = project_mode(&proj, &table).unwrap();
        assert_eq!(mode, table.mode);
        assert_eq!(source, ModeSource::Machine);
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_project_mode_file_gives_project() {
        let _guard = crate::nirvana::with_test_home("p23-file");
        let proj = temp_project("file");
        let nirvana_dir = proj.join(".nirvana");
        std::fs::create_dir_all(&nirvana_dir).unwrap();
        std::fs::write(nirvana_dir.join(SEATS_FILE), "mode = \"sota\"\n").unwrap();
        let table = parse(DEFAULT).unwrap();
        let (mode, source) = project_mode(&proj, &table).unwrap();
        assert_eq!(mode, Mode::Sota);
        assert_eq!(source, ModeSource::Project);
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_project_mode_refuses_other_key() {
        let _guard = crate::nirvana::with_test_home("p23-other-key");
        let proj = temp_project("other-key");
        let nirvana_dir = proj.join(".nirvana");
        std::fs::create_dir_all(&nirvana_dir).unwrap();
        let file = nirvana_dir.join(SEATS_FILE);
        std::fs::write(&file, "# comment\nother = 123\nmode = \"sota\"\n").unwrap();
        let table = parse(DEFAULT).unwrap();
        let err = project_mode(&proj, &table).unwrap_err();
        assert!(
            err.contains(&file.display().to_string()),
            "must name path: {err}"
        );
        assert!(err.contains("line 2"), "must name line: {err}");
        assert!(
            err.contains("only `mode` is allowed"),
            "must name reason: {err}"
        );
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_project_mode_refuses_bad_value() {
        let _guard = crate::nirvana::with_test_home("p23-bad-val");
        let proj = temp_project("bad-val");
        let nirvana_dir = proj.join(".nirvana");
        std::fs::create_dir_all(&nirvana_dir).unwrap();
        let file = nirvana_dir.join(SEATS_FILE);
        std::fs::write(&file, "mode = \"unknown_mode\"\n").unwrap();
        let table = parse(DEFAULT).unwrap();
        let err = project_mode(&proj, &table).unwrap_err();
        assert!(
            err.contains(&file.display().to_string()),
            "must name path: {err}"
        );
        assert!(err.contains("line 1"), "must name line: {err}");
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_an_empty_or_marked_file_reads_as_the_machine() {
        let _guard = crate::nirvana::with_test_home("p23-empty");
        let proj = temp_project("empty");
        let file = proj.join(".nirvana").join(SEATS_FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let table = parse(DEFAULT).unwrap();
        for text in ["", "# mode = \"sota\"\n", "\n\n"] {
            std::fs::write(&file, text).unwrap();
            assert_eq!(
                project_mode(&proj, &table).unwrap(),
                (table.mode, ModeSource::Machine),
                "{text:?}"
            );
        }
        // An editor's byte-order mark and CRLF endings are still the one key.
        std::fs::write(&file, "\u{feff}mode = \"sota\"\r\n").unwrap();
        assert_eq!(
            project_mode(&proj, &table).unwrap(),
            (Mode::Sota, ModeSource::Project)
        );
        // A refused file is never rewritten by a set, and the refusal says the way out.
        std::fs::write(&file, "mode = \"fast\"\n").unwrap();
        let err = set_project_mode(&proj, Mode::Sota).unwrap_err();
        assert!(err.contains("--clear"), "{err}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "mode = \"fast\"\n");
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_two_projects_different_modes() {
        let guard = crate::nirvana::with_test_home("p23-two-proj");
        let home_seats = guard.path.join("nirvana").join(SEATS_FILE);
        std::fs::create_dir_all(home_seats.parent().unwrap()).unwrap();
        std::fs::write(&home_seats, DEFAULT).unwrap();

        let proj_a = temp_project("proj-a");
        let proj_b = temp_project("proj-b");
        std::fs::create_dir_all(proj_a.join(".nirvana")).unwrap();
        std::fs::create_dir_all(proj_b.join(".nirvana")).unwrap();
        std::fs::write(
            proj_a.join(".nirvana").join(SEATS_FILE),
            "mode = \"efficient\"\n",
        )
        .unwrap();
        std::fs::write(
            proj_b.join(".nirvana").join(SEATS_FILE),
            "mode = \"sota\"\n",
        )
        .unwrap();

        let table_a = table_for(&proj_a).unwrap();
        let table_b = table_for(&proj_b).unwrap();
        assert_eq!(table_a.mode, Mode::Efficient);
        assert_eq!(table_b.mode, Mode::Sota);
        assert_ne!(table_a.mode, table_b.mode);

        let _ = std::fs::remove_dir_all(&proj_a);
        let _ = std::fs::remove_dir_all(&proj_b);
    }

    #[test]
    fn test_p23_pick_mode_flag_beats_project_file() {
        let guard = crate::nirvana::with_test_home("p23-pick-mode");
        let home_seats = guard.path.join("nirvana").join(SEATS_FILE);
        std::fs::create_dir_all(home_seats.parent().unwrap()).unwrap();
        std::fs::write(&home_seats, DEFAULT).unwrap();

        let proj = temp_project("pick-mode");
        std::fs::create_dir_all(proj.join(".nirvana")).unwrap();
        std::fs::write(
            proj.join(".nirvana").join(SEATS_FILE),
            "mode = \"efficient\"\n",
        )
        .unwrap();

        // `herdr seat pick`'s own reading: the project's mode with no flag, the flag's over it.
        let (table, picked) = pick_args_for(&proj, &[]).unwrap();
        assert_eq!(table.mode, Mode::Efficient);
        assert_eq!(picked.request.mode, Mode::Efficient);
        let args = ["--mode".to_string(), "sota".to_string()];
        let (_, picked) = pick_args_for(&proj, &args).unwrap();
        assert_eq!(picked.request.mode, Mode::Sota);
        let _ = std::fs::remove_dir_all(&proj);
    }

    #[test]
    fn test_p23_orchestrator_seat_table_for() {
        let guard = crate::nirvana::with_test_home("p23-orch-table-for");
        let home_seats = guard.path.join("nirvana").join(SEATS_FILE);
        std::fs::create_dir_all(home_seats.parent().unwrap()).unwrap();
        std::fs::write(&home_seats, DEFAULT).unwrap();

        let proj_eff = temp_project("orch-eff");
        let proj_sota = temp_project("orch-sota");
        std::fs::create_dir_all(proj_eff.join(".nirvana")).unwrap();
        std::fs::create_dir_all(proj_sota.join(".nirvana")).unwrap();
        std::fs::write(
            proj_eff.join(".nirvana").join(SEATS_FILE),
            "mode = \"efficient\"\n",
        )
        .unwrap();
        std::fs::write(
            proj_sota.join(".nirvana").join(SEATS_FILE),
            "mode = \"sota\"\n",
        )
        .unwrap();

        let (model_eff, effort_eff) =
            crate::nirvana_launch::orchestrator_launch_seat_for(&guard.path, &proj_eff);
        let (model_sota, effort_sota) =
            crate::nirvana_launch::orchestrator_launch_seat_for(&guard.path, &proj_sota);

        assert!(model_eff.contains("sonnet"));
        assert_eq!(effort_eff, "medium");
        assert!(model_sota.contains("opus"));
        assert_eq!(effort_sota, "high");

        // The argv each space's Nirvana Claude is started with carries its own project's seat.
        let flag = |args: &[String], name: &str| {
            let at = args.iter().position(|arg| arg == name).expect(name);
            args[at + 1].clone()
        };
        let eff = crate::nirvana_launch::launch_args(&guard.path, &proj_eff, None);
        let sota = crate::nirvana_launch::launch_args(&guard.path, &proj_sota, None);
        assert!(flag(&eff, "--model").contains("sonnet"), "{eff:?}");
        assert_eq!(flag(&eff, "--effort"), "medium");
        assert!(flag(&sota, "--model").contains("opus"), "{sota:?}");
        assert_eq!(flag(&sota, "--effort"), "high");

        let _ = std::fs::remove_dir_all(&proj_eff);
        let _ = std::fs::remove_dir_all(&proj_sota);
    }

    /// Plan 37 §1.3: the next stronger seat of the class, by the class's score and then the seat's output price:
    /// research.web scores hydrogen and carbon 84 each, and carbon's price is the higher; carbon has none above it.
    /// Plan 70: carbon took argon's place, argon is off. Plan 71: hydrogen took flash's place, flash is off.
    #[test]
    fn p37_step_up_takes_the_next_stronger_seat_of_the_class() {
        let table = parse(DEFAULT).expect("default table");
        assert_eq!(
            step_up(&table, "research.web", "hydrogen", NOW).as_deref(),
            Some("carbon")
        );
        assert_eq!(step_up(&table, "research.web", "carbon", NOW), None);
        // From a lower score the nearest score above is taken (opus 82), not the top one (84); g-sonnet ties sonnet
        // on score and price, so it is not above it.
        assert_eq!(
            step_up(&table, "research.web", "sonnet", NOW).as_deref(),
            Some("opus")
        );
        assert_eq!(step_up(&table, "research.web", "haiku", NOW), None);
        assert_eq!(step_up(&table, "no.such.class", "flash", NOW), None);
    }

    /// Plan 35 R2 (live check 35.1, F1): every enabled agy seat lists `browser`, so `--needs browser` can seat agy;
    /// research.web keeps its scores (one order is not a measurement), and its `why` says so.
    #[test]
    fn lrs_agy_seats_list_browser_and_a_browser_order_can_seat_agy() {
        let table = parse(DEFAULT).unwrap();
        let agy: Vec<&Seat> = table
            .seats
            .iter()
            .filter(|seat| seat.cli == "agy" && seat.enabled)
            .collect();
        assert_eq!(
            agy.iter().map(|seat| seat.id.as_str()).collect::<Vec<_>>(),
            ["g-opus", "g-sonnet", "carbon", "hydrogen"]
        );
        for seat in &agy {
            assert!(
                seat.capabilities.iter().any(|word| word == "browser"),
                "{} lacks browser",
                seat.id
            );
        }
        let mut browser = request("research.web");
        browser.needs = vec!["browser".into()];
        let got = chosen(&table, &world(), &browser);
        assert_eq!(
            table.seat(&got.seat).map(|seat| seat.cli.as_str()),
            Some("agy"),
            "{got:?}"
        );
        // Before R2 (the default plan 35c shipped, v8) the same order went to a claude seat, every agy seat lacking it.
        assert!(HISTORICAL_DEFAULTS.contains(&LRS_V8));
        let before = parse(LRS_V8).unwrap();
        let got = chosen(&before, &world(), &browser);
        assert_eq!(why(&got.runners_up, "flash"), "lacks browser");
        let web = table.class("research.web").unwrap();
        let score = |table: &Table, seat: &str| {
            table
                .class("research.web")
                .and_then(|class| class.seats.iter().find(|entry| entry.seat == seat))
                .map(|entry| entry.score)
        };
        for seat in ["flash", "argon", "sonnet", "opus", "g-sonnet"] {
            assert_eq!(score(&table, seat), score(&before, seat), "{seat} rescored");
        }
        assert!(web.why.contains("not rescored"), "{}", web.why);
        // A desktop server still takes a claude seat: `claude_only` skips agy whatever it lists.
        let mut desktop = browser.clone();
        desktop.claude_only = Some("bambu-studio-beta drives a desktop window".into());
        let got = chosen(&table, &world(), &desktop);
        assert_eq!(
            table.seat(&got.seat).map(|seat| seat.cli.as_str()),
            Some("claude")
        );
        assert_eq!(
            why(&got.runners_up, "hydrogen"),
            "bambu-studio-beta drives a desktop window"
        );
        // The seat skill says `--needs browser` is the way to a browser on agy, and no longer only `bladebro`.
        let skill = include_str!("../nirvana/claude/skills/seat/SKILL.md");
        assert!(
            skill.contains("so `--needs browser` is the way to a browser on agy too"),
            "{skill}"
        );
    }

    const LRS_V8: &str = include_str!("nirvana_seats_v8.toml");

    /// An installed copy of plan 34a's default (v7, which plan 35c left without a copy), of plan 35c's (v8), or of one
    /// of the three intermediates that had none (v9-v11) upgrades to the default with `browser` on agy, its mode kept
    /// and the old file backed up. By content, not by index: the next copy shifts no number here.
    #[test]
    fn lrs_an_installed_34a_or_35c_default_upgrades_to_r2() {
        for (n, old) in [
            (7, include_str!("nirvana_seats_v7.toml")),
            (8, LRS_V8),
            (9, include_str!("nirvana_seats_v9.toml")),
            (10, include_str!("nirvana_seats_v10.toml")),
            (11, include_str!("nirvana_seats_v11.toml")),
            (12, include_str!("nirvana_seats_v12.toml")),
            (13, include_str!("nirvana_seats_v13.toml")),
        ] {
            assert!(HISTORICAL_DEFAULTS.contains(&old), "v{n} is historical");
            assert_ne!(old.replace("\r\n", "\n"), DEFAULT.replace("\r\n", "\n"));
            let dir = temp_dir(&format!("lrs-upgrade-v{n}"));
            let path = dir.join(SEATS_FILE);
            let installed = replace_mode_line(old, "mode = \"efficient\"");
            std::fs::write(&path, &installed).unwrap();
            let text = read_or_create(&path);
            assert_eq!(
                text,
                replace_mode_line(DEFAULT, "mode = \"efficient\""),
                "v{n}"
            );
            assert_eq!(
                std::fs::read_to_string(dir.join("backup").join(SEATS_FILE)).unwrap(),
                installed
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    // ---- plan 40a ----

    /// Plan 40a step 1: the shipped default parses clean and is none of the historical ones, so `maybe_upgrade` moves
    /// every one of them to it; and none of them reads as "differs" (the next read upgrades it).
    #[test]
    fn c40a_the_shipped_default_is_new_and_every_old_one_upgrades() {
        let table = parse(DEFAULT).expect("the shipped default parses");
        assert!(table.problems().is_empty());
        for (n, old) in HISTORICAL_DEFAULTS.iter().enumerate() {
            assert_ne!(
                canonical(old),
                canonical(DEFAULT),
                "v{} is the default",
                n + 1
            );
            assert!(!differs_from_default(old), "v{}", n + 1);
        }
        assert_eq!(
            HISTORICAL_DEFAULTS.last().copied(),
            Some(include_str!("nirvana_seats_v14.toml"))
        );
        // Plan 71: v14 went after v13, which stays historical (control p70_v13_not_historical).
        assert!(HISTORICAL_DEFAULTS.contains(&include_str!("nirvana_seats_v13.toml")));
        assert!(!differs_from_default(DEFAULT));
        assert!(!differs_from_default(&replace_mode_line(
            DEFAULT,
            "mode = \"sota\""
        )));
    }

    /// Review M2: an edited table says so in one line, and `seat diff` shows the change and not the mode.
    #[test]
    fn c40a_an_edited_table_differs_and_seat_diff_shows_the_change() {
        let edited =
            replace_mode_line(DEFAULT, "mode = \"sota\"").replace("floor = 0.05", "floor = 0.08");
        assert!(differs_from_default(&edited));
        let diff = diff_to_default(&edited, "seats.toml");
        assert!(
            diff.starts_with("--- seats.toml\n+++ herdr's default\n@@ "),
            "{diff}"
        );
        assert!(diff.contains("\n-floor = 0.08\n+floor = 0.05\n"), "{diff}");
        assert!(
            !diff.contains("mode ="),
            "the owner's mode is never a change: {diff}"
        );
        assert_eq!(diff.matches("@@ -").count(), 1, "{diff}");
        assert_eq!(
            diff_to_default(&replace_mode_line(DEFAULT, "mode = \"sota\""), "x"),
            "seats.toml is herdr's default.\n"
        );
        let text = list_text(&table(), &world(), Path::new("home"));
        assert!(
            !text.contains(DIFFERS_LINE),
            "list_text adds it in list_cli only"
        );
    }

    /// One `max` window set to a share, or dropped with `None`.
    fn with_fable_week(used: Option<f32>) -> World {
        let mut world = world();
        match used {
            Some(used) => set(&mut world, "max", "week.fable", used),
            None => world.readings.retain(|r| r.window != "week.fable"),
        }
        world
    }

    /// Plan 40a: Fable fails closed. Not read, read stale, or a reserve it would break: skipped with why, in every
    /// mode, SOTA included, and an override is refused for the same reason.
    #[test]
    fn c40a_fable_is_skipped_unless_its_windows_are_fresh_and_its_reserve_holds() {
        let table = table();
        let sota = |world: &World| {
            let mut req = request("plan");
            req.mode = Mode::Sota;
            pick(&table, world, &req)
        };
        // Fresh at 0%: SOTA seats it.
        assert_eq!(sota(&with_fable_week(Some(0.0))).unwrap().seat, "fable");
        // Not read.
        let got = sota(&with_fable_week(None)).unwrap();
        assert_eq!(got.seat, "opus");
        assert_eq!(
            why(&got.runners_up, "fable"),
            "Fable week not read; a headless Fable run past its week bills usage credits without asking"
        );
        // Stale: a reading three hours old.
        let mut stale = with_fable_week(Some(0.0));
        for r in stale
            .readings
            .iter_mut()
            .filter(|r| r.window == "week.fable")
        {
            r.read_at = NOW - 3 * 3600;
        }
        let got = sota(&stale).unwrap();
        assert_eq!(got.seat, "opus");
        assert!(why(&got.runners_up, "fable").starts_with("Fable week reading is stale; "));
        // The pool's own window, not read, skips it too.
        let mut no_week = with_fable_week(Some(0.0));
        no_week
            .readings
            .retain(|r| !(r.pool == "max" && r.window == "week"));
        assert!(why(
            &sota(&no_week).unwrap_or_else(|_| panic!()).runners_up,
            "fable"
        )
        .starts_with("max week not read; "));
        // Over the reserve: 91% used leaves 9% before the order, under 10% after it. The general floor (5%) would not
        // have caught it.
        let got = sota(&with_fable_week(Some(0.91))).unwrap();
        assert_eq!(got.seat, "opus");
        assert!(
            why(&got.runners_up, "fable").starts_with("would leave Fable week at ")
                && why(&got.runners_up, "fable").contains(", under the reserve 10%; "),
            "{}",
            why(&got.runners_up, "fable")
        );
        // 85% used leaves 15% less the order (about 4% since the review round's refit): it holds.
        assert_eq!(sota(&with_fable_week(Some(0.85))).unwrap().seat, "fable");
        // The general floor still applies: the 5 h at 96%.
        let mut floored = with_fable_week(Some(0.0));
        set(&mut floored, "max", "5h", 0.96);
        assert!(why(&sota(&floored).unwrap().runners_up, "fable").contains("floor"));
        // An override is refused for the same reason, and so is an inherited one.
        for inherited in [false, true] {
            let mut over = request("plan");
            over.mode = Mode::Sota;
            over.override_seat = Some(("fable:xhigh".into(), "hard plan".into()));
            over.inherited_override = inherited;
            let refused = pick(&table, &with_fable_week(None), &over).unwrap_err();
            assert!(
                refused
                    .reason
                    .starts_with("override fable:xhigh refused: Fable week not read; "),
                "{}",
                refused.reason
            );
        }
        // Fresh, an override to xhigh is taken (the rule never lifts above high by itself).
        let mut over = request("plan");
        over.override_seat = Some(("fable:xhigh".into(), "hard plan".into()));
        let got = pick(&table, &with_fable_week(Some(0.0)), &over).unwrap();
        assert_eq!((got.seat.as_str(), got.effort), ("fable", Effort::Xhigh));
        // A seat without a window of its own is not touched by the rule: Opus with week.fable unread.
        assert!(scoped_skip(
            &table,
            &with_fable_week(None),
            table.seat("opus").unwrap(),
            &[]
        )
        .is_none());
    }

    /// Plan 40a step 2: what each mode picks for the six classes that list Fable, on three worlds. Expected before the
    /// run (the order's): SOTA seats Fable in the first world and Opus in the other two; Balanced and Efficient seat
    /// Fable in none. The table is printed for the record (`--nocapture`).
    #[test]
    fn c40a_the_pick_table_on_three_worlds() {
        let table = table();
        let classes = [
            "plan",
            "build.load_bearing",
            "review.adversary",
            "fix.debug",
            "build.frontend",
            "judge.visual",
        ];
        let worlds = [
            ("fable week 2%", with_fable_week(Some(0.02))),
            ("fable week 95%", with_fable_week(Some(0.95))),
            ("fable week unread", with_fable_week(None)),
        ];
        for (name, world) in &worlds {
            for class in classes {
                let mut row = Vec::new();
                for mode in [Mode::Efficient, Mode::Balanced, Mode::Sota] {
                    let mut req = request(class);
                    req.mode = mode;
                    let got = chosen(&table, world, &req);
                    row.push(format!(
                        "{} {} {}",
                        mode.word(),
                        got.seat,
                        got.effort.word()
                    ));
                    let fable = got.seat == "fable";
                    let want = mode == Mode::Sota && *name == "fable week 2%";
                    assert_eq!(
                        fable,
                        want,
                        "{name} {class} {}: {}",
                        mode.word(),
                        got.reason
                    );
                    if mode == Mode::Sota && !want {
                        assert_eq!(got.seat, "opus", "{name} {class}");
                    }
                }
                println!("c40a pick | {name} | {class} | {}", row.join(" | "));
            }
        }
    }

    /// Plan 40a: `step_up` (plan 37's redo after invented sources) takes Fable as the last rung of a class that lists
    /// it, and the redo's override then meets the fail-closed rule like any other.
    #[test]
    fn c40a_step_up_ends_at_fable_through_the_same_check() {
        let table = table();
        let now = date("2026-11-10").unwrap();
        assert_eq!(
            step_up(&table, "plan", "opus", now).as_deref(),
            Some("fable")
        );
        assert_eq!(step_up(&table, "plan", "fable", now), None);
        // Today's research.web (no Fable): a sent-back opus order steps up to hydrogen, 84 over 82, the cheaper
        // of the two (plan 71: hydrogen took flash's place).
        assert_eq!(
            step_up(&table, "research.web", "opus", now).as_deref(),
            Some("hydrogen")
        );
        let mut redo = request("plan");
        redo.override_seat = Some(("fable".into(), "step up".into()));
        redo.inherited_override = true;
        assert!(pick(&table, &with_fable_week(None), &redo)
            .unwrap_err()
            .reason
            .contains("Fable week not read"));
    }

    /// Review B1: Jev is asked with the lease lock free, and the pick under the lock keeps the advice only when it was
    /// made among the same seats.
    #[test]
    fn c40a_jev_is_asked_outside_the_lease_lock() {
        let dir = temp_dir("c40a-b1");
        let table = table();
        let req = request("build.bounded");
        let mut asked = false;
        let out = pick_and_book(
            &dir,
            &table,
            World::default(),
            &req,
            Some("o-b1"),
            None,
            Vec::new(),
            |_, pick| {
                asked = true;
                // Another herdr takes the lock while Jev is being asked: it must get it at once.
                let other = Lock::try_take(&dir, 1);
                assert!(other.is_ok(), "the lease lock is held across Jev's call");
                pick
            },
        );
        assert!(asked);
        assert!(matches!(out, Ok(Booking::Seat(_))));
        assert!(load_leases(&dir.join(LEASES_FILE))
            .unwrap()
            .live("o-b1", now())
            .is_some());
        // Advice from a contest the bookings changed is dropped with why.
        let rule = chosen(&table, &world(), &req);
        let mut moved = rule.clone();
        moved.contenders.pop();
        assert!(!same_contest(&rule, &moved));
        assert!(same_contest(&rule, &rule.clone()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review T2: a sign-in answer is kept a minute in `signin.json`, then read again.
    #[test]
    fn c40a_a_sign_in_is_read_once_a_minute() {
        let dir = temp_dir("c40a-t2");
        let mut reads = 0;
        let mut read = |why: Option<&str>| {
            cached_cli_state(&dir, "claude", NOW, || {
                reads += 1;
                why.map(str::to_owned)
            })
        };
        assert_eq!(read(None), None);
        assert_eq!(read(Some("claude not signed in")), None, "kept");
        assert_eq!(reads, 1);
        let later = cached_cli_state(&dir, "claude", NOW + SIGNIN_KEEP_S, || {
            Some("claude not signed in".into())
        });
        assert_eq!(later.as_deref(), Some("claude not signed in"));
        // Another CLI has its own answer.
        assert_eq!(
            cached_cli_state(&dir, "agy", NOW, || Some("agy not signed in".into())).as_deref(),
            Some("agy not signed in")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The test module's readings moved to the wall clock (`pick_and_book` reads `now()`), the Fable week at `fable`
    /// (`None` not read), every reading `age` seconds old.
    fn world_now(fable: Option<f32>, age: u64) -> World {
        let shift = now().saturating_sub(NOW);
        let mut world = with_fable_week(fable);
        for reading in &mut world.readings {
            reading.read_at = now() - age;
            reading.resets_at = reading.resets_at.map(|at| at + shift);
        }
        world.now = now();
        world
    }

    fn sota_plan() -> Request {
        let mut req = request("plan");
        req.mode = Mode::Sota;
        req.order = Some("o-s1".into());
        req
    }

    fn book_again(dir: &Path, table: &Table, world: World) -> Booking {
        let req = sota_plan();
        pick_and_book(
            dir,
            table,
            world,
            &req,
            Some("o-s1"),
            None,
            Vec::new(),
            |_, p| p,
        )
        .unwrap_or_else(|_| panic!("pick_and_book errs"))
    }

    fn booked_seat(out: &Booking) -> Option<(String, String)> {
        match out {
            Booking::Seat(out) => Some((
                out["seat"].as_str().unwrap_or("").to_owned(),
                out["booked"].as_str().unwrap_or("").to_owned(),
            )),
            Booking::None(..) => None,
        }
    }

    /// Package review a, S1: a live Fable booking is answered again only while Fable may spend. A week that went stale
    /// or ran under the reserve since the booking refuses it, releases the lease, and says what to do first; a fresh
    /// week reuses it; a seat with no scoped window is reused untouched while the Fable week is not read at all.
    #[test]
    fn c40a_s1_a_reused_fable_lease_is_checked_again() {
        let table = table();
        let fresh = || world_now(Some(0.02), 30);
        for (name, later, refused) in [
            (
                "stale",
                world_now(Some(0.02), STALE_AFTER_TEST),
                Some("Fable week reading is stale"),
            ),
            (
                "over the reserve",
                world_now(Some(0.95), 30),
                Some("under the reserve"),
            ),
            ("fresh", fresh(), None),
        ] {
            let dir = temp_dir(&format!("c40a-s1-{}", name.replace(' ', "-")));
            let first = book_again(&dir, &table, fresh());
            assert_eq!(
                booked_seat(&first),
                Some(("fable".into(), "new".into())),
                "{name}"
            );
            match (book_again(&dir, &table, later), refused) {
                (Booking::None(no_seat, _), Some(why)) => {
                    assert!(
                        no_seat
                            .reason
                            .starts_with("Book o-s1 again with `herdr seat pick --book o-s1`"),
                        "{name}: {}",
                        no_seat.reason
                    );
                    assert!(no_seat.reason.contains(why), "{name}: {}", no_seat.reason);
                    let leases = load_leases(&dir.join(LEASES_FILE)).unwrap();
                    assert!(
                        leases.live("o-s1", now()).is_none(),
                        "{name}: the lease stays"
                    );
                }
                (out @ Booking::Seat(_), None) => {
                    assert_eq!(booked_seat(&out), Some(("fable".into(), "existing".into())));
                }
                (Booking::Seat(out), Some(_)) => panic!("{name}: reused {out}"),
                (Booking::None(no_seat, _), None) => panic!("{name}: refused {}", no_seat.reason),
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
        // A seat with no scoped window: booked on opus while the Fable week is not read, reused as it was.
        let dir = temp_dir("c40a-s1-opus");
        let first = book_again(&dir, &table, world_now(None, 30));
        assert_eq!(booked_seat(&first), Some(("opus".into(), "new".into())));
        let again = book_again(&dir, &table, world_now(None, 30));
        assert_eq!(
            booked_seat(&again),
            Some(("opus".into(), "existing".into()))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    const STALE_AFTER_TEST: u64 = nirvana_usage::STALE_AFTER + 60;

    /// The review round, Part 2 items 4 and 5: the sizes and the two scores the record set (`nirvana/seat_fit.py`, the
    /// usage audit's finding 7). Flash is no longer seated for load-bearing work at Efficient; sonnet clears the
    /// adversary review's Balanced bar.
    #[test]
    fn c40a_the_record_sets_the_sizes_and_two_scores() {
        let table = table();
        assert!(close(table.size(SizeName::Small).use_factor, 3.0));
        assert!(close(table.size(SizeName::Medium).use_factor, 5.0));
        assert!(close(table.size(SizeName::Large).use_factor, 4.1));
        let score = |class: &str, seat: &str| {
            let class = table.class(class).unwrap();
            let entry = class.seats.iter().find(|e| e.seat == seat).unwrap();
            (entry.score, table.bars(class))
        };
        let (flash, bars) = score("build.load_bearing", "flash");
        assert!(flash < bars.at(Mode::Efficient), "flash {flash}");
        let (sonnet, bars) = score("review.adversary", "sonnet");
        assert!(sonnet >= bars.at(Mode::Balanced), "sonnet {sonnet}");
        let mut req = request("build.load_bearing");
        req.mode = Mode::Efficient;
        req.not_family = vec!["anthropic".into()];
        let got = pick(&table, &world(), &req);
        assert!(!got.as_ref().is_ok_and(|p| p.seat == "flash"), "{got:?}");
        // Plan 71: hydrogen took flash's row at flash's score, a prior, so it stays under the bar too and is not
        // picked.
        let (hydrogen, bars) = score("build.load_bearing", "hydrogen");
        assert!(hydrogen < bars.at(Mode::Efficient), "hydrogen {hydrogen}");
        assert!(!got.as_ref().is_ok_and(|p| p.seat == "hydrogen"), "{got:?}");
    }

    /// Review a, N4: an owner's table that turns sonnet and opus off seats Nirvana Claude on the first claude seat still
    /// on, and the reason says each seat passed over and why; with every claude seat off, the last resort says so.
    #[test]
    fn c40a_n4_an_off_seat_is_named_in_the_orchestrators_reason() {
        let mut table = table();
        table.mode = Mode::Efficient;
        for seat in table.seats.iter_mut() {
            if seat.id == "sonnet" || seat.id == "opus" {
                seat.enabled = false;
            }
        }
        let seat = orchestrator_seat(&table, &world());
        assert_eq!(
            seat.line,
            "nirvana claude -> haiku medium: Efficient, sonnet off in seats.toml, opus off in seats.toml, \
             no named seat can be taken, so the first claude seat on"
        );
        for seat in table.seats.iter_mut().filter(|seat| seat.cli == "claude") {
            seat.enabled = false;
        }
        let seat = orchestrator_seat(&table, &world());
        assert!(
            seat.line
                .ends_with("every claude seat is off, so opus medium as the last resort"),
            "{}",
            seat.line
        );
    }

    /// The review round, Part 2 item 6: old and new outcome words, and the last line for one order and run decides.
    #[test]
    fn c40a_tally_reads_old_and_new_words_and_the_last_line_decides() {
        let line = |order: &str, run: &str, outcome: &str| -> RecordLine {
            serde_json::from_value(serde_json::json!({
                "order": order, "run": run, "seat": "sonnet", "effort": "medium",
                "class": "build.bounded", "chain": "build.bounded", "outcome": outcome,
            }))
            .unwrap()
        };
        for (word, want) in [
            ("pass", Some(true)),
            ("accepted", Some(true)),
            ("failed", Some(false)),
            ("sent_back", Some(false)),
            ("blocked", None),
            ("subagent-no-report", None),
            ("subagent-prose-report", None),
            ("delivered", None),
            ("reseated", None),
            ("asked", None),
            ("stopped", None),
            ("a word from next year", None),
        ] {
            assert_eq!(tally_word(word), want, "{word}");
        }
        let key = (
            "sonnet".to_owned(),
            Effort::Medium,
            "build.bounded".to_owned(),
        );
        let lines = vec![
            line("o-1", "r-1", "failed"),
            line("o-1", "r-1", "accepted"),
            line("o-2", "r-1", "pass"),
            line("o-2", "r-1", "delivered"),
            line("o-1", "r-2", "sent_back"),
            line("o-3", "r-1", "subagent-no-report"),
        ];
        // o-1 r-1 accepted (its last line), o-2 r-1 neither (its last is delivered), o-1 r-2 a failure, o-3 neither.
        assert_eq!(
            tally(&lines)[&key],
            Tally {
                passes: 1,
                orders: 2
            }
        );
    }

    /// S1's question by itself: the order's own booking is not counted against it, others' are, an effort raised past
    /// the booking is priced at that effort, and a seat with no scoped window is never asked.
    #[test]
    fn c40a_s1_may_spend_now_reads_the_world_as_it_is() {
        let table = table();
        let mut world = with_fable_week(Some(0.80));
        let ask = |world: &World, effort: Effort| {
            may_spend_now(
                &table,
                world,
                "fable",
                effort,
                "plan",
                SizeName::Large,
                "o-1",
            )
        };
        assert_eq!(ask(&world, Effort::High), Ok(()));
        let mut own = Lease {
            order: "o-1".into(),
            seat: "fable".into(),
            effort: Effort::High,
            pool: "max".into(),
            class: "plan".into(),
            run: None,
            expected: BTreeMap::from([("week.fable".into(), 0.15)]),
            booked_at: NOW,
            expires_at: NOW + 3600,
            pick: serde_json::Value::Null,
        };
        world.leases = vec![own.clone()];
        assert_eq!(ask(&world, Effort::High), Ok(()), "its own booking counted");
        own.order = "o-2".into();
        world.leases = vec![own];
        assert!(ask(&world, Effort::High)
            .unwrap_err()
            .contains("under the reserve"));
        assert!(may_spend_now(
            &table,
            &with_fable_week(None),
            "sonnet",
            Effort::Medium,
            "plan",
            SizeName::Large,
            "o-1"
        )
        .is_ok());
        assert!(ask(&with_fable_week(None), Effort::High)
            .unwrap_err()
            .starts_with("fable: Fable week not read"));
    }

    // ---- plan 70 ----

    /// Plan 70: Gemini 4 Carbon takes Argon's seat. The owner, 2026-11-10: "Check the cost and if its the same it
    /// might be used to replace Argon." Measured one order per effort: low 1.03x Argon's quota, medium 1.10x its tokens
    /// at list price, both right. Carbon is on in gemini's pool with Argon's price rows; Argon stays in the table, off.
    #[test]
    fn p70_carbon_takes_argons_seat_at_the_same_cost() {
        let table = table();
        let carbon = table.seat("carbon").expect("a carbon seat");
        assert!(carbon.enabled);
        assert_eq!(
            (
                carbon.cli.as_str(),
                carbon.model.as_str(),
                carbon.family.as_str()
            ),
            ("agy", "gemini-4-carbon-{effort}", "google")
        );
        // Its pool is the one its orders moved: agy's "Gemini Models" group, flash's pool.
        assert_eq!(carbon.pool, "gemini");
        assert_eq!(carbon.efforts, [Effort::Low, Effort::Medium, Effort::High]);
        let argon = table.seat("argon").expect("argon stays in the table");
        assert!(!argon.enabled, "argon is off since plan 70");
        assert_eq!(carbon.capabilities, argon.capabilities);
        // Argon's price rows, from the same dates on.
        for day in ["2026-11-29", "2026-11-30"] {
            let (c, a) = (
                carbon.price_at(date(day).unwrap()).unwrap(),
                argon.price_at(date(day).unwrap()).unwrap(),
            );
            assert_eq!(
                (c.input, c.output, c.cache_read, c.cache_write),
                (a.input, a.output, a.cache_read, a.cache_write),
                "{day}"
            );
            assert!(
                c.basis.contains("introducing-gemini-4-carbon")
                    || c.basis.contains("same two sources")
            );
        }
        // Low's cells are the measured probe's quota scaled to a real order by flash's fit x8.97, the gemini
        // pool's only fit: unscaled, carbon would read a ninth of its cost beside the fitted seats.
        let cell = |effort: Effort| carbon.uses.iter().find(|u| u.effort == effort).unwrap();
        assert!(close(cell(Effort::Low).windows["week"], 0.000167 * 8.97));
        assert!(close(cell(Effort::Low).windows["5h"], 0.000667 * 8.97));
        assert!(cell(Effort::Low).basis.contains("fit x8.97"));
        assert!(cell(Effort::Low).basis.starts_with("measured: plan 70"));
        assert!(cell(Effort::Medium).basis.starts_with("derived: plan 70"));
        assert!(cell(Effort::High).basis.starts_with("prior:"));
        // Every class that lists argon lists carbon just before it, at argon's score and effort.
        let mut classes = 0;
        for class in &table.classes {
            let Some(at) = class.seats.iter().position(|e| e.seat == "argon") else {
                assert!(
                    class.seats.iter().all(|e| e.seat != "carbon"),
                    "{}",
                    class.id
                );
                continue;
            };
            classes += 1;
            let (c, a) = (&class.seats[at - 1], &class.seats[at]);
            assert_eq!(c.seat, "carbon", "{}", class.id);
            assert_eq!(
                (c.score, c.effort, &c.only_with),
                (a.score, a.effort, &a.only_with),
                "{}",
                class.id
            );
        }
        assert_eq!(classes, 14);
        // The rule takes carbon where it took argon: video, and the step up in research.web.
        let mut video = request("media.video");
        video.needs = vec!["video".into()];
        assert_eq!(chosen(&table, &world(), &video).seat, "carbon");
        assert_eq!(
            step_up(&table, "research.web", "hydrogen", NOW).as_deref(),
            Some("carbon")
        );
        // An override of argon is refused: it is off.
        let mut over = request("build.bounded");
        over.override_seat = Some(("argon".into(), "x".into()));
        assert!(pick(&table, &world(), &over).is_err());
    }

    // ---- plan 71 ----

    /// Plan 71: Gemini 4 Hydrogen takes Flash 3.8's seat. The owner, 2026-11-12: "it supposidly costs the same as 3.8"
    /// and "Hydrogen can do media level work replace flash". Ten orders, all right; by tokens at Flash's price Hydrogen
    /// cost 0.74x at low, 0.71x at medium, 0.57x at high (the quota could not be read). Hydrogen is on in gemini's pool
    /// with Flash's price row and capabilities; Flash stays in the table, off.
    #[test]
    fn p71_hydrogen_takes_flashs_seat_at_the_same_cost() {
        let table = table();
        let hydrogen = table.seat("hydrogen").expect("a hydrogen seat");
        assert!(hydrogen.enabled);
        assert_eq!(
            (
                hydrogen.cli.as_str(),
                hydrogen.model.as_str(),
                hydrogen.family.as_str(),
                hydrogen.pool.as_str()
            ),
            ("agy", "gemini-4-hydrogen-{effort}", "google", "gemini")
        );
        // agy lists xhigh for Hydrogen (agy 1.5.7); the rule still never picks above high on its own.
        assert_eq!(
            hydrogen.efforts,
            [Effort::Low, Effort::Medium, Effort::High, Effort::Xhigh]
        );
        let flash = table.seat("flash").expect("flash stays in the table");
        assert!(!flash.enabled, "flash is off since plan 71");
        // The owner's word: Hydrogen does media work, so it takes Flash's capabilities, video and audio with them.
        assert_eq!(hydrogen.capabilities, flash.capabilities);
        assert!(hydrogen.capabilities.iter().any(|c| c == "video"));
        assert!(hydrogen.capabilities.iter().any(|c| c == "audio"));
        let day = date("2026-11-12").unwrap();
        let (h, f) = (
            hydrogen.price_at(day).unwrap(),
            flash.price_at(day).unwrap(),
        );
        assert_eq!(
            (h.input, h.output, h.cache_read, h.cache_write),
            (f.input, f.output, f.cache_read, f.cache_write)
        );
        assert!(h.basis.contains("unverified"), "{}", h.basis);
        // Each cell is Flash's cell times the measured ratio at that effort, so it keeps Flash's fit to real orders.
        let cell = |seat: &Seat, effort: Effort| {
            seat.uses
                .iter()
                .find(|u| u.effort == effort)
                .unwrap()
                .windows
                .clone()
        };
        for (effort, ratio) in [
            (Effort::Low, 0.74),
            (Effort::Medium, 0.71),
            (Effort::High, 0.57),
        ] {
            let (h, f) = (cell(hydrogen, effort), cell(flash, effort));
            for window in ["5h", "week"] {
                assert!(
                    (h[window] / (f[window] * ratio) - 1.0).abs() < 0.01,
                    "{effort:?} {window}: {} against {} x {ratio}",
                    h[window],
                    f[window]
                );
            }
        }
        let xhigh = cell(hydrogen, Effort::Xhigh);
        assert!(close(
            xhigh["week"],
            cell(hydrogen, Effort::High)["week"] * 1.6
        ));
        // Every class that lists flash lists hydrogen just before it, at flash's score and effort.
        let mut classes = 0;
        for class in &table.classes {
            let Some(at) = class.seats.iter().position(|e| e.seat == "flash") else {
                assert!(
                    class.seats.iter().all(|e| e.seat != "hydrogen"),
                    "{}",
                    class.id
                );
                continue;
            };
            classes += 1;
            let (h, f) = (&class.seats[at - 1], &class.seats[at]);
            assert_eq!(h.seat, "hydrogen", "{}", class.id);
            assert_eq!(
                (h.score, h.effort, &h.only_with),
                (f.score, f.effort, &f.only_with),
                "{}",
                class.id
            );
        }
        assert_eq!(classes, 20);
        // The rule takes hydrogen where it took flash: audio, and the huge read.
        for class in ["media.audio", "read.huge"] {
            let got = chosen(&table, &world(), &request(class));
            assert_eq!(got.seat, "hydrogen", "{class}");
        }
        // An override of flash is refused: it is off.
        let mut over = request("build.bounded");
        over.override_seat = Some(("flash".into(), "x".into()));
        assert!(pick(&table, &world(), &over).is_err());
    }
}
