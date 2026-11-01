# Reads every agy stream (streams/<order>.jsonl whose first line is agy's init event) under Pojects, read only.
# Prints tool use per order and over all, errored tool steps, the result event's keys and status, and init fields.
# Usage: python -I streams.py <Pojects dir> [order-substring ...]
import json, glob, os, sys, collections

root = sys.argv[1]
only = sys.argv[2:]
total = collections.Counter()
errors = collections.Counter()
web_orders = []
inits = collections.Counter()
results = collections.Counter()
for path in sorted(glob.glob(os.path.join(root, '*', '.nirvana', 'runs', '*', 'streams', '*.jsonl'))):
    with open(path, encoding='utf-8', errors='replace') as f:
        lines = f.readlines()
    if not lines:
        continue
    try:
        first = json.loads(lines[0])
    except Exception:
        continue
    if first.get('event') != 'init' or 'step_update' in lines[0]:
        continue
    order = os.path.basename(path)[:-6]
    rel = path.replace('\\', '/').split('/')
    name = f'{rel[-6]}/{rel[-3]}/{order}'
    init = first.get('init', {})
    inits[(init.get('permission_mode'), len(init.get('tools', [])), 'mcp_servers' in init)] += 1
    tools = collections.Counter()
    errs = collections.Counter()
    cmds = []
    result = None
    for line in lines[1:]:
        try:
            e = json.loads(line)
        except Exception:
            continue
        s = e.get('step_update')
        if s and s.get('step_type') == 'tool':
            t = s.get('tool_name')
            st = s.get('state') or s.get('status')
            if st in (None, 'DONE', 'ERROR', 'CANCELED', 'CANCELLED') or True:
                pass
            key = (s.get('step_index'), t)
            if st in ('DONE', 'ERROR', 'CANCELED', 'CANCELLED'):
                tools[t] += 1
                if st != 'DONE':
                    errs[(t, st)] += 1
                if t == 'run_command':
                    c = (s.get('tool_info') or {}).get('parameters', {}).get('CommandLine', '')
                    cmds.append(c[:140])
        if e.get('event') == 'result' or e.get('type') == 'result':
            result = e
    total.update(tools)
    errors.update(errs)
    r = result or {}
    rr = r.get('result', r)
    res_key = (str(rr.get('status')), str(rr.get('finished_reason')), tuple(sorted(rr.keys()))[:12])
    results[res_key] += 1
    if only and not any(o in name for o in only):
        continue
    if only or tools.get('search_web') or tools.get('read_url_content'):
        web_orders.append((name, dict(tools.most_common()), dict(errs), rr.get('status'), rr.get('finished_reason')))
        if only:
            print('==', name, 'lines', len(lines))
            print('   tools', dict(tools.most_common()))
            print('   errors', dict(errs))
            print('   result keys', sorted(rr.keys()), 'status', rr.get('status'), 'finished_reason', rr.get('finished_reason'))
            for c in cmds[:15]:
                print('   cmd:', c)
if not only:
    print('init (permission_mode, tool count, has mcp_servers key):', dict(inits))
    print('tool steps finished, over all agy streams:', dict(total.most_common()))
    print('errored tool steps:', dict(errors.most_common()))
    print('result events (status, finished_reason, keys):')
    for k, v in results.most_common():
        print('  ', v, k)
    print('orders that used search_web or read_url_content:')
    for w in web_orders:
        print('  ', w[0], 'search_web', w[1].get('search_web', 0), 'read_url', w[1].get('read_url_content', 0),
              'browser_subagent', w[1].get('browser_subagent', 0), 'open_browser_url', w[1].get('open_browser_url', 0),
              'run_command', w[1].get('run_command', 0), 'status', w[3])
