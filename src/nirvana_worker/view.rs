//! The worker tab's view: a headless worker's stream drawn the way Claude Code draws a turn.
//!
//! The owner, 2026-10-22: "I htought agy was similar to claude code in its shell reporting", on a tab that showed
//! `· read_file` two dozen times with no file. A worker runs headless (`--print`, stream-json) so Nirvana gets a
//! checked report, live tokens and a sure end; this module only changes how that stream looks: `⏺ Read(path)` for a
//! call, `⎿` under it for what came back, and the agy report as a few lines instead of raw JSON.

use serde_json::Value;

use super::{Note, Tokens};

const RESULT_LINES: usize = 3;
const LINE_CHARS: usize = 120;

/// The Claude Code name for a tool. Claude's own names are already Claude Code's; agy's are mapped.
pub fn tool_name(cli: &str, raw: &str) -> String {
    if cli != "agy" {
        return raw.to_owned();
    }
    match raw {
        "read_file" | "view_file" => "Read",
        "write_file" | "write_to_file" => "Write",
        "edit_file" | "replace_file_content" | "multi_replace_file_content" => "Edit",
        "run_command" => "Bash",
        "manage_task" | "command_status" => "Task",
        "grep_search" | "codebase_search" => "Search",
        "find_by_name" => "Find",
        "list_dir" => "List",
        "invoke_subagent" | "browser_subagent" => "Agent",
        "call_mcp_tool" => "MCP",
        other if other.starts_with("browser_") => "Browser",
        other => other,
    }
    .to_owned()
}

/// A path as the worker's folder sees it: the folder's own prefix dropped, `/` between parts.
pub fn short_path(path: &str, cwd: Option<&str>) -> String {
    let unify = |text: &str| text.replace('\\', "/");
    let path = unify(path);
    if let Some(cwd) = cwd {
        let cwd = unify(cwd);
        let cwd = cwd.trim_end_matches('/');
        // Windows paths compare without case; a drive letter comes either way.
        if path.len() > cwd.len()
            && path.is_char_boundary(cwd.len())
            && path[..cwd.len()].eq_ignore_ascii_case(cwd)
            && path[cwd.len()..].starts_with('/')
        {
            return path[cwd.len() + 1..].to_owned();
        }
    }
    path
}

fn clip(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim_end();
    if line.chars().count() > LINE_CHARS {
        let mut cut: String = line.chars().take(LINE_CHARS - 1).collect();
        cut.push('…');
        cut
    } else {
        line.to_owned()
    }
}

fn str_of<'a>(params: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| params.get(*key).and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
}

/// `Read(src/pages/WorkoutPage.tsx)`: the tool and what it acts on.
pub fn call_label(cli: &str, raw: &str, params: &Value, cwd: Option<&str>) -> String {
    let name = tool_name(cli, raw);
    let path_keys = [
        "file_path",
        "Path",
        "path",
        "TargetFile",
        "AbsolutePath",
        "notebook_path",
    ];
    let target = if raw == "manage_task" || raw == "command_status" {
        let task = str_of(params, &["TaskId", "CommandId"])
            .map(|id| id.rsplit('/').next().unwrap_or(id).to_owned());
        match (str_of(params, &["Action"]), task) {
            (Some(action), Some(task)) => format!("{action} {task}"),
            (None, Some(task)) => task,
            (Some(action), None) => action.to_owned(),
            (None, None) => String::new(),
        }
    } else if raw == "TodoWrite" {
        let count = params
            .get("todos")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        format!("{count} todo{}", if count == 1 { "" } else { "s" })
    } else if let Some(command) = str_of(params, &["command", "CommandLine"]) {
        clip(command)
    } else if let Some(pattern) = str_of(params, &["pattern", "Pattern", "Query", "query"]) {
        match str_of(params, &path_keys)
            .or_else(|| str_of(params, &["SearchPath", "SearchDirectory"]))
        {
            Some(within) => format!("{} in {}", clip(pattern), short_path(within, cwd)),
            None => clip(pattern),
        }
    } else if let Some(path) = str_of(params, &path_keys) {
        let mut label = short_path(path, cwd);
        let start = params
            .get("StartLine")
            .or_else(|| params.get("offset"))
            .and_then(Value::as_u64);
        if let Some(start) = start.filter(|start| *start > 1) {
            label.push_str(&format!(" from line {start}"));
        }
        label
    } else if let Some(text) = str_of(params, &["url", "Url", "description", "prompt", "Task"]) {
        clip(text)
    } else {
        String::new()
    };
    if target.is_empty() {
        name
    } else {
        format!("{name}({target})")
    }
}

