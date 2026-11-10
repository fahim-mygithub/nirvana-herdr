# P40d record: the memory area's bugs, capped injections, lessons that get read

Plan 40, package d, 2026-11-09. Builder "Opus high - memory fixes and lighter injections", branch `c40d-memory` off
master 86b37c7e (alpha 0.62.0). The owner, 2026-11-09: "create new features that would help improve token
efficenccy, improve multi modal usage and improve project output." and "Do a project lceanup / code review and ifx
any bugs." The review this fixes from is `scratchpad/c40/review-memory.md`; every finding was checked against the code
first. Controls: `nirvana/controls.d/c40d.py`. Sizes are printed by the tests named, so they are recomputed on every
run; the numbers below are the run of 2026-11-09 and rot like any count in prose.

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

- The offer lines (`LESSONS_OFFER_LINE`, `CARDS_OFFER_LINE`) now say "Before a step, open each lesson whose line touches
  what that step does, and leave the others closed." A hypothesis from the census (12 opens in 39 offers in one
  project): watch `herdr lessons stats` opened over offered after it lands.
- Review rule 6 keeps all it said and adds: "One exception: a dead end the stream shows the worker measured and then
  resolved or worked around is worth keeping, as the way through, so the next worker does not repeat it." A hypothesis:
  watch for kept lessons that are setup failures in disguise.

## Cleanup

- D1: `board-seen/` and `jev/compact/<session>.json` are swept of files past 30 days when a new session first writes
  there; `jev/calls.jsonl`, `jev/compact.jsonl` and `nirvana/jev-calls.jsonl` (both writers) drop lines past 30 days
  when written, rewritten at most about daily (only once the first line is 31 days old). Memory backups: past 30 days
  go, but the newest five of a folder, of each kind, stay whatever their age.
- D2: `refresh_preset` is reached from `client/shell/nirvana_mods.rs` three times (the review was wrong); its
  `allow(dead_code)` is gone. `refresh_trim` is reached only from a test, so it is `cfg(test)`.
- M1: the profile template's model line reads `# model: a model id that herdr seat list shows`.
