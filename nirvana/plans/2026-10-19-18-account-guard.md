# Feature 18: the Antigravity account stays the one the owner picked

Status: **BUILT, alpha 0.29.0 (2026-10-19).** Built on agy (Gemini 3.8 Flash high); record `2026-10-19-18-records/p18.md`.

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "Ive noticed by default on every start up Gemini keeps connecting to my fahim.private account. When I change the accoutn e nsure it stays to the new account." | §2 the chosen account, §3 the drift warning | orchestrator | unit tests on recorded logs; client tests on the row and the foot mark |
| L2 | The question was "The Antigravity desktop app is signed in as fahim.private, and has been running since Oct 17 11:58. It shares one sign-in slot with agy, the one in Windows Credential Manager. Each time either one refreshes its sign-in, roughly hourly, it writes its own account into that slot. So which account should the desktop app be on?" The owner answered "Same new account (Recommended)". That option's text was: "You sign the desktop app into the new account once, and both stay there. I add a guard to herdr: it remembers the account you picked in Accounts, and the Accounts row warns you if agy or the desktop app ever shows a different one." | §2, §3; the owner signs the desktop app in, herdr never does | orchestrator | as L1 |

## 1. The cause, measured 2026-10-19

- **One slot, two writers.** agy keeps its sign-in in the Windows Credential Manager entry
  `LegacyGeneric:target=gemini:antigravity`, which `cmdkey /list` names. The Antigravity desktop app (2.0.6, running
  since 2026-10-17 11:58) is signed in as fahim.private: its `state.vscdb` key `antigravityAuthStatus` names that email.
- **The flips are hourly.** Every agy start writes `OAuth: authenticated successfully as <email>` to
  `~/.gemini/antigravity-cli/log/cli-*.log`. Over all 2,485 logs the account changed 21 times, every change after
  2026-10-17 12:37. On 10-17, the switches to personal.fahim.amin landed near :20 past the hour and the switches back
  came 5 to 50 minutes later. That is two holders of two accounts, each writing its refreshed sign-in into the one slot.
  The last flip was 2026-10-19 00:45:18, during herdr's own `/usage` check, which signed in silently with what the slot
  held.
- **herdr writes no credential.** No `CredWrite`, `cmdkey` or Gemini credential file appears in the fork's source.
- **The fix is the owner's sign-in** of the desktop app to the same account (L2). herdr cannot sign anything in. What
  herdr can do is remember the pick and say so the moment the slot drifts.

## 2. The chosen account

- `herdr account login antigravity` records the account it ends on, when it ends signed in, in
  `<home>/nirvana/accounts.json` as `{"antigravity": "<email>"}`. Only that command writes the file. Claude Code and
  Codex keep one account per CLI and are not tracked.
- **No credential is read.** The account comes from agy's own log line, as today. The desktop app's `state.vscdb`
  holds an API key next to the email, so herdr never opens it.

## 3. The drift warning

- **A slow check** runs off the draw path every 60 s, whether or not a panel is open. It reads the newest agy log that
  names a sign-in, and whether an `Antigravity.exe` process runs. The process walk is the existing Toolhelp walk.
- **When the account differs from the chosen one:**
  - **The Accounts row** reads `fahim.private@gmail.com ⚠ chosen personal.fahim.amin@gmail.com`, with the warning colour
    the panel already uses, and its button stays `switch`.
  - **A line under the row** says why:
    - while `Antigravity.exe` runs: `The Antigravity app put this account back. Sign the app in as <chosen> too.`
    - otherwise: `agy went back to this account. Press switch to sign in again.`
  - **The foot's `accounts` label** gains a `⚠` in the same colour, so the drift shows without opening the panel.
- **When the account matches,** or no account was ever chosen, nothing changes from today.
- **Narrow,** the row drops the word `chosen` and then the chosen email; the `⚠` stays.

## 4. Verification

- **Unit tests:**
  - the record written after a signed-in login, and not after one that ends signed out;
  - the drift ruling on recorded log folders: matching, different, no choice, no log;
  - the cause line with and without the app process, both given by the test.
- **Client tests:** the row, the cause line, the foot `⚠`, narrow and wide.
- **No test reads** the real `~/.gemini`, `%APPDATA%\Antigravity` or Credential Manager, or runs agy.
- **Every assertion gets a control** in `nirvana/controls.d/p18.py`.

## 5. Not in this version

- No automatic re-sign-in: that needs the owner in a browser.
- No separate credential slot for herdr's agy. Proving one needs a full sign-in.
