# Plan 40: improvement cycle (2026-11-09)

One pass over the whole fork: what the project is for, what its own logs say about how it was used, what the
design should say now, and the packages built from that. Every number here comes from a script named beside it.

## 1. Ledger

| # | The owner, 2026-11-09 (verbatim) | Surface | Verifier |
|---|---|---|---|
| L1 | "Research the urrent project and its goal" | §2 | the sources §2 cites |
| L2 | "validate how we have been using Nirvana over its lifetime from its usage logs with each model" | `nirvana/usage_audit.py`, `2026-11-09-40-records/usage-audit.md`, §3 | two runs at one `--now` print identical output; 16 unit tests; 7 sabotage reds |
| L3 | "Improve the design and architectural philosphy o this project" | §4 | each principle names its measurement; rulings for the owner in §7 |
| L4 | "create new features that would help improve token efficenccy" | P40a, P40b, P40d | each feature's before and after number in its record |
| L5 | "improve multi modal usage" | P40c `herdr look`, P40b image guard, P40e phone photo note | tests with fakes; the live check in §8 |
| L6 | "improve project output" | P40a Fable seat and step-up, P40b gate fixes and the footer, P40d lessons | tests; the record's pass rates on the next cycle's audit |
| L7 | "Fable 5.2 has now released check the claude documentation and update the apps seating now that we have this model avaiable." | P40a, §5 | the pick table in `p40a-seat.md`; fail-closed tests |
| L8 | "Do a project lceanup / code review and ifx any bugs." | four reviews (`2026-11-09-40-records/review-*.md`), P40a to P40e, `nirvana/clean_temp.py` | a test and a sabotage control per fix |

## 2. What the project is for

- The spec (`Nirvana/docs/spec/2026-09-15-nirvana-design-spec-v0.1.md`): a CLI-first orchestrator that seats work
  orders on vendor CLIs under the owner's own subscriptions, keeps every token and verdict in one log, and
  "improves from its own runs, and the improvement is proven, not assumed" (goal G7). Its key metric is
  performance, and the one it names as paid for most is observation cost: what the orchestrating model must read.
- The fork (`nirvana/README.md`, 2026-10-15): the same objective grown one feature at a time on herdr, with Claude
  Code as the engine. Its money is subscription windows (5 hours, a week), not dollars.
- Decisions that still govern: 0012 (the seat table is a prior, the owner's runs become the truth), 0017 (a review
  prefers another family), 0018 (a class sets its own bar), 0023 (a finished order leaves a lesson), 0029 (the
  router remembers the seat).

## 3. What the logs say

`PYTHONIOENCODING=utf-8 python nirvana/usage_audit.py --now 1794267000` prints all of it; the record
`2026-11-09-40-records/usage-audit.md` ranks ten findings. The ones this plan acts on:

1. **Most spend is not Nirvana.** Lifetime list price $10,060. One plain Claude Code project is 57%, of which
   $5,120 is Opus subagents and Workflow agents that inherited the session's model. The harness's own builds are
   20%. Everything the fork seated is under 9%.
2. **Cheap seats pass as often as dear ones on bounded work.** `build.bounded`: Sonnet 25 of 30 at $0.95 a pass,
   Opus 9 of 10 at $3.58. `review.adversary`: Sonnet 17 of 17 at $0.21. `build.load_bearing`: Flash 9 of 28 with
   15 send-backs, at $5.47 a pass against Sonnet's $1.44.
3. **The record does not yet tell the truth.** 65 orders the orchestrator accepted are recorded as failures, and
   24 orders run as Claude subagents are recorded "no report" whatever they delivered. Turning on `use_record`
   today would punish Sonnet for work it delivered.
4. **The priors are off where it matters.** Small orders cost 4 to 27 times their prior and large ones a third.
   The Max pool's 5-hour factor assumed $25 a window; measured on 2026-11-09, 57% of a window was $78 of list
   price, so a window is about $137 (`per_usd` 0.0073, not 0.04).
5. **The orchestrator is a third of a run.** Six sessions over 8 hours hold nearly all of it, and 118 turns after
   an idle gap of over an hour re-bought their cache: $95, 27% of its spend. Idle compact, shipped in 0.40.0 to
   stop exactly this, has no line in the live server's log since 2026-10-15: it has never fired.
6. **Images are read on the dearest seat.** 406 images entered the orchestrator's own context (up to 12% of its
   spend carried forward). `herdr media` ran twice, lifetime.
7. **A Claude worker starts at 32k tokens and reads 54% of its input at contexts over 150k.** The order file is
   small (median 927 tokens). Sessions compact by themselves at a median 249k (55 auto compactions since
   2026-10-23, none over 430k), so the 967k default the docs give is not what this machine does.

## 4. The design, restated

The spec's shape holds: a deterministic kernel, one model seat that decomposes and escalates, gates that fail
closed. Five principles are added or sharpened. Each names what it rests on.

