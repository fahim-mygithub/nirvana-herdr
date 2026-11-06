//! Plan 39 P39u: phone uploads that never landed (2026-11-06, the owner's iPhone, 0.58.1), and the fixes. Fakes
//! only, as `tests_live.rs`: a fake herdr, no Tailscale, a server on 127.0.0.1 that the test stops.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use super::http;
use super::tests_live::{
    get, post, raw, serve, serve_with, upload_head, upload_streamed, world, OWNER,
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
/// `BODY_RATE`) and a margin, then gives up; it uploads with XHR, whose progress iPhone Safari reports.
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
    for part in [
        "return READ_TIMEOUT_MS + Math.ceil(bytes / BODY_RATE) * 1000 + UPLOAD_MARGIN_MS;",
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
/// upload to finish." only while one is under way.
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
    ] {
        assert!(chat.contains(part), "{what}: {part}");
    }
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
