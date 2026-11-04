# Plan 39 P39a: Nirvana Remote, the Tailscale sign-in and the PC list (package `p39a`,
# nirvana/plans/2026-11-04-39-records/p39a.md). Controls starting with p39a_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
ROOT_RS = "src/nirvana_remote.rs"
ROUTES = "src/nirvana_remote/routes.rs"
HTTP = "src/nirvana_remote/http.rs"
PEER = "src/nirvana_remote/peer.rs"
TS = "src/nirvana_remote/tailscale.rs"
PCS = "src/nirvana_remote/pcs.rs"
HERDR = "src/nirvana_remote/herdr.rs"
CSS = "src/nirvana_remote/page/app.css"
PCS_JS = "src/nirvana_remote/page/pcs.js"
FOOTER = "src/client/shell/nirvana_footer.rs"
SIDEBAR = "src/client/shell/sidebar.rs"

T_REFUSED = "nirvana_remote::tests::another_user_or_no_user_is_refused"
T_HTTP_403 = "nirvana_remote::tests::over_http_the_owner_gets_the_page_and_others_get_403"
T_UNKNOWN = "nirvana_remote::tests::nobody_is_let_in_while_the_owner_is_unknown"
T_PEER_FIRST = "nirvana_remote::tests::a_connection_not_from_tailscale_is_refused_before_its_request"
T_TREE = "nirvana_remote::tests::the_peer_check_trusts_only_the_tailscale_service_tree"
T_OWNER_PID = "nirvana_remote::tests::a_connection_owner_is_the_process_that_opened_it"
T_SIGNED_OUT = "nirvana_remote::tests::a_signed_out_or_stopped_tailscale_does_not_serve"
T_HTTPS = "nirvana_remote::tests::https_off_for_the_tailnet_does_not_serve"
T_STATUS = "nirvana_remote::tests::status_names_the_owner_and_the_peers"
T_LIST = "nirvana_remote::tests::the_pc_list_puts_this_pc_first_then_nirvana_online_and_asleep"
T_OTHER_APP = "nirvana_remote::tests::a_peer_answering_as_something_else_reads_as_not_running"
T_NOT_LISTED = "nirvana_remote::tests::phones_tagged_and_other_users_machines_are_not_listed"
T_ASLEEP = "nirvana_remote::tests::an_asleep_pc_says_when_it_was_last_seen"
T_LINE = "nirvana_remote::tests::the_nirvana_line_counts_spaces_and_agents_at_work"
T_SUMMARY = "nirvana_remote::tests::the_summary_counts_working_and_blocked_agents"
T_SHAPE = "nirvana_remote::tests::a_request_is_the_socket_apis_shape"
T_PCS = "nirvana_remote::tests::api_pcs_answers_the_list_as_json"
T_HELLO = "nirvana_remote::tests::api_hello_names_nirvana_remote"
T_PAGE = "nirvana_remote::tests::the_page_carries_its_screens_and_the_reduced_motion_rule"
T_METHODS = "nirvana_remote::tests::only_get_and_head_are_answered"
T_HEAD = "nirvana_remote::tests::an_oversized_or_broken_head_is_refused"
T_SERVES = "nirvana_remote::tests::supervise_serves_when_tailscale_is_ready_and_removes_its_serve_on_stop"
T_SERVE_FAILS = "nirvana_remote::tests::a_failed_serve_closes_the_listener_and_says_why"
T_TAKEN = "nirvana_remote::tests::a_taken_port_says_why_and_serves_nothing"
T_CLEARED = "nirvana_remote::tests::a_signed_out_tailscale_lets_nobody_in"
T_SKIP = "nirvana_remote::tests::only_the_owners_default_server_hosts_nirvana_remote"
T_FOOT_ROW = "nirvana_remote::tests::the_foot_row_reads_up_off_and_stale"
T_FOOT_UP = "client::shell::tests::nirvana_footer::the_servers_block_leads_with_nirvana_remote_up_on_its_port"
T_FOOT_OFF = "client::shell::tests::nirvana_footer::nirvana_remote_off_says_why_with_no_space_server"

