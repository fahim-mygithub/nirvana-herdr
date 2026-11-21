# Plan 47 P47d: the sources check counts the ways an agy worker really reads a URL (a fetch's `<status> <url>` lines,
# a quote-aware segment cut, Claude's PowerShell tool, WebFetch's url only) and the worker texts ask for that shape.
# The owner's ruling (2026-11-20) adds the cut-command path: guarded, and named on the axis as a note.
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
T_CUT = "p47d_a_cut_command_with_a_fetch_in_sight_passes_and_one_without_is_a_note"
T_NOTE = "p47d_a_cut_command_with_no_fetch_in_sight_counts_with_a_note"
T_GUARD = "p47d_a_cut_command_that_prints_in_sight_or_ended_badly_counts_nothing"
T_VIDEO = "p47d_a_missing_video_keeps_the_cut_note"
T_SCRIPT = "p47d_a_script_run_by_name_counts_nothing"
T_CLAUDE = "p47d_a_claude_status_line_counts_nothing"
T_QUOTE = "p47d_a_quote_holds_its_separators"
T_PS = "p47d_the_powershell_tool_counts_as_bash_does"
T_WF = "p47d_webfetch_counts_only_its_url"
T_TEXT = "p47d_the_check_the_workers_are_told_to_run_counts"

T_EXIT = "p47d_fix_the_exit_line_is_read_below_agy_s_header"
T_CODE = "p47d_fix_a_shell_s_code_quote_closes_and_e_is_code_only_for_an_interpreter"
T_SHELL = "p47d_fix_a_backslash_escapes_a_quote_only_in_bash"

T_PROSE = "p47d_fix2_prose_apostrophes_hide_the_curl_between_them"
T_DQ = "p47d_fix2_double_quotes_keep_an_apostrophe_text"
T_GLUED_FLAG = "p47d_fix2_a_quote_glued_to_its_flag_holds_its_separator"
T_GLUED_CODE = "p47d_fix2_python_takes_its_code_glued_to_c"
T_SUFFIX = "p47d_fix2_a_reader_named_with_exe_or_com_refuses_a_cut_step"
T_REPLAY = "p47d_fix2_the_replay_measures_or_says_why_not"
T_FAILED_RUN = "p47d_fix2_a_command_that_failed_to_run_is_a_failed_command"
TESTS = "src/nirvana_gate/tests.rs"

STATUS_CALL = "        if status_read(url, &reads) {\n            continue;\n        }\n"
# The fix round (S3): agy's command is read for PowerShell.
FETCHES = "                let fetches = command_fetches(command, Shell::PowerShell);\n"
RECORD = "                    reads.status.extend(status_urls(output));\n"
# The fix round (S2): the code quote has arms of its own before the quote arm. The second fix round (F1): the quote
# arm reads every apostrophe as a quote again (the prose rule is gone).
QUOTE_ARM = "            b'\"' | b'\\'' => {\n"
CODE_OPEN = ("            byte @ (b'\"' | b'\\'') if code.is_none() && opens_code(&cmd[start..at]) => "
             "code = Some(byte),\n")
CODE_CLOSE = "            byte @ (b'\"' | b'\\'') if code == Some(byte) => code = None,\n"
CODE_ESCAPE = "            byte if code == Some(b'\"') && shell.escapes(byte) => at += 1,\n"
# The fix round (S3): which byte escapes is the shell's.
POSIX_ESCAPE = "            Shell::Posix => byte == b'\\\\',\n"
PS_ESCAPE = "            Shell::PowerShell => byte == b'`',\n"
EXIT_LINE = '        .find_map(|line| line.strip_prefix("Command exited with code "))\n'
FIRST_LINE_ONLY = '        .next()\n        .and_then(|line| line.strip_prefix("Command exited with code "))\n'
EXIT_HEADER = '    let header = output.split_once("Command output:").map_or(output, |(header, _)| header);\n'
READERS = "const LOCAL_READERS: &[&str] = &[\n"
# The fix round (item 7): what WEB_RESEARCHER and the card say after their curl example.
DEV_NULL = "<url>` (off Windows, \\\n`-o /dev/null`)"
LOOP_FINE = "; a loop over many URLs is fine when each pass prints that line."
CLAUDE_RESULT = "                        if !id.is_empty() && fetch_ids.iter().any(|f| f == id) {\n"
FETCH_ARM = "                if fetches && ended_well {\n"
CUT_ARM = "                } else if ended_well && agy_cut(command) && !prints_visibly(command) {\n"
CUT_TEST = "    command.ends_with(CUT_MARK) && command.chars().count() == AGY_CUT_CHARS + 1\n"
PRINTERS = "const PRINTING_COMMANDS: &[&str] = &[\n    \"echo\",\n    \"printf\",\n    \"write-output\",\n"
NOTE_MARK = ("                \"sources\",\n                Mark::Note,\n                format!(\n"
             "                    \"{} source(s) cited, each verified in stream{unchecked}; {note}\",\n")

