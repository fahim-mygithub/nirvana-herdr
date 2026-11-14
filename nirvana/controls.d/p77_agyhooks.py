# Plan 41 P41b (nirvana/plans/2026-11-14-41-orchestrator-swap.md §5.1): every hook command under `--host agy`.
# Controls starting with p77_. One sabotage per behaviour, each on a product line; the test named must print FAIL with
# it in place (the expected red test is the fifth field, written before the run). `Control` is provided by controls.py.
HOST = "src/nirvana_hookhost.rs"
GUI = "src/nirvana_gui_tools.rs"

_H = "nirvana_hookhost::tests::"
T_FLAG = _H + "the_host_flag_is_exactly_host_agy"
T_CLAUDE = _H + "on_claude_the_input_and_the_answer_are_unchanged"
T_NAMES = _H + "agy_tool_names_map_to_the_claude_tools_the_guards_match"
T_CALL = _H + "an_agy_tool_call_reads_as_claudes"
T_UNSEEN = _H + "an_edit_tool_p41a_never_saw_is_counted_by_all_its_text"
T_CWD = _H + "the_session_folder_is_not_the_hooks_folder"
T_SUB = _H + "a_subagents_start_prompt_and_stop_are_skipped_and_its_tool_calls_carry_an_agent_id"
T_CONTEXT = _H + "context_becomes_an_injected_system_message"
T_DENY = _H + "a_pre_tool_block_is_a_deny_and_no_block_is_an_allow"
T_POST = _H + "a_post_tool_answer_is_empty"
T_MATCHER = _H + "a_tool_outside_the_verbs_claude_matcher_is_allowed_without_running_it"
T_STOP = _H + "the_stop_gate_asks_agy_once_per_prompt_and_never_trusts_continuation_active"
T_GATE = _H + "the_gates_own_output_becomes_one_continue_then_its_notice"
T_PROMPT = _H + "a_prompt_notice_runs_once_per_prompt_and_per_verb"
T_OTHER = _H + "post_invocation_and_unknown_events_run_nothing"
T_OWNER = _H + "the_owners_words_in_an_agy_transcript_never_pass_a_check"
T_OUTSIDE = _H + "an_agy_hook_outside_the_orchestrator_does_nothing"
WORKER = "src/nirvana_worker.rs"
T_WORKER_ENV = "nirvana_worker::tests::a_worker_is_never_started_through_a_shell"

