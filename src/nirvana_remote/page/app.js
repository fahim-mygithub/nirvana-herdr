// Nirvana Remote's page shell (plan 39 P39a). It owns the column, the screen motion and the back button; each
// screen is its own file that registers itself:
//
//   Nirvana.screen('spaces', { render(el, params, nv) { ... return cleanup; } });
//
// and is opened with nv.go('spaces', 'r', { pc: ... }). 'r' slides in from the right (deeper), 'l' from the left
// (back), 'f' fades (first paint). A screen builds its DOM with nv.h and nv.icon, never innerHTML: names come
// from Tailscale and herdr, and are text. What render returns, when it is a function, runs as the screen leaves
// (P39b: a live view closes there).
(function () {
  'use strict';

  // P39o item 7: the home-screen app is laid out for the whole screen (app.css :root.standalone). The owner,
  // 2026-11-08: "it now doesn't need to accommodate for a web browser url bar." iOS answers navigator.standalone;
  // an installed Android Chrome app, display-mode. While the keyboard is up, the visual viewport is smaller than the
  // layout one (iOS does not resize the page for it, and may scroll it): its top and the part below it set the
  // column's top and bottom, so the composer stays just above the keyboard.
  const root = document.documentElement;
  function media(query) { return Boolean(window.matchMedia && window.matchMedia(query).matches); }
  const installed = navigator.standalone === true
    || media('(display-mode: standalone)') || media('(display-mode: fullscreen)');
  const KEYBOARD_PX = 80;
  function fit() {
    const view = window.visualViewport;
    const top = Math.max(0, Math.round(view.offsetTop));
    const below = Math.max(0, Math.round(window.innerHeight - view.height - view.offsetTop));
    root.style.setProperty('--vv-top', top + 'px');
    root.style.setProperty('--vv-bottom', below + 'px');
    root.classList.toggle('kb', below > KEYBOARD_PX);
  }
  if (installed) {
    root.classList.add('standalone');
    if (window.visualViewport) {
      window.visualViewport.addEventListener('resize', fit);
      window.visualViewport.addEventListener('scroll', fit);
      fit();
    }
  }

  const screens = {};
  const enter = { r: 'scr-r', l: 'scr-l', f: 'scr-f' };
  let leave = null;

  function h(tag, props) {
    const el = document.createElement(tag);
    const attrs = props || {};
    for (const key of Object.keys(attrs)) {
      const value = attrs[key];
      if (value === undefined || value === null || value === false) continue;
      if (key === 'class') el.className = value;
      else if (key === 'text') el.textContent = value;
      else if (key.startsWith('on')) el.addEventListener(key.slice(2).toLowerCase(), value);
      else el.setAttribute(key, value === true ? '' : String(value));
    }
    for (let i = 2; i < arguments.length; i++) {
      const child = arguments[i];
      if (child === undefined || child === null || child === false) continue;
      el.append(child instanceof Node ? child : document.createTextNode(String(child)));
    }
    return el;
  }

  // The prototype's icons, as path data; drawn with createElementNS so no markup is parsed.
  const ICONS = {
    lotus: ['M12 20c-4 0-7-2.5-8-6 2.5 2 5 3 8 3s5.5-1 8-3c-1 3.5-4 6-8 6z', 'M12 17c-2.5-2-3.5-5-3.5-7.5S10 4.5 12 3c2 1.5 3.5 4 3.5 6.5S14.5 15 12 17z'],
    chevron: ['M9 6l6 6-6 6'],
    back: ['M15 6l-6 6 6 6'],
    lock: ['M8 11V8a4 4 0 0 1 8 0v3'],
    attach: ['M21 11.5l-8.6 8.6a5 5 0 0 1-7.1-7.1l8.6-8.6a3.4 3.4 0 0 1 4.8 4.8l-8.6 8.6a1.7 1.7 0 0 1-2.4-2.4l7.9-7.9'],
    send: ['M12 19V5M5 12l7-7 7 7'],
    close: ['M6 6l12 12M18 6L6 18'],
    image: ['M21 17l-5-5-9 8'],
    file: ['M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z', 'M14 3v5h5'],
    clipboard: ['M16 5h2a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h2'],
  };

  function icon(name, size, stroke, width) {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('width', size);
    svg.setAttribute('height', size);
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', stroke);
    svg.setAttribute('stroke-width', width || 2);
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');
    svg.setAttribute('aria-hidden', 'true');
    const shape = (tag, attrs) => {
      const el = document.createElementNS(ns, tag);
      for (const key of Object.keys(attrs)) el.setAttribute(key, attrs[key]);
      svg.append(el);
    };
    if (name === 'lock') shape('rect', { x: 5, y: 11, width: 14, height: 9, rx: 2 });
    if (name === 'image') {
      shape('rect', { x: 3, y: 4, width: 18, height: 16, rx: 2 });
      shape('circle', { cx: 9, cy: 10, r: 2 });
    }
    if (name === 'clipboard') shape('rect', { x: 8, y: 3, width: 8, height: 4, rx: 1 });
    for (const d of ICONS[name] || []) shape('path', { d });
    return svg;
  }

  // P39o (record p39o): whether the PC answers, and the error screen when it does not. The owner, 2026-11-08: "add a
  // tailsscale check to the UI when it fails to connect. Like a error page." The screen lies over whatever screen
  // was open, so when the PC answers again the owner is where they were; a screen whose data failed to load is drawn
  // again then, and every live view opens again at once instead of waiting out its own back-off.
  //
  // What the page can tell apart: the phone offline (navigator.onLine false), no answer at all (the PC unreachable:
  // Tailscale off on the phone, the PC asleep or off), tailscaled answering a gateway error with no JSON (the serve
  // mount is there, Nirvana is not), and the PC refusing (403/421 from routes.rs's checks: another Tailscale
  // account). It cannot read whether the phone's VPN is on, so that is a check for the owner, worded as one.
  const LOST_MS = 10000;              // a live view down this long: the PC is checked
  const PROBE_MS = 6000;              // a check waits this long for /api/hello
  const RETRY_S = [2, 4, 8, 15, 30];  // the error screen's automatic tries, then every 30 s
  const RELOAD_GAP_MS = 60000;        // a stale shell reloads at most once a minute
  const BUILD_HEADER = 'Nirvana-Build';
  let current = null;
  const reach = { down: '', layer: null, when: null, grace: null, tick: null, tries: 0, dirty: false, checking: false, revive: new Set() };

  const COPY = {
    unreachable: { title: "Can't reach your PC", lead: 'Nothing came back from the PC. Check these, most likely first.', order: ['tailscale', 'awake', 'nirvana', 'online'] },
    offline: { title: 'This phone is offline', lead: 'The phone has no network, so nothing can reach the PC. Check these.', order: ['online', 'tailscale', 'awake', 'nirvana'] },
    nirvana: { title: 'Nirvana is not answering on the PC', lead: 'Tailscale reached the PC, but Nirvana did not answer behind it. Check these.', order: ['nirvana', 'awake', 'tailscale', 'online'] },
    refused: { title: 'The PC refused this phone', lead: "This phone's Tailscale account is not the PC's. Only the account the PC is signed in with can connect.", order: ['account', 'name', 'started'] },
  };
  const CHECKS = {
    tailscale: ['Tailscale is on on this phone', 'Open the Tailscale app: the VPN switch at the top is on, and it says Connected. This page cannot see the VPN; only you can.'],
    awake: ['The PC is on and awake', 'A sleeping PC drops off the tailnet. Wake it; in the Tailscale app it should show as connected, not "last seen".'],
    nirvana: ['Nirvana is open on the PC', "The Nirvana sidebar's bottom row reads ● remote :8443, its dot green."],
    online: ['This phone is online', ''],
    account: ["Sign in with the PC's account", 'In the Tailscale app, tap your account: it must be the one the PC is signed in to Tailscale with.'],
    name: ['Open the PC by its name', "Use the PC's own address, https://<pc>.<tailnet>.ts.net:8443/, not its IP or another name."],
    started: ['Nirvana just started on the PC?', 'For a few seconds after it starts, it does not know its own account yet. Try again.'],
  };

  function online() {
    return navigator.onLine === false
      ? 'This phone says it is offline: turn off Airplane mode, or join Wi-Fi or mobile data.'
      : 'This phone says it is online.';
  }

  // What an answer says about reaching the PC: 'refused', 'nirvana', or '' (the PC's Nirvana answered).
  function verdict(status, kind) {
    if (status === 403 || status === 421) return 'refused';
    if (status >= 502 && status <= 504 && !(kind || '').includes('json')) return 'nirvana';
    return '';
  }

  function unreachable() { return navigator.onLine === false ? 'offline' : 'unreachable'; }

  // A request that failed. With no answer at all, the screen waits as a dropped live view does (LOST_MS, then a
  // check): a phone moving from Wi-Fi to mobile data loses a request or two and should not flash the screen. The
  // check as the page opens covers a page that came out of the cache. An answer that is not the page's is the screen
  // at once. Only a failed load (a GET) has the screen drawn again when the PC is back: a failed send keeps the
  // chat as it is, the typed text with it.
  function trouble(why, load) {
    if (load) reach.dirty = true;
    if (why === 'network') lost();
    else fallen(why);
  }

  function answered() {
    clearTimeout(reach.grace);
    reach.grace = null;
    if (reach.down) up();
  }

  // A live view went down: it reconnects by itself, and the PC is checked only if it is still down after LOST_MS.
  function lost() {
    if (reach.down || reach.grace) return;
    reach.grace = setTimeout(() => { reach.grace = null; check(); }, LOST_MS);
  }

  // A shell from an older build's cache (the PC was slow, then answered with another tag) loads again from the PC.
  function stale(build) {
    const meta = document.querySelector('meta[name="nirvana-build"]');
    const mine = meta ? meta.getAttribute('content') : '';
    if (!build || !mine || build === mine) return false;
    try {
      const last = Number(sessionStorage.getItem('nirvana-reloaded')) || 0;
      if (Date.now() - last < RELOAD_GAP_MS) return false;
      sessionStorage.setItem('nirvana-reloaded', String(Date.now()));
    } catch (_) {
      return false;
    }
    location.reload();
    return true;
  }

  async function check() {
    if (reach.checking) return;
    reach.checking = true;
    if (reach.when) reach.when.textContent = 'Checking…';
    const stop = new AbortController();
    const timer = setTimeout(() => stop.abort(), PROBE_MS);
    let why = '';
    let build = '';
    try {
      const response = await fetch('/api/hello', { credentials: 'same-origin', cache: 'no-store', signal: stop.signal });
      why = verdict(response.status, response.headers.get('Content-Type'));
      build = response.headers.get(BUILD_HEADER) || '';
    } catch (_) {
      why = unreachable();
    }
    clearTimeout(timer);
    reach.checking = false;
    if (why) fallen(why);
    else if (!stale(build)) up();
  }

  function fallen(why) {
    clearTimeout(reach.grace);
    reach.grace = null;
    if (!reach.down) reach.tries = 0;
    reach.down = why;
    drawDown();
    schedule();
  }

  function schedule() {
    clearInterval(reach.tick);
    let left = RETRY_S[Math.min(reach.tries, RETRY_S.length - 1)];
    reach.tries += 1;
    reach.when.textContent = 'Trying again in ' + left + ' s';
    reach.tick = setInterval(() => {
      left -= 1;
      if (left > 0) { reach.when.textContent = 'Trying again in ' + left + ' s'; return; }
      clearInterval(reach.tick);
      check();
    }, 1000);
  }

  function up() {
    if (!reach.down) return;
    reach.down = '';
    clearInterval(reach.tick);
    if (reach.layer) reach.layer.remove();
    reach.layer = null;
    reach.when = null;
    for (const revive of reach.revive) revive();
    if (reach.dirty && current) show(current.name, 'f', current.params);
    reach.dirty = false;
  }

  function tryNow() {
    clearInterval(reach.tick);
    check();
  }

  function drawDown() {
    const copy = COPY[reach.down] || COPY.unreachable;
    if (!reach.layer) {
      reach.layer = h('section', { class: 'reach scr-f', role: 'alert' });
      document.getElementById('nv').append(reach.layer);
    }
    const items = copy.order.map((key, i) => {
      const [title, how] = CHECKS[key];
      const net = key === 'online';
      return h('li', { class: 'reach-item' },
        h('span', { class: 'reach-n' + (net && navigator.onLine === false ? ' bad' : ''), text: String(i + 1) }),
        h('span', { class: 'reach-text' },
          h('span', { class: 'reach-check', text: title }),
          h('span', { class: 'reach-how', text: net ? online() : how })));
    });
    reach.when = h('span', { class: 'note', role: 'status' });
    reach.layer.replaceChildren(
      h('div', { class: 'brand' },
        h('div', { class: 'brand-mark reach-mark' }, icon('lotus', 26, '#8D939A', 1.7)),
        h('div', { class: 'brand-text' },
          h('span', { class: 'brand-name', text: 'Nirvana' }),
          h('span', { class: 'brand-sub', text: 'Not connected to the PC' }))),
      h('div', { class: 'section' },
        h('h1', { class: 'reach-title', text: copy.title }),
        h('p', { class: 'reach-lead', text: copy.lead })),
      h('ol', { class: 'card reach-list' }, ...items),
      h('div', { class: 'reach-foot' },
        h('button', { class: 'press reach-try', type: 'button', text: 'Try again', onclick: tryNow }),
        reach.when));
  }

  window.addEventListener('offline', () => { if (reach.down) drawDown(); lost(); });
  window.addEventListener('online', () => { if (reach.down) { drawDown(); tryNow(); } });
  document.addEventListener('visibilitychange', () => { if (reach.down && document.visibilityState === 'visible') tryNow(); });

  async function answer(response) {
    let data = null;
    try { data = await response.json(); } catch (_) { /* not JSON */ }
    return { ok: response.ok, status: response.status, data: data || {} };
  }

  // A fetch that tells the reach check what came back (P39o).
  async function reached(path, options) {
    let response;
    try {
      response = await fetch(path, options);
    } catch (_) {
      trouble('network', !options.method);
      throw new Error('no answer from the PC');
    }
    const why = verdict(response.status, response.headers.get('Content-Type'));
    if (why) trouble(why, !options.method);
    else answered();
    return response;
  }

  async function getJson(path) {
    const response = await reached(path, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) {
      let why = response.status + '';
      try { why = (await response.json()).error || why; } catch (_) { /* not JSON */ }
      throw new Error(why);
    }
    return response.json();
  }

  // A POST from this page: the browser adds Origin and Sec-Fetch-Site, which the server requires (record p39a, S5).
  async function postJson(path, body) {
    const response = await reached(path, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    });
    return answer(response);
  }

  // The server's body deadline (http.rs READ_TIMEOUT, BODY_RATE and BODY_MOST): 10 s, plus the size at 16 KiB a
  // second, never past 20 minutes. An upload waits that long and a margin, so the server's own answer and its reason
  // come first; past it the PC is not answering at all (record p39u: on 2026-11-06 a fetch with no end left a chip
  // "uploading" and Send refused).
  const READ_TIMEOUT_MS = 10000;
  const BODY_RATE = 16384;
  const BODY_MOST_MS = 1200000;
  const UPLOAD_MARGIN_MS = 30000;
  function uploadWait(bytes) { return Math.min(READ_TIMEOUT_MS + Math.ceil(bytes / BODY_RATE) * 1000, BODY_MOST_MS) + UPLOAD_MARGIN_MS; }

  // A file as the whole body; the server names it from its type (record p39b, uploads). XMLHttpRequest, not fetch
  // (record p39u): its upload progress works on iPhone Safari, where fetch has none, and its timeout ends an upload
  // that hangs. A same-origin POST carries Origin and Sec-Fetch-Site from XHR as from fetch. `on.progress(sent,
  // total)` as it goes; `done` resolves as answer() does, with `why` when there was no answer, and never rejects.
  function upload(path, file, on) {
    const xhr = new XMLHttpRequest();
    const done = new Promise((resolve) => {
      const failed = (why) => resolve({ ok: false, status: 0, data: {}, why });
      xhr.open('POST', path);
      xhr.setRequestHeader('Content-Type', file.type || 'application/octet-stream');
      xhr.timeout = uploadWait(file.size);
      xhr.upload.onprogress = (event) => { if (on && on.progress && event.lengthComputable) on.progress(event.loaded, event.total); };
      xhr.onload = () => {
        const why = verdict(xhr.status, xhr.getResponseHeader('Content-Type'));
        if (why) trouble(why);
        else answered();
        let data = null;
        try { data = JSON.parse(xhr.responseText); } catch (_) { /* not JSON */ }
        resolve({ ok: xhr.status >= 200 && xhr.status < 300, status: xhr.status, data: data || {} });
      };
      xhr.onerror = () => failed('the connection to the PC failed');
      // P39o: no answer at all is a question for the reach check too.
      xhr.addEventListener('error', () => trouble('network'));
      xhr.ontimeout = () => failed('no answer from the PC in ' + Math.round(xhr.timeout / 1000) + ' s');
      xhr.onabort = () => failed('removed');
      xhr.send(file);
    });
    return { done, abort: () => xhr.abort() };
  }

  // A live view (record p39b, "Why SSE"). EventSource reconnects by itself after a dropped connection; when the PC
  // answers anything but 200 (herdr restarting) it closes for good, so it is opened again here with a growing wait.
  // When iPhone Safari brings the page back from the background, the view is opened again at once: the first event
  // of every stream is the whole state. on.state(true|false) says whether it is live. Safari fires both pageshow and
  // visibilitychange on a wake (and pageshow on the first load), so a view opened in the last second is kept, not
  // opened again: each extra open held a slot on the PC until its next failed write (review round, N2). The
  // handlers get the view's close as their second argument: a view that is over for good (the agent closed) ends
  // itself there, instead of reconnecting to a 404 for as long as the screen is open (S5).
  function stream(path, on) {
    let source = null;
    let wait = 1000;
    let timer = null;
    let over = false;
    let opened = 0;
    function open() {
      if (over) return;
      clearTimeout(timer);
      if (source) source.close();
      opened = Date.now();
      source = new EventSource(path);
      source.onopen = () => { wait = 1000; answered(); if (on.state) on.state(true); };
      source.onerror = () => {
        if (over) return;
        lost();
        if (on.state) on.state(false);
        if (source.readyState === EventSource.CLOSED) {
          timer = setTimeout(open, wait);
          wait = Math.min(wait * 2, 15000);
        }
      };
      for (const name of Object.keys(on)) {
        if (name === 'state') continue;
        source.addEventListener(name, (event) => {
          let data = {};
          try { data = JSON.parse(event.data); } catch (_) { return; }
          if (!over) on[name](data, close);
        });
      }
    }
    function shown() {
      if (document.visibilityState === 'visible' && Date.now() - opened >= 1000) open();
    }
    // P39o: the PC answered again (the reach check): open now, not at the end of the back-off.
    function revive() {
      if (!over && (!source || source.readyState !== EventSource.OPEN)) open();
    }
    function close() {
      over = true;
      reach.revive.delete(revive);
      clearTimeout(timer);
      if (source) source.close();
      document.removeEventListener('visibilitychange', shown);
      window.removeEventListener('pageshow', shown);
    }
    document.addEventListener('visibilitychange', shown);
    window.addEventListener('pageshow', shown);
    reach.revive.add(revive);
    open();
    return close;
  }

  // A bottom sheet over the screen (prototype's sheet and backdrop motion); the content's buttons call close().
  function sheet(build) {
    const root = document.getElementById('nv');
    let closing = false;
    const backdrop = h('button', { class: 'backdrop', type: 'button', 'aria-label': 'Close' });
    const panel = h('div', { class: 'sheet', role: 'dialog' }, h('div', { class: 'grip' }));
    const layer = h('div', { class: 'layer' }, backdrop, panel);
    function close(then) {
      if (closing) return;
      closing = true;
      backdrop.className = 'backdrop backdrop-out';
      panel.className = 'sheet sheet-out';
      setTimeout(() => { layer.remove(); if (then) then(); }, 240);
    }
    backdrop.addEventListener('click', () => close());
    panel.append(build(close));
    root.append(layer);
    return close;
  }

  function has(name) { return Object.prototype.hasOwnProperty.call(screens, name); }

  function show(name, dir, params) {
    if (leave) { try { leave(); } catch (_) { /* the old screen is gone anyway */ } }
    leave = null;
    const def = has(name) ? screens[name] : screens.pcs;
    const el = h('section', { class: 'screen ' + (enter[dir] || enter.f) });
    document.getElementById('nv').replaceChildren(el);
    // P39o: the screen the error screen returns to; the error screen itself stays on top.
    current = { name: has(name) ? name : 'pcs', params: params || {} };
    reach.dirty = false;
    if (reach.layer) document.getElementById('nv').append(reach.layer);
    const cleanup = def.render(el, params || {}, api);
    if (typeof cleanup === 'function') leave = cleanup;
  }

  // A screen change is a history entry, so the phone's back gesture goes back a screen.
  function go(name, dir, params) {
    history.pushState({ name, params: params || {} }, '', '#' + name);
    show(name, dir, params);
  }

  function screen(name, def) { screens[name] = def; }

  // The chat a tapped alert names (P39e, sw.js): `#chat?agent=<pane id>&name=...&space=...`. Back goes to the spaces.
  function linked(hash) {
    const [name, query] = hash.split('?');
    if (name !== 'chat' || !query || !has('chat')) return null;
    const asked = new URLSearchParams(query);
    const agent = asked.get('agent') || '';
    return agent ? { agent, name: asked.get('name') || '', space: asked.get('space') || '' } : null;
  }

  function boot() {
    const name = location.hash.slice(1);
    const chat = linked(name);
    if (chat) {
      history.replaceState({ name: 'spaces', params: {} }, '', '#spaces');
      history.pushState({ name: 'chat', params: chat }, '', '#chat');
      show('chat', 'f', chat);
      return;
    }
    // A screen that needs what led to it (an agent) starts over from the PC list.
    const start = has(name) && name !== 'chat' ? name : 'pcs';
    history.replaceState({ name: start, params: {} }, '', '#' + start);
    show(start, 'f', {});
  }

  window.addEventListener('popstate', (event) => {
    const state = event.state || { name: 'pcs', params: {} };
    show(state.name, 'l', state.params);
  });
  document.addEventListener('DOMContentLoaded', boot);
  // P39o: one check as the page opens. It shows the error screen at once when the page came out of the phone's cache
  // with the PC out of reach, and reloads a shell that is not the running build's.
  document.addEventListener('DOMContentLoaded', check);

  // Whether the phone asked for less motion: the sparkle then stands still.
  function still() { return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches; }

  const api = { screen, go, has, h, icon, getJson, postJson, upload, stream, sheet, still };
  window.Nirvana = api;
})();
