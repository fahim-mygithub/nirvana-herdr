//! Nirvana fork, plan 12 O3: a run's journal, the state it replays to, the decisions herdr makes without a
//! model (defer, re-seat or ask; stalls; waking the orchestrator; no dead end), the run receipt, and
//! `herdr run wake|notice|check|resume|tick|receipt|end|log|status`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §5.6, §6.1, §7.2, §7.4.1)
//!
//! The owner, 2026-10-17: "The goal is to utilize both my claude subscription and my agy gemini subsritpion
//! to buid better apps and longer running agentic workflows." (plan 12 L1), and of the receipt: "a form of run
//! receipt Nirvana had this for checking how the agents ran and how it can improve." (L11)
//!
//! - **The journal** is `<project>/.nirvana/runs/<run>/journal.jsonl`: append-only, one JSON event a line, each
//!   with `v`, `seq` and `at`. Every writer appends under `journal.lock` in the run's folder, one line per
//!   `write_all`, so the processes that write it (the orchestrator's `herdr seat run`, each worker's `herdr seat
//!   exec`, `herdr run wake`) never interleave or lose a line. Replaying it rebuilds the run (§7.2): nothing
//!   about a run lives anywhere else, so a crash, a reload of herdr's server or a new orchestrator session
//!   loses nothing.
//! - **No clock in the receipt.** An order's duration runs from `seated` to its last event (old Nirvana's
//!   `report.py`), so two calls with no new event print the same bytes, and one appended event changes them.
//! - **A figure that is missing says why**, never 0: unknown use read as 0 deferred every order forever in old
//!   Nirvana (plan 12 §3).
//!
//! The worker side (launch argv, the stream, panes, job objects) is `nirvana_worker`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::nirvana_seat::{self, Effort, Mode, RecordLine, SizeName, Tokens};
use crate::nirvana_usage::{self, Reading};

/// The journal's event schema version.
pub const V: u32 = 1;
/// The project's own folder for runs, orders and worktrees.
pub const DIR: &str = ".nirvana";
/// Runs, orders and worktrees are machine state; `lessons/` is not ignored, because O5 tracks lessons in
/// git per the owner's ruling of 2026-10-16 ("Checked lessions should go live automatically, keep it per
/// github remote").
pub const GITIGNORE: &str = "# Written by herdr (plan 12 O3). Runs, orders and worktrees are this machine's state.\n\
# lessons/ is left out on purpose: verified lessons are tracked with the project (plan 12 §7.4.3).\n\
runs/\norders/\nworktrees/\n";
pub const JOURNAL: &str = "journal.jsonl";
const JOURNAL_LOCK: &str = "journal.lock";

/// §7.2: "A headless worker whose JSON stream is silent for 10 minutes is stalled; no model is asked."
pub const STALL_AFTER_S: u64 = 600;
/// §7.2: "Restarts back off: at once, 30 s, 60 s, 120 s, then a crash-loop refusal."
pub const RESTART_DELAYS_S: [u64; 4] = [0, 30, 60, 120];
/// §5.6 step 4: "with a wake set for one minute after it".
pub const RESET_MARGIN_S: u64 = 60;
/// §7.2: "not within 30 seconds of the owner's last key in that pane" (Argon's review F7).
pub const OWNER_QUIET_S: u64 = 30;
/// R6: "a stop after 3 wakes in a row that move no order forward". Old Nirvana's 6 wakes per run would end a
/// 12-hour run in its first hour (Argon's review, §5.1).
pub const WAKE_CAP: u32 = 3;
/// The question the run asks when the wake cap stops it; found again so it is asked once.
pub const CAP_QUESTION: &str =
    "3 wakes in a row moved no order forward; the run waits for the owner";

pub fn now() -> u64 {
    nirvana_usage::unix(SystemTime::now())
}

/// An order or run id: it is typed into a pane's shell and names files, so only `[A-Za-z0-9._-]`, 1 to 64.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

// ---- the project's folder ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
}

/// Set in every worker pane: a worker's cwd is its worktree, whose git top level is not the project.
pub const PROJECT_ENV: &str = "NIRVANA_PROJECT";

impl Project {
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `$NIRVANA_PROJECT`, else the git top level of the current folder, else the folder itself.
    pub fn from_env() -> Self {
        if let Some(root) = std::env::var_os(PROJECT_ENV).filter(|v| !v.is_empty()) {
            return Self::at(PathBuf::from(root));
        }
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let top = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&cwd)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .filter(|top| !top.is_empty())
            // git prints `C:/Users/...` on Windows; the journal and the worker's paths read better in one style.
            .map(|top| {
                if cfg!(windows) {
                    top.replace('/', "\\")
                } else {
                    top
                }
            });
        Self::at(top.map_or(cwd, PathBuf::from))
    }

    pub fn dir(&self) -> PathBuf {
        self.root.join(DIR)
    }

    pub fn run_dir(&self, run: &str) -> PathBuf {
        self.dir().join("runs").join(run)
    }

    pub fn journal(&self, run: &str) -> Journal {
        Journal::new(self.run_dir(run).join(JOURNAL))
    }

    pub fn orders_dir(&self) -> PathBuf {
        self.dir().join("orders")
    }

    pub fn worktrees_dir(&self) -> PathBuf {
        self.dir().join("worktrees")
    }

    /// Creates `.nirvana/` with its `.gitignore`, which is written only when missing: the owner may edit it.
    pub fn ensure(&self) -> std::io::Result<()> {
        ensure_dir(&self.dir())
    }

    /// The run ids with a journal, in name order.
    pub fn runs(&self) -> Vec<String> {
        let mut runs: Vec<String> = std::fs::read_dir(self.dir().join("runs"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().join(JOURNAL).is_file())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
            .collect();
        runs.sort();
        runs
    }

    /// The run that seated `order`; the last one in name order when several did.
    pub fn run_of(&self, order: &str) -> Option<String> {
        self.runs()
            .into_iter()
            .rev()
            .find(|run| replay(&self.journal(run).read().0).order(order).is_some())
    }
}

/// `.nirvana/` with its `.gitignore`, in a project or in a worktree (whose checkout lacks an untracked one).
pub fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(ignore, GITIGNORE)?;
    }
    Ok(())
}

// ---- events ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub seq: u64,
    pub at: u64,
    #[serde(flatten)]
    pub kind: Kind,
}

/// What `seated` records: the pick, the order and where it runs. Everything `seat exec` needs to launch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seated {
    pub order: String,
    pub seat: String,
    pub cli: String,
    pub model: String,
    pub effort: Effort,
    pub pool: String,
    pub class: String,
    #[serde(default)]
    pub chain: Option<String>,
    pub size: SizeName,
    pub mode: Mode,
    #[serde(default)]
    pub needs: Vec<String>,
    /// Families the caller excluded (decision 0017); a re-seat keeps the exclusion.
    #[serde(default)]
    pub not_family: Vec<String>,
    pub reason: String,
    #[serde(default, rename = "override")]
    pub override_reason: Option<String>,
    /// The order's file as the worker reads it.
    pub order_file: String,
    /// The worker's folder: its worktree, or the project.
    pub cwd: String,
    #[serde(default)]
    pub worktree: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    /// The order this one continues: a re-seat or a send-back is a new order with a brief (decision 0010).
    #[serde(default)]
    pub parent: Option<String>,
    /// "reseat" or "send_back" when `parent` is set.
    #[serde(default)]
    pub why: Option<String>,
    // The pick's details, so the record line written when the order ends still has them after the lease that
    // held the pick is released (O2 review S5).
    #[serde(default)]
    pub rule_pick: Option<String>,
    #[serde(default)]
    pub bar_dropped: Option<bool>,
    #[serde(default)]
    pub step_down: Option<bool>,
    #[serde(default)]
    pub use_basis: Option<String>,
    /// The order's expected share of each window, as booked.
    #[serde(default)]
    pub expected: BTreeMap<String, f32>,
    /// Plan 12 O8b (§9 role 3): what Jev advised when more than one seat cleared the bar, beside the rule's
    /// own pick in `seat`. In shadow it changed nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jev_pick: Option<crate::nirvana_jev_roles::JevPick>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Kind {
    /// The run's first event: whose pane wakes, whether it runs in auto mode (R6), and the pools at its start.
    Opened {
        project: String,
        #[serde(default)]
        orchestrator_pane: Option<String>,
        #[serde(default)]
        auto: bool,
        #[serde(default)]
        pools: Vec<Reading>,
        #[serde(default)]
        pools_basis: Option<String>,
    },
    /// Boxed: the largest kind by far; every other event is small.
    Seated(Box<Seated>),
    Started {
        order: String,
        attempt: u32,
        #[serde(default)]
        pid: Option<u32>,
        #[serde(default)]
        pane: Option<String>,
        #[serde(default)]
        tab: Option<String>,
        /// The conversation this attempt resumes: its own, within its own seat (§7.2).
        #[serde(default)]
        resume: Option<String>,
        argv: Vec<String>,
    },
    /// Tokens so far in one attempt, cumulative; the orchestrator's own turns have no order.
    Usage {
        #[serde(default)]
        order: Option<String>,
        #[serde(default)]
        attempt: u32,
        #[serde(default)]
        session: Option<String>,
        tokens: Tokens,
    },
    RateLimited {
        order: String,
        /// "rate_limit_event", "api_retry" or "agy_exit_3".
        signal: String,
        #[serde(default)]
        pool: Option<String>,
        #[serde(default)]
        window: Option<String>,
        #[serde(default)]
        resets_at: Option<u64>,
        detail: String,
    },
    Checkpoint {
        order: String,
        /// "rate_limited", "interrupted" or "stalled".
        why: String,
        #[serde(default)]
        report: Option<Value>,
        #[serde(default)]
        last_turn: Option<String>,
        /// The worktree's diff, a file beside the journal, relative to the run's folder.
        #[serde(default)]
        diff: Option<String>,
        #[serde(default)]
        diff_bytes: Option<u64>,
    },
    Deferred {
        order: String,
        until: u64,
        why: String,
    },
    Reseated {
        order: String,
        child: String,
        seat: String,
        why: String,
    },
    Delivered {
        order: String,
        report: Value,
    },
    Gated {
        order: String,
        verdict: String,
        #[serde(default)]
        reason: Option<String>,
        /// Plan 12 O5: every axis the gate ruled on, as `nirvana_gate::Axis` writes it (§7.3: "gated with the
        /// verdict and every axis"). Empty on a hand-logged verdict and on lines written before O5.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        axes: Vec<Value>,
    },
    Merged {
        order: String,
        #[serde(default)]
        sha: Option<String>,
    },
    Asked {
        #[serde(default)]
        order: Option<String>,
        question: String,
    },
    Woken {
        /// "prompt": typed into the orchestrator's pane; "notice": printed for its prompt hook.
        via: String,
        through: u64,
        digest: String,
    },
    /// Plan 12 O8a: the owner changed the mode while the run was open (the Seats square's key, or `herdr seat
    /// mode`). Each order's own mode is on its `seated`; this says why two orders of one run sat at two modes.
    ModeChanged {
        from: Mode,
        to: Mode,
        /// "seats square" or "herdr seat mode".
        by: String,
    },
    Stalled {
        order: String,
        silent_s: u64,
        /// Stalls of this order so far, this one included.
        stalls: u32,
        /// "restart in 30 s" or "refused".
        next: String,
    },
    Interrupted {
        order: String,
        why: String,
    },
    /// Plan 12 O8b (§9 role 2): Jev read a pane worker's screen. `answer` is yes, no, unsure or failed; only a
    /// yes with `acted` set (the setting is `on`) wakes the orchestrator.
    JevWatch {
        order: String,
        answer: String,
        /// "stuck", "looping" or "waiting_for_answer", for a yes.
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        score: f64,
        #[serde(default)]
        acted: bool,
    },
    /// An order's worker ended (`order` set), or the run ended (`herdr run end`).
    Ended {
        #[serde(default)]
        order: Option<String>,
        outcome: String,
        #[serde(default)]
        why: Option<String>,
        #[serde(default)]
        exit: Option<i32>,
        #[serde(default)]
        pools: Vec<Reading>,
        #[serde(default)]
        pools_basis: Option<String>,
    },
    /// A kind a later herdr wrote: kept in the file, skipped by this replay.
    #[serde(other)]
    Unknown,
}

