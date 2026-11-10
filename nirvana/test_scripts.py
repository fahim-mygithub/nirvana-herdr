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

    def test_a_stale_plugin_folder_finishes_the_install_and_exits_non_zero(self):
        """Review N5: an overlay that left old files named nothing and exited 0."""
        from unittest import mock
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            release = tmp / "target" / "release"
            release.mkdir(parents=True)
            (release / install.EXE).write_bytes(b"new")
            (release / install.OPEN_EXE).write_bytes(b"new")
            held = tmp / "home" / "nirvana" / "claude" / "hooks" / "hooks.json"
            stale = install.windows_shell.StaleFiles([held])
            with mock.patch.object(install, "ROOT", tmp), mock.patch.object(install, "home", lambda: tmp / "home"), \
                    mock.patch.object(install, "version", lambda: "0.0.1"), \
                    mock.patch.object(install, "install", mock.Mock(return_value=tmp / "home" / "bin")) as put, \
                    mock.patch.object(install.windows_shell, "install_plugin", mock.Mock(side_effect=stale)), \
                    mock.patch("builtins.print") as printed:
                code = install.main(["--no-build", "--no-shell"])
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list)
            self.assertEqual(code, 1, text)
            put.assert_called_once()
            self.assertIn(f"stale {held}", text)
            self.assertIn("1 plugin files are stale", text)


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


# A run that holds the worktree's lock until told to die; it prints its own pid (a venv's python.exe may be a launcher
# whose pid is not the interpreter's).
HOLDER = r"""
import json, os, sys
from pathlib import Path
nirvana, scratch = json.loads(sys.argv[1])
sys.path.insert(0, nirvana)
import controls
controls.ROOT = Path(scratch)
controls.RESTORE = Path(scratch) / "target" / "controls-restore"
fd, _ = controls.take_lock()
print(os.getpid() if fd is not None else "refused", flush=True)
sys.stdin.readline()
os._exit(0)
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

    def test_a_second_run_in_the_worktree_is_refused_and_touches_nothing(self):
        """Review S2: two live runs undid each other's restore. A second run says who holds the worktree, exits 2 and
        touches no file, not even the repair; once the holder is gone its lock is taken over after the repair."""
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            (scratch / "f.txt").write_bytes(b"a good line\n")
            self._killed(scratch)
            restore = scratch / "target" / "controls-restore"
            holder = subprocess.Popen([sys.executable, "-c", HOLDER, json.dumps([str(NIRVANA), str(scratch)])],
                                      stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            try:
                pid = holder.stdout.readline().strip()
                self.assertTrue(pid.isdigit(), pid)
                chosen = (controls.Control("t_killed", "f.txt", "good", "bad", "x", "python"),)
                with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore), \
                        mock.patch.object(controls, "CONTROLS", chosen), mock.patch("builtins.print") as printed:
                    code = controls.main(["--dry-run", "--only", "t_killed"])
                text = str(printed.call_args_list)
                self.assertEqual(code, 2, text)
                self.assertIn(f"another controls run holds this worktree: pid {pid}, since ", text)
                self.assertEqual((scratch / "f.txt").read_bytes(), b"a bad line\n", "the refused run touched a file")
            finally:
                holder.communicate("\n", timeout=30)
            with mock.patch.object(controls, "ROOT", scratch), mock.patch.object(controls, "RESTORE", restore), \
                    mock.patch.object(controls, "CONTROLS", chosen), mock.patch("builtins.print") as printed:
                code = controls.main(["--dry-run", "--only", "t_killed"])
            lines = [str(c.args[0]) for c in printed.call_args_list]
            self.assertEqual(code, 0, lines)
            self.assertIn("restored f.txt", lines[0])
            self.assertTrue(lines[1].startswith(f"controls: took over the run lock of pid {pid}, since "), lines)
            self.assertEqual((scratch / "f.txt").read_bytes(), b"a good line\n")
            self.assertEqual(sorted(p.name for p in restore.iterdir()), ["run.lock"], "the note went with the run")

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
        FAIL [   0.030s] (3/3) herdr::bin/herdr app::api::plugins::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override
────────────
     Summary [   0.060s] 3 tests run: 1 passed, 2 failed, 0 skipped
        FAIL [   0.020s] (2/3) herdr::bin/herdr nirvana_spaces::tests::broken
        FAIL [   0.030s] (3/3) herdr::bin/herdr app::api::plugins::tests::windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override
"""
NEXTEST_ALLOWED_ONLY = "\n".join(line for line in NEXTEST_RED.splitlines() if "broken" not in line)


