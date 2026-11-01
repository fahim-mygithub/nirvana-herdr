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
            "    grant_with(cli, needs, installed())\n", "    grant_with(cli, needs, true)\n", T_GRANT, "rust"),
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
    Control("p35c_home_tries", BLADE,
            "pub const REMOVE_TRIES: u32 = 3;\n", "pub const REMOVE_TRIES: u32 = 1;\n", T_HOME, "rust"),
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
    Control("p35c_missing_silent", GUI,
            '            let _ = writeln!(out, "{}", crate::nirvana_bladebro::MISSING_LINE);\n', "", T_MISSING, "rust"),
    Control("p35c_missing_granted", GUI,
            "    let blade = match crate::nirvana_bladebro::grant(cli.unwrap_or_default(), needs) {\n",
            "    let blade = match crate::nirvana_bladebro::grant_with(cli.unwrap_or_default(), needs, true) {\n",
            T_MISSING, "rust"),
    # Item 3: the line goes to a worker granted bladebro, whatever its CLI, and to no one else.
    Control("p35c_line_without_grant", LEAN,
            "        crate::nirvana_bladebro::Grant::No => None,\n",
            "        crate::nirvana_bladebro::Grant::No => Some(BROWSER_LINE),\n", T_LINE, "rust"),
    Control("p35c_line_claude_cli", LEAN,
            "    match crate::nirvana_bladebro::grant(cli, needs) {\n",
            '    match crate::nirvana_bladebro::grant(if cli == "agy" { "codex" } else { cli }, needs) {\n',
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
)
