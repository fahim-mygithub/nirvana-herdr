"""What `install.py` puts outside the binary for Nirvana Claude (plan 12 O4a): the plugin, the mark, the Claude Code
themes, the Windows Terminal profile, JetBrains Mono, the Explorer verb "Open Nirvana", and the toast's app id.

The owner, 2026-10-17 (plan 12 R8): "Lets do the full port were at a stage now I think the herder varient is the
perfered method. Remove Nirvana - CC have standard Nirvana use the herder varient." So the verb moves here from
Nirvana-C, and what that verb leaned on in Nirvana-C moves with it: the "Nirvana" Windows Terminal profile and its
two schemes (a fragment), the two Claude Code custom themes `--settings` names, and JetBrains Mono. Nirvana-C's
verb keys and its fragment are removed; the Nirvana-C repository itself is not touched.

Every writer is pure over a root, the shape Nirvana-C's `verb.py` proved: the registry writers take a registry
object and a base key, so a test passes `FakeRegistry` or a scratch key `Software\\NirvanaHerdrTest-<pid>` that it
deletes, and never the real `Software\\Classes`; the file writers take their folders. Registry: HKCU only, `winreg`
from the stdlib, never `reg.exe` (a console child that flashes) or a `.reg` file.

THE VERB KEY IS `NirvanaHerdr`. `Directory\\shell\\Nirvana` already holds old Nirvana's "Open Nirvana here" on the
owner's machine (read 2026-10-17) and `NirvanaC` holds Nirvana-C's; a second writer at either would overwrite it.
Both are retired after the new verb is written: Nirvana-C's outright, old Nirvana's after a JSON backup and only
when its command names old Nirvana's launcher (`retire_old_nirvana_verb`).

The verb runs nirvana-open.exe directly, so there is no console flash:
`"<home>\\bin\\nirvana-open.exe" "%1"` (and `"%V"` for the background verb). `-w new` keeps the folder's own
window (the taskbar id is per window), and `herdr nirvana window` starts the server, Nirvana Claude, the button,
the badge and the toast (src/nirvana_launch.rs). Limits: a folder whose name has a `;` is split by Windows Terminal,
which reads `;` as its own command separator; a drive root arrives as `C:"` and herdr puts the `\\` back.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
import shutil
import subprocess
import urllib.request
import zipfile
from pathlib import Path

try:
    import winreg
except ImportError:   # not Windows: the module imports, the real registry refuses
    winreg = None  # type: ignore[assignment]

ROOT = Path(__file__).resolve().parents[1]
CLASSES = r"Software\Classes"
VERB_KEY = "NirvanaHerdr"
VERB_TEXT = "Open Nirvana"
# (subkey under the base, the token Explorer substitutes): `%1` is the right-clicked folder, `%V` the folder whose
# empty space was right-clicked.
VERBS: tuple[tuple[str, str], ...] = (
    (rf"Directory\shell\{VERB_KEY}", "%1"),
    (rf"Directory\Background\shell\{VERB_KEY}", "%V"),
)
# Nirvana-C's "Open Nirvana - CC" (its nirvana_c/verb.py VERBS), removed by every install (R8).
OLD_VERBS: tuple[str, ...] = (r"Directory\shell\NirvanaC", r"Directory\Background\shell\NirvanaC")
# Old Nirvana's "Open Nirvana here" (its `~/.nirvana/open-here.pyw`), retired with a backup (R8).
OLD_NIRVANA_VERBS: tuple[str, ...] = (r"Directory\shell\Nirvana", r"Directory\Background\shell\Nirvana")
OLD_NIRVANA_LAUNCHER = "open-here.pyw"
# The fork's toast id; src/nirvana_launch.rs TOAST_APP_ID and the toast code write the same values.
TOAST_APP_ID = "Nirvana.Herdr"
TOAST_KEY = rf"AppUserModelId\{TOAST_APP_ID}"
DISPLAY_NAME = "Nirvana"
PROFILE_NAME = "Nirvana"
SESSION = "nirvana"
FRAGMENT_APP = "Nirvana-Herdr"
OLD_FRAGMENT_APP = "Nirvana-C"
FRAGMENT_FILE = "nirvana.json"
FONT_SIZE = 11
PADDING = "12, 8, 12, 8"
FONT_FACE = "JetBrains Mono, Cascadia Mono"
THEMES = ("nirvana-light.json", "nirvana-dark.json")
# The mark files a running herdr and the registry point at.
ASSETS = ("nirvana.ico", "nirvana-256.png", "nirvana.svg", "nirvana-small.svg", "nirvana-tiny.svg")


# ---- paths ----

def wt_path(env: dict | None = None) -> Path:
    """Windows Terminal's app-execution alias, a real path Explorer can start (the Store package's own folder under
    Program Files\\WindowsApps is not readable by a user process)."""
    env = os.environ if env is None else env
    base = env.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Microsoft" / "WindowsApps" / "wt.exe"


def fragments_root(env: dict | None = None) -> Path:
    env = os.environ if env is None else env
    base = env.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Microsoft" / "Windows Terminal" / "Fragments"


def claude_config_dir(env: dict | None = None) -> Path:
    """`CLAUDE_CONFIG_DIR`, else `~/.claude` (Claude Code's own rule, and src/nirvana.rs's)."""
    env = os.environ if env is None else env
    if env.get("CLAUDE_CONFIG_DIR"):
        return Path(env["CLAUDE_CONFIG_DIR"])
    return Path(env.get("USERPROFILE") or str(Path.home())) / ".claude"


def herdr_exe(home: Path) -> Path:
    return Path(home) / "bin" / "herdr.exe"


def nirvana_open_exe(home: Path) -> Path:
    return Path(home) / "bin" / "nirvana-open.exe"


def icon_path(home: Path) -> Path:
    return Path(home) / "nirvana" / "assets" / "nirvana.ico"


def toast_icon_path(home: Path) -> Path:
    return Path(home) / "nirvana" / "assets" / "nirvana-256.png"


# ---- files ----

def _replace_tree(src: Path, dst: Path) -> None:
    """`dst` becomes a copy of `src`: a file removed from the tree is removed at the install too."""
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(src, dst)


def statusline(home: Path) -> dict:
    """Plan 12 O4b: the statusline `herdr nirvana statusline` draws. A plugin's own settings take only `agent` and
    `subagentStatusLine` (Claude Code's plugins reference), and a `statusLine` command gets no `${CLAUDE_PLUGIN_ROOT}`,
    so the installed copy of the `--settings` file names the installed binary itself. Forward slashes and the quotes
    keep it one word in Git Bash, where Claude Code runs it; a `herdr` on PATH may be upstream herdr."""
    exe = (Path(home) / "bin" / "herdr.exe").as_posix()
    # Event-driven runs go quiet while the orchestrator waits on its workers; 30 s keeps the orders current.
    return {"type": "command", "command": f'"{exe}" nirvana statusline', "padding": 0, "refreshInterval": 30}


def install_plugin(home: Path, src: Path | None = None) -> Path:
    """`nirvana/claude/` to `<home>/nirvana/claude`, which the launch's `--plugin-dir` names, with the installed
    `settings.json` naming the installed binary's statusline."""
    src = ROOT / "nirvana" / "claude" if src is None else Path(src)
    dst = Path(home) / "nirvana" / "claude"
    dst.parent.mkdir(parents=True, exist_ok=True)
    _replace_tree(src, dst)
    settings_path = dst / "settings.json"
    settings = json.loads(settings_path.read_bytes())
    settings["statusLine"] = statusline(home)
    # `.gitattributes` pins LF, and Claude Code reads either; bytes keep Windows from writing CRLF.
    settings_path.write_bytes((json.dumps(settings, indent=2) + "\n").encode("utf-8"))
    return dst


def install_assets(home: Path, src: Path | None = None) -> list[Path]:
    src = ROOT / "nirvana" / "assets" if src is None else Path(src)
    dst = Path(home) / "nirvana" / "assets"
    dst.mkdir(parents=True, exist_ok=True)
    out = []
    for name in ASSETS:
        shutil.copy2(src / name, dst / name)
        out.append(dst / name)
    return out


def install_themes(config_dir: Path, src: Path | None = None) -> list[Path]:
    """The two Claude Code custom themes, byte for byte the ones Nirvana-C installs, so `custom:nirvana-dark` in the
    plugin's settings.json resolves whichever of the two installed them last."""
    src = ROOT / "nirvana" / "themes" if src is None else Path(src)
    dst = Path(config_dir) / "themes"
    dst.mkdir(parents=True, exist_ok=True)
    out = []
    for name in THEMES:
        shutil.copyfile(src / name, dst / name)
        out.append(dst / name)
    return out


def schemes(src: Path | None = None) -> list[dict]:
    src = ROOT / "nirvana" / "terminal" / "schemes.json" if src is None else Path(src)
    return json.loads(src.read_bytes().decode("utf-8"))


def profile(home: Path) -> dict:
    """The "Nirvana" profile, Nirvana-C's with the fork's herdr as its program: chosen from Windows Terminal's list it
    attaches to the `nirvana` session; the verb names the profile and passes its own command."""
    return {
        "name": PROFILE_NAME,
        "commandline": subprocess.list2cmdline([str(herdr_exe(home)), "--session", SESSION, "nirvana-shell"]),
        "icon": str(icon_path(home)),
        "tabTitle": PROFILE_NAME,
        "suppressApplicationTitle": True,
        "font": {"face": FONT_FACE, "size": FONT_SIZE},
        "colorScheme": {"light": "Nirvana Light", "dark": "Nirvana Dark"},
        "cursorShape": "bar",
        "padding": PADDING,
        "useAcrylic": False,
        "intenseTextStyle": "bright",
    }


def fragment(home: Path) -> dict:
    return {"profiles": [profile(home)], "schemes": schemes()}


def fragment_path(fragments: Path, app: str = FRAGMENT_APP) -> Path:
    return Path(fragments) / app / FRAGMENT_FILE


def write_fragment(fragments: Path, data: dict) -> Path:
    path = fragment_path(fragments)
    path.parent.mkdir(parents=True, exist_ok=True)
    # Bytes, LF: a text-mode write on Windows would turn LF into CRLF.
    path.write_bytes((json.dumps(data, indent=2) + "\n").encode("utf-8"))
    return path


def remove_old_fragment(fragments: Path) -> bool:
    """Nirvana-C's fragment defines a second profile named "Nirvana" and the same two scheme names; with both, `wt
    -p Nirvana` could start either. It moves here, so it goes."""
    path = fragment_path(fragments, OLD_FRAGMENT_APP)
    if not path.is_file():
        return False
    path.unlink()
    try:
        path.parent.rmdir()
    except OSError:
        pass
    return True


# ---- the registry ----

class Registry:
    """HKCU through winreg. Paths are relative to HKEY_CURRENT_USER."""

    def __init__(self) -> None:
        if winreg is None:
            raise OSError("winreg is not available on this platform")

    def set(self, path: str, name: str | None, value: str) -> None:
        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, path) as key:
            winreg.SetValueEx(key, name, 0, winreg.REG_SZ, value)

    def get(self, path: str, name: str | None) -> str | None:
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as key:
                return winreg.QueryValueEx(key, name)[0]
        except FileNotFoundError:
            return None

    def exists(self, path: str) -> bool:
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path):
                return True
        except FileNotFoundError:
            return False

    def delete_tree(self, path: str) -> bool:
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path, 0, winreg.KEY_ALL_ACCESS) as key:
                while True:
                    try:
                        child = winreg.EnumKey(key, 0)
                    except OSError:
                        break
                    self.delete_tree(rf"{path}\{child}")
        except FileNotFoundError:
            return False
        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, path)
        return True


