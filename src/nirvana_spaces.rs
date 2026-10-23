//! Nirvana fork, feature 3: each space lists its agents' task and model, with the agent's icon.
//!
//! The owner, 2026-10-15: "See how in our old Nirvana we showcase the task a summerised title and its
//! active model ... for herder it would be the model provider and tyle ie Claude Sonnet. ... Basically I
//! would to have spaces do somethign similar and on the left Instead of a green circle its the agents
//! Icon. We need one for claude antigravity codex and local." (nirvana/plans/2026-10-15-03-space-agents.md)
//!
//! This file holds what both halves share: the kinds, their icons, the labels, and the server's model
//! readers. The server reads each agent's model off its main loop (a cache and one worker thread) and
//! sends it as two entries in the agent's existing `tokens`, so herdr's wire format is unchanged. The
//! client draws the rows (`client/shell/nirvana_spaces.rs`).

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The agent token holding the provider and model, e.g. "Claude Sonnet 5.5".
pub const MODEL_TOKEN: &str = "nirvana_model";
/// The agent token holding a Claude's transcript path (feature 6: the Running panel reads it). The path
/// stays the same for a whole conversation, so the window reads the file itself, as often as it likes,
/// instead of waiting for the server to send a new snapshot.
pub const TRANSCRIPT_TOKEN: &str = "nirvana_transcript";
/// The agent token holding the kind: "claude", "codex", "antigravity" or "local".
pub const KIND_TOKEN: &str = "nirvana_kind";
/// The agent token marking which Claude is the space's Nirvana Claude (plan 26 §4).
pub const ROLE_TOKEN: &str = "nirvana_role";
/// How old a cached model may get before the worker reads it again. A `/model` switch shows within this.
pub const REFRESH: Duration = Duration::from_secs(3);
/// An entry nobody asked for in this long belongs to a pane that closed.
const FORGET: Duration = Duration::from_secs(60);
/// How much of a transcript or log is read, from its end. The last reply is near the end.
const TAIL_BYTES: u64 = 256 * 1024;
/// How many recent Codex rollouts are opened to find the pane's own.
const CODEX_ROLLOUTS_SCANNED: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Claude,
    Codex,
    Antigravity,
    /// Claude Code driven against a local model (Ollama's Anthropic endpoint), the way Nirvana's local
    /// seat runs: a Claude whose model is not an Anthropic model.
    Local,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Antigravity => "antigravity",
            Self::Local => "local",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "antigravity" => Some(Self::Antigravity),
            "local" => Some(Self::Local),
            _ => None,
        }
    }

    /// The kind herdr's own agent label implies, for a server that sent no kind token.
    pub fn from_agent_label(label: &str) -> Option<Self> {
        match crate::detect::parse_agent_label(label)? {
            crate::detect::Agent::Claude => Some(Self::Claude),
            crate::detect::Agent::Codex => Some(Self::Codex),
            crate::detect::Agent::Antigravity => Some(Self::Antigravity),
            _ => None,
        }
    }

    /// One cell wide each, so the rows line up: Claude Code's own glyph, a four-part knot for Codex, a peak
    /// for Antigravity, a house for a model on this machine. None may look like herdr's status marks: the
    /// first live run (2026-10-15) drew Codex as a hexagon, ⬡, which at sidebar size read as the idle ring ○.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Claude => "\u{273B}",
            Self::Codex => "\u{2756}",
            Self::Antigravity => "\u{25B2}",
            Self::Local => "\u{2302}",
        }
    }

    /// The label when the model is not known yet.
    pub fn provider(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
            Self::Local => "Local",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentModel {
    pub kind: Kind,
    pub label: String,
    /// Feature 6: a Claude's transcript, which the window's Running panel reads.
    pub transcript: Option<PathBuf>,
    /// Plan 26 §4: marks which Claude is Nirvana Claude ("nirvana").
    pub role: Option<String>,
}

impl AgentModel {
    pub fn tokens(&self) -> Vec<(String, String)> {
        let mut tokens = vec![
            (KIND_TOKEN.to_string(), self.kind.as_str().to_string()),
            (MODEL_TOKEN.to_string(), self.label.clone()),
        ];
        if let Some(transcript) = &self.transcript {
            tokens.push((
                TRANSCRIPT_TOKEN.to_string(),
                transcript.display().to_string(),
            ));
        }
        if let Some(role) = &self.role {
            tokens.push((ROLE_TOKEN.to_string(), role.clone()));
        }
        tokens
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Claude Code's model aliases (`--model sonnet`).
const CLAUDE_FAMILY_ALIASES: &[&str] = &["sonnet", "opus", "haiku", "fable"];
const CLAUDE_GENERIC_ALIASES: &[&str] = &["default", "best", "opusplan", "<synthetic>"];

/// "claude-sonnet-5-5" -> "Claude Sonnet 5.5"; "claude-3-5-sonnet-20241022" -> "Claude Sonnet 3.5";
/// "claude-opus-5-5[1m]" -> "Claude Opus 5.5"; "sonnet" -> "Claude Sonnet". None: not an Anthropic name.
pub fn claude_label(model: &str) -> Option<String> {
    let model = model.trim();
    let model = model.split('[').next().unwrap_or(model);
    if CLAUDE_FAMILY_ALIASES.contains(&model) {
        return Some(format!("Claude {}", title_case(model)));
    }
    let rest = model.strip_prefix("claude-")?;
    let mut family = Vec::new();
    let mut version = Vec::new();
    for part in rest.split('-').filter(|part| !part.is_empty()) {
        if part.bytes().all(|b| b.is_ascii_digit()) {
            // An eight-digit part is a release date, not a version.
            if part.len() < 8 {
                version.push(part);
            }
        } else {
            family.push(title_case(part));
        }
    }
    if family.is_empty() {
        return None;
    }
    let mut label = format!("Claude {}", family.join(" "));
    if !version.is_empty() {
        label.push(' ');
        label.push_str(&version.join("."));
    }
    Some(label)
}

/// A Claude Code process, by the model it uses: an Anthropic model is Claude, anything else (an Ollama
/// tag such as `qwen3.6:35b-a3b`) is Local.
pub fn claude_model(model: Option<&str>) -> AgentModel {
    let model = model.map(str::trim).filter(|model| !model.is_empty());
    match model {
        None => AgentModel {
            kind: Kind::Claude,
            label: Kind::Claude.provider().into(),
            transcript: None,
            role: None,
        },
        Some(model) if CLAUDE_GENERIC_ALIASES.contains(&model) => AgentModel {
            kind: Kind::Claude,
            label: Kind::Claude.provider().into(),
            transcript: None,
            role: None,
        },
        Some(model) => match claude_label(model) {
            Some(label) => AgentModel {
                kind: Kind::Claude,
                label,
                transcript: None,
                role: None,
            },
            None => AgentModel {
                kind: Kind::Local,
                label: format!("Local {model}"),
                transcript: None,
                role: None,
            },
        },
    }
}

/// A Claude Code process with the Nirvana role token attached.
pub fn claude_nirvana_model(model: Option<&str>) -> AgentModel {
    let mut m = claude_model(model);
    m.role = Some("nirvana".to_string());
    m
}

/// "gpt-5-codex" -> "GPT-5 Codex"; "gpt-5.5" -> "Codex GPT-5.5"; none -> "Codex".
pub fn codex_model(model: Option<&str>) -> AgentModel {
    let label = match model.map(str::trim).filter(|model| !model.is_empty()) {
        None => Kind::Codex.provider().to_string(),
        Some(model) => {
            let parts: Vec<&str> = model.split('-').collect();
            let pretty = if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("gpt") {
                let mut pretty = format!("GPT-{}", parts[1]);
                for part in &parts[2..] {
                    pretty.push(' ');
                    pretty.push_str(&title_case(part));
                }
                pretty
            } else {
                model.to_string()
            };
            if pretty.to_ascii_lowercase().contains("codex") {
                pretty
            } else {
                format!("Codex {pretty}")
            }
        }
    };
    AgentModel {
        kind: Kind::Codex,
        label,
        transcript: None,
        role: None,
    }
}

/// Antigravity names its models for people already ("Gemini 3.8 Flash (High)"); none -> "Antigravity".
pub fn antigravity_model(model: Option<&str>) -> AgentModel {
    AgentModel {
        kind: Kind::Antigravity,
        label: model
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .unwrap_or(Kind::Antigravity.provider())
            .to_string(),
        transcript: None,
        role: None,
    }
}

/// `--model X`, `--model=X` or `-m X` among a process's arguments.
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

/// The value after the last `needle` in `text`, up to the next double quote, skipping empty ones.
pub fn last_quoted_after(text: &str, needle: &str) -> Option<String> {
    let mut end = text.len();
    while let Some(at) = text[..end].rfind(needle) {
        let start = at + needle.len();
        if let Some(len) = text[start..].find('"') {
            let value = &text[start..start + len];
            if !value.is_empty() && value != "<synthetic>" {
                return Some(value.to_string());
            }
        }
        end = at;
    }
    None
}

/// The last `TAIL_BYTES` of a file as text; a cut UTF-8 character at the start is dropped.
pub fn read_tail(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Claude Code's folder name for a working folder: every character that is not a letter or digit
/// becomes `-` (`C:\Users\fahim\Desktop\Pojects\Nirvana` -> `C--Users-fahim-Desktop-Pojects-Nirvana`).
pub fn claude_project_slug(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The model of the last reply in a Claude transcript. The assistant's message opens with its model
/// (`"message":{"model":"claude-opus-5-5","id":...`, read 2026-10-15, Claude Code 2.1.300); a tool's
/// input may carry a `"model"` key of its own (the Agent tool's), which this needle never matches.
pub fn claude_transcript_model(text: &str) -> Option<String> {
    last_quoted_after(text, "\"message\":{\"model\":\"")
}

/// Antigravity writes the model it switches to into its shared log:
/// `Propagating selected model override to backend: label="Gemini 3.8 Flash (High)"` (read 2026-10-15).
pub fn antigravity_log_model(text: &str) -> Option<String> {
    last_quoted_after(text, "selected model override to backend: label=\"")
}

/// A Codex rollout's first line names its folder (`session_meta.payload.cwd`, with a `\\?\` prefix on
/// Windows); its turns name the model (`"model":"gpt-5-codex"`, read 2026-10-15, codex 0.42.0).
pub fn codex_rollout_cwd(first_line: &str) -> Option<PathBuf> {
    let value: serde_json::Value = serde_json::from_str(first_line).ok()?;
    let cwd = value.get("payload")?.get("cwd")?.as_str()?;
    Some(PathBuf::from(cwd.strip_prefix(r"\\?\").unwrap_or(cwd)))
}

pub fn same_folder(left: &Path, right: &Path) -> bool {
    let clean = |path: &Path| {
        let text = path.to_string_lossy().replace('\\', "/");
        let text = text.strip_prefix("//?/").unwrap_or(&text).to_string();
        text.trim_end_matches('/').to_lowercase()
    };
    clean(left) == clean(right)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

pub(crate) fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".codex")))
}

/// The newest rollouts first, from the newest day folders (`sessions/YYYY/MM/DD`).
pub(crate) fn recent_codex_rollouts(sessions: &Path) -> Vec<PathBuf> {
    fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect();
        dirs.sort();
        dirs.reverse();
        dirs
    }
    let mut files = Vec::new();
    'days: for year in sorted_dirs(sessions) {
        for month in sorted_dirs(&year) {
            for day in sorted_dirs(&month) {
                let mut day_files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&day)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
                    .filter_map(|path| Some((path.metadata().ok()?.modified().ok()?, path)))
                    .collect();
                day_files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
                files.extend(day_files.into_iter().map(|(_, path)| path));
                if files.len() >= CODEX_ROLLOUTS_SCANNED {
                    break 'days;
                }
            }
        }
    }
    files.truncate(CODEX_ROLLOUTS_SCANNED);
    files
}

fn codex_rollout_model(cwd: &Path) -> Option<String> {
    let sessions = codex_home()?.join("sessions");
    for path in recent_codex_rollouts(&sessions) {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut first = String::new();
        if std::io::BufRead::read_line(&mut std::io::BufReader::new(file), &mut first).is_err() {
            continue;
        }
        if codex_rollout_cwd(&first).is_some_and(|folder| same_folder(&folder, cwd)) {
            return read_tail(&path).and_then(|text| last_quoted_after(&text, "\"model\":\""));
        }
    }
    None
}

fn codex_config_model() -> Option<String> {
    let text = std::fs::read_to_string(codex_home()?.join("config.toml")).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    value.get("model")?.as_str().map(str::to_string)
}

/// Whether an agent's process arguments indicate it is Nirvana Claude (plan 26 B3).
pub fn is_nirvana_claude(args: &[String]) -> bool {
    let plugin_dir = crate::nirvana::home().map(|h| crate::nirvana_launch::plugin_dir(&h));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    crate::nirvana_run::is_nirvana_claude_argv(&refs, plugin_dir.as_deref())
}

/// The short model representation (plan 26 S1).
/// - Claude: family initial and version, `S5.5`, `O5.5`, `H5.5`, `F5.1`.
/// - Gemini: `Flash` or `Pro`.
/// - Codex: `Codex`.
/// - A local model: its name, cut to 6 cells.
pub fn compact_model(model: &str) -> String {
    let trimmed = model.trim();
    let lower = trimmed.to_lowercase();

    // 1. Claude: family initial and version (e.g. S5.5, O5.5, H5.5, F5.1)
    if lower.contains("sonnet")
        || lower.contains("opus")
        || lower.contains("haiku")
        || lower.contains("fable")
        || trimmed.starts_with("Claude")
    {
        let family_initial = if lower.contains("sonnet") {
            'S'
        } else if lower.contains("opus") {
            'O'
        } else if lower.contains("haiku") {
            'H'
        } else if lower.contains("fable")
            || (trimmed.starts_with("Claude") && lower.contains("flash"))
        {
            'F'
        } else {
            'C'
        };
        // Extract version: e.g. "5.5", "3.5", "5.1", "3.7"
        let ver = trimmed
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .find(|s| !s.is_empty() && s.chars().any(|c| c.is_ascii_digit()));
        return match ver {
            Some(v) => format!("{family_initial}{v}"),
            None => family_initial.to_string(),
        };
    }

    // 2. Gemini: Flash or Pro
    if lower.contains("flash") {
        return "Flash".to_string();
    }
    if lower.contains("pro") {
        return "Pro".to_string();
    }

    // 3. Codex: Codex
    if lower.contains("codex") {
        return "Codex".to_string();
    }

    // 4. Local model: its name, cut to 6 cells
    let name = trimmed.strip_prefix("Local ").unwrap_or(trimmed);
    let mut out = String::new();
    let mut width = 0;
    for ch in name.chars() {
        let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + w > 6 {
            break;
        }
        out.push(ch);
        width += w;
    }
    out
}

/// The agent in a pane's shell and its model, read now. Runs on the worker thread only.
fn probe(shell_pid: u32, pane_cwd: Option<&str>) -> Option<AgentModel> {
    let job = crate::detect::foreground_job(shell_pid)?;
    let (agent, _, pid) = crate::detect::identify_agent_process_in_job(&job)?;
    let args: Vec<String> = job
        .processes
        .iter()
        .find(|process| process.pid == pid)
        .and_then(|process| process.argv.clone())
        .map(|argv| argv.into_iter().skip(1).collect())
        .unwrap_or_default();
    let from_args = argv_model(&args);
    match agent {
        crate::detect::Agent::Claude => {
            let path = crate::nirvana::claude_config_dir().and_then(|config| {
                let session = crate::nirvana::read_claude_session(&config, pid)?;
                let cwd = session.cwd.or_else(|| pane_cwd.map(PathBuf::from))?;
                Some(
                    config
                        .join("projects")
                        .join(claude_project_slug(&cwd))
                        .join(format!("{}.jsonl", session.session_id)),
                )
            });
            let transcript = path
                .as_deref()
                .and_then(read_tail)
                .and_then(|text| claude_transcript_model(&text));
            let mut model = if is_nirvana_claude(&args) {
                claude_nirvana_model(transcript.or(from_args).as_deref())
            } else {
                claude_model(transcript.or(from_args).as_deref())
            };
            model.transcript = path;
            Some(model)
        }
        crate::detect::Agent::Codex => {
            let model = from_args
                .or_else(|| pane_cwd.and_then(|cwd| codex_rollout_model(Path::new(cwd))))
                .or_else(codex_config_model);
            Some(codex_model(model.as_deref()))
        }
        crate::detect::Agent::Antigravity => {
            let model = from_args.or_else(|| {
                let log = home_dir()?
                    .join(".gemini")
                    .join("antigravity-cli")
                    .join("cli.log");
                read_tail(&log).and_then(|text| antigravity_log_model(&text))
            });
            Some(antigravity_model(model.as_deref()))
        }
        _ => None,
    }
}

/// What the worker uses to make the server draw again once a model changed.
#[derive(Clone)]
pub struct Waker {
    pub dirty: Arc<crate::render_signal::RenderSignal>,
    pub notify: Arc<tokio::sync::Notify>,
}

struct Entry {
    model: Option<AgentModel>,
    checked: Option<Instant>,
    pending: bool,
    asked: Instant,
}

struct Request {
    shell_pid: u32,
    cwd: Option<String>,
    waker: Waker,
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
            crate::thread_spawn::spawn_named("nirvana-spaces", move || {
                for request in receiver {
                    let model = probe(request.shell_pid, request.cwd.as_deref());
                    let changed = {
                        let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
                        let Some(entry) = cache.get_mut(&request.shell_pid) else {
                            continue;
                        };
                        let changed = entry.model != model;
                        entry.model = model;
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

/// The model last read for the agent under this pane shell, and a fresh read queued when it is older
/// than `REFRESH`. Never blocks on a file or a process read: the server's loop calls this while it
/// builds each client's snapshot.
pub fn cached_model(shell_pid: u32, cwd: Option<&str>, waker: &Waker) -> Option<AgentModel> {
    let now = Instant::now();
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, entry| now.duration_since(entry.asked) < FORGET);
    let entry = cache.entry(shell_pid).or_insert(Entry {
        model: None,
        checked: None,
        pending: false,
        asked: now,
    });
    entry.asked = now;
    let stale = entry
        .checked
        .is_none_or(|checked| now.duration_since(checked) >= REFRESH);
    if stale && !entry.pending {
        if let Some(worker) = worker() {
            entry.pending = worker
                .send(Request {
                    shell_pid,
                    cwd: cwd.map(str::to_owned),
                    waker: waker.clone(),
                })
                .is_ok();
        }
    }
    entry.model.clone()
}

/// The task row's text. Claude Code titles its terminal with a short summary of the conversation; a
/// title that only names the program or the shell means nothing was asked yet, in the old Nirvana's
/// words "No task yet".
pub fn task_title(title: Option<&str>) -> String {
    const NO_TASK: &str = "No task yet";
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return NO_TASK.into();
    };
    // Claude Code puts its state in front of its title: "✳ Button colour question" while idle or
    // waiting, a spinner glyph while working (measured 2026-10-16). The mark is not the task.
    let title = match title.split_once(' ') {
        Some((mark, rest))
            if mark.chars().count() == 1 && !mark.chars().all(char::is_alphanumeric) =>
        {
            rest.trim()
        }
        _ => title,
    };
    if title.is_empty() {
        return NO_TASK.into();
    }
    let lower = title.to_lowercase();
    let program_only = [
        "claude code",
        "claude",
        "codex",
        "agy",
        "antigravity",
        "antigravity cli",
    ]
    .contains(&lower.as_str());
    let shell_path = lower.ends_with(".exe")
        || title.contains('\\')
        || title.starts_with('/')
        || title.starts_with('~');
    if program_only || shell_path {
        NO_TASK.into()
    } else {
        title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claudes_transcript_rides_in_the_agent_tokens() {
        let mut model = claude_model(Some("claude-opus-5-5"));
        assert_eq!(model.tokens().len(), 2);
        model.transcript = Some(PathBuf::from(r"C:\c\projects\p\s.jsonl"));
        assert!(model.tokens().contains(&(
            TRANSCRIPT_TOKEN.to_string(),
            r"C:\c\projects\p\s.jsonl".to_string()
        )));
    }

    #[test]
    fn claudes_state_mark_is_not_part_of_the_task() {
        assert_eq!(
            task_title(Some("\u{2733} Button colour question")),
            "Button colour question"
        );
        assert_eq!(
            task_title(Some("\u{25D1} Fixing the tests")),
            "Fixing the tests"
        );
        assert_eq!(task_title(Some("A short task")), "A short task");
        assert_eq!(task_title(Some("\u{2733} Claude Code")), "No task yet");
    }

    #[test]
    fn claude_models_read_as_provider_family_and_version() {
        assert_eq!(
            claude_label("claude-sonnet-5-5").as_deref(),
            Some("Claude Sonnet 5.5")
        );
        assert_eq!(
            claude_label("claude-opus-5-5[1m]").as_deref(),
            Some("Claude Opus 5.5")
        );
        assert_eq!(
            claude_label("claude-fable-5-1").as_deref(),
            Some("Claude Fable 5.1")
        );
        assert_eq!(
            claude_label("claude-opus-4-1-20250805").as_deref(),
            Some("Claude Opus 4.1")
        );
        assert_eq!(
            claude_label("claude-3-5-sonnet-20241022").as_deref(),
            Some("Claude Sonnet 3.5")
        );
        assert_eq!(claude_label("sonnet").as_deref(), Some("Claude Sonnet"));
        assert_eq!(claude_label("qwen3.6:35b-a3b"), None);
    }

    #[test]
    fn a_claude_on_a_local_model_is_local() {
        assert_eq!(
            claude_model(Some("qwen3.6:35b-a3b")),
            AgentModel {
                kind: Kind::Local,
                label: "Local qwen3.6:35b-a3b".into(),
                transcript: None,
                role: None,
            }
        );
        assert_eq!(claude_model(Some("claude-haiku-5-5")).kind, Kind::Claude);
        assert_eq!(claude_model(Some("default")).label, "Claude");
        assert_eq!(claude_model(None).label, "Claude");
    }

    #[test]
    fn codex_and_antigravity_models_read_as_people_name_them() {
        assert_eq!(codex_model(Some("gpt-5-codex")).label, "GPT-5 Codex");
        assert_eq!(codex_model(Some("gpt-5.5")).label, "Codex GPT-5.5");
        assert_eq!(codex_model(None).label, "Codex");
        assert_eq!(
            antigravity_model(Some("Gemini 3.8 Flash (High)")).label,
            "Gemini 3.8 Flash (High)"
        );
        assert_eq!(antigravity_model(None).label, "Antigravity");
    }

    #[test]
    fn each_kind_has_its_own_one_cell_icon() {
        let kinds = [Kind::Claude, Kind::Codex, Kind::Antigravity, Kind::Local];
        let icons: std::collections::HashSet<_> = kinds.iter().map(|kind| kind.icon()).collect();
        assert_eq!(icons.len(), kinds.len());
        for kind in kinds {
            assert_eq!(
                unicode_width::UnicodeWidthStr::width(kind.icon()),
                1,
                "{kind:?}"
            );
            // herdr's status marks, dots and symbols (client/shell.rs status_icon).
            for status in [
                "\u{25CF}", "\u{25CB}", "\u{00B7}", "\u{00D7}", "\u{25D0}", "\u{2713}",
            ] {
                assert_ne!(
                    kind.icon(),
                    status,
                    "{kind:?} must not look like a status mark"
                );
            }
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::from_agent_label("claude"), Some(Kind::Claude));
        assert_eq!(Kind::from_agent_label("agy"), Some(Kind::Antigravity));
        assert_eq!(Kind::from_agent_label("pi"), None);
    }

    #[test]
    fn the_model_comes_from_the_last_reply_not_a_tool_input() {
        let transcript = concat!(
            r#"{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_1"}}"#,
            "\n",
            r#"{"type":"assistant","message":{"model":"claude-sonnet-5-5","id":"msg_2","content":[{"type":"tool_use","input":{"model":"haiku"}}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"model":"<synthetic>","id":"msg_3"}}"#,
            "\n",
        );
        assert_eq!(
            claude_transcript_model(transcript).as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert_eq!(claude_transcript_model("{}"), None);
    }

    #[test]
    fn agy_and_codex_files_name_their_models() {
        let log = concat!(
            "I1015 model_config_manager.go:340] Propagating selected model override to backend: label=\"Gemini 3.8 Pro\"\n",
            "I1015 model_config_manager.go:340] Propagating selected model override to backend: label=\"Gemini 3.8 Flash (High)\"\n",
        );
        assert_eq!(
            antigravity_log_model(log).as_deref(),
            Some("Gemini 3.8 Flash (High)")
        );
        let meta =
            r#"{"type":"session_meta","payload":{"cwd":"\\\\?\\C:\\Users\\me\\proj","id":"x"}}"#;
        let cwd = codex_rollout_cwd(meta).unwrap();
        assert_eq!(cwd, PathBuf::from(r"C:\Users\me\proj"));
        assert!(same_folder(&cwd, Path::new(r"C:\Users\me\proj\")));
        assert!(!same_folder(&cwd, Path::new(r"C:\Users\me\other")));
    }

    #[test]
    fn the_transcript_folder_is_claude_codes_slug() {
        assert_eq!(
            claude_project_slug(Path::new(r"C:\Users\fahim\Desktop\Pojects\Nirvana")),
            "C--Users-fahim-Desktop-Pojects-Nirvana"
        );
        assert_eq!(
            claude_project_slug(Path::new("/home/me/a.b")),
            "-home-me-a-b"
        );
    }

    #[test]
    fn model_flags_are_read_from_the_arguments() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            argv_model(&args(&["--settings", "{}", "--model", "opus"])).as_deref(),
            Some("opus")
        );
        assert_eq!(
            argv_model(&args(&["--model=gpt-5.5"])).as_deref(),
            Some("gpt-5.5")
        );
        assert_eq!(argv_model(&args(&["-m", "o3"])).as_deref(), Some("o3"));
        assert_eq!(argv_model(&args(&["--model", "--resume"])), None);
        assert_eq!(argv_model(&args(&["--resume", "x"])), None);
    }

    #[test]
    fn a_title_that_only_names_the_program_is_no_task_yet() {
        assert_eq!(
            task_title(Some("Sort the clocked-in list")),
            "Sort the clocked-in list"
        );
        assert_eq!(task_title(Some("Claude Code")), "No task yet");
        assert_eq!(
            task_title(Some(r"C:\Program Files\PowerShell\7\pwsh.exe")),
            "No task yet"
        );
        assert_eq!(task_title(None), "No task yet");
    }

    #[test]
    fn the_tail_reader_keeps_the_end_of_a_long_file() {
        let dir = std::env::temp_dir().join(format!("nirvana-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.jsonl");
        let mut text = "x".repeat(TAIL_BYTES as usize);
        text.push_str(r#"{"message":{"model":"claude-sonnet-5-5","id":"m"}}"#);
        std::fs::write(&path, &text).unwrap();
        let tail = read_tail(&path).unwrap();
        assert_eq!(tail.len() as u64, TAIL_BYTES);
        assert_eq!(
            claude_transcript_model(&tail).as_deref(),
            Some("claude-sonnet-5-5")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
