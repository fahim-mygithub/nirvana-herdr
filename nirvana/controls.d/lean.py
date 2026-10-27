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
T_CUTS = 'the_cut_list_is_the_one_the_owner_kept'
# The review round (P25-fix): B1, S1 to S3 and the nits.
T_OPEN = 'the_trigger_holds_off_while_a_run_of_the_pane_is_open'
T_TYPED = 'the_trigger_holds_off_once_the_owner_typed_after_the_end'
T_RECENT = 'the_trigger_needs_a_recent_end'
T_SCENE1 = 'scenario_one_a_question_after_the_end_takes_the_record_back'
T_SCENE2 = 'scenario_two_a_run_opened_after_the_end_takes_the_record_back'
T_BOUND = 'the_note_goes_only_to_its_project_and_a_resume_forgets_a_stops_record'
T_OWNER_NOTE = 'the_owners_note_quotes_the_transcript_the_hook_reports'
T_REFUSED = 'a_named_pane_in_another_project_is_refused'
T_OWNER_RECORD = 'the_owners_record_names_no_transcript'
T_STALE = 'a_stale_stop_record_is_forgotten'
T_OWN_TRANSCRIPT = 'the_fresh_start_needs_the_records_own_transcript'
T_S_SENT_RACE = 'a_sent_clear_holds_idle_compact_off_and_a_sent_compact_holds_the_fresh_start_off'
T_S_STALE = 'fresh_server_forgets_a_stale_stop_record'
T_S_TRANSCRIPT = 'fresh_server_types_only_into_the_records_conversation'
# The second review round (P25-fix2): S-A to S-C and the nits.
T_ENDS_AGE = 'a_stop_record_ages_from_its_runs_end'
T_RECHECK = 'the_send_time_check_runs_every_b1_rule_again'
T_RECHECK_OWNER = 'an_owner_record_is_forgotten_once_the_owner_types_after_it'
T_RECHECK_IN = 'the_send_time_check_reads_the_projects_runs_and_the_panes_transcript'
T_OPEN_SAID = 'an_open_run_holding_the_fresh_start_off_is_said'
T_START_STALE = 'a_clear_prints_no_note_for_a_stale_stop_record'
T_IMAGE = 'an_image_is_the_owner_and_a_task_notification_is_not'
T_S_LATE = 'fresh_server_rechecks_b1_when_the_stop_hook_was_late'
T_S_RECHECK = 'fresh_server_rechecks_the_open_run_and_the_ends_age'
T_S_OWNER_TYPED = 'fresh_server_forgets_an_owner_record_the_owner_typed_after'
# The third review round (P25-fix3): R1, R2 and the nit.
T_UNREAD = 'an_unread_run_list_is_a_doubt_at_stop_and_at_send'
T_OWN_RUN = 'a_stop_record_needs_its_own_run_ended_in_the_replay'
T_BANG = 'the_owners_own_bang_fresh_keeps_their_record'
T_S_UNREAD = 'fresh_server_forgets_a_record_whose_runs_cannot_be_read'
T_S_BANG = 'fresh_server_keeps_the_owners_record_through_their_own_bang_command'
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
        '                == Some(pane)\n        })\n        .filter_map(',
        '                != None\n        })\n        .filter_map(',
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
        '            take_back(home, pane);\n            Ok(Err(why))\n',
        '            Ok(Err(why))\n',
        T_STOP, 'rust',
    ),
    # §2 only the Stop hook's own record is taken back, never the owner's (round 3: through `take_back`)
    Control(
        'p25_stop_spares_the_owner', F,
        '        .is_some_and(|record| record.by == "stop")\n    {\n        forget(home, pane);\n    }\n}\n',
        '        .is_some_and(|_| true)\n    {\n        forget(home, pane);\n    }\n}\n',
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
        '        "clear" | "compact" => {}\n',
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
        '            Ok(()) => Decision::Send(record.at),\n',
        '            Ok(()) => Decision::Nothing,\n',
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
        '    pub fn busy(&self) -> bool {\n        self.pending.is_some() || self.sent\n',
        '    pub fn busy(&self) -> bool {\n        self.sent\n',
        T_SCHEDULE, 'rust',
    ),
    # review S3: a /clear whose Enter went holds a compact off for the rest of the stretch
    Control(
        'p25_busy_once_sent', F,
        '    pub fn busy(&self) -> bool {\n        self.pending.is_some() || self.sent\n',
        '    pub fn busy(&self) -> bool {\n        self.pending.is_some()\n',
        T_S_SENT_RACE, 'rust',
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
        '        let (nirvana, transcript) = if waiting && !compact_in_flight && !compact_sent {\n',
        '        let (nirvana, transcript) = if true {\n',
        T_S_NONE, 'rust',
    ),
    # §2 the server: Nirvana Claude is told by its argv
    Control(
        'p25_server_nirvana_argv', S,
        '            (outside.argv.map(|argv| is_nirvana(&argv, home)), transcript)\n',
        '            (outside.argv.map(|_| true), transcript)\n',
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
        'pub const DISALLOWED: [&str; 1] = ["ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 1] = ["Bash"];\n',
        T_NEEDED, 'rust',
    ),
    # §3 the needed list holds what §3.3 names
    Control(
        'p25_needed_list', L,
        '    "AskUserQuestion",\n    "Skill",\n];\n',
        '    "AskUser",\n    "Skill",\n];\n',
        T_NEEDED, 'rust',
    ),
    # §3, the owner's ruling of 2026-10-26: "Keep only ReportFindings cut"
    Control(
        'p25_cut_list', L,
        'pub const DISALLOWED: [&str; 1] = ["ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 1] = ["Artifact"];\n',
        T_CUTS, 'rust',
    ),
    # §3, the owner's ruling: Workflow stays usable
    Control(
        'p25_cut_keeps_workflow', L,
        'pub const DISALLOWED: [&str; 1] = ["ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 1] = ["Workflow"];\n',
        T_CUTS, 'rust',
    ),
    # §3, the owner's ruling: ScheduleWakeup stays usable
    Control(
        'p25_cut_keeps_schedule_wakeup', L,
        'pub const DISALLOWED: [&str; 1] = ["ReportFindings"];\n',
        'pub const DISALLOWED: [&str; 1] = ["ScheduleWakeup"];\n',
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
        '        .any(|arg| arg.starts_with(DISALLOWED_FLAG) || arg.starts_with(DISALLOWED_ALIAS))\n',
        '        .any(|arg| arg.starts_with(DISALLOWED_ALIAS))\n',
        T_DOUBT, 'rust',
    ),
    # review nit: --disallowed-tools, the flag's other spelling, sets it too
    Control(
        'p25_cut_refuses_the_alias', L,
        '        .any(|arg| arg.starts_with(DISALLOWED_FLAG) || arg.starts_with(DISALLOWED_ALIAS))\n',
        '        .any(|arg| arg.starts_with(DISALLOWED_FLAG))\n',
        T_DOUBT, 'rust',
    ),
    # review nit: the alias is Claude Code's spelling
    Control(
        'p25_cut_alias_spelling', L,
        'pub const DISALLOWED_ALIAS: &str = "--disallowed-tools";\n',
        'pub const DISALLOWED_ALIAS: &str = "--disallowed_tools";\n',
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
        'pub const BROWSERS: [&str; 3] = ["browser", "agent-browser", "bladebro"];\n',
        'pub const BROWSERS: [&str; 2] = ["browser", "agent-browser"];\n',
        T_BROWSER, 'rust',
    ),
    # §5 a browser named in any case
    Control(
        'p25_browser_case', L,
        '            .map(str::to_ascii_lowercase)\n',
        '            .map(str::to_owned)\n',
        T_BROWSER, 'rust',
    ),
    # review nit: a name counts as a whole token, so no-browser is not a browser
    Control(
        'p25_browser_whole_token', L,
        '            .any(|token| BROWSERS.contains(&token.as_str()))\n',
        '            .any(|token| BROWSERS.iter().any(|name| token.contains(name)))\n',
        T_BROWSER, 'rust',
    ),
    # review nit: a need is split on commas and whitespace
    Control(
        'p25_browser_split', L,
        "        text.split(|c: char| c == ',' || c.is_whitespace())\n",
        "        text.split(|c: char| c == ';')\n",
        T_BROWSER, 'rust',
    ),
    # review nit: only on the claude seat (decision 0043 grants bladebro to headless Claude orders only)
    Control(
        'p25_browser_claude_seat', L,
        '    if cli != BROWSER_CLI {\n',
        '    if false && cli != BROWSER_CLI {\n',
        T_BROWSER, 'rust',
    ),
    # review nit: the claude seat's CLI is `claude`
    Control(
        'p25_browser_cli_name', L,
        'pub const BROWSER_CLI: &str = "claude";\n',
        'pub const BROWSER_CLI: &str = "agy";\n',
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
    # review nit: the brief asks with the seat's CLI
    Control(
        'p25_browser_brief_cli', L,
        '    if let Some(line) = browser_line(cli, class, needs) {\n',
        '    if let Some(line) = browser_line(BROWSER_CLI, class, needs) {\n',
        T_BRIEF, 'rust',
    ),
    # review B1, rule 1: no open run whose orchestrator_pane is this pane
    Control(
        'p25_trigger_open_run', F,
        '    };\n    if let Some(open) = open_here {\n        return Err(',
        '    };\n    if let Some(open) = None::<&str> {\n        return Err(',
        T_OPEN, 'rust',
    ),
    # review B1, rule 1: an open run is one not ended
    Control(
        'p25_open_of_open_only', F,
        '        .filter(|(_, state)| state.ended.is_none())\n        .find(',
        '        .find(',
        T_OPEN, 'rust',
    ),
    # review B1, rule 1: an open run of this pane, not another's
    Control(
        'p25_open_of_this_pane', F,
        '                == Some(pane)\n        })\n        .map(|(run, _)| run.as_str())\n',
        '                != None\n        })\n        .map(|(run, _)| run.as_str())\n',
        T_OPEN, 'rust',
    ),
    # review B1, rule 1, end to end: the Stop hook reads this pane's open run
    Control(
        'p25_stop_reads_the_open_run', F,
        '    let open = open_of(pane, &runs);\n',
        '    let open: Option<&str> = None;\n',
        T_SCENE2, 'rust',
    ),
    # review B1, rule 3: a recent end
    Control(
        'p25_trigger_recent', F,
        '    if now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {\n        return Err(',
        '    if false && now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {\n        return Err(',
        T_RECENT, 'rust',
    ),
    # review B1, rule 3: at most FRESH_WITHIN, inclusive
    Control(
        'p25_trigger_recent_inclusive', F,
        '    if now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {\n        return Err(',
        '    if now.saturating_sub(ended_at) >= FRESH_WITHIN.as_secs() {\n        return Err(',
        T_RECENT, 'rust',
    ),
    # review B1, rule 3: FRESH_WITHIN is 10 minutes
    Control(
        'p25_fresh_within_value', F,
        'pub const FRESH_WITHIN: Duration = Duration::from_secs(10 * 60);\n',
        'pub const FRESH_WITHIN: Duration = Duration::from_secs(11 * 60);\n',
        T_RECENT, 'rust',
    ),
    # review B1, rule 2: the owner's check is the trigger's
    Control(
        'p25_trigger_owner_check', F,
        '    owner_since(ended_at)?;\n',
        '',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2, end to end: the Stop hook reads the transcript for it
    Control(
        'p25_stop_reads_the_owner', F,
        '        |ended_at| owner_since(transcript, ended_at),\n',
        '        |_| Ok(()),\n',
        T_SCENE1, 'rust',
    ),
    # review B1, rule 2: an owner prompt after the end
    Control(
        'p25_owner_after_end', F,
        '                Some(at) if at < ended_at => {}\n',
        '                Some(_) if true => {}\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: a prompt in the end's own second is a doubt
    Control(
        'p25_owner_same_second', F,
        '                Some(at) if at < ended_at => {}\n',
        '                Some(at) if at <= ended_at => {}\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: an owner prompt whose time cannot be read is one after the end
    Control(
        'p25_owner_untimed', F,
        '                None => return Err("an owner prompt\'s time cannot be read".into()),\n',
        '                None => {}\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: a line that does not parse may be the owner's
    Control(
        'p25_owner_unparsed', F,
        '            return Err("a transcript line does not parse, so it may be the owner\'s".into());\n',
        '            continue;\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: a tail that never reaches back to the end is a doubt
    Control(
        'p25_owner_reaches_the_end', F,
        '    if !reaches_the_end {\n',
        '    if false && !reaches_the_end {\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: the end is reached by a line timed before it
    Control(
        'p25_owner_reached_before', F,
        '        reaches_the_end |= at.is_some_and(|at| at < ended_at);\n',
        '        reaches_the_end |= at.is_some();\n',
        T_TYPED, 'rust',
    ),
    # review B1, rule 2: owner_prompts' own filter (wakes, tool results, meta lines are not the owner's)
    Control(
        'p25_owner_filter', F,
        '        if owners(&value) {\n',
        '        if true {\n',
        T_TYPED, 'rust',
    ),
    # review nit: the Stop hook never overwrites the owner's record
    Control(
        'p25_stop_never_overwrites_the_owner', F,
        '            if existing.as_ref().is_some_and(|record| record.by == "owner") {\n',
        '            if false && existing.as_ref().is_some_and(|record| record.by == "owner") {\n',
        T_STOP, 'rust',
    ),
    # review S1 (R2: through `stale`): a stale record is forgotten
    Control(
        'p25_decide_stale', F,
        '    if let Some(why) = stale(record, facts.now) {\n',
        '    if let Some(why) = None::<String> {\n',
        T_STALE, 'rust',
    ),
    # review S1 (R2: the owner's has an age of its own): a Stop's record ages FRESH_WITHIN, the owner's does not
    Control(
        'p25_decide_stale_only_stop', F,
        '    if record.by == "owner" {\n        return (now',
        '    if false {\n        return (now',
        T_STALE, 'rust',
    ),
    # review S1: a stale record is forgotten before the mark is read, so a mark never keeps it
    Control(
        'p25_decide_stale_before_the_mark', F,
        '    if let Some(why) = stale(record, facts.now) {\n        return Decision::Forget(why);\n    }\n'
        '    if facts.sent_for == Some(record.at) {\n        return Decision::Nothing;\n    }\n',
        '    if facts.sent_for == Some(record.at) {\n        return Decision::Nothing;\n    }\n'
        '    if let Some(why) = stale(record, facts.now) {\n        return Decision::Forget(why);\n    }\n',
        T_STALE, 'rust',
    ),
    # review S1 (R2: in `stale`): a Stop's record ages from its own second
    Control(
        'p25_stale_record_second', F,
        '    if now.saturating_sub(record.at) > FRESH_WITHIN.as_secs() {\n',
        '    if false && now.saturating_sub(record.at) > FRESH_WITHIN.as_secs() {\n',
        T_STALE, 'rust',
    ),
    # review S1: the pane's transcript must be the record's
    Control(
        'p25_decide_transcript', F,
        '            (Some(want), Some(have)) if same_transcript(Path::new(want), have) => {}\n',
        '            (Some(_), Some(_)) if true => {}\n',
        T_OWN_TRANSCRIPT, 'rust',
    ),
    # review S1: a transcript that could not be told is a doubt
    Control(
        'p25_decide_transcript_unknown', F,
        '            (_, None) => return skip("the pane\'s transcript could not be told".into()),\n',
        '            (_, None) => {}\n',
        T_OWN_TRANSCRIPT, 'rust',
    ),
    # review S1: a Stop's record that names no transcript is a doubt
    Control(
        'p25_decide_transcript_unnamed', F,
        '            (None, _) => return skip("the record names no transcript".into()),\n',
        '            (None, _) => {}\n',
        T_OWN_TRANSCRIPT, 'rust',
    ),
    # review S2: the owner's record is not compared
    Control(
        'p25_decide_transcript_not_the_owners', F,
        '    if record.by != "owner" {\n',
        '    if true {\n',
        T_OWN_TRANSCRIPT, 'rust',
    ),
    # review S1: one file spelt two ways is one transcript
    Control(
        'p25_same_transcript', F,
        '    crate::nirvana_spaces::same_folder(left, right)\n',
        '    left == right\n',
        T_OWN_TRANSCRIPT, 'rust',
    ),
    # review S1: the note goes only to the record's own project
    Control(
        'p25_start_own_project', F,
        '        crate::nirvana_spaces::same_folder(project, Path::new(&record.project))\n',
        '        true\n',
        T_BOUND, 'rust',
    ),
    # review S1: a record of another project is forgotten
    Control(
        'p25_start_other_project_forgets', F,
        '    }) {\n        forget(home, pane);\n        return None;\n    }\n',
        '    }) {\n        return None;\n    }\n',
        T_BOUND, 'rust',
    ),
    # review S1: a resume forgets a Stop's record
    Control(
        'p25_start_resume_forgets', F,
        '            {\n                forget(home, pane);\n            }\n            return None;\n',
        '            {\n            }\n            return None;\n',
        T_BOUND, 'rust',
    ),
    # review S1: a resume keeps the owner's record
    Control(
        'p25_start_resume_keeps_the_owner', F,
        '                .is_some_and(|record| record.by == "stop")\n            {\n                forget(home, pane);\n            }\n            return None;\n',
        '                .is_some_and(|_| true)\n            {\n                forget(home, pane);\n            }\n            return None;\n',
        T_BOUND, 'rust',
    ),
    # review S2: the owner's note quotes the transcript the hook reports
    Control(
        'p25_start_owner_transcript', F,
        '        hook_transcript.map(Path::to_path_buf)\n',
        '        record.transcript.as_deref().map(PathBuf::from)\n',
        T_OWNER_NOTE, 'rust',
    ),
    # review S2: a Stop's note quotes its own record's transcript
    Control(
        'p25_start_stop_transcript', F,
        '        record.transcript.as_deref().map(PathBuf::from)\n',
        '        hook_transcript.map(Path::to_path_buf)\n',
        T_OWNER_NOTE, 'rust',
    ),
    # review S2: the note's prompts come from the transcript it is given
    Control(
        'p25_note_input_transcript', F,
        '    let prompts = transcript\n',
        '    let prompts = None::<&Path>\n',
        T_START, 'rust',
    ),
    # review S2: the owner's record names no transcript
    Control(
        'p25_owner_record_no_transcript', F,
        '        context: None,\n        transcript: None,\n        project: project.root.display().to_string(),\n        at: now,\n',
        '        context: None,\n        transcript: Some(String::new()),\n        project: project.root.display().to_string(),\n        at: now,\n',
        T_OWNER_RECORD, 'rust',
    ),
    # review S2: and guesses no context
    Control(
        'p25_owner_record_no_context', F,
        '        context: None,\n        transcript: None,\n        project: project.root.display().to_string(),\n        at: now,\n',
        '        context: Some(0),\n        transcript: None,\n        project: project.root.display().to_string(),\n        at: now,\n',
        T_OWNER_RECORD, 'rust',
    ),
    # review S2: a named pane in another project is refused
    Control(
        'p25_refuses_another_project', F,
        '        Some(target) if crate::nirvana_spaces::same_folder(target, caller) => Ok(()),\n',
        '        Some(_) if true => Ok(()),\n',
        T_REFUSED, 'rust',
    ),
    # review S2: a pane whose folder could not be read is refused
    Control(
        'p25_refuses_an_unread_pane', F,
        '        None => Err("its folder could not be read".into()),\n',
        '        None => Ok(()),\n',
        T_REFUSED, 'rust',
    ),
    # review S3: a stretch whose idle compact went gets no fresh start
    Control(
        'p25_decide_compact_sent', F,
        '    if facts.compact_sent {\n',
        '    if false && facts.compact_sent {\n',
        T_COMPACT, 'rust',
    ),
    # review S3: the server reads the compact's mark
    Control(
        'p25_server_compact_sent_fact', S,
        '        let compact_sent = terminal.idle_stretch.sent;\n',
        '        let compact_sent = false;\n',
        T_S_SENT_RACE, 'rust',
    ),
    # review S1: the server forgets a stale record
    Control(
        'p25_server_forgets', S,
        '                crate::nirvana_fresh::forget(home, &public);\n',
        '',
        T_S_STALE, 'rust',
    ),
    # review S1: the server knows the time, for the record's age
    Control(
        'p25_server_now_fact', S,
        '            now: crate::nirvana_run::now(),\n',
        '            now: 0,\n',
        T_S_STALE, 'rust',
    ),
    # review S1: the server reads the pane's transcript
    Control(
        'p25_server_transcript_fact', S,
        '            (outside.argv.map(|argv| is_nirvana(&argv, home)), transcript)\n',
        '            (outside.argv.map(|argv| is_nirvana(&argv, home)), None)\n',
        T_S_TRANSCRIPT, 'rust',
    ),
    # review round 2, S-B: a Stop's record ages from its run's end
    Control(
        'p25_stale_from_the_end', F,
        '    if now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {\n        return Some(',
        '    if false && now.saturating_sub(ended_at) > FRESH_WITHIN.as_secs() {\n        return Some(',
        T_ENDS_AGE, 'rust',
    ),
    # review round 2, S-B: a Stop's record naming no end is a doubt
    Control(
        'p25_stale_no_end', F,
        '        return Some("the Stop\'s record names no end".into());\n',
        '        return None;\n',
        T_ENDS_AGE, 'rust',
    ),
    # review round 2, S-C: the owner's record lives an hour
    Control(
        'p25_owner_within_value', F,
        'pub const OWNER_WITHIN: Duration = Duration::from_secs(60 * 60);\n',
        'pub const OWNER_WITHIN: Duration = Duration::from_secs(60 * 61);\n',
        T_STALE, 'rust',
    ),
    # review round 2, S-C: an hour old exactly still waits
    Control(
        'p25_owner_within_inclusive', F,
        '        return (now.saturating_sub(record.at) > OWNER_WITHIN.as_secs()).then(|| {\n',
        '        return (now.saturating_sub(record.at) >= OWNER_WITHIN.as_secs()).then(|| {\n',
        T_STALE, 'rust',
    ),
    # review round 2, S-A: the send-time check ages the record again
    Control(
        'p25_recheck_stale', F,
        '    if let Some(why) = stale(record, now) {\n        return Err(why);\n',
        '    if let Some(why) = None::<String> {\n        return Err(why);\n',
        T_ENDS_AGE, 'rust',
    ),
    # review round 2, S-A: the send-time check refuses an open run of the pane
    Control(
        'p25_recheck_open_run', F,
        '        if let Some(open) = open_here {\n            return Err(',
        '        if let Some(open) = None::<&str> {\n            return Err(',
        T_RECHECK, 'rust',
    ),
    # review round 2, S-A: the send-time check reads the owner's prompts after the end
    Control(
        'p25_recheck_owner_since', F,
        '        owner_prompt_since(tail, since)\n    }\n',
        '        Ok(())\n    }\n',
        T_RECHECK, 'rust',
    ),
    # review round 2, S-A: a Stop's record is read from its run's end
    Control(
        'p25_recheck_since_the_end', F,
        '            .ok_or_else(|| "the Stop\'s record names no end".to_owned())?\n',
        '            .map_or(0, |end| end + 100)\n',
        T_RECHECK, 'rust',
    ),
    # review round 2, S-C: the owner's record is read from its own second, and an open run does not hold it
    Control(
        'p25_recheck_owner_branch', F,
        '    let since = if record.by == "owner" {\n',
        '    let since = if false {\n',
        T_RECHECK_OWNER, 'rust',
    ),
    # review round 2, S-C: from its own second, not later
    Control(
        'p25_recheck_owner_at', F,
        '        record.at\n    } else {\n',
        '        record.at + 100\n    } else {\n',
        T_RECHECK_OWNER, 'rust',
    ),
    # review round 2, S-A over the files: the record's project's runs
    Control(
        'p25_recheck_in_runs', F,
        '        let runs = read_run_states(&Project::at(&record.project))?;\n',
        '        let runs = read_run_states(&Project::at(&record.project))?\n            .into_iter()\n            .filter(|(_, state)| state.ended.is_some())\n            .collect::<Vec<_>>();\n',
        T_RECHECK_IN, 'rust',
    ),
    # review round 2, S-A over the files: this pane's open run
    Control(
        'p25_recheck_in_open_of', F,
        '        open_of(pane, &runs),\n        tail',
        '        None,\n        tail',
        T_RECHECK_IN, 'rust',
    ),
    # review round 2, S-A over the files: the transcript's tail
    Control(
        'p25_recheck_in_tail', F,
        '            crate::nirvana_subagents::read_file_tail(path, OWNER_TAIL)\n                .map_err(|err| format!("the transcript could not be read: {err}"))\n        });\n',
        '            crate::nirvana_subagents::read_file_tail(path, 0)\n                .map_err(|err| format!("the transcript could not be read: {err}"))\n        });\n',
        T_RECHECK_IN, 'rust',
    ),
    # review round 2, S-A: decide asks the check before a send
    Control(
        'p25_decide_asks_the_check', F,
        '        InputLine::Empty => match recheck(record) {\n',
        '        InputLine::Empty => match Ok::<(), String>(()) {\n',
        T_RECHECK, 'rust',
    ),
    # review round 2, S-A: the check's failure forgets the record
    Control(
        'p25_decide_check_forgets', F,
        '            Err(why) => Decision::Forget(why),\n',
        '            Err(why) => Decision::Skip(why),\n',
        T_RECHECK, 'rust',
    ),
    # review round 2, S-A: the server passes the check the files
    Control(
        'p25_server_rechecks', S,
        '        let recheck = |record: &crate::nirvana_fresh::Record| {\n',
        '        let recheck = |record: &crate::nirvana_fresh::Record| -> Result<(), String> {\n            return Ok(());\n',
        T_S_LATE, 'rust',
    ),
    # review round 2, S-A: the server's check reads the pane's own transcript
    Control(
        'p25_server_recheck_transcript', S,
        '                facts.transcript.as_deref(),\n',
        '                None,\n',
        T_S_SEND, 'rust',
    ),
    # review round 2, S-A: the server's check reads this pane's runs
    Control(
        'p25_server_recheck_pane', S,
        '                &public,\n                facts.transcript',
        '                "another",\n                facts.transcript',
        T_S_RECHECK, 'rust',
    ),
    # review round 2 nit: an open run holding the fresh start off is said, naming it
    Control(
        'p25_open_blocks_needs_an_end', F,
        '    let (ended, _) = last_end?;\n',
        '    let (ended, _) = last_end.unwrap_or(("none", 0));\n',
        T_OPEN_SAID, 'rust',
    ),
    # review round 2 nit: only when a run is open
    Control(
        'p25_open_blocks_needs_the_run', F,
        '        open_here?\n',
        '        open_here.unwrap_or("none")\n',
        T_OPEN_SAID, 'rust',
    ),
    # review round 2 nit: a stale Stop's record prints no note at a clear
    Control(
        'p25_start_stale', F,
        '        && stale(&record, now).is_some()\n',
        '        && false\n',
        T_START_STALE, 'rust',
    ),
    # review round 2 nit: unless the server cleared for it
    Control(
        'p25_start_stale_unless_sent', F,
        '        && !sent_lately(home, pane, record.at, now)\n',
        '',
        T_START_STALE, 'rust',
    ),
    # review round 2 nit: the owner's record is not aged at a clear
    Control(
        'p25_start_stale_only_stop', F,
        '    if record.by != "owner"\n        && !sent_lately',
        '    if true\n        && !sent_lately',
        T_START_STALE, 'rust',
    ),
    # review round 2 nit: an image the owner pasted is the owner after the end
    Control(
        'p25_image_is_the_owner', F,
        '    let kind = |block: &Value| block.get("type").and_then(Value::as_str) == Some("image");\n',
        '    let kind = |block: &Value| block.get("type").and_then(Value::as_str) == Some("picture");\n',
        T_IMAGE, 'rust',
    ),
    # review round 2 nit: read as a word, so owner_prompt_in sees a prompt
    Control(
        'p25_image_as_a_word', F,
        '            *block = serde_json::json!({"type": "text", "text": "[image]"});\n',
        '            *block = serde_json::json!({"type": "text", "text": ""});\n',
        T_IMAGE, 'rust',
    ),
    # review round 2 nit: owner_prompt_in's header, tool results and leads hold for the image line
    Control(
        'p25_image_through_the_filter', F,
        '    owner_prompt_in(&shown).is_some()\n',
        '    true\n',
        T_IMAGE, 'rust',
    ),
    # review round 2 nit: a line with words of its own is owner_prompt_in's alone
    Control(
        'p25_image_worded', F,
        '    if worded || !blocks.iter().any(kind) {\n',
        '    if !blocks.iter().any(kind) {\n',
        T_IMAGE, 'rust',
    ),
    # review round 2 nit: blank words are no words
    Control(
        'p25_image_blank_words', F,
        '            .is_some_and(|text| !text.trim().is_empty())\n',
        '            .is_some_and(|text| !text.is_empty())\n',
        T_IMAGE, 'rust',
    ),
    # review round 2 nit: a worded prompt is still the owner's first
    Control(
        'p25_typed_prompt_first', F,
        '    if owner_prompt_in(value).is_some() {\n        return true;\n    }\n',
        '',
        T_TYPED, 'rust',
    ),
    # review round 2 nit: a task notification is not the owner
    Control(
        'p25_not_owner_task_notification', F,
        '        "<task-notification>",\n',
        '',
        T_IMAGE, 'rust',
    ),
    # review round 2, S-C, end to end: the server forgets an owner's record the owner typed after
    Control(
        'p25_server_owner_typed', F,
        '    let runs = if record.by == "owner" {\n        Vec::new()\n',
        '    let runs = if record.by == "owner" {\n        return Ok(());\n',
        T_S_OWNER_TYPED, 'rust',
    ),
    # review round 3, R1: the Stop hook reads the runs strictly, not as `run_states` reads them
    Control(
        'p25_r1_stop_reads_strictly', F,
        '    let runs = match read_run_states(project) {\n',
        '    let runs = match Ok::<_, String>(run_states(project)) {\n',
        T_UNREAD, 'rust',
    ),
    # review round 3, R1: a doubt at Stop time takes back the Stop's earlier record
    Control(
        'p25_r1_stop_takes_back', F,
        '            take_back(home, pane);\n            return Ok(Err(why));\n',
        '            return Ok(Err(why));\n',
        T_UNREAD, 'rust',
    ),
    # review round 3, R1: a runs folder that cannot be listed is a doubt
    Control(
        'p25_r1_list_error', F,
        '        Err(err) => return Err(format!("{} could not be listed: {err}", dir.display())),\n',
        '        Err(_) => return Ok(Vec::new()),\n',
        T_UNREAD, 'rust',
    ),
    # review round 3, R1: a journal line that does not parse is a doubt
    Control(
        'p25_r1_bad_lines', F,
        '        if bad > 0 {\n',
        '        if false && bad > 0 {\n',
        T_UNREAD, 'rust',
    ),
    # review round 3, R1: the send-time check reads the runs strictly
    Control(
        'p25_r1_recheck_reads_strictly', F,
        '        let runs = read_run_states(&Project::at(&record.project))?;\n',
        '        let runs = run_states(&Project::at(&record.project));\n',
        T_UNREAD, 'rust',
    ),
    # review round 3, R1: the send-time check needs the record's own run, ended
    Control(
        'p25_r1_recheck_own_run', F,
        '        own_run_ended(record, &runs)?;\n',
        '',
        T_OWN_RUN, 'rust',
    ),
    # review round 3, R1: a run missing from the replay is no run that ended
    Control(
        'p25_r1_own_run_missing', F,
        '        None => Err(format!("run {run} is not among the project\'s runs")),\n',
        '        None => Ok(()),\n',
        T_OWN_RUN, 'rust',
    ),
    # review round 3, R1: nor is a run still open
    Control(
        'p25_r1_own_run_open', F,
        '        Some((_, state)) if state.ended.is_none() => Err(format!("run {run} has not ended")),\n',
        '        Some((_, state)) if false && state.ended.is_none() => Err(format!("run {run} has not ended")),\n',
        T_OWN_RUN, 'rust',
    ),
    # review round 3, R1: a Stop's record naming no run is a doubt
    Control(
        'p25_r1_own_run_named', F,
        '        return Err("the Stop\'s record names no run".into());\n',
        '        return Ok(());\n',
        T_OWN_RUN, 'rust',
    ),
    # review round 3, R1, end to end: the server forgets a record whose run is not in the replay
    Control(
        'p25_r1_server_own_run', F,
        '        own_run_ended(record, &runs)?;\n',
        '',
        T_S_UNREAD, 'rust',
    ),
    # review round 3, R2: a `!` command's lines are not the owner typing after their own record
    Control(
        'p25_r2_bang_skipped', F,
        '    typed_since(text, at, |value| owner_typed_in(value) && !bang_line(value))\n',
        '    typed_since(text, at, owner_typed_in)\n',
        T_BANG, 'rust',
    ),
    # review round 3, R2: a line is a `!` command's by its lead
    Control(
        'p25_r2_bang_lead', F,
        '    owner_prompt_in(value).is_some_and(|text| BANG_LEADS.iter().any(|lead| text.starts_with(lead)))\n',
        '    owner_prompt_in(value).is_some_and(|text| BANG_LEADS.iter().any(|lead| text.ends_with(lead)))\n',
        T_BANG, 'rust',
    ),
    # review round 3, R2: the output's stderr lead too
    Control(
        'p25_r2_bang_stderr', F,
        'pub const BANG_LEADS: [&str; 3] = ["<bash-input>", "<bash-stdout>", "<bash-stderr>"];\n',
        'pub const BANG_LEADS: [&str; 3] = ["<bash-input>", "<bash-stdout>", "<bash-stdout>"];\n',
        T_BANG, 'rust',
    ),
    # review round 3, R2: after a run's end a `!` command is still a doubt for a Stop's record
    Control(
        'p25_r2_stop_counts_bang', F,
        '    typed_since(text, ended_at, owner_typed_in)\n',
        '    typed_since(text, ended_at, |value| owner_typed_in(value) && !bang_line(value))\n',
        T_BANG, 'rust',
    ),
    # review round 3, R2: the send-time check reads the owner's record without the `!` lines
    Control(
        'p25_r2_recheck_owner', F,
        '        owner_typed_since(tail, since)\n',
        '        owner_prompt_since(tail, since)\n',
        T_BANG, 'rust',
    ),
    # review round 3, R2, end to end: the server clears for the owner's `!herdr nirvana fresh`
    Control(
        'p25_r2_server_bang', F,
        '        owner_typed_since(tail, since)\n',
        '        owner_prompt_since(tail, since)\n',
        T_S_BANG, 'rust',
    ),
    # review round 3 nit: the server's mark excepts a stale record only while it is young
    Control(
        'p25_sent_lately_age', F,
        '        && written.is_ok_and(|written| now.saturating_sub(written) <= FRESH_WITHIN.as_secs())\n',
        '        && written.is_ok()\n',
        T_START_STALE, 'rust',
    ),
    # review round 3 nit: and only the mark of this record
    Control(
        'p25_sent_lately_names_the_record', F,
        '    read_sent(home, pane) == Some(at)\n',
        '    read_sent(home, pane).is_some()\n',
        T_START_STALE, 'rust',
    ),
)
