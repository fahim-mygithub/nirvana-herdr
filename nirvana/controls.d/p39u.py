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
            "  const BODY_RATE = 65536;\n",
            "  const BODY_RATE = 16384;\n",
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
)
