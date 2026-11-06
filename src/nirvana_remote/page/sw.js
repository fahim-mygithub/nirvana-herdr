// Nirvana Remote's service worker (plan 39 P39e, record p39e). It is here for one thing: the alerts. A push from the
// PC shows as a notification, and a tap on it opens that agent's chat.
//
// It caches nothing, and has no fetch handler at all, so every request goes to the PC as if there were no worker:
// each answer is live agent data or the page of the build that is running now (a stale page after an update would
// hide the new version), and with the PC out of reach there is nothing a cache could do (the page needs the tailnet).
'use strict';

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));

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
