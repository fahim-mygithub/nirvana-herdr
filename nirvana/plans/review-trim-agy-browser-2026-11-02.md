# A lesson review only when there is something to learn; agy seats take browser orders, 2026-11-02

Branch `review-trim-agy-browser` from master `871acaf2` (alpha 0.52.0). No version bump and no `nirvana/CHANGELOG.md`
entry here: the merge does that. The evidence is `nirvana/plans/live-checks-2026-11-01.md` (F1, F5, F8, check 35.1)
and plan 35 §4 R2. The owner, 2026-11-02, on the recommendations: "Use your recomendation for all of these".

## F8: the lesson review

### The rule

A review is wanted when there is something to learn:

- the verdict is not a pass (a send-back; the gate queues no review after an `asked`, as plan 33 §3.3 says, and this
  does not change that);
- the order's chain holds a correction: a `send_back` parent anywhere above it, or a parent gated other than pass,
  walking up through `reseat` parents too (`nirvana_gate::chain` stops at a reseat, so a reseat child of a redo would
  otherwise read as a first pass);
- the Work Report names any of `gotchas`, `lesson`, `lesson_ops`, `card_ops` (a non-empty list, object or string).

A clean first-pass delivery with none of these is skipped, said as `nothing to learn: a first-pass delivery whose
report names no gotchas, lesson, lesson_ops or card_ops`. The skip is decided before the cap, so it is never queued,
never counted against `lesson_reviews_per_day`, and the Learning square's `review n/cap` (`used_today`) does not move.

`lesson` and `card_ops` are in the list though the work order names only "gotchas or lesson ops". A worker's own
`lesson` and a profiled worker's `card_ops` are the worker saying it learned something, and the review brief offers
the reviewer that lesson to patch into an existing one. On the journals below the two extra keys change no row: every
report that offered a `lesson` also named gotchas. This is a reading of the rule, reported, not a change to it.

### Where it lives, and the gate hunk

The decision is `nirvana_review::idle`, called from `wanted_with`. `wanted` itself cannot hold it: the gate calls
`wanted(project, state, known, table)` before it journals the verdict, in both the pass path and the send-back path,
and nothing in `known` or `state` says which path it is in (the verdict is not journaled yet, the phase is `Delivered`
in both). So:

- `wanted` keeps its signature and its old meaning (the verdict unknown: every review the cap allows), so this branch
  builds while `src/nirvana_gate.rs` is order G's.
- `wanted_for(project, state, known, verdict, table)` is the decision with the verdict. The gate hunk (in the report)
  moves both gate calls to it; then the skip is journaled in the `gated` event as the `review_skipped` axis, and
  `wanted` has no caller and goes (the second hunk, in this file).
- Until the hunk lands, `queue_review` asks `wanted_for` itself as a backstop: a clean first pass seats no review and
  counts nothing, and its line in the ruling's `next` (`lesson review not queued: nothing to learn: ...`) is the only
  word of it; the `gated` event holds no `review_skipped` axis.

