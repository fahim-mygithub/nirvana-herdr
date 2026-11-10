//! Nirvana fork, alpha 0.69.0: a Claude whose turn has ended and that waits on its background work says so in words,
//! not with a working spinner. The owner, 2026-11-10: "One of the agent is working on some sort background testing
//! that needs to deliver then alerts the agent I'd like ui to tell me that instead of [spinning] one. Ensure we are
//! testing accurately for this feature I can foresee an issue where we easily show that a bg task is working but it's
//! actually idling or ghosting".
//!
//! **What is open.** Only the Claude transcript says which background tasks a session started and which ended; the
//! screen does not (all five of the owner's panes read `← 1 agent` on 2026-11-10, one of them idle since Nov 1). A
//! task is open from its launch's result to its end, read from these shapes (Claude Code 2.1.322, measured on the
//! owner's transcripts 2026-11-10):
//! - launch: an assistant `tool_use` of Bash or PowerShell with `run_in_background`, of Monitor, or of Agent; its
//!   `tool_result` names the task: `toolUseResult.backgroundTaskId` and "Command running in background with ID: <id>.
//!   Output is being written to: <path>.output", `toolUseResult.taskId` and "Monitor started (task <id>, ...", or
//!   `toolUseResult.status == "async_launched"` with `agentId`. A result that is an error, or an Agent that ran in the
//!   foreground or spawned a teammate, opens nothing.
//! - end: a `<task-notification>` with that `<task-id>` and a `<status>` (completed, failed, killed), in a
//!   `queue-operation` enqueue, a `queued_command` attachment or a user message; or a TaskStop (KillShell) whose result
//!   is not an error. A Monitor's event is a notification with no `<status>`: it keeps the task open and counts as
//!   output.
//!
//! **Open is never enough to say running.** Each open task is checked against evidence, and only once the transcript
//! says the turn ended (an assistant `end_turn`, a `turn_duration`, or an interrupt): while the turn runs herdr's own
//! state is right and nothing here is read past the transcript.
//! - lost: the Claude process started after the launch (a restart; its tasks died with the old one, and their
//!   notifications never come), the task's output file is gone, or no process under this Claude is the task's. A shell
//!   or Monitor task is a descendant of the pane's `claude.exe` whose command line names its output file (the
//!   PowerShell tool's `cmd.exe` redirect) or carries its command (Git Bash's `bash.exe -c "... eval '<command>'"`),
//!   measured 2026-11-10. A background Agent runs inside Claude, so its evidence is its own transcript.
//! - quiet: alive, with no output for `QUIET_AFTER_S`.
//! - running: alive, with output (or its start) within `QUIET_AFTER_S`.
//!
//! **Where it shows.** The herdr server reads it off its loop (`cached`, one worker thread, as `nirvana_spaces`'s
//! model) and hands it on as the agent token `TOKEN`: in the window's snapshot (`server/client_shell.rs`) and in
//! `agent.list` (`app/agents.rs`), which the phone reads. The sidebar row, the agents panel and the phone draw it
//! (`shown_status`, `View::sentence`); herdr's own agent state is not changed, so `agent wait`, alerts' sources and
//! everything else upstream still see what herdr detected.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::schema::AgentStatus;

#[cfg(test)]
mod tests;

