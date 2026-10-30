//! Nirvana fork, plan 12 O3: the workers. `herdr seat run|exec|status|read|stop|cleanup`: the launch argv of
//! each CLI, the stream a worker prints, its Work Report, the pane it runs in, the job object that ends its
//! process tree, and what herdr does when a worker's window runs out or its stream goes silent.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §4, §5.6, §7.1, §7.2)
//!
//! `herdr seat run` books the order (O2's lease), writes `seated` to the run's journal and opens a pane named
//! `<seat>·<order>` in the run's tab whose shell runs `herdr seat exec <order> --run <run>`. `seat exec` is the
//! worker's supervisor: it starts the CLI headless with a list argv (never a shell: Git Bash rewrote `/usage`
//! into a path twice on 2026-10-17), tees the raw stream to `streams/<order>.jsonl` beside the journal, writes
//! the events the stream carries (usage, a rate limit, the report) to the journal, and draws a readable view in
//! the pane. Outside a herdr pane there is no tab to open, so `seat run` starts `seat exec` detached instead,
//! with its view in `streams/<order>.log`.
//!
//! The launch lessons, each from an incident in old Nirvana's adapters (plan 12 §4):
//! - claude runs with `--setting-sources project --strict-mcp-config`: without it one "PONG" turn cost 16x,
//!   from 120,104 cache-creation tokens of the owner's home configuration.
//! - claude runs with `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`: a print-mode background task died with its
//!   process.
//! - agy takes its prompt as one token `--print=<line>`: a bare `--print` took the next flag as its prompt.
//! - Continuation is `--resume <id>` (claude) and `--conversation <id>` (agy), never `--continue`, which picked
//!   the wrong conversation in a folder that had run twice.
//! - The order is a file and the prompt one short line naming it: the spec inlined in `--print=` hit Windows'
//!   32,767-character command line with `WinError 206` on 2026-10-17.
//! - The report is checked against the Work Report schema; an empty answer is a failure even when agy says
//!   `SUCCESS` (P2: flash ended with an empty answer and `status: SUCCESS`).

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::nirvana_run::{
    self, ensure_dir, now, rate_decision, replay, utc, valid_id, Event, Journal, Kind, Phase,
    Project, RateDecision, Seated,
};
use crate::nirvana_seat::{self, Effort, Mode, SizeName, Tokens};
use crate::nirvana_usage::Reading;

/// The Work Report every worker ends with, passed to both CLIs as `--json-schema` (P3: both take it).
/// `lesson` is O5's: a worker's proposed lesson, which the gate checks before it is written.
pub const REPORT_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["status","summary","files","tests","gotchas"],"properties":{"status":{"type":"string","enum":["done","blocked","needs_answer"]},"summary":{"type":"string","minLength":1},"question":{"type":"string"},"files":{"type":"array","items":{"type":"string"}},"tests":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["name","result"],"properties":{"name":{"type":"string"},"result":{"type":"string","enum":["pass","fail","not_run"]}}}},"gotchas":{"type":"array","items":{"type":"string"}},"lesson":{"type":"object","additionalProperties":false,"required":["name","class","steps","evidence"],"properties":{"name":{"type":"string","minLength":1},"class":{"type":"string","minLength":1},"steps":{"type":"string","minLength":1},"evidence":{"type":"string","minLength":1}}},"lesson_ops":{"type":"array","items":{"type":"object","required":["op"],"properties":{"op":{"type":"string","enum":["new","patch","merge"]},"slug":{"type":"string"},"base_sha":{"type":"string"},"name":{"type":"string"},"class":{"type":"string"},"steps":{"type":"string"},"evidence":{"type":"string"},"slugs":{"type":"array","items":{"type":"string"}}}}},"sources":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["url","used_for"],"properties":{"url":{"type":"string","minLength":1},"used_for":{"type":"string","minLength":1}}}}}}"#;

/// The schema's properties, which the check below allows and nothing else.
pub const REPORT_KEYS: [&str; 9] = [
    "status",
    "summary",
    "question",
    "files",
    "tests",
    "gotchas",
    "lesson",
    "sources",
    "lesson_ops",
];

/// The keys agy adds to its structured answer (P3: "the response is the object, plus agy's own `toolAction`
/// and `toolSummary` keys"); dropped before the check.
const AGY_EXTRA_KEYS: [&str; 2] = ["toolAction", "toolSummary"];
/// §7.4.3: a lesson's steps are at most 120 lines.
const LESSON_LINES: usize = 120;

/// The Work Report's own check. herdr carries no JSON Schema crate, so this is the schema above in code; a
/// test holds the two together on the same fixtures.
pub fn check_report(report: &Value) -> Result<(), String> {
    let object = report
        .as_object()
        .ok_or("the Work Report is not a JSON object")?;
    if object.is_empty() {
        return Err("the Work Report is empty".into());
    }
    if let Some(key) = object
        .keys()
        .find(|key| !REPORT_KEYS.contains(&key.as_str()))
    {
        return Err(format!("the Work Report has an unknown key {key:?}"));
    }
    match object.get("status").and_then(Value::as_str) {
        Some("done" | "blocked" | "needs_answer") => {}
        Some(other) => {
            return Err(format!(
                "status {other:?} is not done, blocked or needs_answer"
            ))
        }
        None => return Err("the Work Report has no status".into()),
    }
    match object.get("summary").and_then(Value::as_str) {
        Some(summary) if !summary.trim().is_empty() => {}
        _ => return Err("the Work Report has no summary".into()),
    }
    if object.get("question").is_some_and(|q| !q.is_string()) {
        return Err("question is not a string".into());
    }
    for key in ["files", "gotchas"] {
        let list = object
            .get(key)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{key} is not a list"))?;
        if list.iter().any(|item| !item.is_string()) {
            return Err(format!("{key} holds something that is not a string"));
        }
    }
    let tests = object
        .get("tests")
        .and_then(Value::as_array)
        .ok_or("tests is not a list")?;
    for test in tests {
        let test = test.as_object().ok_or("a test is not an object")?;
        if test.keys().any(|key| key != "name" && key != "result") {
            return Err("a test has a key other than name and result".into());
        }
        if !test.get("name").is_some_and(Value::is_string) {
            return Err("a test has no name".into());
        }
        if !matches!(
            test.get("result").and_then(Value::as_str),
            Some("pass" | "fail" | "not_run")
        ) {
            return Err("a test's result is not pass, fail or not_run".into());
        }
    }
    if let Some(lesson) = object.get("lesson") {
        let lesson = lesson.as_object().ok_or("lesson is not an object")?;
        for key in ["name", "class", "steps", "evidence"] {
            match lesson.get(key).and_then(Value::as_str) {
                Some(text) if !text.trim().is_empty() => {}
                _ => return Err(format!("the lesson has no {key}")),
            }
        }
        if lesson
            .keys()
            .any(|key| !["name", "class", "steps", "evidence"].contains(&key.as_str()))
        {
            return Err("the lesson has an unknown key".into());
        }
        let lines = lesson["steps"].as_str().unwrap_or("").lines().count();
        if lines > LESSON_LINES {
            return Err(format!(
                "the lesson's steps run {lines} lines, over {LESSON_LINES}"
            ));
        }
    }
    // Plan 33 §3.3: a lesson review's ops; herdr checks each one when it gates the review.
    if object.get("lesson_ops").is_some_and(|ops| {
        !ops.as_array()
            .is_some_and(|items| items.iter().all(Value::is_object))
    }) {
        return Err("lesson_ops is not a list of objects".into());
    }
    if let Some(sources) = object.get("sources") {
        let Some(items) = sources.as_array() else {
            return Err("the Work Report sources is not an array".into());
        };
        for (i, item) in items.iter().enumerate() {
            let Some(obj) = item.as_object() else {
                return Err(format!("the Work Report sources[{i}] is not an object"));
            };
            if let Some(key) = obj
                .keys()
                .find(|k| k.as_str() != "url" && k.as_str() != "used_for")
            {
                return Err(format!(
                    "the Work Report sources[{i}] has an unknown key {key:?}"
                ));
            }
            if obj
                .get("url")
                .and_then(Value::as_str)
                .is_none_or(|s| s.is_empty())
            {
                return Err(format!("the Work Report sources[{i}] has no url"));
            }
            if obj
                .get("used_for")
                .and_then(Value::as_str)
                .is_none_or(|s| s.is_empty())
            {
                return Err(format!("the Work Report sources[{i}] has no used_for"));
            }
        }
    }
    Ok(())
}

// ---- the launch ----

/// What a worker is started with. The prompt is one short line; the order is a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub cli: String,
    pub model: String,
    pub effort: Effort,
    pub prompt: String,
    /// The worker's own conversation, resumed within its own seat (§7.2).
    pub resume: Option<String>,
    /// agy's `--print-timeout`, seconds; 0 waits for the turn.
    pub print_timeout_s: u64,
}

/// Environment a claude worker gets: background tasks died with a print-mode process.
pub const CLAUDE_ENV: [(&str, &str); 1] = [("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS", "1")];

/// What a worker must not inherit from the orchestrator that started it: a worker is not a child session
/// of the orchestrator's Claude (the same list herdr's `pane.rs` removes from a new pane).
pub const ENV_REMOVED: [&str; 6] = [
    "CODEX_THREAD_ID",
    "OMPCODE",
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_MESSAGING_TOKEN",
];

/// The worker's argv, program first. A list, never a shell line.
pub fn argv(spec: &LaunchSpec) -> Result<Vec<String>, String> {
    let mut argv: Vec<String> = Vec::new();
    match spec.cli.as_str() {
        "claude" => {
            argv.extend(
                [
                    "claude",
                    "-p",
                    &spec.prompt,
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--setting-sources",
                    "project",
                    "--strict-mcp-config",
                    "--model",
                    &spec.model,
                    "--effort",
                    spec.effort.word(),
                    // R6: "Auto mode for both is fine". claude 2.1.302's `--permission-mode` lists `auto`.
                    "--permission-mode",
                    "auto",
                    "--json-schema",
                    REPORT_SCHEMA,
                ]
                .map(str::to_owned),
            );
            if let Some(id) = &spec.resume {
                argv.extend(["--resume".to_owned(), id.clone()]);
            }
        }
        "agy" => {
            argv.extend(
                [
                    "agy",
                    "--output-format",
                    "stream-json",
                    // The effort is inside the model id (`gemini-3.8-flash-low`), the one way herdr names it.
                    "--model",
                    &spec.model,
                    "--print-timeout",
                    &format!("{}s", spec.print_timeout_s),
                    // R6: agy's own auto mode is "available on Linux and macOS, with Windows support coming
                    // later" (its changelog, read on 1.4.2), so the worker skips prompts inside its worktree and
                    // its job object, as old Nirvana's adapter did.
                    "--dangerously-skip-permissions",
                    "--json-schema",
                    REPORT_SCHEMA,
                ]
                .map(str::to_owned),
            );
            if let Some(id) = &spec.resume {
                argv.extend(["--conversation".to_owned(), id.clone()]);
            }
            argv.push(format!("--print={}", spec.prompt));
        }
        other => return Err(format!("herdr cannot run a {other} worker")),
    }
    Ok(argv)
}

/// Plan 13 §2 ("Workers. A Claude worker that `herdr seat run` opens in a pane for this folder gets the same
/// file"): a claude worker's argv with `--settings <file>` added when the project folder has a generated preset
/// file. The worker's file holds the preset's switches alone: the plugin's own settings set
/// `NIRVANA_ORCHESTRATOR=1`, which is the orchestrator's mark, not a worker's. An agy worker is untouched (§10: "No
/// preset for agy or Codex panes").
pub fn with_preset_settings(mut argv: Vec<String>, settings: Option<&Path>) -> Vec<String> {
    if let Some(file) = settings {
        if argv.first().map(String::as_str) == Some("claude") {
            argv.extend(["--settings".to_owned(), file.display().to_string()]);
        }
    }
    argv
}

/// Plan 28 §3: the file `with_preset_settings` adds for this worker. On an Opus Claude worker it is the folder's
/// preset with every superpowers plugin off, or those entries alone when the folder has no preset; on any other it is
/// the preset, as before. An error keeps the preset and says so in the worker's view, one line: a worker never fails
/// to start because of this rule.
pub fn worker_settings(
    dir: &Path,
    spec: &LaunchSpec,
    folder: &Path,
    preset: Option<PathBuf>,
    claude_dir: Option<&Path>,
    out: &mut dyn Write,
) -> Option<PathBuf> {
    match crate::nirvana_superpowers::worker_settings_in(
        dir,
        &spec.cli,
        &spec.model,
        folder,
        preset.as_deref(),
        claude_dir,
    ) {
        Ok(settings) => settings,
        Err(why) => {
            let _ = writeln!(
                out,
                "superpowers stay as the settings say for {}: {why}",
                spec.model
            );
            preset
        }
    }
}

/// The worker's argv with its settings file: the folder's worker preset (plan 13), with the superpowers rule over it
/// (plan 28). `dir` is `<home>/nirvana`, given so a test never reaches the installed home.
pub fn worker_argv(
    dir: &Path,
    spec: &LaunchSpec,
    cwd: &Path,
    root: &Path,
    claude_dir: Option<&Path>,
    out: &mut dyn Write,
) -> Result<Vec<String>, String> {
    let argv = argv(spec)?;
    let preset = crate::nirvana_preset::worker_settings_in(dir, cwd, root);
    let settings = worker_settings(dir, spec, cwd, preset, claude_dir, out);
    Ok(with_preset_settings(argv, settings.as_deref()))
}

/// Plan 29 §5.1: a claude worker's argv with the MCP servers its order needs, from the project's `.mcp.json`; an
/// error when it cannot get one it was seated for (review F3).
fn with_order_servers(
    argv: Vec<String>,
    project: &Project,
    run: &str,
    seated: &Seated,
    out: &mut dyn Write,
) -> Result<Vec<String>, String> {
    let table = nirvana_seat::table();
    crate::nirvana_gui_tools::order_argv(
        argv,
        &project.root,
        &project.run_dir(run),
        &seated.order,
        &seated.needs,
        table
            .as_ref()
            .map(crate::nirvana_gui_tools::capabilities)
            .map_err(Clone::clone),
        out,
    )
}

/// Plan 33 §2.3: a worker of an order seated with a profile gets the run's copy, never the project's file. Its sha is
/// checked before every start; a copy gone or changed fails the order, as a server it cannot get does (plan 29 F3).
/// The order copy is brought to the worker's CLI: a send-back or reseat child quotes its parent's, which was written
/// for the parent's CLI, so an agy child gets the body at its top and a claude child loses the agy section.
pub fn with_profile(
    mut argv: Vec<String>,
    run_dir: &Path,
    seated: &Seated,
) -> Result<Vec<String>, String> {
    let file = Path::new(&seated.order_file);
    let Ok(text) = std::fs::read_to_string(file) else {
        return Ok(argv);
    };
    let Some(found) = crate::nirvana_profiles::in_order(&text) else {
        return Ok(argv);
    };
    let found = found.map_err(|why| format!("{}: {why}", seated.order))?;
    let parsed = crate::nirvana_profiles::from_run(run_dir, &found).map_err(|why| {
        format!(
            "{} was seated with profile {}: {why}",
            seated.order, found.name
        )
    })?;
    let wanted = crate::nirvana_profiles::order_text(&text, &found, &parsed, &seated.cli);
    if wanted != text {
        std::fs::write(file, &wanted)
            .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    }
    match seated.cli.as_str() {
        "claude" => argv.extend(crate::nirvana_profiles::claude_args(run_dir, &found, &parsed)?),
        _ if !parsed.disallowed_tools.is_empty() => {
            return Err(format!(
                "{} was seated with profile {}, which forbids tools only a claude worker can be denied, on {}",
                seated.order, found.name, seated.cli
            ))
        }
        _ => {}
    }
    Ok(argv)
}