The hunk was checked in a throwaway worktree at this branch's files (removed afterwards): with it applied, `cargo
nextest run` passed everything except the known `windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override`,
and clippy with `-D warnings` was clean. Every gate path that calls `wanted` is one of the two the hunk changes
(`grep "nirvana_review::wanted"`: `src/nirvana_gate.rs` lines 3245 and 3298, nothing else).

### Measured on real journals (read only)

`measure.py` (in the builder's private TEMP, not committed) read each `seated` of class `lesson-review` with `why`
`lesson_review`, its subject's first `pass` or `sent_back` gate, the subject's delivered report, its chain, and the
review's own `gated` axes (a `lesson_op` or `card_op` axis marked pass other than "nothing to keep" is a kept lesson).

| Journals | Reviews | Skipped by the rule | Skipped that kept a lesson (the cost) | Reviewed that kept a lesson |
|---|---|---|---|---|
| `3D-Printer-Flow-xtool\.nirvana\runs\*` (f2u-setup) | 4 | 0 | 0 | 0 |
| `3D-Printer Flow\.nirvana\runs\*` (quad-pegprep, quad-split, quad3) | 3 | 0 | 0 | 0 |
| the live checks of 2026-11-01 (`scratchpad\next\live\proj\.nirvana\runs\*`) | 10 | 4 | 0 | 0 |
| all | 17 | 4 | 0 | 0 |

- The two printer projects: every one of the 7 subjects named gotchas (2 were send-backs), so the rule skips none
  there. The claim that the reviews there were of clean first passes does not hold; F8 was the live checks' finding.
- The live checks: the 4 skipped are `lc1-agy-blade-lr`, `lc1b-agy-blade-lr`, `lc2-sonnet-blade-lr` and
  `lc4b-agy-rules-lr`, 41,423 + 24,517 + 39,796 + 20,416 = 126,152 fresh tokens (input + output, the last `usage` of
  each) of the 10 reviews' 338,721, 37%. The redos `lc3-agy-mcp-s1` and `lc4-agy-rules-s1` passed clean but have a
  send-back above them, so they stay reviewed. `reel-motion-gemini-lr` was seated and never gated (stopped).
- None of the 17 reviews kept anything, so the cost measured here is 0 lessons; it is 17 reviews, not a bound.

## R2: agy and the browser

- `browser` is added to the four enabled agy seats, `g-opus`, `g-sonnet`, `argon` and `flash` (`gpt-oss` is off).
  `research.web` is not rescored: live check 35.1 is one order. Its `why` says so, and a comment above the agy seats.
- Who reads the capability list: `nirvana_seat::candidate` ("lacks <need>"), the unknown-need check (`browser` was a
  word already, from opus and sonnet), `nirvana_gui_tools::capabilities` / `seat_needs` / `seated_servers` (the same
  word set, so no need moves between "capability" and "server"), `seat list --json` and `nirvana_jev_roles` (print
  it). The Seats square does not read it. `nirvana_bladebro::grant` reads the needs, not the capabilities, and
  granted agy a browser before this.
- A desktop server need still skips agy: `bambu-studio-beta` and `xtool` set `claude_only` in `seat_needs`
  (`nirvana_gui_tools`, plan 29 §5.2), which `candidate` checks after the capabilities, whatever a seat lists. The
  test sets `claude_only` on a `--needs browser` order and sees flash skipped with the desktop reason.
- `seat pick --class research.web --needs browser` on the stand-in world now gives an agy seat; on plan 35c's table
  (v8) it gave a claude seat with flash "lacks browser" (F1).
- The seat skill now says `--needs browser` is the way to a browser on agy, and that a desktop need still takes a
  claude seat. Its agy read-method line names `--needs bladebro` or `--needs browser`.
- **The installed table.** `seats.toml` upgrades itself only when the installed copy equals a past default. Plan 35c
  changed the default's `why` line and added no copy, so an install that held plan 34a's default stayed there: the
  owner's (`%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\seats.toml`, read 2026-11-02) equals plan 34a's default
  byte for byte apart from line endings, and differs from master's only in that `why`. This branch adds
  `nirvana_seats_v7.toml` (plan 34a's, from `40917ee3^`) and `nirvana_seats_v8.toml` (master's, plan 35c's), so both
  upgrade to R2, backed up first. Without them R2 would not reach the owner's install.

## F5, noted, not wired

The init line of lc1 lists 59 agy tools, among them `read_url_content`, `open_browser_url`, `read_browser_page` and
`browser_*`. Plan 35 §1.2 says "14 built-ins in print mode, with `search_web` but no URL reader and no browser". Plan
35 is stale on this. Nothing here wires those tools: herdr's browser on agy stays bladebro, and the gate's sources axis
was not taught `read_url_content`.

## Not done, and open

- The live effect is unmeasured: no order ran on this build.
- Whether an `asked` gate (the send-back cap spent, or a blocked delivery) should queue a review. The order's "a
  failed or asked gate" reads as "has something to learn"; `wanted_for` says so for any verdict other than a pass, but
  the gate queues a review only after a pass or a send-back (plan 33 §3.3), and this does not add one.
- A worker that asked a question mid-order and then delivered a clean report reads as a first pass: `RunState` keeps
  no record of an `asked` event per order that is not overwritten.
- The Learning square counts reviews seated and the cap's count; it shows no skip count (plan 36 never had one), and
  none was added.
