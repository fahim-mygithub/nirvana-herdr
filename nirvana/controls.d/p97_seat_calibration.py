# Plan 46 P46a (the owner, 2026-11-19: "Use the latest logs to update the table as refrencses. During the idle lesson
# learning runs have it check / maintain the table using real runs as calibration / validation"). Controls starting with
# p97_. One sabotage per assertion, each on a product line; the test named in the fifth field is the one expected to
# print its red line with the sabotage in place (written before the run). `Control` is provided by controls.py.
CALIB = "src/nirvana_seat_calib.rs"
SEAT = "src/nirvana_seat.rs"
SQUARE = "src/nirvana_seats_square.rs"
RUN = "src/nirvana_run.rs"
IDLE = "src/nirvana_idle_review.rs"
TABLE = "src/nirvana_seats.toml"
V18 = "src/nirvana_seats_v18.toml"

_C = "nirvana_seat_calib::tests::"
T_FIT = _C + "p97_the_cost_fit_is_seat_fit_pys_on_the_fixture"
T_FACTOR = _C + "p97_a_factor_needs_five_orders_outside_the_band_and_is_clamped"
T_PROJECT = _C + "p97_a_projects_own_figure_wins_at_five_orders"
T_DAYS = _C + "p97_only_the_last_sixty_days_count"
T_SCORES = _C + "p97_scores_are_findings_both_ways_and_never_applied"
T_CELLS = _C + "p97_reference_rows_count_each_cell"
T_LEDGER = _C + "p97_the_ledger_writes_a_row_when_a_factor_appears_moves_or_goes"
T_CHECK = _C + "p97_a_check_writes_the_file_and_a_failed_read_writes_nothing"
T_RECORD = _C + "p97_the_record_line_and_the_journals_carry_the_first_verdict"
T_IDLE = _C + "p97_the_idle_tick_checks_the_table_with_no_seat_and_no_review"
T_CACHED = _C + "p97_the_cached_world_reads_the_factors"
_S = "nirvana_seat::tests::"
T_EMPTY = _S + "p97_an_empty_calibration_or_off_moves_no_pick"
T_SCALE = _S + "p97_a_factor_scales_only_the_booked_use"
T_OWNER = _S + "p97_the_owners_table_upgrades_and_keeps_overrides_per_run"
T_ALONE = _S + "p97_other_edits_leave_the_table_alone"
T_V18 = _S + "p97_v18_is_last_and_an_exact_old_default_upgrades_with_its_mode"
T_DEFAULT = _S + "p97_the_default_turns_calibration_on_and_refits_hydrogen_low"
_Q = "nirvana_seats_square::tests::"
T_MATRIX = _Q + "p97_every_cell_is_the_pick_on_the_same_table_and_readings"
T_SQUARE = _Q + "p97_the_square_reads_the_calibration_and_names_what_differs"

