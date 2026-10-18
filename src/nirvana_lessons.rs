//! Nirvana fork, plan 12 O5: lessons a finished order leaves, once the gate has checked them, and the one line
//! the next order of their class gets in its brief. `herdr lessons for --class C`.
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §7.4.3; old Nirvana decision 0023)
//!
//! The owner's rulings of 2026-10-16, verbatim: "Checked lessions should go live automatically, keep it per github
//! remote assuming that project has a github ... have the lesson stay in its own project. Keep a board per
//! workspace."
//!
//! - **Written only after the gate.** A Work Report's `lesson` (name, class, steps of at most 120 lines, an evidence
//!   quote) becomes `<project>/.nirvana/lessons/<slug>.md` only when its order passed the gate and the quote is in
//!   the worker's own stream, outside the Work Report itself, and only when the project's repository has a GitHub
//!   remote. The Ledger-Lessons research of 2026-10-16 measured 21 of 21 memory writes skipped where the old
//!   harness wrote into a folder the repositories did not have; here the folder is the project's own.
//! - **One line in a brief, never the body.** Decision 0023: "everything in the prompt is paid for on every call".
//!   The worker reads the file when it wants it.
//! - **Counters** in `.nirvana/lessons/index.json`: offered (a brief listed it), opened (the worker's stream read
//!   its file), passed after opened. A lesson offered often that does not help decays out of the briefs (`live`).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::nirvana_run::{self, Project};

/// The folder in `.nirvana/`; tracked with the project (O3's `.gitignore` leaves it out on purpose).
pub const DIR: &str = "lessons";
pub const INDEX: &str = "index.json";
/// §7.4.3: "the steps (at most 120 lines)"; O3's Work Report check holds the same number.
pub const STEPS_LINES: usize = 120;
/// The decay rule, both numbers hypotheses: a lesson stays in the briefs for its first `DECAY_AFTER` offers
/// whatever happens, and after that only while at least one offer in `HELP_EVERY` led to an order that opened it
/// and passed. Six offers is two orders more than the send-back cap's three, so one bad week of a class does not
/// decay a lesson that a later order would have used.
pub const DECAY_AFTER: u32 = 6;
pub const HELP_EVERY: u32 = 3;
/// The orders an index entry remembers having counted, so a re-gated order is not counted twice.
const SEEN_KEPT: usize = 100;
/// The marker of the project's line in `<home>/nirvana/NIRVANA.md`.
const INDEX_MARK: &str = "<!-- herdr-lessons ";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counters {
    #[serde(default)]
    pub offered: u32,
    #[serde(default)]
    pub opened: u32,
    #[serde(default)]
    pub passed_after_opened: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub class: String,
    /// The line a brief carries; the lesson's name, which the worker wrote as its title.
    pub description: String,
    pub order: String,
    pub run: String,
    pub written_at: u64,
    #[serde(flatten)]
    pub counters: Counters,
    /// Orders already counted as opening it (the last `SEEN_KEPT`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    #[serde(default = "one")]
    pub v: u32,
    #[serde(default)]
    pub lessons: BTreeMap<String, Entry>,
}

fn one() -> u32 {
    1
}

/// Whether a lesson still goes into briefs (the decay rule above).
pub fn live(counters: &Counters) -> bool {
    counters.offered < DECAY_AFTER || counters.passed_after_opened * HELP_EVERY >= counters.offered
}

pub fn dir(root: &Path) -> PathBuf {
    root.join(nirvana_run::DIR).join(DIR)
}

/// The lock for `index.json`: under `runs/`, which O3's `.gitignore` ignores, so the lock file never shows in
/// the project's `git status` (a lock inside `lessons/` would be tracked).
fn lock(root: &Path) -> std::io::Result<IndexLock> {
    let path = root
        .join(nirvana_run::DIR)
        .join("runs")
        .join(".lessons.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.lock()?;
    Ok(IndexLock(file))
}

struct IndexLock(std::fs::File);

impl Drop for IndexLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn read_index(root: &Path) -> Index {
    std::fs::read_to_string(dir(root).join(INDEX))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_index(root: &Path, index: &Index) -> std::io::Result<()> {
    std::fs::create_dir_all(dir(root))?;
    let mut text = serde_json::to_string_pretty(index)?;
    text.push('\n');
    crate::nirvana_seat::write_atomic(&dir(root).join(INDEX), text.as_bytes())
}

/// A file name from a lesson's name: lowercase letters, digits and dashes, at most 48.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(48).collect();
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "lesson".into()
    } else {
        out
    }
}

