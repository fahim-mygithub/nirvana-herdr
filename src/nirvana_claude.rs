//! Nirvana fork, plan 12 O4b: herdr's half of the Nirvana Claude plugin (`nirvana/claude/`), the commands its hooks
//! and its statusline run. (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §6, §7.2, §7.4)
//!
//! The owner, 2026-10-17 (plan 12 L6): "The reason we are building this as a seperate version which tbh is proably
//! just Claude Code with these Orchistrator level skills and memories and hooks is to differentiate between the two."
//!
//! - `herdr nirvana digest`, the `SessionStart` hook: the pools the foot draws, a line for the hidden pools when they
//!   have headroom, the board, the open runs with their next step, the open retro proposals and the memory file, in
//!   about 300 tokens. It reads caches only: a usage reader takes 4 to 20 s and a hook delays the session's start.
//! - `herdr nirvana guard-spawn`, the `PreToolUse` hook on `Agent`: refuses a spawn that names no model or no effort
//!   (it would run on the orchestrator's own seat, CLAUDE.md rule 5), and one on `max` that would break the
//!   orchestrator's reserve, from the seat engine's own numbers in `seats.toml`.
//! - `herdr nirvana subagent-stop`, the `SubagentStop` hook: a `worker-*` subagent's outcome goes to the seat record
//!   through `record_once`, which `herdr seat record` uses too.
//! - `herdr nirvana stop`, the `Stop` hook: an open run at a dead end (O3's `check`) is raised once per state, never
//!   blocking forever (§7.2 "No dead end").
//! - `herdr nirvana statusline`: each drawn pool's tightest window, its reset when the width allows, and the open
//!   orders.
//!
//! Every hook stays quiet and fast when it has nothing to say, and never fails the turn: a problem prints nothing.

use std::collections::BTreeMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::nirvana_run::{self, Phase, Project, RunState};
use crate::nirvana_seat::{self, Effort, Leases, Table, Tokens};
use crate::nirvana_usage::{self, Figure, Freshness, Provider, Reading, Source};

/// The digest's ceiling. The brief: "Keep it under 300 tokens". `/context` counted digests on 2026-10-18 at 1.8 to
/// 2 characters a token (753 characters, 380 tokens; 567, 310), paths being dear; plain prose 2.9. So 540 characters,
/// and the trimmed digest's live count is in the O4b record.
pub const DIGEST_MAX_CHARS: usize = 540;
/// Runs and proposals the digest names before "and n more".
const DIGEST_RUNS: usize = 2;
const DIGEST_PROPOSALS: usize = 2;
/// A dead end's or a step's text in the digest, and a proposal's gap, at most.
const STEP_CHARS: usize = 44;
const GAP_CHARS: usize = 24;
/// `<home>/nirvana/NIRVANA.md`: one line per project, its lessons (O5) and its board (here).
pub const MEMORY_FILE: &str = "NIRVANA.md";
/// The marker of the project's board line, so a write replaces only that line, as O5's lessons line does.
const BOARD_MARK: &str = "<!-- herdr-board ";
/// A new `NIRVANA.md`'s head, whichever of the two writers (this board line, O5's lessons line) comes first.
pub(crate) const MEMORY_HEAD: &str = "# Nirvana memory\n\nOne line per project: its lessons, written by herdr once the gate checked them, and its \
shared board. herdr rewrites only its own marked lines; anything else here is yours.\n\n";
/// What the statusline last read from Claude Code: the `max` pool's live windows, fresher than `usage.json`, which
/// `guard-spawn` prefers when it is younger.
pub const STATUSLINE_FILE: &str = "statusline.json";
/// Where `herdr nirvana stop` remembers the dead ends it raised: one raise per run state.
const STOP_FILE: &str = "stop-raised.json";
/// A subagent spawned through the guard is a short order (plan 12 §7.1): `seats.toml`'s small size.
const SUBAGENT_SIZE: nirvana_seat::SizeName = nirvana_seat::SizeName::Small;
/// The record's chain for a subagent's line. Its outcome is not the gate's verdict, so it is kept apart from every
/// class chain, where O2's tally would count it as a failure.
pub const SUBAGENT_CHAIN: &str = "subagent";
/// How long `subagent-stop` waits for a lagging transcript to show the worker's responses; the hook's own timeout
/// in hooks.json is 15 s.
const TRANSCRIPT_WAIT: std::time::Duration = std::time::Duration::from_secs(3);
/// The plugin's worker agents are `worker-<seat>-<effort>`.
pub const WORKER_PREFIX: &str = "worker-";
/// The plugin's name, Claude Code's prefix for its agents.
pub const PLUGIN: &str = "nirvana";
/// Scores, shares: the seat engine's slack (nirvana_seat's EDGE).
const EDGE: f32 = 1e-4;

fn now() -> u64 {
    nirvana_usage::unix(std::time::SystemTime::now())
}

fn read_stdin() -> Value {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    serde_json::from_str(&input).unwrap_or(Value::Null)
}