impl Kind {
    pub fn order(&self) -> Option<&str> {
        match self {
            Self::Seated(seated) => Some(&seated.order),
            Self::Started { order, .. }
            | Self::RateLimited { order, .. }
            | Self::Checkpoint { order, .. }
            | Self::Deferred { order, .. }
            | Self::Reseated { order, .. }
            | Self::Delivered { order, .. }
            | Self::Gated { order, .. }
            | Self::Merged { order, .. }
            | Self::Stalled { order, .. }
            | Self::JevWatch { order, .. }
            | Self::Interrupted { order, .. } => Some(order),
            Self::Usage { order, .. } | Self::Asked { order, .. } | Self::Ended { order, .. } => {
                order.as_deref()
            }
            Self::Opened { .. } | Self::Woken { .. } | Self::ModeChanged { .. } | Self::Unknown => {
                None
            }
        }
    }

    /// Moves an order forward, for the wake cap: the orchestrator seated, gated or merged, or ended the run.
    fn is_progress(&self) -> bool {
        matches!(
            self,
            Self::Seated(_) | Self::Gated { .. } | Self::Merged { .. }
        ) || matches!(self, Self::Ended { order: None, .. })
    }
}

// ---- the journal ----

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journal {
    pub path: PathBuf,
}

impl Journal {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn dir(&self) -> PathBuf {
        self.path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
    }

    /// The events in order, and how many lines did not parse (kept, never rewritten).
    pub fn read(&self) -> (Vec<Event>, usize) {
        parse_lines(&std::fs::read_to_string(&self.path).unwrap_or_default())
    }

    pub fn append(&self, at: u64, kind: Kind) -> std::io::Result<Event> {
        let (mut written, ()) = self.transact(at, |_| Ok((vec![kind], ())))?;
        Ok(written.remove(0))
    }

    /// Reads the journal and appends what `decide` returns, all under the run's lock, so a check and the
    /// event it allows are one step: two `seat run` for one order cannot both seat it (§7.2, idempotent).
    pub fn transact<R>(
        &self,
        at: u64,
        decide: impl FnOnce(&[Event]) -> std::io::Result<(Vec<Kind>, R)>,
    ) -> std::io::Result<(Vec<Event>, R)> {
        let dir = self.dir();
        let _lock = lock_journal(&dir)?;
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let (events, _) = parse_lines(&text);
        let (kinds, out) = decide(&events)?;
        // `seq` is the line number: an unparseable line still counts, so numbers never repeat.
        let lines = text.lines().filter(|line| !line.trim().is_empty()).count() as u64;
        let mut written = Vec::new();
        let mut bytes = String::new();
        for (seq, kind) in (lines + 1..).zip(kinds) {
            let event = Event {
                v: V,
                seq,
                at,
                kind,
            };
            bytes.push_str(&serde_json::to_string(&event)?);
            bytes.push('\n');
            written.push(event);
        }
        if !bytes.is_empty() {
            std::fs::create_dir_all(&dir)?;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?
                .write_all(bytes.as_bytes())?;
        }
        Ok((written, out))
    }
}

/// The journal's lock: an OS file lock on `journal.lock`, which the OS lets go when its holder ends however it
/// ends, so no stale lock is ever taken over (the O2 review's fix round moves `leases.lock` the same way).
struct JournalLock(std::fs::File);

impl Drop for JournalLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn lock_journal(dir: &Path) -> std::io::Result<JournalLock> {
    std::fs::create_dir_all(dir)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(dir.join(JOURNAL_LOCK))?;
    file.lock()?;
    Ok(JournalLock(file))
}

pub fn parse_lines(text: &str) -> (Vec<Event>, usize) {
    let mut events = Vec::new();
    let mut bad = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match serde_json::from_str::<Event>(line.trim()) {
            Ok(event) => events.push(event),
            Err(_) => bad += 1,
        }
    }
    (events, bad)
}

// ---- replay ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// Seated, no worker started yet.
    Seated,
    Running,
    /// Waiting for a window's reset; `herdr run tick` or the worker's own pane starts it again.
    Deferred,
    /// The worker gave a Work Report; the gate has not ruled.
    Delivered,
    /// Waits for an answer: a `needs_answer` report, a crash loop, or no seat left (§5.6 step 5).
    Asked,
    /// Cut mid-order: reported, never rerun; the orchestrator continues, redoes or asks (§7.2).
    Interrupted,
    Gated,
    Merged,
    /// Moved to a child order on another seat.
    Reseated,
    /// Ended with no usable report.
    Failed,
}

