//! The page's shell as the phone keeps it for when the PC cannot be reached (plan 39, package P39o, record p39o).
//!
//! The owner, 2026-11-08: "Can you also add a tailsscale check to the UI when it fails to connect. Like a error
//! page." With Tailscale off on the phone, the PC asleep, or Nirvana closed, no byte comes from the PC, so the error
//! screen has to come from the phone: the service worker (`page/sw.js`) keeps the shell, only the static files, in
//! a cache named for this build, and serves it when the PC does not answer.
//!
//! The build tag is the Nirvana version and a hash of every shell file. It is written into `sw.js` (the cache's name)
//! and into the page (`<meta name="nirvana-build">`), and `/api/hello` answers with it (`BUILD_HEADER`). So a new
//! build changes `sw.js`'s bytes, the browser installs the new worker, and that worker drops every older cache; and a
//! page that came out of an older cache reloads the first time the PC answers with another tag.

use std::sync::OnceLock;

use super::{alerts, routes};

/// The header `/api/hello` carries the build tag in; the page compares it with its own.
pub const BUILD_HEADER: &str = "Nirvana-Build";

/// What `sw.js` and `index.html` hold in place of the tag and the list, so each still reads as valid as written.
const BUILD_MARK: &str = "NIRVANA_BUILD";
const SHELL_MARK: &str = "['NIRVANA_SHELL']";

/// The P39d screens' scripts, which `pick.rs` serves from its own match.
const PICK_SCRIPTS: &[(&str, &[u8])] = &[
    ("/newspace.js", include_bytes!("page/newspace.js")),
    ("/cards.js", include_bytes!("page/cards.js")),
];

/// Every file the shell is made of, as written in the binary: the page's assets, the P39d scripts, and the
/// home-screen app's files. Not `/sw.js` (a worker never caches its own script) and nothing under `/api/` (agent
/// data is never kept on the phone).
pub fn files() -> Vec<(&'static str, &'static [u8])> {
    let mut all: Vec<(&'static str, &'static [u8])> = routes::ASSETS
        .iter()
        .map(|(path, _, body)| (*path, body.as_bytes()))
        .collect();
    all.extend_from_slice(PICK_SCRIPTS);
    all.extend(
        alerts::FILES
            .iter()
            .filter(|(path, _, _)| *path != "/sw.js")
            .map(|(path, _, body)| (*path, *body)),
    );
    all
}

/// The paths the worker caches, in `files()`'s order.
pub fn paths() -> Vec<&'static str> {
    files().into_iter().map(|(path, _)| path).collect()
}

/// `<version>-<16 hex digits>`: FNV-1a 64 over each file's path and bytes, the worker's own script included, so a
/// change to any of them names a new cache even between two releases of the same version.
pub fn build_tag(version: &str, files: &[(&str, &[u8])]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, body) in files {
        for byte in path
            .bytes()
            .chain([0])
            .chain(body.iter().copied())
            .chain([0])
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{version}-{hash:016x}")
}

fn worker_template() -> &'static [u8] {
    alerts::FILES
        .iter()
        .find(|(path, _, _)| *path == "/sw.js")
        .map_or(&[][..], |(_, _, body)| *body)
}

/// This build's tag.
pub fn build() -> &'static str {
    static TAG: OnceLock<String> = OnceLock::new();
    TAG.get_or_init(|| {
        let mut all = files();
        all.push(("/sw.js", worker_template()));
        build_tag(crate::nirvana::VERSION, &all)
    })
}

/// `sw.js` as served: the build tag and the shell's paths written in.
pub fn worker() -> &'static str {
    static WORKER: OnceLock<String> = OnceLock::new();
    WORKER.get_or_init(|| {
        let list = serde_json::to_string(&paths()).unwrap_or_else(|_| "[]".into());
        String::from_utf8_lossy(worker_template())
            .replacen(&format!("'{BUILD_MARK}'"), &format!("'{}'", build()), 1)
            .replacen(SHELL_MARK, &list, 1)
    })
}

/// `index.html` as served: the build tag written into its meta line.
pub fn index() -> &'static str {
    static INDEX: OnceLock<String> = OnceLock::new();
    INDEX.get_or_init(|| {
        let template = routes::ASSETS
            .iter()
            .find(|(path, _, _)| *path == "/")
            .map_or("", |(_, _, body)| *body);
        template.replacen(BUILD_MARK, build(), 1)
    })
}

/// A shell file's body as the PC serves it: `/` with its tag written in, any other as written.
pub fn served(path: &str, body: &'static str) -> &'static str {
    if path == "/" {
        index()
    } else {
        body
    }
}
