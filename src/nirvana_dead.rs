//! Nirvana fork, dead panes (nirvana/plans/2026-11-08-dead-panes.md): a finished worker never reads as running, and
//! its pane closes.
//!
//! The owner, 2026-11-08: "Notice how all the agents underneath are not clcosed and are falsly running. Plus all the
//! dead tabs at the top can wew fix this". Measured that day, read only: 38 tabs and 123 panes in the `nirvana`
//! session, 116 of them a worker's or a job's pane (`sonnet·quad-xl-slice2`, `job·bs209-build`) whose foreground was a
//! bare `pwsh.exe`, run tabs back to 2026-10-22, and agent rows counting 128 h to 391 h.
//!
//! Two causes. The rows: `nirvana_subagents::read_seat_orders` read an order as live from its journal phase alone, so
//! an Asked order (its supervisor gone, the question open) or a Running one whose supervisor died with a server
//! restart counted time for ever. The live signal is the supervisor's lock (`nirvana_worker::live`), which the OS
//! releases however the process ends; `order_live` reads it. The panes: a worker closed its own pane only after a
//! `done` delivery with another run pane beside it (plan 26 §2), and `close_run_tab` ran only at `herdr run end`, which
//! most runs never reach; a job's pane was never closed. `sweep` closes them by the rules in `decide`.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::nirvana_run::{self, Phase, Project};

/// A Seated order counts as live this long with no supervisor's lock yet: `open_pane` waits up to 10.3 s for the
/// new pane's prompt before it types `herdr seat exec`, which takes the lock. A minute covers that with room.
pub const SEAT_GRACE_S: u64 = 60;

/// A finished worker's or job's pane stays this long after its end before the sweep closes it. Long enough for the
/// owner to see the last screen of a worker that just ended, and well past the seat grace, so a pane a new worker of the
/// run is being started in (its label renamed, its `seat exec` not yet typed) is never taken for a finished one. The
/// run's journal, streams and record outlive the pane, so the wait costs nothing that the owner needs.
pub const CLOSE_GRACE_S: u64 = 120;

/// A pane whose order still wants a worker (Seated, Running, Deferred) but whose supervisor holds no lock stays this
/// long, counted from the later of the order's last journal event and the server's start (review S4). That order did
/// not end: its supervisor died with a crash, a reboot or a server restart, and `herdr run tick` restarts a Deferred
/// one in a new pane (`open_pane` falls back to a new tab). After a restart the owner may want the last screen to see
/// where it stopped, so the pane waits an hour, not the two minutes of an order that ended.
pub const STOPPED_GRACE_S: u64 = 3600;

/// At most this many closes per pass (review S2). The 2026-11-08 backlog was 115 panes; at 25 a minute it drains in
/// five passes, so a wrong rule shows in the log and on screen before it takes everything, and a pass stays short.
pub const MAX_CLOSES: usize = 25;

/// One herdr call inside the sweep gives up after this (review S1). A `pane list` answers in milliseconds.
pub const CALL_LIMIT: Duration = Duration::from_secs(15);

/// The sweep's log under the Nirvana home's `nirvana/` folder: one line per pane it closed or failed to close.
pub const LOG_FILE: &str = "dead-panes.log";

/// Finished rows (nirvana/plans/2026-11-09-finished-rows.md): an ended order, and a run whose orders have all ended,
/// stay in the agents panel and the statusline's order list this long after the order's (the run's newest order's)
/// last journal event, then leave by themselves. The owner, 2026-11-09: "the runs on the bottom are not cleared".
/// Five minutes: the orchestrator is woken with the outcome within the minute and reports it in its own pane, a
/// finished worker's pane closes 2 to 3 minutes after its end (`CLOSE_GRACE_S` plus a pass), and the statusline
/// refreshes every 30 s; so the row outlives its pane by a couple of minutes and is gone before the owner has to clear
/// it. The journals stay: `herdr run status` lists every run's orders, `herdr run receipt <run>` and `herdr seat read
/// <order>` print the rest.
pub const ROWS_GRACE_S: u64 = 300;

/// Whether a row stays shown: it is live, or its last journal event is under `ROWS_GRACE_S` ago. For a run's group in
/// the agents panel, `live` is "any of its orders is live" and `last_at` its newest order's last event; for one order
/// in the statusline, the order's own.
pub fn still_shown(live: bool, last_at: u64, now: u64) -> bool {
    live || now < last_at.saturating_add(ROWS_GRACE_S)
}

