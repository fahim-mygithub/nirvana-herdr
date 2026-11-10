# Plan 39 P39c: Nirvana Remote's prompt buttons and Esc, and the parallel-read check of the page's assets (package
# `p39c`, nirvana/plans/2026-11-04-39-records/p39c.md). Controls starting with p39c_. One sabotage per assertion, on
# a product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
PROMPT = "src/nirvana_remote/prompt.rs"
ROUTES = "src/nirvana_remote/routes.rs"
HTTP = "src/nirvana_remote/http.rs"
INDEX = "src/nirvana_remote/page/index.html"
PROMPT_JS = "src/nirvana_remote/page/prompt.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"
TESTS_LIVE = "src/nirvana_remote/tests_live.rs"

_T = "nirvana_remote::tests_prompt::"
T_MENU = _T + "a_claude_menu_reads_as_its_heading_and_numbered_buttons"
T_RAW = _T + "a_screen_that_is_not_a_claude_menu_is_shown_raw_with_every_key"
T_API = _T + "api_prompt_answers_a_blocked_agents_menu_read_off_its_screen"
T_TAP = _T + "a_tap_sends_that_options_key_to_the_prompt_the_phone_saw"
T_CHANGED = _T + "a_tap_on_a_prompt_that_changed_is_refused_with_the_new_one"
T_ANSWERED = _T + "a_tap_after_the_pc_answered_presses_nothing"
T_KEYS = _T + "only_keys_from_the_table_reach_send_keys"
T_ESC = _T + "esc_stops_a_working_agent_and_goes_nowhere_else"
T_REFUSED = _T + "a_refused_caller_gets_nothing_from_the_prompt_routes"
T_PAGE = _T + "the_page_has_the_prompt_sheet_and_the_esc_button"
T_WHOLE = "nirvana_remote::tests_live::asset_reads_in_parallel_arrive_whole"
# Review round
T_WHOLE_PROMPT = _T + "menus_equal_in_their_last_rows_hash_apart_by_what_is_above"
T_SPENT = _T + "the_same_seen_twice_presses_once"
T_ESC_SPENT = _T + "a_second_esc_within_the_window_is_refused"

