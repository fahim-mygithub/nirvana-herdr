# review-trim-agy-browser 2026-11-02: a lesson review only when there is something to learn (live checks F8), and
# agy seats take browser orders (plan 35 R2) (nirvana/plans/review-trim-agy-browser-2026-11-02.md). Controls starting
# with lrs_. One sabotage per assertion, each on a product line; the test named must print FAIL with it in place.
# `Control` is provided by controls.py.
REVIEW = "src/nirvana_review.rs"
SEAT = "src/nirvana_seat.rs"
SEATS = "src/nirvana_seats.toml"
SKILL = "nirvana/claude/skills/seat/SKILL.md"
GATE = "src/nirvana_gate.rs"

T_SKIP = "lrs_a_clean_first_pass_is_skipped_before_the_cap_counts_it"
T_SIGNALS = "lrs_each_report_signal_keeps_the_review"
T_CHAIN = "lrs_a_pass_after_a_send_back_keeps_the_review_through_a_reseat"
T_GATE = "lrs_the_gate_seats_no_review_of_a_clean_first_pass_and_counts_none"
T_AGY = "lrs_agy_seats_list_browser_and_a_browser_order_can_seat_agy"
T_UPGRADE = "lrs_an_installed_34a_or_35c_default_upgrades_to_r2"
T_ASKED = "lrs_a_question_in_the_chain_or_the_one_that_started_it_keeps_the_review"
RUN = "src/nirvana_run.rs"


def _seat(model: str, pool: str, caps: str) -> str:
    return (f'model = "{model}"\n'
            f'family = "{"anthropic" if pool == "google3p" else "google"}"\n'
            f'pool = "{pool}"\nefforts = ["low", "medium", "high"]\ncapabilities = {caps}\n')


AGY_SMALL = '["code", "tools", "image", "pdf", "web", "browser"]'
AGY_BIG = '["code", "tools", "image", "pdf", "web", "browser", "image_gen", "video", "audio", "context_1m"]'

