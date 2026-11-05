// Screen 2, the spaces (plan 39 §2.1, prototype screen 2). Every space with its sparkle mark and its agents, each
// working, needs you, done or idle, live from /api/events (herdr's events.subscribe on the PC, record p39b). A tap on
// an agent opens its chat. P39d: the usage, servers and running cards above the list (cards.js), and the + that opens
// a new space on a Desktop\Pojects folder (newspace.js, prototype screen 6).
(function () {
  'use strict';
  const nv = window.Nirvana;
  const h = nv.h;

  // herdr's own sparkle (src/nirvana_spaces.rs SPARKLE_FRAMES), four frames a second while a space works.
  const SPARKLE = ['✧', '✦', '✶', '✷', '✸', '✷', '✶', '✦'];
  const LABEL = { working: 'working', needs: 'needs you', done: 'done', idle: 'idle' };

  function chip(status) {
    const word = LABEL[status] ? status : 'idle';
    return h('span', { class: 'chip ' + word },
      h('span', { class: 'chip-dot' + (word === 'working' ? ' pulse-t' : word === 'needs' ? ' pulse-p' : '') }),
      h('span', { class: word === 'working' ? 'shim' : '', text: LABEL[word] }));
  }

  nv.screen('spaces', {
    render(el, params) {
      el.classList.add('home');
      const title = h('span', { class: 'top-title', text: params.pc || 'This PC' });
      const version = h('span', { class: 'top-sub mono', text: '' });
      const back = h('button', { class: 'press icon-button', type: 'button', 'aria-label': 'Back to your PCs',
        // Opened from the PC list, back is the phone's own back; opened straight from #spaces there is none.
        onclick: () => (params.pc ? history.back() : nv.go('pcs', 'l')) }, nv.icon('back', 22, '#E9EAEB'));
      const list = h('div', { class: 'spaces' }, h('div', { class: 'skeleton round' }));
      const note = h('span', { class: 'note', role: 'status' });
      // The space the last open named: drawn with the new-row motion and scrolled to once the list has it.
      let shown = null;
      // Spaces an open found on the folder with only plain shells: nothing was started there (review S2).
      const quiet = new Set();
      let last = null;
      const pickers = window.NirvanaNewSpace;
      const add = pickers && h('button', { class: 'press icon-button new-button', type: 'button',
        'aria-label': 'Open a new space',
        onclick: () => pickers.open(nv, (space, existing, agent) => {
          shown = space;
          if (space && existing && !agent) quiet.add(space);
          if (last) draw(last);
        }) }, pickers.plusIcon());
      el.append(h('div', { class: 'top' }, back, h('span', { class: 'top-text' }, title, version), add));
      const cards = window.NirvanaCards ? window.NirvanaCards.mount(nv) : null;
      el.append(h('div', { class: 'scroll body stack' }, cards && cards.el,
        h('div', { class: 'section pad' }, h('span', { class: 'label', text: 'Spaces' }), list, note)));

      const seen = new Set();
      let first = true;
      let frame = 0;
      function draw(data) {
        last = data;
        version.textContent = data.version ? 'alpha ' + data.version : '';
        const cards = (data.spaces || []).map((space) => {
          const working = space.status === 'working';
          const mark = h('span', { class: 'mark ' + space.status + (working ? ' sparkle' : ''),
            text: working && !nv.still() ? SPARKLE[frame % SPARKLE.length] : '✦' });
          const head = h('div', { class: 'space-head' }, mark,
            h('span', { class: 'space-name', text: space.name || 'space' }));
          const agents = (space.agents || []).map((agent) => h('button', {
            class: 'press row agent', type: 'button',
            onclick: () => nv.go('chat', 'r', { agent: agent.id, name: agent.name, space: space.name }),
          }, h('span', { class: 'agent-text' },
            h('span', { class: 'agent-name', text: agent.name }),
            h('span', { class: 'agent-line', text: agent.line || '' })), chip(agent.status)));
          if (agents.length) quiet.delete(space.id);
          else agents.push(h('span', { class: 'agent-none',
            text: quiet.has(space.id) ? 'Open, no agent running. Nothing was started.' : 'No agents' }));
          // A space that came in after the first paint arrives with the prototype's new-row motion, and so does the
          // one a new-space open named, already open or not.
          const fresh = (!first && !seen.has(space.id)) || space.id === shown;
          seen.add(space.id);
          if (space.id === shown) mark.classList.add('spark');
          const card = h('div', { class: 'space' + (fresh ? ' new-row' : '') }, head, ...agents);
          if (space.id === shown) {
            shown = null;
            requestAnimationFrame(() => card.scrollIntoView({ block: 'nearest', behavior: nv.still() ? 'auto' : 'smooth' }));
          }
          return card;
        });
        if (!cards.length) cards.push(h('span', { class: 'note', text: 'No spaces open on this PC.' }));
        list.replaceChildren(...cards);
        first = false;
      }

      const spin = setInterval(() => {
        if (nv.still()) return;
        frame += 1;
        for (const mark of list.querySelectorAll('.mark.sparkle')) mark.textContent = SPARKLE[frame % SPARKLE.length];
      }, 250);
      const close = nv.stream('/api/events', {
        spaces: (data) => { note.textContent = ''; draw(data); if (cards) cards.refresh(); },
        state: (live) => { if (!live) note.textContent = 'Reconnecting to the PC…'; else note.textContent = ''; },
      });
      return () => { clearInterval(spin); close(); if (cards) cards.close(); };
    },
  });
})();
