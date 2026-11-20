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
    Control("p94_lessons_section_kept", DETECT, '    if line.starts_with("## Lessons from earlier ")\n',
            "    if false\n", T_BOILER, "rust"),
    # Red: T_BOILER (the agy profile section's quoted body counted).
    Control("p94_profile_section_kept", DETECT, '    if line.starts_with("## Profile: ") {\n        return Some(Section::Quoted);',
            "    if false {\n        return Some(Section::Quoted);", T_BOILER, "rust"),
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
    # Hostile text: P45b fix round F1 withholds a secret-shaped paragraph whole, so its redaction is p94_secret_*.
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

# The lead 2026-11-19: `herdr run end` runs the detector (P45b report, section 4, hunk 2).
RETRO = "src/nirvana_retro.rs"
T_END_ONE = "nirvana_profile_detect::tests::p94_run_end_writes_the_one_proposal_for_a_repeated_role"
T_END_NONE = "nirvana_profile_detect::tests::p94_run_end_without_a_repeated_role_writes_none_and_says_nothing"
T_END_SAFE = "nirvana_profile_detect::tests::p94_a_spent_budget_or_a_detector_error_leaves_the_run_end_as_it_was"
CONTROLS = CONTROLS + (
    # Red: T_END_ONE's "profiles: pp-1 proposed" (run end runs no detector).
    Control("p94_run_end_no_detector", RETRO,
            "        .chain(crate::nirvana_profile_detect::refresh(project, run))\n", "", T_END_ONE, "rust"),
    # Red: T_END_NONE's "says nothing" (a quiet detection prints its skip line anyway).
    Control("p94_run_end_always_says", DETECT, "    if detection.skipped.any() {", "    if true {", T_END_NONE,
            "rust"),
    # Red: T_END_SAFE's kept-detection assertion (a partial detection replaces the whole one).
    Control("p94_partial_detection_kept", DETECT, "    let kept = if detection.skipped.budget {",
            "    let kept = if false {", T_END_SAFE, "rust"),
)

# The lead 2026-11-19: the store refuses a second waiting proposal of one kind and profile (P45b report, section 4).
PP = "src/nirvana_profile_proposals.rs"
T_STORE = ("nirvana_profile_detect::tests::"
           "p94_the_store_refuses_a_second_waiting_proposal_for_one_profile_under_any_key")
CONTROLS = CONTROLS + (
    # Red: T_STORE's second write (written as pp-2 beside pp-1).
    Control("p94_store_second_waiting_allowed", PP,
            "        .find(|waiting| waiting.kind == draft.kind && waiting.profile == draft.profile)",
            "        .find(|_| false)", T_STORE, "rust"),
)

# The lead 2026-11-19: the idle flow end to end.
T_IDLE_E2E = ("nirvana_idle_review::tests::"
              "p94_an_idle_review_carries_findings_and_its_profile_ops_become_a_proposal")
CONTROLS = CONTROLS + (
    # Red: T_IDLE_E2E's brief assertion (the idle brief gets no findings).
    Control("p94_idle_brief_no_findings", IDLE,
            "            crate::nirvana_profile_detect::with_findings(project, &run, &id, brief, None, None);",
            "            brief;", T_IDLE_E2E, "rust"),
    # Red: T_IDLE_E2E's one proposal (the gate forgets the findings the review was given; the op's finding is refused).
    Control("p94_gate_given_findings_dropped", REVIEW,
            "                let given = crate::nirvana_profile_detect::given(project, run, &root);",
            "                let given: Vec<crate::nirvana_profile_detect::Finding> = Vec::new();", T_IDLE_E2E,
            "rust"),
)

# The P45b fix round (the review's F1 to F11, the lead 2026-11-19). Each red line is written before the run.
T_SECRET = ("nirvana_profile_detect::tests::"
            "p94_a_secret_shaped_paragraph_is_counted_never_drafted_and_never_written")
T_BLOCK = "nirvana_profile_detect::tests::p94_a_proposal_body_keeps_the_block_of_the_newest_order"
T_CROSS_DETECT = ("nirvana_profile_detect::tests::"
                  "p94_a_paragraph_dismissed_through_a_reviewers_proposal_is_not_proposed_by_the_detector")
T_CLI_PARTIAL = "nirvana_profile_detect::tests::p94_detect_write_keeps_the_whole_cache_when_its_budget_ran_out"
T_ORDER_FILE = ("nirvana_profile_detect::tests::"
                "p94_a_journal_order_file_outside_an_orders_folder_is_skipped_and_counted")
