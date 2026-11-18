# Plan 43: what run `ia1` on Megajam says to fix

Status: PROPOSED 2026-11-18. An audit with proposed packages. No code yet. Owner rulings open in §7.

## 1. The ask

The owner, 2026-11-18: "I want you to do an audit of lasat weeks run of Megajam for this project. Cocnsider the jobs
memeories and logs from that project. Where are spaces we can improve our job completion, token efficienies etc."

## 2. Requirements ledger

| # | Quote | Surface | Owner | Verifier |
|---|-------|---------|-------|----------|
| L1 | "an audit of lasat weeks run of Megajam" | run `ia1`, 2026-11-12 20:09Z to 11-18 17:40Z, still live | this plan | the scripts named in §8 |
| L2 | "Cocnsider the jobs memeories and logs" | the journal, orders, streams, reviews, lessons, board, the orchestrator's transcript and memory, herdr's server log | this plan | §3 to §5 |
| L3 | "improve our job completion" | §3 and packages A1 to A10 | herdr | a re-run of the funnel on the next run |
| L4 | "token efficienies" | §4 and packages B1 to B8 | herdr | a re-run of the token split on the next run |

## 3. Job completion

Most work lands. 241 of 293 work lineages landed, which is 82%. 19 more were replaced by a later order that landed. 33
landed nothing. Completion is lost in the repair loop, not in the first attempt.

The gate refused 91 times. 45 refusals were right. 29 came from a defect in the orchestrator's brief. 17 were the gate's
own defect. So 46 of 91 refusals could not be fixed by a worker.

| # | Finding | Measured | Cause |
|---|---|---|---|
| C1 | The send-back child is the wrong tool for half the refusals | 73 orders were sent back at their first gate. herdr's child merged for 16. The orchestrator stopped the child 26 times and merged the parent by hand 11 times. 35 of the run's 77 interrupts are a child stopped at 0 minutes. | The gate seats a worker for a fault only the brief or the gate can fix. |
| C2 | The owned axis refuses new files no other order holds | 26 owned refusals flagged 273 files. 165 were new files the order made. 3 refusals touched a file another live order owned. 20 of the 26 came after 0.71.1. | Briefs leave out the evidence folder, helper scripts and new assets. The gate treats an unowned new file like a contested one. |
| C3 | The sources axis cannot see a read of a local file | research.read passed its first gate 0 times of 6, and all 3 of its lineages ended stopped. 108 of 239 flagged source entries were local paths. | `nirvana_gate::stable_id` maps only `file://` and drive-letter paths. A repo-relative path gives `None`. Checked on master. |
| C4 | One AskUserQuestion in auto mode froze the run for 6.8 hours | A modal question at 2026-11-15 07:49Z blocked for 406 minutes. 9 passed orders waited 173 to 407 minutes to merge. Nothing ran from 11:43Z to 14:36Z. | The orchestrator asked through the modal tool overnight. `nirvana_lean.rs` still lists the tool. |
| C5 | Broken `test:` commands in briefs | 5 of 12 tests refusals were the order's own test failing to parse. | Quoting errors written by the orchestrator. |
| C6 | Finished worktrees fill the disk | 51 worktrees of finished orders hold 183 GB. 3 orders were blocked or stopped by a full disk. | Unreal build outputs stay after merge and after stop. |
| C7 | Wrong seat at seat time | 9 stops were seat corrections at 0 minutes. haiku on build.bounded: 42% stopped of 12. fable on build.load_bearing: 62% stopped of 8. | Six fable orders were reseated on opus one minute after seating. Why is not established. |
| C8 | Owner changes mid-work | 17 stops followed an owner change, 226 worker-minutes. 8 landed nothing under the same name. | A live worker cannot take a note. The only tool is stop and re-order. |
| C9 | Restarts kill workers silently | 7 workers died in restarts after 481 worker-minutes. One had run 315 minutes. Recovery waited for the owner to say Nirvana restarted. One order hung silent for 626 minutes with no stalled event. | herdr does not check worker pids at server start. The hang's cause is not established. |
| C10 | The record undercounts | 43 orders landed on the integration branch with no `merged` event. 9 `merged` events have a null sha. 5 reseated parents have no `ended` event. | A hand merge has no command. A reseat writes no end. |
| C11 | A quota error was misread | An agy 429 said "Resets in 79h6m53s". herdr deferred two orders 31 minutes, got the same 429, then reseated. | `nirvana_run.rs` does not parse "Resets in". Checked on master. |

Per seat and effort, with lesson reviews left out:

