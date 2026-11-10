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
`"<home>\\bin\\nirvana-open.exe" "%1"` (and `"%V"` for the background verb). Plan 21: it runs `herdr nirvana open`,
which opens the folder's space in the one live Nirvana window and flashes its button, or, with none live, starts
`wt -w new ... herdr nirvana window`, which starts the server, Nirvana Claude, the button, the badge and the toast
(src/nirvana_launch.rs). Limits: a folder whose name has a `;` is split by Windows Terminal,
which reads `;` as its own command separator; a drive root arrives as `C:"` and herdr puts the `\\` back.
"""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
import re
import shutil
import stat
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

def _copy_file(src, dst):
    """`copytree`'s copier, by name, so a test can make one copy fail (`copytree` binds `copy2` at definition)."""
    return shutil.copy2(src, dst)


def _remove_quietly(path: Path) -> None:
    """A tree removed as far as it goes: a file a running Nirvana Claude holds stays until the next install. Python
    3.8 and later remove a junction inside as a link, not through it."""
    shutil.rmtree(path, ignore_errors=True)


class StaleFiles(OSError):
    """The plugin folder could not be renamed and these files in it could not be replaced or removed: it now holds
    old and new files mixed. Raised after the rest of the copy is done."""

    def __init__(self, paths: list[Path]):
        super().__init__(f"{len(paths)} plugin files are stale, first {paths[0] if paths else '-'}")
        self.paths = paths


def _is_link(path) -> bool:
    """A symbolic link or any reparse point; `Path.is_symlink` does not see a junction."""
    try:
        st = os.lstat(path)
    except OSError:
        return False
    return stat.S_ISLNK(st.st_mode) or bool(getattr(st, "st_file_attributes", 0) & 0x400)


