# Plan 35 P35c: bladebro for browser orders, on both CLIs (package `p35c_bladebro`,
# nirvana/plans/2026-10-31-35-records/p35c.md). Controls starting with p35c_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place (written before the run). `Control` is provided by
# controls.py.
BLADE = "src/nirvana_bladebro.rs"
GUI = "src/nirvana_gui_tools.rs"
AGY = "src/nirvana_agy_tools.rs"
LEAN = "src/nirvana_lean.rs"
WORKER = "src/nirvana_worker.rs"

T_DEF = "p35c_the_definition_is_decision_0043s_recipe_with_the_orders_own_home"
T_GRANT = "p35c_a_browser_need_grants_bladebro_on_either_cli_and_not_on_path_grants_nothing"
T_ENV = "p35c_the_worker_env_is_read_from_the_config_its_argv_names"
T_HOME = "p35c_the_home_is_removed_and_a_home_that_stays_is_said_once"
T_SEAT = "p35c_the_bladebro_need_is_herdrs_and_the_seat_rule_never_reads_it"
T_CLAUDE = "p35c_a_claude_browser_order_gets_bladebro_in_its_mcp_config"
T_AGY = "p35c_an_agy_browser_order_gets_bladebro_in_its_tools_folder"
T_MISSING = "p35c_bladebro_not_on_path_grants_nothing_and_says_so"
T_GONE = "p35c_the_bladebro_home_goes_with_the_orders_config"
T_SNAPSHOT = "p35c_seat_run_in_the_project_folder_writes_the_seat_snapshot"
T_LINE = "only_a_browser_order_gets_the_bladebro_line"
T_LINE_MISSING = "p35c_a_browser_order_without_bladebro_on_path_is_told_so"
T_LEFTOVERS = "p35b_the_leftover_orders_are_read_from_both_folders"
# Review round.
T_BRIEF = "a_browser_orders_brief_holds_the_line_and_a_build_orders_does_not"
T_DENIES = "p35c_a_profile_denying_bladebro_wins_over_the_grant"
T_DENIED_ARGV = "p35c_a_profile_denying_bladebro_gets_no_grant_and_no_home"
T_NATIVE = "p35c_an_agy_workspace_naming_bladebro_is_not_granted"
T_NATIVE_ARGV = "p35c_an_agy_workspace_naming_bladebro_gets_no_home"
T_AT_LAUNCH = "p35c_the_launch_grants_only_what_the_brief_was_told"
T_UNTOLD_ARGV = "p35c_a_brief_without_the_line_gets_no_grant_at_launch"
T_SEAT_READ = "p35c_the_seat_is_read_back_from_the_brief_and_the_profile_copy"

