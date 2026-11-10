# Plan 40 record: shell and tools fixes (`c40e`)

Built 2026-11-09 on master `86b37c7e` (alpha 0.62.0), branch `c40e-shell`, in parallel with the seat, worker, memory
and media builders. A first builder was stopped by a usage limit about 13 minutes in; a second one checked its
uncommitted Rust edits, finished them and built the Python side. Not merged; `nirvana/VERSION` is not bumped here.
Controls: `nirvana/controls.d/c40e.py`, prefix `c40e_`.

## Ledger

| Item | Verbatim quote (order) | Surface | Verifier |
|---|---|---|---|
| B1 M2 M3 D4 | "the provider and label come from the model id, whatever CLI runs it; one function decides the family" | `nirvana_spaces::{bare_model, model_family, compact_model, antigravity_model}`, `nirvana_subagents::{format_model, gemini_label, provider_of, seated_label}` | `model_label_table_reads_every_id_right`, `labels_come_from_the_model_id_whatever_cli_runs_it`, `a_seated_claude_order_through_agy_reads_as_claude`, `compact_model_table_test` |
| B2 | "a failed font download does not stop the install" | `windows_shell.install_all` | `FontFailureTest` |
| B3 | "the plugin folder is replaced without a moment where it is half there" | `windows_shell._replace_tree` | `ReplaceTreeTest` |
| B4 | "a failed binary copy puts the old binary back" | `install.install_binary` | `InstallTest.test_a_failed_copy_puts_the_old_binary_back` |
| B5 | "a zero-byte upload is refused with \"the file is empty\" and leaves no file" | `send::save` | `a_zero_byte_upload_is_refused_and_leaves_no_file` |
| B6 | "keeps the original bytes of a file it is sabotaging in a git-ignored restore folder and repairs any leftover at the start of the next run" | `controls.{_save_original, repair, main}`, `target/controls-restore/` | `ControlsRepairTest` |
| B7 | "the Running square drops a tool name when its transcript can no longer be read" | `nirvana_running::settle` | `running_drops_the_tool_when_the_transcript_cannot_be_read` |
| B8 | "`herdr_self` drains stdout while it waits" | `nirvana_subagents::stdout_within` | `a_child_answer_larger_than_a_pipe_is_read_whole` |
| D3 | "one table for the phone page's served files" | `routes::ASSETS` (pick.rs match and `shell::PICK_SCRIPTS` gone) | `the_page_has_the_cards_and_the_new_space_sheet`, the offline shell tests |
| D1 | "`.partial` upload leftovers older than an hour are removed by the existing sweep. Finished uploads are the owner's files: never delete them." | `nirvana_dead::sweep_partials`, called from `sweep` | `the_sweep_removes_old_upload_partials_and_never_a_finished_upload` |
| X | "For an image ... add, after its path, one short note in parentheses" | `send::{message, note_for, header_size}` | `a_sent_photo_carries_a_note_naming_herdr_look`, `the_note_comes_from_the_servers_name_and_bytes_not_the_phones_text`, `image_headers_give_their_size_cheaply` |
| gate | "`nirvana/gate.py`: the one way to run the scoped gates" | `nirvana/gate.py` | `GateTest` |
| clean | "`nirvana/clean_temp.py`: removes the backlog. Default is a dry run" | `nirvana/clean_temp.py` | `CleanTempTest` |

## Decisions

- The bare Antigravity chip reads `Agy`, whole, where it was the cut `Antigr`.
- The B6 restore folder is `target/controls-restore/`: `target/` is git-ignored per worktree, so no `.gitignore` edit.
  A repair puts the bytes back only when the file still holds the sabotage (its sha256 is in the note); a file edited
  after the kill is left, and its backup kept, for a person.
- `controls.py` runs Python tests with `-B`: two controls on `clean_temp.py` whose replacements were the same size ran
  within one second and the second ran the first's bytecode (it read green with its sabotage never loaded).
- `clean_temp.py` renames a folder to `<name>.cleaning-<pid>` before removing it, a stronger form of the order's
  "rename-to-self test": Windows refuses either rename while a file inside is open, and the tombstone means a folder
  is never half removed under its own name. It also skips a folder whose pid still runs.
- `clean_temp.py` follows the order's name rule strictly: a name must end in a pid or a pid and a timestamp. The two
  largest creators do not: old Nirvana's `ui/dev/fake-daemon.py` (`tempfile.mkdtemp`, a random suffix) and
  `src/nirvana_preset/tests.rs scratch()` (`nirvana-p13a-<pid>-<count>-<name>`). The dry run lists them as kept.
- `herdr remote uploads` is not built; recorded here as the next step for D1.
