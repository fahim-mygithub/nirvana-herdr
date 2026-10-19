# Plan 13 P3: the session record and the memory trim (nirvana/plans/2026-10-18-13-records/p13c.md). One sabotage per
# assertion; each test named must print FAIL with its sabotage in place. `Control` is provided by controls.py.
REC = "src/nirvana_session_record.rs"
TRIM = "src/nirvana_memory_trim.rs"
HOOKS = "nirvana/claude/hooks/hooks.json"

FORMAT = "the_record_is_the_fixed_format_under_the_folders_hash"
SUBFOLDER = "a_session_that_changed_directory_is_still_filed_under_its_start_folder"
NOTHING = "a_hook_that_describes_no_session_writes_nothing_and_does_not_fail"
OKAY = "the_command_exits_ok_on_garbage_a_missing_home_and_an_unwritable_home"
PLUGIN = "the_plugin_runs_record_session_from_session_start_and_stop_in_the_same_exec_form_as_the_others"

ENCODING = "the_folder_encoding_matches_the_names_claude_code_wrote"
REFUSE = "the_pointer_check_refuses_a_proposal_that_drops_a_pointer_and_writes_nothing"
SMALL = "a_small_index_has_nothing_to_trim"
ACCEPT = "accept_keeps_a_byte_identical_backup_then_replaces_the_index_and_restore_puts_it_back"
STALE = "accept_checks_again_against_the_index_as_it_is_now"
DISCARD = "discard_deletes_the_proposal_and_only_the_proposal"
TWICE = "a_second_accept_makes_its_own_backup_and_restore_takes_the_newest"
LADDER = "when_the_ladder_is_not_enough_needed_hooks_are_dropped_one_at_a_time_oldest_first"
POINTER_HOOK = "a_hook_that_holds_a_pointer_is_never_cut_and_unknown_lines_pass_through"
JEV = "jev_judges_each_entry_and_the_result_is_assembled_by_code"
JEV_FAIL = "a_failed_jev_call_makes_no_proposal_and_is_logged_as_failed"
REDACT = "a_secret_in_a_hook_is_redacted_before_it_is_sent"

NO_CLAUDE_READ = "no_test_of_this_package_reads_the_owners_claude_folder"
PAREN_TARGET = "a_link_target_with_parentheses_parses_whole_and_survives_a_cut"
COUNTED = "pointers_are_counted_so_dropping_one_of_two_links_to_a_file_is_refused"
OTHER_FORMS = "a_hook_naming_a_file_in_another_form_is_never_cut_and_its_pointer_is_counted"
REL_CLI = "a_relative_folder_is_made_absolute_before_it_is_encoded"
REL_RECORD = "a_relative_folder_is_filed_under_its_absolute_hash"
VERBATIM = "a_verbatim_folder_and_its_plain_spelling_are_one_record"
TEMP_NAMES = "temp_names_are_unique_and_append_to_the_whole_file_name"
ATOMIC_NEIGHBOUR = "write_atomic_leaves_a_neighbour_the_old_temp_name_would_have_taken"
RECORD_NEIGHBOUR = "a_record_write_leaves_a_neighbour_the_old_temp_name_would_have_taken"

