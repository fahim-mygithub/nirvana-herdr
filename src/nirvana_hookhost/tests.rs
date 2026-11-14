//! Plan 41 P41b: the agy adapter on the payloads agy 1.5.9 sent in the P41a probe (2026-11-14), paths shortened.

use super::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Value {
    let text = match name {
        "session_start" => include_str!("fixtures/session_start.json"),
        "interactive_session_start" => include_str!("fixtures/interactive_session_start.json"),
        "pre_invocation_first" => include_str!("fixtures/pre_invocation_first.json"),
        "pre_invocation_second_prompt" => include_str!("fixtures/pre_invocation_second_prompt.json"),
        "pre_tool_read_file" => include_str!("fixtures/pre_tool_read_file.json"),
        "pre_tool_edit_file" => include_str!("fixtures/pre_tool_edit_file.json"),
        "pre_tool_write_file" => include_str!("fixtures/pre_tool_write_file.json"),
        "pre_tool_invoke_subagent" => include_str!("fixtures/pre_tool_invoke_subagent.json"),
        "post_tool_write_file" => include_str!("fixtures/post_tool_write_file.json"),
        "post_tool_run_command" => include_str!("fixtures/post_tool_run_command.json"),
        "stop_first" => include_str!("fixtures/stop_first.json"),
        "stop_after_continue" => include_str!("fixtures/stop_after_continue.json"),
        "interactive_stop_after_continue" => {
            include_str!("fixtures/interactive_stop_after_continue.json")
        }
        "subagent_pre_tool_read_file" => include_str!("fixtures/subagent_pre_tool_read_file.json"),
        "subagent_stop" => include_str!("fixtures/subagent_stop.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).expect("fixture parses")
}

/// A state folder removed when the test ends (memory: checks that leak temp folders filled 144 GB once).
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!("nirvana-hookhost-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch folder");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const HOOKS_HOME: &str = r"C:\nirvana\agy";

fn agy(verb: &str, raw: &Value, state: &Path) -> HookIn {
    read_for(Host::Agy, verb, &raw.to_string(), Some(Path::new(HOOKS_HOME)), state, 1_000)
}

fn answer(hin: &HookIn, out: &str) -> Value {
    serde_json::from_str(hin.answer_text(out, 1_000).trim()).expect("agy answer is one JSON object")
}

const BLOCK: &str = r#"{"decision":"block","reason":"herdr: an open run is at a dead end"}"#;

#[test]
fn the_host_flag_is_exactly_host_agy() {
    let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(host_in(&args(&["digest", "--host", "agy"])), Host::Agy);
    assert_eq!(host_in(&args(&["notice", "--plugin", "--host", "agy"])), Host::Agy);
    assert_eq!(host_in(&args(&["digest"])), Host::Claude);
    assert_eq!(host_in(&args(&["digest", "--host", "claude"])), Host::Claude);
    assert_eq!(host_in(&args(&["digest", "--host=agy"])), Host::Claude);
    assert_eq!(host_in(&args(&["digest", "agy", "--host"])), Host::Claude);
    assert_eq!(without_host(&args(&["--host", "agy"])), Vec::<String>::new());
    assert_eq!(without_host(&args(&["r-1", "--host", "agy"])), args(&["r-1"]));
}

#[test]
fn on_claude_the_input_and_the_answer_are_unchanged() {
    let state = Scratch::new("claude");
    let input = r#"{"session_id":"s","cwd":"C:\\p","hook_event_name":"Stop","stop_hook_active":false}"#;
    let hin = read_for(Host::Claude, "stop", input, None, &state.0, 1_000);
    assert!(!hin.skip);
    assert_eq!(hin.hook, serde_json::from_str::<Value>(input).unwrap());
    for out in ["", "a digest\n", "{\"decision\":\"block\",\"reason\":\"x\"}\n"] {
        assert_eq!(hin.answer_text(out, 1_000), out, "Claude's output is printed as the verb wrote it");
    }
    // A bad input reads as `null`, as every verb's `read_stdin` read it.
    assert_eq!(read_for(Host::Claude, "stop", "not json", None, &state.0, 1_000).hook, Value::Null);
    assert!(std::fs::read_dir(&state.0).unwrap().next().is_none(), "Claude writes no agy state");
}

#[test]
fn agy_tool_names_map_to_the_claude_tools_the_guards_match() {
    // Seen by a hook in P41a.
    assert_eq!(claude_tool("read_file"), Some("Read"));
    assert_eq!(claude_tool("edit_file"), Some("Edit"));
    assert_eq!(claude_tool("write_file"), Some("Write"));
    assert_eq!(claude_tool("invoke_subagent"), Some("Agent"));
    assert_eq!(claude_tool("run_command"), Some("Bash"));
    // In agy's tool list, not seen by a hook.
    assert_eq!(claude_tool("view_file"), Some("Read"));
    for edit in ["replace_file_content", "multi_replace_file_content", "sed_file"] {
        assert_eq!(claude_tool(edit), Some("Edit"), "{edit}");
    }
    assert_eq!(claude_tool("write_to_file"), Some("Write"));
    assert_eq!(claude_tool("notebook_edit"), Some("NotebookEdit"));
    assert_eq!(claude_tool("edit_notebook"), Some("NotebookEdit"));
    assert_eq!(claude_tool("run_workflow"), Some("Agent"));
    assert_eq!(claude_tool("send_message"), None);
    assert_eq!(claude_tool("browser_subagent"), None);
}

#[test]
fn an_agy_tool_call_reads_as_claudes() {
    let hook = to_claude(&fixture("pre_tool_edit_file"), Some(Path::new("C:/work/proj")), false);
    assert_eq!(hook["hook_event_name"], "PreToolUse");
    assert_eq!(hook["session_id"], "eeb6f04f-ce65-4339-a3e5-7a7c061cd0b0");
    assert_eq!(hook["tool_name"], "Edit");
    assert_eq!(hook["tool_input"]["file_path"], r"C:\work\proj\notes.txt");
    assert_eq!(hook["tool_input"]["old_string"], "APPLE");
    assert_eq!(hook["tool_input"]["new_string"], "PEAR");
    assert!(hook.get("agent_id").is_none(), "the parent's call is the orchestrator's");

    let write = to_claude(&fixture("pre_tool_write_file"), None, false);
    assert_eq!(write["tool_name"], "Write");
    assert_eq!(write["tool_input"]["content"], "hi");
    let read = to_claude(&fixture("pre_tool_read_file"), None, false);
    assert_eq!(read["tool_name"], "Read");
    assert_eq!(read["tool_input"]["file_path"], r"C:\work\proj\notes.txt");
    let spawn = to_claude(&fixture("pre_tool_invoke_subagent"), None, false);
    assert_eq!(spawn["tool_name"], "Agent");
    assert_eq!(spawn["tool_input"]["subagent_type"], "research");
    assert!(spawn["tool_input"]["prompt"].as_str().unwrap().contains("notes.txt"));
    let bash = to_claude(&fixture("post_tool_run_command"), None, false);
    assert_eq!(bash["tool_name"], "Bash");
    assert_eq!(bash["tool_input"]["command"], "Get-ChildItem");
}

#[test]
fn an_edit_tool_p41a_never_saw_is_counted_by_all_its_text() {
    let args = json!({"TargetFile": "C:/p/a.rs", "ReplacementChunks": [
        {"TargetContent": "x", "ReplacementContent": "one\ntwo"},
        {"TargetContent": "y", "ReplacementContent": "three"}]});
    let input = tool_input("Edit", &args);
    assert_eq!(input["file_path"], "C:/p/a.rs");
    let lines = input["new_string"].as_str().unwrap().lines().count();
    assert!(lines >= 3, "every replacement line counts toward the edit limit: {lines}");
}

#[test]
fn the_session_folder_is_not_the_hooks_folder() {
    // --add-dir (run 1): cwd is the hooks folder, the session folder is the other workspace path.
    let raw = fixture("stop_first");
    assert_eq!(raw["cwd"], HOOKS_HOME);
    assert_eq!(
        session_cwd(&raw, Some(Path::new(HOOKS_HOME))),
        Some(PathBuf::from("C:/work/proj"))
    );
    // Without knowing the hooks folder the payload's cwd stands.
    assert_eq!(session_cwd(&raw, None), Some(PathBuf::from(HOOKS_HOME)));
    // A workspace `.agents/` (run 4): cwd is the session folder already.
    let raw = fixture("interactive_session_start");
    let session = session_cwd(&raw, Some(Path::new(r"C:\work\proj\.agents")));
    assert_eq!(session, Some(PathBuf::from(r"C:\work\proj")));
}

#[test]
fn a_subagents_start_prompt_and_stop_are_skipped_and_its_tool_calls_carry_an_agent_id() {
    let state = Scratch::new("subagent");
    let stop = agy("stop", &fixture("subagent_stop"), &state.0);
    assert!(stop.skip, "Claude fires SubagentStop, not Stop, for a subagent");
    assert_eq!(answer(&stop, BLOCK), json!({}), "a skipped hook never blocks");
    let mut start = fixture("subagent_stop");
    start["hookEventName"] = json!("SessionStart");
    assert!(agy("digest", &start, &state.0).skip);
    start["hookEventName"] = json!("PreInvocation");
    assert!(agy("board notice", &start, &state.0).skip);

    let read = agy("guard-read", &fixture("subagent_pre_tool_read_file"), &state.0);
    assert!(!read.skip);
    assert!(
        read.hook["agent_id"].as_str().unwrap().starts_with("agy:"),
        "should_skip_orchestrator_hook skips a subagent's call by its agent_id"
    );
    assert!(crate::nirvana_claude::should_skip_orchestrator_hook(&read.hook, None, true));
}

#[test]
fn context_becomes_an_injected_system_message() {
    let (value, continues) = agy_output(Some(Event::SessionStart), "herdr digest\nline two\n", 0);
    assert_eq!(
        value,
        json!({"injectSteps": [{"systemMessage": {"systemMessage": "herdr digest\nline two"}}]})
    );
    assert!(!continues);
    let json_form = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"notice"}}"#;
    assert_eq!(
        agy_output(Some(Event::UserPromptSubmit), json_form, 0).0,
        json!({"injectSteps": [{"systemMessage": {"systemMessage": "notice"}}]})
    );
    assert_eq!(agy_output(Some(Event::UserPromptSubmit), "\n", 0).0, json!({}));
}

