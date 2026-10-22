# Remote access: Nirvana from the laptop and the phone (design, stored, not started)

Status: the owner approved parts 1 to 3 in conversation on 2026-10-21 and said "Store the design for now". No ledger
and no plan yet; the ledger comes first when this is picked up (CLAUDE.md rule 1). Plan number 22 is held for the agent
flow, so this takes the next free number.

## The ask, in the owner's words

- "how do I expose Nirvana to other devices to controll. Meaning have different computers login to the active pc
  session working right now. This case I have a laptop linux device I want to have exposed and my phone with the
  ability to paste screenshots and upload files as well."
- Phone scope: "Full Nirvana on phone".
- Phone view: "Phone-native page" (chosen over streaming the real Nirvana screen into the browser).
- Phone extras, all four: "Answer prompts, Open a new space, Usage and servers, Alerts when done".
- Laptop: "Maybe we implement a full login soloution that List PC to connect to."
- Login: "Tailscale account".

## What herdr already has (read 2026-10-21)

- `herdr --remote user@host` attaches a client over SSH, Windows hosts included (`src/remote/attach.rs`,
  `find_installed_remote_herdr`), several clients on one live server.
- A local image pasted into a remote client is bridged to the server (`src/client/clipboard_images.rs`).
- Saved SSH machines and an endpoint list (`src/remote/saved.rs`, `src/client/endpoint/`).
- A phone layout at 44 columns (`src/client/shell/tests/mobile.rs`).
- The socket API covers the phone page: `agent.list`, `agent.read`, `agent.prompt`, `agent.send_keys`,
  `workspace.list`, `events.subscribe`.
- Nirvana Claude already starts with `--rc` (`src/nirvana_launch.rs`), so each one is reachable from the Claude app
  today, one agent at a time.

## Gaps found for the laptop path

1. The owner's PATH holds upstream herdr 0.9.5 (`~/.herdr/packages/standalone/...`) and not Nirvana-Herdr, so a remote
   attach would find upstream herdr or offer to install it.
2. The laptop needs a Linux build of the fork at the same version: the Nirvana sidebar is drawn client side.
3. Only images are bridged; other files need a copy into the space's folder plus the path typed to the agent.

## Part 1: the pieces

- **Nirvana Remote**, a new server in the fork, started and stopped with Nirvana-Herdr. It listens on the PC's
  Tailscale address only, serves the phone page and a small API over herdr's socket API, and uses `tailscale cert` for
  HTTPS (`https://<pc>.<tailnet>.ts.net`), which phones need for push and clipboard paste.
- **Login is the Tailscale account.** Each request is named by Tailscale's whois (device and user). Only the owner's
  Tailscale user is accepted. A lost device is removed in Tailscale.
- **PC list.** Each Nirvana Remote asks its Tailscale for the owner's machines and probes which answer as Nirvana. The
  phone's first screen lists them; the laptop gets a new `herdr` start screen listing the same PCs, and a pick opens
  the real Nirvana window over SSH (Windows OpenSSH Server; Tailscale's SSH server is believed not to run on Windows,
  unverified).
- **Phone page:** agent chat, prompt buttons and Esc, open a new space, Usage/Servers/Running cards.
- **Alerts:** Web Push. On iPhone it works only once the page is added to the home screen.

## Part 2: how things travel

1. **Reading an agent:** chat bubbles from the agent's Claude transcript (the file "Open Nirvana here" resumes from),
   tool calls folded to one line; live state from `agent.read`; refresh on `events.subscribe`, no polling.
2. **Sending:** the phone's own text box; `agent.prompt` types it as if typed on the PC.
3. **Screenshots and files:** pasted or picked, sent over HTTPS, saved to `<space>/.nirvana/uploads/` (git-ignored),
   the path added to the message. Claude reads images, PDFs and logs from a path.
4. **Prompts:** a blocked status event; the menu is read off the screen and shown as buttons; a tap sends that choice's
   key. A fixed Esc button interrupts.
5. **New space:** a folder picker limited to allowed roots (such as `Desktop\Pojects`); the same start path as
   "Open Nirvana here".
6. **Cards:** the data the squares draw from, as JSON.
7. **Alerts:** on done or needs-you, a Web Push to each phone that turned alerts on. Push keys live on the PC, outside
   the repo.

## Part 3: failures, tests, build order

Failures:

- Tailscale off or signed out: Nirvana Remote does not start; the Servers square says why. No fallback to the home
  network.
- A device that is not the owner's: whois names another user or none; refused and logged.
- PC asleep: greyed in the list with last seen.
- Sending while the agent works: send now or queue until done.
- Unreadable prompt menu: raw screen with 1 to 9, Enter, Esc buttons.
- Upload over 50 MB: refused before sending.
- Reload on the PC: the page reconnects by itself.
- PC and phone typing at once: both land in the pane, no locking.

Tests: against a fake socket API and a fake whois, each rule with a sabotage control. Two checks are the owner's: the
first phone sign-in and the first push.

Build order, one release each, built on agy:

1. Nirvana Remote, Tailscale login, PC list.
2. Phone chat, Send, paste and upload. (After 1 and 2 the phone is usable.)
3. Prompt buttons and Esc.
4. New space, Usage/Servers/Running cards.
5. Alerts.
6. Laptop: start screen with the PC list, Linux build, the right herdr answering, file transfer.
