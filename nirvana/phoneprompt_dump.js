// The phone page's prompt sheet (src/nirvana_remote/page/prompt.js `show`) drawn in node on a small fake DOM.
// nirvana/test_phoneresume.py runs it. The owner, 2026-11-16: "Also the choice dialog doesn't read well on the app".
//
//   node phoneprompt_dump.js < prompt.json
//
// stdin is one /api/prompt answer; stdout is JSON: the sheet body's children in order (class and text), the box's
// scroll after the first frame, and the answer buttons.
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');

class El {
  constructor(tag) {
    this.tag = tag;
    this.className = '';
    this.textContent = '';
    this.children = [];
    this.attrs = {};
    this.scrollTop = 0;
    this.isConnected = true;
  }
  get scrollHeight() { return 40 * (this.textContent.split('\n').length); }
  append(...kids) { for (const k of kids) if (k != null) this.children.push(k); }
  replaceChildren(...kids) { this.children = []; this.append(...kids); }
  addEventListener() {}
  querySelectorAll() { return []; }
  querySelector() { return null; }
  scrollIntoView() {}
}

function h(tag, attrs, ...kids) {
  const el = new El(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (k === 'class') el.className = v;
    else if (k === 'text') el.textContent = v;
    else el.attrs[k] = v;
  }
  el.append(...kids);
  return el;
}

let sheetBody = null;
let frames = [];
const nv = {
  h,
  sheet(build) { sheetBody = build(() => {}); return () => {}; },
  getJson() { return new Promise(() => {}); },
  postJson() { return new Promise(() => {}); },
};
const context = {
  window: { Nirvana: nv },
  requestAnimationFrame(fn) { frames.push(fn); return frames.length; },
  setTimeout() { return 0; },
  Promise,
  console,
};
vm.createContext(context);
const source = fs.readFileSync(path.join(__dirname, '..', 'src', 'nirvana_remote', 'page', 'prompt.js'), 'utf8');
vm.runInContext(source, context, { filename: 'prompt.js' });

const data = JSON.parse(fs.readFileSync(0, 'utf8'));
const prompts = nv.prompts('7', () => ({ name: 'nirvana-2', space: 'WorkoutApp' }), () => {});
prompts.open(data);
for (const fn of frames.splice(0)) fn(0);

function text(el) { return el.textContent + el.children.map((c) => (typeof c === 'string' ? c : text(c))).join(''); }
const box = sheetBody.children.find((c) => /\bprompt-box\b/.test(c.className)) || null;
const options = [];
(function walk(el) {
  for (const c of el.children) {
    if (typeof c === 'string') continue;
    if (/\banswer-option\b|\bpad-key\b/.test(c.className)) options.push({ class: c.className, text: text(c) });
    else walk(c);
  }
}(sheetBody));
process.stdout.write(JSON.stringify({
  order: sheetBody.children.map((c) => c.className),
  title: text(sheetBody.children[0]),
  box: box && { class: box.className, atEnd: box.scrollTop === box.scrollHeight && box.scrollHeight > 0 },
  options,
}));
