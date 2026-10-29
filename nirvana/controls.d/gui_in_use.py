# Plan 31 P31a: GUI orders reach their servers, the Send rule, late deliveries (package `gui_in_use`,
# nirvana/plans/2026-10-29-31-records/p31a.md). Controls starting with p31_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GUI = "src/nirvana_gui_tools.rs"
FRESH = "src/nirvana_fresh.rs"
WORKER = "src/nirvana_worker.rs"
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
PANE = "src/cli/pane.rs"

T_LINES = "p31_a_send_ok_order_gets_the_send_line_in_place_of_the_device_line"
T_HINT = "p31_the_hint_names_the_server_when_the_order_names_its_app_and_needs_none"
T_QUOTE = "p31_a_send_ok_quote_is_found_only_in_the_owners_own_prompts"
T_VERIFY = "p31_verify_send_ok_reads_the_recorded_session_and_refuses_every_doubt"
T_SEAT = "p31_seat_run_records_a_verified_send_quote_and_refuses_one_it_cannot_find"
T_SEAT_HINT = "p31_seat_run_warns_when_an_order_names_studio_and_needs_no_server"
T_FINISHED = "p31_a_finished_workers_pane_names_its_order_and_a_live_one_does_not"
T_CLI = "p31_send_text_to_a_finished_workers_pane_types_nothing"
T_WAKE = "p31_a_queued_wake_records_its_reason_once_per_new_event"
T_DEDUPE = "p31_queued_event_dedupes_only_the_same_reason_since_the_last_wake"
T_SEND_BACK = "p31_a_send_back_child_keeps_the_device_line_and_the_parents_send_line"
T_CONTINUATION = "p31_a_reseats_continuation_brief_keeps_the_device_line_and_the_send_line"
T_RESEAT = "p31_a_rate_limited_reseat_keeps_the_parents_send_approval_and_its_lines"

