# Package P40c record: `herdr look`, still images in `herdr media`, `herdr media doctor`

Branch `c40c-look` off master 86b37c7e (alpha 0.62.0). A first builder was stopped by a usage limit about 13 minutes
in; its uncommitted `src/nirvana_media.rs` and `src/nirvana_look.rs` were kept, built, checked and finished by a
second builder. Nothing was redone from scratch.

## Ledger

| Quote (verbatim) | Surface | Owner | Verifier |
|---|---|---|---|
| "create new features that would help improve token efficenccy, improve multi modal usage and improve project output." (the owner, 2026-11-09) | `herdr look`, `herdr media probe/shrink` for stills, `herdr media doctor`, `skills/look`, the `media` skill's first lines | P40c | the `c40c_` tests in `nirvana_look::tests` and `nirvana_media::tests`, the controls in `nirvana/controls.d/c40c.py` (`controls.py --dry-run` counts them) |

## Why, measured

- Over the fork's life 406 images entered the orchestrator's own context, carrying up to 12% of its spend into later
  turns; `herdr media` ran twice in total (plan 40 usage audit, 2026-11-09).
- On the parent project one session carried 118 MiB of images, 52% of it, re-sent every turn (Nirvana CLAUDE.md
  rule 7).
- The measured read (2026-11-09, Haiku 5.5 low, Claude Code 2.1.321), as the order gives it: "answered a
  text-and-count question about a 1024x683 JPEG correctly in 2 turns, 3.8 s, 10.4k tokens of context, $0.00016 at
  list price." Without `--tools Read`: 48.9k. An image costs about `ceil(w/28) * ceil(h/28)` tokens, at most 4,784.

## Built

