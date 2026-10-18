//! Nirvana fork, feature 9: level 7 of ten-levels-of-jev, "Should I Compact", as a Claude Code Stop hook.
//!
//! The port of Nirvana's `integrations/claude-code/compact/` (alpha 0.21.0), which was built, shadow tested
//! on synthetic text, and never installed. After each answer the hook reads the context size from the end
//! of the transcript. Under the first line it does nothing. At or above one, it asks Jev four questions in
//! one call (did the work switch gears, did the turn finish a unit of work, how much earlier work does the
//! next step need, is a multi-file change in flight) and, when the moment is good, shows the owner one
//! line: a Stop hook's `systemMessage`, which Nirvana measured reaching the owner as a notice and never
//! the model (a Stop hook's `additionalContext` made the model answer again, so it is not used).
//!
//! A hook cannot start a compaction, so it only tells. The numbers are code; the judgment is Jev's. A Jev
//! failure is never a yes: at the request line it says the plain line with the number, and below it says
//! nothing.
//!
//! **The lines** are Nirvana's: 150k, 300k and 500k tokens, set from the owner's own Claude Code
//! transcripts (13,208 calls: p10 146,059, median 311,688, p90 545,659). They are hypotheses until the
//! log shows where the owner compacts. `NIRVANA_JEV_COMPACT_LINES=notice,recommend,request` overrides them.
//!
//! **What leaves the machine** on a call: the newest request and up to three earlier ones, cut to 600
//! characters, the end of the last reply, cut to 1,500, and the names of the tools the turn used, with
//! secret shapes redacted; never a tool's input or output. Lines Claude Code writes to itself (`!` shell
//! commands, slash commands, reminders) are never a request.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::nirvana_jev::{redact, Client};

const LINES: [u64; 3] = [150_000, 300_000, 500_000];
const LEVELS: [&str; 3] = ["notice", "recommend", "request"];
const COOLDOWN_TURNS: u64 = 8;
/// Nirvana's 3 is cut to 1: live on 2026-10-16 a silent reading on turn 1 rested turns 2 and 3, and
/// turn 3 was the clean break to new work the hook exists to catch (`scratchpad/f9/compact_live.py`).
const SILENT_REST_TURNS: u64 = 1;
const CHUNK: u64 = 128 * 1024;
const LIMIT: u64 = 4 * 1024 * 1024;
const PROMPTS: usize = 4;
const PROMPT_CHARS: usize = 600;
const REPLY_CHARS: usize = 1500;
const MAX_STATE_CHARS: usize = 4000;
/// Nirvana's floors, checked once on synthetic text (`compact.json` provenance): hypotheses.
const FLOOR_SWITCHED: f64 = 0.7;
const FLOOR_BOUNDARY: f64 = 0.6;
const FLOOR_MID_EDIT: f64 = 0.7;
const FLOOR_HISTORY_LOW: f64 = 0.25;
const CLI_TAGS: [&str; 5] = [
    "<bash-",
    "<local-command-",
    "<command-",
    "<task-notification",
    "<system-reminder",
];

pub fn lines() -> [u64; 3] {
    std::env::var("NIRVANA_JEV_COMPACT_LINES")
        .ok()
        .and_then(|text| {
            let numbers: Vec<u64> = text
                .split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            (numbers.len() == 3 && numbers[0] < numbers[1] && numbers[1] < numbers[2])
                .then(|| [numbers[0], numbers[1], numbers[2]])
        })
        .unwrap_or(LINES)
}

/// The highest line the context has reached: 0 notice, 1 recommend, 2 request.
pub fn level_for(tokens: u64, lines: [u64; 3]) -> Option<usize> {
    lines.iter().rposition(|line| tokens >= *line)
}

// ---- the transcript ----------------------------------------------------------------------------------

