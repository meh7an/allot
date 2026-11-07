# Allot runbook

The sections without an OS in their name use Windows paths, relative to `%USERPROFILE%\.allot\`
unless they start with `~` or a drive, and `allot` means `bin\allot.exe`. On Linux and macOS the
same files are in `~/.allot`; the Linux and macOS sections cover what differs.

## Is it working?

| Check | Healthy result |
|-------|----------------|
| `allot daemon status` | `allotd <version> running at http://127.0.0.1:47823 (pid ..., up ... s)` |
| `allot status` | Machine usage and one block per session. A `config REJECTED` line means allotd kept its previous config (see below). |
| `claude mcp get allot` | `Connected`, command `...\.allot\bin\allot-mcp.exe` |
| `Get-ScheduledTask allot` | State `Ready` (allotd already started) or `Running` |
| `Get-ScheduledTaskInfo allot` | `LastTaskResult` 267009 (still running). 1 means allotd exited at once: usually another allotd was already running, which is not logged (it fails before logging starts, so two daemons never share the log); other startup errors are in `logs\allotd.log`. |
| `logs\allotd.log` | A startup line, then lease grants and releases |
| The tray icon | The Allot mark in the notification area (it may sit in the overflow); its tooltip summarizes the jobs. While allotd is down it shows the down icon, and the flyout a "Service stopped" banner with Start service. |
| `Get-ItemProperty HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name 'Allot'` | The quoted path of `bin\tray\allot-tray.exe`, while Launch at logon is on |
| `logs\tray.log` | A `started` line per launch, then the flyout and tray events |

`allotd.exe` is a windowless program: run from a terminal it prints nothing, not even
`--help`. Use `allot daemon status` for its version and `logs\allotd.log` for its errors.

## Who starts allotd

- The logon task `allot`, which the install registers, starts it when you log on.
- `allot-mcp` (on a tool call or a heartbeat) and `allot status` start it when it is down,
  using the `allotd.exe` beside their own executable.
- The hook never starts it, so a heavy command never waits on a daemon start.
- `allot daemon start` starts it by hand; `allot daemon stop` stops it gracefully. The tray
  app's Start service and Stop service do the same.

The tray app itself starts at logon from the Run key value `Allot` (Launch at logon in its
tray menu, or Startup apps in Task Manager). `allot install` turns it on the first time it
installs the tray app and leaves it as you set it afterwards. Launching
`bin\tray\allot-tray.exe` while it runs opens the flyout; `allot-tray.exe --quit` asks the
running app to quit.

## Linux

Paths in this section start at your home folder, and the data folder is `~/.allot`.

The systemd user unit `allotd.service` does the logon task's job. `allot install` writes it to
`~/.config/systemd/user/`, enables it, and starts it; `allot uninstall` removes it, but only when
it starts this install's `allotd`.

```bash
systemctl --user status allotd
journalctl --user -u allotd
```

- `allot-mcp`, `allot status`, and `allot daemon start` start allotd through the unit when the
  unit runs the same file, and directly otherwise.
- The unit has no restart policy, like the logon task: the next client starts a crashed allotd.
- allotd stops at your last logout. `loginctl enable-linger` keeps it running after that.
- `allotd.log` is in `~/.allot/logs/`, the same as on Windows.

| Symptom | Cause and fix |
|---------|---------------|
| `[todo] allotd at login` | No systemd user session, such as WSL without systemd. The line prints the commands. On WSL, set `systemd=true` under `[boot]` in `/etc/wsl.conf` and restart WSL. Clients still start allotd without the unit. |
| `systemctl --user list-units 'allot-*.scope'` lists nothing while a job runs | `linux.scopes` needs cgroup v2 and a systemd user session; without them leases follow the process tree. See Configuration. |
| `allot: command not found` | `~/.local/bin` is not on `PATH`, or it did not exist during the install. Add `~/.allot/bin` to `PATH`, or create the folder and install again. |
| `version GLIBC_2.xx not found` | The system's glibc is older than 2.35. Build from source there. |
| The GNOME extension is not listed after installing it | The shell only finds a new extension when it starts: log out and in, then `gnome-extensions enable allot@mehran.tech`. |
| The GNOME extension shows an error | `gnome-extensions info allot@mehran.tech` shows its state, and `journalctl --user -b /usr/bin/gnome-shell \| grep -i allot` the reason. |
| The KDE widget is not in Add Widgets, or does not change after an upgrade | Plasma reads packages when it starts: `systemctl --user restart plasma-plasmashell`. |
| The KDE widget misbehaves | `journalctl --user -b -u plasma-plasmashell \| grep -i allot`. |
| The widget or its notifications have a generic icon | The `allot` icon comes from `allot install`. Install, then restart Plasma. |

