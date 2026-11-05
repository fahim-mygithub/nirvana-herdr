// Screen 1, "Your PCs" (plan 39 §1.3, prototype screen 1). The rows come from /api/pcs: this PC first, then the
// owner's other PCs answering as Nirvana, online without it, and asleep with last seen.
(function () {
  'use strict';
  const nv = window.Nirvana;
  const h = nv.h;

  function row(pc) {
    const dot = h('span', { class: 'dot ' + pc.state + (pc.state === 'nirvana' ? ' pulse-t' : '') });
    const text = h('span', { class: 'pc-text' },
      h('span', { class: 'pc-name', text: pc.name }),
      h('span', { class: 'pc-line', text: pc.line }));
    if (pc.state !== 'nirvana') {
      return h('div', { class: 'pc' + (pc.state === 'asleep' ? ' asleep' : '') }, dot, text);
    }
    const chevron = nv.icon('chevron', 18, '#7D838A');
    chevron.classList.add('chevron');
    if (!pc.self && pc.url) {
      // Another PC's Nirvana is its own page at its own address, behind its own Tailscale check.
      return h('a', { class: 'pc press row', href: pc.url }, dot, text, chevron);
    }
    return h('button', {
      class: 'pc press row',
      type: 'button',
      onclick: () => {
        if (nv.has('spaces')) nv.go('spaces', 'r', { pc: pc.name });
        else note.textContent = "This PC's spaces open here in the next release.";
      },
    }, dot, text, chevron);
  }

  let note = null;

  nv.screen('pcs', {
    render(el) {
      el.classList.add('pcs');
      const mark = h('div', { class: 'brand-mark' }, nv.icon('lotus', 26, '#E9F7F8', 1.7));
      el.append(h('div', { class: 'brand' }, mark,
        h('div', { class: 'brand-text' },
          h('span', { class: 'brand-name', text: 'Nirvana' }),
          h('span', { class: 'brand-sub', text: 'Signed in through Tailscale' }))));
      const card = h('div', { class: 'card' }, h('div', { class: 'skeleton' }));
      note = h('span', { class: 'note', role: 'status' });
      el.append(h('div', { class: 'section' }, h('span', { class: 'label', text: 'Your PCs' }), card, note));
      el.append(h('div', { class: 'lock' }, nv.icon('lock', 16, '#8D939A'),
        h('span', { text: 'Only your Tailscale account can connect. Remove a lost phone in Tailscale.' })));
      nv.getJson('/api/pcs').then((data) => {
        const rows = [];
        data.pcs.forEach((pc, i) => {
          if (i > 0) rows.push(h('div', { class: 'divider' }));
          rows.push(row(pc));
        });
        card.replaceChildren(...rows);
      }).catch((error) => {
        card.replaceChildren(h('div', { class: 'pc' },
          h('span', { class: 'dot asleep' }),
          h('span', { class: 'pc-text' },
            h('span', { class: 'pc-name', text: 'This PC did not answer' }),
            h('span', { class: 'pc-line', text: error.message }))));
      });
    },
  });
})();
