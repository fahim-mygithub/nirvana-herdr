# Plan 42: a Questions square for what the agents need from the owner

Status: PROPOSED 2026-11-14. Measured on run `ia1`. No code yet. Owner rulings Q1-Q4 open.

## 1. The ask

The owner, 2026-11-14: "Sometimes the agent has a list of questions that I end up keep having to ask over an dover
becuases it gets buried. For another nirvana module could we build some sort of module next to git that acts as a
checck list or working questions needed from the user? Use the recent megajam nirvana run as context."

## 2. Requirements ledger

| # | Quote | Surface | Owner | Verifier |
|---|-------|---------|-------|----------|
| L1 | "a list of questions" | `.nirvana/questions.jsonl`, one record per question | herdr | tests on the record and its dedupe |
| L2 | "keep having to ask over an dover becuases it gets buried" | the square shows the open count and the oldest one's age at all times | herdr | a re-run of the §3 count on the next run |
| L3 | "module next to git" | a Questions square after Git in the square strip, and its panel | herdr | a draw test and a live look |
| L4 | "acts as a checck list" | items are open, answered or withdrawn; answered ones tick off | herdr | tests |
| L5 | "working questions needed from the user" | orchestrator and worker questions both land there | herdr | a worker `asked` event appears as an item |

## 3. What run `ia1` shows

Run `ia1` on the owner's Megajam project, 2026-11-12 20:05 to 11-14 20:50 UTC, from the orchestrator's transcript and
`.nirvana/runs/ia1/journal.jsonl`. Counts are heuristic, clustered by hand, ±10.

| Measure | Value |
|---|---|
| Orchestrator end-of-turn messages | 446 |
| Messages carrying a question or decision for the owner | 128 |
| Distinct questions | about 50 |
| Questions asked in more than one turn | about 15 |
| Times the owner asked what they still needed to approve | 13 |
| "Waiting on you" footers | 48 messages |
| Question in the last quarter of its message | 49 of 66 |
| Question inside a list item | 33 of 66 |
| Median length of a question-carrying message | 1,931 chars, against 1,075 for all |

The worst cases:

- **Scope cut of L06, L11 and L12:** asked in 10 turns over 14.2 hours. It was never answered, with 160k chars between
  the first ask and the end of the run.
- **"Yes, download the assets":** 9 footers over 2 hours, then answered by "Go ahead".
- **Merge `ia1` to main:** the owner said yes 4 times, and the orchestrator kept listing it as open. An answer has no
  place to be recorded.
- **Worker questions:** 13 `asked` events. About 6 needed the owner. `c6-candle-noise` asked "Where exactly did you see
  the candle noise" and waited 367 minutes. The owner's only sign of a worker question is a Blocked row in the agents
  panel. The orchestrator's own messages named the asking order in 1 case of the 13.
- **5 owner replies were "Go with recommended".** Questions that carry a recommended option get a one-word answer.

## 4. What exists today

- **Asked order state:** a worker's question keeps its row Blocked until a later order answers it
  (`nirvana_dead::answered`). This covers workers only, as a text blob, with no list.
- **`herdr board`:** free text per workspace, with no ids and no answered state, and other agents rewrite it.
- **`herdr run notice`:** carries run events to the orchestrator's next prompt. It is the right path for an answer back.
- **Phone prompt sheet:** answers an agent's on-screen menu. Its `seen` hash stops a double tap answering twice, and that
  pattern is reusable.
- **Phone alerts:** Web Push on agent state changes. A new question can push the same way.
- **Square strip:** `src/client/shell/nirvana_mods.rs`, in the order Jobs, Preload, Learning, Memory, Git. A square is one
  `draw_square` call and one `OpenPanel` variant.

## 5. The design

### 5.1 An item

One record in `<project>/.nirvana/questions.jsonl`, append-only, with the latest record for each `id` winning:

