//! Plan 39 P39u: phone uploads that never landed (2026-11-06, the owner's iPhone, 0.58.1), and the fixes. Fakes
//! only, as `tests_live.rs`: a fake herdr, no Tailscale, a server on 127.0.0.1 that the test stops.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use super::http;
use super::tests_live::{
    get, post, raw, serve, serve_with, upload_head, upload_streamed, upload_timed, world, OWNER,
};

/// Everything the server logs, from every thread: the upload route runs on tokio's blocking pool, so a thread-local
/// subscriber would see nothing. nextest runs each test in its own process, so this is the only subscriber there.
fn captured() -> Arc<Mutex<Vec<u8>>> {
    static LOG: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    LOG.get_or_init(|| {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || Line(sink.clone()))
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("another subscriber is set: run this test alone, as nextest does");
        log
    })
    .clone()
}

struct Line(Arc<Mutex<Vec<u8>>>);

impl Write for Line {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn upload_lines(log: &Mutex<Vec<u8>>) -> Vec<String> {
    String::from_utf8_lossy(&log.lock().unwrap())
        .lines()
        .filter(|line| line.contains("nirvana remote upload"))
        .map(str::to_string)
        .collect()
}

/// Item 2: on 2026-11-06 both uploads failed and herdr-server.log had no line for either. Now a failure logs one
/// line with the route, the declared size, the bytes read, the time and why, never the file's bytes; the same
/// failure again within the throttle's minute adds none; the answer carries the same figures for the page's chip.
#[test]
fn a_failed_upload_logs_one_line_with_its_size_bytes_time_and_why() {
    let log = captured();
    let w = world("p39u-log");
    let running = serve_with(
        w.site(),
        http::Limits {
            read_timeout: Duration::from_millis(300),
            ..http::Limits::DEFAULT
        },
    );
    let slow = || {
        upload_streamed(running.addr, upload_head(Some(1000)), |out| {
            let _ = out.write_all(b"SECRET-BYTES-15");
            std::thread::sleep(Duration::from_secs(2));
        })
    };
    let (status, body) = slow();
    assert_eq!(
        status, 400,
        "not 408, which a browser re-sends by itself: {body}"
    );
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["error"], "the upload was too slow");
    assert_eq!(answer["read"], 15, "how far it got: {body}");
    assert!(answer["seconds"].is_u64(), "{body}");
    let lines = upload_lines(&log);
    assert_eq!(lines.len(), 1, "one line: {lines:#?}");
    let line = &lines[0];
    for part in [
        "nirvana remote upload failed",
        "route=\"/api/upload\"",
        "size=1000",
        "read=15",
        "elapsed_ms=",
        "status=400",
        "why=the upload was too slow",
    ] {
        assert!(line.contains(part), "{part} in {line}");
    }
    assert!(!line.contains("SECRET"), "never the file's bytes: {line}");
    // The same failure again: throttled, no second line.
    assert_eq!(slow().0, 400);
    assert_eq!(upload_lines(&log).len(), 1, "{:#?}", upload_lines(&log));
    // A failure on the head logs too: a 413 has read nothing.
    let (status, _, _) = raw(
        running.addr,
        upload_head(Some(super::send::UPLOAD_MOST + 1)).as_bytes(),
    );
    assert_eq!(status, 413);
    let lines = upload_lines(&log);
    assert!(
        lines.iter().any(|line| line.contains("status=413")
            && line.contains("read=0")
            && line.contains(&format!("size={}", super::send::UPLOAD_MOST + 1))),
        "{lines:#?}"
    );
    // And a saved upload says its size and time: the phone's uplink, measured.
    let (status, _, body) = post(
        running.addr,
        "/api/upload?agent=p2",
        "image/jpeg",
        b"jpeg bytes",
        OWNER,
    );
    assert_eq!(status, 200, "{body}");
    assert!(
        upload_lines(&log)
            .iter()
            .any(|line| line.contains("nirvana remote upload saved")
                && line.contains("size=10")
                && line.contains("read=10")),
        "{:#?}",
        upload_lines(&log)
    );
}

