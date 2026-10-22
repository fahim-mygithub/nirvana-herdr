---
name: gate
description: Use when a worker's order is delivered, sent back, waiting on a review or a held-out check, or ready to merge. herdr's completion gate - its axes, the gate lines an order must carry, the send-back loop, the reviewer and the held-out check, and when to merge. Also before writing any build order, for the lines the gate will hold it to.
---

# The completion gate

`herdr` below is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`; the session digest names it too.
A `herdr` found on PATH may be upstream herdr, which has none of these commands.

herdr's gate rules on a delivered order with no model call (plan 12 §7.3). You start it, read its ruling and do
what it names next. You never pass an order by hand: `herdr run log` with a `gated` event skips every axis.

## Write the gate lines into every order first

Each at the start of a line, anywhere in the order file. The gate reads them from the order the chain started with,
so a send-back keeps them.

```text
owns: src/calc.py tests/test_calc.py     every file the worker may change; `*` within a name, `**` across folders
test: python -m pytest -q -rA tests      one command, no shell, run in the worker's folder
check: test_mul test_mul_negative        names the test output must print; pytest needs -rA to print passing names
serves: 8770                             ops.terminal orders only: the port the worker's own pane must listen on
```

- A build or debug order with no `test:` and `check:` lines passes the tests axis on nothing. Name the check that
  proves the order, not the suite: a suite total cannot see a check that never ran.
- An order of class `build.*`, `fix.*` or `plan` must carry an `owns:` line naming the files it may change; `herdr seat run` refuses with exit 2 without one, and the `owned` axis fails on empty `owns:`. Read-only classes keep the Note.
- Research and media orders are gated like builds: their Work Report must carry a `sources` array (`url`, `used_for`), verified by the `sources` axis against the worker stream.
- A server started from a worker's own shell dies with the worker's job object, so an ops order must say how the
  server outlives it. The way this gate was built for, not yet measured live: the order tells the worker to run
  `herdr pane run "$HERDR_PANE_ID" "<server command>"`, which types the command into its own pane's shell to run
  once the worker ends.

## The axes, in order

| Axis | Passes when | A failure sends back with |
|---|---|---|
| rebase | the branch rebases onto `nirvana-integrate/<run>`; the worker's uncommitted work is committed first | the conflicting files |
| tests | `test:` exits 0 and every `check:` name is in the filtered output | the filtered lines |
| report | the Work Report is well formed and its status is `done` | why |
| owned | every changed file matches an `owns:` pattern (herdr's `.nirvana/` never counts) | the unowned files |
| sources | research and media orders: cited URLs appear in the worker's stream as fetch, read or browser calls | uncited or unverified URLs |
| served | ops orders: each `serves:` port is listened on in the worker's own pane | what it does listen on |
| heldout | feature-size orders (a build or debug fix, size medium or large): the held-out check passes at the head | the failing case ids only |
| review | load-bearing orders (plan, build.load_bearing): a reviewer of another family approves | the reviewer's gotchas |

A `note` is said and never blocks. A `wait` means a review or a held-out check is still to come: exit 4.

## The commands

- `herdr gate run <order>`: exit 0 pass, 1 sent back or asked, 4 waiting. It prints each axis and a `next:` line.
- `herdr gate merge <order>`: merges a passed order into `nirvana-integrate/<run>` with no checkout, then runs
  `herdr seat cleanup` on it and the send-backs before it. It refuses when the branch or the integration branch
  moved since the gate: gate it again. A merge to `main` is the owner's, never yours, except in auto mode within
  its budgets.
- `herdr gate status [<order>]`: the verdict, the send-backs used, the reviews and the held-out check of each order.
- `herdr run end` refuses while an order is delivered and not gated. `herdr run end --abandon <order> --why <text>` records an `abandoned` verdict so the run can end.

## The send-back loop

A failing axis seats a child `<root>-s<n>` in the same worktree with the reasons, on the seat the rule picks, and
starts it. At most 3 per order. The fourth failure asks the owner: the order is `asked`, and you answer the owner,
not the worker. Do not seat the order again by hand to dodge the cap.

When the child delivers, `herdr gate run <child>`. Its branch holds the root's work and its own.

## The reviewer (load-bearing orders)

When the deterministic axes pass, `gate run` books a reviewer of another family (`review.adversary --not-family
<builder's family>`; decision 0017: only when no other family has a seat, the builder's own family, marked
`SAME-FAMILY REVIEW`) and prints the `herdr seat run` line. Start it as printed. When it delivers, `herdr gate run
<order>-rev<n>` or `herdr gate run <order>`: `done` approves, `blocked` sends back with its gotchas, `needs_answer`
waits for the owner.

## The held-out check (feature-size orders)

Before the builder starts (decision 0019):

1. Book the builder: `herdr seat pick --class <c> --size <s> --book <order> --run <run>`.
2. `herdr gate author <order> --order <order file> --run <run>` books an author of another family and prints its
   `herdr seat run ... --worktree` line. Start it.
3. When the author delivers: `herdr gate run <order>-acc1`. The gate takes the check out of the author's folder,
   keeps it beside the journal, and runs it at the base: valid when a case fails there. Invalid once, it books a
   second author with the reason; invalid twice, the build goes on without one, and every gate says so.
4. Then start the builder with `herdr seat run`. It never sees the check; a failing case reaches it as its id.

## Lessons

A Work Report's `lesson` is written to `.nirvana/lessons/<slug>.md` only when the order passes, its evidence quote
is in the worker's own stream, and the repository has a GitHub remote. The `lesson` axis says which. The next order
of that class gets one line naming it. `herdr lessons for --class <c>` lists them.
