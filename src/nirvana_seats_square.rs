//! Nirvana fork, plan 12 O8a: what the Seats square shows, read from the run journals and the leases, and the
//! mode it changes. The square and its panel are drawn by `client::shell::nirvana_seats`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §5.3 step 5, §8)
//!
//! The plan, §8: "A Seats square in the mod strip of a Nirvana tab: each open order with its seat, model and
//! effort, pool, state, time and use; its reason line; a click focuses the worker's pane." and "The mode
//! (Efficient, Balanced, SOTA) in the Seats square's header, one key to change it."
//!
//! - **The read is files only.** `<project>/.nirvana/runs/*/journal.jsonl` replayed (O3's `replay`), herdr's
//!   `leases.json` and `seats.toml`. No usage reader, no `git`, no CLI: the use an order shows is its own
//!   streamed tokens at its seat's price (the receipt's basis), else what its booking expects. The window asks
//!   for it while it draws, so `cached` only hands back the last view; the reads run on a thread of their own,
//!   started by the window's timer tick (`take_changed`) at most every `REREAD`, and a journal whose length and
//!   time did not change is not replayed again.
//! - **An open order** is one of a run that has not ended whose phase is not terminal (O3's `Phase::terminal`),
//!   and a live lease of such a run that no `seated` names yet ("booked"). A lease of a run that is not in this
//!   project is left out: `leases.json` is the machine's, and a lease does not name its project.
//! - **The mode** is `seats.toml`'s, where `herdr seat pick` reads its default; a change during an open run is a
//!   `mode_changed` event in that run's journal.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use crate::nirvana_run::{self, Kind, Phase, Project, RunState};
use crate::nirvana_seat::{self, Effort, Leases, Mode, Table};

/// How often the window's copy is read again; Running's transcript is read as often (feature 6).
pub const REREAD: Duration = Duration::from_secs(2);
/// A project the window has not drawn for this long is no longer read.
const FORGET_AFTER: Duration = Duration::from_secs(30);

/// What an order's use figure is based on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseBasis {
    /// Its streamed tokens at its seat's price on the day it was seated, as the receipt computes it.
    Spent,
    /// What the pick expected it to spend, from its booking: no usage was streamed yet.
    Booked,
}

/// One window's share of an order: the window the order weighs most on.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderUse {
    pub window: String,
    pub share: f32,
    pub basis: UseBasis,
}

/// One open order, as the square shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub run: String,
    pub order: String,
    pub class: String,
    pub seat: String,
    pub model: String,
    pub effort: Effort,
    pub pool: String,
    /// O3's phase word, or "booked" for a lease no `seated` names yet.
    pub state: String,
    /// The order waits on someone: asked, or interrupted (the orchestrator continues, redoes or asks).
    pub attention: bool,
    /// Seated, or booked, at.
    pub since: u64,
    pub deferred_until: Option<u64>,
    pub use_: Option<OrderUse>,
    /// §5.3 step 5's reason line, as `seat pick` gave it.
    pub reason: String,
    pub override_reason: Option<String>,
    /// The mode it was seated at.
    pub mode: Option<Mode>,
    /// The worker's pane, its public id (`HERDR_PANE_ID` of the attempt's `seat exec`).
    pub pane: Option<String>,
}

/// One open run: §5.3 step 4, "the journal counts each run's classes, and the Seats square shows them".
#[derive(Debug, Clone, PartialEq)]
pub struct RunLine {
    pub run: String,
    pub orders: usize,
    pub classes: BTreeMap<String, u32>,
    pub overrides: u32,
    /// The last mode change in the run: its time and the mode it went to.
    pub mode_changed: Option<(u64, Mode)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    /// `seats.toml`'s mode; `None` when the table is refused, with why in `note`.
    pub mode: Option<Mode>,
    pub note: Option<String>,
    pub runs: Vec<RunLine>,
    pub rows: Vec<Row>,
}

impl View {
    /// The orders waiting on someone.
    pub fn attention(&self) -> usize {
        self.rows.iter().filter(|row| row.attention).count()
    }
}

fn largest(shares: impl IntoIterator<Item = (String, f32)>, basis: UseBasis) -> Option<OrderUse> {
    shares
        .into_iter()
        .filter(|(_, share)| share.is_finite())
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(window, share)| OrderUse {
            window,
            share,
            basis,
        })
}

/// An order's use: spent when it streamed tokens and its seat has a price, else what it booked.
fn order_use(table: Option<&Table>, order: &nirvana_run::OrderState) -> Option<OrderUse> {
    let spent = table.zip(order.tokens()).and_then(|(table, tokens)| {
        let seat = table.seat(&order.seated.seat)?;
        nirvana_seat::computed_use(table, seat, &tokens, order.seated_at)
    });
    match spent {
        Some(spent) => largest(spent, UseBasis::Spent),
        None => largest(order.seated.expected.clone(), UseBasis::Booked),
    }
}

