# Feature 37: every delivery is gated, invented sources are caught, and agy briefs say how to read

Status: **BUILT (2026-11-01, alpha 0.50.0)**. Was PLANNED (2026-10-31). Started after plan 35's P35a and P35b merge (it edits their files). The evidence is
the MotoProject analysis of 2026-10-31, `nirvana/plans/2026-10-31-35-records/moto-gemini.md`, and plan 35's records.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "CHeck the recent motoproject runs gemini failed pretty badly and I think we need to improve that failure point. Doe sthe changed we made currently address this or is ther emore work" | all | builders | §4 live checks |
| L2 | "From that report we may need the Nirvana seat to have a stronger prompting strategies when using agy" | §1.4 | P37 | §3 tests |
| L3 | "Yes, after the fixes (Recommended)" (answer, 2026-10-31, to: once the source and owned-file fixes land, should herdr gate every delivered order itself) | §1.1 | P37 | §3 tests |
| L4 | "Yes, note on failure (Recommended)" (answer, 2026-10-31, to: should the gate check that cited YouTube videos exist through oEmbed; a 404 fails, an unreachable check is a note) | §1.2 | P37 | §3 tests |
| L5 | "Step up one seat (Recommended)" (answer, 2026-10-31, to: which seat a redo gets after a send-back for invented sources) | §1.3 | P37 | §3 tests |

## 1. What it changes

1. **herdr gates every delivered order itself** (L3), with the same call `seat exec` makes for review and teaching
   orders today, then wakes Nirvana Claude with the verdict. It applies only once P35a's sources rule and owned axis
   are merged: on a tree without them it is refused at build time. Nirvana Claude may still regate by hand. Measured
   cause: 1 of 26 deliveries gated in 8 days in MotoProject, so a fabrication went unchecked and no lesson review ran.
2. **Existence check for cited YouTube ids** (L4): the gate asks YouTube's oEmbed for each cited video id; 404 fails
   the sources axis naming the id; a timeout, a network error or any other status adds a note and never fails. A
   short per-call timeout and a cap on calls per gate, constants beside the others. Reddit is not checked (it rate
   limits). Measured: 6 of 6 invented ids 404, 48 of 48 real ids 200.
3. **A send-back for invented sources steps up one seat** (L5): when the sources axis fails with any cited id that
   the existence check found missing, the redo is seated on the next stronger seat of the same class in the seat
   table, and its brief says nothing from the first try is trusted. Other send-backs keep today's re-pick.
4. **The agy brief strategy** (L2), in `nirvana/claude/ROLE.md` and `nirvana/claude/skills/seat/SKILL.md`: for an agy
   research or media order, Nirvana Claude writes the source method (yt_dlp for video metadata and transcripts,
   Reddit `.rss`, curl per page, bladebro when the order needs a browser), "never cite a URL you did not open in this
   session", what done means and how to check it, and never promises a connector agy does not have. Each rule names
   the MotoProject failure it answers. The plan 35 P35b order line stays as herdr's floor.
   The rules and the checklist are the analysis's §3, `nirvana/plans/2026-10-31-35-records/moto-gemini-briefs.md`
   (rules 1-9, the seat-skill checklist; rule 4 is dropped, since P35a fixes the sources rule).
5. **A `web-researcher` profile template** (the analysis's rule 10): herdr ships its body (rules 1-3 and 7) as a
   template that `herdr profile new <name> --from web-researcher` writes into a project's `.nirvana/profiles/`, and
   ROLE.md tells Nirvana Claude to seat agy research orders with it when the project has one. Its lessons then
   build up through plan 33's reviews instead of being retyped.

## 2. Package P37

Owns `src/nirvana_gate.rs` (items 2, 3), the delivery path in `src/nirvana_worker.rs` (item 1), the re-pick in
`src/nirvana_seat.rs` (item 3), `nirvana/claude/ROLE.md` and `nirvana/claude/skills/seat/SKILL.md` (item 4), the profile template and its
`profile new --from` path in `src/nirvana_profiles.rs` (item 5),
`nirvana/controls.d/p37_delivery.py`, and the record `nirvana/plans/2026-10-31-37-records/p37.md`. Built on master after
P35a and P35b merge; if P35c is still building, its one `SKILL.md` line is joined at merge.

## 3. Tests

Each with a sabotage control: an ordinary delivery is gated without an orchestrator turn and the wake carries the
verdict; the existence check fails a 404 id, passes a 200, and only notes a timeout (a stand-in HTTP server, never
the network, in tests); the step-up picks the next seat and its brief says nothing is trusted; an ordinary send-back
keeps its seat; the ROLE.md and seat skill rules are present and each names its failure.

## 4. Live checks for the owner

1. Seat an agy research order in MotoProject: it is gated on delivery, and its YouTube ids are checked.
2. Replay the invented `reel-motion-gemini` stream through `herdr gate` on a scratch copy: it fails sources with the
   404 ids named, and its redo goes to the next seat.