/// The transcript's lines, newest first, read in chunks from the end and never past `limit` bytes.
fn tail_lines(path: &Path, limit: u64) -> Vec<String> {
    let Ok(mut file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let Ok(mut pos) = file.seek(SeekFrom::End(0)) else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    let mut carry: Vec<u8> = Vec::new();
    let mut read = 0;
    while pos > 0 && read < limit {
        let n = CHUNK.min(pos).min(limit - read);
        pos -= n;
        let mut data = vec![0; n as usize];
        if file.seek(SeekFrom::Start(pos)).is_err() || file.read_exact(&mut data).is_err() {
            break;
        }
        read += n;
        data.extend_from_slice(&carry);
        let mut parts: Vec<&[u8]> = data.split(|b| *b == b'\n').collect();
        carry = if pos > 0 {
            parts.remove(0).to_vec()
        } else {
            Vec::new()
        };
        for part in parts.into_iter().rev() {
            if !part.iter().all(u8::is_ascii_whitespace) {
                lines.push(String::from_utf8_lossy(part).into_owned());
            }
        }
    }
    lines
}

/// The context size: what the last main-thread assistant call sent. `None` when a compaction came after it.
pub fn context_tokens(lines: &[String]) -> Option<u64> {
    for line in lines {
        if line.contains("compact_boundary") {
            let value: Value = serde_json::from_str(line).unwrap_or(Value::Null);
            if value["type"] == "system" && value["subtype"] == "compact_boundary" {
                return None;
            }
        }
        if !line.contains("\"input_tokens\"") {
            continue;
        }
        let value: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        if value["type"] != "assistant"
            || value["isSidechain"] == true
            || value["message"]["model"] == "<synthetic>"
        {
            continue;
        }
        let usage = &value["message"]["usage"];
        let total = [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
        ]
        .iter()
        .map(|k| usage[*k].as_u64().unwrap_or(0))
        .sum::<u64>();
        if total > 0 {
            return Some(total);
        }
    }
    None
}

fn text_of(content: &Value, requests_only: bool) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .filter(|t| !(requests_only && is_cli(t)))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned(),
        _ => String::new(),
    }
}

fn is_cli(text: &str) -> bool {
    let text = text.trim_start();
    CLI_TAGS.iter().any(|tag| text.starts_with(tag))
}

/// The owner's real requests, oldest first, the last reply, and the tools the newest request's turn used.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recent {
    pub prompts: Vec<String>,
    pub reply: String,
    pub tools: BTreeMap<String, u64>,
}

pub fn recent(lines: &[String]) -> Recent {
    let mut out = Recent::default();
    let mut in_turn = true;
    for line in lines {
        let user = line.contains("\"user\"");
        if !user && !line.contains("\"assistant\"") {
            continue;
        }
        if user
            && line
                .get(..2000)
                .unwrap_or(line)
                .contains("[{\"type\":\"tool_result\"")
        {
            continue;
        }
        let value: Value = serde_json::from_str(line).unwrap_or(Value::Null);
        if value["isSidechain"] == true {
            continue;
        }
        let content = &value["message"]["content"];
        if value["type"] == "assistant" {
            if out.reply.is_empty() {
                out.reply = text_of(content, false);
            }
            if in_turn {
                for block in content.as_array().into_iter().flatten() {
                    if block["type"] == "tool_use" {
                        if let Some(name) = block["name"].as_str() {
                            *out.tools.entry(name.to_owned()).or_default() += 1;
                        }
                    }
                }
            }
            continue;
        }
        if value["type"] != "user" || value["isMeta"] == true || value["isCompactSummary"] == true {
            continue;
        }
        if content
            .as_array()
            .is_some_and(|blocks| blocks.iter().any(|b| b["type"] == "tool_result"))
        {
            continue;
        }
        let text = text_of(content, true);
        if text.trim().is_empty() || is_cli(&text) {
            continue;
        }
        in_turn = false;
        out.prompts.push(text);
        if out.prompts.len() >= PROMPTS {
            break;
        }
    }
    out.prompts.reverse();
    out
}

// ---- the state and the questions ---------------------------------------------------------------------

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn head(text: &str, n: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= n {
        text.to_owned()
    } else {
        chars[..n - 1].iter().collect::<String>() + "…"
    }
}

fn tail(text: &str, n: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= n {
        text.to_owned()
    } else {
        "…".to_owned() + &chars[chars.len() - (n - 1)..].iter().collect::<String>()
    }
}

