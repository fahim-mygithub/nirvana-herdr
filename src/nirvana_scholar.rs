//! Nirvana fork, plan 47 P47c: `herdr scholar "<question>"`. herdr splits a research question into research styles,
//! seats one researcher per style side by side, then seats a writer that builds a report in the project
//! (nirvana/plans/2026-11-20-47-scholar-everywhere.md, L3 to L5; rulings R2, R3, R4).
//!
//! The owner, 2026-11-20: "I want every project to have the a default Researcher agent,called the scholar agent where it
//! breaks down our prompt to different research styles and runs parallel research agents and finally builds an
//! artifact."
//!
//! The shape: the verb checks, writes `<run>/scholar/state.json` and the run's `opened` line, starts the driver
//! detached and returns. The driver (`herdr scholar drive <run>`) carries the run from the question to the report with
//! no model deciding a step: one headless route turn, the researchers seated back to back as gated orders, the writer,
//! a check of the files, the log line, then the run's end. The orchestrator spends no turn until the run has its
//! outcome: `replay` keeps every line of a researcher or the writer that the driver handles off the wake queue
//! (`STAGE_WHY`), so the driver's own end line (`tell_line`: the page's path, or why there is none, and what did not
//! deliver) is the one wake.
//!
//! Everything a resume needs is in the state file and the run's journal; the driver keeps nothing only in memory.
//! A driver holds an OS lock (`take_driver`) so one runs per run, and the lock goes when its process ends.

use crate::nirvana_look::End;
use crate::nirvana_run::{self, Kind, Phase, Project, RunState};
use crate::nirvana_seat::{self, Effort, Mode, Request, SizeName, Table, Tokens};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---- the menu ----

/// The eight research styles, each a card of the built-in `scholar` profile (`p47c_every_style_has_a_card_in_the_built_in`).
pub const STYLES: [&str; 8] = [
    "benchmark",
    "breadth-then-cluster",
    "competitive-analysis",
    "decision-matrix",
    "hypothesis-elimination",
    "source-of-truth",
    "synthesis",
    "trajectory",
];
/// The route text's fallback, and the one order a failed routing runs with the owner's words.
pub const FALLBACK_STYLE: &str = "breadth-then-cluster";
/// Plan 47 §4 step 1: "at most five". Five agy researchers at once is past anything recorded (the most was 5 agy
/// orders of any class, Megajam 2026-11-14; the most research.web at once was 2), so the cap is not raised.
pub const MAX_ORDERS: usize = 5;

pub const ROUTE_CLASS: &str = "scholar.route";
pub const RESEARCH_CLASS: &str = "research.web";
/// Not `research.*`: the sources axis would fail the writer for URLs it never opened (it reads the deliveries).
pub const WRITE_CLASS: &str = "write.docs";
pub const PROFILE: &str = "scholar";
/// The `seated` line's `why` for every order herdr scholar seats. `replay` reads it on a chain's root; every other
/// reader of `why` compares it with "send_back" or "reseat" only, so it is inert there.
pub const STAGE_WHY: &str = "scholar_stage";

/// Shipped as files, embedded (the P47b drafts with their card headers removed, changed as the P47c report lists).
pub const ROUTE_TEXT: &str = include_str!("../nirvana/scholar/route.md");
pub const WRITE_TEXT: &str = include_str!("../nirvana/scholar/write.md");

/// The route class as `look.image` is scored (plan 40c): the same seats, scores, bars and efforts, no `image` need.
/// Appended in memory to a table that lacks it, as `herdr look` does: the owner's installed seats.toml keeps its own
/// text. claude seats only: the route turn runs with no tools, and only claude has a tool filter.
pub const ROUTE_CLASS_TOML: &str = r#"
[[class]]
id = "scholar.route"
chain = "research"
blast = "bounded"
effort = "low"
efficient_effort = "low"
sota_effort = "low"
weight = 0.2
bars = { efficient = 70, balanced = 80, sota = 100 }
why = "plan 47 P47c: herdr scholar's route turn, one short JSON answer with no tools; scored as look.image is (plan 40c), a prior"
seats = [
  { seat = "haiku", score = 84 },
  { seat = "sonnet", score = 86 },
]
"#;

// ---- caps and files ----

/// Plan 47 P47c brief item 6: from the stage's start. Sizing (scope report §10): five researchers in parallel take
/// about the slowest, 15 to 40 minutes, plus send-backs.
pub const RESEARCH_CAP_S: u64 = 90 * 60;
pub const WRITE_CAP_S: u64 = 45 * 60;
pub const RESEARCH_CAP_ENV: &str = "NIRVANA_SCHOLAR_RESEARCH_CAP_S";
pub const WRITE_CAP_ENV: &str = "NIRVANA_SCHOLAR_WRITE_CAP_S";
/// How long the driver waits, after the run's outcome, for its wake to be typed before it ends the run anyway:
/// `herdr run end` drops a queued wake (`wake_once` answers "the run has ended", `herdr run notice` skips an ended
/// run). A choice, not a measurement: an orchestrator busy for six hours has an owner who will read `status`.
pub const END_WAIT_S: u64 = 6 * 3600;
pub const END_WAIT_ENV: &str = "NIRVANA_SCHOLAR_END_WAIT_S";
/// The driver reads the journal this often.
pub const POLL_S: u64 = 5;
/// One short turn; the route answer is a few hundred tokens.
pub const ROUTE_TIMEOUT: Duration = Duration::from_secs(180);
/// A failed route call is tried once more, then the run goes on with the fallback order.
pub const ROUTE_TRIES: u32 = 2;
/// A route turn's booking: it ends within `ROUTE_TIMEOUT`.
pub const ROUTE_LEASE_MINUTES: u64 = 5;
/// `herdr run end` refused (a delivered lesson review not gated yet): tried at each poll for this long from the first
/// refusal, then the run is left open and the orchestrator told the command. Review F10: 60 polls was five minutes; a
/// lesson review's gate can take longer than that.
pub const END_REFUSED_S: u64 = 30 * 60;
/// The route prompt reaches `claude` as one argument, and Windows cuts a command line near 32,767 characters; the
/// prompt is the route text, the question, 20 report titles and the schema, so the question keeps well under it.
pub const QUESTION_MAX: usize = 8_000;
/// A driver's take of its lock: 40 tries 50 ms apart, two seconds. Review F12: one try lost to a concurrent `status`,
/// whose `driver_live` takes the same lock for an instant. No OS call reads a file lock's holder without trying to
/// take it, and `driver.json`'s pid is not liveness (a pid is reused), so `status` keeps its instant take and the
/// driver waits it out.
pub const DRIVER_TAKE_ATTEMPTS: u32 = 40;

pub const STATE_VERSION: u32 = 1;
pub const LOG_VERSION: u32 = 1;
/// `<run>/scholar/`: the state file, the order texts, the researchers' scratch folders, the driver's log and lock.
pub const DIR: &str = "scholar";
pub const STATE_FILE: &str = "state.json";
pub const STOP_FILE: &str = "stop";
/// Ruling R4: "docs/research/ in the project, tracked, so the next session can find it".
pub const REPORTS: &str = "docs/research";
pub const LOG_FILE: &str = "log.jsonl";
pub const SLUG_MAX: usize = 40;
/// The route prompt lists at most this many existing report titles.
pub const TITLES_MAX: usize = 20;

// ---- the slug and the ids ----

/// Lower-case ASCII letters, digits and dashes from the question, at most `SLUG_MAX`, cut at a dash when one is in
/// the last half; `question-<8 hex of its sha256>` when it has no ASCII letter or digit. No `.`, `/` or `\` survives,
/// so no slug reaches outside its folder.
pub fn slug(question: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in question.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        let sha = crate::nirvana_look::sha256_hex(question.as_bytes());
        return format!("question-{}", &sha[..8]);
    }
    if out.len() <= SLUG_MAX {
        return out;
    }
    // ASCII only, so a byte cut is a character cut. A word that ends exactly at the cap is kept whole.
    if out.as_bytes()[SLUG_MAX] == b'-' {
        return out[..SLUG_MAX].to_owned();
    }
    let cut = &out[..SLUG_MAX];
    match cut.rfind('-') {
        Some(at) if at >= SLUG_MAX / 2 => cut[..at].to_owned(),
        _ => cut.trim_end_matches('-').to_owned(),
    }
}

/// `(YYYYMMDD, YYYY-MM-DD)` of `unix`, in UTC.
pub fn dates(unix: u64) -> (String, String) {
    let at = time::OffsetDateTime::from_unix_timestamp(unix as i64)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    let (y, m, d) = (at.year(), u8::from(at.month()), at.day());
    (
        format!("{y:04}{m:02}{d:02}"),
        format!("{y:04}-{m:02}-{d:02}"),
    )
}

/// `base`, else `base-2`, `base-3`, ... the first that `taken` says is free.
fn first_free(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap_or_else(|| base.to_owned())
}

/// `scholar-<YYYYMMDD>-<slug>`, with `-2`, `-3` when that run folder exists.
pub fn run_id(project: &Project, ymd: &str, slug: &str) -> String {
    first_free(&format!("scholar-{ymd}-{slug}"), |id| {
        project.run_dir(id).exists()
    })
}

/// `docs/research/<YYYY-MM-DD>-<slug>`, relative to the root, with `-2`, `-3` when it exists or another run's state
/// file claims it. Review F2: the writer makes the folder, so two runs of one question on one day both saw it free and
/// the second writer overwrote the first report. The claim is read from the state files rather than made by creating
/// the folder at `start`: a run whose driver dies before its route leaves no empty folder in the tracked
/// `docs/research/`, and a refused start writes no state file, so it claims nothing.
pub fn report_dir(root: &Path, date: &str, slug: &str, claimed: &[String]) -> String {
    first_free(&format!("{REPORTS}/{date}-{slug}"), |rel| {
        root.join(rel).exists() || claimed.iter().any(|dir| dir == rel)
    })
}

/// Every report folder a scholar state file of the project names, done runs included.
pub fn claimed_dirs(project: &Project) -> Vec<String> {
    scholar_runs(project)
        .iter()
        .filter_map(|run| load_state(project, run).ok())
        .map(|state| state.report_dir)
        .collect()
}

/// The longest suffix an order id of a run gets: the writer, reseated once, sent back three times.
const LONGEST_SUFFIX: &str = "-write-r1-s3";

/// The slug cut so every order id of the run, with its longest suffix, stays within `valid_id`'s 64.
pub fn order_base(slug: &str) -> String {
    let room = 64 - LONGEST_SUFFIX.len();
    slug.chars()
        .take(room)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

pub fn research_id(slug: &str, n: u32) -> String {
    format!("{}-r{n}", order_base(slug))
}

pub fn writer_id(slug: &str) -> String {
    format!("{}-write", order_base(slug))
}

// ---- the state ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Route,
    Research,
    Write,
    /// Stopping its orders on the owner's `herdr scholar stop`.
    Stopping,
    /// The outcome is decided: told, logged, and the run's end waits for its wake.
    Ending,
    Done,
}

impl Stage {
    pub fn word(self) -> &'static str {
        match self {
            Self::Route => "route",
            Self::Research => "research",
            Self::Write => "write",
            Self::Stopping => "stopping",
            Self::Ending => "ending",
            Self::Done => "done",
        }
    }
}

