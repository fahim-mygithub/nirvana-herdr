# Plan 33 P33b: lessons that are kept, reviewed and merged (nirvana/plans/2026-10-30-33-profiles-and-learning.md §3,
# record nirvana/plans/2026-10-30-33-records/p33b.md). Controls starting with p33b_. One sabotage per assertion, each
# on a product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
LESSONS = "src/nirvana_lessons.rs"
REVIEW = "src/nirvana_review.rs"
GATE = "src/nirvana_gate.rs"
RETRO = "src/nirvana_retro.rs"
SEATS = "src/nirvana_seats.toml"

T_PASS = "p33_a_review_is_queued_after_a_pass_and_its_new_op_keeps_a_lesson"
T_SENT = "p33_a_review_is_queued_after_a_send_back_with_its_reasons"
T_CAP = "p33_over_the_daily_cap_the_review_is_skipped_and_journaled"
T_PER_DAY = "p33_the_cap_reads_lesson_reviews_per_day_from_constants"
T_SEAT = "p33_the_review_seats_flash_first_then_sonnet_low_at_efficient"
T_OPS = "p33_each_lesson_op_refusal_says_why"
T_MERGE = "p33_a_crowded_class_is_merged_archived_ledgered_and_rolled_back"
T_END = "p33_run_end_queues_the_merge_of_a_crowded_class_and_the_receipt_prints_origins"
T_NO_REMOTE = "p33_a_lesson_is_written_after_a_pass_in_a_project_with_no_remote"
T_OFFER = "p33_offer_gives_the_class_lessons_with_no_profile_and_the_profiles_own"
T_STATS = "p33_stats_count_per_origin"
T_IMPORT = "p33_import_keeps_only_gated_pass_lessons_and_the_dry_run_writes_nothing"
T_PROFILE = "p33_the_order_copy_names_its_profile_at_a_lines_start"

