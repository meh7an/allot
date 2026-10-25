# Contributing

Thanks for your interest in Allot. Bug reports, ideas, and pull requests are all welcome.

## Before you start

- For anything larger than a small fix, open an issue first so the approach can be agreed on
  before you spend time on it.
- Allot runs on Windows only. The sampler uses Windows APIs, so the workspace does not build
  on other platforms.

## Development setup

You need Windows 10 or 11 and Rust stable with the MSVC toolchain (1.99 or newer), including
`rustfmt` and `clippy`. The workspace links with `rust-lld` (see `.cargo/config.toml`), which
ships with the Rust toolchain.

```powershell
cargo build --workspace
```

The workspace crates:

| Crate | Path | Role |
|-------|------|------|
| `allot-shared` | `shared/` | Protocol, config, and path types |
| `allot-core` | `core/` | Ledger, admission, classifier, sampler trait, history |
| `allot-sampler` | `sampler/` | Windows CPU, RAM, GPU, and VRAM sampling |
| `allotd` | `daemon/` | The daemon: HTTP API, SQLite persistence |
| `allot-client` | `client/` | HTTP client for allotd |
| `allot-mcp` | `mcp/` | The MCP server |
| `allot-cli` | `cli/` | The `allot` CLI and the hooks |

Database changes go through Diesel migrations in `daemon/migrations/`; regenerate
`daemon/src/db/schema.rs` with the Diesel CLI after adding one.

If you run Allot while developing it, your own sessions will ask for a `cpu_heavy` lease
before these builds, like any other.

## Checks

CI runs these on `windows-latest`, and a pull request needs all three to pass:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The tests use a fake sampler and temporary databases, so they do not touch an installed allotd.

## Pull requests

- Keep each pull request to one change, with tests for new behavior.
- Write commit messages in the imperative mood ("Add", "Fix", "Refactor"), one specific line
  under 72 characters, without a type prefix.
- Update `README.md`, `docs/runbook.md`, and the `[Unreleased]` section of `CHANGELOG.md` when
  behavior a user can see changes.

## License

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in the work by you, as defined in the Apache-2.0 license, shall be dual licensed under the MIT
and Apache-2.0 licenses, without any additional terms or conditions.
