# Plan 36 P36: the Learning square and its panel (package `p36_learning`,
# nirvana/plans/2026-10-31-36-records/p36.md). Controls starting with p36_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
LEARN = "src/nirvana_learning.rs"
SQUARE = "src/client/shell/nirvana_learning.rs"
MODS = "src/client/shell/nirvana_mods.rs"
COMPOSE = "src/client/shell/composition.rs"
INPUT = "src/client/shell/input.rs"
MOUSE = "src/client/shell/mouse.rs"
KNOW = "src/nirvana_knowledge.rs"

T_VIEW = "p36_the_view_reads_lessons_ledgers_profiles_today_and_waiting_from_a_fixture_project"
T_MARK = "p36_the_mark_follows_the_last_review_gated_today"
T_STATS = "p36_the_counters_agree_with_herdr_lessons_stats"
T_UNDO = "p36_undo_rolls_back_exactly_that_row_as_the_cli_does"
T_NOWRITE = "p36_reading_the_view_writes_nothing"
T_STAMP = "p36_the_stamp_changes_with_the_files_the_view_is_made_from"
T_CACHE = "p36_the_cache_reads_on_a_thread_of_its_own"
T_REASON = "p36_refusal_reasons_count_as_one_whatever_their_numbers"
T_CAP = "p36_recent_changes_stop_at_the_newest_twelve"
T_PLACE = "p36_the_learning_square_sits_after_preload_and_before_memory_and_git"
T_VALUE = "p36_the_square_reads_lessons_and_reviews_against_the_cap_wide_and_narrow"
T_PANEL = "p36_the_panel_shows_every_section_from_a_fixture_project"
T_U = "p36_u_asks_first_and_then_rolls_back_exactly_that_row"
T_OPEN = "p36_enter_opens_the_lesson_or_card_in_the_edit_tab"
T_DRAW = "p36_drawing_reads_no_file_and_browsing_writes_nothing"

