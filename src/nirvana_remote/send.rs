//! Plan 39 P39b, §2.3: the text box, and the files it carries (prototype screen 5).
//!
//! **Send.** The message goes through `agent.prompt`, the same call `herdr agent prompt` makes: herdr pastes the
//! text into the agent's pane (bracketed, when the pane asked for it) and then presses Enter, through the pane's own
//! input queue (`src/app/api/agents.rs` `submit_agent_prompt`, `queue_user_input_submission`). To the agent it is
//! typing on the PC. While the agent works, the owner picks (plan §2, the failure paragraph):
//! - **queue**: `agent.prompt` as it is. Claude Code takes a message entered during its turn without stopping the
//!   turn: it holds it and hands it over when it next can.
//! - **send now**: Esc first (`agent.send_keys ["esc"]`, which is Claude Code's interrupt), then `agent.wait` for the
//!   agent to leave `working` (at most `STOP_WAIT`), then `agent.prompt`. If it has not stopped by then, the
//!   message is still sent, and so queued, and the answer says so.
//!
//! A blocked agent (a question or a permission prompt on its screen) is not typed into: herdr refuses it
//! (`agent_blocked`), and so does this, before anything is sent; P39c answers prompts.
//!
//! **No lock.** The phone and the PC's keyboard both write into the pane's one input queue, so both land, each in the
//! order it reached herdr; two phones' sends are two `agent.prompt`s. One thing this does not prevent: text the owner
//! has typed on the PC and not yet sent sits in Claude Code's input box, and a message from the phone is pasted
//! after it and sent with it.
//!
//! **Uploads.** A file goes to `<space>/.nirvana/uploads/`, the space's folder being the agent's own working folder
//! as herdr reports it (`cwd`). The name is the server's (`name_for`): the client's file name, path and type are
//! never used for it, so there is no traversal, no Windows reserved name (`CON`, `NUL`, ...), no stream (`:`) and no
//! overwrite (`create_new`). It is written as `<name>.partial` and renamed when the whole body is in. A
//! `.gitignore` holding `*` inside `uploads/` keeps the folder out of git: it works in any folder, in a git
//! repository or not, in a linked worktree (whose `.git` is a file, its `info/exclude` shared with every other
//! worktree of the repository), and it touches no tracked file and nothing under `.git/`. `*` matches the
//! `.gitignore` too, so `git status` stays clean.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The largest upload: 50 MiB (plan §2.3 "Over 50 MB is refused").
pub const UPLOAD_MOST: u64 = 50 * 1024 * 1024;
/// The largest send: the message's JSON.
pub const SEND_MOST: u64 = 64 * 1024;
/// How long "send now" waits for the agent to stop after Esc.
pub const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(8);
/// What the uploads folder holds so git leaves it alone.
pub const IGNORE: &str = "*\n";
const PREFIX: &str = "phone-";

/// The file types an upload is named for; anything else is `.bin`.
const KINDS: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/heic", "heic"),
    ("image/heif", "heif"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("application/pdf", "pdf"),
    ("text/plain", "txt"),
    ("text/csv", "csv"),
    ("application/json", "json"),
    ("application/zip", "zip"),
    ("video/mp4", "mp4"),
    ("video/quicktime", "mov"),
];

fn extension_for(content_type: &str) -> &'static str {
    let base = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    KINDS
        .iter()
        .find(|(kind, _)| *kind == base)
        .map_or("bin", |(_, ext)| ext)
}

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A server name for an upload: `phone-<yyyymmdd>-<hhmmss>-<8 hex>.<ext>`.
pub fn name_for(content_type: &str, now: time::OffsetDateTime) -> String {
    let count = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    let mark = (count.wrapping_mul(0x9E37_79B9) ^ u64::from(nanos) ^ u64::from(std::process::id()))
        & 0xFFFF_FFFF;
    format!(
        "{PREFIX}{:04}{:02}{:02}-{:02}{:02}{:02}-{mark:08x}.{}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        extension_for(content_type)
    )
}

