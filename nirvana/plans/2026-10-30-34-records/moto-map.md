Everything below comes from reading files only. I did not touch D:\MotoProject or open images. In 1 and 5 I name `.mcp.json` and settings files only by name, with no contents.

## 1. The reel editor

**What it is.** The editor is a FastAPI + uvicorn app (Python), with plain JS in `editor/static/` (timeline, inspector, presets, sweep, moves, stab and others). Its backend modules are in `editor/server/`: `main.py`, `frames.py`, `segments.py`, `renders.py`, `presets.py`, `moves.py`, `templates.py`, `grade.py`, `title.py`, `story.py`, `stab.py`, `postview.py`, `finds.py`, `cuts.py`, `speed.py`, `reel.py`, `sources.py`.

**How it starts.** `python editor/server/main.py --port 8770` (`editor/CONTRACT.md` §5). The `--hike` flag selects the hike profile, and `open_hike_editor.bat` runs it that way. Server modules are resolved once at mount, so a new module needs a server restart (CLAUDE.md "Working rules"). Static JS and CSS are served from disk, so a reload is enough for those.

**Control surfaces today**
- **HTTP/JSON API** (`editor/CONTRACT.md` §5, lines ~119-192, 535-537). The routes I read:
  - `GET /api/project` and `GET /api/manifest[?path=]`
  - `PATCH /api/segment/{seq}`, taking yaw, pitch, roll, h_fov, beats, src_in, src_out and move
  - `GET /api/frame`, `GET /api/clip` and `GET /media/{name}`
  - `GET /api/move/schema`, `/api/move/curve` and `/api/moves`
  - `POST /api/segment/{seq}/render` and `POST /api/reel/render`, with `GET /api/reel/status?job=`
  - `GET /api/preset`, `PUT /api/preset/{name}` and `POST /api/preset/{name}/apply`
  - `GET /api/template`, `/api/template/{name}` and `POST /api/template/{name}/plan`
  - There are about 51 route decorators across `editor/server/*.py`. The hike, story, grade, title and stab routes are not in the CONTRACT lines I read.
- **File formats.** Files on disk are authoritative and the UI holds no state (CONTRACT §4).
  - `plan/*.json` is the manifest for a reel (moto) or a post (hike carousel, `plan/post_*.json`).
  - `angles/*.json` holds presets, and `marks/<clip>.json` holds marks plus `forward_yaw`.
  - `templates/*.json` holds styles, and `profiles/hike.json` holds the activity profile.
  - Writes go through `review_marks.write_atomic`, LF newlines. `marks` use the custom `dump_marks` serialiser, never `json.dump`.
- **Schemas** in `schemas/`: `angles`, `marks`, `story` and `template`. There is no manifest or post schema. The post slide key set is enforced in code (`scripts/post.py` SLIDE_KEYS, which rejects unknown keys except `*_note`).
- **CLI scripts.**
  - Reels: `scripts/plan.py` (`--template`, `--list-templates`, `--flat-scores`, `--t0`, `--beats`, `--out`), `render.py` (`--draft`), `index.py`, `proxy.py`, `motion.py --rate`, `analyze_audio.py`, `intro.py`.
  - Hike: `hike_scan.py`, `compose.py run|verify-clips|walk`, `post.py`, `reveal.py build|check`, `stabilise.py`, `story_compile.py`, `story_build.py`, `story_photos.py`, `hike_ingest.py`.
- **MCP and browser.** There is no `.mcp.json` and no `.claude/settings*` in D:\MotoProject. `.claude/` holds only `skills/`. No editor MCP server exists. I saw no browser-automation harness for the editor, but I did not search for one exhaustively.

**Most reliable surface.** Manifest JSON plus the CLIs is the surest, because the files are authoritative and the scripts validate them. The HTTP API is second and is the best path for live preview, but CLAUDE.md warns that test-driven PATCH or GET calls against the owner's server on :8770 wrote into `plan/`. Use a throwaway port, for example 8771. A `GET /api/manifest?path=` also switches which file the server edits.

**Missing**
- There is no MCP wrapper or single typed tool layer over these.
- There is no manifest schema.
- The editor cannot PATCH `stabilise` or the pose of a drift slide (needs a `postview` change, per memory `hike-reel-process.md`).
- Re-planning with `overwrite:true` silently discards hand-set framing, and there is no git in that project to recover it.
- Several measurements (motion, seam share, mean Lab) are scattered across probe scripts rather than one gate.

## 2. Styles

**There is a real notion of a style, and it is a template.** `templates/<name>.json`, validated by `schemas/template.schema.json`, is "EDITORIAL WEIGHTING, not a saved edit" (CONTRACT ~line 525). Templates never name mark ids or absolute yaw. Template blocks:
- `name`, `title`, `note` and `reference`
- `pacing`: default/min/max beats, shape (flat, accelerate, decelerate, arc), `shape_strength`, `max_same_run`, `skew`
- `select`: order, `min_motion`, `tag_weights`, `lead`, `tail`
- `shots`: preset weights, `tag_presets`, `max_uses`, `direction_weight`
- `transitions`: `on`, whip, punch, drift, plus motion_blur, dissolve and `cut_offset_frames`, all inert when absent
- a grade look

