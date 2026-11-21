# Plan 47 P47c (the owner, 2026-11-20: "breaks down our prompt to different research styles and runs parallel research
# agents and finally builds an artifact"; rulings R2 herdr runs the researchers, R3 the seats, R4 docs/research/).
# `herdr scholar "<question>"`: a driver seats a route turn, researchers side by side, a writer, checks the files, logs
# the run and ends it, and `replay` keeps a researcher's delivery off the orchestrator's queue.
#
# Controls start with p47c_ (free at ccb8bef0). One sabotage per new assertion, each on a product line (the shipped
# texts under nirvana/scholar/ and the role text are product). The comment above each says which assertion of the test
# in the fifth field goes red and why, written before the run.
# `Control` is provided by controls.py.
S = "src/nirvana_scholar.rs"
RUN = "src/nirvana_run.rs"
WORKER = "src/nirvana_worker.rs"
SUB = "src/nirvana_subagents.rs"
ROUTE = "nirvana/scholar/route.md"
WRITE = "nirvana/scholar/write.md"
ROLE = "nirvana/claude/ROLE.md"
SKILL = "nirvana/claude/skills/seat/SKILL.md"

_T = "nirvana_scholar::tests::"
T_CARDS = _T + "p47c_every_style_has_a_card_in_the_built_in"
T_EXAMPLES = _T + "p47c_the_six_route_examples_parse_and_validate"
T_COUNTS = _T + "p47c_one_two_and_five_orders_validate_and_seven_are_cut_to_five"
T_REFUSED = _T + "p47c_an_unknown_style_a_repeated_style_and_an_empty_question_are_refused"
T_FALLBACK = _T + "p47c_a_route_that_fails_twice_runs_one_fallback_order_with_the_owners_words"
T_PROMPT = _T + "p47c_the_route_prompt_lists_the_projects_reports_and_the_answer_is_read"
T_ALLPASS = _T + "p47c_all_pass_seats_the_writer"
T_ONEASKS = _T + "p47c_one_passes_and_one_asks_the_writer_gets_the_one_and_hears_of_the_other"
T_SENDBACK = _T + "p47c_a_chain_sent_back_twice_then_passing_waits_on_its_leaf"
T_NONE = _T + "p47c_no_researcher_passing_ends_no_delivery_and_seats_no_writer"
T_INTERRUPTED = _T + "p47c_an_interrupted_chain_is_reseated_once_and_twice_is_not_delivered"
T_CAP = _T + "p47c_the_cap_stops_what_still_runs_and_counts_it_not_delivered"
T_WRITER = _T + "p47c_the_writer_passing_is_checked_and_failing_ends_no_report"
T_STOP = _T + "p47c_a_stop_in_each_stage_stops_what_runs_then_ends_stopped"
T_RESUME = _T + "p47c_a_resume_in_each_stage_goes_on_from_the_state_file"
T_ENDWAIT = _T + "p47c_the_run_ends_only_after_its_wake_is_typed_within_a_bound"
T_REPLAY = _T + "p47c_replay_quiets_what_the_driver_handles_and_every_line_of_an_ordinary_order_queues"
T_ROWS = _T + "p47c_a_quiet_researcher_keeps_its_row_on_the_agents_panel"
T_WAKEPATH = _T + "p47c_the_writers_summary_path_fits_the_wake_with_a_40_character_slug"
T_REFUSALS = _T + "p47c_each_refusal_of_the_verb_says_one_line_and_writes_nothing"
T_OPENED = _T + "p47c_the_opened_line_names_the_verbs_pane_and_the_driver_inherits_its_env"
T_SLUG = _T + "p47c_slugs_and_ids_stay_plain_and_short"
T_STOPORDER = _T + "p47c_stop_order_stops_an_order_with_no_supervisor_as_seat_stop_does"
T_CAPDRIVE = _T + "p47c_a_driver_at_the_cap_stops_the_running_chain_and_tells_the_orchestrator"
T_HTML = _T + "p47c_the_html_check_fails_every_outside_load_and_passes_a_link"
T_TEXTS = _T + "p47c_the_order_texts_name_no_tool_and_quote_the_question"
T_VERSION = _T + "p47c_an_unknown_state_version_is_refused_and_a_second_driver_is_named"
T_LOG = _T + "p47c_the_log_line_the_tag_and_the_list"
T_E2E = _T + "p47c_a_whole_scripted_run_seats_researchers_then_the_writer_logs_and_ends"
T_NOTHING = _T + "p47c_nothing_to_research_ends_told_logged_and_with_no_order"
T_ROLE = _T + "p47c_the_role_and_the_seat_skill_send_research_to_herdr_scholar"
# The fix round's tests (the P47c review of 2026-11-20, F1 to F12; the fix brief's items 1 to 9).
T_TWO = _T + "p47c_two_runs_of_one_question_on_one_day_get_two_folders_and_two_ids"
T_LOGNL = _T + "p47c_the_log_keeps_each_line_whole_and_writes_a_run_once"
T_LOGTWICE = _T + "p47c_a_resume_after_the_log_append_writes_no_second_line"
T_VERBS = _T + "p47c_the_verbs_refuse_in_a_worker_and_an_unknown_run_writes_nothing"
T_PANIC = _T + "p47c_a_driver_that_panics_leaves_the_gave_up_line_and_status_names_resume"
T_ENDREFUSED = _T + "p47c_a_run_end_that_keeps_refusing_is_tried_for_thirty_minutes_then_named"
T_STATUS = _T + "p47c_status_names_a_report_only_when_there_is_one"
T_RACE = _T + "p47c_a_starting_driver_wins_against_a_status_that_reads_the_lock"
T_QCAP = _T + "p47c_a_question_over_8000_characters_is_refused_before_anything_is_written"
T_HTML2 = _T + "p47c_the_html_check_reads_tags_styles_and_scripts_not_prose"
T_LOST = _T + "p47c_the_end_line_and_the_method_notes_say_what_did_not_deliver"
T_MATRIX = _T + "p47c_the_kill_matrix_resumes_clean_at_every_stop_point"
# The second review's tests (review 2 F1 to F7).
T_CHECK_DEAD = _T + "p47c_fix2_check_names_resume_for_a_dead_driver_and_nothing_for_a_live_one"
T_CHECK_PLAIN = _T + "p47c_fix2_check_reads_an_ordinary_run_as_before_beside_a_scholar_run"
T_CHECK_LOCK = _T + "p47c_fix2_check_reads_the_driver_without_taking_its_lock"
T_ENDCUT = _T + "p47c_fix2_the_end_line_says_what_was_lost_within_the_cut"
T_LOGENC = _T + "p47c_fix2_a_log_with_a_bom_or_in_utf16_is_read_and_said"
T_HTML3 = _T + "p47c_fix2_the_html_check_strips_css_comments_reads_srcdoc_and_module_imports"

QUIET_KINDS = "                Kind::Interrupted { .. } | Kind::Delivered { .. } | Kind::Asked { .. } => true,\n"
TELL_THEN_WAKE = ("            state.told = true;\n            save_state(project, state)?;\n"
                  "            (world.wake)(project, &state.run);\n")
NO_DELIVERY = '    Step::Finish(ends::NO_DELIVERY, format!("no researcher passed: {why}"))'
LINK_FILTER = "            if let Some(href) = get(\"href\").filter(|href| outside(href) && !followed) {"

