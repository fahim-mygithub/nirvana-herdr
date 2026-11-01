# Plan 35 P35b: what an agy worker starts with (package `p35b_agy_tools`,
# nirvana/plans/2026-10-31-35-records/p35b.md). Controls starting with p35b_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place (written before the run). `Control` is provided by
# controls.py.
AGY = "src/nirvana_agy_tools.rs"
GUI = "src/nirvana_gui_tools.rs"
WORKER = "src/nirvana_worker.rs"
PROFILES = "src/nirvana_profiles.rs"

T_DENY = "p35b_each_claude_tool_maps_to_its_measured_agy_rule_and_any_other_is_refused"
T_SERVER = "p35b_a_server_is_translated_to_agys_shape_and_one_agy_cannot_take_is_refused"
T_ORDER = "p35b_an_agy_server_order_gets_its_servers_in_the_tools_folder_and_f3_holds"
T_NOTHING = "p35b_an_order_with_nothing_to_add_keeps_todays_argv_byte_for_byte"
T_RULES = "p35b_a_project_with_only_claude_md_gives_agy_its_rules_and_the_prompt_names_both_folders"
T_OWN = "p35b_agy_rules_of_the_projects_own_mean_no_copy"
T_LONG = "p35b_a_long_claude_md_still_goes_and_the_view_says_agy_may_cut_it"
T_SKILLS = "p35b_the_projects_claude_skills_are_copied_whole_unless_the_workspace_has_them"
T_DENY_FILE = "p35b_a_denied_order_gets_its_rules_in_the_settings_file_and_no_add_dir"
T_REMOVE = "p35b_the_tools_folder_and_the_deny_file_go_and_a_failed_remove_says_so"
T_SOURCES = "p35b_only_an_agy_research_or_media_order_gets_the_sources_line"
T_SEAT = "p35b_a_non_desktop_server_order_may_seat_on_agy_unless_agy_cannot_take_its_definition"
T_FINISHED = "p35b_an_agy_result_with_a_finished_reason_fails_naming_it"
T_EXEC = "p35b_exec_gives_an_agy_worker_its_tools_folder_and_removes_it_after_a_failed_one_included"
T_PLAIN = "p35b_exec_an_agy_order_with_nothing_to_add_starts_with_todays_argv"
T_REVIEW = "p35b_an_agy_lesson_review_is_denied_commands_and_writes"
T_LINE = "p35b_seat_run_gives_an_agy_research_order_the_sources_line"
T_PROFILE = "p35b_a_profile_denying_mapped_tools_seats_on_agy_and_an_unmapped_one_does_not"
# The review round (records/p35b.md "Review round").
T_CAP = "p35b_a_skill_over_the_cap_is_left_out_and_the_skills_together_have_a_cap"
T_LINK = "p35b_a_linked_skill_is_left_out_with_a_line"
T_WALK = "p35b_the_workspace_walk_stops_at_a_checkout_and_reaches_the_root_below_one"
T_VERBATIM = "p35b_a_verbatim_path_is_named_one_way"
T_LEFTOVERS = "p35b_the_leftover_orders_are_read_from_both_folders"
T_SWEEP = "p35b_exec_sweeps_the_tool_files_ended_orders_left_and_journals_an_unknown_finish"
T_CLEANUP = "p35b_seat_cleanup_removes_the_orders_tool_files"
T_CHILD = "p35b_an_agy_research_child_brief_gets_the_sources_line_once"

