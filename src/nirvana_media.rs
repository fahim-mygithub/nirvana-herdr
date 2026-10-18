//! Nirvana fork, plan 12 O6: media seats through probe P2's path (`herdr media probe|shrink|prompt`).
//! (nirvana/plans/2026-10-17-12-nirvana-claude-orchestrator.md §2, §4, §5.2, §5.4, §6, §11)
//!
//! Probe P2 (`nirvana/evidence/2026-10-17-o0-probes/README.md`) measured three facts on Antigravity (`agy`):
//! 1. Both `argon` and `flash` read MP4 and WebM video and audio when the prompt names the file's full
//!    absolute path and instructs the worker to open it with the `view_file` tool (omitting `StartLine` and
//!    `EndLine`). With a bare file name, `flash` tried `dir`, plan mode refused it, and the turn ended with
//!    an empty answer and `status: SUCCESS`.
//! 2. `view_file` refuses files over 100 MB (a 186 MB recording was refused before reaching Gemini).
//! 3. A text transcript alone discards spatial and temporal visual detail that `argon` can see directly when
//!    diagnosing UI bugs or animations.
//!
//! This module gives the orchestrator and runner three deterministic subcommands, all invoking `ffprobe` and
//! `ffmpeg` with a list `argv` (never through a shell):
//! - `herdr media probe <file> [--json]`: duration, size in bytes, container, video codec and resolution,
//!   audio presence, and whether the file exceeds `view_file`'s 100 MB limit.
//! - `herdr media shrink <file> [--out <path>] [--max-mb 95] [--json]`: re-encodes with `ffmpeg` to at most
//!   1280 px wide, at most 10 fps, and mono 48 kbps AAC audio, sizing the video bitrate from duration with a
//!   safety margin and retrying once at a lower bitrate if pass 1 is still over the limit. Never overwrites
//!   the input file.
//! - `herdr media prompt <file> --ask "<question>" [--seat argon|flash]`: prints the single-line worker
//!   prompt with the full absolute path, the `view_file` instruction, and what to return.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

/// Choice: 1 MB is taken as 1,000,000 decimal bytes rather than 1,048,576 MiB so that the 100 MB check and
/// the default 95 MB shrink target stay strictly below `view_file`'s 100 MB cap whether the tool checks
/// decimal MB or binary MiB.
pub const MB_BYTES: u64 = 1_000_000;
pub const VIEW_FILE_LIMIT_MB: u64 = 100;
pub const VIEW_FILE_LIMIT_BYTES: u64 = VIEW_FILE_LIMIT_MB * MB_BYTES;
pub const DEFAULT_MAX_MB: u64 = 95;

/// Screen-recording downscale caps (plan 12 §11 O6).
pub const MAX_WIDTH: u32 = 1280;
pub const MAX_FPS: u32 = 10;
pub const AUDIO_CHANNELS: u32 = 1;
pub const AUDIO_BITRATE_KBPS: u64 = 48;

/// Bitrate budgeting constants: leave 15% headroom for container/VBV variance, clamp very short clips so
/// they do not encode at ratusan Mbps, clamp multi-hour clips to a 32 kbps floor so `ffmpeg` never receives
/// `0k`, and cut video bitrate to 60% on the retry pass if pass 1 overshoots.
pub const SAFETY_MARGIN: f64 = 0.85;
pub const RETRY_BITRATE_FACTOR: f64 = 0.60;
pub const MIN_VIDEO_KBPS: u64 = 32;
pub const MAX_VIDEO_KBPS: u64 = 4_000;

/// Which Gemini media seat the prompt addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaSeat {
    Argon,
    Flash,
}

impl MediaSeat {
    pub fn word(self) -> &'static str {
        match self {
            Self::Argon => "argon",
            Self::Flash => "flash",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "argon" => Some(Self::Argon),
            "flash" => Some(Self::Flash),
            _ => None,
        }
    }
}

/// Metadata extracted from `ffprobe` for a media file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub path: String,
    pub duration_secs: f64,
    pub size_bytes: u64,
    pub container: String,
    pub video_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub resolution: Option<String>,
    pub audio_present: bool,
    pub audio_codec: Option<String>,
    pub limit_bytes: u64,
    pub over_limit: bool,
}