To remove everything, take the panel package away first, since the KDE widget puts its
notifications file back each time it starts:

```bash
gnome-extensions uninstall allot@mehran.tech
kpackagetool6 -t Plasma/Applet -r tech.mehran.allot
allot uninstall
```

`allot uninstall` removes the `allot` icon and `~/.local/share/knotifications6/allot.notifyrc`;
`--purge` also removes `~/.allot` and `~/.config/allot.notifyrc`, your notification choices.

## macOS

Paths in this section start at your home folder, and the data folder is `~/.allot`.

The LaunchAgent `tech.mehran.allotd` does the logon task's job. `allot install` writes it to
`~/Library/LaunchAgents/` and loads it; `allot uninstall` removes it. Homebrew has no service for
Allot, so `brew services` does not list it.

```bash
launchctl print "gui/$(id -u)/tech.mehran.allotd"
launchctl kickstart "gui/$(id -u)/tech.mehran.allotd"
```

- It has no restart policy, like the logon task: the next client starts a crashed allotd. The
  menu bar app's Start Service uses the agent when it is there.
- `allotd.log` is in `~/.allot/logs/`, the same as on Windows.
- Homebrew's copy of the binaries is only what `allot install` copies from, so after
  `brew upgrade allot` run `allot install`. Sessions run from `~/.allot/bin`.

While the kernel reports normal memory pressure, admission counts only wired memory as load, plus
what leases reserved. macOS keeps memory full and compresses or swaps the rest on demand. At the
warning or critical level it counts everything Activity Monitor calls Memory Used. So the
Resources card's RAM bar can run past its headroom while jobs still start. Wired memory includes
a model a local LLM server has loaded. A running GPU lease's memory can count twice, as wired and
as reserved, so other leases wait more than they need to until it ends.

| Symptom | Cause and fix |
|---------|---------------|
| `brew` refuses the tap, formula, or cask as untrusted | `brew trust --tap meh7an/allot`. |
| "Apple could not verify Allot" | The app is signed ad hoc. System Settings > Privacy & Security > Open Anyway, once per version. |
| The app says to move it to Applications | It runs from a copy macOS moved aside, which cannot launch at login. Move it to `/Applications`. |
| Login Items lists an unidentified `allotd` | That is the LaunchAgent. Turning it off stops allotd starting at login; clients still start it when needed. |
| The app shows the service down right after installing | The cask does not run the install. Run `allot install`. |
| A session says no lease ending would make room | Wired memory or the request's own size is in the way; `allot status` shows the memory. Unload the local model, or lower the request. |

To remove everything:

```bash
allot uninstall
brew uninstall --cask allot-app
brew uninstall allot
```

`--purge` on the first command also removes `~/.allot`.

## Stopping a job

Kill (in the flyout, or `allot kill <id>`) works on an active lease, whichever session holds it:

1. allotd looks up the process tree the lease is bound to: the first new child of the session's
   `claude.exe` after the hook reported the command.
2. It refuses, and the lease stays, when that root is protected (Claude Code and its
   ancestors, `allotd`, `allot`, `allot-mcp`) or started before the command was reported.
3. Otherwise the lease ends as killed, and the tree is ended root first. When nothing is bound
   or the root has already exited, the lease still ends and nothing is terminated; the
   response says so.

On Linux with `linux.scopes` on, a lease's command also runs in its own systemd scope, and Kill
ends every process in that scope at once (`cgroup.kill`), then anything of the tree that started
before it moved into the scope. A scope that holds a nested Claude session or an allot binary is
left to the tree kill, which spares them. Releasing a lease never ends what still runs in its
scope, such as a job started in the background.

Remove (`allot remove <id>`) ends a queued lease before it starts. Either way the session gets
a notice through its next hook call or MCP tool result, and Claude tells you and asks whether
to retry or skip. The notice is delivered once.

Only one allotd runs per user: a second one finds `allotd.lock` taken and exits. Stopping
allotd while sessions are open is temporary: a session holding a lease starts it again on its next
heartbeat (within about 20 s), and any session does on its next tool call.

## Fail-open behavior

The hook asks allotd for the session's leases with a 1 s budget. When that fails (allotd down,
starting, or answering with an error):

- `hook.fail_open = true` (default): the command runs, and the session shows the warning
  "allotd is unavailable, `<command>` runs unchecked: <reason>". Nothing is coordinated until
  allotd is back.
- `hook.fail_open = false`: the command is denied, and Claude is told to ask you to run
  `allot daemon start`.

