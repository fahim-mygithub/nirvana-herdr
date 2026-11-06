# Plan 39 P39e: Nirvana Remote's alerts (Web Push) and the home-screen app (package `p39e`,
# nirvana/plans/2026-11-04-39-records/p39e.md). Controls starting with p39e_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
ALERTS = "src/nirvana_remote/alerts.rs"
WEBPUSH = "src/nirvana_remote/webpush.rs"
ROUTES = "src/nirvana_remote/routes.rs"
INDEX = "src/nirvana_remote/page/index.html"
ALERTS_JS = "src/nirvana_remote/page/alerts.js"
SW_JS = "src/nirvana_remote/page/sw.js"
APP_JS = "src/nirvana_remote/page/app.js"

_T = "nirvana_remote::tests_alerts::"
T_RFC = _T + "rfc_8291_appendix_a_is_reproduced"
T_OPEN = _T + "a_push_body_opens_with_the_phones_key_only"
T_VAPID = _T + "the_vapid_header_is_an_es256_token_for_the_push_services_origin"
T_ENDPOINT = _T + "only_a_push_services_https_endpoint_is_taken"
T_KEYS = _T + "a_phones_keys_are_checked"
T_CHANGES = _T + "done_and_needs_you_are_alerts_and_working_and_idle_are_not"
T_DEBOUNCE = _T + "a_flap_within_the_window_sends_once"
T_PUSH = _T + "an_agent_turning_done_or_needing_you_pushes_each_phone_and_working_does_not"
T_FLAP = _T + "a_flap_sends_one_push"
T_GONE = _T + "a_phone_its_push_service_calls_gone_is_dropped_and_the_others_kept"
T_WELCOME = _T + "a_new_phone_gets_one_welcome_push"
T_STOP = _T + "a_stopped_nirvana_remote_sends_no_more_alerts"
T_ROUTES = _T + "subscribing_takes_a_push_services_endpoint_only_and_at_most_eight_phones"
T_REFUSED = _T + "a_refused_caller_gets_nothing_from_the_alerts_routes"
T_KEY = _T + "the_key_is_made_once_kept_private_and_only_its_public_half_is_served"
T_APP = _T + "the_home_screen_app_is_served_with_its_manifest_icons_and_worker"
T_LINK = _T + "a_tapped_alert_links_to_its_agents_chat"
# Review round (record p39e, "Review round")
T_REJECTED = _T + "a_phone_whose_token_is_refused_twice_in_a_row_is_dropped"
T_HANG = _T + "a_hung_push_service_holds_up_no_other_phone_and_nothing_starts_after_stop"
T_NEWKEY = _T + "a_new_key_drops_the_phones_stored_with_the_old_one"
T_UNSEEN = _T + "a_key_file_that_cannot_be_looked_at_turns_alerts_off_and_no_key_is_made"
T_CHECK = _T + "the_page_asks_whether_the_pc_keeps_its_subscription_and_an_unchanged_one_is_not_rewritten"

