//! Nirvana fork, plan 12 O5: the completion gate. `herdr gate run|author|merge|status`, deterministic and calling no
//! model. (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §7.3; old Nirvana decisions 0017, 0019)
//!
//! The owner, 2026-10-17: "The goal is to utilize both my claude subscription and my agy gemini subsritpion to buid
//! better apps" (plan 12 L1). Better apps is what the gate holds a delivery to.
//!
//! `herdr gate run <order>` rules on a delivered order, in this order (§7.3):
//! 1. **rebase**: the order's branch onto the tip of the run's integration branch `nirvana-integrate/<run>`, so two
//!    orders that each pass alone cannot break the build together (Argon's review F6). The worker's uncommitted work
//!    is committed first, since a rebase cannot carry a dirty tree. A conflict sends it back with the files.
//! 2. **tests**: the order's `test:` command in its worktree. The output is filtered, and every `check:` the order
//!    names must be in it: a suite total cannot see a check that never ran (Nirvana CLAUDE.md rule 9).
//! 3. **report**: the Work Report against O3's schema; an empty or missing report fails, and so does `blocked`.
//! 4. **owned**: only the files the order's `owns:` lines name changed.
//! 5. **served**, for `ops.terminal` orders: each `serves:` port is listened on by a process in the worker's own pane
//!    (decision 0041 R6; old Nirvana's incident of 2026-10-08: a worker found a parallel session's listener on 8770,
//!    reported `done`, and the gate passed it).
//!
//! Then **held-out**, for a feature-size order: a check another family wrote before the build (decision 0019, and
//! plan 12 §14: Argon's review asked to cut it; not taken, since old Nirvana's M5 row passed 8 of 8 with it against
//! 4 of 8 without). The builder sees only the failing case ids. Then **review**, for a load-bearing order: a reviewer
//! of another family (decision 0017), whose verdict comes back as its Work Report.
//!
//! A failing axis is a **send-back**: a child order `<root>-s<n>` in the same worktree with the reasons, at most 3 per
//! order; the fourth failure **asks** the owner. Old Nirvana's decision 0010: a continuation is a child order.
//!
//! The order file's gate lines, each at the start of a line, anywhere in the order, any number of times:
//!
//! ```text
//! owns: src/calc.py tests/**/*.py      paths from the repository root; `*` within a name, `**` across folders
//! test: python -m pytest -q -rA tests  one command, run without a shell, in the worker's folder
//! check: test_mul test_add             names that must appear in the test output
//! serves: 8770                         ops orders: ports the worker's pane must listen on
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::nirvana_run::{self, now, replay, Journal, Kind, OrderState, Phase, Project, Seated};
use crate::nirvana_seat::{self, Blast, NoSeat, Pick, Request, SizeName, Table, World};

/// §7.3: "At most 3 send-backs per order; then the owner is asked."
pub const SEND_BACK_CAP: u32 = 3;
/// A test command running longer than this is stopped and fails the axis. A hypothesis: the largest suite herdr
/// gates today, the fork's own nextest run, takes about 9 minutes on this PC.
pub const TEST_TIMEOUT_S: u64 = 1200;
/// The run's integration branch. Not under `nirvana/`, where O3 names each order's branch: `nirvana/<order>` and
/// `nirvana/<run>/...` could not both exist when an order is named like a run.
pub const INTEGRATION_PREFIX: &str = "nirvana-integrate/";
/// The filtered output kept in an axis and in a send-back's brief.
const KEPT_LINES: usize = 40;
/// Held-out authors per order: one retry with the reason, then the build goes on without (decision 0019 §2.3).
const AUTHOR_TRIES: usize = 2;
/// `herdr gate run` exits with this while a review or a held-out check is still to come.
pub const WAIT_EXIT: i32 = 4;

// ---- the outcome words ----

/// The words the gate's `gated` events, the record and the tally use. O3 left the gate's words to this package
/// ("pass" is the gate's word; every other word counted as a failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The gate passed it.
    Pass,
    /// The gate found a failing axis and seated a child order with the reasons.
    SentBack,
    /// The order ended without a delivery the gate could pass: its worker failed, was cut, or it was never gated.
    Failed,
    /// It waits for the owner: the send-back cap, a worker's question, no seat left.
    Asked,
    /// Its pool ran out and a child on another seat carries the work (§5.6).
    Reseated,
}

impl Outcome {
    pub const ALL: [Self; 5] = [
        Self::Pass,
        Self::SentBack,
        Self::Failed,
        Self::Asked,
        Self::Reseated,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::SentBack => "sent_back",
            Self::Failed => "failed",
            Self::Asked => "asked",
            Self::Reseated => "reseated",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|outcome| outcome.word() == word)
    }

    /// What the seat record's tally counts (decision 0029's blend): `Some(true)` a pass, `Some(false)` a failure,
    /// `None` not counted. A re-seat is not counted: the pool's window ran out, which says nothing of the seat's
    /// work, and its child's own line carries the outcome. A word this herdr does not know (O2's "sent back", a
    /// later word) counts as a failure, as O2 counted it.
    pub fn tally(word: &str) -> Option<bool> {
        match Self::parse(word) {
            Some(Self::Pass) => Some(true),
            Some(Self::Reseated) => None,
            Some(Self::SentBack | Self::Failed | Self::Asked) | None => Some(false),
        }
    }
}

// ---- axes ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    Pass,
    /// Blocks: a send-back.
    Fail,
    /// Said, never blocking: an axis that does not apply, or a disclosure.
    Note,
    /// Not decided yet: a review or a held-out check to come.
    Wait,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Axis {
    pub axis: String,
    pub mark: Mark,
    pub why: String,
    /// The filtered output, the conflicting or unowned files: what the reader needs, never the raw output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
}

impl Axis {
    fn new(axis: &str, mark: Mark, why: impl Into<String>) -> Self {
        Self {
            axis: axis.to_owned(),
            mark,
            why: why.into(),
            lines: Vec::new(),
        }
    }

    fn with(mut self, lines: Vec<String>) -> Self {
        self.lines = lines;
        self
    }
}

pub fn failing(axes: &[Axis]) -> Vec<&Axis> {
    axes.iter().filter(|axis| axis.mark == Mark::Fail).collect()
}

// ---- the order's gate lines ----

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spec {
    pub owns: Vec<String>,
    pub test: Option<String>,
    pub checks: Vec<String>,
    pub serves: Vec<u16>,
}

