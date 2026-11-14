"""Plan 41 P41c: the agy orchestrator's plugin, built from `nirvana/claude/` and installed with
`agy plugin install` (agy_plugin.py, install.py).

    python -m unittest nirvana.test_agy_plugin

The tests use literals, not the module's constants (`--host agy`, `always_on`, the tool names, `nirvana`): a test
that read a constant back would pass any sabotage of it. The live check that agy loads the plugin is in the P41c
record, not here. The one test that runs agy's own validator skips, and says so, where agy is not on PATH.

No test here may reach the real `agy plugin install`, which writes under `~/.gemini`. Every test that goes through
`agy_plugin.install` or `install.main` runs with PATH set to a folder holding only a fake `agy`, or no agy at all
(`FakeAgy`). Each such test checks that `agy` resolves into that folder before it installs anything.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import agy_plugin  # noqa: E402
import install  # noqa: E402

SRC = Path(__file__).resolve().parent / "claude"


def claude_hooks() -> dict:
    return json.loads((SRC / "hooks" / "hooks.json").read_bytes())


def copy_src(tmp: Path) -> Path:
    src = tmp / "claude"
    shutil.copytree(SRC, src)
    return src


# The fake: it logs each call (argv, whether stdin is a terminal, NO_COLOR) as a JSON line and answers like agy
# 1.5.9. The validate answer is `--json`'s shape. The install answer is the summary format strings read out of
# agy.exe. FAKE_AGY_MODE `validate-fails` or `install-fails` makes that call exit 1, as agy does.
FAKE_AGY = r"""import json, os, sys
args = sys.argv[1:]
with open(os.environ["FAKE_AGY_LOG"], "a", encoding="utf-8") as log:
    log.write(json.dumps({"argv": args, "stdin_tty": sys.stdin.isatty(), "stdin": sys.stdin.read(),
                          "no_color": os.environ.get("NO_COLOR")}) + "\n")
mode = os.environ.get("FAKE_AGY_MODE", "")
if args[:2] == ["plugin", "validate"]:
    bad = mode == "validate-fails"
    print(json.dumps({"findings": [{"severity": "error", "message": "canned finding"}] if bad else [],
                      "components": {"skills": 8}}))
    sys.exit(1 if bad else 0)
if args[:2] == ["plugin", "install"]:
    if mode == "install-fails":
        print("Error: canned install failure", file=sys.stderr)
        sys.exit(1)
    print('Installed plugin "nirvana" from ' + args[2])
    print("          Installed to C:/fake-gemini/config/plugins/nirvana (replaced existing installation)")
    sys.exit(0)
