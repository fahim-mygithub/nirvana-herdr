//! Alpha 0.69.0: the background-wait reader. The transcript lines are cut-down copies of the shapes Claude Code
//! 2.1.322 wrote in the owner's transcripts (2026-11-10, `WorkOutAppRefactor/b4c82fab...jsonl` and the orchestrator's
//! session), with the paths scrubbed and every field the reader does not look at dropped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::*;

/// 2026-11-10T12:00:00Z in Unix seconds.
const S0: u64 = 1_794_312_000;
const OUT: &str =
    r"C:\Users\USER\AppData\Local\Temp\claude\C--Users-USER-proj\sess\tasks\b24g7efc5.output";

fn at(secs: u64) -> String {
    let t = time::OffsetDateTime::from_unix_timestamp(secs as i64).unwrap();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.000Z",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

fn line(value: Value) -> String {
    value.to_string()
}

fn assistant_tool(id: &str, name: &str, input: Value, secs: u64) -> String {
    line(json!({
        "type": "assistant", "isSidechain": false, "timestamp": at(secs),
        "message": {"model": "claude-haiku-5-5", "stop_reason": "tool_use",
                    "content": [{"type": "tool_use", "id": id, "name": name, "input": input}]}
    }))
}

fn bash_launch(id: &str, command: &str, secs: u64) -> String {
    assistant_tool(
        id,
        "Bash",
        json!({"command": command, "description": "Run the tests", "run_in_background": true}),
        secs,
    )
}

fn bash_result(id: &str, task: &str, secs: u64) -> String {
    let out = OUT.replace("b24g7efc5", task);
    line(json!({
        "type": "user", "isSidechain": false, "timestamp": at(secs),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": id, "is_error": false,
            "content": format!("Command running in background with ID: {task}. Output is being written to: {out}. You will be notified when it completes.")}]},
        "toolUseResult": {"stdout": "", "stderr": "", "interrupted": false, "backgroundTaskId": task}
    }))
}

fn end_turn(secs: u64) -> String {
    line(json!({
        "type": "assistant", "isSidechain": false, "timestamp": at(secs),
        "message": {"model": "claude-haiku-5-5", "stop_reason": "end_turn",
                    "content": [{"type": "text", "text": "Started it in the background."}]}
    }))
}

fn turn_duration(secs: u64) -> String {
    line(
        json!({"type": "system", "subtype": "turn_duration", "timestamp": at(secs), "isMeta": false}),
    )
}

fn notification(task: &str, status: Option<&str>) -> String {
    let status = status.map_or(String::new(), |s| format!("<status>{s}</status>\n"));
    format!(
        "<task-notification>\n<task-id>{task}</task-id>\n<tool-use-id>toolu_x</tool-use-id>\n<output-file>{}</output-file>\n{status}<summary>Background command \"Run the tests\" completed (exit code 0)</summary>\n</task-notification>",
        OUT.replace("b24g7efc5", task)
    )
}

fn enqueue(task: &str, status: Option<&str>, secs: u64) -> String {
    line(
        json!({"type": "queue-operation", "operation": "enqueue", "timestamp": at(secs), "sessionId": "sess",
                "content": notification(task, status)}),
    )
}

fn queued_attachment(task: &str, secs: u64) -> String {
    line(
        json!({"type": "attachment", "isSidechain": false, "timestamp": at(secs),
                "attachment": {"type": "queued_command", "prompt": notification(task, Some("completed")),
                               "commandMode": "task-notification"}}),
    )
}

fn compact_boundary(secs: u64) -> String {
    line(
        json!({"parentUuid": null, "isSidechain": false, "type": "system", "subtype": "compact_boundary",
                "content": "Conversation compacted", "timestamp": at(secs),
                "compactMetadata": {"trigger": "auto", "preTokens": 429164, "postTokens": 24509}}),
    )
}

fn prompt(text: &str, secs: u64) -> String {
    line(
        json!({"type": "user", "isSidechain": false, "timestamp": at(secs),
                "message": {"role": "user", "content": text}}),
    )
}

fn ledger(lines: &[String]) -> Ledger {
    let mut ledger = Ledger::default();
    for line in lines {
        ledger.feed(line);
    }
    ledger
}

