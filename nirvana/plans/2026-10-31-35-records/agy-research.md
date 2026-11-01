## The answer

**The gap is real, but most of it is not agy being weaker.** Three things cause it:

1. **Headless agy has no browser and no tool that reads a URL, and herdr's gate cannot see how agy reads pages instead.**
   - In print mode the model gets 14 built-in tools. It has no `browser_*`, no `read_url_content` and no `open_browser_url`, and two probes on different models confirmed this.
   - The 60-name list in the stream's `init` event is a catalogue, not what the model gets. Across all 78 real herdr agy streams the model only ever used 7 tool names.
   - So agy reads pages with `python -c "urllib..."` (370 commands) and `gh api` / `gh repo view`. The gate's sources check only counts `curl`, `wget`, `iwr`, `agent-browser` and `bladebro`, so it marks those reads as "not fetched".
   - **8 of agy's 12 send-backs come from this check.** In the 9 sources send-backs, 74 of 81 cited URLs (all agy's but 3) appear in a command the worker actually ran.
   - If you leave out final verdicts that rest on this check, agy's pass rate goes from 67% to 84%. Claude's is 80–81%.
2. **herdr gives an agy worker no project MCP servers and no project rules when the project has only `CLAUDE.md`.**
   - agy reads `AGENTS.md` and `GEMINI.md` but ignores `CLAUDE.md` (probe 2).
   - Headless agy does load a workspace `.agents/mcp_config.json` (probe 2), but herdr never writes one. Per-order servers go to claude workers only.
3. **agy's shell runs on an isolated Windows desktop (`exebox-*`), so it cannot see or drive desktop apps.** This one is agy's design, not herdr's. Routing the script through `herdr job run` works around it: two such GUI benchmarks were accepted.

**agy does not need an update.** 1.4.12 is installed and 1.4.12 is the latest release (GitHub, 30 Oct 2026).

## 1. How herdr launches agy today

Code read on Nirvana-Herdr master `8096cea9`, without changing anything.

- **argv** (`src/nirvana_worker.rs:248-271`) is a list, never a shell line:
  `agy --output-format stream-json --model <id-with-effort> --print-timeout <lease_minutes*60>s --dangerously-skip-permissions --json-schema <REPORT_SCHEMA> [--conversation <id>] --print=<one line>`
  - The effort is part of the model id (`gemini-3.8-flash-low`, `nirvana_seats.toml:113`). agy also has a separate `--effort` flag, but `agy models` lists the effort-suffixed ids, so this works.
  - The timeout comes from `print_timeout_s` (`:1453`).
- **Prompt** (`first_line`, `:425`): "Do the work order in <abs path>. Work only in this folder. End with the Work Report…"
  - The order is a file in `<project>/.nirvana/orders/`, with `ORDER_FOOTER` appended (`:440`): no subagents or background processes unless the order allows it.
  - A profile body is quoted at the top of the order for agy (`with_profile`, `:368-407`).
- **What only claude workers get:**
  - `--setting-sources project --strict-mcp-config`.
  - The per-order `--mcp-config` from `.mcp.json`. `nirvana_gui_tools.rs:356` returns early when `argv.first() != Some("claude")`.
  - The preset `--settings` (`:283`), and `--append-system-prompt-file` / `--disallowedTools` for profiles. A profile that denies tools fails an agy order outright (`:397-402`).
  - Review limits (`:412`).
- **Env, cwd and stdin** (`run_attempt`, `:1217-1260`):
  - cwd is the seated folder: the project root, or `.nirvana/worktrees/<order>`.
  - stdin is null; stdout and stderr are piped.
  - `ENV_REMOVED` strips `CLAUDECODE`, the `CLAUDE_CODE_*` variables and `CODEX_THREAD_ID`.
  - The process starts with `CREATE_NO_WINDOW|CREATE_SUSPENDED` inside a job object.
  - agy gets no settings isolation. It loads the owner's whole user config: 4 plugins, the gemini-api plugin's MCP server, the global hooks and the user permissions.
- **Output and end of run:**
  - The stream is copied to `streams/<order>.jsonl` and read by `feed_agy` (`:707`).
  - `report()` (`:774-817`) fails when `status != "SUCCESS"`, then takes `structured_output` or parses `response`.
  - Exit code 3 with an `AGY_ERROR` line naming a quota counts as a rate limit (`:822`).
  - **`finished_reason` (added in agy 1.4.11) is never read.**
- **The orchestrator's side:** `nirvana/claude/ROLE.md:32` and the seat skill §3 say every agy order is a pane worker started with `herdr seat run`. The seat table's premise for web research is `why = "plan 19 §4: agy searches and reads URLs"` (`nirvana_seats.toml:494`), and it scores flash and argon at 84, above Sonnet at 80.