sys.exit(9)
"""


class FakeAgy:
    """PATH holding only a fake `agy` (`with_agy=True`) or nothing, for the length of a `with`. The real agy, and
    bladebro, cannot be found while it holds."""

    def __init__(self, tmp: Path, mode: str = "", with_agy: bool = True):
        self.bin = tmp / "fake-bin"
        self.bin.mkdir(parents=True, exist_ok=True)
        self.log = tmp / "fake-agy.jsonl"
        if with_agy:
            script = self.bin / "fake_agy.py"
            script.write_bytes(FAKE_AGY.encode("utf-8"))
            if os.name == "nt":
                (self.bin / "agy.cmd").write_bytes(f'@"{sys.executable}" "{script}" %*\r\n'.encode("utf-8"))
            else:
                exe = self.bin / "agy"
                exe.write_bytes(f'#!/bin/sh\nexec "{sys.executable}" "{script}" "$@"\n'.encode("utf-8"))
                exe.chmod(0o755)
        self._env = mock.patch.dict(os.environ, {"PATH": str(self.bin), "FAKE_AGY_LOG": str(self.log),
                                                 "FAKE_AGY_MODE": mode})
        self.with_agy = with_agy

    def __enter__(self):
        self._env.start()
        found = shutil.which("agy")
        if self.with_agy:
            assert found and Path(found).parent == self.bin, f"agy resolves to {found}, not the fake in {self.bin}"
        else:
            assert found is None, f"agy resolves to {found} with PATH emptied"
        return self

    def __exit__(self, *exc):
        self._env.stop()

    def calls(self) -> list[dict]:
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text(encoding="utf-8").splitlines()]


def run_install_main(tmp: Path, home: Path):
    """`install.main --no-build --no-shell` with the binary copy and the Claude plugin stubbed. The real
    `agy_plugin.install` runs, so the agy step is the one under test."""
    release = tmp / "target" / "release"
    release.mkdir(parents=True, exist_ok=True)
    (release / install.EXE).write_bytes(b"new")
    (release / install.OPEN_EXE).write_bytes(b"new")
    with mock.patch.object(install, "ROOT", tmp), mock.patch.object(install, "home", lambda: home), \
            mock.patch.object(install, "version", lambda: "0.0.1"), \
            mock.patch.object(install, "install", mock.Mock(return_value=home / "bin")) as put, \
            mock.patch.object(install.windows_shell, "install_plugin", mock.Mock(return_value=home)), \
            mock.patch("builtins.print") as printed:
        code = install.main(["--no-build", "--no-shell"])
    text = "\n".join(str(c.args[0]) for c in printed.call_args_list)
    return code, text, put


class AgyBuilt(unittest.TestCase):
    """One build into a temp install, read by each test."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.home = self.tmp / "home"
        self.dot = self.tmp / "out" / ".agents"
        self.report = agy_plugin.build(self.dot, self.home, with_bladebro=True)
        self.hooks = json.loads((self.dot / "hooks.json").read_bytes())
        self.exe = str(self.home / "bin" / "herdr.exe")

    def tearDown(self):
        self._tmp.cleanup()

    def handlers(self, event: str) -> list[dict]:
        out = []
        for spec in self.hooks.values():
            for item in spec.get(event, []):
                out.extend(item["hooks"] if "hooks" in item else [item])
        return out


class AgyHooksTest(AgyBuilt):
    def test_every_claude_hook_runs_the_installed_herdr_with_host_agy_on_its_agy_event(self):
        want = {"SessionStart": "SessionStart", "UserPromptSubmit": "PreInvocation", "PreToolUse": "PreToolUse",
                "PostToolUse": "PostToolUse", "Stop": "Stop"}
        n = 0
        for event, groups in claude_hooks()["hooks"].items():
            for group in groups:
                for hook in group["hooks"]:
                    line = subprocess.list2cmdline([self.exe, *hook["args"], "--host", "agy"])
                    if event == "SubagentStop":
                        self.assertNotIn(line, json.dumps(self.hooks))
                        continue
                    got = [h for h in self.handlers(want[event]) if h["command"] == line]
                    self.assertEqual(len(got), 1, f"{event} {hook['args']}: {line}")
                    self.assertEqual(got[0]["timeout"], hook["timeout"])
                    self.assertEqual(got[0]["type"], "command")
                    n += 1
        self.assertEqual(n, sum(len(self.handlers(e)) for e in want.values()))
        self.assertTrue(any(s.startswith("hook SubagentStop") for s in self.report["skipped"]), self.report)

    def test_flat_events_are_handler_lists_and_tool_events_are_matcher_groups(self):
        for spec in self.hooks.values():
            for event, items in spec.items():
                for item in items:
                    if event in ("SessionStart", "PreInvocation", "Stop"):
                        self.assertEqual(set(item), {"type", "command", "timeout"}, item)
                    else:
                        self.assertEqual(set(item), {"matcher", "hooks"}, item)
        self.assertEqual(self.handlers("UserPromptSubmit"), [])

    def test_claude_tool_matchers_become_agy_tool_names(self):
        groups = {}
        for spec in self.hooks.values():
            for event in ("PreToolUse", "PostToolUse"):
                for g in spec.get(event, []):
                    for h in g["hooks"]:
                        groups[(event, h["command"].split()[2])] = g["matcher"].split("|")
        # Measured in the P41c live check (agy 1.5.9, hook payload toolCall.name).
        self.assertIn("write_file", groups[("PreToolUse", "guard-edit")])
        self.assertIn("edit_file", groups[("PreToolUse", "guard-edit")])
        self.assertIn("read_file", groups[("PreToolUse", "guard-read")])
        self.assertEqual(groups[("PreToolUse", "guard-spawn")], ["invoke_subagent"])
        self.assertIn("run_command", groups[("PostToolUse", "check-edit")])
        self.assertIn("edit_file", groups[("PostToolUse", "check-edit")])
        for names in groups.values():
            for claude in ("Edit", "Write", "MultiEdit", "NotebookEdit", "Read", "Bash", "Agent"):
                self.assertNotIn(claude, names)

    def test_a_claude_tool_with_no_agy_row_fails_the_build(self):
        src = copy_src(self.tmp)
        hooks = claude_hooks()
        hooks["hooks"]["PreToolUse"][0]["matcher"] = "Agent|Glob"
        (src / "hooks" / "hooks.json").write_bytes(json.dumps(hooks).encode("utf-8"))
        with self.assertRaises(agy_plugin.BuildError):
            agy_plugin.build(self.tmp / "o2" / ".agents", self.home, src, with_bladebro=False)

    def test_a_hook_that_is_not_the_installed_herdr_fails_the_build(self):
        src = copy_src(self.tmp)
        hooks = claude_hooks()
        hooks["hooks"]["Stop"][0]["hooks"][0]["command"] = "herdr"
        (src / "hooks" / "hooks.json").write_bytes(json.dumps(hooks).encode("utf-8"))
        with self.assertRaises(agy_plugin.BuildError):
            agy_plugin.build(self.tmp / "o3" / ".agents", self.home, src, with_bladebro=False)


