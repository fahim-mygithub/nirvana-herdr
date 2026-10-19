//! Plan 13 P3: the session record. Every test uses a scratch home; nothing touches the owner's install.

use super::*;
use serde_json::json;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nirvana-session-record-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

#[cfg(windows)]
const TRANSCRIPT: &str =
    r"C:\Users\o\.claude\projects\C--work-proj\5a1743f9-865e-4cf5-add9-6e27cab86194.jsonl";
#[cfg(not(windows))]
const TRANSCRIPT: &str =
    "/home/o/.claude/projects/-work-proj/5a1743f9-865e-4cf5-add9-6e27cab86194.jsonl";

fn hook(extra: Value) -> Value {
    let mut base = json!({
        "session_id": "5a1743f9-865e-4cf5-add9-6e27cab86194",
        "transcript_path": TRANSCRIPT,
        "cwd": "C:/work/proj",
        "hook_event_name": "SessionStart",
    });
    for (key, value) in extra.as_object().cloned().unwrap_or_default() {
        base[key] = value;
    }
    base
}

#[test]
fn the_record_is_the_fixed_format_under_the_folders_hash() {
    let home = scratch("format");
    let folder = Path::new("C:/work/proj");
    let written = handle(
        &home,
        &hook(json!({})),
        Some("C:/work/proj"),
        "2026-10-18T10:00:00Z",
    )
    .expect("written");
    assert_eq!(written, path_in(&home, folder));
    assert!(written.starts_with(home.join("nirvana").join("sessions")));
    assert_eq!(
        written.file_name().and_then(|n| n.to_str()),
        Some(format!("{}.json", crate::nirvana_launch::folder_hash(folder)).as_str())
    );
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(&written).unwrap()).unwrap();
    assert_eq!(
        raw.as_object().unwrap().len(),
        3,
        "exactly session_id, at and transcript"
    );
    assert_eq!(raw["session_id"], "5a1743f9-865e-4cf5-add9-6e27cab86194");
    assert_eq!(raw["at"], "2026-10-18T10:00:00Z");
    assert_eq!(raw["transcript"], TRANSCRIPT);
    assert_eq!(
        read(&home, folder).map(|r| r.session_id),
        Some("5a1743f9-865e-4cf5-add9-6e27cab86194".into())
    );
}

#[test]
fn stop_updates_the_time_and_keeps_one_file() {
    let home = scratch("stop");
    handle(
        &home,
        &hook(json!({})),
        Some("C:/work/proj"),
        "2026-10-18T10:00:00Z",
    )
    .expect("start");
    handle(
        &home,
        &hook(json!({"hook_event_name": "Stop"})),
        Some("C:/work/proj"),
        "2026-10-18T11:30:00Z",
    )
    .expect("stop");
    let dir = home.join("nirvana").join("sessions");
    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(
        files.len(),
        1,
        "one record per folder, and no temporary file left"
    );
    assert_eq!(
        read(&home, Path::new("C:/work/proj")).unwrap().at,
        "2026-10-18T11:30:00Z"
    );
}

#[test]
fn a_session_that_changed_directory_is_still_filed_under_its_start_folder() {
    // Measured: at Stop, after a Bash `cd sub`, the hook's cwd is the subfolder and CLAUDE_PROJECT_DIR is not.
    let home = scratch("subfolder");
    let after_cd = hook(json!({"hook_event_name": "Stop", "cwd": "C:/work/proj/sub"}));
    handle(
        &home,
        &after_cd,
        Some("C:/work/proj"),
        "2026-10-18T12:00:00Z",
    )
    .expect("written");
    assert!(read(&home, Path::new("C:/work/proj")).is_some());
    assert!(read(&home, Path::new("C:/work/proj/sub")).is_none());
    // With no project dir the hook's cwd is all there is.
    assert_eq!(
        folder_of(&after_cd, None),
        Some(PathBuf::from("C:/work/proj/sub"))
    );
    assert_eq!(
        folder_of(&after_cd, Some("  ")),
        Some(PathBuf::from("C:/work/proj/sub"))
    );
    assert_eq!(folder_of(&json!({}), None), None);
}