**Existing styles by name.** `dusk_drift` (v1 to v6, v6 is the current best), `golden_hour`, `hard_cut`, `machine`. Hike post "styles" are not templates. They are `plan/post_alum_cave_v*.json` (v4 to v12), built by hand or via the story compiler.

**Other style carriers**
- **Presets.** `angles/*.json` (moto: forward, forward_tight, hero_rear, machine_insert, rear, rider, rig_forward, right, sweep_right, treeline) and `angles_hike/*.json` (canopy_up, look_back, overhang_in, ridge_view, side_detail, trail_ahead, trail_walk).
- **Grades.** Named chains of ffmpeg filters (curves, vibrance, selectivecolor, colorbalance) per look, e.g. `overcast_green`, `open_ridge`, `rock_overhang`. These live in `profiles/hike.json` and the templates, and `editor/server/grade.py` serves them.
- **Moves.** Whip, punch and drift, with easing in `render.MOVE_EASE`, served by `/api/move/schema`.
- **Titles and openers.** CLAUDE.md "Openers and closers" and "Titles".
- **Music sync.** `audio/beats.json` (180 BPM, bar = 4 beats), and every cut lands on a beat.
- **No LUT files.** Grading is ffmpeg filter chains. Transitions beyond whip, punch, drift, dissolve and reveal do not exist.

## 3. Knowledge

Technique knowledge is recorded in prose, and almost none of it is machine-readable.
- **`CLAUDE.md` (~1,100+ lines)** is the main knowledge base. It covers:
  - cut policy, with DEAD END notes;
  - Stillness vs Whiplash (two opposite complaints);
  - the join toolkit, openers and closers, templates, colour, titles;
  - the yaw model, telemetry, a pipeline-stage list, an "Adding a new ride" order, and "Adding a PRESET";
  - "Working rules" and the hike-post loop at line 1012.

  Claude Code loads it automatically.
- **Auto-memory** at `~\.claude\projects\D--MotoProject\memory`: `hike-reel-process.md` holds the owner's taste and v8-v12 lessons, plus `video-analysis-argon.md`. It loads automatically for sessions in D:\MotoProject.
- **`.claude/skills/hike-compose/SKILL.md`** is the only skill. It teaches hike funnel to judge to roles and quotas, with scrub-vs-vista and canopy-vs-frame rejection rules. It loads on a description match.
- **`docs/plans/2026-08-04-motion-skills-extraction.md`** is the model case of "taught a new technique". It extracted `iart-ai/motion-skills`, treated the files as data, and recorded a verdict per item against measurements:
  - TAKE: engine-bed splice at cuts, a safe-area check, and hold-after-climax.
  - REJECT: velocity matching and beat rounding.
  - The rest were already built or not applicable.
- **`notes/`:**
  - `editor_feature_spec.md` (tier A instrument, B vocabulary, C planner, D provenance) and `reel_gap_spec.md`
  - `findings.md`, `projection_parity.md`, `safe_area_measured.md`, `dissolve_preview.md`
  - `gap_audit/` (about 30 measurement scripts and JSON, `statistic_finding.md`, `speed_proto.md`)
  - `spec_research/`, `research/` (Gemini reveal format, YouTube/Reddit motion research, `improve/` v-reports) and `story/`
  - Data files: `mark_motion.json`, `motion_scan.json`, `cut_review.json`, `hike_finds_*` and `hike_scan_*`
- **`marks/*.json`** holds per-clip timestamped windows with tags. **`index/`** is ffprobe output only.
- **Read automatically:** CLAUDE.md, the memory directory, and the `hike-compose` skill by description. Everything in `notes/` and `docs/plans/` is read only if an order names it. There is no searchable index and no technique-by-name lookup. Taught techniques have no standard landing place beyond a plan doc or a CLAUDE.md paragraph.

## 4. Nirvana usage (`.nirvana/`)

There are 28 orders, 14 task files, 4 run dirs (2026-10-22 to 10-23), `retro/gaps.jsonl` (35 rows: 34 `gotcha`, 1 `override`) and one proposal. `seats.toml` has `mode = "sota"`.

**Reel and post orders, one line each**
- `v6-build` to `v12-build`: successive builds of the Alum Cave hike post.
  - `v6`: re-pose off the stitch seam, with measurement.
  - `v10`: build, review, scout.
  - `v12`: fewer people, more rock, creek and vista.
  - `v12-motion-build`, `-research` and `-verify`: keep the camera on the motif and steadier. It owns `post.py`, `stabilise.py`, `stab.py` and `postview.py`.
