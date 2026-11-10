# Review: area "memory" (plan 40), read-only, master 86b37c7e

Read in full or in the injection and write paths: nirvana_board, nirvana_jev_memory, nirvana_jev_install,
nirvana_jev_compact, nirvana_lean, nirvana_memory_trim, nirvana_idle_compact (+ app/ half), nirvana_review (briefs, cap,
idle), nirvana_lessons (offer, index line), nirvana_knowledge (offer, teach brief), nirvana_profiles (order text, template),
nirvana_fresh (trigger, owner-prompt filters, handoff input), nirvana_jev (client, reply check, guide CLI),
nirvana_jev_roles (head), nirvana_preload (settings write, trim glue, Jev rows). Only skimmed: nirvana_learning and
client/shell/nirvana_learning (UI views, no model text), nirvana_retro (journal gaps, no model call), most of the test
modules. Not traced: app/nirvana_fresh.rs, nirvana_knowledge ops parsing, nirvana_lessons ledger/rollback.

## B findings (must fix)

### B1. Idle compact reads a pre-compaction context and compacts again every `minutes` of idle
- `src/nirvana_idle_compact.rs:562-586` (`last_context_tokens`, `read_context_tokens`), used at `:763-764`; decision at `:230-235`.
- Scenario: Claude idle 50 min with 300k context, `/compact` goes (Enter at `app/nirvana_idle_compact.rs:268`), the pane
  turns Working, so `IdleStretch::after` (`:103`) resets the stretch and `sent` becomes false. The pane is idle again with a
  summary of about 10k tokens. After `minutes` the transcript age passes rule 4, and `read_context_tokens` returns the LAST
  ASSISTANT LINE's usage, which is still the 300k from before the compaction (a compaction writes a `compact_boundary`
  system line and an `isCompactSummary` user line, not an assistant line; `nirvana_jev_compact.rs:116-121` was written
  knowing this and returns `None` when a boundary is newer than the last assistant line). `tokens >= min_tokens` holds,
  so `/compact` is typed again, and again after every following idle stretch, each a paid model turn, until the owner
  speaks. Pasting into the owner's pane for a no-op also repeats the worst-outcome path the module warns about.
- Fix: in `last_context_tokens` scan newest first and return `None` when a line with `"subtype":"compact_boundary"`
  (type system) or `isCompactSummary` comes before the first assistant line (copy `nirvana_jev_compact::context_tokens`).
  Better, make the two modules share one function.
- Test: a transcript text ending `assistant(320000)`, `system compact_boundary`, `user isCompactSummary` gives `None`;
  `decide` with `context_tokens: None` skips. Add a sabotage control that removes the boundary check.
- Confidence: plausible (the code path is confirmed end to end; that Claude Code writes no assistant line for the compact
  call is inferred from jev_compact's own premise; confirm by grepping the owner's log for two "idle Claude sent /compact"
  lines in one pane about `minutes` apart with no owner turn between).

### B2. `Duration::from_secs_f64` on a server-supplied Retry-After panics
- `src/nirvana_jev.rs:360-362`, header read at `:282` and `:287`.
- Scenario: an HTTP 429 with `Retry-After: -1`, `NaN`, `inf` or `1e30` parses as f64 and `from_secs_f64` panics
  (negative, non-finite or overflow). The caller is a Stop hook (`herdr jev compact`), a Jev CLI call from an agent, the
  memory-trim thread and the seating advisor, so the panic kills the hook or an agent's tool call and, in the herdr server
  (jev roles, trim), a worker thread.
- Fix: `Duration::try_from_secs_f64(v).ok().filter(|d| *d <= Duration::from_secs(5))`, else `RETRY_WAIT`.
- Test: mock transport returning `(429, "", Some("-1"))`, `"NaN"`, `"1e30"`; expect a normal `Http`/retry result, no panic.
- Confidence: confirmed by reading.

### B3. `herdr memory restore` overwrites the live memory index without keeping it
- `src/nirvana_memory_trim.rs:992-1004`.
- Scenario: the owner accepts a trim on Monday (backup B1 = the Monday index). During the week Claude Code adds eleven
  entries to `MEMORY.md`. The owner runs `restore`; the index becomes B1 and the eleven entries (their files stay on disk)
  are no longer pointed at. No copy of the index being replaced is made, and the pointer check that guards `accept`
  (`:942`) is not run. This is the owner's data and the module's own header promises "this cannot lose a pointer".
- Fix: before `write_atomic`, back up the current index the way `accept_in` does (a second backup file named
  `...-pre-restore`), and refuse or warn when `check(backup, current)` shows the current index holds pointers the backup
  lacks, unless a `--force` is given.