impl Phase {
    pub fn word(self) -> &'static str {
        match self {
            Self::Seated => "seated",
            Self::Running => "running",
            Self::Deferred => "deferred",
            Self::Delivered => "delivered",
            Self::Asked => "asked",
            Self::Interrupted => "interrupted",
            Self::Gated => "gated",
            Self::Merged => "merged",
            Self::Reseated => "reseated",
            Self::Failed => "failed",
        }
    }

    /// Nothing more happens to the order itself.
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Gated | Self::Merged | Self::Reseated | Self::Failed
        )
    }

    /// A worker may be running or due to run for it.
    pub fn wants_worker(self) -> bool {
        matches!(self, Self::Seated | Self::Running | Self::Deferred)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OrderState {
    pub seated: Seated,
    pub phase: Phase,
    pub seated_at: u64,
    pub last_at: u64,
    pub attempts: u32,
    /// The conversation id the worker's stream gave, for its own resume.
    pub session: Option<String>,
    /// Per attempt, the last cumulative usage.
    pub usage: BTreeMap<u32, Tokens>,
    pub pane: Option<String>,
    pub tab: Option<String>,
    pub deferred_until: Option<u64>,
    pub report: Option<Value>,
    pub verdict: Option<String>,
    pub merged_sha: Option<String>,
    pub outcome: Option<String>,
    pub why: Option<String>,
    pub children: Vec<String>,
    pub rate_limited: u32,
    pub deferrals: u32,
    pub reseats: u32,
    pub stalls: u32,
    pub send_backs: u32,
    pub last_checkpoint: Option<String>,
    pub last_rate_limit: Option<(Option<String>, Option<String>, Option<u64>)>,
}

impl OrderState {
    pub fn id(&self) -> &str {
        &self.seated.order
    }

    /// The order's tokens: each attempt's last cumulative figure, summed; `None` when no usage was streamed.
    pub fn tokens(&self) -> Option<Tokens> {
        if self.usage.is_empty() {
            return None;
        }
        let mut sum = Tokens::default();
        for tokens in self.usage.values() {
            sum.input += tokens.input;
            sum.cache_write += tokens.cache_write;
            sum.cache_read += tokens.cache_read;
            sum.output += tokens.output;
        }
        Some(sum)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QueueItem {
    pub seq: u64,
    pub order: Option<String>,
    pub line: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Opened {
    pub at: u64,
    pub project: String,
    pub orchestrator_pane: Option<String>,
    pub auto: bool,
    pub pools: Vec<Reading>,
    pub pools_basis: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RunState {
    pub opened: Option<Opened>,
    pub orders: Vec<OrderState>,
    /// `herdr run end`: its time and the pools then.
    pub ended: Option<(u64, Vec<Reading>, Option<String>)>,
    pub events: usize,
    pub last_seq: u64,
    pub woken_through: u64,
    /// Wake-worthy events after `woken_through`: what the orchestrator has not been told.
    pub queue: Vec<QueueItem>,
    /// Trailing prompt wakes with no order moved forward since (the last one counted until now).
    pub unproductive_wakes: u32,
    /// The wake cap's question is open: asked after the last progress.
    pub cap_asked: bool,
    pub orchestrator: Option<Tokens>,
    pub last_at: Option<u64>,
    /// Plan 12 O8a: each mode change while the run was open: its time, from, to.
    pub modes: Vec<(u64, Mode, Mode)>,
}

impl RunState {
    pub fn order(&self, id: &str) -> Option<&OrderState> {
        self.orders.iter().find(|order| order.id() == id)
    }

    fn order_mut(&mut self, id: &str) -> Option<&mut OrderState> {
        self.orders.iter_mut().find(|order| order.id() == id)
    }

    pub fn auto(&self) -> bool {
        self.opened.as_ref().is_some_and(|opened| opened.auto)
    }
}

fn short(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        let cut: String = line.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

/// "14:05Z" for a time today in UTC terms, "Oct 19 14:05Z" otherwise; never the local clock.
pub fn utc(unix: u64) -> String {
    let Ok(at) = time::OffsetDateTime::from_unix_timestamp(unix as i64) else {
        return format!("@{unix}");
    };
    format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second()
    )
}

/// The line a wake-worthy event adds to the orchestrator's queue; `None` for an event it need not hear.
fn queue_line(kind: &Kind) -> Option<String> {
    Some(match kind {
        Kind::Delivered { order, report } => {
            let status = report.get("status").and_then(Value::as_str).unwrap_or("?");
            let summary = report.get("summary").and_then(Value::as_str).unwrap_or("");
            format!("{order} delivered {status}: {}", short(summary, 90))
        }
        Kind::Asked { order, question } => match order {
            Some(order) => format!("{order} asks: {}", short(question, 120)),
            None => format!("the run asks: {}", short(question, 120)),
        },
        Kind::Stalled {
            order,
            silent_s,
            next,
            ..
        } => format!("{order} stalled ({silent_s} s silent): {next}"),
        Kind::Interrupted { order, why } => format!("{order} interrupted: {}", short(why, 90)),
        Kind::JevWatch {
            order,
            answer,
            kind,
            score,
            acted: true,
        } if answer == "yes" => format!(
            "{order}: Jev reads its screen as {} ({score:.2}); look at its pane",
            kind.as_deref().unwrap_or("stuck")
        ),
        Kind::Reseated {
            order,
            child,
            seat,
            why,
        } => format!("{order} re-seated as {child} on {seat}: {}", short(why, 80)),
        Kind::Deferred { order, until, why } => {
            format!("{order} deferred to {}: {}", utc(*until), short(why, 80))
        }
        Kind::Ended {
            order: Some(order),
            outcome,
            why,
            ..
        } if outcome != "delivered" => match why {
            Some(why) => format!("{order} ended {outcome}: {}", short(why, 90)),
            None => format!("{order} ended {outcome}"),
        },
        _ => return None,
    })
}

/// Rebuilds a run from its events. Pure: the receipt, status, check and resume all read this.
pub fn replay(events: &[Event]) -> RunState {
    let mut state = RunState {
        events: events.len(),
        ..RunState::default()
    };
    let mut wake_open = false;
    let mut streak = 0u32;
    for event in events {
        state.last_seq = state.last_seq.max(event.seq);
        state.last_at = Some(state.last_at.map_or(event.at, |at| at.max(event.at)));
        if event.kind.is_progress() {
            streak = 0;
            wake_open = false;
            state.cap_asked = false;
        }
        if let Some(order) = event.kind.order() {
            if let Some(known) = state.order_mut(order) {
                known.last_at = known.last_at.max(event.at);
            }
        }
        match &event.kind {
            Kind::Opened {
                project,
                orchestrator_pane,
                auto,
                pools,
                pools_basis,
            } => {
                if state.opened.is_none() {
                    state.opened = Some(Opened {
                        at: event.at,
                        project: project.clone(),
                        orchestrator_pane: orchestrator_pane.clone(),
                        auto: *auto,
                        pools: pools.clone(),
                        pools_basis: pools_basis.clone(),
                    });
                }
            }
            Kind::Seated(seated) => {
                if state.order(&seated.order).is_none() {
                    if seated.why.as_deref() == Some("send_back") {
                        if let Some(parent) = seated
                            .parent
                            .as_deref()
                            .and_then(|parent| state.order_mut(parent))
                        {
                            parent.send_backs += 1;
                        }
                    }
                    state.orders.push(OrderState {
                        seated: seated.as_ref().clone(),
                        phase: Phase::Seated,
                        seated_at: event.at,
                        last_at: event.at,
                        attempts: 0,
                        session: None,
                        usage: BTreeMap::new(),
                        pane: None,
                        tab: None,
                        deferred_until: None,
                        report: None,
                        verdict: None,
                        merged_sha: None,
                        outcome: None,
                        why: None,
                        children: Vec::new(),
                        rate_limited: 0,
                        deferrals: 0,
                        reseats: 0,
                        stalls: 0,
                        send_backs: 0,
                        last_checkpoint: None,
                        last_rate_limit: None,
                    });
                }
            }
            Kind::Started {
                order,
                attempt,
                pane,
                tab,
                resume,
                ..
            } => {
                if let Some(known) = state.order_mut(order) {
                    known.attempts = known.attempts.max(*attempt);
                    if !known.phase.terminal() {
                        known.phase = Phase::Running;
                    }
                    known.deferred_until = None;
                    if pane.is_some() {
                        known.pane = pane.clone();
                    }
                    if tab.is_some() {
                        known.tab = tab.clone();
                    }
                    if resume.is_some() && known.session.is_none() {
                        known.session = resume.clone();
                    }
                }
            }
            Kind::Usage {
                order,
                attempt,
                session,
                tokens,
            } => match order {
                Some(order) => {
                    if let Some(known) = state.order_mut(order) {
                        known.usage.insert(*attempt, *tokens);
                        if session.is_some() {
                            known.session = session.clone();
                        }
                    }
                }
                None => {
                    let sum = state.orchestrator.get_or_insert_with(Tokens::default);
                    sum.input += tokens.input;
                    sum.cache_write += tokens.cache_write;
                    sum.cache_read += tokens.cache_read;
                    sum.output += tokens.output;
                }
            },
            Kind::RateLimited {
                order,
                pool,
                window,
                resets_at,
                ..
            } => {
                if let Some(known) = state.order_mut(order) {
                    known.rate_limited += 1;
                    known.last_rate_limit = Some((pool.clone(), window.clone(), *resets_at));
                }
            }
            Kind::Checkpoint { order, diff, .. } => {
                if let Some(known) = state.order_mut(order) {
                    known.last_checkpoint = diff.clone();
                }
            }
            Kind::Deferred { order, until, why } => {
                if let Some(known) = state.order_mut(order) {
                    known.deferrals += 1;
                    known.deferred_until = Some(*until);
                    known.why = Some(why.clone());
                    if !known.phase.terminal() {
                        known.phase = Phase::Deferred;
                    }
                }
            }
            Kind::Reseated {
                order, child, why, ..
            } => {
                if let Some(known) = state.order_mut(order) {
                    known.reseats += 1;
                    known.children.push(child.clone());
                    known.phase = Phase::Reseated;
                    known.why = Some(why.clone());
                }
            }
            Kind::Delivered { order, report } => {
                if let Some(known) = state.order_mut(order) {
                    known.report = Some(report.clone());
                    if !known.phase.terminal() {
                        known.phase = Phase::Delivered;
                    }
                }
            }
            Kind::Gated {
                order,
                verdict,
                reason,
                ..
            } => {
                if let Some(known) = state.order_mut(order) {
                    known.verdict = Some(verdict.clone());
                    if reason.is_some() {
                        known.why = reason.clone();
                    }
                    if known.phase != Phase::Merged {
                        known.phase = Phase::Gated;
                    }
                }
            }
            Kind::Merged { order, sha } => {
                if let Some(known) = state.order_mut(order) {
                    known.merged_sha = sha.clone();
                    known.phase = Phase::Merged;
                }
            }
            Kind::Asked { order, question } => {
                if question == CAP_QUESTION {
                    state.cap_asked = true;
                }
                if let Some(known) = order.as_deref().and_then(|order| state.order_mut(order)) {
                    if !known.phase.terminal() {
                        known.phase = Phase::Asked;
                    }
                    known.why = Some(question.clone());
                }
            }
            Kind::Woken { via, through, .. } => {
                state.woken_through = state.woken_through.max(*through);
                if via == "prompt" {
                    if wake_open {
                        streak += 1;
                    }
                    wake_open = true;
                }
            }
            Kind::ModeChanged { from, to, .. } => state.modes.push((event.at, *from, *to)),
            Kind::Stalled {
                order,
                stalls,
                next,
                ..
            } => {
                if let Some(known) = state.order_mut(order) {
                    known.stalls = known.stalls.max(*stalls);
                    if next == "refused" && !known.phase.terminal() {
                        known.phase = Phase::Failed;
                        known.outcome = Some("crash_loop".into());
                    }
                }
            }
            Kind::Interrupted { order, why } => {
                if let Some(known) = state.order_mut(order) {
                    if !known.phase.terminal() {
                        known.phase = Phase::Interrupted;
                    }
                    known.why = Some(why.clone());
                }
            }
            Kind::Ended {
                order,
                outcome,
                why,
                pools,
                pools_basis,
                ..
            } => match order {
                Some(order) => {
                    if let Some(known) = state.order_mut(order) {
                        known.outcome = Some(outcome.clone());
                        if why.is_some() {
                            known.why = why.clone();
                        }
                        if known.phase == Phase::Running || known.phase == Phase::Seated {
                            known.phase = match outcome.as_str() {
                                "delivered" => Phase::Delivered,
                                "stopped" | "interrupted" => Phase::Interrupted,
                                "asked" => Phase::Asked,
                                _ => Phase::Failed,
                            };
                        }
                    }
                }
                None => {
                    if state.ended.is_none() {
                        state.ended = Some((event.at, pools.clone(), pools_basis.clone()));
                    }
                }
            },
            // Shadow or on, it moves no order: the line below is all a yes does, and only when it acted.
            Kind::JevWatch { .. } | Kind::Unknown => {}
        }
        if let Some(line) = queue_line(&event.kind) {
            state.queue.push(QueueItem {
                seq: event.seq,
                order: event.kind.order().map(str::to_owned),
                line,
            });
        }
    }
    if wake_open {
        streak += 1;
    }
    state.unproductive_wakes = streak;
    let through = state.woken_through;
    state.queue.retain(|item| item.seq > through);
    state
}

// ---- decisions herdr makes without a model ----

/// §5.6 after a typed rate-limit signal and a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateDecision {
    Defer { until: u64, why: String },
    Reseat { seat: String },
    Ask { why: String },
}

/// The re-seat pick, as the decision needs it: a seat, or none with the soonest reset among the skipped.
pub type ReseatPick = Result<String, Option<u64>>;

/// §5.6 steps 2 to 5, in order:
/// 2. defer when the spent window resets within `defer_within_s` (old Nirvana's `MAX_DEFER_S`, 30 min);
/// 3. else re-seat on what the rule picks with the spent pool marked spent;
/// 4. else, in auto mode, defer to the earliest reset known plus one minute;
/// 5. else ask the owner. Nothing is dropped silently.
pub fn rate_decision(
    now: u64,
    resets_at: Option<u64>,
    defer_within_s: u64,
    reseat: &ReseatPick,
    auto: bool,
) -> RateDecision {
    if let Some(at) = resets_at {
        if at.saturating_sub(now) <= defer_within_s {
            return RateDecision::Defer {
                until: at.max(now) + RESET_MARGIN_S,
                why: format!(
                    "the window resets at {}, within {} min",
                    utc(at),
                    defer_within_s / 60
                ),
            };
        }
    }
    let earliest = match reseat {
        Ok(seat) => return RateDecision::Reseat { seat: seat.clone() },
        Err(earliest) => [*earliest, resets_at].into_iter().flatten().min(),
    };
    match (auto, earliest) {
        (true, Some(at)) => RateDecision::Defer {
            until: at.max(now) + RESET_MARGIN_S,
            why: format!(
                "no other seat has room; auto mode waits for the earliest reset, {}",
                utc(at)
            ),
        },
        (true, None) => RateDecision::Ask {
            why: "no other seat has room and no reset time is known".into(),
        },
        (false, _) => RateDecision::Ask {
            why: "no other seat has room and the run is not in auto mode".into(),
        },
    }
}

/// §7.2: silent for 10 minutes.
pub fn stalled(last_line_at: u64, now: u64) -> bool {
    now.saturating_sub(last_line_at) >= STALL_AFTER_S
}

/// The wait before the restart that follows a stall, given the stalls before it; `None` is the crash-loop
/// refusal.
pub fn restart_delay(stalls_before: u32) -> Option<u64> {
    RESTART_DELAYS_S.get(stalls_before as usize).copied()
}

/// §7.2 "No dead end": an unfinished run with no live worker always has a pending next step. `Ok` names the
/// step; `Err` says why the run is stuck. `live` says whether an order's worker process runs.
pub fn check(state: &RunState, live: &dyn Fn(&str) -> bool, now: u64) -> Result<String, String> {
    if state.ended.is_some() {
        return Ok("the run ended".into());
    }
    if state.orders.is_empty() {
        return Err("the run has no order seated".into());
    }
    let live_orders: Vec<&str> = state
        .orders
        .iter()
        .filter(|order| live(order.id()))
        .map(OrderState::id)
        .collect();
    if !live_orders.is_empty() {
        return Ok(format!("worker live: {}", live_orders.join(", ")));
    }
    if let Some(order) = state
        .orders
        .iter()
        .find(|order| order.phase == Phase::Deferred)
    {
        let until = order.deferred_until.unwrap_or(now);
        return Ok(format!(
            "{} is deferred to {}{}",
            order.id(),
            utc(until),
            if until <= now {
                ": due, `herdr run tick` starts it"
            } else {
                ""
            }
        ));
    }
    if !state.queue.is_empty() {
        return Ok(format!(
            "{} event(s) wait for the orchestrator: `herdr run notice`",
            state.queue.len()
        ));
    }
    for (phase, step) in [
        (Phase::Delivered, "waits for the gate"),
        (Phase::Asked, "waits for an answer"),
        (
            Phase::Interrupted,
            "was interrupted: the orchestrator continues, redoes or asks",
        ),
    ] {
        if let Some(order) = state.orders.iter().find(|order| order.phase == phase) {
            return Ok(format!("{} {step}", order.id()));
        }
    }
    if let Some(order) = state.orders.iter().find(|order| order.phase.wants_worker()) {
        return Err(format!(
            "{} is {} with no live worker and no pending step: `herdr run resume`",
            order.id(),
            order.phase.word()
        ));
    }
    if state.cap_asked {
        return Ok("the wake cap asked the owner".into());
    }
    Err(
        "every order is finished and the run is not ended: `herdr run end`, or seat the next order"
            .into(),
    )
}

/// The orders `herdr run resume` reports as interrupted: they want a worker and none runs, and no deferral
/// holds them.
pub fn interrupted_steps(state: &RunState, live: &dyn Fn(&str) -> bool) -> Vec<String> {
    state
        .orders
        .iter()
        .filter(|order| matches!(order.phase, Phase::Seated | Phase::Running))
        .filter(|order| !live(order.id()))
        .map(|order| order.id().to_owned())
        .collect()
}

/// What herdr knows about the orchestrator's pane when it would wake it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaneView {
    /// herdr's agent status for the pane: idle, working, blocked, done or unknown.
    pub status: Option<String>,
    /// The prompt line's own text: `Some("")` empty, `None` when no prompt line was found on the screen.
    pub prompt: Option<String>,
    /// Unix seconds of the owner's last key in the pane (written by the server, `note_owner_key`).
    pub last_key_at: Option<u64>,
    /// What runs in the pane: a digest is typed only into Nirvana Claude (`pane_program`).
    pub program: PaneProgram,
}

/// What runs in a pane, as herdr reads it.
///
/// On 2026-10-18 the O3 build's live run woke a pane that held a plain interactive Claude Code on the owner's
/// Nirvana repository: it took the digest as a prompt and ran commands there. So a wake types only into a pane
/// shown to run Nirvana Claude, and anything else, an unread pane included, is queued for the notice route.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PaneProgram {
    /// herdr's answers were missing: never typed into.
    #[default]
    Unknown,
    NirvanaClaude,
    /// Anything else, with what herdr saw.
    Other(String),
}

impl PaneProgram {
    pub fn why(&self) -> String {
        match self {
            Self::Unknown => "herdr could not say what runs in it".into(),
            Self::NirvanaClaude => "it runs Nirvana Claude".into(),
            Self::Other(why) => why.clone(),
        }
    }
}

/// Whether a pane runs Nirvana Claude, from herdr's `pane get` and `pane process-info` answers.
///
/// Both must say so: `pane get`'s `agent` is `claude` (herdr's live detection, which drops the label when the
/// agent's process exits, while `process-info` may answer from a cached process snapshot), and one of the
/// foreground processes has Nirvana Claude's argv, `--agent nirvana` with `--plugin-dir` this home's
/// `nirvana/claude` (`nirvana_launch::nirvana_claude_args`). The argv is what tells Nirvana Claude from plain
/// Claude Code: both are `claude` to herdr's detection. `NIRVANA_ORCHESTRATOR=1` is not read: herdr exposes no
/// process's environment, and when the verb's seeded shell starts Nirvana Claude the variable comes from the
/// plugin's settings.json `env`, which reaches Claude's tools, not `claude.exe` (O4a). The herdr agent name
/// `nirvana` is a label `herdr agent start` gives; a Nirvana Claude typed into a shell has none.
pub fn pane_program(pane: &Value, process: &Value, plugin_dir: Option<&Path>) -> PaneProgram {
    let agent = ["/result/pane/agent", "/pane/agent"]
        .iter()
        .find_map(|pointer| pane.pointer(pointer))
        .and_then(Value::as_str);
    if agent != Some("claude") {
        return PaneProgram::Other(format!(
            "herdr reads its agent as {}",
            agent.unwrap_or("none")
        ));
    }
    let Some(plugin_dir) = plugin_dir else {
        return PaneProgram::Other("no Nirvana home to find Nirvana Claude's plugin in".into());
    };
    let Some(processes) = [
        "/result/process_info/foreground_processes",
        "/process_info/foreground_processes",
    ]
    .iter()
    .find_map(|pointer| process.pointer(pointer))
    .and_then(Value::as_array) else {
        return PaneProgram::Unknown;
    };
    let wanted = crate::nirvana_launch::folder_key(plugin_dir);
    let nirvana = processes.iter().any(|process| {
        let argv: Vec<&str> = match process.get("argv").and_then(Value::as_array) {
            Some(argv) => argv.iter().filter_map(Value::as_str).collect(),
            None => process
                .get("cmdline")
                .and_then(Value::as_str)
                .map(|line| line.split_whitespace().collect())
                .unwrap_or_default(),
        };
        let agent = flag_value(&argv, "--agent");
        let plugin = flag_value(&argv, "--plugin-dir");
        matches!(agent, Some("nirvana" | "nirvana:nirvana"))
            && plugin.is_some_and(|dir| crate::nirvana_launch::folder_key(Path::new(dir)) == wanted)
    });
    if nirvana {
        PaneProgram::NirvanaClaude
    } else {
        PaneProgram::Other(format!(
            "its claude has no --agent nirvana with --plugin-dir {}",
            plugin_dir.display()
        ))
    }
}

/// A flag's value in an argv: `--flag value` or `--flag=value`.
fn flag_value<'a>(argv: &[&'a str], flag: &str) -> Option<&'a str> {
    argv.iter().enumerate().find_map(|(index, arg)| {
        if *arg == flag {
            argv.get(index + 1).copied()
        } else {
            arg.strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
        }
    })
}

