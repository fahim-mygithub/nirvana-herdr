# Plan 12 O4a record: Nirvana Claude's launch, the "new" menu, "Open Nirvana", the icon, badge and toast

Builder "Opus 5.5 high - O4a verb, icon, launch build", 2026-10-17, branch `o4a-launch` from `o2-seats` ba32830e.
O4b (the plugin's skills, hooks, worker agents and statusline) comes after this and is not here.

## What was built

| Part | Where |
|---|---|
| The launch argv and env, the menu's kinds, the folder's hash, id and letter, `herdr nirvana window <folder>` | `src/nirvana_launch.rs` |
| The plugin's minimal tree: manifest, main-thread agent, settings (theme, `NIRVANA_ORCHESTRATOR`) | `nirvana/claude/` |
| The server half: `command.invoke nirvana.new:<kind>` (the menu) and `nirvana.open:[fresh:]<folder>` (the verb) | `src/app/nirvana_new.rs`, a hook in `src/app/custom_commands.rs` |
| The "new" menu: Nirvana, Claude Code, Antigravity, Shell | `src/client/shell/nirvana_new_menu.rs`, hooks in `state.rs`, `context_menu.rs`, `mouse.rs` |
| The window's taskbar identity, the badge, the toast | `src/nirvana_taskbar.rs`; `src/platform/windows/notifications.rs` |
| `herdr.exe`'s icon resource | `build.rs`, `tests/nirvana_icon.rs` |
| install's Windows side: plugin, mark, themes, profile, font, toast id, verb, Nirvana-C's verb removed | `nirvana/windows_shell.py`, `nirvana/install.py`, `nirvana/themes/`, `nirvana/terminal/schemes.json` |
| Controls | `nirvana/controls.d/o4a.py` (49) |

## Registry keys install.py writes and deletes (all HKCU)

Writes:

- `Software\Classes\Directory\shell\NirvanaHerdr`: `MUIVerb = Open Nirvana`, `Icon = <home>\nirvana\assets\nirvana.ico`
- `Software\Classes\Directory\shell\NirvanaHerdr\command`: `(Default) = "<LOCALAPPDATA>\Microsoft\WindowsApps\wt.exe" -w new -p "Nirvana" -d "%1" "<home>\bin\herdr.exe" --session nirvana nirvana window "%1"`
- `Software\Classes\Directory\Background\shell\NirvanaHerdr` and `...\command`: the same with `%V`
- `Software\Classes\AppUserModelId\Nirvana.Herdr`: `DisplayName = Nirvana`, `IconUri = <home>\nirvana\assets\nirvana-256.png`
- only when no JetBrains Mono face is registered: `Software\Microsoft\Windows NT\CurrentVersion\Fonts`, six
  `JetBrains Mono <face> (TrueType)` values (not written on the owner's PC, where all six are registered)

Deletes (whole trees): `Software\Classes\Directory\shell\NirvanaC`, `Software\Classes\Directory\Background\shell\NirvanaC`.

Leaves alone: `Software\Classes\Directory\shell\Nirvana` (old Nirvana's "Open Nirvana here").

At run time herdr's toast code also writes, as upstream does: `Software\Classes\AppUserModelId\Nirvana.Herdr`
(`DisplayName`, `CustomActivator`, `IconUri` when the mark is installed) and `Software\Classes\herdr-notification-<16
hex of the exe path>` (`URL Protocol`, `shell\open\command`).

Files: `<home>\nirvana\claude\` (replaced whole), `<home>\nirvana\assets\` (five files),
`<claude config>\themes\nirvana-light.json` and `nirvana-dark.json`,
`%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\Nirvana-Herdr\nirvana.json` written,
`...\Fragments\Nirvana-C\nirvana.json` deleted.

## Measured

- `claude --help` (2.1.302): `--plugin-dir`, `--agent`, `-n/--name`, `--settings` exist as the plan writes them.
- `/context` of Nirvana Claude: 0 turns, $0; `Model: claude-opus-5-5`; `nirvana:nirvana | Plugin | 93` in Custom
  Agents; System prompt 1.5k. Plain Claude: 0 turns, $0, no nirvana row, System prompt 2.5k.
- `settings.json`'s `env` reaches the Bash tool: `printenv NIRVANA_ORCHESTRATOR` printed `1` (haiku low, $0.005).
- `conhost.exe --headless -- cmd /c ping -n 3 127.0.0.1`: exit 0, 0 visible windows.
- `RelaunchCommand`: at most 255 characters (255 set; 256-259 E_INVALIDARG; 260+ ERROR_INSUFFICIENT_BUFFER).
- The built `herdr.exe`: 1 icon group, RT_ICON sizes 554, 1071, 1318, 2158, 3079, 19691 = the `.ico`'s six images.
- Live run 1: the button did not split (RelaunchCommand 487 characters refused; the icon property alone took). Fixed.
- Live run 2: id `Nirvana.Herdr.066791b9d817e117`, icon `<debug herdr.exe>,0`, name `Nirvana`; taskbar "Nirvana - 1
  running window", 383 teal and 189 saffron pixels; Terminal's button 0 and 0. Toast "Nirvana · Pojects" with
  `appLogoOverride` in the history of `Nirvana.Herdr`. The pane ran `claude --plugin-dir ... --agent nirvana --name
  Nirvana --settings ...` and stopped at Claude Code's folder-trust question (not answered). After the server
  stopped: the window's four properties empty, the Nirvana button gone, Terminal's button back to 38 saffron pixels,
  all in its bottom underline (rows 40-42 of 48), as before the run: nothing stale on Terminal's group.

## Choices where the plan was silent

1. **The verb runs Windows Terminal directly** (`wt -w new -p Nirvana ... herdr nirvana window`), no Python
   launcher: no console flash, no interpreter path in the registry, errors print in the window. `-w new` because the
   taskbar id is per window.
2. **Which windows are Nirvana's:** the one `herdr nirvana window` runs in (the verb, and a pinned button), and one
   whose client picked Nirvana in the "new" menu, unless it already is one. The verb's window sets
   `NIRVANA_WINDOW_FOLDER` for the client so the menu does not claim it again. The supervisor process sets and
   clears, so a reload keeps the button and the toast shows once.
3. **The session:** `nirvana`, as Nirvana-C's verb used.
4. **A fresh server's seeded shell space takes Nirvana Claude** when it is the server's only space, one pane, no agent,
   on the folder; otherwise a new space opens. Its `NIRVANA_ORCHESTRATOR` then comes from `settings.json`'s `env`,
   which is why the settings carry it as well as the pane env.
5. **The second click** focuses the space whose pane has a herdr agent named `nirvana` or `nirvana-<n>`, live or
   starting, on the same folder (case and separators ignored).
6. **The menu has a fourth entry, Shell**, what "new" did before, so nothing is lost.
7. **The theme is `custom:nirvana-dark`**: one settings file carries one theme, and Windows Terminal's default is dark
   (Nirvana-C chose at run time between two).
8. **The themes stay user themes** in `<claude config>\themes`, byte for byte Nirvana-C's: a plugin `themes/` folder
   exists, but which `theme` value names a plugin theme was not measured.
9. **The toast id is one for the fork, `Nirvana.Herdr`**; the taskbar ids are per folder. Every herdr toast now uses
   it, so the header reads Nirvana.
10. **The letter:** the first letter or digit (Unicode), upper-cased when that is one character; `N` when there is
    none. Non-Latin letters are drawn by GDI's font linking (not checked on screen).
11. **The badge's size** is the small-icon metric (16 px at 100%), the toast's picture 96 px.
12. **No build dependency for the icon:** build.rs writes the `.res`; `rustc-link-arg-bins`, MSVC only.
13. **The relaunch line** uses `wt.exe` by alias and drops `-d`, to fit 255 characters; past that it opens the profile.
14. **The plugin manifest has no `version`** (a validate warning): the fork's number lives in `nirvana/VERSION`, and a
    second copy would rot.

## Controls

49 in `nirvana/controls.d/o4a.py`, each run alone with `python nirvana/controls.py --only <name>`; results in the
package report.