/// Item 3: the page waits for an upload as long as the server would (its `READ_TIMEOUT` plus the size at its
/// `BODY_RATE`, never past its `BODY_MOST`) and a margin, then gives up; it uploads with XHR, whose progress iPhone
/// Safari reports.
#[test]
fn the_page_gives_up_on_an_upload_just_after_the_servers_own_deadline() {
    let w = world("p39u-wait");
    let running = serve(w.site());
    let (_, _, app) = get(running.addr, "/app.js", OWNER);
    assert!(
        app.contains(&format!(
            "const READ_TIMEOUT_MS = {};",
            http::READ_TIMEOUT.as_millis()
        )),
        "the page's copy of READ_TIMEOUT"
    );
    assert!(
        app.contains(&format!("const BODY_RATE = {};", http::BODY_RATE)),
        "the page's copy of BODY_RATE"
    );
    assert!(
        app.contains(&format!(
            "const BODY_MOST_MS = {};",
            http::BODY_MOST.as_millis()
        )),
        "the page's copy of BODY_MOST"
    );
    for part in [
        "return Math.min(READ_TIMEOUT_MS + Math.ceil(bytes / BODY_RATE) * 1000, BODY_MOST_MS) + UPLOAD_MARGIN_MS;",
        "xhr.timeout = uploadWait(file.size);",
        "xhr.ontimeout = () => failed(",
        "xhr.onerror = () => failed(",
        "xhr.upload.onprogress = ",
    ] {
        assert!(app.contains(part), "{part}");
    }
    assert!(
        !app.contains("body: file"),
        "the upload is no longer a fetch with no end"
    );
}

/// Item 3: a failed chip says why on itself and has Retry; Send with one asks to send without it; "Wait for the
/// upload to finish." only while one is under way, cleared in `drawPending` so the X clears it too (review round,
/// S3); at most the server's `UPLOADS_AT_ONCE` at once, the rest waiting on the page (review round, S1).
#[test]
fn a_failed_chip_says_why_and_retries_and_send_asks_to_go_without_it() {
    let w = world("p39u-chip");
    let running = serve(w.site());
    let (_, _, chat) = get(running.addr, "/chat.js", OWNER);
    for (part, what) in [
        (
            "if (p.state === 'bad') return p.label + ' · failed: ' + p.why;",
            "the reason on the chip",
        ),
        (
            "p.state === 'bad' && h('button', { class: 'press chip-retry', type: 'button', text: 'Retry', onclick: () => start(p) }),",
            "Retry on a failed chip",
        ),
        ("if (p.req) p.req.abort();", "the X stops its upload"),
        (
            "if (failed.length && !skip) { without(failed, () => send(mode, true)); return; }",
            "send without it, asked",
        ),
        (
            "if (pending.some((p) => p.state === 'up')) { note.textContent = WAIT; return; }",
            "wait only while one uploads",
        ),
        (
            "return why + ' (' + size(r.data.read) + ' in ' + r.data.seconds + ' s)';",
            "how far it got",
        ),
        (
            "pending.splice(pending.indexOf(p), 1); drawPending(); next(); }",
            "the X gives its place to a waiting chip",
        ),
        (
            "p.queued = uploading() >= UPLOADS_AT_ONCE;",
            "past the PC's number, a chip waits",
        ),
        (
            "for (const q of pending) if (q.queued && uploading() < UPLOADS_AT_ONCE) start(q);",
            "a waiting chip starts when a place is free",
        ),
    ] {
        assert!(chat.contains(part), "{what}: {part}");
    }
    assert!(
        chat.contains(&format!(
            "const UPLOADS_AT_ONCE = {};",
            super::live::UPLOADS_AT_ONCE
        )),
        "the page's copy of UPLOADS_AT_ONCE"
    );
    // S3: the stale note is cleared in drawPending itself, which every path (the X too) goes through.
    let draw = chat
        .split("function drawPending() {")
        .nth(1)
        .and_then(|rest| rest.split("chips.replaceChildren(").next())
        .expect("drawPending");
    assert!(
        draw.contains(
            "if (note.textContent === WAIT && !pending.some((q) => q.state === 'up')) note.textContent = '';"
        ),
        "the note cleared in drawPending: {draw}"
    );
    assert!(
        !chat.contains("note.textContent = 'The upload failed: '"),
        "the reason is no longer only in the note"
    );
}