/// Finished rows, review round: whether one order's row stays in the agents panel and the statusline. Live, always. Not
/// live, by what the order still waits for:
/// - Deferred (review S1): a rate limit journaled `deferred { until }` and its supervisor returned, so no lock is held,
///   but the order is pending, due at `until` (often hours away, the window's reset). It stays until `until` plus
///   `ROWS_GRACE_S`, or until its phase changes (`herdr run tick` reseats it). Hiding it 5 minutes after the deferral
///   left nothing on screen saying work was queued.
/// - Asked and not `answered` (review N1): the question is open; the row is the owner's only sign of it on screen. It
///   stays until a later order answers it (`OrderState::answers`) or the run ends (an ended run lists only live orders).
/// - Seated or Running with no supervisor (review N2): it died with a crash or a server restart. It stays
///   `STOPPED_GRACE_S` after its last event, as long as the sweep keeps its pane, so the row and the pane leave together.
///   The sweep counts that hour from the later of the last event and the server's start; the rows cannot see the
///   server's start, so after a restart the pane may outlive the row. It is then an ordinary pane again (`worker_pane`
///   no longer holds it), shown as whatever herdr detects in it, not hidden.
/// - Anything else that ended (delivered, interrupted, answered): `ROWS_GRACE_S` after its last event.
pub fn order_shown(order: &nirvana_run::OrderState, live: bool, answered: bool, now: u64) -> bool {
    if live {
        return true;
    }
    match order.phase {
        Phase::Asked if !answered => true,
        Phase::Deferred => {
            let due = order.deferred_until.unwrap_or(0).max(order.last_at);
            now < due.saturating_add(ROWS_GRACE_S)
        }
        Phase::Seated | Phase::Running => now < order.last_at.saturating_add(STOPPED_GRACE_S),
        _ => still_shown(false, order.last_at, now),
    }
}

/// Finished rows, review S2: whether an order's pane is still its worker's, so it is hidden from the space's agent rows
/// and the agents panel and shown only as the order's row. The row's own rule, except that an open question does not
/// hold the pane: the worker that asked has exited, and an owner who opens a Claude in that pane to answer must see it.
pub fn worker_pane(order: &nirvana_run::OrderState, live: bool, now: u64) -> bool {
    order_shown(order, live, true, now)
}

/// Whether a later order of the run answers `order`'s question (`nirvana_run::replay` sets `answers` on the next order
/// seated with no parent while a question is open).
pub fn answered(orders: &[nirvana_run::OrderState], order: &nirvana_run::OrderState) -> bool {
    orders
        .iter()
        .any(|other| other.answers.as_deref() == Some(order.id()))
}

/// Whether an order's worker is live: its phase wants a worker and a supervisor holds its lock, or it was seated
/// under `SEAT_GRACE_S` ago and its supervisor has not started yet. Asked, Delivered and the other ended phases never
/// are: the supervisor exits as it journals them. The one liveness rule for the agents panel, the spaces' worker rows
/// and the statusline's order list.
pub fn order_live(phase: Phase, supervised: bool, seated_at: u64, now: u64) -> bool {
    phase.wants_worker()
        && (supervised || (phase == Phase::Seated && now.saturating_sub(seated_at) < SEAT_GRACE_S))
}

/// The word an order's phase shows as: a phase that wants a worker with none live is "stopped".
pub fn phase_word(phase: Phase, live: bool) -> &'static str {
    if phase.wants_worker() && !live {
        "stopped"
    } else {
        phase.word()
    }
}

/// Whose pane a label names: `<seat>·<order>` (`nirvana_worker::expected_pane_title`) or `job·<name>`
/// (`nirvana_job::open_job_pane`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Owner {
    Worker { seat: String, order: String },
    Job { name: String },
}

/// The owner a pane's label names, `None` for any other label: such a pane is never closed.
pub fn parse_label(label: &str) -> Option<Owner> {
    let (head, tail) = label.split_once('\u{b7}')?;
    if head == "job" {
        return (!tail.trim().is_empty()).then(|| Owner::Job {
            name: tail.to_owned(),
        });
    }
    (nirvana_run::valid_id(head) && nirvana_run::valid_id(tail)).then(|| Owner::Worker {
        seat: head.to_owned(),
        order: tail.to_owned(),
    })
}

