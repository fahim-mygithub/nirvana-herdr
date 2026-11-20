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
            '            let refused = format!("refused: herdr profile check {}", row.name);',
            '            let refused = String::from("refused");', T_ROWS, "rust"),
    # ---- the use ----
    Control("p93_use_nothing_as_a_count", PANEL, "        Some((0, 0)) => NO_USE.to_owned(),",
            '        Some((0, 0)) => "used 0 of 0".to_owned(),', T_USE, "rust"),
    Control("p93_use_fit_of_carried", PANEL, '        Some((carried, fit)) => format!("used {carried} of {fit}"),',
            '        Some((carried, fit)) => format!("used {fit} of {carried}"),', T_USE, "rust"),
    Control("p93_use_fit_as_carried", PANEL, "                    .map(|row| (row.carried, row.fit))",
            "                    .map(|row| (row.fit, row.fit))", T_USE, "rust"),
    # ---- the empty states ----
    Control("p93_empty_profiles_silent", PANEL,
            '        lines.push(Line::Text(format!("  {NO_PROFILES}"), Tone::Muted));\n', "", T_EMPTY, "rust"),
    Control("p93_empty_suggested_silent", PANEL,
            '        lines.push(Line::Text(format!("  {NOTHING_SUGGESTED}"), Tone::Muted));\n', "", T_EMPTY, "rust"),
    # ---- Suggested and the detail ----
    Control("p93_suggested_no_evidence", PANEL, "        let evidence = one_line(&proposal.evidence);",
            "        let evidence = String::new();", T_DETAIL, "rust"),
    Control("p93_detail_wrong_proposal", PANEL, "                        id: proposal.id.clone(),",
            "                        id: String::new(),", T_DETAIL, "rust"),
    Control("p93_detail_body_cut", PANEL, "    for line in wrap(&proposal.body, cells.saturating_sub(2)) {",
            "    for line in wrap(&proposal.body, cells.saturating_sub(2)).into_iter().take(3) {", T_DETAIL, "rust"),
    Control("p93_detail_no_evidence", PANEL, "    for line in wrap(&proposal.evidence, cells.saturating_sub(2)) {",
            "    for line in wrap(&proposal.evidence, cells.saturating_sub(2)).into_iter().take(0) {", T_DETAIL,
            "rust"),
    Control("p93_detail_never_scrolls", PANEL, "        detail.scroll = detail.scroll.min(most);",
            "        detail.scroll = detail.scroll.min(0);", T_DETAIL, "rust"),
    # ---- Accept, only in the detail ----
    Control("p93_accept_from_the_list", PANEL,
            "            (KeyCode::Char('a'), _) => panel.message = Some(ACCEPT_IN_DETAIL.to_owned()),",
            "            (KeyCode::Char('a'), _) => {\n                if let Some(proposal) = view.proposals.get(selected) {\n"
            "                    let id = proposal.id.clone();\n                    self.profile_accept(&id);\n"
            "                }\n            }", T_ACCEPT, "rust"),
    Control("p93_accept_no_note", PANEL, "                    (proposals::ACCEPT_NOTE.to_owned(), Tone::Attention),\n",
            "", T_ACCEPT, "rust"),
    Control("p93_accept_note_never_clears", PANEL, "        panel.notice.clear();\n", "", T_ACCEPT, "rust"),
    Control("p93_accept_key_gone", PANEL,
            "                KeyCode::Char('a') => {\n                    let id = detail.id.clone();",
            "                KeyCode::Char('A') => {\n                    let id = detail.id.clone();", T_ACCEPT, "rust"),
    Control("p93_accept_writes_nothing", PANEL, "        let result = proposals::accept(&root, id);",
            '        let result: Result<String, String> = Err(format!("{id} not sent"));', T_WRITES, "rust"),
    Control("p93_refusal_swallowed", PANEL,
            "            }\n            Err(why) => self.profile_notice(vec![(why, Tone::Attention)]),\n        }\n    }\n\n"
            "    fn profile_dismiss(",
            '            }\n            Err(_) => self.profile_notice(vec![("not accepted".to_owned(), Tone::Attention)]),\n'
            "        }\n    }\n\n    fn profile_dismiss(", T_REFUSED, "rust"),
    # ---- Dismiss ----
    Control("p93_dismiss_writes_nothing", PANEL, "        let result = proposals::dismiss(&root, id, None);",
            '        let result: Result<String, String> = Err(format!("{id} not sent"));', T_WRITES, "rust"),
    Control("p93_dismiss_not_in_the_list", PANEL,
            "            (KeyCode::Char('d'), Part::Suggested) => match view.proposals.get(selected) {",
            "            (KeyCode::Char('D'), Part::Suggested) => match view.proposals.get(selected) {", T_WRITES, "rust"),
    Control("p93_dismiss_not_in_the_detail", PANEL,
            "                KeyCode::Char('d') => {\n                    let id = detail.id.clone();",
            "                KeyCode::Char('D') => {\n                    let id = detail.id.clone();", T_REFUSED, "rust"),
    # ---- New ----
    Control("p93_new_refused_name_asks_a_shape", PANEL,
            "                        Some(why) => {\n                            panel.notice = vec![(format!(\"not made: {why}\"), Tone::Attention)]\n"
            "                        }\n",
            "                        Some(_) => panel.shaping = Some((name.clone(), 0)),\n", T_NEW, "rust"),
    Control("p93_new_no_shape_is_a_shape", PANEL, "            None => crate::nirvana_profiles::new_profile(&root, name),",
            '            None => crate::nirvana_profiles::new_profile_shape(&root, name, "reviewer"),', T_NEW, "rust"),
    Control("p93_new_shape_ignored", PANEL,
            "                Some(shape) => crate::nirvana_profiles::new_profile_shape(&root, name, shape),",
            "                Some(_) => crate::nirvana_profiles::new_profile(&root, name),", T_NEW, "rust"),
    Control("p93_new_not_opened", PANEL,
            "            Ok(file) => self.profile_edit_tab(crate::nirvana::EDIT_FILE_COMMAND, &file, outcome),",
            "            Ok(_) => {}", T_NEW, "rust"),
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
    Control("p93_footer_never_short", PANEL, "    if width(long) <= cells {", "    if true {", T_WIDTHS, "rust"),
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
)
