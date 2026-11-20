# Plan 47 P47a (the owner, 2026-11-20: "Im hoping for the UI under teh agent panal to showcase when we are using that
# build agent profile for a task."). The agents panel's live worker rows, the spaces view's worker line and the Profiles
# panel show the profile an order carries. Controls starting with p96_row_: the jobs-sizing package already holds the
# bare p96_ prefix (p96_jobs_sizing.py). One sabotage per assertion, each on a product line; the test named in the
# fifth field is the one expected to print its red line with the sabotage in place, and the comment above each says
# which of its assertions goes red and why (written before the run). `Control` is provided by controls.py.
#
# Fix round (review F1, F3, E): the profile has a line of its own under a live worker's model and time, so the line-2
# controls of the first round went with their code; p96_row_fix_* are the new ones.
SUBAGENTS = "src/nirvana_subagents.rs"
SIDEBAR = "src/client/shell/agent_sidebar.rs"
SPACES = "src/client/shell/nirvana_spaces.rs"
PANEL = "src/client/shell/nirvana_profile_panel.rs"

T_ROWS = "nirvana_subagents::tests::p96_row_rows_carry_the_profile_of_their_own_seated_line"
T_LIVE = "nirvana_subagents::tests::p96_row_live_profiles_count_running_orders_of_this_folder_only"
T_ONCE = "nirvana_subagents::tests::p96_row_fix_live_profiles_count_an_order_once"
T_ASK = "nirvana_subagents::tests::p96_row_fix_live_profiles_ask_once_per_folder"
_S = "client::shell::agent_sidebar::tests::"
T_SWEEP = _S + "p96_row_fix_every_width_a_profiled_row_is_three_lines"
T_REAL = _S + "p96_row_fix_the_four_real_names_show_whole_at_26_cells"
T_HEIGHTS = _S + "p96_row_fix_heights_scroll_and_the_cap_agree_with_what_is_drawn"
T_CLICK = _S + "p96_row_fix_a_click_on_the_profile_line_acts_as_one_on_the_name"
T_BARE = _S + "p96_row_fix_without_a_profile_the_panel_is_the_profiled_one_less_its_profile_lines"
T_INVISIBLE = _S + "p96_row_an_invisible_character_in_a_profile_name_is_named"
T_DUMP = _S + "p96_row_dump_the_agents_panel_with_three_live_workers"
_W = "client::shell::tests::nirvana_space_workers::"
T_SPACE_LINE = _W + "p96_row_a_worker_line_shows_its_profile_where_its_line_has_room"
T_SPACE_COLOUR = _W + "p96_row_the_spaces_worker_profile_is_drawn_in_its_own_colour"
T_SPACE_CARRY = _W + "p96_row_space_workers_carry_the_rows_profile"
T_PANEL = "client::shell::tests::nirvana_profile_panel::p96_row_a_profile_in_use_now_says_so_with_the_count"

