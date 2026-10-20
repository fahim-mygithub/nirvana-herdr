# Plan 19: seat modes recalibrated (plans/2026-10-20-19-seat-modes.md).
# Red line for each: its test prints FAIL.

CONTROLS = (
    # L7 (§3): Enabled Google-family seats are flash and argon only.
    Control(
        'p19_l7_google_family_flash_argon_only',
        'src/nirvana_seat.rs',
        '            .filter(|s| s.enabled && s.family == "google")\n',
        '            .filter(|s| s.enabled && (s.family == "google" || s.id == "haiku"))\n',
        'l7_google_family_enabled_seats_are_flash_and_argon_only',
        'rust',
    ),
    # L6 (§5): Automatic effort capped at high.
    Control(
        'p19_l6_effort_capped_at_high',
        'src/nirvana_seat.rs',
        '        want = Effort::High;\n        note = Some("capped at high".to_owned());\n',
        '        want = Effort::Xhigh;\n        note = Some("capped at high".to_owned());\n',
        'l6_no_class_in_any_mode_chooses_an_effort_above_high_and_override_works',
        'rust',
    ),
    # L6 (§5): Effort cap note is "capped at high".
    Control(
        'p19_l6_effort_cap_note',
        'src/nirvana_seat.rs',
        '        note = Some("capped at high".to_owned());\n',
        '        note = None;\n',
        'l6_no_class_in_any_mode_chooses_an_effort_above_high_and_override_works',
        'rust',
    ),
    # L2 (§6.1): Efficient mode uses efficient_effort.
    Control(
        'p19_l2_efficient_mode_lowest_effort',
        'src/nirvana_seat.rs',
        '            Mode::Efficient => class.efficient_effort.unwrap_or(class.effort),\n',
        '            Mode::Efficient => class.effort,\n',
        'l2_efficient_for_every_class_seats_lowest_pressure_at_efficient_effort',
        'rust',
    ),
    # L3 (§6.2): Balanced mode bar drops with climb.
    Control(
        'p19_l3_balanced_bar_halfway',
        'src/nirvana_seat.rs',
        '            let b_bar = bars.balanced - t * (bars.balanced - bars.efficient);\n',
        '            let b_bar = bars.balanced;\n',
        'l3_balanced_on_cool_world_seats_opus_and_sonnet_and_climbs',
        'rust',
    ),
    # L3 (§6.2): Balanced mode effort steps down at t >= 0.5.
    Control(
        'p19_l3_balanced_effort_step_down',
        'src/nirvana_seat.rs',
        '            if t >= 0.5 {\n',
        '            if t >= 1.5 {\n',
        'l3_balanced_on_cool_world_seats_opus_and_sonnet_and_climbs',
        'rust',
    ),
    # L5 (§6.3): SOTA mode bypasses filter 6 (headroom).
    Control(
        'p19_l5_sota_bypasses_filter_6',
        'src/nirvana_seat.rs',
        '    if request.mode != Mode::Sota {\n',
        '    if true || request.mode != Mode::Sota {\n',
        'l5_sota_pick_is_the_same_with_no_use_and_with_non_floored_pool_at_95_pct',
        'rust',
    ),
    # L5 (§6.3): SOTA mode ranks by score.
    Control(
        'p19_l5_sota_ranks_by_score',
        'src/nirvana_seat.rs',
        '        if mode == Mode::Sota {\n            return cand_b\n                .score\n                .total_cmp(&cand_a.score)\n',
        '        if mode == Mode::Sota {\n            return cand_a\n                .score\n                .total_cmp(&cand_b.score)\n',
        'l5_sota_pick_is_the_same_with_no_use_and_with_non_floored_pool_at_95_pct',
        'rust',
    ),
    # L8 (§6.4): Fallback seats rank after non-fallback seats.
    Control(
        'p19_l8_fallback_ranking',
        'src/nirvana_seat.rs',
        '        if fallback_a != fallback_b {\n            return fallback_a.cmp(&fallback_b);\n        }\n',
        '        if false && fallback_a != fallback_b {\n            return fallback_a.cmp(&fallback_b);\n        }\n',
        'l8_g_opus_wins_plan_only_when_all_non_fallback_seats_are_skipped',
        'rust',
    ),
    # §2: Default table Gemini per_usd factor.
    Control(
        'p19_section2_table_gemini_per_usd',
        'src/nirvana_seats.toml',
        'per_usd = { "5h" = 0.0021, week = 0.00107 }\n',
        'per_usd = { "5h" = 0.0021, week = 0.005 }\n',
        'section2_shipped_table_per_usd_and_use_cells_match_plan19',
        'rust',
    ),
    # §8: agy stream without Argon group falls back to herdr record.
    Control(
        'p19_section8_argon_from_record',
        'src/nirvana_usage.rs',
        '    if has_gemini_group && !has_argon_group && !has_argon_bucket {\n',
        '    if false && has_gemini_group && !has_argon_group && !has_argon_bucket {\n',
        'section8_agy_stream_with_no_argon_group_gives_known_argon_week_from_record',
        'rust',
    ),
    # §10: Untouched seats.toml upgrades to new default preserving mode.
    Control(
        'p19_section10_v3_upgrade_keeps_mode',
        'src/nirvana_seat.rs',
        '    let upgraded = replace_mode_line(&current_canonical, mode_line);\n',
        '    let upgraded = current_canonical;\n',
        'section10_v3_upgrade_keeps_efficient_mode_and_edited_does_not',
        'rust',
    ),
    # §7: Seats square title renders SOTA in red when SOTA is selected.
    Control(
        'p19_client_sota_word_red_title',
        'src/client/shell/nirvana_seats.rs',
        '    if shown.mode == Some(Mode::Sota) {\n',
        '    if false && shown.mode == Some(Mode::Sota) {\n',
        'sota_word_is_red_in_title_and_mode_row_when_sota_is_on_not_otherwise',
        'rust',
    ),
    # §7: Modes panel mode row renders SOTA in red when SOTA is selected.
    Control(
        'p19_client_sota_word_red_mode_row',
        'src/client/shell/nirvana_seats.rs',
        '                        let color = if mode == Mode::Sota {\n                            palette.red\n                        } else {\n                            palette.green\n                        };\n',
        '                        let color = if mode == Mode::Sota {\n                            palette.green\n                        } else {\n                            palette.green\n                        };\n',
        'sota_word_is_red_in_title_and_mode_row_when_sota_is_on_not_otherwise',
        'rust',
    ),
    # §7: Seats square in mod strip renders SOTA in red when SOTA is selected.
    Control(
        'p19_client_sota_word_red_mod_strip',
        'src/client/shell/nirvana_mods.rs',
        '        if seats.and_then(|s| s.mode) == Some(crate::nirvana_seat::Mode::Sota) {\n',
        '        if false && seats.and_then(|s| s.mode) == Some(crate::nirvana_seat::Mode::Sota) {\n',
        'sota_word_is_red_in_title_and_mode_row_when_sota_is_on_not_otherwise',
        'rust',
    ),
)