CONTROLS = (
    # F8: only a pass can be skipped; a send-back always has something to learn
    Control("lrs_only_a_pass_is_skipped", REVIEW,
            "    if *verdict != Verdict::Pass {\n        return None;\n    }\n",
            "    if false {\n        return None;\n    }\n",
            T_SKIP, "rust"),
    # F8: the skip is decided before the cap, so a skipped review is never reported as the cap's
    Control("lrs_skip_before_cap", REVIEW,
            "    if let Some(why) = idle(state, known, verdict) {\n",
            "    if let Some(why) =\n        idle(state, known, verdict).filter(|_| cap_refusal(&project.root, cap, at).is_none())\n    {\n",
            T_SKIP, "rust"),
    # F8: each report signal keeps the review, one control per key
    Control("lrs_signal_gotchas", REVIEW,
            'const SIGNALS: [&str; 4] = ["gotchas", "lesson", "lesson_ops", "card_ops"];',
            'const SIGNALS: [&str; 3] = ["lesson", "lesson_ops", "card_ops"];',
            T_SIGNALS, "rust"),
    Control("lrs_signal_lesson", REVIEW,
            'const SIGNALS: [&str; 4] = ["gotchas", "lesson", "lesson_ops", "card_ops"];',
            'const SIGNALS: [&str; 3] = ["gotchas", "lesson_ops", "card_ops"];',
            T_SIGNALS, "rust"),
    Control("lrs_signal_lesson_ops", REVIEW,
            'const SIGNALS: [&str; 4] = ["gotchas", "lesson", "lesson_ops", "card_ops"];',
            'const SIGNALS: [&str; 3] = ["gotchas", "lesson", "card_ops"];',
            T_SIGNALS, "rust"),
    Control("lrs_signal_card_ops", REVIEW,
            'const SIGNALS: [&str; 4] = ["gotchas", "lesson", "lesson_ops", "card_ops"];',
            'const SIGNALS: [&str; 3] = ["gotchas", "lesson", "lesson_ops"];',
            T_SIGNALS, "rust"),
    # F8: an empty list is the clean report's own shape, not a signal
    Control("lrs_empty_list_is_no_signal", REVIEW,
            "        Value::Array(items) => items.iter().any(said),\n",
            "        Value::Array(_) => true,\n",
            T_SIGNALS, "rust"),
    # review round nit: a blank gotcha, null, an empty object or a blank string says nothing; a lesson line does
    Control("lrs_blank_item_is_no_signal", REVIEW,
            "        Value::Array(items) => items.iter().any(said),\n",
            "        Value::Array(items) => !items.is_empty(),\n",
            T_SIGNALS, "rust"),
    Control("lrs_null_is_no_signal", REVIEW,
            "        Value::Null => false,\n",
            "        Value::Null => true,\n",
            T_SIGNALS, "rust"),
    Control("lrs_empty_object_is_no_signal", REVIEW,
            "        Value::Object(fields) => !fields.is_empty(),\n",
            "        Value::Object(_) => true,\n",
            T_SIGNALS, "rust"),
    Control("lrs_blank_string_is_no_signal", REVIEW,
            "        Value::String(text) => !text.trim().is_empty(),\n",
            "        Value::String(_) => true,\n",
            T_SIGNALS, "rust"),
    Control("lrs_string_is_a_signal", REVIEW,
            "        Value::String(text) => !text.trim().is_empty(),\n",
            "        Value::String(_) => false,\n",
            T_SIGNALS, "rust"),
    # review round nit: a cap of 0 says reviews are off, not "nothing to learn"
    Control("lrs_cap_zero_says_off", REVIEW,
            "    if cap == 0 {\n        return Err(cap_refusal(&project.root, cap, at));\n    }\n",
            "",
            T_SKIP, "rust"),
    # F8: a send-back above the order keeps the review
    Control("lrs_send_back_in_chain", REVIEW,
            '            (Some("send_back"), Some(_)) => return None,\n',
            '            (Some("send_back"), Some(_)) => break,\n',
            T_CHAIN, "rust"),
    # F8: the walk goes on through a reseat, which `nirvana_gate::chain` stops at
    Control("lrs_walk_through_reseat", REVIEW,
            "                    return None;\n                }\n                current = parent.clone();\n",
            "                    return None;\n                }\n                break;\n",
            T_CHAIN, "rust"),
    # review round nit: a reseat of a first order (its parent never gated) is still the first pass (test r3)
    Control("lrs_ungated_reseat_parent_is_first_pass", REVIEW,
            "                    .is_some_and(|verdict| verdict != Outcome::Pass.word())\n",
            "                    .map_or(true, |verdict| verdict != Outcome::Pass.word())\n",
            T_CHAIN, "rust"),
    # F8: a reseat whose parent was gated other than pass keeps the review
    Control("lrs_non_pass_parent", REVIEW,
            "                    .is_some_and(|verdict| verdict != Outcome::Pass.word())\n",
            "                    .is_some_and(|_| false)\n",
            T_CHAIN, "rust"),
    # F8: the gate's pass path asks wanted_for with the pass, so a clean first pass seats no review (merge-time (a))
    Control("lrs_gate_pass_asks_wanted_for", GATE,
            "            &crate::nirvana_review::Verdict::Pass,\n            table,\n        );\n",
            "            &crate::nirvana_review::Verdict::SentBack(Vec::new()),\n            table,\n        );\n",
            T_GATE, "rust"),
    # and the send-back path with the send-back, so a clean first order sent back is still reviewed
    Control("lrs_gate_send_back_asks_wanted_for", GATE,
            "        &crate::nirvana_review::Verdict::SentBack(reasons.clone()),\n",
            "        &crate::nirvana_review::Verdict::Pass,\n",
            "p33_a_review_is_queued_after_a_send_back_with_its_reasons", "rust"),
    # review S3: a question this order's chain asked keeps the review (a worker that asked mid-order)
    Control("lrs_asked_in_chain", REVIEW,
            "        (order.asked > 0 || order.answers.is_some()) && chain_root(state, order.id()) == root\n",
            "        order.answers.is_some() && chain_root(state, order.id()) == root\n",
            T_ASKED, "rust"),
    # review S3: the run's question that started the chain keeps it (the 0.52.0 blocked-asked redo)
    Control("lrs_answer_in_chain", REVIEW,
            "        (order.asked > 0 || order.answers.is_some()) && chain_root(state, order.id()) == root\n",
            "        order.asked > 0 && chain_root(state, order.id()) == root\n",
            T_ASKED, "rust"),
    # review S3: only the order's own chain counts, not every question in the run
    Control("lrs_question_same_root_only", REVIEW,
            "        (order.asked > 0 || order.answers.is_some()) && chain_root(state, order.id()) == root\n",
            "        order.asked > 0 || order.answers.is_some()\n",
            T_ASKED, "rust"),
    # review S3: the chain root is found up through a reseat
    Control("lrs_chain_root_walks_up", REVIEW,
            "                    Some(parent) => current = parent.clone(),\n",
            "                    Some(_) => break,\n",
            T_ASKED, "rust"),
    # review S3: replay counts each question an order asked
    Control("lrs_run_counts_asked", RUN,
            "                    known.asked += 1;\n",
            "",
            T_ASKED, "rust"),
    # review S3: a question stays open for the next fresh order
    Control("lrs_run_opens_question", RUN,
            "                        open_questions.push(known.seated.order.clone());\n",
            "",
            T_ASKED, "rust"),
    # review S3: the next fresh order answers the open question
    Control("lrs_run_answer_claims", RUN,
            "                            && !open_questions.is_empty())\n",
            "                            && false)\n",
            T_ASKED, "rust"),
    # review S3: a worker that asked and then delivered answered its own question
    Control("lrs_run_delivered_answers_own", RUN,
            "                open_questions.retain(|open| open != order);\n",
            "",
            T_ASKED, "rust"),
    # review S3: a lesson review seated after a question answers none
    Control("lrs_run_reviews_answer_none", RUN,
            '    class == crate::nirvana_review::CLASS || class == "review.adversary"\n',
            '    class == "review.adversary"\n',
            T_ASKED, "rust"),
    # review round nit: the desktop assertion's own line, `claude_only` skipping a non-claude seat
    Control("lrs_claude_only_skips_agy", SEAT,
            '        .filter(|_| seat.cli != "claude")\n',
            "        .filter(|_| false)\n",
            T_AGY, "rust"),
    # R2: each enabled agy seat lists browser
    Control("lrs_flash_browser", SEATS,
            _seat("gemini-3.8-flash-{effort}", "gemini", AGY_BIG),
            _seat("gemini-3.8-flash-{effort}", "gemini", AGY_BIG.replace(' "browser",', "")),
            T_AGY, "rust"),
    Control("lrs_argon_browser", SEATS,
            _seat("gemini-4-argon-{effort}", "argon", AGY_BIG),
            _seat("gemini-4-argon-{effort}", "argon", AGY_BIG.replace(' "browser",', "")),
            T_AGY, "rust"),
    Control("lrs_g_opus_browser", SEATS,
            _seat("claude-opus-5-5-{effort}", "google3p", AGY_SMALL),
            _seat("claude-opus-5-5-{effort}", "google3p", AGY_SMALL.replace(', "browser"', "")),
            T_AGY, "rust"),
    Control("lrs_g_sonnet_browser", SEATS,
            _seat("claude-sonnet-5-5-{effort}", "google3p", AGY_SMALL),
            _seat("claude-sonnet-5-5-{effort}", "google3p", AGY_SMALL.replace(', "browser"', "")),
            T_AGY, "rust"),
    # R2: research.web is not rescored, and its why says so
    Control("lrs_research_web_why", SEATS,
            "agy seats list browser since live check 35.1 (one order, so not rescored)",
            "agy seats list browser since live check 35.1",
            T_AGY, "rust"),
    Control("lrs_research_web_not_rescored", SEATS,
            '(one order, so not rescored)"\nseats = [\n  { seat = "flash", score = 84 },\n',
            '(one order, so not rescored)"\nseats = [\n  { seat = "flash", score = 86 },\n',
            T_AGY, "rust"),
    # R2: the seat skill names `--needs browser` as the way to a browser on agy
    Control("lrs_skill_browser_on_agy", SKILL,
            "so `--needs browser` is the way to a browser on agy too",
            "so `--needs bladebro` is the way to a browser on agy",
            T_AGY, "rust"),
    # R2: plan 35c's default (v8) is a historical one, so an installed copy upgrades to R2
    Control("lrs_v8_historical", SEAT,
            '    include_str!("nirvana_seats_v8.toml"),\n',
            "",
            T_UPGRADE, "rust"),
    # and plan 34a's (v7), which 35c left without a copy (the owner's install holds it)
    Control("lrs_v7_historical", SEAT,
            '    include_str!("nirvana_seats_v7.toml"),\n',
            "",
            T_UPGRADE, "rust"),
    # review round nit: the three intermediates that shipped a default with no copy
    Control("lrs_v9_historical", SEAT,
            '    include_str!("nirvana_seats_v9.toml"),\n',
            "",
            T_UPGRADE, "rust"),
    Control("lrs_v10_historical", SEAT,
            '    include_str!("nirvana_seats_v10.toml"),\n',
            "",
            T_UPGRADE, "rust"),
    Control("lrs_v11_historical", SEAT,
            '    include_str!("nirvana_seats_v11.toml"),\n',
            "",
            T_UPGRADE, "rust"),
)