/// Whether `name` is one `name_for` gives: the only names `send` turns into paths.
pub fn is_upload_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(PREFIX) else {
        return false;
    };
    let Some((stem, ext)) = rest.split_once('.') else {
        return false;
    };
    let parts: Vec<&str> = stem.split('-').collect();
    let digits =
        |part: &str, len: usize| part.len() == len && part.bytes().all(|b| b.is_ascii_digit());
    let hex = |part: &str| part.len() == 8 && part.bytes().all(|b| b.is_ascii_hexdigit());
    matches!(parts.as_slice(), [day, time, mark] if digits(day, 8) && digits(time, 6) && hex(mark))
        && (ext == "bin" || KINDS.iter().any(|(_, known)| *known == ext))
}

/// The space's folder: the agent's working folder as herdr reports it.
pub fn folder_of(agent: &serde_json::Value) -> Result<PathBuf, String> {
    let folder = agent["cwd"]
        .as_str()
        .or_else(|| agent["foreground_cwd"].as_str())
        .filter(|folder| !folder.is_empty())
        .ok_or("herdr does not know this agent's folder")?;
    let folder = PathBuf::from(folder);
    if !folder.is_absolute() || !folder.is_dir() {
        return Err("the agent's folder is not there".into());
    }
    Ok(folder)
}

/// A link, a junction, or anything that is not a plain folder.
pub(crate) fn plain_dir(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: a junction reads as a folder to `is_dir`.
        if meta.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    meta.is_dir() && !meta.file_type().is_symlink()
}

/// `<folder>/.nirvana/uploads/`, made when missing, with its `.gitignore`. A link or junction on the way is refused:
/// the file would land somewhere else.
///
/// Two uploads at once may both find the folder missing (P39u, 2026-11-06: the owner picked two photos in a space
/// with no `uploads/` yet; both asked at 01:55:36, one made the folder and the other's `create_dir` failed with
/// "Cannot create a file when that file already exists", a 409 on its chip). So a folder that another upload made
/// in between is taken, and checked as one made here; the `.gitignore` likewise.
pub fn uploads_dir(folder: &Path) -> Result<PathBuf, String> {
    let nirvana = folder.join(crate::nirvana_run::DIR);
    let uploads = nirvana.join("uploads");
    for dir in [&nirvana, &uploads] {
        match std::fs::create_dir(dir) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(format!("{}: {err}", dir.display())),
        }
        if !plain_dir(dir) {
            return Err(format!("{} is not a plain folder", dir.display()));
        }
    }
    let ignore = uploads.join(".gitignore");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ignore)
    {
        Ok(mut file) => file
            .write_all(IGNORE.as_bytes())
            .map_err(|err| format!("{}: {err}", ignore.display()))?,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => return Err(format!("{}: {err}", ignore.display())),
    }
    Ok(uploads)
}

/// Write the body to `<dir>/<name>`, through `<name>.partial`; the size, or the status and why. Nothing is left on
/// a failure.
pub fn save(
    dir: &Path,
    name: &str,
    read: impl FnOnce(&mut dyn Write) -> Result<u64, super::http::BodyError>,
) -> Result<(PathBuf, u64), (u16, String)> {
    let path = dir.join(name);
    let partial = dir.join(format!("{name}.partial"));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|err| (500, format!("the upload could not be made: {err}")))?;
    let written = {
        let mut sink = std::io::BufWriter::with_capacity(256 * 1024, &mut file);
        let read = read(&mut sink);
        read.and_then(|size| {
            sink.flush()
                .map(|()| size)
                .map_err(|err| super::http::BodyError::Write(err.to_string()))
        })
    };
    drop(file);
    match written {
        // Review B5: iOS hands Safari a 0-byte placeholder for a photo still coming down from iCloud; kept, the agent
        // was told to look at an empty image.
        Ok(0) => {
            let _ = std::fs::remove_file(&partial);
            Err((400, "the file is empty".to_string()))
        }
        Ok(size) => match std::fs::rename(&partial, &path) {
            Ok(()) => Ok((path, size)),
            Err(err) => {
                let _ = std::fs::remove_file(&partial);
                Err((500, format!("the upload could not be kept: {err}")))
            }
        },
        Err(error) => {
            let _ = std::fs::remove_file(&partial);
            let why = match &error {
                super::http::BodyError::TooLarge => "over 50 MB".to_string(),
                super::http::BodyError::Timeout => "the upload was too slow".to_string(),
                super::http::BodyError::Bad(why) => (*why).to_string(),
                super::http::BodyError::Write(why) => why.clone(),
            };
            Err((error.status(), why))
        }
    }
}

