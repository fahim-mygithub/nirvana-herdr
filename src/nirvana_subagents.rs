//! Nirvana fork, plan 17: subagents under each agent in the agents panel.
//! Claude subagent reader: folder encoding, session resolution with shared-folder refusal,
//! meta and tail parse, running vs done, model spelling, token figure, workflow nesting,
//! and per-pane cache with poll, take_changed, and cached.
//! (nirvana/plans/2026-10-19-17-subagents.md §3)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Provider of a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentProvider {
    Claude,
    Gemini,
    Local,
}

impl SubagentProvider {
    /// Provider mark symbol: ✻ for Claude, ◆ for Gemini, ● for Local.
    pub fn mark(&self) -> &'static str {
        match self {
            Self::Claude => "\u{273B}", // ✻
            Self::Gemini => "\u{25C6}", // ◆
            Self::Local => "\u{25CF}",  // ●
        }
    }

    /// Provider colour: Anthropic orange/coral for Claude, purple for Gemini, None for Local.
    /// Calls `nirvana_usage::Provider::colour()` so a palette change reaches both.
    pub fn colour(&self) -> Option<(u8, u8, u8)> {
        let usage_provider = match self {
            Self::Claude => crate::nirvana_usage::Provider::Claude,
            Self::Gemini => crate::nirvana_usage::Provider::Antigravity,
            Self::Local => crate::nirvana_usage::Provider::Local,
        };
        usage_provider.colour()
    }
}

/// Status of a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentState {
    Running,
    Done,
    Blocked,
    Failed,
    Stopped,
}

/// A parsed subagent row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subagent {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub description: String,
    pub provider: SubagentProvider,
    pub model: String,
    pub state: SubagentState,
    pub elapsed_secs: u64,
    pub tokens: Option<u64>,
    pub ended_at: Option<u64>,
    pub depth: usize,
    pub target_pane_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
}

/// Cached view of subagents for a pane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentsView {
    pub subagents: Vec<Subagent>,
    pub total_count: usize,
    pub overflow: usize,
}

// -----------------------------------------------------------------------------
// Folder Encoding (§3)
// -----------------------------------------------------------------------------

/// Encodes a project folder path as Claude Code encodes it for `~/.claude/projects/<enc>`:
/// every character that is not an ASCII letter or digit becomes `-`.
pub fn encode_project_folder(folder: &Path) -> String {
    let s = folder.to_string_lossy().replace('\\', "/");
    let trimmed = s.trim_end_matches('/');
    let s = if trimmed.is_empty() { &s[..] } else { trimmed };
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Claude projects directory under `claude_dir`: `<claude_dir>/projects/<encoded_folder>`.
pub fn claude_project_dir(claude_dir: &Path, folder: &Path) -> PathBuf {
    claude_dir
        .join("projects")
        .join(encode_project_folder(folder))
}

// -----------------------------------------------------------------------------
// Model Spelling & Tokens Formatting (§2)
// -----------------------------------------------------------------------------

/// Spells model names as the strip spells models (e.g. `Sonnet 5.5`, `Opus 5.5`, `Haiku 5.5`,
/// `Gemini 3.8 Flash`, `Gemini 4 Argon`). A Claude subagent with no model in its transcript reads `inherit`.
pub fn format_model(model: Option<&str>, provider: SubagentProvider) -> String {
    match provider {
        SubagentProvider::Claude => {
            let Some(m) = model.map(str::trim).filter(|s| !s.is_empty()) else {
                return "inherit".to_string();
            };
            if m.eq_ignore_ascii_case("inherit") {
                return "inherit".to_string();
            }
            if let Some(label) = crate::nirvana_spaces::claude_label(m) {
                if let Some(stripped) = label.strip_prefix("Claude ") {
                    stripped.to_string()
                } else {
                    label
                }
            } else if m.starts_with("claude-") {
                m.strip_prefix("claude-").unwrap_or(m).to_string()
            } else {
                match m.to_ascii_lowercase().as_str() {
                    "sonnet" => "Sonnet".to_string(),
                    "opus" => "Opus".to_string(),
                    "haiku" => "Haiku".to_string(),
                    other => other.to_string(),
                }
            }
        }
        SubagentProvider::Gemini => {
            let Some(m) = model.map(str::trim).filter(|s| !s.is_empty()) else {
                return "Gemini".to_string();
            };
            let lower = m.to_ascii_lowercase();
            if lower.contains("3.8") || lower.contains("3-8") || lower.contains("flash") {
                "Gemini 3.8 Flash".to_string()
            } else if lower.contains("argon") || lower.contains("4") {
                "Gemini 4 Argon".to_string()
            } else if lower.starts_with("gemini") {
                m.to_string()
            } else {
                format!("Gemini {m}")
            }
        }
        SubagentProvider::Local => {
            let Some(m) = model.map(str::trim).filter(|s| !s.is_empty()) else {
                return "Local".to_string();
            };
            if m.starts_with("Local ") {
                m.to_string()
            } else {
                format!("Local {m}")
            }
        }
    }
}

/// Drops the version number from a formatted model string when displayed in narrow width.
/// E.g. "Sonnet 5.5" -> "Sonnet", "Gemini 3.8 Flash" -> "Gemini Flash", "Gemini 4 Argon" -> "Gemini Argon", "Opus 5.5" -> "Opus".
pub fn short_model(model: &str) -> String {
    if let Some(rest) = model.strip_prefix("Gemini ") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            return format!("Gemini {}", parts.last().unwrap_or(&""));
        }
    }
    model.split_whitespace().next().unwrap_or(model).to_string()
}

/// Formats a token figure: e.g. "133.7k", "1.2M", "500", or "-" when none.
pub fn format_tokens(tokens: Option<u64>) -> String {
    let Some(t) = tokens else {
        return "-".to_string();
    };
    if t >= 1_000_000 {
        format!("{:.1}M", t as f64 / 1_000_000.0)
    } else if t >= 1_000 {
        format!("{:.1}k", t as f64 / 1_000.0)
    } else {
        t.to_string()
    }
}

/// Formats elapsed time: e.g. "6m59s", "12m", "45s", "1h5m".
pub fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{m}m")
        } else {
            format!("{m}m{s:02}s")
        }
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h{m}m")
        }
    }
}

/// Sums Claude Code usage fields: input + cache_creation + cache_read + output tokens.
/// Measured on Claude Code 2.1.303: matches the turn's total context.
pub fn parse_claude_usage(usage: &Value) -> u64 {
    let input = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    input + cache_create + cache_read + output
}

// -----------------------------------------------------------------------------
// Metadata & Transcript Tail Parsing (§3)
// -----------------------------------------------------------------------------

/// Metadata read from `agent-<id>.meta.json`.
#[derive(Debug, Clone, Default)]
pub struct SubagentMeta {
    pub agent_type: String,
    pub description: String,
    pub model: Option<String>,
    pub finished: Option<bool>,
    pub parent_agent_id: Option<String>,
    pub name: Option<String>,
    pub is_failed: bool,
}

/// Parses an `agent-<id>.meta.json` file.
pub fn parse_meta_json(content: &str) -> Option<SubagentMeta> {
    let val: Value = serde_json::from_str(content.trim_start_matches('\u{feff}')).ok()?;
    let obj = val.as_object()?;
    let agent_type = obj
        .get("agentType")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let description = obj
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let model = obj.get("model").and_then(Value::as_str).map(String::from);
    let finished = obj.get("finished").and_then(Value::as_bool);
    let parent_agent_id = obj
        .get("parentAgentId")
        .and_then(Value::as_str)
        .map(String::from);
    let name = obj.get("name").and_then(Value::as_str).map(String::from);
    let is_failed = obj.get("error").is_some()
        || obj.get("failed") == Some(&Value::Bool(true))
        || obj.get("status") == Some(&Value::String("failed".into()));

    Some(SubagentMeta {
        agent_type,
        description,
        model,
        finished,
        parent_agent_id,
        name,
        is_failed,
    })
}

