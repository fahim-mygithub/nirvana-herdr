"""Level 7 live: a three-turn Claude session with herdr's Stop hook on and the lines set low.

Red lines written before the run: every turn's hook writes a compact.jsonl line with a Jev reading or an
error; the turn that switches to new work after a finished answer is the one most likely to carry a
systemMessage starting "Jev:". A turn mid-work must not.
"""
import json
import os
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRATCH = HERE.parent
HOME = SCRATCH / "nh-jev-ab"
EXE = HOME / "bin" / "herdr.exe"
D = SCRATCH / "jev-ab" / "compact-live"
shutil.rmtree(D, ignore_errors=True)
shutil.copytree(SCRATCH / "jev-ab" / "base", D)
env = dict(os.environ, NIRVANA_HERDR_HOME=str(HOME), BROWSER="true", NIRVANA_JEV_COMPACT_LINES="20000,25000,90000")
env.pop("CLAUDECODE", None)
subprocess.run([str(EXE), "jev", "on", "compact", "--cwd", str(D)], env=env, check=True)
log = HOME / "jev" / "compact.jsonl"
before = len(log.read_text(encoding="utf-8").splitlines()) if log.exists() else 0

turns = [
    "Read ledgerly/subscriptions.py and ledgerly/money.py and tell me in two sentences how a cancellation refund is computed.",
    "Now edit ledgerly/subscriptions.py so _prorated_amount uses money.round_cents instead of int(). Then I will want the same fix checked in invoices.py and taxes.py next, so do not stop there: say which other files you will look at next.",
    "Thanks, that is done. Completely different question now: what does the README say about running the tests? One sentence.",
]
session = None
for i, prompt in enumerate(turns):
    cmd = ["claude", "-p", prompt, "--model", "haiku", "--output-format", "stream-json", "--verbose",
           "--dangerously-skip-permissions"]
    if session:
        cmd += ["--resume", session]
    out = subprocess.run(cmd, cwd=D, env=env, capture_output=True, text=True, encoding="utf-8", timeout=600,
                         stdin=subprocess.DEVNULL).stdout
    messages = []
    for line in out.splitlines():
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        if ev.get("type") == "system" and ev.get("session_id"):
            session = ev["session_id"]
        blob = json.dumps(ev)
        if "Jev:" in blob and ev.get("type") != "assistant":
            messages.append({k: ev.get(k) for k in ("type", "subtype")} | {"text": blob[blob.index("Jev:"):blob.index("Jev:") + 200]})
        if ev.get("type") == "result":
            result = ev.get("result", "")[:200]
            usage = ev.get("usage", {})
    print(f"turn {i + 1}: result={result!r}")
    print(f"  usage in={usage.get('input_tokens')} cache_read={usage.get('cache_read_input_tokens')} cache_new={usage.get('cache_creation_input_tokens')}")
    print(f"  hook messages in stream: {messages}")

lines = log.read_text(encoding="utf-8").splitlines()[before:]
print("compact.jsonl lines:")
for line in lines:
    print(" ", line)