| Seat and effort | Orders | Gated | First-pass | Sent back | Stopped | Median work minutes |
|---|---|---|---|---|---|---|
| opus high | 140 | 118 | 87% | 10% | 14% | 74.6 |
| sonnet medium | 79 | 61 | 39% | 48% | 20% | 16.4 |
| carbon high | 48 | 37 | 92% | 8% | 12% | 22.9 |
| hydrogen high | 42 | 34 | 74% | 26% | 14% | 16.7 |
| hydrogen medium | 26 | 24 | 83% | 12% | 8% | 3.5 |
| fable high | 20 | 14 | 64% | 21% | 25% | 37.5 |
| haiku low | 14 | 9 | 78% | 0% | 36% | 0.8 |
| opus medium | 14 | 9 | 44% | 56% | 36% | 12.8 |
| carbon medium | 12 | 10 | 80% | 10% | 8% | 19.3 |

These rates are confounded. Sonnet medium ran almost all of its orders in Balanced, early in the run, before 0.71.1 fixed
six hollow refusals. About half its refusals were brief or gate faults. SOTA had 85% first-pass of 180 gated, against
60% of 159 for Balanced, and SOTA came later.

By class: build.bounded 86% first-pass of 147 gated. build.load_bearing 34% of 38. plan 30% of 10. research.read 0% of
6, which is C3.

## 4. Tokens

The run spent about $2,490 at Claude list price. That is a list-price equivalent, not subscription usage. Claude workers
took 85.5% and the orchestrator's main thread 13.6%. agy is counted in tokens only.

SOTA on Opus high was $1,551, 62% of the run. That is the owner's choice and not a finding.

| # | Finding | Measured | Cause | Expected saving |
|---|---|---|---|---|
| T1 | Claude workers never compact | 313 Claude worker sessions, 0 compactions. Median peak context 148k, max 902k. Cache reads above 200k per turn cost $464, 18.7% of the run. Balanced's share of that is $53. | Workers report a 1M context window and herdr passes no cap. | 4 to 8%, about 1% outside SOTA |
| T2 | Work that never landed | Redo series under new order names cost $212. Send-backs and reseats cost $124. Stopped Claude rows cost $89. | A continuation worktree starts from main, not the parent's branch, so the parent's diff is re-applied by hand. Also C1 and C8. | 2 to 3%, not established |
| T3 | Idle compact never fired | 33 decisions on the orchestrator's pane, 0 compactions. 24 of 25 compactions were manual. 22 turns after a gap over 1 hour re-bought $39 of cache. | 15 times the input box was off screen. 18 times the transcript looked young. The hypothesis is that metadata lines keep its mtime fresh. | about 0.8% |
| T4 | The orchestrator did hands-on work | 484 GUI and browser calls, $24. 349 `git show` and `ls-tree` calls, $29. 220 images entered its main context and were re-sent in 13,171 image-turns. | The 0.66.0 image guard hooks only `Read`. Desktop and Chrome screenshots do not pass through it. | 1.5 to 2.5% |
| T5 | `herdr look` refused 1 call in 5 | 80 of 401 calls answered "no seat". Workers then read 2,214 images themselves. | From 11-16 05:21Z Claude's 5 h window reads "not read", and look will not seat a Claude seat on an unread window. | 1 to 2% |
| T6 | The orchestrator runs at 200k to 350k context | Median 203k per turn, p90 344k. One session for six days. MEMORY.md is 19k chars and loads every turn. | `autocompact_tokens` ships at 0, from when Claude Code compacted near 249k by itself. | about 1% |
| T7 | Seat polling | `seat read`, `status` and `pick` turns cost $20.50. | `seat run` does not print its pick and reasons. | 0.8% |

T2's dollar figure for "never merged" is left out on purpose. It counted by `merged` events, and C10 shows 43 orders
landed with no such event.

## 5. Lessons and memory

Lessons cost almost nothing in tokens. Briefs carry pointers only, and a Claude worker's first turn is a median 30,881
tokens. The problem is quality.

- 112 lessons, all from this run. Briefs offered them 745 times and workers opened them 158 times, 21%. 56 lessons were
  offered and never opened.
- 81 of 93 lesson reviews kept nothing. The ledger has 0 merge and 0 retire operations in 121.
- 16 lessons describe one shared build-queue constraint. They share a topic and not words, so a word-overlap test
  finds none. It is a machine fact that belongs once in the builder profile.
- 10 lessons are workarounds for Nirvana bugs: `herdr look` "no seat", the owned axis, the sources check, a lost job
  tail. They should have been bug reports.
- The orchestrator's memory has 110 files and 238k chars, with 88 writes against 12 reads. Five files are dated
  diaries. One still prescribes a workaround for a bug 0.71.1 fixed.
- `board.md` is stale since 11-13.

## 6. Packages

Ranked inside each group by expected gain.

