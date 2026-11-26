// The phone page's wait after Restart Nirvana (src/nirvana_remote/page/app.js `restarting`), in node. Plan 48.
// nirvana/test_phonerestart.py runs it.
//
//   node phonerestart_dump.js <scenario>
//
// app.js loads in a fake window with a small DOM, a fake clock and timers, a fake EventSource and a fake fetch whose
// /api/hello answers come from the scenario. A test screen is open with one live view. The page is told a restart
// was asked of server OLD (what the POST answered), and the clock runs. stdout is JSON, one entry per step: whether
// the wait's screen is up, whether the error screen is, how often the test screen was drawn, and how many live views
// were opened. A server's mark is its pid and its start (fix round A, src/nirvana_restart_all.rs server_mark).
//
//   back    hello answers: OLD (the old server, still up), no answer, tailscaled's 502, then NEW: the new server,
//           which Windows gave the old one's pid (fix round A: only its start differs).
//   kept    as back, on a screen the shell keeps (the chat's draft): the wait ends and the screen is not redrawn.
//   never   hello never answers: past the wait's limit the reach check takes over and draws the error screen.
//   old     hello answers OLD every time: the old server is never taken for the new one.
//   confirm fix round H: spaces.js loads after app.js and the spaces screen opens. Restart Nirvana is pressed, the
//           confirm reads hello (working: 2), Restart is pressed, the POST answers 202 naming OLD, and hello then
//           answers OLD, then NEW. Each step also says the confirm's text, the POSTs, and the marks the wait was
//           started with.
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const scenario = process.argv[2] || 'back';

// --- a small DOM ---
class Node {
  constructor(tag) {
    this.tag = tag;
    this.children = [];
    this.parent = null;
    this.attrs = {};
    this.className = '';
    this.text = '';
    this.dataset = {};
    this.style = { setProperty() {} };
    this.on = {};
    const self = this;
    this.classList = {
      add(c) { self.className = (self.className + ' ' + c).trim(); },
      remove(c) { self.className = self.className.split(' ').filter((x) => x !== c).join(' '); },
      toggle() {},
      contains(c) { return self.className.split(' ').includes(c); },
    };
  }
  set textContent(value) { this.text = String(value); this.children = []; }
  get textContent() { return this.text + this.children.map((c) => c.textContent).join(''); }
  setAttribute(k, v) { this.attrs[k] = v; }
  getAttribute(k) { return this.attrs[k]; }
  addEventListener(name, fn) { (this.on[name] = this.on[name] || []).push(fn); }
  removeEventListener() {}
  append(...nodes) {
    for (const n of nodes) {
      if (n.parent) n.parent.children = n.parent.children.filter((c) => c !== n);
      n.parent = this;
      this.children.push(n);
    }
  }
  replaceChildren(...nodes) {
    for (const c of this.children) c.parent = null;
    this.children = [];
    this.append(...nodes);
  }
  remove() {
    if (this.parent) this.parent.children = this.parent.children.filter((c) => c !== this);
    this.parent = null;
  }
  // spaces.js's sparkle asks for its marks every 250 ms; the confirm scenario draws no space.
  querySelectorAll() { return []; }
}
function textNode(value) { const n = new Node('#text'); n.text = String(value); return n; }

function target() {
  const on = {};
  return {
    on,
    addEventListener(name, fn) { (on[name] = on[name] || []).push(fn); },
    removeEventListener(name, fn) { on[name] = (on[name] || []).filter((f) => f !== fn); },
    fire(name, event) { for (const fn of (on[name] || []).slice()) fn(event || {}); },
  };
}

const nv = new Node('main');
const doc = Object.assign(target(), {
  documentElement: new Node('html'),
  visibilityState: 'visible',
  getElementById(id) { return id === 'nv' ? nv : null; },
  createElement(tag) { return new Node(tag); },
  createElementNS(_ns, tag) { return new Node(tag); },
  createTextNode: textNode,
  querySelector() { return null; },
});
const win = Object.assign(target(), { innerHeight: 844, matchMedia() { return { matches: false }; } });

// --- a fake clock and timers ---
let clock = 1000000;
let timers = [];
let nextTimer = 1;
function setTimeoutFake(fn, ms) { const id = nextTimer++; timers.push({ id, at: clock + (ms || 0), fn, every: 0 }); return id; }
function setIntervalFake(fn, ms) { const id = nextTimer++; timers.push({ id, at: clock + ms, fn, every: ms }); return id; }
function clearFake(id) { timers = timers.filter((t) => t.id !== id); }
async function settle() { for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r)); }
async function advance(ms) {
  const end = clock + ms;
  for (;;) {
    timers.sort((a, b) => a.at - b.at);
    const due = timers.find((t) => t.at <= end);
    if (!due) break;
    clock = due.at;
    if (due.every) due.at += due.every; else timers = timers.filter((t) => t !== due);
    due.fn();
    await settle();
  }
  clock = end;
  await settle();
}
const FakeDate = { now: () => clock };

// --- a fake EventSource ---
let opened = 0;
const sources = [];
class FakeEventSource {
  constructor(url) { this.url = url; this.readyState = 0; opened += 1; sources.push(this); }
  addEventListener() {}
  close() { this.readyState = 2; }
}
FakeEventSource.OPEN = 1;
FakeEventSource.CLOSED = 2;

