Read-only map of Nirvana-Herdr (master, alpha 0.44.0). I changed nothing, ran no builds or tests and touched no process. Line numbers are in the `src/` files named. I did not read the digest builder (`nirvana_claude.rs:292`), the stall and re-seat paths, or any test. Anything marked "unsure" is a gap in my reading.

**The short version for your design:** there is no profile, persona or per-project worker config today. Every Claude worker gets the same launch recipe, and the only per-project setting is the seat mode. Learning exists, in two forms. A gated lesson per order class is offered to later briefs, and a post-run retro writes proposals the owner applies by hand. A profile is a new object, but a lot of what it should bundle already has a home.

## 1. How a worker is configured today

**Seat model.** `herdr seat pick` chooses a seat; `herdr seat run <seat> --order <file>` launches it.
- A seat is one CLI, model, family, pool, effort list, capabilities list and price/use schedule (`nirvana_seat.rs:282-300`).
- The table is `<home>/nirvana/seats.toml`, defaulted from `src/nirvana_seats.toml`. It has `[constants]`, `[size.*]`, `[[pool]]`, `[[seat]]` and `[[class]]`.
- Seat kinds are `cli = "claude"` and `cli = "agy"`. The launcher refuses anything else (`nirvana_worker.rs:206-251`).
- A class holds `chain`, `blast`, `effort`, per-mode efforts, `needs`, `weight`, `bars`, and a score-ranked seat list (`nirvana_seat.rs:330-362`).
- The seat table is a prior, owned by the owner once written. Mode is `efficient`, `balanced` or `sota` (`nirvana_seats.toml:11-12`).
- Selection flags are `--class --size --needs --not-family --mode --book --run --override --reason` (`nirvana_seat.rs:3732-3776`). Efficient and SOTA modes and the `5%` floor and `15%` reserve are in the constants.

**Claude worker argv** is fixed code at `nirvana_worker.rs:202-240`:
- Fixed flags: `-p <one-line prompt>`, `--output-format stream-json`, `--setting-sources project`, `--strict-mcp-config`, `--model`, `--effort`, `--permission-mode auto`, `--json-schema <Work Report>`.
- No `--append-system-prompt`, `--agent` or persona. The prompt is `first_line()` (`:354-358`), "Do the work order in <file>", and `ORDER_FOOTER` (`:362-366`) is appended to every order.
- agy gets `--model <id-with-effort> --dangerously-skip-permissions --json-schema --print=<prompt>`.

**What varies per worker today:**
- **`--settings <file>` (Claude only).** The file is the folder's Preload preset, `<home>/nirvana/preload/<hash>.settings.json`, with the plan 28 superpowers rule layered on top (`:264-322`, `nirvana_superpowers.rs`). Superpowers are forced off on Opus and left alone on Sonnet. The launch files go to `<home>/nirvana/launch/`.
- **A per-order `--mcp-config`** holding only the `.mcp.json` servers named in the order's `needs`, plus `--allowedTools mcp__<server>__*` (`nirvana_gui_tools.rs:1-26`, `nirvana_worker.rs:325-345`). The file is `.nirvana/runs/<run>/mcp/<order>.json`, removed when the worker ends. A `needs` word naming a server forces a Claude seat (`nirvana_seat.rs:3771`). Names match exactly. This is the closest existing thing to a profile's MCP bundle.
- **Brief lines appended by code:** the lessons section (`nirvana_worker.rs:3158`), device and Send lines (`nirvana_gui_tools.rs`, `with_brief_lines`), a `sources` line for research and media classes (`:3175`), and a bladebro batch line for browser orders (`nirvana_lean.rs`, `with_browser_line`).
- **Order-file gate lines**, at line start: `Class:`, `Order:`, `Run:`, `owns:`, `test:`, `check:`, `serves:` (`nirvana_gate.rs:36-47`).

**Per-project settings today.**
- `<project>/.nirvana/seats.toml` holds one key, `mode`. Any table or other key is refused, with the path and line (`nirvana_seat.rs:3102-3190`, plan 23). `seats.toml` itself stays one file per machine; plan 23 says a per-project table is "a later plan, if the owner asks" (`2026-10-22-23-project-mode.md`).
- Other contents of `<project>/.nirvana/` are `board.md`, `lessons/`, `retro/gaps.jsonl`, `proposals/`, `runs/<run>/` and `orders/`. I did not trace `.nirvana/.gitignore` beyond what the module headers say.
- Machine-level files under `<home>/nirvana/` are `preload.toml`, `preload-catalog.toml`, `jev.toml`, `usage.json`, `leases.json`, `record.jsonl`, `NIRVANA.md`, `sessions/`, `fresh/` and `accounts.json`.
- The Preload preset (`nirvana_preset.rs:204`, `nirvana_preload.toml`) is a tag-based allowlist: `keep` tags plus `auto_memory`, chosen by folder-type rules and written to the generated `--settings`. It is per-folder-type and automatic, not user-designed. It governs skills, MCP and plugins by tag, on and off only.
- There is no persona or profile concept anywhere. A grep of `src/nirvana*.rs`, `nirvana/claude` and `nirvana/plans` found no persona or profile meaning, only unrelated uses of the word.

