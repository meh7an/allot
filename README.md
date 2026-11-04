<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/png/allot-mark-color-dark-2x.png">
    <img src="assets/brand/png/allot-mark-color-light-2x.png" alt="" width="40" height="40" align="absmiddle">
  </picture>
  Allot
</h1>

[![CI](https://github.com/meh7an/allot/actions/workflows/ci.yml/badge.svg)](https://github.com/meh7an/allot/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/meh7an/allot)](https://github.com/meh7an/allot/releases/latest)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
![Platform: Windows, Linux and macOS](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-0078D6)

Coordinates CPU and GPU heavy work between the Claude Code sessions on one machine. Sessions
ask a per-user daemon for a lease before a build, test suite, benchmark, or training run, and
wait their turn when the machine is busy, instead of all compiling at once. A tray app shows
every session's jobs and lets you stop one; the session is told and asks you what to do next.

## Why

Run three or four Claude Code sessions on one machine and they will, sooner or later, all start
`cargo build`, a test suite, or a training run in the same minute. Each job then takes several
times longer, RAM runs out, tool calls time out, and any timing a session measures is noise.
None of the sessions can see the others.

Allot gives them a shared view of the machine and a ledger of who is using what. A session
reserves the cores, RAM, GPU, and VRAM a job needs before running it; when they are not
free, it waits in a queue and is told what is ahead and roughly how long.

## How it works

```mermaid
flowchart LR
    subgraph session["Each Claude Code session"]
        skill["resource-guard skill"]
        mcp["allot-mcp<br/>(MCP server)"]
        hook["allot hook pre-tool / post-tool<br/>(hooks)"]
    end
    tray["allot-tray<br/>(tray app)"]
    allotd["allotd daemon<br/>127.0.0.1 + token"]
    db[("SQLite<br/>leases, history")]
    os["Windows PDH, DXGI,<br/>process table"]

    skill -- "acquire_lease / release_lease" --> mcp
    mcp -- "HTTP + heartbeat" --> allotd
    hook -- "covering lease? stopped jobs?" --> allotd
    tray -- "state, kill, remove" --> allotd
    allotd --- db
    os -- "samples every second" --> allotd
```

| Part | What it does |
|------|--------------|
| `allotd` | The daemon: samples the machine every second, keeps the lease ledger in SQLite, and serves an HTTP API on `127.0.0.1` with a bearer token. No window; it logs to a file. |
| `allot-mcp` | The MCP server each session starts (`allot`): `system_snapshot`, `can_fit`, `acquire_lease`, `release_lease`, `list_leases`. It heartbeats its leases and releases them when the session ends. |
| `allot hook pre-tool` | A PreToolUse hook for the Bash and PowerShell tools. It denies a heavy command when the session holds no covering lease, and lets it through with a warning when allotd is down. |
| `allot hook post-tool` | A PostToolUse and PostToolUseFailure hook that tells the session when you stopped its job. |
| `allot-tray` | The tray app: an icon that shows how busy the machine is, and a flyout with every session's jobs, Kill and Remove, resource meters, and toasts. Only a client of allotd. |
| Menu bar app, GNOME extension, KDE widget | The same for macOS and Linux desktops, also only clients of allotd. |
| `resource-guard` skill | Tells Claude when and how to lease. A CLAUDE.md rule points sessions to it. |
| `allot` | The CLI: status, leases, history, kill, remove, daemon control, install. |

What the daemon does with a request:

- **Admission.** A lease is granted when its profile fits in what the machine has free, keeping
  a headroom (15% by default). Otherwise it queues; smaller jobs may backfill ahead of a
  queued one only when that does not delay it, so nothing starves.
- **Learned profiles.** Every released lease records what the job really used. The next run
  of the same command in the same folder is estimated from that history, not only from what
  the session declared.
- **Benchmarks.** A `benchmark_exclusive` lease drains the machine and waits for it to stay
  quiet before it is granted, so measurements are not disturbed. The background noise during
  the run is recorded on release.
- **Liveness.** Leases are kept alive by heartbeats and tied to the session's process. A
  session that ends or crashes loses its leases within a second or two, so they never leak.
- **Fail-open.** If allotd is down, the hook lets commands run with a warning instead of
  blocking every shell call.

Runs on Windows, Linux, and macOS. Everything lives in `~/.allot`, which on Windows is
`%USERPROFILE%\.allot\`, not under `AppData`, because the Claude desktop app is MSIX-packaged and Windows redirects its writes to `AppData`
into a folder that terminal sessions cannot see.

## Install

### Windows

Needs Windows 10 or 11 (x64) and Claude Code. No admin rights: everything is installed for
your user only.

1. Download `allot-<version>-setup.exe` from the
   [latest release](https://github.com/meh7an/allot/releases/latest) and run
   it. Close the Claude sessions you have open first if you can: the setup edits their
   settings and CLAUDE.md. The setup is not code-signed, so SmartScreen may ask you to confirm
   (More info, Run anyway).
2. Keep both choices on the Select Additional Tasks page, unless you want to set up Claude
   Code yourself or start the tray app by hand.
3. Start a new Claude session (sessions load MCP servers and CLAUDE.md only when they start).

The setup:

- puts `allot.exe`, `allotd.exe`, and `allot-mcp.exe` in `%USERPROFILE%\.allot\bin`,
  restarts allotd on them when it is running, and registers the `allot` scheduled task
  that starts allotd when you sign in (no time limit, also on battery);
- puts the tray app in `bin\tray`, starts it, sets its Launch at logon, and adds it to the
  Start menu;
- copies the `resource-guard` skill to `~/.claude/skills/resource-guard/`;
- registers the MCP server at user scope, with `claude mcp add` when `claude` is on `PATH` and
  otherwise in `~/.claude.json` directly;
- adds the PreToolUse, PostToolUse, and PostToolUseFailure hooks to `~/.claude/settings.json`;
- writes the rule to `~/.claude/allot.md` and imports it into `~/.claude/CLAUDE.md`
  with the line `@allot.md`. A copy of the rule pasted by hand before is replaced
  by that line;
- adds Allot to Apps & features, for uninstalling.

A file is only rewritten when something in it changes, and the old one is kept beside it as
`<name>.allot-backup`. What the setup did is in `%LOCALAPPDATA%\Programs\Allot\install.log`.

Check it with:

```powershell
~\.allot\bin\allot.exe status
claude mcp get allot
```

To type `allot` anywhere, add `%USERPROFILE%\.allot\bin` to your user `PATH`
(optional; the examples below assume it).

### Windows without the setup

The release also has `allot-<version>-x86_64-pc-windows-msvc.zip`. Extract it and
run `allot.exe install` from that folder: it does everything the setup does except the Start
menu and Apps & features entries, and prints `[done]` for each step, or `[todo]` with what to
do by hand when a step could not be done. `allot install --help` lists its options
(`--no-claude`, `--no-logon-task`, `--launch-at-logon on|off`).

To build from source you need Rust (stable, MSVC toolchain, 1.99 or newer), and for the tray
app the .NET 10 SDK and Visual Studio's C++ build tools (the Native AOT linker). Inside a
Claude session each build is a `cpu_heavy` lease like any other.

```powershell
cargo build --release
dotnet publish tray\src\Allot.Tray -c Release -o target\release\tray
target\release\allot.exe install
```

The publish finds the linker through `vswhere.exe`; if it cannot, add
`C:\Program Files (x86)\Microsoft Visual Studio\Installer` to `PATH`. The setup itself is
built from `installer\allot.iss` with Inno Setup 7 (see the script's header).

### Linux

Needs x86_64 or aarch64, glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and later), and
Claude Code. systemd starts allotd at login and gives each job a cgroup of its own; without a
user manager the install says what it could not do, and leases follow the job's process tree.
No root needed. Linux packages are published from v0.3.0 on.

```bash
version=0.3.0
target=x86_64-unknown-linux-gnu    # or aarch64-unknown-linux-gnu
base=https://github.com/meh7an/allot/releases/download/v$version
curl -fLO "$base/allot-$version-$target.tar.gz"
curl -fLO "$base/allot-$version-$target.tar.gz.sha256"
sha256sum -c "allot-$version-$target.tar.gz.sha256"
tar -xzf "allot-$version-$target.tar.gz"
"allot-$version-$target/allot" install
```

Then start a new Claude session. The folder can be deleted afterwards. The install:

- puts `allot`, `allotd`, and `allot-mcp` in `~/.allot/bin`, and links `allot` into
  `~/.local/bin` when that folder exists and has no other `allot`;
- sets up the systemd user unit `allotd.service`, which starts allotd when you log in;
- installs the `allot` icon into `~/.local/share/icons/hicolor`;
- sets up Claude Code the way the Windows setup does (skill, MCP server, hooks, and the rule).

`allot install --help` lists its options (`--no-claude`, `--no-autostart`), and it prints `[done]`
for each step, or `[todo]` with what to do by hand. Run a newer package's `allot install` the same
way to upgrade.

#### The panel

The desktop part is a separate package, because a desktop only loads it from its own folders.
Both are in the release, with a `.sha256` each, and work on either architecture.

On GNOME Shell (48 to 51; tested on 50):

```bash
gnome-extensions install --force "allot-$version-gnome-extension.zip"
```

Log out and in, since the shell only finds a new extension when it starts, and then:

```bash
gnome-extensions enable allot@mehran.tech
```

On KDE Plasma 6 (tested on 6.8), then add Allot to a panel with Add Widgets:

```bash
kpackagetool6 -t Plasma/Applet -i "allot-$version-kde-widget.plasmoid"
```

To upgrade the widget, use `-u` in place of `-i` and restart Plasma with
`systemctl --user restart plasma-plasmashell`. Other desktops have no panel yet; `allot status`
shows the same jobs and `allot kill ID` stops one.

#### From source

You need Rust (stable, 1.99 or newer) and a C compiler for the bundled SQLite. Inside a Claude
session each build is a `cpu_heavy` lease like any other.

```bash
cargo build --release
target/release/allot install
```

`gnome-extensions install` wants a zip, so zip `apps/gnome/allot@mehran.tech` from inside the
folder first; `kpackagetool6 -i apps/kde/tech.mehran.allot` takes the folder as it is.

### macOS

Needs Apple Silicon, macOS 14 or newer, Homebrew, and Claude Code. No admin rights. macOS packages
are published from v0.3.0 on.

```bash
brew tap meh7an/allot
brew trust --tap meh7an/allot
brew install allot
allot install
```

Homebrew only loads a third-party tap you have trusted. Then start a new Claude session. The
install:

- copies `allot`, `allotd`, and `allot-mcp` to `~/.allot/bin`, and links `allot` into
  `~/.local/bin` when that folder exists and has no other `allot`;
- sets up the LaunchAgent `tech.mehran.allotd`, which starts allotd when you log in;
- sets up Claude Code the way the Windows setup does (skill, MCP server, hooks, and the rule).

`allot install --help` lists its options (`--no-claude`, `--no-autostart`).

#### The menu bar app

```bash
brew install --cask allot-app
open /Applications/Allot.app
```

The app is signed ad hoc, not with an Apple Developer ID, so macOS stops it once: it says Apple
could not verify Allot, and System Settings > Privacy & Security then offers Open Anyway. A new
version asks again. The app also needs the install above, since it only talks to allotd.

#### Without Homebrew

The release has `allot-<version>-aarch64-apple-darwin.tar.gz` and
`allot-<version>-macos-app.zip`, with a `.sha256` each. Check them with `shasum -a 256 -c`, run
`./allot install` from the unpacked tarball, and move `Allot.app` to Applications; from any other
folder macOS runs a copy it moved aside, which cannot launch at login.

#### From source

You need Rust (stable, 1.99 or newer) and Xcode's command line tools. The app also needs Xcode and
[XcodeGen](https://github.com/yonaskolb/XcodeGen).

```bash
cargo build --release
target/release/allot install
cd apps/macos && xcodegen generate
xcodebuild -project Allot.xcodeproj -scheme Allot -configuration Release -derivedDataPath build build
```

## Usage

Once installed there is nothing to call by hand: the skill and the CLAUDE.md rule make each
session lease before a heavy command, and the hook stops the ones that forget. A typical
exchange inside a session:

1. Claude is about to run `cargo test --workspace`, so it calls `acquire_lease` with the class
   `cpu_heavy` and the command as label.
2. The machine is busy with another session's build, so the lease comes back `queued` with what
   is ahead and an estimated wait. Claude tells you in one line and waits.
3. The other build's lease is released; this one is granted, the tests run, and Claude calls
   `release_lease`.

The classes are `light`, `cpu_heavy`, `gpu_heavy`, and `mixed`; timing work adds the lease
kind `benchmark_exclusive`. `allot classes` prints the class profiles and the rules that decide
which commands count as heavy.

### Everyday commands

| Command | Shows or does |
|---------|---------------|
| `allot status` | Machine usage, sessions, and their leases. Starts allotd if it is down. |
| `allot leases [--all]` | Active and queued leases, or the most recent ones in any state. |
| `allot history [FINGERPRINT]` | What past jobs needed, or the runs of one job. |
| `allot classes` | The heavy-command rules, class profiles, and presets in effect. |
| `allot kill ID` / `allot remove ID` | Stop a running job, or take a queued one out of the queue, as the tray app does. |
| `allot daemon start\|stop\|status` | Control allotd. `status` never starts it. |

### The tray app

The icon in the notification area shows whether the machine is idle, busy, or has jobs
waiting. Click it for the flyout:

- **Jobs.** Each running or queued job with its process, command, class, time, and estimate;
  a bar fills toward the estimate. Finished and stopped jobs stay for a moment with how they
  ended.
- **Kill** (running jobs) and **Remove** (queued ones) ask once more (Kill?) before they act.
  Kill ends only the process tree the lease's command started: never Claude Code itself,
  allotd, allot, allot-mcp, or a process older than the command. The lease ends and its session is
  told that you stopped the job, so Claude says so and asks you whether to retry or skip; it
  never retries on its own.
- **Benchmarks** show their phase: draining the machine, waiting for quiet, running alone.
- **Resources** (More options, Meters) shows CPU, RAM, GPU, VRAM, and drives, with what is
  used, reserved by leases, and free.
- **Toasts** for a stopped service, a job running well past its estimate, finished jobs, and
  jobs stopped from outside the flyout; each can be turned off under More options.
- **Stay open** and **Pin** keep the flyout up; a pinned flyout can be dragged.

The tray menu starts and stops allotd and turns Launch at logon on or off (it is also listed in
Task Manager's Startup apps as Allot). Stop service only lasts until a session's
`allot-mcp` needs allotd again and starts it. The tray app keeps its settings in `tray.json` and
logs to `logs\tray.log`.

## Configuration

`%USERPROFILE%\.allot\config.toml` is optional. Each table overrides the defaults
field by field, unknown keys are rejected, and allotd reloads the file within 5 s. The common
keys:

```toml
[admission]
headroom_pct = 15          # capacity kept free of leases

[hook]
fail_open = true           # let commands run when allotd is down

[benchmark]
max_quiet_wait_secs = 60   # grant a benchmark without quiet after this long
```

Every key, the class profiles, and the classifier rules are in the
[runbook](docs/runbook.md#configuration).

## Upgrade

Run the new setup over the old install (or `allot.exe install` from a new zip or build). Binaries
that running sessions hold open cannot be overwritten, so the old file is renamed aside
(`allot-mcp.old-<millis>.exe`) and the new one copied in. Old copies that are still in use are
deleted by the next install. allotd restarts on the new build and restores its active and queued
leases. Open sessions keep their old `allot-mcp` until they restart. The hook picks up the new
`allot.exe` at once.

The tray app is asked to quit, replaced, and started again. The setup remembers your choices
from the last time; `allot install` without `--launch-at-logon` keeps Launch at logon as you set
it. The rule in `~/.claude/allot.md` is rewritten, and CLAUDE.md keeps its import.

Allot used to be called Resources Helper. Its setup, or `allot install`, takes over a
Resources Helper install: it copies the database, token, config, tray settings, and logs from
`%USERPROFILE%\.resources-helper\` into `%USERPROFILE%\.allot\`, replaces the logon task, the
tray app's Launch at logon, the Start menu entry, the MCP server (now `allot`), the hooks, and
the CLAUDE.md import (now `@allot.md`), and removes the old binaries. allotd keeps the old port
and token, so sessions that were open keep their old MCP server working until they restart.
The old folder is deleted by a later install once nothing runs from it.

On macOS, Homebrew updates its own copy only. `allot install` is what puts the new binaries in
`~/.allot/bin`, where sessions and allotd run them from:

```bash
brew upgrade allot
allot install
brew upgrade --cask allot-app
```

## Uninstall

Uninstall Allot in Settings > Apps > Installed apps. It asks whether to delete your
data too, then quits and removes the tray app with its Launch at logon entry and Start menu
shortcut, removes the MCP server, the hooks, the CLAUDE.md import, and the scheduled task,
stops allotd, and removes the binaries and the skill copy. A copy of the rule pasted into
CLAUDE.md by hand is left for you to delete.

Without the setup, run `allot.exe uninstall` from the extracted release or the build output, not
from the bin directory (a running exe cannot delete itself). It does the same and prints what
it could not do. The database, token, config, tray settings, and logs stay unless you choose
to delete them, or pass `--purge`.

On Linux, remove the panel package first, since the KDE widget restores its notifications file
when it starts, and then run `allot uninstall`. It takes away the unit, the binaries, the link,
the skill copy, the icon, and that file, and keeps the data unless you pass `--purge`.

```bash
gnome-extensions uninstall allot@mehran.tech
kpackagetool6 -t Plasma/Applet -r tech.mehran.allot
allot uninstall
```

On macOS, run `allot uninstall` before Homebrew takes the `allot` command away. It removes the
LaunchAgent, the binaries, the link, and the skill copy, and keeps the data unless you pass
`--purge`.

```bash
allot uninstall
brew uninstall --cask allot-app
brew uninstall allot
```

## Files

All under `%USERPROFILE%\.allot\`, or `~/.allot` on Linux and macOS, which have no `bin/tray/` and
keep the panel's settings in `tray.json` too:

| Path | Contents |
|------|----------|
| `bin\` | `allot.exe`, `allotd.exe`, `allot-mcp.exe` |
| `bin\tray\` | The tray app, `allot-tray.exe` and its runtime. |
| `config.toml` | Optional overrides of the defaults. allotd reloads it within 5 s. |
| `allot.db` | Leases, their audit trail, and job history (SQLite, WAL). |
| `token` | The bearer token clients send to allotd. |
| `tray.json` | The tray app's settings: meters, toasts, first run. |
| `logs\allotd.log` | The daemon log, rotated at 10 MB, five files kept. |
| `logs\tray.log` | The tray app's log. |

In `~/.claude/`, the install writes `skills/resource-guard/SKILL.md` and `allot.md`
(the rule CLAUDE.md imports), and adds its entries to `settings.json` and `CLAUDE.md`.

## Troubleshooting

Health checks, fail-open behavior, crash recovery, and resetting the database are in the
[runbook](docs/runbook.md). The quickest checks:

- `allot daemon status` tells you whether allotd is up, its version, and its port.
- A heavy command "Blocked by allot" in a session without an `acquire_lease` tool
  means the session started before the MCP server was registered; start a new one.
- Every heavy command running with a "runs unchecked" warning means allotd is down; read the end
  of `logs\allotd.log`.
- No tray icon after an install: `allot install` says why. Another copy of the tray app (such as
  a development build) may hold its place; quit it from its tray menu and start
  `bin\tray\allot-tray.exe`.

## Platform support

Windows 10 and 11 on x86_64: the sampler reads Windows performance counters (PDH) and DXGI for
vendor-neutral GPU and VRAM usage, autostart uses a Windows logon task and the Run key, and the
tray app is a WinUI 3 app. GPU numbers cover one adapter.

Linux on x86_64 and aarch64 with glibc 2.35 or newer: the sampler reads `/proc` and sysfs, and
NVML or DRM for the GPU, autostart is a systemd user unit, and jobs run in cgroup v2 scopes where
systemd allows it. The panel is a GNOME Shell extension or a KDE Plasma 6 widget.

macOS 14 or newer on Apple Silicon: the sampler reads the Mach VM statistics and memory pressure
level, and IOKit for the GPU and drives (per-process GPU figures are estimates), autostart is a
LaunchAgent, and the panel is a menu bar app. Memory is shared with the GPU, so video memory
counts against RAM. The app is signed ad hoc.

## Contributing

Issues and pull requests are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md). Report security
issues privately as described in [SECURITY.md](SECURITY.md). Changes are listed in
[CHANGELOG.md](CHANGELOG.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
