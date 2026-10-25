# Changelog

All notable changes to this project are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Resources Helper is now called Allot, and everything it installs is renamed:

  | Before | Now |
  |--------|-----|
  | `rh.exe`, `rhd.exe`, `rh-mcp.exe`, `rh-tray.exe` | `allot.exe`, `allotd.exe`, `allot-mcp.exe`, `allot-tray.exe` |
  | `%USERPROFILE%\.resources-helper\` | `%USERPROFILE%\.allot\` |
  | `rh.db`, `rhd.lock`, `logs\rhd.log` | `allot.db`, `allotd.lock`, `logs\allotd.log` |
  | MCP server `resources-helper` | MCP server `allot` |
  | `~/.claude/resources-helper.md` | `~/.claude/allot.md` |
  | Logon task `resources-helper`, Run value `Resources Helper` | Logon task `allot`, Run value `Allot` |
  | `RH_*` environment variables | `ALLOT_*` |

  The setup, or `allot install`, takes over an existing Resources Helper install: it copies
  the database, token, config, tray settings, and logs into the new folder, replaces the logon
  task, the Run entry, the Start menu shortcut, the MCP server, the hooks, and the CLAUDE.md
  import, and removes the old binaries. allotd keeps the old port and token, so sessions that
  are open during the upgrade keep working until they restart. The rest of the old folder is
  deleted by a later install once nothing runs from it.

## [0.2.0] - 2026-10-25

Adds a tray app that shows every session's jobs and can stop them, and makes `allot install`
set up everything on its own.

### Added

- The Allot tray app (`allot-tray.exe`, WinUI 3, Native AOT): a notification-area icon that
  shows whether the machine is idle, busy, or has jobs waiting, and a flyout with every
  session's running and queued jobs, their process, command, class, time, and estimate.
- Kill and Remove in the flyout, both two-step. Kill ends only the process tree the lease's
  command started and refuses Claude Code itself, allotd, allot, allot-mcp, and anything older
  than the command. Remove takes a queued request out of the queue. The session is told either way and
  asks you whether to retry or skip.
- A benchmark card that follows a `benchmark_exclusive` lease through draining, waiting for
  quiet, and running alone, and reports background noise when it finishes.
- A Resources card with CPU, RAM, GPU, VRAM, and drive meters showing used, reserved, and free
  capacity, and a CPU history line.
- Toasts for a stopped service, a job running well past its estimate, finished jobs and
  benchmarks, and jobs stopped from outside the flyout, each with its own switch.
- A service-down banner with Start service, a first-run card, and Launch at logon.
- Keyboard and Narrator support, high contrast, and layouts that hold at 100-300% scaling and
  larger text.
- `allot kill` and `allot remove`, and end causes in `allot status` and `allot leases --all`.
- `allot hook post-tool`, a PostToolUse and PostToolUseFailure hook that tells the session when
  you stopped its job; PreToolUse and the MCP tool results report it too.
- Lease presets (`cpu_small`, `cpu_medium`, `gpu_small`, `quick`, `low_impact`, and your own
  under `[presets.<name>]`), so a job can reserve less than a whole class.
- Quick leases (an estimate of 15 s or less) and low-impact leases, which can run while a
  benchmark waits or runs; allotd ends a quick lease still active after 60 s.
- allotd ends a lease that was forgotten: no command runs under it any more and its session has
  been idle for 3 minutes.
- Fixed drives with free space and read and write rates, and executable paths, in the
  machine snapshot.

- `allot-<version>-setup.exe`, a per-user setup (no admin) that installs everything, sets up
  Claude Code and autostart, adds a Start menu entry, and uninstalls from Apps & features,
  optionally with your data.

### Changed

- `allot install` sets up everything itself, with nothing left to paste:
  - the tray app in `bin\tray\`, started, with Launch at logon on the first install
    (`--launch-at-logon on|off` to choose);
  - the MCP server, through `claude mcp add`, or in `~/.claude.json` when `claude` is not
    found;
  - the three hook entries in `~/.claude/settings.json`;
  - the rule, in `~/.claude/allot.md`, imported into `~/.claude/CLAUDE.md` in place of a copy
    pasted by hand;
  - the scheduled task that starts allotd at logon.

  Edited files are backed up beside them as `<name>.allot-backup`. `--no-claude` and
  `--no-logon-task` leave those parts out, and `--tray <dir>` installs a tray build from
  elsewhere.
- `allot uninstall` also quits and removes the tray app and its Launch at logon entry, and
  removes the MCP server, the hooks, the rule import, and the scheduled task.
- The release zip includes the tray app in a `tray` folder.
- A queued lease keeps waiting without asking you first; the session says in one line what is
  ahead and the estimated wait.

### Removed

- `claude-md-rule.md` in the data folder; the rule now lives in `~/.claude/allot.md`.

## [0.1.0] - 2026-10-23

First public release.

### Added

- `allotd`, a per-user daemon on `127.0.0.1` with bearer-token auth, a single-instance lock,
  and a SQLite ledger (Diesel, WAL, embedded migrations) of leases, their audit trail, and job
  history.
- A Windows sampler for CPU, RAM, GPU, and VRAM through PDH counters and DXGI, with per-process
  attribution, including short-lived build processes.
- Admission control with a configurable headroom, EASY backfill without starvation, heartbeat
  expiry, and reaping of leases whose session process has exited.
- `benchmark_exclusive` leases that drain the machine, wait for a quiet window (capped by
  `benchmark.max_quiet_wait_secs`), and record background noise on release.
- Learned job profiles: each released lease records what the job used, and later runs of the
  same command in the same folder are estimated from the p90 of that history.
- `allot-mcp`, an MCP server with `system_snapshot`, `can_fit`, `acquire_lease`,
  `release_lease`, and `list_leases`, which heartbeats its leases and releases them when the session ends.
- `allot hook pre-tool`, a PreToolUse hook for the Bash and PowerShell tools that denies heavy
  commands without a covering lease and fails open when allotd is down.
- A token-based heavy-command classifier with configurable rules, shown by `allot classes`.
- The `resource-guard` skill and a CLAUDE.md rule that tell sessions when and how to lease.
- The `allot` CLI: `status`, `leases`, `history`, `classes`, `daemon start|stop|status`,
  `install`, and `uninstall [--purge]`.
- Restart recovery of active and queued leases, log rotation, retention pruning, and config
  hot reload.
- A logon task for autostart, and a runbook for health checks, troubleshooting, and
  configuration.

[Unreleased]: https://github.com/meh7an/allot/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/meh7an/allot/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/meh7an/allot/releases/tag/v0.1.0
