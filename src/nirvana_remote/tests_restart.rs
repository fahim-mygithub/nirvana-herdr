//! Plan 48: the phone's Restart Nirvana (`POST /api/restart`), against fakes only: a herdr with canned lists, a
//! Tailscale never asked, a peer check that admits, and a restart seam that counts its calls and starts nothing. The
//! identity is the made-up header tailscaled would set. The page's script is read as served; the restart path itself
//! runs in node (nirvana/test_phonerestart.py).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use super::herdr::Herdr;
use super::routes::{self, Site};
use super::tests_live::{
    from_page, get, post, raw, serve, Admit, NoProbe, NoTailscale, DESK_IP, HOST, OWNER, PHONE,
    REFUSED,
};

/// Two spaces; one agent working, one blocked on the owner, one idle.
struct Lists;

impl Herdr for Lists {
    fn call(&self, method: &str, _params: Value) -> Result<Value, String> {
        match method {
            "workspace.list" => Ok(json!({"workspaces": [
                {"workspace_id": "w1", "label": "made-up-space"},
                {"workspace_id": "w2", "label": "other-space"},
            ]})),
            "agent.list" => Ok(json!({"agents": [
                {"pane_id": "p1", "workspace_id": "w1", "agent_status": "working"},
                {"pane_id": "p2", "workspace_id": "w1", "agent_status": "blocked"},
                {"pane_id": "p3", "workspace_id": "w2", "agent_status": "idle"},
            ]})),
            other => Err(format!("unexpected {other}")),
        }
    }
}

/// The site with a restart seam that counts, and answers `answer`.
fn site(asked: Arc<AtomicUsize>, answer: Result<(), &'static str>) -> Arc<Site> {
    let mut site = Site::new(
        Arc::new(NoTailscale),
        Arc::new(Lists),
        Arc::new(Admit),
        Arc::new(NoProbe),
        super::HTTPS_PORT,
        super::tests_live::fixed_now,
    );
    *site.owner.write().unwrap() = Some(OWNER.into());
    *site.me.write().unwrap() = Some(routes::Me {
        host: HOST.into(),
        ips: vec![DESK_IP.parse().unwrap()],
    });
    site.restart = Arc::new(move || {
        asked.fetch_add(1, Ordering::SeqCst);
        answer.map_err(str::to_owned)
    });
    Arc::new(site)
}

fn restart_post(addr: std::net::SocketAddr) -> (u16, String, String) {
    post(addr, "/api/restart", "application/json", b"{}", OWNER)
}

#[test]
fn the_phones_restart_starts_the_worker_and_says_how_many_were_working() {
    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Ok(())));
    let (status, _, body) = restart_post(running.addr);
    assert_eq!(status, 202, "{body}");
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["restarting"], true);
    assert_eq!(body["working"], 2, "the working and the blocked turns stop");
    assert_eq!(
        body["server"],
        std::process::id(),
        "the page tells the new server from this one by it"
    );
    assert_eq!(asked.load(Ordering::SeqCst), 1, "one worker");
}

/// The page asks `/api/hello` while the PC restarts; the server it names is how it knows the new one came.
#[test]
fn hello_names_the_server_that_answers() {
    let running = serve(site(Arc::new(AtomicUsize::new(0)), Ok(())));
    let (status, _, body) = get(running.addr, "/api/hello", OWNER);
    assert_eq!(status, 200);
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["server"], std::process::id());
    assert_eq!(body["working"], 2, "the count the confirm shows");
}

