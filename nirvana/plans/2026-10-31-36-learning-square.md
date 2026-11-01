# Feature 36: a Learning square that shows the self-learning loop

Status: **BUILT (2026-11-01, alpha 0.47.0)**. Was PLANNED (2026-10-31). One package, P36, on master 9304a821 (alpha 0.46.0), in parallel with plan 35.
The loop it shows is plan 33's (lessons kept without a remote, the post-gate `lesson-review`, `lesson_ops`, merges,
the ledger and rollback) and plan 34a's (knowledge cards, `profile teach`, `card_ops`, `--adopt`).

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Also for the learning scheme we brought from hermes ensure we have a module that is tracking this beor ethe git module and ater the preload module." | the strip and a Learning panel | P36 | §3 tests; §5 live check |

- "Module" is read as a square in the task strip, with a panel behind it, like the Jobs and Git squares (plans 15
  and 16). Today the strip draws Running, Seats, Jobs, Preload, Memory, Git (`src/client/shell/nirvana_mods.rs`
  `render_strip`). The Learning square goes directly after Preload, so it sits after Preload and before Git; Memory
  follows it.

## 1. What it shows

The project is the tab's folder, as the Git square finds it. Every read is local files and the run journals: no
model call, no network, no seat, no herdr command that writes.

1. **The square.** Label `Learning`; value, in words, the live lesson count and today's reviews against the cap
   (`lesson_reviews_per_day` from the parsed seat table), for example `14 lessons · review 3/20`, a narrow form, and a
   mark when the last review today was held back, failed, or refused every op. `no lessons yet` when there are none.
2. **The panel**, sections in this order:
   - **Today:** reviews queued, gated, done, held back or failed, against the cap; ops applied and refused, with the
     refusal reasons counted.
   - **Recent changes:** the last ledger rows of lessons and of every folder profile's cards (`knowledge/ledger.jsonl`),
     newest first: id, op (`new`, `patch`, `merge`, `rollback`, card ops), slug, the order that caused it, when.
   - **Lessons:** one row each, live first: name, origin, class and profile scope, offered / opened / passed after
     opened, decayed or live, using the same counters and `live()` the offer uses (no second definition).
   - **Profiles:** each profile, its card count, cards herdr does not know (the `--adopt` line), and the warning cards.
   - **Waiting:** crowded classes a merge would take, if plan 33 exposes that count; otherwise leave it out and say so.
3. **Keys**, matching the other panels: `j`/`k` move, `Enter` or `o` opens a lesson or card in the edit tab, `u` on
   a ledger row asks in the panel (no browser dialog, no modal) "undo <id>?" and then runs the same rollback as
   `herdr lessons rollback` / `herdr kb rollback`, journaled the same way, `Esc` closes.

## 2. How it reads

- A cached view like `nirvana_git::cached`: built off the draw path, refreshed when the lessons folder, a profile's
  `knowledge/` folder or the run journal changes (mtime or size), and at most every few seconds. Drawing never reads
  a file. The constants (refresh interval, row caps) sit beside the existing ones.
- It reuses plan 33's and 34a's readers (`read_index`, `ledger`, `stats`, `live`, the knowledge index and ledger,
  the cap read). Where a reader is private, make it `pub(crate)` rather than copy it.

## 3. P36: the package

Owns a new `src/nirvana_learning.rs` (the view and its cache), a new `src/client/shell/nirvana_learning.rs` (square
value, panel render, keys), their `mod` lines, and the strip and shell wiring in `src/client/shell/nirvana_mods.rs`,
`composition.rs`, `state.rs`, `mouse.rs` and the shell's input routing; the visibility changes it needs in
`src/nirvana_lessons.rs` and `src/nirvana_knowledge.rs` (visibility only; anything more is a hunk in the report);
`nirvana/controls.d/p36_learning.py`; the record `nirvana/plans/2026-10-31-36-records/p36.md`.

Plan 35 is building in parallel: P35a owns `src/nirvana_gate.rs`, P35b owns `src/nirvana_agy_tools.rs`,
`src/nirvana_gui_tools.rs`, `src/nirvana_worker.rs`, `src/nirvana_profiles.rs` and a `mod` line in `src/main.rs`.
P36 touches none of those except its own `mod` line; that line will meet P35b's at merge and is joined there.

Tests, each with a sabotage control: the square's place in the strip (after Preload, before Git), its value words
and narrow form, every panel section from a fixture project (lessons, a ledger with each op kind, a folder profile
with known, unknown and warning cards, reviews done, held back and over the cap), the counters agreeing with
`herdr lessons stats`, `u` asking first and then rolling back exactly that row, no file read on the draw path, and
nothing written by opening or browsing the panel.

## 4. Order

P36 now, in parallel with plan 35. Merge P36 when green; plan 35's packages merge on their own.

## 5. Live check for the owner

Open a Nirvana tab on a project with lessons (3D-Printer Flow after its import, or MotoProject's reel-editor cards):
the Learning square sits between Preload and Memory, its counts match `herdr lessons stats`, and the panel lists the
reel-editor's 40 cards.
