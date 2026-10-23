# Feature 28: superpowers only on Sonnet

Status: **PLANNED (2026-10-23)**, to build on a Claude seat (Opus high) at master alpha 0.38.0, beside plan 27.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Another caviat im noticing is that agents perform poorer with superpowers attached to opus. Given the preload cost of enabling superpowers if you are using opus level model for planning and building features, do not preload supoerpowers." | §2, §3 | orchestrator | §2 and §3 tests |
| L2 | "Nirvana in theory on startup should be using opus medium." | §1 | orchestrator | §1 measurement |
| L3 | "If were constrained on tokens and have to start planning and building with a sonnet levle model, superpowers seems to provide osme improvements." | §2 | orchestrator | §2 tests |
| L4 | "Create this rule / login on Nirvana." | §2, §3 | orchestrator | §2 and §3 tests |

The scope, stated to the owner when the order was written: the rule covers Claude sessions Nirvana starts, meaning
Nirvana Claude and Claude workers. The owner's own plain Claude panes keep whatever Preload says.

## 1. Measured 2026-10-23, on master at alpha 0.38.0

The builder re-measures every claim before relying on it.

- **Nirvana Claude starts on Opus medium by default.**
  - `nirvana_seats.toml` sets `mode = "balanced"`.
  - `orchestrator_seat` (`src/nirvana_seat.rs`) gives opus at medium effort in Balanced when readings are missing or
    stale. Efficient gives sonnet medium, and Sota gives opus high.
  - The fallback in `launch_args` is `claude-opus-5-5` at medium.
- **Its argv:** `nirvana_claude_args` and `launch_args` (`src/nirvana_launch.rs`) pass exactly one `--settings`.
  - That is the plugin's `nirvana/claude/settings.json`, or the folder's generated preset file when one exists
    (`<home>/nirvana/preload/<hash>.settings.json`, `nirvana_preset.rs`).
  - A test asserts there is one `--settings`.
  - Restart Claude (`src/app/nirvana_restart.rs`) reuses `launch_args` and re-reads the mode.
- **Workers:** `argv` in `src/nirvana_worker.rs` passes `--model <spec.model>` and `--setting-sources project`.
  `with_preset_settings` adds `--settings <worker preset file>` for Claude workers only, and only when the folder has
  one.
- **Plugins:** Preload turns a plugin off with `enabledPlugins: {"<id>": false}` (`nirvana_preload.rs`).
  - `nirvana_preload.toml` lists `superpowers` under `core`, which "never switched off".
  - The id seen in fixtures is `superpowers@superpowers-marketplace`. Plan 25 found it enabled twice, from two
    marketplaces, so match on the name before the `@`.
- **No rule is keyed by model today.**
- **Cost:** plan 25 estimated about 1.1k tokens for the using-superpowers hook text, plus its share of the skill listing
  (about 7.0k for 63 skills). These are char/4 estimates.

## 2. The rule

- One function decides from a model id or label:
  - **Opus family:** every `superpowers` plugin off.
  - **Sonnet family:** no entry, so superpowers loads as the user's settings say.
  - **Any other model** (Haiku, Fable, unknown): no entry. Nothing changes.

  A table test covers `claude-opus-5-5`, `opus`, `Claude Opus 5.5`, `claude-sonnet-5-5`, `sonnet`, `claude-haiku-5-5`,
  `claude-fable-5-1` and an empty string.
- The plugin ids come from every installed or enabled plugin whose name before `@` is `superpowers`.
  - Read the names from Claude's `~/.claude/plugins/installed_plugins.json` and from the folder's settings.
  - Read plugin names only, never anything else in those files.
  - When neither lists one, use `superpowers@superpowers-marketplace`.

  In tests, these reads take parameters.
- The rule outranks Preload's `core` for these launches only. The Preload panel and the owner's own panes are
  unchanged.

## 3. Where it applies

- **Nirvana Claude:** after `launch_args` knows the seat's model, the one `--settings` file it passes carries the
  rule's `enabledPlugins` entries.
  - Write a launch settings file: the file it would have passed, merged with the rule's entries. The merge keeps every
    other key.
  - Put it at `<home>/nirvana/launch/<folder hash>.<family>.settings.json`.
  - The argv still has exactly one `--settings`.
  - New Nirvana, the space launch and Restart Claude all go through `launch_args`. Each picks up the rule from its own
    model.
- **Claude workers:** `with_preset_settings` does the same from `spec.model`.
  - A worker with no preset file gets a settings file holding the rule's entries alone, when the rule has entries.
  - Re-measure whether a worker loads superpowers at all under `--setting-sources project`. Report it. The rule
    applies either way, since a `--settings` file always applies.
- An error building the file falls back to today's argv, and says so in one log line. A launch never fails because of
  this rule.

## 4. Controls and gates

- Every assertion in §2 and §3 has a sabotage control in `nirvana/controls.d/superpowers_by_model.py`, named `p28_*`,
  on a product line, naming the test that goes red.
- Every older control whose target line changes is updated and still reddens its test.
- Tests use `crate::nirvana::with_test_home(name)` and temporary folders only. No test reads the real
  `~/.claude`.

## 5. Live check, for the owner

1. Start Nirvana in Balanced mode. Its `/context` lists no superpowers skills.
2. Switch the project to Efficient (sonnet) and press Restart Claude. Superpowers is back.
3. An Opus worker's stream shows no superpowers skill.

## 6. Not in this plan

- The owner's own Claude panes and the Preload panel.
- Other plugins.