#[test]
fn a_get_or_another_method_is_refused_and_starts_nothing() {
    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Ok(())));
    let (status, head, _) = get(running.addr, "/api/restart", OWNER);
    assert_eq!(status, 405, "a GET: {head}");
    assert!(head.contains("Allow: POST"), "{head}");
    for method in ["PUT", "DELETE", "HEAD"] {
        let request = format!(
            "{method} /api/restart HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: \
             {OWNER}\r\n{}Content-Length: 0\r\n\r\n",
            from_page()
        );
        let (status, head, _) = raw(running.addr, request.as_bytes());
        assert_eq!(status, 405, "{method}: {head}");
    }
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_missing_or_wrong_owner_is_refused_and_starts_nothing() {
    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Ok(())));
    let (status, _, body) = post(
        running.addr,
        "/api/restart",
        "application/json",
        b"{}",
        "someone-else@example.com",
    );
    assert_eq!((status, body.as_str()), (403, REFUSED), "another login");
    let missing = format!(
        "POST /api/restart HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\n{}Content-Type: \
         application/json\r\nContent-Length: 2\r\n\r\n{{}}",
        from_page()
    );
    let (status, _, body) = raw(running.addr, missing.as_bytes());
    assert_eq!((status, body.as_str()), (403, REFUSED), "no login at all");
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_cross_site_restart_is_refused_and_starts_nothing() {
    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Ok(())));
    let with = |headers: &str| {
        let request = format!(
            "POST /api/restart HTTP/1.1\r\nHost: {HOST}\r\nX-Forwarded-For: {PHONE}\r\nTailscale-User-Login: \
             {OWNER}\r\n{headers}Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}"
        );
        raw(running.addr, request.as_bytes())
    };
    let (status, _, _) = with("Sec-Fetch-Site: cross-site\r\nOrigin: https://made-up.example\r\n");
    assert_eq!(status, 403, "another site's page on the phone");
    let (status, _, _) = with("Sec-Fetch-Site: same-origin\r\nOrigin: https://made-up.example\r\n");
    assert_eq!(status, 403, "an Origin that is not this PC's page");
    let (status, _, _) = with("");
    assert_eq!(status, 403, "neither header: not a browser on this page");
    assert_eq!(asked.load(Ordering::SeqCst), 0);
}

#[test]
fn a_restart_that_is_not_json_or_cannot_start_says_so() {
    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Ok(())));
    let (status, _, _) = post(running.addr, "/api/restart", "text/plain", b"{}", OWNER);
    assert_eq!(status, 415, "a form post cannot reach it");
    assert_eq!(asked.load(Ordering::SeqCst), 0);

    let asked = Arc::new(AtomicUsize::new(0));
    let running = serve(site(asked.clone(), Err("made-up reason")));
    let (status, _, body) = restart_post(running.addr);
    assert_eq!(status, 503, "{body}");
    assert!(body.contains("made-up reason"), "{body}");
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}

/// The site a test builds asks nothing of the PC: a `Site::new` restart starts no worker.
#[test]
fn a_new_site_starts_no_worker() {
    let site = Site::new(
        Arc::new(NoTailscale),
        Arc::new(Lists),
        Arc::new(Admit),
        Arc::new(NoProbe),
        super::HTTPS_PORT,
        super::tests_live::fixed_now,
    );
    assert!((site.restart)().is_err());
}

/// The served page: a Restart Nirvana control on the spaces screen, a confirm that says how many agents are working
/// and that their turns stop, the POST, and the shell's wait for the new server, which ends by itself.
#[test]
fn the_page_confirms_a_restart_and_comes_back_when_the_new_server_answers() {
    let running = serve(site(Arc::new(AtomicUsize::new(0)), Ok(())));
    let (_, _, spaces) = get(running.addr, "/spaces.js", OWNER);
    let (_, _, app) = get(running.addr, "/app.js", OWNER);
    assert!(spaces.contains("text: 'Restart Nirvana'"), "the control");
    assert!(
        spaces.contains("nv.getJson('/api/hello', { quiet: true })"),
        "the count is read when the confirm opens"
    );
    assert!(
        spaces.contains("' working now: their turns stop.'"),
        "the confirm says what stops"
    );
    assert!(
        spaces.contains("nv.postJson('/api/restart', {})"),
        "the POST, from this page"
    );
    assert!(
        spaces.contains("close(() => nv.restarting(answer.data.server));"),
        "then the shell waits"
    );
    assert!(
        app.contains("const back = response.ok && data.server && data.server !== wait.old;"),
        "a new server, not the old one still answering"
    );
    assert!(
        app.contains("if (reach.restarting) return;"),
        "no error screen while it restarts"
    );
}
