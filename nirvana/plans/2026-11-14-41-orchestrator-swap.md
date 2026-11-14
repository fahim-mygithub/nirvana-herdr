# Plan 41: the orchestrator seat moves to agy when Claude runs low

Status: PROPOSED 2026-11-14. Probe done. No code yet. Owner rulings R1-R4 open.

## 1. The ask

The owner, 2026-11-14: "Currently our Nirvana design open the orchistrator panal with Claude always. However if we are
running out of weekly or 5h limit we have to stop all work because the orhcistrator cant run even if all its job are
in Gemini agy. Is it possible for Nirvana to swap the Orchistrator seat during mid work as our limits get constrained
to Gemini. Can the full orchistrator seat get passed off? Will context job runns seating skills preload tools and mcps
carry over? What about lessons?"

Then, to the proposal of a probe and a plan: "Go ahead".

## 2. Requirements ledger

| # | Quote | Surface | Owner | Verifier |
|---|-------|---------|-------|----------|
| L1 | "swap the Orchistrator seat during mid work as our limits get constrained" | a handoff trigger on the Claude pool's readings | herdr | a test on fixture readings, live check at a real low window |
| L2 | "Can the full orchistrator seat get passed off?" | the agy orchestrator gets the same hooks, skills, role and MCPs | herdr + install | the probe in §4, then a control per hook |
| L3 | "Will context ... carry over?" | the brief from `nirvana_fresh` starts the agy session | herdr | a test that the brief names the open run and orders |
| L4 | "job runns seating" | runs, orders, jobs and `herdr seat` stay as they are | herdr | nothing changes; a live check that a seated job finishes across the swap |
| L5 | "skills preload tools and mcps" | an agy plugin built from `nirvana/claude` | install | `agy plugin validate --strict` and a live `/hooks`, `/agents`, `/mcp` read |
| L6 | "What about lessons?" | lessons in briefs keep working; learning reads agy transcripts | herdr | a test on a fixture agy transcript |

## 3. What carries over today, and what does not

Carries over without work, because herdr owns it and any agent that runs `herdr` gets it:

- runs, orders, jobs, worktrees and checkpoints under `.nirvana/`;
- seat routing, bookings and the override cap, since `herdr seat run` is a command;
- lessons as used: herdr writes them into each worker's brief, whatever the orchestrator is;
- the board, the usage readings, the agents panel, the dead-panes sweep.

Does not carry over:

- the conversation. By design: plan 12 §2, a model switch drops the reasoning. The new session gets a brief.
- the Claude plugin: 15 hooks, 8 skills, the `nirvana` agent, `ROLE.md`, `settings.json`. §5 ports it.
- `--rc` and `--chrome`. agy has its own `--remote-control`, and the owner's agy already has `chrome-devtools-plugin`.
- learning from the session. `memory record-session` and the lesson review read Claude transcripts only.

## 4. The probe, 2026-11-14

### 4.1 Gemini CLI is out

Gemini CLI 0.25.0-nightly has hooks with the right shape: `AfterAgent` can block and sends its reason back as a new
prompt (`core/client.js`, `isBlockingDecision`). But it does not start on the owner's account:

```
Error authenticating: ProjectIdRequiredError: This account requires setting the GOOGLE_CLOUD_PROJECT or
GOOGLE_CLOUD_PROJECT_ID env var.
```

Its cached login is a Workspace account that needs a Google Cloud project. That is a sign-in change, so the probe
stopped there. The hooks never fired.

### 4.2 agy 1.5.9 has what the orchestrator needs

agy updated itself on 2026-11-14 00:45. The note in memory `agy-headless-gap` (2026-10-31: no MCP from herdr,
ignores CLAUDE.md) is out of date. 1.5.9 has plugins with skills, MCP servers, subagents, rules and hooks, plus
`--agent`, `--settings` for one session, `--remote-control` and `agy plugin import claude`. Its built-in docs give the
hook contract: events `SessionStart`, `PreInvocation`, `PreToolUse`, `PostToolUse`, `PostInvocation`, `Stop`, camelCase
JSON on stdin, JSON on stdout.

The probe ran `agy -p ... --dangerously-skip-permissions` in a scratch folder with a workspace `.agents/hooks.json`.
Nothing under `~/.gemini` was written. The hook script logged every event and acted as Nirvana's hooks would. The
prompt asked agy to write `a.txt`, then report a secret word, a run id and whether the write worked.

| Check | Hook output | Result |
|---|---|---|
| control: no hooks file | none | the write worked; "Secret word: None was provided" |
| start context, our `nirvana digest` | `SessionStart` `injectSteps` systemMessage "the secret word is MANGO" | answered `MANGO` |
| per-prompt notice, our `run notice` | `PreInvocation` `injectSteps` "the run id is R-77" | answered `R-77` |
| edit guard, our `guard-edit` | `PreToolUse` `{"decision":"deny"}` on tool `write_file` | "No, `write_file` was denied by a pre-tool hook"; no `a.txt` |
| the gate, our `nirvana stop` | `Stop` `{"decision":"continue","reason":...}` | the agent continued and answered `GATE-SEEN` |

The deny held under `--dangerously-skip-permissions`. Each payload carried `transcriptPath`, here
`~/.gemini/antigravity-cli/brain/<id>/.system_generated/logs/transcript_full.jsonl`, and `modelName`
`gemini-4-carbon-medium`.

One defect found. agy's docs say `continuationActive` is true while the agent works on a continuation a Stop hook
asked for. In `-p` it stayed `false` through every Stop, so the probe's hook asked again each time: 10 Stops, 10
`GATE-SEEN` lines, then agy ended the turn by its own limit. A gate on agy must count its own asks and not trust that
field. The interactive mode, which the orchestrator pane uses, was not probed.