/// The agent token that carries a pane's reading, as `View`'s JSON.
pub const TOKEN: &str = "nirvana_bgwait";
/// How old a reading may get before the worker reads again. The model's own refresh (`nirvana_spaces::REFRESH`).
pub const REFRESH: Duration = Duration::from_secs(3);
/// An entry nobody asked for in this long belongs to a pane that closed.
const FORGET: Duration = Duration::from_secs(60);
/// A live task with no output for this long reads quiet, not running. A guess, not a measurement: long enough that a
/// test suite printing a line per file stays running, short enough that a hung one shows within a few minutes.
pub const QUIET_AFTER_S: u64 = 180;
/// A lost task stops being shown this long after its launch. A guess: a ghost is news for a while, then noise, and
/// a transcript keeps a launch with no end for good (the owner's Nov 1 pane).
pub const LOST_HIDE_AFTER_S: u64 = 3600;
/// A Claude process that started more than this after a launch is a restart, not the process that launched it. The
/// launch's `timestamp` and the process's creation time come from different clocks of the same PC; a guess at slack.
pub const RESTART_SLACK_S: u64 = 5;
/// The first read of a transcript takes at most its last this many bytes; later reads take only what was added. The
/// owner's longest live transcript was 162 MiB on 2026-11-10. A launch older than this window is not seen, which shows
/// nothing rather than something false.
pub const FIRST_READ_MOST: u64 = 16 * 1024 * 1024;
/// A label is cut here: the sidebar has 19 cells, the phone a line.
const LABEL_MOST: usize = 60;
/// The token keeps this many tasks; the rest are counted.
const TASKS_KEPT: usize = 4;
/// Launches waiting for their result, and open tasks, are capped so a transcript cannot grow the ledger for good.
const PENDING_MOST: usize = 256;
const OPEN_MOST: usize = 64;
/// A command is matched to a process by this many of its first characters, quotes and backslashes taken out.
const COMMAND_MATCH: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Shell,
    Monitor,
    Agent,
}

/// A background task the transcript started and has not ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenTask {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    /// The shell command it runs, for matching it to its process.
    pub command: Option<String>,
    /// Its output file, from the launch's result.
    pub output: Option<PathBuf>,
    pub launched_at: u64,
    /// A Monitor's last event.
    pub last_event_at: Option<u64>,
}

#[derive(Debug, Clone)]
struct Launch {
    kind: Kind,
    label: String,
    command: Option<String>,
}

/// What a transcript says so far: the open tasks and whether the turn ended. Fed line by line, in file order.
#[derive(Debug, Default)]
pub struct Ledger {
    launches: HashMap<String, Launch>,
    stops: HashMap<String, String>,
    open: Vec<OpenTask>,
    pub turn_ended: bool,
}

fn truthy(value: &Value) -> bool {
    // The Agent tool has been seen with `"run_in_background": "true"`, a string (lead transcript, 2026-11-10).
    value.as_bool() == Some(true) || value.as_str() == Some("true")
}

fn cut(text: &str, most: usize) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if line.chars().count() <= most {
        return line.to_string();
    }
    let mut out: String = line.chars().take(most.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

fn label_of(input: &Value) -> String {
    let text = input["description"]
        .as_str()
        .filter(|d| !d.trim().is_empty())
        .or_else(|| input["command"].as_str())
        .or_else(|| input["prompt"].as_str())
        .unwrap_or("background task");
    cut(text, LABEL_MOST)
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// The word after `marker` in `text`: letters, digits, `-` and `_`.
fn word_after(text: &str, marker: &str) -> Option<String> {
    let rest = &text[text.find(marker)? + marker.len()..];
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!word.is_empty()).then_some(word)
}

/// What is between `<tag>` and `</tag>`.
fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&close)?;
    Some(text[start..end].trim())
}

impl Ledger {
    /// The open tasks, oldest launch first.
    pub fn open(&self) -> &[OpenTask] {
        &self.open
    }