/// Parsed assistant turn from the tail of a transcript.
#[derive(Debug, Clone, Default)]
pub struct TranscriptTail {
    pub model: Option<String>,
    pub tokens: Option<u64>,
    pub timestamp_secs: Option<u64>,
    pub has_assistant: bool,
}

/// Reads at most `max_bytes` from the tail of `path`.
pub fn read_file_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let text = String::from_utf8_lossy(&buf).into_owned();
    if start > 0 {
        if let Some(pos) = text.find('\n') {
            return Ok(text[pos + 1..].to_string());
        }
    }
    Ok(text)
}

/// Parses the latest assistant turn in a transcript tail:
/// takes the last assistant line (which contains the latest turn's model and cumulative usage).
pub fn parse_transcript_tail(text: &str) -> TranscriptTail {
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if val.get("type").and_then(Value::as_str) == Some("assistant") {
            let msg = val.get("message");
            let model = msg
                .and_then(|m| m.get("model"))
                .and_then(Value::as_str)
                .map(String::from);
            let tokens = msg.and_then(|m| m.get("usage")).map(parse_claude_usage);
            let timestamp_secs = val
                .get("timestamp")
                .and_then(Value::as_str)
                .and_then(crate::nirvana_usage::rfc3339);
            return TranscriptTail {
                model,
                tokens,
                timestamp_secs,
                has_assistant: true,
            };
        }
    }
    TranscriptTail::default()
}

pub const INITIAL_TAIL_WINDOW: u64 = 65536; // 64 KB
pub const MAX_TAIL_WINDOW: u64 = 8 * 1024 * 1024; // 8 MB

/// Reads transcript tail with window expansion: starts at 64 KB, multiplying by 4 up to 8 MB
/// until an assistant turn is found or the beginning of file is reached.
pub fn read_transcript_tail_with_expansion(path: &Path) -> TranscriptTail {
    let mut window = INITIAL_TAIL_WINDOW;
    loop {
        let tail_text = match read_file_tail(path, window) {
            Ok(t) => t,
            Err(_) => return TranscriptTail::default(),
        };
        let tail = parse_transcript_tail(&tail_text);
        if tail.has_assistant {
            return tail;
        }
        if window >= MAX_TAIL_WINDOW {
            return tail;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() <= window {
                return tail;
            }
        }
        window = (window * 4).min(MAX_TAIL_WINDOW);
    }
}

/// Determines whether a subagent is Running, Done, or Failed.
/// Running means no `finished: true` and the transcript was written within the last 5 minutes.
/// Older meta files with no `finished` key stand in as done after 5 minutes of inactivity.
pub fn determine_subagent_state(
    meta: &SubagentMeta,
    transcript_mtime_secs: u64,
    now_secs: u64,
) -> SubagentState {
    if meta.is_failed {
        return SubagentState::Failed;
    }
    if meta.finished == Some(true) {
        return SubagentState::Done;
    }
    if now_secs.saturating_sub(transcript_mtime_secs) <= 300 {
        SubagentState::Running
    } else {
        SubagentState::Done
    }
}

// -----------------------------------------------------------------------------
// Session Resolution (§3)
// -----------------------------------------------------------------------------

/// Resolves the Claude session transcript for a pane:
/// 1. herdr's own `agent_session` for the pane, when reported;
/// 2. Nirvana session record (`nirvana_session_record::read`), when younger than the pane's agent;
/// 3. otherwise, the newest `<session>.jsonl` written while the pane's agent ran.
///
/// Shared-folder refusal: when two Claude panes share one folder and nothing tells them apart,
/// neither shows subagents (returns `None`).
pub fn resolve_claude_session(
    _pane_id: &str,
    pane_folder: &Path,
    agent_session: Option<&str>,
    agent_start_secs: Option<u64>,
    other_claude_panes: &[(&str, &Path, Option<&str>)],
    claude_dir: &Path,
    nirvana_home: &Path,
    now_secs: u64,
) -> Option<PathBuf> {
    let enc_folder = encode_project_folder(pane_folder);

    // Check shared-folder conflict with any other Claude pane sharing the same encoded folder
    let has_conflict = other_claude_panes
        .iter()
        .any(|(_, other_folder, other_sess)| {
            if encode_project_folder(other_folder) == enc_folder {
                // If neither has an explicit session, or both have the same session, they cannot be told apart
                match (agent_session, other_sess) {
                    (Some(s1), Some(s2)) => s1 == *s2,
                    _ => true,
                }
            } else {
                false
            }
        });

    if has_conflict {
        // Refusal: two Claude panes share one folder and nothing tells them apart
        return None;
    }

    let projects_dir = claude_dir.join("projects").join(&enc_folder);

    // 1. herdr's own agent_session
    if let Some(session) = agent_session.map(str::trim).filter(|s| !s.is_empty()) {
        let path = Path::new(session);
        if path.is_absolute() && path.is_file() {
            return Some(path.to_path_buf());
        }
        let direct = projects_dir.join(format!("{session}.jsonl"));
        if direct.is_file() {
            return Some(direct);
        }
    }

    // 2. Nirvana session record for the pane's folder
    if let Some(record) = crate::nirvana_session_record::read(nirvana_home, pane_folder) {
        if let Some(record_at) = crate::nirvana_usage::rfc3339(&record.at) {
            let younger = agent_start_secs.is_none_or(|start| record_at >= start);
            let transcript = Path::new(&record.transcript);
            if younger && transcript.is_file() {
                return Some(transcript.to_path_buf());
            }
        }
    }

    // 3. Newest <session>.jsonl written while the pane's agent ran
    let Ok(entries) = std::fs::read_dir(&projects_dir) else {
        return None;
    };
    let mut newest: Option<(PathBuf, u64)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if agent_start_secs.is_none_or(|start| mtime >= start)
            && mtime <= now_secs + 300
            && newest
                .as_ref()
                .is_none_or(|(_, best_mtime)| mtime > *best_mtime)
        {
            newest = Some((path, mtime));
        }
    }

    newest.map(|(path, _)| path)
}

// -----------------------------------------------------------------------------
// Reading & Nesting Subagents from a Session (§2, §3)
// -----------------------------------------------------------------------------

/// Reads subagents from `<session_dir>/subagents/`:
/// parses metadata, tails transcripts, applies the 10-minute finished filter,
/// and nests workflow children beneath their parents.
pub fn read_claude_subagents(
    session_transcript_path: &Path,
    parent_pane_id: &str,
    now_secs: u64,
) -> Vec<Subagent> {
    let session_dir = session_transcript_path.with_extension("");
    let subagents_dir = session_dir.join("subagents");
    let Ok(entries) = std::fs::read_dir(&subagents_dir) else {
        return Vec::new();
    };

    let mut raw_subagents = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        if !file_name.ends_with(".meta.json") {
            continue;
        }
        let base_name = file_name.strip_suffix(".meta.json").unwrap_or("");
        let subagent_id = base_name.strip_prefix("agent-").unwrap_or(base_name);

        let Ok(meta_content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(meta) = parse_meta_json(&meta_content) else {
            continue;
        };

        let transcript_path = subagents_dir.join(format!("{base_name}.jsonl"));
        let (transcript_mtime, tail) = if transcript_path.is_file() {
            let mtime = transcript_path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let tail = read_transcript_tail_with_expansion(&transcript_path);
            (mtime, tail)
        } else {
            (now_secs, TranscriptTail::default())
        };

        let state = determine_subagent_state(&meta, transcript_mtime, now_secs);

        // Filter: finished ones show for 10 minutes (600 s) after they end
        if state != SubagentState::Running && now_secs.saturating_sub(transcript_mtime) > 600 {
            continue;
        }

        let name = meta.name.clone().unwrap_or(meta.agent_type.clone());
        let model = format_model(
            tail.model.as_deref().or(meta.model.as_deref()),
            SubagentProvider::Claude,
        );

        let start_secs = tail.timestamp_secs.unwrap_or(transcript_mtime);
        let elapsed_secs = if state == SubagentState::Running {
            now_secs.saturating_sub(start_secs)
        } else {
            transcript_mtime.saturating_sub(start_secs)
        };

        raw_subagents.push(Subagent {
            id: subagent_id.to_string(),
            parent_id: meta.parent_agent_id,
            name,
            description: meta.description,
            provider: SubagentProvider::Claude,
            model,
            state,
            elapsed_secs,
            tokens: tail.tokens,
            ended_at: if state != SubagentState::Running {
                Some(transcript_mtime)
            } else {
                None
            },
            depth: 0,
            target_pane_id: Some(parent_pane_id.to_string()),
            run_id: None,
        });
    }

    nest_subagents(raw_subagents)
}