/// What leaves the machine, every string redacted before it is cut, the whole at most 4,000 characters.
pub fn state(recent: &Recent) -> (Value, usize) {
    let mut redacted = 0;
    let mut clean = |text: &str| {
        let (text, n) = redact(&squash(text), None);
        redacted += n;
        text
    };
    let (latest, earlier) = recent
        .prompts
        .split_last()
        .map_or((String::new(), &[][..]), |(l, e)| (l.clone(), e));
    let mut earlier: Vec<String> = earlier
        .iter()
        .map(|p| head(&clean(p), PROMPT_CHARS))
        .collect();
    let mut latest = head(&clean(&latest), PROMPT_CHARS);
    let mut reply = tail(&clean(&recent.reply), REPLY_CHARS);
    let build = |earlier: &Vec<String>, latest: &String, reply: &String| json!({"earlier_requests": earlier, "latest_request": latest, "last_reply": reply, "last_turn_tools": recent.tools});
    let mut value = build(&earlier, &latest, &reply);
    while value.to_string().chars().count() > MAX_STATE_CHARS {
        let reply_len = reply.chars().count();
        if reply_len > 200 {
            reply = tail(&reply, (reply_len * 7 / 10).max(200));
        } else if !earlier.is_empty() {
            earlier.remove(0);
        } else if latest.chars().count() > 120 {
            latest = head(&latest, (latest.chars().count() * 7 / 10).max(120));
        } else {
            break;
        }
        value = build(&earlier, &latest, &reply);
    }
    (value, redacted)
}

fn questions() -> Map<String, Value> {
    let noul = |instructions: &str, yes: &str, no: &str| json!({"type": "noul", "instructions": instructions, "criteria": {"true": yes, "false": no}});
    let mut q = Map::new();
    q.insert("switched_gears".into(), noul(
        "latest_request is what the owner just asked. earlier_requests are what they asked before it, oldest first. Is latest_request about different work than earlier_requests, so that doing it does not need the earlier work? With no earlier_requests, the answer is false.",
        "It starts different work", "It is the same work or its direct follow-on"));
    q.insert("at_boundary".into(), noul(
        "last_reply is the agent's most recent reply. Does it end a unit of work: what it was doing is done, and nothing is left half finished or waiting on the owner?",
        "A unit of work is complete", "Work is unfinished or waiting"));
    q.insert("needs_history".into(), json!({"type": "score",
        "instructions": "How much of the work in earlier_requests would the owner's likely next step need to still have in view?",
        "criteria": ["Almost none of it", "Some of it", "Most of it"]}));
    q.insert("mid_edit".into(), noul(
        "last_turn_tools counts the tools the last turn used. Is the agent in the middle of a change that spans several files: edits were made and last_reply says more edits, fixes, tests or files are still to come?",
        "A multi-file change is in flight", "No change is in flight"));
    q
}

/// Jev's four answers, read against the floors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub switched: bool,
    pub boundary: bool,
    pub mid_edit: bool,
    pub history_low: bool,
}