fn words(rest: &str) -> impl Iterator<Item = String> + '_ {
    rest.split([' ', ',', '\t'])
        .map(|word| word.trim().trim_matches('`'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
}

/// The gate lines of an order file (the syntax in the module comment). herdr's own footer and a send-back's brief
/// never start a line with one of these words, so a child order reads its root's lines unchanged.
pub fn spec(text: &str) -> Spec {
    let mut spec = Spec::default();
    for line in text.lines() {
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix("owns:") {
            spec.owns
                .extend(words(rest).map(|path| path.replace('\\', "/")));
        } else if let Some(rest) = line.strip_prefix("test:") {
            let command = rest.trim().trim_matches('`').trim();
            if !command.is_empty() {
                spec.test = Some(command.to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("check:") {
            spec.checks.extend(words(rest));
        } else if let Some(rest) = line.strip_prefix("serves:") {
            spec.serves
                .extend(words(rest).filter_map(|port| port.parse::<u16>().ok()));
        }
    }
    spec
}

/// `*` and `?` within one path segment, `**` across any number of segments, `/` the only separator.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    fn segments(p: &[&str], s: &[&str]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(&"**"), _) => segments(&p[1..], s) || (!s.is_empty() && segments(p, &s[1..])),
            (Some(pat), Some(seg)) => {
                segment(pat.as_bytes(), seg.as_bytes()) && segments(&p[1..], &s[1..])
            }
            _ => false,
        }
    }
    fn segment(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => segment(&p[1..], s) || (!s.is_empty() && segment(p, &s[1..])),
            (Some(b'?'), Some(_)) => segment(&p[1..], &s[1..]),
            (Some(a), Some(b)) => a == b && segment(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let pattern = pattern.trim_start_matches("./");
    let path = path.trim_start_matches("./");
    let p: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let s: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments(&p, &s)
}

/// herdr's own folder in a worktree: the order copy, offered lessons, `.gitignore`. Never the worker's change.
fn harness_path(path: &str) -> bool {
    path == nirvana_run::DIR || path.starts_with(&format!("{}/", nirvana_run::DIR))
}

/// Plan 24 §6 (ledger L3): build.*, fix.*, and plan orders must name the files they own; with no owned files,
/// the axis fails. Other classes (e.g. read-only) keep the Note from decision 0016 §8.8 point 4.
pub fn requires_owns(class: &str) -> bool {
    class.starts_with("build.") || class.starts_with("fix.") || class == "plan"
}

/// Axis 4. Plan 24 §6: build.*, fix.*, and plan orders must name the files they own.
pub fn owned_axis(class: &str, changed: &[String], owns: &[String]) -> Axis {
    let changed: Vec<&String> = changed.iter().filter(|path| !harness_path(path)).collect();
    if owns.is_empty() {
        if requires_owns(class) {
            return Axis::new(
                "owned",
                Mark::Fail,
                "unbounded: the order names no owned file",
            )
            .with(changed.iter().map(|path| (*path).clone()).collect());
        }
        return Axis::new(
            "owned",
            Mark::Note,
            format!(
                "the order names no owned file; {} file(s) changed, not checked",
                changed.len()
            ),
        )
        .with(changed.iter().map(|path| (*path).clone()).collect());
    }
    let outside: Vec<String> = changed
        .iter()
        .filter(|path| !owns.iter().any(|pattern| glob_match(pattern, path)))
        .map(|path| (*path).clone())
        .collect();
    if outside.is_empty() {
        Axis::new(
            "owned",
            Mark::Pass,
            format!("{} changed file(s), each owned", changed.len()),
        )
    } else {
        Axis::new(
            "owned",
            Mark::Fail,
            format!(
                "{} file(s) changed that the order does not own",
                outside.len()
            ),
        )
        .with(outside)
    }
}

/// Axis 3: O3's check, and a report that says it is done. A `blocked` worker's work is not passed.
pub fn report_axis(report: Option<&Value>) -> Axis {
    let Some(report) = report else {
        return Axis::new("report", Mark::Fail, "no Work Report was delivered");
    };
    if let Err(why) = crate::nirvana_worker::check_report(report) {
        return Axis::new("report", Mark::Fail, why);
    }
    match report.get("status").and_then(Value::as_str) {
        Some("done") => Axis::new("report", Mark::Pass, "well formed, status done"),
        Some(status) => Axis::new(
            "report",
            Mark::Fail,
            format!(
                "the worker reported {status}: {}",
                report.get("summary").and_then(Value::as_str).unwrap_or("")
            ),
        ),
        None => Axis::new("report", Mark::Fail, "the Work Report has no status"),
    }
}

/// A line of test output worth keeping: a named check, a failure, or a summary (Nirvana CLAUDE.md rule 9: the
/// doctrine measured a raw suite at 195,087 bytes of which 45 were information).
fn keep_line(line: &str, checks: &[String]) -> bool {
    if checks.iter().any(|check| line.contains(check.as_str())) {
        return true;
    }
    let lower = line.to_ascii_lowercase();
    [
        "fail",
        "error",
        "panicked",
        "traceback",
        "passed",
        "test result:",
        "tests run",
        "ran ",
    ]
    .iter()
    .any(|mark| lower.contains(mark))
        || line.trim_start().starts_with("OK")
}

pub fn filter(output: &str, checks: &[String]) -> Vec<String> {
    let kept: Vec<String> = output
        .lines()
        .map(str::trim_end)
        .filter(|line| keep_line(line, checks))
        .map(|line| line.chars().take(300).collect())
        .collect();
    if kept.len() > KEPT_LINES {
        // The last lines hold the summary a reader reads first.
        kept[kept.len() - KEPT_LINES..].to_vec()
    } else {
        kept
    }
}

/// Axis 2, from a finished run of the order's test command.
pub fn tests_axis(test: Option<&str>, run: Option<&CommandRun>, checks: &[String]) -> Axis {
    let (Some(test), Some(run)) = (test, run) else {
        return Axis::new(
            "tests",
            if checks.is_empty() {
                Mark::Note
            } else {
                Mark::Fail
            },
            if checks.is_empty() {
                "the order names no test command".to_owned()
            } else {
                "the order names checks but no `test:` command to run them".to_owned()
            },
        );
    };
    let lines = filter(&run.output, checks);
    if let Some(why) = &run.error {
        return Axis::new("tests", Mark::Fail, format!("`{test}` did not run: {why}")).with(lines);
    }
    if run.timed_out {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!("`{test}` ran past {TEST_TIMEOUT_S} s and was stopped"),
        )
        .with(lines);
    }
    // The named checks are looked for in the FILTERED output, which is what the reader is shown: a check the
    // filter dropped was not seen.
    let absent: Vec<&String> = checks
        .iter()
        .filter(|check| !lines.iter().any(|line| line.contains(check.as_str())))
        .collect();
    if run.exit != Some(0) {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!(
                "`{test}` exited {}",
                run.exit
                    .map_or("without a code".to_owned(), |c| c.to_string())
            ),
        )
        .with(lines);
    }
    if !absent.is_empty() {
        return Axis::new(
            "tests",
            Mark::Fail,
            format!(
                "the named check(s) {} never ran: a suite total cannot see a check that never ran",
                absent
                    .iter()
                    .map(|check| check.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
        .with(lines);
    }
    Axis::new(
        "tests",
        Mark::Pass,
        if checks.is_empty() {
            format!("`{test}` exited 0 (the order names no check)")
        } else {
            format!("`{test}` exited 0 and printed {}", checks.join(", "))
        },
    )
    .with(lines)
}

/// Axis 5. `listened` is every port listened on inside the worker's pane's process tree.
pub fn served_axis(class: &str, wanted: &[u16], listened: Result<Vec<u16>, String>) -> Axis {
    if class != "ops.terminal" {
        return Axis::new("served", Mark::Note, "not an ops order");
    }
    if wanted.is_empty() {
        return Axis::new("served", Mark::Fail, "an ops order names no `serves:` port");
    }
    let listened = match listened {
        Ok(ports) => ports,
        Err(why) => {
            return Axis::new(
                "served",
                Mark::Fail,
                format!("the worker's pane could not be read: {why}"),
            )
        }
    };
    let missing: Vec<String> = wanted
        .iter()
        .filter(|port| !listened.contains(port))
        .map(u16::to_string)
        .collect();
    if missing.is_empty() {
        Axis::new(
            "served",
            Mark::Pass,
            format!("the worker's pane listens on {}", join_ports(wanted)),
        )
    } else {
        Axis::new(
            "served",
            Mark::Fail,
            format!(
                "no process in the worker's own pane listens on {} (it listens on {})",
                missing.join(", "),
                if listened.is_empty() {
                    "nothing".to_owned()
                } else {
                    join_ports(&listened)
                }
            ),
        )
    }
}

fn join_ports(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Trims fragment and trailing slash from a URL, lowercases scheme and host, drops www., and treats http and https as equal.
pub fn normalize_url(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url).trim();
    if without_fragment.is_empty() {
        return String::new();
    }
    let (scheme, rest) = if let Some(idx) = without_fragment.find("://") {
        let rest = &without_fragment[idx + 3..];
        ("http://", rest)
    } else {
        ("", without_fragment)
    };
    let (host, path_and_query) = match rest.find(['/', '?']) {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    let mut host = host.to_ascii_lowercase();
    if host.starts_with("www.") {
        host = host[4..].to_string();
    }
    let (path, query) = match path_and_query.find('?') {
        Some(q_idx) => (&path_and_query[..q_idx], &path_and_query[q_idx..]),
        None => (path_and_query, ""),
    };
    let trimmed_path = path.trim_end_matches('/');
    format!("{scheme}{host}{trimmed_path}{query}")
}

fn collect_json_strings(val: &Value, out: &mut Vec<String>) {
    match val {
        Value::String(s) => out.push(s.clone()),
        Value::Array(arr) => {
            for v in arr {
                collect_json_strings(v, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_json_strings(v, out);
            }
        }
        _ => {}
    }
}

pub fn extract_url_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let slice = &text[cursor..];
        let next_http = slice.find("http://");
        let next_https = slice.find("https://");
        let next_pos = match (next_http, next_https) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some(pos) = next_pos else {
            break;
        };
        let start = cursor + pos;
        let url_part = &text[start..];
        let len = url_part
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>'))
            .unwrap_or(url_part.len());
        let token = trim_url_tail(&url_part[..len]);
        tokens.push(token.to_string());
        cursor = start + len;
    }
    tokens
}

/// A URL in a shell line or in prose often ends at `;`, `,`, `.` or a closing `)`; a `)` stays when the URL opened
/// one, as in Wikipedia's `Foo_(bar)`.
fn trim_url_tail(token: &str) -> &str {
    let mut t = token;
    while let Some(last) = t.chars().last() {
        let opened = last == ')' && t.matches('(').count() >= t.matches(')').count();
        if matches!(last, ';' | ',' | '.' | ')' | '|' | '&') && !opened {
            t = &t[..t.len() - 1];
        } else {
            break;
        }
    }
    t
}

/// The URLs a shell command fetches. A command is cut at `;`, `|`, `&&` and newlines, and a segment's URLs count only
/// when the segment itself runs a fetch tool, looked for outside its URLs: `curl a; echo <url>` and
/// `echo https://curl.se/` fetch nothing.
pub fn shell_fetch_urls(cmd: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for segment in cmd.split(['\n', ';', '|']).flat_map(|s| s.split("&&")) {
        let tokens = extract_url_tokens(segment);
        let mut rest = segment.to_owned();
        for token in &tokens {
            rest = rest.replace(token.as_str(), " ");
        }
        if command_is_shell_fetch(&rest) {
            urls.extend(tokens);
        }
    }
    urls
}

pub const SHELL_FETCH_WORDS: &[&str] = &[
    "agent-browser",
    "bladebro",
    "curl",
    "wget",
    "Invoke-WebRequest",
    "iwr",
];

pub fn command_is_shell_fetch(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    SHELL_FETCH_WORDS
        .iter()
        .any(|word| lower.contains(&word.to_ascii_lowercase()))
}

/// Checks if a normalized URL was fetched, read or browsed in the worker stream.
fn url_in_stream(norm_url: &str, stream: &str) -> bool {
    if norm_url.is_empty() {
        return false;
    }
    for line in stream.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };

        // Claude: message.content[] items with type == "tool_use"
        if let Some(content) = val.pointer("/message/content").and_then(Value::as_array) {
            for block in content {
                if block.get("type").and_then(Value::as_str) == Some("tool_use") {
                    let name = block.get("name").and_then(Value::as_str).unwrap_or("");
                    let is_fetch = name == "WebFetch"
                        || (name.starts_with("mcp__")
                            && (name.contains("browser")
                                || name.contains("bladebro")
                                || name.contains("chrome")
                                || name.contains("playwright")));
                    if is_fetch {
                        if let Some(input) = block.get("input") {
                            let mut strings = Vec::new();
                            collect_json_strings(input, &mut strings);
                            for s in strings {
                                for tok in extract_url_tokens(&s) {
                                    if normalize_url(&tok) == norm_url {
                                        return true;
                                    }
                                }
                            }
                        }
                    } else if name == "Bash" {
                        // The command only: a `description` naming curl and a URL fetches nothing.
                        let command = block
                            .pointer("/input/command")
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        if shell_fetch_urls(command)
                            .iter()
                            .any(|tok| normalize_url(tok) == norm_url)
                        {
                            return true;
                        }
                    }
                }
            }
        }

        // agy: step_update tool steps
        let step = val.get("step_update").unwrap_or(&val);
        if step.get("step_type").and_then(Value::as_str) == Some("tool") {
            let tool_name = step.get("tool_name").and_then(Value::as_str).unwrap_or("");
            let params = &step["tool_info"]["parameters"];
            let is_browser_mcp = tool_name == "call_mcp_tool" && {
                let mut p_strings = Vec::new();
                collect_json_strings(params, &mut p_strings);
                p_strings.iter().any(|s| {
                    let s_lower = s.to_ascii_lowercase();
                    s_lower.contains("browser")
                        || s_lower.contains("bladebro")
                        || s_lower.contains("chrome")
                        || s_lower.contains("playwright")
                })
            };
            let is_agy_fetch = tool_name == "read_url_content"
                || tool_name == "open_browser_url"
                || tool_name == "read_browser_page"
                || tool_name.starts_with("browser_")
                || is_browser_mcp;
            if is_agy_fetch {
                let mut strings = Vec::new();
                collect_json_strings(params, &mut strings);
                for s in strings {
                    for tok in extract_url_tokens(&s) {
                        if normalize_url(&tok) == norm_url {
                            return true;
                        }
                    }
                }
            } else if tool_name == "run_command" {
                // agy's command is `CommandLine` (the recorded fixtures); its other parameters are not the command.
                let command = params
                    .get("CommandLine")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if shell_fetch_urls(command)
                    .iter()
                    .any(|tok| normalize_url(tok) == norm_url)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Plan 24 §5: gate axis for `research.*` and `media.*` classes.
/// Fails with no sources; fails for each source URL that does not appear in the order's stream
/// as a fetch, read or browser call. URLs are compared after trimming a trailing `/` and the fragment.
pub fn sources_axis(class: &str, report: Option<&Value>, stream: &str) -> Option<Axis> {
    if !class.starts_with("research.") && !class.starts_with("media.") {
        return None;
    }
    let Some(report) = report else {
        return Some(Axis::new(
            "sources",
            Mark::Fail,
            "no sources: no Work Report was delivered",
        ));
    };
    let sources = match report.get("sources").and_then(Value::as_array) {
        Some(s) if !s.is_empty() => s,
        _ => {
            return Some(Axis::new(
                "sources",
                Mark::Fail,
                "no sources: the Work Report names no sources",
            ))
        }
    };
    let mut missing = Vec::new();
    for item in sources {
        let Some(url) = item.get("url").and_then(Value::as_str) else {
            missing.push("item without url".to_owned());
            continue;
        };
        let norm = normalize_url(url);
        if !url_in_stream(&norm, stream) {
            missing.push(url.to_owned());
        }
    }
    if missing.is_empty() {
        Some(Axis::new(
            "sources",
            Mark::Pass,
            format!("{} source(s) cited, each verified in stream", sources.len()),
        ))
    } else {
        Some(
            Axis::new(
                "sources",
                Mark::Fail,
                format!(
                    "{} source(s) cited that were not fetched, read or browsed",
                    missing.len()
                ),
            )
            .with(missing),
        )
    }
}

/// The ports listened on in the tree under the pane's shell, from one process table and one listener table:
/// `nirvana_servers`'s own walk (feature 4), so a server counts here exactly when the sidebar shows it.
pub fn pane_ports(
    shell_pid: u32,
    processes: &[crate::nirvana_servers::Process],
    listeners: &[crate::nirvana_servers::Listener],
) -> Vec<u16> {
    crate::nirvana_servers::space_ports(&[shell_pid], processes, listeners)
}

// ---- held-out cases ----

/// One case of a check's output: its id and whether it passed. pytest `-rA` (`PASSED id`, `FAILED id - why`),
/// cargo test (`test id ... ok`), unittest `-v` (`name (id) ... ok`).
pub fn cases(output: &str) -> Vec<(String, bool)> {
    let mut found: Vec<(String, bool)> = Vec::new();
    let mut push = |id: &str, pass: bool| {
        let id = id.trim().to_owned();
        if id.is_empty() {
            return;
        }
        match found.iter_mut().find(|(known, _)| *known == id) {
            Some(entry) => entry.1 &= pass,
            None => found.push((id, pass)),
        }
    };
    for line in output.lines().map(str::trim) {
        let mut parts = line.split_whitespace();
        let first = parts.next().unwrap_or("");
        // pytest -v: `tests/test_x.py::test_a PASSED [ 50%]`.
        if first.contains("::") {
            match parts.clone().next() {
                Some("PASSED" | "XFAIL") => push(first, true),
                Some("FAILED" | "ERROR" | "XPASS") => push(first, false),
                _ => {}
            }
        }
        match first {
            "PASSED" | "XFAIL" => push(parts.next().unwrap_or(""), true),
            // unittest ends `FAILED (errors=2)`: a count, not a case (the scripted run of 2026-10-18 read it as a
            // fourth case of a three-case check).
            "FAILED" | "ERROR" | "XPASS" => {
                let id = parts.next().unwrap_or("");
                if !id.starts_with('(') {
                    push(id, false);
                }
            }
            "test" => {
                if let Some((id, result)) = line[4..].split_once(" ... ") {
                    match result.trim() {
                        "ok" => push(id, true),
                        "FAILED" => push(id, false),
                        _ => {}
                    }
                }
            }
            _ => {
                if let Some((head, result)) = line.split_once(" ... ") {
                    if let (Some(open), true) = (head.find(" ("), head.ends_with(')')) {
                        let id = &head[open + 2..head.len() - 1];
                        match result.trim() {
                            "ok" => push(id, true),
                            "FAIL" | "ERROR" => push(id, false),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    found
}

/// Decision 0019 §2.3: a check is valid at the base when at least one case ran and at least one fails there; a
/// check that passes at the base tests nothing.
pub fn heldout_valid(base: &[(String, bool)]) -> Result<(), String> {
    if base.is_empty() {
        return Err("no case of the check ran at the base".into());
    }
    if base.iter().all(|(_, pass)| *pass) {
        return Err(format!(
            "all {} case(s) pass at the base: the check tests nothing",
            base.len()
        ));
    }
    Ok(())
}

/// The held-out axis at the order's head. The ids are the only thing the builder will see (§7.3: "the builder sees
/// only the failing test ids"), so the axis holds them and nothing of the check's output.
pub fn heldout_axis(run: &CommandRun) -> Axis {
    let found = cases(&run.output);
    let failing: Vec<String> = found
        .iter()
        .filter(|(_, pass)| !pass)
        .map(|(id, _)| id.clone())
        .collect();
    if found.is_empty() {
        return Axis::new("heldout", Mark::Fail, "the held-out check printed no case");
    }
    match (failing.is_empty(), run.exit) {
        (true, Some(0)) => Axis::new(
            "heldout",
            Mark::Pass,
            format!("{} held-out case(s) pass", found.len()),
        ),
        (true, exit) => Axis::new(
            "heldout",
            Mark::Fail,
            format!(
                "the held-out check exited {exit:?} with its {} case(s) passing",
                found.len()
            ),
        ),
        (false, _) => Axis::new(
            "heldout",
            Mark::Fail,
            format!("{} held-out case(s) fail", failing.len()),
        )
        .with(failing),
    }
}

// ---- running a command without a shell ----

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRun {
    pub exit: Option<i32>,
    pub output: String,
    pub timed_out: bool,
    /// It did not start.
    pub error: Option<String>,
}

/// Words of a command line, with `"` and `'` quoting. No shell: Git Bash rewrote `/usage` into a path twice on
/// 2026-10-17 (plan 12 §3), and a gate line is the orchestrator's text.
pub fn split_command(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    let mut any = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if any || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                    any = false;
                }
            }
            None => word.push(c),
        }
    }
    if any || !word.is_empty() {
        words.push(word);
    }
    words
}

/// Runs `line` in `cwd`, stdout and stderr together, stopped after `timeout`. On Windows a program that is a
/// `.cmd` (npm) is not found by name, so a not-found start is retried through `cmd /C`.
pub fn run_command(cwd: &Path, line: &str, timeout: Duration) -> CommandRun {
    let argv = split_command(line);
    let Some((program, args)) = argv.split_first() else {
        return CommandRun {
            error: Some("an empty command".into()),
            ..CommandRun::default()
        };
    };
    let spawn = |program: &str, args: &[String]| {
        let mut command = std::process::Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        crate::platform::configure_background_command(&mut command);
        command.spawn()
    };
    let child = match spawn(program, args) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && cfg!(windows) => {
            let mut through = vec!["/C".to_owned()];
            through.extend(argv.iter().cloned());
            spawn("cmd", &through)
        }
        other => other,
    };
    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            return CommandRun {
                error: Some(err.to_string()),
                ..CommandRun::default()
            }
        }
    };
    let pipe = |reader: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = Vec::new();
            if let Some(mut reader) = reader {
                let _ = reader.read_to_end(&mut text);
            }
            String::from_utf8_lossy(&text).into_owned()
        })
    };
    let out = pipe(
        child
            .stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let err = pipe(
        child
            .stderr
            .take()
            .map(|r| Box::new(r) as Box<dyn Read + Send>),
    );
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() > timeout => {
                timed_out = true;
                let _ = child.kill();
                break child.wait().ok();
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => break None,
        }
    };
    let mut output = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if !stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&stderr);
    }
    CommandRun {
        exit: status.and_then(|status| status.code()),
        output,
        timed_out,
        error: None,
    }
}

