# Review of P40e SHELL, c40e-shell 78147aa1 against 86b37c7e

Ran: `python -B -m unittest nirvana.test_scripts nirvana.test_windows_shell` (108 tests OK). Probe in a private folder: on
this machine `os.rename` of a folder with a file inside is refused (winerror 5) with share modes read+write and
read+write+delete alike, so the `clean_temp.py` claim "Windows refuses the rename while a file is open" holds for Rust
handles too. Did not run cargo, controls (not even `--dry-run`), or `--apply`.

## S (must fix before merge)

**S1. `remove_tree` follows a link at its top; two callers do not re-check.** `nirvana/clean_temp.py:111` (`walk` starts
with `os.scandir(top)`, docstring: "`top` itself must not be a link (the caller checks)").
- `clean_temp.apply` (`clean_temp.py:~211`) checks the link only in `scan`, which with `sizes=True` walks every folder
  first (35,326 folders: minutes). Between the scan and `os.rename(item.path, tomb)` a name can be swapped for a junction
  (rename then renames the junction, `remove_tree(tomb)` descends through it and empties the target).
- `gate.py:~141` `finally: clean_temp.remove_tree(private)` never checks. `private.mkdir(exist_ok=True)` accepts an
  existing `%LOCALAPPDATA%\Temp\ng-<pid>` that is a junction (pre-planted, or a leftover) and the `finally` then empties
  whatever it points at.
- Fix: first lines of `remove_tree`: `if is_link(top): failures.append(f"{top}: a link, not followed"); return failures`.
  In `gate.run`, refuse (exit 2) when `private` exists and `clean_temp.is_link(private)`, or take the folder with
  `tempfile.mkdtemp(prefix=f"ng-{pid}-", dir=base)` (then widen `PRIVATE` to match). Add a test per caller and a control.

**S2. Two live control runs in one worktree still undo each other, and the new repair hides it.** `nirvana/controls.py`
`run_one` (`~1167`) and `_save_original` (`~1102`). `repair()` skips a note whose pid lives, but nothing stops a second
`controls.py` from starting. Trace: run A sabotages F (control X); run B (control Y, same file) reads F as `original`,
which is A's sabotaged bytes (the find text occurs once if Y's text is untouched by X), saves them as "the original",
sabotages again. A finishes first and restores the true bytes; B's `finally` then writes A's sabotaged bytes back and
deletes its own note. Result: F stays sabotaged, no note, no repair, and `c40e_clean_*` controls (several on
`clean_temp.py`, the case the `-B` comment describes) are exactly the same-file pairs. Same-name runs also share
`<name>.bak`/`.json`. This is not new, but B6's stated aim is that a leftover is always put back, and this leaves one
silently. Fix: a per-worktree run lock in `RESTORE` (`os.open(lock, O_CREAT|O_EXCL)`, pid inside, stale when its pid is
dead) taken in `main` before `repair()` and released in `finally`; a second run exits 2 with "another controls run
(pid N) is live in this worktree". Test: start two, the second refuses.

## N (record or fix, not blocking)

**N1. `--dry-run` writes.** `controls.py` `main` calls `repair()` before the `--dry-run` branch, so a dry run can rewrite
a source file. Right behaviour, wrong flag name; say it in `--help` or skip repair when `--dry-run`.

**N2. gate: interrupt leaves grandchildren.** `gate.py` `run_child` uses `subprocess.run` with no timeout; on
KeyboardInterrupt only `cargo` is killed, `cargo-nextest` and test binaries (grandchildren) keep running with TEMP set to
a folder the `finally` then empties (they fail noisily, nothing outside `ng-<pid>` is touched). A console Ctrl-C reaches
them all; a killed parent (no `finally` at all) leaves `ng-<pid>` and orphan children that keep running; the 24 h sweep
then removes it once the gate's pid is gone, even if an orphan still uses it. Fix:
a Job object / `taskkill /T /F` of the child tree on interrupt, and a `timeout=` on the step.

**N3. gate: `ALLOWED` match by last segment** (`gate.py` `judge`): a same-named test in another module would be hidden.
Match the full `...::windows_plugin_pane_...` path suffix `tests::name` of the one real module instead.

**N4. clean_temp cases traced, all fail closed.**
- Names come from `os.scandir` of the root: no `..`, no separator can appear.
- Top-level link or junction: skipped (`scan`, `is_link` covers reparse points). Nested: removed as a link by `rmdir`/`unlink`,
  never entered. A cloud-placeholder reparse file is treated as a link and unlinked: fine.
