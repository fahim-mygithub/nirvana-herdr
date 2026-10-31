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


def _drop(word: str) -> str:
    return f'    "{word}",\n'


CONTROLS = (
    # Item 1, Python: drop each new word; each case of T_PY holds only its own word.
    Control("p35a_no_urlopen", GATE, _drop("urlopen("), "", T_PY, "rust"),
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
    Control("p35a_no_iwr", GATE, _drop("iwr"), "", T_PS, "rust"),
    Control("p35a_no_invoke_webrequest", GATE, _drop("Invoke-WebRequest"), "", T_PS, "rust"),
    # Items 1 and 4, per segment: irm as a substring ("confirm"), a `$irm` variable, `python` alone, per script.
    Control("p35a_irm_substring", GATE,
            "        || FETCH_TOKENS.iter().any(|word| has_token(&lower, word))\n",
            "        || FETCH_TOKENS.iter().any(|word| lower.contains(word))\n", T_SEG, "rust"),
    Control("p35a_irm_variable", GATE,
            "        !before.is_some_and(|c| joins(c) || c == '$') && !after.is_some_and(joins)\n",
            "        !before.is_some_and(joins) && !after.is_some_and(joins)\n", T_SEG, "rust"),
    Control("p35a_python_is_a_fetch", GATE, _drop("urlopen("), _drop("python") + _drop("urlopen("), T_SEG, "rust"),
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
            '                        || s_lower.contains("bladebro")\n', "", T_P2, "rust"),
    # Items 4 and 5: the real send-backs count exactly what the twin counted, neither less (gh) nor more (per script).
    Control("p35a_real_no_gh", GATE, GH_CALL, "", T_REAL, "rust"),
    Control("p35a_real_per_script", GATE, PER_SEGMENT, "    for segment in [cmd] {\n", T_REAL, "rust"),
)
