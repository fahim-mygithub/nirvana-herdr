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
T_CACHED = _C + "p97_the_cached_world_reads_no_factors_and_the_idle_pick_reads_the_pooled"
# The P46a review's fix round: findings 1 to 7 and the two cases the review did not run.
T_STALE = _C + "p97_a_factor_belongs_to_the_cells_it_was_measured_on"
T_CLAMP_READ = _C + "p97_factors_out_of_bounds_are_dropped_when_read"
T_BOUNDED = _C + "p97_the_idle_check_is_bounded"
T_DEDUPE = _C + "p97_cost_counts_one_line_per_order"
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
# The follow-up (the hold, the live world's factors, the comment edit, `--picks`).
T_HELD = _S + "p97_a_held_pair_books_as_the_table"
T_KEEPS_PLAN = _S + "p97_the_default_holds_opus_high_and_keeps_balanced_plan"
T_HOLD_LINE = _S + "p97_the_hold_line_carries_and_names_a_seat"
T_LIVE = _S + "p97_the_live_worlds_factors_follow_the_switch"
T_CALIB_HELD = _C + "p97_held_pairs_are_measured_and_marked"
T_NO_BACKUP = _S + "p97_no_upgrade_without_a_backup"
T_ADDED = _S + "p97_an_added_plain_line_carries"
T_TRAILING = _S + "p97_trailing_whitespace_is_no_edit"
T_DROPPED = _S + "p97_a_dropped_key_is_named_once"
# The P46a second review's fix round: F1 to F6, the wording, and the section 5 tests.
T_FAILED_WRITE = _S + "p97_a_failed_write_keeps_the_table_and_says_so_once"
T_NOTE = _S + "p97_a_refusal_note_stands_only_for_its_key_and_time"
T_REFUSALS = _S + "p97_every_refusal_is_said_once_and_leaves_no_file"
T_REUSE = _S + "p97_a_backup_holding_the_table_is_used_again"
T_UNREADABLE = _S + "p97_an_unreadable_table_is_left_and_named"
T_RACE = _S + "p97_a_save_during_the_upgrade_is_kept"
T_CARRY = _S + "p97_a_refused_carry_is_named_once"
T_BOM = _S + "p97_a_table_with_a_bom_is_left_and_said_once"
T_FPRINT = _C + "p97_the_fingerprint_sees_only_what_the_ratio_reads"
T_FUTURE = _C + "p97_a_back_off_note_from_the_future_is_ignored"
T_CHECK_LINE = _Q + "p97_the_check_line_says_why_nothing_books"

