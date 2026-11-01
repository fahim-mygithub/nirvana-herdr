# Counts agy vs claude orders in every .nirvana/runs journal under Pojects, read only.
# Usage: python -I count_runs.py <Pojects dir> [--detail]
import json, glob, os, sys, collections

root = sys.argv[1]
detail = '--detail' in sys.argv
orders = {}  # (project, run, order) -> dict
for j in sorted(glob.glob(os.path.join(root, '*', '.nirvana', 'runs', '*', 'journal.jsonl'))):
    parts = j.replace('\\', '/').split('/')
    project, run = parts[-5], parts[-2]
    for line in open(j, encoding='utf-8'):
        try:
            e = json.loads(line)
        except Exception:
            continue
        ev, oid = e.get('event'), e.get('order')
        if not oid:
            continue
        key = (project, run, oid)
        o = orders.setdefault(key, {'starts': 0, 'ends': [], 'verdicts': [], 'reports': [], 'tokens': None,
                                     'asked': 0, 'interrupted': 0})
        if ev == 'seated':
            o.update(cli=e.get('cli'), seat=e.get('seat'), model=e.get('model'), effort=e.get('effort'),
                     cls=e.get('class'), parent=e.get('parent'), needs=e.get('needs'), cwd=e.get('cwd'),
                     worktree=e.get('worktree'), at=e.get('at'))
        elif ev == 'started':
            o['starts'] += 1
        elif ev == 'ended':
            o['ends'].append((e.get('outcome'), (e.get('why') or '')[:160], e.get('exit')))
        elif ev == 'gated':
            o['verdicts'].append((e.get('verdict'), (e.get('reason') or '')[:200]))
        elif ev == 'delivered':
            r = e.get('report') or {}
            o['reports'].append((r.get('status'), (r.get('summary') or '')[:160], len(r.get('files') or []),
                                 len(r.get('tests') or [])))
        elif ev == 'usage':
            o['tokens'] = e.get('tokens')
        elif ev == 'asked':
            o['asked'] += 1
        elif ev == 'interrupted':
            o['interrupted'] += 1

def final(o):
    if o['verdicts']:
        return o['verdicts'][-1][0]
    if o['ends']:
        return 'ended:' + (o['ends'][-1][0] or '?') + ' (ungated)'
    return 'no end'

by_cli = collections.defaultdict(collections.Counter)
by_seat = collections.defaultdict(collections.Counter)
verdict_events = collections.defaultdict(collections.Counter)
ends = collections.defaultdict(collections.Counter)
for key, o in orders.items():
    cli = o.get('cli', '?')
    by_cli[cli][final(o)] += 1
    by_seat[(cli, o.get('seat'))][final(o)] += 1
    for v, _ in o['verdicts']:
        verdict_events[cli][v] += 1
    for outcome, _, _ in o['ends']:
        ends[cli][outcome] += 1

print('orders by cli:', {k: sum(v.values()) for k, v in by_cli.items()})
for cli, c in by_cli.items():
    print(f'  final state, {cli}:', dict(c))
print('gate verdict events (every gated line):')
for cli, c in verdict_events.items():
    print(f'  {cli}:', dict(c))
print('ended outcomes (every ended line):')
for cli, c in ends.items():
    print(f'  {cli}:', dict(c))
print('by seat:')
for k, c in sorted(by_seat.items(), key=lambda x: str(x[0])):
    print(f'  {k}:', dict(c))
print('projects:', dict(collections.Counter(k[0] for k in orders)))
if detail:
    print('\nagy orders in detail:')
    for key, o in sorted(orders.items(), key=lambda x: x[1].get('at') or 0):
        if o.get('cli') != 'agy':
            continue
        print('-', '/'.join(key), o.get('seat'), o.get('model'), o.get('cls'), 'starts', o['starts'],
              'worktree' if o.get('worktree') else 'no-worktree', 'needs', o.get('needs'), 'parent', o.get('parent'))
        for x in o['ends']:
            print('    ended', x)
        for x in o['reports']:
            print('    report', x)
        for x in o['verdicts']:
            print('    gated', x)
        print('    tokens', o['tokens'])

# Every sent_back line, classed by its reason's opening word, per cli; and research/media orders per cli.
print('\nsent_back reasons by class, per cli:')
sb = collections.defaultdict(collections.Counter)
for key, o in orders.items():
    for v, reason in o['verdicts']:
        if v != 'sent_back':
            continue
        kind = 'sources axis' if reason.startswith('sources:') else 'review order' if reason.startswith('review:') else 'orchestrator judgment'
        sb[o.get('cli')][kind] += 1
for cli, c in sb.items():
    print(f'  {cli}:', dict(c))
print('research.*/media.* orders per cli, final state:')
rm = collections.defaultdict(collections.Counter)
for key, o in orders.items():
    if (o.get('cls') or '').startswith(('research.', 'media.')):
        rm[o.get('cli')][final(o)] += 1
for cli, c in rm.items():
    print(f'  {cli}:', dict(c))
print('send-back children stopped by seat stop within 30 s of start, per cli:')
fast = collections.Counter()
for key, o in orders.items():
    if o.get('parent') and o['ends'] and o['ends'][-1][0] == 'stopped' and not o['reports']:
        fast[o.get('cli')] += 1
print(' ', dict(fast))
print('classes per cli:')
cc = collections.defaultdict(collections.Counter)
for key, o in orders.items():
    cc[o.get('cli')][(o.get('cls') or '?').split('.')[0]] += 1
for cli, c in cc.items():
    print(f'  {cli}:', dict(c))
print('gated orders, lesson-review class left out: final positive (accepted or pass) / gated, per cli; and with a')
print('final sent_back whose reason is the sources axis read as unknown (left out):')
for cli in ('claude', 'agy'):
    pos = gated = pos2 = gated2 = 0
    for key, o in orders.items():
        if o.get('cli') != cli or o.get('cls') == 'lesson-review' or not o['verdicts']:
            continue
        v, reason = o['verdicts'][-1]
        good = v in ('accepted', 'pass')
        gated += 1; pos += good
        if not (v == 'sent_back' and reason.startswith('sources:')):
            gated2 += 1; pos2 += good
    print(f'  {cli}: {pos}/{gated} = {pos/gated:.0%}; without sources-axis finals {pos2}/{gated2} = {pos2/gated2:.0%}')
