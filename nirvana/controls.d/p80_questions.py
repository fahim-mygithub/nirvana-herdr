# Plan 42 P42a: the Questions record and `herdr ask` (nirvana/plans/2026-11-14-42-questions-square.md §5.1, §5.2).
# Controls starting with p80_. One sabotage per assertion, each on a product line in src/nirvana_questions.rs; the test
# named (the fifth field, written before the run) must print FAIL with it in place. `Control` is provided by
# controls.py.
Q = "src/nirvana_questions.rs"

_T = "nirvana_questions::tests::"
T_LATEST = _T + "the_latest_line_for_an_id_wins_in_its_first_place"
T_ADD = _T + "add_files_numbered_questions_with_options_and_blocks"
T_SAME = _T + "the_same_text_asked_again_raises_its_count_and_makes_no_second_item"
T_AGAIN = _T + "a_question_asked_again_after_its_answer_prints_the_answer"
T_BAD = _T + "bad_asks_are_refused_with_exit_2_and_nothing_written"
T_BY = _T + "the_cli_answer_is_the_orchestrators_and_no_flag_makes_it_the_owners"
T_OWNER = _T + "the_owner_answers_through_the_library_and_may_correct_the_orchestrator"
T_WITHDRAW = _T + "withdraw_takes_an_open_question_back_and_an_unknown_id_exits_1"
T_LIST = _T + "list_shows_open_items_and_all_with_all"
T_SUMMARY = _T + "the_summary_counts_open_items_and_the_oldest_age"
T_MIRROR = _T + "a_workers_asked_event_becomes_one_item_however_often_it_is_read"
T_LONG = _T + "a_long_worker_question_is_cut_to_one_line_and_kept_in_full"
T_SETTLE = _T + "a_worker_question_the_run_answers_is_settled_by_the_journal"
T_CHILD = _T + "a_worker_question_a_child_order_continues_is_settled_by_the_journal"
T_GOES_ON = _T + "a_worker_question_settles_when_its_order_goes_on"
T_WAITS = _T + "a_worker_question_stays_open_while_its_order_waits"

