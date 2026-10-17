### 1. Verdict

The architecture in SPEC.md will deadlock in multi-hour runs and misroute parallel bursts because it does not deduct in-flight worker usage from pool headroom, freezes the entire system whenever the `max` pool exhausts, and passes unbounded prompts on the Windows command line via `--print=<prompt>`. Its pressure math divides 5-hour expected usage by 7-day remaining fractions, divides by zero at window resets, and treats missing provider buckets as `unknown` in a way that permanently demotes Gemini seats. Cut the shadow Jev roles, dynamic usage-fitting, pre-build blind test authoring, and TUI chrome, and fix the concurrency, quota-booking, and durability bugs below before writing Package O1.

---

### 2. Findings

#### F1. [BLOCKER] §5.3, §6.1 — Unbooked in-flight quota causes parallel seating stampedes
- **What is wrong or missing:** `headroom` (`1 - used - reserve`) is computed solely from the last `/usage` reading (cached for up to 5 minutes, §3 L78). It does not subtract the `expected_use` of workers already seated in the same turn or currently running.
- **Concrete failure scenario:** `max` 5h is at 79% used (`headroom = 1 - 0.79 - 0.15 = 6%`). In §6.1 step 2, the orchestrator calls `herdr seat pick` 4 times in 5 seconds for 4 parallel `build.bounded` orders (`expected_use = 5%` each). All 4 calls read the same cached 79% value, see 6% headroom, and seat all 4 workers on `max` (`sonnet`) instead of spilling to `g-sonnet`. Together they burn 20%, wiping out the 15% orchestrator reserve and killing the orchestrator mid-run.
- **Fix:** Track active orders in the journal and compute `effective_used = reading.used + sum(order.expected_use for order in active_orders_on_pool)`. Write a `seated` lease to the journal atomically inside `seat pick`/`seat run` so back-to-back calls see decremented headroom immediately.

#### F2. [BLOCKER] §3, §5.6, §12 R3 — Single-pool orchestrator starvation freezes all three pools when `max` exhausts
- **What is wrong or missing:** The orchestrator is pinned exclusively to `max` (§12 R3: *"When the Max 5 h window runs out... It waits for the reset... herdr keeps the workers on Google's pools running and wakes it after"*). Meanwhile, the orchestrator itself spends `max` on every wake, gate, plan update, small question, and small edit (§6 L238-L239), and an `opus` worker that overshoots its prior can easily consume the 15% reserve.
- **Concrete failure scenario:** At hour 1.5 of a 5h window, an `opus` load-bearing worker + orchestrator wakes push `max` 5h to 100% used. `gemini` and `google3p` are at 90% remaining. Two `argon` and `g-sonnet` workers finish their phase 1 orders 5 minutes later. Because the orchestrator is blocked on `max` for the remaining 3.5 hours, nothing gates phase 1, nothing merges, and nothing seats phase 2. All three pools sit completely idle for 3.5 hours.
- **Fix:** Reverse §12 R3: when `max` is exhausted and `resets_at > 15 min`, `herdr` must either launch a stateless stand-in orchestrator turn on `g-opus` (`google3p`) reading `.nirvana/runs/<run>/journal.jsonl` and the board, or raise the `max` reserve dynamically by `orchestrator_burn_per_wake * remaining_planned_orders`.

#### F3. [BLOCKER] §4, §5.6, §7.1 — Inlining order text in `agy --print=<prompt>` hits Windows `CreateProcessW` 32 KB limit
- **What is wrong or missing:** §4 L114-L115 mandates inlining the entire order text into `--print=<prompt>` via list `argv` (§3 L82) because passing a bare file path caused `agy` to misinterpret it. On Windows, `CreateProcessW` fails hard at 32,767 UTF-16 characters across the full command line.
- **Concrete failure scenario:** An `opus` worker hits a rate limit halfway through a feature and leaves a 35 KB worktree diff. §5.6 step 3 re-seats on `g-opus` (`agy`) with a continuation brief containing the order, prior report, and worktree diff in `--print=<prompt>`. `std::process::Command::spawn` fails immediately with `os error 206` (`ERROR_FILENAME_EXCED_RANGE`), triggering 4 rapid backoff retries and a permanent crash-loop refusal (§7.2 L280).
- **Fix:** *(Confidence: High — from `agy` runtime & `view_file` tool behavior.)* Write the order, checkpoint report, and diff to `.nirvana/orders/<id>.md` inside the worktree, and pass a fixed short prompt: `--print="Read .nirvana/orders/<id>.md with view_file and execute the instructions inside."`

