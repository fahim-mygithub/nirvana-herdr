# Plan 40 P40b ORDERS (2026-11-09): the worker, gate and run fixes from review-orders, the lighter worker start, the
# image guard, and the usage audit's addendum (accepted orders, media sources). Controls starting with c40b_. One
# sabotage per assertion, each on a product line; the test named must print FAIL with it in place (the expected red
# line, written before the run). `Control` is provided by controls.py.
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
WORKER = "src/nirvana_worker.rs"
JOB = "src/nirvana_worker/job.rs"
VIEW = "src/nirvana_worker/view.rs"
CLAUDE = "src/nirvana_claude.rs"
HOOKS = "nirvana/claude/hooks/hooks.json"

T_CHAIN = "c40b_a_reseat_child_is_part_of_its_parents_chain"
T_TREE = "c40b_a_timed_out_test_is_killed_with_its_whole_tree"
T_CHECK = "c40b_a_check_that_ran_early_is_not_cut_by_later_passed_lines"
T_CAFE = "c40b_a_non_ascii_new_file_is_committed"
T_CASE = "c40b_test_check_and_serves_ignore_case"
T_CAP = "c40b_file_lists_are_capped_with_a_count_of_the_rest"
T_TALLY = "c40b_an_accepted_order_counts_as_a_pass"
T_VIDEO = "c40b_a_media_order_cites_the_video_it_opened_by_path"
T_PAGES = "c40b_a_media_order_still_fails_pages_it_never_fetched"
T_CEILING = "c40b_an_open_tool_call_is_allowed_the_longer_ceiling"
T_WORD = "c40b_an_accepted_verdict_is_the_records_outcome"
T_CORRECT = "c40b_an_accept_after_a_failed_gate_corrects_the_orders_line"
T_ARGV = "c40b_a_claude_worker_excludes_the_dynamic_system_prompt_sections"
T_FOOTER = "c40b_the_order_footer_says_finish_the_order_and_nothing_twice"
T_LEAN = "c40b_a_report_without_tests_or_files_passes"
T_SCHEMA = "the_report_check_and_the_schema_name_the_same_fields"
T_TEE = "c40b_the_tee_drops_an_images_base64_and_keeps_the_rest"
T_OPEN = "c40b_a_tool_call_is_open_from_its_call_to_its_answer"
T_LOSSY = "c40b_a_non_utf8_line_is_read_lossily_and_reading_goes_on"
T_POOL = "only_a_rejected_rate_limit_event_or_a_rate_limited_retry_stops_a_claude_worker"
T_COUNTS = "c40b_a_reports_tests_show_as_counts"
T_SEAT = "c40b_the_guard_names_the_sessions_own_seat"
T_IMAGE = "c40b_the_orchestrator_may_not_read_a_large_image"
T_HOOK = "c40b_the_image_guard_is_a_read_hook"
T_SUB = "a_worker_subagents_outcome_is_recorded_under_its_seat_effort_and_class"
# Review round.
T_REFUSE = "c40b_a_media_cite_needs_a_call_that_opened_it"
T_OPENED = "c40b_a_media_cite_counts_a_call_that_opened_it"
T_PROSE = "c40b_a_check_line_of_prose_names_no_check"
T_PIN = "c40b_the_pinned_check_line_is_its_result"
T_NOTE = "c40b_a_correction_that_fails_is_noted_beside_the_record"

