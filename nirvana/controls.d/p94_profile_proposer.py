# Plan 45 P45b (nirvana/plans/2026-11-18-45-agent-profiles.md §2 L8, §5 "The proposer", §6): the detector with no
# model, its own proposals, `profile_ops` in the lesson and idle reviews, `herdr profile detect`. Controls starting with
# p94_. One sabotage per assertion, each on a product line; the test named in the fifth field is the one expected to
# print its red line with the sabotage in place, and the comment above each says which assertion goes red (written
# before the run). `Control` is provided by controls.py.
DETECT = "src/nirvana_profile_detect.rs"
REVIEW = "src/nirvana_review.rs"
IDLE = "src/nirvana_idle_review.rs"

T_ONE = ("nirvana_profile_detect::tests::"
         "p94_a_repeated_role_yields_exactly_one_proposal_and_a_project_without_one_none")
T_EDGE = "nirvana_profile_detect::tests::p94_each_threshold_holds_at_its_edge"
T_KINDS = "nirvana_profile_detect::tests::p94_send_backs_reviews_teach_and_herdrs_own_briefs_are_not_counted"
T_BOILER = "nirvana_profile_detect::tests::p94_herdrs_own_lines_are_not_counted"
T_DISMISS = ("nirvana_profile_detect::tests::"
             "p94_a_dismissed_finding_is_not_proposed_again_and_a_waiting_one_is_not_duplicated")
T_PATCH = "nirvana_profile_detect::tests::p94_a_paragraph_all_orders_of_one_profile_type_is_a_patch_of_it"
T_USAGE = "nirvana_profile_detect::tests::p94_usage_counts_a_carried_an_unused_and_a_never_fitting_profile"
T_CAPS = "nirvana_profile_detect::tests::p94_the_caps_and_the_budget_skip_and_say_how_much"
T_HOSTILE = "nirvana_profile_detect::tests::p94_hostile_order_text_lands_redacted_and_carries_no_key_of_its_own"
T_CLI = "nirvana_profile_detect::tests::p94_the_cli_prints_and_writes_only_with_write"
T_BRIEF = ("nirvana_profile_detect::tests::"
           "p94_a_brief_is_unchanged_byte_for_byte_without_findings_and_gets_them_with")
T_OPS = "nirvana_review::tests::p94_profile_ops_are_parsed_validated_and_refused_with_a_reason"
T_GATE = "nirvana_review::tests::p94_a_reviews_profile_ops_land_as_proposals_and_never_block_its_lesson_ops"
T_BYTES = "nirvana_review::tests::p94_a_review_brief_with_no_findings_is_todays_byte_for_byte"
T_LINE = "nirvana_idle_review::tests::p94_the_result_line_counts_profile_ops"
T_GIVEN = "nirvana_idle_review::tests::p94_an_idle_reviews_given_streams_are_the_streams_of_the_orders_it_read"