- `id`, short and stable: `q7`;
- `run` and `source`: the orchestrator, or the order id of the worker that asked;
- `text`: one line, at most 200 chars;
- `options`, with one marked recommended, or none for an open question;
- `blocks`: the orders or steps that wait on it;
- `state`: open, answered or withdrawn;
- `answer`, `answered_at`, `asked_at`, `asked_count`.

### 5.2 How items get in

1. **The orchestrator:** `herdr ask add "<text>" [--option a --option b --recommended a] [--blocks l05]`. The same text,
   or one that `--same q7` names, bumps `asked_count` and does not make a second item. The orchestrator's role and its
   `seat` skill say to file every question this way and not repeat it in prose. ROLE.md changes are the owner's call,
   so they go in the package for approval.
2. **Workers:** every `asked` event in a run's journal becomes an item with the worker's order as its source.
3. **No parsing of prose.** The 62 "Decision for you" footers vary too much to scrape.

A Stop hook nudge, not a block: when the orchestrator's last message has a question line and it filed no item that
turn, its next prompt gets one line, "You asked the owner something without `herdr ask add`". That is Q2.

### 5.3 How the owner answers

- **The window:** the Questions square opens a panel with one row per open item, its options and the recommended one
  marked. A key picks an option. Typing gives a free answer. A finished item ticks off and drops to an answered list.
- **The phone:** the same list on the home page, under the agent cards. The prompt sheet's `seen` hash stops a double
  tap answering twice.
- **Typed to the orchestrator:** still works. The orchestrator runs `herdr ask answer q7 "<answer>"`, and the square
  ticks it off.

Only the window and the phone count as the owner answering. An agent's `herdr ask answer` is recorded as the
orchestrator's reading of a typed prompt, with that prompt's time, and the panel shows it in a different color. The
override finding on `feat/owner-override` applies: an agent can fake typing, so herdr marks only its own key press or
phone tap as the owner's.

### 5.4 How the answer reaches the asker

- **The orchestrator:** one line on its next prompt through `herdr run notice`: "q7 answered: <answer>".
- **A worker that asked:** the answer goes in the brief of its continuation order, the way an answered Asked row works
  today. An answered item clears that worker's Blocked row.

### 5.5 What the square shows

The number of open items and the age of the oldest, such as `3 · 2h`. It is yellow when one arrived since the owner
last opened the panel. A new item can also send a phone alert, which is Q3.

## 6. Packages

| Package | What | Gate |
|---|---|---|
| P42a | the record, `herdr ask add/answer/withdraw/list`, worker `asked` mirroring | tests, a sabotage control per assertion |
| P42b | the square and its panel in the window | draw tests, live look |
| P42c | the phone list and answering | page tests, live check on the iPhone |
| P42d | the orchestrator side: role and skill text, the Stop nudge, answers on its next prompt | tests; ROLE.md diff for the owner |

P42a goes first. P42b and P42c can then run in parallel, since their files don't overlap.

## 7. Owner rulings

- **Q1, the name.** "Questions", or "Asks", or "To you". Recommended: "Questions".
- **Q2, the nudge.** When the orchestrator asks in prose without filing an item, should its next prompt remind it?
  Recommended: yes. It only reminds and never blocks.
- **Q3, phone alerts.** Push a phone alert for each new question? Recommended: yes for worker questions and for items
  that block an order. Questions that block nothing only update the square.
- **Q4, accept all recommended.** One key or button that answers every open item with its recommended option?
  Recommended: no. A merge or a scope cut deserves its own press. Each item gets its own one-key "recommended".

## 8. Not done, and unsure

- The counts are heuristic. The research clustered questions by hand and did not read every message.
- Whether the rook and tin-soldier questions were answered outside this transcript is unknown.
- Earlier transcripts, from before 11-12 20:05, were not read.
- How often the owner opened the agents panel, so how often a Blocked row was seen, is not logged.
- Research scripts are in the scratchpad folder `questions` of the session that wrote this plan. They are not tracked.