# A step that starts a grandchild; both write their pid, the grandchild holds a file open in TEMP, both sleep.
TREE_GRANDCHILD = ("import os, sys, time; f = open(os.path.join(os.environ['TEMP'], 'held.bin'), 'wb'); "
                   "open(sys.argv[1], 'a').write(f'{os.getpid()}\\n'); time.sleep(120)")
TREE_CHILD = ("import os, subprocess, sys, time; "
              f"subprocess.Popen([sys.executable, '-c', {TREE_GRANDCHILD!r}, sys.argv[1]]); "
              "open(sys.argv[1], 'a').write(f'{os.getpid()}\\n'); time.sleep(120)")
# A gate process that runs that step and is then killed from outside.
KILLED_GATE = r"""
import json, os, sys
from pathlib import Path
nirvana, child, pids = json.loads(sys.argv[1])
sys.path.insert(0, nirvana)
import gate
gate.run_child([sys.executable, "-c", child, pids], dict(os.environ), Path(nirvana))
"""


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
            private = seen[0][1]
            self.assertRegex(Path(private).name, r"^ng-4242-[a-z0-9_]{8}$")
            self.assertEqual(Path(private).parent, base)
            self.assertEqual([s[0][:3] for s in seen],
                             [["cargo", "nextest", "run"], ["cargo", "nextest", "run"], ["cargo", "clippy", "--all-targets"],
                              [sys.executable, str(gate.ROOT / "nirvana" / "controls.py"), "--only"]])
            self.assertEqual(seen[0][0][-2:], ["-E", "test(/nirvana_spaces/)"])
            self.assertEqual(seen[3][0][-1], "c40e_x")
            self.assertTrue(all(s[1] == private and s[2] == private and s[3] for s in seen), seen)
            self.assertFalse(os.path.lexists(private))
            self.assertEqual(list(base.iterdir()), [])
            self.assertEqual(out[-1], "gate: 4 of 4 steps passed")

    def test_failed_tests_are_named_and_the_allowed_one_is_apart(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"
            code, out, _ = self._run(base, {"nextest": (100, NEXTEST_RED)}, root)
            self.assertEqual(code, 1)
            self.assertTrue(out[0].startswith("FAIL  nextest nirvana_spaces: exit 100; failed: "
                                              "nirvana_spaces::tests::broken; log "), out)
            self.assertIn("ALLOWED  nextest nirvana_spaces: app::api::plugins::tests::"
                          "windows_plugin_pane_commands_resolve_from_plugin_root_with_cwd_override", out[1])
            code, out, _ = self._run(base, {"nextest": (100, NEXTEST_ALLOWED_ONLY)}, root)
            self.assertEqual(code, 0, out)
            self.assertEqual(out[0], "PASS  nextest nirvana_spaces: exit 100")
            self.assertEqual(out[-1], "gate: 1 of 1 steps passed")
            # Review N3: the same name in another module is a real failure.
            other = NEXTEST_ALLOWED_ONLY.replace("app::api::plugins::tests::", "nirvana_spaces::tests::")
            code, out, _ = self._run(base, {"nextest": (100, other)}, root)
            self.assertEqual(code, 1, out)
            self.assertIn("failed: nirvana_spaces::tests::windows_plugin_pane_commands", out[0])

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
            self.assertEqual(list(base.iterdir()), [])

    def test_a_planted_junction_at_the_private_name_is_never_used_or_emptied(self):
        """Review S1: `mkdir(exist_ok=True)` took an existing `ng-<pid>`, a junction included, and the `finally`
        emptied its target. A real junction, made by `mklink /J`."""
        with tempfile.TemporaryDirectory() as tmp:
            base, root, outside = Path(tmp) / "Temp", Path(tmp) / "repo", Path(tmp) / "outside"
            base.mkdir()
            outside.mkdir()
            (outside / "keep.txt").write_bytes(b"the owner's file")
            _junction(base / "ng-4242", outside)
            code, out, seen = self._run(base, {}, root)
            self.assertEqual(code, 0, out)
            self.assertNotEqual(Path(seen[0][1]), base / "ng-4242", "the run took the planted name")
            _assert_kept(self, (outside / "keep.txt"))
            self.assertTrue(clean_temp.is_link(base / "ng-4242"))

    def test_a_private_temp_swapped_for_a_junction_mid_run_is_not_emptied(self):
        with tempfile.TemporaryDirectory() as tmp:
            base, root, outside = Path(tmp) / "Temp", Path(tmp) / "repo", Path(tmp) / "outside"
            outside.mkdir()
            (outside / "keep.txt").write_bytes(b"the owner's file")
            out: list[str] = []

            def swaps(argv, env, cwd):
                os.rmdir(env["TEMP"])
                _junction(Path(env["TEMP"]), outside)
                return 0, ""

            gate.run(["nirvana_spaces"], False, [], runner=swaps, base=base, root=root, pid=4244, out=out.append)
            _assert_kept(self, (outside / "keep.txt"))
            self.assertTrue(any("a link or junction, not followed" in line for line in out), out)
            for link in base.iterdir():
                os.rmdir(link)

    @unittest.skipUnless(os.name == "nt", "the job object is the Windows way; elsewhere a process group")
    def test_an_interrupt_ends_the_whole_tree_before_the_temp_goes(self):
        """Review N2: on an interrupt only `cargo` died; its children kept writing into the folder being removed.
        The step starts a grandchild that holds a file open in the private TEMP; a Ctrl-C (`interrupt_main`) must
        end both before the `finally`, or the held file stops the removal."""
        import _thread
        import threading
        # A tree left running holds a file in the folder: the cleanup must not turn the red line into an ERROR.
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            base, root = Path(tmp) / "Temp", Path(tmp) / "repo"
            root.mkdir()
            pids = Path(tmp) / "pids.txt"

            def step(argv, env, cwd):
                return gate.run_child([sys.executable, "-c", TREE_CHILD, str(pids)], env, cwd)

            def interrupt_when_both_run():
                deadline = time.monotonic() + 60
                while time.monotonic() < deadline:
                    if pids.exists() and len(pids.read_text().split()) == 2 and (base.exists() and any(
                            (p / "held.bin").exists() for p in base.iterdir())):
                        break
                    time.sleep(0.1)
                _thread.interrupt_main()

            out: list[str] = []
            threading.Thread(target=interrupt_when_both_run, daemon=True).start()
            started = time.monotonic()
            with self.assertRaises(KeyboardInterrupt):
                gate.run(["x"], False, [], runner=step, base=base, root=root, pid=4245, out=out.append)
            self.assertLess(time.monotonic() - started, 60, "the run waited for the tree instead of ending it")
            alive = [int(p) for p in pids.read_text().split() if clean_temp.pid_alive(int(p))]
            self.assertEqual(alive, [], f"left running: {alive}")
            self.assertEqual(len(pids.read_text().split()), 2)
            self.assertEqual(list(base.iterdir()), [], out)

    @unittest.skipUnless(os.name == "nt", "the job object is the Windows way; elsewhere a process group")
    def test_a_killed_gate_takes_its_tree_with_it(self):
        """Review N2: a killed gate runs no `finally`; the job's last handle closes with it and ends the tree."""
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            pids = Path(tmp) / "pids.txt"
            private = Path(tmp) / "private"
            private.mkdir()
            env = dict(os.environ, TEMP=str(private), TMP=str(private))
            gate_proc = subprocess.Popen([sys.executable, "-c", KILLED_GATE,
                                          json.dumps([str(NIRVANA), TREE_CHILD, str(pids)])], env=env)
            try:
                deadline = time.monotonic() + 60
                while time.monotonic() < deadline and not (pids.exists() and len(pids.read_text().split()) == 2):
                    time.sleep(0.1)
                self.assertEqual(len(pids.read_text().split()), 2)
            finally:
                gate_proc.kill()
                gate_proc.wait(timeout=30)
            tree = [int(p) for p in pids.read_text().split()]
            deadline = time.monotonic() + 10
            while time.monotonic() < deadline and any(clean_temp.pid_alive(p) for p in tree):
                time.sleep(0.1)
            self.assertEqual([p for p in tree if clean_temp.pid_alive(p)], [], "the tree outlived its gate")

    def test_stale_private_temps_go_only_when_old_and_dead(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            now = time.time()
            for name in ("ng-11", "ng-12", "ng-13", "ng-x"):
                (base / name / "sub").mkdir(parents=True)
                os.utime(base / name, (now - 2 * 86400, now - 2 * 86400))
            (base / "ng-14").mkdir()       # young, its pid dead
            (base / "ng-15-abcd_123").mkdir()
            os.utime(base / "ng-15-abcd_123", (now - 2 * 86400, now - 2 * 86400))
            lines = gate.sweep_stale(base, now, alive=lambda pid: pid == 12)
            self.assertEqual(sorted(p.name for p in base.iterdir()), ["ng-12", "ng-14", "ng-x"])
            self.assertEqual(sorted(lines), ["removed a stale ng-11", "removed a stale ng-13",
                                             "removed a stale ng-15-abcd_123"])

    def test_the_stale_sweep_leaves_a_junction_at_the_top_as_it_is(self):
        """Review S1: the sweep's own top-level link rule; `now` three days on, so age does not keep it."""
        with tempfile.TemporaryDirectory() as tmp:
            base, outside = Path(tmp) / "Temp", Path(tmp) / "outside"
            base.mkdir()
            (outside / "sub").mkdir(parents=True)
            (outside / "sub" / "keep.txt").write_bytes(b"the owner's file")
            _junction(base / "ng-16", outside)
            lines = gate.sweep_stale(base, time.time() + 3 * 86400, alive=lambda pid: False)
            self.assertEqual(lines, [])
            _assert_kept(self, (outside / "sub" / "keep.txt"))
            self.assertTrue(clean_temp.is_link(base / "ng-16"))
            os.rmdir(base / "ng-16")


def _assert_kept(case: unittest.TestCase, path: Path) -> None:
    """A FAIL, not an ERROR, when a link was followed and the owner's file is gone (a control reads `FAIL:`)."""
    case.assertTrue(path.is_file(), f"{path} is gone: a link was followed")
    case.assertEqual(path.read_bytes(), b"the owner's file")


def _junction(link: Path, target: Path) -> None:
    """A real junction, as the owner's tools make one: `mklink /J` through `cmd` (no privilege needed)."""
    if os.name == "nt":
        done = subprocess.run(["cmd", "/C", "mklink", "/J", str(link), str(target)], capture_output=True, text=True)
        assert done.returncode == 0, done
        assert clean_temp.is_link(link), link
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
        folder("nirvana-p13a-52548-0-owner")                     # scratch() in nirvana_preset/tests.rs
        folder("nirvana-p13a-31337-1-owner")                     # the same, its pid runs
        folder("nirvana-fake-daemon-k2_9xq0z")                   # old Nirvana's fake daemon, mkdtemp's suffix
        folder("nirvana-fake-daemon-a1b2c3d4", at=now)           # the same, young
        folder("nirvana-notes")                                  # no pid, no named rule
        folder("nirvana-notes-2024-draft")                       # digits, but not at the end
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
                "nirvana-p13a-52548-0-owner": "removable",
                "nirvana-p13a-31337-1-owner": "pid 31337 still runs",
                "nirvana-fake-daemon-k2_9xq0z": "removable",
                "nirvana-fake-daemon-a1b2c3d4": "younger than 48 h",
                "nirvana-notes": "its name does not end in a pid",
                "nirvana-notes-2024-draft": "its name does not end in a pid",
                "herdr-top-link-556": "a link or junction: left as it is",
                "nirvana-file-557": "not a folder",
            })
            self.assertEqual({f.name: f.size for f in found}["nirvana-jev-guards-26392"], 100)
            self.assertEqual({f.name: f.pattern for f in found}["herdr-o5-gate-owned-42660-1792463775117999200"],
                             "herdr-o5-gate-owned-<pid>-<ts>")
            patterns = {f.name: f.pattern for f in found}
            self.assertEqual(patterns["nirvana-p13a-52548-0-owner"], "nirvana-p13a-<pid>-<n>-<name>")
            self.assertEqual(patterns["nirvana-fake-daemon-k2_9xq0z"], "nirvana-fake-daemon-<random>")
            text = "\n".join(clean_temp.summary(found, 10))
            self.assertIn("13 herdr-/nirvana- entries", text)
            self.assertIn("5 removable (", text)
            self.assertEqual(sorted(p.name for p in root.iterdir()), before)

    def test_apply_removes_only_the_removable_and_never_through_a_junction(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, outside, now = self._world(Path(tmp))
            found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
            lines, removed, size = clean_temp.apply(found)
            self.assertEqual((lines, removed), ([], 5))
            self.assertEqual(size, 500)
            self.assertEqual(sorted(p.name for p in root.iterdir()), [
                "herdr-top-link-556", "nirvana-alive-31337", "nirvana-claude-stale-777", "nirvana-fake-daemon-a1b2c3d4",
                "nirvana-file-557", "nirvana-notes", "nirvana-notes-2024-draft", "nirvana-p13a-31337-1-owner",
                "other-thing-123"])
            self.assertTrue((outside / "keep.txt").is_file(), "the junction was followed")
            _assert_kept(self, (outside / "keep.txt"))
            self.assertTrue((root / "herdr-top-link-556" / "keep.txt").is_file())

    @unittest.skipUnless(os.name == "nt", "Windows refuses to rename a folder with an open file")
    def test_a_folder_in_use_is_skipped_whole(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, _, now = self._world(Path(tmp))
            held = root / "nirvana-jev-guards-26392" / "sub" / "f.bin"
            with open(held, "rb"):
                found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
                lines, removed, _ = clean_temp.apply(found)
            self.assertEqual(removed, 4)
            self.assertEqual(len(lines), 1, lines)
            self.assertTrue(lines[0].startswith("skipped nirvana-jev-guards-26392: in use"), lines)
            self.assertEqual(held.read_bytes(), b"x" * 100, "nothing of it was removed")

    def test_the_named_patterns_take_only_their_own_shape(self):
        """The two named makers; each keeps the age, pid, link and top-level rules."""
        for name, ruled in (("nirvana-fake-daemon-abc_1234", True), ("nirvana-fake-daemon-abc123", False),
                            ("nirvana-fake-daemon-ABCD1234", False), ("nirvana-fake-daemon-abcd1234-x", False),
                            ("nirvana-fake-daemon-abcd1234.cleaning-88", True),
                            ("nirvana-p13a-52548-0-owner", True), ("nirvana-p13a-52548-12-tag_cap-x", True),
                            ("nirvana-p13a-x-0-owner", False), ("nirvana-p13a-52548-owner", False),
                            ("nirvana-p13a-launch-52548-owner", False)):
            self.assertEqual(any(rule.match(name) for _, rule in clean_temp.NAMED), ruled, name)

    def test_remove_tree_refuses_a_junction_at_its_top(self):
        """Review S1: the check is made where the delete is, not only in the scan minutes before."""
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / "outside" / "sub").mkdir(parents=True)
            (tmp / "outside" / "sub" / "keep.txt").write_bytes(b"the owner's file")
            _junction(tmp / "herdr-x-9", tmp / "outside")
            failures = clean_temp.remove_tree(tmp / "herdr-x-9")
            self.assertEqual(len(failures), 1, failures)
            self.assertIn("a link or junction, not followed", failures[0])
            _assert_kept(self, (tmp / "outside" / "sub" / "keep.txt"))
            self.assertTrue(clean_temp.is_link(tmp / "herdr-x-9"))
            os.rmdir(tmp / "herdr-x-9")

    def test_a_name_swapped_for_a_junction_after_the_scan_is_left(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, outside, now = self._world(Path(tmp))
            found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
            swapped = root / "nirvana-jev-guards-26392"
            clean_temp.remove_tree(swapped)
            _junction(swapped, outside)
            lines, removed, _ = clean_temp.apply(found)
            self.assertEqual(removed, 4)
            self.assertEqual(lines, ["skipped nirvana-jev-guards-26392: became a link or junction since the scan: "
                                     "left as it is"])
            _assert_kept(self, (outside / "keep.txt"))
            self.assertTrue(clean_temp.is_link(swapped))

    def test_a_name_swapped_between_the_check_and_the_rename_is_renamed_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            root, outside, now = self._world(Path(tmp))
            found = clean_temp.scan(root, now, 48, alive=lambda pid: pid == 31337)
            swapped = root / "nirvana-jev-guards-26392"
            real = os.rename

            def swap_then_rename(a, b):
                if Path(a) == swapped:
                    clean_temp.remove_tree(swapped)
                    _junction(swapped, outside)
                return real(a, b)

            with mock.patch.object(clean_temp.os, "rename", swap_then_rename):
                lines, removed, _ = clean_temp.apply(found)
            self.assertEqual(removed, 4)
            self.assertEqual(lines, ["skipped nirvana-jev-guards-26392: a link or junction after the rename: renamed "
                                     "back, left as it is"])
            _assert_kept(self, (outside / "keep.txt"))
            self.assertTrue(clean_temp.is_link(swapped))

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
