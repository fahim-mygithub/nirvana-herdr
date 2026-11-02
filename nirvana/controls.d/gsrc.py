# Live checks 2026-11-01 F2 and F3 (package `gsrc`, nirvana/plans/sources-effort-2026-11-02.md): the sources axis
# asks for sources only of an order that asks for the web, and a child's re-pick keeps its parent's effort. The owner,
# 2026-11-02: "Use your recomendation for all of these". Controls starting with gsrc_. One sabotage per assertion, each
# on a product line or a doc's text; the test named must print FAIL with it in place (written before the run).
# `Control` is provided by controls.py. No control reaches the network.
GATE = "src/nirvana_gate.rs"
SEAT = "src/nirvana_seat.rs"
WORKER = "src/nirvana_worker.rs"
ROLE = "nirvana/claude/ROLE.md"
GATE_SKILL = "nirvana/claude/skills/gate/SKILL.md"

T_WEB = "gsrc_the_web_is_asked_by_the_class_a_need_or_the_order_text"
T_NOTE = "gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note"
T_BRIEF = "gsrc_a_send_back_of_a_no_web_order_wants_no_fetch"
T_SEAT = "gsrc_seat_run_asks_sources_only_of_an_order_that_asks_for_the_web"
T_KEEP = "gsrc_a_send_back_keeps_the_parents_effort"
T_STEP = "gsrc_a_stepped_up_redo_keeps_the_parents_effort"
T_RESEAT = "gsrc_a_rate_limited_reseat_keeps_the_parents_effort"
T_NOSEAT = "gsrc_a_seat_without_the_parents_effort_keeps_the_picks"
T_DOCS = "gsrc_the_docs_say_sources_only_when_the_order_asks_for_the_web"

WEB_CLASS_ARM = '    if class == WEB_CLASS {\n        return Some(format!("the class is {WEB_CLASS}"));\n'
NEEDS_ARM = "    if let Some(need) = needs.iter().find(|need| WEB_NEEDS.contains(&need.as_str())) {\n"
LINE_CLASS = ('    if !class.starts_with("research.") && !class.starts_with("media.") {\n        return None;\n    }\n'
              "    Some(match web_asked(class, needs, written) {\n")
WEB_OF_CLASS = '    if class == WEB_CLASS {\n        Some("the class is research.web")\n'
NO_WEB_NOTE = '        None => Axis::new(\n            "sources",\n            Mark::Note,\n'
ASKED_FAIL = '        Some(asked) => Axis::new(\n            "sources",\n            Mark::Fail,\n'
CHECK_START = "    let reads = stream_reads(stream);\n    let mut missing = Vec::new();\n"
BRIEF_IF = "    let no_web = if spec(root_text).no_web {\n"
SEAT_LINE = "        crate::nirvana_gate::sources_brief_line(&booked.class, &request.needs, &text);\n"

