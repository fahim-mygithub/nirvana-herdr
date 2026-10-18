# Plan 12 O8b: Jev's roles 2 and 3 in shadow (nirvana/plans/2026-10-17-12-records/o8b.md). One sabotage per
# assertion; each test named must print FAIL with its sabotage in place. `Control` is provided by controls.py.
ROLES = "src/nirvana_jev_roles.rs"
RUN = "src/nirvana_run.rs"
SEAT = "src/nirvana_seat.rs"

SHADOW = "shadow_records_both_picks_and_changes_nothing"
FLOOR = "the_choice_floor_and_a_none_are_never_a_seat"
FAILED = "a_failed_call_is_never_a_choice_and_the_pick_stands"
QUIET = "no_key_off_one_contender_or_an_override_asks_nothing"
ON = "on_takes_jevs_seat_through_the_override_guard_and_the_run_cap_holds"
QUESTION = "the_seating_question_offers_each_contender_and_none_and_carries_no_order_text"
SETTINGS = "the_setting_is_shadow_by_default_off_for_a_word_it_does_not_know_and_set_by_the_cli"
AGREE = "agree_counts_agreement_difference_declines_failures_and_the_watch"
FLOORS = "the_floors_make_a_yes_a_no_or_an_unsure"
DUE = "a_worker_is_due_after_ten_minutes_with_a_pane_and_not_twice_in_five"
WSHADOW = "shadow_writes_the_watch_event_and_wakes_nobody"
WON = "on_queues_a_yes_for_the_wake_and_a_no_or_a_failure_queues_nothing"
TWO = "the_rule_leaves_more_than_one_seat_for_the_class_these_tests_use"
PRINTS = "the_pick_prints_its_contenders_and_jevs_advice_in_json"
SCREEN = "a_screen_is_cut_to_its_tail_and_redacted_before_it_is_sent"
QUIET_WATCH = "off_and_no_key_read_no_screen_and_ask_nothing"