/// What the record says of the pane's worker or job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// No journal or job record names it: never closed.
    Unknown,
    /// Its supervisor or its job's process runs.
    Running,
    /// Its run seated an order under `SEAT_GRACE_S` ago: `open_pane` may be about to reuse this pane (review S5).
    Seating,
    /// Its order still wants a worker but no supervisor holds the lock, since this unix second (the order's last
    /// journal event): a crash or a restart mid-run. Waits `STOPPED_GRACE_S`.
    Stopped(u64),
    /// It ended at this unix second (the order's last journal event, the job's end).
    At(u64),
}

/// The facts the close rule reads about one pane, its processes apart (`decide` asks for them last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFacts {
    pub pane: String,
    pub owner: Option<Owner>,
    /// A pane of its tab, itself included, is one herdr sees an agent in (`pane list`'s `agent`): the space's own
    /// agent tab, or a pane a Claude runs in.
    pub tab_has_agent: bool,
    /// The pane is the only pane of its space's only tab: closing it would close the space.
    pub last_of_space: bool,
    /// Its tab is on someone's screen: the server's focused tab or pane, or a client's focused tab (review S2).
    pub watched: bool,
    pub ended: Ended,
    /// The owner's last key in the pane; `Err` when the record could not be read.
    pub owner_key_at: Result<Option<u64>, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Close,
    Keep(String),
}

/// Whether the sweep closes the pane. Every doubt keeps it. `processes` is `pane process-info`'s foreground list,
/// asked only when every other rule allows the close, so a pass costs one herdr call per candidate, not per pane.
/// `server_started` is the server's start in unix seconds (0 when unknown): a stopped order's hour counts from it too.
pub fn decide(
    facts: &PaneFacts,
    now: u64,
    server_started: u64,
    processes: impl FnOnce() -> Option<Vec<Value>>,
) -> Verdict {
    let keep = |why: &str| Verdict::Keep(why.to_owned());
    if facts.owner.is_none() {
        return keep("no worker or job label");
    }
    if facts.tab_has_agent {
        return keep("its tab holds an agent");
    }
    if facts.last_of_space {
        return keep("it is its space's last pane");
    }
    if facts.watched {
        return keep("it is on screen");
    }
    let (ended_at, since, grace, why) = match facts.ended {
        Ended::Unknown => return keep("no record names its worker or job"),
        Ended::Running => return keep("its worker or job still runs"),
        Ended::Seating => return keep("its run is starting a worker"),
        Ended::Stopped(at) => (
            at,
            at.max(server_started),
            STOPPED_GRACE_S,
            "its worker stopped mid-run under the hour's grace ago",
        ),
        Ended::At(at) => (
            at,
            at,
            CLOSE_GRACE_S,
            "its worker or job ended under the grace ago",
        ),
    };
    if now < since.saturating_add(grace) {
        return keep(why);
    }
    match &facts.owner_key_at {
        Err(_) => return keep("the owner's last key in it could not be read"),
        Ok(Some(key)) if *key >= ended_at => {
            return keep("the owner typed in it after its worker or job ended")
        }
        Ok(_) => {}
    }
    match processes() {
        None => keep("its processes could not be read"),
        Some(list) if list.is_empty() => keep("herdr named no process in it"),
        Some(list) if !crate::nirvana_worker::only_shells(&list) => {
            keep("a program other than a shell runs in it")
        }
        Some(_) => Verdict::Close,
    }
}

/// The projects a worker's pane may belong to: the one its supervisor noted (`nirvana_worker::pane_project`), then
/// the first folder at or above the pane's cwd that holds `.nirvana/runs` (a worktree's cwd climbs to its project).
fn candidate_projects(home: &Path, pane: &str, cwd: Option<&str>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = crate::nirvana_worker::pane_project(home, pane)
        .into_iter()
        .collect();
    let mut dir = cwd.filter(|c| !c.is_empty()).map(PathBuf::from);
    while let Some(at) = dir {
        if at.join(nirvana_run::DIR).join("runs").is_dir() {
            if !roots.contains(&at) {
                roots.push(at);
            }
            break;
        }
        dir = at.parent().map(Path::to_path_buf);
    }
    roots
}

