---
name: jobs
description: Use when running long builds, test suites, or gates (>1 min). herdr job commands - run in visible Jobs tab, wait, list, status, tail, and stop.
---

# Jobs: long commands in view

`herdr` is the fork's binary, `"${CLAUDE_PLUGIN_ROOT}/../../bin/herdr.exe"`.

Run builds, test suites and gates longer than about a minute with `herdr job run`, then check them with
`herdr job status` or `herdr job tail`, never by reading the whole log into context.

## Commands

- `herdr job run --name <name> [--cwd <dir>] [--wait] -- <command...>`:
  Runs the command in a visible pane in the space's Jobs tab (created if none exists). The owner's working tab is
  never switched away from. Output is copied both to the pane and to `<home>/nirvana/jobs/<id>/out.log`.
  Prints the job id and returns. With `--wait`, blocks until the command ends, prints the last 20 lines, and exits
  with the job's code.
  Outside herdr, with no server, runs in place with the same record and log.
- `herdr job list`:
  Lists all recent jobs: id, name, state (`running`, `passed`, `failed` or `lost`), elapsed time, and last line.
  "lost" means the exec process is gone and no exit status was recorded.
- `herdr job status <id>`:
  Prints one job's status line.
- `herdr job tail <id> [-n N]`:
  Prints the last N lines (20 by default) of the job's log.
- `herdr job stop <id>`:
  Ends that job's process tree cleanly, never affecting any other process.
- `herdr job clean`:
  Removes jobs that ended over 7 days ago.

## Square and panel

The mod strip holds a **Jobs** square:
- Shows counts of running and failed jobs and Claude background tasks, e.g. `Jobs 2 running · 1 failed`.
- With nothing in the last 12 hours it reads `Jobs`.
- A click on the square opens the panel listing jobs and Claude tasks (newest first).
- Clicking a job focuses its pane in the Jobs tab.
- Clicking a Claude task opens a pane in the Jobs tab tailing the file.