/// Types into a pane only when it runs Nirvana Claude: the one place a wake's text leaves herdr. The caller
/// reads the program again just before, since the pane may have changed since the wake decided.
pub fn type_if_nirvana(
    program: &PaneProgram,
    send: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if *program != PaneProgram::NirvanaClaude {
        return Err(format!(
            "the pane does not run Nirvana Claude: {}",
            program.why()
        ));
    }
    send()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wake {
    /// Nothing waits.
    Nothing,
    /// Type this digest into the pane.
    Type(String),
    /// The events wait in the queue for `herdr run notice` or a later wake.
    Queue(String),
    /// The wake cap: a stop after 3 wakes that moved no order forward (R6).
    Refuse(String),
}

/// §7.2's waking rules. `urgent` is the owner's stop, which steers a busy orchestrator; every other event is a
/// follow-up, which queues while it works (durable-pi's `whenBusy`).
pub fn wake_decision(
    run: &str,
    state: &RunState,
    view: Option<&PaneView>,
    now: u64,
    urgent: bool,
) -> Wake {
    if state.queue.is_empty() {
        return Wake::Nothing;
    }
    if state.unproductive_wakes >= WAKE_CAP && !urgent {
        return Wake::Refuse(CAP_QUESTION.into());
    }
    let Some(view) = view else {
        return Wake::Queue("the run has no orchestrator pane".into());
    };
    // Before the busy rule, and for an urgent stop too: a pane that is not Nirvana Claude is never typed into.
    if view.program != PaneProgram::NirvanaClaude {
        return Wake::Queue(format!(
            "the orchestrator's pane does not run Nirvana Claude: {}",
            view.program.why()
        ));
    }
    match view.status.as_deref() {
        Some("blocked") => {
            return Wake::Queue("the orchestrator is blocked on a prompt: retried later".into())
        }
        Some("working") if !urgent => {
            return Wake::Queue("the orchestrator is working: a follow-up queues".into())
        }
        Some("idle" | "done" | "working") => {}
        other => {
            return Wake::Queue(format!(
                "the orchestrator's state is {}",
                other.unwrap_or("not known")
            ))
        }
    }
    if let Some(at) = view.last_key_at {
        let since = now.saturating_sub(at);
        if since < OWNER_QUIET_S {
            return Wake::Queue(format!("the owner typed in the pane {since} s ago"));
        }
    }
    match view.prompt.as_deref() {
        None => Wake::Queue("no prompt line on the orchestrator's screen".into()),
        Some(text) if !text.is_empty() => Wake::Queue("the prompt line holds text".into()),
        Some(_) => Wake::Type(digest(run, &state.queue)),
    }
}

/// One short line: the orchestrator reads the detail with `herdr run notice`.
pub fn digest(run: &str, queue: &[QueueItem]) -> String {
    const MAX: usize = 400;
    let mut text = format!("herdr run {run}:");
    for (index, item) in queue.iter().enumerate() {
        let next = format!(" {};", item.line);
        if text.chars().count() + next.chars().count() > MAX - 60 {
            text.push_str(&format!(" and {} more;", queue.len() - index));
            break;
        }
        text.push_str(&next);
    }
    text.push_str(&format!(" detail: herdr run notice {run}"));
    text
}

/// The text on Claude Code's prompt line, from the pane's screen with its colours (`pane read --format
/// ansi`): `Some("")` when the line is empty, `None` when the screen shows no prompt line.
///
/// Claude Code 2.1.302, read in a herdr pane on 2026-10-18: the prompt box is the body between the screen's
/// last two horizontal rules, its first line `❯` and a no-break space, then the text. An empty prompt shows a
/// placeholder drawn faint (`ESC[2m`, `Try "fix lint errors"`), so text drawn faint, bright black or in a
/// grey 256 or true colour is not the owner's. Older Claude Code drew `│ > text │`; that is read too.
pub fn prompt_line_text(screen: &str) -> Option<String> {
    let lines: Vec<Vec<(String, bool)>> = screen.lines().map(sgr_runs).collect();
    let plain: Vec<String> = lines
        .iter()
        .map(|runs| runs.iter().map(|(text, _)| text.as_str()).collect())
        .collect();
    let rules: Vec<usize> = plain
        .iter()
        .enumerate()
        .filter(|(_, line)| is_rule(line))
        .map(|(index, _)| index)
        .collect();
    if let [.., top, bottom] = rules[..] {
        if let Some(after) = plain.get(top + 1).and_then(|line| after_mark(line)) {
            let owned: Vec<String> = lines[top + 1..bottom]
                .iter()
                .enumerate()
                .map(|(row, runs)| owned_text(runs, if row == 0 { after } else { 0 }))
                .collect();
            return Some(owned.join("\n").trim().to_owned());
        }
    }
    let index = plain.iter().rposition(|line| after_mark(line).is_some())?;
    let after = after_mark(&plain[index])?;
    Some(
        owned_text(&lines[index], after)
            .trim()
            .trim_end_matches(['│', '|'])
            .trim()
            .to_owned(),
    )
}

/// A line of `─` only, the prompt box's border.
fn is_rule(line: &str) -> bool {
    let line = line.trim();
    line.chars().count() >= 10 && line.chars().all(|c| c == '─')
}

/// The byte offset after the prompt mark (`❯` or `>`, then a space or a no-break space), when the line has one.
fn after_mark(line: &str) -> Option<usize> {
    let trimmed = line.trim_start_matches([' ', '│', '|']);
    let lead = line.len() - trimmed.len();
    let mark = ['❯', '>']
        .into_iter()
        .find(|mark| trimmed.starts_with(*mark))?;
    let rest = &trimmed[mark.len_utf8()..];
    let gap = rest.chars().next();
    match gap {
        Some(' ' | '\u{a0}') => Some(lead + mark.len_utf8() + gap.map_or(0, char::len_utf8)),
        None => Some(line.len()),
        Some(_) => None,
    }
}

/// The text of a line from byte `skip` on, leaving out what is drawn as a placeholder.
fn owned_text(runs: &[(String, bool)], skip: usize) -> String {
    let mut owned = String::new();
    let mut at = 0usize;
    for (text, dim) in runs {
        let end = at + text.len();
        if end > skip && !dim {
            owned.push_str(&text[skip.saturating_sub(at).min(text.len())..]);
        }
        at = end;
    }
    owned
}

/// Splits a line with SGR escapes into runs of text, each with whether it is drawn as placeholder.
fn sgr_runs(line: &str) -> Vec<(String, bool)> {
    let mut runs = Vec::new();
    let mut dim = false;
    let mut text = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            text.push(c);
            continue;
        }
        if chars.peek() != Some(&'[') {
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut end = None;
        for c in chars.by_ref() {
            if c.is_ascii_alphabetic() || c == '~' {
                end = Some(c);
                break;
            }
            params.push(c);
        }
        if end != Some('m') {
            continue;
        }
        if !text.is_empty() {
            runs.push((std::mem::take(&mut text), dim));
        }
        dim = sgr_dim(&params, dim);
    }
    if !text.is_empty() {
        runs.push((text, dim));
    }
    runs
}

