# Order LEARN (2026-11-09): lessons reach the orders they would help, and a memory review in idle time on spare usage
# (nirvana/plans/2026-11-09-learn-idle.md). Controls starting with learn_. One sabotage per assertion, each on a product
# line; the test named must print FAIL with it in place (the expected red line, written before the run). `Control` is
# provided by controls.py.
LESSONS = "src/nirvana_lessons.rs"
IDLE = "src/nirvana_idle_review.rs"
REVIEW = "src/nirvana_review.rs"
LEARN = "src/nirvana_learning.rs"

T_ROUTED = "learn_a_worker_lesson_is_kept_under_its_orders_class"
T_RECLASS = "learn_reclass_moves_old_lessons_through_the_ledger_and_rolls_back"
T_RETIRE = "learn_retire_takes_only_a_lesson_that_never_helped"
T_IDLE = "learn_idle_needs_a_quiet_owner_and_quiet_orders"
T_SPARE = "learn_a_missing_or_old_spare_reading_means_no_review"
T_HALF = "learn_the_spare_must_be_half_unused_and_reset_soon"
T_CAPS = "learn_caps_per_space_per_day_and_per_window"
T_BUSY = "learn_never_while_an_order_runs_and_the_switch_turns_it_off"
T_TICK = "learn_a_tick_queues_one_idle_review_and_its_gate_applies_the_ops"
T_FACTS = "learn_the_idle_facts_come_from_keys_journals_and_streams"
# Review round (learn-review.md).
T_LEASES = "learn_an_unreadable_lease_file_means_no_review"
T_WEEK = "learn_the_week_must_have_room_too"
T_BACKOFF = "learn_a_failed_queue_counts_once_and_backs_off"
T_CRASH = "learn_a_tick_that_dies_after_its_reservation_never_runs_the_orders_twice"
T_MOVED = "learn_use_of_a_pool_by_anyone_stops_a_review"
T_EDITED = "learn_reclass_leaves_a_lesson_the_owner_edited"
T_LAUNCH = "learn_a_launch_that_fails_is_a_failure"
T_TICKFACTS = "learn_a_tick_reads_the_owners_key_a_fresh_stream_and_its_lock"
T_KNOWN = "learn_known_projects_skip_fixtures_gone_roots_and_spellings"
T_NEVER = "learn_a_seated_review_that_never_started_is_settled_once_its_lease_ends"

