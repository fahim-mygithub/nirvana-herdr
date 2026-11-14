# Plan 73, Hermes's idle queue and lesson lifecycle (nirvana/plans/2026-11-14-lesson-lifecycle.md). The owner,
# 2026-11-14: "Sure lets add the top 3". Controls starting with p73_. Each breaks one assertion's code and expects its
# test to print FAIL.

LIFE = 'src/nirvana_lessons/life.rs'
LESSONS = 'src/nirvana_lessons.rs'
IDLE = 'src/nirvana_idle_review.rs'

T_LIFE = 'p73_a_lesson_unused_14_days_is_stale_and_archived_at_30_never_deleted'
T_PIN = 'p73_a_pinned_lesson_is_never_stale_archived_or_retired'
T_USE = 'p73_an_offer_is_a_use_and_an_undated_lesson_is_dated_not_archived'
T_WRITE = 'p73_a_workers_write_is_a_ledger_op_that_undo_takes_back'
T_FORCE = 'p73_orders_that_waited_the_max_age_need_only_the_short_quiet'
T_TICK = 'p73_a_tick_forces_a_review_whose_orders_waited_and_sweeps_lessons'

CONTROLS = (
    # Unused 14 days is stale
    Control('p73_stale_at_14', LIFE,
            '    } else if unused >= STALE_AFTER_S {\n', '    } else if false {\n', T_LIFE, 'rust'),
    # Unused 30 days is archived
    Control('p73_archive_at_30', LIFE,
            '    if unused >= ARCHIVE_AFTER_S {\n', '    if false {\n', T_LIFE, 'rust'),
    # Archived is never deleted: the file is copied to .archive/ first
    Control('p73_archive_keeps_copy', LESSONS,
            '    std::fs::copy(&from, &copy).map_err(|err| format!("archive {slug}: {err}"))?;\n',
            '    let _ = &copy;\n', T_LIFE, 'rust'),
    # An undone archive is a use, so the next sweep keeps it
    Control('p73_undo_archive_is_use', LESSONS,
            'if matches!(row.op.as_str(), "archive" | "retire") {', 'if false {', T_LIFE, 'rust'),
    # The archive listing leaves out an op that was undone
    Control('p73_archived_skips_undone', LIFE,
            '            || rows\n                .iter()\n                .any(|r| r.of.as_deref() == Some(row.id.as_str()))\n',
            '            || false\n', T_LIFE, 'rust'),
    # A pinned lesson is never stale or archived
    Control('p73_pinned_exempt', LIFE,
            '    if entry.pinned {\n        return Life::Pinned;\n', '    if false {\n        return Life::Pinned;\n',
            T_PIN, 'rust'),
    # A pinned lesson is never retired
    Control('p73_pinned_not_retired', LESSONS,
            '        if entry.pinned {\n            return Err(format!("{slug} is pinned',
            '        if false {\n            return Err(format!("{slug} is pinned', T_PIN, 'rust'),
    # Undoing a pin keeps the offers counted since
    Control('p73_pin_undo_keeps_counters', LESSONS,
            'Some(entry) if matches!(row.op.as_str(), "reclass" | "pin" | "unpin") => {',
            'Some(entry) if row.op == "reclass" => {', T_PIN, 'rust'),
    # An unpin starts the clock again
    Control('p73_unpin_restarts_clock', LIFE,
            '        if !on {\n            entry.used_at = Some(at);\n', '        if false {\n            entry.used_at = Some(at);\n',
            T_PIN, 'rust'),
    # An offer into a brief is a use
    Control('p73_offer_is_use', LESSONS,
            '            entry.used_at = Some(entry.used_at.map_or(at, |was| was.max(at)));\n', '', T_USE, 'rust'),
    # A lesson with no use on record is dated by the sweep, not archived
    Control('p73_undated_dated', LIFE,
            '                    entry.used_at = Some(now);\n', '', T_USE, 'rust'),
    # The gate's worker write is a ledger op of its own
    Control('p73_write_ledgered', LESSONS,
            'let written = ledgered(root, "write", order, run, at, &slugs, |index, _| {',
            'let written = ledgered(root, "new", order, run, at, &slugs, |index, _| {', T_WRITE, 'rust'),
    # A write is a use
    Control('p73_write_is_use', LESSONS,
            'used_at: Some(used_at.map_or(source.at, |was| was.max(source.at))),', 'used_at,', T_WRITE, 'rust'),
    # Undo takes a lesson's slug: the newest op on it
    Control('p73_undo_by_slug', LIFE,
            '                .chain(&row.after)\n                .any(|s| s.slug == target)',
            '                .any(|s| s.slug == target && s.file.is_none())', T_WRITE, 'rust'),
    # Orders that waited MAX_WAIT_S are forced
    Control('p73_force_after_max_age', IDLE,
            '.is_some_and(|at| facts.now.saturating_sub(at) >= MAX_WAIT_S)', '.is_some_and(|_| false)', T_FORCE, 'rust'),
    # A forced review still needs FORCE_QUIET_S of quiet
    Control('p73_force_keeps_short_quiet', IDLE,
            '    let quiet = if waited_out(facts) {\n        FORCE_QUIET_S\n',
            '    let quiet = if waited_out(facts) {\n        0\n', T_FORCE, 'rust'),
    # A forced review never skips the lease rule, the spare, the caps or the use of the pool
    Control('p73_force_keeps_every_other_rule', IDLE,
            '    if let Err(why) = ready_within(facts, quiet) {\n',
            '    if let Err(why) = ready_within(facts, quiet).or_else(|why| if waited_out(facts) { Ok(()) } else { Err(why) }) {\n',
            T_FORCE, 'rust'),
    # The tick counts the orders when the short quiet allows a forced review
    Control('p73_tick_counts_on_short_quiet', IDLE,
            'let gated = if full || ready_within(&facts, FORCE_QUIET_S).is_ok() {', 'let gated = if full {',
            T_TICK, 'rust'),
    # The tick reads when the oldest waiting order was gated
    Control('p73_tick_pending_since', IDLE,
            'facts.pending_since = gated.iter().map(|r| r.at).min();', 'facts.pending_since = None;', T_TICK, 'rust'),
    # The tick runs the lessons' sweep
    Control('p73_tick_sweeps', IDLE,
            'let swept = nirvana_lessons::life::sweep(root, now, false);',
            'let swept = nirvana_lessons::life::Swept::default();', T_TICK, 'rust'),
)
