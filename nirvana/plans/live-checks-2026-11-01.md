# Live checks for plans 35 and 37, 2026-11-01

Master 5fd98b52 (alpha 0.51.1), built in this worktree (`cargo build --locked`, debug, so herdr's config folder is
`%APPDATA%\herdr-dev`, never the owner's `%APPDATA%\herdr`). Every order ran through that binary, copied to the
scratch `env\bin\herdr.exe`, against a private herdr server (`HERDR_SESSION=livecheck`), in a scratch project
(`...\scratchpad\next\live\proj`, a git repo with its own `.nirvana`, a `CLAUDE.md` with rules LC-7 and LC-9, a
`.mcp.json`). `NIRVANA_HERDR_HOME` was a scratch folder (`env\home`, holding copies of the owner's `seats.toml` and
`usage.json`), so leases, pane indexes and the seat table's writes never reached the owner's install home.
`BROWSER` was `C:\Windows\System32\where.exe` for every CLI.

## Step 0, isolation

Expected, before any order: the private socket is `%APPDATA%\herdr-dev\sessions\livecheck\herdr.sock`; a `pane list`
from the scratch shell answers from it (no panes) while the owner's server keeps its own; the owner's session
folders, sockets and herdr processes are the same before and after; nothing under `~/.nirvana` is written.

Seen:
- `src/session.rs`: `--session` beats `HERDR_SOCKET_PATH`, which beats `HERDR_SESSION`; the socket is
  `config_dir()/sessions/<name>/herdr.sock`, and a debug build's `config_dir()` is `%APPDATA%\herdr-dev`.
- The server was started with herdr's own command (`herdr server`, `HERDR_SESSION=livecheck`, out of the tool's
  job), pid 32456. `herdr session list --json` named `livecheck` running at
  `%APPDATA%\herdr-dev\sessions\livecheck\herdr.sock`; `workspace list` and `pane list` answered empty (the owner's
  server has panes), so the calls reached the private socket.
- A pane of that server got `HERDR_SOCKET_PATH` = the private socket, `HERDR_PANE_ID=w1:p1`,
  `NIRVANA_HERDR_HOME` = the scratch home, `BROWSER` = `where.exe`.
- A pane's shell resolves `herdr` to upstream herdr 0.9.5 (`~\.herdr\packages\...`), whatever PATH the server had.
  `pane_env` passes `PATH=<current exe dir>;...` with `--env`, and a test pane split with that `--env` resolved the
  scratch build. So every `seat exec` ran the scratch build.
