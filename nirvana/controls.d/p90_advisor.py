# The advisor: Claude Code's advisor for Claude workers (the owner, 2026-11-18: "Go ahead and build in the consulting it
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
        'Some(launch) => with_advisor(argv, &launch.model, fresh),',
        'Some(_) => argv,',
        'p90_exec_launches_the_seats_advisor_and_ceiling_by_mode',
        'rust',
    ),
    # The ceiling reaches Anthropic models only, not qwen through the claude CLI
    Control(
        'p90_ceiling_family',
        'src/nirvana_worker.rs',
        'Some(seat) if seat.cli == "claude" && seat.family == "anthropic" => {',
        'Some(seat) if seat.cli == "claude" => {',
        'p90_the_ceiling_reaches_anthropic_models_above_it_only',
        'rust',
    ),
    # A window at the ceiling gets none, as one below it
    Control(
        'p90_ceiling_window',
        'src/nirvana_worker.rs',
        'if seat.context_tokens.is_some_and(|window| window <= ceiling) {',
        'if seat.context_tokens.is_some_and(|window| window < ceiling) {',
        'p90_the_ceiling_reaches_anthropic_models_above_it_only',
        'rust',
    ),
    # The help is read for the flag as a word of its own
    Control(
        'p90_help_offers_word',
        'src/nirvana_worker.rs',
        '.any(|word| word == flag)',
        '.any(|word| word.starts_with(flag))',
        'p90_the_help_and_the_refusal_are_read',
        'rust',
    ),
    # The refusal names --advisor and an unknown option, both
    Control(
        'p90_flag_rejected_mark',
        'src/nirvana_worker.rs',
        'line.contains("--advisor")\n                && (line.contains("unknown option")',
        'line.contains("--advisor")\n                || (line.contains("unknown option")',
        'p90_the_help_and_the_refusal_are_read',
        'rust',
    ),
    # A claude whose help lacks --autocompact gets no ceiling
    Control(
        'p90_help_drops_ceiling',
        'src/nirvana_worker.rs',
        '                compact = 0;\n                let why = format!("{why}; the worker starts without the ceiling");',
        '                let why = format!("{why}; the worker starts without the ceiling");',
        'p90_exec_drops_the_ceiling_a_claude_without_the_flag_would_refuse',
        'rust',
    ),
    # A refusal before the first stream line starts the attempt again
    Control(
        'p90_advisor_relaunch',
        'src/nirvana_worker.rs',
        '(Some(_), Ok((AttemptEnd::Exited { stderr, .. }, stream))) if stream.lines == 0 => {',
        '(Some(_), Ok((AttemptEnd::Exited { stderr, .. }, stream))) if stream.lines == 1_000_000 => {',
        'p90_exec_relaunches_once_without_an_advisor_the_claude_refuses',
        'rust',
    ),
    # The relaunch drops the flag and its line
    Control(
        'p90_relaunch_without_flag',
        'src/nirvana_worker.rs',
        '                &without_advisor(&argv),',
        '                &argv,',
        'p90_exec_relaunches_once_without_an_advisor_the_claude_refuses',
        'rust',
    ),
    # A resumed attempt gets no second advisor line
    Control(
        'p90_resume_no_line',
        'src/nirvana_worker.rs',
        'if fresh && argv.get(1).map(String::as_str) == Some("-p") {',
        'if argv.get(1).map(String::as_str) == Some("-p") {',
        'p90_a_resumed_attempt_gets_the_flag_without_the_line',
        'rust',
    ),
    # A stream with no modelUsage gives no cost, not $0
    Control(
        'p90_usd_unknown',
        'src/nirvana_worker.rs',
        '            .and(launch.price.as_ref())',
        '            .map_or(launch.price.as_ref(), |_| launch.price.as_ref())',
        'p90_advisor_rows_say_what_herdr_cannot_tell',
        'rust',
    ),
    # No consult and no advisor entry is unseen, not "advisor on"
    Control(
        'p90_unseen',
        'src/nirvana_worker.rs',
        '(consults == 0 && entry.is_none() && dropped.is_none())',
        '(consults == 1_000_000 && entry.is_none() && dropped.is_none())',
        'p90_advisor_rows_say_what_herdr_cannot_tell',
        'rust',
    ),
    # One attempt that consulted makes the order's row seen
    Control(
        'p90_sum_seen',
        'src/nirvana_seat.rs',
        'if let Some(sum) = total.as_mut().filter(|sum| sum.consults > 0) {',
        'if let Some(sum) = total.as_mut().filter(|sum| sum.consults > 1_000_000) {',
        'p90_advisor_rows_say_what_herdr_cannot_tell',
        'rust',
    ),
    # Advisor tokens with no main tokens still count
    Control(
        'p90_record_advisor_alone',
        'src/nirvana_seat.rs',
        '            let spent = line.spent.get_or_insert_with(BTreeMap::new);\n            for (window, share) in shares {',
        '            let Some(spent) = line.spent.as_mut() else {\n                return Ok(line);\n            };\n            for (window, share) in shares {',
        'p90_advisor_tokens_count_once_with_or_without_the_orders_own',
        'rust',
    ),
    # The Seats square adds the advisor's share
    Control(
        'p90_square_advisor',
        'src/nirvana_seats_square.rs',
        '            .and_then(|advisor| nirvana_seat::advisor_use(table, &advisor, order.seated_at))',
        '            .and_then(|advisor| nirvana_seat::advisor_use(table, &advisor, order.seated_at))\n            .filter(|_| false)',
        'p90_an_orders_use_adds_its_advisors_share',
        'rust',
    ),
)
