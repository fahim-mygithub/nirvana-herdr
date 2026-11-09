# Lessons reach their orders, and an idle memory review on spare usage (2026-11-09)

Builder: "Opus high - lessons reach orders and idle review". Branch `learn-idle` off master a57fa44a (alpha 0.60.1). No
version bump, no CHANGELOG line: the merge does those.

## The owner

> "how is the memory system working is it improving the agents and storing finding in memory? Can we have it review
> during long periods of idle if we have ree usage left?"

## 1. How memory works today (read from the code at a57fa44a)

| What | Where | Written when, by whom | What the next order gets |
|---|---|---|---|
| Lessons | `<project>/.nirvana/lessons/<slug>.md` + `index.json` (counters) + `ledger.jsonl` | the gate, after a pass, from the worker's Work Report `lesson` when its evidence quote is in the worker's own stream (`nirvana_lessons::verify_and_write`); a `lesson-review` order's `lesson_ops` at its gate (`nirvana_review::gate_review`); a merge order at `herdr run end` when a class holds over 8 live lessons | one line per live lesson whose class is the order's **exactly** (`nirvana_lessons::fits`), never the body (decision 0023); `offered` +1 per brief |
| Counters | `index.json` | the gate: `opened` when the worker's tool input names the file, `passed_after_opened` when that order passed (`note_gated`) | a lesson offered 6+ times that helped in fewer than 1 of 3 offers decays out of the briefs (`live`) |
| Knowledge cards | `<project>/.nirvana/profiles/<name>/knowledge/` | `herdr profile teach` orders and lesson reviews of profiled orders (`card_ops`) | up to 15 lines to an order seated with that profile |
| The board | one file per workspace (`nirvana_board`) | any agent, `herdr board write`; a running Claude sees changed sections on its next prompt | read on demand |
| `NIRVANA.md` | `<home>/nirvana/NIRVANA.md` | the gate rewrites one line per project (live lesson count) | the orchestrator's SessionStart digest |
| `JEV.md` | `<home>/jev/JEV.md` | the owner (Preload `e`) | Jev instructions, through Preload's switch |
| Claude Code's own memory | `~/.claude/projects/<enc>/memory/MEMORY.md` | Claude Code; Nirvana only flags its size and proposes a trim (`herdr memory`); Preload can switch auto memory off | Claude Code loads it |
| Retro gaps | `<project>/.nirvana/retro/gaps.jsonl` | `herdr run end`, from the journal, no model | proposals the owner applies |

## 2. Measured (`python nirvana/lessons_census.py`, read only; rerun it to refresh)

The script reads each project's `lessons/index.json`, `ledger.jsonl` and run journals; the projects are the ones
`<home>/nirvana/record.jsonl` names. Its summary lines on 2026-11-09, verbatim:

```
== the machine, last 14 days: 10731 journal events; gaps of 30 min or more 45 (287.5 h); of 60 min or more 39 (283.5 h)
== C:\Users\fahim\Desktop\Pojects\3D-Printer Flow
   lessons 6 (live 6, class not a router class 5, never offered 6) · offered 0 · opened 0 · passed after opened 0
   orders seated 138 · gated events 123 · lesson-review orders 18 · ledger ops {'new': 1}
== C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool
   lessons 3 (live 3, class not a router class 2, never offered 3) · offered 0 · opened 0 · passed after opened 0
   orders seated 75 · gated events 20 · lesson-review orders 10 · ledger ops {}
== C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool-xcs
   lessons 0 (live 0, class not a router class 0, never offered 0) · offered 0 · opened 0 · passed after opened 0
   orders seated 9 · gated events 6 · lesson-review orders 0 · ledger ops {}
== C:\Users\fahim\Desktop\Pojects\WorkoutApp\WorkOutAppRefactor
   lessons 16 (live 11, class not a router class 5, never offered 7) · offered 39 · opened 12 · passed after opened 3
   orders seated 96 · gated events 87 · lesson-review orders 22 · ledger ops {'new': 3}
== C:\Users\fahim\Desktop\Pojects\groove-coach
   lessons 11 (live 11, class not a router class 8, never offered 9) · offered 4 · opened 5 · passed after opened 2
   orders seated 79 · gated events 63 · lesson-review orders 19 · ledger ops {'new': 3}
== D:\MotoProject
   lessons 0 (live 0, class not a router class 0, never offered 0) · offered 0 · opened 0 · passed after opened 0
   orders seated 57 · gated events 14 · lesson-review orders 7 · ledger ops {}
```