CONTROLS = (
    # F2: what asks for the web. The class, each need, each kind of word, the case of the text.
    Control("gsrc_web_class_ignored", GATE, WEB_CLASS_ARM, WEB_CLASS_ARM.replace("class == WEB_CLASS", "false"),
            T_WEB, "rust"),
    Control("gsrc_web_needs_ignored", GATE, NEEDS_ARM, NEEDS_ARM.replace("|need| WEB_NEEDS", "|need| false && WEB_NEEDS"),
            T_WEB, "rust"),
    Control("gsrc_web_need_web_dropped", GATE, 'const WEB_NEEDS: [&str; 3] = ["browser", "bladebro", "web"];',
            'const WEB_NEEDS: [&str; 3] = ["browser", "bladebro", "webx"];', T_WEB, "rust"),
    # A word is spoilt, not removed, so the array keeps its length and the crate builds.
    Control("gsrc_web_word_url_dropped", GATE, '    "https://",\n    "www.",\n', '    "https:/x/",\n    "www.",\n',
            T_WEB, "rust"),
    Control("gsrc_web_word_the_web_dropped", GATE, '    "the web",\n    "web search",\n',
            '    "thex web",\n    "web search",\n', T_WEB, "rust"),
    Control("gsrc_web_word_youtube_dropped", GATE, '    "youtube",\n    "internet",\n',
            '    "youtubex",\n    "internet",\n', T_WEB, "rust"),
    Control("gsrc_web_text_case_kept", GATE, "    let lower = written.to_lowercase();\n",
            "    let lower = written.to_owned();\n", T_WEB, "rust"),
    # F2: the line herdr writes, and only for research and media orders.
    Control("gsrc_no_web_gets_web_line", GATE, "        None => NO_WEB_SOURCES,\n", "        None => WEB_SOURCES,\n",
            T_WEB, "rust"),
    Control("gsrc_line_for_every_class", GATE, LINE_CLASS, LINE_CLASS.replace(
        '!class.starts_with("research.") && !class.starts_with("media.")', "false"), T_WEB, "rust"),
    # F2: the gate reads the line back; the class wins; an order with neither line keeps the old fail.
    Control("gsrc_spec_misses_no_web", GATE, "            spec.no_web = true;\n", "            spec.no_web = false;\n",
            T_NOTE, "rust"),
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
            '    if web.is_none() {\n        return Some(Axis::new("sources", Mark::Pass, "unchecked"));\n    }\n'
            + CHECK_START, T_NOTE, "rust"),
    Control("gsrc_gate_always_web", GATE, "    let web = web_of(subject.class, subject.spec);\n",
            '    let web = Some("always");\n', T_NOTE, "rust"),
    # F2: a send-back of a no-web order says no source is wanted, and only of a no-web order.
    Control("gsrc_brief_never_no_web", GATE, BRIEF_IF, "    let no_web = if false {\n", T_BRIEF, "rust"),
    Control("gsrc_brief_always_no_web", GATE, BRIEF_IF, "    let no_web = if true {\n", T_BRIEF, "rust"),
    # F2: `seat run` writes the line, read from the order as written.
    Control("gsrc_seat_reads_no_text", WORKER, SEAT_LINE, SEAT_LINE.replace("&text);", '"https://");'),
            T_SEAT, "rust"),
    Control("gsrc_seat_writes_no_line", WORKER, "    if let Some(line) = sources_line {\n",
            "    if let Some(line) = sources_line.filter(|_| false) {\n", T_SEAT, "rust"),
    # F3: each re-pick keeps the parent's effort: a send-back (its step-up too) and a rate-limit reseat.
    Control("gsrc_send_back_drops_effort", GATE, "    nirvana_seat::keep_effort(table, &mut pick, seated.effort);\n",
            "", T_KEEP, "rust"),
    Control("gsrc_step_up_drops_effort", GATE, "    nirvana_seat::keep_effort(table, &mut pick, seated.effort);\n",
            "", T_STEP, "rust"),
    Control("gsrc_reseat_drops_effort", WORKER,
            "            nirvana_seat::keep_effort(&table, &mut pick, seated.effort);\n", "", T_RESEAT, "rust"),
    Control("gsrc_keep_sets_no_effort", SEAT, "    pick.effort = effort;\n    pick.model = seat.model_for(effort);\n",
            "    pick.model = seat.model_for(effort);\n", T_KEEP, "rust"),
    Control("gsrc_keep_sets_no_model", SEAT, "    pick.effort = effort;\n    pick.model = seat.model_for(effort);\n",
            # T_RESEAT: its child is on agy, whose model id names the effort; research.read's re-pick is a claude seat.
            "    pick.effort = effort;\n", T_RESEAT, "rust"),
    Control("gsrc_keep_says_nothing", SEAT, '        "{}; effort {} kept from the parent",\n',
            '        "{}; effort {}",\n', T_KEEP, "rust"),
    Control("gsrc_keep_ignores_offer", SEAT, "    if !seat.efforts.contains(&effort) {\n", "    if false {\n",
            T_NOSEAT, "rust"),
    # The docs.
    Control("gsrc_role_rule_gone", ROLE, "herdr asks an order for web sources only when it asks for the web",
            "herdr asks every research order for web sources", T_DOCS, "rust"),
    Control("gsrc_gate_skill_fails_nothing", GATE_SKILL, "citing nothing is a note", "citing nothing fails",
            T_DOCS, "rust"),
    Control("gsrc_gate_table_every_order", GATE_SKILL, "one that asks for the web cites at least one",
            "each cites at least one", T_DOCS, "rust"),
)
