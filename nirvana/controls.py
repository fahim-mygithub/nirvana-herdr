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
    # Feature 3: each space lists its agents (plans/2026-10-15-03-space-agents.md). Red line for each: its test prints FAIL.
    Control("spaces_rows_hidden", "src/client/shell/sidebar.rs",
            "            &agents,\n            entry.indented,", "            &[],\n            entry.indented,",
            "a_space_lists_its_agents_task_and_model_with_the_agent_icon", "rust"),
    Control("spaces_rect_short", "src/client/shell/sidebar.rs",
            "let row_height = ((space_rows + super::nirvana_spaces::agents_height(agents.len()))",
            "let row_height = ((space_rows + super::nirvana_spaces::agents_height(0))",
            "each_agent_adds_two_rows_to_its_space", "rust"),
    Control("spaces_scroll_short", "src/client/shell/sidebar.rs",
            "                        + super::nirvana_spaces::agents_height(agents.len()))",
            "                        + super::nirvana_spaces::agents_height(0))",
            "spaces_with_agents_make_the_list_scroll", "rust"),
    Control("spaces_no_cap", "src/client/shell/nirvana_spaces.rs",
            "pub(super) const AGENTS_SHOWN: usize = 3;", "pub(super) const AGENTS_SHOWN: usize = 30;",
            "a_space_shows_three_agents_then_how_many_more", "rust"),
    Control("spaces_more_row_hidden", "src/client/shell/nirvana_spaces.rs",
            "if agents.len() > shown && more_y < area.bottom() && width > 0 {",
            "if false && more_y < area.bottom() && width > 0 {",
            "a_space_shows_three_agents_then_how_many_more", "rust"),
    Control("spaces_icon_is_status_mark", "src/nirvana_spaces.rs",
            'Self::Codex => "\\u{2756}",', 'Self::Codex => "\\u{25CB}",',
            "each_kind_has_its_own_one_cell_icon", "rust"),
    Control("spaces_dot_not_icon", "src/client/shell/nirvana_spaces.rs",
            "icon: kind.map_or_else(|| status_icon(agent.agent_status, indicators), Kind::icon),",
            "icon: status_icon(agent.agent_status, indicators),",
            "each_kind_draws_its_own_icon_and_others_keep_the_dot", "rust"),
    Control("spaces_icon_not_status_colour", "src/client/shell/nirvana_spaces.rs",
            ".fg(status_color(agent.status, palette))", ".fg(palette.text)",
            "a_space_lists_its_agents_task_and_model_with_the_agent_icon", "rust"),
    Control("spaces_kind_token_ignored", "src/client/shell/nirvana_spaces.rs",
            "let kind = token(KIND_TOKEN)\n                .and_then(Kind::parse)",
            "let kind = None::<Kind>",
            "each_kind_draws_its_own_icon_and_others_keep_the_dot", "rust"),
    Control("spaces_title_never_cut", "src/client/shell/nirvana_spaces.rs",
            "    if unicode_width::UnicodeWidthStr::width(text) <= width {",
            "    if true {",
            "a_long_title_is_cut_to_the_sidebar_with_an_ellipsis", "rust"),
    Control("spaces_tool_input_model", "src/nirvana_spaces.rs",
            'last_quoted_after(text, "\\"message\\":{\\"model\\":\\"")',
            'last_quoted_after(text, "\\"model\\":\\"")',
            "the_model_comes_from_the_last_reply_not_a_tool_input", "rust"),
    Control("spaces_local_reads_as_claude", "src/nirvana_spaces.rs",
            "            None => AgentModel {\n                kind: Kind::Local,",
            "            None => AgentModel {\n                kind: Kind::Claude,",
            "a_claude_on_a_local_model_is_local", "rust"),
    Control("spaces_date_as_version", "src/nirvana_spaces.rs",
            "if part.len() < 8 {", "if part.len() < 80 {",
            "claude_models_read_as_provider_family_and_version", "rust"),
    Control("spaces_slug_keeps_separators", "src/nirvana_spaces.rs",
            ".map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })",
            ".map(|c| if c.is_ascii_alphanumeric() || c == ':' { c } else { '-' })",
            "the_transcript_folder_is_claude_codes_slug", "rust"),
    Control("spaces_program_title_is_a_task", "src/nirvana_spaces.rs",
            "if program_only || shell_path {", "if false {",
            "a_title_that_only_names_the_program_is_no_task_yet", "rust"),
    Control("spaces_agy_log_unread", "src/nirvana_spaces.rs",
            'last_quoted_after(text, "selected model override to backend: label=\\"")',
            'last_quoted_after(text, "model override label=\\"")',
            "agy_and_codex_files_name_their_models", "rust"),
    Control("spaces_codex_unc_prefix", "src/nirvana_spaces.rs",
            r'Some(PathBuf::from(cwd.strip_prefix(r"\\?\").unwrap_or(cwd)))',
            "Some(PathBuf::from(cwd))",
            "agy_and_codex_files_name_their_models", "rust"),
    Control("spaces_tail_from_start", "src/nirvana_spaces.rs",
            "file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES)))",
            "file.seek(SeekFrom::Start(len.saturating_sub(len)))",
            "the_tail_reader_keeps_the_end_of_a_long_file", "rust"),
    Control("spaces_short_model_flag", "src/nirvana_spaces.rs",
            'if arg == "--model" || arg == "-m" {', 'if arg == "--model" {',
            "model_flags_are_read_from_the_arguments", "rust"),
    Control('servers_dynamic_port_kept', 'src/nirvana_servers.rs',
            'port != 0 && port < DYNAMIC_PORTS_START && ', 'port != 0 && ',
            'plumbing_ports_are_not_servers', "rust"),
    Control('servers_agent_port_kept', 'src/nirvana_servers.rs',
            ' && !owner.is_some_and(is_agent_program)', '',
            'plumbing_ports_are_not_servers', "rust"),
    Control('servers_children_not_walked', 'src/nirvana_servers.rs',
            'queue.extend(children.get(&pid).into_iter().flatten().copied());', 'let _ = &children;',
            'a_server_anywhere_under_the_pane_shell_is_the_spaces', "rust"),
    Control('servers_no_dedup', 'src/nirvana_servers.rs',
            '    ports.dedup();\n', '',
            'a_server_on_both_address_families_is_one_port', "rust"),
    Control('servers_dead_space_kept', 'src/nirvana_servers.rs',
            '.any(|process| process.pid != 0 && shell_pids.contains(&process.pid))', '.any(|process| process.pid != 0)',
            'a_space_is_read_while_a_shell_lives', "rust"),
    Control('usage_claude_wrong_week', 'src/nirvana_usage.rs',
            'percent_after(text, "Current week (all models):")', 'percent_after(text, "Current week (Fable):")',
            'claude_reads_the_session_and_the_all_models_week', "rust"),
    Control('usage_claude_under_one_is_one', 'src/nirvana_usage.rs',
            "Some(if rest.starts_with('<') {", 'Some(if false {',
            'claude_reads_the_session_and_the_all_models_week', "rust"),
    Control('usage_agy_remaining_as_used', 'src/nirvana_usage.rs',
            'Some(percent((1.0 - remaining.clamp(0.0, 1.0)) * 100.0))', 'Some(percent(remaining.clamp(0.0, 1.0) * 100.0))',
            'antigravity_reads_the_gemini_group', "rust"),
    Control('usage_agy_no_stand_in', 'src/nirvana_usage.rs',
            'id.starts_with("gemini-")', 'id.starts_with("nothing-")',
            'antigravity_reads_the_gemini_group', "rust"),
    Control('usage_codex_reset_kept', 'src/nirvana_usage.rs',
            'let figure = if live { percent(used) } else { 0 };', 'let figure = percent(used);',
            'codex_reads_its_windows_and_drops_reset_ones', "rust"),
    Control('usage_codex_stale_shown', 'src/nirvana_usage.rs',
            '(any_live && (five_hour.is_some()', '(true && (five_hour.is_some()',
            'codex_reads_its_windows_and_drops_reset_ones', "rust"),
    Control('usage_failed_read_wipes', 'src/nirvana_usage.rs',
            '(None, _) => return,', '(None, _) => Figure::Word("not exposed"),',
            'a_failed_read_keeps_the_last_figure', "rust"),
    Control('usage_bar_scale', 'src/nirvana_usage.rs',
            '} => Some(f32::from(*used) / 100.0),', '} => Some(f32::from(*used) / 50.0),',
            'the_bar_follows_the_five_hour_window', "rust"),
    Control('footer_servers_head_always', 'src/client/shell/nirvana_footer.rs',
            'if !servers.is_empty() && y < area.bottom() {', 'if y < area.bottom() {',
            'no_server_hides_the_servers_block', "rust"),
    Control('footer_no_cap', 'src/client/shell/nirvana_footer.rs',
            'servers.iter().take(SERVERS_SHOWN)', 'servers.iter()',
            'four_servers_then_how_many_more', "rust"),
    Control('footer_more_row_hidden', 'src/client/shell/nirvana_footer.rs',
            'if servers.len() > SERVERS_SHOWN && y < area.bottom() {', 'if false && y < area.bottom() {',
            'four_servers_then_how_many_more', "rust"),
    Control('footer_agents_starved', 'src/client/shell/nirvana_footer.rs',
            'if detail.height < foot.saturating_add(AGENTS_KEEP) {', 'if detail.height < foot {',
            'a_short_sidebar_keeps_the_agents_panel', "rust"),
    Control('footer_not_at_the_bottom', 'src/client/shell/nirvana_footer.rs',
            'Rect::new(detail.x, detail.y + agents, detail.width, foot),', 'Rect::new(detail.x, detail.y + agents - 1, detail.width, foot),',
            'the_foot_lists_a_spaces_server_and_the_four_usage_rows', "rust"),
    Control('footer_address_over_name', 'src/client/shell/nirvana_footer.rs',
            'if name_width + 1 + long.len() <= room {', 'if long.len() <= room {',
            'the_foot_lists_a_spaces_server_and_the_four_usage_rows', "rust"),
    Control('footer_server_not_clickable', 'src/client/shell/nirvana_footer.rs',
            'hits.push((Rect::new(area.x, y, area.width, 1), server.port));', 'let _ = &hits;',
            'a_click_on_a_server_row_opens_its_address', "rust"),
    Control('footer_no_brand_colour', 'src/client/shell/nirvana_footer.rs',
            '.map_or(palette.overlay1, |(r, g, b)| Color::Rgb(r, g, b));', '.map_or(palette.overlay1, |_| palette.overlay1);',
            'each_provider_dot_has_its_colour', "rust"),
    Control('footer_no_bar', 'src/client/shell/nirvana_footer.rs',
            'let bar = area.width >= BAR_FROM_WIDTH;', 'let bar = false;',
            'a_wide_sidebar_draws_a_bar_for_the_five_hour_window', "rust"),
    Control('footer_no_dashed_track', 'src/client/shell/nirvana_footer.rs',
            'let track = if row.figure.fill().is_some() {', 'let track = if true {',
            'a_wide_sidebar_draws_a_bar_for_the_five_hour_window', "rust"),
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