// --- a fake fetch: /api/hello answers in order, the last one again ---
const OLD = '41-1700000000000';
const NEW = '41-1700000009000';
const ANSWERS = {
  back: [{ server: OLD }, 'none', 'gateway', { server: NEW }],
  kept: [{ server: OLD }, 'none', 'gateway', { server: NEW }],
  never: ['none'],
  old: [{ server: OLD }],
  confirm: [{ server: OLD, working: 2 }, { server: OLD }, { server: NEW }],
};
const answers = ANSWERS[scenario];
let asked = 0;
const posts = [];
function fetchFake(url, options) {
  if (options && options.method === 'POST') {
    posts.push({ url, method: options.method, body: options.body });
    const ok = url === '/api/restart';
    const json = ok ? { restarting: true, working: 2, server: OLD } : { error: 'not found' };
    return Promise.resolve({
      ok, status: ok ? 202 : 404,
      headers: { get(name) { return name === 'Content-Type' ? 'application/json' : ''; } },
      json() { return Promise.resolve(json); },
    });
  }
  // A screen's load while the PC restarts: tailscaled answers its gateway error, which is the error screen at once
  // when no restart is under way (app.js trouble, fallen).
  if (url === '/api/spaces') {
    return Promise.resolve({ ok: false, status: 502, headers: { get() { return 'text/plain'; } }, json() { return Promise.reject(new Error('not JSON')); } });
  }
  if (url !== '/api/hello') return new Promise(() => {});
  const answer = answers[Math.min(asked, answers.length - 1)];
  asked += 1;
  if (answer === 'none') return Promise.reject(new Error('no answer'));
  const json = answer === 'gateway' ? null
    : { app: 'nirvana-remote', version: '9.9.9', spaces: 1, working: answer.working || 0, server: answer.server };
  return Promise.resolve({
    ok: json !== null,
    status: json ? 200 : 502,
    headers: { get(name) { return name === 'Content-Type' ? (json ? 'application/json' : 'text/plain') : ''; } },
    json() { return json ? Promise.resolve(json) : Promise.reject(new Error('not JSON')); },
  });
}

const context = {
  window: win,
  document: doc,
  navigator: { onLine: true },
  location: { hash: '', href: 'https://pc/', reload() {} },
  history: { pushState() {}, replaceState() {} },
  sessionStorage: { getItem() { return null; }, setItem() {} },
  console,
  setTimeout: setTimeoutFake,
  clearTimeout: clearFake,
  setInterval: setIntervalFake,
  clearInterval: clearFake,
  URLSearchParams,
  AbortController,
  EventSource: FakeEventSource,
  fetch: fetchFake,
  Node,
  Date: FakeDate,
  Promise,
};
win.Nirvana = null;
vm.createContext(context);
const source = fs.readFileSync(path.join(__dirname, '..', 'src', 'nirvana_remote', 'page', 'app.js'), 'utf8');
vm.runInContext(source, context, { filename: 'app.js' });
const Nirvana = win.Nirvana;
if (scenario === 'confirm') {
  const spaces = fs.readFileSync(path.join(__dirname, '..', 'src', 'nirvana_remote', 'page', 'spaces.js'), 'utf8');
  vm.runInContext(spaces, context, { filename: 'spaces.js' });
}
// The marks the wait is started with: spaces.js calls nv.restarting on this same object.
const marks = [];
const restartingReal = Nirvana.restarting;
Nirvana.restarting = (mark) => { marks.push(mark); restartingReal(mark); };

// The first node under `node` whose own text is `text`.
function find(node, text) {
  if (node.text === text) return node;
  for (const child of node.children) {
    const hit = find(child, text);
    if (hit) return hit;
  }
  return null;
}
function press(node) {
  if (!node) throw new Error('nothing to press');
  for (const fn of (node.on.click || []).slice()) fn({});
}
function sheetText() {
  return nv.children.filter((c) => c.className === 'layer').map((c) => c.textContent).join(' ');
}

let renders = 0;
// `kept`: the screen is one the shell never draws again (the chat, which keeps the owner's draft): only its live view
// opens again.
Nirvana.screen('pcs', { keep: scenario === 'kept', render(el) { renders += 1; el.append(textNode('the test screen')); } });

function layerText(kind) {
  return nv.children.filter((c) => c.className.split(' ').includes('reach')).map((c) => c.textContent)
    .some((text) => text.includes(kind));
}
const out = [];
function note(step) {
  out.push({ step, waiting: layerText('Restarting Nirvana'), error: layerText("Can't reach your PC") || layerText('not answering'), renders, opened,
    sheet: sheetText(), posts: posts.slice(), marks: marks.slice() });
}

async function confirm() {
  Nirvana.go('spaces', 'f', {});
  await settle();
  note('open');
  press(find(nv, 'Restart Nirvana'));
  await settle();
  note('confirm');
  press(find(nv, 'Restart'));
  await settle();
  // The sheet closes in 240 ms, then the wait starts.
  await advance(300);
  note('restarted');
  await advance(2000);
  note('first ask');
  await advance(2000);
  note('back');
  process.stdout.write(JSON.stringify(out));
}

(async () => {
  if (scenario === 'confirm') return confirm();
  Nirvana.go('pcs', 'f', {});
  Nirvana.stream('/api/events', { state() {} });
  note('open');
  Nirvana.restarting(OLD);
  note('asked');
  // The live view drops with the old server: EventSource reports an error and closes.
  sources[sources.length - 1].readyState = FakeEventSource.CLOSED;
  sources[sources.length - 1].onerror();
  // And a load fails behind tailscaled's gateway.
  Nirvana.getJson('/api/spaces').catch(() => {});
  await advance(2000);
  note('first ask');
  await advance(15000);
  note('later');
  if (scenario === 'never') {
    await advance(180000);
    note('past the limit');
  } else {
    await advance(4000);
    note('end');
  }
  process.stdout.write(JSON.stringify(out));
})();