A second run started agy in another folder and passed the hooks folder with `--add-dir`. All four hooks loaded from
there: `MANGO`, `R-77`, `write_file` denied, `GATE-SEEN`. The gate kept its own count in a marker file, and agy
stopped after 2 Stops with one `GATE-SEEN`. So the plugin can reach agy with no write under `~/.gemini` and no file in
the project, the same way plan 35 gives agy workers their tools.

## 5. The design

### 5.1 One hook adapter in herdr

Each Nirvana hook command takes `--host agy`. It reads agy's camelCase input and writes agy's output shape:

| Claude hook | agy event | Output change |
|---|---|---|
| SessionStart: `nirvana digest`, `log-usage`, `memory record-session` | `SessionStart` | `additionalContext` becomes `injectSteps` systemMessage |
| UserPromptSubmit: `board notice`, `run notice`, `prompt-baseline` | `PreInvocation` | fires before every model call, not once per prompt: dedupe on `lastUserInput` |
| PreToolUse Agent: `guard-spawn` | `PreToolUse` on agy's subagent tool | name to find in the interactive probe |
| PreToolUse Edit/Write: `guard-edit` | `PreToolUse` `write_file` and agy's edit tools | `deny` with the reason |
| PreToolUse Read: `guard-read` | `PreToolUse` `view_file` | same |
| PostToolUse: `check-edit` | `PostToolUse` | output `{}` |
| SubagentStop: `subagent-stop` | none | the agents panel already reads seated orders; agy's own subagents go unseen |
| Stop: `nirvana stop`, `log-usage`, `record-session`, `jev compact` | `Stop` | `block` becomes `decision: continue`; the gate keeps its own ask count |

The owner-typing checks (`owner_typed_since`, `--send-ok`, the override cap) read Claude's `promptSource`. On agy they
read `lastUserInput` and the transcript's user steps. Until a test proves that an agent cannot forge those, every
check that needs the owner's words answers "not proven" on agy. The forgery finding on `feat/owner-override` applies
here too.

### 5.2 An agy plugin built from `nirvana/claude`

`install.py` writes `nirvana/agy/.agents/`: a `hooks.json` in agy's named-hook format, `skills/`, `agents/`,
`ROLE.md` as a rule, and bladebro as a plugin MCP server. `agy plugin validate --strict` gates the build. How it reaches
agy is R1.

### 5.3 The handoff

1. **Trigger.** herdr reads the Claude pool at each task boundary of the orchestrator: a run ends, an order is seated,
   or the owner sends a prompt. At or above `handoff_at` on any window, it offers the swap in the window and on the
   phone. It acts by itself at `handoff_force`. Both live in `[orchestrator]` in `seats.toml`. Their values are R2.
2. **Brief.** `nirvana_fresh::note` already builds a fresh session's note: the open run, its orders, the last owner
   prompts. The handoff writes it to `.nirvana/orchestrator/handoff-<n>.md`.
3. **Seat.** The Claude session gets a last prompt to finish its turn. Then herdr starts `agy --agent nirvana --model
   <seat> -i "<brief>"` in the same pane, with the agy plugin on. The seat is R3.
4. **Return.** When the Claude window resets and the agy session reaches a task boundary, herdr offers the way back,
   with a brief written the same way.

Jobs and seated workers do not stop at any step. They belong to herdr, not to the orchestrator's session.

### 5.4 Learning

`memory record-session` and the idle lesson review take agy's `transcriptPath` from the hook input and read its
steps. Lessons used in briefs need no change.

## 6. Packages

| Package | What | Gate |
|---|---|---|
| P41a | interactive probe in a scratch herdr pane: `continuationActive`, agy's subagent and edit tool names, `/hooks` read | numbers in the plan record |
| P41b | the `--host agy` adapter for each hook, with fixture payloads from §4.2 | tests + a sabotage control per hook |
| P41c | the agy plugin build and its validation | `agy plugin validate --strict`, controls |
| P41d | trigger, brief, launch and return | tests on fixture readings; live check at a real low window |
| P41e | learning from agy transcripts | a test on a fixture transcript |

P41b and P41c can run in parallel: disjoint files. P41d needs both.

## 7. Owner rulings

- **R1, where the agy plugin lives.** (a) A `.agents/` folder in the install, passed with `--add-dir` at launch. It
  touches no `~/.gemini` and no project, and §4.2 proved hooks load that way. Skills, agents and MCP from that folder
  are still to prove in P41a. (b) `install.py` runs `agy plugin install` from a local folder, which writes into agy's
  config under `~/.gemini`. (c) A `.agents/` folder in each project. Recommended: (a), with (b) only if P41a finds a
  part that loads from a plugin alone.
- **R2, the thresholds.** Recommended: offer at 90% of the 5h window or 95% of the week. Force at 98% of either.
- **R3, the agy orchestrator seat.** Recommended: Carbon high. The owner already names it for checking work: "validate it with AGY
  Carbon high".
- **R4, Gemini CLI.** Recommended: drop it. It needs a Google Cloud project on the owner's account, and agy already
  works.

## 8. Not done, and unsure

- The interactive mode was not probed. `continuationActive` may work there.
- agy's names for its subagent tool and its edit tools other than `write_file` are not known yet.
- Whether an agy session can forge `lastUserInput` is untested. §5.1 treats it as forgeable.
- The Gemini quota a long agy orchestrator session would use is not measured. The probe's one turn did not move the
  readings in a way worth quoting.
- Probe files: the scratchpad folders `gprobe` and `aprobe` of the session that wrote this plan. They are not tracked.