/// Outcome of `herdr media shrink`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShrinkOutcome {
    pub input: String,
    pub output: String,
    pub duration_secs: f64,
    pub input_size_bytes: u64,
    pub size_bytes: u64,
    pub max_bytes: u64,
    pub over_limit: bool,
    pub passes: u32,
    pub video_kbps: u64,
}

/// Whether `size_bytes` exceeds `limit_bytes`.
pub fn over_limit(size_bytes: u64, limit_bytes: u64) -> bool {
    size_bytes > limit_bytes
}

/// Whether `size_bytes` exceeds `agy`'s 100 MB `view_file` limit.
pub fn over_view_file_limit(size_bytes: u64) -> bool {
    over_limit(size_bytes, VIEW_FILE_LIMIT_BYTES)
}

/// The list `argv` passed to `ffprobe` (never through a shell).
pub fn probe_argv(input: &Path) -> Vec<String> {
    vec![
        "-v".to_owned(),
        "error".to_owned(),
        "-print_format".to_owned(),
        "json".to_owned(),
        "-show_format".to_owned(),
        "-show_streams".to_owned(),
        input.display().to_string(),
    ]
}

fn parse_f64_field(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n >= 0.0)
}

fn parse_u64_field(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
}

/// Parses `ffprobe -print_format json -show_format -show_streams` output into a [`Probe`].
pub fn parse_ffprobe(
    json: &str,
    path_override: Option<&Path>,
    fallback_size: Option<u64>,
) -> Result<Probe, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|err| format!("ffprobe JSON parse error: {err}"))?;
    let format = root
        .get("format")
        .and_then(Value::as_object)
        .ok_or_else(|| "ffprobe output is missing `format` object".to_owned())?;
    let streams = root
        .get("streams")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let container = format
        .get("format_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "ffprobe output is missing `format.format_name`".to_owned())?
        .to_owned();

    let stream_duration = streams
        .iter()
        .filter_map(|stream| stream.get("duration").and_then(parse_f64_field))
        .fold(None::<f64>, |acc, d| {
            Some(acc.map_or(d, |prev| prev.max(d)))
        });

    let duration_secs = format
        .get("duration")
        .and_then(parse_f64_field)
        .or(stream_duration)
        .ok_or_else(|| "ffprobe output is missing media duration".to_owned())?;

    let size_bytes = format
        .get("size")
        .and_then(parse_u64_field)
        .or(fallback_size)
        .ok_or_else(|| "ffprobe output is missing media byte size".to_owned())?;

    let path = path_override
        .map(|p| p.display().to_string())
        .or_else(|| {
            format
                .get("filename")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_default();

    let video_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("video"));
    let video_codec = video_stream
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let width = video_stream
        .and_then(|stream| stream.get("width"))
        .and_then(Value::as_u64)
        .map(|w| w as u32);
    let height = video_stream
        .and_then(|stream| stream.get("height"))
        .and_then(Value::as_u64)
        .map(|h| h as u32);
    let resolution = match (width, height) {
        (Some(w), Some(h)) => Some(format!("{w}x{h}")),
        _ => None,
    };

    let audio_stream = streams
        .iter()
        .find(|stream| stream.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let audio_present = audio_stream.is_some();
    let audio_codec = audio_stream
        .and_then(|stream| stream.get("codec_name"))
        .and_then(Value::as_str)
        .map(str::to_owned);

    Ok(Probe {
        path,
        duration_secs,
        size_bytes,
        container,
        video_codec,
        width,
        height,
        resolution,
        audio_present,
        audio_codec,
        limit_bytes: VIEW_FILE_LIMIT_BYTES,
        over_limit: over_view_file_limit(size_bytes),
    })
}

/// Human-readable text summary of a [`Probe`].
pub fn probe_text(probe: &Probe) -> String {
    let video = match (&probe.video_codec, &probe.resolution) {
        (Some(codec), Some(res)) => format!("{codec} {res}"),
        (Some(codec), None) => codec.clone(),
        (None, _) => "none".to_owned(),
    };
    let audio = match (probe.audio_present, &probe.audio_codec) {
        (true, Some(codec)) => format!("yes ({codec})"),
        (true, None) => "yes".to_owned(),
        (false, _) => "no".to_owned(),
    };
    let over = if probe.over_limit { "yes" } else { "no" };
    format!(
        "path: {}\nduration: {:.2}s\nsize: {} bytes ({:.2} MB)\ncontainer: {}\nvideo: {}\naudio: {}\nover 100 MB view_file limit: {} (limit {} bytes)\n",
        probe.path,
        probe.duration_secs,
        probe.size_bytes,
        probe.size_bytes as f64 / MB_BYTES as f64,
        probe.container,
        video,
        audio,
        over,
        probe.limit_bytes
    )
}

/// Target video bitrate in kbps for `duration_secs` to fit inside `max_bytes` with `SAFETY_MARGIN`,
/// reserving `AUDIO_BITRATE_KBPS` for mono audio and clamping to `[MIN_VIDEO_KBPS, MAX_VIDEO_KBPS]`.
pub fn target_video_kbps(duration_secs: f64, max_bytes: u64) -> u64 {
    let secs = duration_secs.max(1.0);
    let budget_bits = (max_bytes as f64) * SAFETY_MARGIN * 8.0;
    let total_kbps = (budget_bits / secs / 1000.0).floor() as u64;
    let video_kbps = total_kbps.saturating_sub(AUDIO_BITRATE_KBPS);
    video_kbps.clamp(MIN_VIDEO_KBPS, MAX_VIDEO_KBPS)
}

/// Lower video bitrate for the retry pass when pass 1 still exceeds `max_bytes`.
pub fn retry_video_kbps(first_kbps: u64) -> u64 {
    let lowered = ((first_kbps as f64) * RETRY_BITRATE_FACTOR).floor() as u64;
    lowered.clamp(
        MIN_VIDEO_KBPS,
        first_kbps.saturating_sub(1).max(MIN_VIDEO_KBPS),
    )
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

/// Resolves `path` to a clean absolute path without requiring the file to exist on disk.
pub fn absolute_media_path(path: &Path) -> PathBuf {
    if let Ok(canon) = std::fs::canonicalize(path) {
        return strip_verbatim(canon);
    }
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut clean = PathBuf::new();
    for comp in joined.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    strip_verbatim(clean)
}

/// Whether `input` and `output` resolve to the same file path.
pub fn paths_collide(input: &Path, output: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(input), std::fs::canonicalize(output)) {
        if a == b {
            return true;
        }
    }
    let a = absolute_media_path(input);
    let b = absolute_media_path(output);
    if cfg!(windows) {
        a.to_string_lossy()
            .replace('/', "\\")
            .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
    } else {
        a == b
    }
}

/// Default output path next to `input`: `<stem>.shrunk.mp4`.
pub fn default_out_path(input: &Path) -> PathBuf {
    let parent = input
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("media");
    let candidate = parent.join(format!("{stem}.shrunk.mp4"));
    if paths_collide(input, &candidate) {
        parent.join(format!("{stem}.shrunk.1.mp4"))
    } else {
        candidate
    }
}

/// Builds the list `argv` for `ffmpeg` to shrink `input` into `output` at `video_kbps`.
/// Refuses to overwrite `input`.
pub fn shrink_argv(input: &Path, output: &Path, video_kbps: u64) -> Result<Vec<String>, String> {
    if paths_collide(input, output) {
        return Err(format!(
            "refusing to overwrite input file {}",
            input.display()
        ));
    }
    let kbps = video_kbps.max(MIN_VIDEO_KBPS);
    let v_rate = format!("{kbps}k");
    let buf_rate = format!("{}k", kbps * 2);
    let a_rate = format!("{AUDIO_BITRATE_KBPS}k");
    Ok(vec![
        "-y".to_owned(),
        "-v".to_owned(),
        "error".to_owned(),
        "-i".to_owned(),
        input.display().to_string(),
        "-vf".to_owned(),
        format!("scale='min({MAX_WIDTH},iw)':-2"),
        "-r".to_owned(),
        MAX_FPS.to_string(),
        "-c:v".to_owned(),
        "libx264".to_owned(),
        "-preset".to_owned(),
        "veryfast".to_owned(),
        "-pix_fmt".to_owned(),
        "yuv420p".to_owned(),
        "-b:v".to_owned(),
        v_rate.clone(),
        "-maxrate".to_owned(),
        v_rate,
        "-bufsize".to_owned(),
        buf_rate,
        "-c:a".to_owned(),
        "aac".to_owned(),
        "-ac".to_owned(),
        AUDIO_CHANNELS.to_string(),
        "-b:a".to_owned(),
        a_rate,
        "-movflags".to_owned(),
        "+faststart".to_owned(),
        output.display().to_string(),
    ])
}

/// Builds the one-line prompt for an `agy` worker on `argon` or `flash` following probe P2's lesson:
/// names the full absolute path, instructs the worker to open it with `view_file` (without `StartLine` or
/// `EndLine`), and asks for timestamps, what happens on screen, what the video shows that a transcript
/// would lose, and findings for `ask`.
pub fn prompt_line(path: &Path, ask: &str, seat: MediaSeat) -> Result<String, String> {
    let clean_ask = ask.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean_ask.is_empty() {
        return Err("herdr media prompt: --ask requires a non-empty question".to_owned());
    }
    let abs = absolute_media_path(path);
    let role_detail = match seat {
        MediaSeat::Argon => {
            "watch the recording directly and cross-check relevant source files if needed."
        }
        MediaSeat::Flash => "extract a complete timestamped transcript and visual event timeline.",
    };
    Ok(format!(
        "Use the view_file tool (with AbsolutePath set to \"{}\" and without StartLine or EndLine) to inspect this media file directly; do not start subagents or background commands. Seat role ({}): {} Return: (1) timestamps (MM:SS) for each key event, (2) what happens on screen and any spoken audio/transcript, (3) visual details the video shows that a text transcript alone would lose, and (4) findings answering: {}",
        abs.display(),
        seat.word(),
        role_detail,
        clean_ask
    ))
}

// ---- subprocess execution & CLI ----

const USAGE: &str = "usage: herdr media probe <file> [--json]
       herdr media shrink <file> [--out <path>] [--max-mb 95] [--json]
       herdr media prompt <file> --ask \"<question>\" [--seat argon|flash]";

fn usage_exit() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

fn ensure_tool(program: &str, subcommand: &str) {
    if !crate::nirvana_usage::on_path(program) {
        eprintln!(
            "herdr media {subcommand}: {program} not found on PATH; install ffmpeg (which provides ffmpeg and ffprobe)"
        );
        std::process::exit(2);
    }
}

fn run_list_argv(program: &str, args: &[String]) -> Result<String, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::platform::configure_background_command(&mut command);
    let output = command.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            format!("{program} not found on PATH")
        } else {
            format!("failed to run {program}: {err}")
        }
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            format!("{program} exited with {}", output.status)
        } else {
            format!("{program} failed: {detail}")
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_probe(file: &Path) -> Result<Probe, String> {
    if !file.is_file() {
        return Err(format!("media file not found: {}", file.display()));
    }
    let fallback_size = std::fs::metadata(file).ok().map(|m| m.len());
    let abs = absolute_media_path(file);
    let argv = probe_argv(&abs);
    let stdout = run_list_argv("ffprobe", &argv)?;
    parse_ffprobe(&stdout, Some(&abs), fallback_size)
}

pub fn cli(args: &[String]) -> std::io::Result<()> {
    match args.first().map(String::as_str) {
        Some("probe") => probe_cli(&args[1..]),
        Some("shrink") => shrink_cli(&args[1..]),
        Some("prompt") => prompt_cli(&args[1..]),
        _ => usage_exit(),
    }
}

fn probe_cli(args: &[String]) -> std::io::Result<()> {
    let mut file: Option<PathBuf> = None;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            flag if flag.starts_with('-') => usage_exit(),
            path if file.is_none() => file = Some(PathBuf::from(path)),
            _ => usage_exit(),
        }
    }
    let Some(file) = file else {
        usage_exit();
    };
    ensure_tool("ffprobe", "probe");
    let probe = match run_probe(&file) {
        Ok(probe) => probe,
        Err(err) => {
            eprintln!("herdr media probe: {err}");
            std::process::exit(2);
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&probe)?);
    } else {
        print!("{}", probe_text(&probe));
    }
    Ok(())
}

