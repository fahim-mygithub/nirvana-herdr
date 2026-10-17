//! Nirvana fork, feature 10: the Jev memory, one file every agent's Jev instructions come from.
//!
//! The owner, 2026-10-17, after two single runs on the 568 KB project saved nothing: "maybe just have a
//! tool flag in memory ... have it a seperate file. This Jev memory preload file will give it
//! instructions on how to make Jev tool chain handoffs and at what sizes it would make sence to invoke it."
//!
//! - **The file** is `<home>/jev/JEV.md`, written from `DEFAULT` the first time anything reads it, and
//!   the owner's own from then on: Preload's `e` opens it, and every agent's next session gets the edit.
//! - **Its front matter holds the sizes**, so a size lives in one place: the body names them as
//!   `{read_file_kb}` and so on, and `min_project_kb` decides what a folder gets.
//! - **A folder under `min_project_kb`** gets one line, the tool flag: Jev is there, and only for a big
//!   file or long output. At or over it, the whole body and a line measuring the project.
//!
//! Preload's "Jev memory" switch puts it in front of the agents (`nirvana_jev_install`): Claude Code's
//! `SessionStart` hook prints `for_folder`, and Codex and agy get the same text in `AGENTS.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const FILE_NAME: &str = "JEV.md";

/// herdr's first Jev memory. The sizes are hypotheses from two A/Bs (2026-10-16 and 17, Claude Sonnet):
/// on a 71 KB project the guide changed tokens by noise; on a 568 KB one by -16% to +17% across four
/// wordings, with a plain search as cheap as Jev wherever a word found the place, and output of a
/// four-test run too short to hand off. `min_project_kb` sits above both.
pub const DEFAULT: &str = "---
# herdr's Jev memory: what each agent is told about Jev, from Preload's \"Jev memory\" switch.
# Edit freely; each new agent session gets the new text. The sizes are named in the text below.
# Under min_project_kb a folder gets only the one-line flag; at or over it, everything after the ---.
min_project_kb: 1024
read_file_kb: 20
ask_file_kb: 50
files_kb: 200
output_lines: 200
---
## Jev (herdr): a cheap judge, for big files and long output
Jev answers a typed question about a file or a command's output in about a second, for a fraction of a cent, and the text never enters your context. herdr is `\"{herdr}\"`.

When to ask it. A read you skip is not paid again on later turns, but every extra tool turn re-reads your whole context, so ask Jev only where it replaces reading:
- A word or a name finds the place (a function, an error message): search. Jev does not beat a search.
- One file under {read_file_kb} KB, or one you will edit, call into or quote: read it.
- One file over {ask_file_kb} KB, and you need one fact from it: `herdr jev file <path> --ask \"<yes or no question>\"`.
- Candidate files over {files_kb} KB in all, and a question about behaviour that no word finds: `herdr jev files \"<glob>\" --ask \"<question>\"`.
- Output over {output_lines} lines: `<command> 2>&1 | herdr jev text --ask \"<question>\" --choice \"a,b,c\"`. Read shorter output yourself.

Handoffs:
- `jev files`, then open only its `read first` file, and another only if that one does not answer.
- `jev text --choice \"bug_in_code,wrong_test,environment\"`: on bug_in_code, find the file (a search, or `jev files`) and read it to fix it.
- Before an edit, read the file you change and any helper you will call: one read now is cheaper than a search later.
- An answer of `failed`, or a confidence under 0.7: read it yourself.
";

/// The flag a folder under `min_project_kb` gets instead of the body.
const FLAG: &str = "Jev (herdr) is installed: `\"{herdr}\" jev file|files|text --ask \"...\"`, a cheap judge that answers about a file or output without it entering your context. This project is {project}, under {min_project_kb} KB, so search and read as usual; ask Jev only about one file over {ask_file_kb} KB, or output over {output_lines} lines.\n";

pub fn path() -> PathBuf {
    crate::nirvana::home()
        .unwrap_or_else(|| std::env::temp_dir().join("nirvana-herdr"))
        .join("jev")
        .join(FILE_NAME)
}

/// The memory file's text, written from `DEFAULT` when it is missing.
pub fn read_or_create(path: &Path) -> String {
    if let Ok(text) = std::fs::read_to_string(path) {
        return text;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, DEFAULT);
    DEFAULT.to_owned()
}

/// The front matter's `name: value` lines, and the body after it. `#` lines are comments.
pub fn split(text: &str) -> (HashMap<String, String>, &str) {
    let mut values = HashMap::new();
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (values, text);
    };
    let mut offset = text.len() - rest.len();
    for line in rest.split_inclusive('\n') {
        offset += line.len();
        let line = line.trim();
        if line == "---" {
            return (values, &text[offset..]);
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            values.insert(name.trim().to_owned(), value.trim().to_owned());
        }
    }
    (HashMap::new(), text) // no closing line: all of it is the body
}

