# Feature 13: the smart Preload, a preset per folder chosen when "Open Nirvana" opens it

Status: **DESIGNED 2026-10-18, awaiting the owner's go.** Built on Preload (features 5 and 7, `src/nirvana_preload.rs`),
Jev (features 9, 10 and plan 12 O8b) and the launch (plan 12 O4a/O4b).

## Ledger

| # | Verbatim quote | Surface | Owner | Verifier |
|---|---|---|---|---|
| L1 | "One feature I would like to create is a smart tool Preloader. Either use a Haiku Model or create a logical prescript and Jev utilization to determine what the memory and skills should be avaiable." | a classifier: free rules first, Jev when the rules cannot tell; no Haiku (owner ruling R1 below) | orchestrator | unit tests on fixture folders; Jev answers through a fake client; at most 5 live Jev calls, logged |
| L2 | "The idea being when you right click open nirvana here it enables or disables tools and mcps and skills so we save context per turn." | "Open Nirvana" computes the folder's preset before Nirvana Claude starts and launches it with the preset's switches | orchestrator | `/context` before and after on scratch fixture folders (free); the per-turn total written down before the run |
| L3 | "I have alot of stuff like bambu skills that are not really needed for every project." | a catalog that tags each skill, MCP server and plugin with domains; an item with domain tags none of which the folder wants is off | orchestrator | catalog tests; the owner's real catalog tagged and printed by `herdr preload catalog` |
| L4 | "It can look at the directory and deside oh this is a 3d print project we should allow bambu mcp. Or oh this is a blank directory we should have a different suite of tools." | a folder fingerprint (file types, marker files, emptiness) mapped to a preset: `3d-print`, `blank` and the others in §3 | orchestrator | fingerprint tests on fixtures: a folder of `.3mf`/`.stl` reads `3d-print`, an empty one `blank` |
| L5 | "We should also ensure the memory and claude md is designed for this or resumed from last session." | three parts the owner picked (R3): resume Nirvana Claude's last session in the folder; offer to trim an oversized MEMORY.md; auto memory on or off per preset | orchestrator | resume test on a recorded session file; trim through a fake Jev with the backup restored; `/context` Memory files before and after |

Image #21, as the owner pasted it: the Nirvana Herdr window at alpha 0.21.0 on `3D-Printer-Flow-xtool`, the Preload
panel open: "31.3k of 1m preloaded · Claude Code itself 2.5k". Under "You (~/.claude) 18.6k" are MEMORY.md 7.8k, 22 user
skills from 47 to 302 tokens each (bambu-h2c-cosplay-print, batch-photo-processing, the scholar-* family,
landscape-color-correction and others) and bladebro "MCP, 5 tools 6.5k". Under the plugins, github "MCP, 46 tools
19.6k" and supabase 563. kotlin-lsp and rust-analyzer-lsp are already off by hand.

### Owner rulings, 2026-10-18

- **R1, the classifier:** "Rules, then Jev". Free file-type rules first; Jev only when the rules are unsure and to tag
  new skills; cached per folder.
- **R2, applying:** "Apply, show undo". The preset applies before Claude starts; a toast and the Preload header say
  what went off, with one key to undo. Manual toggles always win.
- **R3, memory:** "Resume last session, Trim oversized memory, Memory per preset". The starter CLAUDE.md was not
  picked, so it is not built.

## 1. What a turn actually carries (measured 2026-10-18, Claude Code 2.1.302, scratch folder)

`claude -p /context --no-session-persistence --verbose --output-format json`, from Python with a list argv, cost 0:

| Category | Tokens | Kind |
|---|---|---|
| System prompt | 2,562 | used |
| System tools | 5,541 | used |
| MCP tools | 618 | used |
| MCP server instructions | 2,834 | used |
| Custom agents | 423 | used |
| Skills (73) | 9,947 | used |
| Messages (hook text) | 1,620 | used |
| **Total** | **23,545** | |
| MCP tools (deferred) | 129,919 | deferred, loaded only when searched |
| System tools (deferred) | 14,401 | deferred |

**What this means for the design:**
- **MCP tool schemas are already deferred.** github's 19.6k row in the panel is a deferred figure, so it is not paid
  each turn. An MCP server's per-turn cost is its instructions and its share of the tool-name list.
