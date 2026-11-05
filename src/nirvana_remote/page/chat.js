// Screens 3 and 5, an agent's chat and the text box (plan 39 §2.2-2.3, prototype screens 3 and 5). The bubbles come
// from the agent's own Claude transcript on the PC, each tool call folded to one line that opens on a tap; the live
// screen from herdr's agent.read; both kept current by /api/chat/events. The box sends through agent.prompt, as if
// typed on the PC; while the agent works the owner picks queue or send now. A pasted screenshot, a photo or a file
// is uploaded to the space's .nirvana/uploads/ first, and its path goes with the message. The Esc button and the
// prompt sheet are P39c's.
(function () {
  'use strict';
  const nv = window.Nirvana;
  const h = nv.h;
  const MOST = 50 * 1024 * 1024;
  const LABEL = { working: 'working', needs: 'needs you', done: 'done', idle: 'idle' };

  function size(bytes) {
    if (bytes >= 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
    return Math.max(1, Math.round(bytes / 1024)) + ' KB';
  }

  function chip(status) {
    const word = LABEL[status] ? status : 'idle';
    return h('span', { class: 'chip ' + word },
      h('span', { class: 'chip-dot' + (word === 'working' ? ' pulse-t' : word === 'needs' ? ' pulse-p' : '') }),
      h('span', { class: word === 'working' ? 'shim' : '', text: LABEL[word] }));
  }

  function option(label, sub, icon, onclick) {
    return h('button', { class: 'press row option', type: 'button', onclick },
      h('span', { class: 'option-icon' }, nv.icon(icon, 20, '#86CBD2', 1.8)),
      h('span', { class: 'option-text' }, h('span', { class: 'option-name', text: label }),
        h('span', { class: 'option-sub', text: sub })));
  }

  nv.screen('chat', {
    render(el, params) {
      const id = params.agent || '';
      el.classList.add('chat');
      let agent = { name: params.name || 'agent', space: params.space || '', model: '', status: 'idle' };
      const pending = [];
      const byId = new Map();
      const nodes = new Map();
      const opened = new Set();
      let local = [];

      // The head: back, the agent and where it runs, its state.
      const name = h('span', { class: 'top-title small', text: agent.name });
      const where = h('span', { class: 'top-sub mono', text: agent.space });
      const state = h('span', { class: 'top-chip' }, chip(agent.status));
      const back = h('button', { class: 'press icon-button', type: 'button', 'aria-label': 'Back to spaces',
        onclick: () => history.back() }, nv.icon('back', 22, '#E9EAEB'));
      el.append(h('div', { class: 'top ruled' }, back, h('span', { class: 'top-text' }, name, where), state));

      // The bubbles, newest at the bottom; the scroll box keeps the bottom in view.
      const list = h('div', { class: 'messages' });
      const live = h('pre', { class: 'live mono', hidden: true });
      const liveBox = h('button', { class: 'press live-box', type: 'button', hidden: true,
        onclick: () => liveBox.classList.toggle('open') },
      h('span', { class: 'live-label', text: 'On the PC now' }), live);
      const working = h('div', { class: 'msg at-work', hidden: true },
        h('span', { class: 'dots' }, h('span'), h('span'), h('span')),
        h('span', { class: 'shim', text: 'working' }));
      el.append(h('div', { class: 'scroll thread' }, h('div', { class: 'thread-in' }, list, working, liveBox)));

      // The box.
      const note = h('span', { class: 'note', role: 'status' });
      const chips = h('div', { class: 'pending' });
      const input = h('input', { id: 'nv-draft', class: 'draft', type: 'text', autocomplete: 'off',
        enterkeyhint: 'send', placeholder: 'Message ' + agent.name });
      const sendButton = h('button', { class: 'press send-button', type: 'button', 'aria-label': 'Send' },
        nv.icon('send', 20, '#0E1B1D', 2.2));
      const photos = h('input', { class: 'hidden-file', type: 'file', accept: 'image/*', multiple: true, tabindex: '-1' });
      const anyFile = h('input', { class: 'hidden-file', type: 'file', multiple: true, tabindex: '-1' });
      const attach = h('button', { class: 'press attach-button', type: 'button', 'aria-label': 'Attach a screenshot or file' },
        nv.icon('attach', 20, '#B0B5BA'));
      el.append(h('div', { class: 'composer' }, note, chips,
        h('div', { class: 'composer-row' }, attach,
          h('label', { for: 'nv-draft', class: 'hidden-label', text: 'Message the agent' }), input, sendButton),
        photos, anyFile));

      function drawHead() {
        name.textContent = agent.name;
        where.textContent = [agent.space, agent.model].filter(Boolean).join(' · ');
        input.placeholder = 'Message ' + agent.name;
        state.replaceChildren(chip(agent.status));
        working.hidden = agent.status !== 'working';
      }

      function bubble(m) {
        if (m.kind === 'user') return h('div', { class: 'msg bubble-user', text: m.text });
        if (m.kind === 'agent') return h('div', { class: 'msg bubble-agent', text: m.text });
        if (m.kind === 'file') {
          return h('div', { class: 'msg bubble-file' }, h('span', { class: 'file-icon' }, nv.icon('image', 20, '#86CBD2', 1.8)),
            h('span', { class: 'file-text' }, h('span', { class: 'file-name', text: m.text }),
              h('span', { class: 'file-sub mono', text: m.sub || '' })));
        }
        const open = opened.has(m.id);
        const row = h('button', { class: 'msg press tool' + (open ? ' open' : ''), type: 'button', 'aria-expanded': open ? 'true' : 'false' },
          h('span', { class: 'tool-line' },
            h('span', { class: 'caret', text: '›' }),
            h('span', { class: 'tool-name', text: m.tool }),
            h('span', { class: 'tool-arg', text: m.arg || '' }),
            h('span', { class: 'tool-result ' + (m.ok ? 'ok' : 'bad'), text: m.result || '' })),
          h('span', { class: 'tool-detail', text: m.detail || '' }));
        row.addEventListener('click', () => {
          if (opened.has(m.id)) opened.delete(m.id); else opened.add(m.id);
          const now = !row.classList.contains('open');
          row.classList.toggle('open', now);
          row.setAttribute('aria-expanded', now ? 'true' : 'false');
        });
        return row;
      }

      // Upsert by id: a bubble already drawn is replaced in place (a tool whose result came), a new one is added.
      function upsert(messages) {
        for (const m of messages) {
          const node = bubble(m);
          if (nodes.has(m.id)) {
            node.classList.remove('msg');
            nodes.get(m.id).replaceWith(node);
          } else {
            list.append(node);
          }
          nodes.set(m.id, node);
          byId.set(m.id, m);
        }
        for (const node of local) list.append(node);
      }

      function drawLive(text) {
        live.textContent = text || '';
        live.hidden = !text;
        liveBox.hidden = !text;
      }

      function drawPending() {
        chips.replaceChildren(...pending.map((p) => h('span', { class: 'msg pending-chip' + (p.state === 'bad' ? ' bad' : '') },
          nv.icon(p.image ? 'image' : 'file', 16, '#86CBD2', 1.8),
          h('span', { text: p.label + (p.state === 'up' ? ' · uploading' : ' · ' + size(p.size)) }),
          h('button', { class: 'press chip-x', type: 'button', 'aria-label': 'Remove the attachment',
            onclick: () => { pending.splice(pending.indexOf(p), 1); drawPending(); } }, nv.icon('close', 14, '#B0B5BA', 2.2)))));
        sendButton.classList.toggle('ready', !!(input.value.trim() || pending.some((p) => p.file)));
      }

      async function uploadFile(file) {
        if (file.size > MOST) { note.textContent = (file.name || 'That file') + ' is over 50 MB: not sent.'; return; }
        const image = (file.type || '').startsWith('image/');
        const p = { label: file.name || (image ? 'screenshot' : 'file'), size: file.size, state: 'up', image, file: null };
        pending.push(p);
        drawPending();
        const r = await nv.upload('/api/upload?agent=' + encodeURIComponent(id), file).catch(() => ({ ok: false, data: {} }));
        if (r.ok && r.data.file) { p.file = r.data.file; p.state = 'ok'; note.textContent = ''; }
        else { p.state = 'bad'; note.textContent = 'The upload failed: ' + (r.data.error || r.status || 'no answer'); }
        drawPending();
      }

      function filesFrom(list) { for (const file of Array.from(list || [])) uploadFile(file); }

      async function pasteScreenshot() {
        // Safari asks with its own Paste button; where it cannot, the box takes a long-press Paste.
        if (navigator.clipboard && navigator.clipboard.read) {
          try {
            for (const item of await navigator.clipboard.read()) {
              const type = item.types.find((t) => t.startsWith('image/'));
              if (type) { const blob = await item.getType(type); uploadFile(new File([blob], 'screenshot.' + type.split('/')[1], { type })); return; }
            }
            note.textContent = 'No picture on the clipboard.';
            return;
          } catch (_) { /* refused or not supported: the long-press way */ }
        }
        input.focus();
        note.textContent = 'Long-press the message box and tap Paste.';
      }

      function openAttach() {
        nv.sheet((close) => h('div', { class: 'sheet-body' },
          h('span', { class: 'sheet-title', text: 'Add to the message' }),
          option('Paste screenshot', 'From the clipboard', 'clipboard', () => close(pasteScreenshot)),
          option('Photo library', 'Pictures and screen recordings', 'image', () => close(() => photos.click())),
          option('File', 'PDF, log or anything up to 50 MB', 'file', () => close(() => anyFile.click())),
          h('span', { class: 'sheet-foot mono', text: 'Saved on the PC in .nirvana/uploads/' })));
      }

      function choose(go) {
        nv.sheet((close) => h('div', { class: 'sheet-body' },
          h('span', { class: 'sheet-title', text: agent.name + ' is working' }),
          option('Queue', 'Claude takes it when it can, without stopping its turn', 'send', () => close(() => go('queue'))),
          option('Send now', 'Stops its turn with Esc, then sends', 'close', () => close(() => go('now')))));
      }

      async function send(mode) {
        const text = input.value.trim();
        if (pending.some((p) => p.state === 'up')) { note.textContent = 'Wait for the upload to finish.'; return; }
        const ready = pending.filter((p) => p.file);
        if (!text && !ready.length) return;
        if (agent.status === 'working' && !mode) { choose(send); return; }
        note.textContent = '';
        const r = await nv.postJson('/api/send', { agent: id, text, files: ready.map((p) => p.file), mode: mode || null })
          .catch(() => ({ ok: false, status: 0, data: {} }));
        if (r.status === 409 && r.data.choose) { agent.status = 'working'; drawHead(); choose(send); return; }
        if (!r.ok) { note.textContent = r.data.error || 'Not sent: the PC did not answer.'; return; }
        if (r.data.stopped === false) note.textContent = 'It did not stop in time, so the message waits in its queue.';
        for (const p of ready) local.push(bubble({ kind: 'file', text: p.label, sub: size(p.size) + ' · ' + p.file }));
        if (text) local.push(bubble({ kind: 'user', text }));
        for (const node of local) list.append(node);
        input.value = '';
        pending.splice(0, pending.length);
        drawPending();
      }

      attach.addEventListener('click', openAttach);
      sendButton.addEventListener('click', () => send());
      input.addEventListener('keydown', (event) => { if (event.key === 'Enter') { event.preventDefault(); send(); } });
      input.addEventListener('input', drawPending);
      // A screenshot pasted straight into the box (the iOS long-press Paste) arrives as a file on the paste event.
      input.addEventListener('paste', (event) => {
        const files = event.clipboardData ? Array.from(event.clipboardData.files || []) : [];
        if (files.length) { event.preventDefault(); filesFrom(files); }
      });
      photos.addEventListener('change', () => { filesFrom(photos.files); photos.value = ''; });
      anyFile.addEventListener('change', () => { filesFrom(anyFile.files); anyFile.value = ''; });

      drawHead();
      drawPending();
      return nv.stream('/api/chat/events?agent=' + encodeURIComponent(id), {
        chat: (data) => {
          agent = Object.assign(agent, data.agent || {});
          drawHead();
          list.replaceChildren();
          nodes.clear();
          byId.clear();
          local = [];
          if (!data.transcript && !(data.messages || []).length) {
            list.append(h('span', { class: 'note', text: 'No transcript for this agent: the PC’s screen is below.' }));
          }
          upsert(data.messages || []);
          drawLive(data.live);
        },
        messages: (data) => {
          const messages = data.messages || [];
          // The transcript has the message now; the one drawn on send goes.
          if (messages.some((m) => m.kind === 'user' && !byId.has(m.id))) { for (const node of local) node.remove(); local = []; }
          upsert(messages);
        },
        agent: (data) => { agent = Object.assign(agent, data); drawHead(); },
        live: (data) => drawLive(data.live),
        gone: () => { note.textContent = 'This agent has closed on the PC.'; },
        state: (on) => { if (!on) note.textContent = 'Reconnecting to the PC…'; else if (note.textContent.startsWith('Reconnecting')) note.textContent = ''; },
      });
    },
  });
})();