KEY_ARM = "        Some(Some(name)) if crate::nirvana_profiles::valid_name(name.trim()) => {\n"
ROW_SET = "                profile: seated_profile(&order.seated),\n"
RUNNING = "                    .filter(|row| row.state == SubagentState::Running)\n"
SHOWN = "    let shown = super::nirvana_profile_panel::visible(name).replace('\\n', \"<U+000A>\");\n"
MIN = "    (room >= PROFILE_MIN).then(|| profile_line(name, room))\n"
ROW_LINES = "    2 + usize::from(sub.profile.is_some())\n"
LINE3 = "            if let Some(name) = sub.profile.as_deref() {\n                if *current_y >= rect.bottom() {\n"
LINE3_STYLE = (
    "                        Style::default()\n"
    "                            .fg(palette.mauve)\n"
    "                            .add_modifier(Modifier::BOLD),\n"
)
LINE3_ROOM = "                        profile_line(name, (rect.width as usize).saturating_sub(6)),\n"
LINE3_DRAW = (
    "                        profile_line(name, (rect.width as usize).saturating_sub(6)),\n"
    "                        Style::default()\n"
    "                            .fg(palette.mauve)\n"
    "                            .add_modifier(Modifier::BOLD),\n"
    "                    ),\n"
    "                ];\n"
    "                Paragraph::new(Line::from(line3_spans))\n"
    "                    .render(Rect::new(rect.x, *current_y, rect.width, 1), buffer);\n"
)
LINE2_MODEL = "                ratatui::text::Span::styled(short_m, Style::default().fg(palette.overlay1)),\n"
HIT = "                (live_row_lines(sub) as u16).min(rect.bottom().saturating_sub(*current_y));\n"
CAP_ROW = "                let lines = live_row_lines(live_w);\n"
PANEL_HEIGHT = "        |row| agent_row_lines(row, area.width),\n"
LIVE_ROW = "                        Some((folder.clone(), order, row.profile.clone()?))\n"
LIVE_ONCE = "        if ok && counted.insert(order) {\n"
LIVE_ASK = "        let ok = *accepted\n            .entry(folder)\n            .or_insert_with_key(|folder| here(folder));\n"