/// What the journals or the job records say of the pane's owner at `now`.
pub fn ended_of(home: &Path, pane: &str, cwd: Option<&str>, owner: &Owner, now: u64) -> Ended {
    match owner {
        Owner::Worker { seat, order } => {
            for root in candidate_projects(home, pane, cwd) {
                let project = Project::at(root);
                let Some(run) = project.run_of(order) else {
                    continue;
                };
                let state = nirvana_run::replay(&project.journal(&run).read().0);
                let Some(known) = state.order(order) else {
                    continue;
                };
                if known.seated.seat != *seat {
                    continue;
                }
                if crate::nirvana_worker::live(&project, &run, order) {
                    return Ended::Running;
                }
                // Review S5: `open_pane` may reuse a finished pane of the run's tab for an order it seats; while one
                // was seated under the seat grace ago, no pane of the run closes. The race left: a sweep that read
                // the journal just before the `seated` line, then closed the pane as `open_pane` picked it (a
                // window of one pass's `process-info`, `pane get` and `pane close`, well under a second).
                if state.orders.iter().any(|other| {
                    other.phase == Phase::Seated
                        && now.saturating_sub(other.seated_at) < SEAT_GRACE_S
                }) {
                    return Ended::Seating;
                }
                if known.phase.wants_worker() {
                    return Ended::Stopped(known.last_at);
                }
                return Ended::At(known.last_at);
            }
            Ended::Unknown
        }
        Owner::Job { name } => {
            let Some(record) = crate::nirvana_job::list_jobs(home)
                .into_iter()
                .find(|record| record.pane.as_deref() == Some(pane))
            else {
                return Ended::Unknown;
            };
            if record.name != *name {
                return Ended::Unknown;
            }
            match crate::nirvana_job::job_state(&record) {
                crate::nirvana_job::JobState::Running => Ended::Running,
                _ => Ended::At(record.ended.unwrap_or(record.started)),
            }
        }
    }
}

fn str_at<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

fn array_at(value: &Value, pointers: &[&str]) -> Vec<Value> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_array))
        .cloned()
        .unwrap_or_default()
}

/// A herdr answer: its JSON, or its error code (`pane_not_found`) and message (`nirvana_worker::herdr_json_coded`).
pub type Answer = Result<Value, (Option<String>, String)>;

/// What one pass knows beyond the panes: when, whether it only reads, the server's start, and the tabs the server's
/// clients have focused (the server passes them, `app::nirvana_dead_panes`; `pane list` names only its own focus).
#[derive(Debug, Clone, Default)]
pub struct Pass {
    pub now: u64,
    pub dry_run: bool,
    pub server_started: u64,
    pub watched_tabs: HashSet<String>,
}

impl Pass {
    pub fn at(now: u64) -> Self {
        Self {
            now,
            ..Self::default()
        }
    }
}

fn is_true(value: &Value, pointer: &str) -> bool {
    value.pointer(pointer).and_then(Value::as_bool) == Some(true)
}

fn gone(code: &Option<String>) -> bool {
    code.as_deref() == Some("pane_not_found")
}

/// A pane the pass closed or found gone leaves its tab one pane emptier, and its space one tab emptier with the last.
fn forget(
    panes_in_tab: &mut HashMap<String, usize>,
    tabs_in_space: &mut HashMap<String, usize>,
    tab: &str,
    space: &str,
) {
    let left = panes_in_tab.entry(tab.to_owned()).or_default();
    *left = left.saturating_sub(1);
    if *left == 0 {
        let tabs_left = tabs_in_space.entry(space.to_owned()).or_default();
        *tabs_left = tabs_left.saturating_sub(1);
    }
}

