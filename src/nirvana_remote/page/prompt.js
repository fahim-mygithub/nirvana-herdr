// Screen 4, an agent's prompt as buttons, and the Esc button (plan 39 §2.4, prototype screen 4). When herdr marks the
// agent blocked ("needs you"), the menu read off its screen opens in a sheet; a tap sends that option's key. A
// screen that does not read as a menu is shown as it is, with 1 to 9, Enter and Esc. Each tap carries `seen`, the
// server's hash of what this sheet shows: when the PC has answered or the prompt has changed, nothing is pressed and
// the sheet says so, or shows the new prompt (prompt.rs). Esc stops a working agent, the key the PC sends.
//
// chat.js makes one per chat screen: nv.prompts(id, agent, say) gives { answer, esc, status(s), picker(on), open(data,
// why), watch(), close() }; the two buttons go in the composer row, status runs on each draw of the agent's state,
// close as the screen leaves.
//
// Pickers (alpha 0.68.0). The owner, 2026-11-10, sent `/resume` and then "1": the PC opened Claude Code's session
// picker, the phone had no way to move in it, and the "1" was typed in as a search. A picker (prompt.rs `picker`) now
// gets its own sheet: the PC's list with the highlighted row marked, the arrows, Enter, Esc and Backspace, and a
// search box where the picker has one. The sheet stays open while the owner moves, and reads the PC again after
// each key. It opens by itself when the live screen shows a picker (`picker`), after a slash command (`watch`), and
// when a message is refused because a picker is open (`open`); the Keys button opens it again.
(function () {
  'use strict';
  const nv = window.Nirvana;
  const h = nv.h;

  nv.prompts = function (id, agent, say) {
    let status = '';
    let body = null;
    let pressed = false;
    let over = false;

    const esc = h('button', { class: 'press esc-button mono', type: 'button', text: 'Esc', hidden: true,
      'aria-label': 'Stop the agent: Esc, as on the PC' });
    const answer = h('button', { class: 'press answer-button', type: 'button', text: 'Answer', hidden: true });

    function open() { return !!(body && body.isConnected); }

    function sheetClose() {
      if (body && body.close) body.close();
      body = null;
    }

    // One press per prompt (review S2): every key of the sheet is off from the tap until the answer, so a double tap
    // cannot press a second key into whatever Claude draws next. The server refuses a second press of the same seen
    // as well, which covers a second phone.
    function hold(on) {
      if (body) body.querySelectorAll('button').forEach((b) => { b.disabled = on; });
    }

    async function press(key, seen, note, menu) {
      note.textContent = '';
      hold(true);
      const r = await nv.postJson('/api/key', { agent: id, key, seen })
        .catch(() => ({ ok: false, status: 0, data: {} }));
      if (r.ok) { pressed = true; if (menu && key !== 'esc') after(key, seen); else sheetClose(); return; }
      const now = r.data.prompt;
      if (r.status === 409 && now && now.blocked) { show(now, 'The prompt changed on the PC: this is the one there now.'); return; }
      if (r.status === 409 && now) { sheetClose(); say('Answered on the PC.'); return; }
      hold(false);
      note.textContent = r.data.error || 'Not sent: the PC did not answer.';
    }

    // After a menu's digit (review N1): read again and draw what the PC asks now instead of closing, so a
    // multi-select can go on; close when the agent no longer asks. Claude redraws after a key with some delay (not
    // measured), so a read that still shows the pressed prompt is tried a few times before the sheet closes.
    async function after(key, seen) {
      for (let i = 0; i < 4; i++) {
        await new Promise((go) => setTimeout(go, 400));
        if (over || !open()) return;
        const data = await nv.getJson('/api/prompt?agent=' + encodeURIComponent(id), { quiet: true }).catch(() => null);
        if (!open()) return;
        if (!data || !data.blocked) { sheetClose(); return; }
        if (data.seen !== seen) { show(data, 'Sent ' + key + '. This is what the PC asks now.'); return; }
      }
      sheetClose();
    }

    // ---- A picker ----
    const PICKER_REST = 350;
    const PAD = [['up', '\u2191'], ['down', '\u2193'], ['backspace', '\u232B'], ['esc', 'Esc'], ['enter', 'Enter']];

    // The PC's rows, the highlighted one (`\u276F`) marked so the owner sees where Enter lands.
    function pickerRows(screen) {
      return (screen || '').split('\n').map((line) => h('span', {
        class: 'pick-line' + (/^\s*\u276F/.test(line) ? ' sel' : ''), text: line + '\n' }));
    }

    function showPicker(data, why) {
      if (over) return;
      sheetClose();
      let seen = data.seen;
      let busy = false;
      nv.sheet((close) => {
        const note = h('span', { class: 'note prompt-note', role: 'status', text: why || '' });
        const first = (data.screen || '').split('\n').map((l) => l.trim()).find((l) => l && !/^[\u256D\u2570\u2502]/.test(l));
        const box = h('pre', { class: 'prompt-box mono pick-box' }, ...pickerRows(data.screen));
        const keys = PAD.map(([k, label]) => h('button', { class: 'press pad-key mono' + (label.length > 1 ? ' wide' : ''),
          type: 'button', 'aria-label': k, text: label, onclick: () => key(k) }));
        const search = h('input', { class: 'pick-search', type: 'search', autocomplete: 'off', enterkeyhint: 'search',
          placeholder: 'Search the list' });
        const searchRow = data.search ? h('div', { class: 'pick-search-row' }, search,
          h('button', { class: 'press pick-search-go', type: 'button', text: 'Search', onclick: () => typeSearch() })) : null;
        search.addEventListener('keydown', (event) => {
          if (event.key !== 'Enter' || event.isComposing || event.keyCode === 229) return;
          event.preventDefault();
          typeSearch();
        });

        function update(now) {
          seen = now.seen;
          box.replaceChildren(...pickerRows(now.screen));
          const sel = box.querySelector('.sel');
          if (sel) sel.scrollIntoView({ block: 'nearest' });
        }

        // Read the PC again: the picker as it is now, or closed when it is gone (Enter opened what was picked).
        async function refresh(said) {
          await new Promise((go) => setTimeout(go, PICKER_REST));
          if (over || !open()) return;
          const now = await nv.getJson('/api/prompt?agent=' + encodeURIComponent(id), { quiet: true }).catch(() => null);
          if (!open()) return;
          if (now && now.picker) { update(now); return; }
          if (now && now.blocked) { show(now, 'The PC asks this now.'); return; }
          sheetClose();
          if (said) say(said);
        }

        async function key(k) {
          if (busy) return;
          busy = true;
          note.textContent = '';
          const r = await nv.postJson('/api/key', { agent: id, key: k, seen })
            .catch(() => ({ ok: false, status: 0, data: {} }));
          busy = false;
          if (r.ok) { pressed = true; await refresh(k === 'enter' ? 'Sent Enter: the PC took the pick.' : k === 'esc' ? 'Closed the picker on the PC.' : ''); return; }
          const now = r.data.prompt;
          if (r.status === 409 && now && now.picker) { update(now); note.textContent = 'The picker changed on the PC: tap again.'; return; }
          if (r.status === 409 && now) { sheetClose(); say('The picker closed on the PC.'); return; }
          note.textContent = r.data.error || 'Not sent: the PC did not answer.';
        }

        async function typeSearch() {
          const text = search.value.trim();
          if (!text || busy) return;
          busy = true;
          const r = await nv.postJson('/api/type', { agent: id, text, seen })
            .catch(() => ({ ok: false, status: 0, data: {} }));
          busy = false;
          if (r.ok) { search.value = ''; note.textContent = 'Typed. Pick with the arrows and Enter.'; await refresh(''); return; }
          const now = r.data.prompt;
          if (r.status === 409 && now && now.picker) { update(now); note.textContent = 'The picker changed on the PC: search again.'; return; }
          note.textContent = r.data.error || 'Not typed: the PC did not answer.';
        }

        const el = h('div', { class: 'sheet-body prompt' },
          h('div', { class: 'prompt-head' }, h('span', { class: 'prompt-dot pulse-p' }),
            h('span', { class: 'prompt-title', text: first || 'A picker is open on the PC' })),
          box,
          h('span', { class: 'prompt-sub', text: 'Move with the arrows: the marked row is the one Enter picks.' }),
          searchRow,
          h('div', { class: 'keypad pick-pad' }, ...keys),
          note);
        el.close = close;
        el.picker = true;
        body = el;
        requestAnimationFrame(() => { const sel = box.querySelector('.sel'); if (sel) sel.scrollIntoView({ block: 'nearest' }); });
        return el;
      });
    }

    function show(data, why) {
      if (over) return;
      if (data.picker) { showPicker(data, why); return; }
      sheetClose();
      const a = agent();
      nv.sheet((close) => {
        const note = h('span', { class: 'note prompt-note', role: 'status', text: why || '' });
        const menu = data.menu;
        const lines = menu ? menu.lines.slice() : [];
        // The question is the heading; the lines above it (the command, the tool) go in the box.
        const asks = menu && lines.length && /\?$/.test(lines[lines.length - 1]) ? lines.pop() : '';
        const title = asks || (a.name || 'The agent') + ' needs you';
        const shown = menu ? lines.join('\n') : data.screen;
        const keys = menu
          ? menu.options.map((o, i) => h('button', { class: 'press answer-option' + (i === 0 ? ' first' : ''), type: 'button',
            onclick: () => press(o.key, data.seen, note, true) }, h('span', { class: 'answer-key mono', text: o.key }), h('span', { text: o.label })))
          : [h('div', { class: 'keypad' }, ...data.keys.map((k) => h('button', { class: 'press pad-key mono' + (k.length > 1 ? ' wide' : ''), type: 'button',
            text: k === 'enter' ? 'Enter' : k === 'esc' ? 'Esc' : k, onclick: () => press(k, data.seen, note) })))];
        const el = h('div', { class: 'sheet-body prompt' },
          h('div', { class: 'prompt-head' }, h('span', { class: 'prompt-dot pulse-p' }), h('span', { class: 'prompt-title', text: title })),
          shown ? h('pre', { class: 'prompt-box mono', text: shown }) : null,
          h('span', { class: 'prompt-sub', text: menu
            ? 'In ' + (a.space || 'this space') + ', read off the PC’s prompt.'
            : 'Not read as a menu: this is the PC’s screen, and each key goes to it as pressed.' }),
          h('div', { class: 'answer-list' }, ...keys),
          menu ? h('button', { class: 'press answer-option esc-option', type: 'button', onclick: () => press('esc', data.seen, note) },
            h('span', { class: 'answer-key mono', text: 'Esc' }), h('span', { text: 'Cancel the prompt' })) : null,
          note);
        el.close = close;
        body = el;
        return el;
      });
    }

    // Quiet (P39o review round, S1): a failed read leaves nothing stale, and must not have the chat drawn again when
    // the PC is back (the owner's draft and pending attachments would go).
    async function read() {
      const data = await nv.getJson('/api/prompt?agent=' + encodeURIComponent(id), { quiet: true }).catch(() => null);
      if (data && data.blocked && status === 'needs') show(data);
      else if (data && data.picker && !(open() && body.picker)) show(data);
      return data;
    }

    // The Answer button is Keys while a picker is open and nothing is asked.
    let picking = false;
    function drawAnswer() {
      answer.hidden = !(status === 'needs' || picking);
      answer.textContent = status !== 'needs' && picking ? 'Keys' : 'Answer';
    }

    // Each draw of the agent's state. Into "needs you": the sheet opens with the prompt. Out of it with the sheet
    // still open and no tap from here: the PC answered.
    function draw(now) {
      const was = status;
      status = now;
      esc.hidden = now !== 'working';
      if (now === 'working') picking = false;
      drawAnswer();
      if (now === 'needs' && was !== 'needs') { pressed = false; read(); }
      if (now !== 'needs' && was === 'needs' && open() && !body.picker) { sheetClose(); if (!pressed) say('Answered on the PC.'); }
    }

    // The live screen's hint (live.rs `picker_hint`). Into a picker: its sheet opens. Out of one with its sheet open:
    // read again, which closes the sheet when the picker really is gone.
    function pickerHint(on) {
      const was = picking;
      picking = !!on && status !== 'working';
      drawAnswer();
      if (picking && !was && !(open() && body.picker)) read();
      if (!picking && was && open() && body.picker) read().then((data) => { if (!data || !data.picker) sheetClose(); });
    }

    // After a slash command: the PC may open a picker the stream does not tell of (its status need not change), so
    // the page asks a few times.
    async function watch() {
      for (const wait of [700, 1500, 3000]) {
        await new Promise((go) => setTimeout(go, wait));
        if (over || (open() && body.picker)) return;
        const data = await read();
        if (data && (data.picker || data.blocked)) { picking = !!data.picker; drawAnswer(); return; }
      }
    }

    answer.addEventListener('click', read);
    // One Esc per stop (review S4): the button is off while the key is on its way and for ESC_REST after it went, so
    // a second tap cannot reach a Claude that already stopped (Esc Esc there clears the PC's input or opens rewind).
    // The server refuses a second Esc to the same pane within the same time (prompt.rs SPENT_FOR).
    const ESC_REST = 3000;
    esc.addEventListener('click', async () => {
      esc.disabled = true;
      const r = await nv.postJson('/api/key', { agent: id, key: 'esc', seen: null })
        .catch(() => ({ ok: false, status: 0, data: {} }));
      if (r.ok) { say('Esc sent: the agent stops its turn.'); setTimeout(() => { esc.disabled = false; }, ESC_REST); return; }
      esc.disabled = false;
      if (r.status === 409 && r.data.prompt && r.data.prompt.blocked) { show(r.data.prompt, 'It is asking on the PC now.'); return; }
      say(r.data.error || 'Not sent: the PC did not answer.');
    });

    return {
      answer, esc, status: draw, picker: pickerHint, watch,
      open(data, why) { picking = !!data.picker; drawAnswer(); show(data, why); },
      close() { over = true; sheetClose(); },
    };
  };
})();
