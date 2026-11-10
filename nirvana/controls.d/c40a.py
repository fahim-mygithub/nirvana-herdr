# Plan 40a (2026-11-09): Fable 5.2 takes a seat, Nirvana Claude's seat comes from the table, and the seat review's
# bugs (nirvana/plans/2026-11-09-40-records/p40a-seat.md). Controls starting with c40a_. One sabotage per assertion, each
# on a product line; the test named must print FAIL with it in place (the expected red line, written before the run).
# `Control` is provided by controls.py.
SEAT = "src/nirvana_seat.rs"
TOML = "src/nirvana_seats.toml"
USAGE = "src/nirvana_usage.rs"
ACCOUNTS = "src/nirvana_accounts.rs"
LAUNCH = "src/nirvana_launch.rs"
IDLE = "src/nirvana_idle_review.rs"
SQUARE = "src/nirvana_seats_square.rs"
SHELL = "src/client/shell/nirvana_seats.rs"

T_UPGRADE = "c40a_the_shipped_default_is_new_and_every_old_one_upgrades"
T_DIFF = "c40a_an_edited_table_differs_and_seat_diff_shows_the_change"
T_FABLE = "c40a_fable_is_skipped_unless_its_windows_are_fresh_and_its_reserve_holds"
T_ORCH = "c40a_sota_orchestrator_falls_back_to_opus_with_why"
T_SOTA = "orchestrator_seat_sota_mode"
T_NAME = "c40a_a_renamed_fable_line_is_still_week_fable"
T_LAUNCH = "c40a_the_launch_carries_the_subagent_model_and_the_autocompact_window"
T_B1 = "c40a_jev_is_asked_outside_the_lease_lock"
T_B2 = "c40a_the_gate_reads_agys_newest_log_only"
T_B3 = "c40a_a_write_waits_for_another_processs_lock_on_the_cache"
T_B4 = "c40a_run_returns_when_the_child_exits_and_a_grandchild_holds_stdout"
T_B5 = "c40a_a_damaged_idle_review_file_is_a_doubt_never_on"
T_T2 = "c40a_a_sign_in_is_read_once_a_minute"
T_SQUARE = "c40a_the_square_shows_sotas_fallback_and_an_edited_table"

