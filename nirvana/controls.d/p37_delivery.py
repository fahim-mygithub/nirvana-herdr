# Plan 37 P37: herdr gates every delivery, checks cited videos, steps up invented sources, and briefs agy (package
# `p37_delivery`, nirvana/plans/2026-10-31-37-records/p37.md). Controls starting with p37_. One sabotage per
# assertion, each on a product line or a rule's text; the test named must print FAIL with it in place (written before
# the run). `Control` is provided by controls.py. No control reaches the network: the one that points a test build at
# an oEmbed points it at 127.0.0.1.
GATE = "src/nirvana_gate.rs"
WORKER = "src/nirvana_worker.rs"
SEAT = "src/nirvana_seat.rs"
PROFILES = "src/nirvana_profiles.rs"
ROLE = "nirvana/claude/ROLE.md"
SKILL = "nirvana/claude/skills/seat/SKILL.md"

T_EXIST = "p37_the_existence_check_fails_a_404_passes_a_200_and_only_notes_a_timeout"
T_DET = "p37_the_gate_asks_oembed_for_a_research_orders_videos"
T_STEP = "p37_a_send_back_for_invented_videos_steps_up_one_seat_and_trusts_nothing"
T_KEEP = "p37_an_ordinary_send_back_keeps_the_rules_seat"
T_GATES = "p37_herdr_gates_each_done_delivery_but_a_helpers"
T_EXEC = "p37_exec_gates_an_ordinary_done_delivery_before_the_wake"
T_LINE = "p37_a_gate_by_hand_gives_a_booked_reviewers_line_again"
T_DOCS = "p37_the_agy_brief_rules_are_in_role_and_the_seat_skill_each_naming_its_failure"
T_STEPUP = "p37_step_up_takes_the_next_stronger_seat_of_the_class"
T_TEMPLATE = "p37_profile_new_from_web_researcher_writes_the_template"

GATED_ARM = '        || (status == "done" && read_sidecar(project, run, order).helps.is_none())'
STEP_SET = "        request.override_seat = Some((up.clone(), reason.clone()));\n"

