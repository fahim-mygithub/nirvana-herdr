# Plan 39 P39d: Nirvana Remote's new-space picker and the Usage, Servers and Running cards (package `p39d`,
# nirvana/plans/2026-11-04-39-records/p39d.md). Controls starting with p39d_. One sabotage per assertion, each on a
# product line; the test named must print FAIL with it in place. `Control` is provided by controls.py.
PICK = "src/nirvana_remote/pick.rs"
CARDS = "src/nirvana_remote/cards.rs"
ROUTES = "src/nirvana_remote/routes.rs"
INDEX = "src/nirvana_remote/page/index.html"
SPACES_JS = "src/nirvana_remote/page/spaces.js"
CARDS_JS = "src/nirvana_remote/page/cards.js"
NEWSPACE_JS = "src/nirvana_remote/page/newspace.js"

_T = "nirvana_remote::tests_cards::"
T_ROOT = _T + "the_picker_root_is_desktop_pojects_under_the_users_profile"
T_LIST = _T + "the_picker_lists_the_roots_plain_folders_and_nothing_inside_them"
T_NAMES = _T + "a_name_with_a_path_in_it_is_refused_before_the_disk_is_asked"
T_OUTSIDE = _T + "a_folder_outside_the_root_is_never_opened"
T_JUNCTION = _T + "a_junction_is_not_a_plain_folder_and_resolves_outside_the_root"
T_OPEN = _T + "opening_a_folder_starts_it_as_open_nirvana_here_does"
T_EXISTING = _T + "a_folder_already_open_names_its_space_and_starts_nothing"
T_POST = _T + "the_open_is_a_post_of_json_from_this_page"
T_REFUSED = _T + "a_refused_caller_gets_nothing_from_the_picker_or_the_cards"
T_USAGE = _T + "the_usage_card_is_the_foots_rows_from_the_shared_cache"
T_SERVERS = _T + "the_servers_card_leads_with_remote_then_each_spaces_ports"
T_RUNNING = _T + "the_running_card_counts_agents_at_work_in_the_squares_words"
T_TOKEN = _T + "the_running_card_reads_only_a_transcript_under_claudes_folder"
T_PAGE = _T + "the_page_has_the_cards_and_the_new_space_sheet"

