# Contributing to Bullet

Thanks for contributing! Because Bullet integrates with the League Client, every change is held to a clear quality bar.

## Before You Start

- **Questions & setup help:** ask on [Discord](https://discord.gg/e2dH2nUjd9) before starting large changes.
- **Bug reports:** use the issue template and attach the bundle from **Support → Export diagnostics** (`%LOCALAPPDATA%\Bullet\logs` on Windows or `~/Library/Application Support/Bullet/logs` on macOS).
- **Security issues:** report privately — see [SECURITY.md](SECURITY.md).

## Development Setup

1. Install [Rust](https://rustup.rs) (toolchain from `rust-toolchain.toml`). On Windows, install [Inno Setup 6](https://jrsoftware.org/isinfo.php) to build installers.
2. Run `cargo xtask check` (and `cargo deny check` if dependencies changed) before opening a PR.

## Code Standards

- No `unwrap()` or `expect()` outside tests; propagate errors with context.
- Every discarded `Result` (`let _ = ...`) must include `// ignore-ok: <reason>`.
- Spawn background tasks through `Supervisor`, never bare `tokio::spawn`.
- Mutate shared state only through named transitions in `bullet-core::state`.
- Validate all offsets in binary parsers; malformed input must return a typed error, never panic.
- Keep code, logs, and docs in English; place user-facing strings in i18n tables.
- No comments in source files (only `// ignore-ok: <reason>`, `# zizmor: ignore[...]`, and pinned action tags), max 400 lines per code file, and max 5 code files per folder (enforced by `cargo xtask check`).
- Keep OS-specific logic isolated behind `#[cfg(windows)]` / `#[cfg(target_os = "macos")]`.

## Pull Requests & License

- Keep PRs focused on a single change and include real-match verification logs for gameplay/injection changes.
- All CI checks must pass. Contributions are licensed under the [MIT License](../LICENSE).