- `~/.nirvana`: the fork has no path to it (the old Nirvana's folder). Its own state is `NIRVANA_HERDR_HOME`
  (`%LOCALAPPDATA%\Programs\Nirvana-Herdr`: `nirvana/leases.json`, `usage.json`, `panes/`, `seats.toml`), which the
  scratch home replaced. Before and after the whole session: `~/.nirvana`'s newest file and `events.sqlite` mtime the
  same; no file in the owner's install home names the scratch path, `livecheck` or an order id.
- The owner's session folders and sockets (`nirvana\herdr.sock`, `herdr-client.sock`) the same before and after;
  the owner's herdr pids 54184, 67372, 67496 the same. One new owner process appeared at 17:25:38Z: the installed
  herdr's `seat exec editor-render-viewer --run run-20261023-024041 --pane wN:p1T`, child of a pwsh under the owner's
  server 54184: the owner's own MotoProject worker, not touched.
- `HERDR_PANE_ID`, `HERDR_WORKSPACE_ID` and `HERDR_SOCKET_PATH` were absent from the shell at the start, and set to
  the private pane `w1:p1`, workspace `w1` and the private socket for the orders.

Verdict: isolation held.

## Expectations, written before the runs

| # | Check | Expected |
|---|---|---|
| 35.1 | agy flash low, `research.web`, bladebro, "Open https://example.com with bladebro, report its title, and cite it." | `call_mcp_tool` naming bladebro in the stream; sources axis passes; the tools folder and `BLADE_HOME` gone afterwards; 0 processes name that home during or after (scoped to the path); no `.warmed` file in the home; no Chrome window (Chrome processes naming the home all carry `--headless`). Prior 75% all hold. Seen before the run: `--needs browser` picks sonnet, since no agy seat lists `browser` (R2 not applied), so the agy order uses `--needs bladebro`, which the seat skill documents as not limiting the pick. |
| 35.2 | The same order on sonnet low, `--needs browser` | the claude worker's init lists `mcp__bladebro__*` tools; the gate passes. Prior 80%. |
| 35.3 | agy flash low, `--needs livetools` (a stdio server in the scratch `.mcp.json`, one tool `lc_ping`) | the worker lists `livetools` and its tool; calling it returns `LIVETOOLS-3381`. Prior 70%. |
| 35.4 | agy flash low, a project with only `CLAUDE.md` | the worker quotes rule LC-7 or LC-9 (from `<tools>/AGENTS.md`, plan 35 §3.5). Prior 75%. |
| 35.5 | agy flash low, `--profile nowrite` (`disallowed_tools: [Write]`), asked to write a file | not refused at seating (seen: `seat pick --profile nowrite` gives flash); the write is denied with "Matches user-configured deny rule". Prior 65%. |
| 35.6 | `--needs bambu-studio-beta` | every agy seat skipped: the pick goes to a claude seat, and an override to each agy seat is refused naming the desktop server. Prior 90%. No worker runs. |
| 37.1 | agy flash low, research order that must cite one real YouTube video | gated on delivery by herdr itself (a gate entry in the journal with no orchestrator turn); the cited video id checked by oEmbed (200, so no fail). Prior 60% (the model may cite no id or an invented one; either is a finding). |
| 37.2 | `herdr gate` on a scratch copy of MotoProject's `reel-motion-gemini` stream | sources fail, naming the 404 ids; the redo names the next stronger seat of the class. Prior 70%. |

## Results

Each order: `herdr seat run <seat> --order .nirvana/orders/<id>.md --order-id <id> --run <run> --class <c> --size small
--effort low [...]` from the scratch project, through `drive.py`, which also ran a process poller scoped to the order's
`BLADE_HOME` (a WQL `LIKE` on that path, every ~0.4 s; control: a python process naming a scratch path was seen at
0.3 s, and a folder holding `.warmed` was reported at 0.2 s). Outputs below are filtered from the run's
`journal.jsonl` and `streams/<id>.jsonl`; the filter is named on each line.

| # | Expected | Seen | Verdict |
|---|---|---|---|
| 0 | private socket, owner unchanged | as expected (Step 0 above) | pass |
| 35.1 | bladebro called; sources pass; folder and home gone; 0 processes after; no `.warmed`; no window | `call_mcp_tool {"ServerName":"bladebro","ToolName":"run","Arguments":{"steps":[{"action":"see","url":"https://example.com"}]}}`, 1.15 s, title "Example Domain"; gate `sources pass: 1 source(s) cited, each verified in stream`; `runs/lc35/agy/` and `blade/` empty after; 13 chrome.exe named the home during, every one with window handle 0; 0 processes named it after; no `.warmed` in 141 polls while the home existed | pass |
| 35.2 | claude lists `mcp__bladebro__*`; gate passes | init `mcp_servers: [bladebro connected]`, tools `mcp__bladebro__act/run/see/state/vision`; argv `--mcp-config ... --allowedTools mcp__bladebro__*`; called `mcp__bladebro__run`; gate pass; 12 chrome.exe, all window 0, 0 after | pass |
| 35.3 | agy lists the `.mcp.json` server and calls it | read `~/.gemini/antigravity-cli/mcp/livetools/lc_ping.json` (agy's own cache), then `call_mcp_tool livetools lc_ping {"word":"hi"}` -> `pong hi LIVETOOLS-3381` | pass (tools); see finding F2 for the gate |
| 35.4 | agy quotes a CLAUDE.md rule | lc4 (research.read): summary said it quoted, but held no quote; sent back on sources. lc4b (chore.script, the one rerun): only tool call was reading its order; summary quoted both rules verbatim and named `...\proj\CLAUDE.md`; the prompt (`--print=`) and the order file hold no `PELICAN`/`LC-7` | pass (lc4b) |
| 35.5 | not refused at seating; denial text | seated on flash with `--profile nowrite`, argv `--settings ...\agy\lc5-agy-deny.settings.json`; `write_file` step ERROR; report: "Permission denied for write_file(...note.txt). Matches user-configured deny rule."; no `note.txt` | pass |
| 35.6 | every agy seat skipped | `seat pick --needs bambu-studio-beta` -> sonnet; `--override flash|argon|g-sonnet|g-opus` each exit 3, "override <seat> refused: bambu-studio-beta drives a desktop window and agy panes cannot see it (plan 29 §5.2)"; control without the need: `--override flash` exit 0, flash | pass |
| 37.1 | gated on delivery, no orchestrator turn; video id checked | journal seq 10 delivered, seq 12 `gated pass` with `sources: ... 1 cited video(s) exist on YouTube`, seq 14 `wake_queued` (the orchestrator pane runs no Nirvana Claude): the gate ran before any wake. Cited `watch?v=nx9Fos8ZQbc`, read with its own `curl`; my oEmbed check: 200 | pass |
| 37.2 | sources fail naming the 404 ids; redo on the next seat | replay 2 (`lc37replay2`): `sent_back`; sources `fail: 12 source(s) cited that were not fetched, read or browsed; 6 cited video(s) do not exist on YouTube`, lines `no such video on YouTube (oEmbed 404): 3g6_aVqSg_A, kYv9yY8A7bI, Tq_YlH6u7o4, Vl0PjC7k3f8, f9F1aZ2G2m8, Jm3Xf9-uJyo`; child `reel-motion-gemini-s1 seated on argon low`, override "step up from flash to argon: 6 cited video(s) do not exist (plan 37 §1.3)". Stopped at once with `herdr seat stop` (11,227 tokens in) | pass |

### Findings

- **F1. `--needs browser` cannot seat an agy order.** No agy seat lists `browser` (R2 is not applied), so
  `seat pick --class research.web --needs browser` gives sonnet. Check 35.1 as written (`--needs browser`, seat
  flash) is refused by the rule; it ran with `--needs bladebro`, which the seat skill documents.
- **F2. The sources axis sends back research orders that need no web, and the redo games it.** `research.*` and
  `media.*` orders always get the sources axis. lc3 (an MCP call) and lc4 (rules from memory) delivered done and were
  sent back "no sources: the Work Report names no sources". The redos passed by citing a page unrelated to the
  order: lc3-s1 cited `https://modelcontextprotocol.io/llms.txt`; lc4-s1 ran `curl https://example.com` "as required
  by the order's sources specification" and read other orders' streams and gate files first (139k tokens in).
- **F3. A send-back's re-pick drops the caller's effort.** lc4 ran at `--effort low`; its redo lc4-s1 was seated at
  flash medium.
- **F4. The step-up from flash in `research.web` lands on argon at the same effort and the same score** (both 84 in
  the class), so "one seat stronger" here is a different model, not a higher-scored one.
- **F5. agy's tool list has a URL reader and browser tools now.** The init line of lc1 lists 59 tools, among them
  `read_url_content`, `open_browser_url`, `read_browser_page`, `browser_*` and `write_to_file`. Plan 35 §1.2 says
  "14 built-ins in print mode, with `search_web` but no URL reader and no browser". The workers here did not use them.
- **F6. A blocked delivery is not gated and queues no wake event.** lc5 (status `blocked`) ended `delivered` with no
  `gated` and no `wake_queued` line in the journal. By the code this is the design (`gates_on_delivery` gates a done
  delivery); whether the orchestrator still hears of it was not checked.
- **F7. agy leaves a server's schema cache in the owner's `~/.gemini`.** After 35.3 the folder name `livetools`
  exists under `~/.gemini/antigravity-cli/mcp/` (seen as the worker's `dir` output and one name listing; nothing
  under `~/.gemini` was read). agy writes it; herdr's cleanup cannot reach it. Not removed.
- **F8. Every delivery also seats a lesson review** on flash low (10 of the 23 rows), about 20k-50k tokens each.

### Spend

Fresh tokens (input + output, from `herdr seat status --json`, the replayed MotoProject rows left out): flash about
0.95 M over 19 orders, argon 11,337 (stopped redo), sonnet 577 plus cache. Reruns: 35.1 once (to measure processes
with the fast poller; the first run's 2-s poll saw nothing in a 12-s run), 35.4 once (class and wording), 37.2 once
(the first replay ran with `agy` hidden from PATH so the redo could not start; that made argon "agy not installed"
and the gate asked instead of seating, and an untracked `.mcp.json` failed `owned`. The second replay committed
`.mcp.json`, kept agy on PATH and stopped the child at once). The auto redos lc3-s1 and lc4-s1 were herdr's.

### End

`herdr session stop livecheck` ("stopped session livecheck"); pid 32456 gone, 0 children of it alive; then
`herdr session delete livecheck`. Processes naming `scratchpad\next\live` afterwards: 0; naming the livecheck socket:
0. Pids started here: 32456 (server). The two workers stopped (`reel-motion-gemini-s1`, `-lr`) were this session's.
