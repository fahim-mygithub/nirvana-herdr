# Plan 47 P47d: the sources check counts the ways an agy worker really reads a URL (a fetch's `<status> <url>` lines,
# a quote-aware segment cut, Claude's PowerShell tool, WebFetch's url only) and the worker texts ask for that shape.
# Controls starting with p47d_. One sabotage per assertion, each on a product line; the test named in the fifth field
# is the one expected to print its red line with the sabotage in place, and the comment above each says which of its
# assertions goes red and why (written before the run). `Control` is provided by controls.py.
GATE = "src/nirvana_gate.rs"
AGY = "src/nirvana_agy_tools.rs"
PROFILES = "src/nirvana_profiles.rs"

T_LOOP = "p47d_a_cut_loop_of_five_status_lines_passes_and_a_404_fails_alone"
T_ECHO = "p47d_an_echoed_or_printed_status_line_counts_nothing"
T_EARLIER = "p47d_a_fetch_in_an_earlier_step_does_not_count_a_later_echo"
T_ENDED = "p47d_a_status_line_counts_only_from_a_step_that_ended_well"
T_CODES = "p47d_only_a_2xx_or_3xx_alone_with_its_url_counts"
T_MATCH = "p47d_a_status_url_matches_as_any_cited_url_matches"
T_CUT = "p47d_a_cut_command_counts_by_the_words_it_shows"
T_SCRIPT = "p47d_a_script_run_by_name_counts_nothing"
T_CLAUDE = "p47d_a_claude_status_line_counts_nothing"
T_QUOTE = "p47d_a_quote_holds_its_separators"
T_PS = "p47d_the_powershell_tool_counts_as_bash_does"
T_WF = "p47d_webfetch_counts_only_its_url"
T_TEXT = "p47d_the_check_the_workers_are_told_to_run_counts"

STATUS_CALL = "        if status_read(url, &reads) {\n            continue;\n        }\n"
FETCHES = "                let fetches = command_fetches(command);\n"
RECORD = "                    reads.status.extend(status_urls(output));\n"
QUOTE_ARM = "            b'\"' | b'\\'' if !opens_code(&cmd[start..at]) => {\n"
ESCAPE_ARM = "            b'\\\\' | b'`' if quote == b'\"' => {\n"
CLAUDE_RESULT = "                        if !id.is_empty() && fetch_ids.iter().any(|f| f == id) {\n"

