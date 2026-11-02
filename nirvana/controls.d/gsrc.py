# Live checks 2026-11-01 F2 and F3 (package `gsrc`, nirvana/plans/sources-effort-2026-11-02.md): the sources axis
# asks for sources only of an order that asks for the web, and a child's re-pick keeps its parent's effort. The owner,
# 2026-11-02: "Use your recomendation for all of these". Controls starting with gsrc_. One sabotage per assertion, each
# on a product line or a doc's text; the test named must print FAIL with it in place (written before the run).
# The review round (gsrc-review.md S1-S6, N1-N5) re-pointed the effort controls at `Request::effort` and added the
# rest. `Control` is provided by controls.py. No control reaches the network.
GATE = "src/nirvana_gate.rs"
SEAT = "src/nirvana_seat.rs"
WORKER = "src/nirvana_worker.rs"
ROLE = "nirvana/claude/ROLE.md"
GATE_SKILL = "nirvana/claude/skills/gate/SKILL.md"

T_WEB = "gsrc_the_web_is_asked_by_the_class_a_need_or_the_order_text"
T_WORDS = "gsrc_the_words_catch_lookups_and_skip_local_hosts_and_code"
T_NOTE = "gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note"
T_OPENED = "gsrc_a_no_web_order_that_opened_pages_must_cite_them"
T_BRIEF = "gsrc_a_send_back_of_a_no_web_order_wants_no_fetch"
T_SEAT = "gsrc_seat_run_asks_sources_only_of_an_order_that_asks_for_the_web"
T_BOOKED = "gsrc_seat_run_reads_the_needs_the_booking_was_made_with"
T_KEEP = "gsrc_a_send_back_keeps_the_parents_effort"
T_STEP = "gsrc_a_stepped_up_redo_keeps_the_parents_effort"
T_RESEAT = "gsrc_a_rate_limited_reseat_keeps_the_parents_effort"
T_NOSEAT = "gsrc_a_seat_without_the_parents_effort_keeps_the_picks"
T_PRESSURE = "gsrc_a_child_is_picked_at_the_parents_effort_and_pressure_now_steps_it_down"
T_BASE = "gsrc_a_childs_base_is_the_parents_effort_for_the_mode_the_cap_and_the_climb"
T_DOCS = "gsrc_the_docs_say_sources_only_when_the_order_asks_for_the_web"

WEB_CLASS_ARM = '    if class == WEB_CLASS {\n        return Some(format!("the class is {WEB_CLASS}"));\n'
NEEDS_ARM = "    if let Some(need) = needs.iter().find(|need| WEB_NEEDS.contains(&need.as_str())) {\n"
URL_ARM = "        .find(|url| !local_host(url))\n    {\n"
NAMES_ARM = "    if let Some(word) = WEB_NAMES.iter().find(|word| has_word(&lower, word)) {\n"
LOOKUP_ARM = "        .find_map(|sentence| LOOKUP_WORDS.iter().find(|word| has_word(sentence, word)));\n"
LOCAL_ARM = "        .filter(|sentence| !LOCAL_MARKS.iter().any(|mark| sentence.contains(mark)))\n"
LINE_CLASS = ('    if !class.starts_with("research.") && !class.starts_with("media.") {\n        return None;\n    }\n'
              "    Some(match web_asked(class, needs, written) {\n")
WEB_OF_CLASS = '    if class == WEB_CLASS {\n        Some("the class is research.web")\n'
NO_WEB_NOTE = '        None => Axis::new(\n            "sources",\n            Mark::Note,\n'
ASKED_FAIL = '        Some(asked) => Axis::new(\n            "sources",\n            Mark::Fail,\n'
OPENED_ARM = "        None if !opened.is_empty() => Axis::new(\n"
PAGE_FILTER = "            urls.is_empty() || urls.iter().any(|url| !local_host(url))\n"
CHECK_START = "    };\n    let mut missing = Vec::new();\n"
BRIEF_IF = "    let no_web = if spec(root_text).no_web {\n"
SEAT_LINE = "    let sources_line = crate::nirvana_gate::sources_brief_line(&booked.class, &web_needs, &text);\n"
ARGS_NEEDS = ("    picked.needs = picked.request.needs.clone();\n"
              "    // Plan 29 §5.1: a need naming one of the project's MCP servers is no seat capability; the order takes a "
              "claude seat.\n"
              "    let needs = crate::nirvana_gui_tools::seat_needs(&table, root, &picked.request.needs)?;\n"
              "    (picked.request.needs, picked.request.claude_only) = (needs.rest, needs.claude_only);\n")
