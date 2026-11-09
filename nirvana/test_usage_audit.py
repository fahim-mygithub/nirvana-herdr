"""Plan 40 AUDIT: the parsers of usage_audit.py on tiny made-up fixtures (never a real transcript excerpt).

    python -m unittest nirvana.test_usage_audit
"""

import base64
import json
import sqlite3
import struct
import tempfile
import unittest
from pathlib import Path

try:
    from nirvana import usage_audit as U
except ImportError:  # run from inside nirvana/
    import usage_audit as U


def compact(x):
    """Claude Code writes its transcripts without spaces; the scanner's cheap line filter relies on it."""
    return json.dumps(x, separators=(",", ":"))


def prices():
    return U.Prices(U.load_seat_table())


def assistant(uuid, msg_id, req, model="claude-sonnet-5-5", usage=None, content=None, ts="2026-11-01T00:00:00Z",
              sid="s1", side=False):
    return {"type": "assistant", "uuid": uuid, "sessionId": sid, "timestamp": ts, "requestId": req,
            "isSidechain": side, "entrypoint": "cli",
            "message": {"id": msg_id, "model": model, "content": content or [],
                        "usage": usage or {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 100,
                                           "cache_creation_input_tokens": 20}}}


def png_b64(w, h):
    head = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", w, h) + b"\x08\x02\x00\x00\x00"
    return base64.b64encode(head + b"\x00" * 16).decode()


def jpeg_b64(w, h):
    app0 = b"\xff\xe0" + struct.pack(">H", 16) + b"JFIF\x00" + b"\x00" * 9
    sof = b"\xff\xc0" + struct.pack(">H", 17) + b"\x08" + struct.pack(">HH", h, w) + b"\x03" + b"\x00" * 9
    return base64.b64encode(b"\xff\xd8" + app0 + sof + b"\x00" * 8).decode()


class TranscriptParsing(unittest.TestCase):
    def scan(self, lines, folder="C--Users-fahim-Desktop-Pojects-groove-coach"):
        d = Path(tempfile.mkdtemp())
        p = d / "s1.jsonl"
        p.write_text("\n".join(compact(x) for x in lines) + "\n", encoding="utf-8", newline="\n")
        sc = U.TranscriptScan(prices())
        sc.scan_file(p, folder)
        return sc

    def test_one_response_on_two_lines_counts_its_usage_once_and_both_blocks(self):
        tool_a = {"type": "tool_use", "id": "t1", "name": "Read", "input": {"file_path": "a.py"}}
        tool_b = {"type": "tool_use", "id": "t2", "name": "Read", "input": {"file_path": "b.png"}}
        sc = self.scan([assistant("u1", "m1", "r1", content=[tool_a]), assistant("u2", "m1", "r1", content=[tool_b])])
        s = sc.sessions["s1"]
        self.assertEqual(len(s.turns), 1)
        self.assertEqual(s.tools["Read:text"], 1)
        self.assertEqual(s.tools["Read:image"], 1)
        self.assertEqual(sc.stats["repeated_usage_lines"], 1)

    def test_a_line_copied_into_a_second_file_counts_once(self):
        line = assistant("u1", "m1", "r1")
        sc = self.scan([line])
        d = Path(tempfile.mkdtemp())
        p = d / "s2.jsonl"
        p.write_text(compact(line) + "\n", encoding="utf-8", newline="\n")
        sc.scan_file(p, "x")
        self.assertEqual(sum(len(s.turns) for s in sc.sessions.values()), 1)
        self.assertEqual(sc.stats["dup_lines"], 1)

    def test_tool_result_size_and_image_in_result(self):
        use = assistant("u1", "m1", "r1", content=[{"type": "tool_use", "id": "t1", "name": "Bash",
                                                    "input": {"command": "ls"}},
                                                   {"type": "tool_use", "id": "t2", "name": "Read",
                                                    "input": {"file_path": "x.png"}}])
        res = {"type": "user", "uuid": "u2", "sessionId": "s1", "timestamp": "2026-11-01T00:00:01Z",
               "message": {"role": "user", "content": [
                   {"type": "tool_result", "tool_use_id": "t1", "content": "x" * 400},
                   {"type": "tool_result", "tool_use_id": "t2", "content": [
                       {"type": "image", "source": {"type": "base64", "media_type": "image/png",
                                                    "data": png_b64(750, 100)}}]}]}}
        sc = self.scan([use, res])
        self.assertEqual(sc.tool_results["Bash"], [100])
        self.assertEqual(sc.tool_results["Read:image"], [100])  # 750 x 100 / 750
        self.assertEqual(len(sc.sessions["s1"].images), 1)

    def test_nirvana_title_and_effort_by_request(self):
        req = {"type": "api-request", "id": "q1", "sessionId": "s1",
               "params": {"output_config": {"effort": "high"}}}
        title = {"type": "custom-title", "customTitle": "Nirvana", "sessionId": "s1"}
        a = assistant("u1", "m1", "r1")
        a["requestRef"] = "q1"
        sc = self.scan([req, title, a])
        s = sc.sessions["s1"]
        self.assertTrue(s.title_nirvana)
        self.assertEqual(s.turns[0][7], "high")
        self.assertEqual(U.classify(s, {}, ["C--Users-fahim-Desktop-Pojects-groove-coach"]), "fork-orchestrator")

    def test_seat_run_through_a_variable_counts(self):
        a = assistant("u1", "m1", "r1", content=[{"type": "tool_use", "id": "t1", "name": "PowerShell", "input": {
            "command": '$h = "C:/x/herdr.exe"; & $h seat run sonnet --order a'}}])
        sc = self.scan([a])
        self.assertEqual(sc.sessions["s1"].seat_runs, 1)