// ---- git ----

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn rev(cwd: &Path, name: &str) -> Option<String> {
    git(
        cwd,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{commit}}"),
        ],
    )
    .ok()
    .map(|sha| sha.trim().to_owned())
    .filter(|sha| !sha.is_empty())
}

pub fn integration_branch(run: &str) -> String {
    format!("{INTEGRATION_PREFIX}{run}")
}

/// The run's integration branch, made from the project's HEAD the first time an order of the run is gated.
pub fn ensure_integration(root: &Path, run: &str) -> Result<String, String> {
    let branch = integration_branch(run);
    if let Some(sha) = rev(root, &format!("refs/heads/{branch}")) {
        return Ok(sha);
    }
    let head = rev(root, "HEAD")
        .ok_or("the project has no commit to start the integration branch from")?;
    git(root, &["branch", &branch, &head])?;
    Ok(head)
}

/// `-c user.name=... -c user.email=...` when the repository has no identity: a gate commit must not fail on a
/// machine where git was never told who commits.
fn identity(cwd: &Path) -> Vec<String> {
    let set = |key: &str| git(cwd, &["config", key]).is_ok_and(|value| !value.trim().is_empty());
    if set("user.email") && set("user.name") {
        Vec::new()
    } else {
        [
            "-c",
            "user.name=herdr gate",
            "-c",
            "user.email=herdr-gate@localhost",
        ]
        .map(str::to_owned)
        .to_vec()
    }
}

/// Every path, outside herdr's own folder, with a change the worker has not committed.
fn uncommitted(cwd: &Path) -> Result<Vec<String>, String> {
    let status = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "-uall",
            "--",
            ".",
            ":(exclude).nirvana",
        ],
    )?;
    Ok(status
        .lines()
        .filter(|line| line.len() > 3)
        .map(|line| {
            let path = &line[3..];
            path.rsplit(" -> ")
                .next()
                .unwrap_or(path)
                .trim_matches('"')
                .to_owned()
        })
        .filter(|path| !harness_path(path))
        .collect())
}

/// Commits the worker's work on its own branch, herdr's folder left out: every change to a tracked file, and each
/// new file an `owns:` pattern names. A new file no pattern names stays out of the commit and is returned: the live
/// run of 2026-10-18 committed the worker's `__pycache__/*.pyc`, which its own test run had made in a project with
/// no `.gitignore`, and then failed the order on the owned axis for the gate's own commit. With no `owns:` line every
/// new file is the worker's, as before.
pub fn commit_work(cwd: &Path, order: &str, owns: &[String]) -> Result<Vec<String>, String> {
    let status = git(
        cwd,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).nirvana",
        ],
    )?;
    let (mut tracked, mut new, mut left_out) = (false, Vec::new(), Vec::new());
    for line in status.lines().filter(|line| line.len() > 3) {
        let path = line[3..]
            .rsplit(" -> ")
            .next()
            .unwrap_or("")
            .trim_matches('"')
            .to_owned();
        if !line.starts_with("??") {
            tracked = true;
        } else if owns.is_empty() || owns.iter().any(|pattern| glob_match(pattern, &path)) {
            new.push(path);
        } else {
            left_out.push(path);
        }
    }
    if !tracked && new.is_empty() {
        return Ok(left_out);
    }
    if tracked {
        git(cwd, &["add", "-u", "--", ".", ":(exclude).nirvana"])?;
    }
    for chunk in new.chunks(64) {
        let mut args = vec!["add", "--"];
        args.extend(chunk.iter().map(String::as_str));
        git(cwd, &args)?;
    }
    let mut args = identity(cwd);
    args.extend(
        [
            "commit",
            "-q",
            "-m",
            &format!("herdr gate: the work of {order}\n\nNirvana-Order: {order}"),
        ]
        .map(str::to_owned),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    git(cwd, &args)?;
    Ok(left_out)
}

