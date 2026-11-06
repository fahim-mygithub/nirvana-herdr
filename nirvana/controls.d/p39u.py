# Plan 39 P39u: phone uploads that never landed, and a failed one that said nothing (package `p39u`,
# nirvana/plans/2026-11-04-39-records/p39u.md). Controls starting with p39u_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
SEND = "src/nirvana_remote/send.rs"
LIVE = "src/nirvana_remote/live.rs"
HTTP = "src/nirvana_remote/http.rs"
APP_JS = "src/nirvana_remote/page/app.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"

_T = "nirvana_remote::tests_upload::"
T_RACE = _T + "uploads_at_once_into_a_new_space_all_get_the_folder"
T_TWO = _T + "two_photos_at_once_into_a_new_space_both_land"
T_LOG = _T + "a_failed_upload_logs_one_line_with_its_size_bytes_time_and_why"
T_WAIT = _T + "the_page_gives_up_on_an_upload_just_after_the_servers_own_deadline"
T_CHIP = _T + "a_failed_chip_says_why_and_retries_and_send_asks_to_go_without_it"
T_CEILING = _T + "an_upload_is_cut_at_the_ceiling_whatever_its_rate"
T_BUSY = _T + "a_fifth_upload_at_once_is_told_the_pc_is_busy"
T_DRAIN = _T + "a_slow_caller_that_reads_after_sending_still_gets_the_400"

