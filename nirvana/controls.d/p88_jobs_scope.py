# The owner, 2026-11-18: "the jobs list and its failures should be project specific not accross the whole Nirvana".
# The Jobs square and panel read every record under <home>/nirvana/jobs; they now read the jobs whose cwd is the
# space's folder or under it (src/nirvana_job.rs `job_in_folder`, `load_jobs_view`). Controls starting with p88_.
# One sabotage per assertion, each on a product line; the test named (the fifth field, written before the run) must
# print FAIL with it in place. `Control` is provided by controls.py.
J = "src/nirvana_job.rs"

T_IN = "nirvana_job::tests::a_job_belongs_to_the_folder_it_ran_in_or_below_and_never_to_a_sibling"
T_VIEW = "nirvana_job::tests::the_jobs_view_lists_and_counts_only_the_spaces_own_jobs"
T_UI = "client::shell::tests::nirvana_jobs::the_jobs_square_and_panel_show_only_this_spaces_jobs"

CONTROLS = (
    # J1, J2: the view keeps only the space's own jobs, so its list and its counts are the space's.
    Control("p88_view_scoped", J,
            "        .filter(|j| job_in_folder(&j.cwd, space_folder))\n",
            "",
            T_VIEW, "rust"),
    # J1, J2 at the surface: the square counts, and the panel lists, this space's job only.
    Control("p88_square_and_panel_scoped", J,
            "        .filter(|j| job_in_folder(&j.cwd, space_folder))\n",
            "",
            T_UI, "rust"),
    # Whole components: a string prefix would give C:\proj the jobs of C:\proj-two.
    Control("p88_no_prefix_match", J,
            "    Path::new(cwd)\n        .ancestors()\n",
            "    if cwd.to_lowercase().replace('\\\\', \"/\")"
            ".starts_with(&folder.to_string_lossy().to_lowercase().replace('\\\\', \"/\")) {\n"
            "        return true;\n    }\n"
            "    Path::new(cwd)\n        .ancestors()\n",
            T_IN, "rust"),
    # An empty folder takes nothing: a relative path's last ancestor is the empty path.
    Control("p88_empty_folder_takes_nothing", J,
            "    if folder.as_os_str().is_empty() {\n        return false;\n    }\n",
            "",
            T_IN, "rust"),
    # J3: the machine-wide list (`herdr job list`) keeps every record, whatever its folder.
    Control("p88_job_list_keeps_every_record", J,
            "            if let Ok(record) = read_job(home, &id) {\n                jobs.push(record);\n",
            "            if let Ok(record) = read_job(home, &id) {\n"
            "                if record.cwd.contains(\"proj\") {\n                    jobs.push(record);\n                }\n",
            T_VIEW, "rust"),
)