## 2. How agy is meant to run, and what native sessions get that herdr's don't

**CLI surface:**
- `agy --version` prints `1.4.12`.
- `--help` lists, among others: `--add-dir --agent --effort --mode (accept-edits|auto|plan) --settings --sandbox --input-format --json-schema --print-timeout --project`.
- Subcommands: agent(s), changelog, install, mcp, mic-serve, models, plugin(s), remote-control, update.
- `agy mcp list` prints "No MCP servers configured." (the user `mcp_config.json` is 0 bytes).
- Installed plugins: chrome-devtools-plugin (6 skills), gemini-api (3 skills, 1 MCP server), google_maps_platform, modern-web-guidance-plugin.

**Config files (key names only):**
- `~/.gemini/antigravity-cli/settings.json`: `colorScheme`, `permissions.allow` (16 entries), `trustedWorkspaces` (27). `C:\Users\fahim` is trusted, so folder trust is not the gap.
- `~/.gemini/config/config.json` `userSettings`: `artifactReviewMode`, `autoExecutionPolicy`, `browserJsExecutionPolicy`, `enableTerminalSandbox=false`, `nonWorkspaceFileAccessPolicy`, `remoteControlHostname`.
- `~/.gemini/config/hooks.json` has one hook, `herdr.PreInvocation`, which runs `herdr-agent-state.ps1 session`. Outside a pane it exits at once in 290–421 ms. Inside a worker pane it calls `herdr pane report-agent-session`.
- `~/.gemini/GEMINI.md` is empty.

**From the agy docs** (antigravity.google/docs/rules, /docs/mcp?app=cli, /docs/cli/reference):
- Rules come from `AGENTS.md`, `GEMINI.md` and `.agents/rules/*.md`, walking up to the workspace root, plus the global `~/.gemini/{AGENTS,GEMINI}.md`. `CLAUDE.md` is not read.
- MCP servers come from the global `~/.gemini/config/mcp_config.json` and from `.agents/mcp_config.json` in the workspace.
- Workspace skills come from `.agents/skills/`.
- `--mode auto` exists on Windows from 1.4.4. It needs Windows 11 24H2, a personal account and the terminal sandbox.

**Probes.** I ran 3, within the cap. Each used herdr's argv minus `--json-schema`, a new folder under the scratchpad, `BROWSER` set to a no-op `.cmd`, the `CLAUDE_CODE_*` variables removed, and a 180 s wall. I wrote the expectations down before each run. After probe 1, I folded the rules and MCP tests into one folder to stay within the cap.

| Probe | Folder | What I expected | What happened |
|---|---|---|---|
| 1, flash-low | empty | the 60 init tools; gemini-api MCP only; no rules | init listed 60 names, but **the model declared 14 built-ins**: run_command, read_file, edit_file, write_file, manage_task, schedule, invoke/define/manage_subagent(s), send_message, ask_question, search_web, list/read_resource. MCP: `gemini-api_gemini-api-docs` only. cwd correct, no rules. Exit 0, 6 s, 9,917 tokens in. |
| 2, flash-low | AGENTS.md (MARKER-AGENTS-7731), CLAUDE.md (MARKER-CLAUDE-4410), `.agents/mcp_config.json` with a stdio `probe` server | AGENTS.md loaded, CLAUDE.md not, probe server listed (I was 60% sure of the last) | **AGENTS.md loaded with its marker; CLAUDE.md not loaded; the `probe` server was listed with `probe_echo`.** Exit 0, 6 s. |
| 3, claude-sonnet-5-5-low through agy | empty | the same 14, no browser | The same 14 built-ins plus the 2 gemini-api tools. Its answer: "**Browser or URL-reading tool? No.**" Exit 0, 6 s, 15,459 tokens in. |

**What a native session has that a herdr one doesn't:**
- Measured: project rules when the project has only CLAUDE.md, and workspace MCP servers.
- Measured: the chance to ask the owner a question. Headless runs settle questions themselves (changelog 1.1.12) and skip subagent questions (1.3.6).
- Not measured: whether an interactive session declares `browser_*` and `read_url_content`. The CLI reference lists `/browser`, and the owner's `browserJsExecutionPolicy` is set to TURBO. If interactive sessions do have the browser, that is the largest capability native has and herdr lacks.

## 3. Does agy need an update?