class FakeRegistry:
    """The same four calls over a dict, case-insensitive like the real one; `ops` records every write and delete."""

    def __init__(self) -> None:
        self.values: dict[str, dict[str | None, str]] = {}
        self.ops: list[tuple] = []

    @staticmethod
    def _k(path: str) -> str:
        return path.lower()

    def set(self, path: str, name: str | None, value: str) -> None:
        self.ops.append(("set", path, name, value))
        key = self._k(path)
        parts = key.split("\\")
        for i in range(1, len(parts) + 1):     # parent keys exist, as CreateKey makes them
            self.values.setdefault("\\".join(parts[:i]), {})
        self.values[key][name] = value

    def get(self, path: str, name: str | None) -> str | None:
        return self.values.get(self._k(path), {}).get(name)

    def exists(self, path: str) -> bool:
        return self._k(path) in self.values

    def delete_tree(self, path: str) -> bool:
        self.ops.append(("delete", path))
        key = self._k(path)
        found = [k for k in self.values if k == key or k.startswith(key + "\\")]
        for k in found:
            del self.values[k]
        return bool(found)


def verb_command(launcher: Path | str, token: str) -> str:
    """The command a click runs: nirvana-open starts herdr without a console window."""
    return f'"{launcher}" "{token}"'


def install_verb(reg, base: str, launcher: Path | str, icon: Path | str) -> dict:
    written = {}
    for sub, token in VERBS:
        command = verb_command(launcher, token)
        reg.set(rf"{base}\{sub}", "MUIVerb", VERB_TEXT)
        reg.set(rf"{base}\{sub}", "Icon", str(icon))
        reg.set(rf"{base}\{sub}\command", None, command)
        written[sub] = {"MUIVerb": VERB_TEXT, "Icon": str(icon), "command": command}
    return written


