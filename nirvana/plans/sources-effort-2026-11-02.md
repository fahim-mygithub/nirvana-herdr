# Sources only when the order asks for the web; a send-back keeps the caller's effort

Built 2026-11-02 on `gate-sources-effort`, off master 871acaf2 (alpha 0.52.0). The evidence is
`live-checks-2026-11-01.md`, findings F2 and F3. The owner, 2026-11-02, on the recommendations for both: "Use your
recomendation for all of these".

## F2, the sources rule

### What the live checks saw (read from the scratch run, `runs/lc35/`)

- lc3's order (an MCP call) and lc4's (`research.read`, quote the project rules without opening a file) carry herdr's
  line `Your Work Report must include the `sources` array of `{url, used_for}` for each source consulted.`, added by
  `seat run` to every `research.*` and `media.*` order (`nirvana_worker.rs`, before this package), and agy's
  `SOURCES_LINE` after it.
- `sources_axis` failed every research or media delivery that cited nothing: "no sources: the Work Report names no
  sources".
- lc4-s1's brief (`orders/lc4-agy-rules-s1.md`) opens "fix what the gate found, do not start over", lists
  `**sources**: no sources: the Work Report names no sources`, and quotes the original order with herdr's line in it.
  Its worker ran `curl -s https://example.com` (stream line 57) and cited it "as required by the order's sources
  specification" (line 59). So the "sources specification" was herdr's own line, made binding by the send-back's
  finding over it.

### The rule

An order asks for the web when (`nirvana_gate::web_asked`, read at seating):

1. its class is `research.web` (ROLE.md: "Web research, shopping and reading pages are `research.web`"); or
2. it needs `browser`, `bladebro` (the two that grant bladebro, `nirvana_bladebro::GRANTING`) or `web`; or
3. its text as the orchestrator wrote it, before herdr's lessons, cards and lines (which name pages of their own),
   holds a URL whose host is not this machine, a bare domain with a path (`github.com/org/repo`), a word that names
   the web outright (`WEB_NAMES`), or a lookup word (`LOOKUP_WORDS`) in a sentence that names no project file or
   folder (`LOCAL_MARKS`). Words match as words, case ignored; `online` and `google` alone are left out, since a
   research order says "online docs" or names a Google API as often as it asks for a page. The review round's table
   gives each word.

Why these three: the class is the orchestrator's own statement (and `research.web` is the only class whose `why` names
fetching); a need is what grants the tool; the text catches a `research.read` or `media.*` order that names a page
without a class or need for it. `media.*` alone is not the web: `media.video` and `media.audio` are processing on
Gemini (`nirvana_seats.toml`), usually of a local file.

`seat run` writes `WEB_SOURCES` (the old line, unchanged) for an order that asks, and `NO_WEB_SOURCES` for any other
research or media order: "herdr found no web page asked for in this order, so the gate asks you for no source. If
answering needs a page you do not have, open it and cite it in the Work Report's `sources` array (`{url, used_for}`);
never open one only to have a source." agy's `SOURCES_LINE` still follows either. The gate reads the decision back
from the root order's copy (`Spec::no_web`, set by a line starting with `NO_WEB_MARK`, the line's first clause) and
the class again (`web_of`): a `research.web`
order always asks; an order with neither line (seated before this package) asks, as every research order did.

`sources_axis`: an order that asks and cites nothing (or delivers no report) fails as before, the reason now naming why
it asks; an order that does not ask, cites nothing and opened no page gets a note, `no web asked; none cited and no
page opened (...)`; one that opened a page and cites nothing fails. Every source an
order cites is checked as before, web or not: the stream rule, the stable ids, the oEmbed check, the invented-source
step-up are unchanged.

A send-back of an order seated with `NO_WEB_SOURCES`, on any axis, opens with `NO_WEB_REDO`: "herdr found no web page
asked for in this order: the gate wants a source from you only for a page you open. If the fix needs a page you do not
have, open it and cite it; never open one only to have a source." A sources note is never in a send-back's findings
(only failing axes are).

### What a lazy worker can now get away with

- On an order that asks for the web: nothing new. Citing nothing fails; a cited URL the stream did not fetch fails; a
  cited video oEmbed calls missing fails and steps up.