/// Rebases the worktree's branch onto `onto`. A conflict aborts the rebase and names the files.
pub fn rebase(cwd: &Path, onto: &str) -> Result<(), Vec<String>> {
    let mut args = identity(cwd);
    args.extend(["rebase", "-q", onto].map(str::to_owned));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match git(cwd, &args) {
        Ok(_) => Ok(()),
        Err(why) => {
            let files: Vec<String> = git(cwd, &["diff", "--name-only", "--diff-filter=U"])
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect();
            let _ = git(cwd, &["rebase", "--abort"]);
            Err(if files.is_empty() { vec![why] } else { files })
        }
    }
}

/// The paths the order's commits change against `base`.
pub fn changed_files(cwd: &Path, base: &str) -> Result<Vec<String>, String> {
    Ok(
        git(cwd, &["diff", "--name-only", &format!("{base}...HEAD")])?
            .lines()
            .map(str::to_owned)
            .filter(|path| !path.is_empty())
            .collect(),
    )
}

/// Merges the gated head into the integration branch with no checkout: the order's branch is rebased onto the
/// branch's tip, so the merge's tree is the head's tree. `update-ref` takes the old value, so a branch that moved
/// since the gate is refused, not overwritten. The served checkout is never touched (memory
/// served-checkout-is-a-shared-writer: a landing step ran `reset` in the served `main` on 2026-09-20).
pub fn merge(
    root: &Path,
    run: &str,
    order: &str,
    base: &str,
    head: &str,
) -> Result<String, String> {
    let branch = integration_branch(run);
    let tip = rev(root, &format!("refs/heads/{branch}")).ok_or(format!("no branch {branch}"))?;
    if tip != base {
        return Err(format!(
            "{branch} moved since the gate ({} then, {} now): gate {order} again",
            short_sha(base),
            short_sha(&tip)
        ));
    }
    let mut args = identity(root);
    args.extend(
        [
            "commit-tree",
            &format!("{head}^{{tree}}"),
            "-p",
            base,
            "-p",
            head,
            "-m",
            &format!("herdr gate merge: {order}\n\nNirvana-Order: {order}\nNirvana-Run: {run}"),
        ]
        .map(str::to_owned),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let sha = git(root, &args)?.trim().to_owned();
    git(
        root,
        &["update-ref", &format!("refs/heads/{branch}"), &sha, base],
    )?;
    Ok(sha)
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(8)]
}

// ---- the deterministic half ----

/// What the five axes saw: the integration tip the branch was rebased onto, and the head they ran at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Det {
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    pub axes: Vec<Axis>,
}

/// The order as the five axes need it.
pub struct Subject<'a> {
    pub root: &'a Path,
    pub cwd: &'a Path,
    pub run: &'a str,
    pub order: &'a str,
    /// The order's branch, when it ran in a worktree.
    pub branch: Option<&'a str>,
    pub class: &'a str,
    pub report: Option<&'a Value>,
    pub spec: &'a Spec,
    /// The ports listened on in the worker's pane, read only for an ops order.
    pub ports: &'a dyn Fn() -> Result<Vec<u16>, String>,
}

/// Axes 1 to 5. The test output goes to `log` whole; the axes keep it filtered.
pub fn deterministic(subject: &Subject, log: Option<&Path>) -> Det {
    let mut axes = Vec::new();
    let mut det = Det::default();
    let (base, worktree) = match subject.branch {
        Some(_) => {
            let base = match ensure_integration(subject.root, subject.run) {
                Ok(base) => base,
                Err(why) => {
                    axes.push(Axis::new("rebase", Mark::Fail, why));
                    det.axes = axes;
                    return det;
                }
            };
            match commit_work(subject.cwd, subject.order, &subject.spec.owns) {
                Ok(left_out) if !left_out.is_empty() => axes.push(
                    Axis::new(
                        "commit",
                        Mark::Note,
                        format!(
                            "{} new file(s) that no `owns:` pattern names were left out of the commit",
                            left_out.len()
                        ),
                    )
                    .with(left_out),
                ),
                Ok(_) => {}
                Err(why) => {
                    axes.push(Axis::new(
                        "rebase",
                        Mark::Fail,
                        format!("the worker's work could not be committed: {why}"),
                    ));
                    det.axes = axes;
                    return det;
                }
            }
            match rebase(subject.cwd, &integration_branch(subject.run)) {
                Ok(()) => axes.push(Axis::new(
                    "rebase",
                    Mark::Pass,
                    format!(
                        "onto {} at {}",
                        integration_branch(subject.run),
                        short_sha(&base)
                    ),
                )),
                Err(files) => {
                    axes.push(
                        Axis::new(
                            "rebase",
                            Mark::Fail,
                            format!(
                                "the branch does not rebase onto {}: {} file(s) conflict",
                                integration_branch(subject.run),
                                files.len()
                            ),
                        )
                        .with(files),
                    );
                    det.base = Some(base);
                    det.axes = axes;
                    return det;
                }
            }
            (Some(base), true)
        }
        None => {
            axes.push(Axis::new(
                "rebase",
                Mark::Note,
                "the order ran in the project's folder, not a worktree: no branch to rebase",
            ));
            (None, false)
        }
    };
    det.head = rev(subject.cwd, "HEAD");
    det.base = base.clone();
    let run = subject.spec.test.as_deref().map(|test| {
        let run = run_command(subject.cwd, test, Duration::from_secs(TEST_TIMEOUT_S));
        if let Some(log) = log {
            if let Some(parent) = log.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(log, &run.output);
        }
        run
    });
    axes.push(tests_axis(
        subject.spec.test.as_deref(),
        run.as_ref(),
        &subject.spec.checks,
    ));
    axes.push(report_axis(subject.report));
    let changed = match (&base, worktree) {
        (Some(base), true) => changed_files(subject.cwd, base),
        _ => uncommitted(subject.cwd),
    };
    axes.push(match changed {
        Ok(changed) => owned_axis(subject.class, &changed, &subject.spec.owns),
        Err(why) => Axis::new("owned", Mark::Fail, why),
    });
    let listened = if subject.class == "ops.terminal" {
        (subject.ports)()
    } else {
        Ok(Vec::new())
    };
    axes.push(served_axis(subject.class, &subject.spec.serves, listened));
    let stream = stream_text(&Project::at(subject.root), subject.run, subject.order);
    if let Some(axis) = sources_axis(subject.class, subject.report, &stream) {
        axes.push(axis);
    }
    det.axes = axes;
    det
}

// ---- the gate's own record, beside the journal ----

/// `<run>/gate/<order>.json`: what the journal's events do not carry. The relation of an order to its reviewers and
/// held-out authors, the check as captured, and the last deterministic result with the shas it ran at, so a
/// returning review does not run the tests again on the same trees.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub order: String,
    #[serde(default)]
    pub gates: u32,
    #[serde(default)]
    pub last: Option<Det>,
    #[serde(default)]
    pub reviews: Vec<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub heldout: Option<Heldout>,
    /// Why the order has no held-out check, said in every gate of it (decision 0019 §2.3: "never silently").
    #[serde(default)]
    pub heldout_skipped: Option<String>,
    /// The subject when this order is a reviewer or an author: `(kind, subject)`.
    #[serde(default)]
    pub helps: Option<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heldout {
    pub author: String,
    /// The captured folder, beside the journal; the builder's worktree never holds it.
    pub dir: String,
    pub command: String,
    pub base_cases: Vec<(String, bool)>,
}

fn gate_dir(project: &Project, run: &str) -> PathBuf {
    project.run_dir(run).join("gate")
}

pub fn read_sidecar(project: &Project, run: &str, order: &str) -> Sidecar {
    std::fs::read_to_string(gate_dir(project, run).join(format!("{order}.json")))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| Sidecar {
            order: order.to_owned(),
            ..Sidecar::default()
        })
}

fn write_sidecar(project: &Project, run: &str, sidecar: &Sidecar) -> std::io::Result<()> {
    let dir = gate_dir(project, run);
    std::fs::create_dir_all(&dir)?;
    let mut text = serde_json::to_string_pretty(sidecar)?;
    text.push('\n');
    nirvana_seat::write_atomic(
        &dir.join(format!("{}.json", sidecar.order)),
        text.as_bytes(),
    )
}

/// One gate at a time per project (§7.2: "the gate's builds run one at a time"; old Nirvana's three gates inside
/// four minutes collided on ports, `WinError 10048`). An OS lock under `runs/`, which git ignores.
struct GateLock(std::fs::File);

impl Drop for GateLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

fn gate_lock(project: &Project) -> std::io::Result<GateLock> {
    let path = project.dir().join("runs").join(".gate.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(GateLock(file))
}

// ---- chains, sizes and families ----

/// The order a send-back chain started from, and how many send-backs it has had, this order's own included.
pub fn chain(state: &nirvana_run::RunState, order: &str) -> (String, u32) {
    let mut current = order.to_owned();
    let mut depth = 0;
    while let Some(known) = state.order(&current) {
        match (&known.seated.why, &known.seated.parent) {
            (Some(why), Some(parent)) if why == "send_back" => {
                depth += 1;
                current = parent.clone();
            }
            _ => break,
        }
    }
    (current, depth)
}

/// What the gate does with a failing order (§7.3): a child, or the owner once the cap is spent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendBack {
    Child { id: String, n: u32 },
    Ask,
}

pub fn plan_send_back(state: &nirvana_run::RunState, order: &str) -> SendBack {
    let (root, depth) = chain(state, order);
    if depth >= SEND_BACK_CAP {
        return SendBack::Ask;
    }
    let n = depth + 1;
    let base: String = root.chars().take(56).collect();
    let id = (0..)
        .map(|extra| {
            if extra == 0 {
                format!("{base}-s{n}")
            } else {
                format!("{base}-s{n}-{extra}")
            }
        })
        .find(|id| state.order(id).is_none())
        .unwrap_or_else(|| format!("{base}-s{n}"));
    SendBack::Child { id, n }
}

/// A load-bearing order gets a reviewer of another family: the class's blast in the table (plan, build.load_bearing).
pub fn load_bearing(table: Option<&Table>, class: &str) -> bool {
    match table.and_then(|table| table.class(class)) {
        Some(class) => class.blast == Blast::LoadBearing,
        None => matches!(class, "plan" | "build.load_bearing"),
    }
}

/// A feature-size order gets a held-out check: a build or a debug fix that is not small. §7.3 names "feature-size"
/// without a rule; this reads it off what the orchestrator already says of an order, its class and its size.
pub fn feature_size(class: &str, size: SizeName) -> bool {
    (class.starts_with("build.") || class == "fix.debug") && size != SizeName::Small
}

