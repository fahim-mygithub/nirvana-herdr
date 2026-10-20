# Feature 20: every Claude spawn is seated, and the orchestrator's own seat follows the mode

Status: **PLANNED (2026-10-20).** Builds after plan 19, whose climb (§6.2) it reads. To be built on agy (Gemini 3.8
Flash high); record `2026-10-20-20-records/p20.md`.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "We should address the two still not in this plan" | §2 the spawn guard, §3 the orchestrator's seat | orchestrator | as L2 and L3 |
| L2 | The first of the two, as the owner read it: "Forcing Nirvana Claude to use the picker. In your watch project, Nirvana Claude started 12 Claude subagents and never asked the picker once. Better seat choices only help when it asks, so this matters most for your usage." | §2 | orchestrator | unit tests on `guard` with fixed leases |
| L3 | The second, as the owner read it: "Nirvana Claude's own model. It stays Opus medium, as you ruled on Oct 17." The owner then chose "Follow the mode (Recommended)", whose text was: "Efficient: Sonnet medium. Balanced: Opus medium while Claude's week is cool, Sonnet medium once it climbs. SOTA: Opus high. The model is picked when Claude starts. Mid-session, a notice tells you when a restart would switch it, and the Restart Claude button does it." | §3 | orchestrator | unit tests on the seat rule and the launch args; client test on the notice |

L3 replaces plan 12 §12 R1 ("The orchestrator is Opus 5.5 at medium") for every mode but a cool Balanced.

## 1. Measured 2026-10-20

- **The spawns:** in 3D-Printer-Flow-xtool, Nirvana Claude made 12 `Agent` calls, all `general-purpose` with
  `model: sonnet`, and 0 calls of `herdr seat pick`. Five were web shopping research, which the picker sends to Flash.
  Today's guard checks only that a spawn names a model and an effort, and the Max 5 h reserve.
- **The orchestrator's own turns:** 341 Opus turns and three compactions in that session, $28 at list price, more than
  its 12 subagents ($21).
- **`--model` wins over the agent file.** `claude -p --plugin-dir <copy of the plugin's agents> --agent nirvana --model
  claude-haiku-5-5 --effort low` ran on `claude-haiku-5-5` ($0.0003); with `--model claude-sonnet-5-5` it ran on
  `claude-sonnet-5-5`. `agents/nirvana.md` names `claude-opus-5-5`. So the launch flags alone choose the seat.

## 2. The spawn guard

`herdr nirvana guard-spawn`, the `PreToolUse` hook on `Agent`, adds one rule before its reserve check:

- **A spawn needs a booking.** Its brief's `Order:` line must name an order with a live lease in `leases.json`, booked
  by `herdr seat pick --book <order>`.
- **The booking must be for this seat.** The lease's seat must be the Claude seat the spawn runs on (the model, by
  `seat_for_model`), at the spawn's effort or lower.
- **On a refusal, the reason says what to do,** in one message:
  - no `Order:` line, or no live lease: `herdr guard: seat this first: herdr seat pick --class <class> --size small
    --book <order> --run <run>`, naming the brief's `Class:` when it has one;
  - the lease is an agy seat: `herdr guard: the picker seated <order> on <seat> <effort> (<pool>). Start it with herdr
    seat run <seat> --order ...`;
  - another Claude seat or a higher effort: the lease's seat and effort, and that the spawn must match it.
- **The existing refusals stay:** the `nirvana` agent, a fork, no model or effort, the reserve.
- **No exceptions by agent type.** Claude Code's built-in agents (`general-purpose`, `Explore`, and the rest) are
  seated like any other. A one-off read the orchestrator does itself needs no spawn (ROLE's small-asks rule).
- **The fork's own `worker-*` agents** say in their description that a booking comes first.

ROLE.md and the `seat` skill gain one line each:
- "Every subagent is seated first: `herdr seat pick --book <order>`, then spawn the seat it gives, or `herdr seat run`
  when it gives an agy seat. The hook refuses a spawn without a booking."
- "Web research, shopping and reading pages are `research.web`: seat them, never browse yourself for more than one
  page."

## 3. The orchestrator's seat follows the mode

### 3.1 The rule

`herdr seat orchestrator [--json]` prints the seat for Nirvana Claude:

| Mode | Seat |
|---|---|
| Efficient | sonnet medium |
| Balanced | opus medium while the max pool's climb `t` (plan 19 §6.2) is under 0.5; sonnet medium from 0.5 |
| SOTA | opus high |

- `t` is plan 19's: the same `climb_start` and `climb_full`, on the max pool's windows.
- **When the readings are missing or stale,** Balanced keeps opus medium, today's seat, and says why.
- **The line it prints:** `nirvana claude -> sonnet medium: Balanced, max week climb 1.18`.

### 3.2 At launch

- `nirvana_claude_args` and `launch_args` take the seat from the rule in place of the `MODEL` and `EFFORT` constants:
  `--model claude-sonnet-5-5 --effort medium`, for example.
- If the table or the readings fail to load, the launch uses opus medium, today's seat, so a broken table never blocks
  a start.
- **The Restart Claude button** resumes the session through `launch_args`, so a restart takes the rule's seat at that
  moment and keeps the conversation.

### 3.3 Mid-session

- **The `SessionStart` hook** records the seat it started on, from the hook input's `model` field and the launch's
  effort, in `<home>/nirvana/sessions/<session_id>.json`, next to what is kept there today.
- **The `UserPromptSubmit` notice** compares that seat with the rule's seat now. When they differ, it adds one line
  for the orchestrator to pass on, once per change:
  `Nirvana Claude runs on opus medium; the mode now seats sonnet medium (Balanced, max week climb 1.18). Press Restart
  Claude to switch.`
  It says it again only when the rule's seat changes again.
- **The Seats square** shows the orchestrator's seat as its first row: `Nirvana Claude · sonnet medium · Balanced,
  climb 1.18`, and `restart to switch` when the running seat differs.
- **ROLE.md** stops saying "You run on Opus 5.5 at medium effort". It says the session digest names its seat, and that
  when the notice says the seat changed it tells the owner in one line.

## 4. Not in this plan

- **Workflows.** The `Workflow` tool starts its own agents, outside the `Agent` hook. It runs only when the owner asks
  for one, so it is not guarded here.
- **The orchestrator browsing.** The ROLE line in §2 asks it to seat page work. Nothing enforces it.
- **Switching the model without a restart.** Claude Code changes models mid-session only by `/model`, typed into the
  prompt. herdr does not type into the owner's prompt.

## 5. Verification

- **Unit tests:**
  - `guard`: no `Order:` line; an order with no lease; an expired lease; a lease on flash; a lease on sonnet medium
    with an opus spawn; with a sonnet high spawn; with a sonnet medium spawn, which is allowed; the existing refusals
    unchanged. Each refusal's message names the command to run.
  - The seat rule on fixed worlds: each mode; Balanced at `t` 0.3, 0.5 and 1.0; stale readings.
  - `launch_args` carries the rule's model and effort, and opus medium when the table fails to parse.
  - The notice: same seat, no line; a different seat, one line; the same difference again, no line; a second change,
    a new line.
- **Client test:** the Seats square's orchestrator row, with and without `restart to switch`.
- **No test** runs Claude, reads `~/.claude` or the real install home.
- **Controls:** every assertion gets a sabotage control in `nirvana/controls.d/p20.py`.
- **After the install**, one live check: `herdr seat orchestrator` on the owner's readings, pasted into the record.
