# Plan 46 P46b (the owner, 2026-11-19: "In this modal I would like to see a submenu wher we have a table for the 3
# different modes efficent balanced and SOTA. I would like to see the seating table with the task and model type.").
# Controls starting with p98_. One sabotage per assertion, each on a product line of the panel; the test named in the
# fifth field is the one expected to print its red line with the sabotage in place (written before the run).
# `Control` is provided by controls.py.
PANEL = "src/client/shell/nirvana_seats.rs"

_T = "client::shell::tests::nirvana_seats::"
T_STRIP = _T + "p98_the_page_strip_names_both_pages_and_brackets_the_one_shown"
T_SWITCH = _T + "p98_t_tab_and_a_click_on_a_page_word_switch_pages"
T_MODES = _T + "p98_three_mode_columns_with_the_projects_mode_marked"
T_CELLS = _T + "p98_each_cell_is_the_matrix_cell_given"
T_MARKS = _T + "p98_the_forced_unread_none_held_and_measured_marks"
T_SELECT = _T + "p98_selection_moves_with_the_keys_and_stops_at_both_ends"
T_CLICK = _T + "p98_a_click_selects_a_table_row_and_the_wheel_moves_it"
T_SCROLL = _T + "p98_a_long_matrix_scrolls_and_keeps_its_header"
T_PLAIN = _T + "p98_the_detail_of_a_plain_row"
T_FORCED = _T + "p98_the_detail_of_a_forced_row_names_the_cool_pick"
T_HELD = _T + "p98_the_detail_of_a_held_row_never_reads_as_the_booking"
T_NONE = _T + "p98_the_detail_of_a_row_with_no_seat_and_no_record"
T_ORCH = _T + "p98_the_detail_of_the_orchestrator_row"
T_RECORD = _T + "p98_the_record_line_shortens_to_fit"
T_CHECK = _T + "p98_the_check_line_is_checked_line"
T_REFUSED = _T + "p98_no_matrix_says_so_in_one_line_and_never_panics"
T_WIDTHS = _T + "p98_80_100_and_140_columns"
T_SHORT = _T + "p98_a_short_panel_shrinks_the_detail_before_the_table"
T_GOLDEN = _T + "p98_the_orders_page_is_drawn_as_before"
T_DIFFERS = _T + "p98_the_differs_line_is_the_views_on_both_pages"
T_FOOTER = _T + "p98_the_footer_names_the_page_keys_on_both_pages"

