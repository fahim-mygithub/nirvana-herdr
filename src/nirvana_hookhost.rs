//! Plan 41 §5.1, P41b: every Nirvana hook command also runs under agy (Antigravity CLI 1.5.9), the seat the
//! orchestrator moves to when Claude's usage runs low.
//!
//! A hook command given a trailing `--host agy` reads agy's hook input and writes agy's hook output; without the flag
//! nothing changes, byte for byte. Each verb's logic is untouched: this module turns agy's input into the Claude Code
//! input the verbs already read (`to_claude`), and the verb's Claude output into agy's (`agy_output`). The agy
//! `hooks.json` that calls `<home>/bin/herdr.exe <the Claude args> --host agy` is P41c's.
//!
//! What the P41a probe measured on 2026-11-14 (payloads in `nirvana_hookhost/fixtures/`, paths shortened):
//!
//! - **Events.** `SessionStart`, `PreInvocation`, `PreToolUse`, `PostToolUse`, `PostInvocation`, `Stop`. No
//!   `UserPromptSubmit` and no `SubagentStop`. `PreInvocation` fires before every model call: 5 times for the first
//!   of two prompts in run 1. `invocationNum` restarts at 0 for each new prompt, and also when a subagent's message
//!   wakes the parent (run 1, twice), so it does not mark a prompt; `lastUserInput` does (`first_for_prompt`).
//! - **Tool names a hook sees** differ from the names in agy's own tool list: the hook saw `read_file`, `edit_file`,
//!   `write_file`, `run_command`, `invoke_subagent`, `send_message`, where `--output-format stream-json`'s `init`
//!   lists `view_file`, `replace_file_content`, `multi_replace_file_content`, `write_to_file`. Asked by name for
//!   `multi_replace_file_content` and `sed_file`, the model made `edit_file` calls; for `list_dir` and `grep_search`,
//!   `run_command` calls (run 6). `claude_tool` maps both sets.
//! - **`continuationActive` stays `false`** after a Stop hook's `continue`, in `-p` (§4.2), in stream-json (run 1)
//!   and in interactive `-i` in a ConPTY (run 4). `executionNum` went 0 to 1 on the continuation in all three, but
//!   agy's docs do not promise it, so the gate counts its own asks (`stop_asks`, `count_ask`).
//! - **A subagent fires the parent's hooks** with its own `conversationId` and an empty `lastUserInput`: its
//!   `SessionStart`, `PreInvocation`, `PreToolUse` and `Stop` all reached the probe (run 1, one subagent).
//! - **`cwd` is not always the session's folder.** With the hooks passed by `--add-dir`, the payload's `cwd` was the
//!   hooks folder and `workspacePaths` was `[hooks folder, session folder]` (run 1); with a workspace `.agents/`
//!   it was the session folder (run 4). The hook process itself runs in the folder holding `hooks.json`
//!   (`<hooks folder>/.agents`). `session_cwd` resolves the session folder from these.
//! - **`PostToolUse` output is ignored**: an `injectSteps` system message from it never reached the model (run 8:
//!   the model answered NONE). So `check-edit`'s block cannot reach agy, and its output is `{}`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The flag a hook command takes, and its one value. P41c's `hooks.json` appends exactly `--host agy`.
pub const HOST_FLAG: &str = "--host";
pub const AGY: &str = "agy";

/// The Stop blocks agy is given per owner prompt. One, as Claude Code allows: Claude sets `stop_hook_active` on the
/// Stop that follows a block, and the gate (`nirvana_claude::stop_output`) then gives a notice, not a block. agy's
/// `continuationActive` never turned true (P41a), and a gate that trusted it asked 10 times in one `-p` turn (§4.2).
pub const AGY_STOP_ASKS_PER_PROMPT: u32 = 1;
/// Prompt and ask records older than this are dropped when the file is next written: a conversation is not resumed
/// after a week by the orchestrator, and the file would otherwise only grow.
const STATE_KEEP_S: u64 = 7 * 24 * 60 * 60;
/// `<home>/nirvana/<file>`, beside the gate's `stop-raised.json`.
pub const ASKS_FILE: &str = "agy-stop-asks.json";
pub const PROMPTS_FILE: &str = "agy-prompts.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    Claude,
    Agy,
}

