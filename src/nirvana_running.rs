//! Nirvana fork, feature 6: the Running square's two halves, what a Claude is doing now and what it is
//! waiting on the user for.
//!
//! The owner, 2026-10-16: "Marge the running and waiting on you into one panel the top being running and
//! what is actively being run with a summary. Then underneath that running is the waiting on you."
//! (nirvana/plans/2026-10-16-06-running-and-waiting.md)
//!
//! Measured 2026-10-16 with Claude Code 2.1.301 in herdr ("Sonnet - measure Claude waiting prompts in
//! herdr"):
//! - **Running** comes from the transcript. A tool call is written as soon as Claude makes it (a Bash
//!   call was on disk while its permission dialog was open), so "a tool call with no result yet" is what
//!   runs now; the last user prompt is what was asked, the last assistant text what Claude last said.
//! - **Waiting on you** comes from the pane's own screen. An AskUserQuestion call reached the transcript
//!   only after it was answered, and a permission dialog's options change with the case ("Yes, and always
//!   allow access to <folder>", "switch to auto mode"), so only the screen says what the numbers mean.
//!   herdr marks the agent `blocked` within a second of the prompt; one digit key answers it, no Enter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

/// A transcript is read again after this long, while the strip shows its agent.
pub const REREAD: Duration = Duration::from_secs(2);

/// A tool call in flight.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    /// The call's own words for itself: its description, else its command, path, pattern or query.
    pub detail: String,
    /// Unix seconds the call was made.
    pub since: Option<u64>,
}

/// What a Claude's transcript says about its current turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// The last prompt the user typed, first line.
    pub asked: Option<String>,
    /// Unix seconds the last prompt was typed: the turn's start.
    pub asked_at: Option<u64>,
    /// The last text Claude wrote.
    pub said: Option<String>,
    /// The last tool call with no result yet.
    pub now: Option<Tool>,
}

/// "2026-10-16T05:36:56.377Z" as unix seconds.
pub fn unix_seconds(timestamp: &str) -> Option<u64> {
    let (date, time) = timestamp.split_once('T')?;
    let mut date = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (date.next()?.ok()?, date.next()?.ok()?, date.next()?.ok()?);
    let mut time = time.trim_end_matches('Z').split(':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: f64 = time.next()?.parse().ok()?;
    // Days from the civil date (Howard Hinnant's algorithm).
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    #[allow(clippy::cast_possible_truncation)]
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second as i64;
    u64::try_from(seconds).ok()
}

fn first_line(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty())?;
    Some(line.to_owned())
}

/// A typed prompt, not a tool result, a slash command's echo or a note Claude Code adds itself.
fn typed_prompt(entry: &Value) -> Option<String> {
    if entry.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let content = entry.pointer("/message/content")?;
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    first_line(text)
}

fn tool_detail(input: &Value) -> String {
    [
        "description",
        "command",
        "file_path",
        "path",
        "pattern",
        "url",
        "query",
        "prompt",
    ]
    .iter()
    .find_map(|key| input.get(*key).and_then(Value::as_str))
    .and_then(first_line)
    .unwrap_or_default()
}