#[test]
fn a_pre_tool_block_is_a_deny_and_no_block_is_an_allow() {
    // guard-edit's shape, then guard-spawn's and guard-read's.
    let edit = r#"{"decision":"block","reason":"edits at most 1 file","hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"edits at most 1 file"}}"#;
    assert_eq!(
        agy_output(Some(Event::PreToolUse), edit, 0).0,
        json!({"decision": "deny", "reason": "edits at most 1 file"})
    );
    let spawn = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"name a seat"}}"#;
    assert_eq!(
        agy_output(Some(Event::PreToolUse), &format!("{spawn}\n"), 0).0,
        json!({"decision": "deny", "reason": "name a seat"})
    );
    assert_eq!(agy_output(Some(Event::PreToolUse), "", 0).0, json!({"decision": "allow"}));
}

#[test]
fn a_post_tool_answer_is_empty() {
    // check-edit's block: agy ignores a PostToolUse answer (P41a run 8).
    assert_eq!(agy_output(Some(Event::PostToolUse), BLOCK, 0), (json!({}), false));
    let state = Scratch::new("post");
    let hin = agy("check-edit", &fixture("post_tool_write_file"), &state.0);
    assert!(!hin.skip);
    assert_eq!(hin.hook["tool_name"], "Write");
    assert_eq!(answer(&hin, BLOCK), json!({}));
}