CONTROLS = (
    # ---- the pages ----
    Control("p98_strip_no_brackets", PANEL, '                format!("[{}]", page.word())',
            '                format!(" {} ", page.word())', T_STRIP, "rust"),
    Control("p98_strip_not_clickable", PANEL, "        pages.push((Rect::new(at, rect.y, width, 1), page));\n", "",
            T_STRIP, "rust"),
    Control("p98_t_does_not_switch", PANEL, "            panel.page = panel.page.other();\n", "", T_SWITCH, "rust"),
    Control("p98_tab_not_a_page_key", PANEL, "KeyCode::Char('t') | KeyCode::Tab | KeyCode::BackTab",
            "KeyCode::Char('t') | KeyCode::BackTab", T_SWITCH, "rust"),
    Control("p98_page_click_ignored", PANEL, "                panel.page = page;\n", "", T_SWITCH, "rust"),
    # ---- the columns and cells ----
    Control("p98_live_mode_unmarked", PANEL, '            format!(" [{}]", mode.title())',
            '            format!("  {}", mode.title())', T_MODES, "rust"),
    Control("p98_cell_not_by_mode", PANEL, "    row.cells.iter().find(|cell| cell.mode == mode)",
            "    row.cells.iter().rev().find(|_| true)", T_CELLS, "rust"),
    Control("p98_model_cut_not_dropped", PANEL, "        if width(&with_model) <= room {", "        if true {",
            T_WIDTHS, "rust"),
    Control("p98_effort_never_short", PANEL, '    fit(&format!("{seat} {}", effort_short(effort)), room)',
            "    fit(&full, room)", T_WIDTHS, "rust"),
    Control("p98_table_page_as_narrow_as_orders", PANEL, "min(TABLE_WIDTH);", "min(SEATS_WIDTH);", T_WIDTHS, "rust"),
    # ---- the marks ----
    Control("p98_forced_unmarked", PANEL, "    } else if cell.forced {\n        '!'", "    } else if false {\n        '!'",
            T_MARKS, "rust"),
    Control("p98_unread_and_forced_reads_hot", PANEL, "    let usage = if cell.unread {\n        '?'",
            "    let usage = if cell.unread && !cell.forced {\n        '?'", T_MARKS, "rust"),
    Control("p98_held_unmarked", PANEL, "    let calibration = if cell.held {", "    let calibration = if false {",
            T_MARKS, "rust"),
    Control("p98_measured_unmarked", PANEL, "    } else if cell.measured.is_some() {\n        '~'",
            "    } else if false {\n        '~'", T_MARKS, "rust"),
    Control("p98_no_seat_blank", PANEL, '            None => fit("no seat", room),', "            None => String::new(),",
            T_MARKS, "rust"),
    Control("p98_unread_said_as_hot", PANEL, '            "usage unread, not hot: cool pick {}",',
            '            "hot usage moved it: cool pick {}",', T_MARKS, "rust"),
    Control("p98_legend_missing", PANEL, "            notes.push((TABLE_LEGEND.into(), Tone::Muted));\n", "", T_MARKS,
            "rust"),
    # ---- selection and scroll ----
    Control("p98_down_unbounded", PANEL, "            KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(last),",
            "            KeyCode::Down | KeyCode::Char('j') => selected + 1,", T_SELECT, "rust"),
    Control("p98_page_down_one_row", PANEL, "            KeyCode::PageDown => (selected + TABLE_PAGE).min(last),",
            "            KeyCode::PageDown => (selected + 1).min(last),", T_SELECT, "rust"),
    Control("p98_home_stays", PANEL, "            KeyCode::Home => 0,", "            KeyCode::Home => selected,", T_SELECT,
            "rust"),
    Control("p98_end_stays", PANEL, "            KeyCode::End => last,", "            KeyCode::End => selected,", T_SELECT,
            "rust"),
    Control("p98_enter_focuses_from_the_table", PANEL, "            KeyCode::Enter => selected,\n", "", T_SELECT, "rust"),
    Control("p98_row_click_ignored", PANEL, "                panel.table_selected = index;\n", "", T_CLICK, "rust"),
    Control("p98_wheel_dead", PANEL, "                    (panel.table_selected + WHEEL_STEP).min(last)",
            "                    panel.table_selected", T_CLICK, "rust"),
    Control("p98_scroll_does_not_follow", PANEL, "            panel.table_scroll = panel.table_selected + 1 - shown_rows;",
            "            panel.table_scroll = 0;", T_SCROLL, "rust"),
    Control("p98_selection_not_held_to_rows", PANEL,
            "    panel.table_selected = panel.table_selected.min(count.saturating_sub(1));\n", "", T_REFUSED, "rust"),
    # ---- the detail ----
    Control("p98_live_mode_unmarked_in_detail", PANEL, '            if live == Some(mode) { "\\u{25b8} " }',
            '            if false { "\\u{25b8} " }', T_PLAIN, "rust"),
    Control("p98_next_unsaid", PANEL, '        facts.push(format!("next {seat} {}", effort.word()));\n', "", T_PLAIN,
            "rust"),
    Control("p98_off_unsaid", PANEL, "        if !seat.enabled {\n", "        if false {\n", T_PLAIN, "rust"),
    Control("p98_no_record_unsaid", PANEL, "        if seat.record.is_empty() {\n", "        if false {\n", T_PLAIN, "rust"),
    Control("p98_thin_reads_measured", PANEL, "        row.status(),\n        row.orders,\n        row.first_pass,\n"
            "        row.gated,\n        row.recent\n    );\n",
            '        "measured",\n        row.orders,\n        row.first_pass,\n        row.gated,\n        row.recent\n    );\n',
            T_PLAIN, "rust"),
    Control("p98_cool_pick_unsaid", PANEL, '            "hot usage moved it: cool pick {}",',
            '            "hot usage moved it{}",', T_FORCED, "rust"),
    Control("p98_facts_never_wrap", PANEL,
            "        if unicode_width::UnicodeWidthStr::width(one.as_str()) <= room || facts.is_empty() {",
            "        if true {", T_FORCED, "rust"),
    Control("p98_held_reads_as_booking", PANEL, "        facts.push(if cell.held {", "        facts.push(if false {",
            T_HELD, "rust"),
    Control("p98_none_reason_unsaid", PANEL, '        facts.push(format!("no seat fits: {why}"));\n', "", T_NONE, "rust"),
    Control("p98_orchestrator_lists_seats", PANEL, "    if row.orchestrator {\n        return lines;\n    }\n", "",
            T_ORCH, "rust"),
    Control("p98_record_never_short", PANEL,
            "    if unicode_width::UnicodeWidthStr::width(long.as_str()) <= room {", "    if true {", T_RECORD, "rust"),
    # ---- the notes ----
    Control("p98_check_line_missing", PANEL,
            "            notes.push((square::checked_line(matrix, now), Tone::Muted));\n", "", T_CHECK, "rust"),
    Control("p98_refused_unsaid", PANEL, "        (Some(_), None) => notes.push((TABLE_REFUSED.into(), Tone::Attention)),",
            "        (Some(_), None) => {}", T_REFUSED, "rust"),
    Control("p98_note_unsaid", PANEL, "    if let Some(note) = &shown.note {",
            "    if let Some(note) = None::<&String> {", T_REFUSED, "rust"),
    Control("p98_short_row_panics", PANEL, "    row.cells.iter().find(|cell| cell.mode == mode)",
            "    Some(&row.cells[MODES.iter().position(|m| *m == mode).unwrap_or(0)])", T_REFUSED, "rust"),
    Control("p98_differs_line_fixed", PANEL, "    view.differs_line\n        .clone()",
            "    None::<String>\n        .clone()", T_DIFFERS, "rust"),
    Control("p98_table_differs_missing", PANEL, "    if shown.differs {", "    if false {", T_DIFFERS, "rust"),
    # ---- sizes ----
    Control("p98_header_is_a_row", PANEL, "    let mut left = avail.saturating_sub(1);", "    let mut left = avail;",
            T_SHORT, "rust"),
    Control("p98_detail_before_the_first_rows", PANEL, "    let first = rows.min(TABLE_MIN_ROWS).min(left);",
            "    let first = 0;", T_SHORT, "rust"),
    Control("p98_cut_detail_unsaid", PANEL, "            let text = if cut && place + 1 == room {",
            "            let text = if false {", T_SHORT, "rust"),
    # ---- the Orders page and the footers ----
    Control("p98_orders_body_moved", PANEL, "    let mut lines = vec![Line::Mode];",
            "    let mut lines = vec![Line::Text(String::new(), Tone::Plain), Line::Mode];", T_GOLDEN, "rust"),
    Control("p98_title_cut_for_the_strip", PANEL,
            "    let title_room = usize::from(strip_x.saturating_sub(rect.x + 3));", "    let title_room = 14;",
            T_GOLDEN, "rust"),
    Control("p98_orders_footer_no_page_key", PANEL,
            '        (false, false) => {\n            "t/tab table \\u{b7} ', '        (false, false) => {\n            "',
            T_FOOTER, "rust"),
    Control("p98_table_footer_no_page_key", PANEL,
            '        "t/tab orders \\u{b7} \\u{2191}/\\u{2193} pgup/pgdn row \\u{b7} m mode \\u{b7} esc close"',
            '        "\\u{2191}/\\u{2193} pgup/pgdn row \\u{b7} m mode \\u{b7} esc close"', T_FOOTER, "rust"),
)
