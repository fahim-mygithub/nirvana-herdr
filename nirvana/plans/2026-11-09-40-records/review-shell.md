# Review: shell (phone page, sidebar/panels, taskbar, Python tools, plugin)

Read-only pass at master 86b37c7e. No cargo, no git change, no herdr/claude/agy run. Everything below was traced
in the code named; "confirmed" means I read the path end to end, "plausible" means I did not see it fail.

## Answer to extra question 1: the phone page identity checks

I found no route that skips the checks, and no path traversal. Evidence, in the order a request meets them:

- `http.rs connection()` -> `Handler::admit` (`routes.rs:324`, `peer.rs`) runs before one request byte is read:
  the TCP row's owning pid must be the Tailscale service or its `tailscaled.exe` child. Fails closed: no row,
  no service pid, or an unreadable table is a refusal.
- `Site::route` (`routes.rs:348-375`) runs, for every method and every path, in this order: exactly one
  `Tailscale-User-Login` equal to this PC's owner (an unknown owner refuses everyone), exactly one `X-Forwarded-For`
  inside Tailscale's ranges and not this PC's own address, exactly one `Host` equal to `<pc>.ts.net:8443`
  (421), then `same_site` (`Sec-Fetch-Site` same-origin/none and `Origin` equal to the page, required for any
  non-GET, and for GET/HEAD under `/api/` when sent). A refusal returns before `new_space_and_cards`,
  `alerts_routes`, the POST match, the asset match or the GET match, so every file and API route is behind it.
  The static assets are behind the first three checks too; only `same_site` is skipped for them, by design.
- Live views re-check the caller every `stream::WAKE` (`live.rs:226`, `stream.rs:127`).
- Upload names are the server's: `send::name_for` builds `phone-<date>-<time>-<8 hex>.<ext>` from the content type
  table (`send.rs:45-94`); the client's file name, path and type never reach the disk. `..`, `/`, `\`, `:`, `CON`,
  `NUL` and streams cannot occur in it. `send::save` uses `create_new` on `<name>.partial`, then rename. A send
  names files by that same name only: `is_upload_name` (`send.rs:97`) rejects anything else, including
  `x.jpg.partial` (its extension would be `jpg.partial`). `uploads_dir` refuses a junction or symlink on the way
  (`send.rs:127-175`).
- Size and rate: 50 MiB (`UPLOAD_MOST`), a `Content-Length` over it is a 413 before a byte is read, a chunked body
  is counted as it streams, 4 uploads at once, 16 KiB/s average after 10 s, 20 min ceiling, 32 connections, 6 live
  views. The send, key, open and alerts bodies are 64 KiB, 4 KiB, 4 KiB, 4 KiB.
- Picker: `pick.rs resolve` takes a name that must equal an entry of a fresh listing of `Desktop\Pojects`, plain
  folder, canonical parent equals the root, checked again after the preset. Alerts: the push endpoint must be https
  on three host names, no port, no redirect followed (`alerts.rs:93`, `:450`).
- Residual (documented in `routes.rs:142-152`, not a finding): a GET under `/api/` with neither `Sec-Fetch-Site`
  nor `Origin` is let in.

## B findings (bugs), most severe first

### B1. A Claude seat run through agy is labelled "Gemini claude-opus-5-5-high" / "O5" / "Gemini"
`src/nirvana_subagents.rs:1110-1116`. Provider is chosen from `order.seated.cli == "agy"`, not from the model. The
`g-opus` and `g-sonnet` seats are `cli = "agy"` with `model = "claude-opus-5-5-{effort}"`
(`src/nirvana_seats.toml:193-216`), and `seat.rs:583` writes the substituted id into the journal. Scenario, real
data: the installed `record.jsonl` already holds `"model":"claude-opus-5-5-high"` and
`"model":"claude-sonnet-5-5-medium"`. `format_model(Some("claude-opus-5-5-high"), Gemini)` falls to the last
Gemini arm and returns `"Gemini claude-opus-5-5-high"`. Then `agent_sidebar.rs:113` `compact_model` prints `O5`
(the version is read from the first digit run, so `5-5` loses its minor), and `agent_sidebar.rs:733` `short_model`
prints `Gemini` (one word after the prefix, so the `parts.len() >= 2` rule does not fire). The owner sees a
Gemini seat where an Opus 5.5 order runs.
Fix: choose the provider from the model id: `claude-` prefix means `SubagentProvider::Claude` whatever the cli;
in `format_model` strip a trailing `-low|-medium|-high` effort before `claude_label`. Keep the cli only for
telling the usage pool apart.
Test: `format_model(Some("claude-opus-5-5-high"), provider_of("agy", "claude-opus-5-5-high")) == "Opus 5.5"`, and
`compact_model("Opus 5.5") == "O5.5"`; plus a row built from a seated order with cli agy and that model.
Confidence: confirmed.