CONTROLS = (
    # Thresholds. Red: T_EDGE's "100 characters count" (101 needed) and "4 orders are under the threshold" (4 reach).
    Control("p94_min_chars_raised", DETECT, "pub const MIN_CHARS: usize = 100;", "pub const MIN_CHARS: usize = 101;",
            T_EDGE, "rust"),
    Control("p94_min_orders_lowered", DETECT, "pub const MIN_ORDERS: usize = 5;", "pub const MIN_ORDERS: usize = 4;",
            T_EDGE, "rust"),
    # What counts as an original. Red: T_KINDS's "only the four originals" (a send-back and a reseat counted).
    Control("p94_children_counted", DETECT, "            if child\n", "            if false\n", T_KINDS, "rust"),
    # Red: T_KINDS (the lesson review o2-lr, which has no parent here, counted as a fifth original).
    Control("p94_review_class_counted", DETECT, "                || class == crate::nirvana_review::CLASS\n",
            "                || false\n", T_KINDS, "rust"),
    # Red: T_KINDS (the teach order counted).
    Control("p94_teach_class_counted", DETECT, "                || class == crate::nirvana_knowledge::TEACH_CLASS\n",
            "                || false\n", T_KINDS, "rust"),
    # Red: T_KINDS (the gate review and the held-out author, herdr's own briefs, counted).
    Control("p94_herdr_briefs_counted", DETECT, ".is_some_and(|text| herdr_brief(&own_text(text)))",
            ".is_some_and(|_| false)", T_KINDS, "rust"),
    # Boilerplate. Red: T_BOILER's "repeated is empty" (the sources paragraph, herdr's two lines, counted).
    Control("p94_herdr_lines_kept", DETECT, "                line = line.replace(herdr.as_str(), \" \");",
            "                line = line.clone();", T_BOILER, "rust"),
    # Red: T_BOILER (the lessons section's line holding the judging paragraph counted).
    Control("p94_lessons_section_kept", DETECT, '            || line.starts_with("## Lessons from earlier ")\n',
            "            || false\n", T_BOILER, "rust"),
    # Red: T_BOILER (the agy profile section's quoted body counted).
    Control("p94_profile_section_kept", DETECT, '        if line.starts_with("## Profile: ") {',
            "        if false {", T_BOILER, "rust"),
    # Red: T_BOILER (the two Profile lines, 113 characters joined, counted as a paragraph).
    Control("p94_profile_lines_kept", DETECT,
            "        if line.starts_with(nirvana_profiles::LINE) || line.starts_with(nirvana_profiles::SHA_LINE)\n",
            "        if false\n", T_BOILER, "rust"),
    # The store's memory and the waiting check. Red: T_DISMISS's "the dismissed two left out" (the judging paragraph
    # comes back in the new proposal).
    Control("p94_dismissed_paragraphs_forgotten", DETECT, "            if dismissed.contains(sha) {",
            "            if false {", T_DISMISS, "rust"),
    # Red: T_DISMISS's "waits already as pp-1" (the plan does not see the waiting proposal; the store then refuses).
    Control("p94_waiting_unseen", DETECT, ".find(|waiting| waiting.kind == kind && waiting.profile == name)",
            ".find(|_| false)", T_DISMISS, "rust"),
    # Patch. Red: T_PATCH's "one waiting patch of ue-builder" (no patch target).
    Control("p94_patch_target_gone", DETECT, "            (1, 0) => {", "            (9, 9) => {", T_PATCH, "rust"),
    # Red: T_PATCH's "proposes nothing more" after accept (the body's own paragraph proposed again).
    Control("p94_patch_repeats_body", DETECT,
            "            if kind == Kind::Patch && body_now.contains(&text.to_lowercase()) {",
            "            if false {", T_PATCH, "rust"),
    # Fit. Red: T_USAGE's `unused` row (0, 5): any class fits.
    Control("p94_fit_ignores_class", DETECT, "        && profile.class.as_deref().is_none_or(|own| own == class)\n",
            "        && true\n", T_USAGE, "rust"),
    # Red: T_USAGE's `unused` row (0, 5) becomes (0, 6): the media order without the need fits.
    Control("p94_fit_ignores_needs", DETECT, "        && profile.needs.iter().all(|need| needs.contains(need))",
            "        && true", T_USAGE, "rust"),
    # Red: T_USAGE's `bare` row (0, 0): a profile naming neither class nor needs fits every order.
    Control("p94_bare_profile_fits_all", DETECT, "    (profile.class.is_some() || !profile.needs.is_empty())",
            "    true", T_USAGE, "rust"),
    # Red: T_USAGE's unused == ["unused"] (5 missed is under a `>`).
    Control("p94_unused_threshold", DETECT, "        if used.fit - used.carried >= MIN_ORDERS {",
            "        if used.fit - used.carried > MIN_ORDERS {", T_USAGE, "rust"),
    # Red: T_USAGE's last assertion (the fitted media orders reported unprofiled).
    Control("p94_unprofiled_ignores_fit", DETECT, "        if orders.len() >= MIN_ORDERS && !fitted && !class.is_empty() {",
            "        if orders.len() >= MIN_ORDERS && !class.is_empty() {", T_USAGE, "rust"),
    # Caps and budget. Red: T_CAPS's files == 1 (the huge copy read).
    Control("p94_order_byte_cap_gone", DETECT, "                Some(Ok(meta)) if meta.len() > limits.order_bytes => {",
            "                Some(Ok(meta)) if meta.len() > u64::MAX - 1 => {", T_CAPS, "rust"),
    # Red: T_CAPS's originals == 4 (run-a read too).
    Control("p94_run_cap_gone", DETECT, "    runs.truncate(limits.runs);", "    runs.truncate(usize::MAX);", T_CAPS,
            "rust"),
    # Red: T_CAPS's (orders, skipped) == (2, 5).
    Control("p94_order_cap_gone", DETECT, "            if out.orders >= limits.orders || over(&started) {",
            "            if over(&started) {", T_CAPS, "rust"),
    # Red: T_CAPS's budget assertion (a zero budget reads everything).
    Control("p94_budget_gone", DETECT, "    let over = |started: &Instant| started.elapsed() > limits.budget;",
            "    let over = |_started: &Instant| false;", T_CAPS, "rust"),
    # Hostile text. Red: T_HOSTILE's kept-detection assertion (the token lands in the cache).
    Control("p94_detection_unredacted", DETECT, "                        text: nirvana_lessons::redact(&paragraph),",
            "                        text: paragraph.clone(),", T_HOSTILE, "rust"),
    # Red: T_ONE's "one waiting" (with no front matter of herdr's, the hostile-free body is refused by the store).
    Control("p94_front_matter_not_herdrs", DETECT,
            '                let mut text = format!("---\\ndescription: {description}\\nclass: {class}\\n");',
            "                let mut text = String::new();", T_ONE, "rust"),
    # CLI. Red: T_CLI's "nothing is written" (a run without --write writes).
    Control("p94_cli_writes_without_flag", DETECT, "    let outcomes: Vec<(Planned, Option<Outcome>)> = if write {",
            "    let outcomes: Vec<(Planned, Option<Outcome>)> = if true {", T_CLI, "rust"),
    # Brief. Red: T_BRIEF's brief_section(&[]) == "" (the rules printed with no finding).
    Control("p94_rules_without_findings", DETECT, "    if findings.is_empty() {\n        return String::new();\n    }\n",
            "", T_BRIEF, "rust"),
    # Red: T_BRIEF's "proposed as pp-1, waiting for the owner".
    Control("p94_waiting_not_said", DETECT,
            ".find(|waiting| waiting.key.contains(sha) || waiting.key == finding.key())", ".find(|_| false)", T_BRIEF,
            "rust"),
    # Red: T_BYTES (a review with nothing about its class gets a changed brief and a given file).
    Control("p94_findings_always_added", DETECT, "    if found.is_empty() {\n        return brief;\n    }\n", "", T_BYTES,
            "rust"),
    # Review ops. Red: T_OPS line 2 (a `model` key is not named; the op lands as the same proposal and is refused so).
    Control("p94_op_keys_unchecked", REVIEW, "object.keys().find(|key| !allowed.contains(&key.as_str()))",
            "object.keys().find(|_| false)", T_OPS, "rust"),
    # Red: T_OPS line 10 (a second proposal for frame-judge reaches the store, whose refusal reads otherwise).
    Control("p94_op_waiting_unseen", REVIEW,
            "                .find(|waiting| waiting.kind == kind && waiting.profile == name)\n            {",
            "                .find(|_| false)\n            {", T_OPS, "rust"),
    # Red: T_OPS line 7 (an op quoting nothing found is written).
    Control("p94_op_evidence_unchecked", REVIEW, "            if !found {", "            if false {", T_OPS, "rust"),
    # Red: T_OPS line 8 (a finding the review was not given is taken).
    Control("p94_op_finding_unchecked", REVIEW,
            ".filter(|finding| !keys.iter().any(|key| key == finding))", ".filter(|_| false)", T_OPS, "rust"),
    # Red: T_OPS line 4 (a two-line description writes a `model:` line, refused by the store instead).
    Control("p94_op_description_lines", REVIEW,
            "if description.is_empty() || description.contains(['\\n', '\\r']) {", "if description.is_empty() {",
            T_OPS, "rust"),
    # Red: T_OPS's report_axis Pass (profile_ops left in, check_report names an unknown key).
    Control("p94_report_keeps_profile_ops", REVIEW, ".and_then(|object| object.remove(PROFILE_OPS));",
            ".and_then(|object| object.get(PROFILE_OPS).cloned());", T_OPS, "rust"),
    # Gate. Red: T_GATE's two profile_op axes (none applied).
    Control("p94_gate_skips_profile_ops", REVIEW, "        if !held_any && !profile_ops.is_empty() {",
            "        if false {", T_GATE, "rust"),
    # Red: T_GATE's "no nothing to keep" (a review with profile ops said it kept nothing).
    Control("p94_nothing_to_keep_with_ops", REVIEW,
            "                lines: if card_ops.is_empty() && profile_ops.is_empty() {",
            "                lines: if card_ops.is_empty() {", T_GATE, "rust"),
    # Red: T_GATE's brief assertion (the review's own class is not the one asked).
    Control("p94_brief_wrong_class", REVIEW, "        Some(&known.seated.class),", '        Some("no-such-class"),',
            T_GATE, "rust"),
    # Idle. Red: T_LINE (the profile ops not counted: "0 applied").
    Control("p94_result_line_lessons_only", IDLE,
            '    let counted = |axis: &Axis| axis.axis == "lesson_op" || axis.axis == review::PROFILE_OP;',
            '    let counted = |axis: &Axis| axis.axis == "lesson_op";', T_LINE, "rust"),
    # Red: T_GIVEN's TOTAL 12 passed (no stream given).
    Control("p94_given_streams_empty", IDLE, "        .map(|(r, o, _, _)| nirvana_gate::stream_text(project, r, o))",
            '        .map(|(_, _, _, _)| String::from("x"))', T_GIVEN, "rust"),
)

