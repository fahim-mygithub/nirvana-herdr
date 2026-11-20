# Plan 45 P45d (nirvana/plans/2026-11-18-45-agent-profiles.md §5 "The fuller profile" last bullet and "Friction found
# in the record", §7 R2): the fitting-profile line at seating, the `seated` event's `profile` and `profile_fit`, the
# detector counting from them, a gate review that never takes its subject's profile, herdr's own lesson line read as no
# web ask, and the note for a look asked for that did not run. Controls starting with p95_. One sabotage per
# assertion, each on a product line (ROLE.md's is the role text the test reads); the test named in the fifth field is
# the one expected to print its red line with the sabotage in place, and the comment above each says which assertion
# goes red, written before the run. `Control` is provided by controls.py.
WORKER = "src/nirvana_worker.rs"
GATE = "src/nirvana_gate.rs"
RUN = "src/nirvana_run.rs"
DETECT = "src/nirvana_profile_detect.rs"
ROLE = "nirvana/claude/ROLE.md"

T_LINE = "nirvana_worker::tests::p45d_the_fit_line_names_one_three_and_then_and_n_more"
T_NOLINE = "nirvana_worker::tests::p45d_no_line_for_a_passed_profile_a_teach_a_review_or_an_acceptance_order"
T_E2E = "nirvana_worker::tests::p45d_seat_run_prints_the_fitting_line_and_journals_null_then_the_name"
T_REFED = "nirvana_worker::tests::p45d_herdrs_own_lesson_line_in_a_refed_order_does_not_ask_for_the_web"
T_ROLE = "nirvana_worker::tests::p45d_the_role_says_the_fitting_line_is_a_hint_and_the_profile_its_call"
T_JOURNAL = "nirvana_run::tests::p45d_an_old_seated_line_reads_unknown_and_a_new_one_null_or_a_name"
T_DETECT = ("nirvana_profile_detect::tests::"
            "p45d_the_journal_profile_decides_and_its_absence_falls_back_to_the_reason")
T_FITTING = "nirvana_profile_detect::tests::p45d_fitting_names_the_profiles_usage_counts_as_fit"
T_REVIEW = "nirvana_gate::tests::p45d_a_gate_review_or_author_brief_carries_no_subject_profile"
T_NAMES = "nirvana_gate::tests::p45d_names_look_takes_the_command_and_not_a_word_that_holds_it"
T_LOOK = "nirvana_gate::tests::p45d_a_look_asked_for_that_did_not_run_is_a_note"
T_WIRED = "nirvana_gate::tests::p45d_the_gate_notes_a_missing_look_and_still_passes"
T_CHILD = "nirvana_gate::tests::p45d_a_send_back_child_journals_its_parents_profile"

CHILD = ("        profile: seated.profile.clone(),\n"
         "        profile_fit: seated.profile.as_ref().map(|_| Vec::new()),\n")
FIT_GUARD = "    if passed.is_some() || crate::nirvana_profiles::takes_no_profile_servers(class) {\n"

