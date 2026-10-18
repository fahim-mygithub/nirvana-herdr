# Nirvana-Herdr changelog

Each entry: Changed → Because → Measured → If you remove this. herdr's own changes are in the root CHANGELOG.md,
which the fork never edits.

## alpha 0.16.0 (2026-10-18): the runner, `herdr seat run|exec|status|read|stop|cleanup`, the journal, waking, the run receipt (plan 12 O3)

Plan 12, package O3 (`plans/2026-10-17-12-nirvana-claude-orchestrator.md` §4, §5.6, §7.1, §7.2, §7.4.1; the record is
`plans/2026-10-17-12-records/o3.md`).

- **Changed:** a run's journal, `<project>/.nirvana/runs/<run>/journal.jsonl`: append-only, one JSON event a line with
  `v`, `seq` and `at`, every writer under an OS file lock on `journal.lock` in the run's folder (released when its
  holder ends, however it ends), so the orchestrator's `seat run`, each
  worker's supervisor and `run wake` never interleave a line. Events: opened, seated, started, usage, rate_limited,
  checkpoint, deferred, reseated, delivered, gated, merged, asked, woken, stalled, interrupted, ended. Replaying it
  rebuilds the run; a kind a later herdr writes is skipped, a line that does not parse is counted and kept. herdr writes
  `.nirvana/.gitignore` (runs, orders, worktrees; not lessons) when it is missing.
  **Because:** plan 12 §7.2: "replaying the journal rebuilds the run", and the owner, 2026-10-17: "longer running
  agentic workflows" (L1). Nothing about a run may live in a process that a crash or a reload of herdr's server ends.
  **Measured:** `replaying_the_journal_rebuilds_the_run`, `a_line_that_does_not_parse_is_counted_kept_and_numbered`,
  `appends_from_many_threads_lose_no_line` (8 threads, 40 lines, seq 1 to 40); controls `run_replay_*`, `run_seq_*`,
  `run_journal_unlocked`, `run_gitignore_lessons`. Live: two runs on this PC, 21 and 8 events.
  **If you remove this:** a run is whatever the orchestrator's context still holds, and a resumed orchestrator seats
  orders twice.
- **Changed:** `herdr seat run <seat> --order <file>` books the order (O2's lease; the rule must agree with the seat, or
  `--reason` makes it one of the run's 3 overrides), copies the order with herdr's rules appended to
  `.nirvana/orders/<id>.md` (and into the worktree with `--worktree`, where claude's print-mode file tools can read it),
  writes `seated`, and opens a pane `<seat>·<order>` in the run's tab whose shell runs `herdr seat exec <order> --run
  <run>` with this build first on `PATH`. A second `seat run` for an order the run knows returns the first. Outside a
  herdr pane it starts `seat exec` detached, never a tab on the owner's default session.
  **Because:** plan 12 §7.1 and §7.2: "`seat run` refuses a second live worker for an order that has one, and returns
  the first."
  **Measured:** `a_second_run_for_an_order_returns_the_first_and_writes_nothing` (6 racing threads seat it once);
  controls `worker_seat_twice`, `worker_opened_every_order`, `worker_footer_twice`, `worker_worktree_unignored`. Live:
  the second `seat run` of `mul` printed `mul existing`, exit 0.
  **If you remove this:** the orchestrator starts every worker by hand, and a retried call starts two.
- **Changed:** the launches. claude: `-p <line> --output-format stream-json --verbose --setting-sources project
  --strict-mcp-config --model <m> --effort <e> --permission-mode auto --json-schema <Work Report>`, with
  `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`; agy: `--output-format stream-json --model <id-with-effort> --print-timeout
  <lease> --dangerously-skip-permissions --json-schema <Work Report> --print=<line>`. Continuation `--resume` and
  `--conversation`, never `--continue`. A list argv, never a shell. Each worker starts suspended, goes into a
  kill-on-close job object, then runs, so a stop, a rate limit or a stall ends its whole process tree.
  **Because:** plan 12 §4, each line an incident in old Nirvana's adapters (16x a turn without `--setting-sources
  project`; a print-mode background task that died; `--print` taking the next flag; `--continue` picking the wrong
  conversation; `WinError 206` on an inlined order).
  **Measured:** `a_claude_worker_starts_with_the_launch_lessons`,
  `an_agy_worker_takes_its_line_as_one_print_token_and_its_effort_in_the_model`, `a_worker_is_never_started_through_a_shell`,
  `a_stop_ends_the_workers_whole_process_tree`; controls `worker_*` for each flag and `worker_tree_outside_the_job`
  (with the worker outside the job the stop took 56 s, ping's 30 s, against 0.6 s). Live: `seat stop` of a haiku worker
  running `time.sleep(240)` returned in 0.55 s and no such process was left.
  **If you remove this:** each lesson is learned again at its old price.
- **Changed:** the Work Report schema (status done, blocked or needs_answer; summary; files; tests; gotchas; optional
  question and lesson) is checked in herdr; an empty or malformed answer ends the order `failed` even when agy says
  `SUCCESS`, and agy's own `toolAction` and `toolSummary` keys are dropped first.
  **Because:** P2: flash "ended with an empty answer and `status: SUCCESS`".
  **Measured:** `an_empty_answer_is_a_failure_even_when_agy_says_success` on P2's recorded stream,
  `the_report_check_and_the_schema_name_the_same_fields`, `a_malformed_report_fails`,
  `agys_own_keys_are_dropped_from_an_answer_given_as_text`; controls `worker_report_any_key`,
  `worker_empty_answer_unnamed`, `worker_agy_status_ignored`, `worker_agy_keys_kept`.
  **If you remove this:** an empty run reads as a delivery.
- **Changed:** rate limits, typed only: claude's `rate_limit_event` with status `rejected` or an `api_retry` with
  `error: rate_limit`, agy's exit 3 with an `AGY_ERROR` line naming a quota. herdr stops the worker, writes a
  checkpoint (the report so far, the worktree's diff, the last turn), then defers when the window resets within
  `defer_min` (30 min), else re-seats through O2's `pick` with the spent pool read as spent (a child order `<id>-r<n>`
  with a continuation brief, in the same worktree, never the same conversation), else in auto mode defers to the
  earliest reset plus a minute, else asks.
  **Because:** plan 12 §5.6.
  **Measured:** `only_a_rejected_rate_limit_event_or_a_rate_limited_retry_stops_a_claude_worker` on P3's recorded
  `rate_limit_event`, `agy_runs_out_only_with_exit_3_and_a_quota_error_line`, `a_spent_pool_reseats_on_another_pool`,
  `a_window_that_resets_within_30_minutes_is_waited_for`,
  `with_no_seat_left_auto_mode_waits_for_the_earliest_reset_and_otherwise_asks`, `a_rejected_limit_stops_the_worker_at_once`;
  controls `worker_allowed_is_a_limit`, `worker_retry_*`, `worker_agy_any_*`, `worker_spent_pool_unmarked`,
  `run_defer_*`, `run_auto_mode_ignored`, `worker_limit_lets_it_run`.
  **Hypothesis, not measured:** no window was spent on purpose, so the rejected event is P3's recorded line with its
  status changed, the `api_retry` line is built from the documented shape, and agy's quota status string is not
  recorded (P5). The defer and re-seat paths have not run live.
  **If you remove this:** a spent window kills an order, or the order waits for the owner on a night run.
- **Changed:** a deferral's timer lives in the worker's own supervisor, which waits in its pane and then resumes the
  worker's own conversation; `herdr run tick` starts any due deferral whose supervisor is gone. Stalls: a stream
  silent for 10 minutes is killed and restarted at once, then after 30, 60 and 120 s, then refused as a crash loop and
  asked. **Because:** plan 12 §7.2. The server was not chosen for the timer: a reload restarts it, and its loop would
  need upstream hooks; the journal is what survives, and tick reads it.
  **Measured:** `a_stream_silent_for_ten_minutes_is_stalled_and_restarts_back_off_to_a_refusal`,
  `a_refused_restart_fails_the_order_and_asks`; controls `run_stall_line_late`, `run_restart_delays_flat`,
  `run_crash_loop_not_failed`. **Hypothesis:** a stall and a tick restart have not run live.
  **If you remove this:** a hung worker holds its lease until it expires, and a deferral dies with its pane.
- **Changed:** waking. `herdr run wake <run>` types a one-line digest into the orchestrator's pane with `herdr agent
  prompt` only when herdr says it is idle, the prompt line on its screen is empty (Claude Code 2.1.302's box: `❯`, a
  no-break space, and a faint placeholder that is not the owner's text), and the owner's last key in that pane is 30 s
  old or more; `--urgent` (the owner's stop) steers a working one. Otherwise the events wait for `herdr run notice`,
  which prints them for the `UserPromptSubmit` hook. After 3 wakes in a row that move no order forward the run asks the
  owner instead. herdr's server writes the owner's last key per pane to `<home>/nirvana/keys/` (one hook in
  `server/headless.rs`, at most one write a second). `herdr run check` exits 1 for a run with no live worker and no
  next step; `herdr run resume` reports each interrupted step and never reruns it.
  **Because:** plan 12 §7.2 and R6; Argon's review F7: a wake typed into the pane splices into the owner's typing.
  **Measured:** `a_wake_types_only_into_an_idle_orchestrator_with_an_empty_prompt`, `a_wake_never_types_while_the_owner_does`,
  `the_prompt_line_is_read_from_claude_codes_own_screen` (recorded screens), `three_wakes_in_a_row_that_move_no_order_forward_stop_the_waking`,
  `an_unfinished_run_with_no_live_worker_and_no_next_step_is_a_dead_end`,
  `resume_reports_each_interrupted_step_and_never_a_finished_or_live_one`; controls `run_wake_*`, `run_prompt_*`,
  `run_check_*`, `run_resume_cuts_live`. Live: a delivery woke an idle interactive Claude in the run's orchestrator pane
  (`woken via prompt`); with no such pane the next one queued ("state is not known") and `run notice` printed it once.
  **Hypothesis:** the owner-key hook has not run with a real client attached.
  **If you remove this:** the orchestrator learns of a delivery only when the owner next types.
- **Changed:** the wake guard. `herdr run wake` types only into a pane that runs Nirvana Claude, and queues the events,
  as for a busy pane, for any other pane, an urgent stop included. The signal is two of herdr's own readings: `pane
  get`'s `agent` is `claude`, and one of `pane process-info`'s foreground processes has Nirvana Claude's argv,
  `--agent nirvana` with `--plugin-dir` this home's `nirvana/claude`. `type_prompt` reads both again just before it
  types. `NIRVANA_ORCHESTRATOR=1` is not the signal: herdr exposes no process's environment, and in the verb's seeded
  shell it reaches Claude's tools through settings.json, not `claude.exe` itself.
  **Because:** the O3 build's live run, 2026-10-18 (record `o3.md`): the run's orchestrator pane "was an interactive
  haiku Claude Code I had started in the isolated server to record its prompt screen, with its folder the owner's
  Nirvana repository. It took the digest as a prompt and ran read-only commands there (`cat` of a fixture, `git
  worktree list`, a `find` under the home folder) before I interrupted it with Esc". To herdr, plain Claude Code and
  Nirvana Claude were the same idle `claude`.
  **Measured:** `a_wake_types_only_into_a_pane_that_runs_nirvana_claude`, on `pane get` and `pane process-info`
  recorded from four panes of an isolated herdr server (Nirvana Claude, plain Claude Code, pwsh, agy); controls
  `run_wake_guard_*` and `run_type_unguarded`. Live, run m1 in an isolated server with a pwsh pane as its orchestrator:
  no `woken` event after either delivery, `run wake m1` printed "2 event(s) queued: the orchestrator's pane does not run
  Nirvana Claude: herdr reads its agent as none", the pane showed only its prompt, and `run notice m1` printed both.
  **If you remove this:** a wake types a digest into whatever program holds the recorded pane, a plain Claude Code or a
  shell, which runs it as a prompt or a command.
- **Changed:** on the merge onto 0.15.0, the runner's own bookings take the O2 fix round's paths: `seat run` books
  through `book_pick` (an override's count carries its time, so the 8-day prune keeps it) and refuses an unknown
  `--needs` or `--not-family` word, exit 2; every writer of `leases.json` loads it with `load_leases`, so a file that
  does not parse is left as it is (`seat run` and a re-seat exit 2; `record_once` still records the order and says
  the booking was not released). The lossy `read_leases` is removed.
  **Because:** O3 was built on 0.13.0, before the fix round. Its booking set `overrides += 1` with no time, which the
  fix round's prune reads as a run 8 days old and drops; its lossy read wrote a corrupt file back as empty, every run's
  override count with it.
  **Hypothesis, not measured:** no test drives `seat run`'s booking (it reads the live world); `book_pick` and
  `load_leases` are the fix round's, tested there.
  **If you remove this:** the runner's overrides escape the 3-a-run cap after the first prune.
- **Changed:** `herdr run receipt <run> [--json]`, recomputed from the journal and `record.jsonl`, no cache and no
  clock (an order's duration runs from `seated` to its last event); `herdr run end` writes `receipt.md` and
  `receipt.json` beside the journal. Each missing figure says why. O2's `RecordLine` gains optional `v`, pool, reason,
  override, seated_at, ended_at, send_backs, reseats, deferrals, rate_limit_stops, stalls, and from the booked pick
  rule_pick, mode, size, bar_dropped, step_down, use_basis and expected use per window, plus the herdr version and a
  journal reference `<path>#<seq>`; keys it does not know are kept on a round trip. `herdr seat record` and the runner
  both record through `record_once`: under `leases.lock`, refuse an order and run already recorded, fill the line from
  the live booking, append, then release the booking. A line is written once per order when it ends (gated, failed,
  re-seated) and at `run end`.
  **Because (O2 review S5):** a round trip dropped unknown fields; releasing the lease lost the pick, and expected
  against real use is what offline tuning needs; a retry appended twice because the line was written before the lock.
  **Because:** the owner, 2026-10-17: "a form of run receipt Nirvana had this for checking how the agents ran and how it
  can improve." (L11), and old `report.py`'s rule.
  **Measured:** `the_receipt_is_the_same_bytes_twice_and_changes_with_one_appended_event`,
  `a_missing_figure_says_why_and_is_never_zero`, `the_record_line_carries_the_runs_outcome_and_old_lines_still_parse`
  (an O2 line parses and writes back byte for byte), `a_record_line_keeps_keys_it_does_not_know`,
  `an_order_is_recorded_once_with_its_booking_and_the_booking_ends`, `racing_record_calls_write_one_line` (8 threads,
  one line); controls `run_receipt_*`, `run_record_*`, `record_*`, `run_send_back_uncounted`.
  Live: two `run receipt` calls on run live1 were the same bytes (`cmp`).
  **If you remove this:** "how did it run" is answered from memory.

## alpha 0.15.0 (2026-10-18): Nirvana Claude's launch, the "new" menu, "Open Nirvana", its own icon, badge and toast (plan 12 O4a)

Plan 12, package O4a (`plans/2026-10-17-12-nirvana-claude-orchestrator.md` L5, L6, L7, §6, §8, R1, R8; record
`plans/2026-10-17-12-records/o4a.md`).

- **Changed:** Nirvana Claude is Claude Code started as `claude --plugin-dir <home>/nirvana/claude --agent nirvana
  --name Nirvana --settings <home>/nirvana/claude/settings.json` with `NIRVANA_ORCHESTRATOR=1`. The plugin's minimal
  tree is `nirvana/claude/`: the manifest, `agents/nirvana.md` (the main-thread agent: its role, §6.1's loop, §6's
  rules verbatim, R2's small asks; `model: claude-opus-5-5`, `effort: medium`) and `settings.json` (the theme
  `custom:nirvana-dark` and `env.NIRVANA_ORCHESTRATOR=1`). `install.py` copies it to `<home>/nirvana/claude` before
  the binary, so a reload never meets an older plugin. Plain Claude Code and agy are typed as `claude` and `agy`
  alone.
  **Because:** the owner, 2026-10-17 (L6): "this as a seperate version which tbh is proably just Claude Code with
  these Orchistrator level skills and memories and hooks"; R1: "opus 5.5 medium".
  **Measured:** each flag exists as written in `claude --help` of Claude Code 2.1.302. Live `/context` of
  `claude --plugin-dir <worktree>/nirvana/claude --agent nirvana -p /context`: 0 turns, $0, `Model: claude-opus-5-5`,
  `nirvana:nirvana | Plugin`, system prompt 1.5k; plain `claude -p /context`: 0 turns, no nirvana row, system prompt
  2.5k. `settings.json`'s `env` reaches Claude's Bash tool (`printenv NIRVANA_ORCHESTRATOR` printed `1`, one haiku
  low turn, $0.005). Tests `nirvana_launch::tests::*`, `app::nirvana_new::tests::*`; controls `o4a_argv_*`,
  `o4a_plain_*`, `o4a_server_plain_gets_plugin`, `o4a_settings_env_off`, `o4a_agent_effort_high`.
  **If you remove this:** the verb and the menu open a stock Claude Code, and nothing tells the two apart.
- **Hypothesis, not measured:** a main-thread agent's prompt replaces Claude Code's default system prompt (the
  sub-agents page; 2.5k against 1.5k above). Whether Nirvana Claude edits and uses its tools as well without that
  prompt is for O7's A/B to measure. The agent file carries a short working-style section for it.
- **Changed:** the sidebar's "new" opens a menu: Nirvana, Claude Code, Antigravity, and Shell (what "new" did
  before). Each agent entry opens a space on the folder a plain "new" would use and starts its agent in the root pane
  the way `herdr agent start` does (`command.invoke nirvana.new:<kind>`). Only Nirvana gets the plugin argv and the
  pane env. Choosing Nirvana also makes the client's window a Nirvana window and shows the toast. The new-space key
  still opens a shell space.
  **Because:** L7: "Sometimes I want a Orchistrator style model that tries to utilize both Claude and ANtigravity and
  sometimes I just want Claude Code or agy traditional to tackle the project."
  **Measured:** `client::shell::tests::nirvana_new_menu::*`, the upstream click test updated to open the menu then
  choose Shell; controls `o4a_menu_*`, `o4a_new_click_no_menu`, `o4a_new_not_routed`.
  **If you remove this:** "new" opens a shell again and every agent is typed by hand.
- **Changed:** Explorer's "Open Nirvana" (`HKCU\Software\Classes\Directory\shell\NirvanaHerdr` and
  `Directory\Background\shell\NirvanaHerdr`) runs `wt -w new -p Nirvana -d "%1" <home>\bin\herdr.exe --session
  nirvana nirvana window "%1"`. `herdr nirvana window <folder>` starts the session's server if it is not running,
  focuses the Nirvana Claude already on the folder or starts one (a fresh server's seeded shell space takes it, so no
  second space opens beside it), claims the window, shows the toast, runs the client under the reload supervisor, and
  clears the window when the client leaves. `install.py` (through the new `windows_shell.py`) also writes the
  "Nirvana" Windows Terminal profile and its two schemes as the fragment `Fragments\Nirvana-Herdr\nirvana.json`, the
  two Claude Code themes to `<claude config>\themes`, JetBrains Mono when no face of it is registered, and removes
  Nirvana-C's "Open Nirvana - CC" keys (`Directory\shell\NirvanaC`, `Directory\Background\shell\NirvanaC`) and its
  fragment `Fragments\Nirvana-C\nirvana.json`.
  **Because:** R8: "Lets do the full port were at a stage now I think the herder varient is the perfered method.
  Remove Nirvana - CC have standard Nirvana use the herder varient."
  **Measured:** `nirvana/test_windows_shell.py` (a fake registry, and the real one under
  `HKCU\Software\NirvanaHerdrTest-<pid>`, created and deleted); controls `o4a_py_*`. The key is `NirvanaHerdr`
  because `Directory\shell\Nirvana` holds old Nirvana's "Open Nirvana here" (read 2026-10-17). No real verb key was
  written during the build.
  **If you remove this:** the right-click opens Nirvana-C's Claude, or nothing.
