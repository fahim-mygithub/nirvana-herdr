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
    Control("c40d_lessons_uncapped", LESSONS,
            "    lines.truncate(CARDS_OFFERED);\n",
            "    lines.truncate(usize::MAX);\n", T_T2, "rust"),
    Control("c40d_lessons_unranked", LESSONS,
            "        (Some(a), Some(b)) => rank(a, b),\n",
            "        (Some(_), Some(_)) => std::cmp::Ordering::Equal,\n", T_T2, "rust"),
    Control("c40d_lessons_old_wording", LESSONS,
            "line touches what that step does, and leave the others closed.\";\n",
            "line touches what that step does, and leave the others closed. Open one only if it fits this order.\";\n",
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
            "line. Before a step, open each card",
            "line. Open one when it fits this order. Before a step, open each card", T_T5_CARDS, "rust"),
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
            "            .is_some_and(|at| SystemTime::from(at) < when)\n",
            "            .is_some_and(|at| SystemTime::from(at) < when && false)\n", T_D1_LOG, "rust"),
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
)
