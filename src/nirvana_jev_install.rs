//! Nirvana fork, feature 9: putting Jev in front of each agent in a folder, from Preload's two switches.
//!
//! - **Jev memory** tells every agent started in the folder how to ask Jev (`nirvana_jev_memory`). Claude
//!   Code gets it from a `SessionStart` hook in the folder's `.claude/settings.local.json`, whose output
//!   Claude Code adds to the session's context; the other agents get it as described in `others`.
//! - **Compact advice** is level 7 for Claude Code: a `Stop` hook running `herdr jev compact`.
//!
//! Every hook is herdr's own line, found again by its command's ending, so a switch adds or removes only
//! that line and every other setting and hook stays as it was.

use std::path::Path;

use serde_json::{json, Map, Value};

/// `--hook`: the guide reads the hook's folder and stays quiet when Claude reads it from AGENTS.md.
pub const GUIDE: &str = "jev guide --hook";
pub const COMPACT: &str = "jev compact";
pub const GUIDE_EVENT: &str = "SessionStart";
pub const COMPACT_EVENT: &str = "Stop";

fn entries<'a>(local: &'a Value, event: &str) -> impl Iterator<Item = &'a Value> {
    local
        .pointer(&format!("/hooks/{event}"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|group| group.get("hooks").and_then(Value::as_array))
        .flatten()
}

fn is_ours(hook: &Value, tail: &str) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.trim_end().ends_with(tail))
}

/// Whether the folder's local settings run herdr's `tail` command on `event`.
pub fn has_hook(local: &Value, event: &str, tail: &str) -> bool {
    entries(local, event).any(|hook| is_ours(hook, tail))
}

/// Adds or removes herdr's `tail` hook on `event`. Every other hook stays.
pub fn set_hook(local: &mut Value, event: &str, tail: &str, exe: &Path, on: bool) {
    if !local.is_object() {
        *local = Value::Object(Map::new());
    }
    let root = local.as_object_mut().expect("an object");
    if on {
        if has_hook(&Value::Object(root.clone()), event, tail) {
            return;
        }
        let hooks = root
            .entry("hooks")
            .or_insert_with(|| Value::Object(Map::new()));
        if !hooks.is_object() {
            *hooks = Value::Object(Map::new());
        }
        let list = hooks
            .as_object_mut()
            .expect("an object")
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()));
        if !list.is_array() {
            *list = Value::Array(Vec::new());
        }
        list.as_array_mut().expect("an array").push(json!({
            "hooks": [{"type": "command", "command": format!("\"{}\" {tail}", exe.display()), "timeout": 30}]
        }));
        return;
    }
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(list) = hooks.get_mut(event).and_then(Value::as_array_mut) {
        for group in list.iter_mut() {
            if let Some(inner) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                inner.retain(|hook| !is_ours(hook, tail));
            }
        }
        list.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|inner| !inner.is_empty())
        });
        if list.is_empty() {
            hooks.remove(event);
        }
    }
    if hooks.is_empty() {
        root.remove("hooks");
    }
}

/// Marks herdr's block in a project's `AGENTS.md`, so a switch finds it again and removes only it.
pub const BLOCK_START: &str = "<!-- herdr: Jev guide, switched in Preload; herdr removes this block when it is switched off -->";
pub const BLOCK_END: &str = "<!-- herdr: end of Jev guide -->";
const EXCLUDE_LINE: &str = "/AGENTS.md";

/// Codex and agy get the guide from the project's `AGENTS.md`: both load it from the repository root
/// (measured 2026-10-16, leaf "Sonnet - agent instruction surfaces": Codex 0.42.0 recorded the root and a
/// sub folder's `AGENTS.md` in its session's instructions and nothing else, no override file, no import,
/// no hook; agy 1.4.0 reported a code word from `AGENTS.md`). Claude Code never loads `AGENTS.md`, so
/// no agent reads the guide twice.
///
/// A project with no `AGENTS.md` gets one holding only the block, kept out of git through the
/// repository's own `.git/info/exclude`, which is never committed; switching off deletes both. A project
/// with its own `AGENTS.md` gets the block appended: git shows that file changed until it is switched off.
pub fn others(cwd: &Path, on: bool) -> Result<(), String> {
    let root = crate::nirvana_board::root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let path = root.join("AGENTS.md");
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(format!("{}: {err}", path.display())),
    };
    // Written at the switch: the project's size in it is the size then.
    let block = format!(
        "{BLOCK_START}\n{}{BLOCK_END}\n",
        crate::nirvana_jev_memory::for_folder(&root, &crate::nirvana_board::exe())
    );
    let (text, created) = agents_md(existing.as_deref(), &block, on);
    let write = |text: &str| {
        std::fs::write(&path, text).map_err(|err| format!("{}: {err}", path.display()))
    };
    match text {
        Some(text) => write(&text)?,
        None if path.exists() => {
            std::fs::remove_file(&path).map_err(|err| format!("{}: {err}", path.display()))?
        }
        None => {}
    }
    if let Some(exclude) = info_exclude(&root) {
        // Only a file herdr made is excluded, and the line is taken back only with that file.
        if created {
            set_exclude(&exclude, true)?;
        } else if !on && existing.as_deref().is_some_and(only_ours) {
            set_exclude(&exclude, false)?;
        }
    }
    Ok(())
}

