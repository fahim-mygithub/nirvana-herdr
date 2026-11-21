# Plan 47: the profile on the agents panel, and a scholar in every project

Status: RULED 2026-11-20 (§6). P47a LANDED as alpha 0.92.0, P47b as alpha 0.93.0. P47d and P47c follow, in that order.
This plan takes over the number 44 held for "scholar on agy": the owner's fetch rulings of 2026-11-18 are in §3.

## 1. The ask

The owner, 2026-11-20: "Im hoping for the UI under teh agent panal to showcase when we are using that build agent
profile for a task. I want every project to have the a default Researcher agent,called the scholar agent where it
breaks down our prompt to different research styles and runs parallel research agents and finally builds an artifact.
Similarly we have done these patterns before now I want this skill across all projects."

## 2. Requirements ledger

| # | Verbatim | Surface | Package | Verifier |
|---|---|---|---|---|
| L1 | "Im hoping for the UI under teh agent panal to showcase when we are using that build agent profile for a task." | the worker rows under an agent in the sidebar, the spaces view, the Profiles panel | P47a | tests on a drawn buffer; owner's live check |
| L2 | "I want every project to have the a default Researcher agent,called the scholar agent" | `nirvana_profiles`: a `scholar` profile no project has to make | P47b | `profile list` in a project with no profiles folder |
| L3 | "where it breaks down our prompt to different research styles" | a routing step that names the styles a question needs | P47c | tests on recorded questions; a live run |
| L4 | "and runs parallel research agents" | one order per style, seated side by side | P47c | the run's journal; rows on the agents panel |
| L5 | "and finally builds an artifact." | a report page written into the project, published when the session can | P47c, P47d | the file exists and every claim in it carries a source |
| L6 | "Similarly we have done these patterns before now I want this skill across all projects." | the owner's 16 research skills at user scope, which no worker can load | P47b | the profile's body and cards hold the method |

## 3. What exists today (read 2026-11-20 at alpha 0.91.0)

- **The agents panel.** A seated worker's pane has no row of its own. It is a row under its orchestrator, built in
  the window by a poller thread that replays the run journals (`nirvana_subagents.rs`). A live row is two lines:
  the order's name, then the model and the elapsed time. The row carries no profile, though the `seated` journal
  line has held one since 0.90.0.
- **Profiles are per project.** `nirvana_profiles::list` reads `<project>/.nirvana/profiles/` and nothing else.
  The `researcher` shape and the `web-researcher` template are text in the binary, written into a project only
  when someone runs `herdr profile new`.
- **The owner's research skills.** Sixteen skills in `~/.claude/skills`: `scholar-router`, `scholar-core`,
  `scholar-visual`, `scholar-media`, `scholar-artifact`, `scholar-log`, `scholar-recall`, `scholar-improve`, and the
  eight styles `breadth-then-cluster`, `competitive-analysis`, `decision-matrix`, `hypothesis-elimination`,
  `source-of-truth`, `synthesis`, `trajectory`, `benchmark`. A seated Claude worker starts with
  `--setting-sources project`, so it loads none of them. An agy worker is handed project skills only. The audit of
  2026-11-18 found 23 runs, no browser escalation ever, and no run tagged with an outcome.
- **Fan-out.** The orchestrator can seat orders side by side today, and each is gated. A worker may also start its
  own subagents, but those get no row and no gate.
- **An artifact.** No worker can publish a page: the publish tool is not in a `-p` session. Nothing in herdr writes
  or stores a report page.
- **The owner's fetch rulings, 2026-11-18.** "We wouldnt need api or should uses api wqe should use agy ffor fetch
  like tools", then "Agy and google has a deala to acces reddit links if the resource fails then uses browser
  automation to fetch it", then "That ladder makes sencse and youtube being pulled is ideal as well". Measured on
  headless agy: native search first; bladebro reads a full Reddit thread and a YouTube transcript when the check of
  the native answer fails; papers, Hugging Face and GitHub through their own APIs. A Flash order with no "open
  every source" line once invented 12 of 12 citations.

## 4. The design

**P47a, the profile on the row (L1).** The poller already replays the journal, so the row takes the profile from
the `seated` line, with the reason as the fallback for a line written before 0.90.0. A live row's second line
gains the profile's name. The spaces view's worker line gains it too. The Profiles panel marks a profile that a
live order carries now, with the count. Names from the journal are drawn through `visible`, as the panel's are.
Nothing is read while the window draws.

**P47b, a scholar every project has (L2, L6).** A profile named `scholar` that herdr carries in its binary.
`profile list`, the Profiles panel, the fit line and `seat run --profile scholar` see it in every project, marked
built-in. A project that makes its own `scholar` file overrides it, and the panel's edit key on the built-in one
writes that copy first. Its body holds the method in under 4,000 characters: the evidence tiers, "cite only what
you opened, with the URL and the exact quote", the fetch ladder of §3, when to stop, and the pass that argues
against the answer before it is written. Its cards are the eight styles, one each, and the ladder. A worker reads
a card when its order names that style. The text is distilled from the owner's skills, not copied: a worker cannot
load a skill, and the skills write outside the project.

**P47c, the scholar run (L3, L4, L5).** `herdr scholar "<question>"`, also offered to the orchestrator in
`ROLE.md` for any research ask.

