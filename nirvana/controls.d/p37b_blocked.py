# Plan 37b: herdr gates blocked deliveries too, and asks with the block first (package `p37b_blocked`,
# nirvana/plans/2026-10-31-37-records/p37b.md). The owner, 2026-11-01: "send it blocked as well for validationg".
# Controls starting with p37b_. One sabotage per assertion, each on a product line or a rule's text; the test named
# must print FAIL with it in place (written before the run). `Control` is provided by controls.py. No control reaches
# the network.
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
ROLE = "nirvana/claude/ROLE.md"
SEAT_SKILL = "nirvana/claude/skills/seat/SKILL.md"
GATE_SKILL = "nirvana/claude/skills/gate/SKILL.md"
RETRO = "src/nirvana_retro.rs"

T_GATES = "p37b_herdr_gates_a_blocked_delivery_but_never_a_helpers"
T_ASK = "p37b_a_blocked_delivery_is_asked_with_its_block_and_queued_once"
T_INVENTED = "p37b_a_blocked_delivery_with_invented_sources_is_asked_not_stepped_up"
T_EXEC = "p37b_exec_gates_a_blocked_delivery_and_its_wake_keeps_the_block"
T_BLOCK = "p37b_the_block_is_the_summary_and_its_question"
T_DOCS = "p37b_the_docs_say_a_blocked_delivery_is_gated_and_asked"
# Review round.
T_OUTCOME = "the_outcome_words_and_what_the_tally_counts"
T_CARDS = "p37b_a_blocked_gate_costs_the_seat_the_cards_and_the_lessons_nothing"
T_RETRO = "p37b_a_blocked_ask_is_a_block_not_the_send_back_cap"
T_ONE = "p37b_one_blocked_delivery_wakes_with_one_entry"
T_NO_TEST = "p37b_a_blocked_gate_notes_the_test_command_and_does_not_run_it"

ARM = '        || (matches!(status, "done" | "blocked")\n'
ASKS = "    if let Some(block) = &block {\n"
BLOCK_REASON = '        .map(|block| format!("blocked: {block}"))\n'

