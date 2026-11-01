# Expectations for the plan-35 probes, written before each run (2026-10-31)

Both: herdr's agy argv minus --json-schema (agy --output-format stream-json --model gemini-3.8-flash-low
--print-timeout 170s --dangerously-skip-permissions ...), cwd an empty folder, stdin null, CLAUDECODE / CLAUDE_CODE_*
removed, BROWSER and GH_BROWSER = ..\bin\noop.cmd, a 180 s wall from Python with a list argv.

## Probe 1: can --settings or --add-dir carry MCP servers (and rules)?

cwd p1/work (empty). Flags added: --settings <p1/settings.json> holding mcpServers.probeset and
permissions.deny ["write_file(*)"]; --add-dir <p1/side> whose .agents/mcp_config.json names probeadd and whose
AGENTS.md holds MARKER-ADDDIR-5521. The model is asked to list its MCP servers and tools, quote any MARKER line,
and try to write one file x.txt in its cwd.

- probeset listed: NO (60%). The CLI reference lists 32 settings.json keys and mcpServers is not one; --settings is
  documented only as "settings.json keys". An unknown key is most likely ignored, possibly an error (then exit != 0).
- probeadd listed: NO (55%). The MCP page names `<workspace>/.agents/mcp_config.json`; whether an --add-dir folder
  counts as a workspace for MCP is not written. If YES, herdr can keep the config outside the worktree.
- MARKER-ADDDIR-5521 quoted: YES (55%): rules walk "up to the workspace root", and --add-dir adds "a directory to
  the workspace".
- write_file denied by --settings: YES (65%): changelog "headless runs now honor persisted settings.json policies,
  including permissions", and the CLI reference "Deny rules apply in every mode"; whether a --settings deny beats
  --dangerously-skip-permissions is unwritten.

### Probe 1 as run: refused at argv parse, no model call

`Error: --settings: unknown setting "mcpServers"`, exit 1 in 0.1 s, no session, no tokens. So --settings cannot carry
MCP servers (measured), and it validates keys. Probe 2 below drops mcpServers from the settings file and folds in the
bladebro test, so the cap of two agy invocations holds.

## Probe 2 (folder p2): --add-dir MCP and rules, a --settings deny, and bladebro under exebox

cwd p2/work, holding only .agents/mcp_config.json naming `bladebro` = `cmd /c bladebro mcp` with BLADE_HOME
p2/blade, BLADE_IDLE_TIMEOUT 60, BLADE_CHROME_FLAGS --headless=new, BLADE_LANE agent, BLADE_NO_UPDATE_CHECK 1
(decision 0043's recipe). --settings p2/settings.json = {"permissions":{"deny":["write_file(*)"]}}. --add-dir p2/side
(.agents/mcp_config.json naming probeadd; AGENTS.md with MARKER-ADDDIR-5521).

- bladebro listed with its five tools (act, see, state, run, vision): YES (85%), the workspace channel proven by the
  research's probe 2.
- bladebro reads https://example.com and the title "Example Domain" comes back: YES (60%). The doubt is exebox: the MCP
  server may start inside the isolated desktop; headless Chrome needs no desktop, but a job or desktop limit may stop
  Chrome. The call shows as call_mcp_tool with the server name in its parameters (70%).
- Chrome processes left under p2/blade after the run: none (70%): on Windows the server owns its Chrome.
- probeadd listed: NO (55%). MARKER-ADDDIR-5521 quoted: YES (55%).
- --settings deny stops write_file: YES (60%); --dangerously-skip-permissions may win.
