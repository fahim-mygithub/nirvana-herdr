//! Plan 39 P39b, §2.2: an agent's conversation as bubbles (prototype screen 3).
//!
//! **Which file.** The page names an agent by its herdr pane id and nothing else; the file is chosen here, from
//! herdr's own answer for that agent, never from the request:
//! 1. The agent's `nirvana_transcript` token. The herdr server sets it for each Claude from the Claude process's
//!    own session file and its folder (`src/nirvana_spaces.rs` `probe`, the `crate::detect::Agent::Claude` arm:
//!    `<claude config>/projects/<claude_project_slug(cwd)>/<session_id>.jsonl`), and the Running square reads the
//!    same token (`src/client/shell/nirvana_mods.rs:82`). It is the live conversation, so it is the same file
//!    "Open Nirvana here" resumes from once that Claude stops.
//! 2. With no token (the probe has not run yet, or a Claude that never prompted): the folder's session record,
//!    which is what "Open Nirvana here" resumes from (`src/nirvana_launch.rs` `resume_session_at`, called by
//!    `resume_session` from `src/app/nirvana_new.rs:213`), with the same checks: a plain session id, the file's stem
//!    that id, its folder Claude Code's own name for the space's folder, and the file there.
//!
//! Either way the file must sit directly in a folder of `<claude config>/projects/` and be `<plain id>.jsonl`.
//!
//! **Whose record** (review round, S1). The record is the folder's, not an agent's, so it is taken only for an agent
//! that is a Claude (its `nirvana_kind` token, else herdr's own agent label, as `nirvana_spaces::Kind` reads them)
//! and the only Claude of this herdr in that folder. A Codex or Antigravity agent, or a second Claude in a folder
//! whose other Claude may own the record, gets none: its chat shows the live screen from `agent.read` and the note
//! that there is no transcript, never another agent's conversation. A Claude's own token, once its probe has run,
//! is its answer whatever else is in the folder.
//!
//! **Bubbles.** The transcript's tail (`read_tail`, 256 KiB), one JSON entry a line: a typed prompt is a user
//! bubble, the assistant's text an agent bubble, and each tool call one folded line whose result (the matching
//! `tool_result`) fills in when it comes. Notes Claude Code adds itself (`isMeta`), a subagent's lines
//! (`isSidechain`) and slash-command echoes (text that opens with `<`) are left out, as the Running square does
//! (`src/nirvana_running.rs` `typed_prompt`).
//!
//! **Live.** The bottom of the agent's screen from `agent.read`, as the PC shows it now.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// At most this many bubbles, the newest.
pub const MOST: usize = 150;
/// A bubble's text is cut here; a tool's detail sooner.
const TEXT_MOST: usize = 4000;
const DETAIL_MOST: usize = 1500;
/// A tool's result on its folded line.
const RESULT_MOST: usize = 48;
/// Lines of the live screen sent.
pub const LIVE_LINES: usize = 14;

