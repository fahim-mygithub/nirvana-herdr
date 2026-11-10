# P40b ORDERS: worker, gate and run fixes; a lighter worker start; the image guard (plan 40, 2026-11-09)

Built from `review-orders.md` and the usage audit's addendum. Controls: `nirvana/controls.d/c40b.py`. Each number
below names where it was measured; a number with no source is a hypothesis and says so.

## The review's findings, checked against the code

| Item | Verdict on the review | What changed |
|---|---|---|
| B1 | Right. `chain()` stopped at `why = "reseat"`. | `chain` follows `send_back` and `reseat`, counts only `send_back`. |
| B2 | Right. `child.kill()` ended only `cmd`/`cargo`. | The gate's test runs in the worker's job object, lifted to `src/nirvana_worker/job.rs`; readers joined with a deadline (5 s after a timeout, 30 s after an exit). The gate lock is released when `gate()` returns, which it now does. |
| B3 | Right for both readers. | `job::lossy_lines` (bytes, lossy decode) in the worker and the job runner. After stdout ends the supervisor keeps the stop and stall rules (`try_wait`), never a bare `wait()`. The job runner ends on the child's exit, not on pipe EOF. |
| B4 | Right. | `absent` is judged on the whole output; the first line naming each check survives the tail cut. |
| B5 | Right. | `-z` parse in `commit_work`. |
| B6 | Right. | `strip_key` for `test:`, `check:`, `serves:`. |
| B7 | Unmeasured, as the review said. | 1800 s ceiling while a call is open, a hypothesis. "Open", read from the fixtures: claude, a `tool_use` id until the `user` line whose `tool_result` names it; agy, a `step_type: tool` step `ACTIVE` until the same `step_index` is `DONE` or `ERROR`. Both fixtures close every call by their end (`c40b_a_tool_call_is_open_from_its_call_to_its_answer`). |
| B8 | Right. | `nirvana_session_record::strip_verbatim` is `pub(crate)`; the media builder drops its own copy. |
| B9 | Right. | The pane counts `tests` (`2 pass, 1 fail, 1 not run`); a string is still shown as before. |
| B10 | Plausible, no incident. | Every `herdr_json`/`herdr_ok`/`herdr_text` call has a 10 s limit (a hypothesis). On a timeout the title reads as unknown, so the pane is kept; the order's outcome is already written. |
| M1 | Right. | The guard says "your own seat (<seat> <effort>)" from the session's recorded seat, else "your own seat". No worker name as an example. |
| M3 | Right. | The stream reader names no pool; `rate_limited` spends `seated.pool`. |
| D1 | Right. | `nirvana_job` uses the worker's herdr helpers. Its `in_herdr` stays: it also asks the server, the worker's does not. |
| D2 | Right. | `nirvana_claude` and `nirvana_job` use `nirvana_run::now`. The copies in `nirvana_seat`, `nirvana_usage`, `nirvana_seats_square` are other builders' files. |
| D3, D4 | Right. | Removed; the test-only wrappers are `#[cfg(test)]`. |

## A lighter worker start

- **`--exclude-dynamic-system-prompt-sections`** on every claude worker argv. The owner's measurement, 2026-11-09,
  Haiku, the worker's flags, empty folders: 7,315 cache tokens written and 16,785 read without it; 5,390 and 18,463
  with it. About 1,900 fewer written per start in a fresh worktree. The audit's median claude first turn is about
  32k tokens, and a whole order far more, so this is small.
- **T1.** File lists in the owned, commit and rebase axes are capped at 40 lines and `+N more`. The axis's count
  stays the whole list's. The incident in `order_changes`' comment listed 3,958 files.
- **T2.** The footer now says only what neither the prompt nor the schema says, plus the owner's finish sentence. Its
  length barely moved (319 to 311 characters, `python` over `git show master:` and the worktree); the gain is the
  sentence, not tokens.
- **T3.** `tests` and `files` are optional in the schema and `check_report`, still checked when present. Saved output
  tokens per report: not measured.
- **T4.** `CLAUDE.md` size, read only: `3D-Printer Flow`, `3D-Printer-Flow-xtool`, `groove-coach`,
  `WorkOutAppRefactor`: none at the folder root, so nothing is copied. `D:\MotoProject`: 66,547 bytes, about 16.6k
  tokens at 4 bytes a token, copied into every agy order there. Over 20 KB, so what I would do, not done: copy a trim
  of at most 20 KB (the headings and the rule sections) and one line naming the full file's path for agy to
  `read_file` when it needs more. The cut is the owner's call: it decides which of the project's rules agy loses.