/// Whether `c` reaches the pane as itself, not as a key (review round, S2). herdr's `agent.prompt` writes the text
/// as it is, inside `ESC[200~ ... ESC[201~` when the pane asked for bracketed paste (`encode_api_text`,
/// `src/app/api_helpers.rs:25`), so an `ESC[201~` in the text ends the paste early and what follows is read as
/// keys: a `\r` submits, an `ESC` interrupts. Every C0 control but `\n` and `\t` goes (`ESC` with them, so no
/// sequence can start), and DEL, and the C1 controls (U+0080-U+009F: U+009B is a one-character CSI to a terminal
/// that reads C1).
fn typed(c: char) -> bool {
    c == '\n' || c == '\t' || !c.is_control()
}

/// The message as typed into the agent: the text with its controls dropped (`typed`), then each file's full path,
/// after a space, in quotes when it has one (a space's folder may: "3D-Printer Flow"). Not on a line of its own: in a
/// pane that did not ask for bracketed paste, that newline would be an Enter, and send the text before its files.
///
/// Plan 40, "phone photos tell the agent what they are": after an image's, a PDF's or a video's path comes one short
/// note in parentheses (`note_for`), naming the command that reads it without pulling the bytes into the agent's own
/// context. The path stays first, so an agent that just reads the file still can. The note is built from the
/// server's own file name and the bytes on disk, never from the phone's text.
pub fn message(text: &str, files: &[PathBuf]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let text: String = text.chars().filter(|&c| typed(c)).collect();
    let text = text.trim();
    if !text.is_empty() {
        parts.push(text.to_string());
    }
    parts.extend(files.iter().map(|file| {
        let path = shown_path(file);
        match note_for(file, &path) {
            Some(note) => format!("{path} ({note})"),
            None => path,
        }
    }));
    parts.join(" ")
}

/// A path as typed: controls dropped, in double quotes when it holds whitespace.
fn shown_path(file: &Path) -> String {
    let path: String = file
        .display()
        .to_string()
        .chars()
        .filter(|&c| typed(c) && c != '\n')
        .collect();
    if path.contains(char::is_whitespace) {
        format!("\"{path}\"")
    } else {
        path
    }
}

/// What a file is, by the extension the server gave it (`name_for`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Media {
    Image,
    /// HEIC and HEIF: Claude's `Read` cannot show them; `herdr look` converts them.
    Heic,
    Pdf,
    Video,
}

fn media_of(file: &Path) -> Option<Media> {
    let ext = file.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" => Some(Media::Image),
        "heic" | "heif" => Some(Media::Heic),
        "pdf" => Some(Media::Pdf),
        "mp4" | "mov" => Some(Media::Video),
        _ => None,
    }
}

