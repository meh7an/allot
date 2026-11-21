# Changelog

All notable changes to this project are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.1] - 2026-11-21

### Added

- macOS: `allot-<version>.dmg`, a window to drag Allot onto Applications. Allot.app now carries
  `allot`, `allotd` and `allot-mcp`. The first run of a new app version installs them with its
  own `allot install` when `~/.allot/bin` holds older ones or none, which restarts allotd
  through its LaunchAgent and sets up Claude Code, and a notification says so. A later run that
  finds an older allotd offers Update Service in the panel and in Settings. The app never
  replaces a newer allotd.

### Changed

- `allot install` on macOS clears the quarantine flag of the binaries it copies, so Gatekeeper
  does not stop a copy that came from a downloaded DMG or archive.

### Fixed

- Settings in every panel say plainly when the service running is older than the panel and has
  no settings routes, with Restart Service and Open Logs, instead of reporting an answer
  Settings can't use. Reasons from allotd start with a capital letter in every banner.

## [0.6.0] - 2026-11-21

### Added

- A Settings window in every panel (the Windows tray app, the macOS menu bar app, the GNOME
  extension and the KDE widget) that edits every key of `config.toml` and the panel's own
  settings. Pages: Panel, Admission, Sizes, Commands, Benchmarks, Leases and Service. Single
  values save as you change them; classes, presets and rules are edited in a dialog and saved
  whole. allotd writes the file: it validates the change first, keeps your comments and layout,
  writes atomically and reloads. A hand edit made meanwhile is caught and shown, and an invalid
  file or a stopped service makes the pages read-only.
- What fits on this machine: Admission and Sizes show how many jobs of each class fit now and
  with every lease ended, at full size and shrunk, for the values on screen before they are
  saved.
- Test a command: Commands shows which rule and class each command of a line gets, with the
  rules being edited, and marks the matching rule.
- `GET` and `PATCH /settings`, `POST /settings/preview` and `POST /classify` on allotd, with
  their schemas in `contract/`.
- `POST /shutdown` takes an optional body, `{ "by": "cli" }` or `{ "by": "panel" }`. With it,
  allotd writes the stop record before it stops (`contract/shutdown-request.schema.json`).
- Screen readers hear errors, banners, the outcome of a save and the result of Test a command
  without a focus change, and read each class or preset row as one sentence.

### Changed

- Stopping the service keeps it stopped. `allot daemon stop` and Stop service in a panel write
  `~/.allot/stopped.json`; until `allot daemon start`, Start service in a panel, or the next
  sign-in, sessions do not start allotd, and the hook lets heavy commands through without a
  lease. `allot status` and `allot doctor` show the stopped state.
- The panels' More options menus shrink to Settings, Stop or Start service and Open logs (and
  Quit on macOS); meters and notifications moved into Settings.
- The GNOME extension talks to allotd over the session bus (`tech.mehran.Allot`) instead of
  loopback HTTP, as extensions.gnome.org review asked. It follows leases at once through a
  `JobsChanged` signal and needs the allotd of the same release.
- On Windows the flyout opens next to the tray icon and slides from whichever edge the taskbar
  is on: bottom, top, left or right. The tray icon's menu opens away from the taskbar.
- allotd refuses a request whose `Host` is not `127.0.0.1` or `localhost` on its port, `/health`
  included, so a web page cannot reach it through DNS rebinding. Lease labels and command lines
  are cut to 512 characters. A config error on `/health` names `config.toml` without its folder.
- A session's agent no longer runs `allot kill`, `allot remove` or `allot daemon stop` on its
  own: the hook asks you first.
- Kill spares a Claude Code session inside the killed tree, such as a `claude -p` the command
  started, whether or not it holds a lease, on every OS and with or without a Linux scope.
- `leases.heartbeat_ttl_secs` must be at least 10, and a class or preset's `cores` at most
  4096; a `config.toml` outside these is rejected with the key named.
- allotd re-reads its token file when a request brings another token, so a token file replaced
  while it runs takes effect without a restart, and the panels' Restart service works again.
- The GNOME extension starts allotd only through its systemd user unit, and says to run
  `allot install` when the unit is missing. When allotd runs without the session bus, the
  extension says so and offers a restart instead of showing it stopped, and it never starts a
  second one. Opening Settings leaves window handling to GNOME.
- The GNOME and KDE Settings windows share one wording: Open File and Show in Folder in Files,
  one accessible description per row, and a note when `config.toml` does not exist yet.
- A settings request whose body is not the JSON the route takes gets a `SettingsRefusal` over
  HTTP and D-Bus, as every other settings refusal does.
- The GNOME extension's Settings window no longer names a port; it says when allotd is not on
  the session bus. The extension ships with its license and without the HTTP client, which
  only the KDE widget uses.

### Fixed

- allotd shuts down gracefully when it is stopped right after it starts. A stop signal
  sent once its port answered, but before its handlers were in place, ended it at once.
- On Windows a Kill no longer ends processes that another program started after it was given
  the PID of a process the kill had already ended.