/// The repro harness (record p39u, "How the cause was looked for"): the real server, its real limits, the fake
/// herdr, left up for `P39U_SECS` so a proxy and a client outside the test can be pointed at it. Its port goes to
/// `P39U_PORT_FILE`. Run by hand: `<herdr test binary> p39u_harness --ignored --nocapture`.
#[test]
#[ignore = "the P39u repro harness, run by hand"]
fn p39u_harness_serves_for_the_proxy_repro() {
    let port_file = std::env::var("P39U_PORT_FILE").expect("P39U_PORT_FILE");
    let secs: u64 = std::env::var("P39U_SECS")
        .ok()
        .and_then(|secs| secs.parse().ok())
        .unwrap_or(120);
    let log = captured();
    let w = world("harness");
    let running = serve(w.site());
    std::fs::write(&port_file, running.addr.port().to_string()).unwrap();
    println!("serving on {} for {secs} s", running.addr);
    // The server's upload lines as they come, so a harness stopped early still shows them.
    let mut shown = 0;
    for _ in 0..secs {
        std::thread::sleep(Duration::from_secs(1));
        let lines = upload_lines(&log);
        for line in &lines[shown..] {
            println!("{line}");
        }
        shown = lines.len();
    }
    println!("uploaded: {:?}", w.uploaded());
}

/// The cause found (record p39u): two uploads into a space with no `.nirvana/uploads/` yet. Both found it missing,
/// one made it, and the other's `create_dir` failed ("Cannot create a file when that file already exists", os error
/// 183), which the page drew as a failed chip at once. Many rounds of eight at once, each round a fresh folder, so
/// the window between the check and the make is hit; every one must get the folder.
#[test]
fn uploads_at_once_into_a_new_space_all_get_the_folder() {
    let root = std::env::temp_dir().join(format!("p39u-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut failures = Vec::new();
    for round in 0..150 {
        let folder = root.join(format!("space-{round}"));
        std::fs::create_dir_all(&folder).unwrap();
        let gate = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (folder, gate) = (folder.clone(), gate.clone());
                std::thread::spawn(move || {
                    gate.wait();
                    super::send::uploads_dir(&folder)
                })
            })
            .collect();
        for thread in threads {
            if let Err(why) = thread.join().unwrap() {
                failures.push(why);
            }
        }
        assert_eq!(
            std::fs::read_to_string(folder.join(".nirvana").join("uploads").join(".gitignore"))
                .unwrap(),
            super::send::IGNORE
        );
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        failures.is_empty(),
        "{} failed: {:?}",
        failures.len(),
        failures.first()
    );
}

/// The incident's shape end to end: two photos picked at once in a space with no uploads folder, both through the
/// server at once; both land.
#[test]
fn two_photos_at_once_into_a_new_space_both_land() {
    let w = world("p39u-two");
    let running = serve(w.site());
    let addr = running.addr;
    let gate = Arc::new(std::sync::Barrier::new(2));
    let both: Vec<_> = (0..2)
        .map(|_| {
            let gate = gate.clone();
            std::thread::spawn(move || {
                gate.wait();
                upload_streamed(addr, upload_head(Some(200_000)), |out| {
                    let _ = out.write_all(&vec![b'j'; 200_000]);
                })
            })
        })
        .collect();
    for upload in both {
        let (status, body) = upload.join().unwrap();
        assert_eq!(status, 200, "{body}");
    }
    assert_eq!(w.uploaded().len(), 2, "{:?}", w.uploaded());
}

