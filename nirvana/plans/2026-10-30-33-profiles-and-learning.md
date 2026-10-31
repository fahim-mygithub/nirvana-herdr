# Feature 33: agent profiles per project, and lessons that are kept

Status: **PLANNED (2026-10-30)**. Two packages in two worktrees, built in parallel. The evidence is the Hermes Agent
source review and the Nirvana-Herdr map of 2026-10-30, copied to `nirvana/plans/2026-10-30-33-records/`.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "For our next feature I want to implement the profile agent from Hermes. Essentially give each project level aa designable agent profiles." | P33a | P33a | §2 tests |
| L2 | "Also in general deep research Hermes github https://github.com/nousresearch/hermes-agent What are features that we could bring to Nirvana. Pay particular atention to the self learning after each run." | P33b, §5 | P33b, orchestrator | §3 tests, the research records |
| L3 | "Sure" (answer to: may lessons be written in a folder with no GitHub remote, kept in that folder and never pushed; and may a review run after every gated order on a cheap seat, with a daily cap) | P33b §3.1, §3.2 | P33b | §3 tests |

- The ruling of 2026-10-16 ("keep it per github remote") is replaced by L3 for writing. Lessons still stay in their
  own project, and herdr never pushes them.
- Not in scope: Hermes' chat gateways, remote terminal backends, plugin hub, and whole-home profile isolation.
  Accounts, usage pools, the board and Preload stay shared across profiles. §5 lists the other features for later.

## 1. What was measured

- Nothing named a profile exists. Every Claude worker gets the same argv (`src/nirvana_worker.rs` `argv`), with no
  system prompt. Per-worker variation today: the Preload `--settings`, the per-order `--mcp-config` from `needs`,
  and brief lines added by code.
- Workers do write lessons, and all of them were lost. In `3D-Printer Flow\.nirvana\runs`, worker Work Reports
  carry 41 distinct lesson names, and `.nirvana/lessons/` holds 0 files, because the folder has no git remote and
  `verify_and_write` refuses without a GitHub one. Across every project on this PC: 2 lessons kept.
- About 25 of the 41 are narrow Studio GUI tips (`studio-gui-*`, `studio-open-dialog-click-field-first`, ...). Kept
  one per file, they would crowd the briefs. Hermes' answer is class-level "umbrella" skills.
- Hermes' learning loop (`agent/background_review.py`): after a turn is delivered, a forked agent with only the
  skill and memory tools reviews the transcript. Its prompt prefers patching an existing skill, says "The same lesson
  learned twice is ONE rule", treats the user's corrections as first-class signals, refuses a patch to a skill not
  read in the same review, and has a do-not-capture list. A weekly curator archives unused skills and, opt-in,
  merges them into umbrellas. Hermes never measures whether a skill helped. Nirvana's lesson counters already do.

## 2. P33a: profiles

**Worktree** `C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-p33a`, branch `p33a-profiles`, from this plan's commit.
**Owns:** `src/nirvana_profiles.rs` (new), `src/nirvana_worker.rs`, `src/nirvana_seat.rs`, `src/nirvana_claude.rs`,
`src/client/shell/nirvana_mods.rs` and any new `src/client/shell/nirvana_profiles*.rs`, `nirvana/claude/ROLE.md`,
`nirvana/claude/skills/seat/SKILL.md`, `nirvana/controls.d/p33a_profiles.py`, the module lists that register new
files. Anything else: an exact hunk in the report.

