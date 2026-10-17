# P5: what a spent quota looks like (2026-10-17)

No window was spent on purpose. Sources: `agy changelog` on this PC (agy updated itself from 1.4.0 to 1.4.1 during the
probes), and Claude's `rate_limit_event` from the P1 and P4 streams in this folder.

## agy

- **Headless failure**: "they now exit with code `3` and print the `AGY_ERROR` line, and JSON error output includes the
  partial response" (1.2.10). The line is "a structured `AGY_ERROR: {...}` JSON line on stderr with canonical status,
  HTTP or gRPC error code, retryability, and error ID ... and exits with code `3` instead of `1`" (1.2.6).
- **A spent individual quota is reported at once**: "reaching your individual quota is reported right away instead of
  being retried" (1.3.3).
- **Retry rule**: agy "waits the delay the server asks for ... and stops right away instead of retrying when the delay
  is longer than 30 seconds or the quota is a daily or billing cap" (1.2.13).
- **Credits**: past the plan's quota with credits too low, agy "now immediately says 'Your AI credits balance is too
  low to continue.'" (1.2.15). A third-party page says the overage setting is Never or Always; plan 12 assumes Never.
- **A 503 exited 0** in old Nirvana's adapter (before the 1.2.10 fix); herdr reads `status` and the exit code both.

So herdr's typed signal for agy is: exit 3 with an `AGY_ERROR` line whose status names a quota or resource
exhaustion, or a `result` whose `status` is not `SUCCESS`. The exact status string for a spent quota is not in the
changelog and is recorded when it first happens.

## Claude

See `results.jsonl`, the `rate_limit_events` of each Claude run: the fields herdr reads are the event's status and its
window utilizations and reset times.
