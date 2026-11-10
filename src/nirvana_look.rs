//! Nirvana fork, plan 40c: `herdr look <file> --ask "<question>"`, one image or PDF read on a cheap seat that comes
//! back as text, so the picture never enters the caller's context (nirvana/plans/2026-11-09-40-records/p40c-look.md).
//!
//! Why: a photo or screenshot named to the orchestrator is read on the dearest seat and stays in its context for the
//! rest of the session. On the parent project that was 118 MiB, 52% of one session, re-sent every turn (Nirvana
//! CLAUDE.md rule 7). Video had `herdr media probe|shrink|prompt`; stills and PDFs had nothing, and the phone page
//! now drops photos into `<space>\.nirvana\uploads\` daily.
//!
//! Measured 2026-11-09 (Haiku 5.5 low, Claude Code 2.1.321): `claude_argv`'s line, run in the image's folder, answered
//! a text-and-count question about a 1024x683 JPEG correctly in 2 turns, 3.8 s, 10.4k tokens of context, $0.00016 at
//! list price. Without `--tools Read` the same read carried 48.9k. Claude Code downsizes a large image to the model's
//! cap (2,576 px long edge, at most 4,784 tokens; about `ceil(w/28) * ceil(h/28)`), so a 1024 px copy costs about
//! 925 tokens and a full one about 4,700: `--detail low` sends the first.
//!
//! A look is not an order: no order file, no gate, no journal. It is still seated by the rule (`nirvana_seat::pick`
//! over the `look.image` class), leased while it runs, recorded when it ends, and its lease is released on every path.
//!
//! The picture is untrusted content (review round, 2026-11-09): a phone screenshot of a web page can carry text
//! written to steer a model. So a look can do exactly one thing, read the copy it was given. It runs only on a claude
//! seat (agy has no tool filter, so a steered agy turn would have a shell), in a folder under the Nirvana home that
//! holds nothing but copies, with `Read(./**)` as its only allowed tool. Measured 2026-11-09 (Haiku 5.5 low, Claude
//! Code 2.1.321), a canary one folder up from the image: with a bare `Read` rule the turn quoted the canary, 0
//! permission denials; with `Read(./**)` and the folder as cwd it answered REFUSED and `permission_denials` had 1
//! entry. Its answer is data for the caller to judge, and the refused reads are counted back to the caller.

use crate::nirvana_seat::{self, Effort, Pick, Request, SizeName, Table, Tokens, World};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const CLASS: &str = "look.image";

/// The class as the seat builder's hunk adds it to `src/nirvana_seats.toml`. Until that merges, `look_table` appends
/// it to a table that lacks it, so `herdr look` works on the owner's installed `seats.toml` too (which keeps its own
/// text and would not get a new class from the default). Scores and bars: the order's numbers, priors.
pub const CLASS_TOML: &str = r#"
[[class]]
id = "look.image"
chain = "media"
blast = "bounded"
effort = "low"
efficient_effort = "low"
sota_effort = "low"
needs = ["image"]
weight = 0.2
bars = { efficient = 70, balanced = 80, sota = 100 }
why = "plan 40c: herdr look, one question about one image or PDF; weight 0.2 is a prior (10.4k tokens measured against P4's 45k); claude seats only: agy has no tool filter and the picture is untrusted (plan 34a: no agy seat teaches)"
seats = [
  { seat = "haiku", score = 84 },
  { seat = "sonnet", score = 86 },
]
"#;

/// The phone page refuses an upload over 50 MB (`chat.js:229`), so nothing it sends is refused here.
pub const MAX_BYTES: u64 = 50 * 1024 * 1024;
/// `--detail low`: about 925 tokens for a 3:2 photo (measured shape above).
pub const LOW_EDGE: u32 = 1024;
/// `--detail high`: the model's own cap, so nothing larger is ever sent.
pub const HIGH_EDGE: u32 = 2576;
/// The most tokens one image costs the model, whatever its size.
pub const TOKEN_CAP: u64 = 4784;
const TILE: u64 = 28;
/// Claude Code's Read takes a PDF of up to 10 pages whole; more needs `pdftoppm` (poppler), not installed here.
pub const PDF_WHOLE_PAGES: u32 = 10;
/// One cheap turn: the measured read took 3.8 s.
pub const TIMEOUT: Duration = Duration::from_secs(120);
/// A look's booking: a look ends within `TIMEOUT`, so an interrupted one (Ctrl-C skips the drop guard, review N5)
/// holds its seat 5 minutes, not the small size's 60.
pub const LEASE_MINUTES: u64 = 5;
/// Copies older than this are deleted when a look runs (review N4). The answer caches are kept.
pub const PRUNE_AFTER: Duration = Duration::from_secs(30 * 24 * 3600);
/// `<project>\.nirvana\look\` for `herdr media shrink`'s copies; `<home>\nirvana\look\<hash>\` for a look's.
pub const DIR: &str = "look";
/// As `uploads\` holds (`nirvana_remote::send::IGNORE`): git leaves the folder alone in any project.
pub const IGNORE: &str = "*\n";
/// A cache hit spends nothing, so its record line is kept off every seat's chain: `tally` keys by chain, and no
/// class is in this one.
pub const CACHE_CHAIN: &str = "look-cache";
pub const POPPLER_FIX: &str =
    "Install Poppler with `winget install oschwartz10612.Poppler` so Claude Code can read long PDFs, then pass --pages in ranges of at most 10.";
/// The demuxers a still copy may be read with. A file named `.png` that is really a concat or HLS playlist would
/// otherwise make ffmpeg read other files, local or remote, into the copy the model sees (review N3). `mov` is
/// HEIC's. Checked with ffmpeg 8.0.1 on this PC: a concat text named `evil.png` is refused, "Format not on whitelist".
pub const STILL_FORMATS: &str = "png_pipe,jpeg_pipe,image2,gif,webp_pipe,bmp_pipe,mov";

// ---- the file ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Png,
    Jpeg,
    Gif,
    Webp,
    Bmp,
    Heic,
    Pdf,
}

impl Kind {
    pub fn is_still(self) -> bool {
        self != Self::Pdf
    }
}

pub const TAKES: &str = "png, jpg, jpeg, gif, webp, bmp, heic, heif or pdf";

