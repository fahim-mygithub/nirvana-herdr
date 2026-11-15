// The phone page's markdown renderer (src/nirvana_remote/page/md.js) run in node, for nirvana/test_phonemd.py.
//
//   node nirvana/phonemd_dump.js < inputs.json        -> a JSON list: each input's nodes, written out as markup
//   node nirvana/phonemd_dump.js --text < inputs.json -> a JSON list: each input's nodes as the text they read as
//   node nirvana/phonemd_dump.js --time message.txt   -> how long one render of that file takes, in ms
//
// The page's own `h` makes DOM nodes; this stand-in makes plain objects with the same contract (props.text is the
// node's text, strings among the children are text, null/false/undefined are skipped), so the tree md.js builds is the
// tree the page gets. Writing it out escapes every text and attribute value: a `<` in the output is a node md.js made,
// never one the agent's text made.
'use strict';
const fs = require('fs');
const path = require('path');
const { render } = require(path.join(__dirname, '..', 'src', 'nirvana_remote', 'page', 'md.js'));

function h(tag, props, ...children) {
  const attrs = {};
  const kids = [];
  for (const [key, value] of Object.entries(props || {})) {
    if (value === undefined || value === null || value === false) continue;
    if (key === 'text') kids.push(String(value));
    else if (key.startsWith('on')) throw new Error('md.js set an event handler: ' + key);
    else attrs[key] = value === true ? '' : String(value);
  }
  for (const child of children) {
    if (child === undefined || child === null || child === false) continue;
    kids.push(typeof child === 'object' ? child : String(child));
  }
  return { tag, attrs, kids };
}

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

function write(node) {
  if (typeof node === 'string') return esc(node);
  const attrs = Object.entries(node.attrs).map(([k, v]) => ' ' + k + '="' + esc(v) + '"').join('');
  return '<' + node.tag + attrs + '>' + node.kids.map(write).join('') + '</' + node.tag + '>';
}

if (process.argv[2] === '--time') {
  const text = fs.readFileSync(process.argv[3], 'utf8');
  const runs = 200;
  for (let i = 0; i < 20; i++) render(text, h);
  const t0 = process.hrtime.bigint();
  for (let i = 0; i < runs; i++) render(text, h);
  const ms = Number(process.hrtime.bigint() - t0) / 1e6 / runs;
  console.log(JSON.stringify({ chars: text.length, lines: text.split('\n').length, ms_per_render: +ms.toFixed(3), runs }));
} else if (process.argv[2] === '--text') {
  // What the bubble reads as text (its textContent, a <br> as a newline): every character the agent wrote that is not
  // a mark md.js read must be here.
  const textOf = (n) => (typeof n === 'string' ? n : n.tag === 'br' ? '\n' : n.kids.map(textOf).join(''));
  const inputs = JSON.parse(fs.readFileSync(0, 'utf8'));
  console.log(JSON.stringify(inputs.map((text) => render(text, h).map(textOf).join('\n'))));
} else {
  const inputs = JSON.parse(fs.readFileSync(0, 'utf8'));
  console.log(JSON.stringify(inputs.map((text) => render(text, h).map(write).join(''))));
}