/// The view, from runs already replayed. Pure: the tests compose it from recorded journals and leases.
pub fn compose(
    table: Option<&Table>,
    note: Option<String>,
    runs: &[(String, RunState)],
    leases: &Leases,
    now: u64,
) -> View {
    let mut view = View {
        mode: table.map(|table| table.mode),
        note,
        ..View::default()
    };
    for (run, state) in runs.iter().filter(|(_, state)| state.ended.is_none()) {
        let mut classes = BTreeMap::new();
        for order in &state.orders {
            *classes.entry(order.seated.class.clone()).or_insert(0) += 1;
        }
        view.runs.push(RunLine {
            run: run.clone(),
            orders: state.orders.len(),
            classes,
            overrides: state
                .orders
                .iter()
                .filter(|order| order.seated.override_reason.is_some())
                .count() as u32,
            mode_changed: state.modes.last().map(|(at, _, to)| (*at, *to)),
        });
        for order in state.orders.iter().filter(|order| !order.phase.terminal()) {
            let seated = &order.seated;
            view.rows.push(Row {
                run: run.clone(),
                order: seated.order.clone(),
                class: seated.class.clone(),
                seat: seated.seat.clone(),
                model: seated.model.clone(),
                effort: seated.effort,
                pool: seated.pool.clone(),
                state: order.phase.word().to_owned(),
                attention: matches!(order.phase, Phase::Asked | Phase::Interrupted),
                since: order.seated_at,
                deferred_until: order
                    .deferred_until
                    .filter(|_| order.phase == Phase::Deferred),
                use_: order_use(table, order),
                reason: seated.reason.clone(),
                override_reason: seated.override_reason.clone(),
                mode: Some(seated.mode),
                pane: order.pane.clone(),
            });
        }
        for lease in leases.leases.iter().filter(|lease| {
            lease.live(now)
                && lease.run.as_deref() == Some(run.as_str())
                && state.order(&lease.order).is_none()
        }) {
            let pick = |key: &str| {
                lease
                    .pick
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            };
            view.rows.push(Row {
                run: run.clone(),
                order: lease.order.clone(),
                class: lease.class.clone(),
                seat: lease.seat.clone(),
                model: pick("model").unwrap_or_default(),
                effort: lease.effort,
                pool: lease.pool.clone(),
                state: "booked".into(),
                attention: false,
                since: lease.booked_at,
                deferred_until: None,
                use_: largest(lease.expected.clone(), UseBasis::Booked),
                reason: pick("reason").unwrap_or_default(),
                override_reason: pick("override"),
                mode: pick("mode").as_deref().and_then(Mode::parse),
                pane: None,
            });
        }
    }
    view
}

/// "0.02%", "1.5%", "38%".
pub fn percent(share: f32) -> String {
    let pct = share * 100.0;
    if pct < 1.0 {
        format!("{pct:.2}%")
    } else if pct < 10.0 {
        format!("{pct:.1}%")
    } else {
        format!("{pct:.0}%")
    }
}

/// The use as the square writes it: "5h 0.02%", or "5h ~1.2% booked" before any token was streamed.
pub fn use_text(use_: Option<&OrderUse>) -> String {
    match use_ {
        None => "use not known".into(),
        Some(OrderUse {
            window,
            share,
            basis: UseBasis::Spent,
        }) => format!("{window} {}", percent(*share)),
        Some(OrderUse {
            window,
            share,
            basis: UseBasis::Booked,
        }) => format!("{window} ~{} booked", percent(*share)),
    }
}

/// The time as the square writes it: "until 14:05Z" for a deferral, else how long since it was seated.
pub fn time_text(row: &Row, now: u64) -> String {
    match row.deferred_until {
        Some(until) => format!(
            "until {}",
            nirvana_run::utc(until)
                .get(11..16)
                .map_or_else(|| nirvana_run::utc(until), |hm| format!("{hm}Z"))
        ),
        None => crate::nirvana_running::elapsed(now.saturating_sub(row.since)),
    }
}

/// The project folder for the agent's folder: its git root, read from the folder tree, else the folder.
/// `Project::from_env` asks `git`; the window must not start a process while it draws.
pub fn project_root(cwd: &Path) -> PathBuf {
    crate::nirvana_board::root(cwd).unwrap_or_else(|| cwd.to_path_buf())
}

// ---- the read ----