1. Route. One short order reads the question and answers with the styles it needs and one sub-question per style,
   at most five. The eight styles are the menu. A question of one shape gets one order.
2. Research. One order per sub-question, seated side by side with the `scholar` profile and that style's card.
   Each is gated as research orders are: every cited URL must show as opened in the worker's stream.
3. Write. One order reads the gated deliveries and writes `report.md` and a single-file `report.html` under the
   project's `docs/research/<date>-<slug>/`. Every claim carries its source and quote. Where the parallel orders
   disagree, the report says so.
4. Show. The orchestrator is woken with the path. An interactive session that has the publish tool may publish the
   page; herdr itself only writes the file.

The rows of step 2 are what P47a draws: five workers under the orchestrator, each marked `scholar`.

**P47d, what the run needs fixed around it.** The gate must count the ways agy really reads a URL, or a scholar
order on agy is sent back for sources it did open (8 of 12 send-backs in the 2026-10-31 record; to be measured
again first). The scholar log: one line per run in the project, with an outcome the owner can set, so the
improvement loop the skills promised has data.

## 5. What this does not do

- It does not load the owner's user-scope skills into workers. That brings the owner's hooks, permissions and
  plugins with it, and one test turn cost 16 times more with home config.
- It does not publish anything by itself.
- It does not use the Gemini API key. Fetching is agy's own search, then the browser.
- It does not show the profile on the phone page. That page does not read run journals; it is its own package.

## 6. Rulings

| # | Question | Recommendation |
|---|---|---|
| R1 | Is the default scholar built into herdr, or a file written into every project? | Built in. It updates with herdr and adds no untracked file to a repo. A project's own `scholar` file wins. |
| R2 | Who runs the parallel researchers: herdr, as separate gated orders, or one worker with its own subagents? | herdr. Each researcher gets a row, a seat by the table and a gate. A worker's own subagents get none of the three. |
| R3 | Which seats? | Researchers on agy, by the ruling of 2026-11-18. The route step and the writer by the seat table for the project's mode. SOTA lifts the cost rule as it does elsewhere. |
| R4 | Where does the report live? | `docs/research/` in the project, tracked, so the next session can find it. The run's raw deliveries stay in `.nirvana/runs/`. |
| R5 | Video frames: allow the clip download, or a browser screenshot? | Carried from 2026-11-18, still open. Until ruled, a scholar order reads transcripts and says it saw no frames. |

The owner, 2026-11-20, answering R1 to R5 in order: "Built in is fine, herdr can run, go with recomnded, go with
recomneded, Allow clip downloads and browser screenshots".

- R1: the scholar is built into herdr. A project's own `scholar` file wins.
- R2: herdr runs the parallel researchers, as separate gated orders.
- R3: researchers on agy; the route step and the writer by the seat table for the project's mode.
- R4: the report lives in `docs/research/` in the project.
- R5: a scholar order may download a clip and may take browser screenshots to see frames. Until this ruling the
  draft text said "say you saw no frames"; it now says how to see them.

## 7. Packages

| Package | What | Files it owns |
|---|---|---|
| P47a | The profile on worker rows, the spaces view and the Profiles panel's "in use now" | `nirvana_subagents.rs`, `client/shell/agent_sidebar.rs`, `nirvana_spaces.rs`, `nirvana_profile_panel.rs` |
| P47b | The built-in `scholar` profile with its body and cards; the built-in mark; the override | `nirvana_profiles.rs`, the Profiles panel's row and edit key, `nirvana_profile_detect.rs` for the counts |
| P47c | `herdr scholar`: route, parallel orders, the writer, the report files; the `ROLE.md` rule | a new `nirvana_scholar.rs`, `nirvana/claude/ROLE.md` |
| P47d | The gate's reading of agy's fetches, measured first; the scholar log | `nirvana_gate.rs`, `nirvana_scholar.rs` |

P47a builds now. P47b lands before P47c. P47d's gate part is measured before anything is changed.

### What P47a changed from this plan

- The profile is a third line of a live worker's row, not the end of its second. At the default sidebar width the
  second line had 8 cells left and cut most names.
- The spaces view shows the profile only when its one-line row has room, which the default width rarely gives.
- "In use now" counts what the window's poller holds: a project whose orchestrator no panel drew in the last minute
  shows no count.

### What P47b changed from this plan

- The panel's edit key on the built-in does not write the copy at once. The first press shows a notice and the same
  key again makes the copy, so a stray key writes nothing into a project.
- `profile new scholar` refuses unless it copies the built-in (`--from scholar`) or says `--replace-builtin`.
- The built-in's cards cannot be rewritten, adopted or patched by a lesson review; a project refines its own copy.
- The detector counts the built-in as fitting `research.web`, so it no longer reports those orders as unprofiled.
- Every folder profile is now written under a temporary name and renamed into place.
- The order of the remaining packages changed: P47d's gate part lands before P47c, because a scholar run on agy
  would be sent back for sources it opened.

## 8. Live checks owed after building

1. Seat one order with a profile and see its name on the row under the orchestrator and in the Profiles panel.
2. Open the Profiles panel in a project with no profiles folder and see `scholar`, marked built-in.
3. Run `herdr scholar` on a question with two shapes; see the researchers side by side and open the report.