- **Changed:** install also retires old Nirvana's "Open Nirvana here" (`Directory\shell\Nirvana` and its Background
  twin). First it writes each key's MUIVerb, Icon and command to `<home>\nirvana\backup\old-nirvana-verb.json`, and
  only a key whose command names `open-here.pyw` is removed. The old daemon itself is not touched.
  **Because:** R8, "have standard Nirvana use the herder varient". With both installed, the folder menu would show
  "Open Nirvana here" (the old daemon's window) beside "Open Nirvana" (this fork). Read from the registry
  2026-10-18; added by the orchestrator at the merge.
  **Measured:** `test_install_writes_the_verb_and_toast_id_and_removes_nirvana_cs_verb` checks the removal and the
  backup; `test_a_nirvana_key_of_another_program_is_left_alone` checks that another program's key stays. Controls are
  in `controls.d/o4a_retire.py`.
  **If you remove this:** two Nirvana entries in the folder menu, one of them opening the retired daemon's window.
- **Changed:** `herdr.exe` carries `nirvana/assets/nirvana.ico` as icon group 1. build.rs writes the `.res` itself
  (one RT_ICON per image, one RT_GROUP_ICON) and passes it to MSVC's linker, so no build dependency and no `rc.exe`.
  **Because:** P7: "The icon must be a valid resource at the time the id is set ... An icon embedded in herdr.exe
  (`<herdr.exe>,0`) is the stable choice."
  **Measured:** the built exe has 1 icon group and six RT_ICON resources of 554, 1071, 1318, 2158, 3079 and 19691
  bytes, the six images of the `.ico`; `tests/nirvana_icon.rs`; control `o4a_icon_group_two`. `Cargo.lock` unchanged.
  **If you remove this:** the Nirvana window's button shows a blank page icon, which Windows does not re-read under
  the same id (P7).
- **Changed:** a Nirvana window gets its own taskbar button: on the Terminal window (`GetAncestor(GetConsoleWindow(),
  GA_ROOTOWNER)`) herdr sets `RelaunchIconResource = <herdr.exe>,0`, `RelaunchCommand`, `RelaunchDisplayNameResource
  = Nirvana`, then `AppUserModel_ID = Nirvana.Herdr.<16 hex of the folder's sha256>`, then the overlay badge: the
  folder's first letter or digit (N when it has none), drawn at run time from the saffron template's numbers. On exit
  the badge goes first, then the properties.
  **Because:** R8, the owner: "have its own Icon and a toast with the directory first Letter as the icon".
  **Measured:** `RelaunchCommand` takes at most 255 characters on this machine (`IPropertyStore::SetValue`: 255 set,
  256 to 259 E_INVALIDARG, 260 and more ERROR_INSUFFICIENT_BUFFER); the first live run used the verb's 487-character
  line and the button never split, so the relaunch line names `wt.exe` by its alias and falls back to `wt.exe -w new
  -p "Nirvana"`. Live run 2 (one new Terminal window, debug build, session `o4a-live`, scratch home): the window read
  `id=Nirvana.Herdr.066791b9d817e117`, `icon=<herdr.exe>,0`, `name=Nirvana`; the taskbar gained "Nirvana - 1 running
  window" with 383 teal and 189 saffron pixels while Terminal's button had 0 of each; after the server stopped the
  properties read empty, the Nirvana button was gone, and Terminal's button had 38 saffron pixels, all in rows 40-42
  (the running-window underline in the accent colour), the same 38 as before the run. Tests
  `nirvana_taskbar::tests::*`; controls `o4a_badge_*`, `o4a_id_first`, `o4a_clear_badge_last`, `o4a_set_keeps_going`,
  `o4a_relaunch_over_limit`, `o4a_template_drift`, `o4a_tile_no_rim`, `o4a_gdi_black_ink`.
  **If you remove this:** a Nirvana window is one of Terminal's windows in its group, with nothing to tell it apart.
- **Changed:** herdr's toasts fire on the fork's install. `notifications.rs` returned NotFound without the bundled
  ConPTY runtime, which the fork's install has never had; it now falls back to Windows' own
  `System32\conhost.exe --headless --`. The toasts use the fork's id `Nirvana.Herdr` (DisplayName "Nirvana", IconUri
  the installed `nirvana-256.png`), and a toast may carry an `appLogoOverride` picture: on open, "Nirvana · <folder>"
  with the badge as a 96 px PNG.
  **Because:** P7: "`notifications.rs:161-167` returns NotFound without it, so herdr's own Windows toasts most likely
  never fire on the fork's install."
  **Measured:** `conhost.exe --headless -- cmd /c ping -n 3 127.0.0.1` ran to exit 0 with 0 visible windows. Live:
  the toast history of `Nirvana.Herdr` held "Nirvana · Pojects" with `appLogoOverride` `badge-0050-96.png` after each
  open. Test `nirvana_toasts_carry_the_forks_id_the_letter_and_a_host_that_exists`; controls `o4a_toast_*`.
  **If you remove this:** no herdr toast is ever seen on the fork.