CONTROLS = (
    # ---- item 1: a status line counts, under its conditions ----
    # The rule unused: the five-page loop's pages 5 (past the cut) reads not opened, so `all` fails.
    Control("p47d_status_unused", GATE, STATUS_CALL, "", T_LOOP, "rust"),
    # Nothing recorded: the loop SOURCES_LINE names prints two status lines and neither counts.
    Control("p47d_status_not_recorded", GATE, RECORD, "", T_TEXT, "rust"),
    # No fetch word needed: `echo "200 <url>"` (whole, not cut) counts, the first printed case.
    Control("p47d_status_no_fetch_needed", GATE, FETCH_ARM, "                if ended_well {\n", T_ECHO, "rust"),
    # The fetch word from any earlier step: step 2's echo counts after step 1's curl, so the axis passes.
    Control("p47d_status_fetch_any_step", GATE, FETCHES,
            "                let fetches = command_fetches(command, Shell::PowerShell) || !reads.texts.is_empty();\n",
            T_EARLIER, "rust"),
    # A non-zero exit counts: `Command exited with code 6.` then the line passes.
    Control("p47d_status_failed_counts", GATE,
            "                let ended_well = answered\n                    && agy_command_ok(output)\n",
            "                let ended_well = answered\n", T_ENDED, "rust"),
    # An unanswered or failed step counts: the `ERROR` step's line passes (its output has no exit line).
    Control("p47d_status_unanswered_counts", GATE,
            "                let ended_well = answered\n", "                let ended_well = true\n", T_ENDED, "rust"),
    # A `file://` fetch counts: curl reading status.txt as a file passes, the last printed case.
    Control("p47d_status_file_url_counts", GATE,
            "                    && !command.to_ascii_lowercase().contains(\"file://\");\n", "                    ;\n",
            T_ECHO, "rust"),
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
    # A cut command read as a fetch: the URL list whose curl lies past the cut passes plainly, not as a note.
    Control("p47d_status_cut_any", GATE, FETCHES,
            "                let fetches = command_fetches(command, Shell::PowerShell)\n"
            "                    || command.trim_end().ends_with(CUT_MARK);\n",
            T_CUT, "rust"),
    # A script run by name counts: `python.exe run_check.py`'s line passes.
    Control("p47d_status_script_by_name", GATE, FETCHES,
            "                let fetches = command_fetches(command, Shell::PowerShell)"
            " || command.contains(\"python\");\n", T_SCRIPT, "rust"),
    # Claude gains the rule: the Bash result's status line passes.
    Control("p47d_status_claude", GATE, CLAUDE_RESULT,
            "                        reads.status.extend(status_urls(&json_text(block.get(\"content\").unwrap_or(&Value::Null))));\n"
            + CLAUDE_RESULT, T_CLAUDE, "rust"),
    # ---- the owner's ruling: a cut command with no fetch in sight counts, guarded and named ----
    # The path unused: calib-research-s1's list by shape fails instead of a note (the first assertion).
    Control("p47d_cut_path_unused", GATE, CUT_ARM, "                } else if false {\n", T_NOTE, "rust"),
    # Guard (a) gone: `echo "200 <url>"` before the padding counts, the first printing case.
    Control("p47d_cut_prints_ignored", GATE, CUT_ARM,
            "                } else if ended_well && agy_cut(command) {\n", T_GUARD, "rust"),
    # Guard (a) without echo: the echo case counts.
    Control("p47d_cut_no_echo", GATE, PRINTERS,
            "const PRINTING_COMMANDS: &[&str] = &[\n    \"printf\",\n    \"write-output\",\n", T_GUARD, "rust"),
    # Guard (a) without Write-Output: the third printing case counts (echo and printf before it still refused).
    Control("p47d_cut_no_write_output", GATE, PRINTERS,
            "const PRINTING_COMMANDS: &[&str] = &[\n    \"echo\",\n    \"printf\",\n", T_GUARD, "rust"),
    # Guard (a) without the script calls: `python -c "print('200 <url>')"`, the last printing case, counts.
    Control("p47d_cut_no_print_call", GATE,
            'const PRINTING_CALLS: &[&str] = &["print(", "console.log(", "[console]::write"];',
            "const PRINTING_CALLS: &[&str] = &[];", T_GUARD, "rust"),
    # Guard (b) gone on this path: `Command exited with code 1.` then the line counts.
    Control("p47d_cut_failed_counts", GATE, CUT_ARM,
            "                } else if agy_cut(command) && !prints_visibly(command) {\n", T_GUARD, "rust"),
    # The cut read from the mark alone: the 760-character command ending in `…` counts.
    Control("p47d_cut_any_length", GATE, CUT_TEST, "    command.ends_with(CUT_MARK)\n", T_GUARD, "rust"),
    # The cut read from the length alone: the 513-character command with no mark counts.
    Control("p47d_cut_no_mark", GATE, CUT_TEST, "    command.chars().count() == AGY_CUT_CHARS + 1\n", T_GUARD, "rust"),
    # No note: the one-URL axis is a plain pass, not a note.
    Control("p47d_cut_note_silent", GATE, NOTE_MARK, NOTE_MARK.replace("Mark::Note", "Mark::Pass"), T_NOTE, "rust"),
    # The fail's line does not name the cut path: `failed.why` ends at "browsed".
    Control("p47d_cut_note_fail_unnamed", GATE, "        unchecked.push_str(&format!(\"; {note}\"));\n", "",
            T_NOTE, "rust"),
    # Steps named once each: without the dedup the three URLs read "steps 14, 14, 132".
    Control("p47d_cut_note_steps_repeated", GATE, "    named.dedup();\n", "", T_NOTE, "rust"),
    # The path counts a URL another rule already counts: the re-fetched URL reads as a note, not a pass.
    Control("p47d_cut_note_counts_other_rules", GATE,
            "        if let Some(step) = cut_status_step(url, &reads).filter(|_| !url_in_stream(&norm, stream)) {\n",
            "        if let Some(step) = cut_status_step(url, &reads) {\n", T_NOTE, "rust"),
    # A missing video replaces the note's text instead of adding to it.
    Control("p47d_cut_note_lost_to_video", GATE,
            "        axis.why = if matches!(axis.mark, Mark::Fail | Mark::Note) {\n",
            "        axis.why = if axis.mark == Mark::Fail {\n", T_VIDEO, "rust"),
    # ---- item 3: the segment cut respects quotes ----
    # Quote-blind again: the bladebro eval's `||` cuts its `--url` away from `bladebro`, the first case.
    Control("p47d_quote_blind", GATE, QUOTE_ARM, "            b'\"' | b'\\'' if false => {\n", T_QUOTE, "rust"),
    # A shell's quoted code read as a string: the `-Command "...; echo <m>; curl <n>"` joins m to the curl. (The fix
    # round moved the code quote to an arm of its own; this drops that arm.)
    Control("p47d_quote_code_not_cut", GATE, CODE_OPEN, "", T_QUOTE, "rust"),
    # An unclosed quote runs to the end: `echo it's here; echo <i>; ... curl <j>` is one segment, i passes.
    Control("p47d_quote_unclosed_swallows", GATE,
            "            _ => end += 1,\n        }\n    }\n    None\n}\n",
            "            _ => end += 1,\n        }\n    }\n    Some(bytes.len() - 1)\n}\n", T_QUOTE, "rust"),
    # A quote spans lines: `echo "notes\n<k>; curl <l>"` is one segment, k passes.
    Control("p47d_quote_spans_lines", GATE, "            b'\\n' => return None,\n", "", T_QUOTE, "rust"),
    # No backslash escape in Bash: `"X-Note: \"a; b\""` closes at `\"`, its `;` cuts, the c case (a Claude `Bash`
    # call since the fix round) fails.
    Control("p47d_quote_no_backslash", GATE, POSIX_ESCAPE, "            Shell::Posix => false,\n", T_QUOTE, "rust"),
    # No backtick escape on agy: the PowerShell d case fails the same way.
    Control("p47d_quote_no_backtick", GATE, PS_ESCAPE, "            Shell::PowerShell => false,\n", T_QUOTE, "rust"),
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
    # ---- the fix round (the review's S1 to S5) ----
    # S1, the exit line read on the first line only: the uncut fetch whose header holds `Command exited with code 6.`
    # below `Created At:`/`Completed At:` counts, so the `headed` case passes (the prefix-form case before it fails).
    Control("p47d_fix_exit_first_line", GATE, EXIT_LINE, FIRST_LINE_ONLY, T_ENDED, "rust"),
    # The same, on the cut path: the padded `Set-Location` step whose header says exit 1 counts with a note.
    Control("p47d_fix_exit_first_line_cut", GATE, EXIT_LINE, FIRST_LINE_ONLY, T_GUARD, "rust"),
    # S1, the command's own text read as agy's: a header with no exit line, then `Command exited with code 1.` printed
    # by the command, reads as a failure (the last but two rows of T_EXIT).
    Control("p47d_fix_exit_below_marker", GATE, EXIT_HEADER, "    let header = output;\n", T_EXIT, "rust"),
    # S2, a code quote that never closes: `python -c "print(1)"; yt-dlp ...`'s closing quote swallows the `;` again.
    Control("p47d_fix_code_quote_unclosed", GATE, CODE_CLOSE, "", T_CODE, "rust"),
    # S2, no escape inside a code quote: `-Command "echo `"a`"; yt-dlp ..."` closes at the first escaped quote, and
    # the second opens a string that hides the yt-dlp (the sixth command fails).
    Control("p47d_fix_code_quote_no_escape", GATE, CODE_ESCAPE, "", T_CODE, "rust"),
    # S2, `-e` code for any command word: `grep -e "curl"`'s status line counts (the second pattern case).
    Control("p47d_fix_e_any_command", GATE,
            '            CODE_E_COMMANDS.contains(&program.strip_suffix(".exe").unwrap_or(&program))\n',
            "            !program.is_empty()\n", T_CODE, "rust"),
    # S2, `-e` in any case: `node -E "curl"` reads as code, its status line counts (the last pattern case).
    Control("p47d_fix_e_any_case", GATE, '    if flag == "-e" {\n', '    if flag.eq_ignore_ascii_case("-e") {\n',
            T_CODE, "rust"),
    # (The second fix round removed p47d_fix_prose_quote and p47d_fix_prose_prefix with the prose rule they pinned.)
    # S3, a backslash escapes on agy: `cd "C:\dir\"; curl ...` is one segment again and its status line fails.
    Control("p47d_fix_agy_backslash", GATE, PS_ESCAPE,
            "            Shell::PowerShell => matches!(byte, b'\\\\' | b'`'),\n", T_SHELL, "rust"),
    # S3, Claude's PowerShell tool read as Bash: its `echo "C:\dir\"; yt-dlp ..."` hides the yt-dlp, the id fails.
    Control("p47d_fix_claude_powershell_as_bash", GATE, '        if name == "PowerShell" {\n', "        if false {\n",
            T_SHELL, "rust"),
    # S3, a segment's code read for either shell: `if (Test-Path "C:\d\") { curl.exe ... }` hides its curl in a
    # string, the last case fails.
    Control("p47d_fix_code_either_shell", GATE,
            "        match quote_end(&bytes, at, shell).map(|end| end - at - 1) {\n",
            "        match quote_end(&bytes, at, Shell::Either).map(|end| end - at - 1) {\n", T_SHELL, "rust"),
    # S4, the local readers not refused on the cut path: `Get-Content status.txt` in sight counts with a note (the
    # first reader among T_GUARD's cases, a reader since the fix round; every reader shares this line).
    Control("p47d_fix_cut_reader_allowed", GATE, "            || segment_reads_local(segment)\n", "", T_GUARD, "rust"),
    # S5, one per word. Each case of T_GUARD named below counts with a note when its word is gone.
    # `Write-Host 200 <url>`.
    Control("p47d_fix_guard_no_write_host", GATE, '    "write-host",\n    "write-information",\n',
            '    "write-information",\n', T_GUARD, "rust"),
    # `Write-Information "200 <url>"`.
    Control("p47d_fix_guard_no_write_information", GATE, '    "write-information",\n    "out-host",\n',
            '    "out-host",\n', T_GUARD, "rust"),
    # `$lines | Out-Host`.
    Control("p47d_fix_guard_no_out_host", GATE, '    "out-host",\n];\n\n/// Script calls that print.\n',
            "];\n\n/// Script calls that print.\n", T_GUARD, "rust"),
    # `cat status.txt` (a local reader since the fix round).
    Control("p47d_fix_guard_no_cat", GATE, READERS + '    "cat",\n', READERS, T_GUARD, "rust"),
    # `type status.txt`.
    Control("p47d_fix_guard_no_type", GATE, '    "type",\n    "get-content",\n    "gc",\n    "head",\n',
            '    "get-content",\n    "gc",\n    "head",\n', T_GUARD, "rust"),
    # `Get-Content status.txt`.
    Control("p47d_fix_guard_no_get_content", GATE, '    "get-content",\n    "gc",\n    "head",\n',
            '    "gc",\n    "head",\n', T_GUARD, "rust"),
    # `gc status.txt`.
    Control("p47d_fix_guard_no_gc", GATE, '    "gc",\n    "head",\n', '    "head",\n', T_GUARD, "rust"),
    # `more status.txt`.
    Control("p47d_fix_guard_no_more", GATE, '    "tail",\n    "more",\n', '    "tail",\n', T_GUARD, "rust"),
    # Item 7: WEB_RESEARCHER rule 3 names no `-o /dev/null` for other systems.
    Control("p47d_fix_text_researcher_dev_null", PROFILES, DEV_NULL + LOOP_FINE + " On", "<url>`" + LOOP_FINE + " On",
            T_TEXT, "rust"),
    # The card names none either.
    Control("p47d_fix_text_card_dev_null", PROFILES, DEV_NULL + LOOP_FINE + "\\n", "<url>`" + LOOP_FINE + "\\n",
            T_TEXT, "rust"),
    # SOURCES_LINE, which goes only to agy here, names it anyway.
    Control("p47d_fix_text_sources_dev_null", AGY, "$_ }` does; a loop is fine",
            "$_ }` (off Windows, `-o /dev/null`) does; a loop is fine", T_TEXT, "rust"),
    # SOURCES_LINE's loop follows redirects: its `url_effective` would be the final URL.
    Control("p47d_fix_text_sources_redirects", AGY, "curl.exe -s -o NUL", "curl.exe -s -L -o NUL", T_TEXT, "rust"),
    # ---- the second fix round (the second review's F1 to F7) ----
    # F1, an apostrophe opens nothing: `echo don't; curl -s -K urls.txt ...; echo it's` cuts at its `;`, the curl is a
    # segment of its own, and its status line counts (Pass where the test wants Fail).
    Control("p47d_fix2_prose_apostrophe_opens_nothing", GATE, QUOTE_ARM, "            b'\"' => {\n", T_PROSE, "rust"),
    # A double quote opens nothing: `"don't"`'s apostrophe opens a string that runs to `"it'`, which hides the curl
    # (Fail where the test wants Pass).
    Control("p47d_fix2_double_quote_opens_nothing", GATE, QUOTE_ARM, "            b'\\'' => {\n", T_DQ, "rust"),
    # F2, an apostrophe opens nothing: `-H'Accept: text/html; q=1'` cuts at its `;` and the URL lands in a segment with
    # no fetch word (Fail where the test wants Pass).
    Control("p47d_fix2_glued_header_cut", GATE, QUOTE_ARM, "            b'\"' => {\n", T_GLUED_FLAG, "rust"),
    # The glued code flag read as before this round: `python -c'...'`'s quote is a string, the urlopen in it is text,
    # the command fetches nothing, and its status line fails (the first assertion).
    Control("p47d_fix2_glued_code_unread", GATE, "fn opens_code(head: &str) -> bool {\n",
            "fn opens_code(head: &str) -> bool {\n    if !head.ends_with(char::is_whitespace) {\n"
            "        return false;\n    }\n", T_GLUED_CODE, "rust"),
    # F4, the local readers' follow check without the suffix: `cat.exe status.txt`, the first case, counts with a note.
    Control("p47d_fix2_reader_suffix_unread", GATE, "            past_suffix(&code[at + word.len()..])\n",
            "            code[at + word.len()..]\n", T_SUFFIX, "rust"),
    # F4, `.exe` only: `more.com status.txt`, the fourth case, counts with a note (the three `.exe` cases still fail).
    Control("p47d_fix2_com_unread", GATE,
            '    after.strip_prefix(".exe").or_else(|| after.strip_prefix(".com")).unwrap_or(after)\n',
            '    after.strip_prefix(".exe").unwrap_or(after)\n', T_SUFFIX, "rust"),
    # F5, a list with no orders measures nothing and passes: `empty.json` (`[]`) does not panic.
    Control("p47d_fix2_replay_empty_passes", TESTS, "        .filter(|rows| !rows.is_empty())\n", "", T_REPLAY,
            "rust"),
    # F6, `Command failed to run` read as before: that output alone, the first case, reads as ended well.
    Control("p47d_fix2_failed_to_run_ok", GATE,
            '    if header.lines().any(|line| line.starts_with("Command failed to run")) {\n', "    if false {\n",
            T_FAILED_RUN, "rust"),
    # F7, a backslash escapes on agy: the agy form of `curl -H "X-Note: \"a; b\"" <url>` counts its URL (Pass where
    # the test wants Fail; the Bash case before it still passes).
    Control("p47d_fix2_agy_backslash_counts", GATE, PS_ESCAPE,
            "            Shell::PowerShell => matches!(byte, b'\\\\' | b'`'),\n", T_QUOTE, "rust"),
)