/// The first few lines of a tool's answer, and how many more there were.
fn first_lines(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut shown: Vec<String> = lines
        .iter()
        .take(RESULT_LINES)
        .map(|line| clip(line))
        .collect();
    if lines.len() > RESULT_LINES {
        shown.push(format!("… +{} lines", lines.len() - RESULT_LINES));
    }
    shown
}

/// What an agy tool step gave back: `(ok, lines)`, from its DONE or ERROR step.
pub fn agy_result(raw: &str, tool_info: &Value, failed: bool) -> (bool, Vec<String>) {
    if failed {
        let why = tool_info
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or("the tool failed");
        return (false, vec![clip(why)]);
    }
    let Some(output) = tool_info.get("output").and_then(Value::as_str) else {
        return (true, vec!["done".into()]);
    };
    match raw {
        // "39 lines, 3408 bytes"
        "read_file" | "view_file" => (
            true,
            vec![output.split(',').next().unwrap_or(output).trim().to_owned()],
        ),
        // "Command exited with code 1.\n\nCommand output:\n..."
        "run_command" => {
            let code = output
                .strip_prefix("Command exited with code ")
                .and_then(|rest| rest.split('.').next())
                .and_then(|code| code.trim().parse::<i64>().ok());
            // Without output agy says so on a second line instead of a "Command output:" block.
            let body = match output.split_once("Command output:") {
                Some((_, body)) => body.to_owned(),
                None => output
                    .lines()
                    .filter(|line| {
                        !line.starts_with("Command exited with code ")
                            && line.trim() != "No output from the command."
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            };
            let mut lines = first_lines(&body);
            let ok = code.is_none_or(|code| code == 0);
            // Claude Code names the exit only when it failed.
            if let Some(code) = code.filter(|code| *code != 0) {
                lines.insert(0, format!("exit {code}"));
            }
            if lines.is_empty() {
                lines.push("done".into());
            }
            (ok, lines)
        }
        // "Task: …\nStatus: RUNNING\nLog: …\nLog output:\n…"
        "manage_task" | "command_status" => {
            let status = output
                .lines()
                .find_map(|line| line.strip_prefix("Status:"))
                .map(|status| status.trim().to_lowercase());
            let last = output
                .split_once("Log output:")
                .and_then(|(_, log)| {
                    // agy closes the log with its own "Last progress: 6s ago"; the line before it is the news.
                    log.lines().rev().find(|line| {
                        !line.trim().is_empty() && !line.trim_start().starts_with("Last progress:")
                    })
                })
                .map(|line| clip(line.trim()));
            let line = match (status, last) {
                (Some(status), Some(last)) => format!("{status} · {last}"),
                (Some(status), None) => status,
                (None, Some(last)) => last,
                (None, None) => clip(output),
            };
            (true, vec![line])
        }
        _ => {
            let lines = first_lines(output);
            (
                true,
                if lines.is_empty() {
                    vec!["done".into()]
                } else {
                    lines
                },
            )
        }
    }
}

/// What a Claude `tool_result` gave back: `(ok, lines)`.
pub fn claude_result(raw: &str, content: &Value, is_error: bool) -> (bool, Vec<String>) {
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    if is_error {
        return (false, vec![clip(text.trim())]);
    }
    // Claude Code answers an edit with its own short line; the CLI's sentence repeats the whole path.
    if matches!(raw, "Edit" | "MultiEdit" | "Write" | "NotebookEdit") {
        return (true, vec!["done".into()]);
    }
    if raw == "Read" {
        let count = text.lines().count();
        return (
            true,
            vec![format!("{count} line{}", if count == 1 { "" } else { "s" })],
        );
    }
    let lines = first_lines(&text);
    (
        true,
        if lines.is_empty() {
            vec!["done".into()]
        } else {
            lines
        },
    )
}

struct Paint(bool);

impl Paint {
    fn wrap(&self, code: &str, text: &str) -> String {
        if self.0 {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }
}

/// A report the worker answered with, as JSON in its text: shown as a few lines, not raw JSON.
fn report_lines(text: &str, paint: &Paint) -> Option<Vec<String>> {
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    let status = value.get("status")?.as_str()?;
    let mut lines = vec![format!(
        "{} {}",
        paint.wrap("32", "⏺"),
        paint.wrap("1", &format!("Report: {status}"))
    )];
    if let Some(summary) = value.get("summary").and_then(Value::as_str) {
        lines.extend(
            first_lines(summary)
                .into_iter()
                .map(|line| format!("  {line}")),
        );
    }
    if let Some(files) = value.get("files").and_then(Value::as_array) {
        lines.push(format!(
            "  {} file{} changed",
            files.len(),
            if files.len() == 1 { "" } else { "s" }
        ));
    }
    if let Some(tests) = value.get("tests").and_then(Value::as_str) {
        lines.push(format!("  tests: {}", clip(tests)));
    }
    Some(lines)
}

/// The lines a note draws in the worker's tab. `paint` adds colour; `herdr seat read` passes false.
pub fn note_lines(note: &Note, paint: bool) -> Vec<String> {
    let paint = Paint(paint);
    match note {
        Note::Tool(label) => {
            let (name, rest) = label
                .split_once('(')
                .map_or((label.as_str(), String::new()), |(name, rest)| {
                    (name, format!("({rest}"))
                });
            vec![format!(
                "{} {}{rest}",
                paint.wrap("32", "⏺"),
                paint.wrap("1", name)
            )]
        }
        Note::ToolOut { label, ok, lines } => lines
            .iter()
            .enumerate()
            .map(|(at, line)| {
                let mut line = line.clone();
                if at == 0 {
                    if !ok {
                        line = format!("✗ {line}");
                    }
                    if let Some(label) = label {
                        line = format!("{label}: {line}");
                    }
                }
                let line = if *ok {
                    paint.wrap("2", &line)
                } else {
                    paint.wrap("31", &line)
                };
                let lead = if at == 0 { "  ⎿  " } else { "     " };
                format!("{}{line}", paint.wrap("2", lead))
            })
            .collect(),
        Note::Text(text) => report_lines(text, &paint).unwrap_or_else(|| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .enumerate()
                .map(|(at, line)| {
                    // The tab is not a markdown view: bold markers are noise in it.
                    let line = line.replace("**", "");
                    if at == 0 {
                        format!("{} {line}", paint.wrap("37", "⏺"))
                    } else {
                        format!("  {line}")
                    }
                })
                .collect()
        }),
        _ => Vec::new(),
    }
}

fn amount(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=999_999 => format!("{:.1}k", count as f64 / 1_000.0).replace(".0k", "k"),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0).replace(".0M", "M"),
    }
}

