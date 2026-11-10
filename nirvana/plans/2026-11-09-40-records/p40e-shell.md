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
| S1 | "`remove_tree` must refuse a link at its own top at the moment it deletes" | `clean_temp.{remove_tree, apply}`, `gate.{run, sweep_stale}` | `CleanTempTest`, `GateTest` junction tests |
| S2 | "one `controls.py` run per worktree at a time" | `controls.{take_lock, release_lock, main}` | `ControlsRepairTest.test_a_second_run_in_the_worktree_is_refused_and_touches_nothing` |
| N2 | "ends its whole child tree before it removes the private TEMP" | `gate.{_Tree, run_child}` | `GateTest.test_an_interrupt_ends_the_whole_tree_before_the_temp_goes`, `test_a_killed_gate_takes_its_tree_with_it` |
| N5 | "the install reports which files are stale and exits non-zero after finishing the copy" | `windows_shell.{_overlay, StaleFiles, install_plugin}`, `install.main` | `ReplaceTreeTest`, `InstallTest.test_a_stale_plugin_folder_finishes_the_install_and_exits_non_zero` |
| scratch | "make `scratch()` in `src/nirvana_preset/tests.rs` stop leaving a folder per test" | `nirvana_preset::tests::scratch` | the folder count, 52 before and 0 after |

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
- `clean_temp.py` first followed the order's name rule strictly: a name must end in a pid or a pid and a timestamp.
  The two largest creators do not. The review round added both as named rules: old Nirvana's `ui/dev/fake-daemon.py`
  (`nirvana-fake-daemon-` and `mkdtemp`'s 8-character suffix, no pid: age, link, in-use and top-level rules only) and
  `src/nirvana_preset/tests.rs scratch()` (`nirvana-p13a-<pid>-<count>-<name>`, the pid rule too).
- `herdr remote uploads` is not built; recorded here as the next step for D1.

## Review round (`pkg-review-e.md`)

- S1: `remove_tree` tests for a link on every folder right before it lists it, so a top that became a junction is
  refused where the delete is; `clean_temp.apply` asks again before the rename and after it (a link renamed by mistake
  is renamed back). `gate.py` takes its private TEMP with `tempfile.mkdtemp` (`ng-<pid>-<random>`), so its `finally`
  only ever removes a folder that run made. Tests make real junctions with `mklink /J`. A race of milliseconds is
  left between the last test and `os.scandir`; closing it needs a handle opened with `FILE_FLAG_OPEN_REPARSE_POINT`.
- S2: one `controls.py` run per worktree, by an OS lock on `target/controls-restore/run.lock` taken before the repair.
  Windows drops the lock when its holder dies however it dies, so a dead holder's lock is free by itself;
  `run.lock.info` names the holder for a refused run and is reported as taken over. The retarget script and the tests
  call `run_one` directly and take no lock.
- N2: each gate step runs in a kill-on-close job object, started suspended and resumed once inside it; an interrupt
  ends the job before the `finally`, and a killed gate's job ends with its last handle. A step times out at 90 min.
- N5: the overlay copies each file beside its target and renames it over, never enters a link, and raises
  `StaleFiles` naming each file it could not replace or remove, after the rest is down; `install.py` goes on and exits
  1 with the list. Only `claude.new-<digits>` and `claude.old-<digits>` siblings are removed.
- N7: the phone note keeps the path first for every agent kind and names the command as "in a Nirvana pane, herdr
  look ...". `herdr look` exists only once c40c merges: merge c40c first.
- N8: the order asked only that a bare Antigravity is not cut to `Antigr`; `Agy` is this package's choice of word,
  for the CHANGELOG at the merge.
- N1, N3, N6, N9 fixed (help text, the allowed test by its path tail, `.nirvana` junction refused by the sweep, one
  `pid_alive`). N4's optional refusal of an odd `--root` is not built.
- `scratch()` removes its folders when the test's thread ends (a thread-local's destructor): measured with a private
  TEMP, one run of the `nirvana_preset` tests left 52 `nirvana-p13a-*` folders before and 0 after.
