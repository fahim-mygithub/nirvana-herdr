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

  async function answer(response) {
    let data = null;
    try { data = await response.json(); } catch (_) { /* not JSON */ }
    return { ok: response.ok, status: response.status, data: data || {} };
  }

  async function getJson(path) {
    const response = await fetch(path, { credentials: 'same-origin', cache: 'no-store' });
    if (!response.ok) {
      let why = response.status + '';
      try { why = (await response.json()).error || why; } catch (_) { /* not JSON */ }
      throw new Error(why);
    }
    return response.json();
  }

  // A POST from this page: the browser adds Origin and Sec-Fetch-Site, which the server requires (record p39a, S5).
  async function postJson(path, body) {
    const response = await fetch(path, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
    });
    return answer(response);
  }

  // A file as the whole body; the server names it from its type (record p39b, uploads).
  async function upload(path, file) {
    const response = await fetch(path, {
      method: 'POST', credentials: 'same-origin', cache: 'no-store',
      headers: { 'Content-Type': file.type || 'application/octet-stream' }, body: file,
    });
    return answer(response);
  }

  // A live view (record p39b, "Why SSE"). EventSource reconnects by itself after a dropped connection; when the PC
  // answers anything but 200 (herdr restarting) it closes for good, so it is opened again here with a growing wait.
  // When iPhone Safari brings the page back from the background, the view is opened again at once: the first event
  // of every stream is the whole state. on.state(true|false) says whether it is live.
  function stream(path, on) {
    let source = null;
    let wait = 1000;
    let timer = null;
    let over = false;
    function open() {
      if (over) return;
      clearTimeout(timer);
      if (source) source.close();
      source = new EventSource(path);
      source.onopen = () => { wait = 1000; if (on.state) on.state(true); };
      source.onerror = () => {
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
          on[name](data);
        });
      }
    }
    function shown() { if (document.visibilityState === 'visible') open(); }
    document.addEventListener('visibilitychange', shown);
    window.addEventListener('pageshow', shown);
    open();
    return function close() {
      over = true;
      clearTimeout(timer);
      if (source) source.close();
      document.removeEventListener('visibilitychange', shown);
      window.removeEventListener('pageshow', shown);
    };
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
    const cleanup = def.render(el, params || {}, api);
    if (typeof cleanup === 'function') leave = cleanup;
  }

  // A screen change is a history entry, so the phone's back gesture goes back a screen.
  function go(name, dir, params) {
    history.pushState({ name, params: params || {} }, '', '#' + name);
    show(name, dir, params);
  }

  function screen(name, def) { screens[name] = def; }

  function boot() {
    const name = location.hash.slice(1);
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

  // Whether the phone asked for less motion: the sparkle then stands still.
  function still() { return window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches; }

  const api = { screen, go, has, h, icon, getJson, postJson, upload, stream, sheet, still };
  window.Nirvana = api;
})();
