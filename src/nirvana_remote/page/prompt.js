// Screen 4, an agent's prompt as buttons, and the Esc button (plan 39 §2.4, prototype screen 4). When herdr marks the
// agent blocked ("needs you"), the menu read off its screen opens in a sheet; a tap sends that option's key. A
// screen that does not read as a menu is shown as it is, with 1 to 9, Enter and Esc. Each tap carries `seen`, the
// server's hash of what this sheet shows: when the PC has answered or the prompt has changed, nothing is pressed and
// the sheet says so, or shows the new prompt (prompt.rs). Esc stops a working agent, the key the PC sends.
//
// chat.js makes one per chat screen: nv.prompts(id, agent, say) gives { answer, esc, status(s), close() }; the two
// buttons go in the composer row, status runs on each draw of the agent's state, close as the screen leaves.
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
        const data = await nv.getJson('/api/prompt?agent=' + encodeURIComponent(id)).catch(() => null);
        if (!open()) return;
        if (!data || !data.blocked) { sheetClose(); return; }
        if (data.seen !== seen) { show(data, 'Sent ' + key + '. This is what the PC asks now.'); return; }
      }
      sheetClose();
    }

    function show(data, why) {
      if (over) return;
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

    async function read() {
      const data = await nv.getJson('/api/prompt?agent=' + encodeURIComponent(id)).catch(() => null);
      if (data && data.blocked && status === 'needs') show(data);
    }

    // Each draw of the agent's state. Into "needs you": the sheet opens with the prompt. Out of it with the sheet
    // still open and no tap from here: the PC answered.
    function draw(now) {
      const was = status;
      status = now;
      esc.hidden = now !== 'working';
      answer.hidden = now !== 'needs';
      if (now === 'needs' && was !== 'needs') { pressed = false; read(); }
      if (now !== 'needs' && was === 'needs' && open()) { sheetClose(); if (!pressed) say('Answered on the PC.'); }
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

    return { answer, esc, status: draw, close() { over = true; sheetClose(); } };
  };
})();
