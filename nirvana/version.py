"""The fork's version, one number in nirvana/VERSION, bumped once per merge (Nirvana CLAUDE.md rule 16: "That way I
can see if the Nirvana window is running the new version").

    python nirvana/version.py show
    python nirvana/version.py bump feature   # 0.1.3 -> 0.2.0
    python nirvana/version.py bump fix       # 0.1.3 -> 0.1.4

herdr's own version in Cargo.toml is left alone: it names the upstream base and gates herdr's protocol checks.
"""
from __future__ import annotations

import sys
from pathlib import Path

FILE = Path(__file__).resolve().parent / "VERSION"


def bumped(current: str, kind: str) -> str:
    major, feature, fix = (int(p) for p in current.strip().split("."))
    if kind == "feature":
        return f"{major}.{feature + 1}.0"
    if kind == "fix":
        return f"{major}.{feature}.{fix + 1}"
    raise ValueError(f"bump kind is feature or fix, not {kind!r}")


def main(argv: list[str]) -> int:
    current = FILE.read_text(encoding="utf-8").strip()
    if argv[:1] == ["show"] or not argv:
        print(f"alpha {current}")
        return 0
    if argv[:1] == ["bump"] and len(argv) == 2:
        new = bumped(current, argv[1])
        FILE.write_bytes((new + "\n").encode("utf-8"))
        print(f"alpha {current} -> alpha {new}")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
