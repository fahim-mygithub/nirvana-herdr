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
why = "plan 40c: herdr look, one question about one image or PDF; weight 0.2 is a prior (10.4k tokens measured against P4's 45k)"
seats = [
  { seat = "haiku", score = 84 },
  { seat = "flash", score = 84 },
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
/// agy stops itself before herdr's limit kills it, so its own result line still arrives.
const AGY_PRINT_TIMEOUT_S: u64 = 110;
/// `<project>\.nirvana\look\`: the copies and the answer cache.
pub const DIR: &str = "look";
/// As `uploads\` holds (`nirvana_remote::send::IGNORE`): git leaves the folder alone in any project.
pub const IGNORE: &str = "*\n";
/// A cache hit spends nothing, so its record line is kept off every seat's chain: `tally` keys by chain, and no
/// class is in this one.
pub const CACHE_CHAIN: &str = "look-cache";
pub const POPPLER_FIX: &str =
    "`winget install oschwartz10612.Poppler` or an Antigravity sign-in fixes it";

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

/// `<stem>-<first 12 of sha256>-<edge>.<ext>`: two files with one name never share a copy, and a copy is made once.
pub fn copy_name(input: &Path, sha: &str, edge: u32, kind: Kind) -> String {
    let stem: String = input
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
        .collect();
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
    let json: Value =
        serde_json::from_str(&tool("ffprobe", &crate::nirvana_media::probe_argv(input)).ok()?)
            .ok()?;
    let stream = json
        .get("streams")?
        .as_array()?
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"))?;
    Some((
        u32::try_from(stream.get("width")?.as_u64()?).ok()?,
        u32::try_from(stream.get("height")?.as_u64()?).ok()?,
    ))
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
    let made = std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0) && out.is_none();
    if !made {
        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        tool("ffmpeg", &still_argv(input, &output, edge)).map_err(|err| {
            if kind == Kind::Heic {
                format!(
                    "ffmpeg could not turn {} into a JPEG: {err}",
                    input.display()
                )
            } else {
                err
            }
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

/// ffmpeg or ffprobe run with a list argv, never through a shell.
pub fn real_tool(program: &str, args: &[String]) -> Result<String, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::platform::configure_background_command(&mut command);
    let output = command
        .output()
        .map_err(|err| format!("{program} did not start: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("{program} failed: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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

/// Which seats a look may take: `--seat`, and a CLI when the PDF route asks for one.
#[derive(Debug, Clone, Default)]
pub struct Narrowing {
    pub seat: Option<String>,
    pub cli: Option<String>,
}

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
    let mut narrowed = table.clone();
    let mut any = false;
    let mut stale = Vec::new();
    for seat in narrowed.seats.iter_mut().filter(|seat| seat.enabled) {
        let taken = listed.contains(&seat.id.as_str())
            && narrow.seat.as_ref().is_none_or(|only| *only == seat.id)
            && narrow.cli.as_ref().is_none_or(|cli| *cli == seat.cli);
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
    let (lease, _) =
        nirvana_seat::book_pick(&mut leases, table, &pick, Some(order), None, world.now);
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

/// The measured line (module docs): one Read, no MCP, no user settings, no saved session, plain JSON out. Never a
/// schema: a Work Report would be forced on a one-line answer.
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
        "Read",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// agy reads the file with `view_file` (probe P2's shape, `nirvana_media::still_prompt_line`); the effort is in the
/// model id. The flags are the worker's (`nirvana_worker::argv`) without `--json-schema`: no schema here either.
pub fn agy_argv(model: &str, prompt: &str) -> Vec<String> {
    vec![
        "agy".to_owned(),
        "--output-format".to_owned(),
        "stream-json".to_owned(),
        "--model".to_owned(),
        model.to_owned(),
        "--print-timeout".to_owned(),
        format!("{AGY_PRINT_TIMEOUT_S}s"),
        // As the worker: agy's auto mode needs the terminal sandbox, which is off on this machine (plan 35 §3.6).
        "--dangerously-skip-permissions".to_owned(),
        format!("--print={prompt}"),
    ]
}

/// The claude seat's question, naming the full path as the measured run did.
pub fn claude_prompt(path: &Path, ask: &str, pages: Option<(u32, u32)>, kind: Kind) -> String {
    let what = if kind == Kind::Pdf { "PDF" } else { "image" };
    let pages = pages.map_or(String::new(), |(first, last)| {
        format!(" Read only pages {first}-{last}, with the Read tool's pages option.")
    });
    format!(
        "Read the {what} at \"{}\" with the Read tool.{pages} Then answer in plain text, in a few short lines, giving \
         numbers and copying visible text exactly: {}",
        path.display(),
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

fn drain(pipe: Option<impl std::io::Read + Send + 'static>) -> std::sync::mpsc::Receiver<String> {
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

/// The answer and its tokens from what the CLI printed, or why there is none. claude's `--output-format json` prints
/// one JSON array of events whose last `result` holds the answer (measured); agy prints stream-json lines. Both are
/// read by the worker's own `Stream`.
pub fn answer_of(cli: &str, stdout: &str) -> Result<(String, Tokens), String> {
    let mut stream = crate::nirvana_worker::Stream::new(cli);
    if cli == "claude" {
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
    } else {
        for line in stdout.lines() {
            stream.feed(line);
        }
    }
    let result = stream
        .result
        .clone()
        .ok_or_else(|| format!("{cli} printed no result"))?;
    let (failed, text) = if cli == "claude" {
        let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);
        let subtype = result.get("subtype").and_then(Value::as_str).unwrap_or("");
        (
            (is_error || subtype != "success")
                .then(|| format!("claude ended with {subtype:?}, is_error {is_error}")),
            result
                .get("result")
                .and_then(Value::as_str)
                .map(str::to_owned),
        )
    } else {
        let status = result.get("status").and_then(Value::as_str).unwrap_or("");
        let reason = result.get("finished_reason").and_then(Value::as_str);
        (
            (status != "SUCCESS"
                || reason.is_some_and(|why| crate::nirvana_worker::AGY_UNFINISHED.contains(&why)))
            .then(|| format!("agy status {status:?}, finished {reason:?}")),
            result
                .get("response")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| stream.last_text.clone()),
        )
    };
    if let Some(why) = failed {
        return Err(why);
    }
    let text = text.unwrap_or_default().trim().to_owned();
    if text.is_empty() {
        // Probe P2: agy ended a turn that never opened the file with an empty answer and status SUCCESS.
        return Err("an empty answer, though the CLI said it succeeded".to_owned());
    }
    Ok((text, stream.tokens))
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
}

pub fn cache_path(dir: &Path, input: &Path) -> PathBuf {
    let name = input
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    dir.join(format!("{name}.look.jsonl"))
}

/// The last answer for this file's bytes, question, detail and pages.
pub fn cached(
    path: &Path,
    sha: &str,
    question: &str,
    detail: &str,
    pages: Option<&str>,
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
    /// The file the seat read: the copy, or the file itself.
    pub read: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// What the image costs the model (`token_estimate`); none for a PDF.
    pub estimate_tokens: Option<u64>,
    pub pages: Option<u32>,
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
    let pages_word = pages.map(|(first, last)| format!("{first}-{last}"));
    let bytes =
        std::fs::read(&input).map_err(|err| refuse(2)(format!("{}: {err}", input.display())))?;
    let sha = sha256_hex(&bytes);
    let dir = look_dir(&env.root).map_err(refuse(2))?;
    let detail = if args.high { "high" } else { "low" };
    let table = look_table(&env.root).map_err(refuse(2))?;
    let cache = cache_path(&dir, &input);
    if !args.fresh {
        if let Some(hit) = cached(&cache, &sha, &question, detail, pages_word.as_deref()) {
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
                width: None,
                height: None,
                estimate_tokens: None,
                pages: None,
                order,
                argv: Vec::new(),
                cwd: PathBuf::new(),
            });
        }
    }

    // The file the seat reads.
    let edge = if args.high { HIGH_EDGE } else { LOW_EDGE };
    let (read, width, height, page_count) = if kind.is_still() {
        let copy = still_copy(env.tool, env.on_path, &input, kind, edge, &dir, &sha, None)
            .map_err(refuse(2))?;
        (copy.path, copy.width, copy.height, None)
    } else {
        (input.clone(), None, None, pdf_pages(&bytes))
    };
    let estimate = width.zip(height).map(|(w, h)| token_estimate(w, h));

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
        cli: None,
    };
    // A PDF over 10 pages (or of unknown length), or a page range: agy reads a PDF whole; a claude seat needs
    // pdftoppm for it, which is not installed on this PC (2026-11-09).
    let long_pdf = kind == Kind::Pdf
        && (pages.is_some() || page_count.is_none_or(|count| count > PDF_WHOLE_PAGES));
    let (pick, guard) = if long_pdf {
        let agy = Narrowing {
            cli: Some("agy".to_owned()),
            ..narrow.clone()
        };
        match book(&table, &world, &request, &agy, &order) {
            Ok(booked) => booked,
            Err(agy_why) => {
                if !(env.on_path)("pdftoppm") {
                    return Err(refuse(nirvana_seat::NO_SEAT_EXIT)(format!(
                        "This PDF needs a seat that reads past {PDF_WHOLE_PAGES} pages. No agy seat took it ({agy_why}), \
                         and pdftoppm is missing for a claude seat. {POPPLER_FIX}."
                    )));
                }
                let claude = Narrowing {
                    cli: Some("claude".to_owned()),
                    ..narrow
                };
                book(&table, &world, &request, &claude, &order)
                    .map_err(|why| refuse(nirvana_seat::NO_SEAT_EXIT)(format!("No seat: {why}.")))?
            }
        }
    } else {
        book(&table, &world, &request, &narrow, &order)
            .map_err(|why| refuse(nirvana_seat::NO_SEAT_EXIT)(format!("No seat: {why}.")))?
    };

    // The launch.
    let argv = if pick.cli == "agy" {
        let prompt = crate::nirvana_media::still_prompt_line(&read, &question, pages);
        agy_argv(&pick.model, &prompt)
    } else {
        claude_argv(
            &pick.model,
            pick.effort,
            &claude_prompt(&read, &question, pages, kind),
        )
    };
    let cwd = read.parent().map_or_else(|| dir.clone(), Path::to_path_buf);
    let program = env.stand_in.clone().unwrap_or_else(|| argv.clone());
    let extra = serde_json::json!({"cached": false, "detail": detail, "kind": kind});
    let end = launch(&program, &cwd, &pick.cli, env.timeout);
    let answered = match end {
        Ok(End::Exited {
            stdout,
            stderr,
            code,
        }) => answer_of(&pick.cli, &stdout).map_err(|why| {
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
    let (answer, tokens) = match answered {
        Ok(answered) => answered,
        Err(why) => {
            record(&table, &pick, &order, "failed", Tokens::default(), extra);
            drop(guard);
            return Err(refuse(1)(why));
        }
    };
    record(&table, &pick, &order, "pass", tokens, extra);
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
            tokens,
            answer: answer.clone(),
            read: read.display().to_string(),
            at: crate::nirvana_run::now(),
        },
    );
    Ok(Outcome {
        answer,
        seat: pick.seat,
        model: pick.model,
        effort: pick.effort,
        tokens,
        cached: false,
        read: read.display().to_string(),
        width,
        height,
        estimate_tokens: estimate,
        pages: page_count,
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
            println!("{}", outcome.answer);
            Ok(())
        }
        Err(refused) => {
            eprintln!("herdr look: {}", refused.line);
            std::process::exit(refused.code);
        }
    }
}

#[cfg(test)]
mod tests;