### B2. A failed JetBrains Mono download stops the Windows install after the binary is installed
`nirvana/windows_shell.py:445-448` and `nirvana/install.py:104-107`. `install_all` calls `install_font` whenever no
JetBrains Mono face is registered; `opener(...)` raises `URLError` offline (or on a proxy page, the sha check raises
`ValueError`). Nothing catches it. `install.py` has already printed "installed Nirvana-Herdr alpha X" and
written the stamp, so the running window offers the reload, but `register_toast_id`, `install_verb` and the
Nirvana-C cleanup (lines 451-455) never run and the script exits with a traceback. The docstring says "a failure
leaves nothing that points at a missing file"; it does leave a new binary with the old verb.
Fix: wrap the font step in `try/except (OSError, ValueError)` and set `report["font"] = f"failed: {err}"`; font is
cosmetic.
Test: `install_all(..., reg=FakeRegistry(), font=True)` with an `opener` that raises `URLError`; assert the verb is
in `reg` and `report["font"]` starts with "failed". Needs `install_all` to take `opener`.
Confidence: confirmed by reading (no run).

### B3. `_replace_tree` is not atomic: a locked file leaves the plugin half deleted and aborts the install
`nirvana/windows_shell.py:115-119`. `shutil.rmtree(dst)` then `copytree`. A running Nirvana Claude holds
`<home>\nirvana\claude` (hooks run from it, skill files are read, a Windows antivirus or editor can lock a file).
If `rmtree` raises midway, the plugin is partly gone and the live orchestrator's next hook or skill read fails;
`install.py` aborts before the binary is installed (plugin runs first, `install.py:102`). If `rmtree` succeeds but
`copytree` fails, the same holds.
Fix: `copytree(src, dst + ".new")`, then rename `dst` to `dst + ".old"`, rename `.new` to `dst`, then delete `.old`
(the `install_binary` rename-aside idea); or `copytree(..., dirs_exist_ok=True)` and delete only files the tree no
longer has.
Test: a scratch `dst` with a read-only/locked file; assert the old tree is still complete after the failure.
Confidence: plausible (Windows file locks); the non-atomic order is confirmed.

### B4. `install_binary` leaves no `herdr.exe` if the copy fails
`nirvana/install.py:59-62`. The old binary is renamed aside, then `shutil.copy2(built, target)`. A full disk or a
virus scanner holding `target\herdr.exe` open between the rename and the copy raises, and the script exits with no
`bin\herdr.exe` at all: the Explorer verb (`nirvana-open.exe`) and the plugin hooks
(`${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe`, `hooks.json`) then point at a missing file until the next install.
Fix: on any exception from `copy2`, rename the `.old-*` file back and re-raise.
Test: `install_binary` with a `built` that does not exist; assert `target` still exists afterwards.
Confidence: confirmed by reading.