CONTROLS = (
    # The cause: the folder made by check-then-create, as 0.58.1 did; two uploads at once into a new space race
    Control("p39u_dir_race", SEND,
            "        match std::fs::create_dir(dir) {\n"
            "            Ok(()) => {}\n"
            "            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}\n"
            "            Err(err) => return Err(format!(\"{}: {err}\", dir.display())),\n"
            "        }\n",
            "        if !dir.exists() {\n"
            "            std::fs::create_dir(dir).map_err(|err| format!(\"{}: {err}\", dir.display()))?;\n"
            "        }\n",
            T_RACE, "rust"),
    # The same race, end to end through the server: two photos at once
    Control("p39u_dir_race_served", SEND,
            "        match std::fs::create_dir(dir) {\n"
            "            Ok(()) => {}\n"
            "            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}\n"
            "            Err(err) => return Err(format!(\"{}: {err}\", dir.display())),\n"
            "        }\n",
            "        if !dir.exists() {\n"
            "            std::fs::create_dir(dir).map_err(|err| format!(\"{}: {err}\", dir.display()))?;\n"
            "        }\n",
            T_TWO, "rust"),
    # A .gitignore another upload made first is taken, not an error
    Control("p39u_ignore_taken", SEND,
            "        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}\n"
            "        Err(err) => return Err(format!(\"{}: {err}\", ignore.display())),\n",
            "        Err(err) => return Err(format!(\"{}: {err}\", ignore.display())),\n",
            T_RACE, "rust"),
    # A body that is too slow is not a 408, which a browser re-sends by itself without telling the page
    Control("p39u_slow_not_408", HTTP,
            "            Self::Timeout => 400,\n",
            "            Self::Timeout => 408,\n",
            T_LOG, "rust"),
    # Item 2: a failed upload logs its line
    Control("p39u_log_failed", LIVE,
            '                        "nirvana remote upload failed"\n',
            '                        "nirvana remote"\n',
            T_LOG, "rust"),
    # Item 2: the line counts the bytes read
    Control("p39u_log_read", LIVE,
            "        self.read.set(self.read.get() + wrote as u64);\n",
            "",
            T_LOG, "rust"),
    # Item 2: one line per kind of failure a minute (the refusals' throttle)
    Control("p39u_log_throttled", LIVE,
            '                    .allow(&format!("upload {status} {why}"), Instant::now())\n',
            '                    .allow(&format!("upload {status} {why} {:?}", Instant::now()), Instant::now())\n',
            T_LOG, "rust"),
    # Item 2: a saved upload says its size and time
    Control("p39u_log_saved", LIVE,
            '                    "nirvana remote upload saved"\n',
            '                    "nirvana remote"\n',
            T_LOG, "rust"),
    # Item 3: the failure's answer carries how far it got, for the chip
    Control("p39u_answer_read", LIVE,
            '                        "read": read.get(),\n',
            "",
            T_LOG, "rust"),
    # Item 3: the page's copy of the server's rate is the server's
    Control("p39u_page_rate", APP_JS,
            "  const BODY_RATE = 16384;\n",
            "  const BODY_RATE = 65536;\n",
            T_WAIT, "rust"),
    # Item 3: a hung upload times out
    Control("p39u_page_timeout", APP_JS,
            "      xhr.timeout = uploadWait(file.size);\n",
            "",
            T_WAIT, "rust"),
    # Item 3: progress, from XHR's upload events
    Control("p39u_page_progress", APP_JS,
            "      xhr.upload.onprogress = ",
            "      xhr.onprogress = ",
            T_WAIT, "rust"),
    # Item 3: the reason on the chip itself
    Control("p39u_chip_why", CHAT_JS,
            "        if (p.state === 'bad') return p.label + ' · failed: ' + p.why;\n",
            "",
            T_CHIP, "rust"),
    # Item 3: Retry on a failed chip
    Control("p39u_chip_retry", CHAT_JS,
            "          p.state === 'bad' && h('button', { class: 'press chip-retry', type: 'button', text: 'Retry', onclick: () => start(p) }),\n",
            "",
            T_CHIP, "rust"),
    # Item 3: the X stops the upload it removes
    Control("p39u_chip_abort", CHAT_JS,
            "if (p.req) p.req.abort(); ",
            "",
            T_CHIP, "rust"),
    # Item 3: Send with a failed chip asks to go without it
    Control("p39u_send_without", CHAT_JS,
            "        if (failed.length && !skip) { without(failed, () => send(mode, true)); return; }\n",
            "",
            T_CHIP, "rust"),
    # Item 3: how far it got, on the chip
    Control("p39u_chip_how_far", CHAT_JS,
            "        return why + ' (' + size(r.data.read) + ' in ' + r.data.seconds + ' s)';\n",
            "        return why;\n",
            T_CHIP, "rust"),
    # Review round S1: the floor is 128 kbit/s, not P39b's 0.5 Mbit/s
    Control("p39u_body_rate", HTTP,
            "pub const BODY_RATE: u64 = 16 * 1024;\n",
            "pub const BODY_RATE: u64 = 64 * 1024;\n",
            T_CEILING, "rust"),
    # Review round S1: no body takes longer than the ceiling
    Control("p39u_ceiling", HTTP,
            "            .min(self.limits.body_most);\n",
            ";\n",
            T_CEILING, "rust"),
    # Review round S1: the page's copy of the ceiling is the server's
    Control("p39u_page_most", APP_JS,
            "  const BODY_MOST_MS = 1200000;\n",
            "  const BODY_MOST_MS = 600000;\n",
            T_WAIT, "rust"),
    # Review round S1: the page waits no longer than the ceiling (and its margin)
    Control("p39u_page_ceiling", APP_JS,
            "return Math.min(READ_TIMEOUT_MS + Math.ceil(bytes / BODY_RATE) * 1000, BODY_MOST_MS) + UPLOAD_MARGIN_MS;",
            "return READ_TIMEOUT_MS + Math.ceil(bytes / BODY_RATE) * 1000 + UPLOAD_MARGIN_MS;",
            T_WAIT, "rust"),
    # Review round S1: at most UPLOADS_AT_ONCE uploads are read at once; the next is a 503
    Control("p39u_busy", LIVE,
            "                (now < UPLOADS_AT_ONCE).then_some(now + 1)\n",
            "                Some(now + 1)\n",
            T_BUSY, "rust"),
    # Review round S1: a place is given back when its upload answers
    Control("p39u_busy_given_back", LIVE,
            "        self.0.fetch_sub(1, Ordering::SeqCst);\n",
            "",
            T_BUSY, "rust"),
    # Review round S2: after a body cut for being slow, the drain reads on, so a caller still sending reads the 400
    Control("p39u_cut_drain", HTTP,
            "        let drain = self.drain.unwrap_or_else(Drain::after_answer);\n",
            "        let drain = Drain::after_answer();\n",
            T_DRAIN, "rust"),
    # Review round S3: the stale note is cleared where every path goes, the X included
    Control("p39u_note_cleared", CHAT_JS,
            "        if (note.textContent === WAIT && !pending.some((q) => q.state === 'up')) note.textContent = '';\n",
            "",
            T_CHIP, "rust"),
    # Review round S1: the page's copy of UPLOADS_AT_ONCE is the server's
    Control("p39u_page_at_once", CHAT_JS,
            "  const UPLOADS_AT_ONCE = 4;\n",
            "  const UPLOADS_AT_ONCE = 6;\n",
            T_CHIP, "rust"),
    # Review round S1: past the PC's number a chip waits on the page, not a 503
    Control("p39u_page_queue", CHAT_JS,
            "        p.queued = uploading() >= UPLOADS_AT_ONCE;\n",
            "        p.queued = false;\n",
            T_CHIP, "rust"),
    # Review round S1: a waiting chip starts when a place comes free
    Control("p39u_page_next", CHAT_JS,
            "        for (const q of pending) if (q.queued && uploading() < UPLOADS_AT_ONCE) start(q);\n",
            "",
            T_CHIP, "rust"),
    # Review round S1: the X gives its place to a waiting chip
    Control("p39u_x_next", CHAT_JS,
            "drawPending(); next(); } }, nv.icon('close'",
            "drawPending(); } }, nv.icon('close'",
            T_CHIP, "rust"),
)
