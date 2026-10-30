# Hermes Agent source review (NousResearch/hermes-agent, shallow clone at main, 2026-10-30)

The clone is at `C:\Users\fahim\AppData\Local\Temp\claude\C--Users-fahim-Desktop-Pojects-Nirvana\cdd39677-8dc6-4a13-8f1c-2970c2340402\scratchpad\hermes-src`. I only read files and ran nothing from it. The repo is huge (about 21.9k tracked files, Python), so my coverage is deep on profiles and the learning loop and shallower on the rest. Paths below are relative to the repo root.

## Summary table

| Feature | Where in code | How it works in one line |
|---|---|---|
| Profile | `hermes_cli/profiles.py`, `profile_cmd.py`, `hermes_constants.py` | A profile is a whole separate `HERMES_HOME` directory (`~/.hermes/profiles/<name>/`), not a config record; `-p <name>` or sticky `active_profile` sets the env var before imports. |
| Background review (the learning loop) | `agent/background_review.py`, `agent/turn_finalizer.py`, `agent/turn_context.py` | Every N turns or N tool iterations, a forked agent replays the transcript after delivery and may write memory and skills. |
| Memory store | `tools/memory_tool.py`, `tools/memory_tool_store.py` | Two char-capped files, `MEMORY.md` (2200 chars) and `USER.md` (1375 chars), frozen into the system prompt at session start. |
| Skills | `tools/skill_manager_tool.py`, `tools/skill_usage.py`, `tools/skill_ledger.py` | The `skill_manage` tool writes `SKILL.md` plus `references/ templates/ scripts/`; a usage sidecar and an audit ledger with rollback sit beside it. |
| Curator | `agent/curator.py`, `hermes_cli/curator.py` | After 7 days and 2 idle hours, a deterministic prune (stale at 14 days, archive at 30) runs; an opt-in LLM fork consolidates skills into umbrellas. |
| Session search | `tools/session_search_tool.py`, `hermes_state_fts.py` | FTS5 over SQLite `state.db`, **no LLM** in the code (the README claims LLM summarisation). |
| User modelling | `agent/memory_provider.py`, `plugin-catalog/` (Honcho) | Pluggable single external memory provider; Honcho is a catalog plugin, docs-only. |
| Subagents | `tools/delegate_tool*.py` | `delegate_task` spawns child `AIAgent`s with fresh context; the parent sees only the summary. |
| Kanban | `plugins/kanban/`, docs `kanban.md` | A shared SQLite task board at `~/.hermes/kanban.db`; dispatcher spawns one OS process per worker profile. |
| Cron | `cron/scheduler.py`, `cron/jobs.py` | Jobs in `~/.hermes/cron/jobs.json`, ticked every 60s by the gateway, with delivery to any platform. |
| Gateway | `gateway/`, `plugins/platforms/*` | One process serves Telegram, Discord, Slack, Signal, WhatsApp, Matrix and about 20 more. |
| Terminal backends | `tools/environments/` | `local`, `docker`, `ssh`, `modal`, `managed_modal`, `daytona`, `vercel_sandbox`. |
| Compression | `agent/context_compressor*.py`, `config_defaults.py` `compression` | An auxiliary model summarises the middle turns at 50% of context (floored at 0.75 for windows under 512K), protecting head and tail. |
| Checkpoints | `tools/checkpoint_manager.py` | A shadow bare git store under `~/.hermes/checkpoints/` snapshots the working dir once per turn before file-mutating operations, with rollback. |
| Approvals | `tools/approval*.py`, config `approvals` | Regex-detected dangerous commands; modes include `smart` (a guardian LLM, default), with a 300s timeout and `cron_mode: deny`. |
| MCP | `tools/mcp_tool*.py` | An MCP client with OAuth, schema cache, liveness checks, and a per-job allowlist by `mcp-<server>` toolset. |
| Plugins and skill hub | `plugins/`, `plugin-catalog/*.yaml`, `tools/skills_hub*.py` | Catalog plugins; the skills hub pulls from official, GitHub, ClawHub and skills.sh sources. |
| Batch and trajectories | `batch_runner.py`, `trajectory_compressor.py`, `agent/trajectory.py` | Multiprocessing JSONL runner with `--resume`, writing from/value trajectories, then a compressor to a token budget. |
| Usage and cost | `agent/insights.py`, `agent/account_usage.py`, `agent/credential_pool.py` | Aggregates state DB usage into token and cost estimates; credential pools and a per-aux-call usage ledger. |

