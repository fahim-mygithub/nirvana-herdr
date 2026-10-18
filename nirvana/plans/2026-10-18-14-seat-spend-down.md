# Feature 14: spend-down seating, so quota that would expire unused is the cheapest seat

Status: **APPROVED 2026-10-18, building on agy (Gemini 4 Argon high).** Amends plan 12 §5.3's pressure step and two
numbers in `seats.toml`. The owner approved the amendment, so CLAUDE.md rule 14 is met.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "my 7 d is being used up and I feel like we have not used gemini considering at best we can just use the Gemini resource pool for claude we should be using it more" | the seat rule's cost step: a pool window's projected spare quota at its reset makes an order that fits in it near free; a window on pace to run out costs more | orchestrator | unit tests on recorded readings; `herdr seat pick` on today's readings prints the picks in §3 |
| L2 | "Should the seat rule count quota that will expire unused as free, so Gemini, Argon and G·Claude take most work while Max is running hot?" answered "Yes, build it next (Recommended)", whose text was: "Projected spare quota at reset is near free; a pool on pace to run out costs more. Fix the Max week prior. One live agy order proves the pane path. Built on agy seats." | §2's cost; §2.3's Max prior; §4's live order | orchestrator | §5 |

## 1. What happened (measured 2026-10-18, transcripts in `~/.claude/projects`, list prices from `seats.toml`)

This week's Claude Max use, from the week's start (about 2026-10-16 05:00 UTC) to 62% used:

| Share | List price | What |
|---|---|---|
| 49.6% | $395 | 3D-Printer-Flow on 10-16, plain Claude Code: 113 subagents, every one `model: inherit`, so all Opus |
| 47.7% | $380 | the orchestrator session building plans 12 and 13 |
| 1.4% | $11 | 3D-Printer-Flow-xtool on 10-18, Nirvana Claude: Opus orchestrating, Sonnet subagents |
| 1.3% | $11 | the O7 A/B runs and the rest |

The total was $796 at list price for 62%, so a Max week is about **$1,280** at list price. `seats.toml` assumes $200
(`per_usd.week = 0.005`).

`herdr seat pick` on that day's readings showed what was wrong:
- **Max is picked for nearly everything.** Claude week 62% used at pace 1.67; G·Claude week 4% used, resetting in 11 h;
  Gemini 1%; Argon 7%.
- **plan:** opus. "g-opus scores the same", but its pressure was 0.85 against opus's 0.024.
- **build.bounded:** sonnet. g-sonnet "scores 77.0, under the bar 85".
- **fix.debug, review.adversary:** opus.
- **Efficient mode** moved only build.bounded and research.read, to Flash.

**Why.** Pressure is the order's expected share of the window's headroom (`WindowState::pressure`). One order is a
sliver of Max and several percent of G·Claude, so Max wins whatever the pace. The only credit for quota about to be
lost is `reset_factor`, which starts 60 minutes before a reset.

## 2. The change

### 2.1 Projected spare

For each read window:
- **elapsed** = the share of the window gone. The window's length is 5 h or 7 d, and `resets_at` gives where it is.
- **projected** = `used / max(elapsed, ease)`. This is where the window lands at its reset at the pace so far. It uses
  the same `pace_ease` the foot's pace uses, so a window just begun does not read as racing.
- **spare** = `max(0, 1 - projected - booked)`. This is quota that will expire unused at this pace.

### 2.2 The cost that replaces pressure

An order's expected share `e` on a window splits in two:
- **The part that fits in the spare is near free.** It is weighted by `spare_weight` (default 0.05). It is not 0, so
  among free seats the smaller order still wins.
- **The rest is priced as today:** `(e - spare)+ / headroom × pace × reset_factor`.

A seat's cost is its worst window, as pressure is today. The bar and the tiers stay as they are. A window too small for
the order is still tier 2: spending spare quota never risks a mid-order rate limit.

The reason line names it: "g-opus high: fits G·Claude's spare (week 96% projected unused, resets in 11h)".

### 2.3 Two numbers in `src/nirvana_seats.toml`

- **Max `per_usd.week` becomes 0.00078** ($1,280 a week), with basis "derived: week of 2026-10-16, $796 at list price
  for 62%". The 5 h value stays a prior, with a comment that it was not measured.
- **build.bounded's g-sonnet score becomes 88**, sonnet's. It is the same model; the 77 was an "overflow seat" ruling made
  when pressure could not tell spare quota from scarce quota. Labelled a hypothesis, for the record (decision 0029) to
  correct.

### 2.4 Not changed

- **The modes and their bars.**
- **The reserve on Max's 5 h window**, so the orchestrator keeps its seat.
- **Jev's shadow role**, which keeps writing its pick beside the rule's.

## 3. Expected picks, written down before the build

Run on the 2026-10-18 readings, recorded into a fixture:
- **plan, fix.debug, review.adversary:** g-opus. The order fits G·Claude's spare and Max is on pace past 100%.
- **build.bounded:** g-sonnet.
- **build.load_bearing, at size large:** opus. g-opus's 5 h window cannot hold a large Opus order (128%), so it is
  tier 2.
- **read.huge, sweep.mechanical:** flash, as today.
- **write.docs:** argon.
- **Max with spare** (a fixture where Max's week is at pace 0.5): the old picks come back.

## 4. One live agy order (plan 12's owed live check)

One `chore.script` or `sweep.mechanical` order runs through `herdr seat run` to a pane worker on flash low. It uses an
isolated herdr server, a scratch git repo and a scratch `NIRVANA_HERDR_HOME`, then goes through `herdr gate run` to a
verdict.
- **Paste:** the journal lines, the gate's verdict and the record line.
- **Expected:** the worker commits, the gate passes or sends back with a reason, and the record line names the seat
  `flash`.

## 5. Verification

- **Unit tests** for each of §2.1 and §2.2: a fresh window, a window near its reset, a racing window, a booked lease,
  and the tier-2 guard.
- **§3's picks** from the fixture, as a test.
- **The existing seat tests pass,** except those whose expected pick this change moves. Each one moved is listed with
  its old and new pick.
- **Every assertion gets a control** in `nirvana/controls.d/p14.py`.
