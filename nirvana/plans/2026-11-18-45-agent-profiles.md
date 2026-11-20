# Plan 45: agent profiles from the sidebar, and lessons that propose them

Status: RULED 2026-11-19 (§7). P45c LANDED as alpha 0.87.0 and P45b as alpha 0.89.0; P45a and P45d follow.

## 1. The ask

The owner, 2026-11-18: "Another feature we could add is for each project we add a plus symbol next to agents and the
agents + becocmes  abutton to store profile. Theses would be besically where we store the agent proiles. Similar to
hermes profiles these agents would be specialized in h olding tools, mcp and most importantly context on how to process
a task. A common pattern we have used for example in the Megajam is a asset critique or reviwer. For motoProject I
would need a reviwer / video editor agent that unerstands how to uses the reeleditor has quick context on shots framing
and cinematography to help me create reels. Stuff like that. Utilzie the context / logs of past projects in Nirvana to
get a picture of how to create this change. Have lessons look for moments where it can consider building an agent like
we described for future runs."

## 2. Requirements ledger

| # | Quote | Surface | Owner | Verifier |
|---|-------|---------|-------|----------|
| L1 | "for each project we add a plus symbol next to agents and the agents + becocmes  abutton to store profile" | the sidebar's `agents` heading; a Profiles panel for that project's folder | P45a | a shell test that a click on the `+` opens the panel on the focused space's folder; the owner sees it in a window |
| L2 | "where we store the agent proiles" | `.nirvana/profiles/` (exists since 0.45.0); the panel lists, creates, opens | P45a | the panel lists what `herdr profile list` prints |
| L3 | "specialized in h olding tools, mcp" | profile keys for skills and a profile's own server file, beside today's `needs` and `disallowed_tools` | P45c | an order seated with the profile has the server and the skill, one seated without has neither |
| L4 | "most importantly context on how to process a task" | the profile body and its knowledge cards (exist); shapes to start from | P45c | `profile new --shape reviewer` passes `profile check` |
| L5 | "A common pattern ... in the Megajam is a asset critique or reviwer" | the `reviewer` shape; the judging block the builders retype | P45c, P45b | §4 M3 re-run on the next run |
| L6 | "For motoProject I would need a reviwer / video editor agent that unerstands how to uses the reeleditor has quick context on shots framing and cinematography" | `D:\MotoProject\.nirvana\profiles\` | R3 | a reel order seated with the profile; the owner judges the reel |
| L7 | "Utilzie the context / logs of past projects in Nirvana to get a picture of how to create this change" | §3 and §4 | this plan | the three audits of 2026-11-18 |
| L8 | "Have lessons look for moments where it can consider building an agent like we described for future runs" | a detector with no model, a `profile_ops` key in the lesson review, a proposals folder, a Suggested list in the panel | P45b | a fixture project with a repeated role yields one proposal; a project without one yields none |

## 3. What exists today

Profiles were built by plans 33, 34 and 36 (alpha 0.45.0 to 0.47.0). Checked on master 5eaf0ebe.

- **On disk.** `.nirvana/profiles/<name>.md`, or a folder `<name>/profile.md` with `knowledge/` cards. Seven keys:
  `description`, `class`, `model`, `family`, `effort`, `needs`, `disallowed_tools`. A body of at most 4000 characters.
- **Context.** The body is the Claude worker's appended system prompt, or a quoted section of an agy order. Cards are
  offered as one line each, 15 at most, and read on demand.
- **MCP.** `needs: [server]` copies that server from the project's `.mcp.json` into a per-order config, for Claude and
  for agy. A profile cannot carry its own server.
- **Tools.** Deny only. No allowed list, no skill selection.
- **Choosing.** Only the caller, with `--profile`. The session start prints the project's profiles to the orchestrator.
- **UI.** A Profiles group at the bottom of the Preload panel: `n` new, `e` edit, `K` cards, keyboard only. The
  Learning square lists profiles and their cards. Neither is reachable from the sidebar's `agents` heading.
- **Lessons.** A review may emit `lesson_ops` and, when its subject had a profile, `card_ops`. No op proposes a profile.
- **The journal.** No event names a profile. The only trace is text inside the `seated` reason.

So storage and delivery are built. The door, the tool surface and the proposing are not.

## 4. What the record says

Three read-only audits, 2026-11-18: the profiles code, Megajam's run `ia1`, and MotoProject with four other projects.

| # | Finding | Measured |
|---|---|---|
| M1 | A profile that is named in the role text gets used; one that is only listed does not | `web-researcher` was carried by 7 of 7 research orders seated after it was made in xtool and groove-coach. MotoProject's `reel-editor` was carried by 0 of 35. All 40 of its card counters read 0. |
| M2 | `reel-editor` was built for work that did not happen | Every reel order since 2026-10-22 builds a hike post through `scripts/post.py`. The moto-reel server refuses posts. No moto-reel tool was ever called, in 78 streams. |
| M3 | The Megajam "reviewer" is mostly a step inside builders, not a separate agent | 121 of 326 original orders capture frames and judge them with `herdr look`. They retype 288,922 characters, 34% of their text. The judging sentence is in 47 orders. The engine sharing block is in 119 orders in 23 wordings. Standalone visual critiques were 8 orders. |
| M4 | The profile did end retyping for the standalone critic | The review before the profile typed its rubric inline. After, the critique briefs repeat 3.6% of their text. The three reviews each read 8 of the 9 cards. |
| M5 | A judge that cannot be seated is skipped in silence | `herdr look` had no seat in 35 delivered orders, 18 of them on one day. Nearly all reported `done` on the worker's own reading. The gate does not check it. The reviewer profile, pinned to a family with native video, never hit this. |
| M6 | Making the reviewer profile by hand cost three workarounds | `profile teach` refused the profile's own `needs` and `family`, so the orchestrator commented them out with `sed` and restored them. Gate review orders copied the builder's `Profile:` line, were refused, and had the line stripped by hand. Two reviews of local files were sent back for naming no sources, and merged by hand. |
| M7 | Lessons hold role know-how that nothing moves to a profile | In Megajam 96 of 115 lessons are keyed to `ue-builder`, about 14 of them a judging method or a capture recipe. 105 lesson reviews proposed no profile, skill or agent. |
| M8 | Repeated roles with no profile, by project | 3D-Printer Flow: 42 orders needing the Bambu Studio server. WorkOutAppRefactor: 31 adversary reviews. MotoProject: 37 post builds and 15 render judgings. Megajam: 39 asset orders and 44 web prototype orders. |
| M9 | `herdr kb search` is not how workers read cards | 0 calls in the five reviewer streams. They read the card files from the paths herdr lists. |
| M10 | MotoProject keeps no lessons | 16 lesson reviews and 4 idle reviews ran. There is no `lessons` folder. Not explained; outside this plan. |

## 5. The design

The feature is three things: a door, a fuller profile, and a proposer. It extends the existing files and adds no second
store.

**The door (L1, L2).** A `+` after the word `agents` in the sidebar's bottom panel, which follows the focused project.
The project's right-click menu gets a `Profiles` line for the same panel. The panel shows, for that project's folder:

- each profile: description, seat pins, servers, cards, and its use, as orders carried against orders that fit;
- `Suggested`: proposals waiting, each with its evidence, and Accept or Dismiss;
- `New`: a name and a shape. The file opens in an edit tab, as Preload does today.

A profile with no use is shown as such. `reel-editor` would have read "0 of 35".

**The fuller profile (L3, L4, L5).**

- `skills: [a, b]` names project skills the worker gets. Today agy gets all of them and nothing selects.
- A profile folder may hold `mcp.json`, its own servers, merged into the per-order config the same way a `needs`
  server is. Only a hand-written or owner-accepted file counts. A proposal never carries one (§6).
- `profile new <name> --shape reviewer|operator|builder|researcher` writes the folder form with a starter body and
  starter cards. The shapes come from the profiles that worked:
  - `reviewer`: a rubric card with bands, "open every file yourself", "say what you could not check", what to put in
    `sources` for local files, a family with native image and video. From `aaa-anim-reviewer`.
  - `operator`: works only through named servers, drafts before the costly step. From `reel-editor` and the Bambu
    orders.
  - `builder`: standing project rules and shared-resource rules, said once. From `ue-builder`.
  - `researcher`: today's `web-researcher`.
- `seat run` with no `--profile` prints one line when a project profile fits the order's class and needs. The `seated`
  event gains a `profile` field, so use is counted from the journal.

**The proposer (L8).** Two halves, the first with no model.

1. A detector, run at the end of a run and by the idle review, reads the project's order copies and journal and finds:
   - a paragraph of 100 characters or more typed into 5 or more orders: text for a profile's body;
   - 5 or more orders of one class and one set of needs with no profile: a missing profile;
   - a profile that 5 or more fitting orders did not carry: an unused profile;
   - lessons of one class that the review marks as method, not trap: cards for a profile.
2. The lesson review and the idle review gain a `profile_ops` key: `propose` a new profile, or `patch` the body of one
   that exists. The detector's findings are in the brief as evidence. Every op must quote its evidence, and the draft
   must pass the profile parser.

A proposal is written to `.nirvana/profile-proposals/<name>.md`, never into `profiles/`. Accept moves it and writes a
ledger row, so it can be rolled back. Dismiss is recorded, so the same proposal is not made again.

**Friction found in the record (M5, M6).** Small fixes in the same files:

- `profile teach` does not take the taught profile's seat pins.
- A gate review order never takes its subject's profile. A lesson review already does not.
- A review of local files is not sent back for an empty `sources`. Why 0.53.0 did not cover this is to be found first.
- The gate notes a brief that asked for a look when no look ran.

## 6. What a proposal may not do

A worker's stream is not trusted text. A proposal drafted from it may set `description`, `class`, `effort`, the body,
cards, and `needs` that name servers the project already defines. It may not carry a server definition, a `skills`
entry that does not exist, `model` or `family` pins, or any text the profile parser refuses today, such as the send
flag. It is inert until the owner accepts it.

## 7. Rulings

| # | Question | Recommendation |
|---|---|---|
| R1 | Does a proposed profile go live by itself, or wait for Accept? | Wait. A profile changes seat picks and what a worker is told on every later order. Cards and lessons stay auto-live as ruled. |
| R2 | When a profile fits and none was passed: a line, or seat with it? | A line and a count first. Seat with it by default only if the count shows the line is ignored. |
| R3 | MotoProject: retire `reel-editor` as it stands and seed `reel-reviewer` and `post-builder` as the first use of this flow? | Yes. The reviewer's cinematography cards need sources: the owner's references, or a `profile teach` from links the owner picks. |

The owner, 2026-11-19: "1. yes, 2, go with reocmendation, 3, Idk if I want to retire it but let it ask if they can
just build only one pieces of it".

- R1: a proposed profile waits for Accept.
- R2: a line and a count at seating; no seating with a profile the caller did not pass.
- R3, as the orchestrator reads it, to be confirmed by the owner: `reel-editor` is not retired and nothing is seeded
  in MotoProject by hand. The flow asks: a proposal is one piece, a new profile or one change to a profile, and each
  is accepted or dismissed by itself. An unused profile is shown as unused and is never removed by herdr.

## 8. Packages

| Package | What | Files it owns |
|---|---|---|
| P45a | The `+`, the project menu line, the Profiles panel with use counts and New | `client/shell/agent_sidebar.rs`, `context_menu.rs`, `state.rs`, `mouse.rs`, a new `client/shell/nirvana_profile_panel.rs` |
| P45b | The detector and `profile_ops`, writing into P45c's store | a new `nirvana_profile_detect.rs`, `nirvana_review.rs`, `nirvana_idle_review.rs` |
| P45c | `skills`, a profile's own `mcp.json`, the shapes; the proposals store with list, Accept, Dismiss and rollback; teach without the profile's seat pins | `nirvana_profiles.rs`, `nirvana_knowledge.rs`, `nirvana_gui_tools.rs`, `nirvana_agy_tools.rs`, a new `nirvana_profile_proposals.rs` |
| P45d | The fitting-profile line, the `profile` journal field, the three other friction fixes | `nirvana_seat.rs`, `nirvana_worker.rs`, `nirvana_gate.rs`, `nirvana_run.rs`, `nirvana/claude/ROLE.md` |

P45c lands first: P45a's New and Suggested list and P45b's drafts all go through its format and its store. P45a,
P45b and P45d then build side by side on disjoint files. P45d shares files with plan 43's `seat run`
package, so the two are built one after the other, not together.

### What P45c changed from this plan

- §3's "Seven keys" is eight with `skills`. `skills: []` means none.
- A teach order, a lesson review and any `review.*` or `acceptance.*` order get no server from a profile. This narrows
  plans 33 and 34 and is the orchestrator's call; the owner may reverse it.
- A profile's own `mcp.json` is kept out of git by `.nirvana/profiles/.gitignore`, which herdr writes once and never
  rewrites. The run's copy of it goes when the last open order seated with it ends.
- The proposals store's `was_dismissed` takes kind, profile and key. A patch's `base_sha` is the whole `profile.md`.
- For P45d, from reading only: `review_brief` and `author_brief` in `nirvana_gate.rs` pass the subject's `Profile:`
  lines through, so a review of a profiled order may be refused at seating.

### What P45b changed from this plan

- The detector has three findings, not four. Lessons that read as method and belong on a profile's cards are left
  to the reviewer, by rule 4 of the rules a review is shown.
- One waiting proposal per kind and profile, in the store itself.
- A waiting proposal is not tracked: herdr writes `.nirvana/profile-proposals/.gitignore` when it is missing. The
  ledger is ignored with it. The owner's Accept is the one crossing into a tracked file.
- A paragraph that looks like it holds a secret is counted and never drafted, quoted or stored. The list of shapes
  is a second layer and is not complete.
- `herdr profile accept`, `dismiss` and `proposals rollback` are refused inside a seated worker, by the
  `NIRVANA_ORDER` marker every seated worker now carries. This is the orchestrator's call; the owner's
  orchestrator may still accept on the owner's word.
- A review writes at most 3 proposals.
- For P45a: `nirvana_profile_detect::usage(root)` gives each profile's carried and fit counts for the panel. The
  library `accept` prints nothing, so the panel must show `nirvana_profile_proposals::ACCEPT_NOTE` after an Accept.

## 9. Live checks owed after building

1. Press `+` in a project with profiles and one without.
2. Make a `reviewer` profile from the panel, teach it one source, seat one order with it.
3. Let a run with a repeated block finish, and see the proposal under Suggested with its evidence.
4. Accept one proposal, seat an order with it, then roll it back.

Plans 33 §6, 34 §6 and 36 §5 list owner checks of the existing profile features that have no record of being run.
