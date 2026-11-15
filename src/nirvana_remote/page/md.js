// The agent's markdown, drawn as DOM nodes (feature phone-markdown). The owner, 2026-11-14: "Ive noticed for the
// nirvana app the text formatting is not rendered. We have alot spacing and bolding in normal claude code agent but it
// doesnt read as easily and maybe ignored in Nirvana phone app." An agent bubble was its text in a pre-wrap box, so
// **bold** read as asterisks and a table as a wall of pipes.
//
// The agent's text is untrusted for markup: nothing in it becomes HTML. Every node is made by the `h` passed in
// (nv.h on the page, a stand-in in nirvana/test_phonemd.py), every value goes in as text, and the only attributes set
// are this file's own, plus an http(s) link's href. No innerHTML: the page's CSP is default-src 'self'.
//
// What it reads is what Claude Code agents write: paragraphs, # to ###### headings, **bold**, *italic* and _italic_,
// `code`, fenced code, - * + and 1. lists nested by indent, > quotes, --- rules, pipe tables, [text](http/https url)
// and bare http(s) URLs. Anything else stays the text it was: a character it does not read as a mark is never dropped.
// A mark with no closer (an unclosed ** or ` in a message cut at chat.rs TEXT_MOST) is shown as written, and a fence
// with no closer runs to the end of its own message only, as each message is drawn on its own.
(function () {
  'use strict';

  // Quotes and lists inside quotes and lists: past this depth the rest is a plain paragraph of its lines, so a message
  // of 2000 '>' cannot run the stack out.
  const DEEPEST = 12;
  // Backslash escapes, only for the marks this file reads. Not every punctuation mark, as CommonMark has it: the owner's
  // paths are Windows ones, and C:\.claude or \\server would lose a backslash.
  const ESCAPABLE = '*_`';

  const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
  const HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
  const RULE = /^ {0,3}([-*_])(?:[ \t]*\1){2,}[ \t]*$/;
  const QUOTE = /^ {0,3}> ?(.*)$/;
  const ITEM = /^( *)([-*+]|\d{1,9}[.)])( +|$)(.*)$/;
  const DELIMITER = /^ *\|? *:?-+:? *(\| *:?-+:? *)*\|? *$/;
  const URL = /^https?:\/\/[^\s<>"]+$/i;

  // Leading tabs as 4 spaces, so indents compare as columns.
  function untab(line) {
    const lead = /^[ \t]*/.exec(line)[0];
    return lead.indexOf('\t') < 0 ? line : lead.replace(/\t/g, '    ') + line.slice(lead.length);
  }

  function indent(line) { return /^ */.exec(line)[0].length; }
  function blank(line) { return /^\s*$/.test(line); }

  // An item's marker: its indent, whether ordered, its number, and the column its text starts at.
  function item(line) {
    const m = ITEM.exec(line);
    if (!m || RULE.test(line)) return null;
    const ordered = /\d/.test(m[2]);
    // Five or more spaces after the marker: the text starts one space in (CommonMark's indented code rule).
    const gap = m[3].length >= 5 ? 1 : Math.max(1, m[3].length);
    return { indent: m[1].length, ordered, start: ordered ? parseInt(m[2], 10) : 1, col: m[1].length + m[2].length + gap,
      text: m[3].length >= 5 ? m[3].slice(1) + m[4] : m[4] };
  }

  function cells(line) {
    let s = line.trim();
    if (s.startsWith('|')) s = s.slice(1);
    if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
    const out = [];
    let cell = '';
    for (let i = 0; i < s.length; i++) {
      if (s[i] === '\\' && s[i + 1] === '|') { cell += '|'; i++; } else if (s[i] === '|') { out.push(cell.trim()); cell = ''; } else cell += s[i];
    }
    out.push(cell.trim());
    return out;
  }

  function table(lines, i) {
    if (i + 1 >= lines.length || lines[i].indexOf('|') < 0 || !DELIMITER.test(lines[i + 1]) || lines[i + 1].indexOf('-') < 0) return false;
    const head = cells(lines[i]);
    const align = cells(lines[i + 1]);
    // GFM: the delimiter row names as many columns as the head, or it is no table. One column needs a pipe somewhere.
    if (head.length !== align.length || (head.length === 1 && lines[i + 1].indexOf('|') < 0)) return false;
    return true;
  }

  // Whether the line starts a block other than a paragraph, so a paragraph (or a lazy list line) ends before it.
  function starts(lines, i) {
    const line = lines[i];
    return FENCE.test(line) || HEADING.test(line) || RULE.test(line) || QUOTE.test(line) || item(line) !== null || table(lines, i);
  }

  // --- Inline: code spans, links, bare URLs, emphasis, line breaks ---------------------------------------------------

  // The end of the code span opening at `i` (the index after its closing run), or -1.
  function codeEnd(s, i) {
    let n = 0;
    while (s[i + n] === '`') n++;
    for (let j = i + n; j < s.length;) {
      if (s[j] !== '`') { j++; continue; }
      let m = 0;
      while (s[j + m] === '`') m++;
      if (m === n) return j + m;
      j += m;
    }
    return -1;
  }

  // The closer for the emphasis mark `mark` opening at `from`, skipping code spans; -1 when there is none.
  function closer(s, from, mark) {
    const n = mark.length;
    for (let j = from; j < s.length; j++) {
      const c = s[j];
      if (c === '\\' && ESCAPABLE.indexOf(s[j + 1]) >= 0) { j++; continue; }
      if (c === '`') { const end = codeEnd(s, j); if (end > 0) { j = end - 1; continue; } let k = j; while (s[k + 1] === '`') k++; j = k; continue; }
      if (s.startsWith(mark, j) && j > from && !/\s/.test(s[j - 1])) {
        const after = s[j + n];
        // A single * or _ that is half of a ** or __ is not its closer.
        if (n === 1 && (s[j + 1] === mark || s[j - 1] === mark)) continue;
        // ***x***: the ** closes at the run's end, leaving the inner *x* whole.
        if (n === 2 && after === mark[0]) continue;
        // _ closes only at a word's end, so snake_case_names stay whole.
        if (mark[0] === '_' && after !== undefined && /[\p{L}\p{N}]/u.test(after)) continue;
        return j;
      }
    }
    return -1;
  }

  function opens(s, i, mark) {
    const after = s[i + mark.length];
    if (after === undefined || /\s/.test(after)) return false;
    if (mark.length === 1 && after === mark) return false;
    if (mark[0] === '_' && i > 0 && /[\p{L}\p{N}]/u.test(s[i - 1])) return false;
    return true;
  }

  // `[text](url)` at `i`: [the text, the url, the index after it], or null.
  function link(s, i) {
    let depth = 0;
    let j = i;
    for (; j < s.length; j++) {
      if (s[j] === '[') depth++;
      else if (s[j] === ']' && --depth === 0) break;
      else if (s[j] === '\n') return null;
    }
    if (j >= s.length || s[j + 1] !== '(') return null;
    let k = j + 2;
    let parens = 1;
    for (; k < s.length; k++) {
      if (s[k] === '(') parens++;
      else if (s[k] === ')' && --parens === 0) break;
      else if (/\s/.test(s[k])) return null;
    }
    if (k >= s.length) return null;
    return [s.slice(i + 1, j), s.slice(j + 2, k), k + 1];
  }

  // A bare URL at `i`, without the punctuation that ends the sentence around it.
  function bare(s, i) {
    if (i > 0 && /[\p{L}\p{N}]/u.test(s[i - 1])) return null;
    if (!/^https?:\/\//i.test(s.slice(i, i + 8))) return null;
    const m = /^https?:\/\/[^\s<>"`]+/i.exec(s.slice(i, i + 2048));
    if (!m) return null;
    let url = m[0];
    while (/[.,;:!?'*_]$/.test(url) || (url.endsWith(')') && (url.match(/\(/g) || []).length < (url.match(/\)/g) || []).length)) url = url.slice(0, -1);
    return /^https?:\/\/[^/]/i.test(url) ? url : null;
  }

  function anchor(h, url, children) {
    return h('a', { class: 'md-link', href: url, rel: 'noopener noreferrer', target: '_blank' }, ...children);
  }

  function inline(h, s, links) {
    const out = [];
    let text = '';
    const flush = () => { if (text) { out.push(text); text = ''; } };
    for (let i = 0; i < s.length;) {
      const c = s[i];
      if (c === '\\' && ESCAPABLE.indexOf(s[i + 1]) >= 0) { text += s[i + 1]; i += 2; continue; }
      if (c === '\n') { flush(); out.push(h('br')); i++; continue; }
      if (c === '`') {
        const end = codeEnd(s, i);
        let n = 0;
        while (s[i + n] === '`') n++;
        if (end < 0) { text += s.slice(i, i + n); i += n; continue; }
        let code = s.slice(i + n, end - n).replace(/\n/g, ' ');
        if (code.length >= 2 && code[0] === ' ' && code[code.length - 1] === ' ' && code.trim()) code = code.slice(1, -1);
        flush();
        out.push(h('code', { class: 'md-code-span', text: code }));
        i = end;
        continue;
      }
      if (c === '[' && links) {
        const l = link(s, i);
        if (l && URL.test(l[1])) {
          flush();
          out.push(anchor(h, l[1], l[0] ? inline(h, l[0], false) : [l[1]]));
          i = l[2];
          continue;
        }
        // Any other scheme (javascript:, data:, a file path) stays the text it was, brackets and all.
      }
      if ((c === 'h' || c === 'H') && links) {
        const url = bare(s, i);
        if (url) { flush(); out.push(anchor(h, url, [url])); i += url.length; continue; }
      }
      if (c === '*' || c === '_') {
        const mark = s[i + 1] === c ? c + c : c;
        if (opens(s, i, mark)) {
          const end = closer(s, i + mark.length, mark);
          if (end > 0) {
            flush();
            out.push(h(mark.length === 2 ? 'strong' : 'em', {}, ...inline(h, s.slice(i + mark.length, end), links)));
            i = end + mark.length;
            continue;
          }
        }
        text += mark;
        i += mark.length;
        continue;
      }
      text += c;
      i++;
    }
    flush();
    return out;
  }

  // --- Blocks --------------------------------------------------------------------------------------------------------

  function paragraph(h, lines) {
    return h('p', { class: 'md-p' }, ...inline(h, lines.map((l) => l.trim()).join('\n'), true));
  }

  // The list starting at lines[i]: [the node, the index after it].
  function list(h, lines, i, depth) {
    const first = item(lines[i]);
    const base = first.indent;
    const items = [];
    let cur = null;
    let lastBlank = false;
    let j = i;
    for (; j < lines.length; j++) {
      const line = lines[j];
      const it = item(line);
      if (it && it.indent <= base + 1) {
        if (it.ordered !== first.ordered) break;
        cur = { col: it.col, lines: [it.text] };
        items.push(cur);
        lastBlank = false;
        continue;
      }
      if (blank(line)) { cur.lines.push(''); lastBlank = true; continue; }
      const n = indent(line);
      if (n >= cur.col) { cur.lines.push(line.slice(cur.col)); lastBlank = false; continue; }
      // Indented less than the item's text but more than its marker: a nested list or a continuation written with
      // fewer spaces (Claude's "1. a\n  - b").
      if (n >= base + 2) { cur.lines.push(line.slice(n)); lastBlank = false; continue; }
      // A lazy continuation: the paragraph above goes on, unless a block starts here.
      if (!lastBlank && !starts(lines, j)) { cur.lines.push(line.trim()); continue; }
      break;
    }
    // Blank lines at the end belong after the list.
    while (j > i && blank(lines[j - 1])) {
      j--;
      const last = items[items.length - 1];
      if (last.lines.length > 1 && blank(last.lines[last.lines.length - 1])) last.lines.pop();
    }
    const props = { class: 'md-list' };
    if (first.ordered && first.start !== 1) props.start = String(first.start);
    const node = h(first.ordered ? 'ol' : 'ul', props, ...items.map((it) => h('li', { class: 'md-li' }, ...blocks(h, it.lines, depth + 1))));
    return [node, j];
  }

  function blocks(h, lines, depth) {
    const out = [];
    if (depth > DEEPEST) {
      const text = lines.join('\n').trim();
      return text ? [h('p', { class: 'md-p', text })] : [];
    }
    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (blank(line)) { i++; continue; }
      let m = FENCE.exec(line);
      if (m) {
        const mark = m[1];
        const info = m[2].trim();
        // A ``` fence's info has no backtick (GFM); otherwise the line is text, read inline.
        if (!(mark[0] === '`' && info.indexOf('`') >= 0)) {
          const lead = indent(line);
          const body = [];
          let j = i + 1;
          const end = new RegExp('^ {0,3}' + (mark[0] === '`' ? '`' : '~') + '{' + mark.length + ',}[ \\t]*$');
          for (; j < lines.length && !end.test(lines[j]); j++) body.push(lines[j].slice(Math.min(lead, indent(lines[j]))));
          out.push(h('div', { class: 'md-code' },
            info ? h('span', { class: 'md-lang mono', text: info }) : null,
            h('pre', { class: 'md-pre mono' }, h('code', { text: body.join('\n') }))));
          i = j + 1;
          continue;
        }
      }
      m = HEADING.exec(line);
      if (m) {
        const level = m[1].length;
        out.push(h('div', { class: 'md-h md-h' + Math.min(level, 4), role: 'heading', 'aria-level': String(level) }, ...inline(h, m[2] || '', true)));
        i++;
        continue;
      }
      if (RULE.test(line)) { out.push(h('hr', { class: 'md-rule' })); i++; continue; }
      if (QUOTE.test(line)) {
        const inner = [];
        for (; i < lines.length && QUOTE.test(lines[i]); i++) inner.push(QUOTE.exec(lines[i])[1]);
        out.push(h('blockquote', { class: 'md-quote' }, ...blocks(h, inner, depth + 1)));
        continue;
      }
      if (item(line)) {
        const [node, next] = list(h, lines, i, depth);
        out.push(node);
        i = next;
        continue;
      }
      if (table(lines, i)) {
        const head = cells(lines[i]);
        const align = cells(lines[i + 1]).map((c) => (c.startsWith(':') && c.endsWith(':') ? 'c' : c.endsWith(':') ? 'r' : ''));
        const rows = [];
        let j = i + 2;
        for (; j < lines.length && !blank(lines[j]) && lines[j].indexOf('|') >= 0 && !(FENCE.test(lines[j]) || QUOTE.test(lines[j])); j++) rows.push(cells(lines[j]));
        const cell = (tag, text, k) => h(tag, { class: 'md-cell' + (align[k] ? ' md-al-' + align[k] : '') }, ...inline(h, text, true));
        // A row with fewer cells than the head is padded; one with more keeps them, as text in a last cell, so nothing
        // the agent wrote is dropped.
        const fit = (row) => {
          const r = row.slice(0, head.length);
          while (r.length < head.length) r.push('');
          if (row.length > head.length) r[head.length - 1] += ' | ' + row.slice(head.length).join(' | ');
          return r;
        };
        out.push(h('div', { class: 'md-scroll' }, h('table', { class: 'md-table' },
          h('thead', {}, h('tr', {}, ...head.map((t, k) => cell('th', t, k)))),
          h('tbody', {}, ...rows.map((row) => h('tr', {}, ...fit(row).map((t, k) => cell('td', t, k))))))));
        i = j;
        continue;
      }
      const para = [];
      for (; i < lines.length && !blank(lines[i]) && (para.length === 0 || !starts(lines, i)); i++) para.push(lines[i]);
      out.push(paragraph(h, para));
    }
    return out;
  }

  // The nodes for one message, in order; `h(tag, props, ...children)` makes each, strings among the children as text.
  function render(text, h) {
    const lines = String(text || '').replace(/\r\n?/g, '\n').split('\n').map(untab);
    return blocks(h, lines, 0);
  }

  if (typeof module === 'object' && module.exports) module.exports = { render };
  if (typeof window === 'object' && window.Nirvana) window.Nirvana.md = (text) => render(text, window.Nirvana.h);
})();