/// How a run ended. `Report` is the one success.
pub mod ends {
    pub const REPORT: &str = "report";
    pub const NOTHING: &str = "nothing_to_research";
    pub const NO_DELIVERY: &str = "no_delivery";
    pub const NO_REPORT: &str = "no_report";
    pub const INCOMPLETE: &str = "report_incomplete";
    pub const STOPPED: &str = "stopped";
    pub const GAVE_UP: &str = "gave_up";
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SeatNote {
    pub seat: String,
    pub cli: String,
    pub family: String,
    pub model: String,
    pub effort: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChainEnd {
    pub passed: bool,
    /// The order the end was read from: the chain's last send-back child, its reseat, or the root.
    pub leaf: String,
    #[serde(default)]
    pub why: Option<String>,
    pub at: u64,
}

/// One sub-question's chain (or the writer's): its root order, the root seated again once after an interrupt, and
/// how it ended.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chain {
    /// 1 to 5 for a researcher, 0 for the writer.
    pub n: u32,
    pub id: String,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub reseat: Option<String>,
    #[serde(default)]
    pub seat: Option<SeatNote>,
    #[serde(default)]
    pub end: Option<ChainEnd>,
}

impl Chain {
    /// The root the chain runs under now: the reseat once there is one.
    pub fn active(&self) -> &str {
        self.reseat.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteRecord {
    pub styles: Vec<String>,
    pub note: String,
    pub fell_back: bool,
    #[serde(default)]
    pub why: Option<String>,
    pub tries: u32,
    #[serde(default)]
    pub seat: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tokens: Tokens,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub run: String,
    pub question: String,
    pub slug: String,
    pub date: String,
    /// Relative to the project root: `docs/research/<date>-<slug>`.
    pub report_dir: String,
    pub mode: Mode,
    /// The orchestrator's pane the verb was started from (`HERDR_PANE_ID`); none from a plain shell.
    #[serde(default)]
    pub pane: Option<String>,
    pub stage: Stage,
    pub started_at: u64,
    /// When the current stage began: the caps run from it.
    pub stage_at: u64,
    #[serde(default)]
    pub route: Option<RouteRecord>,
    #[serde(default)]
    pub orders: Vec<Chain>,
    #[serde(default)]
    pub writer: Option<Chain>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub end_why: Option<String>,
    #[serde(default)]
    pub ended_at: Option<u64>,
    /// What the files check found wrong (`report_incomplete`).
    #[serde(default)]
    pub check: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub told: bool,
    #[serde(default)]
    pub logged: bool,
    #[serde(default)]
    pub end_tries: u32,
    /// When `herdr run end` first refused: the driver keeps trying for `END_REFUSED_S` from here.
    #[serde(default)]
    pub end_refused_at: Option<u64>,
}

pub fn scholar_dir(project: &Project, run: &str) -> PathBuf {
    project.run_dir(run).join(DIR)
}

pub fn state_path(project: &Project, run: &str) -> PathBuf {
    scholar_dir(project, run).join(STATE_FILE)
}

/// The state file, or why it cannot be used. An unknown version is refused, never guessed.
pub fn load_state(project: &Project, run: &str) -> Result<State, String> {
    let path = state_path(project, run);
    let text = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            format!("{run} is no scholar run: {} is not there", path.display())
        }
        _ => format!("{}: {err}", path.display()),
    })?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("{} does not parse: {err}", path.display()))?;
    match value.get("version").and_then(Value::as_u64) {
        Some(v) if v == u64::from(STATE_VERSION) => {}
        other => {
            return Err(format!(
                "{} has version {}, which this herdr does not know (it writes {STATE_VERSION}); it is left as it is",
                path.display(),
                other.map_or("none".to_owned(), |v| v.to_string())
            ))
        }
    }
    serde_json::from_value(value).map_err(|err| format!("{} does not parse: {err}", path.display()))
}

pub fn save_state(project: &Project, state: &State) -> Result<(), String> {
    let path = state_path(project, &state.run);
    let bytes = serde_json::to_vec_pretty(state).map_err(|err| err.to_string())?;
    nirvana_seat::write_atomic(&path, &bytes).map_err(|err| format!("{}: {err}", path.display()))
}

// ---- the driver's lock ----

/// One driver per run: `nirvana_seat::Lock` (an OS file lock the OS lets go when its holder ends however it ends) on
/// `<run>/scholar/driver/`, as a supervisor holds its order's. Its own folder: `close_run_tab` takes the run folder's.
pub struct DriverLock {
    _lock: nirvana_seat::Lock,
}

fn driver_dir(project: &Project, run: &str) -> PathBuf {
    scholar_dir(project, run).join("driver")
}

fn holder(project: &Project, run: &str) -> String {
    std::fs::read_to_string(scholar_dir(project, run).join("driver.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map_or("another process".to_owned(), |value| {
            format!(
                "pid {} (since {})",
                value["pid"].as_u64().unwrap_or(0),
                nirvana_run::utc(value["at"].as_u64().unwrap_or(0))
            )
        })
}

/// The run's driver lock, or who holds it.
pub fn take_driver(project: &Project, run: &str) -> Result<DriverLock, String> {
    match nirvana_seat::Lock::try_take(&driver_dir(project, run), DRIVER_TAKE_ATTEMPTS) {
        Ok(lock) => {
            let note = serde_json::json!({"pid": std::process::id(), "at": nirvana_run::now()});
            let _ = nirvana_seat::write_atomic(
                &scholar_dir(project, run).join("driver.json"),
                note.to_string().as_bytes(),
            );
            Ok(DriverLock { _lock: lock })
        }
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {
            Err(format!("a driver holds {run}: {}", holder(project, run)))
        }
        Err(err) => Err(format!("{run}: the driver's lock: {err}")),
    }
}

/// Whether a driver holds the run now.
pub fn driver_live(project: &Project, run: &str) -> bool {
    driver_dir(project, run).exists()
        && matches!(
            nirvana_seat::Lock::try_take(&driver_dir(project, run), 1),
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut
        )
}

// ---- route ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteOrder {
    pub style: String,
    pub question: String,
    #[serde(default)]
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteAnswer {
    pub orders: Vec<RouteOrder>,
    pub note: String,
}

/// The answer's JSON Schema: `{"orders":[{"style","question","why"}],"note"}`, the style one of the eight.
pub fn route_schema() -> String {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["orders", "note"],
        "properties": {
            "orders": {"type": "array", "items": {
                "type": "object",
                "additionalProperties": false,
                "required": ["style", "question", "why"],
                "properties": {
                    "style": {"type": "string", "enum": STYLES},
                    "question": {"type": "string", "minLength": 1},
                    "why": {"type": "string"},
                },
            }},
            "note": {"type": "string"},
        },
    })
    .to_string()
}

/// The answer held to the schema and the rules the schema cannot say: each style one of the eight and used once, each
/// question not empty. Orders past five are dropped and the note says so.
pub fn validate(value: &Value) -> Result<RouteAnswer, String> {
    let object = value.as_object().ok_or("the answer is not a JSON object")?;
    let orders = object
        .get("orders")
        .and_then(Value::as_array)
        .ok_or("the answer has no orders list")?;
    let mut note = object
        .get("note")
        .and_then(Value::as_str)
        .ok_or("the answer has no note")?
        .trim()
        .to_owned();
    let mut kept = Vec::new();
    for (at, order) in orders.iter().take(MAX_ORDERS).enumerate() {
        let style = order
            .get("style")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("order {} has no style", at + 1))?;
        if !STYLES.contains(&style) {
            return Err(format!(
                "order {} names the unknown style {style:?}",
                at + 1
            ));
        }
        if kept.iter().any(|known: &RouteOrder| known.style == style) {
            return Err(format!("the style {style} is used twice"));
        }
        let question = order
            .get("question")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|question| !question.is_empty())
            .ok_or_else(|| format!("order {} has no question", at + 1))?;
        kept.push(RouteOrder {
            style: style.to_owned(),
            question: question.to_owned(),
            why: order
                .get("why")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_owned(),
        });
    }
    if orders.len() > MAX_ORDERS {
        let dropped: Vec<&str> = orders[MAX_ORDERS..]
            .iter()
            .map(|order| order.get("style").and_then(Value::as_str).unwrap_or("?"))
            .collect();
        let line = format!(
            "{} order(s) past five were dropped: {}",
            dropped.len(),
            dropped.join(", ")
        );
        note = if note.is_empty() {
            line
        } else {
            format!("{note}; {line}")
        };
    }
    Ok(RouteAnswer { orders: kept, note })
}

/// The one order a failed routing runs: the fallback style with the owner's question word for word.
pub fn fallback(question: &str) -> RouteAnswer {
    RouteAnswer {
        orders: vec![RouteOrder {
            style: FALLBACK_STYLE.to_owned(),
            question: question.to_owned(),
            why: "the routing failed; the fallback style takes the owner's words".to_owned(),
        }],
        note: String::new(),
    }
}

/// Each `docs/research/<folder>/report.md`'s path and title line, newest folder first, at most `TITLES_MAX`. The
/// driver reads them; the route turn gets them in its prompt and has no tools.
pub fn existing_reports(root: &Path) -> Vec<(String, String)> {
    let mut folders: Vec<String> = std::fs::read_dir(root.join(REPORTS))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().join("report.md").is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    folders.sort();
    folders.reverse();
    folders
        .into_iter()
        .take(TITLES_MAX)
        .map(|folder| {
            let text = std::fs::read_to_string(root.join(REPORTS).join(&folder).join("report.md"))
                .unwrap_or_default();
            let title = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("")
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(160)
                .collect::<String>();
            (format!("{REPORTS}/{folder}/report.md"), title)
        })
        .collect()
}