fn home_dir() -> PathBuf {
    crate::nirvana::home().unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
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

/// "14.8%": a share beside the reserve, where "15%" would read as at the reserve when it is under it.
fn pct1(share: f32) -> String {
    format!("{:.1}%", share * 100.0)
}

/// The first line of a text, at most `max` characters.
fn short(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        let cut: String = line.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

// ---- the readings, from the caches only ----

/// Each reader's last answer from `usage.json`, never a new read.
pub fn cached_sources() -> [Source; 4] {
    let cache = nirvana_usage::cache_path()
        .map(|path| nirvana_usage::read_cache(&path))
        .unwrap_or_default();
    let mut sources: [Source; 4] = Default::default();
    // `Provider::ALL` is in the array's order (nirvana_usage's `index`).
    for (at, provider) in Provider::ALL.iter().enumerate() {
        if let Some(source) = cache.sources.get(provider.key()) {
            sources[at] = source.clone();
        }
    }
    sources
}

/// The seat table, or `None` when the owner's edit does not parse: a hook never exits over it.
fn table() -> Option<Table> {
    let path = nirvana_seat::dir().join(nirvana_seat::SEATS_FILE);
    nirvana_seat::parse(&nirvana_seat::read_or_create(&path)).ok()
}

/// The `max` pool's windows as Claude Code's statusline gave them, `rate_limits.five_hour` and `seven_day`
/// (used 0 to 100, resets in Unix seconds). Claude Code reads them from each response, so they are the freshest
/// figure of the orchestrator's own pool (plan 12 P6).
pub fn statusline_readings(input: &Value, read_at: u64) -> Vec<Reading> {
    let mut readings = Vec::new();
    for (key, window) in [("five_hour", "5h"), ("seven_day", "week")] {
        let Some(limit) = input.pointer(&format!("/rate_limits/{key}")) else {
            continue;
        };
        let Some(used) = limit.get("used_percentage").and_then(Value::as_f64) else {
            continue;
        };
        readings.push(Reading {
            pool: nirvana_usage::MAX.to_owned(),
            window: window.to_owned(),
            used: Some((used / 100.0).clamp(0.0, 1.0) as f32),
            resets_at: limit.get("resets_at").and_then(Value::as_u64),
            resets_text: None,
            read_at,
            source: "claude statusline".to_owned(),
        });
    }
    readings
}

/// The statusline's readings put over `max`'s cached ones, window by window; `week.fable`, which the statusline
/// does not carry, stays as cached.
pub fn with_live_max(mut sources: [Source; 4], live: &[Reading]) -> [Source; 4] {
    if live.is_empty() {
        return sources;
    }
    let claude = &mut sources[0];
    for reading in live {
        claude
            .readings
            .retain(|known| !(known.pool == reading.pool && known.window == reading.window));
        claude.readings.push(reading.clone());
    }
    claude
        .readings
        .sort_by_key(|reading| match reading.window.as_str() {
            "5h" => 0,
            "week" => 1,
            _ => 2,
        });
    let at = live.iter().map(|reading| reading.read_at).max();
    claude.read_at = claude.read_at.max(at);
    claude.word = None;
    sources
}

fn read_statusline_file(home: &Path) -> Vec<Reading> {
    std::fs::read_to_string(home.join(nirvana_seat::DIR).join(STATUSLINE_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

// ---- the digest (SessionStart) ----

/// What the digest is made of, gathered by `digest_cli` and given whole to `digest_text` in the tests.
#[derive(Debug, Clone, Default)]
pub struct Digest {
    pub now: u64,
    pub exe: String,
    pub sources: [Source; 4],
    /// The seat table's floor: a hidden pool has headroom while each window is known and above it.
    pub floor: f32,
    /// `None`: no git root, so no board. `Some((revision, section names))`.
    pub board: Option<(u64, Vec<String>)>,
    /// Each open run and its next step (`Ok`) or its dead end (`Err`), from O3's `check`.
    pub runs: Vec<(String, Result<String, String>)>,
    /// Each open proposal's id and gap.
    pub proposals: Vec<(String, String)>,
    pub memory: String,
}

/// "Claude 5h 41% (2h10m), week 37% (5d4h)": a pool's windows every seat spends, each with its reset, and the age
/// of a reading that is not fresh.
fn pool_phrase(pool: &str, readings: &[&Reading], now: u64) -> String {
    let mut parts = Vec::new();
    let mut oldest: Option<u64> = None;
    for reading in readings.iter().filter(|r| !r.window.contains('.')) {
        let fresh = nirvana_usage::freshness(reading, now);
        let used = reading.used.map_or_else(|| "not read".to_owned(), pct);
        let reset = match (fresh, reading.resets_at) {
            (Freshness::ResetPassed, _) => " (reset since)".to_owned(),
            (_, Some(at)) => format!(
                " ({})",
                nirvana_usage::short_duration(at.saturating_sub(now))
            ),
            _ => String::new(),
        };
        if fresh != Freshness::Fresh {
            oldest = Some(oldest.map_or(reading.read_at, |at| at.min(reading.read_at)));
        }
        parts.push(format!("{} {used}{reset}", reading.window));
    }
    let mut phrase = format!("{} {}", nirvana_usage::label(pool), parts.join(", "));
    if let Some(at) = oldest {
        phrase.push_str(&format!(
            ", read {} ago",
            nirvana_usage::short_duration(now.saturating_sub(at))
        ));
    }
    phrase
}

/// The pools in their order (`POOLS`, then any other a reader gave), each with its readings.
fn pools(sources: &[Source; 4]) -> Vec<(String, &Source, Vec<&Reading>)> {
    let mut names: Vec<String> = nirvana_usage::POOLS
        .iter()
        .map(|p| (*p).to_owned())
        .collect();
    for source in sources {
        for reading in &source.readings {
            if !names.contains(&reading.pool) {
                names.push(reading.pool.clone());
            }
        }
    }
    names
        .into_iter()
        .map(|pool| {
            let at = Provider::ALL
                .iter()
                .position(|p| *p == nirvana_usage::provider_of(&pool))
                .unwrap_or(0);
            let source = &sources[at];
            let readings = source.readings.iter().filter(|r| r.pool == pool).collect();
            (pool, source, readings)
        })
        .collect()
}

/// A hidden pool has headroom when every window every seat spends is read, its reset has not passed since, and
/// each is above the seat engine's floor.
fn has_headroom(readings: &[&Reading], floor: f32, now: u64) -> bool {
    let windows: Vec<&&Reading> = readings
        .iter()
        .filter(|r| !r.window.contains('.'))
        .collect();
    !windows.is_empty()
        && windows.iter().all(|reading| {
            nirvana_usage::freshness(reading, now) != Freshness::ResetPassed
                && reading.used.is_some_and(|used| 1.0 - used > floor + EDGE)
        })
}

/// A line ends in a full stop, unless its last part was cut short.
fn sentence(text: String) -> String {
    if text.ends_with('…') || text.ends_with('.') {
        text
    } else {
        format!("{text}.")
    }
}

pub fn digest_text(digest: &Digest) -> String {
    let now = digest.now;
    let mut drawn = Vec::new();
    let mut hidden = Vec::new();
    for (pool, source, readings) in pools(&digest.sources) {
        if nirvana_usage::FOOT_HIDDEN.contains(&pool.as_str()) {
            if source.word.is_none() && has_headroom(&readings, digest.floor, now) {
                let tight: Vec<Reading> = readings.iter().map(|r| (*r).clone()).collect();
                if let Some(reading) = nirvana_usage::tightest(&tight) {
                    hidden.push(format!(
                        "{} {} {}",
                        nirvana_usage::label(&pool),
                        reading.window,
                        reading.used.map_or_else(String::new, pct)
                    ));
                }
            }
            continue;
        }
        // A pool in a word (not installed, not exposed, off) has nothing to seat or decide: the foot shows it.
        if source.word.is_none()
            && source.read_at.is_some()
            && readings.iter().any(|r| r.window != "vram")
        {
            drawn.push(pool_phrase(&pool, &readings, now));
        }
    }
    // Each line: its place in the digest, and its full and short forms. The first three always print; the rest go
    // in by priority while they fit `DIGEST_MAX_CHARS`, the short form when the full one does not.
    let mut lines: Vec<(u8, String, Option<String>)> = vec![
        (
            0,
            format!(
                "Nirvana {}; herdr is \"{}\".",
                crate::nirvana::VERSION,
                digest.exe
            ),
            None,
        ),
        (
            1,
            if drawn.is_empty() {
                "Pools: none read yet; `herdr usage` reads them.".to_owned()
            } else {
                sentence(format!("Pools: {}", drawn.join("; ")))
            },
            None,
        ),
        (9, format!("Memory: {}.", digest.memory), None),
    ];
    let mut optional: Vec<(u8, String, Option<String>)> = Vec::new();
    if !digest.runs.is_empty() {
        let dead = digest.runs.iter().filter(|(_, step)| step.is_err()).count();
        // Dead ends first: they are what the orchestrator must act on.
        let mut runs: Vec<&(String, Result<String, String>)> = digest.runs.iter().collect();
        runs.sort_by_key(|(_, step)| step.is_ok());
        let mut named: Vec<String> = runs
            .iter()
            .take(DIGEST_RUNS)
            .map(|(run, step)| match step {
                Ok(step) => format!("{run}: {}", short(step, STEP_CHARS)),
                Err(why) => format!("{run}: DEAD END, {}", short(why, STEP_CHARS)),
            })
            .collect();
        if runs.len() > DIGEST_RUNS {
            named.push(format!("and {} more", runs.len() - DIGEST_RUNS));
        }
        optional.push((
            5,
            sentence(format!("Runs open: {}", named.join("; "))),
            Some(format!(
                "Runs open: {}, {dead} at a dead end; `herdr run status`.",
                digest.runs.len()
            )),
        ));
    }
    if !digest.proposals.is_empty() {
        let mut named: Vec<String> = digest
            .proposals
            .iter()
            .take(DIGEST_PROPOSALS)
            .map(|(id, gap)| format!("{id} ({})", short(gap, GAP_CHARS)))
            .collect();
        if digest.proposals.len() > DIGEST_PROPOSALS {
            named.push(format!(
                "and {} more",
                digest.proposals.len() - DIGEST_PROPOSALS
            ));
        }
        // Old Nirvana's proposals sat unread for 24 days (plan 12 §7.4.2): the owner hears of them here.
        optional.push((
            6,
            sentence(format!("Retro proposals open: {}", named.join(", "))),
            Some(format!(
                "Retro proposals open: {} in .nirvana/proposals.",
                digest.proposals.len()
            )),
        ));
    }
    match &digest.board {
        None => {}
        Some((_, sections)) if sections.is_empty() => {
            optional.push((4, "Board: empty.".to_owned(), None));
        }
        Some((revision, sections)) => optional.push((
            4,
            format!("Board rev {revision}: {}.", sections.join(", ")),
            Some(format!(
                "Board rev {revision}, {} sections.",
                sections.len()
            )),
        )),
    }
    if !hidden.is_empty() {
        optional.push((
            2,
            sentence(format!(
                "Not in the foot, with headroom: {}",
                hidden.join(", ")
            )),
            None,
        ));
    }
    // Priority: the runs, then the proposals, the board, the hidden pools (the order pushed above).
    let size = |lines: &[(u8, String, Option<String>)]| {
        lines
            .iter()
            .map(|(_, line, _)| line.chars().count() + 1)
            .sum::<usize>()
    };
    for (at, full, compact) in optional {
        let room = DIGEST_MAX_CHARS.saturating_sub(size(&lines));
        if full.chars().count() < room {
            lines.push((at, full, None));
        } else if let Some(compact) = compact.filter(|c| c.chars().count() < room) {
            lines.push((at, compact, None));
        }
    }
    lines.sort_by_key(|(at, _, _)| *at);
    let text = lines
        .into_iter()
        .map(|(_, line, _)| line)
        .collect::<Vec<_>>()
        .join("\n");
    if text.chars().count() > DIGEST_MAX_CHARS {
        let cut: String = text.chars().take(DIGEST_MAX_CHARS - 1).collect();
        return format!("{cut}…");
    }
    text
}

/// The project a hook's folder belongs to: `$NIRVANA_PROJECT`, else the nearest folder holding `.git`, the board's
/// rule. No `git` process: a hook is on the session's path.
fn project_at(cwd: Option<&Path>) -> Option<Project> {
    if let Some(root) = std::env::var_os(nirvana_run::PROJECT_ENV).filter(|v| !v.is_empty()) {
        return Some(Project::at(PathBuf::from(root)));
    }
    let cwd = cwd
        .map(Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok())?;
    crate::nirvana_board::root(&cwd).map(Project::at)
}

fn hook_cwd(hook: &Value) -> Option<PathBuf> {
    hook.get("cwd")
        .and_then(Value::as_str)
        .filter(|cwd| !cwd.is_empty())
        .map(PathBuf::from)
}

/// The runs not yet ended, in name order, each with its state.
fn open_runs(project: &Project) -> Vec<(String, RunState)> {
    project
        .runs()
        .into_iter()
        .filter_map(|run| {
            let state = nirvana_run::replay(&project.journal(&run).read().0);
            state.ended.is_none().then_some((run, state))
        })
        .collect()
}

fn run_steps(project: &Project, now: u64) -> Vec<(String, Result<String, String>, u64)> {
    open_runs(project)
        .into_iter()
        .map(|(run, state)| {
            let live = |order: &str| crate::nirvana_worker::live(project, &run, order);
            let step = nirvana_run::check(&state, &live, now);
            (run, step, state.last_seq)
        })
        .collect()
}

/// The project's board line in `NIRVANA.md`.
pub fn board_line_text(root: &Path) -> String {
    let board = crate::nirvana_board::board_path(root);
    format!(
        "- board for {}: {}; `herdr board read` {BOARD_MARK}{} -->",
        root.display(),
        board.display(),
        root.display()
    )
}

/// Writes the project's board line into `<home>/nirvana/NIRVANA.md`, replacing its own line and nothing else; a
/// new file gets a short head. Written only when it changes, so a session start is no write at all.
pub fn write_board_line(home: &Path, root: &Path) -> std::io::Result<PathBuf> {
    let path = home.join(nirvana_seat::DIR).join(MEMORY_FILE);
    let text = std::fs::read_to_string(&path).ok();
    let mark = format!("{BOARD_MARK}{} -->", root.display());
    let line = board_line_text(root);
    let mut lines: Vec<String> = text
        .as_deref()
        .unwrap_or(MEMORY_HEAD)
        .lines()
        .map(str::to_owned)
        .collect();
    match lines.iter().position(|known| known.ends_with(&mark)) {
        Some(at) if lines[at] == line => return Ok(path),
        Some(at) => lines[at] = line,
        None => lines.push(line),
    }
    let mut out = lines.join("\n");
    out.push('\n');
    nirvana_seat::write_atomic(&path, out.as_bytes())?;
    Ok(path)
}

fn digest_cli() -> std::io::Result<()> {
    let hook = read_stdin();
    let now = now();
    let home = home_dir();
    let project = project_at(hook_cwd(&hook).as_deref());
    let memory = home.join(nirvana_seat::DIR).join(MEMORY_FILE);
    if let Some(project) = &project {
        let _ = write_board_line(&home, &project.root);
    }
    let board = project.as_ref().map(|project| {
        let board = crate::nirvana_board::read(&project.root).unwrap_or_default();
        (
            board.revision,
            board.sections.iter().map(|s| s.name.clone()).collect(),
        )
    });
    let sources = with_live_max(cached_sources(), &read_statusline_file(&home));
    let digest = Digest {
        now,
        exe: crate::nirvana_board::exe().display().to_string(),
        sources,
        floor: table().map_or(0.05, |table| table.constants.floor),
        board,
        runs: project
            .as_ref()
            .map(|project| {
                run_steps(project, now)
                    .into_iter()
                    .map(|(run, step, _)| (run, step))
                    .collect()
            })
            .unwrap_or_default(),
        // O5's reader, the one the receipt names them by (O4b carried a copy until O5 merged).
        proposals: project
            .as_ref()
            .map(|project| {
                crate::nirvana_retro::open(project)
                    .into_iter()
                    .map(|(id, gap, _)| (id, gap))
                    .collect()
            })
            .unwrap_or_default(),
        memory: memory.display().to_string(),
    };
    if let Some(session_id) = hook.get("session_id").and_then(Value::as_str) {
        let model = hook
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(crate::nirvana_launch::MODEL);
        let folder = project
            .as_ref()
            .map(|p| p.root.as_path())
            .unwrap_or(Path::new("."));
        let (_launch_model, launch_effort) =
            crate::nirvana_launch::orchestrator_launch_seat_for(&home, folder);
        let _ = record_session_seat(&home, session_id, model, &launch_effort);
    }
    println!("{}", digest_text(&digest));
    Ok(())
}

/// Plan 20 §3.3: records the orchestrator's seat at session start in <home>/nirvana/sessions/<session_id>.json.
pub fn session_seat_path(home: &Path, session_id: &str) -> PathBuf {
    home.join(nirvana_seat::DIR)
        .join("sessions")
        .join(format!("{session_id}.json"))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSeatRecord {
    pub session_id: String,
    pub model: String,
    pub seat: String,
    pub effort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_notified_seat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_notified_effort: Option<String>,
}

pub fn record_session_seat(
    home: &Path,
    session_id: &str,
    model: &str,
    effort: &str,
) -> std::io::Result<PathBuf> {
    let seat = if let Ok(table) = crate::nirvana_seat::table() {
        seat_for_model(&table, model)
            .map(|s| s.id.clone())
            .unwrap_or_else(|| {
                if model.contains("sonnet") {
                    "sonnet".to_string()
                } else {
                    "opus".to_string()
                }
            })
    } else if model.contains("sonnet") {
        "sonnet".to_string()
    } else {
        "opus".to_string()
    };
    let record = SessionSeatRecord {
        session_id: session_id.to_string(),
        model: model.to_string(),
        seat,
        effort: effort.to_string(),
        last_notified_seat: None,
        last_notified_effort: None,
    };
    let path = session_seat_path(home, session_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(&record)?;
    std::fs::write(&path, text)?;
    Ok(path)
}

pub fn read_session_seat(home: &Path, session_id: &str) -> Option<SessionSeatRecord> {
    let path = session_seat_path(home, session_id);
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

pub fn write_session_seat(home: &Path, record: &SessionSeatRecord) -> std::io::Result<()> {
    let path = session_seat_path(home, &record.session_id);
    let text = serde_json::to_string_pretty(record)?;
    std::fs::write(&path, text)
}

/// Plan 20 §3.3: compares the running seat with the rule's seat now.
/// Returns (Some(line), Some(notified_pair)) when they differ, once per change.
pub fn seat_notice(
    running_seat: &str,
    running_effort: &str,
    rule_seat: &str,
    rule_effort: &str,
    rule_detail: &str,
    last_notified: Option<(&str, &str)>,
) -> (Option<String>, Option<(String, String)>) {
    if running_seat == rule_seat && running_effort == rule_effort {
        return (None, None);
    }
    if let Some((notified_seat, notified_effort)) = last_notified {
        if notified_seat == rule_seat && notified_effort == rule_effort {
            return (
                None,
                Some((notified_seat.to_string(), notified_effort.to_string())),
            );
        }
    }
    let line = format!(
        "Nirvana Claude runs on {running_seat} {running_effort}; the mode now seats {rule_seat} {rule_effort} ({rule_detail}). Press Restart Claude to switch."
    );
    (
        Some(line),
        Some((rule_seat.to_string(), rule_effort.to_string())),
    )
}

// ---- guard-spawn (PreToolUse on Agent) ----

/// A subagent definition's `model` and `effort`, from its file's front matter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentDef {
    pub name: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

pub fn front_matter(text: &str) -> AgentDef {
    let mut def = AgentDef::default();
    let text = text.trim_start_matches('\u{feff}');
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return def;
    };
    for line in rest.lines() {
        if line.trim_end() == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'').to_owned();
        let value = (!value.is_empty()).then_some(value);
        match key.trim() {
            "name" => def.name = value,
            "model" => def.model = value,
            "effort" => def.effort = value,
            _ => {}
        }
    }
    def
}

/// Where a spawn's `subagent_type` is defined: this plugin's `agents/` for `nirvana:<name>`, else the project's
/// and the user's `agents/` folders, matched by the file's `name`. Another plugin's agents, and Claude Code's
/// built-in ones, are not found: a spawn of those must name both itself.
pub fn find_agent(
    agent_type: &str,
    plugin_root: Option<&Path>,
    dirs: &[PathBuf],
) -> Option<AgentDef> {
    if let Some((plugin, name)) = agent_type.split_once(':') {
        if plugin != PLUGIN {
            return None;
        }
        let file = plugin_root?
            .join("agents")
            .join(format!("{}.md", name.replace(':', "/")));
        return std::fs::read_to_string(file)
            .ok()
            .map(|text| front_matter(&text));
    }
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let def = front_matter(&text);
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
            if def.name.as_deref().or(stem.as_deref()) == Some(agent_type) {
                return Some(def);
            }
        }
    }
    None
}

/// The pool a spawn would spend, and its 5 h window as the guard sees it.
#[derive(Debug, Clone, Default)]
pub struct Reserve {
    /// The reserve window's share used, when known and fresh; `None` reads as unknown and lets the spawn through,
    /// as the seat engine's reserve filter does.
    pub used: Option<f32>,
    pub resets_at: Option<u64>,
    /// What live leases on the pool expect to spend of the window.
    pub booked: f32,
    pub now: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allow,
    Deny(String),
}

/// A spawn's request, from the hook's `tool_input`.
#[derive(Debug, Clone, Default)]
pub struct Spawn {
    pub agent_type: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub prompt: String,
}

impl Spawn {
    pub fn from_input(input: &Value) -> Self {
        let text = |key: &str| {
            input
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
        };
        Self {
            agent_type: text("subagent_type").unwrap_or_else(|| "general-purpose".to_owned()),
            model: text("model"),
            effort: text("effort"),
            prompt: text("prompt").unwrap_or_default(),
        }
    }
}

/// The seat a Claude model id or alias sits in: the table's `claude` seat with that model, else the one whose id
/// the name holds (`opus`, `claude-opus-5-5`, `opus[1m]`).
pub fn seat_for_model<'a>(table: &'a Table, model: &str) -> Option<&'a nirvana_seat::Seat> {
    let model = model.to_ascii_lowercase();
    let claude = || table.seats.iter().filter(|seat| seat.cli == "claude");
    claude()
        .find(|seat| seat.model == model)
        .or_else(|| claude().find(|seat| model.contains(&seat.id)))
}

/// A seat's expected share of `window` at an effort, as the seat engine prices it (`Seat::use_cell`): the cell for
/// the effort, else the nearest dearer one, else the dearest; a window the cell does not name takes its largest.
pub fn expected_share(seat: &nirvana_seat::Seat, effort: Effort, window: &str) -> f32 {
    let cell = seat
        .uses
        .iter()
        .find(|cell| cell.effort == effort)
        .or_else(|| {
            seat.uses
                .iter()
                .filter(|cell| cell.effort > effort)
                .min_by_key(|cell| cell.effort)
        })
        .or_else(|| seat.uses.iter().max_by_key(|cell| cell.effort));
    cell.map_or(0.0, |cell| {
        cell.windows
            .get(window)
            .copied()
            .unwrap_or_else(|| cell.windows.values().copied().fold(0.0, f32::max))
    })
}

/// The brief's own lines: `Class: <class>`, `Order: <id>`, `Run: <id>` (the seat skill asks for them first).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BriefFields {
    pub class: Option<String>,
    pub order: Option<String>,
    pub run: Option<String>,
}

pub fn brief_fields(text: &str) -> BriefFields {
    let mut fields = BriefFields::default();
    for line in text.lines().take(40) {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim().trim_matches('`').to_owned();
        if value.is_empty() || value.contains(' ') {
            continue;
        }
        let slot = match key
            .trim()
            .trim_start_matches(['-', '*', ' '])
            .to_ascii_lowercase()
            .as_str()
        {
            "class" => &mut fields.class,
            "order" => &mut fields.order,
            "run" => &mut fields.run,
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(value);
        }
    }
    fields
}

fn effort_of(word: &str) -> Option<Effort> {
    serde_json::from_value(Value::String(word.to_ascii_lowercase())).ok()
}

/// Loads live leases from `<home>/nirvana/leases.json`.
pub fn load_live_leases(home: &Path) -> Leases {
    let path = home.join(nirvana_seat::DIR).join(nirvana_seat::LEASES_FILE);
    nirvana_seat::load_leases(&path).unwrap_or_default()
}

/// The rule (plan 12 §6, plan 20 §2, CLAUDE.md rule 5): a spawn names a model and an effort, its own or its agent file's;
/// its brief's `Order:` must have a live booking in `leases.json` for that seat and effort (plan 20 §2); and a spawn on
/// the reserve pool leaves the orchestrator its reserve of the reserve window.
pub fn guard(spawn: &Spawn, def: Option<&AgentDef>, table: &Table, reserve: &Reserve) -> Verdict {
    let leases = load_live_leases(&home_dir());
    guard_with_leases(spawn, def, table, reserve, &leases)
}

pub fn guard_with_leases(
    spawn: &Spawn,
    def: Option<&AgentDef>,
    table: &Table,
    reserve: &Reserve,
    leases: &Leases,
) -> Verdict {
    // The orchestrator's own agent file has no prompt (it keeps Claude Code's), so a spawn of it is a bare Opus
    // subagent at the orchestrator's effort, not a worker.
    if matches!(spawn.agent_type.as_str(), "nirvana" | "nirvana:nirvana") {
        return Verdict::Deny(
            "herdr guard: nirvana is the orchestrator's own seat, not a worker. Spawn a nirvana:worker-* agent, or \
             seat the order with `herdr seat pick`."
                .to_owned(),
        );
    }
    if spawn.agent_type == "fork" {
        return Verdict::Deny(
            "herdr guard: a fork always runs on your own seat (Opus 5.5 at your effort). Spawn a worker agent that \
             names its seat and effort, such as nirvana:worker-sonnet-medium, or seat the order with `herdr seat pick`."
                .to_owned(),
        );
    }
    let model = spawn
        .model
        .clone()
        .or_else(|| def.and_then(|def| def.model.clone()))
        .filter(|model| model != "inherit");
    let effort = spawn
        .effort
        .clone()
        .or_else(|| def.and_then(|def| def.effort.clone()));
    let missing = match (model.is_none(), effort.is_none()) {
        (true, true) => Some("no model and no effort"),
        (true, false) => Some("no model"),
        (false, true) => Some("no effort"),
        (false, false) => None,
    };
    if let Some(missing) = missing {
        return Verdict::Deny(format!(
            "herdr guard: this spawn of {} names {missing}, in its call or its agent file, so it would inherit your \
             seat (Opus 5.5 at your effort). Spawn a worker agent that names both, such as \
             nirvana:worker-sonnet-medium, or pass model and effort.",
            spawn.agent_type,
        ));
    }
    let (model, effort_word) = (model.unwrap_or_default(), effort.unwrap_or_default());
    let Some(effort) = effort_of(&effort_word) else {
        return Verdict::Deny(format!(
            "herdr guard: effort {effort_word:?} is not one of low, medium, high, xhigh, max."
        ));
    };

    // Plan 20 §2: A spawn needs a booking in leases.json for this seat and effort.
    let fields = brief_fields(&spawn.prompt);
    let lease = fields
        .order
        .as_deref()
        .and_then(|order| leases.live(order, reserve.now));

    let Some(lease) = lease else {
        let class_arg = fields.class.as_deref().unwrap_or("<class>");
        let order_arg = fields.order.as_deref().unwrap_or("<order>");
        let run_arg = fields.run.as_deref().unwrap_or("<run>");
        return Verdict::Deny(format!(
            "herdr guard: seat this first: herdr seat pick --class {class_arg} --size small --book {order_arg} --run {run_arg}"
        ));
    };

    let lease_seat_def = table.seat(&lease.seat);
    if lease_seat_def.as_ref().is_some_and(|s| s.cli == "agy") {
        return Verdict::Deny(format!(
            "herdr guard: the picker seated {} on {} {} ({}). Start it with herdr seat run {} --order ...",
            lease.order,
            lease.seat,
            lease.effort.word(),
            lease.pool,
            lease.seat,
        ));
    }

    let spawn_seat = seat_for_model(table, &model);
    let seat_matches = spawn_seat.is_some_and(|s| s.id == lease.seat);
    let effort_ok = effort <= lease.effort;

    if !seat_matches || !effort_ok {
        let spawn_seat_id = spawn_seat.map(|s| s.id.as_str()).unwrap_or(model.as_str());
        return Verdict::Deny(format!(
            "herdr guard: {} is booked on {} {}, but this spawn is on {} {}. A spawn must match its booking: spawn nirvana:worker-{}-{}, or seat with herdr seat pick --override {} --book {}",
            lease.order,
            lease.seat,
            lease.effort.word(),
            spawn_seat_id,
            effort.word(),
            lease.seat,
            lease.effort.word(),
            spawn_seat_id,
            lease.order,
        ));
    }

    let constants = &table.constants;
    let seat = spawn_seat.unwrap();
    if seat.pool != constants.reserve_pool {
        return Verdict::Allow;
    }
    let Some(used) = reserve.used else {
        return Verdict::Allow;
    };
    let weight = fields
        .class
        .and_then(|class| table.class(&class).map(|class| class.weight))
        .unwrap_or(1.0);
    let expected = expected_share(seat, effort, &constants.reserve_window)
        * table.size(SUBAGENT_SIZE).use_factor
        * weight;
    let after = 1.0 - used - reserve.booked - expected;
    if after + EDGE >= constants.reserve {
        return Verdict::Allow;
    }
    let resets = reserve
        .resets_at
        .map(|at| {
            format!(
                ", or wait for its reset in {}",
                nirvana_usage::short_duration(at.saturating_sub(reserve.now))
            )
        })
        .unwrap_or_default();
    let booked = if reserve.booked > 0.0005 {
        format!(", {} booked by live orders", pct(reserve.booked))
    } else {
        String::new()
    };
    Verdict::Deny(format!(
        "herdr guard: {} {} on {} would leave {} {} at {}, under your reserve of {} ({} used{booked}, about {} for \
         this spawn). Seat the order on another pool with `herdr seat pick`{resets}.",
        seat.id,
        effort.word(),
        seat.pool,
        seat.pool,
        constants.reserve_window,
        pct1(after.max(0.0)),
        pct(constants.reserve),
        pct1(used),
        pct(expected),
    ))
}

/// The reserve window as the guard reads it: the fresher of the statusline's reading and `usage.json`'s, each only
/// while fresh; and the live leases' bookings on the pool.
fn live_reserve(table: &Table, home: &Path, now: u64) -> Reserve {
    let constants = &table.constants;
    let mut readings: Vec<Reading> = cached_sources()
        .iter()
        .flat_map(|source| source.readings.iter().cloned())
        .collect();
    readings.extend(read_statusline_file(home));
    let reading = readings
        .into_iter()
        .filter(|r| r.pool == constants.reserve_pool && r.window == constants.reserve_window)
        .filter(|r| r.used.is_some() && nirvana_usage::freshness(r, now) == Freshness::Fresh)
        .max_by_key(|r| r.read_at);
    let leases = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
        .unwrap_or_default();
    let booked = leases
        .leases
        .iter()
        .filter(|lease| lease.live(now) && lease.pool == constants.reserve_pool)
        .filter_map(|lease| lease.expected.get(&constants.reserve_window))
        .sum();
    Reserve {
        used: reading.as_ref().and_then(|r| r.used),
        resets_at: reading.as_ref().and_then(|r| r.resets_at),
        booked,
        now,
    }
}

/// The user's Claude config folder: `$CLAUDE_CONFIG_DIR`, else `~/.claude`.
fn claude_config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|home| PathBuf::from(home).join(".claude"))
        })
}

