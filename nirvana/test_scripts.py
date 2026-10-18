"""The fork's Python scripts: install puts the build and its stamp where src/nirvana.rs looks; version bumps once.

    python -m unittest nirvana.test_scripts
"""
from __future__ import annotations

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import install  # noqa: E402
import version  # noqa: E402
from test_windows_shell import *  # noqa: E402,F401,F403  plan 12 O4a: the Windows side of install
from test_ab import *  # noqa: E402,F401,F403  plan 12 O7: the A/B driver (nirvana/bench/ab.py)


class InstallTest(unittest.TestCase):
    def test_home_follows_the_same_rule_as_the_binary(self):
        self.assertEqual(install.home({"NIRVANA_HERDR_HOME": r"D:\x"}), Path(r"D:\x"))
        self.assertEqual(install.home({"LOCALAPPDATA": r"C:\L"}), Path(r"C:\L") / "Programs" / "Nirvana-Herdr")

    def test_install_moves_the_old_build_aside_and_stamps_last(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            built = tmp / "built.exe"
            built.write_bytes(b"new")
            root = tmp / "home"
            (root / "bin").mkdir(parents=True)
            (root / "bin" / install.EXE).write_bytes(b"old")
            target = install.install(built, root, "0.1.1")
            self.assertEqual(target.read_bytes(), b"new")
            self.assertEqual((root / "bin" / install.STAMP).read_text(encoding="utf-8"), "0.1.1\n")
            aside = list((root / "bin").glob("herdr.old-*"))
            self.assertEqual([p.read_bytes() for p in aside], [b"old"])
            self.assertFalse((root / "bin" / (install.STAMP + ".tmp")).exists())


class VersionTest(unittest.TestCase):
    def test_bump(self):
        self.assertEqual(version.bumped("0.1.3", "feature"), "0.2.0")
        self.assertEqual(version.bumped("0.1.3", "fix"), "0.1.4")
        with self.assertRaises(ValueError):
            version.bumped("0.1.3", "major")

    def test_version_file_is_one_line(self):
        text = version.FILE.read_bytes()
        self.assertTrue(text.endswith(b"\n") and text.count(b"\n") == 1 and b"\r" not in text, text)
        self.assertEqual(len(text.strip().split(b".")), 3)


if __name__ == "__main__":
    unittest.main()