/// Decision 0017 through O2's `pick`, whose fix round (B1) holds the fallback: another family first; only when no
/// other family has a seat, the builder's own in a fresh session, `same_family_fallback` set and the reason ending
/// "(decision 0017)". The gate puts its mark on top, at the start of the reason, so the orchestrator reads which
/// second opinion it is starting before it reads why.
pub fn pick_other_family(
    table: &Table,
    world: &World,
    class: &str,
    family: &str,
    mode: nirvana_seat::Mode,
    order: &str,
    mark: &str,
) -> Result<(Pick, bool), NoSeat> {
    let mut request = Request::new(class);
    request.mode = mode;
    request.size = SizeName::Small;
    request.order = Some(order.to_owned());
    request.not_family = vec![family.to_owned()];
    let mut pick = nirvana_seat::pick(table, world, &request)?;
    let same = pick.same_family_fallback;
    if same {
        pick.reason = format!("{mark}: {}", pick.reason);
    }
    Ok((pick, same))
}

fn family_of(table: Option<&Table>, seat: &str) -> String {
    table
        .and_then(|table| table.seat(seat))
        .map_or_else(|| "unknown".to_owned(), |seat| seat.family.clone())
}

// ---- briefs ----

fn original(text: &str) -> String {
    text.split("\n\n---\nFrom herdr")
        .next()
        .unwrap_or("")
        .trim_end()
        .to_owned()
}

/// A send-back child's brief: the reasons, then the root order. A held-out failure gives the case ids only.
pub fn send_back_brief(
    order: &str,
    root: &str,
    n: u32,
    axes: &[Axis],
    root_text: &str,
    integration: &str,
) -> String {
    let mut brief = format!(
        "# Send-back {n} of {SEND_BACK_CAP}: {root}\n\nherdr's gate sent back {order}. Its work is in this folder, committed \
on this branch. You are a new session: fix what the gate found, do not start over.\n\n## What the gate found\n\n"
    );
    for axis in failing(axes) {
        brief.push_str(&format!("- **{}**: {}\n", axis.axis, axis.why));
        match axis.axis.as_str() {
            "heldout" => {
                brief.push_str("  A check you cannot see fails these cases; make the work meet the order so they pass:\n");
                for id in &axis.lines {
                    brief.push_str(&format!("  - `{id}`\n"));
                }
            }
            "rebase" => {
                brief.push_str(&format!(
                    "  Run `git rebase {integration}` here and resolve, keeping both changes:\n"
                ));
                for line in &axis.lines {
                    brief.push_str(&format!("  - `{line}`\n"));
                }
            }
            _ => {
                for line in &axis.lines {
                    brief.push_str(&format!("  > {line}\n"));
                }
            }
        }
    }
    brief.push_str("\n## The original order\n\n");
    brief.push_str(&original(root_text));
    brief.push('\n');
    brief
}

fn review_brief(
    order: &str,
    root_text: &str,
    worktree: &str,
    diff: &str,
    same_family: bool,
) -> String {
    format!(
        "# Review of {order} (decision 0017)\n\nAnother agent did the order below. Judge its change as a stranger's work{}. \
Read the diff and the files; change nothing, run what you need to.\n\n- the diff: {diff}\n- its folder: {worktree}\n\n\
Your Work Report is the verdict:\n- status `done`: the change does what the order asks, and nothing it must not.\n\
- status `blocked`: it must go back; put each defect in `gotchas`, one line each, with the file and what is wrong.\n\
- status `needs_answer`: you cannot judge it without the owner; the question in `question`.\n\n## The order\n\n{}\n",
        if same_family { ", though a model of its own family wrote it" } else { "" },
        original(root_text)
    )
}

fn author_brief(order: &str, request: &str, retry: Option<&str>) -> String {
    format!(
        "# Held-out check for {order} (decision 0019)\n\nAnother agent will do the order below after you. You write the \
check it will be held to; you do not build anything.{}\n\n## What to write\n\n\
- A folder `.nirvana/heldout/` in this folder, holding the check: one test file or a few, written from the order \
alone. Name every case. Import the code under test inside each case.\n\
- `.nirvana/heldout/COMMAND`: one line that runs the check from this folder, with `{{dir}}` where the check folder's \
path goes, e.g. `python -m pytest -q -rA {{dir}}`. It must print one line per case with its result (pytest `-rA`, \
`cargo test`, unittest `-v`).\n\
- The check must FAIL here as the folder is now: at least one case fails before the build.\n\
- Change no other file. Read no network.\n\nEnd with the Work Report: status done, the cases named in the summary.\n\n\
## The order\n\n{}\n",
        retry.map(|why| format!("\n\nYour first try was refused: {why}. Write it again.")).unwrap_or_default(),
        original(request)
    )
}

// ---- the gate ----

/// What `herdr gate run` decided.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ruling {
    pub order: String,
    pub run: String,
    /// pass, sent_back, asked, or wait.
    pub verdict: String,
    pub axes: Vec<Axis>,
    /// What comes next: the child, the review or author order to start, or the merge.
    pub next: Vec<String>,
}

impl Ruling {
    pub fn exit(&self) -> i32 {
        match self.verdict.as_str() {
            "pass" => 0,
            "wait" => WAIT_EXIT,
            _ => 1,
        }
    }
}

fn order_text(seated: &Seated) -> String {
    std::fs::read_to_string(&seated.order_file).unwrap_or_default()
}

fn stream_text(project: &Project, run: &str, order: &str) -> String {
    std::fs::read_to_string(
        project
            .run_dir(run)
            .join("streams")
            .join(format!("{order}.jsonl")),
    )
    .unwrap_or_default()
}

fn axes_values(axes: &[Axis]) -> Vec<Value> {
    axes.iter()
        .filter_map(|axis| serde_json::to_value(axis).ok())
        .collect()
}

/// The reason a `gated` event carries: the first failing axis, with no order id in it, so the retro can match the
/// same gap across runs.
pub fn reason_line(axes: &[Axis]) -> Option<String> {
    failing(axes)
        .first()
        .map(|axis| format!("{}: {}", axis.axis, axis.why))
}

fn seat_run_line(
    pick: &Pick,
    file: &Path,
    id: &str,
    run: &str,
    class: &str,
    not_family: Option<&str>,
) -> String {
    format!(
        "herdr seat run {} --order \"{}\" --order-id {id} --run {run} --class {class}{}",
        pick.seat,
        file.display(),
        not_family
            .map(|family| format!(" --not-family {family}"))
            .unwrap_or_default()
    )
}

/// Books `id` on `pick` so the `herdr seat run` the orchestrator types gets the same seat (O3 reads a live booking
/// first).
fn book(table: &Table, pick: &Pick, id: &str, run: &str) -> std::io::Result<()> {
    let dir = nirvana_seat::dir();
    let _lock = nirvana_seat::Lock::take(&dir)?;
    let path = dir.join(nirvana_seat::LEASES_FILE);
    // `load_leases` and `book_pick`, as O3's `seat run` books since its merge: a corrupt file is an error, never read
    // as empty and written back over every run's override count (O2 fix round).
    let mut leases = nirvana_seat::load_leases(&path).map_err(std::io::Error::other)?;
    nirvana_seat::book_pick(&mut leases, table, pick, Some(id), Some(run), now());
    nirvana_seat::write_leases(&path, &leases)
}

fn write_order_file(project: &Project, id: &str, text: &str) -> std::io::Result<PathBuf> {
    project.ensure()?;
    std::fs::create_dir_all(project.orders_dir())?;
    let path = project.orders_dir().join(format!("{id}.md"));
    std::fs::write(&path, text)?;
    Ok(path)
}

/// The worker's pane's listening ports, read live: `herdr pane process-info` for the shell, then the tables.
fn live_ports(pane: Option<&str>) -> Result<Vec<u16>, String> {
    let pane = pane.ok_or("the order has no pane")?;
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let out = std::process::Command::new(exe)
        .args(["pane", "process-info", pane])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    let value: Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| "pane process-info gave no JSON".to_owned())?;
    let shell = [
        "/result/process_info/shell_pid",
        "/process_info/shell_pid",
        "/shell_pid",
    ]
    .iter()
    .find_map(|pointer| value.pointer(pointer).and_then(Value::as_u64))
    .ok_or("pane process-info named no shell pid")? as u32;
    // The Spaces square's own reader (`nirvana_servers::read`), so the gate and the square see one table.
    let processes = crate::nirvana_servers::read::processes();
    let listeners = crate::nirvana_servers::read::listeners();
    Ok(pane_ports(shell, &processes, &listeners))
}

/// Gates one order. The caller holds nothing; this takes the project's gate lock.
pub fn gate(project: &Project, run: &str, order: &str) -> Result<Ruling, String> {
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in run {run}"))?
        .clone();
    let sidecar = read_sidecar(project, run, order);
    let table = nirvana_seat::table_for(&project.root).ok();
    match sidecar.helps.clone() {
        Some((kind, subject)) if kind == "author" => {
            return gate_author(project, run, &journal, &known, &subject, table.as_ref())
        }
        Some((kind, subject)) if kind == "review" => {
            let own = gate_reviewer(&journal, &known, run)?;
            if own.verdict != "pass" {
                return Ok(own);
            }
            drop(_lock);
            return gate(project, run, &subject);
        }
        _ => {}
    }
    match known.phase {
        Phase::Delivered => {}
        // A passed order whose branch or integration tip moved since (another order merged first) is gated again,
        // onto the new tip: `gate merge` refuses the old trees.
        Phase::Gated
            if known.verdict.as_deref() == Some("pass")
                && !trees_moved(project, run, &known, &sidecar) =>
        {
            return Ok(Ruling {
                order: order.to_owned(),
                run: run.to_owned(),
                verdict: "pass".into(),
                axes: sidecar.last.map(|det| det.axes).unwrap_or_default(),
                next: vec![format!("herdr gate merge {order} --run {run}")],
            })
        }
        Phase::Gated if known.verdict.as_deref() == Some("pass") => {}
        phase => {
            return Err(format!(
                "{order} is {}{}: the gate rules on a delivered order",
                phase.word(),
                known
                    .verdict
                    .as_deref()
                    .map(|v| format!(" ({v})"))
                    .unwrap_or_default()
            ))
        }
    }
    gate_subject(
        project,
        run,
        &journal,
        &state,
        &known,
        sidecar,
        table.as_ref(),
    )
}

/// Whether the trees a passed order's gate ran on are gone: its branch's head or the integration tip moved.
fn trees_moved(project: &Project, run: &str, known: &OrderState, sidecar: &Sidecar) -> bool {
    let Some(last) = &sidecar.last else {
        return false;
    };
    if known.seated.branch.is_none() {
        return false;
    }
    let head = rev(Path::new(&known.seated.cwd), "HEAD");
    let tip = rev(
        &project.root,
        &format!("refs/heads/{}", integration_branch(run)),
    );
    last.head != head || last.base != tip
}