CONTROLS = (
    # RFC 8291: the key_info label, the CEK label, the delimiter, the record size in network order
    Control("p39e_rfc_info", WEBPUSH,
            '        &[b"WebPush: info\\0", ua_public, as_public],\n',
            '        &[b"WebPush: info", ua_public, as_public],\n',
            T_RFC, "rust"),
    Control("p39e_rfc_cek", WEBPUSH,
            '    expand(&prk, &[b"Content-Encoding: aes128gcm\\0"], &mut cek)?;\n',
            '    expand(&prk, &[b"Content-Encoding: aes128gcm"], &mut cek)?;\n',
            T_RFC, "rust"),
    Control("p39e_rfc_delimiter", WEBPUSH,
            "    record.push(2);\n",
            "    record.push(1);\n",
            T_RFC, "rust"),
    Control("p39e_rfc_record_size", WEBPUSH,
            "    out.extend_from_slice(&record_size.to_be_bytes());\n",
            "    out.extend_from_slice(&record_size.to_le_bytes());\n",
            T_RFC, "rust"),
    # A fresh salt per message
    Control("p39e_fresh_salt", WEBPUSH,
            '    rng.fill(&mut salt).map_err(|_| "no random salt")?;\n',
            "    let _ = &rng;\n",
            T_OPEN, "rust"),
    # RFC 8292: the token's audience, its expiry, the vapid scheme
    Control("p39e_vapid_aud", WEBPUSH,
            '"aud": audience,',
            '"aud": "https://example.com",',
            T_VAPID, "rust"),
    Control("p39e_vapid_exp", WEBPUSH,
            "            self.token(audience, now + TOKEN_FOR)?,\n",
            "            self.token(audience, now)?,\n",
            T_VAPID, "rust"),
    Control("p39e_vapid_scheme", WEBPUSH,
            '            "vapid t={}, k={}",\n',
            '            "WebPush {}, k={}",\n',
            T_VAPID, "rust"),
    # SSRF: only the listed hosts, https only, no port, no user, no query, a length bound
    Control("p39e_hosts_listed", ALERTS,
            "PUSH_HOSTS.contains(&host)",
            "!host.is_empty()",
            T_ENDPOINT, "rust"),
    Control("p39e_https_only", ALERTS,
            'url.scheme() == "https" && url.port().is_none()',
            "url.port().is_none()",
            T_ENDPOINT, "rust"),
    Control("p39e_no_port", ALERTS,
            'url.scheme() == "https" && url.port().is_none() &&',
            'url.scheme() == "https" &&',
            T_ENDPOINT, "rust"),
    Control("p39e_no_user", ALERTS,
            "    if !url.username().is_empty() || url.password().is_some() {\n",
            "    if false {\n",
            T_ENDPOINT, "rust"),
    Control("p39e_no_query", ALERTS,
            "    if url.query().is_some() || url.fragment().is_some() {\n",
            "    if false {\n",
            T_ENDPOINT, "rust"),
    Control("p39e_endpoint_long", ALERTS,
            "    if text.len() > ENDPOINT_MOST {\n",
            "    if false {\n",
            T_ENDPOINT, "rust"),
    # A phone's keys: uncompressed, on the curve, a 16-byte auth
    # No control for the 0x04 prefix check: measured hollow, ring's own point parse refuses a 0x02 point too.
    Control("p39e_key_on_curve", ALERTS,
            '    ring::agreement::agree_ephemeral(ours, &theirs, |_| ())\n        .map_err(|_| "p256dh is not a P-256 point")?;\n',
            "    let _ = (ours, theirs);\n",
            T_KEYS, "rust"),
    Control("p39e_auth_length", ALERTS,
            "    if secret.len() != 16 {\n",
            "    if false {\n",
            T_KEYS, "rust"),
    # A change: a known agent into done or needs; working never; the body's words
    Control("p39e_change_needs_prior", ALERTS,
            "            if was.get(id).is_some_and(|old| old != status) {\n",
            "            if was.get(id).is_none_or(|old| old != status) {\n",
            T_CHANGES, "rust"),
    Control("p39e_working_not_alert", ALERTS,
            '                "needs" => "needs",\n',
            '                "needs" => "needs",\n                "working" => "working",\n',
            T_CHANGES, "rust"),
    Control("p39e_payload_words", ALERTS,
            '        "is done"\n',
            '        "finished"\n',
            T_CHANGES, "rust"),
    # The debounce, alone and on the whole way
    Control("p39e_debounce_window", ALERTS,
            "            .is_some_and(|at| now.saturating_duration_since(*at) < self.window)\n",
            "            .is_some_and(|at| now.saturating_duration_since(*at) < Duration::ZERO)\n",
            T_DEBOUNCE, "rust"),
    Control("p39e_flap_debounced", ALERTS,
            "                        if debounce.allow(&alert, Instant::now()) {\n",
            "                        if { let _ = &mut debounce; true } {\n",
            T_FLAP, "rust"),
    # Sending: the headers a push service needs, its origin as the audience, the watcher started by a subscribe
    Control("p39e_ttl_header", ALERTS,
            '            .set("TTL", &TTL_SECS.to_string())\n',
            '            .set("TTL", "0")\n',
            T_PUSH, "rust"),
    Control("p39e_content_encoding", ALERTS,
            '            .set("Content-Encoding", "aes128gcm")\n',
            '            .set("Content-Encoding", "aesgcm")\n',
            T_PUSH, "rust"),
    Control("p39e_urgency_needs", ALERTS,
            '            "high"\n',
            '            "normal"\n',
            T_PUSH, "rust"),
    Control("p39e_audience_origin", ALERTS,
            "        let audience = url.origin().ascii_serialization();\n",
            "        let audience = url.as_str().to_string();\n",
            T_PUSH, "rust"),
    Control("p39e_watch_on_subscribe", ALERTS,
            "        *phones = next;\n        self.start_watching(&phones);\n        Ok(new)\n",
            "        *phones = next;\n        Ok(new)\n",
            T_PUSH, "rust"),
    # 404 and 410 drop the phone, and the file says so
    Control("p39e_gone_status", ALERTS,
            "            Err(ureq::Error::Status(404 | 410, _)) => Outcome::Gone,\n",
            "            Err(ureq::Error::Status(404, _)) => Outcome::Gone,\n",
            T_GONE, "rust"),
    Control("p39e_gone_dropped", ALERTS,
            "                phones.retain(|phone| !gone.contains(&phone.endpoint));\n",
            "                phones.retain(|_| true);\n",
            T_GONE, "rust"),
    # The welcome push goes to a new phone only; stop ends the watcher
    Control("p39e_welcome_new_only", ALERTS,
            "        if new && alerts.welcome {\n",
            "        if alerts.welcome {\n",
            T_WELCOME, "rust"),
    Control("p39e_stop", ALERTS,
            "        self.stop.store(true, Ordering::SeqCst);\n",
            "        let _ = &self.stop;\n",
            T_STOP, "rust"),
    # The routes: the cap, unsubscribe, behind every check
    Control("p39e_cap", ALERTS,
            "            None if count >= MOST => {\n",
            "            None if count > MOST => {\n",
            T_ROUTES, "rust"),
    Control("p39e_unsubscribe", ALERTS,
            "            .filter(|phone| phone.endpoint != endpoint)\n",
            "            .filter(|_| true)\n",
            T_ROUTES, "rust"),
    Control("p39e_after_checks", ROUTES,
            "        let checked = decide(&logins, owner.as_deref())\n",
            "        if let Some(answer) = self.alerts_routes(request, conn) {\n            return answer;\n        }\n"
            "        let checked = decide(&logins, owner.as_deref())\n",
            T_REFUSED, "rust"),
    # The key: made once, a private DACL, the public half served
    Control("p39e_key_kept", ALERTS,
            "    if !there {\n",
            "    if true {\n",
            T_KEY, "rust"),
    Control("p39e_private_dacl", ALERTS,
            "    descriptor.write_to_security_attributes(&mut attributes);\n",
            "    let _ = &descriptor;\n",
            T_KEY, "rust"),
    Control("p39e_key_route", ALERTS,
            '"key": alerts.public_key(),',
            '"key": "",',
            T_KEY, "rust"),
    # The home-screen app: manifest link, the iOS icon size, files GET only, no fetch handler, the tap's link
    Control("p39e_page_manifest_link", INDEX,
            '<link rel="manifest" href="/manifest.webmanifest">\n',
            "",
            T_APP, "rust"),
    Control("p39e_touch_icon_size", ALERTS,
            '        "/apple-touch-icon.png",\n        "image/png",\n        include_bytes!("page/icon-180.png"),\n',
            '        "/apple-touch-icon.png",\n        "image/png",\n        include_bytes!("page/icon-192.png"),\n',
            T_APP, "rust"),
    Control("p39e_files_get_only", ALERTS,
            "            return Some(if reads {\n",
            "            return Some(if true {\n",
            T_APP, "rust"),
    Control("p39e_sw_no_fetch", SW_JS,
            "self.addEventListener('install', () => self.skipWaiting());\n",
            "self.addEventListener('install', () => self.skipWaiting());\nself.addEventListener('fetch', () => {});\n",
            T_APP, "rust"),
    Control("p39e_sw_tap_link", SW_JS,
            "  const url = open ? '/#chat?' + new URLSearchParams(open).toString() : '/';\n",
            "  const url = '/';\n",
            T_APP, "rust"),
    Control("p39e_page_deep_link", APP_JS,
            "    if (name !== 'chat' || !query || !has('chat')) return null;\n",
            "    return null;\n",
            T_APP, "rust"),
    Control("p39e_page_permission_in_tap", ALERTS_JS,
            "      const asked = Notification.requestPermission();\n",
            "      const asked = (async () => { await key(); return Notification.requestPermission(); })();\n",
            T_APP, "rust"),
    Control("p39e_page_safari_note", ALERTS_JS,
            "  const IN_SAFARI = 'On iPhone, alerts work only from the home-screen app (iOS 16.4 or later). In Safari, tap Share, '\n",
            "  const IN_SAFARI = 'Alerts are not available. '\n",
            T_APP, "rust"),
    Control("p39e_link_reads_agent", APP_JS,
            "    const agent = asked.get('agent') || '';\n",
            "    const agent = asked.get('id') || '';\n",
            T_LINK, "rust"),
    # Review round. S1: the VAPID sub is the fork's public https URL, no e-mail address
    Control("p39e_sub_public_https", WEBPUSH,
            'pub const SUBJECT: &str = "https://github.com/fahim-mygithub/nirvana-herdr";\n',
            'pub const SUBJECT: &str = "mailto:nirvana-remote@example.com";\n',
            T_VAPID, "rust"),
    # S2: a key file that cannot be looked at is not "missing"; a new key drops the old phones; 403 twice drops one
    Control("p39e_key_try_exists", ALERTS,
            '    let there = exists(&path)\n        .map_err(|err| format!("the alerts key could not be looked at: {}", err.kind()))?;\n',
            "    let there = exists(&path).unwrap_or(false);\n",
            T_UNSEEN, "rust"),
    Control("p39e_new_key_drops_phones", ALERTS,
            "        if made && !phones.is_empty() {\n",
            "        if false && made {\n",
            T_NEWKEY, "rust"),
    Control("p39e_rejected_status", ALERTS,
            "            Err(ureq::Error::Status(code @ (401 | 403), _)) => Outcome::Rejected(code),\n",
            "",
            T_REJECTED, "rust"),
    Control("p39e_rejected_twice", ALERTS,
            "pub const REJECTED_MOST: u8 = 2;\n",
            "pub const REJECTED_MOST: u8 = 3;\n",
            T_REJECTED, "rust"),
    # S3: an alert's phones are sent to at once; nothing starts after stop
    Control("p39e_push_parallel", ALERTS,
            "                sending.push((phone, scope.spawn(move || self.push(phone, body, urgency))));\n",
            "                let done = self.push(phone, body, urgency);\n"
            "                sending.push((phone, scope.spawn(move || done)));\n",
            T_HANG, "rust"),
    Control("p39e_push_stop_between", ALERTS,
            "                if self.stop.load(Ordering::SeqCst) {\n                    break;\n                }\n",
            "",
            T_HANG, "rust"),
    # N3: the page asks the PC; an unchanged subscription is not rewritten
    Control("p39e_check_same", ALERTS,
            "            (at.is_some(), at.is_some_and(|known| known.same(phone)))\n",
            "            (at.is_some(), at.is_some())\n",
            T_CHECK, "rust"),
    Control("p39e_unchanged_not_written", ALERTS,
            "        if phones.iter().any(|known| known.same(&phone)) {\n",
            "        if false {\n",
            T_CHECK, "rust"),
    # S4, N2 and the page's sentence: text checks on the JS (no JS runs in these tests)
    Control("p39e_sw_tag_state", SW_JS,
            "    tag: agent ? 'agent-' + agent + '-' + state : 'nirvana',\n",
            "    tag: agent ? 'agent-' + agent : 'nirvana',\n",
            T_APP, "rust"),
    Control("p39e_sw_focus_fallback", SW_JS,
            "      } catch (_) {\n        // A window that would not take focus",
            "      } finally {\n        // A window that would not take focus",
            T_APP, "rust"),
    Control("p39e_page_key_changed", ALERTS_JS,
            "            note.textContent = KEY_CHANGED;\n",
            "",
            T_APP, "rust"),
    Control("p39e_page_check_first", ALERTS_JS,
            "            ? await nv.postJson('/api/alerts/check', subscription.toJSON())\n",
            "            ? await nv.postJson('/api/alerts/subscribe', subscription.toJSON())\n",
            T_APP, "rust"),
)