fn guard_cli() -> std::io::Result<()> {
    let hook = read_stdin();
    let spawn = Spawn::from_input(hook.get("tool_input").unwrap_or(&Value::Null));
    let plugin_root = std::env::var_os("CLAUDE_PLUGIN_ROOT").map(PathBuf::from);
    let mut dirs = Vec::new();
    if let Some(cwd) = hook_cwd(&hook) {
        dirs.push(cwd.join(".claude").join("agents"));
    }
    if let Some(config) = claude_config_dir() {
        dirs.push(config.join("agents"));
    }
    let def = find_agent(&spawn.agent_type, plugin_root.as_deref(), &dirs);
    // A table that does not parse still refuses a spawn with no model or effort; only the reserve needs it.
    let table = table().unwrap_or_else(|| {
        nirvana_seat::parse(nirvana_seat::DEFAULT).expect("herdr's default seat table parses")
    });
    let reserve = live_reserve(&table, &home_dir(), now());
    if let Verdict::Deny(reason) = guard(&spawn, def.as_ref(), &table, &reserve) {
        println!(
            "{}",
            json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }})
        );
    }
    Ok(())
}

// ---- subagent-stop (SubagentStop) ----

/// `nirvana:worker-sonnet-medium` to its seat and effort.
pub fn worker_seat(agent_type: &str) -> Option<(String, Effort)> {
    let name = agent_type
        .strip_prefix(PLUGIN)
        .and_then(|rest| rest.strip_prefix(':'))?;
    let rest = name.strip_prefix(WORKER_PREFIX)?;
    let (seat, effort) = rest.rsplit_once('-')?;
    Some((seat.to_owned(), effort_of(effort)?))
}