/// One file's length and time: a journal whose signature did not change is not replayed again.
type Signature = Option<(u64, Option<SystemTime>)>;

fn signature(path: &Path) -> Signature {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// The table and why it is not used. A missing `seats.toml` reads as herdr's default, which is what the
/// next `seat pick` writes; this read never writes it.
fn read_table(seats_dir: &Path) -> (Option<Table>, Option<String>) {
    let path = seats_dir.join(nirvana_seat::SEATS_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| nirvana_seat::DEFAULT.to_owned());
    match nirvana_seat::parse(&text) {
        Ok(table) => (Some(table), None),
        Err(_) => (
            None,
            Some(format!(
                "{} is refused: herdr seat pick says why",
                nirvana_seat::SEATS_FILE
            )),
        ),
    }
}

/// Replayed runs, kept per journal with its signature.
type Replays = HashMap<PathBuf, (Signature, RunState)>;

/// Reads the project's view. `replays` keeps each journal's last replay; an unchanged journal is reused.
fn read(root: &Path, seats_dir: &Path, replays: &mut Replays, now: u64) -> View {
    let project = Project::at(root);
    let mut runs = Vec::new();
    for run in project.runs() {
        let journal = project.journal(&run);
        let sig = signature(&journal.path);
        let state = match replays.get(&journal.path) {
            Some((known, state)) if *known == sig => state.clone(),
            _ => {
                let state = nirvana_run::replay(&journal.read().0);
                replays.insert(journal.path.clone(), (sig, state.clone()));
                state
            }
        };
        runs.push((run, state));
    }
    let (table, mut note) = read_table(seats_dir);
    let leases = match nirvana_seat::load_leases(&seats_dir.join(nirvana_seat::LEASES_FILE)) {
        Ok(leases) => leases,
        Err(_) => {
            note.get_or_insert_with(|| {
                format!(
                    "{} does not parse: bookings not shown",
                    nirvana_seat::LEASES_FILE
                )
            });
            Leases::default()
        }
    };
    compose(table.as_ref(), note, &runs, &leases, now)
}

/// The view for a project, read now, for the tests; the window uses `cached`.
#[cfg(test)]
pub fn load(root: &Path, seats_dir: &Path) -> View {
    read(root, seats_dir, &mut Replays::new(), now())
}

/// The clock the square's times and live leases read: the test's under test, so a recorded lease stays live.
pub fn now() -> u64 {
    #[cfg(test)]
    if let Some(now) = TEST_NOW.with_borrow(|now| *now) {
        return now;
    }
    nirvana_run::now()
}

#[derive(Default)]
struct Entry {
    view: Option<View>,
    replays: Replays,
    reading: bool,
    read_at: Option<Instant>,
    asked_at: Option<Instant>,
}

static CHANGED: AtomicBool = AtomicBool::new(false);

fn entries() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn start_read(root: &Path) {
    let replays = {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = entries.get_mut(root) else {
            return;
        };
        let fresh = entry.read_at.is_some_and(|at| at.elapsed() < REREAD);
        if entry.reading || fresh {
            return;
        }
        entry.reading = true;
        std::mem::take(&mut entry.replays)
    };
    let folder = root.to_path_buf();
    let spawned = crate::thread_spawn::spawn_named("nirvana-seats", move || {
        let mut replays = replays;
        let view = read(&folder, &seats_dir(), &mut replays, now());
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.entry(folder).or_default();
        entry.reading = false;
        entry.read_at = Some(Instant::now());
        entry.replays = replays;
        // A running order's time moves with no new event, so the window repaints while one runs.
        let moving = view.rows.iter().any(|row| row.state == "running");
        if entry.view.as_ref() != Some(&view) || moving {
            entry.view = Some(view);
            CHANGED.store(true, Ordering::Release);
        }
    });
    if spawned.is_err() {
        if let Some(entry) = entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(root)
        {
            entry.reading = false;
        }
    }
}

/// The project's view as last read, for the window while it draws: nothing here reads a file. The first ask
/// starts a read; later reads are the timer's (`take_changed`). Under test the read is done at once, from the
/// test's home when one is set.
pub fn cached(root: &Path) -> Option<View> {
    #[cfg(test)]
    {
        TEST_HOME
            .with_borrow(Clone::clone)
            .map(|home| load(root, &home))
    }
    #[cfg(not(test))]
    {
        let first = {
            let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
            let entry = entries.entry(root.to_path_buf()).or_default();
            entry.asked_at = Some(Instant::now());
            if let Some(view) = &entry.view {
                return Some(view.clone());
            }
            !entry.reading
        };
        if first {
            start_read(root);
        }
        None
    }
}

/// Starts the reads that are due: every project the window drew in the last `FORGET_AFTER`.
fn poll() {
    let due: Vec<PathBuf> = {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, entry| entry.asked_at.is_some_and(|at| at.elapsed() < FORGET_AFTER));
        entries
            .iter()
            .filter(|(_, entry)| {
                !entry.reading && entry.read_at.is_none_or(|at| at.elapsed() >= REREAD)
            })
            .map(|(root, _)| root.clone())
            .collect()
    };
    for root in due {
        start_read(&root);
    }
}

