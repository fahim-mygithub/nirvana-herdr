# Plan 29 P29c: workers can reach the project's MCP servers (package `gui_tools`,
# nirvana/plans/2026-10-26-29-records/p29c.md). Controls starting with p29_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GUI = "src/nirvana_gui_tools.rs"
SEAT = "src/nirvana_seat.rs"
WORKER = "src/nirvana_worker.rs"
GATE = "src/nirvana_gate.rs"

T_NAMES = "the_server_names_are_the_keys_of_mcp_servers"
T_AMONG = "a_need_names_a_server_exactly_and_never_a_capability"
T_WHY = "only_a_claude_seat_takes_a_server_order_and_a_desktop_server_says_why"
T_PICK = "a_desktop_server_order_skips_every_agy_seat_and_is_refused_without_a_claude_seat"
T_SEAT_NEEDS = "seat_needs_takes_the_server_needs_out_and_reads_the_projects_mcp_json"
T_CONFIG = "the_order_config_holds_only_the_named_servers_copied_whole"
T_ARGV = "the_argv_keeps_strict_mcp_and_adds_the_config_and_the_servers_tools"
T_ORDER = "a_worker_gets_its_orders_config_file_and_only_a_claude_server_order_does"
T_BRIEF = "every_server_order_brief_says_never_start_a_device_and_a_desktop_one_batches"
# The review round (records/p29c.md "Review round"): the call sites through their public paths, and F2-F5, the nit.
T_SEAT_ORDER = "p29_seat_order_seats_a_server_order_on_claude_and_its_brief_never_starts_a_device"
T_PICK_ARGS = "seat_pick_reads_the_server_needs_and_takes_a_claude_seat"
T_RESEAT = "p29_a_rate_limited_server_order_is_reseated_on_claude_or_not_at_all"
T_SEND_BACK = "p29_a_send_back_seats_a_server_order_on_claude_and_a_broken_mcp_json_asks"
T_EXEC = "p29_exec_gives_the_worker_its_config_removes_it_after_and_never_starts_without_it"
T_LAUNCH = "an_order_that_cannot_get_its_server_at_launch_does_not_start"
T_REMOVE = "the_order_config_goes_when_the_worker_ends_and_a_failed_remove_says_so"
T_BROKEN = "a_broken_mcp_json_refuses_a_server_need_naming_the_file"
T_CLASH = "two_servers_whose_tools_flatten_alike_refuse_the_order"

