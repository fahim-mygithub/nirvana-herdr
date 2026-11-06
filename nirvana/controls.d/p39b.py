# Plan 39 P39b: Nirvana Remote's spaces, chat, send, paste and upload (package `p39b`,
# nirvana/plans/2026-11-04-39-records/p39b.md). Controls starting with p39b_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
ROOT_RS = "src/nirvana_remote.rs"
ROUTES = "src/nirvana_remote/routes.rs"
HTTP = "src/nirvana_remote/http.rs"
SPACES = "src/nirvana_remote/spaces.rs"
STREAM = "src/nirvana_remote/stream.rs"
CHAT = "src/nirvana_remote/chat.rs"
LIVE = "src/nirvana_remote/live.rs"
SEND = "src/nirvana_remote/send.rs"
LAUNCH = "src/nirvana_launch.rs"
INDEX = "src/nirvana_remote/page/index.html"
APP_JS = "src/nirvana_remote/page/app.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"

_T = "nirvana_remote::tests_live::"
T_LIST = _T + "the_spaces_list_has_every_space_its_agents_and_their_states"
T_VIEW = _T + "the_live_view_pushes_a_change_that_events_subscribe_reports"
T_SUBS = _T + "the_feed_subscribes_to_each_agents_pane_and_again_when_agents_change"
T_RESUB = _T + "a_herdr_stream_that_ends_is_subscribed_again"
T_STOP = _T + "a_live_view_ends_when_nirvana_remote_stops_and_says_when_to_retry"
T_MOST = _T + "at_most_six_live_views_and_a_new_one_ends_the_oldest"
T_SUPERVISED = "nirvana_remote::tests::stopping_nirvana_remote_ends_the_pages_live_views"
T_BUBBLES = _T + "a_transcript_reads_as_bubbles_with_each_tool_call_folded"
T_OWN = _T + "the_transcript_is_the_agents_own_never_a_path_from_the_request"
T_RECORD = _T + "the_folder_session_record_is_the_fallback_with_its_checks"
T_CHAT = _T + "api_chat_answers_the_bubbles_and_the_live_screen"
T_GROW = _T + "the_chat_view_pushes_new_bubbles_as_the_transcript_grows"
T_PROMPT = _T + "a_message_is_typed_through_agent_prompt"
T_CHOOSE = _T + "a_working_agent_asks_queue_or_send_now"
T_NOW = _T + "send_now_presses_esc_and_waits_before_typing"
T_BLOCKED = _T + "a_blocked_agent_is_not_typed_into"
T_BOTH = _T + "two_sends_at_once_both_land"
T_REFUSED = _T + "a_refused_caller_gets_nothing_from_the_new_routes"
T_UPLOAD = _T + "an_upload_lands_in_the_spaces_uploads_folder_under_the_servers_name"
T_TOO_BIG = _T + "an_upload_over_50_mb_is_refused_before_its_body_is_read"
T_CHUNKED = _T + "a_chunked_upload_is_refused_the_moment_it_passes_50_mb"
T_SLOW = _T + "a_slow_upload_is_cut_and_leaves_no_file"  # was ..._gets_408_...: a cut body is a 400 since P39u
T_CAP = _T + "the_33rd_connection_waits_while_a_50_mb_upload_holds_a_slot"
T_PATHS = _T + "a_send_adds_the_uploaded_files_paths_and_refuses_any_other_name"
T_NAMES = _T + "upload_names_are_the_servers_own"
T_FRAMING = _T + "the_body_framing_is_read_from_the_head"
T_PAGE = _T + "the_page_has_the_spaces_and_chat_screens_and_no_inline_script"
# Review round (record p39b, "Review round").
HERDR = "src/nirvana_remote/herdr.rs"
T_WHOSE = _T + "the_folders_record_is_a_lone_claudes_only"
T_FIRST_PROMPT = _T + "a_chat_view_takes_the_transcript_that_comes_with_the_first_prompt"
T_CONTROLS = _T + "phone_text_reaches_agent_prompt_without_controls"
T_BETWEEN = _T + "a_change_between_the_snapshot_and_the_subscription_is_seen"
T_SPIN = _T + "a_stream_that_ends_at_once_is_not_subscribed_in_a_loop"
T_OLD = _T + "an_old_stream_ends_when_the_feed_subscribes_again"
T_SILENT = _T + "a_socket_that_never_answers_is_given_up_on"
T_STOP_READER = _T + "a_subscription_told_to_stop_ends_its_reader"
T_RECHECK = _T + "a_live_view_ends_when_the_owner_or_this_pc_changes"
T_CROSS_READ = _T + "a_cross_site_read_of_the_api_gets_nothing"
T_EARLY_413 = _T + "a_413_on_the_head_reaches_a_caller_that_sends_its_body_first"
T_CHUNK_HEX = _T + "a_chunk_size_is_plain_hex"
T_PAGE_ROUND = _T + "the_page_ends_a_gone_view_and_waits_out_composition"