pub fn read_answers(answers: &Map<String, Value>) -> Reading {
    let p = |id: &str| answers[id]["noul"].as_f64().unwrap_or(0.0);
    let probs = answers["needs_history"]["probabilities"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut keys: Vec<(f64, f64)> = probs
        .iter()
        .filter_map(|(k, v)| Some((k.parse::<f64>().ok()?, v.as_f64()?)))
        .collect();
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    let position = keys
        .iter()
        .enumerate()
        .map(|(i, (_, p))| p * i as f64)
        .sum::<f64>()
        / (keys.len().max(2) - 1) as f64;
    Reading {
        switched: p("switched_gears") >= FLOOR_SWITCHED,
        boundary: p("at_boundary") >= FLOOR_BOUNDARY,
        mid_edit: p("mid_edit") >= FLOOR_MID_EDIT,
        history_low: position <= FLOOR_HISTORY_LOW,
    }
}

fn short(tokens: u64) -> String {
    if tokens >= 999_500 {
        format!("{:.1}M", tokens as f64 / 1e6)
    } else {
        format!("{}k", (tokens as f64 / 1000.0).round() as u64)
    }
}

/// THE POLICY, pure: the level reached, the size, Jev's reading (`None`: no answer) to the line to say.
/// Nirvana's `cc_policy.decide`, unchanged.
pub fn decide(
    level: usize,
    tokens: u64,
    reading: Option<Reading>,
    request_at: u64,
) -> (Option<String>, &'static str) {
    let (k, r) = (short(tokens), short(request_at));
    let Some(reading) = reading else {
        return if level == 2 {
            (Some(format!("Context is {k} tokens, past the {r} line: run /compact at the next stopping point.")), "no_answer")
        } else {
            (None, "no_answer")
        };
    };
    let good = reading.boundary && !reading.mid_edit;
    if level == 2 {
        return if good {
            (Some(format!("Context is {k} tokens, past the {r} line, and this is a clean break: run /compact now.")), "clean_break")
        } else {
            (Some(format!("Context is {k} tokens, past the {r} line. Finish the change in hand, then run /compact.")),
             if reading.mid_edit { "mid_edit" } else { "not_a_boundary" })
        };
    }
    if !good {
        return (
            None,
            if reading.mid_edit {
                "mid_edit"
            } else {
                "not_a_boundary"
            },
        );
    }
    if reading.switched {
        return (Some(format!("Context is {k} tokens and you just switched to new work: /compact now keeps the summary clean.")), "switched_gears");
    }
    if level == 1 && reading.history_low {
        return (Some(format!("Context is {k} tokens and you just finished a unit of work: /compact now keeps the summary clean.")), "little_history_needed");
    }
    (
        None,
        if level == 0 {
            "no_switch"
        } else {
            "history_needed"
        },
    )
}

// ---- the session's memory: the cooldown and the rest -------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Memory {
    pub turn: u64,
    /// The turn, size and level of the last line said.
    pub said: Option<(u64, u64, usize)>,
    /// The turn and level of the last call that ended silent.
    pub silent: Option<(u64, usize)>,
}

/// Whether this turn may ask Jev at `level`.
pub fn may_ask(memory: &Memory, level: usize, tokens: u64) -> Result<(), &'static str> {
    if let Some((turn, said_tokens, said_level)) = memory.said {
        let compacted = tokens * 2 < said_tokens;
        if !compacted && level <= said_level && memory.turn < turn + COOLDOWN_TURNS {
            return Err("cooldown");
        }
    }
    if let Some((turn, silent_level)) = memory.silent {
        if level <= silent_level && memory.turn <= turn + SILENT_REST_TURNS {
            return Err("resting");
        }
    }
    Ok(())
}

fn memory_path(session: &str) -> Option<PathBuf> {
    let session: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!session.is_empty()).then(|| {
        crate::nirvana::home()
            .unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
            .join("jev")
            .join("compact")
            .join(format!("{session}.json"))
    })
}

fn load(session: &str) -> Memory {
    memory_path(session)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save(session: &str, memory: &Memory) {
    if let Some(path) = memory_path(session) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, serde_json::to_string(memory).unwrap_or_default());
    }
}

// ---- the hook ----------------------------------------------------------------------------------------