CONTROLS = (
    # The class fix: the gate keeps a worker's lesson under its order's seated class, and the offer reaches it.
    Control("learn_class_not_routed", LESSONS,
            "    let (lesson, topic) = routed(root, run, order, lesson);\n",
            "    let (lesson, topic) = (lesson, None::<String>);\n", T_ROUTED, "rust"),
    Control("learn_offer_exact_class_broken", LESSONS,
            "        None => entry.class == class,\n", "        None => entry.class == class && false,\n",
            T_ROUTED, "rust"),
    # The one-time fix-up: the index moves, through one ledger op that rolls back.
    Control("learn_reclass_index_unmoved", LESSONS,
            "                entry.class = to.clone();\n", "                entry.class = from.clone();\n",
            T_RECLASS, "rust"),
    Control("learn_reclass_takes_router_classes", LESSONS,
            "        if classes.contains(&entry.class) {\n", "        if !classes.is_empty() && false {\n",
            T_RECLASS, "rust"),
    # Retire: only a lesson offered DECAY_AFTER times that never helped.
    Control("learn_retire_any", LESSONS,
            "        if !retirable(&entry.counters) {\n", "        if false {\n", T_RETIRE, "rust"),
    # Idle: the owner's keys and the orders' writes.
    Control("learn_owner_typing_ignored", IDLE,
            "        Ok(Some(at)) if facts.now.saturating_sub(*at) < IDLE_AFTER_S => {\n",
            "        Ok(Some(at)) if facts.now.saturating_sub(*at) < 1 => {\n", T_IDLE, "rust"),
    Control("learn_owner_doubt_ignored", IDLE,
            "        Err(why) => return skip(format!(\"the owner's last key cannot be read: {why}\")),\n",
            "        Err(_) => {}\n", T_IDLE, "rust"),
    Control("learn_order_writes_ignored", IDLE,
            "        if facts.now.saturating_sub(*at) < IDLE_AFTER_S {\n",
            "        if facts.now.saturating_sub(*at) < 1 {\n", T_IDLE, "rust"),
    Control("learn_nothing_new_still_runs", IDLE,
            "        Some(n) if n > 0 => Decision::Run,\n", "        Some(_) => Decision::Run,\n", T_IDLE, "rust"),
    Control("learn_stream_writes_unread", IDLE,
            "            newest = newest.max(modified(&stream));\n", "            let _ = &stream;\n",
            T_FACTS, "rust"),
    # The spare gate: a missing or old reading is no review; half unused; resets within two hours.
    Control("learn_missing_spare_guessed", IDLE,
            "    let Some(share) = spare.spare else {\n", "    let Some(share) = spare.spare.or(Some(1.0)) else {\n",
            T_SPARE, "rust"),
    Control("learn_spare_floor_dropped", IDLE,
            "    if share < SPARE_MIN {\n", "    if share < 0.0 {\n", T_HALF, "rust"),
    Control("learn_reset_window_dropped", IDLE,
            "    if left > RESETS_WITHIN_S {\n", "    if left > u64::MAX - 1 {\n", T_HALF, "rust"),
    # The caps.
    Control("learn_day_cap_off_by_one", IDLE,
            "    if facts.used_today >= PER_SPACE_PER_DAY {\n", "    if facts.used_today > PER_SPACE_PER_DAY {\n",
            T_CAPS, "rust"),
    Control("learn_window_cap_off_by_one", IDLE,
            "    if facts.used_window >= PER_WINDOW {\n", "    if facts.used_window > PER_WINDOW {\n", T_CAPS, "rust"),
    Control("learn_lesson_cap_ignored", IDLE,
            "    if let Some(why) = &facts.review_cap {\n", "    if let Some(why) = facts.review_cap.as_ref().filter(|_| false) {\n",
            T_CAPS, "rust"),
    Control("learn_window_never_matches", IDLE,
            "        (Some((at, count)), Some(resets)) if at.abs_diff(resets) <= SAME_WINDOW_S => *count,\n",
            "        (Some((at, count)), Some(resets)) if at.abs_diff(resets) <= SAME_WINDOW_S && false => *count,\n",
            T_CAPS, "rust"),
    # Never while busy, and the switch.
    Control("learn_busy_ignored", IDLE,
            "    if let Some(why) = &facts.busy {\n", "    if let Some(why) = facts.busy.as_ref().filter(|_| false) {\n",
            T_BUSY, "rust"),
    Control("learn_lease_unread", IDLE,
            "        .find(|lease| lease.live(world.now))\n", "        .find(|lease| lease.live(world.now) && false)\n",
            T_TICK, "rust"),
    Control("learn_switch_ignored", IDLE,
            "    if !facts.on {\n", "    if false {\n", T_BUSY, "rust"),
    # End to end: the gate hands an idle review's ops to `apply`, the class is the order's, retire applies, the result
    # reaches the Learning square and the run ends.
    Control("learn_gate_skips_idle_apply", REVIEW,
            "        } else if subject == crate::nirvana_idle_review::SUBJECT {\n",
            "        } else if subject == \"@nothing\" {\n", T_TICK, "rust"),
    Control("learn_class_from_reviewer", IDLE,
            "                                class: class.clone(),\n", "                                class: lesson.class.clone(),\n",
            T_TICK, "rust"),
    Control("learn_retire_op_dropped", IDLE,
            "            IdleOp::Retire(slug) => nirvana_lessons::retire(root, &slug, by, run, at)\n",
            "            IdleOp::Retire(slug) => Err::<String, String>(slug.clone())\n", T_TICK, "rust"),
    Control("learn_result_not_recorded", REVIEW,
            "        crate::nirvana_idle_review::finished(project, run, &order, verdict.word(), &axes);\n",
            "        let _ = (&axes, &verdict);\n", T_TICK, "rust"),
    Control("learn_view_not_read", LEARN,
            "        idle: crate::nirvana_idle_review::view(root),\n",
            "        idle: crate::nirvana_idle_review::IdleView::default(),\n", T_TICK, "rust"),
    # ---- review round (learn-review.md) ----
    # S1: an unreadable leases.json, runs folder or record is a doubt, and a doubt is no review.
    Control("learn_s1_doubt_ignored", IDLE,
            "    if let Some(why) = &facts.doubt {\n", "    if let Some(why) = facts.doubt.as_ref().filter(|_| false) {\n",
            T_LEASES, "rust"),
    Control("learn_s1_lease_error_read_as_none", IDLE,
            "        Err(why) => Some(why),\n", "        Err(_) => None,\n", T_LEASES, "rust"),
    Control("learn_s1_runs_error_read_as_quiet", IDLE,
            "            Err(err) => Err(format!(\"{}: {err}\", dir.display())),\n", "            Err(_) => Ok(Vec::new()),\n",
            T_LEASES, "rust"),
    Control("learn_s1_activity_doubt_dropped", IDLE,
            "            Err(why) => doubt = doubt.or(Some(why)),\n", "            Err(_) => {}\n", T_LEASES, "rust"),
    Control("learn_s1_unreadable_record_is_empty", IDLE,
            "        Err(err) => return Err(format!(\"{}: {err}\", record.display())),\n", "        Err(_) => String::new(),\n",
            T_LEASES, "rust"),
    # S2: the week must be read fresh and have room.
    Control("learn_s2_week_floor_dropped", IDLE,
            "        if *share < WEEK_SPARE_MIN {\n", "        if *share < 0.0 {\n", T_WEEK, "rust"),
    Control("learn_s2_missing_week_guessed", IDLE,
            "        let Some(share) = share else {\n", "        let Some(share) = share.as_ref().or(Some(&1.0)) else {\n",
            T_WEEK, "rust"),
    Control("learn_s2_no_week_runs", IDLE,
            "    if spare.week.is_empty() {\n", "    if spare.week.is_empty() && false {\n", T_WEEK, "rust"),
    Control("learn_s2_week_not_read", IDLE,
            "            .filter(|window| window.window.split('.').next() == Some(\"week\"))\n",
            "            .filter(|window| window.window.split('.').next() == Some(\"none\"))\n", T_WEEK, "rust"),
    # S3: a failed queue is counted, backs off, gives back the lesson-review count and keeps its orders for later.
    Control("learn_s3_no_backoff", IDLE,
            "                                state.backoff_until = Some(now + BACKOFF_S);\n",
            "                                let _ = BACKOFF_S;\n", T_BACKOFF, "rust"),
    Control("learn_s3_backoff_ignored", IDLE,
            "    if let Some(until) = facts.backoff_until.filter(|until| *until > facts.now) {\n",
            "    if let Some(until) = facts.backoff_until.filter(|_| false) {\n", T_BACKOFF, "rust"),
    Control("learn_s3_through_kept_on_failure", IDLE,
            "                                    last.through = since;\n", "                                    let _ = since;\n",
            T_BACKOFF, "rust"),
    Control("learn_s3_review_count_kept", REVIEW,
            "    if let Err(err) = nirvana_gate::book(table, &pick, id, run) {\n        unqueue();\n",
            "    if let Err(err) = nirvana_gate::book(table, &pick, id, run) {\n", T_BACKOFF, "rust"),
    # S4: the reservation before the queue, and a reservation that never reached the journal settled.
    Control("learn_s4_count_not_reserved", IDLE,
            "                    state.count += 1;\n", "                    state.count += 0;\n", T_CRASH, "rust"),
    Control("learn_s4_through_not_reserved", IDLE,
            "                        through,\n", "                        through: since,\n", T_CRASH, "rust"),
    Control("learn_s4_window_not_reserved", IDLE,
            "                        count_window(global, &spare.pool, spare.resets_at.unwrap_or(now))\n",
            "                        { let _ = (global, spare); }\n", T_CRASH, "rust"),
    Control("learn_s4_stranded_never_settled", IDLE,
            "        if !stale {\n", "        if true {\n", T_CRASH, "rust"),
    # S5: use of a pool across the idle span, from the history; no history is no review.
    Control("learn_s5_rise_ignored", IDLE,
            "    if own > USAGE_RISE {\n", "    if own > 1.0 {\n", T_MOVED, "rust"),
    Control("learn_s5_other_pools_ignored", IDLE,
            "        if let Some(up) = rise(other).filter(|up| *up > USAGE_RISE) {\n",
            "        if let Some(up) = rise(other).filter(|_| false) {\n", T_MOVED, "rust"),
    Control("learn_s5_old_window_baseline", IDLE,
            "                (Some(a), Some(b)) => a.abs_diff(b) <= SAME_WINDOW_S,\n", "                (Some(_), Some(_)) => true,\n",
            T_MOVED, "rust"),
    Control("learn_s5_recent_baseline", IDLE,
            "            .filter(|sample| sample.read_at <= before)\n",
            "            .filter(|sample| sample.read_at <= world.now)\n", T_MOVED, "rust"),
    Control("learn_s5_history_missing_runs", IDLE,
            "        Err(why) => return skip(format!(\"no usage history to rule out other use: {why}\")),\n",
            "        Err(_) => {}\n", T_MOVED, "rust"),
    Control("learn_s5_moved_unwired", IDLE,
            "            usage_moved: moved.clone(),\n", "            usage_moved: Ok(None),\n", T_TICKFACTS, "rust"),
    # S6: the automatic reclass moves only what herdr wrote last, and keeps a file's own line endings; N6.
    Control("learn_s6_edited_moved", LESSONS,
            "            Some(_) if !edited && !herdr_wrote_last(root, slug) => out.left.push((\n",
            "            Some(_) if false => out.left.push((\n", T_EDITED, "rust"),
    Control("learn_s6_any_log_line_trusted", LESSONS,
            "    if logged.as_deref() == Some(sha.as_str()) {\n", "    if logged.is_some() {\n", T_EDITED, "rust"),
    Control("learn_s6_endings_lost", LESSONS,
            "        out.push_str(&format!(\"class: {to}{ending}\"));\n",
            "        out.push_str(&format!(\"class: {to}\\n\"));\n", T_EDITED, "rust"),
    Control("learn_n6_counters_reset", LESSONS,
            "                    entry.counters = now.counters.clone();\n", "                    let _ = &now.counters;\n",
            T_RECLASS, "rust"),
    # N5: a failed launch is a failure; a seated review that never started is settled.
    Control("learn_n5_launch_failure_ok", REVIEW,
            "    if let (Err(err), true) = (&launched, subject == crate::nirvana_idle_review::SUBJECT) {\n",
            "    if let (Err(err), true) = (&launched, false) {\n", T_LAUNCH, "rust"),
    Control("learn_n5_lease_kept", REVIEW,
            "        crate::nirvana_worker::release_lease(id);\n        unqueue();\n        return Err(format!(\"launch failed: {err}\"));\n",
            "        unqueue();\n        return Err(format!(\"launch failed: {err}\"));\n", T_LAUNCH, "rust"),
    Control("learn_n5_never_started_kept", IDLE,
            "        && (matches!(known.phase, Phase::Failed | Phase::Interrupted) || never_started)\n",
            "        && matches!(known.phase, Phase::Failed | Phase::Interrupted)\n", T_NEVER, "rust"),
    # N8: a bare order id two runs share names neither.
    Control("learn_n8_bare_id_ambiguous", IDLE,
            "                if matching.len() > 1 {\n", "                if matching.len() > 99 {\n", T_TICK, "rust"),
    # N10: the tick's own wiring of the owner's key, the orders' writes and its lock; N3's project list and window.
    Control("learn_n10_owner_key_unwired", IDLE,
            "            owner_key: owner.clone(),\n", "            owner_key: Ok(None),\n", T_TICKFACTS, "rust"),
    Control("learn_n10_activity_unwired", IDLE,
            "            activity: activity.clone(),\n", "            activity: None,\n", T_TICKFACTS, "rust"),
    Control("learn_n10_tick_lock_ignored", IDLE,
            "    if file.try_lock().is_err() {\n", "    if false {\n", T_TICKFACTS, "rust"),
    Control("learn_n3_fixture_roots_kept", IDLE,
            "            || (!temp.is_empty() && key.starts_with(&temp))\n", "            || false\n", T_KNOWN, "rust"),
    Control("learn_n3_gone_root_kept", IDLE,
            "            || !root.join(nirvana_run::DIR).is_dir()\n", "            || false\n", T_KNOWN, "rust"),
    Control("learn_n3_spellings_twice", IDLE,
            "            || !seen.insert(key)\n", "            || { seen.insert(key); false }\n", T_KNOWN, "rust"),
    Control("learn_n3_window_not_per_pool", IDLE,
            "    match (global.windows.get(pool), resets_at) {\n", "    match (global.windows.values().next(), resets_at) {\n",
            T_CAPS, "rust"),
)