fn gate_subject(
    project: &Project,
    run: &str,
    journal: &Journal,
    state: &nirvana_run::RunState,
    known: &OrderState,
    mut sidecar: Sidecar,
    table: Option<&Table>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    let seated = &known.seated;
    let (root, _) = chain(state, &order);
    let root_seated = state
        .order(&root)
        .map_or_else(|| seated.clone(), |root| root.seated.clone());
    let root_text = order_text(&root_seated);
    let spec = spec(&root_text);
    let cwd = PathBuf::from(&seated.cwd);
    let pane = known.pane.clone();
    let ports = move || live_ports(pane.as_deref());
    let head_now = rev(&cwd, "HEAD");
    let base_now = rev(
        &project.root,
        &format!("refs/heads/{}", integration_branch(run)),
    );
    let dirty = uncommitted(&cwd)
        .map(|paths| !paths.is_empty())
        .unwrap_or(true);
    // A returning review reruns nothing when the trees are the ones the axes ran on.
    let reuse = sidecar.last.as_ref().is_some_and(|last| {
        !dirty
            && last.head.is_some()
            && last.head == head_now
            && last.base == base_now
            && failing(&last.axes).is_empty()
    });
    sidecar.gates += 1;
    let det = if reuse {
        sidecar.last.clone().unwrap_or_default()
    } else {
        let log = gate_dir(project, run).join(format!("{order}-g{}.log", sidecar.gates));
        deterministic(
            &Subject {
                root: &project.root,
                cwd: &cwd,
                run,
                order: &order,
                branch: seated.branch.as_deref(),
                class: &seated.class,
                report: known.report.as_ref(),
                spec: &spec,
                ports: &ports,
            },
            Some(&log),
        )
    };
    sidecar.last = Some(det.clone());
    let mut axes = det.axes.clone();
    // Held-out: the chain root's check, run at this order's head.
    let root_sidecar = if root == order {
        sidecar.clone()
    } else {
        read_sidecar(project, run, &root)
    };
    if failing(&axes).is_empty() {
        if let Some(heldout) = &root_sidecar.heldout {
            let line = heldout.command.replace("{dir}", &heldout.dir);
            let result = run_command(&cwd, &line, Duration::from_secs(TEST_TIMEOUT_S));
            let _ = std::fs::write(
                gate_dir(project, run).join(format!("{order}-g{}-heldout.log", sidecar.gates)),
                &result.output,
            );
            axes.push(heldout_axis(&result));
        } else if let Some(author) = root_sidecar
            .authors
            .last()
            .filter(|_| root_sidecar.heldout_skipped.is_none())
        {
            axes.push(Axis::new(
                "heldout",
                Mark::Wait,
                format!("{author} wrote the held-out check and it is not judged yet: `herdr gate run {author}`"),
            ));
        } else if feature_size(&root_seated.class, root_seated.size)
            || root_sidecar.heldout_skipped.is_some()
        {
            axes.push(Axis::new(
                "heldout",
                Mark::Note,
                root_sidecar.heldout_skipped.clone().unwrap_or_else(|| {
                    "no held-out check was written before the build (decision 0019: said, never silent)".into()
                }),
            ));
        }
    }
    // The review, last: a reviewer reads work that already passes the rest.
    let mut next = Vec::new();
    if failing(&axes).is_empty()
        && !axes.iter().any(|axis| axis.mark == Mark::Wait)
        && load_bearing(table, &seated.class)
    {
        match review_state(state, &sidecar) {
            ReviewState::None => {
                let line =
                    request_review(project, run, known, &root_text, &det, &mut sidecar, table)?;
                axes.push(Axis::new(
                    "review",
                    Mark::Wait,
                    "a reviewer of another family is booked; start it",
                ));
                next.push(line);
            }
            ReviewState::Pending(id) => {
                axes.push(Axis::new(
                    "review",
                    Mark::Wait,
                    format!("{id} has not delivered its verdict"),
                ));
            }
            ReviewState::Verdict(id, report) => {
                // The reviewer's own gate, when it was not gated by name: its report is well formed.
                if let Some(review) = state.order(&id).filter(|review| review.verdict.is_none()) {
                    let _ = gate_reviewer(journal, review, run);
                }
                axes.push(review_axis(&id, &report));
            }
        }
    }
    write_sidecar(project, run, &sidecar).map_err(|err| err.to_string())?;
    if axes.iter().any(|axis| axis.mark == Mark::Wait) {
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: "wait".into(),
            axes,
            next,
        });
    }
    let stream = stream_text(project, run, &order);
    if failing(&axes).is_empty() {
        // Lessons: the order passed; the quote must be in its own stream (§7.4.3).
        if let Some(lesson) = known
            .report
            .as_ref()
            .and_then(|report| report.get("lesson"))
        {
            let verdict = crate::nirvana_lessons::verify_and_write(
                &project.root,
                lesson,
                true,
                &stream,
                &order,
                run,
                now(),
                crate::nirvana_lessons::has_github_remote(&project.root),
            );
            axes.push(match verdict {
                crate::nirvana_lessons::Verdict::Written(path) => {
                    if let Some(home) = crate::nirvana::home() {
                        let _ = crate::nirvana_lessons::write_index_line(&home, &project.root);
                    }
                    Axis::new(
                        "lesson",
                        Mark::Pass,
                        format!("written to {}", path.display()),
                    )
                }
                crate::nirvana_lessons::Verdict::Refused(why) => {
                    Axis::new("lesson", Mark::Note, format!("not written: {why}"))
                }
            });
        }
        let _ = crate::nirvana_lessons::note_gated(&project.root, &stream, &order, true);
        journal
            .append(
                now(),
                Kind::Gated {
                    order: order.clone(),
                    verdict: Outcome::Pass.word().into(),
                    reason: None,
                    axes: axes_values(&axes),
                },
            )
            .map_err(|err| err.to_string())?;
        record(project, run, &order);
        next.push(format!("herdr gate merge {order} --run {run}"));
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: Outcome::Pass.word().into(),
            axes,
            next,
        });
    }
    let _ = crate::nirvana_lessons::note_gated(&project.root, &stream, &order, false);
    send_back(
        project, run, journal, state, known, &root, &root_text, axes, table,
    )
}

/// The record line, once, from the journal as it now stands.
fn record(project: &Project, run: &str, order: &str) {
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    if let Some(known) = state.order(order) {
        let _ = nirvana_run::write_record(run, known, nirvana_run::journal_ref(&journal, &state));
    }
}

#[allow(clippy::too_many_arguments)]
fn send_back(
    project: &Project,
    run: &str,
    journal: &Journal,
    state: &nirvana_run::RunState,
    known: &OrderState,
    root: &str,
    root_text: &str,
    axes: Vec<Axis>,
    table: Option<&Table>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    let reason = reason_line(&axes);
    let ask = |question: String, axes: Vec<Axis>| -> Result<Ruling, String> {
        journal
            .transact(now(), |_| {
                Ok((
                    vec![
                        Kind::Gated {
                            order: order.clone(),
                            verdict: Outcome::Asked.word().into(),
                            reason: reason.clone(),
                            axes: axes_values(&axes),
                        },
                        Kind::Asked {
                            order: Some(order.clone()),
                            question: question.clone(),
                        },
                    ],
                    (),
                ))
            })
            .map_err(|err| err.to_string())?;
        record(project, run, &order);
        Ok(Ruling {
            order: order.clone(),
            run: run.to_owned(),
            verdict: Outcome::Asked.word().into(),
            axes,
            next: vec![question],
        })
    };
    let (id, n) = match plan_send_back(state, &order) {
        SendBack::Ask => {
            return ask(
                format!(
                    "{order}: {SEND_BACK_CAP} send-backs are spent and the gate still finds {}; take it over, change the order, or drop it?",
                    reason.clone().unwrap_or_default()
                ),
                axes,
            )
        }
        SendBack::Child { id, n } => (id, n),
    };
    let Some(table) = table else {
        return ask(
            format!("{order} must go back, but seats.toml is refused"),
            axes,
        );
    };
    let seated = &known.seated;
    let brief = send_back_brief(&order, root, n, &axes, root_text, &integration_branch(run));
    let mut request = Request::new(&seated.class);
    request.size = seated.size;
    // Plan 29 §5.2: an order seated for a project MCP server goes back on a claude seat or not at all.
    let needs = match crate::nirvana_gui_tools::seat_needs(table, &project.root, &seated.needs) {
        Ok(needs) => needs,
        Err(why) => return ask(format!("{order} must go back, but {why}"), axes),
    };
    (request.needs, request.claude_only) = (needs.rest, needs.claude_only);
    request.not_family = seated.not_family.clone();
    request.mode = seated.mode;
    request.order = Some(id.clone());
    if let Some(reason) = &seated.override_reason {
        request.override_seat = Some((
            format!("{}:{}", seated.seat, seated.effort.word()),
            reason.clone(),
        ));
        request.inherited_override = true;
    }
    let mut world = nirvana_seat::live_world(table);
    let leases_path = nirvana_seat::dir().join(nirvana_seat::LEASES_FILE);
    if let Ok(leases) = nirvana_seat::load_leases(&leases_path) {
        world.leases = leases.leases.clone();
        world.overrides_used = leases.overrides(run);
    }
    let (pick, why_seated, child_override_reason) =
        match nirvana_seat::pick(table, &world, &request) {
            Ok(pick) => (pick, None, seated.override_reason.clone()),
            Err(no_seat) if request.inherited_override => {
                let mut fallback_request = request.clone();
                fallback_request.override_seat = None;
                fallback_request.inherited_override = false;
                match nirvana_seat::pick(table, &world, &fallback_request) {
                    Ok(pick) => {
                        let seat_name = &seated.seat;
                        let why = format!(
                            "inherited override {} refused: {}; the rule picked",
                            seat_name, no_seat.reason
                        );
                        (pick, Some(why), None)
                    }
                    Err(err) => {
                        return ask(
                            format!("{order} must go back, and no seat is left: {}", err.reason),
                            axes,
                        );
                    }
                }
            }
            Err(no_seat) => {
                return ask(
                    format!(
                        "{order} must go back, and no seat is left: {}",
                        no_seat.reason
                    ),
                    axes,
                )
            }
        };
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    let cwd = PathBuf::from(&seated.cwd);
    let order_file = crate::nirvana_worker::write_order(project, &cwd, &id, &brief)
        .map_err(|err| err.to_string())?;
    let child = Seated {
        order: id.clone(),
        seat: pick.seat.clone(),
        cli: pick.cli.clone(),
        model: pick.model.clone(),
        effort: pick.effort,
        pool: pick.pool.clone(),
        class: seated.class.clone(),
        chain: Some(pick.chain.clone()),
        size: seated.size,
        mode: seated.mode,
        needs: seated.needs.clone(),
        not_family: seated.not_family.clone(),
        reason: why_seated.unwrap_or_else(|| pick.reason.clone()),
        override_reason: child_override_reason,
        order_file,
        cwd: seated.cwd.clone(),
        worktree: seated.worktree.clone(),
        branch: seated.branch.clone(),
        parent: Some(order.clone()),
        why: Some("send_back".into()),
        rule_pick: None,
        bar_dropped: Some(pick.bar_dropped),
        step_down: Some(pick.step_down),
        use_basis: pick.use_basis.clone(),
        expected: pick
            .windows
            .iter()
            .map(|w| (w.window.clone(), w.expected))
            .collect(),
        jev_pick: None,
    };
    journal
        .transact(now(), |_| {
            Ok((
                vec![
                    Kind::Gated {
                        order: order.clone(),
                        verdict: Outcome::SentBack.word().into(),
                        reason: reason.clone(),
                        axes: axes_values(&axes),
                    },
                    Kind::Seated(Box::new(child.clone())),
                ],
                (),
            ))
        })
        .map_err(|err| err.to_string())?;
    record(project, run, &order);
    let how = crate::nirvana_worker::launch_worker(project, run, &child).unwrap_or_else(|err| err);
    Ok(Ruling {
        order: order.clone(),
        run: run.to_owned(),
        verdict: Outcome::SentBack.word().into(),
        axes,
        next: vec![format!(
            "{id} seated on {} {} ({how}): send-back {n} of {SEND_BACK_CAP}",
            pick.seat,
            pick.effort.word()
        )],
    })
}

