"""The home-screen page's column after a switch out of the app and back (src/nirvana_remote/page/app.js `fit`), in node.

The owner, 2026-11-16: "switch to nirvana after I switch out on iOS causes this bug". Their screenshot: the prompt
sheet in the top half of the screen, the bottom half empty, the sheet's top under the clock. The column's bottom is
--vv-bottom, the part of the screen under the visual viewport; it was read only on visualViewport events, and iOS sends
none for a keyboard that went while the page was away, so the column kept the keyboard's 336 px.

    python -m unittest nirvana.test_phoneresume

nirvana/phoneresume_dump.js loads app.js in a fake installed-app window, raises the keyboard, hides the page, drops
the keyboard silently, and brings the page back by one way. No node is a failure, not a skip.
"""
from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path

from nirvana.test_phonemd import node

DUMP = Path(__file__).resolve().parent / "phoneresume_dump.js"


def back(way: str) -> list[dict]:
    p = subprocess.run([node(), str(DUMP), way], capture_output=True, text=True, encoding="utf-8", timeout=60)
    if p.returncode != 0:
        raise AssertionError(f"node exited {p.returncode}: {p.stderr.strip()[-2000:]}")
    return json.loads(p.stdout)


KEYBOARD = {"step": "keyboard", "bottom": "336px", "kb": True}
WHOLE = {"step": "back", "bottom": "0px", "kb": False}


class PhoneResumeTest(unittest.TestCase):
    def check(self, way: str) -> None:
        steps = back(way)
        # The keyboard did lift the column: what follows is the return, not a page that never fitted.
        self.assertEqual(steps[1], KEYBOARD)
        self.assertEqual(steps[2], WHOLE, f"back by {way}: the column kept the keyboard's height")

    def test_the_column_is_whole_again_when_the_page_is_shown(self):
        self.check("visible")

    def test_the_column_is_whole_again_out_of_the_page_cache(self):
        self.check("pageshow")

    def test_the_column_is_whole_again_on_a_window_resize(self):
        self.check("resize")

    def test_the_column_is_whole_again_when_the_box_lets_go_of_the_keyboard(self):
        self.check("focusout")

    def test_a_size_ios_reports_a_frame_late_is_read(self):
        self.check("late")

    def test_with_no_way_back_the_driver_shows_the_stale_column(self):
        # The driver's own control: with no event on return, nothing reads the viewport, so the column is stale.
        # If this passed as whole, the other tests could not tell a fix from a driver that never raised the keyboard.
        self.assertEqual(back("none")[2], {"step": "back", "bottom": "336px", "kb": True})


if __name__ == "__main__":
    unittest.main()