CONTROLS = (
    # ---- the row's profile, from its own seated line (nirvana_subagents) ----
    # The key ignored: o-key reads the reason's scholar, not its key's builder.
    Control("p96_row_key_ignored", SUBAGENTS, KEY_ARM,
            "        Some(Some(name)) if false && crate::nirvana_profiles::valid_name(name.trim()) => {\n",
            T_ROWS, "rust"),
    # A key that is no name trusted: o-badkey reads "Not A Name", not the reason's scholar.
    Control("p96_row_key_unchecked", SUBAGENTS, KEY_ARM, "        Some(Some(name)) => {\n", T_ROWS, "rust"),
    # A written null read as unknown: o-null reads the reason's scholar, not none.
    Control("p96_row_null_reads_reason", SUBAGENTS, "        Some(None) => None,\n",
            "        Some(None) => crate::nirvana_profiles::from_reason(&seated.reason),\n", T_ROWS, "rust"),
    # No fallback for a line from before 0.90.0: o-absent reads none, not scholar.
    Control("p96_row_absent_no_fallback", SUBAGENTS,
            "        _ => crate::nirvana_profiles::from_reason(&seated.reason),\n", "        _ => None,\n",
            T_ROWS, "rust"),
    # The row never set: o-key reads none.
    Control("p96_row_not_set", SUBAGENTS, ROW_SET, "                profile: None,\n", T_ROWS, "rust"),
    # A send-back child left without its own line's profile: o-child reads none, not reviewer.
    Control("p96_row_child_without_its_own", SUBAGENTS, ROW_SET,
            "                profile: seated_profile(&order.seated).filter(|_| order.seated.parent.is_none()),\n",
            T_ROWS, "rust"),
    # Fix round (review H): the gated parent listed: o-parent reads Some(Some("builder")), not None (no row).
    Control("p96_row_fix_gated_parent_listed", SUBAGENTS, "                | crate::nirvana_run::Phase::Gated\n", "",
            T_ROWS, "rust"),

    # ---- live_profiles ----
    # A finished order counted: builder reads 3 (a5 is Done), scholar 2 (a6 is Blocked).
    Control("p96_row_live_counts_finished", SUBAGENTS, RUNNING, "", T_LIVE, "rust"),
    # Another folder's order counted: builder reads 3 (b1 is in project-b).
    Control("p96_row_live_any_folder", SUBAGENTS, LIVE_ASK,
            "        let ok = *accepted\n            .entry(folder)\n            .or_insert_with_key(|_| true);\n",
            T_LIVE, "rust"),
    # An order with no profile counted, under "": the map gains a "" key (a4).
    Control("p96_row_live_counts_none", SUBAGENTS, LIVE_ROW,
            "                        Some((folder.clone(), order, row.profile.clone().unwrap_or_default()))\n",
            T_LIVE, "rust"),
    # Fix round (F3): an order counted under each pane that holds it: builder reads 3, not 2.
    Control("p96_row_fix_live_counted_twice", SUBAGENTS, LIVE_ONCE,
            "        if ok && (counted.insert(order) || true) {\n", T_ONCE, "rust"),
    # Fix round (E): `here` asked once per row: asked reads 6, not 1.
    Control("p96_row_fix_live_asked_per_row", SUBAGENTS, LIVE_ASK, "        let ok = here(&folder);\n", T_ASK,
            "rust"),

    # ---- the Profiles panel's "in use now" ----
    # The count never filled: builder's facts lack "in use: 2 now".
    Control("p96_row_panel_not_counted", PANEL, "                live: live.get(&name).copied().unwrap_or(0),\n",
            "                live: 0,\n", T_PANEL, "rust"),
    # The count never shown: builder's facts lack "in use: 2 now".
    Control("p96_row_panel_not_shown", PANEL,
            "        if let Some(live) = live_text(row) {\n            facts.push(live);\n        }\n", "",
            T_PANEL, "rust"),
    # A count of 0 shown: spare's facts read "in use: 0 now".
    Control("p96_row_panel_zero_shown", PANEL, "    (row.live > 0).then(|| format!(\"in use: {} now\", row.live))\n",
            "    true.then(|| format!(\"in use: {} now\", row.live))\n", T_PANEL, "rust"),
    # Another project's orders counted: builder reads "in use: 3 now" and spare "in use: 1 now".
    Control("p96_row_panel_other_project", PANEL, "        super::nirvana_profiles::root(folder) == root\n",
            "        super::nirvana_profiles::root(folder) == root || true\n", T_PANEL, "rust"),
    # A finished order counted: builder reads "in use: 3 now" (b3 is Done).
    Control("p96_row_panel_finished_counts", SUBAGENTS, RUNNING, "", T_PANEL, "rust"),

    # ---- the agents panel's live row: the profile on line 3 (fix round, review F1) ----
    # The height not counting the profile line: the width loop's first assert reads 9, not 11, at width 8.
    Control("p96_row_fix_height_uncounted", SIDEBAR, ROW_LINES, "    2\n", T_HEIGHTS, "rust"),
    # The same in the sweep: "height not two more" is a bad line (and the row is cut, so its line 3 too).
    Control("p96_row_fix_sweep_height_uncounted", SIDEBAR, ROW_LINES, "    2\n", T_SWEEP, "rust"),
    # The panel's row height short of what is drawn: at the body that holds both rows, the orchestrator is drawn one
    # line short (its cap cuts w-c to "+1 more") and its last line, lines[13], is the plain agent's, not "      @scholar".
    Control("p96_row_fix_panel_height_short", SIDEBAR, PANEL_HEIGHT,
            "        |row| agent_row_lines(row, area.width).saturating_sub(1),\n", T_HEIGHTS, "rust"),
    # The cap counting a profiled row as two: a capped body draws a profiled row without its line 3, so the cap loop's
    # line-3 assert (or drawn + more) fails.
    Control("p96_row_fix_cap_counts_two", SIDEBAR, CAP_ROW, "                let lines = 2;\n", T_HEIGHTS, "rust"),
    # Line 3 never drawn: the sweep's first bad is "line 3 Some(\"\"), not \"      @a\"" at width 8.
    Control("p96_row_fix_line3_dropped", SIDEBAR, LINE3,
            "            if let Some(name) = sub.profile.as_deref().filter(|_| false) {\n"
            "                if *current_y >= rect.bottom() {\n", T_SWEEP, "rust"),
    # The same for the four real names: "      @reel-editor" is not in the lines.
    Control("p96_row_fix_real_names_dropped", SIDEBAR, LINE3,
            "            if let Some(name) = sub.profile.as_deref().filter(|_| false) {\n"
            "                if *current_y >= rect.bottom() {\n", T_REAL, "rust"),
    # The same in the dump: lines[at(p47a-build) + 2] reads "", not "      @builder".
    Control("p96_row_fix_dump_dropped", SIDEBAR, LINE3,
            "            if let Some(name) = sub.profile.as_deref().filter(|_| false) {\n"
            "                if *current_y >= rect.bottom() {\n", T_DUMP, "rust"),
    # The room not less the indent: at width 8 "abcd" is clipped to "      @a", not cut to "      @…".
    Control("p96_row_fix_line3_room_whole", SIDEBAR, LINE3_ROOM,
            "                        profile_line(name, rect.width as usize),\n", T_SWEEP, "rust"),
    # Line 3 not cut by `fit`: at width 8 "abcd" is clipped to "      @a", not "      @…".
    Control("p96_row_fix_line3_uncut", SIDEBAR, "    super::nirvana_spaces::fit(&profile_mark(name), room)\n",
            "    profile_mark(name)\n", T_SWEEP, "rust"),
    # Line 3 drawn uncut into a rect 12 cells past the panel: the bad lines include "drawn past the edge".
    Control("p96_row_fix_line3_past_edge", SIDEBAR, LINE3_DRAW,
            LINE3_DRAW.replace("(rect.width as usize).saturating_sub(6)", "usize::MAX")
                      .replace("rect.width, 1), buffer);", "rect.width + 12, 1), buffer);"), T_SWEEP, "rust"),
    # Line 3 in the model's muted colour: "line 3 cell 6 not bold mauve".
    Control("p96_row_fix_line3_muted", SIDEBAR, LINE3_STYLE,
            LINE3_STYLE.replace(".fg(palette.mauve)", ".fg(palette.overlay1)"), T_SWEEP, "rust"),
    # Line 3 not bold: "line 3 cell 6 not bold mauve".
    Control("p96_row_fix_line3_not_bold", SIDEBAR, LINE3_STYLE,
            "                        Style::default()\n                            .fg(palette.mauve),\n",
            T_SWEEP, "rust"),
    # Line 2 changed beside a profile, the model in mauve: the sweep's first bad is "line 4 changed" (cells differ).
    Control("p96_row_fix_line2_model_colour", SIDEBAR, LINE2_MODEL,
            "                ratatui::text::Span::styled(short_m, Style::default().fg(if sub.profile.is_some() "
            "{ palette.mauve } else { palette.overlay1 })),\n", T_SWEEP, "rust"),
    # The click rectangle two lines tall: a click on w-a's line 3 finds no row (None), not p96-w-a.
    Control("p96_row_fix_click_two_lines", SIDEBAR, HIT,
            "                2u16.min(rect.bottom().saturating_sub(*current_y));\n", T_CLICK, "rust"),
    # A no-profile row given a third, blank line: the bare panel is not the profiled one less its profile lines.
    Control("p96_row_fix_bare_row_three", SIDEBAR, ROW_LINES,
            "    2 + usize::from(sub.profile.is_some()) + usize::from(sub.profile.is_none())\n", T_BARE, "rust"),
    # The name from disk drawn raw: the ESC and the zero-width space reach the buffer; "@a<U+200B>b" is not there.
    Control("p96_row_raw_name", SIDEBAR, SHOWN, "    let shown = name.replace('\\n', \"<U+000A>\");\n", T_INVISIBLE,
            "rust"),
    # A newline kept: profile_mark("n\nl") reads "@n\nl".
    Control("p96_row_newline_kept", SIDEBAR, SHOWN,
            "    let shown = super::nirvana_profile_panel::visible(name);\n", T_INVISIBLE, "rust"),

    # ---- the spaces view's worker line ----
    # The profile not carried: workers[0].profile is None.
    Control("p96_row_spaces_not_carried", SPACES, "                profile: sub.profile.clone(),\n",
            "                profile: None,\n", T_SPACE_CARRY, "rust"),
    # The profile not added to the line: worker_line(28) reads "v8-build S5.5 1m07s".
    Control("p96_row_spaces_line_no_profile", SPACES, "        line = format!(\"{line} {profile}\");\n", "",
            T_SPACE_LINE, "rust"),
    # The room not what the line leaves: worker_line(24) reads "v8-build S5.5 1m07s @builder", 28 cells, not "@bu…".
    Control("p96_row_spaces_room_whole", SPACES,
            "super::agent_sidebar::profile_in(profile, width.saturating_sub(drawn + 1))",
            "super::agent_sidebar::profile_in(profile, width)", T_SPACE_LINE, "rust"),
    # Fewer than 4 cells shown (moved here from the agents panel, whose line 3 has no floor): worker_line(23) reads
    # "v8-build S5.5 1m07s @b…".
    Control("p96_row_min_ignored", SIDEBAR, MIN, "    (room > 0).then(|| profile_line(name, room))\n", T_SPACE_LINE,
            "rust"),
    # Dropped while 4 cells fit: worker_line(24) reads "v8-build S5.5 1m07s", not "@bu…".
    Control("p96_row_min_too_strict", SIDEBAR, MIN, "    (room > PROFILE_MIN).then(|| profile_line(name, room))\n",
            T_SPACE_LINE, "rust"),
    # Not cut by `fit` in the spaces line: worker_line(24) reads "... @builder" whole, not "@bu…".
    Control("p96_row_uncut", SIDEBAR, MIN, "    (room >= PROFILE_MIN).then(|| profile_mark(name))\n", T_SPACE_LINE,
            "rust"),
    # The profile's start not past the gap: profile_at reads 19, not 20.
    Control("p96_row_spaces_at", SPACES, "        profile_at: if profile.is_some() { drawn + 1 } else { drawn },\n",
            "        profile_at: drawn,\n", T_SPACE_LINE, "rust"),
    # Fix round (review H): with no profile the start past the drawn text: bare.profile_at reads 20, not 19.
    Control("p96_row_fix_spaces_at_bare", SPACES,
            "        profile_at: if profile.is_some() { drawn + 1 } else { drawn },\n",
            "        profile_at: drawn + 1,\n", T_SPACE_LINE, "rust"),
    # The name from disk drawn raw in the spaces line: "@a<U+200B><U+001B>b" is not in it.
    Control("p96_row_spaces_raw_name", SIDEBAR, SHOWN, "    let shown = name.replace('\\n', \"<U+000A>\");\n",
            T_SPACE_LINE, "rust"),
    # The profile in the time's muted colour: cells 25..33 read overlay0, not mauve.
    Control("p96_row_spaces_colour", SPACES, "                    .fg(palette.mauve)\n",
            "                    .fg(palette.overlay0)\n", T_SPACE_COLOUR, "rust"),
    # Fix round (review H): the model in the time's colour: model cell 14 reads overlay0, not overlay1.
    Control("p96_row_fix_spaces_model_colour", SPACES,
            "    let model_style = Style::default().fg(palette.overlay1);\n"
            "    let elapsed_style = Style::default().fg(palette.overlay0);\n",
            "    let model_style = Style::default().fg(palette.overlay0);\n"
            "    let elapsed_style = Style::default().fg(palette.overlay0);\n", T_SPACE_COLOUR, "rust"),
    # Fix round (review H): the time in the model's colour: time cell 19 reads overlay1, not overlay0.
    Control("p96_row_fix_spaces_time_colour", SPACES,
            "    let model_style = Style::default().fg(palette.overlay1);\n"
            "    let elapsed_style = Style::default().fg(palette.overlay0);\n",
            "    let model_style = Style::default().fg(palette.overlay1);\n"
            "    let elapsed_style = Style::default().fg(palette.overlay1);\n", T_SPACE_COLOUR, "rust"),
)
