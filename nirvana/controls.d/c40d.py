# Plan 40 P40d (2026-11-09): the memory area's bugs, capped injections, lessons that get read
# (nirvana/plans/2026-11-09-40-records/p40d-memory.md). Controls starting with c40d_. One sabotage per assertion, each
# on a product line; the test named must print FAIL with it in place (the expected red line, written before the run:
# nextest's "FAIL [ time] (1/1) herdr::bin/herdr <module>::tests::<test>"). `Control` is provided by controls.py.
JEV_COMPACT = "src/nirvana_jev_compact.rs"
IDLE = "src/nirvana_idle_compact.rs"
JEV = "src/nirvana_jev.rs"
TRIM = "src/nirvana_memory_trim.rs"
PRELOAD = "src/nirvana_preload.rs"
LESSONS = "src/nirvana_lessons.rs"
BOARD = "src/nirvana_board.rs"
JEV_MEMORY = "src/nirvana_jev_memory.rs"
KNOWLEDGE = "src/nirvana_knowledge.rs"
REVIEW = "src/nirvana_review.rs"
PROFILES = "src/nirvana_profiles.rs"

T_B1 = "a_compaction_after_the_last_turn_is_no_reading"
T_B1_SUMMARY = "a_compact_summary_alone_resets_the_size"
T_B2 = "a_hostile_retry_after_never_panics"
T_B3 = "restore_keeps_the_index_it_replaces_and_refuses_to_drop_a_pointer"
T_B4 = "the_jev_rows_never_measure_on_the_draw_path"
T_B5 = "two_writers_of_nirvana_md_lose_no_line"
T_B6 = "a_change_between_read_and_write_is_kept"
T_B7 = "every_placeholder_has_a_default"
T_T1 = "a_big_boards_first_notice_is_capped"
T_T2 = "a_brief_offers_at_most_fifteen_lessons_ranked_and_counts_only_when_seated"
T_T34 = "the_hook_prints_the_guide_only_with_a_key_and_never_on_resume"
T_T5_CARDS = "an_uncounted_card_offer_counts_only_when_seated"
T_T6 = "the_stop_hook_reads_a_small_tail_first"
T_RULE6 = "rule_six_keeps_a_measured_and_resolved_dead_end"
T_D1_LOG = "old_files_and_log_lines_are_pruned"
T_D1_BACKUPS = "old_backups_are_pruned_but_the_newest_five_stay"
T_M1 = "the_template_points_at_the_seat_table_for_a_model"
# Plan 40d review round (the record's "Review round" section).
APP_IDLE = "src/app/nirvana_idle_compact.rs"
T_SUM = "context_tokens_leave_out_output"
T_RULE4 = "rule4_reads_the_last_turn_past_the_away_recap"
T_WHY_FACTS = "why_names_every_fact_and_the_decision"
T_WHY_SERVER = "idle_compact_server_says_why_it_left_a_pane_alone"
T_N2 = "the_cap_keeps_three_places_for_new_lessons"
T_N4 = "a_damaged_head_is_pruned_past_and_a_late_append_is_kept"
T_N9 = "the_session_start_edit_waits_about_a_second_and_skips"
T_N12 = "the_jev_rows_repaint_only_when_their_facts_change"