fn elapsed(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3_599 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3_600, seconds % 3_600 / 60),
    }
}

/// The tab's first line: what runs, on what, and which attempt.
pub fn header(
    order: &str,
    cli: &str,
    model: &str,
    effort: &str,
    class: &str,
    attempt: u32,
    paint: bool,
) -> String {
    let paint = Paint(paint);
    format!(
        "{} {} · {cli} {model} ({effort}) · {class} · attempt {attempt}",
        paint.wrap("35", "✻"),
        paint.wrap("1", order)
    )
}

/// The tab's last line: how the attempt ended, how long it took, what it spent.
pub fn footer(ended: &str, ok: bool, seconds: u64, tokens: Tokens, paint: bool) -> String {
    let paint = Paint(paint);
    let read = tokens.input + tokens.cache_read + tokens.cache_write;
    let line = format!(
        "{ended} · {} · {} in, {} out",
        elapsed(seconds),
        amount(read),
        amount(tokens.output)
    );
    if ok {
        format!("{} {}", paint.wrap("35", "✻"), line)
    } else {
        format!("{} {}", paint.wrap("31", "✗"), paint.wrap("31", &line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CWD: &str = r"C:\Users\me\Pojects\App\.nirvana\worktrees\list-mode";

    #[test]
    fn agy_names_map_to_claude_code_and_paths_drop_the_worker_folder() {
        let params = json!({"Path": r"C:\Users\me\Pojects\App\.nirvana\worktrees\list-mode\src\pages\WorkoutPage.tsx", "StartLine": 1, "EndLine": 800});
        assert_eq!(
            call_label("agy", "read_file", &params, Some(CWD)),
            "Read(src/pages/WorkoutPage.tsx)"
        );
        let params = json!({"CommandLine": "npx tsc --noEmit"});
        assert_eq!(
            call_label("agy", "run_command", &params, Some(CWD)),
            "Bash(npx tsc --noEmit)"
        );
        let params = json!({"Action": "status", "TaskId": "6eebae5a/task-15"});
        assert_eq!(
            call_label("agy", "manage_task", &params, Some(CWD)),
            "Task(status task-15)"
        );
        // A path outside the folder is kept whole; a case difference in the drive is still the folder.
        let params = json!({"Path": r"c:\users\me\pojects\app\.nirvana\worktrees\list-mode\a.ts"});
        assert_eq!(
            call_label("agy", "write_file", &params, Some(CWD)),
            "Write(a.ts)"
        );
        let params = json!({"file_path": "/tmp/other.py"});
        assert_eq!(
            call_label("claude", "Read", &params, Some(CWD)),
            "Read(/tmp/other.py)"
        );
    }

    #[test]
    fn agy_results_read_as_lines_counts_exits_and_task_status() {
        let (ok, lines) = agy_result(
            "read_file",
            &json!({"output": "39 lines, 3408 bytes"}),
            false,
        );
        assert!(ok);
        assert_eq!(lines, ["39 lines"]);
        let output = "Command exited with code 1.\n\nCommand output:\nsrc/a.tsx(1,1): error TS2322\nb\nc\nd\n";
        let (ok, lines) = agy_result("run_command", &json!({"output": output}), false);
        assert!(!ok);
        assert_eq!(
            lines,
            [
                "exit 1",
                "src/a.tsx(1,1): error TS2322",
                "b",
                "c",
                "… +1 lines"
            ]
        );
        let output = "Task: x/task-15\nStatus: RUNNING\nLog: file:///x\nLog output:\n\n RUN  v3.2.4\n ✓ list (3 tests)\nLast progress: 6s ago\n";
        let (_, lines) = agy_result("manage_task", &json!({"output": output}), false);
        assert_eq!(lines, ["running · ✓ list (3 tests)"]);
        let quiet = "Command exited with code 0.\nNo output from the command.";
        let (ok, lines) = agy_result("run_command", &json!({"output": quiet}), false);
        assert!(ok);
        assert_eq!(lines, ["done"]);
        let fine = "Command exited with code 0.\n\nCommand output:\nok\n";
        assert_eq!(
            agy_result("run_command", &json!({"output": fine}), false).1,
            ["ok"]
        );
        let error = json!({"error": {"message": "target content not found in the file"}});
        let (ok, lines) = agy_result("edit_file", &error, true);
        assert!(!ok);
        assert_eq!(lines, ["target content not found in the file"]);
    }

    #[test]
    fn claude_results_count_a_read_and_flag_an_error() {
        let (ok, lines) = claude_result("Read", &json!("1\tone\n2\ttwo\n"), false);
        assert!(ok);
        assert_eq!(lines, ["2 lines"]);
        let (ok, lines) = claude_result(
            "Bash",
            &json!([{"type": "text", "text": "boom\nmore"}]),
            true,
        );
        assert!(!ok);
        assert_eq!(lines, ["boom"]);
    }

    #[test]
    fn notes_draw_like_claude_code_and_paint_only_when_asked() {
        let call = Note::Tool("Read(src/a.ts)".into());
        assert_eq!(note_lines(&call, false), ["⏺ Read(src/a.ts)"]);
        let out = Note::ToolOut {
            label: None,
            ok: false,
            lines: vec!["exit 1".into(), "error".into()],
        };
        assert_eq!(note_lines(&out, false), ["  ⎿  ✗ exit 1", "     error"]);
        let late = Note::ToolOut {
            label: Some("Bash(ls)".into()),
            ok: true,
            lines: vec!["a".into()],
        };
        assert_eq!(note_lines(&late, false), ["  ⎿  Bash(ls): a"]);
        assert!(note_lines(&call, true)[0].contains("\x1b["));
        assert!(!note_lines(&out, false).concat().contains('\x1b'));
        // agy answers with its report as JSON text: drawn as a report, not raw JSON.
        let report = Note::Text(r#"{"status":"done","summary":"Added list mode.","files":["a","b"],"tests":"vitest 12 passed"}"#.into());
        assert_eq!(
            note_lines(&report, false),
            [
                "⏺ Report: done",
                "  Added list mode.",
                "  2 files changed",
                "  tests: vitest 12 passed"
            ]
        );
    }

    #[test]
    fn the_header_and_footer_name_the_run() {
        assert_eq!(
            header(
                "list-mode",
                "agy",
                "gemini-3.8-flash-medium",
                "medium",
                "build.frontend",
                1,
                false
            ),
            "✻ list-mode · agy gemini-3.8-flash-medium (medium) · build.frontend · attempt 1"
        );
        let tokens = Tokens {
            input: 72_437,
            cache_write: 0,
            cache_read: 59_004,
            output: 958,
        };
        assert_eq!(
            footer("delivered: done", true, 252, tokens, false),
            "✻ delivered: done · 4m 12s · 131.4k in, 958 out"
        );
        assert!(footer("failed: x", false, 5, tokens, false).starts_with("✗ failed: x · 5s"));
    }
}