- Pid reused: `pid_alive` true, kept. Access denied (5) counts as alive. Pid not running, nonexistent: removable.
- Clock set back: `age` negative, `< limit`, kept. Clock forward only makes folders older.
- Rename fails (cwd of a process, an open file): "skipped ... in use", exit stays 0; remove failure: "failed ...", exit 1;
  both continue to the next folder (it does not stop on the first error).
- Tombstone: `<name>.cleaning-<mypid>` matches `NAME` with the original pid, so a later run finds it. A half-emptied
  tombstone has a fresh mtime (deleting children touches it) so it waits another 48 h and is invisible under its old name
  meanwhile; harmless for test folders, say so in the docstring.
- TEMP unset or odd: `default_root` takes TEMP, TMP, then `tempfile.gettempdir()`; `--root` takes anything. Only children
  named `herdr-`/`nirvana-` (case sensitive) ending in a pid, old and dead are touched, so a drive root or the profile
  loses nothing else. A folder `nirvana-notes-2024` in a root would match (pid 2024). Suggest refusing a root whose name
  is not `Temp`/`tmp` unless `--root` is given with `--apply --i-mean-it`; optional.

**N5. Installer.** `windows_shell._replace_tree`:
- Prune stays inside `dst` (`dst.rglob`, paths relative to `dst`), so nothing outside the plugin tree; anything the owner
  put inside `dst` is deleted, as the old `rmtree` did. The owner's files beside it are safe except a sibling named
  `claude.new-*` or `claude.old-*`, which the opening glob removes (`windows_shell.py:~139`); `claude.old-2025` by hand would go.
  Narrow the glob to `\d+` suffixes.
- A junction placed inside `dst` is descended by `rglob` on Python before 3.13 and its non-listed files are unlinked
  (`is_symlink` is False for a junction). Use `os.scandir` with `is_link` as `clean_temp` does.
- Overlay branch: a `copy2` that fails after truncating (disk full) leaves a corrupt file, and a failure midway leaves old
  and new files mixed, so "complete plugin folder" holds only in the rename branch. `herdr.exe` is always present after a
  single failure (restored by `install_binary`'s `except BaseException`); a power cut between the rename and the copy is not covered.
- Tests: `test_a_folder_that_cannot_be_renamed_is_overlaid_in_place` passes with the whole fix removed (old
  `rmtree`+`copytree` never calls `os.replace`); only `c40e_tree_overlay_prunes` bites, on the prune. The midway-copy test
  does fail without the fix (old code deletes `dst` first).

**N6. `sweep_partials`.** Only `<uploads>/<name>.partial` where `is_upload_name(name)`, a plain file, mtime over 3600 s.
An upload is cut at 20 min (`http::BODY_MOST`), so even a directory-entry mtime that was never refreshed (NTFS) is the
creation time, 20 min < 60 min: a live one is safe. Gap: `.nirvana` itself may be a junction (`plain_dir` checks only
`uploads`), where `uploads_dir` refuses it; check both.

**N7. Phone note** (`send.rs` `note_for`, `header_size`). Every piece is the server's: extension from the server name,
`metadata().len()`, header numbers. Dimensions are `u32` printed as digits (no negative, no text); a huge or zero value
only prints. No panic: every read is `get`/`try_into` and the JPEG loop strictly advances. Read bounded at 128 KiB, no
time bound (local file, `metadata().is_file()` first). Facts I checked: PNG/GIF/WebP VP8, VP8L, VP8X offsets and the
JPEG SOF width/height order are right. Concerns:
- `herdr look` is not in this branch or master (`grep` finds it only in `send.rs` and tests). The note names a command
  that exists only after c40c merges: merge c40c first, or c40e ships a note that sends agents to a missing command.
- `live.rs:409` uses the note for every agent kind. Codex or agy (both read images natively) are told to shell out to
  `herdr look`; a plain shell pane would get a syntax error from `(`, `;`, `<`. If the kind is known at that point, add
  the note for Nirvana Claude only.
- `a_zero_byte_upload` refuses every empty file, not only images; an empty `.txt` from the phone now errors. Say so.

**N8. Labels.** Consumers of `compact_model`/`short_model`/`format_model` outside the two modules: `agent_sidebar.rs:113,733`
and `client/shell/nirvana_spaces.rs:188` only; no phone JS reads a model name (`grep Opus|Sonnet|Flash` in
`nirvana_remote/page/*.js`: none). `Antigr` is not matched anywhere else. `Agy` was not asked for (the order said only "not cut to
`Antigr`"); it is a new visible string, record it in the CHANGELOG as a choice. `provider_of` makes agy+Claude id a Claude
row, which changes which usage pool a sidebar row is drawn under only for display (the CLI still names the pool).

**N9. controls.** `c40e_controls_*` and `-B` look right. `_pid_alive` in `controls.py` returns False on access denied
while `clean_temp.pid_alive` returns True; use one function.