CONTROLS = (
    # B1: one reading for idle compact and the Stop hook; a compaction newer than the last turn is no reading.
    Control("c40d_compaction_ignored", JEV_COMPACT,
            "            if value[\"type\"] == \"system\" && value[\"subtype\"] == \"compact_boundary\" {\n",
            "            if false {\n", T_B1, "rust"),
    Control("c40d_summary_line_ignored", JEV_COMPACT,
            "            if value[\"type\"] == \"user\" && value[\"isCompactSummary\"] == true {\n",
            "            if false {\n", T_B1_SUMMARY, "rust"),
    # B2: a Retry-After the server writes never panics, and a negative one is the plain retry.
    Control("c40d_retry_after_panics", JEV,
            "            .unwrap_or(Duration::MAX)\n",
            "            .unwrap_or_else(|_| Duration::from_secs_f64(secs))\n", T_B2, "rust"),
    Control("c40d_retry_after_negative_capped", JEV,
            "        Some(secs) if secs >= 0.0 => Duration::try_from_secs_f64(secs)\n",
            "        Some(secs) if !secs.is_nan() => Duration::try_from_secs_f64(secs)\n", T_B2, "rust"),
    # B3: restore refuses to drop a pointer, keeps the index it replaces, and never takes that copy as a backup.
    Control("c40d_restore_no_pointer_check", TRIM,
            "        if !lost.is_empty() && !force {\n",
            "        if false {\n", T_B3, "rust"),
    Control("c40d_restore_keeps_no_copy", TRIM,
            "        kept = Some(copy);\n",
            "        drop(copy);\n", T_B3, "rust"),
    Control("c40d_pre_restore_taken_as_backup", TRIM,
            "const PRE_RESTORE: &str = \".pre-restore\";\n",
            "const PRE_RESTORE: &str = \".pre-restore.md\";\n", T_B3, "rust"),
    # B4: the Jev rows' facts are read on their own thread, never on the draw call.
    Control("c40d_jev_rows_measure_on_draw", PRELOAD,
            "    if !background {\n        return Some(compute(cwd));\n",
            "    if !background || true {\n        return Some(compute(cwd));\n",
            T_B4, "rust"),
    # B5: every read-modify-write of NIRVANA.md holds the lock beside it.
    Control("c40d_memory_file_unlocked", LESSONS,
            "    let _lock = crate::nirvana_board::FileLock::take(&path)?;\n",
            "    let _lock = ();\n", T_B5, "rust"),
    # B6: the settings file is read again before the write; a change since the first read is applied afresh.
    Control("c40d_settings_no_recheck", PRELOAD,
            "        if now != before {\n",
            "        if now != before && false {\n", T_B6, "rust"),
    # B7: every size the shipped body names has a default.
    Control("c40d_placeholder_defaults", JEV_MEMORY,
            "    for (name, value) in split(DEFAULT).0 {\n",
            "    for (name, value) in split(\"\").0 {\n", T_B7, "rust"),
    # T1: a big board's first notice is capped.
    Control("c40d_first_notice_uncapped", BOARD,
            "            if size <= FIRST_NOTICE_WHOLE_CHARS {\n",
            "            if size <= usize::MAX {\n", T_T1, "rust"),
    # T2: at most CARDS_OFFERED lessons, ranked by the counters, and the offer line's new wording.
    # Plan 40d: the cap moved into keep_places_for_new; its early return is the cap's edge.
    Control("c40d_lessons_uncapped", LESSONS,
            "    if ranked.len() <= CARDS_OFFERED {\n        return ranked;\n",
            "    if true {\n        return ranked;\n", T_T2, "rust"),
    Control("c40d_lessons_unranked", LESSONS,
            "        (Some(a), Some(b)) => rank(a, b),\n",
            "        (Some(_), Some(_)) => std::cmp::Ordering::Equal,\n", T_T2, "rust"),
    Control("c40d_lessons_old_wording", LESSONS,
            "step you are doing, and leave the others closed.\";\n",
            "step you are doing, and leave the others closed. Open one only if it fits this order.\";\n",
            T_T2, "rust"),
    # T5: an offer counts nothing until the seat is written (`count_offered`).
    Control("c40d_lessons_counted_early", LESSONS,
            "    Offer { section, slugs }\n}\n",
            "    let offer = Offer { section, slugs };\n    drop(_lock);\n    count_offered(root, &offer);\n    offer\n}\n",
            T_T2, "rust"),
    Control("c40d_cards_counted_early", KNOWLEDGE,
            "    nirvana_lessons::Offer { section, slugs }\n}\n",
            "    let offer = nirvana_lessons::Offer { section, slugs };\n    drop(_lock);\n"
            "    count_offered(root, profile, &offer);\n    offer\n}\n",
            T_T5_CARDS, "rust"),
    Control("c40d_cards_old_wording", KNOWLEDGE,
            "line. Open a card when its line names",
            "line. Open one when it fits this order. Open a card when its line names", T_T5_CARDS, "rust"),
    # T3 and T4: no Jev key, no guide; a resume does not print it again.
    Control("c40d_guide_without_key", JEV_MEMORY,
            "    if !key_present || source == Some(\"resume\") {\n",
            "    if source == Some(\"resume\") {\n", T_T34, "rust"),
    Control("c40d_guide_on_resume", JEV_MEMORY,
            "    if !key_present || source == Some(\"resume\") {\n",
            "    if !key_present {\n", T_T34, "rust"),
    # T6: the Stop hook reads one chunk first, and widens at a line.
    Control("c40d_stop_hook_reads_4mib", JEV_COMPACT,
            "    let small = tail_lines(transcript, CHUNK);\n",
            "    let small = tail_lines(transcript, LIMIT);\n", T_T6, "rust"),
    Control("c40d_stop_hook_never_widens", JEV_COMPACT,
            "            let full = tail_lines(transcript, LIMIT);\n",
            "            let full = tail_lines(transcript, CHUNK);\n", T_T6, "rust"),
    # Rule 6's exception is written into the rule.
    Control("c40d_rule6_no_exception", REVIEW,
            "One exception: a dead end the stream shows the worker \\\n",
            "A dead end the stream shows the worker \\\n", T_RULE6, "rust"),
    # D1: old log lines go; the newest five backups of a folder stay whatever their age.
    Control("c40d_prune_log_keeps_old", BOARD,
            "    let old = |line: &str| line_time(line).is_some_and(|at| at < cutoff);\n",
            "    let old = |line: &str| line_time(line).is_some_and(|at| at < cutoff && false);\n",
            T_D1_LOG, "rust"),
    Control("c40d_backups_keep_none", TRIM,
            "pub const KEEP_BACKUPS: usize = 5;\n",
            "pub const KEEP_BACKUPS: usize = 0;\n", T_D1_BACKUPS, "rust"),
    Control("c40d_prune_ignores_keep", BOARD,
            "    let candidates = oldest_first.len().saturating_sub(keep_newest);\n",
            "    let candidates = oldest_first.len().saturating_sub(keep_newest * 0);\n", T_D1_BACKUPS, "rust"),
    # M1: the template's model line names no model id.
    Control("c40d_template_names_a_model", PROFILES,
            "# model: a model id that herdr seat list shows\\n\\\n",
            "# model: claude-sonnet-5-5\\n\\\n", T_M1, "rust"),
    # S1: the shared reading sums input, cache read and cache write (the test's 60,510 is no single one of them).
    Control("c40d_context_not_summed", JEV_COMPACT,
            "        .sum::<u64>();\n        if total > 0 {\n",
            "        .max()\n        .unwrap_or(0);\n        if total > 0 {\n", T_SUM, "rust"),
    # Part 1: rule 4 reads the last turn past Claude Code's idle recap; a torn line falls back to the file's age.
    Control("c40d_rule4_file_time_only", IDLE,
            "    Some(by_turn.map_or(by_file, |turn| turn.max(by_file)))\n",
            "    Some(by_file)\n", T_RULE4, "rust"),
    Control("c40d_rule4_recap_is_activity", IDLE,
            "    \"away_summary\",\n",
            "    \"away_summary_never\",\n", T_RULE4, "rust"),
    Control("c40d_rule4_torn_line_skipped", IDLE,
            "        .find(|line| !idle_line(line))?;\n",
            "        .find(|line| !idle_line(line) && serde_json::from_str::<serde_json::Value>(line).is_ok())?;\n",
            T_RULE4, "rust"),
    # Part 1: why. The owner's text is counted, never shown; every check writes the snapshot; a reason is noted once.
    Control("c40d_why_shows_typed_text", IDLE,
            "                InputLine::Text(text) => format!(\"holds {} characters\", text.chars().count()),\n",
            "                InputLine::Text(text) => format!(\"holds {text}\"),\n", T_WHY_FACTS, "rust"),
    Control("c40d_why_not_written", APP_IDLE,
            "        self.idle_compact_why(now, true);\n        typed\n",
            "        typed\n", T_WHY_SERVER, "rust"),
    Control("c40d_skip_reason_not_noted", APP_IDLE,
            "                        if new {\n                            terminal.idle_stretch.logged = Some(key);\n",
            "                        if false {\n                            terminal.idle_stretch.logged = Some(key);\n",
            T_WHY_SERVER, "rust"),
    # N2: three of the fifteen places go to the newest lessons never opened.
    Control("c40d_new_lessons_no_places", LESSONS,
            "pub const NEW_LESSON_PLACES: usize = 3;\n",
            "pub const NEW_LESSON_PLACES: usize = 0;\n", T_N2, "rust"),
    # N4: the prune reads past a damaged head, and starts again when the log grew under it.
    Control("c40d_prune_head_first_line_only", BOARD,
            "            .take(PRUNE_PROBE_LINES)\n",
            "            .take(1)\n", T_N4, "rust"),
    Control("c40d_prune_late_append_lost", BOARD,
            "        if grown {\n",
            "        if false {\n", T_N4, "rust"),
    # N9: the session-start edit waits about a second, and an unchanged line takes no lock.
    Control("c40d_session_lock_waits_long", LESSONS,
            "    edit_memory_file_within(home, SESSION_LOCK_WAIT, edit)\n",
            "    edit_memory_file_within(home, std::time::Duration::from_secs(3), edit)\n", T_N9, "rust"),
    Control("c40d_session_lock_always_taken", LESSONS,
            "    if edit(std::fs::read_to_string(&path).ok().as_deref()).is_none() {\n",
            "    if false {\n", T_N9, "rust"),
    # N12: equal facts ask for no repaint.
    Control("c40d_preload_repaints_always", PRELOAD,
            "    if facts.is_none() || facts == slot.facts {\n",
            "    if facts.is_none() {\n", T_N12, "rust"),
    # N5: the oldest backup of each kind stays.
    Control("c40d_backups_oldest_deleted", TRIM,
            "            kind.remove(0);\n",
            "            kind.retain(|_| true);\n", T_D1_BACKUPS, "rust"),
    # N11: rule 6's exception never covers this machine's setup.
    Control("c40d_rule6_scope_missing", REVIEW,
            "worker does not repeat it. The exception never covers a missing program, a sign-in or a path on this machine.",
            "worker does not repeat it.", T_RULE6, "rust"),
    # T5, the worker half (applied at the plan 40 merge): cards are counted as offered only once the seat is written.
    Control("c40d_worker_counts_no_cards", "src/nirvana_worker.rs",
            "        crate::nirvana_knowledge::count_offered(&project.root, name, &cards);\n",
            "        let _ = (name, &cards);\n",
            "p34_an_order_seated_with_a_folder_profile_gets_one_line_per_live_card_most_helpful_first_at_most_15",
            "rust"),
)