fn shrink_cli(args: &[String]) -> std::io::Result<()> {
    let mut file: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut max_mb: u64 = DEFAULT_MAX_MB;
    let mut json = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--out" => out = Some(PathBuf::from(value())),
            "--max-mb" => {
                max_mb = value()
                    .parse::<u64>()
                    .ok()
                    .filter(|mb| *mb > 0)
                    .unwrap_or_else(|| usage_exit());
            }
            "--json" => json = true,
            flag if flag.starts_with('-') => usage_exit(),
            path if file.is_none() => file = Some(PathBuf::from(path)),
            _ => usage_exit(),
        }
    }
    let Some(file) = file else {
        usage_exit();
    };
    ensure_tool("ffprobe", "shrink");
    ensure_tool("ffmpeg", "shrink");
    let probe = match run_probe(&file) {
        Ok(probe) => probe,
        Err(err) => {
            eprintln!("herdr media shrink: {err}");
            std::process::exit(2);
        }
    };
    let input_abs = absolute_media_path(&file);
    let out_path = out.unwrap_or_else(|| default_out_path(&input_abs));
    let out_abs = absolute_media_path(&out_path);
    let max_bytes = max_mb.saturating_mul(MB_BYTES);

    let mut video_kbps = target_video_kbps(probe.duration_secs, max_bytes);
    let argv1 = match shrink_argv(&input_abs, &out_abs, video_kbps) {
        Ok(argv) => argv,
        Err(err) => {
            eprintln!("herdr media shrink: {err}");
            std::process::exit(2);
        }
    };
    if let Some(parent) = out_abs.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(err) = run_list_argv("ffmpeg", &argv1) {
        eprintln!("herdr media shrink: {err}");
        std::process::exit(1);
    }
    let mut passes = 1u32;
    let mut size_bytes = std::fs::metadata(&out_abs).map_or(0, |m| m.len());

    // If pass 1 is still over the limit, retry once at a lower bitrate.
    if over_limit(size_bytes, max_bytes) {
        let ratio_kbps =
            ((video_kbps as f64) * (max_bytes as f64 / size_bytes.max(1) as f64) * SAFETY_MARGIN)
                .floor() as u64;
        video_kbps = retry_video_kbps(video_kbps).min(ratio_kbps.max(MIN_VIDEO_KBPS));
        let argv2 = match shrink_argv(&input_abs, &out_abs, video_kbps) {
            Ok(argv) => argv,
            Err(err) => {
                eprintln!("herdr media shrink: {err}");
                std::process::exit(2);
            }
        };
        if let Err(err) = run_list_argv("ffmpeg", &argv2) {
            eprintln!("herdr media shrink: retry failed: {err}");
            std::process::exit(1);
        }
        passes = 2;
        size_bytes = std::fs::metadata(&out_abs).map_or(0, |m| m.len());
    }

    let still_over = over_limit(size_bytes, max_bytes);
    let outcome = ShrinkOutcome {
        input: input_abs.display().to_string(),
        output: out_abs.display().to_string(),
        duration_secs: probe.duration_secs,
        input_size_bytes: probe.size_bytes,
        size_bytes,
        max_bytes,
        over_limit: still_over,
        passes,
        video_kbps,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&outcome)?);
    } else {
        println!(
            "output: {}\nsize: {} bytes ({:.2} MB, limit {:.2} MB, passes: {}, video: {} kbps)",
            outcome.output,
            outcome.size_bytes,
            outcome.size_bytes as f64 / MB_BYTES as f64,
            outcome.max_bytes as f64 / MB_BYTES as f64,
            outcome.passes,
            outcome.video_kbps
        );
    }
    if still_over {
        eprintln!(
            "herdr media shrink: {} is {} bytes, still over limit {} bytes after {} passes",
            outcome.output, outcome.size_bytes, outcome.max_bytes, outcome.passes
        );
        std::process::exit(1);
    }
    Ok(())
}