- Test: accept, add an entry to the index, restore; expect either a refusal naming the new pointer or a backup holding it.
- Confidence: confirmed by reading.

### B4. `Preload` panel work on the draw path: git + 100k stats, and a file write
- `src/nirvana_preload.rs:887-907` (`jev_items`) is called from `items` (`:753`), which the client calls from
  `client/shell/nirvana_mods.rs:609-611` (`panel_items`). It runs `nirvana_jev_memory::detail` and `guide_tokens`
  (both `measure`, `nirvana_jev_memory.rs:198-210`, a `git ls-files -z` child plus a `metadata` per file, up to
  `MAX_FILES` 100,000), `file()` (`read_or_create` writes `JEV.md` when missing, `:278-284`) and `today_line` (reads
  the whole Jev log every 5 s, `nirvana_jev.rs:1524-1551`).
- Scenario: a 100k-file repo, panel open. Every 60 s (cache `:203`) the draw thread blocks for the git child and the stats
  (about 1 to 3 s on Windows), the window freezes. A `git` that hangs (network drive, lock) has no timeout at `:138-146`.
- Fix: measure in `store()`/`start_read`'s thread and keep the `Size` in `View`; give the child a bounded wait (the
  repo has `nirvana_learning::GIT_WAIT` at 5 s as a model). Read the Jev log in the same thread.
- Test: `jev_items` with a stand-in `measure` that sleeps 2 s must return at once from the draw call.
- Confidence: plausible (I did not trace the client call to the draw loop itself, only that `panel_items` is the caller).

### B5. Two writers rewrite `NIRVANA.md` with no lock
- `src/nirvana_lessons.rs:1737-1762` (`write_index_line`, called from the gate `nirvana_gate.rs:3472` and from
  `nirvana_review.rs:1298`) and `src/nirvana_claude.rs:496-519` (`write_board_line`, from the SessionStart digest).
- Scenario: read, replace own line, `write_atomic`. Atomic write stops a torn file but not a lost update: a gate in project
  A and a digest in project B both read the same text, each writes back its own line, and one project's line (and the
  `Read that file when a lesson may apply` pointer it gives) is gone until that project's next gate. The file is also
  never pruned (a line for every project ever gated).
- Fix: take a lock file beside it (the board and lessons already have `Lock` helpers) around read-modify-write.
- Test: two threads each adding a distinct line 200 times; expect both sets present.
- Confidence: confirmed by reading; severity low.

### B6. `Preload` writes `.claude/settings.local.json` by read, edit, write with a fixed temp name
- `src/nirvana_preload.rs:1047-1071`, `switch_item` `:1074-1092`, and `nirvana_board::share` `:553-559`.
- Scenario: Claude Code itself writes that file when the owner chooses "always allow" for a tool. A Preload switch read the
  file earlier in the same second, so the permission the owner just granted is written back out of the file. The temp name
  `.settings.local.json.nirvana-tmp` is shared, so two panes' switches in one folder can interleave (rename of a temp the
  other is rewriting).
- Fix: per-call unique temp name (the `temp_sibling` helper already exists: `nirvana_memory_trim.rs:881`) and re-read the
  file immediately before the write, aborting if its mtime changed since the read.
- Confidence: plausible; narrow window; it is the owner's permissions file, so it earns a line.

### B7. `JEV.md` placeholders `{read_file_kb}` and `{files_kb}` have no default
- `src/nirvana_jev_memory.rs:234-248` fills defaults for `min_project_kb`, `ask_file_kb`, `output_lines` only; the shipped body
  uses `{read_file_kb}` and `{files_kb}` too (`:42`, `:45`).
- Scenario: the owner edits the front matter and drops `files_kb` (the file says "Edit freely"). Every agent in a project
  of 1 MB or more is told "Candidate files over {files_kb} KB in all", literal braces, at each SessionStart.
- Fix: add the two `.or_insert_with` defaults (20 and 200), or fill unknown `{...}` placeholders with the shipped default.
- Test: `render` of a text with front matter `min_project_kb: 1` only, expect no `{` in the output.
- Confidence: confirmed by reading; low.

(Considered and dropped, not bugs: `answers[...]` indexing in jev_compact and memory_trim cannot panic because
`check_reply` requires every asked id to be answered, `nirvana_jev.rs:425-430`; `lines.get(..2000)` in
`jev_compact::recent` is char-boundary safe.)