/// The one line a worker is started with (§4).
pub fn first_line(order_file: &str) -> String {
    format!(
        "Do the work order in {order_file}. Work only in this folder. End with the Work Report your output schema asks for."
    )
}

/// The line a worker continues its own conversation with, after a stall or a deferral.
pub fn continue_line(order_file: &str, why: &str) -> String {
    format!(
        "Continue the work order in {order_file}; your last turn stopped ({why}). End with the Work Report your output schema asks for."
    )
}

/// What herdr adds to every order it copies: the rules a worker cannot be trusted to know.
pub const ORDER_FOOTER: &str = "\n\n---\nFrom herdr (plan 12 §4): do not start subagents or background processes unless this order allows it. \
Work only in this folder. Your last message is the Work Report: status (done, blocked or needs_answer), summary, \
files you changed, tests with their results, gotchas, and, when a step is worth keeping, a lesson.\n";
pub(crate) const FOOTER_MARK: &str = "From herdr (plan 12 §4)";

// ---- the stream ----

/// A typed rate-limit signal (§5.6): never guessed from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimit {
    /// "rate_limit_event", "api_retry" or "agy_exit_3".
    pub signal: String,
    pub pool: Option<String>,
    pub window: Option<String>,
    pub resets_at: Option<u64>,
    pub detail: String,
}

/// One thing the stream said, for the pane's view and the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    Session(String),
    Usage,
    Text(String),
    /// A call, as Claude Code names it: `Read(src/a.ts)`.
    Tool(String),
    /// What a call gave back. `label` names the call when it is not the one shown just before.
    ToolOut {
        label: Option<String>,
        ok: bool,
        lines: Vec<String>,
    },
    RateLimited(RateLimit),
    Result,
}

/// A worker's stream as read so far.
#[derive(Debug, Clone, Default)]
pub struct Stream {
    pub cli: String,
    pub session: Option<String>,
    pub tokens: Tokens,
    /// claude: usage per message id (one message streams as several lines with the same usage).
    by_message: BTreeMap<String, Tokens>,
    /// agy: usage per step index.
    by_step: BTreeMap<u64, Tokens>,
    pub last_text: Option<String>,
    agy_text: String,
    pub result: Option<Value>,
    /// claude's last `rate_limit_info`, for an `api_retry` that carries no reset.
    last_limit: Option<(Option<String>, Option<u64>)>,
    pub lines: u64,
    /// The worker's folder, dropped from the paths the view shows.
    cwd: Option<String>,
    /// Calls by id (claude's tool_use id, agy's step index): the raw tool name and the label shown.
    calls: BTreeMap<String, (String, String)>,
    /// The call shown last, so an answer right under it needs no label.
    last_call: Option<String>,
}

fn claude_window(kind: &str) -> Option<String> {
    match kind {
        "five_hour" => Some("5h".into()),
        "seven_day" => Some("week".into()),
        other if other.starts_with("seven_day_") => {
            Some(format!("week.{}", other.trim_start_matches("seven_day_")))
        }
        _ => None,
    }
}

fn u64_of(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn claude_tokens(usage: &Value) -> Tokens {
    Tokens {
        input: u64_of(usage, "input_tokens"),
        cache_write: u64_of(usage, "cache_creation_input_tokens"),
        cache_read: u64_of(usage, "cache_read_input_tokens"),
        output: u64_of(usage, "output_tokens"),
    }
}

/// agy's `output_tokens` holds its thinking: `total_tokens` is input plus output in P4's flash and argon
/// streams, with `thinking_tokens` inside it.
fn agy_tokens(usage: &Value) -> Tokens {
    Tokens {
        input: u64_of(usage, "input_tokens"),
        cache_write: 0,
        cache_read: u64_of(usage, "cache_read_tokens"),
        output: u64_of(usage, "output_tokens"),
    }
}

fn sum(values: impl Iterator<Item = Tokens>) -> Tokens {
    values.fold(Tokens::default(), |mut total, tokens| {
        total.input += tokens.input;
        total.cache_write += tokens.cache_write;
        total.cache_read += tokens.cache_read;
        total.output += tokens.output;
        total
    })
}

impl Stream {
    pub fn new(cli: &str) -> Self {
        Self {
            cli: cli.to_owned(),
            ..Self::default()
        }
    }

    /// The same stream, showing paths from the worker's folder.
    pub fn in_dir(mut self, cwd: &str) -> Self {
        self.cwd = Some(cwd.to_owned());
        self
    }

    fn call(&mut self, id: String, raw: &str, params: &Value) -> Option<Note> {
        if view::hidden(&self.cli, raw) {
            // Kept so that its answer is known, and hidden too.
            self.calls.insert(id, (raw.to_owned(), String::new()));
            return None;
        }
        let label = view::call_label(&self.cli, raw, params, self.cwd.as_deref());
        self.calls
            .insert(id.clone(), (raw.to_owned(), label.clone()));
        self.last_call = Some(id);
        Some(Note::Tool(label))
    }

    fn answer(
        &mut self,
        id: &str,
        answer: impl FnOnce(&str) -> (bool, Vec<String>),
    ) -> Option<Note> {
        let (raw, label) = self.calls.remove(id).unwrap_or_default();
        if view::hidden(&self.cli, &raw) {
            return None;
        }
        let (ok, lines) = answer(&raw);
        let label = (self.last_call.as_deref() != Some(id) && !label.is_empty()).then_some(label);
        Some(Note::ToolOut { label, ok, lines })
    }

    /// Reads one stdout line. A line that is not JSON is shown and otherwise ignored.
    pub fn feed(&mut self, line: &str) -> Vec<Note> {
        self.lines += 1;
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            let text = line.trim();
            return if text.is_empty() {
                Vec::new()
            } else {
                vec![Note::Text(text.to_owned())]
            };
        };
        if self.cli == "agy" {
            self.feed_agy(&value)
        } else {
            self.feed_claude(&value)
        }
    }

    fn feed_claude(&mut self, value: &Value) -> Vec<Note> {
        let mut notes = Vec::new();
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        let subtype = value.get("subtype").and_then(Value::as_str).unwrap_or("");
        if let Some(id) = value.get("session_id").and_then(Value::as_str) {
            if self.session.as_deref() != Some(id) {
                self.session = Some(id.to_owned());
                notes.push(Note::Session(id.to_owned()));
            }
        }
        match (kind, subtype) {
            ("assistant", _) => {
                let message = &value["message"];
                if let (Some(id), Some(usage)) = (
                    message.get("id").and_then(Value::as_str),
                    message.get("usage"),
                ) {
                    self.by_message.insert(id.to_owned(), claude_tokens(usage));
                    self.tokens = sum(self.by_message.values().copied());
                    notes.push(Note::Usage);
                }
                for block in message
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or("").to_owned();
                            if !text.trim().is_empty() {
                                self.last_text = Some(text.clone());
                                notes.push(Note::Text(text));
                            }
                        }
                        Some("tool_use") => {
                            let name = block["name"].as_str().unwrap_or("tool");
                            let id = block["id"].as_str().unwrap_or("").to_owned();
                            notes.extend(self.call(id, name, &block["input"]));
                        }
                        _ => {}
                    }
                }
            }
            ("user", _) => {
                for block in value
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter(|block| {
                        block.get("type").and_then(Value::as_str) == Some("tool_result")
                    })
                {
                    let id = block["tool_use_id"].as_str().unwrap_or("");
                    let is_error = block.get("is_error").and_then(Value::as_bool) == Some(true);
                    notes.extend(self.answer(id, |raw| {
                        view::claude_result(raw, &block["content"], is_error)
                    }));
                }
            }
            ("rate_limit_event", _) => {
                let info = &value["rate_limit_info"];
                let window = info
                    .get("rateLimitType")
                    .and_then(Value::as_str)
                    .and_then(claude_window);
                let resets_at = info.get("resetsAt").and_then(Value::as_u64);
                self.last_limit = Some((window.clone(), resets_at));
                if info.get("status").and_then(Value::as_str) == Some("rejected") {
                    notes.push(Note::RateLimited(RateLimit {
                        signal: "rate_limit_event".into(),
                        pool: Some("max".into()),
                        window,
                        resets_at,
                        detail: format!("rate_limit_event status rejected: {info}"),
                    }));
                }
            }
            ("system", "api_retry") => {
                if value.get("error").and_then(Value::as_str) == Some("rate_limit") {
                    let (window, resets_at) = self.last_limit.clone().unwrap_or((None, None));
                    notes.push(Note::RateLimited(RateLimit {
                        signal: "api_retry".into(),
                        pool: Some("max".into()),
                        window,
                        resets_at,
                        detail: format!("api_retry error=rate_limit: {value}"),
                    }));
                }
            }
            ("result", _) => {
                if let Some(usage) = value.get("usage") {
                    self.tokens = claude_tokens(usage);
                    notes.push(Note::Usage);
                }
                self.result = Some(value.clone());
                notes.push(Note::Result);
            }
            _ => {}
        }
        notes
    }

    fn feed_agy(&mut self, value: &Value) -> Vec<Note> {
        let mut notes = Vec::new();
        let event = value.get("event").and_then(Value::as_str).unwrap_or("");
        let conversation = value
            .get("conversation_id")
            .or_else(|| value.pointer("/step_update/conversation_id"))
            .or_else(|| value.pointer("/result/conversation_id"))
            .and_then(Value::as_str);
        if let Some(id) = conversation {
            if self.session.as_deref() != Some(id) {
                self.session = Some(id.to_owned());
                notes.push(Note::Session(id.to_owned()));
            }
        }
        match event {
            "step_update" => {
                let step = &value["step_update"];
                let index = step.get("step_index").and_then(Value::as_u64).unwrap_or(0);
                if let Some(usage) = step.get("usage") {
                    self.by_step.insert(index, agy_tokens(usage));
                    self.tokens = sum(self.by_step.values().copied());
                    notes.push(Note::Usage);
                }
                if let Some(delta) = step.get("text_delta").and_then(Value::as_str) {
                    self.agy_text.push_str(delta);
                    if step.get("state").and_then(Value::as_str) == Some("DONE") {
                        let text = std::mem::take(&mut self.agy_text);
                        if !text.trim().is_empty() {
                            self.last_text = Some(text.clone());
                            notes.push(Note::Text(text));
                        }
                    }
                }
                if step.get("step_type").and_then(Value::as_str) == Some("tool") {
                    let id = index.to_string();
                    let name = step["tool_name"].as_str().unwrap_or("tool");
                    match step.get("state").and_then(Value::as_str) {
                        Some("ACTIVE") => {
                            notes.extend(self.call(id, name, &step["tool_info"]["parameters"]));
                        }
                        Some(state @ ("DONE" | "ERROR")) => {
                            let info = &step["tool_info"];
                            notes.extend(
                                self.answer(&id, |_| {
                                    view::agy_result(name, info, state == "ERROR")
                                }),
                            );
                        }
                        _ => {}
                    }
                }
            }
            "result" => {
                let result = &value["result"];
                if let Some(usage) = result.get("usage") {
                    self.tokens = agy_tokens(usage);
                    notes.push(Note::Usage);
                }
                self.result = Some(result.clone());
                notes.push(Note::Result);
            }
            _ => {}
        }
        notes
    }

    /// The Work Report, checked, or why there is none. An empty answer is a failure whatever the status says.
    pub fn report(&self) -> Result<Value, String> {
        let result = self
            .result
            .as_ref()
            .ok_or("the stream ended with no result")?;
        let (failed, text) = if self.cli == "agy" {
            let status = result.get("status").and_then(Value::as_str).unwrap_or("");
            (
                (status != "SUCCESS").then(|| format!("agy status {status:?}")),
                result.get("response").and_then(Value::as_str),
            )
        } else {
            let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);
            let subtype = result.get("subtype").and_then(Value::as_str).unwrap_or("");
            (
                (is_error || subtype != "success")
                    .then(|| format!("claude ended with {subtype:?}, is_error {is_error}")),
                result.get("result").and_then(Value::as_str),
            )
        };
        if let Some(why) = failed {
            return Err(why);
        }
        let mut report = match result.get("structured_output") {
            Some(output) if !output.is_null() => output.clone(),
            _ => {
                let text = text.unwrap_or("").trim();
                if text.is_empty() {
                    return Err("an empty answer, though the CLI said it succeeded (P2)".into());
                }
                serde_json::from_str(text).map_err(|_| {
                    "the answer is not a Work Report: no structured output".to_owned()
                })?
            }
        };
        if let Some(object) = report.as_object_mut() {
            for key in AGY_EXTRA_KEYS {
                object.remove(key);
            }
        }
        check_report(&report)?;
        Ok(report)
    }
}

/// agy's typed failure (P5): exit 3 with an `AGY_ERROR: {...}` line on stderr whose status names a quota or
/// resource exhaustion. The exact status string for a spent quota is not yet recorded, so any field that
/// names `RESOURCE_EXHAUSTED` or a quota, or an HTTP 429, reads as one.
pub fn agy_rate_limit(exit: Option<i32>, stderr: &str, pool: &str) -> Option<RateLimit> {
    if exit != Some(3) {
        return None;
    }
    for line in stderr.lines() {
        let Some(json) = line.trim().strip_prefix("AGY_ERROR:") else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(json.trim()) else {
            continue;
        };
        let mut quota = false;
        let mut stack = vec![&value];
        while let Some(item) = stack.pop() {
            match item {
                Value::String(text) => {
                    let upper = text.to_ascii_uppercase();
                    quota |= upper.contains("RESOURCE_EXHAUSTED") || upper.contains("QUOTA");
                }
                Value::Number(n) => quota |= n.as_u64() == Some(429),
                Value::Object(map) => stack.extend(map.values()),
                Value::Array(items) => stack.extend(items.iter()),
                _ => {}
            }
        }
        if quota {
            return Some(RateLimit {
                signal: "agy_exit_3".into(),
                pool: Some(pool.to_owned()),
                window: None,
                resets_at: None,
                detail: format!("AGY_ERROR: {value}"),
            });
        }
    }
    None
}

// ---- liveness: a lock the worker's supervisor holds open ----

fn live_dir(project: &Project, run: &str) -> PathBuf {
    project.run_dir(run).join("live")
}

/// Held open by `seat exec` while it supervises an order. A pid file is not liveness (an owner daemon died with
/// its pid file in place): the OS releases this file when the process ends, however it ends.
pub struct LiveLock {
    _file: std::fs::File,
    path: PathBuf,
}

impl Drop for LiveLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path.with_extension("stop"));
        let _ = std::fs::remove_file(self.path.with_extension(PANE_EXT));
    }
}

/// Plan 31 review S4: beside a live lock, `<order>.pane` names the pane its supervisor runs in. herdr's launch types
/// `seat exec` into a reused pane, and the supervisor holds its lock before its worker's `started` event, so for that
/// stretch the pane's last `started` is the finished worker before it.
const PANE_EXT: &str = "pane";

/// Plan 31 review S2: set only by herdr's own calls of its CLI (`nirvana_accounts::herdr_self`), so its own typing
/// into a pane is never taken for a message to a finished worker. The owner's way past the guard is `--force`.
pub const SELF_SEND_ENV: &str = "HERDR_NIRVANA_SELF_SEND";