/// The route turn's prompt: the shipped text, the question, and the reports already there.
pub fn route_prompt(question: &str, reports: &[(String, String)]) -> String {
    let listed = if reports.is_empty() {
        "none".to_owned()
    } else {
        reports
            .iter()
            .map(|(path, title)| format!("- {path}: {title}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "{}\n\nThe question, word for word:\n<question>\n{}\n</question>\n\nReports already in {REPORTS}/ (path: title):\n{listed}\n",
        ROUTE_TEXT.trim_end(),
        question.trim()
    )
}

/// The route turn's argv: `herdr look`'s measured line with no tools at all and the answer's schema.
pub fn route_argv(model: &str, effort: Effort, prompt: &str) -> Vec<String> {
    [
        "claude",
        "-p",
        prompt,
        "--model",
        model,
        "--effort",
        effort.word(),
        "--output-format",
        "json",
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--no-session-persistence",
        crate::nirvana_worker::EXCLUDE_DYNAMIC,
        "--tools",
        "",
        "--json-schema",
        &route_schema(),
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The answer and its tokens from what claude printed: the last `result` event (as `nirvana_look::answer_of` finds
/// it), its `structured_output`, else its `result` text read as JSON.
pub fn answer_value(stdout: &str) -> Result<(Value, Tokens), String> {
    let value: Value =
        serde_json::from_str(stdout.trim()).map_err(|_| "claude printed no JSON".to_owned())?;
    let events = match value {
        Value::Array(events) => events,
        other => vec![other],
    };
    let result = events
        .iter()
        .rev()
        .find(|event| event.get("type").and_then(Value::as_str) == Some("result"))
        .ok_or_else(|| "claude printed no result".to_owned())?;
    let mut stream = crate::nirvana_worker::Stream::new("claude");
    stream.feed(&result.to_string());
    let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);
    let subtype = result.get("subtype").and_then(Value::as_str).unwrap_or("");
    if is_error || subtype != "success" {
        return Err(format!(
            "claude ended with {subtype:?}, is_error {is_error}"
        ));
    }
    if let Some(found) = result
        .get("structured_output")
        .filter(|found| found.is_object())
    {
        return Ok((found.clone(), stream.tokens));
    }
    let text = result
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .map_or(text, |inner| inner.trim_end_matches("```"))
        .trim();
    serde_json::from_str(text)
        .map(|found| (found, stream.tokens))
        .map_err(|_| "the answer is not JSON".to_owned())
}

/// The project's table with `scholar.route` in it, appended as `nirvana_look::look_table` appends its class.
pub fn route_table(root: &Path) -> Result<Table, String> {
    let table = nirvana_seat::table_for(root)?;
    if table.class(ROUTE_CLASS).is_some() {
        return Ok(table);
    }
    let text = nirvana_seat::read_or_create(&nirvana_seat::dir().join(nirvana_seat::SEATS_FILE));
    let with = if text.contains(&format!("id = \"{ROUTE_CLASS}\"")) {
        text
    } else {
        format!("{}\n{ROUTE_CLASS_TOML}", text.trim_end())
    };
    let mut parsed = nirvana_seat::parse(&with)
        .map_err(|why| format!("seats.toml with {ROUTE_CLASS} added is refused: {why}"))?;
    parsed.mode = table.mode;
    Ok(parsed)
}

/// Releases a booking when it goes out of scope, whatever path it leaves by (`nirvana_look`'s guard, which is private).
struct LeaseGuard(String);

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        let dir = nirvana_seat::dir();
        if let Ok(_lock) = nirvana_seat::Lock::take(&dir) {
            let path = dir.join(nirvana_seat::LEASES_FILE);
            if let Ok(mut leases) = nirvana_seat::load_leases(&path) {
                if leases.release(&self.0, nirvana_run::now()) {
                    let _ = nirvana_seat::write_leases(&path, &leases);
                }
            }
        }
    }
}

/// The route turn's folder, under the Nirvana home and outside every project, as a look's is: a folder inside the
/// project would load its CLAUDE.md and settings into the turn.
pub fn route_home() -> Result<PathBuf, String> {
    let dir = nirvana_seat::dir().join("scholar-route");
    std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    Ok(dir)
}

fn record_route(
    table: &Table,
    pick: &nirvana_seat::Pick,
    order: &str,
    outcome: &str,
    tokens: Tokens,
) {
    let now = nirvana_run::now();
    let line = serde_json::json!({
        "order": order,
        "seat": pick.seat,
        "model": pick.model,
        "effort": pick.effort,
        "class": ROUTE_CLASS,
        "outcome": outcome,
        "tokens": tokens,
        "ended_at": now,
    });
    let Ok(line) = serde_json::from_value::<nirvana_seat::RecordLine>(line) else {
        return;
    };
    let Ok(line) = nirvana_seat::complete(table, line, now) else {
        return;
    };
    if let Err(err) = nirvana_seat::record_once(&nirvana_seat::dir(), line, false, now) {
        eprintln!("herdr scholar: the route's record line was not written: {err}");
    }
}

/// One route turn: picked by the rule over claude seats, booked, run in the route folder, its answer validated.
fn route_once(
    project: &Project,
    state: &State,
    world: &World,
    attempt: u32,
) -> Result<(RouteAnswer, String, String, Tokens), String> {
    let table = route_table(&project.root)?;
    let order = format!("{}-route-{attempt}", order_base(&state.slug));
    let mut request = Request::new(ROUTE_CLASS);
    request.size = SizeName::Small;
    request.mode = state.mode;
    request.order = Some(order.clone());
    request.claude_only = Some(
        "the route turn runs with no tools, and only a claude seat has a tool filter".to_owned(),
    );
    let dir = nirvana_seat::dir();
    let pick = {
        let _lock = nirvana_seat::Lock::take(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(nirvana_seat::LEASES_FILE);
        let mut leases = nirvana_seat::load_leases(&path)?;
        let mut seats = nirvana_seat::live_world(&table);
        seats.now = nirvana_run::now();
        seats.leases = leases.leases.clone();
        let pick = nirvana_seat::pick(&table, &seats, &request)
            .map_err(|no_seat| format!("no seat for the route: {}", no_seat.reason))?;
        let mut short = table.clone();
        short.size.small.lease_minutes = ROUTE_LEASE_MINUTES;
        let (lease, _) =
            nirvana_seat::book_pick(&mut leases, &short, &pick, Some(&order), None, seats.now);
        if lease.is_none() {
            return Err(format!("{order} was not booked"));
        }
        nirvana_seat::write_leases(&path, &leases).map_err(|err| err.to_string())?;
        pick
    };
    let _guard = LeaseGuard(order.clone());
    let cwd = route_home()?;
    if let Some(found) = crate::nirvana_look::steering_file(&cwd) {
        return Err(format!(
            "{} would load into the route turn; the route folder must hold nothing",
            found.display()
        ));
    }
    let prompt = route_prompt(&state.question, &existing_reports(&project.root));
    let argv = route_argv(&pick.model, pick.effort, &prompt);
    let answered = match (world.route)(&argv, &cwd) {
        Ok(End::Exited { stdout, code, .. }) => answer_value(&stdout).map_err(|why| {
            format!(
                "{} {} gave no answer: {why} (exit {})",
                pick.seat,
                pick.model,
                code.map_or("none".to_owned(), |code| code.to_string())
            )
        }),
        Ok(End::TimedOut) => Err(format!(
            "{} {} gave no answer in {} s",
            pick.seat,
            pick.model,
            ROUTE_TIMEOUT.as_secs()
        )),
        Err(why) => Err(why),
    };
    match answered.and_then(|(value, tokens)| validate(&value).map(|answer| (answer, tokens))) {
        Ok((answer, tokens)) => {
            record_route(&table, &pick, &order, "pass", tokens);
            Ok((answer, pick.seat, pick.model, tokens))
        }
        Err(why) => {
            record_route(&table, &pick, &order, "failed", Tokens::default());
            Err(why)
        }
    }
}

/// The route stage: tried `ROUTE_TRIES` times, then the fallback order with the owner's words.
pub fn route(project: &Project, state: &State, world: &World) -> (RouteAnswer, RouteRecord) {
    let mut why = String::new();
    for attempt in 1..=ROUTE_TRIES {
        match route_once(project, state, world, attempt) {
            Ok((answer, seat, model, tokens)) => {
                let record = RouteRecord {
                    styles: answer
                        .orders
                        .iter()
                        .map(|order| order.style.clone())
                        .collect(),
                    note: answer.note.clone(),
                    fell_back: false,
                    why: (attempt > 1).then(|| format!("try 1 failed: {why}")),
                    tries: attempt,
                    seat: Some(seat),
                    model: Some(model),
                    tokens,
                };
                return (answer, record);
            }
            Err(failed) => {
                why = if why.is_empty() {
                    failed
                } else {
                    format!("{why}; then {failed}")
                };
            }
        }
    }
    let answer = fallback(&state.question);
    let record = RouteRecord {
        styles: vec![FALLBACK_STYLE.to_owned()],
        note: String::new(),
        fell_back: true,
        why: Some(format!("the routing failed twice: {why}")),
        tries: ROUTE_TRIES,
        ..RouteRecord::default()
    };
    (answer, record)
}

// ---- the order texts ----

/// `.nirvana/runs/<run>/scholar/r<n>/`, relative to the project root: every working file of researcher `n`.
pub fn scratch_rel(run: &str, n: u32) -> String {
    format!("{}/runs/{run}/{DIR}/r{n}/", nirvana_run::DIR)
}

pub fn findings_rel(run: &str, n: u32) -> String {
    format!("{}findings.md", scratch_rel(run, n))
}

/// Text from another agent or from the owner, quoted line by line, so no line of it starts with a key the gate reads
/// (`owns:`, `test:`, `check:`, `serves:`, `Profile:`).
fn quoted(text: &str) -> String {
    text.trim()
        .lines()
        .map(|line| format!("> {}", line.trim_end()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A researcher's order. It names no tool, since the seat may be agy or claude; the profile's body and the card name
/// the method, and the fetch ladder names the fallbacks.
pub fn research_text(state: &State, chain: &Chain) -> String {
    let style = chain.style.as_deref().unwrap_or(FALLBACK_STYLE);
    let question = chain.question.as_deref().unwrap_or(&state.question);
    let scratch = scratch_rel(&state.run, chain.n);
    let findings = findings_rel(&state.run, chain.n);
    format!(
        "# Scholar research r{n}: {style}\n\n\
         Your question, which is the one you answer:\n\n{q}\n\n\
         The owner's whole question, as context only (other researchers take its other parts):\n\n{whole}\n\n\
         Your style is {style}. Read the card `{style}` before you start, and `fetch-ladder` when a fetch fails.\n\n\
         Put every working file, download and screenshot under `{scratch}` and nowhere else.\n\n\
         Write your findings to `{findings}`: each claim, its URL, the exact quote you took, and its tier; then what \
         you did not find, and what you did not check.\n\n\
         Your Work Report: `summary` a short answer; `sources` every URL cited in the findings file; `gotchas` at least \
         one line from the pass that argues against the answer; `files` the findings file.\n",
        n = chain.n,
        q = quoted(question),
        whole = quoted(&state.question),
    )
}

/// The page's path, relative to the project root: the writer's summary starts with it.
pub fn report_html_rel(state: &State) -> String {
    format!("{}/report.html", state.report_dir)
}

/// The writer's order: the shipped text, the question, each passed researcher's findings file and report from the
/// journal, what did not deliver and why, the Method notes, the folder and its `owns:` line.
pub fn write_text(state: &State, run: &RunState) -> String {
    let mut text = format!(
        "{}\n\n## The owner's question\n\n{}\n\n## The folder\n\nWrite `{dir}/report.md` and `{dir}/report.html`.\n\nowns: {dir}/\n\n## The deliveries\n",
        WRITE_TEXT.trim_end(),
        quoted(&state.question),
        dir = state.report_dir,
    );
    for chain in state
        .orders
        .iter()
        .filter(|chain| chain.end.as_ref().is_some_and(|end| end.passed))
    {
        let leaf = chain
            .end
            .as_ref()
            .map(|end| end.leaf.as_str())
            .unwrap_or(&chain.id);
        let report = run
            .order(leaf)
            .and_then(|known| known.report.clone())
            .unwrap_or(Value::Null);
        let style = chain.style.as_deref().unwrap_or("");
        text.push_str(&format!(
            "\n### r{} · {style}\n\nSub-question:\n\n{}\n\nFindings file: `{}`\n\nSummary:\n\n{}\n\nGotchas:\n",
            chain.n,
            quoted(chain.question.as_deref().unwrap_or("")),
            findings_rel(&state.run, chain.n),
            quoted(&one_line(report["summary"].as_str().unwrap_or(""))),
        ));
        let gotchas: Vec<&str> = report["gotchas"]
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if gotchas.is_empty() {
            text.push_str("- none given\n");
        }
        for gotcha in gotchas {
            text.push_str(&format!("- {}\n", one_line(gotcha)));
        }
        text.push_str("\nSources:\n");
        let sources: Vec<(String, String)> = report["sources"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        (
                            one_line(item["url"].as_str().unwrap_or("")),
                            one_line(item["used_for"].as_str().unwrap_or("")),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if sources.is_empty() {
            text.push_str("- none given\n");
        }
        for (url, used_for) in sources {
            text.push_str(&format!("- {url} · {used_for}\n"));
        }
    }
    text.push_str("\n## Did not deliver\n\n");
    let missed: Vec<&Chain> = state
        .orders
        .iter()
        .filter(|chain| chain.end.as_ref().is_some_and(|end| !end.passed))
        .collect();
    if missed.is_empty() {
        text.push_str("- none\n");
    }
    for chain in missed {
        text.push_str(&format!(
            "- r{} · {}: {} (why: {})\n",
            chain.n,
            chain.style.as_deref().unwrap_or(""),
            one_line(chain.question.as_deref().unwrap_or("")),
            one_line(
                chain
                    .end
                    .as_ref()
                    .and_then(|end| end.why.as_deref())
                    .unwrap_or("not delivered")
            ),
        ));
    }
    text.push_str("\n## For the Method section\n\n");
    for line in method_notes(state, run) {
        text.push_str(&format!("- {}\n", one_line(&line)));
    }
    text.push_str(&format!(
        "\n## Your Work Report\n\n`summary` starts with `{}`, then the answer line. `files` lists both paths.\n",
        report_html_rel(state)
    ));
    text
}

/// The longest reason a lost line carries: a question asked is quoted up to this many characters (brief item 1).
pub const LOST_WHY_MAX: usize = 200;

/// Each researcher that did not deliver, with its reason, and the writer's reseat: `r2 (benchmark) over the time cap`,
/// `r3 (trajectory) asked: "<the question>"`, `r1 (synthesis) interrupted twice: ... (interrupted and reseated once)`,
/// `... (sent back 2 times)`. Fix round item 1: `replay` keeps these lines off the orchestrator's queue while the
/// driver handles them, so the driver's end line and the writer's Method notes are where they are said.
pub fn lost_lines(state: &State, run: &RunState) -> Vec<String> {
    let mut lines = Vec::new();
    for chain in &state.orders {
        let Some(end) = chain.end.as_ref().filter(|end| !end.passed) else {
            continue;
        };
        let why = one_line(end.why.as_deref().unwrap_or("not delivered"));
        // A question is quoted: it is the worker's or the gate's words, not the driver's.
        let why = match why
            .strip_prefix("asked: ")
            .or_else(|| why.strip_prefix("gated asked: "))
        {
            Some(question) => format!(
                "asked: \"{}\"",
                question.chars().take(LOST_WHY_MAX).collect::<String>()
            ),
            None => why.chars().take(LOST_WHY_MAX).collect(),
        };
        let mut extra = Vec::new();
        if chain.reseat.is_some() {
            extra.push("interrupted and reseated once".to_owned());
        }
        let roots: Vec<&str> = std::iter::once(chain.id.as_str())
            .chain(chain.reseat.as_deref())
            .collect();
        let send_backs = run
            .orders
            .iter()
            .filter(|order| order.seated.why.as_deref() == Some("send_back"))
            .filter(|order| roots.contains(&crate::nirvana_gate::chain(run, order.id()).0.as_str()))
            .count();
        if send_backs > 0 {
            extra.push(format!("sent back {send_backs} time(s)"));
        }
        let extra = if extra.is_empty() {
            String::new()
        } else {
            format!(" ({})", extra.join("; "))
        };
        lines.push(format!(
            "r{} ({}) {why}{extra}",
            chain.n,
            chain.style.as_deref().unwrap_or("")
        ));
    }
    if let Some(reseat) = state.writer.as_ref().and_then(|writer| writer.reseat.as_deref()) {
        lines.push(format!("the writer was interrupted and reseated once as {reseat}"));
    }
    lines
}

/// The routing note, every seat note, every reseat and what did not deliver (`lost_lines`): the writer's Method
/// section.
pub fn method_notes(state: &State, run: &RunState) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(route) = &state.route {
        lines.push(format!(
            "routing: {} order(s), styles {}",
            state.orders.len(),
            route.styles.join(", ")
        ));
        if route.fell_back {
            lines.push(format!(
                "the routing failed, so the run went on with one {FALLBACK_STYLE} order on the owner's words: {}",
                route.why.as_deref().unwrap_or("")
            ));
        }
        if !route.note.is_empty() {
            lines.push(format!("routing note: {}", route.note));
        }
    }
    lines.extend(
        state
            .notes
            .iter()
            .filter(|note| {
                note.starts_with("seat:") || note.contains("was interrupted and reseated once")
            })
            .cloned(),
    );
    lines.extend(
        lost_lines(state, run)
            .into_iter()
            .filter(|line| line.starts_with('r'))
            .map(|line| format!("did not deliver: {line}")),
    );
    lines
}

// ---- reading a chain ----

#[derive(Debug, Clone, PartialEq)]
pub enum ChainStatus {
    /// Not seated, or still working (seated, running, deferred, delivered and not gated).
    Running(Option<String>),
    Passed(String),
    NotDelivered(String, String),
    /// Cut by a dead supervisor or worker: the driver reseats it once.
    Interrupted(String, String),
}

/// The chain's leaf: the last order the journal seated whose chain root (`nirvana_gate::chain`) is `root`.
pub fn leaf<'a>(run: &'a RunState, root: &str) -> Option<&'a nirvana_run::OrderState> {
    run.orders
        .iter()
        .filter(|order| crate::nirvana_gate::chain(run, order.id()).0 == root)
        .last()
}

/// How the chain under `root` stands, read from its leaf. A passing verdict is the only pass; a question, an amend,
/// a failure, a crash loop or a stop did not deliver; a dead worker is `Interrupted`.
pub fn status(run: &RunState, root: &str) -> ChainStatus {
    let Some(leaf) = leaf(run, root) else {
        return ChainStatus::Running(None);
    };
    let id = leaf.id().to_owned();
    let why = || leaf.why.clone().unwrap_or_default();
    match leaf.phase {
        Phase::Seated | Phase::Running | Phase::Deferred | Phase::Delivered | Phase::Reseated => {
            ChainStatus::Running(Some(id))
        }
        Phase::Merged => ChainStatus::Passed(id),
        Phase::Gated => match leaf.verdict.as_deref() {
            Some("pass") => ChainStatus::Passed(id),
            // The child is seated in the verdict's own transaction; until it shows, the chain still runs.
            Some("sent_back") => ChainStatus::Running(Some(id)),
            Some(verdict) => ChainStatus::NotDelivered(id, format!("gated {verdict}: {}", why())),
            None => ChainStatus::Running(Some(id)),
        },
        Phase::Asked => ChainStatus::NotDelivered(id, format!("asked: {}", why())),
        Phase::Failed => ChainStatus::NotDelivered(
            id,
            format!(
                "failed ({}): {}",
                leaf.outcome.as_deref().unwrap_or("failed"),
                why()
            ),
        ),
        Phase::Interrupted if leaf.outcome.as_deref() == Some("stopped") => {
            ChainStatus::NotDelivered(id, "stopped".to_owned())
        }
        Phase::Interrupted => ChainStatus::Interrupted(id, why()),
    }
}

// ---- the planner ----

/// What the driver does next. Pure: `next` picks it from the state and the replayed journal.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Route,
    /// Seat researcher `orders[i]` under its root id.
    Seat(usize),
    /// Seat `orders[i]` once more under a new id, after its chain was interrupted.
    Reseat(usize, String),
    SeatWriter,
    ReseatWriter(String),
    /// Chains that ended: (index, or `None` for the writer), and how.
    Ends(Vec<(Option<usize>, ChainEnd)>),
    /// Stop these leaves and count their chains as not delivered, for this reason.
    Stop(Vec<(Option<usize>, String)>, String),
    /// The writer passed: check the files.
    Check,
    /// The run's outcome and why.
    Finish(&'static str, String),
    /// Append the `asked` line that carries the outcome to the orchestrator, and wake it.
    Tell,
    Log,
    EndRun,
    Wait,
    Done,
}

/// The caps and the facts the planner reads that are not in the state or the journal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caps {
    pub research_s: u64,
    pub write_s: u64,
    pub end_wait_s: u64,
    /// Whether the run's orchestrator pane can be woken: `None` until the run is ending (it costs a herdr call).
    pub wakeable: Option<bool>,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            research_s: RESEARCH_CAP_S,
            write_s: WRITE_CAP_S,
            end_wait_s: END_WAIT_S,
            wakeable: None,
        }
    }
}

fn env_secs(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(default)
}

impl Caps {
    pub fn from_env() -> Self {
        Self {
            research_s: env_secs(RESEARCH_CAP_ENV, RESEARCH_CAP_S),
            write_s: env_secs(WRITE_CAP_ENV, WRITE_CAP_S),
            end_wait_s: env_secs(END_WAIT_ENV, END_WAIT_S),
            wakeable: None,
        }
    }
}

/// The step for one chain that has not ended: seat it, reseat it once, or record its end.
enum ChainStep {
    Seat,
    Reseat(String),
    End(ChainEnd),
    Running(Option<String>),
}

fn chain_step(chain: &Chain, run: &RunState, now: u64) -> ChainStep {
    if run.order(chain.active()).is_none() {
        return match &chain.reseat {
            Some(id) => ChainStep::Reseat(id.clone()),
            None => ChainStep::Seat,
        };
    }
    match status(run, chain.active()) {
        ChainStatus::Running(leaf) => ChainStep::Running(leaf),
        ChainStatus::Passed(leaf) => ChainStep::End(ChainEnd {
            passed: true,
            leaf,
            why: None,
            at: now,
        }),
        ChainStatus::NotDelivered(leaf, why) => ChainStep::End(ChainEnd {
            passed: false,
            leaf,
            why: Some(why),
            at: now,
        }),
        ChainStatus::Interrupted(..) if chain.reseat.is_none() => {
            ChainStep::Reseat(crate::nirvana_worker::child_id(run, &chain.id))
        }
        ChainStatus::Interrupted(leaf, why) => ChainStep::End(ChainEnd {
            passed: false,
            leaf,
            why: Some(format!("interrupted twice: {why}")),
            at: now,
        }),
    }
}

/// The next step. `stop` is the owner's stop marker. Pure.
pub fn next(state: &State, run: &RunState, now: u64, caps: &Caps, stop: bool) -> Step {
    if state.stage == Stage::Done {
        return Step::Done;
    }
    if let Some(end) = state.end.as_deref() {
        // Every end tells, a report too: `replay` keeps the writer's lines off the queue as it keeps the
        // researchers', so the driver's end line is the run's one wake and says what did not deliver (fix round
        // item 1). It was the writer's `delivered` line, which could not say it.
        if !state.told {
            return Step::Tell;
        }
        if !state.logged {
            return Step::Log;
        }
        // A run `herdr run end` kept refusing is left `Done` by `apply`, read at the top.
        if run.ended.is_some() {
            return Step::Done;
        }
        // `herdr run end` drops a wake that is queued and not typed, so the run ends once the queue is empty, at
        // once when nobody can be woken or the owner stopped it, and at the bound in any case.
        let waited = now.saturating_sub(state.ended_at.unwrap_or(now));
        if end == ends::STOPPED
            || caps.wakeable == Some(false)
            || run.queue.is_empty()
            || waited >= caps.end_wait_s
        {
            return Step::EndRun;
        }
        return Step::Wait;
    }
    // The owner's stop, in any stage: the live chains stop, then the run ends `stopped`.
    if stop || state.stage == Stage::Stopping {
        let mut live = Vec::new();
        for (at, chain) in state.orders.iter().enumerate() {
            if chain.end.is_none() {
                if let ChainStatus::Running(Some(leaf)) = status(run, chain.active()) {
                    live.push((Some(at), leaf));
                }
            }
        }
        if let Some(writer) = state.writer.as_ref().filter(|writer| writer.end.is_none()) {
            if let ChainStatus::Running(Some(leaf)) = status(run, writer.active()) {
                live.push((None, leaf));
            }
        }
        if !live.is_empty() {
            return Step::Stop(live, "stopped by `herdr scholar stop`".to_owned());
        }
        return Step::Finish(ends::STOPPED, "stopped by `herdr scholar stop`".to_owned());
    }
    match state.stage {
        Stage::Route => Step::Route,
        Stage::Research => research_step(state, run, now, caps),
        Stage::Write => write_step(state, run, now, caps),
        Stage::Stopping | Stage::Ending | Stage::Done => Step::Wait,
    }
}

fn research_step(state: &State, run: &RunState, now: u64, caps: &Caps) -> Step {
    let mut ended = Vec::new();
    let mut running = Vec::new();
    for (at, chain) in state.orders.iter().enumerate() {
        if chain.end.is_some() {
            continue;
        }
        match chain_step(chain, run, now) {
            ChainStep::Seat => return Step::Seat(at),
            ChainStep::Reseat(id) => return Step::Reseat(at, id),
            ChainStep::End(end) => ended.push((Some(at), end)),
            ChainStep::Running(leaf) => running.push((Some(at), leaf)),
        }
    }
    if !ended.is_empty() {
        return Step::Ends(ended);
    }
    if !running.is_empty() {
        if now.saturating_sub(state.stage_at) >= caps.research_s {
            return Step::Stop(
                running
                    .into_iter()
                    .map(|(at, leaf)| (at, leaf.unwrap_or_default()))
                    .collect(),
                "over the time cap".to_owned(),
            );
        }
        return Step::Wait;
    }
    if state
        .orders
        .iter()
        .any(|chain| chain.end.as_ref().is_some_and(|end| end.passed))
    {
        return Step::SeatWriter;
    }
    let why = lost_lines(state, run).join("; ");
    Step::Finish(ends::NO_DELIVERY, format!("no researcher passed: {why}"))
}

fn write_step(state: &State, run: &RunState, now: u64, caps: &Caps) -> Step {
    let Some(writer) = &state.writer else {
        return Step::SeatWriter;
    };
    if let Some(end) = &writer.end {
        return if end.passed {
            Step::Check
        } else {
            Step::Finish(
                ends::NO_REPORT,
                format!(
                    "the writer did not deliver: {}",
                    end.why.as_deref().unwrap_or("")
                ),
            )
        };
    }
    match chain_step(writer, run, now) {
        ChainStep::Seat => Step::SeatWriter,
        ChainStep::Reseat(id) => Step::ReseatWriter(id),
        ChainStep::End(end) => Step::Ends(vec![(None, end)]),
        ChainStep::Running(leaf) => {
            if now.saturating_sub(state.stage_at) >= caps.write_s {
                Step::Stop(
                    vec![(None, leaf.unwrap_or_default())],
                    "over the time cap".to_owned(),
                )
            } else {
                Step::Wait
            }
        }
    }
}

// ---- the files check ----

/// The first 80 characters of a value, for a failure line (brief item 5).
fn cut80(value: &str) -> String {
    value.chars().take(80).collect()
}

/// `&#NN;`, `&#xHH;` (the `;` optional, as a browser reads it) and the named entities a URL can be spelled with, in
/// text already lower-cased. Anything else is left as it is.
fn decode_entities(text: &str) -> String {
    const NAMED: [(&str, char); 11] = [
        ("&colon;", ':'),
        ("&sol;", '/'),
        ("&bsol;", '\\'),
        ("&period;", '.'),
        ("&amp;", '&'),
        ("&quot;", '"'),
        ("&apos;", '\''),
        ("&lt;", '<'),
        ("&gt;", '>'),
        ("&tab;", '\t'),
        ("&newline;", '\n'),
    ];
    let mut out = String::new();
    let mut rest = text;
    'outer: while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some(number) = rest.strip_prefix("&#") {
            let hex = number.starts_with('x');
            let digits = &number[usize::from(hex)..];
            let len = digits
                .chars()
                .take_while(|c| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() })
                .count();
            let code = u32::from_str_radix(&digits[..len], if hex { 16 } else { 10 })
                .ok()
                .and_then(char::from_u32);
            if let (true, Some(code)) = (len > 0, code) {
                out.push(code);
                let used = 2 + usize::from(hex) + len;
                rest = &rest[used..];
                rest = rest.strip_prefix(';').unwrap_or(rest);
                continue;
            }
        }
        for (name, code) in NAMED {
            if let Some(after) = rest.strip_prefix(name) {
                out.push(code);
                rest = after;
                continue 'outer;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// An attribute's value as the browser reads it: entities decoded, then tabs and line breaks dropped (a URL parser
/// drops them anywhere, so `ht&#9;tps:` is `https:`) and leading spaces and control characters trimmed.
fn attr_value(raw: &str) -> String {
    decode_entities(raw)
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect::<String>()
        .trim_start_matches(|c: char| c <= ' ')
        .to_owned()
}

/// Whether a URL (lower-cased) is on another host: a network scheme, or scheme-relative. A browser reads `\` as `/` in
/// such a URL, so `\\host` is `//host`, and from a page opened as a file it is a network share.
fn outside(url: &str) -> bool {
    let url = url.trim_start_matches(|c: char| c <= ' ' || c == '"' || c == '\'');
    ["//", "\\\\", "/\\", "\\/"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
        || ["http:", "https:", "ws:", "wss:", "ftp:"]
            .iter()
            .any(|scheme| url.starts_with(scheme))
}

/// A start tag read from just after its `<`: its name, its attributes (quoted values may hold `>`), and where it ends.
struct StartTag {
    name: String,
    attrs: Vec<(String, String)>,
    end: usize,
}

fn read_tag(lower: &str, start: usize) -> StartTag {
    let bytes = lower.as_bytes();
    let mut at = start;
    while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'-') {
        at += 1;
    }
    let name = lower[start..at].to_owned();
    let mut attrs = Vec::new();
    loop {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        if at >= bytes.len() {
            return StartTag { name, attrs, end: at };
        }
        if bytes[at] == b'>' {
            return StartTag { name, attrs, end: at + 1 };
        }
        let key_from = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && !b"=>/".contains(&bytes[at]) {
            at += 1;
        }
        let key = lower[key_from..at].to_owned();
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        if at < bytes.len() && bytes[at] == b'=' {
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            let value = match bytes.get(at) {
                Some(&quote) if quote == b'"' || quote == b'\'' => {
                    let from = at + 1;
                    let to = lower[from..]
                        .find(quote as char)
                        .map_or(lower.len(), |close| from + close);
                    at = (to + 1).min(lower.len());
                    &lower[from..to]
                }
                _ => {
                    let from = at;
                    while at < bytes.len()
                        && !bytes[at].is_ascii_whitespace()
                        && bytes[at] != b'>'
                    {
                        at += 1;
                    }
                    &lower[from..at]
                }
            };
            attrs.push((key, value.to_owned()));
        } else if !key.is_empty() {
            attrs.push((key, String::new()));
        } else {
            at += 1;
        }
    }
}

/// CSS that loads: an `@import` (the page is one file, so any import is a second load), or a `url(` on another host.
fn css_loads(label: &str, css: &str, found: &mut Vec<String>) {
    if let Some(at) = css.find("@import") {
        found.push(format!("{label} @import: {}", cut80(&css[at..])));
    }
    let mut from = 0;
    while let Some(hit) = css[from..].find("url(") {
        let at = from + hit;
        if outside(&css[at + 4..]) {
            found.push(format!("{label} url(: {}", cut80(&css[at..])));
            break;
        }
        from = at + 4;
    }
}

/// Script that talks to a host: `fetch(`, `XMLHttpRequest`, `import(` or `new WebSocket`. The report needs no script
/// that talks; its claims data is a `<script type="application/json">` block, which never runs.
fn script_talks(label: &str, js: &str, found: &mut Vec<String>) {
    for marker in ["fetch(", "xmlhttprequest", "import("] {
        if let Some(at) = js.find(marker) {
            found.push(format!("{label} {marker}: {}", cut80(&js[at..])));
        }
    }
    let mut from = 0;
    while let Some(hit) = js[from..].find("websocket") {
        let at = from + hit;
        let before = js[..at].trim_end();
        if before.len() < at && before.ends_with("new") {
            let new = before.len() - 3;
            found.push(format!("{label} new websocket: {}", cut80(&js[new..])));
            break;
        }
        from = at + "websocket".len();
    }
}

/// `rel` words of a `<link>` a reader follows, never a load (review F4: `<link rel=canonical>` failed a report).
const LINK_FOLLOWED: [&str; 10] = [
    "canonical",
    "alternate",
    "author",
    "license",
    "help",
    "next",
    "prev",
    "bookmark",
    "external",
    "me",
];

/// What one start tag loads from another host, or runs that talks to one.
fn tag_loads(tag: &StartTag, found: &mut Vec<String>) {
    let name = tag.name.as_str();
    let get = |key: &str| {
        tag.attrs
            .iter()
            .find(|(known, _)| known == key)
            .map(|(_, value)| attr_value(value))
    };
    let mut push = |key: &str, value: &str| found.push(format!("<{name}> {key}: {}", cut80(value)));
    match name {
        // Any `src`: the page is one file.
        "script" => {
            if let Some(src) = get("src") {
                push("src", &src);
            }
        }
        "link" => {
            let rel = get("rel").unwrap_or_default();
            let words: Vec<&str> = rel.split_whitespace().collect();
            let followed =
                !words.is_empty() && words.iter().all(|word| LINK_FOLLOWED.contains(word));
            if let Some(href) = get("href").filter(|href| outside(href) && !followed) {
                found.push(format!("<link rel={}> href: {}", words.join(" "), cut80(&href)));
            }
        }
        "img" | "iframe" | "frame" | "source" | "video" | "audio" | "embed" | "object"
        | "input" | "track" => {
            for key in ["src", "data", "poster"] {
                if let Some(value) = get(key).filter(|value| outside(value)) {
                    push(key, &value);
                }
            }
            if let Some(set) = get("srcset")
                .filter(|set| set.split(',').any(|candidate| outside(candidate.trim())))
            {
                push("srcset", &set);
            }
        }
        // svg: `<image>`, `<use>` and `<feImage>` load what their href names.
        "image" | "use" | "feimage" => {
            for key in ["href", "xlink:href"] {
                if let Some(value) = get(key).filter(|value| outside(value)) {
                    push(key, &value);
                }
            }
        }
        // Any `<base href>`: it moves every relative link and load of the page, and a one-file page needs none.
        "base" => {
            if let Some(href) = get("href") {
                push("href", &href);
            }
        }
        "meta" => {
            let content = get("content").unwrap_or_default();
            let target = content.find("url").map(|at| {
                content[at + 3..]
                    .trim_start_matches(|c: char| c <= ' ' || c == '=' || c == '"' || c == '\'')
            });
            if get("http-equiv").as_deref() == Some("refresh") && target.is_some_and(outside) {
                found.push(format!("<meta http-equiv=refresh> content: {}", cut80(&content)));
            }
        }
        "form" => {
            if let Some(action) = get("action").filter(|action| outside(action)) {
                push("action", &action);
            }
        }
        _ => {}
    }
    let mut push = |key: &str, value: &str| found.push(format!("<{name}> {key}: {}", cut80(value)));
    if let Some(value) = get("formaction").filter(|value| outside(value)) {
        push("formaction", &value);
    }
    // `<body background>`, and the table cells that take one too.
    if let Some(value) = get("background").filter(|value| outside(value)) {
        push("background", &value);
    }
    if let Some(style) = get("style") {
        css_loads(&format!("<{name}> style"), &style, found);
    }
    for (key, value) in tag.attrs.iter().filter(|(key, _)| key.starts_with("on")) {
        script_talks(&format!("<{name}> {key}"), &attr_value(value), found);
    }
}

/// Every load `html` makes from another host, and every script that talks to one, each named by its tag and the
/// first 80 characters of its value. Review F4: the check read the whole page as one string, so the word `@import` in
/// prose failed a report about CSS, and an svg `<image>`, a `<base>`, a `<meta refresh>` or an inline `fetch` passed.
/// It reads tags now: a style counts inside a `<style>` element or a `style=` attribute, a script inside a running
/// `<script>` or an `on…=` attribute, and text, `<pre>`, `<code>` and comments count for nothing. A link the reader
/// clicks (`<a href>`, `<link rel=canonical>`) is fine.
pub fn outside_loads(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(open) = lower[at..].find('<') {
        let start = at + open + 1;
        let rest = &lower[start..];
        if let Some(comment) = rest.strip_prefix("!--") {
            at = comment
                .find("-->")
                .map_or(lower.len(), |close| start + 3 + close + 3);
            continue;
        }
        if rest.starts_with(['!', '?', '/']) {
            at = rest.find('>').map_or(lower.len(), |close| start + close + 1);
            continue;
        }
        if !rest.starts_with(|c: char| c.is_ascii_alphabetic()) {
            at = start;
            continue;
        }
        let tag = read_tag(&lower, start);
        at = tag.end;
        tag_loads(&tag, &mut found);
        // Raw text: what is inside is not markup. A style's is CSS, a running script's is code, a title's is text.
        if matches!(tag.name.as_str(), "script" | "style" | "title" | "textarea") {
            let close = format!("</{}", tag.name);
            let body_end = lower[at..].find(&close).map_or(lower.len(), |close| at + close);
            let body = &lower[at..body_end];
            let data = tag
                .attrs
                .iter()
                .any(|(key, value)| key == "type" && value.contains("json"));
            match tag.name.as_str() {
                "style" => css_loads("<style>", body, &mut found),
                "script" if !data && !tag.attrs.iter().any(|(key, _)| key == "src") => {
                    script_talks("<script>", body, &mut found)
                }
                _ => {}
            }
            at = body_end;
        }
    }
    found.dedup();
    found
}

/// What is wrong with the report folder: a missing or empty `report.md` or `report.html`, or an outside load.
pub fn check_files(dir: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for name in ["report.md", "report.html"] {
        match std::fs::read_to_string(dir.join(name)) {
            Ok(text) if !text.trim().is_empty() => {
                if name == "report.html" {
                    problems.extend(
                        outside_loads(&text)
                            .into_iter()
                            .map(|why| format!("report.html loads {why}")),
                    );
                }
            }
            Ok(_) => problems.push(format!("{name} is empty")),
            Err(_) => problems.push(format!("{name} is not there")),
        }
    }
    problems
}

// ---- the world ----

/// What the driver does to the machine; the tests give stand-ins.
pub struct World<'a> {
    pub now: &'a dyn Fn() -> u64,
    pub sleep: &'a dyn Fn(u64),
    /// The route turn: argv and folder in, what the CLI ended with out.
    pub route: &'a dyn Fn(&[String], &Path) -> Result<End, String>,
    pub seat: &'a dyn Fn(&Project, &Seating) -> Result<SeatNote, String>,
    pub stop: &'a dyn Fn(&Project, &str, &str) -> String,
    pub wake: &'a dyn Fn(&Project, &str),
    pub end_run: &'a dyn Fn(&Project, &str) -> Result<(), String>,
    /// Whether a wake can be typed into this pane: it runs Nirvana Claude.
    pub wakeable: &'a dyn Fn(&str) -> bool,
}

/// One order to seat: its id, class, profile, text and the run's mode.
#[derive(Debug, Clone)]
pub struct Seating {
    pub run: String,
    pub id: String,
    pub class: String,
    pub profile: Option<String>,
    pub text: String,
    pub mode: Mode,
}

fn real_now() -> u64 {
    nirvana_run::now()
}

fn real_sleep(secs: u64) {
    std::thread::sleep(Duration::from_secs(secs));
}

fn real_route(argv: &[String], cwd: &Path) -> Result<End, String> {
    crate::nirvana_look::launch(argv, cwd, "claude", ROUTE_TIMEOUT)
}

fn real_stop(project: &Project, run: &str, order: &str) -> String {
    crate::nirvana_worker::stop_order(project, run, order)
        .unwrap_or_else(|err| format!("{order}: {err}"))
}

fn real_wake(project: &Project, run: &str) {
    let _ = nirvana_run::wake(project, run, false);
}

/// `herdr run end <run>` in a child process: the journal's end, the record, the retro, the receipt and the tab, as the
/// orchestrator's own end does. Its output goes to the driver's log.
fn real_end_run(project: &Project, run: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let out = std::process::Command::new(exe)
        .args(["run", "end", run])
        .env(nirvana_run::PROJECT_ENV, &project.root)
        .current_dir(&project.root)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    print!("{}", String::from_utf8_lossy(&out.stdout));
    eprint!("{}", String::from_utf8_lossy(&out.stderr));
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn real_wakeable(pane: &str) -> bool {
    crate::nirvana_worker::pane_view(pane).program == nirvana_run::PaneProgram::NirvanaClaude
}

/// The order's seat by the two existing steps: the rule's pick under `leases.lock` (`nirvana_seat::pick`, then
/// `book_pick`), then `seat_order_why`, which reuses that lease. The profile narrows the pick; there is no family pin.
pub fn seat_real(project: &Project, seating: &Seating) -> Result<SeatNote, String> {
    let orders = scholar_dir(project, &seating.run).join("orders");
    std::fs::create_dir_all(&orders).map_err(|err| format!("{}: {err}", orders.display()))?;
    let order_file = orders.join(format!("{}.md", seating.id));
    std::fs::write(&order_file, &seating.text)
        .map_err(|err| format!("{}: {err}", order_file.display()))?;
    let table = nirvana_seat::table_for(&project.root)?;
    let mut request = Request::new(&seating.class);
    request.mode = seating.mode;
    request.order = Some(seating.id.clone());
    let mut wanted: Vec<String> = Vec::new();
    if let Some(name) = &seating.profile {
        let loaded = crate::nirvana_profiles::load(&project.root, name)?;
        crate::nirvana_profiles::narrow(&mut request, &loaded.profile);
        wanted = crate::nirvana_profiles::joined_needs(&[], &loaded.profile);
    }
    let needs = crate::nirvana_gui_tools::seat_needs(&table, &project.root, &wanted)?;
    request.needs = needs.rest.clone();
    if request.claude_only.is_none() {
        request.claude_only = needs.claude_only.clone();
    }
    let dir = nirvana_seat::dir();
    let note = {
        let _lock = nirvana_seat::Lock::take(&dir).map_err(|err| err.to_string())?;
        let path = dir.join(nirvana_seat::LEASES_FILE);
        let mut leases = nirvana_seat::load_leases(&path)?;
        let mut world = nirvana_seat::live_world(&table);
        world.now = nirvana_run::now();
        if let Some(lease) = leases.live(&seating.id, world.now) {
            // A resume after a crash between the booking and the seat: the booking stands.
            let field = |key: &str| lease.pick[key].as_str().unwrap_or("").to_owned();
            SeatNote {
                seat: field("seat"),
                cli: field("cli"),
                family: field("family"),
                model: field("model"),
                effort: field("effort"),
                reason: field("reason"),
            }
        } else {
            world.leases = leases.leases.clone();
            world.overrides_used = leases.overrides(&seating.run);
            let mut pick = nirvana_seat::pick(&table, &world, &request).map_err(|no_seat| {
                format!(
                    "no seat: {}{}",
                    no_seat.reason,
                    no_seat
                        .earliest_reset
                        .map(|at| format!("; earliest reset {}", nirvana_run::utc(at)))
                        .unwrap_or_default()
                )
            })?;
            pick.needs = wanted.clone();
            let (lease, _) = nirvana_seat::book_pick(
                &mut leases,
                &table,
                &pick,
                Some(&seating.id),
                Some(&seating.run),
                world.now,
            );
            if lease.is_none() {
                return Err(format!("{} was not booked", seating.id));
            }
            nirvana_seat::write_leases(&path, &leases).map_err(|err| err.to_string())?;
            SeatNote {
                seat: pick.seat,
                cli: pick.cli,
                family: pick.family,
                model: pick.model,
                effort: pick.effort.word().to_owned(),
                reason: pick.reason,
            }
        }
    };
    let request = crate::nirvana_worker::SeatRequest {
        seat: note.seat.clone(),
        order_file,
        order: seating.id.clone(),
        run: seating.run.clone(),
        class: Some(seating.class.clone()),
        size: None,
        needs: Vec::new(),
        not_family: Vec::new(),
        mode: Some(seating.mode),
        effort: None,
        reason: None,
        worktree: false,
        auto: true,
        send_ok: None,
    };
    crate::nirvana_worker::seat_order_why(
        project,
        &request,
        seating.profile.as_deref(),
        Some(STAGE_WHY),
    )
    .map_err(|(code, why)| format!("{why} (exit {code})"))?;
    Ok(note)
}

pub fn real_world() -> World<'static> {
    World {
        now: &real_now,
        sleep: &real_sleep,
        route: &real_route,
        seat: &seat_real,
        stop: &real_stop,
        wake: &real_wake,
        end_run: &real_end_run,
        wakeable: &real_wakeable,
    }
}

// ---- the driver ----

fn stop_marker(project: &Project, run: &str) -> PathBuf {
    scholar_dir(project, run).join(STOP_FILE)
}

/// The line the run's `asked` event carries: the outcome, the status command, why, then what did not deliver
/// (`lost_lines`). The queue keeps 120 characters of it (`queue_line`), so what the orchestrator acts on comes first;
/// `status` and the journal keep it whole. A `no_delivery` why is the lost list already.
pub fn tell_line(state: &State, run: &RunState) -> String {
    let end = state.end.as_deref().unwrap_or("?");
    let why = one_line(state.end_why.as_deref().unwrap_or(""));
    let lost = lost_lines(state, run);
    let lost = if lost.is_empty() || end == ends::NO_DELIVERY {
        String::new()
    } else {
        format!("; did not deliver: {}", lost.join("; "))
    };
    if end != ends::REPORT {
        return format!("scholar {end}: `herdr scholar status {}`; {why}{lost}", state.run);
    }
    // A report: the page's path first (its why), so it is inside the 120 characters, then the writer's answer line,
    // its summary after the path it opens with.
    let answer = state
        .writer
        .as_ref()
        .and_then(|writer| writer.end.as_ref())
        .and_then(|end| run.order(&end.leaf))
        .and_then(|leaf| leaf.report.as_ref())
        .and_then(|report| report["summary"].as_str())
        .map(|summary| one_line(summary.trim().trim_start_matches(why.as_str())))
        .filter(|answer| !answer.is_empty())
        .map(|answer| format!(": {}", answer.chars().take(LOST_WHY_MAX).collect::<String>()))
        .unwrap_or_default();
    format!(
        "scholar report: {why} (`herdr scholar status {}`){answer}{lost}",
        state.run
    )
}

/// One researcher's chain as the log and the status read it: every order whose chain root is the chain's root or its
/// reseat, their send-backs, tokens and wall seconds.
fn chain_facts(run: &RunState, chain: &Chain) -> Value {
    let roots: Vec<&str> = std::iter::once(chain.id.as_str())
        .chain(chain.reseat.as_deref())
        .collect();
    let members: Vec<&nirvana_run::OrderState> = run
        .orders
        .iter()
        .filter(|order| roots.contains(&crate::nirvana_gate::chain(run, order.id()).0.as_str()))
        .collect();
    let mut tokens = Tokens::default();
    let mut any = false;
    for order in &members {
        if let Some(found) = order.tokens() {
            any = true;
            tokens.input += found.input;
            tokens.cache_write += found.cache_write;
            tokens.cache_read += found.cache_read;
            tokens.output += found.output;
        }
    }
    let first = members.iter().map(|order| order.seated_at).min();
    let last = members.iter().map(|order| order.last_at).max();
    let send_backs = members
        .iter()
        .filter(|order| order.seated.why.as_deref() == Some("send_back"))
        .count();
    let end = chain.end.as_ref();
    serde_json::json!({
        "id": chain.id,
        "style": chain.style,
        "reseat": chain.reseat,
        "seat": chain.seat.as_ref().map(|seat| &seat.seat),
        "family": chain.seat.as_ref().map(|seat| &seat.family),
        "model": chain.seat.as_ref().map(|seat| &seat.model),
        "end": end.map(|end| if end.passed { "passed" } else { "not_delivered" }),
        "why": end.and_then(|end| end.why.clone()),
        "send_backs": send_backs,
        "wall_s": first.zip(last).map(|(first, last)| last.saturating_sub(first)),
        "tokens": any.then_some(tokens),
    })
}

/// The run's one log line (`docs/research/log.jsonl`), however it ended.
pub fn log_line(state: &State, run: &RunState, now: u64) -> Value {
    serde_json::json!({
        "v": LOG_VERSION,
        "run": state.run,
        "date": state.date,
        "question": state.question,
        "slug": state.slug,
        "end": state.end,
        "why": state.end_why,
        "report": (state.end.as_deref() == Some(ends::REPORT)).then(|| report_html_rel(state)),
        "mode": state.mode,
        "routing": state.route.as_ref().map(|route| serde_json::json!({
            "styles": route.styles,
            "note": route.note,
            "fell_back": route.fell_back,
            "why": route.why,
            "seat": route.seat,
            "model": route.model,
            "tokens": route.tokens,
        })),
        "orders": state.orders.iter().map(|chain| chain_facts(run, chain)).collect::<Vec<_>>(),
        "writer": state.writer.as_ref().map(|chain| chain_facts(run, chain)),
        "check": state.check,
        "notes": state.notes,
        "wall_s": now.saturating_sub(state.started_at),
        "outcome": Value::Null,
    })
}

pub fn log_path(root: &Path) -> PathBuf {
    root.join(REPORTS).join(LOG_FILE)
}

/// One line at the end of `path`. Review F3: a log whose last line lost its newline (a hand edit, an editor that strips
/// it) joined the next line to it, and both were lost to `read_log`; a newline is written first when the file does
/// not end in one.
fn append_line(path: &Path, value: &Value) -> Result<(), String> {
    use std::io::{Read as _, Seek as _, SeekFrom};
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let mut text = value.to_string();
    text.push('\n');
    let said = |err: std::io::Error| format!("{}: {err}", path.display());
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(said)?;
    if file.metadata().map_err(said)?.len() > 0 {
        let mut last = [0u8; 1];
        file.seek(SeekFrom::End(-1)).map_err(said)?;
        file.read_exact(&mut last).map_err(said)?;
        if last[0] != b'\n' {
            text.insert(0, '\n');
        }
    }
    file.write_all(text.as_bytes()).map_err(said)
}

/// The writer's order id is seated once its chain list holds it.
fn writer_chain(state: &State) -> Chain {
    Chain {
        n: 0,
        id: writer_id(&state.slug),
        style: None,
        question: None,
        why: None,
        reseat: None,
        seat: None,
        end: None,
    }
}

fn seat_note_line(label: &str, note: &SeatNote) -> Option<String> {
    (note.family != "google").then(|| {
        format!(
            "seat: {label} landed off the google family, on {} ({} {}): {}",
            note.seat, note.family, note.model, note.reason
        )
    })
}

/// Applies one step. Returns whether the driver should sleep before the next.
fn apply(
    project: &Project,
    state: &mut State,
    run: &RunState,
    step: Step,
    world: &World,
) -> Result<bool, String> {
    let now = (world.now)();
    match step {
        Step::Done | Step::Wait => return Ok(matches!(step, Step::Wait)),
        Step::Route => {
            let (answer, record) = route(project, state, world);
            state.orders = answer
                .orders
                .iter()
                .enumerate()
                .map(|(at, order)| Chain {
                    n: at as u32 + 1,
                    id: research_id(&state.slug, at as u32 + 1),
                    style: Some(order.style.clone()),
                    question: Some(order.question.clone()),
                    why: Some(order.why.clone()),
                    reseat: None,
                    seat: None,
                    end: None,
                })
                .collect();
            let nothing = state.orders.is_empty();
            let note = record.note.clone();
            state.route = Some(record);
            state.stage = Stage::Research;
            state.stage_at = now;
            if nothing {
                finish(
                    project,
                    state,
                    ends::NOTHING,
                    format!("the route found nothing to research: {note}"),
                    now,
                );
            }
        }
        Step::Seat(at) | Step::Reseat(at, _) => {
            let reseat = match &step {
                Step::Reseat(_, id) => Some(id.clone()),
                _ => None,
            };
            if let Some(id) = &reseat {
                if state.orders[at].reseat.is_none() {
                    state.orders[at].reseat = Some(id.clone());
                    state.notes.push(format!(
                        "r{} was interrupted and reseated once as {id}",
                        state.orders[at].n
                    ));
                    save_state(project, state)?;
                }
            }
            let chain = state.orders[at].clone();
            let scratch = project.root.join(scratch_rel(&state.run, chain.n));
            let _ = std::fs::create_dir_all(&scratch);
            let seating = Seating {
                run: state.run.clone(),
                id: chain.active().to_owned(),
                class: RESEARCH_CLASS.to_owned(),
                profile: Some(PROFILE.to_owned()),
                text: research_text(state, &chain),
                mode: state.mode,
            };
            match (world.seat)(project, &seating) {
                Ok(note) => {
                    if let Some(line) =
                        seat_note_line(&format!("r{} {}", chain.n, seating.id), &note)
                    {
                        state.notes.push(line);
                    }
                    if state.orders[at].seat.is_none() {
                        state.orders[at].seat = Some(note);
                    }
                }
                Err(why) => {
                    state.orders[at].end = Some(ChainEnd {
                        passed: false,
                        leaf: seating.id,
                        why: Some(format!("not seated: {why}")),
                        at: now,
                    })
                }
            }
        }
        Step::SeatWriter | Step::ReseatWriter(_) => {
            if state.stage != Stage::Write {
                state.stage = Stage::Write;
                state.stage_at = now;
            }
            let mut writer = state.writer.clone().unwrap_or_else(|| writer_chain(state));
            if let Step::ReseatWriter(id) = &step {
                if writer.reseat.is_none() {
                    writer.reseat = Some(id.clone());
                    state.notes.push(format!(
                        "the writer was interrupted and reseated once as {id}"
                    ));
                }
            }
            state.writer = Some(writer.clone());
            save_state(project, state)?;
            let seating = Seating {
                run: state.run.clone(),
                id: writer.active().to_owned(),
                class: WRITE_CLASS.to_owned(),
                profile: None,
                text: write_text(state, run),
                mode: state.mode,
            };
            let _ = std::fs::create_dir_all(project.root.join(&state.report_dir));
            match (world.seat)(project, &seating) {
                Ok(note) => {
                    if let Some(writer) =
                        state.writer.as_mut().filter(|writer| writer.seat.is_none())
                    {
                        writer.seat = Some(note);
                    }
                }
                Err(why) => {
                    if let Some(writer) = state.writer.as_mut() {
                        writer.end = Some(ChainEnd {
                            passed: false,
                            leaf: seating.id,
                            why: Some(format!("not seated: {why}")),
                            at: now,
                        });
                    }
                }
            }
        }
        Step::Ends(found) => {
            for (at, end) in found {
                match at {
                    Some(at) => state.orders[at].end = Some(end),
                    None => {
                        if let Some(writer) = state.writer.as_mut() {
                            writer.end = Some(end);
                        }
                    }
                }
            }
        }
        Step::Stop(leaves, why) => {
            if why != "over the time cap" && state.stage != Stage::Stopping {
                state.stage = Stage::Stopping;
                save_state(project, state)?;
            }
            for (at, leaf) in leaves {
                if !leaf.is_empty() {
                    let said = (world.stop)(project, &state.run, &leaf);
                    state.notes.push(format!("stop {leaf}: {said}"));
                }
                let end = ChainEnd {
                    passed: false,
                    leaf,
                    why: Some(why.clone()),
                    at: now,
                };
                match at {
                    Some(at) => state.orders[at].end = Some(end),
                    None => {
                        if let Some(writer) = state.writer.as_mut() {
                            writer.end = Some(end);
                        }
                    }
                }
            }
        }
        Step::Check => {
            let problems = check_files(&project.root.join(&state.report_dir));
            if problems.is_empty() {
                let path = report_html_rel(state);
                finish(project, state, ends::REPORT, path, now);
            } else {
                state.check = problems.clone();
                finish(
                    project,
                    state,
                    ends::INCOMPLETE,
                    format!(
                        "the writer passed but its files fail the check: {}",
                        problems.join("; ")
                    ),
                    now,
                );
            }
        }
        Step::Finish(end, why) => finish(project, state, end, why, now),
        Step::Tell => {
            project
                .journal(&state.run)
                .append(
                    now,
                    Kind::Asked {
                        order: None,
                        question: tell_line(state, run),
                    },
                )
                .map_err(|err| err.to_string())?;
            // `told` is saved before the wake, on purpose (review F5). A kill between the two loses only the typing:
            // the line is on the queue, and the orchestrator's next prompt notice reads it. Waking first, a kill
            // before the save would append and type the line a second time on resume.
            state.told = true;
            save_state(project, state)?;
            (world.wake)(project, &state.run);
        }
        Step::Log => {
            // Two steps, the append and the save of `logged`: a kill between them asks for the line again, and
            // `append_run_line` skips a run the log already holds (review F11).
            append_run_line(&project.root, &log_line(state, run, now))?;
            state.logged = true;
        }
        Step::EndRun => {
            match (world.end_run)(project, &state.run) {
                Ok(()) => {
                    if state.end != Some(ends::STOPPED.to_owned())
                        && !run.queue.is_empty()
                        && state.pane.is_some()
                    {
                        state.notes.push(format!(
                            "the run was ended with {} line(s) still queued for the orchestrator",
                            run.queue.len()
                        ));
                    }
                    state.stage = Stage::Done;
                }
                Err(why) => {
                    state.end_tries += 1;
                    let first = *state.end_refused_at.get_or_insert(now);
                    if now.saturating_sub(first) < END_REFUSED_S {
                        save_state(project, state)?;
                        return Ok(true);
                    }
                    // Review F10: the note and the queued line name the exact command and herdr's own words, and the
                    // line leads with the command: the queue keeps a run's question to 120 characters.
                    let command = format!("`herdr run end {}`", state.run);
                    state.notes.push(format!(
                        "the run was left open: {command} refused for {} minutes ({} tries); herdr said: {why}",
                        END_REFUSED_S / 60,
                        state.end_tries
                    ));
                    project
                        .journal(&state.run)
                        .append(
                            now,
                            Kind::Asked {
                                order: None,
                                question: format!(
                                    "scholar {}: {command} refused for {} minutes; run it once it can end: {why}",
                                    state.end.as_deref().unwrap_or("?"),
                                    END_REFUSED_S / 60
                                ),
                            },
                        )
                        .map_err(|err| err.to_string())?;
                    state.stage = Stage::Done;
                    save_state(project, state)?;
                    (world.wake)(project, &state.run);
                }
            }
        }
    }
    save_state(project, state)?;
    Ok(false)
}

/// The outcome: written, and the stage moves to ending. A folder left empty by a run with no report goes.
fn finish(project: &Project, state: &mut State, end: &'static str, why: String, now: u64) {
    state.end = Some(end.to_owned());
    state.end_why = Some(why);
    state.ended_at = Some(now);
    state.stage = Stage::Ending;
    if end != ends::REPORT {
        let dir = project.root.join(&state.report_dir);
        if std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_none()) {
            let _ = std::fs::remove_dir(&dir);
        }
    }
}

/// The driver's loop, holding the run's lock: replay, plan, apply, until the run is done.
pub fn drive(project: &Project, run: &str, world: &World, caps: Caps) -> Result<State, String> {
    // Read first: taking the lock makes `driver/` and `driver.json`, which a run id that is not there must not get
    // (review F6).
    load_state(project, run)?;
    let _lock = take_driver(project, run)?;
    drive_held(project, run, world, caps)
}

/// `drive` with the lock already held (`herdr scholar stop` with no live driver takes it itself).
fn drive_held(project: &Project, run: &str, world: &World, caps: Caps) -> Result<State, String> {
    // A step that changes nothing would repeat for ever without a sleep; five in a row is a bug, said, not a spin.
    let mut last: Option<(Step, u32)> = None;
    // Whether the orchestrator's pane can be woken costs two herdr calls, and the end can wait hours: it is asked
    // once, then again every `WAKEABLE_EVERY` polls (five minutes), not at every poll.
    const WAKEABLE_EVERY: u32 = 60;
    let mut wakeable: Option<(bool, u32)> = None;
    loop {
        let mut state = load_state(project, run)?;
        let journal = project.journal(run);
        let replayed = nirvana_run::replay(&journal.read().0);
        let stop = stop_marker(project, run).exists();
        let mut caps = caps;
        if state.end.is_some() && state.logged && caps.wakeable.is_none() {
            let (known, polls) = match wakeable {
                Some((known, polls)) if polls < WAKEABLE_EVERY => (known, polls + 1),
                _ => (
                    replayed
                        .orchestrator_pane
                        .as_deref()
                        .is_some_and(|pane| (world.wakeable)(pane)),
                    0,
                ),
            };
            wakeable = Some((known, polls));
            caps.wakeable = Some(known);
        }
        let step = next(&state, &replayed, (world.now)(), &caps, stop);
        if step == Step::Done {
            if state.stage != Stage::Done {
                state.stage = Stage::Done;
                save_state(project, &state)?;
            }
            return Ok(state);
        }
        let repeats = match &last {
            Some((was, n)) if *was == step && step != Step::Wait => n + 1,
            _ => 1,
        };
        if repeats >= 5 {
            return Err(format!("the step {step:?} made no progress five times"));
        }
        last = Some((step.clone(), repeats));
        if apply(project, &mut state, &replayed, step, world)? {
            last = None;
            (world.sleep)(POLL_S);
        }
    }
}

// ---- the verb ----

/// A refusal: its exit code and its one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub code: i32,
    pub line: String,
}

fn refuse(line: impl Into<String>) -> Refused {
    Refused {
        code: 2,
        line: line.into(),
    }
}

/// Whether `root` is inside a git work tree. The gate's `owned` axis runs `git status` in an order's folder and would
/// send every order of a folder that is not one back as a worker fault (scope report §5).
pub fn is_git_repo(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok_and(|out| {
            out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true"
        })
}

/// The project's scholar runs with a state file, by run id.
pub fn scholar_runs(project: &Project) -> Vec<String> {
    let mut runs: Vec<String> = std::fs::read_dir(project.dir().join("runs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().join(DIR).join(STATE_FILE).is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    runs.sort();
    runs
}

/// What `start` made.
#[derive(Debug, Clone)]
pub struct Started {
    pub state: State,
}

/// The verb's checks and its writes: the state file and the run's `opened` line naming `pane`. It starts nothing.
pub fn start(
    project: &Project,
    question: &str,
    pane: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
    force: bool,
    now: u64,
) -> Result<Started, Refused> {
    if let Some(order) =
        env(crate::nirvana_profiles::SEATED_ORDER_ENV).filter(|order| !order.trim().is_empty())
    {
        return Err(refuse(format!(
            "refused inside order {}, a worker seated by herdr: a scholar run is the orchestrator's to start",
            order.trim()
        )));
    }
    let question = question.trim();
    if question.is_empty() {
        return Err(refuse(
            "the question is empty: herdr scholar \"<question>\"",
        ));
    }
    let length = question.chars().count();
    if length > QUESTION_MAX {
        return Err(refuse(format!(
            "the question is {length} characters; the limit is 8,000. Put the background in a file in the project and \
             name the file in the question."
        )));
    }
    if !is_git_repo(&project.root) {
        return Err(refuse(format!(
            "{} is not a git repository: the gate checks each order's files with git status, so every order would be \
             sent back. `git init` it first.",
            project.root.display()
        )));
    }
    if !force {
        for run in scholar_runs(project) {
            let unfinished =
                load_state(project, &run).is_ok_and(|state| state.stage != Stage::Done);
            if unfinished && driver_live(project, &run) {
                return Err(refuse(format!(
                    "the scholar run {run} is still running here (`herdr scholar status {run}`); --force starts another"
                )));
            }
        }
    }
    let slug = slug(question);
    let (ymd, date) = dates(now);
    // The run id needs no claim of its own: `save_state` below makes its folder, which `run_id` reads.
    let run = run_id(project, &ymd, &slug);
    let report_dir = report_dir(&project.root, &date, &slug, &claimed_dirs(project));
    let mode = nirvana_seat::table_for(&project.root).map_err(refuse)?.mode;
    let state = State {
        version: STATE_VERSION,
        run: run.clone(),
        question: question.to_owned(),
        slug,
        date,
        report_dir,
        mode,
        pane: pane.map(str::to_owned),
        stage: Stage::Route,
        started_at: now,
        stage_at: now,
        route: None,
        orders: Vec::new(),
        writer: None,
        end: None,
        end_why: None,
        ended_at: None,
        check: Vec::new(),
        notes: Vec::new(),
        told: false,
        logged: false,
        end_tries: 0,
        end_refused_at: None,
    };
    project.ensure().map_err(|err| refuse(err.to_string()))?;
    save_state(project, &state).map_err(refuse)?;
    // The run's first line names the orchestrator's pane, so the driver's seats, which run from a detached process,
    // and the asked line of a failed outcome wake that pane.
    let (pools, pools_basis) = nirvana_run::cached_pools();
    project
        .journal(&run)
        .append(
            now,
            Kind::Opened {
                project: project.root.display().to_string(),
                orchestrator_pane: pane.map(str::to_owned),
                auto: true,
                pools,
                pools_basis,
            },
        )
        .map_err(|err| refuse(err.to_string()))?;
    Ok(Started { state })
}

/// The detached driver's command: `herdr scholar drive <run>` in the project, its output in the run's driver log. It
/// inherits this process's environment, the orchestrator pane's `HERDR_PANE_ID` among it, so its seats name that pane.
pub fn driver_command(
    exe: &Path,
    project: &Project,
    run: &str,
    log: std::fs::File,
) -> std::io::Result<std::process::Command> {
    let mut command = std::process::Command::new(exe);
    command
        .args(["scholar", "drive", run])
        .env(nirvana_run::PROJECT_ENV, &project.root)
        .current_dir(&project.root)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    Ok(command)
}

/// Starts the driver detached, out of the caller's job when it allows it (a Bash tool's kill-on-close job would end
/// it with the command), as `nirvana_worker::spawn_detached` starts a supervisor.
pub fn spawn_driver(project: &Project, run: &str) -> Result<u32, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let log_path = scholar_dir(project, run).join("driver.log");
    std::fs::create_dir_all(scholar_dir(project, run)).map_err(|err| err.to_string())?;
    let spawn = |breakaway: bool| -> std::io::Result<u32> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let mut command = driver_command(&exe, project, run, log)?;
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::{
                CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW,
            };
            let mut flags = CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP;
            if breakaway {
                flags |= CREATE_BREAKAWAY_FROM_JOB;
            }
            command.creation_flags(flags);
        }
        #[cfg(not(windows))]
        let _ = breakaway;
        command.spawn().map(|child| child.id())
    };
    spawn(true)
        .or_else(|_| spawn(false))
        .map_err(|err| err.to_string())
}