/// Reads the end of a Claude transcript. Lines that do not parse (the first, cut by the tail) are skipped.
pub fn activity(tail: &str) -> Activity {
    let mut activity = Activity::default();
    let mut calls: Vec<(String, Tool)> = Vec::new();
    for entry in tail
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        let at = entry
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(unix_seconds);
        let blocks = entry
            .pointer("/message/content")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        match entry.get("type").and_then(Value::as_str) {
            Some("user") => {
                for block in &blocks {
                    if block.get("type").and_then(Value::as_str) == Some("tool_result") {
                        if let Some(id) = block.get("tool_use_id").and_then(Value::as_str) {
                            calls.retain(|(call, _)| call != id);
                        }
                    }
                }
                if let Some(prompt) = typed_prompt(&entry) {
                    activity.asked = Some(prompt);
                    activity.asked_at = at;
                    // A new turn: what ran before it has ended, answered or not.
                    calls.clear();
                    activity.said = None;
                }
            }
            Some("assistant") => {
                for block in &blocks {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") => {
                            if let Some(text) = block.get("text").and_then(Value::as_str) {
                                if !text.trim().is_empty() {
                                    activity.said = Some(text.trim().to_owned());
                                }
                            }
                        }
                        Some("tool_use") => {
                            let id = block.get("id").and_then(Value::as_str).unwrap_or_default();
                            calls.push((
                                id.to_owned(),
                                Tool {
                                    name: block
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_owned(),
                                    detail: block.get("input").map(tool_detail).unwrap_or_default(),
                                    since: at,
                                },
                            ));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    activity.now = calls.pop().map(|(_, tool)| tool);
    activity
}

/// What Claude's prompt on screen asks: its heading lines, then its numbered options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prompt {
    pub lines: Vec<String>,
    /// The digit that picks an option, and its label as drawn.
    pub options: Vec<(u8, String)>,
}

/// "❯ 1. Yes" or "  2. Blue": the digit and the label.
fn option_line(row: &str) -> Option<(u8, String)> {
    let row = row.trim_start().trim_start_matches('\u{276F}').trim_start();
    let (number, label) = row.split_once(". ")?;
    let digit: u8 = number.parse().ok()?;
    let label = label.trim();
    ((1..=9).contains(&digit) && !label.is_empty()).then(|| (digit, label.to_owned()))
}

fn is_rule(row: &str, rule: char) -> bool {
    let row = row.trim();
    row.chars().count() >= 3 && row.chars().all(|c| c == rule)
}

/// Reads the prompt a waiting Claude draws at the bottom of its pane. Options are numbered from 1 with
/// description lines and rules between them; above option 1 come the heading lines, up to the first
/// full rule (`─`), skipping the dialog's inner dashed rules and "Tip:" lines.
pub fn prompt(rows: &[String]) -> Option<Prompt> {
    let last = rows.iter().rposition(|row| option_line(row).is_some())?;
    let mut options = Vec::new();
    let mut first = last;
    let mut expect = option_line(&rows[last])?.0;
    for index in (0..=last).rev() {
        if let Some((digit, label)) = option_line(&rows[index]) {
            if digit != expect {
                break;
            }
            options.push((digit, label));
            first = index;
            if digit == 1 {
                break;
            }
            expect -= 1;
        }
    }
    if options.last().map(|(digit, _)| *digit) != Some(1) {
        return None;
    }
    options.reverse();
    let mut lines = Vec::new();
    for row in rows[..first].iter().rev() {
        if is_rule(row, '\u{2500}') || lines.len() == 6 {
            break;
        }
        let text = row.trim();
        if text.is_empty() || is_rule(row, '\u{254C}') || text.starts_with("Tip:") {
            continue;
        }
        lines.push(text.to_owned());
    }
    lines.reverse();
    Some(Prompt { lines, options })
}

#[derive(Default)]
struct Entry {
    activity: Option<Activity>,
    reading: bool,
    read_at: Option<Instant>,
}

static CHANGED: AtomicBool = AtomicBool::new(false);

fn entries() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn start_read(path: &Path) {
    {
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.entry(path.to_path_buf()).or_default();
        let fresh = entry.read_at.is_some_and(|at| at.elapsed() < REREAD);
        if entry.reading || fresh {
            return;
        }
        entry.reading = true;
    }
    let file = path.to_path_buf();
    let spawned = crate::thread_spawn::spawn_named("nirvana-running", move || {
        let activity = crate::nirvana_spaces::read_tail(&file).map(|tail| activity(&tail));
        let mut entries = entries().lock().unwrap_or_else(|e| e.into_inner());
        let entry = entries.entry(file).or_default();
        entry.reading = false;
        entry.read_at = Some(Instant::now());
        if activity.is_some() && entry.activity != activity {
            entry.activity = activity;
            CHANGED.store(true, Ordering::Release);
        }
    });
    if spawned.is_err() {
        if let Some(entry) = entries()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(path)
        {
            entry.reading = false;
        }
    }
}

/// The transcript's activity as last read; a read starts when it is older than `REREAD`. Under test no
/// read starts and a test sets what is returned.
pub fn cached(path: &Path) -> Option<Activity> {
    #[cfg(test)]
    if let Some(activity) = TEST_ACTIVITY.with_borrow(Clone::clone) {
        return Some(activity);
    }
    if !cfg!(test) {
        start_read(path);
    }
    entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(path)
        .and_then(|entry| entry.activity.clone())
}

#[cfg(test)]
thread_local! {
    static TEST_ACTIVITY: std::cell::RefCell<Option<Activity>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub fn set_activity_for_test(activity: Option<Activity>) {
    TEST_ACTIVITY.with_borrow_mut(|slot| *slot = activity);
}

/// Whether an activity changed since the last call: the window repaints on its next timer tick.
pub fn take_changed() -> bool {
    CHANGED.swap(false, Ordering::AcqRel)
}

/// Seconds as "41s", "2m 13s", "1h 5m".
pub fn elapsed(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3_599 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn timestamps_read_as_unix_seconds() {
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            unix_seconds("2026-10-16T05:36:56.377Z"),
            Some(1_792_129_016)
        );
        assert_eq!(unix_seconds("2000-03-01T00:00:00.000Z"), Some(951_868_800));
        assert_eq!(unix_seconds("not a time"), None);
    }

    /// Lines as Claude Code 2.1.301 wrote them on 2026-10-16 (case B of the measurement), trimmed.
    const TRANSCRIPT: &str = r#"{"type":"user","message":{"role":"user","content":"Run the shell command: echo nirvana-wait-check > wait-check.txt"},"timestamp":"2026-10-16T05:38:00.000Z"}
{"type":"assistant","message":{"content":[{"type":"thinking","thinking":""}]},"timestamp":"2026-10-16T05:38:02.000Z"}
{"type":"assistant","message":{"content":[{"type":"text","text":"I'll write the marker and read it back."}]},"timestamp":"2026-10-16T05:38:03.000Z"}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"echo nirvana-wait-check > wait-check.txt && cat wait-check.txt","description":"Write marker text to wait-check.txt and read it back"}}]},"timestamp":"2026-10-16T05:38:04.000Z"}"#;

    #[test]
    fn a_tool_call_without_a_result_is_what_runs_now() {
        let activity = activity(&format!("{{\"cut line\n{TRANSCRIPT}"));
        assert_eq!(
            activity.asked.as_deref(),
            Some("Run the shell command: echo nirvana-wait-check > wait-check.txt")
        );
        assert_eq!(activity.asked_at, unix_seconds("2026-10-16T05:38:00Z"));
        assert_eq!(
            activity.said.as_deref(),
            Some("I'll write the marker and read it back.")
        );
        let now = activity.now.expect("a call in flight");
        assert_eq!(now.name, "Bash");
        assert_eq!(
            now.detail,
            "Write marker text to wait-check.txt and read it back"
        );
        assert_eq!(now.since, unix_seconds("2026-10-16T05:38:04Z"));
        // Its result ends it.
        let done = format!(
            "{TRANSCRIPT}\n{}",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"nirvana-wait-check"}]},"timestamp":"2026-10-16T05:38:09.000Z"}"#
        );
        let done = super::activity(&done);
        assert_eq!(done.now, None);
        // A tool result is not a typed prompt.
        assert_eq!(
            done.asked.as_deref(),
            Some("Run the shell command: echo nirvana-wait-check > wait-check.txt")
        );
        // A new prompt starts a new turn.
        let next = format!(
            "{TRANSCRIPT}\n{}",
            r#"{"type":"user","message":{"role":"user","content":"thanks"},"timestamp":"2026-10-16T05:39:00.000Z"}"#
        );
        let next = super::activity(&next);
        assert_eq!(next.asked.as_deref(), Some("thanks"));
        assert_eq!(next.now, None);
        assert_eq!(next.said, None);
    }

    #[test]
    fn a_question_on_screen_reads_as_its_heading_and_options() {
        // AskUserQuestion as drawn (measurement case A).
        let screen = rows(
            "──────────────────────────────────────\n \u{2610} Button colour\nWhich colour should the button be?\n\u{276F} 1. Red (Recommended)\n     Red button colour.\n  2. Blue\n     Blue button colour.\n  3. Type something.\n───────────────────────────────────────\n  4. Chat about this\nEnter to select \u{b7} \u{2191}/\u{2193} to navigate \u{b7} Esc to cancel",
        );
        let prompt = prompt(&screen).expect("a prompt");
        assert_eq!(
            prompt.lines,
            vec![
                "\u{2610} Button colour",
                "Which colour should the button be?"
            ]
        );
        assert_eq!(
            prompt.options,
            vec![
                (1, "Red (Recommended)".to_owned()),
                (2, "Blue".to_owned()),
                (3, "Type something.".to_owned()),
                (4, "Chat about this".to_owned()),
            ]
        );
    }

    #[test]
    fn a_permission_dialog_reads_with_its_command() {
        // The Bash permission dialog as drawn (measurement case B).
        let screen = rows(
            "\u{25CF} earlier output\n─────────────────────────────\n Bash command\n Tip: auto mode handles these prompts for you\n Write marker text to wait-check.txt and read it back\n\u{254C}\u{254C}\u{254C}\u{254C}\u{254C}\n echo nirvana-wait-check > wait-check.txt && cat wait-check.txt\n\u{254C}\u{254C}\u{254C}\u{254C}\u{254C}\n Do you want to proceed?\n \u{276F} 1. Yes\n   2. Yes, and always allow access to C:\\x from this project\n   3. Yes, and switch to auto mode\n   4. No\n Esc to cancel \u{b7} Tab to amend",
        );
        let prompt = prompt(&screen).expect("a prompt");
        assert_eq!(
            prompt.lines,
            vec![
                "Bash command",
                "Write marker text to wait-check.txt and read it back",
                "echo nirvana-wait-check > wait-check.txt && cat wait-check.txt",
                "Do you want to proceed?",
            ]
        );
        assert_eq!(prompt.options.len(), 4);
        assert_eq!(prompt.options[3], (4, "No".to_owned()));
    }

    #[test]
    fn numbered_text_that_is_not_a_prompt_from_one_is_not_read() {
        assert_eq!(prompt(&rows("  2. step two\n  3. step three")), None);
        assert_eq!(prompt(&rows("nothing here")), None);
    }

    #[test]
    fn durations_read_short() {
        assert_eq!(elapsed(41), "41s");
        assert_eq!(elapsed(133), "2m 13s");
        assert_eq!(elapsed(3_900), "1h 5m");
    }
}
