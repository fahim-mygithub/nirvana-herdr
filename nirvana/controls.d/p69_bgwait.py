# Alpha 0.70.0: a Claude waiting on its background work says so, never a spinner, and never "running" for a task that is
# idle or a ghost. The owner, 2026-11-10: "I can foresee an issue where we easily show that a bg task is working but
# it's actually idling or ghosting". Controls starting with p69_. One sabotage per assertion, each on a product line;
# the test named must print FAIL with it in place. `Control` is provided by controls.py.
BG = "src/nirvana_bgwait.rs"
PHONE = "src/nirvana_remote/spaces.rs"
HERDR = "src/nirvana_remote/herdr.rs"
ROW = "src/client/shell/nirvana_spaces.rs"
PANEL = "src/client/shell/agent_sidebar.rs"
SPACES_JS = "src/nirvana_remote/page/spaces.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"

_T = "nirvana_bgwait::tests::"
T_LAUNCH = _T + "a_launch_alone_is_open_with_its_output_file"
T_FOREGROUND = _T + "a_foreground_command_opens_nothing"
T_NOTIFIED = _T + "a_launch_and_its_notification_is_closed"
T_STOP = _T + "a_task_stop_closes_its_task_and_a_failed_one_does_not"
T_TWO = _T + "of_two_tasks_one_finished_leaves_the_other_open"
T_COMPACT = _T + "a_compaction_between_launch_and_finish_keeps_the_task_and_still_closes_it"
T_ERROR = _T + "a_launch_whose_result_is_an_error_opens_nothing"
T_MONITOR = _T + "a_monitor_event_keeps_it_open_and_its_end_closes_it"
T_AGENT = _T + "only_a_background_agent_is_a_task"
T_TURN = _T + "the_turn_ends_on_end_turn_turn_duration_or_an_interrupt_and_not_on_a_tool"
T_SIDECHAIN = _T + "a_subagents_line_is_not_this_sessions"
T_TAIL = _T + "the_tail_reads_whole_lines_and_only_what_was_added"
T_RUNNING = _T + "alive_and_growing_is_running"
T_QUIET = _T + "alive_with_no_output_past_the_threshold_is_quiet"
T_NO_PROCESS = _T + "open_with_no_process_is_lost_never_running"
T_RESTART = _T + "a_live_process_under_a_claude_that_restarted_since_the_launch_is_lost"
T_GONE = _T + "an_output_file_that_is_gone_is_lost"
T_UNKNOWN = _T + "with_no_process_list_only_output_is_a_sign_of_life"
T_MATCH = _T + "a_process_is_the_tasks_by_its_output_file_or_its_command"
T_AGENT_FILE = _T + "a_background_agent_reads_its_own_transcript"
T_TURN_GOES_ON = _T + "nothing_is_read_while_the_turn_goes_on"
T_LOST_HIDDEN = _T + "a_lost_task_is_shown_for_an_hour_then_dropped"
T_WORDS = _T + "the_reading_in_words"
T_TOKEN = _T + "the_token_round_trips"
T_SHOWN = _T + "a_background_wait_is_never_shown_as_working"
T_FRESH = _T + "a_reading_made_under_another_state_is_not_shown"
T_PHONE = _T + "the_phone_row_says_waiting_not_working"
T_SUMMARY = _T + "the_pc_list_does_not_count_a_background_wait_as_at_work"
T_FEED = _T + "the_feed_builds_again_after_an_event_and_while_a_reading_shows"
T_PAGE = _T + "the_page_words_a_background_wait"
# Fix round 2026-11-11 (the review's findings 1-3 and a ghost Agent)
T_SHAPES = _T + "each_notification_shape_leaves_the_turn_as_measured"
T_WAKE = _T + "a_new_turn_after_a_notification_reads_as_working"
T_POLL = _T + "the_worker_reads_again_when_the_transcript_grows_or_a_shown_reading_ages"
T_AGENT_GHOST = _T + "a_background_agent_with_no_transcript_past_the_grace_is_lost"
T_MEASURED = _T + "measured_command_lines_match_their_tasks"
_C = "client::shell::tests::nirvana_space_agents::"
T_ROW = _C + "a_claude_waiting_on_background_work_says_so_and_does_not_spin"
T_PANEL = _C + "the_agents_panel_words_the_background_wait"