CONTROLS = (
    # §3.1: a lesson is written with no remote. The sabotage puts the old condition back: a remote must exist.
    Control("p33b_write_needs_remote", LESSONS,
            "    let lesson = match check(lesson, passed, stream) {\n",
            "    let lesson = match check(lesson, passed && std::process::Command::new(\"git\").args([\"remote\"])"
            ".current_dir(root).output().is_ok_and(|o| !o.stdout.is_empty()), stream) {\n",
            T_NO_REMOTE, "rust"),
    # O5's lesson checks that stand, moved here from o5.py: their find texts or tests changed with §3.1 (the O5
    # controls `lessons_written_without_remote` and `lessons_any_remote` retire with the remote rule).
    Control("p33b_written_without_pass", LESSONS,
            "    if !passed {\n", "    if false {\n", T_NO_REMOTE, "rust"),
    Control("p33b_quote_unverified", LESSONS,
            "    if !quote_in_stream(stream, &lesson.evidence) {\n", "    if false {\n", T_NO_REMOTE, "rust"),
    Control("p33b_long_steps", LESSONS,
            "        if lesson.steps.lines().count() > STEPS_LINES {\n", "        if false {\n",
            "steps_over_120_lines_are_refused", "rust"),
    # §3.2: the offer is class C with no profile, plus profile P's own.
    Control("p33b_offer_any_profile", LESSONS,
            "        Some(own) => profile == Some(own),\n", "        Some(_) => true,\n", T_OFFER, "rust"),
    Control("p33b_offer_any_class", LESSONS,
            "        None => entry.class == class,\n", "        None => true,\n", T_OFFER, "rust"),
    Control("p33b_profile_line_unread", GATE,
            'line.strip_prefix("Profile:")', 'line.strip_prefix("profile:")', T_PROFILE, "rust"),
    Control("p33b_profile_name_unchecked", LESSONS,
            ".all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')",
            ".all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b' ')", T_PROFILE, "rust"),
    Control("p33b_worker_lesson_no_profile", GATE,
            "                    profile: profile.as_deref(),\n                    origin: crate::nirvana_lessons::Origin::Worker,",
            "                    profile: None,\n                    origin: crate::nirvana_lessons::Origin::Worker,",
            T_PASS, "rust"),
    # §3.3: a review after a pass and after a send-back; never of a review; in the project's folder, at Efficient.
    Control("p33b_no_review_after_pass", GATE,
            "        if review.is_ok() {\n", "        if review.is_ok() && false {\n", T_PASS, "rust"),
    Control("p33b_no_review_after_send_back", GATE,
            "    if review.is_ok() && ruling.verdict == Outcome::SentBack.word() {\n",
            "    if false && ruling.verdict == Outcome::SentBack.word() {\n", T_SENT, "rust"),
    Control("p33b_review_gated_as_plain", GATE,
            "            if kind == crate::nirvana_review::REVIEW || kind == crate::nirvana_review::MERGE =>",
            "            if false =>", T_PASS, "rust"),
    Control("p33b_review_reviews_review", REVIEW,
            "    if known.seated.class == CLASS {\n", "    if known.seated.class == \"never\" {\n", T_CAP, "rust"),
    Control("p33b_review_in_worktree_cwd", REVIEW,
            "        cwd: project.root.display().to_string(),\n",
            "        cwd: std::env::temp_dir().display().to_string(),\n", T_PASS, "rust"),
    Control("p33b_review_seated_balanced", REVIEW,
            "        mode: Mode::Efficient,\n", "        mode: Mode::Balanced,\n", T_PASS, "rust"),
    Control("p33b_review_flash_dropped", SEATS,
            '  { seat = "flash", score = 80 },\n  { seat = "sonnet", score = 78 },\n]',
            '  { seat = "sonnet", score = 78 },\n]', T_SEAT, "rust"),
    Control("p33b_review_effort_medium", SEATS,
            'effort = "low"\nefficient_effort = "low"\nsota_effort = "low"',
            'effort = "low"\nefficient_effort = "medium"\nsota_effort = "low"', T_SEAT, "rust"),
    # The brief: the stream named, never held; the send-back reasons; the reviewer's own lesson never kept.
    Control("p33b_brief_no_stream_path", REVIEW,
            "        subject.stream.display(),\n", "        \"the stream\",\n", T_PASS, "rust"),
    Control("p33b_brief_no_reasons", REVIEW,
            '                text.push_str(&format!("- {reason}\\n"));\n',
            '                text.push_str(&format!("- {}\\n", reason.len()));\n', T_SENT, "rust"),
    Control("p33b_reviewer_lesson_kept", REVIEW,
            '        if report.get("lesson").is_some() {\n',
            '        if let Some(l) = report.get("lesson") {\n            let _ = nirvana_lessons::verify_and_write('
            '&project.root, l, true, &nirvana_gate::stream_text(project, run, subject), &order, run, 0, '
            'nirvana_lessons::Meta::default());\n',
            T_PASS, "rust"),
    # The daily cap, journaled as `review_skipped`.
    Control("p33b_cap_ignored", REVIEW,
            "    (used >= cap).then(|| {\n", "    (used > cap + 1000).then(|| {\n", T_CAP, "rust"),
    Control("p33b_cap_never_resets", REVIEW,
            "    let used = if state.day == at / 86_400 {\n", "    let used = if true {\n", T_CAP, "rust"),
    Control("p33b_skip_not_journaled", GATE,
            "                crate::nirvana_review::SKIPPED,\n                Mark::Note,\n",
            "                \"other\",\n                Mark::Note,\n", T_CAP, "rust"),
    Control("p33b_per_day_key", REVIEW,
            '.get("lesson_reviews_per_day")?', '.get("lesson_reviews")?', T_PER_DAY, "rust"),
    # Each lesson_ops refusal.
    Control("p33b_evidence_unchecked", REVIEW,
            "            if !nirvana_lessons::quote_in_stream(stream, evidence) {\n",
            "            if false {\n", T_OPS, "rust"),
    Control("p33b_base_sha_unchecked", LESSONS,
            "        if now_sha != base_sha {\n", "        if false {\n", T_OPS, "rust"),
    Control("p33b_unknown_op_accepted", REVIEW,
            '                Some(other) => Err(format!("unknown op {other:?}")),\n',
            "                Some(_) => Lesson::from_value(item).map(Op::New),\n", T_OPS, "rust"),
    Control("p33b_new_over_an_existing", LESSONS,
            "            if index.lessons.contains_key(&slug) || lesson_path(root, &slug).exists() {\n",
            "            if false {\n", T_OPS, "rust"),
    # §3.4: merge archive, ledger, rollback, summed counters; a merge order merges only what it was given.
    Control("p33b_merge_no_archive", LESSONS,
            'archive.join(format!("{id}-{part}.md"))', 'archive.join("one.md")', T_MERGE, "rust"),
    Control("p33b_merge_counters_not_summed", LESSONS,
            "            counters.offered += entry.counters.offered;\n",
            "            counters.offered = entry.counters.offered;\n", T_MERGE, "rust"),
    Control("p33b_ledger_op_word", LESSONS,
            "            op: op.to_owned(),\n", "            op: \"change\".to_owned(),\n", T_MERGE, "rust"),
    Control("p33b_ledger_after_is_before", LESSONS,
            "        .map(|was| snap(root, &index, &was.slug))\n", "        .map(Clone::clone)\n", T_MERGE, "rust"),
    Control("p33b_rollback_restores_nothing", LESSONS,
            "            Some(text) => std::fs::write(&path, text).map_err(|err| err.to_string())?,\n",
            "            Some(_) => {}\n", T_MERGE, "rust"),
    Control("p33b_rollback_twice", LESSONS,
            "    if rows.iter().any(|r| r.of.as_deref() == Some(id)) {\n", "    if false {\n", T_MERGE, "rust"),
    Control("p33b_merge_foreign_slug", REVIEW,
            "            if let Some(slug) = slugs.iter().find(|slug| !shas.contains_key(*slug)) {\n",
            "            if let Some(slug) = slugs.iter().find(|_| false) {\n", T_MERGE, "rust"),
    Control("p33b_merge_applied_as_review", REVIEW,
            "        } else if kind == MERGE {\n", "        } else if kind == \"never\" {\n", T_MERGE, "rust"),
    Control("p33b_merge_pending_twice", REVIEW,
            "        .any(|p| p.kind == MERGE && p.subject == subject)\n", "        .any(|_| false)\n",
            T_MERGE, "rust"),
    Control("p33b_settle_skips_delivered", REVIEW,
            "            Some(known) if known.phase == Phase::Delivered => {\n",
            "            Some(known) if known.phase == Phase::Merged => {\n", T_MERGE, "rust"),
    Control("p33b_crowded_at_nine", REVIEW,
            "        .filter(|(_, count)| *count > MERGE_ABOVE)\n",
            "        .filter(|(_, count)| *count > MERGE_ABOVE + 1)\n", T_MERGE, "rust"),
    Control("p33b_run_end_no_merge", RETRO,
            "        .chain(crate::nirvana_review::merge_crowded(project, run))\n",
            "        .chain(Vec::<String>::new())\n", T_END, "rust"),
    # §3.5: per origin, on the receipt and in stats; an old entry is a worker's.
    Control("p33b_receipt_no_origins", RETRO,
            "    text.push_str(&crate::nirvana_lessons::stats_section(&project.root));\n", "", T_END, "rust"),
    Control("p33b_stats_all_origins", LESSONS,
            ".filter(|e| e.origin == *origin)", ".filter(|_| true)", T_STATS, "rust"),
    Control("p33b_old_entry_not_worker", LESSONS,
            "    #[default]\n    Worker,\n    Review,\n", "    Worker,\n    #[default]\n    Review,\n", T_STATS, "rust"),
    # §3.6: import on a fixture: only gated pass (and the pre-O5 `accepted`), the gate's checks, a dry run that
    # writes nothing.
    Control("p33b_import_any_verdict", LESSONS,
            "            let passed = PASS_WORDS.contains(&verdict.as_str());\n",
            "            let passed = !verdict.is_empty();\n", T_IMPORT, "rust"),
    Control("p33b_import_no_accepted", LESSONS,
            'pub const PASS_WORDS: [&str; 2] = ["pass", "accepted"];',
            'pub const PASS_WORDS: [&str; 2] = ["pass", "pass"];', T_IMPORT, "rust"),
    Control("p33b_import_dry_writes", LESSONS,
            "            let result = if dry {\n", "            let result = if false {\n", T_IMPORT, "rust"),
    Control("p33b_import_skips_quote", LESSONS,
            "    if !quote_in_stream(stream, &lesson.evidence) {\n", "    if false {\n", T_IMPORT, "rust"),
)
