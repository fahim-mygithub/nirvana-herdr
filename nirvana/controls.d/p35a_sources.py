# Plan 35 P35a: the sources check sees agy's reads (package `p35a_sources`,
# nirvana/plans/2026-10-31-35-records/p35a.md). Controls starting with p35a_. One sabotage per assertion, each on a
# product line in the gate; the test named must print FAIL with it in place. `Control` is provided by controls.py.
GATE = "src/nirvana_gate.rs"

T_PY = "p35a_python_fetch_words_count_their_segment"
T_PS = "p35a_powershell_fetch_words_count_and_irm_as_a_whole_word"
T_GH_API = "p35a_gh_api_counts_the_repository_and_its_endpoint"
T_GH_VIEW = "p35a_gh_repo_view_counts_the_repository"
T_SEG = "p35a_a_url_counts_only_from_a_segment_that_fetches"
T_P2 = "p35a_the_probe_2_bladebro_stream_passes_example_com"
T_REAL = "p35a_real_sources_send_backs_count_what_the_twin_counts"

PER_SEGMENT = "    for segment in cmd.split(['\\n', ';', '|']).flat_map(|s| s.split(\"&&\")) {\n"
GH_CALL = "        urls.extend(gh_read_urls(segment));\n"


SCRIPT_HEAD = "pub const SCRIPT_FETCH_WORDS: &[&str] = &[\n"


def _drop(word: str) -> str:
    return f'    "{word}",\n'


CONTROLS = (
    # Item 1, Python: drop each new word; each case of T_PY holds only its own word.
    # The review round's FETCH_CALLS and FETCH_COMMANDS hold "urlopen(" and "iwr" too, so these name their const.
    Control("p35a_no_urlopen", GATE, SCRIPT_HEAD + _drop("urlopen("), SCRIPT_HEAD, T_PY, "rust"),
    Control("p35a_no_urllib_request", GATE, _drop("urllib.request"), "", T_PY, "rust"),
    Control("p35a_no_requests_get", GATE, _drop("requests.get"), "", T_PY, "rust"),
    Control("p35a_no_requests_post", GATE, _drop("requests.post"), "", T_PY, "rust"),
    Control("p35a_no_requests_head", GATE, _drop("requests.head"), "", T_PY, "rust"),
    Control("p35a_no_requests_session", GATE, _drop("requests.Session"), "", T_PY, "rust"),
    Control("p35a_no_httpx_get", GATE, _drop("httpx.get"), "", T_PY, "rust"),
    Control("p35a_no_httpx_client", GATE, _drop("httpx.Client"), "", T_PY, "rust"),
    # Item 1, PowerShell: the new word, the whole-word irm, and the two older words this test also holds.
    Control("p35a_no_invoke_restmethod", GATE, _drop("Invoke-RestMethod"), "", T_PS, "rust"),
    Control("p35a_no_irm", GATE,
            'pub const FETCH_TOKENS: &[&str] = &["irm"];', "pub const FETCH_TOKENS: &[&str] = &[];", T_PS, "rust"),
    Control("p35a_no_iwr", GATE, _drop("Invoke-WebRequest") + _drop("iwr"), _drop("Invoke-WebRequest"), T_PS, "rust"),
    Control("p35a_no_invoke_webrequest", GATE, _drop("Invoke-WebRequest"), "", T_PS, "rust"),
    # Items 1 and 4, per segment: irm as a substring ("confirm"), a `$irm` variable, `python` alone, per script.
    Control("p35a_irm_substring", GATE,
            "        || FETCH_TOKENS.iter().any(|word| has_token(&lower, word))\n",
            "        || FETCH_TOKENS.iter().any(|word| lower.contains(word))\n", T_SEG, "rust"),
    Control("p35a_irm_variable", GATE,
            "        !before.is_some_and(|c| joins(c) || c == '$') && !after.is_some_and(joins)\n",
            "        !before.is_some_and(joins) && !after.is_some_and(joins)\n", T_SEG, "rust"),
    Control("p35a_python_is_a_fetch", GATE, SCRIPT_HEAD + _drop("urlopen("),
            SCRIPT_HEAD + _drop("python") + _drop("urlopen("), T_SEG, "rust"),
    Control("p35a_per_script", GATE, PER_SEGMENT, "    for segment in [cmd] {\n", T_SEG, "rust"),
    # Item 2, gh: the mapping, the endpoint URL, a flag's value, repo view, its URL form, `.git`, the API URL, and a
    # widened subcommand.
    Control("p35a_no_gh_mapping", GATE, GH_CALL, "", T_GH_API, "rust"),
    Control("p35a_gh_endpoint_dropped", GATE,
            "                    if !tail.is_empty() {\n", "                    if false {\n", T_GH_API, "rust"),
    Control("p35a_gh_flag_value_is_endpoint", GATE,
            "            args.next();\n", "", T_GH_API, "rust"),
    Control("p35a_gh_view_unmapped", GATE,
            '            ("repo", Some("view")) => {\n', '            ("repo", Some("view")) if false => {\n',
            T_GH_VIEW, "rust"),
    Control("p35a_gh_view_url_unread", GATE,
            '                        rest.strip_prefix("github.com/").unwrap_or("")\n', '                        ""\n',
            T_GH_VIEW, "rust"),
    Control("p35a_gh_view_git_kept", GATE,
            '                let arg = arg.strip_suffix(".git").unwrap_or(arg);\n', "", T_GH_VIEW, "rust"),
    Control("p35a_gh_no_api_url", GATE,
            '            format!("https://api.github.com/repos/{o}/{n}"),\n', "", T_GH_VIEW, "rust"),
    Control("p35a_gh_any_subcommand", GATE,
            '            ("repo", Some("view")) => {\n', '            ("repo" | "search", _) => {\n', T_GH_VIEW, "rust"),
    # Item 3: agy's bladebro call through call_mcp_tool.
    Control("p35a_p2_no_bladebro", GATE,
            '                || s_lower.contains("bladebro")\n', "", T_P2, "rust"),
    # Items 4 and 5: the real send-backs count exactly what the twin counted. Reading per script no longer moves them
    # (the stable-id and output rules already count every URL per script would), so that sabotage is T_SEG's alone.
    Control("p35a_real_no_gh", GATE, GH_CALL, "", T_REAL, "rust"),
)