/// Plan 31 review S3: the file under the Nirvana home that names the project whose worker last ran in a pane, so the
/// server, which serves every project's panes, finds the journals to read.
fn pane_index(home: &Path, pane: &str) -> PathBuf {
    let safe: String = pane
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    home.join(nirvana_seat::DIR).join("panes").join(safe)
}

/// Records, as the supervisor takes a pane, that it runs `order` there (`PANE_EXT`) and that the pane's journals are
/// `project`'s (`pane_index`). A failed write loses the guard's facts, never the worker.
pub(crate) fn note_worker_pane(project: &Project, run: &str, order: &str, pane: &str) {
    let _ = std::fs::write(
        live_dir(project, run).join(format!("{order}.{PANE_EXT}")),
        pane,
    );
    if let Some(home) = crate::nirvana::home() {
        let path = pane_index(&home, pane);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, project.root.display().to_string());
    }
}

#[cfg(windows)]
fn open_exclusive(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
}

#[cfg(windows)]
fn held(err: &std::io::Error) -> bool {
    // ERROR_SHARING_VIOLATION
    err.raw_os_error() == Some(32)
}

/// Takes the order's live lock, or `None` when another supervisor holds it. Retried briefly, since `live` opens
/// the same file for an instant to look.
pub fn take_live(project: &Project, run: &str, order: &str) -> std::io::Result<Option<LiveLock>> {
    let dir = live_dir(project, run);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{order}.lock"));
    #[cfg(windows)]
    {
        for _ in 0..10 {
            match open_exclusive(&path) {
                Ok(mut file) => {
                    let _ = file.write_all(std::process::id().to_string().as_bytes());
                    return Ok(Some(LiveLock { _file: file, path }));
                }
                Err(err) if held(&err) => std::thread::sleep(Duration::from_millis(100)),
                Err(err) => return Err(err),
            }
        }
        Ok(None)
    }
    #[cfg(not(windows))]
    {
        // Not built or measured off Windows: a pid in the file, live while `/proc/<pid>` exists.
        if live(project, run, order) {
            return Ok(None);
        }
        let file = std::fs::File::create(&path)?;
        std::fs::write(&path, std::process::id().to_string())?;
        Ok(Some(LiveLock { _file: file, path }))
    }
}

/// Plan 31 §2.4: why text must not be typed into `pane`, when the last worker any run of the project started in it
/// has ended; `None` when no worker ran there or the last one has not ended. `runs` are each run's id and events.
///
/// On 2026-10-29 Nirvana Claude answered `dune-slice-v5`'s question, then the owner's go-ahead, with `herdr pane
/// send-text wP:p1D` and `send-keys Enter` (its transcript, lines 4283 and 4312) after the worker had ended. Nobody read
/// them: 4,111 s passed until the owner asked "whats the progresss can we print?". A pane a later worker reuses
/// (`place::Placement::Reuse`) belongs to that worker. Review S4: `live` says whether a supervisor holds the order's
/// lock; a pane whose last order is live is not finished. Pure but for `live`.
pub fn finished_worker_in(
    runs: &[(String, Vec<Event>)],
    pane: &str,
    live: impl Fn(&str, &str) -> bool,
) -> Option<String> {
    let (run, events, started) = runs
        .iter()
        .flat_map(|(run, events)| {
            events.iter().filter_map(move |event| match &event.kind {
                Kind::Started {
                    order,
                    pane: Some(at),
                    ..
                } if at == pane => Some((run, events, event, order)),
                _ => None,
            })
        })
        .max_by_key(|(_, _, event, _)| (event.at, event.seq))
        .map(|(run, events, event, order)| (run, events, (event.seq, order)))?;
    let (seq, order) = started;
    if live(run, order) {
        return None;
    }
    let ended = events.iter().find_map(|event| match &event.kind {
        Kind::Ended {
            order: Some(done),
            outcome,
            ..
        } if done == order && event.seq > seq => Some((event.at, outcome)),
        _ => None,
    })?;
    let (at, outcome) = ended;
    Some(format!(
        "pane {pane} held order {order} of run {run}, whose worker ended {outcome} at {}; nothing was typed, since a \
         finished worker reads nothing. Answer it through the gate (`herdr gate run {order} --run {run}`: a send-back \
         carries the answer to a new worker) or seat a new order. If the pane is yours now, pass `--force`. \
         (`pane run` is not checked: herdr's own launch types into a reused pane with it.)",
        utc(at)
    ))
}

/// `finished_worker_in` for a pane, over the journals of the project `pane_index` names under `home`; `None` when no
/// worker of any project ran there. Called where herdr's server serves `pane send-text`, `pane send-keys` and
/// `agent send-keys` (review S3), so a socket client meets it as the CLI does. Review S4: a pane any live supervisor
/// names (`PANE_EXT`) is never finished.
pub fn finished_worker_at(home: &Path, pane: &str) -> Option<String> {
    let root = std::fs::read_to_string(pane_index(home, pane)).ok()?;
    let project = Project::at(root.trim());
    if !project.dir().join("runs").is_dir() {
        return None;
    }
    let runs: Vec<(String, Vec<Event>)> = project
        .runs()
        .into_iter()
        .map(|run| {
            let events = project.journal(&run).read().0;
            (run, events)
        })
        .collect();
    let held = runs.iter().any(|(run, _)| {
        std::fs::read_dir(live_dir(&project, run))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == PANE_EXT))
            .any(|entry| {
                let path = entry.path();
                let order = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("");
                std::fs::read_to_string(&path).is_ok_and(|named| named.trim() == pane)
                    && live(&project, run, order)
            })
    });
    if held {
        return None;
    }
    finished_worker_in(&runs, pane, |run, order| live(&project, run, order))
}

/// Whether a supervisor runs for the order now.
pub fn live(project: &Project, run: &str, order: &str) -> bool {
    let path = live_dir(project, run).join(format!("{order}.lock"));
    if !path.exists() {
        return false;
    }
    #[cfg(windows)]
    {
        match open_exclusive(&path) {
            Ok(_) => false,
            Err(err) => held(&err),
        }
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
            .is_some_and(|pid| Path::new(&format!("/proc/{pid}")).exists())
    }
}

fn stop_file(project: &Project, run: &str, order: &str) -> PathBuf {
    live_dir(project, run).join(format!("{order}.stop"))
}

// ---- the job object: a worker's whole process tree ends with it (§4, Argon's review F5) ----

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    /// A kill-on-close job: closing it, or `kill`, ends every process the worker started.
    pub struct Job(HANDLE);

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }

        /// Puts a child started suspended in the job, then lets it run, so nothing it starts escapes.
        pub fn adopt(&self, child: &std::process::Child) -> std::io::Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle().cast()) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            resume(child.id())
        }

        pub fn kill(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn resume(pid: u32) -> std::io::Result<()> {
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error());
        }
        let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<THREADENTRY32>() as u32;
        let mut resumed = false;
        let mut ok = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while ok {
            if entry.th32OwnerProcessID == pid {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if !thread.is_null() {
                    unsafe {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                    resumed = true;
                }
            }
            ok = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        if resumed {
            Ok(())
        } else {
            Err(std::io::Error::other(
                "the worker's thread could not be resumed",
            ))
        }
    }
}

/// Off Windows the child is killed alone; not built or measured there.
#[cfg(not(windows))]
mod job {
    pub struct Job;
    impl Job {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }
        pub fn adopt(&self, _child: &std::process::Child) -> std::io::Result<()> {
            Ok(())
        }
        pub fn kill(&self) {}
    }
}

// ---- running one attempt ----

enum Line {
    Out(String),
    Done,
}

#[derive(Debug)]
pub enum AttemptEnd {
    Exited { code: Option<i32>, stderr: String },
    RateLimited(RateLimit),
    Stalled { silent_s: u64 },
    Stopped,
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for the worker's program: `run_attempt` starts it in place of the argv it journals, so a test
    /// drives `exec` without a real CLI.
    pub(crate) static TEST_COMMAND: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Starts the worker in its job and reads its stream until it ends, stops, stalls or runs out.
fn run_attempt(
    journal: &Journal,
    stream_path: &Path,
    seated: &Seated,
    attempt: u32,
    argv: &[String],
    stop: &Path,
    pane: Option<&str>,
    out: &mut dyn Write,
) -> std::io::Result<(AttemptEnd, Stream)> {
    #[cfg(test)]
    let stand_in = TEST_COMMAND.with_borrow(Clone::clone);
    #[cfg(not(test))]
    let stand_in: Option<Vec<String>> = None;
    let spawned = stand_in.as_deref().unwrap_or(argv);
    let mut command = std::process::Command::new(&spawned[0]);
    command
        .args(&spawned[1..])
        .current_dir(&seated.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for key in ENV_REMOVED {
        command.env_remove(key);
    }
    if seated.cli == "claude" {
        for (key, value) in CLAUDE_ENV {
            command.env(key, value);
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED};
        // Suspended until it is in the job, so nothing it starts escapes the job.
        command.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    }
    let job = job::Job::new()?;
    let mut child = command.spawn()?;
    if let Err(err) = job.adopt(&child) {
        let _ = child.kill();
        return Err(err);
    }
    journal.append(
        now(),
        Kind::Started {
            order: seated.order.clone(),
            attempt,
            pid: Some(child.id()),
            pane: pane.map(str::to_string),
            tab: std::env::var("HERDR_TAB_ID").ok(),
            resume: argv
                .iter()
                .position(|arg| arg == "--resume" || arg == "--conversation")
                .and_then(|at| argv.get(at + 1).cloned()),
            argv: argv.to_vec(),
        },
    )?;
    let (send, lines) = mpsc::channel::<Line>();
    let stdout = child.stdout.take();
    let reader = std::thread::spawn(move || {
        if let Some(stdout) = stdout {
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if send.send(Line::Out(line)).is_err() {
                    break;
                }
            }
        }
        let _ = send.send(Line::Done);
    });
    let stderr = child.stderr.take();
    let stderr_text = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let stderr_sink = stderr_text.clone();
    let stderr_reader = std::thread::spawn(move || {
        if let Some(stderr) = stderr {
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                let mut text = stderr_sink.lock().unwrap_or_else(|e| e.into_inner());
                if text.len() < 256 * 1024 {
                    text.push_str(&line);
                    text.push('\n');
                }
            }
        }
    });
    let mut tee = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(stream_path)?;
    let mut stream = Stream::new(&seated.cli).in_dir(&seated.cwd);
    let mut last_line = Instant::now();
    let mut usage_at: Option<Instant> = None;
    let mut usage_dirty = false;
    let write_usage = |stream: &Stream| {
        journal.append(
            now(),
            Kind::Usage {
                order: Some(seated.order.clone()),
                attempt,
                session: stream.session.clone(),
                tokens: stream.tokens,
            },
        )
    };
    let end = 'read: loop {
        match lines.recv_timeout(Duration::from_millis(500)) {
            Ok(Line::Out(line)) => {
                last_line = Instant::now();
                writeln!(tee, "{line}")?;
                for note in stream.feed(&line) {
                    match note {
                        Note::Usage | Note::Session(_) => usage_dirty = true,
                        Note::Text(_) | Note::Tool(_) | Note::ToolOut { .. } => {
                            // Fitted to the pane as it is now: a row that wraps on its own starts at the left edge.
                            for shown in view::fit(&view::note_lines(&note, true), view::width()) {
                                let _ = writeln!(out, "{shown}");
                            }
                        }
                        Note::Result => {}
                        Note::RateLimited(limit) => {
                            // The signal arrives between turns: a rejected request has no turn in flight, so
                            // this is the turn boundary §5.6 step 1 stops at.
                            job.kill();
                            let _ = child.wait();
                            break 'read AttemptEnd::RateLimited(limit);
                        }
                    }
                }
                // At most one usage event in 10 s, and none before the first token: the journal stays small.
                if usage_dirty
                    && stream.tokens != Tokens::default()
                    && usage_at.is_none_or(|at| at.elapsed() >= Duration::from_secs(10))
                {
                    write_usage(&stream)?;
                    usage_at = Some(Instant::now());
                    usage_dirty = false;
                }
            }
            Ok(Line::Done) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let status = child.wait()?;
                let _ = stderr_reader.join();
                let stderr = stderr_text
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if let Some(limit) = agy_rate_limit(status.code(), &stderr, &seated.pool)
                    .filter(|_| seated.cli == "agy")
                {
                    break AttemptEnd::RateLimited(limit);
                }
                break AttemptEnd::Exited {
                    code: status.code(),
                    stderr,
                };
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.exists() {
                    job.kill();
                    let _ = child.wait();
                    break AttemptEnd::Stopped;
                }
                // `stalled` takes two clock readings; the silence is measured on a monotonic clock here.
                let silent = last_line.elapsed().as_secs();
                if nirvana_run::stalled(0, silent) {
                    job.kill();
                    let _ = child.wait();
                    break AttemptEnd::Stalled { silent_s: silent };
                }
            }
        }
    };
    drop(job);
    let _ = reader.join();
    if usage_dirty || stream.lines > 0 {
        write_usage(&stream)?;
    }
    Ok((end, stream))
}

/// The worktree's state at a stop: `git status`, then `git diff HEAD`, in a file beside the journal.
pub fn write_diff(
    run_dir: &Path,
    order: &str,
    tag: &str,
    cwd: &Path,
) -> (Option<String>, Option<u64>) {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let (Some(status), Some(diff)) = (
        git(&["status", "--porcelain=v1"]),
        git(&["diff", "HEAD", "--no-color"]),
    ) else {
        return (None, None);
    };
    let relative = format!("checkpoints/{order}-{tag}.diff");
    let path = run_dir.join(&relative);
    let text = format!("# git status --porcelain=v1\n{status}\n# git diff HEAD\n{diff}");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match std::fs::write(&path, &text) {
        Ok(()) => (Some(relative), Some(text.len() as u64)),
        Err(_) => (None, None),
    }
}

fn release_lease(order: &str) {
    let dir = nirvana_seat::dir();
    if let Ok(_lock) = nirvana_seat::Lock::take(&dir) {
        let path = dir.join(nirvana_seat::LEASES_FILE);
        // A leases.json that does not parse is left as it is (O2 fix round); the lease then expires.
        if let Ok(mut leases) = nirvana_seat::load_leases(&path) {
            if leases.release(order, now()) {
                let _ = nirvana_seat::write_leases(&path, &leases);
            }
        }
    }
}

fn last_turn(stream: &Stream) -> Option<String> {
    stream
        .last_text
        .as_ref()
        .map(|text| text.chars().take(4000).collect())
}

fn print_timeout_s(seated: &Seated) -> u64 {
    nirvana_seat::table()
        .map(|table| table.size(seated.size).lease_minutes * 60)
        .unwrap_or(0)
}