class AgyFilesTest(AgyBuilt):
    def test_the_build_is_a_native_agy_plugin_named_nirvana_with_hooks_at_its_root(self):
        self.assertTrue((self.dot / "plugin.json").is_file(), "no plugin.json: agy does not see a native plugin")
        manifest = json.loads((self.dot / "plugin.json").read_bytes())
        version = (Path(__file__).resolve().parent / "VERSION").read_text(encoding="utf-8").strip()
        self.assertEqual(manifest, {"name": "nirvana", "version": version})
        # agy: a native plugin "must place `hooks.json` directly at the plugin root".
        self.assertTrue((self.dot / "hooks.json").is_file())
        for part in ("skills", "rules", "agents"):
            self.assertTrue((self.dot / part).is_dir(), part)
        self.assertTrue((self.dot / "mcp_config.json").is_file())

    def test_a_rebuild_writes_the_same_bytes(self):
        again = self.tmp / "again" / ".agents"
        agy_plugin.build(again, self.home, with_bladebro=True)

        def tree(root: Path) -> dict:
            return {p.relative_to(root).as_posix(): p.read_bytes() for p in root.rglob("*") if p.is_file()}

        first, second = tree(self.dot), tree(again)
        self.assertEqual(sorted(first), sorted(second))
        for rel in first:
            self.assertEqual(first[rel], second[rel], rel)

    def test_skills_are_copied_with_the_installed_herdr_in_place_of_the_plugin_root(self):
        names = sorted(p.name for p in (SRC / "skills").iterdir() if p.is_dir())
        self.assertEqual(sorted(p.name for p in (self.dot / "skills").iterdir()), names)
        exe = (self.home / "bin" / "herdr.exe").as_posix()
        for name in names:
            text = (self.dot / "skills" / name / "SKILL.md").read_text(encoding="utf-8")
            self.assertNotIn("CLAUDE_PLUGIN_ROOT", text, name)
            self.assertIn(f'"{exe}"', text, name)
            fields, _ = agy_plugin.frontmatter(text)
            self.assertEqual(fields["name"], name)
            self.assertLessEqual(len(text.encode("utf-8")), 24 * 1024, name)
            original = (SRC / "skills" / name / "SKILL.md").read_text(encoding="utf-8")
            self.assertEqual(text, original.replace('"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"', f'"{exe}"'))

    def test_a_skill_named_unlike_its_folder_fails_the_build(self):
        src = copy_src(self.tmp)
        skill = src / "skills" / "gate" / "SKILL.md"
        skill.write_bytes(skill.read_bytes().replace(b"name: gate\n", b"name: gatex\n", 1))
        with self.assertRaises(agy_plugin.BuildError):
            agy_plugin.build(self.tmp / "o4" / ".agents", self.home, src, with_bladebro=False)

    def test_the_role_is_an_always_on_rule_holding_role_md(self):
        rule = (self.dot / "rules" / "nirvana-role.md").read_text(encoding="utf-8")
        head = "---\ntrigger: always_on\n---\n\n"
        self.assertTrue(rule.startswith(head), rule[:80])
        self.assertEqual(rule[len(head):], (SRC / "ROLE.md").read_text(encoding="utf-8"))
        self.assertEqual(sorted(p.name for p in (self.dot / "rules").iterdir()), ["nirvana-role.md"])

    def test_only_the_orchestrator_agent_maps_as_agys_main_agent_with_a_body(self):
        self.assertEqual(sorted(p.name for p in (self.dot / "agents").iterdir()), ["nirvana.md"])
        fields, body = agy_plugin.frontmatter((self.dot / "agents" / "nirvana.md").read_text(encoding="utf-8"))
        self.assertEqual(fields["name"], "nirvana")
        self.assertEqual(fields["mainAgent"], "true")
        self.assertEqual(fields["subagent"], "false")
        for tool in ("read_file", "write_file", "edit_file", "run_command"):
            self.assertIn(tool, fields["tools"])
        for claude in ("model", "effort", "color"):
            self.assertNotIn(claude, fields)
        # Measured: an empty body left `--agent nirvana` on agy's default agent; a body opening with an H1 sent no
        # model call in 180 s.
        self.assertTrue(body.strip())
        self.assertFalse(body.lstrip().startswith("#"))
        workers = sorted(p.stem for p in (SRC / "agents").glob("worker-*.md"))
        self.assertEqual(len(workers), 7)
        for worker in workers:
            self.assertTrue(any(s.startswith(f"agent {worker}:") for s in self.report["skipped"]), worker)

    def test_bladebro_is_decision_0043s_recipe_with_the_orchestrators_own_home(self):
        config = json.loads((self.dot / "mcp_config.json").read_bytes())
        self.assertEqual(list(config), ["mcpServers"])
        server = config["mcpServers"]["bladebro"]
        self.assertEqual((server["command"], server["args"]), ("cmd", ["/c", "bladebro", "mcp"]))
        self.assertEqual(server["env"], {
            "BLADE_HOME": (self.home / "nirvana" / "blade-orchestrator").as_posix(),
            "BLADE_CHROME_FLAGS": "--headless=new", "BLADE_LANE": "agent", "BLADE_IDLE_TIMEOUT": "60",
            "BLADE_NO_UPDATE_CHECK": "1", "BLADE_NO_WARMING": "1", "BLADE_PLAIN": "1"})
        self.assertEqual(set(server), {"command", "args", "env"})

    def test_no_bladebro_on_path_writes_no_mcp_config_and_says_so(self):
        report = agy_plugin.build(self.tmp / "o5" / ".agents", self.home, with_bladebro=False)
        self.assertFalse((self.tmp / "o5" / ".agents" / "mcp_config.json").exists())
        self.assertEqual(report["mcp"], [])
        self.assertTrue(any(s.startswith("mcp bladebro") for s in report["skipped"]), report)