fn message_text(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// A subagent's transcript: its brief (the first user message) and its tokens, each API response once. Claude
/// Code writes one line per content block with the response's usage so far on each, so lines are grouped by message
/// id and the group's largest figure per field counts: the last line holds the final output count (live on
/// 2026-10-18 a response's lines read 3, 3, 3, 2325, and keeping the first gave a record of 98 output for ~3,500).
pub fn read_transcript(text: &str) -> (String, Tokens) {
    let mut brief = None;
    let mut seen: std::collections::HashMap<String, [u64; 4]> = std::collections::HashMap::new();
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        let message = &entry["message"];
        match entry.get("type").and_then(Value::as_str) {
            Some("user") if brief.is_none() => {
                let text = message_text(message);
                if !text.trim().is_empty() {
                    brief = Some(text);
                }
            }
            Some("assistant") => {
                let id = message
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("line-{}", seen.len()));
                let Some(usage) = message.get("usage") else {
                    continue;
                };
                let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                let row = [
                    n("input_tokens"),
                    n("cache_creation_input_tokens"),
                    n("cache_read_input_tokens"),
                    n("output_tokens"),
                ];
                let slot = seen.entry(id).or_insert([0; 4]);
                for (kept, now) in slot.iter_mut().zip(row) {
                    *kept = (*kept).max(now);
                }
            }
            _ => {}
        }
    }
    let mut tokens = Tokens::default();
    for [input, cache_write, cache_read, output] in seen.into_values() {
        tokens.input += input;
        tokens.cache_write += cache_write;
        tokens.cache_read += cache_read;
        tokens.output += output;
    }
    (brief.unwrap_or_default(), tokens)
}