/// Nests child workflow subagents (depth 1) directly beneath their parent subagent (depth 0).
pub fn nest_subagents(items: Vec<Subagent>) -> Vec<Subagent> {
    let mut roots = Vec::new();
    let mut children_by_parent: HashMap<String, Vec<Subagent>> = HashMap::new();

    for item in items {
        if let Some(pid) = &item.parent_id {
            children_by_parent
                .entry(pid.clone())
                .or_default()
                .push(item);
        } else {
            roots.push(item);
        }
    }

    let mut result = Vec::new();
    for mut root in roots {
        root.depth = 0;
        let root_id = root.id.clone();
        result.push(root);
        if let Some(children) = children_by_parent.remove(&root_id) {
            for mut child in children {
                child.depth = 1;
                result.push(child);
            }
        }
    }

    // Any remaining orphans without a matching parent in the current list
    for (_, orphans) in children_by_parent {
        for mut orphan in orphans {
            orphan.depth = 0;
            result.push(orphan);
        }
    }

    result
}

// -----------------------------------------------------------------------------
// The agy Readers (§4)
// -----------------------------------------------------------------------------

/// Extracts `--model X`, `--model=X` or `-m X` from command line arguments.
pub fn argv_model(args: &[String]) -> Option<String> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--model" || arg == "-m" {
            return rest.next().filter(|value| !value.starts_with('-')).cloned();
        }
        if let Some(value) = arg.strip_prefix("--model=") {
            return Some(value.to_string());
        }
    }
    None
}

