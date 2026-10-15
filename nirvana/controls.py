"""Sabotage controls for the fork's own tests (Nirvana CLAUDE.md rule 10: every assertion ships with a control,
restored in a wrapper that runs even if the command dies, with the expected red line written down before the run).

A control is one byte-for-byte edit to one file, the one test it must redden, and the line the runner must print.
The file is restored in a `finally`. Rust controls recompile the crate (about 30 s each).

    python nirvana/controls.py --dry-run
    python nirvana/controls.py
    python nirvana/controls.py --only reload_single_press
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TIMEOUT = 900


@dataclass(frozen=True)
class Control:
    name: str
    file: str
    find: str
    replace: str
    test: str        # a nextest filter (rust) or a unittest id (python)
    kind: str        # "rust" or "python"

    @property
    def red_line(self) -> str:
        if self.kind == "rust":
            return f"FAIL "           # nextest prints "FAIL [ time] (1/1) herdr::bin/herdr <test>"
        return f"FAIL: {self.test.rsplit('.', 1)[-1]}"


CONTROLS: tuple[Control, ...] = (
    Control("header_drops_version", "src/client/shell/nirvana_header.rs",
            'format!(" {} ", crate::nirvana::label(crate::nirvana::VERSION)),', '" Nirvana ".to_string(),',
            "brand_row_names_nirvana_and_its_running_version", "rust"),
    Control("header_reload_unclickable", "src/client/shell/nirvana_header.rs",
            "if update_available.is_some() && end > start {", "if false && end > start {",
            "a_newer_build_turns_the_badge_into_a_reload_button", "rust"),
    Control("header_badge_clipped", "src/client/shell/nirvana_header.rs",
            "let right = toggle_x;", "let right = toggle_x.saturating_sub(1);",
            "brand_row_names_nirvana_and_its_running_version", "rust"),
    Control("reload_single_press", "src/nirvana.rs",
            "if RELOAD_ARMED.swap(false, Ordering::Relaxed) {", "if true {",
            "reload_takes_two_presses", "rust"),
    Control("same_build_counts_as_newer", "src/nirvana.rs",
            "(Some(running), Some(installed)) => installed > running,",
            "(Some(running), Some(installed)) => installed >= running,",
            "only_a_later_build_is_newer", "rust"),
    Control("version_not_from_file", "build.rs",
            'println!("cargo:rustc-env=NIRVANA_VERSION={}", nirvana_version.trim());',
            'println!("cargo:rustc-env=NIRVANA_VERSION=9.9.9{}", nirvana_version.trim().len());',
            "version_comes_from_the_version_file", "rust"),
    Control("title_upstream_default", "src/config/window_title.rs",
            '    "{workspace} · Nirvana".to_string()\n', '    "{hostname}: {workspace}".to_string()\n',
            "nirvana_default_title_names_the_project_then_nirvana", "rust"),
    Control("install_stamps_wrong_version", "nirvana/install.py",
            'tmp.write_bytes((ver + "\\n").encode("utf-8"))', 'tmp.write_bytes(b"0.0.0\\n")',
            "nirvana.test_scripts.InstallTest.test_install_moves_the_old_build_aside_and_stamps_last", "python"),
    Control("install_overwrites_old_build", "nirvana/install.py",
            "    if target.exists():\n        target.rename(",
            "    if False:\n        target.rename(",
            "nirvana.test_scripts.InstallTest.test_install_moves_the_old_build_aside_and_stamps_last", "python"),
    Control("bump_feature_keeps_fix", "nirvana/version.py",
            'return f"{major}.{feature + 1}.0"', 'return f"{major}.{feature + 1}.{fix}"',
            "nirvana.test_scripts.VersionTest.test_bump", "python"),
)


def command(c: Control) -> list[str]:
    if c.kind == "rust":
        return ["cargo", "nextest", "run", "--locked", "--no-fail-fast", c.test]
    return [sys.executable, "-m", "unittest", c.test]


def run_one(c: Control) -> tuple[bool, str]:
    path = ROOT / c.file
    original = path.read_bytes()
    find, replace = c.find.encode("utf-8"), c.replace.encode("utf-8")
    if original.count(find) != 1:
        return False, f"find text occurs {original.count(find)} times, not once"
    path.write_bytes(original.replace(find, replace))
    try:
        p = subprocess.run(command(c), cwd=str(ROOT), capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=TIMEOUT)
        lines = (p.stdout + "\n" + p.stderr).splitlines()
        if c.kind == "rust":
            seen = any(c.red_line in line and c.test in line for line in lines)
        else:
            seen = any(line.startswith(c.red_line) for line in lines)
        tail = [line for line in lines if line.strip()][-1:] if lines else []
        return seen, "red line seen" if seen else f"red line not seen; exit {p.returncode}; tail: {tail}"
    except subprocess.TimeoutExpired:
        return False, f"timed out after {TIMEOUT}s"
    finally:
        path.write_bytes(original)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="controls")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--only")
    ns = ap.parse_args(argv)
    chosen = [c for c in CONTROLS if ns.only is None or c.name == ns.only.strip()]
    if not chosen:
        print(f"unknown control {ns.only!r}; names: {', '.join(c.name for c in CONTROLS)}")
        return 2
    failed = 0
    for c in chosen:
        if ns.dry_run:
            print(f"PLAN  {c.name}: {c.file} {c.find!r} -> {c.replace!r}; expect {c.red_line!r} for {c.test}")
            continue
        ok, why = run_one(c)
        restored = (ROOT / c.file).read_bytes().count(c.find.encode("utf-8")) == 1
        failed += (not ok) or (not restored)
        print(f"{'PASS' if ok and restored else 'FAIL'}  {c.name}: {why}{'' if restored else '; NOT RESTORED'}",
              flush=True)
    if ns.dry_run:
        return 0
    print(f"controls: {len(chosen) - failed} of {len(chosen)} reddened their test")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