/// The `message` of the last `SubagentHandback` call in a subagent's transcript. A worker that ends through that tool
/// writes its report there and has no final text block, so the hook's `last_assistant_message` is empty of it.
pub fn handback_message(transcript: &str) -> Option<String> {
    transcript
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("assistant"))
        .flat_map(|entry| {
            entry["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .filter(|block| block.get("name").and_then(Value::as_str) == Some("SubagentHandback"))
        .filter_map(|block| block["input"]["message"].as_str().map(str::to_owned))
        .next_back()
}

/// The Work Report in a worker's last message: the message itself when it is JSON, else its last fenced block
/// that parses, else the span from its last line that opens an object to the end. Checked by O3's `check_report`.
pub fn report_in(message: &str) -> Result<Value, String> {
    let trimmed = message.trim();
    let mut candidates: Vec<String> = vec![trimmed.to_owned()];
    let mut fences: Vec<String> = Vec::new();
    let mut inside: Option<Vec<&str>> = None;
    for line in trimmed.lines() {
        if line.trim_start().starts_with("```") {
            match inside.take() {
                Some(block) => fences.push(block.join("\n")),
                None => inside = Some(Vec::new()),
            }
            continue;
        }
        if let Some(block) = inside.as_mut() {
            block.push(line);
        }
    }
    candidates.extend(fences.into_iter().rev());
    if let Some(at) = trimmed.rfind("\n{") {
        candidates.push(trimmed[at + 1..].to_owned());
    }
    let mut last_err = "no Work Report: the last message holds no JSON object".to_owned();
    for candidate in candidates {
        let Ok(value) = serde_json::from_str::<Value>(candidate.trim()) else {
            continue;
        };
        match crate::nirvana_worker::check_report(&value) {
            Ok(()) => return Ok(value),
            Err(err) => last_err = err,
        }
    }
    Err(last_err)
}

/// The record line for a finished worker subagent, or why there is none. `None` back means the agent is not one of
/// this plugin's workers, which records nothing and says nothing.
pub fn subagent_line(
    table: &Table,
    hook: &Value,
    transcript: &str,
) -> Option<Result<nirvana_seat::RecordLine, String>> {
    let agent_type = hook.get("agent_type").and_then(Value::as_str)?;
    let (seat, effort) = worker_seat(agent_type)?;
    Some((|| {
        if table.seat(&seat).is_none() {
            return Err(format!("{agent_type}: no seat {seat} in seats.toml"));
        }
        let (brief, tokens) = read_transcript(transcript);
        let fields = brief_fields(&brief);
        let class = fields.class.ok_or_else(|| {
            format!("{agent_type}: its brief names no `Class:`, so its outcome has no class to be recorded under")
        })?;
        let agent = hook
            .get("agent_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let said = hook
            .get("last_assistant_message")
            .and_then(Value::as_str)
            .unwrap_or("");
        // The report is in the last text, else in the last handback call's message (a worker that ends through
        // SubagentHandback has no final text block).
        let handback = handback_message(transcript);
        let report = report_in(said).or_else(|err| match &handback {
            Some(message) => report_in(message),
            None => Err(err),
        });
        let mut summary = None;
        let outcome = match report {
            Ok(report) => format!(
                "subagent-{}",
                report
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("done")
            ),
            // A handback that says what it did in prose has reported, in the wrong shape: not the same fact as a
            // worker that said nothing, so it gets its own word and the start of what it said.
            Err(_) => match handback.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
                Some(message) => {
                    summary = Some(message.chars().take(200).collect::<String>());
                    "subagent-prose-report".to_owned()
                }
                None => "subagent-no-report".to_owned(),
            },
        };
        let tokens = (tokens != Tokens::default()).then_some(tokens);
        let mut line = json!({
            "order": fields.order.unwrap_or_else(|| format!("sub-{agent}")),
            "run": fields.run,
            "seat": seat,
            "effort": effort,
            "class": class,
            "chain": SUBAGENT_CHAIN,
            "outcome": outcome,
            "tokens": tokens,
        });
        if let Some(summary) = summary {
            line["summary"] = Value::String(summary);
        }
        serde_json::from_value(line).map_err(|err| err.to_string())
    })())
}

fn subagent_stop_cli() -> std::io::Result<()> {
    let hook = read_stdin();
    let Some(table) = table() else {
        return Ok(());
    };
    let path = hook
        .get("agent_transcript_path")
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let read = || {
        path.as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default()
    };
    // The transcript lags the hook (Claude Code's hooks page: read `last_assistant_message`, "because the transcript
    // may lag"): live on 2026-10-18 a worker's record had no tokens, its last response written after the hook read
    // the file. So a transcript with a brief and no response yet is read again, for at most TRANSCRIPT_WAIT.
    let worker = hook
        .get("agent_type")
        .and_then(Value::as_str)
        .and_then(worker_seat)
        .is_some();
    let started = std::time::Instant::now();
    let mut transcript = read();
    while worker
        && path.is_some()
        && read_transcript(&transcript).1 == Tokens::default()
        && started.elapsed() < TRANSCRIPT_WAIT
    {
        std::thread::sleep(std::time::Duration::from_millis(100));
        transcript = read();
    }
    match subagent_line(&table, &hook, &transcript) {
        None => {}
        // A hook's stderr shows only in Claude Code's verbose mode: the turn is never failed over a record.
        Some(Err(why)) => eprintln!("herdr nirvana subagent-stop: not recorded: {why}"),
        Some(Ok(line)) => match nirvana_seat::complete(&table, line, now()) {
            Ok(line) => {
                if let Err(err) = nirvana_seat::record_once(&nirvana_seat::dir(), line, true, now())
                {
                    eprintln!("herdr nirvana subagent-stop: {err}");
                }
            }
            Err(err) => eprintln!("herdr nirvana subagent-stop: not recorded: {err}"),
        },
    }
    Ok(())
}

// ---- stop (Stop) ----

/// What the Stop hook prints for the open runs' dead ends: each (run, state) is raised once. Not yet raised and
/// not already continuing from a Stop hook: a block, so the orchestrator takes the next step before it stops.
/// Already continuing: a notice to the owner. Raised before: nothing. So it never blocks forever.
pub fn stop_output(
    dead_ends: &[(String, String, u64)],
    raised: &BTreeMap<String, u64>,
    stop_hook_active: bool,
) -> Option<Value> {
    let fresh: Vec<&(String, String, u64)> = dead_ends
        .iter()
        .filter(|(run, _, seq)| raised.get(run) != Some(seq))
        .collect();
    if fresh.is_empty() {
        return None;
    }
    let named: Vec<String> = fresh
        .iter()
        .map(|(run, why, _)| format!("{run}: {}", short(why, STEP_CHARS)))
        .collect();
    let text = format!(
        "herdr: an open run is at a dead end, with no live worker and no next step ({}). Seat its next order, gate \
         what waits, ask the owner, or end it with `herdr run end <run>`.",
        named.join("; ")
    );
    Some(if stop_hook_active {
        json!({"systemMessage": text})
    } else {
        json!({"decision": "block", "reason": text})
    })
}

fn stop_cli() -> std::io::Result<()> {
    let hook = read_stdin();
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return Ok(());
    }
    let Some(project) = project_at(hook_cwd(&hook).as_deref()) else {
        return Ok(());
    };
    let dead_ends: Vec<(String, String, u64)> = run_steps(&project, now())
        .into_iter()
        .filter_map(|(run, step, seq)| step.err().map(|why| (run, why, seq)))
        .collect();
    let path = home_dir().join(nirvana_seat::DIR).join(STOP_FILE);
    let mut all: BTreeMap<String, BTreeMap<String, u64>> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let key = project.root.display().to_string();
    let raised = all.get(&key).cloned().unwrap_or_default();
    let stop_hook_active = hook
        .get("stop_hook_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let out = stop_output(&dead_ends, &raised, stop_hook_active);
    // Only the runs at a dead end now are kept, so a run that moved on and stuck again is raised again.
    let now_raised: BTreeMap<String, u64> = dead_ends
        .iter()
        .map(|(run, _, seq)| (run.clone(), *seq))
        .collect();
    if now_raised != raised {
        if now_raised.is_empty() {
            all.remove(&key);
        } else {
            all.insert(key, now_raised);
        }
        let _ = nirvana_seat::write_atomic(&path, serde_json::to_string(&all)?.as_bytes());
    }
    if let Some(out) = out {
        println!("{out}");
    }
    Ok(())
}

// ---- the statusline ----

/// One open order for the statusline: its id, seat and phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenOrder {
    pub order: String,
    pub seat: String,
    pub phase: String,
}

fn open_orders(project: &Project) -> Vec<OpenOrder> {
    open_runs(project)
        .into_iter()
        .flat_map(|(_, state)| state.orders)
        .filter(|order| !order.phase.terminal() && order.phase != Phase::Reseated)
        .map(|order| OpenOrder {
            order: order.id().to_owned(),
            seat: order.seated.seat.clone(),
            phase: order.phase.word().to_owned(),
        })
        .collect()
}

/// The statusline: each drawn pool's tightest window (`FOOT_HIDDEN` honoured, as the foot does), its reset when
/// the width allows, and the open orders. Narrower: resets go first, then the orders' names for their count.
pub fn statusline_text(
    sources: &[Source; 4],
    orders: &[OpenOrder],
    width: usize,
    now: u64,
) -> String {
    let rows: Vec<nirvana_usage::UsageRow> = nirvana_usage::pool_rows(sources, now)
        .into_iter()
        .filter(|row| !nirvana_usage::FOOT_HIDDEN.contains(&row.pool.as_str()))
        .collect();
    let pool_parts = |resets: bool| -> Vec<String> {
        rows.iter()
            .filter_map(|row| match &row.figure {
                Figure::Window(shown) => {
                    let used = shown.used.map_or_else(|| "--".to_owned(), pct);
                    let stale = if shown.stale { "?" } else { "" };
                    let reset = match shown.resets_in {
                        Some(left) if resets => format!(" {}", nirvana_usage::short_duration(left)),
                        _ => String::new(),
                    };
                    Some(format!(
                        "{} {} {used}{stale}{reset}",
                        row.label(),
                        shown.window
                    ))
                }
                _ => None,
            })
            .collect()
    };
    let order_part = |names: bool| -> Option<String> {
        if orders.is_empty() {
            return None;
        }
        if names {
            let named: Vec<String> = orders
                .iter()
                .map(|o| format!("{} {} {}", o.order, o.seat, o.phase))
                .collect();
            Some(format!("orders: {}", named.join(", ")))
        } else {
            Some(format!(
                "{} order{}",
                orders.len(),
                if orders.len() == 1 { "" } else { "s" }
            ))
        }
    };
    let compose = |resets: bool, names: bool| -> String {
        let mut parts = pool_parts(resets);
        parts.extend(order_part(names));
        parts.join(" · ")
    };
    for (resets, names) in [(true, true), (false, true), (false, false)] {
        let line = compose(resets, names);
        if line.chars().count() <= width {
            return line;
        }
    }
    let line = compose(false, false);
    if width == 0 {
        return String::new();
    }
    let cut: String = line.chars().take(width.saturating_sub(1)).collect();
    format!("{cut}…")
}

fn statusline_cli() -> std::io::Result<()> {
    let input = read_stdin();
    let now = now();
    let home = home_dir();
    let live = statusline_readings(&input, now);
    if !live.is_empty() {
        let _ = nirvana_seat::write_atomic(
            &home.join(nirvana_seat::DIR).join(STATUSLINE_FILE),
            serde_json::to_string(&live)?.as_bytes(),
        );
    }
    let sources = with_live_max(cached_sources(), &live);
    let cwd = input
        .pointer("/workspace/current_dir")
        .or_else(|| input.get("cwd"))
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let orders = project_at(cwd.as_deref())
        .map(|project| open_orders(&project))
        .unwrap_or_default();
    // Claude Code sets COLUMNS for the statusline (its docs: the script's output is captured, so `tput cols` cannot
    // read the terminal). The prompt box's padding takes a few cells.
    let width = std::env::var("COLUMNS")
        .ok()
        .and_then(|cols| cols.trim().parse::<usize>().ok())
        .unwrap_or(120)
        .saturating_sub(4);
    println!("{}", statusline_text(&sources, &orders, width, now));
    Ok(())
}

// ---- hooks the folder runs itself ----

/// The settings files Claude Code reads for a session in `cwd`: the folder's and its repository root's local and
/// shared settings (Preload writes the local ones), then the user's.
pub fn settings_files(cwd: &Path, config: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![cwd.to_path_buf()];
    if let Some(root) = crate::nirvana_board::root(cwd).filter(|root| root != cwd) {
        dirs.push(root);
    }
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| {
            let claude = dir.join(".claude");
            [
                claude.join("settings.local.json"),
                claude.join("settings.json"),
            ]
        })
        .collect();
    if let Some(config) = config {
        files.push(config.join("settings.json"));
    }
    files
}

