"""Reads an agy stream-json transcript: {"event": X, X: {...}} lines (agy 1.4.0, measured 2026-10-16)."""
import json
import re


def parse(path):
    tools, jev, answer, usage, steps = [], 0, "", {}, 0
    for line in open(path, encoding="utf-8", errors="replace"):
        if not line.strip():
            continue
        try:
            ev = json.loads(line)
        except ValueError:
            continue
        kind = ev.get("event")
        body = ev.get(kind) or {}
        if kind == "step_update" and body.get("step_type") == "tool" and body.get("state") == "DONE":
            steps += 1
            info = str(body.get("tool_info", ""))
            name = body.get("tool_name", "?")
            m = re.search(r"'(?:CommandLine|AbsolutePath|SearchPath|Query|DirectoryPath)': '([^']{0,120})", info)
            tools.append(f"{name}:{m.group(1) if m else ''}")
            if name == "run_command" and re.search(r"jev (file|files|text)", info):
                jev += 1
        if kind == "result":
            answer = body.get("response") or ""
            usage = body.get("usage") or {}
    return {"tools": tools, "jev_commands": jev, "answer": answer,
            "input_tokens_total": int(usage.get("input_tokens") or 0),
            "cache_read_tokens": int(usage.get("cache_read_tokens") or 0),
            "output_tokens": usage.get("output_tokens"), "steps": steps}