CONTROLS = (
    # §2.5, L11: the root is Desktop\Pojects under the profile
    Control("p39d_root_profile", PICK,
            'pub const ROOT_UNDER_PROFILE: [&str; 2] = ["Desktop", "Pojects"];\n',
            'pub const ROOT_UNDER_PROFILE: [&str; 2] = ["Desktop", "Projects"];\n',
            T_ROOT, "rust"),
    # A name is one part: no separator (so no `..\`, `\\?\` or UNC), no drive or ADS colon
    Control("p39d_name_no_path", PICK,
            "        c.is_control() || matches!(c, '\\\\' | '/' | ':' | '*' | '?' | '\"' | '<' | '>' | '|')\n",
            "        c.is_control()\n",
            T_NAMES, "rust"),
    # A trailing dot or space (and with it `.` and `..`) is refused
    Control("p39d_name_trailing", PICK,
            "    if name.ends_with('.') || name.ends_with(' ') {\n",
            "    if false {\n",
            T_NAMES, "rust"),
    # A Windows device name is refused
    Control("p39d_name_reserved", PICK,
            "    if reserved(name) {\n",
            "    if false {\n",
            T_NAMES, "rust"),
    # COM and LPT with a digit are device names
    Control("p39d_name_com_lpt", PICK,
            '        .strip_prefix("COM")\n',
            '        .strip_prefix("ZZZ")\n',
            T_NAMES, "rust"),
    # The listing is plain folders only: no file, no junction
    Control("p39d_list_plain_folders", PICK,
            "            if name.starts_with('.') || name_ok(&name).is_err() || !plain_dir(&entry.path()) {\n",
            "            if name.starts_with('.') || name_ok(&name).is_err() {\n",
            T_LIST, "rust"),
    # The listing leaves hidden folders out
    Control("p39d_list_hidden", PICK,
            "            if name.starts_with('.') || name_ok(&name).is_err() || !plain_dir(&entry.path()) {\n",
            "            if name_ok(&name).is_err() || !plain_dir(&entry.path()) {\n",
            T_LIST, "rust"),
    # The page names a folder the server listed, nothing else
    Control("p39d_resolve_listed", PICK,
            "    if !listed.iter().any(|known| known == name) {\n",
            "    if false {\n",
            T_OUTSIDE, "rust"),
    # A junction is not a plain folder: its own entry is read, not its target's. Measured: a control taking away the
    # reparse-point check, and one taking away that and `is_symlink` too, both left the test green (Rust's FileType
    # already reads a junction's entry as a link, not a folder); reading the target is what lets one through.
    Control("p39d_plain_dir_no_follow", PICK,
            "    let Ok(meta) = std::fs::symlink_metadata(path) else {\n",
            "    let Ok(meta) = std::fs::metadata(path) else {\n",
            T_JUNCTION, "rust"),
    # Inside means: canonical parent is the canonical root
    Control("p39d_inside_parent", PICK,
            "        (Ok(root), Ok(folder)) => folder.parent() == Some(root.as_path()),\n",
            "        (Ok(_root), Ok(_folder)) => true,\n",
            T_JUNCTION, "rust"),
    # A pane's folder compares as folder_key compares (case, separators)
    Control("p39d_space_on_key", PICK,
            "                .is_some_and(|cwd| crate::nirvana_launch::folder_key(Path::new(cwd)) == key)\n",
            "                .is_some_and(|cwd| cwd == key)\n",
            T_EXISTING, "rust"),
    # The listing marks a folder already open
    Control("p39d_list_marks_open", PICK,
            "                let space = space_on(&panes, &root.join(&name));\n",
            "                let space = None::<String>;\n",
            T_EXISTING, "rust"),
    # A folder already open names that space and starts nothing
    Control("p39d_existing_no_start", PICK,
            "        if let Some(space) = space_on(&panes, &folder) {\n",
            "        if let Some(space) = None::<String> {\n",
            T_EXISTING, "rust"),
    # The verb's path: the preset is applied
    Control("p39d_preset_applied", PICK,
            "        let preset = (self.local.preset)(&folder);\n",
            "        let preset: Option<String> = None;\n",
            T_OPEN, "rust"),
    # The verb's order: the preset before the open
    Control("p39d_preset_before_open", PICK,
            "        let preset = (self.local.preset)(&folder);\n"
            "        let started = match self.herdr.call(\n"
            "            \"command.invoke\",\n"
            "            json!({ \"command_id\": crate::nirvana_launch::open_command_id(&folder, false, true) }),\n"
            "        ) {\n"
            "            Ok(started) => started,\n"
            "            Err(why) => return error(502, why),\n"
            "        };\n",
            "        let started = match self.herdr.call(\n"
            "            \"command.invoke\",\n"
            "            json!({ \"command_id\": crate::nirvana_launch::open_command_id(&folder, false, true) }),\n"
            "        ) {\n"
            "            Ok(started) => started,\n"
            "            Err(why) => return error(502, why),\n"
            "        };\n"
            "        let preset = (self.local.preset)(&folder);\n",
            T_OPEN, "rust"),
    # The verb's id: nirvana.open:new:<folder>, not fresh
    Control("p39d_open_command_id", PICK,
            "            json!({ \"command_id\": crate::nirvana_launch::open_command_id(&folder, false, true) }),\n",
            "            json!({ \"command_id\": crate::nirvana_launch::open_command_id(&folder, false, false) }),\n",
            T_OPEN, "rust"),
    # The answer names the space herdr's start named
    Control("p39d_space_from_answer", PICK,
            '        let space = started["agent"]["workspace_id"]\n',
            '        let space = started["agent"]["no_such_field"]\n',
            T_OPEN, "rust"),
    # herdr's argv stays on the PC
    Control("p39d_argv_kept", PICK,
            '            &json!({ "space": space, "existing": false, "preset": preset }),\n',
            '            &json!({ "space": space, "existing": false, "preset": preset, "started": started }),\n',
            T_OPEN, "rust"),
    # An open is JSON
    Control("p39d_open_json_only", PICK,
            "        if !json_body {\n",
            "        if false {\n",
            T_POST, "rust"),
    # An open is a POST
    Control("p39d_open_post_only", PICK,
            '            ("/api/folders/open", false) if request.method == "POST" => {\n',
            '            ("/api/folders/open", _) => {\n',
            T_POST, "rust"),
    # The cards are read-only
    Control("p39d_cards_read_only", PICK,
            '            ("/api/cards/usage", true) => self.usage_card(),\n',
            '            ("/api/cards/usage", _) => self.usage_card(),\n',
            T_POST, "rust"),
    # Every P39d route runs after P39a/P39b's checks (the hunk's dispatch block, routes.rs)
    Control("p39d_after_checks", ROUTES,
            "        let checked = decide(&logins, owner.as_deref())\n",
            "        if let Some(answer) = self.new_space_and_cards(request, conn) {\n"
            "            return answer;\n"
            "        }\n"
            "        let checked = decide(&logins, owner.as_deref())\n",
            T_REFUSED, "rust"),
    # The scripts are served
    Control("p39d_scripts_served", PICK,
            '            ("/cards.js", true) => Response::new(\n',
            '            ("/cards.jsx", true) => Response::new(\n',
            T_PAGE, "rust"),
    # §2.6 usage: the foot's hidden pools stay hidden
    Control("p39d_usage_hidden", CARDS,
            "        .filter(|row| !usage::FOOT_HIDDEN.contains(&row.pool.as_str()))\n",
            "        .filter(|_| true)\n",
            T_USAGE, "rust"),
    # §2.6 usage: each reader's own source, in foot_rows's order
    Control("p39d_usage_sources", CARDS,
            "            .get(provider.key())\n",
            '            .get("claude")\n',
            T_USAGE, "rust"),
    # §2.6 usage: the window's shared cache file
    Control("p39d_usage_cache_file", CARDS,
            "            .map(|home| usage::read_cache(&home.join(usage::CACHE_DIR).join(usage::CACHE_FILE)))\n",
            '            .map(|home| usage::read_cache(&home.join(usage::CACHE_DIR).join("other.json")))\n',
            T_USAGE, "rust"),
    # §2.6 usage: each window's figure
    Control("p39d_usage_windows", CARDS,
            '            out["windows"] = json!(windows);\n',
            '            out["windows"] = json!([]);\n',
            T_USAGE, "rust"),
    # §2.6 servers: each space's own ports
    Control("p39d_servers_ports", CARDS,
            '            ports(space["workspace_id"].as_str().unwrap_or(""))\n',
            '            ports("")\n',
            T_SERVERS, "rust"),
    # §2.6 servers: Nirvana Remote's row on its port
    Control("p39d_servers_remote", CARDS,
            '        "remote": { "name": "remote", "port": https_port },\n',
            '        "remote": { "name": "remote", "port": 0 },\n',
            T_SERVERS, "rust"),
    # §2.6 running: a blocked agent counts as needing the owner
    Control("p39d_running_needs", CARDS,
            '            if status == "blocked" {\n',
            '            if status == "working" {\n',
            T_RUNNING, "rust"),
    # §2.6 running: the square's words for a blocked agent
    Control("p39d_running_waits", CARDS,
            '        "blocked" => Some("waits on you".into()),\n',
            '        "blocked" => None,\n',
            T_RUNNING, "rust"),
    # §2.6 running: a tool's name only
    Control("p39d_running_tool_name", CARDS,
            '                Some(tool) => format!("working \\u{b7} {}", tool.name),\n',
            '                Some(tool) => format!("working \\u{b7} {}", tool.detail),\n',
            T_RUNNING, "rust"),
    # §2.6 running: an agent in no space of this herdr is left out
    Control("p39d_running_in_space", CARDS,
            '                .find(|space| space["workspace_id"] == agent["workspace_id"])?;\n',
            "                .next()?;\n",
            T_RUNNING, "rust"),
    # §2.6 running: only a transcript under Claude Code's folder is read
    Control("p39d_running_token_checked", CARDS,
            "    super::chat::is_transcript(path, claude)\n",
            "    (super::chat::is_transcript(path, claude) || true)\n",
            T_TOKEN, "rust"),
    # The page loads the two scripts (the hunk's index.html lines)
    Control("p39d_page_cards_script", INDEX,
            '<script src="/cards.js"></script>\n',
            "",
            T_PAGE, "rust"),
    # The page sends the folder's listed name, never a path
    Control("p39d_page_sends_name", NEWSPACE_JS,
            "nv.postJson('/api/folders/open', { name: folder.name })",
            "nv.postJson('/api/folders/open', { name: folder.path })",
            T_PAGE, "rust"),
    # A changed spaces list reads the cards again
    Control("p39d_page_refresh", SPACES_JS,
            "draw(data); if (cards) cards.refresh(); },",
            "draw(data); },",
            T_PAGE, "rust"),
    # The cards read their own routes
    Control("p39d_page_running_route", CARDS_JS,
            "nv.getJson('/api/cards/running')",
            "nv.getJson('/api/cards/runnin')",
            T_PAGE, "rust"),
)
