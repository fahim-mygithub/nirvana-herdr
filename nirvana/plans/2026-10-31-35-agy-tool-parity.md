# Feature 35: agy workers get the tools Claude workers get

Status: **PLANNED (2026-10-31).** Three packages. P35a and P35b start in parallel on master `9304a821` (alpha 0.46.0).
P35c starts after P35b merges and after the owner rules on R1.
The evidence is the agy research of 2026-10-31 (`2026-10-31-35-records/agy-research.md`, its scripts and probe streams), two plan probes run
for this plan (`2026-10-31-35-records/probes/`, expectations in `EXPECT-PLAN.md`, written before each run), and the code read on master.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "I feel like there is a gap between running agy natively vs having Claude Spawn an agy agent. ... Then check if there are better ways to lauch or have agy more tool scope. Maybe how herdr launches is a gap?" | §1 and §2 (the answer); P35a, P35b | orchestrator, builders | §1 measurements; §6 live checks 1, 4 |
| L2 | "Can agy not use bladebro?" | P35c | P35c | §1 probe 2 (it can); §5 P35c tests; §6 live check 1 |
| L3 | "Have herdr give similar tool abilities if they need it the same way we manage claude tools as preloads and mccp servers." | P35b (servers, rules, skills, denials), P35c (bladebro) | P35b, P35c | §2 parity table; §5 tests; §6 live checks 2-6 |
| L4 | The native agy session's answer, as the owner pasted it: "Right now, I have a search_web tool enabled for searching the web, rather than an interactive browser tool. If your task involves browsing web pages, inspecting the DOM, or interacting with web applications, you can use the /browser slash command to enable browser automation and interaction capabilities." | §1.4 (why `/browser` is not the way) | orchestrator | §1.4 docs and changelog lines |
| L5 | "Yes, both CLIs (Recommended)" (answer, 2026-10-31, to R1: should a browser order get bladebro on either CLI, with its own browser home per order) | P35c | P35c | §5 P35c tests; §6 live check 1 |
| L6 | "Yes, allow (Recommended)" (answer, 2026-10-31, to R4: may orders that need a non-desktop MCP server seat on agy) | P35b | P35b | §5 P35b tests; §6 live checks |
| L7 | "Keep them" (answer, 2026-10-31, to R3: the owner's global agy plugins) | none | owner | nothing to build |

- Not in scope: desktop servers on agy (exebox, §1.3); teaching orders on agy (plan 34 keeps them on Claude); the seat
  table's scores (R2).
- herdr never writes into a project's tracked files or its own `.agents/`, `AGENTS.md`, `GEMINI.md` or `.mcp.json`
  (L3: "the same way", and plan 13's rule that a preset touches nothing in the owner's folder).

## 1. What was measured

### 1.1 A claude worker's tools today (code on `9304a821`, and 123 claude worker streams in 4 projects)

The order's premise was that a claude worker gets Preload choices, MCP servers through `needs`, and bladebro for
browser orders. Only the second is true.

- **argv** (`src/nirvana_worker.rs:220-249`): `claude -p ... --setting-sources project --strict-mcp-config
  --permission-mode auto --json-schema ...`. With `--setting-sources project` none of the owner's user skills, user
  plugins or user MCP servers load. The init line of 122 streams shows 29 built-in tools (WebFetch and WebSearch among
  them), 19 built-in skills and 3 built-in plugins, every time.
- **MCP servers:** only plan 29's per-order `--mcp-config`, from the project's `.mcp.json`, for a `needs` that names a
  server (`nirvana_gui_tools.rs:order_argv`, which returns early for any argv not starting `claude`). 120 of 123 claude
  streams show `mcp_servers: []`. The rest are Bambu GUI orders (`bambu-studio-beta`).
- **bladebro: no claude worker has it.** None of the 123 claude streams names bladebro or agent-browser. Decision
  0043's per-order bladebro config is old Nirvana's (alpha 0.51.0) and was never ported. Plan 25's record said so
  (`2026-10-22-25-records/p25.md`, "The review round's work order was wrong"). Since then `nirvana_lean::BROWSER_LINE`
  ("Use bladebro `run` to batch steps") has gone to browser orders on claude seats, naming a tool the worker lacks.
  The `browser` capability on the opus and sonnet seats is a word the seat pick reads, nothing more.
- **Preload:** the panel (`src/client/shell/nirvana_mods.rs`, `nirvana_preload.rs`) switches items for the
  interactive Claude in a tab. A worker gets the folder's worker preset through `--settings`
  (`worker_argv`, plan 13 §2) plus plan 28's superpowers rule. Every switch turns something off
  (`skillOverrides`, `deniedMcpServers`, `enabledPlugins`, `permissions.deny`, `disableAllHooks`), so on top of
  `--setting-sources project` it removes items that would not load anyway. **No Preload item adds a tool to a worker.**
- **Profiles** (`nirvana_profiles.rs`): `needs` join the order's (`joined_needs`); the body goes in as
  `--append-system-prompt-file`; `disallowed_tools` becomes `--disallowedTools`. Lesson reviews and teachers get
  `--disallowedTools` lists (`with_review_limits`, `with_teach_limits`).
- **Project rules and skills:** `--setting-sources project` loads the project's `CLAUDE.md`, `.claude/settings.json`
  hooks and `.claude/skills/`.
- **Brief lines** (`seat_order_with`): lessons, profile cards, `DEVICE_LINE` / `BRIEF_LINE` / `SEND_LINE` for server
  orders, and the browser line.

### 1.2 An agy worker's tools today

- **argv** (`nirvana_worker.rs:251-271`): `agy --output-format stream-json --model <id-effort> --print-timeout <s>
  --dangerously-skip-permissions --json-schema <schema> [--conversation <id>] --print=<line>`. It has no settings
  isolation, no `--add-dir` and no `--settings`.
- **Tools:** 14 built-ins in print mode, with `search_web` but no URL reader and no browser (research probes 1 and 3).
  It also gets every global item of the owner's: 4 plugins (the gemini-api plugin's MCP server among them), the global
  hooks, and the user permissions.