1. **The file.** `<project>/.nirvana/profiles/<name>.md`, tracked with the project. A front-matter block, then the
   persona body. `<name>` is `[a-z0-9-]{1,32}`.
   ```markdown
   ---
   description: Slices and sends prints in the beta Bambu Studio through its server.
   class: gui
   model: claude-sonnet-5-5
   effort: medium
   needs: [bambu-studio-beta]
   disallowed_tools: []
   ---
   You drive Bambu Studio only through the bambu-studio-beta tools. ...
   ```
   - `description` is required, at most 120 characters: the line Nirvana Claude picks by.
   - `class`, `model`, `family`, `effort`, `needs`, `disallowed_tools` are optional. Any other key is refused, naming
     the file and line, as `.nirvana/seats.toml` does.
   - The body is at most 4,000 characters. It is paid on every call of the worker (decision 0023).
   - A profile never holds `--send-ok` or anything else that grants a Send. Plan 31's rule stands: that is per order.
2. **Seating.** `herdr seat run <seat> --profile <name> --order <file>`, and `herdr seat pick --profile <name>`.
   - `model` and `family` narrow the candidate seats; they never skip usage floors, reserves or modes. When no
     candidate fits, the pick says so, naming the profile, and the order waits as an unseated order does today.
     `--override` keeps its meaning.
   - `effort` replaces the class effort. `class` is used when the order names none.
   - `needs` is joined to the order's own `needs`, before the plan 29 server routing and the plan 31 warning.
   - The profile file is copied, with its sha256, into the run folder at seating. A later edit never changes a
     running order. The `seated` event journals the profile name and the sha.
   - A send-back child and a reseat keep the parent's profile copy.
3. **The worker.**
   - Claude: `--append-system-prompt-file <run>/profiles/<name>.md` holding the body, and
     `--disallowedTools` when the list is not empty.
   - agy: the body goes at the top of the order copy, under `## Profile: <name>`, before the order text.
4. **Designing them.**
   - `herdr profile list|show <name>|new <name>|check [<name>]`. `new` writes the template above with the project's
     `.mcp.json` server names as a comment. `check` runs the parser and prints every refusal.
   - A Profiles group in the mods panel beside Preload, listing the project's profiles. `e` opens the file in an
     edit tab, through the same `command.invoke` path Preload's edit uses. `n` asks for a name and runs `new`.
5. **Nirvana Claude.**
   - The session digest lists each profile: name, description, model or family, needs. At most 10 lines; more prints
     a count and `herdr profile list`.
   - `ROLE.md`: when an order fits a profile's description, seat it with `--profile`. Name the incident: plan 31's
     audit, 22 of 22 GUI orders seated with no server.
6. **Tests and controls**: the parser's refusals (unknown key, long body, long description, bad name, a `send_ok`
   key); the seat narrowing and its no-fit message; the copy and sha in the journal, and an edit after seating not
   reaching the worker; the Claude argv and the agy brief; inheritance by send-back and reseat; the digest lines.
   A control for each.

## 3. P33b: lessons that are kept, reviewed and merged

**Worktree** `C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-p33b`, branch `p33b-learning`, from this plan's commit.
**Owns:** `src/nirvana_lessons.rs`, `src/nirvana_review.rs` (new), `src/nirvana_gate.rs`, `src/nirvana_retro.rs`,
`src/nirvana_seats.toml` (one new class and constants only), `nirvana/controls.d/p33b_learning.py`, the module
lists that register new files. Anything else, `src/nirvana_worker.rs` included: an exact hunk in the report.

1. **No GitHub remote needed (L3).** `verify_and_write` writes in any project. The quote-in-stream and passed-gate
   checks stay. Lessons are never pushed by herdr. Remove `has_github_remote` from the write path; keep the tests
   honest about the new rule.
2. **Profile scope.** An index entry gains an optional `profile`. A lesson written from an order seated with a
   profile carries it. `offer` lists, for an order with profile P and class C: lessons of class C with no profile,
   plus lessons of profile P. The profile name comes from the order copy's `Profile:` gate line, which this package
   adds to the gate's line list. P33a writes that line; the contract is the line `Profile: <name>` at line start.