/// Whether text after these SGR parameters is drawn as a placeholder: faint, bright black, or a grey.
fn sgr_dim(params: &str, mut dim: bool) -> bool {
    let codes: Vec<u32> = params
        .split([';', ':'])
        .map(|code| code.parse().unwrap_or(0))
        .collect();
    let mut index = 0;
    while index < codes.len() {
        match codes[index] {
            0 => dim = false,
            2 | 90 => dim = true,
            22 | 39 => dim = false,
            30..=37 | 91..=97 => dim = false,
            38 if codes.get(index + 1) == Some(&5) => {
                let n = codes.get(index + 2).copied().unwrap_or(0);
                // 8 is bright black; 232 to 255 the grey ramp, the darker two thirds of it a placeholder.
                dim = n == 8 || (232..=250).contains(&n);
                index += 2;
            }
            38 if codes.get(index + 1) == Some(&2) => {
                let rgb = [
                    codes.get(index + 2).copied().unwrap_or(0),
                    codes.get(index + 3).copied().unwrap_or(0),
                    codes.get(index + 4).copied().unwrap_or(0),
                ];
                let (max, min) = (
                    rgb.iter().copied().max().unwrap_or(0),
                    rgb.iter().copied().min().unwrap_or(0),
                );
                dim = max - min <= 16 && max <= 170;
                index += 4;
            }
            _ => {}
        }
        index += 1;
    }
    dim
}

// ---- the receipt (§7.4.1) ----

/// A figure, or why it is missing. Never a 0 that means "not known".
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Figure<T> {
    Known(T),
    Missing { missing: String },
}

