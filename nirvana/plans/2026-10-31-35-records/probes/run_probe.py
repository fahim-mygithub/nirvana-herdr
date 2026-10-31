# Runs one headless agy probe with herdr's argv shape: list argv, stdin null, CLAUDE_* removed, BROWSER a no-op,
# a 180 s wall. Usage: run_probe.py <cwd> <out.jsonl> <prompt> [extra agy args...]
import os, subprocess, sys, time
cwd, out, prompt, extra = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:]
here = os.path.dirname(os.path.abspath(__file__))
noop = os.path.join(here, '..', 'bin', 'noop.cmd')
env = {k: v for k, v in os.environ.items()
       if k not in ('CODEX_THREAD_ID', 'OMPCODE', 'CLAUDECODE') and not k.startswith('CLAUDE_CODE_') and not k.startswith('HERDR_')}
env['BROWSER'] = noop; env['GH_BROWSER'] = noop
argv = ['agy', '--output-format', 'stream-json', '--model', 'gemini-3.8-flash-low', '--print-timeout', '170s',
        '--dangerously-skip-permissions', *extra, '--print=' + prompt]
t = time.time()
with open(out, 'wb') as o, open(out + '.err', 'wb') as e:
    p = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=o, stderr=e)
    try:
        code = p.wait(180)
    except subprocess.TimeoutExpired:
        p.kill(); code = 'killed at the wall'
print('exit', code, 'seconds', round(time.time() - t, 1))
