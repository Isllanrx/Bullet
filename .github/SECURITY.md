# Security Policy

## Reporting a Vulnerability

Report security vulnerabilities **privately** via [GitHub Security Advisories](https://github.com/Isllanrx/Bullet/security/advisories/new). Please do not open public issues or post exploit details on Discord.

Include when possible:

- Bullet version, OS (Windows / macOS), and architecture (`x86_64` / `arm64`),
- Impact and reproduction steps,
- Diagnostic logs from **Support → Export diagnostics**.

Maintainer: **Isllan Toso** — [isllan.dev](https://isllan.dev/). Acknowledgements are sent within a few days, and fixes ship first as a pre-release.

## Scope & Supported Versions

**In scope:** `bullet` desktop binary, Windows installer/uninstaller, `install.ps1`, in-app updater, GitHub StarGate auth, Party Mode E2EE protocol & relay (`relay-worker/`, `crates/bullet-relay`), macOS Mach-O patcher, and `.github/` workflows.

**Out of scope:** The League of Legends client/game and Riot anti-cheat enforcement (see [Disclaimer](../README.md#disclaimer--license)).

Only the latest release and latest pre-release receive security updates.

## Verifying a Download

Every release includes `SHA256SUMS`, its Ed25519 signature (`SHA256SUMS.sig`), and a GitHub build provenance attestation:

```powershell
Get-FileHash .\Bullet-Setup-<version>-x64.exe -Algorithm SHA256
gh attestation verify .\Bullet-Setup-<version>-x64.exe --repo Isllanrx/Bullet
```