def _remove_link(path) -> None:
    """The link itself, never its target: a directory link or junction by `rmdir`, a file link by `unlink`."""
    if os.name == "nt" and getattr(os.lstat(path), "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_DIRECTORY:
        os.rmdir(path)
    else:
        os.unlink(path)


def _overlay(new: Path, dst: Path) -> list[Path]:
    """`new` laid over `dst`, then what `new` does not have removed from `dst`; the paths left stale.

    Review N5: the first overlay was `copytree(dirs_exist_ok=True)`, which truncates a file and then writes it (a
    full disk left a corrupt file) and stopped at its end with old and new files mixed and nothing said; its prune
    walked `rglob`, which before Python 3.13 descends a junction inside `dst`. Now each file is copied beside its
    target and renamed over it, so a file is whole old or whole new; nothing is entered through a link; and every
    file that kept its old bytes, or that should have gone and did not, is named."""
    stale: list[Path] = []

    def lay(s_dir: Path, d_dir: Path) -> None:
        for entry in os.scandir(s_dir):
            target = d_dir / entry.name
            if entry.is_dir(follow_symlinks=False):
                try:
                    if _is_link(target):
                        _remove_link(target)
                    elif os.path.lexists(target) and not target.is_dir():
                        os.unlink(target)
                    target.mkdir(exist_ok=True)
                except OSError:
                    stale.append(target)
                    continue
                lay(Path(entry.path), target)
                continue
            tmp = target.with_name(f"{target.name}.install-{os.getpid()}")
            try:
                if _is_link(target):
                    _remove_link(target)
                _copy_file(entry.path, tmp)
                os.replace(tmp, target)
            except OSError:
                try:
                    os.unlink(tmp)
                except OSError:
                    pass
                stale.append(target)

    def prune(s_dir: Path | None, d_dir: Path) -> None:
        """`s_dir` None: nothing under `d_dir` is kept."""
        for entry in os.scandir(d_dir):
            path = Path(entry.path)
            mirror = None if s_dir is None else s_dir / entry.name
            try:
                if _is_link(path):
                    if mirror is None or not os.path.lexists(mirror):
                        _remove_link(path)
                elif entry.is_dir(follow_symlinks=False):
                    if mirror is not None and mirror.is_dir() and not _is_link(mirror):
                        prune(mirror, path)
                    else:
                        prune(None, path)
                        path.rmdir()
                elif mirror is None or not (mirror.is_file() and not _is_link(mirror)):
                    os.unlink(path)
            except OSError:
                stale.append(path)

    lay(new, dst)
    prune(new, dst)
    return stale


def _replace_tree(src: Path, dst: Path) -> None:
    """`dst` becomes a copy of `src`: a file removed from the tree is removed at the install too.

    Plan 40 review B3: this was `rmtree(dst)` then `copytree`, so a file a running Nirvana Claude held (a hook, a
    skill, an antivirus scan) stopped the delete midway and left the live plugin half gone, and the install aborted
    before the binary. Now the new tree is copied whole beside it first, and only a finished copy takes the name: a
    failed copy leaves `dst` as it was. If `dst` itself cannot be renamed (Windows refuses a folder with an open
    file), the copy is laid over it file by file and only files the tree no longer has are removed; every file is
    then the old one or the new one, never missing, and a file that could not be replaced raises `StaleFiles` once
    the rest is laid down."""
    src, dst = Path(src), Path(dst)
    tag = f"{os.getpid()}"
    # Review N5: `claude.old-*` also took a sibling the owner named so by hand; only this function's own names go.
    own = re.compile(rf"{re.escape(dst.name)}\.(?:new|old)-\d+")
    for stale in dst.parent.iterdir() if dst.parent.is_dir() else ():
        if own.fullmatch(stale.name) and not _is_link(stale):
            _remove_quietly(stale)
    new = dst.with_name(f"{dst.name}.new-{tag}")
    try:
        shutil.copytree(src, new, copy_function=_copy_file)
    except BaseException:
        _remove_quietly(new)
        raise
    if not dst.exists():
        os.replace(new, dst)
        return
    old = dst.with_name(f"{dst.name}.old-{tag}")
    try:
        os.replace(dst, old)
    except OSError:
        # A held file: overlay in place.
        try:
            stale = _overlay(new, dst)
        finally:
            _remove_quietly(new)
        if stale:
            raise StaleFiles(stale)
        return
    try:
        os.replace(new, dst)
    except BaseException:
        os.replace(old, dst)
        _remove_quietly(new)
        raise
    _remove_quietly(old)


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
    stale = None
    try:
        _replace_tree(src, dst)
    except StaleFiles as err:
        stale = err          # the rest of the copy is down; the settings below still name the installed binary
    settings_path = dst / "settings.json"
    settings = json.loads(settings_path.read_bytes())
    settings["statusLine"] = statusline(home)
    # `.gitattributes` pins LF, and Claude Code reads either; bytes keep Windows from writing CRLF.
    settings_path.write_bytes((json.dumps(settings, indent=2) + "\n").encode("utf-8"))
    if stale is not None:
        raise stale
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
    attaches to the `nirvana` session; the verb names the profile and passes its own command.

    No font, colour scheme, padding or cursor: the window takes Windows Terminal's defaults, the look plain Claude
    Code has in the owner's PowerShell window. The owner, 2026-10-21: "I would like the standard Claude Code color
    scheme back ... and the font as well". The Nirvana schemes stay in the fragment, so either can still be picked
    in Terminal's settings."""
    return {
        "name": PROFILE_NAME,
        "commandline": subprocess.list2cmdline([str(herdr_exe(home)), "--session", SESSION, "nirvana-shell"]),
        "icon": str(icon_path(home)),
        "tabTitle": PROFILE_NAME,
        "suppressApplicationTitle": True,
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
                config_dir: Path | None = None, wt: Path | None = None, font: bool = True,
                font_present=None, fonts_dir: Path | None = None, font_cache: Path | None = None,
                opener=urllib.request.urlopen) -> dict:
    """The whole Windows side, in an order where a failure leaves nothing that points at a missing file: the files
    first, then the keys that name them, Nirvana-C's verb removed last so a failed install never leaves no verb.

    `font_present`, `fonts_dir`, `font_cache` and `opener` are the font step's world, for a test to replace."""
    home = Path(home)
    font_present = font_installed if font_present is None else font_present
    reg = Registry() if reg is None else reg
    fragments = fragments_root() if fragments is None else Path(fragments)
    config_dir = claude_config_dir() if config_dir is None else Path(config_dir)
    wt = wt_path() if wt is None else Path(wt)
    report: dict = {}
    report["assets"] = [str(p) for p in install_assets(home)]
    report["themes"] = [str(p) for p in install_themes(config_dir)]
    report["fragment"] = str(write_fragment(fragments, fragment(home)))
    report["old_fragment_removed"] = remove_old_fragment(fragments)
    if font and not font_present():
        local = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
        fonts_dir = local / "Microsoft" / "Windows" / "Fonts" if fonts_dir is None else Path(fonts_dir)
        font_cache = local / "Nirvana-Herdr" / "font-cache" if font_cache is None else Path(font_cache)
        # Plan 40 review B2: offline, `opener` raised URLError (a proxy page fails the sha with ValueError) after
        # install.py had put the new binary and its stamp in place, so the toast id, the verb and Nirvana-C's
        # cleanup never ran and the script ended in a traceback. The font is cosmetic: the rest goes on.
        try:
            report["font"] = install_font(reg, USER_FONTS_KEY, fonts_dir, font_cache, opener=opener)
        except (OSError, ValueError, zipfile.BadZipFile, KeyError) as err:
            report["font"] = f"failed: {err}"
    else:
        report["font"] = "already installed" if font else "skipped"
    report["toast"] = register_toast_id(reg, base, toast_icon_path(home))
    report["verb"] = install_verb(reg, base, nirvana_open_exe(home), icon_path(home))
    report["old_verb_removed"] = remove_old_verb(reg, base)
    report["old_nirvana_verb_retired"] = retire_old_nirvana_verb(reg, base,
                                                                 home / "nirvana" / "backup" / "old-nirvana-verb.json")
    return report
