# O0 probes for plan 12 (2026-10-17)

The owner: "Looks good", to running probes P1 to P7. Each quota probe ran alone, one after another
(`run_probes.py`, `p2b.py`, kept in the session's scratch folder with every stream). agy updated itself from 1.4.0
to 1.4.1 during the runs. P5, P6 and P7 have their own files here.

## P3: launch flags

| Check | Result |
|---|---|
| agy `--model claude-sonnet-5-5 --effort low` | ran, `SUCCESS`. Old Nirvana's "`--effort` kills the worker" no longer holds on 1.4.x |
| agy `--model gemini-3.8-flash --effort low` (base name) | ran, `SUCCESS` |
| agy `--json-schema` | `SUCCESS`; the response is the object, plus agy's own `toolAction` and `toolSummary` keys |
| claude `-p --json-schema` | `structured_output` = `{"ok": true, "word": "ok"}` |

## P1: the pools

- **agy now reports an Argon bucket**: `gemini-argon-wtu-weekly`, weekly only, appeared between two reads during the
  P4 runs. Argon spends it, not the Gemini group: a small build order took 1.01% of it at low and at medium, a video
  read 0.30%. There is no Argon 5 h window.
- **Fable** (from P6's endpoint read): Fable spends the all-models week, with an allowance of 50% of it. A tiny Fable
  turn did not move `/usage`'s whole percents.
- **The groups refill continuously**: `remaining_fraction` rose between reads with nothing running, by up to 1.9% of a
  5 h window. A delta is only a rough price of one order; plan 12 prices orders from their tokens (Argon's F8).

## P4: one small order on seven seats

The order: `slugify` per five rules, in `slug.py`, plus the worker's own tests. Graded by a hidden 10-test pytest,
which is deterministic and free, so Jev was not needed as the grader.

| Seat | Hidden tests | Time | Out tokens (thinking) | Cache read | Pool spent |
|---|---|---|---|---|---|
| flash low | 10/10 | 28 s | 1,476 (0) | 36,542 | below the refill noise |
| flash medium | 10/10 | 86 s | 12,883 (8,742) | 406,294 | refill noise |
| flash high | 10/10 | 180 s | 24,298 (19,251) | 489,231 | gemini 5 h 0.68% |
| argon low | 10/10 | 44 s | 1,180 (0) | 55,053 | argon week 1.01% |
| argon medium | 10/10 | 53 s | 1,894 (502) | 73,051 | argon week 1.01% |
| g-sonnet medium | 10/10 | 16 s | 1,287 (0) | 45,863 | **google3p 5 h 3.31%, week 1.66%** |
| sonnet medium (Max) | 10/10 | 11 s | 1,286 (105) | 43,807 | not separable: other agents ran on Max |

Readings: every seat can do a small bounded order. Flash at medium and high spent 9x to 16x flash low's output for
the same result, the M7 pattern. Google's Claude pool is small: one small order is 3.3% of its 5 h window, about 30
orders, so `g-opus` and `g-sonnet` are overflow seats, not a second Max.

## P2: video

- **Argon low** read the MP4 exactly: red, green, blue thirds with 1, 2, 3, and the two tones at 5 s and 25 s.
- **Flash low and medium** read the MP4 and the WebM as well, about 35k tokens each, **once the prompt named the
  file's full path and the `view_file` tool**. With a bare file name, flash ran `dir`, plan mode refused it, and it
  ended with an **empty answer and `status: SUCCESS`**. The gate must treat an empty report as a failure.
- **Over 100 MB** (a 186 MB file): `view_file` refused it, as Argon's review said, and Argon offered to downscale it
  with ffmpeg. The media skill shrinks recordings first.
