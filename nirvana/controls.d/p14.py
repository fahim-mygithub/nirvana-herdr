# Plan 14: spend-down seating (plans/2026-10-18-14-seat-spend-down.md). Red line for each: its test prints FAIL.
PROJECTED = 'projected_spare_uses_elapsed_with_ease_floor_and_subtracts_booked_leases'
COST = 'cost_splits_spare_and_rest_across_fresh_resetting_racing_and_booked_windows'
SPEND_DOWN = 'spend_down_on_2026_10_18_readings_and_when_max_has_spare_too'
TABLE = 'the_default_table_is_written_once_parses_and_holds_the_plans_seats_and_classes'

CONTROLS = (
    # §2.1: projected use divides by max(elapsed, pace_ease), and spare subtracts projected and booked.
    Control('p14_projected_ease_floor', 'src/nirvana_seat.rs',
            '    let projected = used / elapsed.max(pace_ease).max(1e-6);\n',
            '    let projected = used / elapsed.max(1e-6);\n',
            PROJECTED, "rust"),
    Control('p14_spare_ignores_booked', 'src/nirvana_seat.rs',
            '    let spare = (1.0 - projected - booked).max(0.0);\n',
            '    let spare = (1.0 - projected).max(0.0);\n',
            PROJECTED, "rust"),
    # §2.2: cost splits expected use into spare_part * spare_weight and rest priced at full pressure.
    Control('p14_cost_no_spare_split', 'src/nirvana_seat.rs',
            '        let spare = self.spare.unwrap_or(0.0).clamp(0.0, self.expected);\n',
            '        let spare = 0.0_f32.clamp(0.0, self.expected);\n',
            COST, "rust"),
    Control('p14_cost_rest_dropped', 'src/nirvana_seat.rs',
            '        let rest = (self.expected - spare).max(0.0);\n',
            '        let rest = 0.0_f32;\n',
            COST, "rust"),
    Control('p14_spare_phrase_any_window', 'src/nirvana_seat.rs',
            '        || !windows.iter().all(|window| {\n',
            '        || !windows.iter().any(|window| {\n',
            COST, "rust"),
    Control('p14_spare_reason_omitted', 'src/nirvana_seat.rs',
            '        spare_phrase(&seat.pool, &won.windows, now)\n',
            '        None::<String>\n',
            SPEND_DOWN, "rust"),
    # §2.2 & §2.3: constants.spare_weight, max pool per_usd.week, and build.bounded g-sonnet score.
    Control('p14_toml_spare_weight', 'src/nirvana_seats.toml',
            'spare_weight = 0.05\n',
            'spare_weight = 1.0\n',
            SPEND_DOWN, "rust"),
    Control('p14_toml_max_week_per_usd', 'src/nirvana_seats.toml',
            'per_usd = { "5h" = 0.04, week = 0.00078 }\n',
            'per_usd = { "5h" = 0.04, week = 0.005 }\n',
            TABLE, "rust"),
    Control('p14_toml_build_bounded_g_sonnet_score', 'src/nirvana_seats.toml',
            '  { seat = "flash", score = 78, effort = "low" },\n  { seat = "g-sonnet", score = 88 },',
            '  { seat = "flash", score = 78, effort = "low" },\n  { seat = "g-sonnet", score = 77 },',
            SPEND_DOWN, "rust"),
    # 0.22.1: a table written before plan 14 (no spare_weight line) still parses, with the default.
    Control('p14_spare_weight_required', 'src/nirvana_seat.rs',
            '    #[serde(default = "default_spare_weight")]\n',
            '',
            TABLE, "rust"),
)