    /// One transcript line.
    pub fn feed(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            return;
        };
        // A subagent's own lines live in its own file; one copied here is not this session's turn.
        if entry["isSidechain"] == true {
            return;
        }
        let at = entry["timestamp"]
            .as_str()
            .and_then(crate::nirvana_usage::rfc3339);
        match entry["type"].as_str() {
            Some("assistant") => self.assistant(&entry),
            Some("user") => self.user(&entry, at),
            Some("queue-operation") if entry["operation"] == "enqueue" => {
                if let Some(content) = entry["content"].as_str() {
                    self.notifications(content, at);
                }
            }
            Some("attachment") if entry["attachment"]["type"] == "queued_command" => {
                if let Some(prompt) = entry["attachment"]["prompt"].as_str() {
                    self.notifications(prompt, at);
                }
            }
            Some("system") if entry["subtype"] == "turn_duration" => self.turn_ended = true,
            _ => {}
        }
    }

    fn assistant(&mut self, entry: &Value) {
        let message = &entry["message"];
        for block in message["content"].as_array().into_iter().flatten() {
            if block["type"] != "tool_use" {
                continue;
            }
            let Some(id) = block["id"].as_str() else {
                continue;
            };
            let input = &block["input"];
            let launch = |kind| Launch {
                kind,
                label: label_of(input),
                command: input["command"].as_str().map(str::to_owned),
            };
            match block["name"].as_str().unwrap_or("") {
                "Bash" | "PowerShell" if truthy(&input["run_in_background"]) => {
                    self.launch(id, launch(Kind::Shell));
                }
                "Monitor" => self.launch(id, launch(Kind::Monitor)),
                // Whether it runs in the background is in its result: `async_launched`.
                "Agent" | "Task" => self.launch(id, launch(Kind::Agent)),
                "TaskStop" | "KillShell" | "KillBash" => {
                    if let Some(task) = input["task_id"]
                        .as_str()
                        .or_else(|| input["shell_id"].as_str())
                    {
                        if self.stops.len() >= PENDING_MOST {
                            self.stops.clear();
                        }
                        self.stops.insert(id.to_owned(), task.to_owned());
                    }
                }
                _ => {}
            }
        }
        // `tool_use` (or none, mid-stream) means the turn goes on; `end_turn`, `max_tokens`, `stop_sequence` end it.
        self.turn_ended = message["stop_reason"]
            .as_str()
            .is_some_and(|reason| reason != "tool_use");
    }

    fn launch(&mut self, id: &str, launch: Launch) {
        if self.launches.len() >= PENDING_MOST {
            self.launches.clear();
        }
        self.launches.insert(id.to_owned(), launch);
    }

    fn user(&mut self, entry: &Value, at: Option<u64>) {
        match &entry["message"]["content"] {
            Value::String(text) => self.user_text(entry, text, at),
            Value::Array(blocks) => {
                for block in blocks {
                    match block["type"].as_str() {
                        Some("tool_result") => self.result(entry, block, at),
                        Some("text") => {
                            self.user_text(entry, block["text"].as_str().unwrap_or(""), at)
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn user_text(&mut self, entry: &Value, text: &str, at: Option<u64>) {
        let start = text.trim_start();
        if text.contains("<task-notification>") {
            self.notifications(text, at);
            self.turn_ended = false;
        } else if start.starts_with("[Request interrupted") {
            self.turn_ended = true;
        } else if entry["isMeta"] == true
            || start.starts_with("<command-name>")
            || start.starts_with("<local-command")
            || start.starts_with("<command-message>")
        {
            // Claude Code's own notes and a slash command's echo start no turn.
        } else if !start.is_empty() {
            self.turn_ended = false;
        }
    }

    fn result(&mut self, entry: &Value, block: &Value, at: Option<u64>) {
        self.turn_ended = false;
        let Some(id) = block["tool_use_id"].as_str() else {
            return;
        };
        let failed = block["is_error"] == true;
        if let Some(task) = self.stops.remove(id) {
            if !failed {
                self.close(&task);
            }
        }
        let Some(launch) = self.launches.remove(id) else {
            return;
        };
        if failed {
            return;
        }
        let text = text_of(&block["content"]);
        let done = &entry["toolUseResult"];
        let task = match launch.kind {
            Kind::Shell => done["backgroundTaskId"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| word_after(&text, "running in background with ID: ")),
            Kind::Monitor => done["taskId"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| word_after(&text, "Monitor started (task ")),
            Kind::Agent => {
                let background = done["status"] == "async_launched"
                    || done["isAsync"] == true
                    || text.contains("Async agent launched");
                if !background {
                    return;
                }
                done["agentId"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| word_after(&text, "agentId: "))
            }
        };
        let Some(task) = task else {
            return;
        };
        let output = text
            .find("Output is being written to: ")
            .map(|at| &text[at + "Output is being written to: ".len()..])
            .and_then(|rest| {
                rest.find(".output")
                    .map(|end| &rest[..end + ".output".len()])
            })
            .map(|path| PathBuf::from(path.trim()));
        self.open.retain(|open| open.id != task);
        if self.open.len() >= OPEN_MOST {
            self.open.remove(0);
        }
        self.open.push(OpenTask {
            id: task,
            kind: launch.kind,
            label: launch.label,
            command: launch.command,
            output,
            launched_at: at.unwrap_or(0),
            last_event_at: None,
        });
    }

    fn notifications(&mut self, text: &str, at: Option<u64>) {
        for part in text.split("<task-notification>").skip(1) {
            let part = part.split("</task-notification>").next().unwrap_or(part);
            let Some(task) = tag(part, "task-id") else {
                continue;
            };
            if tag(part, "status").is_some() {
                self.close(task);
            } else if let Some(open) = self.open.iter_mut().find(|open| open.id == task) {
                open.last_event_at = at.or(open.last_event_at);
            }
        }
    }

    fn close(&mut self, task: &str) {
        self.open.retain(|open| open.id != task);
    }
}

// -------------------------------------------------------------------------------------------------------------------
// Evidence and the reading
// -------------------------------------------------------------------------------------------------------------------

/// One process under the pane's Claude.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proc {
    pub pid: u32,
    pub command_line: String,
}

/// A file's length and change time (Unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub len: u64,
    pub modified: u64,
}

/// What a task is checked against.
pub struct Evidence<'a> {
    pub now: u64,
    /// When the pane's Claude process started.
    pub claude_started: Option<u64>,
    /// The processes under the pane's Claude; `None` when they cannot be listed here (not Windows), and then no task
    /// is lost for want of one, but none is running without output either.
    pub processes: Option<&'a [Proc]>,
    /// `<transcript without .jsonl>`: a background Agent's transcript is `subagents/agent-<id>.jsonl` under it.
    pub session_dir: Option<&'a Path>,
    pub stat: &'a dyn Fn(&Path) -> Option<Stat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Running,
    Quiet,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    NoProcess,
    ClaudeRestarted,
    OutputGone,
}

impl Why {
    pub fn words(self) -> &'static str {
        match self {
            Self::NoProcess => "no process",
            Self::ClaudeRestarted => "Claude restarted",
            Self::OutputGone => "output file gone",
        }
    }
}

/// One task's reading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub started: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_output: Option<u64>,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<Why>,
}