T_ONE_WORD = ("nirvana_profile_detect::tests::"
              "p94_a_journal_class_or_need_that_is_not_one_word_yields_no_draft_and_one_line")
T_BRIEF_KINDS = ("nirvana_profile_detect::tests::"
                 "p94_a_brief_keeps_an_unprofiled_and_an_unused_finding_beside_many_repeated_ones")
T_SEATED = ("nirvana_profile_detect::tests::"
            "p94_accept_and_rollback_from_a_seated_worker_are_refused_naming_the_order")
T_SHAPES = "nirvana_profile_proposals::tests::p94_secret_shaped_names_each_shape_and_passes_prose"
T_STORE_SECRET = ("nirvana_profile_proposals::tests::"
                  "p94_the_store_refuses_a_secret_shaped_body_or_evidence_and_accept_checks_again")
T_OPS_SECRET = ("nirvana_review::tests::"
                "p94_profile_ops_refuse_secret_shaped_text_and_quotes_of_herdrs_own_wording")
T_CAP = "nirvana_review::tests::p94_one_review_writes_at_most_three_proposals"
T_CROSS_REVIEW = ("nirvana_review::tests::"
                  "p94_a_paragraph_dismissed_through_the_detectors_proposal_is_not_proposed_by_a_reviewer")
NEVER = 'r"NEVERxMATCHx9"'
CONTROLS = CONTROLS + (
    # F1, the detector. Red: T_SECRET's "each still counts" (no finding is marked withheld; the values reach the cache).
    Control("p94_secret_paragraph_kept", DETECT,
            "                let secret = proposals::secret_shaped(&paragraph)\n"
            "                    .or_else(|| proposals::secret_shaped(&block));",
            "                let secret: Option<&'static str> = None;", T_SECRET, "rust"),
    # Red: T_SECRET's "no withheld line in the body" (the withheld findings join the judging paragraph's draft).
    Control("p94_secret_finding_drafted", DETECT, "        if secret.is_some() {\n            continue;\n        }",
            "        if false {\n            continue;\n        }", T_SECRET, "rust"),
    # F1, the shapes. Red: T_SHAPES at the shape's own text (found None, or another shape's name).
    Control("p94_shape_pem_gone", PP, '            ("PEM header", r"-----BEGIN [A-Z]"),',
            '            ("PEM header", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_aws_gone", PP, '            ("AWS access key id", r"\\b(?:AKIA|ASIA)[0-9A-Z]{16}\\b"),',
            '            ("AWS access key id", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_slack_gone", PP, '            ("Slack token", r"\\bxox[a-z]-[A-Za-z0-9-]+"),',
            '            ("Slack token", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_stripe_gone", PP, '            ("Stripe key", r"\\b(?:sk|rk)_(?:live|test)_[A-Za-z0-9]+"),',
            '            ("Stripe key", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_google_gone", PP, '            ("Google API key", r"\\bAIza[0-9A-Za-z_-]{16,}"),',
            '            ("Google API key", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_url_gone", PP,
            '            ("URL credentials", r"[A-Za-z][A-Za-z0-9+.-]*://[^/\\s:@]+:[^@\\s/]+@"),',
            '            ("URL credentials", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_uuid_gone", PP,
            '                r"(?i)\\b[a-z0-9_.-]*(?:key|token|secret|password|auth)[\\s=:\'\\x22]{0,4}[0-9a-f]{8}-',
            '                r"NEVERxMATCH(?i)\\b[a-z0-9_.-]*(?:key|token|secret|password|auth)[\\s=:\'\\x22]{0,4}[0-9a-f]{8}-',
            T_SHAPES, "rust"),
    Control("p94_shape_hex_gone", PP, '            ("run of 32 or more hex characters", r"\\b[0-9a-fA-F]{32,}\\b"),',
            '            ("run of 32 or more hex characters", ' + NEVER + '),', T_SHAPES, "rust"),
    Control("p94_shape_basic_gone", PP, "    if basic.captures_iter(text).any(|found| {",
            "    if false && basic.captures_iter(text).any(|found| {", T_SHAPES, "rust"),
    # Red: T_SHAPES's prose "A basic understanding of the engine" (any run after `basic` read as encoded).
    Control("p94_shape_basic_prose", PP,
            "        run.bytes().any(|b| b.is_ascii_digit() || matches!(b, b'+' | b'/' | b'='))\n"
            "            || run.bytes().skip(1).any(|b| b.is_ascii_uppercase())",
            "        true", T_SHAPES, "rust"),
    Control("p94_shape_assigned_gone", PP, "    if assigned.captures_iter(text).any(|found| {",
            "    if false && assigned.captures_iter(text).any(|found| {", T_SHAPES, "rust"),
    # Red: T_SHAPES's prose "Key: remember the cook order." (a colon and any word read as a credential).
    Control("p94_shape_colon_prose", PP,
            "            _ => count >= 5 && (quoted.is_some() || value.bytes().any(|b| b.is_ascii_digit())),",
            "            _ => count >= 5,", T_SHAPES, "rust"),
    Control("p94_shape_redact_gone", PP, "    if nirvana_lessons::redact(text) != text {", "    if false {",
            T_SHAPES, "rust"),
    # F1, the store. Red: T_STORE_SECRET's first refusal (the AWS key id is written to pp-1.md).
    Control("p94_store_secret_unchecked", PP,
            "    problems.extend(secret_problems(&draft.body, &draft.evidence));\n", "", T_STORE_SECRET, "rust"),
    # Red: T_STORE_SECRET's accept (the hand-edited 32-hex body becomes judge-d's profile).
    Control("p94_accept_secret_unchecked", PP,
            "    problems.extend(secret_problems(&proposal.body, &proposal.evidence));\n", "", T_STORE_SECRET, "rust"),
    # F1, the reviewer's path. Red: T_OPS_SECRET line 1 (the store's "the body holds", not the op's "its body holds").
    Control("p94_op_secret_unchecked", REVIEW,
            "                if let Some(shape) = crate::nirvana_profile_proposals::secret_shaped(text) {",
            "                if let Some(shape) = None::<&str> {", T_OPS_SECRET, "rust"),
    # F11. Red: T_OPS_SECRET's corpus assertion (herdr's template wording is quotable).
    Control("p94_quotable_template", REVIEW,
            "                    out.push(text.clone());\n                    if !block.is_empty() {",
            "                    out.push(finding.line(usize::MAX));\n                    if !block.is_empty() {",
            T_OPS_SECRET, "rust"),
    # Red: T_OPS_SECRET line 5's "one quote checked".
    Control("p94_quote_note_dropped", REVIEW, "                     checked, not the counts)\",",
            "                     not the counts)\",", T_OPS_SECRET, "rust"),
    # F9. Red: T_CAP's fourth op (written as pp-4).
    Control("p94_review_cap_gone", REVIEW, "            if written >= PROPOSALS_PER_REVIEW {", "            if false {",
            T_CAP, "rust"),
    # Red: T_CAP (nothing counted, so the fourth is written).
    Control("p94_review_cap_uncounted", REVIEW, "                written += 1;\n", "", T_CAP, "rust"),
    # F4. Red: T_CROSS_REVIEW (the reviewer's proposal of the dismissed paragraph is written as pp-2).
    Control("p94_cross_dismiss_reviewer", REVIEW,
            "                if crate::nirvana_profile_detect::dismissed_paragraphs(root, kind, &name).contains(sha) {",
            "                if false {", T_CROSS_REVIEW, "rust"),
    # Red: T_CROSS_DETECT (the detector proposes the paragraph the owner dismissed in a reviewer's proposal).
    Control("p94_cross_dismiss_detector", DETECT, '                        .or_else(|| key.strip_prefix("paragraph:"))',
            "                        .or_else(|| None::<&str>)", T_CROSS_DETECT, "rust"),
    # F3. Red: T_BLOCK's body assertion (the flattened one-line text).
    Control("p94_body_flattened", DETECT, "                let typed = if block.is_empty() { text } else { block };",
            "                let typed = text;", T_BLOCK, "rust"),
    # Red: T_BLOCK's body assertion (the first order's block, not the newest's).
    Control("p94_block_oldest_kept", DETECT, "                if para.block_run == run_at {", "                if false {",
            T_BLOCK, "rust"),
    # F5. Red: T_CLI_PARTIAL's findings assertion (the partial detection replaces the whole one).
    Control("p94_cli_partial_cache_replaced", DETECT, "        if detection.skipped.budget {\n            let _ = writeln!(",
            "        if false {\n            let _ = writeln!(", T_CLI_PARTIAL, "rust"),
    # F7. Red: T_ORDER_FILE's elsewhere == 5 (every named file read).
    Control("p94_order_file_anywhere", DETECT,
            '    (in_orders && !climbs && file == format!("{order}.md")).then(|| PathBuf::from(named))',
            "    let _ = (in_orders, climbs, file);\n    Some(PathBuf::from(named))", T_ORDER_FILE, "rust"),
    # Red: T_ORDER_FILE's elsewhere == 5 (the two under another name in an orders folder read).
    Control("p94_order_file_name_unchecked", DETECT, '(in_orders && !climbs && file == format!("{order}.md"))',
            "(in_orders && !climbs && !file.is_empty())", T_ORDER_FILE, "rust"),
    # F8. Red: T_ONE_WORD's "the journal's class" line (the class goes to the store, which says otherwise).
    Control("p94_class_word_unchecked", DETECT, "        let bad = if !crate::nirvana_review::one_word(&class) {",
            "        let bad = if false {", T_ONE_WORD, "rust"),
    # Red: T_ONE_WORD's need part (no refusal; a draft with the need).
    Control("p94_need_word_unchecked", DETECT, "                .find(|need| !crate::nirvana_review::one_word(need))",
            "                .find(|_| false)", T_ONE_WORD, "rust"),
    # Red: T_ONE_WORD's propose outcomes (a refused plan is read as known).
    Control("p94_refused_plan_known", DETECT, "                (Some(why), _) => Outcome::Refused(why.clone()),",
            "                (Some(why), _) => Outcome::Known(why.clone()),", T_ONE_WORD, "rust"),
    # F10. Red: T_BRIEF_KINDS's kinds (six repeated paragraphs).
    Control("p94_brief_kinds_starved", DETECT,
            "    let kept: Vec<usize> = [first_of(true), first_of(false)].into_iter().flatten().collect();",
            "    let kept: Vec<usize> = Vec::new();", T_BRIEF_KINDS, "rust"),
    # F2. Red: T_SEATED's first case (accept inside order b1-lr goes on).
    Control("p94_seated_accept_allowed", PROFILES, "    if let Some(why) = seated_refusal(args, env) {",
            "    if let Some(why) = None::<String> {", T_SEATED, "rust"),
    # Red: T_SEATED's first case (the order's variable is not read; nothing else marks the worker).
    Control("p94_seated_order_unread", PROFILES,
            "    let order = env(SEATED_ORDER_ENV).filter(|order| !order.trim().is_empty());",
            "    let order: Option<String> = None;", T_SEATED, "rust"),
    # Red: T_SEATED's claude case (the claude seat's own marker is not read).
    Control("p94_seated_claude_unread", PROFILES,
            "        .all(|(key, value)| env(key).as_deref() == Some(*value));", "        .all(|_| false);", T_SEATED,
            "rust"),
    # Red: T_SEATED's rollback case.
    Control("p94_seated_rollback_allowed", PROFILES,
            '        (Some("proposals"), Some("rollback")) => "proposals rollback",\n', "", T_SEATED, "rust"),
    # Applied at the merge (nirvana_worker.rs was nobody's file): every seat's worker names its order. Red: the env
    # of the command an attempt starts holds no NIRVANA_ORDER.
    Control("p94_worker_env_names_no_order", WORKER,
            "    command.env(crate::nirvana_profiles::SEATED_ORDER_ENV, &seated.order);\n", "",
            "nirvana_worker::tests::p94_an_attempt_names_its_order_in_the_workers_env", "rust"),
)