# The lead 2026-11-19: `herdr profile new --help` wrote `.nirvana/profiles/--help.md` and the proposals lock.
PROFILES = "src/nirvana_profiles.rs"
T_DASH = ("nirvana_profile_detect::tests::"
          "p94_a_new_profile_name_never_starts_with_a_dash_and_one_on_disk_still_loads")
T_HELP = "nirvana_profile_detect::tests::p94_help_after_any_profile_verb_prints_the_usage"
T_GUARD = "nirvana_profile_detect::tests::p94_a_bad_proposal_id_or_op_is_refused_before_the_proposals_lock"
T_DETECT_HELP = "nirvana_profile_detect::tests::p94_detect_help_prints_the_usage_and_writes_nothing"
CONTROLS = CONTROLS + (
    # Red: T_DASH's "never starts with `-`" (`--help` passes `valid_name` and is written).
    Control("p94_new_name_dash_allowed", PROFILES, "    if name.starts_with('-') {\n        return Some(format!(",
            "    if false {\n        return Some(format!(", T_DASH, "rust"),
    # Red: T_HELP's first case (`profile --help` is not the usage).
    Control("p94_help_not_guarded", PROFILES, "    if help(0) || help(1) || (nested && help(2)) {",
            "    if false && nested {", T_HELP, "rust"),
    # Red: T_GUARD's `accept pp-x` (it reaches the store, whose lock comes first).
    Control("p94_bad_id_not_guarded", PROFILES,
            "    if let Some(id) = id.filter(|id| !crate::nirvana_profile_proposals::valid_id(id)) {",
            "    if let Some(id) = id.filter(|_| false) {", T_GUARD, "rust"),
    # Red: T_DETECT_HELP (`--help` is an unknown flag, exit 2).
    Control("p94_detect_help_gone", DETECT, '            "--help" | "-h" => {', '            "--never-a-flag" => {',
            T_DETECT_HELP, "rust"),
)

