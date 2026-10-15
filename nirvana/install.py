"""Build the fork and install it where a running Nirvana-Herdr looks for a newer build.

    python nirvana/install.py             # cargo build --release, then install
    python nirvana/install.py --no-build  # install target/release/herdr.exe as it is

The install root is $NIRVANA_HERDR_HOME, else %LOCALAPPDATA%\\Programs\\Nirvana-Herdr (src/nirvana.rs reads the
same rule). The binary goes to <root>/bin/herdr.exe and its version to <root>/bin/nirvana-version.txt.

Order matters. A running herdr.exe cannot be overwritten on Windows but can be renamed, so the old one is moved
aside first. The stamp is written last, after the new binary is in place: the running server reads the stamp every
5 s and offers a reload as soon as it changes, so a stamp that came first could offer a binary that is not there yet.
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXE = "herdr.exe" if os.name == "nt" else "herdr"
STAMP = "nirvana-version.txt"


def home(env: dict | None = None) -> Path:
    env = os.environ if env is None else env
    if env.get("NIRVANA_HERDR_HOME"):
        return Path(env["NIRVANA_HERDR_HOME"])
    base = env.get("LOCALAPPDATA") or str(Path.home() / "AppData" / "Local")
    return Path(base) / "Programs" / "Nirvana-Herdr"


def version() -> str:
    return (ROOT / "nirvana" / "VERSION").read_text(encoding="utf-8").strip()


def install(built: Path, root: Path, ver: str) -> Path:
    bin_dir = root / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    target = bin_dir / EXE
    for old in bin_dir.glob("herdr.old-*"):
        try:
            old.unlink()          # an old build still running (a supervisor, a server) stays locked; skip it
        except OSError:
            pass
    if target.exists():
        target.rename(bin_dir / f"herdr.old-{time.strftime('%Y%m%d-%H%M%S')}-{os.getpid()}{target.suffix}")
    shutil.copy2(built, target)
    tmp = bin_dir / (STAMP + ".tmp")
    tmp.write_bytes((ver + "\n").encode("utf-8"))
    os.replace(tmp, bin_dir / STAMP)
    return target


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="install")
    ap.add_argument("--no-build", action="store_true")
    ns = ap.parse_args(argv)
    if not ns.no_build:
        done = subprocess.run(["cargo", "build", "--release", "--locked"], cwd=str(ROOT))
        if done.returncode != 0:
            print(f"install: cargo build failed (exit {done.returncode}); nothing installed")
            return done.returncode
    built = ROOT / "target" / "release" / EXE
    if not built.exists():
        print(f"install: {built} is missing; run without --no-build")
        return 2
    ver = version()
    target = install(built, home(), ver)
    print(f"installed Nirvana-Herdr alpha {ver} at {target}")
    print("a running Nirvana-Herdr shows a reload badge within 5 s; press it twice to restart onto this build")
    return 0


if __name__ == "__main__":
    sys.exit(main())
