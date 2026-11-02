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
   holds a URL (`http://`, `https://`, `www.`) or names the web (`the web`, `web search`, `web page`, `webpage`,
   `website`, `youtube`, `internet`), case ignored. `online` and `google` are left out: a research.read order says
   "online docs" or names a Google API as often as it asks for a page.

Why these three: the class is the orchestrator's own statement (and `research.web` is the only class whose `why` names
fetching); a need is what grants the tool; the text catches a `research.read` or `media.*` order that names a page
without a class or need for it. `media.*` alone is not the web: `media.video` and `media.audio` are processing on
Gemini (`nirvana_seats.toml`), usually of a local file.

`seat run` writes `WEB_SOURCES` (the old line, unchanged) for an order that asks, and `NO_WEB_SOURCES` for any other
research or media order: "This order asks for no web page, so the gate wants no source from you: put in the Work
Report's `sources` array (`{url, used_for}`) only a page you did open for the order, and never open one just to have
a source to cite." agy's `SOURCES_LINE` still follows either. The gate reads the decision back from the root order's
copy (`Spec::no_web`, set by a line starting with `NO_WEB_SOURCES`) and the class again (`web_of`): a `research.web`
order always asks; an order with neither line (seated before this package) asks, as every research order did.

`sources_axis`: an order that asks and cites nothing (or delivers no report) fails as before, the reason now naming why
it asks; an order that does not ask and cites nothing gets a note, `no web asked; none cited (...)`. Every source an
order cites is checked as before, web or not: the stream rule, the stable ids, the oEmbed check, the invented-source
step-up are unchanged.

A send-back of an order seated with `NO_WEB_SOURCES`, on any axis, opens with `NO_WEB_REDO`: "This order asks for no
web page: the gate does not want a source from you, and opening a page only to cite it is not the fix." A sources note
is never in a send-back's findings (only failing axes are).

### What a lazy worker can now get away with

- On an order that asks for the web: nothing new. Citing nothing fails; a cited URL the stream did not fetch fails; a
  cited video oEmbed calls missing fails and steps up.
- On an order that does not ask: citing nothing passes with a note. That is the intent. A source it does cite is
  checked, so it cannot cite an unread page.
- The gap: a worker on a no-web order that did use the web and cites nothing passes with a note, and a web question
  mis-classed `research.read` with no URL, no web word and no need is a no-web order. The guard for the first would be
  "the stream fetched pages and the report cites none"; it is not built (no function lists a stream's fetched URLs
  today, and lc3's MCP call would need care). The guard for the second is the orchestrator's class, as ROLE.md says.

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

`nirvana_seat::keep_effort(table, pick, effort)` runs after each pick and before the booking, so the lease, the
`seated` event and the launched model agree. A seat that does not offer the parent's effort keeps the pick's, and the
reason says `<seat> has no <effort>, the parent's effort`; otherwise the reason adds `effort <e> kept from the parent`.
The parent's effort is what it ran at, so a parent the rule stepped down under pressure (§5.5) passes the stepped
effort on; I read that as "the parent's effort" too.

## Tests and controls

Tests (all `gsrc_`): `nirvana_gate::tests::{gsrc_the_web_is_asked_by_the_class_a_need_or_the_order_text,
gsrc_an_order_that_asks_no_web_and_cites_nothing_is_a_note, gsrc_a_send_back_of_a_no_web_order_wants_no_fetch,
gsrc_a_send_back_keeps_the_parents_effort, gsrc_a_stepped_up_redo_keeps_the_parents_effort,
gsrc_a_seat_without_the_parents_effort_keeps_the_picks, gsrc_the_docs_say_sources_only_when_the_order_asks_for_the_web}`,
`nirvana_worker::tests::{gsrc_seat_run_asks_sources_only_of_an_order_that_asks_for_the_web,
gsrc_a_rate_limited_reseat_keeps_the_parents_effort}`. The F2 tests use lc3's and lc4's order texts as written.
Each effort test first asserts the rule alone gives another effort, so it has something to keep.

Controls: `nirvana/controls.d/gsrc.py`, 30, each run alone and red. Two older controls in `run_fixes.py`
(`p24_gate_sources_axis_in_deterministic`, `p24_sources_axis_empty_fails`) had their find text moved by this package;
both re-pointed at the new lines and re-run red.

## Where the order was wrong or unsure

- The order says ROLE.md and the gate SKILL.md "state the sources rule". ROLE.md did not state it (only "Web research
  ... are `research.web`"); a rule line was added under that one. The seat skill (owned by order L) names "herdr's own
  sources line" under the agy brief rules; the hunk is in the report.
- Not run live: no probe, no network (the order's rule). The effect on a real agy redo is a hypothesis until a live
  check seats a no-web research order.