PARENT_ARM = "    } else if let Some(eff) = parent {\n        eff\n"
CLIMB_BASE = ("                        let base_effort = request\n                            .effort\n"
              "                            .or_else(|| request.profile.as_ref().and_then(|profile| profile.effort))\n")

CONTROLS = (
    # F2: what asks for the web. The class, each need, the URL and its host, the bare domain, the words.
    Control("gsrc_web_class_ignored", GATE, WEB_CLASS_ARM, WEB_CLASS_ARM.replace("class == WEB_CLASS", "false"),
            T_WEB, "rust"),
    Control("gsrc_web_needs_ignored", GATE, NEEDS_ARM, NEEDS_ARM.replace("|need| WEB_NEEDS", "|need| false && WEB_NEEDS"),
            T_WEB, "rust"),
    Control("gsrc_web_need_web_dropped", GATE, 'const WEB_NEEDS: [&str; 3] = ["browser", "bladebro", "web"];',
            'const WEB_NEEDS: [&str; 3] = ["browser", "bladebro", "webx"];', T_WEB, "rust"),
    Control("gsrc_web_url_ignored", GATE, URL_ARM, URL_ARM.replace("|url| !local_host(url)", "|_| false"),
            T_WEB, "rust"),
    # Review S5: a URL on this machine is not the web.
    Control("gsrc_web_url_local_counts", GATE, URL_ARM, URL_ARM.replace("|url| !local_host(url)", "|_| true"),
            T_WORDS, "rust"),
    Control("gsrc_local_host_name", GATE, '    host == "localhost"\n', '    host == "localhostx"\n', T_WORDS, "rust"),
    Control("gsrc_local_host_127", GATE, '        || host.starts_with("127.")\n', '        || host.starts_with("128.")\n',
            T_WORDS, "rust"),
    Control("gsrc_local_host_v6", GATE, '        || host == "::1"\n', '        || host == "::2"\n', T_WORDS, "rust"),
    # Review S3: a bare domain asks; a path segment does not.
    Control("gsrc_bare_domain_ignored", GATE, "    if let Some(host) = bare_domain(written) {\n",
            "    if let Some(host) = bare_domain(written).filter(|_| false) {\n", T_WORDS, "rust"),
    Control("gsrc_bare_domain_any_segment", GATE, "        let named = labels.len() >= 2\n",
            "        let named = labels.len() >= 1\n", T_WORDS, "rust"),
    # The words: the outright names, the lookups, the local marks, the word edges, the sentences, the case.
    Control("gsrc_web_names_ignored", GATE, NAMES_ARM, NAMES_ARM.replace("|word| has_word(&lower, word)", "|_| false"),
            T_WEB, "rust"),
    Control("gsrc_lookups_ignored", GATE, LOOKUP_ARM, LOOKUP_ARM.replace("|word| has_word(sentence, word)", "|_| false"),
            T_WORDS, "rust"),
    Control("gsrc_local_marks_ignored", GATE, LOCAL_ARM, "        .filter(|_| true)\n", T_WORDS, "rust"),
    Control("gsrc_word_runs_on", GATE, "            && (!joins(last) || !joins(after.chars().next()))\n",
            "            && true\n", T_WORDS, "rust"),
    Control("gsrc_word_no_plural", GATE, "        let after = after.strip_prefix('s').unwrap_or(after);\n", "",
            T_WORDS, "rust"),
    Control("gsrc_sentences_whole", GATE, '        .flat_map(|line| line.split(". "))\n',
            '        .flat_map(|line| line.split("\\u{0}"))\n', T_WORDS, "rust"),
    Control("gsrc_web_text_case_kept", GATE, "    let lower = written.to_lowercase();\n",
            "    let lower = written.to_owned();\n", T_WEB, "rust"),
    # A word is spoilt, not removed: each row of the word table names one.
    Control("gsrc_word_tiktok_dropped", GATE, '    "tiktok",\n', '    "tiktokx",\n', T_WORDS, "rust"),
    Control("gsrc_word_internet_dropped", GATE, '    "on the internet",\n', '    "on thex internet",\n', T_WORDS, "rust"),
    Control("gsrc_word_look_up_dropped", GATE, '    "look up",\n', '    "look upx",\n', T_WORDS, "rust"),
    Control("gsrc_word_search_for_dropped", GATE, '    "search for",\n', '    "search forx",\n', T_WORDS, "rust"),
    Control("gsrc_word_website_dropped", GATE, '''    "'s website",\n''', '''    "'s websitex",\n''', T_WORDS, "rust"),
    Control("gsrc_word_podcast_dropped", GATE, '    "podcast",\n', '    "podcastx",\n', T_WORDS, "rust"),
    Control("gsrc_mark_md_dropped", GATE, '"~/", ".md",', '"~/", ".mdx",', T_WORDS, "rust"),
    Control("gsrc_mark_folder_dropped", GATE, '" folder",', '" folderx",', T_WORDS, "rust"),
    Control("gsrc_mark_src_dropped", GATE, '"src/", "./",', '"srcx/", "./",', T_WORDS, "rust"),
    # F2: the line herdr writes, and only for research and media orders.
    Control("gsrc_no_web_gets_web_line", GATE, "        None => NO_WEB_SOURCES,\n", "        None => WEB_SOURCES,\n",
            T_WEB, "rust"),
    Control("gsrc_line_for_every_class", GATE, LINE_CLASS, LINE_CLASS.replace(
        '!class.starts_with("research.") && !class.starts_with("media.")', "false"), T_WEB, "rust"),
    # F2: the gate reads the line back; the class wins; an order with neither line keeps the old fail.
    Control("gsrc_spec_misses_no_web", GATE, "            spec.no_web = true;\n", "            spec.no_web = false;\n",
            T_NOTE, "rust"),
    # Review N3: the short mark survives a re-wrap; the whole line does not.
    Control("gsrc_spec_reads_whole_line", GATE, "        } else if line.starts_with(NO_WEB_MARK) {\n",
            "        } else if line.starts_with(NO_WEB_SOURCES) {\n", T_NOTE, "rust"),
    Control("gsrc_web_of_class_ignored", GATE, WEB_OF_CLASS, WEB_OF_CLASS.replace("class == WEB_CLASS", "false"),
            T_NOTE, "rust"),
    Control("gsrc_web_of_legacy_no_web", GATE, "    } else if spec.no_web {\n", "    } else if !spec.no_web {\n",
            T_NOTE, "rust"),
    # F2: nothing cited is a note without the web and a fail with it; every source cited is still checked.
    Control("gsrc_no_web_none_cited_fails", GATE, NO_WEB_NOTE, NO_WEB_NOTE.replace("Mark::Note", "Mark::Fail"),
            T_NOTE, "rust"),
    Control("gsrc_web_none_cited_notes", GATE, ASKED_FAIL, ASKED_FAIL.replace("Mark::Fail", "Mark::Note"),
            T_NOTE, "rust"),
    Control("gsrc_no_web_cited_unchecked", GATE, CHECK_START,
            '    };\n    if web.is_none() {\n        return Some(Axis::new("sources", Mark::Pass, "unchecked"));\n    }\n'
            "    let mut missing = Vec::new();\n", T_NOTE, "rust"),
    Control("gsrc_gate_always_web", GATE, "    let web = web_of(subject.class, subject.spec);\n",
            '    let web = Some("always");\n', T_NOTE, "rust"),
    # Review S4: a no-web order whose worker opened pages must cite them; this machine's pages do not count.
    Control("gsrc_opened_pages_ignored", GATE, OPENED_ARM, "        None if false => Axis::new(\n", T_OPENED, "rust"),
    Control("gsrc_opened_local_counts", GATE, PAGE_FILTER, "            true\n", T_OPENED, "rust"),
    Control("gsrc_opened_browse_skipped", GATE, PAGE_FILTER, PAGE_FILTER.replace("urls.is_empty() || ", ""),
            T_OPENED, "rust"),
    # F2: a send-back of a no-web order says no source is wanted, and only of a no-web order.
    Control("gsrc_brief_never_no_web", GATE, BRIEF_IF, "    let no_web = if false {\n", T_BRIEF, "rust"),
    Control("gsrc_brief_always_no_web", GATE, BRIEF_IF, "    let no_web = if true {\n", T_BRIEF, "rust"),
    # Review S2: neither line talks a worker out of a fetch the answer needs.
    Control("gsrc_line_talks_out_of_fetch", GATE,
            "If answering needs a page you do not have, open it and cite it in the Work Report's",
            "Answer without opening a page, and put in the Work Report's", T_BRIEF, "rust"),
    Control("gsrc_redo_talks_out_of_fetch", GATE, "If the fix needs a page you do not have, open it and cite it;",
            "Do not open a page for the fix;", T_BRIEF, "rust"),
    # F2: `seat run` writes the line, read from the order as written.
    Control("gsrc_seat_reads_no_text", WORKER, SEAT_LINE, SEAT_LINE.replace("&text);", '"https://");'),
            T_SEAT, "rust"),
    Control("gsrc_seat_writes_no_line", WORKER, "    if let Some(line) = sources_line {\n",
            "    if let Some(line) = sources_line.filter(|_| false) {\n", T_SEAT, "rust"),
    # Review S1: the needs the booking was made with reach the line, as given.
    Control("gsrc_seat_ignores_booked_needs", WORKER, SEAT_LINE, SEAT_LINE.replace("&web_needs", "&request.needs"),
            T_BOOKED, "rust"),
    Control("gsrc_pick_books_no_needs", SEAT, "            let chosen = Pick { needs, ..chosen };\n",
            "            let chosen = Pick { needs: Vec::new(), ..chosen };\n", T_BOOKED, "rust"),
    Control("gsrc_pick_books_routed_needs", SEAT, ARGS_NEEDS,
            ARGS_NEEDS.replace("    picked.needs = picked.request.needs.clone();\n", "")
            + "    picked.needs = picked.request.needs.clone();\n", T_BOOKED, "rust"),
    Control("gsrc_seat_run_books_no_needs", WORKER, "        pick.needs = request.needs.clone();\n", "", T_BOOKED,
            "rust"),
    # F3: each re-pick is made at the parent's effort: a send-back, its step-up (an independent control, review N5),
    # a rate-limit reseat.
    Control("gsrc_send_back_drops_effort", GATE, "    request.effort = Some(seated.effort);\n", "", T_KEEP, "rust"),
    Control("gsrc_step_up_drops_effort", SEAT,
            "    let parent = parent_effort.filter(|_| override_effort.is_none());\n",
            "    let parent = parent_effort.filter(|_| override_effort.is_none() && !is_override);\n", T_STEP, "rust"),
    Control("gsrc_reseat_drops_effort", WORKER, "    request.effort = Some(seated.effort);\n", "", T_RESEAT, "rust"),
    Control("gsrc_parent_effort_ignored", SEAT, PARENT_ARM, PARENT_ARM.replace("= parent {", "= parent.filter(|_| false) {"),
            T_PRESSURE, "rust"),
    Control("gsrc_parent_note_kept_silent", SEAT, '            format!("effort {} kept from the parent", eff.word())\n',
            '            format!("effort {}", eff.word())\n', T_KEEP, "rust"),
    Control("gsrc_parent_note_lacking_silent", SEAT, '''            format!("the parent's effort {}", eff.word())\n''',
            '            format!("effort {}", eff.word())\n', T_NOSEAT, "rust"),
    # Review S6 and the pressure ruling: pressure read now steps a child down from its parent's effort.
    Control("gsrc_parent_never_stepped", SEAT, "    if step_down {\n        want = want.down().unwrap_or(want);\n",
            "    if step_down && parent.is_none() {\n        want = want.down().unwrap_or(want);\n", T_PRESSURE, "rust"),
    Control("gsrc_parent_lifted_by_sota", SEAT,
            "    if override_effort.is_none() && parent.is_none() && mode == Mode::Sota {\n",
            "    if override_effort.is_none() && mode == Mode::Sota {\n", T_BASE, "rust"),
    Control("gsrc_parent_capped", SEAT, "    } else if want > Effort::High && parent.is_none() {\n",
            "    } else if want > Effort::High {\n", T_BASE, "rust"),
    Control("gsrc_climb_base_is_class", SEAT, CLIMB_BASE,
            "                        let base_effort = request\n"
            "                            .profile\n                            .as_ref()\n"
            "                            .and_then(|profile| profile.effort)\n", T_BASE, "rust"),
    # The docs.
    Control("gsrc_role_rule_gone", ROLE, "herdr asks an order for web sources only when it asks for the web",
            "herdr asks every research order for web sources", T_DOCS, "rust"),
    Control("gsrc_role_opened_gone", ROLE, "one that opened pages must cite them", "one that opened pages may skip them",
            T_DOCS, "rust"),
    Control("gsrc_gate_skill_fails_nothing", GATE_SKILL, "opened is a note", "opened fails", T_DOCS, "rust"),
    Control("gsrc_gate_table_every_order", GATE_SKILL,
            "one that asks for the web, or whose worker opened a page, cites at least one",
            "each cites at least one", T_DOCS, "rust"),
)
