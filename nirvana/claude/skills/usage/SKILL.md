---
name: usage
description: Read the quota pools before seating or waiting. Use when choosing between pools, when a window is near its end, when a worker hits a rate limit, or when the owner asks how much is left - `herdr usage --json`, pace, the orchestrator's reserve, and when to wait.
---

# Pools, pace and the reserve

`herdr` is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`. The statusline and the session digest
already show the pools; read `herdr usage` when a decision turns on a figure.

## Read

`herdr usage --json` (0 tokens; it reads again any reader older than 5 minutes, about 4 to 20 s):

- `pools[]`: `pool`, `state` (`ok`, `failed`, `reading`, `not installed`, `not exposed`, `off`), `tightest`,
  and `windows[]` with `used` (0 to 1), `resets_in_s`, `fresh`, `stale` (`old` or `reset passed`), `pace`.
- The pools: `max` (Claude Max: `5h`, `week`, `week.fable` for Fable only), `gemini` (agy's Gemini models),
  `google3p` (agy's Claude and GPT, the `g-opus` and `g-sonnet` seats: a small overflow pool), `argon` (week only),
  `local`. The foot hides `google3p` and `argon`; `seat pick` still prices them.
- `used: null` is **unknown, never 0**. A reading past its reset says nothing about the new window.
- agy prints what is left and Claude what is used; herdr turns both into `used`.

## What the numbers mean

- **Pace** = `(used + 0.10) / (elapsed + 0.10)`, clamped to 0.5 and 2.0. Above 1 the window is spent faster than it
  passes; the foot marks it.
- **The reserve**: you run on `max`. A worker on `max` is skipped when it would leave less than 15% of the 5 h
  window (`seats.toml` `reserve`, a hypothesis), and the `PreToolUse` hook refuses such a subagent.
- **The floor**: a window at 5% left or less takes no order.
- A week's 20% holds far more than a 5 h window's 20%; `seat pick` prices each window and takes the worst.
- Quota about to be lost is cheap: a window with over 30% left and under 60 minutes to its reset is discounted.

## When to wait

Signals are typed, never guessed from text: Claude's `rate_limit_event` rejected or `api_retry` with
`error=rate_limit`; agy's exit 3 with its `AGY_ERROR` line. herdr's runner acts on them (plan 12 §5.6):

1. The worker stops at a turn boundary with a checkpoint.
2. **Defer** when the window resets within 30 minutes (`defer_min`).
3. Else **re-seat** on the next seat of the chain with a continuation brief (the `handoff` skill); never the same
   conversation on another model.
4. Else, in auto mode, defer to the earliest reset with a wake a minute after it.
5. Else ask the owner. Nothing is dropped.

When `max` itself is spent, your own pane waits for the reset and goes on by itself; herdr carries the run on the
other pools meanwhile and holds every judgment for you.
