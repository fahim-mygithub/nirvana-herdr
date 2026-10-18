# Plan 12 O7: Nirvana Claude against plain Claude Code (generated)

- out: `C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o7\target\o7-bench\paid`
- scenario file sha256: `c58d60d603f5f5ff26138a6d705d0872bcda010b6e8682a13620e1bfa9ffca55`
- both arms: claude-opus-5-5 at medium; gross tokens = input + cache write + cache read (output excluded)
- Claude 5h and week: whole-percent readings of the whole account before and after the run, so another session's use in the span is inside them; the token columns are the meter

## M1 (question)

| run | arm | ended | wall s | truth | failed rules | orchestrator gross | workers gross | person | Claude 5h | Claude week | seats | incidents |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| M1/C-1 | C | finished | 22.07 | PASS (9 pass, 0 fail) | - | 263,852 | 0 | n.a. | +0.00 pts | +0.00 pts | claude-opus-5-5 x10 messages | - |
| M1/N-1 | N | answered | 33.792 | PASS (9 pass, 0 fail) | - | 350,986 | 0 | 0 | +1.00 pts | +0.00 pts | orchestrator only | - |

Tokens by kind (fresh input, cache write, cache read, output), orchestrator and workers apart:

- M1/C-1: orchestrator in 8 / cache write 51,837 / cache read 212,007 / out 2,059; workers in 0 / cache write 0 / cache read 0 / out 0
- M1/N-1: orchestrator in 8 / cache write 55,797 / cache read 295,181 / out 1,717; workers in 0 / cache write 0 / cache read 0 / out 0

- Nirvana Claude (N): runs 1, median wall 33.792 s, median pass fraction 1, median gross 350,986
- plain Claude Code (C): runs 1, median wall 22.07 s, median pass fraction 1, median gross 263,852
- verdict: insufficient runs (n<3); N 1, C 1

## M2 (ops)

| run | arm | ended | wall s | truth | failed rules | orchestrator gross | workers gross | person | Claude 5h | Claude week | seats | incidents |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| M2/C-1 | C | wall | 1,201 | PASS (11 pass, 0 fail) | - | 479,346 | 0 | n.a. | +5.00 pts | +1.00 pts | claude-opus-5-5 x11 messages | wall |
| M2/N-1 | N | answered | 67.222 | PASS (11 pass, 0 fail) | - | 637,826 | 0 | 0 | +2.00 pts | +0.00 pts | orchestrator only | - |

Tokens by kind (fresh input, cache write, cache read, output), orchestrator and workers apart:

- M2/C-1: orchestrator in 14 / cache write 56,001 / cache read 423,331 / out 1,999; workers in 0 / cache write 0 / cache read 0 / out 0
- M2/N-1: orchestrator in 14 / cache write 61,445 / cache read 576,367 / out 2,128; workers in 0 / cache write 0 / cache read 0 / out 0

- Nirvana Claude (N): runs 1, median wall 67.222 s, median pass fraction 1, median gross 637,826
- plain Claude Code (C): runs 1, median wall 1,201 s, median pass fraction 1, median gross 479,346
- verdict: insufficient runs (n<3); N 1, C 1

## M5 (small)

| run | arm | ended | wall s | truth | failed rules | orchestrator gross | workers gross | person | Claude 5h | Claude week | seats | incidents |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| M5/C-1 | C | finished | 35.605 | FAIL (4 pass, 4 fail) | committed_on_base, health_is_200_json, health_names_the_loaded_manifest, health_counts_the_hike_clips | 492,717 | 0 | n.a. | +0.00 pts | +0.00 pts | claude-opus-5-5 x14 messages | - |
| M5/N-1 | N | answered | 49.023 | FAIL (4 pass, 4 fail) | committed_on_base, health_is_200_json, health_names_the_loaded_manifest, health_counts_the_hike_clips | 822,180 | 0 | 0 | +1.00 pts | +0.00 pts | orchestrator only | - |

Tokens by kind (fresh input, cache write, cache read, output), orchestrator and workers apart:

- M5/C-1: orchestrator in 14 / cache write 58,163 / cache read 434,540 / out 2,650; workers in 0 / cache write 0 / cache read 0 / out 0
- M5/N-1: orchestrator in 18 / cache write 62,196 / cache read 759,966 / out 2,472; workers in 0 / cache write 0 / cache read 0 / out 0

- Nirvana Claude (N): runs 1, median wall 49.023 s, median pass fraction 0.5, median gross 822,180
- plain Claude Code (C): runs 1, median wall 35.605 s, median pass fraction 0.5, median gross 492,717
- verdict: insufficient runs (n<3); N 1, C 1

## M9 (medium)

| run | arm | ended | wall s | truth | failed rules | orchestrator gross | workers gross | person | Claude 5h | Claude week | seats | incidents |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| M9/C-1 | C | finished | 118.556 | PASS (8 pass, 0 fail) | - | 994,214 | 0 | n.a. | +3.00 pts | +0.00 pts | claude-opus-5-5 x30 messages | - |
| M9/N-1 | N | answered | 127.771 | PASS (8 pass, 0 fail) | - | 1,162,860 | 336,012 | 0 | +1.00 pts | +0.00 pts | subagent claude-sonnet-5-5 x8 messages | - |

Tokens by kind (fresh input, cache write, cache read, output), orchestrator and workers apart:

- M9/C-1: orchestrator in 24 / cache write 80,478 / cache read 913,712 / out 9,370; workers in 0 / cache write 0 / cache read 0 / out 0
- M9/N-1: orchestrator in 26 / cache write 71,551 / cache read 1,091,283 / out 5,446; workers in 16 / cache write 48,871 / cache read 287,125 / out 3,541

- Nirvana Claude (N): runs 1, median wall 127.771 s, median pass fraction 1, median gross 1,498,872
- plain Claude Code (C): runs 1, median wall 118.556 s, median pass fraction 1, median gross 994,214
- truth parity: N 1, C 1: at parity or better
- usage ratio: N gross / C gross = 1.51 (target 3, ceiling 4): within target