1. **The strongest model plans and judges. The cheapest capable model does.** Fable 5.2 is the orchestrator in
   SOTA mode and the last rung for load-bearing classes, never the default worker. (Finding 2; Anthropic's guide:
   "Early testers had good results with Claude Fable 5.2 as the planner and orchestrator for other models".)
   Effort follows the role: low for a well-specified worker, medium for conversation, high for load-bearing work,
   and nothing above high without the owner.
2. **Context is the cost, not the turn.** A turn costs what it re-reads. So: nothing is injected twice, an idle
   session is compacted while its cache is warm, a worker's fixed start is shared and small, and the orchestrator
   reads results, not transcripts or pixels. (Findings 5, 6, 7.)
3. **Perception happens at the edge.** An image, PDF, video or recording is turned into text and numbers once,
   by the cheapest seat that can perceive it, and the text is what travels and is kept. (Finding 6; measured
   2026-11-09: a one-shot image read on Haiku with only the Read tool is 10k tokens and $0.00016.)
4. **The record closes the loop, and each cycle proves it.** The table is a prior; a script refits it from the
   record; an outcome means what the gate and the owner finally decided. An improvement cycle is the audit
   script, the fit, and a review, rerun. (Findings 3, 4; spec goal G7; CLAUDE.md rule 4.)
5. **Spend rules fail closed, and credits are never spent silently.** A missing or stale reading is a reason not
   to seat. This matters most for Fable: Claude Code's docs say a headless Fable run past its weekly allowance
   "bills it without asking". (§5.)

Carried over from the Claude Fable 5.2 guide into how instructions are written: a rule carries its scope and its
reason; cautions that no longer have a reason are removed each cycle; no text tells a model to think less (lower
the effort instead); no token budget is shown to a model.

## 5. Fable 5.2, from the docs (fetched 2026-11-09)

- `claude-fable-5-2`; in Claude Code 2.1.321 `--model fable` resolves to it. 1M context. $8 / $40 per MTok, cache
  read $0.20. Efforts low to max; Claude Code's default is medium.
- Max plan: "You can use up to 50% of your weekly usage limits on Fable models at no extra cost." Past that it is
  usage credits, and with `-p` "Claude Code bills it without asking".
- Subagents inherit the main model unless the call, the agent file or `CLAUDE_CODE_SUBAGENT_MODEL` says otherwise.
- Fast mode bills usage credits on a subscription: the harness never passes it.
- `/usage` on 2026-11-09 still reads `Current week (Fable)`.

## 6. Packages

| Package | What | Record |
|---|---|---|
| AUDIT | `nirvana/usage_audit.py` and its findings | `usage-audit.md` |
| P40a SEAT | Fable 5.2 seat (fail closed on its week), the orchestrator's seat from the table (SOTA takes Fable), cells and prices recomputed by script, the seat review's five bugs | `p40a-seat.md` |
| P40b ORDERS | ten worker, gate and run bugs; the shared system prompt flag; capped file lists; a shorter footer that says to finish the order; the stream keeps no image bytes; the image guard on the orchestrator's Read | `p40b-orders.md` |
| P40c LOOK | `herdr look` (an image or PDF read that returns text, leased, cached, downsized), still images in `herdr media shrink`, `herdr media doctor` | `p40c-look.md` |
| P40d MEMORY | seven memory-area bugs; capped board notice and lessons; no Jev guide without a key; lessons worded to be opened | `p40d-memory.md` |
| P40e SHELL | model labels from the model id; installer steps that cannot leave a half install; phone photos say what they are; `nirvana/gate.py` and `nirvana/clean_temp.py` for 35,326 leaked test folders | `p40e-shell.md` |
| Round 2 | the record's outcomes (accepted, subagent orders), sizes and the 5-hour factor refit from the record, idle compact that fires, media orders at the gate | §7, after the first merges |

## 7. Rulings for the owner

1. **Subagents outside Nirvana.** The largest saving found is not in the fork: set
   `CLAUDE_CODE_SUBAGENT_MODEL=sonnet` in Claude Code's user settings so a subagent with no model stops inheriting
   Opus (finding 1). Not done: it is the owner's settings file.
2. **My own builds.** The standing rule is "builds go to Opus high". Finding 2 says bounded builds pass as often
   on Sonnet at about a quarter of the cost. Proposed: Opus high for load-bearing packages, Sonnet high for
   bounded ones. Not changed in this cycle.
3. **`use_record`.** Leave it off until round 2 has fixed the outcomes, then turn it on.
4. **Jev's shadow seat advice** costs a network call per pick and acts on nothing. Proposed: off until wanted.
5. **Phone uploads** are never deleted. Proposed: nothing automatic; a command lists them.

## 8. Live checks owed

Listed in each package record, in order. The first three: `herdr seat list` shows `fable` ready with its week;
`herdr look <a phone photo> --ask "what is in this photo"` answers in text and a second ask is a cache hit; a
SOTA space's Seats square reads "SOTA" with Fable, and reads the fallback when the Fable week is not read.