// ---- status, tag ----

/// The log's lines: the run lines (with `end`) and the tag lines (`{run, outcome, note, at}`), in file order.
pub fn read_log(root: &Path) -> Vec<Value> {
    read_log_counted(root).0
}

/// The log's lines, and how many lines it skipped because they do not read as a JSON object (review F3: one bad line
/// costs only itself, and `status` says how many there were).
pub fn read_log_counted(root: &Path) -> (Vec<Value>, usize) {
    let text = std::fs::read_to_string(log_path(root)).unwrap_or_default();
    let mut lines = Vec::new();
    let mut bad = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        match serde_json::from_str::<Value>(line) {
            Ok(value) if value.is_object() => lines.push(value),
            _ => bad += 1,
        }
    }
    (lines, bad)
}

/// The run's line, unless the log already holds a run line with its id: `false` then (review F11).
pub fn append_run_line(root: &Path, line: &Value) -> Result<bool, String> {
    let run = line["run"].as_str().unwrap_or("");
    if read_log(root)
        .iter()
        .any(|known| known.get("end").is_some() && known["run"].as_str() == Some(run))
    {
        return Ok(false);
    }
    append_line(&log_path(root), line).map(|()| true)
}

/// The owner's last tag for `run`, the last line winning.
pub fn outcome_of(lines: &[Value], run: &str) -> Option<String> {
    lines
        .iter()
        .rev()
        .filter(|line| line.get("end").is_none() && line["run"].as_str() == Some(run))
        .find_map(|line| line["outcome"].as_str().map(str::to_owned))
}

