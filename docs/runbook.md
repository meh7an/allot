# Allot runbook

Paths below are relative to `%USERPROFILE%\.allot\` unless they start with `~` or a drive.
`allot` means `bin\allot.exe`.

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
