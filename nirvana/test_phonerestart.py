"""The phone page's wait after Restart Nirvana (src/nirvana_remote/page/app.js `restarting`), in node. Plan 48.

The owner, 2026-11-25: "Is there a way to restart all of nirvana and add an option to do so". After the phone's
Restart the PC's server stops and a new one starts: every request fails for some seconds, and the live views drop.
The page must say it is restarting, not that the PC is out of reach, and come back by itself once the new server
answers; the old server answering for a moment must not end the wait.

    python -m unittest nirvana.test_phonerestart

nirvana/phonerestart_dump.js loads app.js in a fake window with a small DOM, a fake clock and a fake fetch whose
/api/hello answers each scenario names. No node is a failure, not a skip.
"""
from __future__ import annotations

import json
import subprocess
import unittest
from pathlib import Path

from nirvana.test_phonemd import node

DUMP = Path(__file__).resolve().parent / "phonerestart_dump.js"


def steps(scenario: str) -> dict[str, dict]:
    p = subprocess.run([node(), str(DUMP), scenario], capture_output=True, text=True, encoding="utf-8", timeout=60)
    if p.returncode != 0:
        raise AssertionError(f"node exited {p.returncode}: {p.stderr.strip()[-2000:]}")
    return {step["step"]: step for step in json.loads(p.stdout)}


class PhoneRestartTest(unittest.TestCase):
    def test_the_page_waits_for_the_new_server_and_comes_back_by_itself(self) -> None:
        seen = steps("back")
        self.assertFalse(seen["open"]["waiting"])
        self.assertTrue(seen["asked"]["waiting"], "the wait's screen as soon as the restart is asked")
        first = seen["first ask"]
        self.assertTrue(first["waiting"], "the old server still answering does not end the wait")
        self.assertFalse(first["error"], "a dropped live view and a gateway error are the restart, not the PC lost")
        later = seen["later"]
        self.assertFalse(later["waiting"], "the new server answered: the wait is over")
        self.assertFalse(later["error"])
        self.assertEqual(later["renders"], 2, "the screen is drawn again from the new server")
        self.assertGreater(later["opened"], first["opened"], "its live view opens again at once")

    def test_a_kept_screen_comes_back_as_it_was(self) -> None:
        # The chat keeps the owner's draft: the wait's screen goes, the chat is not drawn again, its live view opens.
        seen = steps("kept")
        later = seen["later"]
        self.assertFalse(later["waiting"], "the wait's screen goes from over a kept screen too")
        self.assertEqual(later["renders"], 1, "kept, not drawn again")
        self.assertGreater(later["opened"], seen["first ask"]["opened"])

    def test_a_server_that_never_comes_back_ends_on_the_error_screen(self) -> None:
        seen = steps("never")
        self.assertTrue(seen["later"]["waiting"])
        self.assertFalse(seen["later"]["error"], "not while the wait lasts")
        gone = seen["past the limit"]
        self.assertFalse(gone["waiting"])
        self.assertTrue(gone["error"], "past the wait's limit the reach check says the PC is not answering")

    def test_the_old_server_answering_never_ends_the_wait(self) -> None:
        seen = steps("old")
        self.assertTrue(seen["end"]["waiting"], "the same server: not back yet")
        self.assertEqual(seen["end"]["renders"], 1)


if __name__ == "__main__":
    unittest.main()