CONTROLS = (
    # §2.2: a verified quote puts the Send line in place of the device line; a desktop order gets its own brief line.
    Control("p31_lines_no_send_line", GUI,
            "    text.push_str(\"\\n\\n\");\n    text.push_str(SEND_LINE);\n",
            "    text.push_str(\"\\n\\n\");\n", T_LINES, "rust"),
    Control("p31_lines_send_keeps_device", GUI,
            "    if !send_ok {\n        return with_server_lines(text, servers);\n    }\n",
            "    if !send_ok || servers.len() < 9 {\n        return with_server_lines(text, servers);\n    }\n",
            T_LINES, "rust"),
    Control("p31_lines_desktop_says_never_start", GUI,
            "        text.push_str(DESKTOP_SEND_LINE);\n", "        text.push_str(BRIEF_LINE);\n", T_LINES, "rust"),
    # §2.1: the hint names the server when the order names its app, its needs name no server, and the project has it.
    Control("p31_hint_with_granted_server", GUI,
            "    if !granted.is_empty() {\n        return None;\n    }\n    let lower",
            "    let lower", T_HINT, "rust"),
    Control("p31_hint_case_sensitive", GUI,
            "defined.iter().any(|name| name == server) && lower.contains(&app.to_lowercase())",
            "defined.iter().any(|name| name == server) && text.contains(app)", T_HINT, "rust"),
    Control("p31_hint_server_not_defined", GUI,
            "defined.iter().any(|name| name == server) && lower.contains(&app.to_lowercase())",
            "!defined.is_empty() && lower.contains(&app.to_lowercase())", T_HINT, "rust"),
    Control("p31_hint_names_the_app", GUI,
            "add `--needs {server}` if its worker", "add `--needs {app}` if its worker", T_HINT, "rust"),
    # §2.2: the quote is the owner's words, trimmed, never empty, verbatim, from their own prompts only.
    Control("p31_quote_empty_accepted", GUI,
            "    if quote.is_empty() {\n        return Err(\"--send-ok needs the owner's words\".into());\n    }\n",
            "", T_QUOTE, "rust"),
    Control("p31_quote_untrimmed", GUI,
            "    let quote = quote.trim();\n    if quote.is_empty() {\n",
            "    if quote.is_empty() {\n", T_QUOTE, "rust"),
    Control("p31_quote_counts_bang_lines", FRESH,
            "        if bang_line(&value) || !owner_prompt_in(&value)?.contains(quote) {\n",
            "        if !owner_prompt_in(&value)?.contains(quote) {\n", T_QUOTE, "rust"),
    Control("p31_quote_any_line", FRESH,
            "        if bang_line(&value) || !owner_prompt_in(&value)?.contains(quote) {\n",
            "        if !value.to_string().contains(quote) {\n", T_QUOTE, "rust"),
    Control("p31_quote_ignores_case", FRESH,
            "        if bang_line(&value) || !owner_prompt_in(&value)?.contains(quote) {\n",
            "        if bang_line(&value) || !owner_prompt_in(&value)?.to_lowercase().contains(&quote.to_lowercase()) {\n",
            T_QUOTE, "rust"),
    # §2.2: the transcript is the one recorded for the folder; a missing record is named as such.
    Control("p31_verify_wrong_folder", GUI,
            "    let folder = crate::nirvana_session_record::absolute_folder(folder);\n",
            "    let folder = folder.join(\"elsewhere\");\n", T_VERIFY, "rust"),
    Control("p31_verify_no_record_named", GUI,
            "    let record = crate::nirvana_session_record::read(home, &folder).ok_or_else(|| {\n",
            "    let record = crate::nirvana_session_record::read(home, &folder).or_else(|| Some(crate::nirvana_session_record::Record { session_id: String::new(), at: String::new(), transcript: String::new() })).ok_or_else(|| {\n",
            T_VERIFY, "rust"),
    # §2.2: `seat run` refuses an unfound quote before booking, journals the check, and writes the Send line.
    Control("p31_seat_unfound_quote_seats", WORKER,
            "            .map_err(|why| (2, why))?,\n        ),\n        None => None,\n    };\n",
            "            .unwrap_or_else(|_| crate::nirvana_gui_tools::SendOk { quote: quote.clone(), check: String::new() }),\n        ),\n        None => None,\n    };\n",
            T_SEAT, "rust"),
    Control("p31_seat_quote_not_journaled", WORKER,
            "        jev_pick: booked.jev_pick.clone(),\n        send_ok,\n",
            "        jev_pick: booked.jev_pick.clone(),\n        send_ok: None,\n", T_SEAT, "rust"),
    Control("p31_seat_brief_without_send", WORKER,
            "    text = crate::nirvana_gui_tools::with_brief_lines(text, &booked.servers, send_ok.is_some());\n",
            "    text = crate::nirvana_gui_tools::with_brief_lines(text, &booked.servers, false);\n", T_SEAT, "rust"),
    Control("p31_seat_wrong_session_folder", WORKER,
            "        .map_or_else(|| project.root.clone(), PathBuf::from)\n",
            "        .map_or_else(|| project.root.clone(), |dir| PathBuf::from(dir).join(\"elsewhere\"))\n",
            T_SEAT, "rust"),
    # §2.1: `seat run` gives the warning, reading the needs `book` granted as servers.
    Control("p31_seat_no_warning", WORKER, "        \"warning\": warning,\n", "        \"warning\": null,\n",
            T_SEAT_HINT, "rust"),
    Control("p31_seat_hint_ignores_granted", WORKER,
            "        &crate::nirvana_gui_tools::project_servers(&project.root).unwrap_or_default(),\n        &booked.servers,\n",
            "        &crate::nirvana_gui_tools::project_servers(&project.root).unwrap_or_default(),\n        &[],\n",
            T_SEAT_HINT, "rust"),
    # §2.4: the pane's last worker, the one that ended, and only in that pane.
    Control("p31_finished_any_pane", WORKER,
            "                } if at == pane => Some((run, events, event, order)),\n",
            "                } if !at.is_empty() => Some((run, events, event, order)),\n", T_FINISHED, "rust"),
    Control("p31_finished_first_worker", WORKER,
            "        .max_by_key(|(_, _, event, _)| (event.at, event.seq))\n",
            "        .min_by_key(|(_, _, event, _)| (event.at, event.seq))\n", T_FINISHED, "rust"),
    Control("p31_finished_ignores_restart", WORKER,
            "        } if done == order && event.seq > seq => Some((event.at, outcome)),\n",
            "        } if done == order => Some((event.at, outcome)),\n", T_FINISHED, "rust"),
    # §2.4: the CLI's send-text and send-keys type nothing into such a pane.
    Control("p31_cli_guard_types_anyway", PANE,
            "            eprintln!(\"herdr pane {name}: {why}\");\n            return Ok(1);\n",
            "            eprintln!(\"herdr pane {name}: {why}\");\n", T_CLI, "rust"),
    Control("p31_cli_guard_skips_send_keys", PANE,
            "    | Method::PaneSendKeys(crate::api::schema::panes::PaneSendKeysParams { pane_id, .. }) =\n",
            " =\n", T_CLI, "rust"),
    # §2.3: a queued wake records its reason, once until another event comes.
    Control("p31_wake_queue_unrecorded", RUN,
            "        Wake::Queue(why) => {\n            record_queued(&journal, &why);\n",
            "        Wake::Queue(why) => {\n", T_WAKE, "rust"),
    Control("p31_wake_record_repeats", RUN,
            "    (!repeated).then(|| Kind::WakeQueued {\n",
            "    (repeated || !why.is_empty()).then(|| Kind::WakeQueued {\n", T_DEDUPE, "rust"),
    Control("p31_wake_through_counts_itself", RUN,
            "        .find(|event| !matches!(event.kind, Kind::WakeQueued { .. }))\n",
            "        .find(|_| true)\n", T_DEDUPE, "rust"),
    Control("p31_wake_dedupe_reaches_past_news", RUN,
            "        .take_while(|event| matches!(event.kind, Kind::WakeQueued { .. }))\n",
            "        .take_while(|_| true)\n", T_WAKE, "rust"),
    # §2.5: a send-back child and a reseat keep the parent's approval and its order text with its lines.
    Control("p31_send_back_drops_approval", GATE,
            "        send_ok: seated.send_ok.clone(),\n", "        send_ok: None,\n", T_SEND_BACK, "rust"),
    Control("p31_send_back_drops_original", GATE,
            "    brief.push_str(&original(root_text));\n", "", T_SEND_BACK, "rust"),
    Control("p31_continuation_drops_original", WORKER,
            "    brief.push_str(&original);\n    write_order(project, Path::new(&seated.cwd), &child, &brief)\n",
            "    write_order(project, Path::new(&seated.cwd), &child, &brief)\n", T_CONTINUATION, "rust"),
    Control("p31_reseat_drops_approval", WORKER,
            "                send_ok: seated.send_ok.clone(),\n", "                send_ok: None,\n", T_RESEAT, "rust"),
)