CONTROLS = (
    # B1: a reseat is followed to the root, and only a send-back adds to the depth.
    Control("c40b_chain_stops_at_reseat", GATE,
            '(Some(why @ ("send_back" | "reseat")), Some(parent)) => {',
            '(Some(why @ "send_back"), Some(parent)) => {', T_CHAIN, "rust"),
    Control("c40b_reseat_counts_as_send_back", GATE,
            '                if why == "send_back" {\n                    depth += 1;',
            '                if !why.is_empty() {\n                    depth += 1;', T_CHAIN, "rust"),
    # B2: the test runs in a job; outside it, the timeout kills nothing and the gate waits out the ping (29 s).
    Control("c40b_test_outside_job", GATE,
            "        crate::nirvana_worker::job::spawn(&mut command)\n",
            "        command.spawn().and_then(|child| Ok((crate::nirvana_worker::job::Job::new()?, child)))\n",
            T_TREE, "rust"),
    # B4: judged on the whole output, and the check's first line survives the cut.
    Control("c40b_check_read_from_shown_lines", GATE,
            ".filter(|check| !run.output.lines().any(|line| line.contains(check.as_str())))",
            ".filter(|check| !lines.iter().any(|line| line.contains(check.as_str())))", T_CHECK, "rust"),
    Control("c40b_check_line_not_pinned", GATE,
            "            pinned[at] = true;\n", "            pinned[at] = false;\n", T_CHECK, "rust"),
    # B5: `-z` in commit_work's status.
    Control("c40b_commit_reads_quoted_names", GATE,
            '            "status",\n            "-z",\n            "--porcelain=v1",\n',
            '            "status",\n            "--porcelain=v1",\n',
            T_CAFE, "rust"),
    # B6: `Test:` read case-sensitively again.
    Control("c40b_test_key_case_sensitive", GATE,
            '} else if let Some(rest) = strip_key(line, "test:") {',
            '} else if let Some(rest) = line.strip_prefix("test:") {', T_CASE, "rust"),
    # T1: the cap.
    Control("c40b_file_list_uncapped", GATE,
            "    if paths.len() > FILE_LINES_MOST {\n", "    if paths.len() > usize::MAX - 1 {\n", T_CAP, "rust"),
    # Addendum A: accepted counts as a pass.
    Control("c40b_accepted_is_a_failure", GATE,
            "        if word == Self::ACCEPTED {\n            return Some(true);",
            "        if word == Self::ACCEPTED {\n            return Some(false);", T_TALLY, "rust"),
    # Addendum B: the media rule, only for media.*, by URL or by video id, never by the report's own call.
    Control("c40b_media_read_off", GATE,
            '        if class.starts_with("media.") && media_read(url, &reads) {',
            '        if class.starts_with("media.") && false {', T_VIDEO, "rust"),
    Control("c40b_media_read_for_research", GATE,
            '        if class.starts_with("media.") && media_read(url, &reads) {',
            "        if media_read(url, &reads) {", T_VIDEO, "rust"),
    Control("c40b_media_video_id_ignored", GATE,
            "        Some(id @ StableId::Video(_)) => reads.media_files.iter().any(|file| id_in(file, &id)),",
            "        Some(StableId::Video(_)) => false,", T_VIDEO, "rust"),
    Control("c40b_media_report_call_vouches", GATE,
            '        let output = step["tool_info"]["output"].as_str().unwrap_or("");\n',
            '        let output = step["tool_info"]["output"].as_str().unwrap_or("");\n'
            "        reads.media_files.push(json_text(params));\n", T_VIDEO, "rust"),
    Control("c40b_media_url_in_command_ignored", GATE,
            "    if !norm.is_empty() && reads.media_urls.contains(&norm) {\n",
            "    if !norm.is_empty() && false {\n", T_PAGES, "rust"),
    # Review round S1: each refusal. echo and the heredoc are refused by the same line (a command that fetches
    # nothing gives no URL), so one control covers both.
    Control("c40b_media_any_command_names_urls", GATE,
            "    if command_fetches(cmd) {\n        urls.extend(",
            "    if true {\n        urls.extend(", T_REFUSE, "rust"),
    Control("c40b_media_ls_reads_as_ffprobe", GATE,
            '        let probe = command_words(&code, "ffprobe").next().is_some();\n',
            "        let probe = true;\n", T_REFUSE, "rust"),
    Control("c40b_media_error_answer_counts", GATE,
            "                            if block.get(\"is_error\").and_then(Value::as_bool) != Some(true) {\n",
            "                            if true {\n", T_REFUSE, "rust"),
    Control("c40b_media_agy_unanswered_counts", GATE,
            '        let answered = step.get("state").and_then(Value::as_str) == Some("DONE")\n',
            "        let answered = true\n", T_REFUSE, "rust"),
    Control("c40b_media_any_file_counts", GATE,
            '    lower.ends_with(".info.json")\n        ||',
            "    true\n        ||", T_REFUSE, "rust"),
    Control("c40b_media_failed_agy_probe_counts", GATE,
            "                if answered && agy_command_ok(output) {\n",
            "                if answered {\n", T_REFUSE, "rust"),
    Control("c40b_media_ffmpeg_output_counts", GATE,
            '            let input = probe || (at > 0 && words[at - 1] == "-i");\n',
            "            let input = probe || mpeg;\n", T_REFUSE, "rust"),
    # Review round S1: what does count.
    Control("c40b_media_probed_file_dropped", GATE,
            "                files.push((*word).to_owned());\n",
            "                let _ = word;\n", T_OPENED, "rust"),
    Control("c40b_media_probed_url_dropped", GATE,
            "            urls.extend(extract_url_tokens(word).iter().map(|token| normalize_url(token)));\n",
            "", T_OPENED, "rust"),
    # Review round N1, N2, N4, S2.
    Control("c40b_check_prose_read_as_names", GATE,
            "        && (names.len() == 1 || rest.contains('`') || names.iter().all(|word| marked(word))))\n",
            "        && true)\n", T_PROSE, "rust"),
    Control("c40b_check_pins_first_naming_line", GATE,
            "        let result = naming().find(|(_, line)| reports(line, check)).or_else(|| naming().next_back());\n",
            "        let result = naming().next();\n", T_PIN, "rust"),
    Control("c40b_correction_failure_unnoted", RUN,
            "            .open(dir.join(CORRECTION_LOG))\n",
            '            .open(dir.join("elsewhere.log"))\n', T_NOTE, "rust"),
    Control("c40b_footer_locked_file_ordinary", WORKER,
            "Work through ordinary obstacles such as an error or a timeout.",
            "Work through ordinary obstacles such as an error, a timeout or a locked file.", T_FOOTER, "rust"),
    # B7: an open tool call gets the longer ceiling.
    Control("c40b_tool_ceiling_ignored", RUN,
            "        silent_s >= TOOL_STALL_AFTER_S\n", "        silent_s >= STALL_AFTER_S\n", T_CEILING, "rust"),
    # Addendum A: the record's outcome word, and the in-place correction.
    Control("c40b_accepted_verdict_unread", RUN,
            "    if order.verdict.as_deref() == Some(Outcome::ACCEPTED) {\n", "    if false {\n", T_WORD, "rust"),
    Control("c40b_record_not_corrected", RUN,
            "                old.outcome = line.outcome.clone();\n", "                old.outcome = old.outcome.clone();\n",
            T_CORRECT, "rust"),
    Control("c40b_correction_overwrites_a_pass", RUN,
            "                    && Outcome::tally(&old.outcome) == Some(false)\n", "                    && true\n",
            T_CORRECT, "rust"),
    Control("c40b_correction_by_a_failure", RUN,
            "    if Outcome::tally(&line.outcome) != Some(true) {\n        return Ok(false);",
            "    if false {\n        return Ok(false);", T_CORRECT, "rust"),
    # The lighter start: the flag, the footer, the optional lists.
    Control("c40b_exclude_flag_dropped", WORKER,
            "                    EXCLUDE_DYNAMIC,\n", "", T_ARGV, "rust"),
    Control("c40b_footer_says_folder_again", WORKER,
            'Finish the whole order before you report. Work through ordinary',
            'Work only in this folder. Finish the whole order before you report. Work through ordinary',
            T_FOOTER, "rust"),
    Control("c40b_files_required_again", WORKER,
            '    for (key, required) in [("files", false), ("gotchas", true)] {',
            '    for (key, required) in [("files", true), ("gotchas", true)] {', T_LEAN, "rust"),
    Control("c40b_tests_required_again", WORKER,
            "        None => &no_tests,\n", "        None => return Err(format!(\"tests: {}\", no_tests.len())),\n",
            T_LEAN, "rust"),
    Control("c40b_schema_requires_lists", WORKER,
            '"required":["status","summary","gotchas"],"properties":{"status"',
            '"required":["status","summary","files","tests","gotchas"],"properties":{"status"', T_SCHEMA, "rust"),
    # X1: the tee drops the base64.
    Control("c40b_tee_keeps_base64", WORKER,
            "                if let Some(text) = long {\n", "                if let Some(text) = long.filter(|_| false) {\n",
            T_TEE, "rust"),
    # B7: what an open call is.
    Control("c40b_tool_never_open", WORKER,
            "        !self.calls.is_empty()\n    }", "        false\n    }", T_OPEN, "rust"),
    # B3: lossy lines.
    Control("c40b_reader_stops_at_bad_utf8", JOB,
            "                if !each(String::from_utf8_lossy(&bytes).into_owned()) {\n",
            "                let Ok(line) = String::from_utf8(bytes.clone()) else { return };\n"
            "                if !each(line) {\n", T_LOSSY, "rust"),
    # M3: no pool typed into the stream reader.
    Control("c40b_max_pool_typed_again", WORKER,
            "                        pool: None,\n                        window,\n                        resets_at,\n"
            '                        detail: format!("rate_limit_event',
            '                        pool: Some("max".into()),\n                        window,\n                        resets_at,\n'
            '                        detail: format!("rate_limit_event', T_POOL, "rust"),
    # B9: the counts.
    Control("c40b_test_counts_dropped", VIEW,
            "            if let Some(counts) = test_counts(tests) {\n",
            "            if let Some(counts) = test_counts(&tests[..0]) {\n", T_COUNTS, "rust"),
    # M1: the session's seat in the guard's words.
    Control("c40b_seat_typed_again", CLAUDE,
            '        Some(seat) => format!("your own seat ({seat})"),\n',
            '        Some(_) => "your own seat (Opus 5.5 at your effort)".to_owned(),\n', T_SEAT, "rust"),
    # X2: the image guard.
    Control("c40b_image_size_ignored", CLAUDE,
            "    if bytes <= IMAGE_READ_MOST {\n", "    if bytes <= u64::MAX {\n", T_IMAGE, "rust"),
    Control("c40b_image_guard_for_everyone", CLAUDE,
            "    if should_skip_orchestrator_hook(hook, cwd, is_orchestrator) {\n        return None;\n    }\n"
            "    let file_path = hook\n        .pointer(\"/tool_input/file_path\")",
            "    let file_path = hook\n        .pointer(\"/tool_input/file_path\")", T_IMAGE, "rust"),
    Control("c40b_image_any_extension", CLAUDE,
            "    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {\n", "    if IMAGE_EXTENSIONS.is_empty() {\n",
            T_IMAGE, "rust"),
    Control("c40b_image_reason_leads_with_size", CLAUDE,
            '        "Ask about this image without reading it here: `herdr look',
            '        "This image is large. Ask about this image without reading it here: `herdr look',
            T_IMAGE, "rust"),
    Control("c40b_image_hook_unwired", HOOKS,
            '"guard-read"', '"guard-reed"', T_HOOK, "rust"),
    # Merge 5: the plugin's hook sweep counts the image guard and names its matcher.
    Control("c40b_image_hook_wrong_matcher", HOOKS,
            '"matcher": "Read",', '"matcher": "Reed",',
            "every_hook_names_a_herdr_subcommand_that_runs_quiet_and_fast", "rust"),
    # Addendum A: a subagent's prose end is a delivery.
    Control("c40b_subagent_prose_is_no_report", CLAUDE,
            "                            SUBAGENT_DELIVERED.to_owned()\n",
            '                            "subagent-no-report".to_owned()\n', T_SUB, "rust"),
)