fn fill(template: &str, values: &HashMap<String, String>) -> String {
    let mut out = template.to_owned();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// What a project weighs: the files git tracks, or, outside git, the files under it but the folders
/// Jev never reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Size {
    pub bytes: u64,
    pub files: usize,
    /// The largest three, relative to the root.
    pub largest: Vec<(String, u64)>,
}

const MAX_FILES: usize = 100_000;

/// Files no agent reads as text, by their extension: Jev refuses binaries, so they are not its work.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "svgz", "pdf", "zip", "gz", "tgz", "xz",
    "7z", "rar", "jar", "exe", "dll", "so", "dylib", "a", "lib", "o", "obj", "pdb", "wasm", "woff",
    "woff2", "ttf", "otf", "eot", "mp3", "mp4", "mov", "wav", "ogg", "webm", "psd", "sqlite", "db",
    "bin",
];

fn binary_name(rel: &str) -> bool {
    rel.rsplit_once('.')
        .is_some_and(|(_, ext)| BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

fn measure_uncached(root: &Path) -> Size {
    let mut sizes: Vec<(String, u64)> = Vec::new();
    let tracked = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned());
    match tracked {
        Some(list) => {
            for rel in list
                .split('\0')
                .filter(|rel| !rel.is_empty())
                .take(MAX_FILES)
            {
                if let Ok(meta) = std::fs::metadata(root.join(rel)) {
                    sizes.push((rel.to_owned(), meta.len()));
                }
            }
        }
        None => {
            let mut stack = vec![root.to_path_buf()];
            while let Some(dir) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let Ok(kind) = entry.file_type() else {
                        continue;
                    };
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if kind.is_dir() && !crate::nirvana_jev::excluded_dir(&name) {
                        stack.push(entry.path());
                    } else if kind.is_file() && sizes.len() < MAX_FILES {
                        let rel = entry
                            .path()
                            .strip_prefix(root)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_default();
                        sizes.push((rel, entry.metadata().map(|m| m.len()).unwrap_or(0)));
                    }
                }
            }
        }
    }
    // Only text an agent might read counts: on this repository an image was among the largest three.
    sizes.retain(|(rel, _)| !binary_name(rel));
    let bytes = sizes.iter().map(|(_, n)| n).sum();
    let files = sizes.len();
    sizes.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sizes.truncate(3);
    Size {
        bytes,
        files,
        largest: sizes,
    }
}

/// Preload draws often, so a folder's size is measured once a minute at most.
pub fn measure(root: &Path) -> Size {
    static CACHE: Mutex<Option<HashMap<PathBuf, (Instant, Size)>>> = Mutex::new(None);
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if let Some((at, size)) = map.get(root) {
        if at.elapsed() < Duration::from_secs(60) {
            return size.clone();
        }
    }
    let size = measure_uncached(root);
    map.insert(root.to_path_buf(), (Instant::now(), size.clone()));
    size
}

pub fn kb(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}

fn project_words(size: &Size) -> String {
    format!("{} in {} files", kb(size.bytes), size.files)
}

/// Whether a folder of this size gets the whole body, and the line it is judged by.
pub fn is_full(values: &HashMap<String, String>, size: &Size) -> bool {
    let min_kb = values
        .get("min_project_kb")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1024);
    size.bytes >= min_kb * 1024
}