enum ReviewState {
    None,
    Pending(String),
    Verdict(String, Value),
}

fn review_state(state: &nirvana_run::RunState, sidecar: &Sidecar) -> ReviewState {
    let Some(id) = sidecar.reviews.last() else {
        return ReviewState::None;
    };
    match state.order(id) {
        Some(review) => match (&review.report, review.phase) {
            (Some(report), Phase::Delivered | Phase::Gated | Phase::Asked) => {
                ReviewState::Verdict(id.clone(), report.clone())
            }
            (_, Phase::Failed | Phase::Interrupted | Phase::Reseated) => ReviewState::None,
            _ => ReviewState::Pending(id.clone()),
        },
        None => ReviewState::Pending(id.clone()),
    }
}

/// The reviewer's Work Report as an axis: `done` approves, `blocked` sends back with its gotchas, `needs_answer`
/// waits for the owner (a judgment herdr does not make).
pub fn review_axis(id: &str, report: &Value) -> Axis {
    let summary = report
        .get("summary")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    match report.get("status").and_then(Value::as_str) {
        Some("done") => Axis::new("review", Mark::Pass, format!("{id} approves: {summary}")),
        Some("needs_answer") => Axis::new(
            "review",
            Mark::Wait,
            format!(
                "{id} cannot judge it without the owner: {}",
                report
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or(&summary)
            ),
        ),
        _ => Axis::new(
            "review",
            Mark::Fail,
            format!("{id} sends it back: {summary}"),
        )
        .with(
            report
                .get("gotchas")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn request_review(
    project: &Project,
    run: &str,
    known: &OrderState,
    root_text: &str,
    det: &Det,
    sidecar: &mut Sidecar,
    table: Option<&Table>,
) -> Result<String, String> {
    let table = table.ok_or("seats.toml is refused: no reviewer can be picked")?;
    let order = known.id();
    let family = family_of(Some(table), &known.seated.seat);
    let id = format!("{order}-rev{}", sidecar.reviews.len() + 1);
    let world = nirvana_seat::live_world(table);
    let (pick, same) = pick_other_family(
        table,
        &world,
        "review.adversary",
        &family,
        known.seated.mode,
        &id,
        "SAME-FAMILY REVIEW",
    )
    .map_err(|no_seat| format!("no reviewer: {}", no_seat.reason))?;
    let diff_path = gate_dir(project, run).join(format!("{id}.diff"));
    let diff = match (&det.base, &det.head) {
        (Some(base), Some(head)) => {
            git(&project.root, &["diff", "--no-color", base, head]).unwrap_or_default()
        }
        _ => git(
            Path::new(&known.seated.cwd),
            &["diff", "--no-color", "HEAD"],
        )
        .unwrap_or_default(),
    };
    std::fs::create_dir_all(gate_dir(project, run)).map_err(|err| err.to_string())?;
    std::fs::write(&diff_path, diff).map_err(|err| err.to_string())?;
    let text = review_brief(
        order,
        root_text,
        &known.seated.cwd,
        &diff_path.display().to_string(),
        same,
    );
    let file = write_order_file(project, &id, &text).map_err(|err| err.to_string())?;
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    sidecar.reviews.push(id.clone());
    write_sidecar(
        project,
        run,
        &Sidecar {
            order: id.clone(),
            helps: Some(("review".into(), order.to_owned())),
            ..Sidecar::default()
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(seat_run_line(
        &pick,
        &file,
        &id,
        run,
        "review.adversary",
        (!same).then_some(family.as_str()),
    ))
}

/// A reviewer's own gate: its report is well formed. Its verdict is its subject's review axis.
fn gate_reviewer(journal: &Journal, known: &OrderState, run: &str) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    if known.verdict.is_some() {
        return Ok(Ruling {
            order,
            run: run.to_owned(),
            verdict: "pass".into(),
            axes: Vec::new(),
            next: Vec::new(),
        });
    }
    // A reviewer that needs the owner is `asked` by O3, with its report kept: its subject's review axis waits.
    if !matches!(known.phase, Phase::Delivered | Phase::Asked) {
        return Err(format!("the reviewer {order} is {}", known.phase.word()));
    }
    let axis = report_or_verdict(known.report.as_ref());
    let verdict = if axis.mark == Mark::Fail {
        Outcome::Failed
    } else {
        Outcome::Pass
    };
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.clone(),
                verdict: verdict.word().into(),
                reason: (axis.mark == Mark::Fail).then(|| format!("report: {}", axis.why)),
                axes: axes_values(std::slice::from_ref(&axis)),
            },
        )
        .map_err(|err| err.to_string())?;
    Ok(Ruling {
        order,
        run: run.to_owned(),
        verdict: verdict.word().into(),
        axes: vec![axis],
        next: Vec::new(),
    })
}

/// A reviewer's or author's report need only be well formed: `blocked` is a review's verdict, not a failure.
fn report_or_verdict(report: Option<&Value>) -> Axis {
    match report.map(crate::nirvana_worker::check_report) {
        Some(Ok(())) => Axis::new("report", Mark::Pass, "well formed"),
        Some(Err(why)) => Axis::new("report", Mark::Fail, why),
        None => Axis::new("report", Mark::Fail, "no Work Report was delivered"),
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    let mut copied = 0;
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copied += copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// Gating a held-out author's order: capture its check beside the journal, take it out of the author's folder,
/// and judge it at the base (decision 0019 §2.3). Invalid: one retry with the reason; invalid twice: the build goes
/// on without one, said in every gate of it.
fn gate_author(
    project: &Project,
    run: &str,
    journal: &Journal,
    known: &OrderState,
    subject: &str,
    table: Option<&Table>,
) -> Result<Ruling, String> {
    let order = known.id().to_owned();
    if known.phase != Phase::Delivered {
        return Err(format!(
            "the held-out author {order} is {}",
            known.phase.word()
        ));
    }
    let cwd = PathBuf::from(&known.seated.cwd);
    let found = cwd.join(nirvana_run::DIR).join("heldout");
    let captured = project.run_dir(run).join("heldout").join(subject);
    let mut axes = vec![report_or_verdict(known.report.as_ref())];
    let judged: Result<Heldout, String> = (|| {
        if !found.is_dir() {
            return Err("the author wrote no .nirvana/heldout/ folder".to_owned());
        }
        let _ = std::fs::remove_dir_all(&captured);
        copy_dir(&found, &captured).map_err(|err| err.to_string())?;
        std::fs::remove_dir_all(&found).map_err(|err| err.to_string())?;
        let changed = uncommitted(&cwd)?;
        if !changed.is_empty() {
            return Err(format!(
                "the author changed repository files: {}",
                changed.join(", ")
            ));
        }
        let command = std::fs::read_to_string(captured.join("COMMAND"))
            .map_err(|_| "the check has no COMMAND file".to_owned())?
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .to_owned();
        if !command.contains("{dir}") {
            return Err("COMMAND does not name {dir}".to_owned());
        }
        let line = command.replace("{dir}", &captured.display().to_string());
        let result = run_command(&cwd, &line, Duration::from_secs(TEST_TIMEOUT_S));
        let base_cases = cases(&result.output);
        heldout_valid(&base_cases)?;
        Ok(Heldout {
            author: order.clone(),
            dir: captured.display().to_string(),
            command,
            base_cases,
        })
    })();
    let mut subject_card = read_sidecar(project, run, subject);
    let mut next = Vec::new();
    let verdict = match judged {
        Ok(heldout) => {
            let failing = heldout.base_cases.iter().filter(|(_, pass)| !pass).count();
            axes.push(Axis::new(
                "heldout",
                Mark::Pass,
                format!(
                    "{} case(s), {failing} failing at the base: valid",
                    heldout.base_cases.len()
                ),
            ));
            subject_card.heldout = Some(heldout);
            next.push(format!(
                "the build may start: herdr seat run ... --order-id {subject} --run {run}"
            ));
            Outcome::Pass
        }
        Err(why) => {
            axes.push(Axis::new("heldout", Mark::Fail, why.clone()));
            if subject_card.authors.len() < AUTHOR_TRIES {
                // `author` writes the subject's card itself (the new author's id); read it back after.
                let retried = author(project, run, subject, &why, table);
                subject_card = read_sidecar(project, run, subject);
                match retried {
                    Ok(line) => next.push(line),
                    Err(err) => {
                        subject_card.heldout_skipped = Some(format!(
                            "no held-out check: {why}; the retry could not be booked: {err}"
                        ));
                    }
                }
            } else {
                subject_card.heldout_skipped = Some(format!(
                    "no held-out check: refused twice, last because {why}"
                ));
                next.push(format!("the build goes on without a held-out check: {why}"));
            }
            Outcome::Failed
        }
    };
    write_sidecar(project, run, &subject_card).map_err(|err| err.to_string())?;
    journal
        .append(
            now(),
            Kind::Gated {
                order: order.clone(),
                verdict: verdict.word().into(),
                reason: reason_line(&axes),
                axes: axes_values(&axes),
            },
        )
        .map_err(|err| err.to_string())?;
    record(project, run, &order);
    let _ = crate::nirvana_worker::cli(&["cleanup".to_owned(), order.clone()]);
    Ok(Ruling {
        order,
        run: run.to_owned(),
        verdict: verdict.word().into(),
        axes,
        next,
    })
}

/// `herdr gate author <order>`: book and write the held-out author's order for a builder that is booked and not
/// started (`herdr seat pick --book <order>` first: the author's family must differ from the builder's seat).
fn author(
    project: &Project,
    run: &str,
    subject: &str,
    retry: &str,
    table: Option<&Table>,
) -> Result<String, String> {
    let table = table.ok_or("seats.toml is refused")?;
    let mut card = read_sidecar(project, run, subject);
    let request = card_request(project, subject)?;
    let lease = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))?
        .live(subject, now())
        .cloned()
        .ok_or(format!("{subject} has no booking: book the builder first, `herdr seat pick --class <c> --book {subject} --run {run}`"))?;
    let family = lease
        .pick
        .get("family")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| family_of(Some(table), &lease.seat));
    let id = format!("{subject}-acc{}", card.authors.len() + 1);
    let world = nirvana_seat::live_world(table);
    let (pick, same) = pick_other_family(
        table,
        &world,
        "acceptance.author",
        &family,
        table.mode,
        &id,
        "SAME-FAMILY AUTHOR",
    )
    .map_err(|no_seat| format!("no author: {}", no_seat.reason))?;
    let text = author_brief(subject, &request, (!retry.is_empty()).then_some(retry));
    let file = write_order_file(project, &id, &text).map_err(|err| err.to_string())?;
    book(table, &pick, &id, run).map_err(|err| err.to_string())?;
    card.authors.push(id.clone());
    write_sidecar(project, run, &card).map_err(|err| err.to_string())?;
    write_sidecar(
        project,
        run,
        &Sidecar {
            order: id.clone(),
            helps: Some(("author".into(), subject.to_owned())),
            ..Sidecar::default()
        },
    )
    .map_err(|err| err.to_string())?;
    Ok(format!(
        "{} --worktree",
        seat_run_line(
            &pick,
            &file,
            &id,
            run,
            "acceptance.author",
            (!same).then_some(family.as_str())
        )
    ))
}

/// The builder's order text for its author: the file `gate author --order` was given, kept beside the sidecars.
fn card_request(project: &Project, subject: &str) -> Result<String, String> {
    let path = project.orders_dir().join(format!("{subject}.request.md"));
    std::fs::read_to_string(&path)
        .map_err(|_| format!("no request for {subject}: pass --order <file>"))
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr gate run <order> [--run r] [--json]       rule on a delivered order (exit 0 pass, 1 sent back
                                                         or asked, 4 waiting for a review or a held-out check)
       herdr gate author <order> --order <file> [--run r]  book the held-out author for a booked builder
       herdr gate merge <order> [--run r]                 merge a passed order into nirvana-integrate/<run>
       herdr gate status [<order>] [--run r] [--json]";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

struct Args {
    positional: Option<String>,
    run: Option<String>,
    order_file: Option<PathBuf>,
    json: bool,
}

fn parse_args(args: &[String]) -> Args {
    let mut parsed = Args {
        positional: None,
        run: None,
        order_file: None,
        json: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--run" => parsed.run = Some(iter.next().cloned().unwrap_or_else(|| usage_exit())),
            "--order" => {
                parsed.order_file = Some(PathBuf::from(
                    iter.next().cloned().unwrap_or_else(|| usage_exit()),
                ))
            }
            "--json" => parsed.json = true,
            other if !other.starts_with("--") && parsed.positional.is_none() => {
                parsed.positional = Some(other.to_owned())
            }
            _ => usage_exit(),
        }
    }
    parsed
}

fn run_for(project: &Project, order: &str, run: Option<String>) -> String {
    if !nirvana_run::valid_id(order) {
        eprintln!("herdr gate: {order:?} is not an order id");
        std::process::exit(2);
    }
    match run.or_else(|| project.run_of(order)) {
        Some(run) if nirvana_run::valid_id(&run) => run,
        _ => {
            eprintln!("herdr gate: no run seats {order}");
            std::process::exit(1);
        }
    }
}

fn print_ruling(ruling: &Ruling, json: bool) -> std::io::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(ruling)?);
        return Ok(());
    }
    println!("{} · {}", ruling.order, ruling.verdict);
    for axis in &ruling.axes {
        let mark = match axis.mark {
            Mark::Pass => "pass",
            Mark::Fail => "FAIL",
            Mark::Note => "note",
            Mark::Wait => "wait",
        };
        println!("  {mark} {}: {}", axis.axis, axis.why);
        for line in axis.lines.iter().take(12) {
            println!("       {line}");
        }
    }
    for line in &ruling.next {
        println!("next: {line}");
    }
    Ok(())
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let rest = args.get(1..).unwrap_or(&[]);
    let parsed = parse_args(rest);
    match args.first().map(String::as_str) {
        Some("run") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let run = run_for(&project, &order, parsed.run);
            match gate(&project, &run, &order) {
                Ok(ruling) => {
                    print_ruling(&ruling, parsed.json)?;
                    std::process::exit(ruling.exit());
                }
                Err(why) => {
                    eprintln!("herdr gate run: {why}");
                    std::process::exit(2);
                }
            }
        }
        Some("author") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let Some(file) = parsed.order_file else {
                usage_exit()
            };
            let run = parsed
                .run
                .or_else(|| std::env::var("NIRVANA_RUN").ok())
                .unwrap_or_else(|| usage_exit());
            if !nirvana_run::valid_id(&order) || !nirvana_run::valid_id(&run) {
                usage_exit();
            }
            let text = std::fs::read_to_string(&file)?;
            project.ensure()?;
            std::fs::create_dir_all(project.orders_dir())?;
            std::fs::write(
                project.orders_dir().join(format!("{order}.request.md")),
                &text,
            )?;
            let table = nirvana_seat::table_for(&project.root).ok();
            match author(&project, &run, &order, "", table.as_ref()) {
                Ok(line) => {
                    println!("{line}");
                    println!(
                        "then, when it has delivered: herdr gate run {order}-acc1 --run {run}"
                    );
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr gate author: {why}");
                    std::process::exit(3);
                }
            }
        }
        Some("merge") => {
            let order = parsed.positional.clone().unwrap_or_else(|| usage_exit());
            let run = run_for(&project, &order, parsed.run);
            match merge_cli(&project, &run, &order) {
                Ok(line) => {
                    println!("{line}");
                    Ok(())
                }
                Err(why) => {
                    eprintln!("herdr gate merge: {why}");
                    std::process::exit(1);
                }
            }
        }
        Some("status") => status_cli(&project, parsed),
        _ => usage_exit(),
    }
}

