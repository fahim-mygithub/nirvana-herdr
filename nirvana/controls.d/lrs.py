# review-trim-agy-browser 2026-11-02: a lesson review only when there is something to learn (live checks F8), and
# agy seats take browser orders (plan 35 R2) (nirvana/plans/review-trim-agy-browser-2026-11-02.md). Controls starting
# with lrs_. One sabotage per assertion, each on a product line; the test named must print FAIL with it in place.
# `Control` is provided by controls.py.
REVIEW = "src/nirvana_review.rs"
SEAT = "src/nirvana_seat.rs"
SEATS = "src/nirvana_seats.toml"
SKILL = "nirvana/claude/skills/seat/SKILL.md"

T_SKIP = "lrs_a_clean_first_pass_is_skipped_before_the_cap_counts_it"
T_SIGNALS = "lrs_each_report_signal_keeps_the_review"
T_CHAIN = "lrs_a_pass_after_a_send_back_keeps_the_review_through_a_reseat"
T_GATE = "lrs_the_gate_seats_no_review_of_a_clean_first_pass_and_counts_none"
T_AGY = "lrs_agy_seats_list_browser_and_a_browser_order_can_seat_agy"
T_UPGRADE = "lrs_an_installed_34a_or_35c_default_upgrades_to_r2"


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
            "    if let Some(why) = verdict.and_then(|verdict| idle(state, known, verdict)) {\n",
            "    if let Some(why) = verdict\n        .and_then(|verdict| idle(state, known, verdict))\n"
            "        .filter(|_| cap_refusal(&project.root, cap, at).is_none())\n    {\n",
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
            "            Value::Array(items) => !items.is_empty(),\n",
            "            Value::Array(_) => true,\n",
            T_SIGNALS, "rust"),
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
    # F8: a reseat whose parent was gated other than pass keeps the review
    Control("lrs_non_pass_parent", REVIEW,
            "                    .is_some_and(|verdict| verdict != Outcome::Pass.word())\n",
            "                    .is_some_and(|_| false)\n",
            T_CHAIN, "rust"),
    # F8: the gate's queue_review seats no review of a clean first pass until the gate asks wanted_for itself
    Control("lrs_queue_review_backstop", REVIEW,
            "    if let Err(why) = wanted_for(project, &state, known, verdict, Some(table)) {\n",
            "    if let Err(why) = Ok::<(), Option<String>>(()) {\n",
            T_GATE, "rust"),
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
)