CONTROLS = (
    # The flag
    Control("p77_host_any_value", HOST,
            "        .any(|pair| pair[0] == HOST_FLAG && pair[1] == AGY)\n",
            "        .any(|pair| pair[0] == HOST_FLAG)\n",
            T_FLAG, "rust"),
    Control("p77_claude_converted", HOST,
            "        if self.host == Host::Claude {\n            return out.to_owned();\n",
            "        if false {\n            return out.to_owned();\n",
            T_CLAUDE, "rust"),
    # The input
    Control("p77_edit_file_unmapped", HOST,
            '        "edit_file" | "replace_file_content" | "multi_replace_file_content" | "sed_file" => "Edit",\n',
            '        "replace_file_content" | "multi_replace_file_content" | "sed_file" => "Edit",\n',
            T_NAMES, "rust"),
    Control("p77_edit_text_dropped", HOST,
            '            "new_string": new_text(args),\n',
            '            "new_string": "",\n',
            T_CALL, "rust"),
    Control("p77_unseen_edit_uncounted", HOST,
            '    out.join("\\n")\n',
            "    String::new()\n",
            T_UNSEEN, "rust"),
    Control("p77_cwd_is_hooks_folder", HOST,
            "        Some(cwd) if is_home(cwd) => other().or_else(|| Some(cwd.to_owned())),\n",
            "        Some(cwd) if false => other().or_else(|| Some(cwd.to_owned())),\n",
            T_CWD, "rust"),
    Control("p77_subagent_no_agent_id", HOST,
            "    if is_subagent(raw) {\n",
            "    if false {\n",
            T_SUB, "rust"),
    Control("p77_subagent_stop_runs", HOST,
            "        Some(Event::SessionStart | Event::Stop) => subagent,\n",
            "        Some(Event::SessionStart | Event::Stop) => false,\n",
            T_SUB, "rust"),
    Control("p77_matcher_ignored", HOST,
            "        Some(Event::PreToolUse | Event::PostToolUse) => verb_tools(verb).is_some_and(|tools| {\n",
            '        Some(Event::PreToolUse | Event::PostToolUse) => verb_tools("none").is_some_and(|tools| {\n',
            T_MATCHER, "rust"),
    # The output
    Control("p77_context_not_injected", HOST,
            '                    json!({"injectSteps": [{"systemMessage": {"systemMessage": text}}]}),\n',
            '                    json!({"systemMessage": text}),\n',
            T_CONTEXT, "rust"),
    Control("p77_deny_spelled_block", HOST,
            '            Some(reason) => (json!({"decision": "deny", "reason": reason}), false),\n',
            '            Some(reason) => (json!({"decision": "block", "reason": reason}), false),\n',
            T_DENY, "rust"),
    Control("p77_post_tool_blocks", HOST,
            "        Some(Event::PreToolUse) => match block_reason(out) {\n",
            "        Some(Event::PreToolUse | Event::PostToolUse) => match block_reason(out) {\n",
            T_POST, "rust"),
    Control("p77_other_event_runs", HOST,
            "        None => true,\n    };\n    HookIn {\n",
            "        None => false,\n    };\n    HookIn {\n",
            T_OTHER, "rust"),
    # The Stop gate's own count (plan 41 §4.2: continuationActive never turned true)
    Control("p77_stop_no_cap", HOST,
            "            Some(reason) if asks < AGY_STOP_ASKS_PER_PROMPT => {\n",
            "            Some(reason) => {\n",
            T_STOP, "rust"),
    Control("p77_stop_not_counted", HOST,
            "        if continues {\n            count_ask(&self.state_dir, &self.raw, now);\n",
            "        if false {\n            count_ask(&self.state_dir, &self.raw, now);\n",
            T_GATE, "rust"),
    Control("p77_stop_trusts_continuation", HOST,
            "    let hook = to_claude(&raw, cwd.as_deref(), asks > 0);\n",
            '    let hook = to_claude(&raw, cwd.as_deref(), asks > 0 || raw["continuationActive"] == json!(true));\n',
            T_STOP, "rust"),
    Control("p77_stop_count_never_resets", HOST,
            "        .filter(|mark| mark.prompt == prompt_key(raw))\n        .map_or(0, |mark| mark.asks)\n",
            "        .map_or(0, |mark| mark.asks)\n",
            T_STOP, "rust"),
    # Once per prompt
    Control("p77_prompt_every_call", HOST,
            "    if marks.get(&key).is_some_and(|mark| mark.prompt == prompt) {\n",
            "    if false {\n",
            T_PROMPT, "rust"),
    Control("p77_prompt_shared_by_verbs", HOST,
            '    let key = format!("{verb}|{}", conversation(raw));\n',
            '    let key = format!("|{}", conversation(raw));\n',
            T_PROMPT, "rust"),
    Control("p77_prompt_text_kept", HOST,
            '    digest[..8].iter().map(|b| format!("{b:02x}")).collect()\n',
            '    let _ = digest;\n    text(raw, "lastUserInput").unwrap_or_default().to_owned()\n',
            T_PROMPT, "rust"),
    # The owner's words on agy refuse (plan 41 §5.1). The sabotage is a reader that takes the quote from any line.
    Control("p77_send_ok_any_line", GUI,
            "    let Some(found) = crate::nirvana_fresh::owner_quote_at(text, quote) else {\n",
            "    if text.contains(quote) {\n        return Ok(SendOk { quote: quote.into(), prompt: quote.into(), "
            "at: String::new(), transcript: String::new(), check: String::new() });\n    }\n"
            "    let Some(found) = crate::nirvana_fresh::owner_quote_at(text, quote) else {\n",
            T_OWNER, "rust"),
    # P41c scope: the installed agy plugin reaches every agy session; only the orchestrator's works.
    Control("p77_outside_orchestrator_works", HOST,
            "    host == Host::Agy && marker != Some(\"1\")\n",
            "    host == Host::Agy && marker == Some(\"never\")\n",
            T_OUTSIDE, "rust"),
    Control("p77_worker_keeps_marker", WORKER,
            "    crate::nirvana_launch::ORCHESTRATOR_ENV,\n    \"CODEX_THREAD_ID\",\n",
            "    \"NIRVANA_ORCHESTRATOR_KEPT\",\n    \"CODEX_THREAD_ID\",\n",
            T_WORKER_ENV, "rust"),
)
