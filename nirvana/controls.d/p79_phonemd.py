# Phone markdown (feature phone-markdown): agent bubbles on the phone page draw Claude Code's markdown as nodes. The
# owner, 2026-11-14: "the text formatting is not rendered". Controls starting with p79_. One sabotage per assertion,
# each on a product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
# The renderer's behaviour is nirvana/test_phonemd.py's (python, node underneath); its serving and wiring is the rust
# test in tests_live.rs.
MD_JS = "src/nirvana_remote/page/md.js"
CHAT_JS = "src/nirvana_remote/page/chat.js"
INDEX = "src/nirvana_remote/page/index.html"
APP_CSS = "src/nirvana_remote/page/app.css"
ROUTES = "src/nirvana_remote/routes.rs"

_P = "nirvana.test_phonemd.PhoneMarkdownTest."
T_INLINE = _P + "test_bold_italic_and_inline_code"
T_PARA = _P + "test_paragraphs_split_on_blank_lines_and_keep_line_breaks"
T_HEAD = _P + "test_headings"
T_FENCE = _P + "test_fenced_code_keeps_its_text_and_language"
T_OPEN = _P + "test_a_fence_open_at_the_end_runs_to_the_end_only"
T_LIST = _P + "test_lists_nest_by_indent_and_items_continue"
T_START = _P + "test_an_ordered_list_keeps_its_start_and_a_blank_line_ends_a_list"
T_QUOTE = _P + "test_quotes_and_rules"
T_TABLE = _P + "test_a_pipe_table_scrolls_in_its_own_box_and_keeps_every_cell"
T_LINK = _P + "test_links_are_made_only_for_http_and_https"
T_MARKUP = _P + "test_markup_in_the_agent_text_stays_text"
T_UNCLOSED = _P + "test_an_unclosed_mark_keeps_every_character"
T_PATHS = _P + "test_paths_and_snake_case_stay_as_written"
T_DEEP = _P + "test_deep_nesting_is_bounded"
T_SERVED = "nirvana_remote::tests_live::the_markdown_script_is_served_under_the_csp_and_draws_agent_bubbles_only"