# The MotoProject round (orchestrator, 2026-10-31): stable ids, yt_dlp, file sources, `Owns:`, the sources send-back
# brief, and the owned axis in the project folder. Rewritten in the review round where its find text moved.
T_SID = "p35a_stable_ids_are_video_post_repository_and_path"
T_STABLE = "p35a_a_stable_id_counts_in_a_fetching_command_or_its_output"
T_YT = "p35a_yt_dlp_is_a_fetch_word"
T_FILE = "p35a_a_file_source_counts_against_read_file"
T_OWNS = "p35a_the_owns_key_is_read_in_any_case"
T_BRIEF = "p35a_a_sources_send_back_says_no_cited_source_is_trusted"
T_SNAP = "p35a_the_owned_axis_in_the_project_folder_counts_only_the_orders_changes"
T_MTIME = "p35a_with_no_snapshot_the_owned_axis_counts_files_modified_since_seating"
T_MOTO = "p35a_motoproject_streams_count_what_the_twin_counts"

STABLE_CALL = "        if stable_read(url, &reads) {\n            continue;\n        }\n"
NO_STABLE = ""
AGY_FETCH_ONLY = "                command_fetches(command).then(|| command.to_owned())\n"
AGY_ANY = "                Some(command.to_owned())\n"
VIDEO = 'pub const VIDEO_FETCH_WORDS: &[&str] = &["yt_dlp", "yt-dlp", "YoutubeDL"];'
WHOLE_TREE = "        _ => uncommitted(subject.cwd).map(|changed| (changed, None::<&str>)),\n"

