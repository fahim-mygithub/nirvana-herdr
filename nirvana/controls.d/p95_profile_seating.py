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
# The fix round's tests (review findings 1 to 6).
REVIEW = "src/nirvana_review.rs"
T_FIX_REFED = "nirvana_worker::tests::p45d_fix_a_refed_copy_holding_herdrs_no_web_line_does_not_ask_for_the_web"
T_FIX_CONT = "nirvana_worker::tests::p45d_fix_a_continuation_brief_reads_as_its_original_order"
T_FIX_RESEAT = "nirvana_worker::tests::p45d_fix_a_reseat_child_journals_its_parents_profile"
T_FIX_REVIEW = "nirvana_review::tests::p45d_fix_a_lesson_review_journals_no_profile_as_known"
T_FIX_OWN = ("nirvana_profile_detect::tests::"
             "p45d_fix_own_text_cuts_every_line_herdr_writes_and_keeps_the_orders_words")
T_FIX_SECTION = "nirvana_profile_detect::tests::p45d_fix_own_text_cuts_only_herdrs_section_and_reads_what_follows"
T_FIX_DASH = "nirvana_profile_detect::tests::p45d_fix_fitting_never_names_a_profile_that_starts_with_a_dash"
T_FIX_NAMES = "nirvana_gate::tests::p45d_fix_names_look_wants_an_argument_after_look"
T_FIX_RUNS = "nirvana_gate::tests::p45d_fix_looks_in_counts_herdr_only_as_the_command_word"

CHILD = ("        profile: seated.profile.clone(),\n"
         "        profile_fit: seated.profile.as_ref().map(|_| Vec::new()),\n")
RESEAT_CHILD = ("                profile: seated.profile.clone(),\n"
                "                profile_fit: seated.profile.as_ref().map(|_| Vec::new()),\n")