## 1. Profiles

**Definition.** A profile is a separate Hermes home directory. `hermes_cli/profiles.py` says "Profile management for multiple isolated Hermes instances". Docs `website/docs/user-guide/profiles.md` also say it carries its own `config.yaml`, `.env`, `SOUL.md`, memories, sessions, skills, cron jobs and `state.db`.

**Location.** It is per user: `~/.hermes/profiles/<name>/`, and `%LOCALAPPDATA%\hermes` on native Windows. It is not per project. A directory counts as a profile only if it holds one identity file: `config.yaml`, `.env`, `SOUL.md`, `profile.yaml`, `auth.json` or `state.db`.

**Bootstrapped layout** (`profiles.py`, `_PROFILE_DIRS`): `memories, sessions, skills, skins, logs, plans, workspace, cron, home`.

**Fields.** There is no profile schema; the fields are the contents of the home directory:
- **`config.yaml`** (`cli-config.yaml.example`, defaults in `hermes_cli/config_defaults.py`). It has `model.{default,provider,base_url,context_length,...}`, plus sections for toolsets, `memory`, `skills`, `delegation`, `compression`, `approvals`, `auxiliary.<task>`, `curator`, `cron`, MCP servers and terminal backend. It is versioned with `_config_version: 52`.
- **`.env`**: per-profile secrets.
- **`SOUL.md`**: the personality and system prompt. The repo's own example is a single paragraph beginning "You are Hermes Agent, built by Nous Research. Be direct: match the length of your reply to the weight of the ask…".
- **`profile.yaml`**: tiny metadata only. `read_profile_meta` reads `description`, `description_auto`, `display_name`, `previous_names` and `ui_meta['hermes-bots'].title`. The description is what the kanban orchestrator routes on.
- **`distribution.yaml`** (docs-only, not seen in code): used only when sharing a profile.

**Real example.** `distribution.yaml` from the docs (`profile-distributions.md`):
```yaml
name: research-bot
version: 1.0.0
description: "Autonomous research assistant with arXiv and web tools"
hermes_requires: ">=0.12.0"
env_requires:
  - name: OPENAI_API_KEY
    required: true
```
The distribution repo layout is `distribution.yaml, SOUL.md, config.yaml, skills/, cron/, mcp.json`.

**Selection.** `-p/--profile <name>` is pre-parsed in `hermes_cli/main.py:_apply_profile_override`, which sets `HERMES_HOME` before imports. Otherwise it falls back to a sticky `~/.hermes/active_profile` file, set by `hermes profile use`. `hermes profile create <name>` also writes a wrapper command, so `coder chat` works.

**Isolation.** Everything lives under the profile home. `agent/memory_provider.py:ctx_bound` notes that profile isolation is a ContextVar-scoped `HERMES_HOME` override, and a worker started with an empty context "silently lands on the default profile". OAuth logins for Anthropic, Codex and xAI are shared from root `~/.hermes/auth.json`, because refresh tokens are single-use. Static API keys are copied on clone.

**Create, edit, clone.**
- **CLI.** `hermes profile create|delete|describe|show|alias|rename|export|import|install|update|info`. Create takes `--clone` (config, `.env`, `SOUL.md`, skills, `MEMORY.md`, `USER.md`), `--clone-all` (everything except history, cron and messaging channels) and `--clone-from`. `profile describe` can auto-generate the description with an LLM (`profile_describer.py`).
- **By the agent.** I did not find a profile-creation tool. Docs mention a dashboard and Desktop "Bot Mode" UI, which I did not trace.
- **Warning.** Docs say never point two agent processes at one profile.

## 2. Self-learning after each run

### 2a. Trigger (`agent/turn_finalizer.py`, `agent/turn_context.py`, `agent/agent_init.py`)

