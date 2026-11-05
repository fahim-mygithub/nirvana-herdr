// Screen 6, "Open a new space" (plan 39 §2.5, the prototype's sheet from screen 2's + button). The folders of
// Desktop\Pojects on the PC (pick.rs); a tap opens a space on it as "Open Nirvana here" does, or, when one is open on
// it already, names that space. The page sends the folder's name as listed, never a path. No innerHTML.
(function () {
  'use strict';

  // The prototype's folder icon, drawn here so the shell's icon table stays as it is.
  function folderIcon() {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    for (const [key, value] of Object.entries({ width: 20, height: 20, viewBox: '0 0 24 24', fill: 'none',
      stroke: '#B0B5BA', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true' })) {
      svg.setAttribute(key, value);
    }
    const path = document.createElementNS(ns, 'path');
    path.setAttribute('d', 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z');
    svg.append(path);
    return svg;
  }

  // The + of screen 2's top bar.
  function plusIcon() {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    for (const [key, value] of Object.entries({ width: 20, height: 20, viewBox: '0 0 24 24', fill: 'none',
      stroke: '#5FB3BC', 'stroke-width': 2.2, 'stroke-linecap': 'round', 'aria-hidden': 'true' })) {
      svg.setAttribute(key, value);
    }
    const path = document.createElementNS(ns, 'path');
    path.setAttribute('d', 'M12 5v14M5 12h14');
    svg.append(path);
    return svg;
  }

  // The sheet. opened(space, existing, agent) runs once it has closed, with the space herdr named; agent is false
  // for a space open on the folder with only plain shells, where nothing was started (review S2).
  function open(nv, opened) {
    const h = nv.h;
    const list = h('div', { class: 'folders scroll' }, h('div', { class: 'skeleton round' }));
    const root = h('span', { class: 'sheet-sub mono', text: 'Desktop\\Pojects' });
    const note = h('span', { class: 'note', role: 'status' });
    let busy = false;
    nv.sheet((close) => {
      async function pick(folder, button) {
        if (busy) return;
        busy = true;
        button.setAttribute('aria-busy', 'true');
        note.textContent = 'Opening ' + folder.name + '…';
        const r = await nv.postJson('/api/folders/open', { name: folder.name })
          .catch(() => ({ ok: false, status: 0, data: {} }));
        busy = false;
        button.removeAttribute('aria-busy');
        if (!r.ok) { note.textContent = r.data.error || 'The PC did not open it.'; return; }
        close(() => opened(r.data.space || null, !!r.data.existing, r.data.agent !== false));
      }
      nv.getJson('/api/folders').then((data) => {
        root.textContent = data.root || 'Desktop\\Pojects';
        const rows = (data.folders || []).map((folder) => {
          const button = h('button', { class: 'press row folder', type: 'button' }, folderIcon(),
            h('span', { class: 'folder-name', text: folder.name }),
            h('span', { class: 'folder-note', text: !folder.open ? '' : folder.agent ? 'open' : 'open, no agent running' }));
          button.addEventListener('click', () => pick(folder, button));
          return button;
        });
        if (!rows.length) rows.push(h('span', { class: 'note', text: 'No folders in Desktop\\Pojects.' }));
        list.replaceChildren(...rows);
      }, (err) => { list.replaceChildren(); note.textContent = err.message || 'The folders could not be read.'; });
      return h('div', { class: 'sheet-body' },
        h('span', { class: 'sheet-title', text: 'Open a new space' }), root, list, note);
    });
  }

  window.NirvanaNewSpace = { open, plusIcon };
})();