CONTROLS = (
    # ---- item 1: a status line counts, under its conditions ----
    # The rule unused: the five-page loop's pages 5 (past the cut) reads not opened, so `all` fails.
    Control("p47d_status_unused", GATE, STATUS_CALL, "", T_LOOP, "rust"),
    # Nothing recorded: the loop SOURCES_LINE names prints two status lines and neither counts.
    Control("p47d_status_not_recorded", GATE, RECORD, "", T_TEXT, "rust"),
    # No fetch word needed: `echo "200 <url>"` counts, the first printed case.
    Control("p47d_status_no_fetch_needed", GATE,
            "                if fetches\n                    && answered\n", "                if answered\n", T_ECHO, "rust"),
    # The fetch word from any earlier step: step 2's echo counts after step 1's curl, so the axis passes.
    Control("p47d_status_fetch_any_step", GATE, FETCHES,
            "                let fetches = command_fetches(command) || !reads.texts.is_empty();\n", T_EARLIER, "rust"),
    # A non-zero exit counts: `Command exited with code 6.` then the line passes.
    Control("p47d_status_failed_counts", GATE,
            "                    && answered\n                    && agy_command_ok(output)\n",
            "                    && answered\n", T_ENDED, "rust"),
    # An unanswered or failed step counts: the `ERROR` step's line passes (its output has no exit line).
    Control("p47d_status_unanswered_counts", GATE,
            "                if fetches\n                    && answered\n", "                if fetches\n", T_ENDED, "rust"),
    # A `file://` fetch counts: curl reading status.txt as a file passes, the last printed case.
    Control("p47d_status_file_url_counts", GATE,
            "                    && !command.to_ascii_lowercase().contains(\"file://\")\n", "", T_ECHO, "rust"),
    # Any code: `404 <url>` passes, the first of the refused lines.
    Control("p47d_status_any_code", GATE,
            "                && matches!(code.as_bytes()[0], b'2' | b'3');\n", "                && true;\n",
            T_CODES, "rust"),
    # Other words on the line: `200 <url> OK` passes (404 to 2000 before it still fail).
    Control("p47d_status_extra_words", GATE, "                && words.next().is_none()\n", "", T_CODES, "rust"),
    # Any length of code: `2000 <url>` passes.
    Control("p47d_status_code_len", GATE,
            "            let status = code.len() == 3\n", "            let status = true\n", T_CODES, "rust"),
    # No scheme needed: `200 example.com/a` counts for a cite with no scheme, the last case.
    Control("p47d_status_no_scheme", GATE,
            "                && (url.starts_with(\"https://\") || url.starts_with(\"http://\")))\n", ")\n",
            T_CODES, "rust"),
    # Stored as printed: `200 https://www.example.com/a/` no longer matches the cite `https://example.com/a`.
    Control("p47d_status_unnormalized", GATE,
            "            .then(|| normalize_url(url))\n", "            .then(|| url.to_owned())\n", T_MATCH, "rust"),
    # A cut command counts whatever it shows: the URL list whose curl lies past the cut passes.
    Control("p47d_status_cut_any", GATE, FETCHES,
            "                let fetches = command_fetches(command) || command.trim_end().ends_with(CUT_MARK);\n",
            T_CUT, "rust"),
    # A script run by name counts: `python.exe run_check.py`'s line passes.
    Control("p47d_status_script_by_name", GATE, FETCHES,
            "                let fetches = command_fetches(command) || command.contains(\"python\");\n", T_SCRIPT, "rust"),
    # Claude gains the rule: the Bash result's status line passes.
    Control("p47d_status_claude", GATE, CLAUDE_RESULT,
            "                        reads.status.extend(status_urls(&json_text(block.get(\"content\").unwrap_or(&Value::Null))));\n"
            + CLAUDE_RESULT, T_CLAUDE, "rust"),
    # ---- item 3: the segment cut respects quotes ----
    # Quote-blind again: the bladebro eval's `||` cuts its `--url` away from `bladebro`, the first case.
    Control("p47d_quote_blind", GATE, QUOTE_ARM, "            b'\"' | b'\\'' if false => {\n", T_QUOTE, "rust"),
    # A shell's quoted code read as a string: the `-Command "...; echo <m>; curl <n>"` joins m to the curl.
    Control("p47d_quote_code_not_cut", GATE, QUOTE_ARM, "            b'\"' | b'\\'' => {\n", T_QUOTE, "rust"),
    # An unclosed quote runs to the end: `echo it's here; echo <i>; ... curl <j>` is one segment, i passes.
    Control("p47d_quote_unclosed_swallows", GATE,
            "            _ => end += 1,\n        }\n    }\n    None\n}\n",
            "            _ => end += 1,\n        }\n    }\n    Some(bytes.len() - 1)\n}\n", T_QUOTE, "rust"),
    # A quote spans lines: `echo "notes\n<k>; curl <l>"` is one segment, k passes.
    Control("p47d_quote_spans_lines", GATE, "            b'\\n' => return None,\n", "", T_QUOTE, "rust"),
    # No backslash escape: `"X-Note: \"a; b\""` closes at `\"`, its `;` cuts, the c case fails.
    Control("p47d_quote_no_backslash", GATE, ESCAPE_ARM, "            b'`' if quote == b'\"' => {\n", T_QUOTE, "rust"),
    # No backtick escape: the PowerShell d case fails the same way.
    Control("p47d_quote_no_backtick", GATE, ESCAPE_ARM, "            b'\\\\' if quote == b'\"' => {\n", T_QUOTE, "rust"),
    # ---- items 4 and 5: Claude's PowerShell tool, WebFetch's url ----
    # Bash only: PowerShell's `curl.exe -s <a>` fails.
    Control("p47d_powershell_unread", GATE,
            "                    } else if matches!(name, \"Bash\" | \"PowerShell\") {\n",
            "                    } else if name == \"Bash\" {\n", T_PS, "rust"),
    # The prompt read: WebFetch's prompt URL b passes.
    Control("p47d_webfetch_prompt_counts", GATE,
            "                            if name == \"WebFetch\" {\n", "                            if false {\n", T_WF, "rust"),
    # ---- item 6: the worker texts ----
    # SOURCES_LINE no longer asks for the status line.
    Control("p47d_text_sources_no_status", AGY,
            "Check pages with a command that prints one `<status> <url>` line per URL and names its fetch",
            "Check pages with a command that names its fetch", T_TEXT, "rust"),
    # SOURCES_LINE's loop is one the gate cannot count: its fetch is a variable, so the final axis fails.
    Control("p47d_text_sources_loop_uncounted", AGY,
            "`Get-Content urls.txt | % { curl.exe -s", "`Get-Content urls.txt | % { & $fetch -s", T_TEXT, "rust"),
    # The 12-of-12 sentence dropped: SOURCES_LINE no longer starts with it.
    Control("p47d_text_sources_sentence", AGY,
            "\"Never cite a URL you did not open in this session. Check pages", "\"Check pages", T_TEXT, "rust"),
    # WEB_RESEARCHER rule 3 no longer says a loop is fine when each pass prints the line.
    Control("p47d_text_researcher_loop", PROFILES,
            "is fine when each pass prints that line. On agy", "is fine. On agy", T_TEXT, "rust"),
    # The card no longer says it.
    Control("p47d_text_card_loop", PROFILES,
            "URLs is fine when each pass prints that line.\\n\",\n", "URLs is fine.\\n\",\n", T_TEXT, "rust"),
)