The hook also never blocks on its own failures: unreadable input, an invalid config, or
wrong arguments produce a warning and let the command through. It always exits 0, so a
broken `allot.exe` cannot block every shell call.

## Troubleshooting

| Symptom | Cause and fix |
|---------|---------------|
| A session's heavy command is "Blocked by allot", but the session has no `acquire_lease` tool | The session started before the MCP server was registered, or with `--strict-mcp-config`. Start a new session. |
| Every heavy command runs with the "runs unchecked" warning | allotd is down. `allot daemon status`, then read the end of `logs\allotd.log`. |
| allotd exits right after starting | `logs\allotd.log` says why. "another allotd is already running": one is up already. "binding 127.0.0.1:<port>": another program has the port, so set `[daemon] port` in `config.toml`. A config error names the file and key. |
| `config REJECTED` in `allot status` | `config.toml` has an invalid value; allotd keeps the previous config until the file is valid again (checked every 5 s). The message names the key. |
| Nothing can reach allotd after a port change | allotd keeps its old port until it restarts, but clients read the new one at once, `allot daemon stop` included. End `allotd.exe` in Task Manager, then run `allot daemon start`. |
| 401 errors | The token in `token` does not match allotd's. Clients re-read the file once. If it persists, restart allotd: it reads `token` at startup. |
| A lease stays queued | `allot leases` shows what is ahead. A `benchmark_exclusive` lease drains the machine and then waits up to `benchmark.max_quiet_wait_secs` for quiet. Leases of sessions that ended expire within a second (holder process gone) or after the heartbeat TTL (60 s). |
| `allot install` lists files "still in use" | Old binaries that running sessions or allotd still hold. They are harmless and are deleted by the next install once nothing uses them. |
| `%USERPROFILE%\.resources-helper\` is still there after the upgrade from Resources Helper | Sessions that were open during the upgrade still run its `rh-mcp.exe`, which reads its `token` and `config.toml`. Restart them and run `allot install` again; it deletes the folder once nothing runs from it. A "kept ... exists already" line means the new folder already had that file, so the old one was left for you to compare. |
| A session started before the upgrade says its tools come from `resources-helper` | That is its old MCP server, which works against the new allotd until the session restarts. New sessions get the `allot` server. |
| Copying a build over `bin\allot-mcp.exe` fails | Running sessions hold it open. Use `allot install`, which renames the old file aside first. |
| No tray icon after `allot install` | The install prints why. "exited at once" means another copy of the tray app holds its single-instance place, such as a development build: quit it from its tray menu, then start `bin\tray\allot-tray.exe`. Otherwise read `logs\tray.log`. |
| The flyout says "Service needs an update · run allot install" | allotd is older than the tray app. Run `allot install` from the same release. |
| The tray app does not start at logon | Launch at logon is off in its tray menu, or disabled in Task Manager's Startup apps. Turning it on in the tray menu clears Task Manager's mark. |
| Kill shows a red refusal | The bound process is protected or older than the command (see Stopping a job). The lease keeps running; end the process yourself if you must. |
| The setup says Allot could not be installed | `%LOCALAPPDATA%\Programs\Allot\install.log` holds the output of `allot install`, which names the step and the error. Running the setup again is safe. |
| The install log says "claude was not found" for the MCP server | It wrote the entry to `~/.claude.json` itself. To have `claude mcp add` do it, set `ALLOT_CLAUDE` to the full path of `claude.exe` (or `claude.cmd`) and run `allot install` again. |
| `[todo] MCP server` | `claude mcp add` failed, or `~/.claude.json` could not be written; the line says why. Run the printed `claude mcp add` yourself. |
| `[check] Hooks` | `~/.claude/settings.json` is not valid JSON or has an unexpected shape, so it was left alone. Fix the file and install again, or add the printed entries yourself. |
| "also has its own copy of the rule" | CLAUDE.md has a copy pasted by hand and edited since, which the install does not touch. Delete that section; the import brings the current rule. Backups of edited files are beside them as `<name>.allot-backup`. |
| `[todo] allotd at logon` | `schtasks` refused the task; the line says why, and the printed PowerShell registers it by hand. A task named `allot` that starts another `allotd.exe` is replaced by the install, and never removed by the uninstall. |
| Registering the logon task fails with "Access is denied" | Fall back to the per-user Run key, which also needs no admin: `New-ItemProperty -Path HKCU:\Software\Microsoft\Windows\CurrentVersion\Run -Name allot -Value '"C:\Users\<you>\.allot\bin\allotd.exe"'`. |
| Terminal sessions see a different daemon, token, or database than desktop sessions | Something wrote under `%LOCALAPPDATA%` from inside the Claude desktop app, which MSIX redirects to `%LOCALAPPDATA%\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Local\`. Only `%USERPROFILE%\.allot\` is shared. Stale copies in the package folder are safe to delete. |

## Shutdown and crashes

A stop (`allot daemon stop`, `allot install`, `allot uninstall`, or Ctrl+C) answers every open
acquire wait at once: the lease stays queued, and the verdict tells the session to resume it with
`lease_id` once allotd is back. The stop then takes about a second.

Windows ends allotd without a graceful shutdown at logoff and shutdown, and a crash does the
same. Nothing is lost: SQLite runs in WAL mode, and on the next start allotd restores the queued
and active leases it had, keeping their grant times. Leases whose holders are gone expire on
the first sample. A run that spans a restart records no job history.

## Configuration

`config.toml` is optional; each table overrides the defaults field by field, and unknown keys
are rejected. `allot classes` prints the classifier rules and class profiles in effect.

| Key | Default | Meaning |
|-----|---------|---------|
| `daemon.port` | 47823 | Port on 127.0.0.1. A change needs an allotd restart. |
| `admission.headroom_pct` | 15 | Capacity kept free of leases. |
| `benchmark.quiet_cpu_pct`, `quiet_gpu_pct` | 10, 10 | Load a benchmark waits under. |
| `benchmark.quiet_secs` | 10 | How long the machine must stay quiet. |
| `benchmark.max_quiet_wait_secs` | 60 | After this, a benchmark is granted without quiet. |
| `leases.heartbeat_ttl_secs` | 60 | A lease without a heartbeat for this long expires. |
| `retention.lease_days`, `history_days` | 30, 90 | Age at which ended leases and job runs are pruned (hourly). |
| `retention.keep_runs_per_job`, `max_job_runs` | 20, 5000 | Runs kept per job regardless of age, and in total. |
| `hook.fail_open` | true | See Fail-open behavior. |
| `linux.scopes` | true | Linux: run each lease's command in a systemd scope of its own (`allot-<id>-<grant time>-<tag>.scope`), which gives its exact CPU time and memory and a kill that ends it at once. Needs cgroup v2 and a systemd user session; without them leases follow their process tree. |
| `linux.enforce_cpu` | false | Linux: cap a lease's scope at its reserved cores (`CPUQuota`). |
| `linux.enforce_memory` | false | Linux: throttle a lease's scope above its reserved RAM (`MemoryHigh`); it is never killed for memory. |
| `[classes.<class>]` | built in | Profiles of `light`, `cpu_heavy`, `gpu_heavy`, `mixed`. |
| `[[classifier.rules]]` | built in | Heavy-command rules. A list here replaces the default list. |

## Resetting the database

This deletes all leases and job history; learned profiles start over.

1. `allot daemon stop` (and close the sessions that hold leases, or their `allot-mcp` starts
   allotd again).
2. Delete `allot.db`, `allot.db-wal`, and `allot.db-shm`.
3. `allot daemon start`. allotd creates the database and applies the migrations.

Sessions that still hold leases get "unknown lease" on their next heartbeat and drop them.

To reset the token, stop allotd, delete `token`, and start allotd; it writes a new one, and
clients read it on their next request.

## Logs

`logs\allotd.log` rotates to `allotd.1.log` ... `allotd.4.log` at 10 MB. `allot-mcp` writes its
diagnostics to stderr, which Claude Code captures with its other MCP server output. The tray
app writes `logs\tray.log`; More options > Open logs opens the folder.

## Releasing

Publishing the draft release is the one gate: nothing reaches a package manager before it.

1. `cargo xtask version 0.4.0` sets the version in the workspace, the crates' path dependencies,
   `Cargo.lock`, the tray and Mac projects and the Linux snippet in `docs/install.md`, and moves
   `[Unreleased]` in `CHANGELOG.md` to a dated section. It refuses an empty `[Unreleased]` and a
   version that is not newer. GNOME's and KDE's metadata and the Windows installer get the version at packaging.
2. Review the diff, commit it, push `main`, and wait for CI. A test in CI's `check` job fails if
   the places disagree; `cargo xtask version --check` runs the same check locally.
3. `git tag v0.4.0` and `git push origin v0.4.0`. `release.yml` builds every platform and uploads
   to a draft release.
4. Check the draft's assets, then publish it: click Publish, or run
   `gh release edit v0.4.0 --draft=false --latest`. It has to be a person's login, because an
   event caused by a workflow's `GITHUB_TOKEN` starts no workflow.
5. `distribute.yml` starts, using the copy at the tag. Read each job's result with
   `gh run view`.
6. Do what the `Manual stores for v0.4.0` issue lists, then close it.

| Channel | Job | Secret on `meh7an/allot` |
|---------|-----|--------------------------|
| Homebrew tap | `homebrew` | `TAP_DEPLOY_KEY`: private half of a write deploy key on `meh7an/homebrew-allot` |
| winget (`Mehran.Allot`) | `winget` | `WINGET_TOKEN`: classic token of `meh7an` with `public_repo` and `workflow` |
| npm (`@meh7an/allot` and its four platform packages) | `npm` | None; trusted publishing (OIDC), set on each package |
| Manual stores issue | `checklist` | None; the job's own token |

### Re-running a channel

Each channel fails alone, so Re-run failed jobs on the run's page finishes a release. Running a
channel again is safe: the tap is only pushed when it differs and never moves to an older version,
winget gets no pull request for a version it already has or one with a pull request open, npm
skips each package whose version is already published, and the issue is opened once per tag. A prerelease goes to no channel.

A re-run uses the workflow at the tag. To get a later fix to `distribute.yml`, or for a tag that
predates the file (v0.3.0 and older, which publishing could not start), run `main`'s copy by hand:

```bash
gh workflow run distribute.yml -f tag=v0.4.0 -f channel=homebrew
```

### The tap's deploy key

The key never expires. To create it or replace it, with the old one then removed from the tap's
settings:

```bash
ssh-keygen -t ed25519 -N "" -C "allot distribute" -f tap-key
gh repo deploy-key add tap-key.pub --repo meh7an/homebrew-allot --allow-write --title "allot distribute"
gh secret set TAP_DEPLOY_KEY --repo meh7an/allot < tap-key
rm tap-key tap-key.pub
```

To prove the key and the job without a release, push a fake version to a throwaway branch of the
tap, read its diff, and delete the branch. The job refuses a fake version on the tap's `main`.

```bash
gh workflow run distribute.yml -f tag=v0.3.0 -f channel=homebrew -f test_version=0.99.0 -f tap_branch=test/fake-version
```

### The winget token

The `winget` job runs [Komac](https://github.com/russellbanks/Komac), which writes the new
version's manifests from the last one merged in `microsoft/winget-pkgs` and opens the pull
request from the fork `meh7an/winget-pkgs`. The first version, 0.3.0, went in by hand. The pull
request is merged by winget's moderators after its checks pass; one that the bot hands back for
changes closes itself after 8 days without an answer.

Komac cannot open the pull request with a fine-grained token, so `WINGET_TOKEN` is a classic
token (Settings, Developer settings, Personal access tokens, Tokens (classic)) with `public_repo`,
which `komac sync-fork` also needs `workflow` beside. It can write to every public repository of
`meh7an`, so give it an expiry and replace it before then:

```bash
gh secret set WINGET_TOKEN --repo meh7an/allot
```

To prove the token and the job without a pull request, print the manifests Komac would submit:

```bash
gh workflow run distribute.yml -f tag=v0.3.0 -f channel=winget -f winget_dry_run=true
```

### The npm packages

The `npm` job builds five packages from the release's archives, each checked against its
`.sha256`: `@meh7an/allot-win32-x64`, `-linux-x64`, `-linux-arm64` and `-darwin-arm64` with the
binaries (and the tray app on Windows), then `@meh7an/allot` with the launcher, which depends on
the four as optional dependencies. It publishes them in that order, by trusted publishing, so no
npm token is stored; npm adds provenance itself. Every run uploads the tarballs as the artifact
`npm-<tag>`.

npm only lets a package that exists take a trusted publisher, so the first version of each went
up by hand: 0.3.0, from a dry run's artifact. Publish the tarballs CI packed, not ones packed on
Windows, where the binaries would lose their executable bit:

```bash
gh workflow run distribute.yml -f tag=v0.3.0 -f channel=npm -f npm_dry_run=true
gh run download <run id> --repo meh7an/allot --name npm-v0.3.0 --dir npm-v0.3.0
npm login
while read -r name tarball <&3; do
  npm publish "npm-v0.3.0/$tarball" --access public
done 3< npm-v0.3.0/packages.txt
```

Then give each of the five packages its trusted publisher, with 2FA on the account and npm
11.15.0 or newer. A new one expires unless a publish uses it within 2 days, so do this right
before publishing the next release:

```bash
while read -r name tarball <&3; do
  npm trust github "$name" --file distribute.yml --repo meh7an/allot --allow-publish
done 3< npm-v0.3.0/packages.txt
```

Once a release has gone through, Settings, Publishing access, "Require two-factor authentication
and disallow tokens" on each package leaves trusted publishing as the only way in.