- **Memory review.** `_tick_memory_nudge` counts user turns. When `_turns_since_memory >= memory.nudge_interval` (default **10**, `config_defaults.py`), it fires. It needs the `memory` tool present and a memory store. The counter resets whenever the model itself calls `memory` (`tool_executor.py:~748`). Hydrated on resume as `prior_user_turns % interval`.
- **Skill review.** `_iters_since_skill` is bumped once per tool-loop iteration (`turn_iteration_prep.py:166`, only if `skill_manage` is a valid tool). When it reaches `skills.creation_nudge_interval` (default **10**), it fires at the end of the turn. It resets when the model calls `skill_manage`.
- **Conditions.** There must be a final response, the turn must not be interrupted, and `skip_background_review` must be false (cron sets it; the comment says the fork "costs ~30K tokens / event"). A setting of `0` disables either nudge, and the fork itself is built with both nudges at 0 so it cannot recurse (`background_review.py:999`).
- **Timing.** It runs after delivery, in a daemon thread. A new live turn cancels it. It runs on the **same model, credentials and cached system prompt** as the parent for prefix-cache hits, unless `auxiliary.background_review.{provider,model}` is set. The fork is capped at `_REVIEW_MAX_ITERATIONS = 16`.

### 2b. Prompt and tools

The dispatch-side whitelist (`_review_tool_whitelist`) allows the `skills` toolset, the `memory` toolset only if the memory trigger fired, and read-only `read_file` and `search_files`. Writes go through `skill_manage` and `memory` only, and dangerous commands are auto-denied (`_bg_review_auto_deny`).

Key text from `_SKILL_REVIEW_PROMPT`:
> "Review the conversation above and update the skill library. Be ACTIVE — most sessions produce at least one skill update, even if small. A pass that does nothing is a missed learning opportunity… Target shape of the library: CLASS-LEVEL skills, each with a SKILL.md of always-on rules and a small `references/` set of topical depth."

Preference order in the prompt: (1) patch a currently-loaded skill, (2) patch an existing umbrella, (3) add a `references/`, `templates/` or `scripts/` file, (4) create a new class-level skill. Names must not be PR numbers or "fix-X" session artefacts.

Other prompt constraints:
- **Quality rule.** "A pitfall is a generalizable rule + one clause of WHY… No PR/issue numbers, dates… The same lesson learned twice is ONE rule."
- **Read-before-write.** `skill_manage` refuses a patch unless `skill_view` was called in that review.
- **Frustration as signal.** Corrections and complaints are "FIRST-CLASS skill signals".
- **Do-not-capture block.** It excludes environment-dependent failures, negative claims like "tool X is broken", one-off task narratives and unresolved failures dressed up as best practice.
- **Memory routing** (`_MEMORY_REVIEW_PROMPT`). There are two stores: `USER.md` for who the user is and `MEMORY.md` for environment facts. "One fact goes to ONE store, never both." The fallback is "'Nothing to save.'".
- **Live-turn guidance** (`prompt_builder.py:build_memory_guidance`). Skills come first; memory is "the narrow exception"; write declarative facts, not imperatives ("User prefers concise responses" not "Always respond concisely"). Its rule `SKILLS_GUIDANCE` was reworded to dodge an Anthropic filter that rejected the old sentence on OAuth credentials, a quirk relevant to Claude Code seats.

### 2c. Writes, caps and anti-bloat

- **Memory.** `MEMORY.md` is capped at 2200 chars and `USER.md` at 1375 (`memory.memory_char_limit` / `user_char_limit`), with `§`-delimited entries. Over-limit adds are rejected with "free at least N chars" (`memory_tool_store.py:_over_limit`). Mid-session writes hit disk but the prompt snapshot stays frozen so the prefix cache is intact.
- **Skills.** They land in `~/.hermes/skills/<category>/<name>/SKILL.md` with frontmatter `name`, `description` (the system-prompt index truncates it to 60 chars), `version`, `author`, `platforms`, `metadata.hermes.tags` and `related_skills`. Example: `skills/software-development/codebase-inspection/SKILL.md`.
- **Usage sidecar.** `~/.hermes/skills/.usage.json` (`tools/skill_usage.py`) holds `created_by: agent`, `use_count`, `view_count`, `patch_count`, `last_used_at`, `pinned`, and lifecycle state active, stale or archived.
- **Audit ledger.** Every skill mutation appends to `skills/.curator_ledger.jsonl` with before and after blobs (`tools/skill_ledger.py`). `hermes curator rollback <id>` restores one edit. The ledger is capped at 5 MB.

