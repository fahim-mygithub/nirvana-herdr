# Package P40c record: `herdr look`, still images in `herdr media`, `herdr media doctor`

Branch `c40c-look` off master 86b37c7e (alpha 0.62.0). A first builder was stopped by a usage limit about 13 minutes
in; its uncommitted `src/nirvana_media.rs` and `src/nirvana_look.rs` were kept, built, checked and finished by a
second builder. Nothing was redone from scratch.

## Ledger

| Quote (verbatim) | Surface | Owner | Verifier |
|---|---|---|---|
| "create new features that would help improve token efficenccy, improve multi modal usage and improve project output." (the owner, 2026-11-09) | `herdr look`, `herdr media probe/shrink` for stills, `herdr media doctor`, `skills/look`, the `media` skill's first lines | P40c | 29 tests in `nirvana_look::tests` and `nirvana_media::tests`, 35 controls in `nirvana/controls.d/c40c.py` |

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
   exit 3. The launch: the measured claude argv (cwd the copy's folder), or agy with `still_prompt_line`; a
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

## Open

- The live check (a paid Haiku turn and an agy turn) is the orchestrator's after the merge.
- `herdr look` reads the live world like `seat pick` does, which reads Claude usage with the CLI.
- `look.image` weight 0.2 and the scores are priors.
- `strip_verbatim` is a private duplicate of `nirvana_session_record`'s until that one is `pub(crate)`.
- The job object is a duplicate of the worker's private one.