CONTROLS = (
    # What the transcript says is open: a background launch's result opens it, its notification or a TaskStop ends it
    Control("p69_launch_not_seen", BG,
            '"Bash" | "PowerShell" if truthy(&input["run_in_background"]) => {',
            '"Bash" | "PowerShell" if false => {', T_LAUNCH, "rust"),
    Control("p69_foreground_counted", BG,
            '"Bash" | "PowerShell" if truthy(&input["run_in_background"]) => {',
            '"Bash" | "PowerShell" => {', T_FOREGROUND, "rust"),
    Control("p69_notification_ignored", BG,
            '            if tag(part, "status").is_some() {\n                self.close(task);',
            '            if false {\n                self.close(task);', T_NOTIFIED, "rust"),
    Control("p69_attachment_ignored", BG,
            'Some("attachment") if entry["attachment"]["type"] == "queued_command" => {',
            'Some("attachment") if false => {', T_NOTIFIED, "rust"),
    Control("p69_monitor_event_closes", BG,
            '            if tag(part, "status").is_some() {',
            '            if tag(part, "task-id").is_some() {', T_MONITOR, "rust"),
    Control("p69_monitor_event_not_output", BG,
            "open.last_event_at = at.or(open.last_event_at);",
            "open.last_event_at = None;", T_MONITOR, "rust"),
    Control("p69_stop_ignored", BG,
            "            if !failed {\n                self.close(&task);",
            "            if false {\n                self.close(&task);", T_STOP, "rust"),
    Control("p69_failed_stop_closes", BG,
            "            if !failed {\n                self.close(&task);",
            "            if true {\n                self.close(&task);", T_STOP, "rust"),
    Control("p69_close_closes_all", BG,
            "    fn close(&mut self, task: &str) {\n        self.open.retain(|open| open.id != task);",
            "    fn close(&mut self, task: &str) {\n        let _ = task;\n        self.open.clear();", T_TWO, "rust"),
    Control("p69_compaction_resets", BG,
            '            Some("system") if entry["subtype"] == "turn_duration" => self.turn_ended = true,',
            '            Some("system") if entry["subtype"] == "compact_boundary" => self.open.clear(),\n'
            '            Some("system") if entry["subtype"] == "turn_duration" => self.turn_ended = true,',
            T_COMPACT, "rust"),
    Control("p69_error_opens", BG,
            "        if failed {\n            return;\n        }\n",
            "", T_ERROR, "rust"),
    Control("p69_foreground_agent_counted", BG,
            "                if !background {\n                    return;\n                }\n",
            "", T_AGENT, "rust"),
    # When the turn ended: end_turn, an interrupt; a typed prompt starts one; a subagent's line is not this turn
    Control("p69_end_turn_ignored", BG,
            '.is_some_and(|reason| reason != "tool_use");',
            '.is_some_and(|reason| reason == "never");', T_TURN, "rust"),
    Control("p69_prompt_keeps_ended", BG,
            "        } else if !start.is_empty() {\n            self.turn_ended = false;",
            "        } else if false {\n            self.turn_ended = false;", T_TURN, "rust"),
    Control("p69_interrupt_ignored", BG,
            'start.starts_with("[Request interrupted")',
            'start.starts_with("[never")', T_TURN, "rust"),
    Control("p69_sidechain_read", BG,
            '        if entry["isSidechain"] == true {',
            "        if false {", T_SIDECHAIN, "rust"),
    # The tail: whole lines only, and a file that shrank is read again
    Control("p69_tail_reads_half_lines", BG,
            "        let Some(last_newline) = bytes.iter().rposition(|b| *b == b'\\n') else {",
            "        let Some(last_newline) = Some(bytes.len() - 1) else {", T_TAIL, "rust"),
    Control("p69_tail_shrink_kept", BG,
            "        if len < self.offset || len - self.offset > FIRST_READ_MOST {",
            "        if len.saturating_sub(self.offset) > FIRST_READ_MOST {", T_TAIL, "rust"),
    # The evidence: never running without a process, a restart and a gone file are lost, quiet past the threshold
    Control("p69_no_process_runs", BG,
            "                        return reading(State::Lost, last_output, Some(Why::NoProcess));\n",
            "", T_NO_PROCESS, "rust"),
    Control("p69_restart_ignored", BG,
            ".is_some_and(|started| started > task.launched_at.saturating_add(RESTART_SLACK_S))",
            ".is_some_and(|started| started == u64::MAX)", T_RESTART, "rust"),
    Control("p69_restart_no_slack", BG,
            ".is_some_and(|started| started > task.launched_at.saturating_add(RESTART_SLACK_S))",
            ".is_some_and(|started| started >= task.launched_at)", T_RESTART, "rust"),
    Control("p69_output_gone_ignored", BG,
            "                    None => return reading(State::Lost, None, Some(Why::OutputGone)),",
            "                    None => {}", T_GONE, "rust"),
    Control("p69_never_quiet", BG,
            "if active_at.is_some_and(|at| evidence.now.saturating_sub(at) <= QUIET_AFTER_S) {",
            "if active_at.is_some() {", T_QUIET, "rust"),
    Control("p69_growth_ignored", BG,
            "                    Some(stat) => last_output = last_output.max(wrote(stat)),",
            "                    Some(_) => {}", T_RUNNING, "rust"),
    Control("p69_unknown_process_runs", BG,
            "                None => last_output,",
            "                None => Some(last_output.unwrap_or(0).max(task.launched_at)),", T_UNKNOWN, "rust"),
    Control("p69_match_by_command_off", BG,
            "    head.chars().count() >= 4 && squash(command_line).contains(&head)",
            "    head.chars().count() >= 4 && false", T_MATCH, "rust"),
    Control("p69_match_by_file_off", BG,
            '    if command_line.contains(&format!("{}.output", task.id)) {',
            "    if false {", T_MATCH, "rust"),
    Control("p69_quotes_kept", BG,
            ".filter(|c| !matches!(c, '\\'' | '\"' | '\\\\'))",
            ".filter(|_| true)", T_MATCH, "rust"),
    Control("p69_agent_transcript_ignored", BG,
            "                    Some(stat) => last_output = wrote(stat).max(last_output),",
            "                    Some(_) => {}", T_AGENT_FILE, "rust"),
    # The pane's reading: none while the turn goes on, a ghost dropped after an hour, the words, the token
    Control("p69_read_while_turn_runs", BG,
            "    if !ledger.turn_ended || ledger.open.is_empty() {",
            "    if ledger.open.is_empty() {", T_TURN_GOES_ON, "rust"),
    Control("p69_lost_never_dropped", BG,
            "                || evidence.now.saturating_sub(reading.started) <= LOST_HIDE_AFTER_S",
            "                || true", T_LOST_HIDDEN, "rust"),
    Control("p69_words_no_last_output", BG,
            '            (State::Running, Some(at)) => format!("last output {} ago", since(at)),',
            '            (State::Running, Some(_)) => "no output yet".to_string(),', T_WORDS, "rust"),
    Control("p69_token_not_found", BG,
            "        .find(|(name, _)| *name == TOKEN)",
            '        .find(|(name, _)| *name == "nirvana_model")', T_TOKEN, "rust"),
    Control("p69_wait_shown_working", BG,
            "        (AgentStatus::Working, Some(_)) => AgentStatus::Idle,",
            "        (AgentStatus::Working, Some(_)) => AgentStatus::Working,", T_SHOWN, "rust"),
    Control("p69_reading_any_state", BG,
            "    read_for == Some(now)\n",
            "    let _ = (read_for, now);\n    true\n", T_FRESH, "rust"),
    # The window: the space row's word, its sparkle, the agents panel's line and icon
    Control("p69_row_word_kept", ROW,
            "        (Some((long, _)), true) => long,",
            "        (Some(_), true) => status_word(agent.status),", T_ROW, "rust"),
    Control("p69_space_sparkles", ROW,
            "        .any(|agent| busy(shown_agent_status(agent)) && member(&agent.workspace_id))",
            "        .any(|agent| busy(agent.agent_status) && member(&agent.workspace_id))", T_ROW, "rust"),
    Control("p69_panel_no_line", PANEL,
            "        bg_line: bg_text,",
            "        bg_line: None,", T_PANEL, "rust"),
    Control("p69_panel_icon_working", PANEL,
            "    Some(AgentRow {\n        pane_id: agent.pane_id.clone(),\n        status,",
            "    Some(AgentRow {\n        pane_id: agent.pane_id.clone(),\n        status: agent.agent_status,",
            T_PANEL, "rust"),
    # The phone: the row says waiting, keeps needs, a ghost is not working; the PC list; the feed's own builds
    Control("p69_phone_working", PHONE,
            '            crate::nirvana_bgwait::Shown::Waiting | crate::nirvana_bgwait::Shown::Quiet => "bg",',
            '            crate::nirvana_bgwait::Shown::Waiting | crate::nirvana_bgwait::Shown::Quiet => "working",',
            T_PHONE, "rust"),
    Control("p69_phone_blocked_hidden", PHONE,
            '    if let Some(view) = background(agent).filter(|_| herdr_status != "blocked") {',
            "    if let Some(view) = background(agent) {", T_PHONE, "rust"),
    Control("p69_phone_ghost_working", PHONE,
            '            crate::nirvana_bgwait::Shown::Lost if herdr_status == "working" => "idle",',
            '            crate::nirvana_bgwait::Shown::Lost if false => "idle",', T_PHONE, "rust"),
    Control("p69_summary_counts_wait", HERDR,
            '                Some("working") => super::spaces::background(agent).is_none(),',
            '                Some("working") => true,', T_SUMMARY, "rust"),
    Control("p69_feed_no_follow_up", PHONE,
            "    if after_event {\n        Some(FOLLOW_UP)",
            "    if false {\n        Some(FOLLOW_UP)", T_FEED, "rust"),
    Control("p69_feed_no_background_tick", PHONE,
            "    } else if background {\n        Some(BACKGROUND_EVERY)",
            "    } else if false {\n        Some(BACKGROUND_EVERY)", T_FEED, "rust"),
    # The page (read as source, as p68's page control does)
    Control("p69_page_line_dropped", SPACES_JS,
            "text: (agent.bg && agent.bg.line) || agent.line || ''",
            "text: agent.line || ''", T_PAGE, "rust"),
    Control("p69_chat_keeps_old_reading", CHAT_JS,
            "        agent: (data) => { agent = Object.assign(agent, { bg: null }, data); drawHead(); },",
            "        agent: (data) => { agent = Object.assign(agent, data); drawHead(); },", T_PAGE, "rust"),
    # Fix round 2026-11-11. A dequeue and a queued_command attachment start the turn, an enqueue alone does not, a
    # remove closes what it carries; the tail sees growth; the worker reads again on growth and while a reading shows;
    # an Agent with no transcript past the grace is lost; measured command lines match and no plain shell is a task.
    Control("p69_dequeue_keeps_ended", BG,
            'Some("dequeue") => self.turn_ended = false,',
            'Some("dequeue") => {}', T_SHAPES, "rust"),
    Control("p69_dequeue_keeps_reading", BG,
            'Some("dequeue") => self.turn_ended = false,',
            'Some("dequeue") => {}', T_WAKE, "rust"),
    Control("p69_attachment_keeps_ended", BG,
            "                // A queued input handed to the model as an attachment is part of a turn at work.\n"
            "                self.turn_ended = false;\n",
            "", T_SHAPES, "rust"),
    Control("p69_enqueue_starts_turn", BG,
            'Some("enqueue" | "remove") => {\n                    if let Some(content)',
            'Some("enqueue" | "remove") => {\n                    self.turn_ended = false;\n'
            '                    if let Some(content)', T_SHAPES, "rust"),
    Control("p69_remove_ignored", BG,
            'Some("enqueue" | "remove") => {',
            'Some("enqueue") => {', T_SHAPES, "rust"),
    Control("p69_growth_never_seen", BG,
            "std::fs::metadata(&self.path).is_ok_and(|meta| meta.len() != self.len_seen)",
            "std::fs::metadata(&self.path).is_err()", T_WAKE, "rust"),
    Control("p69_len_seen_unset", BG,
            "        self.len_seen = len;\n",
            "", T_WAKE, "rust"),
    Control("p69_poll_ignores_growth", BG,
            "    grew || (shown && since_read >= REFRESH)",
            "    let _ = grew;\n    shown && since_read >= REFRESH", T_POLL, "rust"),
    Control("p69_poll_never_refreshes", BG,
            "    grew || (shown && since_read >= REFRESH)",
            "    let _ = (shown, since_read);\n    grew", T_POLL, "rust"),
    Control("p69_agent_no_file_runs", BG,
            "                        return reading(State::Lost, last_output, Some(Why::OutputGone));\n",
            "", T_AGENT_GHOST, "rust"),
    Control("p69_agent_no_grace", BG,
            "None if evidence.now.saturating_sub(task.launched_at) > AGENT_FILE_GRACE_S => {",
            "None if evidence.now >= task.launched_at => {", T_AGENT_GHOST, "rust"),
    Control("p69_measured_quotes_kept", BG,
            ".filter(|c| !matches!(c, '\\'' | '\"' | '\\\\'))",
            ".filter(|_| true)", T_MEASURED, "rust"),
    Control("p69_any_shell_is_task", BG,
            '    if command_line.contains(&format!("{}.output", task.id)) {',
            '    if command_line.contains("bash.exe") {', T_MEASURED, "rust"),
)
