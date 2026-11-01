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

T_GATES = "p37b_herdr_gates_a_blocked_delivery_but_never_a_helpers"
T_ASK = "p37b_a_blocked_delivery_is_asked_with_its_block_and_queued_once"
T_INVENTED = "p37b_a_blocked_delivery_with_invented_sources_is_asked_not_stepped_up"
T_EXEC = "p37b_exec_gates_a_blocked_delivery_and_its_wake_keeps_the_block"
T_BLOCK = "p37b_the_block_is_the_summary_and_its_question"
T_DOCS = "p37b_the_docs_say_a_blocked_delivery_is_gated_and_asked"

ARM = '        || (matches!(status, "done" | "blocked")\n'
ASKS = "    if let Some(block) = &block {\n"
REPORT_FIRST = '                .find(|axis| axis.axis == "report")\n'

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
    Control("p37b_reason_first_axis", GATE, REPORT_FIRST, REPORT_FIRST.replace('"report"', '"never"'),
            T_INVENTED, "rust"),
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
    Control("p37b_role_left_by_hand", ROLE, "(a\n   helper's, a gate waiting on a review)",
            "(a\n   helper's, a `blocked` one, a gate waiting on a review)", T_DOCS, "rust"),
    Control("p37b_seat_gated", SEAT_SKILL, "herdr gates each `done` or `blocked` delivery itself",
            "herdr gates each `done` delivery itself", T_DOCS, "rust"),
    Control("p37b_seat_asked", SEAT_SKILL, "then asked, never sent back", "then sent back", T_DOCS, "rust"),
    Control("p37b_gate_skill_never_sent_back", GATE_SKILL, "A `blocked` delivery is never sent back",
            "A `blocked` delivery is sent back", T_DOCS, "rust"),
)