CONTROLS = (
    # ---- the cost fit, seat_fit.py's section 1 ----
    Control("p97_fit_no_class_weight", CALIB, "    Some(nirvana_seat::cost(price, &tokens) * per / (share * weight))",
            "    Some(nirvana_seat::cost(price, &tokens) * per / share)", T_FIT, "rust"),
    # Fix round: the one subagent guard is the dedupe's now (cost, scores and rows); `unit`'s copy went.
    Control("p97_fit_keeps_subagents", CALIB,
            "        if line.chain.as_deref() == Some(crate::nirvana_claude::SUBAGENT_CHAIN) {\n"
            "            continue;\n        }\n        last.insert(",
            "        last.insert(", T_FIT, "rust"),
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
    # Fix round: the n check moved to the top of the read, for pooled rows too (finding 3).
    Control("p97_project_under_five", CALIB, "            if row.n < MIN_N {\n                continue;\n            }\n",
            "", T_PROJECT, "rust"),
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
    Control("p97_dry_run_writes", CALIB, "    if write {\n        save(dir, &mut checked)?;",
            "    if true {\n        save(dir, &mut checked)?;", T_CHECK, "rust"),
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
    # Fix round (finding 7): the cached world reads no factors; the idle pick's pooled ones follow the switch.
    Control("p97_cached_world_reads_factors", SEAT, "            .leases,\n        ..World::default()\n    }\n}",
            "            .leases,\n        calibration: crate::nirvana_seat_calib::read(dir)\n"
            "            .map(|cal| crate::nirvana_seat_calib::Factors::from_calibration(&cal, None))\n"
            "            .unwrap_or_default(),\n        ..World::default()\n    }\n}", T_CACHED, "rust"),
    Control("p97_pooled_ignores_the_switch", SEAT,
            "    if table.calibration {\n        crate::nirvana_seat_calib::Factors::from_file(dir, table, None)",
            "    if true {\n        crate::nirvana_seat_calib::Factors::from_file(dir, table, None)", T_CACHED, "rust"),
    # ---- the upgrade that carries plain edits ----
    Control("p97_upgrade_never_carries", SEAT, "        let carried = carried_from_kept(&canonical, default, kept)?;",
            "        let carried: Carried = None?;", T_OWNER, "rust"),
    Control("p97_carried_value_lost", SEAT, "            Some(i) => out[i] = mine.clone(),", "            Some(_) => {}",
            T_OWNER, "rust"),
    Control("p97_backup_one_name", SEAT,
            '    let mut name = backup_dir.join(format!("seats-{stamp}.toml"));\n    let mut n = 2;\n    while name.exists() {',
            "    let mut name = backup_dir.join(SEATS_FILE);\n    let mut n = 2;\n    while false && name.exists() {",
            T_OWNER, "rust"),
    Control("p97_no_upgrade_note", SEAT,
            '    for log in [dir.join("backup").join(UPGRADE_LOG), dir.join(UPGRADE_LOG)] {',
            "    for log in [dir.join(UPGRADE_LOG)] {", T_OWNER, "rust"),
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
    Control("p97_carry_unparsed", SEAT, "        if let Err(problems) = parse(&upgraded) {",
            "        if let Err(problems) = Ok::<(), String>(()) {", T_ALONE, "rust"),
    Control("p97_current_rewritten", SEAT, "    if kept == current {\n        return None;\n    }\n",
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
    Control("p97_square_measured_unread", SQUARE,
            "            .filter(|_| table.calibration)\n            .map(|applied| (applied.words(),",
            "            .filter(|_| false)\n            .map(|applied| (applied.words(),", T_SQUARE, "rust"),
    # ---- the follow-up: the owner's hold on Opus high ----
    Control("p97_hold_ignored", SEAT, "    let held = found.filter(|_| table.held(&seat.id, effort));",
            "    let held = found.filter(|_| false);", T_HELD, "rust"),
    Control("p97_held_basis_silent", SEAT, '                    "{basis}; held: {}, not booked (calibration_hold)",',
            '                    "{basis}; {}",', T_HELD, "rust"),
    Control("p97_held_matches_nothing", SEAT, '            .any(|pair| pair.trim() == format!("{seat}:{}", effort.word()))',
            '            .any(|pair| pair.trim() == format!("{seat}{}", effort.word()))', T_KEEPS_PLAN, "rust"),
    Control("p97_default_holds_nothing", TABLE, 'calibration_hold = ["opus:high"]\n', "calibration_hold = []\n",
            T_KEEPS_PLAN, "rust"),
    Control("p97_hold_unchecked", SEAT,
            "            if named.is_none_or(|(seat, effort)| !seat.efforts.contains(&effort)) {",
            "            if named.is_none() && false {", T_HOLD_LINE, "rust"),
    # Fix round (finding 5): the hold's special case is now the rule for any added top-level plain line.
    Control("p97_hold_line_not_carried", SEAT,
            "        carried\n            .lines\n            .push((None, key, lines[index].to_owned(), value));",
            "        let _ = (&carried, key, value);", T_HOLD_LINE, "rust"),
    Control("p97_hold_not_taken_out", SEAT,
            "                .filter(|(key, _)| !theirs.contains_key(*key) && known.contains_key(*key))",
            "                .filter(|_| false)", T_HOLD_LINE, "rust"),
    Control("p97_arrays_not_plain", SEAT, "        toml::Value::Array(items) => items.iter().all(scalar),",
            "        toml::Value::Array(_) => false,", T_HOLD_LINE, "rust"),
    Control("p97_calib_held_unmarked", CALIB, "            held: table.held(seat, effort),", "            held: false,",
            T_CALIB_HELD, "rust"),
    Control("p97_summary_counts_held_booked", CALIB,
            "        .filter(|row| row.project.is_none() && row.factor.is_some() && !row.held)",
            "        .filter(|row| row.project.is_none() && row.factor.is_some())", T_CALIB_HELD, "rust"),
    Control("p97_text_held_silent", CALIB,
            '                Some(f) if row.held => format!("held at the table\'s booking (measured x{f:.2})"),\n'
            "                Some(f) if row.clamped =>",
            "                Some(f) if row.clamped =>", T_CALIB_HELD, "rust"),
    Control("p97_picks_compare_off_with_off", CALIB, "    on.calibration = true;", "    on.calibration = false;",
            T_CALIB_HELD, "rust"),
    Control("p97_journal_files_uncounted", CALIB, "        journals.files += 1;\n", "", T_RECORD, "rust"),
    Control("p97_matrix_never_held", SQUARE, "            held: found(&pick).is_some_and(|(_, held)| held),",
            "            held: false,", T_SQUARE, "rust"),
    # ---- the follow-up: the two lines named uncovered ----
    Control("p97_comment_reads_as_plain", SEAT, "    let (key, value) = entries.next()?;",
            "    let Some((key, value)) = entries.next() else {\n"
            '        return Some(("#".to_owned(), toml::Value::Boolean(true)));\n    };', T_ALONE, "rust"),
    Control("p97_live_world_always_reads", SEAT,
            "    if table.calibration {\n        crate::nirvana_seat_calib::Factors::for_this_project(dir, table)",
            "    if true {\n        crate::nirvana_seat_calib::Factors::for_this_project(dir, table)", T_LIVE, "rust"),
    # ---- the P46a review's fix round. Expected red, written before the run: the test named in each line fails at
    # the assertion the comment names. ----
    # Finding 1: a factor belongs to the cells it was measured on. Red: "fits" on today's refit cell.
    Control("p97_stale_factor_fits", CALIB, "        self.table_fingerprint == fingerprint(table)\n",
            "        self.table_fingerprint == fingerprint(table) || !table.seats.is_empty()\n", T_STALE, "rust"),
    # Red: "not due at once" for every changed table.
    Control("p97_stale_not_due", CALIB, "            !cal.fits(table)\n                || now",
            "            false\n                || now", T_STALE, "rust"),
    # Red: "overrides_per_run: does not fit" (an unrelated edit dropped the factors).
    Control("p97_fingerprint_reads_constants", CALIB,
            '    let _ = writeln!(text, "size.medium.use {:?}", table.size.medium.use_factor);\n',
            '    let _ = writeln!(text, "size.medium.use {:?}", table.size.medium.use_factor);\n'
            '    let _ = writeln!(text, "overrides {}", table.constants.overrides_per_run);\n', T_STALE, "rust"),
    # Red: "a price: fits".
    Control("p97_fingerprint_no_prices", CALIB, "        let mut prices: Vec<_> = seat.prices.iter().collect();",
            "        let mut prices: Vec<_> = seat.prices.iter().take(0).collect();", T_STALE, "rust"),
    # Red: the square's "nothing booked" on a stale file.
    Control("p97_square_books_stale", SQUARE, "            .filter(|cal| table.calibration && cal.fits(table))",
            "            .filter(|_| table.calibration)", T_SQUARE, "rust"),
    # Red: the square's summary is the stale file's own ("checked"), not STALE_SUMMARY.
    Control("p97_square_stale_unsaid", SQUARE, "    if calibration.fits(table) {\n        match crate::nirvana_seat_calib::dropped_rows",
            "    if true {\n        match crate::nirvana_seat_calib::dropped_rows", T_SQUARE, "rust"),
    # Second review F6: a factor outside CLAMP is dropped on read, not clamped. Red: "1e9: dropped" (opus high is
    # booked at 1e9).
    Control("p97_read_out_of_bounds_booked", CALIB,
            "    factor.is_finite() && (CLAMP.0..=CLAMP.1).contains(&factor)",
            "    factor.is_finite() && factor > 0.0", T_CLAMP_READ, "rust"),
    # Red: hydrogen low (0) is booked ("Low: dropped").
    Control("p97_read_keeps_zero", CALIB, "    factor.is_finite() && (CLAMP.0..=CLAMP.1).contains(&factor)",
            "    factor.is_finite() && factor <= CLAMP.1", T_CLAMP_READ, "rust"),
    # Red: dropped_rows is 0, not 5 ("0, -2, NaN, 1e9 and 0.1").
    Control("p97_dropped_rows_uncounted", CALIB,
            "        .filter(|row| row.n >= MIN_N && row.factor.is_some_and(|factor| !factor_in_bounds(factor)))",
            "        .filter(|_| false)", T_CLAMP_READ, "rust"),
    # Red: a file of another version reads.
    Control("p97_any_version_reads", CALIB, "        .filter(|cal| cal.version == VERSION)", "        .filter(|_| true)",
            T_CLAMP_READ, "rust"),
    # Red: one text-valued factor loses the whole file ("one bad row does not lose the file").
    Control("p97_factor_strict", CALIB,
            "    Ok(lenient_number(Option::<serde_json::Value>::deserialize(\n        de,\n    )?))\n}\n\nfn lenient_f64",
            "    Option::<f64>::deserialize(de)\n}\n\nfn lenient_f64", T_CLAMP_READ, "rust"),
    # Finding 4: bounded. Red: the 5000-byte journal is read (large 0, files 3).
    Control("p97_journal_no_size_cap", CALIB, "            Ok(meta) if meta.len() > limits.max_bytes => {",
            "            Ok(meta) if false && meta.len() > limits.max_bytes => {", T_BOUNDED, "rust"),
    # Red: capped.files is 2, not 1.
    Control("p97_journal_no_file_cap", CALIB, "        if journals.files >= limits.max_files {",
            "        if false {", T_BOUNDED, "rust"),
    # Red: late.files is 2, not 0.
    Control("p97_journal_no_budget", CALIB, "        if started.elapsed() >= limits.budget {",
            "        if false {", T_BOUNDED, "rust"),
    # Red: the summary does not end with the skipped journals.
    Control("p97_skipped_unsaid", CALIB, '        calibration.summary.push_str(&format!("; {words}"));',
            "        let _ = words;", T_BOUNDED, "rust"),
    # Red: "backing off" (due a second after the failed write).
    Control("p97_failed_write_replays", CALIB, "    if backing_off(dir, now) {\n        return false;",
            "    if false && backing_off(dir, now) {\n        return false;", T_BOUNDED, "rust"),
    # Finding 2: no upgrade without a backup. Red: the carried table is rewritten with the folder blocked.
    Control("p97_upgrade_without_backup", SEAT, "        None => match take_backup(path, &bytes, &backup_dir) {",
            "        None => match take_backup(path, &bytes, &backup_dir)\n"
            '            .or_else(|_| Ok::<PathBuf, String>(backup_dir.join("none")))\n        {', T_NO_BACKUP, "rust"),
    # Red: the log holds two lines after the second attempt (second fix round: the test that removes the refusal
    # note between attempts; T_NO_BACKUP's second read is now skipped by the note).
    Control("p97_refusal_said_every_read", SEAT, "        if said_last {\n            return;",
            "        if said_last && false {\n            return;", T_REFUSALS, "rust"),
    # Finding 5. Red: an unknown added key (`mystery = 1`) is taken and the table rewritten.
    Control("p97_added_unknown_key_taken", SEAT,
            "!theirs.contains_key(*key) && known.contains_key(*key)", "!theirs.contains_key(*key)", T_ADDED, "rust"),
    # Finding 6. Red: the hydrogen row of the doubled fixture has n 14.
    Control("p97_cost_reads_every_line", CALIB, "    for line in last.values() {", "    for line in &kept {", T_DEDUPE,
            "rust"),
    # Finding 7. Red: the trailing space is carried and the upgraded table is not the default.
    Control("p97_trailing_space_is_an_edit", SEAT, "        if mine.trim_end() == theirs.trim_end() {",
            "        if mine == theirs {", T_TRAILING, "rust"),
    # The dropped-key path. Red: the log reads "keeping your retired_knob = 2".
    Control("p97_dropped_named_as_kept", SEAT, "            .filter(|(_, key, _, _)| !dropped.contains(key))",
            "            .filter(|_| true)", T_DROPPED, "rust"),
    # ---- the P46a second review's fix round (expected red results written before the run) ----
    # F1. Red in T_FAILED_WRITE: a failed rename is taken for done, so "no backup left" fails (the backup stays) and the
    # log says "took".
    Control("p97_failed_rename_taken", SEAT, "    if let Err(err) = std::fs::rename(&temp, path) {",
            "    if let Err(err) = std::fs::rename(&temp, path).or(Ok::<(), std::io::Error>(())) {", T_FAILED_WRITE,
            "rust"),
    # Red in T_FAILED_WRITE: "no backup left" (the failed attempt keeps its backup, and the next keeps another).
    Control("p97_failed_attempt_keeps_backup", SEAT,
            "        if made {\n            let _ = std::fs::remove_file(&backup);\n        }",
            "        let _ = made;", T_FAILED_WRITE, "rust"),
    # Red in T_FAILED_WRITE: "skipped, not tried" (every read tries again and rewrites the note).
    Control("p97_refusal_retried_every_read", SEAT, "    if refused_lately(dir, &key, now()) {",
            "    if false && refused_lately(dir, &key, now()) {", T_FAILED_WRITE, "rust"),
    # Red in T_NOTE: "dated after now" (a note from the future stands).
    Control("p97_note_from_the_future_stands", SEAT,
            "        .is_ok_and(|at| at <= now && now < at.saturating_add(REFUSED_S))",
            "        .is_ok_and(|at| now < at.saturating_add(REFUSED_S))", T_NOTE, "rust"),
    # Red in T_REUSE: three backups, not two (the one holding the table is copied again).
    Control("p97_backup_not_reused", SEAT, "    let (backup, made) = match existing_backup(&backup_dir, &bytes) {",
            "    let (backup, made) = match None::<PathBuf> {", T_REUSE, "rust"),
    # F2. Red in T_REFUSALS: "a backup file left" (the half copy and the wrong copy stay).
    Control("p97_failed_backup_left", SEAT,
            "    let failed = |why: String| {\n        let _ = std::fs::remove_file(&backup);",
            "    let failed = |why: String| {\n        let _ = &backup;", T_REFUSALS, "rust"),
    # Red in T_REFUSALS: two log lines for the copy kind (its words carry the dated name, a second apart).
    Control("p97_refusal_names_dated_file", SEAT,
            '            "a backup cannot be written in {} ({err})",\n            backup_dir.display()',
            '            "a backup cannot be written in {} ({err})",\n            backup.display()', T_REFUSALS, "rust"),
    # The read-back itself. Red in T_REFUSALS: the "wrong" kind upgrades over a backup holding other bytes.
    Control("p97_backup_not_compared", SEAT, "        Ok(copy) if copy == bytes => Ok(backup),",
            "        Ok(_) => Ok(backup),", T_REFUSALS, "rust"),
    # F3. Red in T_UNREADABLE: the table that is not UTF-8 is replaced by the default ("starts_with(UNREADABLE)").
    Control("p97_unreadable_overwritten", SEAT, "        Ok(bytes) => String::from_utf8(bytes).map_err(|err| {",
            "        Ok(bytes) => String::from_utf8(bytes).or_else(|_| {\n"
            "            let _ = std::fs::write(path, DEFAULT);\n"
            "            Ok::<String, std::string::FromUtf8Error>(DEFAULT.to_owned())\n"
            "        }).map_err(|err| {", T_UNREADABLE, "rust"),
    # F4. Red in T_RACE: "the owner's save stands" (the upgrade writes over the save).
    Control("p97_save_written_over", SEAT, "    if std::fs::read(path).ok().as_deref() != Some(&bytes[..]) {",
            "    if false {", T_RACE, "rust"),
    # F5. Red in T_CARRY: "a refusal note" (nothing is said, so there is no note to remove).
    Control("p97_refused_carry_silent", SEAT,
            "            refuse(&refused_carry(&current_canonical, &carried, &problems));",
            "            let _ = &problems;", T_CARRY, "rust"),
    # Red in T_CARRY: "no 'keeps your' for a refused table" (today's default with version = 1001 keeps it).
    Control("p97_differs_promises_refused", SEAT,
            ".filter(|carried| !carried.lines.is_empty() && parse(text).is_ok())",
            ".filter(|carried| !carried.lines.is_empty())", T_CARRY, "rust"),
    # BOM. Red in T_BOM: v15's log is empty, not one line.
    Control("p97_bom_silent", SEAT, "    if bom {\n        refuse(BOM_WORDS);", "    if bom {\n        let _ = BOM_WORDS;",
            T_BOM, "rust"),
    # Wording. Red in T_CHECK_LINE: a file with no fingerprint reads STALE_SUMMARY.
    Control("p97_unbound_said_stale", SQUARE, "    } else if calibration.table_fingerprint.is_empty() {",
            "    } else if false {", T_CHECK_LINE, "rust"),
    # Red in T_CHECK_LINE: the summary has no dropped-rows count.
    Control("p97_dropped_unsaid", SQUARE, "            1 => format!(\"{}; {DROPPED_ONE}\", calibration.summary),",
            "            1 => calibration.summary.clone(),", T_CHECK_LINE, "rust"),
    # F3 in the square. Red in T_CHECK_LINE: "no table shown for a file that is not UTF-8".
    Control("p97_square_unreadable_default", SQUARE, "        Ok(Ok(text)) => text,",
            "        Ok(Ok(text)) => text,\n        Ok(Err(_)) => nirvana_seat::DEFAULT.to_owned(),", T_CHECK_LINE, "rust"),
    # Section 5. Red in T_FUTURE: "a month ahead: ignored" (the note holds checks off).
    Control("p97_future_backoff_stands", CALIB,
            "        .is_some_and(|at| at <= now && now < at.saturating_add(BACKOFF_S))",
            "        .is_some_and(|at| now < at.saturating_add(BACKOFF_S))", T_FUTURE, "rust"),
    # Red in T_FPRINT: "every block moved" (seats hashed in file order).
    Control("p97_fingerprint_unsorted", CALIB, "    seats.sort_by(|a, b| a.id.cmp(&b.id));",
            "    let _ = &mut seats;", T_FPRINT, "rust"),
    # Red in T_FPRINT: "pool ... per_usd 5h" (only the week window hashed).
    Control("p97_fingerprint_week_only", CALIB,
            '        let _ = writeln!(text, "pool {} per_usd {:?}", pool.id, pool.per_usd);',
            '        let _ = writeln!(text, "pool {} per_usd {:?}", pool.id, pool.per_usd.get(WINDOW));', T_FPRINT, "rust"),
)