pub const OUTCOMES: [&str; 3] = ["hit", "partial", "miss"];

/// `herdr scholar tag`'s line, or why it is refused: the run must have a line in the log.
pub fn tag_line(
    root: &Path,
    run: &str,
    outcome: &str,
    note: Option<&str>,
    now: u64,
) -> Result<Value, String> {
    if !OUTCOMES.contains(&outcome) {
        return Err(format!(
            "the outcome is hit, partial or miss, not {outcome:?}"
        ));
    }
    let lines = read_log(root);
    if !lines
        .iter()
        .any(|line| line.get("end").is_some() && line["run"].as_str() == Some(run))
    {
        return Err(format!(
            "no scholar run {run} in {}: a run is tagged after it ended",
            log_path(root).display()
        ));
    }
    Ok(serde_json::json!({"run": run, "outcome": outcome, "note": note, "at": now}))
}

/// `herdr scholar status` with no run: this project's runs from the log and the unfinished state files, newest
/// first, each with its outcome.
pub fn list_text(project: &Project) -> String {
    let (lines, bad) = read_log_counted(&project.root);
    let unread = (bad > 0).then(|| {
        format!("{bad} line(s) of {REPORTS}/{LOG_FILE} could not be read\n")
    });
    let mut rows: Vec<(u64, String)> = Vec::new();
    let mut logged = std::collections::BTreeSet::new();
    for line in lines.iter().filter(|line| line.get("end").is_some()) {
        let run = line["run"].as_str().unwrap_or("").to_owned();
        logged.insert(run.clone());
        let at = load_state(project, &run).map_or(0, |state| state.started_at);
        rows.push((
            at,
            format!(
                "{run} · {} · {} · outcome {} · {}",
                line["date"].as_str().unwrap_or(""),
                line["end"].as_str().unwrap_or("?"),
                outcome_of(&lines, &run).unwrap_or_else(|| "not tagged".to_owned()),
                line["report"].as_str().unwrap_or("no report")
            ),
        ));
    }
    for run in scholar_runs(project)
        .into_iter()
        .filter(|run| !logged.contains(run))
    {
        if let Ok(state) = load_state(project, &run) {
            rows.push((
                state.started_at,
                format!(
                    "{run} · {} · {}{} · {}",
                    state.date,
                    state.stage.word(),
                    if driver_live(project, &run) {
                        ""
                    } else {
                        " (no driver: `herdr scholar resume`)"
                    },
                    state.question.chars().take(80).collect::<String>()
                ),
            ));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out: String = if rows.is_empty() {
        "no scholar run in this project\n".to_owned()
    } else {
        rows.into_iter().map(|(_, row)| row + "\n").collect()
    };
    out.extend(unread);
    out
}

/// `herdr scholar status <run>`: the state file and the journal, no writes.
pub fn status_text(project: &Project, state: &State, run: &RunState) -> String {
    // Review F12: the page's path only for a run that wrote one.
    let report = (state.end.as_deref() == Some(ends::REPORT))
        .then(|| format!("report: {}\n", report_html_rel(state)))
        .unwrap_or_default();
    let mut out = format!(
        "{} · {}{}\nquestion: {}\n{report}",
        state.run,
        state.stage.word(),
        state
            .end
            .as_ref()
            .map(|end| format!(" · {end}: {}", state.end_why.as_deref().unwrap_or("")))
            .unwrap_or_default(),
        one_line(&state.question),
    );
    if let Some(route) = &state.route {
        out.push_str(&format!(
            "route: {}{}{}\n",
            route.styles.join(", "),
            if route.fell_back {
                " (the routing failed: fallback)"
            } else {
                ""
            },
            if route.note.is_empty() {
                String::new()
            } else {
                format!(" · note: {}", route.note)
            },
        ));
    }
    let line = |label: String, chain: &Chain| {
        let now = match (&chain.end, status(run, chain.active())) {
            (Some(end), _) if end.passed => "passed".to_owned(),
            (Some(end), _) => format!("not delivered: {}", end.why.as_deref().unwrap_or("")),
            (None, ChainStatus::Running(Some(leaf))) => format!("running ({leaf})"),
            (None, ChainStatus::Running(None)) => "not seated".to_owned(),
            (None, other) => format!("{other:?}"),
        };
        format!(
            "{label} {} · {} · {now}\n",
            chain.active(),
            chain
                .seat
                .as_ref()
                .map(|seat| format!("{} {}", seat.seat, seat.model))
                .unwrap_or_else(|| "no seat yet".to_owned()),
        )
    };
    for chain in &state.orders {
        out.push_str(&line(
            format!("r{} {}", chain.n, chain.style.as_deref().unwrap_or("")),
            chain,
        ));
    }
    if let Some(writer) = &state.writer {
        out.push_str(&line("writer".to_owned(), writer));
    }
    for note in &state.notes {
        out.push_str(&format!("note: {note}\n"));
    }
    if !driver_live(project, &state.run) && state.stage != Stage::Done {
        out.push_str(&format!(
            "no driver holds this run: `herdr scholar resume {}` continues it\n",
            state.run
        ));
    }
    out
}

// ---- the verbs a worker must not run ----

/// Review F6: every verb that starts paid turns, ends a run or writes the log refuses in a worker herdr seated, as the
/// question verb does: a worker told by a web page to run `herdr scholar drive <run>` would start a driver's turns.
/// `status` only reads, and stays open to a worker.
pub fn refused_in_worker(env: &dyn Fn(&str) -> Option<String>, verb: &str) -> Result<(), String> {
    match env(crate::nirvana_profiles::SEATED_ORDER_ENV).filter(|order| !order.trim().is_empty()) {
        Some(order) => Err(format!(
            "refused inside order {}, a worker seated by herdr: `herdr scholar {verb}` is the orchestrator's",
            order.trim()
        )),
        None => Ok(()),
    }
}

/// `herdr scholar drive <run>`: the guard, the state read before the lock (an unknown run id writes nothing), then
/// the driver. A driver that stops, by an error or a panic, says so on the run with the `asked` line the standard wake
/// carries (review F11: a panic left no trace). A kill cannot be caught; `status` names a run no driver holds.
pub fn drive_verb(
    project: &Project,
    run: &str,
    world: &World,
    caps: Caps,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<State, String> {
    refused_in_worker(env, "drive")?;
    load_state(project, run)?;
    let driven = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drive(project, run, world, caps)
    }))
    .unwrap_or_else(|panic| {
        let what = panic
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_owned());
        Err(format!("the driver panicked: {what}"))
    });
    match driven {
        Ok(state) => Ok(state),
        Err(why) => {
            if !why.starts_with("a driver holds") {
                let _ = project.journal(run).append(
                    (world.now)(),
                    Kind::Asked {
                        order: None,
                        question: format!(
                            "scholar {}: `herdr scholar status {run}`; the driver stopped: {why}; `herdr scholar resume {run}` goes on",
                            ends::GAVE_UP
                        ),
                    },
                );
                (world.wake)(project, run);
            }
            Err(why)
        }
    }
}