/// One pass over every pane herdr lists: closes each one `decide` allows, at most `MAX_CLOSES`, and returns a line
/// per pane it closed, found gone or failed to close (with `dry_run`, a line per labelled pane and nothing closed).
/// `herdr` is this build's CLI against the server (`nirvana_worker::herdr_json_within`); a test passes a fake.
pub fn sweep(
    home: &Path,
    herdr: &dyn Fn(&[&str]) -> Answer,
    pass: &Pass,
) -> Result<Vec<String>, String> {
    let now = pass.now;
    let list = |args: &[&str], pointers: &[&str]| {
        herdr(args)
            .map(|value| array_at(&value, pointers))
            .map_err(|(_, why)| why)
    };
    let panes = list(&["pane", "list"], &["/result/panes", "/panes"])?;
    let tabs = list(&["tab", "list"], &["/result/tabs", "/tabs"])?;
    let tab_of = |pane: &Value| str_at(pane, &["/tab_id"]).unwrap_or("").to_owned();
    // Live counts, so a pass that closes several panes never takes a space's last one.
    let mut panes_in_tab: HashMap<String, usize> = HashMap::new();
    for pane in &panes {
        *panes_in_tab.entry(tab_of(pane)).or_default() += 1;
    }
    let mut tabs_in_space: HashMap<String, usize> = HashMap::new();
    let mut space_of_tab: HashMap<String, String> = HashMap::new();
    for tab in &tabs {
        let (Some(tab_id), Some(space)) =
            (str_at(tab, &["/tab_id"]), str_at(tab, &["/workspace_id"]))
        else {
            continue;
        };
        *tabs_in_space.entry(space.to_owned()).or_default() += 1;
        space_of_tab.insert(tab_id.to_owned(), space.to_owned());
    }
    let agent_tabs: HashSet<String> = panes
        .iter()
        .filter(|pane| str_at(pane, &["/agent"]).is_some())
        .map(tab_of)
        .collect();
    // Review S2: a tab on screen keeps every pane in it. Scroll and mouse never reach `note_owner_key`, so a pane the
    // owner is only reading would close under them; the server's focus is in `pane list` and `tab list`, the clients'
    // own focus comes in `pass.watched_tabs`.
    let mut watched = pass.watched_tabs.clone();
    for tab in &tabs {
        if is_true(tab, "/focused") {
            watched.extend(str_at(tab, &["/tab_id"]).map(str::to_owned));
        }
    }
    for pane in &panes {
        if is_true(pane, "/focused") {
            watched.insert(tab_of(pane));
        }
    }

    let mut lines = Vec::new();
    let mut tries = 0;
    for pane in &panes {
        let Some(id) = str_at(pane, &["/pane_id"]) else {
            continue;
        };
        let label = str_at(pane, &["/label"]).unwrap_or("");
        let Some(owner) = parse_label(label) else {
            continue;
        };
        if !pass.dry_run && tries >= MAX_CLOSES {
            lines.push(format!(
                "stopped at {MAX_CLOSES} closes; the rest wait for the next pass"
            ));
            break;
        }
        let tab = tab_of(pane);
        let space = space_of_tab.get(&tab).cloned().unwrap_or_default();
        let facts = PaneFacts {
            pane: id.to_owned(),
            tab_has_agent: agent_tabs.contains(&tab),
            last_of_space: panes_in_tab.get(&tab).copied().unwrap_or(0) <= 1
                && tabs_in_space.get(&space).copied().unwrap_or(0) <= 1,
            watched: watched.contains(&tab),
            ended: ended_of(home, id, str_at(pane, &["/cwd"]), &owner, now),
            owner_key_at: nirvana_run::owner_key_in(home, id),
            owner: Some(owner),
        };
        let read_processes = || {
            herdr(&["pane", "process-info", "--pane", id])
                .ok()
                .map(|info| {
                    array_at(
                        &info,
                        &[
                            "/result/process_info/foreground_processes",
                            "/process_info/foreground_processes",
                        ],
                    )
                })
        };
        let verdict = decide(&facts, now, pass.server_started, read_processes);
        if pass.dry_run {
            lines.push(match &verdict {
                Verdict::Close => format!("would close {id} ({label})"),
                Verdict::Keep(why) => format!("keeps {id} ({label}): {why}"),
            });
            continue;
        }
        if verdict != Verdict::Close {
            continue;
        }
        tries += 1;
        // The label is read again just before the close: a run that reuses the pane renames it first
        // (`nirvana_worker::open_pane`), so a pane taken since the list is left alone.
        let label_now = match herdr(&["pane", "get", id]) {
            Ok(info) => str_at(&info, &["/result/pane/label", "/pane/label"]).map(str::to_owned),
            Err((code, _)) if gone(&code) => {
                lines.push(format!("{id} ({label}) was already gone"));
                forget(&mut panes_in_tab, &mut tabs_in_space, &tab, &space);
                continue;
            }
            Err(_) => None,
        };
        if label_now.as_deref() != Some(label) {
            lines.push(format!(
                "kept {id} ({label}): its label changed since the list"
            ));
            continue;
        }
        match herdr(&["pane", "close", id]) {
            Ok(_) => {
                lines.push(format!("closed {id} ({label})"));
                forget(&mut panes_in_tab, &mut tabs_in_space, &tab, &space);
            }
            // Review S5: a pane gone between the `pane get` and the close (its tab closed, a reuse failed) is one
            // the pass no longer counts, not a failure.
            Err((code, _)) if gone(&code) => {
                lines.push(format!("{id} ({label}) was already gone"));
                forget(&mut panes_in_tab, &mut tabs_in_space, &tab, &space);
            }
            Err((_, why)) => lines.push(format!("failed to close {id} ({label}): {why}")),
        }
    }
    // Plan 40 review D1: a phone upload cut by a killed PC is left as `<name>.partial` for ever (`send::save` removes
    // it only on a handled error). Each pane's folder is a space the phone may have sent into.
    let mut folders: Vec<PathBuf> = panes
        .iter()
        .filter_map(|pane| str_at(pane, &["/cwd"]))
        .map(PathBuf::from)
        .collect();
    folders.sort();
    folders.dedup();
    lines.extend(sweep_partials(&folders, now, pass.dry_run));
    Ok(lines)
}

