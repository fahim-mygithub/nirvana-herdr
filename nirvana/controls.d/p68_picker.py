# Alpha 0.68.0: the phone works a picker the PC opened (`/resume`, `/model`). The owner, 2026-11-10: "1" sent from the
# phone into `/resume` became its search. Controls starting with p68_. One sabotage per assertion, each on a product
# line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
PROMPT = "src/nirvana_remote/prompt.rs"
LIVE = "src/nirvana_remote/live.rs"
PROMPT_JS = "src/nirvana_remote/page/prompt.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"

_T = "nirvana_remote::tests_prompt::"
T_READ = _T + "a_picker_reads_from_its_border_with_its_own_keys_and_a_stable_seen"
T_API = _T + "api_prompt_answers_a_picker_on_an_agent_that_is_not_working"
T_KEYS = _T + "a_pickers_arrows_go_every_tap_and_its_enter_once"
T_TYPE = _T + "a_search_is_typed_into_the_picker_the_phone_saw_with_no_enter"
T_SEND = _T + "a_message_to_an_open_picker_is_not_typed_and_the_page_gets_its_keys"
T_HINT = _T + "the_chat_says_when_its_live_screen_is_a_picker"
T_PAGE = _T + "the_page_has_the_picker_sheet_and_opens_it"

CONTROLS = (
    # Reading a picker: the way out on the last row, from the border down, the footer's hash, a search offered
    Control("p68_exit_anywhere", PROMPT,
            "        .any(|exit| last.contains(exit) && footer.contains(exit))\n",
            "        .any(|exit| footer.contains(exit))\n", T_READ, "rust"),
    Control("p68_interrupt_is_an_exit", PROMPT,
            '    "esc to cancel",\n    "esc to exit",\n',
            '    "esc to",\n    "esc to exit",\n', T_READ, "rust"),
    Control("p68_whole_screen_shown", PROMPT,
            "        .map_or(bottom.len().saturating_sub(RAW_LINES), |at| at + 1);\n",
            "        .map_or(bottom.len().saturating_sub(RAW_LINES), |_| 0);\n", T_READ, "rust"),
    Control("p68_seen_from_screen", PROMPT,
            '        seen: hash(&format!("picker\\n{footer}")),\n',
            '        seen: hash(&format!("picker\\n{screen}")),\n', T_READ, "rust"),
    Control("p68_search_never_offered", PROMPT,
            '        picker: Some(footer.contains("type to search")),\n',
            "        picker: Some(false),\n", T_API, "rust"),
    # The keys: a picker's own, a menu takes no arrow, a raw screen no Backspace
    Control("p68_picker_takes_digits", PROMPT,
            "            return PICKER_KEYS.contains(&key);\n",
            "            return true;\n", T_KEYS, "rust"),
    Control("p68_raw_takes_backspace", PROMPT,
            '        if key == "backspace" {\n            return false;\n        }\n',
            "", T_KEYS, "rust"),
    # Only an agent that is not working is read for one, and the arrows are not spent
    Control("p68_working_read", PROMPT,
            '        if status == "working" {\n            return Ok((json!({"blocked": false, "agent": row}), None));\n        }\n',
            "", T_API, "rust"),
    Control("p68_idle_not_read", PROMPT,
            "            let Some(shown) = picker(&self.screen(found.id())?) else {\n",
            "            let Some(shown) = None::<Shown> else {\n", T_API, "rust"),
    Control("p68_arrows_spent", PROMPT,
            '        let moves = now["picker"] == true && NAVIGATION.contains(&key);\n',
            "        let moves = false;\n", T_KEYS, "rust"),
    Control("p68_enter_not_spent", PROMPT,
            '        let moves = now["picker"] == true && NAVIGATION.contains(&key);\n',
            '        let moves = now["picker"] == true;\n', T_KEYS, "rust"),
    # The search: pasted with no key, only into the picker seen, only one that offers it, no control
    Control("p68_search_with_enter", PROMPT,
            '            json!({"pane_id": found.id(), "text": text, "keys": []}),\n',
            '            json!({"pane_id": found.id(), "text": text, "keys": ["Enter"]}),\n', T_TYPE, "rust"),
    Control("p68_search_any_picker", PROMPT,
            "            Some(shown) if shown.picker == Some(true) && shown.seen == body.seen => {}\n",
            "            Some(shown) if shown.picker.is_some() => {}\n", T_TYPE, "rust"),
    Control("p68_search_keeps_newline", PROMPT,
            "        .filter(|&c| c != '\\n' && c != '\\t' && super::send::typed(c))\n",
            "        .filter(|&c| super::send::typed(c))\n", T_READ, "rust"),
    # A message to an open picker is not typed; the chat hints a picker; a working agent is no picker
    Control("p68_send_types_into_picker", LIVE,
            "                Ok((now, Some(_))) => {\n",
            "                Ok((now, Some(_))) if false => {\n", T_SEND, "rust"),
    Control("p68_hint_never", LIVE,
            "    super::prompt::picker_footer(&rows).is_some()\n",
            "    let _ = rows;\n    false\n", T_HINT, "rust"),
    Control("p68_hint_while_working", LIVE,
            '    if matches!(status, "working" | "blocked") {\n        return false;\n    }\n',
            "", T_HINT, "rust"),
    # The page: the picker sheet, its marked row, its refresh, its search, and the three ways it opens
    Control("p68_page_no_picker_sheet", PROMPT_JS,
            "      if (data.picker) { showPicker(data, why); return; }\n",
            "", T_PAGE, "rust"),
    Control("p68_page_row_unmarked", PROMPT_JS,
            "class: 'pick-line' + (/^\\s*\\u276F/.test(line) ? ' sel' : '')",
            "class: 'pick-line'", T_PAGE, "rust"),
    Control("p68_page_no_refresh", PROMPT_JS,
            "if (r.ok) { pressed = true; await refresh(",
            "if (r.ok) { pressed = true; sheetClose(", T_PAGE, "rust"),
    Control("p68_page_hint_ignored", CHAT_JS,
            "        live: (data) => { drawLive(data.live); prompts.picker(data.picker); },\n",
            "        live: (data) => drawLive(data.live),\n", T_PAGE, "rust"),
    Control("p68_page_refused_send_silent", CHAT_JS,
            "        if (r.status === 409 && r.data.picker && r.data.prompt) {\n",
            "        if (false) {\n", T_PAGE, "rust"),
    Control("p68_page_slash_not_watched", CHAT_JS,
            "        if (text.startsWith('/')) prompts.watch();\n",
            "", T_PAGE, "rust"),
)
