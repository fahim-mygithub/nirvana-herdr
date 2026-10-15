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
    # Feature 2: Restart Claude (plans/2026-10-15-02-restart-claude.md). Red line for each: its test prints FAIL.
    Control("restart_menu_hidden", "src/client/shell/context_menu.rs",
            "if *has_claude {", "if false && *has_claude {",
            "a_tab_running_claude_offers_restart_claude", "rust"),
    Control("restart_menu_any_agent", "src/client/shell/context_menu.rs",
            'agent.agent.as_deref() == Some("claude")', "agent.agent.is_some()",
            "a_tab_running_claude_offers_restart_claude", "rust"),
    Control("restart_wrong_command", "src/client/shell/context_menu.rs",
            "command_id: crate::nirvana::RESTART_CLAUDE_COMMAND.into(),", 'command_id: "restart".into(),',
            "restart_claude_asks_the_server_for_that_tab", "rust"),
    Control("restart_not_routed", "src/app/custom_commands.rs",
            "if params.command_id == crate::nirvana::RESTART_CLAUDE_COMMAND {", "if false {",
            "restart_claude_reaches_the_server_and_stops_nothing_without_claude", "rust"),
    Control("restart_drops_flags", "src/nirvana.rs",
            "_ => args.push(arg.clone()),", "_ => {}",
            "restart_keeps_the_flags_and_resumes_the_same_conversation", "rust"),
    Control("restart_keeps_old_resume", "src/nirvana.rs",
            '"--resume" | "-r" | "--session-id" => {', '"--session-id" => {',
            "restart_keeps_the_flags_and_resumes_the_same_conversation", "rust"),
    Control("restart_no_continue_fallback", "src/nirvana.rs",
            'None => args.push("--continue".to_string()),', "None => {}",
            "restart_keeps_the_flags_and_resumes_the_same_conversation", "rust"),
    Control("terminate_query_only", "src/nirvana.rs",
            "let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);",
            "let handle = OpenProcess(windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);",
            "terminate_really_stops_a_running_process", "rust"),
    Control("restart_bracketed_paste", "src/nirvana.rs",
            "let mut bytes = command.as_bytes().to_vec();",
            'let mut bytes = format!("\x1b[200~{command}\x1b[201~").into_bytes();',
            "the_restart_command_is_typed_as_a_plain_line", "rust"),
    Control("session_stale_pid_accepted", "src/nirvana.rs",
            "if value.get(\"pid\").and_then(serde_json::Value::as_u64) != Some(u64::from(pid)) {", "if false {",
            "a_session_file_left_by_another_process_is_not_used", "rust"),
    Control("session_id_unchecked", "src/nirvana.rs",
            "let id_ok = !session_id.is_empty()", "let id_ok = true || !session_id.is_empty()",
            "a_session_file_left_by_another_process_is_not_used", "rust"),
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