- On an order that does not ask: citing nothing passes with a note only when the worker opened no page (review
  round, S4). A worker that fetched, read or browsed a page off this machine and cites none fails. A source it does
  cite is checked, so it cannot cite an unread page. lc3's MCP call is to a project server, not a browser, so it is
  no page read.
- The remaining gap: a current-fact order the words miss ("What does the Bambu H2C cost today?"), answered from
  memory with no read, passes with a note; `gsrc_a_no_web_order_that_opened_pages_must_cite_them` pins it. herdr
  cannot tell a memory answer from a looked-up one without a read; the guard is the orchestrator's class, and ROLE.md
  now says a current fact is `research.web`.

## F3, the effort

`herdr seat run --effort e` applies the caller's effort after the pick (`book_with`, "effort e by the caller"). Every
re-pick of a child built a fresh `Request` with no effort in it, so the rule gave the class's effort:

| Path | Where | Before | Now |
|---|---|---|---|
| send-back | `nirvana_gate::send_back` | class effort (research.read: medium; lc4 low -> lc4-s1 flash medium) | the parent's |
| step-up (plan 37 §1.3) | the same, `request.override_seat = (up, reason)` with no `:effort` | class effort on the stronger seat | the parent's: `step_up` names a seat, never an effort |
| inherited override | the same, `seat:effort` | the parent's already | unchanged |
| inherited override refused, the rule picks | the same | class effort | the parent's |
| rate-limit reseat | `nirvana_worker::rate_limited` | class effort, unless the override was kept | the parent's |

Since the review round, each re-pick sets `Request::effort` to the parent's effort before the pick, and the rule takes
it as the base in place of the class's and the profile's (`effort_for`), so the windows, the use basis, the lease's
expected use and the pressure check are all the child's effort's. The first round patched the pick after the fact
(`keep_effort`), which left them at the class's effort. A seat that does not offer the parent's effort runs at the
nearest it offers below, and the reason says `the parent's effort <e>; <seat> has no <e>; <got>`; otherwise `effort
<e> kept from the parent`. The pressure ruling is in the review round.

## Tests and controls

Tests (all `gsrc_`): `nirvana_gate::tests::{gsrc_the_web_is_asked_by_the_class_a_need_or_the_order_text,
gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note, gsrc_a_send_back_of_a_no_web_order_wants_no_fetch,
gsrc_a_send_back_keeps_the_parents_effort, gsrc_a_stepped_up_redo_keeps_the_parents_effort,
gsrc_a_seat_without_the_parents_effort_keeps_the_picks, gsrc_the_docs_say_sources_only_when_the_order_asks_for_the_web}`,
`nirvana_worker::tests::{gsrc_seat_run_asks_sources_only_of_an_order_that_asks_for_the_web,
gsrc_a_rate_limited_reseat_keeps_the_parents_effort}`. The F2 tests use lc3's and lc4's order texts as written.
Each effort test first asserts the rule alone gives another effort, so it has something to keep.

Controls: `nirvana/controls.d/gsrc.py`, each run alone and red. Two older controls in `run_fixes.py`
(`p24_gate_sources_axis_in_deterministic`, `p24_sources_axis_empty_fails`) had their find text moved by this package;
both re-pointed at the new lines and re-run red. The review round's tests and controls are in its section.

## Where the order was wrong or unsure

