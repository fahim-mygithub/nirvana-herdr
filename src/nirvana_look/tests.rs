//! Plan 40c: `herdr look`. Fakes only: a stand-in command for the CLI (as the worker's tests use), a fake ffmpeg that
//! writes a PNG header, a stand-in usage world. No `claude` or `agy` is started.

use super::*;
use crate::nirvana_usage::Reading;
use std::cell::RefCell;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "c40c-look-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// The 24 bytes `image_size` reads of a PNG, plus the rest of IHDR.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 2, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

/// The header of a real JPEG, ffmpeg 8.0.1 `testsrc2=size=1500x3001` (stored 1500x3000), up to its SOF0: JFIF, a
/// comment, DQT, DHT (0xC4, which is not a frame header), then SOF0.
const JPEG_1500X3000: &str = "ffd8ffe000104a46494600010200000100010000fffe00104c61766336322e31312e31303000ffdb0043000810101310131616161616161a181a1b1b1b1a1a1a1a1b1b1b1d1d1d2222221d1d1d1b1b1d1d2020222225262523232223262628282830302e2e38383a454553ffc400bd0001000203010101000000000000000000000605030407020108010100020301010100000000000000000000050406070308020110010001020205070b0305000104020301000111022131034112045182612213057133b23281c191b1a14383b335f052d114e14223f162157253a234d292c224e2110002010203020b040806020202020105000102110321041231415105712261b1328113b33574c142063382b23491a1c28343146223f052d1e153f17224d215a26392739364ffc00011080bb805dc";

/// A fake ffmpeg/ffprobe: ffmpeg writes a PNG header of `out` size to its last argument; every call is kept.
struct FakeTool {
    calls: RefCell<Vec<(String, Vec<String>)>>,
    out: (u32, u32),
}

impl FakeTool {
    fn new(out: (u32, u32)) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            out,
        }
    }

    fn run(&self, program: &str, args: &[String]) -> Result<String, String> {
        self.calls
            .borrow_mut()
            .push((program.to_owned(), args.to_vec()));
        if program == "ffmpeg" {
            let output = args.last().expect("output");
            std::fs::write(output, png(self.out.0, self.out.1)).map_err(|err| err.to_string())?;
        }
        Ok(String::new())
    }

    fn ffmpeg_calls(&self) -> usize {
        self.calls
            .borrow()
            .iter()
            .filter(|(program, _)| program == "ffmpeg")
            .count()
    }
}

fn all_on_path(_: &str) -> bool {
    true
}

fn none_on_path(_: &str) -> bool {
    false
}

fn no_pdftoppm(program: &str) -> bool {
    program != "pdftoppm"
}

fn reading(pool: &str, window: &str, now: u64) -> Reading {
    Reading {
        pool: pool.into(),
        window: window.into(),
        used: Some(0.1),
        resets_at: Some(now + 3600),
        resets_text: None,
        read_at: now,
        source: "test".into(),
    }
}

/// Fresh readings for the pools `look.image`'s seats sit on: max (haiku, sonnet) and gemini (flash).
fn fresh_world(pools: &[&str]) -> World {
    let now = crate::nirvana_run::now();
    World {
        now,
        readings: pools
            .iter()
            .flat_map(|pool| [reading(pool, "5h", now), reading(pool, "week", now)])
            .collect(),
        ..World::default()
    }
}

/// The stand-in CLI: `cmd /c type <file>` prints a recorded answer.
#[cfg(windows)]
fn printing(dir: &Path, name: &str, text: &str) -> Vec<String> {
    let file = dir.join(name);
    std::fs::write(&file, text).expect("fixture");
    vec![
        "cmd.exe".into(),
        "/c".into(),
        format!("type {}", file.display()),
    ]
}

/// claude's `--output-format json`: one array of events, the answer in the last `result` (measured 2026-11-09).
const CLAUDE_ANSWER: &str = r#"[{"type":"system","subtype":"init","session_id":"s1"},{"type":"assistant","message":{"id":"m1","content":[{"type":"text","text":"working"}]},"session_id":"s1"},{"type":"result","subtype":"success","is_error":false,"result":"The sign reads OPEN. 3 people.","session_id":"s1","total_cost_usd":0.00016,"usage":{"input_tokens":900,"cache_creation_input_tokens":0,"cache_read_input_tokens":9500,"output_tokens":20}}]"#;

