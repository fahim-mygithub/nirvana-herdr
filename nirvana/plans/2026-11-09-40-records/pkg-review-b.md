# Package review B: P40b ORDERS (c40b-orders ff46e7ea vs 86b37c7e)

Read only. Ran: `controls.py --dry-run` (3245 of 3247 find their text once; the 2 misses are `worker_tree_outside_the_job` in
o3.py and `gate_named_check_unchecked` in o5.py, whose find text this package moved: the hunks are needed and right in
kind). No cargo run (read and traced only).

## S findings (fix before merge)

**S1. `media_read` is far weaker than the addendum asked.** `src/nirvana_gate.rs` `media_read` (~line 1290) and
`stream_reads` (`reads.calls.push`, ~1476-1540). The order said a call "whose command or arguments name that URL (a
download or a probe)". The code pushes EVERY Bash/PowerShell command and every `Read`/`read_file` path, and `run_command`
CommandLine, into `calls`, with no `command_fetches` filter (research.* keeps that filter). Weakest deliveries that now
pass on a `media.*` order and did not before:
- cite `https://youtu.be/XXXXXXXXXXX`, ran `echo https://youtu.be/XXXXXXXXXXX` (or `ls`, `git log`, or a heredoc that
  writes the URL into a notes file): `extract_url_tokens(call)` equals the cited URL, so the source counts as read.
- cite a video, then `Read` of `C:\x\XXXXXXXXXXX` (a path that does not exist, or any folder named by the id): the
  tool_use is recorded whether or not the Read succeeded, and `id_in` only needs the id with non-id characters around it.
  So yes: an order can cite a video it never watched by touching a path that holds the id.
- only the oEmbed check (`invented_videos`) remains, and it passes any real video id.
Suggested fix: count a command only when it fetches or opens (`command_fetches`, or the first word is
`yt-dlp|ffprobe|ffmpeg|curl|wget|python`), and count a path only when the file tool call was answered without error and
the file has a media-ish name (`.mp4 .webm .mkv .jpg .json` under the id folder). Drop `echo`-class commands. Tests:
cite+`echo URL` must stay refused; cite+Read of a missing id path must stay refused.

**S2. New footer lets a worker push past a missing credential.** `src/nirvana_worker.rs` `ORDER_FOOTER`:
"Finish the whole order before you report. Work through ordinary obstacles such as an error, a timeout or a locked file.
Report `blocked` only for something the order forbids or only the owner can decide." A missing key, a sign-in prompt or a
file another live agent holds is neither "forbidden by the order" nor "a decision"; "locked file" is named as ordinary.
CLAUDE.md rule 12 and file ownership (rule 2) are the incidents. Suggested: "...Report `blocked` for a credential or
sign-in you lack, a file you do not own or another agent holds, something the order forbids, or a decision only the owner
can make. Never look for a credential elsewhere." ROLE.md edits (image sentence, "your own seat at your own effort") push
past nothing.

## N findings (record or fix when convenient)

**N1. B6 changes meaning of ordinary prose.** `spec()` now reads `Test:`, `Check:`, `Serves:` in any case. An order that
has a line "Check: the page loads" becomes check names `the`,`page`,`loads` (output must contain each; order sent back),
and a prose line "Test: click through the form" replaces the real `test:` command with that text (last one wins). Never
easier to pass, but a false send-back for an owner who edited nothing. Suggest: require the colon directly after the key
(already so) and ignore a `Test:`/`Check:` line whose rest has spaces and no backticks, or log the line read in the gate
ruling.

**N2. B4 weakest new pass.** With exit 0, a check name appearing anywhere (also stderr, a `Compiling <name>` line, a
"... ignored" line) outside the last 40 lines now satisfies it; before it had to be in the last 40. Same class of weakness
as before for short output, wider for long. `filter()` pins the FIRST line naming a check, which may be a compile line
rather than the result line: the reader sees "it ran" for a line that is not its result. Pin the last line naming the
check, or the first line that also matches pass/fail words.

**N3. T3 is safe for the gate.** grep shows no gate axis, lesson or remote reader consumes the report's `files` or
`tests` (only `view.rs`, which tolerates absence). A done report of just status, summary, gotchas now passes
`check_report`. No change needed.

