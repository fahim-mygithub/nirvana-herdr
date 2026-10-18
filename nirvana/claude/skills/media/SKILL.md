---
name: media
description: "Route video, audio, and >400k-token files to Gemini seats (argon or flash) on Antigravity (agy). Use whenever a task involves screen recordings, animations, UI reproduction videos, voice-overs, audio recordings, or huge files that would burn the Claude Max pool."
---

# Media and Huge-Read Seating (`media.video`, `media.audio`, `read.huge`)

As the Nirvana Claude orchestrator, **never read a video, an audio file, or a huge (>400k token) file into your own context**. Claude cannot ingest video or audio at all, and reading huge inputs on Claude burns the `>150k` context tier that consumes 74% of the Max weekly pool. Always delegate media and huge reads to an Antigravity (`agy`) worker seated on `argon` or `flash`.

`herdr` below is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`. A plain `herdr` on this machine's
PATH is upstream herdr, which has no `media` or `seat` command (measured by O4b, 2026-10-18).

## 1. Probe and shrink before seating (`< 100 MB`)

Antigravity's `view_file` tool supports `image`, `pdf`, `video`, and `audio` up to a **hard 100 MB limit** (probe P2: a 186 MB video was refused at the tool layer before reaching Gemini).

1. **Probe the file first:**
   ```bash
   herdr media probe <path/to/file> --json
   ```
   This runs `ffprobe` (with a list `argv`, never a shell) and reports `duration_secs`, `size_bytes`, `container`, `video_codec`, `resolution`, `audio_present`, and `over_limit` (`size_bytes > 100,000,000`).

2. **Shrink when `over_limit` is `true`:**
   ```bash
   herdr media shrink <path/to/file> --max-mb 95 --json
   ```
   - Re-encodes with `ffmpeg` to `<name>.shrunk.mp4` next to the input (unless `--out <path>` is passed), never overwriting the input file.
   - Caps width at `1280` px (`scale='min(1280,iw)':-2`) and frame rate at `10` fps for screen recordings.
   - Preserves voice-overs as mono low-bitrate AAC (`-ac 1 -b:a 48k`), because spoken narration carries intent.
   - Computes target video bitrate from duration with a safety margin, and retries once at a lower bitrate if pass 1 still exceeds `--max-mb`.
   - Pass the resulting `.shrunk.mp4` path to the worker.

## 2. Pick the right seat (`herdr seat pick`)

Choose the class that matches the work:

- **Screen recordings, UI bugs, animations, and visual verification with source code (`media.video`):**
  ```bash
  herdr seat pick --class media.video --json
  ```
  Seats `argon` (`medium`) first, falling back to `flash` (`low`). For visual UI work (`build.frontend` or `judge.visual`) that involves motion or a screen recording, pass `--needs video` so `herdr seat pick` routes to `argon`.
  - Use **`argon`** when the worker should watch the video *and* cross-check the repository's source files to diagnose a bug or verify a fix. A text transcript alone throws away spatial and temporal visual evidence.
  - Use **`flash`** (`low`) when you only need a fast timestamped transcript and event extraction.

- **Audio recordings and voice notes (`media.audio`):**
  ```bash
  herdr seat pick --class media.audio --json
  ```
  Seats `flash` (`low`) first, then `argon` (`low`).

- **Huge text/log/bundle reads over 400k tokens (`read.huge`):**
  ```bash
  herdr seat pick --class read.huge --json
  ```
  Seats `flash` (`medium`) first, then `argon` (`medium`) using their `context_1m` capability.
  - **One exception to the "never resume" rule:** for a read-only follow-up question on a `read.huge` session, resume the same `agy` conversation with `agy --conversation <id>`. Starting a fresh session re-reads the entire >400k-token input, whereas cached input on Gemini costs 95% less.

## 3. Build the worker prompt (`herdr media prompt`)

Probe P2 proved that `flash` and `argon` read MP4 and WebM files reliably **only when the prompt names the file's full absolute path and explicitly instructs the worker to use the `view_file` tool** (omitting `StartLine` and `EndLine`). With a bare filename, `flash` tried to run `dir`, plan mode refused it, and the worker exited with an empty answer and `status: SUCCESS`.

Generate the single-line worker prompt with:

```bash
herdr media prompt <path/to/file-or-shrunk.mp4> --ask "<your question>" --seat argon
```

This line instructs the `agy` worker to:
1. Open the full absolute path with `view_file` (without `StartLine` or `EndLine`) and start no subagents or background commands.
2. Return:
   - **Timestamps (`MM:SS`)** for each key event or transition.
   - **Transcript and screen timeline**: what happens on screen and any spoken audio verbatim.
   - **Visual-only evidence**: what the video shows spatially/temporally (layout shifts, flicker, cursor/focus state, timing) that a text transcript alone would lose.
   - **Findings** answering the specific question (and, on `argon`, the relevant source file/line diagnosis).

## 4. Gate rule: an empty answer is always a failure

Never trust `status: SUCCESS` or exit code `0` alone from an `agy` media worker. If the worker's response is empty, lacks timestamps/visual observations, or did not call `view_file` on the absolute path, treat the delivery as **failed** and re-run or re-seat with the full-path `herdr media prompt` line.