/// Whether the folder's own settings already run herdr's `tail` command on `event` (Preload's switches,
/// `nirvana_jev_install`), so the plugin's copy of the same hook stays quiet and the session hears it once.
pub fn folder_runs(input: &str, event: &str, tail: &str) -> bool {
    folder_runs_with(input, event, tail, claude_config_dir().as_deref())
}

/// `folder_runs` with the user's config folder given, so a test does not read the owner's own settings.
pub fn folder_runs_with(input: &str, event: &str, tail: &str, config: Option<&Path>) -> bool {
    let hook: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    let Some(cwd) = hook_cwd(&hook).or_else(|| std::env::current_dir().ok()) else {
        return false;
    };
    settings_files(&cwd, config)
        .iter()
        .filter_map(|file| std::fs::read_to_string(file).ok())
        .filter_map(|text| serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok())
        .any(|settings| crate::nirvana_jev_install::has_hook(&settings, event, tail))
}

// ---- log-usage (Stop, SessionStart resume) ----

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UsageOffsetState {
    pub offset: u64,
    #[serde(default)]
    pub seen_ids: Vec<String>,
}

/// Sums assistant token usage from a Claude Code JSON transcript from byte `offset`,
/// advancing only to the last newline, skipping sidechain lines, and deduplicating by message id.
pub fn read_transcript_usage(
    path: &Path,
    offset: u64,
    prior_seen_ids: &[String],
) -> std::io::Result<(Tokens, u64, Vec<String>)> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let seek_pos = if offset <= len { offset } else { 0 };
    file.seek(std::io::SeekFrom::Start(seek_pos))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;

    // Advance the offset only to the last \n read.
    let (to_process, new_offset) = match text.rfind('\n') {
        Some(idx) => (&text[..=idx], seek_pos + (idx + 1) as u64),
        None => ("", seek_pos),
    };

    let prior_set: std::collections::HashSet<&str> =
        prior_seen_ids.iter().map(String::as_str).collect();

    let mut seen: std::collections::HashMap<String, [u64; 4]> = std::collections::HashMap::new();
    let mut newly_seen_ids = Vec::new();

    for line in to_process.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        // Skip lines with "isSidechain": true
        if entry.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        if entry.get("type").and_then(Value::as_str) == Some("assistant") {
            let message = &entry["message"];
            let id = message
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("line-{}", seen.len()));

            // Remember the ids counted at the end of each read, so a later read never counts the same id again.
            if prior_set.contains(id.as_str()) {
                continue;
            }

            let Some(usage) = message.get("usage") else {
                continue;
            };
            let n = |key: &str| usage.get(key).and_then(Value::as_u64).unwrap_or(0);
            let row = [
                n("input_tokens"),
                n("cache_creation_input_tokens"),
                n("cache_read_input_tokens"),
                n("output_tokens"),
            ];
            if !seen.contains_key(&id) {
                newly_seen_ids.push(id.clone());
            }
            let usage_slot = seen.entry(id).or_insert([0; 4]);
            for (saved, cur) in usage_slot.iter_mut().zip(row) {
                *saved = (*saved).max(cur);
            }
        }
    }
    let mut tokens = Tokens::default();
    for [input, cache_write, cache_read, output] in seen.into_values() {
        tokens.input += input;
        tokens.cache_write += cache_write;
        tokens.cache_read += cache_read;
        tokens.output += output;
    }

    let mut combined_ids = prior_seen_ids.to_vec();
    for id in newly_seen_ids {
        if !combined_ids.contains(&id) {
            combined_ids.push(id);
        }
    }
    if combined_ids.len() > 200 {
        let keep_from = combined_ids.len() - 200;
        combined_ids = combined_ids.split_off(keep_from);
    }

    Ok((tokens, new_offset, combined_ids))
}

