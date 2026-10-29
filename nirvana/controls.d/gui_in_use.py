# Plan 31 P31a: GUI orders reach their servers, the Send rule, late deliveries (package `gui_in_use`,
# nirvana/plans/2026-10-29-31-records/p31a.md). Controls starting with p31_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GUI = "src/nirvana_gui_tools.rs"
FRESH = "src/nirvana_fresh.rs"
WORKER = "src/nirvana_worker.rs"
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
PANE = "src/cli/pane.rs"
API = "src/app/api/panes.rs"
ACCOUNTS = "src/nirvana_accounts.rs"

T_LINES = "p31_a_send_ok_order_gets_the_send_line_in_place_of_the_device_line"
T_HINT = "p31_the_hint_names_the_server_when_the_order_names_its_app_and_needs_none"
T_STUDIO = "p31_a_bare_studio_names_the_one_desktop_server"
T_QUOTE = "p31_a_send_ok_quote_is_found_only_in_the_owners_own_prompts"
T_SHORT = "p31_a_short_quote_refuses_though_the_owner_typed_it"
T_AGE = "p31_a_seven_hour_old_prompt_refuses"
T_RELAY = "p31_a_relayed_message_holding_the_quote_refuses"
T_VERIFY = "p31_verify_send_ok_reads_the_recorded_session_and_refuses_every_doubt"
T_SEAT = "p31_seat_run_records_a_verified_send_quote_and_refuses_one_it_cannot_find"
T_SEAT_HINT = "p31_seat_run_warns_when_an_order_names_studio_and_needs_no_server"
T_FINISHED = "p31_a_finished_workers_pane_names_its_order_and_a_live_one_does_not"
T_PANE_LIVE = "p31_a_pane_a_live_supervisor_names_is_never_finished"
T_API = "p31_the_server_types_nothing_into_a_finished_workers_pane"
T_SELF = "p31_herdrs_own_send_and_force_pass_the_guard"
T_ACCOUNTS = "p31_herdrs_own_cli_calls_carry_the_self_send_escape"
T_WAKE = "p31_a_queued_wake_records_its_reason_once_per_new_event"
T_DEDUPE = "p31_queued_event_dedupes_only_the_same_reason_since_the_last_wake"
T_OWNER_TYPING = "a_wake_never_types_while_the_owner_does"
T_RETRY = "p31_a_queued_wake_is_delivered_by_the_retry_once_the_owner_goes_quiet"
T_NOTICE = "p31_a_notice_that_reached_the_hook_first_is_not_typed_again"
T_BACKOFF = "p31_the_retry_backs_off_from_30_s_to_5_minutes"
T_SEND_BACK = "p31_a_send_back_child_never_inherits_the_send_approval"
T_CONTINUATION = "p31_a_reseats_continuation_brief_keeps_the_device_line_and_drops_the_send_line"
T_RESEAT = "p31_a_rate_limited_reseat_never_inherits_the_send_approval"
T_CHILD = "p31_a_child_brief_never_carries_the_send_line"