fn only_ours(text: &str) -> bool {
    text.contains(BLOCK_START) && strip(text).trim().is_empty()
}

fn strip(text: &str) -> String {
    match (text.find(BLOCK_START), text.find(BLOCK_END)) {
        (Some(start), Some(end)) if end > start => {
            let after = &text[end + BLOCK_END.len()..];
            let after = after.strip_prefix('\n').unwrap_or(after);
            let before = text[..start].trim_end_matches('\n');
            if before.is_empty() {
                after.to_owned()
            } else if after.is_empty() {
                format!("{before}\n")
            } else {
                format!("{before}\n\n{after}")
            }
        }
        _ => text.to_owned(),
    }
}

/// The new text of `AGENTS.md` (`None`: no file) and whether herdr's block is the whole of it, so the
/// file is herdr's to keep out of git.
pub fn agents_md(existing: Option<&str>, block: &str, on: bool) -> (Option<String>, bool) {
    let rest = existing.map(strip);
    let theirs = rest.as_deref().filter(|r| !r.trim().is_empty());
    match (on, theirs) {
        (true, None) => (Some(block.to_owned()), true),
        (true, Some(theirs)) => (
            Some(format!("{}\n\n{block}", theirs.trim_end_matches('\n'))),
            false,
        ),
        (false, None) => (None, false),
        (false, Some(theirs)) => (Some(theirs.to_owned()), false),
    }
}

/// The repository's own exclude file: `.git/info/exclude`, or the common git folder's for a worktree.
fn info_exclude(root: &Path) -> Option<std::path::PathBuf> {
    let git = root.join(".git");
    let dir = if git.is_dir() {
        git
    } else {
        let text = std::fs::read_to_string(&git).ok()?;
        let gitdir = root.join(text.trim().strip_prefix("gitdir:")?.trim());
        match std::fs::read_to_string(gitdir.join("commondir")) {
            Ok(common) => gitdir.join(common.trim()),
            Err(_) => gitdir,
        }
    };
    Some(dir.join("info").join("exclude"))
}

fn set_exclude(path: &Path, on: bool) -> Result<(), String> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let has = text.lines().any(|line| line.trim() == EXCLUDE_LINE);
    let new = if on && !has {
        let mut text = text.clone();
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text + EXCLUDE_LINE + "\n"
    } else if !on && has {
        text.lines()
            .filter(|line| line.trim() != EXCLUDE_LINE)
            .map(|line| format!("{line}\n"))
            .collect()
    } else {
        return Ok(());
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    std::fs::write(path, new).map_err(|err| format!("{}: {err}", path.display()))
}

/// Whether Claude Code reads the project's `AGENTS.md`, herdr's block in it, in this folder: it does
/// only when no CLAUDE.md stands beside it. Measured 2026-10-16 with `/context` in a trusted folder on
/// Claude Code 2.1.301: `AGENTS.md` alone loaded as a Project memory file (469 tokens); with `CLAUDE.md`,
/// or with `CLAUDE.local.md` alone, it did not. `.claude/CLAUDE.md` is counted as one too, untested: if
/// that is wrong the guide arrives twice, never not at all.
pub fn claude_reads_agents_block(folder: &Path) -> bool {
    let root = crate::nirvana_board::root(folder).unwrap_or_else(|| folder.to_path_buf());
    let has_ours = std::fs::read_to_string(root.join("AGENTS.md"))
        .is_ok_and(|text| text.contains(BLOCK_START));
    let claude_md = [
        root.join("CLAUDE.md"),
        root.join("CLAUDE.local.md"),
        root.join(".claude").join("CLAUDE.md"),
    ];
    has_ours && !claude_md.iter().any(|path| path.is_file())
}