pub fn log_usage_from(hook: &Value) -> std::io::Result<Option<Tokens>> {
    if should_skip_orchestrator_hook(hook, None) {
        return Ok(None);
    }
    let is_session_start =
        hook.get("hook_event_name").and_then(Value::as_str) == Some("SessionStart");
    if is_session_start && hook.get("source").and_then(Value::as_str) != Some("resume") {
        return Ok(None);
    }
    let Some(transcript_path) = hook
        .get("transcript_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
    else {
        return Ok(None);
    };
    let Some(session_id) = hook.get("session_id").and_then(Value::as_str) else {
        return Ok(None);
    };

    let dir = home_dir().join(nirvana_seat::DIR).join("usage-offsets");
    let offset_path = dir.join(format!("{session_id}.json"));
    let stored_state: Option<UsageOffsetState> = std::fs::read_to_string(&offset_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());

    // S6: On SessionStart for a session with no stored offset, set the offset to the file's current length and count nothing.
    if is_session_start && stored_state.is_none() {
        let len = std::fs::metadata(&transcript_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let init_state = UsageOffsetState {
            offset: len,
            seen_ids: Vec::new(),
        };
        let _ = std::fs::create_dir_all(&dir);
        let _ = nirvana_seat::write_atomic(
            &offset_path,
            serde_json::to_string(&init_state)?.as_bytes(),
        );
        return Ok(None);
    }

    let (stored_offset, prior_seen_ids) = match &stored_state {
        Some(s) => (s.offset, s.seen_ids.clone()),
        None => (0, Vec::new()),
    };

    let (tokens, new_offset, combined_ids) =
        match read_transcript_usage(&transcript_path, stored_offset, &prior_seen_ids) {
            Ok(res) => res,
            Err(_) => return Ok(None),
        };

    let project = project_at(hook_cwd(hook).as_deref());
    if let Some(project) = project {
        let runs = open_runs(&project);
        let most_recent = runs.iter().max_by_key(|(_, state)| {
            (
                state.opened.as_ref().map(|o| o.at).unwrap_or(0),
                state.last_seq,
            )
        });
        if let Some((run_name, _)) = most_recent {
            if tokens != Tokens::default() {
                let journal = project.journal(run_name);
                let kind = crate::nirvana_run::Kind::Usage {
                    order: None,
                    attempt: 0,
                    session: Some(session_id.to_string()),
                    tokens,
                };
                let _ = journal.transact(now(), |_| Ok((vec![kind], ())));
            }
        }
    }

    let new_state = UsageOffsetState {
        offset: new_offset,
        seen_ids: combined_ids,
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = nirvana_seat::write_atomic(&offset_path, serde_json::to_string(&new_state)?.as_bytes());
    Ok(Some(tokens))
}

fn log_usage_cli() -> std::io::Result<()> {
    log_usage_from(&read_stdin())?;
    Ok(())
}

// ---- own-edit limit (UserPromptSubmit, PreToolUse, PostToolUse) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditLimit {
    pub files: usize,
    pub lines: usize,
}

impl Default for EditLimit {
    fn default() -> Self {
        Self {
            files: 1,
            lines: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FileDelta {
    pub added: usize,
    pub deleted: usize,
    pub hash: Option<String>,
    #[serde(default)]
    pub is_tracked: bool,
    #[serde(default)]
    pub is_binary: bool,
    #[serde(default)]
    pub line_count: usize,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EditState {
    pub files: BTreeMap<String, FileDelta>,
    #[serde(default)]
    pub fail_open: bool,
}

fn is_claude_project_memory(normalized: &str) -> bool {
    let is_abs = normalized.starts_with('/')
        || normalized.starts_with('~')
        || (normalized.len() >= 2 && normalized.as_bytes()[1] == b':');
    if is_abs {
        let under_projects = normalized.contains("/.claude/projects/")
            || normalized.starts_with("~/.claude/projects/");
        let in_memory = normalized.contains("/memory/") || normalized.ends_with("/memory");
        if under_projects && in_memory {
            return true;
        }
    }
    false
}

pub fn is_out_of_scope(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with(".nirvana/")
        || normalized == ".nirvana"
        || normalized.starts_with(".claude/")
        || normalized == ".claude"
        || normalized == MEMORY_FILE
        || normalized.starts_with("../")
        || is_claude_project_memory(&normalized)
    {
        return true;
    }
    false
}

fn multiset_line_diff(base_content: &str, now_content: &str) -> usize {
    let mut base_counts = std::collections::HashMap::new();
    for line in base_content.lines() {
        *base_counts.entry(line).or_insert(0usize) += 1;
    }
    let mut now_counts = std::collections::HashMap::new();
    for line in now_content.lines() {
        *now_counts.entry(line).or_insert(0usize) += 1;
    }
    let mut diff = 0;
    let mut all_lines = std::collections::HashSet::new();
    all_lines.extend(base_counts.keys().copied());
    all_lines.extend(now_counts.keys().copied());
    for line in all_lines {
        let b = base_counts.get(line).copied().unwrap_or(0);
        let n = now_counts.get(line).copied().unwrap_or(0);
        diff += (n as isize - b as isize).unsigned_abs();
    }
    diff
}

pub fn edit_budget(baseline: &EditState, now: &EditState, limit: EditLimit) -> Option<String> {
    if now.fail_open || baseline.fail_open {
        return None;
    }

    let mut changed_files = 0;
    let mut changed_lines = 0;

    for (path, now_entry) in &now.files {
        if is_out_of_scope(path) {
            continue;
        }
        let base_entry = baseline.files.get(path);
        match base_entry {
            None => {
                changed_files += 1;
                if now_entry.is_binary {
                    // Binary files count as 1 file and 0 lines
                } else if now_entry.is_tracked {
                    changed_lines += now_entry.added + now_entry.deleted;
                } else {
                    let lines = if now_entry.line_count > 0 {
                        now_entry.line_count
                    } else {
                        now_entry.added
                    };
                    changed_lines += lines;
                }
            }
            Some(base_entry) => {
                if now_entry.is_binary || base_entry.is_binary {
                    if now_entry.hash != base_entry.hash {
                        changed_files += 1;
                    }
                } else if let (Some(base_content), Some(now_content)) =
                    (&base_entry.content, &now_entry.content)
                {
                    let diff = multiset_line_diff(base_content, now_content);
                    if diff > 0 {
                        changed_files += 1;
                        changed_lines += diff;
                    }
                } else if base_entry.hash.is_some() || now_entry.hash.is_some() {
                    if base_entry.hash != now_entry.hash {
                        changed_files += 1;
                        let line_diff = (now_entry.line_count as isize
                            - base_entry.line_count as isize)
                            .unsigned_abs();
                        changed_lines += line_diff;
                    }
                } else if base_entry != now_entry {
                    let added_diff =
                        (now_entry.added as isize - base_entry.added as isize).unsigned_abs();
                    let deleted_diff =
                        (now_entry.deleted as isize - base_entry.deleted as isize).unsigned_abs();
                    if added_diff + deleted_diff > 0 {
                        changed_files += 1;
                        changed_lines += added_diff + deleted_diff;
                    }
                }
            }
        }
    }

    if changed_files > limit.files || changed_lines > limit.lines {
        let file_word = if changed_files == 1 { "file" } else { "files" };
        Some(format!(
            "Nirvana Claude edits at most {} file and {} lines itself (R2). This change is {} {}, {} lines. Hand it to a worker: `herdr seat pick --book`, then an order.",
            limit.files, limit.lines, changed_files, file_word, changed_lines
        ))
    } else {
        None
    }
}

pub fn capture_edit_state(root: &Path) -> EditState {
    capture_edit_state_cached(root, None)
}

pub fn capture_edit_state_cached(root: &Path, prev: Option<&EditState>) -> EditState {
    const MAX_PATHS: usize = 2000;
    const MAX_FILE_SIZE: u64 = 256 * 1024;
    const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

    let mut files = BTreeMap::new();
    let mut total_bytes: usize = 0;

    let numstat_out = std::process::Command::new("git")
        .args([
            "diff",
            "HEAD",
            "--numstat",
            "--relative",
            "--no-color",
            "--",
            ".",
        ])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["diff", "--numstat", "--relative", "--no-color", "--", "."])
                .current_dir(root)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success())
        });

    let mut tracked_paths: Vec<(String, usize, usize, bool)> = Vec::new();
    if let Some(out) = numstat_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let is_bin = parts[0] == "-" || parts[1] == "-";
                let added = parts[0].parse::<usize>().unwrap_or(0);
                let deleted = parts[1].parse::<usize>().unwrap_or(0);
                let path = parts[2].trim().trim_matches('"');
                let norm = path.replace('\\', "/");
                if !is_out_of_scope(&norm) {
                    tracked_paths.push((norm, added, deleted, is_bin));
                }
            }
        }
    }

    let ls_out = std::process::Command::new("git")
        .args(["ls-files", "-o", "--exclude-standard", "--", "."])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success());

    let mut untracked_paths = Vec::new();
    if let Some(out) = ls_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let path = line.trim().trim_matches('"');
            if path.is_empty() {
                continue;
            }
            let norm = path.replace('\\', "/");
            if !is_out_of_scope(&norm) {
                untracked_paths.push(norm);
            }
        }
    }

    if tracked_paths.len() + untracked_paths.len() > MAX_PATHS {
        return EditState {
            files: BTreeMap::new(),
            fail_open: true,
        };
    }

    for (norm, added, deleted, is_numstat_bin) in tracked_paths {
        let full_path = root.join(&norm);
        let meta = std::fs::metadata(&full_path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if is_numstat_bin {
            files.insert(
                norm,
                FileDelta {
                    is_tracked: true,
                    is_binary: true,
                    added: 0,
                    deleted: 0,
                    line_count: 0,
                    hash: None,
                    content: None,
                    size,
                    mtime,
                },
            );
            continue;
        }

        let mut is_bin = false;
        if let Ok(mut f) = std::fs::File::open(&full_path) {
            let mut buf = [0u8; 8192];
            if let Ok(n) = f.read(&mut buf) {
                if buf[..n].contains(&0) {
                    is_bin = true;
                }
            }
        }

        if is_bin {
            files.insert(
                norm,
                FileDelta {
                    is_tracked: true,
                    is_binary: true,
                    added: 0,
                    deleted: 0,
                    line_count: 0,
                    hash: None,
                    content: None,
                    size,
                    mtime,
                },
            );
            continue;
        }

        if size <= MAX_FILE_SIZE && total_bytes + (size as usize) <= MAX_TOTAL_BYTES {
            if let Ok(text) = std::fs::read_to_string(&full_path) {
                use sha2::{Digest, Sha256};
                let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
                let line_count = text.lines().count();
                total_bytes += text.len();
                files.insert(
                    norm,
                    FileDelta {
                        is_tracked: true,
                        is_binary: false,
                        added,
                        deleted,
                        line_count,
                        hash: Some(hash),
                        content: Some(text),
                        size,
                        mtime,
                    },
                );
                continue;
            }
        }

        if let Ok(bytes) = std::fs::read(&full_path) {
            use sha2::{Digest, Sha256};
            let hash = format!("{:x}", Sha256::digest(&bytes));
            let line_count = bytes.iter().filter(|&&b| b == b'\n').count();
            files.insert(
                norm,
                FileDelta {
                    is_tracked: true,
                    is_binary: false,
                    added,
                    deleted,
                    line_count,
                    hash: Some(hash),
                    content: None,
                    size,
                    mtime,
                },
            );
        }
    }

    for norm in untracked_paths {
        let full_path = root.join(&norm);
        let meta = std::fs::metadata(&full_path).ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(prev_entry) = prev.and_then(|p| p.files.get(&norm)) {
            if prev_entry.size == size && prev_entry.mtime == mtime {
                files.insert(norm, prev_entry.clone());
                continue;
            }
        }

        let mut is_bin = false;
        if let Ok(mut f) = std::fs::File::open(&full_path) {
            let mut buf = [0u8; 8192];
            if let Ok(n) = f.read(&mut buf) {
                if buf[..n].contains(&0) {
                    is_bin = true;
                }
            }
        }

        if is_bin {
            files.insert(
                norm,
                FileDelta {
                    is_tracked: false,
                    is_binary: true,
                    added: 0,
                    deleted: 0,
                    line_count: 0,
                    hash: None,
                    content: None,
                    size,
                    mtime,
                },
            );
            continue;
        }

        if size <= MAX_FILE_SIZE && total_bytes + (size as usize) <= MAX_TOTAL_BYTES {
            if let Ok(text) = std::fs::read_to_string(&full_path) {
                use sha2::{Digest, Sha256};
                let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
                let line_count = text.lines().count();
                total_bytes += text.len();
                files.insert(
                    norm,
                    FileDelta {
                        is_tracked: false,
                        is_binary: false,
                        added: line_count,
                        deleted: 0,
                        line_count,
                        hash: Some(hash),
                        content: Some(text),
                        size,
                        mtime,
                    },
                );
                continue;
            }
        }

        if let Ok(bytes) = std::fs::read(&full_path) {
            use sha2::{Digest, Sha256};
            let hash = format!("{:x}", Sha256::digest(&bytes));
            let line_count = bytes.iter().filter(|&&b| b == b'\n').count();
            files.insert(
                norm,
                FileDelta {
                    is_tracked: false,
                    is_binary: false,
                    added: line_count,
                    deleted: 0,
                    line_count,
                    hash: Some(hash),
                    content: None,
                    size,
                    mtime,
                },
            );
        }
    }

    EditState {
        files,
        fail_open: false,
    }
}