- `v9-scout` and `v12-scout`: agy (Gemini) finds moments and renders a still per candidate.
- `reveal-build`, `reveal-build-s1` and `reveal-gemini`: the "reveal" slide, one continuous v360 move from the group to the vista (`scripts/reveal.py`).
- `reel-motion-gemini`: research (YouTube/Reddit) on why the post "feels static".
- `imp-gemini-video`, `-transition` and `-aspect`: Gemini defect logs on draft renders.
- `hike-video-read` and `hike-photo-read`: build `notes/story/video_timeline.json` and `photo_catalog.json`.
- `story-compiler` (plus `-fix1` and `-rev1`): `scripts/story_compile.py`, `story_build.py` and `schemas/story.schema.json`.
- `story-photo-slide` and `editor-story-panel`.

I saw no moto-reel build orders here. The moto reels (dusk_drift v1 to v6) predate Nirvana in this folder and were done interactively.

**Recurring failures and rediscoveries.** The retro mostly holds worker "gotchas" about the data (clip 006 elevation nulls, GPS fluctuation under overhangs, ProRAW .dng/.HEIC pairs, privacy rule: never write lat/lon). From the orders, memory and CLAUDE.md:
- **Stitch seam.** The stitch seam shows up near yaw ±90 above about 15 degrees latitude, and the wearer's hat or the rig shows in vistas.
- **Probing frame 0 only.** Workers probed at frame 0 instead of through the stabiliser's per-frame poses.
- **Agy's numbers were wrong.** A drift peak claimed 1.23 %/frame measured 2.23 through `sphere.drift_peak_pct` (v10).
- **Missing post.py capability.** `post.py` has no motion blur or dissolve (v11), and `story_compile` hit `post.py` SLIDE_KEYS rejecting `story_slot`, `score` and `photo` (workaround: `*_note` keys).
- **Moves drift off the motif.** Templated drifts and locked sphere on walking footage carry the centre 13-21% off (v12).
- **Too many friends in frame.** The owner said "limit the friends ones" (v11).
- **Render timeouts.** Full renders exceed the 10-minute tool timeout, so re-run to resume from cache.
- **Shell and encoding.** A bash heredoc write failed silently, and cp1252 truncated a report.
- **Test suites against :8770.** The suites write into `plan/`.
- **Infrastructure.** `herdr seat run` launched a bare upstream `herdr` (`.nirvana/proposals/2026-10-26-seat-run-bare-herdr.md`), and the orchestrator overrode a model rule at the owner's request.

## 5. `.claude/` and toolchain

- **`.claude/`** has only `skills/hike-compose`. There are no hooks, agents or settings files in D:\MotoProject. The Nirvana-side `check-edit` PostToolUse hook mentioned in the proposal is not in this folder.
- **Toolchain.**
  - ffmpeg is the renderer, with `v360` reprojection and `-hwaccel cuda` decode.
  - Python includes numpy and SegFormer (person mask), and madmom for beats.
  - Gemini video runs through agy via Nirvana, and exiftool is used for photos.
  - Insta360 Studio produces the stabilised masters, and the insv accelerometer feeds hike stop detection.
  - I found no Remotion or Blender in the files I read, though I did not grep for them.
- **Disk limits.** D: is nearly full (about 6.6 GB). Hike intermediates go to `E:/hike_work`.
- **No git in D:\MotoProject**, so manifest edits cannot be undone.

## What an agent profile would need

1. **A typed tool layer** over the editor API and CLIs: manifest read, patch and validate, plan-with-template, draft render, status, frame probe. Safe by default: a throwaway port, never :8770, no `overwrite:true`, and a snapshot of any manifest before it is changed.
2. **Style registry exposed as data.** It would list templates, presets, grades, moves and profiles with their schema notes, and let an agent create a new template or grade through the schema.
3. **A manifest and post schema**, and the missing `stabilise`/drift patching, so a style change is a validated edit.
4. **A knowledge base that is searchable.** Technique cards (what, when, measured effect, DEAD END, source and date), split out of the CLAUDE.md prose and `notes/`, plus an index. Write the CLAUDE.md DEAD END and "do not re-run" entries in as cards.
5. **A teach-new-technique workflow** modelled on the motion-skills extraction. Read as data, check against measurements, record a TAKE or REJECT verdict, and write the card.
6. **A review gate.** It would hold the pre-render probes (seam share per frame through the stabiliser, mean Lab join delta, `min_motion`, ramp and settle metrics, adjacent-pair equality) and the post-draft clipmetrics, run before any full render.
7. **Owner taste and constraints**: the beat-grid and 29.97 invariants, the privacy rule (no lat/lon), and the friends limit.
8. **Execution conventions.** Long renders must be resumable (cached segments), and draft first. D: space is tight, so intermediates go to E:.

**Unsure about**
- The routes in `editor/server/*.py` beyond the CONTRACT lines I read (hike, story and stab).
- Whether anything MCP or browser-based drives the editor outside this folder.
- Whether the Nirvana `herdr`/`agy` seat tooling counts as the agent's "tools" for this profile. It lives outside D:\MotoProject.