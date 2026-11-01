**What failed, in short.** One Gemini order made up its sources. The other agy research work in MotoProject was real, but herdr's gate can't recognise how agy reads pages, so real work gets sent back.

- `reel-motion-gemini` (flash low, 2026-10-23) invented all 12 citations. The 6 YouTube ids return 404 from YouTube's oEmbed; the 6 Reddit ids could not be checked because Reddit blocked my requests (403 and 429). The run made 13 `search_web` calls and opened no page. The order had no "open every URL" line.
- It went unnoticed for 8 days because the orchestrator never ran the gate on it. In run-20261023, 1 of 26 deliveries was gated between Oct 23 03:53Z and Oct 31 17:33Z.
- When it was finally gated today, herdr sent it back to the same flash-low seat, with a brief that said "do not start over". The orchestrator stopped that redo.
- The owner's retry, `reel-motion-gemini-r2` (argon medium), was real and good. All 22 sources appear in its stream, and its 10 YouTube ids return 200. The gate still sent it back on two axes, both wrong:
  - **sources:** "22 source(s) cited that were not fetched, read or browsed". It read through `yt_dlp`, Python URL lists and Reddit `.rss`, and the gate counts none of that.
  - **owned:** "3958 file(s) changed that the order does not own". The order ran in the project folder, so the gate counted the whole uncommitted tree as its work.
- The gate then started `r2-s1` on argon. The orchestrator stopped it at 18:15Z after checking the sources itself; my interim message to you crossed with that.

**Do our changes fix it?** Mostly not.
- Plans 33, 34a and 36 don't touch any of the root causes.
- P35a's new sources rule counts **0 more** of MotoProject's real reads than master does. My Python copy of each rule: r2 0/22, reveal-gemini 0/18, v13-drone-research 0/12, imp-gemini-aspect 1/17. Its own plan says it leaves URL-list scripts uncounted.
- P35b's agy brief line ("Read every URL you cite in a command of its own…") is the only planned change that would have helped `reel-motion-gemini`, and it only helps if the worker obeys it.
- P35c (bladebro) helps only orders seated with `--needs browser`; r2 was seated with `--needs video`.

## 1. What happened, order by order (run-20261023-024041, times UTC)