pub fn should_skip_orchestrator_hook(hook: &Value, cwd: Option<&Path>) -> bool {
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return true;
    }
    if std::env::var(crate::nirvana_launch::ORCHESTRATOR_ENV).as_deref() != Ok("1") {
        return true;
    }
    if let Some(cwd) = cwd {
        if cwd
            .to_string_lossy()
            .replace('\\', "/")
            .contains(".nirvana/worktrees")
        {
            return true;
        }
    }
    false
}

pub fn guard_edit_decision(
    hook: &Value,
    project_root: Option<&Path>,
    cwd: Option<&Path>,
    limit: EditLimit,
) -> Option<Value> {
    if should_skip_orchestrator_hook(hook, cwd) {
        return None;
    }
    let input = hook.get("tool_input").unwrap_or(&Value::Null);
    let file_path = input.get("file_path").and_then(Value::as_str);
    if let Some(path) = file_path {
        let rel = if let Some(root) = project_root {
            Path::new(path)
                .strip_prefix(root)
                .map(|p| p.to_str().unwrap_or(path))
                .unwrap_or(path)
        } else {
            path
        };
        if is_out_of_scope(rel) {
            return None;
        }
    }
    let tool_name = hook.get("tool_name").and_then(Value::as_str).unwrap_or("");
    let call_lines = match tool_name {
        "Write" => input
            .get("content")
            .and_then(Value::as_str)
            .map(|s| s.lines().count())
            .unwrap_or(0),
        "Edit" => input
            .get("new_string")
            .and_then(Value::as_str)
            .map(|s| s.lines().count())
            .unwrap_or(0),
        "MultiEdit" => input
            .get("edits")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.get("new_string").and_then(Value::as_str))
                    .map(|s| s.lines().count())
                    .sum()
            })
            .unwrap_or(0),
        _ => 0,
    };
    if call_lines > limit.lines {
        let reason = format!(
            "Nirvana Claude edits at most {} file and {} lines itself (R2). This change is 1 file, {} lines. Hand it to a worker: `herdr seat pick --book`, then an order.",
            limit.files, limit.lines, call_lines
        );
        Some(json!({
            "decision": "block",
            "reason": reason,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        }))
    } else {
        None
    }
}

pub fn check_edit_decision(
    hook: &Value,
    cwd: Option<&Path>,
    baseline: &EditState,
    now_state: &EditState,
    limit: EditLimit,
) -> Option<Value> {
    if should_skip_orchestrator_hook(hook, cwd) {
        return None;
    }
    edit_budget(baseline, now_state, limit).map(|reason| {
        json!({
            "decision": "block",
            "reason": reason,
        })
    })
}

pub fn prompt_baseline_from(hook: &Value) -> std::io::Result<()> {
    let cwd = hook_cwd(hook).or_else(|| std::env::current_dir().ok());
    if should_skip_orchestrator_hook(hook, cwd.as_deref()) {
        return Ok(());
    }
    let Some(session_id) = hook.get("session_id").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(project) = project_at(cwd.as_deref()) else {
        return Ok(());
    };
    let state = capture_edit_state(&project.root);
    if state.fail_open {
        return Ok(());
    }
    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{session_id}.json"));
    let _ = nirvana_seat::write_atomic(&path, serde_json::to_string(&state)?.as_bytes());
    Ok(())
}

fn prompt_baseline_cli() -> std::io::Result<()> {
    prompt_baseline_from(&read_stdin())
}

fn guard_edit_cli() -> std::io::Result<()> {
    let hook = read_stdin();
    let cwd = hook_cwd(&hook).or_else(|| std::env::current_dir().ok());
    let project = project_at(cwd.as_deref());
    if let Some(decision) = guard_edit_decision(
        &hook,
        project.as_ref().map(|p| p.root.as_path()),
        cwd.as_deref(),
        EditLimit::default(),
    ) {
        println!("{decision}");
    }
    Ok(())
}

pub fn check_edit_from(hook: &Value) -> std::io::Result<Option<Value>> {
    let cwd = hook_cwd(hook).or_else(|| std::env::current_dir().ok());
    if should_skip_orchestrator_hook(hook, cwd.as_deref()) {
        return Ok(None);
    }
    let Some(session_id) = hook.get("session_id").and_then(Value::as_str) else {
        return Ok(None);
    };
    let Some(project) = project_at(cwd.as_deref()) else {
        return Ok(None);
    };
    let dir = home_dir().join(nirvana_seat::DIR).join("edit-baselines");
    let path = dir.join(format!("{session_id}.json"));
    let baseline: Option<EditState> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok());

    let now_state = capture_edit_state(&project.root);
    if now_state.fail_open {
        return Ok(None);
    }

    let baseline = match baseline {
        Some(b) => b,
        None => {
            // B1: When check-edit finds no baseline for its session, it writes one
            // from the current state and returns without blocking.
            let _ = std::fs::create_dir_all(&dir);
            let _ =
                nirvana_seat::write_atomic(&path, serde_json::to_string(&now_state)?.as_bytes());
            return Ok(None);
        }
    };

    Ok(check_edit_decision(
        hook,
        cwd.as_deref(),
        &baseline,
        &now_state,
        EditLimit::default(),
    ))
}

fn check_edit_cli() -> std::io::Result<()> {
    if let Some(decision) = check_edit_from(&read_stdin())? {
        println!("{decision}");
    }
    Ok(())
}

// ---- the CLI ----

pub const SUBCOMMANDS: [&str; 9] = [
    "digest",
    "guard-spawn",
    "subagent-stop",
    "stop",
    "statusline",
    "log-usage",
    "prompt-baseline",
    "guard-edit",
    "check-edit",
];

/// `herdr nirvana <digest|guard-spawn|subagent-stop|stop|statusline|log-usage|prompt-baseline|guard-edit|check-edit>`, each reading Claude Code's JSON on stdin.
/// `None` when `name` is not one of these.
pub fn cli(name: &str) -> Option<std::io::Result<()>> {
    Some(match name {
        "digest" => digest_cli(),
        "guard-spawn" => guard_cli(),
        "subagent-stop" => subagent_stop_cli(),
        "stop" => stop_cli(),
        "statusline" => statusline_cli(),
        "log-usage" => log_usage_cli(),
        "prompt-baseline" => prompt_baseline_cli(),
        "guard-edit" => guard_edit_cli(),
        "check-edit" => check_edit_cli(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
