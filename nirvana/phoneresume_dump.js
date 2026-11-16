// The home-screen page's viewport fit (src/nirvana_remote/page/app.js `fit`) over a switch out and back, in node.
// nirvana/test_phoneresume.py runs it. The owner, 2026-11-16: "switch to nirvana after I switch out on iOS causes
// this bug": the column kept the keyboard's height after the keyboard was gone.
//
//   node phoneresume_dump.js <way back>
//
// The page loads as the installed app on an 844 px screen, the keyboard comes up (a visualViewport resize, which iOS
// does send), the page is hidden, and the keyboard goes while it is hidden: iOS sends no visualViewport event then.
// <way back> is the event the page gets on return. stdout is JSON: --vv-bottom and the kb class after each step.
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const SCREEN = 844;
const KEYBOARD = 336;
const way = process.argv[2] || 'none';

function target() {
  const on = {};
  return {
    on,
    addEventListener(name, fn) { (on[name] = on[name] || []).push(fn); },
    removeEventListener(name, fn) { on[name] = (on[name] || []).filter((f) => f !== fn); },
    fire(name, event) { for (const fn of (on[name] || []).slice()) fn(event || {}); },
  };
}

const props = {};
const classes = new Set();
const root = {
  style: { setProperty(k, v) { props[k] = v; } },
  classList: {
    add(c) { classes.add(c); },
    toggle(c, on) { if (on) classes.add(c); else classes.delete(c); },
    contains(c) { return classes.has(c); },
  },
};
const doc = Object.assign(target(), {
  documentElement: root,
  visibilityState: 'visible',
  getElementById() { return null; },
});
const view = Object.assign(target(), { height: SCREEN, offsetTop: 0, scale: 1 });
let frames = [];
const win = Object.assign(target(), {
  innerHeight: SCREEN,
  visualViewport: view,
  matchMedia() { return { matches: false }; },
  requestAnimationFrame(fn) { frames.push(fn); return frames.length; },
});
function flush() { const now = frames; frames = []; for (const fn of now) fn(0); }

const context = {
  window: win,
  document: doc,
  navigator: { standalone: true, onLine: true },
  location: { hash: '', href: 'https://pc/' },
  history: { pushState() {}, replaceState() {} },
  console,
  setTimeout() { return 0; },
  clearTimeout() {},
  setInterval() { return 0; },
  clearInterval() {},
  URLSearchParams,
  AbortController,
  // app.js's own visibilitychange asks the PC whether its build is current: no PC here, so the ask never ends.
  fetch() { return new Promise(() => {}); },
  Date,
  Promise,
};
win.Nirvana = null;
vm.createContext(context);
const source = fs.readFileSync(path.join(__dirname, '..', 'src', 'nirvana_remote', 'page', 'app.js'), 'utf8');
vm.runInContext(source, context, { filename: 'app.js' });

const out = [];
function note(step) { flush(); out.push({ step, bottom: props['--vv-bottom'], kb: classes.has('kb') }); }

note('loaded');
view.height = SCREEN - KEYBOARD;
view.fire('resize');
note('keyboard');
doc.visibilityState = 'hidden';
doc.fire('visibilitychange');
// Whatever hiding asked for a frame runs while the keyboard is still up (else a read at hide time would pass for a
// read on return: the first p82_resume_visible run was not caught that way).
flush();
// The keyboard goes while the page is away, with no visualViewport event.
view.height = SCREEN;
doc.visibilityState = 'visible';
if (way === 'visible') doc.fire('visibilitychange');
else if (way === 'pageshow') win.fire('pageshow', { persisted: true });
else if (way === 'resize') win.fire('resize');
else if (way === 'focusout') doc.fire('focusout');
else if (way === 'late') {
  // iOS reports the old size in the task that shows the page, and the new one a frame later.
  view.height = SCREEN - KEYBOARD;
  doc.fire('visibilitychange');
  view.height = SCREEN;
}
note('back');
process.stdout.write(JSON.stringify(out));