CONTROLS = (
    # The record: where, what, when it is not written.
    Control("p13c_record_wrong_key", REC,
            "        crate::nirvana_launch::folder_hash(folder)",
            '        crate::nirvana_launch::folder_hash(Path::new("x"))', FORMAT, "rust"),
    Control("p13c_cwd_beats_start_dir", REC,
            "        .filter(|dir| !dir.is_empty())\n",
            "        .filter(|_| false)\n", SUBFOLDER, "rust"),
    Control("p13c_subagent_recorded", REC,
            'if hook.get("agent_id").is_some_and(|a| !a.is_null()) {',
            "if false {", NOTHING, "rust"),
    Control("p13c_odd_session_id_written", REC,
            "        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')",
            "        .all(|_| true)", NOTHING, "rust"),
    Control("p13c_command_fails_the_hook", REC,
            "        let _ = handle(home, &hook, project_dir, &time_now());",
            '        handle(home, &hook, project_dir, &time_now()).map_err(std::io::Error::other)?;', OKAY, "rust"),
    # The hooks.
    Control("p13c_session_start_hook_misspelled", HOOKS,
            '              "memory",\n              "record-session"\n            ],\n            "timeout": 10\n          }\n        ]\n      }\n    ],\n    "UserPromptSubmit"',
            '              "memory",\n              "record-sesion"\n            ],\n            "timeout": 10\n          }\n        ]\n      }\n    ],\n    "UserPromptSubmit"',
            PLUGIN, "rust"),
    Control("p13c_stop_hook_misspelled", HOOKS,
            '              "memory",\n              "record-session"\n            ],\n            "timeout": 10\n          }\n        ]\n      }\n    ]\n  }\n}',
            '              "memory",\n              "record-sesion"\n            ],\n            "timeout": 10\n          }\n        ]\n      }\n    ]\n  }\n}',
            PLUGIN, "rust"),
    # Where the index is.
    Control("p13c_dot_kept_in_the_folder_name", TRIM,
            "        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })",
            "        .map(|c| if c.is_ascii_alphanumeric() || c == '.' { c } else { '-' })", ENCODING, "rust"),
    Control("p13c_trailing_separator_kept", TRIM,
            "    let trimmed = text.trim_end_matches(['\\\\', '/']);",
            "    let trimmed = text.as_str();", ENCODING, "rust"),
    # The pointer check and its neighbours.
    Control("p13c_dropped_pointer_passes", TRIM,
            "        if have < had {",
            "        if false {", REFUSE, "rust"),
    Control("p13c_larger_proposal_passes", TRIM,
            "    if tokens_of(proposed) >= tokens_of(original) {",
            "    if false {", REFUSE, "rust"),
    Control("p13c_empty_proposal_passes", TRIM,
            "    if proposed.trim().is_empty() {",
            "    if false {", REFUSE, "rust"),
    Control("p13c_small_index_still_trimmed", TRIM,
            "    if tokens_of(&original) <= ctx.limit {",
            "    if false {", SMALL, "rust"),
    # Accept, discard, restore.
    Control("p13c_backup_is_not_the_original", TRIM,
            "    std::fs::write(&backup, original.as_bytes())",
            "    std::fs::write(&backup, original.trim_end().as_bytes())", ACCEPT, "rust"),
    Control("p13c_accept_skips_the_second_check", TRIM,
            "    // The original may have gained an entry since the proposal: the check runs again against what is there now.\n    check(&original, &proposed)?;",
            "    // The original may have gained an entry since the proposal: the check runs again against what is there now.\n    let _ = &original;", STALE, "rust"),
    Control("p13c_discard_deletes_the_index", TRIM,
            "    match std::fs::remove_file(proposal_path(&index)) {",
            "    match std::fs::remove_file(&index) {", DISCARD, "rust"),
    Control("p13c_restore_takes_the_oldest", TRIM,
            "    found.sort();",
            "    found.sort_by(|a, b| b.cmp(a));", TWICE, "rust"),
    Control("p13c_backup_counter_unsorted", TRIM,
            '        .map(|n| dir.join(format!("{stem}-{n:02}.md")))',
            '        .map(|n| if n == 0 { dir.join(format!("{stem}.md")) } else { dir.join(format!("{stem}-{n}.md")) })',
            TWICE, "rust"),
    # The ladder.
    Control("p13c_newest_dropped_first", TRIM,
            "    let oldest_first: Vec<usize> = (0..entries).collect();",
            "    let oldest_first: Vec<usize> = (0..entries).rev().collect();", LADDER, "rust"),
    Control("p13c_final_drop_all_at_once", TRIM,
            "            cuts[i] = Cut::Drop;",
            "            cuts.fill(Cut::Drop);", LADDER, "rust"),
    Control("p13c_pointer_hook_cut", TRIM,
            "            .all(|line| pointers(line).is_empty())",
            "            .all(|_| true)", POINTER_HOOK, "rust"),
    # Jev.
    Control("p13c_unsure_answer_cuts", TRIM,
            '                            answer["confidence"].as_f64().unwrap_or(0.0) >= CHOICE_FLOOR;',
            "                            true;", JEV, "rust"),
    Control("p13c_failed_call_goes_on", TRIM,
            '                    return Err(format!("Jev failed: {}", error.why()));',
            "                    continue;", JEV_FAIL, "rust"),
    Control("p13c_hook_sent_unredacted", TRIM,
            '                    "hook": redact(&hook, self.key.as_deref()).0,',
            '                    "hook": hook,', REDACT, "rust"),
    # Fix round. 1: no test reads the owner's ~/.claude. The sabotage puts a read of it back into the encoding test.
    Control("p13c_live_claude_read_returns", "src/nirvana_memory_trim/tests.rs",
            '        "a trailing separator is not part of the name"\n    );\n',
            '        "a trailing separator is not part of the name"\n    );\n    let _ = PathBuf::from(".").join(".claude");\n',
            NO_CLAUDE_READ, "rust"),
    # 2: the pointer check. Balanced parentheses, counts, and the other forms of a file pointer.
    Control("p13c_target_ends_at_the_first_paren", TRIM,
            "            '(' => depth += 1,",
            "            '(' => {}", PAREN_TARGET, "rust"),
    Control("p13c_pointers_compared_as_a_set", TRIM,
            "        if have < had {",
            "        if have == 0 && had > 0 {", COUNTED, "rust"),
    Control("p13c_other_pointer_forms_are_cuttable", TRIM,
            "            .all(|line| pointers(line).is_empty())",
            '            .all(|line| !line.contains("](") && !line.contains("[["))', OTHER_FORMS, "rust"),
    Control("p13c_bare_md_token_not_counted", TRIM,
            "            add(out, token.to_owned());",
            "            let _ = token;", OTHER_FORMS, "rust"),
    Control("p13c_backticked_path_not_counted", TRIM,
            '            add(out, format!("`{body}`"));',
            "            let _ = &body;", OTHER_FORMS, "rust"),
    Control("p13c_autolink_not_counted", TRIM,
            '            add(out, format!("<{body}>"));',
            "            let _ = body;", OTHER_FORMS, "rust"),
    Control("p13c_reference_link_not_counted", TRIM,
            '        add(out, format!("[{}]", reference.trim().to_lowercase()));',
            "        let _ = reference;", OTHER_FORMS, "rust"),
    Control("p13c_reference_definition_not_counted", TRIM,
            '    add(out, format!("[{}]: {dest}", label.trim().to_lowercase()));',
            "    let _ = (label, dest);", OTHER_FORMS, "rust"),
    # 3: relative folders and the verbatim prefix.
    Control("p13c_cli_folder_left_relative", TRIM,
            "    Ok(crate::nirvana_session_record::absolute_folder(&folder))",
            "    Ok(folder)", REL_CLI, "rust"),
    Control("p13c_record_folder_left_relative", REC,
            "        .map(|dir| absolute_folder(Path::new(dir)))",
            "        .map(PathBuf::from)", REL_RECORD, "rust"),
    Control("p13c_verbatim_prefix_kept", REC,
            "    match absolute.to_str().and_then(strip_verbatim) {",
            "    match None::<String> {", VERBATIM, "rust"),
    # 4: temporary names.
    Control("p13c_temp_name_without_the_counter", REC,
            "        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)",
            "        0u64", TEMP_NAMES, "rust"),
    Control("p13c_temp_name_by_with_extension", REC,
            "    path.with_file_name(name)",
            '    path.with_extension(format!("tmp.{}", std::process::id()))', TEMP_NAMES, "rust"),
    Control("p13c_atomic_write_uses_with_extension", TRIM,
            "    let tmp = crate::nirvana_session_record::temp_sibling(path);",
            '    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));', ATOMIC_NEIGHBOUR, "rust"),
    Control("p13c_record_write_uses_with_extension", REC,
            "    let tmp = temp_sibling(&path);",
            '    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));', RECORD_NEIGHBOUR, "rust"),
)
