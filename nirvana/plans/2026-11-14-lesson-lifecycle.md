# Hermes's idle queue and lesson lifecycle, added to the lesson reviews (2026-11-14)

Builder: "Opus - lesson lifecycle". Branch `feat/lesson-lifecycle` off master b1cf149e (alpha 0.71.2).

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Sure lets add the top 3" (the owner, 2026-11-14), for the item "Hermes's idle queue and lesson lifecycle, added to the lesson reviews." | lessons, idle review | builder | tests and p73 controls |
| L2 | "one pending review slot per space (project), newest snapshot replaces older, dispatch only after a quiet settle window, force-dispatch after a max age" (proposal detail A, from hermes `agent/review_idle_queue.py`) | `nirvana_idle_review` | builder | tests and p73 controls |
| L3 | "a lesson unused for 14 days is marked stale and archived at 30, never deleted; pinned lessons are exempt; every lesson change is logged (append-only JSONL with before/after content) so one edit can be undone" (proposal detail B, hermes `agent/curator.py`, `tools/skill_ledger.py`) | `nirvana_lessons`, `herdr lessons` | builder | tests and p73 controls |
| L4 | "`herdr lessons pin\|unpin\|undo <id>` and an archive listing" (the brief) | `herdr lessons` CLI | builder | tests |

## What is there today, read from the code at b1cf149e

| Ask | Today | Missing |
|---|---|---|
| One pending slot per space | `SpaceState.last` holds one review per project; a tick queues at most one review on the whole machine | nothing |
| Newest snapshot replaces older | the review reads every order gated since `last.through` when it is queued; nothing is snapshotted before that, so it is always the newest | nothing |
| Quiet settle window | `IDLE_AFTER_S`, 30 min with no owner key and no order write anywhere | nothing |
| Force after a max age | none: a busy day with no 30 min gap never reviews | **added** |
| A use of a lesson | `offered` counter, with no time | **a time**, `used_at`, set when a brief offers it |
| Stale at 14 days, archived at 30 | `retire` archives a lesson offered 6 times that never helped, by an idle review's op | **a time rule**, run by the idle tick |
| Pinned exempt | none | **`pinned`**, and `herdr lessons pin\|unpin` |
| Every change in the JSONL with before and after | `ledger.jsonl` holds review, merge, retire, reclass and rollback ops | **the gate's own worker write**, and pin, unpin, archive |
| Undo one edit | `herdr lessons rollback <op>` | **`undo`**, which also takes a lesson's slug |
| Archive listing | `.archive/` files, no listing | **`herdr lessons archived`** |

## The numbers

- Force after `MAX_WAIT_S`, 6 h. Hermes forces at 30 min over a 15 s settle, about 120 settles. Here the settle is
  30 min and the review also waits for a 5 h window that resets within 2 h with half of it spare. A wait of 6 h is more
  than one whole 5 h window gone by with orders waiting.
- A forced review still needs `FORCE_QUIET_S`, 10 min, two ticks, with no owner key and no order write. It never skips
  the lease rule, the spare usage, the week, the caps or the use of the pool by anyone. Those are the owner's spend and
  the owner's "never while any order runs anywhere".
- Stale at 14 days and archived at 30: the owner's numbers in the proposal. Hypotheses.
- A lesson written before this version has no `used_at`. The first sweep sets it to the sweep's time. Counting from
  `written_at` would archive a lesson that a brief offered yesterday.

## Stale is read, not stored

`stale` is a lesson's `used_at` older than 14 days and not pinned. It shows in `herdr lessons list` and in `sweep`. The
Learning square draws in `src/client/`, which this package does not own. A stored mark would need clearing on every offer, a second index write on the seat path.

## Out of scope, reported

- A project left for 30 days loses every unpinned lesson to the archive on its first tick back. The archive keeps them
  and `undo` puts each back. Counting only the days the project ran orders would fix it; that changes L3's "14 days",
  which is the owner's call.