impl<T> Figure<T> {
    fn missing(why: impl Into<String>) -> Self {
        Self::Missing {
            missing: why.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReceiptOrder {
    pub order: String,
    pub class: String,
    pub chain: Option<String>,
    pub seat: String,
    pub cli: String,
    pub model: String,
    pub effort: Effort,
    pub pool: String,
    pub reason: String,
    #[serde(rename = "override")]
    pub override_reason: Option<String>,
    pub parent: Option<String>,
    pub phase: Phase,
    pub tokens: Figure<Tokens>,
    pub tokens_basis: String,
    #[serde(rename = "use")]
    pub spent: Figure<BTreeMap<String, f64>>,
    pub use_basis: String,
    pub seated_at: u64,
    pub last_at: u64,
    pub duration_s: u64,
    pub attempts: u32,
    pub verdict: Figure<String>,
    pub report_status: Option<String>,
    pub send_backs: u32,
    pub reseats: u32,
    pub deferrals: u32,
    pub rate_limited: u32,
    pub stalls: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReceiptPools {
    pub readings: Figure<Vec<ReceiptReading>>,
    pub basis: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReceiptReading {
    pub pool: String,
    pub window: String,
    pub used: Figure<f64>,
    pub read_at: u64,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Receipt {
    pub v: u32,
    pub run: String,
    pub project: Figure<String>,
    pub auto: bool,
    pub events: usize,
    pub last_seq: u64,
    pub bad_lines: usize,
    pub ended: bool,
    pub orders: Vec<ReceiptOrder>,
    pub workers: Figure<Tokens>,
    pub workers_basis: String,
    pub orchestrator: Figure<Tokens>,
    pub orchestrator_basis: String,
    pub pools_start: ReceiptPools,
    pub pools_end: ReceiptPools,
}

fn round6(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

fn pools_view(readings: &[Reading], basis: Option<&str>, absent: &str) -> ReceiptPools {
    if readings.is_empty() {
        return ReceiptPools {
            readings: Figure::missing(absent),
            basis: basis.unwrap_or("none").to_owned(),
        };
    }
    ReceiptPools {
        readings: Figure::Known(
            readings
                .iter()
                .map(|reading| ReceiptReading {
                    pool: reading.pool.clone(),
                    window: reading.window.clone(),
                    used: reading.used.map_or_else(
                        || Figure::missing("the reader did not expose this window"),
                        |used| Figure::Known(round6(f64::from(used))),
                    ),
                    read_at: reading.read_at,
                    source: reading.source.clone(),
                })
                .collect(),
        ),
        basis: basis.unwrap_or("usage.json as cached").to_owned(),
    }
}

/// The receipt, from the replayed journal and `record.jsonl`. Pure, no clock: the same inputs give the same
/// receipt, byte for byte.
pub fn receipt(run: &str, state: &RunState, bad_lines: usize, record: &[RecordLine]) -> Receipt {
    let mut orders = Vec::new();
    let mut workers = Tokens::default();
    let mut any_tokens = false;
    for order in &state.orders {
        let seated = &order.seated;
        let line = record
            .iter()
            .rev()
            .find(|line| line.order == seated.order && line.run.as_deref() == Some(run));
        let tokens = order.tokens();
        if let Some(tokens) = tokens {
            any_tokens = true;
            workers.input += tokens.input;
            workers.cache_write += tokens.cache_write;
            workers.cache_read += tokens.cache_read;
            workers.output += tokens.output;
        }
        let (spent, use_basis) = match line.and_then(|line| line.spent.as_ref()) {
            Some(spent) => (
                Figure::Known(
                    spent
                        .iter()
                        .map(|(window, share)| (window.clone(), round6(f64::from(*share))))
                        .collect(),
                ),
                "record.jsonl: the order's tokens at its seat's price on its day, per window"
                    .to_owned(),
            ),
            None => (
                Figure::missing(match line {
                    Some(_) => "the record line has no use: its seat has no price or factor",
                    None => "no record line yet: the order has not ended",
                }),
                "record.jsonl".to_owned(),
            ),
        };
        orders.push(ReceiptOrder {
            order: seated.order.clone(),
            class: seated.class.clone(),
            chain: seated.chain.clone(),
            seat: seated.seat.clone(),
            cli: seated.cli.clone(),
            model: seated.model.clone(),
            effort: seated.effort,
            pool: seated.pool.clone(),
            reason: seated.reason.clone(),
            override_reason: seated.override_reason.clone(),
            parent: seated.parent.clone(),
            phase: order.phase,
            tokens: tokens.map_or_else(
                || {
                    Figure::missing(if order.attempts == 0 {
                        "no worker started"
                    } else {
                        "the worker streamed no usage"
                    })
                },
                Figure::Known,
            ),
            tokens_basis: format!(
                "the stream's last usage in each of {} attempt(s), summed",
                order.attempts
            ),
            spent,
            use_basis,
            seated_at: order.seated_at,
            last_at: order.last_at,
            duration_s: order.last_at.saturating_sub(order.seated_at),
            attempts: order.attempts,
            verdict: order.verdict.clone().map_or_else(
                || Figure::missing("no gated event: the gate has not ruled"),
                Figure::Known,
            ),
            report_status: order
                .report
                .as_ref()
                .and_then(|report| report.get("status"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            send_backs: order.send_backs,
            reseats: order.reseats,
            deferrals: order.deferrals,
            rate_limited: order.rate_limited,
            stalls: order.stalls,
        });
    }
    let opened = state.opened.as_ref();
    Receipt {
        v: V,
        run: run.to_owned(),
        project: opened.map_or_else(
            || Figure::missing("no opened event"),
            |opened| Figure::Known(opened.project.clone()),
        ),
        auto: state.auto(),
        events: state.events,
        last_seq: state.last_seq,
        bad_lines,
        ended: state.ended.is_some(),
        orders,
        workers: if any_tokens {
            Figure::Known(workers)
        } else {
            Figure::missing("no worker streamed usage")
        },
        workers_basis: "the orders' tokens above, summed".into(),
        orchestrator: state.orchestrator.map_or_else(
            || {
                Figure::missing(
                    "no orchestrator usage event in the journal (Nirvana Claude's hooks log its turns, O4)",
                )
            },
            Figure::Known,
        ),
        orchestrator_basis: "usage events with no order".into(),
        pools_start: opened.map_or_else(
            || pools_view(&[], None, "no opened event"),
            |opened| {
                pools_view(
                    &opened.pools,
                    opened.pools_basis.as_deref(),
                    "usage.json held no reading when the run opened",
                )
            },
        ),
        pools_end: match &state.ended {
            Some((_, pools, basis)) => pools_view(
                pools,
                basis.as_deref(),
                "usage.json held no reading when the run ended",
            ),
            None => pools_view(&[], None, "the run has not ended"),
        },
    }
}

fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn tokens_text(tokens: &Figure<Tokens>) -> String {
    match tokens {
        Figure::Known(t) => format!(
            "in {} · cache write {} · cache read {} · out {}",
            thousands(t.input),
            thousands(t.cache_write),
            thousands(t.cache_read),
            thousands(t.output)
        ),
        Figure::Missing { missing } => format!("missing: {missing}"),
    }
}

fn pools_text(pools: &ReceiptPools) -> String {
    match &pools.readings {
        Figure::Known(readings) => {
            let parts: Vec<String> = readings
                .iter()
                .map(|reading| {
                    let used = match &reading.used {
                        Figure::Known(used) => format!("{:.1}%", used * 100.0),
                        Figure::Missing { .. } => "not exposed".into(),
                    };
                    format!("{} {} {used}", reading.pool, reading.window)
                })
                .collect();
            format!("{} (basis: {})", parts.join(" · "), pools.basis)
        }
        Figure::Missing { missing } => format!("missing: {missing}"),
    }
}

/// The receipt as one page of text, `receipt.md`.
pub fn receipt_text(receipt: &Receipt) -> String {
    let mut out = String::new();
    let line = |out: &mut String, text: String| {
        out.push_str(&text);
        out.push('\n');
    };
    line(&mut out, format!("# Run {} receipt", receipt.run));
    line(&mut out, String::new());
    line(
        &mut out,
        format!(
            "project {} · {} · auto mode {} · journal {} events, last seq {}{}",
            match &receipt.project {
                Figure::Known(project) => project.clone(),
                Figure::Missing { missing } => format!("missing: {missing}"),
            },
            if receipt.ended { "ended" } else { "open" },
            if receipt.auto { "on" } else { "off" },
            receipt.events,
            receipt.last_seq,
            if receipt.bad_lines > 0 {
                format!(", {} line(s) unreadable", receipt.bad_lines)
            } else {
                String::new()
            }
        ),
    );
    for order in &receipt.orders {
        line(&mut out, String::new());
        line(
            &mut out,
            format!(
                "## {} · {} → {} {} ({} {}) · pool {}",
                order.order,
                order.class,
                order.seat,
                order.effort.word(),
                order.cli,
                order.model,
                order.pool
            ),
        );
        line(&mut out, String::new());
        line(&mut out, format!("- reason: {}", order.reason));
        if let Some(why) = &order.override_reason {
            line(&mut out, format!("- override: {why}"));
        }
        if let Some(parent) = &order.parent {
            line(&mut out, format!("- continues: {parent}"));
        }
        line(
            &mut out,
            format!(
                "- tokens: {} ({})",
                tokens_text(&order.tokens),
                order.tokens_basis
            ),
        );
        line(
            &mut out,
            format!(
                "- use: {} ({})",
                match &order.spent {
                    Figure::Known(spent) if spent.is_empty() => "no window priced".to_owned(),
                    Figure::Known(spent) => spent
                        .iter()
                        .map(|(window, share)| format!("{window} {:.4}%", share * 100.0))
                        .collect::<Vec<_>>()
                        .join(" · "),
                    Figure::Missing { missing } => format!("missing: {missing}"),
                },
                order.use_basis
            ),
        );
        line(
            &mut out,
            format!(
                "- duration: {} s, seated {} to its last event {}",
                order.duration_s,
                utc(order.seated_at),
                utc(order.last_at)
            ),
        );
        line(
            &mut out,
            format!(
                "- outcome: {}{} · gate {} · attempts {} · send-backs {} · re-seats {} · deferrals {} · rate-limit stops {} · stalls {}",
                order.phase.word(),
                order
                    .report_status
                    .as_ref()
                    .map(|status| format!(" (report {status})"))
                    .unwrap_or_default(),
                match &order.verdict {
                    Figure::Known(verdict) => verdict.clone(),
                    Figure::Missing { missing } => format!("missing: {missing}"),
                },
                order.attempts,
                order.send_backs,
                order.reseats,
                order.deferrals,
                order.rate_limited,
                order.stalls
            ),
        );
    }
    line(&mut out, String::new());
    line(&mut out, "## Totals".into());
    line(&mut out, String::new());
    line(
        &mut out,
        format!(
            "- workers: {} order(s), {} ({})",
            receipt.orders.len(),
            tokens_text(&receipt.workers),
            receipt.workers_basis
        ),
    );
    line(
        &mut out,
        format!(
            "- orchestrator: {} ({})",
            tokens_text(&receipt.orchestrator),
            receipt.orchestrator_basis
        ),
    );
    line(
        &mut out,
        format!("- pools at the start: {}", pools_text(&receipt.pools_start)),
    );
    line(
        &mut out,
        format!("- pools at the end: {}", pools_text(&receipt.pools_end)),
    );
    out
}

// ---- the record line an order leaves (O2's `record.jsonl`, extended) ----

/// The outcome word for the record, one of O5's five (`nirvana_gate::Outcome`): the gate's verdict when it ruled,
/// else what happened to the order. An order that ends a run with no passing gate, cut, failed or never gated,
/// did not deliver gated work: `failed`, its why kept on the journal.
pub fn outcome_word(order: &OrderState) -> String {
    use crate::nirvana_gate::Outcome;
    if let Some(verdict) = order.verdict.as_deref().and_then(Outcome::parse) {
        return verdict.word().into();
    }
    // O3's two arms are kept as they were written; its controls find them.
    match order.phase {
        Phase::Merged => "pass".into(),
        Phase::Reseated => "reseated".into(),
        Phase::Asked => Outcome::Asked.word().into(),
        _ => Outcome::Failed.word().into(),
    }
}

/// `journal` is the line's reference back to the run: `<journal path>#<last seq>`.
pub fn record_line(run: &str, order: &OrderState, journal: Option<String>) -> RecordLine {
    let seated = &order.seated;
    RecordLine {
        order: seated.order.clone(),
        run: Some(run.to_owned()),
        seat: seated.seat.clone(),
        model: Some(seated.model.clone()),
        effort: seated.effort,
        class: seated.class.clone(),
        chain: seated.chain.clone(),
        outcome: outcome_word(order),
        tokens: order.tokens(),
        spent: None,
        at: Some(order.last_at),
        v: Some(V),
        pool: Some(seated.pool.clone()),
        reason: Some(seated.reason.clone()),
        override_reason: seated.override_reason.clone(),
        seated_at: Some(order.seated_at),
        ended_at: Some(order.last_at),
        send_backs: Some(order.send_backs),
        reseats: Some(order.reseats),
        deferrals: Some(order.deferrals),
        rate_limit_stops: Some(order.rate_limited),
        stalls: Some(order.stalls),
        rule_pick: seated.rule_pick.clone(),
        mode: Some(seated.mode),
        size: Some(seated.size),
        bar_dropped: seated.bar_dropped,
        step_down: seated.step_down,
        use_basis: seated.use_basis.clone(),
        expected: (!seated.expected.is_empty()).then(|| seated.expected.clone()),
        herdr: Some(crate::nirvana::VERSION.to_owned()),
        journal,
        extra: serde_json::Map::new(),
    }
}

/// Writes the order's record line once, through O2's `record_once`: under `leases.lock`, a line for the same
/// order and run is never written twice, and a booking still live fills what the journal lacks.
pub fn write_record(
    run: &str,
    order: &OrderState,
    journal: Option<String>,
) -> std::io::Result<bool> {
    let line = record_line(run, order, journal);
    let line = match nirvana_seat::table() {
        Ok(table) => {
            nirvana_seat::complete(&table, line.clone(), line.at.unwrap_or(0)).unwrap_or(line)
        }
        Err(_) => line,
    };
    let (written, _) = nirvana_seat::record_once(&nirvana_seat::dir(), line, true, now())?;
    Ok(written)
}

/// The journal reference a record line carries.
pub fn journal_ref(journal: &Journal, state: &RunState) -> Option<String> {
    Some(format!("{}#{}", journal.path.display(), state.last_seq))
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr run status [<run>] [--json]
       herdr run wake <run> [--urgent]       type the queued events into the orchestrator's pane, if it is idle
       herdr run notice [<run>]              print the queued events (for the UserPromptSubmit hook)
       herdr run check <run>                 exit 1 when an unfinished run has no live worker and no next step
       herdr run resume <run>                report each interrupted step, never rerunning it
       herdr run tick [<run>]                start what a deferral holds once it is due; wake when idle
       herdr run receipt <run> [--json]
       herdr run end <run>                   end the run; write receipt.md and receipt.json beside the journal
       herdr run log <run> --json '<event>'  append gated, merged, asked or usage (the orchestrator's turns)
       herdr run retro <run> | retro reject <id> [--why text] | retro propose --json '<proposal>' | retro list";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn run_arg(project: &Project, arg: Option<&String>) -> String {
    match arg {
        Some(run) if valid_id(run) => run.clone(),
        Some(run) => {
            eprintln!("herdr run: {run:?} is not a run id");
            std::process::exit(2);
        }
        None => match project.runs().pop() {
            Some(run) => run,
            None => {
                eprintln!("herdr run: no run in {}", project.dir().display());
                std::process::exit(1);
            }
        },
    }
}

fn load(project: &Project, run: &str) -> (Journal, Vec<Event>, usize, RunState) {
    let journal = project.journal(run);
    if !journal.path.is_file() {
        eprintln!("herdr run: no journal {}", journal.path.display());
        std::process::exit(1);
    }
    let (events, bad) = journal.read();
    let state = replay(&events);
    (journal, events, bad, state)
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        Some("status") => crate::nirvana_worker::status_cli(rest, true),
        Some("wake") => {
            let urgent = rest.iter().any(|arg| arg == "--urgent");
            let run = run_arg(&project, rest.iter().find(|arg| !arg.starts_with("--")));
            let (verdict, _) = wake(&project, &run, urgent)?;
            println!("{verdict}");
            Ok(())
        }
        Some("notice") => notice_cli(&project, rest),
        Some("check") => {
            let run = run_arg(&project, rest.first());
            let (_, _, _, state) = load(&project, &run);
            let live = |order: &str| crate::nirvana_worker::live(&project, &run, order);
            match check(&state, &live, now()) {
                Ok(step) => {
                    println!("{run}: {step}");
                    Ok(())
                }
                Err(why) => {
                    println!("{run}: dead end: {why}");
                    std::process::exit(1);
                }
            }
        }
        Some("resume") => resume_cli(&project, rest),
        Some("tick") => tick_cli(&project, rest),
        Some("receipt") => {
            let json = rest.iter().any(|arg| arg == "--json");
            let run = run_arg(&project, rest.iter().find(|arg| !arg.starts_with("--")));
            let (_, _, bad, state) = load(&project, &run);
            let record =
                nirvana_seat::read_record(&nirvana_seat::dir().join(nirvana_seat::RECORD_FILE));
            let receipt = receipt(&run, &state, bad, &record);
            if json {
                println!("{}", receipt_json(&project, &receipt)?);
            } else {
                print!("{}", receipt_page(&project, &receipt));
            }
            Ok(())
        }
        Some("end") => end_cli(&project, rest),
        // Plan 12 O5: the retro (§7.4.2).
        Some("retro") => crate::nirvana_retro::cli(&project, rest),
        Some("log") => log_cli(&project, rest),
        _ => usage_exit(),
    }
}

/// The readings `usage.json` holds now, with their basis; never a new read, so opening a run costs nothing.
pub fn cached_pools() -> (Vec<Reading>, Option<String>) {
    let Some(path) = nirvana_usage::cache_path() else {
        return (Vec::new(), Some("no herdr home".into()));
    };
    let cache = nirvana_usage::read_cache(&path);
    let readings: Vec<Reading> = cache
        .sources
        .values()
        .flat_map(|source| source.readings.iter().cloned())
        .filter(|reading| reading.pool != "local")
        .collect();
    (
        readings,
        Some("usage.json as cached; each reading's read_at is its age".into()),
    )
}

/// The readings at a run's end: a reader whose entry is stale is read again first (0 tokens, about 4 s), as
/// `herdr usage` does, so the end figure is not the start figure carried over. The live run of 2026-10-18 read
/// the same cache at both ends before this.
fn current_pools() -> (Vec<Reading>, Option<String>) {
    let readings: Vec<Reading> = nirvana_usage::current_sources(false)
        .iter()
        .flat_map(|source| source.readings.iter().cloned())
        .filter(|reading| reading.pool != "local")
        .collect();
    (
        readings,
        Some("usage.json, stale readers read again at the run's end".into()),
    )
}

pub fn delivered_ungated(state: &RunState) -> Vec<String> {
    state
        .orders
        .iter()
        .filter(|order| order.phase == Phase::Delivered)
        .map(|order| order.id().to_owned())
        .collect()
}

pub fn end_decision(state: &RunState, args: &[String]) -> Result<Vec<Kind>, String> {
    let mut abandon: Option<String> = None;
    let mut why: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--abandon" => {
                i += 1;
                if i < args.len() {
                    abandon = Some(args[i].clone());
                } else {
                    return Err("--abandon requires an order id".into());
                }
            }
            "--why" => {
                i += 1;
                if i < args.len() {
                    why = Some(args[i].clone());
                } else {
                    return Err("--why requires text".into());
                }
            }
            _ => {}
        }
        i += 1;
    }
    let mut kinds = Vec::new();
    if let Some(order) = abandon {
        let reason = why.ok_or_else(|| "--abandon requires --why <text>".to_string())?;
        kinds.push(Kind::Gated {
            order: order.clone(),
            verdict: "abandoned".into(),
            reason: Some(reason),
            axes: Vec::new(),
        });
    }
    if state.ended.is_none() {
        let ungated: Vec<String> = delivered_ungated(state)
            .into_iter()
            .filter(|id| {
                !kinds
                    .iter()
                    .any(|k| matches!(k, Kind::Gated { order, .. } if order == id))
            })
            .collect();
        if !ungated.is_empty() {
            return Err(format!(
                "delivered order(s) not gated: {}; gate each one or abandon with --abandon <order> --why <text>",
                ungated.join(", ")
            ));
        }
    }
    Ok(kinds)
}

fn end_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let mut run_opt: Option<String> = None;
    for arg in args {
        if !arg.starts_with("--") && run_opt.is_none() {
            run_opt = Some(arg.to_owned());
        }
    }
    let run = run_arg(project, run_opt.as_ref());
    let (journal, _, _, _) = load(project, &run);
    let (events, _) = journal.read();
    let state = replay(&events);

    let kinds = match end_decision(&state, args) {
        Ok(k) => k,
        Err(err) => {
            eprintln!("herdr run end {run}: {err}");
            std::process::exit(2);
        }
    };

    for kind in kinds {
        journal.append(now(), kind)?;
    }

    let (events, _) = journal.read();
    let state = replay(&events);
    let (pools, basis) = if state.ended.is_none() {
        current_pools()
    } else {
        (Vec::new(), None)
    };
    if state.ended.is_none() {
        journal.append(
            now(),
            Kind::Ended {
                order: None,
                outcome: "ended".into(),
                why: None,
                exit: None,
                pools,
                pools_basis: basis,
            },
        )?;
    }
    let (events, bad) = journal.read();
    let state = replay(&events);
    for order in &state.orders {
        if order.phase.terminal() || !order.phase.wants_worker() {
            write_record(&run, order, journal_ref(&journal, &state))?;
        }
    }
    // Plan 12 O5: the retro reads the ended journal, before the receipt that names its open proposals.
    match crate::nirvana_retro::retro(project, &run) {
        Ok(summary) => eprintln!("{summary}"),
        Err(err) => eprintln!("herdr run retro {run}: {err}"),
    }
    let record = nirvana_seat::read_record(&nirvana_seat::dir().join(nirvana_seat::RECORD_FILE));
    let receipt = receipt(&run, &state, bad, &record);
    let dir = journal.dir();
    nirvana_seat::write_atomic(
        &dir.join("receipt.json"),
        receipt_json(project, &receipt)?.as_bytes(),
    )?;
    nirvana_seat::write_atomic(
        &dir.join("receipt.md"),
        receipt_page(project, &receipt).as_bytes(),
    )?;
    println!("{}", dir.join("receipt.md").display());
    Ok(())
}

/// The receipt's page with the project's open retro proposals after it (plan 12 §7.4.2: "Because proposals sat
/// unread for 24 days before, the receipt ... name[s] the open ones"). `receipt_text` stays the journal's alone.
fn receipt_page(project: &Project, receipt: &Receipt) -> String {
    let mut page = receipt_text(receipt);
    page.push_str(&crate::nirvana_retro::open_section(project));
    page
}

fn receipt_json(project: &Project, receipt: &Receipt) -> std::io::Result<String> {
    let mut value = serde_json::to_value(receipt)?;
    value["open_proposals"] = serde_json::to_value(crate::nirvana_retro::open_ids(project))?;
    Ok(serde_json::to_string_pretty(&value)?)
}

fn log_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let [run, flag, text] = args else {
        usage_exit()
    };
    if flag != "--json" || !valid_id(run) {
        usage_exit();
    }
    let journal = project.journal(run);
    let mut value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("herdr run log: not JSON: {err}");
            std::process::exit(2);
        }
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("v".into(), V.into());
        object.insert("seq".into(), 0.into());
        object.insert("at".into(), 0.into());
    }
    let kind = match serde_json::from_value::<Event>(value) {
        Ok(event) => event.kind,
        Err(err) => {
            eprintln!("herdr run log: not an event: {err}");
            std::process::exit(2);
        }
    };
    // The orchestrator logs its own rulings and turns; every other kind is herdr's to write.
    if !matches!(
        kind,
        Kind::Gated { .. }
            | Kind::Merged { .. }
            | Kind::Asked { .. }
            | Kind::Usage { order: None, .. }
    ) {
        eprintln!("herdr run log: only gated, merged, asked and the orchestrator's usage are logged by hand");
        std::process::exit(2);
    }
    let (written, ()) = journal.transact(now(), |events| {
        let state = replay(events);
        if let Some(order) = kind.order() {
            if state.order(order).is_none() {
                return Err(std::io::Error::other(format!("no order {order} in {run}")));
            }
        }
        Ok((vec![kind.clone()], ()))
    })?;
    let event = &written[0];
    if let Kind::Gated { order, .. } | Kind::Merged { order, .. } = &event.kind {
        let state = replay(&journal.read().0);
        if let Some(known) = state.order(order) {
            if known.phase.terminal() {
                write_record(run, known, journal_ref(&journal, &state))?;
            }
        }
    }
    println!("{}", serde_json::to_string(event)?);
    Ok(())
}

fn notice_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let runs = match args.first() {
        Some(run) => vec![run_arg(project, Some(run))],
        None => project.runs(),
    };
    for run in runs {
        let journal = project.journal(&run);
        let (written, lines) = journal.transact(now(), |events| {
            let state = replay(events);
            if state.queue.is_empty() || state.ended.is_some() {
                return Ok((Vec::new(), Vec::new()));
            }
            let lines: Vec<String> = state.queue.iter().map(|item| item.line.clone()).collect();
            Ok((
                vec![Kind::Woken {
                    via: "notice".into(),
                    through: state.last_seq,
                    digest: digest(&run, &state.queue),
                }],
                lines,
            ))
        })?;
        if !written.is_empty() {
            println!("herdr run {run}:");
            for line in lines {
                println!("- {line}");
            }
        }
    }
    Ok(())
}

/// Wakes the orchestrator when §7.2 allows it, else leaves the events queued. Returns the verdict line and
/// whether a digest was typed.
pub fn wake(project: &Project, run: &str, urgent: bool) -> std::io::Result<(String, bool)> {
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    if state.ended.is_some() {
        return Ok(("the run has ended".into(), false));
    }
    let pane = state
        .opened
        .as_ref()
        .and_then(|opened| opened.orchestrator_pane.clone());
    let view = pane.as_deref().map(crate::nirvana_worker::pane_view);
    let verdict = wake_decision(run, &state, view.as_ref(), now(), urgent);
    match verdict {
        Wake::Nothing => Ok(("nothing waits".into(), false)),
        Wake::Queue(why) => Ok((
            format!("{} event(s) queued: {why}", state.queue.len()),
            false,
        )),
        Wake::Refuse(question) => {
            journal.transact(now(), |events| {
                let state = replay(events);
                Ok((
                    if state.cap_asked {
                        Vec::new()
                    } else {
                        vec![Kind::Asked {
                            order: None,
                            question: question.clone(),
                        }]
                    },
                    (),
                ))
            })?;
            Ok((format!("refused: {question}"), false))
        }
        Wake::Type(digest) => {
            let pane = pane.unwrap_or_default();
            match crate::nirvana_worker::type_prompt(&pane, &digest) {
                Ok(()) => {
                    journal.append(
                        now(),
                        Kind::Woken {
                            via: "prompt".into(),
                            through: state.last_seq,
                            digest: digest.clone(),
                        },
                    )?;
                    Ok((format!("woke {pane}: {digest}"), true))
                }
                Err(why) => Ok((
                    format!("{} event(s) queued: {why}", state.queue.len()),
                    false,
                )),
            }
        }
    }
}

fn resume_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let run = run_arg(project, args.first());
    let journal = project.journal(&run);
    let live = |order: &str| crate::nirvana_worker::live(project, &run, order);
    let at = now();
    let mut reported = Vec::new();
    for order in interrupted_steps(&replay(&journal.read().0), &live) {
        let seated = replay(&journal.read().0)
            .order(&order)
            .map(|known| known.seated.clone());
        let Some(seated) = seated else { continue };
        let (diff, bytes) = crate::nirvana_worker::write_diff(
            &journal.dir(),
            &order,
            "resume",
            Path::new(&seated.cwd),
        );
        journal.transact(at, |events| {
            // Checked again under the lock: a worker that came back meanwhile is not cut.
            let state = replay(events);
            let still = state
                .order(&order)
                .is_some_and(|known| matches!(known.phase, Phase::Seated | Phase::Running));
            if !still || live(&order) {
                return Ok((Vec::new(), ()));
            }
            Ok((
                vec![
                    Kind::Checkpoint {
                        order: order.clone(),
                        why: "interrupted".into(),
                        report: None,
                        last_turn: None,
                        diff: diff.clone(),
                        diff_bytes: bytes,
                    },
                    Kind::Interrupted {
                        order: order.clone(),
                        why: "no live worker when the run was resumed".into(),
                    },
                ],
                (),
            ))
        })?;
        reported.push(order);
    }
    let state = replay(&journal.read().0);
    println!(
        "run {run}: {} order(s){}",
        state.orders.len(),
        if state.auto() {
            ", auto mode: continue without asking"
        } else {
            ""
        }
    );
    for order in &state.orders {
        let mark = if reported.iter().any(|id| id == order.id()) {
            "  <- interrupted now; continue, redo or ask"
        } else {
            ""
        };
        println!(
            "  {} {} on {} {}{}",
            order.id(),
            order.phase.word(),
            order.seated.seat,
            order.seated.effort.word(),
            mark
        );
    }
    match check(&state, &live, at) {
        Ok(step) => println!("next: {step}"),
        Err(why) => println!("dead end: {why}"),
    }
    Ok(())
}

