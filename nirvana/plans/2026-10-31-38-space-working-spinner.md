# Feature 38: a space shows a soft animated sparkle while anything under it is working

Status: **PLANNED (2026-10-31)**. One package, P38, on master 9304a821 (alpha 0.46.0), in parallel with plans 35 and 36.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "See the chekc and circle boes in the spaces. I want to use that space as a soft icon to show whats going on. Mainly if there are agents underneath that project that are still working and are not actively idle have an animated icon represent that similar to teh claude code working icon  but lets choose our own or htis app" | the marker before each space name in the sidebar's spaces list | P38 | §3 tests; §4 live check |
| L2 | "Nirvana sparkle" (answer, 2026-10-31, to: which animated icon a space shows while an agent under it is working; preview frames `✧ ✦ ✶ ✷ ✸ ✷ ✶ ✦`, about 4 frames a second) | the frames | P38 | §3 tests |

## 1. What was measured

The owner's screenshot (2026-10-31, alpha 0.46.0): 3D-Printer Flow shows `✓` while its worker `quad-pegs…` has run
56 s, and MotoProject shows `✓` while its Flash worker `reel-m…` has run 7m54s. The space marker is one status per
space, and herdr's `status_priority` (`src/client/shell.rs`) ranks Done (3) above Working (2), so a finished Nirvana
Claude hides a working worker. Seated workers and their panes may also not count toward the space's status; P38
checks which agents feed it.

## 2. What it does

1. **Working means:** any agent under the space is Working or Blocked: its Nirvana Claude, plain Claude, Codex, agy
   or local panes, and the seated workers and their panes that the sidebar lists under the space. Done, Idle and
   Unknown are not working. Blocked keeps its own sign if herdr has one that matters more (say which you chose).
2. **The marker:** while working, the slot shows the sparkle frames `✧ ✦ ✶ ✷ ✸ ✷ ✶ ✦` in order, about 4 frames a
   second (a constant, with the frames, beside the other sidebar constants), in the space's existing working colour.
   When nothing is working it shows what it shows today (`✓`, `○`, ...). Every space animates in the same phase from
   one clock, so the list does not shimmer.
3. **Cost:** the animation repaints only the marker cells and only while at least one visible space is working; with
   nothing working there is no extra repaint at all. No new reads per frame: working-ness comes from the snapshot the
   sidebar already holds.
4. **Width:** every frame is one terminal cell in the fonts herdr ships with (JetBrains Mono, Windows Terminal);
   measure that each glyph's width is 1 with the same width function the sidebar uses, and fall back to `◐` if a
   glyph is wide.
5. **Off switch:** the existing `ui.status_indicators` setting keeps meaning; add no new setting unless one is needed
   to turn animation off, and say why.

## 3. P38: the package

Owns `src/client/shell/nirvana_spaces.rs`, `src/nirvana_spaces.rs`, the space-marker drawing in
`src/client/shell/sidebar.rs` and `src/client/shell/agent_sidebar.rs` where the marker is drawn, a tick for the frame
clock, `nirvana/controls.d/p38_spinner.py`, and the record `nirvana/plans/2026-10-31-38-records/p38.md`. herdr's
shared `status_icon` and `status_priority` in `src/client/shell.rs` stay as they are for herdr's own views; the
sparkle is a Nirvana layer over the spaces list.

Plan 36 (in parallel) owns `src/client/mod.rs`, `src/client/shell/nirvana_mods.rs`, `composition.rs`, `state.rs`,
`mouse.rs`, `surface_patch.rs`; plan 35 owns `src/nirvana_gate.rs`, `src/nirvana_worker.rs`, `src/nirvana_gui_tools.rs`,
`src/nirvana_profiles.rs`, `src/nirvana_agy_tools.rs`. If the frame tick needs a line in one of those, put it in the
report as an exact hunk.

Tests, each with a sabotage control: a space with a Done Nirvana Claude and a Working worker animates; a space with
only Done and Idle agents shows today's marker; the frames cycle in order on the clock; two spaces share a phase; no
repaint is scheduled when nothing is working; every frame is one cell wide.

## 4. Live check for the owner

With a worker running under a space whose Nirvana Claude is done, the space's marker sparkles; when the worker ends
it settles to `✓`.