/// `herdr scholar stop <run>`: the guard and the state read before anything is written; then the stop marker, and
/// the run carried to its end here when no driver holds it.
pub fn stop_verb(
    project: &Project,
    run: &str,
    world: &World,
    caps: Caps,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    refused_in_worker(env, "stop")?;
    let state = load_state(project, run)?;
    if state.stage == Stage::Done {
        return Ok(format!(
            "{run} is done: {}",
            state.end.as_deref().unwrap_or("")
        ));
    }
    std::fs::write(stop_marker(project, run), nirvana_run::now().to_string())
        .map_err(|err| err.to_string())?;
    match take_driver(project, run) {
        Err(_) => Ok(format!(
            "{run}: asked its driver to stop; it stops the run's orders and ends the run"
        )),
        Ok(lock) => {
            let state = drive_held(project, run, world, caps)?;
            drop(lock);
            Ok(format!("{run}: {}", state.end.as_deref().unwrap_or("stopped")))
        }
    }
}

pub fn tag_verb(
    root: &Path,
    run: &str,
    outcome: &str,
    note: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
    now: u64,
) -> Result<Value, String> {
    refused_in_worker(env, "tag")?;
    let line = tag_line(root, run, outcome, note, now)?;
    // `append_line` writes a newline first when the log lost its last one (review F3).
    append_line(&log_path(root), &line)?;
    Ok(line)
}

