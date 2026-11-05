# Feature 39: Nirvana Mobile, the phone drives the live PC session

Status: **P39a BUILT (2026-11-04, alpha 0.55.0)**; P39b-e to build. Was PLANNED (2026-11-04) at master 70c88735 (alpha 0.54.0). It builds the phone half of
`2026-10-22-remote-access-design.md` (parts 1 to 3, build steps 1 to 5). The laptop half (step 6) is its own plan
later. The look and motion are the approved prototype, https://claude.ai/artifact/3RvheUWNNUHWziP8LKZF8M.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "On the next major task can you help me create a mobile app that is able to connect to this app. Build aa ui motion prototype as an artiact or review." | all | builders | §5 owner checks |
| L2 | "This looks good" (2026-11-04, on the prototype) | §2 page | P39b-d | §4 tests, §5 |
| L3 | "how do I expose Nirvana to other devices to controll. Meaning have different computers login to the active pc session working right now. This case I have a laptop linux device I want to have exposed and my phone with the ability to paste screenshots and upload files as well." | §1, §2.3 | P39a, P39b | §4, §5 |
| L4 | "Full Nirvana on phone" | §2 | P39b-d | §4 |
| L5 | "Phone-native page" (chosen over streaming the real Nirvana screen) | §1.4 | P39b | §4 |
| L6 | "Answer prompts, Open a new space, Usage and servers, Alerts when done" | §2.4-2.7 | P39c-e | §4, §5 |
| L7 | "Maybe we implement a full login soloution that List PC to connect to." | §1.2, §1.3 | P39a | §4 |
| L8 | "Tailscale account" (the login) | §1.2 | P39a | §4, §5 |
| L9 | "Home-screen page (Recommended)" (answer, 2026-11-04, to: a page from the PC added to the home screen, or a native app) | §1.4 | P39b | §5 |
| L10 | "Start P39a now (Recommended)" (answer, 2026-11-04) | §3 | P39a | §4 |
| L11 | "Desktop\Pojects only (Recommended)" (answer, 2026-11-04, to: which folders the new-space picker may open) | §2.5 | P39d | §4 |

"Mobile app" in L1 is read as the phone-native page of L5, added to the home screen: no app store, no second
codebase, one install; the owner ruled so (L9). Nothing below blocks a native app later, since it would call the same
API.

## 1. The pieces

1. **Nirvana Remote**, a server inside Nirvana-Herdr, started and stopped with it, one per PC. It serves the page and
   a small JSON API over herdr's own socket API (`agent.list`, `agent.read`, `agent.prompt`, `agent.send_keys`,
   `workspace.list`, `events.subscribe`, all in `src/api/schema.rs`, read 2026-11-04). No HTTP server is in the tree
   today (`Cargo.toml` has tokio and ureq only); P39a picks one that runs on the tokio already there.
2. **Login is the Tailscale account.** Each request is named by Tailscale (device and user); only the owner's
   Tailscale user is let in; anything else is refused and logged. P39a settles the route by a probe and says which:
   (a) `tailscale serve` terminates HTTPS and passes the identity in its `Tailscale-User-Login` header to Nirvana
   Remote on 127.0.0.1; or (b) Nirvana Remote listens on the PC's Tailscale address with a `tailscale cert` and asks
   `tailscale whois` per connection. Route (a) trusts a header any local process could forge, but a local process can
   already drive the herdr socket, so it adds no new door; the record must say this, measured, or pick (b).
3. **PC list.** Nirvana Remote asks its Tailscale for the owner's machines and probes which answer as Nirvana: online,
   online without Nirvana, or asleep with last seen (prototype screen 1).
4. **The page** is plain HTML, CSS and JS embedded in the binary (no build step, no new installs), drawn from the
   prototype: Nirvana Dark, Inter and JetBrains Mono, the same motion and the reduced-motion rule. It must read right
   at 360 to 430 px wide.