CONTROLS = (
    # The record: the latest line for an id wins.
    Control("p80_latest_line_wins", Q,
            "            Some(known) => *known = item,\n",
            "            Some(_) => {}\n",
            T_LATEST, "rust"),
    # A line that does not parse is counted, not silently lost.
    Control("p80_bad_lines_counted", Q,
            "            Err(_) => bad_lines += 1,\n",
            "            Err(_) => {}\n",
            T_LATEST, "rust"),
    # Ids number on: q1, q2.
    Control("p80_next_id", Q,
            '        format!("q{}", top_number(&self.items) + 1)\n',
            '        format!("q{}", top_number(&self.items))\n',
            T_ADD, "rust"),
    # The same text from the orchestrator finds the item that exists.
    Control("p80_same_text_found", Q,
            "            None => questions.same_text(ORCHESTRATOR, &ask.text),\n",
            "            None => None,\n",
            T_SAME, "rust"),
    # Case is set aside when texts are compared.
    Control("p80_normal_case", Q,
            "    let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();\n",
            "    let words: Vec<String> = text.split_whitespace().map(str::to_owned).collect();\n",
            T_SAME, "rust"),
    # --same names the item to bump.
    Control("p80_same_flag", Q,
            '            "--same" if verb == "add" => ask.same = Some(value(&mut iter, arg)?.clone()),\n',
            '            "--same" if verb == "add" => drop(value(&mut iter, arg)?),\n',
            T_SAME, "rust"),
    # Asking again raises the count.
    Control("p80_count_raised", Q,
            "            item.asked_count += 1;\n            item.last_asked_at = now;\n",
            "            item.last_asked_at = now;\n",
            T_SAME, "rust"),
    # Asking again adds the new blocks.
    Control("p80_blocks_union", Q,
            "            union(&mut item.blocks, &ask.blocks);\n",
            "",
            T_SAME, "rust"),
    # A question asked again after its answer prints the answer.
    Control("p80_answered_again_prints", Q,
            "                Added::Answered(item.clone())\n",
            "                Added::Again(item.clone())\n",
            T_AGAIN, "rust"),
    # A recommended value must be one of the options.
    Control("p80_recommended_checked", Q,
            "            .any(|option| option.trim() == recommended.trim())\n",
            "            .any(|_| true)\n",
            T_BAD, "rust"),
    # At most TEXT_MAX chars.
    Control("p80_text_max", Q,
            "    if chars > TEXT_MAX {\n",
            "    if chars > TEXT_MAX + 1 {\n",
            T_BAD, "rust"),
    # One line.
    Control("p80_one_line", Q,
            "    if text.contains(['\\n', '\\r']) {\n",
            "    if false {\n",
            T_BAD, "rust"),
    # An option given twice is refused.
    Control("p80_option_twice", Q,
            "            .any(|other| other.trim() == option.trim())\n",
            "            .any(|_| false)\n",
            T_BAD, "rust"),
    # Blocks are ids.
    Control("p80_blocks_checked", Q,
            "        if !nirvana_run::valid_id(block) {\n",
            "        if false {\n",
            T_BAD, "rust"),
    # Bad input exits 2.
    Control("p80_invalid_exit_2", Q,
            "            Self::Invalid(_) => 2,\n",
            "            Self::Invalid(_) => 1,\n",
            T_BAD, "rust"),
    # `herdr ask answer` records the orchestrator, never the owner.
    Control("p80_cli_answer_is_orchestrator", Q,
            "    answer_with(project, id, answer, By::Orchestrator, now)\n",
            "    answer_with(project, id, answer, By::Owner, now)\n",
            T_BY, "rust"),
    # An unknown flag (`--by owner`) is refused as a flag.
    Control("p80_unknown_flag_refused", Q,
            '            flag if flag.starts_with("--") => {\n',
            '            flag if false && flag.starts_with("--") => {\n',
            T_BY, "rust"),
    # Only an open item takes the orchestrator's answer.
    Control("p80_answer_only_open", Q,
            "        let may = known.state == State::Open\n",
            "        let may = true\n",
            T_BY, "rust"),
    # The owner may correct an answer that is not the owner's.
    Control("p80_owner_corrects", Q,
            "            || (by == By::Owner\n",
            "            || (false\n",
            T_OWNER, "rust"),
    # The owner's own answer stands: a second tap answers nothing.
    Control("p80_owner_answers_once", Q,
            "known.answered_by != Some(By::Owner)",
            "true",
            T_OWNER, "rust"),
    # Withdraw sets withdrawn.
    Control("p80_withdraw_state", Q,
            "        item.state = State::Withdrawn;\n",
            "        item.state = State::Open;\n",
            T_WITHDRAW, "rust"),
    # Only an open item is withdrawn.
    Control("p80_withdraw_only_open", Q,
            "        if known.state != State::Open {\n",
            "        if false {\n",
            T_WITHDRAW, "rust"),
    # A withdrawn question asked again is a new item.
    Control("p80_withdrawn_is_new", Q,
            "            item.state != State::Withdrawn && item.source == source && normal(&item.text) == key\n",
            "            item.source == source && normal(&item.text) == key\n",
            T_WITHDRAW, "rust"),
    # A missing or closed item exits 1.
    Control("p80_not_found_exit_1", Q,
            "            Self::NotFound(_) | Self::NotOpen { .. } | Self::Io(_) => 1,\n",
            "            Self::NotFound(_) | Self::NotOpen { .. } | Self::Io(_) => 3,\n",
            T_WITHDRAW, "rust"),
    # list shows open items only, unless --all.
    Control("p80_list_open_only", Q,
            "                .filter(|item| all || item.state == State::Open)\n",
            "                .filter(|_| true)\n",
            T_LIST, "rust"),
    # The summary counts open items only.
    Control("p80_summary_open_only", Q,
            "            .filter(|item| item.state == State::Open)\n",
            "            .filter(|_| true)\n",
            T_SUMMARY, "rust"),
    # The summary's age is the oldest's.
    Control("p80_summary_oldest", Q,
            "                .map(|item| now.saturating_sub(item.asked_at))\n                .max(),\n",
            "                .map(|item| now.saturating_sub(item.asked_at))\n                .min(),\n",
            T_SUMMARY, "rust"),
    # A mirrored event is found again by its run#seq key: no second ask on a re-read.
    Control("p80_mirror_key_dedupe", Q,
            "                if items.iter().any(|item| item.journal.contains(&key)) {\n",
            "                if false {\n",
            T_MIRROR, "rust"),
    # A mirrored item's source is the worker's order.
    Control("p80_mirror_source", Q,
            "                            source: order.clone(),\n",
            "                            source: ORCHESTRATOR.into(),\n",
            T_MIRROR, "rust"),
    # A mirrored item was asked when the event was.
    Control("p80_mirror_asked_at", Q,
            "                            asked_at: event.at,\n",
            "                            asked_at: now,\n",
            T_MIRROR, "rust"),
    # A long question is cut to one line and kept in full.
    Control("p80_mirror_cut", Q,
            "    if flat.chars().count() <= TEXT_MAX && !question.trim().contains('\\n') {\n",
            "    if true {\n",
            T_LONG, "rust"),
    # A review's question is not mirrored.
    Control("p80_mirror_skips_reviews", Q,
            "                    .is_some_and(|known| nirvana_run::reviews(&known.seated.class))\n",
            "                    .is_some_and(|_| false)\n",
            T_SETTLE, "rust"),
    # A later order that answers it settles the item.
    Control("p80_settle_by_answers", Q,
            "other.answers.as_deref() == Some(order) || ",
            "",
            T_SETTLE, "rust"),
    # The journal's settling is marked as the journal's.
    Control("p80_settle_by_journal", Q,
            "                    item.answered_by = Some(By::Journal);\n",
            "                    item.answered_by = Some(By::Owner);\n",
            T_SETTLE, "rust"),
    # A child order that continues it settles the item.
    Control("p80_settle_by_child", Q,
            " || other.seated.parent.as_deref() == Some(order)",
            "",
            T_CHILD, "rust"),
    # An order that went on settles its own question.
    Control("p80_settle_by_phase", Q,
            "    if known.phase != Phase::Asked {\n",
            "    if false {\n",
            T_GOES_ON, "rust"),
    # An order that still waits keeps its question open.
    Control("p80_waiting_stays_open", Q,
            "    Ok(())\n}\n\n/// Mirrors every worker",
            '    Err("x".into())\n}\n\n/// Mirrors every worker',
            T_WAITS, "rust"),
)
