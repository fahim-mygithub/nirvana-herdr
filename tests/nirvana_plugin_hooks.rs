//! Nirvana fork, plan 12 O4b: every hook in the Nirvana Claude plugin's `hooks/hooks.json` names a herdr subcommand
//! that exists, and each runs quiet and fast when there is nothing to say. The built binary runs each hook's own
//! `args` with that event's JSON on stdin, in a scratch repository with a scratch home, as Claude Code would.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const PLACEHOLDER_EXE: &str = "${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe";
/// A hook is on the session's path; a debug build in a busy machine still answers well under this.
const FAST: Duration = Duration::from_secs(10);

fn plugin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("nirvana")
        .join("claude")
}

fn hooks() -> Value {
    let text =
        std::fs::read_to_string(plugin().join("hooks").join("hooks.json")).expect("hooks.json");
    serde_json::from_str(&text).expect("hooks.json is JSON")
}

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("nirvana-o4b-hooks-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for dir in ["proj/.git", "home", "config"] {
            std::fs::create_dir_all(root.join(dir)).expect("scratch");
        }
        Self { root }
    }

    fn run(&self, args: &[String], input: &Value) -> (i32, String, Duration) {
        let started = Instant::now();
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr"))
            .args(args)
            .current_dir(self.root.join("proj"))
            .env("NIRVANA_HERDR_HOME", self.root.join("home"))
            .env("CLAUDE_CONFIG_DIR", self.root.join("config"))
            .env("CLAUDE_PLUGIN_ROOT", plugin())
            .env_remove("NIRVANA_PROJECT")
            .env_remove("OPENROUTER_API_KEY")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("herdr starts");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.to_string().as_bytes())
            .expect("the hook's JSON");
        let out = child.wait_with_output().expect("herdr ends");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            started.elapsed(),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn event(name: &str, scratch: &Scratch) -> Value {
    let cwd = scratch.root.join("proj").display().to_string();
    let mut input = json!({"session_id": "o4b-hooks-test", "cwd": cwd, "hook_event_name": name,
                           "transcript_path": scratch.root.join("none.jsonl").display().to_string()});
    match name {
        "PreToolUse" => {
            input["tool_name"] = "Agent".into();
            input["tool_input"] = json!({"subagent_type": "nirvana:worker-haiku-low", "description": "d",
                                         "prompt": "Class: chore.script\nCount the files."});
        }
        "SubagentStop" => {
            input["agent_type"] = "Explore".into();
            input["agent_id"] = "a1".into();
            input["last_assistant_message"] = "done".into();
        }
        "Stop" => input["stop_hook_active"] = false.into(),
        "SessionStart" => input["source"] = "startup".into(),
        _ => {}
    }
    input
}

#[test]
fn every_hook_names_a_herdr_subcommand_that_runs_quiet_and_fast() {
    let hooks = hooks();
    let events = hooks["hooks"].as_object().expect("an event map");
    let mut names: Vec<&str> = events.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "PreToolUse",
            "SessionStart",
            "Stop",
            "SubagentStop",
            "UserPromptSubmit"
        ]
    );
    assert_eq!(hooks["hooks"]["PreToolUse"][0]["matcher"], "Agent");
    let scratch = Scratch::new("quiet");
    let mut ran = 0;
    for (name, groups) in events {
        for group in groups.as_array().expect("groups") {
            for hook in group["hooks"].as_array().expect("hooks") {
                // Exec form, so no shell re-quotes the path (Claude Code's hooks reference).
                assert_eq!(hook["type"], "command");
                assert_eq!(hook["command"], PLACEHOLDER_EXE, "{name}");
                let args: Vec<String> = hook["args"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{name}: no args"))
                    .iter()
                    .map(|arg| arg.as_str().expect("a string").to_owned())
                    .collect();
                let (code, out, took) = scratch.run(&args, &event(name, &scratch));
                assert_eq!(code, 0, "{name} {args:?} exited {code}: {out}");
                assert!(took < FAST, "{name} {args:?} took {took:?}");
                let quiet = !matches!(
                    args[..2].join(" ").as_str(),
                    "nirvana digest" | "board notice"
                );
                if quiet {
                    assert!(out.trim().is_empty(), "{name} {args:?} said {out:?}");
                }
                ran += 1;
            }
        }
    }
    assert_eq!(ran, 7, "seven hooks");
}

/// A folder whose own settings run `herdr board notice` (Preload's shared board) hears the notice once: the plugin's
/// `--plugin` copy stays quiet there, and speaks where the folder does not run it.
#[test]
fn the_plugins_board_notice_stays_quiet_where_the_folder_runs_its_own() {
    let scratch = Scratch::new("dedupe");
    let args: Vec<String> = ["board", "notice", "--plugin"]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
    let (code, out, _) = scratch.run(&args, &event("UserPromptSubmit", &scratch));
    assert_eq!(code, 0);
    assert!(
        out.contains("shared board"),
        "the plugin's copy speaks alone: {out:?}"
    );
    let claude = scratch.root.join("proj").join(".claude");
    std::fs::create_dir_all(&claude).expect(".claude");
    std::fs::write(
        claude.join("settings.local.json"),
        json!({"hooks": {"UserPromptSubmit": [{"hooks": [{"type": "command",
               "command": "\"C:\\h\\herdr.exe\" board notice", "timeout": 30}]}]}})
        .to_string(),
    )
    .expect("the folder's settings");
    let mut fresh = event("UserPromptSubmit", &scratch);
    fresh["session_id"] = "o4b-hooks-test-2".into();
    let (code, out, _) = scratch.run(&args, &fresh);
    assert_eq!(code, 0);
    assert!(
        out.trim().is_empty(),
        "the folder's own hook prints it: {out:?}"
    );
}

#[test]
fn the_hooks_speak_when_they_have_something_to_say() {
    let scratch = Scratch::new("speak");
    let args = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    let (code, out, _) = scratch.run(
        &args(&["nirvana", "digest"]),
        &event("SessionStart", &scratch),
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("Memory: ") && out.contains("NIRVANA.md"),
        "{out}"
    );
    assert!(out.trim_end().chars().count() <= 540, "{out}");
    // A fork is refused with a PreToolUse decision.
    let mut spawn = event("PreToolUse", &scratch);
    spawn["tool_input"] = json!({"subagent_type": "fork", "prompt": "p", "description": "d"});
    let (code, out, _) = scratch.run(&args(&["nirvana", "guard-spawn"]), &spawn);
    assert_eq!(code, 0);
    let decision: Value = serde_json::from_str(out.trim()).expect("JSON");
    assert_eq!(decision["hookSpecificOutput"]["permissionDecision"], "deny");
    // An unknown subcommand is a usage error, so a hook naming one would show as a failure.
    let (code, _, _) = scratch.run(
        &args(&["nirvana", "no-such-hook"]),
        &event("Stop", &scratch),
    );
    assert_eq!(code, 2);
}
