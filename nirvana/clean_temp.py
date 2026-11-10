"""Removes the backlog of test folders in %TEMP% (plan 40, "tests that stop leaking temp folders").

    python nirvana/clean_temp.py                        # dry run: counts and bytes by name pattern, nothing removed
    python nirvana/clean_temp.py --apply                # remove what the dry run calls removable
    python nirvana/clean_temp.py --older-than-hours 72 --top 50

Measured 2026-11-09: 35,326 folders named `herdr-*` or `nirvana-*` in %TEMP%, 10,595 from the last 24 hours. The fork's
tests make a folder under `std::env::temp_dir()` named for the test and the process id and never remove it; nextest
runs every test in its own process, so every run of the suite adds one folder per test.

What `--apply` removes, and only that: a directory directly under the root (default %TEMP%) whose name starts with
`herdr-` or `nirvana-` and ends with a numeric pid, or a pid and a timestamp (`herdr-o5-gate-owned-42660-17924637751`),
last written more than `--older-than-hours` ago (default 48), whose pid no longer runs, and that a rename tells is not
open: Windows refuses to rename a folder with an open file or a process's working directory inside it. The folder is
renamed to `<name>.cleaning-<pid>` first and removed under that name, so a folder in use is never half removed, and a
run that dies leaves a name the next run picks up again.

Inside a folder nothing is followed: each entry is checked before it is descended into, and a junction or a symbolic
link is removed as a link, never through (a test that made a junction to a real folder would otherwise have that folder
emptied). A link or junction at the top is skipped. Every folder left is printed with the reason, in the dry run too.
"""
from __future__ import annotations

import argparse
import collections
import os
import re
import stat
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

# `<kind>-<stem>-<pid>` or `<kind>-<stem>-<pid>-<timestamp>`, and the tombstone a dead run of this script left. A pid
# is at most 7 digits (Windows pids are multiples of 4 below 2^32, in practice under a million); a timestamp at least
# 10 (unix seconds, or nanoseconds as the herdr tests write them).
NAME = re.compile(
    r"^(?P<kind>herdr|nirvana)-(?P<stem>.+?)-(?P<pid>\d{1,7})(?:-(?P<ts>\d{10,}))?(?:\.cleaning-\d{1,7})?$"
)
# Two named makers whose folders do not end in a pid (plan 40 review round: the pid rule alone kept 20,864 of the
# 35,326 folders the first dry run counted). Each is matched before `NAME`, so a p13a folder whose test name happens to
# end in digits is not read as a pid there.
#   `nirvana-fake-daemon-<8 of [a-z0-9_]>`: old Nirvana's `ui/dev/fake-daemon.py`, `tempfile.mkdtemp(prefix=...)`. No
#   pid in the name, so only the age, link, in-use and top-level rules decide.
#   `nirvana-p13a-<pid>-<count>-<name>`: `scratch()` in the fork's `src/nirvana_preset/tests.rs`.
NAMED = (
    ("nirvana-fake-daemon-<random>",
     re.compile(r"^nirvana-fake-daemon-[a-z0-9_]{8}(?:\.cleaning-\d{1,7})?$")),
    ("nirvana-p13a-<pid>-<n>-<name>",
     re.compile(r"^nirvana-p13a-(?P<pid>\d{1,7})-\d+-[A-Za-z0-9_-]+(?:\.cleaning-\d{1,7})?$")),
)
PREFIXES = ("herdr-", "nirvana-")


def pid_alive(pid: int) -> bool:
    """Whether `pid` runs. Never `os.kill(pid, 0)`: on Windows that is TerminateProcess."""
    if pid == os.getpid():
        return True
    if os.name == "nt":
        import ctypes
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        handle = kernel.OpenProcess(0x1000, False, pid)    # PROCESS_QUERY_LIMITED_INFORMATION
        if not handle:
            # Access denied means it exists (a protected process); anything else, it does not.
            return ctypes.get_last_error() == 5
        try:
            code = ctypes.c_ulong()
            ok = kernel.GetExitCodeProcess(handle, ctypes.byref(code))
            return bool(ok) and code.value == 259          # STILL_ACTIVE
        finally:
            kernel.CloseHandle(handle)
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    return True


def is_link(path: str | os.PathLike) -> bool:
    """A symbolic link or any reparse point (a junction is one; `os.path.islink` does not see a junction)."""
    try:
        st = os.lstat(path)
    except OSError:
        return False
    if stat.S_ISLNK(st.st_mode):
        return True
    return bool(getattr(st, "st_file_attributes", 0) & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0x400))