## M findings (model and seat drift)
- M1. `nirvana_profiles.rs:706` the new-profile template names `# model: claude-sonnet-5-5` as its example. It is a valid id
  today, but it is the only model id the area hard-codes in shipped text; it will rot. Point the comment at
  `herdr seat table` or print the first seat's model from the table.
- Nothing else in the area names a Claude model, a price or an effort list outside tests. The Jev model strings
  (`~typesafe/jev-latest`) are not Claude. Context thresholds (`fresh_above` 120k, jev compact 150k/300k/500k) are code
  constants labelled hypotheses; they assume a 1M window and do not read the model's real window, so a 200k seat would
  never reach the 300k and 500k lines.

## T findings (token waste)
- T1. Board notice: the first prompt of every session (and of every `/clear`, since the session id changes) prints the WHOLE
  board with no size cap, `nirvana_board.rs:386-399`, plus a 330-char how-to. A board that grows to 20 KB costs about 5k
  tokens per session start per agent. Cap it (names and first line per section, `herdr board read` for the rest), as the
  fresh note already does (`nirvana_fresh.rs:815-832`).
- T2. `lessons::lines_for` has no cap (`nirvana_lessons.rs:1427-1443`); knowledge cards are capped at 15 (`CARDS_OFFERED`).
  Only the daily merge review (over 8 per class) limits it, and a merge review is itself a paid turn. Cap lessons at the
  same number and print "n more: herdr lessons for --class".
- T3. The Jev guide is injected whether or not a Jev key exists (`nirvana_jev_memory::for_folder` has no key check), so on a
  machine without `~/.config/jev/openrouter.env` every agent is told to call a tool that answers `failed: unavailable (read
  it yourself)`, a wasted tool turn each. Print nothing (or the flag line) when `find_key()` is `None`.
- T4. The guide fires on every SessionStart source (startup, resume, clear, compact), so about 2 KB is re-sent after each
  compaction, which the hook cannot avoid unless it reads the `source` field of its stdin JSON. Reading `source` and printing
  nothing on `resume` (the text is already in the resumed transcript) removes one copy per resume. Plausible.
- T5. `offer_for` and `knowledge::offer` count an `offered` for a brief that may never launch (the worker lines at
  `nirvana_worker.rs:3721-3756` run before later steps that can return `Err`), so a lesson decays (`live`, `DECAY_AFTER` 6)
  on failed seatings. Move the count after the seat is written.
- T6. `jev_compact::hook_input` reads and splits a 4 MiB transcript tail on every Stop hook of every turn, even under the
  150k line (`:478`); only the newest few lines are needed below the line. Read 128 KB first, widen only when needed.

## D findings (dead code and cleanup)
- D1. Per-session files that only grow: `<home>/board-seen/<session>.md` (`nirvana_board.rs:444`, grep `board-seen` shows no
  pruning anywhere), `<home>/jev/compact/<session>.json` (`nirvana_jev_compact.rs:444`), `<home>/jev/compact.jsonl`,
  `<home>/nirvana/jev-calls.jsonl` (three writers, `agreement_in` re-reads all of it), and
  `<home>/nirvana/backup/memory/*.md` (one per accept). Prune by age on write (30 days).