CONTROLS = (
    # --- What md.js draws (python, node underneath) ---
    # **bold** is a strong node
    Control("p79_md_strong", MD_JS,
            "            out.push(h(mark.length === 2 ? 'strong' : 'em', {}, ...inline(h, s.slice(i + mark.length, end), links)));\n",
            "            out.push(h(mark.length === 2 ? 'em' : 'em', {}, ...inline(h, s.slice(i + mark.length, end), links)));\n",
            T_INLINE, "python"),
    # A line break inside a paragraph stays a line break
    Control("p79_md_line_break", MD_JS,
            r"      if (c === '\n') { flush(); out.push(h('br')); i++; continue; }" "\n",
            r"      if (c === '\n') { flush(); out.push(' '); i++; continue; }" "\n",
            T_PARA, "python"),
    # # to ###### are headings
    Control("p79_md_heading", MD_JS,
            "  const HEADING = /^ {0,3}(#{1,6})",
            "  const HEADING = /^ {0,3}(#{7})",
            T_HEAD, "python"),
    # A fence's language shows; no character of it is dropped
    Control("p79_md_fence_lang", MD_JS,
            "            info ? h('span', { class: 'md-lang mono', text: info }) : null,\n",
            "            null,\n",
            T_FENCE, "python"),
    # A fence with no closer runs to the end of its message: its last line is not read again as text
    Control("p79_md_fence_open", MD_JS,
            "          i = j + 1;\n          continue;\n",
            "          i = Math.min(j + 1, lines.length - 1);\n          continue;\n",
            T_OPEN, "python"),
    # A bullet under "1. a" at two spaces is a nested list, not the end of the item
    Control("p79_md_list_nest", MD_JS,
            "      if (n >= base + 2) {",
            "      if (n >= base + 99) {",
            T_LIST, "python"),
    # An ordered list that starts at 3 says 3
    Control("p79_md_list_start", MD_JS,
            "    if (first.ordered && first.start !== 1) props.start = String(first.start);\n",
            "",
            T_START, "python"),
    # > is a quote
    Control("p79_md_quote", MD_JS,
            "      if (QUOTE.test(line)) {\n",
            "      if (false && QUOTE.test(line)) {\n",
            T_QUOTE, "python"),
    # A row with more cells than the head keeps them
    Control("p79_md_table_extra", MD_JS,
            "          if (row.length > head.length) r[head.length - 1] += ' | ' + row.slice(head.length).join(' | ');\n",
            "",
            T_TABLE, "python"),
    # Only http(s) becomes a link: javascript:, data:, file: stay text
    Control("p79_md_link_scheme", MD_JS,
            "  const URL = /^https?:\\/\\/[^\\s<>\"]+$/i;\n",
            "  const URL = /^[a-z]+:/i;\n",
            T_LINK, "python"),
    # A bare http(s) URL is a link
    Control("p79_md_bare_url", MD_JS,
            "        if (url) { flush(); out.push(anchor(h, url, [url])); i += url.length; continue; }\n",
            "",
            T_LINK, "python"),
    # The test sees a renderer that turns the agent's <tag into a node
    Control("p79_md_markup_node", MD_JS,
            "      text += c;\n      i++;\n",
            "      if (c === '<') { const t = /^<(\\w+)/.exec(s.slice(i)); if (t) { flush(); out.push(h(t[1], {})); i += t[0].length; continue; } }\n"
            "      text += c;\n      i++;\n",
            T_MARKUP, "python"),
    # An opener with no closer stays as written
    Control("p79_md_unclosed_kept", MD_JS,
            "        text += mark;\n        i += mark.length;\n",
            "        i += mark.length;\n",
            T_UNCLOSED, "python"),
    # A backslash before . or \ is the path's, not an escape: C:\Users\x\.claude and \\server stay whole
    Control("p79_md_escape_set", MD_JS,
            "  const ESCAPABLE = '*_`';\n",
            "  const ESCAPABLE = '*_`.\\\\';\n",
            T_PATHS, "python"),
    # Nesting stops at DEEPEST
    Control("p79_md_depth_bound", MD_JS,
            "  const DEEPEST = 12;\n",
            "  const DEEPEST = 100000;\n",
            T_DEEP, "python"),

    # --- How it is served and wired (rust) ---
    Control("p79_served_row", ROUTES,
            "        \"/md.js\",\n",
            "        \"/md-off.js\",\n",
            T_SERVED, "rust"),
    Control("p79_served_type", ROUTES,
            "        \"/md.js\",\n        \"text/javascript; charset=utf-8\",\n",
            "        \"/md.js\",\n        \"text/plain; charset=utf-8\",\n",
            T_SERVED, "rust"),
    Control("p79_served_csp", ROUTES,
            "        self.route(peer, &request, conn)\n            .with(\"Content-Security-Policy\", CSP)\n",
            "        self.route(peer, &request, conn)\n",
            T_SERVED, "rust"),
    Control("p79_md_hook", MD_JS,
            "  if (typeof window === 'object' && window.Nirvana) window.Nirvana.md = (text) => render(text, window.Nirvana.h);\n",
            "",
            T_SERVED, "rust"),
    Control("p79_md_no_innerhtml", MD_JS,
            "  function anchor(h, url, children) {\n",
            "  function raw(el, s) { el.innerHTML = s; }\n  function anchor(h, url, children) {\n",
            T_SERVED, "rust"),
    Control("p79_served_link_scheme", MD_JS,
            "  const URL = /^https?:\\/\\/[^\\s<>\"]+$/i;\n",
            "  const URL = /^[a-z]+:/i;\n",
            T_SERVED, "rust"),
    Control("p79_index_loads_md", INDEX,
            "<script src=\"/md.js\"></script>\n",
            "",
            T_SERVED, "rust"),
    # md.js before app.js: window.Nirvana is not there yet, so nv.md would never be set. The md.js line then shows
    # twice; the test's find() reads the first.
    Control("p79_index_order", INDEX,
            "<script src=\"/app.js\"></script>\n<!-- md.js after app.js",
            "<script src=\"/md.js\"></script>\n<script src=\"/app.js\"></script>\n<!-- md.js after app.js",
            T_SERVED, "rust"),
    Control("p79_chat_agent_md", CHAT_JS,
            "        if (m.kind === 'agent') return h('div', { class: 'msg bubble-agent md' }, ...nv.md(m.text || ''));\n",
            "        if (m.kind === 'agent') return h('div', { class: 'msg bubble-agent', text: m.text });\n",
            T_SERVED, "rust"),
    Control("p79_chat_user_plain", CHAT_JS,
            "        if (m.kind === 'user') return h('div', { class: 'msg bubble-user', text: m.text });\n",
            "        if (m.kind === 'user') return h('div', { class: 'msg bubble-user' }, ...nv.md(m.text));\n",
            T_SERVED, "rust"),
    Control("p79_css_pre_scrolls", APP_CSS,
            "white-space: pre; overflow-wrap: normal; overflow-x: auto;",
            "white-space: pre-wrap; overflow-wrap: anywhere; overflow-x: auto;",
            T_SERVED, "rust"),
    Control("p79_css_table_box", APP_CSS,
            ".md .md-scroll { min-width: 0; max-width: 100%; overflow-x: auto;",
            ".md .md-scroll { min-width: 0; max-width: 100%;",
            T_SERVED, "rust"),
    Control("p79_css_bold", APP_CSS,
            ".md strong { font-weight: 700;",
            ".md strong { font-weight: 400;",
            T_SERVED, "rust"),
)
