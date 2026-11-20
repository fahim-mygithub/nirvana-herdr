# Plan 47 P47a (the owner, 2026-11-20: "Im hoping for the UI under teh agent panal to showcase when we are using that
# build agent profile for a task."). The agents panel's live worker rows, the spaces view's worker line and the Profiles
# panel show the profile an order carries. Controls starting with p96_row_: the jobs-sizing package already holds the
# bare p96_ prefix (p96_jobs_sizing.py). One sabotage per assertion, each on a product line; the test named in the
# fifth field is the one expected to print its red line with the sabotage in place, and the comment above each says
# which of its assertions goes red and why (written before the run). `Control` is provided by controls.py.
SUBAGENTS = "src/nirvana_subagents.rs"
SIDEBAR = "src/client/shell/agent_sidebar.rs"
SPACES = "src/client/shell/nirvana_spaces.rs"
PANEL = "src/client/shell/nirvana_profile_panel.rs"

T_ROWS = "nirvana_subagents::tests::p96_row_rows_carry_the_profile_of_their_own_seated_line"
T_LIVE = "nirvana_subagents::tests::p96_row_live_profiles_count_running_orders_of_this_folder_only"
_S = "client::shell::agent_sidebar::tests::"
T_WIDTHS = _S + "p96_row_a_live_worker_shows_its_profile_on_line_two_at_three_widths"
T_CUT = _S + "p96_row_the_profile_is_cut_never_dropped_while_four_cells_fit"
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
DRAWN = "            if let Some(profile) = profile {\n                line2_spans.push"
MARK_STYLE = "                        .fg(palette.mauve)\n                        .add_modifier(Modifier::BOLD),\n                ));\n"
SHOWN = "    let shown = super::nirvana_profile_panel::visible(name).replace('\\n', \"<U+000A>\");\n"

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

    # ---- live_profiles ----
    # A finished order counted: builder reads 3 (a5 is Done), scholar 2 (a6 is Blocked).
    Control("p96_row_live_counts_finished", SUBAGENTS, RUNNING, "", T_LIVE, "rust"),
    # Another folder's order counted: builder reads 3 (b1 is in project-b).
    Control("p96_row_live_any_folder", SUBAGENTS, "        if here(&folder) {\n",
            "        if true || here(&folder) {\n", T_LIVE, "rust"),
    # An order with no profile counted, under "": the map gains a "" key.
    Control("p96_row_live_counts_none", SUBAGENTS,
            "                    .filter_map(move |row| Some((folder.clone(), row.profile.clone()?)))\n",
            "                    .filter_map(move |row| Some((folder.clone(), row.profile.clone().unwrap_or_default())))\n",
            T_LIVE, "rust"),

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

    # ---- the agents panel's live row, line 2 ----
    # The profile never drawn: line 2 reads "      S5.5  1m30s" at 36 cells.
    Control("p96_row_line2_dropped", SIDEBAR, DRAWN,
            "            if let Some(profile) = profile.filter(|_| false) {\n                line2_spans.push",
            T_WIDTHS, "rust"),
    # The same, in the dump: "@builder" is not in it.
    Control("p96_row_dump_dropped", SIDEBAR, DRAWN,
            "            if let Some(profile) = profile.filter(|_| false) {\n                line2_spans.push",
            T_DUMP, "rust"),
    # The mark in the model's muted colour: the `@` cell's fg is overlay1, not mauve.
    Control("p96_row_muted", SIDEBAR, MARK_STYLE,
            "                        .fg(palette.overlay1)\n                        .add_modifier(Modifier::BOLD),\n"
            "                ));\n", T_WIDTHS, "rust"),
    # The mark not bold: the `@` cell has no BOLD.
    Control("p96_row_not_bold", SIDEBAR, MARK_STYLE, "                        .fg(palette.mauve),\n                ));\n",
            T_WIDTHS, "rust"),
    # Fewer than 4 cells shown: line2(21) reads "      S5.5  1m30s @b…".
    Control("p96_row_min_ignored", SIDEBAR, "    (room >= PROFILE_MIN).then(", "    (room > 0).then(", T_CUT, "rust"),
    # Dropped while 4 cells fit: line2(22) reads "      S5.5  1m30s", not "@bu…".
    Control("p96_row_min_too_strict", SIDEBAR, "    (room >= PROFILE_MIN).then(", "    (room > PROFILE_MIN).then(",
            T_CUT, "rust"),
    # Not cut by `fit`: line2(23) is clipped by the paragraph to "@buil", not "@bui…".
    Control("p96_row_uncut", SIDEBAR, "super::nirvana_spaces::fit(&profile_mark(name), room))",
            "profile_mark(name))", T_CUT, "rust"),
    # The gap before the profile not counted: line2(23) cuts to "@buil…" and is clipped to "@buil", not "@bui…".
    Control("p96_row_gap_uncounted", SIDEBAR, "saturating_sub(used + 1)));", "saturating_sub(used)));", T_CUT,
            "rust"),
    # The time not counted in the room: line2(23) takes "@builder" whole, clipped to "@buil", not "@bui…".
    Control("p96_row_time_uncounted", SIDEBAR,
            "            let used = 6 + display_width(&short_m) + 2 + display_width(&elapsed_str);\n",
            "            let used = 6 + display_width(&short_m);\n", T_CUT, "rust"),
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
    # The profile's start not past the gap: profile_at reads 19, not 20.
    Control("p96_row_spaces_at", SPACES, "        profile_at: if profile.is_some() { drawn + 1 } else { drawn },\n",
            "        profile_at: drawn,\n", T_SPACE_LINE, "rust"),
    # The name from disk drawn raw in the spaces line: "@a<U+200B><U+001B>b" is not in it.
    Control("p96_row_spaces_raw_name", SIDEBAR, SHOWN, "    let shown = name.replace('\\n', \"<U+000A>\");\n",
            T_SPACE_LINE, "rust"),
    # The profile in the time's muted colour: cells 25..33 read overlay0, not mauve.
    Control("p96_row_spaces_colour", SPACES, "                    .fg(palette.mauve)\n",
            "                    .fg(palette.overlay0)\n", T_SPACE_COLOUR, "rust"),
)