const AGY_ANSWER: &str = r#"{"event":"step_update","step_update":{"step_index":0,"conversation_id":"c1"}}
{"event":"result","result":{"status":"SUCCESS","response":"Two charts. The top one peaks at 42.","conversation_id":"c1"}}"#;

fn env<'a>(
    root: &Path,
    world: World,
    stand_in: Vec<String>,
    tool: Tool<'a>,
    on_path: OnPath<'a>,
) -> Env<'a> {
    Env {
        root: root.to_path_buf(),
        world: Some(world),
        stand_in: Some(stand_in),
        timeout: Duration::from_secs(20),
        tool,
        on_path,
    }
}

fn args(file: &Path, ask: &str) -> Args {
    Args {
        file: file.to_path_buf(),
        ask: ask.to_owned(),
        ..Args::default()
    }
}

/// No lease of any order is live in the test home's `leases.json`.
fn no_live_lease() -> bool {
    let leases = nirvana_seat::load_leases(&nirvana_seat::dir().join(nirvana_seat::LEASES_FILE))
        .expect("leases");
    let now = crate::nirvana_run::now();
    leases.leases.iter().all(|lease| !lease.live(now))
}

fn record_lines() -> Vec<nirvana_seat::RecordLine> {
    nirvana_seat::read_record(&nirvana_seat::dir().join(nirvana_seat::RECORD_FILE))
}

// ---- the launch lines ----