/// Parses a command line string into arguments and extracts the `--model` value.
pub fn parse_cmdline_model(cmdline: &str) -> Option<String> {
    let mut words = Vec::new();
    let mut in_quote = false;
    let mut current = String::new();
    for c in cmdline.chars() {
        if c == '"' {
            in_quote = !in_quote;
        } else if c.is_whitespace() && !in_quote {
            if !current.is_empty() {
                words.push(current);
                current = String::new();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    argv_model(&words)
}

/// Parses an agy worker stream file (e.g. `<run>/streams/<order>.jsonl`) and returns
/// the latest step's token count (input + output).
pub fn parse_agy_stream_tokens(path: &Path) -> Option<u64> {
    let text = read_file_tail(path, 65536).ok()?;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if let Some(usage) = val
            .pointer("/step_update/usage")
            .or_else(|| val.pointer("/result/usage"))
        {
            let input = usage
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let cache_read = usage
                .get("cache_read_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let output = usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            return Some(input + cache_read + output);
        }
    }
    None
}

/// Finds all descendant process IDs of `parent_pid`.
#[cfg(windows)]
pub fn find_child_pids(parent_pid: u32) -> Vec<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut descendants = Vec::new();
    let mut to_visit = vec![parent_pid];
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot != INVALID_HANDLE_VALUE {
        let mut processes = Vec::new();
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        while ok {
            processes.push((entry.th32ProcessID, entry.th32ParentProcessID));
            ok = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        unsafe { CloseHandle(snapshot) };

        let mut index = 0;
        while index < to_visit.len() {
            let current = to_visit[index];
            index += 1;
            for &(child, parent) in &processes {
                if parent == current && child > 4 && !descendants.contains(&child) {
                    descendants.push(child);
                    to_visit.push(child);
                }
            }
        }
    }
    descendants
}

#[cfg(not(windows))]
pub fn find_child_pids(_parent_pid: u32) -> Vec<u32> {
    Vec::new()
}

/// Returns true if the command line starts with an executable named `agy` or `agy.exe`.
pub fn is_agy_cmdline(cmdline: &str) -> bool {
    let trimmed = cmdline.trim();
    if trimmed.is_empty() {
        return false;
    }
    let exe = if let Some(stripped) = trimmed.strip_prefix('"') {
        stripped.split('"').next().unwrap_or("")
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };
    is_agy_executable(exe)
}

/// Returns true if `exe_path` has the file name `agy.exe` or `agy`.
pub fn is_agy_executable(exe_path: &str) -> bool {
    let path = Path::new(exe_path);
    let file_name = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(exe_path);
    file_name.eq_ignore_ascii_case("agy.exe") || file_name.eq_ignore_ascii_case("agy")
}

/// Converts Windows FILETIME ticks (100-nanosecond intervals since January 1, 1601 UTC)
/// to Unix epoch seconds (seconds since January 1, 1970 UTC).
pub fn filetime_ticks_to_unix_secs(ticks: u64) -> u64 {
    const TICKS_PER_SEC: u64 = 10_000_000;
    const UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;
    if ticks < UNIX_EPOCH_TICKS {
        0
    } else {
        (ticks - UNIX_EPOCH_TICKS) / TICKS_PER_SEC
    }
}

/// Returns process creation time in Unix epoch seconds from GetProcessTimes.
#[cfg(windows)]
pub fn process_creation_secs(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut creation_time = unsafe { std::mem::zeroed::<FILETIME>() };
    let mut exit_time = unsafe { std::mem::zeroed::<FILETIME>() };
    let mut kernel_time = unsafe { std::mem::zeroed::<FILETIME>() };
    let mut user_time = unsafe { std::mem::zeroed::<FILETIME>() };
    let ok = unsafe {
        GetProcessTimes(
            handle,
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    };
    unsafe { CloseHandle(handle) };
    if ok == 0 {
        return None;
    }
    let ticks =
        (u64::from(creation_time.dwHighDateTime) << 32) | u64::from(creation_time.dwLowDateTime);
    Some(filetime_ticks_to_unix_secs(ticks))
}

#[cfg(not(windows))]
pub fn process_creation_secs(_pid: u32) -> Option<u64> {
    None
}

/// Finds `agy` processes running under `shell_pid` (e.g. started from a Bash call in Claude).
pub fn find_agy_processes_under(
    shell_pid: u32,
    parent_pane_id: &str,
    now_secs: u64,
) -> Vec<Subagent> {
    let mut subagents = Vec::new();
    for pid in find_child_pids(shell_pid) {
        if let Some(cmdline) = crate::platform::process_command_line(pid) {
            if is_agy_cmdline(&cmdline) {
                let model_name = parse_cmdline_model(&cmdline);
                let model = format_model(model_name.as_deref(), SubagentProvider::Gemini);
                let description = crate::platform::process_cwd(pid)
                    .and_then(|cwd| cwd.file_name().and_then(|n| n.to_str()).map(String::from))
                    .unwrap_or_default();
                let created_secs = process_creation_secs(pid);
                let elapsed_secs = created_secs
                    .map(|c| now_secs.saturating_sub(c))
                    .unwrap_or(0);

                subagents.push(Subagent {
                    id: format!("agy-{pid}"),
                    parent_id: None,
                    name: "agy".into(),
                    description,
                    provider: SubagentProvider::Gemini,
                    model,
                    state: SubagentState::Running,
                    elapsed_secs,
                    tokens: None, // Probed: agy db does not expose readable token figures; shows '-'
                    ended_at: None,
                    depth: 0,
                    target_pane_id: Some(parent_pane_id.to_string()),
                    run_id: None,
                });
            }
        }
    }
    subagents
}

/// Reads agy jobs from `<home>/nirvana/jobs/` that were started by `pane_id`.
pub fn read_agy_jobs(home: &Path, pane_id: &str, now_secs: u64) -> Vec<Subagent> {
    let jobs_dir = crate::nirvana_job::jobs_dir(home);
    let Ok(entries) = std::fs::read_dir(&jobs_dir) else {
        return Vec::new();
    };

    let mut subagents = Vec::new();
    for entry in entries.flatten() {
        let record_path = entry.path().join("job.json");
        let Ok(content) = std::fs::read_to_string(&record_path) else {
            continue;
        };
        let Ok(job) = serde_json::from_str::<crate::nirvana_job::JobRecord>(&content) else {
            continue;
        };
        if job.pane.as_deref() != Some(pane_id) {
            continue;
        }
        let is_agy = job
            .argv
            .first()
            .is_some_and(|exe| exe.to_ascii_lowercase().contains("agy"));
        if !is_agy {
            continue;
        }

        let is_done = job.exit == Some(0);
        let is_failed =
            job.exit.is_some_and(|c| c != 0) || (job.ended.is_some() && job.exit.is_none());
        let is_running = !is_done && !is_failed;

        let ended_at = job.ended;
        if !is_running && ended_at.is_some_and(|ended| now_secs.saturating_sub(ended) > 600) {
            continue;
        }

        let state = if is_done {
            SubagentState::Done
        } else if is_failed {
            SubagentState::Failed
        } else {
            SubagentState::Running
        };

        let model = format_model(argv_model(&job.argv).as_deref(), SubagentProvider::Gemini);
        let elapsed_secs = ended_at.unwrap_or(now_secs).saturating_sub(job.started);

        subagents.push(Subagent {
            id: format!("job-{}", job.id),
            parent_id: None,
            name: job.name,
            description: job.argv.join(" "),
            provider: SubagentProvider::Gemini,
            model,
            state,
            elapsed_secs,
            tokens: None,
            ended_at,
            depth: 0,
            target_pane_id: Some(pane_id.to_string()),
            run_id: None,
        });
    }
    subagents
}

/// Reads seat orders (`herdr seat run`) nesting under `orchestrator_pane_id`.
pub fn read_seat_orders(
    project_folder: &Path,
    orchestrator_pane_id: &str,
    now_secs: u64,
) -> Vec<Subagent> {
    let runs_dir = project_folder.join(".nirvana").join("runs");
    let Ok(entries) = std::fs::read_dir(&runs_dir) else {
        return Vec::new();
    };

    let mut subagents = Vec::new();
    for entry in entries.flatten() {
        let journal_path = entry.path().join("journal.jsonl");
        let Ok(file_content) = std::fs::read_to_string(&journal_path) else {
            continue;
        };
        let mut events = Vec::new();
        for line in file_content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<crate::nirvana_run::Event>(trimmed) {
                events.push(event);
            }
        }
        let run_state = crate::nirvana_run::replay(&events);
        let is_our_orch = run_state
            .opened
            .as_ref()
            .and_then(|o| o.orchestrator_pane.as_deref())
            == Some(orchestrator_pane_id);
        if !is_our_orch {
            continue;
        }

        for order in run_state.orders {
            if matches!(
                order.phase,
                crate::nirvana_run::Phase::Merged | crate::nirvana_run::Phase::Gated
            ) || order.verdict.as_deref() == Some("abandoned")
            {
                continue;
            }

            let is_live =
                !order.phase.terminal() && order.phase != crate::nirvana_run::Phase::Delivered;
            let ended_at = if !is_live { Some(order.last_at) } else { None };

            let state = if is_live {
                SubagentState::Running
            } else if order.phase == crate::nirvana_run::Phase::Delivered {
                let report_status = order
                    .report
                    .as_ref()
                    .and_then(|r| r.get("status"))
                    .and_then(Value::as_str)
                    .unwrap_or("done");
                if report_status == "done" {
                    SubagentState::Done
                } else if report_status == "blocked" {
                    SubagentState::Blocked
                } else {
                    SubagentState::Failed
                }
            } else if order.phase == crate::nirvana_run::Phase::Failed {
                SubagentState::Failed
            } else if order.phase == crate::nirvana_run::Phase::Interrupted
                || order.outcome.as_deref() == Some("stopped")
                || order.outcome.as_deref() == Some("interrupted")
            {
                SubagentState::Stopped
            } else {
                SubagentState::Failed
            };

            let provider = if order.seated.cli == "agy" {
                SubagentProvider::Gemini
            } else {
                SubagentProvider::Claude
            };

            let model = format_model(Some(&order.seated.model), provider);

            let stream_path = entry
                .path()
                .join("streams")
                .join(format!("{}.jsonl", order.seated.order));
            let tokens = if stream_path.is_file() {
                parse_agy_stream_tokens(&stream_path)
            } else {
                order.tokens().map(|t| t.input + t.cache_read + t.output)
            };

            if let Some(pane) = &order.pane {
                record_worker_pane(pane.clone());
            }

            let elapsed_secs = ended_at.unwrap_or(now_secs).saturating_sub(order.seated_at);

            let target_pane = order
                .pane
                .clone()
                .or_else(|| Some(orchestrator_pane_id.to_string()));

            let description = if !order.seated.reason.is_empty() {
                order.seated.reason.clone()
            } else {
                order.seated.class.clone()
            };

            let run_id = entry.file_name().to_string_lossy().to_string();

            subagents.push(Subagent {
                id: format!("order-{}", order.seated.order),
                parent_id: None,
                name: order.seated.order.clone(),
                description,
                provider,
                model,
                state,
                elapsed_secs,
                tokens,
                ended_at,
                depth: 0,
                target_pane_id: target_pane,
                run_id: Some(run_id),
            });
        }
    }
    subagents
}

// -----------------------------------------------------------------------------
// Cache & Polling Architecture (§2)
// -----------------------------------------------------------------------------

struct PaneCacheEntry {
    view: Option<SubagentsView>,
    read_at: Option<Instant>,
    reading: bool,
    asked_at: Option<Instant>,
    folder: Option<PathBuf>,
    is_claude: bool,
    agent_session: Option<String>,
    agent_start_secs: Option<u64>,
    shell_pid: Option<u32>,
    shell_pid_failed_at: Option<Instant>,
}

struct PollTask {
    pane_id: String,
    folder: PathBuf,
    is_claude: bool,
    agent_session: Option<String>,
    agent_start_secs: Option<u64>,
    shell_pid: Option<u32>,
    shell_pid_failed_at: Option<Instant>,
    other_claude: Vec<(String, PathBuf, Option<String>)>,
}

static ENTRIES: Mutex<Option<HashMap<String, PaneCacheEntry>>> = Mutex::new(None);
static CHANGED: AtomicBool = AtomicBool::new(false);
static WORKER_PANES: Mutex<Option<HashSet<String>>> = Mutex::new(None);
#[cfg(test)]
static TEST_CLAUDE_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Claude Code's config folder: `CLAUDE_CONFIG_DIR`, else `USERPROFILE` (then `HOME`) joined with `.claude`.
pub fn claude_config_dir() -> Option<PathBuf> {
    #[cfg(test)]
    {
        let guard = TEST_CLAUDE_DIR.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = guard.as_ref() {
            return Some(dir.clone());
        }
    }
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()))
        .map(|home| PathBuf::from(home).join(".claude"))
}

#[cfg(test)]
pub fn set_claude_dir_for_test(dir: Option<PathBuf>) {
    let mut guard = TEST_CLAUDE_DIR.lock().unwrap_or_else(|e| e.into_inner());
    *guard = dir;
}

fn herdr_self(args: &[&str], timeout: Duration) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let mut command = std::process::Command::new(exe);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    crate::platform::configure_background_command(&mut command);
    let mut child = command.spawn().ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    let mut stdout = Vec::new();
                    if let Some(mut out) = child.stdout.take() {
                        let _ = std::io::Read::read_to_end(&mut out, &mut stdout);
                    }
                    return Some(String::from_utf8_lossy(&stdout).into_owned());
                } else {
                    return None;
                }
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Returns the CLI arguments for querying pane process information.
pub fn pane_process_info_args(pane_id: &str) -> Vec<String> {
    vec![
        "pane".into(),
        "process-info".into(),
        "--pane".into(),
        pane_id.into(),
    ]
}