CONTROLS = (
    # ---- the menu and the shipped texts ----
    # A style renamed in the menu: T_CARDS's "no card trajectories in [...]".
    Control("p47c_style_renamed", S, '    "trajectory",\n];', '    "trajectories",\n];', T_CARDS, "rust"),
    # Example 6 names a style the menu lacks: T_EXAMPLES's validate panics "order 4 names the unknown style".
    Control("p47c_example_style_unknown", ROUTE, '{"style":"competitive-analysis","question":"How do other studios',
            '{"style":"competitive-analyses","question":"How do other studios', T_EXAMPLES, "rust"),
    # The draft's first paragraph back (the model lists the folder itself): T_EXAMPLES's "you have no tools" assert.
    Control("p47c_route_text_draft_reads", ROUTE,
            "You do no research and you have no tools: you open nothing, on the web or on disk. The reports this "
            "project already holds are listed after the question, each by its path and its title line.",
            "You do no research and open no web source. You may list the project's docs/research/ folder and read a "
            "report's heading line.", T_EXAMPLES, "rust"),
    # The draft's rule 10 back: T_EXAMPLES's "10. If a report listed after the question" assert.
    Control("p47c_route_text_draft_rule_10", ROUTE, "10. If a report listed after the question already answers it",
            "10. If docs/research/ holds a report that already answers it", T_EXAMPLES, "rust"),
    # The draft's summary line back: T_EXAMPLES's "The summary starts with the page's path" assert.
    Control("p47c_write_text_draft_summary", WRITE,
            "The summary starts with the page's path relative to the project root, exactly as your order gives it, "
            "then the answer line",
            "The summary gives the answer line", T_EXAMPLES, "rust"),
    # ---- route: validation ----
    # Seven kept: T_COUNTS's `got.orders.len()` is 7, not 5.
    Control("p47c_validate_keeps_seven", S, "    for (at, order) in orders.iter().take(MAX_ORDERS).enumerate() {",
            "    for (at, order) in orders.iter().take(MAX_ORDERS + 2).enumerate() {", T_COUNTS, "rust"),
    # The drop unsaid: T_COUNTS's note is "", not "2 order(s) past five were dropped: ...".
    Control("p47c_validate_note_silent", S,
            '        note = if note.is_empty() {\n            line\n        } else {\n            format!("{note}; {line}")\n        };\n',
            "        let _ = line;\n", T_COUNTS, "rust"),
    # An unknown style taken: T_REFUSED's `validate(&unknown).unwrap_err()` panics on Ok.
    Control("p47c_validate_unknown_kept", S, "        if !STYLES.contains(&style) {",
            "        if false && !STYLES.contains(&style) {", T_REFUSED, "rust"),
    # A repeated style taken: T_REFUSED's `validate(&twice).unwrap_err()` panics on Ok.
    Control("p47c_validate_repeat_kept", S, "        if kept.iter().any(|known: &RouteOrder| known.style == style) {",
            "        if false && kept.iter().any(|known: &RouteOrder| known.style == style) {", T_REFUSED, "rust"),
    # An empty question taken: T_REFUSED's `validate(&empty).unwrap_err()` panics on Ok.
    Control("p47c_validate_empty_question_kept", S, "            .filter(|question| !question.is_empty())\n",
            "            .filter(|_| true)\n", T_REFUSED, "rust"),
    # The schema without the eight: T_REFUSED's enum assert reads null.
    Control("p47c_schema_no_enum", S, '                    "style": {"type": "string", "enum": STYLES},',
            '                    "style": {"type": "string"},', T_REFUSED, "rust"),
    # ---- route: the call and the fallback ----
    # Tried once: T_FALLBACK's `calls.get()` is 1, not 2.
    Control("p47c_route_tried_once", S, "    for attempt in 1..=ROUTE_TRIES {", "    for attempt in 1..=1 {",
            T_FALLBACK, "rust"),
    # The fallback drops the owner's words: T_FALLBACK's question is "", not "Is wgpu still on 0.30?".
    Control("p47c_fallback_not_owners_words", S,
            "            style: FALLBACK_STYLE.to_owned(),\n            question: question.to_owned(),",
            "            style: FALLBACK_STYLE.to_owned(),\n            question: String::new(),", T_FALLBACK, "rust"),
    # The route turn given a tool: T_FALLBACK's stand-in asserts `--tools ""` and panics on "Read".
    Control("p47c_route_gets_a_tool", S, '        "--tools",\n        "",', '        "--tools",\n        "Read",',
            T_FALLBACK, "rust"),
    # The Method notes silent on a fallback: T_FALLBACK's notes lack "the routing failed".
    Control("p47c_method_silent_on_fallback", S, "        if route.fell_back {\n            lines.push(format!(",
            "        if false {\n            lines.push(format!(", T_FALLBACK, "rust"),
    # More than twenty titles: T_PROMPT's `existing_reports(..).len()` is 25, not 20.
    Control("p47c_titles_uncapped", S, "        .take(TITLES_MAX)\n", "        .take(TITLES_MAX + 5)\n", T_PROMPT,
            "rust"),
    # Oldest first: T_PROMPT's reports start with 2026-11-01-a.
    Control("p47c_titles_oldest_first", S, "    folders.reverse();\n", "", T_PROMPT, "rust"),
    # structured_output not read: T_PROMPT's answer_value of a result with "" text is an error.
    Control("p47c_answer_ignores_structured", S, '        .get("structured_output")\n',
            '        .get("structured_outputs")\n', T_PROMPT, "rust"),
    # The question left out of the prompt: T_PROMPT's "<question>\nShould I?\n</question>" assert.
    Control("p47c_prompt_drops_question", S, "        ROUTE_TEXT.trim_end(),\n        question.trim()\n",
            '        ROUTE_TEXT.trim_end(),\n        ""\n', T_PROMPT, "rust"),
    # ---- the planner ----
    # The writer never seated: T_ALLPASS's settle is Finish(no_delivery), not SeatWriter.
    Control("p47c_writer_never_seated", S, "    {\n        return Step::SeatWriter;\n    }\n    let why = lost_lines",
            "    {\n        return Step::Finish(ends::NO_DELIVERY, String::new());\n    }\n    let why = lost_lines",
            T_ALLPASS, "rust"),
    # A non-passing verdict's reason lost: T_ONEASKS's r2 why is "asked", not "gated asked: the worker ...".
    Control("p47c_not_delivered_why_plain", S,
            'Some(verdict) => ChainStatus::NotDelivered(id, format!("gated {verdict}: {}", why())),',
            'Some(verdict) => ChainStatus::NotDelivered(id, format!("{verdict}")),', T_ONEASKS, "rust"),
    # The writer not told what failed: T_ONEASKS's "- r2 · trajectory: sub-question 2 (why: ...)" assert.
    Control("p47c_writer_hears_no_miss", S,
            "        .filter(|chain| chain.end.as_ref().is_some_and(|end| !end.passed))\n",
            "        .filter(|_| false)\n", T_ONEASKS, "rust"),
    # The writer handed a failed researcher's findings: T_ONEASKS's `!text.contains(findings r2)` assert.
    Control("p47c_writer_gets_failed_findings", S,
            "        .filter(|chain| chain.end.as_ref().is_some_and(|end| end.passed))\n    {",
            "        .filter(|chain| chain.end.is_some())\n    {", T_ONEASKS, "rust"),
    # The root taken for the leaf: T_SENDBACK's last settle is Wait (the root reads sent back), not SeatWriter.
    Control("p47c_leaf_is_root", S,
            "        .filter(|order| crate::nirvana_gate::chain(run, order.id()).0 == root)\n        .last()",
            "        .filter(|order| crate::nirvana_gate::chain(run, order.id()).0 == root)\n        .next()",
            T_SENDBACK, "rust"),
    # A send-back ends the chain: T_SENDBACK's first settle is Finish(no_delivery), not Wait.
    Control("p47c_sent_back_ends_chain", S, '            Some("sent_back") => ChainStatus::Running(Some(id)),',
            '            Some("sent_back") => ChainStatus::NotDelivered(id, String::new()),', T_SENDBACK, "rust"),
    # The wrong end word: T_NONE's `end` is no_report, not no_delivery.
    Control("p47c_no_delivery_word", S, NO_DELIVERY,
            '    Step::Finish(ends::NO_REPORT, format!("no researcher passed: {why}"))', T_NONE, "rust"),
    # The reasons dropped: T_NONE's why lacks "r1 gated asked: 3 send-backs spent".
    Control("p47c_no_delivery_why_silent", S, NO_DELIVERY,
            '    Step::Finish(ends::NO_DELIVERY, format!("no researcher passed {}", why.len()))', T_NONE, "rust"),
    # No reseat: T_INTERRUPTED's first settle is Finish(no_delivery), not Reseat(0, "q-r1-r1").
    Control("p47c_interrupt_not_reseated", S, "        ChainStatus::Interrupted(..) if chain.reseat.is_none() => {",
            "        ChainStatus::Interrupted(..) if false && chain.reseat.is_none() => {", T_INTERRUPTED, "rust"),
    # The second interrupt unsaid: T_INTERRUPTED's why does not start "interrupted twice: ".
    Control("p47c_interrupted_twice_unsaid", S, '            why: Some(format!("interrupted twice: {why}")),',
            "            why: Some(why),", T_INTERRUPTED, "rust"),
    # The research cap one second late: T_CAP's at-cap step is Wait, not Stop.
    Control("p47c_research_cap_late", S, "        if now.saturating_sub(state.stage_at) >= caps.research_s {",
            "        if now.saturating_sub(state.stage_at) > caps.research_s {", T_CAP, "rust"),
    # The writer's cap one second late: T_CAP's writer step is Wait, not Stop.
    Control("p47c_write_cap_late", S, "            if now.saturating_sub(state.stage_at) >= caps.write_s {",
            "            if now.saturating_sub(state.stage_at) > caps.write_s {", T_CAP, "rust"),
    # The cap's minutes changed: T_CAP's `RESEARCH_CAP_S == 90 * 60` assert.
    Control("p47c_cap_constant", S, "pub const RESEARCH_CAP_S: u64 = 90 * 60;", "pub const RESEARCH_CAP_S: u64 = 60 * 60;",
            T_CAP, "rust"),
    # The variable not read: T_CAP's env_secs is 99, not 7.
    Control("p47c_env_ignored", S, "        .and_then(|value| value.trim().parse().ok())\n",
            "        .and_then(|value: String| value.trim().parse::<u64>().ok().filter(|_| false))\n", T_CAP, "rust"),
    # A passing writer unchecked: T_WRITER's settle is Finish(report), not Check.
    Control("p47c_writer_pass_unchecked", S, "        return if end.passed {\n            Step::Check\n",
            "        return if end.passed {\n            Step::Finish(ends::REPORT, String::new())\n", T_WRITER, "rust"),
    # The writer's failure unsaid: T_WRITER's why lacks "owned: a tracked file outside owns".
    Control("p47c_writer_fail_silent", S,
            '                format!(\n                    "the writer did not deliver: {}",\n'
            '                    end.why.as_deref().unwrap_or("")\n                ),',
            '                format!("the writer did not deliver"),', T_WRITER, "rust"),
    # A stop that stops nothing: T_STOP's research step is Finish(stopped), not Stop([(0, q-r1)]).
    Control("p47c_stop_ignored_mid_stage", S, "        if !live.is_empty() {\n            return Step::Stop(",
            "        if false {\n            return Step::Stop(", T_STOP, "rust"),
    # The marker not read: T_STOP's route step is Route, not Finish(stopped).
    Control("p47c_stop_ignored_in_route", S, "    if stop || state.stage == Stage::Stopping {",
            "    if state.stage == Stage::Stopping {", T_STOP, "rust"),
    # A stopped run waits for its wake: T_STOP's last step is Wait, not EndRun.
    Control("p47c_stopped_waits_for_wake", S, "        if end == ends::STOPPED\n            || caps.wakeable",
            "        if false\n            || caps.wakeable", T_STOP, "rust"),
    # An unseated order left: T_RESUME's research step is Wait, not Seat(1).
    Control("p47c_resume_skips_unseated", S, "            None => ChainStep::Seat,\n",
            "            None => ChainStep::Running(None),\n", T_RESUME, "rust"),
    # Never told: T_RESUME's ending step is Log, not Tell.
    Control("p47c_never_tells", S, "        if !state.told {\n            return Step::Tell;",
            "        if false && !state.told {\n            return Step::Tell;", T_RESUME, "rust"),
    # ---- ending ----
    # A typed wake not seen: T_ENDWAIT's typed step is Wait, not EndRun.
    Control("p47c_end_ignores_typed", S, "            || run.queue.is_empty()\n            || waited",
            "            || false\n            || waited", T_ENDWAIT, "rust"),
    # No bound: T_ENDWAIT's bound step is Wait, not EndRun.
    Control("p47c_end_no_bound", S, "            || waited >= caps.end_wait_s\n", "            || waited > u64::MAX - 1\n",
            T_ENDWAIT, "rust"),
    # Waits for a pane nobody can wake: T_ENDWAIT's unwakeable step is Wait, not EndRun.
    Control("p47c_end_waits_for_nobody", S, "            || caps.wakeable == Some(false)\n", "", T_ENDWAIT, "rust"),
    # The bound's length changed: T_ENDWAIT's `END_WAIT_S == 6 * 3600` assert.
    Control("p47c_end_wait_constant", S, "pub const END_WAIT_S: u64 = 6 * 3600;", "pub const END_WAIT_S: u64 = 3600;",
            T_ENDWAIT, "rust"),
    # A run ended elsewhere not seen: T_ENDWAIT's last step is EndRun, not Done.
    Control("p47c_ended_run_not_done", S, "        if run.ended.is_some() {\n            return Step::Done;",
            "        if false {\n            return Step::Done;", T_ENDWAIT, "rust"),
    # ---- the replay hunk ----
    # The hunk off: T_REPLAY's scholar queue holds every s- line, not only the stall.
    Control("p47c_replay_hunk_off", RUN, "        let owed = owed && !scholar_quiet;\n",
            "        let owed = owed && !(scholar_quiet && false);\n", T_REPLAY, "rust"),
    # The child's own why read, not the root's: T_REPLAY's scholar queue holds s-r2-s1's lines.
    Control("p47c_replay_child_not_followed", RUN,
            "                        .order(&crate::nirvana_gate::chain(&state, known.id()).0)\n",
            "                        .order(known.id())\n", T_REPLAY, "rust"),
    # A send-back verdict queued: T_REPLAY's scholar queue holds "s-r2 gated sent_back".
    Control("p47c_replay_send_back_verdict_queued", RUN,
            "                    crate::nirvana_gate::Outcome::SentBack.word(),\n", "", T_REPLAY, "rust"),
    # Fix round F1: the writer's lines loud again: T_REPLAY's scholar queue holds the s-w and s-w-r1 lines.
    Control("p47c_replay_writer_loud", RUN, "                    crate::nirvana_scholar::WRITE_CLASS,\n                ]",
            "                    \"never\",\n                ]", T_REPLAY, "rust"),
    # A stall made quiet: T_REPLAY's scholar queue is empty, not the s-r7 stall line.
    Control("p47c_replay_stall_quiet", RUN,
            "                Kind::Ended { outcome, .. } => outcome != \"delivered\",\n                _ => false,",
            "                Kind::Ended { outcome, .. } => outcome != \"delivered\",\n                _ => true,",
            T_REPLAY, "rust"),
    # Fix round F1 and F7, the hunk's kinds as built (only a delivery): T_REPLAY's scholar queue holds the
    # interrupted lines of s-r4, s-r5 and s-w and the asks line of s-r3.
    Control("p47c_fix_interrupt_and_ask_loud", RUN, QUIET_KINDS,
            "                Kind::Delivered { .. } => true,\n", T_REPLAY, "rust"),
    # Fix round F7: a gated asked loud: T_REPLAY's scholar queue holds "s-r3 gated asked: blocked".
    Control("p47c_fix_gated_asked_loud", RUN, "                    crate::nirvana_gate::Outcome::Asked.word(),\n", "",
            T_REPLAY, "rust"),
    # A stop and a failure loud: T_REPLAY's scholar queue holds "s-r5 ended stopped" and "s-r6 ended failed".
    Control("p47c_fix_ended_loud", RUN, "                Kind::Ended { outcome, .. } => outcome != \"delivered\",\n",
            "                Kind::Ended { .. } => false,\n", T_REPLAY, "rust"),
    # An ordinary order made quiet: T_REPLAY's ordinary queue holds the o-r7 stall only, not all 17 lines.
    Control("p47c_fix_ordinary_quiet", RUN,
            "                            root.seated.why.as_deref() == Some(crate::nirvana_scholar::STAGE_WHY)\n",
            "                            root.seated.why.as_deref() != Some(\"never\")\n", T_REPLAY, "rust"),
    # Through the driver: a dead worker's line wakes the orchestrator mid-run: T_MATRIX's Dies rows read mid_run 1.
    Control("p47c_fix_matrix_interrupt_loud", RUN, QUIET_KINDS,
            "                Kind::Delivered { .. } | Kind::Asked { .. } => true,\n", T_MATRIX, "rust"),
    # F5's order turned round, the wake before `told` is saved: T_MATRIX's WakeBefore(1) rows read wake_calls 1, not 0
    # (the resume tells and wakes again).
    Control("p47c_fix_wake_before_told", S, TELL_THEN_WAKE,
            "            (world.wake)(project, &state.run);\n            state.told = true;\n"
            "            save_state(project, state)?;\n", T_MATRIX, "rust"),
    # A delivered order's row dropped: T_ROWS's rows lack rows-r1.
    Control("p47c_rows_hide_delivered", SUB,
            "                crate::nirvana_run::Phase::Merged\n                    | crate::nirvana_run::Phase::Gated\n",
            "                crate::nirvana_run::Phase::Merged\n                    | crate::nirvana_run::Phase::Delivered\n"
            "                    | crate::nirvana_run::Phase::Gated\n", T_ROWS, "rust"),
    # ---- the writer's path in the wake ----
    # The order asks for the path last: T_WAKEPATH's "`summary` starts with `<path>`" assert.
    Control("p47c_writer_text_path_unasked", S, "`summary` starts with `{}`, then the answer line.",
            "`summary` ends with `{}`, then the answer line.", T_WAKEPATH, "rust"),
    # No owns line: T_WAKEPATH's "\nowns: <dir>/\n" assert.
    Control("p47c_writer_owns_unsaid", S, "owns: {dir}/", "owned: {dir}/", T_WAKEPATH, "rust"),
    # The run's question cut shorter: T_WAKEPATH's queue line lacks the 77-character path and its " (".
    Control("p47c_queue_line_too_short", RUN, '            None => format!("the run asks: {}", short(question, 120)),',
            '            None => format!("the run asks: {}", short(question, 60)),', T_WAKEPATH, "rust"),
    # Fix round item 1: the report's end line with the status command first: T_WAKEPATH's queue line does not start
    # "the run asks: scholar report: <path>: ". Retargeted in the second fix round (review 2 F3 moved the command last).
    Control("p47c_fix_report_path_late", S,
            '    format!("scholar report{count}: {why}{answer}{lost}{status}")',
            '    format!("scholar report{count}: (`herdr scholar status {}`) {why}{answer}{lost}", state.run)', T_WAKEPATH,
            "rust"),
    # ---- the verb ----
    # A seated worker not refused: T_REFUSALS's first line is the git refusal, not "refused inside order x-r1".
    Control("p47c_seated_not_refused", S,
            "        env(crate::nirvana_profiles::SEATED_ORDER_ENV).filter(|order| !order.trim().is_empty())",
            '        env("NIRVANA_ORDER_X").filter(|order| !order.trim().is_empty())', T_REFUSALS, "rust"),
    # An empty question not refused: T_REFUSALS's second line is the git refusal.
    Control("p47c_empty_not_refused", S, "    if question.is_empty() {\n        return Err(refuse(",
            "    if false {\n        return Err(refuse(", T_REFUSALS, "rust"),
    # A folder that is not a repository taken: T_REFUSALS's third `unwrap_err` panics on Ok.
    Control("p47c_non_git_not_refused", S, "    if !is_git_repo(&project.root) {",
            "    if false && !is_git_repo(&project.root) {", T_REFUSALS, "rust"),
    # A live run not refused: T_REFUSALS's busy `unwrap_err` panics on Ok.
    Control("p47c_busy_not_refused", S, "            if unfinished && driver_live(project, &run) {",
            "            if false && unfinished && driver_live(project, &run) {", T_REFUSALS, "rust"),
    # The opened line without the pane: T_OPENED's orchestrator_pane is None, not "wT:p7".
    Control("p47c_opened_no_pane", S, "                orchestrator_pane: pane.map(str::to_owned),\n",
            "                orchestrator_pane: None,\n", T_OPENED, "rust"),
    # The driver drops the pane from its env: T_OPENED's envs list has a second entry.
    Control("p47c_driver_drops_pane_env", S, '        .args(["scholar", "drive", run])\n',
            '        .args(["scholar", "drive", run])\n        .env_remove("HERDR_PANE_ID")\n', T_OPENED, "rust"),
    # ---- slugs and ids ----
    # Not cut: T_SLUG's `cut.len() <= SLUG_MAX`.
    Control("p47c_slug_uncut", S, "    if out.len() <= SLUG_MAX {\n        return out;",
            "    if out.len() <= SLUG_MAX * 9 {\n        return out;", T_SLUG, "rust"),
    # The lesson word for no ASCII: T_SLUG's `starts_with("question-")`.
    Control("p47c_slug_lesson_word", S, '        return format!("question-{}", &sha[..8]);',
            '        return format!("lesson{}", &sha[..0]);', T_SLUG, "rust"),
    # Dots kept: T_SLUG's path slug is not "windows-system32-config-sam-txt".
    Control("p47c_slug_keeps_dots", S, "        if c.is_ascii_alphanumeric() {\n            if gap",
            "        if c.is_ascii_alphanumeric() || c == '.' {\n            if gap", T_SLUG, "rust"),
    # No -2: T_SLUG's second run_id equals the first.
    Control("p47c_ids_never_suffixed", S, "    if !taken(base) {", "    if true || !taken(base) {", T_SLUG, "rust"),
    # A word that ends at the cap cut: T_SLUG's "aaaaaaaaaa-bbbbbbbbbb-cccccccccc-ddddddd" assert.
    Control("p47c_slug_word_at_cap_cut", S, "    if out.as_bytes()[SLUG_MAX] == b'-' {",
            "    if false && out.as_bytes()[SLUG_MAX] == b'-' {", T_SLUG, "rust"),
    # ---- the html check ----
    # A script src passes: T_HTML's first bad page finds [], not ["<script> src: app.js"].
    Control("p47c_html_script_src_ok", S, '            if let Some(src) = get("src") {\n                push("src", &src);',
            '            if let Some(src) = get("src").filter(|_| false) {\n                push("src", &src);', T_HTML,
            "rust"),
    # A stylesheet from a host passes: T_HTML's <link> page finds [].
    Control("p47c_html_link_ok", S, LINK_FILTER, LINK_FILTER.replace(".filter(|href|", ".filter(|_| false).filter(|href|"),
            T_HTML, "rust"),
    # An @import passes: T_HTML's @import page finds [].
    Control("p47c_html_import_ok", S, '    if let Some(at) = css.find("@import") {',
            '    if let Some(at) = css.find("@import").filter(|_| false) {', T_HTML, "rust"),
    # A url() from a host passes: T_HTML's url() pages find [].
    Control("p47c_html_url_ok", S, "        if outside(&css[at + 4..]) {", "        if false && outside(&css[at + 4..]) {",
            T_HTML, "rust"),
    # Fix round F4: a canonical link fails: T_HTML2's good row `<link rel="canonical" ...>` finds a <link> line.
    Control("p47c_html_canonical_fails", S,
            "            let followed =\n                !words.is_empty() && words.iter().all(|word| LINK_FOLLOWED.contains(word));",
            "            let followed = false;", T_HTML2, "rust"),
    # F4: the whole page read as CSS again: T_HTML2's first url() row finds a second, "<page> url(: ...", line (and the
    # prose rows would find lines).
    Control("p47c_fix_html_reads_prose", S, "    found.dedup();\n    found\n}",
            '    css_loads("<page>", &lower, &mut found);\n    found.dedup();\n    found\n}', T_HTML2, "rust"),
    # Quotes not read as quotes: T_HTML2's first row's value keeps its quotes ("<script> src: \"app.js\"").
    Control("p47c_fix_html_quotes_ignored", S, "                Some(&quote) if quote == b'\"' || quote == b'\\'' => {",
            "                Some(&quote) if false && (quote == b'\"' || quote == b'\\'') => {", T_HTML2, "rust"),
    # An entity left raw: T_HTML2's `&#104;ttps:` row finds [].
    Control("p47c_fix_html_entities_raw", S, "    decode_entities(raw)\n        .chars()",
            "    raw.to_owned()\n        .chars()", T_HTML2, "rust"),
    # A backslash host taken for a path: T_HTML2's `\\evil.example\share` row finds [].
    Control("p47c_fix_html_backslash_ok", S, '    ["//", "\\\\\\\\", "/\\\\", "\\\\/"]', '    ["//"]', T_HTML2, "rust"),
    # svg loads pass: T_HTML2's <image> and <use> rows find [].
    Control("p47c_fix_html_svg_ok", S, '        "image" | "use" | "feimage" => {', '        "never-a-tag" => {', T_HTML2,
            "rust"),
    # A <base> passes: T_HTML2's <base> row finds [].
    Control("p47c_fix_html_base_ok", S, '        "base" => {', '        "base-never" => {', T_HTML2, "rust"),
    # A meta refresh to a host passes: T_HTML2's <meta http-equiv=refresh> row finds [].
    Control("p47c_fix_html_meta_ok", S,
            '            if get("http-equiv").as_deref() == Some("refresh") && target.is_some_and(outside) {',
            '            if false && target.is_some_and(outside) {', T_HTML2, "rust"),
    # A form to a host passes: T_HTML2's <form action> row finds [].
    Control("p47c_fix_html_form_ok", S, '        "form" => {', '        "form-never" => {', T_HTML2, "rust"),
    # A background from a host passes: T_HTML2's <body background> row finds [].
    Control("p47c_fix_html_background_ok", S,
            '    if let Some(value) = get("background").filter(|value| outside(value)) {',
            '    if let Some(value) = get("background").filter(|_| false) {', T_HTML2, "rust"),
    # A script that talks passes: T_HTML2's fetch( row finds [].
    Control("p47c_fix_html_script_talks_ok", S, '                    script_talks("<script>", body, &mut found)',
            '                    script_talks("<script>", "", &mut found)', T_HTML2, "rust"),
    # The claims data block read as a script: T_HTML2's application/json good row finds "<script> fetch(: ...".
    Control("p47c_fix_html_json_runs", S, '                .any(|(key, value)| key == "type" && value.contains("json"));',
            "                .any(|_| false);", T_HTML2, "rust"),
    # An event handler passes: T_HTML2's onerror row finds [].
    Control("p47c_fix_html_handlers_ok", S,
            '    for (key, value) in tag.attrs.iter().filter(|(key, _)| key.starts_with("on")) {',
            "    for (key, value) in tag.attrs.iter().filter(|_| false) {", T_HTML2, "rust"),
    # The value not cut: T_HTML2's long row is 221 characters of value, not 80.
    Control("p47c_fix_html_value_uncut", S, "    value.chars().take(80).collect()", "    value.chars().take(800).collect()",
            T_HTML2, "rust"),
    # A comment read as markup: T_HTML2's comment row finds "<img> src: https://evil.example/x.png".
    Control("p47c_fix_html_comment_read", S,
            '            at = comment\n                .find("-->")\n                .map_or(lower.len(), |close| start + 3 + close + 3);',
            "            at = start + 3;", T_HTML2, "rust"),
    # A srcset read by its first candidate: T_HTML2's "a.png 1x, https://..." row finds [].
    Control("p47c_fix_html_srcset_first_only", S,
            "                .filter(|set| set.split(',').any(|candidate| outside(candidate.trim())))\n",
            "                .filter(|set| outside(set))\n", T_HTML2, "rust"),
    # An empty page passes: T_HTML's check_files of "  \n" is [] not ["report.html is empty"].
    Control("p47c_html_empty_ok", S, "            Ok(text) if !text.trim().is_empty() => {",
            "            Ok(text) if true || !text.trim().is_empty() => {", T_HTML, "rust"),
    # ---- the order texts ----
    # Not quoted: T_TEXTS's spec reads `owns: src/` from the question.
    Control("p47c_quote_off", S, '        .map(|line| format!("> {}", line.trim_end()))',
            "        .map(|line| line.trim_end().to_owned())", T_TEXTS, "rust"),
    # The card unnamed: T_TEXTS's "Read the card `benchmark`" assert.
    Control("p47c_card_unnamed", S,
            "Read the card `{style}` before you start, and `fetch-ladder` when a fetch fails.",
            "Read your card before you start.", T_TEXTS, "rust"),
    # The scratch folder unsaid: T_TEXTS's "under `.nirvana/runs/...`" assert.
    Control("p47c_scratch_unsaid", S,
            "Put every working file, download and screenshot under `{scratch}` and nowhere else.",
            "Put every working file where you like.", T_TEXTS, "rust"),
    # ---- state, lock, log ----
    # An unknown version guessed: T_VERSION's `load_state(..).unwrap_err()` panics on Ok.
    Control("p47c_version_guessed", S, "        Some(v) if v == u64::from(STATE_VERSION) => {}",
            "        Some(_) => {}", T_VERSION, "rust"),
    # The holder unnamed: T_VERSION's refusal lacks ": pid <me>".
    Control("p47c_holder_unnamed", S, '            Err(format!("a driver holds {run}: {}", holder(project, run)))',
            '            Err(format!("a driver holds {run}"))', T_VERSION, "rust"),
    # A held lock reads free: T_VERSION's `driver_live` is false while held.
    Control("p47c_driver_live_never", S, "    driver_dir(project, run).exists()\n        && matches!(",
            "    false\n        && matches!(", T_VERSION, "rust"),
    # The outcome pre-set: T_LOG's outcome is "hit", not null.
    Control("p47c_log_outcome_set", S, '        "outcome": Value::Null,', '        "outcome": "hit",', T_LOG, "rust"),
    # Send-backs uncounted: T_LOG's send_backs is 0, not 1.
    Control("p47c_log_send_backs_uncounted", S,
            '    let send_backs = members\n        .iter()\n        .filter(|order| order.seated.why.as_deref() == Some("send_back"))',
            '    let send_backs = members\n        .iter()\n        .filter(|order| order.seated.why.as_deref() == Some("reseat"))',
            T_LOG, "rust"),
    # A run the log lacks tagged: T_LOG's `unwrap_err()` on scholar-nope panics on Ok.
    Control("p47c_tag_unknown_run_taken", S, "    if !lines\n        .iter()\n        .any(|line| line.get(\"end\")",
            "    if false && !lines\n        .iter()\n        .any(|line| line.get(\"end\")", T_LOG, "rust"),
    # The first tag wins: T_LOG's outcome_of is "miss", not "hit".
    Control("p47c_first_tag_wins", S,
            "    lines\n        .iter()\n        .rev()\n        .filter(|line| line.get(\"end\").is_none()",
            "    lines\n        .iter()\n        .filter(|line| line.get(\"end\").is_none()", T_LOG, "rust"),
    # Oldest first: T_LOG's first row is the old run.
    Control("p47c_list_oldest_first", S, "    rows.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));",
            "    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));", T_LOG, "rust"),
    # ---- a whole run ----
    # Seated without the mark: T_E2E's `seated.why == scholar_stage` assert (and the researchers then wake).
    Control("p47c_seat_no_why", S, "        Some(STAGE_WHY),\n    )\n    .map_err(|(code, why)|",
            "        None,\n    )\n    .map_err(|(code, why)|", T_E2E, "rust"),
    # The worker's hunk undone: T_E2E's `seated.why` is None.
    Control("p47c_worker_why_dropped", WORKER, "        why: why.map(str::to_owned),\n", "        why: None,\n", T_E2E,
            "rust"),
    # Researchers seated with no profile: T_E2E's `seated.profile` is Some(None), not Some(Some("scholar")).
    Control("p47c_seat_no_profile", S, "                profile: Some(PROFILE.to_owned()),\n",
            "                profile: None,\n", T_E2E, "rust"),
    # The writer seated as research: T_E2E's writer class is research.web, not write.docs.
    Control("p47c_writer_as_research", S, "                class: WRITE_CLASS.to_owned(),\n",
            "                class: RESEARCH_CLASS.to_owned(),\n", T_E2E, "rust"),
    # Scratch outside .nirvana: T_E2E's listing gains files under scratch/.
    Control("p47c_scratch_outside_nirvana", S, '    format!("{}/runs/{run}/{DIR}/r{n}/", nirvana_run::DIR)',
            '    format!("scratch/{run}/{DIR}/r{n}/")', T_E2E, "rust"),
    # No log line: T_E2E's `read_log(..).len()` is 0, not 1.
    Control("p47c_log_not_written", S,
            "            append_run_line(&project.root, &log_line(state, run, now))?;\n", "", T_E2E, "rust"),
    # The run never ended: T_E2E's `ended.get()` is false.
    Control("p47c_run_not_ended", S, "            match (world.end_run)(project, &state.run) {",
            "            match Ok::<(), String>(()) {", T_E2E, "rust"),
    # ---- nothing to research ----
    # The empty answer researched anyway: T_NOTHING's end is no_delivery, not nothing_to_research.
    Control("p47c_nothing_researched_anyway", S, "            let nothing = state.orders.is_empty();",
            "            let nothing = false;", T_NOTHING, "rust"),
    # The tell line without the status command, the run id where the why was: T_NOTHING's asked line does not start
    # "scholar nothing_to_research: the route found nothing" (retargeted in the second fix round, review 2 F3).
    Control("p47c_tell_line_plain", S,
            '        return format!("scholar {end}{count}: {why}{lost}{status}");',
            '        return format!("scholar {end} {}{count}; {why}{lost}", state.run);', T_NOTHING, "rust"),
    # Told but not woken: T_NOTHING's wakes is 0, not 1.
    Control("p47c_tell_no_wake", S, TELL_THEN_WAKE,
            "            state.told = true;\n            save_state(project, state)?;\n", T_NOTHING, "rust"),
    # ---- stop_order and the cap in the driver ----
    # stop_order writes no stop: T_STOPORDER's status is Running, not NotDelivered("stopped").
    Control("p47c_stop_order_writes_nothing", WORKER,
            "    let mut out = Vec::new();\n    stopped(project, &journal, run, &known.seated, None, &mut out)?;\n",
            "    let out: Vec<u8> = b\"stopped\".to_vec();\n", T_STOPORDER, "rust"),
    # The cap stops nothing: T_CAPDRIVE's stops list is empty.
    Control("p47c_cap_stop_not_called", S, "                    let said = (world.stop)(project, &state.run, &leaf);",
            '                    let said = String::from("not stopped");', T_CAPDRIVE, "rust"),
    # The cap's reason lost: T_CAPDRIVE's why is None, not "over the time cap".
    Control("p47c_cap_why_lost", S,
            "                    why: Some(why.clone()),\n                    at: now,\n                };\n                match at {",
            "                    why: None,\n                    at: now,\n                };\n                match at {",
            T_CAPDRIVE, "rust"),
    # ---- the role text and the seat skill ----
    # ROLE.md without the verb: T_ROLE's "goes to `herdr scholar \"<question>\"`" assert.
    Control("p47c_role_unsent", ROLE, 'goes to `herdr scholar "<question>"`', "goes to a research order", T_ROLE,
            "rust"),
    # The seat skill without its section: T_ROLE's heading assert.
    Control("p47c_skill_unsaid", SKILL, "### A research question: `herdr scholar`", "### Research", T_ROLE, "rust"),
    # Fix round F12: two profiles for a lookup again: T_ROLE's "seated as the line above says" assert.
    Control("p47c_fix_role_two_profiles", ROLE, "order, seated as the line above says.",
            "order, seated with `--profile scholar`.", T_ROLE, "rust"),
    # F12: the role silent on doing the research twice: T_ROLE's "do not also run the research yourself" assert.
    Control("p47c_fix_role_research_twice", ROLE, "wait for its wake: do\n  not also run the research yourself",
            "wait for its wake, or\n  run the research yourself", T_ROLE, "rust"),
    # ---- the fix round: what the end line says (item 1) ----
    # The lost list dropped from the end line: T_LOST's line does not start "...linker.; did not deliver: ".
    Control("p47c_fix_lost_unsaid", S, '        format!("; did not deliver: {}", lost.join("; "))\n',
            "        String::new()\n", T_LOST, "rust"),
    # The Method notes silent on what did not deliver: T_LOST's "- did not deliver: r2 ..." assert.
    Control("p47c_fix_method_lost_unsaid", S, "            .filter(|line| line.starts_with('r'))\n",
            "            .filter(|_| false)\n", T_LOST, "rust"),
    # A question quoted whole: T_LOST's r3 line is not the 200-character quote.
    Control("p47c_fix_question_uncut", S,
            "                question.chars().take(LOST_WHY_MAX).collect::<String>()\n",
            "                question.to_owned()\n", T_LOST, "rust"),
    # A reseat unsaid: T_LOST's r2 line lacks "(interrupted and reseated once)".
    Control("p47c_fix_reseat_unsaid", S, '            extra.push("interrupted and reseated once".to_owned());\n', "",
            T_LOST, "rust"),
    # A send-back unsaid: T_LOST's r3 line lacks "(sent back 1 time(s))".
    Control("p47c_fix_send_backs_unsaid", S, '            extra.push(format!("sent back {send_backs} time(s)"));\n', "",
            T_LOST, "rust"),
    # The writer's reseat unsaid: T_LOST's line lacks "the writer was interrupted and reseated once as q-write-r1".
    Control("p47c_fix_writer_reseat_unsaid", S,
            '        lines.push(format!("the writer was interrupted and reseated once as {reseat}"));\n', "", T_LOST, "rust"),
    # The writer's answer dropped from the report's line: T_E2E's told line does not end "): The build is slow ...".
    Control("p47c_fix_report_answer_dropped", S, "        .filter(|answer| !answer.is_empty())\n",
            "        .filter(|_| false)\n", T_E2E, "rust"),
    # ---- the report folder (item 2, F2) ----
    # Another run's claim not read: T_TWO's forced folder equals the first's.
    Control("p47c_fix_claims_ignored", S, "        root.join(rel).exists() || claimed.iter().any(|dir| dir == rel)\n",
            "        root.join(rel).exists()\n", T_TWO, "rust"),
    # The verb passes no claims: T_TWO's forced folder equals the first's.
    Control("p47c_fix_claims_unread", S,
            "    let report_dir = report_dir(&project.root, &date, &slug, &claimed_dirs(project));",
            "    let report_dir = report_dir(&project.root, &date, &slug, &[]);", T_TWO, "rust"),
    # ---- the log (item 3, F3 and F11) ----
    # No newline put back: T_LOGNL's (lines, bad) is (0, 1), not (2, 0).
    Control("p47c_fix_log_no_newline", S, "        if last[0] != b'\\n' {\n            text.insert(0, '\\n');",
            "        if false {\n            text.insert(0, '\\n');", T_LOGNL, "rust"),
    # A bad line not counted: T_LOGNL's (lines, bad) is (3, 0), not (3, 1).
    Control("p47c_fix_log_bad_uncounted", S, "            _ => bad += 1,\n", "            _ => {}\n", T_LOGNL, "rust"),
    # The count unsaid: T_LOGNL's list lacks "1 line(s) of docs/research/log.jsonl could not be read".
    Control("p47c_fix_log_bad_unsaid", S, "    out.extend(unread);\n", "    let _ = unread;\n", T_LOGNL, "rust"),
    # A run line written twice: T_LOGTWICE's count is 2, not 1.
    Control("p47c_fix_log_twice", S, "    {\n        return Ok(false);\n    }\n    append_line(",
            "    {\n        let _ = false;\n    }\n    append_line(", T_LOGTWICE, "rust"),
    # ---- the verbs a worker must not run (item 4, F6) ----
    # No guard: T_VERBS's drive in a worker drives (its route stand-in panics, caught as "the driver panicked").
    Control("p47c_fix_worker_guard_off", S,
            "    match env(crate::nirvana_profiles::SEATED_ORDER_ENV).filter(|order| !order.trim().is_empty()) {",
            '    match env("NIRVANA_ORDER_X").filter(|order| !order.trim().is_empty()) {', T_VERBS, "rust"),
    # `stop` unguarded: T_VERBS's stop in a worker ends the run (Ok), "stop ran in a worker".
    Control("p47c_fix_stop_in_worker", S, '    refused_in_worker(env, "stop")?;\n', "", T_VERBS, "rust"),
    # `tag` unguarded: T_VERBS's tag in a worker reads "no scholar run ... in log", not the refusal.
    Control("p47c_fix_tag_in_worker", S, '    refused_in_worker(env, "tag")?;\n', "", T_VERBS, "rust"),
    # `drive` reads no state first: T_VERBS's unknown run gets the gave-up asked line (a journal in the listing).
    Control("p47c_fix_drive_verb_no_read", S, '    refused_in_worker(env, "drive")?;\n    load_state(project, run)?;\n',
            '    refused_in_worker(env, "drive")?;\n', T_VERBS, "rust"),
    # `drive()` takes the lock before it reads: T_VERBS's direct drive of an unknown run leaves driver.json.
    Control("p47c_fix_drive_lock_first", S, "    load_state(project, run)?;\n    let _lock = take_driver(project, run)?;",
            "    let _lock = take_driver(project, run)?;", T_VERBS, "rust"),
    # `stop` writes before it reads: T_VERBS's listing gains the unknown run's stop file.
    Control("p47c_fix_stop_writes_first", S, '    refused_in_worker(env, "stop")?;\n    let state = load_state',
            '    refused_in_worker(env, "stop")?;\n    let _ = std::fs::create_dir_all(scholar_dir(project, run))'
            '.and_then(|()| std::fs::write(stop_marker(project, run), "0"));\n    let state = load_state', T_VERBS, "rust"),
    # ---- ends that leave a trace (item 6, F10 and F11) ----
    # A panic not caught: T_PANIC's drive_verb panics out.
    Control("p47c_fix_panic_uncaught", S, '        Err(format!("the driver panicked: {what}"))',
            "        std::panic::resume_unwind(Box::new(what))", T_PANIC, "rust"),
    # The refusal counted in polls: T_ENDREFUSED's tries is 60, not 361.
    Control("p47c_fix_end_tries_60", S, "                    if now.saturating_sub(first) < END_REFUSED_S {",
            "                    if state.end_tries < 60 {", T_ENDREFUSED, "rust"),
    # The bound changed: T_ENDREFUSED's `END_REFUSED_S == 30 * 60` assert.
    Control("p47c_fix_end_refused_constant", S, "pub const END_REFUSED_S: u64 = 30 * 60;",
            "pub const END_REFUSED_S: u64 = 5 * 60;", T_ENDREFUSED, "rust"),
    # The command unnamed: T_ENDREFUSED's note lacks "`herdr run end <run>`".
    Control("p47c_fix_end_command_unnamed", S, '                    let command = format!("`herdr run end {}`", state.run);',
            '                    let command = "`herdr run end`".to_owned();', T_ENDREFUSED, "rust"),
    # Given up in silence: T_ENDREFUSED's wakes is 0, not 1.
    Control("p47c_fix_end_untold", S,
            "                    state.stage = Stage::Done;\n                    save_state(project, state)?;\n"
            "                    (world.wake)(project, &state.run);\n",
            "                    state.stage = Stage::Done;\n                    save_state(project, state)?;\n",
            T_ENDREFUSED, "rust"),
    # ---- small (item 7, F12) ----
    # A report named for every run: T_STATUS's running run prints "report:".
    Control("p47c_fix_report_always", S, "    let report = (state.end.as_deref() == Some(ends::REPORT))\n",
            "    let report = (true || state.end.as_deref() == Some(ends::REPORT))\n", T_STATUS, "rust"),
    # One try for the driver's lock: T_RACE's take loses to the held look ("a driver holds ...").
    Control("p47c_fix_driver_one_try", S, "pub const DRIVER_TAKE_ATTEMPTS: u32 = 40;",
            "pub const DRIVER_TAKE_ATTEMPTS: u32 = 1;", T_RACE, "rust"),
    # ---- the question's cap (item 8) ----
    # No cap: T_QCAP's 8,001 `unwrap_err` panics on Ok.
    Control("p47c_fix_question_uncapped", S, "    if length > QUESTION_MAX {", "    if length > QUESTION_MAX * 9 {", T_QCAP,
            "rust"),
    # Bytes, not characters: T_QCAP's 8,000 two-byte characters are refused.
    Control("p47c_fix_question_bytes", S, "    let length = question.chars().count();", "    let length = question.len();",
            T_QCAP, "rust"),
    # ---- the second review (review 2 F1 to F7; the brief p47c-fix2 items 1 to 7) ----
    # Each comment says the assertion that goes red and why, written before the run.
    # F1. `check` never asks the scholar answer: T_CHECK_DEAD's first Dies assert reads the old
    # Ok("...-r2 was interrupted: the orchestrator continues, redoes or asks"), not Err(resume).
    Control("p47c_fix2_check_unasked", RUN,
            "    if let Some(answer) = crate::nirvana_scholar::check_answer(state, now) {\n        return answer;\n    }\n",
            "", T_CHECK_DEAD, "rust"),
    # F1. Any scholar run of the second matches: T_CHECK_PLAIN's case "live" reads the scholar run's Err(resume), not
    # Ok("worker live: o-1").
    Control("p47c_fix2_check_any_scholar_run", S, "            nirvana_run::replay(&upto) == *run\n",
            "            !upto.is_empty()\n", T_CHECK_PLAIN, "rust"),
    # F1. The lock never read: T_CHECK_DEAD's Dies assert after `take_driver` reads Err(resume), not CHECK_HELD.
    Control("p47c_fix2_check_no_lock_read", S, "    let held = driver_live(&project, &state.run);\n",
            "    let held = false;\n", T_CHECK_DEAD, "rust"),
    # F1. The look takes the lock as a driver does: T_CHECK_LOCK's driver.json is rewritten with this pid, not
    # {"pid":1,"at":5}.
    Control("p47c_fix2_check_takes_lock", S, "    let held = driver_live(&project, &state.run);\n",
            "    let held = take_driver(&project, &state.run).is_err();\n", T_CHECK_LOCK, "rust"),
    # F1. No grace: T_CHECK_DEAD's "before the route" assert reads Err(resume), not CHECK_STARTING.
    Control("p47c_fix2_check_no_grace", S, "pub const START_GRACE_S: u64 = 60;", "pub const START_GRACE_S: u64 = 0;",
            T_CHECK_DEAD, "rust"),
    # F1. The grace ignores `driver.json`: T_CHECK_DEAD's first Dies assert (35 s after the start) reads
    # CHECK_STARTING, not Err(resume).
    Control("p47c_fix2_check_grace_after_take", S,
            "        && !scholar_dir(&project, &state.run).join(\"driver.json\").exists()\n", "", T_CHECK_DEAD, "rust"),
    # F1. A queued line unsaid: T_CHECK_DEAD's stall assert reads CHECK_HELD, not "1 event(s) wait ...".
    Control("p47c_fix2_check_queue_unsaid", S, "    Some(Ok(if !run.queue.is_empty() {", "    Some(Ok(if false {",
            T_CHECK_DEAD, "rust"),
    # F1. A done stage still the driver's: T_CHECK_DEAD's last assert reads Err(resume), not "no order seated".
    Control("p47c_fix2_check_done_kept", S,
            ".filter(|state| state.started_at == opened.at && state.stage != Stage::Done)",
            ".filter(|state| state.started_at == opened.at)", T_CHECK_DEAD, "rust"),
    # F2. The end line appended again on resume: T_MATRIX's "Mixed Told(1)" row has end_lines 2, not 1.
    Control("p47c_fix2_tell_twice", S, "            if !said {\n", "            if !said || true {\n", T_MATRIX, "rust"),
    # F3. No count: T_ENDCUT's received line starts "scholar report: <path>", not "scholar report (5 of 5 lost)".
    Control("p47c_fix2_count_unsaid", S, "        format!(\" ({lost_count} of {} lost)\", state.orders.len())\n",
            "        String::new()\n", T_ENDCUT, "rust"),
    # F3. The status command back at the head: T_ENDCUT's received line does not hold the whole path.
    Control("p47c_fix2_status_first", S, "    format!(\"scholar report{count}: {why}{answer}{lost}{status}\")",
            "    format!(\"scholar report{count} ({}): {why}{answer}{lost}\", &status[2..])", T_ENDCUT, "rust"),
    # F3. The brief's wording: 41 + 79 characters, one past the cut: T_ENDCUT's "the path is cut" assert
    # ("...report.htm…").
    Control("p47c_fix2_count_words_long", S, "        format!(\" ({lost_count} of {} lost)\", state.orders.len())\n",
            "        format!(\" ({lost_count} of {} did not deliver)\", state.orders.len())\n", T_ENDCUT, "rust"),
    # F4. An amend on a scholar chain loud: T_REPLAY's scholar queue gains "s-r8 gated amend: ...".
    Control("p47c_fix2_amend_loud", RUN, "                    crate::nirvana_gate::Outcome::Amend.word(),\n", "",
            T_REPLAY, "rust"),
    # F4. A failed verdict on a scholar chain loud: T_REPLAY's scholar queue gains "s-r9 gated failed: ...".
    Control("p47c_fix2_failed_loud", RUN, "                    crate::nirvana_gate::Outcome::Failed.word(),\n", "",
            T_REPLAY, "rust"),
    # F4. The untrue sentence back in the seat skill: T_ROLE's "is said" assert.
    Control("p47c_fix2_skill_untrue", SKILL, "a doubt about one of their workers still does, as any order's would.",
            "a doubt about one of their workers still does, as any order's would. No researcher's or writer's line "
            "wakes you.", T_ROLE, "rust"),
    # F4. ROLE.md without what the driver handles: T_ROLE's "The driver handles its researchers' ..." not in ROLE.md.
    Control("p47c_fix2_role_handles_unsaid", ROLE,
            "(plan 47, the owner's scholar). The driver handles its researchers' and\n  writer's deliveries, verdicts, "
            "questions and interruptions; a stall or a deferral of one still wakes you. One wake\n",
            "(plan 47, the owner's scholar). One wake\n", T_ROLE, "rust"),
    # F1 in the role: ROLE.md without the dead end: T_ROLE's "or a dead end naming ..." not in ROLE.md.
    Control("p47c_fix2_role_resume_unsaid", ROLE,
            "  wants it. A wake that says `scholar gave_up`, or a dead end naming `herdr scholar resume <run>`, is "
            "answered with\n  that command.",
            "  wants it. A wake that says `scholar gave_up` is answered with `herdr scholar resume <run>`.", T_ROLE,
            "rust"),
    # F5. The BOM kept: T_LOGENC's BOM log reads (0, 1), not (1, 0).
    Control("p47c_fix2_log_bom_kept", S, "    let text = text.strip_prefix('\\u{feff}').unwrap_or(text.as_ref());\n",
            "    let text: &str = text.as_ref();\n", T_LOGENC, "rust"),
    # F5. A UTF-16 log read as empty: T_LOGENC's UTF-16 log reads (0, 0), not (0, 1).
    Control("p47c_fix2_log_utf16_empty", S,
            "    let bytes = std::fs::read(log_path(root)).unwrap_or_default();\n"
            "    let text = String::from_utf8_lossy(&bytes);\n",
            "    let text: std::borrow::Cow<str> = std::fs::read_to_string(log_path(root)).unwrap_or_default().into();\n",
            T_LOGENC, "rust"),
    # F5. NULs not trimmed: T_LOGENC's UTF-16 log reads (0, 2), its trailing NUL a second unreadable line.
    Control("p47c_fix2_log_nul_line", S,
            "        .map(|line| line.trim_matches(|c: char| c.is_whitespace() || c == '\\0'))\n",
            "        .map(str::trim)\n", T_LOGENC, "rust"),
    # F5. `tag` silent on the unread lines: T_LOGENC's tag refusal lacks "1 line(s) of ... could not be read".
    Control("p47c_fix2_tag_unread_unsaid", S,
            "            format!(\"; {bad} line(s) of {REPORTS}/{LOG_FILE} could not be read\")\n",
            "            String::new()\n", T_LOGENC, "rust"),
    # F6. CSS comments read: T_HTML3's first passing page finds "<style> url(: url(https://x.org/a.png) */ ...".
    Control("p47c_fix2_css_comments_read", S, "    let css = css_without_comments(css);\n",
            "    let css = css.to_owned();\n", T_HTML3, "rust"),
    # F6. A `/*` inside a CSS string read as a comment: T_HTML3's string page finds [] (the url swallowed).
    Control("p47c_fix2_css_string_comment", S, "            '\"' | '\\'' => {\n                out.push(c);",
            "            '\\u{0}' => {\n                out.push(c);", T_HTML3, "rust"),
    # F6. `srcdoc` unread: T_HTML3's first srcdoc page finds [], not "<iframe> srcdoc: <img> src: ...".
    Control("p47c_fix2_srcdoc_unread", S, "            if let Some(doc) = get(\"srcdoc\").filter(|_| name == \"iframe\") {",
            "            if let Some(doc) = get(\"srcdoc\").filter(|_| false) {", T_HTML3, "rust"),
    # F6. Static imports unread: T_HTML3's first module page finds [], not "<script> import: ...".
    Control("p47c_fix2_module_import_unread", S, "                        module_imports(body, &mut found);\n", "",
            T_HTML3, "rust"),
    # F6. Static imports read in a classic script: T_HTML3's classic page finds "<script> import: import x from ...".
    Control("p47c_fix2_module_in_classic", S,
            "                    if tag.attrs.iter().any(|(key, value)| key == \"type\" && value.contains(\"module\")) {",
            "                    if true {", T_HTML3, "rust"),
    # F6. Not UTF-8 unsaid: T_HTML3's Latin-1 report reads "report.html could not be read: stream did not contain
    # valid UTF-8".
    Control("p47c_fix2_html_not_utf8_unsaid", S,
            "            Err(err) if err.kind() == std::io::ErrorKind::InvalidData => {\n"
            "                problems.push(format!(\"{name} is not UTF-8\"))\n            }\n", "", T_HTML3, "rust"),
    # F6. Missing unsaid: T_HTML3's missing report.md reads "report.md could not be read: ...".
    Control("p47c_fix2_html_not_there_unsaid", S,
            "            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {\n"
            "                problems.push(format!(\"{name} is not there\"))\n            }\n", "", T_HTML3, "rust"),
    # F7. A bug that panics elsewhere: T_MATRIX's first row's payload is Some("a bug in the files check"), not None.
    Control("p47c_fix2_matrix_bug_panic", S,
            "            let problems = check_files(&project.root.join(&state.report_dir));\n",
            "            let problems: Vec<String> = panic!(\"a bug in the files check\");\n", T_MATRIX, "rust"),
    # F7. The stop point moved (no reseat): T_MATRIX's "Dies Sleep(3)" kills at write/sleep, not research/sleep.
    Control("p47c_fix2_matrix_no_reseat", S, "        ChainStatus::Interrupted(..) if chain.reseat.is_none() => {",
            "        ChainStatus::Interrupted(..) if false => {", T_MATRIX, "rust"),
    # The merge's own hunk. The Stop text without its first clause: the test's "Do what it names" is not found, so
    # its expect "the text says to do what the dead end names" panics.
    Control("p47c_merge_stop_names_first", "src/nirvana_claude.rs", "({}). Do what it names; else \\\n",
            "({}). \\\n",
            "nirvana_claude::tests::p47c_merge_the_stop_text_says_first_to_do_what_the_dead_end_names", "rust"),
)