/// Review round S1: one upload may take at most `BODY_MOST`, whatever its size and rate. Here the rate alone would
/// give a 1000-byte body 1000 s; the ceiling, shortened to 1.5 s, cuts it. And the real figures: the incident's
/// 2.7 MB photo gets at least 170 s (128 kbit/s), and the ceiling is 20 minutes.
#[test]
fn an_upload_is_cut_at_the_ceiling_whatever_its_rate() {
    let photo = http::READ_TIMEOUT + Duration::from_secs(2_700_000 / http::BODY_RATE);
    assert!(photo >= Duration::from_secs(170), "{photo:?}");
    assert_eq!(http::BODY_MOST, Duration::from_secs(20 * 60));
    let w = world("p39u-ceiling");
    let running = serve_with(
        w.site(),
        http::Limits {
            read_timeout: Duration::from_millis(300),
            body_rate: 1,
            body_most: Duration::from_millis(1500),
            ..http::Limits::DEFAULT
        },
    );
    let (status, body, answered) = upload_timed(running.addr, upload_head(Some(1000)), |out| {
        // Two bytes each 100 ms for 8 s: never idle for the 0.3 s, and the rate's deadline is far off.
        for _ in 0..80 {
            if out.write_all(b"xx").is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("the upload was too slow"), "{body}");
    assert!(
        answered < Duration::from_secs(5),
        "cut at the ceiling, not when the sender stopped: {answered:?}"
    );
    assert!(w.uploaded().is_empty(), "{:?}", w.uploaded());
}

/// Review round S1: at most `UPLOADS_AT_ONCE` uploads are read at once, in all; one more is answered 503 at once and
/// logged like any other failure; a place given back is taken again.
#[test]
fn a_fifth_upload_at_once_is_told_the_pc_is_busy() {
    let log = captured();
    let w = world("p39u-busy");
    let running = serve(w.site());
    let addr = running.addr;
    let holding: Vec<_> = (0..super::live::UPLOADS_AT_ONCE)
        .map(|_| {
            let (go, wait) = mpsc::channel::<()>();
            let upload = std::thread::spawn(move || {
                upload_streamed(addr, upload_head(Some(1000)), move |out| {
                    let _ = out.write_all(&[b'h'; 10]);
                    let _ = wait.recv();
                    let _ = out.write_all(&[b'h'; 990]);
                })
            });
            (go, upload)
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));
    let (status, body) = upload_streamed(addr, upload_head(Some(10)), |out| {
        let _ = out.write_all(&[b'f'; 10]);
    });
    assert_eq!(status, 503, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answer["error"], "the PC is busy with other uploads");
    assert!(
        upload_lines(&log)
            .iter()
            .any(|line| line.contains("status=503")
                && line.contains("why=the PC is busy with other uploads")),
        "{:#?}",
        upload_lines(&log)
    );
    for (go, upload) in holding {
        let _ = go.send(());
        let (status, body) = upload.join().unwrap();
        assert_eq!(status, 200, "{body}");
    }
    let (status, _, body) = post(addr, "/api/upload?agent=p2", "image/jpeg", b"after", OWNER);
    assert_eq!(status, 200, "the places were given back: {body}");
    assert_eq!(w.uploaded().len(), super::live::UPLOADS_AT_ONCE + 1);
}

/// Review round S2: a caller that is still sending when its body is cut, and reads only once it has sent it all, gets
/// the 400. The server drains the rest of the declared length for up to `CUT_DRAIN`; with the 1 s drain every other
/// answer has, it closed while this caller still sent.
#[test]
fn a_slow_caller_that_reads_after_sending_still_gets_the_400() {
    let w = world("p39u-drain");
    let running = serve_with(
        w.site(),
        http::Limits {
            read_timeout: Duration::from_millis(300),
            body_rate: 1000,
            ..http::Limits::DEFAULT
        },
    );
    let mut stream = TcpStream::connect(running.addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    stream
        .write_all(upload_head(Some(3000)).as_bytes())
        .unwrap();
    // 3000 bytes at 1000/s get 0.3 s plus 3 s. Slowly for 4 s, past the cut; then the rest over 2.5 s, past the
    // 1 s drain; then read.
    for _ in 0..40 {
        let _ = stream.write_all(b"ss");
        std::thread::sleep(Duration::from_millis(100));
    }
    for _ in 0..10 {
        let _ = stream.write_all(&[b's'; 292]);
        std::thread::sleep(Duration::from_millis(250));
    }
    let mut answer = Vec::new();
    let read = stream.read_to_end(&mut answer);
    let text = String::from_utf8_lossy(&answer);
    assert!(
        text.starts_with("HTTP/1.1 400 "),
        "the 400 arrived ({read:?}): {text:?}"
    );
    assert!(text.contains("the upload was too slow"), "{text}");
    assert!(w.uploaded().is_empty(), "{:?}", w.uploaded());
}

/// Plan 40 review B5: a zero-byte upload (iOS hands Safari a 0-byte placeholder for a photo still downloading from
/// iCloud) is refused with "the file is empty" and leaves no file, not even its `.partial`.
#[test]
fn a_zero_byte_upload_is_refused_and_leaves_no_file() {
    let w = world("c40e-empty");
    let running = serve(w.site());
    let (status, _, body) = post(
        running.addr,
        "/api/upload?agent=p2",
        "image/jpeg",
        b"",
        OWNER,
    );
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("the file is empty"), "{body}");
    assert!(w.uploaded().is_empty(), "{:?}", w.uploaded());
    let _ = std::fs::remove_dir_all(w.folder.parent().unwrap());
}

/// A scratch folder for the note tests, removed by the test (plan 40: tests that stop leaking temp folders).
fn note_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("herdr-c40e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A JPEG's start: SOI, then a SOF0 frame header for 4000 x 3000.
const JPEG_HEAD: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x0B, 0xB8, 0x0F,
    0xA0, 0x03, 0x01, 0x22, 0x00,
];

/// Plan 40 "phone photos tell the agent what they are": the exact note after a jpg, a png, a heic, a pdf, an mp4,
/// and none after a txt. Every figure is the server's own reading of the file.
#[test]
fn a_sent_photo_carries_a_note_naming_herdr_look() {
    let dir = note_dir("notes");
    let file = |name: &str, bytes: &[u8]| {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let jpg = file("phone-20261109-143012-9f3a1c2e.jpg", JPEG_HEAD);
    let mut png_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    png_bytes.extend_from_slice(&1170u32.to_be_bytes());
    png_bytes.extend_from_slice(&2532u32.to_be_bytes());
    let png = file("phone-20261109-143012-9f3a1c2f.png", &png_bytes);
    let heic = file("phone-20261109-143012-9f3a1c30.heic", b"ftypheic!");
    let pdf = file("phone-20261109-143012-9f3a1c31.pdf", b"%PDF-1.7");
    let mp4 = file("phone-20261109-143012-9f3a1c32.mp4", &[0u8; 2048]);
    let txt = file("phone-20261109-143012-9f3a1c33.txt", b"notes");
    let at = |path: &std::path::Path| path.display().to_string();
    let look = |path: &std::path::Path| {
        format!(
            "in a Nirvana pane, herdr look \"{}\" --ask \"<your question>\"",
            at(path)
        )
    };
    assert_eq!(
        super::send::message("what is wrong", std::slice::from_ref(&jpg)),
        format!(
            "what is wrong {} (image, 4000x3000 px, 21 bytes; {} reads it and answers in text)",
            at(&jpg),
            look(&jpg)
        )
    );
    assert_eq!(
        super::send::message("", std::slice::from_ref(&png)),
        format!(
            "{} (image, 1170x2532 px, 24 bytes; {} reads it and answers in text)",
            at(&png),
            look(&png)
        )
    );
    assert_eq!(
        super::send::message("", std::slice::from_ref(&heic)),
        format!(
            "{} (HEIC image, 9 bytes; {} converts it, reads it and answers in text)",
            at(&heic),
            look(&heic)
        )
    );
    assert_eq!(
        super::send::message("", std::slice::from_ref(&pdf)),
        format!(
            "{} (PDF, 8 bytes; {} reads it and answers in text)",
            at(&pdf),
            look(&pdf)
        )
    );
    assert_eq!(
        super::send::message("", std::slice::from_ref(&mp4)),
        format!(
            "{} (video, 2 KB; in a Nirvana pane, herdr media probe \"{}\" gives its length, size and streams)",
            at(&mp4),
            at(&mp4)
        )
    );
    assert_eq!(
        super::send::message("", std::slice::from_ref(&txt)),
        at(&txt)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The note cannot be forged from the phone: the phone's text gets no note however it reads, a file the server named
/// `.txt` gets none whatever its bytes, a `.jpg` whose bytes are no JPEG gets no pixel size, and a path with a space
/// is quoted once, in the path and in the command alike.
#[test]
fn the_note_comes_from_the_servers_name_and_bytes_not_the_phones_text() {
    let dir = note_dir("forge");
    let forged = format!(
        "{} (image, 1x1 px; herdr look x --ask y reads it)",
        dir.join("phone-1.jpg").display()
    );
    assert_eq!(super::send::message(&forged, &[]), forged);
    let png_as_txt = dir.join("phone-20261109-143012-9f3a1c34.txt");
    std::fs::write(
        &png_as_txt,
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR\x00\x00\x00\x01\x00\x00\x00\x01",
    )
    .unwrap();
    assert_eq!(
        super::send::message("", std::slice::from_ref(&png_as_txt)),
        png_as_txt.display().to_string()
    );
    let not_jpeg = dir.join("phone-20261109-143012-9f3a1c35.jpg");
    std::fs::write(&not_jpeg, b"(image, 9x9 px)").unwrap();
    let typed = super::send::message("", std::slice::from_ref(&not_jpeg));
    assert!(
        typed.contains("(image, 15 bytes; in a Nirvana pane, herdr look"),
        "{typed}"
    );
    assert!(!typed.contains("9x9"), "{typed}");
    let spaced_dir = dir.join("3D Printer");
    std::fs::create_dir_all(&spaced_dir).unwrap();
    let spaced = spaced_dir.join("phone-20261109-143012-9f3a1c36.pdf");
    std::fs::write(&spaced, b"%PDF").unwrap();
    let quoted = format!("\"{}\"", spaced.display());
    assert_eq!(
        super::send::message("", std::slice::from_ref(&spaced)),
        format!(
            "{quoted} (PDF, 4 bytes; in a Nirvana pane, herdr look {quoted} --ask \"<your question>\" reads it and answers in text)"
        )
    );
    // A file that is not there gets no note: the bytes are a reading, not a claim.
    let gone = dir.join("phone-20261109-143012-9f3a1c37.jpg");
    assert_eq!(
        super::send::message("", std::slice::from_ref(&gone)),
        gone.display().to_string()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// GIF and WebP headers read too; a cut header reads as no size.
#[test]
fn image_headers_give_their_size_cheaply() {
    assert_eq!(
        super::send::header_size(b"GIF89a\x40\x01\xf0\x00"),
        Some((320, 240))
    );
    let mut vp8x = b"RIFF\x00\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x00\x00\x00\x00".to_vec();
    vp8x.extend_from_slice(&[0x7F, 0x07, 0x00, 0x37, 0x04, 0x00]);
    assert_eq!(super::send::header_size(&vp8x), Some((1920, 1080)));
    assert_eq!(super::send::header_size(&JPEG_HEAD[..12]), None);
    assert_eq!(super::send::header_size(b"plain"), None);
}