/// `--host agy` anywhere in `args`. Any other value, or none, is Claude: the hooks.json of the Claude plugin has no
/// such flag, and a typo must not change what Claude's hooks print.
pub fn host_in(args: &[String]) -> Host {
    if args
        .windows(2)
        .any(|pair| pair[0] == HOST_FLAG && pair[1] == AGY)
    {
        Host::Agy
    } else {
        Host::Claude
    }
}

/// This process's host, from its own command line. Read from `std::env::args` so the dispatchers in other modules
/// (`nirvana_jev`'s `compact`, `nirvana_board`'s `notice`) need not pass it along.
pub fn host() -> Host {
    host_in(&std::env::args().collect::<Vec<_>>())
}

/// `args` without `--host agy`, for a verb that reads its own positional words (`run notice [run]`).
pub fn without_host(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == HOST_FLAG {
            iter.next();
            continue;
        }
        out.push(arg.clone());
    }
    out
}

/// The Claude Code event a verb was written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Stop,
}

impl Event {
    pub fn claude_name(self) -> &'static str {
        match self {
            Event::SessionStart => "SessionStart",
            Event::UserPromptSubmit => "UserPromptSubmit",
            Event::PreToolUse => "PreToolUse",
            Event::PostToolUse => "PostToolUse",
            Event::Stop => "Stop",
        }
    }
}

/// agy's `hookEventName` as the Claude event the same hook ran on (plan 41 §5.1's table). `PreInvocation` stands for
/// `UserPromptSubmit`, deduped to once per prompt. `PostInvocation` has no Claude hook here.
pub fn agy_event(name: &str) -> Option<Event> {
    Some(match name {
        "SessionStart" => Event::SessionStart,
        "PreInvocation" => Event::UserPromptSubmit,
        "PreToolUse" => Event::PreToolUse,
        "PostToolUse" => Event::PostToolUse,
        "Stop" => Event::Stop,
        _ => return None,
    })
}

/// agy's tool name as the Claude tool the guards match on. The names a hook saw in P41a first, then the names in
/// agy's tool list a hook may yet see (a later agy, another model): an edit tool unmapped would pass the edit guard
/// unseen, so every file-writing name in the list is here. `browser_subagent` is not an `Agent`: on Claude a browser
/// lookup is not a spawn the guard books.
pub fn claude_tool(agy: &str) -> Option<&'static str> {
    Some(match agy {
        "write_file" | "write_to_file" => "Write",
        "edit_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file" => "Edit",
        // `notebook_edit` is in agy's tool list, `edit_notebook` a step type in its binary (P41c's read).
        "notebook_edit" | "edit_notebook" => "NotebookEdit",
        "read_file" | "view_file" => "Read",
        "invoke_subagent" | "define_subagent" | "run_workflow" => "Agent",
        "run_command" => "Bash",
        _ => return None,
    })
}

