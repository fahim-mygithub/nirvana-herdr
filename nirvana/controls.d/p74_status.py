# Program status reports, OSC 7501 (nirvana/plans/2026-11-14-osc7501-status.md). Controls starting with p74_. One
# sabotage per assertion, each on a product line; the test named must print FAIL with it in place (the expected red
# test is the fifth field, written before the run). `Control` is provided by controls.py.
PARSE = "src/terminal/program_status.rs"
OSC = "src/pane/osc.rs"
TERM = "src/pane/terminal.rs"
STATE = "src/terminal/state.rs"
ACTIONS = "src/app/actions.rs"
API = "src/app/api.rs"
CREATION = "src/app/creation.rs"
SEAT = "src/nirvana_worker/pane_status.rs"

_P = "terminal::program_status::tests::"
T_EXAMPLES = _P + "program_status_parses_the_spec_examples"
T_IGNORE = _P + "program_status_ignores_what_the_spec_says_to_ignore"
T_LIMIT = _P + "program_status_discards_a_report_that_breaks_a_limit"
T_TRACKER = "pane::osc::tests::program_status_report_is_kept_and_cleared_by_the_specs_events"
T_QUERY = "pane::osc::tests::program_status_query_is_counted_and_not_a_report"
T_REPLY = "pane::terminal::tests::process_pty_bytes_answers_the_program_status_query_first_and_returns_the_report"
T_WINS = "app::actions::tests::a_program_status_report_wins_over_detection_and_ends_with_the_shell"
T_DONE = "app::actions::tests::a_done_report_survives_the_shell_and_a_clear_ends_it"
T_PROBE = "app::api::tests::a_live_program_status_turns_on_the_shell_return_probe"
T_TOKENS = "app::api::panes::tests::pane_get_carries_the_program_status_and_its_message_as_tokens"
T_RESPAWN = "terminal::state::tests::a_respawned_pane_drops_its_program_status"
_S = "nirvana_worker::pane_status::tests::"
T_BEGIN = _S + "seat_exec_reports_working_with_the_order_and_its_folder"
T_END = _S + "seat_exec_end_follows_the_orders_phase"
T_DETACHED = _S + "a_detached_seat_exec_writes_no_escape"