| Order | Seat | Stream | Sources: cited / in its own stream / YouTube ids that exist | Gate | Next |
|---|---|---|---|---|---|
| reel-motion-gemini (10-23 02:40) | agy flash-3.8 low, research.web | 13 search_web, 3 run_command, 0 reads; 131k in / 3.3k out; status SUCCESS, finished_reason null | 12 / **0** / **0 of 6** | not gated until 10-31 17:33, then sent_back: sources 12/12 | orchestrator read the file at 02:45 and said at 02:46 "some look invented"; never gated it |
| reveal-gemini (02:46) | argon low (override) | 13 search, 10 cmd | 18 / 18 / 9 of 9 | never gated | used in the synthesis |
| imp-gemini-video, -aspect, -transition (15:31) | argon medium / argon low / flash low | aspect 35 search, 15 cmd; transition 89 cmd | aspect 17/17, 10 of 10; transition 12/12, 7 of 7 | never gated | appended to plan.md |
| v9/v10/v12-scout, v10-review, v12-motion-verify, v12-smudge-map (media.video) | flash medium / argon high | local frame work, no web | `file:///` sources or none | never gated | fed the v9–v13 builds |
| v12-smudge-fix (10-30) | argon high, build.load_bearing | stalled 600 s waiting on its own background render (`task-309`), resumed, delivered | n/a | never gated | Claude's review said "ship with changes"; the owner: "the sharpening kinda made some of these shots worse" |
| v13-drone-research (10-31 04:27) | argon high | yt_dlp search plus watching frames | 12 / 12 / 12 of 12 | never gated | orchestrator: most of the 8 candidates "look like ordinary eye-level views"; kept 3 |
| reel-motion-gemini-s1 (17:33) | flash low again (herdr's send-back re-pick) | 1 search, then stopped | — | — | orchestrator stopped it as stale |
| reel-motion-gemini-lr (17:33) | flash low, lesson-review | 6 cmd | — | pass, `lesson_ops: []` | — |
| reel-motion-gemini-r2 (17:51) | argon medium (override) | 56 cmd (16 name yt_dlp), 30 read_file, 15 search; 582k in, 8.6M cache read; ~26 min | 22 / 22 / 10 of 10 | sent_back: sources 22/22 and owned 3958 | r2-s1 seated on argon; orchestrator stopped it at 18:15 |

**Comparison with Claude.** No Claude research.web order exists in this project's runs. The nearest is `v12-motion-research` (claude opus, research.read). It cited 14 `file:///` sources, and would fail the same sources axis, which doesn't count local reads on either CLI.

## 2. The failure points

- **F1. The brief, not the model.** Four Gemini research orders carried "REAL URLs ONLY: open every page and mark each item VERIFIED". All four were real: 47 cited, 26 of 26 YouTube ids exist. That includes imp-gemini-transition, which ran on the same flash-low seat that fabricated. The one order without that line fabricated (`orders/reel-motion-gemini.md:4-8`). The weak model makes it easier to happen, but four orders against one is too few to say how much.
- **F2. The gate is advisory, and the orchestrator skipped it.** Journal seq 18 (delivered, 10-23 02:43), then seq 2097 (gated, 10-31 17:33). The orchestrator wrote "Its twelve YouTube and Reddit URLs were not opened… some look invented, so I'm treating them as unverified" (transcript 7f6c57a6, 10-23 02:46), and still didn't gate. herdr auto-gates only teach and review orders (`nirvana_worker.rs:1662-1671`); otherwise only `run end` refuses an ungated delivery, and this run has been open 8 days.
- **F3. The send-back loop: late, same seat, wrong brief.**
  - It re-picks by class, and research.web picks flash low again (seq 2098).
  - The brief says "fix what the gate found, do not start over" (`nirvana_gate.rs:1593`), which is wrong when the content was invented.
  - It has no check for staleness: the order asked about the 10-slide stills post, which the reel has since replaced with all-360 video.
- **F4. The sources axis can't see agy's reads.** Each command is split into segments at newlines, `;`, `|` and `&&`. A URL counts only if its own segment contains a fetch word (`nirvana_gate.rs:574-603`; P35a adds Python and PowerShell words). Three things break it:
  - agy puts URLs in lists and variables inside scripts;
  - `yt_dlp` is not a fetch word;
  - the cited canonical Reddit URL is not the fetched `.rss` URL, and the gate compares exact normalised URLs.
- **F5. The owned axis is wrong for orders that run in the project folder.** It uses `uncommitted(cwd)` (`nirvana_gate.rs:1371-1374`), the whole dirty tree. Evidence: reveal-build "1515 files" (10-23) and r2 "3958" (10-31). The original order dodged it only because its line was capitalised `Owns:` and the parser reads lowercase `owns:` (`:188`).
- **F6. Wasted quota.** False send-backs auto-seat a paid redo: r2-s1 on argon (seq 2195).
- **F7. Learning never happened.** Lesson reviews run only after a gate (`nirvana_review.rs:1`). The only gated research order was the fabrication, and rule 6 rightly keeps nothing from it. The orders that worked out the method were never gated, so never reviewed. reveal-gemini's own gotcha was "Google Search grounding returns opaque vertexaisearch redirect URLs that must be resolved". MotoProject has no `.nirvana/lessons/`, and "The live lessons of research.web: None yet".
- **Other causes, ruled out or minor:**
  - **"YouTube connector":** agy has none. r2's init lists 60 tools, has no MCP servers, and it made no `call_mcp_tool` call. yt_dlp does that job and worked.
  - **MotoProject's project setup:** its ~3958 uncommitted files drive F5.
  - **Missing moto-reel MCP server, reel-editor profile, the :8770 editor:** none of the failures trace to these. These orders were research and scouting, done through scripts.
  - **exebox (agy's isolated desktop):** no failure traced to it.
  - **E: rule:** r2 downloaded 6 YouTube mp4s into agy's own scratch on C:. The E: rule lives only in the reel-editor profile, which research orders don't get.
  - **Model and footage quality:** the smudge sharpening and the eye-level drone picks are judgment and footage limits (a 1.1 m hand-held stick), not herdr faults.

## 3. Coverage

| Cause | Plans 33 and 34a (merged 9304a821) | P35a | P35b | P35c | Plan 36 | reel-editor profile / moto-reel |
|---|---|---|---|---|---|---|
| F1 fabrication | no | no | **partly**: the agy research line asks for a read per URL | partly (only with `--needs browser`) | no | no |
| F2 ungated | no | no | no | no | no | no |
| F3 send-back loop | no | no | no | no | no | no |
| F4 sources blind | no | **no effect on MotoProject** (twin 0/22, 0/18, 0/12, 1/17) | partly, if workers curl each cited URL in its own command | partly (bladebro calls are counted) | no | no |
| F5 owned | no | no | no | no | no | no |
| F7 learning | the machinery exists, but it starves without F2 and F4 | — | — | — | shows the loop only | no |

## 4. More work, ranked by how many of the failures each would have prevented

1. **A sources rule keyed on stable ids** (YouTube video id, Reddit post id, local paths) that counts any URL anywhere in a command naming a fetch word (yt_dlp included), plus that command's output, and `file:///` sources against `read_file`.
   - Measured with `proposed_rule_strict.py`: reel-motion-gemini stays 0/12. Real orders: r2 22/22, reveal 18/18, aspect 17/17, transition 12/12, v12-smudge-map 12/12, v9-scout 5/5, v12-scout 4/4. Still failing: v13-drone 0/12 and v10-scout 0/3.
   - Sabotage control: a fake step that prints the notes file raised the loose variant to 2/12; the strict variant stayed 0.
   - Prevents F4 and F6, and would stop about 11 of 13 MotoProject orders being sent back wrongly. Cost: small, `nirvana_gate.rs` only, so it belongs in P35a now.
   - Risk: the strict variant still misses the drone run's watch-the-frames path, so pair it with item 2.
2. **An existence check for cited YouTube ids** (oEmbed), passing a source either rule accepts.
   - Measured: 6 of 6 invented ids return 404; 48 of 48 real ids return 200, a 200 control included.
   - Prevents F1 slipping through even when nobody reads the notes. Cost: small.
   - Risks: the gate goes to the network; Reddit rate-limits fast (I saw 429s), so an unreachable check should be a note, not a fail.
3. **Owned axis for project-folder orders:** take the dirty-file set at seat start and diff it at delivery. Prevents F5 (2 of 2 such orders failed it falsely). Cost: medium (seat start plus the gate). Low risk.
4. **Gate on delivery automatically, only after items 1 and 3.** Gating today would auto-send-back most good agy orders and spend argon on redos. Prevents F2, the stale side of F3, and F7's starvation. Cost: small, the same call `seat exec` already makes for review orders.
5. **herdr writes the "REAL URLs ONLY / VERIFIED" clause into every agy research.* order.** Widen P35b's line to "never cite a URL you did not open in this session". Prevents F1. Cost: trivial. It's an instruction, not enforcement.
6. **Send-back on a sources fail: "discard, none of it is trusted", plus step up a seat when the cited ids don't exist.** Prevents F3. Cost: small.
7. **The orchestrator's brief:** stop promising a YouTube connector, and name yt_dlp metadata/transcript and Reddit `.rss` as the methods. Cost: trivial, a skill or orchestrator rules line.

## Checked claims, and corrections to the brief

- "MotoProject Gemini failed pretty badly": true for one order's content (`reel-motion-gemini`). Since then the gate, not Gemini, has caused the failures: r2's work is real.
- "Agy should have youtube connector access" (the owner, quoted in the r2 order): **wrong.** r2's init has no MCP servers and its stream has no `call_mcp_tool`; it used yt_dlp.
- `reel-motion-gemini-lr.md` was listed among the "orders" to study; it is a lesson-review brief written by herdr, not a work order.
- P35a's own expectation ("at least 70 of 81 count") was measured on groove-coach. It doesn't carry over to MotoProject, where the gain is 0.
- The order's "the missing MCP server" as a candidate cause: no failure traces to it.

## Commands and outputs (filtered, verbatim)

All scripts are in `C:\Users\fahim\AppData\Local\Temp\claude\C--Users-fahim-Desktop-Pojects-Nirvana\cdd39677-8dc6-4a13-8f1c-2970c2340402\scratchpad\moto-gemini\`: `agy_stream.py`, `cited.py`, `gate_twin.py`, `proposed_rule.py`, `proposed_rule_strict.py`, `runs_table.py`, `orch.py`, `compact.py`, `r2_partial.py`.

`curl …/oembed?url=…watch?v=<id>` (filter: status code only):
```
3g6_aVqSg_A 404 | kYv9yY8A7bI 404 | Tq_YlH6u7o4 404 | Vl0PjC7k3f8 404 | f9F1aZ2G2m8 404 | Jm3Xf9-uJyo 404 | control dQw4w9WgXcQ 200
```

`python -I gate_twin.py <streams>` (filter: none):
```
reel-motion-gemini.jsonl  cited 12 | master 0 | p35a 0 | in a command at all 0
reveal-gemini.jsonl       cited 18 | master 0 | p35a 0 | in a command at all 17
imp-gemini-aspect.jsonl   cited 17 | master 0 | p35a 1 | in a command at all 9
imp-gemini-transition     cited 12 | master 12 | p35a 12 | in a command at all 12
v13-drone-research.jsonl  cited 12 | master 0 | p35a 0 | in a command at all 0
reel-motion-gemini-r2     cited 22 | master 0 | p35a 0 | in a command at all 21
```

`python -I cited.py --live …r2.jsonl` (filter: `summary`):
```
summary: in stream 22 / 22  youtube 10 exists 10
```

`python -I runs_table.py` (filter: totals line):
```
{('agy','delivered'): 20, ('agy','gated'): 7, ('claude','delivered'): 16, ('claude','gated'): 2}
```

r2's gate event (journal seq 2194, filter: axes marked fail):
```
owned fail 3958 file(s) changed that the order does not own
sources fail 22 source(s) cited that were not fetched, read or browsed
```

## What I didn't do, and what I'm unsure of

- I wrote no `report.md`: the harness forbids report files, so this message is the report.
- I didn't verify the Reddit ids (403 and 429). The fabrication finding stands on the 6 YouTube ids.
- The proposed rule is a Python model of a rule, not Rust, and it was run on one project's streams.
- Whether the orchestrator skipped gating on purpose is not in its transcript. That it skipped is measured: 1 of 26 deliveries gated in that window.
- `reel-motion-gemini-r2-lr` is seated but hasn't started yet, so whether it keeps a lesson is unknown.
- I didn't check P35b's worktree code, only its plan text.