/// The Claude tools each tool verb is called for: the matchers of `nirvana/claude/hooks/hooks.json`. agy's
/// `hooks.json` may match more widely (P41c's choice), so a verb answers "allow" for any other tool without running,
/// as it never runs for one on Claude.
pub fn verb_tools(verb: &str) -> Option<&'static [&'static str]> {
    Some(match verb {
        "guard-spawn" => &["Agent"],
        "guard-edit" => &["Edit", "Write", "MultiEdit"],
        "guard-read" => &["Read"],
        "check-edit" => &["Edit", "Write", "MultiEdit", "NotebookEdit", "Bash"],
        _ => return None,
    })
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// The file an agy tool call names. `Path` is what P41a saw on every file tool; the others are the spellings in
/// agy's tool list.
fn file_arg(args: &Value) -> Option<&str> {
    ["Path", "AbsolutePath", "TargetFile", "File"]
        .iter()
        .find_map(|key| text(args, key))
}

/// The new text of an edit: `Replacement` (P41a's `edit_file`), else every other string the call carries, joined.
/// An edit tool whose arguments P41a never saw is counted by all its text: the edit guard is a limit, and counting
/// too many lines refuses an edit a worker should make anyway, where counting none would pass any edit.
fn new_text(args: &Value) -> String {
    if let Some(replacement) = text(args, "Replacement") {
        return replacement.to_owned();
    }
    fn strings(value: &Value, key: Option<&str>, out: &mut Vec<String>) {
        match value {
            Value::String(s)
                if !matches!(
                    key,
                    Some("Path" | "AbsolutePath" | "TargetFile" | "File" | "Target" | "toolSummary")
                ) =>
            {
                out.push(s.clone())
            }
            Value::Array(items) => items.iter().for_each(|v| strings(v, None, out)),
            Value::Object(map) => map.iter().for_each(|(k, v)| strings(v, Some(k), out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    strings(args, None, &mut out);
    out.join("\n")
}

/// Claude's `tool_input` for an agy call already mapped to `tool`.
pub fn tool_input(tool: &str, args: &Value) -> Value {
    let file = file_arg(args);
    match tool {
        "Write" => json!({"file_path": file, "content": text(args, "Content").unwrap_or_default()}),
        "Edit" => json!({
            "file_path": file,
            "old_string": text(args, "Target").unwrap_or_default(),
            "new_string": new_text(args),
        }),
        "NotebookEdit" => json!({"notebook_path": file}),
        "Read" => json!({"file_path": file}),
        // agy names no model or effort on a spawn (P41a: Prompt, Role, TypeName), so the guard refuses it as it
        // refuses a Claude spawn that names neither; the orchestrator seats work with `herdr seat run` instead.
        "Agent" => json!({
            "subagent_type": text(args, "TypeName"),
            "description": text(args, "Role"),
            "prompt": text(args, "Prompt").unwrap_or_default(),
        }),
        "Bash" => json!({"command": text(args, "CommandLine").unwrap_or_default()}),
        _ => args.clone(),
    }
}

/// A subagent's hook: P41a's one subagent had an empty `lastUserInput` on every event, and the parent's was never
/// empty (an agy session fires `SessionStart` at its first prompt, with that prompt). Hypothesis on one subagent.
pub fn is_subagent(raw: &Value) -> bool {
    text(raw, "lastUserInput").is_none_or(|input| input.trim().is_empty())
}

fn same_folder(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    norm(a) == norm(b)
}

/// The folder whose `.agents/hooks.json` agy ran this hook from: the hook process runs in `<folder>/.agents`
/// (P41a run 4), and a plugin's hooks have `PLUGIN_ROOT` (agy's changelog).
pub fn hooks_home() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("PLUGIN_ROOT").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(root));
    }
    let cwd = std::env::current_dir().ok()?;
    (cwd.file_name()? == ".agents").then(|| cwd.parent().map(Path::to_path_buf))?
}

/// The session's folder: the payload's `cwd`, unless that is the hooks folder, as it was with `--add-dir` (P41a run
/// 1), and then the first workspace path that is not. A Nirvana hook finds the project from it.
pub fn session_cwd(raw: &Value, hooks_home: Option<&Path>) -> Option<PathBuf> {
    let cwd = text(raw, "cwd").filter(|c| !c.is_empty());
    let home = hooks_home.map(|h| h.to_string_lossy().into_owned());
    let is_home = |path: &str| home.as_deref().is_some_and(|h| same_folder(path, h));
    let other = || {
        raw.get("workspacePaths")?
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .find(|path| !is_home(path))
            .map(str::to_owned)
    };
    match cwd {
        Some(cwd) if is_home(cwd) => other().or_else(|| Some(cwd.to_owned())),
        Some(cwd) => Some(cwd.to_owned()),
        None => other(),
    }
    .map(PathBuf::from)
}

/// agy's hook input as the Claude Code input the verbs read. `stop_hook_active` is the gate's own count, never
/// `continuationActive`. A subagent's hook carries an `agent_id`, as Claude's does, so every verb that skips a
/// subagent skips it here too.
pub fn to_claude(raw: &Value, cwd: Option<&Path>, stop_hook_active: bool) -> Value {
    let event = text(raw, "hookEventName").and_then(agy_event);
    let mut hook = json!({
        "host": AGY,
        "session_id": text(raw, "conversationId"),
        "transcript_path": text(raw, "transcriptPath"),
        "cwd": cwd.map(|c| c.display().to_string()),
        "hook_event_name": event.map(Event::claude_name),
        "model": text(raw, "modelName"),
    });
    if is_subagent(raw) {
        hook["agent_id"] = json!(format!("agy:{}", text(raw, "conversationId").unwrap_or("")));
    }
    match event {
        // agy names no source; `startup` is what a Claude session that is not resumed or cleared sends.
        Some(Event::SessionStart) => hook["source"] = json!("startup"),
        Some(Event::UserPromptSubmit) => hook["prompt"] = json!(text(raw, "lastUserInput")),
        Some(Event::PreToolUse | Event::PostToolUse) => {
            let call = raw.get("toolCall").unwrap_or(&Value::Null);
            let name = text(call, "name").unwrap_or_default();
            let args = call.get("args").unwrap_or(&Value::Null);
            let tool = claude_tool(name);
            hook["tool_name"] = json!(tool.unwrap_or(name));
            hook["tool_input"] = tool.map_or_else(|| args.clone(), |t| tool_input(t, args));
            hook["tool_use_id"] = json!(text(call, "id"));
        }
        Some(Event::Stop) => hook["stop_hook_active"] = json!(stop_hook_active),
        None => {}
    }
    hook
}

/// The text a context hook (SessionStart, UserPromptSubmit) gave Claude: plain stdout, or the JSON form's
/// `additionalContext` or `systemMessage`.
fn context_text(out: &str) -> String {
    let trimmed = out.trim();
    if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(trimmed) {
        let value = Value::Object(map);
        return value
            .pointer("/hookSpecificOutput/additionalContext")
            .or_else(|| value.get("systemMessage"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned();
    }
    trimmed.to_owned()
}

/// A Claude block (`decision: block`, or a `PreToolUse` `permissionDecision: deny`) and its reason. A verb prints
/// one JSON object per line.
fn block_reason(out: &str) -> Option<String> {
    out.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .find_map(|value| {
            let denied = value.pointer("/hookSpecificOutput/permissionDecision")
                == Some(&json!("deny"))
                || value.get("decision") == Some(&json!("block"));
            denied.then(|| {
                value
                    .pointer("/hookSpecificOutput/permissionDecisionReason")
                    .or_else(|| value.get("reason"))
                    .and_then(Value::as_str)
                    .unwrap_or("herdr: refused")
                    .to_owned()
            })
        })
}

/// agy's answer for a verb's Claude output on `event`, and whether it asks agy to continue (a Stop block, which
/// `count_ask` records). `asks` is how many continues this prompt has had: at `AGY_STOP_ASKS_PER_PROMPT` a block is
/// dropped whatever the verb said, so no verb can loop agy. A Stop's `systemMessage` (the gate's notice to the owner)
/// has no agy form and is dropped: agy shows a Stop hook's output to no one.
pub fn agy_output(event: Option<Event>, out: &str, asks: u32) -> (Value, bool) {
    let empty = (json!({}), false);
    match event {
        Some(Event::SessionStart | Event::UserPromptSubmit) => {
            let text = context_text(out);
            if text.is_empty() {
                empty
            } else {
                (
                    json!({"injectSteps": [{"systemMessage": {"systemMessage": text}}]}),
                    false,
                )
            }
        }
        Some(Event::PreToolUse) => match block_reason(out) {
            Some(reason) => (json!({"decision": "deny", "reason": reason}), false),
            None => (json!({"decision": "allow"}), false),
        },
        Some(Event::Stop) => match block_reason(out) {
            Some(reason) if asks < AGY_STOP_ASKS_PER_PROMPT => {
                (json!({"decision": "continue", "reason": reason}), true)
            }
            _ => empty,
        },
        // P41a run 8: agy ignores a PostToolUse answer.
        Some(Event::PostToolUse) | None => empty,
    }
}

// ---- the state: one prompt's notices, one prompt's asks ----

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    prompt: String,
    #[serde(default)]
    asks: u32,
    at: u64,
}

/// A short, stable name for a conversation's prompt: the state files hold no prompt text.
pub fn prompt_key(raw: &Value) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(text(raw, "lastUserInput").unwrap_or_default().as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn conversation(raw: &Value) -> String {
    text(raw, "conversationId").unwrap_or_default().to_owned()
}

fn load(path: &Path) -> BTreeMap<String, Mark> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(path: &Path, mut marks: BTreeMap<String, Mark>, now: u64) {
    marks.retain(|_, mark| now.saturating_sub(mark.at) < STATE_KEEP_S);
    if let Ok(text) = serde_json::to_string(&marks) {
        let _ = crate::nirvana_seat::write_atomic(path, text.as_bytes());
    }
}

/// True the first time `verb` sees this prompt of this conversation, and records it. agy fires `PreInvocation`
/// before every model call (P41a: 5 for one prompt), where Claude fires `UserPromptSubmit` once; a notice repeated
/// per call would cost a turn's context each time, and `prompt-baseline` would reset the edit budget mid-turn.
/// Each verb keeps its own mark, since each is its own hook process. The one thing missed: the owner typing the same
/// prompt twice in one conversation gets no second notice.
pub fn first_for_prompt(state_dir: &Path, verb: &str, raw: &Value, now: u64) -> bool {
    let path = state_dir.join(PROMPTS_FILE);
    let mut marks = load(&path);
    let key = format!("{verb}|{}", conversation(raw));
    let prompt = prompt_key(raw);
    if marks.get(&key).is_some_and(|mark| mark.prompt == prompt) {
        return false;
    }
    marks.insert(
        key,
        Mark {
            prompt,
            asks: 0,
            at: now,
        },
    );
    save(&path, marks, now);
    true
}

/// The continues agy has been asked for in this prompt of this conversation. A new prompt starts at 0, as Claude's
/// `stop_hook_active` is false on the first Stop after an owner prompt.
pub fn stop_asks(state_dir: &Path, raw: &Value) -> u32 {
    load(&state_dir.join(ASKS_FILE))
        .get(&conversation(raw))
        .filter(|mark| mark.prompt == prompt_key(raw))
        .map_or(0, |mark| mark.asks)
}

/// Records one more continue for this prompt.
pub fn count_ask(state_dir: &Path, raw: &Value, now: u64) {
    let path = state_dir.join(ASKS_FILE);
    let mut marks = load(&path);
    let prompt = prompt_key(raw);
    let mark = marks.entry(conversation(raw)).or_default();
    if mark.prompt != prompt {
        *mark = Mark::default();
        mark.prompt = prompt;
    }
    mark.asks += 1;
    mark.at = now;
    save(&path, marks, now);
}

// ---- one hook, both hosts ----

/// One hook's input, read for `verb`. `hook` is Claude Code's shape whatever the host. `skip` says the verb must not
/// run: a hook Claude would not have fired (a subagent's start, prompt or stop; a repeated `PreInvocation`; a tool
/// outside the verb's matcher).
pub struct HookIn {
    pub host: Host,
    pub event: Option<Event>,
    pub hook: Value,
    pub skip: bool,
    raw: Value,
    asks: u32,
    state_dir: PathBuf,
}

fn state_dir() -> PathBuf {
    crate::nirvana::home()
        .unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
        .join(crate::nirvana_seat::DIR)
}

/// `read_for` on this process's stdin and host. On agy it also moves the process into the session's folder, where
/// Claude Code runs its hooks: `nirvana_run`'s `Project::from_env` and the verbs' `current_dir` fallbacks find the
/// project from there, and agy runs the hook in `<hooks folder>/.agents` (P41a).
pub fn read_stdin_for(verb: &str) -> HookIn {
    use std::io::Read;
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let hin = read_for(
        host(),
        verb,
        &input,
        hooks_home().as_deref(),
        &state_dir(),
        crate::nirvana_run::now(),
    );
    if hin.host == Host::Agy {
        if let Some(cwd) = hin
            .hook
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|c| Path::new(c).is_dir())
        {
            let _ = std::env::set_current_dir(cwd);
        }
    }
    hin
}

/// One hook's input from `input`. On Claude `hook` is the input as it stands (or `null`, as the verbs read a bad
/// one) and nothing is skipped. Pure but for the agy state files under `state_dir`.
pub fn read_for(
    host: Host,
    verb: &str,
    input: &str,
    hooks_home: Option<&Path>,
    state_dir: &Path,
    now: u64,
) -> HookIn {
    let parsed: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    if host == Host::Claude {
        return HookIn {
            host,
            event: None,
            hook: parsed,
            skip: false,
            raw: Value::Null,
            asks: 0,
            state_dir: state_dir.to_path_buf(),
        };
    }
    let raw = parsed;
    let event = text(&raw, "hookEventName").and_then(agy_event);
    let subagent = is_subagent(&raw);
    let asks = if event == Some(Event::Stop) {
        stop_asks(state_dir, &raw)
    } else {
        0
    };
    let cwd = session_cwd(&raw, hooks_home);
    let hook = to_claude(&raw, cwd.as_deref(), asks > 0);
    let skip = match event {
        // Claude fires no SessionStart, UserPromptSubmit or Stop for a subagent (it fires SubagentStop, which has
        // no agy event, plan 41 §5.1).
        Some(Event::SessionStart | Event::Stop) => subagent,
        Some(Event::UserPromptSubmit) => subagent || !first_for_prompt(state_dir, verb, &raw, now),
        Some(Event::PreToolUse | Event::PostToolUse) => verb_tools(verb).is_some_and(|tools| {
            !hook
                .get("tool_name")
                .and_then(Value::as_str)
                .is_some_and(|tool| tools.contains(&tool))
        }),
        None => true,
    };
    HookIn {
        host,
        event,
        hook,
        skip,
        raw,
        asks,
        state_dir: state_dir.to_path_buf(),
    }
}

impl HookIn {
    /// What to print for the verb's Claude output `out`: `out` itself on Claude, agy's JSON on agy. A Stop's continue
    /// is counted here, so the next Stop of the prompt reads `stop_hook_active`.
    pub fn answer_text(&self, out: &str, now: u64) -> String {
        if self.host == Host::Claude {
            return out.to_owned();
        }
        let out = if self.skip { "" } else { out };
        let (value, continues) = agy_output(self.event, out, self.asks);
        if continues {
            count_ask(&self.state_dir, &self.raw, now);
        }
        format!("{value}\n")
    }

    /// Prints `answer_text`. Every verb's last word.
    pub fn answer(&self, out: &str) {
        use std::io::Write;
        let text = self.answer_text(out, crate::nirvana_run::now());
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }

    pub fn is_agy(&self) -> bool {
        self.host == Host::Agy
    }
}

#[cfg(test)]
mod tests;