fn prompt_cli(args: &[String]) -> std::io::Result<()> {
    let mut file: Option<PathBuf> = None;
    let mut ask: Option<String> = None;
    let mut seat = MediaSeat::Argon;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().cloned().unwrap_or_else(|| usage_exit());
        match arg.as_str() {
            "--ask" => ask = Some(value()),
            "--seat" => {
                let raw = value();
                seat = MediaSeat::parse(&raw).unwrap_or_else(|| {
                    eprintln!("herdr media prompt: --seat must be argon or flash (got {raw:?})");
                    std::process::exit(2);
                });
            }
            flag if flag.starts_with('-') => usage_exit(),
            path if file.is_none() => file = Some(PathBuf::from(path)),
            _ => usage_exit(),
        }
    }
    let (Some(file), Some(ask)) = (file, ask) else {
        usage_exit();
    };
    match prompt_line(&file, &ask, seat) {
        Ok(line) => {
            println!("{line}");
            Ok(())
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded with `ffprobe -v error -print_format json -show_format -show_streams` on a real 2s 1920x1080
    /// H.264 + mono AAC MP4 generated with `ffmpeg` (`testsrc2=size=1920x1080:rate=30` + `sine=frequency=440`).
    const FFPROBE_FIXTURE: &str = r#"{
    "streams": [
        {
            "index": 0,
            "codec_name": "h264",
            "codec_long_name": "H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10",
            "profile": "High",
            "codec_type": "video",
            "width": 1920,
            "height": 1080,
            "pix_fmt": "yuv420p",
            "r_frame_rate": "30/1",
            "avg_frame_rate": "30/1",
            "duration": "2.000000",
            "bit_rate": "6200304",
            "nb_frames": "60"
        },
        {
            "index": 1,
            "codec_name": "aac",
            "codec_long_name": "AAC (Advanced Audio Coding)",
            "profile": "LC",
            "codec_type": "audio",
            "sample_rate": "44100",
            "channels": 1,
            "channel_layout": "mono",
            "duration": "2.000000",
            "bit_rate": "69698",
            "nb_frames": "88"
        }
    ],
    "format": {
        "filename": "target/o6-media/fixture.mp4",
        "nb_streams": 2,
        "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
        "format_long_name": "QuickTime / MOV",
        "start_time": "0.000000",
        "duration": "2.000000",
        "size": "1571212",
        "bit_rate": "6284848",
        "probe_score": 100
    }
}"#;

    #[test]
    fn media_over_limit_checks_the_100mb_view_file_cap() {
        assert!(!over_view_file_limit(100_000_000));
        assert!(over_view_file_limit(100_000_001));
        assert!(!over_limit(95_000_000, 95_000_000));
        assert!(over_limit(95_000_001, 95_000_000));
    }

    #[test]
    fn media_ffprobe_json_reads_duration_size_container_video_audio_and_limit() {
        let probe = parse_ffprobe(FFPROBE_FIXTURE, None, None).expect("fixture parses");
        assert_eq!(probe.path, "target/o6-media/fixture.mp4");
        assert!((probe.duration_secs - 2.0).abs() < 1e-6);
        assert_eq!(probe.size_bytes, 1_571_212);
        assert_eq!(probe.container, "mov,mp4,m4a,3gp,3g2,mj2");
        assert_eq!(probe.video_codec.as_deref(), Some("h264"));
        assert_eq!(probe.width, Some(1920));
        assert_eq!(probe.height, Some(1080));
        assert_eq!(probe.resolution.as_deref(), Some("1920x1080"));
        assert!(probe.audio_present);
        assert_eq!(probe.audio_codec.as_deref(), Some("aac"));
        assert_eq!(probe.limit_bytes, 100_000_000);
        assert!(!probe.over_limit);

        let text = probe_text(&probe);
        assert!(text.contains("duration: 2.00s"));
        assert!(text.contains("size: 1571212 bytes"));
        assert!(text.contains("container: mov,mp4,m4a,3gp,3g2,mj2"));
        assert!(text.contains("video: h264 1920x1080"));
        assert!(text.contains("audio: yes (aac)"));
        assert!(text.contains("over 100 MB view_file limit: no"));

        // A 186 MB silent file (like probe P2's over-limit recording) is flagged over_limit with no audio.
        let big_silent = FFPROBE_FIXTURE
            .replace("\"size\": \"1571212\"", "\"size\": \"186000000\"")
            .replace("\"codec_type\": \"audio\"", "\"codec_type\": \"subtitle\"");
        let big_probe = parse_ffprobe(&big_silent, None, None).expect("big fixture parses");
        assert!(big_probe.over_limit);
        assert!(!big_probe.audio_present);
    }

    #[test]
    fn media_bitrate_scales_with_duration_and_clamps_very_short_and_very_long_files() {
        let max_bytes = 95_000_000;

        // 3-minute (180s) recording: 95 MB * 0.85 * 8 / 180 / 1000 = 3588 kbps total -> 3540 kbps video.
        let three_min = target_video_kbps(180.0, max_bytes);
        assert_eq!(three_min, 3540);

        // 10-minute (600s) recording: 95 MB * 0.85 * 8 / 600 / 1000 = 1076 kbps total -> 1028 kbps video.
        let ten_min = target_video_kbps(600.0, max_bytes);
        assert_eq!(ten_min, 1028);
        assert!(ten_min < three_min);
        let projected_bytes = ((ten_min + 48) * 1000 * 600) / 8;
        assert!(projected_bytes < max_bytes);

        // Very short file (0.2s or 1.0s): clamped to 4000 kbps, never 300+ Mbps.
        assert_eq!(target_video_kbps(0.2, max_bytes), 4_000);
        assert_eq!(target_video_kbps(1.0, max_bytes), 4_000);

        // Very long file (10 hours = 36,000s): clamped to 32 kbps floor, never 0.
        assert_eq!(target_video_kbps(36_000.0, max_bytes), 32);

        // Retry pass lowers the bitrate by 0.60 and respects the 32 kbps floor.
        assert_eq!(retry_video_kbps(1000), 600);
        assert_eq!(retry_video_kbps(32), 32);
    }

    #[test]
    fn media_shrink_argv_pins_scale_fps_mono_audio_and_never_overwrites_input() {
        let input = Path::new("clips/demo.mp4");
        let default_out = default_out_path(input);
        assert_eq!(default_out, PathBuf::from("clips/demo.shrunk.mp4"));

        let argv = shrink_argv(input, &default_out, 1200).expect("distinct output succeeds");
        assert!(argv
            .windows(2)
            .any(|w| w == ["-vf", "scale='min(1280,iw)':-2"]));
        assert!(argv.windows(2).any(|w| w == ["-r", "10"]));
        assert!(argv.windows(2).any(|w| w == ["-ac", "1"]));
        assert!(argv.windows(2).any(|w| w == ["-b:a", "48k"]));
        assert!(argv.windows(2).any(|w| w == ["-b:v", "1200k"]));
        assert!(argv.windows(2).any(|w| w == ["-maxrate", "1200k"]));

        // Never overwrite the input file, even when written with `.` segments.
        assert!(shrink_argv(input, input, 1200).is_err());
        assert!(shrink_argv(Path::new("clips/./demo.mp4"), input, 1200).is_err());
    }

    #[test]
    fn media_prompt_line_names_full_path_view_file_and_expected_return_fields() {
        let rel = Path::new("target/o6-media/bug.shrunk.mp4");
        let abs = absolute_media_path(rel);
        assert!(abs.is_absolute());

        let line = prompt_line(
            rel,
            "What UI glitch happens\nwhen the sidebar closes?",
            MediaSeat::Argon,
        )
        .expect("prompt line succeeds");
        assert!(!line.contains('\n') && !line.contains('\r'));
        assert!(line.contains(&abs.display().to_string()));
        assert!(line.contains("view_file"));
        assert!(line.contains("without StartLine or EndLine"));
        assert!(line.contains("timestamps"));
        assert!(line.contains("what happens on screen"));
        assert!(line.contains("transcript"));
        assert!(line.contains("Seat role (argon)"));
        assert!(line.contains("What UI glitch happens when the sidebar closes?"));

        let flash_line =
            prompt_line(rel, "Transcribe the narration.", MediaSeat::Flash).expect("flash prompt");
        assert!(flash_line.contains("Seat role (flash)"));

        assert!(prompt_line(rel, "   \n\t  ", MediaSeat::Argon).is_err());
    }
}