### B5. A zero-byte upload is accepted and handed to the agent
`src/nirvana_remote/live.rs:527-557`. `Content-Length: 0` is `Framing::Length(0)`, passes the 411 check and the
50 MiB check, and `save` writes an empty `phone-....jpg`; the page then adds its path to the message
(`chat.js:217`). Scenario: iOS gives Safari a placeholder for a photo still downloading from iCloud (a 0-byte
`File`); the agent is told to look at an empty image.
Fix: after `read_body`, a size of 0 returns `(400, "the file is empty")` and removes the partial (do it inside
`send::save`'s `read` closure result).
Test: POST `/api/upload` with `Content-Length: 0`; expect 400 and no file in `uploads/`.
Confidence: confirmed.

### B6. A killed controls run leaves the product file sabotaged, with nothing to repair it
`nirvana/controls.py:1070-1090`. The restore is a Python `finally`, which dies with the process: `TaskStop`, a
terminal close, or a power loss during a 30 s cargo build (the `TIMEOUT` is 900 s) leaves the broken find/replace
text in the source tree. Memory note `taskstop-leaves-bash-loop-running` records a stopped control loop that kept
sabotaging; there is no marker file, so the next run's `original = path.read_bytes()` takes the sabotaged bytes as
the original and the sabotage becomes permanent in the working tree (the `count(find) != 1` guard then says "find
text occurs 0 times", which hides it).
Fix: before writing, save `original` to `nirvana/.controls-restore/<name>.bak` (git-ignored) and, at the start of
`main`, restore any `.bak` found. Delete it after the `finally`.
Test: run `run_one` with a command that raises `SystemExit`, then call the start-up repair, assert bytes equal.
Confidence: confirmed by reading; the failure needs an unclean kill.

### B7. The Running square can keep a dead tool name
`src/nirvana_running.rs:276-283`. `read_tail(&file).map(|tail| activity(&tail))` is `None` when the transcript can
no longer be read (moved by a Claude Code rotation, locked, deleted), and the update is guarded by
`activity.is_some() && ...`, so the old `Activity` stays in the cache for ever. `running_value` then prints
"working · Bash" for as long as herdr says the agent is working, from a tool call that ended hours ago.
Fix: set `entry.activity = None` (and flag `CHANGED`) when the read fails after having succeeded before.
Test: `cached` on a path, delete the file, wait `REREAD`; expect `None`. (Under `cfg(test)` no read starts; the
test needs a hook on `start_read`.)
Confidence: plausible; low severity (needs an unreadable transcript while the agent runs).

### B8. `herdr_self` can time out on output larger than a pipe buffer
`src/nirvana_subagents.rs:1225-1264`. stdout is piped but read only after `try_wait` reports exit; a child that
writes more than the OS pipe buffer (64 KiB on Windows) blocks on the write and is killed at `timeout`, so the
caller gets `None`. Current callers ask for `pane process-info` and other small answers, so this does not fail
today; it will for the first caller that asks for a list. Fix: drain stdout on a thread, as `nirvana_dead::output_within`
(`nirvana_dead.rs:555`) and `nirvana_git::run_cmd` already do; better, have `herdr_self` call `output_within`.
Test: a child printing 200 KiB; expect `Some(_)`. Confidence: plausible (latent).

## Question 2: Upload path ("Upload path")

- Page: `chat.js` picks files (`<input accept="image/*" multiple>`, a File option for anything, a clipboard paste for
  screenshots). `uploadFile` rejects over 50 MB on the client (`chat.js:229`). `nv.upload` (`app.js:393`) POSTs the
  file as the whole body with its own `Content-Type`.
- Server: `POST /api/upload?agent=<pane id>` saves it as `<agent cwd>\.nirvana\uploads\phone-<yyyymmdd>-<hhmmss>-<hex>.<ext>`
  (`send.rs:77`) and answers `{file, path, size}`. The extension comes from a 13-entry content-type table (png,
  jpg, heic, heif, gif, webp, pdf, txt, csv, json, zip, mp4, mov); anything else is `.bin`. **No type check of the
  bytes and no resize, on the phone or the PC**: a 12-megapixel photo is stored as it came, up to 50 MiB.
- Send: `POST /api/send {agent, text, files:[names]}` -> `send::message` (`send.rs:235`): the text with control
  characters dropped (newline and tab kept), then each file's **full absolute path** after a space, in double quotes
  when it contains whitespace. Example sent into the pane:
  `what is wrong here C:\Users\fahim\Desktop\Pojects\MotoProject\.nirvana\uploads\phone-20261109-143012-9f3a1c2e.jpg`.
  Nothing else: no "view this file", no size, no type, no note that it is an image. It goes through `agent.prompt`,
  i.e. as typed text plus Enter, to whichever agent the chat is open on (Nirvana Claude, a Claude worker pane, Codex,
  agy).
- Does the orchestrator read the image into its context? Only if it chooses to `Read` the path, which it will, because
  a bare image path in a prompt is the usual cue. Nothing in the plugin stops it: `nirvana/claude/ROLE.md` has no
  image rule (grep "image|screenshot|photo" in `nirvana/claude` finds only `skills/seat` and `skills/media`), and
  `hooks.json` has no `Read` matcher. Repo CLAUDE.md rule 7 ("never Read an image in the orchestrator", 118 MiB
  measured) protects the people who work on this repo, not the orchestrator the plugin starts. Claude Code
  downsizes an image it reads before the model sees it, but the transcript keeps it and later turns re-send it
  (the rule's measurement).
- HEIC/HEIF: stored as `.heic`/`.heif`; Claude's `Read` cannot show them. iOS Safari usually converts to JPEG for
  `accept="image/*"`, but the "File" option and a paste can hand HEIC through.
- Limits: 50 MiB per file, 4 uploads at once, nothing per space or per day, no cleanup (see D1).
- Suggested (not done, not mine): append a fixed short note per image, e.g. ` (image; give it to a leaf agent, do not Read it here)`
  when the agent is Nirvana Claude, and resize to 1568 px on the PC or in the page (canvas) before saving.

## Question 3: Model labels ("Model labels")

Code that maps a model id to a label:

| Code | Used for |
| --- | --- |
| `nirvana_spaces.rs:145 claude_label` | id -> "Claude Fable 5.2" (family words title-cased, digit runs joined by ".", an 8-digit run dropped as a date, `[1m]` cut) |
| `nirvana_spaces.rs:177 claude_model` / `antigravity_model` / `codex_model` | the `nirvana_model` pane token (sidebar header, phone chat header) |
| `nirvana_spaces.rs:428 compact_model` | the narrow chip: sidebar space rows (`client/shell/nirvana_spaces.rs:188`) and agents panel (`agent_sidebar.rs:113`) |
| `nirvana_subagents.rs:118 format_model` / `:174 short_model` | subagent and order rows ("Sonnet 5.5", "Gemini 3.8 Flash") |

What each prints:

| Id | token / panel row | chip (`compact_model`) | `short_model` |
| --- | --- | --- | --- |
| `claude-fable-5-2` | "Claude Fable 5.2" / "Fable 5.2" | `F5.2` | `Fable` |
| `claude-opus-5-5` | "Claude Opus 5.5" / "Opus 5.5" | `O5.5` | `Opus` |
| `claude-sonnet-5-5` | "Claude Sonnet 5.5" / "Sonnet 5.5" | `S5.5` | `Sonnet` |
| `claude-haiku-5-5` | "Claude Haiku 5.5" / "Haiku 5.5" | `H5.5` | `Haiku` |
| `gemini-3.8-flash-{low,medium,high}` | "Gemini 3.8 Flash" | `Flash` | `Gemini Flash` |
| `gemini-4-argon-{effort}` | "Gemini 4 Argon" | **`Gemini`** (no "flash", no "pro": falls to the 6-cell local rule) | `Gemini Argon` |
| `claude-opus-5-5-{effort}` (g-opus, via agy) | **"Gemini claude-opus-5-5-high"** | **`O5`** | **`Gemini`** |
| `claude-sonnet-5-5-{effort}` (g-sonnet) | **"Gemini claude-sonnet-5-5-medium"** | **`S5`** | **`Gemini`** |
| a bare Claude with no model | "Claude" | `C` | |
| a bare agy with no model | "Antigravity" | **`Antigr`** | |

(Fable 5.2 reads right everywhere because the family word is generic; only the seat table and tests still say 5.1.)
The sidebar for a Fable pane also shows the same `F` letter as the `Claude && flash` branch (`nirvana_spaces.rs:445`),
which no real label can reach: "Claude Gemini Flash" is not produced anywhere.

## M findings (model and seat drift)

- **M1.** `src/nirvana_seats.toml:174` `model = "claude-fable-5-1"` for seat `fable`, prices and the `week.fable`
  comment ("claude-api skill price table, cached 2026-10-06") predate Fable 5.2 (`claude-fable-5-2`). The seat is
  `enabled = false`, so nothing spends today, but the day the owner enables it the CLI is started with a model id the
  seat table owns and gets the older model while the panel (via `claude_label`) would print what the transcript says.
  The tests that pin 5.1 (`nirvana_spaces.rs:853`, `client/shell/tests/nirvana_subagents.rs:1110`) are examples, not
  drift. Owner of the table is another area; named here because every label above depends on it.
- **M2.** `compact_model` (`nirvana_spaces.rs:428`) has four wrong outputs: the dashed id loses its minor version
  (`O5`, B1), `Gemini 4 Argon` and any Gemini model whose label has neither "flash" nor "pro" print `Gemini`,
  a bare "Antigravity" prints `Antigr`, and any local model name containing "pro" (e.g. `deepseek-prover`) prints
  `Pro`. Fix: one `model_family(label) -> Family{Claude(x), Gemini(Flash|Pro|Argon), Codex, Local}` and print
  `Argon` for argon. Test: the table above as `assert_eq!` rows.
- **M3.** `format_model` Gemini arm (`nirvana_subagents.rs:144-158`) hard-codes exactly two models and picks
  "Gemini 4 Argon" for **any id containing the character `4`** (`lower.contains("4")`), e.g. `gemini-3-pro-20260414`.
  Fix: match `argon` only; otherwise print the id with the effort suffix cut.
- **M4.** The plugin repeats the seat table's job in prose and front matter: `nirvana/claude/agents/worker-*.md`
  (`model: claude-opus-5-5`, "Opus 5.5 at high"), `agents/nirvana.md` (orchestrator `claude-opus-5-5`, `effort:
  medium`), `ROLE.md:64` ("runs on your own seat, Opus at your effort"), `skills/seat/SKILL.md:57` ("opus, sonnet and
  the four agy"). A Fable orchestrator (the owner's current model family) would be told it is Opus, and there is no
  `worker-fable-*` agent, so a Fable pick cannot be spawned as a subagent. A count of seats in prose also rots
  (CLAUDE.md rule 4). Move the model and effort to one generated block, or have `windows_shell.install_plugin`
  write the agent files from the table.

## T findings (token waste)

- **T1.** `hooks/hooks.json` starts `herdr.exe` three times on every prompt (`board notice`, `run notice`,
  `nirvana prompt-baseline`), three on SessionStart, four on Stop, one per Edit/Write/Bash tool call
  (`nirvana check-edit`). I could not run them to count printed bytes, so this is a hypothesis: whatever
  `board notice` and `run notice` print on every prompt is context added to every turn of the orchestrator. If they
  print "no notice" lines, make them print nothing; a single `herdr nirvana hook <event>` that does the three
  jobs in one process would also cut Windows process starts (each a `herdr.exe` image of the fork).

## D findings (dead code and cleanup)

- **D1.** Phone uploads are never removed. `grep -rn "uploads" src --include=*.rs` (not tests) finds only the
  writer (`send.rs`, `live.rs`), the route table and the page; nothing deletes `<space>\.nirvana\uploads\*`.
  A killed PC mid-upload also leaves `<name>.partial` for ever (`save` removes it only on a handled error).
  Photos are 3-8 MB each, so a busy week is hundreds of MB per space, in a folder the agent (and `git status`
  once someone removes the `*` ignore) sees. Fix: a sweep with the dead-pane pass (`nirvana_dead`), files older
  than 14 days not named in the space's last transcript, `.partial` older than an hour.
- **D2.** Three controls edit test code, so they redden a test by construction rather than prove a product line:
  `p19_l7_google_family_flash_argon_only` (`src/nirvana_seat.rs:4494` is inside `mod tests`),
  `p13c_live_claude_read_returns` (`src/nirvana_memory_trim/tests.rs`), `p39c_whole_checks`
  (`src/nirvana_remote/tests_live.rs`; this one is a deliberate test of the `whole` helper, keep). Method: imported
  `nirvana/controls.py`, then for every control checked the file exists, `find` occurs once, and the offset is
  not inside an inline `#[cfg(test)] mod x {` block or a file named `tests*`. All 3211 find their text once; those
  were the only hits. Script: `scratchpad/c40/scan_controls2.py` (read-only).
- **D3.** Three lists of the phone page's served files must stay equal by hand: `routes.rs ASSETS`,
  `pick.rs new_space_and_cards` (`include_str!` of `newspace.js`, `cards.js`) with `shell.rs PICK_SCRIPTS`
  (`include_bytes!` of the same two), and `alerts.rs FILES`. A script added to one place but not `PICK_SCRIPTS`
  is served but not cached for the offline error screen, and its bytes are left out of the build tag. Make
  `PICK_SCRIPTS` the only table and have `pick.rs` look the body up in it.
- **D4.** `compact_model`'s `trimmed.starts_with("Claude") && lower.contains("flash")` arm
  (`nirvana_spaces.rs:445-447`) can no longer be reached by a label this code makes (grep for the only producers of
  a "Claude ... Flash" string finds none outside the test at `tests/nirvana_subagents.rs`); remove with M2.

## X findings (multimodal)

- **X1.** Photos reach agents as a bare absolute path in typed text, unresized, up to 50 MiB, with no image-handling
  note (full detail under "Upload path"). The one place that can contain it is the string built in
  `send::message`; the orchestrator's `ROLE.md` and a `Read` hook are the second.
- **X2.** `.heic`/`.heif`, `.mov` and `.mp4` are saved with their real extension but Claude cannot read them; the
  chat should tell the owner which agents can (agy reads video/pdf/image up to 100 MB per `skills/media`).
- **X3.** The per-frame draw path question (extra question 4): see below.

## Question 4: file reads or process spawns in the draw path

None found. The shell panels draw from caches that a worker thread fills: `nirvana_git::cached`
(`nirvana_git.rs:765`, spawns a thread on first ask, `run_cmd` with `CREATE_NO_WINDOW`), `nirvana_running::cached`
(`:299`, `start_read` is throttled by `REREAD` and runs on a named thread), `nirvana_learning::cached`,
`nirvana_servers::cached_ports`, `nirvana_usage::rows`. `render_learning_panel` and the git, jobs, accounts and
footer renderers take their data as arguments. The `is_file()`/`is_dir()` hits in `client/shell/nirvana_learning.rs:572,580`
and `std::fs::create_dir_all` in `nirvana_profiles.rs:64` run on a key press (Enter, `K`), not per frame.
One cost per frame is the lock + `PathBuf` clone in `nirvana_running::cached` and `nirvana_git::cached`
(`entry.asked_at = Some(Instant::now())`, a `HashMap` entry insert per call); negligible.
Process spawns outside the draw path all go through `configure_background_command` or `CREATE_NO_WINDOW`
(`nirvana_dead_panes.rs:91`, `nirvana_subagents.rs:1233`, `nirvana_git.rs:208`, `tailscale.rs` `command()`).

## Not covered

I did not read `src/nirvana_taskbar.rs`, `nirvana_servers.rs` beyond the cache functions, `app/nirvana_new.rs`,
`app/nirvana_restart.rs`, `app/nirvana_relaunch.rs`, `app/nirvana_edit.rs`, `app/nirvana_wake_retry.rs`,
`client/shell/nirvana_mods.rs`, `nirvana_seats.rs` (excluded), `nirvana_footer.rs` or `nirvana_jobs.rs` line by line,
the `skills/*/SKILL.md` bodies, or `nirvana/controls.py` lines 40-1040 (a table of 3211 controls; only the runner and a
mechanical scan). Dropped for lack of a traced scenario: `as u16` casts in panel layout (no overflow at real terminal
sizes), the `nirvana_git` `reading` flag staying true if `read_git_status` panics, and the chat bubble `line{index}`
id shifting when a transcript line has no `uuid` (Claude Code writes one on every line).