def remove_verb(reg, base: str) -> dict:
    return {sub: reg.delete_tree(rf"{base}\{sub}") for sub, _ in VERBS}


def remove_old_verb(reg, base: str) -> dict:
    """Nirvana-C's "Open Nirvana - CC", both keys, whole trees."""
    return {sub: reg.delete_tree(rf"{base}\{sub}") for sub in OLD_VERBS}


def retire_old_nirvana_verb(reg, base: str, backup: Path) -> dict:
    """Old Nirvana's "Open Nirvana here", which opens the old daemon's window, removed after a JSON backup of its keys.

    The owner, R8: "have standard Nirvana use the herder varient". With both verbs installed, the folder menu showed
    "Open Nirvana here" (old daemon) beside "Open Nirvana" (this fork), checked 2026-10-18. A key is removed only when
    its command names old Nirvana's launcher, so another program's `Nirvana` key is never touched. The backup holds
    each key's MUIVerb, Icon and command; nothing here stops or edits the old daemon.
    """
    found: dict = {}
    for sub in OLD_NIRVANA_VERBS:
        command = reg.get(rf"{base}\{sub}\command", None)
        if command is not None and OLD_NIRVANA_LAUNCHER in command:
            found[sub] = {"MUIVerb": reg.get(rf"{base}\{sub}", "MUIVerb"), "Icon": reg.get(rf"{base}\{sub}", "Icon"),
                          "command": command}
    if not found:
        return {}
    backup = Path(backup)
    backup.parent.mkdir(parents=True, exist_ok=True)
    backup.write_text(json.dumps(found, indent=2) + "\n", encoding="utf-8")
    return {sub: reg.delete_tree(rf"{base}\{sub}") for sub in found}