## 2. Nirvana Claude (the orchestrator) setup

- **Launch argv** (`nirvana_launch.rs:14-15, 212-224`): `claude --plugin-dir <home>/nirvana/claude --agent nirvana --model claude-opus-5-5 --effort medium --append-system-prompt-file <plugin>/ROLE.md --name Nirvana --settings <file>`, with `NIRVANA_ORCHESTRATOR=1` in the environment.
  - `--agent nirvana` has an empty body so Claude Code's own system prompt is kept (`nirvana_launch.rs:18-28`).
  - The model and effort follow the mode through `herdr seat orchestrator` (plan 20).
  - `--settings` is the folder's Preload file when one exists, else `nirvana/claude/settings.json`. On Opus it becomes a launch file with superpowers off.
  - Plan 25 adds `--disallowedTools ReportFindings` (`nirvana_lean.rs:12-30`).
- **Doctrine text** is `nirvana/claude/ROLE.md`, 82 lines. It covers the workflow, rules each tagged with its incident (seat every order, the spawn hook, Send approval, jobs) and the "small asks" rule.
- **Plugin parts** (`nirvana/claude/`):
  - `hooks/hooks.json`: SessionStart runs `nirvana digest`, `nirvana log-usage` and `memory record-session`. UserPromptSubmit runs `board notice --plugin`, `run notice` and `nirvana prompt-baseline`. PreToolUse on Agent runs `nirvana guard-spawn`. There are also `guard-edit`, `check-edit`, `subagent-stop`, `jev compact` and `nirvana stop` hooks, which I did not open.
  - `agents/worker-{opus-high,opus-medium,sonnet-medium,sonnet-low,haiku-low}.md` and `nirvana.md`. Each worker file fixes its model and effort in frontmatter and carries a short worker persona (do only the order, change only owned files, Work Report) (`worker-sonnet-medium.md`). The orchestrator's subagent spawns are a ready-made profile precedent: name, model, effort, tools and prompt in one markdown file.
  - `skills/{gate,handoff,jev,jobs,media,seat,usage}/SKILL.md`.
- **Session digest** (SessionStart, `nirvana_claude.rs:292`) names pools, seat, board, open runs, open retro proposals and `NIRVANA.md`.
- **Fresh session after a run** (plan 25 §2, `nirvana_fresh.rs:1-30`): the Stop hook marks a pending fresh start, the server types `/clear` into an idle pane, and the SessionStart hook prints a handoff note of at most 2,000 tokens built from files. The note holds the open runs, the owner's last three prompts, the board and lessons. No model writes it.

## 3. Anything memory- or learning-shaped

**Written after a run, read by a later run: yes, three channels.**

1. **Lessons** (`nirvana_lessons.rs`).
   - A worker's Work Report may carry `lesson {name, class, steps (at most 120 lines), evidence}` (`nirvana_worker.rs:44, 111-136`, and `ORDER_FOOTER` tells workers to include one).
   - It is written to `<project>/.nirvana/lessons/<slug>.md` only if the order passed the gate, the evidence quote is in the worker's own stream, and the repository has a GitHub remote (`nirvana_gate.rs:1985-2012`, `nirvana_lessons.rs:259`).
   - The next order of the same class gets one line per live lesson in its brief, never the body (`offer`, `:370`, called at `nirvana_worker.rs:3158`).
   - Counters in `lessons/index.json` are offered, opened and passed-after-opened. A lesson decays out of briefs after 6 offers unless at least one in 3 led to an order that opened it and passed (`DECAY_AFTER=6`, `HELP_EVERY=3`, `:29-36`).
   - `herdr lessons for --class C` reads them. The owner's rule is "checked lessons go live automatically, per GitHub remote, in their own project".
   - Lessons are keyed by class, not by profile. This is the existing "learning after a run" and your feature (b) should extend it.
2. **Retro** (`nirvana_retro.rs`). `herdr run end` runs it with no model call.
   - Gaps come from journal events: send-backs, rate limits, stalls, re-seats, overrides, failed workers, gotchas. Each has a verbatim quote, and the gap ledger is `.nirvana/retro/gaps.jsonl`.
   - A gap graduates after 3 distinct runs, or 1 outcome-tagged miss. A graduated gap becomes `.nirvana/proposals/<id>.md` in the CHANGELOG shape. Proposals are the owner's to apply or refuse, and a refusal is permanent (`retro reject`).
   - The retro may propose only expected-use cells of seats.toml. It never touches gates, decisions, CLAUDE.md or seats constants (`:15-17`).
   - The digest and `receipt.md` list open proposals.
3. **`NIRVANA.md`** (`<home>/nirvana/NIRVANA.md`, `nirvana_claude.rs:44-45`): one line per project for the board and lessons, read at the digest.