REVIEW_SEATED = "        profile: Some(None),\n        profile_fit: Some(Vec::new()),\n"
LOOK_STARTS = "    token.starts_with(['\"', '\\'', '-'])\n"
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
    # Item 6, the rule. The fix round moved the word's end into `look_argument`'s space after `look`. Red: T_NAMES's
    # "herdr look-alike" (read as a look whose argument is the flag `-alike`).
    Control("p95_names_look_any_suffix", GATE, "    if arg.len() == after.len() {\n        return false;\n    }\n",
            "", T_NAMES, "rust"),
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

    # ---- fix round ----
    # Finding 1, own_text's herdr lines. Red: T_FIX_REFED's re-fed nw2 copy holds WEB_SOURCES (NO_WEB_SOURCES's
    # `web page` read as the ask).
    Control("p95_own_text_keeps_herdr_lines", DETECT,
            "            out.extend(without_herdr_lines(std::mem::take(&mut paragraph)));\n",
            "            out.extend(std::mem::take(&mut paragraph).into_iter().map(str::to_owned));\n",
            T_FIX_REFED, "rust"),
    # Red: T_FIX_OWN's older wording (`NO_WEB_MARK ... so cite none.`) kept.
    Control("p95_own_text_old_mark_kept", DETECT,
            "            if flat.starts_with(mark) || (rest != flat && rest.is_empty()) {\n",
            "            if rest != flat && rest.is_empty() {\n", T_FIX_OWN, "rust"),
    # Red: T_FIX_OWN's re-wrapped line beside the order's own line (its pieces after the first line kept).
    Control("p95_own_text_fragments_kept", DETECT,
            "            } else if herdr_lines().iter().any(|herdr| herdr.contains(flat.as_str())) {\n",
            "            } else if false {\n", T_FIX_OWN, "rust"),
    # Red: T_FIX_OWN's "One plate." (the typed line holding DEVICE_LINE dropped whole).
    Control("p95_own_text_mixed_line_dropped", DETECT,
            "            } else if rest != flat {\n                Some(rest)\n",
            "            } else if rest != flat {\n                None\n", T_FIX_OWN, "rust"),
    # Red: T_FIX_OWN's "# Fix it" (SOURCES_REDO kept, not a herdr line; NO_WEB_REDO also goes by NO_WEB_MARK).
    Control("p95_own_text_no_redo_line", DETECT, "            crate::nirvana_gate::SOURCES_REDO,\n", "", T_FIX_OWN,
            "rust"),
    # Red: T_FIX_OWN's send-back brief (its heading and opening, NO_WEB_REDO beside the gate's words, kept).
    Control("p95_own_text_send_back_kept", DETECT,
            '        && (send_back || line.starts_with("# Continuation of ") || line == "## What the gate found"))\n',
            '        && (line.starts_with("# Continuation of ") || line == "## What the gate found"))\n',
            T_FIX_OWN, "rust"),
    # Red: T_FIX_CONT's `own == order` (the continuation's heading, opening and quoted last turn kept).
    Control("p95_own_text_continuation_kept", DETECT,
            '        && (send_back || line.starts_with("# Continuation of ") || line == "## What the gate found"))\n',
            '        && (send_back || line == "## What the gate found"))\n', T_FIX_CONT, "rust"),
    # Red: T_FIX_OWN's titled order (`# Send-back 1 of 3: the parcel`, no original-order heading) cut.
    Control("p95_own_text_briefed_ignored", DETECT, "    (briefed\n        && (send_back",
            "    (true\n        && (send_back", T_FIX_OWN, "rust"),
    # Red: T_FIX_OWN's send-back brief (`## The original order` kept as the orchestrator's heading).
    Control("p95_own_text_original_heading_kept", DETECT,
            "        if briefed && line == ORIGINAL_HEADING {\n            section = None;\n            continue;\n"
            "        }\n", "", T_FIX_OWN, "rust"),
    # Finding 6. Red: T_FIX_SECTION's first `own` (the owner's `## Also` section cut with the lessons).
    Control("p95_own_text_listed_to_the_end", DETECT,
            "            Some(Section::Listed) if !is_heading(line) => continue,\n",
            "            Some(Section::Listed) => continue,\n", T_FIX_SECTION, "rust"),
    # Red: T_FIX_SECTION's unended profile section (the search line after the quoted body cut).
    Control("p95_own_text_quoted_to_the_end", DETECT,
            "            Some(Section::Quoted) if line.trim().is_empty() || line.starts_with('>') => continue,\n",
            "            Some(Section::Quoted) => continue,\n", T_FIX_SECTION, "rust"),
    # Finding 2, the reseat child. Red: T_FIX_RESEAT's named `reseated.profile == Some(Some("judge"))` (None).
    Control("p95_reseat_drops_the_profile", WORKER, RESEAT_CHILD,
            "                profile: None,\n"
            "                profile_fit: seated.profile.as_ref().map(|_| Vec::new()),\n", T_FIX_RESEAT, "rust"),
    # Red: T_FIX_RESEAT's old `reseated.profile_fit == None` (an empty list where the parent's was unknown).
    Control("p95_reseat_fit_always_known", WORKER, RESEAT_CHILD,
            "                profile: seated.profile.clone(),\n                profile_fit: Some(Vec::new()),\n",
            T_FIX_RESEAT, "rust"),
    # Finding 2, a lesson review. Red: T_FIX_REVIEW's `review.profile == Some(None)` (absent: None).
    Control("p95_review_profile_unknown", REVIEW, REVIEW_SEATED,
            "        profile: None,\n        profile_fit: Some(Vec::new()),\n", T_FIX_REVIEW, "rust"),
    # Red: T_FIX_REVIEW's `review.profile_fit == Some([])` (None).
    Control("p95_review_fit_unknown", REVIEW, REVIEW_SEATED,
            "        profile: Some(None),\n        profile_fit: None,\n", T_FIX_REVIEW, "rust"),
    # Finding 3, an argument after `look`. Red: T_FIX_NAMES's "Keep the herdr look and feel ..." (read as an ask).
    Control("p95_look_takes_prose", GATE, LOOK_STARTS, "    true || token.starts_with(['\"', '\\'', '-'])\n",
            T_FIX_NAMES, "rust"),
    # Red: T_FIX_NAMES's ROLE.md form, `herdr look "<path>" --ask` (a quoted argument not read).
    Control("p95_look_no_quote_or_flag", GATE, LOOK_STARTS, "    false\n", T_FIX_NAMES, "rust"),
    # Red: T_FIX_NAMES's look skill form, `herdr look <file> --ask` (a placeholder not read).
    Control("p95_look_no_placeholder", GATE, "        || token.starts_with(['<', '$', '%', '{', '~'])\n", "",
            T_FIX_NAMES, "rust"),
    # Red: T_FIX_NAMES's `herdr look C:\Users\o\Desktop\shot --ask q` (a path with no extension not read).
    Control("p95_look_no_path", GATE, "        || path\n", "        || path && false\n", T_FIX_NAMES, "rust"),
    # Red: T_FIX_NAMES's `herdr look frame_012.png --ask` (a file name not read).
    Control("p95_look_no_extension", GATE, "        || extension\n", "        || extension && false\n", T_FIX_NAMES,
            "rust"),
    # Finding 4, herdr as the command word. Red: T_FIX_RUNS's `echo herdr look a.png > notes.txt` (1, 1).
    Control("p95_looks_in_reads_any_word", GATE,
            '                        if matches!(name, "Bash" | "PowerShell") && runs_look(command) {\n',
            '                        if matches!(name, "Bash" | "PowerShell") && names_look(command) {\n',
            T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's agy `findstr /s "herdr look a.png" notes.md` (1, 1).
    Control("p95_agy_looks_reads_any_word", GATE, "        if !runs_look(command) {\n",
            "        if !names_look(command) {\n", T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's `cd shots && herdr look a.png --ask q` (the line read as one command whose word is cd).
    Control("p95_look_segments_not_read", GATE, "    shell_segments(line).iter().any(|segment| {\n",
            "    [line.to_owned()].iter().any(|segment| {\n", T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's `& "C:\...\herdr.exe" look` (PowerShell's call operator read as the command word: (0, 0)).
    Control("p95_look_call_operator_kept", GATE, '.skip_while(|word| *word == "&" || env_assignment(word));\n',
            ".skip_while(|word| env_assignment(word));\n", T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's `FRAME=a.png herdr look "$FRAME"` (the assignment read as the command word: (0, 0)).
    Control("p95_look_assignment_not_skipped", GATE, '.skip_while(|word| *word == "&" || env_assignment(word));\n',
            '.skip_while(|word| *word == "&");\n', T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's `C:/Users/o/.../herdr.exe look` (a path to herdr.exe not read: (0, 0)).
    Control("p95_look_herdr_path_not_read", GATE, "    let name = word.rsplit(['/', '\\\\']).next().unwrap_or(word);\n",
            "    let name = word;\n", T_FIX_RUNS, "rust"),
    # Red: T_FIX_RUNS's note `shows no \`herdr look\` call` (the old "shows no look").
    Control("p95_look_note_says_none_ran", GATE, "stream shows no `herdr look` call (one through a shell \\\n",
            "stream shows no look (one through a shell \\\n", T_FIX_RUNS, "rust"),
    # Finding 5. Red: T_FIX_DASH's `["judge"]` (`--help` and `-x` named).
    # Plan 47 P47b: the detector's profiles carry a built-in flag, a third field.
    Control("p95_fit_names_a_dash", DETECT, "        .filter(|(name, _, _)| !name.starts_with('-'))\n", "",
            T_FIX_DASH, "rust"),
)