**N4. Record rewrite (addendum A), traced.** `nirvana_run.rs` `correct_record`: takes `Lock::take(dir)`, the same OS lock
`record_once` and every appender uses (the only other `RECORD_FILE` writer is a test). Write temp `record.jsonl.correct`
then `rename` over: atomic on NTFS; a crash or full disk before the rename leaves the original whole and a stale temp
that the next run overwrites; no appender can interleave because it holds the lock. It rewrites only the FIRST line with
the same order and run whose tally is `Some(false)`; all other lines are copied byte for byte (`raw`), but CRLF endings
become LF and a final line without newline gets one (harmless). No line: `record_once` writes the new one and no
correction runs. Several failing lines for one order and run (should not exist): only the first is corrected. A pass
already there: untouched, so a second call is a no-op. The journal is untouched, so its `gated` failure still comes
first, and the line keeps `gated_outcome`. The rewritten line is re-serialised (struct order, `extra` flattened, so no
field is lost). Old installed lines are not rewritten unless that order is accepted again; no one-time repair runs.
Gaps: `at`/`ended_at` keep the gate's time, not the accept time; every caller is `let _ = write_record(...)`, so a lock
timeout (10 s) leaves the order `failed` with no word anywhere. Log the error.

**N5. Process control.** `TerminateJobObject` ends only processes assigned to the job: the child (started suspended,
adopted before resume) and its descendants. The herdr server and the owner's shell are not descendants, so `kill()`
cannot reach them. One consequence: `drop(job)` after a normal exit now also kills anything the test started and left
running (a build daemon such as sccache or gradle, or a `herdr` client that auto-starts a server as its child). Say so in
the changelog. Nested jobs are supported from Windows 8; this box is Windows 11, and `AssignProcessToJobObject` failure
makes `spawn` return Err, which the gate reports as `did not run` (fails closed). The gate lock is a guard returned from
`gate_lock`; `run_command` always returns (timeout kill, then `join_within` 5 s/30 s), so it is released on every path,
panic included.

**N6. B7 and B3 hang bounds.** "Open" is `!self.calls.is_empty()`: a claude `tool_use` id until a `user` line whose
`tool_result` names it, an agy tool step until DONE/ERROR; hidden calls count. A stream that ends (stdout EOF) with an
unanswered call and a live process is now stalled after 1800 s instead of 600 s (three times longer before the Stalled
event). A tool that never answers in `-p` mode (AskUserQuestion, ExitPlanMode) also waits 1800 s. Suggest: after
`stdout_ended` use `STALL_AFTER_S` whatever `tool_open` says, and exclude interactive tools from `calls`. B3: after EOF
the loop uses `try_wait`, the stop file and `stalled_silence`, so it cannot block forever (stall always fires at 1800 s at
most); `join_within` bounds the readers.

**N7. Image guard (`guard_read_decision`).** Whose Read: `should_skip_orchestrator_hook` skips when `agent_id` is set
(subagents), when `NIRVANA_ORCHESTRATOR` is not `1` (plain Claude Code panes, workers: the plugin settings set it only for
the orchestrator), and when the hook cwd contains `.nirvana/worktrees`. Fail open traced: no `file_path`, no extension,
non-image extension, missing file, directory (`is_file`), unreadable metadata, size <= 300 KB all return None; a panic
exits 101 (non-blocking hook error, the Read proceeds); a missing binary is also non-blocking. Only deny JSON blocks.
Costs: the hook spawns `herdr.exe` on every Read in every session that loads the plugin, before the env check; and the
reason names `herdr look`, which exists only on branch c40c-look (`git grep` at ff46e7ea finds `shrink` but no `look`).
Merge c40c before or with this package, or the refusal points at a missing verb for its first sentence.

**N8. X1 tee.** Lines under 16 KB are written untouched. Longer lines are parsed; a parse failure writes the line as it
is; a strip rewrites the line as compact one-line JSON (keys sorted, because serde_json maps are ordered), so each line is
still valid JSON with all tool calls, paths and text. Only a string under key `data` or `base64` of at least 1,024
characters made wholly of base64 characters is emptied, with `data_omitted_chars` beside it. The word "base64" in a text
or a `source.type` value is never touched. Risk: a legitimate long alphanumeric `data` string (a hash dump) is emptied;
no reader of the sources axis uses such a key.

**N9. `--exclude-dynamic-system-prompt-sections`.** One place (`argv`, claude arm), so resume (`--resume` appended after
it) and reseat (a new spec through the same `argv`) carry it; the agy arm does not (test asserts both). A session started
before upgrade and resumed after changes its system prompt once (one cache miss).

**N10. Hunks and controls.** The three hunks are: o3.py `worker_tree_outside_the_job` (file `src/nirvana_worker/job.rs`),
o5.py `gate_named_check_unchecked` (find text `!run.output.lines().any(...)`), o4b.py `o4b_guard_fork_passes` (test
`c40b_the_guard_names_the_sessions_own_seat`). Dry-run confirms the first two now read 0x on the old text, so the
retarget is required. The claim that `o4b_guard_fork_passes` caught nothing on master is plausible (a fork that names a
model is also refused by the later missing-seat check, and the test asserted only "refused"; the new test compares the
lead text of the fork refusal) but I did not run it. The c40b controls use product-line find text.
