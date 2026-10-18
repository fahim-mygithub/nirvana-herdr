# Feature 12: Nirvana Claude, the orchestrator that seats work on Claude and Antigravity

Status: **DESIGN, for the owner's approval.** Nothing is built. §12 lists the owner's rulings, which come before
package O1. This plan replaces old Nirvana's daemon as the way forward. §10 maps every old Nirvana feature to its home
here.

## 1. Ledger

The owner, 2026-10-17, verbatim. A row is a requirement; the verifier is named before the build.

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "The goal is to utilize both my claude subscription and my agy gemini subsritpion to buid better apps and longer running agentic workflows." | seats on three quota pools: Claude Max, Google Gemini, Google's Claude and GPT pool (§3); runs that outlive one session, kept in a journal (§7) | orchestrator | the A/B in §11 O8; a run of more than 5 h that crosses a window reset and resumes |
| L2 | "have a orchistrator agent that is aware of the 5h and 7s usage limits and can seat and coordinate which agent and model can take the task to efficiently use usage credits." | `herdr usage` reads every window with its reset (§3); `herdr seat pick` scores seats by pool pressure (§5); the orchestrator seats every order through it | orchestrator | `nirvana_seat` tests with recorded readings; a live run whose journal shows each seating's reason line |
| L3 | "Nirvana was built to do this but we are now going to have the herdr varient work on this." | the fork carries the seating; old Nirvana's measured doctrine is ported, its daemon is not (§10) | orchestrator | §10's parity table, every row with a home or a reason |
| L4 | "Ensure you are feature complete we are moving to that implementation moving forwward." | §10: each old Nirvana feature is ported, replaced, or dropped with its reason | owner | the owner reads §10 |
| L5 | "Claude will be the orchistrator model alwys when you right click on a folder or start a Nirvana herder session  you will create a Nirvana version of Claude with all the tool / skills ready to work task in the manner we are describing." | the Explorer verb and a new Nirvana session open Nirvana Claude: Claude Code started with the `nirvana` plugin and agent (§6) | orchestrator | live: the verb opens a pane whose `/context` lists the nirvana agent, skills and hooks |
| L6 | "The reason we are building this as a seperate version which tbh is proably just Claude Code with these Orchistrator level skills and memories and hooks is to differentiate between the two." | Nirvana Claude is Claude Code plus a plugin and a launch argv; nothing in Claude Code is patched (§6) | orchestrator | a test that plain Claude's argv carries no Nirvana flag; `/context` on both |
| L7 | "Sometimes I want a Orchistrator style model that tries to utilize both Claude and ANtigravity and sometimes I just want Claude Code or agy traditional to tackle the project." | the sidebar's "new" menu offers Nirvana, Claude Code and Antigravity; only Nirvana gets the plugin (§8) | orchestrator | client test on the menu; live: a Claude Code tab has no nirvana items in `/context` |
| L8 | "Be aware a new model called Gemini 4 Argon is out." / "Research the token usage and the capbilities version 3.8 flash. Descide how to seat both model and effort level for their capabilites." | the seat table (§4) with Argon and Flash at each effort, and the class chains (§5.4) | orchestrator | probes P1 to P4 (§11 O0) replace the prior's quota costs with measured ones |
| L9 | "Argon also has capabilities that Claude does not have like ingesting video. Consider these unique apabilities as part of the orchistrator seating process." | capability filters: `video`, `audio`, `context_1m`; classes `media.video`, `media.audio`, `read.huge` (§5.2) | orchestrator | probe P2: agy reads a video in the workspace; `seat pick --needs video` never returns a Claude seat (test) |
| L10 | "Cosnider Jev and its capbilities in this process as well." | Jev's four roles in §9: keeping reads out of context, watching workers, advising the seating, and advising compaction | orchestrator | each role ships `off` or `shadow` with its call log; turned on by the owner |
| L11 | "One other thing to consider is a form of run receipt Nirvana had this for checking how the agents ran and how it can improve. This was sort of baked into that memmory feature we were talking about." (2026-10-17, during O2) | §7.4: `herdr run receipt`, recomputed from the journal and the record; the retro's gap ledger; lessons from orders the gate verified, under the Ledger-Lessons rulings of 2026-10-16 | orchestrator | a receipt test on a recorded journal that renders the same bytes twice and changes with one appended event; a retro test where a gap in 3 distinct runs graduates and a quote not in the journal is refused; a lesson written only after its order passed the gate |

## 2. What was measured before this design (2026-10-17, this PC)

- **agy 1.4.0 models** (`agy models`): `gemini-4-argon-{low,medium,high}`, `gemini-3.8-flash-{low,medium,high}`,
  `claude-opus-5-5-{low,medium,high}`, `claude-sonnet-5-5-{low,medium,high}`, `gpt-oss-120b-medium`.
- **agy's quota** (`agy -p /usage --output-format stream-json`, 0 tokens) has two groups. "Gemini Models: Gemini
  Flash, Gemini Pro" and "Claude and GPT models: Claude Opus, Claude Sonnet, GPT-OSS". Each group has a `5h` and a
  `weekly` bucket with a full-precision `remaining_fraction` and a `reset_time`. agy's own words: "Quota is consumed
  proportionally to the cost of the tokens. Thus, limits will last longer with shorter tasks or using more
  cost-effective models." No separate Argon bucket appears, against an X post that says Argon has its own weekly
  quota. Probe P1 settles it.
- **Argon's own review of this plan** (§14, `nirvana/evidence/2026-10-17-argon-review/`): 61,323 tokens on
  `gemini-4-argon-high` in 389 s, and neither agy group went down. The Gemini group's remaining went up, 0.98976 to
  0.99758 for the week. One sample says Argon spends a quota agy does not report. The same run hit Windows' 32,767
  character command line: the spec inlined in `--print=` failed with `WinError 206`; agy read it from `SPEC.md` when
  told to.
- **The O0 probes** (`nirvana/evidence/2026-10-17-o0-probes/`):
  - agy reports an Argon bucket after all, `gemini-argon-wtu-weekly`, weekly only; a small build order spent 1.01%
    of it. Fable spends the all-models week with a 50% allowance (Claude's usage endpoint, read once).
  - A small bounded order passed a hidden 10-test check on all seven seats. Flash at medium and high spent 9x to 16x
    flash low's output for the same result. One order on `g-sonnet` spent 3.3% of Google's Claude pool's 5 h window:
    that pool is small, an overflow, not a second Max.
  - agy's groups refill continuously; a `remaining_fraction` delta is a rough price only.
  - agy 1.4.x takes `--effort` on Claude models and base names; `--json-schema` works on agy and on claude.
  - Argon and Flash both read MP4 and WebM correctly. Flash needs the file's full path and the `view_file` tool named;
    with a bare name it ended with an empty answer and `status: SUCCESS`. Files over 100 MB are refused.
  - The taskbar icon, badge and toast work from inside Windows Terminal (P7).
- **Claude Max** (`claude -p /usage`, 0 tokens) prints three windows: "Current session" (5 h), "Current week (all
  models)" and "Current week (Fable)". There is no Opus-only weekly cap. Its usage notes for the last 7 days: "74% of
  your usage was at >150k context", "16% of your usage went to always-on context over 40k tokens", "95% of your usage
  came from subagent-heavy sessions". Large context is what spends the Max pool.
- **Prices**, as a stand-in for agy's cost-weighted quota: Argon $2 in / $10 out per Mtok until 2026-11-30, then $4 /
  $20, cached input 95% off (Google blog). Flash $0.75 / $3.75 (aggregators, medium confidence). So one Argon token
  costs about 2.7 Flash tokens of the Gemini pool. Both models are verbose: Artificial Analysis measured Argon at 110M
  tokens on its index against an 82M median, and Flash at 120M to 170M against 71M.