**Other memory-shaped parts:**
- **Seat record:** `record.jsonl`, one line per finished order with its tokens. It stays off the rule (`use_record = false`), and decision 0029's blend formula is in the constants.
- **Board:** `.nirvana/board.md` is one shared file per workspace. A revision counter refuses stale writes, `board notice` pushes changed sections into the next prompt, and it is built for any provider through `herdr board read|write` (`nirvana_board.rs:1-30`). The Memory square is its panel (`client/shell/nirvana_memory.rs`).
- **JEV.md** is `<home>/jev/JEV.md`, the Jev usage instructions for big files. It is not project learning.
- **Session record** `<home>/nirvana/sessions/<hash>.json` is resume-only (`nirvana_session_record.rs`).
- **Memory trim** (`nirvana_memory_trim.rs`) shortens an auto-memory index over 4k tokens, with a proposal and a backup. Preload presets also switch `auto_memory` on and off.

**What happens when an order finishes** (`nirvana_worker.rs:1500-1520`): the supervisor validates the Work Report, journals `Delivered {report}` (plus `Asked` if `needs_answer`) and `Ended {delivered}`, then releases the lease. `herdr run wake` and `run notice` queue the event into Nirvana Claude, which gates with `herdr gate run`. That checks rebase, tests with named checks, the report, `owns:` and `served:`, then held-out and review for larger orders. A fail sends back a child order `<root>-s<n>`, at most 3, and the fourth asks the owner. A pass journals `Gated` and `herdr gate merge` merges into `nirvana-integrate/<run>` only. `run end` writes `receipt.md` and `receipt.json` and runs the retro.

**Gaps:** nothing summarises a run with a model or writes a skill file. Lessons depend on the worker volunteering one, which is a hypothesis nobody has measured. I did not look for evidence of how often workers actually emit a `lesson`. That would be `.nirvana/lessons/` in real projects.

## 4. `nirvana/plans/` touching your topics

| Plan | Status line |
|---|---|
| `2026-10-16-08-memory-square-and-board` | Built, alpha 0.8.0. The log says lessons are "the next feature". |
| `2026-10-17-10-jev-memory-file` | Built, alpha 0.10.0 |
| `2026-10-17-12-nirvana-claude-orchestrator` | **BUILT** 0.12.0-0.21.0. Holds the routing table, lessons §7.4.3 and retro §7.4.2. The records are in `…-12-records/o5.md`. |
| `2026-10-18-13-smart-preload` | **BUILT** 0.23.0-0.26.0. Presets and the memory trim. |
| `2026-10-18-14-seat-spend-down` | Approved and building. The status line says "APPROVED 2026-10-18, building on agy"; plans 15-17 below it are marked built, so I read it as built but did not confirm. |
| `2026-10-20-19-seat-modes` | **BUILT** 0.31.0 |
| `2026-10-20-20-seat-every-spawn` | **BUILT** 0.32.0 |
| `2026-10-22-23-project-mode` | **PLANNED** per its own status line. A per-project mode file was clearly read by the code at `nirvana_seat.rs:3102`, so I read it as landed. Mark this unsure. |
| `2026-10-22-24-run-fixes` | PLANNED (it also shows built work; same caveat) |
| `2026-10-22-25-lean-orchestrator` | **BUILT** 0.42.0 |
| `2026-10-23-28-superpowers-by-model` | PLANNED on its line, but the code is in `nirvana_superpowers.rs`; the status line is stale. |
| `2026-10-26-29-gui-tools` and `2026-10-29-31-gui-tools-in-use` | P29c built 0.43.0; P31a built 0.44.0; P31b and P31c built and handed to the owner, not merged |

Only plan 23's closing remark on a per-project seat table touches per-project settings. None of the other plans is titled for profiles, personas or learning.

## 5. Controls

`nirvana/controls.py` defines `Control(name, file, find, replace, test, kind)`. Each control is a byte-exact sabotage edit to one file, plus the one test (a nextest filter, or a unittest id for Python) that must go red, restored in a `finally`. Rust controls recompile, about 30 s each.

`controls.py` loads every `nirvana/controls.d/<package>.py` that defines `CONTROLS = (Control(...), ...)`, so parallel packages stop colliding in one tuple. A new feature adds `controls.d/<plan>.py`, prefixes names with the plan (e.g. `p25_`), and writes the expected red test names before the run. The tracked-file rules are in the repo CLAUDE.md, rules 9 and 10.

Run `python nirvana/controls.py --dry-run` to list them. `--only` takes exact names. Per your memory notes, run controls in a throwaway worktree, not a live one.

## Where a profile could attach

- **Claude worker spawn:** `nirvana_worker.rs::argv`/`worker_argv` is the one place the argv is built; today it passes no system prompt.
- **MCP bundle:** the `.mcp.json` order machinery in `nirvana_gui_tools.rs` already does per-order MCP selection.
- **Settings:** the `--settings` layering already stacks Preload then superpowers.
- **Seat model:** the `[[class]]` and `[[seat]]` tables already hold model, effort and fallbacks, but per machine.
- **Memory scope:** the lessons index is per project and keyed by class, and the board is per workspace.