fn tick_cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let runs = match args.first() {
        Some(run) => vec![run_arg(project, Some(run))],
        None => project.runs(),
    };
    let at = now();
    for run in runs {
        let state = replay(&project.journal(&run).read().0);
        if state.ended.is_some() {
            continue;
        }
        for order in &state.orders {
            let due = order.phase == Phase::Deferred
                && order.deferred_until.is_some_and(|until| until <= at);
            if due && !crate::nirvana_worker::live(project, &run, order.id()) {
                match crate::nirvana_worker::launch_worker(project, &run, &order.seated) {
                    Ok(how) => println!("{run}: {} started again ({how})", order.id()),
                    Err(err) => println!("{run}: {} could not start: {err}", order.id()),
                }
            }
        }
        // Plan 12 O8b (§9 role 2): Jev reads the long-running pane workers' screens before the wake, so an
        // `on` yes is in the queue this wake carries and goes through its guard and the owner-typing rule.
        for line in crate::nirvana_jev_roles::watch_run(project, &run)? {
            println!("{run}: {line}");
        }
        let (verdict, _) = wake(project, &run, false)?;
        println!("{run}: {verdict}");
    }
    Ok(())
}

// ---- the owner's last key, from the server (§7.2, Argon's review F7) ----

/// The last time each pane's key was written to disk, so a typing owner costs one small write a second.
static KEYS_WRITTEN: std::sync::Mutex<Option<BTreeMap<String, u64>>> = std::sync::Mutex::new(None);

fn key_file(home: &Path, pane: &str) -> PathBuf {
    let safe: String = pane
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    home.join(nirvana_seat::DIR).join("keys").join(safe)
}

/// Called by herdr's server when it forwards an owner's key to a pane. Writes the second, at most once a
/// second per pane, to `<home>/nirvana/keys/<pane>`; `herdr run wake` reads it in another process.
pub fn note_owner_key(pane: &str) {
    let at = now();
    {
        let mut written = KEYS_WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
        let map = written.get_or_insert_with(BTreeMap::new);
        if map.get(pane).is_some_and(|last| *last >= at) {
            return;
        }
        map.insert(pane.to_owned(), at);
    }
    if let Some(home) = crate::nirvana::home() {
        let path = key_file(&home, pane);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, at.to_string());
    }
}

pub fn owner_key_at(pane: &str) -> Option<u64> {
    let home = crate::nirvana::home()?;
    std::fs::read_to_string(key_file(&home, pane))
        .ok()?
        .trim()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests;
