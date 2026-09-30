# Build, tests and CI

## Toolchain

Rust stable, edition 2024, pinned in `rust-toolchain.toml`. CI reads the same file, so a developer machine
and CI cannot drift apart. The release target is `x86_64-pc-windows-msvc` with a static C runtime. Every
crate inherits its version from `[workspace.package]` in the root `Cargo.toml`.

## Local checks

```powershell
cargo xtask check    # error-handling sweep, rustfmt, clippy -D warnings, all tests
cargo deny check     # advisories, licenses, banned crates, sources
```

`cargo xtask check` runs, in order:

1. the error-handling sweep (every discarded `Result` must carry a written justification),
2. `cargo fmt --all -- --check`,
3. `cargo clippy --workspace --all-targets -- -D warnings`,
4. `cargo test --workspace`.

## Running the tests without Windows

On Linux, the tests can run for the Windows target under Wine by building for `x86_64-pc-windows-gnu`. Two
things are needed:

1. `WebView2Loader.dll` on `WINEPATH`, because the `wry` crate links against it. Without it, the test binaries
   that depend on `bullet-platform` fail to load (`status c0000135`), which is not a real failure.
2. A virtual display (`xvfb`) for the Win32 window calls.

```sh
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=<script that runs wine with WINEPATH set>
xvfb-run -a cargo test --workspace --target x86_64-pc-windows-gnu
```

This validates the logic on the Windows target. It does not replace a real match: injection, game modes and
party mode are only proven in game.

## Test layout

Large test modules live in sibling files named `<module>_tests.rs`, included with
`#[cfg(test)] #[path = "..."] mod tests;`. They are still the module's own `tests` module with
`use super::*`, so they see private items. The split only makes files easier to navigate.

## Test layers

What a test can prove depends on what it runs. Bullet keeps three layers apart and never lets a lower one
claim what only a higher one can show.

| Layer | What it runs | What it proves | Where |
| --- | --- | --- | --- |
| Assets and runtime | Bullet's own generator, mod import and staging, compatibility check and overlay builder, on a synthetic game install built by the test | The chosen skin, its companions and a custom mod end up in the archives the game would open: slot 0 holds the skin's own definition, every link resolves, the animation graph the skin names is reachable, archives that share a path agree, headers and untouched bytes are the game's, output is deterministic | `crates/bullet-app/tests/skin_pipeline.rs` (runs everywhere, including CI) |
| Same, on the installed game | The same production code against a real install | The above for every champion, skin and chroma of the current patch | `cargo xtask harness`, `cargo xtask skin-audit`, `native_overlay_faithful` (ignored; needs the game) |
| Interface | Bullet's windows and pages in Edge, and the real WebView2 window | Pages render, react and pass accessibility checks; a release changes only what it meant to | `cargo xtask ipc-probe` and the page dumps (`BULLET_UI_DUMP`) used by the visual tests |
| Real match | The game engine | How the skin looks and animates in game: models, animations, shaders, particles | Manual, with the log; the only layer that can show it |

The first two layers show that Bullet loads and applies the skin files correctly. They do not render
anything: no test here can show how a skin looks or animates in the game, because that needs the game engine.

## Pipeline

All workflows live in `.github/workflows`. Every third-party action is pinned to a commit SHA, every job
starts with read-only permissions, and no workflow runs pull request code with a write token.

### On every push and pull request

| Workflow | Job | What it proves |
| --- | --- | --- |
| `ci.yml` | Format, lint, test | `Cargo.lock` is current, and `cargo xtask check` passes on Windows |
| | Release build | The release profile builds, the executable carries its file details, and its manifest runs it without elevation |
| | Dependencies | `cargo deny`: no known advisory, no disallowed license, no banned crate or unknown source |
| | Relay worker | `npm audit` and a strict TypeScript typecheck |
| | Workflow lint and audit | `actionlint` (syntax, expressions) and `zizmor` (template injection, credential leaks, unpinned actions) |
| | **CI OK** | Aggregate: fails if any job above failed, was cancelled or skipped |
| `security.yml` | CodeQL | Static analysis of Rust, TypeScript and the workflows themselves (`security-extended` queries) |
| | Secret scan | `gitleaks` over the whole history, since a deleted secret is still public |
| | Dependency review | On pull requests: blocks new dependencies with known vulnerabilities |
| | **Security OK** | Aggregate of the jobs above |

