# Contributing to Bullet

Thanks for contributing. Because Bullet interacts closely with the game client, every change is held to a clear bar.

## Before You Start

- **Questions & ideas:** ask on [Discord](https://discord.gg/e2dH2nUjd9) first to align before writing code.
- **Bug reports:** use the issue template and attach the bundle from **Support → Export diagnostics** (or logs from `%LOCALAPPDATA%\Bullet\logs`).
- **Security issues:** report privately — see [SECURITY.md](SECURITY.md).

## Development Setup

1. Windows 10/11 x64, [Rust](https://rustup.rs) (toolchain from `rust-toolchain.toml`), and [Inno Setup 6](https://jrsoftware.org/isinfo.php) for the installer.
2. Run `cargo xtask check` (and `cargo deny check` if dependencies changed) before opening a PR.
3. Check the crate README (`crates/<crate>/README.md`) and [`docs/`](../docs/README.md) for the area you touch.

## Code Rules

- No `unwrap()` or `expect()` outside tests; handle or propagate errors with context.
- Every discarded `Result` (`let _ = ...`) must carry `// ignore-ok: <reason>`.
- Spawn background tasks through `Supervisor`, never a bare `tokio::spawn`.
- Mutate shared state only through named transitions in `bullet-core::state`.
- Validate all offsets in binary parsers; malformed input returns a typed error and never panics.
- Code, logs, and docs are in English; user-facing text lives in the i18n tables.
- No comments in code files (only `// ignore-ok: <reason>`, `# zizmor: ignore[...]`, and pinned action version tags). Enforced by `cargo xtask check` (`cargo xtask comments --strip` removes the rest).
- Max 400 lines per code file and max 5 code files per folder (tests included). Enforced by `cargo xtask check`.
- No hardcoded drive letters, install paths, servers, or languages; log state changes, never inside loops.
- Justify any new dependency in the PR description.

## Pull Requests & Releases

- Keep PRs focused on one change and complete the PR checklist.
- Changes affecting in-game behavior (injection, modes, party) must include real-match evidence (Bullet log + observed result).
- CI must pass. Approved PRs auto-merge except sensitive areas (workflows, installer, `xtask`, toolchain/deny policy, lockfile, trigger/injector, party crypto, relay deploy config), which are merged manually.
- Versions are bumped automatically on merge into a pre-release and promoted only after real-match verification. See [docs/build-and-ci.md](../docs/build-and-ci.md).

## License

Contributions are licensed under the [MIT License](../LICENSE).