#[test]
fn c40c_claude_argv_is_the_measured_line_with_no_schema() {
    let argv = claude_argv("claude-haiku-5-5", Effort::Low, "Read it.");
    assert_eq!(
        argv,
        [
            "claude",
            "-p",
            "Read it.",
            "--model",
            "claude-haiku-5-5",
            "--effort",
            "low",
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
    );
    assert!(!argv.iter().any(|arg| arg.contains("schema")));
}

#[test]
fn c40c_agy_argv_reads_with_view_file_and_no_schema() {
    let path = Path::new(r"C:\p\.nirvana\look\shot-abc-1024.png");
    let prompt = crate::nirvana_media::still_prompt_line(path, "what does\n the sign say?", None);
    let argv = agy_argv("gemini-3.8-flash-low", &prompt);
    assert_eq!(argv[0], "agy");
    assert!(argv
        .windows(2)
        .any(|w| w == ["--model", "gemini-3.8-flash-low"]));
    assert!(argv
        .windows(2)
        .any(|w| w == ["--output-format", "stream-json"]));
    assert!(!argv.iter().any(|arg| arg.contains("schema")));
    let print = argv.last().expect("print");
    assert!(print.starts_with("--print=Open \""), "{print}");
    assert!(print.contains("view_file"), "{print}");
    assert!(
        print.contains(r"C:\p\.nirvana\look\shot-abc-1024.png"),
        "{print}"
    );
    assert!(print.ends_with("what does the sign say?"), "{print}");
    // A page range reaches the prompt.
    assert!(crate::nirvana_media::still_prompt_line(path, "q", Some((3, 7))).contains("pages 3-7"));
}

#[test]
fn c40c_claude_prompt_names_the_full_path_and_pages() {
    let path = Path::new(r"C:\p\doc.pdf");
    let prompt = claude_prompt(path, "total on page 2?", Some((2, 2)), Kind::Pdf);
    assert!(prompt.contains(r#""C:\p\doc.pdf""#), "{prompt}");
    assert!(prompt.contains("PDF"), "{prompt}");
    assert!(prompt.contains("pages 2-2"), "{prompt}");
}

// ---- the file ----

#[test]
fn c40c_header_sizes_read_png_jpeg_gif_bmp_webp() {
    assert_eq!(image_size(&png(3000, 2000)), Some((3000, 2000)));
    assert_eq!(image_size(&unhex(JPEG_1500X3000)), Some((1500, 3000)));
    // Headers of real files made with ffmpeg 8.0.1 from `testsrc2=size=640x480`.
    let gif = unhex("4749463839618002e001f71f310000002400004800006c0000900000b40000d8");
    let bmp = unhex("424d36100e0000000000360000002800000080020000e0010000010018000000");
    let webp_lossy = unhex("524946468c230000574542505650382080230000d0bf009d012a8002e0013e91");
    let webp_lossless = unhex("5249464642880000574542505650384c358800002f7fc2770011b66d23397a2d");
    assert_eq!(image_size(&gif), Some((640, 480)));
    assert_eq!(image_size(&bmp), Some((640, 480)));
    assert_eq!(image_size(&webp_lossy), Some((640, 480)));
    assert_eq!(image_size(&webp_lossless), Some((640, 480)));
    assert_eq!(image_size(b"not an image at all"), None);
}

#[test]
fn c40c_token_estimate_matches_the_measured_formula() {
    // The order: a 1024 px copy of a 3:2 photo costs about 925 tokens; a full one is capped at 4,784.
    assert_eq!(token_estimate(1024, 683), 37 * 25);
    assert_eq!(token_estimate(1024, 683), 925);
    assert_eq!(token_estimate(3000, 2000), TOKEN_CAP);
    assert_eq!(token_estimate(28, 28), 1);
    assert_eq!(token_estimate(29, 28), 2);
}

#[test]
fn c40c_a_small_still_is_read_as_it_is_and_a_large_one_is_copied_small() {
    let dir = temp("downsize");
    let look = look_dir(&dir).expect("look dir");
    let small = dir.join("small.png");
    std::fs::write(&small, png(1024, 600)).expect("small");
    let tool = FakeTool::new((1024, 683));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let kept = still_copy(
        &run,
        &all_on_path,
        &small,
        Kind::Png,
        LOW_EDGE,
        &look,
        "aa",
        None,
    )
    .expect("small");
    assert!(!kept.copied);
    assert_eq!(kept.path, small);
    assert_eq!(
        tool.ffmpeg_calls(),
        0,
        "a still within the edge is never copied"
    );

    let big = dir.join("big.png");
    std::fs::write(&big, png(3000, 2000)).expect("big");
    let sha = sha256_hex(&std::fs::read(&big).expect("bytes"));
    let copy = still_copy(
        &run,
        &all_on_path,
        &big,
        Kind::Png,
        LOW_EDGE,
        &look,
        &sha,
        None,
    )
    .expect("big");
    assert!(copy.copied);
    assert_eq!(
        copy.path.parent(),
        Some(look.as_path()),
        "copies go in the look folder"
    );
    assert_ne!(
        copy.path.parent(),
        big.parent(),
        "never beside the owner's file"
    );
    assert_eq!((copy.width, copy.height), (Some(1024), Some(683)));
    assert_eq!(tool.ffmpeg_calls(), 1);
    let calls = tool.calls.borrow();
    let argv = &calls.last().expect("call").1;
    assert!(
        argv.iter().any(|arg| arg.contains("min(1024,iw)")),
        "{argv:?}"
    );
    drop(calls);
    // The same copy again is not made twice.
    still_copy(
        &run,
        &all_on_path,
        &big,
        Kind::Png,
        LOW_EDGE,
        &look,
        &sha,
        None,
    )
    .expect("again");
    assert_eq!(tool.ffmpeg_calls(), 1);
    // The input is untouched.
    assert_eq!(std::fs::read(&big).expect("big"), png(3000, 2000));
    // ffmpeg missing: said plainly.
    let other = dir.join("other.png");
    std::fs::write(&other, png(4000, 3000)).expect("other");
    let why = still_copy(
        &run,
        &none_on_path,
        &other,
        Kind::Png,
        LOW_EDGE,
        &look,
        "bb",
        None,
    )
    .expect_err("no ffmpeg");
    assert!(why.contains("ffmpeg is not on PATH"), "{why}");
}

#[test]
fn c40c_detail_high_copies_at_the_models_cap() {
    let dir = temp("high");
    let look = look_dir(&dir).expect("look dir");
    let mid = dir.join("mid.png");
    std::fs::write(&mid, png(2000, 1500)).expect("mid");
    let tool = FakeTool::new((2576, 1717));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    // 2000 px is over the low edge but within the high one.
    assert!(
        still_copy(
            &run,
            &all_on_path,
            &mid,
            Kind::Png,
            LOW_EDGE,
            &look,
            "m1",
            None
        )
        .expect("low")
        .copied
    );
    assert!(
        !still_copy(
            &run,
            &all_on_path,
            &mid,
            Kind::Png,
            HIGH_EDGE,
            &look,
            "m2",
            None
        )
        .expect("high")
        .copied
    );
    let huge = dir.join("huge.png");
    std::fs::write(&huge, png(8000, 6000)).expect("huge");
    let copy = still_copy(
        &run,
        &all_on_path,
        &huge,
        Kind::Png,
        HIGH_EDGE,
        &look,
        "h1",
        None,
    )
    .expect("huge");
    assert!(copy.copied);
    assert!(copy.path.display().to_string().ends_with("-2576.png"));
    assert_eq!(copy_edge(8000, 6000, HIGH_EDGE), Some(2576));
    assert_eq!(copy_edge(2576, 100, HIGH_EDGE), None);
}

#[test]
fn c40c_heic_is_turned_into_a_jpeg_and_missing_ffmpeg_is_said() {
    let dir = temp("heic");
    let look = look_dir(&dir).expect("look dir");
    let heic = dir.join("IMG_0042.HEIC");
    std::fs::write(&heic, b"\0\0\0\x18ftypheic....").expect("heic");
    let tool = FakeTool::new((1024, 768));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let copy = still_copy(
        &run,
        &all_on_path,
        &heic,
        Kind::Heic,
        LOW_EDGE,
        &look,
        "c0ffee00c0ffee",
        None,
    )
    .expect("heic");
    assert!(copy.copied);
    assert_eq!(tool.ffmpeg_calls(), 1, "a HEIC always goes through ffmpeg");
    assert!(
        copy.path.display().to_string().ends_with(".jpg"),
        "{}",
        copy.path.display()
    );
    let why = still_copy(
        &run,
        &none_on_path,
        &heic,
        Kind::Heic,
        LOW_EDGE,
        &look,
        "c0ffee00c0ffee",
        None,
    )
    .expect_err("no ffmpeg");
    assert!(why.contains("is HEIC, and ffmpeg is not on PATH"), "{why}");
}

#[test]
fn c40c_refusals_name_the_file_and_what_is_taken() {
    let dir = temp("refuse");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let e = env(&dir, World::default(), Vec::new(), &run, &all_on_path);
    let refused = |file: &Path, ask: &str| look(&args(file, ask), &e).expect_err("refused");

    let text = dir.join("notes.txt");
    std::fs::write(&text, "x").expect("txt");
    let r = refused(&text, "q");
    assert_eq!(r.code, 2);
    assert!(r.line.contains(TAKES), "{}", r.line);

    let empty = dir.join("empty.png");
    std::fs::write(&empty, b"").expect("empty");
    assert!(refused(&empty, "q").line.contains("is empty: 0 bytes."));

    assert!(refused(&dir.join("gone.png"), "q")
        .line
        .starts_with("No such file:"));

    let large = dir.join("large.jpg");
    std::fs::File::create(&large)
        .and_then(|file| file.set_len(MAX_BYTES + 1))
        .expect("large");
    assert!(refused(&large, "q")
        .line
        .contains("herdr look takes 50 MB at most."));

    let ok = dir.join("ok.png");
    std::fs::write(&ok, png(10, 10)).expect("ok");
    assert!(refused(&ok, "   ").line.contains("--ask needs a question."));
    let mut paged = args(&ok, "q");
    paged.pages = Some("2-3".into());
    assert!(look(&paged, &e)
        .expect_err("pages")
        .line
        .contains("--pages is for a PDF."));
    assert_eq!(
        tool.calls.borrow().len(),
        0,
        "a refused file starts nothing"
    );
}

#[test]
fn c40c_pdf_pages_are_counted_or_unknown() {
    let three = b"%PDF-1.4\n1 0 obj << /Type /Pages /Kids [2 0 R 3 0 R 4 0 R] /Count 3 >>\n2 0 obj << /Type /Page >>\n3 0 obj << /Type/Page /Parent 1 0 R >>\n4 0 obj <</Type /Page>>\n";
    assert_eq!(pdf_pages(three), Some(3));
    // Object streams can hide page objects: unknown, which counts as over 10.
    assert_eq!(
        pdf_pages(b"%PDF-1.5\n5 0 obj << /Type /ObjStm /N 4 >>\n2 0 obj << /Type /Page >>"),
        None
    );
    assert_eq!(pdf_pages(b"%PDF-1.4\n"), None);
    assert_eq!(parse_pages("3-7"), Some((3, 7)));
    assert_eq!(parse_pages("4"), Some((4, 4)));
    assert_eq!(parse_pages("7-3"), None);
    assert_eq!(parse_pages("0"), None);
}

// ---- the seat, the launch, the cache ----

#[test]
fn c40c_the_class_is_added_once_and_parses() {
    let once = with_class(nirvana_seat::DEFAULT);
    assert_eq!(with_class(&once), once, "added once");
    let table = nirvana_seat::parse(&once).expect("the table with look.image parses");
    let class = table.class(CLASS).expect("class");
    assert_eq!(class.chain, "media");
    assert_eq!(class.needs, ["image"]);
    let seats: Vec<&str> = class
        .seats
        .iter()
        .map(|entry| entry.seat.as_str())
        .collect();
    assert_eq!(seats, ["haiku", "flash", "sonnet"]);
}

#[test]
fn c40c_copies_go_in_the_look_folder_with_a_gitignore() {
    let dir = temp("ignore");
    let look = look_dir(&dir).expect("look dir");
    assert_eq!(look, dir.join(".nirvana").join("look"));
    assert_eq!(
        std::fs::read_to_string(look.join(".gitignore")).expect("ignore"),
        "*\n"
    );
    // An owner's edit of the ignore file is kept.
    std::fs::write(look.join(".gitignore"), "*\n!keep\n").expect("edit");
    look_dir(&dir).expect("again");
    assert_eq!(
        std::fs::read_to_string(look.join(".gitignore")).expect("ignore"),
        "*\n!keep\n"
    );
}

#[test]
fn c40c_no_fresh_reading_fails_closed_and_starts_nothing() {
    let _home = crate::nirvana::with_test_home("c40c-stale");
    let dir = temp("stale");
    let file = dir.join("shot.png");
    std::fs::write(&file, png(800, 600)).expect("png");
    let marker = dir.join("started");
    let stand_in = vec![
        "cmd.exe".into(),
        "/c".into(),
        format!("echo x> {}", marker.display()),
    ];
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let e = env(&dir, World::default(), stand_in, &run, &all_on_path);
    let refused = look(&args(&file, "what?"), &e).expect_err("no reading");
    assert_eq!(refused.code, nirvana_seat::NO_SEAT_EXIT);
    assert!(
        refused.line.contains("fresh usage reading"),
        "{}",
        refused.line
    );
    assert!(!marker.exists(), "nothing was started");
    assert!(no_live_lease());
    // A fresh reading for another pool does not make a stale seat pickable.
    let mut only = args(&file, "what?");
    only.seat = Some("haiku".into());
    let e = env(
        &dir,
        fresh_world(&["gemini"]),
        Vec::new(),
        &run,
        &all_on_path,
    );
    assert!(look(&only, &e)
        .expect_err("haiku stale")
        .line
        .contains("fresh usage reading"));
}

#[cfg(windows)]
#[test]
fn c40c_a_look_answers_records_and_releases_its_lease() {
    let _home = crate::nirvana::with_test_home("c40c-answer");
    let dir = temp("answer");
    let file = dir.join("photo.png");
    std::fs::write(&file, png(3000, 2000)).expect("png");
    let tool = FakeTool::new((1024, 683));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let stand_in = printing(&dir, "answer.json", CLAUDE_ANSWER);
    let e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &all_on_path,
    );
    let mut asked = args(&file, "what does the sign say, and how many people?");
    asked.seat = Some("haiku".into());
    let outcome = look(&asked, &e).expect("an answer");
    assert_eq!(outcome.answer, "The sign reads OPEN. 3 people.");
    assert_eq!(
        (outcome.seat.as_str(), outcome.effort),
        ("haiku", Effort::Low)
    );
    assert!(!outcome.cached);
    assert_eq!(outcome.tokens.input, 900);
    assert_eq!(outcome.tokens.cache_read, 9500);
    assert_eq!(outcome.estimate_tokens, Some(925));
    // The seat read the small copy, from the copy's folder, with the measured line.
    let look_folder = dir.join(".nirvana").join("look");
    assert!(
        outcome.read.starts_with(&look_folder.display().to_string()),
        "{}",
        outcome.read
    );
    assert_eq!(outcome.cwd, look_folder);
    assert_eq!(outcome.argv[0], "claude");
    assert!(
        outcome.argv[2].contains(&outcome.read),
        "{}",
        outcome.argv[2]
    );
    // One record line, filled from the live booking, then the lease is gone.
    let lines = record_lines();
    let line = lines
        .iter()
        .find(|line| line.order == outcome.order)
        .expect("record line");
    assert_eq!(line.outcome, "pass");
    assert_eq!(line.class, CLASS);
    assert_eq!(
        line.pool.as_deref(),
        Some("max"),
        "the line was filled from the booking"
    );
    assert_eq!(line.tokens.map(|t| t.input), Some(900));
    assert!(no_live_lease(), "the lease is released");
}

#[cfg(windows)]
#[test]
fn c40c_the_same_question_again_is_a_cache_hit_and_fresh_asks_again() {
    let _home = crate::nirvana::with_test_home("c40c-cache");
    let dir = temp("cache");
    let file = dir.join("chart.png");
    std::fs::write(&file, png(900, 600)).expect("png");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let counter = dir.join("runs.txt");
    // Each start appends a line to runs.txt, then prints the answer.
    let answer = dir.join("answer.json");
    std::fs::write(&answer, CLAUDE_ANSWER).expect("answer");
    let stand_in = vec![
        "cmd.exe".into(),
        "/c".into(),
        format!(
            "echo run>> {} & type {}",
            counter.display(),
            answer.display()
        ),
    ];
    let e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &all_on_path,
    );
    let mut asked = args(&file, "the peak value?");
    asked.seat = Some("haiku".into());
    let runs = || {
        std::fs::read_to_string(&counter)
            .unwrap_or_default()
            .lines()
            .count()
    };
    let first = look(&asked, &e).expect("first");
    assert!(!first.cached);
    assert_eq!(runs(), 1);
    let cache = dir
        .join(".nirvana")
        .join("look")
        .join("chart.png.look.jsonl");
    assert!(cache.is_file(), "the cache sits beside the copies");
    // The same file, question and detail: the cached answer, nothing started, nothing leased.
    let again = look(&asked, &e).expect("again");
    assert!(again.cached);
    assert_eq!(again.answer, first.answer);
    assert_eq!(runs(), 1, "a cache hit spends nothing");
    let hit = record_lines()
        .into_iter()
        .find(|line| line.order == again.order)
        .expect("hit line");
    assert_eq!(hit.chain.as_deref(), Some(CACHE_CHAIN));
    // Another detail is another question.
    let mut high = asked.clone();
    high.high = true;
    assert!(!look(&high, &e).expect("high").cached);
    assert_eq!(runs(), 2);
    // --fresh asks again.
    let mut fresh = asked.clone();
    fresh.fresh = true;
    assert!(!look(&fresh, &e).expect("fresh").cached);
    assert_eq!(runs(), 3);
    // Changed bytes are another file.
    std::fs::write(&file, png(900, 601)).expect("changed");
    assert!(!look(&asked, &e).expect("changed").cached);
    assert_eq!(runs(), 4);
    assert!(no_live_lease());
}

#[cfg(windows)]
#[test]
fn c40c_a_failed_launch_releases_the_lease() {
    let _home = crate::nirvana::with_test_home("c40c-failed");
    let dir = temp("failed");
    let file = dir.join("shot.png");
    std::fs::write(&file, png(640, 480)).expect("png");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let stand_in = printing(&dir, "junk.txt", "Error: not signed in\n");
    let e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &all_on_path,
    );
    let mut asked = args(&file, "q");
    asked.seat = Some("haiku".into());
    let refused = look(&asked, &e).expect_err("no answer");
    assert_eq!(refused.code, 1);
    assert!(refused.line.contains("gave no answer"), "{}", refused.line);
    assert!(no_live_lease(), "the lease is released on a failed launch");
    let lines = record_lines();
    assert!(lines
        .iter()
        .any(|line| line.outcome == "failed" && line.class == CLASS));
    // A failure is never cached.
    assert!(!dir
        .join(".nirvana")
        .join("look")
        .join("shot.png.look.jsonl")
        .exists());
    // An error result with text is a failure too.
    let errored = CLAUDE_ANSWER.replace(r#""is_error":false"#, r#""is_error":true"#);
    assert!(answer_of("claude", &errored).is_err());
}

#[cfg(windows)]
#[test]
fn c40c_a_timeout_stops_the_look_and_releases_the_lease() {
    let _home = crate::nirvana::with_test_home("c40c-timeout");
    let dir = temp("timeout");
    let file = dir.join("shot.png");
    std::fs::write(&file, png(640, 480)).expect("png");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let stand_in = vec![
        "cmd.exe".into(),
        "/c".into(),
        "ping -n 30 127.0.0.1 >nul".into(),
    ];
    let mut e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &all_on_path,
    );
    e.timeout = Duration::from_secs(1);
    let mut asked = args(&file, "q");
    asked.seat = Some("haiku".into());
    let started = Instant::now();
    let refused = look(&asked, &e).expect_err("timed out");
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "{:?}",
        started.elapsed()
    );
    assert!(
        refused
            .line
            .contains("gave no answer in 1 s; it was stopped."),
        "{}",
        refused.line
    );
    assert!(no_live_lease(), "the lease is released after a timeout");
}

