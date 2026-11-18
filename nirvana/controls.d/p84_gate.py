# Plan 43 A1 to A3 and A7's hand merge (the gate half): a refusal no worker can fix goes to the orchestrator as a
# `herdr gate amend` line and seats no child; a new file no live order owns is a note; a repo-relative source read in
# the worktree counts; a hand merge is journaled with its sha. Megajam run ia1 (2026-11-12 to 18): 46 of 91 refusals
# could not be fixed by a worker. Controls starting with p84_. One sabotage per assertion, each on a product line of
# src/nirvana_gate.rs; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
_T = "nirvana_gate::tests::"

CONTROLS = (
    # --- A2: new files, holders, case ---
    # A new file no live order owns is a note, not a fail
    Control("p84_new_file_is_a_note", GATE,
            "            None if is_new(path) => match fenced_new(path) {\n",
            "            None if false && is_new(path) => match fenced_new(path) {\n",
            _T + "p43_a_new_file_no_live_order_owns_is_a_note_and_an_old_one_the_briefs_fail", "rust"),
    # An old file outside owns with no holder is the brief's fault, with the --owns that covers it
    Control("p84_narrow_owns_is_the_briefs", GATE,
            "        if held.is_empty() && fenced.is_empty() {\n            Fault::Brief\n",
            "        if held.is_empty() && fenced.is_empty() {\n            Fault::Worker\n",
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
            "        let reads_here = segment_reads_local(segment) || runs(segment, LOCAL_COUNTERS);\n",
            "        let reads_here = segment_reads_local(segment) && runs(segment, LOCAL_COUNTERS);\n",
            _T + "p43_a_repo_relative_source_read_in_the_worktree_counts", "rust"),
    # A heredoc body is no read
    Control("p84_heredoc_is_no_read", GATE,
            "    let (text, scripts) = without_heredocs(cmd);\n",
            "    let (text, scripts) = (cmd.to_owned(), Vec::<String>::new());\n",
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

    # --- plan 43 fix round (F1 to F6, G1 to G3) ---
    # F1: a SyntaxError whose frame is a worker's file is the worker's, even with a <string> frame in the output
    Control("p84_f1_frame_is_the_workers", GATE,
            "            if !lines[frame].starts_with(\"File \\\"<string>\\\"\") {\n",
            "            if false && !lines[frame].starts_with(\"File \\\"<string>\\\"\") {\n",
            _T + "p43_f1_a_workers_broken_module_under_python_c_stays_the_workers", "rust"),
    # F1: a <string> frame under a traceback is code the -c text ran (exec), the worker's
    Control("p84_f1_traceback_is_the_workers", GATE,
            "            if !traced {\n",
            "            if !traced || true {\n",
            _T + "p43_f1_a_workers_broken_module_under_python_c_stays_the_workers", "rust"),
    # F1: bash's own -c: line only, not another shell's
    Control("p84_f1_bash_own_c_line", GATE,
            "            own(line)\n",
            "            line.contains(\"-c:\")\n",
            _T + "p43_f1_a_workers_broken_module_under_python_c_stays_the_workers", "rust"),
    # F2: a fenced new file is no note
    Control("p84_f2_fence_applies", GATE,
            "            None if is_new(path) => match fenced_new(path) {\n",
            "            None if is_new(path) => match None::<&'static str> {\n",
            _T + "p43_f2_a_fenced_new_file_fails_as_the_workers_and_an_ordinary_one_is_a_note", "rust"),
    # F2: a dot-prefixed component is fenced
    Control("p84_f2_dot_prefixed", GATE,
            "    if parts.iter().any(|part| part.starts_with('.')) {\n",
            "    if parts.iter().any(|part| part.starts_with(\"never.\")) {\n",
            _T + "p43_f2_a_fenced_new_file_fails_as_the_workers_and_an_ordinary_one_is_a_note", "rust"),
    # F2: nirvana/controls.d/ is fenced
    Control("p84_f2_controls_d", GATE,
            "    if lower.starts_with(\"nirvana/controls.d/\") {\n",
            "    if lower.starts_with(\"never/controls.d/\") {\n",
            _T + "p43_f2_a_fenced_new_file_fails_as_the_workers_and_an_ordinary_one_is_a_note", "rust"),
    # F2: build.rs and conftest.py are fenced
    Control("p84_f2_build_and_conftest", GATE,
            "    if name == \"build.rs\" || name == \"conftest.py\" {\n",
            "    if name == \"never.rs\" {\n",
            _T + "p43_f2_a_fenced_new_file_fails_as_the_workers_and_an_ordinary_one_is_a_note", "rust"),
    # F2: git hooks are fenced
    Control("p84_f2_git_hooks", GATE,
            "    if (in_hooks && HOOKS.contains(&stem))\n",
            "    if (false && in_hooks && HOOKS.contains(&stem))\n",
            _T + "p43_f2_a_fenced_new_file_fails_as_the_workers_and_an_ordinary_one_is_a_note", "rust"),
    # F2: a credential-looking name is fenced (p72's secret.txt)
    Control("p84_f2_credential_names", GATE,
            "    if key_ext || ssh_key || lower.contains(\"secret\") || lower.contains(\"credential\") {\n",
            "    if key_ext || ssh_key {\n",
            _T + "p72_a_forged_start_record_passes_nothing", "rust"),
    # F3: an amend's wake names the command that prints the line whole
    Control("p84_f3_wake_full_line", RUN,
            "                \"{order} gated {verdict}: {}; full line: herdr gate run {order}\",\n",
            "                \"{order} gated {verdict}: {}\",\n",
            _T + "p43_f3_the_amend_wake_names_the_full_line", "rust"),
    # F4: a Claude call answered with an error takes its reads back
    Control("p84_f4_claude_error_takes_back", GATE,
            "                            reads.local.retain(|read| read.call != id);\n",
            "                            reads.local.retain(|_| true);\n",
            _T + "p43_f4_a_read_answered_with_an_error_is_no_read", "rust"),
    # F4: an agy step that failed takes its reads back
    Control("p84_f4_agy_error_takes_back", GATE,
            "            reads.local.retain(|read| read.call != call);\n",
            "            reads.local.retain(|_| true);\n",
            _T + "p43_f4_a_read_answered_with_an_error_is_no_read", "rust"),
    # F4: a path counts only in a reading command's own segment
    Control("p84_f4_segment_scoped", GATE,
            "        if !reads_here || runs(segment, LOCAL_WRITERS) {\n",
            "        if runs(segment, LOCAL_WRITERS) {\n",
            _T + "p43_f4_a_shell_read_vouches_for_its_own_segment_only", "rust"),
    # F4: the media note needs media judged, not any read
    Control("p84_f4_media_judged", GATE,
            "            .any(|read| !read.search && media_file(&read.path));\n",
            "            .any(|read| !read.search);\n",
            _T + "p43_f4_a_media_order_that_read_only_text_files_and_cited_none_fails", "rust"),
    # F5: a bare name is local only with a known file extension
    Control("p84_f5_local_needs_a_file", GATE,
            "                !stem.is_empty() && FILE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())\n",
            "                !stem.is_empty()\n",
            _T + "p43_f5_research_read_cannot_cite_invented_sources", "rust"),
    # F5: every entry skipped says none was checked
    Control("p84_f5_none_checked", GATE,
            "    if missing.is_empty() && skipped == sources.len() {\n",
            "    if false && missing.is_empty() && skipped == sources.len() {\n",
            _T + "p43_f5_research_read_cannot_cite_invented_sources", "rust"),
    # F6: the amend is journaled as gated amending before its re-gate
    Control("p84_f6_amend_journaled", GATE,
            "                    verdict: Outcome::AMENDING.into(),\n                    reason: Some(reason.clone()),\n",
            "                    verdict: Outcome::Amend.word().into(),\n                    reason: Some(reason.clone()),\n",
            _T + "p43_f6_the_amend_is_journaled_before_its_regate_and_gate_run_finishes_it", "rust"),
    # F6: an order left amending is re-gated by gate run
    Control("p84_f6_amending_regates", GATE,
            "        Phase::Gated if known.verdict.as_deref() == Some(Outcome::AMENDING) => {}\n",
            "        Phase::Gated if known.verdict.as_deref() == Some(\"never\") => {}\n",
            _T + "p43_f6_the_amend_is_journaled_before_its_regate_and_gate_run_finishes_it", "rust"),
    # F6: an order gated asked is not amended
    Control("p84_f6_asked_refused", GATE,
            "    if known.phase == Phase::Gated && verdict == Outcome::Asked.word() {\n",
            "    if false && known.phase == Phase::Gated && verdict == Outcome::Asked.word() {\n",
            _T + "p43_f6_an_asked_or_ungated_order_is_not_amended", "rust"),
    # F6: a delivered order not gated is gated first
    Control("p84_f6_ungated_refused", GATE,
            "    if known.phase == Phase::Delivered {\n        return Err(format!(\n",
            "    if false && known.phase == Phase::Delivered {\n        return Err(format!(\n",
            _T + "p43_f6_an_asked_or_ungated_order_is_not_amended", "rust"),
    # G1: a tracked file of herdr's comes back as it was
    Control("p84_g1_tracked_back_as_it_was", GATE,
            "        let put_back = entry.tracked || !to.exists();\n",
            "        let put_back = !to.exists();\n",
            _T + "p43_g1_a_set_aside_comes_back_as_it_was_and_leaves_no_copy", "rust"),
    # G1: the aside copy is deleted after the restore
    Control("p84_g1_aside_deleted", GATE,
            "        std::fs::remove_dir_all(aside).map_err(|err| format!(\"{}: {err}\", aside.display()))?;\n",
            "        let _ = aside;\n",
            _T + "p43_g1_a_set_aside_comes_back_as_it_was_and_leaves_no_copy", "rust"),
    # G1: the restore runs when the rebase fails
    Control("p84_g1_restore_after_failed_rebase", GATE,
            "            let restored = guard.restore();\n",
            "            let restored: Result<Vec<String>, String> = { std::mem::forget(guard); Ok(Vec::new()) };\n",
            _T + "p43_g1_a_failed_rebase_still_puts_herdrs_files_back", "rust"),
    # G1: a set-aside that fails midway restores before it returns
    Control("p84_g1_midway_restores", GATE,
            "        let restored = restore_harness(cwd, aside)\n",
            "        let restored = Ok::<Vec<String>, String>(Vec::new())\n",
            _T + "p43_g1_a_set_aside_that_fails_midway_puts_everything_back", "rust"),
    # G1: the next gate restores what a dead gate set aside
    Control("p84_g1_dead_gate_recovered", GATE,
            "            if aside.join(ASIDE_MANIFEST).is_file() {\n",
            "            if false && aside.join(ASIDE_MANIFEST).is_file() {\n",
            _T + "p43_g1_the_next_gate_restores_what_a_dead_gate_set_aside", "rust"),
    # G2: gate merge --by-hand takes no amend flag
    Control("p84_g2_by_hand_flags", GATE,
            "        Some(reason) => !amend_flags && !reason.trim().is_empty(),\n",
            "        Some(reason) => !reason.trim().is_empty(),\n",
            _T + "p43_g2_merge_by_hand_takes_no_amend_flag", "rust"),
    # G3: a shell read after a cd is under its folder
    Control("p84_g3_read_follows_cd", GATE,
            "        if let Some(dir) = folder.as_ref().filter(|_| !absolute(&path)) {\n",
            "        if let Some(dir) = None::<&String> {\n",
            _T + "p43_a_shell_read_follows_cd_quoted_pipes_and_script_heredocs", "rust"),
    # G3: a script heredoc that opens a file reads its quoted paths
    Control("p84_g3_script_heredoc_reads", GATE,
            "        if !script.lines().any(segment_reads_local) {\n",
            "        if true {\n",
            _T + "p43_a_shell_read_follows_cd_quoted_pipes_and_script_heredocs", "rust"),
    # G3: a | inside quotes does not cut the segment
    Control("p84_g3_quoted_pipe", GATE,
            "                '\"' | '\\'' => {\n                    quote = Some(c);\n",
            "                '\"' | '\\'' => {\n                    quote = None;\n",
            _T + "p43_a_shell_read_follows_cd_quoted_pipes_and_script_heredocs", "rust"),
    # G3: --send-back seats a child
    Control("p84_g3_send_back_seats", GATE,
            "        if send_back {\n            Route::Child\n",
            "        if false {\n            Route::Child\n",
            _T + "p43_g3_send_back_seats_a_child_and_a_live_child_refuses_the_amend", "rust"),
    # G3: a live child refuses the amend
    Control("p84_g3_live_child_refuses", GATE,
            "        .find(|child| child.seated.parent.as_deref() == Some(order) && child.phase.wants_worker())\n",
            "        .find(|child| child.seated.parent.as_deref() == Some(order) && child.phase.wants_worker() && false)\n",
            _T + "p43_g3_send_back_seats_a_child_and_a_live_child_refuses_the_amend", "rust"),
    # G3: --sha must be on the integration branch
    Control("p84_g3_by_hand_sha_on_branch", GATE,
            "            if !on_branch(&full) {\n",
            "            if false && !on_branch(&full) {\n",
            _T + "p43_a_hand_merge_is_journaled_with_its_sha_and_reason", "rust"),
)
