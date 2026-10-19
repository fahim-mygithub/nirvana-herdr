//! Nirvana fork, plan 17: subagents under each agent in the agents panel.
//! Claude subagent reader: folder encoding, session resolution with shared-folder refusal,
//! meta and tail parse, running vs done, model spelling, token figure, workflow nesting,
//! and per-pane cache with poll, take_changed, and cached.
//! (nirvana/plans/2026-10-19-17-subagents.md §3)

use std::collections::HashMap;
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
    pub fn colour(&self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Claude => Some((0xD8, 0x88, 0x6A)),
            Self::Gemini => Some((0xAB, 0x93, 0xD6)),
            Self::Local => None,
        }
    }
}

/// Status of a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentState {
    Running,
    Done,
    Failed,
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

/// Claude projects directory under `home`: `<home>/.claude/projects/<encoded_folder>`.
pub fn claude_project_dir(home: &Path, folder: &Path) -> PathBuf {
    home.join(".claude")
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
    let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
    let cache_create = usage
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
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
    let model = obj
        .get("model")
        .and_then(Value::as_str)
        .map(String::from);
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
            };
        }
    }
    TranscriptTail::default()
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
    home: &Path,
    now_secs: u64,
) -> Option<PathBuf> {
    let enc_folder = encode_project_folder(pane_folder);

    // Check shared-folder conflict with any other Claude pane sharing the same encoded folder
    let has_conflict = other_claude_panes.iter().any(|(_, other_folder, other_sess)| {
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

    let projects_dir = home.join(".claude").join("projects").join(&enc_folder);

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
    if let Some(record) = crate::nirvana_session_record::read(home, pane_folder) {
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
        if agent_start_secs.is_none_or(|start| mtime >= start) && mtime <= now_secs + 300 {
            if newest.as_ref().is_none_or(|(_, best_mtime)| mtime > *best_mtime) {
                newest = Some((path, mtime));
            }
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
            let tail = read_file_tail(&transcript_path, 65536)
                .ok()
                .map(|t| parse_transcript_tail(&t))
                .unwrap_or_default();
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
        if let Some(usage) = val.pointer("/step_update/usage").or_else(|| val.pointer("/result/usage")) {
            let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
            let cache_read = usage.get("cache_read_tokens").and_then(Value::as_u64).unwrap_or(0);
            let output = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
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

/// Finds `agy` processes running under `shell_pid` (e.g. started from a Bash call in Claude).
pub fn find_agy_processes_under(shell_pid: u32, parent_pane_id: &str) -> Vec<Subagent> {
    let mut subagents = Vec::new();
    for pid in find_child_pids(shell_pid) {
        if let Some(cmdline) = crate::platform::process_command_line(pid) {
            let lower = cmdline.to_ascii_lowercase();
            if lower.contains("agy") {
                let model_name = parse_cmdline_model(&cmdline);
                let model = format_model(model_name.as_deref(), SubagentProvider::Gemini);
                subagents.push(Subagent {
                    id: format!("agy-{pid}"),
                    parent_id: None,
                    name: "agy".into(),
                    description: cmdline,
                    provider: SubagentProvider::Gemini,
                    model,
                    state: SubagentState::Running,
                    elapsed_secs: 0,
                    tokens: None, // Probed: agy db does not expose readable token figures; shows '-'
                    ended_at: None,
                    depth: 0,
                    target_pane_id: Some(parent_pane_id.to_string()),
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
        let is_failed = job.exit.is_some_and(|c| c != 0) || (job.ended.is_some() && job.exit.is_none());
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
        });
    }
    subagents
}

/// Reads seat orders (`herdr seat run`) nesting under `orchestrator_pane_id`.
pub fn read_seat_orders(project_folder: &Path, orchestrator_pane_id: &str, now_secs: u64) -> Vec<Subagent> {
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
            let is_done = matches!(order.phase, crate::nirvana_run::Phase::Merged | crate::nirvana_run::Phase::Gated);
            let is_failed = order.phase == crate::nirvana_run::Phase::Failed;
            let is_running = !order.phase.terminal();

            let ended_at = if order.phase.terminal() {
                Some(order.last_at)
            } else {
                None
            };
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

            let provider = if order.seated.cli == "agy" {
                SubagentProvider::Gemini
            } else {
                SubagentProvider::Claude
            };

            let model = format_model(Some(&order.seated.model), provider);

            let stream_path = entry.path().join("streams").join(format!("{}.jsonl", order.seated.order));
            let tokens = if stream_path.is_file() {
                parse_agy_stream_tokens(&stream_path)
            } else {
                order.tokens().map(|t| t.input + t.cache_read + t.output)
            };

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
}

static ENTRIES: Mutex<Option<HashMap<String, PaneCacheEntry>>> = Mutex::new(None);
static CHANGED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    static TEST_VIEW: std::cell::RefCell<Option<HashMap<String, SubagentsView>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_view_for_test(view: Option<HashMap<String, SubagentsView>>) {
    TEST_VIEW.with_borrow_mut(|v| *v = view);
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
        });
    entry.folder = folder;
    entry.is_claude = is_claude;
    entry.agent_session = agent_session;
    entry.agent_start_secs = agent_start_secs;
    entry.shell_pid = shell_pid;
}

/// Polls panes off the draw path: fills the cache per pane, every 2s while any listed
/// subagent runs and every 10s otherwise.
pub fn poll() {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let home = crate::nirvana::home();

    let due: Vec<(String, PathBuf, bool, Option<String>, Option<u64>, Option<u32>)> = {
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

        let mut to_read = Vec::new();
        for (pane_id, entry) in map.iter_mut() {
            if entry.reading {
                continue;
            }
            let Some(folder) = entry.folder.clone() else {
                continue;
            };
            let any_running = entry
                .view
                .as_ref()
                .is_some_and(|v| v.subagents.iter().any(|s| s.state == SubagentState::Running));
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
                to_read.push((
                    pane_id.clone(),
                    folder,
                    entry.is_claude,
                    entry.agent_session.clone(),
                    entry.agent_start_secs,
                    entry.shell_pid,
                ));
            }
        }
        to_read
    };

    if due.is_empty() {
        return;
    }

    std::thread::spawn(move || {
        let Some(home) = home else {
            return;
        };
        for (pane_id, folder, is_claude, agent_session, agent_start_secs, shell_pid) in due {
            let mut all_subagents = Vec::new();

            if is_claude {
                let session_path = resolve_claude_session(
                    &pane_id,
                    &folder,
                    agent_session.as_deref(),
                    agent_start_secs,
                    &[], // Other panes checked by client query
                    &home,
                    now_secs,
                );

                if let Some(session_path) = session_path {
                    all_subagents.extend(read_claude_subagents(&session_path, &pane_id, now_secs));
                }
            }

            // Seat orders
            all_subagents.extend(read_seat_orders(&folder, &pane_id, now_secs));

            // agy jobs
            all_subagents.extend(read_agy_jobs(&home, &pane_id, now_secs));

            // Bash-started agy
            if let Some(pid) = shell_pid {
                all_subagents.extend(find_agy_processes_under(pid, &pane_id));
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
                if let Some(entry) = map.get_mut(&pane_id) {
                    entry.reading = false;
                    entry.read_at = Some(Instant::now());
                    if entry.view != view {
                        entry.view = view;
                        CHANGED.store(true, Ordering::Release);
                    }
                }
            }
        }
    });
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
        assert_eq!(
            format_model(None, SubagentProvider::Claude),
            "inherit"
        );
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
            200,
        );
        assert_eq!(res, Some(file1));
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
}