// ---- the CLI ----

pub const USAGE: &str = "usage: herdr scholar \"<question>\" [--json] [--force]
       herdr scholar status [<run>] [--json]
       herdr scholar resume <run>
       herdr scholar stop <run>
       herdr scholar tag <run> hit|partial|miss [--note \"<text>\"]
       herdr scholar drive <run>        the driver (herdr starts it)";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn fail(line: impl std::fmt::Display, code: i32) -> ! {
    eprintln!("herdr scholar: {line}");
    std::process::exit(code);
}

fn run_arg(arg: Option<&String>) -> String {
    match arg {
        Some(run) if nirvana_run::valid_id(run) => run.clone(),
        Some(run) => fail(format!("{run:?} is not a run id"), 2),
        None => usage_exit(),
    }
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let rest = args.get(1..).unwrap_or(&[]);
    let env = |key: &str| std::env::var(key).ok();
    match args.first().map(String::as_str) {
        None => usage_exit(),
        Some("drive") => {
            let run = run_arg(rest.first());
            refused_in_worker(&env, "drive").unwrap_or_else(|why| fail(why, 2));
            match drive_verb(&project, &run, &real_world(), Caps::from_env(), &env) {
                Ok(state) => {
                    println!(
                        "{run}: {}",
                        state.end.as_deref().unwrap_or(state.stage.word())
                    );
                    Ok(())
                }
                Err(why) => fail(why, 1),
            }
        }
        Some("status") => {
            let json = rest.iter().any(|arg| arg == "--json");
            match rest.iter().find(|arg| !arg.starts_with("--")) {
                None => {
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&read_log(&project.root))?
                        );
                    } else {
                        print!("{}", list_text(&project));
                    }
                }
                Some(run) => {
                    let run = run_arg(Some(run));
                    let state = load_state(&project, &run).unwrap_or_else(|why| fail(why, 1));
                    let replayed = nirvana_run::replay(&project.journal(&run).read().0);
                    if json {
                        println!("{}", serde_json::to_string_pretty(&state)?);
                    } else {
                        print!("{}", status_text(&project, &state, &replayed));
                    }
                }
            }
            Ok(())
        }
        Some("resume") => {
            let run = run_arg(rest.first());
            refused_in_worker(&env, "resume").unwrap_or_else(|why| fail(why, 2));
            let state = load_state(&project, &run).unwrap_or_else(|why| fail(why, 1));
            if state.stage == Stage::Done {
                fail(
                    format!("{run} is done: {}", state.end.as_deref().unwrap_or("")),
                    1,
                );
            }
            if driver_live(&project, &run) {
                fail(
                    format!("a driver holds {run}: {}", holder(&project, &run)),
                    1,
                );
            }
            let pid = spawn_driver(&project, &run).unwrap_or_else(|why| fail(why, 1));
            println!(
                "{run}: the driver goes on at {} (pid {pid})",
                state.stage.word()
            );
            Ok(())
        }
        Some("stop") => {
            let run = run_arg(rest.first());
            refused_in_worker(&env, "stop").unwrap_or_else(|why| fail(why, 2));
            let said = stop_verb(&project, &run, &real_world(), Caps::from_env(), &env)
                .unwrap_or_else(|why| fail(why, 1));
            println!("{said}");
            Ok(())
        }
        Some("tag") => {
            let run = run_arg(rest.first());
            refused_in_worker(&env, "tag").unwrap_or_else(|why| fail(why, 2));
            let outcome = rest.get(1).cloned().unwrap_or_else(|| usage_exit());
            let note = match rest.get(2..).unwrap_or(&[]) {
                [] => None,
                [flag, text] if flag == "--note" => Some(text.clone()),
                _ => usage_exit(),
            };
            let line = tag_verb(
                &project.root,
                &run,
                &outcome,
                note.as_deref(),
                &env,
                nirvana_run::now(),
            )
            .unwrap_or_else(|why| fail(why, 2));
            println!("{line}");
            Ok(())
        }
        Some(_) => {
            let mut question: Option<String> = None;
            let (mut json, mut force) = (false, false);
            for arg in args {
                match arg.as_str() {
                    "--json" => json = true,
                    "--force" => force = true,
                    flag if flag.starts_with("--") => usage_exit(),
                    text if question.is_none() => question = Some(text.to_owned()),
                    _ => usage_exit(),
                }
            }
            let pane = env("HERDR_PANE_ID").filter(|pane| !pane.is_empty());
            let started = start(
                &project,
                question.as_deref().unwrap_or(""),
                pane.as_deref(),
                &env,
                force,
                nirvana_run::now(),
            )
            .unwrap_or_else(|refused| fail(refused.line, refused.code));
            let state = started.state;
            let pid = spawn_driver(&project, &state.run).unwrap_or_else(|why| fail(why, 1));
            let plain = state.pane.is_none().then(|| {
                format!(
                    "started from a plain shell: no orchestrator pane will be woken; `herdr scholar status {}` shows the result",
                    state.run
                )
            });
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "run": state.run,
                        "report_dir": state.report_dir,
                        "status": format!("herdr scholar status {}", state.run),
                        "pane": state.pane,
                        "driver_pid": pid,
                        "note": plain,
                    }))?
                );
            } else {
                println!("scholar run {} started (driver pid {pid})", state.run);
                println!("the report will be in {}/", state.report_dir);
                println!("status: herdr scholar status {}", state.run);
                if let Some(plain) = plain {
                    println!("{plain}");
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
