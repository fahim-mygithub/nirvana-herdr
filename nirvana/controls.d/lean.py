# Plan 25: a leaner Nirvana Claude (package `lean`): §2 a fresh session after a run ends, §3 a lean start, §5 browser
# batches. Controls starting with p25_. Each breaks one product line and names the test that must go red.

T_TRIGGER = 'fresh_trigger_fires_only_after_a_run_end_and_above_fresh_above'
T_LAST_END = 'the_last_end_is_this_panes_latest_ended_run'
T_SEATS = 'seats_toml_carries_fresh_above_and_an_older_table_defaults_it'
T_STOP = 'the_stop_hook_records_a_pending_start_and_takes_it_back'
T_BAD_RECORD = 'a_record_that_does_not_parse_is_a_doubt_not_none'
T_NOTE = 'the_note_keeps_its_budget_and_holds_the_open_runs_and_the_last_prompts'
T_PROMPTS = 'the_owners_prompts_are_the_last_three_they_typed_not_herdrs_or_claudes'
T_START = 'a_clear_prints_the_note_journals_fresh_and_forgets_the_record'
T_SENDS = 'the_fresh_start_sends_when_every_rule_holds'
T_COMPACT = 'the_fresh_start_waits_for_an_idle_compacts_paste'
T_RECORD = 'the_fresh_start_needs_a_record_it_has_not_cleared_for'
T_NIRVANA = 'the_fresh_start_types_only_into_nirvana_claude'
T_BUSY = 'the_fresh_start_types_nothing_into_a_busy_pane'
T_OWNER = 'the_fresh_start_types_nothing_while_the_owner_is_at_the_pane'
T_LINE = 'the_fresh_start_types_nothing_unless_the_line_is_provably_empty'
T_AFTER = 'after_the_clear_paste_reads_for_clear_with_idle_compacts_rule'
T_SCHEDULE = 'the_fresh_stretch_looks_up_then_polls_and_never_spins'
T_ENDS = 'the_fresh_stretch_ends_with_the_idle_stretch'
T_S_SEND = 'fresh_server_types_clear_then_enter_into_an_idle_nirvana_claude'
T_S_NONE = 'fresh_server_with_no_record_types_nothing_and_looks_again_later'
T_S_PLAIN = 'fresh_server_types_only_into_nirvana_claude'
T_S_OWNER = 'fresh_server_types_nothing_while_the_owner_types_or_the_line_holds_text'
T_S_MARK = 'fresh_server_never_clears_twice_for_one_record'
T_S_ERASE = 'fresh_server_erases_a_clear_whose_enter_never_went'
T_S_RACE = 'fresh_start_and_idle_compact_never_share_a_pane'
T_S_LOOP = 'fresh_deadline_reaches_the_server_loop'
T_S_NAME = 'fresh_deadline_counts_only_panes_started_as_nirvana_claude'
T_NEEDED = 'no_tool_nirvana_claude_needs_is_cut'
T_CUTS = 'the_cut_list_is_the_three_the_probe_passed'
T_DOUBT = 'a_doubt_keeps_todays_argv'
T_LAUNCH = 'the_launch_carries_the_probes_cut_list_before_the_resume'
T_BROWSER = 'only_a_browser_order_gets_the_bladebro_line'
T_BRIEF = 'a_browser_orders_brief_holds_the_line_and_a_build_orders_does_not'

F = 'src/nirvana_fresh.rs'
S = 'src/app/nirvana_fresh.rs'
L = 'src/nirvana_lean.rs'