- **Coding strength**: Terminal-Bench 4.0, Opus 5.5 66.4% against Argon 57.4% (vendor); Artificial Analysis 59.6%
  against 57.1%. Argon ranks first on the Vals Index, 68.9%. These are priors, not truth (decision 0012).
- **Video**: Flash takes video and audio (llm-stats; Vertex documents about 45 minutes with audio, 10 videos per
  prompt). Argon's long-video score, LVBench 91.7%, implies video input; Google's page gives no limits. agy's file read
  attaches video: its 1.3.4 changelog fixed "the agent read a truncated or header-only WebM file". Claude takes images
  and PDFs, not video or audio.
- **Old Nirvana's record** (Moto A/B, 2026-10-12): an agy `gemini-3.8-flash-high` builder passed the held-out check 3
  of 3, in 485 to 622 s, at 16x Claude Code's tokens. One question run on the agy seat passed 1 of 3. Seat records:
  `gemini-3.8-flash` 9 of 12, `3.1-pro-high` 0 of 3. A seat is a model at an effort, not a provider.
- **Claude's thinking rules** (platform docs, "Thinking" and "Preserved thinking"): on the 5.x models thinking is
  adaptive and always on; `effort` is the lever. From Fable 5.1 on, a thinking block is valid only while everything
  before it is unchanged, and accounts made from 2026-08-31 get a 400 error on an edit. Changing effort or the thinking
  setting restarts the prompt cache. Switching a conversation's model drops reasoning that the new model cannot read.
  So a worker is never re-seated mid-conversation: a new seat is a new session with a written brief.

## 3. Pools and readings

A **pool** is one quota that several seats spend from. Each has one or more **windows**.

| Pool | Seats that spend it | Windows | Reader | Cost |
|---|---|---|---|---|
| `max` | Claude Code on the Max plan, every model | `5h`, `week`, `week.fable` (Fable only) | `claude -p /usage`, text; and every Claude worker's `rate_limit_event` | 0 tokens |
| `gemini` | agy Gemini models | `5h`, `week` (and `week.argon` if P1 finds one) | `agy -p /usage --output-format stream-json` | 0 tokens |
| `google3p` | agy's Claude Opus and Sonnet, GPT-OSS | `5h`, `week` | the same agy call | 0 tokens |
| `argon` | agy Gemini 4 Argon | `week` only, bucket `gemini-argon-wtu-weekly` (P1: agy started reporting it during the probes) | the same agy call | 0 tokens |
| `local` | Ollama, off by default | free VRAM | `nvidia-smi` | 0 |

`nirvana_usage.rs` grows from "percent used per provider" to readings:
`Reading { pool, window, used: f32, resets_at, read_at, source }`. It takes agy's groups as they come, not two
hard-coded bucket ids, so a new Argon bucket shows up with no code change. A new `herdr usage --json` prints them for
the plugin, the statusline and the seat engine.

Rules carried from old Nirvana and from claudexor, each with its incident:

- **A window that is not exposed reads as unknown, never as 0.** Old Nirvana: reading it as 0 would defer every order
  forever. A reader that fails ranks its pool below known headroom and above exhausted. A pool that is never exposed is
  not demoted for it (Argon's review, §4.5): its headroom is herdr's own count of its tokens against a budget in
  `seats.toml`, until a typed exhaustion says otherwise. Argon was that case until agy began reporting its bucket
  during P1; a window a pool does not have, like Argon's 5 h, is absent, not unknown.
- **A reading past its reset is stale, not zero.** Old Nirvana's "stuck at zero for claude" came from a passed reset.
  Staleness is keyed on `read_at`: older than 5 minutes while seating, or read before a reset that has since passed.
  A reading taken after the reset is fresh, so an idle account does not loop on re-reads (Argon's review, §4.6).
- **In-flight orders are booked.** `seat pick` and `seat run` write a lease to the journal, and a pool's headroom
  subtracts the expected use of every live order on it. Four parallel picks in five seconds cannot all see the same
  headroom and break the reserve together (Argon's review, F1).
- **Remaining against used.** agy prints remaining and Claude prints used. Reading one as the other drew 1% left on a
  full account.
- **Git Bash rewrites `/usage` into a path.** It cost a paid turn twice on 2026-10-17, once in a scholar run and once in
  this design's probe. herdr runs both readers with a list argv, never through a shell.
- **The orchestrator's reserve.** The orchestrator itself runs on `max`. A worker seat on `max` is skipped when
  seating it would leave less than `reserve` of the 5 h window: 15% to start, a hypothesis.

## 4. Seats

A **seat** is a CLI, a model and an effort, in one pool, with capabilities. The table is `<home>/nirvana/seats.toml`.
herdr writes it from a built-in default when missing, and from then on it belongs to the owner, like JEV.md.

| Seat | CLI and model | Effort | Pool | Capabilities | Prior strengths |
|---|---|---|---|---|---|
| `opus` | claude `opus-5-5` | low, medium, high, xhigh | max | code, tools, image, pdf, browser | plan, load-bearing builds, debugging, visual judgment |
| `sonnet` | claude `sonnet-5-5` | low, medium, high | max | code, tools, image, pdf, browser | bounded builds, ops, docs, questions |
| `haiku` | claude `haiku-5-5` | low, medium | max | code, tools, image | chores, sweeps |
| `fable` | claude `fable-5-1` | medium | max, `week.fable` | code, tools, image, pdf | §12 R1: the orchestrator or nothing |
| `g-opus` | agy `claude-opus-5-5-<e>` | low, medium, high, as the model id | google3p | code, tools, image, pdf, web | the same as `opus`, on Google's quota |
| `g-sonnet` | agy `claude-sonnet-5-5-<e>` | low, medium, high, as the model id | google3p | code, tools, image, pdf, web | the same as `sonnet`, on Google's quota |
| `argon` | agy `gemini-4-argon-<e>` | low, medium, high | argon | code, tools, image, pdf, web, image_gen, **video**, **audio**, **context_1m** | reviews of Claude's work, writing, debugging, reasoning over video, very large reads |
| `flash` | agy `gemini-3.8-flash-<e>` | low, medium, high | gemini | code, tools, image, pdf, web, image_gen, **video**, **audio**, **context_1m** | extraction, summaries, sweeps, bulk reads, transcripts, web lookups |
| `gpt-oss` | agy `gpt-oss-120b-medium` | medium | google3p | code, tools | not in the first version |
| `local` | Claude Code on Ollama `qwen3.6:35b-a3b` | medium | local | code, tools | not in the first version; `chore.*` only, off unless switched on |

`web` is agy's built-in search and URL reading, and bladebro for Claude. `image_gen` is agy's image-generator
subagent. Argon reported both from its own tool list, and its view tool's limit: image, pdf, video and audio files up
to 100 MB (§14; P2 checks it). The first version has seven seats: opus, sonnet, haiku, g-opus, g-sonnet, argon, flash.

For each seat the table also holds an **expected use** per class, and a **price schedule**: per-token prices with
their dates and any long-context tier, since Argon's price doubles on 2026-11-30. An order's use is computed from its
own streamed tokens (fresh in, cache write, cache read, out) times the schedule, never from pool deltas, which parallel
workers and the orchestrator's own turns blur (Argon's review, F8). One factor per pool turns that into a fraction of
the pool's windows; probe P4 fits it with one order running alone. The first version keeps the priors static and
logs every order's real use to `record.jsonl` for offline tuning.

Lessons that shape the launch, all from old Nirvana's adapters:

- herdr names agy's effort inside the model id (`gemini-3.8-flash-low`). On agy 1.2, `--effort` on a Claude model
  killed the worker before its first token (run-20260920T010056); P3 found 1.4.x accepts it, so either works and the
  model id stays the one way herdr uses.
- A worker's final report is checked against a JSON schema with `--json-schema` on both CLIs (P3). An empty answer
  is a failure even when agy says `SUCCESS` (P2).
- agy's `--print=<prompt>` needs the `=`; failure is read from `status`, because a 503 exited 0.
- The order goes in a file, `.nirvana/orders/<id>.md` in the worktree, and the prompt is one short line naming it.
  Inlining hit Windows' 32,767-character command line on 2026-10-17 (§2). Old Nirvana inlined it because agy once
  went looking for a path; Argon's review read `SPEC.md` when the prompt said to. P3 checks it on a worker.
- An agy worker can start its own subagents and background commands (Argon's review, F5). Its order says it may not,
  unless the order allows it, and its tokens include its subagents'. On Windows every worker runs in a job object that
  kills its whole process tree when the worker ends.
- A Claude worker runs with `--setting-sources project --strict-mcp-config`. Without it, one "PONG" turn cost 16x,
  from 120,104 cache-creation tokens of the owner's home configuration.
- A Claude worker runs with `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`, because a print-mode background task died with
  its process.
- Continuation is `--resume <id>` for claude and `--conversation <id>` for agy, never `--continue`, which picked the
  wrong conversation in a folder that had run twice.
- `claude -p --output-format json` writes only at exit, so a killed run leaves nothing. Workers stream JSON.

## 5. Seating

### 5.1 Who decides what

The orchestrator, a Claude, decides the **class**, the **needs** and the **size** of each order as part of its own
turn: no extra triage call. herdr decides the **seat**, deterministically, and prints why. The orchestrator may
overrule with a reason, which goes in the journal. This is decision 0012's stance: the table is a prior, and the
owner's own runs become the truth.

`herdr seat pick --class build.bounded --size small [--needs video] [--not-family anthropic] [--mode balanced] --json`
returns the seat, the effort, the pool, the reason line, and the runners-up with the reason each lost.

### 5.2 Classes

Old Nirvana's 16 classes, plus three for what only Gemini can do:

`plan`, `build.load_bearing`, `build.bounded`, `build.frontend`, `fix.debug`, `ops.terminal`, `review.adversary`,
`acceptance.author`, `sweep.mechanical`, `research.read`, `research.web`, `judge.visual`, `write.docs`, `chore.memory`,
`chore.script`, `chore.github`, and new: `media.video` (needs `video`), `media.audio` (needs `audio`), `read.huge`
(an input over 400k tokens, needs `context_1m`).

### 5.3 The rule

1. **Filters**, in order, each skip reported with its reason: the CLI is not installed or not signed in (Accounts,
   0.11); a need the seat lacks; the class excludes the seat; the caller excluded its family (a different family
   reviews, decision 0017, a preference); a pool window is at or below the floor (5%, old Nirvana's measured value);
   a window's headroom is below the order's expected use while another pool could hold the whole order (Argon's
   review, §4.4); a `max` seat would break the orchestrator's reserve.
2. **The bar**: the seat's score for the class must reach the class's bar at the mode's position (decision 0018). The
   modes: Efficient, Balanced and SOTA. If no seat clears it, the bar drops to the best one left, but never below the
   class's Efficient bar. If nothing is left, `seat pick` returns `NoSeat { reason, earliest_reset }`, and §5.6 defers
   or asks (Argon's review, F4).
3. **Price**: of the seats that clear the bar, take the one with the least **pressure**, computed for each window of
   the seat's pool and taking the worst (Argon's review, §4.1: a week's 20% holds far more than a 5 h window's 20%):

   `pressure = max over windows w of (expected_use_w / headroom_w × pace_w × reset_w)`

   - `expected_use_w` is the order's expected use as a fraction of window `w`; `headroom_w` is `1 - used - booked`
     after the reserve.
   - `pace_w = clamp((used + 0.10) / (elapsed + 0.10), 0.5, 2.0)`, where `elapsed` is the share of the window gone.
     The 0.10 keeps the first minutes after a reset from reading as 0 or as 3x ahead (Argon's review, §4.2).
   - `reset_w` lowers the price of quota about to be lost: a window with more than 30% left and under 60 minutes to
     its reset is discounted by up to half, scaled by the share of the order that fits before the reset (§4.3).
   - The 0.10, the clamp, the 30% and the 60 minutes are hypotheses, with their numbers in `seats.toml`.
   - Ties go to the higher score, then to table order.
4. **An override** by the orchestrator still passes the filters of step 1, reserve included, and at most 3 per run
   are accepted without the owner. A class that only Opus may take cannot be used to dodge the price (§4.7): the
   journal counts each run's classes, and the Seats square shows them.
5. **The reason line** goes to the journal, `herdr seat list` and, after O8, the Seats square, for example: `build.bounded → g-sonnet medium: max 5h
   49% used, reserve 15%, google3p 100% left; sonnet scores the same.`

### 5.4 The chains this gives, at Balanced

The first seat that is free and has headroom wins; the rest are what pressure falls back to.

| Class | Seats in prior order | Effort | Why |
|---|---|---|---|
| plan | opus, g-opus, argon | high | strongest at agentic coding (Terminal-Bench 4.0) |
| build.load_bearing | opus, g-opus, argon | high | quality first; argon only when both Claude pools are spent, and then a Claude reviews |
| build.bounded | sonnet, argon, flash, g-sonnet | medium; argon low; flash low | P4: all passed a small order; flash at medium and high cost 9x to 16x flash low's output for the same result, and Google's Claude pool took 3.3% of its window for one order, so it comes last |
| build.frontend, judge.visual | opus, g-opus; argon for anything that moves | high | the owner's rule: visual work and visual verification go to Opus. An animation, a transition or a screen recording goes to argon, which sees video |
| fix.debug | opus, g-opus, argon | medium; argon high | argon at medium under-reasons on a root cause (Argon's review) |
| ops.terminal | sonnet, g-sonnet, flash | medium | |
| review.adversary | another family: argon for Claude's work, opus or g-opus for Gemini's | high | decision 0017 |
| acceptance.author | another family from the builder: argon or sonnet | medium | decision 0019 |
| research.read | the orchestrator for a small question; sonnet; flash for bulk extraction | medium; flash low | M1: a question on a cheaper seat lost quality, so flash extracts and a Claude answers |
| research.web | flash, argon, g-sonnet, sonnet | low | agy searches and reads URLs at no Max cost |
| read.huge | flash, then argon | medium | 1M context; on Claude this spends the `>150k` usage the Max notes name. A read-only follow-up resumes the same agy conversation, since a fresh one re-reads the whole input |
| media.video | argon, which watches the video and reads the source; flash for transcripts and extraction | medium; flash low | only Gemini sees video; P2: both read MP4 and WebM exactly when given the full path. A transcript alone throws away what the video shows (Argon's review) |
| media.audio | flash, argon | low | |
| write.docs | argon, sonnet, g-sonnet | low | Argon's Vals Index lead; keeps docs off the Max pool. A Claude edits it to the house style when the doc is owner-facing |
| sweep.mechanical | flash, haiku | low | |
| chore.* | haiku, flash | low | |

The first version runs these as nine chains: plan, build.load_bearing, build.bounded, build.visual, fix.debug,
review, research, media, chore. The classes stay as labels in the journal, so the record can split them later
(Argon's review, cut 5).

### 5.5 Effort

- Effort follows the class and the blast radius, not the seat: a bounded order gets medium, a load-bearing one high.
  SOTA mode lifts plan and load-bearing builds to xhigh. Max is never chosen automatically.
- Effort is fixed for a session. A change restarts the prompt cache (Claude docs).
- Under pressure: first the same model on another pool (`sonnet` to `g-sonnet` is the same quality), then the other
  family's seat at the class's full effort, and only then one effort step down. Stepping down first spends the
  congested pool and weakens the reasoning (Argon's review). Old Nirvana changed the family first.

### 5.6 When a window runs out

The signals are typed, never guessed from text: Claude's `rate_limit_event` with a rejected status or an `api_retry`
with `error=rate_limit`; agy's exit code 3 with its `AGY_ERROR` line (scholar run, to confirm in P4). Then:

1. The worker stops at a turn boundary. herdr writes a checkpoint: the worker's report so far, its worktree's diff
   and the last turn.
2. **Defer** if the window resets within 30 minutes (old Nirvana's `MAX_DEFER_S`, a hypothesis).
3. Otherwise **re-seat**: a new session on the next seat in the chain with a continuation brief. Never the same
   conversation on another model.
4. Otherwise, in auto mode, **defer to the earliest reset**, up to the window's whole length, with a wake set for one
   minute after it. An unattended night run waits for the reset instead of waiting for the owner (Argon's review,
   §5.2).
5. Otherwise **ask the owner**. Nothing is dropped silently.

A Claude Code pane run interactively waits for the reset and goes on by itself (Claude Code 2.1.234 and later). The
orchestrator's own pane behaves this way; §12 R3 asks what happens to it meanwhile.

## 6. Nirvana Claude

Nirvana Claude is Claude Code, unpatched, started like this:

```
claude --plugin-dir <home>/nirvana/claude --agent nirvana --name "Nirvana" --settings <home>/nirvana/claude/settings.json
```

with `NIRVANA_ORCHESTRATOR=1` in its environment. Plain Claude Code is the same `claude` without these. The plugin
lives in the fork's tree under `nirvana/claude/` and is copied to `<home>` by `install.py`, so it updates with herdr.

| Part | What it holds |
|---|---|
| `agents/nirvana.md` | the main-thread agent: its role, the seating loop of §6.1, the rules below; model and effort per §12 R1 |
| `agents/worker-*.md` | Claude subagents for short orders on `max`, one per seat and effort, each naming its model and effort; omitting them inherits the orchestrator's seat |
| `skills/seat` | how to classify an order, call `herdr seat pick`, start it, read it and gate it |
| `skills/usage` | how to read `herdr usage`, what pace and reserve mean, when to wait |
| `skills/media` | how to hand a video or an audio file to argon or flash, and what to ask for back: a transcript, timestamps, findings |
| `skills/gate` | the completion gate's axes and the send-back loop (§7.3) |
| `skills/handoff` | the continuation brief for a re-seat |
| `skills/jev` | herdr's Jev tools, as today's JEV.md |
| hooks | `SessionStart`: the usage digest, the board, the open runs (about 300 tokens); `UserPromptSubmit`: `herdr board notice` and queued run events; `PreToolUse` on `Agent`: refuses a spawn with no model or no effort, and one on `max` that would break the reserve; `SubagentStop`: records the outcome in the seat record; `Stop`: `herdr jev compact`, and a check that no open run is left without a next step |
| statusline | each pool's tightest window, its reset, and the open orders |
| memory | `<home>/nirvana/NIRVANA.md`: lessons from finished orders that the gate verified (old decision 0023), plus the project's board |

Rules the agent file states, each from an incident:

- Seat every order through `herdr seat pick`; say the seat, model and effort in one line.
- Answer a question itself when the answer needs at most a few file reads. Make a small edit itself, one file and a
  few lines (§12 R2). Everything else goes to a worker.
- A question is answered, never built. A triage that fell back to a build turned a code-tour question into a
  7.72M-token build.
- Never read a large file or a video into its own context. Ask Jev, or seat flash or argon.
- Never resume a finished worker for a new question; seat a new one with the question. The one exception is a
  read-only follow-up to a `read.huge` agy session, which resumes it by `--conversation <id>`: a new session would
  re-read the whole input, and cached input costs a twentieth.
- A merge to `main` asks the owner, except in auto mode within its budgets (§12 R6).

### 6.1 The loop

1. The owner asks. The orchestrator writes the run's plan to the board, splitting the ask into orders with owned files
   (decision 0015).
2. For each order: `seat pick`, then `herdr seat run`, which opens a worktree and a worker pane.
3. Workers run in parallel. The orchestrator ends its turn. Between turns no model runs; herdr watches.
4. herdr wakes the orchestrator when an order ends, asks, fails or stalls (§7.2).
5. The orchestrator gates each delivery, sends back or merges, and seats what comes next.
6. The run ends when its plan is done and gated. The orchestrator answers the owner in one message.

**When the orchestrator cannot run** because the Max window is spent, herdr carries the run without it (Argon's
review, F2): the plan's next orders are on the board with their classes, and seating is herdr's anyway. herdr seats
them on the pools that are left, runs the deterministic gate, and holds every judgment: a review verdict, a send-back
reason, a merge. When the window resets, herdr wakes the orchestrator with what is waiting. §12 R3 asks whether a
stand-in on `g-opus` may make those judgments meanwhile.

**The orchestrator's context** rolls at a task boundary (decision 0038), and at the latest past 150k tokens: herdr
writes the run's state to the journal and the board, and a fresh orchestrator session takes the run from there. The
Max notes put 74% of the week's use above 150k (Argon's review, §5.3).

## 7. herdr's half: the runner, the journal, the gate

### 7.1 Workers

`herdr seat run <seat> --order <file> [--worktree] [--effort e]` opens a pane named `<seat>·<order>` in the run's tab.
It runs the worker headless, streaming JSON, with the launch lessons of §4. The pane shows a readable view of the
stream. herdr writes the raw stream to the journal and reads usage, rate-limit events and the final report from it. A
worker's last message is a Work Report checked against a JSON schema: `agy --json-schema` exists, and P3 checks
Claude's.

Claude subagents serve short orders that need no pane: a read, a review, a chore. Anything longer, and every agy or
local order, is a pane worker, so the owner can watch it, and Space agents shows its model.

### 7.2 Supervision and waking

- herdr's server, which already outlives the window, keeps the run's state. It records `seated`, `started`, `usage`,
  `rate_limited`, `checkpoint`, `deferred`, `reseated`, `delivered`, `gated`, `merged`, `asked` and `woken` in
  `.nirvana/runs/<run>/journal.jsonl`, append-only.
- **Waking the orchestrator**: when it is idle, herdr types a short digest into its pane with `herdr agent prompt`,
  which refuses a blocked agent. When it is busy, the `UserPromptSubmit` hook carries the queued events, the way
  `board notice` does today. herdr never types while the owner is: not when the prompt line holds text, and not
  within 30 seconds of the owner's last key in that pane. The wake waits in the queue (Argon's review, F7).
- **Stalls are read from the stream first.** A headless worker whose JSON stream is silent for 10 minutes is stalled;
  no model is asked. Jev's screen watch (§9) is for later.
- **No dead end**: an unfinished run with no live worker always has a pending next step (old Nirvana's
  `no_dead_end` check). The `Stop` hook and the server both check it.
- **Stalls**: a worker silent for 10 minutes, or one whose screen Jev judges to be looping or asking (§9), wakes the
  orchestrator. Restarts back off: at once, 30 s, 60 s, 120 s, then a crash-loop refusal.
- **Resume after a crash**: replaying the journal rebuilds the run. A new orchestrator session on the folder offers to
  continue the open runs; a run in auto mode continues without asking, since nobody may be there to answer (Argon's
  review, §5.5). A worker resumes its own conversation by id only within its own seat.
- **Cleanup**: a merged order's worktree and pane are removed right after the merge, and its job object ends every
  process it started, so no handle keeps a folder from being removed. Build output is shared per project where the
  toolchain allows it, with the gate's builds run one at a time (Argon's review, §5.4).
- **Seating is idempotent.** Each order has an id that is its idempotency key: `seat run` refuses a second live
  worker for an order that has one, and returns the first. A resumed orchestrator cannot seat a task twice.
  From durable-pi's `requestId` and `Idempotency-Key`.
- **An interrupted step is reported, never rerun.** On resume, a worker that was cut mid-order gets a checkpoint
  marked `interrupted`, with its worktree's diff. The orchestrator decides whether to continue, redo or ask. Durable is
  checkpointed, not exactly-once; durable-pi says the same of itself.
- **A run is phases.** The plan's orders are its phases, each one a checkpoint in the journal. A run stopped by a
  window resumes at the next unfinished order. No checkpoint sits inside a Claude conversation.
- **Waking has a busy rule**, from durable-pi's `whenBusy`: an event for a busy orchestrator queues (follow-up); an
  owner's urgent stop interrupts (steer); a wake for a blocked pane is refused and retried.

### 7.3 The gate

The completion gate is herdr's, deterministic. First the order's branch is rebased onto the tip of the run's
integration branch, so two orders that each pass alone cannot break the build together (Argon's review, F6). Then:
tests pass with the order's named check present in the filtered output; the report is well formed; only owned files changed; served, for ops orders, a port that the worker's own
pane listens on (decision 0041 R6). Then, for load-bearing orders, a reviewer of another family. A feature-size order
gets a held-out check written before the build by another family; the builder sees only the failing test ids
(decision 0019). At most 3 send-backs per order; then the owner is asked.

### 7.4 The run receipt, and what it teaches (L11)

Old Nirvana had three pieces: the one-page run report (`nirvanad/report.py`), the retro and its gap ledger
(`nirvanad/improve/retro.py`), and the skill a finished order leaves (decision 0023). The Ledger-Lessons research of
2026-10-16 measured where they broke: 21 of 21 memory writes skipped since 2026-09-22, because the repositories had
no `.harness/`; 6 retro proposals open for 24 days and none merged; 0 of 7 skills carried their class. Here the
three come back, built on the journal and the gate.

1. **The receipt.** `herdr run receipt <run> [--json]` prints one page, recomputed from
   `.nirvana/runs/<run>/journal.jsonl` and `<home>/nirvana/record.jsonl`. It is never cached and never reads a clock:
   an order's duration runs from `seated` to its last event, so two calls with no new event print the same bytes (old
   `report.py`'s rule). For each order: class, seat, model, effort, the reason line and any override, pool, tokens
   (fresh in, cache write, cache read, out), computed use per window, duration, and the outcome: the gate's verdict,
   send-backs, re-seats, deferrals, rate-limit stops and stalls. Then the totals: the orchestrator's own turns
   apart from the workers', and each pool's windows at the run's start and end, each figure with its basis. A figure
   that is missing says why, never 0. At the run's end herdr writes `receipt.md` and `receipt.json` beside the
   journal, and the orchestrator's last message names the file.
2. **The retro.** At the run's end herdr scans the journal for gaps: a send-back and its reason, a rate-limit stop,
   a stall, a re-seat, an override, a `NoSeat`, a gotcha a worker reported. No model is called. Gaps are keyed by
   (gap, run), matched by Sørensen-Dice on character bigrams at 0.6 (old Nirvana measured 0.90 for one gap phrased
   two ways and 0.18 for two different gaps). A gap seen in 3 distinct runs, or once with an outcome-tagged miss and a
   clear cause, becomes a proposal in `.nirvana/proposals/`, in the CHANGELOG shape, quoting the journal lines that
   motivated it. A quote not found in the journal is refused. A proposal never edits a gate, a decision, CLAUDE.md or
   seats.toml's constants; it may propose a seats.toml change, such as a measured expected-use cell, for the owner to
   apply. A refused proposal stays refused, whatever new evidence arrives. Because proposals sat unread for 24 days
   before, the receipt and the orchestrator's `SessionStart` digest name the open ones.
3. **Lessons.** These follow the owner's rulings of 2026-10-16: "Checked lessions should go live automatically, keep
   it per github remote assuming that project has a github ... have the lesson stay in its own project. Keep a board
   per workspace."
   - A worker's Work Report may carry a `lesson`: a name, its class, the steps (at most 120 lines) and an evidence
     quote from its own stream.
   - The gate checks it: the order passed and the quote is in the worker's stream. Only then is it written to
     `.nirvana/lessons/<slug>.md` in the project, and only when the project's repository has a GitHub remote.
   - The next order of that class gets one line in its brief: the path and the description, never the body
     (decision 0023: everything in the prompt is paid for on every call).
   - Each lesson keeps counters: offered, opened, and passed after it was opened. A lesson offered often that does
     not help decays out of the briefs.
   - `NIRVANA.md` (§6) indexes them.
4. **Where it shows.** The receipt's one-line summary and the lessons offered and written go to the Memory square
   (0.8.0). The record (§4) stays the input to seat scores; the receipt is the human view of it.

The receipt is built in O3, which owns the journal. The retro and the lessons are built in O5, because the gate
verifies them.

## 8. What the owner sees

- **The sidebar's foot**: one row per pool, now with Google's Claude pool; each row shows its tightest window, a pace
  mark when ahead of pace, and the reset time when wide enough.
- **A Seats square** in the mod strip of a Nirvana tab: each open order with its seat, model and effort, pool, state,
  time and use; its reason line; a click focuses the worker's pane.
- **The mode** (Efficient, Balanced, SOTA) in the Seats square's header, one key to change it.
- **The "new" menu** offers Nirvana, Claude Code and Antigravity. A plain Claude Code or agy tab is exactly what
  today's fork opens.
- **The Explorer verb** "Open Nirvana" opens Nirvana Claude on the folder in the herdr fork. It moves from Nirvana-C
  into this fork's `install.py`, and "Open Nirvana - CC" is removed (R8). What the verb did that Nirvana-C still
  holds moves too: the Windows Terminal profile fragment, the Nirvana themes passed by `--settings`, JetBrains Mono.
- **Its own icon** (R8, the owner: "Can wew have our own icon for this? ... have its own Icon and a toast with the
  directory first Letter as the icon"):
  - `herdr.exe` carries the Nirvana mark as its icon resource, so Task Manager and Explorer show it.
  - The window stays Windows Terminal. P7 (`nirvana/evidence/2026-10-17-o0-probes/p7-icon.md`) measured the way:
    the herdr client finds its Terminal window as the root owner of `GetConsoleWindow()`, and sets on it the window
    properties `RelaunchIconResource = <herdr.exe>,0`, `RelaunchCommand`, `RelaunchDisplayNameResource = Nirvana`, and
    last `AppUserModel_ID = Nirvana.Herdr.<folder hash>`. That window gets its own taskbar button with the Nirvana
    mark.
  - The folder's first letter is the button's overlay badge, `ITaskbarList3::SetOverlayIcon`, drawn by herdr at run
    time in a colour that stands off the mark. Windows draws it top-right. Each folder needs its own id, or two
    windows combine into one button that shows only the last badge.
  - On detach or exit herdr clears the badge, then the properties, because the Terminal window outlives herdr and a
    badge left behind sticks to Terminal's own group.
  - On open, a Windows toast "Nirvana · <folder>" with the letter as its picture: herdr's toast code gets the fork's
    own app id, registered under `HKCU\Software\Classes\AppUserModelId` with `IconUri` pointing at the mark.
  - The mark is the new Nirvana logo. The owner: "Create a logo for this not just blue N", then, of three options drawn
    by the leaf "Opus - Nirvana logo design", "The lotus looks good". It lives in `nirvana/assets/`: `nirvana.svg`
    (three petals on a crescent, on the teal tile), simplified `nirvana-small.svg` for 24 to 32 px and
    `nirvana-tiny.svg` for 16 px, PNGs at each size, and `nirvana.ico` with 16, 24, 32, 48, 64 and 256. The badge is
    `badge/badge_template_saffron_*.svg`, saffron with a near-black letter, since teal on the teal tile did not read in
    P7; the ink version is kept beside it. The right petal's tip sits under the badge; the left petal and the cradle
    carry the mark.

## 9. Jev in the loop

Jev answers yes, no, a choice or a score in about a second, for a fraction of a cent, through OpenRouter. It never
decides alone: floors of 0.7 and 0.3, and a failure is never a yes.

1. **Reads stay out of context.** `herdr jev file|files|text` already keep a judgment about big files and command
   output out of the agent's context. The orchestrator and every Claude worker get the guide. It matters most here:
   74% of the Max pool's week went to sessions over 150k tokens.
2. **Watching workers**, after O8. Every 5 minutes, for a pane worker longer than 10 minutes, Jev reads its screen's
   tail: "is this agent stuck, looping, or waiting for an answer?" A yes wakes the orchestrator. This costs no Claude
   token. The first version reads stalls from the stream instead (§7.2); this role is for what a silent stream
   misses, a worker looping while it talks.
3. **Advising the seating**, after O8. After §5.3's filters, when more than one seat is left, Jev answers a choice
   question over their strengths, the optim-jev router's pattern with a `none` option. It ships in `shadow`: the
   journal holds both picks, and the owner turns it on when they agree enough.

Roles 2 and 3 wait until the A/B has run, as Argon's review advised: in shadow they change nothing, and the first
version should not need them.
4. **Compaction for the orchestrator.** `herdr jev compact`, the 0.9.0 Stop hook, advises the orchestrator to compact
   at a task boundary (decision 0038). Compaction is Claude Code's own summary, which keeps the preserved-thinking
   rule. optim-jev's way of pruning tool calls in place does not.

Jev is not a seat: it writes no code and reads no repository.

## 10. Old Nirvana, feature by feature

| Old Nirvana | Here |
|---|---|
| Daemon, kernel, conductor (0005, 0016) | herdr's server and its journal; the orchestrator is Claude, woken by herdr |
| The Agent, the one layer the owner reads (0027) | Nirvana Claude's pane |
| Ask preview, triage seat (0009, 0030) | the orchestrator's own turn; no extra call |
| Routing table, slider, class bars (0012, 0018) | `seats.toml`, the mode, `seat pick` |
| Seat record by model and effort (0029) | the same formula from gate outcomes, in `<home>/nirvana/record.jsonl` |
| Budget: defer, reassign, ask | §5.6 |
| Local seat for chores (0007, 0013) | the `local` seat, off by default |
| Work orders, worktrees, work reports | §7.1 |
| One ask becomes several orders (0015) | §6.1 step 1 |
| Continuation as a child order (0010) | a re-seat or a send-back is a new order with a brief |
| Completion gate, held-out check, a different family reviews (0017, 0019) | §7.3 |
| Auto mode and its budgets (0024, 0025) | §12 R6 |
| The conversation reaches the worker, a question channel (0022) | a worker's question wakes the orchestrator; the owner can type into the worker's pane |
| A finished order leaves a skill (0023) | lessons in `.nirvana/lessons/`, live once the gate has checked them (§7.4.3) |
| The one-page run report (`report.py`) | `herdr run receipt` (§7.4.1) |
| The retro and its gap ledger (L16, L21) | §7.4.2: proposals with quotes from the journal, applied only by the owner |
| A directory carries its own tasks (0026) | the board and `.nirvana/runs/` in the repository; board writes gain path-level operations beside today's whole-board revision check, after durable-pi's App State |
| The seat gets a shell; Jev screens its output (0036, 0037) | the orchestrator's Bash, with `herdr jev text` |
| The Agent rolls at a task boundary (0038) | §9.4 |
| Jev decides the verify round (0039) | dropped for now: it shipped off and was never turned on |
| Jev picks the worker's browser; bladebro (0040, 0043) | a browser order's Claude worker gets bladebro's MCP config, as in 0.51.0 |
| A worker asks for a server (0041) | herdr's Servers, 0.4.0; a worker starts its server in its own pane |
| Accounts (0014) | the Accounts panel, 0.11.0 |
| Attachments, paste, drag and drop (0011) | Claude Code's own |
| The graph, the flow, the cards and tabs (0004, 0031, 0032, 0042) | herdr's panes and tabs, the Seats square, Space agents |
| Voice worker (0008) | dropped: not used since September; Claude Code has its own voice input |
| The browser UI and its tokens (0021, 0033) | replaced by herdr's terminal UI |

## 11. Packages

One feature at a time, each a minor bump with its own plan, tests and sabotage controls.

| # | Package | Builder | Verifier |
|---|---|---|---|
| O0 | Probes P1 to P5 below, before any code | orchestrator, Sonnet 5.5 low leaves | the probe's own numbers |
| O1 | Usage readings: every pool and window with reset, freshness, pace; `herdr usage --json`; the foot's new rows | Opus 5.5 high | Sonnet 5.5 medium review; controls |
| O2 | Seats: `seats.toml`, `herdr seat list|pick`, the rule, reason lines, the record | Opus 5.5 high | Opus 5.5 high review against §5 |
| O3 | Runner: `herdr seat run|status|read|stop`, panes, the journal, rate-limit stops, checkpoints, waking; the run receipt (§7.4.1) | Opus 5.5 high | live run with one claude and one agy worker, and its receipt |
| O4 | Nirvana Claude: the plugin, the launch, the "new" menu; the full port of the Explorer verb with the icon, badge and toast from P7; Nirvana-C's verb removed | Opus 5.5 high | live: the verb, the taskbar button, `/context`, a first seated run |
| O5 | The gate: axes, send-backs, held-out check, reviewer seat; the retro and lessons (§7.4.2-3) | Opus 5.5 high | the Moto A/B checks; a retro graduating on recorded journals |
| O6 | Media seats through P2's path: the `media` skill, an ffmpeg step that keeps a recording under 100 MB | Sonnet 5.5 high | a recorded UI bug diagnosed by argon |
| O7 | A/B: Nirvana Claude against plain Claude Code, the Moto meter, 3 runs per arm, plus one run longer than 5 h | orchestrator | the owner reads it |
| O8 | After the A/B: the Seats square and the mode in the mod strip; Jev's roles 2 and 3 in shadow | Opus 5.5 high (visual); Sonnet 5.5 high | Opus 5.5 high, screenshot by a leaf; the call logs |

Until O8, the statusline, `herdr usage` and `herdr seat list` show the seating (Argon's review, cut 6).

The probes, each spending a little quota:

- **P1, Argon's pool**: half answered by Argon's review (§2): 61k Argon tokens moved neither group. Two more Argon
  turns, read before and after, confirm it, and watch for agy's exhaustion message once its weekly count is known.
  Also one short Fable turn: does it move "Current week (all models)" as well as "(Fable)"?
- **P2, video**: a 30-second MP4 and a WebM screen recording in a scratch folder; agy headless on flash low and argon
  low: "list what happens, with timestamps". Do they read it, what does it cost, and does a file over 100 MB fail as
  Argon said?
- **P3, launch flags**: agy 1.4.0 with `--effort` on a Claude model; `claude -p --json-schema`; agy `--json-schema`
  on a final report.
- **P4, use per order**: one small fixed order on flash low, medium, high and argon low, medium, measured by the
  `gemini` pool's `remaining_fraction`; and on sonnet medium against g-sonnet medium. This gives the first measured
  expected-use cells and says whether flash high costs more of the pool than argon low.
- **P5, running out**: from agy's own docs and changelog, the exit code and message on a spent quota. Claude's
  `rate_limit_event` fields from an existing worker transcript. No window is spent on purpose.
- **P6, Claude's readings** (R5): what the statusline's `rate_limits` gives a Nirvana Claude for free, against
  `claude -p /usage`; then one read of `api.anthropic.com/api/oauth/usage`. The endpoint is used only if it reports
  something the other two do not, such as a per-model window or a finer fraction. herdr would then hold Claude
  Code's token in memory for the call and never write it anywhere, which changes the Accounts rule that herdr never
  reads a credential.
- **P7, the icon**: a Windows Terminal window and a console window from a shortcut, each with the Nirvana mark, a
  letter badge and a toast. Which shows the mark in the taskbar, and which takes the badge.

Jev cannot run the probes: they measure what a model or a CLI does, and Jev answers questions about text. It grades
P4's outputs instead of a Claude grader, which keeps the Max pool out of the probes. Each probe uses one tiny fixed
task at low effort.

## 12. Rulings for the owner

**The owner's answers, 2026-10-17, verbatim:**

| # | Answer | What it decides |
|---|---|---|
| R1, R2 | "Seems fine but we are going to have to choose a model with the appropriate thinking effort so we dont burn a ton of usage on small asks. Im going ot hazard a  guess that opus 5.5 medium would be the best then." and, of R1, "Based on patterns of people using this online, benchmarks so on what is the best one?" | The orchestrator is Opus 5.5 at medium. Anthropic's model-selection page starts most workloads on Opus 5.5 and names Fable 5.1 only when Opus at xhigh or max falls short; Opus 5.5 leads Fable 5.1 on every row of Anthropic's own launch table (Terminal-Bench 4.0 66.4 against 55.8), finished a migration in 9.5 h against 12 h at 51% lower cost, and defaults to medium. Medium on SWE-bench Pro scored 92.8% at $0.21 against 95.3% at $0.29 for high. Adaptive thinking spends little on a small ask at any effort. Fable is an escalation seat for one hard plan or review, since Max lets Fable use at most half the week. The orchestrator does small asks itself |
| R3 | "Makes sence." | herdr carries the run while the Max window is spent; judgments wait |
| R4 | "That works as well," | `g-opus` and `g-sonnet` are seats |
| R5 | "Thats fine but if there are any important findings with reading endpoints consider bypassing." | No endpoint by default; P6 decides whether it adds enough to use |
| R6 | "Auto mode for both is fine," | Auto mode for Claude and agy workers. agy's own auto mode is not on Windows yet (1.3.5 changelog), so an agy worker skips permission prompts inside its own worktree, in its job object, as old Nirvana's adapter did |
| R7 | "Can Jev do the probes to save token usage?" | No, see §11; Jev grades P4 |
| R8 | "Lets do the full port were at a stage now I think the herder varient is the perfered method. Remove Nirvana - CC have standard Nirvana use the herder varient." | §8: "Open Nirvana" opens the fork; Nirvana-C's verb is removed; the icon, the badge and the toast |

The questions as they were put:

| # | Question | Recommendation |
|---|---|---|
| R1 | Which Claude runs the orchestrator? | Opus 5.5 at medium. Fable 5.1 has its own weekly meter, 0% used today; P1 says whether it is truly separate, and then Fable at medium is worth a trial |
| R2 | May the orchestrator answer and make small edits itself? | Yes: questions that need a few reads, and edits of one file and a few lines. Scholar's sources say tiny tasks cost more when handed off |
| R3 | When the Max 5 h window runs out, what happens to the orchestrator? | It waits for the reset. herdr carries the run meanwhile: it seats the planned orders on Google's pools and runs the deterministic gate, and holds every judgment for the orchestrator (§6.1). Argon's review calls waiting alone a blocker, since every pool can sit idle for hours. The other choice is a stand-in on `g-opus`, Claude on Google's pool, for those judgments; it is Claude, but not Claude Code |
| R4 | Seat Claude-quality work on Google's Claude pool (`g-opus`, `g-sonnet`)? | Yes. It is the same model on a quota the old harness never spent |
| R5 | Read Claude's per-model windows from `api.anthropic.com/api/oauth/usage` with Claude Code's token? | No. `/usage` prints session, week and Fable at no cost, and the token use is a terms grey area |
| R6 | Auto mode by default? | Yes within a run: seat, gate and send back without asking, 3 send-backs per order, and a stop after 3 wakes in a row that move no order forward. Old Nirvana's 6 wakes per run would end a 12-hour run in its first hour (Argon's review, §5.1). A merge to `main` asks |
| R7 | Spend a little quota on probes P1 to P4? | Yes, about one 5 h percent of each pool |
| R8 | The Explorer verb | "Open Nirvana" opens Nirvana Claude; "Open Nirvana - CC" stays until you retire it |

## 13. Not measured, and unsure

- P4 measured one small order per seat, once. Larger orders, and Max's price per order with nothing else running,
  are not measured. The pace and reserve numbers are hypotheses.
- Argon's video limits beyond 100 MB per file, and video length, are not measured.
- Claude Code subagents cannot run on agy. Every agy order is a pane worker, which costs a pane and a launch.
- Jev's accuracy at watching screens and advising seats is unmeasured. Both roles ship in shadow.
- Reddit was not read directly; community practice comes from GitHub projects and one mirror. The cluster that
  shaped §3 and §5: ryderderder/orchestrator, razzant/claudexor, hraness/xcb, raysonmeng/agent-bridge,
  mikecubed/Hydra, oh-my-opencode, yuting0624/antigravity-for-claude-code.
- Argon's claims about agy's own tools (subagents, background daemons, web search, the image generator, the 100 MB
  view limit) come from the tool list agy gave it. They are likely right and still unmeasured; P2 and P3 check the
  ones the design leans on. Its claim that Argon's price doubles above 200k tokens of context is not on Google's
  Argon page and is unverified.
- Two repositories were read for patterns only, never as code. Oqura-ai/optim-jev gave §9's seat advisor; its
  compacter prunes tool calls in place, which Claude's preserved-thinking rule forbids. disler/durable-pi gave §7.2's
  idempotent seating, interrupted steps and phases; its mid-run model change and history forks are not taken. Its
  replay internals were not read, only its README and API.

## 14. Gemini 4 Argon's review, and what was done with it

The owner, 2026-10-17: "Actually take this design spec and have gemini argon review it." Argon at high effort, in plan
mode, read the spec and answered; `nirvana/evidence/2026-10-17-argon-review/` holds its answer and the run. Each finding
was weighed against this plan's evidence before it changed anything.

| Finding | Verdict | Where |
|---|---|---|
| F1 in-flight orders not booked, so parallel picks break the reserve | taken | §3 booked leases, §5.3 |
| F2 a spent Max window stops every pool, since only the orchestrator moves a run | taken in part: herdr carries the run and holds judgments; a stand-in is R3 | §6.1, R3 |
| F3 inlined orders hit the 32,767-character command line | taken; the review's own first run failed this way | §2, §4 |
| F4 the bar has no floor, and an empty seat list has no answer | taken | §5.3 step 2 |
| F5 agy workers start subagents and background daemons | taken: the order forbids them unless allowed, job objects end the process tree | §4 |
| F6 parallel orders pass alone and break the build together | taken: rebase onto the integration tip before the gate's tests | §7.3 |
| F7 a wake typed into the pane splices into the owner's typing | taken | §7.2 |
| F8 pool deltas cannot price one order among many | taken: price from the order's own tokens | §4 |
| F9 price schedules change: Argon's on 2026-11-30, a long-context tier | taken for the schedule; the long-context tier is unverified | §4, §13 |
| F10 `research.web` had no chain; `g-*` lacked pdf | taken | §4, §5.4 |
| Seating: argon third for load-bearing builds, before flash for bounded ones, high for debugging, first for docs, on video for visual checks | taken; docs keep a Claude edit for owner-facing text | §5.4 |
| Effort: switch family before stepping effort down | taken | §5.5 |
| Video: argon reads the video and the source, not a transcript; ffmpeg under 100 MB | taken | §5.4, O6 |
| `read.huge` follow-ups resume the agy conversation | taken, as the one exception to "never resume" | §6 |
| Pressure per window, a regularized pace, the reset discount scaled by the order, headroom at least the order | taken; the numbers are hypotheses | §5.3 |
| An unexposed bucket must not demote its seats | taken, and §2's measurement says Argon is that case | §3 |
| A passed reset must not loop on re-reads | taken | §3 |
| Overrides could dodge the price | taken: filters still apply, 3 per run | §5.3 step 4 |
| 6 wakes per run ends a long run early | taken: 3 unproductive wakes in a row | R6 |
| Defer only 30 minutes stalls a night run | taken for auto mode | §5.6 step 4 |
| A hard limit on the orchestrator's context | taken: 150k at the latest | §6.1 |
| Worktrees fill the disk and Windows handles block removal | taken | §7.2 |
| An auto run resumes without asking | taken | §7.2 |
| Cut Jev's roles 2 and 3 | taken as "after the A/B"; the owner asked for Jev, so they stay in the plan | §9, O8 |
| Cut local, Fable and GPT-OSS from the first version | taken for local and GPT-OSS; Fable stays with R1 | §4 |
| Static priors, offline tuning | taken | §4 |
| Cut the held-out check written before the build | **not taken.** Old Nirvana's M5 row passed 8 of 8 with it, against 4 of 8 for plain Claude Code, and the builder sees only failing test ids, so names do not leak | §7.3 |
| Collapse the classes into nine chains | taken; the classes stay as labels | §5.4 |
| Move the Seats square after the A/B | taken | O8 |

## 15. O1 record

Built 2026-10-17 on master 09ed59f0 (alpha 0.11.1), branch `o1-usage`, as alpha 0.12.0, by the leaf "Opus - O1 usage
readings build". Not merged.

**Measured before building (this PC, both read with a list argv, no shell).**
- `claude -p /usage --no-session-persistence --output-format text`: 2.8 s, exit 0. "Current session: 41% used ·
  resets Oct 17, 8:50pm (America/New_York)", "Current week (all models): 37% used · resets Oct 23, 12pm
  (America/New_York)", "Current week (Fable): 0% used · resets Oct 23, 12pm (America/New_York)". The zone is the
  machine's, an IANA name.
- `agy -p /usage --output-format stream-json` (agy 1.4.1): 2.0 s, exit 0. Three groups, five buckets: "Gemini Models"
  `gemini-weekly` and `gemini-5h`, "Claude and GPT models" `3p-weekly` and `3p-5h`, "Gemini 4 Argon"
  `gemini-argon-wtu-weekly`, each with `window`, `remaining_fraction` and `reset_time`. Argon's group says "For paid
  Pro plans, Argon quota is separate."
- The fork's only time crate is `time` 0.3 with `formatting`, which has no zone database. Windows' ICU (`icuin.dll`,
  `ucal_*` in windows-sys's `Win32_Globalization`, a feature the fork already turns on) resolves IANA zones.

**Shape, as built.**
- `Reading { pool, window, used: Option<f32>, resets_at, resets_text, read_at, source }`; `used` is `None` for a
  window the pool has that the read did not print. Windows are `5h`, `week`, and `week.<model>` for a week scoped to
  some models.
- `<home>/nirvana/usage.json` holds each reader's last answer (`Source`: `read_at`, `failed_at`, a word, the GPU's
  memory, the readings). The window writes it after each read; `herdr usage` reads it and runs the readers whose entry
  is missing or stale. `herdr usage --json`'s shape is the doc comment on `nirvana_usage::view`.
- The foot's row per pool shows the tightest window: the most used of the windows with no dot in their name.

**Files.** `src/nirvana_usage.rs` (readings, parsers, ICU reset, freshness, pace, cache, `herdr usage`, tests);
`src/main.rs` (the `usage` subcommand); `src/client/shell/nirvana_footer.rs` and `sidebar.rs` (a row per pool, the
mark, the reset, the muted stale figure, the foot's height from the row count);
`src/client/shell/tests/nirvana_footer.rs`; two upstream tests raised in height
(`agents_worktrees_notifications.rs`, `popup_focus_projection.rs`, each with a `Nirvana fork:` comment);
`nirvana/controls.py`, `nirvana/CHANGELOG.md`, `nirvana/VERSION`, `nirvana/README.md`, this section.

**Controls.** 48 new or rewritten (`usage_*`, `footer_week_label`, `footer_label_is_pool`, `footer_four_rows`,
`footer_no_pace_mark_text`, `footer_mark_colour`, `footer_stale_not_muted`, `footer_reset_narrow`,
`footer_reset_never`), and the 11 foot controls of 0.4.0 and 0.11.0 run again, four pointed at renamed tests.
First run, one at a time with `--only`: 57 of 59 reddened their test, and the tree was clean after. The two
that stayed green, and what was wrong:
- `usage_claude_reset_any_zone` removed the check after `ucal_getCanonicalTimeZoneID`, but that call's error
  status still stopped `ucal_open`, since ICU skips every call after a failure. The control now resets the status
  instead, and reddens: ICU does open an unknown zone as GMT without an error, so the check is what stops it.
- `footer_agents_starved` (0.4.0's): at 20 rows the foot, now 11 rows, was taller than the whole lower section,
  so the agents' floor never decided. The test composes at 22 rows; the control reddens it at 22 and at 24.
Both rerun alone: PASS. The dry-run does not check that a find occurs once; a script did: every control of this
package finds its text once. Five older controls find nothing on master 09ed59f0: `panels_both_open`,
`edit_tab_unnamed`, `memory_panel_beside_preload`, `panel_painted_over`, `jev_rows_missing`. Not touched here.

**Live run.** `NIRVANA_HERDR_HOME` a folder under the worktree's `target`, the debug build, `herdr usage --refresh`,
3.6 s, exit 0:

```text
pool       window       used  resets in  pace  read
Claude     5h            44%  2h44m      0.98  fresh 3s
Claude     week          38%  5d17h      1.72  fresh 3s
Claude     week.fable     0%  5d17h      0.50  fresh 3s
Gemini     week           1%  6d17h      0.83  fresh 3s
Gemini     5h             1%  4h29m      0.56  fresh 3s
Argon      week           2%  6d17h      0.89  fresh 3s
G·Claude   week           5%  1d12h      0.50  fresh 3s
G·Claude   5h             3%  4h34m      0.73  fresh 3s
Codex      not exposed
Local      off
```

**Departures from the brief, found while building.**
1. *Pace has no floor at a fresh window.* With nothing used just after a reset the formula gives 0.10 / 0.10 = 1.0,
   on pace, which is what the ease is for; the clamp's 0.5 applies later in an idle window.
2. *agy lists each week before its 5 h window.* The readings put 5 h first in every pool, so a tie in the tightest
   window goes to 5 h for agy's pools as for Claude's.
3. *Two buckets on one pool and window keep the tighter.* None does today; the rule keeps a pool's binding figure.
4. *The idle Claude read moved from 600 s to 240 s and agy's from 300 s to 240 s,* under the 300 s staleness line, so
   an open window keeps the cache fresh.

**Not measured.** The reset on Linux or macOS (no timestamp there; not built). Two windows writing the cache at once:
the newer-entry rule and the rename keep the file whole, but a read and write that interleave can drop the other
window's newer entry until its next read. `herdr usage` with no `claude` or `agy` on PATH (exit 1 path) was not run
live. The foot in a real window, read by a leaf from a screenshot: not done in this package. Whether the owner wants
the pace mark when any window of the pool is ahead: live, Claude's week read pace 1.72 while the 5 h window, the
tightest, read 0.98, so the row carried no mark.

The table above was read before the readings were sorted 5 h first; agy's pools now print 5 h, then the week.
