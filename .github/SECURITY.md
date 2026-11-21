# Security Policy

## Reporting a Vulnerability

Report security issues **privately** through [GitHub Security Advisories](https://github.com/Isllanrx/Bullet/security/advisories/new). Do not open a public issue or post details on Discord.

Include when possible:

- Bullet version (`bullet.exe` → Properties → Details),
- Impact and preconditions,
- Reproduction steps and logs from `%LOCALAPPDATA%\Bullet\logs` (or **Support → Export diagnostics**).

Maintainer: **Isllan Toso** — [isllan.dev](https://isllan.dev/). Acknowledgements are sent within a few days; fixes ship first as a pre-release and are credited unless you prefer otherwise.

## Scope

**In scope:**

- `bullet.exe`, installer, and uninstaller,
- `bullet_launcher.exe` and `bullet_bridge.dll` (built from private `bullet_ltk` during official releases),
- GitHub sign-in and repository star check,
- Party Mode protocol, E2EE cipher, and relay (`relay-worker/`, `crates/bullet-relay`),
- `install.ps1` and the in-app updater,
- CI/release workflows in `.github/`.

**Out of scope:**

- The League of Legends client or game,
- Detection or penalties by Riot's anti-cheat (see the README [Disclaimer](../README.md#disclaimer)).

## Supported Versions

Only the latest release and the latest pre-release receive security fixes.

## Verifying a Download

Every release includes `SHA256SUMS`, its signature (`SHA256SUMS.sig`), and a GitHub build provenance attestation. Both the in-app updater and `install.ps1` verify checksums before running the installer. Releases are not code-signed:

```powershell
Get-FileHash .\Bullet-Setup-<version>-x64.exe -Algorithm SHA256
gh attestation verify .\Bullet-Setup-<version>-x64.exe --repo Isllanrx/Bullet
```