CONTROLS = (
    # §2 the trigger: no ended run of this pane, no fresh start
    Control(
        'p25_trigger_needs_an_end', F,
        '    let Some((run, ended_at)) = last_end else {\n        return Err("no run of this pane has ended".into());\n    };\n',
        '    let (run, ended_at) = last_end.unwrap_or(("none", u64::MAX));\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the trigger: an unknown session start is no fresh start
    Control(
        'p25_trigger_needs_the_start', F,
        '    let Some(began) = session_began else {\n        return Err("the session\'s start is not known".into());\n    };\n',
        '    let began = session_began.unwrap_or(0);\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the trigger: the run must have ended since the session's last clear
    Control(
        'p25_trigger_since_the_clear', F,
        '    if ended_at < began {\n',
        '    if false && ended_at < began {\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the trigger: an end in the clear's own second counts
    Control(
        'p25_trigger_same_second', F,
        '    if ended_at < began {\n',
        '    if ended_at <= began {\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the trigger: an unread context is no fresh start
    Control(
        'p25_trigger_needs_the_context', F,
        '    let Some(context) = context else {\n        return Err("the session\'s context could not be read".into());\n    };\n',
        '    let context = context.unwrap_or(u64::MAX);\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the trigger: above fresh_above, not at it
    Control(
        'p25_trigger_above', F,
        '    if context <= fresh_above {\n',
        '    if context < fresh_above {\n',
        T_TRIGGER, 'rust',
    ),
    # §2 fresh_above is 120k to start
    Control(
        'p25_fresh_above_value', F,
        'pub const FRESH_ABOVE: u64 = 120_000;\n',
        'pub const FRESH_ABOVE: u64 = 100_000;\n',
        T_TRIGGER, 'rust',
    ),
    # §2 the run this pane ended: other panes' runs do not count
    Control(
        'p25_last_end_this_pane', F,
        '                == Some(pane)\n',
        '                != None\n',
        T_LAST_END, 'rust',
    ),
    # §2 the run this pane last ended: the latest end
    Control(
        'p25_last_end_latest', F,
        '        .max_by_key(|(_, at)| *at)\n',
        '        .min_by_key(|(_, at)| *at)\n',
        T_LAST_END, 'rust',
    ),
    # §2 fresh_above lives in seats.toml
    Control(
        'p25_seats_toml_fresh_above', 'src/nirvana_seats.toml',
        'fresh_above = 120000\n',
        'fresh_above = 1\n',
        T_SEATS, 'rust',
    ),
    # §2 a table written before plan 25 reads fresh_above as 120k
    Control(
        'p25_seats_default_fresh_above', 'src/nirvana_seat.rs',
        '    crate::nirvana_fresh::FRESH_ABOVE\n}\n',
        '    0\n}\n',
        T_SEATS, 'rust',
    ),
    # §2 the Stop hook writes the pending record
    Control(
        'p25_stop_writes', F,
        '            write_record(home, pane, &record)?;\n',
        '',
        T_STOP, 'rust',
    ),
    # §2 the record carries the context before the clear
    Control(
        'p25_stop_context', F,
        '                context,\n                transcript: Some(transcript.display().to_string()),\n',
        '                context: None,\n                transcript: Some(transcript.display().to_string()),\n',
        T_STOP, 'rust',
    ),
    # §2 a later turn keeps the record's second, which the server's mark names
    Control(
        'p25_stop_keeps_its_second', F,
        '                .map_or(now, |record| record.at);\n',
        '                .map_or(now, |_| now);\n',
        T_STOP, 'rust',
    ),
    # §2 a record the rule no longer holds for is taken back
    Control(
        'p25_stop_takes_back', F,
        '                forget(home, pane);\n            }\n            Ok(Err(why))\n',
        '            }\n            Ok(Err(why))\n',
        T_STOP, 'rust',
    ),
    # §2 only the Stop hook's own record is taken back, never the owner's
    Control(
        'p25_stop_spares_the_owner', F,
        '                .is_some_and(|record| record.by == "stop")\n',
        '                .is_some_and(|_| true)\n',
        T_STOP, 'rust',
    ),
    # §2 a record that does not parse is a doubt, not "none"
    Control(
        'p25_record_doubt', F,
        '    serde_json::from_str(&text)\n        .map(Some)\n        .map_err(|err| format!("{} is not a record: {err}", path.display()))\n',
        '    Ok(serde_json::from_str(&text).ok())\n',
        T_BAD_RECORD, 'rust',
    ),
    # §2 the note's last line
    Control(
        'p25_note_closing', F,
        '            "This is a fresh session after run {run}. Ask the owner before assuming anything the note does not say."\n',
        '            "This is a fresh session after {run}. Ask the owner before assuming anything the note does not say."\n',
        T_NOTE, 'rust',
    ),
    # §2 the note keeps to its budget
    Control(
        'p25_note_budget', F,
        '    let mut text = cut(&body, room);\n',
        '    let mut text = body.clone();\n',
        T_NOTE, 'rust',
    ),
    # §2 the budget is 2,000 tokens
    Control(
        'p25_note_budget_tokens', F,
        'pub const NOTE_MAX_TOKENS: usize = 2_000;\n',
        'pub const NOTE_MAX_TOKENS: usize = 4_000;\n',
        T_NOTE, 'rust',
    ),
    # §2 measured as characters / 4
    Control(
        'p25_note_chars_per_token', F,
        'pub const NOTE_MAX_CHARS: usize = NOTE_MAX_TOKENS * 4;\n',
        'pub const NOTE_MAX_CHARS: usize = NOTE_MAX_TOKENS * 5;\n',
        T_NOTE, 'rust',
    ),
    # §2 the note's size, characters / 4 rounded up
    Control(
        'p25_note_tokens', F,
        '    note.chars().count().div_ceil(4) as u64\n',
        '    (note.chars().count() / 4) as u64\n',
        T_NOTE, 'rust',
    ),
    # §2 each prompt has its own cap, so all three show
    Control(
        'p25_note_prompt_cap', F,
        '                cut(prompt, PROMPT_MAX_CHARS).replace(\'\\n\', "\\n> ")\n',
        '                prompt.replace(\'\\n\', "\\n> ")\n',
        T_NOTE, 'rust',
    ),
    # §2 the prompts are quoted
    Control(
        'p25_note_prompts_quoted', F,
        '                "> {}\\n",\n',
        '                "{}\\n",\n',
        T_NOTE, 'rust',
    ),
    # §2 every open run, with its orders and phases
    Control(
        'p25_note_open_runs', F,
        '            body.push_str(&format!("- {run}: {orders}\\n"));\n',
        '            body.push_str(&format!("- {run}\\n"));\n',
        T_NOTE, 'rust',
    ),
    # §2 the open runs past the note's five are counted, not named
    Control(
        'p25_note_runs_bound', F,
        '        for (run, orders) in input.open.iter().take(NOTE_RUNS) {\n',
        '        for (run, orders) in input.open.iter() {\n',
        T_NOTE, 'rust',
    ),
    # §2 the board's open items
    Control(
        'p25_note_board', F,
        '            body.push_str(&format!("- {name}: {}\\n", cut(first, 160)));\n',
        '            body.push_str(&format!("- {name}\\n"));\n',
        T_NOTE, 'rust',
    ),
    # §2 the lessons index line
    Control(
        'p25_note_lessons', F,
        '        body.push_str(&format!("\\n{lessons}\\n"));\n',
        '',
        T_NOTE, 'rust',
    ),
    # §2 the ended run's receipt line
    Control(
        'p25_note_ended', F,
        '        body.push_str(&format!("\\nThe run that ended: {ended}\\n"));\n',
        '',
        T_NOTE, 'rust',
    ),
    # §2 the owner's prompts: Claude Code's meta lines are not the owner's
    Control(
        'p25_prompts_not_meta', F,
        '        || value.get("isMeta").and_then(Value::as_bool) == Some(true)\n',
        '',
        T_PROMPTS, 'rust',
    ),
    # §2 the owner's prompts: a subagent's lines are not the owner's
    Control(
        'p25_prompts_not_sidechain', F,
        '        || value.get("isSidechain").and_then(Value::as_bool) == Some(true)\n',
        '',
        T_PROMPTS, 'rust',
    ),
    # §2 the owner's prompts: a tool result is not a prompt
    Control(
        'p25_prompts_not_tool_results', F,
        '                .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))\n',
        '                .any(|_| false)\n',
        T_PROMPTS, 'rust',
    ),
    # §2 the owner's prompts: herdr's own wake digests are not the owner's
    Control(
        'p25_prompts_not_herdr', F,
        '        "herdr run ",\n',
        '',
        T_PROMPTS, 'rust',
    ),
    # §2 the owner's prompts: Claude Code's command echoes are not the owner's
    Control(
        'p25_prompts_not_commands', F,
        '        "<command-",\n',
        '',
        T_PROMPTS, 'rust',
    ),
    # §2 the owner's last prompts, oldest first
    Control(
        'p25_prompts_oldest_first', F,
        '    prompts.reverse();\n',
        '',
        T_PROMPTS, 'rust',
    ),
    # §2 the note comes on a /clear only; a resume leaves the record
    Control(
        'p25_start_on_clear_only', F,
        '        "clear" => {}\n',
        '        "clear" | "resume" => {}\n',
        T_START, 'rust',
    ),
    # §2 a new session is already fresh: its record is forgotten
    Control(
        'p25_start_startup_forgets', F,
        '        "startup" => {\n            forget(home, pane);\n',
        '        "startup" => {\n',
        T_START, 'rust',
    ),
    # §2 the journal's fresh event has the context before the clear
    Control(
        'p25_journal_context', F,
        '                    context: record.context,\n',
        '                    context: None,\n',
        T_START, 'rust',
    ),
    # §2 the journal's fresh event has the note's size
    Control(
        'p25_journal_note_size', F,
        '                    note_tokens: note_tokens(&text),\n',
        '                    note_tokens: 0,\n',
        T_START, 'rust',
    ),
    # §2 the delivered note's record is removed
    Control(
        'p25_start_forgets', F,
        '    forget(home, pane);\n    Some(text)\n',
        '    Some(text)\n',
        T_START, 'rust',
    ),
    # §2 forgetting removes the server's mark too
    Control(
        'p25_forget_mark', F,
        '    let _ = std::fs::remove_file(sent_path(home, pane));\n',
        '',
        T_START, 'rust',
    ),
    # §2 the server's rule: no paste while a compact's paste is in flight
    Control(
        'p25_decide_compact_in_flight', F,
        '    if facts.compact_in_flight {\n',
        '    if false && facts.compact_in_flight {\n',
        T_COMPACT, 'rust',
    ),
    # §2 the server's rule: an unreadable record is a doubt
    Control(
        'p25_decide_record_doubt', F,
        '        Err(why) => return skip(why.clone()),\n',
        '        Err(_) => return Decision::Nothing,\n',
        T_RECORD, 'rust',
    ),
    # §2 the server's rule: one record is cleared for at most once
    Control(
        'p25_decide_sent_for', F,
        '    if facts.sent_for == Some(record.at) {\n',
        '    if false && facts.sent_for == Some(record.at) {\n',
        T_RECORD, 'rust',
    ),
    # §2 the wake's rule: only into Nirvana Claude
    Control(
        'p25_decide_nirvana_only', F,
        '        Some(false) => return skip("the pane does not run Nirvana Claude".into()),\n',
        '        Some(false) => {}\n',
        T_NIRVANA, 'rust',
    ),
    # §2 the wake's rule: an unread process is not Nirvana Claude
    Control(
        'p25_decide_nirvana_unread', F,
        '        None => return skip("the pane\'s process could not be read".into()),\n',
        '        None => {}\n',
        T_NIRVANA, 'rust',
    ),
    # §2 the wake's rule: only while it is idle
    Control(
        'p25_decide_idle', F,
        '    if facts.state != AgentState::Idle {\n',
        '    if false && facts.state != AgentState::Idle {\n',
        T_BUSY, 'rust',
    ),
    # §2 the wake's rule: only after the owner has been quiet for OWNER_QUIET_S
    Control(
        'p25_decide_owner_quiet', F,
        '    if let Some(why) = crate::nirvana_idle_compact::owner_quiet_breach(&facts.owner_key) {\n',
        '    if let Some(why) = None::<String> {\n',
        T_OWNER, 'rust',
    ),
    # §2 every rule holding sends
    Control(
        'p25_decide_send', F,
        '        InputLine::Empty => Decision::Send(record.at),\n',
        '        InputLine::Empty => Decision::Nothing,\n',
        T_SENDS, 'rust',
    ),
    # §2 the wake's rule: text on the line is left alone
    Control(
        'p25_decide_text', F,
        '        InputLine::Text(_) => skip("the input line holds text".into()),\n',
        '        InputLine::Text(_) => Decision::Send(record.at),\n',
        T_LINE, 'rust',
    ),
    # §2 the wake's rule: a line not provably empty is left alone
    Control(
        'p25_decide_unsure', F,
        '        InputLine::Unsure(why) => skip(format!("the input line is not provably empty: {why}")),\n',
        '        InputLine::Unsure(_) => Decision::Send(record.at),\n',
        T_LINE, 'rust',
    ),
    # §2 the command typed is /clear
    Control(
        'p25_command_is_clear', F,
        'pub const CLEAR: &str = "/clear";\n',
        'pub const CLEAR: &str = "/clean";\n',
        T_AFTER, 'rust',
    ),
    # §2 one Backspace per character of /clear
    Control(
        'p25_erase_count', F,
        'pub const CLEAR_ERASE_KEYS: usize = CLEAR.len();\n',
        'pub const CLEAR_ERASE_KEYS: usize = CLEAR.len() + 2;\n',
        T_AFTER, 'rust',
    ),
    # §2 Enter only at a read of exactly /clear (idle compact's rule, read for /clear)
    Control(
        'p25_after_reads_clear', F,
        '        InputLine::Text(text) if text == CLEAR => {\n',
        '        InputLine::Text(text) if text == "/x" => {\n',
        T_AFTER, 'rust',
    ),
    # §2 a line reading /compact is not this paste
    Control(
        'p25_after_compact_is_a_doubt', F,
        '        InputLine::Text(text) if text == crate::nirvana_idle_compact::COMMAND => {\n',
        '        InputLine::Text(text) if text == "/x" => {\n',
        T_AFTER, 'rust',
    ),
    # §2 the schedule: a paste's reads come first
    Control(
        'p25_schedule_paste_first', F,
        '        if let Some(pending) = &self.pending {\n            return pending.due();\n        }\n        if self.sent {\n',
        '        if self.sent {\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 the schedule: once sent, nothing more in the stretch
    Control(
        'p25_schedule_sent', F,
        '        if self.sent {\n            return None;\n        }\n',
        '',
        T_SCHEDULE, 'rust',
    ),
    # §2 the schedule: the lookups after the stretch begins
    Control(
        'p25_schedule_lookups', F,
        '            (Some(after), _) => since.checked_add(*after)?,\n',
        '            (Some(_), _) => since,\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 the schedule: a record left for now waits the owner's quiet window, else the minute's poll
    Control(
        'p25_schedule_retry', F,
        '            (None, Some(checked)) => checked.checked_add(if self.found { RETRY } else { POLL })?,\n',
        '            (None, Some(checked)) => checked.checked_add(POLL)?,\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 the schedule: never sooner than a second after the last check
    Control(
        'p25_schedule_min_gap', F,
        '            Some(checked) => Some(at.max(checked.checked_add(MIN_GAP)?)),\n',
        '            Some(_) => Some(at),\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 the lookups are at 2 s, 10 s and 30 s
    Control(
        'p25_schedule_lookup_times', F,
        '    Duration::from_secs(10),\n    Duration::from_secs(30),\n];\n',
        '    Duration::from_secs(10),\n    Duration::from_secs(31),\n];\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 a /clear in flight is what holds a compact off
    Control(
        'p25_busy', F,
        '    pub fn busy(&self) -> bool {\n        self.pending.is_some()\n',
        '    pub fn busy(&self) -> bool {\n        false\n',
        T_SCHEDULE, 'rust',
    ),
    # §2 the fresh state lives on the idle stretch, so any other state ends it
    Control(
        'p25_stretch_holds_fresh', 'src/nirvana_idle_compact.rs',
        '        if state != AgentState::Idle {\n            return Self::default();\n        }\n',
        '        if state != AgentState::Idle {\n            return Self { fresh: self.fresh.clone(), ..Self::default() };\n        }\n',
        T_ENDS, 'rust',
    ),
    # §2 the server: a paste's reads are taken on their pass
    Control(
        'p25_server_after_paste_pass', S,
        '        if let Some(pending) = terminal.idle_stretch.fresh.pending.take() {\n',
        '        if let Some(pending) = None::<Pending> {\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: a check is stamped
    Control(
        'p25_server_stamps', S,
        '        fresh.checked_at = Some(now);\n',
        '',
        T_S_NONE, 'rust',
    ),
    # §2 the server: a check is counted, for the lookups
    Control(
        'p25_server_counts', S,
        '        fresh.checks += 1;\n',
        '',
        T_S_NONE, 'rust',
    ),
    # §2 the server: a compact's paste in flight is a fact
    Control(
        'p25_server_compact_fact', S,
        '        let compact_in_flight = terminal.idle_stretch.pending.is_some();\n',
        '        let compact_in_flight = false;\n',
        T_S_RACE, 'rust',
    ),
    # §2 the server: the pane's own record
    Control(
        'p25_server_reads_the_record', S,
        '        let record = crate::nirvana_fresh::read_record(home, &pane.public);\n',
        '        let record: Result<Option<crate::nirvana_fresh::Record>, String> = Ok(None);\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: the process is read only when a record waits
    Control(
        'p25_server_reads_the_process_only_for_a_record', S,
        '        let nirvana = if waiting && !compact_in_flight {\n',
        '        let nirvana = if true {\n',
        T_S_NONE, 'rust',
    ),
    # §2 the server: Nirvana Claude is told by its argv
    Control(
        'p25_server_nirvana_argv', S,
        '            .map(|argv| is_nirvana(&argv, home))\n',
        '            .map(|_| true)\n',
        T_S_PLAIN, 'rust',
    ),
    # §2 the server: the mark is a fact
    Control(
        'p25_server_mark_fact', S,
        '            sent_for: crate::nirvana_fresh::read_sent(home, &pane.public),\n',
        '            sent_for: None,\n',
        T_S_MARK, 'rust',
    ),
    # §2 the server: the owner's last key is a fact
    Control(
        'p25_server_owner_fact', S,
        '            owner_key: owner_key_of(&pane.public),\n',
        '            owner_key: crate::nirvana_idle_compact::OwnerKey::Never,\n',
        T_S_OWNER, 'rust',
    ),
    # §2 the server: the strict input line, from the pane's screen
    Control(
        'p25_server_line_fact', S,
        '        let input_line = runtime.map_or_else(\n            || InputLine::Unsure("the pane has no runtime".into()),\n            input_line_of,\n        );\n',
        '        let input_line = InputLine::Empty;\n',
        T_S_OWNER, 'rust',
    ),
    # §2 the server: a record left for now waits the quiet window
    Control(
        'p25_server_found', S,
        '                    terminal.idle_stretch.fresh.found = true;\n',
        '',
        T_S_PLAIN, 'rust',
    ),
    # §2 the server: the paste is /clear
    Control(
        'p25_server_pastes_clear', S,
        '        let (text, _) = super::api_helpers::encode_api_submission_parts(runtime, CLEAR);\n',
        '        let (text, _) = super::api_helpers::encode_api_submission_parts(runtime, "/x");\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: the paste remembers its record, for the mark
    Control(
        'p25_server_record_at', S,
        '            fresh.record_at = Some(at);\n',
        '            fresh.record_at = None;\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: Enter goes to the pane
    Control(
        'p25_server_enter', S,
        '                if let Err(err) = write_keys(runtime, enter) {\n',
        '                if let Err(err) = write_keys(runtime, Vec::new()) {\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: once Enter went the stretch is sent
    Control(
        'p25_server_marks_sent', S,
        '                        terminal.idle_stretch.fresh.sent = true;\n',
        '',
        T_S_SEND, 'rust',
    ),
    # §2 the server: once Enter went the mark names the record
    Control(
        'p25_server_writes_the_mark', S,
        '                    if let Err(err) = crate::nirvana_fresh::write_sent(home, &public, at) {\n',
        '                    if let Err(err) = Ok::<(), std::io::Error>(()) {\n',
        T_S_SEND, 'rust',
    ),
    # §2 the server: an Empty read waits for the next
    Control(
        'p25_server_next_read', S,
        '                    terminal.idle_stretch.fresh.pending = Some(Pending { step, ..pending });\n',
        '',
        T_S_ERASE, 'rust',
    ),
    # §2 the server: the erase sends one Backspace per character of /clear
    Control(
        'p25_server_erase_keys', S,
        '        .repeat(CLEAR_ERASE_KEYS)\n',
        '        .repeat(8)\n',
        T_S_ERASE, 'rust',
    ),
    # §2 the server: after the erase the line is read once more
    Control(
        'p25_server_reads_after_the_erase', S,
        '                            step: Step::Erased,\n',
        '                            step: Step::Erase,\n',
        T_S_ERASE, 'rust',
    ),
    # §2 the server: only panes herdr started as Nirvana Claude
    Control(
        'p25_server_nirvana_name', S,
        '                .is_some_and(crate::nirvana_launch::is_nirvana_agent_name)\n',
        '                .is_some_and(|_| true)\n',
        T_S_NAME, 'rust',
    ),
    # §2 the server: the deadline reaches the loop
    Control(
        'p25_loop_deadline', 'src/app/runtime.rs',
        '            self.fresh_deadline(),\n',
        '',
        T_S_LOOP, 'rust',
    ),
    # §2 (work order) a /clear in flight holds the compact off
    Control(
        'p25_compact_yields', 'src/app/nirvana_idle_compact.rs',
        '        if terminal.idle_stretch.fresh.busy() {\n',
        '        if false && terminal.idle_stretch.fresh.busy() {\n',
        T_S_RACE, 'rust',
    ),
    # §2 (work order) the held compact is stamped, so it waits RECHECK
    Control(
        'p25_compact_yields_stamped', 'src/app/nirvana_idle_compact.rs',
        '            terminal.idle_stretch.checked_at.replace(now);\n',
        '',
        T_S_RACE, 'rust',
    ),
    # §3 no tool Nirvana Claude needs is cut
    Control(
        'p25_cut_never_needed', L,
        'pub const DISALLOWED: [&str; 3] = ["Workflow", "ScheduleWakeup", "ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 3] = ["Workflow", "ScheduleWakeup", "Bash"];\n',
        T_NEEDED, 'rust',
    ),
    # §3 the needed list holds what §3.3 names
    Control(
        'p25_needed_list', L,
        '    "AskUserQuestion",\n    "Skill",\n];\n',
        '    "AskUser",\n    "Skill",\n];\n',
        T_NEEDED, 'rust',
    ),
    # §3 the cut list is the three the probe passed
    Control(
        'p25_cut_list', L,
        'pub const DISALLOWED: [&str; 3] = ["Workflow", "ScheduleWakeup", "ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 3] = ["Workflow", "ScheduleWakeup", "Artifact"];\n',
        T_CUTS, 'rust',
    ),
    # §3 one comma-joined value, the form the probe measured
    Control(
        'p25_cut_joined', L,
        '    Ok((!cuts.is_empty()).then(|| cuts.join(",")))\n',
        '    Ok((!cuts.is_empty()).then(|| cuts.join(" ")))\n',
        T_CUTS, 'rust',
    ),
    # §3 a cut list naming a needed tool is refused
    Control(
        'p25_cut_refuses_needed', L,
        '    if let Some(needed) = cuts.iter().find(|tool| NEEDED.contains(tool)) {\n',
        '    if let Some(needed) = cuts.iter().find(|_| false) {\n',
        T_DOUBT, 'rust',
    ),
    # §3 an argv that already sets the flag is left as it is
    Control(
        'p25_cut_refuses_a_second_flag', L,
        '    if args.iter().any(|arg| arg.starts_with(DISALLOWED_FLAG)) {\n',
        '    if args.iter().any(|arg| arg == "--never") {\n',
        T_DOUBT, 'rust',
    ),
    # §3 a doubt keeps today's argv
    Control(
        'p25_cut_doubt_keeps_argv', L,
        '            tracing::warn!("nirvana: Nirvana Claude starts with every tool: {why}");\n            args\n',
        '            tracing::warn!("nirvana: Nirvana Claude starts with every tool: {why}");\n            Vec::new()\n',
        T_DOUBT, 'rust',
    ),
    # §3 the cut is added to the argv
    Control(
        'p25_cut_added', L,
        '            args.extend([DISALLOWED_FLAG.to_owned(), value]);\n',
        '            let _ = value;\n',
        T_DOUBT, 'rust',
    ),
    # §3 launch_args carries the cut list
    Control(
        'p25_launch_cuts', 'src/nirvana_launch.rs',
        '    let mut args = crate::nirvana_lean::with_cuts(args, &crate::nirvana_lean::DISALLOWED);\n',
        '    let mut args = args;\n',
        T_LAUNCH, 'rust',
    ),
    # §5 the line is bladebro's run batches
    Control(
        'p25_browser_line_text', L,
        'pub const BROWSER_LINE: &str = "Use bladebro `run` to batch steps; read the result, not each page.";\n',
        'pub const BROWSER_LINE: &str = "Use bladebro to read each page.";\n',
        T_BROWSER, 'rust',
    ),
    # §5 bladebro by name is a browser
    Control(
        'p25_browser_names', L,
        '        text.contains("browser") || text.contains("bladebro")\n',
        '        text.contains("browser")\n',
        T_BROWSER, 'rust',
    ),
    # §5 a browser named in any case
    Control(
        'p25_browser_case', L,
        '        let text = text.to_ascii_lowercase();\n',
        '        let text = text.to_owned();\n',
        T_BROWSER, 'rust',
    ),
    # §5 the order's needs count, not only its class
    Control(
        'p25_browser_needs', L,
        '    (names_browser(class) || needs.iter().any(|need| names_browser(need))).then_some(BROWSER_LINE)\n',
        '    names_browser(class).then_some(BROWSER_LINE)\n',
        T_BROWSER, 'rust',
    ),
    # §5 a browser order's brief holds the line
    Control(
        'p25_browser_brief', L,
        '        brief.push_str(line);\n',
        '        let _ = line;\n',
        T_BRIEF, 'rust',
    ),
)
