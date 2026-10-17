# P7: Nirvana's own taskbar icon, a folder-letter badge, and a toast (2026-10-17)

Leaf "Opus - P7 taskbar icon probe", Windows 11 build 26200, a throwaway C# tool (ITaskbarList3, WM_SETICON,
SHGetPropertyStoreForWindow, IShellLinkW) and WinRT toasts from PowerShell 5.1. Screenshots stayed in the probe's
scratch folder. Everything it created was removed and checked; the owner's windows and herdr were not touched.

## Results

- **Windows Terminal can carry it.** Setting the window's `PKEY_AppUserModel_ID` with `RelaunchIconResource`,
  `RelaunchCommand` and `RelaunchDisplayNameResource` in one commit splits that Terminal window into its own taskbar
  button with the Nirvana mark. It works from another process, and Terminal left the property alone for 20 minutes.
  WM_SETICON alone does nothing: Terminal groups under its packaged id.
- **The badge** is `ITaskbarList3::SetOverlayIcon`; it works across processes and stays after the setter exits. Windows
  draws it at the button's **top-right**, not bottom-right.
- **Per-folder badges need per-folder ids.** Two windows with one id combine into one button showing only the last
  badge; distinct ids give two buttons, each with its badge.
- **Order matters.** A badge set before the id left a stale copy on Terminal's own group. Set the id first, then the
  badge; on exit clear the badge, then the properties. Done that way, nothing stale was left.
- **The icon must be a valid resource** at the time the id is set: an invalid one gives a blank page icon that is not
  re-read under the same id. An icon embedded in `herdr.exe` (`<herdr.exe>,0`) is the stable choice.
- **From inside the console**, where herdr's client runs, `GetConsoleWindow` returns Terminal's pseudo console window;
  its root owner is the Terminal window, and badging it worked. herdr already makes this call in
  `src/notifications.rs:208`.
- **Toast** "Nirvana · Pojects" with the letter as `appLogoOverride` showed in all three routes tried. A registry id
  `HKCU\Software\Classes\AppUserModelId\<id>` with `DisplayName` and `IconUri` gives the Nirvana mark in the header;
  no Start-menu shortcut is needed.
- **A console window from a shortcut** also gets its own button and badge, and herdr runs in it with colour and mouse,
  but its font has no fallback: the right column and some glyphs draw as boxes. It is the fallback only.

## A finding about herdr itself

`HKCU\...\AppUserModelId\Herdr.Desktop` does not exist on this PC and the install has no
`bin\conpty\x64\OpenConsole.exe`; `notifications.rs:161-167` returns NotFound without it, so herdr's own Windows toasts
most likely never fire on the fork's install. Read from the code, not run.

## What the owner saw during the probe

Probe windows opened and one took the foreground, and Terminal's group showed a stale badge for about 25 minutes until
the cleanup. Not measured: never-combine taskbar mode, several monitors, pinning, a tab dragged between Terminal
windows, toast clicks, Focus Assist.
