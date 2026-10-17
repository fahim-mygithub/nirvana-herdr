# Feature 10: the Jev memory, its own file, sized to the project

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "So it seems like the repo has to be quite large to get token efficiency gains maybe just have a tool flag in memory." | a project under `min_project_kb` gets one line, the tool flag; at or over it, the whole memory | orchestrator | `nirvana_jev_memory::tests::a_small_project_gets_only_the_flag_and_a_big_one_everything`; live on 70 KB, 555 KB and 44 MB projects |
| L2 | "I wawnt to also ensure I can toggle this so maybe have it a seperate file." | `<home>/jev/JEV.md`; Preload's "Jev memory" row switches it per folder, and `e` opens the file | orchestrator | `the_file_is_written_once_and_then_kept`; `each_kind_of_item_gets_its_proven_switch`; live in a herdr window |
| L3 | "This Jev memory preload file will give it instructions on how to make Jev tool chain handoffs and at what sizes it would make sence to invoke it." | the file's body: "When to ask it" with the sizes, and "Handoffs"; the sizes are front matter the body names | orchestrator | `the_sizes_come_from_the_file_the_owner_edits`; live: an edit to the file changed what the next agent got |

Background: two single runs (2026-10-16 and 17, `nirvana/evidence/2026-10-16-jev-ab/single-results.jsonl`) on the
568 KB project saved nothing: the shipped guide used Jev three times for +8% tokens; a "search first" wording used it
never, for a noise swing of +17%.

## Shape

- **The file** is `<home>/jev/JEV.md`, written from herdr's `DEFAULT` when missing, the owner's from then on. Its
  front matter holds `min_project_kb`, `read_file_kb`, `ask_file_kb`, `files_kb`, `output_lines`; the body names them
  as `{name}`, so a size lives in one place.
- **The project's size** is what git tracks, or outside git a walk that skips Jev's excluded folders; binaries by
  extension do not count. Measured at most once a minute per folder, since Preload draws often.
- **What reaches the agents**: Claude's `SessionStart` hook prints it for the hook's folder (once, as before, when
  Claude already reads `AGENTS.md`); Codex and agy get the same text in the `AGENTS.md` block, written at the switch.
- **The sizes are hypotheses**: no A/B has been run above 568 KB. `min_project_kb: 1024` sits above both projects
  measured.

## Files

- `src/nirvana_jev_memory.rs` (new): the file, front matter, the size, `render`, `for_folder`, `detail`.
- `src/nirvana_jev.rs`: the built-in guide removed; `guide` prints the memory; `on|off memory`; `excluded_dir`.
- `src/nirvana_jev_install.rs`: the block and `guide_tokens` from the memory, per folder.
- `src/nirvana_preload.rs`, `src/client/shell/nirvana_mods.rs`: the row "Jev memory", its detail, file and messages.
- `src/main.rs`; tests beside each; `nirvana/controls.py`; `nirvana/CHANGELOG.md`.

## Record

Built 2026-10-17 on alpha 0.9.0 (909ca191) as alpha 0.10.0.

1. **The gate** (`herdr jev guide --cwd`): 70 KB and 555 KB copies got the flag; this repository, 44.2 MB in 3,117
   text files, got the whole memory with its largest three.
2. **The owner's edit**: `min_project_kb: 400` and `ask_file_kb: 30` in `JEV.md` gave the 555 KB copy the whole
   memory with "over 30 KB".
3. **Real agents**: Claude Haiku quoted the flag in the 555 KB copy and the project line in a 1.6 MB copy, through
   `AGENTS.md`; with a `CLAUDE.md` beside it, through the `SessionStart` hook. agy 1.4.0 quoted the project line.
   Flag against full: 41,922 against 42,405 input tokens.
4. **In a herdr window** (scratch home): the row "Jev memory", "JEV.md: a flag, 555 K...", 179 tokens, `edit`; the
   switch wrote the hook and a git-ignored `AGENTS.md`; `edit` opened "JEV.md - edit" holding the file.
5. 7 new controls redden their tests; 165 controls find their lines; the suite passes but the one upstream test that
   failed before.

**Not measured.** Whether the memory saves tokens above 1024 KB: no A/B there yet. Codex (its sign-in is dead on this
PC). The `AGENTS.md` block keeps the project size from the moment of the switch, so it goes stale as the project
grows; switching off and on writes it again.
