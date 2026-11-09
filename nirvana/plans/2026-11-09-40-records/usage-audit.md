# Plan 40 AUDIT: how Nirvana has been used, per model, over its lifetime

Owner, 2026-11-09: "validate how we have been using Nirvana over its lifetime from its usage logs with each model."

Every number here is printed by `nirvana/usage_audit.py`; this file holds what the numbers decide. The snapshot is the
run below, with `--now` as a cutoff so it can be reprinted byte for byte (two runs at a past `--now` printed identical
output, 485 lines, 15 s each):

    PYTHONIOENCODING=utf-8 python nirvana/usage_audit.py --now 1794267000          # the text report, sections A-G
    PYTHONIOENCODING=utf-8 python nirvana/usage_audit.py --now 1794267000 --json   # the same as JSON
    PYTHONIOENCODING=utf-8 python nirvana/usage_audit.py --since 2026-11-01        # any window

Drop `--now` for today's numbers. Each finding names the section of the report that prints it.

## What the sources can and cannot show

- **Cost is list price, not quota.** Every dollar is the token count times the list price (seat table, with two
  corrections: Sonnet 5.5 cache read $0.10, not $0.20; Fable 5.2 $8/$40, cache read $0.20). The owner pays a
  subscription; section A's week calibration is the only bridge from dollars to quota.
- **Claude Code transcripts start 2026-09-22** (file dates). Old Nirvana before that is in `events.sqlite` only. The
  old-Nirvana transcript folders are listed but kept out of the shares, because sqlite already counts those turns.
- **agy (Gemini, G·Claude) tokens come from the seat record and old Nirvana's sqlite.** The agy CLI logs were not read
  (9,732 files, and a token count in them could not be told from a credential without opening them).
- **Effort is known for 41% of Claude responses** (the `api-request` lines that carry it); the rest print `?`.
- **The record covers fewer orders than the journals seat.** Claude workers' transcripts hold 904 M cache-read tokens
  against the record's 630 M for the same seats (A, "check, claude workers"); all 225 Claude worker sessions the
  journals name were found. Which orders the record misses was not traced.
- **Image tokens are estimates** (w x h / 750 after 1568 px and 1.15 MP scaling, from each image's header); the
  carry-over of an image into later turns is an upper bound (a `/clear` or compaction ends it and is not counted).
- **A cache miss** is a turn after a gap of over 5 min whose cache write is at least half of a context of 20k or more.

## What this says (ranked by what is at stake)

1. **Most spend is not Nirvana.** Lifetime list price $10,060: one plain Claude Code game project is 57%
   ($5,710, of which $5,120 is Opus 5.5 subagents and Workflow agents), the harness's own builds 20% ($1,977, two
   thirds subagents), old Nirvana 11% ($1,127), and everything Nirvana-Herdr ran (workers, agy seats, orchestrator)
   under 9%. Last 14 days: $1,481, harness builds 61%, workers 27%, orchestrator 12%. Opus 5.5 is 79% of lifetime
   spend. Section A. *Cannot show:* use on claude.ai or another machine.
2. **The record calls 65 accepted orders failures.** 65 record lines (n 323) end with the journal verdict `accepted`
   (the orchestrator took the delivery with a reason): 55 are recorded `failed`, 10 `sent_back`. The record's pass
   rate for herdr 0.37-0.43 reads 0-7%; with accepted it is 42-74%. Sonnet on `ops.terminal` goes from 0 of 59 to
   32 of 59, on `build.bounded` from 9 to 25 of 30. `nirvana_lessons.rs` already counts `accepted` as a pass; the seat
   record does not. The day `use_record` turns on, the blend would punish Sonnet for orders it delivered.
   Sections B and F. *Cannot show:* whether an accept was right.
3. **The size multipliers are the priors most off.** Booked against expected use, median, week window: flash low small
   26x (n 45), flash medium small 15x (n 10), argon medium small 27x (n 5), opus small 4-5x (n 8), haiku low small 3.7x
   (n 14); large orders the other way, sonnet medium large 0.14x (n 7), flash high large 0.29x (n 7), opus high large
   0.38x (n 11). Medium is near right (sonnet medium medium 1.0x, n 72). Small orders cost 4-25x the prior, large ones a
   third of it: `size.small = 1` and `size.large = 20` against `medium = 5` are not what the orders spend.
   Section B priors. *Cannot show:* whether the orchestrator labels size consistently.
4. **The max pool's 5 h prior disagrees with its own week prior by about 6x.** For the same orders the 5 h ratio is
   6.5x the week ratio (sonnet medium medium: 6.48 against 1.01, n 72; opus high medium 7.0 against 1.09, n 9): the
   seat `use` priors put 5 h at 8x the week, the pool's `per_usd` at 51x. One of them is wrong, and the reserve rule
   reads the 5 h one. Section B priors. *Cannot show:* which, without 5 h readings paired with spend.
5. **A Max week buys about $900-$1,180 of list price on this machine.** Four weeks of `claude /usage` readings
   against every Claude Code turn between them: $1,177 and $1,158 per full week for the two whole weeks, $900 and $914
   for partial ones. The table's `per_usd` (0.00078) means $1,282. This is a lower bound on the true week (use
   elsewhere moves the reading and is not in the transcripts), and `/usage` reads in whole percents. Section A
   calibration.
