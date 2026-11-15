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
//!   wakes the parent (run 1, twice); `initialNumSteps` only grows. A turn is named by both (`turn_start`, and
//!   the state section's comment), so the owner typing the same prompt twice is two turns (`first_for_turn`).
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

/// The Stop blocks agy is given per turn (`turn_key`). One, as Claude Code allows: Claude sets `stop_hook_active` on the
/// Stop that follows a block, and the gate (`nirvana_claude::stop_output`) then gives a notice, not a block. agy's
/// `continuationActive` never turned true (P41a), and a gate that trusted it asked 10 times in one `-p` turn (§4.2).
pub const AGY_STOP_ASKS_PER_PROMPT: u32 = 1;
/// Prompt and ask records older than this are dropped when the file is next written: a conversation is not resumed
/// after a week by the orchestrator, and the file would otherwise only grow.
const STATE_KEEP_S: u64 = 7 * 24 * 60 * 60;
/// `<home>/nirvana/<file>`, beside the gate's `stop-raised.json`.
pub const ASKS_FILE: &str = "agy-stop-asks.json";
pub const PROMPTS_FILE: &str = "agy-prompts.json";
pub const CONVERSATIONS_FILE: &str = "agy-conversations.json";
/// Held while a hook reads and writes the three files above (`lock_state`).
pub const LOCK_FILE: &str = "agy-state.lock";

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
///
/// A table, not a `match`, so a test can hold it against `nirvana/agy_tool_names.json`, the file `agy_plugin.py`'s
/// hook matchers are tested against too: the Sonnet review of P41b/P41c found the plugin's matchers missing
/// `sed_file`, `notebook_edit`, `define_subagent` and `run_workflow`, so agy would never have called the guard for a
/// tool this table maps (`the_tool_table_is_the_shared_fixture`).
pub const AGY_TOOLS: &[(&str, &str)] = &[
    ("write_file", "Write"),
    ("write_to_file", "Write"),
    ("edit_file", "Edit"),
    ("replace_file_content", "Edit"),
    ("multi_replace_file_content", "Edit"),
    ("sed_file", "Edit"),
    // `notebook_edit` is in agy's tool list, `edit_notebook` a step type in its binary (P41c's read).
    ("notebook_edit", "NotebookEdit"),
    ("edit_notebook", "NotebookEdit"),
    ("read_file", "Read"),
    ("view_file", "Read"),
    ("invoke_subagent", "Agent"),
    ("define_subagent", "Agent"),
    ("run_workflow", "Agent"),
    ("run_command", "Bash"),
];