CONTROLS = (
    # §5.1: the server names are the keys of `mcpServers`.
    Control("p29_names_wrong_key", GUI,
            '    match value.get("mcpServers") {\n',
            '    match value.get("servers") {\n', T_NAMES, "rust"),
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
            '    run_dir.join("mcp").join(format!("{order}.json"))\n',
            '    run_dir.join(format!("{order}.json"))\n', T_ORDER, "rust"),
    Control("p29_order_no_servers_line", GUI,
            '    let _ = writeln!(out, "MCP servers for this order: {}", servers.join(", "));\n',
            "", T_ORDER, "rust"),
    Control("p29_order_refused_seats_silent", GUI,
            '                "seats.toml is refused ({why}); the needs {MCP_FILE} names are read as its servers"\n',
            '                "{why}"\n', T_ORDER, "rust"),
    Control("p29_order_empty_config_written", GUI,
            "    if servers.is_empty() {\n        return Ok(argv);\n    }\n    let file = write_order_config(",
            "    let file = write_order_config(", T_ORDER, "rust"),
    # §5.3 and review F4: every server order's brief says never start a device; a desktop one batches too.
    Control("p29_brief_no_device_line", GUI,
            "    text.push_str(DEVICE_LINE);\n", "", T_BRIEF, "rust"),
    Control("p29_brief_desktop_for_all", GUI,
            "        .any(|server| DESKTOP_SERVERS.contains(&server.as_str()))\n",
            "        .any(|server| !server.is_empty())\n", T_BRIEF, "rust"),
    Control("p29_brief_without_servers", GUI,
            "    if servers.is_empty() {\n        return text;\n    }\n", "", T_BRIEF, "rust"),
    Control("p29_brief_reads_raw_needs", WORKER,
            "    text = crate::nirvana_gui_tools::with_server_lines(text, &booked.servers);\n",
            "    text = crate::nirvana_gui_tools::with_server_lines(text, &request.needs);\n", T_SEAT_ORDER, "rust"),
    Control("p29_book_drops_servers", WORKER,
            "    booked.servers = needs.servers;\n", "", T_SEAT_ORDER, "rust"),
    # Review F1: each call site that builds a seat request takes a claude seat, driven through its public path.
    Control("p29_book_drops_claude_only", WORKER,
            "        pick_request.claude_only = needs.claude_only.clone();\n", "", T_SEAT_ORDER, "rust"),
    Control("p29_seat_pick_drops_claude_only", SEAT,
            "    (picked.request.needs, picked.request.claude_only) = (needs.rest, needs.claude_only);\n",
            "    (picked.request.needs, picked.request.claude_only) = (needs.rest, None);\n", T_PICK_ARGS, "rust"),
    Control("p29_reseat_drops_claude_only", WORKER,
            "        (request.needs, request.claude_only) = (needs.rest.clone(), needs.claude_only.clone());\n",
            "        request.needs = needs.rest.clone();\n", T_RESEAT, "rust"),
    Control("p29_send_back_drops_claude_only", GATE,
            "    (request.needs, request.claude_only) = (needs.rest, needs.claude_only);\n",
            "    request.needs = needs.rest;\n", T_SEND_BACK, "rust"),
    Control("p29_exec_no_servers", WORKER,
            "        .and_then(|argv| with_order_servers(argv, project, run, &seated, out))\n", "", T_EXEC, "rust"),
    # Review F3: an order that cannot get its server at launch never starts without it.
    Control("p29_exec_starts_without", WORKER,
            "        .and_then(|argv| with_order_servers(argv, project, run, &seated, out))\n",
            "        .map(|argv| with_order_servers(argv.clone(), project, run, &seated, out).unwrap_or(argv))\n",
            T_EXEC, "rust"),
    Control("p29_launch_keeps_gone_server", GUI,
            "    if let Some(gone) = servers.iter().find(|server| !names.contains(server)) {\n",
            "    if let Some(gone) = servers.iter().find(|_| false) {\n", T_LAUNCH, "rust"),
    # Review F2: the config goes when the worker ends, delivered or failed; a remove that fails says so.
    Control("p29_exec_keeps_config", WORKER,
            "        crate::nirvana_gui_tools::remove_order_config(&run_dir, order, out);\n", "", T_EXEC, "rust"),
    Control("p29_remove_nothing", GUI,
            "    match std::fs::remove_file(&file) {\n", "    match Ok::<(), std::io::Error>(()) {\n",
            T_REMOVE, "rust"),
    Control("p29_remove_silent", GUI,
            "        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {\n",
            "        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {\n", T_REMOVE, "rust"),
    # Review F5: a `.mcp.json` that does not parse refuses a server need by the file's name, and nothing else.
    Control("p29_broken_reads_unknown", GUI,
            '                Some(need) => Err(format!(\n                    "{why}, so the need {need} names none of its servers"\n                )),\n',
            "                Some(_) => Ok(Needs { rest: needs.to_vec(), ..Needs::default() }),\n", T_BROKEN, "rust"),
    Control("p29_broken_refuses_all", GUI,
            "            return match needs.iter().find(|need| !words.contains(&need.as_str())) {\n",
            "            return match needs.iter().find(|_| true) {\n", T_BROKEN, "rust"),
    # The nit: two servers whose tool names flatten alike refuse the order, at seat time and at launch.
    Control("p29_clash_ignored", GUI,
            "    if let Some(why) = key_clash(&named, &servers) {\n",
            "    if let Some(why) = key_clash(&named, &[]) {\n", T_CLASH, "rust"),
    Control("p29_clash_with_itself", GUI,
            "            .find(|other| *other != server && crate::nirvana_preload::tool_server_key(other) == key)\n",
            "            .find(|other| crate::nirvana_preload::tool_server_key(other) == key)\n", T_CLASH, "rust"),
    Control("p29_clash_at_launch_ignored", GUI,
            "    if let Some(why) = key_clash(servers, &names) {\n",
            "    if let Some(why) = key_clash(servers, &[]) {\n", T_CLASH, "rust"),
)
