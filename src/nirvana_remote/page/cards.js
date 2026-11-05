// The Usage, Servers and Running cards on screen 2 (plan 39 §2.6, the prototype's three cards above the spaces).
// Read-only: each is a GET of its own route (cards.rs), the same data the PC's square draws. spaces.js mounts the
// row and calls refresh() when the spaces list changes; the row also reads again every REREAD while it is shown,
// for usage and servers, which change without a herdr event. No innerHTML: every value is text.
(function () {
  'use strict';

  // The cards read again this often while screen 2 is shown: usage moves at the readers' pace (2 to 4 min on the
  // PC) and the servers block reads its ports every 3 s, so a minute's lag is the PC's own lag at most.
  const REREAD = 30000;
  // A burst of spaces events is one read.
  const SETTLE = 800;
  // The servers block's own cut (nirvana_footer.rs SERVERS_SHOWN).
  const SERVERS_SHOWN = 4;
  const BAR = { claude: 'claude', antigravity: 'agy', codex: 'codex', local: 'local' };

  function percent(used) { return Math.round(Math.max(0, Math.min(1, used)) * 100) + '%'; }

  // One pool's figure: its tightest window, the one that fills the foot's bar (nirvana_usage Figure::fill and
  // tightest_shown: the most used, the first listed on a tie), with the pace mark when any window is ahead. Both
  // windows do not fit a 164 px card beside the pool's name (measured headless at 360 px: the name went to "Cl…").
  function figure(row) {
    if (row.word) return row.word;
    if (row.memory) return Math.round(row.memory.used_mib / 1024) + '/' + Math.round(row.memory.total_mib / 1024) + ' GiB';
    const windows = (row.windows || []).filter((w) => typeof w.used === 'number');
    if (!windows.length) return 'not exposed';
    const tight = windows.reduce((best, w) => (w.used > best.used ? w : best));
    const ahead = (row.windows || []).some((w) => w.ahead);
    return percent(tight.used) + ' ' + (tight.window === 'week' ? '7d' : tight.window) + (ahead ? '▲' : '');
  }

  function mount(nv) {
    const h = nv.h;
    const usage = h('div', { class: 'card-box wide' }, h('span', { class: 'card-label', text: 'Usage' }));
    const servers = h('div', { class: 'card-box' }, h('span', { class: 'card-label', text: 'Servers' }));
    const running = h('div', { class: 'card-box' }, h('span', { class: 'card-label', text: 'Running' }));
    const el = h('div', { class: 'scroll cards', role: 'group', 'aria-label': 'Usage, servers and running' },
      usage, servers, running);
    let drawnUsage = false;

    function fill(card, ...children) { card.replaceChildren(card.firstChild, ...children); }

    function drawUsage(data) {
      const rows = (data.rows || []).map((row) => {
        const bar = h('div', { class: 'bar-fill ' + (BAR[row.provider] || 'local') + (drawnUsage ? '' : ' grow') });
        // A width set through the CSSOM, which the page's CSP (no unsafe-inline) allows; no style attribute.
        bar.style.width = (typeof row.fill === 'number' ? percent(row.fill) : '0%');
        return h('div', { class: 'meter' + (row.windows && row.windows.some((w) => w.stale) ? ' stale' : '') },
          h('div', { class: 'meter-line' }, h('span', { class: 'meter-name', text: row.label }),
            h('span', { class: 'meter-figure mono', text: figure(row) })),
          h('div', { class: 'bar' }, bar));
      });
      if (!rows.length) rows.push(h('span', { class: 'card-sub', text: 'Not read yet on the PC' }));
      fill(usage, ...rows);
      drawnUsage = true;
    }

    function drawServers(data) {
      const lines = [h('span', { text: data.remote.name + ' :' + data.remote.port })];
      const list = data.servers || [];
      for (const row of list.slice(0, SERVERS_SHOWN)) lines.push(h('span', { text: row.space + ' :' + row.port }));
      if (list.length > SERVERS_SHOWN) lines.push(h('span', { text: '+' + (list.length - SERVERS_SHOWN) + ' more' }));
      fill(servers, h('span', { class: 'card-big', text: data.up + ' up' }),
        h('span', { class: 'card-lines mono' }, ...lines));
    }

    function drawRunning(data) {
      const live = data.working + data.needs;
      const first = (data.agents || [])[0];
      fill(running, h('span', { class: 'card-big', text: live + (live === 1 ? ' agent' : ' agents') }),
        h('span', { class: 'card-needs', text: data.needs ? data.needs + ' needs you' : 'none waiting on you' }),
        first ? h('span', { class: 'card-sub', text: first.name + ' · ' + first.words }) : null);
    }

    function failed(card) { fill(card, h('span', { class: 'card-sub', text: 'Not read' })); }

    let timer = null;
    let over = false;
    function read() {
      if (over) return;
      nv.getJson('/api/cards/usage').then(drawUsage, () => failed(usage));
      nv.getJson('/api/cards/servers').then(drawServers, () => failed(servers));
      nv.getJson('/api/cards/running').then(drawRunning, () => failed(running));
    }
    function refresh() {
      clearTimeout(timer);
      timer = setTimeout(read, SETTLE);
    }
    const every = setInterval(() => { if (document.visibilityState === 'visible') read(); }, REREAD);
    read();
    return { el, refresh, close() { over = true; clearTimeout(timer); clearInterval(every); } };
  }

  window.NirvanaCards = { mount };
})();