#[test]
fn the_start_dir_and_the_open_command_agree_on_the_hash_however_the_path_is_spelled() {
    let a = path_in(Path::new("h"), Path::new("C:/work/proj"));
    let b = path_in(Path::new("h"), Path::new(r"C:\Work\Proj\"));
    assert_eq!(a, b);
}

#[test]
fn a_hook_that_describes_no_session_writes_nothing_and_does_not_fail() {
    let home = scratch("nothing");
    for (bad, why) in [
        (hook(json!({"session_id": ""})), "no session_id"),
        (hook(json!({"session_id": null})), "no session_id"),
        (
            hook(json!({"session_id": "a b; rm"})),
            "a session_id with an odd character",
        ),
        (hook(json!({"transcript_path": null})), "no transcript_path"),
        (
            hook(json!({"transcript_path": "relative/t.jsonl"})),
            "a relative transcript_path",
        ),
        (hook(json!({"agent_id": "sub-1"})), "a subagent's hook"),
        (Value::Null, "no session_id"),
    ] {
        assert_eq!(
            handle(&home, &bad, Some("C:/work/proj"), "2026-10-18T10:00:00Z").unwrap_err(),
            why
        );
    }
    assert!(
        handle(&home, &hook(json!({})), None, "t").is_ok(),
        "the hook's own cwd is enough"
    );
    assert!(
        handle(&home, &hook(json!({"cwd": ""})), None, "t").is_err(),
        "but nothing at all is no folder"
    );
    assert!(!home
        .join("nirvana")
        .join("sessions")
        .join("missing")
        .exists());
}

#[test]
fn an_unwritable_home_is_an_error_and_a_garbled_record_reads_as_none() {
    let home = scratch("unwritable");
    // A file where the sessions directory should be.
    std::fs::create_dir_all(home.join("nirvana")).unwrap();
    std::fs::write(home.join("nirvana").join("sessions"), "in the way").unwrap();
    assert_eq!(
        handle(&home, &hook(json!({})), Some("C:/work/proj"), "t").unwrap_err(),
        "the record could not be written"
    );
    let other = scratch("garbled");
    let path = path_in(&other, Path::new("C:/work/proj"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();
    assert!(read(&other, Path::new("C:/work/proj")).is_none());
}

#[test]
fn the_command_exits_ok_on_garbage_a_missing_home_and_an_unwritable_home() {
    let home = scratch("run");
    let good = hook(json!({})).to_string();
    assert!(run("not json at all", Some(&home), Some("C:/work/proj")).is_ok());
    assert!(run("", Some(&home), None).is_ok());
    assert!(run(&good, None, Some("C:/work/proj")).is_ok());
    assert!(
        !home.join("nirvana").exists(),
        "garbage and a missing home write nothing"
    );
    std::fs::create_dir_all(home.join("nirvana")).unwrap();
    std::fs::write(home.join("nirvana").join("sessions"), "in the way").unwrap();
    assert!(run(&good, Some(&home), Some("C:/work/proj")).is_ok());
    let fine = scratch("run-fine");
    assert!(run(&good, Some(&fine), Some("C:/work/proj")).is_ok());
    assert!(
        read(&fine, Path::new("C:/work/proj")).is_some(),
        "and a good hook is written"
    );
}

#[test]
fn the_plugin_runs_record_session_from_session_start_and_stop_in_the_same_exec_form_as_the_others()
{
    let hooks: Value = serde_json::from_str(include_str!("../../nirvana/claude/hooks/hooks.json"))
        .expect("hooks.json");
    for event in ["SessionStart", "Stop"] {
        let commands: Vec<&Value> = hooks["hooks"][event]
            .as_array()
            .expect(event)
            .iter()
            .flat_map(|group| group["hooks"].as_array().expect("hooks"))
            .collect();
        let mine: Vec<&&Value> = commands
            .iter()
            .filter(|h| h["args"] == json!(["memory", "record-session"]))
            .collect();
        assert_eq!(mine.len(), 1, "{event} runs record-session once");
        // The same exec form as its neighbours: the fork's herdr.exe by the plugin root, args as a list.
        assert_eq!(mine[0]["type"], "command");
        assert_eq!(mine[0]["command"], commands[0]["command"], "{event}");
        assert!(mine[0]["timeout"]
            .as_u64()
            .is_some_and(|s| (1..=15).contains(&s)));
    }
}

// ---- the fix round -----------------------------------------------------------------------------------

#[test]
fn a_relative_folder_is_filed_under_its_absolute_hash() {
    let home = scratch("relative");
    let cwd = std::env::current_dir().unwrap();
    handle(
        &home,
        &hook(json!({})),
        Some("rel-proj"),
        "2026-10-18T10:00:00Z",
    )
    .expect("written");
    assert!(read(&home, &cwd.join("rel-proj")).is_some());
    assert!(read(&home, Path::new("rel-proj")).is_none());
    assert_eq!(
        folder_of(&json!({"cwd": "rel-cwd"}), None),
        Some(cwd.join("rel-cwd")),
        "the hook's own cwd is resolved too"
    );
}

#[test]
fn a_verbatim_prefix_is_dropped_and_other_verbatim_forms_are_left_alone() {
    assert_eq!(
        strip_verbatim(r"\\?\C:\work\proj").as_deref(),
        Some(r"C:\work\proj")
    );
    assert_eq!(
        strip_verbatim(r"\\?\UNC\srv\share\x").as_deref(),
        Some(r"\\srv\share\x")
    );
    assert_eq!(strip_verbatim(r"\\?\Volume{1234}\x"), None);
    assert_eq!(strip_verbatim(r"C:\work\proj"), None);
}

#[cfg(windows)]
#[test]
fn a_verbatim_folder_and_its_plain_spelling_are_one_record() {
    let home = scratch("verbatim");
    handle(
        &home,
        &hook(json!({})),
        Some(r"\\?\C:\work\proj"),
        "2026-10-18T10:00:00Z",
    )
    .expect("written");
    assert!(read(&home, Path::new(r"C:\work\proj")).is_some());
    assert_eq!(
        absolute_folder(Path::new(r"\\?\C:\work\proj")),
        PathBuf::from(r"C:\work\proj")
    );
}

#[test]
fn temp_names_are_unique_and_append_to_the_whole_file_name() {
    // A dotted stem: `with_extension` would have replaced `.json` and kept `notes.v1`, or cut the stem short.
    let path = Path::new("dir").join("notes.v1.json");
    let names: Vec<PathBuf> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| scope.spawn(|| (0..200).map(|_| temp_sibling(&path)).collect::<Vec<_>>()))
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect()
    });
    let unique: std::collections::BTreeSet<&PathBuf> = names.iter().collect();
    assert_eq!(unique.len(), 8 * 200, "no name is handed out twice");
    let pid = std::process::id().to_string();
    for name in &names {
        assert_eq!(name.parent(), path.parent());
        let file = name.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(file.starts_with("notes.v1.json.tmp."), "{file}");
        assert!(
            file["notes.v1.json.tmp.".len()..].starts_with(&pid),
            "{file}"
        );
    }
}

#[test]
fn a_record_write_leaves_a_neighbour_the_old_temp_name_would_have_taken() {
    let home = scratch("neighbour");
    let folder = std::env::current_dir().unwrap().join("work-proj");
    let path = path_in(&home, &folder);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    // The name the old code used: `<hash>.json` with `json.<pid>.tmp` in the place of its extension.
    let neighbour = path.with_file_name(format!(
        "{}.json.{}.tmp",
        crate::nirvana_launch::folder_hash(&folder),
        std::process::id()
    ));
    std::fs::write(&neighbour, "keep").unwrap();
    let record = record_of(&hook(json!({})), "2026-10-18T10:00:00Z").unwrap();
    write(&home, &folder, &record).unwrap();
    write(&home, &folder, &record).unwrap();
    assert!(read(&home, &folder).is_some());
    assert_eq!(std::fs::read_to_string(&neighbour).unwrap(), "keep");
    let left = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .flatten()
        .count();
    assert_eq!(left, 2, "the record and the neighbour, no temporary file");
}