/// What an agent in a project of `size` is told, from the memory file's `text`.
pub fn render(text: &str, exe: &Path, size: &Size) -> String {
    let (mut values, body) = split(text);
    values.insert("herdr".into(), exe.to_string_lossy().replace('\\', "/"));
    values.insert("project".into(), project_words(size));
    values
        .entry("min_project_kb".into())
        .or_insert_with(|| "1024".into());
    values
        .entry("ask_file_kb".into())
        .or_insert_with(|| "50".into());
    values
        .entry("output_lines".into())
        .or_insert_with(|| "200".into());
    if !is_full(&values, size) {
        return fill(FLAG, &values);
    }
    let largest = size
        .largest
        .iter()
        .map(|(rel, n)| format!("{rel} ({})", kb(*n)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut out = fill(body.trim_start_matches(['\r', '\n']), &values);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!(
        "This project: {}; the largest: {largest}.\n",
        project_words(size)
    ));
    out
}

/// The memory file's text: the owner's, or under test herdr's own, so no test reads or writes the
/// owner's file.
fn text() -> String {
    if cfg!(test) {
        DEFAULT.to_owned()
    } else {
        read_or_create(&path())
    }
}

/// The memory file, there to open: Preload's `e`.
pub fn file() -> PathBuf {
    let path = path();
    if !cfg!(test) {
        read_or_create(&path);
    }
    path
}

/// The Jev memory for agents started in `folder`.
pub fn for_folder(folder: &Path, exe: &Path) -> String {
    let root = crate::nirvana_board::root(folder).unwrap_or_else(|| folder.to_path_buf());
    render(&text(), exe, &measure(&root))
}

/// Preload's detail for the row: whether this folder gets the flag or everything.
pub fn detail(folder: &Path) -> String {
    let root = crate::nirvana_board::root(folder).unwrap_or_else(|| folder.to_path_buf());
    let size = measure(&root);
    let text = text();
    let (values, _) = split(&text);
    let min_kb = values
        .get("min_project_kb")
        .cloned()
        .unwrap_or_else(|| "1024".into());
    if is_full(&values, &size) {
        format!("JEV.md: in full, {} >= {min_kb} KB", kb(size.bytes))
    } else {
        format!("JEV.md: a flag, {} < {min_kb} KB", kb(size.bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(bytes: u64) -> Size {
        Size {
            bytes,
            files: 18,
            largest: vec![("ledgerly/reports.py".into(), 55_038)],
        }
    }

    #[test]
    fn a_small_project_gets_only_the_flag_and_a_big_one_everything() {
        let exe = Path::new(r"C:\h\bin\herdr.exe");
        let small = render(DEFAULT, exe, &size(568 * 1024));
        assert!(small.starts_with("Jev (herdr) is installed"), "{small}");
        assert!(small.contains("\"C:/h/bin/herdr.exe\""));
        assert!(
            small.contains("568 KB in 18 files, under 1024 KB"),
            "{small}"
        );
        assert!(small.contains("over 50 KB") && small.contains("over 200 lines"));
        assert_eq!(small.lines().count(), 1);

        let big = render(DEFAULT, exe, &size(2 * 1024 * 1024));
        for needle in [
            "## Jev (herdr)",
            "\"C:/h/bin/herdr.exe\"",
            "under 20 KB",
            "over 50 KB",
            "over 200 KB in all",
            "over 200 lines",
            "Handoffs:",
            "read first",
            "bug_in_code",
            "read it yourself",
            "This project: 2.0 MB in 18 files; the largest: ledgerly/reports.py (54 KB).",
        ] {
            assert!(big.contains(needle), "{needle}\n{big}");
        }
        assert!(!big.contains('{'), "every size is filled in: {big}");
        assert!(
            !big.contains("min_project_kb"),
            "the front matter stays out"
        );
    }

    #[test]
    fn the_sizes_come_from_the_file_the_owner_edits() {
        let exe = Path::new("herdr");
        let edited = DEFAULT
            .replace("min_project_kb: 1024", "min_project_kb: 100")
            .replace("read_file_kb: 20", "read_file_kb: 7");
        let text = render(&edited, exe, &size(568 * 1024));
        assert!(text.contains("under 7 KB"), "{text}");
        let flag = render(&edited, exe, &size(50 * 1024));
        assert!(flag.contains("under 100 KB"), "{flag}");
    }

    #[test]
    fn the_file_is_written_once_and_then_kept() {
        let dir = std::env::temp_dir().join(format!("nirvana-jev-memory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("jev").join(FILE_NAME);
        assert_eq!(read_or_create(&file), DEFAULT);
        std::fs::write(&file, "mine").expect("edit");
        assert_eq!(read_or_create(&file), "mine");
        let (values, body) = split("mine");
        assert!(values.is_empty());
        assert_eq!(body, "mine");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_is_measured_by_what_git_tracks() {
        let dir = std::env::temp_dir().join(format!("nirvana-jev-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("node_modules")).expect("dir");
        std::fs::write(dir.join("a.py"), vec![b'a'; 3000]).expect("a");
        std::fs::write(dir.join("b.py"), vec![b'b'; 1000]).expect("b");
        std::fs::write(dir.join("shot.PNG"), vec![b'p'; 5000]).expect("png");
        std::fs::write(dir.join("node_modules").join("x.js"), vec![b'x'; 9000]).expect("x");
        // Outside git: the walk skips the folders Jev never reads.
        let walked = measure_uncached(&dir);
        assert_eq!((walked.bytes, walked.files), (4000, 2));
        assert_eq!(walked.largest[0], ("a.py".to_owned(), 3000));
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output()
                .expect("git")
        };
        git(&["init", "-q"]);
        git(&["add", "a.py"]);
        let tracked = measure_uncached(&dir);
        assert_eq!((tracked.bytes, tracked.files), (3000, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