**No.**
- Installed: `1.4.12`.
- Published: 1.4.12 on GitHub releases (30 Oct, 21:39), and mise-versions also lists 1.4.12 as the latest.
- The CLI log shows agy updates itself (`auto_updater.go: Last check was less than 15 minutes ago`).
- If it ever falls behind, the owner runs `agy update` or `winget upgrade Google.AntigravityCLI` (it is a winget install).

**What changed since herdr's comment** (`nirvana_worker.rs:259-261`, "read on 1.4.2"):
- 1.4.4: auto mode on Windows, so that comment is now out of date.
- 1.4.6: MCP results of up to 10 KB come back inline.
- 1.4.7: headless runs use much less CPU.
- 1.4.11: a `finished_reason` field in the result (`print_timeout`, `interrupted`, `error`), and PreInvocation hooks can stop a turn.

## 4. How agy orders actually went

I wrote the scripts in the scratchpad (`count_runs.py`, `streams.py`, `sources_split.py`) and ran them read-only on the run records in 4 projects: 3D-Printer Flow, its two xtool siblings, and groove-coach. Nirvana-Herdr has no `.nirvana` folder.

**Order counts and verdicts** (`python -I count_runs.py <Pojects>`, summary lines verbatim):
```
orders by cli: {'claude': 121, 'agy': 79}
  final state, claude: {'accepted': 43, 'sent_back': 11, 'abandoned': 5, 'ended:delivered (ungated)': 36, 'ended:stopped (ungated)': 4, 'pass': 22}
  final state, agy: {'accepted': 11, 'sent_back': 11, 'abandoned': 2, 'ended:stopped (ungated)': 9, 'pass': 30, 'ended:delivered (ungated)': 15, 'no end': 1}
gate verdict events: claude {'accepted': 43, 'sent_back': 14, 'abandoned': 5, 'pass': 22} | agy {'accepted': 11, 'sent_back': 12, 'abandoned': 2, 'pass': 30}
sent_back reasons: claude {'orchestrator judgment': 13, 'sources axis': 1} | agy {'orchestrator judgment': 2, 'sources axis': 8, 'review order': 2}
classes: claude {'ops': 51, 'build': 52, 'review': 9, 'research': 6, 'plan': 2, 'fix': 1}
         agy {'research': 32, 'review': 7, 'ops': 2, 'build': 17, 'chore': 1, 'media': 4, 'fix': 1, 'lesson-review': 15}
claude: 65/81 = 80%; without sources-axis finals 65/80 = 81%
agy: 26/39 = 67%; without sources-axis finals 26/31 = 84%
```
The last two lines exclude lesson-review orders and count accepted or pass over gated orders.

**Tool use across agy streams** (`python -I streams.py <Pojects>`, summary lines):
```
init (permission_mode, tool count, has mcp_servers key): {('always-proceed', 60, False): 78}
tool steps finished: {'run_command': 2099, 'read_file': 725, 'search_web': 621, 'manage_task': 268, 'write_file': 229, 'schedule': 27, 'edit_file': 26}
run_command kinds: {'no-url': 1566, 'python-http': 370, 'curl': 86, 'other-http': 64, 'herdr job': 12, 'iwr': 1}
result events: 69 SUCCESS with finished_reason None; 9 with no result (the stopped orders)
```

**Every sources-check send-back** (`python -I sources_split.py <Pojects>`):
- body-extrapolation: 20 cited, 20 appear in a command.
- asl-inspiration: 12 cited, 10 in a command, 2 not.
- asl-report: 14 cited, 13 in a command, 1 not.
- ref-authoring: 10 cited, 9 in a command, 1 not.
- ref-authoring-s1: 10 cited, 10 in a command.
- ref-dance-apps: 8 cited, 0 in a command. They were read with `gh repo view` / `gh api repos/<owner>/<name>`, so no URL appears in the command.
- yt-video-pick: 5 cited, 2 in a command. The other 3 were built with f-strings (`watch?v={vid}`).
- video-breakdown: 2 cited, both local `file://` paths.
- The only claude one, repo-breakdown: 6 cited, 3 in a command; its sources were local, non-URL.

**agy failures, by kind:**
- **The gate cannot see how agy read the page: 8 send-backs**, all in groove-coach after plan 24 added the sources check. agy's own lesson says so: `groove-coach/.nirvana/lessons/web-source-accounting.md`.
- **8 re-seated orders stopped 5–16 s after starting** by `herdr seat stop`, each having used 10–60k tokens in and under 420 out. The record does not say why the orchestrator stopped them.
- **No desktop windows (exebox):** gui-bench-agy and crawl-beta-gui-2 (g-sonnet) were blocked; the two `herdr job run` bridge runs were accepted.
- **Real quality send-backs: 4.** Two were orchestrator judgment (sanding-block-stl, jev-images-research) and two came from review orders (m1d-backend, m2a-mediapipe-extractor). Claude had 13 orchestrator-judgment send-backs.
- **Not seen at all:** no tools reached, wrong cwd (every init cwd matches the seated one), quota exits, stalls, truncated output, or a broken Work Report.
- **Cost note:** agy orders carry very large cache reads, for example 38.1M (xtool-status-fix) and 14.2M (dial-shopping-2). A fresh headless start costs 9.9–15.5k input tokens.