CONTROLS = (
    # §2.2: a verified quote puts the Send line in place of the device line; a desktop order gets its own brief line.
    Control("p31_lines_no_send_line", GUI,
            "    text.push_str(\"\\n\\n\");\n    text.push_str(SEND_LINE);\n",
            "    text.push_str(\"\\n\\n\");\n", T_LINES, "rust"),
    Control("p31_lines_send_keeps_device", GUI,
            "    if !send_ok || servers.is_empty() {\n",
            "    if !send_ok || servers.len() < 9 {\n", T_LINES, "rust"),
    # Review N3: no server, no Send line.
    Control("p31_lines_send_without_server", GUI,
            "    if !send_ok || servers.is_empty() {\n", "    if !send_ok {\n", T_LINES, "rust"),
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
    # Review round: a bare "Studio" is the app of the one desktop server the project defines, and only then.
    Control("p31_studio_not_read", GUI,
            "        .or_else(bare_studio)?;\n", "        .or_else(|| bare_studio().filter(|_| false))?;\n",
            T_STUDIO, "rust"),
    Control("p31_studio_with_two_servers", GUI,
            "        let [only] = desktop[..] else { return None };\n",
            "        let [only, ..] = desktop[..] else { return None };\n", T_STUDIO, "rust"),
    Control("p31_studio_inside_a_word", GUI,
            "            .any(|word| word == \"studio\")\n", "            .any(|word| word.contains(\"studio\"))\n",
            T_STUDIO, "rust"),
    # §2.2 and review B1: the quote is the owner's words, trimmed, verbatim, long enough, recent, from their own
    # prompts only (told by `origin`), with no relayed text anywhere in them.
    Control("p31_quote_untrimmed", GUI,
            "    let quote = quote.trim();\n    if quote.chars()", "    if quote.chars()", T_QUOTE, "rust"),
    Control("p31_quote_any_origin", FRESH,
            "    if value.pointer(\"/origin/kind\").and_then(Value::as_str) != Some(\"human\") {\n        return None;\n    }\n",
            "", T_QUOTE, "rust"),
    Control("p31_quote_flags_ignored", FRESH,
            "            .is_some_and(|set| *set != Value::Bool(false))\n",
            "            .is_some_and(|_| false)\n", T_QUOTE, "rust"),
    Control("p31_quote_tool_use_result", FRESH,
            "    if flagged || value.get(\"toolUseResult\").is_some() || value.get(\"sourceToolUseID\").is_some() {\n",
            "    if flagged || value.get(\"sourceToolUseID\").is_some() {\n", T_QUOTE, "rust"),
    Control("p31_quote_source_tool_use", FRESH,
            "    if flagged || value.get(\"toolUseResult\").is_some() || value.get(\"sourceToolUseID\").is_some() {\n",
            "    if flagged || value.get(\"toolUseResult\").is_some() {\n", T_QUOTE, "rust"),
    Control("p31_quote_herdr_digest", FRESH,
            "        return None;\n    }\n    owner_prompt_in(value)\n}\n",
            "        return None;\n    }\n    Some(texts.join(\"\\n\"))\n}\n", T_QUOTE, "rust"),
    Control("p31_quote_ignores_case", FRESH,
            "        if !prompt.contains(quote) {\n",
            "        if !prompt.to_lowercase().contains(&quote.to_lowercase()) {\n", T_QUOTE, "rust"),
    Control("p31_quote_short_chars", GUI,
            "    if quote.chars().count() < QUOTE_MIN_CHARS || quote.split_whitespace()",
            "    if quote.split_whitespace()", T_SHORT, "rust"),
    Control("p31_quote_short_words", GUI,
            " || quote.split_whitespace().count() < QUOTE_MIN_WORDS\n", "\n", T_SHORT, "rust"),
    Control("p31_quote_any_age", GUI,
            "    if age > QUOTE_MAX_AGE_S {\n", "    if age > u64::MAX - 1 {\n", T_AGE, "rust"),
    Control("p31_quote_age_edge", GUI,
            "    if age > QUOTE_MAX_AGE_S {\n", "    if age >= QUOTE_MAX_AGE_S {\n", T_AGE, "rust"),
    Control("p31_quote_untimed", FRESH,
            "            at_s: crate::nirvana_usage::rfc3339(at)?,\n",
            "            at_s: crate::nirvana_usage::rfc3339(at).unwrap_or(u64::MAX),\n", T_AGE, "rust"),
    Control("p31_relay_at_start_only", FRESH,
            "        .any(|text| RELAYED_MARKS.iter().any(|mark| text.contains(mark)))\n",
            "        .any(|text| RELAYED_MARKS.iter().any(|mark| text.starts_with(mark)))\n", T_RELAY, "rust"),
    Control("p31_relay_teammate", FRESH,
            "    \"<teammate-message\",\n", "    \"<teammate-message-x\",\n", T_RELAY, "rust"),
    Control("p31_relay_cross_session", FRESH,
            "    \"<cross-session-message\",\n", "    \"<cross-session-message-x\",\n", T_RELAY, "rust"),
    Control("p31_relay_artifact_comment", FRESH,
            "    \"[Artifact comment sent to Claude]\",\n", "    \"[Artifact comment x]\",\n", T_RELAY, "rust"),
    Control("p31_relay_pasted", FRESH,
            "    \"<pasted_content\",\n", "    \"<pasted_content-x\",\n", T_RELAY, "rust"),
    # §2.2: the transcript is the one recorded for the folder; a missing record is named as such.
    Control("p31_verify_wrong_folder", GUI,
            "    let folder = crate::nirvana_session_record::absolute_folder(folder);\n",
            "    let folder = folder.join(\"elsewhere\");\n", T_VERIFY, "rust"),
    Control("p31_verify_no_record_named", GUI,
            "    let record = crate::nirvana_session_record::read(home, &folder).ok_or_else(|| {\n",
            "    let record = crate::nirvana_session_record::read(home, &folder).or_else(|| Some(crate::nirvana_session_record::Record { session_id: String::new(), at: String::new(), transcript: String::new() })).ok_or_else(|| {\n",
            T_VERIFY, "rust"),
    # §2.2: `seat run` refuses an unfound quote before booking, journals the check, prints it, and writes the Send line.
    Control("p31_seat_unfound_quote_seats", WORKER,
            "                .map_err(|why| (2, why))?,\n            )\n        }\n        None => None,\n",
            "                .unwrap_or_else(|_| crate::nirvana_gui_tools::SendOk { quote: quote.clone(), prompt: String::new(), at: String::new(), transcript: String::new(), check: String::new() }),\n            )\n        }\n        None => None,\n",
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
    # Review N5: `CLAUDE_PROJECT_DIR` wins over the project's root.
    Control("p31_seat_root_wins", WORKER,
            "    std::env::var_os(crate::nirvana_session_record::PROJECT_DIR_ENV)\n        .filter(|dir| !dir.is_empty())\n",
            "    None::<std::ffi::OsString>\n        .filter(|dir| !dir.is_empty())\n", T_SEAT, "rust"),
    # Review N3: an approval for an order granted no device server is refused.
    Control("p31_seat_send_ok_without_server", WORKER,
            "            if !request.needs.iter().any(|need| defined.contains(need)) {\n",
            "            if defined.is_empty() {\n", T_SEAT, "rust"),
    # Review B1.4: `seat run` gives back the prompt, its time and the session file.
    Control("p31_seat_prints_no_prompt", WORKER,
            "        \"send_ok\": seated.send_ok,\n", "        \"send_ok\": null,\n", T_SEAT, "rust"),
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
    # Review S4: a live order, or a pane a live supervisor names, is never finished.
    Control("p31_finished_ignores_live", WORKER,
            "    if live(run, order) {\n        return None;\n    }\n", "", T_FINISHED, "rust"),
    Control("p31_finished_ignores_pane_lock", WORKER,
            "    if held {\n        return None;\n    }\n", "", T_PANE_LIVE, "rust"),
    Control("p31_finished_pane_unnamed", WORKER,
            "        live_dir(project, run).join(format!(\"{order}.{PANE_EXT}\")),\n",
            "        live_dir(project, run).join(format!(\"{order}.unnamed\")),\n", T_PANE_LIVE, "rust"),
    Control("p31_finished_no_index", WORKER,
            "        let _ = std::fs::write(path, project.root.display().to_string());\n", "", T_PANE_LIVE, "rust"),
    # Review S3: the server, where the socket's sends are served, types nothing into such a pane; `force` passes.
    Control("p31_api_text_unguarded", API,
            "            return encode_error(id, \"finished_worker\", why);\n        }\n        let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {\n            return pane_not_found(id, &params.pane_id);\n        };\n        if let Err(err)",
            "            let _ = why;\n        }\n        let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {\n            return pane_not_found(id, &params.pane_id);\n        };\n        if let Err(err)",
            T_API, "rust"),
    Control("p31_api_keys_unguarded", API,
            "            return encode_error(id, \"finished_worker\", why);\n        }\n        let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {\n            return pane_not_found(id, &params.pane_id);\n        };\n        let encoded_keys",
            "            let _ = why;\n        }\n        let Some(runtime) = self.lookup_runtime_sender(ws_idx, pane_id) else {\n            return pane_not_found(id, &params.pane_id);\n        };\n        let encoded_keys",
            T_API, "rust"),
    Control("p31_api_force_ignored", API,
            "        if force {\n            return None;\n        }\n        let public", "        let public", T_API, "rust"),
    # Review S2: herdr's own calls pass the escape, and the owner's `--force` sets it.
    Control("p31_cli_self_send_ignored", PANE,
            "        *force |= from_herdr;\n", "        *force |= false && from_herdr;\n", T_SELF, "rust"),
    Control("p31_accounts_no_escape", ACCOUNTS,
            "        .env(crate::nirvana_worker::SELF_SEND_ENV, \"1\")\n", "", T_ACCOUNTS, "rust"),
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
    # Review N1: the reason holds no count of seconds.
    Control("p31_wake_reason_counts_seconds", RUN,
            "                \"the owner typed in the pane in the last {OWNER_QUIET_S} s\"\n",
            "                \"the owner typed in the pane {since} s ago\"\n", T_OWNER_TYPING, "rust"),
    # Review W: a queued wake is retried with a back-off until delivered, and delivered once.
    Control("p31_retry_never_scheduled", RUN,
            "    if queued {\n        schedule_retry(home.as_deref(), project, run, now());\n",
            "    if false {\n        schedule_retry(home.as_deref(), project, run, now());\n", T_RETRY, "rust"),
    Control("p31_retry_not_cleared", RUN,
            "    } else {\n        clear_retry(home.as_deref(), project, run);\n    }\n    Ok((verdict, typed))\n",
            "    }\n    Ok((verdict, typed))\n", T_RETRY, "rust"),
    Control("p31_retry_resets_tries", RUN,
            "    if path.exists() {\n        return;\n    }\n    let retry = WakeRetry {\n",
            "    let retry = WakeRetry {\n", T_RETRY, "rust"),
    Control("p31_retry_tries_not_counted", RUN,
            "        retry.tries += 1;\n", "", T_RETRY, "rust"),
    Control("p31_retry_due_not_moved", RUN,
            "        retry.due = at + retry_backoff(retry.tries);\n", "        retry.due = at;\n", T_RETRY, "rust"),
    Control("p31_retry_notice_keeps_it", RUN,
            "        clear_retry(crate::nirvana::home().as_deref(), project, &run);\n", "", T_NOTICE, "rust"),
    Control("p31_retry_types_twice", RUN,
            "            if delivered {\n", "            if delivered && false {\n", T_NOTICE, "rust"),
    Control("p31_retry_flat_backoff", RUN,
            "    (WAKE_RETRY_FIRST_S << tries.min(5)).min(WAKE_RETRY_MAX_S)\n", "    WAKE_RETRY_FIRST_S\n",
            T_BACKOFF, "rust"),
    Control("p31_retry_no_ceiling", RUN,
            "    (WAKE_RETRY_FIRST_S << tries.min(5)).min(WAKE_RETRY_MAX_S)\n",
            "    WAKE_RETRY_FIRST_S << tries.min(5)\n", T_BACKOFF, "rust"),
    # Review S1: a send-back child and a reseat never inherit the approval; their briefs keep the device line.
    Control("p31_send_back_inherits_approval", GATE,
            "        // Plan 31 review S1: a new Send needs a new `--send-ok` with a new quote.\n        send_ok: None,\n",
            "        send_ok: seated.send_ok.clone(),\n", T_SEND_BACK, "rust"),
    Control("p31_send_back_keeps_send_line", GATE,
            "    let brief = crate::nirvana_gui_tools::child_brief_lines(&brief, known.seated.send_ok.is_some());\n",
            "", T_SEND_BACK, "rust"),
    Control("p31_send_back_drops_original", GATE,
            "    brief.push_str(&original(root_text));\n", "", T_SEND_BACK, "rust"),
    Control("p31_continuation_drops_original", WORKER,
            "    brief.push_str(&original);\n    // Plan 31 review S1", "    // Plan 31 review S1", T_CONTINUATION, "rust"),
    Control("p31_continuation_keeps_send_line", WORKER,
            "    let brief = crate::nirvana_gui_tools::child_brief_lines(&brief, seated.send_ok.is_some());\n",
            "", T_CONTINUATION, "rust"),
    Control("p31_reseat_inherits_approval", WORKER,
            "                // Plan 31 review S1: a new Send needs a new `--send-ok` with a new quote.\n                send_ok: None,\n",
            "                send_ok: seated.send_ok.clone(),\n", T_RESEAT, "rust"),
    Control("p31_child_no_sent_before", GUI,
            "    text.push_str(SENT_BEFORE_LINE);\n", "", T_CHILD, "rust"),
    Control("p31_child_parent_flag_only", GUI,
            "    let approved = parent_send_ok || brief.contains(SEND_LINE) || brief.contains(SENT_BEFORE_LINE);\n",
            "    let approved = parent_send_ok;\n", T_CHILD, "rust"),
    Control("p31_child_keeps_send_line", GUI,
            "        .replace(SEND_LINE, DEVICE_LINE);\n", "        .replace(SEND_LINE, SEND_LINE);\n", T_CHILD, "rust"),
    Control("p31_child_desktop_line_kept", GUI,
            "            &format!(\"{DEVICE_LINE}\\n{BRIEF_LINE}\"),\n",
            "            &format!(\"{DEVICE_LINE}\\n{DESKTOP_SEND_LINE}\"),\n", T_CHILD, "rust"),
    Control("p31_child_bare_no_device", GUI,
            "    if !text.contains(DEVICE_LINE) {\n        text.push_str(\"\\n\\n\");\n        text.push_str(DEVICE_LINE);\n    }\n",
            "", T_CHILD, "rust"),
)