# The P45b second fix round (review2: 20 of 40 shapes reached pp-1.md). Each red line is written before the run.
T_IGNORED = ("nirvana_profile_detect::tests::"
             "p94_a_written_proposal_is_invisible_to_git_and_an_ignore_file_there_is_kept")
T_NOTE = "nirvana_profile_proposals::tests::p94_accept_says_the_profile_is_not_ignored_and_the_check_is_a_list"
T_OLD_CACHE = ("nirvana_profile_detect::tests::"
               "p94_an_old_cache_holding_raw_secret_text_is_checked_again_before_a_brief")
T_FENCE = "nirvana_profile_detect::tests::p94_a_block_with_an_odd_count_of_fence_lines_is_not_the_body"
T_LAYER = "nirvana_profile_proposals::tests::p94_the_second_layer_names_each_review2_shape_and_passes_prose"
NEVER_PREFIX = "NEVERxMATCH"
CONTROLS = CONTROLS + (
    # Item 1. Red: T_IGNORED's "git lists nothing under the folder" (pp-1.md shows as untracked).
    Control("p94_ignore_unwritten", PP, "    if let Err(err) = ensure_ignore(root) {",
            "    if let Err(err) = Ok::<bool, std::io::Error>(false) {", T_IGNORED, "rust"),
    # Red: T_IGNORED's "git lists nothing under the folder" (the file holds only comments).
    Control("p94_ignore_star_dropped", PP, '*\\n";\n', '\\n";\n', T_IGNORED, "rust"),
    # Red: T_IGNORED's last assertion (the owner's own ignore file is rewritten with herdr's text).
    Control("p94_ignore_overwritten", PP,
            "        .create_new(true)\n        .open(folder(root).join(nirvana_profiles::IGNORE_FILE))",
            "        .create(true)\n        .truncate(true)\n        .open(folder(root).join(nirvana_profiles::IGNORE_FILE))",
            T_IGNORED, "rust"),
    # Red: T_IGNORED's "git lists nothing under the folder" (write makes the folder without the file).
    Control("p94_ignore_skipped_at_write", PP, "    make_folder(root).map_err(|err| {",
            "    std::fs::create_dir_all(folder(root)).map_err(|err| {", T_IGNORED, "rust"),
    # Red: T_IGNORED's read after the dismissal (the ledger's write does not put the removed file back).
    Control("p94_ignore_skipped_at_ledger", PP, "    make_folder(root)?;",
            "    std::fs::create_dir_all(folder(root))?;", T_IGNORED, "rust"),
    # Item 2. Red: T_NOTE's "lines.len() == 2" (one line only).
    Control("p94_accept_note_gone", PP, '                let _ = writeln!(out, "{ACCEPT_NOTE}");\n', "", T_NOTE,
            "rust"),
    # Item 4 (review2 F3). Red: T_OLD_CACHE's "no hunter2hunter2 in the brief".
    Control("p94_old_cache_unchecked", DETECT, "    rechecked(&mut detection.findings);\n", "", T_OLD_CACHE, "rust"),
    # Item 5 (review2 F4). Red: T_SEATED's dismiss case (dismiss inside order b1-lr goes on).
    Control("p94_seated_dismiss_allowed", PROFILES, '        (Some("dismiss"), _) => "dismiss",\n', "", T_SEATED,
            "rust"),
    # Item 6 (review2 F6). Red: T_FENCE's first assertion (the cut block, its one fence, is kept as typed).
    Control("p94_odd_fence_kept", DETECT,
            "            let block = if fences % 2 == 0 && nirvana_lessons::one_line(&block) == line {",
            "            let block = if fences % 2 < 2 && nirvana_lessons::one_line(&block) == line {", T_FENCE, "rust"),
    # Item 3, the assignment rule widened. Red: T_LAYER at the named sample.
    # `passphrase: correct horse battery staple`.
    Control("p94_layer_passphrase", PP,
            '            _ if found[1].eq_ignore_ascii_case("passphrase") => count >= 3,\n', "", T_LAYER, "rust"),
    # `password="my pass phrase here"`.
    Control("p94_layer_quoted", PP, "        let quoted = found.get(3).or_else(|| found.get(4));",
            "        let quoted = None::<regex::Match>;", T_LAYER, "rust"),
    # `key=ab1`.
    Control("p94_layer_eq_three", PP, '            "=" => count >= 3,', '            "=" => count >= 4,', T_LAYER,
            "rust"),
    # `pwd: abc12`.
    Control("p94_layer_colon_five", PP, "            _ => count >= 5 && (quoted.is_some()",
            "            _ => count >= 6 && (quoted.is_some()", T_LAYER, "rust"),
    # `?sig=...` (the first of the new words in T_LAYER's list).
    Control("p94_layer_words", PP,
            "(passphrase|signature|sig|authorization|auth|credentials?|api[-_ ]?key|access[-_ ]?key|client[-_ ]?secret"
            "|key|secret|password|passwd|token|pwd)",
            "(passphrase|key|secret|password|passwd|token|pwd)", T_LAYER, "rust"),
    # The second layer's rules, each at its own sample.
    Control("p94_layer_authz_gone", PP, '                r#"(?i)\\bauthorization["\']?',
            '                r#"' + NEVER_PREFIX + '(?i)\\bauthorization["\']?', T_LAYER, "rust"),
    Control("p94_layer_is_gone", PP, '                r#"(?i)\\b(?:password|passphrase|passwd|pwd|secret|token|api',
            '                r#"' + NEVER_PREFIX + '(?i)\\b(?:password|passphrase|passwd|pwd|secret|token|api', T_LAYER,
            "rust"),
    Control("p94_layer_flag_gone", PP, '                r#"(?i)(?:^|\\s)--(?:token',
            '                r#"' + NEVER_PREFIX + '(?i)(?:^|\\s)--(?:token', T_LAYER, "rust"),
    Control("p94_layer_sshpass_gone", PP, '("sshpass password", r#"(?i)',
            '("sshpass password", r#"' + NEVER_PREFIX + '(?i)', T_LAYER, "rust"),
    Control("p94_layer_curl_gone", PP, '("curl user and password", r#"(?i)',
            '("curl user and password", r#"' + NEVER_PREFIX + '(?i)', T_LAYER, "rust"),
    Control("p94_layer_jwt_gone", PP, '("JWT-shaped text (eyJ...)", r"\\beyJ',
            '("JWT-shaped text (eyJ...)", r"' + NEVER_PREFIX + '\\beyJ', T_LAYER, "rust"),
    Control("p94_layer_ya29_gone", PP, '("Google OAuth token (ya29.)", r"\\bya29',
            '("Google OAuth token (ya29.)", r"' + NEVER_PREFIX + '\\bya29', T_LAYER, "rust"),
    Control("p94_layer_run_gone", PP, "    if run.find_iter(text).any(|found| {",
            "    if false && run.find_iter(text).any(|found| {", T_LAYER, "rust"),
    Control("p94_joined_pem_gone", PP, '("across whitespace: PEM header", r"',
            '("across whitespace: PEM header", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_aws_gone", PP, '("across whitespace: AWS access key id", r"',
            '("across whitespace: AWS access key id", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_slack_gone", PP, '("across whitespace: Slack token", r"',
            '("across whitespace: Slack token", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_stripe_gone", PP, '("across whitespace: Stripe key", r"',
            '("across whitespace: Stripe key", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_google_gone", PP, '("across whitespace: Google API key", r"',
            '("across whitespace: Google API key", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_github_gone", PP, '("across whitespace: GitHub token", r"',
            '("across whitespace: GitHub token", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_jwt_gone", PP, '("across whitespace: JWT-shaped text", r"',
            '("across whitespace: JWT-shaped text", r"' + NEVER_PREFIX, T_LAYER, "rust"),
    Control("p94_joined_hex_gone", PP, "    if hex.find_iter(&joined).any(|found| {",
            "    if false && hex.find_iter(&joined).any(|found| {", T_LAYER, "rust"),
    # The prose beside them. Red: T_LAYER's prose, at the line named.
    # "authorization: owner approval required before the merge" (any value after a scheme read as a secret).
    Control("p94_layer_looks_any", PP,
            "        !value.starts_with(['$', '%', '<'])\n"
            "            && (count >= long || (count >= 4 && value.bytes().any(|b| b.is_ascii_digit())))",
            "        { let _ = (count, long); true }", T_LAYER, "rust"),
    # "pass --token $DEPLOY_TOKEN_V2 from the env" (a variable read as the secret).
    Control("p94_layer_placeholder", PP, "        !value.starts_with(['$', '%', '<'])\n            && ",
            "        ", T_LAYER, "rust"),
    # "Supercalifragilisticexpialidocious is a long word" (a run of letters alone).
    Control("p94_layer_run_letters", PP,
            "        found.bytes().any(|b| b.is_ascii_digit()) && found.bytes().any(|b| b.is_ascii_alphabetic())",
            "        !found.is_empty()", T_LAYER, "rust"),
    # "the ids 1234 5678 ... in a column" (joined digits alone).
    Control("p94_joined_hex_digits", PP,
            "        found.bytes().any(|b| b.is_ascii_alphabetic()) && found.bytes().any(|b| b.is_ascii_digit())",
            "        !found.is_empty()", T_LAYER, "rust"),
    # "they Just went home and eyJ is no word" (one `eyJ` part read as a JWT).
    Control("p94_joined_jwt_one_part", PP, r'r"eyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{2,}"',
            r'r"eyJ[A-Za-z0-9_-]{2,}"', T_LAYER, "rust"),
    # "run fork_test_cases and more" (a letter before `rk_` read as a key's start).
    Control("p94_joined_stripe_left", PP, 'r"(?:^|[^A-Za-z])(?:sk|rk)_', 'r"(?:sk|rk)_', T_LAYER, "rust"),
)
