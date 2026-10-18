//! Nirvana fork, plan 12 O5: the retro. `herdr run retro <run>`, which `herdr run end` runs; its gap ledger; its
//! proposals, which only the owner applies. (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §7.4.2;
//! old Nirvana's `nirvanad/improve/retro.py`)
//!
//! The owner, 2026-10-17: "One other thing to consider is a form of run receipt Nirvana had this for checking how
//! the agents ran and how it can improve." (plan 12 L11)
//!
//! - **Gaps come from the journal**, and no model is called: a send-back and its reason, a rate-limit stop, a stall,
//!   a re-seat, an override, no seat left, a worker that failed, a gotcha a Work Report gave. Each carries a
//!   verbatim quote: the journal line it came from.
//! - **The ledger** is `<project>/.nirvana/retro/gaps.jsonl`, one row per (gap, run), so three occurrences in one
//!   noisy run are one run's evidence. A gap said two ways is one gap when the Sørensen-Dice score of their
//!   character bigrams is at least 0.6: old Nirvana measured 0.90 for one gap phrased twice and 0.18 for two
//!   different gaps (retro.py, 2026-09-15), and 0.6 sits between.
//! - **A gap graduates** after 3 distinct runs, or after 1 outcome-tagged miss with a clear cause (old design spec
//!   §11.1): the send-back cap spent on a named axis, a crash loop, a worker that failed with its reason.
//! - **A proposal** is `<project>/.nirvana/proposals/<id>.md` in the CHANGELOG shape, quoting journal lines
//!   verbatim. A quote not found in its journal line is refused: a retro that could cite its own summary as
//!   evidence would hold an assertion whose expectation and subject are one source.
//! - **Never a gate, a decision, CLAUDE.md or a seats.toml constant.** An expected-use cell of seats.toml may be
//!   proposed, for the owner to apply. Nothing here writes any target: there is no apply.
//! - **A refusal stays.** `herdr run retro reject <id>` writes `.nirvana/retro/rejected.jsonl`; a refused gap
//!   never graduates again, whatever later evidence arrives (old Nirvana, from tigerless's `ledger.py`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::nirvana_run::{self, now, parse_lines, replay, Kind, Project};
use crate::nirvana_seat::{self, RecordLine};

/// Old Nirvana's measured threshold (the module comment).
pub const DICE: f64 = 0.6;
/// Old design spec §11.1: ">= 3 occurrences", counted as distinct runs.
pub const RUNS_TO_GRADUATE: usize = 3;
/// A quote is the journal line, cut to this many characters: a prefix of a line is still verbatim in it.
const QUOTE_CHARS: usize = 400;

// ---- Sørensen-Dice on character bigrams (old retro.py's `dice`) ----

pub fn normalise(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn bigrams(text: &str) -> BTreeMap<(char, char), usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = BTreeMap::new();
    for pair in chars.windows(2) {
        *out.entry((pair[0], pair[1])).or_insert(0) += 1;
    }
    out
}

pub fn dice(a: &str, b: &str) -> f64 {
    let (a, b) = (normalise(a), normalise(b));
    let (x, y) = (bigrams(&a), bigrams(&b));
    let total: usize = x.values().sum::<usize>() + y.values().sum::<usize>();
    if total == 0 {
        return if a == b { 1.0 } else { 0.0 };
    }
    let shared: usize = x
        .iter()
        .map(|(pair, n)| (*n).min(*y.get(pair).unwrap_or(&0)))
        .sum();
    2.0 * shared as f64 / total as f64
}

/// FNV-1a: a stable key from text, the same in every process (herdr carries no hash crate).
fn hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")[..12].to_owned()
}

pub fn gap_key(text: &str) -> String {
    format!("g-{}", hash(&normalise(text)))
}

// ---- gaps from a journal ----

/// One gap found in one run's journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    /// send_back, send_back_cap, rate_limit, stall, crash_loop, reseat, override, no_seat, failed, gotcha.
    pub kind: String,
    pub gap: String,
    pub seq: u64,
    /// The journal line, verbatim (cut to `QUOTE_CHARS`).
    pub quote: String,
    pub outcome_tagged: bool,
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub seat: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    /// The gate axis a send-back named.
    #[serde(default)]
    pub axis: Option<String>,
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The raw line of each `seq`: quotes are taken from these and checked against them.
pub fn raw_lines(text: &str) -> BTreeMap<u64, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let seq = serde_json::from_str::<Value>(line.trim())
                .ok()?
                .get("seq")?
                .as_u64()?;
            Some((seq, line.trim().to_owned()))
        })
        .collect()
}