#[cfg(windows)]
#[test]
fn c40c_agy_answer_is_read_from_its_result() {
    let _home = crate::nirvana::with_test_home("c40c-agy");
    let dir = temp("agy");
    let file = dir.join("board.png");
    std::fs::write(&file, png(800, 600)).expect("png");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    let stand_in = printing(&dir, "agy.jsonl", AGY_ANSWER);
    let e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &all_on_path,
    );
    let mut asked = args(&file, "how many charts?");
    asked.seat = Some("flash".into());
    let outcome = look(&asked, &e).expect("agy answer");
    assert_eq!(outcome.answer, "Two charts. The top one peaks at 42.");
    assert_eq!(outcome.argv[0], "agy");
    assert!(outcome.argv.last().expect("print").contains("view_file"));
    // Probe P2: SUCCESS with an empty answer is a failure.
    let empty = AGY_ANSWER.replace("Two charts. The top one peaks at 42.", "");
    assert!(answer_of("agy", &empty).is_err());
}

#[cfg(windows)]
#[test]
fn c40c_a_long_pdf_with_no_agy_and_no_pdftoppm_is_refused_with_the_fix() {
    let _home = crate::nirvana::with_test_home("c40c-pdf");
    let dir = temp("pdf");
    let file = dir.join("manual.pdf");
    // Twelve page objects: over the ten a claude seat's Read takes whole.
    let mut text = String::from("%PDF-1.4\n");
    for n in 0..12 {
        text.push_str(&format!("{} 0 obj << /Type /Page >>\n", n + 2));
    }
    std::fs::write(&file, text).expect("pdf");
    let tool = FakeTool::new((1, 1));
    let run = |program: &str, args: &[String]| tool.run(program, args);
    // Only max is read: no agy seat has a fresh reading.
    let e = env(&dir, fresh_world(&["max"]), Vec::new(), &run, &no_pdftoppm);
    let refused = look(&args(&file, "what is on page 11?"), &e).expect_err("refused");
    assert!(
        refused.line.contains("pdftoppm is missing"),
        "{}",
        refused.line
    );
    assert!(
        refused
            .line
            .contains("winget install oschwartz10612.Poppler"),
        "{}",
        refused.line
    );
    assert!(no_live_lease());
    // With gemini read, flash (agy) takes it whole.
    let stand_in = printing(&dir, "agy.jsonl", AGY_ANSWER);
    let e = env(
        &dir,
        fresh_world(&["max", "gemini"]),
        stand_in,
        &run,
        &no_pdftoppm,
    );
    let outcome = look(&args(&file, "what is on page 11?"), &e).expect("agy takes it");
    assert_eq!(outcome.seat, "flash");
    assert_eq!(outcome.pages, Some(12));
}