/// An unfinished upload this old is a dead one: an upload is cut at `http::BODY_MOST` (20 min).
pub const PARTIAL_AGE_S: u64 = 3600;

/// Removes `<folder>/.nirvana/uploads/<upload name>.partial` files older than `PARTIAL_AGE_S` (by their last write,
/// against `now` in unix seconds), a line each. Only a name `send::name_for` gives, only a plain file in a plain
/// folder: a finished upload is the owner's file and is never touched, nor anything a link points at.
pub fn sweep_partials(folders: &[PathBuf], now: u64, dry_run: bool) -> Vec<String> {
    use crate::nirvana_remote::send;
    let mut lines = Vec::new();
    for folder in folders {
        let dot = folder.join(nirvana_run::DIR);
        let uploads = dot.join("uploads");
        // Review N6: `.nirvana` itself may be a junction, which `uploads_dir` refuses; so does the sweep.
        if !send::plain_dir(&dot) || !send::plain_dir(&uploads) {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&uploads) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(upload) = name.to_str().and_then(|name| name.strip_suffix(".partial")) else {
                continue;
            };
            if !send::is_upload_name(upload) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if !meta.file_type().is_file() {
                continue;
            }
            let written = meta
                .modified()
                .ok()
                .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(now, |since| since.as_secs());
            if now.saturating_sub(written) < PARTIAL_AGE_S {
                continue;
            }
            let path = entry.path();
            if dry_run {
                lines.push(format!(
                    "would remove {}: an upload never finished",
                    path.display()
                ));
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => lines.push(format!(
                    "removed {}: an upload never finished",
                    path.display()
                )),
                Err(err) => lines.push(format!("failed to remove {}: {err}", path.display())),
            }
        }
    }
    lines
}

/// Waits for `child` up to `limit`; past it, kills it and answers `None` (review S1).
pub fn wait_within(child: &mut Child, limit: Duration) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + limit;
    let mut nap = Duration::from_millis(5);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(nap.min(left));
        nap = (nap * 2).min(Duration::from_millis(100));
    }
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> Option<std::thread::JoinHandle<Vec<u8>>> {
    pipe.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.read_to_end(&mut bytes);
            bytes
        })
    })
}

/// `command`'s output, or an error once it runs past `limit` (killed). The pipes drain on threads so a full pipe never
/// stalls the child; on a timeout they are left to end with it, not joined.
pub fn output_within(mut command: Command, limit: Duration) -> Result<Output, String> {
    let mut child = command.spawn().map_err(|err| err.to_string())?;
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let status = wait_within(&mut child, limit).map_err(|err| err.to_string())?;
    let Some(status) = status else {
        return Err(format!("no answer in {} s; killed", limit.as_secs_f32()));
    };
    let joined = |handle: Option<std::thread::JoinHandle<Vec<u8>>>| {
        handle
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default()
    };
    Ok(Output {
        status,
        stdout: joined(stdout),
        stderr: joined(stderr),
    })
}