class Classification(unittest.TestCase):
    def test_folders(self):
        pf = [U.project_folder_name(r"C:\Users\fahim\Desktop\Pojects\3D-Printer Flow")]
        self.assertEqual(pf, ["C--Users-fahim-Desktop-Pojects-3D-Printer-Flow"])
        self.assertEqual(U.folder_kind("C--Users-fahim--nirvana-agent-ask-1", pf), "old-nirvana-transcripts")
        self.assertEqual(U.folder_kind("C--Users-fahim-AppData-Local-Temp-bench-moto-1", pf), "bench-probe")
        self.assertEqual(U.folder_kind("C--Users-fahim-Desktop-Pojects-Nirvana-Herdr", pf), "harness-build")
        self.assertEqual(U.folder_kind("C--Users-fahim-Desktop-Pojects-3D-Printer-Flow", pf), "same-project")
        self.assertEqual(U.folder_kind("C--Users-fahim-Desktop-Pojects-3D-Printer-Flow--nirvana-worktrees-a", pf),
                         "same-project")
        self.assertEqual(U.folder_kind("C--Users-fahim-Desktop-Pojects-3D-Printer-Flow-workspace", pf),
                         "other-projects")

    def test_a_journal_worker_id_wins_over_the_folder(self):
        s = U.SessionAgg("w1", "C--Users-fahim-Desktop-Pojects-3D-Printer-Flow")
        s.entry["sdk-cli"] = 3
        self.assertEqual(U.classify(s, {"w1": ("p", "r", "o")}, ["C--Users-fahim-Desktop-Pojects-3D-Printer-Flow"]),
                         "fork-worker")
        self.assertEqual(U.classify(s, {}, ["C--Users-fahim-Desktop-Pojects-3D-Printer-Flow"]), "fork-print-other")