/// `herdr seat exec <order> --run <run> [--pane <pane>]`: the worker's supervisor, in its pane.
pub fn exec(
    project: &Project,
    run: &str,
    order: &str,
    pane: Option<&str>,
    out: &mut dyn Write,
) -> std::io::Result<i32> {
    let journal = project.journal(run);
    let Some(_live) = take_live(project, run, order)? else {
        writeln!(out, "{order} already has a live worker")?;
        return Ok(0);
    };
    if let Some(pane) = pane {
        note_worker_pane(project, run, order, pane);
    }
    let run_dir = project.run_dir(run);
    let streams = run_dir.join("streams");
    std::fs::create_dir_all(&streams)?;
    let stream_path = streams.join(format!("{order}.jsonl"));
    let stop = stop_file(project, run, order);
    let _ = std::fs::remove_file(&stop);
    loop {
        let state = replay(&journal.read().0);
        let Some(known) = state.order(order).cloned() else {
            writeln!(out, "no order {order} in run {run}")?;
            return Ok(2);
        };
        if !known.phase.wants_worker() {
            writeln!(out, "{order} is {}; nothing to run", known.phase.word())?;
            return Ok(0);
        }
        let seated = known.seated.clone();
        // A deferral is waited out here, in the worker's own pane; `herdr run tick` starts it if this pane died.
        if let Some(until) = known.deferred_until.filter(|until| *until > now()) {
            writeln!(
                out,
                "{order} waits for {} ({})",
                utc(until),
                known.why.as_deref().unwrap_or("deferred")
            )?;
            while now() < until {
                if stop.exists() {
                    return stopped(project, &journal, run, &seated, None, out);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        let attempt = known.attempts + 1;
        let resume = (attempt > 1).then(|| known.session.clone()).flatten();
        let prompt = match (&resume, known.phase) {
            (Some(_), Phase::Deferred) => continue_line(&seated.order_file, "its window ran out"),
            (Some(_), _) => continue_line(&seated.order_file, "it stalled"),
            (None, _) => first_line(&seated.order_file),
        };
        let spec = LaunchSpec {
            cli: seated.cli.clone(),
            model: seated.model.clone(),
            effort: seated.effort,
            prompt,
            resume,
            print_timeout_s: print_timeout_s(&seated),
        };
        // Plan 29 review F3: an order that cannot get a server it was seated for fails here and never starts without.
        let argv = match worker_argv(
            &nirvana_seat::dir(),
            &spec,
            Path::new(&seated.cwd),
            &project.root,
            crate::nirvana_superpowers::claude_dir().as_deref(),
            out,
        )
        .and_then(|argv| with_order_servers(argv, project, run, &seated, out))
        .and_then(|argv| with_profile(argv, &run_dir, &seated))
        {
            Ok(argv) => argv,
            Err(why) => {
                end_failed(project, &journal, run, &seated, &why, None)?;
                writeln!(out, "{why}")?;
                return Ok(1);
            }
        };
        writeln!(
            out,
            "{}",
            view::header(
                &seated.order,
                &seated.cli,
                &seated.model,
                seated.effort.word(),
                &seated.class,
                attempt,
                true
            )
        )?;
        let attempt_started = Instant::now();
        let attempt_end = run_attempt(
            &journal,
            &stream_path,
            &seated,
            attempt,
            &argv,
            &stop,
            pane,
            out,
        );
        // Plan 29 review F2: the order's server definitions, tokens and all, go when its worker process ends.
        crate::nirvana_gui_tools::remove_order_config(&run_dir, order, out);
        let (end, stream) = match attempt_end {
            Ok(done) => done,
            Err(err) => {
                let why = format!("the worker could not start: {err}");
                end_failed(project, &journal, run, &seated, &why, None)?;
                writeln!(out, "{why}")?;
                return Ok(1);
            }
        };
        match end {
            AttemptEnd::Exited { code, stderr } => {
                return match stream.report() {
                    Ok(report) => {
                        let status = report["status"].as_str().unwrap_or("done").to_owned();
                        let mut kinds = vec![Kind::Delivered {
                            order: order.to_owned(),
                            report: report.clone(),
                        }];
                        if status == "needs_answer" {
                            kinds.push(Kind::Asked {
                                order: Some(order.to_owned()),
                                question: report
                                    .get("question")
                                    .or_else(|| report.get("summary"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_owned(),
                            });
                        }
                        kinds.push(Kind::Ended {
                            order: Some(order.to_owned()),
                            outcome: "delivered".into(),
                            why: None,
                            exit: code,
                            pools: Vec::new(),
                            pools_basis: None,
                        });
                        journal.transact(now(), |_| Ok((kinds, ())))?;
                        release_lease(order);
                        writeln!(
                            out,
                            "{}",
                            view::footer(
                                &format!("delivered: {status}"),
                                true,
                                attempt_started.elapsed().as_secs(),
                                stream.tokens,
                                true
                            )
                        )?;
                        let _ = nirvana_run::wake(project, run, false);
                        // Plan 26 §2: A worker that delivers done closes its own pane through herdr
                        // as the last thing it does. The tab's last pane stays while the run is open.
                        if in_herdr() {
                            // S3: the count and the close happen under the run's lock; without it, close nothing.
                            let lock = crate::nirvana_seat::Lock::take(&project.run_dir(run));
                            if let Err(why) = &lock {
                                eprintln!("pane kept: the run's lock was not taken: {why}");
                            }
                            let state = replay(&journal.read().0);
                            let my_pane = pane
                                .or_else(|| state.order(order).and_then(|o| o.pane.as_deref()))
                                .filter(|_| lock.is_ok());
                            if let Some(pane_id) = my_pane {
                                let expected_title = expected_pane_title(&seated.seat, order);
                                let actual_title = pane_title(pane_id);
                                let run_panes: Vec<String> =
                                    state.orders.iter().filter_map(|o| o.pane.clone()).collect();
                                let tab_panes_count =
                                    herdr_json(&["pane", "layout", "--pane", pane_id])
                                        .ok()
                                        .and_then(|layout| {
                                            ["/result/layout/panes", "/layout/panes"]
                                                .iter()
                                                .find_map(|p| layout.pointer(p))
                                                .and_then(Value::as_array)
                                                .map(|arr| {
                                                    let ids: Vec<&str> = arr
                                                        .iter()
                                                        .filter_map(|p| {
                                                            p.get("pane_id").and_then(Value::as_str)
                                                        })
                                                        .collect();
                                                    count_run_tab_panes(&ids, &run_panes)
                                                })
                                        })
                                        .unwrap_or(0);
                                if exec_should_close_pane_after_delivered(
                                    &state,
                                    order,
                                    &status,
                                    Some(pane_id),
                                    actual_title.as_deref(),
                                    &expected_title,
                                    tab_panes_count,
                                ) {
                                    if let Err(why) = herdr_json(&["pane", "close", pane_id]) {
                                        eprintln!("failed to close pane {pane_id}: {why}");
                                    }
                                } else if actual_title.as_deref() != Some(&expected_title) {
                                    eprintln!("pane {pane_id} title is not {expected_title}; keeping pane");
                                }
                            }
                        }
                        Ok(0)
                    }
                    Err(why) => {
                        let tail: String =
                            stderr.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
                        let why = if tail.is_empty() {
                            why
                        } else {
                            format!(
                                "{why}; stderr: {}",
                                tail.chars().take(300).collect::<String>()
                            )
                        };
                        end_failed(project, &journal, run, &seated, &why, code)?;
                        writeln!(
                            out,
                            "{}",
                            view::footer(
                                &format!("failed: {why}"),
                                false,
                                attempt_started.elapsed().as_secs(),
                                stream.tokens,
                                true
                            )
                        )?;
                        Ok(1)
                    }
                };
            }
            AttemptEnd::Stopped => {
                return stopped(project, &journal, run, &seated, Some(&stream), out);
            }
            AttemptEnd::Stalled { silent_s } => {
                let stalls = known.stalls + 1;
                let delay = nirvana_run::restart_delay(known.stalls);
                let next = match delay {
                    Some(0) => "restart at once".to_owned(),
                    Some(s) => format!("restart in {s} s"),
                    None => "refused".to_owned(),
                };
                let (diff, bytes) = write_diff(
                    &run_dir,
                    order,
                    &format!("stall{stalls}"),
                    Path::new(&seated.cwd),
                );
                let mut kinds = vec![
                    Kind::Checkpoint {
                        order: order.to_owned(),
                        why: "stalled".into(),
                        report: stream.report().ok(),
                        last_turn: last_turn(&stream),
                        diff,
                        diff_bytes: bytes,
                    },
                    Kind::Stalled {
                        order: order.to_owned(),
                        silent_s,
                        stalls,
                        next: next.clone(),
                    },
                ];
                if delay.is_none() {
                    kinds.push(Kind::Asked {
                        order: Some(order.to_owned()),
                        question: format!(
                            "{order} stalled {stalls} times: a crash loop; restart it, re-seat it or drop it?"
                        ),
                    });
                }
                journal.transact(now(), |_| Ok((kinds, ())))?;
                let _ = nirvana_run::wake(project, run, false);
                writeln!(out, "stalled after {silent_s} s silent: {next}")?;
                match delay {
                    Some(s) => std::thread::sleep(Duration::from_secs(s)),
                    None => {
                        release_lease(order);
                        let state = replay(&journal.read().0);
                        if let Some(known) = state.order(order) {
                            let _ = nirvana_run::write_record(
                                run,
                                known,
                                nirvana_run::journal_ref(&journal, &state),
                            );
                        }
                        return Ok(1);
                    }
                }
            }
            AttemptEnd::RateLimited(limit) => {
                if !rate_limited(project, &journal, run, &seated, &limit, &stream, out)? {
                    return Ok(0);
                }
            }
        }
    }
}

fn end_failed(
    project: &Project,
    journal: &Journal,
    run: &str,
    seated: &Seated,
    why: &str,
    exit: Option<i32>,
) -> std::io::Result<()> {
    journal.append(
        now(),
        Kind::Ended {
            order: Some(seated.order.clone()),
            outcome: "failed".into(),
            why: Some(why.to_owned()),
            exit,
            pools: Vec::new(),
            pools_basis: None,
        },
    )?;
    release_lease(&seated.order);
    let state = replay(&journal.read().0);
    if let Some(known) = state.order(&seated.order) {
        let _ = nirvana_run::write_record(run, known, nirvana_run::journal_ref(journal, &state));
    }
    let _ = nirvana_run::wake(project, run, false);
    Ok(())
}

fn stopped(
    project: &Project,
    journal: &Journal,
    run: &str,
    seated: &Seated,
    stream: Option<&Stream>,
    out: &mut dyn Write,
) -> std::io::Result<i32> {
    let (diff, bytes) = write_diff(
        &project.run_dir(run),
        &seated.order,
        "stop",
        Path::new(&seated.cwd),
    );
    journal.transact(now(), |_| {
        Ok((
            vec![
                Kind::Checkpoint {
                    order: seated.order.clone(),
                    why: "interrupted".into(),
                    report: stream.and_then(|stream| stream.report().ok()),
                    last_turn: stream.and_then(last_turn),
                    diff,
                    diff_bytes: bytes,
                },
                Kind::Interrupted {
                    order: seated.order.clone(),
                    why: "stopped by `herdr seat stop`".into(),
                },
                Kind::Ended {
                    order: Some(seated.order.clone()),
                    outcome: "stopped".into(),
                    why: None,
                    exit: None,
                    pools: Vec::new(),
                    pools_basis: None,
                },
            ],
            (),
        ))
    })?;
    release_lease(&seated.order);
    writeln!(out, "stopped; the worktree's diff is in the checkpoint")?;
    Ok(0)
}

/// The world as the rule sees it after a rate limit: the spent pool's window (or every window, when the signal
/// names none) read as used up now, so O2's floor skips every seat on that pool (§5.6 step 3: "the spent seat
/// excluded"; its pool is spent for every seat on it).
pub fn spent_world(
    table: &nirvana_seat::Table,
    mut world: nirvana_seat::World,
    limit: &RateLimit,
    pool: &str,
) -> nirvana_seat::World {
    let windows: Vec<String> = match &limit.window {
        Some(window) => vec![window.clone()],
        None => table
            .pool(pool)
            .map(|pool| pool.windows.clone())
            .unwrap_or_default(),
    };
    for window in windows.into_iter().rev() {
        world.readings.insert(
            0,
            Reading {
                pool: pool.to_owned(),
                window,
                used: Some(1.0),
                resets_at: limit.resets_at,
                resets_text: None,
                read_at: world.now,
                source: format!("{} signal", limit.signal),
            },
        );
    }
    world
}

pub fn rate_limited_override(seated: &Seated, limit_pool: &str) -> (bool, bool, Option<String>) {
    match &seated.override_reason {
        Some(_) => {
            if limit_pool == seated.pool.as_str() {
                (true, false, None)
            } else {
                (
                    false,
                    true,
                    Some(format!("{}:{}", seated.seat, seated.effort.word())),
                )
            }
        }
        None => (false, false, None),
    }
}

/// §5.6 for one order. Returns whether the supervisor goes on (a deferral it waits out itself).
pub(crate) fn rate_limited(
    project: &Project,
    journal: &Journal,
    run: &str,
    seated: &Seated,
    limit: &RateLimit,
    stream: &Stream,
    out: &mut dyn Write,
) -> std::io::Result<bool> {
    let order = &seated.order;
    let pool = limit.pool.clone().unwrap_or_else(|| seated.pool.clone());
    let at = now();
    let state = replay(&journal.read().0);
    let attempt = state.order(order).map_or(1, |known| known.attempts);
    let (diff, bytes) = write_diff(
        &project.run_dir(run),
        order,
        &format!("limit{attempt}"),
        Path::new(&seated.cwd),
    );
    journal.transact(at, |_| {
        Ok((
            vec![
                Kind::RateLimited {
                    order: order.clone(),
                    signal: limit.signal.clone(),
                    pool: Some(pool.clone()),
                    window: limit.window.clone(),
                    resets_at: limit.resets_at,
                    detail: limit.detail.chars().take(600).collect(),
                },
                Kind::Checkpoint {
                    order: order.clone(),
                    why: "rate_limited".into(),
                    report: stream.report().ok(),
                    last_turn: last_turn(stream),
                    diff: diff.clone(),
                    diff_bytes: bytes,
                },
            ],
            (),
        ))
    })?;
    let table = nirvana_seat::table_for(&project.root).map_err(std::io::Error::other)?;
    let child = child_id(&state, order);
    let mut request = nirvana_seat::Request::new(&seated.class);
    request.size = seated.size;
    // Plan 29 §5.2: an order seated for a project MCP server is re-seated on a claude seat or not at all.
    let needs = crate::nirvana_gui_tools::seat_needs(&table, &project.root, &seated.needs);
    if let Ok(needs) = &needs {
        (request.needs, request.claude_only) = (needs.rest.clone(), needs.claude_only.clone());
    }
    request.not_family = seated.not_family.clone();
    request.mode = seated.mode;
    request.order = Some(child.clone());
    // Plan 33 §2.2: "A send-back child and a reseat keep the parent's profile copy", and so its narrowing.
    let narrowed = crate::nirvana_profiles::narrow_for_order(
        &mut request,
        &project.run_dir(run),
        &std::fs::read_to_string(&seated.order_file).unwrap_or_default(),
    );
    let limit_pool = limit.pool.as_deref().unwrap_or(&pool);
    let (override_dropped, inherited_override, target) = rate_limited_override(seated, limit_pool);
    if let (Some(target), Some(reason)) = (target, &seated.override_reason) {
        request.override_seat = Some((target, reason.clone()));
        request.inherited_override = true;
    }
    let world = spent_world(&table, nirvana_seat::live_world(&table), limit, &pool);
    let picked = match needs.and(narrowed) {
        Ok(_) => nirvana_seat::pick(&table, &world, &request),
        Err(why) => Err(nirvana_seat::NoSeat {
            class: seated.class.clone(),
            reason: why,
            earliest_reset: None,
            defer: false,
            skipped: Vec::new(),
        }),
    };
    let reseat = match &picked {
        Ok(pick) => Ok(pick.seat.clone()),
        Err(no_seat) => Err(no_seat.earliest_reset),
    };
    let decision = rate_decision(
        at,
        limit.resets_at,
        table.constants.defer_min * 60,
        &reseat,
        state.auto(),
    );
    match decision {
        RateDecision::Defer { until, why } => {
            journal.append(
                now(),
                Kind::Deferred {
                    order: order.clone(),
                    until,
                    why: why.clone(),
                },
            )?;
            writeln!(
                out,
                "rate limited ({}); deferred to {}: {why}",
                limit.signal,
                utc(until)
            )?;
            let _ = nirvana_run::wake(project, run, false);
            Ok(true)
        }
        RateDecision::Reseat { seat } => {
            let Ok(pick) = picked else { return Ok(false) };
            let brief = continuation_brief(project, run, seated, &state, diff.as_deref(), stream)?;
            let booked = book_child(&table, &pick, &child, run)?;
            let child_seated = Seated {
                order: child.clone(),
                seat: booked.seat.clone(),
                cli: booked.cli.clone(),
                model: booked.model.clone(),
                effort: booked.effort,
                pool: booked.pool.clone(),
                class: seated.class.clone(),
                chain: booked.chain.clone(),
                size: seated.size,
                mode: seated.mode,
                needs: seated.needs.clone(),
                not_family: seated.not_family.clone(),
                reason: booked.reason.clone(),
                override_reason: if override_dropped {
                    None
                } else if inherited_override {
                    seated.override_reason.clone()
                } else {
                    None
                },
                order_file: brief,
                cwd: seated.cwd.clone(),
                worktree: seated.worktree.clone(),
                branch: seated.branch.clone(),
                parent: Some(order.clone()),
                why: Some("reseat".into()),
                rule_pick: booked.rule_pick.clone(),
                bar_dropped: booked.bar_dropped,
                step_down: booked.step_down,
                use_basis: booked.use_basis.clone(),
                expected: booked.expected(),
                jev_pick: None,
                // Plan 31 review S1: a new Send needs a new `--send-ok` with a new quote.
                send_ok: None,
            };
            let why = if override_dropped {
                format!(
                    "{} {} ran out ({}); dropped override on {}; a new session with a continuation brief",
                    seated.seat,
                    seated.effort.word(),
                    limit.signal,
                    seated.pool
                )
            } else if inherited_override {
                format!(
                    "{} {} ran out ({}); kept override {}:{}; a new session with a continuation brief",
                    seated.seat,
                    seated.effort.word(),
                    limit.signal,
                    seated.seat,
                    seated.effort.word()
                )
            } else {
                format!(
                    "{} {} ran out ({}); a new session with a continuation brief",
                    seated.seat,
                    seated.effort.word(),
                    limit.signal
                )
            };
            journal.transact(now(), |_| {
                Ok((
                    vec![
                        Kind::Seated(Box::new(child_seated.clone())),
                        Kind::Reseated {
                            order: order.clone(),
                            child: child.clone(),
                            seat: seat.clone(),
                            why: why.clone(),
                        },
                    ],
                    (),
                ))
            })?;
            release_lease(order);
            let state = replay(&journal.read().0);
            if let Some(known) = state.order(order) {
                let _ = nirvana_run::write_record(
                    run,
                    known,
                    nirvana_run::journal_ref(journal, &state),
                );
            }
            let how = launch_worker(project, run, &child_seated).unwrap_or_else(|err| err);
            writeln!(out, "rate limited; re-seated as {child} on {seat} ({how})")?;
            let _ = nirvana_run::wake(project, run, false);
            Ok(false)
        }
        RateDecision::Ask { why } => {
            journal.transact(now(), |_| {
                Ok((
                    vec![
                        Kind::Asked {
                            order: Some(order.clone()),
                            question: format!(
                                "{order} ran out on {} ({}): {why}. Wait, re-seat by hand, or drop it?",
                                seated.seat, limit.signal
                            ),
                        },
                        Kind::Ended {
                            order: Some(order.clone()),
                            outcome: "asked".into(),
                            why: Some(why.clone()),
                            exit: None,
                            pools: Vec::new(),
                            pools_basis: None,
                        },
                    ],
                    (),
                ))
            })?;
            release_lease(order);
            writeln!(out, "rate limited; asked: {why}")?;
            let _ = nirvana_run::wake(project, run, false);
            Ok(false)
        }
    }
}

/// `<order>-r<n>`, the first free number in the run.
pub(crate) fn child_id(state: &nirvana_run::RunState, order: &str) -> String {
    let base: String = order.chars().take(58).collect();
    (1..)
        .map(|n| format!("{base}-r{n}"))
        .find(|id| state.order(id).is_none())
        .unwrap_or_else(|| format!("{base}-r"))
}

/// The continuation brief of a re-seat: the original order, where it stopped, its diff and its last turn.
fn continuation_brief(
    project: &Project,
    run: &str,
    seated: &Seated,
    state: &nirvana_run::RunState,
    diff: Option<&str>,
    stream: &Stream,
) -> std::io::Result<String> {
    let child = child_id(state, &seated.order);
    let original = std::fs::read_to_string(&seated.order_file).unwrap_or_default();
    let original = original
        .split("\n\n---\nFrom herdr")
        .next()
        .unwrap_or("")
        .to_owned();
    let mut brief = format!(
        "# Continuation of {}\n\nThe worker on {} {} stopped when its pool ran out. You are a new session on another \
seat: continue its work, do not start over. Its changes are already in this folder (`git diff HEAD`).\n\n",
        seated.order,
        seated.seat,
        seated.effort.word()
    );
    if let Some(diff) = diff {
        brief.push_str(&format!(
            "- Its diff at the stop: {}\n",
            project.run_dir(run).join(diff).display()
        ));
    }
    if let Some(turn) = last_turn(stream) {
        brief.push_str("- Its last turn:\n\n");
        for line in turn.lines() {
            brief.push_str(&format!("> {line}\n"));
        }
    }
    brief.push_str("\n## The original order\n\n");
    brief.push_str(&original);
    // Plan 31 review S1: the reseat never inherits the approval; it keeps the device line, and hears it may have sent.
    let brief = crate::nirvana_gui_tools::child_brief_lines(&brief, seated.send_ok.is_some());
    write_order(project, Path::new(&seated.cwd), &child, &brief)
}

/// What `seat run` books: the pick's fields the journal keeps.
#[derive(Debug, Clone, Deserialize)]
pub struct Booked {
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
    pub reason: String,
    #[serde(default, rename = "override")]
    pub override_reason: Option<String>,
    #[serde(default)]
    pub rule_pick: Option<String>,
    #[serde(default)]
    pub bar_dropped: Option<bool>,
    #[serde(default)]
    pub step_down: Option<bool>,
    #[serde(default)]
    pub use_basis: Option<String>,
    #[serde(default)]
    pub windows: Vec<BookedWindow>,
    /// Plan 12 O8b: Jev's advice stored with the pick.
    #[serde(default)]
    pub jev_pick: Option<crate::nirvana_jev_roles::JevPick>,
    /// Plan 29: the needs `book` granted as the project's MCP servers; never in the lease.
    #[serde(skip)]
    pub servers: Vec<String>,
    /// Plan 33 §2.2: the profile the pick was narrowed by, from the lease.
    #[serde(default)]
    pub profile: Option<String>,
}

/// One window of the pick: the order's expected share of it, for the record (expected against real use).
#[derive(Debug, Clone, Deserialize)]
pub struct BookedWindow {
    pub window: String,
    #[serde(default)]
    pub expected: f32,
}

impl Booked {
    pub fn expected(&self) -> BTreeMap<String, f32> {
        self.windows
            .iter()
            .map(|window| (window.window.clone(), window.expected))
            .collect()
    }
}

fn book_child(
    table: &nirvana_seat::Table,
    pick: &nirvana_seat::Pick,
    order: &str,
    run: &str,
) -> std::io::Result<Booked> {
    let dir = nirvana_seat::dir();
    let _lock = nirvana_seat::Lock::take(&dir)?;
    let path = dir.join(nirvana_seat::LEASES_FILE);
    let mut leases = nirvana_seat::load_leases(&path).map_err(std::io::Error::other)?;
    let lease = nirvana_seat::lease_for(table, pick, order, Some(run), now());
    let booked = leases.book(lease, now());
    nirvana_seat::write_leases(&path, &leases)?;
    serde_json::from_value(booked.pick).map_err(std::io::Error::other)
}

/// Writes an order's file where its worker reads it (the worker's folder may be a worktree: claude's file tools
/// stay inside the working folders in print mode), and a copy in the project's own `.nirvana/orders/`. Public for O5's
/// send-backs, which write a child's order into its parent's worktree.
pub fn write_order(
    project: &Project,
    cwd: &Path,
    order: &str,
    text: &str,
) -> std::io::Result<String> {
    let mut text = text.to_owned();
    if !text.contains(FOOTER_MARK) {
        text.push_str(ORDER_FOOTER);
    }
    project.ensure()?;
    std::fs::create_dir_all(project.orders_dir())?;
    let canonical = project.orders_dir().join(format!("{order}.md"));
    std::fs::write(&canonical, &text)?;
    let local = cwd.join(nirvana_run::DIR);
    ensure_dir(&local)?;
    let path = local.join("orders").join(format!("{order}.md"));
    if path != canonical {
        std::fs::create_dir_all(path.parent().unwrap_or(cwd))?;
        std::fs::write(&path, &text)?;
    }
    Ok(path.display().to_string())
}

/// Whether a worker ending its attempt should close its own pane (plan 26 §2, B1, S3).
///
/// Decision on closing pane in exec: only after Delivered has been journaled (plan 26 §2).
pub fn exec_should_close_pane_after_delivered(
    state: &crate::nirvana_run::RunState,
    order: &str,
    status: &str,
    pane: Option<&str>,
    actual_title: Option<&str>,
    expected_title: &str,
    tab_panes_count: usize,
) -> bool {
    // An order the journal does not hold closes nothing (plan 26 review R6).
    let Some(phase) = state.order(order).map(|o| o.phase) else {
        return false;
    };
    worker_should_close_pane(
        status,
        phase,
        pane,
        actual_title,
        expected_title,
        tab_panes_count,
    )
}

/// A worker closes its pane as soon as it delivers done and its order is Delivered.
/// Kept panes:
/// - A worker delivering blocked, failed or anything other than done keeps its pane.
/// - So does a stopped or interrupted worker.
/// - The tab's last pane stays while the run is open, so the tab stays open and the
///   next worker can reuse that pane (tab_panes_count from this run only).
/// - The pane's title must equal the title `open_pane` gave it (<seat>·<order>).
pub fn worker_should_close_pane(
    status: &str,
    phase: crate::nirvana_run::Phase,
    pane_id: Option<&str>,
    actual_title: Option<&str>,
    expected_title: &str,
    tab_panes_count: usize,
) -> bool {
    status == "done"
        && phase == crate::nirvana_run::Phase::Delivered
        && pane_id.is_some()
        && actual_title == Some(expected_title)
        && tab_panes_count > 1
}

/// Counts how many panes in `tab_panes` belong to the current run (`run_panes`) (plan 26 S3).
pub fn count_run_tab_panes(tab_panes: &[&str], run_panes: &[String]) -> usize {
    tab_panes
        .iter()
        .filter(|id| run_panes.iter().any(|rp| rp == **id))
        .count()
}

/// The title `open_pane` assigns to a worker's pane: `<seat>·<order>` (plan 26 B1).
pub fn expected_pane_title(seat: &str, order: &str) -> String {
    format!("{}\u{b7}{}", seat, order)
}

/// Reads the title or manual label of a pane via `herdr pane get` (plan 26 B1).
pub fn pane_title(pane: &str) -> Option<String> {
    let info = herdr_json(&["pane", "get", pane]).ok()?;
    find_str(
        &info,
        &[
            "/result/pane/label",
            "/pane/label",
            "/result/pane/title",
            "/pane/title",
        ],
    )
    .map(str::to_owned)
}

// ---- herdr's own API, for panes and the orchestrator's state ----

fn herdr_command(args: &[&str]) -> std::process::Command {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("herdr"));
    let mut command = std::process::Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    crate::platform::configure_background_command(&mut command);
    command
}

/// Runs this herdr with `args` against the server the caller's pane belongs to; the answer's JSON.
pub(crate) fn herdr_json(args: &[&str]) -> Result<Value, String> {
    let out = herdr_command(args)
        .output()
        .map_err(|err| format!("herdr {}: {err}", args.join(" ")))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let value: Option<Value> = serde_json::from_str(text.trim()).ok();
    if !out.status.success() {
        // herdr prints an error response on stderr (`pane close` of a gone pane, 2026-10-18).
        let errors: Option<Value> =
            serde_json::from_str(String::from_utf8_lossy(&out.stderr).trim()).ok();
        let why = value
            .as_ref()
            .or(errors.as_ref())
            .and_then(|value| value.pointer("/error/message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{} {}",
                    text.trim(),
                    String::from_utf8_lossy(&out.stderr).trim()
                )
            });
        return Err(why);
    }
    value.ok_or_else(|| format!("herdr {}: not JSON: {}", args.join(" "), text.trim()))
}

/// For the verbs that answer with nothing on success (`pane run` printed no JSON, measured 2026-10-18).
pub(crate) fn herdr_ok(args: &[&str]) -> Result<(), String> {
    let out = herdr_command(args)
        .output()
        .map_err(|err| format!("herdr {}: {err}", args.join(" ")))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "herdr {}: {} {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

pub(crate) fn herdr_text(args: &[&str]) -> Option<String> {
    let out = herdr_command(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Whether this process runs inside a herdr pane, whose server can open the run's tab. Outside one, `herdr tab
/// create` would reach the owner's default session, so herdr never tries it.
pub fn in_herdr() -> bool {
    std::env::var_os(crate::api::SOCKET_PATH_ENV_VAR).is_some()
        || std::env::var_os("HERDR_PANE_ID").is_some()
}

fn find_str<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

/// The orchestrator's pane as a wake sees it.
pub fn pane_view(pane: &str) -> nirvana_run::PaneView {
    let info = herdr_json(&["pane", "get", pane]).ok();
    let status = info.as_ref().and_then(|value| {
        find_str(value, &["/result/pane/agent_status", "/pane/agent_status"]).map(str::to_owned)
    });
    let program = info
        .as_ref()
        .map_or(nirvana_run::PaneProgram::Unknown, |info| {
            program_of(pane, info)
        });
    let prompt = herdr_text(&[
        "pane", "read", pane, "--source", "visible", "--format", "ansi",
    ])
    .and_then(|screen| nirvana_run::prompt_line_text(&screen));
    nirvana_run::PaneView {
        status,
        prompt,
        last_key_at: nirvana_run::owner_key_at(pane),
        program,
    }
}

/// What runs in the pane (`nirvana_run::pane_program`), from `pane get`'s answer and `pane process-info`.
fn program_of(pane: &str, info: &Value) -> nirvana_run::PaneProgram {
    let Ok(process) = herdr_json(&["pane", "process-info", "--pane", pane]) else {
        return nirvana_run::PaneProgram::Unknown;
    };
    let plugin = crate::nirvana::home().map(|home| crate::nirvana_launch::plugin_dir(&home));
    nirvana_run::pane_program(info, &process, plugin.as_deref())
}

/// `herdr agent prompt`, which refuses a blocked agent before it types anything, here only into a pane that
/// runs Nirvana Claude, read again just before (`nirvana_run::type_if_nirvana`). The runner's only other typing
/// is `open_pane`'s `pane run`, into the shell of the pane that same call created.
pub fn type_prompt(pane: &str, text: &str) -> Result<(), String> {
    let program = herdr_json(&["pane", "get", pane])
        .map_or(nirvana_run::PaneProgram::Unknown, |info| {
            program_of(pane, &info)
        });
    nirvana_run::type_if_nirvana(&program, || {
        herdr_json(&["agent", "prompt", pane, text])
            .map(|_| ())
            .map_err(|why| format!("herdr agent prompt refused: {why}"))
    })
}

fn pane_env(project: &Project) -> Vec<String> {
    let mut env = vec![format!(
        "{}={}",
        nirvana_run::PROJECT_ENV,
        project.root.display()
    )];
    // `herdr` in the pane's shell is this build: the pane's command is then the same in every shell.
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        let path = std::env::var("PATH").unwrap_or_default();
        let sep = if cfg!(windows) { ';' } else { ':' };
        env.push(format!("PATH={}{sep}{path}", dir.display()));
    }
    if let Ok(home) = std::env::var(crate::nirvana::HOME_ENV) {
        env.push(format!("{}={home}", crate::nirvana::HOME_ENV));
    }
    env
}

/// Opens the worker's pane in the run's tab (a new tab for the run's first worker) and starts `seat exec` in it.
fn open_pane(project: &Project, run: &str, seated: &Seated) -> Result<String, String> {
    let state = replay(&project.journal(run).read().0);
    let env = pane_env(project);
    let mut env_args: Vec<&str> = Vec::new();
    for pair in &env {
        env_args.extend(["--env", pair.as_str()]);
    }
    // The run's tab: where its last worker pane still is.
    let known_pane = state
        .orders
        .iter()
        .rev()
        .filter_map(|order| order.pane.clone())
        .find(|pane| herdr_json(&["pane", "get", pane]).is_ok());
    let pane = match known_pane {
        Some(neighbour) => match place::place(&tab_slots(&state, &neighbour)) {
            Some(place::Placement::Reuse(pane)) => pane,
            placement => {
                let (target, direction) = match placement {
                    Some(place::Placement::Split { pane, right }) => {
                        (pane, if right { "right" } else { "down" })
                    }
                    _ => (neighbour, "right"),
                };
                let mut args = vec![
                    "pane",
                    "split",
                    target.as_str(),
                    "--direction",
                    direction,
                    "--cwd",
                    &seated.cwd,
                    "--no-focus",
                ];
                args.extend(env_args.iter().copied());
                let value = herdr_json(&args)?;
                find_str(&value, &["/result/pane/pane_id", "/pane/pane_id"])
                    .ok_or("pane split gave no pane id")?
                    .to_owned()
            }
        },
        None => {
            let label = format!("run {run}");
            let workspace = std::env::var("HERDR_WORKSPACE_ID").ok();
            let mut args = vec![
                "tab",
                "create",
                "--cwd",
                &seated.cwd,
                "--label",
                &label,
                "--no-focus",
            ];
            if let Some(workspace) = &workspace {
                args.extend(["--workspace", workspace.as_str()]);
            }
            args.extend(env_args.iter().copied());
            let value = herdr_json(&args)?;
            find_str(&value, &["/result/root_pane/pane_id", "/root_pane/pane_id"])
                .ok_or("tab create gave no pane id")?
                .to_owned()
        }
    };
    let _ = herdr_ok(&[
        "pane",
        "rename",
        &pane,
        &format!("{}\u{b7}{}", seated.seat, seated.order),
    ]);
    // The shell draws its prompt a moment after the pane opens; a line typed before it can be lost.
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if herdr_text(&["pane", "read", &pane, "--source", "visible"])
            .is_some_and(|screen| !screen.trim().is_empty())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(300));
    herdr_ok(&[
        "pane",
        "run",
        &pane,
        &format!("herdr seat exec {} --run {run} --pane {pane}", seated.order),
    ])?;
    Ok(format!("pane {pane}"))
}

/// The panes of the tab `neighbour` is in, measured, with which of them a finished worker of this run left idle.
fn tab_slots(state: &nirvana_run::RunState, neighbour: &str) -> Vec<place::Slot> {
    let Ok(layout) = herdr_json(&["pane", "layout", "--pane", neighbour]) else {
        return Vec::new();
    };
    let panes = ["/result/layout/panes", "/layout/panes"]
        .iter()
        .find_map(|pointer| layout.pointer(pointer))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    panes
        .iter()
        .filter_map(|pane| {
            let id = pane.get("pane_id")?.as_str()?.to_owned();
            let size = |key: &str| {
                pane.pointer(&format!("/rect/{key}"))
                    .and_then(Value::as_u64)
                    .and_then(|value| u16::try_from(value).ok())
            };
            // The pane's latest worker of this run says whether it is done with it.
            let latest = state
                .orders
                .iter()
                .enumerate()
                .rev()
                .find(|(_, order)| order.pane.as_deref() == Some(id.as_str()));
            let ended = latest.is_some_and(|(_, order)| finished(order.phase));
            Some(place::Slot {
                finished: ended && idle_shell(&id),
                age: latest.map(|(at, _)| at),
                cols: size("width")?,
                rows: size("height")?,
                pane: id,
            })
        })
        .collect()
}

/// A worker's supervisor has exited by these phases; a deferred one starts again in its own pane, an asked one
/// waits on an answer there.
fn finished(phase: nirvana_run::Phase) -> bool {
    use nirvana_run::Phase;
    matches!(
        phase,
        Phase::Delivered
            | Phase::Interrupted
            | Phase::Gated
            | Phase::Merged
            | Phase::Reseated
            | Phase::Failed
    )
}

/// Only a shell runs in the pane, so a typed command reaches nobody's program. Any doubt reads as busy.
fn idle_shell(pane: &str) -> bool {
    let Ok(process) = herdr_json(&["pane", "process-info", "--pane", pane]) else {
        return false;
    };
    let processes = [
        "/result/process_info/foreground_processes",
        "/process_info/foreground_processes",
    ]
    .iter()
    .find_map(|pointer| process.pointer(pointer))
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
    only_shells(&processes)
}

/// Every foreground process is a shell. The journal has already said the worker ended; this catches its
/// supervisor still drawing the footer, or the owner running something in the pane since.
fn only_shells(processes: &[Value]) -> bool {
    processes.iter().all(|process| {
        let name = process
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        matches!(
            name,
            "pwsh" | "powershell" | "cmd" | "bash" | "zsh" | "sh" | "fish" | "nu"
        )
    })
}

/// Starts `seat exec` with no pane: its view goes to `streams/<order>.log`.
fn spawn_detached(project: &Project, run: &str, seated: &Seated) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let streams = project.run_dir(run).join("streams");
    std::fs::create_dir_all(&streams).map_err(|err| err.to_string())?;
    let log_path = streams.join(format!("{}.log", seated.order));
    let spawn = |breakaway: bool| -> std::io::Result<u32> {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let mut command = std::process::Command::new(&exe);
        command
            .args(["seat", "exec", &seated.order, "--run", run])
            .env(nirvana_run::PROJECT_ENV, &project.root)
            .current_dir(&seated.cwd)
            .stdin(std::process::Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use windows_sys::Win32::System::Threading::{
                CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW,
            };
            // Out of the caller's job when it allows it: a Bash tool's kill-on-close job would end the worker
            // with the command that started it.
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
    let pid = spawn(true)
        .or_else(|_| spawn(false))
        .map_err(|err| err.to_string())?;
    Ok(format!(
        "detached pid {pid}, view in {}",
        log_path.display()
    ))
}

/// Starts the order's supervisor: in a pane inside herdr, detached otherwise or with `NIRVANA_NO_PANE=1`.
pub fn launch_worker(project: &Project, run: &str, seated: &Seated) -> Result<String, String> {
    if live(project, run, &seated.order) {
        return Ok("already live".into());
    }
    if in_herdr() && std::env::var_os("NIRVANA_NO_PANE").is_none() {
        open_pane(project, run, seated)
    } else {
        spawn_detached(project, run, seated)
    }
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr seat run <seat> --order <file> [--order-id id] [--run id] [--class c] [--size s]
                      [--needs a,b] [--not-family f] [--mode m] [--effort e] [--reason \"why\"] [--worktree] [--no-pane]
                      [--no-auto] [--send-ok \"the owner's words\"] [--profile name] [--json]
       herdr seat exec <order> --run <run> [--pane <pane>]   the worker's supervisor (a pane runs it)
       herdr seat status [<order>|--run <run>] [--json]
       herdr seat read <order> [--tail n]
       herdr seat stop <order>
       herdr seat cleanup <order>";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        Some("run") => run_cli(rest),
        Some("exec") => {
            let mut order: Option<String> = None;
            let mut run: Option<String> = None;
            let mut pane: Option<String> = None;
            let mut iter = rest.iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--run" => run = iter.next().cloned(),
                    "--pane" => pane = iter.next().cloned(),
                    other if !other.starts_with("--") && order.is_none() => {
                        order = Some(other.to_string());
                    }
                    _ => usage_exit(),
                }
            }
            let (Some(order), Some(run)) = (order, run) else {
                usage_exit();
            };
            if !valid_id(&order) || !valid_id(&run) {
                usage_exit();
            }
            let project = Project::from_env();
            let mut out = std::io::stdout();
            let code = exec(&project, &run, &order, pane.as_deref(), &mut out)?;
            out.flush()?;
            std::process::exit(code);
        }
        Some("status") => status_cli(rest, false),
        Some("read") => read_cli(rest),
        Some("stop") => stop_cli(rest),
        Some("cleanup") => cleanup_cli(rest),
        _ => usage_exit(),
    }
}

/// An order id from a file name: lowercase letters, digits and dashes.
pub fn order_id_from(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let id: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect();
    if id.is_empty() {
        "order".into()
    } else {
        id
    }
}

fn run_cli(args: &[String]) -> std::io::Result<()> {
    let Some(seat) = args.first().filter(|arg| !arg.starts_with("--")).cloned() else {
        usage_exit()
    };
    let mut order_file: Option<PathBuf> = None;
    let mut order_id: Option<String> = None;
    let mut run: Option<String> = None;
    let mut class: Option<String> = None;
    let mut size: Option<SizeName> = None;
    let mut needs: Vec<String> = Vec::new();
    let mut not_family: Vec<String> = Vec::new();
    let mut mode: Option<Mode> = None;
    let mut effort: Option<Effort> = None;
    let mut reason: Option<String> = None;
    let mut send_ok: Option<String> = None;
    let mut profile: Option<String> = None;
    let (mut worktree, mut no_pane, mut auto, mut json) = (false, false, true, false);
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--order" => order_file = Some(PathBuf::from(value())),
            "--order-id" => order_id = Some(value()),
            "--run" => run = Some(value()),
            "--class" => class = Some(value()),
            "--size" => size = Some(SizeName::parse(&value()).unwrap_or_else(|| usage_exit())),
            "--needs" => {
                needs = value()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            }
            "--not-family" => {
                not_family = value()
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            }
            "--mode" => mode = Some(Mode::parse(&value()).unwrap_or_else(|| usage_exit())),
            "--effort" => {
                effort = Some(
                    serde_json::from_value(Value::String(value())).unwrap_or_else(|_| usage_exit()),
                )
            }
            "--reason" => reason = Some(value()),
            "--send-ok" => send_ok = Some(value()),
            "--profile" => profile = Some(value()),
            "--worktree" => worktree = true,
            "--no-pane" => no_pane = true,
            "--no-auto" => auto = false,
            "--json" => json = true,
            _ => usage_exit(),
        }
    }
    let Some(order_file) = order_file else {
        usage_exit()
    };
    let order = order_id.unwrap_or_else(|| order_id_from(&order_file));
    let run = run
        .or_else(|| std::env::var("NIRVANA_RUN").ok())
        .unwrap_or_else(|| {
            let stamp = utc(now()).replace(['-', ':', 'Z'], "").replace(' ', "-");
            format!("run-{stamp}")
        });
    if !valid_id(&order) || !valid_id(&run) {
        eprintln!(
            "herdr seat run: ids are 1 to 64 of [A-Za-z0-9._-]: order {order:?}, run {run:?}"
        );
        std::process::exit(2);
    }
    if no_pane {
        std::env::set_var("NIRVANA_NO_PANE", "1");
    }
    let request = SeatRequest {
        seat,
        order_file,
        order,
        run,
        class,
        size,
        needs,
        not_family,
        mode,
        effort,
        reason,
        worktree,
        auto,
        send_ok,
    };
    let project = Project::from_env();
    let outcome = seat_order_with(&project, &request, profile.as_deref());
    match outcome {
        Ok(outcome) => {
            // Plan 31 §2.1: one line, never a refusal; on stderr, so `--json` stays one JSON value.
            if let Some(warning) = outcome["warning"].as_str() {
                eprintln!("herdr seat run: warning: {warning}");
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&outcome)?);
            } else {
                println!(
                    "{} {} · run {} · {} {} {} · {}",
                    outcome["order"].as_str().unwrap_or(""),
                    outcome["booked"].as_str().unwrap_or(""),
                    outcome["run"].as_str().unwrap_or(""),
                    outcome["seat"].as_str().unwrap_or(""),
                    outcome["model"].as_str().unwrap_or(""),
                    outcome["effort"].as_str().unwrap_or(""),
                    outcome["worker"].as_str().unwrap_or("")
                );
                if let Some(reason) = outcome["reason"].as_str() {
                    println!("{reason}");
                }
                if let Some(profile) = outcome["profile"].as_object() {
                    let field = |key: &str| profile.get(key).and_then(Value::as_str).unwrap_or("");
                    println!(
                        "profile {} (sha256 {}), copied to {}",
                        field("name"),
                        field("sha256"),
                        field("copy")
                    );
                }
                // Plan 31 review B1 item 4: the prompt the approval was read from, so the owner can see it is theirs.
                if let Some(send_ok) = outcome["send_ok"].as_object() {
                    let field = |key: &str| send_ok.get(key).and_then(Value::as_str).unwrap_or("");
                    println!(
                        "send approved by the owner's prompt at {} in {}:\n> {}",
                        field("at"),
                        field("transcript"),
                        field("prompt").replace('\n', "\n> ")
                    );
                }
            }
            Ok(())
        }
        Err((code, why)) => {
            eprintln!("herdr seat run: {why}");
            std::process::exit(code);
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeatRequest {
    pub seat: String,
    pub order_file: PathBuf,
    pub order: String,
    pub run: String,
    pub class: Option<String>,
    pub size: Option<SizeName>,
    pub needs: Vec<String>,
    pub not_family: Vec<String>,
    pub mode: Option<Mode>,
    pub effort: Option<Effort>,
    pub reason: Option<String>,
    pub worktree: bool,
    pub auto: bool,
    /// Plan 31 §2.2: `--send-ok "<the owner's words>"`, checked against the owner's prompts before anything is booked.
    pub send_ok: Option<String>,
}

/// What an existing order answers a second `seat run` with: the first one (§7.2, idempotent per order id).
fn existing(run: &str, known: &nirvana_run::OrderState, live: bool) -> Value {
    serde_json::json!({
        "order": known.id(),
        "run": run,
        "booked": "existing",
        "phase": known.phase.word(),
        "live": live,
        "seat": known.seated.seat,
        "model": known.seated.model,
        "effort": known.seated.effort.word(),
        "reason": known.seated.reason,
        "worker": known.pane.clone().map_or_else(
            || if live { "live".to_owned() } else { known.phase.word().to_owned() },
            |pane| format!("pane {pane}")
        ),
    })
}

/// The events that seat an order, or the order as it already is: a second `seat run` for an order id the run
/// knows returns the first and writes nothing (§7.2: "a resumed orchestrator cannot seat a task twice"). Called
/// under the journal's lock, so two at once cannot both seat it.
pub fn seating(
    events: &[Event],
    seated: &Seated,
    opened: Kind,
) -> Result<Vec<Kind>, Box<nirvana_run::OrderState>> {
    let state = replay(events);
    if let Some(known) = state.order(&seated.order) {
        return Err(Box::new(known.clone()));
    }
    let mut kinds = Vec::new();
    if state.opened.is_none() {
        kinds.push(opened);
    }
    kinds.push(Kind::Seated(Box::new(seated.clone())));
    Ok(kinds)
}

/// Books the order on the seat (the rule must agree, or the caller gives a reason: an override, 3 a run). The tests'
/// entry; `seat_order_with` books through `book_with`.
#[cfg_attr(not(test), allow(dead_code))]
fn book(project: &Project, request: &SeatRequest) -> Result<Booked, (i32, String)> {
    book_with(project, request, None)
}

/// `book` with a profile, which narrows the pick (plan 33 §2.2); `request.needs` already holds its needs.
fn book_with(
    project: &Project,
    request: &SeatRequest,
    profile: Option<&crate::nirvana_profiles::Profile>,
) -> Result<Booked, (i32, String)> {
    let table = nirvana_seat::table_for(&project.root).map_err(|why| (2, why))?;
    let Some(seat) = table.seat(&request.seat) else {
        return Err((2, format!("no seat {} in seats.toml", request.seat)));
    };
    let dir = nirvana_seat::dir();
    let leases_path = dir.join(nirvana_seat::LEASES_FILE);
    let mut world = nirvana_seat::live_world(&table);
    // Plan 29 §5.1: a need naming one of the project's MCP servers is no seat capability.
    let needs = crate::nirvana_gui_tools::seat_needs(&table, &project.root, &request.needs)
        .map_err(|why| (2, why))?;
    let words = nirvana_seat::unknown_words(&table, &needs.rest, &request.not_family);
    if !words.is_empty() {
        return Err((2, words.join("; ")));
    }
    let lock = nirvana_seat::Lock::take(&dir).map_err(|err| (2, err.to_string()))?;
    // `load_leases`, never the lossy read: a corrupt file read as empty and written back would lose every
    // run's override count (the O2 fix round's exit 2).
    let mut leases = nirvana_seat::load_leases(&leases_path).map_err(|why| (2, why))?;
    world.now = now();
    let mut booked: Booked = if let Some(lease) = leases.live(&request.order, world.now) {
        serde_json::from_value(lease.pick.clone()).map_err(|err| (2, err.to_string()))?
    } else {
        let Some(class) = &request.class else {
            return Err((
                2,
                format!(
                    "{} has no booking: name its --class, or book it with `herdr seat pick --book {}`",
                    request.order, request.order
                ),
            ));
        };
        if table.class(class).is_none() {
            return Err((2, format!("no class {class} in seats.toml")));
        }
        let mut pick_request = nirvana_seat::Request::new(class);
        pick_request.mode = request.mode.unwrap_or(table.mode);
        if let Some(size) = request.size {
            pick_request.size = size;
        }
        pick_request.needs = needs.rest.clone();
        pick_request.claude_only = needs.claude_only.clone();
        pick_request.not_family = request.not_family.clone();
        pick_request.order = Some(request.order.clone());
        if let Some(profile) = profile {
            crate::nirvana_profiles::narrow(&mut pick_request, profile);
        }
        world.leases = leases.leases.clone();
        world.overrides_used = leases.overrides(&request.run);
        let mut pick = nirvana_seat::pick(&table, &world, &pick_request);
        if pick.as_ref().is_ok_and(|pick| pick.seat != request.seat) {
            let Some(reason) = &request.reason else {
                let rule = pick
                    .as_ref()
                    .map(|pick| pick.reason.clone())
                    .unwrap_or_default();
                return Err((
                    3,
                    format!(
                        "the rule seats this order elsewhere ({rule}); pass --reason to override (at most {} a run)",
                        table.constants.overrides_per_run
                    ),
                ));
            };
            pick_request.override_seat = Some((request.seat.clone(), reason.clone()));
            pick = nirvana_seat::pick(&table, &world, &pick_request);
        }
        // Plan 12 O8b (§9 role 3): the caller named its seat, so Jev's advice is recorded and never acts here.
        let client = crate::nirvana_jev::live_client().map(|(client, _)| client);
        let pick = pick.map(|chosen| {
            crate::nirvana_jev_roles::advise_seating(
                client.as_ref(),
                &dir,
                crate::nirvana_jev_roles::settings_in(&dir).seating,
                &table,
                &world,
                &pick_request,
                chosen,
                false,
            )
        });
        let pick = pick.map_err(|no_seat| {
            (
                nirvana_seat::NO_SEAT_EXIT,
                format!(
                    "no seat: {}{}",
                    no_seat.reason,
                    no_seat
                        .earliest_reset
                        .map(|at| format!("; earliest reset {}", utc(at)))
                        .unwrap_or_default()
                ),
            )
        })?;
        // The fix round's `book_pick`: the override is counted with its time, so the 8-day prune keeps it.
        let (lease, _) = nirvana_seat::book_pick(
            &mut leases,
            &table,
            &pick,
            Some(&request.order),
            Some(&request.run),
            world.now,
        );
        let Some(lease) = lease else {
            return Err((2, format!("{} was not booked", request.order)));
        };
        nirvana_seat::write_leases(&leases_path, &leases).map_err(|err| (2, err.to_string()))?;
        serde_json::from_value(lease.pick).map_err(|err| (2, err.to_string()))?
    };
    drop(lock);
    if booked.seat != request.seat {
        return Err((
            2,
            format!(
                "{} is booked on {}, not {}",
                request.order, booked.seat, request.seat
            ),
        ));
    }
    if let Some(effort) = request.effort.filter(|effort| *effort != booked.effort) {
        if !seat.efforts.contains(&effort) {
            return Err((2, format!("{} has no {} effort", seat.id, effort.word())));
        }
        booked.reason = format!("{}; effort {} by the caller", booked.reason, effort.word());
        booked.effort = effort;
        booked.model = seat.model_for(effort);
    }
    booked.servers = needs.servers;
    Ok(booked)
}

fn git_in(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|err| err.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

/// `herdr seat run`: book, copy the order, open the worktree, journal `seated`, start the worker. The tests' entry;
/// the CLI passes its `--profile` to `seat_order_with`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn seat_order(project: &Project, request: &SeatRequest) -> Result<Value, (i32, String)> {
    seat_order_with(project, request, None)
}

/// `seat_order` with `--profile <name>` (plan 33 §2.2): the project's profile file is read once, its class used when
/// the order names none, its needs joined to the order's, the pick narrowed by it, and the bytes copied into the run
/// with their sha, which the order copy and the `seated` event's reason name.
pub fn seat_order_with(
    project: &Project,
    request: &SeatRequest,
    profile: Option<&str>,
) -> Result<Value, (i32, String)> {
    let io = |err: std::io::Error| (2, err.to_string());
    project.ensure().map_err(io)?;
    let journal = project.journal(&request.run);
    // A second run for a live order returns the first, before anything is booked or written.
    let state = replay(&journal.read().0);
    if let Some(known) = state.order(&request.order) {
        let live = live(project, &request.run, &request.order);
        if known.phase.terminal() {
            return Err((
                2,
                format!(
                    "{} already ran in {} and is {}; seat a new order id",
                    request.order,
                    request.run,
                    known.phase.word()
                ),
            ));
        }
        return Ok(existing(&request.run, known, live));
    }
    let text = std::fs::read_to_string(&request.order_file).map_err(|err| {
        (
            2,
            format!(
                "cannot read the order {}: {err}",
                request.order_file.display()
            ),
        )
    })?;
    let spec = crate::nirvana_gate::spec(&text);
    // Plan 33 §2.2. An order whose text already says `Profile:` at line start is refused unless it is seated with that
    // profile: P33b reads the line, and a profile herdr never applied must not scope its lessons.
    let loaded = profile
        .map(|name| crate::nirvana_profiles::load(&project.root, name))
        .transpose()
        .map_err(|why| (2, why))?;
    let written = crate::nirvana_profiles::in_order(&text).map(|found| match found {
        Ok(found) => found.name,
        Err(_) => text
            .lines()
            .find_map(|line| line.strip_prefix(crate::nirvana_profiles::LINE))
            .unwrap_or("")
            .trim()
            .to_owned(),
    });
    if let Some(written) = written.filter(|written| Some(written.as_str()) != profile) {
        return Err((
            2,
            format!(
                "the order says `{} {written}` at line start; seat it with `--profile {written}`, or take the line out",
                crate::nirvana_profiles::LINE
            ),
        ));
    }
    let request = &match &loaded {
        Some(loaded) => {
            let mut request = request.clone();
            request.needs = crate::nirvana_profiles::joined_needs(&request.needs, &loaded.profile);
            if request.class.is_none() {
                request.class = loaded.profile.class.clone();
            }
            request
        }
        None => request.clone(),
    };
    // Plan 31 §2.2: a quote herdr cannot find in the owner's own prompts refuses the order, before it is booked.
    let send_ok = match &request.send_ok {
        Some(quote) => {
            // Review N3: the Send line goes only to an order granted a device server, so an approval for an order
            // with none would be journaled and never told to its worker. It is refused instead, naming the fix.
            let defined =
                crate::nirvana_gui_tools::project_servers(&project.root).unwrap_or_default();
            if !request.needs.iter().any(|need| defined.contains(need)) {
                return Err((
                    2,
                    "--send-ok needs an order granted a device server: name it in --needs (bambu-studio-beta, xtool, \
                     bambu)"
                        .into(),
                ));
            }
            Some(
                crate::nirvana_gui_tools::verify_send_ok(
                    crate::nirvana::home().as_deref(),
                    &send_ok_folder(project),
                    quote,
                )
                .map_err(|why| (2, why))?,
            )
        }
        None => None,
    };
    if let Some(class) = &request.class {
        if crate::nirvana_gate::requires_owns(class) && spec.owns.is_empty() {
            return Err((
                2,
                "a build order needs an `owns:` line naming the files it may change".into(),
            ));
        }
    }
    let mut booked = book_with(
        project,
        request,
        loaded.as_ref().map(|loaded| &loaded.profile),
    )?;
    // A booking `seat pick --book` narrowed by a profile is seated with that profile, and only that one.
    let named = loaded.as_ref().map(|loaded| loaded.profile.name.as_str());
    if booked.profile.as_deref() != named {
        return Err((
            2,
            match (&booked.profile, named) {
                (Some(booked), _) => format!(
                    "{} was booked with profile {booked}: seat it with `--profile {booked}`",
                    request.order
                ),
                (None, Some(named)) => format!(
                    "{} was booked without a profile: `herdr seat release {}`, then book it with `--profile {named}`",
                    request.order, request.order
                ),
                (None, None) => unreachable!("equal"),
            },
        ));
    }
    // §2.2: "The profile file is copied, with its sha256, into the run folder at seating."
    let profiled = match &loaded {
        Some(loaded) => {
            let copy =
                crate::nirvana_profiles::copy_into_run(&project.run_dir(&request.run), loaded)
                    .map_err(|why| (2, why))?;
            booked.reason = format!(
                "{}; profile {} sha256 {}",
                booked.reason, loaded.profile.name, loaded.sha
            );
            Some((
                crate::nirvana_profiles::Ref {
                    name: loaded.profile.name.clone(),
                    sha: loaded.sha.clone(),
                },
                copy,
            ))
        }
        None => None,
    };
    // With no --class, `book` only reuses a booking made by `herdr seat pick --book` and writes nothing, so the
    // refusal keeps that booking: the retry, once `owns:` is added, must not have to book again.
    if crate::nirvana_gate::requires_owns(&booked.class) && spec.owns.is_empty() {
        return Err((
            2,
            "a build order needs an `owns:` line naming the files it may change".into(),
        ));
    }
    let (cwd, worktree, branch) = if request.worktree {
        let path = project.worktrees_dir().join(&request.order);
        let branch = format!("nirvana/{}", request.order);
        if !path.exists() {
            git_in(
                &project.root,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &branch,
                    &path.display().to_string(),
                    "HEAD",
                ],
            )
            .map_err(|why| (2, format!("git worktree add: {why}")))?;
        }
        (path.clone(), Some(path.display().to_string()), Some(branch))
    } else {
        (project.root.clone(), None, None)
    };
    // Plan 12 O5 (§7.4.3): one line per live lesson of the class, the path and the description, never the body.
    let lessons = crate::nirvana_lessons::offer_for(
        &project.root,
        &cwd,
        &booked.class,
        // The `--profile` the order is seated with: the order as written carries no `Profile:` line, herdr adds it
        // to the copy below, and a written line that differs from `--profile` was refused above.
        profile,
    );
    // Plan 31 §2.1: read from the order as written, before herdr's lessons and lines, which name apps of their own.
    let warning = crate::nirvana_gui_tools::server_hint(
        &text,
        &crate::nirvana_gui_tools::project_servers(&project.root).unwrap_or_default(),
        &booked.servers,
    );
    let mut text = if lessons.is_empty() {
        text
    } else {
        format!("{}{lessons}", text.trim_end())
    };
    // Plan 29 §5.3 and review F4: an order granted a server never starts a device; a desktop one batches, reads crops.
    // Plan 31 §2.2: an order with the owner's verified approval may Send once, after an explained slice.
    text = crate::nirvana_gui_tools::with_brief_lines(text, &booked.servers, send_ok.is_some());
    if booked.class.starts_with("research.") || booked.class.starts_with("media.") {
        text.push_str("\n\nYour Work Report must include the `sources` array of `{url, used_for}` for each source consulted.");
    }
    // Plan 25 §5: a browser order's worker batches its steps, so only the result reaches the model.
    // `booked.cli` is the booked seat's CLI (`seats.toml`'s `cli`), the one `seat exec` starts.
    let text =
        crate::nirvana_lean::with_browser_line(text, &booked.cli, &booked.class, &request.needs);
    // Plan 33 §2.3: the `Profile:` lines at the top, and for agy the body above the order.
    let text = match (&profiled, &loaded) {
        (Some((found, _)), Some(loaded)) => {
            crate::nirvana_profiles::order_text(&text, found, &loaded.profile, &booked.cli)
        }
        _ => text,
    };
    let order_file = write_order(project, &cwd, &request.order, &text).map_err(io)?;
    let seated = Seated {
        order: request.order.clone(),
        seat: booked.seat.clone(),
        cli: booked.cli.clone(),
        model: booked.model.clone(),
        effort: booked.effort,
        pool: booked.pool.clone(),
        class: booked.class.clone(),
        chain: booked.chain.clone(),
        size: booked.size,
        mode: booked.mode,
        needs: request.needs.clone(),
        not_family: request.not_family.clone(),
        reason: booked.reason.clone(),
        override_reason: booked.override_reason.clone(),
        order_file,
        cwd: cwd.display().to_string(),
        worktree,
        branch,
        parent: None,
        why: None,
        rule_pick: booked.rule_pick.clone(),
        bar_dropped: booked.bar_dropped,
        step_down: booked.step_down,
        use_basis: booked.use_basis.clone(),
        expected: booked.expected(),
        jev_pick: booked.jev_pick.clone(),
        send_ok,
    };
    let (pools, pools_basis) = nirvana_run::cached_pools();
    let opened = Kind::Opened {
        project: project.root.display().to_string(),
        orchestrator_pane: std::env::var("HERDR_PANE_ID").ok(),
        auto: request.auto,
        pools,
        pools_basis,
    };
    let (written, first) = journal
        .transact(now(), |events: &[Event]| {
            Ok(match seating(events, &seated, opened) {
                Ok(kinds) => (kinds, None),
                Err(known) => (Vec::new(), Some(existing(&request.run, &known, false))),
            })
        })
        .map_err(io)?;
    if let Some(first) = first {
        return Ok(first);
    }
    let worker = launch_worker(project, &request.run, &seated).map_err(|why| (1, why))?;
    Ok(serde_json::json!({
        "order": seated.order,
        "run": request.run,
        "booked": "new",
        "seat": seated.seat,
        "cli": seated.cli,
        "model": seated.model,
        "effort": seated.effort.word(),
        "pool": seated.pool,
        "reason": seated.reason,
        "order_file": seated.order_file,
        "cwd": seated.cwd,
        "journal": journal.path.display().to_string(),
        "events": written.len(),
        "worker": worker,
        "warning": warning,
        "send_ok": seated.send_ok,
        "profile": profiled.map(|(found, copy)| serde_json::json!({
            "name": found.name,
            "sha256": found.sha,
            "copy": copy.display().to_string(),
        })),
    }))
}

/// The folder Nirvana Claude's session is recorded under (`nirvana_session_record`): the directory it started in,
/// which Claude Code passes its tools as `CLAUDE_PROJECT_DIR`, else the project's root.
fn send_ok_folder(project: &Project) -> PathBuf {
    std::env::var_os(crate::nirvana_session_record::PROJECT_DIR_ENV)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| project.root.clone(), PathBuf::from)
}

/// `herdr seat status` (positional: an order) and `herdr run status` (positional: a run).
pub fn status_cli(args: &[String], positional_is_run: bool) -> std::io::Result<()> {
    let project = Project::from_env();
    let json = args.iter().any(|arg| arg == "--json");
    let mut run: Option<String> = None;
    let mut order: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => {}
            "--run" => run = iter.next().cloned(),
            other if !other.starts_with("--") => {
                if positional_is_run {
                    run = Some(other.to_owned());
                } else {
                    order = Some(other.to_owned());
                }
            }
            _ => usage_exit(),
        }
    }
    if let Some(order) = &order {
        run = run.or_else(|| project.run_of(order));
        if run.is_none() {
            eprintln!("herdr seat status: no run seats {order}");
            std::process::exit(1);
        }
    }
    let runs = match run {
        Some(run) => vec![run],
        None => project.runs(),
    };
    let mut views = Vec::new();
    for run in &runs {
        let state = replay(&project.journal(run).read().0);
        for known in &state.orders {
            if order.as_deref().is_some_and(|order| order != known.id()) {
                continue;
            }
            let is_live = live(&project, run, known.id());
            views.push(serde_json::json!({
                "run": run,
                "order": known.id(),
                "phase": known.phase.word(),
                "live": is_live,
                "seat": known.seated.seat,
                "model": known.seated.model,
                "effort": known.seated.effort.word(),
                "pool": known.seated.pool,
                "class": known.seated.class,
                "attempts": known.attempts,
                "tokens": known.tokens(),
                "pane": known.pane,
                "deferred_until": known.deferred_until,
                "last_at": known.last_at,
                "why": known.why,
                "parent": known.seated.parent,
            }));
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&views)?);
        return Ok(());
    }
    if views.is_empty() {
        println!("no order in {}", project.dir().display());
    }
    for view in &views {
        let text = |key: &str| view[key].as_str().unwrap_or("").to_owned();
        let tokens = view["tokens"]
            .as_object()
            .map(|t| {
                format!(
                    " · in {} out {}",
                    t["input"].as_u64().unwrap_or(0) + t["cache_read"].as_u64().unwrap_or(0),
                    t["output"].as_u64().unwrap_or(0)
                )
            })
            .unwrap_or_default();
        println!(
            "{} {} {}{} · {} {} · pool {} · attempt {}{} · last {}{}",
            text("run"),
            text("order"),
            text("phase"),
            if view["live"].as_bool() == Some(true) {
                " (live)"
            } else {
                ""
            },
            text("seat"),
            text("effort"),
            text("pool"),
            view["attempts"],
            tokens,
            view["last_at"].as_u64().map(utc).unwrap_or_default(),
            view["why"]
                .as_str()
                .map(|why| format!(" · {why}"))
                .unwrap_or_default()
        );
    }
    Ok(())
}