class AgyInstallTest(unittest.TestCase):
    def test_a_failed_validation_keeps_the_installed_folder_and_leaves_no_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            old = home / "nirvana" / "agy" / ".agents" / "hooks.json"
            old.parent.mkdir(parents=True)
            old.write_bytes(b"old")
            bad = mock.Mock(return_value={"ok": False, "why": "exit 1", "findings": [{"severity": "error"}]})
            with FakeAgy(home / "t", with_agy=False), self.assertRaises(agy_plugin.BuildError):
                agy_plugin.install(home, with_bladebro=False, check=bad)
            self.assertEqual(old.read_bytes(), b"old")
            self.assertEqual(sorted(p.name for p in (home / "nirvana").iterdir()), ["agy"])

    def test_a_passed_validation_replaces_the_folder_whole(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            # The R1 (a) folder name: an install over it leaves only the plugin.
            gone = home / "nirvana" / "agy" / ".agents" / "skills" / "gone" / "SKILL.md"
            gone.parent.mkdir(parents=True)
            gone.write_bytes(b"old")
            good = mock.Mock(return_value={"ok": True, "why": "exit 0", "findings": []})
            put = mock.Mock(return_value={"ok": True, "why": "exit 0", "argv": []})
            with FakeAgy(Path(tmp), with_agy=False):
                dst, report = agy_plugin.install(home, with_bladebro=False, check=good, register=put)
            self.assertEqual(dst, home / "nirvana" / "agy")
            self.assertFalse(gone.exists())
            self.assertEqual(sorted(p.name for p in dst.iterdir()), ["nirvana"])
            self.assertTrue((dst / "nirvana" / "hooks.json").is_file())
            good.assert_called_once()
            self.assertEqual(report["validate"]["ok"], True)
            self.assertEqual(sorted(p.name for p in (home / "nirvana").iterdir()), ["agy"])

    def test_install_main_builds_the_agy_folder_and_a_failed_one_exits_non_zero(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            release = tmp / "target" / "release"
            release.mkdir(parents=True)
            (release / install.EXE).write_bytes(b"new")
            (release / install.OPEN_EXE).write_bytes(b"new")
            home = tmp / "home"
            failed = mock.Mock(side_effect=agy_plugin.BuildError("strict failed"))
            with FakeAgy(tmp, with_agy=False), \
                    mock.patch.object(install, "ROOT", tmp), mock.patch.object(install, "home", lambda: home), \
                    mock.patch.object(install, "version", lambda: "0.0.1"), \
                    mock.patch.object(install, "install", mock.Mock(return_value=home / "bin")) as put, \
                    mock.patch.object(install.windows_shell, "install_plugin", mock.Mock(return_value=home)), \
                    mock.patch.object(install.agy_plugin, "install", failed), \
                    mock.patch("builtins.print") as printed:
                code = install.main(["--no-build", "--no-shell"])
            text = "\n".join(str(c.args[0]) for c in printed.call_args_list)
            failed.assert_called_once_with(home)
            put.assert_called_once()     # the binary still installs
            self.assertEqual(code, 1, text)
            self.assertIn("strict failed", text)


class AgyPluginInstallTest(unittest.TestCase):
    """`agy plugin install` after a passing build, against the fake agy (R1 (b))."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.tmp = Path(self._tmp.name)
        self.home = self.tmp / "home"
        self.plugin = str(self.home / "nirvana" / "agy" / "nirvana")

    def tearDown(self):
        self._tmp.cleanup()

    def test_install_runs_after_validate_passes_on_the_installed_folder(self):
        with FakeAgy(self.tmp) as agy:
            dst, report = agy_plugin.install(self.home, with_bladebro=False)
        calls = agy.calls()
        self.assertEqual([c["argv"] for c in calls],
                         [["plugin", "validate", calls[0]["argv"][2], "--strict", "--json"],
                          ["plugin", "install", self.plugin]])
        # Validated in the stage, installed from the folder that took its place: the same tree.
        self.assertEqual(Path(calls[0]["argv"][2]).name, "nirvana")
        self.assertTrue(Path(self.plugin, "plugin.json").is_file())
        done = report["agy_install"]
        self.assertIs(done["ok"], True, done)
        self.assertEqual(done["name"], "nirvana")
        self.assertEqual(done["folder"], "C:/fake-gemini/config/plugins/nirvana")
        self.assertIs(done["replaced"], True)
        self.assertEqual(done["argv"][1:], ["plugin", "install", self.plugin])

    def test_agy_calls_read_an_empty_pipe_with_no_colour(self):
        # Windows NUL reads as a terminal, so a prompt would wait on it; colour codes would break the summary parse.
        with FakeAgy(self.tmp) as agy:
            agy_plugin.install(self.home, with_bladebro=False)
        for call in agy.calls():
            self.assertIs(call["stdin_tty"], False, call)
            self.assertEqual(call["stdin"], "", call)
            self.assertEqual(call["no_color"], "1", call)

    def test_a_failed_validation_never_calls_agy_plugin_install(self):
        with FakeAgy(self.tmp, mode="validate-fails") as agy, self.assertRaises(agy_plugin.BuildError) as err:
            agy_plugin.install(self.home, with_bladebro=False)
        self.assertEqual([c["argv"][:2] for c in agy.calls()], [["plugin", "validate"]])
        self.assertIn("canned finding", str(err.exception))
        self.assertFalse(Path(self.plugin).exists())

    def test_agy_missing_validates_and_installs_nothing_and_says_so(self):
        put = mock.Mock(return_value={"ok": True, "why": "exit 0", "argv": []})
        with FakeAgy(self.tmp, with_agy=False):
            dst, report = agy_plugin.install(self.home, with_bladebro=False, register=put)
        put.assert_not_called()
        self.assertIsNone(report["validate"]["ok"])
        self.assertIsNone(report["agy_install"]["ok"])
        self.assertIn("not on PATH", report["agy_install"]["why"])
        self.assertTrue(Path(self.plugin, "plugin.json").is_file())

    def test_a_real_agy_is_never_handed_a_plugin_from_the_temp_folder(self):
        # The home stands in for the temp folder; the fake agy sits outside it, as the real agy does.
        with FakeAgy(self.tmp) as agy, mock.patch.object(agy_plugin, "temp_root", lambda: self.home):
            dst, report = agy_plugin.install(self.home, with_bladebro=False)
        self.assertEqual([c["argv"][:2] for c in agy.calls()], [["plugin", "validate"]])
        self.assertIsNone(report["agy_install"]["ok"])
        self.assertIn("temp folder", report["agy_install"]["why"])

    def test_install_main_with_agy_missing_skips_with_a_line_and_exits_0(self):
        with FakeAgy(self.tmp, with_agy=False):
            code, text, put = run_install_main(self.tmp, self.home)
        put.assert_called_once()
        self.assertEqual(code, 0, text)
        self.assertIn("agy plugin install: skipped, agy is not on PATH", text)

    def test_install_main_prints_the_installed_plugin_name_and_folder(self):
        with FakeAgy(self.tmp):
            code, text, put = run_install_main(self.tmp, self.home)
        self.assertEqual(code, 0, text)
        self.assertIn("agy plugin install: installed plugin nirvana to C:/fake-gemini/config/plugins/nirvana "
                      "(replaced the earlier copy)", text)

    def test_a_failed_agy_install_exits_1_naming_agys_error_and_the_binary_still_installs(self):
        with FakeAgy(self.tmp, mode="install-fails") as agy:
            code, text, put = run_install_main(self.tmp, self.home)
        self.assertEqual([c["argv"][:2] for c in agy.calls()], [["plugin", "validate"], ["plugin", "install"]])
        put.assert_called_once()     # the binary still installs
        self.assertEqual(code, 1, text)
        self.assertIn("Error: canned install failure", text)


@unittest.skipIf(shutil.which("agy") is None, "agy is not on PATH: the folder is not validated here")
class AgyValidateTest(unittest.TestCase):
    def test_agys_strict_validator_passes_the_build_and_fails_a_broken_rule(self):
        with tempfile.TemporaryDirectory() as tmp:
            dot = Path(tmp) / "nirvana"
            agy_plugin.build(dot, Path(tmp) / "home", with_bladebro=True)
            good = agy_plugin.validate(dot)
            self.assertIs(good["ok"], True, good)
            self.assertEqual(good["components"], {"agents": 1, "hooks": 5, "mcpServers": 1, "rules": 1,
                                                  "sidecars": 0, "skills": 8})
            rule = dot / "rules" / "nirvana-role.md"
            rule.write_bytes(rule.read_bytes().replace(b"trigger: always_on", b"trigger: bogus", 1))
            bad = agy_plugin.validate(dot)
            self.assertIs(bad["ok"], False, bad)


if __name__ == "__main__":
    unittest.main()