`security.yml` also runs weekly, so new advisories and queries reach code that has not changed.

### From approval to release

```text
maintainer approves a pull request
  └─ pr-approved.yml records the PR number and the approved commit (read-only token)
     └─ automerge.yml re-checks everything through the API and enables auto-merge (squash)
        └─ GitHub merges only when "CI OK" and "Security OK" pass on that exact commit
           └─ release.yml: if the workspace version has no release yet, full gate without cache,
              bullet.exe signed, installer built around it and signed (when signing is configured),
              checksums,
              build provenance attestation → published as a PRE-RELEASE
              └─ after testing it in a real match, a maintainer runs promote.yml, which checks
                 the checksums and the attestation and marks the release as latest
```

**What auto-merge refuses**, even with an approval:

- a commit pushed after the approval,
- a draft, a pull request not targeting `main`, or one with changes still requested,
- an approval from someone without write access,
- the `no-automerge` label,
- any change to paths that decide trust, permissions or what ships: `.github/`, `installer/`, `xtask/`,
  `.cargo/`, the toolchain, `deny.toml`, `Cargo.lock`, the app's build script and injection trigger, the
  injector host and DLL validation, the party cipher and token, and the relay's deploy
  config. These are merged by hand.

The `main` branch ruleset (`.github/rulesets/main.json`) enforces the rest: pull requests only, squash merges,
a code owner's approval, stale approvals dismissed on push, required checks up to date with `main`, no force
pushes or deletions.

### Repository settings the pipeline needs

| Setting | Why |
| --- | --- |
| GitHub App with *Contents* and *Pull requests* write, installed on the repository; `BULLET_BOT_APP_ID` variable and `BULLET_BOT_PRIVATE_KEY` secret | A merge made with the default token would not trigger the release workflow |
| *Allow auto-merge* and *Allow squash merging* | Auto-merge waits for the required checks |
| The ruleset, applied with `gh api -X POST repos/<owner>/<repo>/rulesets --input .github/rulesets/main.json` | Makes the checks and reviews mandatory |
| `production` environment with a required reviewer | Gates promotion to latest |
| Approval required for workflows from outside collaborators | First-time contributors cannot run workflows unreviewed |
| SignPath project; `SIGNPATH_API_TOKEN` secret; `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_SIGNING_POLICY_SLUG`, `SIGNPATH_EXE_CONFIGURATION_SLUG` and `SIGNPATH_SETUP_CONFIGURATION_SLUG` variables | Authenticode signing of `bullet.exe` and the installer. Without the secret the signing steps are skipped and the release ships unsigned, which Windows SmartScreen and antivirus reputation treat as an unknown program |

## Packaging

- `cargo xtask package` builds the release, copies it to `dist\` and writes `SHA256SUMS`.
- The LTK injector is **not** packaged. Its license does not allow other projects to redistribute League
  Toolkit's signed binaries, so users copy `ltk_patcher_host.exe` and `ltk_patcher_dll.dll` from an official
  LTK Manager release into `Program Files\Bullet\tools`. Bullet only accepts them if their SHA-256 matches the
  audited hashes in `bullet_app::trigger`. `xtask` reads the same constants for the install audit.
- `cargo xtask installer` passes the workspace version to Inno Setup (`installer/bullet.iss`) and produces
  `Bullet-Setup-<version>-x64.exe`. With `--prebuilt` it packages the `distullet.exe` already there instead
  of rebuilding it, so the release can sign the binary before it goes into the installer.
- The release signs with Authenticode through SignPath. The Sigstore attestation proves where a build came from
  to anyone who checks it with `gh`, but Windows does not read it: only an Authenticode signature counts for
  SmartScreen and antivirus reputation.
- The skin library is not shipped. Store skins are generated from the installed game on each patch, and the
  installer only creates an empty `library` folder.
- Files are listed one by one in the installer, never with a wildcard.

## Install and uninstall audit

```powershell
cargo xtask install-audit installed
cargo xtask install-audit uninstalled
cargo xtask install-audit uninstalled --kept-user-content
```

This checks files, tool hashes and registry entries (the uninstall entry, and that no "run as administrator"
flag was left under HKLM or HKCU) against what the installer promises. The install location is read from the
registry, never assumed.