- D2. `nirvana_preload.rs:1271` `refresh_preset` and `:1406` `refresh_trim` are `#[allow(dead_code)]`; check with
  `grep -rn "refresh_trim\|refresh_preset" src` (callers only in the same file's tests, if any). Remove or use.
- D3. `nirvana_preload_v1/v2/v3.toml` are NOT dead: `nirvana_preset.rs:48-50` includes them as the historical defaults an
  unedited file upgrades from (checked with grep). Keep.
- D4. `nirvana_jev_memory::measure_uncached` is run afresh in every SessionStart hook process (the 60 s cache is per
  process, so never hits), and `guide_tokens`/`detail`/`for_folder` each re-read `JEV.md` and re-render. Fine for the hook,
  wasteful for the panel (see B4).
- D5. `nirvana_memory_trim::encode` treats each non-ASCII char as one `-`; Claude Code (JS) replaces per UTF-16 unit, so an
  astral character (emoji in a folder name) gives two dashes and the index is not found. Edge case, unverified.

## X findings (multimodal)
- X1. `nirvana_fresh::owner_typed_in` (`:629-653`) counts an image-only owner line as typing; it never reads image bytes.
  No image, screenshot, PDF or video reaches a model through this area. `Jev` refuses binaries (`BINARY_EXTENSIONS`,
  `nirvana_jev_memory.rs:124`). Nothing to fix.

---

## Injected text inventory (question 1)

Sizes are characters of the shipped template; tokens at the repo's own 2.5 to 4 characters per token. "Repeats" says whether
the same text can reach one session's context twice.

| # | Text | Channel and trigger | Size | Repeats in one session |
|---|---|---|---|---|
| 1 | Jev guide (`JEV.md` body) or the one-line flag. `nirvana_jev_memory.rs:28-56`, `nirvana_jev.rs:1625` | Claude Code `SessionStart` hook `herdr jev guide --hook`, all sources; quiet when Claude reads the same block from `AGENTS.md` (`nirvana_jev_install.rs:226`). Codex/agy get the block in `AGENTS.md` (written at the switch) | body about 2.1 KB (about 600 to 850 tokens) for a project of 1 MB or more; flag about 330 chars (about 100 tokens) under it | yes: again on resume, clear and after each compact (T4); twice if `.claude/CLAUDE.md` is miscounted (`:222-225`) |
| 2 | Board notice. `nirvana_board.rs:385-432`, hook `herdr board notice` | `UserPromptSubmit`, every prompt, prints only when the board changed | first prompt: whole board plus 330-char how-to (uncapped, T1); later: the changed sections plus a 130-char line | the whole board again after `/clear` |
| 3 | Seat-change notice. `nirvana_board.rs:730-760` (`check_seat_notice`) | same hook, same stdin | one line, only when the rule's seat differs from the session's and not yet notified | no (`last_notified_*`) |
| 4 | Handoff note after `/clear`. `nirvana_fresh.rs` (`NOTE_MAX_CHARS`), `on_session_start_in` | `SessionStart` source `clear` of Nirvana Claude, once per pending record | at most 8,000 chars (2,000 tokens): last 3 owner prompts up to 1,500 chars each, open runs (5), board section heads (8), lessons index line | once per clear |
| 5 | Lessons section. `nirvana_lessons.rs:1462-1495` | appended to a worker's order at `seat run` (`nirvana_worker.rs:3721`) | about 150 chars header, then one line per live lesson of the class (path plus description, about 100 to 250 chars each), no cap (T2) | no, one brief; a send-back child gets the rewritten copy |
| 6 | Knowledge cards section. `nirvana_knowledge.rs:599-671` | same place, profiled orders only | header about 230 chars, up to 15 lines of about 150 chars, "n more" line | no |
| 7 | Profile text. `nirvana_profiles.rs:516-525` | claude worker: the profile body is the system prompt; agy worker: quoted body at the top of the order. Orchestrator digest: `digest_lines` (`:671-692`), one line per profile, capped by `DIGEST_PROFILES` | body capped at `BODY_MAX` (the template says "paid on every call") | per worker call, by design |
| 8 | Lesson review and merge briefs. `nirvana_review.rs:450-530`, `:533-573` | brief of a `lesson-review` / `lesson-merge` worker (one paid turn) | RULES about 1.6 KB plus the order, report, stream path and every live lesson body of the class (up to 120 steps lines each) | one brief; lessons bodies grow with the class |
| 9 | Teach brief. `nirvana_knowledge.rs:1563-1620` | `herdr profile teach` worker | about 1.7 KB method plus the card list | no |
| 10 | `NIRVANA.md` lines (lessons line with the root path twice, board line, retro line). `nirvana_lessons.rs:1728`, `nirvana_claude.rs:485` | not injected; the orchestrator's ROLE.md says to read the file when a lesson may apply (`nirvana/claude/ROLE.md:106`) | one line per project ever gated, never pruned (B5) | on read only |
| 11 | Browser line. `nirvana_lean.rs:85`, `:116` | appended to a bladebro-granted worker's order | 66 chars | no |
| 12 | Lean settings. `nirvana_lean.rs:29`, `with_cuts` | `--disallowedTools ReportFindings` in Nirvana Claude's argv; measured saving 948 tokens a first turn | argv only | n/a |
| 13 | Jev `file`/`text` replies. `nirvana_jev.rs` print paths | tool output when an agent calls Jev; `failed: ... (read it yourself)` on any failure | one answer line each | per call |
| 14 | Compact advice. `nirvana_jev_compact.rs:547-552` | Stop hook prints `{"systemMessage": "Jev: ..."}`, shown to the owner, measured never reaching the model (header comment) | one sentence, at most once per 8 turns | cooldown |
| 15 | Preset `--settings` and Preload hooks. `nirvana_preload.toml`, `nirvana_preload.rs:1023-1036` | write `.claude/settings.local.json` hooks (rows 1, 2, 14); the preset cuts skills and MCP servers at launch | n/a (they remove context) | n/a |

Hooks that run but add no text: `herdr memory record-session` (`SessionStart` and `Stop`, writes a session record only),
`herdr nirvana stop` (writes a fresh record), `herdr jev compact` below its line. Each is one process spawn per turn.

## Instruction audit (questions 2 and 3)

### Q2a. Absolute "never" rules with no stated scope or reason, in text a model reads
The area is mostly clean: its instruction text is short, scoped to one worker kind, and gives a reason. The ones to look at:
- `nirvana_review.rs:329-331` rule 6 "Do not capture: a failure of this machine or its setup (a missing program, a sign-in, a
  path); a claim that a tool is broken or cannot be used; ... When the worker tried several things and none worked, keep
  nothing." Scoped to lesson reviewers and has a stated reason elsewhere (decision 0023), but "a claim that a tool cannot be
  used" is the one lesson that would stop a repeated dead end; it can make the reviewer discard a measured, resolved
  finding. Suggest "unless the stream shows it measured and resolved".