/// Whether a view changed since the last call: the window repaints. Called on the window's timer tick, which
/// is also where the due reads start, so the draw path itself never reads.
pub fn take_changed() -> bool {
    poll();
    CHANGED.swap(false, Ordering::AcqRel)
}

/// After a mode change: the window's copy shows the new mode at once, not 2 s later, and is read again on the
/// next tick.
fn mode_written(root: &Path, mode: Mode) {
    if let Some(entry) = entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(root)
    {
        entry.read_at = None;
        if let Some(view) = entry.view.as_mut() {
            view.mode = Some(mode);
        }
    }
}

/// Where `leases.json` and `seats.toml` are: herdr's `<home>/nirvana`, or the test's folder under test.
fn seats_dir() -> PathBuf {
    // A test never reaches the owner's own seats.toml, home set or not.
    #[cfg(test)]
    return TEST_HOME
        .with_borrow(Clone::clone)
        .unwrap_or_else(|| std::env::temp_dir().join("nirvana-seats-square-test-no-home"));
    #[cfg(not(test))]
    nirvana_seat::dir()
}

#[cfg(test)]
thread_local! {
    static TEST_HOME: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    static TEST_NOW: std::cell::RefCell<Option<u64>> = const { std::cell::RefCell::new(None) };
}

/// Under test: the folder `cached` reads `leases.json` and `seats.toml` from, and `change_mode` writes, and the
/// clock; `None` draws no Seats data.
#[cfg(test)]
pub fn set_home_for_test(home: Option<PathBuf>, now: Option<u64>) {
    TEST_HOME.with_borrow_mut(|slot| *slot = home);
    TEST_NOW.with_borrow_mut(|slot| *slot = now);
}

// ---- the mode ----

/// Writes `mode_changed` into each open run of the project; the runs written.
pub fn log_mode(root: &Path, from: Mode, to: Mode, by: &str) -> std::io::Result<Vec<String>> {
    let project = Project::at(root);
    let mut logged = Vec::new();
    for run in project.runs() {
        let journal = project.journal(&run);
        if nirvana_run::replay(&journal.read().0).ended.is_some() {
            continue;
        }
        journal.append(
            now(),
            Kind::ModeChanged {
                from,
                to,
                by: by.to_owned(),
            },
        )?;
        logged.push(run);
    }
    Ok(logged)
}

/// Sets the mode in `seats.toml` and logs it in the project's open runs; what to tell the owner.
pub fn change_mode(root: &Path, to: Mode, by: &str) -> Result<String, String> {
    let previous = nirvana_seat::set_mode_in(&seats_dir(), to)?;
    mode_written(root, to);
    if previous == to {
        return Ok(format!("mode {} already", to.title()));
    }
    let runs = log_mode(root, previous, to, by).map_err(|err| format!("journal: {err}"))?;
    Ok(match runs.as_slice() {
        [] => format!("mode {}: the next seat pick here uses it", to.title()),
        runs => format!(
            "mode {}: the next seat pick uses it; logged in run {}",
            to.title(),
            runs.join(", ")
        ),
    })
}

const USAGE: &str = "usage: herdr seat mode [efficient|balanced|sota]
Prints seats.toml's mode, the one `herdr seat pick` uses when --mode is not given, or sets it. A change is
logged as mode_changed in each open run of this project ($NIRVANA_PROJECT, else the git top level).";

/// `herdr seat mode [<mode>]`.
pub fn mode_cli(args: &[String]) -> std::io::Result<()> {
    match args {
        [] => match nirvana_seat::table() {
            Ok(table) => {
                println!("{}", table.mode.word());
                Ok(())
            }
            Err(err) => {
                eprintln!("herdr seat mode: {err}");
                std::process::exit(2);
            }
        },
        [word] => {
            let Some(mode) = Mode::parse(word) else {
                eprintln!("{USAGE}");
                std::process::exit(2);
            };
            let project = Project::from_env();
            match change_mode(&project.root, mode, "herdr seat mode") {
                Ok(message) => {
                    println!("{message}");
                    Ok(())
                }
                Err(err) => {
                    eprintln!("herdr seat mode: {err}");
                    std::process::exit(2);
                }
            }
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests;
