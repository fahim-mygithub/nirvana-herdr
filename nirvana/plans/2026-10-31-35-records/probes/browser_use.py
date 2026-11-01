# Read-only: per CLI, how many worker streams name bladebro / agent-browser / mcp__ tools, and in which tool calls.
import json, sys, glob, os, collections
root = sys.argv[1]
c = collections.Counter()
files = glob.glob(os.path.join(root, '*', '.nirvana', 'runs', '*', 'streams', '*.jsonl'))
for f in files:
    txt = open(f, encoding='utf-8', errors='replace').read()
    cli = 'agy' if '"step_update"' in txt or '"event": "init"' in txt or '"event":"init"' in txt else 'claude'
    c[(cli, 'streams')] += 1
    for word in ['bladebro', 'agent-browser', 'mcp__', 'call_mcp_tool', 'mcp_servers":[]', 'mcp_servers": []']:
        if word in txt:
            c[(cli, word)] += 1
for k in sorted(c):
    print(k, c[k])
