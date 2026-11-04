// Nirvana Remote's page shell (plan 39 P39a). It owns the column, the screen motion and the back button; each
// screen is its own file that registers itself:
//
//   Nirvana.screen('spaces', { render(el, params, nv) { ... } });
//
// and is opened with nv.go('spaces', 'r', { pc: ... }). 'r' slides in from the right (deeper), 'l' from the left
// (back), 'f' fades (first paint). A screen builds its DOM with nv.h and nv.icon, never innerHTML: names come
// from Tailscale and herdr, and are text.
(function () {
  'use strict';

  const screens = {};
  const enter = { r: 'scr-r', l: 'scr-l', f: 'scr-f' };

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
    if (name === 'lock') {
      const rect = document.createElementNS(ns, 'rect');
      rect.setAttribute('x', 5); rect.setAttribute('y', 11);
      rect.setAttribute('width', 14); rect.setAttribute('height', 9); rect.setAttribute('rx', 2);
      svg.append(rect);
    }
    for (const d of ICONS[name] || []) {
      const path = document.createElementNS(ns, 'path');
      path.setAttribute('d', d);
      svg.append(path);
    }
    return svg;
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

  function has(name) { return Object.prototype.hasOwnProperty.call(screens, name); }

  function show(name, dir, params) {
    const def = has(name) ? screens[name] : screens.pcs;
    const el = h('section', { class: 'screen ' + (enter[dir] || enter.f) });
    document.getElementById('nv').replaceChildren(el);
    def.render(el, params || {}, api);
  }

  // A screen change is a history entry, so the phone's back gesture goes back a screen.
  function go(name, dir, params) {
    history.pushState({ name, params: params || {} }, '', '#' + name);
    show(name, dir, params);
  }

  function screen(name, def) { screens[name] = def; }

  function boot() {
    const name = location.hash.slice(1);
    const start = has(name) ? name : 'pcs';
    history.replaceState({ name: start, params: {} }, '', '#' + start);
    show(start, 'f', {});
  }

  window.addEventListener('popstate', (event) => {
    const state = event.state || { name: 'pcs', params: {} };
    show(state.name, 'l', state.params);
  });
  document.addEventListener('DOMContentLoaded', boot);

  const api = { screen, go, has, h, icon, getJson };
  window.Nirvana = api;
})();