3. **The review (L3).** After `herdr gate run` journals `Gated` (pass) or a send-back, herdr queues one review of
   that order.
   - It is a herdr order of a new class `lesson-review`, seated by the normal seat rules on the cheapest seat with
     spare quota (in `efficient` order: agy Flash, then Sonnet low). Nothing in it bypasses the seat table.
   - Its brief holds: the order text, the Work Report, the gate verdict and send-back reasons, the worker's own
     lesson if it offered one, and the live lessons of that class and profile, each with its body and a body sha.
     It does not hold the worker's whole stream; the brief names the stream file, and the reviewer may read it.
   - Its prompt carries Hermes' rules, rewritten for lessons: patch an existing lesson before writing a new one;
     the same lesson twice is one rule; a rule plus one clause of why; send-back reasons and the owner's
     corrections are the strongest signal; do not capture machine-specific failures, "tool X is broken" claims,
     one-off stories, or a failure that was never resolved. "Nothing to keep" is a valid answer and the default.
   - Its Work Report carries `lesson_ops`: a list of `{op: new|patch, slug?, base_sha?, name, class, steps,
     evidence}`. herdr applies each op only when the evidence is in the reviewed worker's stream, and a patch's
     `base_sha` is the lesson's current body sha (read before write). Every other op is refused with its reason.
   - A daily cap: `lesson_reviews_per_day` per project in `[constants]`, default 20. Over the cap the review is
     skipped and journaled `review_skipped` with the reason. A review never reviews a review.
   - The reviewer's own lesson, if any, is ignored.
4. **Merging.** `herdr lessons merge` and the same step at `herdr run end` when a class holds more than 8 live
   lessons: a `lesson-review` order whose brief holds that class's lessons and asks for umbrella lessons. Its ops are
   `merge {slugs, name, steps}`. herdr keeps every merged-away file under `.nirvana/lessons/.archive/` and appends
   each op to `.nirvana/lessons/ledger.jsonl` with the before and after bodies. `herdr lessons rollback <id>`
   restores one op. The merged lesson's counters are the sum of its parts.
5. **Measured, not assumed.** The index records each lesson's origin: `worker`, `review` or `merge`. The receipt and
   `herdr lessons stats` print, per origin, offered, opened and passed after opened. Decay is unchanged.
6. **Recovering the lost ones.** `herdr lessons import [--run <run>]` walks the project's run journals and writes
   each lesson a worker offered on an order the journal shows gated pass, through `verify_and_write` with the same
   checks. It prints written and refused counts with reasons. It changes nothing else.
7. **Tests and controls**: a write with no remote; the profile filter in `offer`; a review queued after a pass and
   after a send-back, and not after a review; the cap; each `lesson_ops` refusal (evidence not in the stream, stale
   `base_sha`, unknown op); merge archive, ledger and rollback; counters summed; import on a fixture run with a
   passed and a failed order. A control for each.

## 4. Merge order

- P33b first (it changes no argv), then P33a. Each merge runs `python nirvana/version.py bump feature` in the merge
  worktree with its CHANGELOG entry.
- After both: run `herdr lessons import` in `3D-Printer Flow` from master's build and report the counts to the owner.
  This writes into the owner's project folder, so it is the owner's go-ahead at hand-over, not a builder step.

## 5. Other Hermes features, ranked, not in this plan

1. Per-task cheap models (`auxiliary.<task>` in Hermes' config). P33b's review is the first; summaries next.
2. Shadow-git checkpoints before each turn's file edits, with rollback. For folders with no remote.
3. A consecutive-failure circuit breaker and a warning at about 90% of a worker's step budget.
4. Cron orders with delivery to the phone.
5. An LLM reviewer for dangerous commands; unattended runs refuse instead of waiting.

## 6. Live checks for the owner

1. Make a `slicer` profile in `3D-Printer Flow` from the mods panel, and have Nirvana Claude seat a Bambu order with
   it. Read the `seated` event: profile name and sha.
2. After the next gated order, read `herdr lessons stats`: one review ran, on a cheap seat, and what it kept.
3. Run `herdr lessons import` in `3D-Printer Flow` and read the counts.
