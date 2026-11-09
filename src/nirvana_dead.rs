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
use std::io::Write;
use std::path::{Path, PathBuf};

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

/// The sweep's log under the Nirvana home's `nirvana/` folder: one line per pane it closed or failed to close.
pub const LOG_FILE: &str = "dead-panes.log";

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
pub fn decide(
    facts: &PaneFacts,
    now: u64,
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
    let ended_at = match facts.ended {
        Ended::Unknown => return keep("no record names its worker or job"),
        Ended::Running => return keep("its worker or job still runs"),
        Ended::At(at) => at,
    };
    if now < ended_at.saturating_add(CLOSE_GRACE_S) {
        return keep("its worker or job ended under the grace ago");
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

/// What the journals or the job records say of the pane's owner.
pub fn ended_of(home: &Path, pane: &str, cwd: Option<&str>, owner: &Owner) -> Ended {
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

/// One pass over every pane herdr lists: closes each one `decide` allows, and returns a line per pane it closed or
/// failed to close (with `dry_run`, a line per labelled pane and nothing closed). `herdr` is this build's CLI against
/// the server (`nirvana_worker::herdr_json`); a test passes a fake.
pub fn sweep(
    home: &Path,
    herdr: &dyn Fn(&[&str]) -> Result<Value, String>,
    now: u64,
    dry_run: bool,
) -> Result<Vec<String>, String> {
    let panes = array_at(&herdr(&["pane", "list"])?, &["/result/panes", "/panes"]);
    let tabs = array_at(&herdr(&["tab", "list"])?, &["/result/tabs", "/tabs"]);
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

    let mut lines = Vec::new();
    for pane in &panes {
        let Some(id) = str_at(pane, &["/pane_id"]) else {
            continue;
        };
        let label = str_at(pane, &["/label"]).unwrap_or("");
        let Some(owner) = parse_label(label) else {
            continue;
        };
        let tab = tab_of(pane);
        let space = space_of_tab.get(&tab).cloned().unwrap_or_default();
        let facts = PaneFacts {
            pane: id.to_owned(),
            tab_has_agent: agent_tabs.contains(&tab),
            last_of_space: panes_in_tab.get(&tab).copied().unwrap_or(0) <= 1
                && tabs_in_space.get(&space).copied().unwrap_or(0) <= 1,
            ended: ended_of(home, id, str_at(pane, &["/cwd"]), &owner),
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
        let verdict = decide(&facts, now, read_processes);
        if dry_run {
            lines.push(match &verdict {
                Verdict::Close => format!("would close {id} ({label})"),
                Verdict::Keep(why) => format!("keeps {id} ({label}): {why}"),
            });
            continue;
        }
        if verdict != Verdict::Close {
            continue;
        }
        // The label is read again just before the close: a run that reuses the pane renames it first
        // (`nirvana_worker::open_pane`), so a pane taken since the list is left alone.
        let label_now = herdr(&["pane", "get", id]).ok().and_then(|info| {
            str_at(&info, &["/result/pane/label", "/pane/label"]).map(str::to_owned)
        });
        if label_now.as_deref() != Some(label) {
            lines.push(format!(
                "kept {id} ({label}): its label changed since the list"
            ));
            continue;
        }
        match herdr(&["pane", "close", id]) {
            Ok(_) => {
                lines.push(format!("closed {id} ({label})"));
                let left = panes_in_tab.entry(tab.clone()).or_default();
                *left = left.saturating_sub(1);
                if *left == 0 {
                    let tabs_left = tabs_in_space.entry(space).or_default();
                    *tabs_left = tabs_left.saturating_sub(1);
                }
            }
            Err(why) => lines.push(format!("failed to close {id} ({label}): {why}")),
        }
    }
    Ok(lines)
}

/// `herdr run sweep [--dry-run]`: one pass against the server this process belongs to. The server starts it every
/// `PASS_EVERY` (`app::nirvana_dead_panes`); the owner runs `--dry-run` to see each labelled pane's verdict.
pub fn cli(args: &[String]) -> std::io::Result<()> {
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    if !crate::nirvana_worker::in_herdr() {
        println!("not in herdr: no server to sweep");
        return Ok(());
    }
    let Some(home) = crate::nirvana::home() else {
        println!("no Nirvana home");
        return Ok(());
    };
    let lines = match sweep(
        &home,
        &crate::nirvana_worker::herdr_json,
        nirvana_run::now(),
        dry_run,
    ) {
        Ok(lines) => lines,
        Err(why) => {
            eprintln!("herdr run sweep: {why}");
            std::process::exit(1);
        }
    };
    for line in &lines {
        println!("{line}");
    }
    if !dry_run && !lines.is_empty() {
        let dir = home.join(crate::nirvana_seat::DIR);
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(LOG_FILE))
        {
            let at = nirvana_run::utc(nirvana_run::now());
            for line in &lines {
                let _ = writeln!(log, "{at} {line}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