/// `text` with quotes and backslashes taken out and whitespace runs made one space: the same command reads the same in
/// the tool's input and in a shell's command line, whatever quoting the launch added.
pub fn squash(text: &str) -> String {
    let kept: String = text
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | '\\'))
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether `command_line` is `task`'s process: it names the task's output file, or carries its command.
pub fn is_task_process(task: &OpenTask, command_line: &str) -> bool {
    if command_line.contains(&format!("{}.output", task.id)) {
        return true;
    }
    let Some(command) = task.command.as_deref().map(squash) else {
        return false;
    };
    let head: String = command.chars().take(COMMAND_MATCH).collect();
    head.chars().count() >= 4 && squash(command_line).contains(&head)
}

/// One open task against the evidence.
pub fn classify(task: &OpenTask, evidence: &Evidence) -> Reading {
    let reading = |state, last_output, why| Reading {
        id: task.id.clone(),
        kind: task.kind,
        label: task.label.clone(),
        started: task.launched_at,
        last_output,
        state,
        why,
    };
    if evidence
        .claude_started
        .is_some_and(|started| started > task.launched_at.saturating_add(RESTART_SLACK_S))
    {
        return reading(State::Lost, None, Some(Why::ClaudeRestarted));
    }
    let mut last_output = task.last_event_at;
    let wrote = |stat: Stat| (stat.len > 0).then_some(stat.modified);
    let active_at = match task.kind {
        Kind::Shell | Kind::Monitor => {
            if let Some(path) = &task.output {
                match (evidence.stat)(path) {
                    None => return reading(State::Lost, None, Some(Why::OutputGone)),
                    Some(stat) => last_output = last_output.max(wrote(stat)),
                }
            }
            match evidence.processes {
                Some(processes) => {
                    if !processes
                        .iter()
                        .any(|process| is_task_process(task, &process.command_line))
                    {
                        return reading(State::Lost, last_output, Some(Why::NoProcess));
                    }
                    Some(last_output.unwrap_or(0).max(task.launched_at))
                }
                // No process evidence: only output it wrote counts as a sign of life.
                None => last_output,
            }
        }
        Kind::Agent => {
            if let Some(dir) = evidence.session_dir {
                let path = dir
                    .join("subagents")
                    .join(format!("agent-{}.jsonl", task.id));
                last_output = last_output.max((evidence.stat)(&path).and_then(wrote));
            }
            Some(last_output.unwrap_or(0).max(task.launched_at))
        }
    };
    let state = if active_at.is_some_and(|at| evidence.now.saturating_sub(at) <= QUIET_AFTER_S) {
        State::Running
    } else {
        State::Quiet
    };
    reading(state, last_output, None)
}