Per lesson (the script's `- ` lines): every lesson whose class is not a router class has `offered 0`; the dry run of
the fix-up below finds the seated class of every one of them in its journal.

**The class-mismatch guess is confirmed.** `fits` is `entry.class == class` for a lesson with no profile, and the
brief's class is the router's (`booked.class`). A worker writes its own words in `lesson.class`, and before this change
the gate kept them as written. No such lesson can ever be offered.

**What else the numbers say.**
- The lesson reviews do run and keep little: 76 `lesson-review` orders seated across the six projects, 7 ledger ops,
  all `new`. Plan 33's rule 1 ("nothing to keep is the usual answer") is doing what it says.
- Where lessons do reach briefs (WorkOutAppRefactor, router-class lessons), workers open them (12 opens of 39 offers)
  but 4 lessons have decayed at 6 offers with `passed after opened 0`. Every order that opened one of those was sent
  back, and its send-back children were sent back again or never gated (read from the journals); so the zero is the
  orders' outcome, not a counting fault.
- The machine is idle in long stretches: 45 gaps of 30 minutes or more in 14 days, 287 hours in all.

## 3. What was built

### 3.1 Lessons reach the orders they would help (the root fix)

- `verify_and_write` keeps a worker's lesson under **the class its own order was seated with** (`seated_class`, read
  from the journal's raw lines as `import` reads them), the worker's words kept as a `topic:` line. A journal that
  cannot say keeps the worker's words, as before. The `lesson-review` class is never a lesson's class.
- `herdr lessons reclass [--dry-run]`: each worker lesson whose class is not one `seats.toml` names
  (`router_classes`, read without writing) moves to its order's seated class, in **one ledger op** (`reclass`) that
  `herdr lessons rollback <id>` undoes byte for byte. A review's or merge's lesson is left. The idle review runs it
  before its brief, so a project's old lessons are fixed the first time it is reviewed; the owner can run it now.
- Dry run on this machine (read only), the moves it would make: 3D-Printer Flow 5, 3D-Printer-Flow-xtool 2,
  WorkOutAppRefactor 5, groove-coach 8, MotoProject 0; left 0 everywhere.
- Decision 0023's one line per lesson and the decay rule are unchanged; offering is still by exact class (now the
  router's).

### 3.2 Idle review on spare usage (`src/nirvana_idle_review.rs`)

(The review round changed this rule; §6.2 is the rule as it stands, §6.4 the live check. This section is the first
build, kept as built.)

The rule is one pure function, `decide`, over `Facts`; the server starts `herdr lessons idle tick` as a windowless child
every 5 minutes (`start`, one line in `server/headless.rs`); the tick reads files only, queues at most one review, and
takes a lock so two ticks never overlap. In order, a review runs only when:

1. idle review is on (`herdr lessons idle on|off`, `<home>/nirvana/idle-review.json`, default on);
2. **no order runs anywhere**: no live lease in `leases.json`;
3. **no owner key in any pane for 30 minutes** (`<home>/nirvana/keys/`); a key file that cannot be read is a doubt and
   no review;
4. **no journal or stream written in any known project for 30 minutes** (a worker that only streams is still running);
5. at least one order was gated in this project since its last idle review (the first reads back 7 days);
6. `lesson_reviews_per_day` still has room (an idle review is a lesson review and counts there too);
7. fewer than **2 idle reviews in this project this UTC day**, and fewer than **3 across the machine in this 5 h window**
   of the pool it would run on;
8. the seat it would take (the lesson review's own pick: Efficient, small, on `cached_world`, the readings the open
   window keeps fresh) has a **fresh 5 h reading** of its pool, **at least 50% projected unused**, and the window
   **resets within 2 hours**. A missing, old (over `STALE_AFTER`, 300 s) or reset-passed reading is no review.

When it runs: the reclass, then a run of its own (`idle-YYYYMMDD-HHMM`, `opened` with no orchestrator pane), a digest
per order (the last 24,000 characters of the stream's own strings, never a whole transcript), a brief with the newest 12
orders' verdicts, reasons and Work Reports, every live lesson with its body and base sha, and the "Never helped"
lessons; then `nirvana_review::queue_idle` seats it as every lesson review is seated, on the same world the gate read.
At its gate (`gate_review`, subject `@idle`) `apply` takes `new` and `patch` ops through `apply_review`'s evidence
checks against the stream of the order they name (`from`), or the first given order whose stream holds the quote, with
the class set to that order's; `merge` through `nirvana_lessons::merge` with the brief's shas; `retire` through
`nirvana_lessons::retire` (only a lesson offered 6+ times with `passed after opened 0`; archived, ledgered,
rollback-able). Then `finished` writes the result into `runs/.idle-review.json` and ends the run.

The Learning view carries `idle` (`IdleView`): on or off, when the last idle review ran, on which seat, how many orders
it read, what it did (`pass · 2 applied (op-7: new …; op-8: retired …) · 1 refused`), and the last check's answer
(`skipped: the owner typed 4m ago; idle after 30m`). The panel's lines come from `nirvana_idle_review::lines`; drawing
them is the hunk in §5.

### 3.3 The numbers, each a hypothesis

| Number | Value | Why |
|---|---|---|
| `IDLE_AFTER_S` | 30 min | the owner's "near 30 min"; Claude's prompt cache lives an hour (plan 27), so an owner back inside it loses nothing; 45 such gaps in 14 days |
| `TICK` | 5 min | a tick is file stats and no model; a review lands in the first sixth of an idle hour |
| `SPARE_MIN` | 50% projected unused | the owner's example |
| `RESETS_WITHIN_S` | 2 h | the owner's "within the next ~2 h": spend that the reset would lose |
| `PER_SPACE_PER_DAY` | 2 | a review reads everything since the last; a second reads only what ran between |
| `PER_WINDOW` | 3 per 5 h window | a flash-low lesson review was 20k-50k tokens (live checks 2026-11-01 F8) |
| `MAX_ORDERS`, `DIGEST_CHARS`, `LOOKBACK_S` | 12, 24,000, 7 days | bound the brief; median stream in 3D-Printer Flow is 2.5 MB |

## 4. The owner's live check, in order (first build; superseded by §6.4)

1. `herdr lessons reclass --dry-run` in each project; then `herdr lessons reclass` where the moves look right. Note the
   op id it prints; `herdr lessons rollback <id>` undoes it.
2. `herdr lessons for --class build.bounded` in 3D-Printer Flow: the moved lessons are listed.
3. Seat one `build.bounded` order there: its brief has a "Lessons from earlier build.bounded orders" section; the
   Learning square's offered count rises.
4. `herdr lessons idle status`: on, and each project's last check.
5. Leave herdr open (so usage stays read) with no order running and no typing for 30+ minutes, inside 2 h of the cheap
   seat's 5 h reset with half of it unused; within 5 minutes an `idle-…` run appears in the panel with one
   `…-review` order on flash low (or sonnet low).
6. When it is gated, the Learning panel's "Idle review" lines say when it ran and what it did; `herdr lessons
   rollback <op>` undoes any op it applied.
7. `herdr lessons idle off`; the next check says "idle review is off".

## 5. Not built here, and why

- The Learning panel's drawing (`src/client/shell/nirvana_learning.rs`) and the Work Report schema's `retire` op and
  `from` field (`src/nirvana_worker.rs` `REPORT_SCHEMA`) are other owners' files: exact hunks in the builder's report.
  Without the schema hunk a claude reviewer cannot emit `retire` (the schema's `op` enum is new, patch, merge); agy and
  the other ops work.
- The switch is a CLI and a file, not a Learning square menu item: the square's menu is client code.

## 6. Review round (2026-11-09, `learn-review.md` against 14b42571)

Builder: "Opus high - lessons and idle review round". Every finding was checked against the code before it was fixed;
the reviewer was right on every one (§6.1 says where a fix differs from the suggestion).

### 6.1 What each finding came to

| Finding | Checked | What was done |
|---|---|---|
| S1 busy fails open | right: `cached_world` reads an unreadable `leases.json` as no leases | `tick_in` reads `leases.json` itself; an error is `Facts.doubt` and no review. Also a doubt: a runs or streams folder that exists and cannot be listed (`activity` is now a `Result`), and a `record.jsonl` that exists and cannot be read (`known_projects_in`). `cached_world` is unchanged |
| S2 week not judged | right | `Spare.week`: each week window of the seat (`week`, `week.fable`) must be read fresh and at least `WEEK_SPARE_MIN` (25%) projected unused at its own reset; a seat with no week reading is no review |
| S3 failed queue loops | right | the try is counted before the queue (the S4 reservation) and stays counted; `backoff_until` (1 h) stops the next tries; `last.through` goes back so the orders are read later; `queue_on` gives back the lesson-review count and the pending row when `book` or the journal write fails (all reviews, not only idle ones) |
| S4 crash double-runs | right | the reservation (day count, the pool's window count, `last` with the run, order and `through`) is written before `queue`; `settle` ends a reservation that never reached the journal after `STRANDED_S` (15 min) as "never seated" |
| S5 owner's use outside herdr | right | the tick keeps each pool's fresh 5 h readings for 90 min (`Global.samples`); no review when the seat's pool has no reading from 30 min ago in the same window, or when any pool's `used` rose more than half a point since then. Not built: a pane's "working" state (the tick is a child process with no server state) |
| S6 reclass rewrites owner edits | right | the automatic reclass moves only a lesson whose bytes are what herdr last wrote (the newest writes-log sha, or the newest ledger `after`); the rest is left and listed as edited. `herdr lessons reclass --edited` moves them for the owner; `with_class` keeps every byte but the class line (CRLF, trailing lines) |
| N1 `through` skips unread orders | right | recorded, not changed: the newest 12 are read and the rest only counted in the brief, on purpose. A backlog older than the newest 12 is not worth a paid read; the next review starts after the newest |
| N2 cheap checks first | right | `ready` checks every fact but the order count; `gated_since` (the journal replay) runs only when `ready` passes; the pick runs once per tick, not per project. No test: what it saves is time, not behaviour |
| N3 activity, window, projects | right | the window count is per pool (`Global.windows`); `known_projects_in` skips roots under the temp folder, roots with no `.nirvana` now, and a second spelling of one root on Windows. Not changed: `activity` still stats every stream (fine at today's counts; recorded) |
| N4 lost updates | right | one writer: `change_space` / `change_global` read, change and write under `<file>.lock`; the tick, `settle`, `finished` and `herdr lessons idle on/off` all go through them. No test (a race) |
| N5 launch failure counts as success | right | `queue_on` journals an idle review that did not launch as interrupted, releases its lease, gives back the count and returns `Err`; `settle` ends a review seated and never started once its lease is gone and `STRANDED_S` has passed (not when `leases.json` was unreadable). An ordinary lesson review's launch failure is unchanged |
| N6 rollback resets counters | right | a `reclass` rollback restores the class and file and keeps the counters and `seen` earned since |
| N7 crowding sooner | right | recorded: live-check step 3 below |
| N8 bare order id | right | a bare `from` that names two given orders refuses the op ("name it run/order") |
| N9 tick has no timeout | right | `start` kills its own tick child after `TICK_TIMEOUT` (10 min). No test: `start` does nothing under test |
| N10 product wiring untested | right | tick-level tests for the owner key, a fresh stream, a pool's rise and the lock; tests for the project list, the per-pool window, the never-started settle; a control on each wiring line |

### 6.2 The rule as it stands (`ready`, then the order count), cheapest first

1. idle review is on;
2. no doubt: `leases.json`, each project's runs folder and `record.jsonl` all read;
3. no live lease anywhere;
4. no owner key in any pane for 30 minutes; a key file that cannot be read is a doubt;
5. no journal or stream written in any known project for 30 minutes;
6. **no pool's 5 h use rose by more than half a point in the last 30 minutes, and the seat's pool has a reading from
   30 minutes ago in this window** (S5; the first half hour after the server starts never reviews);
7. no back-off from a failed try here (S3);
8. `lesson_reviews_per_day` has room; under 2 idle reviews here this UTC day; under 3 in this pool's 5 h window;
9. the seat's 5 h window: read fresh, 50%+ projected unused, resets within 2 h;
10. **every week window of the seat: read fresh, 25%+ projected unused at its reset** (S2);
11. at least one order gated here since the last idle review (read only now: N2).

"Usage that would be lost anyway" is true of the 5 h window only: what a review spends there is gone at the reset if
unused. The week keeps counting the same tokens until its own reset; rule 10 is why that is acceptable.

### 6.3 The numbers added, each a hypothesis

| Number | Value | Why |
|---|---|---|
| `WEEK_SPARE_MIN` | 25% projected unused | a review is under 1% of a week; the floor is a margin against a straight-line projection. The Max week of 2026-10-18 (48% used at 32% elapsed) projects 0% spare and refuses |
| `USAGE_RISE` | 0.5 point | Claude's `/usage` reads whole percents: any rise it can show is over it |
| samples kept | 90 min | three idle spans: a baseline 30 min old survives two missed ticks |
| `BACKOFF_S` | 1 h | a corrupt `leases.json` or a held lock costs at most the day cap's two tries |
| `STRANDED_S` | 15 min | longer than `TICK_TIMEOUT`, so a slow tick is never taken for a dead one |
| `TICK_TIMEOUT` | 10 min | a tick is file reads; one past ten minutes is stuck (most likely a gate in `settle`) |

Measured (`python nirvana/lessons_census.py`, rerun 2026-11-09 after the round; its new `reclass:` line is S6's split
of each project's worker lessons that are not under a router class), verbatim:

```
== the machine, last 14 days: 10545 journal events; gaps of 30 min or more 45 (287.5 h); of 60 min or more 39 (283.5 h)
== C:\Users\fahim\Desktop\Pojects\3D-Printer Flow
   lessons 6 (live 6, class not a router class 5, never offered 6) · offered 0 · opened 0 · passed after opened 0
   orders seated 138 · gated events 123 · lesson-review orders 18 · ledger ops {'new': 1}
   reclass: automatic 5 · left for the owner (not as herdr wrote it) 0
== C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool
   lessons 3 (live 3, class not a router class 2, never offered 3) · offered 0 · opened 0 · passed after opened 0
   orders seated 77 · gated events 22 · lesson-review orders 11 · ledger ops {}
   reclass: automatic 2 · left for the owner (not as herdr wrote it) 0
== C:\Users\fahim\Desktop\Pojects\3D-Printer-Flow-xtool-xcs
   lessons 0 (live 0, class not a router class 0, never offered 0) · offered 0 · opened 0 · passed after opened 0
   orders seated 9 · gated events 6 · lesson-review orders 0 · ledger ops {}
   reclass: automatic 0 · left for the owner (not as herdr wrote it) 0
== C:\Users\fahim\Desktop\Pojects\WorkoutApp\WorkOutAppRefactor
   lessons 16 (live 11, class not a router class 5, never offered 7) · offered 39 · opened 12 · passed after opened 3
   orders seated 96 · gated events 87 · lesson-review orders 22 · ledger ops {'new': 3}
   reclass: automatic 2 · left for the owner (not as herdr wrote it) 3
== C:\Users\fahim\Desktop\Pojects\groove-coach
   lessons 11 (live 11, class not a router class 8, never offered 9) · offered 4 · opened 5 · passed after opened 2
   orders seated 79 · gated events 63 · lesson-review orders 19 · ledger ops {'new': 3}
   reclass: automatic 4 · left for the owner (not as herdr wrote it) 4
== D:\MotoProject
   lessons 0 (live 0, class not a router class 0, never offered 0) · offered 0 · opened 0 · passed after opened 0
   orders seated 57 · gated events 14 · lesson-review orders 7 · ledger ops {}
   reclass: automatic 0 · left for the owner (not as herdr wrote it) 0
```

The lessons left for the owner have no writes-log line or ledger row matching their bytes. The script cannot tell an
owner's edit from a lesson written before herdr kept its writes log; `git log -p` on each file (they are tracked)
can. The script's per-lesson lines (`herdr T/F`) name them.

### 6.4 The owner's live check, in order

1. In each project, `herdr lessons reclass --dry-run`: the moves, and the lessons left as "edited since herdr wrote
   it". For each one left, `git log -p .nirvana/lessons/<slug>.md` shows whether you edited it; if not (it predates
   the writes log), `herdr lessons reclass --edited` moves it too. Note the op id; `herdr lessons rollback <id>`
   undoes it and keeps the counters earned since.
2. `herdr lessons for --class build.bounded` in 3D-Printer Flow: the moved lessons are listed.
3. **Crowding (N7):** in WorkOutAppRefactor, `herdr lessons list` after the reclass. A class with more than 8 live
   lessons queues a merge order at the next `herdr run end`, and each brief of that class carries one line per live
   lesson. If that is sooner than wanted, say so: a per-project delay before the first merge is not built.
4. Seat one `build.bounded` order in 3D-Printer Flow: its brief has a "Lessons from earlier build.bounded orders"
   section; the Learning square's offered count rises.
5. `herdr lessons idle status`: on, and each project's last check. Right after a server start it says "no usage
   history to rule out other use" for half an hour: expected (rule 6).
6. Leave herdr open (so usage is read) with no order running, no typing and no Claude Code use anywhere (the phone
   too) for 30+ minutes, inside 2 h of the cheap seat's 5 h reset with half of it unused and its week a quarter or more
   unused. Within 5 minutes an `idle-…` run appears in the panel with one `…-review` order.
7. A second check of the S5 rule: during such a stretch, send one prompt in a Claude Code window outside herdr. The
   next check says "Max's 5h use rose … someone is using it" once the reading moves (Claude reads whole percents, so a
   tiny prompt may not move it).
8. When the review is gated, the Learning panel's "Idle review" lines say when it ran and what it did; `herdr lessons
   rollback <op>` undoes any op it applied.
9. `herdr lessons idle off`; the next check says "idle review is off". `herdr lessons idle on` again.

### 6.5 Not done in this round

- S5's second signal (a pane whose agent is working) is not built: the tick is a child process and reads files only.
- N1 is kept as built (above); N3's bound on `activity` is not built.
- N2, N4 and N9 have no test: time saved, a race, and a thread that does nothing under test.
- A non-idle lesson review whose launch fails still holds its lease until it expires (the old path; N5 was scoped to
  idle reviews).