#[test]
fn a_tool_outside_the_verbs_claude_matcher_is_allowed_without_running_it() {
    let state = Scratch::new("matcher");
    let hin = agy("guard-edit", &fixture("pre_tool_read_file"), &state.0);
    assert!(hin.skip, "guard-edit runs for Edit|Write|MultiEdit only");
    assert_eq!(answer(&hin, BLOCK), json!({"decision": "allow"}));
    assert!(!agy("guard-read", &fixture("pre_tool_read_file"), &state.0).skip);
    assert!(!agy("guard-edit", &fixture("pre_tool_edit_file"), &state.0).skip);
    assert!(!agy("guard-spawn", &fixture("pre_tool_invoke_subagent"), &state.0).skip);
    assert!(!agy("check-edit", &fixture("post_tool_run_command"), &state.0).skip);
}

#[test]
fn the_stop_gate_asks_agy_once_per_prompt_and_never_trusts_continuation_active() {
    let state = Scratch::new("stop");
    let first = agy("stop", &fixture("stop_first"), &state.0);
    assert!(!first.skip);
    assert_eq!(first.hook["stop_hook_active"], false);
    assert_eq!(
        answer(&first, BLOCK),
        json!({"decision": "continue", "reason": "herdr: an open run is at a dead end"})
    );
    // The Stop after agy continued: `continuationActive` is false in the payload (P41a), the count says true.
    let again = fixture("stop_after_continue");
    assert_eq!(again["continuationActive"], false);
    let second = agy("stop", &again, &state.0);
    assert_eq!(second.hook["stop_hook_active"], true);
    assert_eq!(answer(&second, BLOCK), json!({}), "the cap: no second continue for one prompt");
    assert_eq!(stop_asks(&state.0, &again), AGY_STOP_ASKS_PER_PROMPT);

    // A new owner prompt starts the count again, as Claude's stop_hook_active is false after one.
    let mut next = fixture("stop_after_continue");
    next["lastUserInput"] = json!("the owner's next prompt");
    let third = agy("stop", &next, &state.0);
    assert_eq!(third.hook["stop_hook_active"], false);
    assert_eq!(answer(&third, BLOCK)["decision"], "continue");

    // A payload claiming continuationActive true is not believed either way.
    let state = Scratch::new("stop-claim");
    let mut claim = fixture("stop_first");
    claim["continuationActive"] = json!(true);
    assert_eq!(agy("stop", &claim, &state.0).hook["stop_hook_active"], false);
}