/// The note after a media file's path, or `None` for any other file. `path` is the path as typed (quoted when it
/// holds a space), so the command in the note can be pasted as it is.
fn note_for(file: &Path, path: &str) -> Option<String> {
    let media = media_of(file)?;
    // Only a file that is there: the bytes are the server's own reading, not a claim.
    let meta = std::fs::metadata(file).ok().filter(std::fs::Metadata::is_file)?;
    let size = Some(human_bytes(meta.len()));
    let quoted = if path.starts_with('"') {
        path.to_string()
    } else {
        format!("\"{path}\"")
    };
    let look = format!("herdr look {quoted} --ask \"<your question>\"");
    let mut facts: Vec<String> = Vec::new();
    let note = match media {
        Media::Image => {
            facts.push("image".into());
            if let Some((width, height)) = image_size(file) {
                facts.push(format!("{width}x{height} px"));
            }
            facts.extend(size);
            format!("{}; {look} reads it and answers in text", facts.join(", "))
        }
        Media::Heic => {
            facts.push("HEIC image".into());
            facts.extend(size);
            format!(
                "{}; {look} converts it, reads it and answers in text",
                facts.join(", ")
            )
        }
        Media::Pdf => {
            facts.push("PDF".into());
            facts.extend(size);
            format!("{}; {look} reads it and answers in text", facts.join(", "))
        }
        Media::Video => {
            facts.push("video".into());
            facts.extend(size);
            format!(
                "{}; herdr media probe {quoted} gives its length, size and streams",
                facts.join(", ")
            )
        }
    };
    Some(note)
}

/// "512 bytes", "41 KB", "3.1 MB": the size a person reads.
fn human_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} bytes")
    } else if bytes < MB {
        format!("{} KB", bytes.div_ceil(KB))
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

/// How much of an image's start is read for its size. A JPEG's frame header sits after its EXIF block, which one
/// APP1 segment holds, at most 64 KiB by its own length field; twice that leaves room for an APP2 colour profile.
const HEADER_MOST: u64 = 128 * 1024;

/// An image's width and height from its header, when the header can be read cheaply: PNG, GIF, JPEG and WebP.
pub fn image_size(file: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = Vec::new();
    std::fs::File::open(file)
        .ok()?
        .take(HEADER_MOST)
        .read_to_end(&mut head)
        .ok()?;
    header_size(&head)
}

/// The size a header names; `None` for a header this does not read or a cut one.
pub fn header_size(head: &[u8]) -> Option<(u32, u32)> {
    let be16 = |at: usize| -> Option<u32> {
        Some(u32::from(u16::from_be_bytes(
            head.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    let le16 = |at: usize| -> Option<u32> {
        Some(u32::from(u16::from_le_bytes(
            head.get(at..at + 2)?.try_into().ok()?,
        )))
    };
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(head.get(at..at + 4)?.try_into().ok()?))
    };
    let le24 = |at: usize| -> Option<u32> {
        let b = head.get(at..at + 3)?;
        Some(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
    };
    if head.starts_with(b"\x89PNG\r\n\x1a\n") && head.get(12..16) == Some(&b"IHDR"[..]) {
        return Some((be32(16)?, be32(20)?));
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some((le16(6)?, le16(8)?));
    }
    if head.starts_with(b"RIFF") && head.get(8..12) == Some(&b"WEBP"[..]) {
        return match head.get(12..16)? {
            b"VP8 " => Some((le16(26)? & 0x3FFF, le16(28)? & 0x3FFF)),
            b"VP8L" => {
                let bits = u32::from_le_bytes(head.get(21..25)?.try_into().ok()?);
                Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
            }
            b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
            _ => None,
        };
    }
    if head.starts_with(&[0xFF, 0xD8]) {
        let mut at = 2;
        while at + 4 <= head.len() {
            if head[at] != 0xFF {
                return None;
            }
            let marker = head[at + 1];
            if marker == 0xFF {
                at += 1;
                continue;
            }
            // SOF0 to SOF15 carry the frame's size; DHT (C4), JPG (C8) and DAC (CC) share the range and do not.
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                return Some((be16(at + 7)?, be16(at + 5)?));
            }
            at += 2 + usize::try_from(be16(at + 2)?).ok()?;
        }
    }
    None
}