CONTROLS = (
    # Item 1: a menu is read as buttons for a Claude only (only Claude Code's digit picks were measured)
    Control("p39c_menu_claude_only", PROMPT,
            "    let menu = chat::is_claude(agent)\n",
            "    let menu = true\n",
            T_RAW, "rust"),
    # Item 1: the numbers must be the bottom of the screen, not a list Claude wrote above an MCP form
    Control("p39c_menu_at_bottom", PROMPT,
            "                .is_some_and(|(last, _)| at_bottom(&rows, *last))\n",
            "                .is_some_and(|_| true)\n",
            T_RAW, "rust"),
    # Item 1: only rules and footer rows follow the last option
    Control("p39c_footer_only", PROMPT,
            "    after.iter().all(|row| rule(row) || footer(row))\n        && ",
            "    ",
            T_RAW, "rust"),
    # Item 1: the footer names Esc
    Control("p39c_footer_names_esc", PROMPT,
            "\n        && after.iter().any(|row| row.contains(\"esc to\"))\n",
            "\n",
            T_RAW, "rust"),
    # Item 2: the raw fallback is the bottom of the pane
    Control("p39c_raw_bottom", PROMPT,
            "    let skip = bottom.len().saturating_sub(RAW_LINES);\n",
            "    let skip = 0;\n",
            T_RAW, "rust"),
    # Item 1: the menu is read off the visible rows (agent.read)
    Control("p39c_read_visible", PROMPT,
            '"source": "visible"',
            '"source": "recent"',
            T_API, "rust"),
    # Item 1: an agent that is not blocked has no prompt, and its screen is not read
    Control("p39c_blocked_only", PROMPT,
            '        if status != "blocked" {\n',
            "        if false {\n",
            # Alpha 0.68.0: the old test asks only a working agent, which now returns before this line; the picker
            # test asks an idle one at its input box, which this sabotage reads as blocked.
            "nirvana_remote::tests_prompt::api_prompt_answers_a_picker_on_an_agent_that_is_not_working", "rust"),
    # Item 1: a menu takes no Enter (the highlight can move on the PC under the same text)
    Control("p39c_menu_no_enter", PROMPT,
            '                key == "esc"\n',
            '                key == "esc" || key == "enter"\n',
            T_MENU, "rust"),
    # Item 4: the key goes only when the prompt now is the one the phone saw
    Control("p39c_seen_compared", PROMPT,
            "            (Some(shown), Some(seen)) if shown.seen == seen => {\n",
            "            (Some(shown), Some(_)) => {\n",
            T_CHANGED, "rust"),
    # Item 4: a menu's seen is its own text
    Control("p39c_seen_is_the_menu", PROMPT,
            '        Some(menu) => hash(&format!("menu\\n{}", whole_prompt(&rows, menu))),\n',
            '        Some(menu) => hash("menu"),\n',
            T_CHANGED, "rust"),
    # Item 4: a raw screen's seen is its own text
    Control("p39c_seen_is_the_screen", PROMPT,
            '        None => hash(&format!("raw\\n{screen}")),\n',
            '        None => hash("raw"),\n',
            T_CHANGED, "rust"),
    # Item 4: a tap on a prompt that went away (the PC answered) presses nothing, Esc included
    Control("p39c_answered_presses_nothing", PROMPT,
            "        match (shown, body.seen.as_deref()) {\n",
            "        match (shown, body.seen.as_deref().filter(|_| false)) {\n",
            T_ANSWERED, "rust"),
    # Item 5: a key is the table's name exactly
    Control("p39c_key_exact", PROMPT,
            "        .find(|(name, _)| *name == asked)\n",
            "        .find(|(name, _)| name.eq_ignore_ascii_case(asked.trim()))\n",
            T_KEYS, "rust"),
    # Item 5: a key not in the table is refused, before herdr is asked
    Control("p39c_unknown_key_refused", PROMPT,
            "        let Some(key) = key(&body.key) else {\n",
            '        let Some(key) = key(&body.key).or(Some("1")) else {\n',
            T_KEYS, "rust"),
    # Item 5: a key in the table but not this menu's is refused
    Control("p39c_takes_checked", PROMPT,
            "                if !shown.takes(key) {\n",
            "                if false {\n",
            T_KEYS, "rust"),
    # Item 5: a key is JSON (a cross-site form cannot make one)
    Control("p39c_key_json_only", PROMPT,
            '        if !json_body {\n            return error(415, "a key is JSON");\n',
            '        if false {\n            return error(415, "a key is JSON");\n',
            T_KEYS, "rust"),
    # Item 3: Esc goes to a working agent only
    Control("p39c_esc_working_only", PROMPT,
            '                if found.agent["agent_status"].as_str() != Some("working") {\n',
            "                if false {\n",
            T_ESC, "rust"),
    # Item 3: with no prompt seen, only Esc is pressed
    Control("p39c_esc_only_key", PROMPT,
            '                if key != "esc" {\n',
            "                if false {\n",
            T_ESC, "rust"),
    # Item 5: the key route runs after P39a's and P39b's checks
    Control("p39c_key_after_checks", ROUTES,
            "        let logins = request.header(LOGIN_HEADER);\n",
            '        if request.path == "/api/key" {\n            return self.key(request, conn);\n        }\n'
            "        let logins = request.header(LOGIN_HEADER);\n",
            T_REFUSED, "rust"),
    # Item 5: the prompt route runs after the checks
    Control("p39c_prompt_after_checks", ROUTES,
            "        let logins = request.header(LOGIN_HEADER);\n",
            '        if request.path == "/api/prompt" {\n            return self.prompt(request);\n        }\n'
            "        let logins = request.header(LOGIN_HEADER);\n",
            T_REFUSED, "rust"),
    # The page: prompt.js is served and loaded
    Control("p39c_page_asset", ROUTES,
            '        "/prompt.js",\n',
            '        "/prompt-gone.js",\n',
            T_PAGE, "rust"),
    Control("p39c_page_script", INDEX,
            '<script src="/prompt.js"></script>\n',
            "",
            T_PAGE, "rust"),
    # Item 1: the sheet opens on the way into "needs you"
    Control("p39c_page_opens_on_needs", PROMPT_JS,
            "      if (now === 'needs' && was !== 'needs') { pressed = false; read(); }\n",
            "",
            T_PAGE, "rust"),
    # Item 4: a tap carries what the sheet showed
    Control("p39c_page_seen_sent", PROMPT_JS,
            "onclick: () => press(o.key, data.seen, note, true)",
            "onclick: () => press(o.key, null, note, true)",
            T_PAGE, "rust"),
    # Item 4: a prompt that changed is shown again
    Control("p39c_page_changed_shown", PROMPT_JS,
            "show(now, 'The prompt changed on the PC: this is the one there now.')",
            "sheetClose()",
            T_PAGE, "rust"),
    # Item 3: the Esc button sends Esc with no prompt seen
    Control("p39c_page_esc_sent", PROMPT_JS,
            "{ agent: id, key: 'esc', seen: null }",
            "{ agent: id, key: 'esc', seen: 'x' }",
            T_PAGE, "rust"),
    # Items 1 and 3: the chat screen carries the Answer and Esc buttons and draws the state into them
    Control("p39c_page_chat_buttons", CHAT_JS,
            "        prompts.status(agent.status);\n",
            "",
            T_PAGE, "rust"),
    # Item 6: an answer's body is its whole Content-Length (the test helper now sees a short body or a read error)
    Control("p39c_asset_whole", HTTP,
            "    let _ = stream.write_all(answer).await;\n",
            "    let _ = stream.write_all(&answer[..answer.len() / 2]).await;\n",
            T_WHOLE, "rust"),
    # --- Review round ---
    # S1: `whole` checks the length. The page's test asserts only with `contains`, so it goes red here through
    # `whole` alone: `raw` hands it the answer one byte short.
    Control("p39c_whole_checks", TESTS_LIVE,
            '    whole(&answer, bytes.starts_with(b"HEAD "));\n',
            '    whole(&answer[..answer.len() - 1], bytes.starts_with(b"HEAD "));\n',
            T_PAGE, "rust"),
    # S3: a menu's seen is the whole prompt, not the capped lines the sheet shows
    Control("p39c_seen_whole", PROMPT,
            '        Some(menu) => hash(&format!("menu\\n{}", whole_prompt(&rows, menu))),\n',
            '        Some(menu) => hash(&format!("menu\\n{menu:?}")),\n',
            T_WHOLE_PROMPT, "rust"),
    # S3: from the rule above option 1, not from option 1
    Control("p39c_seen_from_the_rule", PROMPT,
            "    for row in &rows[top..] {\n",
            "    for row in &rows[one..] {\n",
            T_WHOLE_PROMPT, "rust"),
    # S3: the highlight is stripped
    Control("p39c_seen_no_highlight", PROMPT,
            "    let bare = |row: &str| row.trim().trim_start_matches('\\u{276F}').trim().to_string();\n",
            "    let bare = |row: &str| row.trim().to_string();\n",
            T_WHOLE_PROMPT, "rust"),
    # S2: a pressed seen is spent
    Control("p39c_spent_claimed", PROMPT,
            "        if !moves && !self.spent.claim(found.id(), &what) {\n",
            "        if !moves && body.seen.is_none() && !self.spent.claim(found.id(), &what) {\n",
            T_SPENT, "rust"),
    # S2: only the same seen is refused, the next prompt is not
    Control("p39c_spent_same_only", PROMPT,
            "            if was == what && at.elapsed() < self.window {\n",
            "            if at.elapsed() < self.window {\n",
            T_SPENT, "rust"),
    # S2: only within the window
    Control("p39c_spent_window", PROMPT,
            "            if was == what && at.elapsed() < self.window {\n",
            "            if was == what && at.elapsed() < SPENT_FOR {\n",
            T_SPENT, "rust"),
    # S2: a key that did not reach herdr spends nothing
    Control("p39c_spent_released", PROMPT,
            "                self.spent.release(found.id(), &what);\n",
            "",
            T_SPENT, "rust"),
    # S4: a second Esc to the same pane is refused
    Control("p39c_esc_spent", PROMPT,
            "        if !moves && !self.spent.claim(found.id(), &what) {\n",
            "        if !moves && body.seen.is_some() && !self.spent.claim(found.id(), &what) {\n",
            T_ESC_SPENT, "rust"),
    # S2: the sheet's keys are off until the answer
    Control("p39c_page_hold", PROMPT_JS,
            "      hold(true);\n",
            "",
            T_PAGE, "rust"),
    # N1: after a menu's digit the sheet reads again
    Control("p39c_page_after", PROMPT_JS,
            "if (menu && key !== 'esc') after(key, seen); else sheetClose();",
            "sheetClose();",
            T_PAGE, "rust"),
    # S4: the Esc button rests after a 200
    Control("p39c_page_esc_rest", PROMPT_JS,
            "setTimeout(() => { esc.disabled = false; }, ESC_REST)",
            "esc.disabled = false",
            T_PAGE, "rust"),
)
