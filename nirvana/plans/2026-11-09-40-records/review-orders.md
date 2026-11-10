# Review: area "orders" (Nirvana-Herdr at 86b37c7e, read only)

Read end to end: nirvana_worker.rs (launch, stream, exec, rate limit, seat run), nirvana_gate.rs (axes, git, run_command, gate_subject, send_back, briefs), nirvana_run.rs (journal, wake, end/close), nirvana_claude.rs (hooks), nirvana_job.rs (run_exec, stop), nirvana_media.rs (whole), nirvana_bladebro.rs, nirvana_gui_tools.rs (launch half), nirvana_agy_tools.rs (with_tools, remove), nirvana_session_record.rs (whole), worker/view.rs (report lines). Skimmed, not traced: the stream-reading heuristics of the sources axis (gate.rs 600-1500), nirvana_gate tests, nirvana_run receipt/queue text, nirvana_job panel/cache half (1-590), place.rs. No cargo, no claude/agy, nothing changed.

Counts: B 10, M 3, T 5, D 5, X 3 (nothing dropped for the caps).

## B findings (most severe first)

### B1. A reseat child is not part of its parent's chain: the held-out check, the seat snapshot and the send-back cap are lost
- nirvana_gate.rs:2799-2812 `chain()` follows `parent` only while `why == "send_back"`. A rate-limit reseat child is seated with `why = "reseat"` (nirvana_worker.rs:2297), so `chain(state, "o1-r1")` returns `("o1-r1", 0)`. Every other module follows both: `helps_of` (gate.rs:4118 `"send_back" | "reseat"`), `nirvana_learning::CARRIES_ON`, `nirvana_review.rs:300`.
- Scenario: build order `o1` (worktree, feature size) has a held-out check written before the build (`gate/o1.json` holds `heldout`). `o1` runs out of Max 5 h window, is reseated as `o1-r1`, delivers. `gate_subject` (gate.rs:3312) takes `root = "o1-r1"`, `read_sidecar(.., "o1-r1")` (3363) has no `heldout`/`authors`, so the held-out axis becomes the Note "no held-out check was written before the build" (3384-3393) and the order passes without the check that was written for it. Same root error:
  - `order_changes` (2365) reads `seat_snapshot_path(.., &root)`: for a project-folder order (research, media, ops) the snapshot belongs to `o1`, not `o1-r1`; it falls to the time rule (`nanos >= seated_at` of the child), so files the first worker changed outside `owns:` before the reseat have older mtimes and are not counted. The owned axis passes.
  - `plan_send_back` (2821) counts depth from the child, so the 3-send-back cap restarts after each reseat.
- Fix: in `chain`, follow `send_back` and `reseat` to the root, count only `send_back` in `depth`. Check `plan_send_back` ids (`<root>-s<n>`) stay unique (they search for a free id).
- Test: journal with `o1` (sidecar `heldout` set), `o1-r1` (`why=reseat`, parent `o1`); `chain(state,"o1-r1") == ("o1", 0)`; a gate of `o1-r1` returns a `heldout` axis that is not the "none written" Note. A second test: `o1-r1` project-folder order, snapshot for `o1` lists a dirty unowned file, expect the owned axis to fail.
- Confidence: confirmed by reading the path end to end; not run.

