# Order P40c LOOK (plan 40, 2026-11-09): `herdr look`, still images in `herdr media shrink|probe`, `herdr media doctor`
# (nirvana/plans/2026-11-09-40-records/p40c-look.md). Controls starting with c40c_. One sabotage per assertion, each on
# a product line; the test named must print FAIL with it in place (the expected red line, written before the run).
# `Control` is provided by controls.py.
LOOK = "src/nirvana_look.rs"
MEDIA = "src/nirvana_media.rs"

T_CLAUDE = "c40c_claude_argv_is_the_measured_line_with_no_schema"
T_AGY = "c40c_agy_argv_reads_with_view_file_and_no_schema"
T_HEADER = "c40c_header_sizes_read_png_jpeg_gif_bmp_webp"
T_TOKENS = "c40c_token_estimate_matches_the_measured_formula"
T_DOWN = "c40c_a_small_still_is_read_as_it_is_and_a_large_one_is_copied_small"
T_HEIC = "c40c_heic_is_turned_into_a_jpeg_and_missing_ffmpeg_is_said"
T_REFUSE = "c40c_refusals_name_the_file_and_what_is_taken"
T_PDF = "c40c_pdf_pages_are_counted_or_unknown"
T_LONGPDF = "c40c_a_long_pdf_with_no_agy_and_no_pdftoppm_is_refused_with_the_fix"
T_STALE = "c40c_no_fresh_reading_fails_closed_and_starts_nothing"
T_ANSWER = "c40c_a_look_answers_records_and_releases_its_lease"
T_CACHE = "c40c_the_same_question_again_is_a_cache_hit_and_fresh_asks_again"
T_FAILED = "c40c_a_failed_launch_releases_the_lease"
T_TIMEOUT = "c40c_a_timeout_stops_the_look_and_releases_the_lease"
T_AGYANS = "c40c_agy_answer_is_read_from_its_result"
T_CLASS = "c40c_the_class_is_added_once_and_parses"
T_IGNORE = "c40c_copies_go_in_the_look_folder_with_a_gitignore"
T_UNC = "c40c_media_strip_verbatim_keeps_a_share_path"
T_STILL = "c40c_media_a_still_is_told_from_a_recording"
T_SPROBE = "c40c_media_still_probe_reports_size_pixels_and_tokens"
T_SHRINK = "c40c_media_shrink_still_copies_never_over_input"
T_DOCTOR = "c40c_media_doctor_names_tools_seats_and_clis"

