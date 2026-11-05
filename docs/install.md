# Installing Allot

Every way to install, upgrade, and remove Allot. The [README](../README.md) has the short version.

- [Windows](#windows)
- [Linux](#linux)
- [macOS](#macos)
- [Upgrade](#upgrade)
- [Uninstall](#uninstall)

## Windows

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

To type `allot` anywhere, add `%USERPROFILE%\.allot\bin` to your user `PATH` (optional).

### Without the setup

The release also has `allot-<version>-x86_64-pc-windows-msvc.zip`. Extract it and
run `allot.exe install` from that folder: it does everything the setup does except the Start
menu and Apps & features entries, and prints `[done]` for each step, or `[todo]` with what to
do by hand when a step could not be done. `allot install --help` lists its options
(`--no-claude`, `--no-logon-task`, `--launch-at-logon on|off`).

### From source

You need Rust (stable, MSVC toolchain, 1.99 or newer), and for the tray app the .NET 10 SDK and
Visual Studio's C++ build tools (the Native AOT linker). Inside a Claude session each build is a
`cpu_heavy` lease like any other.

```powershell
cargo build --release
dotnet publish tray\src\Allot.Tray -c Release -o target\release\tray
target\release\allot.exe install
```

The publish finds the linker through `vswhere.exe`; if it cannot, add
`C:\Program Files (x86)\Microsoft Visual Studio\Installer` to `PATH`. The setup itself is
built from `installer\allot.iss` with Inno Setup 7 (see the script's header).

## Linux

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

### The panel

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

### From source

You need Rust (stable, 1.99 or newer) and a C compiler for the bundled SQLite. Inside a Claude
session each build is a `cpu_heavy` lease like any other.

```bash
cargo build --release
target/release/allot install
```

`gnome-extensions install` wants a zip, so zip `apps/gnome/allot@mehran.tech` from inside the
folder first; `kpackagetool6 -i apps/kde/tech.mehran.allot` takes the folder as it is.

## macOS

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

### The menu bar app

```bash
brew install --cask allot-app
open /Applications/Allot.app
```

The app is signed ad hoc, not with an Apple Developer ID, so macOS stops it once: it says Apple
could not verify Allot, and System Settings > Privacy & Security then offers Open Anyway. A new
version asks again. The app also needs the install above, since it only talks to allotd.

### Without Homebrew

The release has `allot-<version>-aarch64-apple-darwin.tar.gz` and
`allot-<version>-macos-app.zip`, with a `.sha256` each. Check them with `shasum -a 256 -c`, run
`./allot install` from the unpacked tarball, and move `Allot.app` to Applications; from any other
folder macOS runs a copy it moved aside, which cannot launch at login.

### From source

You need Rust (stable, 1.99 or newer) and Xcode's command line tools. The app also needs Xcode and
[XcodeGen](https://github.com/yonaskolb/XcodeGen).

```bash
cargo build --release
target/release/allot install
cd apps/macos && xcodegen generate
xcodebuild -project Allot.xcodeproj -scheme Allot -configuration Release -derivedDataPath build build
```

## Upgrade

On Windows, run the new setup over the old install (or `allot.exe install` from a new zip or
build). Binaries that running sessions hold open cannot be overwritten, so the old file is renamed
aside (`allot-mcp.old-<millis>.exe`) and the new one copied in. Old copies that are still in use
are deleted by the next install. allotd restarts on the new build and restores its active and
queued leases. Open sessions keep their old `allot-mcp` until they restart. The hook picks up the
new `allot.exe` at once.

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

On Linux, run the newer package's `allot install`, as above.

On macOS, Homebrew updates its own copy only. `allot install` is what puts the new binaries in
`~/.allot/bin`, where sessions and allotd run them from:

```bash
brew upgrade allot
allot install
brew upgrade --cask allot-app
```

## Uninstall

On Windows, uninstall Allot in Settings > Apps > Installed apps. It asks whether to delete your
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
