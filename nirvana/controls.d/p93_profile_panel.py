# Plan 45 P45a (the owner, 2026-11-18: "for each project we add a plus symbol next to agents and the agents + becocmes
# abutton to store profile. Theses would be besically where we store the agent proiles."). Controls starting with p93_.
# One sabotage per assertion, each on a product line of the `+`, the space menu's line or the Profiles panel; the test
# named in the fifth field is the one expected to print its red line with the sabotage in place (written before the
# run). `Control` is provided by controls.py.
PANEL = "src/client/shell/nirvana_profile_panel.rs"
SIDEBAR = "src/client/shell/agent_sidebar.rs"
MENU = "src/client/shell/context_menu.rs"
MOUSE = "src/client/shell/mouse.rs"
PATCH = "src/client/shell/surface_patch.rs"

_T = "client::shell::tests::nirvana_profile_panel::"
T_PLUS = _T + "p93_the_plus_follows_agents_and_a_click_opens_the_panel_on_the_focused_space"
T_NARROW = _T + "p93_the_plus_gives_way_to_the_sort_label_at_a_narrow_sidebar"
T_MARK = _T + "p93_the_plus_marks_the_proposals_waiting"
T_MENU = _T + "p93_the_space_menu_has_a_profiles_line_that_opens_the_panel"
T_ROWS = _T + "p93_a_profile_row_with_every_field_a_bare_one_and_a_refused_one"
T_USE = _T + "p93_the_use_reads_carried_of_fit_and_in_words_when_nothing_fits"
T_EMPTY = _T + "p93_the_empty_states_say_so_in_plain_words"
T_DETAIL = _T + "p93_suggested_lists_each_proposal_and_enter_opens_its_whole_detail"
T_ACCEPT = _T + "p93_accept_is_only_in_the_detail_and_its_note_stays_until_the_next_key"
T_WRITES = _T + "p93_an_accept_writes_one_profile_and_one_ledger_row_and_a_dismiss_no_profile"
T_REFUSED = _T + "p93_a_refused_accept_is_shown_word_for_word_and_dismiss_works_from_the_detail"
T_NEW = _T + "p93_new_with_and_without_a_shape_and_a_refused_name"
T_EDIT = _T + "p93_e_enter_and_k_open_the_profile_and_its_cards"
T_KEYS = _T + "p93_keys_tab_the_wheel_and_a_click_select"
T_FOLLOW = _T + "p93_the_selection_follows_its_name_or_id_through_a_reread"
T_ESC = _T + "p93_esc_closes_the_detail_then_the_panel"
T_WIDTHS = _T + "p93_80_100_and_140_columns"
T_LONG = _T + "p93_a_long_name_and_a_4000_character_body_never_panic"
T_REDRAWN = _T + "p93_the_panel_is_redrawn_when_a_pane_prints"
T_ONE = _T + "p93_opening_the_panel_closes_the_strips_panels"