CONTROLS = (
    # The launch: the measured claude line, and agy's view_file prompt.
    Control("c40c_claude_mcp_not_strict", LOOK,
            '        "--strict-mcp-config",\n', "", T_CLAUDE, "rust"),
    Control("c40c_agy_prompt_without_view_file", MEDIA,
            "with the view_file tool (AbsolutePath", "with the read_file tool (AbsolutePath", T_AGY, "rust"),
    # The file: the header, the cost, the downsize decision, a copy made once.
    Control("c40c_jpeg_dht_read_as_a_frame", LOOK,
            "0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {", "0xC0..=0xCF => {", T_HEADER, "rust"),
    Control("c40c_token_estimate_uncapped", LOOK,
            "div_ceil(TILE)).min(TOKEN_CAP)", "div_ceil(TILE))", T_TOKENS, "rust"),
    Control("c40c_small_still_copied", LOOK,
            "    (width.max(height) > edge).then_some(edge)", "    (width.max(height) > edge / 2).then_some(edge)",
            T_DOWN, "rust"),
    Control("c40c_copy_made_every_time", LOOK,
            "    let made = std::fs::metadata(&output).is_ok_and(|meta| meta.len() > 0) && out.is_none();",
            "    let made = out.is_some() && false;", T_DOWN, "rust"),
    Control("c40c_heic_copy_not_jpeg", LOOK,
            "    let ext = if matches!(kind, Kind::Jpeg | Kind::Heic) {", "    let ext = if matches!(kind, Kind::Jpeg) {",
            T_HEIC, "rust"),
    Control("c40c_heic_without_ffmpeg_unsaid", LOOK,
            "            (Kind::Heic, _) => format!(", "            (Kind::Pdf, _) => format!(", T_HEIC, "rust"),
    # The refusals.
    Control("c40c_text_file_taken", LOOK,
            '        "png" => Kind::Png,', '        "png" | "txt" => Kind::Png,', T_REFUSE, "rust"),
    Control("c40c_over_50_mb_taken", LOOK,
            "        len if len > MAX_BYTES => Some(format!(", "        len if len > MAX_BYTES * 2 => Some(format!(",
            T_REFUSE, "rust"),
    # PDF: an object stream makes the count unknown; a long PDF with no agy and no pdftoppm is refused with the fix.
    Control("c40c_objstm_counted", LOOK,
            '    if contains(bytes, b"/ObjStm") {', "    if false {", T_PDF, "rust"),
    Control("c40c_long_pdf_tries_claude_without_pdftoppm", LOOK,
            '                if !(env.on_path)("pdftoppm") {', "                if false {", T_LONGPDF, "rust"),
    # The seat: fail closed without a fresh reading; the lease ends on every path.
    Control("c40c_stale_seat_picked", LOOK,
            "        if !fresh(table, world, seat) {", "        if false {", T_STALE, "rust"),
    Control("c40c_lease_never_released", LOOK,
            "                if leases.release(&self.order, crate::nirvana_run::now()) {", "                if false {",
            T_FAILED, "rust"),
    Control("c40c_no_timeout", LOOK,
            "        if Instant::now() >= deadline {", "        if Instant::now() >= deadline + Duration::from_secs(60) {",
            T_TIMEOUT, "rust"),
    # The answer: read from the right place, recorded, an error or an empty answer is a failure.
    Control("c40c_cwd_not_the_copys_folder", LOOK,
            "    let cwd = read.parent().map_or_else(|| dir.clone(), Path::to_path_buf);",
            "    let cwd = read.parent().map_or_else(|| dir.clone(), |_| env.root.clone());", T_ANSWER, "rust"),
    Control("c40c_pass_not_recorded", LOOK,
            '    record(&table, &pick, &order, "pass", tokens, extra);', "    let _ = (&tokens, &extra);",
            T_ANSWER, "rust"),
    Control("c40c_claude_error_ignored", LOOK,
            '        let is_error = result.get("is_error").and_then(Value::as_bool) == Some(true);',
            "        let is_error = false;", T_FAILED, "rust"),
    Control("c40c_empty_answer_taken", LOOK,
            "    if text.is_empty() {", "    if false {", T_AGYANS, "rust"),
    # The cache: keyed by bytes, question and detail; a hit spends nothing and keeps off every seat's chain.
    Control("c40c_cache_never_read", LOOK,
            "    if !args.fresh {", "    if false {", T_CACHE, "rust"),
    Control("c40c_cache_ignores_bytes", LOOK,
            "            line.sha256 == sha\n", "            !sha.is_empty()\n", T_CACHE, "rust"),
    Control("c40c_cache_ignores_detail", LOOK,
            "                && line.detail == detail\n", "", T_CACHE, "rust"),
    Control("c40c_cache_hit_on_the_media_chain", LOOK,
            '        "chain": CACHE_CHAIN,\n', "", T_CACHE, "rust"),
    # The class and the look folder.
    Control("c40c_class_without_haiku", LOOK,
            '  { seat = "haiku", score = 84 },\n', "", T_CLASS, "rust"),
    Control("c40c_class_added_twice", LOOK,
            '    if text.contains(&format!("id = \\"{CLASS}\\"")) {', "    if false {", T_CLASS, "rust"),
    Control("c40c_look_folder_not_ignored", LOOK,
            'pub const IGNORE: &str = "*\\n";', 'pub const IGNORE: &str = "\\n";', T_IGNORE, "rust"),
    # Media: review B8, X3, the still probe and the doctor.
    Control("c40c_unc_relative_again", MEDIA,
            '        return PathBuf::from(format!(r"\\\\{rest}"));', "        return PathBuf::from(rest.to_string());",
            T_UNC, "rust"),
    Control("c40c_still_by_duration_only", MEDIA,
            '    name.ends_with("_pipe") || name == "image2" || !timed', "    !timed", T_STILL, "rust"),
    Control("c40c_still_probe_no_pixels", MEDIA,
            "        pixels: width.zip(height).map(|(w, h)| u64::from(w) * u64::from(h)),", "        pixels: None,",
            T_SPROBE, "rust"),
    Control("c40c_still_copy_beside_input", MEDIA,
            "    let dir = look::look_dir(root)?;",
            "    let dir = input.parent().unwrap_or(root).to_path_buf();", T_SHRINK, "rust"),
    Control("c40c_still_a_video_again", LOOK,
            '        "-frames:v",\n        "1",\n', '        "-r",\n        "10",\n', T_SHRINK, "rust"),
    Control("c40c_copy_over_input", LOOK,
            "    if crate::nirvana_media::paths_collide(input, &output) {", "    if false {", T_SHRINK, "rust"),
    Control("c40c_doctor_pdftoppm_always_found", MEDIA,
            '    lines.push(if on_path("pdftoppm") {', "    lines.push(if true {", T_DOCTOR, "rust"),
    Control("c40c_doctor_age_wrong", MEDIA,
            "            .map(|reading| world.now.saturating_sub(reading.read_at))",
            "            .map(|reading| reading.read_at)", T_DOCTOR, "rust"),
    Control("c40c_doctor_ignores_capability", MEDIA,
            "            .filter(|seat| seat.enabled && seat.capabilities.iter().any(|c| c == capability))",
            "            .filter(|seat| seat.enabled)", T_DOCTOR, "rust"),
)