fn ids(ledger: &Ledger) -> Vec<&str> {
    ledger.open().iter().map(|task| task.id.as_str()).collect()
}

// ---- what the transcript says is open -----------------------------------------------------------------------------

#[test]
fn a_launch_alone_is_open_with_its_output_file() {
    let l = ledger(&[
        bash_launch("toolu_1", "sleep 90", S0),
        bash_result("toolu_1", "b24g7efc5", S0 + 1),
        end_turn(S0 + 2),
    ]);
    assert_eq!(ids(&l), ["b24g7efc5"]);
    let task = &l.open()[0];
    assert_eq!(task.kind, Kind::Shell);
    assert_eq!(task.label, "Run the tests");
    assert_eq!(task.command.as_deref(), Some("sleep 90"));
    assert_eq!(task.output.as_deref(), Some(Path::new(OUT)));
    assert_eq!(task.launched_at, S0 + 1);
    assert!(l.turn_ended);
}

#[test]
fn a_launch_and_its_notification_is_closed() {
    let launch = [
        bash_launch("toolu_1", "sleep 90", S0),
        bash_result("toolu_1", "b24g7efc5", S0 + 1),
        end_turn(S0 + 2),
    ];
    let mut lines = launch.to_vec();
    lines.push(enqueue("b24g7efc5", Some("completed"), S0 + 90));
    assert!(ledger(&lines).open().is_empty());
    // The attachment Claude Code writes for the same notification closes it on its own too.
    let mut lines = launch.to_vec();
    lines.push(queued_attachment("b24g7efc5", S0 + 90));
    assert!(ledger(&lines).open().is_empty());
    // A failed or killed task is closed as well: it is over either way.
    let mut lines = launch.to_vec();
    lines.push(enqueue("b24g7efc5", Some("killed"), S0 + 90));
    assert!(ledger(&lines).open().is_empty());
}

#[test]
fn a_task_stop_closes_its_task_and_a_failed_one_does_not() {
    let launch = [
        bash_launch("toolu_1", "npx vite", S0),
        bash_result("toolu_1", "bfodeik5c", S0 + 1),
    ];
    let stop = assistant_tool(
        "toolu_2",
        "TaskStop",
        json!({"task_id": "bfodeik5c"}),
        S0 + 10,
    );
    let stopped = |is_error: bool| {
        line(json!({"type": "user", "timestamp": at(S0 + 11),
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_2", "is_error": is_error,
                "content": "{\"message\":\"Successfully stopped task: bfodeik5c (npx vite)\",\"task_id\":\"bfodeik5c\"}"}]}}))
    };
    let mut lines = launch.to_vec();
    lines.extend([stop.clone(), stopped(false)]);
    assert!(ledger(&lines).open().is_empty());
    let mut lines = launch.to_vec();
    lines.extend([stop, stopped(true)]);
    assert_eq!(ids(&ledger(&lines)), ["bfodeik5c"]);
}

#[test]
fn of_two_tasks_one_finished_leaves_the_other_open() {
    let l = ledger(&[
        bash_launch("toolu_1", "npm test", S0),
        bash_result("toolu_1", "aaaaaaaa1", S0 + 1),
        bash_launch("toolu_2", "npm run lint", S0 + 2),
        bash_result("toolu_2", "bbbbbbbb2", S0 + 3),
        end_turn(S0 + 4),
        enqueue("aaaaaaaa1", Some("completed"), S0 + 60),
    ]);
    assert_eq!(ids(&l), ["bbbbbbbb2"]);
}

#[test]
fn a_compaction_between_launch_and_finish_keeps_the_task_and_still_closes_it() {
    let lines = [
        bash_launch("toolu_1", "cargo test", S0),
        bash_result("toolu_1", "b24g7efc5", S0 + 1),
        end_turn(S0 + 2),
        compact_boundary(S0 + 30),
        prompt("go on", S0 + 31),
        end_turn(S0 + 40),
    ];
    assert_eq!(ids(&ledger(&lines)), ["b24g7efc5"]);
    let mut lines = lines.to_vec();
    lines.push(enqueue("b24g7efc5", Some("completed"), S0 + 90));
    assert!(ledger(&lines).open().is_empty());
}

