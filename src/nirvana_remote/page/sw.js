// Nirvana Remote's service worker (plan 39 P39e, record p39e). A push from the PC shows as a notification, and a tap
// on it opens that agent's chat.
//
// P39o (record p39o) gave it one more job: when the PC cannot be reached at all (Tailscale off on the phone, the PC
// asleep, Nirvana closed), no byte comes from the PC, so the page and its error screen come from here. It keeps the
// shell, the static files only (SHELL, written in by the PC: the page, its scripts and styles, the manifest, the
// icons), in one cache named for this build. It never keeps or answers anything under /api/: agent data is always
// live, from the PC, or nothing.
//
// The PC first, always: a shell file is fetched from the PC and the cache is only the answer when the PC does not
// give one (no connection, no answer in WAIT_MS, or a refusal or gateway error that the shell explains better than a
// bare text page). So while the PC answers, the page is the running build's, as P39e required. A new build writes a
// new BUILD here; these bytes change, the browser installs the new worker on the next visit, and its activate drops
// every older nirvana-shell- cache.
'use strict';

const BUILD = 'NIRVANA_BUILD';
const SHELL = ['NIRVANA_SHELL'];
const PREFIX = 'nirvana-shell-';
const CACHE = PREFIX + BUILD;
// How long a shell file waits for the PC when a kept copy exists. A sleeping PC on the tailnet does not refuse, it
// just never answers (TCP waits about a minute); Tailscale off fails at once (the ts.net name does not resolve).
const WAIT_MS = 4000;
// After the PC failed, the next shell files go straight to the cache for this long: one wait per page load, not one
// per script.
const DOWN_MS = 15000;
// The answers that come from the PC's side but are not the page: the login checks' refusals (403, 421) and
// tailscaled's own gateway errors when Nirvana is not behind the serve mount (502, 503, 504).
const NOT_THE_PAGE = [403, 421, 502, 503, 504];
let downUntil = 0;
// The error screen's Try again and its automatic tries ask /api/hello, which this worker never answers, so the
// DOWN_MS window does not hold them back: they reach the PC (or fail) at once (review round, N6).

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

// The shell, fetched once as the worker installs. A file that fails is skipped, not fatal: a failed install would
// leave the phone with no worker, and the alerts with it. The PC-first fetches below fill any gap.
self.addEventListener('install', (event) => event.waitUntil((async () => {
  const cache = await caches.open(CACHE);
  await Promise.all(SHELL.map(async (path) => {
    try {
      const answer = await fetch(path, { cache: 'no-store', credentials: 'same-origin' });
      if (answer.ok) await cache.put(path, answer);
    } catch (_) { /* filled later by a load that reaches the PC */ }
  }));
})()));

// Only this build's cache stays: an older build's shell cannot outlive the worker that replaced it.
self.addEventListener('activate', (event) => event.waitUntil((async () => {
  for (const name of await caches.keys()) {
    if (name.startsWith(PREFIX) && name !== CACHE) await caches.delete(name);
  }
})()));

// A shell file, the PC first. Anything else (every /api/ call, the live views, the fonts, another origin) is not
// answered here, so the browser sends it to the network exactly as it would with no worker.
self.addEventListener('fetch', (event) => {
  const request = event.request;
  if (request.method !== 'GET') return;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;
  if (url.pathname.startsWith('/api/')) return;
  if (!SHELL.includes(url.pathname)) return;
  event.respondWith(shellFile(event, url.pathname));
});

async function shellFile(event, path) {
  const cache = await caches.open(CACHE);
  const kept = await cache.match(path);
  if (kept && Date.now() < downUntil) return kept;
  // The request as the page made it: a navigation cannot be rebuilt with options, and the PC sends no-store anyway.
  const asked = fetch(event.request).then((answer) => {
    if (answer.ok) {
      const saved = cache.put(path, answer.clone()).catch(() => {});
      // The worker is kept alive for the write; a waitUntil too late in the event throws, and the write runs anyway.
      try { event.waitUntil(saved); } catch (_) { /* still written */ }
    } else if (answer.status >= 502) {
      downUntil = Date.now() + DOWN_MS;
    }
    return answer;
  }, (error) => {
    downUntil = Date.now() + DOWN_MS;
    throw error;
  });
  if (!kept) return asked;
  // A late answer still refreshes the cache (the waitUntil above); the page is not held for it.
  event.waitUntil(asked.catch(() => {}));
  try {
    const answer = await within(asked, WAIT_MS);
    return NOT_THE_PAGE.includes(answer.status) ? kept : answer;
  } catch (_) {
    downUntil = Date.now() + DOWN_MS;
    return kept;
  }
}

function within(promise, ms) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('no answer in ' + ms + ' ms')), ms);
    promise.then((value) => { clearTimeout(timer); resolve(value); }, (error) => { clearTimeout(timer); reject(error); });
  });
}

// The page found it is an older build than the PC's (app.js stale) and is about to reload: the down window goes
// first, or the reload would be served this cache's old shell again (review round, S2). The answer on the port tells
// the page it may reload now.
self.addEventListener('message', (event) => {
  const data = event.data || {};
  if (data.forget !== 'down') return;
  downUntil = 0;
  if (event.ports && event.ports[0]) event.ports[0].postMessage({ forgot: true });
});

// The body is what alerts.rs's payload writes: title (the space), body (the agent and its state), and the agent's
// pane id, name and space for the tap. iOS ends a subscription that gets pushes without notifications, so every
// push shows one, even one that cannot be read.
self.addEventListener('push', (event) => {
  let data = {};
  try { data = event.data ? event.data.json() : {}; } catch (_) { data = {}; }
  const agent = typeof data.agent === 'string' ? data.agent : '';
  const state = typeof data.state === 'string' ? data.state : '';
  event.waitUntil(self.registration.showNotification(data.title || 'Nirvana', {
    body: data.body || '',
    icon: '/icon-192.png',
    // One notification per agent and state: "needs you" then "is done" are two. With one tag per agent the second
    // would replace the first, and iOS (no renotify) shows a replacement with no new banner or sound.
    tag: agent ? 'agent-' + agent + '-' + state : 'nirvana',
    data: { agent, name: data.name || '', space: data.space || '' },
  }));
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  const data = event.notification.data || {};
  const open = data.agent ? { agent: data.agent, name: data.name || '', space: data.space || '' } : null;
  const url = open ? '/#chat?' + new URLSearchParams(open).toString() : '/';
  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    for (const client of windows) {
      if (!('focus' in client)) continue;
      try {
        await client.focus();
        if (open) client.postMessage({ open: 'chat', params: open });
        return;
      } catch (_) {
        // A window that would not take focus: the tap still opens the chat, in a new one.
      }
    }
    await self.clients.openWindow(url);
  })());
});