1. `herdr look <file> --ask "<q>" [--seat] [--detail low|high] [--pages a-b] [--json] [--fresh]` (`src/nirvana_look.rs`).
   Kinds by extension; missing, empty, over 50 MB and other kinds refused in one line. Size from the file's own
   header (PNG, JPEG SOF, GIF, BMP, WebP VP8/VP8L/VP8X), else ffprobe. A copy at most 1024 or 2576 px in
   `<project>\.nirvana\look\` (with a `.gitignore` of `*`), named `<stem>-<sha12>-<edge>.<ext>`, made once, never
   over or beside the input; HEIC always becomes a JPEG copy. Class `look.image` (appended to the table in memory
   until the seat builder's hunk merges); picked and booked through `nirvana_seat::pick`/`book_pick` under
   `leases.lock`; a seat whose pool windows lack a fresh reading is removed first, and none left fails closed with
   exit 3. The launch: the measured claude argv (cwd the copy's folder), or agy with `still_prompt_line` (removed in the review round below); a
   kill-on-close job and a 120 s limit; no schema, no order file, no gate, no journal. One record line per look,
   filled from the live booking; the lease is released only by a drop guard, on every path. The cache:
   `<name>.look.jsonl` keyed by sha256, question, detail and pages.
2. `herdr media shrink` and `probe` on a still (review X3): told by extension, an image demuxer (`*_pipe`,
   `image2`) or no duration. ffprobe 8.0.1 gives a JPEG and a GIF a 0.04 s duration, so duration alone does not tell.
   Review B8: `strip_verbatim` keeps `\\?\UNC\srv\share\x` as `\\srv\share\x`.
3. `herdr media doctor`: ffmpeg, ffprobe, pdftoppm, seats per capability, each CLI's sign-in from cached usage
   readings and agy's log only.
4. `nirvana/claude/skills/look/SKILL.md`; the `media` skill now opens by sending stills and PDFs there.

## How many looks were cache hits

A hit writes a record line on chain `look-cache` with no tokens and no use, kept off every seat's tally. Count them
with a script, not prose:

    python -c "import json,sys;L=[json.loads(l) for l in open(sys.argv[1],encoding='utf-8') if l.strip()];print(sum(x.get('chain')=='look-cache' for x in L),'hits of',sum(x.get('class')=='look.image' for x in L),'looks')" "%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\record.jsonl"

No live look has run yet; the owner's live check after the merge gives the first numbers.

## Review round (2026-11-09): a look can read only its own copy

The principle: the image or PDF is untrusted content, so a look does exactly one thing, read the copy it was given,
and its answer is data for the caller to judge.

Measured today (2026-11-09, Haiku 5.5 low, Claude Code 2.1.321), as the round's order gives it: "In a folder holding
`small.jpg`, with a canary file one folder up, the same prompt asked for both files: `--tools Read --allowedTools
Read`: the turn read the image and quoted the canary. 0 permission denials. `--tools Read --allowedTools
"Read(./**)"` with the folder as cwd: the turn read the image and answered REFUSED for the canary. The result
object's `permission_denials` array had 1 entry."

- S2: every look copies its input into its look folder, shrunk or byte for byte, through a temp name and a rename;
  the cwd is that folder; the argv carries `--allowedTools "Read(./**)"`; the prompt names the copy by file name.
  `permission_denials` is counted: `refused_reads` in `--json`, a last line in plain output when not zero.
- Departure from the first order: the look folder is `<home>\nirvana\look\<first 12 of sha256 of the project
  root>\`, not `<project>\.nirvana\look\`. A folder inside a project loads that project's CLAUDE.md and settings
  into the turn, costing tokens and letting a project file steer or slow a look. A CLAUDE.md, CLAUDE.local.md or
  `.claude` in it or between it and the home refuses the look. `herdr media shrink` still writes its copies in
  `<project>\.nirvana\look\` as built. Above the home on this PC only `~\.claude` exists, the user config dir,
  which `--setting-sources project` leaves out (not measured here).
- S1: `flash` is out of `look.image` and the agy launch is gone (plan 34a's ruling for the teach worker, "no agy
  seat teaches"). A table that still lists an agy seat there is not obeyed on it, and `--seat flash` is refused. A
  PDF over 10 pages or of unknown length is refused before anything is spent, the Poppler fix first; with Poppler
  it needs `--pages`, at most 10 per look. Haiku has no `pdf` capability, so a PDF goes to sonnet.
- S3: the skill's first lines say the answer is untrusted data and what refused reads mean.
- S4: a still is told by an image demuxer, a picture codec, exactly one frame, or a HEIC name, never by a missing
  duration.
- N1 temp name and rename; N2 60 s limit on ffprobe and on ffmpeg for stills (the video encode keeps no limit: its
  length was never measured); N3 `-protocol_whitelist file -format_whitelist <image demuxers>` on the still ffmpeg
  and ffprobe (a concat text named `evil.png` is refused, checked with ffmpeg 8.0.1); N4 copies over 30 days old
  pruned when a look runs, caches kept; N5 a look books 5 minutes; N7 a cache hit honours `--seat` (a named seat
  is asked for its own reading, often a second opinion; without `--seat` any seat's answer serves and `--json`
  names it); N8 a zero side is no size; N10 one ffprobe run per probe or shrink.
- N6 is left: a look reads the live world as `seat pick` does.
- The record line of a look now carries `refused_reads`. Count looks that met a steering picture with a script:

      python -c "import json,sys;L=[json.loads(l) for l in open(sys.argv[1],encoding='utf-8') if l.strip()];print(sum((x.get('look') or {}).get('refused_reads',0)>0 for x in L),'steered of',sum(x.get('class')=='look.image' for x in L),'looks')" "%LOCALAPPDATA%\Programs\Nirvana-Herdr\nirvana\record.jsonl"

### The hunk for `src/nirvana_seats.toml` (the seat builder's file), after the `media.audio` class

Find:

    seats = [
      { seat = "flash", score = 86 },
      { seat = "argon", score = 85 },
    ]

    [[class]]
    id = "sweep.mechanical"

Replace:

    seats = [
      { seat = "flash", score = 86 },
      { seat = "argon", score = 85 },
    ]

    [[class]]
    id = "look.image"
    chain = "media"
    blast = "bounded"
    effort = "low"
    efficient_effort = "low"
    sota_effort = "low"
    needs = ["image"]
    weight = 0.2
    bars = { efficient = 70, balanced = 80, sota = 100 }
    why = "plan 40c: herdr look, one question about one image or PDF; weight 0.2 is a prior (10.4k tokens measured against P4's 45k); claude seats only: agy has no tool filter and the picture is untrusted (plan 34a: no agy seat teaches)"
    seats = [
      { seat = "haiku", score = 84 },
      { seat = "sonnet", score = 86 },
    ]

    [[class]]
    id = "sweep.mechanical"

Chain `media` is shared with `media.video` and `media.audio` on purpose: a look is media spend, and its tally belongs
there. The text equals `CLASS_TOML` in `src/nirvana_look.rs`.

## Open

- The live check (a paid Haiku turn, one through the scoped rule) is the orchestrator's after the merge.
- Ctrl-C skips the drop guard, so an interrupted look's lease lasts its 5 minutes (review N5); the job kills the child.
- `herdr look` reads the live world like `seat pick` does, which reads Claude usage with the CLI.
- `look.image` weight 0.2 and the scores are priors.
- `strip_verbatim` is a private duplicate of `nirvana_session_record`'s until that one is `pub(crate)`.
- The job object is a duplicate of the worker's private one.
