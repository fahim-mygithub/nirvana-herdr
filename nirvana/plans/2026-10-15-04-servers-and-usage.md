# Feature 4: running servers and usage at the foot of the sidebar

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Lets work on the next feature. This running server and usage from Nirvana lets port it to the Nirvana herder [Image #17]" | the foot of the expanded sidebar, under the agents panel: a "servers" block listing each server running in a space, then a "usage" block with Claude, Codex, Antigravity and Local | orchestrator | unit tests on the readers, the listener filter, the layout and the rows; controls that redden each; a live run with a server started in a space and the four usage rows read from this machine, screenshot read by a leaf agent |

Image #17, as the owner pasted it: the old Nirvana rail's foot. "Running servers", one row: a green dot, "MotoProject",
"localhost:8770". Then "Usage", with "this machine" and a "local off" switch at the right of its head, and four rows,
each a coloured dot, a name, a thin bar, and figures: "Claude 5h 0% 7D 68%", "Codex installed — not exposed" (the
picture cuts the first letters), "Antigravity 5h 0% 7D 0%", "Local off".

## What old Nirvana does (read 2026-10-15 by "Sonnet - map Nirvana usage and servers")

- **Running servers** are a registry of servers Nirvana itself started (`nirvanad/servers/supervisor.py`), shown as the
  folder name and `localhost:<port>`, green while it answers. Not a port scan.
- **Claude** 5h/7D come from the `rate_limit_event` line of `claude -p ... --output-format stream-json`
  (`unifiedWindows.five_hour.utilization`, a fraction), plus one paid startup turn when the reading is stale.
- **Codex** reads `rate_limits.primary/secondary.used_percent` from its rollout's `token_count` events, only for Codex
  runs Nirvana started; otherwise "installed — not exposed".
- **Antigravity** runs `agy -p /usage --output-format stream-json` every 300 s and reads the `gemini-` buckets'
  `remaining_fraction`.
- **Local** is Nirvana's local seat: a switch (off by default) and, when on, Ollama's state and the GPU's memory.

## Shape

herdr has no daemon that starts servers or runs seats, so each row is read from this machine instead.

- **Servers.** Every listening TCP port owned by a process inside a space's panes (the pane's shell and everything under
  it, so a server Claude starts with a background Bash counts). One row per port: green dot, the space's name, then
  `localhost:<port>`, or `:<port>` when the sidebar is too narrow. A click opens `http://localhost:<port>/` in the
  browser. The block hides when no space runs a server, as Nirvana's does. Not counted:
  - ports in Windows' dynamic range, 49152 and up: a program that asks for any free port gets one there, and the live
    check on 2026-10-15 found an idle Claude Code on 64839 and agy's app on 60015/60016;
  - ports owned by an agent program itself (claude, codex, agy, Antigravity), which are its own plumbing.
  Read by the server (it knows each pane's processes), on a worker thread every 3 s; the ports ride in the space's
  existing `tokens` as `nirvana_servers`, so the wire format is unchanged.
- **Usage**, "this machine", read by the window itself (a subscription window belongs to the machine, not a space), on a
  worker thread:
  - **Claude:** `claude -p /usage --no-session-persistence --output-format json`. Claude Code answers this locally: the
    live check returned `model: "<synthetic>"`, 0 tokens, `total_cost_usd: 0`, in 2.7 s. "Current session: N% used" is
    5h, "Current week (all models): N% used" is 7D. Every 120 s while a Claude in the window is working, else every
    600 s, and once at start. Nirvana's own reader paid a model turn for this; this one does not. `--bare` was tried
    and drops the subscription sign-in, so it is not used.
  - **Codex:** the newest `rate_limits` in the newest rollouts under `~/.codex/sessions` (`used_percent`, with
    `window_minutes` 300 as 5h and 10080 as 7D). A reset already past reads 0%. No rollout figure: "not exposed", as
    Nirvana; Codex not on PATH: "not installed". Every 30 s.
  - **Antigravity:** `agy -p /usage --output-format stream-json`, the `gemini-5h` and `gemini-weekly` buckets,
    used = 1 - `remaining_fraction`. Every 300 s (one run took 20.8 s).
  - **Local:** Ollama on 127.0.0.1:11434. Not answering: "off". Answering: the GPU's memory, "3/32 GiB", from
    `nvidia-smi`, else "on". Every 30 s.
  - A failed read keeps the last figure. Before the first read a row says "reading".
  - Dots in Nirvana's provider colours (dark set): Claude #D8886A, Codex #7BAEDD, Antigravity #AB93D6, Local in the
    theme's muted text.
- **Layout.** At the foot of the expanded sidebar, under the agents panel, which gives up the rows:
  ```
  ──────────────────────────
   servers
   ● MotoProject      :8770
   usage            5h   7D
   ● Claude          2%  70%
   ● Codex      not exposed
   ● Antigravity     0%   0%
   ● Local         3/32 GiB
  ```
  Two figure columns under a "5h 7D" head, which fits herdr's default 26-column sidebar. From 34 columns a bar (the
  5h figure, as Nirvana's bar shows the shortest window) sits between the name and the figures. At most four server rows,
  then "+N more". The block is dropped when the agents panel would keep fewer than five rows.
- **Departures from Nirvana, by design.** No "local on/off" switch: herdr routes nothing to a local seat, so Local shows
  the engine's state instead. No "this machine" word: the head carries the column labels at 26 columns.

## Files

| File | Change |
|---|---|
| src/nirvana_servers.rs (new) | the listener table (Windows), the filter, the cache and its worker, tests |
| src/nirvana_usage.rs (new) | the four readers and parsers, the rows, the worker, tests |
| src/main.rs | the two modules |
| src/server/client_shell.rs | the `nirvana_servers` token on each snapshot space |
| src/client/shell/nirvana_footer.rs (new), src/client/shell.rs | the blocks: height, rows, drawing, server hits |
| src/client/shell/sidebar.rs | the foot taken from the agents panel's area |
| src/client/shell/state.rs, src/client/shell/mouse.rs | the server-row hits; a click opens the address |
| src/client/mod.rs | a changed usage reading repaints on the next timer tick |
| src/client/shell/tests/nirvana_footer.rs (new), src/client/shell/tests/mod.rs | rendering tests |
| Cargo.toml | windows-sys `Win32_NetworkManagement_IpHelper` for the listener table |
| nirvana/controls.py, nirvana/CHANGELOG.md, nirvana/VERSION, nirvana/README.md | controls, entry, 0.4.0, usage |

## Record

Built 2026-10-15 on alpha 0.3.0 (2deb7a04), as alpha 0.4.0. The parked 0.2.1 "Reload first" notice stays in `git stash`
and is not in this build.

**Measured before building (2026-10-15, this machine).**
- `claude -p "/usage" --no-session-persistence`: model `<synthetic>`, 0 tokens, `total_cost_usd` 0, 2.7 s. Run from Git
  Bash without `MSYS_NO_PATHCONV=1`, the argument became `C:/Program Files/Git/usage` and cost one real model turn; the
  window passes it without a shell, so it is not exposed to that. `--bare` answers with the API cost view instead.
- `agy -p /usage --output-format stream-json`: 20.8 s, `command_result` with three groups; `gemini-5h` and
  `gemini-weekly` are the Gemini group's windows.
- Codex 0.42: the 2026 rollouts on this machine never finished a turn, so none holds `rate_limits`; a 2025 rollout
  does (`used_percent`, `window_minutes` 299 and 10079, `resets_in_seconds`). Codex reads "not exposed" today.
- Listening sockets of agent programs: an idle `claude.exe` on 127.0.0.1:64839, `Antigravity.exe` on 60015 and 60016.

**Departures from the shape, found while building.**
1. *The full name wins over "localhost".* `localhost:<port>` is drawn only when the space's whole name fits beside it;
   at herdr's default 26 columns a name like "MotoProject" takes the cells and the address is `:8770`.
2. *The foot is all or nothing.* When the agents panel would keep fewer than five rows, the foot is not drawn; the
   servers block is not dropped first.
3. *Upstream's scrollbar test composes at 32 rows,* not 26 (CHANGELOG).

**Live run**, scratch home `scratchpad/nh-test4`, session `nirvana-s4`, folder `scratchpad/nirvana-foot-test`, a
`python -m http.server 8791 --bind 127.0.0.1` started in the space's pane with `herdr pane run`. The owner had a game in
front, so the first capture (`f4/shot-1.png`, the old script's screen copy) caught the game; `f4/capture-quiet.ps1`
reads the window with `PrintWindow` without raising it. Leaf agent "Sonnet - read quiet foot shot" read `f4/shot-2.png`:
`servers`, `● nirvana-foot-te…  :8791` (green), `usage 5h 7D`, `● Claude 5% 70%` (salmon), `● Codex not exposed`
(light blue), `● Antigravity 1% 1%` (lavender), `● Local 9/32 GiB` (grey); figures under their column labels, no
"reading", no missing glyph.

**A defect the live run found, and its fix.** The first run (`f4/shot-3.png` to `shot-6.png`): after Ctrl+C stopped the
server (port 8791 refused, "Keyboard interrupt received, exiting."), the row `nirvana-foot-te…  :8791` stayed, even
after an `echo` redrew the pane. Cause: the worker dropped a space nobody asked about for 60 s, and the server asks only
when it rebuilds a window's snapshot, which pane output does not do. A temporary test driving the worker directly
cleared a killed server within 3 s, which placed the fault in the asking. The worker now keeps a space while any of its
pane shells runs. Second run, after a rebuild: `python -m http.server 8792` started, 75 s idle, then Ctrl+C. Leaf agent
"Sonnet - read running/stopped shots": `shot-7-running.png` holds `servers` / `● nirvana-foot-te… :8792`;
`shot-8-stopped.png`, 8 s after the stop, has no servers block and the four usage rows (`Claude 7% 70%`,
`Codex not exposed`, `Antigravity 0% 0%`, `Local 9/32 GiB`).

**Not measured.** A click on a live server row: it would open a tab in the owner's browser while they were in a game;
`a_click_on_a_server_row_opens_its_address` holds the action, and opening it is upstream's `OpenSafeWebUrl` path. The
Claude reading's faster pace while a Claude works (120 s): no Claude worked in the scratch window. A Codex figure: no
rollout on this machine holds a live one. Servers on Linux or macOS: the listener table is read on Windows only. The
same ask-driven refresh in feature 3's model cache: a model changes when an agent's state changes, which rebuilds the
snapshot, so it was not seen to stick; a `/model` switch with no state change was not run.