#[test]
fn a_launch_whose_result_is_an_error_opens_nothing() {
    let error = line(json!({"type": "user", "timestamp": at(S0 + 1),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "is_error": true,
            "content": "<tool_use_error>InputValidationError: exactly one of command or ws</tool_use_error>"}]},
        "toolUseResult": "InputValidationError: exactly one of command or ws"}));
    let l = ledger(&[
        bash_launch("toolu_1", "sleep 90", S0),
        error,
        end_turn(S0 + 2),
    ]);
    assert!(l.open().is_empty());
    // An error that still carries a task id (a hook that blocked the result after the start, say) opens nothing
    // either: the reader shows nothing rather than a task it cannot be sure of.
    let blocked = line(json!({"type": "user", "timestamp": at(S0 + 1),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "is_error": true,
            "content": "PostToolUse hook blocked. Command running in background with ID: b24g7efc5."}]},
        "toolUseResult": {"backgroundTaskId": "b24g7efc5"}}));
    let l = ledger(&[
        bash_launch("toolu_1", "sleep 90", S0),
        blocked,
        end_turn(S0 + 2),
    ]);
    assert!(l.open().is_empty());
}

#[test]
fn a_foreground_command_opens_nothing() {
    let launch = assistant_tool(
        "toolu_1",
        "Bash",
        json!({"command": "sleep 1", "description": "Wait"}),
        S0,
    );
    let result = line(json!({"type": "user", "timestamp": at(S0 + 2),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": ""}]},
        "toolUseResult": {"stdout": "", "stderr": "", "interrupted": false, "backgroundTaskId": "b24g7efc5"}}));
    assert!(ledger(&[launch, result, end_turn(S0 + 3)])
        .open()
        .is_empty());
}

#[test]
fn a_monitor_event_keeps_it_open_and_its_end_closes_it() {
    let launch = assistant_tool(
        "toolu_m",
        "Monitor",
        json!({"command": "tail -n 0 -F seq.log | grep --line-buffered end", "description": "Each run's end line", "timeout_ms": 1_800_000}),
        S0,
    );
    let started = line(json!({"type": "user", "timestamp": at(S0 + 1),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_m",
            "content": "Monitor started (task bzziirw1s, expires in 30m unless the source ends first)."}]},
        "toolUseResult": {"taskId": "bzziirw1s", "timeoutMs": 1_800_000, "persistent": false}}));
    let event = line(
        json!({"type": "queue-operation", "operation": "enqueue", "timestamp": at(S0 + 50),
        "content": "<task-notification>\n<task-id>bzziirw1s</task-id>\n<summary>Monitor event: \"Each run's end line\"</summary>\n<event>end M1 exit 0</event>\n</task-notification>"}),
    );
    let l = ledger(&[
        launch.clone(),
        started.clone(),
        end_turn(S0 + 2),
        event.clone(),
    ]);
    assert_eq!(ids(&l), ["bzziirw1s"]);
    assert_eq!(l.open()[0].kind, Kind::Monitor);
    assert_eq!(l.open()[0].last_event_at, Some(S0 + 50));
    assert_eq!(l.open()[0].output, None);
    let l = ledger(&[
        launch,
        started,
        end_turn(S0 + 2),
        event,
        enqueue("bzziirw1s", Some("completed"), S0 + 99),
    ]);
    assert!(l.open().is_empty());
}

#[test]
fn only_a_background_agent_is_a_task() {
    let launch = |id: &str| {
        assistant_tool(
            id,
            "Agent",
            json!({"description": "Sonnet 5.5 medium - explore", "prompt": "Read.", "run_in_background": "true"}),
            S0,
        )
    };
    let async_result = line(json!({"type": "user", "timestamp": at(S0 + 1),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_a",
            "content": [{"type": "text", "text": "Async agent launched successfully.\nagentId: a071754251fec83ec (internal ID)"}]}]},
        "toolUseResult": {"isAsync": true, "status": "async_launched", "agentId": "a071754251fec83ec"}}));
    let teammate = line(json!({"type": "user", "timestamp": at(S0 + 1),
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_t",
            "content": [{"type": "text", "text": "Spawned successfully.\nagent_id: asonnet-bench-driver-1f977d248b89549d"}]}]},
        "toolUseResult": {"status": "teammate_spawned", "agentId": "asonnet-bench-driver-1f977d248b89549d"}}));
    let l = ledger(&[launch("toolu_a"), async_result, end_turn(S0 + 2)]);
    assert_eq!(ids(&l), ["a071754251fec83ec"]);
    assert_eq!(l.open()[0].kind, Kind::Agent);
    let l = ledger(&[launch("toolu_t"), teammate, end_turn(S0 + 2)]);
    assert!(l.open().is_empty());
}