fn order_and_run(project: &Project, order: &str) -> (String, Journal) {
    if !valid_id(order) {
        usage_exit();
    }
    let Some(run) = project.run_of(order) else {
        eprintln!("herdr seat: no run seats {order}");
        std::process::exit(1);
    };
    let journal = project.journal(&run);
    (run, journal)
}

fn read_cli(args: &[String]) -> std::io::Result<()> {
    let Some(order) = args.first() else {
        usage_exit()
    };
    let tail = match args.get(1..) {
        Some([flag, n]) if flag == "--tail" => n.parse::<usize>().unwrap_or_else(|_| usage_exit()),
        Some([]) | None => 40,
        _ => usage_exit(),
    };
    let project = Project::from_env();
    let (run, journal) = order_and_run(&project, order);
    let state = replay(&journal.read().0);
    let Some(known) = state.order(order) else {
        usage_exit()
    };
    if let Some(report) = &known.report {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    let path = project
        .run_dir(&run)
        .join("streams")
        .join(format!("{order}.jsonl"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut stream = Stream::new(&known.seated.cli).in_dir(&known.seated.cwd);
    let mut shown = Vec::new();
    for line in text.lines() {
        for note in stream.feed(line) {
            shown.extend(view::note_lines(&note, false));
        }
    }
    let start = shown.len().saturating_sub(tail);
    println!(
        "{order} is {}; no report yet. The stream's last {} line(s):",
        known.phase.word(),
        shown.len() - start
    );
    for line in &shown[start..] {
        println!("{line}");
    }
    Ok(())
}

fn stop_cli(args: &[String]) -> std::io::Result<()> {
    let [order] = args else { usage_exit() };
    let project = Project::from_env();
    let (run, journal) = order_and_run(&project, order);
    if live(&project, &run, order) {
        std::fs::write(stop_file(&project, &run, order), now().to_string())?;
        let started = Instant::now();
        while live(&project, &run, order) && started.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(250));
        }
        if live(&project, &run, order) {
            println!("{order}: asked to stop; its supervisor has not ended yet");
        } else {
            println!("{order}: stopped, checkpoint written");
        }
        return Ok(());
    }
    let state = replay(&journal.read().0);
    let Some(known) = state.order(order) else {
        usage_exit()
    };
    if !known.phase.wants_worker() {
        println!("{order} is {}; nothing runs", known.phase.word());
        return Ok(());
    }
    // No supervisor (it died, or a deferral's pane is gone): the stop is written here.
    stopped(
        &project,
        &journal,
        &run,
        &known.seated,
        None,
        &mut std::io::stdout(),
    )?;
    Ok(())
}

/// `git worktree remove --force`, retried for 5 s while a closing pane's shell lets go of the folder; a folder
/// git has already unregistered is deleted and pruned.
fn remove_worktree(root: &Path, worktree: &Path) -> Result<(), String> {
    let path = worktree.display().to_string();
    let started = Instant::now();
    loop {
        let removed = git_in(root, &["worktree", "remove", "--force", &path]);
        if removed.is_ok() || !worktree.exists() {
            let _ = git_in(root, &["worktree", "prune"]);
            return Ok(());
        }
        let unregistered = git_in(root, &["worktree", "list", "--porcelain"])
            .map(|list| {
                !list
                    .lines()
                    .filter_map(|line| line.strip_prefix("worktree "))
                    .any(|known| Path::new(known) == worktree)
            })
            .unwrap_or(false);
        if unregistered && std::fs::remove_dir_all(worktree).is_ok() {
            let _ = git_in(root, &["worktree", "prune"]);
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(5) {
            return removed.map(|_| ());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn cleanup_cli(args: &[String]) -> std::io::Result<()> {
    let [order] = args else { usage_exit() };
    let project = Project::from_env();
    let (run, journal) = order_and_run(&project, order);
    if live(&project, &run, order) {
        stop_cli(args)?;
    }
    let state = replay(&journal.read().0);
    let Some(known) = state.order(order) else {
        usage_exit()
    };
    // A child of a re-seat shares its parent's worktree; the last order on it removes it.
    let shared = state.orders.iter().any(|other| {
        other.id() != order
            && other.seated.worktree == known.seated.worktree
            && !other.phase.terminal()
    });
    // The pane first: its shell's folder is the worktree, and Windows will not delete a folder a process has
    // as its current directory (the live run of 2026-10-18: "failed to delete ... Permission denied").
    if let Some(pane) = known.pane.as_deref().filter(|_| in_herdr()) {
        let expected = expected_pane_title(&known.seated.seat, order);
        let actual = pane_title(pane);
        if actual.as_deref() == Some(&expected) {
            match herdr_json(&["pane", "close", pane]) {
                Ok(_) => println!("closed the pane {pane}"),
                Err(why) => println!("the pane {pane} stays: {why}"),
            }
        } else {
            println!("the pane {pane} stays: title is not {expected}");
        }
    }
    if let Some(worktree) = known.seated.worktree.as_deref().filter(|_| !shared) {
        match remove_worktree(&project.root, Path::new(worktree)) {
            Ok(()) => println!("removed the worktree {worktree}"),
            Err(why) => println!("the worktree {worktree} stays: {why}"),
        }
    }
    release_lease(order);
    println!(
        "{order}: cleaned up; its branch {} stays",
        known.seated.branch.as_deref().unwrap_or("(none)")
    );
    Ok(())
}

mod place;
mod view;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod preset_tests {
    use super::*;

    #[test]
    fn a_claude_worker_gets_the_folders_preset_file_and_agy_does_not() {
        let file = Path::new(r"C:\home\nirvana\preload\0123456789abcdef.worker.settings.json");
        let claude = vec!["claude".to_owned(), "-p".to_owned(), "go".to_owned()];
        assert_eq!(
            with_preset_settings(claude.clone(), Some(file)),
            vec![
                "claude".to_owned(),
                "-p".to_owned(),
                "go".to_owned(),
                "--settings".to_owned(),
                file.display().to_string(),
            ]
        );
        // No generated file: the argv O3 measured, unchanged.
        assert_eq!(with_preset_settings(claude.clone(), None), claude);
        let agy = vec!["agy".to_owned(), "--print=go".to_owned()];
        assert_eq!(with_preset_settings(agy.clone(), Some(file)), agy);
    }

    fn spec(cli: &str, model: &str) -> LaunchSpec {
        LaunchSpec {
            cli: cli.into(),
            model: model.into(),
            effort: Effort::Medium,
            prompt: "go".into(),
            resume: None,
            print_timeout_s: 0,
        }
    }

    #[test]
    fn an_opus_claude_worker_starts_with_superpowers_off_and_an_error_keeps_the_preset() {
        // Plan 28 §3.
        let root = std::env::temp_dir().join(format!("nirvana-p28-worker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("nirvana");
        let folder = root.join("project");
        std::fs::create_dir_all(&folder).unwrap();
        let mut out = Vec::new();
        let opus = spec("claude", "claude-opus-5-5");
        // No preset: a file of the rule's entries alone, added to the argv.
        let settings = worker_settings(&dir, &opus, &folder, None, None, &mut out).expect("a file");
        assert_eq!(
            settings,
            crate::nirvana_superpowers::worker_settings_path(
                &dir,
                &folder,
                crate::nirvana_superpowers::Family::Opus
            )
        );
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        assert_eq!(
            written,
            serde_json::json!({"enabledPlugins": {"superpowers@superpowers-marketplace": false}})
        );
        let claude = argv(&opus).unwrap();
        let with = with_preset_settings(claude.clone(), Some(&settings));
        assert_eq!(with.iter().filter(|a| *a == "--settings").count(), 1);
        assert_eq!(with.last(), Some(&settings.display().to_string()));
        // Sonnet with no preset: today's argv, no file.
        let sonnet = spec("claude", "claude-sonnet-5-5");
        assert_eq!(
            worker_settings(&dir, &sonnet, &folder, None, None, &mut out),
            None
        );
        // agy on an Opus model reads no Claude settings: no file.
        let agy = spec("agy", "claude-opus-5-5-high");
        assert_eq!(
            worker_settings(&dir, &agy, &folder, None, None, &mut out),
            None
        );
        assert!(out.is_empty());
        // A preset that cannot be read: the preset kept, one line in the view.
        let broken = root.join("broken.worker.settings.json");
        std::fs::write(&broken, "{not json").unwrap();
        assert_eq!(
            worker_settings(&dir, &opus, &folder, Some(broken.clone()), None, &mut out),
            Some(broken)
        );
        let said = String::from_utf8(out).unwrap();
        assert_eq!(said.lines().count(), 1, "{said:?}");
        assert!(said.starts_with("superpowers stay as the settings say for claude-opus-5-5"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_workers_argv_carries_the_preset_under_the_rule_for_its_own_model() {
        // Plan 28 §3: what `seat exec` starts, from the folder's worker preset and `spec.model`.
        let root = std::env::temp_dir().join(format!("nirvana-p28-argv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("nirvana");
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let preset = crate::nirvana_preset::worker_settings_path(&dir, &project);
        std::fs::create_dir_all(preset.parent().unwrap()).unwrap();
        std::fs::write(&preset, r#"{"skillOverrides":{"docx":"off"}}"#).unwrap();
        let settings_of = |argv: &[String]| -> Option<String> {
            assert!(argv.iter().filter(|a| *a == "--settings").count() <= 1);
            let at = argv.iter().position(|a| a == "--settings")?;
            Some(argv[at + 1].clone())
        };
        let mut out = Vec::new();
        let opus = worker_argv(
            &dir,
            &spec("claude", "claude-opus-5-5"),
            &project,
            &project,
            None,
            &mut out,
        )
        .unwrap();
        let launch = crate::nirvana_superpowers::worker_settings_path(
            &dir,
            &project,
            crate::nirvana_superpowers::Family::Opus,
        );
        assert_eq!(settings_of(&opus), Some(launch.display().to_string()));
        let written: Value =
            serde_json::from_str(&std::fs::read_to_string(&launch).unwrap()).unwrap();
        assert_eq!(
            written,
            serde_json::json!({
                "skillOverrides": {"docx": "off"},
                "enabledPlugins": {"superpowers@superpowers-marketplace": false},
            })
        );
        let sonnet = worker_argv(
            &dir,
            &spec("claude", "claude-sonnet-5-5"),
            &project,
            &project,
            None,
            &mut out,
        )
        .unwrap();
        assert_eq!(settings_of(&sonnet), Some(preset.display().to_string()));
        let agy = worker_argv(
            &dir,
            &spec("agy", "claude-opus-5-5-high"),
            &project,
            &project,
            None,
            &mut out,
        )
        .unwrap();
        assert_eq!(settings_of(&agy), None);
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
