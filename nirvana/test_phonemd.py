"""The phone page's markdown renderer for agent bubbles (src/nirvana_remote/page/md.js), run in node.

The owner, 2026-11-14: "Ive noticed for the nirvana app the text formatting is not rendered. We have alot spacing and
bolding in normal claude code agent but it doesnt read as easily and maybe ignored in Nirvana phone app."

    python -m unittest nirvana.test_phonemd

node writes each render out as markup with every text escaped (nirvana/phonemd_dump.js), so a `<tag` in the output is
a node md.js made and `&lt;` is the agent's text. No node is a failure, not a skip: a gate that skips reads as a pass.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
DUMP = HERE / "phonemd_dump.js"


def node() -> str:
    found = shutil.which("node") or shutil.which("node.exe")
    if found:
        return found
    for candidate in (r"C:\Program Files\nodejs\node.exe", "/usr/bin/node", "/usr/local/bin/node"):
        if os.path.isfile(candidate):
            return candidate
    raise AssertionError("node is not installed: the markdown renderer cannot be tested")


def run(texts: list[str], mode: str | None = None) -> list[str]:
    argv = [node(), str(DUMP)] + ([mode] if mode else [])
    p = subprocess.run(argv, input=json.dumps(texts), capture_output=True, text=True, encoding="utf-8", timeout=60)
    if p.returncode != 0:
        raise AssertionError(f"node exited {p.returncode}: {p.stderr.strip()[-2000:]}")
    return json.loads(p.stdout)


def md(text: str) -> str:
    return run([text])[0]


def plain(text: str) -> str:
    return run([text], "--text")[0]


P = '<p class="md-p">'
LI = '<li class="md-li">'


class PhoneMarkdownTest(unittest.TestCase):
    def test_bold_italic_and_inline_code(self):
        self.assertEqual(md("a **bold** b"), P + "a <strong>bold</strong> b</p>")
        self.assertEqual(md("a *it* and _it_ b"), P + "a <em>it</em> and <em>it</em> b</p>")
        self.assertEqual(md("***both***"), P + "<strong><em>both</em></strong></p>")
        self.assertEqual(md("run `cargo **x**` now"),
                         P + 'run <code class="md-code-span">cargo **x**</code> now</p>')
        self.assertEqual(md("**Note:** `a`"), P + '<strong>Note:</strong> <code class="md-code-span">a</code></p>')

    def test_paragraphs_split_on_blank_lines_and_keep_line_breaks(self):
        self.assertEqual(md("one\ntwo\n\n\nthree"), P + "one<br></br>two</p>" + P + "three</p>")
        self.assertEqual(md("a\r\nb"), P + "a<br></br>b</p>")

    def test_headings(self):
        self.assertEqual(md("# Top\n## Mid **b**\n### Low\n#### Four"),
                         '<div class="md-h md-h1" role="heading" aria-level="1">Top</div>'
                         '<div class="md-h md-h2" role="heading" aria-level="2">Mid <strong>b</strong></div>'
                         '<div class="md-h md-h3" role="heading" aria-level="3">Low</div>'
                         '<div class="md-h md-h4" role="heading" aria-level="4">Four</div>')
        self.assertEqual(md("#hashtag"), P + "#hashtag</p>")

    def test_fenced_code_keeps_its_text_and_language(self):
        out = md("Before\n```rust\nfn x() { **y** < 1 }\n    indented\n```\nAfter")
        self.assertEqual(out, P + "Before</p>"
                         '<div class="md-code"><span class="md-lang mono">rust</span>'
                         '<pre class="md-pre mono"><code>fn x() { **y** &lt; 1 }\n    indented</code></pre></div>'
                         + P + "After</p>")
        self.assertEqual(md("~~~\na\n~~~"), '<div class="md-code"><pre class="md-pre mono"><code>a</code></pre></div>')

    def test_a_fence_open_at_the_end_runs_to_the_end_only(self):
        self.assertEqual(md("Text\n```py\nprint(1)\n**not bold**"),
                         P + "Text</p>" '<div class="md-code"><span class="md-lang mono">py</span>'
                         '<pre class="md-pre mono"><code>print(1)\n**not bold**</code></pre></div>')
        # The next message is drawn on its own: an open fence in one does not reach it.
        self.assertEqual(run(["```\nopen", "**bold**"])[1], P + "<strong>bold</strong></p>")

    def test_lists_nest_by_indent_and_items_continue(self):
        out = md("Steps:\n- one\n  goes on\n- two\n  - inner\n    - deeper\n- three")
        self.assertEqual(out, P + "Steps:</p>" '<ul class="md-list">'
                         + LI + P + "one<br></br>goes on</p></li>"
                         + LI + P + "two</p>" '<ul class="md-list">' + LI + P + "inner</p>"
                         '<ul class="md-list">' + LI + P + "deeper</p></li></ul></li></ul></li>"
                         + LI + P + "three</p></li></ul>")
        # Claude's "1. a" with a bullet under it at two spaces, fewer than the item's text column
        out = md("1. first\n  - sub\n2. second")
        self.assertEqual(out, '<ol class="md-list">' + LI + P + "first</p>" '<ul class="md-list">' + LI + P
                         + "sub</p></li></ul></li>" + LI + P + "second</p></li></ol>")

    def test_an_ordered_list_keeps_its_start_and_a_blank_line_ends_a_list(self):
        self.assertEqual(md("3. c\n4. d\n\nafter"), '<ol class="md-list" start="3">' + LI + P + "c</p></li>"
                         + LI + P + "d</p></li></ol>" + P + "after</p>")
        self.assertEqual(md("- a\n1. b"), '<ul class="md-list">' + LI + P + "a</p></li></ul>"
                         '<ol class="md-list">' + LI + P + "b</p></li></ol>")

    def test_quotes_and_rules(self):
        self.assertEqual(md("> said **this**\n> and more\n\n---\nend"),
                         '<blockquote class="md-quote">' + P + "said <strong>this</strong><br></br>and more</p>"
                         '</blockquote><hr class="md-rule"></hr>' + P + "end</p>")

    def test_a_pipe_table_scrolls_in_its_own_box_and_keeps_every_cell(self):
        out = md("| Name | Size |\n|:-----|-----:|\n| `a.rs` | **12** |\n| b \\| c |\n| x | y | extra |")
        self.assertEqual(out, '<div class="md-scroll"><table class="md-table"><thead><tr>'
                         '<th class="md-cell">Name</th><th class="md-cell md-al-r">Size</th></tr></thead><tbody>'
                         '<tr><td class="md-cell"><code class="md-code-span">a.rs</code></td>'
                         '<td class="md-cell md-al-r"><strong>12</strong></td></tr>'
                         '<tr><td class="md-cell">b | c</td><td class="md-cell md-al-r"></td></tr>'
                         '<tr><td class="md-cell">x</td><td class="md-cell md-al-r">y | extra</td></tr>'
                         "</tbody></table></div>")
        # A pipe in prose with no delimiter row under it is text
        self.assertEqual(md("a | b"), P + "a | b</p>")

    def test_links_are_made_only_for_http_and_https(self):
        link = '<a class="md-link" href="https://example.com/a_(b)" rel="noopener noreferrer" target="_blank">'
        self.assertEqual(md("see [the **doc**](https://example.com/a_(b))."),
                         P + "see " + link + "the <strong>doc</strong></a>.</p>")
        bare = '<a class="md-link" href="http://x.io/p" rel="noopener noreferrer" target="_blank">http://x.io/p</a>'
        self.assertEqual(md("at http://x.io/p, then"), P + "at " + bare + ", then</p>")
        for other in ["[x](javascript:alert(1))", "[x](data:text/html,hi)", "[x](file:///C:/a)", "[x](/api/send)",
                      "[x](JAVASCRIPT:alert(1))", "javascript:alert(1)"]:
            self.assertNotIn("<a ", md(other), other)
            self.assertEqual(plain(other), other, other)

    def test_markup_in_the_agent_text_stays_text(self):
        for evil in ['<img src=x onerror=alert(1)>', "<script>alert(1)</script>", '**<b onclick="x()">hi</b>**',
                     '[<img src=x>](https://a.b/"onmouseover="x)', "`<script>`", "```html\n<script>alert(1)</script>\n```",
                     '| <svg onload=x> |\n|---|\n| <iframe> |']:
            out = md(evil)
            for tag in ("<img", "<script", "<svg", "<iframe", "<b ", "onerror=\"", "onclick=\"", "onload=\""):
                self.assertNotIn(tag, out, evil)
        self.assertEqual(md("<img src=x onerror=alert(1)>"), P + "&lt;img src=x onerror=alert(1)&gt;</p>")
        self.assertEqual(md("<script>alert(1)</script>"), P + "&lt;script&gt;alert(1)&lt;/script&gt;</p>")

    def test_an_unclosed_mark_keeps_every_character(self):
        for text in ["**bold that never closes", "a ` b", "``x`", "*a", "_a", "a **b *c", "a **** b", "[x](https://a.b",
                     "trailing **"]:
            self.assertEqual(plain(text), text, text)
        # A code span binds before emphasis: its ** is code, and the opener with no closer is text
        self.assertEqual(md("**a `b**` c"), P + '**a <code class="md-code-span">b**</code> c</p>')
        # ...and does not reach the next paragraph
        self.assertEqual(md("**open\n\nnext **b**"), P + "**open</p>" + P + "next <strong>b</strong></p>")

    def test_paths_and_snake_case_stay_as_written(self):
        for text in ["src/nirvana_remote/page/chat.js:131", "snake_case_name and __x", r"C:\Users\fahim\.claude",
                     r"\\server\share", "glob src/**/*.rs", "2 * 3 * 4", "file_path:line_number"]:
            self.assertEqual(plain(text), text, text)
        self.assertEqual(md(r"\*not\* it"), P + "*not* it</p>")

    def test_deep_nesting_is_bounded(self):
        out = md("> " * 3000 + "x")
        self.assertIn("x", out)
        self.assertLessEqual(out.count("<blockquote"), 14)
        self.assertEqual(plain("- " * 2000 + "y").strip()[-1], "y")


if __name__ == "__main__":
    unittest.main()