- **Skills are the main lever.** They are 9.9k of the 15.4k a preset can move.
- **Memory is the other lever.** In the owner's 3D folder, MEMORY.md alone is 7.8k.
- **The panel misleads today.** Its rows add deferred tokens into the provider sums. §5 fixes it.

The 2.5k "Claude Code itself" figure in the header is the system prompt alone; the tools are another 5.5k.

The first try at this reading passed `/context` through Git Bash, which rewrote it to `C:/Program Files/Git/context`
and sent it to the model as a prompt: one paid turn, $0.20. Every probe in this plan runs from Python with a list argv.

## 2. Where the switches go: a generated `--settings` file, not the folder

Nirvana Claude already starts with `--settings <home>/nirvana/claude/settings.json` (the statusline). The preset is
merged into a generated file, `<home>/nirvana/preload/<folder_hash>.settings.json`, made of that file plus the preset's
switches. The launch passes it in place of the plugin's file.

**Why not the folder's `.claude/settings.local.json`:**
- The preset touches nothing in the owner's folder. A plain `claude` started there keeps everything.
- Undo deletes nothing in the folder; it regenerates the file without the preset.
- The switches are the same ones Preload proved for both places (plan 05 §"What Claude Code exposes": "from the
  folder's `.claude/settings.local.json` and from `--settings`").

**Manual toggles win** (R2). Command-line settings outrank local ones in Claude Code. So the generated file leaves out
any item the owner has set by hand:
- **An item the owner switched off** in the panel stays in `settings.local.json`, as today.
- **An item the preset turned off that the owner switches on** is recorded as `pinned_on` in
  `<home>/nirvana/preload/<folder_hash>.json`, and the generated file leaves it out.

**Workers.** A Claude worker that `herdr seat run` opens in a pane for this folder gets the same file. The plugin
agents spawned inside Nirvana Claude inherit its tools already.

**Build check, written down before the run:** a skill set off through the generated file is gone from `/context`.
Running the same file with that skill on in `settings.local.json` shows the generated file wins. Leaving the skill out
of the generated file shows the local file then wins.

## 3. The classifier

### 3.1 The fingerprint (free, no model)

A bounded walk of the folder:
- It skips `.git`, `node_modules`, `target`, `.venv` and the Jev excluded dirs (`nirvana_jev::excluded_dir`).
- It stops at 5,000 files or 1 s, whichever comes first.
- It counts extensions and notes marker files: `Cargo.toml`, `package.json`, `pyproject.toml`, `*.3mf`, `*.gcode`,
  `.git`.
- It reads the first 2 KB of `README*`.
- A folder with nothing but `.claude` and `.git` in it is `blank`.

The fingerprint hash is the sorted extension counts, bucketed by powers of two, plus the marker names. The preset is
re-chosen only when the hash changes.

### 3.2 Presets and rules: `<home>/nirvana/preload.toml`

The default file lives in `src/nirvana_preload.toml` and is written on first read. After that it is the owner's to
edit, like `seats.toml` and `JEV.md`.

```toml
# Items tagged "core" are never switched off. Untagged items stay on: a preset only removes what it knows.
core = ["nirvana", "bladebro", "superpowers", "context7"]

[preset.3d-print]
when = { ext = ["3mf", "stl", "gcode", "step", "scad", "f3d"], share = 0.2 }
keep = ["3d-print", "cad", "code"]

[preset.photo]
when = { ext = ["cr3", "arw", "nef", "dng", "xmp", "raf"], share = 0.2 }
keep = ["photo"]

[preset.rust]
when = { marker = ["Cargo.toml"] }
keep = ["code", "rust"]

[preset.web]
when = { marker = ["package.json"] }
keep = ["code", "web", "db"]

[preset.python]
when = { marker = ["pyproject.toml", "requirements.txt", "setup.py"] }
keep = ["code", "python"]

[preset.research]
when = { ext = ["md", "tex", "bib", "pdf"], share = 0.6 }
keep = ["research", "writing"]

[preset.blank]
when = { blank = true }
keep = ["code", "research", "writing"]
auto_memory = false

[preset.general]
# Chosen when nothing matches and Jev answers none: nothing goes off.
keep = ["*"]
```

`auto_memory = false` writes `autoMemoryEnabled: false` (R3, "Memory per preset").

### 3.3 The catalog: `<home>/nirvana/preload-catalog.toml`

Each item (`skill:<name>`, `mcp:<server>`, `plugin:<id>`) maps to its tags.
- **Name rules seed it for free.** `bambu*`, `blender-stl*` and `cosplay*` get `3d-print`. `*-color-correction` and
  `batch-photo-*` get `photo`. `scholar-*` gets `research`. `*-lsp` plugins get their language. The `claude.ai`
  personal connectors (Gmail, Calendar, Instacart, Uber Eats, Booking.com, Felt Maps) get `personal`.
- **Jev tags the rest.** Any item the rules leave untagged goes to Jev once, with its description. Skill descriptions
  come from the SKILL.md front matter, and an MCP server is described by its name and tool names. Jev answers a choice
  over the known tags plus `none`, with the 0.7 floor of plan 12 O8b. Below the floor, or on any failure, the item
  stays untagged, which means it stays on.
- **The owner's edits win.** A line the owner wrote is marked `# owner` and never retagged.

**A rule of the catalog:** an item goes off only when it has tags, none of them is in the preset's `keep`, and it is not
`core`.

### 3.4 Choosing the preset

1. **The rules.** The first preset in file order whose `when` matches wins. A marker match beats an extension share.
2. **Jev**, when no rule matches, or when two rules match within 10% share. Jev gets the fingerprint summary (extension
   counts, markers, the README's first 2 KB, never file contents) and the preset names plus `none`. The floors are 0.7
   and 0.3, as in O8b. `none` or a failure means `general`.
3. **The cache.** The choice, its reason line and the fingerprint hash go to `<home>/nirvana/preload/<folder_hash>.json`.
   An open with the same hash spends nothing.

Every Jev call goes to `<home>/nirvana/jev-calls.jsonl` with role `preload-tag` or `preload-folder`.

## 4. Applying at "Open Nirvana" (R2)

`nirvana window <folder>` already resolves the folder before `invoke_open`, so the preset is computed there, in that
process and before Claude starts:
1. **Fingerprint, then the cached or new choice.** Jev is called only on a cache miss, with a 3 s wall. Past the wall
   the open goes ahead on `general`, and the answer is used next time.
2. **Write the generated settings file.**
3. **Start Nirvana Claude** with `--settings <generated>`.
4. **Toast once:** "3d-print preset · 31 off · −5.9k a turn · Preload p to undo". The figure is an estimate from the
   catalog's last `/context` tokens. The panel's next reading measures it.

**Undo.** In the Preload panel, `p` turns the folder's preset off. The folder is then recorded `preset = "off"` and the
next open keeps everything on. Pressing `p` again turns the preset back on. Either way the change takes effect at the
next Claude start, and the panel's `r restart Claude` is already there.

**ROLE.md** gains one line: some tools may be off by the folder's preset; if one is needed, say which, and the owner
presses `p` or switches it on in Preload.

## 5. The Preload panel

- **The header line** gains the preset: "Preset 3d-print · 31 off · p undo". Its token figure becomes the per-turn
  total, with deferred apart: "23.5k a turn · 130k deferred".
- **Deferred rows.** MCP tool rows show their tokens muted with "deferred", and the provider sums count only used
  tokens. github reads "MCP, 46 tools · deferred 19.6k". Its server instructions are its per-turn row.
- **A preset-off row** is marked `[p]` instead of `[ ]`. Switching it on records `pinned_on` (§2).
- **The memory row.** A MEMORY.md over `trim_over_tokens` (default 4,000) shows "over 4k · t trim".

## 6. Memory (R3)

### 6.1 Resume the last session

- **Recording.** Nirvana Claude's SessionStart hook (the digest hook) writes `<home>/nirvana/sessions/<folder_hash>.json`
  with `{session_id, at}`. Its `Stop` hook updates `at`.
- **Resuming.** "Open Nirvana" on a folder whose record is under `resume_within_hours` old (default 24) launches with
  `--resume <session_id>`. The launch menu keeps a "Fresh Nirvana Claude" choice.
- **When resume fails.** If the transcript file is gone, the launch drops `--resume` and starts fresh.
- **Settings still apply.** A resumed session reads the settings at start, so the preset still applies.

### 6.2 Trim an oversized MEMORY.md

- **The trigger.** A folder's auto memory index (`~/.claude/projects/<enc>/memory/MEMORY.md`) over `trim_over_tokens`
  is flagged in the panel (§5). Nothing changes on its own.
- **`t` writes a proposal.** Jev rewrites the index to under the limit, keeping every `[[link]]` and file pointer, and
  dropping or merging hooks only. The proposal goes beside the file as `MEMORY.md.proposed`, and the panel shows
  "−3.9k · a accept · x discard".
- **Accepting keeps a backup.** The original is copied to `<home>/nirvana/backup/memory/<folder_hash>-<time>.md` first.
  `herdr preload memory restore` puts it back.
- **A failed check means no proposal.** Jev's output is rejected if any pointer line of the original is missing from
  it.

### 6.3 Memory per preset

`auto_memory = false` in a preset (§3.2) turns auto memory off for that folder's Nirvana Claude. It is on for every
seeded preset except `blank`.

## 7. The command

`herdr preload show|why|apply|off|on|catalog|tag|memory trim|memory accept|memory restore [<folder>]`
- `why` prints the fingerprint, the rule or Jev answer, and each off item with its tag.
- `apply` writes the generated file without launching.
- `catalog` prints every item with its tags and where each came from (rule, Jev or owner).

## 8. Packages (parallel, merged in order)

| Pkg | Model | Owns | Builds |
|---|---|---|---|
| P1 engine | Opus 5.5 high | `src/nirvana_preset.rs` (new), `src/nirvana_preload.toml` (new), the `preload` dispatch in `src/main.rs`, `src/nirvana_launch.rs` (`--settings`), the worker args in `src/nirvana_worker.rs`, `nirvana/claude/ROLE.md`, `nirvana/controls.d/p13a.py` | §2, §3, §4 (not the panel), §6.3, §7 |
| P2 panel | Sonnet 5.5 high | `src/nirvana_preload.rs` and its client draw code, `nirvana/controls.d/p13b.py` | §5; calls P1's `nirvana_preset::{view, set_preset, pin_on}` and P3's `nirvana_memory_trim::{status, propose, accept}` through stubs named here |
| P3 memory | Sonnet 5.5 high | `src/nirvana_memory_trim.rs` (new), `src/nirvana_session_record.rs` (new), the digest and stop hooks under `nirvana/claude/hooks/`, the menu's Fresh choice, `nirvana/controls.d/p13c.py` | §6.1, §6.2 |

The interfaces P2 calls are fixed in this plan:
- `nirvana_preset::view(cwd) -> PresetView { name, reason, off: Vec<(Switch, tag)>, saved_tokens }`
- `nirvana_preset::set_preset(cwd, on: bool)`
- `nirvana_preset::pin_on(cwd, &Switch, on: bool)`
- `nirvana_memory_trim::status(cwd) -> Option<TrimStatus>`
- `nirvana_memory_trim::propose(cwd)`, `accept(cwd)` and `discard(cwd)`

The merges run P1, then P3, then P2, each as a `version.py bump feature`.

## 9. Verification, written down before the build

**Fixtures.** Scratch folders under `target/`:
- `3d` holds a `.3mf`, two `.stl`, a `.gcode` and a README.
- `blank` is empty.
- `rust` holds a `Cargo.toml` and `src/main.rs`.
- `mixed` has no clear type and goes to Jev.

**`/context` before and after** on each fixture, through the generated file, from Python with a list argv, all free.

**Expected:**
- **3d:** the photo and scholar skills and the personal connectors are off; bambu and blender stay on.
- **blank:** the 3D, photo and personal items are off, and auto memory is off.
- **rust:** the 3D, photo and personal items are off.
- **The per-turn total** in each is below the 23.5k baseline. The exact figure is measured at the build, not promised
  here.

**Further checks:**
- **Manual toggles win:** the §2 check.
- **Undo:** `p` regenerates the file without the preset, and the next `/context` matches the baseline.
- **Resume:** a recorded session id launches with `--resume`; a missing transcript falls back to fresh.
- **Trim:** a fake Jev drops a pointer and the proposal is refused; a good one is accepted, and the backup is
  byte-identical to the original.
- **Jev:** at most 5 live calls in all, with their log lines pasted without the key.

Every assertion gets a control in its package's `controls.d` file.

**Limits.** The owner's own folders are never written by a check. Every live check uses scratch homes and an isolated
herdr server. No interactive Claude starts in an owner folder.

## 10. What this does not do

- **No starter CLAUDE.md.** It was not picked in R3.
- **No change to `claudeMdExcludes` by preset.** A project's CLAUDE.md is its own and is always loaded.
- **No preset for agy or Codex panes.** Their tool lists are not switched by Claude settings.
- **No Haiku.** That is R1.