#[test]
fn the_gates_own_output_becomes_one_continue_then_its_notice() {
    let state = Scratch::new("gate");
    let dead = vec![("r-1".to_owned(), "no live worker".to_owned(), 7)];
    let raised = BTreeMap::new();
    let first = agy("stop", &fixture("stop_first"), &state.0);
    let active = first.hook["stop_hook_active"].as_bool().unwrap();
    let out = crate::nirvana_claude::stop_output(&dead, &raised, active).unwrap();
    assert_eq!(out["decision"], "block");
    assert_eq!(answer(&first, &format!("{out}\n"))["decision"], "continue");
    // The interactive payload of the next Stop (run 4) has the same conversation shape; this one is run 1's.
    let second = agy("stop", &fixture("stop_after_continue"), &state.0);
    let active = second.hook["stop_hook_active"].as_bool().unwrap();
    let out = crate::nirvana_claude::stop_output(&dead, &raised, active).unwrap();
    assert!(out.get("systemMessage").is_some(), "stop_output gives its notice once active");
    assert_eq!(answer(&second, &format!("{out}\n")), json!({}));
    assert_eq!(fixture("interactive_stop_after_continue")["continuationActive"], false);
}

#[test]
fn a_prompt_notice_runs_once_per_prompt_and_per_verb() {
    let state = Scratch::new("prompt");
    let first = fixture("pre_invocation_first");
    let hin = agy("board notice", &first, &state.0);
    assert!(!hin.skip);
    assert_eq!(hin.hook["hook_event_name"], "UserPromptSubmit");
    assert!(hin.hook["prompt"].as_str().unwrap().starts_with("Use your file-reading tool"));
    // The next model call of the same prompt (agy's invocationNum 1).
    let mut call = first.clone();
    call["invocationNum"] = json!(1);
    assert!(agy("board notice", &call, &state.0).skip);
    assert!(!agy("run notice", &call, &state.0).skip, "each verb keeps its own mark");
    assert!(!agy("board notice", &fixture("pre_invocation_second_prompt"), &state.0).skip);
    let back = agy("board notice", &call, &state.0);
    assert!(!back.skip, "the first prompt again after the second is a new prompt");
    let text = std::fs::read_to_string(state.0.join(PROMPTS_FILE)).unwrap();
    assert!(!text.contains("file-reading"), "the state holds no prompt text: {text}");
}

#[test]
fn post_invocation_and_unknown_events_run_nothing() {
    let state = Scratch::new("other");
    let mut raw = fixture("pre_invocation_first");
    raw["hookEventName"] = json!("PostInvocation");
    let hin = agy("board notice", &raw, &state.0);
    assert!(hin.skip);
    assert_eq!(answer(&hin, "a notice\n"), json!({}));
}

#[test]
fn the_owners_words_in_an_agy_transcript_never_pass_a_check() {
    // Plan 41 §5.1: a check on the owner's words is not proven on agy and must refuse. The owner's prompt is in the
    // transcript as agy wrote it, and `--send-ok`'s reader (`nirvana_fresh::owner_quote_at`) does not take it as theirs.
    let text = include_str!("fixtures/transcript_full.jsonl");
    let quote = "Read notes.txt with your file tool and tell me its first line";
    assert!(text.contains(quote));
    let checked = crate::nirvana_gui_tools::send_ok_in(
        text,
        quote,
        Path::new("C:/agy/transcript_full.jsonl"),
        crate::nirvana_usage::rfc3339("2026-11-14T18:45:00Z").unwrap(),
    );
    assert!(checked.is_err(), "send-ok refuses on an agy transcript: {checked:?}");
}

/// Plan 41 P41c: the installed agy plugin reaches every agy session; only the orchestrator's does any work.
#[test]
fn an_agy_hook_outside_the_orchestrator_does_nothing() {
    assert!(outside_orchestrator(Host::Agy, None));
    assert!(outside_orchestrator(Host::Agy, Some("0")));
    assert!(!outside_orchestrator(Host::Agy, Some("1")));
    assert!(!outside_orchestrator(Host::Claude, None));
    let dir = Scratch::new("outside");
    let pre = json!({"hookEventName": "PreToolUse", "conversationId": "c1", "lastUserInput": "go",
        "toolCall": {"name": "write_file", "args": {"Path": "a.txt"}}});
    let hin = read_for(Host::Agy, "guard-edit", &pre.to_string(), None, &dir.0, 100);
    assert_eq!(hin.skipped_text().trim(), r#"{"decision":"allow"}"#);
    let stop = json!({"hookEventName": "Stop", "conversationId": "c1", "lastUserInput": "go"});
    let hin = read_for(Host::Agy, "stop", &stop.to_string(), None, &dir.0, 100);
    assert_eq!(hin.skipped_text().trim(), "{}");
}
