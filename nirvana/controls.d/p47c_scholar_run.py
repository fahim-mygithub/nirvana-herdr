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
T_REPLAY = _T + "p47c_replay_queues_nothing_for_a_scholar_researcher_and_every_line_of_the_writer"
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

LINK_ARM = '            "link" | "img" | "iframe"'
NO_DELIVERY = '    Step::Finish(ends::NO_DELIVERY, format!("no researcher passed: {why}"))'

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
    Control("p47c_writer_never_seated", S, "    {\n        return Step::SeatWriter;\n    }\n    let why = state",
            "    {\n        return Step::Finish(ends::NO_DELIVERY, String::new());\n    }\n    let why = state",
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
    Control("p47c_never_tells", S, "        if tells(end) && !state.told {", "        if false && tells(end) && !state.told {",
            T_RESUME, "rust"),
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
    Control("p47c_ended_run_not_done", S, "        if run.ended.is_some() || state.end_tries >= END_TRIES {",
            "        if state.end_tries >= END_TRIES {", T_ENDWAIT, "rust"),
    # ---- the replay hunk ----
    # The hunk off: T_REPLAY's first queue assert holds the researchers' lines.
    Control("p47c_replay_hunk_off", RUN, "        let owed = owed && !scholar_quiet;\n",
            "        let owed = owed && !(scholar_quiet && false);\n", T_REPLAY, "rust"),
    # The child's own why read, not the root's: T_REPLAY's first queue holds q-r2-s1's lines.
    Control("p47c_replay_child_not_followed", RUN,
            "                        .order(&crate::nirvana_gate::chain(&state, known.id()).0)\n",
            "                        .order(known.id())\n", T_REPLAY, "rust"),
    # A send-back verdict queued: T_REPLAY's first queue holds "q-r2 gated sent_back".
    Control("p47c_replay_send_back_verdict_queued", RUN,
            "                    || verdict == crate::nirvana_gate::Outcome::SentBack.word()\n", "", T_REPLAY, "rust"),
    # The writer made quiet: T_REPLAY's starts lack the two q-write lines.
    Control("p47c_replay_writer_quiet", RUN,
            "                known.seated.class == crate::nirvana_scholar::RESEARCH_CLASS\n",
            "                known.seated.class != \"never\"\n", T_REPLAY, "rust"),
    # A question made quiet: T_REPLAY's starts lack the q-r3 and q-r4 lines.
    Control("p47c_replay_asks_quiet", RUN, "            _ => false,\n        } && event",
            "            _ => true,\n        } && event", T_REPLAY, "rust"),
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
    # The wake's summary cut shorter: T_WAKEPATH's queue line lacks the 77-character path.
    Control("p47c_queue_line_too_short", RUN, '            format!("{order} delivered {status}: {}", short(summary, 90))',
            '            format!("{order} delivered {status}: {}", short(summary, 60))', T_WAKEPATH, "rust"),
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
    # A script src passes: T_HTML's first bad page finds nothing.
    Control("p47c_html_script_src_ok", S,
            '            "script" if !attr("src").is_empty() => found.push("a <script> with a src".to_owned()),',
            '            "script" if false => found.push("a <script> with a src".to_owned()),', T_HTML, "rust"),
    # A stylesheet from a host passes: T_HTML's <link> page finds nothing.
    Control("p47c_html_link_ok", S, LINK_ARM, '            "img" | "iframe"', T_HTML, "rust"),
    # An @import passes: T_HTML's @import page finds nothing.
    Control("p47c_html_import_ok", S, '    if lower.contains("@import") {', '    if false && lower.contains("@import") {',
            T_HTML, "rust"),
    # A url() from a host passes: T_HTML's url() pages find nothing.
    Control("p47c_html_url_ok", S, "        if outside(&lower[after..]) {", "        if false && outside(&lower[after..]) {",
            T_HTML, "rust"),
    # A link the reader clicks fails: T_HTML's good page finds "a <a> href on another host".
    Control("p47c_html_anchor_fails", S, LINK_ARM, '            "a" | "link" | "img" | "iframe"', T_HTML, "rust"),
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
            '        .filter(|order| order.seated.why.as_deref() == Some("send_back"))',
            '        .filter(|order| order.seated.why.as_deref() == Some("reseat"))', T_LOG, "rust"),
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
            "            append_line(&log_path(&project.root), &log_line(state, run, now))?;\n", "", T_E2E, "rust"),
    # The run never ended: T_E2E's `ended.get()` is false.
    Control("p47c_run_not_ended", S, "            match (world.end_run)(project, &state.run) {",
            "            match Ok::<(), String>(()) {", T_E2E, "rust"),
    # ---- nothing to research ----
    # The empty answer researched anyway: T_NOTHING's end is no_delivery, not nothing_to_research.
    Control("p47c_nothing_researched_anyway", S, "            let nothing = state.orders.is_empty();",
            "            let nothing = false;", T_NOTHING, "rust"),
    # The tell line without the status command: T_NOTHING's asked line does not start "scholar nothing_to_research:".
    Control("p47c_tell_line_plain", S, '        "scholar {}: `herdr scholar status {}`; {}",',
            '        "scholar {} {}; {}",', T_NOTHING, "rust"),
    # Told but not woken: T_NOTHING's wakes is 0, not 1.
    Control("p47c_tell_no_wake", S, "            (world.wake)(project, &state.run);\n", "", T_NOTHING, "rust"),
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
)