CONTROLS = (
    # Step 1: the default before Fable 5.2 is a historical one, so an installed copy upgrades.
    Control("c40a_v12_historical", SEAT,
            '    // upgrades.\n    include_str!("nirvana_seats_v12.toml"),\n',
            '    // upgrades.\n',
            T_UPGRADE, "rust"),
    # Fail closed: a window with no reading skips the seat.
    Control("c40a_fable_unread_skips", SEAT,
            "        if let Some(why) = window\n            .used\n            .is_none()\n",
            "        if let Some(why) = window\n            .used\n            .is_some()\n",
            T_FABLE, "rust"),
    # ... and a reading that is stale.
    Control("c40a_fable_stale_skips", SEAT,
            "        Some(reading) if nirvana_usage::freshness(reading, world.now) != Freshness::Fresh => {\n",
            "        Some(reading) if false && nirvana_usage::freshness(reading, world.now) != Freshness::Fresh => {\n",
            T_FABLE, "rust"),
    # ... and an order that would leave less than `fable_reserve` of the Fable week.
    Control("c40a_fable_reserve", SEAT,
            "        if after + EDGE < reserve {\n",
            "        if after + EDGE < 0.0 * reserve {\n",
            T_FABLE, "rust"),
    # The window name: "Fable 5.2" and "Fable models" are week.fable.
    Control("c40a_week_scope_used", USAGE,
            "                let name = week_scope(name);\n",
            "",
            T_NAME, "rust"),
    Control("c40a_week_scope_models", USAGE,
            '        let models = last.eq_ignore_ascii_case("model") || last.eq_ignore_ascii_case("models");\n',
            '        let models = false;\n',
            T_NAME, "rust"),
    # The orchestrator: a seat with its own window is checked before SOTA takes it.
    Control("c40a_orch_scoped_check", SEAT,
            "fn scoped_orchestrator_ok(table: &Table, world: &World, seat: &Seat) -> Result<(), String> {\n    if seat.extra_windows.is_empty() {\n",
            "fn scoped_orchestrator_ok(table: &Table, world: &World, seat: &Seat) -> Result<(), String> {\n    if true || seat.extra_windows.is_empty() {\n",
            T_ORCH, "rust"),
    # ... the threshold is `fable_orchestrator_left`, not the workers' reserve,
    Control("c40a_orch_left_threshold", SEAT,
            "        if 1.0 - used - booked + EDGE < table.constants.fable_orchestrator_left {\n",
            "        if 1.0 - used - booked + EDGE < table.constants.fable_reserve {\n",
            T_ORCH, "rust"),
    # ... booked Fable orders count,
    Control("c40a_orch_counts_booked", SEAT,
            "        if 1.0 - used - booked + EDGE < table.constants.fable_orchestrator_left {\n",
            "        if 1.0 - used + EDGE < table.constants.fable_orchestrator_left {\n",
            T_ORCH, "rust"),
    # ... an unread window says "not read",
    Control("c40a_orch_unread_reason", SEAT,
            "        if let Some(why) = unread_why(world, &seat.pool, window) {\n            return Err(why);\n",
            "        if let Some(why) = unread_why(world, &seat.pool, window).filter(|_| false) {\n            return Err(why);\n",
            T_ORCH, "rust"),
    # ... and the square shows the reason, not the bare mode.
    Control("c40a_orch_square_reason", SEAT,
            "            _ => self.reason.clone(),\n",
            "            _ => self.mode.title().to_string(),\n",
            T_ORCH, "rust"),
    # A table without `[orchestrator]` seats SOTA on Opus high, as before plan 40a.
    Control("c40a_orch_default_is_todays", SEAT,
            '            sota: OrchestratorChoice::new("opus", Effort::High),\n',
            '            sota: OrchestratorChoice::new("fable", Effort::High),\n',
            T_SOTA, "rust"),
    # The shipped table seats SOTA on Fable high.
    Control("c40a_toml_sota_fable", TOML,
            'sota = { seat = "fable", effort = "high" }\n',
            'sota = { seat = "opus", effort = "high" }\n',
            T_SOTA, "rust"),
    # The launch: CLAUDE_CODE_SUBAGENT_MODEL in the launch settings' env,
    Control("c40a_subagent_env", LAUNCH,
            "    env.insert(SUBAGENT_MODEL_ENV.to_owned(), model.into());\n",
            "",
            T_LAUNCH, "rust"),
    # ... `--autocompact <tokens>` in the argv,
    Control("c40a_autocompact_flag", LAUNCH,
            "    if tokens > 0 {\n",
            "    if false && tokens > 0 {\n",
            T_LAUNCH, "rust"),
    # ... and a window outside Claude Code's range refused.
    Control("c40a_autocompact_range", SEAT,
            "        if c.autocompact_tokens != 0 && !(100_000..=1_000_000).contains(&c.autocompact_tokens) {\n",
            "        if false && c.autocompact_tokens != 0 && !(100_000..=1_000_000).contains(&c.autocompact_tokens) {\n",
            T_LAUNCH, "rust"),
    # B1: Jev is asked with the lease lock free.
    Control("c40a_b1_lock_before_jev", SEAT,
            "    let advised = if booked_already {\n",
            "    let _early = Lock::take(dir).map_err(BookError::Io)?;\n    let advised = if booked_already {\n",
            T_B1, "rust"),
    # ... and advice is kept only from the same contest.
    Control("c40a_b1_same_contest", SEAT,
            "    a.seat == b.seat && a.effort == b.effort && line(a) == line(b)\n",
            "    a.seat == b.seat && a.effort == b.effort\n",
            T_B1, "rust"),
    # B2: an old newest log with no sign-in line is signed out for the gate.
    Control("c40a_b2_newest_only", ACCOUNTS,
            "    if !settling {\n        return None;\n    }\n",
            "    if false {\n        return None;\n    }\n",
            T_B2, "rust"),
    # B3: the cache's read-merge-write is under the OS lock.
    Control("c40a_b3_cache_lock", USAGE,
            "    lock.lock()?;\n    let mut cache = read_cache(path);\n",
            "    let mut cache = read_cache(path);\n",
            T_B3, "rust"),
    # B4: after the child exits, stdout's EOF is waited for a bounded time.
    Control("c40a_b4_bounded_join", USAGE,
            "    let _ = reader.recv_timeout(AFTER_EXIT);\n",
            "    let _ = reader.recv();\n",
            T_B4, "rust"),
    # B5: a global file that does not read is a doubt,
    Control("c40a_b5_global_doubt", IDLE,
            "            doubt = doubt.or(Some(err.to_string()));\n            Global::default()\n",
            "            let _ = err;\n            Global::default()\n",
            T_B5, "rust"),
    # ... a parse error is never the default,
    Control("c40a_b5_parse_is_error", IDLE,
            '            .map_err(|err| format!("{} does not parse: {err}", path.display())),\n',
            '            .or_else(|_| Ok(T::default())),\n',
            T_B5, "rust"),
    # ... and a damaged space file is a doubt for its space.
    Control("c40a_b5_space_doubt", IDLE,
            "            doubt: doubt.clone().or(space_doubt),\n",
            "            doubt: doubt.clone().or(space_doubt.filter(|_| false)),\n",
            T_B5, "rust"),
    # M2: an old default is not "differs" (the next read upgrades it),
    Control("c40a_m2_historical_not_differs", SEAT,
            "    text != canonical(DEFAULT) && !HISTORICAL_DEFAULTS.iter().any(|old| text == canonical(old))\n",
            "    text != canonical(DEFAULT)\n",
            T_UPGRADE, "rust"),
    # ... the square reads it,
    Control("c40a_m2_square_reads", SQUARE,
            "    view.differs = differs;\n",
            "    view.differs = differs && false;\n",
            T_SQUARE, "rust"),
    # ... and shows its line.
    Control("c40a_m2_square_line", SHELL,
            "    if view.differs {\n",
            "    if false && view.differs {\n",
            T_SQUARE, "rust"),
    # ... and `seat diff` prints a removal before its addition.
    Control("c40a_m2_diff_order", SEAT,
            "        } else if i < n && (j == m || lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {\n",
            "        } else if i < n && (j == m || lcs[at(i + 1, j)] > lcs[at(i, j + 1)]) {\n",
            T_DIFF, "rust"),
    # T2: a sign-in answer is kept a minute.
    Control("c40a_t2_kept", SEAT,
            "        .filter(|entry| entry.at <= now && now - entry.at < SIGNIN_KEEP_S)\n",
            "        .filter(|_| false)\n",
            T_T2, "rust"),
)
