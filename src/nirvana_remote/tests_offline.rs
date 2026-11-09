//! Plan 39 P39o: the error page when the phone cannot reach the PC (record p39o). Fakes only, as `tests_live.rs`: a
//! fake herdr, no Tailscale, a server on 127.0.0.1 that the test stops. The page's JavaScript does not run here; its
//! lines are read as text, and the offline path itself was run once in headless Chromium (the record says how).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use super::routes::CSP;
use super::shell;
use super::tests_live::{get, proxied, serve, world, OWNER};

/// A GET's head and its body as bytes (the icons are not text).
fn fetch(addr: SocketAddr, path: &str) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream
        .write_all(format!("{}\r\n", proxied("GET", path, OWNER)).as_bytes())
        .unwrap();
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).unwrap();
    super::tests_live::whole(&answer, false);
    let end = answer.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&answer[..end]).to_string(),
        answer[end + 4..].to_vec(),
    )
}

fn before(text: &str, first: &str, then: &str) -> bool {
    match (text.find(first), text.find(then)) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

/// Item 1: the worker as the PC serves it names its cache for this build, lists the shell, and has one fetch
/// handler, which leaves every `/api/` request (and anything not in the shell) to the network.
#[test]
fn the_worker_names_its_cache_for_this_build_and_never_answers_api() {
    let w = world("offline-worker");
    let running = serve(w.site());
    let (head, body) = fetch(running.addr, "/sw.js");
    let sw = String::from_utf8(body).unwrap();
    assert!(
        head.contains("200 OK") && head.contains("text/javascript"),
        "{head}"
    );
    assert!(head.contains("Cache-Control: no-store"), "{head}");
    assert!(
        head.contains(&format!("Content-Security-Policy: {CSP}")),
        "the CSP as tight as before: {head}"
    );

    let build = shell::build();
    assert!(
        build.starts_with(&format!("{}-", crate::nirvana::VERSION)) && build.len() > 17,
        "{build}"
    );
    assert!(
        sw.contains(&format!("const BUILD = '{build}';")),
        "the build written in"
    );
    let list = serde_json::to_string(&shell::paths()).unwrap();
    assert!(
        sw.contains(&format!("const SHELL = {list};")),
        "the shell's paths written in"
    );
    assert!(
        !sw.contains("NIRVANA_BUILD") && !sw.contains("NIRVANA_SHELL"),
        "no mark left"
    );
    assert!(sw.contains("const PREFIX = 'nirvana-shell-';"));
    assert!(sw.contains("const CACHE = PREFIX + BUILD;"));
    // An older build's cache goes when this worker takes over.
    assert!(sw.contains(
        "    if (name.startsWith(PREFIX) && name !== CACHE) await caches.delete(name);\n"
    ));

    assert_eq!(
        sw.matches("addEventListener('fetch'").count(),
        1,
        "one fetch handler"
    );
    let handler = sw.split("addEventListener('fetch'").nth(1).unwrap();
    let handler = &handler[..handler.find("\n});").unwrap()];
    for guard in [
        "  if (request.method !== 'GET') return;\n",
        "  if (url.origin !== self.location.origin) return;\n",
        "  if (url.pathname.startsWith('/api/')) return;\n",
        "  if (!SHELL.includes(url.pathname)) return;\n",
    ] {
        assert!(
            before(handler, guard, "event.respondWith("),
            "{guard:?} before respondWith: {handler}"
        );
    }

    // The PC first: the cache answers only without an answer from the PC, or with one that is not the page.
    let file = sw.split("async function shellFile").nth(1).unwrap();
    assert!(file.contains("  if (kept && Date.now() < downUntil) return kept;\n"));
    assert!(
        file.contains("    return NOT_THE_PAGE.includes(answer.status) ? kept : answer;\n"),
        "{file}"
    );
    assert!(sw.contains("const NOT_THE_PAGE = [403, 421, 502, 503, 504];"));
    assert!(sw.contains("const WAIT_MS = 4000;"));
    assert!(
        file.contains("    if (answer.ok) {\n      const saved = cache.put(path, answer.clone())")
    );
    // A failed file does not fail the install, or the phone would lose its worker and its alerts.
    let install = sw
        .split("event.waitUntil((async () => {\n  const cache = await caches.open(CACHE);")
        .nth(1)
        .unwrap();
    let install = &install[..install.find("})()));").unwrap()];
    assert!(install.contains("    } catch (_) {"), "{install}");
    assert!(install.contains("      if (answer.ok) await cache.put(path, answer);\n"));
    // P39e's alerts are still there.
    assert!(sw.contains("showNotification"));
}

/// Item 1: each shell file is served as the worker keeps it, with the page's headers; none is under `/api/`; every
/// file the page loads is in it; the page and `/api/hello` carry the build tag.
#[test]
fn every_shell_file_is_served_whole_and_the_page_loads_nothing_outside_it() {
    let w = world("offline-shell");
    let running = serve(w.site());
    let paths = shell::paths();
    for (path, bytes) in shell::files() {
        assert!(!path.starts_with("/api/") && path != "/sw.js", "{path}");
        let (head, body) = fetch(running.addr, path);
        assert!(head.contains("200 OK"), "{path}: {head}");
        assert!(
            head.contains(&format!("Content-Security-Policy: {CSP}")),
            "{path}"
        );
        if path == "/" {
            assert_eq!(body, shell::index().as_bytes(), "/");
        } else {
            assert!(
                body == bytes,
                "{path}: {} of {} bytes",
                body.len(),
                bytes.len()
            );
        }
    }

    let (_, page) = fetch(running.addr, "/");
    let page = String::from_utf8(page).unwrap();
    assert!(page.contains(&format!(
        r#"<meta name="nirvana-build" content="{}">"#,
        shell::build()
    )));
    let mut local = Vec::new();
    for attr in [r#"src=""#, r#"href=""#] {
        for piece in page.split(attr).skip(1) {
            let link = &piece[..piece.find('"').unwrap()];
            if link.starts_with('/') {
                local.push(link.to_string());
            }
        }
    }
    assert!(local.len() >= 10, "{local:?}");
    for link in &local {
        assert!(paths.contains(&link.as_str()), "{link} is not in the shell");
    }
    let (_, manifest) = fetch(running.addr, "/manifest.webmanifest");
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    for icon in manifest["icons"].as_array().unwrap() {
        let src = icon["src"].as_str().unwrap();
        assert!(paths.contains(&src), "{src}");
    }

    let (status, head, _) = get(running.addr, "/api/hello", OWNER);
    assert_eq!(status, 200);
    assert!(
        head.contains(&format!("Nirvana-Build: {}", shell::build())),
        "{head}"
    );
}

/// Item 1: a new build replaces the old shell: the tag changes with the version, with any byte of any file, and with
/// a path; the served tag is the one over the shell and the worker.
#[test]
fn a_change_to_any_shell_file_or_the_version_names_a_new_cache() {
    let files: Vec<(&str, &[u8])> = vec![("/", b"<html>"), ("/app.js", b"one")];
    let base = shell::build_tag("0.58.2", &files);
    assert_ne!(base, shell::build_tag("0.58.3", &files), "the version");
    let changed: Vec<(&str, &[u8])> = vec![("/", b"<html>"), ("/app.js", b"onf")];
    assert_ne!(base, shell::build_tag("0.58.2", &changed), "one byte");
    let moved: Vec<(&str, &[u8])> = vec![("/", b"<html>"), ("/app2.js", b"one")];
    assert_ne!(base, shell::build_tag("0.58.2", &moved), "a path");
    // The boundary between path and body counts: "/a" + "bc" is not "/ab" + "c".
    let split_a: Vec<(&str, &[u8])> = vec![("/a", b"bc")];
    let split_b: Vec<(&str, &[u8])> = vec![("/ab", b"c")];
    assert_ne!(
        shell::build_tag("v", &split_a),
        shell::build_tag("v", &split_b)
    );
    assert_eq!(
        base,
        shell::build_tag("0.58.2", &files),
        "the same files, the same tag"
    );

    let mut all = shell::files();
    let worker = super::alerts::FILES
        .iter()
        .find(|(path, _, _)| *path == "/sw.js")
        .unwrap()
        .2;
    all.push(("/sw.js", worker));
    assert_eq!(
        shell::build(),
        shell::build_tag(crate::nirvana::VERSION, &all)
    );
}

/// Items 2 to 4: the error screen's copy, its checks in order, the cases it tells apart, and how it comes and goes.
#[test]
fn the_error_screen_says_which_case_and_what_to_check() {
    let w = world("offline-screen");
    let running = serve(w.site());
    let (_, app) = fetch(running.addr, "/app.js");
    let app = String::from_utf8(app).unwrap();
    // Item 4: the cases.
    assert!(app.contains("    if (status === 403 || status === 421) return 'refused';\n"));
    assert!(app.contains(
        "    if (status >= 502 && status <= 504 && !(kind || '').includes('json')) return 'nirvana';\n"
    ));
    assert!(app.contains(
        "  function unreachable() { return navigator.onLine === false ? 'offline' : 'unreachable'; }\n"
    ));
    assert!(
        app.contains("      why = unreachable();\n"),
        "no answer: offline or unreachable"
    );
    // Item 2: the titles, and the refusal names the account, not reach.
    assert!(app.contains(r#"unreachable: { title: "Can't reach your PC","#));
    assert!(app.contains("offline: { title: 'This phone is offline',"));
    assert!(app.contains(
        r#"refused: { title: 'The PC refused this phone', lead: "This phone's Tailscale account is not the PC's."#
    ));
    let refused = app.split("    refused: {").nth(1).unwrap();
    let refused = &refused[..refused.find('\n').unwrap()];
    assert!(!refused.contains("reach"), "{refused}");
    // The checklist, most likely first, each with how to check it; the VPN is the owner's check, not the page's.
    assert!(app.contains("order: ['tailscale', 'awake', 'nirvana', 'online'] },"));
    assert!(app.contains("    tailscale: ['Tailscale is on on this phone', 'Open the Tailscale app: the VPN switch at the top is on, and it says Connected. This page cannot see the VPN; only you can.'],"));
    assert!(app.contains("    awake: ['The PC is on and awake',"));
    assert!(app.contains(r#"    nirvana: ['Nirvana is open on the PC', "The Nirvana sidebar's bottom row reads ● remote :8443, its dot green."],"#));
    assert!(
        app.contains("        h('span', { class: 'reach-how', text: net ? online() : how })));")
    );
    assert!(app.contains("    return navigator.onLine === false\n"));
    // Try again, and the automatic tries with back-off.
    assert!(app.contains("h('button', { class: 'press reach-try', type: 'button', text: 'Try again', onclick: tryNow }),"));
    assert!(app.contains("  const RETRY_S = [2, 4, 8, 15, 30];"));
    assert!(app.contains("    let left = RETRY_S[Math.min(reach.tries, RETRY_S.length - 1)];\n"));
    // Item 3: a live view down LOST_MS is checked; the check waits PROBE_MS.
    assert!(app.contains("  const LOST_MS = 10000;"));
    assert!(app.contains("  const PROBE_MS = 6000;"));
    assert!(app.contains("        lost();\n        if (on.state) on.state(false);\n"));
    assert!(app.contains(
        "      source.onopen = () => { wait = 1000; answered(); if (on.state) on.state(true); };"
    ));
    // Every request reports what came back; one with no answer waits as a live view does; only a failed load
    // (not a send, whose typed text would go) has its screen drawn again.
    assert!(app.contains("    if (load) reach.dirty = true;\n"));
    assert!(app.contains("      trouble('network', !options.method);\n"));
    assert!(app.contains("    if (why === 'network') lost();\n    else fallen(why);\n"));
    assert!(app.contains("    const response = await reached(path, { credentials: 'same-origin', cache: 'no-store' });\n"));
    assert!(app.contains("      xhr.addEventListener('error', () => trouble('network'));"));
    // Back where the owner was: the screen stays under the error screen, a failed screen is drawn again, live views
    // open at once; a screen change keeps the error screen on top.
    assert!(
        app.contains("    if (reach.dirty && current) show(current.name, 'f', current.params);\n")
    );
    assert!(app.contains("    for (const revive of reach.revive) revive();\n"));
    assert!(
        app.contains("    if (reach.layer) document.getElementById('nv').append(reach.layer);\n")
    );
    // A check as the page opens; a shell of another build reloads.
    assert!(app.contains("  document.addEventListener('DOMContentLoaded', check);\n"));
    assert!(app.contains("    else if (!stale(build)) up();\n"));
    assert!(app.contains("    if (!build || !mine || build === mine) return false;\n"));
    // The live views still say "Reconnecting" first.
    for (path, line) in [
        (
            "/spaces.js",
            "if (!live) note.textContent = 'Reconnecting to the PC…';",
        ),
        (
            "/chat.js",
            "if (!on) note.textContent = 'Reconnecting to the PC…';",
        ),
    ] {
        let (_, script) = fetch(running.addr, path);
        assert!(String::from_utf8(script).unwrap().contains(line), "{path}");
    }
    // The screen's look: Nirvana Dark, from the page's own sheet (no inline style: the CSP).
    let (_, css) = fetch(running.addr, "/app.css");
    let css = String::from_utf8(css).unwrap();
    assert!(
        css.contains(".reach { position: absolute; inset: 0; z-index: 30; background: var(--bg);")
    );
}

/// Item 7: the home-screen app is laid out for the whole screen (the owner's iPhone, 2026-11-08: empty bands above
/// the header and below the composer); a Safari tab keeps its old room. The layout itself was measured in headless
/// Chromium with emulated insets (record p39o); this holds the lines that make it.
#[test]
fn the_home_screen_app_is_laid_out_for_the_whole_screen() {
    let w = world("offline-layout");
    let running = serve(w.site());
    let (_, css) = fetch(running.addr, "/app.css");
    let css = String::from_utf8(css).unwrap();
    // The insets are read in one place, and every screen pads with them.
    assert!(css.contains("  --safe-top: env(safe-area-inset-top, 0px);\n"));
    assert!(css.contains("  --safe-bottom: env(safe-area-inset-bottom, 0px);\n"));
    assert_eq!(
        css.matches("env(safe-area-inset").count(),
        4,
        "only in :root"
    );
    assert!(css.contains("padding: calc(var(--gap-top) + var(--safe-top)) 12px 10px;"));
    assert!(css.contains("padding: 10px 12px calc(var(--gap-bottom) + var(--safe-bottom));"));
    // A tab keeps the prototype's room; the app keeps a small gap past the insets.
    assert!(css.contains("  --gap-top: 48px;\n  --gap-bottom: 28px;\n"));
    assert!(css.contains(":root.standalone { --gap-top: 10px; --gap-bottom: 10px; }"));
    // The keyboard covers the home indicator.
    assert!(css.contains(":root.standalone.kb { --safe-bottom: 0px; }"));
    // The app's column is the screen, not a height from vh, and follows the visual viewport.
    assert!(css.contains(
        ":root.standalone .nv {\n  position: fixed;\n  top: var(--vv-top, 0px);\n  bottom: var(--vv-bottom, 0px);\n"
    ));
    // Landscape: the column stays clear of a notch on either side.
    assert!(css.contains(
        "  max-width: min(430px, calc(100vw - 2 * max(var(--safe-left), var(--safe-right))));\n"
    ));

    let (_, app) = fetch(running.addr, "/app.js");
    let app = String::from_utf8(app).unwrap();
    assert!(app.contains("  const installed = navigator.standalone === true\n    || media('(display-mode: standalone)') || media('(display-mode: fullscreen)');\n"));
    assert!(app.contains("  if (installed) {\n    root.classList.add('standalone');\n"));
    assert!(app.contains(
        "    const below = Math.max(0, Math.round(window.innerHeight - view.height - view.offsetTop));\n"
    ));
    assert!(app.contains("    root.style.setProperty('--vv-bottom', below + 'px');\n"));
    assert!(app.contains("    root.classList.toggle('kb', below > KEYBOARD_PX);\n"));
    assert!(app.contains("      window.visualViewport.addEventListener('resize', fit);\n"));

    let (_, _, page) = get(running.addr, "/", OWNER);
    assert!(page.contains("viewport-fit=cover"), "edge to edge");
    assert!(page.contains(
        r#"<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">"#
    ));
}

/// Item 8: "On the PC now" shows content, not Claude Code's empty input box. A screen as the owner's screenshot of
/// 2026-11-08 showed it: six rules, the box's titled rule, the prompt line, seven rules, a short one, a blank, the
/// status lines.
#[test]
fn the_pc_now_box_collapses_runs_of_blank_and_rule_lines() {
    let rule = "─".repeat(80);
    let mut screen = vec![
        "  - Restart Claude Code to load the new MCP tools".to_string(),
        String::new(),
        "* Crunched for 48s · done 11:28 PM".to_string(),
        String::new(),
        "136465 tokens".to_string(),
    ];
    screen.extend(std::iter::repeat_n(rule.clone(), 6));
    screen.push(format!("{} Nirvana ─", "─".repeat(46)));
    screen.push("❯ fix the e2e fixture and run those two tests".to_string());
    screen.extend(std::iter::repeat_n(format!("{rule}   "), 7));
    screen.push("━".repeat(60));
    screen.push(String::new());
    screen.push("  Claude 5h 19% 3h42m · Gemini week 7% 4d19h".to_string());
    screen.push("═".repeat(10) + "  " + &"─".repeat(10));
    screen.push("  ⏵⏵ auto mode on (shift+tab to cycle) · ← 1 agent".to_string());
    screen.push(String::new());
    let read = serde_json::json!({ "read": { "text": screen.join("\n") } });
    let live = super::chat::live(&read);
    let want = [
        "  - Restart Claude Code to load the new MCP tools",
        "",
        "* Crunched for 48s · done 11:28 PM",
        "",
        "136465 tokens",
        rule.as_str(),
        &format!("{} Nirvana ─", "─".repeat(46)),
        "❯ fix the e2e fixture and run those two tests",
        rule.as_str(),
        "  Claude 5h 19% 3h42m · Gemini week 7% 4d19h",
        &("═".repeat(10) + "  " + &"─".repeat(10)),
        "  ⏵⏵ auto mode on (shift+tab to cycle) · ← 1 agent",
    ]
    .join("\n");
    assert_eq!(live, want);
    // A run that starts blank keeps a rule, which still separates.
    let read = serde_json::json!({ "read": { "text": format!("a\n\n{rule}\n\nb") } });
    assert_eq!(super::chat::live(&read), format!("a\n{rule}\nb"));
    // Blank lines alone still collapse to one, as before.
    let read = serde_json::json!({ "read": { "text": "a\n\n\n\nb\n\n" } });
    assert_eq!(super::chat::live(&read), "a\n\nb");
}