CONTROLS = (
    # §2.1: herdr's blocked reads as "needs you"
    Control("p39b_status_needs", SPACES,
            '        "blocked" => "needs",\n',
            '        "blocked" => "idle",\n',
            T_LIST, "rust"),
    # §2.1: an agent shows under its own space only, and one in no space of this herdr not at all
    Control("p39b_agent_in_its_space", SPACES,
            '                .filter(|agent| agent["workspace_id"] == space["workspace_id"])\n',
            "                .filter(|_| true)\n",
            T_LIST, "rust"),
    # §2.1: the space shows needs you over working over done
    Control("p39b_space_status_order", SPACES,
            '    ["needs", "working", "done"]\n',
            '    ["done", "working", "needs"]\n',
            T_LIST, "rust"),
    # §2.1: the agent's line is its task, not Claude Code's state mark
    Control("p39b_task_title", SPACES,
            '        "line": crate::nirvana_spaces::task_title(agent["title"].as_str()),\n',
            '        "line": agent["title"],\n',
            T_LIST, "rust"),
    # §2.1 live: a list that changed is pushed to the page
    Control("p39b_feed_pushes_change", SPACES,
            "            if state.last.as_deref() != Some(list.as_str()) {\n",
            "            if state.last.is_none() {\n",
            T_VIEW, "rust"),
    # §2.1 live: each agent's pane is subscribed for its state
    Control("p39b_feed_watches_panes", SPACES,
            '            .map(|pane| json!({ "type": "pane.agent_status_changed", "pane_id": pane })),\n',
            '            .map(|pane| json!({ "type": "pane.updated", "pane_id": pane })),\n',
            T_SUBS, "rust"),
    # §2.1 live: new agents, a new subscription with their panes
    Control("p39b_feed_resubscribes_new_agents", SPACES,
            "                if changed_agents {\n",
            "                if false {\n",
            T_SUBS, "rust"),
    # §2 failures, herdr side: a stream that ends is subscribed again
    Control("p39b_feed_resubscribes_on_end", SPACES,
            "                    // The stream ended (herdr fell behind, or its server is going): subscribe again.\n"
            "                    break;\n",
            "                    return;\n",
            T_RESUB, "rust"),
    # Stopping ends every live view
    Control("p39b_close_all_ends_views", STREAM,
            "                ended.store(true, Ordering::SeqCst);\n",
            "                let _ = ended;\n",
            T_STOP, "rust"),
    # §2 failures, page side: the view says how soon EventSource reconnects
    Control("p39b_retry_sent", STREAM,
            '    conn.write(format!("retry: {RETRY_MS}\\n\\n").as_bytes())\n',
            "    Ok(())\n",
            T_STOP, "rust"),
    # A live view has no length (the body ends with the connection)
    Control("p39b_stream_head_no_length", STREAM,
            "    conn.stream_head(&head)?;\n",
            "    conn.write(&head.encode(false))?;\n",
            T_STOP, "rust"),
    # At most six views; a new one ends the oldest
    Control("p39b_views_capped", STREAM,
            "            while open.len() >= MOST {\n",
            "            while open.len() > MOST {\n",
            T_MOST, "rust"),
    # The supervisor's stop ends the views
    Control("p39b_supervisor_ends_views", ROOT_RS,
            "        self.streams.close_all();\n",
            "",
            T_SUPERVISED, "rust"),
    # §2.2: a tool call's result fills its folded line
    Control("p39b_tool_result_folded", CHAT,
            '                    call["result"] = json!(cut(&first_line(&result), RESULT_MOST));\n',
            '                    call["result"] = json!("");\n',
            T_BUBBLES, "rust"),
    # §2.2: a failed call reads as failed
    Control("p39b_tool_error", CHAT,
            '                    call["ok"] = json!(!failed);\n',
            '                    call["ok"] = json!(true);\n',
            T_BUBBLES, "rust"),
    # §2.2: Claude Code's own notes are not bubbles
    Control("p39b_meta_skipped", CHAT,
            '        if entry["isMeta"] == true || entry["isSidechain"] == true {\n',
            '        if entry["isSidechain"] == true {\n',
            T_BUBBLES, "rust"),
    # §2.2: a subagent's lines are not bubbles
    Control("p39b_sidechain_skipped", CHAT,
            '        if entry["isMeta"] == true || entry["isSidechain"] == true {\n',
            '        if entry["isMeta"] == true {\n',
            T_BUBBLES, "rust"),
    # §2.2: a slash command's echo is not a bubble
    Control("p39b_echo_skipped", CHAT,
            "                if !text.is_empty() && !text.starts_with('<') {\n",
            "                if !text.is_empty() {\n",
            T_BUBBLES, "rust"),
    # §2.2 trust: a transcript is under this PC's Claude config only, whatever names it
    Control("p39b_transcript_under_claude", CHAT,
            '    stem_ok && path.is_absolute() && path.is_file() && same_dir(projects, &claude.join("projects"))\n',
            "    stem_ok && path.is_absolute() && path.is_file()\n",
            T_OWN, "rust"),
    # §2.2 trust: its name is a plain session id
    Control("p39b_transcript_plain_id", CHAT,
            "            .is_some_and(plain_id);\n",
            "            .is_some();\n",
            T_OWN, "rust"),
    # §2.2 trust: an agent of this herdr's spaces only
    Control("p39b_agent_in_a_space", LIVE,
            '                    space: space(&agent["workspace_id"])?,\n',
            '                    space: space(&agent["workspace_id"]).unwrap_or_default(),\n',
            T_OWN, "rust"),
    # §2.2: with no token, the folder's session record (what "Open Nirvana here" resumes)
    Control("p39b_record_fallback", CHAT,
            "    let home = home?;\n",
            "    let home: &Path = None?;\n",
            T_RECORD, "rust"),
    # §2.2: the record's own check (the file's stem is its id), reused from resume_session_at
    Control("p39b_record_stem_checked", LAUNCH,
            "        || !stem_matches\n",
            "",
            T_RECORD, "rust"),
    # §2.2: herdr's token, when there is one, is the answer; a refused token does not fall through to the record
    Control("p39b_token_wins", CHAT,
            "        return is_transcript(&path, claude).then_some(path);\n",
            "        if is_transcript(&path, claude) {\n            return Some(path);\n        }\n",
            T_RECORD, "rust"),
    # §2.2: the live state is agent.read's screen
    Control("p39b_live_from_read", LIVE,
            "            .map(|read| chat::live(&read))\n",
            "            .map(|_read| String::new())\n",
            T_CHAT, "rust"),
    # §2.2 live: the transcript growing pushes its new bubbles
    Control("p39b_chat_grows", LIVE,
            "            if fresh != stamp {\n",
            "            if false {\n",
            T_GROW, "rust"),
    # §2.2 live: only new or changed bubbles are pushed
    Control("p39b_chat_changed_only", LIVE,
            "                        if sent.get(&key) == Some(&text) {\n",
            "                        if false {\n",
            T_GROW, "rust"),
    # §2.2 live: a state change pushes the agent
    Control("p39b_chat_state_pushed", LIVE,
            "                            if now != row {\n",
            "                            if false {\n",
            T_GROW, "rust"),
    # §2.3: the message as typed is agent.prompt's text
    Control("p39b_send_prompt_text", LIVE,
            '            .call("agent.prompt", json!({"target": id, "text": message}))\n',
            '            .call("agent.prompt", json!({"target": id, "text": body.text}))\n',
            T_PROMPT, "rust"),
    # §2.3: a send is JSON (a cross-site form cannot make one)
    Control("p39b_send_json_only", LIVE,
            "        if !json_body {\n",
            "        if false {\n",
            T_PROMPT, "rust"),
    # §2.3: a send over its limit is refused on its head
    Control("p39b_body_length_limit", HTTP,
            "            Framing::Length(length) if length > limit => return Err(BodyError::TooLarge),\n",
            "",
            T_PROMPT, "rust"),
    # §2 failure paragraph: while the agent works, the owner picks
    Control("p39b_working_asks", LIVE,
            '            "working" => match body.mode.as_deref() {\n',
            '            "working-never" => match body.mode.as_deref() {\n',
            T_CHOOSE, "rust"),
    # send now: Esc first
    Control("p39b_send_now_esc", LIVE,
            '                        .call("agent.send_keys", json!({"target": id, "keys": ["esc"]}))\n',
            '                        .call("agent.send_keys", json!({"target": id, "keys": ["enter"]}))\n',
            T_NOW, "rust"),
    # send now: then wait for it to stop
    Control("p39b_send_now_waits", LIVE,
            '                                "agent.wait",\n',
            '                                "agent.get",\n',
            T_NOW, "rust"),
    # A blocked agent is not typed into
    Control("p39b_blocked_refused", LIVE,
            '            "blocked" => {\n',
            '            "blocked-never" => {\n',
            T_BLOCKED, "rust"),
    # §2 failures: PC and phone at once both land, no lock (a lock that refuses the second reddens it)
    Control("p39b_no_lock", LIVE,
            "        let message = send::message(&body.text, &files);\n",
            "        static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);\n"
            "        if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {\n"
            '            return error(409, "busy");\n'
            "        }\n"
            "        let message = send::message(&body.text, &files);\n",
            T_BOTH, "rust"),
    # Every new route runs after the checks: a send placed before them reaches herdr for anyone
    Control("p39b_send_after_checks", ROUTES,
            "        let logins = request.header(LOGIN_HEADER);\n",
            '        if request.path == "/api/send" {\n'
            "            return self.send(request, conn);\n"
            "        }\n"
            "        let logins = request.header(LOGIN_HEADER);\n",
            T_REFUSED, "rust"),
    # ... and so does a chat placed before them
    Control("p39b_chat_after_checks", ROUTES,
            "        let logins = request.header(LOGIN_HEADER);\n",
            '        if request.path == "/api/chat" {\n'
            "            return self.chat(request);\n"
            "        }\n"
            "        let logins = request.header(LOGIN_HEADER);\n",
            T_REFUSED, "rust"),
    # Uploads: the server names the file, never the client
    Control("p39b_upload_server_name", LIVE,
            "        let name = send::name_for(kind, (self.now)());\n",
            '        let name = format!("{}.png", super::live::query_value(&request.query, "name").unwrap_or_default());\n',
            T_UPLOAD, "rust"),
    # Uploads: kept out of git by the folder's own .gitignore
    Control("p39b_upload_gitignore", SEND,
            "            .write_all(IGNORE.as_bytes())\n",
            '            .write_all(b"")\n',
            T_UPLOAD, "rust"),
    # Uploads: over 50 MB by its Content-Length, refused before the body (and before herdr is asked)
    Control("p39b_upload_head_413", LIVE,
            "            Framing::Length(length) if length > send::UPLOAD_MOST => {\n",
            "            Framing::Length(length) if length > send::UPLOAD_MOST * 2 => {\n",
            T_TOO_BIG, "rust"),
    # Uploads: a chunked body is counted while it streams
    Control("p39b_chunked_limit", HTTP,
            "            if total > limit {\n",
            "            if total > limit * 2 {\n",
            T_CHUNKED, "rust"),
    # Uploads: a refused one leaves no .partial
    Control("p39b_partial_removed", SEND,
            "        Err(error) => {\n            let _ = std::fs::remove_file(&partial);\n",
            "        Err(error) => {\n",
            T_CHUNKED, "rust"),
    # Uploads: the chunks are decoded, framing dropped
    Control("p39b_chunk_end_checked", HTTP,
            "            if !self.line(deadline)?.is_empty() {\n",
            "            if self.line(deadline)?.is_empty() {\n",
            T_CHUNKED, "rust"),
    # P39a's slow-request rule holds for a body: under the average rate is cut at the deadline
    Control("p39b_body_rate_deadline", HTTP,
            "        let allowed = (self.limits.read_timeout + Duration::from_secs(size / rate))\n",
            "        let allowed = (self.limits.read_timeout + Duration::from_secs(size / rate + 60))\n",
            T_SLOW, "rust"),
    # ... and a body that stops is cut (a 400 since P39u)
    Control("p39b_body_timeout_408", HTTP,
            "                Err(BodyError::Timeout)\n            }\n            Err(_) => {\n",
            '                Err(BodyError::Bad("timed out"))\n            }\n            Err(_) => {\n',
            T_SLOW, "rust"),
    # P39a's 32-connection cap holds while a 50 MB upload is in flight
    Control("p39b_cap_with_upload", HTTP,
            "    let slots = Arc::new(tokio::sync::Semaphore::new(limits.connections));\n",
            "    let slots = Arc::new(tokio::sync::Semaphore::new(limits.connections + 1));\n",
            T_CAP, "rust"),
    # The message carries each upload's path
    Control("p39b_send_adds_paths", SEND,
            "    parts.extend(files.iter().map(|file| {\n",
            "    parts.extend(files.iter().filter(|_| false).map(|file| {\n",
            T_PATHS, "rust"),
    # Only the server's upload names become paths
    Control("p39b_send_names_checked", LIVE,
            "                if !send::is_upload_name(name) || !path.is_file() {\n",
            "                if !path.is_file() {\n",
            T_PATHS, "rust"),
    # An upload name has a known extension (no .exe, no ADS stream)
    Control("p39b_name_extension", SEND,
            '        && (ext == "bin" || KINDS.iter().any(|(_, known)| *known == ext))\n',
            "        && !ext.is_empty()\n",
            T_NAMES, "rust"),
    # ... and exactly its 8 hex digits
    Control("p39b_name_hex", SEND,
            "    let hex = |part: &str| part.len() == 8 && part.bytes().all(|b| b.is_ascii_hexdigit());\n",
            "    let hex = |part: &str| !part.is_empty();\n",
            T_NAMES, "rust"),
    # A Content-Length is plain digits
    Control("p39b_length_digits", HTTP,
            "            if length.is_empty() || !length.bytes().all(|b| b.is_ascii_digit()) {\n",
            "            if length.is_empty() {\n",
            T_FRAMING, "rust"),
    # Two framings at once (the smuggling shape) are refused
    Control("p39b_two_framings_bad", HTTP,
            "        _ => Framing::Bad,\n",
            "        _ => Framing::None,\n",
            T_FRAMING, "rust"),
    # The page loads the chat screen
    Control("p39b_page_chat_script", INDEX,
            '<script src="/chat.js"></script>\n',
            "",
            T_PAGE, "rust"),
    # iPhone Safari back from the background: the view opens again
    Control("p39b_page_visibility", APP_JS,
            "    document.addEventListener('visibilitychange', shown);\n",
            "",
            T_PAGE, "rust"),
    # A screenshot pasted into the box is taken
    Control("p39b_page_paste", CHAT_JS,
            "      input.addEventListener('paste', (event) => {\n",
            "      input.addEventListener('drop', (event) => {\n",
            T_PAGE, "rust"),
    # The photo library on iPhone
    Control("p39b_page_photos", CHAT_JS,
            "accept: 'image/*'",
            "accept: '*/*'",
            T_PAGE, "rust"),
    # --- Review round ---
    # S1: the folder's record is a Claude's only, never a Codex's or an Antigravity's
    Control("p39b_record_claude_only", CHAT,
            "    if !is_claude(agent) {\n",
            "    if false {\n",
            T_WHOSE, "rust"),
    # S1: ... and only a lone Claude's in that folder
    Control("p39b_record_lone_claude", CHAT,
            "    if shared {\n",
            "    if false {\n",
            T_WHOSE, "rust"),
    # S1: a Claude before its probe is known by herdr's label
    Control("p39b_kind_from_label", CHAT,
            '        None => agent["agent"].as_str().and_then(Kind::from_agent_label),\n',
            "        None => None,\n",
            T_WHOSE, "rust"),
    # S1: a chat view takes the transcript that comes later, and sends the whole chat from it
    Control("p39b_chat_path_rechecked", LIVE,
            "                                    Some(found) if fresh != path => {\n",
            "                                    Some(found) if false => {\n",
            T_FIRST_PROMPT, "rust"),
    # S2: the phone's text loses its controls before agent.prompt
    Control("p39b_text_controls_dropped", SEND,
            "    let text: String = text.chars().filter(|&c| typed(c)).collect();\n",
            "    let text: String = text.to_string();\n",
            T_CONTROLS, "rust"),
    # S2: ... the C1 ones too
    Control("p39b_c1_dropped", SEND,
            "    c == '\\n' || c == '\\t' || !c.is_control()\n",
            "    c == '\\n' || c == '\\t' || (c >= ' ' && c != '\\u{7f}')\n",
            T_CONTROLS, "rust"),
    # S2: a file's path follows after a space, not an Enter
    Control("p39b_files_after_space", SEND,
            '    parts.join(" ")\n',
            '    parts.join("\\n")\n',
            T_PATHS, "rust"),
    # S2: a path with a space is quoted
    Control("p39b_path_quoted", SEND,
            "        if path.contains(char::is_whitespace) {\n",
            "        if false {\n",
            T_CONTROLS, "rust"),
    # S3a: a change between the snapshot and the subscription is sent
    Control("p39b_feed_snapshot_after_subscribe", SPACES,
            "                    if !self.send(Some(again.to_string())) {\n",
            "                    if false {\n",
            T_BETWEEN, "rust"),
    # S3b: a wait before every subscription after the first
    Control("p39b_feed_backoff", SPACES,
            "                std::thread::sleep(retry);\n",
            "",
            T_SPIN, "rust"),
    # S3b: ... reset only by an event, not by the ack
    Control("p39b_feed_reset_on_event", SPACES,
            "            let _ends = Stop(stop);\n",
            "            let _ends = Stop(stop);\n            retry = RETRY_FIRST;\n",
            T_SPIN, "rust"),
    # S3c: a stream left behind is told to stop
    Control("p39b_old_stream_stopped", SPACES,
            "        self.0.store(true, Ordering::SeqCst);\n",
            "        let _ = &self.0;\n",
            T_OLD, "rust"),
    # S3c: the real socket reader ends once told to stop
    Control("p39b_polled_stop", HERDR,
            "                .is_some_and(|stop| stop.load(Ordering::SeqCst))\n",
            "                .is_some_and(|_| false)\n",
            T_STOP_READER, "rust"),
    # S4: a socket call gives up at its wait plus the margin
    Control("p39b_socket_deadline", HERDR,
            "    reader.get_mut().deadline = Some(Instant::now() + wait);\n",
            "    reader.get_mut().deadline = Some(Instant::now() + wait * 3);\n",
            T_SILENT, "rust"),
    # S4: ... and not before: agent.wait's own timeout plus the margin
    Control("p39b_socket_wait_margin", HERDR,
            "        .map_or(ANSWER_TIMEOUT, |ms| Duration::from_millis(ms) + MARGIN)\n",
            "        .map_or(ANSWER_TIMEOUT, Duration::from_millis)\n",
            T_SILENT, "rust"),
    # S5: a closed agent's view ends on the page
    Control("p39b_page_gone_closes", CHAT_JS,
            "        gone: (_, close) => { close(); note.textContent = 'This agent has closed on the PC.'; },\n",
            "        gone: () => { note.textContent = 'This agent has closed on the PC.'; },\n",
            T_PAGE_ROUND, "rust"),
    # S5: a handler is given its view's close
    Control("p39b_page_handler_close", APP_JS,
            "          if (!over) on[name](data, close);\n",
            "          on[name](data);\n",
            T_PAGE_ROUND, "rust"),
    # S6: a live view asks again each wake who it serves
    Control("p39b_view_rechecks_caller", STREAM,
            "    while !ticket.over() && still() {\n",
            "    while !ticket.over() {\n",
            T_RECHECK, "rust"),
    # S7: a read of /api/ is held to Sec-Fetch-Site and Origin
    Control("p39b_api_reads_checked", ROUTES,
            '    if reads && !request.path.starts_with("/api/") {\n',
            "    if reads {\n",
            T_CROSS_READ, "rust"),
    # S7: the page's own files still open from a link
    Control("p39b_page_files_open_from_links", ROUTES,
            '    if reads && !request.path.starts_with("/api/") {\n',
            "    if false {\n",
            T_CROSS_READ, "rust"),
    # S7: a read with neither header (Safari before 16.4, another PC's probe) is let in
    Control("p39b_read_without_headers", ROUTES,
            "        if reads {\n            return Ok(());\n        }\n",
            "",
            T_CROSS_READ, "rust"),
    # N2: one reopen on a wake
    Control("p39b_page_one_reopen", APP_JS,
            "      if (document.visibilityState === 'visible' && Date.now() - opened >= 1000) open();\n",
            "      if (document.visibilityState === 'visible') open();\n",
            T_PAGE_ROUND, "rust"),
    # N3: no send while an IME composes
    Control("p39b_page_ime", CHAT_JS,
            "        if (event.key !== 'Enter' || event.isComposing || event.keyCode === 229) return;\n",
            "        if (event.key !== 'Enter') return;\n",
            T_PAGE_ROUND, "rust"),
    # N4: a 413 on the head reaches a caller that sent megabytes first
    Control("p39b_drain_large", HTTP,
            "pub const DRAIN_LIMIT: usize = 8 * 1024 * 1024;\n",
            "pub const DRAIN_LIMIT: usize = 64 * 1024;\n",
            T_EARLY_413, "rust"),
    # N5: a chunk size is plain hex
    Control("p39b_chunk_plain_hex", HTTP,
            "            if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {\n",
            "            if hex.is_empty() {\n",
            T_CHUNK_HEX, "rust"),
)