#[test]
fn the_turn_ends_on_end_turn_turn_duration_or_an_interrupt_and_not_on_a_tool() {
    let base = [
        bash_launch("toolu_1", "sleep 90", S0),
        bash_result("toolu_1", "b24g7efc5", S0 + 1),
    ];
    assert!(
        !ledger(&base).turn_ended,
        "a tool result means the assistant goes on"
    );
    let mut lines = base.to_vec();
    lines.push(end_turn(S0 + 2));
    assert!(ledger(&lines).turn_ended);
    lines.push(prompt("now what", S0 + 3));
    assert!(!ledger(&lines).turn_ended, "a typed prompt starts a turn");
    lines.push(prompt("[Request interrupted by user]", S0 + 4));
    assert!(ledger(&lines).turn_ended);
    let mut lines = base.to_vec();
    lines.push(turn_duration(S0 + 5));
    assert!(ledger(&lines).turn_ended);
    // A slash command's echo starts no turn.
    lines.push(prompt("<command-name>/model</command-name>", S0 + 6));
    assert!(ledger(&lines).turn_ended);
}

#[test]
fn a_subagents_line_is_not_this_sessions() {
    let mut launch: Value = serde_json::from_str(&bash_launch("toolu_1", "sleep 90", S0)).unwrap();
    launch["isSidechain"] = json!(true);
    let mut result: Value =
        serde_json::from_str(&bash_result("toolu_1", "b24g7efc5", S0 + 1)).unwrap();
    result["isSidechain"] = json!(true);
    let l = ledger(&[launch.to_string(), result.to_string(), end_turn(S0 + 2)]);
    assert!(l.open().is_empty());
}