def register_toast_id(reg, base: str, icon_uri: Path | str) -> dict:
    """`HKCU\\Software\\Classes\\AppUserModelId\\Nirvana.Herdr` with `DisplayName` and `IconUri`: P7 measured that
    this alone gives a toast the Nirvana mark in its header, with no Start-menu shortcut."""
    reg.set(rf"{base}\{TOAST_KEY}", "DisplayName", DISPLAY_NAME)
    reg.set(rf"{base}\{TOAST_KEY}", "IconUri", str(icon_uri))
    return {"DisplayName": DISPLAY_NAME, "IconUri": str(icon_uri)}


def read_verb(reg, base: str) -> dict:
    return {sub: {"MUIVerb": reg.get(rf"{base}\{sub}", "MUIVerb"), "Icon": reg.get(rf"{base}\{sub}", "Icon"),
                  "command": reg.get(rf"{base}\{sub}\command", None)} for sub, _ in VERBS}


# ---- JetBrains Mono (Nirvana-C nirvana_c/font.py, ported) ----

FONT_RELEASE_URL = "https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip"
FONT_RELEASE_SHA256 = "6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf"
FONT_FACES: dict[str, str] = {
    "fonts/ttf/JetBrainsMono-Regular.ttf": "JetBrains Mono Regular",
    "fonts/ttf/JetBrainsMono-Bold.ttf": "JetBrains Mono Bold",
    "fonts/ttf/JetBrainsMono-Italic.ttf": "JetBrains Mono Italic",
    "fonts/ttf/JetBrainsMono-BoldItalic.ttf": "JetBrains Mono Bold Italic",
    "fonts/ttf/JetBrainsMono-Medium.ttf": "JetBrains Mono Medium",
    "fonts/ttf/JetBrainsMono-SemiBold.ttf": "JetBrains Mono SemiBold",
}
USER_FONTS_KEY = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts"
MACHINE_FONTS_KEY = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts"


