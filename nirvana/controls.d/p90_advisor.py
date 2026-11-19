# Plan 90: Claude Code's advisor for Claude workers (the owner, 2026-11-18: "Go ahead and build in the consulting it
# might be cheaper in the long run for balanced runs") and the worker compaction ceiling (plan 43 B1, ruling R1 "1.
# yes"). One control per test function. Controls starting with p90_.

CONTROLS = (
    # The advisor is read for the mode asked, not another
    Control(
        'p90_advisor_by_mode',
        'src/nirvana_seat.rs',
        'Mode::Balanced => self.balanced.as_deref(),',
        'Mode::Balanced => self.efficient.as_deref(),',
        'p90_the_default_rows_give_sonnet_an_opus_advisor_in_balanced_only',
        'rust',
    ),
    # A pair outside the measured stronger ones is refused at load
    Control(
        'p90_advisor_pairs',
        'src/nirvana_seat.rs',
        '.any(|(main, adv)| pair == (Some(*main), Some(*adv)));',
        '.any(|_| pair.0.is_some());',
        'p90_an_advisor_must_be_a_stronger_claude_seat_on_a_claude_seat',
        'rust',
    ),
    # A Fable advisor is asked the Fable seat's money question, on the advisor's own seat
    Control(
        'p90_fable_advisor_money',
        'src/nirvana_seat.rs',
        'may_spend_now(table, world, &advisor.id, effort, class_id, size, order)',
        'may_spend_now(table, world, seat_id, effort, class_id, size, order)',
        'p90_a_fable_advisor_passes_the_fable_seats_money_check',
        'rust',
    ),
    # The advisor's share is added to the record's use
    Control(
        'p90_record_counts_advisor',
        'src/nirvana_seat.rs',
        '*spent.entry(window).or_insert(0.0) += share;',
        '*spent.entry(window).or_insert(0.0) += 0.0 * share;',
        'p90_the_record_counts_the_advisors_tokens_at_its_own_price',
        'rust',
    ),
    # The worker ceiling's default is 300,000
    Control(
        'p90_ceiling_default',
        'src/nirvana_seat.rs',
        'fn default_worker_autocompact_tokens() -> u64 {\n    300_000\n}',
        'fn default_worker_autocompact_tokens() -> u64 {\n    200_000\n}',
        'p90_the_worker_ceiling_defaults_to_300k_and_is_checked',
        'rust',
    ),
    # The advisor's line goes on the prompt with the flag
    Control(
        'p90_advisor_line',
        'src/nirvana_worker.rs',
        'prompt.push_str(ADVISOR_LINE);',
        'prompt.push_str("");',
        'p90_a_claude_worker_gets_the_advisor_flag_and_line_and_agy_never',
        'rust',
    ),
    # SOTA gets no ceiling; Efficient and Balanced do
    Control(
        'p90_ceiling_not_sota',
        'src/nirvana_worker.rs',
        '&& tokens > 0 && mode != Mode::Sota {',
        '&& tokens > 0 && mode != Mode::Efficient {',
        'p90_the_worker_ceiling_is_passed_outside_sota_only',
        'rust',
    ),
    # The advisor's tokens come from its model's modelUsage entry
    Control(
        'p90_advisor_model_usage',
        'src/nirvana_worker.rs',
        'usage.get(&launch.model).or_else(|| {',
        'usage.get("no-such-model").and_then(|_| {',
        'p90_a_real_stream_gives_its_consults_and_the_advisors_tokens',
        'rust',
    ),
    # A stderr line carrying any one of Claude Code's drop messages is a drop
    Control(
        'p90_dropped_any_mark',
        'src/nirvana_worker.rs',
        '.find(|line| ADVISOR_DROPPED.iter().any(|mark| line.contains(mark)))',
        '.find(|line| ADVISOR_DROPPED.iter().all(|mark| line.contains(mark)))',
        'p90_a_dropped_advisor_is_named_from_stderr_or_the_environment',
        'rust',
    ),
    # A compact_boundary event is read as a compaction
    Control(
        'p90_compact_boundary',
        'src/nirvana_worker.rs',
        '("system", "compact_boundary") => {',
        '("system", "compact_boundary_never") => {',
        'p90_a_real_stream_gives_its_compactions',
        'rust',
    ),
    # The journal's compaction row counts on the order
    Control(
        'p90_journal_compactions',
        'src/nirvana_run.rs',
        'known.compactions += 1;',
        'known.compactions += 0;',
        'p90_the_journal_carries_the_advisor_and_compaction_rows',
        'rust',
    ),
    # exec puts the seat's advisor on the argv it starts
    Control(
        'p90_exec_adds_advisor',
        'src/nirvana_worker.rs',
        'Some(launch) => with_advisor(argv, &launch.model),',
        'Some(_) => argv,',
        'p90_exec_launches_the_seats_advisor_and_ceiling_by_mode',
        'rust',
    ),
)
