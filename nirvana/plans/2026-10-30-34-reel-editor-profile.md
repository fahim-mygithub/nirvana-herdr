# Feature 34: a reel-editor profile with tools and a knowledge base

Status: **P34a BUILT (2026-10-31, alpha 0.46.0); P34b built in moto-reel-mcp; P34c staged.** Was PLANNED (2026-10-30). Three packages. P34b and P34c start now. P34a starts on master once plan 33 is merged.
The evidence is the MotoProject map of 2026-10-30, `nirvana/plans/2026-10-30-34-records/moto-map.md`.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "The other feature is the ability to make an agent profile. One example is my MotoProject. I want to create an agent that can ontrol the reeleditor and be able to make different styles and reel have access to tool and have a knowledge bases to pull from as it creates reels or gets taught new techniques." | all | builders | §2-§4 tests, §6 live checks |
| L2 | "Allow .nirvana/profiles only (Recommended)" (answer to: where the reel-editor profile and its knowledge cards go, given the rule never to write under D:\MotoProject) | P34c §4 | orchestrator | the copy step |
| L3 | "New repo outside D: (Recommended)" (answer to: where the reel-editor MCP server lives) | P34b | P34b | §3 tests |
| L4 | "Extract and go live" (answer to: how the first knowledge cards are made from CLAUDE.md, notes and the motion-skills plan) | P34c | P34c | §4 card checks |

- Every other path under D:\MotoProject stays read-only for every builder and for the orchestrator.
- Not in scope: changing the editor itself (the missing `stabilise` and drift-pose patching, a manifest schema). The
  MCP server reports those as missing tools.

## 1. What was measured

- The editor is FastAPI on :8770 (`editor/server/main.py --port`), about 51 routes, files on disk authoritative,
  writes through `review_marks.write_atomic`. Test suites that hit :8770 wrote into `plan/` (MotoProject CLAUDE.md).
- No `.mcp.json`, no git: an edit to `plan/*.json` cannot be undone, and a re-plan with `overwrite:true` drops
  hand-set framing.
- A style is a template: `templates/<name>.json` under `schemas/template.schema.json` (pacing, select, shots,
  transitions, grade). Existing: `dusk_drift`, `golden_hour`, `hard_cut`, `machine`. Presets in `angles/`, grades in
  `profiles/hike.json` and the templates, moves in `render.MOVE_EASE`.
- Knowledge is prose: CLAUDE.md (about 1,100 lines, DEAD END notes), `notes/`, `docs/plans/` (the motion-skills
  extraction: TAKE or REJECT per item, against measurements), one skill `.claude/skills/hike-compose`, and the owner's
  Claude Code memory for the folder. Nothing in `notes/` or `docs/` reaches a worker unless an order names it.

## 2. P34a: profile folders, knowledge cards, search and teaching (Nirvana-Herdr)

Built on master after plan 33's merge, in its own worktree. It extends `src/nirvana_profiles.rs` and the lessons code.

1. **A profile may be a folder.** `.nirvana/profiles/<name>/profile.md` (the same format as `<name>.md`) plus
   `knowledge/<slug>.md`. A name that exists as both a file and a folder is refused. The run copy (plan 33 §2.2) copies
   `profile.md` only; cards are read live, like lessons.
2. **The card format.** Front matter, then a body of at most 60 lines:
   ```markdown
   ---
   name: Hold after the climax
   when: The last cut lands on a peak move or a vista.
   verdict: keep          # keep | reject | dead-end
   effect: Owner preferred 2 of 2 drafts with a 1.5-beat hold; unmeasured beyond that.
   source: D:\MotoProject\docs\plans\2026-08-04-motion-skills-extraction.md:142
   date: 2026-08-04
   tags: [pacing, closer]
   ---
   ```
   `name`, `when`, `verdict`, `source` are required. Unknown keys are refused, naming the file and line.
3. **Offered like lessons.** An order seated with a profile gets one line per live card (`name — when`, the path),
   most helpful first, at most 15, then "N more: `herdr kb search --profile <name> <words>`". Counters and decay are the
   lessons' own (offered, opened, passed after opened), kept in `knowledge/index.json`. A card with verdict `reject` or
   `dead-end` is offered as a warning line ("do not: ...").