# The lead 2026-11-19: the worker's report check names profile_ops (P45b report, section 4, hunk 1).
WORKER = "src/nirvana_worker.rs"
T_DELIVER = "nirvana_review::tests::p94_a_review_delivery_carrying_profile_ops_passes_the_worker_and_the_gate"
T_BUILD_OPS = "nirvana_review::tests::p94_a_build_order_carrying_profile_ops_is_ignored_as_lesson_ops_are"
CONTROLS = CONTROLS + (
    # Red: T_DELIVER's `stream.report()` (the worker's end refuses an unknown key).
    Control("p94_worker_key_unknown", WORKER, '    "profile_ops",\n];', '    "profile_ops_unknown",\n];', T_DELIVER,
            "rust"),
    # Red: T_DELIVER's schema assertion (the schema a worker is started with does not name it).
    Control("p94_worker_schema_unnamed", WORKER, '"profile_ops":{"type":"array","items"',
            '"profile_opz":{"type":"array","items"', T_DELIVER, "rust"),
    # Red: T_DELIVER's last assertion (a profile_ops that is not a list passes).
    Control("p94_worker_list_unchecked", WORKER, '    for key in ["lesson_ops", "card_ops", "profile_ops"] {',
            '    for key in ["lesson_ops", "card_ops"] {', T_DELIVER, "rust"),
    # Red: T_BUILD_OPS's pass (a build order carrying profile_ops fails its report check instead of being ignored).
    Control("p94_build_ops_key_unknown", WORKER, '    "profile_ops",\n];', '    "profile_ops_unknown",\n];',
            T_BUILD_OPS, "rust"),
)