CONTROLS = (
    # Item 2 (L4): oEmbed's answers, read as the ruling says: a 404 fails, anything unsettled is a note.
    Control("p37_404_exists", GATE,
            "Err(ureq::Error::Status(404, _)) => Video::Missing,",
            "Err(ureq::Error::Status(404, _)) => Video::Exists,", T_EXIST, "rust"),
    Control("p37_200_missing", GATE,
            "Ok(response) if response.status() == 200 => Video::Exists,",
            "Ok(response) if response.status() == 200 => Video::Missing,", T_EXIST, "rust"),
    Control("p37_other_status_fails", GATE,
            'Video::Other(status) => unsettled.push(format!("{id}: oEmbed answered {status}")),',
            "Video::Other(_) => missing.push(id.clone()),", T_EXIST, "rust"),
    Control("p37_unreachable_fails", GATE,
            "                unreachable = true;\n",
            "                unreachable = true;\n                missing.push(id.clone());\n", T_EXIST, "rust"),
    Control("p37_unreachable_asks_on", GATE,
            "        if unreachable {\n", "        if false && unreachable {\n", T_EXIST, "rust"),
    Control("p37_no_cap", GATE,
            "        if n >= cap {\n", "        if n >= usize::MAX {\n", T_EXIST, "rust"),
    Control("p37_missing_passes", GATE,
            "        axis.mark = Mark::Fail;\n        axis.lines\n", "        axis.lines\n", T_EXIST, "rust"),
    Control("p37_found_unsaid", GATE,
            "    if found > 0 {\n", "    if found > 1_000_000 {\n", T_EXIST, "rust"),
    Control("p37_invented_unread", GATE,
            ".filter_map(|line| line.strip_prefix(NO_SUCH_VIDEO))",
            '.filter_map(|line| line.strip_prefix("never: "))', T_STEP, "rust"),
    # Item 2 wired into the gate, and a test build that asks nothing unless given a stand-in.
    Control("p37_check_unwired", GATE,
            "        let (axis, note) = match oembed_base() {",
            "        let (axis, note) = match None::<String> {", T_DET, "rust"),
    Control("p37_test_build_asks", GATE,
            "    return TEST_OEMBED.with_borrow(Clone::clone);",
            '    return TEST_OEMBED.with_borrow(Clone::clone).or(Some("http://127.0.0.1:9/oembed".to_owned()));',
            T_DET, "rust"),
    # Item 3 (L5): the step-up, herdr's own (no override spent), kept by the child, and its brief.
    Control("p37_no_step_up", GATE, STEP_SET, "        let _ = (up, reason);\n", T_STEP, "rust"),
    Control("p37_step_counted", GATE,
            STEP_SET + "        request.inherited_override = true;\n",
            STEP_SET + "        request.inherited_override = false;\n", T_STEP, "rust"),
    Control("p37_step_reason_lost", GATE,
            "            Ok(pick) => (pick, None, inherited_reason),",
            "            Ok(pick) => (pick, None, seated.override_reason.clone()),", T_STEP, "rust"),
    Control("p37_brief_not_restarted", GATE,
            "        _ if !invented_videos(axes).is_empty() => INVENTED_REDO.to_owned(),",
            "        _ if false => INVENTED_REDO.to_owned(),", T_STEP, "rust"),
    Control("p37_every_send_back_steps", GATE,
            "    let step_up = (!invented.is_empty())", "    let step_up = (true || !invented.is_empty())",
            T_KEEP, "rust"),
    Control("p37_step_by_score_only", SEAT,
            "        score > here.0 + EDGE || ((score - here.0).abs() <= EDGE && price > here.1)",
            "        score > here.0 + EDGE", T_STEPUP, "rust"),
    Control("p37_step_to_the_top", SEAT,
            ".min_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))",
            ".max_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))", T_STEPUP, "rust"),
    # Item 1 (L3): which deliveries `seat exec` gates, and that it gates before it wakes.
    Control("p37_blocked_gated", GATE, GATED_ARM, GATED_ARM.replace('status == "done"', 'status != "x"'),
            T_GATES, "rust"),
    Control("p37_helpers_gated", GATE, GATED_ARM,
            GATED_ARM.replace("read_sidecar(project, run, order).helps.is_none()",
                              "(true || read_sidecar(project, run, order).helps.is_none())"), T_GATES, "rust"),
    Control("p37_done_not_gated", GATE, GATED_ARM, GATED_ARM.replace('status == "done"', 'status == "never"'),
            T_GATES, "rust"),
    Control("p37_exec_no_gate", WORKER,
            "                        if gates {\n", "                        if false {\n", T_EXEC, "rust"),
    Control("p37_exec_wakes_first", WORKER,
            "                        if gates {\n",
            "                        let _ = nirvana_run::wake(project, run, false);\n                        if gates {\n",
            T_EXEC, "rust"),
    Control("p37_line_not_again", GATE,
            "                if state.order(&id).is_none() {", "                if false {", T_LINE, "rust"),
    Control("p37_line_wrong_seat", GATE, "        lease.seat,\n", '        "some-seat",\n', T_LINE, "rust"),
    Control("p37_line_no_fallback", GATE, "{id}'s booking is gone", "{id} is booked", T_LINE, "rust"),
    # Item 5: the web-researcher template and `profile new --from`.
    Control("p37_template_unrouted", PROFILES,
            '        "web-researcher" => Some(WEB_RESEARCHER),', '        "web-researcher" => None,',
            T_TEMPLATE, "rust"),
    Control("p37_template_no_connector_rule", PROFILES,
            "There is no YouTube connector", "There is a YouTube connector", T_TEMPLATE, "rust"),
    Control("p37_template_class", PROFILES,
            "class: research.web\\n\\\n# A profile", "class: research.read\\n\\\n# A profile", T_TEMPLATE, "rust"),
    # Item 4 (L2): each rule in ROLE.md and the seat skill, and the failure each names.
    Control("p37_role_gate_rule", ROLE,
            "5. herdr gates each `done` delivery itself", "5. Gate each `done` delivery yourself", T_DOCS, "rust"),
    Control("p37_role_gate_failure", ROLE,
            "In MotoProject 1 of 26 deliveries", "In MotoProject some deliveries", T_DOCS, "rust"),
    Control("p37_role_brief_failure", ROLE,
            "and its worker invented 12 of 12 sources", "and its worker failed", T_DOCS, "rust"),
    Control("p37_role_connector", ROLE,
            "r2 was told of a YouTube connector agy does not have", "r2 was told of a tool agy does not have",
            T_DOCS, "rust"),
    Control("p37_role_profile", ROLE,
            "`herdr profile new web-researcher\n  --from web-researcher`", "`herdr profile new web-researcher`",
            T_DOCS, "rust"),
    Control("p37_skill_verify_line", SKILL,
            '1. The verify line, word for word: "Never cite a URL from memory or from a search result.',
            '1. The verify line, word for word: "Cite real URLs.', T_DOCS, "rust"),
    Control("p37_skill_verify_failure", SKILL,
            "its 12 sources were\n   invented", "its 12 sources were\n   wrong", T_DOCS, "rust"),
    Control("p37_skill_method_failure", SKILL, "spent 9 steps", "spent some steps", T_DOCS, "rust"),
    Control("p37_skill_reddit_failure", SKILL, "about 10 failed Reddit reads", "some failed reads", T_DOCS, "rust"),
    Control("p37_skill_no_connector", SKILL, "agy has no YouTube connector", "agy has its own tools", T_DOCS, "rust"),
    Control("p37_skill_done_failure", SKILL, "graded its own prose check", "graded its own check", T_DOCS, "rust"),
    Control("p37_skill_owns_failure", SKILL,
            "the gate ignored reel-motion-gemini's `Owns:` until P35a", "the gate ignored it until P35a",
            T_DOCS, "rust"),
    Control("p37_skill_rules_failure", SKILL, "r2 left 6 mp4s on C:.", "r2 left files behind.", T_DOCS, "rust"),
    Control("p37_skill_foreground_failure", SKILL, "stalled 600 s", "stalled", T_DOCS, "rust"),
    Control("p37_skill_state_failure", SKILL, "a 10-slide post", "an old post", T_DOCS, "rust"),
    Control("p37_skill_order_failure", SKILL,
            "reel-motion-gemini searched and wrote, and opened nothing.", "reel-motion-gemini skipped steps.",
            T_DOCS, "rust"),
    Control("p37_skill_gate_paragraph", SKILL,
            "and its redo goes one seat up", "and its redo goes back", T_DOCS, "rust"),
    Control("p37_skill_checklist_row", SKILL, "- [ ] the verify line\n", "", T_DOCS, "rust"),
    Control("p37_skill_profile_row", SKILL,
            "- [ ] class line, and `--profile web-researcher` when the project has it", "- [ ] class line",
            T_DOCS, "rust"),
)