/// Appends `lines` to the sweep's log, each with the time.
pub fn log_lines(home: &Path, lines: &[String]) {
    let dir = home.join(crate::nirvana_seat::DIR);
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut log) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(LOG_FILE))
    {
        let at = nirvana_run::utc(nirvana_run::now());
        for line in lines {
            let _ = writeln!(log, "{at} {line}");
        }
    }
}

/// Whether a server of this session runs the sweep timer (review S6): only the Nirvana session's, as Nirvana Remote
/// since 0.58.1 (`nirvana_remote::skip_reason`). Pane ids (`wP:p2`) are per session but the key and pane-index files
/// under the shared home are keyed by id alone, so a second session's sweep could read another session's records.
pub fn sweeps_in(session: Option<&str>) -> bool {
    session == Some(crate::nirvana_launch::SESSION)
}

/// Which server a hand-run sweep asks (finished rows, item 5). Inside herdr, its own (`None`). From a plain shell, the
/// 0.59.0 live check's `herdr run sweep --dry-run` printed "not in herdr: no server to sweep": now a dry run asks the
/// session `--session` named, else the Nirvana session, the only one whose server sweeps (`sweeps_in`). A closing pass
/// from a plain shell is refused: it would not know which tabs the window's clients have on screen (`--watched`, review
/// S2), and the server runs one every minute anyway.
///
/// Inside herdr the process's own server wins over `--session` (review N6): `herdr --session nirvana run sweep
/// --dry-run` typed in a pane of another session sweeps that other session. Harmless: a dry run only reads (`pane list`,
/// `tab list`, `pane process-info`), and the closing pass is the server's own.
pub fn sweep_target(
    in_herdr: bool,
    named: Option<&str>,
    dry_run: bool,
) -> Result<Option<String>, String> {
    if in_herdr {
        return Ok(None);
    }
    if !dry_run {
        return Err(
            "not in herdr: only `herdr run sweep --dry-run` runs from a plain shell; the Nirvana server sweeps every minute"
                .to_owned(),
        );
    }
    Ok(Some(
        named.unwrap_or(crate::nirvana_launch::SESSION).to_owned(),
    ))
}

/// `herdr run sweep [--dry-run] [--server-started <unix s>] [--watched <tab>]...`: one pass against the server this
/// process belongs to, or, `--dry-run` from a plain shell, against the Nirvana session's (`sweep_target`). The server
/// starts it every `PASS_EVERY` (`app::nirvana_dead_panes`) with its start and its clients' focused tabs; the owner
/// runs `--dry-run` to see each labelled pane's verdict. Run by hand without `--server-started`, the pass takes the
/// server as started now, so a stopped order's pane reads kept.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let now = nirvana_run::now();
    let mut pass = Pass {
        server_started: now,
        ..Pass::at(now)
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--dry-run" => pass.dry_run = true,
            "--server-started" => {
                if let Some(at) = rest.next().and_then(|at| at.parse().ok()) {
                    pass.server_started = at;
                }
            }
            "--watched" => pass.watched_tabs.extend(rest.next().cloned()),
            _ => {}
        }
    }
    match sweep_target(
        crate::nirvana_worker::in_herdr(),
        crate::session::active_name().as_deref(),
        pass.dry_run,
    ) {
        Ok(None) => {}
        // The herdr calls below are this binary run as a child (`nirvana_worker::herdr_command`), which reads
        // `HERDR_SESSION` for its socket when no socket path is set, as `herdr --session <name>` itself does.
        Ok(Some(session)) => std::env::set_var(crate::session::SESSION_ENV_VAR, session),
        Err(why) => {
            println!("{why}");
            return Ok(());
        }
    }
    let Some(home) = crate::nirvana::home() else {
        println!("no Nirvana home");
        return Ok(());
    };
    let herdr = |args: &[&str]| crate::nirvana_worker::herdr_json_within(args, CALL_LIMIT);
    let lines = match sweep(&home, &herdr, &pass) {
        Ok(lines) => lines,
        Err(why) => {
            eprintln!("herdr run sweep: {why}");
            if !pass.dry_run {
                log_lines(&home, &[format!("the sweep could not list panes: {why}")]);
            }
            std::process::exit(1);
        }
    };
    for line in &lines {
        println!("{line}");
    }
    if !pass.dry_run && !lines.is_empty() {
        log_lines(&home, &lines);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