/// Whether the repository has a GitHub remote: `git remote -v` names github.com (the ruling: "keep it per github
/// remote assuming that project has a github").
pub fn has_github_remote(root: &Path) -> bool {
    std::process::Command::new("git")
        .args(["remote", "-v"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .is_some_and(|out| {
            String::from_utf8_lossy(&out.stdout)
                .to_ascii_lowercase()
                .contains("github.com")
        })
}

/// Every string a worker's stream holds that the worker did not write as its Work Report: tool inputs and
/// results, its text. Left out: the run's `result` line, the `StructuredOutput` call that carries the report,
/// and any string holding the report's JSON (agy streams its answer as text): otherwise the lesson's own
/// evidence field would be found in the stream and every quote would pass.
pub fn stream_strings(stream: &str) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                if !text.contains("\"evidence\"") {
                    out.push(text.replace("\r\n", "\n"));
                }
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, out)),
            Value::Object(map) => {
                if map.get("type").and_then(Value::as_str) == Some("tool_use")
                    && map.get("name").and_then(Value::as_str) == Some("StructuredOutput")
                {
                    return;
                }
                map.values().for_each(|item| walk(item, out));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for line in stream.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) == Some("result")
            || value.get("event").and_then(Value::as_str) == Some("result")
        {
            continue;
        }
        walk(&value, &mut out);
    }
    out
}

/// §7.4.3: "the quote is in the worker's stream".
pub fn quote_in_stream(stream: &str, quote: &str) -> bool {
    let quote = quote.replace("\r\n", "\n");
    let quote = quote.trim();
    !quote.is_empty()
        && stream_strings(stream)
            .iter()
            .any(|text| text.contains(quote))
}

/// Whether the worker opened `<slug>.md`: a tool call whose input names it. The order's own text names every
/// offered lesson, so a tool result (the order file read back) does not count.
pub fn opened_in_stream(stream: &str, slug: &str) -> bool {
    let file = format!("{slug}.md");
    for line in stream.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        // claude: an assistant message's tool_use blocks.
        if let Some(blocks) = value.pointer("/message/content").and_then(Value::as_array) {
            if value.get("type").and_then(Value::as_str) == Some("assistant")
                && blocks.iter().any(|block| {
                    block.get("type").and_then(Value::as_str) == Some("tool_use")
                        && block["input"].to_string().contains(&file)
                })
            {
                return true;
            }
        }
        // agy: a tool step's parameters.
        if value
            .pointer("/step_update/step_type")
            .and_then(Value::as_str)
            == Some("tool")
            && value
                .pointer("/step_update/tool_info/parameters")
                .is_some_and(|params| params.to_string().contains(&file))
        {
            return true;
        }
    }
    false
}

/// What the gate decided about a Work Report's lesson.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Written(PathBuf),
    /// Checked and kept out, with why: no GitHub remote, the order did not pass, the quote is not in the stream.
    Refused(String),
}