CONTROLS = (
    # Role 3: the choice floor, none, failure.
    Control("jevr_choice_floor_ignored", ROLES, "    if confidence < CHOICE_FLOOR {", "    if false {", FLOOR, "rust"),
    Control("jevr_none_taken_as_a_seat", ROLES,
            "    if choice == NONE || !offered.iter().any(|c| c.seat == choice) {", "    if false {", FLOOR, "rust"),
    Control("jevr_failure_names_a_seat", ROLES,
            '                seat: None,\n                answer: "failed".into(),',
            '                seat: Some(rule.clone()),\n                answer: "failed".into(),', FAILED, "rust"),
    Control("jevr_failure_logged_answered", ROLES,
            '            entry["outcome"] = "failed".into();', '            entry["outcome"] = "answered".into();',
            FAILED, "rust"),
    # Role 3: shadow changes nothing, on goes through the guard.
    Control("jevr_shadow_acts", ROLES,
            "    if setting == Setting::On && act {", "    if setting != Setting::Off && act {", SHADOW, "rust"),
    Control("jevr_named_seat_overridden", ROLES,
            "    if setting == Setting::On && act {", "    if setting == Setting::On {", ON, "rust"),
    Control("jevr_guard_bypassed", ROLES,
            "            match crate::nirvana_seat::pick(table, world, &over) {",
            "            match crate::nirvana_seat::pick(table, &World { overrides_used: 0, ..world.clone() }, &over) {",
            ON, "rust"),
    Control("jevr_off_still_asks", ROLES,
            "    if setting == Setting::Off || pick.override_reason", "    if false || pick.override_reason", QUIET, "rust"),
    Control("jevr_override_still_asked", ROLES,
            "pick.override_reason.is_some() || request.override_seat.is_some()", "request.override_seat.is_some()",
            QUIET, "rust"),
    Control("jevr_single_contender_asked", ROLES,
            "    if pick.contenders.len() < 2 {", "    if pick.contenders.is_empty() {", QUIET, "rust"),
    Control("jevr_no_key_logs", ROLES,
            "    let Some(client) = client else {\n        return pick;\n    };",
            "    let Some(client) = client else {\n        log_call(dir, Role::Seating, \"x\", Err(&JevError::Unavailable), json!({}));\n        return pick;\n    };",
            QUIET, "rust"),
    # Role 3: the reason, the log, the question.
    Control("jevr_reason_silent", ROLES,
            '    pick.reason.push_str(&format!("; Jev ({mode}) {said}"));', "    let _ = &said;", SHADOW, "rust"),
    Control("jevr_agreement_unmarked", ROLES,
            "    let agrees = seat.as_deref() == Some(rule.as_str());", "    let agrees = false;",
            "jev_agreeing_with_the_rule_is_said_and_logged_as_the_rules_seat", "rust"),
    Control("jevr_log_rule_dropped", ROLES, 'json!({"rule": rule, "mode": mode, "offered"', 'json!({"mode": mode, "offered"',
            SHADOW, "rust"),
    Control("jevr_log_keeps_the_text", ROLES,
            'json!({"rule": rule, "mode": mode, "offered"', 'json!({"state": state, "rule": rule, "mode": mode, "offered"',
            SHADOW, "rust"),
    Control("jevr_hash_long", ROLES, ".take(16)", ".take(40)", SHADOW, "rust"),
    Control("jevr_log_cost_dropped", ROLES,
            '            entry["cost_usd"] = reply.cost_usd.map_or(Value::Null, Value::from);',
            '            entry["cost_usd"] = Value::Null;', SHADOW, "rust"),
    Control("jevr_log_latency_dropped", ROLES, '            entry["ms"] = reply.ms.into();', '            entry["ms"] = Value::Null;',
            SHADOW, "rust"),
    Control("jevr_log_score_dropped", ROLES, '            entry["score"] = score.into();', '            entry["score"] = Value::Null;',
            SHADOW, "rust"),
    Control("jevr_none_option_missing", ROLES, "    criteria.insert(\n        NONE.into(),",
            '    criteria.insert(\n        "other".into(),', QUESTION, "rust"),
    Control("jevr_first_contender_left_out", ROLES, "    for c in &pick.contenders {\n        criteria.insert",
            "    for c in pick.contenders.iter().skip(1) {\n        criteria.insert", QUESTION, "rust"),
    Control("jevr_mode_not_sent", ROLES, '"mode": request.mode.word(),', '"mode": "balanced",', QUESTION, "rust"),
    # The pick's two fields.
    Control("jevr_no_contenders", SEAT, "        contenders: if beaten.is_empty() {", "        contenders: if true {",
            TWO, "rust"),
    Control("jevr_contenders_pick_last", SEAT,
            "            std::iter::once(winner)\n                .chain(beaten.iter().copied())",
            "            beaten\n                .iter()\n                .copied()\n                .chain(std::iter::once(winner))",
            TWO, "rust"),
    Control("jevr_advice_always_printed", SEAT,
            '    #[serde(skip_serializing_if = "Option::is_none")]\n    pub jev_pick:', "    pub jev_pick:",
            PRINTS, "rust"),
    Control("jevr_contenders_always_printed", SEAT,
            '    #[serde(skip_serializing_if = "Vec::is_empty")]\n    pub contenders:', "    pub contenders:",
            PRINTS, "rust"),
    Control("jevr_seated_writes_none", RUN,
            '    #[serde(default, skip_serializing_if = "Option::is_none")]\n    pub jev_pick:', "    #[serde(default)]\n    pub jev_pick:",
            "a_jev_pick_rides_the_seated_event_and_an_old_line_without_it_still_reads", "rust"),
    # The setting.
    Control("jevr_typo_reads_on", ROLES, "            .unwrap_or(Setting::Off),", "            .unwrap_or(Setting::On),", SETTINGS, "rust"),
    Control("jevr_default_off", ROLES, "        None => Setting::Shadow,", "        None => Setting::Off,", SETTINGS, "rust"),
    Control("jevr_write_clobbers_the_other", ROLES, "        Role::Watch => (setting, now.seating),",
            "        Role::Watch => (setting, Setting::Shadow),", SETTINGS, "rust"),
    # `herdr jev agree`.
    Control("jevr_agree_never_agrees", ROLES, "                } else if answer == rule {", "                } else if false {", AGREE, "rust"),
    Control("jevr_agree_no_declines", ROLES, '                if answer == NONE || answer == "unsure" {',
            "                if false {", AGREE, "rust"),
    Control("jevr_agree_failures_uncounted", ROLES, '            Some("seating") if failed => out.seating_failed += 1,',
            '            Some("seating") if failed => {}', AGREE, "rust"),
    Control("jevr_agree_every_watch_yes", ROLES, '                if entry["answer"].as_str() == Some("yes") {',
            "                if true {", AGREE, "rust"),
    Control("jevr_agree_cost_uncounted", ROLES, '            out.cost_usd += entry["cost_usd"].as_f64().unwrap_or(0.0);',
            "", AGREE, "rust"),
    Control("jevr_agree_percent_scaled", ROLES, "f64::from(a.seating_agreed) * 100.0", "f64::from(a.seating_agreed) * 10.0",
            AGREE, "rust"),
    Control("jevr_agree_empty_says_zero", ROLES, '"no choice yet".to_owned()', '"0%".to_owned()', AGREE, "rust"),
    # Role 2: the floors.
    Control("jevr_yes_floor_exclusive", ROLES, "    if flag >= YES_FLOOR {", "    if flag > YES_FLOOR {", FLOORS, "rust"),
    Control("jevr_yes_floor_low", ROLES, "    if flag >= YES_FLOOR {", "    if flag >= 0.5 {", FLOORS, "rust"),
    Control("jevr_no_ceiling_exclusive", ROLES, '        answer: if flag <= NO_CEILING { "no" }', '        answer: if flag < NO_CEILING { "no" }',
            FLOORS, "rust"),
    Control("jevr_no_ceiling_high", ROLES, '        answer: if flag <= NO_CEILING { "no" }', '        answer: if flag <= 0.5 { "no" }',
            FLOORS, "rust"),
    Control("jevr_unsure_kind_trusted", ROLES, "    let sure_kind = confidence >= CHOICE_FLOOR;", "    let sure_kind = true;", FLOORS, "rust"),
    Control("jevr_contradiction_is_a_yes", ROLES, '        if sure_kind && kind == "working" {', "        if false {", FLOORS, "rust"),
    # Role 2: the screen.
    Control("jevr_screen_not_redacted", ROLES, "crate::nirvana_jev::redact(&tail[start..], key)",
            "(tail[start..].to_owned(), 0usize)", SCREEN, "rust"),
    Control("jevr_screen_not_cut", ROLES, "lines[lines.len().saturating_sub(SCREEN_LINES)..]", "lines[..]",
            SCREEN, "rust"),
    Control("jevr_watch_failure_unlogged", ROLES, "            log_call(dir, Role::Watch, &hash, Err(&error), extra);",
            "            let _ = (&hash, &extra);", "a_failed_watch_is_never_a_yes_and_the_log_says_failed", "rust"),
    # Role 2: who is due.
    Control("jevr_due_at_ten_minutes_exactly", ROLES, "        if now.saturating_sub(started) <= WATCH_AFTER_S {",
            "        if now.saturating_sub(started) < WATCH_AFTER_S {", DUE, "rust"),
    Control("jevr_due_always_rewatched", ROLES, "        if watched.is_some_and(|at| now.saturating_sub(at) < WATCH_EVERY_S) {",
            "        if false {", DUE, "rust"),
    Control("jevr_due_without_supervisor", ROLES, " || !live(order.id())", "", DUE, "rust"),
    Control("jevr_due_headless", ROLES, "        let Some(pane) = &order.pane else { continue };",
            "        let pane = &order.pane.clone().unwrap_or_default();", DUE, "rust"),
    Control("jevr_due_finished_order", ROLES, "        if order.phase != Phase::Running", "        if false",
            DUE, "rust"),
    Control("jevr_due_first_start", ROLES, "            .max()\n            .unwrap_or(order.seated_at);",
            "            .min()\n            .unwrap_or(order.seated_at);", DUE, "rust"),
    # Role 2: shadow, on, the tick.
    Control("jevr_watch_shadow_acts", ROLES, "        let acted = setting == Setting::On;", "        let acted = true;", WSHADOW, "rust"),
    Control("jevr_queue_ignores_acted", RUN, "            acted: true,\n        } if answer", "            acted: _,\n        } if answer",
            WSHADOW, "rust"),
    Control("jevr_queue_every_answer", RUN, '        } if answer == "yes" => format!(', "        } if true => format!(", WON, "rust"),
    Control("jevr_watch_failure_is_yes", ROLES, '            Err(_) => ("failed", None, 0.0),', '            Err(_) => ("yes", None, 0.9),',
            WON, "rust"),
    Control("jevr_watch_kind_dropped", ROLES, "                kind: kind.clone(),\n                score,", "                kind: None,\n                score,",
            WSHADOW, "rust"),
    Control("jevr_watch_reads_wrong_pane", ROLES, "read_screen(&target.pane)", 'read_screen("x")', WSHADOW, "rust"),
    Control("jevr_watch_off_reads", ROLES, "client.filter(|_| setting != Setting::Off)", "client",
            QUIET_WATCH, "rust"),
    Control("jevr_watch_ended_run", ROLES, "    if state.ended.is_some() {\n        return Ok(lines);", "    if false {\n        return Ok(lines);",
            "an_ended_run_is_not_watched", "rust"),
)
