# Contributing

Thanks for your interest in Allot. Bug reports, ideas, and pull requests are all welcome.

## Before you start

- For anything larger than a small fix, open an issue first so the approach can be agreed on
  before you spend time on it.
- Windows code stays in Windows modules, and each OS keeps its own backend behind the same API,
  so a change for one platform should leave the others as they are.

## Development setup

You need Rust stable (1.99 or newer) with `rustfmt` and `clippy`, and a C compiler for the bundled
SQLite. On Windows that means the MSVC toolchain, and the workspace links with `rust-lld` (see
`.cargo/config.toml`), which ships with the Rust toolchain.

```bash
cargo build --workspace
```

The workspace crates:

| Crate | Path | Role |
|-------|------|------|
| `allot-shared` | `shared/` | Protocol, config, path types, and the contract emitters |
| `allot-core` | `core/` | Ledger, admission, classifier, sampler trait, history |
| `allot-sampler` | `sampler/` | CPU, RAM, GPU, VRAM, and drive sampling, one backend per OS |
| `allotd` | `daemon/` | The daemon: HTTP API, SQLite persistence, Linux lease scopes |
| `allot-client` | `client/` | HTTP client for allotd |
| `allot-mcp` | `mcp/` | The MCP server |
| `allot-cli` | `cli/` | The `allot` CLI and the hooks |

The panels are separate projects, and only talk to allotd over HTTP:

| Panel | Path | Needs |
|-------|------|-------|
| Windows tray app | `tray/` | .NET 10 SDK and Visual Studio's C++ build tools |
| macOS menu bar app | `apps/macos/` | Xcode and XcodeGen; `AllotKit` holds the tested logic |
| GNOME Shell extension | `apps/gnome/` | Node and pnpm for its tests and type checks |
| KDE Plasma widget | `apps/kde/` | Node and pnpm; `pnpm build` writes its generated code |

Their types come from `contract/`, generated from the Rust types. A test fails when a generated
file is out of date.

Database changes go through Diesel migrations in `daemon/migrations/`; regenerate
`daemon/src/db/schema.rs` with the Diesel CLI after adding one.

If you run Allot while developing it, your own sessions will ask for a `cpu_heavy` lease
before these builds, like any other.

## Checks

CI runs these on Windows, on Linux (x86_64 and aarch64), and on macOS, and a pull request needs
them all to pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI also tests the tray app, the macOS app, the contract packages, and both Linux panels. The tests
use a fake sampler and temporary databases, so they do not touch an installed allotd.

## Pull requests

- Keep each pull request to one change, with tests for new behavior.
- Write commit messages in the imperative mood ("Add", "Fix", "Refactor"), one specific line
  under 72 characters, without a type prefix.
- Update `README.md`, the files in `docs/`, and the `[Unreleased]` section of `CHANGELOG.md` when
  behavior a user can see changes.

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under the MIT
and Apache-2.0 licenses, without any additional terms or conditions.