## 5. Better ways, ranked

1. **Make the sources check see agy's reads** (findings 1 and 4).
   - In `nirvana_gate.rs:589-596` and `:695-706`, also count `run_command` segments that run Python with `urllib` or `requests`, `gh api` and `gh repo view` (mapping `owner/name` to its github.com URL), and PowerShell loops over a URL list.
   - A stricter alternative is an agy-only footer line for research orders: "read each URL you cite with `curl -sL <url>`, one per command".
   - Cost: small.
   - Risk: a script that names a URL without fetching it would pass, so keep the per-segment rule.
   - Upside: 8 of 12 agy send-backs, a pass rate of 67% → 84%, and the 8 re-seats.
2. **Give agy a browser through a per-order `.agents/mcp_config.json`** in its worktree, naming bladebro, which claude workers already get (probe 2 shows headless agy loads that file).
   - The gate already counts a `call_mcp_tool` call that names bladebro (`nirvana_gate.rs:666-679`).
   - The same file can carry the order's other, non-desktop `needs` servers.
   - Cost: medium: write the file, remove it when the worker ends (as `remove_order_config` does), add a gitignore line, and add the `browser` capability to the agy seats.
   - Risk: an order without `--worktree` would write into the project's own `.agents/`, so only do this for worktree orders. A server token in `env` would sit in a working-tree file. Keep the desktop servers off agy because of exebox.
3. **Rules parity.** When a project has no `AGENTS.md` or `GEMINI.md`, quote its `CLAUDE.md` at the top of agy orders, the way the profile body is quoted.
   - Cost: small.
   - Measured impact: none yet, because none of the 4 run projects has any rules file. This matters for projects like Nirvana and Robot Rouglike, which have only CLAUDE.md.
4. **Read `finished_reason`** in `report()` (`nirvana_worker.rs:779-784`), so a print-timeout partial is reported as one instead of as "no structured output". Cost: trivial. No incident yet.
5. **Correct the seat table's premise** at `nirvana_seats.toml:494`. Seated agy searches, but reads URLs only through shell scripts. Re-score web research after items 1 and 2 have run, not before.
6. **Keep `--dangerously-skip-permissions`** and fix the stale comment at `nirvana_worker.rs:259-261`. Auto mode needs the terminal sandbox, which is off on this machine, and adds a review pass per risky command. Low value.
7. **Interactive pane vs headless: do one owner-side check first.** Open Antigravity interactively in a pane and ask the probe-3 question. If it lists `browser_*` and `read_url_content`, the browser gap is headless-only, and item 2 is the only way to close it for seated work.
8. **Optional:** deny tools to an agy profile with `--settings '{"permissions":{"deny":[...]}}'` instead of failing the order at `:397-402`. I have not tested that syntax.

## Not checked, or unsure

- **Files in the scratchpad:** the harness refused my `report.md` write, so this message is the report. The scratchpad holds `EXPECT.md`, the three scripts, `count_out.txt`, `streams_out.txt`, `changelog.txt`, and the probe streams `probe1.jsonl`–`probe3.jsonl` with their `.err` files.
- **What I changed:** nothing outside the scratchpad. I signed nothing in or out, read no credentials (key names only), and sent nothing to herdr; no `HERDR_*` variables were set in my shell, so the hook exited on its no-op path.
- I did not see the interactive tool set: there is no terminal here and the cap was 3 probes. I also did not measure how good native sessions are; nothing in the run records captures that.
- Byte-searching agy's conversation databases did not separate tool declarations from tool calls, so I did not rely on it.
- I don't know whether the PreInvocation hook fires per model call (~0.3 s each, plus a herdr call inside a pane) or per turn.
- The pass rates mix kinds of work: agy's orders are mostly research, claude's mostly ops and builds.
- I did not read D:\MotoProject. Nirvana and Robot Rouglike have `.nirvana` folders but no runs.
- The probes ran outside herdr: no job object, no pane, no `--json-schema`. Their init cwd, tool count and permission mode match the real streams, so I take the tool set to be the same.