/// The Stop hook, on the hook's JSON as `herdr jev compact` read it from stdin: prints `{"systemMessage": ...}` or
/// nothing. Never fails the turn: every problem prints nothing. The CLI reads stdin first, since `--plugin` (plan 12
/// O4b) looks at the input before this runs.
pub fn hook_input(input: &str) {
    let hook: Value = serde_json::from_str(input).unwrap_or(Value::Null);
    if hook.get("agent_id").is_some_and(|a| !a.is_null()) {
        return; // a subagent's context is not the one paid for turn after turn
    }
    let session = hook["session_id"].as_str().unwrap_or_default().to_owned();
    let Some(transcript) = hook["transcript_path"].as_str().map(PathBuf::from) else {
        return;
    };
    let lines_read = tail_lines(&transcript, LIMIT);
    let mut memory = load(&session);
    memory.turn += 1;
    let Some(tokens) = context_tokens(&lines_read) else {
        save(&session, &memory);
        return;
    };
    let lines = lines();
    let Some(level) = level_for(tokens, lines) else {
        save(&session, &memory);
        return;
    };
    let mut entry = json!({"ts": time_now(), "tool": "compact", "agent": "claude", "session": session,
                           "tokens": tokens, "level": LEVELS[level]});
    if let Err(why) = may_ask(&memory, level, tokens) {
        entry["outcome"] = "skipped".into();
        entry["why"] = why.into();
        append(&entry);
        save(&session, &memory);
        return;
    }
    let mut recent = recent(&lines_read);
    // Claude Code 2.1.301 hands the Stop hook the reply itself (`last_assistant_message`, probed
    // 2026-10-16); the transcript can lag it, which read a finished answer as unfinished (live, turn 1:
    // at_boundary 0.07 for a complete two-sentence summary).
    if let Some(reply) = hook["last_assistant_message"]
        .as_str()
        .filter(|r| !r.trim().is_empty())
    {
        recent.reply = reply.to_owned();
    }
    let (state, redacted) = state(&recent);
    entry["redacted"] = redacted.into();
    let reading = match Client::from_env() {
        Ok(client) => match client.ask(&state, &questions()) {
            Ok(reply) => {
                entry["input_tokens"] = reply.input_tokens.into();
                entry["cost_usd"] = reply.cost_usd.map_or(Value::Null, Value::from);
                entry["ms"] = reply.ms.into();
                let reading = read_answers(&reply.answers);
                entry["reading"] = json!({"switched": reading.switched, "boundary": reading.boundary,
                                          "mid_edit": reading.mid_edit, "history_low": reading.history_low});
                // The raw answers, so the floors can be moved from the owner's own sessions.
                entry["p"] = json!({
                    "switched_gears": reply.answers["switched_gears"]["noul"],
                    "at_boundary": reply.answers["at_boundary"]["noul"],
                    "mid_edit": reply.answers["mid_edit"]["noul"],
                    "needs_history": reply.answers["needs_history"]["probabilities"],
                });
                Some(reading)
            }
            Err(error) => {
                entry["error"] = error.kind().into();
                None
            }
        },
        Err(error) => {
            entry["error"] = error.kind().into();
            None
        }
    };
    let (line, why) = decide(level, tokens, reading, lines[2]);
    entry["why"] = why.into();
    entry["outcome"] = if reading.is_some() {
        "answered"
    } else {
        "failed"
    }
    .into();
    match &line {
        Some(text) => {
            entry["said"] = text.clone().into();
            memory.said = Some((memory.turn, tokens, level));
            println!("{}", json!({"systemMessage": format!("Jev: {text}")}));
        }
        None => memory.silent = Some((memory.turn, level)),
    }
    append(&entry);
    save(&session, &memory);
}