def font_installed() -> bool:
    """Any JetBrains Mono face registered for this user or the machine."""
    if winreg is None:
        return False
    for hive, path in ((winreg.HKEY_CURRENT_USER, USER_FONTS_KEY), (winreg.HKEY_LOCAL_MACHINE, MACHINE_FONTS_KEY)):
        try:
            with winreg.OpenKey(hive, path) as key:
                i = 0
                while True:
                    try:
                        name, _, _ = winreg.EnumValue(key, i)
                    except OSError:
                        break
                    i += 1
                    if name.startswith("JetBrains Mono"):
                        return True
        except OSError:
            pass
    return False


def install_font(reg, fonts_key: str, fonts_dir: Path, cache_dir: Path, opener=urllib.request.urlopen) -> dict:
    """The six faces, per user, the way Windows' own "Install for me" does: the file under
    `%LOCALAPPDATA%\\Microsoft\\Windows\\Fonts` and a `"<face> (TrueType)"` value. The download lands in its own
    folder and is held to the pinned sha256 before the zip is opened; only the named members are extracted."""
    cache_dir = Path(cache_dir)
    cache_dir.mkdir(parents=True, exist_ok=True)
    zip_path = cache_dir / Path(FONT_RELEASE_URL).name
    if not zip_path.is_file() or hashlib.sha256(zip_path.read_bytes()).hexdigest() != FONT_RELEASE_SHA256:
        with opener(FONT_RELEASE_URL, timeout=120) as response:
            data = response.read()
        digest = hashlib.sha256(data).hexdigest()
        if digest != FONT_RELEASE_SHA256:
            raise ValueError(f"JetBrains Mono download hash {digest} is not the pinned {FONT_RELEASE_SHA256}")
        zip_path.write_bytes(data)
    fonts_dir = Path(fonts_dir)
    fonts_dir.mkdir(parents=True, exist_ok=True)
    copied = {}
    with zipfile.ZipFile(zip_path) as archive:
        for member, face in FONT_FACES.items():
            dest = fonts_dir / Path(member).name
            with archive.open(member) as src, open(dest, "wb") as dst:
                shutil.copyfileobj(src, dst)
            reg.set(fonts_key, f"{face} (TrueType)", str(dest))
            copied[face] = str(dest)
    try:
        for path in copied.values():
            ctypes.windll.gdi32.AddFontResourceW(path)
        ctypes.windll.user32.SendMessageW(0xFFFF, 0x001D, 0, 0)   # HWND_BROADCAST, WM_FONTCHANGE
    except (AttributeError, OSError):
        pass
    return copied


# ---- everything ----

def install_all(home: Path, *, reg=None, base: str = CLASSES, fragments: Path | None = None,
                config_dir: Path | None = None, wt: Path | None = None, font: bool = True) -> dict:
    """The whole Windows side, in an order where a failure leaves nothing that points at a missing file: the files
    first, then the keys that name them, Nirvana-C's verb removed last so a failed install never leaves no verb."""
    home = Path(home)
    reg = Registry() if reg is None else reg
    fragments = fragments_root() if fragments is None else Path(fragments)
    config_dir = claude_config_dir() if config_dir is None else Path(config_dir)
    wt = wt_path() if wt is None else Path(wt)
    report: dict = {}
    report["assets"] = [str(p) for p in install_assets(home)]
    report["themes"] = [str(p) for p in install_themes(config_dir)]
    report["fragment"] = str(write_fragment(fragments, fragment(home)))
    report["old_fragment_removed"] = remove_old_fragment(fragments)
    if font and not font_installed():
        local = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
        report["font"] = install_font(reg, USER_FONTS_KEY, local / "Microsoft" / "Windows" / "Fonts",
                                      local / "Nirvana-Herdr" / "font-cache")
    else:
        report["font"] = "already installed" if font else "skipped"
    report["toast"] = register_toast_id(reg, base, toast_icon_path(home))
    report["verb"] = install_verb(reg, base, nirvana_open_exe(home), icon_path(home))
    report["old_verb_removed"] = remove_old_verb(reg, base)
    report["old_nirvana_verb_retired"] = retire_old_nirvana_verb(reg, base,
                                                                 home / "nirvana" / "backup" / "old-nirvana-verb.json")
    return report