Measured 2026-11-04 on this PC: Tailscale 1.102.2 is installed and running, and the tailnet has no HTTPS
certificate domain yet (`CertDomains` empty). Both routes need HTTPS turned on in the Tailscale admin console: an
owner step (§5).

## 2. What the phone does

1. **Spaces** (prototype screen 2): every space with its sparkle mark and its agents, each "working", "needs you",
   "done" or "idle", live from `events.subscribe`, no polling.
2. **Chat** (screen 3): bubbles from the agent's Claude transcript (the file "Open Nirvana here" resumes from), each
   tool call folded to one line that opens on a tap; the live state from `agent.read`.
3. **Send, paste and upload** (screen 5): the text box sends through `agent.prompt`, typed as if on the PC. A pasted
   screenshot, a photo or a file goes over HTTPS to `<space>/.nirvana/uploads/` (git-ignored) and its path is added to
   the message. Over 50 MB is refused before sending.
4. **Prompts** (screen 4): a blocked status event; the menu is read off the screen and shown as buttons, a tap sends
   that choice's key. A fixed Esc button stops the agent. A menu that cannot be read shows the raw screen with 1 to 9,
   Enter and Esc buttons.
5. **New space** (screen 6): a folder picker limited to `Desktop\Pojects` (L11), the same start path
   as "Open Nirvana here".
6. **Cards**: Usage, Servers and Running, from the same data the squares draw, as JSON.
7. **Alerts** (screen 7): on done or needs-you, a Web Push to each phone that turned alerts on. On iPhone it works only
   once the page is on the home screen. Push keys live on the PC, outside the repo.

Failures, from the design doc part 3: Tailscale off or signed out, Nirvana Remote does not start and the Servers
square says why, no fallback to the home network; a device that is not the owner's is refused; sending while the agent
works queues or sends now, the owner's tap; a reload on the PC reconnects the page by itself; PC and phone typing at
once both land, no lock.

## 3. Packages, in order, one release each

| Package | Builds | Owns | Needs |
|---|---|---|---|
| P39a | §1.1-1.3: Nirvana Remote, the login route probe, the PC list, the page shell with screen 1, the Servers square row | new `src/nirvana_remote*`, `Cargo.toml`, the start and stop wiring | none |
| P39b | §2.1-2.3: spaces, chat, send, paste and upload (after this the phone is usable) | the page's spaces and chat, the API's read and send routes, uploads | P39a |
| P39c | §2.4: prompt buttons and Esc | prompt reading, the sheet | P39b |
| P39d | §2.5-2.6: new space, the three cards | folder picker, cards routes | P39b; may build beside P39c, disjoint files |
| P39e | §2.7: alerts | Web Push, keys, the alerts toggle | P39b |

Each package: one Opus high builder, one Sonnet high review, one fix round, then a merge with its own version bump.
P39c and P39d may build in parallel with file ownership split before they start.

## 4. Tests

Against a fake herdr socket API and a fake Tailscale identity, every rule with a sabotage control (CLAUDE.md rule
10): the owner's user let in, another user and no user refused; the PC list states; each route's JSON; a transcript
with tool calls folded; an upload path and the 50 MB refusal; a prompt menu read to buttons and the raw fallback; the
folder picker refusing a path outside the allowed roots; a push sent on done and needs-you and not on working. No
test starts a real Tailscale, herdr server or Claude.

## 5. Owner checks

1. Turn on HTTPS for the tailnet in the Tailscale admin console (once, before P39a's live check).
2. Install Tailscale on the phone and sign in with the same account.
3. The first phone sign-in: the PC list shows the Desktop PC, a tap opens its spaces.
4. After P39b: send a message and paste a screenshot to an agent from the phone.
5. After P39e: add the page to the home screen, turn alerts on, and get the first push.

## 6. Ruled, 2026-11-04

1. The home-screen page, not a native app (L9).
2. The new-space picker opens only folders under `C:\Users\fahim\Desktop\Pojects` (L11).