fn time_now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn append(entry: &Value) {
    let path = crate::nirvana::home()
        .unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
        .join("jev")
        .join("compact.jsonl");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{entry}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(v: Value) -> String {
        v.to_string()
    }

    fn assistant(tokens: u64, text: &str, tool: Option<&str>) -> String {
        let mut content = vec![json!({"type": "text", "text": text})];
        if let Some(tool) = tool {
            content.push(json!({"type": "tool_use", "name": tool, "input": {}}));
        }
        line(
            json!({"type": "assistant", "message": {"model": "claude", "content": content,
            "usage": {"input_tokens": 10, "cache_read_input_tokens": tokens - 10, "cache_creation_input_tokens": 0}}}),
        )
    }

    fn user(text: &str) -> String {
        line(json!({"type": "user", "message": {"role": "user", "content": text}}))
    }

    #[test]
    fn the_size_is_the_last_real_calls_and_a_compaction_resets_it() {
        // Newest first, as the tail reads them.
        let lines = vec![
            line(
                json!({"type": "assistant", "message": {"model": "<synthetic>", "usage": {"input_tokens": 9}}}),
            ),
            assistant(320_000, "done", None),
            assistant(100, "old", None),
        ];
        assert_eq!(context_tokens(&lines), Some(320_000));
        let compacted = vec![
            line(json!({"type": "system", "subtype": "compact_boundary"})),
            assistant(320_000, "x", None),
        ];
        assert_eq!(context_tokens(&compacted), None);
    }

    #[test]
    fn requests_are_the_owners_words_never_the_clis() {
        let lines = vec![
            assistant(
                200_000,
                "All three files are fixed and the tests pass.",
                Some("Edit"),
            ),
            line(
                json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}}),
            ),
            user("<bash-input>cat .env</bash-input>"),
            user("now fix the proration rounding"),
            user("add a login page"),
        ];
        let recent = recent(&lines);
        assert_eq!(
            recent.prompts,
            vec![
                "add a login page".to_owned(),
                "now fix the proration rounding".to_owned()
            ]
        );
        assert_eq!(
            recent.reply,
            "All three files are fixed and the tests pass."
        );
        assert_eq!(recent.tools.get("Edit"), Some(&1));
    }

    #[test]
    fn the_state_is_redacted_and_bounded() {
        let recent = Recent {
            prompts: vec![
                "x".repeat(5000),
                "use key sk-abcdefghijklmnopqrstuvwxyz123".into(),
            ],
            reply: "y".repeat(9000),
            tools: BTreeMap::new(),
        };
        let (value, redacted) = state(&recent);
        assert_eq!(redacted, 1);
        let text = value.to_string();
        assert!(!text.contains("sk-abcdefghijklmnop"));
        assert!(text.chars().count() <= MAX_STATE_CHARS);
    }

    #[test]
    fn the_policy_speaks_only_at_a_clean_break_and_always_at_the_request_line() {
        let clean = Reading {
            switched: true,
            boundary: true,
            mid_edit: false,
            history_low: false,
        };
        let busy = Reading {
            switched: true,
            boundary: true,
            mid_edit: true,
            history_low: true,
        };
        assert_eq!(decide(0, 160_000, Some(clean), 500_000).1, "switched_gears");
        assert!(decide(0, 160_000, Some(busy), 500_000).0.is_none());
        assert!(
            decide(0, 160_000, None, 500_000).0.is_none(),
            "a failure is never a yes"
        );
        let plain = decide(2, 612_000, None, 500_000).0.unwrap();
        assert_eq!(
            plain,
            "Context is 612k tokens, past the 500k line: run /compact at the next stopping point."
        );
        assert!(decide(2, 612_000, Some(busy), 500_000)
            .0
            .unwrap()
            .contains("Finish the change in hand"));
        let light = Reading {
            switched: false,
            boundary: true,
            mid_edit: false,
            history_low: true,
        };
        assert_eq!(
            decide(1, 320_000, Some(light), 500_000).1,
            "little_history_needed"
        );
        assert!(decide(0, 160_000, Some(light), 500_000).0.is_none());
    }

    #[test]
    fn it_rests_after_silence_and_cools_down_after_speaking() {
        let mut memory = Memory {
            turn: 10,
            said: Some((8, 320_000, 1)),
            silent: None,
        };
        assert_eq!(may_ask(&memory, 1, 330_000), Err("cooldown"));
        assert_eq!(
            may_ask(&memory, 2, 510_000),
            Ok(()),
            "a higher line breaks the cooldown"
        );
        assert_eq!(
            may_ask(&memory, 1, 100_000),
            Ok(()),
            "a compaction resets it"
        );
        memory = Memory {
            turn: 11,
            said: None,
            silent: Some((10, 0)),
        };
        assert_eq!(may_ask(&memory, 0, 160_000), Err("resting"));
        assert_eq!(may_ask(&memory, 1, 310_000), Ok(()));
        memory.turn = 12;
        assert_eq!(
            may_ask(&memory, 0, 160_000),
            Ok(()),
            "one turn of rest, then ask again"
        );
    }

    #[test]
    fn the_history_reading_is_the_expected_position() {
        let answers: Map<String, Value> = serde_json::from_value(json!({
            "switched_gears": {"noul": 0.9}, "at_boundary": {"noul": 0.8}, "mid_edit": {"noul": 0.1},
            "needs_history": {"probabilities": {"0": 0.9, "1": 0.08, "2": 0.02}}})).unwrap();
        let reading = read_answers(&answers);
        assert!(reading.switched && reading.boundary && !reading.mid_edit && reading.history_low);
    }

    #[test]
    fn the_lines_come_from_the_environment_only_when_they_are_ordered() {
        assert_eq!(level_for(149_999, LINES), None);
        assert_eq!(level_for(150_000, LINES), Some(0));
        assert_eq!(level_for(600_000, LINES), Some(2));
    }
}
