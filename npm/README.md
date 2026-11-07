# Allot

Coordinates heavy jobs between the Claude Code sessions on one machine. Sessions ask a small local
daemon for a lease before a build, test suite, benchmark, or training run, and wait their turn when
the machine is busy, instead of all compiling at once.

## Install

```bash
npm install -g @meh7an/allot
allot install
```

npm puts the `allot` command on your PATH. `allot install` then copies allot, allotd and
allot-mcp into `~/.allot/bin`, sets up Claude Code (skill, MCP server, hooks and rule) and starts
allotd at login. Start a new Claude session afterwards. Run `allot install` again after each
`npm update -g @meh7an/allot`; until you do, `allot` reminds you. To remove Allot, run
`allot uninstall` first, then `npm uninstall -g @meh7an/allot`.

The package carries the binaries for Windows x86_64, Linux x86_64 and aarch64 (glibc 2.35 or
newer) and macOS 14 or newer on Apple Silicon, and npm installs only the one for your machine;
nothing is downloaded at install time. On Windows it also carries the tray app. Version 0.3.0's
Windows binaries need the
[Visual C++ runtime](https://learn.microsoft.com/cpp/windows/latest-supported-vc-redist).

The other ways to install, uninstalling and the full documentation are on
[GitHub](https://github.com/meh7an/allot#readme).

## License

MIT or Apache-2.0, at your option.
