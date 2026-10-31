## Addendum: how the agy briefs were written, and a brief strategy for Nirvana Claude

**Did the brief cause the failure?** Yes. The one invented-sources order was the only research order that didn't say "open every URL". Four research orders with that line produced only real sources, including one on the same flash-low seat. Length didn't matter: the 171-word imp-gemini-transition brief worked and the 165-word reel-motion-gemini brief didn't.

The briefs also never told agy *how* to read a page or check its own work. The orchestrator's prompting rules for agy cover video files only; nothing covers web research.

### 1. What the briefs looked like

I measured every order the journals seated (`briefs.py`, herdr's footer removed). The flag columns come from regexes, so read them as approximate. Output, the two summary lines verbatim:

```
agy: n=19 median lines 21 words 276 steps 2 paths 6 | check 5/19 owns 13/19 done 3/19 verify 10/19 profile/lessons 0/19
claude: n=16 median lines 36.5 words 563.5 steps 6.5 paths 10.0 | check 5/16 owns 14/16 done 2/16 verify 1/16 profile/lessons 0/16
```

The agy research.web briefs:

| Brief | Lines / words | Numbered steps | Files named | How to read a page | Verify-sources line | Outcome |
|---|---|---|---|---|---|---|
| reel-motion-gemini | 7 / 165 | 0 | 1 | none | **no** | 12 of 12 invented |
| reveal-gemini | 5 / 177 | 0 | 1 | none | yes | 18 of 18 real |
| imp-gemini-aspect | 5 / 189 | 0 | 2 | none | yes | 17 of 17 real |
| imp-gemini-transition (flash low) | 5 / 171 | 0 | 2 | none | yes | 12 of 12 real |
| reel-motion-gemini-r2 | 31 / 424 | 6 | 3 | names a "YouTube connector" that doesn't exist | yes | 22 of 22 real; false send-back |

**Findings:**

- **A check with no command is graded by the worker itself.** reel-motion-gemini's brief said "Check: file exists, >=12 sourced items". The worker reported "Source count check (>= 12 items, 6 YouTube + 6 Reddit)", result "pass", after opening nothing. Agy's guidance asks for "a local verification mechanism"; none of the research briefs gave one.
- **No brief says how to read a page.** Agy's `search_web` returns vertexaisearch redirect links, YouTube needs `yt_dlp`, and Reddit needs `.rss` with a user agent. None of the five briefs says any of that.
  - r2 spent 9 of its 106 tool steps reading other orders' streams (v13-drone-research, imp-gemini-aspect, reel-motion-gemini-lr) to find the method.
  - It then hit about 10 failed Reddit reads (403, 429, XML parse errors) before `.rss` worked.
- **The brief named a tool that doesn't exist.** r2 was told "YouTube through your YouTube connector". Its init lists no MCP servers and it made no `call_mcp_tool` call; it fell back to `yt_dlp`.
- **Project rules don't reach agy unless the brief quotes them.** agy doesn't read `CLAUDE.md` (plan 35 §1.2).
  - The GPS rule was quoted in every research brief and held.
  - The E: rule for working files lives only in the reel-editor profile, so research orders never got it. r2 downloaded 6 YouTube mp4s into agy's scratch folder on C:.
- **A stale question.** reel-motion-gemini asked about "10-slide… held stills" weeks after the reel became 13 slides of 360 video. r2 fixed that by naming `plan/post_alum_cave_v13.json`.
- **A capitalised `Owns:` is silently ignored.** The gate parses lowercase `owns:` only (`nirvana_gate.rs:188`), so reel-motion-gemini got "the order names no owned file".
- **Waits in the background stall the worker.** v12-smudge-fix (argon high) waited on its own background render ("Still waiting for the draft render (`task-309`)"), went 600 s silent and stalled. herdr's footer forbids background processes, yet the brief asked for a render with no line saying to run it in the foreground or how long it takes.
- **No order carried a profile or lessons (0 of 35).** MotoProject has no lessons folder, and the reel-editor profile is for editing, not research. So every research brief was retyped by hand, and the method learned on Oct 23 ("REAL URLs ONLY") never became a standing rule.
- **Against Claude.** The Claude orders here are all builds, about twice as long, with more steps and paths. There's no Claude research.web order to compare with directly. Claude builds also rarely state a check (5 of 16) or what done means (2 of 16), so a missing verification step is a habit of the orchestrator, not an agy-only gap. It does more damage on agy, because agy reports SUCCESS on a self-graded check.

### 2. The prompting rules as they stand

- **ROLE.md** says nothing about writing a brief beyond "Gate each delivery" and keeping the owner's words.
- **The seat skill, §3,** asks for "the goal, the files it owns…, the check that proves it, and the owner's words". It has nothing specific to agy.
- **The media skill** has the one measured agy prompting rule: name the absolute path and tell the worker to use `view_file` (its probe P2). That's the precedent: agy needs the tool named.
- **herdr's own text:** the first line, the footer and the sources line (`nirvana_worker.rs:457-473`, `:3454`) don't tell the worker to read before citing, how to read, or how herdr checks it.

### 3. Agy brief strategy: rules for ROLE.md and the seat skill

Each rule names the failure it would have prevented.

1. **Every agy research or media order carries the verify line:** "Never cite a URL from memory or from a search result. Cite only what you opened in this session; drop what you couldn't open."
   - Prevents F1, the invented sources. Evidence: 4 of 4 orders with the line were real, 1 of 1 without it was invented, on the same flash-low seat.
2. **Name the read method for each kind of source, and name only tools the worker really has.**
   - For search links: "`search_web` links are vertexaisearch redirects; resolve each with `curl -sI` and use the `Location`."
   - For YouTube: `python -m yt_dlp --skip-download --print "%(id)s|%(title)s|%(channel)s" <url>`, plus `--write-auto-subs` when a transcript is needed.
   - For Reddit: `curl -s -A "<ua>" <thread>/.rss`; on a 429, wait for the reset.
   - Prevents the "connector" mistake and r2's 9 method-hunting steps plus about 10 failed Reddit reads.
3. **"Done means" ends with a command, and its output goes into `tests`.** A check written as prose is graded by the worker.
   - For research: one last command that re-opens every cited id, e.g. `python -m yt_dlp --print id <url…>`, with its output pasted.
   - This follows agy's verification-loop guidance and prevents F1's self-graded "pass".
4. **A temporary rule until the sources check is fixed: cite exactly the URL you fetched, and run one `curl` line per cited URL on its own line.** That's the only form master's check (and P35a's) counts.
   - This works around F4 rather than fixing it. Drop it when the gate's sources rule changes (item 1 of §4 in the main report).
5. **Write `owns:` in lowercase, one line per path.** Prevents the silently ignored `Owns:`.
6. **Quote the project rules the order touches, word for word,** because agy doesn't read `CLAUDE.md`. The quoting can stop once P35b copies `CLAUDE.md` to `AGENTS.md`.
   - Rule 7 below would also have kept r2's mp4s on E:.
7. **Say where working files go, and that long steps run in the foreground.** For example: "a draft render takes about N min; run it in the foreground; never wait on a background task."
   - Prevents the v12-smudge-fix stall.
8. **State the current state with absolute paths** (the current plan file, the latest report) and the owner's words verbatim. A redo after a send-back says "none of the old content is trusted."
   - Prevents the stale-question failure.
9. **Order the brief as explore, then read, then write, then verify,** as agy's guidance does: find candidates, open each, write the file, run the done-check. Prevents the search-then-write shortcut F1 took.
10. **Carry the method in a profile, not in retyped prose.** Give research.web a `web-researcher` profile whose body holds rules 1-4 and 7, so every agy research order gets it through `--profile`.
    - Addresses F7 (nothing learned) and the owner's "stronger prompting strategies".
    - Cost: one profile file plus a ROLE line saying to use it.
    - Risk: none of rules 1-4 has been tested yet as profile text, so check it on the next research order.
11. **Not a brief rule, but say it plainly: prose doesn't make the orchestrator gate.** ROLE step 5 already says "Gate each delivery", and only 1 of 26 deliveries was gated in that window. Enforcement has to be in herdr (items 3-4 of §4 in the main report).

**Checklist form for the seat skill §3 (agy research or media order):**
- [ ] class line
- [ ] lowercase `owns:`
- [ ] the owner's words verbatim
- [ ] current state as absolute paths
- [ ] project rules quoted
- [ ] the verify line
- [ ] a read method per source type (only real tools)
- [ ] where working files go, and foreground only
- [ ] a "done means" that ends in a command whose output goes in `tests`
- [ ] the cited URLs equal the fetched URLs
- [ ] `--profile web-researcher`

### Checked, wrong, and not done

- **Checked:** the ROLE.md, seat skill, first-line and footer contents as quoted above.
- **Agy's best-practices page:** it covers verification loops, explore-plan-execute, `AGENTS.md`/`GEMINI.md` rules files and `@` paths. It says nothing about citing sources, so rules 1-4 are mine, not agy's.
- **Unsure:** the brief-shape flags are regexes. The finding that the verify line matters rests on 4 orders against 1, so it's a strong signal, not a measurement.
- **Not done:** I edited none of the prompting files and ran no probe of the template.
- **Scripts:** `C:\Users\fahim\AppData\Local\Temp\claude\C--Users-fahim-Desktop-Pojects-Nirvana\cdd39677-8dc6-4a13-8f1c-2970c2340402\scratchpad\moto-gemini\briefs.py`