CONTROLS = (
    # Item 2 and R4: a non-desktop server no longer keeps an order on claude; a definition agy cannot take does.
    Control("p35b_claude_only_every_server", GUI,
            "    servers.iter().find_map(|server| match defined.get(server) {\n",
            "    if !servers.is_empty() {\n        return Some(String::from(\"every server on claude\"));\n    }\n    servers.iter().find_map(|server| match defined.get(server) {\n",
            T_SEAT, "rust"),
    Control("p35b_seat_definitions_ignored", GUI,
            "        claude_only: claude_only(&named, &definitions),\n",
            "        claude_only: claude_only(&named, &Map::new()),\n", T_SEAT, "rust"),
    # Item 2: the translation (url -> serverUrl, type dropped, the rest passes) and the refusals.
    Control("p35b_url_kept", AGY,
            '                    out.insert("serverUrl".to_owned(), value.clone());\n',
            '                    out.insert("url".to_owned(), value.clone());\n', T_SERVER, "rust"),
    Control("p35b_type_kept", AGY,
            '                "type" => {}\n', "", T_SERVER, "rust"),
    Control("p35b_variable_passed", AGY,
            '        Value::String(text) => text.contains("${"),\n',
            '        Value::String(text) => text.contains("never-a-variable"),\n', T_SERVER, "rust"),
    Control("p35b_sse_allowed", AGY,
            '        "stdio" if command.is_some_and(Value::is_string) && url.is_none() => None,\n',
            '        "sse" => None,\n        "stdio" if command.is_some_and(Value::is_string) && url.is_none() => None,\n',
            T_SERVER, "rust"),
    Control("p35b_unknown_key_kept", AGY,
            "        .find(|key| !SERVER_KEYS.contains(&key.as_str()))\n",
            "        .find(|_| false)\n", T_SERVER, "rust"),
    # Item 2: the agy path of order_argv writes the translated servers, leaves a workspace duplicate out, and F3 holds.
    Control("p35b_agy_order_argv_skipped", GUI,
            '    let agy = cli == Some("agy");\n', "    let agy = false;\n", T_ORDER, "rust"),
    Control("p35b_servers_untranslated", AGY,
            "        let mut agy = translate(definition);\n",
            "        let mut agy = definition.clone();\n", T_ORDER, "rust"),
    Control("p35b_workspace_dup_kept", AGY,
            "        if native.contains(server) {\n", "        if false && native.contains(server) {\n",
            T_ORDER, "rust"),
    Control("p35b_f3_desktop_on_agy", GUI,
            "        if let Some(why) = claude_only(&servers, &all) {\n",
            "        if let Some(why) = claude_only(&servers, &all).filter(|_| false) {\n", T_ORDER, "rust"),
    # Item 1: an order with nothing gets today's argv byte for byte, and a claude argv is never touched.
    Control("p35b_empty_folder_used", AGY,
            "    let used = std::fs::read_dir(&tools).is_ok_and(|mut entries| entries.next().is_some());\n",
            "    let used = true;\n", T_NOTHING, "rust"),
    Control("p35b_claude_argv_touched", AGY,
            '    if argv.first().map(String::as_str) != Some("agy") {\n        return Ok(argv);\n    }\n    let tools = tools_dir',
            "    let tools = tools_dir", T_NOTHING, "rust"),
    # Items 1, 3 and 5: the rules copy, the --add-dir, the write deny on the tools folder, the prompt, the flags' place.
    Control("p35b_add_dir_left_out", AGY,
            '        flags.extend(["--add-dir".to_owned(), plain_path(&tools)]);\n', "", T_RULES, "rust"),
    Control("p35b_tools_deny_missing", AGY,
            '        add(&mut deny, format!("write_file({}/)", rule_path(&tools)));\n', "", T_RULES, "rust"),
    Control("p35b_rule_path_backslash", AGY,
            "    plain_path(path).replace('\\\\', \"/\")\n",
            "    plain_path(path)\n", T_RULES, "rust"),
    Control("p35b_prompt_unchanged", AGY,
            "                first_line(order.order_file, order.cwd, &tools)\n",
            "                crate::nirvana_worker::first_line(order.order_file)\n", T_RULES, "rust"),
    Control("p35b_resume_prompt_replaced", AGY,
            "        if let Some(print) = argv.iter_mut().find(|arg| **arg == today) {\n",
            '        if let Some(print) = argv.iter_mut().rfind(|arg| arg.starts_with("--print=")) {\n',
            T_RULES, "rust"),
    Control("p35b_flags_after_print", AGY,
            '        .rposition(|arg| arg.starts_with("--print="))\n', "        .rposition(|_| false)\n",
            T_RULES, "rust"),
    Control("p35b_rules_not_copied", AGY,
            '        write_file(&tools.join("AGENTS.md"), text.as_bytes(), false)?;\n',
            "        let _ = text;\n", T_RULES, "rust"),
    Control("p35b_imports_unsaid", AGY,
            "    let imports = if has_imports(body) {\n", "    let imports = if false {\n", T_RULES, "rust"),
    Control("p35b_rules_over_own", AGY,
            "        .any(|dir| has_agy_rules(dir))\n", "        .any(|dir| false && has_agy_rules(dir))\n", T_OWN,
            "rust"),
    Control("p35b_budget_silent", AGY,
            "        if text.len() > RULES_BUDGET_BYTES {\n", "        if false {\n", T_LONG, "rust"),
    # Item 5: the project's skills, copied whole, never over the workspace's own.
    Control("p35b_skills_not_copied", AGY,
            "        copy_folder(from, &target).map_err(|err| {\n",
            "        Ok::<(), std::io::Error>(()).map_err(|err| {\n", T_SKILLS, "rust"),
    Control("p35b_skills_over_workspace", AGY,
            '            .any(|dir| dir.join(".agents").join("skills").join(&name).exists())\n',
            "            .any(|_| false)\n", T_SKILLS, "rust"),
    # Item 4: denials. The mapping is never widened; mcp tool and server forms; a review's and a profile's rules.
    Control("p35b_deny_widened", AGY,
            '    let rest = tool.strip_prefix("mcp__")?;\n',
            '    let rest = tool.strip_prefix("mcp__").unwrap_or(tool);\n', T_DENY, "rust"),
    Control("p35b_mcp_tool_form", AGY,
            '    let (server, name) = rest.split_once("__").unwrap_or((rest, "*"));\n',
            '    let (server, name) = (rest, "*");\n', T_DENY, "rust"),
    Control("p35b_settings_not_written", AGY,
            '    flags.extend(["--settings".to_owned(), plain_path(&file)]);\n', "",
            T_DENY_FILE, "rust"),
    Control("p35b_review_not_denied", WORKER,
            "        return Ok(crate::nirvana_agy_tools::REVIEW_DENIED\n            .map(str::to_owned)\n            .to_vec());\n",
            "        return Ok(Vec::new());\n", T_REVIEW, "rust"),
    Control("p35b_profile_not_denied", WORKER,
            "    crate::nirvana_agy_tools::deny_rules(&parsed.disallowed_tools)\n}\n",
            "    let _ = parsed;\n    Ok(Vec::new())\n}\n", T_PROFILE, "rust"),
    Control("p35b_profile_agy_unchecked", WORKER,
            "            crate::nirvana_agy_tools::deny_rules(&parsed.disallowed_tools).map_err(|why| {\n",
            "            Ok::<Vec<String>, String>(Vec::new()).map_err(|why: String| {\n", T_PROFILE, "rust"),
    Control("p35b_narrow_every_denial", PROFILES,
            "    let unmapped = crate::nirvana_agy_tools::unmapped(&profile.disallowed_tools);\n",
            "    let unmapped = profile.disallowed_tools.clone();\n", T_PROFILE, "rust"),
    # Item 3: the sources line, on agy research and media orders only.
    Control("p35b_sources_line_missing", WORKER,
            "    if let Some(line) = crate::nirvana_agy_tools::sources_line(&booked.cli, &booked.class) {\n",
            "    if let Some(line) = None::<&str> {\n", T_LINE, "rust"),
    Control("p35b_sources_line_claude", AGY,
            '    (cli == "agy" && (class.starts_with("research.") || class.starts_with("media.")))\n',
            '    ((class.starts_with("research.") || class.starts_with("media.")))\n', T_SOURCES, "rust"),
    # Item 6: finished_reason fails the result.
    Control("p35b_finished_ignored", WORKER,
            "                .and_then(Value::as_str)\n                .filter(|reason| AGY_UNFINISHED",
            "                .and_then(|_| None::<&str>)\n                .filter(|reason| AGY_UNFINISHED",
            T_FINISHED, "rust"),
    # Items 1 and 7: the folder goes when the worker ends (failed too), a stale one never reaches a start, and an
    # error after the servers were written removes them.
    Control("p35b_folder_not_removed", AGY,
            "    if let Err(err) = std::fs::remove_dir_all(&tools) {\n",
            "    if let Err(err) = Ok::<(), std::io::Error>(()) {\n", T_REMOVE, "rust"),
    Control("p35b_remove_without_agy", GUI,
            "    crate::nirvana_agy_tools::remove(run_dir, order, out);\n", "", T_EXEC, "rust"),
    Control("p35b_exec_no_tools", WORKER,
            "        .and_then(|argv| with_agy_tools(argv, project, &run_dir, &seated, out))\n", "",
            T_EXEC, "rust"),
    Control("p35b_exec_stale_kept", WORKER,
            "        crate::nirvana_gui_tools::remove_order_config(&run_dir, &seated.order, out);\n        ensure_sources_line(",
            "        ensure_sources_line(", T_EXEC, "rust"),
    Control("p35b_exec_error_keeps", WORKER,
            "                crate::nirvana_gui_tools::remove_order_config(&run_dir, &seated.order, out);\n                end_failed(",
            "                end_failed(", T_EXEC, "rust"),
    # The review round. S1: a stdio server with no cwd gets the worker's folder; one with its own keeps it.
    Control("p35b_cwd_not_written", AGY,
            '            if object.contains_key("command") && !object.contains_key("cwd") {\n',
            "            if false {\n", T_ORDER, "rust"),
    Control("p35b_cwd_overwritten", AGY,
            '            if object.contains_key("command") && !object.contains_key("cwd") {\n',
            '            if object.contains_key("command") {\n', T_ORDER, "rust"),
    # S2: only the three named reasons, or a string beside another status, fail; another one is a journaled note.
    Control("p35b_finished_any_string", WORKER,
            '                .filter(|reason| AGY_UNFINISHED.contains(reason) || status != "SUCCESS")\n',
            "                .filter(|_| true)\n", T_FINISHED, "rust"),
    Control("p35b_finished_status_ignored", WORKER,
            '                .filter(|reason| AGY_UNFINISHED.contains(reason) || status != "SUCCESS")\n',
            "                .filter(|reason| AGY_UNFINISHED.contains(reason))\n", T_FINISHED, "rust"),
    Control("p35b_finished_note_silent", WORKER,
            "        (ok && !AGY_UNFINISHED.contains(&reason)).then(|| {\n",
            "        (false && !AGY_UNFINISHED.contains(&reason)).then(|| {\n", T_FINISHED, "rust"),
    Control("p35b_finished_note_unjournaled", WORKER,
            "                            why: note,\n", "                            why: None,\n", T_SWEEP, "rust"),
    # S3: Edit has no agy rule, and the deny file's folder is denied.
    Control("p35b_edit_mapped", AGY,
            '    ("Write", "write_file(*)"),\n', '    ("Edit", "write_file(*)"),\n', T_DENY, "rust"),
    Control("p35b_agy_folder_undenied", AGY,
            '        format!("write_file({}/)", rule_path(&order.run_dir.join(DIR))),\n',
            '        String::from("command(*)"),\n', T_RULES, "rust"),
    # S4: the sweep runs at start, takes the agy and mcp files, and spares an order that wants a worker or is live.
    Control("p35b_sweep_skipped", WORKER,
            "    sweep_leftovers(project, run, order, out);\n", "", T_SWEEP, "rust"),
    Control("p35b_sweep_wanting", WORKER,
            "                .is_some_and(|known| known.phase.wants_worker())\n",
            "                .is_some_and(|_| false)\n", T_SWEEP, "rust"),
    Control("p35b_sweep_live_ignored", WORKER,
            "                || live(project, &other, &name)\n", "", T_SWEEP, "rust"),
    Control("p35b_leftover_mcp_missed", AGY,
            '        .chain(names(run_dir.join("mcp"), ".json", false))\n', "", T_LEFTOVERS, "rust"),
    # S5: seat cleanup removes the order's files; the per-skill and the total caps.
    Control("p35b_cleanup_keeps_files", WORKER,
            "    crate::nirvana_gui_tools::remove_order_config(&project.run_dir(run), order, out);\n", "",
            T_CLEANUP, "rust"),
    Control("p35b_skill_cap_ignored", AGY,
            "        if bytes > per_skill {\n", "        if false {\n", T_CAP, "rust"),
    Control("p35b_skills_total_ignored", AGY,
            "        if copied + bytes > total {\n", "        if false {\n", T_CAP, "rust"),
    # S6: a server part with `_` or `.`, a tool part with `__` or a leading `_`, names no one agy server for sure.
    Control("p35b_flattened_server_mapped", AGY,
            "            .all(|c| c.is_ascii_alphanumeric() || c == '-');\n",
            "            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));\n", T_DENY, "rust"),
    Control("p35b_split_tool_mapped", AGY,
            '            && !name.contains("__")\n', "", T_DENY, "rust"),
    Control("p35b_underscore_tool_mapped", AGY,
            "            && !name.starts_with('_')\n", "", T_DENY, "rust"),
    # Nits: rules once each; one path text; links; the walk; imports; a child brief's sources line.
    Control("p35b_dedup_adjacent", AGY,
            "        if !deny.contains(&rule) {\n", "        if deny.last() != Some(&rule) {\n", T_DENY_FILE, "rust"),
    Control("p35b_add_dir_verbatim", AGY,
            '    match text.strip_prefix(r"\\\\?\\") {\n', "    match None::<&str> {\n", T_VERBATIM, "rust"),
    Control("p35b_link_followed", AGY,
            '        if kind.is_symlink() && entry.path().join("SKILL.md").is_file() {\n',
            "        if false {\n", T_LINK, "rust"),
    Control("p35b_linked_root_followed", AGY,
            "    if is_link(&from) {\n", "    if false {\n", T_LINK, "rust"),
    Control("p35b_walk_past_git", AGY,
            '        if at == root || !at.starts_with(root) || at.join(".git").exists() {\n',
            "        if at == root || !at.starts_with(root) {\n", T_WALK, "rust"),
    Control("p35b_root_skills_ignored", AGY,
            "    let workspace = workspace_dirs(cwd, root);\n", "    let workspace = vec![cwd.to_path_buf()];\n",
            T_WALK, "rust"),
    Control("p35b_types_node_import", AGY,
            "                    .is_some_and(|last| last.trim_start_matches('.').contains('.'))\n",
            "                    .is_some_and(|_| true)\n", T_WALK, "rust"),
    Control("p35b_child_line_twice", WORKER,
            "    if text.contains(line) {\n        return;\n    }\n", "", T_CHILD, "rust"),
    Control("p35b_exec_adds_to_nothing", AGY,
            "    let used = std::fs::read_dir(&tools).is_ok_and(|mut entries| entries.next().is_some());\n",
            "    let used = true;\n", T_PLAIN, "rust"),
)