- **Hypothesis, not measured:** a click on a toast (activation through the system console host), a pinned Nirvana
  button's relaunch, two Nirvana windows on two folders at once, never-combine taskbar mode, and Windows Terminal's
  handling of a folder with `;` (it splits its command line there).
## alpha 0.14.0 (2026-10-18): media seats, `herdr media probe|shrink|prompt`, the media skill (plan 12 O6)

Plan 12, package O6 (`plans/2026-10-17-12-nirvana-claude-orchestrator.md` §2, §4, §5.2, §5.4, §6, §11;
`plans/2026-10-17-12-records/o6.md`).

- **Changed:** `herdr media probe <file> [--json]` (`src/nirvana_media.rs`, dispatched from `src/main.rs`) runs
  `ffprobe` with a list `argv` (never through a shell) and prints the file's duration, byte size, container format,
  video codec and resolution, whether audio is present (with its codec), and whether the file exceeds Antigravity
  `view_file`'s 100 MB limit (`100,000,000` bytes). When `ffprobe` is missing on `PATH`, it says so clearly and exits 2.
  **Because:** the owner, 2026-10-17: "Argon also has capabilities that Claude does not have like ingesting video.
  Consider these unique apabilities as part of the orchistrator seating process." (plan 12 L9). Probe P2 showed
  `view_file` refuses files over 100 MB (a 186 MB recording failed at the tool layer before reaching Gemini), so the
  orchestrator needs a zero-token check before seating a media worker.
  **Measured:** `media_over_limit_checks_the_100mb_view_file_cap` and
  `media_ffprobe_json_reads_duration_size_container_video_audio_and_limit` on an `ffprobe` 8.0.1 JSON fixture recorded
  from a real 1920x1080 H.264 + mono AAC MP4; controls `media_limit_inclusive`, `media_view_limit_wrong_cap`,
  `media_probe_ignores_duration`, `media_probe_ignores_size`, `media_probe_ignores_video_stream`,
  `media_probe_ignores_audio_stream`, `media_probe_wrong_resolution`, `media_probe_over_limit_inverted`. Live on a
  3-minute (180.00 s) 1920x1080 30 fps synthetic screen recording with 440 Hz/1 kHz sine audio (`120,009,381` bytes,
  120.01 MB): `over 100 MB view_file limit: yes (limit 100000000 bytes)`.
  **If you remove this:** the orchestrator sends raw recordings over 100 MB straight to `agy`, and `view_file` refuses
  them mid-turn.
- **Changed:** `herdr media shrink <file> [--out <path>] [--max-mb 95] [--json]` runs `ffmpeg` with a list `argv` to
  re-encode a recording to `<name>.shrunk.mp4` beside the input (unless `--out` is given), refusing any path that
  collides with the input. It downscales width to at most `1280` px (`scale='min(1280,iw)':-2`), caps frame rate at
  `10` fps (`-r 10`), keeps spoken narration as mono 48 kbps AAC (`-ac 1 -b:a 48k`), budgets the video bitrate from
  duration with a 15% safety margin (`SAFETY_MARGIN = 0.85`, clamped between `32` and `4,000` kbps) under VBV rate
  control (`-b:v`, `-maxrate`, `-bufsize`), and retries once at 60% of the first pass's video bitrate (or the measured
  overshoot ratio, whichever is lower) if pass 1 still exceeds `--max-mb`.
  **Because:** plan 12 §11 O6 and Argon's review (§3): raw screen recordings exceed `view_file`'s 100 MB limit, while
  1280 px at 10 fps preserves UI text, layout transitions, and mono voice-over narration within budget.
  **Measured:** `media_bitrate_scales_with_duration_and_clamps_very_short_and_very_long_files` (180 s -> `3,540` kbps,
  600 s -> `1,028` kbps, 0.2 s -> clamped to `4,000` kbps, 36,000 s -> clamped to `32` kbps, retry 1000 -> `600` kbps)
  and `media_shrink_argv_pins_scale_fps_mono_audio_and_never_overwrites_input`; controls `media_bitrate_no_margin`,
  `media_bitrate_no_audio_deduction`, `media_bitrate_no_short_cap`, `media_bitrate_no_long_floor`,
  `media_bitrate_retry_not_lower`, `media_shrink_wrong_scale`, `media_shrink_wrong_fps`, `media_shrink_stereo_audio`,
  `media_shrink_allows_overwrite`, `media_shrink_default_out_wrong_suffix`. Live on the 120.01 MB (`120,009,381`-byte)
  3-minute 1920x1080 30 fps recording: shrunk in 1 pass at `3,540` kbps to `73,048,200` bytes (73.05 MB), 1280x720 at
  10 fps with mono AAC audio (`over_limit: false`).
  **If you remove this:** recordings over 100 MB cannot be ingested by `argon` or `flash`, or a naive re-encode drops
  the voice-over track or overwrites the caller's source file.
- **Hypothesis, not measured:** the `0.85` bitrate safety margin, the `0.60` retry factor, the `32`–`4,000` kbps video
  clamps, and taking `1 MB = 1,000,000` decimal bytes (which errs on the safe side of `100 MiB = 104,857,600` bytes).
- **Changed:** `herdr media prompt <file> --ask "<question>" [--seat argon|flash]` prints the single-line prompt for an
  `agy` media worker, resolving `<file>` to its full absolute path, instructing the worker to call `view_file` with
  `AbsolutePath` and without `StartLine` or `EndLine` (and to start no subagents or background commands), and asking for
  (1) `MM:SS` timestamps, (2) what happens on screen and any spoken transcript, (3) visual details a text transcript
  alone would lose, and (4) findings for `<question>`. `nirvana/claude/skills/media/SKILL.md` gives the Nirvana Claude
  orchestrator the full workflow (`herdr media probe`, `herdr media shrink`, `herdr seat pick --class
  media.video|media.audio|read.huge`, `herdr media prompt`, resuming `read.huge` by `--conversation <id>` for read-only
  follow-ups, never reading media or huge files into the orchestrator's own context, and treating an empty answer as a
  failure even when `status: SUCCESS`).
  **Because:** probe P2 (`nirvana/evidence/2026-10-17-o0-probes/README.md`): with a bare filename, `flash` tried `dir`,
  plan mode refused it, and the turn ended with an empty answer and `status: SUCCESS`; with the full path and
  `view_file` named, both `argon` and `flash` read MP4 and WebM files accurately.
  **Measured:** `media_prompt_line_names_full_path_view_file_and_expected_return_fields`; controls
  `media_prompt_relative_path`, `media_prompt_omits_view_file`, `media_prompt_multiline_kept`,
  `media_prompt_empty_ask_accepted`. Live turn on `agy` 1.4.2 (`gemini-3.8-flash-low`, `--output-format stream-json
  --mode plan`) with the `herdr media prompt` line for the shrunk 3-minute 73.05 MB video: called `view_file` on the
  absolute path (`0.13 s`), exited `SUCCESS` (`58,913` in, `63,884` out including `62,913` thinking tokens, `127.4 s`),
  and returned the timestamped timeline (`00:00` to `03:00`), the `HH:MM:SS.mmm` upper-left counter, the 6 color bars
  and 4-state morphing reticle, and the continuous sine wave tone.
  **If you remove this:** a media worker handed a bare filename in plan mode can exit `0` with `status: SUCCESS` and an
  empty answer, or return a text-only summary that loses the visual bug.

## alpha 0.13.0 (2026-10-17): seats, `herdr seat list|pick|release|record`, the rule, leases and the record

Plan 12, package O2 (`plans/2026-10-17-12-nirvana-claude-orchestrator.md` §3, §4, §5, §16).

- **Changed:** the foot no longer draws Argon or Google's Claude pool (G·Claude). Both are still read, priced by
  `herdr seat` and listed by `herdr usage` and its JSON (`nirvana_usage::FOOT_HIDDEN`). The test fixture's Gemini row
  now shows its week, so the foot keeps a "7d" row and a reset in days under test.
  **Because:** the owner, 2026-10-18: "We dont need tracking for g claude or argon on the usage keep that internally."
  **Measured:** `the_foot_lists_a_spaces_server_and_a_usage_row_per_pool` asserts neither row is drawn and the
  accounts button sits at row 5 under the head; control `footer_hidden_pools_drawn`
  (`nirvana/controls.d/foot_hidden.py`).
  **If you remove this:** the foot grows back to six usage rows, two of them for pools the owner does not watch.
- **Changed:** `nirvana/controls.py` loads each package's controls from `nirvana/controls.d/<package>.py`, and
  `--dry-run` counts each control's find text, exiting 1 unless every one occurs exactly once.
  **Because:** plan 12's packages now build in parallel; packages that each inserted into the one tuple lost a `    ),`
  at merges. The old dry run only printed plans and missed five controls whose text had gone (the O1 builder,
  2026-10-17).
  **Measured:** a probe control with a missing find printed `FAIL ... finds 0x` and exit 1; without it, every
  control's find text occurred once.
  **If you remove this:** parallel merges collide in one tuple again, and a dead control passes a dry run.