### B2. `run_command` times out but then waits for its grandchildren: the gate hangs and holds the project gate lock
- nirvana_gate.rs:2172-2185 (`child.kill()` on timeout, then `out.join()` / `err.join()` at 2184-2185). `kill` ends the direct child only (on Windows the retry path is `cmd /C <test>`, and `cargo test`, `npm test`, `pytest` all start children). The pipe readers (2150-2156) run `read_to_end` until EOF, and EOF needs every inheritor of the write end to exit. A hung test binary that outlives `cmd`/`cargo` keeps the pipe open, so `join()` never returns.
- Scenario: `test: cargo test` where one test deadlocks. At 1200 s `cargo.exe` is killed, `herdr gate run` still blocks on `join()`. `gate()` holds `.gate.lock` (gate.rs:3145, 2792) for the whole time, so every other gate in the project, the reviewer gates, `nirvana_review` (1370) and the idle review (`settle`) wait on it with `file.lock()` (no timeout).
- Fix: run the test in a job object (the worker's `job` module in nirvana_worker.rs:1212 does exactly this and is private: lift it to a shared module, create suspended, adopt, `kill()` on timeout), and join the readers with a short deadline after the kill.
- Test: `run_command(cwd, "cmd /C start /B ping -n 60 127.0.0.1 && ping -n 60 127.0.0.1", 1 s)` returns in under 10 s with `timed_out`.
- Confidence: confirmed by reading (the join order); the Windows handle inheritance is standard.

### B3. One non-UTF-8 byte on a worker's (or a job's) output ends the reader, then the supervisor blocks in `child.wait()`
- nirvana_worker.rs:1419-1426: `BufReader::lines()` returns `Err(InvalidData)` for a line that is not UTF-8, the loop does `else { break }`, then sends `Line::Done`. The main loop treats `Done` as the process end (1496-1498) and calls `child.wait()?`, which blocks forever if the child is still running (it now writes to a pipe nobody reads, so it stalls when the 64 KB buffer fills). The stall check lives only in the `Timeout` arm (1513), so it never runs. The stderr reader has the same break (1433); that case ends as a stall after 600 s, not a hang.
- Same pattern in nirvana_job.rs:1099 and 1112 (`lines().map_while(Result::ok)`): the reader thread ends at the first bad line, the job's pipe fills, the job hangs as "running" with its log frozen. Typical source: a tool that prints in the OEM code page (cargo with a non-ASCII path, msbuild, dotnet) in a `herdr job run`.
- Fix: read with `read_until(b'\n')` into a `Vec<u8>` and `String::from_utf8_lossy`, never break on a decode error. After `Done`, wait with the same stall rule instead of a bare `wait()`.
- Test: stand-in command (`TEST_COMMAND`) that prints `{"type":"assistant"...}`, then a line with byte 0xFF, then the `result` line: `exec` must deliver. For the job: a script that prints 0xFF then 200 KB; `run_exec` must return its exit code.
- Confidence: confirmed by reading; claude's own stream is UTF-8, so the live trigger is stderr/agy/jobs; plausible for the supervisor, confirmed for jobs.

### B4. The "named check never ran" verdict is read from output cut to the last 40 lines
- nirvana_gate.rs:374-387 (`filter` keeps check lines, then `kept[len-KEPT_LINES..]` drops the oldest) and 420-423 (`absent` is computed from `lines`, the cut list). `keep_line` also keeps any line holding "passed", "ran ", "error", "fail" (case-insensitive).
- Scenario: `check: test_gate_lock` and `test: pytest -v`. The check passes early; pytest then prints 70 `... PASSED` lines (each matches "passed"). The kept list is 71 lines, the first 31 are cut, the check line is among them, so the axis fails "never ran: a suite total cannot see a check that never ran" and a correct order is sent back (a paid redo, and a send-back counts toward the cap of 3).
- Fix: compute `absent` over `run.output.lines()` (the full text, which `deterministic` already logs whole) and keep the named check lines out of the tail cut (cut only the other lines). The CLAUDE.md rule 9 intent (a check that never ran fails) stays.
- Test: output = check line + 60 lines of "x PASSED", exit 0: expect Pass. Sabotage: a run that lacks the check name still fails.
- Confidence: confirmed by reading.

### B5. `commit_work` reads git's quoted paths: a non-ASCII untracked file makes the gate commit fail
- nirvana_gate.rs:2400-2426. `git status --porcelain=v1` without `-z` quotes names with non-ASCII bytes (`"caf\303\251.md"`, `core.quotepath` default). The code does `trim_matches('"')` only, then `git add -- 'caf\303\251.md'`, which matches nothing and errors, so `deterministic` returns the axis `rebase: the worker's work could not be committed` (2584-2592). `uncommitted()` (2271) was fixed for exactly this with `-z` (comment "P35a review S3"); `commit_work` was missed. Only untracked files are affected (tracked changes go through `add -u`). The `owns:` glob also sees the escaped spelling.
- Fix: same `-z` parse as `uncommitted`.
- Test: worktree with an untracked `café.md`, `owns: *.md`: `commit_work` commits it.
- Confidence: confirmed by reading; depends on a non-ASCII file name (MotoProject file names are the likely source).

### B6. `test:`, `check:` and `serves:` are matched case-sensitively; a `Test:` line silently drops the tests axis to a Note
- nirvana_gate.rs:216-241. `owns:` goes through `strip_key` (case-insensitive) after the 2026-10-31 incident, the other three through `strip_prefix`. An order with `Test: cargo nextest run` reads as "the order names no test command" (tests_axis 392-404, Mark::Note unless `check:` also given), so the delivery passes with no test run. `Check:` is dropped the same way.
- Fix: `strip_key` for all four keys. Test: `spec("Test: x\nCheck: y\nServes: 80")` yields all three.
- Confidence: confirmed by reading; depends on how the orchestrator capitalises.

### B7. The 600 s stall rule equals Claude Code's longest foreground tool call
- nirvana_run.rs:47 (`STALL_AFTER_S = 600`), nirvana_worker.rs:1520-1525. stream-json prints nothing while a Bash tool runs. A worker that runs a 9 to 10 minute foreground build/test (this repository's builds are that long) plus its next model turn passes 600 s of silence, so `job.kill()`, a `Stalled` event, a restart with `--resume` after 0/30/60/120 s, and at the fourth stall the "crash loop" question. Each restart pays a new turn.
- Fix: do not count silence while a tool call is open (`Stream.calls` non-empty and `last_call` not answered), up to a longer ceiling (for example 30 min); keep 600 s for model silence. Test: a stream whose last line is an unanswered `tool_use` is not stalled at 700 s but is at 1900 s.
- Confidence: plausible (needs a tool call near 10 min; I did not find a measurement in the CHANGELOG for it).

### B8. `herdr media` names `UNC\srv\share\...` for a network file
- nirvana_media.rs:289-296 `strip_verbatim` strips `\\?\` only. `std::fs::canonicalize` of `\\srv\share\v.mp4` returns `\\?\UNC\srv\share\v.mp4`, which becomes `UNC\srv\share\v.mp4`, a relative-looking path; `prompt_line` (411) puts it in the agy prompt as the `AbsolutePath`, `view_file` refuses it, and agy ends with an empty answer and `status: SUCCESS`, the exact P2 failure the module exists to avoid. `probe` and `shrink` pass the same path to ffprobe/ffmpeg, which fail. The correct handler is already in nirvana_session_record.rs:51 (`\\?\UNC\` to `\\`).
- Fix: one shared `strip_verbatim` (see D2). Test: `absolute_media_path` of a `\\?\UNC\srv\share\v.mp4` input (pure helper test on the string function).
- Confidence: confirmed by reading; needs a recording on a share.

### B9. The pane's report summary never shows the tests
- nirvana_worker/view.rs:356 reads `value.get("tests").and_then(Value::as_str)`. The Work Report schema (worker.rs:44, `check_report` 106-124) makes `tests` an array of `{name, result}`, so the branch is dead and the "tests:" line is never drawn for any conforming report. Fix: count `pass`/`fail`/`not_run` and draw `tests: 3 pass, 1 fail`. Test: `report_lines` on a schema-valid report with two tests.
- Confidence: confirmed. Cosmetic, but it hides the one place a reader sees a worker's own test claim.

### B10. Herdr calls made by the supervisor have no timeout while it holds the live lock
- nirvana_worker.rs:2634 `herdr_json`, 2700 `herdr_ok` and `pane_title` use `.output()` with no limit (only the dead-pane sweep has `herdr_json_within`). In `exec` the pane close block (1883-1933) runs after `Delivered`, after the wake, with `_live` still held (1672). A herdr server that accepts and never answers (the memory notes record wedged loopback drops) leaves the supervisor alive: `live()` stays true, the status foot reads the order as running, and the pane never closes. Same shape in nirvana_run.rs `close_run_tab` (2663-2735) and nirvana_job.rs:1226.
- Fix: route these through `output_within(.., 10 s)`; on timeout skip the close. Test: stand-in herdr that sleeps; `exec` returns.
- Confidence: plausible (no observed incident).

## M findings (model and seat drift)

### M1. Guard messages hard-code "Opus 5.5" and a Sonnet worker name
nirvana_claude.rs:921 and 944-945: "a fork always runs on your own seat (Opus 5.5 at your effort)", "would inherit your seat (Opus 5.5 ...)", example `nirvana:worker-sonnet-medium`. With Nirvana Claude on `claude-fable-5-2` the text tells the orchestrator the wrong seat. Use the session's recorded seat (`read_session_seat`, nirvana_claude.rs:650) or the words "your own seat". The example names exist (`nirvana/claude/agents/worker-*.md`: haiku-low, opus-high, opus-medium, sonnet-low, sonnet-medium), but there is no fable or xhigh worker, so say that in the message, not an example that may not match the booking.

### M2. The `fable` seat is on `claude-fable-5-1` with `efforts = ["medium"]`
nirvana_seats.toml:171-176 (outside my files, found while tracing `model_for`). Fable 5.2 is `claude-fable-5-2`. The seat is disabled, so nothing breaks, but `seat_for_model` (nirvana_claude.rs:819) matches an orchestrator on `claude-fable-5-2` only through the id-substring fallback (`model.contains("fable")`), never by exact model, and `record_session_seat` (612-632) falls back to `"opus"` for any model that is not in the table and not "sonnet" when seats.toml fails to parse. A Fable session would be recorded as an opus seat in that case.

### M3. The rate-limit pool name `"max"` is typed into the stream reader
nirvana_worker.rs:766 and 777 `pool: Some("max".into())` for every Claude rate-limit signal. The table owns pool names (`pool = "max"` on the claude seats; `constants.reserve_pool`). If the pool is renamed or a claude seat is put on another pool, `rate_limited` (2167) spends the wrong pool's windows. Use the seated order's own pool (`seated.pool`), which is what the agy path does (`agy_rate_limit(.., &seated.pool)`).

## T findings (token waste)

### T1. Unowned and changed file lists have no cap and are copied into the send-back brief
nirvana_gate.rs:285-327 `owned_axis(...).with(outside)` and 294/304 keep every path; `send_back_brief` (2974) writes one `> path` line each, the `Gated` event journals them (3513/3680), and `print_ruling` prints them. The 2026-10 MotoProject incident named in the comment at 2324 was 3,958 files; at about 25 tokens a path that is roughly 100k tokens into one child's brief (plus the journal and the wake). Cap `lines` at 40 with a "+N more" line in `Axis::with` for these two axes (the test output is already capped by `KEPT_LINES`).

### T2. The tail of each worker brief repeats rules the prompt already gives
`first_line` (worker.rs:516), `ORDER_FOOTER` (530, 319 chars) and `REPORT_SCHEMA` (44, 1,931 chars) each say to end with the Work Report; `first_line` and the footer both say "Work only in this folder". The footer is read through a tool call on every order, send-back child and reseat. About 100 tokens per order; low value, trivial to cut to one sentence.

### T3. The Work Report requires three fields no gate reads
`tests` and `files` are required by the schema (worker.rs:44) and `check_report`, yet the gate runs its own `test:` and reads the diff; only the pane view uses `files`, and nothing uses `tests` (B9). `gotchas` is read by the reviewer axis (gate.rs:3966) and the retro. A worker pays output tokens for the lists on every delivery, a reviewer's report included (review_brief wants `gotchas` only). Make `tests`/`files` optional or fill them from the gate's own readings.

### T4. agy gets a copy of the project's CLAUDE.md in every order
nirvana_agy_tools.rs:550-562: `claude_rules` is read, `rules_text` rewritten and written as `AGENTS.md` into each order's tools folder, with only a warning past `RULES_BUDGET_BYTES = 80_000` (about 20k tokens). There is no trim. For Nirvana's own CLAUDE.md that is small; for a project with a long CLAUDE.md every agy order pays it, and it is also in the cacheable prefix of nothing (a per-order folder). Cap it or link by `@import` as agy supports, and measure per project.

### T5. Per-order worktree path sits in the worker's system prompt
`current_dir(&seated.cwd)` (worker.rs:1370) is `.nirvana/worktrees/<order>`; Claude Code puts the working directory in its system prompt environment block, so two workers of one run differ there even when settings, model, tools and schema are identical. Whether the cut falls before or after the cached static block is not something I can see from this repo. Unverified; a probe that reads `cache_read_input_tokens` of two consecutive sibling orders would settle it (`Stream.tokens.cache_read` already records it per order).

## D findings (dead code and cleanup)

### D1. Duplicated herdr-call helpers in nirvana_job.rs
`herdr_command`, `herdr_json`, `herdr_ok`, `herdr_text`, `find_str`, `pane_env` (job.rs:1214-1345) repeat nirvana_worker.rs:2621-2735 and 2801-2821; the worker's `herdr_json_coded`/`herdr_error` is the richer one and job's `herdr_json` re-parses the error by hand. `herdr_text` in job.rs is `pub(crate)` and so is the worker's. Check: `grep -n "fn herdr_command\|fn find_str" src/*.rs`. One module (worker) should own them.

### D2. `now()` x3 and `strip_verbatim` x2 with different behaviour
`now() -> u64`: nirvana_run.rs:61, nirvana_claude.rs:71, nirvana_job.rs:592 (plus nirvana_seat.rs:3231, nirvana_usage.rs:939, nirvana_seats_square.rs:404). `strip_verbatim`: nirvana_media.rs:289 (breaks UNC, B8) and nirvana_session_record.rs:51 (correct). `grep -rn "fn strip_verbatim\|fn now() -> u64" src`.

### D3. Dead functions in nirvana_job.rs
`list_claude_tasks` (276, "newest 10 Claude tasks in %TEMP%") has one reference in the repo, its own definition: `grep -rn "list_claude_tasks\b" src` finds line 276 only; the panel uses `list_claude_tasks_in`. `with_test_now` (609) and its `TEST_NOW` thread-local (603-606) are never called, so `now()`'s test branch (593-596) is unreachable: `grep -rn "with_test_now" src` finds 609 only.

### D4. Production-compiled wrappers that only tests call
nirvana_worker.rs:3372 `book` and 3531 `seat_order` carry `#[cfg_attr(not(test), allow(dead_code))]`; nirvana_claude.rs:2119 `capture_edit_state` carries `#[allow(dead_code)]` and is called by tests only (`grep -rn "capture_edit_state(" src`). Move them to the tests modules or `#[cfg(test)]`, as `write_order_config` (gui_tools.rs:323) already is.

### D5. The `local` seat cannot run as written
nirvana_seats.toml:284-296: `cli = "claude"`, `model = "qwen3.6:35b-a3b"`. `argv()` (worker.rs:220) passes `--model qwen3.6:35b-a3b` to Claude Code with no `ANTHROPIC_BASE_URL`/auth env (`grep -n "ANTHROPIC_BASE_URL" src/nirvana_worker.rs` is empty; `CLAUDE_ENV` has one entry). `enabled = false` hides it, but `chore.*` classes and `herdr seat pick --class chore.script --json` still list it as a runner-up. Either wire the env in `run_attempt` or take the seat out of the table until it works.

Not found: unused CLI subcommands in this area. A script over every `pub fn/const/struct` of the owned files (references in non-test source across `src`) listed only the four items above.

## Brief anatomy (question 1)

### The worker's order file (what the worker reads)
Built by `seat_order_with` (nirvana_worker.rs:3539-3877), top to bottom:

| # | Part | Source | Per order? | Size |
|---|---|---|---|---|
| 1 | `Profile: <name>` and `Profile sha256: ..` lines (only with `--profile`); for agy also the profile body as a quoted section above the order | nirvana_profiles::order_text | same for every order of a profile | 2 lines, about 90 chars; agy body = the profile's size |
| 2 | The orchestrator's order text, including its `owns:`, `test:`, `check:`, `serves:` lines | the orchestrator | changes | typically 1-4 KB (not bounded) |
| 3 | `## Lessons from earlier <class> orders` header (about 170 chars) and one line per live lesson of the class/profile | nirvana_lessons::offer_for | changes with the class and the index | 170 chars + about 150 chars per lesson |
| 4 | `## <profile> cards`, one line each | nirvana_knowledge::offer | per profile | 100-150 chars per card |
| 5 | `DEVICE_LINE` (about 110 chars) or `SEND_LINE` (about 190), plus `BRIEF_LINE` (about 100) for a desktop server; only with an MCP server | gui_tools::with_brief_lines | per needs | 0-300 chars |
| 6 | Sources line: `WEB_SOURCES` (97) or `NO_WEB_SOURCES` (247) for `research.*`/`media.*`; agy adds `SOURCES_LINE` (about 190) on the same line | gate::sources_brief_line, agy_tools::sources_line | per class | 0-440 chars |
| 7 | Bladebro line (about 70 chars) | nirvana_lean::with_bladebro_line | per needs | 0-70 |
| 8 | `ORDER_FOOTER` | worker::write_order | constant | 319 chars |

So the file is orchestrator text first, herdr's additions after it, and the constant footer last. Only part 1 is shared across a profile's orders. The file is read by the worker through a tool call, so it is a tool result, never part of a cacheable prefix, and the order of its parts has no cache effect. Send-back child: `# Send-back n of 3` header (about 250 chars + the reasons) + the root order text without its footer, then footer. Reseat child: `# Continuation of <order>` header (about 280 chars), diff path, up to 4,000 chars of the last turn quoted, then the original order. Both start a fresh session, so the whole order is paid again (needed).

### The prompt (the only text on the command line)
- claude, first start: `Do the work order in <order_file>. Work only in this folder. End with the Work Report your output schema asks for.` (about 115 chars + a path of about 90).
- claude, after a stall or a deferral: `Continue the work order in <order_file>; your last turn stopped (it stalled). End with the Work Report your output schema asks for.` with `--resume <session>`.
- agy with a tools folder: `Do the work order in <order_file>. Your folder is <cwd>; work only there. <tools> holds herdr's tool config: never write there. End with the Work Report your output schema asks for.` (agy_tools.rs:525; replaces the plain line only when it equals `first_line`, so a continuation prompt keeps the plain text).

### The exact argv
claude (worker.rs:220-250, then added in exec order 1731-1743):
```
claude -p "<prompt>" --output-format stream-json --verbose --setting-sources project --strict-mcp-config
  --model <id> --effort <low|medium|high|xhigh|max> --permission-mode auto --json-schema <REPORT_SCHEMA, 1,931 chars>
  [--resume <session>]
  [--settings <preset+superpowers file>]                         (with_preset_settings, claude only)
  [--mcp-config <run>/mcp/<order>.json --allowedTools mcp__x__*,..]   (order servers and bladebro)
  [--append-system-prompt-file <profile body> --disallowedTools <list>]   (with_profile)
  [--disallowedTools <review denied>]                             (lesson review)
  [--disallowedTools .. --add-dir <root>]                         (teacher)
```
agy (worker.rs:251-276, tools at agy_tools.rs:539-620):
```
agy --output-format stream-json --model <gemini-3.8-flash-low | claude-sonnet-5-5-medium | ..>
  --print-timeout <lease_minutes*60>s --dangerously-skip-permissions --json-schema <REPORT_SCHEMA>
  [--conversation <id>] [--add-dir <run>/agy/<order>] [--settings <run>/agy/<order>.settings.json]
  --print=<prompt>
```
Environment: `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` (claude), six env vars removed, `BLADE_*` for a bladebro grant. cwd = the order's worktree or the project folder.

### Same for every order of a run, and what changes
- Same for all orders on one seat and effort: every flag in the first line of each argv block, the 1,931-char schema, `--settings` (per folder), the profile body (per profile). These are in or before the system prompt and tool list, so they are the cacheable part.
- Changes per order: the path in the prompt (about 90 chars, at the start of the user message, 20 tokens), `--resume`, `--mcp-config`/`--add-dir`/agy `--settings` paths, and the working directory (`.nirvana/worktrees/<order>`; see T5). The changing prompt text is the last part of the request and the first user turn, which is the right place; there is no timestamp or random id near the top. The one per-order cost that lands early is the working directory in Claude Code's own system prompt (T5, unverified). Orders with different MCP sets (bladebro adds about 456 tokens, per the bladebro program notes) differ in the tool block, so they cannot share a cache with orders that have none.

## Question 2: how the effort reaches each CLI
- claude: the flag `--effort <word>` (worker.rs:237-238), beside an unchanged `--model <seat.model>`. `Effort::word` gives low, medium, high, xhigh, max. A caller's `--effort` replaces the booked effort and re-derives the model (`book_with` 3503-3510: refuses an effort the seat lacks).
- agy: no flag. The effort is inside the model id: `Seat::model_for` replaces `{effort}` in `seat.model` (nirvana_seat.rs:582), for example `gemini-3.8-flash-low`, `claude-opus-5-5-high`, `gemini-4-argon-medium`. A seat whose model has no `{effort}` (gpt-oss) ignores the effort.
- The pane header and the Started event record both (`view::header`, `Kind::Started.argv`).

## Media path (question 3)
- Nothing in the four owned runtime paths reads an image. "image" and "pdf" exist only as seat capability words (nirvana_seats.toml `capabilities`), used by `--needs image` to pick a seat. An image, screenshot or PDF named in an order reaches the worker as a path in the order text; the worker opens it with its own tool (Claude Code `Read`, agy `view_file`). There is no probe, downscale, size check or count of pixels for an image anywhere (`grep -n -i "downscale\|\.png\|\.jpg" src/nirvana_*.rs` finds only the BRIEF_LINE text "read crops, not full screenshots" and a extension list in the web-name check).
- Video and audio: `nirvana/claude/skills/media/SKILL.md` tells the orchestrator to run `herdr media probe <file> --json`, then `herdr media shrink <file> --max-mb 95` when over 100 MB (nirvana_media.rs: ffprobe, then ffmpeg to at most 1280 px wide, 10 fps, mono 48 kbps AAC, two passes), then `herdr media prompt <file> --ask "..." --seat argon|flash` to print the one-line agy prompt with the full absolute path and the `view_file` instruction, then seat the order on class `media.video` or `media.audio` (argon or flash only). The shrink targets agy's 100 MB `view_file` limit, not tokens.
- Orchestrator side: the skill forbids reading video, audio or huge files into the orchestrator's context. For images there is no such line in the plugin and no hook; CLAUDE.md rule 7 ("never Read an image in the orchestrator") exists only in the Nirvana repo's file. A hook on `Read` of an image extension, or a size guard in `guard-edit`'s sibling, would enforce it (nothing exists today).
- What a `herdr look <image> --ask <question>` one-shot on a cheap seat could reuse:
  - `nirvana_media::absolute_media_path` (fix B8 first) and a sibling of `prompt_line` that asks for a description plus the answer in one short reply, no timestamps.
  - `nirvana_seat::pick` with a new class (nothing in the table is image-specific; `judge.visual` is the nearest, and `flash`/`argon` carry the `image` capability; a claude haiku seat has it too) and `book_pick`/`lease_for` as `book_child` does (worker.rs:2500), so the read is leased and recorded.
  - `LaunchSpec` + `argv()` for the CLI line, but without `--json-schema` (the Work Report schema would be forced on a one-line answer): the schema is hard-wired in `argv()` (worker.rs:242, 267), so it needs an option. `run_attempt` (1350) for the job-object start, the stall rule and the usage events, with a plain `Stream` whose `last_text` (claude) or `result.response` (agy) is the answer; `Stream::report()` must not be used, it checks a Work Report.
  - `Kind::Usage`/`write_record` for the cost row, or a leaner path with no run journal (a journal-less call must still release its lease: `release_lease`, 1573).
  - A size guard that does not exist yet: read the image header (width/height/bytes, no decoder crate is in the area), refuse or downscale above a cap before the seat sees it. ffmpeg is already a dependency of `media shrink` (`scale='min(1280,iw)':-2` works on a PNG/JPEG as well).
  - For a claude seat, `Read` on an image puts the base64 into the stream file (X1); a `look` on agy avoids the orchestrator's context entirely.

## Question 4: what the final report must contain, where it is said, how the gate reads it
- Required keys: `status` (`done`, `blocked`, `needs_answer`), `summary` (non-empty), `files` (strings), `tests` (`{name, result: pass|fail|not_run}`), `gotchas` (strings). Optional: `question`, `lesson` (`name`, `class`, `steps` at most 120 lines, `evidence`), `lesson_ops`, `card_ops`, `sources` (`url`, `used_for`). No other keys (agy's `toolAction`/`toolSummary` are dropped first).
- Where the worker is told: three places. (1) `--json-schema` carries `REPORT_SCHEMA` (worker.rs:44), which makes both CLIs return a `structured_output`. (2) The prompt line: "End with the Work Report your output schema asks for." (3) `ORDER_FOOTER` in the order file: "Your last message is the Work Report: status (done, blocked or needs_answer), summary, files you changed, tests with their results, gotchas, and, when a step is worth keeping, a lesson." A reviewer's brief (`review_brief`, gate.rs:2986) and a held-out author's (3004) restate the meaning of `done`/`blocked`/`needs_answer` for their job.
- Read: `Stream::report()` (worker.rs:865) takes `result.structured_output` (else the result text parsed as JSON), fails an empty answer, a non-success subtype or an agy `finished_reason` of print_timeout/interrupted/error, then `check_report` (a hand copy of the schema; both must change together). A failure ends the order `failed` with the stderr tail. A pass is journaled as `Delivered{report}`; `needs_answer` also journals an `Asked`.
- The gate then reads: `report_axis` (status must be `done`; `blocked` and `needs_answer` fail the axis; a blocked report is answered, never sent back, gate.rs:3705), `sources_axis` for `research.*`/`media.*` (each `sources[].url` must appear as a read in the stream file; videos are checked against oEmbed), `lesson` (its quote must be in the stream, `verify_and_write`), `lesson_ops`/`card_ops` for review and teach orders, `gotchas` for a reviewer's defects (3966) and the retro. It does not read `files` or `tests` (T3, B9): its own axes are `test:` re-run in the worktree, the git diff against `owns:`, `serves:` ports, the held-out check and the review.

## X findings (multimodal)
- X1. A claude worker that `Read`s an image writes the base64 into `streams/<order>.jsonl` (the tee at worker.rs:1466 stores every line). The gate then loads the whole file with `read_to_string` (`stream_text`, gate.rs:3049) at least twice per gate (`deterministic` 2687 and `gate_subject` 3444) and `stream_reads`, `verify_and_write`, `note_gated` parse it again. A worker that reads ten 4 MB screenshots leaves a 50 MB stream parsed several times per gate. Truncate `image`/`source.data` blocks in the tee (keep type and length) or hold only the text blocks.
- X2. Nothing stops the orchestrator reading an image: the only guard is a sentence in the Nirvana repo's CLAUDE.md. A `PreToolUse` hook on `Read` with an image extension (the `guard-edit` family in nirvana_claude.rs:2487 already parses `file_path`) would turn rule 7 into a gate (CLAUDE.md rule 8).
- X3. `herdr media shrink` accepts any container ffprobe reads, including a still image, and re-encodes it as H.264 video (`-r 10`), so `shrink photo.png` yields a one-frame mp4 that agy cannot read as an image. Refuse inputs with no duration in `run_probe` or branch to a still-image path.

## Notes on what I did not do
- Did not trace the sources-axis stream heuristics (gate.rs 600-1500); the "source-scanning gates are hollow" memory note says they were already found weak, and they are the largest block of code in the file.
- Did not check whether the gate tests pass with the code removed, or whether `nirvana/controls.d` find texts sit on product lines (no control file read).
- B7 and T5 are the two I would test before changing code.
- The journal and `streams/*.jsonl` only grow (usage line at most every 10 s per worker, 250 bytes); `statusline` replays every open run's journal on each refresh. At a few MB that is milliseconds; no cleanup path exists for ended runs, and I measured nothing.
- Hooks: the SessionStart digest is capped at 540 chars (nirvana_claude.rs:37) and the Stop block fires once per dead end; neither prints per prompt. `UserPromptSubmit`'s `prompt-baseline` runs a `git diff --numstat` per prompt (latency, not tokens).
