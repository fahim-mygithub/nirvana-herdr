# Superpowers off on every Nirvana Claude start, Sonnet included

Built 2026-11-04 on `no-superpowers`, off master 70c88735 (alpha 0.54.0). It supersedes plan 28's Sonnet keep
(`2026-10-23-28-superpowers-by-model.md`). The owner, 2026-11-04: "I would take out superpowers it doesnt feel
needed", and, asked where: "Everywhere". The orchestrator set both `superpowers@superpowers-marketplace` and
`superpowers@claude-plugins-official` to false in the owner's `~/.claude/settings.json`. This package does not touch
that file and did not read it.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "I would take out superpowers it doesnt feel needed" | the rule, the Preload default | orchestrator | the tests below |
| L2 | "Everywhere" | every Claude start Nirvana makes | orchestrator | the tests below |

## What Nirvana starts, and what each start now gets

| Start | Path | Before (plan 28) | Now |
|---|---|---|---|
| New Nirvana, Fresh, the verb's space | `app/nirvana_new.rs` -> `nirvana_launch::launch_args` | superpowers off on Opus, on as the user says on Sonnet | off on every model |
| Restart Claude | `app/nirvana_restart.rs` -> `launch_args` | same | off on every model |
| A restored Nirvana pane | `persist/restore.rs` marks `nirvana_relaunch` -> `app/nirvana_relaunch.rs` -> `nirvana_new` -> `launch_args` | same | off on every model |
| A Claude worker (`seat exec`) | `nirvana_worker::worker_argv` -> `worker_settings` | off on Opus, preset as is on Sonnet | off on every model |
| An agy worker | same | untouched: agy reads no Claude settings | untouched |

Left as they are, on purpose:

- The `/context` probes of Preload (`nirvana_preload.rs`, `nirvana_preset.rs`, `CONTEXT_ARGS`). They measure what a
  folder loads, for the panel. Turning superpowers off there would make the panel lie about the user's own setup.
- The `/usage` read (`nirvana_usage.rs`, `CLAUDE_USAGE_ARGS`). It is answered locally, 0 tokens and 0 turns (plan 12
  §16), so no plugin is loaded into a model turn.
- Plain `claude --resume` of a non-Nirvana pane (`agent_resume.rs`). That is the owner's own pane, which plan 28 left
  out of scope. The owner's settings now turn superpowers off there anyway.

## The rule

- `nirvana_superpowers::entries(ids)` turns every superpowers id off. It no longer takes a model.
- `launch_settings_in` now returns `Result<PathBuf, String>`, not `Option`: there is always a launch file. An error
  still keeps today's argv and logs one line.
- `worker_settings_in` asks only whether the worker is `claude`.
- The ids are unchanged: every installed or enabled plugin whose name before `@` is `superpowers`, read from Claude's
  `plugins/installed_plugins.json`, the folder's `.claude/settings.json` and `settings.local.json`, and the file
  passed.
- The mechanism stays as it was: a generated `--settings` file under `<home>/nirvana/launch/`. So a user who turns
  superpowers back on in their own settings still gets it off in Nirvana.

**Does `Family` earn its place?** No, so it was removed: `Family`, `Family::word` and `family()`. The rule was its
only reader, and its only job was the Sonnet keep. The file names lose the family too:

- `<hash>.settings.json`, was `<hash>.opus.settings.json`
- `<hash>.worker.settings.json`, was `<hash>.opus.worker.settings.json`

An installed home keeps its old `.opus.` files until someone deletes them. Nothing writes or reads them any more.

## Preload

- `src/nirvana_preload.toml` `core` is now `["nirvana", "bladebro", "context7"]`.
- The default as shipped at 70c88735 is kept as `src/nirvana_preload_v3.toml`, the third entry in
  `nirvana_preset::HISTORICAL_DEFAULTS`.
- How the upgrade works (`nirvana_preset::load_config`): an installed `preload.toml` equal to a past default (line
  endings aside) is copied to `backup/preload.toml` and replaced by `DEFAULT`. An edited file is never touched.
- So an installed v3 loses `superpowers` from `core`. An owner who edited the file keeps their own `core`.
- Measured in the tests:
  - `superpowers` is no longer core, so it is untagged, and the catalog's seeding now leaves it for Jev to tag like
    any other plugin (`the_name_rules_seed_the_catalog_for_free_and_leave_core_and_the_rest_to_jev`). That puts one
    more item in a background tag batch of up to 16; it adds a call only when it tips a batch over.
  - Once tagged, a preset that does not keep its tag switches it off in the preset's file too.

## Other places that name superpowers

None of them states the old rule as something to follow, so none was changed except as listed.

| Place | What it is | Changed? |
|---|---|---|
| `src/nirvana_lean.rs` module doc, "The superpowers duplicate off (136) ... neither is taken here" | plan 25's measurement record | no: a record, and still true of plan 25 |
| `src/nirvana_preload.rs`, `src/nirvana_preset/tests.rs`, `src/client/shell/tests/nirvana_mods.rs` | fixtures that use superpowers as an example plugin in a `/context` reading | no: test data about the panel, not the rule |
| `nirvana/CHANGELOG.md` "alpha 0.39.0: superpowers only on Sonnet" | the history of plan 28 | no: the merge writes this change's entry |
| plans 13, 25, 31, 33 map, 35 | records | no |
| ROLE.md, skills/, AGENTS.md, the plugin's files | grep finds no "superpowers" | nothing to change |
| `nirvana/plans/2026-10-23-28-superpowers-by-model.md` status line | the plan | marked superseded, with the owner's quote |

## Controls

New controls go in `nirvana/controls.d/nosp.py`, 24 of them.

Retired from `superpowers_by_model.py`, 11 controls, because they stated the Sonnet keep or the families it needed:

- `p28_opus_not_opus`, `p28_sonnet_not_sonnet`, `p28_label_case_kept`, `p28_sonnet_gets_entries`
- `p28_launch_file_for_sonnet`, `p28_launch_path_no_family`
- `p28_worker_sonnet_touched`, `p28_launch_fixed_model`, `p28_restart_ignores_mode`, `p28_sonnet_launch_gets_file`,
  `p28_worker_fixed_model`

Repointed to `nosp_` names, 15 controls, because their line or their test moved. `nosp.py` names the old control
beside each new one.

Kept as written in `superpowers_by_model.py`: 18 controls on the ids, the merge, `claude_dir`, the base,
the error line and the temp name.

## Where the order was wrong or unsure

- "Restart Claude" and "the verb's space" are not separate paths in the code. Both reach `launch_args`, as do New
  Nirvana and a restored pane. The fix lives in one place for Nirvana Claude and one for workers.
- Claude's installed list on this machine has three more `superpowers-*` plugins:
  - `superpowers-chrome@superpowers-marketplace`
  - `superpowers-developing-for-claude-code@superpowers-marketplace`
  - `superpowers-lab@superpowers-marketplace`

  Only the names were read. The rule matches the name `superpowers` exactly, as plan 28 did ("superpowers-lab... are
  other plugins"), so these are not turned off. Whether "Everywhere" covers them is the owner's call.