- A lease no longer binds to another lease's command, or to a command its session started
  after the lease's own command ended, so Kill and idle release act on the right processes.
- Lowering Lease timeout in Settings no longer expires the leases of sessions already open:
  allot-mcp sends each heartbeat by the expiry allotd last gave, not by the startup config.
- A mistake in `config.toml` no longer locks clients out of a running allotd. allot-mcp,
  `allot daemon stop`, `allot status` and `allot doctor` use the file's port, or the default
  one, and print a warning.
- A start of allotd that fails, such as on a taken port or an invalid config, keeps the user's
  stop on record.
- `allot install` keeps the file mode of Claude Code's files and never overwrites a change
  Claude Code saved while it ran.
- A Kill or Stop sent over D-Bus while allotd shuts down finishes first, and a slow session bus
  no longer holds up the HTTP API at start.
- Settings in every panel: a save refused for any reason but a file in use waits for Try again
  instead of retrying every 600 ms, and one refused because allotd did not answer is sent again
  once it answers. A refresh can no longer bring back an old revision and report a change
  outside Settings. A refusal no setting owns is shown before the edits are undone. A blank or
  duplicate entry no longer counts as an unsaved change. Values round half away from zero in
  every panel, so 256 MB reads 0.3 GB everywhere.
- Settings on Windows and macOS show why Start service or Stop service failed and name the port
  the panel dials, and Quit saves changes not sent yet. A Cores value too large for a number
  is refused instead of crashing the macOS app.
- On Windows, banners no longer take keyboard focus every few seconds, the Headroom slider stops
  below 100, and values shown in larger units keep their decimals.
- On macOS, the Settings window keeps its place when left at the top of the screen, and a
  cancelled Stop service no longer reads as stopped.
- The GNOME extension and the KDE widget keep polling after an error while showing an update,
  and GNOME's Stop service shows a short reason instead of the D-Bus error.
- KDE's Open logs works when the home folder's path holds `#` or `%`.

## [0.5.0] - 2026-11-17

### Added

- Run statistics: the median, p90 and max of cores, RAM, GPU, VRAM and run time over a job's
  newest 20 clean runs. `allot history` shows them as a table per job and above a job's runs,
  and they are in `/history` and `/history/runs` (`stats`). `release_lease` says what the run
  used (`run`) and what the next run of the command would reserve (`next`).
- The panels on Windows, macOS, GNOME and KDE show "5.6 of 8 cores" on a running lease that
  started with fewer cores, and a median and max line under a recently finished job with 2 or
  more runs.
- `[estimates]` in `config.toml`: `lower` (true), `min_runs` (3) and `margin_pct` (10).
- The runbook's Admission section explains the room, minimum cores, learned estimates, live
  overruns and how to read a queued lease's reasons. Its Configuration table now lists every
  key, `min_cores` and `[presets.*]` included.

### Changed

- Learned estimates lower as well as raise. After 3 clean runs of the same command in the same
  directory, a lease reserves what they used plus 10% (p90 of cores, GPU and VRAM; the highest
  peak of RAM) instead of its declared profile, never below 0.25 cores, 256 MB or a `min_cores`
  the request names. Run time never lowers, benchmarks never lower, and lowering never makes a
  lease low impact. The reasons say what changed, for example "cores 2.2 from p90 of 3 past
  run(s) plus 10% (declared 8.0)". A job whose work runs outside its Claude session, such as
  tests in containers, no longer counts twice: once as its reservation and once as background.
- Admission counts each session that holds leases at the larger of its reservations and what
  it uses (its 10 s average cores and its RAM), so a job running hotter than it reserved holds
  back new grants, and background load is the machine's 10 s average less those sessions' own.
  For its first 15 s, a lease that history lowered counts at its declared cores. Queued reasons
  name the lease: "3.5 more in use by lease 1", "7.6 more held by lease 1 for its first 15 s".
- Leases carry `declared_cores` when history lowered their cores.

### Fixed

- Windows: a session's CPU load now counts processes that start and exit between two samples,
  and the last stretch of those that exit. allotd puts each session's `claude.exe` in a Job
  Object without limits and credits the job's CPU time that no sampled process accounts for to
  the session. A clean `cargo build --workspace` measured 74 to 77% of its CPU time in its
  session before and 100% after; the rest had counted as load from outside any lease.

## [0.4.0] - 2026-11-15

### Added

- One-line install: `install.sh` for Linux and macOS and `install.ps1` for Windows download the
  release package for the machine, check it against its `.sha256`, and run `allot install` (on
  Windows, the setup, silently). They take a version to pin and stop with a clear message on a
  platform without a package.
- `allot doctor` checks the binaries, `PATH`, the config, allotd and its version, and what
  `allot install` registers, and says what to fix. It changes nothing.
- `allot status --watch [--interval SECS]` redraws the status until Ctrl-C.
- `--json` on `status`, `leases`, `history`, `kill`, `remove`, `daemon status` and `doctor`. It
  prints allotd's own responses, with JSON Schemas in `contract/`.
