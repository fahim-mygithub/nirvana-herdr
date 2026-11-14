"""Build the fork and install it where a running Nirvana-Herdr looks for a newer build.

    python nirvana/install.py             # cargo build --release, then install
    python nirvana/install.py --no-build  # install target/release/herdr.exe as it is
    python nirvana/install.py --no-shell  # the binary and the plugin only: no verb, profile, font or registry
    python nirvana/install.py --no-font   # everything but JetBrains Mono

The install root is $NIRVANA_HERDR_HOME, else %LOCALAPPDATA%\\Programs\\Nirvana-Herdr (src/nirvana.rs reads the
same rule). The binary goes to <root>/bin/herdr.exe and its version to <root>/bin/nirvana-version.txt.

Order matters. A running herdr.exe cannot be overwritten on Windows but can be renamed, so the old one is moved
aside first. The stamp is written last, after the new binary is in place: the running server reads the stamp every
5 s and offers a reload as soon as it changes, so a stamp that came first could offer a binary that is not there yet.

Plan 12 O4a: the Nirvana Claude plugin (`nirvana/claude/` to `<root>/nirvana/claude`) is copied before the binary,
so a reload never starts a Nirvana Claude whose plugin is older than its herdr. Then, on Windows, `windows_shell.py`
writes the mark, the themes, the Windows Terminal profile, JetBrains Mono when missing, the toast's app id and the
Explorer verb "Open Nirvana", and removes Nirvana-C's "Open Nirvana - CC".

Plan 41 P41c: the agy orchestrator's plugin `<root>/nirvana/agy/nirvana` is built from the same `nirvana/claude/`
right after the Claude plugin (`agy_plugin.py`), for the same reason. It is validated with
`agy plugin validate --strict` and then installed with `agy plugin install <root>/nirvana/agy/nirvana`, which copies
it into agy's config under `~/.gemini` (owner ruling R1, changed to (b)). A build that fails validation leaves the
folder and agy's copy as they were. A failed agy install leaves agy's previous copy in place. In both cases the rest
of the install goes on and the exit is 1. With agy not on PATH, both steps are skipped, a line says so, and the exit
code does not change.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import agy_plugin  # noqa: E402
import windows_shell  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
EXE = "herdr.exe" if os.name == "nt" else "herdr"
OPEN_EXE = "nirvana-open.exe" if os.name == "nt" else "nirvana-open"
STAMP = "nirvana-version.txt"


def home(env: dict | None = None) -> Path:
    env = os.environ if env is None else env
    if env.get("NIRVANA_HERDR_HOME"):
        return Path(env["NIRVANA_HERDR_HOME"])
    base = env.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Programs" / "Nirvana-Herdr"


def version() -> str:
    return (ROOT / "nirvana" / "VERSION").read_text(encoding="utf-8").strip()


def install_binary(built: Path, target: Path, prefix: str) -> Path:
    bin_dir = target.parent
    bin_dir.mkdir(parents=True, exist_ok=True)
    for old in bin_dir.glob(f"{prefix}.old-*"):
        try:
            old.unlink()          # an old build still running (a supervisor, a server) stays locked; skip it
        except OSError:
            pass
    aside = None
    if target.exists():
        aside = bin_dir / f"{prefix}.old-{time.strftime('%Y%m%d-%H%M%S')}-{os.getpid()}{target.suffix}"
        target.rename(aside)
    try:
        shutil.copy2(built, target)
    except BaseException:
        # Plan 40 review B4: a full disk or a scanner holding the new file left no herdr.exe at all, and the
        # Explorer verb and the plugin hooks named a missing file until the next install. The old one goes back.
        if aside is not None:
            try:
                target.unlink()
            except OSError:
                pass
            os.replace(aside, target)
        raise
    return target


def install(built: Path, root: Path, ver: str, built_open: Path | None = None) -> Path:
    bin_dir = root / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    target = install_binary(built, bin_dir / EXE, "herdr")
    if built_open is None:
        candidate = built.parent / OPEN_EXE
        if candidate.is_file():
            built_open = candidate
    if built_open is not None and built_open.is_file():
        install_binary(built_open, bin_dir / OPEN_EXE, "nirvana-open")
    tmp = bin_dir / (STAMP + ".tmp")
    tmp.write_bytes((ver + "\n").encode("utf-8"))
    os.replace(tmp, bin_dir / STAMP)
    return target


def agy_report(agy_dir: Path, report: dict) -> None:
    """What `agy_plugin.install` did. The agy line names the command run, the plugin agy named and the folder it
    copied to, so the live check at merge can find the copy without guessing agy's layout."""
    checked = report["validate"]
    print(f"built the agy orchestrator plugin at {agy_dir / agy_plugin.NAME}: skills {len(report['skills'])}, "
          f"agents {len(report['agents'])} (the role is the nirvana agent's body), mcp {report['mcp'] or 'none'}; "
          f"{'validated' if checked['ok'] else checked['why']}")
    for line in report["skipped"]:
        print(f"  skipped {line}")
    done = report.get("agy_install") or {}
    if done.get("ok"):
        print(f"agy plugin install: installed plugin {done.get('name') or '(name not printed)'} to "
              f"{done.get('folder') or '(folder not printed)'}"
              f"{' (replaced the earlier copy)' if done.get('replaced') else ''}")
    elif done.get("ok") is None:
        print(f"agy plugin install: skipped, {done.get('why', 'not run')}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="install")
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--no-shell", action="store_true", help="skip the Windows verb, profile, font and registry")
    ap.add_argument("--no-font", action="store_true", help="skip JetBrains Mono")
    ns = ap.parse_args(argv)
    if not ns.no_build:
        done = subprocess.run(["cargo", "build", "--release", "--locked"], cwd=str(ROOT))
        if done.returncode != 0:
            print(f"install: cargo build failed (exit {done.returncode}); nothing installed")
            return done.returncode
    built = ROOT / "target" / "release" / EXE
    built_open = ROOT / "target" / "release" / OPEN_EXE
    if not built.exists():
        print(f"install: {built} is missing; run without --no-build")
        return 2
    if not built_open.exists():
        print(f"install: {built_open} is missing; run without --no-build")
        return 2
    ver = version()
    root = home()
    # Review N5: a plugin folder that could not be renamed is overlaid, and a file in it a running Nirvana Claude
    # holds keeps its old bytes. The rest of the install goes on, and the exit says the folder is mixed.
    stale: list = []
    try:
        plugin = windows_shell.install_plugin(root)
        print(f"installed the Nirvana Claude plugin at {plugin}")
    except windows_shell.StaleFiles as err:
        stale = err.paths
        print(f"the Nirvana Claude plugin is installed with {len(stale)} old files a running program holds:")
        for path in stale:
            print(f"  stale {path}")
    agy_failed = None
    try:
        agy_dir, report = agy_plugin.install(root)
        agy_report(agy_dir, report)
    except windows_shell.StaleFiles as err:
        stale += err.paths
        print(f"the agy orchestrator folder is installed with {len(err.paths)} old files a running program holds; "
              "agy plugin install was not run:")
        for path in err.paths:
            print(f"  stale {path}")
    except agy_plugin.BuildError as err:
        agy_failed = err
        print(f"the agy orchestrator folder was not installed; the one there stays: {err}")
    except agy_plugin.AgyInstallError as err:
        agy_failed = err
        agy_report(err.dst, err.report)
        print(f"agy plugin install failed; agy keeps the copy it had: {err}")
    target = install(built, root, ver, built_open=built_open)
    print(f"installed Nirvana-Herdr alpha {ver} at {target}")
    if os.name == "nt" and not ns.no_shell:
        report = windows_shell.install_all(root, font=not ns.no_font)
        for sub, values in report["verb"].items():
            print(f"verb HKCU\\{windows_shell.CLASSES}\\{sub}: {values['command']}")
        removed = [sub for sub, gone in report["old_verb_removed"].items() if gone]
        print(f"removed Nirvana-C's verb keys: {removed or 'none were there'}")
        print(f"toast id HKCU\\{windows_shell.CLASSES}\\{windows_shell.TOAST_KEY}: {report['toast']}")
        print(f"Windows Terminal profile: {report['fragment']}; JetBrains Mono: "
              f"{'installed' if isinstance(report['font'], dict) else report['font']}")
    print("a running Nirvana-Herdr shows a reload badge within 5 s; press it twice to restart onto this build")
    if stale:
        print(f"install: {len(stale)} plugin files are stale. Close every Nirvana Claude and run the install again")
        return 1
    if agy_failed is not None:
        print("install: the agy orchestrator plugin failed its build, validation or agy plugin install; see above")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