4. **Search.** `herdr kb list|show|search --profile <name> [words]`: keyword search over name, when, tags and body,
   no model call (Hermes' session search is the same). Results print the path, never more than 20.
5. **Teaching.** `herdr profile teach <name> --from <url|path> [--note "<what to learn>"]` writes and seats an order of
   a new class `profile-teach`, with the profile applied. Its brief is the owner's motion-skills method: read the source
   as data, never run it; test each technique against the project's own measurements where it can; give each one a
   verdict. Its Work Report carries `card_ops` (`new` or `patch` with `base_sha`, as plan 33's `lesson_ops`). herdr
   applies an op only when its evidence (at least 20 characters) is in the teacher's stream and was not written by
   herdr. Teaching is started only by the owner or by Nirvana Claude quoting the owner (plan 31's quote rule).
6. **Reviews feed cards.** Plan 33's post-gate review of a profiled order may also return `card_ops` for that profile,
   under the same checks.
7. **The panel.** The Profiles group shows a folder profile's card count; `k` opens `knowledge/` in an edit tab.
8. **Tests and controls** for each item.

## 3. P34b: the reel-editor MCP server (new repo `C:\Users\fahim\Desktop\Pojects\moto-reel-mcp`)

Python, an MCP server over stdio, with its own git, tests and README. It reads the MotoProject code and runs its CLIs;
it never edits MotoProject source.

1. **Its own editor.** The server starts `editor/server/main.py` on a free port it picks (never 8770), against the
   project root it is given (`MOTO_REEL_ROOT`, default `D:\MotoProject`), and stops it on exit. It refuses to talk to
   :8770.
2. **Tools.** `project_read`, `manifest_read`, `manifest_validate`, `segment_patch` (the PATCH fields only),
   `style_list` (templates, presets, grades, moves, with their schema), `style_show`, `style_create` (a new template,
   validated against `schemas/template.schema.json`, never overwriting an existing name), `plan_from_template` (never
   `overwrite:true`; a new plan file name), `render_draft`, `render_status`, `frame_grab` (writes a still and returns
   its path, never image bytes), `missing_tools` (lists what the editor cannot do yet, from the map).
3. **Undo.** Before any write to `plan/`, `templates/`, `angles/` or `marks/`, the server copies the file to a
   snapshot store (`MOTO_REEL_SNAPSHOTS`, default `%LOCALAPPDATA%\moto-reel-mcp\snapshots`, never on D:) and journals
   it. `snapshot_list` and `snapshot_restore` undo one write.
4. **Renders.** Draft first: `render_full` refuses until a draft of the same plan exists and the plan has not changed
   since. Renders are started in the background and resumed from cache; a tool call never waits more than 60 s.
   Intermediates go where the project's rules say (E:).
5. **Tests** run against a fixture project in a temp folder: copies of the editor code, schemas, templates, angles and
   one small plan JSON, no media. Renders are stubbed. No builder runs the server or the editor against
   D:\MotoProject.

## 4. P34c: the reel-editor profile and its first cards (staged, then copied)

A worker drafts, in a staging folder outside D: (L2), from CLAUDE.md, `notes/`, `docs/plans/` (the motion-skills
extraction first), `.claude/skills/hike-compose/SKILL.md`, and the owner's memory for the folder
(`~\.claude\projects\D--MotoProject\memory`):

1. `profile.md`: description, `needs: [moto-reel]`, the persona with the fixed rules (cuts on the beat grid, 29.97,
   never write latitude or longitude, limit the friends shots, intermediates on E:, draft before full render, never
   :8770, never `overwrite:true`).
2. `knowledge/*.md`: one card per technique, dead end or owner preference, each `source` a path and line that the
   worker quoted. Target 25 to 40 cards. No card without a source line.
3. A check script verifies each card's front matter and that its source line exists and contains the quoted words.

After P34a merges, the orchestrator copies the staged folder to `D:\MotoProject\.nirvana\profiles\reel-editor\`
(L2: the only path written there) and runs `herdr profile check reel-editor`.

## 5. Order

- P34b and P34c now, in parallel. P34a after plan 33 merges. The copy into D: after P34a merges and P34b is handed over.
- The owner adds the `moto-reel` line to `D:\MotoProject\.mcp.json` (given at hand-over); the orchestrator does not
  write it.

## 6. Live checks for the owner

1. Seat a reel order with `--profile reel-editor`: the worker lists the `moto-reel` tools and the card lines.
2. Ask it for a new style: a `templates/<new>.json` appears, validated, and `snapshot_list` shows nothing overwritten.
3. `herdr profile teach reel-editor --from <a tutorial link>`: read the new cards and their verdicts.