/// The kind by extension, or the one-line refusal naming what is taken.
pub fn kind_of(path: &Path) -> Result<Kind, String> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    Ok(match ext.as_str() {
        "png" => Kind::Png,
        "jpg" | "jpeg" => Kind::Jpeg,
        "gif" => Kind::Gif,
        "webp" => Kind::Webp,
        "bmp" => Kind::Bmp,
        "heic" | "heif" => Kind::Heic,
        "pdf" => Kind::Pdf,
        _ => {
            return Err(format!(
                "{} is not an image or a PDF. It takes {TAKES}.",
                path.display()
            ))
        }
    })
}

/// The one-line refusal for a file that cannot be looked at, by size: missing, empty, over 50 MB.
pub fn size_refusal(path: &Path) -> Option<String> {
    let Some(meta) = std::fs::metadata(path).ok().filter(|meta| meta.is_file()) else {
        return Some(format!("No such file: {}.", path.display()));
    };
    match meta.len() {
        0 => Some(format!("{} is empty: 0 bytes.", path.display())),
        len if len > MAX_BYTES => Some(format!(
            "{} is {:.1} MB. herdr look takes 50 MB at most.",
            path.display(),
            len as f64 / (1024.0 * 1024.0)
        )),
        _ => None,
    }
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from(u16::from_le_bytes(
        b.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 3)?;
    Some(u32::from(s[0]) | u32::from(s[1]) << 8 | u32::from(s[2]) << 16)
}

/// Width and height from the file's own header (no image crate in the tree): PNG's IHDR, a JPEG's first SOF, GIF's
/// screen, BMP's DIB header, WebP's VP8, VP8L or VP8X chunk. `None` for a header it cannot read; the caller then asks
/// ffprobe.
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    // A side of 0 is no size at all: a PNG whose IHDR says 0x0 was sent as it was with an estimate of 0 (review N8).
    header_size(bytes).filter(|(w, h)| *w > 0 && *h > 0)
}

