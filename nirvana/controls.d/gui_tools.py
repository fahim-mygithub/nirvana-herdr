# Plan 29 P29c: workers can reach the project's MCP servers (package `gui_tools`,
# nirvana/plans/2026-10-26-29-records/p29c.md). Controls starting with p29_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GUI = "src/nirvana_gui_tools.rs"
SEAT = "src/nirvana_seat.rs"

T_NAMES = "the_server_names_are_the_keys_of_mcp_servers"
T_AMONG = "a_need_names_a_server_exactly_and_never_a_capability"
T_WHY = "only_a_claude_seat_takes_a_server_order_and_a_desktop_server_says_why"
T_PICK = "a_desktop_server_order_skips_every_agy_seat_and_is_refused_without_a_claude_seat"
T_SEAT_NEEDS = "seat_needs_takes_the_server_needs_out_and_reads_the_projects_mcp_json"
T_CONFIG = "the_order_config_holds_only_the_named_servers_copied_whole"
T_ARGV = "the_argv_keeps_strict_mcp_and_adds_the_config_and_the_servers_tools"
T_ORDER = "a_worker_gets_its_orders_config_file_and_any_error_keeps_todays_argv"
T_BRIEF = "a_desktop_order_brief_gets_the_one_line"

CONTROLS = (
    # §5.1: the server names are the keys of `mcpServers`.
    Control("p29_names_wrong_key", GUI,
            '    let servers = value\n        .get("mcpServers")\n',
            '    let servers = value\n        .get("servers")\n', T_NAMES, "rust"),
    # §5.1: names match exactly; a capability stays a capability; once each.
    Control("p29_among_ignores_case", GUI,
            "        if !capabilities.contains(&need.as_str()) && servers.contains(need) && !named.contains(need)\n",
            "        if !capabilities.contains(&need.as_str()) && servers.iter().any(|s| s.eq_ignore_ascii_case(need.trim())) && !named.contains(need)\n",
            T_AMONG, "rust"),
    Control("p29_among_takes_capabilities", GUI,
            "        if !capabilities.contains(&need.as_str()) && servers.contains(need) && !named.contains(need)\n",
            "        if servers.contains(need) && !named.contains(need)\n", T_AMONG, "rust"),
    Control("p29_among_repeats", GUI,
            "        if !capabilities.contains(&need.as_str()) && servers.contains(need) && !named.contains(need)\n",
            "        if !capabilities.contains(&need.as_str()) && servers.contains(need)\n", T_AMONG, "rust"),
    # §5.2: a server order is claude-only, and a desktop server names the window.
    Control("p29_why_none_for_servers", GUI,
            "    if servers.is_empty() {\n        return None;\n    }\n",
            "    if !servers.is_empty() {\n        return None;\n    }\n", T_WHY, "rust"),
    Control("p29_why_no_desktop", GUI,
            "            .find(|server| DESKTOP_SERVERS.contains(&server.as_str()))\n",
            "            .find(|server| server.is_empty())\n", T_WHY, "rust"),
    # §5.2: the seat rule skips every non-claude seat for such an order; no claude seat refuses it with the reason.
    Control("p29_pick_ignores_claude_only", SEAT,
            '        .filter(|_| seat.cli != "claude")\n',
            '        .filter(|_| seat.cli == "nobody")\n',
            T_PICK, "rust"),
    # §5.1: the seat rule reads needs with the server needs taken out, from the project's `.mcp.json`.
    Control("p29_seat_needs_keeps_servers", GUI,
            "        .filter(|need| !named.contains(need))\n",
            "        .filter(|_| true)\n", T_SEAT_NEEDS, "rust"),
    Control("p29_seat_needs_wrong_file", GUI,
            "    let path = root.join(MCP_FILE);\n    match std::fs::read_to_string(&path) {\n",
            '    let path = root.join("mcp.json");\n    match std::fs::read_to_string(&path) {\n',
            T_SEAT_NEEDS, "rust"),
    # §5.1: the per-order config holds only those servers, copied whole.
    Control("p29_config_all_servers", GUI,
            "        chosen.insert(server.clone(), definition.clone());\n    }\n    Ok(serde_json::json!({ \"mcpServers\": chosen }))\n",
            "        chosen.insert(server.clone(), definition.clone());\n    }\n    let _ = chosen;\n    Ok(serde_json::json!({ \"mcpServers\": defined }))\n",
            T_CONFIG, "rust"),
    Control("p29_config_command_only", GUI,
            "        chosen.insert(server.clone(), definition.clone());\n",
            '        chosen.insert(server.clone(), serde_json::json!({"command": definition["command"]}));\n',
            T_CONFIG, "rust"),
    Control("p29_config_missing_skipped", GUI,
            '            .ok_or_else(|| format!("{MCP_FILE} has no server {server:?}"))?;\n',
            "            .unwrap_or(&Value::Null);\n", T_CONFIG, "rust"),
    # §5.1: `--mcp-config <file>` and one `--allowedTools mcp__<server>__*` list.
    Control("p29_argv_no_wildcard", GUI,
            '                "mcp__{}__*",\n', '                "mcp__{}",\n', T_ARGV, "rust"),
    Control("p29_argv_raw_name", GUI,
            "                crate::nirvana_preload::tool_server_key(server)\n",
            "                server\n", T_ARGV, "rust"),
    Control("p29_argv_no_config", GUI,
            '        "--mcp-config".to_owned(),\n', '        "--mcp-configs".to_owned(),\n', T_ARGV, "rust"),
    # §5.1 and §5.4: the worker's file under the run, its argv, the line; agy, no server, or an error: today's argv.
    Control("p29_order_agy_too", GUI,
            '    if needs.is_empty() || argv.first().map(String::as_str) != Some("claude") {\n',
            "    if needs.is_empty() {\n", T_ORDER, "rust"),
    Control("p29_order_wrong_dir", GUI,
            '    let dir = run_dir.join("mcp");\n', '    let dir = run_dir.to_path_buf();\n', T_ORDER, "rust"),
    Control("p29_order_no_servers_line", GUI,
            '            let _ = writeln!(out, "MCP servers for this order: {}", servers.join(", "));\n',
            "", T_ORDER, "rust"),
    Control("p29_order_error_silent", GUI,
            '            let _ = writeln!(out, "the worker starts without its MCP servers: {why}");\n',
            "            let _ = why;\n", T_ORDER, "rust"),
    Control("p29_order_empty_config_written", GUI,
            "    if servers.is_empty() {\n        return Ok(None);\n    }\n    let config = order_config(&text, &servers)?;\n",
            "    let config = order_config(&text, &servers)?;\n", T_ORDER, "rust"),
    # §5.3: a desktop order's brief ends with the one line; any other is unchanged.
    Control("p29_brief_every_order", GUI,
            "        .any(|need| DESKTOP_SERVERS.contains(&need.as_str()))\n",
            "        .any(|need| !need.is_empty())\n", T_BRIEF, "rust"),
    Control("p29_brief_no_gap", GUI,
            '        text.push_str("\\n\\n");\n        text.push_str(BRIEF_LINE);\n',
            '        text.push_str(BRIEF_LINE);\n', T_BRIEF, "rust"),
)
