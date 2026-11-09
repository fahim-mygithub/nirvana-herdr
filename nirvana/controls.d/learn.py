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
            "    if facts.new_orders == 0 {\n", "    if facts.new_orders == usize::MAX {\n", T_IDLE, "rust"),
    Control("learn_stream_writes_unread", IDLE,
            "            newest = newest.max(modified(&stream.path()));\n", "            let _ = &stream;\n",
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
            "        (Some((own, at)), Some(resets)) if own == pool && at.abs_diff(resets) <= SAME_WINDOW_S => {\n",
            "        (Some((own, at)), Some(resets)) if own == pool && at.abs_diff(resets) <= SAME_WINDOW_S && false => {\n",
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
)