### 2d. Human review

- **Default.** None. `skills.write_approval` and `memory.write_approval` both default to `False`. When true, writes are staged and approved with `/skills pending|diff|approve|reject` and `/memory pending|approve|reject`. The background review prints a one-line action summary (`summarize_background_review_actions`).
- **Skill scanning.** `skills.guard_agent_created` defaults off; hub installs are always scanned (`skills_guard.py`, plus an advisory NVIDIA SkillEvaluator scan).

### 2e. Curator (`agent/curator.py`)

- **Trigger.** It is inactivity-triggered, with no daemon: at least `interval_hours` 168 since the last run and `min_idle_hours` 2.
- **Deterministic phase.** It always runs: stale at 14 days unused, archive at 30 days into `skills/.archive/` (recoverable, never deleted). Pinned skills and cron-referenced skills are skipped.
- **LLM consolidation.** `consolidate: False` by default (opt in, or `hermes curator run --consolidate`). When on, `CURATOR_REVIEW_PROMPT` says "UMBRELLA-BUILDING consolidation pass… 'would a human maintainer write this as N separate skills, or as one skill with N labeled subsections?' When the answer is the latter, merge."
- **Hard rules in the prompt.** It never touches bundled or hub skills, never deletes, never touches pinned ones, and does not prune use=0 skills under 30 days old. It can be previewed with `hermes curator run --dry-run`.
- **Bloat rule.** A SKILL.md over about 24k chars is itself a consolidation target.

### 2f. Session search and context

- `tools/session_search_tool.py` searches the SQLite `state.db` with FTS5, including a CJK-bigram tokenizer (`hermes_state_fts.py`). Its modes are discovery, scroll, read and browse. Cron sessions are demoted in ranking.
- Its docstring says "No LLM calls — every shape returns actual DB messages", and the config says "session_search no longer uses an aux LLM (#27590)". **The README's "LLM summarization for cross-session recall" is stale relative to the code.**
- Compaction summaries include "session_search pointers" (`tail_mode: lean`).

### 2g. User modelling

- **Mechanism.** `agent/memory_provider.py` defines an ABC for one external provider at a time (`memory.provider`: honcho, hindsight, mem0, supermemory, retaindb, byterover, holographic, openviking). Its lifecycle is `initialize`, `prefetch`, `sync_turn`, then tools and `on_*` hooks. `agent/memory_manager.py` drives it.
- **Honcho.** Docs-only, not seen in code: the dialectic reasoning lives in the external Plastic Labs plugin. The in-repo side is `profiles_honcho.py`, which clones the Honcho config per profile.
- **Interplay.** Config says `nudge_interval` is "0 when an external provider auto-extracts".

### 2h. Trajectories and evaluation

- `batch_runner.py` runs a multiprocessing pool over a JSONL dataset with `--resume`, writes from/value trajectories and aggregates tool stats. `trajectory_compressor.py` summarises the middle of each trajectory to a token budget and protects the head and tail. `agent/trajectory.py` is the saving helper.
- **No evaluation of whether learned skills helped.** I found only usage counts (view, use, patch, "reused" and `patch_generation` fields) and the curator's staleness logic. There is no outcome-based scoring. The `evals/` directory holds wire and provider regression probes, not skill-efficacy tests.

## 3. Other features worth porting

- **Kanban** (docs `kanban.md`, `kanban-worker-lanes.md`; `plugins/kanban/`; `agent/kanban_stop.py`). This is the closest analogue to herdr's order queue plus seats.
  - Mechanics: a durable SQLite board shared by profiles, where each worker is a full OS process with a profile identity, and the model drives it through `kanban_show/complete/block/heartbeat/comment/create/link/...` tools.
  - Features: a review state (`kanban_request_review`, `kanban_request_changes`), a consecutive-failure circuit breaker, a checkpoint notice at about 90% of the iteration budget (`agent.budget_warning_ratio`), `completion_contract OWNER/REPO` binding a task to a PR, and worker lanes that can wrap Codex or Claude Code (docs-only).
  - Maturity: large and actively developed.
