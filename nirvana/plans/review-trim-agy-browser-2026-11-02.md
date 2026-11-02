# A lesson review only when there is something to learn; agy seats take browser orders, 2026-11-02

Branch `review-trim-agy-browser` from master `871acaf2` (alpha 0.52.0). No version bump and no `nirvana/CHANGELOG.md`
entry here: the merge does that. The evidence is `nirvana/plans/live-checks-2026-11-01.md` (F1, F5, F8, check 35.1)
and plan 35 §4 R2. The owner, 2026-11-02, on the recommendations: "Use your recomendation for all of these".

## F8: the lesson review

### The rule

A review is wanted when there is something to learn:

- the verdict is not a pass (a send-back; the gate queues no review after an `asked`, as plan 33 §3.3 says, and this
  does not change that);
- the order's chain holds a correction: a `send_back` parent anywhere above it, or a parent gated other than pass,
  walking up through `reseat` parents too (`nirvana_gate::chain` stops at a reseat, so a reseat child of a redo would
  otherwise read as a first pass);
- a question in the order's chain, or the run's question that started it (review round S3): any order of the same
  chain root asked (a gated `asked`, the 0.52.0 blocked path included, or a worker's question mid-order), or the
  chain root was the first fresh order seated after an open question;
- the Work Report names any of `gotchas`, `lesson`, `lesson_ops`, `card_ops`, with something said: a list with a
  non-blank item, a non-empty object, a non-blank string (a blank gotcha `[""]` is no signal, review round nit).

A clean first-pass delivery with none of these is skipped, said as `nothing to learn: a first-pass delivery whose
report names no gotchas, lesson, lesson_ops or card_ops`. The skip is decided before the cap, so it is never queued,
never counted against `lesson_reviews_per_day`, and the Learning square's `review n/cap` (`used_today`) does not move.
A cap of 0 is said first, as `lesson reviews are off (lesson_reviews_per_day = 0)`, whatever the delivery.

`lesson` and `card_ops` are in the list though the work order names only "gotchas or lesson ops". A worker's own
`lesson` and a profiled worker's `card_ops` are the worker saying it learned something, and the review brief offers
the reviewer that lesson to patch into an existing one. On the journals below the two extra keys change no row: every
report that offered a `lesson` also named gotchas. This is a reading of the rule, reported, not a change to it.

### Where it lives, and the merge-time change set

The decision is `nirvana_review::idle`, called from `wanted_with`. `wanted` itself cannot hold it: the gate calls
`wanted(project, state, known, table)` before it journals the verdict, in both the pass path and the send-back path,
and nothing in `known` or `state` says which path it is in (the verdict is not journaled yet, the phase is `Delivered`
in both). So, on this branch:

- `wanted` keeps its signature and its old meaning (the verdict unknown: every review the cap allows), so this branch
  builds while `src/nirvana_gate.rs` is order G's.
- `wanted_for(project, state, known, verdict, table)` is the decision with the verdict.
- `queue_review` asks `wanted_for` itself as a backstop: a clean first pass seats no review and counts nothing, and its
  line in the ruling's `next` (`lesson review not queued: nothing to learn: ...`) is the only word of it; the `gated`
  event holds no `review_skipped` axis.

The merge applies two changes, in this order. Every find below occurs exactly once; each block's text ends with a
newline.

**(a), by hand, `src/nirvana_gate.rs` (order G's file).** The gate's two calls move to `wanted_for`, so the skip is
journaled in the `gated` event as the `review_skipped` axis. The finds are only the lines around the
`nirvana_review::wanted` calls, so order G's changes to the gate (sources axis, send-back brief, `keep_effort`) do not
touch them: both finds occur once in order G's working `src/nirvana_gate.rs` as read on 2026-11-02 (`apply_a.py
--check`). The second find starts at the `.collect();` line, because its call line alone is a substring of the first
call's line (8 spaces of indent against 4). The send-back verdict clones `reasons`, which the send-back path uses
again below. Applied as find/replace, this gives the same diff as the saved `lrs-gate.hunk`, byte for byte.

`src/nirvana_gate.rs`, find:

~~~rust
        // Plan 33 §3.3: the cap's refusal is journaled with the verdict, as an axis named `review_skipped`.
        let review = crate::nirvana_review::wanted(project, state, known, table);
~~~

replace:

~~~rust
        // Plan 33 §3.3: the cap's refusal is journaled with the verdict, as an axis named `review_skipped`; so is a
        // clean first pass, which has nothing to learn (review-trim-agy-browser 2026-11-02, live checks F8).
        let review = crate::nirvana_review::wanted_for(
            project,
            state,
            known,
            &crate::nirvana_review::Verdict::Pass,
            table,
        );
~~~

`src/nirvana_gate.rs`, find:

~~~rust
        .collect();
    let review = crate::nirvana_review::wanted(project, state, known, table);
~~~

replace:

~~~rust
        .collect();
    let review = crate::nirvana_review::wanted_for(
        project,
        state,
        known,
        &crate::nirvana_review::Verdict::SentBack(reasons.clone()),
        table,
    );
~~~

**(b), run `python lrs-merge-own.py <merge worktree root>`, after (a), in the same merge commit.** The script holds
the 18 find/replace pairs below and writes nothing unless every find occurs once. With (a) in, `wanted` has no caller
and the backstop can never run, since the gate calls `queue_review` only after `wanted_for` said yes. So (b):

- deletes `wanted` (b1) and `queue_review`'s backstop (b5, b6), and gives `wanted_with` the verdict itself,
  `&Verdict` and not `Option` (b2-b4), because no caller is left that does not know it;
- moves the tests to the new signature and drops the "verdict unknown" assertion (b7-b13). The gate test asserts the
  `review_skipped` axis only (b13, b14): `journaled`, where before it accepted the axis or `queue_review`'s line;
- repoints `lrs_skip_before_cap` at the new line (b16). It replaces `lrs_queue_review_backstop`, which is hollow after
  (a) (review S2), with `lrs_gate_pass_asks_wanted_for` (b15, b17). That one swaps the pass path's
  `Verdict::Pass` for `Verdict::SentBack(Vec::new())`; the expected red test is
  `lrs_the_gate_seats_no_review_of_a_clean_first_pass_and_counts_none`, because `c1-lr` is seated. The script also
  adds `lrs_gate_send_back_asks_wanted_for` (b18), which swaps the send-back path's `Verdict::SentBack(reasons.clone())`
  for `Verdict::Pass`; its expected red test is `p33_a_review_is_queued_after_a_send_back_with_its_reasons`, because
  that clean first order, sent back, would read as a skip.

The whole set was tested in a throwaway worktree (`git worktree add --detach` at `dd7a3963` with this round's files
copied in, removed afterwards). (a) went in as the find/replace pairs above, then the script's edits and `cargo fmt`.
The results: `cargo fmt --check` clean; `cargo nextest run --locked --no-fail-fast` gave 4617 of 4618 passed, the one
failure the known `windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`; `cargo clippy
--all-targets --locked -- -D warnings` clean; no `nirvana_review::wanted(` left in `src`;
`lrs_gate_pass_asks_wanted_for`, `lrs_gate_send_back_asks_wanted_for` and the repointed `lrs_skip_before_cap` each
reddened their test, run alone. The script was generated from the throwaway's files. Run on a copy of this branch's
files, it gives them byte for byte, and a second run refuses (its first find is gone).

(b1) `src/nirvana_review.rs`, find:

~~~rust
/// Whether the gate queues a review of `known` now, not knowing its verdict: every review the cap allows, as before
/// the trim. The gate's two calls move to `wanted_for` (the record's hunk), and then this has no caller and goes.
/// `Err(None)`: none, and nothing to say (it is queued already). `Err(Some(why))`: skipped, journaled as
/// `review_skipped`.
pub fn wanted(
    project: &Project,
    state: &nirvana_run::RunState,
    known: &OrderState,
    table: Option<&Table>,
) -> Result<(), Option<String>> {
    wanted_with(project, state, known, None, per_day(table), now())
}

/// `wanted`, knowing the verdict the gate is about to journal: a clean first-pass delivery is skipped (`idle`).
~~~

replace:

~~~rust
/// Whether the gate queues a review of `known` now, knowing the verdict it is about to journal: a clean first-pass
/// delivery is skipped (`idle`). `Err(None)`: none, and nothing to say (it is queued already). `Err(Some(why))`:
/// skipped, journaled as `review_skipped`.
~~~

(b2) `src/nirvana_review.rs`, find:

~~~rust
    wanted_with(project, state, known, Some(verdict), per_day(table), now())
~~~

replace:

~~~rust
    wanted_with(project, state, known, verdict, per_day(table), now())
~~~

(b3) `src/nirvana_review.rs`, find:

~~~rust
    verdict: Option<&Verdict>,
~~~

replace:

~~~rust
    verdict: &Verdict,
~~~

(b4) `src/nirvana_review.rs`, find:

~~~rust
    if let Some(why) = verdict.and_then(|verdict| idle(state, known, verdict)) {
~~~

replace:

~~~rust
    if let Some(why) = idle(state, known, verdict) {
~~~

(b5) `src/nirvana_review.rs`, find:

~~~rust
    let state = replay(&journal.read().0);
    // The trim's backstop until the gate asks `wanted_for` itself: a clean first pass seats nothing and counts nothing.
    // Its `gated` event then holds no `review_skipped` axis; this line is the only word of it.
    if let Err(why) = wanted_for(project, &state, known, verdict, Some(table)) {
        return format!(
            "lesson review not queued: {}",
            why.unwrap_or_else(|| "it is queued already".into())
        );
    }
~~~

replace:

~~~rust

~~~

(b6) `src/nirvana_review.rs`, find:

~~~rust
    let profile = subject_profile(&state, order);
~~~

replace:

~~~rust
    let profile = subject_profile(&replay(&journal.read().0), order);
~~~

(b7) `src/nirvana_review/tests.rs`, find:

~~~rust
            &review,
            None,
            PER_DAY,
~~~

replace:

~~~rust
            &review,
            &Verdict::Pass,
            PER_DAY,
~~~

(b8) `src/nirvana_review/tests.rs`, find:

~~~rust
    let why = wanted_with(
        &project,
        &state,
        known,
        Some(&Verdict::Pass),
        PER_DAY,
        now(),
    )
    .expect_err("a clean first pass is skipped")
    .expect("with a reason");
~~~

replace:

~~~rust
    let why = wanted_with(&project, &state, known, &Verdict::Pass, PER_DAY, now())
        .expect_err("a clean first pass is skipped")
        .expect("with a reason");
~~~

(b9) `src/nirvana_review/tests.rs`, find:

~~~rust
        wanted_with(
            &project,
            &state,
            known,
            Some(&Verdict::Pass),
            PER_DAY,
            now()
        ),
~~~

replace:

~~~rust
        wanted_with(&project, &state, known, &Verdict::Pass, PER_DAY, now()),
~~~

(b10) `src/nirvana_review/tests.rs`, find:

~~~rust
        wanted_with(&project, &state, known, Some(&Verdict::Pass), 0, now()),
~~~

replace:

~~~rust
        wanted_with(&project, &state, known, &Verdict::Pass, 0, now()),
~~~

(b11) `src/nirvana_review/tests.rs`, find:

~~~rust
    );
    // The verdict unknown (`wanted`, the gate's call until the hunk lands): reviewed, as before the trim.
    assert_eq!(
        wanted_with(&project, &state, known, None, PER_DAY, now()),
        Ok(())
~~~

replace:

~~~rust

~~~

(b12) `src/nirvana_review/tests.rs`, find:

~~~rust
            Some(&Verdict::SentBack(vec!["owned: x".into()])),
~~~

replace:

~~~rust
            &Verdict::SentBack(vec!["owned: x".into()]),
~~~

(b13) `src/nirvana_review/tests.rs`, find:

~~~rust
    // Said once: as the `gated` event's `review_skipped` axis when the gate asks `wanted_for` (the record's hunk), else
    // as `queue_review`'s line.
~~~

replace:

~~~rust
    // Said in the `gated` event, as the `review_skipped` axis: the gate asks `wanted_for` with its verdict.
~~~

(b14) `src/nirvana_review/tests.rs`, find:

~~~rust
    let told = ruling
        .next
        .iter()
        .any(|line| line.starts_with("lesson review not queued: nothing to learn"));
    assert!(journaled != told, "{axes:?} {:?}", ruling.next);
~~~

replace:

~~~rust
    assert!(journaled, "{axes:?} {:?}", ruling.next);
~~~

(b15) `nirvana/controls.d/lrs.py`, find:

~~~python
SKILL = "nirvana/claude/skills/seat/SKILL.md"
~~~

replace:

~~~python
SKILL = "nirvana/claude/skills/seat/SKILL.md"
GATE = "src/nirvana_gate.rs"
~~~

(b16) `nirvana/controls.d/lrs.py`, find:

~~~python
            "    if let Some(why) = verdict.and_then(|verdict| idle(state, known, verdict)) {\n",
            "    if let Some(why) = verdict\n        .and_then(|verdict| idle(state, known, verdict))\n"
            "        .filter(|_| cap_refusal(&project.root, cap, at).is_none())\n    {\n",
~~~

replace:

~~~python
            "    if let Some(why) = idle(state, known, verdict) {\n",
            "    if let Some(why) =\n        idle(state, known, verdict).filter(|_| cap_refusal(&project.root, cap, at).is_none())\n    {\n",
~~~

(b17) `nirvana/controls.d/lrs.py`, find:

~~~python
    # F8: the gate's queue_review seats no review of a clean first pass until the gate asks wanted_for itself
    Control("lrs_queue_review_backstop", REVIEW,
            "    if let Err(why) = wanted_for(project, &state, known, verdict, Some(table)) {\n",
            "    if let Err(why) = Ok::<(), Option<String>>(()) {\n",
~~~

replace:

~~~python
    # F8: the gate's pass path asks wanted_for with the pass, so a clean first pass seats no review (merge-time (a))
    Control("lrs_gate_pass_asks_wanted_for", GATE,
            "            &crate::nirvana_review::Verdict::Pass,\n            table,\n        );\n",
            "            &crate::nirvana_review::Verdict::SentBack(Vec::new()),\n            table,\n        );\n",
~~~

(b18) `nirvana/controls.d/lrs.py`, find:

~~~python
            T_GATE, "rust"),
    # review S3: a question this order's chain asked keeps the review (a worker that asked mid-order)
~~~

replace:

~~~python
            T_GATE, "rust"),
    # and the send-back path with the send-back, so a clean first order sent back is still reviewed
    Control("lrs_gate_send_back_asks_wanted_for", GATE,
            "        &crate::nirvana_review::Verdict::SentBack(reasons.clone()),\n",
            "        &crate::nirvana_review::Verdict::Pass,\n",
            "p33_a_review_is_queued_after_a_send_back_with_its_reasons", "rust"),
    # review S3: a question this order's chain asked keeps the review (a worker that asked mid-order)
~~~

### Measured on real journals (read only)

`measure.py` (in the builder's private TEMP, not committed) read each `seated` of class `lesson-review` with `why`
`lesson_review`, its subject's first `pass` or `sent_back` gate, the subject's delivered report, its chain, and the
review's own `gated` axes (a `lesson_op` or `card_op` axis marked pass other than "nothing to keep" is a kept lesson).

| Journals | Reviews | Skipped by the rule | Skipped that kept a lesson (the cost) | Reviewed that kept a lesson |
|---|---|---|---|---|
| `3D-Printer-Flow-xtool\.nirvana\runs\*` (f2u-setup) | 4 | 0 | 0 | 0 |
| `3D-Printer Flow\.nirvana\runs\*` (quad-pegprep, quad-split, quad3) | 3 | 0 | 0 | 0 |
| the live checks of 2026-11-01 (`scratchpad\next\live\proj\.nirvana\runs\*`) | 10 | 4 | 0 | 0 |
| all | 17 | 4 | 0 | 0 |

Read again in the review round, with S3's question rule added (`measure2.py`, the same TEMP): the 4 skipped rows
stay skipped (none had a question in its chain or before it), and no other row moves. `3D-Printer Flow` had grown
two runs since the first reading, `spinners` (`spinner-fill-prep-lr`) and `spinprint2` (`spinner-print4-lr`), both
send-backs naming gotchas: 5 reviews there, 0 skipped, 0 kept, so 19 in all, 4 skipped, 0 kept. These figures are
one reading, with the script that made them; the next run of it is the authority, not this paragraph.

- The two printer projects: every one of the 7 subjects named gotchas (2 were send-backs), so the rule skips none
  there. The claim that the reviews there were of clean first passes does not hold; F8 was the live checks' finding.
- The live checks: the 4 skipped are `lc1-agy-blade-lr`, `lc1b-agy-blade-lr`, `lc2-sonnet-blade-lr` and
  `lc4b-agy-rules-lr`, 41,423 + 24,517 + 39,796 + 20,416 = 126,152 fresh tokens (input + output, the last `usage` of
  each) of the 10 reviews' 338,721, 37%. The redos `lc3-agy-mcp-s1` and `lc4-agy-rules-s1` passed clean but have a
  send-back above them, so they stay reviewed. `reel-motion-gemini-lr` was seated and never gated (stopped).
- None of the 17 reviews kept anything, so the cost measured here is 0 lessons; it is 17 reviews, not a bound.

## R2: agy and the browser

- `browser` is added to the four enabled agy seats, `g-opus`, `g-sonnet`, `argon` and `flash` (`gpt-oss` is off).
  `research.web` is not rescored: live check 35.1 is one order. Its `why` says so, and a comment above the agy seats.
- Who reads the capability list: `nirvana_seat::candidate` ("lacks <need>"), the unknown-need check (`browser` was a
  word already, from opus and sonnet), `nirvana_gui_tools::capabilities` / `seat_needs` / `seated_servers` (the same
  word set, so no need moves between "capability" and "server"), `seat list --json` and `nirvana_jev_roles` (print
  it). The Seats square does not read it. `nirvana_bladebro::grant` reads the needs, not the capabilities, and
  granted agy a browser before this.
- A desktop server need still skips agy: `bambu-studio-beta` and `xtool` set `claude_only` in `seat_needs`
  (`nirvana_gui_tools`, plan 29 §5.2), which `candidate` checks after the capabilities, whatever a seat lists. The
  test sets `claude_only` on a `--needs browser` order and sees flash skipped with the desktop reason.
- `seat pick --class research.web --needs browser` on the stand-in world now gives an agy seat; on plan 35c's table
  (v8) it gave a claude seat with flash "lacks browser" (F1).
- The seat skill now says `--needs browser` is the way to a browser on agy, and that a desktop need still takes a
  claude seat. Its agy read-method line names `--needs bladebro` or `--needs browser`.
- **The installed table.** `seats.toml` upgrades itself only when the installed copy equals a past default. Plan 35c
  changed the default's `why` line and added no copy, so an install that held plan 34a's default stayed there: the
  owner's (`%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\seats.toml`, read 2026-11-02) equals plan 34a's default
  byte for byte apart from line endings, and differs from master's only in that `why`. This branch adds
  `nirvana_seats_v7.toml` (plan 34a's, from `40917ee3^`) and `nirvana_seats_v8.toml` (master's, plan 35c's), so both
  upgrade to R2, backed up first. Without them R2 would not reach the owner's install.
- Review round: three more past defaults shipped in a commit with no copy, `ad601b8d` (plan 33b), `c2ad3b39` (plan
  33's join) and `b5ab56a3` (plan 34a's first commit). Their bytes come from `git show <sha>:src/nirvana_seats.toml`
  (LF, mode `balanced`, md5 matching the review's) as `nirvana_seats_v9.toml`, `_v10` and `_v11`. A sweep of every
  commit that touched `src/nirvana_seats.toml` now finds a copy for each (`hist.py`, the same TEMP).

## F5, noted, not wired

The init line of lc1 lists 59 agy tools, among them `read_url_content`, `open_browser_url`, `read_browser_page` and
`browser_*`. Plan 35 §1.2 says "14 built-ins in print mode, with `search_web` but no URL reader and no browser". Plan
35 is stale on this. Nothing here wires those tools: herdr's browser on agy stays bladebro, and the gate's sources axis
was not taught `read_url_content`.

## Review round (review of dd7a3963, `scratchpad\next\lrs-review.md`)

| Finding | What I did | Test | Control |
|---|---|---|---|
| S1: the gate hunk is not in the tree | (a) is written above as find/replace pairs; it gives the saved hunk byte for byte | throwaway worktree: nextest, clippy | `lrs_gate_pass_asks_wanted_for`, `lrs_gate_send_back_asks_wanted_for` (after (b)) |
| S2: `lrs_queue_review_backstop` is hollow after (a) | (b) deletes `wanted`, the backstop and that control, adds the two gate-call controls, and makes the gate test assert `journaled` only | `lrs_the_gate_seats_no_review_of_a_clean_first_pass_and_counts_none`, `p33_a_review_is_queued_after_a_send_back_with_its_reasons` | the two above, red alone in the throwaway |
| S3: a clean delivery after a question reads as a first pass | `OrderState.asked` and `.answers` in `replay`, and `idle` returns None when an order of the same chain root asked or answers a question | `lrs_a_question_in_the_chain_or_the_one_that_started_it_keeps_the_review` (the 0.52.0 blocked path end to end through `nirvana_gate::gate`, a mid-order question, a lesson review taking no question, a reseat child, another chain's question) | `lrs_asked_in_chain`, `lrs_answer_in_chain`, `lrs_question_same_root_only`, `lrs_chain_root_walks_up`, `lrs_run_counts_asked`, `lrs_run_opens_question`, `lrs_run_answer_claims`, `lrs_run_delivered_answers_own`, `lrs_run_reviews_answer_none` |
| Nit: `[""]` is a signal | `said`: a list says something when an item does; blank strings, null and `{}` say nothing | `lrs_each_report_signal_keeps_the_review` | `lrs_blank_item_is_no_signal`, `lrs_null_is_no_signal`, `lrs_empty_object_is_no_signal`, `lrs_blank_string_is_no_signal`, `lrs_string_is_a_signal`, `lrs_empty_list_is_no_signal` (repointed) |
| Nit: cap 0 says "nothing to learn" | `wanted_with` says the cap of 0 before `idle` | `lrs_a_clean_first_pass_is_skipped_before_the_cap_counts_it` | `lrs_cap_zero_says_off` |
| Nit: seat tests hard-code the count and indexes | the tests name each copy by `include_str!` and check it is in `HISTORICAL_DEFAULTS` | `lrs_an_installed_34a_or_35c_default_upgrades_to_r2`, `lrs_agy_seats_list_browser_and_a_browser_order_can_seat_agy` | `lrs_v7_historical`, `lrs_v8_historical` |
| Nit: three defaults with no copy | `nirvana_seats_v9.toml`-`_v11.toml` from git, in `HISTORICAL_DEFAULTS` | `lrs_an_installed_34a_or_35c_default_upgrades_to_r2` (now v7-v11) | `lrs_v9_historical`, `lrs_v10_historical`, `lrs_v11_historical` |
| Nit: branches with no dedicated sabotage | one control each: the reseat of a first order (r3), the null/string/object arms, `claude_only` | as named | `lrs_ungated_reseat_parent_is_first_pass`, the arm controls above, `lrs_claude_only_skips_agy` |

I checked each finding against the code first, and none was wrong. Two readings I would correct:

- The review says the hunk existed "only as `C:\Users\fahim\AppData\Local\Temp\lrs\gate.hunk`". A copy was also
  saved at `scratchpad\next\lrs-gate.hunk`, and the two are identical byte for byte (`cmp`). Its point stands: neither copy is in
  the tree.
- S3 names "a `Gated asked` for an order in the same chain root". That alone does not catch the 0.52.0 path: the
  redo after a block is a fresh order whose chain root is itself, not the blocked order. That is why `answers` exists.

## Not done, and open

- The live effect is unmeasured: no order ran on this build.
- Whether an `asked` gate (the send-back cap spent, or a blocked delivery) should queue a review. The order's "a
  failed or asked gate" reads as "has something to learn"; `wanted_for` says so for any verdict other than a pass, but
  the gate queues a review only after a pass or a send-back (plan 33 §3.3), and this does not add one.
- The answer link (review S3) is a heuristic: the first order seated with no parent after an open question answers
  it, oldest question first; a lesson review or a `review.adversary` reviewer takes none. If the orchestrator seats an
  unrelated order first and the redo second, the unrelated one is reviewed and the redo reads as a first pass. The
  journal holds no explicit link (no `--answers` on `seat run`), and adding one is order G's and the skill's ground.
- `OrderState` gained `asked` and `answers`. It is `Serialize`, so any JSON that prints it whole gains two fields;
  `grep` found no such printer, but this is a reading, not a test.
- The Learning square counts reviews seated and the cap's count; it shows no skip count (plan 36 never had one), and
  none was added.