- **Workspace items it loads by itself:** `AGENTS.md`, `GEMINI.md`, `.agents/rules/`, `.agents/skills/`,
  `.agents/mcp_config.json`, `.agents/agents/` and `.agents/hooks.json` in the project (docs; research probe 2 for
  AGENTS.md and the MCP file). It does not read `CLAUDE.md` (research probe 2).
- **Profiles:** the body is quoted at the top of the order copy. A profile with `disallowed_tools` can only seat on
  claude (`narrow`), and `with_profile` fails such an order on agy outright.
- **Pages:** 370 `python urllib` commands plus `gh api` / `gh repo view` reads, which the gate's sources check does not
  count. That check caused 8 of agy's 12 send-backs (research §4).
- **End of run:** `report()` (`:806-850`) reads `status` and never reads `finished_reason`.

### 1.3 The two plan probes (`plan-probes/`, flash-low, herdr's argv minus `--json-schema`, BROWSER a no-op, 180 s wall)

| Probe | What it tried | Expected (written first) | What happened |
|---|---|---|---|
| 1 | `--settings` file holding `mcpServers` | ignored or an error (60% not listed) | **refused at parse:** `Error: --settings: unknown setting "mcpServers"`, exit 1 in 0.1 s, no session, no tokens. So `--settings` cannot carry servers, and it validates its keys. |
| 2 | cwd holding only `.agents/mcp_config.json` naming `bladebro` (`cmd /c bladebro mcp`, decision 0043's `BLADE_*` recipe, its own `BLADE_HOME`); `--add-dir` a folder holding `.agents/mcp_config.json` (`probeadd`) and `AGENTS.md` (`MARKER-ADDDIR-5521`); `--settings {"permissions":{"deny":["write_file(*)"]}}` | bladebro listed 85%, title read 60%; probeadd not listed (55%); marker read (55%); deny holds (60%) | **All five held.** bladebro was listed with `act see state run vision`. Its call was `call_mcp_tool {"ServerName":"bladebro","ToolName":"act","Arguments":{"action":"navigate","url":"https://example.com"}}` and the answer was "Page title: `Example Domain`". **probeadd was listed** and **the add-dir folder's AGENTS.md was read.** The write was refused: "Permission denied for write_file(...). Matches user-configured deny rule." Exit 0, 19.6 s, 38,471 tokens in, 609 out. After the run, 0 processes named the probe's `BLADE_HOME`. |

What the probes settle:
- **A folder herdr owns, added with `--add-dir`, gives an agy worker MCP servers and rules (measured).** No file goes
  into the project, the worktree, or their `.agents/`. That works whether the order has a worktree or not.
- **bladebro works under headless agy (measured, outside a herdr job object).** Before an MCP call, agy reads the tool
  schemas from `~/.gemini/antigravity-cli/mcp/<server>/<tool>.json`, a cache agy writes itself.
- **A `--settings` deny holds against `--dangerously-skip-permissions` (measured for `write_file(*)` only).**
- **Seen in probe 2: the model wrote into the added folder, not its cwd.** The prompt said "your working folder", and
  it picked `p2\side`. With two workspace folders, "this folder" is ambiguous. §3.3 makes it unambiguous and denies
  writes there.
- Not measured: whether bladebro's server or its Chrome ran on the isolated exebox desktop. A headless Chrome does
  not need a desktop, so it did not matter here. Also not measured: what the five tools cost apart from the steps (the
  38,471 tokens include 4 tool steps, against 9,917 for an empty research probe).

### 1.4 Why `/browser` and the agy flags are not the way (L1, L4)

- `/browser` "Delegate[s] web browsing, UI verification, and in-browser debugging to the browser subagent"
  (cli/reference). The built-in `browser` subagent is "invoked exclusively using the `/browser` slash command"
  (docs/subagents). Print mode refuses "the remaining interactive-only slash commands" (changelog line 706), so a
  headless worker cannot reach it. The owner's native session confirms it: an interactive session also starts with
  `search_web` alone and gets a browser only when someone types `/browser`.
- `--project` takes a "Project ID or project name" (help), the cloud project (`CLI Project`, changelog 850), not a
  folder. It is not used here.
- `--settings` takes "a JSON object of settings.json keys". The CLI reference lists 32 keys and none of them switches
  a plugin or names a server. So herdr uses `--settings` for `permissions` alone (§3.4).
- An interactive agy in a pane (`-i`) would have `/browser`, but no `--json-schema` Work Report and no clean end. It
  is rejected.
- No `agy mcp add` scope flag exists (help: it manages "your user-level `mcp_config.json`", changelog 599), so writing
  servers through it would edit the owner's `~/.gemini`. It is rejected.

## 2. The parity table

Marks: **M** measured, **D** documented, **U** unknown, **X** impossible on agy.

| A claude worker can be given | How, today | agy equivalent | Mark | Package |
|---|---|---|---|---|
| A `.mcp.json` server through `needs` (non-desktop) | per-order `--mcp-config` + `--allowedTools` | the same server in `<tools>/.agents/mcp_config.json`, `<tools>` added with `--add-dir` | M (probe 2, stdio) | P35b |
| A desktop server (`bambu-studio-beta`, `xtool`) | the same | none: agy's shell runs on exebox and cannot see the window (plan 29 §1, MainWindowHandle 0) | X | stays refused |
| An http server, a server using `${VAR}` | Claude Code expands `${VAR}` | `serverUrl`/`url`, `headers` (D); no `${VAR}` expansion is documented | D / U | P35b refuses `${...}` on agy |
| bladebro for a browser order | **nothing today** (§1.1) | bladebro in the tools folder, with its own `BLADE_HOME` | M (probe 2) | P35c, both CLIs |
| WebFetch (a URL reader) | built in | bladebro when granted; otherwise shell fetches, which P35a teaches the gate to see | M | P35a, P35c |
| Preload items off (skills, plugins, MCP, hooks) | worker preset `--settings` | none per order: no settings key switches a plugin, and `agy plugin disable` is global | X (per order) | R3, owner's global choice |
| Project skills (`.claude/skills/`) | `--setting-sources project` | copy into `<tools>/.agents/skills/<name>/SKILL.md` (same front matter: `name`, `description`) | D for workspace skills; U for an `--add-dir` folder | P35b, measured at build |
| Project rules (`CLAUDE.md`) | `--setting-sources project` | the project's own `AGENTS.md`/`GEMINI.md`/`.agents/rules` load natively; when it has none, herdr copies `CLAUDE.md` to `<tools>/AGENTS.md` | M (probe 2) | P35b |
| Profile persona | `--append-system-prompt-file` | quoted at the top of the order copy (today) | M (plan 33) | unchanged |
| Profile `disallowed_tools` | `--disallowedTools` | `--settings {"permissions":{"deny":[...]}}` through a mapping table (§3.4); an unmapped name is refused | M for `write_file(*)`; U for the rest | P35b |
| Lesson-review limits | `--disallowedTools` | the same deny file (`command(*)`, `write_file(*)`) | U | P35b |
| Teacher limits | `--disallowedTools` + `--add-dir` | not built: teaching stays on claude (plan 34) | — | none |
| The order text, device and Send lines | the brief | the same brief, which already does not depend on the CLI | M | unchanged |
| Strict MCP (nothing else loads) | `--strict-mcp-config` | none: agy always loads global, plugin and workspace servers | X | risk §7.2 |
| Subagents | the Agent tool, held by the footer | `invoke_subagent` and friends, held by the same footer | M | unchanged |
| Questions to the owner | `needs_answer` | the same, since headless agy settles its own questions (changelog 1.1.12) | M | unchanged |

## 3. The design: one tools folder per agy order

### 3.1 Where

`<project>/.nirvana/runs/<run>/agy/<order>/` (the **tools folder**) sits beside plan 29's `mcp/<order>.json`, inside
`runs/`, which `.nirvana/.gitignore` ignores (`nirvana_run::GITIGNORE`). It is the same for a worktree order, a
root order and a lesson review, so the non-worktree question goes away: no order writes into the cwd's `.agents/`.
The argv gets `--add-dir <tools>`. That folder holds only what herdr writes:

```text
<tools>/.agents/mcp_config.json      the order's servers (P35b; bladebro from P35c)
<tools>/.agents/skills/<name>/       the project's .claude/skills, copied (P35b, if measured to load)
<tools>/AGENTS.md                    the project's CLAUDE.md, when it has no AGENTS.md / GEMINI.md / .agents/rules
<run>/agy/<order>.settings.json      the deny rules, passed with --settings (outside <tools>: not a workspace file)
```

An order with no servers, no skills to copy, no rules to copy and no denials gets no `--add-dir` and no
`--settings`, so its argv stays byte for byte what it is today.

### 3.2 Servers (L3: "the same `needs`")

- `seat_needs` and `claude_only` (`nirvana_gui_tools.rs`) change in one way. A non-desktop server need no longer
  makes an order claude-only. A desktop server still does, and so does a definition agy cannot take: one that uses
  `${...}` or has a `type` other than `stdio`, `http` or `sse`. Each says why, as now.
- `order_argv` takes both CLIs. For claude nothing changes. For agy it writes the same chosen servers, translated:
  - `command`, `args`, `env` and `cwd` pass through;
  - `url` becomes `serverUrl`, `headers` passes, and `type` is dropped;
  - a server the workspace's own `.agents/mcp_config.json` already names is left out, with one line, because it loads
    anyway and which copy wins is not documented.
  Review F3's rule holds: a server the order was seated for and cannot get fails the order before it starts.
- `remove_order_config` also removes the tools folder and the settings file when the worker ends, whatever the
  outcome (review F2's rule), with one line if a remove fails.
- The brief lines (`with_brief_lines`) already go by the booked servers, not the CLI.

### 3.3 The prompt names the folder

For agy, `first_line` becomes: "Do the work order in <abs>. Your folder is <cwd>; work only there. <tools> holds
herdr's tool config: never write there. End with the Work Report your output schema asks for." Every `write_file`
under `<tools>` is also denied in the settings file (`write_file(<tools>/)`, the path form from docs/permissions; the
build measures it, §5). This fixes what probe 2 showed (§1.3).

### 3.4 Denials (profile `disallowed_tools`, review limits)

A fixed table in `nirvana_agy_tools.rs`. Each row is measured once by the builder against a real agy before it is
used:

| Claude tool | agy rule |
|---|---|
| `Bash`, `PowerShell` | `command(*)` |
| `Write`, `Edit`, `NotebookEdit` | `write_file(*)` (measured, probe 2) |
| `Read` | `read_file(*)` |
| `mcp__<s>__<t>` / `mcp__<s>__*` | `mcp(<s>/<t>)` / `mcp(<s>/*)` |
| anything else (`WebSearch`, `Task`, `Grep`, ...) | refused on agy, naming the tool, as `with_profile` refuses today |

`narrow` stops making every denying profile claude-only. Only a profile whose list has an unmapped name is. A
lesson review on agy gets `command(*)` and `write_file(*)`, the same as B3's Claude list.

### 3.5 Rules (research fix 3)

If the cwd, walking up to the project root, has no `AGENTS.md`, `GEMINI.md` or `.agents/rules/*.md`, and the cwd or
the root has a `CLAUDE.md`, herdr writes `<tools>/AGENTS.md` as one header line ("This is <path>, the project's
CLAUDE.md, given to an agy worker as rules by herdr (plan 35)") followed by the file's bytes. The order asked for a
quote in the order. I chose the file because the add-dir `AGENTS.md` loads (measured), agy gives rules their own
20,000-token budget (changelog line 369), and it keeps the order copy small for a 1,100-line CLAUDE.md like
MotoProject's. `@path` imports are not followed, and the header says so when the file has any. A copy over the budget
gets one line in the worker's view.

### 3.6 `finished_reason`, the comment, the premise (research fixes 4-6)

- `report()` fails an agy result whose `finished_reason` is a string (`print_timeout`, `interrupted` or `error`,
  changelog 1.4.11), naming it, before it looks at `structured_output`. A missing or null value keeps today's path.
  69 of 69 real results carried null.
- The comment at `nirvana_worker.rs:261-265` is rewritten. Auto mode is on Windows from 1.4.4, but it needs the
  terminal sandbox, which is off here (`enableTerminalSandbox=false`), and it adds a review per risky command.
  `--dangerously-skip-permissions` stays, and a `--settings` deny still holds under it (probe 2).
- `research.web`'s `why` (`nirvana_seats.toml:494`) becomes: "plan 35: agy searches with search_web and reads pages
  with bladebro when granted, else shell fetches; rescore after §6". The scores stay until the live checks (R2).

## 4. Owner rulings this plan needs

- **R1 (RULED yes, L5; blocks P35c until P35b merges): bladebro for a browser order on either CLI.** herdr's own definition, not the project's
  `.mcp.json`: `cmd /c bladebro mcp`, `BLADE_HOME=<run>/blade/<order>`, `BLADE_CHROME_FLAGS=--headless=new`,
  `BLADE_LANE=agent`, `BLADE_IDLE_TIMEOUT=60`, `BLADE_NO_UPDATE_CHECK=1`, all from decision 0043's measured recipe. A
  need `browser` or `bladebro` grants it, and `browser` stays a capability for the pick. Recommended: yes. It is
  decision 0043 brought to the fork, and claude workers have no browser today either (§1.1). It needs a new decision
  file because it widens 0043 to agy. CLAUDE.md rule 14: the spec is the owner's call.
- **R2: the seat table.** After P35c, add `browser` to the four agy seats. Rescore `research.web` from the §6 checks,
  not before. Also open: the claude seats carry no `web` capability although their workers have WebSearch and
  WebFetch. I have not changed it, because it moves routing.
- **R3 (RULED keep, L7): the owner's global agy plugins** (chrome-devtools, gemini-api with its MCP server, maps, modern-web). Every
  agy worker loads them, and nothing per order can switch them off. Keep them, or disable the unused ones globally.
  That is the owner's own action.
- **R4 (RULED yes, L6): non-desktop server orders may now seat on agy** (for example `bambu` printer control, `moto-reel`). The device
  and Send lines go with the brief on both CLIs. Recommended: yes. Otherwise P35b keeps `claude_only` for every server,
  and only bladebro and rules reach agy.

## 5. Packages

### P35a: the sources check sees agy's reads (Nirvana-Herdr, `src/nirvana_gate.rs` only)

Owns `src/nirvana_gate.rs` and `nirvana/controls.d/p35a_sources.py`. Parallel with P35b.

1. **Per segment, as today.** A command is still cut at newlines, `;`, `|` and `&&`. A segment's URLs count only
   when that segment runs a fetch:
   - Python `urlopen(`, `urllib.request`, `requests.get|post|head|Session`, `httpx.get|Client`;
   - PowerShell `Invoke-WebRequest`, `Invoke-RestMethod`, and `iwr`/`irm` as whole tokens (a substring `irm` is in
     "confirm");
   - `curl`, `wget`, `agent-browser`, `bladebro`, as today.
   A URL in a list on its own line or in an f-string (`watch?v={vid}`) still does not count.
2. **`gh api repos/<o>/<n>[/...]` and `gh repo view <o>/<n>`** count as reads of `https://github.com/<o>/<n>` and
   `https://api.github.com/repos/<o>/<n>`. This covers the ref-dance-apps send-back (8 of 8 cited).
3. **agy's MCP call** keeps today's check (`call_mcp_tool` whose strings name bladebro, browser, chrome or
   playwright). The probe-2 stream `2026-10-31-35-records/probes/p2.jsonl` becomes a test fixture, so a sources check over it passes
   `https://example.com`.
4. **Tests:** one per word family and per `gh` form, with a fixture cut from each of the 9 real sources send-backs
   (groove-coach streams, research §4); the negative cases `echo https://x; python -c "print(1)"`, a URL list
   line, `confirm https://x`; and the bladebro fixture.
5. **Measured before merge:** `sources_split.py` again, with the new rule as a Python twin of the Rust check. The
   expectation, written now: of the 81 URLs cited in the 9 send-backs, at least 70 count (today 0 count for agy's).
   The yt-video-pick f-strings and the 2 `file://` paths stay uncounted.
6. **Controls** (`p35a_` prefix, one per assertion): drop each new word, drop the `gh` mapping, match `irm` as a
   substring, read URLs per script instead of per segment. Each test named must print FAIL with its control in place.

### P35b: what an agy worker starts with (Nirvana-Herdr)

Owns a new `src/nirvana_agy_tools.rs`, its `mod` line in `src/main.rs`, `src/nirvana_gui_tools.rs`,
`src/nirvana_worker.rs`, `src/nirvana_profiles.rs`, and `nirvana/controls.d/p35b_agy_tools.py`. Parallel with P35a.
R4 decides item 2's reach.

1. The tools folder (§3.1): write, remove at the end, and an untouched argv when it is empty.
2. Servers (§3.2): the translation, the refusals (desktop, `${...}`, odd `type`), the workspace duplicate line, and
   F3 on agy.
3. The prompt (§3.3), the agy line for research and media orders ("Read every URL you cite in a command of its own,
   or with bladebro when you have it; a URL built inside a script is not seen as read."), and the `write_file(<tools>/)`
   deny.
4. Denials (§3.4) for profiles and lesson reviews. `with_profile` and `narrow` change as described there.
5. Rules (§3.5) and skills (the project's `.claude/skills/` copied). **The build measures first** whether an
   add-dir `.agents/skills/` loads. Expectation, written now: it loads (60%). If it does not, the skills become one
   line each in the order text (`name: description, path`), like lessons.
6. `finished_reason` and the comment (§3.6).
7. **Tests:**
   - each item through its public path (`order_argv`, `seat_order_with`, `exec` with a fake agy, as plan 29's
     `p29_exec_*` do);
   - an order with nothing gets a byte-identical argv;
   - a desktop server still skips agy;
   - the tools folder is gone after a worker ends, a failed one included.
8. **Real agy, at most 3 runs, expectations written first, in empty folders under the build's own temp:**
   - add-dir skills;
   - path-scoped `write_file(<dir>/)` and `command(*)` under `--dangerously-skip-permissions`;
   - one translated http entry or a `type: stdio` entry, accepted or refused.
   Each row of §3.4 that is not measured is refused on agy, never used unmeasured.
9. **Controls** (`p35b_` prefix): one per assertion. They include: claude_only kept for every server, the folder not
   removed, the `--add-dir` left out, `${...}` passed through, the deny mapping widened, `finished_reason` ignored.

### P35c: bladebro for browser orders, both CLIs (after P35b merges and R1)

Owns a new `src/nirvana_bladebro.rs`, `src/nirvana_lean.rs`, `src/nirvana_seats.toml` (the `why` line, and R2's
`browser` words if ruled), `nirvana/claude/skills/seat/SKILL.md` (one line: `--needs browser` gives the worker
bladebro on any seat), and, handed over from P35b after its merge, `src/nirvana_gui_tools.rs` and
`src/nirvana_worker.rs` for the call sites. Also `nirvana/controls.d/p35c_bladebro.py`.

1. The definition (R1). It goes into claude's per-order `--mcp-config` with `--allowedTools mcp__bladebro__*`, and
   into agy's tools folder. A `bladebro` not on PATH grants nothing, and the order says so in one line (0043 item 6).
2. The worker's environment carries the same `BLADE_*`, so a shell `bladebro` uses the order's home, never `~/.blade`
   (0043's fix round).
3. `BROWSER_LINE` goes to a worker that was granted bladebro, whatever the CLI, and to no one else. This replaces
   `BROWSER_CLI`.
4. The order's `BLADE_HOME` is removed when the worker ends: 3 tries a second apart, then one line. Memory: the
   checks once leaked 144 GB of temp profiles. Nothing is killed by name. On Windows the server owns its Chrome, and
   0043 measured it dying with the worker's process.
5. **Tests and controls** (`p35c_` prefix): definition fields, both argv shapes, not on PATH, the line on both CLIs
   and on neither without a grant, the home removed, and the env laid over the worker.

Each merge runs `nirvana/version.py bump feature` in the merge worktree (P35a, P35b, P35c: three feature bumps).

## 6. Live checks for the owner

Each is a small order through `herdr seat run`, in a scratch project with its own `.nirvana`, never MotoProject.

1. **agy calls bladebro inside herdr** (job object, pane: what probe 2 did not have). `--needs browser`, class
   `research.web`, seat flash low: "Open https://example.com with bladebro, report its title, and cite it." Expect:
   - `call_mcp_tool` naming bladebro in the stream;
   - the sources axis passes;
   - the tools folder and `BLADE_HOME` are gone afterwards, and 0 processes name that home;
   - no Chrome window on the desktop.
2. The same order on sonnet low: the claude worker lists `mcp__bladebro__*` and the gate passes. This is parity.
3. An agy order with a non-desktop `.mcp.json` server (R4): the worker lists the server's tools.
4. An agy order in a project with only `CLAUDE.md` (Nirvana has one): the worker quotes one of its rules.
5. An agy order with a profile denying `Write`: the denial text "Matches user-configured deny rule" appears, and the
   order is not refused at seating.
6. A `--needs bambu-studio-beta` order still skips every agy seat.
7. The research's sources rerun on the next 5 agy research orders: send-backs on the sources axis go from 8 in 12 to
   at most 1. That number is a prior, rescored here (R2).

## 7. Risks

1. **A server token in a working-tree file.**
   - Where it sits: the agy copy of a `.mcp.json` definition sits in `<tools>/.agents/mcp_config.json`, under the
     ignored `runs/`, as plan 29's claude file does. It is never in the worktree or the project's `.agents/`.
   - What is new: the folder is an `--add-dir` workspace folder, so the agy model can read it without asking. A claude
     worker can already read its own config with Read, and the project's `.mcp.json` is readable to both, so this is
     no new reach. Writes there are denied (§3.3).
   - ACL: the file takes its folder's ACL on Windows, as plan 29 F2's file does.
   - Removal: it goes when the worker ends. A crash between write and removal leaves it until the run's cleanup.
     P35b adds the tools folder to `herdr seat cleanup`.
2. **The owner's global agy plugins and hooks load in every worker** (R3).
   - The gemini-api server's tools are offered to every order.
   - herdr's own PreInvocation hook runs per invocation (290-421 ms outside a pane, research §2; whether it runs per
     model call or per turn is unknown).
   - No per-order switch exists, so a Preload "off" cannot reach agy. The panel should say so for agy panes rather
     than show switches that do nothing. That note is for a later plan, not this one.
3. **Cost.**
   - Some agy orders already carry cache reads of 14-38M (xtool-status-fix 38.1M, dial-shopping-2 14.2M).
   - Each MCP call first reads the tool's schema file with `read_file` (2 reads in probe 2), and page reads add more.
     Probe 2 spent 38,471 tokens in against 9,917 for an empty start.
   - Mitigation: `BROWSER_LINE` (batch with `run`, read the result) goes to agy too. Live check 1 records the order's
     tokens. If a browser order's cache reads pass 5M, `research.web`'s agy scores drop under R2.
4. **The model writes into the added folder** (probe 2). Mitigated by §3.3. The build measures it.
5. **Name collisions in agy's schema cache** (`~/.gemini/antigravity-cli/mcp/<server>/`). Two concurrent orders with
   a server of the same name but different definitions (bladebro with two homes) share one cache folder. The schemas
   are the same, so this is a hypothesis with no harm seen. It is unmeasured with two live orders.
6. **exebox.** bladebro ran headless in probe 2, but where its server ran is not measured. If live check 1 fails
   inside herdr's job, the fallback is the order's shell `bladebro --no-daemon` with the same env (P35c item 2), and
   that is measured then.
