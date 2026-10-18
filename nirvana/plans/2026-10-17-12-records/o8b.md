# Package O8b record: Jev's roles 2 and 3, in shadow

Built 2026-10-18 on branch `o8b-jev` (base `b675a9b3`, alpha 0.18.0) by Sonnet 5.5 (high). Files: see the report; the
new ones are `src/nirvana_jev_roles.rs`, `src/nirvana_jev_roles/tests.rs` and `nirvana/controls.d/o8b.py`.

## 1. Ledger rows served

| # | Verbatim quote | Surface | Verifier |
|---|---|---|---|
| L10 | "Cosnider Jev and its capbilities in this process as well." | `herdr jev roles\|agree\|watch`, `seat pick`, `run tick` | `nirvana_jev_roles::tests::*`, controls `jevr_*`, the live lines in §4 |

## 2. What was built

- **`src/nirvana_jev_roles.rs`** (new): the setting (`jev.toml`), the call log (`jev-calls.jsonl`), `advise_seating`
  (role 3), `watch_tick`/`due`/`judge`/`read_watch` (role 2), `agreement_in` and the three CLI verbs.
- **`src/nirvana_seat.rs`**: `Pick` gains `contenders` (the seats that cleared the bar, the pick first; empty with one
  seat or on an override) and `jev_pick`, both skipped when empty, and a `Contender` struct. `pick_cli` calls
  `advise_seating` after the rule picked.
- **`src/nirvana_run.rs`**: `Seated.jev_pick` (optional, skipped when none), `Kind::JevWatch`, its arm in `order()`,
  `replay` and `queue_line` (a yes with `acted` true is one queue line), and the tick calls `watch_run` before the wake.
- **`src/nirvana_worker.rs`**: `Booked.jev_pick`, `book()` records the advice (never acts), `herdr_text` is `pub(crate)`.
- **`src/nirvana_jev.rs`**: `live_client()`, and `roles|agree|watch` routed to the new module.
- The `Seated` struct literals in `nirvana_gate.rs`, `nirvana_worker.rs` and four test files gain `jev_pick: None`.

## 3. Choices where the plan was silent

1. **The setting file is `jev.toml`**, not a table in `seats.toml`: `seats.toml` refuses unknown keys and O8a writes the
   mode there. A word it does not understand is `off`; a missing file or key is `shadow`; a broken file is `shadow`.
2. **`on` is an override through `pick`.** It passes the filters, the reserve included, and counts against the run's 3
   overrides. A caller that named its seat (`seat run`) gets the advice recorded and never acted on.
3. **One call for role 2 carries two questions** (a yes/no and a kind) so a yes names stuck, looping or waiting.
   A kind Jev is sure is plain work turns the yes into unsure.
4. **The 5-minute throttle reads the journal** (the last `jev_watch` per order), so no second state file exists, and a
   failed look is also an event, which keeps a dead key from calling every tick.
5. **No key means nothing happens**: no call, no event, no log line. The failure path is for a key that fails.
6. **The tail is the last 60 lines, at most 24 KB, redacted** by `nirvana_jev::redact` (Jev's own secret shapes).
7. **A worker's 10 minutes run from its last `started`**, not from `seated`.
8. **Role 3 asks nothing about the order's text**: `pick` never sees it. Jev weighs the class's one-line description and
   each seat's facts (model, family, pool, capabilities, class score, pressure).

## 4. Live (isolated server `o8blive`, scratch `NIRVANA_HERDR_HOME`, `BROWSER` a no-op script; 3 Jev calls, stopped by `server stop`)

```text
$ herdr seat pick --class plan --json     (filtered)
opus high contenders ['opus', 'g-opus']
reason: plan → opus high: max 5h 48% used, week 60% used, reserve 15%; g-opus scores the same (google3p 5h 0% used, week 4% used); Jev (shadow) unsure (opus 0.51)
$ herdr run tick r1                       (shadow; the pane printed 14 identical failing-test lines)
r1: a: Jev (shadow) reads its screen: yes (stuck) 0.93
r1: nothing waits
$ herdr jev roles watch on; (the jev_watch line removed); herdr run tick r1
r1: a: Jev (on) reads its screen: yes (stuck) 0.93
r1: 1 event(s) queued: the run has no orchestrator pane
$ herdr run notice r1
- a: Jev reads its screen as stuck (0.93); look at its pane
$ herdr jev agree
seating: 1 answered, no choice yet (0 agreed, 0 differed, 1 declined: none or unsure), 0 failed
watch: 2 answered, 2 yes, 0 failed
cost: $0.0001
```

The call log lines, no key: `{"answer":"unsure","cost_usd":0.000033558,"ms":204,"offered":["opus","g-opus"],"prompt":"352bdcd5b4d8c4ca","role":"seating","rule":"opus","score":0.51}`,
`{"answer":"yes","cost_usd":0.000048384,"ms":163,"order":"a","prompt":"878a693667983f65","role":"watch","score":0.93}` and a second
watch line for the `on` tick.

## 5. Not done

- No live `on` wake into a real Nirvana Claude pane: the run had no orchestrator pane, so the wake queued ("no
  orchestrator pane"). The wake decision on an `on` queue is covered by `on_queues_a_yes_for_the_wake...` with the guard
  and the owner-typing rule, not live.
- No live seating `on`, so no override by Jev was seen outside the unit test.
- The wiring in `pick_cli` and `book()` (setting read, client made) has no unit test: both are CLI bodies. The live
  `seat pick` is its check.