- **Delegation** (`tools/delegate_tool.py` plus ten siblings). Config: `max_concurrent_children` 10, `max_spawn_depth` 1 (2 gives orchestrator to leaf), `child_timeout_seconds`, per-delegation `model` and `provider`, and async background delegation. The parent sees only the summary. Mature.
- **Approval model.** `approvals.mode` defaults to `smart`. `tools/approval_detection.py` normalises and tokenises commands against pattern tables. It protects `~/.ssh`, `~/.hermes/.env` and `config.yaml`, since a config write would change the security policy mid-session. The `approval_smart.py` guardian LLM is an `auxiliary.approval` task. Unattended surfaces use `deny` rather than blocking for 300s. Mature.
- **Checkpoints** (`tools/checkpoint_manager.py`). One shared shadow git store, per-project refs, snapshot once per turn before mutating ops, and rollback. Opt-in via the `checkpoints` config flag. Mature.
- **Context compression.** An auxiliary model summarises the middle turns. It has a "lean" tail mode and a `prepare_ahead` option that writes the next summary between turns, plus `checkpoint_required`, which fails closed unless the memory provider checkpointed first.
- **Cron** (`cron/`, about 50 modules). Natural-language jobs, delivery targets, missed-run handling, a quota hold, and a `cron_mode: deny` approval default.
- **Gateways and backends.** About 25 chat platforms, an OpenAI-compatible API server (`gateway/platforms/api_server*.py`) and the seven terminal backends above. Mature.
- **Auxiliary models.** `config_defaults.py` has an `auxiliary` block of per-task model routes (compression, approval, review, mcp, title_generation, skills_hub, background_review), each with provider and model, so cheap tasks go to cheap models. This maps directly onto your worker-seat and usage-pool idea.
- **Credential pool and cost.** `agent/credential_pool*.py` includes model cooldowns and a rate-limit tracker; `account_usage.py`, `insights.py` and `usage_pricing` produce token and cost reports.
- **Plugins.** Python plugins in `plugins/` and a `plugin-catalog/*.yaml` listing hundreds of entries, with a shell-hook layer (`agent/shell_hooks.py`) and plugin hooks such as `on_session_end`. Large, mature.
- **ACP adapter.** `acp_adapter/` and `agent/acp_client.py` run Hermes as an Agent Client Protocol server.

## 4. Claims I could not verify

- **Honcho's dialectic user modelling**: the engine is external (Plastic Labs), so only the docs page and the in-repo clone helper were seen.
- **Kanban lanes wrapping Codex or Claude Code**: this is described in docs and I did not trace dispatcher code.
- **Whether the review fork's whitelist is airtight.** I read `_review_tool_whitelist`, but not the plugin-level `set_thread_tool_whitelist` enforcement.
- **How much of the approval flow is covered by tests.** The repo has tests I did not run (and was told not to).
- **Runtime behaviour.** Nothing was executed, so default values come from `config_defaults.py` and docs, and could differ from a shipped build.
- **Agent-created profiles.** I did not find a profile-creation tool for the agent (possibly via the dashboard or Bot Mode).
- **Memory-provider backends** (hindsight, mem0, supermemory and so on) were not read; I saw only the ABC.

## Takeaways for a herdr port

1. **Profiles are whole homes, not records.** Hermes gets isolation by making `HERMES_HOME` a ContextVar plus an env var. This maps to your per-account pools and seats, and to the shared board versus private memory split, and the docs warning about two agents on one home matches your shared-writer incidents.
2. **The learning loop is small.** Counters, one forked agent with a tool whitelist, two char-capped files, one skill tool and a weekly prune. The valuable part is the review prompt plus the read-before-write guard, the anti-duplicate rules ("same lesson learned twice is ONE rule") and the "do not capture" block.
3. **Hermes does not measure whether a learned skill helped.** If you port it, that is the gap worth filling.