def _remove_link(path: str) -> None:
    """The link itself: a directory link or junction by `rmdir` (which removes the reparse point, not its target), a
    file link by `unlink`."""
    st = os.lstat(path)
    attrs = getattr(st, "st_file_attributes", 0)
    if os.name == "nt" and attrs & stat.FILE_ATTRIBUTE_DIRECTORY:
        os.rmdir(path)
    else:
        os.unlink(path)


def _unlink(path: str) -> None:
    try:
        os.unlink(path)
    except PermissionError:
        os.chmod(path, stat.S_IWRITE)     # a read-only file (git objects in a test repository)
        os.unlink(path)


def remove_tree(top: str | os.PathLike) -> list[str]:
    """Removes `top` and what it holds without following any link out of it; the failures, one line each (empty
    when all went).

    Plan 40 review S1: this trusted its caller to have checked `top`, and the callers checked minutes earlier (a
    scan of 35,326 folders) or never (`gate.py`'s private TEMP). A name swapped for a junction in between was walked
    and the junction's target emptied. So the link test is made here, on `top` and on every folder, right before
    the folder is listed: a link at the top is refused and left as it is, a link inside is removed as a link."""
    failures: list[str] = []

    def walk(folder: str) -> None:
        if is_link(folder):
            failures.append(f"{folder}: a link or junction, not followed")
            return
        try:
            entries = list(os.scandir(folder))
        except OSError as err:
            failures.append(f"{folder}: {err}")
            return
        for entry in entries:
            path = entry.path
            try:
                if is_link(path):
                    _remove_link(path)
                elif entry.is_dir(follow_symlinks=False):
                    walk(path)
                else:
                    _unlink(path)
            except OSError as err:
                failures.append(f"{path}: {err}")
        try:
            os.rmdir(folder)
        except OSError as err:
            failures.append(f"{folder}: {err}")

    walk(os.fspath(top))
    return failures


def tree_bytes(top: str | os.PathLike) -> int:
    """The bytes of the files under `top`, links not followed."""
    total = 0
    stack = [os.fspath(top)]
    while stack:
        folder = stack.pop()
        try:
            entries = list(os.scandir(folder))
        except OSError:
            continue
        for entry in entries:
            try:
                if is_link(entry.path):
                    continue
                if entry.is_dir(follow_symlinks=False):
                    stack.append(entry.path)
                else:
                    total += entry.stat(follow_symlinks=False).st_size
            except OSError:
                continue
    return total


@dataclass
class Found:
    name: str
    path: Path
    pattern: str
    removable: bool
    why: str
    size: int = 0


def rule_of(name: str) -> tuple[str, re.Match] | None:
    """The named rule the name falls under and its match, or None (then `--apply` never takes it)."""
    for label, rule in NAMED:
        m = rule.match(name)
        if m:
            return label, m
    m = NAME.match(name)
    return ("pid", m) if m else None


def pattern_of(name: str) -> str:
    """The name with its pid suffix as `<pid>` (and `<ts>`), the group a dry run counts it under; a named rule's
    label; any other name with each digit run as `N`."""
    for label, rule in NAMED:
        if rule.match(name):
            return label
    m = NAME.match(name)
    if m:
        return f"{m['kind']}-{m['stem']}-<pid>" + ("-<ts>" if m["ts"] else "")
    return re.sub(r"\d+", "N", name)


def scan(root: Path, now: float, older_than_hours: float, alive=pid_alive, sizes: bool = True) -> list[Found]:
    """Every `herdr-*` and `nirvana-*` entry directly under `root`, each with whether `--apply` may remove it and
    why. Nothing is changed."""
    found: list[Found] = []
    limit = older_than_hours * 3600
    with os.scandir(root) as entries:
        for entry in entries:
            name = entry.name
            if not name.startswith(PREFIXES):
                continue
            path = Path(entry.path)
            item = Found(name, path, pattern_of(name), False, "")
            found.append(item)
            if is_link(path):
                item.why = "a link or junction: left as it is"
                continue
            if not entry.is_dir(follow_symlinks=False):
                item.why = "not a folder"
                continue
            if sizes:
                item.size = tree_bytes(path)
            ruled = rule_of(name)
            if ruled is None:
                item.why = "its name does not end in a pid"
                continue
            m = ruled[1]
            try:
                age = now - entry.stat(follow_symlinks=False).st_mtime
            except OSError as err:
                item.why = f"unreadable: {err}"
                continue
            if age < limit:
                item.why = f"younger than {older_than_hours:g} h"
                continue
            if m.re.groupindex.get("pid") and alive(int(m["pid"])):
                item.why = f"pid {m['pid']} still runs"
                continue
            item.removable, item.why = True, "removable"
    return found