/// Queries herdr's own `pane.process_info` to get the pane's shell pid.
pub fn query_pane_shell_pid(pane_id: &str) -> Option<u32> {
    let args = pane_process_info_args(pane_id);
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = herdr_self(&str_args, Duration::from_secs(2))?;
    let val: Value = serde_json::from_str(&out).ok()?;
    [
        "/result/process_info/shell_pid",
        "/process_info/shell_pid",
        "/shell_pid",
    ]
    .iter()
    .find_map(|pointer| val.pointer(pointer).and_then(Value::as_u64))
    .map(|p| p as u32)
}

#[cfg(test)]
thread_local! {
    static TEST_VIEW: std::cell::RefCell<Option<HashMap<String, SubagentsView>>> = const { std::cell::RefCell::new(None) };
    static TEST_WORKER_PANES: std::cell::RefCell<Option<HashSet<String>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_view_for_test(view: Option<HashMap<String, SubagentsView>>) {
    TEST_VIEW.with_borrow_mut(|v| *v = view);
}

#[cfg(test)]
pub fn set_worker_panes_for_test(panes: Option<HashSet<String>>) {
    TEST_WORKER_PANES.with_borrow_mut(|p| *p = panes);
}

#[cfg(test)]
pub fn get_pane_query_for_test(pane_id: &str) -> Option<(Option<PathBuf>, bool, Option<String>)> {
    let guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()?
        .get(pane_id)
        .map(|e| (e.folder.clone(), e.is_claude, e.agent_session.clone()))
}

/// Returns whether `pane_id` is a known seat worker pane, which should not also be listed
/// as a separate top-level agent row.
pub fn is_seat_worker_pane(pane_id: &str) -> bool {
    #[cfg(test)]
    {
        if let Some(set) = TEST_WORKER_PANES.with_borrow(|s| s.clone()) {
            return set.contains(pane_id);
        }
    }
    let guard = WORKER_PANES.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().is_some_and(|set| set.contains(pane_id))
}

/// Records a pane ID as a seat worker pane.
pub fn record_worker_pane(pane_id: String) {
    let mut guard = WORKER_PANES.lock().unwrap_or_else(|e| e.into_inner());
    guard.get_or_insert_with(HashSet::new).insert(pane_id);
}

/// Returns the cached subagents view for `pane_id`.
pub fn cached(pane_id: &str) -> Option<SubagentsView> {
    #[cfg(test)]
    {
        if let Some(map) = TEST_VIEW.with_borrow(Clone::clone) {
            return map.get(pane_id).cloned();
        }
    }

    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map
        .entry(pane_id.to_string())
        .or_insert_with(|| PaneCacheEntry {
            view: None,
            read_at: None,
            reading: false,
            asked_at: Some(Instant::now()),
            folder: None,
            is_claude: true,
            agent_session: None,
            agent_start_secs: None,
            shell_pid: None,
            shell_pid_failed_at: None,
        });
    entry.asked_at = Some(Instant::now());
    entry.view.clone()
}

/// Updates the metadata query parameters for a pane in the cache.
pub fn update_pane_query(
    pane_id: &str,
    folder: Option<PathBuf>,
    is_claude: bool,
    agent_session: Option<String>,
    agent_start_secs: Option<u64>,
    shell_pid: Option<u32>,
) {
    let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map
        .entry(pane_id.to_string())
        .or_insert_with(|| PaneCacheEntry {
            view: None,
            read_at: None,
            reading: false,
            asked_at: Some(Instant::now()),
            folder: folder.clone(),
            is_claude,
            agent_session: agent_session.clone(),
            agent_start_secs,
            shell_pid,
            shell_pid_failed_at: None,
        });
    if entry.folder != folder || entry.agent_session != agent_session {
        entry.read_at = None;
    }
    entry.folder = folder;
    entry.is_claude = is_claude;
    entry.agent_session = agent_session;
    if agent_start_secs.is_some() {
        entry.agent_start_secs = agent_start_secs;
    }
    // Finding B: keep cached shell_pid when caller passes None
    if shell_pid.is_some() {
        entry.shell_pid = shell_pid;
    }
}

pub fn poll_with<F>(
    now_secs: u64,
    home: Option<PathBuf>,
    claude_dir: Option<PathBuf>,
    query_shell_pid: F,
    sync: bool,
) where
    F: Fn(&str) -> Option<u32> + Send + Sync + 'static,
{
    let due: Vec<PollTask> = {
        let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
        let Some(map) = guard.as_mut() else {
            return;
        };
        // Forget entries unasked for more than 60s
        map.retain(|_, entry| {
            entry
                .asked_at
                .is_some_and(|at| at.elapsed() < Duration::from_secs(60))
        });

        let mut all_claude: Vec<(String, PathBuf, Option<String>)> = Vec::new();
        for (pane_id, entry) in map.iter() {
            if entry.is_claude {
                if let Some(folder) = &entry.folder {
                    all_claude.push((pane_id.clone(), folder.clone(), entry.agent_session.clone()));
                }
            }
        }

        let mut to_read = Vec::new();
        for (pane_id, entry) in map.iter_mut() {
            if entry.reading {
                continue;
            }
            let Some(folder) = entry.folder.clone() else {
                continue;
            };
            let any_running = entry.view.as_ref().is_some_and(|v| {
                v.subagents
                    .iter()
                    .any(|s| s.state == SubagentState::Running)
            });
            let reread_interval = if any_running {
                Duration::from_secs(2)
            } else {
                Duration::from_secs(10)
            };

            if entry
                .read_at
                .is_none_or(|at| at.elapsed() >= reread_interval)
            {
                entry.reading = true;
                let other_claude = all_claude
                    .iter()
                    .filter(|(id, _, _)| id != pane_id)
                    .cloned()
                    .collect();
                to_read.push(PollTask {
                    pane_id: pane_id.clone(),
                    folder,
                    is_claude: entry.is_claude,
                    agent_session: entry.agent_session.clone(),
                    agent_start_secs: entry.agent_start_secs,
                    shell_pid: entry.shell_pid,
                    shell_pid_failed_at: entry.shell_pid_failed_at,
                    other_claude,
                });
            }
        }
        to_read
    };

    if due.is_empty() {
        return;
    }

    let run_tasks = move || {
        for task in due {
            let mut all_subagents = Vec::new();

            if task.is_claude {
                let other_claude_refs: Vec<(&str, &Path, Option<&str>)> = task
                    .other_claude
                    .iter()
                    .map(|(id, folder, sess)| (id.as_str(), folder.as_path(), sess.as_deref()))
                    .collect();

                let session_path = match (&claude_dir, &home) {
                    (Some(c_dir), Some(h_dir)) => resolve_claude_session(
                        &task.pane_id,
                        &task.folder,
                        task.agent_session.as_deref(),
                        task.agent_start_secs,
                        &other_claude_refs,
                        c_dir,
                        h_dir,
                        now_secs,
                    ),
                    _ => None,
                };

                if let Some(session_path) = session_path {
                    all_subagents.extend(read_claude_subagents(
                        &session_path,
                        &task.pane_id,
                        now_secs,
                    ));
                }
            }

            // Seat orders
            all_subagents.extend(read_seat_orders(&task.folder, &task.pane_id, now_secs));

            // agy jobs
            if let Some(home) = &home {
                all_subagents.extend(read_agy_jobs(home, &task.pane_id, now_secs));
            }

            // Bash-started agy: shell_pid from cache, or query pane.process_info off draw path
            let mut shell_pid = task.shell_pid;
            let mut query_failed = false;

            if shell_pid.is_none() {
                let recent_failure = task
                    .shell_pid_failed_at
                    .is_some_and(|failed_at| failed_at.elapsed() < Duration::from_secs(60));
                if !recent_failure {
                    shell_pid = query_shell_pid(&task.pane_id);
                    if shell_pid.is_none() {
                        query_failed = true;
                    }
                }
            }

            if let Some(pid) = shell_pid {
                all_subagents.extend(find_agy_processes_under(pid, &task.pane_id, now_secs));
            }

            let nested = nest_subagents(all_subagents);
            let total_count = nested.len();
            let overflow = total_count.saturating_sub(6);
            let shown = nested.into_iter().take(6).collect();

            let view = Some(SubagentsView {
                subagents: shown,
                total_count,
                overflow,
            });

            let mut guard = ENTRIES.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(map) = guard.as_mut() {
                if let Some(entry) = map.get_mut(&task.pane_id) {
                    if entry.shell_pid.is_none() && shell_pid.is_some() {
                        entry.shell_pid = shell_pid;
                        entry.shell_pid_failed_at = None;
                    } else if query_failed {
                        entry.shell_pid_failed_at = Some(Instant::now());
                    }
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    if entry.view != view {
                        entry.view = view;
                        CHANGED.store(true, Ordering::Release);
                    }
                }
            }
        }
    };

    if sync {
        run_tasks();
    } else {
        std::thread::spawn(run_tasks);
    }
}

/// Polls panes off the draw path: fills the cache per pane, every 2s while any listed
/// subagent runs and every 10s otherwise.
pub fn poll() {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let home = crate::nirvana::home();
    let claude_dir = claude_config_dir();
    poll_with(now_secs, home, claude_dir, query_pane_shell_pid, false);
}

#[cfg(test)]
pub fn poll_sync_for_test(now_secs: u64, home: Option<&Path>, claude_dir: Option<&Path>) {
    poll_with(
        now_secs,
        home.map(PathBuf::from),
        claude_dir.map(PathBuf::from),
        |_| None,
        true,
    );
}

#[cfg(test)]
pub fn poll_sync_with_query_for_test<F>(
    now_secs: u64,
    home: Option<&Path>,
    claude_dir: Option<&Path>,
    query_fn: F,
) where
    F: Fn(&str) -> Option<u32> + Send + Sync + 'static,
{
    poll_with(
        now_secs,
        home.map(PathBuf::from),
        claude_dir.map(PathBuf::from),
        query_fn,
        true,
    );
}

/// Returns whether any subagents view changed since the last call, triggering UI repaint.
pub fn take_changed() -> bool {
    poll();
    CHANGED.swap(false, Ordering::AcqRel)
}

// -----------------------------------------------------------------------------
// Unit Tests (§5)
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn folder_encoding_converts_non_alphanumeric_to_dash() {
        let path = Path::new(r"C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool");
        assert_eq!(
            encode_project_folder(path),
            "C--Users-fahim-Desktop-Pojects-3D-Printer-Flow-xtool"
        );
    }

    #[test]
    fn model_spellings_match_doctrine() {
        assert_eq!(
            format_model(Some("claude-sonnet-5-5"), SubagentProvider::Claude),
            "Sonnet 5.5"
        );
        assert_eq!(
            format_model(Some("claude-opus-5-5"), SubagentProvider::Claude),
            "Opus 5.5"
        );
        assert_eq!(
            format_model(Some("claude-haiku-5-5"), SubagentProvider::Claude),
            "Haiku 5.5"
        );
        assert_eq!(
            format_model(Some("claude-3-5-sonnet-20241022"), SubagentProvider::Claude),
            "Sonnet 3.5"
        );
        assert_eq!(format_model(None, SubagentProvider::Claude), "inherit");
        assert_eq!(
            format_model(Some("gemini-3.8-flash"), SubagentProvider::Gemini),
            "Gemini 3.8 Flash"
        );
        assert_eq!(
            format_model(Some("gemini-4-argon"), SubagentProvider::Gemini),
            "Gemini 4 Argon"
        );
    }

    #[test]
    fn token_figure_sums_measured_context() {
        // Measured 2026-10-19 on Claude Code:
        // {"input_tokens": 2, "cache_creation_input_tokens": 3390, "cache_read_input_tokens": 129589, "output_tokens": 10} -> 132991
        let usage = serde_json::json!({
            "input_tokens": 2,
            "cache_creation_input_tokens": 3390,
            "cache_read_input_tokens": 129589,
            "output_tokens": 10
        });
        let total = parse_claude_usage(&usage);
        assert_eq!(total, 132_991);
        assert_eq!(format_tokens(Some(total)), "133.0k");
        assert_eq!(format_tokens(Some(1_200_000)), "1.2M");
        assert_eq!(format_tokens(None), "-");
    }

    #[test]
    fn elapsed_formatting_matches_spec() {
        assert_eq!(format_elapsed(45), "45s");
        assert_eq!(format_elapsed(419), "6m59s");
        assert_eq!(format_elapsed(720), "12m");
    }

    #[test]
    fn meta_and_transcript_pair_cases() {
        let now = 1_000_000;

        // Case A: Running (no finished key, transcript written 60s ago)
        let meta_running = SubagentMeta {
            agent_type: "general-purpose".into(),
            description: "search".into(),
            model: Some("sonnet".into()),
            finished: None,
            parent_agent_id: None,
            name: None,
            is_failed: false,
        };
        assert_eq!(
            determine_subagent_state(&meta_running, now - 60, now),
            SubagentState::Running
        );

        // Case B: Finished with the flag
        let meta_finished = SubagentMeta {
            finished: Some(true),
            ..meta_running.clone()
        };
        assert_eq!(
            determine_subagent_state(&meta_finished, now - 60, now),
            SubagentState::Done
        );

        // Case C: Old with no flag (> 5 min ago)
        assert_eq!(
            determine_subagent_state(&meta_running, now - 400, now),
            SubagentState::Done
        );

        // Case D: Failed
        let meta_failed = SubagentMeta {
            is_failed: true,
            ..meta_running
        };
        assert_eq!(
            determine_subagent_state(&meta_failed, now - 60, now),
            SubagentState::Failed
        );
    }

    #[test]
    fn workflow_nesting_places_child_under_parent() {
        let parent = Subagent {
            id: "a1".into(),
            parent_id: None,
            name: "general-purpose".into(),
            description: "parent".into(),
            provider: SubagentProvider::Claude,
            model: "Sonnet 5.5".into(),
            state: SubagentState::Running,
            elapsed_secs: 100,
            tokens: Some(5000),
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: None,
        };
        let child = Subagent {
            id: "a2".into(),
            parent_id: Some("a1".into()),
            name: "synthesis".into(),
            description: "child".into(),
            provider: SubagentProvider::Claude,
            model: "Sonnet 5.5".into(),
            state: SubagentState::Running,
            elapsed_secs: 50,
            tokens: Some(2000),
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: None,
        };
        let other = Subagent {
            id: "a3".into(),
            parent_id: None,
            name: "general-purpose".into(),
            description: "sibling".into(),
            provider: SubagentProvider::Claude,
            model: "Sonnet 5.5".into(),
            state: SubagentState::Running,
            elapsed_secs: 20,
            tokens: Some(1000),
            ended_at: None,
            depth: 0,
            target_pane_id: None,
            run_id: None,
        };

        let nested = nest_subagents(vec![child, other, parent]);
        assert_eq!(nested.len(), 3);
        assert_eq!(nested[0].id, "other".replace("other", "a3"));
        assert_eq!(nested[1].id, "a1");
        assert_eq!(nested[1].depth, 0);
        assert_eq!(nested[2].id, "a2");
        assert_eq!(nested[2].depth, 1);
    }

    #[test]
    fn session_resolution_order_and_shared_folder_refusal() {
        let temp = crate::nirvana::with_test_home("subagent-session-res");
        let proj = temp.path.join("proj");
        std::fs::create_dir_all(&proj).unwrap();

        // 1. Shared folder refusal: two panes sharing same folder without distinct sessions
        let pane1 = "pane-1";
        let pane2 = "pane-2";
        let other_panes = vec![(pane2, proj.as_path(), None)];
        let res = resolve_claude_session(
            pane1,
            &proj,
            None,
            Some(100),
            &other_panes,
            &temp.path,
            &temp.path,
            200,
        );
        assert!(res.is_none(), "Shared folder conflict must be refused");

        // 2. Case 1: distinct agent_session reported
        let file1 = temp.path.join("session-1.jsonl");
        std::fs::write(&file1, "").unwrap();
        let other_distinct = vec![(pane2, proj.as_path(), Some("sess-2"))];
        let res = resolve_claude_session(
            pane1,
            &proj,
            Some(file1.to_str().unwrap()),
            Some(100),
            &other_distinct,
            &temp.path,
            &temp.path,
            200,
        );
        assert_eq!(res, Some(file1));
    }

    #[test]
    fn claude_subagents_found_under_claude_dir_separate_from_nirvana_home() {
        let nirvana_temp = crate::nirvana::with_test_home("subagent-nirvana-home");
        let claude_dir = std::env::temp_dir().join(format!(
            "test-claude-dir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&claude_dir);
        std::fs::create_dir_all(&claude_dir).unwrap();
        struct DirGuard(PathBuf);
        impl Drop for DirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _claude_guard = DirGuard(claude_dir.clone());

        let proj = nirvana_temp.path.join("my-project");
        std::fs::create_dir_all(&proj).unwrap();

        // Subagents exist ONLY under claude_dir/projects/<enc_folder>/
        let enc_folder = encode_project_folder(&proj);
        let projects_dir = claude_dir.join("projects").join(&enc_folder);
        std::fs::create_dir_all(&projects_dir).unwrap();

        let session_file = projects_dir.join("test-sess.jsonl");
        std::fs::write(&session_file, "{}\n").unwrap();

        // When resolve_claude_session is called with claude_dir as claude_dir and nirvana_temp as nirvana_home:
        let resolved = resolve_claude_session(
            "p1",
            &proj,
            Some("test-sess"),
            None,
            &[],
            &claude_dir,
            &nirvana_temp.path,
            1000,
        );
        assert_eq!(resolved, Some(session_file));

        // Nirvana home has NO projects folder, and resolving with nirvana_temp as claude_dir yields None
        let resolved_wrong = resolve_claude_session(
            "p1",
            &proj,
            Some("test-sess"),
            None,
            &[],
            &nirvana_temp.path,
            &nirvana_temp.path,
            1000,
        );
        assert_eq!(
            resolved_wrong, None,
            "Must not find subagents under Nirvana home"
        );
    }

    #[test]
    fn shared_folder_refusal_in_poll() {
        let nirvana_temp = crate::nirvana::with_test_home("poll-refusal-nirvana");
        let claude_dir = std::env::temp_dir().join(format!(
            "poll-refusal-claude-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&claude_dir);
        std::fs::create_dir_all(&claude_dir).unwrap();
        struct DirGuard(PathBuf);
        impl Drop for DirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _claude_guard = DirGuard(claude_dir.clone());

        let proj = claude_dir.join("shared-proj");
        std::fs::create_dir_all(&proj).unwrap();

        let enc_folder = encode_project_folder(&proj);
        let projects_dir = claude_dir.join("projects").join(&enc_folder);
        std::fs::create_dir_all(&projects_dir).unwrap();

        let sess1_file = projects_dir.join("sess-1.jsonl");
        std::fs::write(&sess1_file, "{}\n").unwrap();
        let sub1_dir = projects_dir.join("sess-1").join("subagents");
        std::fs::create_dir_all(&sub1_dir).unwrap();
        std::fs::write(
            sub1_dir.join("agent-sub1.meta.json"),
            r#"{"agentType":"Architect","description":"arch1"}"#,
        )
        .unwrap();

        let sess2_file = projects_dir.join("sess-2.jsonl");
        std::fs::write(&sess2_file, "{}\n").unwrap();
        let sub2_dir = projects_dir.join("sess-2").join("subagents");
        std::fs::create_dir_all(&sub2_dir).unwrap();
        std::fs::write(
            sub2_dir.join("agent-sub2.meta.json"),
            r#"{"agentType":"Coder","description":"coder2"}"#,
        )
        .unwrap();

        // Case 1: Two Claude panes in the same folder with NO session IDs
        update_pane_query("pane-a", Some(proj.clone()), true, None, None, None);
        update_pane_query("pane-b", Some(proj.clone()), true, None, None, None);

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        poll_sync_for_test(now_secs, Some(&nirvana_temp.path), Some(&claude_dir));

        let view_a = cached("pane-a").unwrap();
        let view_b = cached("pane-b").unwrap();
        assert!(
            view_a.subagents.is_empty(),
            "Ambiguous shared folder in poll must refuse subagents for pane-a"
        );
        assert!(
            view_b.subagents.is_empty(),
            "Ambiguous shared folder in poll must refuse subagents for pane-b"
        );

        // Case 2: Give each pane its distinct session
        update_pane_query(
            "pane-a",
            Some(proj.clone()),
            true,
            Some("sess-1".into()),
            None,
            None,
        );
        update_pane_query(
            "pane-b",
            Some(proj.clone()),
            true,
            Some("sess-2".into()),
            None,
            None,
        );

        poll_sync_for_test(now_secs, Some(&nirvana_temp.path), Some(&claude_dir));

        let view_a = cached("pane-a").unwrap();
        let view_b = cached("pane-b").unwrap();
        assert_eq!(view_a.subagents.len(), 1);
        assert_eq!(view_a.subagents[0].id, "sub1");
        assert_eq!(view_b.subagents.len(), 1);
        assert_eq!(view_b.subagents[0].id, "sub2");
    }

    #[test]
    fn agy_exec_name_matching() {
        // Pure function tests for is_agy_cmdline and is_agy_executable
        assert!(is_agy_cmdline("agy"));
        assert!(is_agy_cmdline("agy.exe"));
        assert!(is_agy_cmdline(r#"C:\Users\bin\agy.exe --model flash"#));
        assert!(is_agy_cmdline(
            r#""C:\Program Files\agy.exe" --prompt hello"#
        ));
        assert!(is_agy_cmdline("/usr/local/bin/agy -m flash"));
        // Substring matches that are NOT agy executables must return false
        assert!(!is_agy_cmdline("python run.py --agent agy"));
        assert!(!is_agy_cmdline(r#"cmd.exe /c "echo agy""#));
        assert!(!is_agy_cmdline("notagy.exe --test"));

        // Pure function tests for filetime_ticks_to_unix_secs
        const EPOCH_TICKS: u64 = 116_444_736_000_000_000;
        assert_eq!(filetime_ticks_to_unix_secs(EPOCH_TICKS), 0);
        assert_eq!(
            filetime_ticks_to_unix_secs(EPOCH_TICKS + 42 * 10_000_000),
            42
        );
        assert_eq!(
            filetime_ticks_to_unix_secs(EPOCH_TICKS + 1_700_000_000 * 10_000_000),
            1_700_000_000
        );
        assert_eq!(filetime_ticks_to_unix_secs(100), 0);

        // Spawn a child process named so old substring rule matches but new executable rule does not
        #[cfg(windows)]
        {
            struct ChildGuard(std::process::Child);
            impl Drop for ChildGuard {
                fn drop(&mut self) {
                    let _ = self.0.kill();
                    let _ = self.0.wait();
                }
            }

            let mut cmd = std::process::Command::new("ping");
            cmd.args(["-n", "30", "127.0.0.1", "-a", "agy"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            crate::platform::configure_background_command(&mut cmd);
            let child = cmd.spawn().expect("spawn ping child with agy arg");
            let _guard = ChildGuard(child);

            // Give process table a moment
            std::thread::sleep(Duration::from_millis(50));

            // find_agy_processes_under test process: ping has 'agy' in arguments, but executable is ping.exe
            let subagents = find_agy_processes_under(std::process::id(), "p1", 2_000_000_000);
            assert!(
                subagents
                    .iter()
                    .all(|s| s.name == "agy" && is_agy_cmdline(&s.id)),
                "Processes that merely mention agy in args must not be reported as agy subagents"
            );
        }
    }

    #[test]
    fn tail_window_expands_to_find_assistant() {
        let temp = crate::nirvana::with_test_home("tail-expansion-test");
        let transcript_path = temp.path.join("large_tail.jsonl");

        let assistant_line = serde_json::json!({
            "type": "assistant",
            "message": {
                "model": "claude-3-5-sonnet-20241022",
                "usage": {
                    "input_tokens": 5000,
                    "output_tokens": 1200
                }
            },
            "timestamp": "2026-10-19T12:00:00Z"
        })
        .to_string();

        // 200 KB user line following the assistant line
        let large_user_line = serde_json::json!({
            "type": "user",
            "content": "x".repeat(200_000)
        })
        .to_string();

        let mut content = String::with_capacity(250_000);
        content.push_str(&assistant_line);
        content.push('\n');
        content.push_str(&large_user_line);
        content.push('\n');

        std::fs::write(&transcript_path, content).unwrap();

        // Tail reading with window expansion must expand beyond 64 KB up to 256 KB and find the assistant line
        let tail = read_transcript_tail_with_expansion(&transcript_path);
        assert!(
            tail.has_assistant,
            "Must find assistant line despite 200 KB trailing user line"
        );
        assert_eq!(tail.tokens, Some(6200));
        assert_eq!(tail.model.as_deref(), Some("claude-3-5-sonnet-20241022"));
    }

    #[test]
    fn agy_argv_to_model() {
        let argv1 = vec!["agy".into(), "--model".into(), "gemini-3.8-flash".into()];
        assert_eq!(argv_model(&argv1), Some("gemini-3.8-flash".into()));
        assert_eq!(
            format_model(argv_model(&argv1).as_deref(), SubagentProvider::Gemini),
            "Gemini 3.8 Flash"
        );

        let argv2 = vec!["agy".into(), "--model=gemini-4-argon".into()];
        assert_eq!(argv_model(&argv2), Some("gemini-4-argon".into()));
        assert_eq!(
            format_model(argv_model(&argv2).as_deref(), SubagentProvider::Gemini),
            "Gemini 4 Argon"
        );

        let cmdline = r#"C:\bin\agy.exe --model "gemini-3.8-flash" --some-arg"#;
        assert_eq!(
            parse_cmdline_model(cmdline),
            Some("gemini-3.8-flash".into())
        );
    }

    #[test]
    fn agy_stream_to_tokens() {
        let temp = crate::nirvana::with_test_home("agy_stream_tokens");
        let stream_file = temp.path.join("stream.jsonl");
        let content = r#"
{"type":"session","session_id":"test-123"}
{"step_update":{"step_index":0,"usage":{"input_tokens":500,"output_tokens":100}}}
{"step_update":{"step_index":1,"usage":{"input_tokens":1200,"cache_read_tokens":50,"output_tokens":300}}}
"#;
        std::fs::write(&stream_file, content.trim()).unwrap();
        let tokens = parse_agy_stream_tokens(&stream_file);
        assert_eq!(tokens, Some(1550));
        assert_eq!(format_tokens(tokens), "1.6k");
    }

    #[test]
    #[cfg(windows)]
    fn process_walk_finds_child_test_itself_started() {
        use std::os::windows::process::CommandExt;
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-Command", "Start-Sleep -Seconds 10"]);
        cmd.creation_flags(0x08000000);
        let child = cmd.spawn().expect("failed to spawn test child process");
        let child_pid = child.id();
        let _guard = ChildGuard(child);

        let my_pid = std::process::id();
        let mut found = false;
        for _ in 0..10 {
            let children = find_child_pids(my_pid);
            if children.contains(&child_pid) {
                found = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        assert!(
            found,
            "find_child_pids({my_pid}) did not find child pid {child_pid}"
        );
    }

    #[test]
    fn short_model_drops_version_number() {
        assert_eq!(short_model("Sonnet 5.5"), "Sonnet");
        assert_eq!(short_model("Opus 5.5"), "Opus");
        assert_eq!(short_model("Haiku 5.5"), "Haiku");
        assert_eq!(short_model("Sonnet 3.5"), "Sonnet");
        assert_eq!(short_model("Gemini 3.8 Flash"), "Gemini Flash");
        assert_eq!(short_model("Gemini 4 Argon"), "Gemini Argon");
        assert_eq!(short_model("inherit"), "inherit");
    }

    #[test]
    fn worker_pane_filtering() {
        let mut set = HashSet::new();
        set.insert("worker-pane-1".to_string());
        set_worker_panes_for_test(Some(set));

        assert!(is_seat_worker_pane("worker-pane-1"));
        assert!(!is_seat_worker_pane("orch-pane-1"));

        set_worker_panes_for_test(None);
    }

    #[test]
    fn pane_process_info_args_format() {
        let args = pane_process_info_args("target-pane-42");
        assert_eq!(
            args,
            vec!["pane", "process-info", "--pane", "target-pane-42"]
        );
    }

    #[test]
    fn cached_shell_pid_survives_none_update_and_failed_query_throttled() {
        // Part 1: cached Some(pid) survives update_pane_query with None
        update_pane_query("pane-survive-test", None, false, None, None, Some(54321));
        {
            let guard = ENTRIES.lock().unwrap();
            let entry = guard.as_ref().unwrap().get("pane-survive-test").unwrap();
            assert_eq!(entry.shell_pid, Some(54321));
        }

        // Simulated frame draw call with shell_pid = None:
        update_pane_query("pane-survive-test", None, false, None, None, None);
        {
            let guard = ENTRIES.lock().unwrap();
            let entry = guard.as_ref().unwrap().get("pane-survive-test").unwrap();
            assert_eq!(
                entry.shell_pid,
                Some(54321),
                "Cached shell_pid must survive update_pane_query with None"
            );
        }

        // Part 2: failed query is not repeated inside 60s
        use std::sync::atomic::AtomicUsize;
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = call_count.clone();
        let query_fn = move |_pane_id: &str| {
            cc.fetch_add(1, Ordering::SeqCst);
            None // Simulated failed query
        };

        let temp_folder = std::env::temp_dir();
        update_pane_query(
            "pane-throttle-test",
            Some(temp_folder),
            false,
            None,
            None,
            None,
        );

        // First poll: should call query_fn once
        poll_sync_with_query_for_test(1000, None, None, query_fn.clone());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Force entry to be due for reading again
        {
            let mut guard = ENTRIES.lock().unwrap();
            if let Some(entry) = guard.as_mut().and_then(|m| m.get_mut("pane-throttle-test")) {
                entry.read_at = None;
            }
        }

        // Second poll immediately (within 60s of failure): should NOT call query_fn again
        poll_sync_with_query_for_test(1005, None, None, query_fn);
        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "Failed shell pid query must not be repeated within 60s"
        );
    }
}