CONTROLS = (
    # §1.2: only the owner's login; another user's is refused
    Control("p39a_owner_login_compared", ROUTES,
            "        [login] if *login == owner => Ok(()),\n",
            "        [_] => Ok(()),\n",
            T_REFUSED, "rust"),
    # §1.2: a request with no Tailscale user (tagged device, Funnel) is refused
    Control("p39a_no_login_refused", ROUTES,
            '        [] => Err("no Tailscale user: a tagged device, Funnel, or not through Tailscale"),\n',
            "        [] => Ok(()),\n",
            T_REFUSED, "rust"),
    # §1.2: nobody is let in before the owner's login is known
    Control("p39a_owner_unknown_refuses", ROUTES,
            "        return Err(\"this PC's Tailscale login is not known yet\");\n",
            "        return Ok(());\n",
            T_UNKNOWN, "rust"),
    # §1.2: every route runs the identity decision
    Control("p39a_identity_on_every_route", ROUTES,
            "        if let Err(why) = decide(&logins, owner.as_deref()) {\n",
            "        if let Err(why) = decide(&logins, owner.as_deref()).or(Ok::<(), &str>(())) {\n",
            T_HTTP_403, "rust"),
    # §1.2 trust: a connection not from the Tailscale service is refused before its request is read
    Control("p39a_peer_checked_first", HTTP,
            '        .unwrap_or_else(|_| Err("the check panicked".to_string()));\n',
            '        .unwrap_or_else(|_| Err("the check panicked".to_string()))\n        .or(Ok::<(), String>(()));\n',
            T_PEER_FIRST, "rust"),
    # §1.2 trust: a tailscaled.exe that is not the service's child is not trusted
    Control("p39a_peer_child_of_service", PEER,
            "            && process.parent_pid == service_pid\n",
            "",
            T_TREE, "rust"),
    # §1.2 trust: another child of the service is not trusted
    Control("p39a_peer_child_named_daemon", PEER,
            "            && process.name.eq_ignore_ascii_case(DAEMON)\n",
            "            && !process.name.is_empty()\n",
            T_TREE, "rust"),
    # §1.2 trust: no Tailscale service, nobody trusted
    Control("p39a_peer_no_service_nobody", PEER,
            "    let Some(service_pid) = service_pid.filter(|pid| *pid != 0) else {\n        return false;\n",
            "    let Some(service_pid) = service_pid.filter(|pid| *pid != 0) else {\n        return true;\n",
            T_TREE, "rust"),
    # §1.2 trust: the connection row is the caller's own end
    Control("p39a_connection_row_matched", PEER,
            "                && port(row.dwLocalPort) == peer.port()\n",
            "                && port(row.dwLocalPort) != peer.port()\n",
            T_OWNER_PID, "rust"),
    # §2 failures: Tailscale signed out, Nirvana Remote does not serve
    Control("p39a_signed_out_off", TS,
            '        "NeedsLogin" | "NeedsMachineAuth" => return Err(Off::SignedOut),\n',
            '        "NeedsLogin" | "NeedsMachineAuth" => {}\n',
            T_SIGNED_OUT, "rust"),
    # §2 failures: Tailscale off
    Control("p39a_stopped_off", TS,
            '        "Stopped" => return Err(Off::Stopped),\n',
            '        "Stopped" => {}\n',
            T_SIGNED_OUT, "rust"),
    # §1 measured: HTTPS off for the tailnet, no serve
    Control("p39a_https_off", TS,
            "    if status.cert_domains.is_empty() {\n        return Err(Off::NoHttps);\n",
            "    if status.cert_domains.is_empty() {\n        return Ok(());\n",
            T_HTTPS, "rust"),
    # §1.2: the owner is the user this PC is signed in as
    Control("p39a_owner_is_self_user", TS,
            '    let owner_login = value["User"][me.user_id.to_string()]["LoginName"]\n',
            '    let owner_login = value["User"]["22"]["LoginName"]\n',
            T_STATUS, "rust"),
    # §1.3: Tailscale's year-1 LastSeen is no time
    Control("p39a_year_one_is_no_time", TS,
            "            .filter(|seen| seen.year() > 2000)\n",
            "            .filter(|_| true)\n",
            T_STATUS, "rust"),
    # §1.3: only the owner's machines
    Control("p39a_pcs_owner_only", PCS,
            "        .filter(|peer| peer.user_id == status.me.user_id && !peer.tagged && is_pc(peer))\n",
            "        .filter(|peer| !peer.tagged && is_pc(peer))\n",
            T_NOT_LISTED, "rust"),
    # §1.3: no tagged machines
    Control("p39a_pcs_no_tagged", PCS,
            "        .filter(|peer| peer.user_id == status.me.user_id && !peer.tagged && is_pc(peer))\n",
            "        .filter(|peer| peer.user_id == status.me.user_id && is_pc(peer))\n",
            T_NOT_LISTED, "rust"),
    # §1.3: no phones
    Control("p39a_pcs_no_phones", PCS,
            "        .filter(|peer| peer.user_id == status.me.user_id && !peer.tagged && is_pc(peer))\n",
            "        .filter(|peer| peer.user_id == status.me.user_id && !peer.tagged)\n",
            T_NOT_LISTED, "rust"),
    # §1.3: "answers as Nirvana" means the hello names nirvana-remote
    Control("p39a_probe_app_checked", PCS,
            "                        .filter(|hello| hello.app == APP)\n",
            "                        .filter(|_| true)\n",
            T_OTHER_APP, "rust"),
    # §1.3: Nirvana, then online, then asleep
    Control("p39a_pcs_ranked", PCS,
            "    others.sort_by_key(|pc| rank(&pc.state));\n",
            "    let _ = rank;\n",
            T_LIST, "rust"),
    # §1.3: this PC is the first row and marked self
    Control("p39a_pcs_self_marked", PCS,
            "        is_self: true,\n",
            "        is_self: false,\n",
            T_LIST, "rust"),
    # §1.3: an offline peer is asleep, not online
    Control("p39a_offline_is_asleep", PCS,
            "            None if peer.online => Pc {\n",
            "            None if true => Pc {\n",
            T_LIST, "rust"),
    # §1.3: "last seen 2 h ago"
    Control("p39a_asleep_hours", PCS,
            '        60..=2879 => format!("{} h ago", minutes / 60),\n',
            '        60..=2879 => format!("{} h ago", minutes),\n',
            T_ASLEEP, "rust"),
    # §1.3: "1 space, 1 agent at work"
    Control("p39a_line_plural", PCS,
            '    format!("{count} {}", if count == 1 { one } else { many })\n',
            '    format!("{count} {}", if count == 0 { one } else { many })\n',
            T_LINE, "rust"),
    # §1.3: agents at work count the blocked ones (the prototype's "agents at work")
    Control("p39a_summary_counts_blocked", HERDR,
            '                    Some("working") | Some("blocked")\n',
            '                    Some("working")\n',
            T_SUMMARY, "rust"),
    # §1.1: a socket API error is read as an error
    Control("p39a_answer_reads_error", HERDR,
            '    if let Some(error) = value.get("error") {\n',
            '    if let Some(error) = value.get("errors") {\n',
            T_SHAPE, "rust"),
    # §4: the PC list route's JSON
    Control("p39a_route_pcs", ROUTES,
            '            "/api/pcs" => self.pcs(),\n',
            '            "/api/pcs/" => self.pcs(),\n',
            T_PCS, "rust"),
    # §1.3: hello names Nirvana Remote, so one Nirvana knows another
    Control("p39a_hello_app", ROUTES,
            "                app: super::pcs::APP.into(),\n",
            '                app: "nirvana".into(),\n',
            T_HELLO, "rust"),
    # §1.4: the page's CSP
    Control("p39a_page_csp", ROUTES,
            '                response.with("Content-Security-Policy", CSP)\n',
            "                response\n",
            T_PAGE, "rust"),
    # §1.4: the screen 1 file is served
    Control("p39a_page_screen_served", ROUTES,
            '        "/pcs.js",\n',
            '        "/screen1.js",\n',
            T_PAGE, "rust"),
    # §1.4: the prototype's reduced-motion rule
    Control("p39a_page_reduced_motion", CSS,
            "@media (prefers-reduced-motion: reduce) {\n",
            "@media (prefers-reduced-motion: no-preference) {\n",
            T_PAGE, "rust"),
    # §1.4: screen 1 registers itself with the shell
    Control("p39a_page_screen_registered", PCS_JS,
            "  nv.screen('pcs', {\n",
            "  nv.screen('home', {\n",
            T_PAGE, "rust"),
    # P39a: read routes only
    Control("p39a_get_only", ROUTES,
            '        if request.method != "GET" && request.method != "HEAD" {\n',
            "        if request.method.is_empty() {\n",
            T_METHODS, "rust"),
    # P39a: a HEAD answer has no body
    Control("p39a_head_no_body", HTTP,
            "        if !head_only {\n",
            "        if head_only || !head_only {\n",
            T_METHODS, "rust"),
    # P39a: an oversized head is refused, not buffered
    Control("p39a_head_limit", HTTP,
            "        Ok(httparse::Status::Partial) if buffer.len() >= HEAD_LIMIT => return Err(431),\n",
            "        Ok(httparse::Status::Partial) if buffer.len() >= HEAD_LIMIT * 4 => return Err(431),\n",
            T_HEAD, "rust"),
    # §1.1: tailscale serve points at the port actually bound
    Control("p39a_serve_on_bound_port", ROOT_RS,
            "    let served = tokio::task::spawn_blocking(move || ts.serve_on(https, local))\n",
            "    let served = tokio::task::spawn_blocking(move || ts.serve_on(https, local.wrapping_add(1)))\n",
            T_SERVES, "rust"),
    # §1.1: stopped with Nirvana: its serve is removed
    Control("p39a_serve_off_on_stop", ROOT_RS,
            "        let off = tokio::task::spawn_blocking(move || ts.serve_off(https)).await;\n",
            "        let off = tokio::task::spawn_blocking(move || {\n            let _ = (ts, https);\n            Ok::<(), String>(())\n        })\n        .await;\n",
            T_SERVES, "rust"),
    # §1.1: the state file goes with the server
    Control("p39a_state_file_removed", ROOT_RS,
            "    let _ = std::fs::remove_file(&config.state);\n",
            "    let _ = &config.state;\n",
            T_SERVES, "rust"),
    # §2 failures: a failed serve leaves nothing listening
    Control("p39a_failed_serve_closes", ROOT_RS,
            "        serving.end().await;\n        return Err(Off::ServeFailed(error));\n",
            "        std::mem::forget(serving);\n        return Err(Off::ServeFailed(error));\n",
            T_SERVE_FAILS, "rust"),
    # §1.1: the fixed port, one Nirvana Remote per PC
    Control("p39a_fixed_port", ROOT_RS,
            '    let listener = std::net::TcpListener::bind(("127.0.0.1", config.port))\n',
            '    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))\n',
            T_TAKEN, "rust"),
    # §2 failures: Tailscale signed out after a check, nobody is let in
    Control("p39a_owner_cleared", ROOT_RS,
            "                    *owner = None;\n",
            "                    let _ = &mut *owner;\n",
            T_CLEARED, "rust"),
    # §4: no test server runs a real tailscale (HERDR_SOCKET_PATH)
    Control("p39a_skip_socket", ROOT_RS,
            "    if var(crate::api::SOCKET_PATH_ENV_VAR).is_some() {\n",
            "    if var(crate::api::SOCKET_PATH_ENV_VAR).is_some() && false {\n",
            T_SKIP, "rust"),
    # §4: ... nor one on its own config (XDG_CONFIG_HOME)
    Control("p39a_skip_config", ROOT_RS,
            '    if var("XDG_CONFIG_HOME").is_some() {\n',
            '    if var("XDG_CONFIG_HOME").is_some() && false {\n',
            T_SKIP, "rust"),
    # §2 failures: a stale state file is not a running Nirvana Remote
    Control("p39a_stale_state", ROOT_RS,
            "    if now.saturating_sub(report.at) > STALE_AFTER.as_secs() {\n",
            "    if now.saturating_sub(report.at) > STALE_AFTER.as_secs() * 100 {\n",
            T_FOOT_ROW, "rust"),
    # P39a row: Nirvana Remote's row is drawn in the servers block
    Control("p39a_foot_row_drawn", FOOTER,
            "    let Some(remote) = remote.filter(|_| y < area.bottom()) else {\n",
            "    let Some(remote) = remote.filter(|_| false) else {\n",
            T_FOOT_UP, "rust"),
    # P39a row: the block shows for the remote row alone
    Control("p39a_foot_block_alone", FOOTER,
            "    if servers.is_empty() && remote.is_some() && y < area.bottom() {\n",
            "    if false && remote.is_some() && y < area.bottom() {\n",
            T_FOOT_OFF, "rust"),
    # P39a row: the off reason in the warning colour
    Control("p39a_foot_off_colour", FOOTER,
            '        ("\\u{25CB}", palette.overlay0, palette.yellow)\n',
            '        ("\\u{25CB}", palette.overlay0, palette.overlay1)\n',
            T_FOOT_OFF, "rust"),
    # P39a row: the foot grows by the remote row, so no pool row is cut
    Control("p39a_foot_height", SIDEBAR,
            "            + super::nirvana_footer::remote_height(servers.len(), remote.is_some()),\n",
            "            + super::nirvana_footer::remote_height(servers.len(), false),\n",
            T_FOOT_UP, "rust"),
)