CONTROLS = (
    # Item 1, R1: decision 0043's recipe, field by field, under the order's own home.
    Control("p35c_def_no_mcp_word", BLADE,
            '        ("cmd", vec!["/c", SERVER, "mcp"])\n',
            '        ("cmd", vec!["/c", SERVER])\n', T_DEF, "rust"),
    Control("p35c_def_owners_home", BLADE,
            '        ("BLADE_HOME", crate::nirvana_agy_tools::plain_path(home)),\n',
            '        ("BLADE_HOME", "~/.blade".to_owned()),\n', T_DEF, "rust"),
    Control("p35c_def_window", BLADE,
            '        ("BLADE_CHROME_FLAGS", "--headless=new".to_owned()),\n', "", T_DEF, "rust"),
    Control("p35c_def_real_lane", BLADE,
            '        ("BLADE_LANE", "agent".to_owned()),\n', "", T_DEF, "rust"),
    Control("p35c_def_idle_default", BLADE,
            "pub const IDLE_TIMEOUT_S: u64 = 60;\n", "pub const IDLE_TIMEOUT_S: u64 = 600;\n", T_DEF, "rust"),
    Control("p35c_def_update_check", BLADE,
            '        ("BLADE_NO_UPDATE_CHECK", "1".to_owned()),\n', "", T_DEF, "rust"),
    Control("p35c_def_one_home_for_all", BLADE,
            "    run_dir.join(DIR).join(order)\n", "    run_dir.join(DIR)\n", T_DEF, "rust"),
    # Item 1, R1: the grant, on both CLIs, by whole token in any case, and not on PATH grants nothing.
    Control("p35c_grant_claude_only", BLADE,
            '    if !matches!(cli, "claude" | "agy") || !wanted(needs) {\n        Grant::No\n',
            '    if !matches!(cli, "claude") || !wanted(needs) {\n        Grant::No\n', T_GRANT, "rust"),
    Control("p35c_grant_any_cli", BLADE,
            '    if !matches!(cli, "claude" | "agy") || !wanted(needs) {\n        Grant::No\n',
            "    if !wanted(needs) {\n        Grant::No\n", T_GRANT, "rust"),
    Control("p35c_grant_off_path", BLADE,
            "    } else if on_path {\n", "    } else if true {\n", T_GRANT, "rust"),
    Control("p35c_grant_substring", BLADE,
            "            .any(|token| GRANTING.contains(&token.as_str()))\n",
            "            .any(|token| GRANTING.iter().any(|name| token.contains(name)))\n", T_GRANT, "rust"),
    Control("p35c_grant_case", BLADE,
            "            .map(str::to_ascii_lowercase)\n", "            .map(str::to_owned)\n", T_GRANT, "rust"),
    Control("p35c_grant_split", BLADE,
            "        need.split(|c: char| c == ',' || c.is_whitespace())\n",
            "        need.split(|c: char| c == ';')\n", T_GRANT, "rust"),
    Control("p35c_grant_names", BLADE,
            'pub const GRANTING: [&str; 2] = ["browser", SERVER];\n',
            'pub const GRANTING: [&str; 2] = ["browser", "browser"];\n', T_GRANT, "rust"),
    Control("p35c_grant_path_unread", BLADE,
            "        installed(),\n    )\n}\n", "        true,\n    )\n}\n", T_GRANT, "rust"),
    # Item 2: the worker's shell gets the env of the bladebro its argv's config names, on both CLIs.
    Control("p35c_env_claude_unread", BLADE,
            '        Some("claude") => after("--mcp-config"),\n', '        Some("claude") => None,\n', T_ENV, "rust"),
    Control("p35c_env_agy_unread", BLADE,
            '            after("--add-dir").map(|tools| tools.join(crate::nirvana_agy_tools::MCP_CONFIG))\n',
            "            None\n", T_ENV, "rust"),
    Control("p35c_env_other_server", BLADE,
            '    config["mcpServers"][SERVER]["env"]\n', '    config["mcpServers"]["bambu"]["env"]\n', T_ENV, "rust"),
    # Item 4: the home goes, 3 tries a second apart, then one line.
    Control("p35c_home_one_try", BLADE,
            "    for attempt in 0..tries.max(1) {\n", "    for attempt in 0..1 {\n", T_HOME, "rust"),
    Control("p35c_home_no_pause", BLADE,
            "            std::thread::sleep(pause);\n", "            let _ = pause;\n", T_HOME, "rust"),
    Control("p35c_home_silent", BLADE,
            "    if let Some(err) = last {\n", "    if let Some(err) = last.filter(|_| false) {\n", T_HOME, "rust"),
    # Review round N2: the tries and the pause are measured through `remove_home` itself, by its line and its time.
    Control("p35c_home_tries", BLADE,
            "pub const REMOVE_TRIES: u32 = 3;\n", "pub const REMOVE_TRIES: u32 = 1;\n", T_HOME, "rust"),
    Control("p35c_home_short_pause", BLADE,
            "pub const REMOVE_PAUSE: Duration = Duration::from_secs(1);\n",
            "pub const REMOVE_PAUSE: Duration = Duration::from_millis(10);\n", T_HOME, "rust"),
    # Review round N3: a home that is not there is never waited on.
    Control("p35c_home_missing_waits", BLADE,
            "            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,\n", "", T_HOME, "rust"),
    Control("p35c_home_not_removed", BLADE,
            "        match std::fs::remove_dir_all(home) {\n", "        match Err::<(), _>(std::io::Error::other(\"kept\")) {\n",
            T_HOME, "rust"),
    Control("p35c_home_stays_at_end", GUI,
            "    crate::nirvana_bladebro::remove_home(run_dir, order, out);\n", "", T_GONE, "rust"),
    Control("p35c_home_not_swept", AGY,
            "        .chain(homes)\n", "", T_LEFTOVERS, "rust"),
    Control("p35c_home_file_swept", AGY,
            "        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))\n", "", T_LEFTOVERS, "rust"),
    # R1: `bladebro` is herdr's, never the project's, and the seat rule never reads it.
    Control("p35c_project_bladebro", GUI,
            "    need == crate::nirvana_bladebro::SERVER && !capabilities.contains(&need)\n",
            "    false && need == crate::nirvana_bladebro::SERVER && !capabilities.contains(&need)\n", T_SEAT, "rust"),
    Control("p35c_seat_reads_bladebro", GUI,
            "        .filter(|need| !is_bladebro(need, &words))\n", "        .filter(|_| true)\n", T_SEAT, "rust"),
    # Item 1: claude's per-order --mcp-config, approved; agy's tools folder; the home made; no grant, today's argv.
    Control("p35c_claude_def_left_out", GUI,
            "        config[\"mcpServers\"][crate::nirvana_bladebro::SERVER] = definition;\n", "        let _ = definition;\n",
            T_CLAUDE, "rust"),
    Control("p35c_claude_unapproved", GUI,
            "        granted.push(crate::nirvana_bladebro::SERVER.to_owned());\n", "", T_CLAUDE, "rust"),
    Control("p35c_home_not_made", GUI,
            "            std::fs::create_dir_all(home)\n",
            "            std::fs::create_dir_all(home.parent().unwrap_or(home))\n", T_CLAUDE, "rust"),
    Control("p35c_grant_ignored", GUI,
            "    if servers.is_empty() && blade.is_none() {\n", "    if servers.is_empty() {\n", T_AGY, "rust"),
    Control("p35c_agy_def_left_out", GUI,
            "            chosen.insert(crate::nirvana_bladebro::SERVER.to_owned(), definition);\n",
            "            let _ = definition;\n", T_AGY, "rust"),
    Control("p35c_missing_silent", BLADE,
            '            let _ = writeln!(out, "{MISSING_LINE}");\n', "", T_MISSING, "rust"),
    Control("p35c_missing_granted", BLADE,
            "        Grant::Yes => return true,\n", "        Grant::Yes | Grant::Missing => return true,\n",
            T_MISSING, "rust"),
    # Item 3: the line goes to a worker granted bladebro, whatever its CLI, and to no one else.
    Control("p35c_line_without_grant", LEAN,
            "        crate::nirvana_bladebro::Grant::No => None,\n",
            "        crate::nirvana_bladebro::Grant::No => Some(BROWSER_LINE),\n", T_LINE, "rust"),
    Control("p35c_line_claude_cli", LEAN,
            "    line_of(crate::nirvana_bladebro::grant(cli, needs, denied, cwd))\n",
            '    line_of(crate::nirvana_bladebro::grant(if cli == "agy" { "codex" } else { cli }, needs, denied, cwd))\n',
            T_LINE, "rust"),
    Control("p35c_line_missing_unsaid", LEAN,
            "        crate::nirvana_bladebro::Grant::Missing => Some(crate::nirvana_bladebro::MISSING_LINE),\n",
            "        crate::nirvana_bladebro::Grant::Missing => None,\n", T_LINE_MISSING, "rust"),
    # Owed from the plan 35 merge: `seat run` in the project folder writes the seat snapshot (P35a's call).
    Control("p35c_snapshot_dropped", WORKER,
            "            crate::nirvana_gate::write_seat_snapshot(project, &request.run, &seated.order, &cwd)\n",
            "            Ok::<usize, String>(0)\n", T_SNAPSHOT, "rust"),
    Control("p35c_snapshot_worktree_only", WORKER,
            "    if seated.worktree.is_none() {\n", "    if seated.worktree.is_some() {\n", T_SNAPSHOT, "rust"),
    # Review round S1: only `BLADE_*` crosses into the worker's shell.
    Control("p35c_env_any_key", BLADE,
            '                .filter(|(key, _)| key.starts_with("BLADE_"))\n', "", T_ENV, "rust"),
    # Review round trace: a profile that denies bladebro whole wins over the grant, read at launch.
    Control("p35c_denial_ignored", BLADE,
            "    } else if denies(denied) {\n", "    } else if false && denies(denied) {\n", T_DENIED_ARGV, "rust"),
    Control("p35c_denies_star_only", BLADE,
            '        .any(|tool| tool == format!("mcp__{SERVER}") || tool == format!("mcp__{SERVER}__*"))\n',
            '        .any(|tool| tool == format!("mcp__{SERVER}__*"))\n', T_DENIES, "rust"),
    Control("p35c_denies_case", BLADE,
            "        .map(|tool| tool.trim().to_ascii_lowercase())\n", "        .map(|tool| tool.trim().to_owned())\n",
            T_DENIES, "rust"),
    Control("p35c_denies_split", BLADE,
            "        .flat_map(|tools| tools.split(','))\n",
            "        .flat_map(|tools| std::iter::once(tools.as_str()))\n", T_DENIES, "rust"),
    Control("p35c_denied_silent", BLADE,
            '            let _ = writeln!(out, "{DENIED_LINE}");\n', "", T_DENIED_ARGV, "rust"),
    Control("p35c_denial_unread_at_launch", GUI,
            "    let grant = crate::nirvana_bladebro::grant(cli.unwrap_or_default(), needs, &seat.denied, cwd);\n",
            "    let grant = crate::nirvana_bladebro::grant(cli.unwrap_or_default(), needs, &[], cwd);\n",
            T_DENIED_ARGV, "rust"),
    Control("p35c_line_denial_unread", LEAN,
            "    if let Some(line) = browser_line(cli, needs, denied, cwd) {\n",
            "    if let Some(line) = browser_line(cli, needs, &[], cwd) {\n", T_BRIEF, "rust"),
    # Review round S2: an agy workspace naming its own bladebro gets none of herdr's, and no home.
    Control("p35c_native_ignored", BLADE,
            '    } else if cli == "agy" && native {\n', "    } else if false && native {\n", T_NATIVE, "rust"),
    Control("p35c_native_unread", BLADE,
            '        cli == "agy" && workspace_names(cwd),\n', "        false,\n", T_NATIVE_ARGV, "rust"),
    Control("p35c_workspace_other_server", BLADE,
            "        .is_ok_and(|names| names.iter().any(|name| name == SERVER))\n", "        .is_ok()\n",
            T_NATIVE, "rust"),
    Control("p35c_native_silent", BLADE,
            '                "{} names a bladebro of its own, which agy loads anyway',
            '                "{} has one, which agy loads anyway', T_NATIVE_ARGV, "rust"),
    # Review round S3: one reading; the launch grants only what the brief was told, and says each mismatch.
    Control("p35c_untold_granted", BLADE,
            "        Grant::Yes if told == Some(false) => {\n", "        Grant::Yes if false => {\n", T_UNTOLD_ARGV, "rust"),
    Control("p35c_untold_silent", BLADE,
            '            let _ = writeln!(out, "{UNTOLD_LINE}");\n', "", T_AT_LAUNCH, "rust"),
    Control("p35c_told_silent", BLADE,
            "    if told == Some(true) {\n", "    if false {\n", T_AT_LAUNCH, "rust"),
    Control("p35c_told_unread_at_launch", GUI,
            "    let blade = crate::nirvana_bladebro::at_launch(grant, seat.told, cwd, out)\n",
            "    let blade = crate::nirvana_bladebro::at_launch(grant, None, cwd, out)\n", T_UNTOLD_ARGV, "rust"),
    Control("p35c_seat_told", BLADE,
            "            told: Some(text.contains(crate::nirvana_lean::BROWSER_LINE)),\n",
            "            told: Some(true),\n", T_SEAT_READ, "rust"),
    Control("p35c_seat_profile_unread", BLADE,
            "                    .map(|profile| profile.disallowed_tools)\n", "                    .map(|_| Vec::new())\n",
            T_SEAT_READ, "rust"),
    Control("p35c_seat_review_profile", BLADE,
            "            Some(Ok(found)) if class != crate::nirvana_review::CLASS => {\n",
            "            Some(Ok(found)) => {\n", T_SEAT_READ, "rust"),
)