fn header_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.get(12..16) == Some(b"IHDR") {
        let w = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
        let h = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
        return Some((w, h));
    }
    if bytes.starts_with(b"GIF8") {
        return Some((le16(bytes, 6)?, le16(bytes, 8)?));
    }
    if bytes.starts_with(b"BM") {
        let dib = u32::from_le_bytes(bytes.get(14..18)?.try_into().ok()?);
        if dib == 12 {
            return Some((le16(bytes, 18)?, le16(bytes, 20)?));
        }
        let w = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
        // A negative height is a top-down bitmap, the same size.
        let h = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
        return Some((w.unsigned_abs(), h.unsigned_abs()));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return match bytes.get(12..16)? {
            b"VP8 " => Some((le16(bytes, 26)? & 0x3fff, le16(bytes, 28)? & 0x3fff)),
            b"VP8L" => {
                let bits = u32::from_le_bytes(bytes.get(21..25)?.try_into().ok()?);
                Some(((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => Some((le24(bytes, 24)? + 1, le24(bytes, 27)? + 1)),
            _ => None,
        };
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let mut at = 2;
        while at + 4 <= bytes.len() {
            if bytes[at] != 0xFF {
                return None;
            }
            let marker = bytes[at + 1];
            match marker {
                0xFF => at += 1,
                0xD0..=0xD9 | 0x01 => at += 2,
                0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                    return Some((be16(bytes, at + 7)?, be16(bytes, at + 5)?));
                }
                _ => at += 2 + be16(bytes, at + 2)? as usize,
            }
        }
    }
    None
}

/// The model's cost of an image this size: `ceil(w/28) * ceil(h/28)`, at most 4,784 (the measured formula above).
pub fn token_estimate(width: u32, height: u32) -> u64 {
    (u64::from(width).div_ceil(TILE) * u64::from(height).div_ceil(TILE)).min(TOKEN_CAP)
}

/// The long edge a copy needs, or `None` when the file is already within `edge` and is read as it is.
pub fn copy_edge(width: u32, height: u32, edge: u32) -> Option<u32> {
    (width.max(height) > edge).then_some(edge)
}

/// The page count read from the file's own objects (`/Type /Page`, never `/Pages`), or `None` when it cannot be
/// known: no page object seen, or object streams (`/ObjStm`, PDF 1.5) that can hide some. An unknown count is
/// treated as over 10 pages.
pub fn pdf_pages(bytes: &[u8]) -> Option<u32> {
    if contains(bytes, b"/ObjStm") {
        return None;
    }
    let mut count = 0u32;
    let mut at = 0;
    while let Some(found) = find(&bytes[at..], b"/Type") {
        let mut next = at + found + 5;
        while bytes.get(next).is_some_and(u8::is_ascii_whitespace) {
            next += 1;
        }
        if bytes[next..].starts_with(b"/Page")
            && !bytes
                .get(next + 5)
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
        {
            count += 1;
        }
        at = next;
    }
    (count > 0).then_some(count)
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len())
        .position(|window| window == needle)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    find(hay, needle).is_some()
}

/// `--pages 3-7` or `--pages 4`: the first and last page, 1-based.
pub fn parse_pages(text: &str) -> Option<(u32, u32)> {
    let (first, last) = match text.trim().split_once('-') {
        Some((a, b)) => (a.trim().parse().ok()?, b.trim().parse().ok()?),
        None => {
            let page = text.trim().parse().ok()?;
            (page, page)
        }
    };
    (first >= 1 && last >= first).then_some((first, last))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `<root>\.nirvana\look\`, made when missing, with a `.gitignore` holding `*` as `uploads\` has.
pub fn look_dir(root: &Path) -> Result<PathBuf, String> {
    let dir = root.join(crate::nirvana_run::DIR).join(DIR);
    std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    let ignore = dir.join(".gitignore");
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
    Ok(dir)
}

/// `<home>\nirvana\look\`: every project's look folder sits in it.
pub fn looks_root() -> PathBuf {
    nirvana_seat::dir().join(DIR)
}

/// A look's folder, `<home>\nirvana\look\<first 12 of sha256 of the project root>\`, made when missing. Not inside
/// the project (review round, departing from the first order's `<project>\.nirvana\look\`): a folder inside a project
/// loads that project's CLAUDE.md and settings into the turn, which costs tokens and lets a project file steer or
/// slow a look. The root is hashed lowercased, as Windows paths compare.
pub fn look_home(root: &Path) -> Result<PathBuf, String> {
    let key = sha256_hex(root.display().to_string().to_lowercase().as_bytes());
    let dir = looks_root().join(&key[..12]);
    std::fs::create_dir_all(&dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    Ok(dir)
}

/// A CLAUDE.md or a `.claude` in the look folder, or in any folder between it and the Nirvana home, that would load
/// into the turn. The folder must hold copies only.
pub fn steering_file(dir: &Path) -> Option<PathBuf> {
    let home = nirvana_seat::dir();
    let top = home.parent().unwrap_or(&home);
    dir.ancestors()
        .take_while(|folder| folder.starts_with(top))
        .flat_map(|folder| {
            ["CLAUDE.md", "CLAUDE.local.md", ".claude"].map(|name| folder.join(name))
        })
        .find(|path| path.exists())
}

/// `<stem>-<first 12 of sha256>.<ext>`: the byte-identical copy of a file that needs no shrinking.
pub fn same_name(input: &Path, sha: &str, kind: Kind) -> String {
    let ext = match kind {
        Kind::Png => "png",
        Kind::Jpeg | Kind::Heic => "jpg",
        Kind::Gif => "gif",
        Kind::Webp => "webp",
        Kind::Bmp => "bmp",
        Kind::Pdf => "pdf",
    };
    format!("{}-{}.{ext}", safe_stem(input), &sha[..12.min(sha.len())])
}

/// `input` copied byte for byte to `dest`, through a temp name and a rename, so a copy cut short is never reused
/// (review N1). A `dest` already there with the input's length is reused: its name carries the input's sha256.
pub fn place_copy(input: &Path, dest: &Path) -> Result<(), String> {
    let len = std::fs::metadata(input)
        .map_err(|err| format!("{}: {err}", input.display()))?
        .len();
    if std::fs::metadata(dest).is_ok_and(|meta| meta.len() == len) {
        return Ok(());
    }
    let part = part_name(dest);
    std::fs::copy(input, &part)
        .and_then(|_| std::fs::rename(&part, dest))
        .map_err(|err| {
            let _ = std::fs::remove_file(&part);
            format!("could not copy {} for the look: {err}", input.display())
        })
}

/// Deletes the copies under `root` older than `keep`, one folder deep, and keeps every `*.look.jsonl` (review N4).
/// How many went.
pub fn prune(root: &Path, now: std::time::SystemTime, keep: Duration) -> usize {
    let Ok(folders) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut gone = 0;
    for folder in folders.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(files) = std::fs::read_dir(folder.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            let is_cache = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".look.jsonl"));
            let old = file
                .metadata()
                .ok()
                .filter(std::fs::Metadata::is_file)
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age > keep);
            if !is_cache && old && std::fs::remove_file(&path).is_ok() {
                gone += 1;
            }
        }
    }
    gone
}

/// The file's stem in `[A-Za-z0-9_-]`, at most 40 characters: no separator, `..` or drive reaches a copy's name, and
/// the name stays plain for the prompt that names it.
fn safe_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect()
}

/// `<stem>-<first 12 of sha256>-<edge>.<ext>`: two files with one name never share a copy, and a copy is made once.
pub fn copy_name(input: &Path, sha: &str, edge: u32, kind: Kind) -> String {
    let stem = safe_stem(input);
    // A JPEG or a phone's HEIC stays a JPEG (a photo); a screenshot, chart or GIF frame keeps its lossless PNG.
    let ext = if matches!(kind, Kind::Jpeg | Kind::Heic) {
        "jpg"
    } else {
        "png"
    };
    format!("{stem}-{}-{edge}.{ext}", &sha[..12.min(sha.len())])
}

/// The `ffmpeg` argv for a still copy with a long edge of at most `edge`, the aspect kept (`-2` keeps the short side
/// even), one frame (a GIF's first). The edge is reckoned by ffmpeg in its own orientation, so a rotated phone JPEG
/// is not squeezed. Checked with ffmpeg 8.0.1: 3000x2000 to 1024x682, 1500x3001 to 512x1024.
pub fn still_argv(input: &Path, output: &Path, edge: u32) -> Vec<String> {
    let mut argv: Vec<String> = [
        "-y",
        "-v",
        "error",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        STILL_FORMATS,
        "-i",
        &input.display().to_string(),
        "-vf",
        &format!("scale='if(gte(iw,ih),min({edge},iw),-2)':'if(gte(iw,ih),-2,min({edge},ih))'"),
        "-frames:v",
        "1",
    ]
    .map(str::to_owned)
    .to_vec();
    if output
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg"))
    {
        argv.extend(["-q:v".to_owned(), "3".to_owned()]);
    }
    argv.push(output.display().to_string());
    argv
}

/// What the seat reads: a copy, or the file itself when it is already small enough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StillCopy {
    pub path: PathBuf,
    pub copied: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// The program runner for ffmpeg and ffprobe: the real one, or a test's fake.
pub type Tool<'a> = &'a dyn Fn(&str, &[String]) -> Result<String, String>;
pub type OnPath<'a> = &'a dyn Fn(&str) -> bool;

/// Width and height from ffprobe, when the header could not be read.
pub fn probed_size(tool: Tool, input: &Path) -> Option<(u32, u32)> {
    let mut argv: Vec<String> = [
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        STILL_FORMATS,
    ]
    .map(str::to_owned)
    .to_vec();
    argv.extend(crate::nirvana_media::probe_argv(input));
    let json: Value = serde_json::from_str(&tool("ffprobe", &argv).ok()?).ok()?;
    let stream = json
        .get("streams")?
        .as_array()?
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))?;
    Some((
        u32::try_from(stream.get("width")?.as_u64()?).ok()?,
        u32::try_from(stream.get("height")?.as_u64()?).ok()?,
    ))
    .filter(|(w, h)| *w > 0 && *h > 0)
}

/// The still a seat reads, at most `edge` on the long side: the file itself when it already fits (never copied), else
/// a copy in `dir` (or `out`), never beside the owner's file unless `out` says so, never over it. A HEIC is always
/// turned into a JPEG copy: no seat reads HEIC.
#[allow(clippy::too_many_arguments)]
pub fn still_copy(
    tool: Tool,
    on_path: OnPath,
    input: &Path,
    kind: Kind,
    edge: u32,
    dir: &Path,
    sha: &str,
    out: Option<&Path>,
) -> Result<StillCopy, String> {
    let bytes = std::fs::read(input).map_err(|err| format!("{}: {err}", input.display()))?;
    let size = match kind {
        Kind::Heic => None,
        _ => image_size(&bytes).or_else(|| {
            on_path("ffprobe")
                .then(|| probed_size(tool, input))
                .flatten()
        }),
    };
    if kind != Kind::Heic {
        let Some((width, height)) = size else {
            return Err(format!(
                "{} has no size herdr can read. Is it really an image?",
                input.display()
            ));
        };
        if copy_edge(width, height, edge).is_none() {
            return Ok(StillCopy {
                path: input.to_path_buf(),
                copied: false,
                width: Some(width),
                height: Some(height),
            });
        }
    }
    if !on_path("ffmpeg") {
        return Err(match (kind, size) {
            (Kind::Heic, _) => format!(
                "{} is HEIC, and ffmpeg is not on PATH to turn it into a JPEG. Install ffmpeg, or send a JPEG.",
                input.display()
            ),
            (_, Some((w, h))) => format!(
                "{} is {w}x{h}, over the {edge} px edge, and ffmpeg is not on PATH to shrink it. Install ffmpeg.",
                input.display()
            ),
            _ => "ffmpeg is not on PATH. Install ffmpeg.".to_owned(),
        });
    }
    let output = match out {
        Some(out) => crate::nirvana_media::absolute_media_path(out),
        None => dir.join(copy_name(input, sha, edge, kind)),
    };
    if crate::nirvana_media::paths_collide(input, &output) {
        return Err(format!(
            "refusing to write the copy over {}",
            input.display()
        ));
    }
    // Only a finished copy ever has the final name: ffmpeg writes a temp name that is renamed when it ends, so a
    // copy cut short by a kill is never reused or cached (review N1).
    let made = std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0) && out.is_none();
    if !made {
        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let part = part_name(&output);
        tool("ffmpeg", &still_argv(input, &part, edge)).map_err(|err| {
            let _ = std::fs::remove_file(&part);
            if kind == Kind::Heic {
                format!(
                    "ffmpeg could not turn {} into a JPEG: {err}",
                    input.display()
                )
            } else {
                err
            }
        })?;
        std::fs::rename(&part, &output).map_err(|err| {
            let _ = std::fs::remove_file(&part);
            format!("ffmpeg left no copy at {}: {err}", part.display())
        })?;
    }
    let read = std::fs::read(&output)
        .map_err(|err| format!("ffmpeg left no copy at {}: {err}", output.display()))?;
    let (width, height) = image_size(&read).unzip();
    Ok(StillCopy {
        path: output,
        copied: true,
        width,
        height,
    })
}

/// `<name>.part<pid>.<ext>` beside `output`: ffmpeg picks the format by the last extension, so it stays.
pub fn part_name(output: &Path) -> PathBuf {
    let ext = output
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("part");
    output.with_extension(format!("part{}.{ext}", std::process::id()))
}

/// ffmpeg or ffprobe run with a list argv, never through a shell, stopped after 60 s (review N2).
pub fn real_tool(program: &str, args: &[String]) -> Result<String, String> {
    crate::nirvana_media::run_list_argv(program, args, Some(crate::nirvana_media::TOOL_TIMEOUT))
}

pub fn real_on_path(program: &str) -> bool {
    crate::nirvana_usage::on_path(program)
}

// ---- the seat ----

/// The project's table with `look.image` in it: the class from `CLASS_TOML` is appended to a table that lacks it.
pub fn look_table(root: &Path) -> Result<Table, String> {
    let table = nirvana_seat::table_for(root)?;
    if table.class(CLASS).is_some() {
        return Ok(table);
    }
    let text = nirvana_seat::read_or_create(&nirvana_seat::dir().join(nirvana_seat::SEATS_FILE));
    let mut with = nirvana_seat::parse(&with_class(&text))
        .map_err(|why| format!("seats.toml with look.image added is refused: {why}"))?;
    with.mode = table.mode;
    Ok(with)
}

/// `text` with `CLASS_TOML` appended when it has no `look.image` class.
pub fn with_class(text: &str) -> String {
    if text.contains(&format!("id = \"{CLASS}\"")) {
        text.to_owned()
    } else {
        format!("{}\n{CLASS_TOML}", text.trim_end())
    }
}

/// Every window of the seat's pool, and the seat's own, has a fresh reading with a figure. A seat whose usage nobody
/// read could be spending a spent pool, so a look fails closed rather than rank it last as an order would.
pub fn fresh(table: &Table, world: &World, seat: &nirvana_seat::Seat) -> bool {
    let Some(pool) = table.pool(&seat.pool) else {
        return false;
    };
    let windows: Vec<&String> = pool.windows.iter().chain(&seat.extra_windows).collect();
    !windows.is_empty()
        && windows.iter().all(|window| {
            world.readings.iter().any(|reading| {
                reading.pool == seat.pool
                    && reading.window == **window
                    && reading.used.is_some()
                    && crate::nirvana_usage::freshness(reading, world.now)
                        == crate::nirvana_usage::Freshness::Fresh
            })
        })
}

/// Which seats a look may take: `--seat`.
#[derive(Debug, Clone, Default)]
pub struct Narrowing {
    pub seat: Option<String>,
}

/// The one CLI a look runs on. claude has a tool filter (`--tools Read`) and a path rule (`Read(./**)`); agy has
/// neither, so a picture that steers an agy turn would have a shell (review S1). An owner's table that lists an agy
/// seat in `look.image` is not obeyed on that seat.
pub const LOOK_CLI: &str = "claude";

/// The rule's pick over the class's seats that this look may take and that have a fresh reading. Everything else
/// (floors, reserves, the mode's bar, signed-out CLIs) is the rule's own.
pub fn choose(
    table: &Table,
    world: &World,
    request: &Request,
    narrow: &Narrowing,
) -> Result<Pick, String> {
    let class = table
        .class(&request.class)
        .ok_or_else(|| format!("no class {} in seats.toml", request.class))?;
    let listed: Vec<&str> = class
        .seats
        .iter()
        .map(|entry| entry.seat.as_str())
        .collect();
    if let Some(seat) = narrow.seat.as_deref().filter(|seat| !listed.contains(seat)) {
        return Err(format!(
            "seat {seat} does not take {CLASS}. It lists {}.",
            listed.join(", ")
        ));
    }
    if let Some(seat) = narrow
        .seat
        .as_deref()
        .and_then(|id| table.seats.iter().find(|seat| seat.id == id))
        .filter(|seat| seat.cli != LOOK_CLI)
    {
        return Err(format!(
            "seat {} runs {}, which has no tool filter. A look runs only on a {LOOK_CLI} seat.",
            seat.id, seat.cli
        ));
    }
    let mut narrowed = table.clone();
    let mut any = false;
    let mut stale = Vec::new();
    for seat in narrowed.seats.iter_mut().filter(|seat| seat.enabled) {
        let taken = listed.contains(&seat.id.as_str())
            && seat.cli == LOOK_CLI
            && narrow.seat.as_ref().is_none_or(|only| *only == seat.id);
        if !taken {
            seat.enabled = false;
            continue;
        }
        any = true;
        if !fresh(table, world, seat) {
            stale.push(seat.id.clone());
            seat.enabled = false;
        }
    }
    if !any {
        return Err("no enabled seat can take this look".to_owned());
    }
    let left = narrowed
        .seats
        .iter()
        .any(|seat| seat.enabled && listed.contains(&seat.id.as_str()));
    if !left {
        return Err(format!(
            "no seat for {CLASS} has a fresh usage reading ({}). `herdr usage` reads them. Nothing was spent.",
            stale.join(", ")
        ));
    }
    nirvana_seat::pick(&narrowed, world, request).map_err(|no_seat| {
        let mut why = no_seat.reason;
        if !stale.is_empty() {
            why.push_str(&format!("; no fresh usage reading: {}", stale.join(", ")));
        }
        why
    })
}

/// Releases the look's lease when it goes out of scope, whatever path it leaves by.
struct LeaseGuard {
    order: String,
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        let dir = nirvana_seat::dir();
        if let Ok(_lock) = nirvana_seat::Lock::take(&dir) {
            let path = dir.join(nirvana_seat::LEASES_FILE);
            // A leases.json that does not parse is left as it is (O2 fix round); the lease then expires.
            if let Ok(mut leases) = nirvana_seat::load_leases(&path) {
                if leases.release(&self.order, crate::nirvana_run::now()) {
                    let _ = nirvana_seat::write_leases(&path, &leases);
                }
            }
        }
    }
}

/// Picks under `leases.lock` and books the look, as `seat pick --book` does.
fn book(
    table: &Table,
    world: &World,
    request: &Request,
    narrow: &Narrowing,
    order: &str,
) -> Result<(Pick, LeaseGuard), String> {
    let dir = nirvana_seat::dir();
    let _lock = nirvana_seat::Lock::take(&dir).map_err(|err| err.to_string())?;
    let path = dir.join(nirvana_seat::LEASES_FILE);
    let mut leases = nirvana_seat::load_leases(&path)?;
    let mut world = world.clone();
    world.now = crate::nirvana_run::now();
    world.leases = leases.leases.clone();
    let pick = choose(table, &world, request, narrow)?;
    // The booking's length comes from the size; a look's is its own (`LEASE_MINUTES`, review N5).
    let mut short = table.clone();
    short.size.small.lease_minutes = LEASE_MINUTES;
    let (lease, _) =
        nirvana_seat::book_pick(&mut leases, &short, &pick, Some(order), None, world.now);
    if lease.is_none() {
        return Err(format!("{order} was not booked"));
    }
    nirvana_seat::write_leases(&path, &leases).map_err(|err| err.to_string())?;
    Ok((
        pick,
        LeaseGuard {
            order: order.to_owned(),
        },
    ))
}

// ---- the launch ----

/// The allow rule: Read of the cwd and below, the look folder that holds only copies. A bare `Read` permitted every
/// path the user can read; the measured canary was quoted with it (module docs). Spelled exactly as measured.
pub const READ_RULE: &str = "Read(./**)";

/// The measured line (module docs): one Read scoped to the cwd, no MCP, no user settings, no saved session, plain
/// JSON out. Never a schema: a Work Report would be forced on a one-line answer.
pub fn claude_argv(model: &str, effort: Effort, prompt: &str) -> Vec<String> {
    [
        "claude",
        "-p",
        prompt,
        "--model",
        model,
        "--effort",
        effort.word(),
        "--output-format",
        "json",
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--exclude-dynamic-system-prompt-sections",
        "--tools",
        "Read",
        "--allowedTools",
        READ_RULE,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// The claude seat's question. It names the copy by its file name in the cwd, as the measured scoped run did: an
/// absolute path would point the reader outside the folder its rule allows.
pub fn claude_prompt(name: &str, ask: &str, pages: Option<(u32, u32)>, kind: Kind) -> String {
    let what = if kind == Kind::Pdf { "PDF" } else { "image" };
    let pages = pages.map_or(String::new(), |(first, last)| {
        format!(" Read only pages {first}-{last}, with the Read tool's pages option.")
    });
    format!(
        "Read the {what} \"{name}\" in the current folder with the Read tool.{pages} Then answer in plain text, in a \
         few short lines, giving numbers and copying visible text exactly: {}",
        one_line(ask)
    )
}

pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(windows)]
mod job {
    //! A kill-on-close job, so a timed-out look ends with everything it started. The worker's own (`mod job` in
    //! nirvana_worker.rs) is private there and this module may not edit it: this is that one's duplicate, without its
    //! suspended start. The child is put in the job right after it starts, before a CLI that loads for seconds has
    //! started anything (`nirvana_job.rs` does the same).
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct Job(HANDLE);

    impl Job {
        pub fn new() -> std::io::Result<Self> {
            let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if job.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let job = Self(job);
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(job)
        }

        pub fn adopt(&self, child: &std::process::Child) -> std::io::Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle().cast()) } == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn kill(&self) {
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(not(windows))]
mod job {
    pub struct Job;
    impl Job {
        pub fn new() -> std::io::Result<Self> {
            Ok(Self)
        }
        pub fn adopt(&self, _child: &std::process::Child) -> std::io::Result<()> {
            Ok(())
        }
        pub fn kill(&self) {}
    }
}

#[derive(Debug)]
pub enum End {
    Exited {
        stdout: String,
        stderr: String,
        code: Option<i32>,
    },
    TimedOut,
}

pub(crate) fn drain(
    pipe: Option<impl std::io::Read + Send + 'static>,
) -> std::sync::mpsc::Receiver<String> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        // Lossy: one non-UTF-8 byte must not end the read (review B3 on the worker's reader).
        let _ = send.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    receive
}

/// Starts `program` in its job in `cwd` and waits up to `timeout`, then kills the whole job.
pub fn launch(program: &[String], cwd: &Path, cli: &str, timeout: Duration) -> Result<End, String> {
    let (first, rest) = program
        .split_first()
        .ok_or_else(|| "nothing to start".to_owned())?;
    let mut command = Command::new(first);
    command
        .args(rest)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Not a child session of the orchestrator's Claude, as a worker is not.
    for key in crate::nirvana_worker::ENV_REMOVED {
        command.env_remove(key);
    }
    if cli == "claude" {
        for (key, value) in crate::nirvana_worker::CLAUDE_ENV {
            command.env(key, value);
        }
    }
    crate::platform::configure_background_command(&mut command);
    let job = job::Job::new().map_err(|err| format!("no job object: {err}"))?;
    let mut child = command
        .spawn()
        .map_err(|err| format!("{first} did not start: {err}"))?;
    if let Err(err) = job.adopt(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("{first} could not be put in its job: {err}"));
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(err) => {
                job.kill();
                return Err(format!("{first}: {err}"));
            }
        }
        if Instant::now() >= deadline {
            job.kill();
            let _ = child.kill();
            let _ = child.wait();
            return Ok(End::TimedOut);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // A grandchild left holding the pipe is ended with the job; the read then finishes.
    job.kill();
    let wait = Duration::from_secs(5);
    Ok(End::Exited {
        stdout: stdout.recv_timeout(wait).unwrap_or_default(),
        stderr: stderr.recv_timeout(wait).unwrap_or_default(),
        code: status.code(),
    })
}

/// What a look's turn gave back.
#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub text: String,
    pub tokens: Tokens,
    /// Reads the path rule refused: the picture asked the reader for other files.
    pub refused_reads: u64,
}

/// The answer, its tokens and its refused reads from what claude printed, or why there is none. claude's
/// `--output-format json` prints one JSON array of events whose last `result` holds the answer and the
/// `permission_denials` array (measured); the tokens are read by the worker's own `Stream`.
pub fn answer_of(stdout: &str) -> Result<Answer, String> {
    let mut stream = crate::nirvana_worker::Stream::new(LOOK_CLI);
    let value: Value =
        serde_json::from_str(stdout.trim()).map_err(|_| "claude printed no JSON".to_owned())?;
    let events = match value {
        Value::Array(events) => events,
        other => vec![other],
    };
    let result = events
        .iter()
        .rev()
        .find(|event| event.get("type").and_then(Value::as_str) == Some("result"))
        .ok_or_else(|| "claude printed no result".to_owned())?;
    stream.feed(&result.to_string());
    let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);
    let subtype = result.get("subtype").and_then(Value::as_str).unwrap_or("");
    if is_error || subtype != "success" {
        return Err(format!(
            "claude ended with {subtype:?}, is_error {is_error}"
        ));
    }
    let text = result
        .get("result")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if text.is_empty() {
        return Err("an empty answer, though the CLI said it succeeded".to_owned());
    }
    let refused_reads = result
        .get("permission_denials")
        .and_then(Value::as_array)
        .map_or(0, |denials| denials.len() as u64);
    Ok(Answer {
        text,
        tokens: stream.tokens,
        refused_reads,
    })
}

/// The line the plain output ends with when the path rule refused a read.
pub fn refused_line(refused: u64, kind: Kind) -> Option<String> {
    let what = if kind == Kind::Pdf { "PDF" } else { "image" };
    match refused {
        0 => None,
        1 => Some(format!(
            "The {what} asked for 1 other file; it was refused."
        )),
        n => Some(format!(
            "The {what} asked for {n} other files; they were refused."
        )),
    }
}

// ---- the cache ----

/// One answer, one line of `<name>.look.jsonl` in the look folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheLine {
    pub sha256: String,
    pub question: String,
    pub detail: String,
    #[serde(default)]
    pub pages: Option<String>,
    pub seat: String,
    pub model: String,
    pub tokens: Tokens,
    pub answer: String,
    pub read: String,
    pub at: u64,
    #[serde(default)]
    pub refused_reads: u64,
}

pub fn cache_path(dir: &Path, input: &Path) -> PathBuf {
    let name = input
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    dir.join(format!("{name}.look.jsonl"))
}

/// The last answer for this file's bytes, question, detail and pages, and from `seat` when one is named. A named seat
/// is a request for that seat's reading, often a second opinion, so another seat's answer is not it (review N7);
/// without `--seat` any seat's answer serves, and `--json` says which seat gave it.
pub fn cached(
    path: &Path,
    sha: &str,
    question: &str,
    detail: &str,
    pages: Option<&str>,
    seat: Option<&str>,
) -> Option<CacheLine> {
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<CacheLine>(line.trim()).ok())
        .find(|line| {
            line.sha256 == sha
                && line.question == question
                && line.detail == detail
                && line.pages.as_deref() == pages
                && seat.is_none_or(|seat| line.seat == seat)
        })
}

fn append_cache(path: &Path, line: &CacheLine) -> std::io::Result<()> {
    let mut text = serde_json::to_string(line)?;
    text.push('\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(text.as_bytes())
}

// ---- the command ----

#[derive(Debug, Clone, Default)]
pub struct Args {
    pub file: PathBuf,
    pub ask: String,
    pub seat: Option<String>,
    pub high: bool,
    pub pages: Option<String>,
    pub json: bool,
    pub fresh: bool,
}

pub const USAGE: &str =
    "usage: herdr look <file> --ask \"<question>\" [--seat <id>] [--detail low|high] [--pages a-b] [--json] [--fresh]";

pub fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args::default();
    let mut file = None;
    let mut ask = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().ok_or_else(|| USAGE.to_owned());
        match arg.as_str() {
            "--ask" => ask = Some(value()?),
            "--seat" => parsed.seat = Some(value()?),
            "--detail" => {
                parsed.high = match value()?.as_str() {
                    "low" => false,
                    "high" => true,
                    _ => return Err("--detail is low or high.".to_owned()),
                }
            }
            "--pages" => parsed.pages = Some(value()?),
            "--json" => parsed.json = true,
            "--fresh" => parsed.fresh = true,
            flag if flag.starts_with('-') => return Err(USAGE.to_owned()),
            path if file.is_none() => file = Some(PathBuf::from(path)),
            _ => return Err(USAGE.to_owned()),
        }
    }
    let (Some(file), Some(ask)) = (file, ask) else {
        return Err(USAGE.to_owned());
    };
    parsed.file = file;
    parsed.ask = ask;
    Ok(parsed)
}

/// What a look needs from the machine; the tests give fakes.
pub struct Env<'a> {
    pub root: PathBuf,
    /// `None` reads the machine's (`nirvana_seat::live_world`).
    pub world: Option<World>,
    /// A test's stand-in for the CLI: started in place of the argv the outcome names.
    pub stand_in: Option<Vec<String>>,
    pub timeout: Duration,
    pub tool: Tool<'a>,
    pub on_path: OnPath<'a>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Outcome {
    pub answer: String,
    pub seat: String,
    pub model: String,
    pub effort: Effort,
    pub tokens: Tokens,
    pub cached: bool,
    /// The copy the seat read, in the look folder.
    pub read: String,
    pub kind: Kind,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// What the image costs the model (`token_estimate`); none for a PDF.
    pub estimate_tokens: Option<u64>,
    pub pages: Option<u32>,
    /// Reads outside the look folder that the path rule refused. Not 0 means the picture tried to steer its reader.
    pub refused_reads: u64,
    pub order: String,
    /// What was started and where: the tests read them; `--json` leaves them out (the prompt is long).
    #[serde(skip)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub argv: Vec<String>,
    #[serde(skip)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub cwd: PathBuf,
}

/// A refusal: its exit code and its one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub code: i32,
    pub line: String,
}

fn refuse(code: i32) -> impl Fn(String) -> Refused {
    move |line| Refused { code, line }
}

fn record(table: &Table, pick: &Pick, order: &str, outcome: &str, tokens: Tokens, extra: Value) {
    let now = crate::nirvana_run::now();
    let line = serde_json::json!({
        "order": order,
        "seat": pick.seat,
        "model": pick.model,
        "effort": pick.effort,
        "class": CLASS,
        "outcome": outcome,
        "tokens": tokens,
        "ended_at": now,
        "look": extra,
    });
    let Ok(line) = serde_json::from_value::<nirvana_seat::RecordLine>(line) else {
        return;
    };
    let Ok(line) = nirvana_seat::complete(table, line, now) else {
        return;
    };
    // `release: false`: the line is filled from the live booking here, and `LeaseGuard` is the one place the lease
    // ends, on every path (a single release point is what its control can prove).
    if let Err(err) = nirvana_seat::record_once(&nirvana_seat::dir(), line, false, now) {
        eprintln!("herdr look: the record line was not written: {err}");
    }
}

/// A cache hit's line: no lease, no spend, its own chain.
fn record_hit(table: &Table, hit: &CacheLine, order: &str, effort: Effort) {
    let now = crate::nirvana_run::now();
    let line = serde_json::json!({
        "order": order,
        "seat": hit.seat,
        "model": hit.model,
        "effort": effort,
        "class": CLASS,
        "chain": CACHE_CHAIN,
        "outcome": "pass",
        "tokens": Tokens::default(),
        "use": {},
        "ended_at": now,
        "look": {"cached": true, "detail": hit.detail},
    });
    if let Ok(line) = serde_json::from_value::<nirvana_seat::RecordLine>(line) {
        if let Ok(line) = nirvana_seat::complete(table, line, now) {
            let _ = nirvana_seat::record_once(&nirvana_seat::dir(), line, false, now);
        }
    }
}

/// Unique per look: the record refuses a second line for one order and a booking is idempotent per order, so two
/// looks of one file in one second in one process (a look, then its cache hit) must not share an id. Found by the
/// cache test: the hit's line was refused as a duplicate.
fn order_id(sha: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "look-{}-{}-{}-{}",
        &sha[..8.min(sha.len())],
        crate::nirvana_run::now(),
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

/// One look, start to end.
pub fn look(args: &Args, env: &Env) -> Result<Outcome, Refused> {
    let input = crate::nirvana_media::absolute_media_path(&args.file);
    let kind = kind_of(&input).map_err(refuse(2))?;
    if let Some(line) = size_refusal(&input) {
        return Err(refuse(2)(line));
    }
    let question = one_line(&args.ask);
    if question.is_empty() {
        return Err(refuse(2)("--ask needs a question.".to_owned()));
    }
    let pages = match &args.pages {
        Some(text) => Some(parse_pages(text).ok_or_else(|| {
            refuse(2)(format!(
                "--pages is a page or a range such as 3-7, not {text:?}."
            ))
        })?),
        None => None,
    };
    if pages.is_some() && kind != Kind::Pdf {
        return Err(refuse(2)("--pages is for a PDF.".to_owned()));
    }
    // Claude Code's Read takes at most 10 pages of a PDF at a time.
    if let Some((first, last)) = pages.filter(|(first, last)| last - first >= PDF_WHOLE_PAGES) {
        return Err(refuse(2)(format!(
            "--pages takes at most {PDF_WHOLE_PAGES} pages at a time, not {first}-{last}. Ask again per range."
        )));
    }
    let pages_word = pages.map(|(first, last)| format!("{first}-{last}"));
    let bytes =
        std::fs::read(&input).map_err(|err| refuse(2)(format!("{}: {err}", input.display())))?;
    let sha = sha256_hex(&bytes);
    // A PDF over 10 pages, or of unknown length, needs pdftoppm for Claude Code's Read, and a page range. No agy seat
    // takes it instead (review S1), so it is refused before anything is spent.
    let page_count = (kind == Kind::Pdf).then(|| pdf_pages(&bytes)).flatten();
    let long_pdf = kind == Kind::Pdf && page_count.is_none_or(|count| count > PDF_WHOLE_PAGES);
    let length = page_count.map_or("an unknown number of pages".to_owned(), |count| {
        format!("{count} pages")
    });
    if long_pdf && !(env.on_path)("pdftoppm") {
        return Err(refuse(2)(format!(
            "{POPPLER_FIX} This PDF has {length}, over the {PDF_WHOLE_PAGES} Claude Code reads without it."
        )));
    }
    if long_pdf && pages.is_none() {
        return Err(refuse(2)(format!(
            "Pass --pages in ranges of at most {PDF_WHOLE_PAGES}: this PDF has {length}."
        )));
    }
    let dir = look_home(&env.root).map_err(refuse(2))?;
    let _ = prune(&looks_root(), std::time::SystemTime::now(), PRUNE_AFTER);
    let detail = if args.high { "high" } else { "low" };
    let table = look_table(&env.root).map_err(refuse(2))?;
    let cache = cache_path(&dir, &input);
    if !args.fresh {
        if let Some(hit) = cached(
            &cache,
            &sha,
            &question,
            detail,
            pages_word.as_deref(),
            args.seat.as_deref(),
        ) {
            let order = order_id(&sha);
            record_hit(&table, &hit, &order, Effort::Low);
            return Ok(Outcome {
                answer: hit.answer.clone(),
                seat: hit.seat.clone(),
                model: hit.model.clone(),
                effort: Effort::Low,
                tokens: hit.tokens,
                cached: true,
                read: hit.read.clone(),
                kind,
                width: None,
                height: None,
                estimate_tokens: None,
                pages: None,
                refused_reads: hit.refused_reads,
                order,
                argv: Vec::new(),
                cwd: PathBuf::new(),
            });
        }
    }

    // The file the seat reads: always a copy in the look folder, shrunk or byte for byte, so the copy is the only
    // thing the reader's path rule reaches (review S2).
    let edge = if args.high { HIGH_EDGE } else { LOW_EDGE };
    let (read, width, height) = if kind.is_still() {
        let copy = still_copy(env.tool, env.on_path, &input, kind, edge, &dir, &sha, None)
            .map_err(refuse(2))?;
        if copy.copied {
            (copy.path, copy.width, copy.height)
        } else {
            let dest = dir.join(same_name(&input, &sha, kind));
            place_copy(&input, &dest).map_err(refuse(2))?;
            (dest, copy.width, copy.height)
        }
    } else {
        let dest = dir.join(same_name(&input, &sha, kind));
        place_copy(&input, &dest).map_err(refuse(2))?;
        (dest, None, None)
    };
    let estimate = width.zip(height).map(|(w, h)| token_estimate(w, h));
    if let Some(found) = steering_file(&dir) {
        return Err(refuse(2)(format!(
            "{} would load into the look's turn. The look folder must hold copies only; remove it.",
            found.display()
        )));
    }

    // The seat.
    let world = env
        .world
        .clone()
        .unwrap_or_else(|| nirvana_seat::live_world(&table));
    let order = order_id(&sha);
    let mut request = Request::new(CLASS);
    request.size = SizeName::Small;
    request.mode = table.mode;
    request.order = Some(order.clone());
    request.needs = table
        .class(CLASS)
        .map(|class| class.needs.clone())
        .unwrap_or_default();
    if kind == Kind::Pdf {
        request.needs.push("pdf".to_owned());
    }
    let narrow = Narrowing {
        seat: args.seat.clone(),
    };
    let (pick, guard) = book(&table, &world, &request, &narrow, &order)
        .map_err(|why| refuse(nirvana_seat::NO_SEAT_EXIT)(format!("No seat: {why}.")))?;

    // The launch: in the look folder, the copy named by its file name.
    let name = read
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    let argv = claude_argv(
        &pick.model,
        pick.effort,
        &claude_prompt(&name, &question, pages, kind),
    );
    let cwd = dir.clone();
    let program = env.stand_in.clone().unwrap_or_else(|| argv.clone());
    let extra = serde_json::json!({"cached": false, "detail": detail, "kind": kind});
    let end = launch(&program, &cwd, &pick.cli, env.timeout);
    let answered = match end {
        Ok(End::Exited {
            stdout,
            stderr,
            code,
        }) => answer_of(&stdout).map_err(|why| {
            let tail: String = stderr
                .trim()
                .lines()
                .last()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect();
            format!(
                "{} {} gave no answer: {why} (exit {}){}",
                pick.seat,
                pick.model,
                code.map_or("none".to_owned(), |code| code.to_string()),
                if tail.is_empty() {
                    String::new()
                } else {
                    format!(": {tail}")
                }
            )
        }),
        Ok(End::TimedOut) => Err(format!(
            "{} {} gave no answer in {} s; it was stopped.",
            pick.seat,
            pick.model,
            env.timeout.as_secs()
        )),
        Err(why) => Err(why),
    };
    let answer = match answered {
        Ok(answer) => answer,
        Err(why) => {
            record(&table, &pick, &order, "failed", Tokens::default(), extra);
            drop(guard);
            return Err(refuse(1)(why));
        }
    };
    let mut extra = extra;
    extra["refused_reads"] = answer.refused_reads.into();
    record(&table, &pick, &order, "pass", answer.tokens, extra);
    drop(guard);
    let _ = append_cache(
        &cache,
        &CacheLine {
            sha256: sha,
            question,
            detail: detail.to_owned(),
            pages: pages_word,
            seat: pick.seat.clone(),
            model: pick.model.clone(),
            tokens: answer.tokens,
            answer: answer.text.clone(),
            read: read.display().to_string(),
            at: crate::nirvana_run::now(),
            refused_reads: answer.refused_reads,
        },
    );
    Ok(Outcome {
        answer: answer.text,
        seat: pick.seat,
        model: pick.model,
        effort: pick.effort,
        tokens: answer.tokens,
        cached: false,
        read: read.display().to_string(),
        kind,
        width,
        height,
        estimate_tokens: estimate,
        pages: page_count,
        refused_reads: answer.refused_reads,
        order,
        argv,
        cwd,
    })
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(line) => {
            eprintln!("{line}");
            std::process::exit(2);
        }
    };
    let env = Env {
        root: crate::nirvana_run::Project::from_env().root,
        world: None,
        stand_in: None,
        timeout: TIMEOUT,
        tool: &real_tool,
        on_path: &real_on_path,
    };
    match look(&parsed, &env) {
        Ok(outcome) if parsed.json => {
            println!("{}", serde_json::to_string_pretty(&outcome)?);
            Ok(())
        }
        Ok(outcome) => {
            println!("{}", plain_output(&outcome));
            Ok(())
        }
        Err(refused) => {
            eprintln!("herdr look: {}", refused.line);
            std::process::exit(refused.code);
        }
    }
}

/// The answer, and one last line when the path rule refused a read.
pub fn plain_output(outcome: &Outcome) -> String {
    match refused_line(outcome.refused_reads, outcome.kind) {
        Some(line) => format!("{}\n{line}", outcome.answer),
        None => outcome.answer.clone(),
    }
}

#[cfg(test)]
mod tests;