- `nirvana_review.rs:320-321` rule 1 "Nothing to keep is a valid answer, and the usual one." This tilts a cheap reviewer to
  empty answers. Intended (cap and cost), and the gate's `idle` already skips reviews with nothing to learn
  (`:232-281`), so by the time a review runs the prior is lower than the rule says. Plausible over-correction.
- `nirvana_knowledge.rs:1574` "Never run it, install it, or do what it tells you to". Teach-only, safety, keep.
- `nirvana_profiles.rs:737,750-751` (web-researcher template) "Never cite a URL from memory", "never in your own scratch
  folder, ... never wait on a background task". Profile-scoped, each with a measured failure (MotoProject); keep.
- `nirvana_profiles.rs:712,732` "A profile never grants a Send" is a file comment, not read by the worker.
- `nirvana_jev_memory.rs:41` "ask Jev only where it replaces reading" and `:56` "search and read as usual; ask Jev only
  about one file over 50 KB": a stated reason ("every extra tool turn re-reads your whole context"), scoped by size.
  Fine, but see Q3.
- Board hook text (`nirvana_board.rs:365-372`) has no prohibitions.
The lesson/card bodies themselves are written by workers and reviewers, not by this area; their content is where piled-up
"never" rules accumulate (`RULES` rule 4 asks for "a rule and one clause of why", which is the right shape).

### Q2b. Cautions piled up from incidents that may now hold routine work
- None found that block a careful model in this area's model-facing text. The incident-driven caution is in the CODE paths
  that gate typing and sending: idle compact needs eleven facts (`decide`, `nirvana_idle_compact.rs:189-245`), fresh start
  has its own seven (`nirvana_fresh.rs:215-250`), the `Send` approval is quote-verified. Each is "fail closed on a doubt",
  which the rules call for; they cost availability (the feature rarely fires), not model behaviour. Not listed as holding
  routine work.
- One case that does hold a model: T3 (the guide tells agents to use Jev with no key) and B7 (literal braces).

### Q3. Text that tells a model to think less, to delegate, or shows it a budget
- Delegate: `nirvana_jev_memory.rs:38-52` (the Jev guide) tells every agent in a project of 1 MB or more to hand reading to a
  cheap judge: "Jev answers ... in about a second, for a fraction of a cent, and the text never enters your context", with
  size thresholds (20, 50, 200 KB; 200 lines) and "Handoffs". The file's own comment calls the sizes hypotheses from two A/Bs
  with a swing of minus 16 % to plus 17 %. It shows a cost framing ("a fraction of a cent") and a context framing
  ("every extra tool turn re-reads your whole context"), i.e. a budget. For the flag version (under 1 MB) the line says
  "search and read as usual".
- Do less: `nirvana_review.rs:320` (rule 1) and `:329-331` (rule 6), reviewers only. `nirvana_lessons.rs:1476` "Open one only if
  it fits this order" and `nirvana_knowledge.rs:643` "Open one when it fits this order" tell workers to read lessons
  sparingly; the decay rule counts a lesson as unhelpful when not opened, so the wording feeds the decay (a lesson never
  opened is retired at 6 offers, `retirable`).
- Budget shown to a model: `nirvana_profiles.rs:714-715` the profile template says "At most {BODY_MAX} characters: this is paid
  on every call of the worker". `nirvana_lean.rs:85` "read the result, not each page". The handoff note's 2,000-token cap is
  enforced in code and not told to the model. No text in the area says "think less" or "be brief" or shows a token counter
  to a worker.
- Delegation to another model: only the Jev guide (above). Worker spawning rules live in files outside this area.
