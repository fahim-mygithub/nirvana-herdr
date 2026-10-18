"""Plan 12 O7: the A/B driver (nirvana/bench/ab.py). No test here starts a model, a herdr server or a clone of the
owner's project; every input is a fixture recorded on 2026-10-18 (nirvana/bench/fixtures) or a temp directory.

    python -m unittest nirvana.test_ab
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "bench"))

import ab  # noqa: E402

FIX = HERE / "bench" / "fixtures"
SRC = HERE.parent / "src"


def fixture(name: str) -> str:
    return (FIX / name).read_text(encoding="utf-8")


def jsonl(name: str) -> list[dict]:
    return [json.loads(ln) for ln in fixture(name).splitlines() if ln.strip()]


class PathLimitTest(unittest.TestCase):
    def refused(self, fn, why):
        """Fails (not errors) unless `fn` raises Refused: a sabotaged rule lets the call run on into other code."""
        try:
            fn()
        except ab.Refused:
            return
        except Exception as e:  # noqa: BLE001
            self.fail(f"{why}: not refused, {type(e).__name__}")
        self.fail(f"{why}: not refused")

    def test_nothing_is_written_under_the_owners_project_but_a_sibling_is_fine(self):
        for inside in (r"D:\MotoProject", r"D:\MotoProject\runs\x", "d:/motoproject/clone", "D:/MotoProject/"):
            self.refused(lambda inside=inside: ab.refuse_inside(inside, r"D:\MotoProject"), inside)
        self.assertFalse(ab.is_inside(r"D:\MotoProject2\x", r"D:\MotoProject"))
        self.assertFalse(ab.is_inside(r"C:\bench\x", r"D:\MotoProject"))

    def test_a_clone_is_refused_outside_the_out_dir_inside_the_source_or_when_it_exists(self):
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / "source-not-a-repo"      # never cloned: a rule that fails to refuse errors in git, loudly
            repo = {"source": str(src), "sha": "0" * 40}
            out = Path(tmp) / "out"
            out.mkdir()
            self.refused(lambda: ab.make_clone(repo, src / "out" / "clone", src / "out"), "inside the source")
            self.refused(lambda: ab.make_clone(repo, Path(tmp) / "elsewhere" / "clone", out), "outside the out dir")
            (out / "taken").mkdir()
            self.refused(lambda: ab.make_clone(repo, out / "taken", out), "an existing folder")

    def test_a_clone_is_removed_only_when_it_is_inside_the_run_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            run, other = Path(tmp) / "run", Path(tmp) / "other"
            for d in (run / "clone", other / "clone"):
                d.mkdir(parents=True)
                (d / "f.txt").write_text("x")
                subprocess.run(["git", "init", "-q", str(d)], check=True)
            ab.finalize_clone(run / "clone", run, keep=False)
            self.assertFalse((run / "clone").exists())
            self.assertTrue((run / "git-final.txt").is_file())
            note = ab.finalize_clone(other / "clone", run, keep=False)
            self.assertTrue((other / "clone").exists(), "a clone outside the run directory is never removed")
            self.assertNotIn("removed", note)

    def test_the_claude_project_folder_name_is_every_non_alphanumeric_as_a_dash(self):
        self.assertEqual(ab.mangle(r"C:\Users\fahim\Desktop\Pojects\Nirvana-Herdr-o4b\target\o4b-live\proj"),
                         "C--Users-fahim-Desktop-Pojects-Nirvana-Herdr-o4b-target-o4b-live-proj")
        self.assertEqual(ab.mangle(r"C:\p\clone\.nirvana\worktrees\sub"), "C--p-clone--nirvana-worktrees-sub")

    def test_a_process_is_the_runs_when_its_folder_or_command_line_names_the_clone_never_the_owners(self):
        clone = r"C:\bench\runs\M2\N-1\clone"
        self.assertTrue(ab.names_clone(clone, "python editor/server/main.py --port 8770", clone))
        self.assertTrue(ab.names_clone(clone, "python " + clone + r"\editor\server\main.py", r"C:\elsewhere"))
        self.assertFalse(ab.names_clone(clone, "python editor/server/main.py --port 8770", r"D:\MotoProject"))

    def test_the_trust_prompt_is_answered_only_for_a_clone_inside_the_out_dir(self):
        ab.check_trust_scope(r"C:\bench\runs\M1\N-1\clone", r"C:\bench")
        self.refused(lambda: ab.check_trust_scope(r"D:\MotoProject", r"C:\bench"), "the owner's project")
        self.refused(lambda: ab.check_trust_scope(r"C:\bench2\clone", r"C:\bench"), "a sibling of the out dir")

    def test_the_env_isolates_herdr_and_opens_no_browser(self):
        import os
        old = os.environ.get("XDG_STATE_HOME")
        os.environ["XDG_STATE_HOME"] = r"C:\owner\state"       # an inherited value must not reach the bench's server
        try:
            self.env_isolates()
        finally:
            if old is None:
                os.environ.pop("XDG_STATE_HOME", None)
            else:
                os.environ["XDG_STATE_HOME"] = old

    def env_isolates(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = Path(tmp)
            env = ab.bench_env(run, run / "home", "o7x", isolated=True)
            self.assertIn("BROWSER", env)
            self.assertEqual(env["XDG_CONFIG_HOME"], str(run / "xdg-config"))
            self.assertEqual(env["XDG_STATE_HOME"], str(run / "xdg-state"))
            self.assertEqual((env["HERDR_SESSION"], env["NIRVANA_HERDR_HOME"]), ("o7x", str(run / "home")))
            browser = Path(env["BROWSER"])
            self.assertTrue(browser.is_file())
            self.assertNotIn("start", browser.read_text().lower())
            plain = ab.bench_env(run, run / "home", "none", isolated=False)
            self.assertEqual(plain["BROWSER"], env["BROWSER"], "arm C opens no browser either")

    def test_the_inherited_socket_override_cannot_point_the_bench_at_the_owners_server(self):
        import os
        old = os.environ.get("HERDR_SOCKET_PATH")
        os.environ["HERDR_SOCKET_PATH"] = r"C:\owner\herdr.sock"
        try:
            with tempfile.TemporaryDirectory() as tmp:
                env = ab.bench_env(Path(tmp), Path(tmp) / "h", "s", isolated=True)
            self.assertNotIn("HERDR_SOCKET_PATH", env)
        finally:
            if old is None:
                os.environ.pop("HERDR_SOCKET_PATH", None)
            else:
                os.environ["HERDR_SOCKET_PATH"] = old

    def test_the_scratch_home_holds_copies_and_the_owners_files_are_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            owner = tmp / "owner"
            (owner / "nirvana").mkdir(parents=True)
            (owner / "nirvana" / "usage.json").write_text('{"a": 1}')
            (owner / "nirvana" / "seats.toml").write_text("x = 1\n")
            exe, plugin = tmp / "herdr.exe", tmp / "plugin"
            exe.write_bytes(b"exe")
            (plugin / "agents").mkdir(parents=True)
            (plugin / "ROLE.md").write_text("role")
            home = tmp / "scratch"
            seeded = ab.seed_home(home, exe, plugin, owner)
            self.assertEqual(seeded, ["bin/herdr.exe", "nirvana/claude", "nirvana/usage.json", "nirvana/seats.toml"])
            self.assertEqual((home / "bin" / "herdr.exe").read_bytes(), b"exe")
            self.assertTrue((home / "nirvana" / "claude" / "ROLE.md").is_file(), "the plugin is copied")
            self.assertEqual((home / "nirvana" / "claude" / "ROLE.md").read_text(), "role")
            self.assertTrue((owner / "nirvana" / "usage.json").is_file(), "the owner's usage.json was moved")
            self.assertTrue((owner / "nirvana" / "seats.toml").is_file(), "the owner's seats.toml was moved")
            self.assertEqual((owner / "nirvana" / "usage.json").read_text(), '{"a": 1}')
            (home / "nirvana" / "usage.json").write_text("changed")
            self.assertEqual((owner / "nirvana" / "usage.json").read_text(), '{"a": 1}')


class ScenarioFileTest(unittest.TestCase):
    def test_the_scenarios_are_the_three_the_brief_names_with_the_owners_words(self):
        doc = ab.load_scenarios()
        self.assertEqual([s["id"] for s in doc["scenarios"]], ["M1", "M2", "M5"])
        self.assertEqual(ab.scenario(doc, "M1")["prompt"],
                         "Can you explain what this project does and how I start the reel editor?")
        self.assertEqual(ab.scenario(doc, "M2")["prompt"],
                         "Can you start the reel project server with the hike workload loaded?")
        self.assertEqual(set(doc["arms"]), {"N", "C"})
        with self.assertRaises(ab.Refused):
            ab.scenario(doc, "M9")

    def test_each_scenarios_truth_check_is_in_the_fork_and_its_source_is_named(self):
        for sc in ab.load_scenarios()["scenarios"]:
            script = ab.TRUTH_DIR / Path(sc["truth"]["check"]).name
            self.assertTrue(script.is_file(), script)
            self.assertIn("Copied from old Nirvana", script.read_text(encoding="utf-8").splitlines()[0])
        self.assertTrue((ab.TRUTH_DIR / "moto_facts.json").is_file())

    def test_the_guard_is_the_briefs_80_and_85(self):
        self.assertEqual((ab.GUARD_FIVE_H, ab.GUARD_WEEK), (0.80, 0.85))


class ArmCTest(unittest.TestCase):
    def test_arm_c_is_print_mode_on_opus_5_5_medium_with_stream_json(self):
        argv = ab.cc_argv("the ask")
        self.assertEqual(argv[1:], ["-p", "the ask", "--model", "claude-opus-5-5", "--effort", "medium",
                                    "--output-format", "stream-json", "--verbose"])
        self.assertEqual(ab.cc_argv("x", ["--permission-mode", "auto"])[-2:], ["--permission-mode", "auto"])

    def test_the_stream_is_split_four_ways_and_a_message_counts_once(self):
        rows = jsonl("stream-c-m1.jsonl")
        parsed = ab.parse_stream(rows)
        ids = {r["message"]["id"] for r in rows if r["type"] == "assistant"}
        self.assertEqual(parsed["tokens"]["messages"], len(ids))
        self.assertLess(len(ids), sum(1 for r in rows if r["type"] == "assistant"), "the fixture repeats a message id")
        t = parsed["tokens"]["orchestrator"]
        self.assertEqual((t["input"], t["cache_write"], t["cache_read"]), (8, 51089, 211441))
        self.assertEqual(parsed["result_usage"]["output"], 1848, "the result row's output, not the messages' 48")
        self.assertEqual(parsed["tokens"]["workers"], ab.zero_tokens())
        self.assertEqual(ab.gross(t), 8 + 51089 + 211441)
        self.assertTrue(parsed["answer"].startswith("## What the project does"))
        self.assertEqual(parsed["result"]["subtype"], "success")

    def test_a_subagents_messages_are_workers_and_the_orchestrators_stay_apart(self):
        def row(mid, parent, i, cw, cr, o):
            return {"type": "assistant", "parent_tool_use_id": parent,
                    "message": {"id": mid, "usage": {"input_tokens": i, "cache_creation_input_tokens": cw,
                                                     "cache_read_input_tokens": cr, "output_tokens": o}}}
        rows = [row("a", None, 1, 2, 3, 4), row("a", None, 1, 2, 3, 4), row("b", "toolu_1", 10, 20, 30, 40)]
        got = ab.split_tokens(rows)
        self.assertEqual(got["orchestrator"], {"input": 1, "cache_write": 2, "cache_read": 3, "output": 4})
        self.assertEqual(got["workers"], {"input": 10, "cache_write": 20, "cache_read": 30, "output": 40})

    def test_a_run_with_no_result_row_says_so_and_is_not_a_success(self):
        parsed = ab.parse_stream(jsonl("stream-c-m1.jsonl")[:-1])
        self.assertIsNone(parsed["result"])
        self.assertIsNone(parsed["result_usage"])
        self.assertEqual(ab.parse_stream([])["tokens"]["messages"], 0)

    def test_a_half_written_last_line_of_a_killed_run_is_skipped(self):
        rows = ab.parse_lines('{"type":"system"}\n{"type":"assistant","message":{"id":"x","usa\n{"type":"result"}\n'
                              '{"type":"assistant","message":{"id":"y","usa')
        self.assertEqual(rows, [{"type": "system"}, {"type": "result"}])

    def test_a_transcript_counts_the_orchestrator_and_subagent_files_separately(self):
        rows = jsonl("transcript-n-m1.jsonl")
        main = ab.split_tokens(rows)
        self.assertGreater(main["orchestrator"]["cache_read"], 0)
        self.assertEqual(main["workers"], ab.zero_tokens())
        sub = ab.split_tokens(rows, all_workers=True)
        self.assertEqual(sub["workers"], main["orchestrator"])
        self.assertEqual(sub["orchestrator"], ab.zero_tokens())
        sidechain = [dict(r, isSidechain=True) for r in rows]
        self.assertEqual(ab.split_tokens(sidechain)["workers"], main["orchestrator"])


class LaunchTest(unittest.TestCase):
    def test_arm_n_launches_what_the_rust_launch_says(self):
        text = (SRC / "nirvana_launch.rs").read_text(encoding="utf-8")
        self.assertEqual(re.search(r'pub const MODEL: &str = "([^"]+)"', text).group(1), ab.MODEL)
        self.assertEqual(re.search(r'pub const EFFORT: &str = "([^"]+)"', text).group(1), ab.EFFORT)
        body = text[text.index("pub fn nirvana_claude_args"):]
        body = body[:body.index("\n}\n")]
        flags = re.findall(r'"(--[a-z-]+)"\.into\(\)', body)
        home = Path(r"C:\h")
        mine = ab.nirvana_claude_args(home)
        self.assertEqual([w for w in mine if w.startswith("--")], flags)
        self.assertIn("--agent", mine)
        self.assertEqual(mine[mine.index("--agent") + 1], "nirvana")
        self.assertEqual(mine[mine.index("--plugin-dir") + 1], str(home / "nirvana" / "claude"))
        self.assertEqual(mine[mine.index("--name") + 1], re.search(r'DISPLAY_NAME: &str = "([^"]+)"', text).group(1))

    def test_the_typed_line_quotes_only_what_needs_it(self):
        line = ab.powershell_line("claude", ["--name", "Nirvana", "--settings", r"C:\a b\s.json", "it's"])
        self.assertEqual(line, "claude --name Nirvana --settings 'C:\\a b\\s.json' 'it''s'")


class ScreenTest(unittest.TestCase):
    def test_the_recorded_screens_read_as_what_they_are(self):
        self.assertEqual(ab.screen_kind(fixture("screen-idle-2.1.302.txt")), "idle")
        self.assertEqual(ab.screen_kind(fixture("screen-trust-no-highlighted-2.1.302.txt")), "trust")
        ansi = SRC / "nirvana_run" / "fixtures"
        self.assertEqual(ab.screen_kind((ansi / "claude-2.1.302-empty.ansi").read_text(encoding="utf-8", errors="replace")),
                         "idle")
        self.assertEqual(ab.screen_kind((ansi / "claude-2.1.302-typed.ansi").read_text(encoding="utf-8", errors="replace")),
                         "typed")

    def test_a_working_screen_is_the_spinner_line_and_the_asked_text_above_the_input_box_is_not_typed_text(self):
        # Recorded 2026-10-18 on claude 2.1.302: haiku running a 25 s Bash call, and the same pane after "done".
        self.assertEqual(ab.screen_kind(fixture("screen-working-2.1.302.txt")), "working")
        self.assertEqual(ab.screen_kind(fixture("screen-idle-after-ask-2.1.302.txt")), "idle")
        self.assertIn("\u276f Run this exact shell command", fixture("screen-idle-after-ask-2.1.302.txt"))

    def test_a_working_spinner_and_a_permission_prompt_are_not_idle(self):
        self.assertEqual(ab.screen_kind("\u2733 Thinking\u2026 (12s \u00b7 esc to interrupt)\n\u276f \n"), "working")
        self.assertEqual(ab.screen_kind("\u2722 Accomplishing\u2026 (1m 7s \u00b7 \u2193 52 tokens)\n\u276f \n"), "working")
        self.assertNotEqual(ab.screen_kind("\u273b Crunched for 31s \u00b7 done 3:11 AM\n\u276f \n"), "working")
        self.assertNotEqual(ab.screen_kind("The trust this folder, Yes answer: no\n\u276f \n"), "trust")
        self.assertEqual(ab.screen_kind("Do you want to proceed?\n \u276f 1. Yes\n   2. No\n"), "permission")

    def test_enter_is_pressed_only_on_the_yes_line_never_on_no_exit(self):
        self.assertEqual(ab.trust_action(fixture("screen-trust-no-highlighted-2.1.302.txt")), "down")
        self.assertEqual(ab.trust_action(fixture("screen-trust-yes-highlighted-2.1.302.txt")), "enter")
        self.assertIsNone(ab.trust_action("nothing is highlighted here"))


class CheckAndJournalTest(unittest.TestCase):
    def test_every_phrase_the_classifier_reads_is_in_the_rust_check(self):
        rust = (SRC / "nirvana_run.rs").read_text(encoding="utf-8")
        for phrase in ("the run ended", "every order is finished and the run is not ended", "the wake cap asked the owner",
                       "worker live:", "is deferred to", "wait for the orchestrator", "waits for the gate",
                       "waits for an answer", "was interrupted", "no live worker and no pending step",
                       "the run has no order seated"):
            self.assertIn(phrase, rust, phrase)

    def test_each_check_line_is_one_class(self):
        cases = {
            "r: the run ended": "ended",
            "r: dead end: every order is finished and the run is not ended: `herdr run end`": "finished_unended",
            "r: the wake cap asked the owner": "wake_cap",
            "r: worker live: sub, mul": "live",
            "r: slow is deferred to 2026-10-18 06:00Z": "deferred",
            "r: 2 event(s) wait for the orchestrator: `herdr run notice`": "queued",
            "r: sub waits for the gate": "gate",
            "r: sub waits for an answer": "gate",
            "r: sub was interrupted: the orchestrator continues, redoes or asks": "gate",
            "r: dead end: sub is stopped with no live worker and no pending step: `herdr run resume`": "stuck",
            "r: dead end: the run has no order seated": "none",
            "": "unknown",
            "something new": "unknown",
        }
        for text, want in cases.items():
            self.assertEqual(ab.classify_check(text), want, text)

    def test_a_recorded_journal_reads_at_each_point_of_its_run(self):
        events = jsonl("journal-o5live.jsonl")

        def upto(seq):
            return ab.journal_state([e for e in events if e["seq"] <= seq])[0]
        self.assertEqual(upto(0), "empty")
        self.assertEqual(upto(55), "ended")
        self.assertEqual(upto(41), "finished_unended", "every order merged and the run not ended")
        self.assertEqual(upto(43), "working", "sub started")
        self.assertEqual(upto(47), "pending", "sub delivered, the gate has not run")
        self.assertEqual(upto(48), "pending")
        self.assertEqual(upto(49), "pending", "sub was sent back and nothing replaced it yet")
        self.assertEqual(upto(51), "working", "sub-s1 replaced it")
        self.assertEqual(upto(54), "pending", "sub-s1 was stopped")
        self.assertEqual(upto(8), "pending", "mul ended delivered, ungated")

    def test_a_state_the_journal_cannot_place_is_never_ended(self):
        self.assertEqual(ab.journal_state([{"seq": 1, "event": "opened"}])[0], "pending")


class EndDetectorTest(unittest.TestCase):
    def drive(self, samples, **kw):
        det = ab.EndDetector(**kw)
        det.start(0.0)
        for t, status, kind, runs in samples:
            out = det.feed(t, status=status, kind=kind, runs=runs)
            if out:
                return out
        return None

    def quiet(self, start, stop, runs=None, step=3):
        return [(float(t), "idle", "idle", runs or {}) for t in range(start, stop, step)]

    def test_a_question_ends_at_the_first_idle_after_the_answer_not_after_the_settle_time(self):
        samples = [(3.0, "working", "working", {}), (6.0, "working", "working", {})] + self.quiet(9, 60)
        verdict = self.drive(samples)
        self.assertEqual(verdict[0], "over")
        self.assertEqual(verdict[1], 9.0, "the wall is when the quiet began")
        self.assertGreaterEqual(self.quiet(9, 60)[0][0], 9.0)

    def test_a_live_worker_keeps_the_run_open_though_the_orchestrator_is_idle(self):
        live = {"r1": "live"}
        samples = [(3.0, "working", "working", {})] + self.quiet(6, 120, live)
        self.assertIsNone(self.drive(samples))
        ended = self.quiet(120, 160, {"r1": "ended"})
        verdict = self.drive(samples + ended)
        self.assertEqual(verdict[0], "over")
        self.assertEqual(verdict[1], 120.0)

    def test_a_queued_wake_or_a_waiting_gate_is_not_the_end(self):
        for cls in ("queued", "gate", "deferred", "unknown"):
            samples = [(3.0, "working", "working", {})] + self.quiet(6, 120, {"r1": cls})
            self.assertIsNone(self.drive(samples), cls)

    def test_a_run_left_unended_ends_the_wait_and_is_an_incident(self):
        samples = [(3.0, "working", "working", {})] + self.quiet(6, 80, {"r1": "finished_unended"})
        verdict = self.drive(samples)
        self.assertEqual(verdict[0], "over")
        self.assertEqual(verdict[3], ["dead_end"])

    def test_a_busy_sample_restarts_the_quiet(self):
        samples = ([(3.0, "working", "working", {})] + self.quiet(6, 20) + [(21.0, "working", "working", {})]
                   + self.quiet(24, 40))
        self.assertIsNone(self.drive(samples))

    def test_a_pane_blocked_on_a_prompt_is_never_quiet_and_ends_as_stuck(self):
        samples = [(float(t), "blocked", "permission", {}) for t in range(3, 200, 3)]
        verdict = self.drive(samples)
        self.assertEqual(verdict[0], "stuck_blocked")

    def test_an_ask_that_never_started_is_no_response_not_a_pass(self):
        verdict = self.drive(self.quiet(3, 200))
        self.assertEqual(verdict[0], "no_response")
        self.assertEqual(verdict[3], ["never_started"])

    def test_the_prompt_before_the_ask_registered_is_not_an_answer(self):
        samples = self.quiet(3, 30) + [(30.0, "working", "working", {})] + self.quiet(33, 90)
        verdict = self.drive(samples, grace_s=60)
        self.assertEqual(verdict[0], "over")
        self.assertEqual(verdict[1], 33.0)

    def test_text_typed_on_the_prompt_line_is_not_idle(self):
        samples = [(3.0, "working", "working", {})] + [(float(t), "idle", "typed", {}) for t in range(6, 100, 3)]
        self.assertIsNone(self.drive(samples))


class PersonPolicyTest(unittest.TestCase):
    def test_a_turn_that_ends_on_a_question_is_answered_and_one_that_mentions_one_is_not(self):
        self.assertTrue(ab.wants_reply("Done.\n\nShall I merge it to main?"))
        self.assertTrue(ab.wants_reply("Done.\n\n**Merge to main?**"))
        self.assertTrue(ab.wants_reply("Is this right?\n\n"))
        self.assertFalse(ab.wants_reply("Which seat? I picked flash.\n\nIt is merged."))
        self.assertFalse(ab.wants_reply("It is merged."))
        self.assertFalse(ab.wants_reply("Done.\n\nWhy did the check fail? It did not; it is merged."))
        self.assertFalse(ab.wants_reply(""))

    def test_activity_is_the_newest_write_among_transcripts_subagents_and_journals(self):
        import os
        import time
        with tempfile.TemporaryDirectory() as tmp:
            cfg, clone = Path(tmp) / "cfg", Path(tmp) / "clone"
            folder = cfg / "projects" / ab.mangle(clone)
            (folder / "sess" / "subagents").mkdir(parents=True)
            (clone / ".nirvana" / "runs" / "r1").mkdir(parents=True)
            now = time.time()
            for path, age in ((folder / "sess.jsonl", 400), (folder / "sess" / "subagents" / "agent-a.jsonl", 90),
                              (clone / ".nirvana" / "runs" / "r1" / "journal.jsonl", 30)):
                path.write_text("{}\n")
                os.utime(path, (now - age, now - age))
            old = os.environ.get("CLAUDE_CONFIG_DIR")
            os.environ["CLAUDE_CONFIG_DIR"] = str(cfg)
            try:
                got = ab.latest_activity(clone, now - 1000)
                self.assertIsNotNone(got)
                self.assertAlmostEqual(now - got, 30, delta=2)
                (clone / ".nirvana" / "runs" / "r1" / "journal.jsonl").unlink()
                self.assertAlmostEqual(now - ab.latest_activity(clone, now - 1000), 90, delta=2)
                self.assertIsNone(ab.latest_activity(Path(tmp) / "other", now - 1000))
            finally:
                if old is None:
                    os.environ.pop("CLAUDE_CONFIG_DIR", None)
                else:
                    os.environ["CLAUDE_CONFIG_DIR"] = old


class ServedProbeTest(unittest.TestCase):
    CLONE = Path(r"C:\bench\runs\M2\N-1\clone")

    def fake(self, servers):
        def get(url):
            m = re.match(r"http://127\.0\.0\.1:(\d+)(/.*)", url)
            port, path = int(m.group(1)), m.group(2)
            if port not in servers:
                return None, ""
            title, root = servers[port]
            if path == "/":
                return 200, f"<html><title>{title}</title></html>"
            return 200, json.dumps({"project": {"kind": "post", "root": root}})
        return get

    def test_the_owners_server_on_8770_is_told_apart_by_its_root(self):
        sc = {"served": {"path": "/", "api": "/api/project"}}
        servers = {8770: ("reel editor", r"D:\MotoProject"), 8771: ("reel editor", str(self.CLONE))}
        got = ab.probe_served(self.CLONE, sc, get=self.fake(servers), ports=range(8770, 8775), now=lambda: "T")
        self.assertIsNotNone(got)
        self.assertEqual(got["port"], 8771)
        self.assertEqual(got["title"], "reel editor")
        self.assertEqual((got["status"], got["api_status"], got["first_served_at"]), (200, 200, "T"))
        self.assertIn('"kind": "post"', got["api_body"])

    def test_nothing_of_this_runs_is_up_returns_none(self):
        sc = {"served": {"path": "/", "api": "/api/project"}}
        servers = {8770: ("reel editor", r"D:\MotoProject")}
        self.assertIsNone(ab.probe_served(self.CLONE, sc, get=self.fake(servers), ports=range(8770, 8775)))

    def test_a_server_in_a_worktree_of_the_clone_counts(self):
        sc = {"served": {"path": "/", "api": "/api/project"}}
        servers = {8772: ("reel editor", str(self.CLONE / ".nirvana" / "worktrees" / "serve"))}
        got = ab.probe_served(self.CLONE, sc, get=self.fake(servers), ports=range(8770, 8775))
        self.assertIsNotNone(got)
        self.assertEqual(got["port"], 8772)


class MeterTest(unittest.TestCase):
    def snap(self, five=0.2, week=0.56):
        return {"max": {"5h": {"used": five}, "week": {"used": week}}}

    def test_the_recorded_usage_reading_is_a_snapshot_of_every_pool_with_a_window(self):
        snap = ab.usage_snapshot(json.loads(fixture("usage-20261018.json")))
        self.assertEqual(snap["max"]["5h"]["used"], 0.18)
        self.assertEqual(snap["max"]["week"]["used"], 0.56)
        self.assertNotIn("codex", snap, "a pool with no reading is absent, not zero")
        self.assertNotIn("local", snap)
        self.assertIn("week", snap["argon"])

    def test_a_delta_is_after_minus_before_and_a_reset_is_named_not_negative(self):
        before = {"max": {"5h": {"used": 0.20}, "week": {"used": 0.56}}, "gemini": {"5h": {"used": 0.5}}}
        after = {"max": {"5h": {"used": 0.23}, "week": {"used": 0.55}}, "gemini": {"5h": {"used": 0.5}},
                 "argon": {"week": {"used": 0.1}}}
        d = ab.usage_delta(before, after)
        self.assertAlmostEqual(d["max"]["5h"], 0.03)
        self.assertEqual(d["max"]["week"], "reset")
        self.assertEqual(d["gemini"]["5h"], 0.0)
        self.assertNotIn("argon", d, "a window with no earlier reading has no delta")

    def test_the_guard_refuses_above_80_and_85_and_on_a_missing_reading(self):
        self.assertTrue(ab.guard(self.snap(0.80, 0.85))[0], "the limits themselves are allowed")
        self.assertFalse(ab.guard(self.snap(0.81, 0.50))[0])
        self.assertIn("5h", ab.guard(self.snap(0.81, 0.50))[1])
        self.assertFalse(ab.guard(self.snap(0.10, 0.86))[0])
        self.assertIn("week", ab.guard(self.snap(0.10, 0.86))[1])
        self.assertFalse(ab.guard({})[0], "a reading that did not arrive is a refusal")
        self.assertFalse(ab.guard({"max": {"5h": {"used": 0.1}}})[0])
        self.assertTrue(ab.guard({}, allow_unknown=True)[0])


class ReadUsageTest(unittest.TestCase):
    def test_the_reading_goes_through_a_scratch_home_and_the_owners_cache_is_never_written(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            owner = tmp / "owner"
            (owner / "nirvana").mkdir(parents=True)
            cache = owner / "nirvana" / "usage.json"
            cache.write_text('{"owner": true}')
            reading = tmp / "reading.json"
            reading.write_text(fixture("usage-20261018.json"), encoding="utf-8")
            fake = tmp / "fake-herdr.cmd"
            fake.write_text("@echo off\r\n(echo %NIRVANA_HERDR_HOME%) > \"" + str(tmp) + "\\home-seen.txt\"\r\n"
                            "(echo %*) > \"" + str(tmp) + "\\args-seen.txt\"\r\n"
                            "type \"" + str(reading) + "\"\r\n", encoding="ascii")
            scratch = tmp / "scratch"
            snap = ab.read_usage(fake, scratch, owner)
            self.assertEqual(snap["max"]["week"]["used"], 0.56)
            self.assertEqual((tmp / "home-seen.txt").read_text().strip(), str(scratch))
            self.assertIn("usage --json --refresh", (tmp / "args-seen.txt").read_text())
            self.assertEqual(cache.read_text(), '{"owner": true}')
            self.assertEqual((scratch / "nirvana" / "usage.json").read_text(), '{"owner": true}')


class TruthAdapterTest(unittest.TestCase):
    def test_each_scenarios_truth_gets_the_flags_it_reads(self):
        doc = ab.load_scenarios()
        kw = dict(tree=Path("t"), base="b", branch="master", answer=Path("a.txt"), run_dir=Path("r"))
        m1 = ab.truth_argv(ab.scenario(doc, "M1"), served=None, **kw)
        self.assertEqual(Path(m1[2]).name, "M1.py")
        self.assertNotIn("--served", m1)
        self.assertNotIn("--branch", m1)
        m2 = ab.truth_argv(ab.scenario(doc, "M2"), served=Path("s.json"), **kw)
        self.assertIn("--served", m2)
        self.assertEqual(m2[m2.index("--served") + 1], "s.json")
        self.assertNotIn("--branch", m2)
        m5 = ab.truth_argv(ab.scenario(doc, "M5"), served=None, **kw)
        self.assertIn("--branch", m5)
        self.assertEqual(m5[m5.index("--branch") + 1], "master")
        self.assertEqual(m5[1], "-I")

    def test_a_truth_output_is_counted_rule_by_rule(self):
        out = "PASS a\nFAIL b -- why\nPASS c\nRESULT FAIL (1 of 3 failed)\n"
        self.assertEqual(ab.parse_truth(out, 1), {"result": "FAIL", "passed": 2, "failed": ["b"], "exit": 1})
        self.assertEqual(ab.parse_truth("PASS a\nRESULT PASS (1 checks)\n", 0)["result"], "PASS")

    def test_a_truth_that_did_not_conclude_is_an_error_never_a_pass_or_a_fail(self):
        self.assertEqual(ab.parse_truth("usage error: x", 2)["result"], "ERROR")
        self.assertEqual(ab.parse_truth("PASS a\n", 0)["result"], "ERROR", "no RESULT line")
        self.assertEqual(ab.parse_truth("", 1)["result"], "ERROR")

    def test_the_real_m1_truth_passes_a_good_answer_and_fails_an_empty_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            tree = Path(tmp) / "t"
            for rel in ("CLAUDE.md", "editor/server/main.py", "editor/CONTRACT.md"):
                (tree / rel).parent.mkdir(parents=True, exist_ok=True)
                (tree / rel).write_text("x\n")
            g = ["git", "-C", str(tree)]
            subprocess.run(["git", "init", "-q", "-b", "master", str(tree)], check=True)
            subprocess.run(g + ["add", "."], check=True)
            subprocess.run(g + ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base"], check=True)
            base = subprocess.run(g + ["rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
            run = Path(tmp) / "run"
            run.mkdir()
            sc = ab.scenario(ab.load_scenarios(), "M1")
            good = run / "good.txt"
            good.write_text("It turns 360 footage into a vertical Instagram reel. Start the editor with "
                            "python editor/server/main.py --port 8770; presets live in angles/ and marks/.")
            ok = ab.run_truth(sc, run, tree, base, "master", good, None)
            self.assertEqual(ok["result"], "PASS", (run / "truth.txt").read_text())
            empty = run / "empty.txt"
            empty.write_text("")
            bad = ab.run_truth(sc, run, tree, base, "master", empty, None)
            self.assertEqual(bad["result"], "FAIL")
            self.assertIn("answer_not_empty", bad["failed"])


class SeatsAndTableTest(unittest.TestCase):
    def test_seats_come_from_the_receipt_one_line_per_order(self):
        seats = ab.seats_of(json.loads(fixture("receipt-o5live.json")))
        self.assertEqual(len(seats), 6)
        self.assertIn("mul: chore.script -> haiku low (claude claude-haiku-5-5) on max: sent_back", seats[0])
        self.assertEqual(ab.seats_of(None), [])
        w = ab.receipt_workers(json.loads(fixture("receipt-o5live.json")))
        self.assertEqual(w["cache_read"], 557050)
        self.assertIsNone(ab.receipt_workers({}))

    def make_run(self, out, sid, arm, k, orch, workers, passed, failed, wall, five=0.01):
        d = out / "runs" / sid / f"{arm}-{k}"
        d.mkdir(parents=True)
        rec = {"scenario": sid, "arm": arm, "k": k, "ended": "answered", "wall_s": wall, "incidents": [],
               "tokens": {"orchestrator": {"input": orch, "cache_write": 0, "cache_read": 0, "output": 9999},
                          "workers": {"input": workers, "cache_write": 0, "cache_read": 0, "output": 5}},
               "truth": {"result": "PASS" if not failed else "FAIL", "passed": passed, "failed": failed},
               "seats": ["s1"], "usage": {"delta": {"max": {"5h": five, "week": "reset"}}}}
        (d / "run.json").write_text(json.dumps(rec), encoding="utf-8")

    def test_the_table_has_medians_a_usage_ratio_and_a_verdict_from_the_run_directories(self):
        doc = ab.load_scenarios()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            for k, (wall, orch) in enumerate([(30, 100), (50, 200), (40, 300)], 1):
                self.make_run(out, "M1", "N", k, orch, 0, 9, [], wall)
            for k, (wall, orch) in enumerate([(10, 100), (30, 100), (20, 100)], 1):
                self.make_run(out, "M1", "C", k, orch, 0, 8, ["names_where_presets_or_marks_live"], wall)
            text = ab.render_table(out, doc)
        self.assertIn("Nirvana Claude (N): runs 3, median wall 40 s, median pass fraction 1, median gross 200", text)
        self.assertIn("plain Claude Code (C): runs 3, median wall 20 s", text)
        self.assertIn("usage ratio: N gross / C gross = 2.00 (target 1.5, ceiling 2): over target", text)
        self.assertIn("truth parity: N 1, C 0.888889: at parity or better", text)
        self.assertIn("names_where_presets_or_marks_live", text)
        self.assertIn("- M1/N-1: orchestrator in 100 / cache write 0 / cache read 0 / out 9,999; "
                      "workers in 0 / cache write 0 / cache read 0 / out 5", text)
        self.assertIn("+1.00 pts", text)
        self.assertIn("reset", text)
        self.assertIn("## M2", text)
        self.assertIn("No run directories.", text)

    def test_a_token_line_names_the_four_kinds_apart_for_the_orchestrator_and_for_workers(self):
        rec = {"dir": "M1/N-1", "tokens": {"orchestrator": {"input": 1, "cache_write": 2, "cache_read": 3, "output": 4},
                                           "workers": {"input": 5, "cache_write": 6, "cache_read": 7, "output": 8}}}
        self.assertEqual(ab.token_line(rec), "M1/N-1: orchestrator in 1 / cache write 2 / cache read 3 / out 4; "
                                             "workers in 5 / cache write 6 / cache read 7 / out 8")
        self.assertEqual(ab.token_line({"dir": "M1/C-1"}), "M1/C-1: tokens n.a.")

    def test_the_models_a_transcript_used_are_counted_once_per_message(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = Path(tmp) / "agent-a.jsonl"
            rows = [{"type": "assistant", "message": {"id": "m1", "model": "claude-haiku-5-5"}},
                    {"type": "assistant", "message": {"id": "m1", "model": "claude-haiku-5-5"}},
                    {"type": "assistant", "message": {"id": "m2", "model": "claude-sonnet-5-5"}},
                    {"type": "user", "message": {"id": "m3", "model": "ignored"}}]
            f.write_text("\n".join(json.dumps(r) for r in rows), encoding="utf-8")
            self.assertEqual(ab.models_of([f]), {"claude-haiku-5-5": 1, "claude-sonnet-5-5": 1})
            self.assertEqual(ab.models_of([Path(tmp) / "missing.jsonl"]), {})

    def test_the_table_says_the_pool_readings_are_account_wide(self):
        with tempfile.TemporaryDirectory() as tmp:
            text = ab.render_table(Path(tmp), ab.load_scenarios())
        self.assertIn("whole-percent readings of the whole account", text)

    def test_a_median_over_runs_one_of_which_is_unmeasured_is_not_measured(self):
        doc = ab.load_scenarios()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            for k in (1, 2, 3):
                self.make_run(out, "M1", "N", k, 100, 0, 9, [], 30)
                self.make_run(out, "M1", "C", k, 100, 0, 9, [], 30)
            rj = out / "runs" / "M1" / "C-3" / "run.json"
            rec = json.loads(rj.read_text())
            rec["truth"] = {"result": "ERROR", "passed": 0, "failed": []}
            del rec["tokens"]
            rj.write_text(json.dumps(rec))
            text = ab.render_table(out, doc)
        self.assertIn("truth parity: not measured", text)
        self.assertIn("usage ratio: not measured", text)
        self.assertIn("| M1/C-3 | C | answered | 30 | ERROR (0 pass, 0 fail) |", text)

    def test_output_tokens_are_not_in_the_gross_and_workers_are(self):
        self.assertEqual(ab.gross({"input": 1, "cache_write": 2, "cache_read": 3, "output": 1000}), 6)
        self.assertEqual(ab.run_gross({"tokens": {"orchestrator": {"input": 6}, "workers": {"cache_read": 4}}}), 10)
        self.assertIsNone(ab.run_gross({}))

    def test_too_few_runs_is_no_verdict_and_a_run_with_no_json_is_listed_as_a_gap(self):
        doc = ab.load_scenarios()
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.make_run(out, "M1", "N", 1, 100, 0, 9, [], 30)
            (out / "runs" / "M1" / "C-1").mkdir(parents=True)
            text = ab.render_table(out, doc)
        self.assertIn("verdict: insufficient runs (n<3); N 1, C 1", text)
        self.assertIn("no run.json", text)

    def test_a_run_without_a_truth_verdict_has_no_fraction(self):
        self.assertIsNone(ab.frac({}))
        self.assertIsNone(ab.frac({"truth": {"result": "ERROR", "passed": 3, "failed": []}}))
        self.assertEqual(ab.frac({"truth": {"result": "FAIL", "passed": 3, "failed": ["x"]}}), 0.75)


if __name__ == "__main__":
    unittest.main()