class PricesAndImages(unittest.TestCase):
    def test_sonnet_cache_read_is_the_list_price(self):
        p = prices()
        self.assertEqual(p.row("claude-sonnet-5-5")[2], 0.10)
        self.assertAlmostEqual(p.cost("claude-sonnet-5-5", 0, 0, 1_000_000, 0), 0.10)

    def test_fable_5_2_and_one_hour_write(self):
        p = prices()
        self.assertEqual(p.row("claude-fable-5-2[1m]")[:3], (8.0, 40.0, 0.20))
        # 1M written for 1 h at Opus 5.5: 2 x $4
        self.assertAlmostEqual(p.cost("claude-opus-5-5", 0, 0, 0, 1_000_000, cw_1h=1_000_000), 8.0)
        self.assertAlmostEqual(p.cost("claude-opus-5-5", 0, 0, 0, 1_000_000), 5.0)

    def test_haiku_long_prompt_rate(self):
        p = prices()
        self.assertAlmostEqual(p.cost("claude-haiku-5-5", 1_000_000, 0, 0, 0, ctx=50_000), 0.10)
        self.assertAlmostEqual(p.cost("claude-haiku-5-5", 1_000_000, 0, 0, 0, ctx=150_000), 0.50)

    def test_norm(self):
        self.assertEqual(U.Prices.norm("claude-haiku-4-5-20251001"), "claude-haiku-4-5")
        self.assertEqual(U.Prices.norm("gemini-3.8-flash-high"), "gemini-3.8-flash")

    def test_unknown_model_is_listed_not_priced(self):
        p = prices()
        self.assertEqual(p.cost("made-up-model", 5, 5, 0, 0), 0.0)
        self.assertEqual(p.unpriced["made-up-model"], 10)

    def test_image_headers(self):
        self.assertEqual(U.image_dims(png_b64(640, 480)), (640, 480))
        self.assertEqual(U.image_dims(jpeg_b64(1024, 768)), (1024, 768))
        self.assertIsNone(U.image_dims(base64.b64encode(b"not an image at all").decode()))

    def test_image_tokens_scale_to_the_cap(self):
        self.assertEqual(U.image_tokens((750, 100), 0), 100)
        self.assertEqual(U.image_tokens((3000, 3000), 0), U.image_tokens((6000, 6000), 0))


class JournalsAndOld(unittest.TestCase):
    def test_worker_usage_is_a_running_total(self):
        ev = [{"event": "usage", "order": "a", "attempt": 1, "tokens": {"input": 1, "output": 1}},
              {"event": "usage", "order": "a", "attempt": 1, "tokens": {"input": 3, "output": 2}},
              {"event": "usage", "order": "a", "attempt": 2, "tokens": {"input": 5, "output": 5}},
              {"event": "usage", "tokens": {"input": 99}}]
        self.assertEqual(U.worker_totals(ev), {("a", 1): {"input": 3, "output": 2}, ("a", 2): {"input": 5, "output": 5}})

    def test_old_nirvana_reads_model_usage(self):
        d = Path(tempfile.mkdtemp()) / "e.sqlite"
        con = sqlite3.connect(d)
        con.execute("create table events (id integer primary key, run_id text, work_order_id text, provider text, "
                    "ts text, event_type text, tool text, tokens_in int, tokens_out int, duration_ms int, "
                    "payload_ref text, actor text)")
        con.execute("create table payloads (ref text primary key, sha256 text, bytes blob, created_ts text)")
        res = {"modelUsage": {"claude-opus-5": {"inputTokens": 1, "outputTokens": 2, "cacheReadInputTokens": 3,
                                                "cacheCreationInputTokens": 4, "costUSD": 0.5}}}
        con.execute("insert into payloads values ('p1','x',?,'t')", (json.dumps(res).encode(),))
        con.execute("insert into events values (1,'r','wo','claude','2026-09-20T00:00:00Z','result',null,null,null,"
                    "null,'p1','tailer')")
        con.execute("insert into payloads values ('p2','x',?,'t')", (json.dumps({"verdict": "PASS"}).encode(),))
        con.execute("insert into events values (2,'r','wo','claude','2026-09-20T00:00:00Z','verdict',null,null,null,"
                    "null,'p2','gate')")
        con.commit()
        con.close()
        old = U.load_old_nirvana(d)
        self.assertIsNone(old["error"])
        self.assertEqual(old["turns"][0]["cache_write"], 4)
        self.assertEqual(old["verdicts"]["PASS"], 1)


if __name__ == "__main__":
    unittest.main()
