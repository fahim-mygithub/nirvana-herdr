"""install.py's Windows side for Nirvana Claude (plan 12 O4a). No test here writes under `Software\\Classes`: the
registry tests use `FakeRegistry`, or the scratch key `Software\\NirvanaHerdrTest-<pid>`, deleted afterwards.

    python -m unittest nirvana.test_scripts     # runs these too
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import windows_shell as ws  # noqa: E402

# The line a click runs.
GOLDEN_VERB = (r'"C:\L\Microsoft\WindowsApps\wt.exe" -w new -p "Nirvana" -d "%1" "C:\H\bin\herdr.exe" '
               r'--session nirvana nirvana window "%1"')


class VerbTest(unittest.TestCase):
    def test_the_verb_runs_the_window_command_on_the_clicked_folder(self):
        self.assertEqual(ws.verb_command(r"C:\L\Microsoft\WindowsApps\wt.exe", r"C:\H\bin\herdr.exe", "%1"),
                         GOLDEN_VERB)

    def test_the_verb_key_is_neither_old_nirvanas_nor_nirvana_cs(self):
        keys = {sub.rsplit("\\", 1)[1] for sub, _ in ws.VERBS}
        self.assertEqual(keys, {"NirvanaHerdr"})
        self.assertNotIn("Nirvana", keys)
        self.assertNotIn("NirvanaC", keys)
        self.assertEqual([token for _, token in ws.VERBS], ["%1", "%V"])

    def test_install_writes_the_verb_and_toast_id_and_removes_nirvana_cs_verb(self):
        reg = ws.FakeRegistry()
        base = r"Software\Classes"
        for sub in ws.OLD_VERBS:      # Nirvana-C installed, as on the owner's machine
            reg.set(rf"{base}\{sub}", "MUIVerb", "Open Nirvana - CC")
            reg.set(rf"{base}\{sub}\command", None, "pythonw nirvana-herdr.pyw %1")
        for sub, token in zip(ws.OLD_NIRVANA_VERBS, ("%1", "%V")):   # old Nirvana's, as on the owner's machine
            reg.set(rf"{base}\{sub}", "MUIVerb", "Open Nirvana here")
            reg.set(rf"{base}\{sub}\command", None,
                    rf'"C:\Python313\pythonw.exe" "C:\Users\o\.nirvana\open-here.pyw" "{token}"')
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            home = tmp / "home"
            report = ws.install_all(home, reg=reg, base=base, fragments=tmp / "frag", config_dir=tmp / "claude",
                                    wt=Path(r"C:\L\Microsoft\WindowsApps\wt.exe"), font=False)
            backup_file = home / "nirvana" / "backup" / "old-nirvana-verb.json"
            self.assertTrue(backup_file.is_file(), "old Nirvana's keys are backed up before they go")
            backup = json.loads(backup_file.read_text(encoding="utf-8"))
        # Old Nirvana's verb is retired (R8: "have standard Nirvana use the herder varient"), its keys kept in a backup.
        self.assertEqual(report["old_nirvana_verb_retired"], {sub: True for sub in ws.OLD_NIRVANA_VERBS})
        for sub in ws.OLD_NIRVANA_VERBS:
            self.assertFalse(reg.exists(rf"{base}\{sub}"), sub)
            self.assertEqual(backup[sub]["MUIVerb"], "Open Nirvana here")
            self.assertIn("open-here.pyw", backup[sub]["command"])
        for sub, token in ws.VERBS:
            self.assertEqual(reg.get(rf"{base}\{sub}", "MUIVerb"), "Open Nirvana")
            self.assertEqual(reg.get(rf"{base}\{sub}", "Icon"), str(home / "nirvana" / "assets" / "nirvana.ico"))
            command = reg.get(rf"{base}\{sub}\command", None)
            self.assertEqual(command, ws.verb_command(r"C:\L\Microsoft\WindowsApps\wt.exe",
                                                      home / "bin" / "herdr.exe", token))
        for sub in ws.OLD_VERBS:
            self.assertFalse(reg.exists(rf"{base}\{sub}"), sub)
            self.assertFalse(reg.exists(rf"{base}\{sub}\command"), sub)
        self.assertEqual(report["old_verb_removed"], {sub: True for sub in ws.OLD_VERBS})
        self.assertEqual(reg.get(rf"{base}\AppUserModelId\Nirvana.Herdr", "DisplayName"), "Nirvana")
        self.assertEqual(reg.get(rf"{base}\AppUserModelId\Nirvana.Herdr", "IconUri"),
                         str(home / "nirvana" / "assets" / "nirvana-256.png"))
        # The old verb goes only after the new one is written: a failed install never leaves no verb.
        writes = [i for i, op in enumerate(reg.ops) if op[0] == "set" and "NirvanaHerdr" in op[1]]
        deletes = [i for i, op in enumerate(reg.ops) if op[0] == "delete" and "NirvanaC" in op[1]]
        self.assertTrue(writes and deletes and max(writes) < min(deletes), reg.ops)
        # Every key written or deleted is one of the named ones.
        touched = {op[1].lower() for op in reg.ops}
        allowed = {rf"{base}\{sub}".lower() for sub, _ in ws.VERBS} | \
                  {rf"{base}\{sub}\command".lower() for sub, _ in ws.VERBS} | \
                  {rf"{base}\{sub}".lower() for sub in ws.OLD_VERBS} | \
                  {rf"{base}\AppUserModelId\Nirvana.Herdr".lower()} | \
                  {rf"{base}\{sub}".lower() for sub in ws.OLD_NIRVANA_VERBS} | \
                  {rf"{base}\{sub}\command".lower() for sub in ws.OLD_NIRVANA_VERBS} | \
                  {rf"{base}\{sub}\command".lower() for sub in ws.OLD_VERBS}
        self.assertLessEqual(touched, allowed, touched - allowed)

    def test_a_nirvana_key_of_another_program_is_left_alone(self):
        reg = ws.FakeRegistry()
        reg.set(r"B\Directory\shell\Nirvana", "MUIVerb", "Some other Nirvana")
        reg.set(r"B\Directory\shell\Nirvana\command", None, r'"C:\Other\nirvana.exe" "%1"')
        with tempfile.TemporaryDirectory() as tmp:
            backup = Path(tmp) / "backup.json"
            self.assertEqual(ws.retire_old_nirvana_verb(reg, "B", backup), {})
            self.assertFalse(backup.exists())
        self.assertEqual(reg.get(r"B\Directory\shell\Nirvana", "MUIVerb"), "Some other Nirvana")

    def test_removing_the_old_verb_twice_is_harmless(self):
        reg = ws.FakeRegistry()
        self.assertEqual(ws.remove_old_verb(reg, "B"), {sub: False for sub in ws.OLD_VERBS})


@unittest.skipUnless(os.name == "nt", "the real registry is Windows only")
class RealRegistryTest(unittest.TestCase):
    """The winreg layer itself, under a scratch key that is created and deleted here."""

    def setUp(self):
        self.reg = ws.Registry()
        self.base = rf"Software\NirvanaHerdrTest-{os.getpid()}"
        self.assertFalse(self.reg.exists(self.base))

    def tearDown(self):
        self.reg.delete_tree(self.base)
        self.assertFalse(self.reg.exists(self.base), "the scratch key is gone")

    def test_verb_and_toast_id_round_trip_and_the_old_verb_is_deleted(self):
        for sub in ws.OLD_VERBS:
            self.reg.set(rf"{self.base}\{sub}", "MUIVerb", "Open Nirvana - CC")
            self.reg.set(rf"{self.base}\{sub}\command", None, "x")
        written = ws.install_verb(self.reg, self.base, r"C:\wt.exe", r"C:\h\herdr.exe", r"C:\h\n.ico")
        ws.register_toast_id(self.reg, self.base, r"C:\h\n.png")
        removed = ws.remove_old_verb(self.reg, self.base)
        read = ws.read_verb(self.reg, self.base)
        self.assertEqual(read, written)
        self.assertEqual(removed, {sub: True for sub in ws.OLD_VERBS})
        for sub in ws.OLD_VERBS:
            self.assertFalse(self.reg.exists(rf"{self.base}\{sub}"))
        self.assertEqual(self.reg.get(rf"{self.base}\{ws.TOAST_KEY}", "IconUri"), r"C:\h\n.png")
        self.assertEqual(ws.remove_verb(self.reg, self.base), {sub: True for sub, _ in ws.VERBS})


class FilesTest(unittest.TestCase):
    def test_the_plugin_is_copied_whole_and_a_removed_file_goes(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            stale = home / "nirvana" / "claude" / "agents" / "gone.md"
            stale.parent.mkdir(parents=True)
            stale.write_bytes(b"old")
            dst = ws.install_plugin(home)
            self.assertFalse(stale.exists())
            src = ws.ROOT / "nirvana" / "claude"
            # O4b: every file of the tree arrives as it is, the role, the hooks, the worker agents and
            # every skill included; only settings.json gains the statusline.
            rels = sorted(p.relative_to(src).as_posix() for p in src.rglob("*") if p.is_file())
            for rel in ("ROLE.md", "hooks/hooks.json", "skills/seat/SKILL.md", "skills/usage/SKILL.md",
                        "skills/handoff/SKILL.md", "skills/jev/SKILL.md", "skills/media/SKILL.md", "skills/gate/SKILL.md",
                        "agents/worker-opus-high.md", "agents/worker-haiku-low.md"):
                self.assertIn(rel, rels)
            for rel in rels:
                if rel != "settings.json":
                    self.assertEqual((dst / rel).read_bytes(), (src / rel).read_bytes(), rel)
            installed = json.loads((dst / "settings.json").read_bytes())
            mine = json.loads((src / "settings.json").read_bytes())
            self.assertEqual({k: v for k, v in installed.items() if k != "statusLine"}, mine)
            exe = (home / "bin" / "herdr.exe").as_posix()
            self.assertEqual(installed["statusLine"]["command"], f'"{exe}" nirvana statusline')
            self.assertNotIn(b"\r", (dst / "settings.json").read_bytes())

    def test_settings_carry_the_theme_and_the_orchestrator_env(self):
        settings = json.loads((ws.ROOT / "nirvana" / "claude" / "settings.json").read_bytes())
        self.assertEqual(settings["theme"], "custom:nirvana-dark")
        self.assertEqual(settings["env"], {"NIRVANA_ORCHESTRATOR": "1"})

    def test_the_agent_is_opus_at_medium_with_the_plans_rules(self):
        # O4b: the role is appended to Claude Code's own system prompt (ROLE.md); the agent file names the seat and
        # has no prompt, which would replace Claude Code's.
        claude = ws.ROOT / "nirvana" / "claude"
        _, head, body = (claude / "agents" / "nirvana.md").read_text(encoding="utf-8").split("---", 2)
        self.assertIn("name: nirvana\n", head)
        self.assertIn("model: claude-opus-5-5\n", head)
        self.assertIn("effort: medium\n", head)
        self.assertEqual(body.strip(), "")
        text = (claude / "ROLE.md").read_text(encoding="utf-8")
        self.assertFalse(text.startswith("---"), "ROLE.md is prompt text, with no front matter")
        self.assertIn("Opus 5.5 at medium", text)
        for rule in ("Seat every order through `herdr seat pick`; say the seat, model and effort in one line.",
                     "A question is answered, never built.",
                     "Never resume a finished worker for a new question;",
                     "A merge to `main` asks the owner, except in auto mode within its budgets (§12 R6)."):
            self.assertIn(rule, text)

    def test_themes_fragment_and_assets_land_where_claude_and_terminal_read_them(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            themes = ws.install_themes(tmp / "claude")
            for path in themes:
                self.assertEqual(path.read_bytes(), (ws.ROOT / "nirvana" / "themes" / path.name).read_bytes())
            old = ws.fragment_path(tmp / "frag", "Nirvana-C")
            old.parent.mkdir(parents=True)
            old.write_bytes(b"{}")
            path = ws.write_fragment(tmp / "frag", ws.fragment(tmp / "home"))
            self.assertTrue(ws.remove_old_fragment(tmp / "frag"))
            self.assertFalse(old.exists())
            raw = path.read_bytes()
            self.assertNotIn(b"\r", raw)
            data = json.loads(raw)
            (profile,) = data["profiles"]
            self.assertEqual(profile["name"], "Nirvana")
            self.assertIn("--session nirvana nirvana-shell", profile["commandline"])
            self.assertEqual(profile["font"]["face"], "JetBrains Mono, Cascadia Mono")
            self.assertEqual([s["name"] for s in data["schemes"]], ["Nirvana Light", "Nirvana Dark"])
            assets = ws.install_assets(tmp / "home")
            self.assertEqual({p.name for p in assets}, set(ws.ASSETS))


if __name__ == "__main__":
    unittest.main()