/// The gate's check and write. `passed` is the order's gate verdict; `stream` the worker's raw stream.
pub fn verify_and_write(
    root: &Path,
    lesson: &Value,
    passed: bool,
    stream: &str,
    order: &str,
    run: &str,
    at: u64,
    github: bool,
) -> Verdict {
    let text = |key: &str| lesson.get(key).and_then(Value::as_str).unwrap_or("").trim();
    let (name, class, steps, evidence) =
        (text("name"), text("class"), text("steps"), text("evidence"));
    if name.is_empty() || class.is_empty() || steps.is_empty() || evidence.is_empty() {
        return Verdict::Refused("the lesson lacks a name, class, steps or evidence".into());
    }
    if steps.lines().count() > STEPS_LINES {
        return Verdict::Refused(format!("the steps run over {STEPS_LINES} lines"));
    }
    if !passed {
        return Verdict::Refused("the order did not pass the gate".into());
    }
    if !quote_in_stream(stream, evidence) {
        return Verdict::Refused("the evidence quote is not in the worker's stream".into());
    }
    if !github {
        return Verdict::Refused(
            "the repository has no GitHub remote (the ruling of 2026-10-16: lessons live per GitHub remote)"
                .into(),
        );
    }
    match write(root, name, class, steps, evidence, order, run, at) {
        Ok(path) => Verdict::Written(path),
        Err(err) => Verdict::Refused(format!("could not write it: {err}")),
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Writes `<slug>.md` and its index entry. A lesson of the same name replaces the older text and keeps its
/// counters: the newer one was checked more recently.
#[allow(clippy::too_many_arguments)]
fn write(
    root: &Path,
    name: &str,
    class: &str,
    steps: &str,
    evidence: &str,
    order: &str,
    run: &str,
    at: u64,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir(root))?;
    let _lock = lock(root)?;
    let slug = slug(name);
    let path = dir(root).join(format!("{slug}.md"));
    let name_line = one_line(name);
    let body = format!(
        "---\nname: {name_line}\ndescription: {name_line}\nclass: {}\norder: {order}\nrun: {run}\nevidence: {}\n---\n\n{}\n",
        one_line(class),
        one_line(evidence),
        steps.replace("\r\n", "\n").trim_end()
    );
    std::fs::write(&path, body)?;
    let mut index = read_index(root);
    let counters = index
        .lessons
        .get(&slug)
        .map(|entry| (entry.counters.clone(), entry.seen.clone()));
    let (counters, seen) = counters.unwrap_or_default();
    index.lessons.insert(
        slug,
        Entry {
            name: name_line.clone(),
            class: one_line(class),
            description: name_line,
            order: order.to_owned(),
            run: run.to_owned(),
            written_at: at,
            counters,
            seen,
        },
    );
    write_index(root, &index)?;
    Ok(path)
}

/// One brief line per live lesson of `class`: the path from the worker's folder, and the description.
pub fn lines_for(index: &Index, class: &str) -> Vec<(String, String)> {
    index
        .lessons
        .iter()
        .filter(|(_, entry)| entry.class == class && live(&entry.counters))
        .map(|(slug, entry)| {
            (
                slug.clone(),
                format!(
                    "- {}/{DIR}/{slug}.md: {}",
                    nirvana_run::DIR,
                    entry.description
                ),
            )
        })
        .collect()
}

/// O3's brief composer calls this for each new order: the section to add, and each offered lesson's
/// `offered` counted. Empty when the class has no live lesson. The files are copied into the worker's folder
/// when it is a worktree, since claude's print-mode file tools stay inside the working folder.
pub fn offer(root: &Path, cwd: &Path, class: &str) -> String {
    let Ok(_lock) = lock(root) else {
        return String::new();
    };
    let mut index = read_index(root);
    let lines = lines_for(&index, class);
    if lines.is_empty() {
        return String::new();
    }
    let mut section = format!(
        "\n\n## Lessons from earlier {class} orders\n\nEach was checked by herdr's gate. Open one only if it fits this order.\n\n"
    );
    for (slug, line) in &lines {
        if cwd != root {
            let from = dir(root).join(format!("{slug}.md"));
            let to = dir(cwd).join(format!("{slug}.md"));
            if let Some(parent) = to.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(from, to);
        }
        section.push_str(line);
        section.push('\n');
        if let Some(entry) = index.lessons.get_mut(slug) {
            entry.counters.offered += 1;
        }
    }
    let _ = write_index(root, &index);
    section
}

/// The gate's count at an order's verdict: each lesson the worker opened gains `opened`, and
/// `passed_after_opened` when the order passed. Once per order.
pub fn note_gated(
    root: &Path,
    stream: &str,
    order: &str,
    passed: bool,
) -> std::io::Result<Vec<String>> {
    let _lock = lock(root)?;
    let mut index = read_index(root);
    let mut counted = Vec::new();
    for (slug, entry) in index.lessons.iter_mut() {
        if entry.seen.iter().any(|seen| seen == order) || !opened_in_stream(stream, slug) {
            continue;
        }
        entry.counters.opened += 1;
        if passed {
            entry.counters.passed_after_opened += 1;
        }
        entry.seen.push(order.to_owned());
        if entry.seen.len() > SEEN_KEPT {
            entry.seen.remove(0);
        }
        counted.push(slug.clone());
    }
    if !counted.is_empty() {
        write_index(root, &index)?;
    }
    Ok(counted)
}

/// The project's line in `<home>/nirvana/NIRVANA.md`, which the orchestrator's SessionStart digest reads (O4b).
/// One line per project, replaced in place; nothing else in the file is touched.
pub fn index_line_text(root: &Path, live_count: usize) -> String {
    let root = root.display();
    format!(
        "- lessons for {root}: {live_count} live in {root}{sep}{}{sep}{DIR}; `herdr lessons for --class <class>` lists a class's {INDEX_MARK}{root} -->",
        nirvana_run::DIR,
        sep = std::path::MAIN_SEPARATOR
    )
}

pub fn write_index_line(home: &Path, root: &Path) -> std::io::Result<PathBuf> {
    let index = read_index(root);
    let count = index
        .lessons
        .values()
        .filter(|entry| live(&entry.counters))
        .count();
    let path = home.join(crate::nirvana_seat::DIR).join("NIRVANA.md");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mark = format!("{INDEX_MARK}{} -->", root.display());
    let line = index_line_text(root, count);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    match lines.iter().position(|existing| existing.ends_with(&mark)) {
        Some(at) => lines[at] = line,
        None => lines.push(line),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = lines.join("\n");
    out.push('\n');
    crate::nirvana_seat::write_atomic(&path, out.as_bytes())?;
    Ok(path)
}

// ---- the CLI ----

const USAGE: &str = "usage: herdr lessons for --class <class> [--all]   one line per lesson: its path and description
       herdr lessons list [--json]                  every lesson with its counters";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let project = Project::from_env();
    let root = project.root.clone();
    match args.first().map(String::as_str) {
        Some("for") => {
            let mut class = None;
            let mut all = false;
            let mut iter = args[1..].iter();
            while let Some(arg) = iter.next() {
                match arg.as_str() {
                    "--class" => class = iter.next().cloned(),
                    "--all" => all = true,
                    _ => usage_exit(),
                }
            }
            let Some(class) = class else { usage_exit() };
            let index = read_index(&root);
            for (slug, entry) in &index.lessons {
                if entry.class != class {
                    continue;
                }
                let is_live = live(&entry.counters);
                if !is_live && !all {
                    continue;
                }
                println!(
                    "{}: {}{}",
                    dir(&root).join(format!("{slug}.md")).display(),
                    entry.description,
                    if is_live { "" } else { " (decayed)" }
                );
            }
            Ok(())
        }
        Some("list") => {
            let index = read_index(&root);
            if args.get(1).map(String::as_str) == Some("--json") {
                println!("{}", serde_json::to_string_pretty(&index)?);
                return Ok(());
            }
            if index.lessons.is_empty() {
                println!("no lesson in {}", dir(&root).display());
            }
            for (slug, entry) in &index.lessons {
                println!(
                    "{slug} · {} · offered {} · opened {} · passed after opened {}{}",
                    entry.class,
                    entry.counters.offered,
                    entry.counters.opened,
                    entry.counters.passed_after_opened,
                    if live(&entry.counters) {
                        ""
                    } else {
                        " · decayed"
                    }
                );
            }
            Ok(())
        }
        _ => usage_exit(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "herdr-o5-lessons-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn git(root: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A claude stream: the worker ran a command whose output holds the quote, then delivered a report whose
    /// lesson quotes it (the report repeats the quote in StructuredOutput and in the result line).
    fn stream(output: &str, quote: &str) -> String {
        let lesson = serde_json::json!({"name":"Run the suite headless","class":"build.bounded","steps":"1. run it","evidence":quote});
        [
            serde_json::json!({"type":"assistant","message":{"id":"m1","content":[{"type":"tool_use","name":"Bash","input":{"command":"godot --headless -s run_tests.gd"}}]}}),
            serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":output}]}}),
            serde_json::json!({"type":"assistant","message":{"id":"m2","content":[{"type":"tool_use","name":"StructuredOutput","input":{"status":"done","lesson":lesson}}]}}),
            serde_json::json!({"type":"result","subtype":"success","structured_output":{"status":"done","lesson":lesson}}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
    }

    fn lesson(quote: &str) -> Value {
        serde_json::json!({"name":"Run the suite headless","class":"build.bounded","steps":"1. godot --headless -s run_tests.gd\n2. read TOTAL","evidence":quote})
    }

    #[test]
    fn a_quote_counts_only_from_the_stream_outside_the_work_report() {
        let text = stream("=== TOTAL 12 passed, 0 failed", "TOTAL 12 passed");
        assert!(quote_in_stream(&text, "TOTAL 12 passed"));
        // The report's own lesson carries this quote, in StructuredOutput and in the result: not evidence.
        let text = stream("nothing useful", "TOTAL 99 passed");
        assert!(!quote_in_stream(&text, "TOTAL 99 passed"));
        assert!(!quote_in_stream(&text, "   "));
    }

    #[test]
    fn a_lesson_is_written_only_after_a_pass_and_only_with_a_github_remote() {
        let root = temp("write");
        let text = stream("=== TOTAL 12 passed", "TOTAL 12 passed");
        let l = lesson("TOTAL 12 passed");
        assert_eq!(
            verify_and_write(&root, &l, false, &text, "o1", "r1", 5, true),
            Verdict::Refused("the order did not pass the gate".into())
        );
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, false),
            Verdict::Refused(why) if why.contains("no GitHub remote")
        ));
        assert!(matches!(
            verify_and_write(&root, &lesson("not in it"), true, &text, "o1", "r1", 5, true),
            Verdict::Refused(why) if why.contains("not in the worker's stream")
        ));
        assert!(!dir(&root).join("run-the-suite-headless.md").exists());
        let Verdict::Written(path) = verify_and_write(&root, &l, true, &text, "o1", "r1", 5, true)
        else {
            panic!("written")
        };
        assert_eq!(path, dir(&root).join("run-the-suite-headless.md"));
        let body = std::fs::read_to_string(&path).expect("body");
        assert!(
            body.starts_with("---\nname: Run the suite headless\n"),
            "{body}"
        );
        assert!(body.contains("\nclass: build.bounded\n") && body.ends_with("2. read TOTAL\n"));
        let index = read_index(&root);
        assert_eq!(index.lessons["run-the-suite-headless"].order, "o1");
    }

    #[test]
    fn steps_over_120_lines_are_refused() {
        let root = temp("long");
        let mut l = lesson("TOTAL 12 passed");
        l["steps"] = Value::String("step\n".repeat(121));
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        assert!(matches!(
            verify_and_write(&root, &l, true, &text, "o1", "r1", 5, true),
            Verdict::Refused(why) if why.contains("120")
        ));
    }

    #[test]
    fn the_remote_check_reads_git_remote_for_github() {
        let root = temp("remote");
        git(&root, &["init", "-q"]);
        assert!(!has_github_remote(&root));
        git(
            &root,
            &["remote", "add", "origin", "https://gitlab.com/a/b.git"],
        );
        assert!(!has_github_remote(&root));
        git(&root, &["remote", "add", "up", "git@github.com:a/b.git"]);
        assert!(has_github_remote(&root));
    }

    #[test]
    fn the_brief_gets_one_line_per_live_lesson_and_counts_the_offer() {
        let root = temp("brief");
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        verify_and_write(
            &root,
            &lesson("TOTAL 12 passed"),
            true,
            &text,
            "o1",
            "r1",
            5,
            true,
        );
        assert_eq!(offer(&root, &root, "chore.script"), "");
        let section = offer(&root, &root, "build.bounded");
        assert!(
            section.ends_with(
                "\n- .nirvana/lessons/run-the-suite-headless.md: Run the suite headless\n"
            ),
            "{section:?}"
        );
        // The line names the path and the description, never the body.
        assert!(!section.contains("godot --headless"));
        assert_eq!(
            read_index(&root).lessons["run-the-suite-headless"]
                .counters
                .offered,
            1
        );
        // A worktree gets its own copy, where the worker's file tools reach.
        let worktree = root.join("wt");
        offer(&root, &worktree, "build.bounded");
        assert!(dir(&worktree).join("run-the-suite-headless.md").is_file());
    }

    #[test]
    fn opened_and_passed_after_opened_count_once_per_order() {
        let root = temp("count");
        let text = stream("TOTAL 12 passed", "TOTAL 12 passed");
        verify_and_write(
            &root,
            &lesson("TOTAL 12 passed"),
            true,
            &text,
            "o1",
            "r1",
            5,
            true,
        );
        let read = serde_json::json!({"type":"assistant","message":{"id":"m","content":[{"type":"tool_use","name":"Read","input":{"file_path":".nirvana/lessons/run-the-suite-headless.md"}}]}}).to_string();
        // The order file read back names the lesson in a tool result: not an open.
        let named = serde_json::json!({"type":"user","message":{"content":[{"type":"tool_result","content":"- .nirvana/lessons/run-the-suite-headless.md: Run"}]}}).to_string();
        assert_eq!(
            note_gated(&root, &named, "o2", true).expect("note"),
            Vec::<String>::new()
        );
        assert_eq!(
            note_gated(&root, &read, "o2", true).expect("note"),
            vec!["run-the-suite-headless".to_owned()]
        );
        assert!(note_gated(&root, &read, "o2", true)
            .expect("again")
            .is_empty());
        note_gated(&root, &read, "o3", false).expect("o3");
        let c = &read_index(&root).lessons["run-the-suite-headless"].counters;
        assert_eq!((c.opened, c.passed_after_opened), (2, 1));
    }

    #[test]
    fn a_lesson_offered_often_that_does_not_help_decays_out_of_the_briefs() {
        let c = |offered, passed| Counters {
            offered,
            opened: passed,
            passed_after_opened: passed,
        };
        assert!(live(&c(5, 0)));
        assert!(!live(&c(6, 0)));
        assert!(live(&c(6, 2)));
        assert!(!live(&c(9, 2)));
        assert!(live(&c(9, 3)));
        let mut index = Index::default();
        index.lessons.insert(
            "x".into(),
            Entry {
                name: "X".into(),
                class: "build.bounded".into(),
                description: "X".into(),
                order: "o".into(),
                run: "r".into(),
                written_at: 0,
                counters: c(6, 0),
                seen: Vec::new(),
            },
        );
        assert!(lines_for(&index, "build.bounded").is_empty());
    }

    #[test]
    fn nirvana_md_gets_one_line_per_project_replaced_in_place() {
        let home = temp("home");
        let root = temp("proj");
        let path = home.join("nirvana").join("NIRVANA.md");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(&path, "# Nirvana\n\nthe owner's line\n").expect("seed");
        write_index_line(&home, &root).expect("first");
        write_index_line(&home, &root).expect("second");
        let text = std::fs::read_to_string(&path).expect("text");
        assert!(
            text.starts_with("# Nirvana\n\nthe owner's line\n- lessons for "),
            "{text}"
        );
        assert_eq!(text.matches(INDEX_MARK).count(), 1);
        assert!(text.contains(": 0 live in "));
    }

    #[test]
    fn slugs_are_safe_file_names() {
        assert_eq!(
            slug("Run the Godot suite --headless!"),
            "run-the-godot-suite-headless"
        );
        assert_eq!(slug("../../etc"), "etc");
        assert_eq!(slug("…"), "lesson");
    }
}