/// A pane's reading: its open tasks, checked, the first `TASKS_KEPT` of them kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    pub tasks: Vec<Reading>,
    /// Tasks running or quiet, all of them.
    pub live: usize,
    /// Tasks lost, all of them.
    pub lost: usize,
}

/// How the UI words a pane's reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    Waiting,
    Quiet,
    Lost,
}

impl Shown {
    /// The sidebar's state word, full and short (`nirvana_spaces::status_word` and `short_status_word`).
    pub fn words(self) -> (&'static str, &'static str) {
        match self {
            Self::Waiting => ("bg wait", "bg"),
            Self::Quiet => ("bg quiet", "bg"),
            Self::Lost => ("bg lost", "lost"),
        }
    }

    /// The phone's word for it (`nirvana_remote::spaces::agent_row`).
    pub fn phone(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Quiet => "quiet",
            Self::Lost => "lost",
        }
    }
}

/// The reading of a pane whose transcript is `ledger`, or `None` when its turn has not ended or nothing is open.
pub fn view(ledger: &Ledger, evidence: &Evidence) -> Option<View> {
    if !ledger.turn_ended || ledger.open.is_empty() {
        return None;
    }
    let mut tasks: Vec<Reading> = ledger
        .open
        .iter()
        .map(|task| classify(task, evidence))
        .filter(|reading| {
            reading.state != State::Lost
                || evidence.now.saturating_sub(reading.started) <= LOST_HIDE_AFTER_S
        })
        .collect();
    if tasks.is_empty() {
        return None;
    }
    let rank = |state: State| match state {
        State::Running => 0,
        State::Quiet => 1,
        State::Lost => 2,
    };
    tasks.sort_by(|a, b| {
        rank(a.state).cmp(&rank(b.state)).then_with(|| {
            let at = |r: &Reading| r.last_output.unwrap_or(0).max(r.started);
            at(b).cmp(&at(a))
        })
    });
    let lost = tasks.iter().filter(|r| r.state == State::Lost).count();
    let live = tasks.len() - lost;
    tasks.truncate(TASKS_KEPT);
    Some(View { tasks, live, lost })
}

/// "20s", "4m", "2h", "3d".
pub fn ago(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86_399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

fn plural(count: usize) -> &'static str {
    if count == 1 {
        "task"
    } else {
        "tasks"
    }
}

impl View {
    pub fn from_token(text: &str) -> Option<Self> {
        serde_json::from_str::<Self>(text)
            .ok()
            .filter(|view| !view.tasks.is_empty())
    }

    pub fn token(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn shown(&self) -> Shown {
        if self.tasks.iter().any(|r| r.state == State::Running) {
            Shown::Waiting
        } else if self.live > 0 {
            Shown::Quiet
        } else {
            Shown::Lost
        }
    }

    /// The reading in words at `now`, e.g. "waiting on 1 background task: sleep 90, started 4m ago, last output 20s
    /// ago". Plain words, no brackets: the owner's rule for user-facing text.
    pub fn sentence(&self, now: u64) -> String {
        let since = |at: u64| ago(now.saturating_sub(at));
        let Some(lead) = self.tasks.first() else {
            return String::new();
        };
        let total = self.live + self.lost;
        if self.live == 0 {
            let more = if total > 1 {
                format!(" and {} more", total - 1)
            } else {
                String::new()
            };
            let why = lead.why.map_or("no process", Why::words);
            return format!(
                "{total} background {} lost: {}{more}, {why}",
                plural(total),
                lead.label
            );
        }
        let more = if self.live > 1 {
            format!(" and {} more", self.live - 1)
        } else {
            String::new()
        };
        let last = match (lead.state, lead.last_output) {
            (State::Running, Some(at)) => format!("last output {} ago", since(at)),
            (State::Running, None) => "no output yet".to_string(),
            (_, at) => format!("quiet for {}", since(at.unwrap_or(lead.started))),
        };
        let mut line = format!(
            "waiting on {} background {}: {}{more}, started {} ago, {last}",
            self.live,
            plural(self.live),
            lead.label,
            since(lead.started)
        );
        if self.lost > 0 {
            line.push_str(&format!(". {} lost", self.lost));
        }
        line
    }
}

/// The reading in an agent's tokens.
pub fn from_tokens<'a>(mut tokens: impl Iterator<Item = (&'a str, &'a str)>) -> Option<View> {
    tokens
        .find(|(name, _)| *name == TOKEN)
        .and_then(|(_, value)| View::from_token(value))
}

