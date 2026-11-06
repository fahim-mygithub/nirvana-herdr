// The alerts row on screen 2 (plan 39 §2.7, P39e; prototype screen 7 is the notification it turns on). A switch
// asks for notification permission, subscribes this phone with the PC's push key, and posts the subscription to the
// PC; off, it tells the PC and drops the subscription. It also starts the service worker (sw.js) and opens an
// agent's chat when a tapped notification asks the open page to.
//
// iPhone: Web Push works only in a home-screen web app, iOS 16.4 or later; in a Safari tab there is no PushManager.
// The row then says so in plain words. Permission is asked only from a tap, as iOS requires: requestPermission is the
// first call in the click handler, before anything is awaited.
(function () {
  'use strict';
  const nv = window.Nirvana;
  const h = nv.h;

  const workers = 'serviceWorker' in navigator;
  // The registration, or null when the worker cannot start (no HTTPS, a private window).
  const registered = workers
    ? navigator.serviceWorker.register('/sw.js', { scope: '/' }).catch(() => null)
    : Promise.resolve(null);
  if (workers) {
    // A tapped notification while the app is open: the worker focuses this page and names the agent.
    navigator.serviceWorker.addEventListener('message', (event) => {
      const data = event.data || {};
      if (data.open === 'chat' && data.params && data.params.agent) nv.go('chat', 'r', data.params);
    });
  }

  function iphone() {
    return /iPhone|iPad|iPod/.test(navigator.userAgent)
      || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1);
  }

  function homeScreen() {
    return navigator.standalone === true
      || Boolean(window.matchMedia && window.matchMedia('(display-mode: standalone)').matches);
  }

  function pushable() { return workers && 'PushManager' in window && 'Notification' in window; }

  const IN_SAFARI = 'On iPhone, alerts work only from the home-screen app (iOS 16.4 or later). In Safari, tap Share, '
    + 'then Add to Home Screen, and open Nirvana from there.';
  const DENIED = 'Notifications are off for Nirvana. Turn them on in Settings, Notifications, Nirvana.';
  // The phone has a subscription the PC no longer keeps: the PC made a new key (its phones are dropped then), or
  // the push service refused this one and the PC let it go (alerts.rs, REJECTED_MOST).
  const KEY_CHANGED = "The PC's alert key changed: turn alerts on again.";

  // Why the switch cannot work here, or '' when it can.
  function why() {
    if (iphone() && !homeScreen()) return IN_SAFARI;
    if (!pushable()) return 'This browser cannot get alerts.';
    if (Notification.permission === 'denied') return DENIED;
    return '';
  }

  // The PC's key, base64url, as the bytes applicationServerKey takes.
  function bytes(text) {
    const padded = text.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - (text.length % 4)) % 4);
    return Uint8Array.from(atob(padded), (c) => c.charCodeAt(0));
  }

  function sameKey(subscription, key) {
    const had = subscription.options && subscription.options.applicationServerKey;
    if (!had) return false;
    const a = new Uint8Array(had);
    const b = bytes(key);
    return a.length === b.length && a.every((value, i) => value === b[i]);
  }

  async function worker() {
    if (!(await registered)) throw new Error('the service worker did not start');
    return navigator.serviceWorker.ready;
  }

  function mount() {
    let server = null;
    let on = false;
    let busy = false;
    // The phone's subscription is one the PC does not keep: turning on makes a new one.
    let stale = false;
    const knob = h('span', { class: 'switch-knob' });
    const toggle = h('button', { class: 'press switch', type: 'button', role: 'switch', 'aria-checked': 'false',
      'aria-label': 'Alerts when an agent is done or needs you' }, knob);
    const note = h('span', { class: 'note', role: 'status' });
    const el = h('div', { class: 'section pad' }, h('span', { class: 'label', text: 'Alerts' }),
      h('div', { class: 'alerts-row' },
        h('span', { class: 'alerts-text' },
          h('span', { class: 'alerts-title', text: 'When an agent is done or needs you' }),
          h('span', { class: 'alerts-sub', text: 'A notification on this phone. A tap opens the agent.' })),
        toggle),
      note);

    function draw() {
      toggle.setAttribute('aria-checked', on ? 'true' : 'false');
      toggle.classList.toggle('on', on);
      toggle.disabled = busy;
    }

    async function key() {
      if (!server) server = await nv.getJson('/api/alerts');
      return server.key;
    }

    async function turnOn(asked) {
      busy = true;
      draw();
      note.textContent = 'Turning alerts on…';
      try {
        const permission = await asked;
        if (permission !== 'granted') {
          note.textContent = permission === 'denied' ? DENIED : 'Alerts stay off: notifications were not allowed.';
          return;
        }
        const pc = await key();
        const ready = await worker();
        let subscription = await ready.pushManager.getSubscription();
        if (subscription && (stale || !sameKey(subscription, pc))) {
          await subscription.unsubscribe();
          subscription = null;
        }
        if (!subscription) {
          subscription = await ready.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: bytes(pc) });
        }
        const answer = await nv.postJson('/api/alerts/subscribe', subscription.toJSON());
        if (!answer.ok) {
          note.textContent = answer.data.error || 'The PC did not take it (' + answer.status + ').';
          return;
        }
        on = true;
        stale = false;
        note.textContent = answer.data.new ? 'Alerts are on. A first one is on its way.' : 'Alerts are on.';
      } catch (error) {
        note.textContent = 'Alerts could not be turned on: ' + ((error && error.message) || error);
      } finally {
        busy = false;
        draw();
      }
    }

    async function turnOff() {
      busy = true;
      draw();
      try {
        const ready = await worker();
        const subscription = await ready.pushManager.getSubscription();
        if (subscription) {
          const answer = await nv.postJson('/api/alerts/unsubscribe', { endpoint: subscription.endpoint });
          if (!answer.ok) {
            note.textContent = answer.data.error || 'The PC did not take it (' + answer.status + ').';
            return;
          }
          await subscription.unsubscribe();
        }
        on = false;
        note.textContent = 'Alerts are off on this phone.';
      } catch (error) {
        note.textContent = 'Alerts could not be turned off: ' + ((error && error.message) || error);
      } finally {
        busy = false;
        draw();
      }
    }

    toggle.addEventListener('click', () => {
      if (busy) return;
      const cannot = why();
      if (cannot) { note.textContent = cannot; return; }
      if (on) { turnOff(); return; }
      // From the tap itself, before any await: iOS shows its prompt only for a user gesture.
      const asked = Notification.requestPermission();
      turnOn(asked);
    });

    // Where the phone stands. Subscribed, and the PC keeps that subscription with the same keys: on, and nothing is
    // posted. The PC keeps the endpoint with other keys (the browser renewed them): posted again. The PC does not
    // keep it, or the phone's was made with another key: off, and the row says to turn alerts on again.
    (async () => {
      const cannot = why();
      if (cannot) { note.textContent = cannot; return; }
      try {
        const pc = await key();
        const ready = await worker();
        const subscription = await ready.pushManager.getSubscription();
        if (subscription && Notification.permission === 'granted') {
          const check = sameKey(subscription, pc)
            ? await nv.postJson('/api/alerts/check', subscription.toJSON())
            : { ok: true, data: { known: false } };
          if (check.ok && check.data.known && check.data.same) {
            on = true;
          } else if (check.ok && check.data.known) {
            const answer = await nv.postJson('/api/alerts/subscribe', subscription.toJSON());
            on = answer.ok;
          } else if (check.ok) {
            stale = true;
            note.textContent = KEY_CHANGED;
          }
        }
      } catch (error) {
        note.textContent = 'Alerts: ' + ((error && error.message) || error);
      }
      draw();
    })();

    draw();
    return { el };
  }

  window.NirvanaAlerts = { mount };
})();