CONTROLS = CONTROLS + (
    # Stable ids: each form, the id length, a page inside a repository, the API form, a file, the lowercase repo.
    Control("p35a_sid_no_youtu_be", GATE,
            '        "youtu.be" => parts.first().and_then(|id| video_id(id)),\n', '        "youtu.be" => None,\n',
            T_SID, "rust"),
    Control("p35a_sid_no_shorts", GATE,
            '            ["shorts" | "embed" | "live", id, ..] => video_id(id),\n',
            '            ["embed" | "live", id, ..] => video_id(id),\n', T_SID, "rust"),
    Control("p35a_sid_any_length", GATE,
            "    (id.len() == 11 && id.bytes().all(id_byte))", "    (id.bytes().all(id_byte))", T_SID, "rust"),
    Control("p35a_sid_no_post", GATE,
            '            .position(|p| *p == "comments")\n', '            .position(|p| *p == "comment")\n', T_SID, "rust"),
    Control("p35a_sid_deep_repo", GATE,
            "            [owner, name] => repo_id(owner, name),\n",
            "            [owner, name, ..] => repo_id(owner, name),\n", T_SID, "rust"),
    Control("p35a_sid_no_api_repo", GATE,
            '            ["repos", owner, name] => repo_id(owner, name),\n', "            _ if false => None,\n",
            T_SID, "rust"),
    Control("p35a_sid_no_file", GATE,
            '    if let Some(path) = url.strip_prefix("file://") {\n',
            '    if let Some(path) = url.strip_prefix("file:///nothing") {\n', T_SID, "rust"),
    Control("p35a_sid_repo_case_kept", GATE,
            '        .then(|| StableId::Repo(format!("{owner}/{name}").to_ascii_lowercase()))\n',
            '        .then(|| StableId::Repo(format!("{owner}/{name}")))\n', T_SID, "rust"),
    # Counting: the rule is used at all, only fetching commands, outputs read, the id bounds, Claude's results, the
    # `.rss` post.
    Control("p35a_stable_unused", GATE, STABLE_CALL, NO_STABLE, T_STABLE, "rust"),
    Control("p35a_stable_any_command", GATE, AGY_FETCH_ONLY, AGY_ANY, T_STABLE, "rust"),
    Control("p35a_stable_no_output", GATE,
            "            if output_vouches(&text) {\n                reads.outputs.push(output.to_owned());\n            }\n",
            "", T_STABLE, "rust"),
    Control("p35a_video_id_unbounded", GATE,
            "            .any(|(at, _)| free(text, at, v.len(), &|c| c.is_ascii() && id_byte(c as u8))),\n",
            "            .any(|(at, _)| free(text, at, v.len(), &|_| false)),\n", T_STABLE, "rust"),
    Control("p35a_post_id_unbounded", GATE,
            "                        && !lower[at + needle.len()..]\n                            .chars()\n"
            "                            .next()\n                            .is_some_and(|c| c.is_ascii_alphanumeric())\n",
            "                        && true\n", T_STABLE, "rust"),
    Control("p35a_claude_result_unread", GATE,
            "                        if !id.is_empty() && fetch_ids.iter().any(|f| f == id) {\n",
            "                        if false {\n", T_STABLE, "rust"),
    Control("p35a_post_only_redd_it", GATE,
            '            ["comments/", "redd.it/"].iter().any(|lead| {\n', '            ["redd.it/"].iter().any(|lead| {\n',
            T_STABLE, "rust"),
    # yt_dlp: drop each word.
    Control("p35a_no_yt_dlp", GATE, VIDEO, 'pub const VIDEO_FETCH_WORDS: &[&str] = &["yt-dlp", "YoutubeDL"];', T_YT, "rust"),
    Control("p35a_no_yt_dash_dlp", GATE, VIDEO, 'pub const VIDEO_FETCH_WORDS: &[&str] = &["yt_dlp", "YoutubeDL"];', T_YT,
            "rust"),
    Control("p35a_no_youtubedl", GATE, VIDEO, 'pub const VIDEO_FETCH_WORDS: &[&str] = &["yt_dlp", "yt-dlp"];', T_YT,
            "rust"),
    # Files: agy's read_file, Claude's Read, separators and case, the exact path.
    Control("p35a_file_no_read_file", GATE,
            '            "read_file" | "view_file" => {\n', '            "view_file" => {\n', T_FILE, "rust"),
    Control("p35a_file_no_claude_read", GATE,
            '                            "Read" => {\n', '                            "ReadX" => {\n', T_FILE, "rust"),
    Control("p35a_file_raw_path", GATE,
            "    path.replace('\\\\', \"/\").to_lowercase()\n", "    path.to_owned()\n", T_FILE, "rust"),
    Control("p35a_file_any_path", GATE,
            "        Some(StableId::Path(path)) => reads.paths.contains(&path),\n",
            "        Some(StableId::Path(_)) => !reads.paths.is_empty(),\n", T_FILE, "rust"),
    # `Owns:` in any case, and only the key itself.
    Control("p35a_owns_lowercase_only", GATE,
            '        if let Some(rest) = strip_key(line, "owns:") {\n',
            '        if let Some(rest) = line.strip_prefix("owns:") {\n', T_OWNS, "rust"),
    Control("p35a_owns_prefix_only", GATE,
            "        .filter(|head| head.eq_ignore_ascii_case(key))\n",
            "        .filter(|head| head[..4].eq_ignore_ascii_case(&key[..4]))\n", T_OWNS, "rust"),
    # The send-back brief (review S4): the sources text on a sources fail with cited sources, added to the others'
    # instruction on a mixed fail, and never for a fail that cites nothing.
    Control("p35a_brief_never_redo", GATE,
            "        (true, false) => SOURCES_REDO.to_owned(),\n", "        (true, false) => fix.to_owned(),\n",
            T_BRIEF, "rust"),
    Control("p35a_brief_mixed_replaces", GATE,
            '        (true, true) => format!("{fix} For the sources: {SOURCES_REDO}"),\n',
            "        (true, true) => SOURCES_REDO.to_owned(),\n", T_BRIEF, "rust"),
    Control("p35a_brief_mixed_drops_sources", GATE,
            '        (true, true) => format!("{fix} For the sources: {SOURCES_REDO}"),\n',
            "        (true, true) => fix.to_owned(),\n", T_BRIEF, "rust"),
    Control("p35a_brief_redo_without_sources", GATE,
            '        .any(|axis| axis.axis == "sources" && !axis.lines.is_empty());\n',
            '        .any(|axis| axis.axis == "sources");\n', T_BRIEF, "rust"),
    # The owned axis in the project folder: the whole tree again, the snapshot's prints ignored, the time ignored,
    # and (review S3) deletions dropped, the creation time ignored, quoted paths, the missing-snapshot note.
    Control("p35a_owned_whole_tree", GATE, "        _ => order_changes(subject),\n", WHOLE_TREE, T_SNAP, "rust"),
    Control("p35a_owned_whole_tree_mtime", GATE, "        _ => order_changes(subject),\n", WHOLE_TREE, T_MTIME, "rust"),
    Control("p35a_owned_snapshot_paths_only", GATE,
            "                snapshot.get(path) != Some(&now)\n", "                !snapshot.contains_key(path)\n",
            T_SNAP, "rust"),
    Control("p35a_owned_time_ignored", GATE,
            "                .is_none_or(|(_, nanos)| nanos / 1_000_000_000 >= u128::from(seated_at))\n",
            "                .is_none_or(|_| seated_at > 0)\n", T_MTIME, "rust"),
    Control("p35a_owned_deletion_dropped", GATE,
            "                .is_none_or(|(_, nanos)| nanos / 1_000_000_000 >= u128::from(seated_at))\n",
            "                .is_some_and(|(_, nanos)| nanos / 1_000_000_000 >= u128::from(seated_at))\n", T_MTIME, "rust"),
    Control("p35a_owned_created_ignored", GATE,
            "        nanos(meta.modified()).max(nanos(meta.created())),\n", "        nanos(meta.modified()),\n",
            T_MTIME, "rust"),
    Control("p35a_owned_quoted_paths", GATE,
            '            "--porcelain=v1",\n            "-z",\n', '            "--porcelain=v1",\n', T_MTIME, "rust"),
    Control("p35a_owned_no_note", GATE,
            '                owned.why = format!("{} ({note})", owned.why);\n', "", T_MTIME, "rust"),
    Control("p35a_owned_note_with_snapshot", GATE,
            "        return Ok((changed, None));\n", "        return Ok((changed, Some(NO_SNAPSHOT)));\n", T_SNAP, "rust"),
    # MotoProject's streams: the stable rule at all, and only fetching commands (the fabricated order and its notes).
    Control("p35a_moto_stable_unused", GATE, STABLE_CALL, NO_STABLE, T_MOTO, "rust"),
    Control("p35a_moto_any_command", GATE, AGY_FETCH_ONLY, AGY_ANY, T_MOTO, "rust"),
    Control("p35a_real_stable_unused", GATE, STABLE_CALL, NO_STABLE, T_REAL, "rust"),
)