| Package | What | Finding | Gate |
|---|---|---|---|
| A1 | Gate: a brief or gate fault goes to the orchestrator as `gate amend --owns/--test`, which re-gates the same branch with no new worker. Worker faults keep the child. | C1 | tests, a control per assertion |
| A2 | Gate: a new file no live order owns is a note, not a fail. `seat run` adds default owns for the order's evidence folder and warns when the order text names a path `owns:` misses. | C2 | tests, controls |
| A3 | Gate: resolve repo-relative source paths against the worktree and count Read, Grep and shell reads. Skip local entries for research.read. "No sources" is a note for media orders that asked for no web. | C3 | tests, controls |
| A4 | Auto mode refuses AskUserQuestion and points to `herdr ask add`. | C4 | hook test. Fits P42d. |
| A5 | `seat run` parses the `test:` command at seat time and refuses one that does not parse. | C5 | tests |
| A6 | At server start, check the pid of every order listed as running. A dead one is marked interrupted, checkpointed, and the orchestrator is woken. | C9 | tests, a live restart |
| A7 | Records: `gate merge --by-hand <reason>`, `ended: reseated`, and parse "Resets in" from a 429. | C10, C11 | tests |
| A8 | On merge and on stop, delete Unreal build outputs in the order's worktree. Refuse to seat a build under a free-disk floor. | C6 | tests. Owner ruling R2. |
| A9 | No haiku for a large `build.*` order. A `--dry-run` for batch launches. | C7 | tests |
| A10 | `herdr seat amend <order> --note`, injected at the worker's next tool call. | C8 | tests, a live check |
| B1 | A compaction ceiling for Claude workers outside SOTA. First a probe that a headless worker honours it. | T1 | the probe, then tests. Owner ruling R1. |
| B2 | A continuation worktree starts at the parent's branch tip. | T2 | tests |
| B3 | Idle compact dates the transcript by its last assistant turn and passes when the input box is off screen but empty. | T3 | tests, the server log on a live day |
| B4 | The orchestrator's image guard also refuses desktop and Chrome screenshots, pointing to `herdr look` or a browser worker. | T4 | hook tests |
| B5 | `herdr look` falls back to Hydrogen when Claude's 5 h reading is missing. Find why the reading stopped on 11-16. | T5 | tests, a live read |
| B6 | Ship `autocompact_tokens` at about 250k for the orchestrator. | T6 | one default. Owner ruling R3. |
| B7 | `seat run` prints its pick and reasons, and the run notice carries the report. | T7 | tests |
| B8 | Lessons: fold topic duplicates into the profile, route bug workarounds to a bug list, and stop reviews that keep nothing. | §5 | needs its own short design |

A1, A2 and A3 all edit `src/nirvana_gate.rs`, so they are one builder or three in sequence. A4, A6, B3 and B4 touch
separate files and can run beside them.

Recommended first round: A1 to A3 as one package, A4, A6, A7, B3 and the B1 probe.

## 7. Owner rulings

- **R1, worker compaction.** A ceiling near 300k for Claude workers in Efficient and Balanced, with SOTA left alone.
  Recommended: yes, after the probe. The quality cost of compacting a long Unreal order is not measured.
- **R2, deleting build outputs.** Delete `Intermediate/`, `Saved/`, `Binaries/`, `Build/` and `DerivedDataCache/` in a
  finished order's worktree. Recommended: yes on merge, and on stop only after a checkpoint. It deletes files in the
  owner's project folder, so it needs a yes.
- **R3, the orchestrator's ceiling.** 250k. Recommended: yes.
- **R4, new unowned files.** A2 loosens the ownership gate. Recommended: yes. Only 3 of 26 owned refusals were a real
  collision.
- **R5, the modal question.** A4 removes the orchestrator's modal question in auto mode. Recommended: yes.

## 8. Not done, and unsure

- The run is live, so every count will move.
- No pool history exists. The journal's `usage` rows are per-session token totals, not pool readings. The pool
  findings are a proxy from 527 seat reasons.
- Prices are list-price equivalents fitted to the streams' own cost field. Haiku's rate is a guess. Image tokens are
  assumed at 1,600 each.
- Stop causes, refusal verdicts and not-landed causes are hand classifications from the transcript.
- The installed version at each point of the run is not recorded. Three restarts are probably installs.
- T1's saving assumes a headless worker honours a compaction cap. Not tested.
- Not established: why six fable orders were reseated on opus, why one order hung silent for 626 minutes, and why the
  5 h reading stopped on 11-16.
- Not read: agy stream contents except for errors, the checkpoint diffs, and the claims inside the 16 reviews read.
- The scripts are in the scratchpad folders `mjaudit/completion` and `mjaudit/tokens` of the session that wrote this
  plan, with both full reports. They are not tracked.