- **Changed:** `<home>/nirvana/seats.toml` holds the seat table: the pools (their windows and a factor from list-price
  dollars to a share of each window), the seats (on: opus, sonnet, haiku, g-opus, g-sonnet, argon, flash; off: fable,
  gpt-oss and local), each with its CLI, model id, efforts, pool, capabilities, a dated price schedule and its
  expected use per effort, the classes of §5.2 in the chains of §5.4, each listing its seats in §5.4's order with their
  scores, efforts, needs and bars, the order sizes, and §5.3's constants. herdr writes it from its default (`src/nirvana_seats.toml`) when it
  is missing, and from then on it is the owner's, as JEV.md is. An edit with a problem (an unknown seat, bars that
  fall, a bad date, an unknown key) is refused with its reason and never overwritten.
  **Because:** the owner, 2026-10-17: "have a orchistrator agent that is aware of the 5h and 7s usage limits and can
  seat and coordinate which agent and model can take the task to efficiently use usage credits." (plan 12 L2). Plan 12
  §4: "herdr writes it from a built-in default when missing, and from then on it belongs to the owner, like JEV.md."
  **Measured:** `the_default_table_is_written_once_parses_and_holds_the_plans_seats_and_classes`; controls
  `seat_default_*`, `seat_problems_*`, `seat_toml_*`. Expected use is tagged per cell: measured from probe P4
  (g-sonnet medium 3.31% of Google's Claude 5 h window, argon low and medium 1.01% of Argon's week, flash high 0.68% of
  Gemini's 5 h window), derived from P4's token counts priced by the schedule, or a prior.
  **If you remove this:** the seating's numbers live in code, and moving a bar or a prior is a build.
- **Hypothesis, not measured:** every prior score, the bars that depart from decision 0018's 70/80/100 and 85/90/100,
  the class weights, the order sizes (small is P4's order, medium 5x, large 20x), the effort factors (low 0.6, high
  1.6, xhigh 2.5 of medium), and the `max` pool's factor ($25 of list price per 5 h window, $200 per week): P4 could not
  separate one order on Max while other agents ran there.
- **Changed:** `herdr seat pick --class C [--size] [--needs] [--not-family] [--mode] [--json]` seats an order by §5.3's
  rule, a pure function of the table, the readings in `usage.json` (stale readers read first, as `herdr usage` does),
  the leases, the accounts and the request. Filters in order, each skip with its reason: off; CLI not installed or not
  signed in (Accounts); a need the seat lacks; the class does not take it; a family the caller excluded (a preference,
  with decision 0017's fallback, below); a window at or under the floor (5%); a window that cannot hold the order while
  another pool can; a `max` seat that would leave the 5 h window under the orchestrator's reserve (15%). Then the
  mode's bar, dropping to the best seat left but never under the Efficient bar; then three tiers, known headroom, a
  window not read, a known window too small for the order; then the least pressure,
  `max over windows (expected / headroom × pace × reset)`; ties to the higher score, then the class's own order. It
  prints the reason line of §5.3 step 5 and every other seat with why it lost. Exit 0 with a seat, 3 with none, 2 on a
  usage or table error, 4 when another herdr held the seat lock for 10 s.
  **Because:** plan 12 §5.1: "herdr decides the seat, deterministically, and prints why."
  **Measured:** `nirvana_seat::tests::*` on the readings of 2026-10-17 21:51Z, one per filter, the bar and its
  floor, pressure across windows, the reset discount, ties, unknown windows, effort, NoSeat; controls `seat_filter_*`,
  `seat_bar_*`, `seat_pressure_*`, `seat_reset_*`, `seat_unknown_*`, `seat_stale_*`, `seat_ties_*`, `seat_reason_*`.
  Live on this PC: `build.bounded` to sonnet medium, `media.video` to argon medium, `plan` to opus high (xhigh at SOTA).
  **If you remove this:** the orchestrator seats by feel, and every order lands on the pool the orchestrator itself
  runs on.
- **Changed:** effort follows the class and its blast radius (§5.5): SOTA lifts plan and load-bearing builds to xhigh,
  max is never chosen (a class or seat asking max gets xhigh), a seat without the effort gets the nearest it offers at
  or below it, said in the reason. Under pressure the same model on another pool comes first (opus and g-opus, sonnet
  and g-sonnet score the same), then the other family (the bar drop), and only when nothing is left one effort step
  down.
  **Because:** plan 12 §5.5 and Argon's review: "Stepping down first spends the congested pool and weakens the
  reasoning."
  **Measured:** `effort_follows_the_class_sota_lifts_plans_and_max_is_never_chosen`,
  `under_pressure_with_nothing_left_one_effort_step_down`; controls `seat_effort_*`, `seat_step_down_*`.
  **If you remove this:** every order thinks at the seat's one effort, the defect decision 0018 §1 measured.
- **Changed:** `--needs video` never returns a Claude seat: only argon and flash carry `video` and `audio`.
  **Because:** the owner, 2026-10-17: "Argon also has capabilities that Claude does not have like ingesting video.
  Consider these unique apabilities as part of the orchistrator seating process." (L9, whose verifier this is).
  **Measured:** `an_order_that_needs_video_never_gets_a_claude_seat` over every class, mode and size, with and without
  Argon spent, and with an override to each Claude seat; control `seat_filter_need`.
  **If you remove this:** a recording is sent to a model that cannot see it, and P2 showed an empty answer can still
  say `SUCCESS`.
- **Changed:** `--book <order>` writes a lease to `<home>/nirvana/leases.json` (its seat, pool, expected use per window,
  booked and expiry times, and the pick as given) under the OS's lock on `leases.lock` and by a temp file and a
  rename; a second booking
  for the same order returns the first. Headroom subtracts every live lease but the order's own; an expired lease
  (60, 180 or 480 minutes by size) holds nothing. `herdr seat release <order>` drops one; `herdr seat record` drops the
  finished order's.
  **Because:** plan 12 §3: "Four parallel picks in five seconds cannot all see the same headroom and break the reserve
  together" (Argon's review, F1). The rename alone keeps the file whole but lets two writers lose a lease, so the read,
  the pick and the write hold `leases.lock`.
  **Measured:** `live_leases_take_headroom_expired_ones_and_the_orders_own_do_not`,
  `a_booking_is_idempotent_per_order_and_released_by_id`,
  `the_lease_lock_is_exclusive_and_a_file_left_behind_does_not_hold_it`;
  controls `seat_booked_*`, `seat_lease_*`, `seat_book_*`, `seat_release_*`, `seat_lock_*`. Live: a second `--book o-1`
  answered `"booked": "existing"` with the first pick.
  **If you remove this:** parallel orders each see the whole headroom and break the reserve together.
- **Hypothesis, not measured:** the lease lengths. A lease is not shortened when a reading taken after the booking already shows part of its use, so a long
  order counts partly twice until it ends; that errs toward the reserve.
- **Changed:** `--override <seat> --reason "..." --run <id>` takes the orchestrator's seat over the rule's, and still
  passes every filter of step 1, the reserve and the class included; three per run are accepted, counted in
  `leases.json`, and a fourth is refused (exit 3). The reason line names the override and the rule's own pick.
  **Because:** plan 12 §5.3 step 4: "at most 3 per run are accepted without the owner".
  **Measured:** `an_override_passes_every_filter_and_three_are_taken_per_run`; controls `seat_override_*`. Live: the
  fourth override in run r-1 was refused, exit 3.
  **If you remove this:** an override can dodge the price, or the orchestrator cannot overrule at all.
- **Changed:** `herdr seat record --json '<line>'` appends a finished order to `<home>/nirvana/record.jsonl` (order, run,
  seat, model, effort, class, chain, the gate's outcome, tokens, use, time), filling the chain and model from the table
  and the use from the order's own tokens times its seat's price on its day times its pool's factor. A seat's score can
  blend with its record by decision 0029's formula, `min(1, ((passes + 5 × 0.8) / (orders + 5)) / 0.8)`, keyed by seat,
  effort and chain, behind `use_record = false`: off in this version, so the priors stay static and the record is
  logged for offline tuning.
  **Because:** plan 12 §4: "The first version keeps the priors static and logs every order's real use to
  `record.jsonl` for offline tuning." Decision 0029 §2.53 for the formula.
  **Measured:** `the_record_blend_is_decision_0029s_and_ships_off` reproduces 0029's own rows (9 of 12 is 0.956, 0 of 3
  is 0.625); `a_finished_order_is_recorded_with_its_use_from_its_own_tokens` prices P4's g-sonnet order at 3.31% of the
  5 h window; `argons_price_doubles_on_2026_11_30_and_its_expected_use_with_it`; controls `seat_record_*`,
  `seat_price_*`, `seat_use_*`.
  **If you remove this:** O3's runner has nowhere to put an order's outcome, and the table can never learn.
- **Changed:** `herdr seat list [--json]` prints each seat, its pool's windows left (and after the reserve), and why it
  cannot take an order; the JSON adds the pools, classes, chains, prices, use cells and the orders in flight.
  **Because:** plan 12 §11: "Until O8, the statusline, `herdr usage` and `herdr seat list` show the seating."
  **Measured:** `seat_list_shows_each_seat_its_headroom_and_why_not`; controls `seat_list_*`.
  **If you remove this:** the seating has no view before the Seats square of O8.
- **Changed:** the foot's pace mark ▲ shows when any window every seat of the pool spends is fresh and ahead of pace,
  not only the window shown; `week.fable` does not count.
  **Because:** plan 12 §15: live at 22:06Z Claude's week read pace 1.72 while its 5 h window, the one shown, read 0.98,
  and the row carried no mark. The orchestrator chose this; the owner gave no ruling.
  **Measured:** `the_pace_mark_shows_when_any_window_of_the_pool_is_ahead`; controls `footer_mark_shown_window_only`,
  `footer_mark_scoped_week`, `footer_mark_stale_week`, and `usage_ahead_threshold` repointed into `any_ahead`.
  **If you remove this:** a week running out early is invisible while its 5 h window is the fuller.
- **Changed:** five controls that found nothing on master 14c3d4a0 are pointed at the current code and redden their
  tests again: `panels_both_open`, `memory_panel_beside_preload` and `panel_painted_over` (0.11.0 added the Accounts
  panel's line), `edit_tab_unnamed` (0.11.0 moved the tab into `nirvana_open_command_tab`), `jev_rows_missing` (0.10.0
  gave `jev_items` the folder).
  **Because:** plan 12 §15: "Five older controls find nothing on master". A control whose find is gone guards nothing.
  **Measured:** each run alone with `--only`: red line seen, tree clean after.
  **If you remove this:** those five tests can lose their assertion and stay green.
- **Changed:** `nirvana_usage::pace_with` takes pace's three numbers, which `seats.toml` holds (the foot keeps its
  constants), and `nirvana_usage::current_sources` is the read `herdr usage` and `herdr seat` share.
  **Because:** plan 12 §5.3: "with their numbers in seats.toml"; one formula for the foot and the seating.
  **Measured:** `usage_pace_no_ease` and `usage_pace_no_clamp` repointed at `pace_with`, both red when run.
  **If you remove this:** the seat engine needs its own copy of pace and of the cache read.

- **Changed (fix round, the O2 review of 2026-10-18):**
  - A family the caller excluded is a preference: when no other family has a seat and no override was given, the class
    is seated again on its own family, in a fresh session; the pick carries `same_family_fallback` and its reason line
    names each other-family seat and what stopped it. A NoSeat after both passes is the second's, so its earliest reset
    counts both families.
  - `build.bounded` lists sonnet, argon, flash, g-sonnet with g-sonnet at 77; `sweep.mechanical`'s Balanced bar is 79.
    A class lists its seats in §5.4's order, and a class that lists a seat twice is refused.
  - Ranking before pressure is three tiers: known headroom, a window not read (a pool with no windows counts here), a
    known window too small for the order. An unread `max` 5 h window says "reserve 15% not checked".
  - Ties go to the class's own order before the table's; table order no longer breaks a tie.
  - The lease lock is `File::try_lock`: no stale rule, nothing deletes `leases.lock`, and a timeout is exit 4.
  - A NoSeat's earliest reset takes each seat's latest floored window, unknown when one has no reset time.
  - The accounts read at a pick has a 15 s deadline.
  - `--needs` and `--not-family` words no seat has are exit 2; a `leases.json` that does not parse is exit 2, never read
    as empty; a repeated `--book` that names another `--override` is refused (exit 3); an override's `bar` is the seat's
    own score when under the mode's bar, and the seats the rule ranked "lost to the override"; the reset discount's
    "left" is the headroom after booked use and the reserve; the step-down mark is only for a seat whose effort fell;
    runs whose last override is over 8 days old are pruned; USAGE names the default size and exits 2, 3 and 4.
  - `claude -p /usage` runs with `--strict-mcp-config`.
  **Because:** the review: B1 (decision 0017's title, "A review prefers another family, falls back to its own in a
  fresh session"), S1 (the orchestrator ruled on 2026-10-18 that §5.4 is the spec, and the P4 evidence calls g-opus and
  g-sonnet "overflow seats, not a second Max"), S2 (§3: an unread pool ranks "below known headroom and above
  exhausted"), S3 (two takers of a stale lock file could both get in, and a holder's drop deleted another's lock), S4,
  S6 (three hollow tests), S7, S8 and the nits.
  **Measured:** `every_class_at_balanced_seats_its_5_4_head_then_walks_its_row` (§5.4's rows typed from the plan, at
  each size, and walked by spending each pool in turn);
  `a_review_with_no_other_family_left_falls_back_to_its_own_in_a_fresh_session`,
  `a_window_not_read_ranks_below_known_headroom_and_above_a_window_too_small` (its last case flipped to the unread
  seat), `ties_go_to_the_higher_score_then_to_the_classs_own_order` (flipped to flash),
  `the_lease_lock_is_exclusive_and_a_file_left_behind_does_not_hold_it`,
  `a_seat_floored_on_two_windows_is_free_only_when_both_have_reset`,
  `a_booking_counts_its_runs_overrides_and_a_repeat_returns_or_refuses`, `a_sign_in_read_that_hangs_is_given_up_at_its_deadline`
  and the others named in `nirvana/controls.d/o2_fix.py`, each with its control there or repointed in `controls.py`.
  The `/usage` read with `--strict-mcp-config`, live 2026-10-18 02:40Z: three windows, `mcp_servers: []`, 0 turns,
  0 tokens, `total_cost_usd` 0. With another process holding `leases.lock`, `herdr seat release` exited 4 after
  10.0 s, and 0 once it let go.
  **If you remove this:** a family's outage defers every review up to a week; `build.bounded` overflows to Google's
  Claude pool ahead of argon; a seat is placed where its window cannot hold the order; two picks can hold the lock at
  once.

## alpha 0.12.0 (2026-10-17): usage readings for every pool, `herdr usage`, a foot row per pool

Plan 12, package O1 (`plans/2026-10-17-12-nirvana-claude-orchestrator.md` §3, §5.3, §8, §15).

- **Changed:** the usage readers give readings, not a percent per provider: one per pool and window, with the share
  used, the reset, when it was read and by what. The pools are `max` (Claude: `5h`, `week`, and each other "Current
  week (X)" line as `week.x`, today `week.fable`), `gemini`, `argon` and `google3p` (agy), and `codex` and `local` as
  before. agy's groups and buckets are taken as they come: `gemini-argon-*` is Argon, checked before `gemini-*`, which
  is Gemini; `3p-*` is Google's Claude and GPT pool; a bucket no prefix maps goes to a pool named from its group.
  0.4.0's stand-in rule, another `gemini-` bucket for a missing named one, is gone with its control
  `usage_agy_no_stand_in`: every bucket lands in its own pool.
  **Because:** the owner, 2026-10-17: "have a orchistrator agent that is aware of the 5h and 7s usage limits and can
  seat and coordinate which agent and model can take the task to efficiently use usage credits." (plan 12 L2, §3).
  **Measured:** agy 1.4.1 at 21:51Z printed five buckets in three groups (`gemini-weekly`, `gemini-5h`, `3p-weekly`,
  `3p-5h`, `gemini-argon-wtu-weekly`); 0.4.0's reader took two of them. Tests `nirvana_usage::tests::*` on that real
  answer; controls `usage_agy_*`, `usage_claude_*`, `usage_codex_*`.
  **If you remove this:** the foot and the seat engine see one Antigravity figure, Gemini's, and nothing of Argon or
  Google's Claude pool.
- **Changed:** a window the pool has that the reader did not print is unknown (`null`), never 0; a window the pool
  does not have, like Argon's 5 h, is absent. A reading is stale when read more than 300 s ago, or before a reset that
  has since passed; one read after its reset is fresh. A reader that fails keeps its last readings and marks the
  failure's time. Each window has its pace, `clamp((used + 0.10) / (elapsed + 0.10), 0.5, 2.0)`.
  **Because:** plan 12 §3's rules, each from an incident: old Nirvana read an unexposed window as 0 and deferred every
  order; its "stuck at zero for claude" was a reading past its reset; a passed reset must not loop on re-reads on an
  idle account (Argon's review, §4.6). §5.3 for pace.
  **Measured:** controls `usage_stale_age_ignored`, `usage_reset_passed_ignored`, `usage_reset_after_loops`,
  `usage_failed_read_wipes`, `usage_*_unknown_is_zero`, `usage_pace_no_ease`, `usage_pace_no_clamp`.
  **If you remove this:** a seat engine reads a missing window as free quota and an old one as now.
- **Hypothesis, not measured:** the pace constants (0.10, 0.5, 2.0), the mark's line (pace above 1.0) and the 300 s
  staleness line. They are named constants in `nirvana_usage.rs`.
- **Changed:** Claude's reset is a timestamp. Claude Code prints a wall clock in an IANA zone, "Oct 17, 8:50pm
  (America/New_York)"; herdr resolves it through Windows' own ICU (`icuin.dll`), the year from the zone's current one
  (next year when the date passed over a day ago), a time alone as today or tomorrow. The printed text is kept beside it.
  **Because:** pace and the reset-passed rule need the reset as a time. The fork's one time crate, `time` 0.3 with
  `formatting`, has no zone database, and ICU ships with Windows from 10 1703, older than the 1809 herdr's ConPTY
  needs, so no crate is added.
  **Measured:** `a_claude_reset_resolves_in_its_zone_and_year`: 8:50pm EDT on Oct 17 is 00:50Z, 9am EST on Dec 1 is
  14:00Z, a December read of "Jan 2" is next year, an unknown zone is no timestamp (ICU opens one as GMT without an
  error, so the name is checked first; control `usage_claude_reset_any_zone`).
  **If you remove this:** Claude's windows have no reset, so no pace and no reset-passed staleness.
- **Hypothesis, not measured:** on Linux and macOS a Claude reset keeps its text and no timestamp; the fork was not
  built there.
- **Changed:** `herdr usage [--json] [--refresh]` prints the readings: a table (pool, window, used, resets in, pace,
  fresh or stale with its age), or with `--json` the shape documented on `nirvana_usage::view`, for the plugin, the
  statusline and the seat engine. It reads `<home>/nirvana/usage.json`, which every window's readers write after
  each read (a temp file, then a rename; an entry is replaced only by one tried as late or later, since several
  windows share the file). A reader whose entry is missing or stale is run by the command itself, every reader with
  `--refresh`. Exit 1 only when no window has a figure. A new window shows the file's readings at once, with their
  age, instead of "reading".
  **Because:** plan 12 §3: "A new `herdr usage --json` prints them for the plugin, the statusline and the seat
  engine."
  **Measured:** live, `NIRVANA_HERDR_HOME` a scratch folder, `herdr usage --refresh`: all four readers in 3.6 s, exit
  0, every window fresh; the table is in plan 12 §15. Test `the_cache_round_trips_and_keeps_the_newer_entry`; controls
  `usage_cache_*`, `usage_json_*`.
  **If you remove this:** every consumer runs `claude -p /usage` and `agy -p /usage` itself, each through whatever
  shell it has (Git Bash rewrote `/usage` into a path twice on 2026-10-17, each a paid turn).
- **Changed:** Claude is read every 240 s while idle (600 s before) and agy every 240 s (300 s before).
  **Because:** under the 300 s staleness line, an open window keeps the cache fresh, so `herdr usage` rarely reads.
  **Measured:** `claude -p /usage` answered in 2.8 s for 0 tokens, `agy -p /usage` in 2.0 s (2026-10-17).
  **If you remove this:** on an idle machine `herdr usage` finds Claude's reading stale half the time and reads it
  itself.
- **Changed:** the foot has one usage row per pool: Claude, Gemini, Argon, G·Claude, Codex, Local, then any pool agy
  adds. Each shows its tightest window, the most used of those every seat of the pool spends (`week.fable` binds only
  Fable), as "41% 5h"; a peach ▲ when that window is ahead of pace; from 34 columns its reset, "2h58m"; a stale
  reading muted. The "5h 7D" head columns are gone. Google's Claude pool is "G·Claude": 8 cells, so the figure keeps
  its column at herdr's default 26, and the G mirrors plan 12's `g-opus` and `g-sonnet` seats.
  **Because:** plan 12 §8: "one row per pool, now with Google's Claude pool; each row shows its tightest window, a
  pace mark when ahead of pace, and the reset time when wide enough."
  **Measured:** `nirvana_footer` tests (`the_foot_lists_a_spaces_server_and_a_usage_row_per_pool`,
  `a_window_ahead_of_pace_carries_its_mark_in_the_warning_colour`, `a_stale_reading_is_muted`,
  `a_wide_sidebar_draws_a_bar_and_the_reset_for_the_tightest_window`); controls `footer_*`. Two upstream tests compose
  taller for the two extra rows, as 0.4.0's did: `active_agent_view_controls_sidebar_order_and_focus_indices` 30 to 32
  rows, `sidebar_scrollbars_use_proportional_shared_geometry_and_drag` 32 to 36 (34 still left a one-cell thumb).
  **If you remove this:** the foot shows Gemini as "Antigravity" and nothing of Argon or Google's Claude pool.
- **Hypothesis, not measured:** that the mark belongs to the shown window only. Live at 22:06Z, Claude's week read
  38% at pace 1.72 while its 5 h window, 44% at 0.98, was the tightest, so the row carried no mark.
- **Changed:** `a_short_sidebar_keeps_the_agents_panel` composes at 22 rows, not 20.
  **Because:** with six pools and a server the foot is 11 rows, taller than the sidebar's lower section at 20, so
  the foot was hidden whether or not the agents panel kept its five rows: control `footer_agents_starved` stayed
  green on the first run of this package's controls.
  **Measured:** at 22 and at 24 the test passes and the control reddens it.
  **If you remove this:** the test passes with the agents' floor removed.

## alpha 0.11.0 (2026-10-17): Accounts, a sign-in button at the foot of the sidebar

- **Changed:** the sidebar foot ends with an "accounts" button, plain grey text like the spaces list's "new" and
  "menu" (the owner, of the first "[ accounts ]": "have it looks like the text here"). It opens the Accounts panel at the bottom left of
  the panes: one row each for Claude Code, Codex and Antigravity, with who is signed in and a button ("switch" when
  someone is, "log in" when not). The button sends herdr's `command.invoke` with `nirvana.account_login:<cli>`; the
  server opens a focused tab, "log in <CLI>", running `herdr account login <cli>`, which runs the CLI's own sign-in
  in that terminal: `claude auth login`, `codex login`, or for Antigravity `agy`, into which herdr types `/logout`
  and Enter once agy's banner shows the old account, so agy's own sign-in follows. When agy exits, Enter starts it
  again. The tab ends by printing who is signed in now. `herdr account status` prints the three rows.
  **Because:** the owner, 2026-10-17: "In the bottom left let me manage my login accounts. Create a button so that
  this herder instance let me login to claude code or antigravity or codex. I currently have a new antigravity
  account I want to login to."
  **Measured:** `claude auth status` answers JSON in 0.2 s; `codex login status` answers on stderr, not stdout
  (the first reading showed Codex "not signed in" until stderr was read); agy 1.4.0 has no login command,
  `/logout` is refused in print mode (`agy -p /help` lists what print mode answers), and `agy -i "/help"` sends
  "/help" to the model instead of running it; typed into a running agy, "/help" and one Enter ran the command. agy's
  log names the account on every signed-in start ("OAuth: authenticated successfully as ..."). Tests
  `nirvana_accounts::tests::*`, `a_sign_in_opens_its_own_focused_tab_and_an_unknown_cli_opens_none`, the foot's
  `the_accounts_button_*` and `a_rows_button_*`; controls `accounts_*`.
  **If you remove this:** signing in to another account means finding each CLI's own command, and for Antigravity
  knowing that the way in is `/logout` inside agy.
- **Hypothesis, not measured:** that agy shows its sign-in screen after `/logout` rather than exiting. A real
  `/logout` signs the owner out, so the live check typed `/help` through `NIRVANA_ACCOUNT_TYPE` instead; the tab
  restarts agy on Enter for the case where it exits.
- **Changed:** the brand row has no space after "Nirvana"; the badge's own leading space separates them.
  **Because:** a two-digit minor version (" alpha 0.10.0 ", " alpha 0.11.0 ") needed 25 columns beside the toggle in
  the default sidebar and lost its closing pad. It has been clipped since 0.10.0: the leaf reading this feature's live
  screenshot reported the badge cut at the right, and `brand_row_names_nirvana_and_its_running_version` failed.
  **Measured:** that test passes at 0.11.0; control `header_name_pad`.
  **If you remove this:** the badge touches the toggle again from 0.10.0 on.

## alpha 0.10.0 (2026-10-17): the Jev memory, its own file, sized to the project

- **Changed:** the Jev guide is now the Jev memory, one file, `<home>/jev/JEV.md`, written from herdr's text the first
  time and the owner's from then on. Preload's row is "Jev memory": space switches it per folder as before, and `e`
  opens the file in the terminal editor in a new tab. Its front matter holds the sizes (`min_project_kb`,
  `read_file_kb`, `ask_file_kb`, `files_kb`, `output_lines`) and its body names them, so a size lives in one place.
  The body says when to ask Jev, by size, and the handoffs: `jev files` then only its read-first file, `jev text` on
  bug_in_code to the file, read before an edit, read on `failed`.
  **Because:** the owner, 2026-10-17: "maybe just have a tool flag in memory. I wawnt to also ensure I can toggle
  this so maybe have it a seperate file. This Jev memory preload file will give it instructions on how to make Jev
  tool chain handoffs and at what sizes it would make sence to invoke it."
  **Measured:** `nirvana_jev_memory::tests::*`; controls `jev_memory_*`. Live: an edit to `JEV.md` (min 400 KB, ask
  30 KB) changed what the next `jev guide` printed; in a herdr window the row read "JEV.md: a flag, 555 K..." at 179
  tokens with `edit`, the switch wrote the hook and a git-ignored `AGENTS.md`, and `edit` opened a tab titled
  "JEV.md - edit" holding the file.
  **If you remove this:** the guide is fixed text in the binary, the same for every project, and changing it means a
  build.
- **Changed:** a project under `min_project_kb` (1024 KB) gets one line, the tool flag: Jev is there, search and read
  as usual, ask it only about one big file or long output. At or over the line it gets the whole memory and a line
  measuring the project: its size and file count, and the largest three files. The size is what git tracks, or
  outside git a walk skipping Jev's excluded folders; binaries by extension never count.
  **Because:** two single runs on the 568 KB project (`nirvana/evidence/2026-10-16-jev-ab/single-results.jsonl`)
  saved nothing: with the guide Claude used Jev three times for +8% tokens; with a "search first" wording it used
  Jev never, and tokens swung +17% on noise. A plain search was as cheap as Jev wherever a word found the place.
  **Measured:** a real Claude Haiku session in a 555 KB copy quoted the flag; in a 1.6 MB copy it quoted the full
  memory's project line, from `AGENTS.md` and, with a `CLAUDE.md` beside it, from the `SessionStart` hook. agy 1.4.0
  quoted the same line. The full memory costs about 480 tokens more than the flag (41,922 against 42,405 input
  tokens). On this repository an image was among the largest three until binaries were left out. **The 1024 KB line is
  a hypothesis:** no A/B has run above 568 KB.
  **If you remove this:** every project gets the whole text, including those where it measured no saving.

## alpha 0.9.0 (2026-10-16): Jev token savers for Claude, Codex and agy, switched in Preload

- **Changed:** Jev, TypeSafe's cheap judge model reached through OpenRouter, runs from this binary for any agent with a
  shell: `herdr jev file <path> --ask`, `herdr jev files <glob>... --ask` (with the file to read first), and
  `<command> | herdr jev text --ask`, each with `--choice` or `--score`. The file text or output goes to Jev and never
  into the agent's context; the agent gets one line. What is never sent is Nirvana's list, checked in code before any
  call; command output goes with secret shapes redacted; a failure is never a yes. This binary never runs a command
  for an agent. Every call is logged without its text in `<home>/jev/calls.jsonl`, with the agent that made it.
  **Because:** the owner, 2026-10-16: "I would like to actually implement these token saving jev efatures for real.
  I know we only have them shadow tested and never ran Nirvana but lets actually put them to use in herder." Nirvana
  built levels 8 and 9 (`nirvanad/jev/files.py`) and registered them nowhere: 0 real calls.
  **Measured:** `nirvana_jev::tests::*`; controls `jev_*`. Live on the owner's key: one 300 KB file judged right in
  998 ms for $0.0033 (Nirvana's 24 KB cap was set before; the cap here is 256 KB); 18 files in parallel in 1.0 s,
  142k tokens of code judged for $0.0063. The A/B, `nirvana/evidence/2026-10-16-jev-ab/summary.txt` (recomputed by
  `summarize.py`): 46 of 46 runs answered right; with the guide, Claude and agy ran Jev by themselves.
  **If you remove this:** an agent learns about a file only by reading it into its context.
- **Changed:** Preload's first group, "Jev token savers (herdr)": **Jev guide** (off until switched) tells every new
  Claude, Codex and agy in the folder how to ask Jev: a `SessionStart` hook for Claude and a marked block in the
  project's `AGENTS.md` for Codex and agy. An `AGENTS.md` herdr made is kept out of git through `.git/info/exclude`;
  a project's own keeps its text and gets the block appended. **Compact advice** is the Stop hook below. **today**
  shows the day's Jev calls, the tokens no agent had to read, and the cost.
  **Because:** the owner: "Ad a section in the preload so we can toggle this I imagine every agent needs some sort of
  preload file to know how to save token with Jev", "ensure its compatible in claude code, agy and codex".
  **Measured:** live in a herdr window: the group drew first with its rows, a click on Jev guide wrote the hook and an
  `AGENTS.md` git did not see, a second click took both away. Codex 0.42.0's session record held the block (its
  sign-in is dead here, so no Codex model ran). agy ran `& "...herdr.exe" jev files` from PowerShell by itself.
  **If you remove this:** the tools exist and no agent knows to use them.
- **Changed:** Claude gets the guide once. The hook says nothing when Claude already reads herdr's block from
  `AGENTS.md`, which it does only when no `CLAUDE.md` or `CLAUDE.local.md` stands beside it.
  **Because:** measured 2026-10-16 with `/context` in a trusted folder: `AGENTS.md` alone loaded as a Project memory
  file at 469 tokens; with `CLAUDE.md`, or `CLAUDE.local.md` alone, it did not. The research leaf had measured
  "Claude never loads AGENTS.md" in a folder that had a `CLAUDE.md`. The A/B's Claude runs paid the guide twice.
  **Measured:** `claude_gets_the_guide_once_when_it_reads_agents_md`.
  **If you remove this:** a project with no CLAUDE.md pays about 470 tokens more in every Claude session.
- **Changed:** the guide says: after `jev files`, open only the `read first` file, and another only if that one does
  not answer. **Because:** with the first wording Claude ran Jev, then still read the files Jev was unsure of, and each
  read is paid again on every later turn. **Measured:** the 568 KB project, Claude Sonnet, finding the bug: no guide
  6 runs; first wording 3 runs, 2 used Jev, +5% tokens; this wording 6 runs, 6 used Jev, -16% tokens (summary.txt).
  The spread is wide (no guide 135k to 208k, this wording 128k to 218k): a lead, not a proof.
  **If you remove this:** Claude pays for Jev's answer and the reads it was meant to save.
- **Changed:** level 7, "Should I Compact", is a Claude Code Stop hook (`herdr jev compact`): at 150k, 300k and 500k
  tokens of context it asks Jev four questions and, at a clean break, shows the owner one line. Nirvana's port, with
  two changes: the reply judged is the hook's own `last_assistant_message`, and a silent reading rests one turn, not
  three. Every decision and Jev's raw answers go to `<home>/jev/compact.jsonl`.
  **Because:** Nirvana's `integrations/claude-code/compact/` was built and installed by nothing. Live 2026-10-16
  (lines lowered to 20k/25k/90k, Claude Haiku, three turns): the transcript lagged the reply, so a finished summary
  read at_boundary 0.07; and the three-turn rest skipped turn 3, the clean break to new work it exists for.
  **Measured:** after both changes, turn 3 said "Context is 44k tokens and you just finished a unit of work: /compact
  now keeps the summary clean." (`compact-live.jsonl`), at $0.00003 a call. Its lines and floors stay hypotheses.
  **If you remove this:** compaction is left to Claude Code's own threshold.
- **Changed:** a glob's `{a,b}` alternatives are expanded. **Because:** live, Claude asked
  `**/*.{ts,js,py,go,rs,java}` and got "no file to judge". **Measured:** control `jev_no_braces`.
- **Changed:** Restart Claude (the tab menu, and Preload's `r`) starts a Claude that has had no prompt yet fresh, with
  its flags, instead of `--resume <id>`. **Because:** live 2026-10-16, `r` after switching the guide on in a new Claude
  typed `--resume 2a85aa58-...` and Claude answered "No conversation found with session ID" and left the shell:
  Claude Code saves a conversation at its first prompt. `--continue` would open another conversation.
  **Measured:** `a_claude_with_no_saved_conversation_restarts_fresh`, control `restart_resumes_unsaved`; live, the same
  press started Claude again 21 s later with `--model sonnet --allowedTools ...` and no `--resume`.
  **If you remove this:** restarting a fresh Claude to take a Preload switch leaves the pane at a shell prompt.
- **Measured, the whole path in a herdr window** (scratch home, the 568 KB project, Claude Sonnet): Preload's clicks
  switched the guide and compact advice on, `r` restarted Claude, and asked the bug question Claude ran `herdr jev
  files` by itself (17 calls, 142k tokens judged, $0.0062, all logged as `claude`), read 40 lines of one file and
  answered right, at 62.6k tokens of context.

## alpha 0.8.0 (2026-10-16): the Memory square and the shared board

- **Changed:** each workspace has a shared board, `.nirvana/board.md` at the repository's root: a revision, one section
  per topic, each with its last writer. Any agent with a shell reads and writes it through this binary, `herdr board
  read` and `herdr board write <section> --expect <revision> --by <name> -- <text>`. A write naming a stale revision is
  refused (exit 3) and nothing is written; writers take a lock file, and a lock older than 15 s is a dead writer's.
  **Because:** the owner, 2026-10-16: "They seem to have a shared file among all the agents that when you update with
  one agent the other agents pick up on. Kind of like a shared state file", and "Keep a board per workspace." The
  pattern is durable-pi's App State (disler/durable-pi 37165c6): one document, a revision, a refused stale write.
  **Measured:** `nirvana_board::tests::*` (nine); controls `board_*`, `github_*`. Live: `board write Server --expect 0
  --by codex` gave revision 1; the same write again with `--expect 0` exited 3 with "board is at revision 1, not 0".
  **If you remove this:** agents working side by side learn of each other's changes only when you tell them.
- **Changed:** a running Claude is told what changed on the board on its next prompt. `herdr board notice`, run by a
  `UserPromptSubmit` hook, prints the sections changed or removed since that session last saw the board, and nothing
  when none did; a session's first prompt gets the whole board and how to write it. What each session saw is kept under
  the Nirvana-Herdr home, never in the project.
  **Because:** measured 2026-10-16 on Claude Code 2.1.301: an edit to CLAUDE.md or to a file it imports never reaches
  a running session, and a `FileChanged` hook's text never reaches Claude; a `UserPromptSubmit` hook's output does.
  **Measured:** live, Claude Haiku in a scratch repository: the first prompt's notice carried the board, and Claude
  answered "Port 5173" from it with no tool call; an unchanged board added nothing to the next turn (no hook output in
  the transcript); a section another writer added reached the next turn alone; Claude wrote a section itself with the
  command from the notice (`--expect 2`, revision 3). A change notice measured 666 characters, most of it the how-to;
  later notices now carry one line instead (`a_session_is_told_only_what_changed_since_its_last_turn`).
  **If you remove this:** a running Claude acts on a board it read minutes ago.
- **Changed:** the mod strip's third square, Memory: "board r3 · 1 new", "board r3 · not shared", "board empty". Its
  panel lists the sections, marks those this Claude has not seen, and names each one's last writer. Space adds or
  removes the hook in the folder's `.claude/settings.local.json` ("Share the board with every Claude here"); `e` opens
  `board.md` in the terminal editor tab of 0.7.0. The square shows only for a repository with a GitHub remote, read from
  its git config.
  **Because:** the owner: "The next potential feature is a memory manager", "flag this feature only if it opens a
  project with a github"; and "Looks good" to the board and the square first, lessons second.
  **Measured:** `the_memory_square_shows_only_in_a_repository_with_a_github_remote`,
  `the_memory_panel_lists_the_board_shares_it_and_edits_it`; controls `memory_*`, `share_*`, `square_ignores_seen`.
  Live: the square read "board empty", then "board r3 · 1 new"; space wrote the hook, and the Claude already running
  in the folder ran it on its next prompt with no restart.
  **If you remove this:** the board is read by opening the file.
- **Changed:** while a mod panel is open (Preload, Running or Memory), a pane's new output is drawn by composing the
  frame again, never written straight to the screen.
  **Because:** herdr writes a pane's output straight to the screen when nothing blocks that fast path, and the panels
  of 0.5.0 and 0.6.0 did not block it: live on 2026-10-16 the open Memory panel was painted over while Claude answered.
  Every panel since 0.5.0 had the defect; it showed only now because the panel stayed open while an agent printed.
  **Measured:** `an_open_panel_is_redrawn_when_its_agent_prints` (with no panel open the patch still takes the fast
  path); control `panel_painted_over`.
  **If you remove this:** an agent that prints while a panel is open paints over the panel.
- **Changed:** control `panels_both_open` sabotages the Running toggle's line that closes Preload, beside the new
  line that closes Memory.
  **Because:** the new line sat inside its find text, which then occurred 0 times.

## alpha 0.7.0 (2026-10-16): Preload grouped by provider; edit opens a terminal editor in a new tab

- **Changed:** the Preload panel groups its rows by where each comes from: This project, You (~/.claude), one group per
  plugin (its own switch first, then its skills, agents and MCP servers), claude.ai, Built in (Claude Code), Managed.
  Each header shows its group's tokens; each row's detail now names its kind ("memory file", "skill", "MCP, 2 tools",
  "connector, 1 tool", "agent", "tool", "whole plugin").
  **Because:** the owner, 2026-10-16: "Organize the list by provider or built in."
  **Measured:** `each_item_is_grouped_under_the_provider_it_comes_from`,
  `a_switched_off_item_nothing_remembers_is_grouped_by_its_switch`,
  `the_panel_lists_each_group_and_item_with_its_tokens`; controls `provider_*`. A header sums its rows; a plugin's own
  row is left out of its header, since it sums the same rows; Built in adds "System tools", which `/context` does not
  split per tool. The "MCP server instructions" category is no longer shown: `/context` does not split it per server.
  **If you remove this:** a plugin's skills, agents and server are spread over four groups, and what a plugin costs is
  read by adding them up.
- **Changed:** a row switched off keeps its group: the window remembers each row's provider with its tokens; with
  nothing remembered (herdr restarted since), the switch names it (a `plugin:` server is its plugin, a `claude.ai `
  server is claude.ai, a CLAUDE.md under `~/.claude` is You).
  **Because:** a row switched off is gone from `/context`, which is where its provider is read.
  **Measured:** controls `provider_off_item_forgotten`, `provider_off_switch_guess`.
  **If you remove this:** switching a row off moves it to another group.
- **Changed:** "edit" opens the file in a terminal editor that takes the mouse, in a new tab of the panel's space,
  focused and named "edit <file>"; the agent's tab is left as it was. The editor is `NIRVANA_EDITOR` when set, else the
  first found of Microsoft Edit (`edit` on Windows, `msedit` elsewhere), `micro`, `nano --mouse`. With none, the panel
  says so; Notepad is never opened.
  **Because:** the owner, 2026-10-16: "when you edit have a terminal editor that is mouse compatible and open that file
  in a new tab not open through notepad." Microsoft Edit 1.2.1 is part of Windows 11 on this machine, so nothing is
  installed. `$VISUAL`/`$EDITOR` are not read: either may name a window editor.
  **Measured:** `edit_opens_a_terminal_editor_that_takes_the_mouse_never_notepad`,
  `edit_opens_the_file_in_a_new_focused_tab_and_leaves_the_agents_tab_alone`; controls `editor_*`, `edit_tab_*`.
  **If you remove this:** "edit" opens Notepad, over the agent's tab.
- **Changed:** "edit" no longer goes through herdr's scrollback editor helper.
  **Because:** that helper is written for herdr's scrollback temp file: on Linux and macOS it runs
  `rm -f "$scrollback_file"` when the editor exits (`src/platform/linux.rs`), so 0.5.0's "edit" there would have
  deleted the CLAUDE.md it opened. Found while building 0.7.0; never run on Linux or macOS, so never measured.
  **If you remove this:** on Linux and macOS, editing a preloaded file deletes it.
- **Changed:** control `spaces_kind_token_ignored` sabotages `.find(|(name, _)| name == KIND_TOKEN)`.
  **Because:** its find text had been gone since 0.5.0 moved the token read, so it printed "find text occurs 0 times"
  and proved nothing. Found by checking every control's find text before this run.
  **If you remove this:** the agent-icon test has no control.

## alpha 0.6.0 (2026-10-16): the Running square, Running on top and Waiting on you under it

- **Changed:** the mod strip's first square is "Running": "working · Bash", "waits on you" in yellow, "idle". Its
  panel shows Running on top (the task, the last prompt, the tool in flight and for how long, Claude's last words) and
  Waiting on you under it: the prompt Claude is waiting on, read from its pane, each numbered option, "(Recommended)" in
  green. A digit or a click answers it in the pane.
  **Because:** the owner, 2026-10-16: "Marge the running and waiting on you into one panel the top being running and
  what is actively being run with a summary. Then underneath that running is the waiting on you." Asked how Waiting on
  you should behave first, the owner picked "Show and answer"; building the recommended option ahead comes later.
  **Measured:** `nirvana_mods::*` (six tests for this square), `nirvana_running::tests::*` (six),
  `claudes_state_mark_is_not_part_of_the_task`, `a_claudes_transcript_rides_in_the_agent_tokens`; nineteen controls,
  `running_*`, `prompt_*`, `strip_wait_not_yellow`, `strip_blocked_reads_idle`, `answer_*`, `panels_both_open`,
  `title_mark_kept`, `transcript_token_dropped`. The live run is in the plan's Record.
  **If you remove this:** what a Claude is doing and what it waits on is read by switching to its pane.
- **Changed:** Waiting on you reads the prompt from the pane's screen, never from the transcript, and only while herdr
  says the agent is blocked.
  **Because:** measured 2026-10-16: an AskUserQuestion call reaches the transcript only after it is answered, and a
  permission dialog's options change with the case ("Yes, and always allow access to <folder>", "switch to auto
  mode"), so only the screen says what each digit means. One digit picks the option, no Enter.
  **Measured:** `a_question_on_screen_reads_as_its_heading_and_options`,
  `a_permission_dialog_reads_with_its_command`, `a_screen_with_no_waiting_agent_answers_nothing`; controls
  `prompt_heading_past_rule`, `running_prompt_while_working`, `answer_with_enter`.
  **If you remove this:** the panel offers answers to a question it cannot see, or sends a digit into Claude's prompt
  box while it works.
- **Changed:** the answer is typed into the agent's pane through the window's own key path (a pane input event for
  that pane), not sent as an API method.
  **Because:** the first live run of 2026-10-16 sent `pane.send_keys` and herdr answered "Action unavailable": a window
  may send only the methods herdr lists for windows, and neither `pane.send_keys` nor `agent.send_keys` is one. The
  tests had passed, because a test window knows no such list; the test now checks the request a real window sends.
  **Measured:** second live run: the panel's "1" reached Claude, whose transcript recorded "Red (Recommended)", and herdr
  read the agent `idle`; `a_waiting_prompt_is_read_from_the_pane_and_a_digit_answers_it`; control `answer_wrong_digit`.
  **If you remove this:** the panel says it answered and Claude keeps waiting.
- **Changed:** the server sends each Claude's transcript path as the agent token `nirvana_transcript`; the window reads
  the transcript itself every 2 s while the strip shows that agent.
  **Because:** herdr rebuilds a window's snapshot when an agent's state changes, not while it works, so a tool that
  starts mid-turn would wait for the next state change. The path stays the same for the whole conversation.
  **Measured:** `a_claudes_transcript_rides_in_the_agent_tokens`, `a_tool_call_without_a_result_is_what_runs_now`.
  **If you remove this:** Running shows the turn as it was when the agent last changed state.
- **Changed:** a task title no longer carries Claude's state mark ("✳ ", a spinner glyph).
  **Because:** the spaces list read "✳ Button colour que…" in the live run of 2026-10-16.
  **Measured:** `claudes_state_mark_is_not_part_of_the_task`; control `title_mark_kept`.
  **If you remove this:** every Claude task row starts with a mark that is not the task.

## alpha 0.5.0 (2026-10-16): a mod strip under the tab bar, with one mod, Preload

- **Changed:** a tab running Claude Code has a one-row strip under the tab bar, the panes a row shorter. It holds one
  square, " Preload ", with the total a fresh Claude in the agent's folder preloads and "N off" when items are switched
  off there. A click opens the Preload panel: every CLAUDE.md, memory file, skill, MCP server and connector, plugin,
  agent, built-in tool and the hooks' session-start text, each with its tokens; space switches an item off or on for
  every new Claude in the folder, `e` opens its file in the editor, `r` restarts this Claude so it takes the change.
  **Because:** the owner, 2026-10-16: "I want to add the ability to add componoent mods like in Nirvana for each agent.
  ... I wawnt just one mod square for the herder version for now. This mod square is called Preload and it will list all
  the tokens it will preload for this fresh and every Claude session. ... I want to be able to see them and toggle them
  or modify them if I cant remove unselect them."
  **Measured:** `nirvana_mods::*` (eight rendering, key and click tests), `nirvana_preload::tests::*` (six),
  `only_an_existing_absolute_markdown_file_is_editable`; twenty-three controls, `preload_*`, `strip_*`, `panel_*` and
  `edit_any_extension`. The live run is in the plan's Record.
  **If you remove this:** what a Claude session starts with is read with `/context` inside each session, and switching
  an item off means editing Claude's settings by hand.
- **Changed:** the list is Claude Code's own `/context`, run in the folder as `claude -p /context
  --no-session-persistence --verbose --output-format json`, read from its `context_usage` and `init` events.
  **Because:** Claude Code answers it locally: 0 tokens, a cost of 0, in 2.6 s (2026-10-16). `--verbose` gives the
  tokens as integers and the servers, plugins and tools by the names their settings take; without it the answer is a
  markdown table.
  **Measured:** `the_report_reads_into_its_total_and_items` on a report cut from a real run.
  **If you remove this:** the panel guesses at what Claude loads, and its names stop matching the settings.
- **Changed:** each switch is the setting proven to remove that item from a new session's `/context`, written to the
  folder's `.claude/settings.local.json`: `claudeMdExcludes`, `autoMemoryEnabled`, `skillOverrides`, `enabledPlugins`,
  `deniedMcpServers`, `permissions.deny`, `disableAllHooks`. Only the item's entry is written; switching back removes it.
  **Because:** 96 free `/context` runs on 2026-10-16 ("Sonnet - measure Claude preload toggles"). `permissions.deny`
  on an MCP server hid its tools but left it connected with its instructions loaded, so servers use
  `deniedMcpServers`. A plugin's own skill and a custom agent have no switch of their own; the panel says so.
  **Measured:** `each_kind_of_item_gets_its_proven_switch`,
  `switching_writes_only_the_items_entry_and_back_leaves_the_file_as_it_was`,
  `the_local_settings_file_round_trips_and_a_broken_one_is_left_alone`; controls `preload_mcp_by_permission`,
  `preload_empty_left`, `preload_broken_file_replaced`.
  **If you remove this:** a switch that looks off leaves the item loaded, or the owner's settings file is rewritten.
- **Changed:** `command.invoke` with `nirvana.edit_file:<path>` opens an existing absolute `.md` file in herdr's
  scrollback editor, in a zoomed pane.
  **Because:** "or modify them". Every file Preload offers to edit is markdown; the check keeps the command from
  opening anything else a client names.
  **Measured:** `e_asks_the_server_to_open_the_items_file`, `only_an_existing_absolute_markdown_file_is_editable`;
  control `edit_any_extension`.
  **If you remove this:** "edit" has nothing to open the file with.

## alpha 0.4.0 (2026-10-15): running servers and usage at the foot of the sidebar

- **Changed:** the foot of the expanded sidebar, under the agents panel, holds old Nirvana's two blocks. "servers": one
  row per port a process in a space's panes listens on, the space's name and `localhost:<port>` (`:<port>` when the
  name needs the cells); a click opens it in the browser; the block hides when no space runs a server; four rows,
  then "+N more". "usage": Claude, Codex, Antigravity and Local, each with Nirvana's provider colour, the five-hour and
  seven-day windows in two columns under "5h 7D", and from 34 columns a bar for the five-hour window.
  **Because:** the owner, 2026-10-15: "Lets work on the next feature. This running server and usage from Nirvana lets
  port it to the Nirvana herder".
  **Measured:** `nirvana_footer::*` (eight rendering and click tests), `nirvana_servers::tests::*` (seven),
  `nirvana_usage::tests::*` (six); twenty-three controls, `servers_*`, `usage_*` and `footer_*`. The live run is in the
  plan's Record.
  **If you remove this:** which space runs a server, and how much of each subscription is left, is read outside herdr.
- **Changed:** a server is any TCP port listened on inside a space's panes (the shell and its whole tree), read by the
  server on one worker thread every 3 s and sent as the space's `nirvana_servers` token, so the wire is unchanged.
  Ports from 49152 up and ports owned by claude, codex, agy or Antigravity are not servers.
  **Because:** herdr has no daemon that starts servers, which is where old Nirvana's list came from. The live check
  found an idle Claude Code on 64839 and agy's app on 60015 and 60016, which a plain port scan would list.
  **Measured:** `plumbing_ports_are_not_servers`; controls `servers_dynamic_port_kept`, `servers_agent_port_kept`.
  **If you remove this:** each Claude, agy and helper adds a row that is not a server.
- **Changed:** the worker keeps reading a space while any of its pane shells runs, not only while a window asks.
  **Because:** the server rebuilds a window's snapshot when the sidebar's state changes, not on pane output. In the
  first live run a server's row stayed after the server stopped: the space had gone unasked for over 60 s, so the
  worker had dropped it, and the row never cleared.
  **Measured:** second live run, server idle 75 s then stopped: its row was gone 8 s later (plan Record);
  `a_space_is_read_while_a_shell_lives`, control `servers_dead_space_kept`.
  **If you remove this:** a server that stops after a quiet minute keeps its row until something else changes.
- **Changed:** the usage rows are read by the window, never the server: Claude by `claude -p /usage
  --no-session-persistence` (every 120 s while a Claude in the window works, else 600 s), Codex from its rollouts'
  `rate_limits` (30 s), Antigravity by `agy -p /usage` (300 s), Local from Ollama on 127.0.0.1:11434 and `nvidia-smi`
  (30 s). A failed read keeps the last figure.
  **Because:** Claude Code answers `/usage` locally: the live check returned model `<synthetic>`, 0 tokens, a cost of 0,
  in 2.7 s. Old Nirvana paid a model turn for its Claude reading. `--bare` drops the subscription sign-in and answers
  with the API cost view, so it is not used. agy's read took 20.8 s.
  **Measured:** the parsers' tests on text captured from this machine on 2026-10-15; `a_failed_read_keeps_the_last_figure`.
  **If you remove this:** the rows say "reading" forever.
- **Changed:** upstream's `sidebar_scrollbars_use_proportional_shared_geometry_and_drag` composes at 32 rows, not 26.
  **Because:** the foot takes six rows from the agents panel, which at 26 left its ten agents a one-cell thumb.
  **Measured:** at 32 both lists still overflow; the test asserts both tracks.
  **If you remove this:** that upstream test fails on the fork.

## alpha 0.3.0 (2026-10-15): each space lists its agents' task and model, with the agent's icon

- **Changed:** under each space in the spaces list, two rows per agent running there: the task's summarized title
  (Claude Code's terminal title, "No task yet" before the first prompt), then the agent's icon and its provider and
  model, e.g. "✻ Claude Sonnet 5.5". The icon takes the place of the status circle and keeps its colour (idle green,
  working yellow, blocked red, done teal). Icons: Claude ✻, Codex ❖, Antigravity ▲, Local ⌂; any other agent keeps
  herdr's dot. A space shows three agents, then "+N more"; the agents panel below still lists them all.
  **Because:** the owner, 2026-10-15: "See how in our old Nirvana we showcase the task a summerised title and its active
  model ... for herder it would be the model provider and tyle ie Claude Sonnet. ... Basically I would to have spaces do
  somethign similar and on the left Instead of a green circle its the agents Icon. We need one for claude antigravity
  codex and local."
  **Measured:** `nirvana_space_agents::*` (six rendering tests) and `nirvana_spaces::tests::*` (ten reader and label
  tests); eighteen controls, `spaces_*`. The live run is in the plan's Record.
  **If you remove this:** the spaces list shows only folder names; which agent and model works where is read pane by
  pane.
- **Changed:** the server reads each agent's model off its main loop: Claude from its transcript's last reply (found
  through `<config>/sessions/<pid>.json`), Codex from `--model` or its newest rollout in the pane's folder, Antigravity
  from `--model` or its log's last model switch. A Claude on a non-Anthropic model (Nirvana's local seat through
  Ollama) is Local. The result rides in the agent's existing `tokens` as `nirvana_kind` and `nirvana_model`, so the wire
  format is unchanged and an older server still talks to this client.
  **Because:** a model read on the server's loop would stall every pane's output; a new snapshot field would break an
  older server with a newer client, which the 0.1.0 server and 0.2.0 client did for a command on 2026-10-15.
  **Measured:** reads are cached for 3 s and run on one worker thread that wakes a render only when a model changed.
  **If you remove this:** the client has no model to draw.
- **Changed:** upstream's `sidebar_scrollbars_use_proportional_shared_geometry_and_drag` composes at 26 rows, not 20.
  **Because:** its ten agents now also draw under their space, so at 20 rows the spaces thumb is one cell.
  **Measured:** at 26 both lists still overflow; the test asserts both tracks.
  **If you remove this:** that upstream test fails on the fork.

## alpha 0.2.0 (2026-10-15): Restart Claude from the tab menu, same folder, same conversation

- **Changed:** a tab whose pane runs Claude Code has "Restart Claude" in its right-click menu, between Rename and
  Close. It stops that Claude and its children, waits up to 3 s for the pane's shell, and types
  `claude <the old flags> --resume <session id>` into it, keeping the herdr agent name. The id comes from Claude Code's
  own `<config>/sessions/<pid>.json`; with no file the restart uses `--continue` instead. The menu sends herdr's
  existing `command.invoke`, so herdr's API and protocol are unchanged.
  **Because:** the owner, 2026-10-15: "If i right click this tab and I have an option to restart the claude code
  insstance with that exact directory opened and that session resumed." Their screenshot's Claude read "Update
  installed · Restart to update".
  **Measured:** `a_tab_running_claude_offers_restart_claude`, `restart_claude_asks_the_server_for_that_tab`,
  `restart_claude_reaches_the_server_and_stops_nothing_without_claude`,
  `the_session_file_names_the_running_claudes_conversation`, `a_session_file_left_by_another_process_is_not_used`,
  `restart_keeps_the_flags_and_resumes_the_same_conversation`, `terminate_really_stops_a_running_process`,
  `the_restart_command_is_typed_as_a_plain_line`; eleven controls, `restart_*`, `session_*`, `terminate_query_only`.
  The live runs are in the plan: the first two found that herdr's Windows kill is refused (query-only handle) and that
  a stopped Claude leaves bracketed paste on; both are fixed in the fork's own code.
  **If you remove this:** restarting Claude means quitting it, finding the conversation id, and typing the resume
  command with the Nirvana theme flags by hand.

## alpha 0.1.0 (2026-10-15): the Nirvana header, its version, and reload onto the newest build

Base: herdr master b431a063 (Cargo version 0.9.5).

- **Changed:** the sidebar's first row is the brand row: a teal ◆, "Nirvana" in bold, a badge "alpha 0.1.0", and the
  sidebar toggle « at its right edge (it was at the bottom right). " spaces" moved to the second row, which upstream
  left blank, so the workspace list keeps its place.
  **Because:** the owner, 2026-10-15: "have this element for the Herdr instance [Image #11]" (Nirvana's header).
  **Measured:** `brand_row_names_nirvana_and_its_running_version`, `a_narrow_sidebar_keeps_the_toggle_and_clips_the_badge`;
  controls `header_drops_version`, `header_badge_clipped` (the live run found the badge one column short).
  **If you remove this:** the window no longer says which build it runs.
- **Changed:** the fork has its own version in nirvana/VERSION, compiled in as NIRVANA_VERSION, bumped by
  nirvana/version.py. herdr's Cargo version is left alone.
  **Because:** Nirvana's rule 16, "That way I can see if the Nirvana window is running the new version"; herdr's
  version gates its update manifest and protocol checks, so moving it would break both.
  **Measured:** `version_comes_from_the_version_file`, `VersionTest`; controls `version_not_from_file`,
  `bump_feature_keeps_fix`.
  **If you remove this:** the badge has no number of its own to show.
- **Changed:** the server's background version check reads `<home>/bin/nirvana-version.txt` every 5 s instead of
  herdr's release manifest every 30 min, and announces each newer build once through herdr's existing
  `UpdateReady`. The badge then reads "↻ 0.1.1"; a press arms it ("restart?"), a second press detaches, stops the
  server (it saves the session), and exits 75; the `nirvana-shell` supervisor starts the installed build, which
  starts a new server that restores the session. `herdr update` refuses.
  **Because:** "hot realoding the newest changes", read from Nirvana's Reload. Two presses because a restart stops
  the programs in the panes: Windows has no live handoff (src/platform/mod.rs, "Windows has no live handoff").
  **Measured:** `the_stamp_beside_the_installed_build_is_read`, `only_a_later_build_is_newer`,
  `a_newer_build_turns_the_badge_into_a_reload_button`, `reload_takes_two_presses`, `InstallTest`; controls
  `header_reload_unclickable`, `reload_single_press`, `same_build_counts_as_newer`, `install_stamps_wrong_version`,
  `install_overwrites_old_build`. The live reload run is recorded in the plan.
  **If you remove this:** a new build sits unused until someone stops herdr by hand; with the update check back on,
  herdr would offer to install upstream herdr over the fork.
- **Changed:** the default window title is "{workspace} · Nirvana" (was "{hostname}: {workspace}").
  **Because:** the owner's image reads "Downloads · Nirvana": the project in view, then the product.
  **Measured:** `nirvana_default_title_names_the_project_then_nirvana`; control `title_upstream_default`.
  **If you remove this:** the Windows Terminal tab shows the machine name instead of the project.