CONTROLS = (
    # §1.2 Lessons: `live` as the offer reads it, live first.
    Control("p36_live_ignored", LEARN,
            "            live: nirvana_lessons::live(&entry.counters),\n", "            live: true,\n", T_VIEW, "rust"),
    Control("p36_live_ignored_stats", LEARN,
            "            live: nirvana_lessons::live(&entry.counters),\n", "            live: true,\n", T_STATS, "rust"),
    Control("p36_live_not_first", LEARN,
            "    lessons.sort_by(|a, b| b.live.cmp(&a.live).then_with(|| a.slug.cmp(&b.slug)));\n",
            "    lessons.sort_by(|a, b| a.slug.cmp(&b.slug));\n", T_VIEW, "rust"),
    # §1.2 Recent changes: both ledgers, newest first, the undone marked, at most CHANGES_SHOWN.
    Control("p36_changes_oldest_first", LEARN, "        b.at.cmp(&a.at)\n", "        a.at.cmp(&b.at)\n", T_VIEW, "rust"),
    Control("p36_cards_ledger_unread", LEARN,
            "        let rows = knowledge::ledger(dir);\n", "        let rows: Vec<knowledge::Row> = Vec::new();\n",
            T_VIEW, "rust"),
    Control("p36_undone_unmarked", LEARN,
            "            undone: lessons.iter().any(|r| r.of.as_deref() == Some(&row.id)),\n",
            "            undone: false,\n", T_VIEW, "rust"),
    Control("p36_undone_refusal_dropped", LEARN,
            "        } else if self.undone {\n", "        } else if false {\n", T_VIEW, "rust"),
    Control("p36_changes_uncapped", LEARN,
            "    all.truncate(CHANGES_SHOWN);\n", "    all.truncate(usize::MAX);\n", T_CAP, "rust"),
    # §1.2 Profiles: cards, the ones herdr does not know (nirvana_knowledge::known, made pub(crate)), warnings, refused.
    Control("p36_unknown_uncounted", LEARN,
            "                        if !knowledge::known(&index, &rows, &slug, &sha) {\n",
            "                        if false {\n", T_VIEW, "rust"),
    Control("p36_known_always", KNOW,
            "    adopted || ledgered\n", "    adopted || ledgered || true\n", T_VIEW, "rust"),
    Control("p36_warning_uncounted", LEARN,
            "                        if card.verdict.warns() {\n", "                        if false {\n", T_VIEW, "rust"),
    Control("p36_refused_uncounted", LEARN,
            "                    Err(_) => row.refused += 1,\n", "                    Err(_) => {}\n", T_VIEW, "rust"),
    # §1.2 Today: reviews of class lesson-review seated today, against the cap the table holds.
    Control("p36_today_any_class", LEARN,
            "                Kind::Seated(s) if s.class == review::CLASS && day_of(event.at) == day => {\n",
            "                Kind::Seated(s) if day_of(event.at) == day => {\n", T_VIEW, "rust"),
    Control("p36_today_any_day", LEARN,
            "                Kind::Seated(s) if s.class == review::CLASS && day_of(event.at) == day => {\n",
            "                Kind::Seated(s) if s.class == review::CLASS => {\n", T_VIEW, "rust"),
    Control("p36_today_journal_skipped", LEARN,
            "        if day_of(written) < day {\n", "        if day_of(written) <= day {\n", T_VIEW, "rust"),
    Control("p36_held_read_as_failed", LEARN,
            "        .any(|a| a.mark == Mark::Fail && HELD_AXES.contains(&a.axis.as_str()));\n",
            "        .any(|_| false);\n", T_VIEW, "rust"),
    Control("p36_nothing_kept_counted", LEARN,
            "            Mark::Pass if axis.why != NOTHING_KEPT => applied += 1,\n",
            "            Mark::Pass => applied += 1,\n", T_VIEW, "rust"),
    Control("p36_reasons_whole_line", LEARN,
            '                    .split_once("refused: ")\n', '                    .split_once("refused:: ")\n',
            T_VIEW, "rust"),
    Control("p36_reason_numbers_kept", LEARN,
            "        if c.is_ascii_digit() {\n", "        if false && c.is_ascii_digit() {\n", T_REASON, "rust"),
    Control("p36_cap_not_read", LEARN,
            "    review::per_day(seat::parse(&text).ok().as_ref())\n", "    review::per_day(None)\n", T_VIEW, "rust"),
    # §1.1 the mark: the last review gated today held back, failed, or every op refused.
    Control("p36_mark_on_any_refusal", LEARN,
            "                    || (r.refused > 0 && r.applied == 0)\n", "                    || (r.refused > 0)\n",
            T_MARK, "rust"),
    Control("p36_mark_ignores_held", LEARN,
            "                matches!(r.ruled, Ruled::HeldBack | Ruled::Failed)\n",
            "                matches!(r.ruled, Ruled::Failed)\n", T_MARK, "rust"),
    Control("p36_mark_first_review", LEARN,
            "            .max_by_key(|r| r.gated_at)\n", "            .min_by_key(|r| r.gated_at)\n", T_MARK, "rust"),
    # §1.3 undo: the rollback the CLI runs, of exactly that row, in its own ledger.
    Control("p36_undo_other_row", LEARN,
            "        Source::Lessons => nirvana_lessons::rollback(root, &change.id, at),\n",
            '        Source::Lessons => nirvana_lessons::rollback(root, "op-1", at),\n', T_UNDO, "rust"),
    Control("p36_undo_card_in_lessons", LEARN,
            "        Source::Profile(name) => knowledge::rollback(root, name, &change.id, at),\n",
            "        Source::Profile(_) => nirvana_lessons::rollback(root, &change.id, at),\n", T_UNDO, "rust"),
    # Nothing written: the cap read never goes through `read_or_create`.
    Control("p36_cap_writes_table", LEARN,
            "    let text = std::fs::read_to_string(seat::dir().join(seat::SEATS_FILE))\n"
            "        .unwrap_or_else(|_| seat::DEFAULT.to_owned());\n",
            "    let text = seat::read_or_create(&seat::dir().join(seat::SEATS_FILE));\n", T_NOWRITE, "rust"),
    Control("p36_browse_writes_table", LEARN,
            "    let text = std::fs::read_to_string(seat::dir().join(seat::SEATS_FILE))\n"
            "        .unwrap_or_else(|_| seat::DEFAULT.to_owned());\n",
            "    let text = seat::read_or_create(&seat::dir().join(seat::SEATS_FILE));\n", T_DRAW, "rust"),
    # §2 the stamp: index, cards, journals, the day.
    Control("p36_stamp_no_index", LEARN,
            "        lessons.join(nirvana_lessons::INDEX),\n", '        lessons.join("none"),\n', T_STAMP, "rust"),
    Control("p36_stamp_no_cards", LEARN,
            "        paths.extend(listed(&knowledge));\n", "", T_STAMP, "rust"),
    Control("p36_stamp_no_journals", LEARN,
            "        paths.push(run.join(crate::nirvana_run::JOURNAL));\n", "", T_STAMP, "rust"),
    Control("p36_stamp_no_day", LEARN, "        day: day_of(now),\n", "        day: 0,\n", T_STAMP, "rust"),
    # §2 the cache: checked again every REFRESH, read again on a changed stamp, never on the drawing thread.
    Control("p36_cache_never_rechecks", LEARN,
            "    let due = entry.checked_at.is_none_or(|at| at.elapsed() >= REFRESH);\n",
            "    let due = entry.checked_at.is_none();\n", T_CACHE, "rust"),
    Control("p36_cache_never_rereads", LEARN,
            "    let view = (known.as_ref() != Some(&stamp)).then(|| load(folder, now));\n",
            "    let view = known.is_none().then(|| load(folder, now));\n", T_CACHE, "rust"),
    Control("p36_strip_reads_on_draw", COMPOSE,
            "                    .and_then(|tab| crate::nirvana_learning::cached(&tab.folder));\n",
            "                    .map(|tab| crate::nirvana_learning::load(&tab.folder, crate::nirvana_run::now()));\n",
            T_DRAW, "rust"),
    # §1.1 the square: after Preload, before Memory and Git; its words, wide and narrow; the mark.
    Control("p36_learning_gap", MODS,
            "        x = hits.preload.right() + 1;\n        let narrow = area.width < super::nirvana_learning::NARROW_STRIP;\n",
            "        x = hits.preload.right() + 3;\n        let narrow = area.width < super::nirvana_learning::NARROW_STRIP;\n",
            T_PLACE, "rust"),
    Control("p36_git_ignores_learning", MODS,
            "    let before = if hits.learning.width > 0 {\n", "    let before = if false {\n", T_PLACE, "rust"),
    Control("p36_value_no_cap", SQUARE,
            '            format!("{value} \\u{b7} review {used}/{}", today.cap)\n',
            '            format!("{value} \\u{b7} {used} reviews")\n', T_VALUE, "rust"),
    Control("p36_value_narrow_wide", SQUARE,
            '            format!("{value} \\u{b7} {used}/{}", today.cap)\n',
            '            format!("{value} \\u{b7} review {used}/{}", today.cap)\n', T_VALUE, "rust"),
    Control("p36_value_no_lessons_words", SQUARE,
            '        if narrow { "none yet" } else { "no lessons yet" }.to_owned()\n',
            '        if narrow { "none yet" } else { "0 lessons" }.to_owned()\n', T_VALUE, "rust"),
    Control("p36_value_no_mark", SQUARE,
            "    if today.last_went_wrong() {\n        value =", "    if false {\n        value =", T_VALUE, "rust"),
    Control("p36_value_counts_all", SQUARE,
            "            view.live,\n            if view.live == 1", "            view.lessons.len(),\n            if view.live == 1",
            T_VALUE, "rust"),
    # §1.2 the panel's sections.
    Control("p36_today_held_as_done", SQUARE,
            "            today.count(Ruled::HeldBack),\n", "            today.count(Ruled::Done),\n", T_PANEL, "rust"),
    Control("p36_undone_not_said", SQUARE,
            '                if change.undone { " \\u{b7} undone" } else { "" }\n', '                ""\n',
            T_PANEL, "rust"),
    Control("p36_stats_not_shown", SQUARE,
            "    for stat in &view.stats {\n", "    for stat in view.stats.iter().take(0) {\n", T_PANEL, "rust"),
    Control("p36_adopt_line_missing", SQUARE,
            "        if profile.unknown > 0 {\n            out.push(Line::Text(format!(\n",
            "        if false {\n            out.push(Line::Text(format!(\n", T_PANEL, "rust"),
    Control("p36_waiting_not_shown", SQUARE,
            "    for (class, profile, count) in &view.waiting {\n",
            "    for (class, profile, count) in view.waiting.iter().take(0) {\n", T_PANEL, "rust"),
    Control("p36_square_opens_git", MOUSE,
            "                if super::contains(self.hits.nirvana_learning, point) {\n                    self.toggle_learning_panel();\n",
            "                if super::contains(self.hits.nirvana_learning, point) {\n                    self.toggle_git_panel();\n",
            T_PANEL, "rust"),
    Control("p36_open_panel_unnamed", MODS,
            "            OpenPanel::Learning\n        } else {\n", "            OpenPanel::None\n        } else {\n",
            T_PANEL, "rust"),
    # §1.3 the keys: u asks, n keeps, y rolls back that row; a rollback row is refused before asking.
    Control("p36_n_undoes", SQUARE,
            "                KeyCode::Char('n') | KeyCode::Esc => panel.confirming = None,\n",
            "                KeyCode::Char('n') | KeyCode::Esc => self.learning_confirm_undo(),\n", T_U, "rust"),
    Control("p36_u_runs_at_once", SQUARE,
            "            panel.confirming = Some(change);\n",
            "            panel.confirming = Some(change);\n            self.learning_confirm_undo();\n            return;\n",
            T_U, "rust"),
    Control("p36_rollback_row_asked", SQUARE,
            "        if let Some(why) = change.refusal() {\n            return self.learning_say(why);\n        }\n", "",
            T_U, "rust"),
    Control("p36_keys_unrouted", INPUT,
            "                self.route_learning_key(key, outcome);\n", "                self.route_git_key(key, outcome);\n",
            T_U, "rust"),
    Control("p36_panel_hits_dropped", COMPOSE,
            "                self.hits.nirvana_learning_panel = rendered;\n", "", T_U, "rust"),
    # §1.3 Enter or o opens a lesson or card in the edit tab, a folder profile's knowledge/ in an edit tab.
    Control("p36_open_first_lesson", SQUARE,
            "                let file = view.lesson_file(&view.lessons[i]);\n",
            "                let file = view.lesson_file(&view.lessons[0]);\n", T_OPEN, "rust"),
    Control("p36_card_file_wrong", LEARN,
            '            .map(|slug| dir.join(format!("{slug}.md")))\n',
            '            .map(|slug| dir.join(format!("{slug}.txt")))\n', T_OPEN, "rust"),
    Control("p36_profile_opens_file", SQUARE,
            '                        format!("{}{}", crate::nirvana::EDIT_DIR_COMMAND, dir.display())\n',
            '                        format!("{}{}", crate::nirvana::EDIT_FILE_COMMAND, dir.display())\n', T_OPEN, "rust"),
)