CONTROLS = (
    # The parser (`parse_report`)
    Control("p74_state_word", PARSE,
            '        b"working" => ProgramState::Working,\n',
            '        b"working" => ProgramState::Idle,\n',
            T_EXAMPLES, "rust"),
    Control("p74_child_id_ignored", PARSE,
            "    if id.is_some() {\n",
            "    if false {\n",
            T_IGNORE, "rust"),
    Control("p74_bad_id_rejects", PARSE,
            '            if key == b"id" {\n',
            "            if false {\n",
            T_IGNORE, "rust"),
    Control("p74_kind_only_blocked", PARSE,
            "    let kind = (state == ProgramState::Blocked)\n",
            "    let kind = true\n",
            T_IGNORE, "rust"),
    Control("p74_decoded_limit", PARSE,
            "    if bytes.len() > max_decoded {\n",
            "    if false {\n",
            T_LIMIT, "rust"),
    Control("p74_control_chars", PARSE,
            "    (!text.chars().any(char::is_control)).then_some(text)\n",
            "    Some(text)\n",
            T_LIMIT, "rust"),
    # The pane's tracker (`AgentOscStateTracker`)
    Control("p74_prompt_ends_working", OSC,
            "                b\"133\" if payload.first() == Some(&b'A') => {\n",
            "                b\"133\" if false => {\n",
            T_TRACKER, "rust"),
    Control("p74_done_survives_prompt", PARSE,
            "        matches!(self, Self::Idle | Self::Working | Self::Blocked)\n",
            "        matches!(self, Self::Idle | Self::Working | Self::Blocked | Self::Done)\n",
            T_TRACKER, "rust"),
    Control("p74_full_reset", OSC,
            "                    b'c' => {\n                        receive(None);\n",
            "                    b'c' => {\n",
            T_TRACKER, "rust"),
    Control("p74_query_counted", OSC,
            "                        *program_status_queries += 1;\n",
            "                        *program_status_queries += 0;\n",
            T_QUERY, "rust"),
    # The terminal answers and hands the record on
    Control("p74_reply_first", TERM,
            "            terminal_responses.splice(\n                0..0,\n",
            "            terminal_responses.splice(\n                terminal_responses.len()..,\n",
            T_REPLY, "rust"),
    Control("p74_result_carries_report", TERM,
            "        let program_status = core.agent_osc_state.take_program_status_change();\n",
            "        let program_status = core.agent_osc_state.take_program_status_change().and(None);\n",
            T_REPLY, "rust"),
    # The terminal state: the report wins, the shell ends it, late ones are dropped
    Control("p74_report_wins", STATE,
            "        let state = if let Some(status) = &self.program_status {\n",
            "        let state = if let Some(status) = None::<&crate::terminal::program_status::ProgramStatus> {\n",
            T_WINS, "rust"),
    Control("p74_late_report_dropped", STATE,
            "        if self.program_status_at.is_some_and(|at| at > observed_at) {\n",
            "        if false {\n",
            T_WINS, "rust"),
    Control("p74_shell_before_report_keeps", STATE,
            "            && self.program_status_at.is_none_or(|at| at <= observed_at);\n",
            "            && true;\n",
            T_WINS, "rust"),
    Control("p74_shell_return_clears", STATE,
            "        self.program_status = None;\n        if let Some(mutation) = self.clear_self_reported_agent(observed_at) {\n",
            "        if let Some(mutation) = self.clear_self_reported_agent(observed_at) {\n",
            T_WINS, "rust"),
    Control("p74_respawn_clears", STATE,
            "        self.program_status = None;\n        self.program_status_at = None;\n",
            "        self.program_status_at = None;\n",
            T_RESPAWN, "rust"),
    Control("p74_app_applies_report", ACTIONS,
            "                    terminal.set_program_status(status, observed_at)\n",
            "                    terminal.set_program_status(None, observed_at)\n",
            T_WINS, "rust"),
    Control("p74_clear_applies", ACTIONS,
            "                    terminal.report_shell_returned(observed_at)\n",
            "                    terminal.clear_self_reported_agent(observed_at)\n",
            T_WINS, "rust"),
    Control("p74_shell_probe_on", API,
            "                            || terminal.program_status_ends_with_shell(),\n",
            "                            || false,\n",
            T_PROBE, "rust"),
    # `pane list` / `agent list` carry the record
    Control("p74_token_state", PARSE,
            "        tokens.insert(TOKEN.to_owned(), status.state.word().to_owned());\n",
            "",
            T_TOKENS, "rust"),
    Control("p74_token_msg", PARSE,
            "            tokens.insert(MSG_TOKEN.to_owned(), msg.clone());\n",
            "",
            T_TOKENS, "rust"),
    Control("p74_tokens_in_pane_info", CREATION,
            "                terminal.program_status.as_ref(),\n",
            "                None,\n",
            T_TOKENS, "rust"),
    # `herdr seat exec` writes its order's status to its own pane
    Control("p74_seat_osc7", SEAT,
            "    let mut bytes = cwd.map(osc7).unwrap_or_default();\n",
            "    let mut bytes = String::new();\n",
            T_BEGIN, "rust"),
    Control("p74_seat_osc7_encoded", SEAT,
            "matches!(byte, b'/' | b':' | b'.' | b'_' | b'-' | b'~')",
            "matches!(byte, b'/' | b':' | b'.' | b'_' | b'-' | b'~' | b' ')",
            T_BEGIN, "rust"),
    Control("p74_seat_asked_blocked", SEAT,
            "        Some(Phase::Asked) => ProgramState::Blocked,\n",
            "        Some(Phase::Asked) => ProgramState::Done,\n",
            T_END, "rust"),
    Control("p74_seat_failed_error", SEAT,
            "        Some(Phase::Failed | Phase::Interrupted) => ProgramState::Error,\n",
            "        Some(Phase::Failed | Phase::Interrupted) => ProgramState::Done,\n",
            T_END, "rust"),
    Control("p74_seat_asked_kind", SEAT,
            '        return report.replacen(":app=", ":kind=question:app=", 1);\n',
            "        return report;\n",
            T_END, "rust"),
    Control("p74_seat_detached_off", SEAT,
            "            on: pane.is_some() && std::io::stdout().is_terminal(),\n",
            "            on: true,\n",
            T_DETACHED, "rust"),
)
