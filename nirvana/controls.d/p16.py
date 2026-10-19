# Plan 16: the Git square, old Nirvana's GitHub card in the strip (plans/2026-10-19-16-git.md).
# Red line for each: its test prints FAIL.
CONTROLS = (
    # §2: Porcelain v2 dirty files increment changed count.
    Control(
        'p16_porcelain_v2_dirty_count',
        'src/nirvana_git.rs',
        "        } else if !line.is_empty() && !line.starts_with('#') {\n            status.dirty += 1;\n",
        "        } else if !line.is_empty() && !line.starts_with('#') {\n            // status.dirty += 1;\n",
        'porcelain_v2_dirty_files',
        'rust',
    ),
    # §2: Porcelain v2 rename shows new path.
    Control(
        'p16_porcelain_v2_rename_shows_new_path',
        'src/nirvana_git.rs',
        '    } else if line.starts_with("2 ") {\n        let field = line.splitn(10, \' \').nth(9).unwrap_or("");\n        field.split(\'\\t\').next().unwrap_or(field)\n',
        '    } else if line.starts_with("2 ") {\n        let field = line.splitn(10, \' \').nth(9).unwrap_or("");\n        field.split(\'\\t\').nth(1).unwrap_or(field)\n',
        'porcelain_v2_rename_shows_new_path',
        'rust',
    ),
    # §2: Git square narrow label format omits "changed" word.
    Control(
        'p16_label_narrow_omits_changed_word',
        'src/nirvana_git.rs',
        '        if narrow {\n            label.push_str(&format!("{}", status.dirty));\n        } else {\n            label.push_str(&format!("{} changed", status.dirty));\n        }\n',
        '        if false {\n            label.push_str(&format!("{}", status.dirty));\n        } else {\n            label.push_str(&format!("{} changed", status.dirty));\n        }\n',
        'git_label_wide_and_narrow_formats',
        'rust',
    ),
    # §2: Clean tree formats branch name only without zeros.
    Control(
        'p16_label_clean_tree_branch_only',
        'src/nirvana_git.rs',
        '    let mut label = if status.branch.is_empty() {\n        "(detached)".to_string()\n    } else {\n        status.branch.clone()\n    };\n',
        '    let mut label = if status.branch.is_empty() {\n        "(detached)".to_string()\n    } else {\n        format!("{}-branch", status.branch)\n    };\n',
        'git_label_wide_and_narrow_formats',
        'rust',
    ),
    # §2: Outside git repo square is not drawn.
    Control(
        'p16_git_square_not_drawn_outside_repo',
        'src/client/shell/composition.rs',
        '                // Plan 16: the Git square.\n                let git = super::nirvana_git::git_tab(snapshot)\n                    .and_then(|tab| crate::nirvana_git::cached(&tab.folder));\n',
        '                // Plan 16: the Git square.\n                let git = super::nirvana_git::git_tab(snapshot)\n                    .and_then(|tab| crate::nirvana_git::cached(&tab.folder).or_else(|| Some(crate::nirvana_git::GitView::default())));\n',
        'the_git_square_label_narrow_and_wide_forms_and_outside_repo',
        'rust',
    ),
    # §3: expect_branch mismatch refuses action.
    Control(
        'p16_expect_branch_refusal',
        'src/nirvana_git.rs',
        '    if expect_branch != status.branch {\n        return Err(format!(\n',
        '    if false && expect_branch != status.branch {\n        return Err(format!(\n',
        'expect_branch_refusal',
        'rust',
    ),
    # §3: Action refuses when branch has no upstream.
    Control(
        'p16_pull_action_refuses_no_upstream',
        'src/nirvana_git.rs',
        '    if status.upstream.is_empty() {\n        return Err(format!(\n            "git {verb}: refused. {} has no upstream, so there is no remote to {verb} {}. git branch --set-upstream-to sets one.",\n',
        '    if false && status.upstream.is_empty() {\n        return Err(format!(\n            "git {verb}: refused. {} has no upstream, so there is no remote to {verb} {}. git branch --set-upstream-to sets one.",\n',
        'expect_branch_refusal',
        'rust',
    ),
    # §3: Click on Git square opens panel.
    Control(
        'p16_git_square_click_opens_panel',
        'src/client/shell/mouse.rs',
        "                // Plan 16: the strip's Git square opens its panel.\n                if super::contains(self.hits.nirvana_git, point) {\n",
        "                // Plan 16: the strip's Git square opens its panel.\n                if false && super::contains(self.hits.nirvana_git, point) {\n",
        'git_panel_renders_rows_and_buttons',
        'rust',
    ),
    # §3: Push button disabled when nothing ahead.
    Control(
        'p16_git_push_disabled_when_nothing_ahead',
        'src/client/shell/nirvana_git.rs',
        '                if status.ahead == 0 {\n                    panel.message = Some("git push: refused. Nothing ahead to push.".to_string());\n                    return;\n                }\n',
        '                if false && status.ahead == 0 {\n                    panel.message = Some("git push: refused. Nothing ahead to push.".to_string());\n                    return;\n                }\n',
        'disabled_button_reasons_on_status_line',
        'rust',
    ),
    # §3: Confirm step asks before Pull action.
    Control(
        'p16_git_confirm_step_required',
        'src/client/shell/nirvana_git.rs',
        '                if panel.confirming != Some(GitConfirm::Pull) {\n                    panel.confirming = Some(GitConfirm::Pull);\n',
        '                if false && panel.confirming != Some(GitConfirm::Pull) {\n                    panel.confirming = Some(GitConfirm::Pull);\n',
        'confirm_step_and_esc_or_cancel_closes',
        'rust',
    ),
    # §3: Esc cancels confirmation before closing panel.
    Control(
        'p16_git_panel_cancel_clears_confirm',
        'src/client/shell/nirvana_git.rs',
        "            KeyCode::Esc | KeyCode::Char('q') => {\n                if let Some(panel) = self.nirvana_git.as_mut() {\n                    if panel.confirming.is_some() {\n",
        "            KeyCode::Esc | KeyCode::Char('q') => {\n                if let Some(panel) = self.nirvana_git.as_mut() {\n                    if false && panel.confirming.is_some() {\n",
        'confirm_step_and_esc_or_cancel_closes',
        'rust',
    ),
    # §4: CI state parsing running when not completed.
    Control(
        'p16_ci_parse_running_state',
        'src/nirvana_git.rs',
        '    let (state, conclusion_word) = if first.status != "completed" {\n        (CiState::Running, "running".to_string())\n',
        '    let (state, conclusion_word) = if first.status == "completed" {\n        (CiState::Running, "running".to_string())\n',
        'ci_parse_cases',
        'rust',
    ),
    # §4: CI probe only checks github.com remotes.
    Control(
        'p16_ci_probe_github_remote_check',
        'src/nirvana_git.rs',
        'pub fn is_github_remote_url(url: &str) -> bool {\n    url.contains("github.com")\n}\n',
        'pub fn is_github_remote_url(url: &str) -> bool {\n    false && url.contains("github.com")\n}\n',
        'ci_probe_rules',
        'rust',
    ),
    # §4: CI probe env disables prompts.
    Control(
        'p16_ci_env_gh_prompt_disabled',
        'src/nirvana_git.rs',
        '        ("GH_PROMPT_DISABLED", "1"),\n',
        '        ("GH_PROMPT_DISABLED", "0"),\n',
        'ci_probe_rules',
        'rust',
    ),
    # §4: CI panel row text formatting.
    Control(
        'p16_ci_row_text_format',
        'src/nirvana_git.rs',
        '    format!(\n        "CI  {}  {}  {} ago",\n        ci.workflow_name, ci.conclusion_word, ago\n    )\n',
        '    format!(\n        "CI: {} {} {} ago",\n        ci.workflow_name, ci.conclusion_word, ago\n    )\n',
        'ci_row_text_formatting',
        'rust',
    ),
)