fn merge_cli(project: &Project, run: &str, order: &str) -> Result<String, String> {
    let _lock = gate_lock(project).map_err(|err| err.to_string())?;
    let journal = project.journal(run);
    let state = replay(&journal.read().0);
    let known = state
        .order(order)
        .ok_or(format!("no order {order} in {run}"))?;
    if known.phase == Phase::Merged {
        return Ok(format!(
            "{order} is merged already ({})",
            known.merged_sha.clone().unwrap_or_default()
        ));
    }
    if known.verdict.as_deref() != Some(Outcome::Pass.word()) {
        return Err(format!(
            "{order}'s gate verdict is {}: only a passed order merges",
            known.verdict.as_deref().unwrap_or("not given")
        ));
    }
    if known.seated.branch.is_none() {
        return Err(format!(
            "{order} ran in the project's folder: its work is there already, nothing to merge"
        ));
    }
    let det = read_sidecar(project, run, order)
        .last
        .ok_or("the gate kept no record of its trees")?;
    let (Some(base), Some(head)) = (det.base, det.head) else {
        return Err("the gate kept no base or head".into());
    };
    let cwd = PathBuf::from(&known.seated.cwd);
    if rev(&cwd, "HEAD").as_deref() != Some(head.as_str()) {
        return Err(format!(
            "{order}'s branch moved since the gate: gate it again"
        ));
    }
    let sha = merge(&project.root, run, order, &base, &head)?;
    journal
        .append(
            now(),
            Kind::Merged {
                order: order.to_owned(),
                sha: Some(sha.clone()),
            },
        )
        .map_err(|err| err.to_string())?;
    drop(_lock);
    // O3's cleanup: the pane, then the worktree, then the booking; the chain's earlier orders shared the worktree.
    let (root, _) = chain(&state, order);
    let mut chain_orders = vec![order.to_owned()];
    let mut current = order.to_owned();
    while current != root {
        let Some(parent) = state
            .order(&current)
            .and_then(|known| known.seated.parent.clone())
        else {
            break;
        };
        chain_orders.push(parent.clone());
        current = parent;
    }
    for id in &chain_orders {
        let _ = crate::nirvana_worker::cli(&["cleanup".to_owned(), id.clone()]);
    }
    Ok(format!(
        "{order} merged into {} as {}; a merge to main is the owner's (plan 12 §6)",
        integration_branch(run),
        short_sha(&sha)
    ))
}

fn status_cli(project: &Project, args: Args) -> std::io::Result<()> {
    let runs = match (&args.run, &args.positional) {
        (Some(run), _) => vec![run.clone()],
        (None, Some(order)) => project.run_of(order).into_iter().collect(),
        (None, None) => project.runs(),
    };
    let mut rows = Vec::new();
    for run in &runs {
        let state = replay(&project.journal(run).read().0);
        for known in &state.orders {
            if args
                .positional
                .as_deref()
                .is_some_and(|order| order != known.id())
            {
                continue;
            }
            let card = read_sidecar(project, run, known.id());
            let (root, depth) = chain(&state, known.id());
            rows.push(serde_json::json!({
                "run": run,
                "order": known.id(),
                "phase": known.phase.word(),
                "verdict": known.verdict,
                "send_backs": depth,
                "root": root,
                "gates": card.gates,
                "reviews": card.reviews,
                "authors": card.authors,
                "heldout": card.heldout.as_ref().map(|h| h.base_cases.len()),
                "heldout_skipped": card.heldout_skipped,
                "helps": card.helps,
                "failing": card.last.as_ref().map(|det| failing(&det.axes).iter().map(|a| a.axis.clone()).collect::<Vec<_>>()),
            }));
        }
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if rows.is_empty() {
        println!("no order to show");
    }
    for row in rows {
        let text = |key: &str| row[key].as_str().unwrap_or("").to_owned();
        println!(
            "{} {} {} · gate {} · send-backs {} of {SEND_BACK_CAP}{}{}{}",
            text("run"),
            text("order"),
            text("phase"),
            row["verdict"].as_str().unwrap_or("not run"),
            row["send_backs"],
            row["reviews"]
                .as_array()
                .filter(|r| !r.is_empty())
                .map(|r| format!(
                    " · reviews {}",
                    r.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .unwrap_or_default(),
            row["heldout"]
                .as_u64()
                .map(|n| format!(" · held-out {n} case(s)"))
                .unwrap_or_default(),
            row["helps"]
                .as_array()
                .map(|h| format!(
                    " · {} for {}",
                    h[0].as_str().unwrap_or(""),
                    h[1].as_str().unwrap_or("")
                ))
                .unwrap_or_default(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
