# O7, first round: Nirvana Claude against plain Claude Code on the Moto project (2026-10-18)

One run per arm on M1, M2 and M5, on alpha 0.18.0 (the o7-ab worktree's debug build and plugin). `table.md` is the
driver's rendering (`python nirvana/bench/ab.py table <out>`); the runs stayed in the worktree's `target/o7-bench/paid`.
Both arms are Opus 5.5 at medium; arm C is `claude -p` with the owner's normal settings, arm N is Nirvana Claude in an
isolated herdr server. Every run is on a fresh clone of the Moto project, never the owner's folder.

The round is one run per arm because the owner's Claude week was at 58%, pacing 2.0, when it started. The plan's three
runs per arm and the run over 5 hours wait for the weekly reset. The table says "insufficient runs" for that reason.

## Readings
- **M1, a question:** both pass 9 of 9. N 33.8 s and 351k gross tokens, C 22.1 s and 264k.
- **M2, start the server:** both pass 11 of 11. N answered in 67 s. C served the editor but its print-mode session
  ran to the 1,200 s wall, the same print-mode trait the 2026-10-12 A/B recorded.
- **M5, add `/api/health`:** both fail the same 4 of 8 rules, and for the same reason. Each arm wrote a correct route,
  tested it in-process, and said that nothing was committed. The truth check starts the server from the committed tree,
  so the route reads 404 there. The scenario expects a commit that the ask does not ask for. On this run the two arms are
  equal.
- **Seating was not exercised by M1 to M5.** Nirvana Claude answered all three itself, which R2 allows for a question,
  a server start and a 10-line edit to one file. So M9 was added (scenario file, orchestrator-shaped): M5's route plus
  `/api/clips`, tests for both and a commit, over R2's line.
- **M9, two routes, tests, a commit:** both pass 8 of 8 on the committed tree. N took 128 s and 1.50M gross, C 119 s and
  0.99M. N seated it: the `seat` skill ran `herdr seat pick` (build.bounded, sonnet medium on max, booked), the spawn
  passed the guard, a `worker-sonnet-medium` subagent built it, and Nirvana Claude reviewed the diff, fixed the clip
  file name the worker had set to the clip id, ran the tests and committed. About a third of N's gross is the Sonnet
  worker's. Both arms found the same pre-existing failure in `test_backend.py` and left it alone.
- **Two defects M9 found in the record line** (`herdr nirvana subagent-stop`): the worker's output was recorded as 98
  tokens against 2,400+ in its transcript (the first row per message id was kept, not the last), and its report was
  missed (it came in a `SubagentHandback` call, as prose). Fixed in package srf.
- **Not yet exercised live by the orchestrator:** `herdr seat run` (pane workers), agy workers, the gate, the retro.
  M9 is short enough for a subagent, which plan §7.1 allows. The next round needs an order that is long or on agy.
- **Tokens:** N's gross is 1.3 to 1.7 times C's, almost all of it cache reads. That fits its larger fixed context: the
  appended role, about 1.5k tokens, plus the start digest, about 300. Cache reads are the cheapest kind; output is level
  (1.7k to 2.5k on both).