/// The gaps one run's journal shows. Pure: the same journal gives the same gaps.
pub fn gaps(text: &str) -> Vec<Found> {
    let (events, _) = parse_lines(text);
    let raw = raw_lines(text);
    let state = replay(&events);
    let mut found = Vec::new();
    for event in &events {
        let quote = raw
            .get(&event.seq)
            .map(|line| cut(line, QUOTE_CHARS))
            .unwrap_or_default();
        let seated = event
            .kind
            .order()
            .and_then(|order| state.order(order))
            .map(|known| &known.seated);
        let mut push = |kind: &str, gap: String, tagged: bool, axis: Option<String>| {
            found.push(Found {
                kind: kind.to_owned(),
                gap: one_line(&gap),
                seq: event.seq,
                quote: quote.clone(),
                outcome_tagged: tagged,
                class: seated.map(|s| s.class.clone()),
                seat: seated.map(|s| s.seat.clone()),
                effort: seated.map(|s| s.effort.word().to_owned()),
                axis,
            });
        };
        let class = seated.map_or("an", |s| s.class.as_str());
        match &event.kind {
            Kind::Gated {
                verdict, reason, ..
            } if verdict == "sent_back" || verdict == "asked" => {
                let reason = reason.clone().unwrap_or_else(|| "no reason given".into());
                let axis = reason.split(':').next().map(str::trim).map(str::to_owned);
                if verdict == "asked" {
                    push(
                        "send_back_cap",
                        format!("the gate spent its send-backs on a {class} order: {reason}"),
                        true,
                        axis,
                    );
                } else {
                    push(
                        "send_back",
                        format!("the gate sent a {class} order back on {reason}"),
                        false,
                        axis,
                    );
                }
            }
            Kind::RateLimited {
                pool,
                window,
                signal,
                ..
            } => push(
                "rate_limit",
                format!(
                    "a worker ran out of pool {} window {} ({signal})",
                    pool.as_deref().unwrap_or("?"),
                    window.as_deref().unwrap_or("every window")
                ),
                false,
                None,
            ),
            Kind::Stalled { next, .. } => {
                let seat = seated.map_or("a", |s| s.seat.as_str());
                if next == "refused" {
                    push(
                        "crash_loop",
                        format!("a {seat} worker on a {class} order stalled into a crash loop"),
                        true,
                        None,
                    );
                } else {
                    push(
                        "stall",
                        format!("a {seat} worker on a {class} order went silent for 10 minutes"),
                        false,
                        None,
                    );
                }
            }
            Kind::Reseated { .. } => {
                let seat = seated.map_or("a", |s| s.seat.as_str());
                push(
                    "reseat",
                    format!("a {class} order on {seat} was re-seated when its pool ran out"),
                    false,
                    None,
                );
            }
            Kind::Seated(seat) => {
                if let Some(why) = &seat.override_reason {
                    push(
                        "override",
                        format!(
                            "the orchestrator overrode the rule for a {} order to {}: {why}",
                            seat.class, seat.seat
                        ),
                        false,
                        None,
                    );
                }
            }
            Kind::Asked { question, .. }
                if question.contains("no other seat has room") || question.contains("no seat") =>
            {
                push(
                    "no_seat",
                    format!("no seat had room for a {class} order"),
                    false,
                    None,
                );
            }
            Kind::Ended {
                order: Some(_),
                outcome,
                why: Some(why),
                ..
            } if outcome == "failed" => {
                let seat = seated.map_or("a", |s| s.seat.as_str());
                push(
                    "failed",
                    format!("a {seat} worker failed: {}", cut(why, 160)),
                    true,
                    None,
                );
            }
            Kind::Delivered { report, .. } => {
                for gotcha in report
                    .get("gotchas")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(text) = gotcha.as_str().filter(|text| !text.trim().is_empty()) {
                        push(
                            "gotcha",
                            format!("a worker's gotcha: {}", cut(text, 200)),
                            false,
                            None,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    found
}

// ---- the ledger ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub v: u32,
    pub key: String,
    pub run: String,
    pub at: u64,
    #[serde(flatten)]
    pub found: Found,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub key: String,
    #[serde(default)]
    pub proposal: Option<String>,
    pub why: String,
    pub at: u64,
}

pub fn retro_dir(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("retro")
}

pub fn proposals_dir(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join("proposals")
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line.trim()).ok())
        .collect()
}

fn append_jsonl<T: Serialize>(path: &Path, rows: &[T]) -> std::io::Result<()> {
    use std::io::Write;
    if rows.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for row in rows {
        text.push_str(&serde_json::to_string(row)?);
        text.push('\n');
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

pub fn ledger(root: &Path) -> Vec<Row> {
    read_jsonl(&retro_dir(root).join("gaps.jsonl"))
}

pub fn rejected(root: &Path) -> Vec<Rejected> {
    read_jsonl(&retro_dir(root).join("rejected.jsonl"))
}

/// The key a gap belongs to: the best known gap at or above `DICE`, else its own.
pub fn merge_key(rows: &[Row], text: &str) -> String {
    let mut best: Option<(f64, &str)> = None;
    for row in rows {
        let score = dice(text, &row.found.gap);
        if score >= DICE && best.is_none_or(|(known, _)| score > known) {
            best = Some((score, row.key.as_str()));
        }
    }
    best.map_or_else(|| gap_key(text), |(_, key)| key.to_owned())
}

/// Whether a gap's key is refused: by key, or because a refused gap reads the same (Dice at the threshold), so a
/// gap said a new way cannot slip past a refusal.
pub fn is_rejected(rows: &[Row], refused: &[Rejected], key: &str) -> bool {
    if refused.iter().any(|r| r.key == key) {
        return true;
    }
    let texts: Vec<&str> = rows
        .iter()
        .filter(|row| row.key == key)
        .map(|row| row.found.gap.as_str())
        .collect();
    refused.iter().any(|r| {
        rows.iter()
            .filter(|row| row.key == r.key)
            .any(|row| texts.iter().any(|text| dice(text, &row.found.gap) >= DICE))
    })
}

/// A gap graduates after `RUNS_TO_GRADUATE` distinct runs, or one outcome-tagged miss; never once refused.
pub fn graduated(rows: &[Row], refused: &[Rejected], key: &str) -> bool {
    if is_rejected(rows, refused, key) {
        return false;
    }
    let of_key: Vec<&Row> = rows.iter().filter(|row| row.key == key).collect();
    let runs: BTreeSet<&str> = of_key.iter().map(|row| row.run.as_str()).collect();
    runs.len() >= RUNS_TO_GRADUATE || of_key.iter().any(|row| row.found.outcome_tagged)
}

// ---- scope ----

/// The targets a proposal may name, relative to the Nirvana Claude plugin (`<home>/nirvana/claude/`) or
/// `seats.toml:seat.<id>.use.<effort>`; the refusals first, each in the words of the rule it is.
pub fn check_target(target: &str) -> Result<String, String> {
    let raw = target.trim().replace('\\', "/");
    if raw.is_empty() {
        return Err("a proposal names a target".into());
    }
    if raw.starts_with('/') || raw.chars().nth(1) == Some(':') {
        return Err(format!(
            "an absolute path is outside the retro's surface: {raw}"
        ));
    }
    let (path, cell) = raw
        .split_once(':')
        .map_or((raw.as_str(), None), |(p, c)| (p, Some(c)));
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.contains(&"..") {
        return Err(format!(
            "a `..` segment is outside the retro's surface: {raw}"
        ));
    }
    if parts
        .iter()
        .any(|part| part.eq_ignore_ascii_case("CLAUDE.md"))
    {
        return Err(format!(
            "CLAUDE.md is the rule text; a rule change is the owner's: {raw}"
        ));
    }
    if parts
        .iter()
        .any(|part| part.eq_ignore_ascii_case("decisions"))
    {
        return Err(format!("a decision is the owner's call: {raw}"));
    }
    if parts.iter().any(|part| {
        let lower = part.to_ascii_lowercase();
        lower == "gate" || lower == "gates" || lower.contains("nirvana_gate")
    }) {
        return Err(format!(
            "the gate's rules are the owner's; a proposal may not touch a gate: {raw}"
        ));
    }
    if parts
        .last()
        .is_some_and(|last| last.eq_ignore_ascii_case("seats.toml"))
    {
        let ok = parts.len() == 1
            && cell.is_some_and(|cell| {
                let words: Vec<&str> = cell.split('.').collect();
                matches!(words.as_slice(), ["seat", id, "use", effort] if !id.is_empty() && !effort.is_empty())
            });
        return if ok {
            Ok(raw)
        } else {
            Err(format!(
                "seats.toml's constants, bars and scores are the owner's; only an expected-use cell \
`seats.toml:seat.<id>.use.<effort>` may be proposed: {raw}"
            ))
        };
    }
    if cell.is_some() {
        return Err(format!("only seats.toml takes a `:cell`: {raw}"));
    }
    match parts.first() {
        Some(&"skills" | &"agents") if parts.len() >= 2 => Ok(raw),
        _ => Err(format!(
            "a proposal targets the Nirvana Claude plugin's skills/ or agents/, or a seats.toml expected-use cell: {raw}"
        )),
    }
}

// ---- proposals ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub run: String,
    pub seq: u64,
    pub quote: String,
}

/// A proposal in the CHANGELOG shape (Nirvana CLAUDE.md rule 3). `herdr run retro propose --json` takes this
/// object; unknown keys are refused by name, so a misspelt `evidences` cannot open a proposal with none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub gap: String,
    pub target: String,
    pub changed: String,
    pub because: String,
    pub measured: String,
    pub if_you_remove: String,
    pub evidence: Vec<Evidence>,
}

/// Every quote must be verbatim in the journal line it names.
pub fn check_quotes(project: &Project, proposal: &Proposal) -> Result<(), String> {
    if proposal.evidence.is_empty() {
        return Err("a proposal carries no verbatim journal line".into());
    }
    for item in &proposal.evidence {
        if item.quote.trim().is_empty() {
            return Err(format!(
                "an empty quote for {} seq {} is not evidence",
                item.run, item.seq
            ));
        }
        if !nirvana_run::valid_id(&item.run) {
            return Err(format!("{:?} is not a run id", item.run));
        }
        let text = std::fs::read_to_string(project.journal(&item.run).path).unwrap_or_default();
        match raw_lines(&text).get(&item.seq) {
            None => return Err(format!("run {} has no journal line {}", item.run, item.seq)),
            Some(line) if !line.contains(item.quote.as_str()) => {
                return Err(format!(
                    "the quote is not in run {}'s journal line {}: {:?}",
                    item.run,
                    item.seq,
                    cut(&item.quote, 80)
                ))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

pub fn proposal_id(gap_key: &str) -> String {
    format!("p-{}", hash(gap_key))
}

fn proposal_path(root: &Path, id: &str) -> PathBuf {
    proposals_dir(root).join(format!("{id}.md"))
}

pub fn proposal_text(id: &str, key: &str, status: &str, proposal: &Proposal) -> String {
    let mut text = format!(
        "# Proposal {id}: {}\n\nstatus: {status}\ngap: {key}\ntarget: {}\n\nNothing applies this file. The owner reads it, \
applies it by hand or not at all, or refuses it: `herdr run retro reject {id}`.\n\n",
        one_line(&proposal.gap),
        proposal.target
    );
    for (head, body) in [
        ("Changed", &proposal.changed),
        ("Because", &proposal.because),
        ("Measured", &proposal.measured),
        ("If you remove this", &proposal.if_you_remove),
    ] {
        text.push_str(&format!("## {head}\n\n{}\n\n", body.trim()));
    }
    text.push_str("## Evidence, verbatim from the journals\n\n");
    for item in &proposal.evidence {
        text.push_str(&format!(
            "- run `{}` line {}:\n\n  ```json\n  {}\n  ```\n",
            item.run, item.seq, item.quote
        ));
    }
    text
}

/// Opens one proposal: scope, then the refusal, then the quotes, and only then a file.
pub fn propose(
    project: &Project,
    proposal: &Proposal,
    key: Option<&str>,
) -> Result<(String, PathBuf), String> {
    let target = check_target(&proposal.target)?;
    let rows = ledger(&project.root);
    let refused = rejected(&project.root);
    let key = key.map_or_else(|| merge_key(&rows, &proposal.gap), str::to_owned);
    let mut probe = rows.clone();
    probe.push(Row {
        v: 1,
        key: key.clone(),
        run: String::new(),
        at: 0,
        found: Found {
            kind: "proposal".into(),
            gap: proposal.gap.clone(),
            seq: 0,
            quote: String::new(),
            outcome_tagged: false,
            class: None,
            seat: None,
            effort: None,
            axis: None,
        },
    });
    if is_rejected(&probe, &refused, &key) {
        return Err(format!(
            "the owner refused this gap ({key}); a refusal stays"
        ));
    }
    check_quotes(project, proposal)?;
    let id = proposal_id(&key);
    let path = proposal_path(&project.root, &id);
    if path.exists() {
        return Ok((id, path));
    }
    std::fs::create_dir_all(proposals_dir(&project.root)).map_err(|err| err.to_string())?;
    let proposal = Proposal {
        target,
        ..proposal.clone()
    };
    std::fs::write(&path, proposal_text(&id, &key, "open", &proposal))
        .map_err(|err| err.to_string())?;
    Ok((id, path))
}

fn hint(axis: &str) -> &'static str {
    match axis {
        "tests" => "a `test:` command and the `check:` names it prints",
        "owned" => "every file the worker may change, in `owns:` lines",
        "report" => "that the worker ends with a Work Report whose status is done",
        "rebase" => "which orders touch the same files, so they run one after another",
        "heldout" => "the request's edge cases, which a held-out check tests",
        "review" => "what a reviewer of another family will hold the change to",
        "served" => "the `serves:` port and how the worker starts its server in its own pane",
        _ => "what the gate checks",
    }
}

/// The measured use of a seat at an effort, from the record: the mean share of each window over its lines.
pub fn measured_use(record: &[RecordLine], seat: &str, effort: &str) -> Option<String> {
    let lines: Vec<&BTreeMap<String, f32>> = record
        .iter()
        .filter(|line| line.seat == seat && line.effort.word() == effort)
        .filter_map(|line| line.spent.as_ref())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let mut sums: BTreeMap<&str, f64> = BTreeMap::new();
    for spent in &lines {
        for (window, share) in spent.iter() {
            *sums.entry(window.as_str()).or_insert(0.0) += f64::from(*share);
        }
    }
    Some(format!(
        "{} (the mean of {} record line(s))",
        sums.iter()
            .map(|(window, sum)| format!("{window} {:.4}%", sum / lines.len() as f64 * 100.0))
            .collect::<Vec<_>>()
            .join(", "),
        lines.len()
    ))
}

/// The proposal a graduated gap makes: deterministic text per kind, with every occurrence quoted.
pub fn draft(key: &str, rows: &[&Row], record: &[RecordLine]) -> Proposal {
    let first = &rows[0].found;
    let runs: BTreeSet<&str> = rows.iter().map(|row| row.run.as_str()).collect();
    let class = first.class.clone().unwrap_or_else(|| "these".into());
    let seat = first.seat.clone().unwrap_or_else(|| "?".into());
    let effort = first.effort.clone().unwrap_or_else(|| "?".into());
    let tagged = rows.iter().any(|row| row.found.outcome_tagged);
    let (target, changed, remove) = match first.kind.as_str() {
        "send_back" | "send_back_cap" => {
            let axis = first.axis.clone().unwrap_or_default();
            (
                "skills/seat/SKILL.md".to_owned(),
                format!(
                    "Tell the orchestrator to state in every {class} order {}, which the gate's {axis} axis checks.",
                    hint(&axis)
                ),
                format!("{class} orders keep going back on {axis}, each send-back a new paid session."),
            )
        }
        "rate_limit" | "reseat" => (
            format!("seats.toml:seat.{seat}.use.{effort}"),
            format!(
                "Set {seat} {effort}'s expected use to what its orders spent: {}.",
                measured_use(record, &seat, &effort).unwrap_or_else(|| "not yet measured in the record".into())
            ),
            format!("the rule keeps seating on {seat} {effort} with less headroom than its orders spend."),
        ),
        "stall" | "crash_loop" => (
            "skills/seat/SKILL.md".to_owned(),
            format!("Split {class} orders seated on {seat} smaller, or name in the order the step that hangs."),
            format!("{seat} workers on {class} orders keep going silent and restarting."),
        ),
        "override" => (
            "skills/seat/SKILL.md".to_owned(),
            format!("Say when a {class} order may be overridden off the rule's seat, and when not."),
            "overrides keep spending the run's 3 without a stated reason.".to_owned(),
        ),
        "no_seat" => (
            "skills/usage/SKILL.md".to_owned(),
            format!("Seat {class} orders while a pool's window has room, or defer them before it runs out."),
            format!("{class} orders keep finding no seat."),
        ),
        "gotcha" => (
            "skills/seat/SKILL.md".to_owned(),
            format!("Add to {class} orders: {}", first.gap.trim_start_matches("a worker's gotcha: ")),
            format!("the next {class} worker meets the same gotcha."),
        ),
        _ => (
            "skills/seat/SKILL.md".to_owned(),
            format!("Name in {class} orders what made the worker fail: {}", first.gap),
            format!("{class} workers keep failing the same way."),
        ),
    };
    Proposal {
        gap: first.gap.clone(),
        target,
        changed,
        because: format!(
            "The retro found this gap ({key}) in {} distinct run(s){}; plan 12 §7.4.2 graduates a gap at {RUNS_TO_GRADUATE} \
runs or one outcome-tagged miss.",
            runs.len(),
            if tagged { ", once as a miss with a clear cause" } else { "" }
        ),
        measured: rows
            .iter()
            .map(|row| format!("run {} line {}: {}", row.run, row.found.seq, row.found.gap))
            .collect::<Vec<_>>()
            .join("\n"),
        if_you_remove: remove,
        evidence: rows
            .iter()
            .map(|row| Evidence {
                run: row.run.clone(),
                seq: row.found.seq,
                quote: row.found.quote.clone(),
            })
            .collect(),
    }
}

/// What one retro did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub run: String,
    pub found: usize,
    pub recorded: usize,
    pub proposed: Vec<String>,
    pub refused: Vec<String>,
}

/// Scans one run, records its gaps once per (gap, run), and opens a proposal for each gap that graduates.
pub fn retro_with(project: &Project, run: &str, record: &[RecordLine]) -> Result<Summary, String> {
    let journal = project.journal(run);
    let text = std::fs::read_to_string(&journal.path)
        .map_err(|err| format!("{}: {err}", journal.path.display()))?;
    let found = gaps(&text);
    let mut rows = ledger(&project.root);
    let mut new_rows = Vec::new();
    let mut touched = BTreeSet::new();
    for gap in &found {
        let mut known = rows.clone();
        known.extend(new_rows.iter().cloned());
        let key = merge_key(&known, &gap.gap);
        touched.insert(key.clone());
        if known.iter().any(|row| row.key == key && row.run == run) {
            continue;
        }
        new_rows.push(Row {
            v: 1,
            key,
            run: run.to_owned(),
            at: now(),
            found: gap.clone(),
        });
    }
    append_jsonl(&retro_dir(&project.root).join("gaps.jsonl"), &new_rows)
        .map_err(|err| err.to_string())?;
    rows.extend(new_rows.iter().cloned());
    let refused = rejected(&project.root);
    let mut summary = Summary {
        run: run.to_owned(),
        found: found.len(),
        recorded: new_rows.len(),
        ..Summary::default()
    };
    for key in touched {
        if !graduated(&rows, &refused, &key)
            || proposal_path(&project.root, &proposal_id(&key)).exists()
        {
            continue;
        }
        let of_key: Vec<&Row> = rows.iter().filter(|row| row.key == key).collect();
        match propose(project, &draft(&key, &of_key, record), Some(&key)) {
            Ok((id, _)) => summary.proposed.push(id),
            Err(why) => summary.refused.push(format!("{key}: {why}")),
        }
    }
    Ok(summary)
}

pub fn retro(project: &Project, run: &str) -> Result<String, String> {
    let record = nirvana_seat::read_record(&nirvana_seat::dir().join(nirvana_seat::RECORD_FILE));
    let summary = retro_with(project, run, &record)?;
    Ok(summary_line(&summary))
}

fn summary_line(summary: &Summary) -> String {
    let mut line = format!(
        "retro {}: {} gap(s), {} new in the ledger",
        summary.run, summary.found, summary.recorded
    );
    if !summary.proposed.is_empty() {
        line.push_str(&format!("; proposed {}", summary.proposed.join(", ")));
    }
    for refused in &summary.refused {
        line.push_str(&format!("; refused {refused}"));
    }
    line
}

/// The open proposals: id and gap, from each file's own status line.
pub fn open(project: &Project) -> Vec<(String, String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(proposals_dir(&project.root)) else {
        return out;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if !text.lines().any(|line| line == "status: open") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        let gap = text
            .lines()
            .next()
            .and_then(|line| line.split_once(": "))
            .map(|(_, gap)| gap.to_owned())
            .unwrap_or_default();
        out.push((id, gap, path));
    }
    out
}

pub fn open_ids(project: &Project) -> Vec<String> {
    open(project).into_iter().map(|(id, _, _)| id).collect()
}

/// The receipt's section; empty when nothing is open.
pub fn open_section(project: &Project) -> String {
    let open = open(project);
    if open.is_empty() {
        return String::new();
    }
    let mut text = format!("\n## Open retro proposals ({})\n\n", open.len());
    for (id, gap, path) in open {
        text.push_str(&format!("- {id}: {gap} ({})\n", path.display()));
    }
    text
}

/// `herdr run retro reject <id>`: by proposal id or gap key. The file's status turns `rejected`; the ledger keeps
/// the refusal.
pub fn reject(project: &Project, id: &str, why: &str) -> Result<String, String> {
    let (key, proposal) = if id.starts_with("g-") {
        (id.to_owned(), None)
    } else {
        let path = proposal_path(&project.root, id);
        let text = std::fs::read_to_string(&path).map_err(|_| format!("no proposal {id}"))?;
        let key = text
            .lines()
            .find_map(|line| line.strip_prefix("gap: "))
            .ok_or(format!("{id} names no gap"))?
            .to_owned();
        let rewritten: Vec<String> = text
            .lines()
            .map(|line| {
                if line == "status: open" {
                    "status: rejected".to_owned()
                } else {
                    line.to_owned()
                }
            })
            .collect();
        std::fs::write(&path, rewritten.join("\n") + "\n").map_err(|err| err.to_string())?;
        (key, Some(id.to_owned()))
    };
    append_jsonl(
        &retro_dir(&project.root).join("rejected.jsonl"),
        &[Rejected {
            key: key.clone(),
            proposal,
            why: why.to_owned(),
            at: now(),
        }],
    )
    .map_err(|err| err.to_string())?;
    Ok(format!(
        "{id} rejected ({key}); no later evidence opens it again"
    ))
}

pub fn cli(project: &Project, args: &[String]) -> std::io::Result<()> {
    let fail = |why: String| -> ! {
        eprintln!("herdr run retro: {why}");
        std::process::exit(2);
    };
    match args.first().map(String::as_str) {
        Some("reject") => {
            let Some(id) = args.get(1) else {
                fail("name the proposal".into())
            };
            let why = match args.get(2..) {
                Some([flag, why]) if flag == "--why" => why.clone(),
                Some([]) | None => "refused by the owner".into(),
                _ => fail("usage: herdr run retro reject <id> [--why text]".into()),
            };
            match reject(project, id, &why) {
                Ok(line) => println!("{line}"),
                Err(why) => fail(why),
            }
        }
        Some("propose") => {
            let (Some(flag), Some(text)) = (args.get(1), args.get(2)) else {
                fail("usage: herdr run retro propose --json '<proposal>'".into())
            };
            if flag != "--json" {
                fail("usage: herdr run retro propose --json '<proposal>'".into());
            }
            let proposal: Proposal =
                serde_json::from_str(text).unwrap_or_else(|err| fail(format!("refused: {err}")));
            match propose(project, &proposal, None) {
                Ok((id, path)) => println!("{id}: {} (open; nothing was applied)", path.display()),
                Err(why) => fail(format!("refused: {why}")),
            }
        }
        Some("list") => {
            for (id, gap, path) in open(project) {
                println!("{id}: {gap} ({})", path.display());
            }
        }
        Some(run) if nirvana_run::valid_id(run) => match retro(project, run) {
            Ok(line) => println!("{line}"),
            Err(why) => fail(why),
        },
        _ => fail(
            "usage: herdr run retro <run> | reject <id> [--why t] | propose --json '<p>' | list"
                .into(),
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nirvana_run::{Journal, Seated};
    use crate::nirvana_seat::{Effort, Mode, SizeName};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "herdr-o5-retro-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn seated(order: &str) -> Seated {
        Seated {
            order: order.into(),
            seat: "sonnet".into(),
            cli: "claude".into(),
            model: "claude-sonnet-5-5".into(),
            effort: Effort::Medium,
            pool: "max".into(),
            class: "build.bounded".into(),
            chain: Some("build.bounded".into()),
            size: SizeName::Small,
            mode: Mode::Balanced,
            needs: Vec::new(),
            not_family: Vec::new(),
            reason: "build.bounded → sonnet medium".into(),
            override_reason: None,
            order_file: format!("C:/p/.nirvana/orders/{order}.md"),
            cwd: "C:/p".into(),
            worktree: None,
            branch: None,
            parent: None,
            why: None,
            rule_pick: None,
            bar_dropped: None,
            step_down: None,
            use_basis: None,
            expected: BTreeMap::new(),
        }
    }

    /// A recorded run: one order sent back because its named check never ran, said the way each run says it.
    fn journal(project: &Project, run: &str, reason: &str) -> Journal {
        let journal = project.journal(run);
        journal
            .append(100, Kind::Seated(Box::new(seated("a"))))
            .expect("seated");
        journal
            .append(
                160,
                Kind::Gated {
                    order: "a".into(),
                    verdict: "sent_back".into(),
                    reason: Some(reason.into()),
                    axes: Vec::new(),
                },
            )
            .expect("gated");
        journal
    }

    const REASONS: [&str; 4] = [
        "tests: the named check(s) test_mul never ran: a suite total cannot see a check that never ran",
        "tests: the named check(s) test_add never ran: a suite total cannot see a check that never ran",
        "tests: the named check(s) test_div never ran: a suite total cannot see a check that never ran",
        "tests: the named check(s) test_sub never ran: a suite total cannot see a check that never ran",
    ];

    #[test]
    fn dice_scores_one_gap_said_two_ways_above_the_line_and_two_gaps_below() {
        assert!(
            dice(REASONS[0], REASONS[1]) >= DICE,
            "{}",
            dice(REASONS[0], REASONS[1])
        );
        let other = "a sonnet worker on a build.bounded order went silent for 10 minutes";
        assert!(
            dice(REASONS[0], other) < DICE,
            "{}",
            dice(REASONS[0], other)
        );
        assert_eq!(dice("abc", "abc"), 1.0);
    }

    #[test]
    fn a_gap_in_three_distinct_runs_graduates_and_in_two_does_not() {
        let root = temp("grad");
        let project = Project::at(&root);
        for (run, reason) in ["r1", "r2"].iter().zip(REASONS) {
            journal(&project, run, reason);
            let summary = retro_with(&project, run, &[]).expect("retro");
            assert!(summary.proposed.is_empty(), "{run}: {summary:?}");
        }
        // Running a run's retro again records nothing new: one row per (gap, run).
        let again = retro_with(&project, "r2", &[]).expect("again");
        assert_eq!((again.recorded, again.proposed.len()), (0, 0));
        assert_eq!(ledger(&root).len(), 2);
        journal(&project, "r3", REASONS[2]);
        let third = retro_with(&project, "r3", &[]).expect("third");
        assert_eq!(third.proposed.len(), 1, "{third:?}");
        let (id, _, path) = open(&project).pop().expect("open");
        assert_eq!(id, third.proposed[0]);
        let text = std::fs::read_to_string(path).expect("text");
        for head in [
            "## Changed",
            "## Because",
            "## Measured",
            "## If you remove this",
            "status: open",
        ] {
            assert!(text.contains(head), "{head}: {text}");
        }
        assert!(text.contains("target: skills/seat/SKILL.md"));
        // The quotes are the journal lines, verbatim.
        let line =
            raw_lines(&std::fs::read_to_string(project.journal("r3").path).expect("j"))[&2].clone();
        assert!(text.contains(&line), "{text}");
        assert!(open_section(&project).contains(&id));
    }

    #[test]
    fn one_outcome_tagged_miss_with_a_clear_cause_graduates_alone() {
        let root = temp("tagged");
        let project = Project::at(&root);
        let journal = project.journal("r1");
        journal
            .append(100, Kind::Seated(Box::new(seated("a"))))
            .expect("seated");
        journal
            .append(
                160,
                Kind::Gated {
                    order: "a".into(),
                    verdict: "asked".into(),
                    reason: Some("owned: 1 file(s) changed that the order does not own".into()),
                    axes: Vec::new(),
                },
            )
            .expect("gated");
        let summary = retro_with(&project, "r1", &[]).expect("retro");
        assert_eq!(summary.proposed.len(), 1, "{summary:?}");
    }

    #[test]
    fn a_quote_not_in_the_journal_is_refused() {
        let root = temp("quote");
        let project = Project::at(&root);
        journal(&project, "r1", REASONS[0]);
        let line =
            raw_lines(&std::fs::read_to_string(project.journal("r1").path).expect("j"))[&2].clone();
        let mut proposal = Proposal {
            gap: "a gap".into(),
            target: "skills/seat/SKILL.md".into(),
            changed: "c".into(),
            because: "b".into(),
            measured: "m".into(),
            if_you_remove: "r".into(),
            evidence: vec![Evidence {
                run: "r1".into(),
                seq: 2,
                quote: "\"verdict\":\"pass\"".into(),
            }],
        };
        let why = propose(&project, &proposal, None).expect_err("refused");
        assert!(
            why.contains("the quote is not in run r1's journal line 2"),
            "{why}"
        );
        proposal.evidence[0].seq = 9;
        assert!(propose(&project, &proposal, None)
            .expect_err("no line")
            .contains("no journal line 9"));
        proposal.evidence.clear();
        assert!(propose(&project, &proposal, None).is_err());
        assert!(!proposals_dir(&root).exists());
        proposal.evidence = vec![Evidence {
            run: "r1".into(),
            seq: 2,
            quote: cut(&line, 60),
        }];
        assert!(propose(&project, &proposal, None).is_ok());
    }

    #[test]
    fn a_rejected_gap_never_graduates_again() {
        let root = temp("reject");
        let project = Project::at(&root);
        for (run, reason) in ["r1", "r2", "r3"].iter().zip(REASONS) {
            journal(&project, run, reason);
            retro_with(&project, run, &[]).expect("retro");
        }
        let (id, _, path) = open(&project).pop().expect("open");
        reject(&project, &id, "the owner disagrees").expect("reject");
        assert!(open(&project).is_empty());
        assert!(std::fs::read_to_string(&path)
            .expect("text")
            .contains("status: rejected"));
        // A fourth run, and the same gap said a new way, open nothing.
        std::fs::remove_file(&path).expect("even with the file gone");
        journal(&project, "r4", REASONS[3]);
        let fourth = retro_with(&project, "r4", &[]).expect("fourth");
        assert!(fourth.proposed.is_empty(), "{fourth:?}");
        assert!(!path.exists());
    }

    #[test]
    fn forbidden_targets_are_refused_and_an_expected_use_cell_is_not() {
        for (target, word) in [
            ("CLAUDE.md", "CLAUDE.md"),
            ("skills/x/CLAUDE.md", "CLAUDE.md"),
            ("skills/gate/SKILL.md", "gate"),
            ("src/nirvana_gate.rs", "gate"),
            ("docs/decisions/0017.md", "decision"),
            ("skills/decisions/x.md", "decision"),
            ("seats.toml", "constants"),
            ("seats.toml:constants.reserve", "constants"),
            ("seats.toml:class.plan.bars", "constants"),
            ("/etc/passwd", "absolute"),
            ("C:/x", "absolute"),
            ("skills/../CLAUDE.md", ".."),
            ("src/main.rs", "skills/ or agents/"),
        ] {
            let why = check_target(target).expect_err(target);
            assert!(why.contains(word), "{target}: {why}");
        }
        assert!(check_target("seats.toml:seat.sonnet.use.medium").is_ok());
        assert!(check_target("skills/seat/SKILL.md").is_ok());
        assert!(check_target("agents/nirvana.md").is_ok());
    }

    #[test]
    fn a_rate_limit_gap_proposes_a_measured_expected_use_cell() {
        let record: Vec<RecordLine> = vec![serde_json::from_str(
            r#"{"order":"a","seat":"sonnet","effort":"medium","class":"build.bounded","outcome":"pass","use":{"5h":0.002,"week":0.0004}}"#,
        )
        .expect("line")];
        let row = Row {
            v: 1,
            key: "g-x".into(),
            run: "r1".into(),
            at: 0,
            found: Found {
                kind: "rate_limit".into(),
                gap: "a worker ran out of pool max window 5h (rate_limit_event)".into(),
                seq: 3,
                quote: "q".into(),
                outcome_tagged: false,
                class: Some("build.bounded".into()),
                seat: Some("sonnet".into()),
                effort: Some("medium".into()),
                axis: None,
            },
        };
        let proposal = draft("g-x", &[&row], &record);
        assert_eq!(proposal.target, "seats.toml:seat.sonnet.use.medium");
        assert!(
            proposal.changed.contains("5h 0.2000%"),
            "{}",
            proposal.changed
        );
        assert!(check_target(&proposal.target).is_ok());
    }
}