#### F4. [BLOCKER] §5.3 — "The bar drops to the best one left" routes load-bearing tasks to underqualified seats or panics on empty filter set
- **What is wrong or missing:** Step 1 applies hard filters (capabilities, class exclusions, pool floor `<= 5%`, `max` reserve). Step 2 states: *"If no seat clears it [the class's bar], the bar drops to the best one left."* It defines no hard minimum competence floor and no return contract when Step 1 eliminates all seats or leaves only low-tier seats.
- **Concrete failure scenario:** During a `fix.debug` or `build.bounded` run in SOTA mode, `max` is at the 15% reserve and `google3p` is at 4% (filtered out by Step 1). Only `flash` remains. Step 2 silently drops the SOTA bar to `flash` instead of deferring for 15 minutes until `max` resets. Worse, for `build.load_bearing` (which excludes everything except `opus` and `g-opus` in §5.4 L172), when `max` is at reserve and `google3p` is at 4%, Step 1 returns an empty set and Step 2 has no "best one left" to drop to.
- **Fix:** Never drop a class's bar below its `Efficient` floor. If zero seats survive Step 1 + `min_bar`, `herdr seat pick` must return a typed `NoSeat { reason, earliest_reset_at }` payload that triggers §5.6 (`defer` or `ask`).

#### F5. [MAJOR] §4, §6, §7.1 — `agy` workers have unrestricted native subagents and background tasks
- **What is wrong or missing:** *(Confidence: High — direct from `agy`'s built-in tool declarations.)* The spec locks down Claude workers with `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1` (§4 L118) and a `PreToolUse` hook on `Agent` (§6 L231), assuming only Claude has subagents (§13 L412). In fact, `agy` has native `invoke_subagent` (`self`, `research`, `image-generator` with `Workspace: "inherit" | "branch" | "share"` and `Model: "inherit" | "flash_lite" | "flash" | "pro"`), `define_subagent`, `schedule`, and async background commands (`WaitMsBeforeAsync` / `IsDaemon`).
- **Concrete failure scenario:** `herdr` seats a single `argon` worker expecting 4% Gemini pool use. Inside its session, `argon` calls `invoke_subagent` to launch 4 concurrent `self` subagents with `Workspace: "branch"` and spawns a background daemon via `run_command(IsDaemon=true)`. The single order silently burns 20% of the Gemini pool, creates nested branched workspaces `herdr` does not track, and leaves orphaned daemon processes holding file locks after `agy` exits.
- **Fix:** Constrain `agy` workers via workspace rules/prompt instructions forbidding `invoke_subagent`, `define_subagent`, and `schedule` unless explicitly permitted by the order, and wrap every `agy` worker process in a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.

#### F6. [MAJOR] §6.1, §7.3 — Disjoint file ownership does not prevent semantic integration breakage across parallel worktrees
- **What is wrong or missing:** §6.1 splits tasks into parallel orders with disjoint owned files, and §7.3 runs tests inside each worker's isolated worktree branched from the initial base commit.
- **Concrete failure scenario:** Order 1 owns `src/seat.rs` and adds a field to `struct Reading`. Order 2 runs in parallel in its own worktree, owns `src/cli.rs`, and constructs `Reading` without the new field. Both pass `cargo test` in their own worktrees and pass §7.3's gate (tests pass, only owned files changed). Merging both in §6.1 step 5 breaks compilation on the target branch.
- **Fix:** §7.3's gate must rebase/merge the worker's worktree onto the current run integration branch tip *before* running the test check. If post-rebase tests fail, send back to the worker with the rebased state.

#### F7. [MAJOR] §7.2 — Waking the orchestrator by typing into its PTY (`herdr agent prompt`) collides with live user typing
- **What is wrong or missing:** When the orchestrator is not actively running a turn, `herdr` types a digest directly into its interactive pane (§7.2 L274). An agent being "idle between turns" does not mean the human owner's input buffer is empty.
- **Concrete failure scenario:** The owner is mid-sentence typing `"Wait, do not merge order 2 yet"` into the Nirvana Claude pane. A worker finishes and `herdr agent prompt` injects the wake digest + newline into the PTY, submitting the owner's half-typed text spliced with the digest.
- **Fix:** Suppress PTY keystroke injection whenever the pane has received user stdin within the last 30 seconds or when the input line is non-empty; queue the wake event until the user submits or clears input.

#### F8. [MAJOR] §4 — Online `expected_use` learning is corrupted by parallel workers and orchestrator context burn
- **What is wrong or missing:** §4 L105-L108 replaces `seats.toml` priors with the measured median of finished orders: using `remaining_fraction` deltas for `agy` *"when it is the only agy order running"* and fitting a scale factor against whole-percent `/usage` jumps for Claude.
- **Concrete failure scenario:** Because §6.1 runs workers in parallel, large multi-worker runs on `agy` are systematically excluded from measurement (biasing `agy` medians toward lone small runs). On `max`, the orchestrator is *always* co-running on the same pool; when the orchestrator's >150k context turn consumes 0.9% of `max` and a concurrent `haiku` chore consumes 0.1%, pushing `/usage` up by 1%, the fitter attributes the jump to `haiku` and poisons the routing table.
- **Fix:** Compute each order's cost directly from its streamed token counts (`input`, `cache_create`, `cache_read`, `output`) multiplied by model price weights, and calibrate only a pool-wide dollar-to-quota constant during isolated probes (P4).

#### F9. [MAJOR] §2, §4 — Missing >200k long-context pricing tier and 2026-11-30 Argon price doubling
- **What is wrong or missing:** *(Confidence: High on Gemini tiered pricing model; High on the spec's own 2026-11-30 date in §2 L38-L39.)* Gemini Pro/Argon models double per-token input/output prices above 200k context, and Argon's promotional price ($2/$10) doubles to $4/$20 after 2026-11-30. Because `agy` quota is consumed proportionally to token cost (§2 L30-L31), a single `read.huge` (>400k tokens) order on Argon after Nov 30 costs ~4x per token vs the initial prior.
- **Concrete failure scenario:** After 2026-11-30, an uncalibrated `read.huge` class order on `argon` expects 5% pool use based on Oct 2026 sub-200k priors, but actually consumes 20–25% of the `gemini` 5h window in one turn.
- **Fix:** Store token price schedules (including `>200k` context multipliers and effective dates) in `seats.toml` rather than raw fraction priors alone.

#### F10. [MINOR] §4, §5.2, §5.4 — `research.web` is defined in §5.2 but missing from §5.4, and `pdf` is missing from `g-opus`/`g-sonnet` in §4
- **What is wrong or missing:** §5.2 L141 lists `research.web`, but §5.4 omits its chain and §4 has no `web` capability. Also, §4 L97-L98 omits `pdf` from `g-opus` and `g-sonnet` even though `agy`'s `view_file` tool natively supports `pdf` *(Confidence: High — from `agy` `view_file` schema)*.
- **Concrete failure scenario:** `herdr seat pick --class research.web` has no chain in `seats.toml`; an order with `--needs pdf` refuses `g-opus`/`g-sonnet` when `max` is full.
- **Fix:** Add `pdf` to `g-opus`/`g-sonnet`, add `web` capability to `argon`, `flash`, `opus`, `sonnet`, `g-opus`, `g-sonnet`, and add `research.web -> flash (low), argon (low), g-sonnet (low), sonnet (low)` to §5.4.

---

### 3. Seating, Effort Rules, and Gemini / Antigravity Capabilities

*(Confidence note: Statements about `agy`'s built-in tools, file limits, subagents, and workspace modes are **High confidence** directly from `agy`'s runtime tool definitions. Statements about Gemini 4 Argon and 3.8 Flash relative token verbosity and benchmark priors rely on §2's empirical measurements combined with **High confidence** knowledge of Gemini multimodal/caching architecture.)*

#### Corrections to the Spec on Antigravity (`agy`) and Gemini
1. **`agy` binary/video ingestion limits (§2 L45-L47, §11 P2, §13 L410):** §13 L410 says it is unsure whether `agy` reads MP4s and what the size limits are. `agy`'s `view_file` tool natively supports `image, pdf, video, audio` binary files with a **hard 100 MB file-size limit**, and requires omitting `StartLine`/`EndLine`. Raw screen recordings over 100 MB fail at the tool layer before reaching Gemini unless re-encoded with `ffmpeg`.
2. **Flash `high` vs Argon `medium` quota cost (§2 L39-L41, L48-L49):** One Argon token costs ~2.7x one Flash token ($2/$10 vs $0.75/$3.75), but §2 L49 measured `gemini-3.8-flash-high` at **16x Claude's tokens** on builds due to verbose thinking and tool churn. Whenever Flash `high` consumes $>2.7\times$ the tokens of Argon `medium`, **Flash `high` burns more of the `gemini` pool than Argon `medium`** while delivering lower coding accuracy.

#### Disagreements with Class Chains (§5.4) and Effort Rules (§5.5)
- **`build.load_bearing` (`opus, g-opus` -> add `argon` at `high`):** §5.4 trusts `argon (high)` for `plan` and `review.adversary`, yet excludes it from `build.load_bearing`. If `max` and `google3p` are exhausted, load-bearing builds halt even when `gemini` is 100% free. Seat `argon (high)` third.
- **`build.bounded` (`sonnet, g-sonnet, flash [high]` -> change to `sonnet [medium], g-sonnet [medium], argon [medium], flash [high]`):** Putting `flash (high)` ahead of `argon (medium)` is a mistake. On bounded code edits, `argon (medium)` avoids Flash's 16x token runaway loop and is safer on the `gemini` pool.
- **`write.docs` (`sonnet, argon, g-sonnet` -> change to `argon, flash, g-sonnet, sonnet` at `low`/`medium`):** §2 L43 and §5.4 L183 note Argon leads on the Vals Index and oh-my-opencode seats writing on Argon. Putting `sonnet` (`max`) first wastes the orchestrator's scarce pool on docs. Seat `argon` first.
- **`fix.debug` (`opus, g-opus, argon` at `medium` -> raise `argon` to `high`):** Debugging on Argon at `medium` under-allocates reasoning on root-cause analysis, leading to trial-and-error edit loops that cost more cached/output tokens than a single `high`-effort pass.
- **`judge.visual` (`opus, g-opus` -> add `argon [high]` for video/interaction checks):** Static screenshots can stay on Opus, but any visual check involving animations, transitions, scroll behavior, or screen recordings of a reproduction must seat `argon (high)` because Claude cannot ingest video.
- **§5.5 L192-L193 effort downgrade order is backwards:** *"Under pressure: first the same model on another pool, then one effort step down, then the other family."* Downgrading `opus` from `high` to `medium` on a congested Claude pool *before* switching to `argon` at `high` on an uncongested `gemini` pool both burns scarce Claude quota and degrades reasoning. Switch to an uncongested pool's top model at full class effort *before* stepping down effort.

#### Gemini & Antigravity Capabilities Missed or Misused
- **Video reduced to text transcription (§5.4 L181, §6 L227):** Having Gemini merely write a transcript of a video for Claude throws away spatial/temporal visual reasoning. For UI bugs, TUI rendering glitches in `herdr`, or E2E test recordings, `argon` should inspect the video *and* the source files directly to diagnose the bug or gate the fix. Also, `skills/media` needs an `ffmpeg` pre-step (`-vf scale=-2:720 -r 5`) to keep recordings under `agy`'s 100 MB `view_file` cap.
- **1M context cache destruction (§2 L39, §5.4 L180, §6 L243):** §2 L39 notes Argon cached input is **95% off**, yet §6 L243 forbids ever resuming a worker for a follow-up question. On a `read.huge` (500k token) session, spawning a new `agy` session for a second question re-ingests 500k uncached tokens at 20x the cost of `agy --conversation <id>`. Allow resuming `read.huge` sessions for read-only follow-up queries.
- **Native Web Search & URL reading (`search_web`, `read_url_content`):** `agy` has built-in Google Search grounding and URL fetching at zero `max` token cost. Route `research.web` and library/docs verification to `flash (low)` or `argon (low)` on `agy` first.
- **Native Image Generation (`image-generator` subagent in `agy`):** `agy` can generate and edit raster assets/mockups natively. Expose an `image_gen` capability on `argon`/`flash` for `build.frontend` asset tasks.

---

### 4. The Pressure Formula and Usage Rules (§3 and §5.3)

$$\text{pressure} = \frac{\text{expected\_use}}{\text{headroom}} \times \text{pace}$$

1. **Unit mismatch between `expected_use` (5h) and `headroom` (tightest window, 5h or 7d):**
   - §4 L104 defines `expected_use` as a fraction of the pool's **5h window**, while §5.3 L157 sets `headroom` to `1 - used` of the pool's **tightest window** (`5h` or `week`). Dividing a 5h fraction by a weekly remaining fraction is dimensionally broken: 20% left in a *week* holds ~5–10x more tokens than 20% left in *5 hours*, yet the formula scores them as identical headroom. Compute pressure per window $w$ using $\text{expected\_use}_w / \text{headroom}_w$ and take $\max_w \text{pressure}_w$.
2. **Zero-usage / post-reset division collapse (`pace -> 0` or `pace -> infinity`):**
   - Immediately after a window reset ($t_{\text{elapsed}} \to 0$), if `used == 0`, a ratio `pace = used / elapsed` is `0/0` (NaN) or `0`. If `pace == 0`, `pressure = 0` for *every* seat in that pool, erasing `expected_use` so `opus` and `argon` cost the exact same (`0`) as `haiku` and `flash`. Conversely, if a 1% probe runs in minute 1 of a 300-minute window (`elapsed = 0.0033`), `used / elapsed = 3.0`, falsely flagging a 99%-full pool as 3x ahead of pace. Regularize pace: $\text{pace} = \text{clamp}\left(\frac{\text{used} + 0.10}{\text{elapsed} + 0.10}, 0.5, 2.0\right)$.
3. **End-of-window discount bleeds into the next window (§5.3 L159-L160):**
   - Discounting a 5h window with `<60 min` and `>30%` left ignores order duration. If 2 minutes remain before reset and 35% is left, seating a 25-minute build charges 23 minutes of heavy token use to the *newly reset* 5h window. Scale the discount by $\min(1, t_{\text{to\_reset}} / t_{\text{order\_duration}})$.
4. **Step 1 floor check ignores order size on non-`max` pools (§5.3 L149-L150):**
   - A pool at 6% remaining passes the 5% floor even if the order's `expected_use` is 20%, guaranteeing a mid-order rate-limit interrupt. Filter out any seat where `headroom < expected_use` when another pool can fit the whole order.
5. **The "not exposed reads as unknown" rule (§3 L75-L76) breaks optional buckets:**
   - §2 L32 notes `agy` does not expose a separate `week.argon` bucket. If `week.argon` is modeled as an unexposed window that reads as `unknown`, and `unknown` ranks below known headroom (§3 L75), `argon` will permanently rank below every seat with known headroom. Only treat a pool as `unknown` when `/usage` fails or returns zero windows for the entire pool.
6. **Stale-reading loop on idle accounts (§3 L77-L78):**
   - When a 5h window expires while idle, `resets_at` is in the past until a new turn starts a fresh window. If `herdr` treats `now > resets_at` as perpetually stale even after re-reading `used == 0%`, every `seat pick` will block on spawning a fresh `/usage` CLI call. Key staleness on `now - read_at > 5m`, or mark a post-reset reading fresh once re-polled after `resets_at`.
7. **Orchestrator gaming:**
   - Because `build.load_bearing` and `build.frontend` only allow `opus`/`g-opus` (§5.4), the Claude orchestrator can bypass Gemini routing simply by classifying routine edits as `build.load_bearing`, or by abusing §5.1 L129's free-text override. Require overrides to pass `PreToolUse` reserve checks and cap overrides per run.

---

### 5. Long-Running Durability (§7.2): What Breaks in a 12-Hour Run

1. **The "6 wakes per run" auto-mode cap (§12 R6) halts the run in under 1 hour:**
   - Every completed phase, stall, send-back, or window reset wakes the orchestrator (§6.1 L252). A 12-hour run across 15 orders and two 5h resets needs 20–40 wakes. Capping auto mode at 6 total wakes guarantees the run stops and waits for the owner early in hour 1. Replace with a cap of **3 consecutive unproductive wakes** (wakes that do not advance any order to `gated`/`merged`).
2. **`MAX_DEFER_S = 30 min` (§5.6 L202) aborts across 5h window boundaries:**
   - In a 12-hour run, pools will hit exhaustion with 1–4 hours left until their 5h reset. When the last fallback pool exhausts with 45 minutes left until the earliest reset (`> 30 min`), §5.6 refuses to defer and drops to Step 4 (*"ask the owner"*), stalling the unattended run for the rest of the night. When no fallback seat exists, allow deferring up to the full 5h window (`18,000 s`) by setting a wake timer for `min(resets_at) + 60s`.
3. **Orchestrator context bloat and advisory-only compaction (§6 L231, §9.4):**
   - `herdr jev compact` only *advises* the orchestrator in a `Stop` hook. Over 12 hours of ingesting Work Reports and gate outputs, if the orchestrator ignores the advice or compacts late, its context exceeds 150k tokens (where 74% of `max` quota is burned, §2 L35). Enforce a hard token threshold where `herdr` checkpoints state to the board/journal and starts a fresh orchestrator session (or forces `/compact`) every $N$ phases.
4. **Worktree disk explosion and Windows file locks (§7.1 L260, §7.3 L297):**
   - 20+ worktrees running `cargo test` over 12 hours without a shared build cache will consume 50–100+ GB of disk in duplicate `target/` directories; conversely, sharing `CARGO_TARGET_DIR` causes parallel gates to block on Cargo's file lock. Worse, on Windows, any lingering child process (`node.exe`, `rust-analyzer`, or a dev server started for §7.3 L297's port check) holds directory handles open, causing `git worktree remove` and merges to fail with `os error 5` / `os error 32` and exhausting local ports. `herdr` must kill each worker's Win32 Job Object and reap merged worktrees and panes immediately after gating.
5. **Interactive resume prompt after crash (§7.2 L281):**
   - *"A new orchestrator session on the folder offers to continue the open runs."* In an unattended 12-hour run, if the orchestrator pane crashes, "offering" to continue waits forever for a human click. Auto-mode runs must resume open journals automatically on restart.

---

### 6. What to Cut to Ship a First Useful Version

1. **Cut Package O7 and Jev roles 2, 3, and 4 (§9 L322-L330, §11 L377):** Roles 2 and 3 ship in `shadow` (zero runtime effect) while adding OpenRouter dependencies and 5-minute TUI screen-scraping loops. A deterministic 10-minute JSON-stream silence timeout in `herdr` is sufficient to detect stalled headless workers.
2. **Cut the `local` pool (`nvidia-smi`, Ollama `qwen3.6:35b-a3b`), `fable`, and `gpt-oss` (§3 L66, §4 L96-L102):** Seven seats (`opus`, `sonnet`, `haiku`, `g-opus`, `g-sonnet`, `argon`, `flash`) already cover all 3 subscription pools and every capability.
3. **Cut online median replacement and whole-percent scalar fitting (§4 L104-L108):** Use static priors in `seats.toml` calibrated once from Probe P4 plus streamed token counts, and log actual deltas to `record.jsonl` for offline tuning.
4. **Cut pre-build blind held-out test authoring (§7.3 L298-L300):** Having a separate model family write blind tests *before* the builder writes the implementation causes false failures over internal naming/signature mismatches. Keep the deterministic post-build gate (rebased test suite + file-ownership check) and the post-build cross-family `review.adversary`.
5. **Collapse 19 classes (§5.2) into 8 chains:** Merge identical chains into `plan`, `build.load_bearing`, `build.bounded`, `build.visual`, `fix.debug`, `review.adversary`, `research` (read/web/huge), `media` (video/audio), and `chore`.
6. **Defer Package O6 (the Seats TUI square, §8 L306-L308, §11 L376) until after O8:** The statusline (§6 L232) and `herdr seat list` / `herdr usage` CLI output are enough to run and verify the O8 A/B benchmark before building custom TUI widgets.