CONTROLS = (
    # Which deliveries `seat exec` gates: a blocked one, never a helper's, never a needs_answer one.
    Control("p37b_blocked_ungated", GATE, ARM, ARM.replace(' | "blocked"', ""), T_GATES, "rust"),
    Control("p37b_needs_answer_gated", GATE, ARM, ARM.replace('"blocked"', '"blocked" | "needs_answer"'),
            T_GATES, "rust"),
    Control("p37b_helpers_gated", GATE,
            "            && helps_of(project, run, &replay(&project.journal(run).read().0), order).is_none())",
            "            && (status == \"blocked\"\n"
            "                || helps_of(project, run, &replay(&project.journal(run).read().0), order).is_none()))",
            T_GATES, "rust"),
    # The same line seen from `seat exec`: a blocked delivery is gated before the wake.
    Control("p37b_exec_blocked_ungated", GATE, ARM, ARM.replace(' | "blocked"', ""), T_EXEC, "rust"),
    # A blocked delivery is asked, never sent back, and never a pass.
    Control("p37b_blocked_sent_back", GATE, ASKS, "    if let Some(block) = block.as_ref().filter(|_| false) {\n",
            T_ASK, "rust"),
    Control("p37b_blocked_passes", GATE,
            '        Some("done") => Axis::new("report", Mark::Pass, "well formed, status done"),',
            '        Some("done" | "blocked") => Axis::new("report", Mark::Pass, "well formed, status done"),',
            T_ASK, "rust"),
    Control("p37b_hand_gate_of_asked_errs", GATE,
            "Some(verdict) if verdict == Outcome::SentBack.word() || verdict == Outcome::Asked.word()",
            "Some(verdict) if verdict == Outcome::SentBack.word()", T_ASK, "rust"),
    # The block wins over invented sources: no step-up, no redo.
    Control("p37b_invented_steps_up", GATE, ASKS,
            "    if let Some(block) = block.as_ref().filter(|_| invented_videos(&axes).is_empty()) {\n",
            T_INVENTED, "rust"),
    # The block is kept: the gated line's reason, the question's opening, the report's own question.
    # Review N3: as `blocked: <block>`, not the report axis's 37-character prefix nor the first failing axis.
    Control("p37b_reason_first_axis", GATE, BLOCK_REASON, "        .and(None::<String>)\n", T_INVENTED, "rust"),
    Control("p37b_question_no_block", GATE, '"{order} is blocked: {block}. {gate}.', '"{order} is blocked. {gate}.',
            T_ASK, "rust"),
    Control("p37b_question_no_finding", GATE, '        .filter(|axis| axis.axis != "report")\n',
            "        .filter(|_| false)\n", T_INVENTED, "rust"),
    Control("p37b_question_no_count", GATE, '            "the gate also finds {} ({n} cited video(s) do not exist)",',
            '            "the gate also finds {} ({n})",', T_INVENTED, "rust"),
    Control("p37b_block_drops_question", GATE, '        Some(question) => format!("{summary} ({question})"),',
            "        Some(_) => summary.to_owned(),", T_BLOCK, "rust"),
    Control("p37b_block_any_status", GATE,
            '    if report.get("status").and_then(Value::as_str) != Some("blocked") {\n',
            "    if false {\n", T_BLOCK, "rust"),
    # A later verdict on a blocked order queues nothing more (plan 37 review S2's rule, seen for a blocked one).
    Control("p37b_regate_queues", RUN, "                verdict_owed.retain(|owed| owed != order);\n", "",
            T_EXEC, "rust"),
    # The docs: ROLE.md and the seat skill say a blocked delivery is gated and asked, and the gate skill says it is
    # never sent back.
    Control("p37b_role_gated", ROLE, "5. herdr gates each `done` or `blocked` delivery itself",
            "5. herdr gates each `done` delivery itself", T_DOCS, "rust"),
    Control("p37b_role_asked", ROLE, "then asked, never sent", "then sent", T_DOCS, "rust"),
    Control("p37b_role_left_by_hand", ROLE, "(a helper's, a gate waiting on a review)",
            "(a helper's, a `blocked` one, a gate waiting on a review)", T_DOCS, "rust"),
    Control("p37b_seat_gated", SEAT_SKILL, "herdr gates each `done` or `blocked` delivery itself",
            "herdr gates each `done` delivery itself", T_DOCS, "rust"),
    Control("p37b_seat_asked", SEAT_SKILL, "then asked, never sent back", "then sent back", T_DOCS, "rust"),
    Control("p37b_gate_skill_never_sent_back", GATE_SKILL, "A `blocked` delivery is never sent back",
            "A `blocked` delivery is sent back", T_DOCS, "rust"),
    # Review S1: a blocked-asked order costs the seat nothing (the record's word, the tally) and counts no lesson or
    # card the worker opened as failing after it.
    Control("p37b_blocked_tallied", GATE, "        if word == Self::BLOCKED {\n", "        if false {\n",
            T_OUTCOME, "rust"),
    Control("p37b_blocked_record_asked", RUN, "        if verdict == Outcome::Asked && blocked {\n",
            "        if false {\n", T_CARDS, "rust"),
    Control("p37b_blocked_counts_lessons", GATE,
            "    if !blocked {\n        let _ = crate::nirvana_lessons::note_gated(",
            "    if true {\n        let _ = crate::nirvana_lessons::note_gated(", T_CARDS, "rust"),
    Control("p37b_blocked_counts_cards", GATE, "card_profile.as_ref().filter(|_| !blocked)", "card_profile.as_ref()",
            T_CARDS, "rust"),
    # Review S2: the retro calls it a block, untagged, never the send-back cap spent.
    Control("p37b_retro_block_is_cap", RETRO, '                if verdict == "asked" && blocked {\n',
            "                if false {\n", T_RETRO, "rust"),
    Control("p37b_retro_block_tagged", RETRO,
            '                        "blocked",\n                        format!("a {class} worker was blocked: {block}"),\n'
            "                        false,\n",
            '                        "blocked",\n                        format!("a {class} worker was blocked: {block}"),\n'
            "                        true,\n", T_RETRO, "rust"),
    # Review S4: one wake entry, the question; a stopped gate's delivered line kept; a done order's cap ask as it was.
    Control("p37b_blocked_gated_line_queued", RUN, "                    && !blocked_ask\n", "", T_ONE, "rust"),
    Control("p37b_blocked_delivered_line_kept", RUN,
            "                state.queue.retain(|item| !delivered.contains(&item.seq));\n", "", T_ONE, "rust"),
    Control("p37b_any_ask_collapses", RUN,
            "                    && verdict == crate::nirvana_gate::Outcome::Asked.word()\n"
            "                    && blocked_owed.iter().any(|(owed, ..)| owed == order);\n",
            "                    && verdict == crate::nirvana_gate::Outcome::Asked.word();\n", T_ONE, "rust"),
    Control("p37b_exec_three_lines", RUN, "                    && !blocked_ask\n", "", T_EXEC, "rust"),
    # Review S3: ROLE.md and the gate skill say a blocked-asked order is the orchestrator's to resolve first.
    Control("p37b_role_resolve_first", ROLE, "A blocked-asked order is yours to resolve first",
            "A blocked-asked order is the owner's", T_DOCS, "rust"),
    Control("p37b_gate_skill_resolve_first", GATE_SKILL, "a blocked order's question is yours to resolve first",
            "a blocked order's question is the owner's", T_DOCS, "rust"),
    # Review N1: ROLE.md's item 5 keeps 120 columns.
    Control("p37b_role_unwrapped", ROLE, "you gate with\n   `herdr gate run`", "you gate with `herdr gate run`",
            T_DOCS, "rust"),
    # Review N2: an empty block never reads "blocked: .".
    Control("p37b_empty_block", GATE, '        None if summary.is_empty() => "no reason given".to_owned(),\n', "",
            T_BLOCK, "rust"),
    Control("p37b_empty_summary_question", GATE,
            "        Some(question) if summary.is_empty() => question.to_owned(),\n", "", T_BLOCK, "rust"),
    # Review N4: a blocked report's gate does not run the `test:` command, and its tests axis says so.
    Control("p37b_blocked_runs_test", GATE, "    let test = subject.spec.test.as_deref().filter(|_| !blocked);\n",
            "    let test = subject.spec.test.as_deref();\n", T_NO_TEST, "rust"),
    Control("p37b_blocked_tests_axis", GATE, "        Some(test) if blocked => Axis::new(\n",
            "        Some(test) if false => Axis::new(\n", T_NO_TEST, "rust"),
)