CONTROLS = (
    # Item 1, the line's words. Red: T_LINE's five names, "p1, p2, p3 and 2 more" (four shown and 1 more).
    Control("p95_fit_cap_raised", WORKER, "pub const FIT_NAMES: usize = 3;", "pub const FIT_NAMES: usize = 4;",
            T_LINE, "rust"),
    # Red: T_LINE's one name, "profile reel-reviewer fits ... to seat with it" (the plural form, " and reel-reviewer").
    Control("p95_fit_one_name_form_gone", WORKER, "    if rest.is_empty() {\n        return Some(format!(\n",
            "    if false {\n        return Some(format!(\n", T_LINE, "rust"),
    # Red: T_LINE's two names, "p1 and p2" (read "p1, p2").
    Control("p95_fit_and_join_gone", WORKER, '        format!("{} and {last}", head.join(", "))\n',
            '        let _ = (last, head);\n        shown.join(", ")\n', T_LINE, "rust"),
    # Item 1, no line when. Red: T_NOLINE's `Some("k5")` (a passed profile still gets names).
    Control("p95_line_for_a_passed_profile", WORKER, FIT_GUARD,
            "    if crate::nirvana_profiles::takes_no_profile_servers(class) {\n", T_NOLINE, "rust"),
    # Red: T_NOLINE's teach class first (k0 named for a profile-teach order).
    Control("p95_line_for_a_review_class", WORKER, FIT_GUARD, "    if passed.is_some() {\n", T_NOLINE, "rust"),
    # The one rule, shared. Red: T_FITTING's first assertion (any-video, which names no class, missing).
    Control("p95_fit_rule_second_copy", DETECT, "                .is_some_and(|profile| fits(profile, class, needs))\n",
            "                .is_some_and(|profile| profile.class.as_deref() == Some(class))\n", T_FITTING, "rust"),
    # Items 1 and 2, end to end. Red: T_E2E's third printed line (the line not printed).
    Control("p95_line_not_printed", WORKER, '    if let Some(line) = outcome["profile_fit_line"].as_str() {\n',
            '    if let Some(line) = outcome["profile_fit_lines"].as_str() {\n', T_E2E, "rust"),
    # Red: T_E2E's third printed line (the answer carries no line).
    Control("p95_line_not_answered", WORKER, '        "profile_fit_line": fit_line(&profile_fit, &seated.class),\n',
            '        "profile_fit_line": Value::Null,\n', T_E2E, "rust"),
    # Red: T_E2E's third printed line (nothing fits when nothing is read).
    Control("p95_fit_not_read_at_seating", WORKER,
            "    let profile_fit = fitting_profiles(&project.root, profile, &booked.class, &request.needs);\n",
            "    let profile_fit: Vec<String> = Vec::new();\n", T_E2E, "rust"),
    # Item 2. Red: T_E2E's `seated("f1").profile == Some(None)` (the key not written: None).
    Control("p95_profile_not_journaled", WORKER,
            "        profile: Some(loaded.as_ref().map(|loaded| loaded.profile.name.clone())),\n",
            "        profile: None,\n", T_E2E, "rust"),
    # Red: T_E2E's `seated("f1").profile_fit == Some(["tidier"])`.
    Control("p95_profile_fit_not_journaled", WORKER, "        profile_fit: Some(profile_fit.clone()),\n",
            "        profile_fit: None,\n", T_E2E, "rust"),
    # Red: T_JOURNAL's "an unknown stays absent" (an old line written back with `"profile":null`).
    Control("p95_unknown_written_as_null", RUN,
            '        skip_serializing_if = "Option::is_none"\n    )]\n    pub profile: Option<Option<String>>,\n',
            '        skip_serializing_if = "Option::is_some"\n    )]\n    pub profile: Option<Option<String>>,\n',
            T_JOURNAL, "rust"),
    # Red: T_JOURNAL's n1 `Some(None)` (a written `null` read back as absent, None).
    Control("p95_null_read_as_absent", RUN, '        deserialize_with = "present",\n', "", T_JOURNAL, "rust"),
    # Item 3. Red: T_DETECT's carried 3 (o2's `null` falls back to its reason: 4).
    Control("p95_detect_null_falls_back", DETECT, "                Some(Value::Null) => Some(None),\n",
            "                Some(Value::Null) => None,\n", T_DETECT, "rust"),
    # Red: T_DETECT's carried 3 (the key unread: o0 and o2 by reason, 2).
    Control("p95_detect_key_unread", DETECT, '            let journaled = match event.get("profile") {\n',
            '            let journaled = match event.get("no_such_key") {\n', T_DETECT, "rust"),
    # Red: T_DETECT's carried 3 (an absent key does not fall back: o0 lost, 2).
    Control("p95_detect_absent_no_fallback", DETECT, '                None => text_of(&event, "reason")\n',
            '                None => text_of(&event, "no_such_reason")\n', T_DETECT, "rust"),
    # Item 4. Red: T_REVIEW's `in_order(review) is none` (the subject's `Profile: judge` quoted).
    Control("p95_review_quotes_the_profile", GATE, "        quoted(root_text)\n", "        original(root_text)\n",
            T_REVIEW, "rust"),
    # Red: T_REVIEW's `in_order(author) is none` (the author brief quotes it).
    Control("p95_author_quotes_the_profile", GATE, "        quoted(request)\n", "        original(request)\n",
            T_REVIEW, "rust"),
    # Item 5. Red: T_REFED's copy holds NO_WEB_MARK (herdr's lesson line read as a web ask: WEB_SOURCES).
    Control("p95_refed_lessons_read_for_web", WORKER,
            "        &crate::nirvana_profile_detect::own_text(&text),\n    );\n", "        &text,\n    );\n",
            T_REFED, "rust"),
    # Item 6, the rule. Red: T_NAMES's "herdr lookup a.png" (read as a look).
    Control("p95_names_look_any_suffix", GATE, '            && !spaced["look".len()..]\n',
            "            && !spaced[spaced.len()..]\n", T_NAMES, "rust"),
    # Red: T_NAMES's `herdr.exe look a.png` (not read as a look).
    Control("p95_names_look_no_exe", GATE, '        let rest = rest.strip_prefix(".exe").unwrap_or(rest);\n', "",
            T_NAMES, "rust"),
    # Red: T_NAMES's lesson_only (a lesson line naming herdr look read as the order's ask).
    Control("p95_asks_look_reads_lessons", GATE, "    names_look(&crate::nirvana_profile_detect::own_text(text))\n",
            "    names_look(text)\n", T_NAMES, "rust"),
    # Red: T_NAMES's `spec(...).asks_look` (never set).
    Control("p95_asks_look_never_set", GATE, "    spec.asks_look = asks_look(text);\n", "", T_NAMES, "rust"),
    # Item 6, the stream. Red: T_LOOK's `looks_in(&failed) == (1, 0)` (an errored look counted as run).
    Control("p95_errored_look_counted", GATE, "                            ran += usize::from(!failed);\n",
            "                            ran += 1;\n", T_LOOK, "rust"),
    # Red: T_LOOK's `looks_in(&agy_failed) == (1, 0)` (agy's exit 3 counted as run).
    Control("p95_agy_exit_ignored", GATE, "            && agy_command_ok(answer)\n", "            && !answer.is_empty()\n",
            T_LOOK, "rust"),
    # Red: T_LOOK's `looks_in(&agy_failed) == (1, 0)` (agy's repeated step counted twice: 2 made).
    Control("p95_agy_step_counted_twice", GATE, "        if !agy_seen.contains(&call) {\n", "        if true {\n",
            T_LOOK, "rust"),
    # Item 6, the note. Red: T_LOOK's `none.mark == Note` (a fail).
    Control("p95_look_note_is_a_fail", GATE, "            Mark::Note,\n            if made == 0 {\n",
            "            Mark::Fail,\n            if made == 0 {\n", T_LOOK, "rust"),
    # Red: T_LOOK's `look_axis(false, &other) == None` (an order that asked for none noted).
    Control("p95_look_asked_ignored", GATE, "    if !asked {\n        return None;\n    }\n    let (made, ran)",
            "    let _ = asked;\n    let (made, ran)", T_LOOK, "rust"),
    # Red: T_WIRED's `axis(&det, "look")` (no look axis in the gate's axes).
    Control("p95_look_axis_not_wired", GATE, "    axes.extend(look_axis(subject.spec.asks_look, &stream));\n", "",
            T_WIRED, "rust"),
    # Item 2, a child. Red: T_CHILD's r-named `child.profile == Some(Some("judge"))` (None).
    Control("p95_child_drops_the_profile", GATE, CHILD,
            "        profile: None,\n        profile_fit: seated.profile.as_ref().map(|_| Vec::new()),\n",
            T_CHILD, "rust"),
    # Red: T_CHILD's r-old `child.profile_fit == None` (an empty list where the parent's was unknown).
    Control("p95_child_fit_always_known", GATE, CHILD,
            "        profile: seated.profile.clone(),\n        profile_fit: Some(Vec::new()),\n", T_CHILD, "rust"),
    # Item 7. Red: T_ROLE's "Passing `--profile` is your call".
    Control("p95_role_takes_the_call", ROLE, "Passing `--profile` is your call", "Passing `--profile` is herdr's call",
            T_ROLE, "rust"),
)