- The order says ROLE.md and the gate SKILL.md "state the sources rule". ROLE.md did not state it (only "Web research
  ... are `research.web`"); a rule line was added under that one. The seat skill (owned by order L) names "herdr's own
  sources line" under the agy brief rules; the hunk is in the report.
- Not run live: no probe, no network (the order's rule). The effect on a real agy redo is a hypothesis until a live
  check seats a no-web research order.

## Review round (gsrc-review.md on dc90c7a6)

The principle from the order: what the worker did decides first, the words second. Each finding was checked against
the code before it was fixed; every one was right, and two were short (below).

| Finding | What I did | Test | Control |
|---|---|---|---|
| S4: the no-web arm never read the stream | `sources_axis` reads the stream first; `page_reads` keeps the fetches, URL reads and browser calls `stream_reads` counts, less those whose every URL is this machine; a no-web order that opened any and cites none fails and names them | `gsrc_a_no_web_order_that_opened_pages_must_cite_them` (curl, a bladebro click with no URL, localhost, the memory gap, a cited page) | `gsrc_opened_pages_ignored`, `gsrc_opened_local_counts`, `gsrc_opened_browse_skipped` |
| S2: the no-web lines talked a worker out of a fetch | `NO_WEB_SOURCES` and `NO_WEB_REDO` say: if answering (the fix) needs a page you do not have, open it and cite it; never open one only to have a source | `gsrc_a_send_back_of_a_no_web_order_wants_no_fetch` | `gsrc_line_talks_out_of_fetch`, `gsrc_redo_talks_out_of_fetch` |
| S1: needs lost between `pick --book` and `seat run` | `Pick::needs` (skipped in JSON when empty) holds the needs as given: `seat pick --book` sets it from `PickArgs::needs`, taken before plan 29's routing drops `bladebro` and the servers, and `book_with` sets it for `seat run --class`; `seat_order_with` reads the run's needs and the booking's | `gsrc_seat_run_reads_the_needs_the_booking_was_made_with` (books through `nirvana_seat::cli` with `browser`, `bladebro`, none; then `seat run --class --needs bladebro`) | `gsrc_seat_ignores_booked_needs`, `gsrc_pick_books_no_needs`, `gsrc_pick_books_routed_needs`, `gsrc_seat_run_books_no_needs` |
| S3: words missed | URLs, bare domains (`bare_domain`), `WEB_NAMES` anywhere, `LOOKUP_WORDS` in a sentence naming no project file; words match as words, a plural `s` allowed | `gsrc_the_words_catch_lookups_and_skip_local_hosts_and_code` | `gsrc_web_url_ignored`, `gsrc_bare_domain_*`, `gsrc_web_names_ignored`, `gsrc_lookups_ignored`, `gsrc_word_*`, `gsrc_sentences_whole` |
| S5: false positives | `local_host` drops localhost, `*.localhost`, `127.*`, `0.0.0.0`, `::1`; `website` and `internet` only in the forms below; `LOCAL_MARKS` spares a lookup sentence about the project; `has_word` stops `the web` matching `the webhook` | the same | `gsrc_web_url_local_counts`, `gsrc_local_host_*`, `gsrc_local_marks_ignored`, `gsrc_mark_*`, `gsrc_word_runs_on` |
| S6: `keep_effort` left the pick at the old effort and forced effort past the pressure check | `keep_effort` is gone; `Request::effort` makes the rule pick at the parent's effort: SOTA does not lift it, the high cap does not cut it, §5.5's step down applies to it, the mode's climb steps from it | `gsrc_a_child_is_picked_at_the_parents_effort_and_pressure_now_steps_it_down`, `gsrc_a_childs_base_is_the_parents_effort_for_the_mode_the_cap_and_the_climb`, the three re-pick tests | `gsrc_parent_effort_ignored`, `gsrc_parent_note_*`, `gsrc_parent_never_stepped`, `gsrc_parent_lifted_by_sota`, `gsrc_parent_capped`, `gsrc_climb_base_is_class`, `gsrc_send_back_drops_effort`, `gsrc_reseat_drops_effort` |
| N1: no control for the early return | moot: the function is gone | none | none |
| N2: say what a stepped-down parent passes on | in `Request::effort`'s doc and the ruling below | the pressure test | `gsrc_parent_never_stepped` |
| N3: the whole literal as `spec`'s prefix | `NO_WEB_MARK`, the line's first clause | a re-wrapped copy in `gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note` | `gsrc_spec_reads_whole_line` |
| N4: a count in prose | dropped; `controls.py --dry-run` prints the count | none | none |
| N5: two controls on one sabotage | the step-up control now spoils the override arm of `effort_for` (`&& !is_override`), which a plain send-back never reaches | `gsrc_a_stepped_up_redo_keeps_the_parents_effort` | `gsrc_step_up_drops_effort` |

Older control re-pointed: `seat_effort_never_max` (controls.py), whose line now carries `&& parent.is_none()`.

### The pressure ruling (S6)

Both cases have one rule: the parent's effort, as it ran, is the child's base, and pressure is read again at the
child's pick.

- A child under pressure now steps down from the parent's effort (§5.5's second pass; the mode's climb steps from
  the parent's effort, never from the class's). The first round forced the parent's effort past this check, and a
  headroom rule a re-pick can skip is no rule.
- A parent stepped down under pressure passes the stepped effort on, and the child is not raised back. Raising would
  spend more on the pool that was pressed, and the climb's step is not recorded on the pick (`build_pick` flags only
  §5.5's), so undoing one kind of step and not the other would treat them unequally. A child stepped again by the
  climb is floored at the class's `efficient_effort`.

### The words (S3, S5)

Every row is a row of `gsrc_the_words_catch_lookups_and_skip_local_hosts_and_code`. Only research and media orders get
a sources line, so the orders that must not be caught are research orders about the project or this machine.

| Word | Catches | Must not catch |
|---|---|---|
| a URL | "Summarise https://example.com/spec." | "Call http://localhost:8770/api/runs and list the run ids." |
| a URL, 127 | "Read http://docs.example.org/a." | "Quote what http://127.0.0.1:8770/health answers." |
| a URL, IPv6 | "Read https://[2001:db8::2]/notes." | "Quote what http://[::1]:8770/health answers." |
| a bare domain | "Read github.com/fahim-mygithub/nirvana-herdr/issues and list the open bugs." | "List what nirvana/controls.d/gsrc.py controls." |
| a bare domain | "Check docs.rs/serde/latest for the derive attributes." | "Read ./src/nirvana_gate.rs and list its consts." |
| the web | "Search the web for the H2C's nozzle sizes." | "Summarise the webhook handler's retry rule." |
| youtube | "Describe the YouTube video the owner names." | "Describe the clip at E:/clips/a.mp4." |
| tiktok | "Summarise the owner's last three TikTok posts." | "Summarise the owner's last three commits." |
| look up | "Look up the Bambu H2C's build volume." | "Look up the commit rule in CLAUDE.md and quote it." |
| search for | "Search for the xTool F2 Ultra's laser power." | "Search for every caller of seat_once in src/ and list them." |
| latest price | "Give the latest price of a 1 kg PLA spool from Bambu." | "Give the latest price row in nirvana/plans/costs.md." |
| reviews of | "Collect reviews of the Prusa Core One." | "Collect the reviews of P37 in the plans folder." |
| podcast | "Summarise this week's Lex Fridman podcast episode." | "Transcribe the podcast at E:/pod.mp3." |
| 's channel | "List the five newest uploads on Veritasium's channel." | "Name the channel the worker sends its rows on in the code." |
| transcript of | "Give the transcript of the Veritasium video on the Monty Hall problem." | "Give the transcript of meeting.m4a." |
| website | "Read the vendor's website for the warranty terms." | "Explain how the website folder's build script bundles its pages." |
| internet | "Find the answer on the internet." | "Explain how the internet-facing port is chosen in the code." |

The same test also pins: an order with a local URL and a web URL names the web one; "two podcasts" holds `podcast`;
a local mark spares only its own sentence ("Look up the H2C's build volume. Write it to notes.md." asks). Words with
no row of their own and no control (`web search`, `web page`, `webpage`, `search the internet`, `find online`,
`search online`, `look online`, `youtu.be`, `instagram`, `reddit`, `wikipedia`, `current price`, `latest episode`,
`latest video`, `transcribe the`, `official website`, `website of`, `on their website`, `on the website`) are
unpinned: dropping one reddens nothing.

Known misses: non-English orders; a lookup sentence that also names a file ("Look up X and save it to notes.md" asks
for no web, though "Search the web and save it to notes.md" does, by `the web`); "file", "folder" and "repo" spare a
lookup sentence that only mentions one. Each such miss is caught by S4 when the worker opens a page, and is the memory
gap when it does not.

### Where the review was short

- S5 did not see that `the web` matched `the webhook` (a substring, not a word): fixed by `has_word`.
- S1's aside, that `with_bladebro_line` has the same limit: true, and not changed. The bladebro line and the launch
  grant both read the run's `--needs`; giving them the booking's changes what a worker is granted, a launch decision
  this order did not ask for. `Seated::needs` stays the run's needs too, so a send-back re-picks with them as before.
- S6 noted the caller's `seat run --effort` patches after the pick as `keep_effort` did. Not changed: the caller names
  its seat and the pick only checks it, so its windows stay the class effort's (the parity gap the review named).
