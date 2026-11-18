# Plan 43 A1 to A3 and A7's hand merge (the gate half): a refusal no worker can fix goes to the orchestrator as a
# `herdr gate amend` line and seats no child; a new file no live order owns is a note; a repo-relative source read in
# the worktree counts; a hand merge is journaled with its sha. Megajam run ia1 (2026-11-12 to 18): 46 of 91 refusals
# could not be fixed by a worker. Controls starting with p84_. One sabotage per assertion, each on a product line of
# src/nirvana_gate.rs; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GATE = "src/nirvana_gate.rs"
_T = "nirvana_gate::tests::"

CONTROLS = (
    # --- A2: new files, holders, case ---
    # A new file no live order owns is a note, not a fail
    Control("p84_new_file_is_a_note", GATE,
            "            None if is_new(path) => fresh.push(path.clone()),\n",
            "            None if false && is_new(path) => fresh.push(path.clone()),\n",
            _T + "p43_a_new_file_no_live_order_owns_is_a_note_and_an_old_one_the_briefs_fail", "rust"),
    # An old file outside owns with no holder is the brief's fault, with the --owns that covers it
    Control("p84_narrow_owns_is_the_briefs", GATE,
            "        .fault(if held.is_empty() {\n            Fault::Brief\n",
            "        .fault(if held.is_empty() {\n            Fault::Worker\n",
            _T + "p43_a_new_file_no_live_order_owns_is_a_note_and_an_old_one_the_briefs_fail", "rust"),
    # A file another live order owns stays a fail, the worker's
    Control("p84_holder_collides", GATE,
            ".find(|holder| holder.owns.iter().any(|pattern| owns_match(pattern, path)))\n",
            ".find(|holder| holder.owns.iter().any(|pattern| owns_match(pattern, path)) && false)\n",
            _T + "p43_a_file_another_live_order_owns_stays_the_workers_fail", "rust"),
    # owns: ignores case on Windows (h3-hand-seq)
    Control("p84_owns_fold_case", GATE,
            "pub const OWNS_FOLD_CASE: bool = cfg!(windows);\n",
            "pub const OWNS_FOLD_CASE: bool = false;\n",
            _T + "p43_owns_ignores_case_where_the_file_system_does", "rust"),
    # The run's live orders are read from the journal as holders
    Control("p84_live_orders_hold", GATE,
            "            Phase::Seated | Phase::Running | Phase::Deferred | Phase::Delivered | Phase::Asked => {\n"
            "                true\n",
            "            Phase::Seated | Phase::Running | Phase::Deferred | Phase::Delivered | Phase::Asked => {\n"
            "                false\n",
            _T + "p43_the_gate_reads_new_files_from_the_base_and_holders_from_the_journal", "rust"),
    # New is read against the base the axes ran on
    Control("p84_new_against_the_base", GATE,
            "            let is_new = |path: &str| known.as_ref().is_ok_and(|known| !known.contains(path));\n",
            "            let is_new = |path: &str| known.as_ref().is_ok_and(|known| known.contains(path));\n",
            _T + "p43_the_gate_reads_new_files_from_the_base_and_holders_from_the_journal", "rust"),

    # --- A1: the route, the amend, the faults ---
    # No worker fault: no child, the amend line
    Control("p84_no_child_for_a_brief_fault", GATE,
            "    if !blocked && route == Route::ByFault && no_worker_fault(&axes) {\n",
            "    if false && !blocked && route == Route::ByFault && no_worker_fault(&axes) {\n",
            _T + "p43_a_refusal_no_worker_can_fix_seats_no_child_and_names_the_amend_line", "rust"),
    # The reason opens with the exact amend line (the wake carries it)
    Control("p84_reason_opens_with_the_line", GATE,
            "    let reason = format!(\"{line}; {}\", reason_line(&axes).unwrap_or_default());\n",
            "    let reason = format!(\"amend; {}\", reason_line(&axes).unwrap_or_default());\n",
            _T + "p43_a_refusal_no_worker_can_fix_seats_no_child_and_names_the_amend_line", "rust"),
    # A gate by hand of an amend gives its line again
    Control("p84_stored_amend", GATE,
            "                    || verdict == Outcome::Amend.word()\n            ) =>\n",
            "                    || verdict == \"never\"\n            ) =>\n",
            _T + "p43_a_refusal_no_worker_can_fix_seats_no_child_and_names_the_amend_line", "rust"),
    # One worker fault keeps the child
    Control("p84_mixed_keeps_the_child", GATE,
            "    !failing.is_empty() && failing.iter().all(|axis| fault_of(axis) != Fault::Worker)\n",
            "    !failing.is_empty() && failing.iter().any(|axis| fault_of(axis) != Fault::Worker)\n",
            _T + "p43_a_refusal_with_a_worker_fault_still_seats_the_child", "rust"),
    # gate amend's owns reach the re-gate
    Control("p84_amend_reaches_the_regate", GATE,
            "    let spec = amend_spec(spec(&root_text), &read_sidecar(project, run, &root).amends);\n",
            "    let spec = spec(&root_text);\n",
            _T + "p43_gate_amend_adds_owns_and_regates_the_same_branch_with_no_child", "rust"),
    # gate amend journals the old value
    Control("p84_amend_says_the_old_value", GATE,
            "            \"owns + {} (was {})\",\n",
            "            \"owns + {} ({})\",\n",
            _T + "p43_gate_amend_adds_owns_and_regates_the_same_branch_with_no_child", "rust"),
    # A test command whose python -c code does not parse is the brief's
    Control("p84_python_c_unparsed", GATE,
            "    if plain.contains(\"File \\\"<string>\\\"\") {\n",
            "    if plain.contains(\"File \\\"<nothing>\\\"\") {\n",
            _T + "p43_a_test_command_that_does_not_parse_is_the_briefs", "rust"),
    # A pwsh -Command that does not parse is the brief's
    Control("p84_pwsh_unparsed", GATE,
            "            if let Some(rest) = line.strip_prefix(\"ParserError:\") {\n",
            "            if let Some(rest) = line.strip_prefix(\"ParserErr0r:\") {\n",
            _T + "p43_a_test_command_that_does_not_parse_is_the_briefs", "rust"),
    # A worker's own broken .ps1 stays the worker's
    Control("p84_script_stays_the_workers", GATE,
            "                if script(rest) {\n                    continue;\n                }\n",
            "                if script(rest) && false {\n                    continue;\n                }\n",
            _T + "p43_a_test_command_that_does_not_parse_is_the_briefs", "rust"),
    # herdr's own files are set aside for the rebase
    Control("p84_harness_set_aside", GATE,
            "            let moved = match set_aside_harness(subject.cwd, &integration, &aside) {\n",
            "            let moved = match Ok::<Vec<String>, String>(Vec::new()) {\n",
            _T + "p43_herdrs_own_files_never_stop_the_rebase", "rust"),
    # A rebase stopped by herdr's files alone is the gate's fault
    Control("p84_rebase_fault_harness", GATE,
            "        return if listed.iter().all(|path| harness_path(path)) {\n",
            "        return if listed.iter().all(|path| !harness_path(path)) {\n",
            _T + "p43_a_rebase_stopped_by_herdrs_files_alone_is_the_gates", "rust"),

    # --- A3: local sources ---
    # A repo-relative cite resolves against the order's folders
    Control("p84_relative_resolves", GATE,
            "        .map(|root| clean_path(&format!(\"{}/{path}\", root.display())))\n",
            "        .map(|root| clean_path(&format!(\"{}/x/{path}\", root.display())))\n",
            _T + "p43_a_repo_relative_source_read_in_the_worktree_counts", "rust"),
    # A shell command that counts lines (wc) reads the file
    Control("p84_shell_counter_reads", GATE,
            "        .any(|segment| segment_reads_local(segment) || runs(segment, LOCAL_COUNTERS))\n",
            "        .any(|segment| segment_reads_local(segment) && runs(segment, LOCAL_COUNTERS))\n",
            _T + "p43_a_repo_relative_source_read_in_the_worktree_counts", "rust"),
    # A heredoc body is no read
    Control("p84_heredoc_is_no_read", GATE,
            "    let text = without_heredocs(cmd);\n",
            "    let text = cmd.to_owned();\n",
            _T + "p43_a_heredoc_body_is_no_read_and_a_shell_read_is", "rust"),
    # A search over the whole worktree vouches for no file
    Control("p84_root_search_vouches_none", GATE,
            "                        && !tops.contains(place)\n",
            "                        && !tops.contains(&String::new())\n",
            _T + "p43_agy_shell_reads_and_a_search_folder_count", "rust"),
    # A file in the last cited one's folder
    Control("p84_piece_in_last_folder", GATE,
            "            Some(dir) if !piece.contains(['/', '\\\\']) => format!(\"{dir}/{piece}\"),\n",
            "            Some(dir) if !piece.contains(['/', '\\\\']) => piece,\n",
            _T + "p43_agy_shell_reads_and_a_search_folder_count", "rust"),
    # research.read's local entries are not checked
    Control("p84_research_read_skips_local", GATE,
            "            if class == READ_CLASS {\n",
            "            if class == \"research.none\" {\n",
            _T + "p43_research_read_skips_local_entries_and_an_unfetched_url_still_fails", "rust"),
    # A media order that judged local files and cited none is a note
    Control("p84_media_local_note", GATE,
            "        Some(asked) if class.starts_with(\"media.\") && opened.is_empty() && judged_local => Axis::new(\n",
            "        Some(asked) if false && class.starts_with(\"media.\") && opened.is_empty() && judged_local => Axis::new(\n",
            _T + "p43_a_media_order_that_judged_local_files_and_cited_none_is_a_note", "rust"),

    # --- A7: the hand merge ---
    # The commit that landed it
    Control("p84_by_hand_landing_sha", GATE,
            "            landing_commit(&project.root, &head, &tip)\n",
            "            head.clone()\n",
            _T + "p43_a_hand_merge_is_journaled_with_its_sha_and_reason", "rust"),
    # The orchestrator's reason
    Control("p84_by_hand_reason", GATE,
            "                        reason: Some(format!(\"by hand: {reason}\")),\n",
            "                        reason: None,\n",
            _T + "p43_a_hand_merge_is_journaled_with_its_sha_and_reason", "rust"),
)