- `allot completions <bash|zsh|fish|powershell|elvish>` and `allot man`. Release packages carry
  the scripts and `allot.1`, and `allot install` sets up bash and fish completions.
- Colour in `status`, `leases`, `history` and `doctor`, with `--color auto|always|never`. It
  follows `NO_COLOR` and `CLICOLOR_FORCE` and is off when piped.
- `--help` shows examples.
- The Windows setup puts `%USERPROFILE%\.allot\bin` on the user `PATH`, so `allot` runs in any
  new terminal, and its uninstaller takes it off again.
- npm: `npm install -g @meh7an/allot`, then `allot install`. npm installs only the binaries for
  the machine, with nothing downloaded or run at install time, and `allot` reminds you to run
  `allot install` until `~/.allot/bin` matches the package. `allot doctor` accepts npm's
  launcher on `PATH`.
- crates.io: `cargo install --locked allot-cli allotd allot-mcp`, or the same three names with
  `cargo binstall` for the release binaries, then `allot install`. A missing binary is reported
  with that line, and `allot doctor` accepts cargo's identical copy on `PATH`.
- A lease can start with fewer cores than it asked for. When not all of them are free, allotd
  grants the free cores down to a minimum instead of queueing the lease, and its reasons say,
  for example, "granted 5.6 of 8.0 cores". `acquire_lease` and `can_fit` take `min_cores`, and
  each class and preset in `config.toml` has its own `min_cores` (by default half its cores, at
  least 1). RAM, GPU and VRAM never shrink, and a benchmark always waits for its full request.
  Leases carry `asked_cores` and `min_cores`, `allot status` shows "5.6 of 8.0 cores",
  `allot classes` shows each minimum, and the `resource-guard` skill says when to set one.

### Changed

- Errors fit on one line, and a `hint:` line says what to run next when allotd is not running
  or a lease ID is wrong.
- A lease queued for cores says who holds them, for example "cores: 8.2 requested, 0.0 free
  (8.0 held by 1 lease, 5.6 in use outside leases)".

### Fixed

- On a Windows without the Visual C++ Redistributable, `allot.exe` could not start
  (VCRUNTIME140.dll was not found), so the setup installed nothing while reporting success. The
  Windows binaries now carry the C runtime themselves.
- On Windows, a session could have every lease expire the moment it was made ("holder process
  exited") while its Claude Code was running, and the hook then blocked its builds and tests.
  allotd read the session's start time as 0 when it could not open the process the first time it
  saw it, and never read it again. It now reads such start times again on every sample.
- `pnpm test:engines`, `pnpm build:css` and other `pnpm build*` and `pnpm test*` scripts are now
  heavy work, so the hook binds them to their lease. Before, such a lease could end as idle after
  180 s while its tests still ran.

## [0.3.0] - 2026-11-04

Allot now runs on Linux and macOS too, with a native panel on each, and is called Allot instead
of Resources Helper. Windows keeps its setup, tray app and behavior.

### Added

- Linux: x86_64 and aarch64 tarballs for glibc 2.35 and newer. `allot install` sets up a systemd
  user unit and the `allot` icon. Each lease's command runs in a cgroup v2 scope of its own where
  systemd allows it, which gives exact usage and an atomic kill, with optional CPU and memory
  limits; otherwise leases follow the process tree. GPU figures come from NVML, DRM fdinfo, and
  amdgpu sysfs.
- Linux panels, as separate packages: a GNOME Shell extension (48 to 51, tested on 50) and a KDE
  Plasma 6 widget (tested on 6.8), with the Windows flyout's job cards, Kill and Remove, Resources
  card, first run, and notifications.
- macOS on Apple Silicon, 14 or newer: a Homebrew tap, `meh7an/allot`, with a formula for the
  binaries and the `allot-app` cask for the menu bar app, also as release assets. `allot install`
  sets up the LaunchAgent `tech.mehran.allotd`. The app is signed ad hoc, so macOS asks once per
  version (see the README). GPU figures come from IOKit, and video memory counts against RAM.
- Keyboard use, screen readers, text scaling, and high contrast in the three new panels.
- `acquire_lease` tells a session when no lease ending would make room for its request, so it
  says so instead of waiting silently.
- The Windows tray says when Windows has Allot's toasts turned off, with a link to the setting.

### Changed

- On macOS, while memory pressure is normal, admission counts only wired memory as load, so a
  local model holding most of RAM no longer keeps every lease queued.
- `allot --version` prints `allot`, not the package name `allot-cli`.
- The Windows tray icon shows play, benchmark, and clock badges.
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

[Unreleased]: https://github.com/meh7an/allot/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/meh7an/allot/compare/v0.6.0...v0.6.1
[0.6.0]: https://github.com/meh7an/allot/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/meh7an/allot/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/meh7an/allot/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/meh7an/allot/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/meh7an/allot/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/meh7an/allot/releases/tag/v0.1.0
