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

    def test_install_copies_both_binaries_and_moves_old_aside(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            built = tmp / install.EXE
            built.write_bytes(b"new-herdr")
            built_open = tmp / install.OPEN_EXE
            built_open.write_bytes(b"new-open")
            root = tmp / "home"
            (root / "bin").mkdir(parents=True)
            (root / "bin" / install.EXE).write_bytes(b"old-herdr")
            (root / "bin" / install.OPEN_EXE).write_bytes(b"old-open")
            target = install.install(built, root, "0.1.1", built_open=built_open)
            self.assertEqual(target.read_bytes(), b"new-herdr")
            self.assertEqual((root / "bin" / install.OPEN_EXE).read_bytes(), b"new-open")
            herdr_aside = list((root / "bin").glob("herdr.old-*"))
            self.assertEqual([p.read_bytes() for p in herdr_aside], [b"old-herdr"])
            open_aside = list((root / "bin").glob("nirvana-open.old-*"))
            self.assertEqual([p.read_bytes() for p in open_aside], [b"old-open"])


    def test_a_failed_copy_puts_the_old_binary_back(self):
        """Plan 40 review B4: a copy that raised after the rename aside left no herdr.exe at all."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            target = tmp / "bin" / install.EXE
            target.parent.mkdir(parents=True)
            target.write_bytes(b"old")
            with self.assertRaises(OSError):
                install.install_binary(tmp / "missing.exe", target, "herdr")
            self.assertTrue(target.is_file(), "no herdr.exe left")
            self.assertEqual(target.read_bytes(), b"old")
            self.assertEqual(list(target.parent.glob("herdr.old-*")), [])


import json  # noqa: E402
import subprocess  # noqa: E402
import time  # noqa: E402
from unittest import mock  # noqa: E402

import clean_temp  # noqa: E402
import controls  # noqa: E402
import gate  # noqa: E402

NIRVANA = Path(__file__).resolve().parent

# A controls run killed while its sabotage is in place: `os._exit` ends the process with no `finally`, as a TaskStop or
# a closed terminal does.
KILLED_RUN = r"""
import json, os, sys
from pathlib import Path
nirvana, scratch = json.loads(sys.argv[1])
sys.path.insert(0, nirvana)
import controls
controls.ROOT = Path(scratch)
controls.RESTORE = Path(scratch) / "target" / "controls-restore"
controls.subprocess.run = lambda *a, **k: os._exit(9)
controls.run_one(controls.Control("t_killed", "f.txt", "good", "bad", "x", "python"))
"""


class ControlsRepairTest(unittest.TestCase):
    """Plan 40 review B6: a killed run cannot leave the source broken or take the broken bytes as the original."""

    def _killed(self, scratch: Path) -> None:
        done = subprocess.run([sys.executable, "-c", KILLED_RUN, json.dumps([str(NIRVANA), str(scratch)])],
                              capture_output=True, timeout=60)
        self.assertEqual(done.returncode, 9, done.stderr)

    def test_the_next_run_repairs_a_killed_runs_sabotage(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            (scratch / "f.txt").write_bytes(b"a good line\n")
            self._killed(scratch)
            self.assertEqual((scratch / "f.txt").read_bytes(), b"a bad line\n", "the kill left the sabotage")
            restore = scratch / "target" / "controls-restore"
            with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore):
                lines = controls.repair()
                self.assertEqual((scratch / "f.txt").read_bytes(), b"a good line\n")
                self.assertEqual(len(lines), 1, lines)
                self.assertIn("restored f.txt", lines[0])
                self.assertEqual(list(restore.iterdir()), [])
                self.assertEqual(controls.repair(), [], "a second repair has nothing to do")

    def test_a_file_edited_after_the_kill_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            (scratch / "f.txt").write_bytes(b"a good line\n")
            self._killed(scratch)
            (scratch / "f.txt").write_bytes(b"the owner's new line\n")
            restore = scratch / "target" / "controls-restore"
            with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore):
                lines = controls.repair()
            self.assertEqual((scratch / "f.txt").read_bytes(), b"the owner's new line\n")
            self.assertIn("changed since a run died", lines[0])
            self.assertEqual((restore / "t_killed.bak").read_bytes(), b"a good line\n", "the backup is kept")

    def test_every_run_starts_with_the_repair(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            (scratch / "f.txt").write_bytes(b"a good line\n")
            self._killed(scratch)
            restore = scratch / "target" / "controls-restore"
            chosen = (controls.Control("t_killed", "f.txt", "good", "bad", "x", "python"),)
            with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore), \
                    mock.patch.object(controls, "CONTROLS", chosen), mock.patch("builtins.print") as printed:
                code = controls.main(["--dry-run", "--only", "t_killed"])
            self.assertEqual((scratch / "f.txt").read_bytes(), b"a good line\n")
            self.assertEqual(code, 0, printed.call_args_list)
            self.assertIn("restored f.txt", str(printed.call_args_list[0]))

    def test_a_clean_run_leaves_no_backup(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            (scratch / "f.txt").write_bytes(b"a good line\n")
            restore = scratch / "target" / "controls-restore"
            fake = mock.Mock(return_value=mock.Mock(stdout="FAIL: x\n", stderr="", returncode=1))
            with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore), \
                    mock.patch.object(controls.subprocess, "run", fake):
                ok, _ = controls.run_one(controls.Control("t_clean", "f.txt", "good", "bad", "x", "python"))
            self.assertTrue(ok)
            self.assertEqual((scratch / "f.txt").read_bytes(), b"a good line\n")
            self.assertEqual(list(restore.iterdir()), [])


NEXTEST_RED = """\
    Starting 3 tests across 1 binary
        PASS [   0.010s] (1/3) herdr::bin/herdr a::ok
        FAIL [   0.020s] (2/3) herdr::bin/herdr nirvana_spaces::tests::broken
        FAIL [   0.030s] (3/3) herdr::bin/herdr client::shell::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override
────────────
     Summary [   0.060s] 3 tests run: 1 passed, 2 failed, 0 skipped
        FAIL [   0.020s] (2/3) herdr::bin/herdr nirvana_spaces::tests::broken
        FAIL [   0.030s] (3/3) herdr::bin/herdr client::shell::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override
"""
NEXTEST_ALLOWED_ONLY = "\n".join(line for line in NEXTEST_RED.splitlines() if "broken" not in line)


class GateTest(unittest.TestCase):
    def _run(self, base: Path, answers: dict, root: Path, **kw) -> tuple[int, list[str], list]:
        seen, out = [], []

        def runner(argv, env, cwd):
            seen.append((argv, env["TEMP"], env["TMP"], Path(env["TEMP"]).is_dir()))
            for key, answer in answers.items():
                if key in " ".join(argv):
                    if isinstance(answer, BaseException):
                        raise answer
                    return answer
            return 0, ""

        code = gate.run(kw.pop("filters", ["nirvana_spaces"]), kw.pop("clippy", False), kw.pop("controls", []),
                        runner=runner, base=base, root=root, pid=4242, out=out.append, **kw)
        return code, out, seen

    def test_each_step_runs_in_a_private_temp_that_is_removed_after(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"
            code, out, seen = self._run(base, {}, root, filters=["nirvana_spaces", "nirvana_remote"], clippy=True,
                                        controls=["c40e_x"])
            self.assertEqual(code, 0, out)
            private = str(base / "ng-4242")
            self.assertEqual([s[0][:3] for s in seen],
                             [["cargo", "nextest", "run"], ["cargo", "nextest", "run"], ["cargo", "clippy", "--all-targets"],
                              [sys.executable, str(gate.ROOT / "nirvana" / "controls.py"), "--only"]])
            self.assertEqual(seen[0][0][-2:], ["-E", "test(/nirvana_spaces/)"])
            self.assertEqual(seen[3][0][-1], "c40e_x")
            self.assertTrue(all(s[1] == private and s[2] == private and s[3] for s in seen), seen)
            self.assertFalse((base / "ng-4242").exists())
            self.assertEqual(out[-1], "gate: 4 of 4 steps passed")

    def test_failed_tests_are_named_and_the_allowed_one_is_apart(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"
            code, out, _ = self._run(base, {"nextest": (100, NEXTEST_RED)}, root)
            self.assertEqual(code, 1)
            self.assertTrue(out[0].startswith("FAIL  nextest nirvana_spaces: exit 100; failed: "
                                              "nirvana_spaces::tests::broken; log "), out)
            self.assertIn("ALLOWED  nextest nirvana_spaces: client::shell::tests::"
                          "windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override", out[1])
            code, out, _ = self._run(base, {"nextest": (100, NEXTEST_ALLOWED_ONLY)}, root)
            self.assertEqual(code, 0, out)
            self.assertEqual(out[0], "PASS  nextest nirvana_spaces: exit 100")
            self.assertEqual(out[-1], "gate: 1 of 1 steps passed")

    def test_a_build_error_is_a_failure_with_no_test_named(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"
            code, out, _ = self._run(base, {"nextest": (101, "error[E0425]: cannot find value")}, root)
            self.assertEqual(code, 1)
            self.assertIn("no test named", out[0])

    def test_an_interrupted_run_still_removes_its_temp(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"

            def fills(argv, env, cwd):
                (Path(env["TEMP"]) / "herdr-x-1").mkdir()
                raise KeyboardInterrupt

            with self.assertRaises(KeyboardInterrupt):
                gate.run(["nirvana_spaces"], False, [], runner=fills, base=base, root=root, pid=4243,
                         out=lambda _line: None)
            self.assertFalse((base / "ng-4243").exists())

    def test_stale_private_temps_go_only_when_old_and_dead(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            now = time.time()
            for name in ("ng-11", "ng-12", "ng-13", "ng-x"):
                (base / name / "sub").mkdir(parents=True)
                os.utime(base / name, (now - 2 * 86400, now - 2 * 86400))
            (base / "ng-14").mkdir()       # young, its pid dead
            lines = gate.sweep_stale(base, now, alive=lambda pid: pid == 12)
            self.assertEqual(sorted(p.name for p in base.iterdir()), ["ng-12", "ng-14", "ng-x"])
            self.assertEqual(sorted(lines), ["removed a stale ng-11", "removed a stale ng-13"])


def _junction(link: Path, target: Path) -> None:
    if os.name == "nt":
        import _winapi
        _winapi.CreateJunction(str(target), str(link))
    else:
        os.symlink(target, link, target_is_directory=True)


class CleanTempTest(unittest.TestCase):
    """The backlog cleaner on a scratch folder: what goes, what stays and why."""

    def _world(self, tmp: Path) -> tuple[Path, Path, float]:
        root, outside = tmp / "Temp", tmp / "outside"
        root.mkdir()
        outside.mkdir()
        (outside / "keep.txt").write_bytes(b"the owner's file")
        now = time.time()
        old = now - 72 * 3600

        def folder(name: str, at: float = old) -> Path:
            path = root / name
            (path / "sub").mkdir(parents=True)
            (path / "sub" / "f.bin").write_bytes(b"x" * 100)
            os.utime(path, (at, at))
            return path

        folder("herdr-o5-gate-owned-42660-1792463775117999200")
        folder("nirvana-jev-guards-26392")
        folder("nirvana-claude-stale-777", at=now)               # young
        folder("nirvana-alive-31337")                            # its pid runs
        folder("nirvana-p13a-52548-0-owner")                     # no pid at the end
        folder("other-thing-123")                                # not ours
        linked = folder("herdr-linked-555")
        _junction(linked / "out", outside)                       # a junction inside points outside
        os.utime(linked, (old, old))
        _junction(root / "herdr-top-link-556", outside)          # a junction at the top
        (root / "nirvana-file-557").write_bytes(b"x")            # a file, not a folder
        return root, outside, now

    def test_the_dry_run_removes_nothing_and_says_why_each_stays(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, outside, now = self._world(Path(tmp))
            before = sorted(p.name for p in root.iterdir())
            found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
            why = {f.name: f.why for f in found}
            self.assertEqual(why, {
                "herdr-o5-gate-owned-42660-1792463775117999200": "removable",
                "nirvana-jev-guards-26392": "removable",
                "herdr-linked-555": "removable",
                "nirvana-claude-stale-777": "younger than 48 h",
                "nirvana-alive-31337": "pid 31337 still runs",
                "nirvana-p13a-52548-0-owner": "its name does not end in a pid",
                "herdr-top-link-556": "a link or junction: left as it is",
                "nirvana-file-557": "not a folder",
            })
            self.assertEqual({f.name: f.size for f in found}["nirvana-jev-guards-26392"], 100)
            self.assertEqual({f.name: f.pattern for f in found}["herdr-o5-gate-owned-42660-1792463775117999200"],
                             "herdr-o5-gate-owned-<pid>-<ts>")
            text = "\n".join(clean_temp.summary(found, 10))
            self.assertIn("8 herdr-/nirvana- entries", text)
            self.assertIn("3 removable (", text)
            self.assertEqual(sorted(p.name for p in root.iterdir()), before)

    def test_apply_removes_only_the_removable_and_never_through_a_junction(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, outside, now = self._world(Path(tmp))
            found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
            lines, removed, size = clean_temp.apply(found)
            self.assertEqual((lines, removed), ([], 3))
            self.assertEqual(size, 300)
            self.assertEqual(sorted(p.name for p in root.iterdir()), [
                "herdr-top-link-556", "nirvana-alive-31337", "nirvana-claude-stale-777", "nirvana-file-557",
                "nirvana-p13a-52548-0-owner", "other-thing-123"])
            self.assertTrue((outside / "keep.txt").is_file(), "the junction was followed")
            self.assertEqual((outside / "keep.txt").read_bytes(), b"the owner's file")
            self.assertTrue((root / "herdr-top-link-556" / "keep.txt").is_file())

    @unittest.skipUnless(os.name == "nt", "Windows refuses to rename a folder with an open file")
    def test_a_folder_in_use_is_skipped_whole(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, _, now = self._world(Path(tmp))
            held = root / "nirvana-jev-guards-26392" / "sub" / "f.bin"
            with open(held, "rb"):
                found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
                lines, removed, _ = clean_temp.apply(found)
            self.assertEqual(removed, 2)
            self.assertEqual(len(lines), 1, lines)
            self.assertTrue(lines[0].startswith("skipped nirvana-jev-guards-26392: in use"), lines)
            self.assertEqual(held.read_bytes(), b"x" * 100, "nothing of it was removed")

    def test_main_is_a_dry_run_by_default(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, _, _ = self._world(Path(tmp))
            before = sorted(p.name for p in root.iterdir())
            with mock.patch("builtins.print"):
                self.assertEqual(clean_temp.main(["--root", str(root)], alive=lambda pid: False), 0)
            self.assertEqual(sorted(p.name for p in root.iterdir()), before)


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