pub fn claude_tool(agy: &str) -> Option<&'static str> {
    AGY_TOOLS
        .iter()
        .find(|(name, _)| *name == agy)
        .map(|(_, tool)| *tool)
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
///
/// `main_session` is true when this conversation's `SessionStart` came with the owner's prompt
/// (`Conversation::main`). Then an empty `lastUserInput` does not make it a subagent: the Sonnet review found that an
/// orchestrator Stop with an empty input would have skipped the Stop gate. An `agent_id` (or `agentId`) in the
/// payload is a subagent whatever else is true; no agy 1.5.9 payload carried one, so that arm is for a later agy.
pub fn is_subagent(raw: &Value, main_session: bool) -> bool {
    if ["agent_id", "agentId"]
        .iter()
        .any(|key| text(raw, key).is_some_and(|id| !id.is_empty()))
    {
        return true;
    }
    !main_session && text(raw, "lastUserInput").is_none_or(|input| input.trim().is_empty())
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
/// `continuationActive`. A subagent's hook (`subagent`, from `is_subagent`) carries an `agent_id`, as Claude's does,
/// so every verb that skips a subagent skips it here too.
pub fn to_claude(raw: &Value, cwd: Option<&Path>, stop_hook_active: bool, subagent: bool) -> Value {
    let event = text(raw, "hookEventName").and_then(agy_event);
    let mut hook = json!({
        "host": AGY,
        "session_id": text(raw, "conversationId"),
        "transcript_path": text(raw, "transcriptPath"),
        "cwd": cwd.map(|c| c.display().to_string()),
        "hook_event_name": event.map(Event::claude_name),
        "model": text(raw, "modelName"),
    });
    if subagent {
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
/// `count_ask` records). `asks` is how many continues this turn has had: at `AGY_STOP_ASKS_PER_PROMPT` a block is
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

// ---- the state: one turn's notices, one turn's asks ----
//
// A turn is what Claude Code calls one: an owner prompt, the model calls it takes, and the Stop that ends it. The
// first build keyed a turn on `lastUserInput` alone, so an owner who typed "yes" twice in one conversation got no
// second notice, no second `prompt-baseline`, and the second turn's Stop read asks=1: the dead-end gate was spent
// silently (the Sonnet review of P41b, 2026-11-14). What the P41a payloads give instead (every `PreInvocation` of runs
// 1, 4 and 6, `events_*.jsonl` in that session's scratchpad):
//
// - `invocationNum` counts the model calls of one execution and restarts at 0 for each new owner prompt. It does not
//   restart on a Stop hook's continue (run 1: 7 then 8; run 4: 1 then 2), so a continue stays in its turn and the cap
//   holds.
// - `initialNumSteps` is the conversation's step count when that call began. It only grows (run 1: 1, 4, 6, 8, 10,
//   then 12 for the second prompt), so two prompts of the same text get two different numbers.
//
// So a turn starts at a `PreInvocation` with `invocationNum` 0, and is named by its `initialNumSteps`
// (`Conversation::turn`). Every verb of that `PreInvocation` records the same name, so the verbs need not agree on
// who records it. A Stop carries neither field and reads the name its turn's `PreInvocation` left. The turn key
// (`turn_key`) is that name and the prompt's hash, so a conversation whose `PreInvocation` never reached a hook still
// keys on the prompt, as before.
//
// What this costs: `invocationNum` also restarts at 0 when a subagent's message wakes the parent (run 1, twice, with
// the same `lastUserInput`). That wake is a turn here: the notices and `prompt-baseline` run again and the gate may
// ask once more. Claude Code starts a turn for a background agent's message too; whether it fires `UserPromptSubmit`
// for one is not measured here. Telling the two apart would need the transcript's step at `initialNumSteps - 1`.

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    prompt: String,
    #[serde(default)]
    asks: u32,
    at: u64,
}

/// What the hooks know of one agy conversation, in `CONVERSATIONS_FILE`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Conversation {
    /// The `initialNumSteps` of the turn's first `PreInvocation`; empty before one was seen.
    #[serde(default)]
    turn: String,
    /// Its `SessionStart` came with the owner's prompt, so it is a main session, never a subagent (`is_subagent`).
    #[serde(default)]
    main: bool,
    at: u64,
}

trait Aged {
    fn at(&self) -> u64;
}

impl Aged for Mark {
    fn at(&self) -> u64 {
        self.at
    }
}

impl Aged for Conversation {
    fn at(&self) -> u64 {
        self.at
    }
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

fn load<T: serde::de::DeserializeOwned>(path: &Path) -> BTreeMap<String, T> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save<T: Serialize + Aged>(path: &Path, mut marks: BTreeMap<String, T>, now: u64) {
    marks.retain(|_, mark| now.saturating_sub(mark.at()) < STATE_KEEP_S);
    if let Ok(text) = serde_json::to_string(&marks) {
        let _ = crate::nirvana_seat::write_atomic(path, text.as_bytes());
    }
}

/// The `initialNumSteps` of a `PreInvocation` that starts a turn (`invocationNum` 0), as the turn's name.
pub fn turn_start(raw: &Value) -> Option<String> {
    (text(raw, "hookEventName") == Some("PreInvocation")
        && raw.get("invocationNum").and_then(Value::as_u64) == Some(0))
    .then(|| raw.get("initialNumSteps").and_then(Value::as_u64))
    .flatten()
    .map(|steps| steps.to_string())
}

/// Records what this hook says of its conversation (a turn's start, a main session's start) and returns what is
/// known: the turn key and whether it is a main session.
fn note_conversation(state_dir: &Path, raw: &Value, now: u64) -> (String, bool) {
    let path = state_dir.join(CONVERSATIONS_FILE);
    let mut known: BTreeMap<String, Conversation> = load(&path);
    let entry = known.entry(conversation(raw)).or_default();
    let mut changed = false;
    if let Some(start) = turn_start(raw) {
        if entry.turn != start {
            entry.turn = start;
            changed = true;
        }
    }
    let with_prompt = text(raw, "lastUserInput").is_some_and(|input| !input.trim().is_empty());
    if text(raw, "hookEventName") == Some("SessionStart") && with_prompt && !entry.main {
        entry.main = true;
        changed = true;
    }
    let (turn, main) = (turn_key(&entry.turn, raw), entry.main);
    if changed {
        entry.at = now;
        save(&path, known, now);
    }
    (turn, main)
}

/// The key a turn's notices and asks are kept under: its `PreInvocation`'s name and the prompt's hash.
fn turn_key(turn: &str, raw: &Value) -> String {
    format!("{turn}|{}", prompt_key(raw))
}

/// True the first time `verb` sees this turn of this conversation, and records it. agy fires `PreInvocation`
/// before every model call (P41a: 5 for one prompt), where Claude fires `UserPromptSubmit` once; a notice repeated
/// per call would cost a turn's context each time, and `prompt-baseline` would reset the edit budget mid-turn.
/// Each verb keeps its own mark, since each is its own hook process.
pub fn first_for_turn(state_dir: &Path, verb: &str, raw: &Value, turn: &str, now: u64) -> bool {
    let path = state_dir.join(PROMPTS_FILE);
    let mut marks: BTreeMap<String, Mark> = load(&path);
    let key = format!("{verb}|{}", conversation(raw));
    if marks.get(&key).is_some_and(|mark| mark.prompt == turn) {
        return false;
    }
    marks.insert(
        key,
        Mark {
            prompt: turn.to_owned(),
            asks: 0,
            at: now,
        },
    );
    save(&path, marks, now);
    true
}

/// The continues agy has been asked for in this turn of this conversation. A new turn starts at 0, as Claude's
/// `stop_hook_active` is false on the first Stop after an owner prompt.
pub fn stop_asks(state_dir: &Path, raw: &Value, turn: &str) -> u32 {
    load::<Mark>(&state_dir.join(ASKS_FILE))
        .get(&conversation(raw))
        .filter(|mark| mark.prompt == turn)
        .map_or(0, |mark| mark.asks)
}

/// Records one more continue for this turn.
pub fn count_ask(state_dir: &Path, raw: &Value, turn: &str, now: u64) {
    let _lock = lock_state(state_dir);
    let path = state_dir.join(ASKS_FILE);
    let mut marks: BTreeMap<String, Mark> = load(&path);
    let mark = marks.entry(conversation(raw)).or_default();
    if mark.prompt != turn {
        *mark = Mark::default();
        mark.prompt = turn.to_owned();
    }
    mark.asks += 1;
    mark.at = now;
    save(&path, marks, now);
}

/// How long a hook waits for another hook's hold on the state. agy runs every verb of one event at once (P41c's
/// `hooks.json` lists three on `PreInvocation`), each a read-modify-write of the same files, and the last rename
/// would win: a lost prompt mark repeats a notice, a lost ask mark lets the gate ask twice. A hold lasts one small
/// read and write, so 2 s is long; past it the hook goes on unlocked rather than stall agy's model call.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// Holds the OS's lock on `LOCK_FILE` while it lives (`File::try_lock`, as `nirvana_seat`'s lease lock: the OS lets
/// go when the handle closes, a dead hook included, so nothing goes stale). `None` inside when the wait ran out.
struct StateLock(#[allow(dead_code)] Option<std::fs::File>);

fn lock_state(state_dir: &Path) -> StateLock {
    let _ = std::fs::create_dir_all(state_dir);
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(state_dir.join(LOCK_FILE))
    else {
        return StateLock(None);
    };
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    loop {
        if file.try_lock().is_ok() {
            return StateLock(Some(file));
        }
        if std::time::Instant::now() >= deadline {
            return StateLock(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
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
    /// `turn_key` of this hook's turn, empty on Claude and outside the orchestrator.
    turn: String,
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
    read_or_exit(
        host(),
        verb,
        &input,
        hooks_home().as_deref(),
        &state_dir(),
        crate::nirvana_run::now(),
        std::env::var(crate::nirvana_launch::ORCHESTRATOR_ENV)
            .ok()
            .as_deref(),
    )
}

/// `read_stdin_for` with its inputs passed in, so a test can run it in a child process
/// (`an_agy_hook_outside_the_orchestrator_exits_before_any_state`). Outside the orchestrator it prints the skipped
/// answer and ends the process before `read_for`, so the owner's own agy never reads or writes the hook state (the
/// Sonnet review: the first build wrote `agy-prompts.json` on every prompt of every agy session, then skipped). The
/// exit, not `skip`, is the guard: several verbs (`record-session`, `log-usage`) do their work without reading `skip`.
pub fn read_or_exit(
    host: Host,
    verb: &str,
    input: &str,
    hooks_home: Option<&Path>,
    state_dir: &Path,
    now: u64,
    marker: Option<&str>,
) -> HookIn {
    if outside_orchestrator(host, marker) {
        unread(host, input).answer_skipped();
        std::process::exit(0);
    }
    let hin = read_for(host, verb, input, hooks_home, state_dir, now);
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

/// Plan 41 P41c: `agy plugin install` puts Nirvana's plugin in agy's config, and agy turns an installed plugin on for
/// every session on the machine: the owner's own agy and herdr's seated agy workers would run the gate and the
/// guards too. Only the orchestrator's launch sets `NIRVANA_ORCHESTRATOR=1`, and seated workers have it removed
/// (`nirvana_worker::ENV_REMOVED`), so an agy hook without it answers as a skipped hook and does no work. Claude is
/// untouched: its plugin loads only on Nirvana Claude.
pub fn outside_orchestrator(host: Host, marker: Option<&str>) -> bool {
    host == Host::Agy && marker != Some("1")
}

/// A hook read for its event alone, skipped, with no state touched: what a hook outside the orchestrator answers.
pub fn unread(host: Host, input: &str) -> HookIn {
    let raw: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    HookIn {
        host,
        event: text(&raw, "hookEventName").and_then(agy_event),
        hook: Value::Null,
        skip: true,
        raw: Value::Null,
        asks: 0,
        turn: String::new(),
        state_dir: PathBuf::new(),
    }
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
            turn: String::new(),
            state_dir: state_dir.to_path_buf(),
        };
    }
    let raw = parsed;
    let event = text(&raw, "hookEventName").and_then(agy_event);
    // One hold for the conversation record, the prompt mark and the ask count: the verbs of one event run at once.
    let lock = lock_state(state_dir);
    let (turn, main_session) = note_conversation(state_dir, &raw, now);
    let subagent = is_subagent(&raw, main_session);
    let asks = if event == Some(Event::Stop) {
        stop_asks(state_dir, &raw, &turn)
    } else {
        0
    };
    let cwd = session_cwd(&raw, hooks_home);
    let hook = to_claude(&raw, cwd.as_deref(), asks > 0, subagent);
    let skip = match event {
        // Claude fires no SessionStart, UserPromptSubmit or Stop for a subagent (it fires SubagentStop, which has
        // no agy event, plan 41 §5.1).
        Some(Event::SessionStart | Event::Stop) => subagent,
        Some(Event::UserPromptSubmit) => subagent || !first_for_turn(state_dir, verb, &raw, &turn, now),
        Some(Event::PreToolUse | Event::PostToolUse) => verb_tools(verb).is_some_and(|tools| {
            !hook
                .get("tool_name")
                .and_then(Value::as_str)
                .is_some_and(|tool| tools.contains(&tool))
        }),
        None => true,
    };
    drop(lock);
    HookIn {
        host,
        event,
        hook,
        skip,
        raw,
        asks,
        turn,
        state_dir: state_dir.to_path_buf(),
    }
}

impl HookIn {
    /// What to print for the verb's Claude output `out`: `out` itself on Claude, agy's JSON on agy. A Stop's continue
    /// is counted here, so the next Stop of the turn reads `stop_hook_active`.
    pub fn answer_text(&self, out: &str, now: u64) -> String {
        if self.host == Host::Claude {
            return out.to_owned();
        }
        let out = if self.skip { "" } else { out };
        let (value, continues) = agy_output(self.event, out, self.asks);
        if continues {
            count_ask(&self.state_dir, &self.raw, &self.turn, now);
        }
        format!("{value}\n")
    }

    /// What a skipped hook prints: `allow` on a PreToolUse, `{}` otherwise, never a block or a continue.
    pub fn skipped_text(&self) -> String {
        let (value, _) = agy_output(self.event, "", 0);
        format!("{value}\n")
    }

    /// Prints `skipped_text`; nothing is counted.
    pub fn answer_skipped(&self) {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        let _ = stdout.write_all(self.skipped_text().as_bytes());
        let _ = stdout.flush();
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