CONTROLS = (
    # ---- the cost fit, seat_fit.py's section 1 ----
    Control("p97_fit_no_class_weight", CALIB, "    Some(nirvana_seat::cost(price, &tokens) * per / (share * weight))",
            "    Some(nirvana_seat::cost(price, &tokens) * per / share)", T_FIT, "rust"),
    Control("p97_fit_keeps_subagents", CALIB,
            "        return None;\n    }\n    let seat = table.seat(&line.seat)?;",
            "    }\n    let seat = table.seat(&line.seat)?;", T_FIT, "rust"),
    Control("p97_fit_any_size", CALIB, "        if line.size != Some(SizeName::Medium) {",
            "        if line.size.is_none() {", T_FIT, "rust"),
    Control("p97_median_even_takes_upper", CALIB, "        (sorted[mid - 1] + sorted[mid]) / 2.0",
            "        sorted[mid]", T_FIT, "rust"),
    # ---- the booking rule ----
    Control("p97_band_edge_moves", CALIB, "(BAND.0..=BAND.1).contains(&median)", "(BAND.0..BAND.1).contains(&median)",
            T_FACTOR, "rust"),
    Control("p97_four_orders_move", CALIB, "    if n < MIN_N ||", "    if n < MIN_N - 1 ||", T_FACTOR, "rust"),
    Control("p97_no_clamp", CALIB, "    let clamped = median.clamp(CLAMP.0, CLAMP.1);", "    let clamped = median;",
            T_FACTOR, "rust"),
    # ---- a project's own figure ----
    Control("p97_project_ignored", CALIB, "            return own.as_ref();", "            return self.pooled.get(&key);",
            T_PROJECT, "rust"),
    Control("p97_project_under_five", CALIB, "                Some(project) if row.n >= MIN_N => {",
            "                Some(project) => {", T_PROJECT, "rust"),
    Control("p97_project_of_any_path", CALIB, '    let (root, _) = unified.split_once("/.nirvana/")?;',
            '    let root = unified.split_once("/.nirvana/").map_or(unified.as_str(), |(root, _)| root);', T_PROJECT,
            "rust"),
    # ---- 60 days ----
    Control("p97_every_day_counts", CALIB,
            "        .filter(|line| line_at(line).is_some_and(|at| at.saturating_add(KEEP_S) >= now))\n"
            "        .collect();\n    let medium_use",
            "        .filter(|_| true)\n        .collect();\n    let medium_use", T_DAYS, "rust"),
    # ---- the score rule, a finding both ways ----
    Control("p97_score_low_never", CALIB, "    if share < FIRST_PASS_LOW {", "    if share < 0.0 {", T_SCORES, "rust"),
    Control("p97_score_worker_share_is_all", CALIB,
            "            entry.score,\n            worker_gated,\n            first_pass,\n",
            "            entry.score,\n            gated,\n            first_pass,\n", T_SCORES, "rust"),
    Control("p97_score_raise_without_merges", CALIB, "        && merged.is_some_and(|merged| merged >= MERGED_HIGH)",
            "        && merged.unwrap_or(1.0) >= MERGED_HIGH", T_SCORES, "rust"),
    Control("p97_amend_is_a_worker_refusal", CALIB,
            '            line.first_no_worker_fault == Some(true) || first == "amend",',
            "            line.first_no_worker_fault == Some(true),", T_SCORES, "rust"),
    Control("p97_too_few_at_nine", CALIB, "    if gated < GATED_MIN {", "    if gated < GATED_MIN - 1 {", T_SCORES, "rust"),
    # ---- the reference rows ----
    Control("p97_recent_is_every_order", CALIB,
            "                        line_at(o.line).is_some_and(|at| at.saturating_add(RECENT_S) >= now)",
            "                        true", T_CELLS, "rust"),
    Control("p97_merges_never_known", CALIB, "                .merging\n                .contains(&run)",
            "                .merging\n                .is_empty()", T_CELLS, "rust"),
    Control("p97_thin_never", CALIB, "        if self.orders < THIN {", "        if self.orders < 1 {", T_CELLS, "rust"),
    Control("p97_minutes_in_seconds", CALIB, "                    Some(ended.saturating_sub(seated) as f64 / 60.0)",
            "                    Some(ended.saturating_sub(seated) as f64)", T_CELLS, "rust"),
    # ---- the ledger ----
    Control("p97_ledger_small_moves", CALIB, "(b / a - 1.0).abs() > MOVE", "(b / a - 1.0).abs() > 0.05", T_LEDGER, "rust"),
    Control("p97_ledger_no_gone", CALIB, "                (None, None) => false,\n                _ => true,",
            "                (None, None) => false,\n                (None, Some(_)) => true,\n                _ => false,",
            T_LEDGER, "rust"),
    # ---- the file, the dry run, a failed read, due ----
    Control("p97_dry_run_writes", CALIB, "    if write {\n        let mut text", "    if true {\n        let mut text",
            T_CHECK, "rust"),
    Control("p97_due_at_nineteen", CALIB, "|| lines >= cal.record_lines + DUE_LINES",
            "|| lines >= cal.record_lines + DUE_LINES - 1", T_CHECK, "rust"),
    Control("p97_due_without_record", CALIB,
            "    let Some(lines) = record_count(&dir.join(nirvana_seat::RECORD_FILE)) else {\n        return false;",
            "    let Some(lines) = record_count(&dir.join(nirvana_seat::RECORD_FILE)) else {\n        return true;",
            T_CHECK, "rust"),
    Control("p97_failed_read_computes", CALIB,
            "    let record = read_record(&dir.join(nirvana_seat::RECORD_FILE))?;",
            "    let record = read_record(&dir.join(nirvana_seat::RECORD_FILE)).unwrap_or(Record {\n"
            "        lines: Vec::new(),\n        count: 0,\n        bytes: 0,\n    });", T_CHECK, "rust"),
    Control("p97_record_bytes_dropped", CALIB, "    calibration.record_bytes = record.bytes;",
            "    calibration.record_bytes = 0;", T_CHECK, "rust"),
    Control("p97_cli_verb", CALIB, '        "would change"\n', '        "changed"\n', T_CHECK, "rust"),
    # ---- the first verdict: the journals, the replay, the record line ----
    Control("p97_journals_last_verdict", CALIB, "            if let Some(first) = &order.first_verdict {",
            "            if let Some(first) = order.verdict.as_ref() {", T_RECORD, "rust"),
    Control("p97_journals_no_merging", CALIB, "                journals.merging.insert(run.clone());", "", T_RECORD, "rust"),
    Control("p97_record_field_loses", CALIB, "    if let Some(first) = &line.first_verdict {",
            "    if let Some(first) = line.first_verdict.as_ref().filter(|_| false) {", T_RECORD, "rust"),
    Control("p97_replay_keeps_last", RUN, "                    if known.first_verdict.is_none() {",
            "                    if true {", T_RECORD, "rust"),
    Control("p97_amend_has_no_fault", RUN, "    if verdict == Outcome::Amend.word() {\n        return Some(true);",
            "    if verdict == Outcome::Amend.word() {\n        return None;", T_RECORD, "rust"),
    Control("p97_fault_unsaid_reads_known", RUN, "    (said && failing).then(", "    failing.then(", T_RECORD, "rust"),
    Control("p97_record_line_drops_first", RUN, "        first_verdict: order.first_verdict.clone(),",
            "        first_verdict: None,", T_RECORD, "rust"),
    # ---- the idle step ----
    Control("p97_idle_no_check", IDLE,
            "    let calibrated = crate::nirvana_seat_calib::tick(&nirvana_seat::dir(), table, now);",
            "    let calibrated: Option<String> = None;", T_IDLE, "rust"),
    Control("p97_idle_not_kept", IDLE, "            global.calibration = Some((now, summary.clone()));",
            "            let _ = summary;", T_IDLE, "rust"),
    Control("p97_idle_view_drops", IDLE, "        calibration: global.calibration,\n",
            "        calibration: None,\n", T_IDLE, "rust"),
    Control("p97_idle_no_line", IDLE, '        out.push(format!("seat table checked {}: {said}", ago(*at)));',
            "        let _ = (at, said);", T_IDLE, "rust"),
    # ---- the pick ----
    Control("p97_factor_not_applied", SEAT, "        * measured.map_or(1.0, |applied| applied.factor as f32);",
            "        * 1.0;", T_SCALE, "rust"),
    Control("p97_basis_silent", SEAT, '            Some(applied) => format!("{basis}; {}", applied.words()),',
            "            Some(_) => basis,", T_SCALE, "rust"),
    Control("p97_factor_ignores_the_switch", SEAT, "        .filter(|_| table.calibration)",
            "        .filter(|_| true)", T_EMPTY, "rust"),
    Control("p97_absent_reads_on", SEAT, "    #[serde(default)]\n    pub calibration: bool,",
            '    #[serde(default = "crate::api::schema::common::default_true")]\n    pub calibration: bool,', T_EMPTY,
            "rust"),
    Control("p97_cached_world_no_factors", SEAT,
            "    let calibration = crate::nirvana_seat_calib::Factors::from_file(dir, None);",
            "    let calibration = crate::nirvana_seat_calib::Factors::default();", T_CACHED, "rust"),
    # ---- the upgrade that carries plain edits ----
    Control("p97_upgrade_never_carries", SEAT, "        let carried = carried_from_kept(&canonical)?;",
            "        let carried: Carried = None?;", T_OWNER, "rust"),
    Control("p97_carried_value_lost", SEAT, "            Some(i) => out[i] = mine.clone(),", "            Some(_) => {}",
            T_OWNER, "rust"),
    Control("p97_backup_one_name", SEAT,
            '    let mut name = backup_dir.join(format!("seats-{stamp}.toml"));\n    let mut n = 2;\n    while name.exists() {',
            "    let mut name = backup_dir.join(SEATS_FILE);\n    let mut n = 2;\n    while false && name.exists() {",
            T_OWNER, "rust"),
    Control("p97_no_upgrade_note", SEAT, "            .open(backup_dir.join(UPGRADE_LOG))",
            '            .open(backup_dir.join("missing").join(UPGRADE_LOG))', T_OWNER, "rust"),
    Control("p97_differs_line_unnamed", SEAT,
            "                \"seats.toml keeps your {}; the rest is herdr's default.\",",
            '                "{} differs",', T_OWNER, "rust"),
    Control("p97_diff_no_note", SEAT, '    format!("--- {label}\\n+++ herdr\'s default\\n{body}{note}")',
            '    format!("--- {label}\\n+++ herdr\'s default\\n{body}")', T_OWNER, "rust"),
    Control("p97_carry_any_section", SEAT, '        if !matches!(section.as_deref(), None | Some("constants")) {',
            "        if false {", T_ALONE, "rust"),
    Control("p97_carry_renamed_key", SEAT, "        if key != their_key {", "        if false {", T_ALONE, "rust"),
    Control("p97_carry_any_length", SEAT, "    if a.len() != b.len() {\n        return None;\n    }\n    let at",
            "    if false {\n        return None;\n    }\n    let at", T_ALONE, "rust"),
    Control("p97_carry_unparsed", SEAT, "        parse(&upgraded).ok()?;", "        let _ = parse(&upgraded);",
            T_ALONE, "rust"),
    Control("p97_current_rewritten", SEAT, "    if nearest.1 == current {\n        return None;\n    }\n",
            "", T_ALONE, "rust"),
    Control("p97_v18_not_historical", SEAT, '    include_str!("nirvana_seats_v18.toml"),\n', "", T_V18, "rust"),
    Control("p97_v17_dropped", SEAT, '    include_str!("nirvana_seats_v17.toml"),\n', "", T_V18, "rust"),
    # ---- the default table ----
    Control("p97_calibration_off_in_default", TABLE, "\ncalibration = true\n", "\ncalibration = false\n", T_DEFAULT,
            "rust"),
    Control("p97_hydrogen_low_not_refit", TABLE, 'windows = { "5h" = 8.27e-05, week = 4.135e-05 }',
            'windows = { "5h" = 0.0002235, week = 0.0001118 }', T_DEFAULT, "rust"),
    Control("p97_v18_edited", V18, 'windows = { "5h" = 0.0002588, week = 0.0001294 }',
            'windows = { "5h" = 0.0002589, week = 0.0001294 }', T_DEFAULT, "rust"),
    # ---- the matrix ----
    Control("p97_cool_world_unread", SEAT, "                used: Some(0.0),\n                resets_at: Some(live.now + length),",
            "                used: None,\n                resets_at: Some(live.now + length),", T_MATRIX, "rust"),
    Control("p97_cool_world_no_fable_week", SEAT, "            for window in &seat.extra_windows {",
            "            for window in seat.extra_windows.iter().take(0) {", T_MATRIX, "rust"),
    Control("p97_matrix_wrong_next", SQUARE, "                .get(1)\n", "                .get(2)\n", T_MATRIX, "rust"),
    Control("p97_matrix_never_forced", SQUARE, "    let forced = key(&now) != key(&then);", "    let forced = false;",
            T_MATRIX, "rust"),
    Control("p97_matrix_drops_needs", SQUARE, "                request.needs = class.needs.clone();\n                cell(",
            "                cell(", T_MATRIX, "rust"),
    Control("p97_matrix_one_mode_for_nirvana_claude", SQUARE, "            at.mode = *mode;\n",
            "            at.mode = Mode::Balanced;\n", T_MATRIX, "rust"),
    Control("p97_matrix_all_enabled", SQUARE, "                enabled: table.seat(&entry.seat).is_some_and(|seat| seat.enabled),",
            "                enabled: true,", T_MATRIX, "rust"),
    Control("p97_matrix_any_class_record", SQUARE,
            "                            .filter(|row| row.class == class.id && row.seat == entry.seat)",
            "                            .filter(|row| row.seat == entry.seat)", T_MATRIX, "rust"),
    Control("p97_square_pooled_only", SQUARE,
            "            calib::Factors::from_calibration(cal, Some(calib::root_key(root)))",
            "            calib::Factors::from_calibration(&Calibration::default(), Some(calib::root_key(root)))",
            T_SQUARE, "rust"),
    Control("p97_square_no_matrix", SQUARE, "        view.matrix = Some(matrix(table, &world, calibration.as_ref()));",
            "        view.matrix = None;", T_SQUARE, "rust"),
    Control("p97_square_no_differs_line", SQUARE, "    view.differs_line = differs_line;\n",
            "    view.differs_line = None;\n", T_SQUARE, "rust"),
    Control("p97_square_measured_unread", SQUARE, '                .and_then(|basis| basis.split_once("; measured x"))',
            '                .and_then(|basis| basis.split_once("; nothing x"))', T_SQUARE, "rust"),
)