/// The state the UI shows for an agent herdr reads as `status`. A reading means the transcript says the turn ended, so
/// a Working from the screen (`background_agents_working`, `background_mcp_task_working` in
/// `detect/manifests/claude.toml`, or anything else) is not a turn at work: it reads Idle. Blocked is the owner's to
/// answer and stays.
pub fn shown_status(status: AgentStatus, view: Option<&View>) -> AgentStatus {
    match (status, view) {
        (AgentStatus::Working, Some(_)) => AgentStatus::Idle,
        _ => status,
    }
}

/// Unix seconds now.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// -------------------------------------------------------------------------------------------------------------------
// Reading a transcript as it grows
// -------------------------------------------------------------------------------------------------------------------

/// One transcript read so far: where the next read starts, and what it said.
#[derive(Debug, Default)]
pub struct Tail {
    path: PathBuf,
    offset: u64,
    pub ledger: Ledger,
}

impl Tail {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            ..Self::default()
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads what was added since the last read: whole lines only, so a line being written is read next time. A file
    /// that shrank, or grew by more than `FIRST_READ_MOST`, is read again from its last `FIRST_READ_MOST`.
    pub fn catch_up(&mut self) {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(&self.path) else {
            return;
        };
        let Ok(len) = file.metadata().map(|m| m.len()) else {
            return;
        };
        if len < self.offset || len - self.offset > FIRST_READ_MOST {
            self.offset = 0;
            self.ledger = Ledger::default();
        }
        let mut start = self.offset;
        let mut skip_partial = false;
        if start == 0 && len > FIRST_READ_MOST {
            start = len - FIRST_READ_MOST;
            skip_partial = true;
        }
        if start >= len || file.seek(SeekFrom::Start(start)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.take(len - start).read_to_end(&mut bytes).is_err() {
            return;
        }
        let Some(last_newline) = bytes.iter().rposition(|b| *b == b'\n') else {
            return;
        };
        let whole = &bytes[..=last_newline];
        self.offset = start + whole.len() as u64;
        let text = String::from_utf8_lossy(whole);
        let text = if skip_partial {
            text.split_once('\n').map_or("", |(_, rest)| rest)
        } else {
            &text
        };
        for line in text.lines() {
            self.ledger.feed(line);
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------
// The server's cache: read off its loop, as `nirvana_spaces::cached_model`
// -------------------------------------------------------------------------------------------------------------------

/// The processes under `claude_pid`, with their command lines; `None` off Windows, where they are not listed.
fn processes_under(claude_pid: u32) -> Option<Vec<Proc>> {
    if !cfg!(windows) {
        return None;
    }
    Some(
        crate::nirvana_subagents::find_child_pids(claude_pid)
            .into_iter()
            .filter_map(|pid| {
                Some(Proc {
                    pid,
                    command_line: crate::platform::process_command_line(pid)?,
                })
            })
            .collect(),
    )
}

fn stat(path: &Path) -> Option<Stat> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Some(Stat {
        len: meta.len(),
        modified,
    })
}

/// The reading of the Claude under this pane shell, as a token value. Runs on the worker thread only.
fn probe(
    shell_pid: u32,
    cwd: Option<&str>,
    tails: &mut HashMap<u32, (Tail, Instant)>,
) -> Option<String> {
    let job = crate::detect::foreground_job(shell_pid)?;
    let (agent, _, pid) = crate::detect::identify_agent_process_in_job(&job)?;
    if agent != crate::detect::Agent::Claude {
        return None;
    }
    let config = crate::nirvana::claude_config_dir()?;
    let path = crate::nirvana_spaces::claude_transcript(&config, pid, cwd)?;
    let (tail, used) = tails
        .entry(shell_pid)
        .or_insert_with(|| (Tail::new(path.clone()), Instant::now()));
    *used = Instant::now();
    if tail.path() != path {
        *tail = Tail::new(path.clone());
    }
    tail.catch_up();
    if !tail.ledger.turn_ended || tail.ledger.open().is_empty() {
        return None;
    }
    let processes = processes_under(pid);
    let session_dir = path.with_extension("");
    let evidence = Evidence {
        now: now_secs(),
        claude_started: crate::nirvana_subagents::process_creation_secs(pid),
        processes: processes.as_deref(),
        session_dir: Some(&session_dir),
        stat: &stat,
    };
    view(&tail.ledger, &evidence).map(|view| view.token())
}

struct Entry {
    token: Option<String>,
    /// herdr's state for the pane when the read that made `token` was asked for.
    read_for: Option<AgentStatus>,
    checked: Option<Instant>,
    pending: bool,
    asked: Instant,
}

struct Request {
    shell_pid: u32,
    cwd: Option<String>,
    status: AgentStatus,
    waker: crate::nirvana_spaces::Waker,
}

/// Whether a reading made while herdr read the pane as `read_for` may be shown now that it reads `now`. The live
/// probe of 2026-11-10 (12:36:19) caught the cache handing out a "running" reading for 8 s after the task's
/// notification had started a new turn and herdr read working: the reading was older than the turn. A reading
/// made under another state is not shown; the read it waits for is asked at once.
pub fn usable(read_for: Option<AgentStatus>, now: AgentStatus) -> bool {
    read_for == Some(now)
}

fn cache() -> &'static Mutex<HashMap<u32, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<u32, Entry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn worker() -> Option<&'static mpsc::Sender<Request>> {
    static WORKER: OnceLock<Option<mpsc::Sender<Request>>> = OnceLock::new();
    WORKER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<Request>();
            crate::thread_spawn::spawn_named("nirvana-bgwait", move || {
                let mut tails: HashMap<u32, (Tail, Instant)> = HashMap::new();
                for request in receiver {
                    tails.retain(|_, (_, used)| used.elapsed() < FORGET);
                    let token = probe(request.shell_pid, request.cwd.as_deref(), &mut tails);
                    let changed = {
                        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
                        let Some(entry) = cache.get_mut(&request.shell_pid) else {
                            continue;
                        };
                        let changed =
                            entry.token != token || entry.read_for != Some(request.status);
                        entry.token = token;
                        entry.read_for = Some(request.status);
                        entry.checked = Some(Instant::now());
                        entry.pending = false;
                        changed
                    };
                    if changed {
                        request.waker.dirty.request_generic();
                        request.waker.notify.notify_one();
                    }
                }
            })
            .ok()
            .map(|_| sender)
        })
        .as_ref()
}

/// The reading last made for the Claude under this pane shell, as a token value, and a fresh read queued when it is
/// older than `REFRESH` or was made while herdr read the pane in another state than `status` (`usable`). Never
/// blocks on a file or a process: the server's loop calls this while it builds each client's snapshot and each
/// `agent.list` answer.
pub fn cached(
    shell_pid: u32,
    cwd: Option<&str>,
    status: AgentStatus,
    waker: &crate::nirvana_spaces::Waker,
) -> Option<String> {
    let now = Instant::now();
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, entry| now.duration_since(entry.asked) < FORGET);
    let entry = cache.entry(shell_pid).or_insert(Entry {
        token: None,
        read_for: None,
        checked: None,
        pending: false,
        asked: now,
    });
    entry.asked = now;
    let fresh_for_status = usable(entry.read_for, status);
    let stale = !fresh_for_status
        || entry
            .checked
            .is_none_or(|checked| now.duration_since(checked) >= REFRESH);
    if stale && !entry.pending {
        if let Some(worker) = worker() {
            entry.pending = worker
                .send(Request {
                    shell_pid,
                    cwd: cwd.map(str::to_owned),
                    status,
                    waker: waker.clone(),
                })
                .is_ok();
        }
    }
    if fresh_for_status {
        entry.token.clone()
    } else {
        None
    }
}
