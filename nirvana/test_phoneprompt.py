"""The phone page's prompt sheet (src/nirvana_remote/page/prompt.js `show`, app.css .prompt*).

The owner, 2026-11-16: "Also the choice dialog doesn't read well on the app". Their screenshot: a screen not read as a
menu, headed only by the agent's name (scrolled off the top), a box opening on the top of a long command with the
choices below it out of sight.

    python -m unittest nirvana.test_phoneprompt

What the sheet draws runs in node (nirvana/phoneprompt_dump.js, a small fake DOM). How it lays out needs a browser;
the tests here read the declarations app.css gives the sheet's parts, which is all a page test can see of layout: a
live look on the iPhone is what confirms it reads.
"""
from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path

from nirvana.test_phonemd import node

HERE = Path(__file__).resolve().parent
DUMP = HERE / "phoneprompt_dump.js"
CSS = HERE.parent / "src" / "nirvana_remote" / "page" / "app.css"

# The owner's screen, shaped: the tool line, a command wrapped over many rows, the question and its options last.
RAW = "\n".join(
    ["● Bash(grep -n \"^export\" src/store/slices/plannedWorkoutsSlice.ts | head -20; ls src/components/plan"]
    + [f"  command row {i}" for i in range(14)]
    + [" Do you want to proceed?", " ❯ 1. Yes", "   2. Yes, and don't ask again for: herdr seat pick", "   3. No",
       " Esc to cancel · Tab to amend"])
KEYS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "enter", "esc", "up", "down"]


def sheet(data: dict) -> dict:
    data = {"blocked": True, "seen": "s", "picker": False, **data}
    p = subprocess.run([node(), str(DUMP)], input=json.dumps(data), capture_output=True, text=True,
                       encoding="utf-8", timeout=60)
    if p.returncode != 0:
        raise AssertionError(f"node exited {p.returncode}: {p.stderr.strip()[-2000:]}")
    return json.loads(p.stdout)


def rules() -> dict[str, dict[str, str]]:
    """Each plain selector's declarations in app.css, later ones winning (the cascade at equal weight)."""
    text = re.sub(r"/\*.*?\*/", "", CSS.read_text(encoding="utf-8"), flags=re.S)
    out: dict[str, dict[str, str]] = {}
    for selectors, body in re.findall(r"([^{}@]+)\{([^{}]*)\}", text):
        decls = {}
        for decl in body.split(";"):
            if ":" in decl:
                k, v = decl.split(":", 1)
                decls[k.strip()] = v.strip()
        for sel in selectors.split(","):
            out.setdefault(sel.strip(), {}).update(decls)
    return out


def number(value: str, unit: str) -> float:
    m = re.fullmatch(r"([\d.]+)" + unit, value)
    if not m:
        raise AssertionError(f"{value!r} is not a length in {unit}")
    return float(m.group(1))


class PromptSheetDrawTest(unittest.TestCase):
    def test_a_raw_screen_is_headed_by_its_last_question(self):
        got = sheet({"menu": None, "keys": KEYS, "screen": RAW})
        self.assertEqual(got["order"][0], "prompt-head")
        self.assertEqual(got["title"], "Do you want to proceed?")

    def test_a_raw_screen_with_no_question_is_headed_by_the_agent(self):
        got = sheet({"menu": None, "keys": KEYS, "screen": "some output\n  more output"})
        self.assertEqual(got["title"], "nirvana-2 needs you")

    def test_a_raw_screen_box_opens_at_its_end(self):
        got = sheet({"menu": None, "keys": KEYS, "screen": RAW})
        self.assertEqual(got["box"], {"class": "prompt-box mono screen-box", "atEnd": True})

    def test_the_keypad_is_only_for_a_screen_not_read_as_a_menu(self):
        raw = sheet({"menu": None, "keys": KEYS, "screen": RAW})
        self.assertEqual([o["text"] for o in raw["options"]][:3], ["1", "2", "3"])
        self.assertTrue(all("pad-key" in o["class"] for o in raw["options"]))
        menu = sheet({"screen": "", "keys": KEYS, "menu": {
            "lines": ["Bash command", "git push origin main", "Do you want to proceed?"],
            "options": [{"key": "1", "label": "Yes"}, {"key": "2", "label": "No"}]}})
        self.assertEqual(menu["title"], "Do you want to proceed?")
        self.assertFalse(any("pad-key" in o["class"] for o in menu["options"]))
        self.assertEqual([o["text"] for o in menu["options"]], ["1Yes", "2No", "EscCancel the prompt"])
        # The first choice, the one Claude highlights, is the marked one, and only it.
        self.assertEqual([("first" in o["class"].split()) for o in menu["options"]], [True, False, False])
        self.assertFalse(menu["box"]["atEnd"])


class PromptSheetLayoutTest(unittest.TestCase):
    def test_the_sheet_stays_below_the_status_bar(self):
        sheet_ = rules()[".sheet"]
        self.assertIn("var(--safe-top)", sheet_["max-height"])
        self.assertEqual(sheet_["overflow-y"], "auto")

    def test_the_question_stays_at_the_top_of_the_sheet(self):
        head = rules()[".prompt-head"]
        self.assertEqual((head["position"], head["top"], head["background"]), ("sticky", "0", "var(--card)"))

    def test_the_prompt_body_shrinks_to_the_sheet_and_scrolls(self):
        body = rules()[".sheet-body.prompt"]
        self.assertEqual((body["min-height"], body["overflow-y"]), ("0", "auto"))

    def test_the_screen_box_is_short_and_scrolls_in_itself(self):
        box = rules()[".prompt-box"]
        self.assertLessEqual(number(box["max-height"], "dvh"), 30)
        self.assertEqual(box["overflow-y"], "auto")

    def test_the_choices_are_large(self):
        option = rules()[".answer-option"]
        self.assertGreaterEqual(number(option["min-height"], "px"), 52)
        self.assertGreaterEqual(number(option["font-size"], "px"), 16)


if __name__ == "__main__":
    unittest.main()