/// A session id as Claude Code writes one (`nirvana_launch::plain_session_id`'s rule).
fn plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn same_dir(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// `path` is a transcript Claude Code wrote: `<claude>/projects/<folder>/<plain id>.jsonl`, and there.
pub fn is_transcript(path: &Path, claude: &Path) -> bool {
    let stem_ok = path.extension().is_some_and(|ext| ext == "jsonl")
        && path
            .file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(plain_id);
    let Some(projects) = path.parent().and_then(Path::parent) else {
        return false;
    };
    stem_ok && path.is_absolute() && path.is_file() && same_dir(projects, &claude.join("projects"))
}

/// Whether herdr's row is a Claude: its kind token, else herdr's agent label.
pub fn is_claude(agent: &Value) -> bool {
    use crate::nirvana_spaces::{Kind, KIND_TOKEN};
    let kind = match agent["tokens"][KIND_TOKEN].as_str() {
        Some(token) => Kind::parse(token),
        None => agent["agent"].as_str().and_then(Kind::from_agent_label),
    };
    kind == Some(Kind::Claude)
}

fn folder_of(agent: &Value) -> Option<&Path> {
    agent["cwd"]
        .as_str()
        .or_else(|| agent["foreground_cwd"].as_str())
        .filter(|folder| !folder.is_empty())
        .map(Path::new)
}

fn same_folder(left: &Path, right: &Path) -> bool {
    left == right || same_dir(left, right)
}

/// The agent's own transcript, by the rules above; `None` when it has none. `others` are herdr's other agents.
pub fn transcript_for(
    agent: &Value,
    others: &[Value],
    claude: Option<&Path>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let claude = claude?;
    if let Some(token) = agent["tokens"][crate::nirvana_spaces::TRANSCRIPT_TOKEN].as_str() {
        let path = PathBuf::from(token);
        return is_transcript(&path, claude).then_some(path);
    }
    if !is_claude(agent) {
        return None;
    }
    let home = home?;
    let folder = folder_of(agent)?;
    let shared = others.iter().any(|other| {
        other["pane_id"] != agent["pane_id"]
            && is_claude(other)
            && folder_of(other).is_some_and(|theirs| same_folder(theirs, folder))
    });
    if shared {
        return None;
    }
    let id = crate::nirvana_launch::resume_session_at(
        home,
        folder,
        crate::nirvana_run::now(),
        f64::INFINITY,
    )?;
    let text =
        std::fs::read_to_string(crate::nirvana_launch::session_record_path(home, folder)).ok()?;
    let record: crate::nirvana_launch::SessionRecord =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let path = PathBuf::from(record.transcript);
    (record.session_id == id && is_transcript(&path, claude)).then_some(path)
}

fn cut(text: &str, most: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= most {
        return text.to_string();
    }
    let mut out: String = text.chars().take(most).collect();
    out.push('\u{2026}');
    out
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .to_string()
}

/// The text blocks of a message's content.
fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// A transcript's tail as bubbles, oldest first. Lines that do not parse (the first, cut by the tail) are skipped.
pub fn bubbles(tail: &str) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for (index, entry) in tail
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .enumerate()
    {
        if entry["isMeta"] == true || entry["isSidechain"] == true {
            continue;
        }
        let key = entry["uuid"]
            .as_str()
            .map_or_else(|| format!("line{index}"), str::to_string);
        let content = &entry["message"]["content"];
        match entry["type"].as_str() {
            Some("user") => {
                for block in content.as_array().map(Vec::as_slice).unwrap_or_default() {
                    if block["type"] != "tool_result" {
                        continue;
                    }
                    let Some(id) = block["tool_use_id"].as_str() else {
                        continue;
                    };
                    let Some(call) = out.iter_mut().rev().find(|bubble| bubble["id"] == id) else {
                        continue;
                    };
                    let result = text_of(&block["content"]);
                    let failed = block["is_error"] == true;
                    call["ok"] = json!(!failed);
                    call["result"] = json!(cut(&first_line(&result), RESULT_MOST));
                    let detail = format!("{}\n\n{}", call["detail"].as_str().unwrap_or(""), result);
                    call["detail"] = json!(cut(&detail, DETAIL_MOST));
                }
                let text = text_of(content);
                let text = text.trim();
                if !text.is_empty() && !text.starts_with('<') {
                    out.push(json!({"id": key, "kind": "user", "text": cut(text, TEXT_MOST)}));
                }
            }
            Some("assistant") => {
                for (part, block) in content
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    match block["type"].as_str() {
                        Some("text") => {
                            let text = block["text"].as_str().unwrap_or("").trim();
                            if !text.is_empty() {
                                out.push(json!({
                                    "id": format!("{key}:{part}"),
                                    "kind": "agent",
                                    "text": cut(text, TEXT_MOST),
                                }));
                            }
                        }
                        Some("tool_use") => {
                            let input = &block["input"];
                            let detail = serde_json::to_string_pretty(input).unwrap_or_default();
                            out.push(json!({
                                "id": block["id"].as_str().map_or_else(|| format!("{key}:{part}"), str::to_string),
                                "kind": "tool",
                                "tool": block["name"].as_str().unwrap_or("tool"),
                                "arg": cut(&crate::nirvana_running::tool_detail(input), 200),
                                "result": "",
                                "ok": true,
                                "detail": cut(&detail, DETAIL_MOST),
                            }));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let skip = out.len().saturating_sub(MOST);
    out.split_off(skip)
}

/// The bubbles of the transcript at `path`; none when there is no file.
pub fn read(path: Option<&Path>) -> Vec<Value> {
    path.and_then(crate::nirvana_spaces::read_tail)
        .map(|tail| bubbles(&tail))
        .unwrap_or_default()
}

/// The last lines of an `agent.read` answer, trailing blanks and runs of blank lines dropped.
pub fn live(read: &Value) -> String {
    let text = read["read"]["text"].as_str().unwrap_or("");
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim_end) {
        if line.is_empty() && lines.last().is_none_or(|last| last.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    let skip = lines.len().saturating_sub(LIVE_LINES);
    lines[skip..].join("\n")
}

/// The file's length and change time, to see that it grew without reading it.
pub fn stamp(path: Option<&Path>) -> Option<(u64, std::time::SystemTime)> {
    let meta = std::fs::metadata(path?).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}
