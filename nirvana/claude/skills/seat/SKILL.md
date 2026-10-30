---
name: seat
description: Seat an order on Claude or Antigravity through herdr. Use for every order you hand off - classify it, run `herdr seat pick`, start it with `herdr seat run` or a worker subagent, read it, and gate it with `herdr gate`.
---

# Seating an order

`herdr` below is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`; the session digest names it too.
A `herdr` found on PATH may be upstream herdr, which has none of these commands.

## 1. Classify (your own turn, no extra call)

Pick the **class**, the **needs** and the **size**; herdr picks the seat (plan 12 §5.1).

| Class | For |
|---|---|
| `plan` | splitting an ask into orders |
| `build.load_bearing`, `build.bounded`, `build.frontend` | code: core and risky, contained, UI |
| `fix.debug` | a root cause |
| `ops.terminal` | running and fixing commands, servers |
| `review.adversary`, `acceptance.author` | a review by another family; a held-out check written before the build |
| `research.read`, `research.web`, `read.huge` | the repository; the web; an input over 400k tokens (`context_1m`) |
| `judge.visual`, `media.video`, `media.audio` | screenshots; video (`video`); audio (`audio`): the `media` skill |
| `write.docs`, `sweep.mechanical`, `chore.memory`, `chore.script`, `chore.github` | docs; mechanical sweeps; chores |

Size: `small` (minutes), `medium` (under an hour), `large`. A question is answered, never built: a question is
`research.read`, and you answer it yourself when a few reads do.
Web research, shopping and reading pages are `research.web`: seat them, never browse yourself for more than one page.

## 2. Pick

```
herdr seat pick --class build.bounded --size small [--needs video] [--not-family anthropic] --book <order> --run <run> --json
```

It prints the seat, model, effort, pool and the reason line, and books the order's expected use so parallel picks
see each other. When the order fits one of the project's profiles (the session digest lists them; `herdr profile
list`), add `--profile <name>`: its `class` stands in when you name none, its `needs` join yours, its model or family
narrows the seats and its effort replaces the class's. Exit 3 with a profile names it: no seat fits that profile. Say the pick in one line: `build.bounded -> sonnet medium (max): <reason>`. Exit 3 is no seat:
its `earliest_reset` says when; defer, or ask the owner. To overrule, `--override <seat> --reason "..."`, at most 3
a run.

## 3. Start it

Write the order to `.nirvana/orders/<order>.md`: the goal, the files it owns (a `build.*`, `fix.*` or `plan` order
must carry an `owns:` line; `herdr seat run` refuses without one), the check that proves it, and the
owner's words where they matter. Then:

- **A pane worker** (anything longer than a few minutes, and every agy or local order):
  `herdr seat run <seat> --order .nirvana/orders/<order>.md --order-id <order> --run <run> --class <class> --size <size> --worktree`.
  It reuses the booking, opens a pane, and returns at once. A second `seat run` of the same order returns the first.
  An order that drives Bambu Studio or xTool Studio adds `--needs bambu-studio-beta` or `--needs xtool`, the
  project's `.mcp.json` server, so its worker gets the server's tools. An order that may press Send adds
  `--send-ok "<the owner's words>"`, copied exactly from the owner's prompt of the last 6 hours (4 words or more);
  herdr refuses a quote it cannot find, and a send-back or reseat never inherits it.
  An order picked with `--profile <name>` is run with the same `--profile <name>`: herdr copies the profile into the
  run, gives the worker its persona (a system prompt on Claude, the top of the order on agy), and a send-back or
  reseat keeps the copy. Design one with `herdr profile new <name>`, then `herdr profile check <name>`.
- **A worker subagent** (a short read, review or chore on Claude that needs no pane): spawn
  `nirvana:worker-<seat>-<effort>` with the seat and effort the pick gave (`opus-high`, `opus-medium`,
  `sonnet-medium`, `sonnet-low`, `haiku-low`). Every subagent is seated first: `herdr seat pick --book <order>`,
  then spawn the seat it gives, or `herdr seat run` when it gives an agy seat. The hook refuses a spawn without a
  booking. Start its brief with three lines, `Class: <class>`, `Order: <order>`, `Run: <run>`; herdr records its
  outcome under them when it stops. A spawn that names no model or effort, or that would break your reserve on `max`,
  is refused by the `PreToolUse` hook.

## 4. Read it

End your turn while workers run; herdr wakes you when an order delivers, asks, fails or stalls.

- `herdr seat status [--run <run>]`: each order's phase, seat, tokens.
- `herdr seat read <order> [--tail 40]`: the worker's stream, readable.
- `herdr run status <run>`, `herdr run check <run>`: the run and its next step.
- `herdr seat stop <order>`: stops it with a checkpoint; never rerun an interrupted step blindly.

## 5. Gate it

- `herdr gate run <order> --run <run>`: rules on a delivered order. Exit 0 passed; 1 sent back or asked, with the
  reason; 4 waits for a review or a held-out check.
- `herdr gate merge <order> --run <run>`: merges a passed order into the run's integration branch. A merge to
  `main` asks the owner, except in auto mode within its budgets.
- `herdr gate status [<order>] --run <run>`.

The `gate` skill holds the axes and the send-back loop. When the plan is done and gated, `herdr run end <run>`
writes the receipt; name its file in your answer.