def apply(found: list[Found]) -> tuple[list[str], int, int]:
    """Removes each removable folder through its tombstone; (lines for what was skipped or failed, folders removed,
    bytes removed)."""
    lines: list[str] = []
    removed = removed_bytes = 0
    for item in found:
        if not item.removable:
            continue
        # Plan 40 review S1: the scan saw a folder, minutes ago. Asked again now, right before the rename, and again
        # after it (`remove_tree` asks a third time before it lists anything): a name that became a link since is
        # left, and a link renamed by mistake is renamed back.
        if is_link(item.path):
            item.removable, item.why = False, "became a link or junction since the scan: left as it is"
            lines.append(f"skipped {item.name}: {item.why}")
            continue
        if ".cleaning-" in item.name:
            tomb = item.path        # a dead run's tombstone already
        else:
            tomb = item.path.with_name(f"{item.name}.cleaning-{os.getpid()}")
            try:
                os.rename(item.path, tomb)
            except OSError as err:
                item.removable, item.why = False, f"in use: {err.strerror or err}"
                lines.append(f"skipped {item.name}: {item.why}")
                continue
        if is_link(tomb):
            if tomb != item.path:
                try:
                    os.rename(tomb, item.path)
                except OSError:
                    pass
            item.removable, item.why = False, "a link or junction after the rename: renamed back, left as it is"
            lines.append(f"skipped {item.name}: {item.why}")
            continue
        failures = remove_tree(tomb)
        if failures:
            lines.append(f"failed {item.name}: {len(failures)} entries left, first {failures[0]}")
            continue
        removed += 1
        removed_bytes += item.size
    return lines, removed, removed_bytes


def human(n: int) -> str:
    for unit, size in (("GB", 1 << 30), ("MB", 1 << 20), ("KB", 1 << 10)):
        if n >= size:
            return f"{n / size:.1f} {unit}"
    return f"{n} bytes"


def summary(found: list[Found], top: int) -> list[str]:
    """Totals, then the largest groups that may go and the reasons the rest stay, each with its count and bytes."""
    lines: list[str] = []
    can = [f for f in found if f.removable]
    stay = [f for f in found if not f.removable]
    lines.append(f"{len(found)} herdr-/nirvana- entries, {human(sum(f.size for f in found))}; "
                 f"{len(can)} removable ({human(sum(f.size for f in can))}), {len(stay)} kept")
    why = collections.Counter()
    why_bytes = collections.Counter()
    for f in stay:
        reason = re.sub(r"pid \d+ still runs", "its pid still runs", f.why)
        why[reason] += 1
        why_bytes[reason] += f.size
    for reason, count in why.most_common():
        lines.append(f"  kept {count:>6}  {human(why_bytes[reason]):>10}  {reason}")
    for label, items in (("removable", can), ("kept", stay)):
        groups = collections.Counter(f.pattern for f in items)
        sizes = collections.Counter()
        for f in items:
            sizes[f.pattern] += f.size
        lines.append(f"{label}, by name pattern ({len(groups)} patterns, the largest {min(top, len(groups))}):")
        for pattern, count in groups.most_common(top):
            lines.append(f"  {count:>6}  {human(sizes[pattern]):>10}  {pattern}")
    return lines


def default_root() -> Path:
    return Path(os.environ.get("TEMP") or os.environ.get("TMP") or tempfile.gettempdir())


def main(argv: list[str] | None = None, alive=pid_alive) -> int:
    ap = argparse.ArgumentParser(prog="clean_temp")
    ap.add_argument("--apply", action="store_true", help="remove what the dry run calls removable")
    ap.add_argument("--older-than-hours", type=float, default=48.0)
    ap.add_argument("--root", type=Path, default=None, help="the folder to clean (default %%TEMP%%)")
    ap.add_argument("--top", type=int, default=30, help="name patterns printed per list")
    ns = ap.parse_args(argv)
    root = default_root() if ns.root is None else ns.root
    found = scan(root, time.time(), ns.older_than_hours, alive=alive)
    print(f"root {root}; older than {ns.older_than_hours:g} h; {'apply' if ns.apply else 'dry run, nothing removed'}")
    for line in summary(found, ns.top):
        print(line)
    if not ns.apply:
        return 0
    lines, removed, removed_bytes = apply(found)
    for line in lines:
        print(line)
    print(f"removed {removed} folders, {human(removed_bytes)}; {len(lines)} skipped or failed")
    return 1 if any(line.startswith("failed") for line in lines) else 0


if __name__ == "__main__":
    sys.exit(main())
