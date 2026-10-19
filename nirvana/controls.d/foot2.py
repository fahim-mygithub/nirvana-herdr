# Foot2 (package foot2): the usage foot shows both windows (5h and week).
# Red lines: tests in src/client/shell/tests/nirvana_footer.rs and src/nirvana_usage.rs.
CONTROLS = (
    # 1. Narrow row fits both windows and pool name exactly at default width 26 without truncation
    Control("foot2_narrow_row_both_windows", "src/client/shell/nirvana_footer.rs",
            "let name = fit(row.label(), room.saturating_sub(text_width));",
            "let name = fit(row.label(), room.saturating_sub(text_width + 1));",
            "the_foot_lists_a_spaces_server_and_a_usage_row_per_pool", "rust"),

    # 2. Wide mode shows reset for tightest window
    Control("foot2_wide_row_tightest_reset", "src/client/shell/nirvana_footer.rs",
            "let tightest = crate::nirvana_usage::tightest_shown(windows);",
            "let tightest = windows.first();",
            "a_wide_sidebar_draws_a_bar_and_the_reset_for_the_tightest_window", "rust"),

    # 3. Bar fill follows tightest window
    Control("foot2_wide_row_tightest_bar_fill", "src/nirvana_usage.rs",
            "// Foot2: the bar is filled by the tightest window.\n                tightest_shown(windows)",
            "// Foot2: the bar is filled by the tightest window.\n                windows.first()",
            "the_bar_follows_the_tightest_window", "rust"),

    # 4. Pace mark shown on ahead window alone
    Control("foot2_ahead_one_window_only", "src/client/shell/nirvana_footer.rs",
            "if win.ahead && (idx * FIGURE + MARK_AT) < text_width {",
            "if (idx * FIGURE + MARK_AT) < text_width {",
            "a_pool_ahead_on_one_window_only_marks_that_window_alone", "rust"),

    # 5. Stale week beside fresh 5h is muted independently
    Control("foot2_stale_beside_fresh_muted", "src/client/shell/nirvana_footer.rs",
            "let win_style = if !win.stale && win.used.is_some() {\n                    Style::default().fg(palette.text)",
            "let win_style = if win.used.is_some() {\n                    Style::default().fg(palette.text)",
            "a_stale_week_beside_a_fresh_5h_is_muted_while_5h_is_fresh", "rust"),

    # 6. One-window pool formats its single window and draws bar
    Control("foot2_one_window_pool", "src/client/shell/nirvana_footer.rs",
            "if bar {\n                    WIDE_FIGURE\n                } else {\n                    FIGURE\n                }",
            "if bar {\n                    100\n                } else {\n                    FIGURE\n                }",
            "a_one_window_pool_shows_that_single_window", "rust"),

    # 7. Dotted windows like week.fable are excluded from the foot
    Control("foot2_no_dotted_windows", "src/nirvana_usage.rs",
            "filter(|reading| reading.pool == pool && !reading.window.contains('.'))",
            "filter(|reading| reading.pool == pool)",
            "dotted_windows_are_never_shown_in_the_foot", "rust"),

    # 8. 5h window is sorted before week window
    Control("foot2_5h_first_order", "src/nirvana_usage.rs",
            'readings.sort_by_key(|r| match r.window.as_str() {\n                "5h" => 0,',
            'readings.sort_by_key(|r| match r.window.as_str() {\n                "5h" => 2,',
            "dotted_windows_are_never_shown_in_the_foot", "rust"),
)