6. **The orchestrator is a third of a run's list price, and an idle orchestrator re-buys its cache.** Over 100 runs,
   Nirvana Claude's own turns inside a run's window are 33% of run spend ($255 against workers' $513), median run 21%,
   19 runs over half. Its hooks log 83% of those tokens to the journal (501 M of 606 M). Six orchestrator sessions ran
   over 8 h and hold $345 of its $347 lifetime; 118 turns after an idle gap over 60 min rewrote the cache, $95 above
   what a cache read would have cost (27% of its spend). Sections C and D. *Cannot show:* what the owner was doing in
   the gaps.
7. **Cheap seats win per pass where n allows a call.** `review.adversary`: sonnet 17 of 17 at $0.21 per pass, flash 9 of
   11 at $0.50 (within noise on the rate, flash dearer per pass). `build.bounded`: sonnet 25 of 30 at $0.95, flash 14 of
   16 at $1.22, opus 9 of 10 at $3.58 (rates within noise; opus 3-4x per pass). `build.load_bearing`: flash 9 of 28 at
   $5.47 per pass with 15 send-backs, sonnet 5 of 9 at $1.44, opus 5 of 13 at $18.66: flash costs more in the end, and
   opus is not buying passes here. Section B. *Cannot show:* class difficulty differs by project; n under 30 per cell.
8. **The orchestrator reads images in its own context.** 575 images entered Nirvana Claude's sessions, 406 in its main
   chain (about 0.66 M tokens by estimate); carried through later turns that is at most 222 M cache-read tokens, up to
   $41 (12% of its spend). Workers took 1,819 images, carry at most $9. `herdr media` ran 2 times, bladebro `vision`
   2 times, all lifetime. `judge.visual` on sonnet: 6 of 6 ended with no report; on flash 1 of 1 passed. `media.video`
   on argon: 1 of 6 passed, 4 sent back. Two uploads (3.3 MB) in one project; 5 order files mention the uploads
   folder or a file in it.
   Section E. *Cannot show:* image quality or whether a smaller image would have served.
9. **A Claude worker starts at about 32k tokens; an agy worker at about 10k.** First turn, input + cache: sonnet median
   31,649 (n 147), opus 32,502 (n 63), haiku 25,651 (n 15), flash 10,399 (n 182), argon 10,043 (n 24). The order file
   itself is small (median 927 tokens, p90 3,060, n 387), and a later order of a run repeats a median 33% of an earlier
   order's lines (p90 86%, n 293). Workers then read 54% of their input at contexts over 150k (16% over 300k). The
   fixed overhead and the context growth are the levers, not the brief. Section D. *Cannot show:* which part of the 32k
   is tools, which is system.
10. **Lessons have barely been tested.** 36 lessons across the projects; offered 43 times, opened 17, a pass after
    opening 5; 77 `lesson-review` orders ran (30 in the record, flash, $0.11 per pass). Six lessons in one project were
    never offered. Fable: no order ever ran on a Fable seat (the seat is off); 1,944 Fable 5.1 responses ($427) in
    harness and game sessions, 2026-10-02 to 10-15, and 41 Fable 5.2 responses ($9.59) on 2026-11-09 in a harness
    session. `week.fable` read 0% at every journal reading through the cutoff. Sections F and G. *Cannot show:* lesson
    effect at n 5.

Also printed and not ranked: 32 `asked` events (an order waiting on the owner) over 18 of 100 runs (max 9 in one run); 14 runs without a receipt;
57 orders seated as children of a passed order (a next step and a redo look the same in the journal); old Nirvana's
gate: PASS 33, SEND_BACK 27, WITHHELD 5.

## The table's priors that the record contradicts

From section B priors (booked use / expected use, week window unless named). A prior is listed when its median ratio is
outside 0.5-2.0 with n of 5 or more.

| seat | class/size | prior | measured (median ratio) | n |
|---|---|---|---|---|
| flash | low, small | `use` low x `size.small` | 26x | 45 |
| flash | low, medium | `use` low x `size.medium` | 8.9x | 12 |
| flash | medium, small | `use` medium x `size.small` | 15x | 10 |
| flash | medium, medium | `use` medium x `size.medium` | 2.7x | 17 |
| flash | high, small | `use` high x `size.small` | 2.1x | 12 |
| flash | high, large | `use` high x `size.large` | 0.29x | 7 |
| argon | medium, small | `use` medium x `size.small` | 27x | 5 |
| haiku | low, small | `use` low x `size.small` | 3.7x | 14 |
| opus | medium, medium | `use` medium x `size.medium` | 2.1x | 9 |
| opus | high, large | `use` high x `size.large` | 0.38x | 11 |
| sonnet | medium, large | `use` medium x `size.large` | 0.14x | 7 |
| sonnet | medium, medium, 5 h | `use` 5 h against the max pool's 5 h `per_usd` | 6.5x | 72 |
| sonnet | medium, small, 5 h | the same | 9.4x | 41 |
| opus | high, large, 5 h | the same | 2.4x | 11 |
| max pool | week | `per_usd` 0.00078 ($1,282 per week) | $1,158-$1,177 per full week (a lower bound) | 2 weeks |
| sonnet seat | price | cache read $0.20 | list $0.10 | - |
| fable seat | model | `claude-fable-5-1` at $10/$50 | Fable 5.2 is $8/$40 | - |

The record's `use` is the order's tokens priced by the table and turned into a share by the pool's `per_usd`, so these
ratios test the token priors and the size multipliers, not a quota reading.
