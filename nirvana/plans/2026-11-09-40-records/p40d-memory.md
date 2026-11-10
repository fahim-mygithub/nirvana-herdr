# P40d record: the memory area's bugs, capped injections, lessons that get read

Plan 40, package d, 2026-11-09. Builder "Opus high - memory fixes and lighter injections", branch `c40d-memory` off
master 86b37c7e (alpha 0.62.0). The owner, 2026-11-09: "create new features that would help improve token
efficenccy, improve multi modal usage and improve project output." and "Do a project lceanup / code review and ifx
any bugs." The review this fixes from is `scratchpad/c40/review-memory.md`; every finding was checked against the code
first. Controls: `nirvana/controls.d/c40d.py`. Sizes are printed by the tests named, so they are recomputed on every
run; the numbers below are the run of 2026-11-09 and rot like any count in prose.

## Review round (2026-11-09, "Opus high - memory review round")

### The owner's live check, first

After the merge and the daemon restart: leave a Claude pane with a large context idle for over 50 minutes, then run
`herdr compact why`. It prints each Claude pane's state, idle time, next check, and the last check's facts and
decision. The server's log has one INFO line per new skip reason in a stretch, `nirvana: idle compact left the pane
alone`, and `nirvana: idle Claude sent /compact` when it fires. If it skips, the reason it prints is the blocker this
round could not see from the code.

### Part 1: idle compact never fired

- The suspicion is overturned. "done" is the agents view's name for `Idle` on a pane nobody has looked at yet
  (`src/app/agent_view.rs:327`). The terminal's state is `Idle`, and idle compact never reads the pane's seen mark.
  Test `idle_compact_server_compacts_a_finished_pane_nobody_has_looked_at`.
- Claude's state comes from its screen. herdr's Claude hook reports only `SessionStart`, so the stretch starts and
  ends with screen detection (`detect/manifests/claude.toml`).
- Nothing in the code is never true for an unattended pane. Rules 4 and 5 were replayed over the 127 idle gaps of
  over 60 min in this machine's transcripts since 2026-10-24 (`C:\Users\fahim\AppData\Local\Temp\c40d\gap_probe3.py`,
  `gap_probe4.py`, numbers from their run). Both rules held inside the hour in 109 gaps before this round and 115
  after.
- Fixed: Claude Code writes an `away_summary` recap about 3 min after a turn, and a `bridge_status` line. These kept
  the file's write time young, so rule 4 first held at 54 min in 45 gaps. Rule 4 now reads the newest line that is not
  one of those (`transcript_age`). A torn line falls back to the file's age.
- Not fixed, and not known: argv, the transcript choice, the owner key and the strict input line all need the live
  pane. The input line was recorded from Claude Code 2.1.302; the gaps ran 2.1.310 to 2.1.319. `herdr compact why`
  exists to answer this.
- Found: Claude Code compacts an idle session on its own at 55 min (`compact_boundary`, trigger `auto`). That
  happened in 25 of the 127 gaps, every one with a context over 200k. Only the owner can say if herdr's compact is
  still worth it below 200k.
- Every skip used to log at DEBUG, so the live log's 20,049 INFO lines held nothing about idle compact. Now each new
  reason in a stretch is one INFO line, and the server writes `<home>\nirvana\idle-compact.json` after each check and
  at most once a minute otherwise. The owner's typed text is never written there, only its length.

### Part 2

| Finding | What changed | Test | Control |
|---|---|---|---|
| S1 seven stale controls | Already in the hunk script. Dry-run: 3246 of 3253 without it, 3253 of 3253 with it, in a throwaway copy. | (the controls') | `c40d_context_not_summed` on `read_context`'s sum |
| N2 the cap starved new lessons | `keep_places_for_new`: 3 of the 15 places go to the newest lessons never opened. | `the_cap_keeps_three_places_for_new_lessons` | `c40d_new_lessons_no_places` |
| N4 a damaged first line | The trigger reads to the first readable `ts`, at most 64 lines. | `a_damaged_head_is_pruned_past_and_a_late_append_is_kept` | `c40d_prune_head_first_line_only` |
| N4 a late append was lost | The size is read again before the rename; if it grew, the rewrite starts again (3 tries). | same | `c40d_prune_late_append_lost` |
| N3, N4 the 30-day horizon | `herdr jev agree`'s help and `prune_log`'s doc say it. The audit script sees 30 days too. `PRUNE_AFTER` is the owner's to change. | none | none |
| N5 the first backup | The oldest backup of each kind stays too. | `old_backups_are_pruned_but_the_newest_five_stay` | `c40d_backups_oldest_deleted` |
| N6 the pre-restore copy | `restore` prints how to put it back by hand. | none | none |
| N9 the session start waited 3 s | `edit_memory_file_at_start`: reads first, locks only to change, waits 1 s, then skips its line with a message on stderr. The `nirvana_claude.rs` hunk in the script uses it and replaces the first round's hunk. | `the_session_start_edit_waits_about_a_second_and_skips` | `c40d_session_lock_waits_long`, `c40d_session_lock_always_taken` |
| N10 the fresh note | The claim was wrong: `nirvana_fresh` is unchanged since master. The comment at `FIRST_NOTICE_WHOLE_CHARS` now says so. | none | none |
| N11 the wording | Offer lines: "Open a lesson (card) when its line names the step you are doing". Rule 6's exception covers a dead end worked around "in the project's own code or method" and "never covers a missing program, a sign-in or a path on this machine". Both are hypotheses. | `rule_six_keeps_a_measured_and_resolved_dead_end`, T2's and T5's tests | `c40d_rule6_scope_missing`, the two wording controls re-pointed |
| N12 repaint every 5 s | `land` asks for a repaint only when the facts differ. | `the_jev_rows_repaint_only_when_their_facts_change` | `c40d_preload_repaints_always` |
| Part 1 rule 4 | `transcript_age`, `last_activity` | `rule4_reads_the_last_turn_past_the_away_recap` | `c40d_rule4_file_time_only`, `c40d_rule4_recap_is_activity`, `c40d_rule4_torn_line_skipped` |
| Part 1 why | `fact_rows`, `render_why`, `cli`, the server's snapshot and INFO line | `why_names_every_fact_and_the_decision`, `idle_compact_server_says_why_it_left_a_pane_alone` | `c40d_why_shows_typed_text`, `c40d_why_not_written`, `c40d_skip_reason_not_noted` |

Recorded, not fixed: N4, a clock set a year forward drops every log line. N7, the residuals stand, and `serde_json`
still re-sorts the owner's keys. N9, a stale lock takeover is still not race-free. N1, a session with no Jev key that
gets one later and then resumes has no guide until `clear` or `compact`.

## Bugs

| Finding | What changed | Test | Control |
|---|---|---|---|
| B1 idle compact re-compacts a compacted session | One reading, `nirvana_jev_compact::read_context` (tokens, compacted, or unknown), used by the Stop hook, idle compact and the fresh start. A `compact_boundary` system line or an `isCompactSummary` user line newer than the last assistant line is no reading, and the tail read stops there instead of widening. | `a_compaction_after_the_last_turn_is_no_reading`, `a_compact_summary_alone_resets_the_size` | `c40d_compaction_ignored`, `c40d_summary_line_ignored` |
| B2 hostile `Retry-After` panics | `retry_wait`: NaN, negative or unreadable waits the plain 400 ms; anything else, infinite or past `Duration` included, is capped one second past the deadline, so it means no retry. | `a_hostile_retry_after_never_panics` | `c40d_retry_after_panics`, `c40d_retry_after_negative_capped` |
| B3 `memory restore` overwrites the live index | Refuses, naming the pointers, when the index points at files the backup does not, unless `--force`; keeps the replaced index as `<hash>-<time>-NN.pre-restore` (not `.md`, so a second restore never takes it). | `restore_keeps_the_index_it_replaces_and_refuses_to_drop_a_pointer` | `c40d_restore_no_pointer_check`, `c40d_restore_keeps_no_copy`, `c40d_pre_restore_taken_as_backup` |
| B4 Preload draw path runs git and stats | The Jev rows read their facts (measure, `JEV.md`, today's log) on a `nirvana-preload-jev` thread, at most every 5 s; the draw call shows "measuring" until it lands. `git ls-files` goes through `nirvana_git::run_cmd` with a 5 s bound. No hunk in `client/shell/nirvana_mods.rs` was needed. | `the_jev_rows_never_measure_on_the_draw_path` (draw call measured at 0 ms against a 2 s read) | `c40d_jev_rows_measure_on_draw` |
| B5 two NIRVANA.md writers lose lines | `nirvana_lessons::edit_memory_file` holds `NIRVANA.md.lock` (the board's lock, now `nirvana_board::FileLock`) for the whole read-modify-write. The `nirvana_claude.rs` writer needs the hunk in the report. | `two_writers_of_nirvana_md_lose_no_line`, `a_file_lock_keeps_both_writers_lines` | `c40d_memory_file_unlocked` |
| B6 settings.local.json lost update | `nirvana_preload::update_local`: unique temp name (`temp_sibling`), the file read again just before the write, the edit applied afresh once if it changed, an error on a second change. The board's share switch uses it too. | `a_change_between_read_and_write_is_kept` | `c40d_settings_no_recheck` |
| B7 placeholders with no default | Every size `DEFAULT`'s front matter names is the default for one the owner's file leaves out. | `every_placeholder_has_a_default` | `c40d_placeholder_defaults` |

Found while testing B5, not in the review: on Windows the board's lock failed with "Access is denied" when it was
taken just as the last holder removed it (the file is "delete pending"). Measured: 2 of 12 runs of the two lock tests
failed before, 16 of 16 passed after. The lock now waits on that error as on a held lock.

## Less text per session

| Finding | What changed | Before | After |
|---|---|---|---|
| T1 first board notice | Over 1,200 characters, each section's name and first line, the size, and the `board read` command. Later notices unchanged. | 17,263 chars (14 sections) | 1,051 |
| T2 lessons in a brief | At most 15, ranked helped, opened, newest; `n more: herdr lessons for --class <c> [--profile <p>]`. `lessons for` takes `--profile` and lists most helpful first. | 3,894 chars (40 live) | 1,629 |
| T3 guide with no Jev key | The hook prints nothing. | 1,470 chars | 0 |
| T4 guide on `resume` | The hook reads `source` from stdin (the shape `integration/assets/claude/herdr-agent-state.sh` and `nirvana_claude` already read) and prints nothing on `resume`; `startup`, `clear`, `compact` and no source keep it. | 1,470 chars | 0 |
| T5 `offered` counted at brief time | `offer_uncounted` and `count_offered` for lessons and cards; `offer_for` and `offer` keep the old order, so the seat code works with or without the `nirvana_worker.rs` hunk. | counted on a failed seating | counted after the journal write, with the hunk |
| T6 Stop hook 4 MiB read | One 128 KB chunk first; the full tail only at a line or with no reading in the chunk. | 4,187,049 chars read | 130,351 |

## Lessons that get read: two wording changes, both hypotheses

- The offer lines (`LESSONS_OFFER_LINE`, `CARDS_OFFER_LINE`) now say "Open a lesson when its line names the step you
  are doing, and leave the others closed." (review round N11; the first round's "touches what that step does" re-tested
  every lesson at every step). A hypothesis from the census (12 opens in 39 offers in one project): watch
  `herdr lessons stats` opened over offered after it lands.
- Review rule 6 keeps all it said and adds: "One exception: a dead end the stream shows the worker measured and then
  worked around in the project's own code or method is worth keeping, as the way through, so the next worker does not
  repeat it. The exception never covers a missing program, a sign-in or a path on this machine." A hypothesis: watch
  for kept lessons that are setup failures in disguise.

## Cleanup

- D1: `board-seen/` and `jev/compact/<session>.json` are swept of files past 30 days when a new session first writes
  there; `jev/calls.jsonl`, `jev/compact.jsonl` and `nirvana/jev-calls.jsonl` drop lines past 30 days (`jev-calls.jsonl`
  has three writers; `nirvana_preset.rs` does not prune, the other two do, so it stays bounded)
  when written, rewritten at most about daily (only once the first line is 31 days old). Memory backups: past 30 days
  go, but the newest five of a folder, of each kind, stay whatever their age.
- D2: `refresh_preset` is reached from `client/shell/nirvana_mods.rs` three times (the review was wrong); its
  `allow(dead_code)` is gone. `refresh_trim` is reached only from a test, so it is `cfg(test)`.
- M1: the profile template's model line reads `# model: a model id that herdr seat list shows`.
