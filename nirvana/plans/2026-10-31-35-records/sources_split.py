# For each order the gate sent back on the sources axis, splits its cited URLs into: named in a run_command
# (a script fetch the gate's word list cannot see, or a curl/iwr it should), or named nowhere a tool touched.
# Usage: python -I sources_split.py <Pojects dir>
import json, glob, os, sys, re

root = sys.argv[1]

def norm(u):
    u = u.split('#')[0].rstrip('/')
    return u

for j in sorted(glob.glob(os.path.join(root, '*', '.nirvana', 'runs', '*', 'journal.jsonl'))):
    run_dir = os.path.dirname(j)
    events = [json.loads(l) for l in open(j, encoding='utf-8') if l.strip()]
    seated = {e['order']: e for e in events if e.get('event') == 'seated'}
    reports = {}
    for e in events:
        if e.get('event') == 'delivered':
            reports[e['order']] = e.get('report') or {}
    for e in events:
        if e.get('event') != 'gated' or e.get('verdict') != 'sent_back' or not (e.get('reason') or '').startswith('sources:'):
            continue
        order = e['order']
        cli = seated.get(order, {}).get('cli')
        stream = os.path.join(run_dir, 'streams', order + '.jsonl')
        cmds, queries, fetch_tool_params = [], [], []
        if os.path.exists(stream):
            for l in open(stream, encoding='utf-8', errors='replace'):
                try:
                    x = json.loads(l)
                except Exception:
                    continue
                s = x.get('step_update') or {}
                if s.get('step_type') == 'tool' and s.get('state') == 'DONE':
                    p = (s.get('tool_info') or {}).get('parameters') or {}
                    if s.get('tool_name') == 'run_command':
                        cmds.append(p.get('CommandLine', ''))
                    else:
                        fetch_tool_params.append(json.dumps(p))
                # claude tool_use
                for b in ((x.get('message') or {}).get('content') or []):
                    if isinstance(b, dict) and b.get('type') == 'tool_use':
                        inp = b.get('input') or {}
                        if b.get('name') == 'Bash':
                            cmds.append(inp.get('command', ''))
                        else:
                            fetch_tool_params.append(json.dumps(inp))
        blob_cmd = '\n'.join(cmds)
        in_cmd = in_none = 0
        sample = []
        for src in reports.get(order, {}).get('sources') or []:
            u = norm(src.get('url', ''))
            if u and u in blob_cmd:
                in_cmd += 1
            else:
                in_none += 1
                if len(sample) < 2:
                    sample.append(u)
        n = len(reports.get(order, {}).get('sources') or [])
        print(f"{os.path.basename(os.path.dirname(os.path.dirname(run_dir)))[:16]}/{order} cli={cli} cited={n} "
              f"named-in-a-command={in_cmd} named-nowhere={in_none} sample-nowhere={sample}")
