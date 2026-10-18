---
name: handoff
description: Write the continuation brief for a re-seat - when an order moves to another seat (a spent window, a send-back to a different seat, an interrupted step you choose to continue, or your own context rolling at a task boundary). Never continue a conversation on another model.
---

# The continuation brief

A new seat is a new session with a written brief, never the same conversation on another model: a thinking block is
valid only while everything before it is unchanged, and a model switch drops the reasoning the new model cannot read
(plan 12 §2). herdr writes this brief itself when a pool runs out mid-order (`<order>-r<n>`, in the same worktree).
Write one yourself when you move work by hand. `herdr` is `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`.

## The shape

Write it to `.nirvana/orders/<order>-r<n>.md`, then seat it as a new order (`seat` skill), in the parent's worktree:

```
Class: <the parent's class>
Order: <order>-r<n>
Run: <run>

# Continuation of <order>

The worker on <seat> <effort> stopped because <why: its pool ran out / the gate sent it back / it was interrupted>.
You are a new session: continue its work, do not start over. Its changes are in this folder (`git diff HEAD`).

- Where it stopped: <the last step done, and the next one>
- Its diff at the stop: <the checkpoint's file under .nirvana/runs/<run>/checkpoints/>
- The gate's reason, verbatim, when it was sent back: <...>
- Its last turn:
  > <quoted, short>

## The original order

<the order file's text, unchanged, with the files it owns and its check>
```

## Rules

- Quote; do not summarise. The owner's words and the gate's reason go in verbatim; a paraphrase is a defect.
- Name the files it owns again. A continuation owns exactly what the parent owned.
- An interrupted step is reported, never rerun: say what was cut and let the new worker check it before redoing it.
- Keep the `--not-family` of a review: a re-seated review stays with another family.
- A read-only follow-up to a `read.huge` agy session is the one exception: resume it by `--conversation <id>`
  instead, since a new session re-reads the whole input.
- Your own context: past 150k tokens, or at a task boundary, write the run's state to the board
  (`herdr board write`) and let a fresh orchestrator session take the run (decision 0038); the next session's digest
  names the open runs.