# The review round (p35a-review.md): B1 plain URLs only per segment, S1 `gh` as a command word, S2 fetch words as
# command words, cut commands, listings and local reads, WebFetch's url, S4 above, and the nits' bounds.
T_PLAIN = "p35a_a_plain_url_never_counts_from_an_output"
T_WORD = "p35a_a_fetch_word_counts_only_as_a_command_word"
T_CUT = "p35a_a_cut_command_counts_its_fetch_import"
T_LIST = "p35a_a_listing_or_a_local_read_does_not_vouch_for_its_output"
T_WF = "p35a_claude_webfetch_counts_its_url_and_a_browser_mcp_call_its_input"
T_GH = "p35a_gh_counts_only_as_a_command_word"
T_BOUNDS = "p35a_repo_and_post_ids_keep_their_bounds"

PLAIN_NONE = "        None => false,\n"
CHILD = "        && (FETCH_CALLS.iter().any(|call| calls(&code, call)) || runs_fetch_child(segment, &code))\n"
CUT = "        || (cmd.trim_end().ends_with(CUT_MARK) && segments(cmd).any(imports_fetch))\n"
YT_HOST = '        || (["youtube.com/", "youtu.be/"]\n'

CONTROLS = CONTROLS + (
    # B1: a plain URL from an output (round 2's arm), or from anywhere in a fetching command's text.
    Control("p35a_plain_url_in_output", GATE, PLAIN_NONE,
            "        None => reads.outputs.iter().any(|output| {\n"
            "            extract_url_tokens(output).iter().any(|t| normalize_url(t) == normalize_url(url))\n"
            "        }),\n", T_PLAIN, "rust"),
    Control("p35a_plain_url_in_command_text", GATE, PLAIN_NONE,
            "        None => reads.texts.iter().any(|text| {\n"
            "            extract_url_tokens(text).iter().any(|t| normalize_url(t) == normalize_url(url))\n"
            "        }),\n", T_STABLE, "rust"),
    # S2: a fetch word anywhere, joined to a longer name, after `$`, in a comment, in a string, in a shell's quoted
    # code; a printing segment's calls; the subprocess argv, any argv.
    Control("p35a_fetch_word_anywhere", GATE,
            "        (!joined && command_at(code, at)).then_some(at)\n", "        (!joined).then_some(at)\n", T_WORD, "rust"),
    Control("p35a_fetch_word_joined", GATE,
            "        (!joined && command_at(code, at)).then_some(at)\n", "        command_at(code, at).then_some(at)\n",
            T_WORD, "rust"),
    Control("p35a_fetch_word_variable", GATE,
            "    if code[..at].ends_with('$') {\n        return false;\n    }\n", "", T_WORD, "rust"),
    Control("p35a_comment_read", GATE,
            "        if byte == b'#' && (at == 0 || bytes[at - 1].is_ascii_whitespace()) {\n", "        if false {\n",
            T_WORD, "rust"),
    Control("p35a_string_read", GATE,
            "                bytes[at..at + len + 2].fill(b' ');\n", "", T_WORD, "rust"),
    Control("p35a_shell_code_blanked", GATE,
            "            bytes[at] = b'(';\n            at += 1;\n            continue;\n", "", T_WORD, "rust"),
    Control("p35a_printing_segment_calls", GATE,
            "    !ARG_COMMANDS.contains(&head)\n", "    true\n", T_WORD, "rust"),
    Control("p35a_no_subprocess", GATE,
            CHILD, "        && FETCH_CALLS.iter().any(|call| calls(&code, call))\n", T_WORD, "rust"),
    # No T_MOTO control for the argv: every r2 command with a `subprocess` curl is also cut and imports
    # urllib.request, so dropping the argv moved no real count (measured, the control was hollow).
    Control("p35a_subprocess_any_argv", GATE,
            "                FETCH_COMMANDS.contains(&word)\n", "                !word.is_empty()\n", T_WORD, "rust"),
    # S2, agy's cut: ignored, an import counted uncut, any module counted.
    Control("p35a_cut_ignored", GATE, CUT, "", T_CUT, "rust"),
    Control("p35a_moto_cut_ignored", GATE, CUT, "", T_MOTO, "rust"),
    Control("p35a_import_counts_uncut", GATE,
            CUT, "        || segments(cmd).any(imports_fetch)\n", T_CUT, "rust"),
    Control("p35a_import_any_module", GATE,
            '        .any(|name| FETCH_IMPORTS.contains(&name.split(" as ").next().unwrap_or("").trim()))\n',
            "        .any(|name| !name.trim().is_empty())\n", T_CUT, "rust"),
    # S2, listings and local reads: ignored at all, each named listing, YouTube pages, yt-dlp's own video, a local
    # reader, `file://`, python's `open(`, a reader later in a pipe.
    Control("p35a_listing_ignored", GATE, "        && !listing(&lower)\n", "", T_LIST, "rust"),
    Control("p35a_listing_no_ytsearch", GATE, _drop("ytsearch"), "", T_LIST, "rust"),
    Control("p35a_listing_no_flat_playlist", GATE, _drop("--flat-playlist"), "", T_LIST, "rust"),
    Control("p35a_listing_no_channel", GATE, _drop("youtube.com/@"), "", T_LIST, "rust"),
    Control("p35a_listing_no_reddit_search", GATE, _drop("search.json"), "", T_LIST, "rust"),
    Control("p35a_listing_no_github_search", GATE, _drop("github.com/search"), "", T_LIST, "rust"),
    Control("p35a_listing_no_related", GATE, YT_HOST, "        || (false && [\"youtube.com/\", \"youtu.be/\"]\n",
            T_LIST, "rust"),
    Control("p35a_listing_yt_dlp_too", GATE,
            '            && !["yt_dlp", "yt-dlp", "youtubedl"]\n                .iter()\n'
            "                .any(|word| lower.contains(word)))\n", ")\n", T_LIST, "rust"),
    Control("p35a_local_reader_ignored", GATE,
            'const LOCAL_READERS: &[&str] = &[\n    "cat",\n', "const LOCAL_READERS: &[&str] = &[\n", T_LIST, "rust"),
    Control("p35a_file_url_output", GATE, '    !lower.contains("file://")\n', "    true\n", T_LIST, "rust"),
    Control("p35a_open_call_ignored", GATE,
            '        || code.match_indices("open(").any(|(at, _)| {\n',
            '        || code.match_indices("open(x").any(|(at, _)| {\n', T_LIST, "rust"),
    Control("p35a_reader_in_pipe", GATE,
            "            .filter_map(|pipeline| pipeline.split('|').next())\n",
            "            .flat_map(|pipeline| pipeline.split('|'))\n", T_LIST, "rust"),
    # S2, Claude: WebFetch's prompt read as the fetch, bladebro's MCP call unread.
    Control("p35a_webfetch_prompt", GATE,
            '                            "WebFetch" => {\n'
            "                                input.get(\"url\").and_then(Value::as_str).map(str::to_owned)\n"
            "                            }\n",
            '                            "WebFetch" => Some(json_text(input)),\n', T_WF, "rust"),
    Control("p35a_claude_mcp_unread", GATE,
            '                                && ["browser", "bladebro", "chrome", "playwright"]\n',
            '                                && ["browser", "chrome", "playwright"]\n', T_WF, "rust"),
    # S1: `gh` anywhere in a segment.
    Control("p35a_gh_any_word", GATE,
            '    for at in command_words(&code, "gh") {\n',
            '    for at in code.match_indices("gh").map(|(at, _)| at) {\n', T_GH, "rust"),
    # Nits: the repository's bounds and `.git`, the host prefixes, a post id outside reddit.com, a Unix `file://`
    # path, a `%XX` escape.
    Control("p35a_repo_unbounded", GATE,
            "                !lower[..at].chars().next_back().is_some_and(joins)\n"
            "                    && !tail.chars().next().is_some_and(joins)\n",
            "                true\n", T_BOUNDS, "rust"),
    Control("p35a_repo_git_kept", GATE,
            '                let tail = tail.strip_prefix(".git").unwrap_or(tail);\n', "", T_BOUNDS, "rust"),
    Control("p35a_host_prefixes_dropped", GATE,
            '    let host = ["www.", "m.", "old.", "music.", "embed."]\n', '    let host = ["www."]\n', T_BOUNDS, "rust"),
    Control("p35a_post_outside_reddit", GATE,
            '                    (*lead == "redd.it/" || token.contains("reddit.com/"))\n', "                    true\n",
            T_BOUNDS, "rust"),
    Control("p35a_file_unix_slash_dropped", GATE,
            "        let drive = b.len() > 2 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':';\n",
            "        let drive = !b.is_empty();\n", T_SID, "rust"),
    Control("p35a_file_escape_kept", GATE,
            "            (b'%', Some(byte)) => {\n", "            (b'%', Some(byte)) if false => {\n", T_SID, "rust"),
)