/// The guide's size in tokens, as Preload shows it. Claude Code counted herdr's AGENTS.md block, the
/// guide and its two marker lines, at 469 tokens for 1,150 characters (2026-10-16): about 2.5 characters
/// a token, paths and code being dear.
pub fn guide_tokens(folder: &Path) -> u32 {
    let text = crate::nirvana_jev_memory::for_folder(folder, &crate::nirvana_board::exe());
    u32::try_from((text.chars().count() * 2).div_ceil(5)).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_is_added_once_and_removed_alone() {
        let exe = Path::new(r"C:\h\bin\herdr.exe");
        let mut local = json!({"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine.ps1"}]}]}});
        set_hook(&mut local, COMPACT_EVENT, COMPACT, exe, true);
        set_hook(&mut local, COMPACT_EVENT, COMPACT, exe, true);
        assert!(has_hook(&local, COMPACT_EVENT, COMPACT));
        assert_eq!(local["hooks"]["Stop"].as_array().map(Vec::len), Some(2));
        assert!(!has_hook(&local, GUIDE_EVENT, GUIDE));
        set_hook(&mut local, COMPACT_EVENT, COMPACT, exe, false);
        assert_eq!(
            local,
            json!({"model": "opus", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine.ps1"}]}]}})
        );
        let mut empty = json!({});
        set_hook(&mut empty, GUIDE_EVENT, GUIDE, exe, true);
        set_hook(&mut empty, GUIDE_EVENT, GUIDE, exe, false);
        assert_eq!(empty, json!({}));
    }

    #[test]
    fn agents_md_gets_the_block_and_gives_it_back() {
        let block = format!("{BLOCK_START}\nguide\n{BLOCK_END}\n");
        assert_eq!(agents_md(None, &block, true), (Some(block.clone()), true));
        assert_eq!(agents_md(Some(&block), &block, false), (None, false));
        let theirs = "# Project rules\nUse tabs.\n";
        let (with, created) = agents_md(Some(theirs), &block, true);
        let with = with.unwrap();
        assert!(!created && with.starts_with(theirs.trim_end()) && with.ends_with(&block));
        assert_eq!(
            agents_md(Some(&with), &block, true).0.unwrap(),
            with,
            "switching on twice adds one block"
        );
        assert_eq!(agents_md(Some(&with), &block, false).0.unwrap(), theirs);
    }

    #[test]
    fn a_file_herdr_made_is_kept_out_of_git_and_let_back_in() {
        let dir = std::env::temp_dir().join(format!("nirvana-jev-agents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git").join("info")).unwrap();
        std::fs::write(dir.join(".git/info/exclude"), "# local excludes\n").unwrap();
        others(&dir, true).unwrap();
        let agents = std::fs::read_to_string(dir.join("AGENTS.md")).unwrap();
        // An empty folder is small: the block is the one-line flag.
        assert!(agents.contains("jev file|files|text") && agents.starts_with(BLOCK_START));
        let exclude = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude, "# local excludes\n/AGENTS.md\n");
        others(&dir, false).unwrap();
        assert!(!dir.join("AGENTS.md").exists());
        let exclude = std::fs::read_to_string(dir.join(".git/info/exclude")).unwrap();
        assert_eq!(exclude, "# local excludes\n");
    }

    #[test]
    fn claude_gets_the_guide_once_when_it_reads_agents_md() {
        let dir = std::env::temp_dir().join(format!("nirvana-jev-once-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert!(
            !claude_reads_agents_block(&dir),
            "no AGENTS.md: the hook speaks"
        );
        others(&dir, true).unwrap();
        assert!(
            claude_reads_agents_block(&dir),
            "AGENTS.md and no CLAUDE.md: Claude reads the block"
        );
        for name in ["CLAUDE.md", "CLAUDE.local.md"] {
            std::fs::write(dir.join(name), "# rules\n").unwrap();
            assert!(
                !claude_reads_agents_block(&dir),
                "{name} stops Claude reading AGENTS.md"
            );
            std::fs::remove_file(dir.join(name)).unwrap();
        }
        std::fs::write(dir.join("AGENTS.md"), "# theirs, no block\n").unwrap();
        assert!(
            !claude_reads_agents_block(&dir),
            "an AGENTS.md without herdr's block"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_agents_md_keeps_its_text_and_stays_in_git() {
        let dir =
            std::env::temp_dir().join(format!("nirvana-jev-agents-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".git").join("info")).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "# Ours\n").unwrap();
        others(&dir, true).unwrap();
        assert!(std::fs::read_to_string(dir.join("AGENTS.md"))
            .unwrap()
            .contains("jev file|files|text"));
        assert!(!std::fs::read_to_string(dir.join(".git/info/exclude"))
            .unwrap_or_default()
            .contains("/AGENTS.md"));
        others(&dir, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("AGENTS.md")).unwrap(),
            "# Ours\n"
        );
    }
}
