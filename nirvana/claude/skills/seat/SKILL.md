---
name: seat
description: Seat an order on Claude or Antigravity through herdr. Use for every order you hand off - classify it, run `herdr seat pick`, start it with `herdr seat run` or a worker subagent, read it, and gate it with `herdr gate`.
---

# Seating an order

`herdr` below is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`; the session digest names it too.
A `herdr` found on PATH may be upstream herdr, which has none of these commands.

## 1. Classify (your own turn, no extra call)

Pick the **class**, the **needs** and the **size**; herdr picks the seat (plan 12 §5.1).

| Class | For |
|---|---|
| `plan` | splitting an ask into orders |
| `build.load_bearing`, `build.bounded`, `build.frontend` | code: core and risky, contained, UI |
| `fix.debug` | a root cause |
| `ops.terminal` | running and fixing commands, servers |
| `review.adversary`, `acceptance.author` | a review by another family; a held-out check written before the build |
| `research.read`, `research.web`, `read.huge` | the repository; the web; an input over 400k tokens (`context_1m`) |
| `judge.visual`, `media.video`, `media.audio` | screenshots; video (`video`); audio (`audio`): the `media` skill |
| `write.docs`, `sweep.mechanical`, `chore.memory`, `chore.script`, `chore.github` | docs; mechanical sweeps; chores |

Size: `small` (minutes), `medium` (under an hour), `large`. A question is answered, never built: a question is
`research.read`, and you answer it yourself when a few reads do.
Web research, shopping and reading pages are `research.web`: seat them, never browse yourself for more than one page.

## 2. Pick

```
herdr seat pick --class build.bounded --size small [--needs video] [--not-family anthropic] --book <order> --run <run> --json
```

It prints the seat, model, effort, pool and the reason line, and books the order's expected use so parallel picks
see each other. When the order fits one of the project's profiles (the session digest lists them; `herdr profile
list`), add `--profile <name>`: its `class` stands in when you name none, its `needs` join yours, its model or family
narrows the seats and its effort replaces the class's. Exit 3 with a profile names it: no seat fits that profile. Say the pick in one line: `build.bounded -> sonnet medium (max): <reason>`. Exit 3 is no seat:
its `earliest_reset` says when; defer, or ask the owner. To overrule, `--override <seat> --reason "..."`, at most 3
a run.

## 3. Start it

Write the order to `.nirvana/orders/<order>.md`: the goal, the files it owns (a `build.*`, `fix.*` or `plan` order
must carry an `owns:` line; `herdr seat run` refuses without one), the check that proves it, and the
owner's words where they matter. Then:

- **A pane worker** (anything longer than a few minutes, and every agy or local order):
  `herdr seat run <seat> --order .nirvana/orders/<order>.md --order-id <order> --run <run> --class <class> --size <size> --worktree`.
  It reuses the booking, opens a pane, and returns at once. A second `seat run` of the same order returns the first.
  An order that drives Bambu Studio or xTool Studio adds `--needs bambu-studio-beta` or `--needs xtool`, the
  project's `.mcp.json` server, so its worker gets the server's tools. An order that may press Send adds
  `--send-ok "<the owner's words>"`, copied exactly from the owner's prompt of the last 6 hours (4 words or more);
  herdr refuses a quote it cannot find, and a send-back or reseat never inherits it.
  `--needs browser` or `--needs bladebro` gives the worker bladebro, a headless browser with its own profile, on a
  claude or an agy seat. `browser` also limits the pick to the seats that list it, opus, sonnet, fable and the four
  agy seats now, so `--needs browser` is the way to a browser on agy too; `bladebro` does not limit the pick. A
  profile that denies `mcp__bladebro__*` wins, and so does an agy workspace whose own `.agents/mcp_config.json` names
  one. A desktop server need (`bambu-studio-beta`, `xtool`) still takes a claude seat.
  An order picked with `--profile <name>` is run with the same `--profile <name>`: herdr copies the profile into the
  run, gives the worker its persona (a system prompt on Claude, the top of the order on agy), and a send-back or
  reseat keeps the copy. Design one with `herdr profile new <name>`, then `herdr profile check <name>`.
- **A worker subagent** (a short read, review or chore on Claude that needs no pane): spawn
  `nirvana:worker-<seat>-<effort>` with the seat and effort the pick gave (`fable-high`, `fable-medium`, `opus-high`, `opus-medium`,
  `sonnet-medium`, `sonnet-low`, `haiku-low`). Every subagent is seated first: `herdr seat pick --book <order>`,
  then spawn the seat it gives, or `herdr seat run` when it gives an agy seat. The hook refuses a spawn without a
  booking. Start its brief with three lines, `Class: <class>`, `Order: <order>`, `Run: <run>`; herdr records its
  outcome under them when it stops. A spawn that names no model or effort, or that would break your reserve on `max`,
  is refused by the `PreToolUse` hook.
  Fable 5.2 is seated only when the Fable week was read fresh and keeps its reserve: past its week a headless
  Fable run bills usage credits without asking, so never name Fable by hand to get around a skip.

### An agy research or media order

agy reads no `CLAUDE.md` and grades a check written as prose itself. Each rule names the MotoProject failure it
answers (`nirvana/plans/2026-10-31-35-records/moto-gemini-briefs.md`). herdr's own sources line stays under the
brief as the floor.

1. The verify line, word for word: "Never cite a URL from memory or from a search result. Cite only what you opened
   in this session; drop what you couldn't open." reel-motion-gemini's brief lacked it and its 12 sources were
   invented; the 4 briefs with it cited 47 sources, all real, one on the same flash-low seat.
2. A read method per kind of source, naming only tools the worker has: a `search_web` link is a vertexaisearch
   redirect, resolved with `curl -sI` and its `Location`; YouTube by
   `python -m yt_dlp --skip-download --print "%(id)s|%(title)s|%(channel)s" <url>` (`--write-auto-subs` for a
   transcript); Reddit by `curl -s -A "<ua>" <thread>/.rss`, waiting out a 429; any other page one `curl` each, or
   bladebro when the order was seated with `--needs bladebro` or `--needs browser`. agy has no YouTube connector: r2 was promised one,
   and spent 9 steps finding the method and about 10 failed Reddit reads.
3. "Done means" ends with a command whose output goes into `tests`: for research, one last command re-opening every
   cited id (`python -m yt_dlp --skip-download --print id <url>...`). reel-motion-gemini graded its own prose check
   "pass" after opening nothing.
4. `owns:` in lowercase, one line per path: the gate ignored reel-motion-gemini's `Owns:` until P35a.
5. The project rules the order touches, quoted word for word: the E: rule for working files lived only in the
   reel-editor profile, and r2 left 6 mp4s on C:.
6. Where working files go, and that long steps run in the foreground ("a draft render takes about N min; run it in
   the foreground; never wait on a background task"): v12-smudge-fix waited on its own background render and
   stalled 600 s.
7. The current state as absolute paths (the current plan file, the latest report) and the owner's words verbatim:
   reel-motion-gemini asked about a 10-slide post weeks after the reel became 13 slides of video.
8. Explore, read, write, verify, in that order: find candidates, open each, write the file, run the done-check.
   reel-motion-gemini searched and wrote, and opened nothing.

The checklist, before `herdr seat run`:
- [ ] class line, and `--profile web-researcher` when the project has it
- [ ] lowercase `owns:`
- [ ] the owner's words verbatim, and the current state as absolute paths
- [ ] the project rules quoted
- [ ] the verify line
- [ ] a read method per source type, only real tools
- [ ] where working files go, and foreground only
- [ ] a "done means" that ends in a command whose output goes in `tests`

### A research question: `herdr scholar`

Plan 47 P47c (the owner, 2026-11-20: "breaks down our prompt to different research styles and runs parallel research
agents and finally builds an artifact"). A question that needs outside sources and more than one page is not briefed
by hand: `herdr scholar "<question>"` routes it, seats the researchers and the writer, and wakes you once.

- Put the owner's words in the question, and the constraints that matter: the project, versions, the deadline, what
  the answer is for. The route turn sees only the question and the titles of the reports already in `docs/research/`.
- Then end your turn. herdr runs one short route turn, at most five `research.web` orders side by side on agy with the
  built-in `scholar` profile (each reads its style's card), then one `write.docs` order that writes
  `docs/research/<date>-<slug>/report.md` and `report.html`. The driver handles a researcher's and the writer's
  delivery, verdict, question and interruption, so none of those wakes you; a stall, a deferral, a rate-limit reseat or
  a doubt about one of their workers still does, as any order's would.
- The one wake for the outcome is the driver's end line, under the wake's `herdr run <run>:` header.
  `scholar report: <path>: <answer>` opens with the page's path; `scholar report (2 of 5 lost): <path>` says two of
  five researchers did not deliver, and the line goes on to name them and why. Tell the owner the path, and what was
  lost. Publish the page only when this session has the publish tool and the owner wants it.
- `scholar <end>: <why>` (with `(k of n lost)` after the end word when researchers did not deliver) is a run that ended
  without a report (`nothing_to_research`, `no_delivery`, `no_report`, `report_incomplete`, `stopped`): run
  `herdr scholar status <run>` and tell the owner what it says.
- An interrupted researcher or writer is the driver's: it reseats it once. `scholar gave_up` means the driver itself
  stopped, and a dead end naming `herdr scholar resume <run>` (`herdr run check`, the digest, the Stop hook) means no
  driver holds the run: answer either with that command and nothing else. `herdr scholar stop <run>` stops a run.
- After the owner reads the report, `herdr scholar tag <run> hit|partial|miss --note "<their words>"` records it in
  `docs/research/log.jsonl`.
- `herdr scholar` refuses in a folder that is not a git repository, inside a seated worker, and while another scholar
  run of the project has a live driver (`--force` starts one anyway).
- A single lookup (one fact, one page) stays an ordinary `research.web` order, briefed by the checklist above and
  seated with `--profile web-researcher` when the project has one.

## 4. Read it

End your turn while workers run; herdr wakes you when an order delivers, asks, fails or stalls.

- `herdr seat status [--run <run>]`: each order's phase, seat, tokens.
- `herdr seat read <order> [--tail 40]`: the worker's stream, readable.
- `herdr run status <run>`, `herdr run check <run>`: the run and its next step.
- `herdr seat stop <order>`: stops it with a checkpoint; never rerun an interrupted step blindly.

## 5. Gate it

herdr gates each `done` or `blocked` delivery itself, before it wakes you, and the wake says
`<order> gated <verdict>: <reason>`, with no reason on a pass (plan 37 §1.1). A `blocked` delivery is gated for what
it did deliver and then asked, never sent back, its question opening with the block (plan 37b). A helper's delivery
(a reviewer's, a held-out author's) and a gate that waits on a review you gate by hand; `herdr gate run` again names
the next step, a booked reviewer's `herdr seat run` line included, and for an order already sent back or asked it
gives the child or the question again. A gate that stopped wakes you as `the run asks: herdr's gate of <order>
stopped`: run it by hand. A cited YouTube video that does not exist fails the sources axis, and its redo goes
one seat up with a brief that trusts nothing from the first try; when no stronger seat is open herdr asks you instead
of redoing it on the seat that invented it.

- `herdr gate run <order> --run <run>`: rules on a delivered order. Exit 0 passed; 1 sent back or asked, with the
  reason; 4 waits for a review or a held-out check.
- `herdr gate merge <order> --run <run>`: merges a passed order into the run's integration branch. A merge to
  `main` asks the owner, except in auto mode within its budgets.
- `herdr gate amend <order> [--owns <glob>]... [--test "<cmd>"] [--send-back]`: for a `gated amend` wake, a refusal
  no worker can fix (`owns:` too narrow with no live order holding the files, a `test:` that does not parse,
  herdr's own files). The wake cuts its reason at 90 characters, `…` marking the cut, and ends with
  `full line: herdr gate run <order>`, which prints the amend line whole. The amend adds the owns or replaces the
  test, journals the old and new values as `gated amending` before the re-gate, and re-gates the same branch with no
  new worker; an order left amending is re-gated by `herdr gate run`. `--send-back` seats a worker instead. It
  refuses an order gated asked (the owner's answer is pending), one delivered and not gated yet, and one whose child
  is live. A new file outside `owns:` that no live order owns is a note (`owned_new`), not a fail (plan 43), unless
  it is fenced: a dot-prefixed path, `nirvana/controls.d/`, `build.rs`, `conftest.py`, a git hook, or a
  credential-looking name fails as the worker's.
- `herdr gate merge <order> --by-hand "<reason>" [--sha <sha>]`: records a merge you made yourself, with its commit.
  It takes none of `gate amend`'s flags and exits 2 on a refusal, as `gate amend` does.
- `herdr gate status [<order>] --run <run>`.
- An order whose seat has an advisor in its mode starts with Claude Code's `--advisor`, and `seat run` prints
  `advisor <model>`. Its consults, tokens and dollars are the journal's `advisor` row and count in the order's use; a
  row with `dropped` ran without one, and says why; a row with `unseen` had no consult and no advisor tokens, so herdr
  cannot tell whether the advisor was on.

The `gate` skill holds the axes and the send-back loop. When the plan is done and gated, `herdr run end <run>`
writes the receipt; name its file in your answer.