#[test]
fn the_tail_reads_whole_lines_and_only_what_was_added() {
    let dir = std::env::temp_dir().join(format!("herdr-bgwait-tail-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sess.jsonl");
    let first = format!(
        "{}\n{}\n",
        bash_launch("toolu_1", "sleep 90", S0),
        bash_result("toolu_1", "b24g7efc5", S0 + 1)
    );
    let half = end_turn(S0 + 2);
    let (head, rest) = half.split_at(half.len() / 2);
    std::fs::write(&path, format!("{first}{head}")).unwrap();
    let mut tail = Tail::new(path.clone());
    tail.catch_up();
    assert_eq!(ids(&tail.ledger), ["b24g7efc5"]);
    assert!(
        !tail.ledger.turn_ended,
        "the half-written line is not read yet"
    );
    std::fs::write(
        &path,
        format!(
            "{first}{head}{rest}\n{}\n",
            enqueue("b24g7efc5", Some("completed"), S0 + 9)
        ),
    )
    .unwrap();
    tail.catch_up();
    assert!(tail.ledger.open().is_empty());
    assert!(tail.ledger.turn_ended);
    // A file that shrank is read again from the start.
    std::fs::write(&path, format!("{first}{}\n", end_turn(S0 + 2))).unwrap();
    tail.catch_up();
    assert_eq!(ids(&tail.ledger), ["b24g7efc5"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the evidence -------------------------------------------------------------------------------------------------

fn open_shell() -> OpenTask {
    OpenTask {
        id: "b24g7efc5".into(),
        kind: Kind::Shell,
        label: "Run the tests".into(),
        command: Some("npm test -- --reporter 'dot'".into()),
        output: Some(PathBuf::from(OUT)),
        launched_at: S0,
        last_event_at: None,
    }
}

/// What Git Bash runs for the Bash tool (measured 2026-11-10: the command inside `eval '...'`, its own quotes as `'\''`).
fn bash_process(pid: u32) -> Proc {
    Proc {
        pid,
        command_line: r#""C:\Program Files\Git\bin\bash.exe" -c "source /c/Users/USER/.claude/shell-snapshots/snapshot-bash-1.sh 2>/dev/null || true && eval 'npm test -- --reporter '\''dot'\''' < /dev/null && pwd -P >| /c/Users/USER/AppData/Local/Temp/claude-d2f1-cwd""#.into(),
    }
}

fn files(entries: &[(&str, Stat)]) -> HashMap<PathBuf, Stat> {
    entries
        .iter()
        .map(|(path, stat)| (PathBuf::from(path), *stat))
        .collect()
}

fn read(
    task: &OpenTask,
    now: u64,
    claude_started: Option<u64>,
    processes: Option<&[Proc]>,
    files: &HashMap<PathBuf, Stat>,
) -> Reading {
    let stat = |path: &Path| files.get(path).copied();
    classify(
        task,
        &Evidence {
            now,
            claude_started,
            processes,
            session_dir: Some(Path::new(r"C:\c\projects\p\sess")),
            stat: &stat,
        },
    )
}

#[test]
fn alive_and_growing_is_running() {
    let out = files(&[(
        OUT,
        Stat {
            len: 900,
            modified: S0 + 230,
        },
    )]);
    let r = read(
        &open_shell(),
        S0 + 250,
        Some(S0 - 600),
        Some(&[bash_process(7)]),
        &out,
    );
    assert_eq!(r.state, State::Running);
    assert_eq!(r.last_output, Some(S0 + 230));
    // Just started and silent is running too: its process is there and its start is the last sign of life.
    let empty = files(&[(
        OUT,
        Stat {
            len: 0,
            modified: S0,
        },
    )]);
    let r = read(
        &open_shell(),
        S0 + 30,
        Some(S0 - 600),
        Some(&[bash_process(7)]),
        &empty,
    );
    assert_eq!((r.state, r.last_output), (State::Running, None));
}

#[test]
fn alive_with_no_output_past_the_threshold_is_quiet() {
    let out = files(&[(
        OUT,
        Stat {
            len: 900,
            modified: S0 + 10,
        },
    )]);
    let r = read(
        &open_shell(),
        S0 + 10 + QUIET_AFTER_S + 1,
        Some(S0 - 600),
        Some(&[bash_process(7)]),
        &out,
    );
    assert_eq!(r.state, State::Quiet);
    let r = read(
        &open_shell(),
        S0 + 10 + QUIET_AFTER_S,
        Some(S0 - 600),
        Some(&[bash_process(7)]),
        &out,
    );
    assert_eq!(r.state, State::Running);
}

#[test]
fn open_with_no_process_is_lost_never_running() {
    let out = files(&[(
        OUT,
        Stat {
            len: 900,
            modified: S0 + 230,
        },
    )]);
    // Output fresh a moment ago, but nothing under Claude is the task: lost, whatever the file says.
    let other = Proc {
        pid: 9,
        command_line: "cmd /c bladebro mcp".into(),
    };
    let r = read(
        &open_shell(),
        S0 + 240,
        Some(S0 - 600),
        Some(&[other]),
        &out,
    );
    assert_eq!((r.state, r.why), (State::Lost, Some(Why::NoProcess)));
}

#[test]
fn a_live_process_under_a_claude_that_restarted_since_the_launch_is_lost() {
    let out = files(&[(
        OUT,
        Stat {
            len: 900,
            modified: S0 + 230,
        },
    )]);
    let r = read(
        &open_shell(),
        S0 + 240,
        Some(S0 + 100),
        Some(&[bash_process(7)]),
        &out,
    );
    assert_eq!((r.state, r.why), (State::Lost, Some(Why::ClaudeRestarted)));
    // A Claude that started a moment before its own first launch is the same process.
    let r = read(
        &open_shell(),
        S0 + 240,
        Some(S0 + RESTART_SLACK_S),
        Some(&[bash_process(7)]),
        &out,
    );
    assert_eq!(r.state, State::Running);
}

#[test]
fn an_output_file_that_is_gone_is_lost() {
    let r = read(
        &open_shell(),
        S0 + 30,
        Some(S0 - 600),
        Some(&[bash_process(7)]),
        &HashMap::new(),
    );
    assert_eq!((r.state, r.why), (State::Lost, Some(Why::OutputGone)));
}

#[test]
fn with_no_process_list_only_output_is_a_sign_of_life() {
    let out = files(&[(
        OUT,
        Stat {
            len: 900,
            modified: S0 + 230,
        },
    )]);
    assert_eq!(
        read(&open_shell(), S0 + 240, Some(S0 - 600), None, &out).state,
        State::Running
    );
    let empty = files(&[(
        OUT,
        Stat {
            len: 0,
            modified: S0,
        },
    )]);
    assert_eq!(
        read(&open_shell(), S0 + 30, Some(S0 - 600), None, &empty).state,
        State::Quiet
    );
}

#[test]
fn a_process_is_the_tasks_by_its_output_file_or_its_command() {
    let task = open_shell();
    assert!(is_task_process(&task, &bash_process(1).command_line));
    // The PowerShell tool's wrapper names the output file (measured 2026-11-10).
    let wrapper = format!(
        r#"C:\WINDOWS\System32\cmd.exe /d /s /c ""pwsh.exe" -NoProfile -Command "Invoke-Expression -Command $s" > "{OUT}" 2>&1""#
    );
    let powershell = OpenTask {
        command: Some("Get-Date".into()),
        ..open_shell()
    };
    assert!(is_task_process(&powershell, &wrapper));
    assert!(!is_task_process(&task, "cmd /c bladebro mcp"));
    assert!(!is_task_process(
        &OpenTask {
            command: Some("npm run lint".into()),
            ..open_shell()
        },
        &bash_process(1).command_line
    ));
}

#[test]
fn a_background_agent_reads_its_own_transcript() {
    let task = OpenTask {
        id: "a071754251fec83ec".into(),
        kind: Kind::Agent,
        label: "explore".into(),
        command: None,
        output: None,
        launched_at: S0,
        last_event_at: None,
    };
    let path = r"C:\c\projects\p\sess\subagents\agent-a071754251fec83ec.jsonl";
    let fresh = files(&[(
        path,
        Stat {
            len: 5000,
            modified: S0 + 290,
        },
    )]);
    let r = read(&task, S0 + 300, Some(S0 - 600), Some(&[]), &fresh);
    assert_eq!((r.state, r.last_output), (State::Running, Some(S0 + 290)));
    let stale = files(&[(
        path,
        Stat {
            len: 5000,
            modified: S0 + 10,
        },
    )]);
    assert_eq!(
        read(
            &task,
            S0 + 10 + QUIET_AFTER_S + 1,
            Some(S0 - 600),
            Some(&[]),
            &stale
        )
        .state,
        State::Quiet
    );
}

// ---- the pane's reading and its words -----------------------------------------------------------------------------

fn evidence_view(
    l: &Ledger,
    now: u64,
    processes: &[Proc],
    files: &HashMap<PathBuf, Stat>,
) -> Option<View> {
    let stat = |path: &Path| files.get(path).copied();
    view(
        l,
        &Evidence {
            now,
            claude_started: Some(S0 - 600),
            processes: Some(processes),
            session_dir: None,
            stat: &stat,
        },
    )
}

#[test]
fn nothing_is_read_while_the_turn_goes_on() {
    let l = ledger(&[
        bash_launch("toolu_1", "npm test -- --reporter 'dot'", S0),
        bash_result("toolu_1", "b24g7efc5", S0),
    ]);
    let out = files(&[(
        OUT,
        Stat {
            len: 1,
            modified: S0,
        },
    )]);
    assert_eq!(evidence_view(&l, S0 + 5, &[bash_process(7)], &out), None);
}

#[test]
fn a_lost_task_is_shown_for_an_hour_then_dropped() {
    let l = ledger(&[
        bash_launch("toolu_1", "npm test -- --reporter 'dot'", S0),
        bash_result("toolu_1", "b24g7efc5", S0),
        end_turn(S0 + 1),
    ]);
    let out = files(&[(
        OUT,
        Stat {
            len: 1,
            modified: S0,
        },
    )]);
    let v = evidence_view(&l, S0 + 60, &[], &out).unwrap();
    assert_eq!((v.shown(), v.live, v.lost), (Shown::Lost, 0, 1));
    assert_eq!(
        v.sentence(S0 + 60),
        "1 background task lost: Run the tests, no process"
    );
    assert_eq!(
        evidence_view(&l, S0 + LOST_HIDE_AFTER_S + 1, &[], &out),
        None
    );
}

#[test]
fn the_reading_in_words() {
    let running = |label: &str, started, last| Reading {
        id: "x".into(),
        kind: Kind::Shell,
        label: label.into(),
        started,
        last_output: last,
        state: State::Running,
        why: None,
    };
    let one = View {
        tasks: vec![running("sleep 90", S0 - 240, Some(S0 - 20))],
        live: 1,
        lost: 0,
    };
    assert_eq!(one.shown(), Shown::Waiting);
    assert_eq!(
        one.sentence(S0),
        "waiting on 1 background task: sleep 90, started 4m ago, last output 20s ago"
    );
    let silent = View {
        tasks: vec![running("sleep 90", S0 - 30, None)],
        live: 1,
        lost: 0,
    };
    assert_eq!(
        silent.sentence(S0),
        "waiting on 1 background task: sleep 90, started 30s ago, no output yet"
    );
    let quiet = View {
        tasks: vec![Reading {
            state: State::Quiet,
            ..running("sleep 300", S0 - 720, None)
        }],
        live: 1,
        lost: 0,
    };
    assert_eq!(quiet.shown(), Shown::Quiet);
    assert_eq!(
        quiet.sentence(S0),
        "waiting on 1 background task: sleep 300, started 12m ago, quiet for 12m"
    );
    let many = View {
        tasks: vec![running("npm test", S0 - 60, Some(S0 - 5))],
        live: 3,
        lost: 1,
    };
    assert_eq!(many.sentence(S0), "waiting on 3 background tasks: npm test and 2 more, started 1m ago, last output 5s ago. 1 lost");
}

#[test]
fn the_token_round_trips() {
    let l = ledger(&[
        bash_launch("toolu_1", "npm test -- --reporter 'dot'", S0),
        bash_result("toolu_1", "b24g7efc5", S0),
        end_turn(S0 + 1),
    ]);
    let out = files(&[(
        OUT,
        Stat {
            len: 10,
            modified: S0 + 50,
        },
    )]);
    let v = evidence_view(&l, S0 + 60, &[bash_process(7)], &out).unwrap();
    assert_eq!(View::from_token(&v.token()), Some(v.clone()));
    assert_eq!(
        from_tokens([("nirvana_model", "x"), (TOKEN, v.token().as_str())].into_iter()),
        Some(v)
    );
    assert_eq!(
        View::from_token("{\"tasks\":[],\"live\":0,\"lost\":0}"),
        None
    );
}

#[test]
fn a_reading_made_under_another_state_is_not_shown() {
    // Made while the pane read done; the task's notification started a turn and herdr reads working now.
    assert!(!usable(Some(AgentStatus::Done), AgentStatus::Working));
    assert!(!usable(None, AgentStatus::Done));
    assert!(usable(Some(AgentStatus::Done), AgentStatus::Done));
    assert!(usable(Some(AgentStatus::Working), AgentStatus::Working));
}

#[test]
fn a_background_wait_is_never_shown_as_working() {
    let v = View {
        tasks: vec![],
        live: 1,
        lost: 0,
    };
    assert_eq!(
        shown_status(AgentStatus::Working, Some(&v)),
        AgentStatus::Idle
    );
    assert_eq!(
        shown_status(AgentStatus::Working, None),
        AgentStatus::Working
    );
    assert_eq!(
        shown_status(AgentStatus::Blocked, Some(&v)),
        AgentStatus::Blocked
    );
    assert_eq!(shown_status(AgentStatus::Done, Some(&v)), AgentStatus::Done);
}

// ---- the phone ----------------------------------------------------------------------------------------------------

fn herdr_agent(status: &str, view: Option<&View>) -> Value {
    let mut tokens = serde_json::Map::new();
    if let Some(view) = view {
        tokens.insert(TOKEN.into(), json!(view.token()));
    }
    json!({"pane_id": "wP:p1", "agent": "claude", "agent_status": status, "title": "✳ Run the suite", "tokens": tokens})
}

fn waiting_view() -> View {
    View {
        tasks: vec![Reading {
            id: "b24g7efc5".into(),
            kind: Kind::Shell,
            label: "sleep 90".into(),
            started: S0 - 240,
            last_output: Some(S0 - 20),
            state: State::Running,
            why: None,
        }],
        live: 1,
        lost: 0,
    }
}

#[test]
fn the_phone_row_says_waiting_not_working() {
    use crate::nirvana_remote::spaces::agent_row_at;
    let view = waiting_view();
    let row = agent_row_at(&herdr_agent("working", Some(&view)), S0);
    assert_eq!(row["status"], "bg");
    assert_eq!(row["bg"]["state"], "waiting");
    assert_eq!(
        row["bg"]["line"],
        "waiting on 1 background task: sleep 90, started 4m ago, last output 20s ago"
    );
    // herdr's done with a reading is not done yet: the agent waits.
    assert_eq!(
        agent_row_at(&herdr_agent("done", Some(&view)), S0)["status"],
        "bg"
    );
    // A blocked agent is the owner's to answer.
    let blocked = agent_row_at(&herdr_agent("blocked", Some(&view)), S0);
    assert_eq!(
        (blocked["status"].as_str(), blocked.get("bg")),
        (Some("needs"), None)
    );
    // No reading, no change.
    let plain = agent_row_at(&herdr_agent("working", None), S0);
    assert_eq!(
        (plain["status"].as_str(), plain.get("bg")),
        (Some("working"), None)
    );
    // Every task lost: herdr's state, but a Working from the screen reads idle.
    let lost = View {
        tasks: vec![Reading {
            state: State::Lost,
            why: Some(Why::NoProcess),
            ..view.tasks[0].clone()
        }],
        live: 0,
        lost: 1,
    };
    let row = agent_row_at(&herdr_agent("working", Some(&lost)), S0);
    assert_eq!(
        (row["status"].as_str(), row["bg"]["state"].as_str()),
        (Some("idle"), Some("lost"))
    );
}

#[test]
fn the_pc_list_does_not_count_a_background_wait_as_at_work() {
    struct Fake(Value);
    impl crate::nirvana_remote::herdr::Herdr for Fake {
        fn call(&self, method: &str, _: Value) -> Result<Value, String> {
            Ok(match method {
                "workspace.list" => {
                    json!({"workspaces": [{"workspace_id": "w1", "label": "proj"}]})
                }
                _ => self.0.clone(),
            })
        }
    }
    let view = waiting_view();
    let agents = json!({"agents": [herdr_agent("working", Some(&view)), herdr_agent("working", None), herdr_agent("blocked", Some(&view))]});
    let summary = crate::nirvana_remote::herdr::summary(&Fake(agents)).unwrap();
    assert_eq!(summary.working, 2);
}

#[test]
fn the_feed_builds_again_after_an_event_and_while_a_reading_shows() {
    use crate::nirvana_remote::spaces::{next_build, BACKGROUND_EVERY, FOLLOW_UP};
    assert_eq!(next_build(true, false), Some(FOLLOW_UP));
    assert_eq!(next_build(false, true), Some(BACKGROUND_EVERY));
    assert_eq!(next_build(false, false), None);
    assert!(FOLLOW_UP > REFRESH);
}

// ---- the page ------------------------------------------------------------------------------------------------------

/// The page has no runtime in this crate's tests, so this reads its source, as `tests_prompt` does for the picker
/// sheet: the chip's word, the reading's line in the list and the chat, and the chat clearing a reading that went.
#[test]
fn the_page_words_a_background_wait() {
    let spaces = include_str!("../nirvana_remote/page/spaces.js");
    let chat = include_str!("../nirvana_remote/page/chat.js");
    let css = include_str!("../nirvana_remote/page/app.css");
    for page in [spaces, chat] {
        assert!(page.contains("bg: 'waiting'"), "the chip has a word for bg");
        assert!(page.contains("word === 'bg' && bg && bg.state ? bg.state : LABEL[word]"));
    }
    assert!(spaces.contains("(agent.bg && agent.bg.line) || agent.line"));
    assert!(spaces.contains("chip(agent.status, agent.bg)"));
    assert!(chat.contains("waiting.hidden = !agent.bg;"));
    assert!(chat.contains("agent = Object.assign(agent, { bg: null }, data);"));
    assert!(chat.contains("agent = Object.assign(agent, { bg: null }, data.agent || {});"));
    assert!(css.contains(".chip.bg .chip-dot"));
}