CONTROLS = (
    # ---- the `+` ----
    Control("p93_plus_not_drawn", SIDEBAR, "    render_profiles_plus(buffer, area, snapshot, config, hits);\n", "",
            T_PLUS, "rust"),
    Control("p93_plus_not_clickable", SIDEBAR, "        hits.agent_profiles = Rect::new(x, area.y + 1, text_width, 1);",
            "        hits.agent_profiles = Rect::default();", T_PLUS, "rust"),
    Control("p93_plus_click_ignored", MOUSE, "                    self.toggle_profile_panel();\n", "", T_PLUS, "rust"),
    Control("p93_plus_first_space", PANEL, "        .find(|workspace| workspace.focused)", "        .find(|_| true)",
            T_PLUS, "rust"),
    Control("p93_plus_space_folder_ignored", PANEL, "        .or_else(in_tab)", "        .or_else(|| None)", T_PLUS,
            "rust"),
    Control("p93_plus_never_gives_way", SIDEBAR, "    if x + text_width + 1 > sort_x {", "    if false {", T_NARROW,
            "rust"),
    Control("p93_plus_no_brackets", PANEL, '        _ => "[+]".to_owned(),', '        _ => "+".to_owned(),', T_NARROW,
            "rust"),
    Control("p93_plus_no_count", PANEL, '        Some(count) if count > 0 => format!("[+{count}]"),',
            '        Some(count) if count > 0 => "[+]".to_owned(),', T_MARK, "rust"),
    # ---- the space menu ----
    Control("p93_menu_no_line", MENU, "            items.push(item(SPACE_PROFILES, Action::Profiles));\n", "",
            T_MENU, "rust"),
    Control("p93_menu_line_not_last", MENU, "            items.push(item(SPACE_PROFILES, Action::Profiles));",
            "            items.insert(1, item(SPACE_PROFILES, Action::Profiles));", T_MENU, "rust"),
    Control("p93_menu_line_does_nothing", MENU,
            "            ClientContextMenuAction::Profiles => self.open_space_profile_panel(&workspace_id),",
            "            ClientContextMenuAction::Profiles => {}", T_MENU, "rust"),
    # ---- a profile's row ----
    Control("p93_row_no_family", PANEL, '        parts.push(format!("family {family}"));\n', "", T_ROWS, "rust"),
    Control("p93_row_no_model", PANEL, '        parts.push(format!("model {model}"));\n', "", T_ROWS, "rust"),
    Control("p93_row_no_effort", PANEL, '        parts.push(format!("effort {effort}"));\n', "", T_ROWS, "rust"),
    Control("p93_row_own_servers_unread", PANEL, "                    .is_some_and(|file| file.is_file()),",
            "                    .is_some_and(|_| false),", T_ROWS, "rust"),
    Control("p93_row_no_skills", PANEL, '        Some(skills) => parts.push(format!("skills {}", skills.join(", "))),',
            "        Some(_) => {}", T_ROWS, "rust"),
    Control("p93_row_empty_skills_silent", PANEL,
            '        Some(skills) if skills.is_empty() => parts.push("skills none".to_owned()),',
            "        Some(skills) if skills.is_empty() => {}", T_ROWS, "rust"),
    Control("p93_row_no_cards", PANEL,
            '        parts.push(format!("{cards} card{}", if cards == 1 { "" } else { "s" }));\n', "", T_ROWS, "rust"),
    Control("p93_row_refused_no_hint", PANEL,
            '            let refused = format!("refused: herdr profile check {name}");',
            '            let refused = String::from("refused");', T_ROWS, "rust"),
    # ---- the use ----
    Control("p93_use_nothing_as_a_count", PANEL, "        (Some((0, 0)), false) => NO_USE.to_owned(),",
            '        (Some((0, 0)), false) => "used 0 of 0".to_owned(),', T_USE, "rust"),
    Control("p93_use_fit_of_carried", PANEL,
            '        (Some((carried, fit)), false) => format!("used {carried} of {fit}"),',
            '        (Some((carried, fit)), false) => format!("used {fit} of {carried}"),', T_USE, "rust"),
    Control("p93_use_fit_as_carried", PANEL, "                    .map(|row| (row.carried, row.fit))",
            "                    .map(|row| (row.fit, row.fit))", T_USE, "rust"),
    # ---- the empty states ----
    Control("p93_empty_profiles_silent", PANEL,
            '        lines.push(Line::Text(format!("  {NO_PROFILES}"), Tone::Muted));\n', "", T_EMPTY, "rust"),
    Control("p93_empty_suggested_silent", PANEL,
            '        lines.push(Line::Text(format!("  {NOTHING_SUGGESTED}"), Tone::Muted));\n', "", T_EMPTY, "rust"),
    # ---- Suggested and the detail ----
    Control("p93_suggested_no_evidence", PANEL, "        let evidence = visible(&one_line(&proposal.evidence));",
            "        let evidence = String::new();", T_DETAIL, "rust"),
    Control("p93_detail_wrong_proposal", PANEL,
            "                    panel.detail = Some(Detail::new(proposal.id.clone()));",
            "                    panel.detail = Some(Detail::new(String::new()));", T_DETAIL, "rust"),
    # P45a fix round: the detail's text is the store's `target`, the file Accept writes.
    Control("p93_detail_body_cut", PANEL,
            "            for line in wrap(&visible(&target.after), cells.saturating_sub(2)) {",
            "            for line in wrap(&visible(&target.after), cells.saturating_sub(2)).into_iter().take(3) {",
            T_DETAIL, "rust"),
    Control("p93_detail_no_evidence", PANEL,
            "    for line in wrap(&visible(&proposal.evidence), cells.saturating_sub(2)) {",
            "    for line in wrap(&visible(&proposal.evidence), cells.saturating_sub(2)).into_iter().take(0) {",
            T_DETAIL, "rust"),
    Control("p93_detail_never_scrolls", PANEL, "        detail.scroll = detail.scroll.min(most);",
            "        detail.scroll = detail.scroll.min(0);", T_DETAIL, "rust"),
    # ---- Accept, only in the detail ----
    Control("p93_accept_from_the_list", PANEL,
            "            (KeyCode::Char('a'), _) => panel.message = Some(ACCEPT_IN_DETAIL.to_owned()),",
            "            (KeyCode::Char('a'), _) => {\n                if let Some(seen) = view.proposals.get(selected)"
            ".and_then(|known| view.seen.get(&known.id)).cloned() {\n"
            "                    self.profile_accept(&seen);\n"
            "                }\n            }", T_ACCEPT, "rust"),
    Control("p93_accept_no_note", PANEL, "                    (proposals::ACCEPT_NOTE.to_owned(), Tone::Attention),\n",
            "", T_ACCEPT, "rust"),
    Control("p93_accept_note_never_clears", PANEL, "        panel.notice.clear();\n", "", T_ACCEPT, "rust"),
    Control("p93_accept_key_gone", PANEL,
            "                KeyCode::Char('a') => match (accept_missing(detail), detail.shown.clone()) {",
            "                KeyCode::Char('A') => match (accept_missing(detail), detail.shown.clone()) {", T_ACCEPT,
            "rust"),
    Control("p93_accept_writes_nothing", PANEL, "        let result = proposals::accept_seen(&root, seen);",
            '        let result: Result<String, Refused> = Err(Refused::No(format!("{} not sent", seen.id)));',
            T_WRITES, "rust"),
    Control("p93_refusal_swallowed", PANEL,
            "            Err(Refused::No(why)) => panel.notice = vec![(why, Tone::Attention)],",
            '            Err(Refused::No(_)) => panel.notice = vec![("not accepted".to_owned(), Tone::Attention)],',
            T_REFUSED, "rust"),
    # ---- Dismiss ----
    Control("p93_dismiss_writes_nothing", PANEL, "        let result = proposals::try_dismiss(&root, id, None);",
            '        let result: Result<String, Refused> = Err(Refused::No(format!("{id} not sent")));', T_WRITES,
            "rust"),
    Control("p93_dismiss_not_in_the_list", PANEL,
            "            (KeyCode::Char('d'), Part::Suggested) => match panel.drawn_proposal.take() {",
            "            (KeyCode::Char('D'), Part::Suggested) => match panel.drawn_proposal.take() {", T_WRITES,
            "rust"),
    Control("p93_dismiss_not_in_the_detail", PANEL,
            "                KeyCode::Char('d') => {\n                    let id = detail.id.clone();",
            "                KeyCode::Char('D') => {\n                    let id = detail.id.clone();", T_REFUSED, "rust"),
    # ---- New ----
    Control("p93_new_refused_name_asks_a_shape", PANEL,
            "                        Some(why) => {\n                            panel.notice = vec![(format!(\"not made: {why}\"), Tone::Attention)]\n"
            "                        }\n",
            "                        Some(_) => panel.shaping = Some((name.clone(), 0)),\n", T_NEW, "rust"),
    # P45a fix round: New runs on a thread of its own, and lands in `profile_made`.
    Control("p93_new_no_shape_is_a_shape", PANEL,
            "                    None => crate::nirvana_profiles::new_profile(&root, &owned),",
            '                    None => crate::nirvana_profiles::new_profile_shape(&root, &owned, "reviewer"),', T_NEW,
            "rust"),
    Control("p93_new_shape_ignored", PANEL,
            "                            crate::nirvana_profiles::new_profile_shape(&root, &owned, shape)\n",
            "                            crate::nirvana_profiles::new_profile(&root, &owned)\n", T_NEW, "rust"),
    Control("p93_new_not_opened", PANEL,
            "                self.profile_open_tab(crate::nirvana::EDIT_FILE_COMMAND, &file, outcome);\n",
            "", T_NEW, "rust"),
    Control("p93_new_shapes_retyped", PANEL, "            .chain(crate::nirvana_profiles::SHAPES)",
            '            .chain(["reviewer"])', T_NEW, "rust"),
    # ---- e, Enter and K ----
    Control("p93_enter_does_not_edit", PANEL, "            (KeyCode::Enter | KeyCode::Char('e'), Part::Profiles) => {",
            "            (KeyCode::Char('e'), Part::Profiles) => {", T_EDIT, "rust"),
    Control("p93_cards_open_as_a_file", PANEL,
            "                Ok(()) => self.profile_edit_tab(crate::nirvana::EDIT_DIR_COMMAND, &dir, outcome),",
            "                Ok(()) => self.profile_edit_tab(crate::nirvana::EDIT_FILE_COMMAND, &dir, outcome),", T_EDIT,
            "rust"),
    # ---- keys, the wheel, clicks ----
    Control("p93_pagedown_stays", PANEL, "            KeyCode::PageDown => Some((selected + PAGE).min(last)),",
            "            KeyCode::PageDown => Some(selected),", T_KEYS, "rust"),
    Control("p93_tab_not_a_switch", PANEL,
            "            (KeyCode::Tab | KeyCode::BackTab, _) => panel.focus = panel.focus.other(),",
            "            (KeyCode::BackTab, _) => panel.focus = panel.focus.other(),", T_KEYS, "rust"),
    Control("p93_wheel_stays", PANEL, "                    (selected + WHEEL_STEP).min(count.saturating_sub(1))",
            "                    selected", T_KEYS, "rust"),
    Control("p93_click_does_not_select", PANEL, "            panel.select(&view, index);\n", "", T_KEYS, "rust"),
    Control("p93_heading_click_ignored", PANEL, "            panel.focus = *part;\n", "", T_KEYS, "rust"),
    Control("p93_selected_needs_colour", PANEL, '                    "\\u{25b8} "\n', '                    "  "\n',
            T_KEYS, "rust"),
    Control("p93_selection_by_index", PANEL, "        *selected = at;\n", "", T_FOLLOW, "rust"),
    Control("p93_esc_keeps_the_detail", PANEL, "                KeyCode::Esc | KeyCode::Char('q') => panel.detail = None,",
            "                KeyCode::Esc | KeyCode::Char('q') => {}", T_ESC, "rust"),
    # ---- widths and long text ----
    Control("p93_footer_never_short", PANEL, "    if width(&long) <= cells {", "    if true {", T_WIDTHS, "rust"),
    Control("p93_facts_on_one_line", PANEL,
            "        if !line.is_empty() && width(&line) + 3 + width(&part) > cells {", "        if false {", T_WIDTHS,
            "rust"),
    Control("p93_long_word_not_split", PANEL, "                if used + cell > cells && used > 0 {",
            "                if false {", T_LONG, "rust"),
    # ---- one panel, drawn on top ----
    Control("p93_panel_painted_over", PATCH,
            "    } else if state.nirvana_profile_panel.is_some() {\n        // Plan 45 P45a: the Profiles panel, for the "
            "same reason.\n        Some(\"client_surface_patch.fallback.nirvana_panel\")\n", "", T_REDRAWN, "rust"),
    Control("p93_strip_panel_left_open", PANEL,
            "    pub(super) fn open_profile_panel(&mut self, space: SpaceFolder) {\n        self.nirvana_preload = None;\n",
            "    pub(super) fn open_profile_panel(&mut self, space: SpaceFolder) {\n", T_ONE, "rust"),
    # ---- P45a fix round. The expected red, written before each run: the named test prints its FAIL line. ----
    # Finding 1 and 5: opening a detail is not reading it (red: Enter and `a` in one batch accept).
    Control("p93_fix_open_counts_as_read", PANEL,
            "                    panel.detail = Some(Detail::new(proposal.id.clone()));",
            "                    panel.detail = Some(Detail {\n                        drawn: true,\n"
            "                        read_to_end: true,\n"
            "                        shown: view.seen.get(&proposal.id).cloned(),\n"
            "                        ..Detail::new(proposal.id.clone())\n                    });",
            _T + "p93_fix_accept_waits_for_a_drawn_detail_read_to_its_end", "rust"),
    # ...the end must have been on screen (red: `a` accepts at the top of a long body).
    Control("p93_fix_end_never_needed", PANEL, "            detail.read_to_end = detail.covered >= detail_len;",
            "            detail.read_to_end = true;", _T + "p93_fix_accept_waits_for_a_drawn_detail_read_to_its_end",
            "rust"),
    # ...a mark while more is below (red: no "more below").
    Control("p93_fix_no_more_below", PANEL,
            '            place.push_str(&format!(" \\u{b7} \\u{25bc} {} unread", detail.below));\n', "",
            _T + "p93_fix_accept_waits_for_a_drawn_detail_read_to_its_end", "rust"),
    # ...the footer names `a` only when it acts (red: "a accept" on an unread detail).
    Control("p93_fix_footer_names_a_unread", PANEL, "        if accept_ready(detail) {", "        if true {",
            _T + "p93_fix_accept_waits_for_a_drawn_detail_read_to_its_end", "rust"),
    # Finding 3: a changed proposal reopens the detail from the top (red: the detail stays drawn and read).
    Control("p93_fix_changed_not_reopened", PANEL,
            "                panel.detail = Some(Detail::new(seen.id.clone()));\n", "",
            _T + "p93_fix_accept_holds_to_what_was_drawn", "rust"),
    # ...a re-read that changes an open detail starts the read again (red: read_to_end stays true).
    Control("p93_fix_reread_kept_read", PANEL,
            "                *detail = Detail::new(detail.id.clone());\n", "",
            _T + "p93_fix_a_detail_changed_while_open_is_read_again", "rust"),
    # ...the shown text is the store's own target, redaction included (red: the detail shows sk-FAKE).
    Control("p93_fix_shown_unredacted", "src/nirvana_profile_proposals.rs",
            "                target: target(root, &prepared(proposal.clone())),",
            "                target: target(root, &proposal),", _T + "p93_fix_accept_holds_to_what_was_drawn",
            "rust"),
    # Library: accept_seen holds to the proposal's sha and the target (red: the rewritten proposal is accepted).
    Control("p93_fix_store_seen_unchecked", "src/nirvana_profile_proposals.rs",
            "        if seen.sha != nirvana_lessons::sha256(&text) || seen.target != target {", "        if false {",
            "nirvana_profile_proposals::tests::p94_fix_accept_seen_writes_what_was_drawn_or_nothing", "rust"),
    # ...and to the profile the patch writes (red: a profile edited after the draw reads as stale, not changed).
    Control("p93_fix_store_target_unchecked", "src/nirvana_profile_proposals.rs",
            "        if seen.sha != nirvana_lessons::sha256(&text) || seen.target != target {",
            "        if seen.sha != nirvana_lessons::sha256(&text) {",
            "nirvana_profile_proposals::tests::p94_fix_accept_seen_writes_what_was_drawn_or_nothing", "rust"),
    # Finding 2: only a bare key acts (red: Ctrl+D dismisses).
    Control("p93_fix_modifiers_act", PANEL,
            "        if !bare && matches!(code, KeyCode::Char(_) | KeyCode::Enter) {",
            "        if false && !bare && matches!(code, KeyCode::Char(_) | KeyCode::Enter) {",
            _T + "p93_fix_only_a_bare_key_acts", "rust"),
    # Finding 4, the store: write refuses an invisible character (red: no "the body holds U+001B,").
    Control("p93_fix_store_write_takes_invisible", "src/nirvana_profile_proposals.rs",
            "    problems.extend(invisible_problems(&draft.body, &draft.evidence));\n", "",
            "nirvana_profile_proposals::tests::p94_fix_write_and_accept_refuse_what_the_panel_cannot_show", "rust"),
    # ...accept refuses it in a hand-written file (red: the panel's Accept writes the ESC).
    Control("p93_fix_store_accept_takes_invisible", "src/nirvana_profile_proposals.rs",
            "    problems.extend(invisible_problems(&proposal.body, &proposal.evidence));\n", "",
            _T + "p93_fix_invisible_characters_are_drawn_and_refused", "rust"),
    # ...a lone CR is not text (red: the CR draft is written).
    Control("p93_fix_store_cr_taken", "src/nirvana_profile_proposals.rs",
            "    (c.is_control() && c != '\\n' && c != '\\t')",
            "    (c.is_control() && c != '\\n' && c != '\\t' && c != '\\r')",
            "nirvana_profile_proposals::tests::p94_fix_write_and_accept_refuse_what_the_panel_cannot_show", "rust"),
    # ...bidi overrides (red: U+202E is written).
    Control("p93_fix_store_bidi_taken", "src/nirvana_profile_proposals.rs", "                | 0x2028..=0x202E\n",
            "                | 0x2028..=0x2029\n",
            "nirvana_profile_proposals::tests::p94_fix_write_and_accept_refuse_what_the_panel_cannot_show", "rust"),
    # ...the conjoining Hangul jamo the sweep found (red: the sweep finds U+1161 drawn as nothing).
    Control("p93_fix_store_jamo_taken", "src/nirvana_profile_proposals.rs", "                | 0x115F..=0x11FF\n",
            "                | 0x115F..=0x1160\n",
            "nirvana_profile_proposals::tests::p94_fix_nothing_the_store_takes_is_dropped_by_the_screen", "rust"),
    # Finding 4, the panel: the first layer names a refused character (red: the tag characters ride on "g" unseen).
    Control("p93_fix_invisible_not_named", PANEL, "            c if proposals::invisible(c) => named(&mut first, c),\n",
            "", _T + "p93_fix_invisible_characters_are_drawn_and_refused", "rust"),
    # ...the second layer names a grapheme the buffer drops (red: a lone U+0301 is not drawn).
    Control("p93_fix_dropped_not_named", PANEL,
            '        if grapheme == "\\n" || (!grapheme.contains(char::is_control) && width(grapheme) > 0) {',
            '        if grapheme == "\\n" || true {', _T + "p93_fix_invisible_characters_are_drawn_and_refused",
            "rust"),
    # ...a tab is drawn as spaces (red: "A    Tabbed." is "A<U+0009>Tabbed.").
    Control("p93_fix_tab_not_spaces", PANEL, "            '\\t' => first.push_str(TAB),",
            "            '\\t' => first.push('\\t'),", _T + "p93_fix_invisible_characters_are_drawn_and_refused",
            "rust"),
    # ...and in the list (red: the list's evidence hides U+200B).
    Control("p93_fix_list_not_visible", PANEL, "        let evidence = visible(&one_line(&proposal.evidence));",
            "        let evidence = one_line(&proposal.evidence);",
            _T + "p93_fix_invisible_characters_are_drawn_and_refused", "rust"),
    # Finding 6: Accept and Dismiss never wait on the lock (red: the key waits for the holder).
    Control("p93_fix_accept_waits_on_lock", PANEL, "        let result = proposals::accept_seen(&root, seen);",
            "        let result = proposals::accept(&root, &seen.id).map_err(Refused::No);",
            _T + "p93_fix_a_held_lock_never_waits", "rust"),
    Control("p93_fix_dismiss_waits_on_lock", PANEL, "        let result = proposals::try_dismiss(&root, id, None);",
            "        let result = proposals::dismiss(&root, id, None).map_err(Refused::No);",
            _T + "p93_fix_a_held_lock_never_waits", "rust"),
    # ...New does not either (red: the shape's Enter waits for the thread).
    Control("p93_fix_new_waits_on_lock", PANEL, "        match spawned {\n",
            "        match spawned.map(|made| made.join()) {\n", _T + "p93_fix_a_held_lock_never_waits", "rust"),
    # ...busy is said (red: no BUSY after `a`).
    Control("p93_fix_accept_busy_unsaid", PANEL,
            "            Err(Refused::Busy) => panel.message = Some(proposals::BUSY.to_owned()),",
            "            Err(Refused::Busy) => {}", _T + "p93_fix_a_held_lock_never_waits", "rust"),
    Control("p93_fix_dismiss_busy_unsaid", PANEL,
            "                panel.message = Some(proposals::BUSY.to_owned());\n                Err(Refused::Busy)",
            "                Err(Refused::Busy)", _T + "p93_fix_a_held_lock_never_waits", "rust"),
    # ...a busy `d` keeps its row for the press again (red: drawn_proposal is gone).
    Control("p93_fix_busy_row_lost", PANEL, "                            panel.drawn_proposal = Some(id);",
            "                            let _ = id;", _T + "p93_fix_a_held_lock_never_waits", "rust"),
    # ...keys wait while New works (red: no "working" message).
    Control("p93_fix_making_keys_silent", PANEL,
            '                panel.message = Some(format!("making {}: working, keys wait", making.name));',
            "                let _ = making;", _T + "p93_fix_a_held_lock_never_waits", "rust"),
    # Library: the try forms (red: accept_seen waits for the holder, then accepts).
    Control("p93_fix_store_accept_seen_waits", "src/nirvana_profile_proposals.rs",
            "    let _lock = try_lock(root).map_err(Refused::No)?.ok_or(Refused::Busy)?;\n    accept_locked(",
            "    let _lock = lock(root).map_err(Refused::No)?;\n    accept_locked(",
            "nirvana_profile_proposals::tests::p94_fix_the_try_forms_never_wait", "rust"),
    Control("p93_fix_store_try_dismiss_waits", "src/nirvana_profile_proposals.rs",
            "    let _lock = try_lock(root).map_err(Refused::No)?.ok_or(Refused::Busy)?;\n    dismiss_locked(",
            "    let _lock = lock(root).map_err(Refused::No)?;\n    dismiss_locked(",
            "nirvana_profile_proposals::tests::p94_fix_the_try_forms_never_wait", "rust"),
    # Finding 7: `d` dismisses the row drawn (red: a neighbour by index is dismissed).
    Control("p93_fix_dismiss_by_index", PANEL,
            "            (KeyCode::Char('d'), Part::Suggested) => match panel.drawn_proposal.take() {",
            "            (KeyCode::Char('d'), Part::Suggested) => match view.proposals.get(selected)"
            ".map(|known| known.id.clone()) {", _T + "p93_fix_dismiss_acts_on_the_row_drawn", "rust"),
    # ...once per draw (red: the second `d` says "no longer waiting", not DISMISS_NOT_DRAWN).
    Control("p93_fix_dismiss_twice_per_draw", PANEL,
            "            (KeyCode::Char('d'), Part::Suggested) => match panel.drawn_proposal.take() {",
            "            (KeyCode::Char('d'), Part::Suggested) => match panel.drawn_proposal.clone() {",
            _T + "p93_fix_dismiss_acts_on_the_row_drawn", "rust"),
    # ...a move clears it (red: Up then `d` dismisses the row drawn before the move).
    Control("p93_fix_move_keeps_drawn", PANEL, "        self.drawn_proposal = None;\n", "",
            _T + "p93_fix_dismiss_acts_on_the_row_drawn", "rust"),
    # Finding 8: the full path under the title (red: no path line names the project).
    Control("p93_fix_path_unsaid", PANEL, "        &middle(&visible(&panel.root.display().to_string()), inner),",
            "        &folder_name(&panel.root),", _T + "p93_fix_the_panel_says_the_project_path", "rust"),
    # ...cut in the middle, never at the end (red: at 80 columns it ends in "…").
    Control("p93_fix_path_cut_at_end", PANEL, "        &middle(&visible(&panel.root.display().to_string()), inner),",
            "        &fit(&visible(&panel.root.display().to_string()), inner),",
            _T + "p93_fix_the_panel_says_the_project_path", "rust"),
    # ...and New says the full path it wrote (red: "made" alone).
    Control("p93_fix_new_path_unsaid", PANEL, '                    (format!("made {}", file.display()), Tone::Plain),',
            '                    ("made".to_owned(), Tone::Plain),', T_NEW, "rust"),
    # Finding 9: a scan cut short says so (red: no "(partial)").
    Control("p93_fix_partial_unread", PANEL, "            .is_some_and(|detection| partial(&detection.skipped)),",
            "            .is_some_and(|_| false),", _T + "p93_fix_partial_use_counts_say_so", "rust"),
    Control("p93_fix_partial_count_unsaid", PANEL,
            '        (Some((carried, fit)), true) => format!("used {carried} of {fit} (partial)"),',
            '        (Some((carried, fit)), true) => format!("used {carried} of {fit}"),',
            _T + "p93_fix_partial_use_counts_say_so", "rust"),
    Control("p93_fix_partial_none_unsaid", PANEL, "        (Some((0, 0)), true) => NO_USE_PARTIAL.to_owned(),",
            "        (Some((0, 0)), true) => NO_USE.to_owned(),", _T + "p93_fix_partial_use_counts_say_so", "rust"),
    # Finding 10: single flight (red: a second read starts while the first runs).
    Control("p93_fix_reader_two_at_once", PANEL, "            if entry.reading || (fresh && !force) {",
            "            if fresh && !force {",
            _T + "p93_fix_the_reader_reads_one_at_a_time_and_full_only_while_drawn", "rust"),
    # ...full only while drawn (red: every read full).
    Control("p93_fix_reader_always_full", PANEL, "            let full = force || showing;",
            "            let full = true;",
            _T + "p93_fix_the_reader_reads_one_at_a_time_and_full_only_while_drawn", "rust"),
    # ...a stale read is dropped (red: the old read overwrites the refresh).
    Control("p93_fix_reader_stale_kept", PANEL, "        if entry.generation != generation {", "        if false {",
            _T + "p93_fix_a_read_older_than_a_refresh_is_dropped", "rust"),
    Control("p93_fix_reader_refresh_unbumped", PANEL,
            "        entry.generation = entry.generation.wrapping_add(1);\n", "",
            _T + "p93_fix_a_read_older_than_a_refresh_is_dropped", "rust"),
    # ...a panic does not leave "reading" on (red: the wait for the read's end times out).
    Control("p93_fix_reader_panic_stuck", PANEL,
            "            let read = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {\n"
            "                (reader.read)(&owned, want)\n            }));\n"
            "            reader.finish(&owned, generation, want, read.ok());",
            "            let read = (reader.read)(&owned, want);\n"
            "            reader.finish(&owned, generation, want, Some(read));",
            _T + "p93_fix_a_read_that_panics_does_not_stay_reading", "rust"),
    # Finding 11: a panel not drawn takes no key (red: `a` accepts in a window too small).
    Control("p93_fix_small_takes_keys", PANEL, "        if !panel.drawn {", "        if false {",
            _T + "p93_fix_a_window_too_small_takes_no_key", "rust"),
    Control("p93_fix_small_stays_drawn", PANEL, "    panel.drawn = false;\n    let panel_width", "    let panel_width",
            _T + "p93_fix_a_window_too_small_takes_no_key", "rust"),
    Control("p93_fix_small_unsaid", PANEL,
            "            put_text(\n                buffer,\n                area.x,\n                area.y,\n"
            "                area.width,\n                &said,\n                base.fg(palette.yellow),\n"
            "            );\n", "", _T + "p93_fix_a_window_too_small_takes_no_key", "rust"),
    # Finding 13: the shape range is the shapes there are (red: "0-5").
    Control("p93_fix_shape_range_retyped", PANEL, "            crate::nirvana_profiles::SHAPES.len()\n        )",
            "            crate::nirvana_profiles::SHAPES.len() + 1\n        )",
            _T + "p93_fix_the_shape_footer_counts_the_shapes", "rust"),
    # ---- P45a second fix round. The expected red, written before each run, is in the comment above each. ----
    # F1: no scroll opens a gap (red: "End skipped lines" in the 600-line case).
    Control("p93_fix2_end_opens_a_gap", PANEL, "        detail.scroll = detail.scroll.min(prefix + detail.covered);\n",
            "", _T + "p93_fix2_end_never_skips_the_middle", "rust"),
    # ...PageDown and the wheel neither (red: "PageDown x5" names a gap).
    Control("p93_fix2_page_opens_a_gap", PANEL, "        detail.scroll = detail.scroll.min(prefix + detail.covered);\n",
            "", _T + "p93_fix2_pagedown_the_wheel_and_down_open_no_gap", "rust"),
    # ...the count is from the mark, not the scroll (red: after Home, `below` is not the unread count before it).
    Control("p93_fix2_unread_from_the_scroll", PANEL,
            "        detail.below = detail_len.saturating_sub(detail.covered);",
            "        detail.below = body.len() - shown_to;", _T + "p93_fix2_back_up_and_end_again", "rust"),
    # ...a re-wrap starts the count again (red: at 100 columns the scroll is not 0).
    Control("p93_fix2_resize_keeps_the_count", PANEL, "                detail.covered = 0;\n", "",
            _T + "p93_fix2_a_resize_in_the_middle_starts_again", "rust"),
    # ...a height-only resize keeps it (red: the taller screen's scroll is 0).
    Control("p93_fix2_height_resets_the_count", PANEL,
            "            if detail.laid.is_some_and(|before| before != laid) {",
            "            if detail.laid.is_some() {", _T + "p93_fix2_a_resize_in_the_middle_starts_again", "rust"),
    # F2: `d` in the detail waits for a draw (red: "nothing dismissed unseen", one proposal left).
    Control("p93_fix2_d_unseen", PANEL,
            "                KeyCode::Char('d') if !detail_drawn(detail) => {\n"
            "                    panel.message = Some(DISMISS_DETAIL_NOT_DRAWN.to_owned())\n                }\n", "",
            _T + "p93_fix2_d_in_the_detail_waits_for_a_draw", "rust"),
    # F3: the indent is drawn (red: `lead("code block line")` is 3, not 7).
    Control("p93_fix2_indent_dropped", PANEL,
            '        let mut line = " ".repeat(indent);\n        let mut used = indent;\n',
            "        let mut line = String::new();\n        let mut used = 0;\n",
            _T + "p93_fix2_the_body_keeps_its_indent", "rust"),
    # ...a wrapped line keeps it (red: `wrap("    aaa bbb ccc", 11)` ends "ccc").
    Control("p93_fix2_hang_dropped", PANEL,
            "            if !first && used + 1 + word_width > cells {\n"
            '                out.push(std::mem::replace(&mut line, " ".repeat(hang)));\n                used = hang;\n',
            "            if !first && used + 1 + word_width > cells {\n"
            "                out.push(std::mem::take(&mut line));\n                used = 0;\n",
            _T + "p93_fix2_the_body_keeps_its_indent", "rust"),
    # ...a line of spaces is empty (red: `wrap("   ", 10)` is ["   "]).
    Control("p93_fix2_spaces_line_drawn", PANEL,
            "        if words.is_empty() {\n            out.push(String::new());\n            continue;\n        }\n", "",
            _T + "p93_fix2_the_body_keeps_its_indent", "rust"),
    # ...a tab is still spaces (red: `lead("Tabbed")` is 3, the tab named <U+0009>).
    Control("p93_fix2_tab_not_spaces", PANEL, "            '\\t' => first.push_str(TAB),",
            "            '\\t' => first.push('\\t'),", _T + "p93_fix2_the_body_keeps_its_indent", "rust"),
    # F4: a New is kept by project (red: the reopened panel has no "after the panel closed").
    Control("p93_fix2_new_forgotten", PANEL, "                unlanded().insert(project, making.clone());\n", "",
            _T + "p93_fix2_esc_while_making_keeps_the_result", "rust"),
    # ...a New done while closed is a notice, not a live landing (red: no "after the panel closed").
    Control("p93_fix2_late_lands_live", PANEL,
            "            match making.take() {\n                Some(made) => {\n"
            "                    unlanded().remove(&panel.root);\n"
            "                    landed_late(&mut panel, &making.name, made);\n                }\n"
            "                None => panel.making = Some(making),\n            }\n",
            "            panel.making = Some(making);\n", _T + "p93_fix2_esc_while_making_keeps_the_result", "rust"),
    # ...said once (red: "nothing left making", the third panel says making for good).
    Control("p93_fix2_late_said_twice", PANEL, "                    unlanded().remove(&panel.root);\n", "",
            _T + "p93_fix2_esc_while_making_keeps_the_result", "rust"),
    # ...a New landed in a panel is not landed again (red: the panel after `settle` says making).
    Control("p93_fix2_landed_kept", PANEL, "        unlanded().remove(&root);\n", "",
            _T + "p93_fix2_esc_while_making_keeps_the_result", "rust"),
    # ...on its own project only (red: the other project's panel gets the notice).
    Control("p93_fix2_other_project_lands", PANEL, "        let pending = unlanded().get(&panel.root).cloned();",
            "        let pending = unlanded().values().next().cloned();",
            _T + "p93_fix2_esc_while_making_keeps_the_result", "rust"),
    # F6: a profile's name is drawn visible (red: "the row at 140").
    Control("p93_fix2_profile_name_raw", PANEL,
            "    let names: Vec<String> = view.profiles.iter().map(|row| visible(&row.name)).collect();",
            "    let names: Vec<String> = view.profiles.iter().map(|row| row.name.clone()).collect();",
            _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # ...the title's folder (red: "the title at 140").
    Control("p93_fix2_title_raw", PANEL, "    let folder = visible(&folder_name(&panel.root));",
            "    let folder = folder_name(&panel.root);", _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # ...the path line (red: "the path line at 140").
    Control("p93_fix2_path_raw", PANEL, "        &middle(&visible(&panel.root.display().to_string()), inner),",
            "        &middle(&panel.root.display().to_string(), inner),",
            _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # ...a proposal's target profile in the list (red: "the proposal's row at 140").
    Control("p93_fix2_target_raw_in_list", PANEL,
            "            visible(&proposal.profile)\n        );\n        let evidence",
            "            proposal.profile.clone()\n        );\n        let evidence",
            _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # ...and in the detail (red: "the detail at 140").
    Control("p93_fix2_target_raw_in_detail", PANEL,
            "    let (id, profile) = (visible(&proposal.id), visible(&proposal.profile));",
            "    let (id, profile) = (visible(&proposal.id), proposal.profile.clone());",
            _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # ...a notice naming it (red: "the notice at 140").
    Control("p93_fix2_notice_raw", PANEL, "                    wrap(&visible(said), inner)",
            "                    wrap(said, inner)", _T + "p93_fix2_names_from_disk_are_drawn_visible", "rust"),
    # F7: the scan is not every read (red: the second read is Want { full: true, usage: true }).
    Control("p93_fix2_usage_every_read", PANEL, "            let usage = full\n                && (entry.usage_due",
            "            let usage = full\n                && (true || entry.usage_due",
            _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval", "rust"),
    # ...a refresh asks for one (red: "a refresh asks for a scan").
    Control("p93_fix2_refresh_not_due", PANEL,
            "            keep_usage(&mut fresh, old);\n        }\n        entry.usage_due = true;\n",
            "            keep_usage(&mut fresh, old);\n        }\n",
            _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval", "rust"),
    # ...an open asks for one (red: `reader.usage_due(folder)` is false after `opened`).
    Control("p93_fix2_open_not_due", PANEL,
            "        entry.asked_at = Some(Instant::now());\n        entry.usage_due = true;\n    }",
            "        entry.asked_at = Some(Instant::now());\n    }",
            _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval", "rust"),
    # ...and the window's panel asks it when it opens (red: `usage_asked` is false).
    Control("p93_fix2_open_asks_nothing", PANEL, "        reader().opened(&panel.folder);\n", "",
            _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval", "rust"),
    # ...a read without a scan keeps the counts (red: "the counts are kept").
    Control("p93_fix2_counts_dropped", PANEL, "                keep_usage(&mut view, old);",
            "                let _ = old;", _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval",
            "rust"),
    # ...past the interval, a scan (red: "4 s after a scan, past 4 s").
    Control("p93_fix2_interval_ignored", PANEL,
            "                        .is_none_or(|at| at.elapsed() >= self.usage_every));",
            "                        .is_none());",
            _T + "p93_fix2_the_use_scan_runs_when_asked_and_at_most_every_interval", "rust"),
    # F8: a missing order copy is not partial (red: "(partial)" on the screen).
    Control("p93_fix2_partial_any_skip", PANEL,
            "    skipped.runs > 0 || skipped.journals > 0 || skipped.orders > 0 || skipped.budget",
            "    skipped.any()", _T + "p93_fix2_a_missing_order_copy_is_not_partial", "rust"),
    # ...a run past the cap is (red: p93_fix_partial_use_counts_say_so finds no " (partial)").
    Control("p93_fix2_partial_runs_ignored", PANEL,
            "    skipped.runs > 0 || skipped.journals > 0 || skipped.orders > 0 || skipped.budget",
            "    skipped.journals > 0 || skipped.orders > 0 || skipped.budget",
            _T + "p93_fix_partial_use_counts_say_so", "rust"),
)