- **X1.** The tee keeps an image block's type, media type and `data_omitted_chars`, and drops the base64 of a `data`
  or `base64` string of at least 1,024 base64 characters in a line over 16 KB. Text, paths and calls are kept, so
  the sources reader still finds a `Read` (test). The audit counted 1,819 images taken by workers.

## The image guard (X2)

`herdr nirvana guard-read`, a `PreToolUse` hook on `Read`, orchestrator only. It refuses an image over 300 KB (a
hypothesis) and leads with `herdr look` and `herdr media shrink`. It fails open. ROLE.md has the rule in two sentences.

## ROLE.md: "never" rules with no scope or reason (proposed, not changed)

1. "Two orders never own the same file." Proposed: "Two orders that run at the same time never own the same file:
   concurrent writers into mutually-citing files measured a 50x error rate (Nirvana report 01 §4)."
2. "never browse yourself for more than one page." No reason given. Proposed: "never browse more than one page
   yourself: every page you read is re-sent with each later turn." The incident behind it is the owner's to name.
3. "Never read a large file or a video into its own context." "Large" has no size. Proposed: "Never read a file over
   <size the owner picks> or any video into your own context: it is re-sent with every later turn."
4. "A profile never grants a Send." The reason sits in the next bullet. Proposed: "A profile never grants a Send:
   only the owner's own words on the order do (the 2026-10-29 print sent with a warning unread)."
5. "never by reading the whole log." No reason. Proposed: "never by reading the whole log: a log read here is
   re-sent with every later turn."

## Addendum A: the record says the final decision

- Where: `nirvana_run::outcome_word` gave the record's word. `accepted` is journaled by the orchestrator through
  `herdr run log` (a `gated` event, free verdict). `Outcome::parse` knows only the gate's five words, so an accepted
  order fell to its phase and read `failed`. When the gate had already recorded a failure, `record_once` refused the
  second line.
- Now: the word is `accepted`, which `Outcome::tally` counts as a pass. When the order already has a failure line,
  `nirvana_run::correct_record` rewrites that one line in place under `leases.lock` (temp file, then rename). The old
  word is kept as `gated_outcome`. One line per order stays true, so every reader counts the order once. No
  `nirvana_seat.rs` hunk is needed.
- Subagents: a subagent has no output schema, so its last message is prose, and `report_in` finds no Work Report.
  A non-empty final message is now `subagent-delivered`. `subagent-no-report` means an empty end. `tally` already
  skips the subagent chain, so neither word counts for or against a seat.
- The installed record, recomputed by
  `scratchpad/c40/c40b_recount.py` (read only, 323 lines, 95 journals, none unreadable): 55 `failed` and 10
  `sent_back` would read `accepted`. Tallied passes go from 118 of 289 to 183 of 289. The 24 `subagent-no-report`
  lines cannot be recomputed: the record does not keep their final message. A one-time repair is the owner's call.

## Addendum B: media orders and their sources

Read from the three journals and streams (read only):
- `video-breakdown` (groove-coach run `spec`): it cited `file:///C:/.../Downloads/IMG_4941.mov` and its own order
  file, and it had opened both with `read_file`. It was gated at 02:46Z on 2026-10-31. The local-file rule
  (`StableId::Path`, plan 35a round 2, commit a916585a) landed at 20:27Z the same day. Today's code passes it.
- `yt-breakdown` (run `youtube`): it cited `https://www.youtube.com/watch?v=8aQ8SoQl5EY`. No tool call named that URL.
  The worker opened a download made before its order: `read_file` of
  `AppData\Local\groove-authoring\youtube\8aQ8SoQl5EY\ingest.json`, `contact_sheet.jpg`, `proxy.mp4`, then probed
  them with ffmpeg/ffprobe.
- `yt-breakdown-illit`: the video in the same way (the `KuKVOfwBPQI` folder). It also cited tunebat.com and
  songbpm.com pages that no tool call fetched. Those two were rightly refused, and still are.

So the addendum's rule as written ("a call names the URL") would not have passed either YouTube cite. The rule built
for `media.*` only: a cited URL counts when a shell command or a file tool's path names it. For a YouTube URL, its
video id in an opened path or a run command also counts. The report's own `finish`/`StructuredOutput` call and file
writes never count. The oEmbed check of cited videos still runs. `research.*` is unchanged.
