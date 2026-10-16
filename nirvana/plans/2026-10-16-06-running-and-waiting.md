# Feature 6: one mod square for Running and Waiting on you

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Marge the running and waiting on you into one panel the top being running and what is actively being run with a summary. Then underneath that running is the waiting on you." | a second square in the mod strip; its panel's top part is Running (what the agent is doing now, with a summary), its lower part Waiting on you | orchestrator | rendering tests; a live run |
| L2 | "This is where I want you to take inspiration from pi durable. Look it up it's new . I believe they create the update while the agent is running with the recommended solution and when you return to your oc you can decide after it's built to accept that change and add it. Double check that for me" | the Waiting on you part, shaped after pi durable's handling of work that waits on a person; the claim checked against pi's own sources first | orchestrator | a leaf's primary-source read, quoted here before any design |

Old Nirvana's squares, from the owner's Image #18 (2026-10-16): "Running  Nothing running", "Waiting on you  Nothing
needs you", "Memory  5 changes, 14 recalled", "Last run cost  +7% Claude 5h · out 57k".

## The pi durable claim, checked (leaf "Sonnet - verify pi durable accept-after-build claim", 2026-10-16)

Clone pulled to 4954d76 (pi-durable 1.3.0, 2026-10-15); nothing since 7d75a0e concerns approval, proposals, drafts or
an away mode.
- **Not in pi-durable.** A task waits only on other tasks: "A phase may commit `{ status: "waiting", checkpoint, on,
  policy }` ... resumes at `checkpoint` once every task in `on` is terminal" (docs/spec.md:2231); `on` holds task ids,
  never a person. The only human gate in the repo is a hook that blocks a tool call, `{ block: "Needs approval" }`
  (README.md:139). Cloudflare's adapter docs: "Pi Durable has no approval or permission step for tool calls yet."
  No proposals, drafts, speculative work or accept/apply flow anywhere in pi. No hosted "pi durable platform": Pi 1.0
  and Pi Durable were announced 2026-10-01 as an experimental MIT library.
- **The closest match is a third-party demo,** github.com/disler/durable-pi (not Earendil's; read through a
  summarising fetch): an incident agent builds the fix and a regression test on a branch first, then "waits for you to
  approve the deploy"; the person can approve, decline, or question the agent, and "Approvals and questions are durable
  tasks, so an agent can wait hours for a decision". Build first, then a person gates the deploy.
- **pi-durable pieces that fit the panel:** `pi.live` (running work, nested tool calls with arguments and a summary
  when done), `pi.inbox` (queued input), forks of a conversation (`root.fork`), reset handoff notes as summaries.

## The owner's ruling (2026-10-16)

Asked: "How should Waiting on you behave in this first version?" Answered: "Show and answer (Recommended)": Waiting on
you lists Claude's open question or permission prompt and its recommended option, and the owner answers from the panel.
The build-ahead flow (a second Claude builds the recommended option in a worktree; accept or drop it on return) comes
later as its own feature.

## What Claude Code shows while it waits (measured 2026-10-16, leaf "Sonnet - measure Claude waiting prompts in herdr")

Claude Code 2.1.301 run interactively in a headless herdr session, Haiku, five turns.
- herdr reports the agent `blocked` within 1 to 3 s of a prompt and `working` within 0.4 s of the answer.
- An AskUserQuestion call is written to the transcript only after it is answered; while it waits the transcript
  holds no assistant line for it. A Bash call is on disk while its permission dialog is open, with no result.
- The prompt as drawn: AskUserQuestion shows "❯ 1. Red (Recommended)", descriptions under each option, then
  "3. Type something.", a rule, "4. Chat about this". The Bash dialog shows "Bash command", a "Tip:" line, the
  description, the command between dashed rules, "Do you want to proceed?", then "1. Yes", "2. Yes, and always allow
  access to <folder> from this project", "3. Yes, and switch to auto mode", "4. No". The options change with the case.
- One digit sent with `pane send-keys` picks the option at once, no Enter, for both.
- Claude's terminal title reads "✳ <summary>" while idle or waiting and a spinner glyph while working.

## Shape

- **The square.** "Running" is the strip's first square, before Preload. Its value: "working · Bash" (the tool in
  flight), "working", "waits on you" in yellow, "idle", or "starting".
- **The panel.** A click opens it over the panes, as tall as its content: "Running · <folder>".
  - **Running**, on top, with the state at the right ("working 41s" from the last prompt, "waiting on you", "idle"):
    Task (Claude's own title, its state mark removed), Asked (the last prompt typed), Now (the tool in flight, its
    description or command, and how long it has run; "thinking" between tools; "nothing running" when idle), Said
    (Claude's last text, up to three lines).
  - **Waiting on you**, under it: while herdr says the agent is blocked, the prompt read from the pane's own screen,
    its heading lines and each numbered option, "(Recommended)" in green; else "Nothing needs you".
  - A digit, or a click on an option, sends that digit to the agent's pane; the panel says what was answered. A digit
    that is not an option sends nothing. Esc closes. One panel is open at a time.
- **Sources.** The server's Claude probe already finds the transcript; it now sends its path as the agent token
  `nirvana_transcript` (the wire is unchanged). The window reads the transcript's end itself every 2 s while the strip
  shows the agent, so Running moves without waiting for a new snapshot. The prompt comes from the pane surface the
  window already has.
- **The task mark.** Claude's "✳ " before its title is dropped from every task row, the spaces list's too.

## Files

| File | Change |
|---|---|
| src/nirvana_running.rs (new) | the transcript reader (asked, said, the call in flight), the screen prompt reader, the per-transcript cache, tests |
| src/nirvana_spaces.rs | `TRANSCRIPT_TOKEN`; `AgentModel.transcript`; Claude's state mark dropped from task titles |
| src/main.rs, src/client/mod.rs | the module; a transcript read since the last tick repaints |
| src/client/shell/nirvana_mods.rs | the Running square, the panel, its keys and clicks, one panel at a time |
| src/client/shell/state.rs, composition.rs, input.rs, mouse.rs | the panel's state, drawing, keys, clicks |
| src/client/shell/tests/nirvana_mods.rs | six rendering, key and click tests |
| nirvana/controls.py, nirvana/CHANGELOG.md, nirvana/VERSION, nirvana/README.md | controls, entry, 0.6.0, the feature row |

## Record

Built 2026-10-16 on alpha 0.5.0 (d7fd9cc2), as alpha 0.6.0. The parked 0.2.1 "Reload first" notice stays in `git stash`.

**Departures from the shape, found while building.**
1. *The answer is typed into the pane, not sent as an API method.* The shape said "sends that digit to the agent's
   pane"; the first build sent herdr's `pane.send_keys`, which a window may not send. It is now a pane input event for
   the agent's pane, the path the window uses for every key typed into a pane.
2. *A second "is it waiting" check in the answer path was dropped:* `running_view` reads a prompt only while herdr says
   the agent is blocked, and that one check carries a control.

**Live runs**, scratch home `scratchpad/nh-test7`, session `nirvana-s7`, folder `scratchpad/nirvana-wait-test`, in a
classic console window started minimized and placed behind every other window without activation (`f5/behind.ps1`);
clicks and keys written to the window's console input (`f5/inject.ps1`, fixed: its access mask is a UInt32 and it
writes nothing while attached); captured with `PrintWindow`. `claude --model haiku` in the pane, then the prompt "Use
the AskUserQuestion tool to ask me one question: which colour should the button be, with options Red and Blue, Red
marked recommended. Do nothing else."
1. First build. herdr read the agent `blocked`; a click on cell (39, 1) opened the panel. Leaf "Sonnet - read Running
   panel shots" read `f6/shot-1-idle.png`: strip "✻ Claude   Running idle ▾   Preload 23.4k ▾"; `f6/shot-2-waiting.png`:
   strip "Running waits on you ▴", the panel with Task "Button colour question", Asked (the prompt, cut), Now
   "thinking", "Waiting on you", "☐ Button colour", "Which colour should the button be?", options 1 to 4 with
   "Red (Recommended)" in green, "1-4 answer in the pane · esc close". The key "1" then gave "answered 1: Red
   (Recommended)" in the panel, a toast "Action unavailable", and Claude still waiting (`f6/shot-3-answered.png`):
   the defect in departure 1.
2. After the fix: the same steps; the key "1" in the panel reached Claude. herdr read the agent `idle` 8 s later and the
   transcript recorded "Your questions have been answered: "Which colour should the button be?"="Red (Recommended)"".
   Screenshots `f6/shot-4-waiting.png`, `f6/shot-5-answered.png`. The window was then closed and the session stopped.

**Not measured.** A permission prompt answered from the panel (its options and digit were measured by hand, not through
the panel). A multi-question or multi-select AskUserQuestion. The "Now" row with a tool running live (the run's only
tool was the question, which never reaches the transcript before its answer). An agent in a pane that is not focused.
