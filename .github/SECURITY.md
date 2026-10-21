# Security policy

## Reporting a vulnerability

Please report security issues **privately** through
[GitHub Security Advisories](https://github.com/Isllanrx/Bullet/security/advisories/new). Do not open a
public issue or post details on Discord.

Useful details to include:

- the Bullet version (Explorer → `bullet.exe` → Properties → Details),
- what an attacker could do, and under which conditions,
- steps to reproduce, and the log from `%LOCALAPPDATA%\Bullet\logs` if relevant.

Maintainer: Isllan Toso, [isllan.dev](https://isllan.dev/).

You will get an acknowledgement within a few days. Fixes ship as a new pre-release first and are credited in
the release notes unless you prefer otherwise.

## Scope

In scope:

- `bullet.exe`, the installer and the uninstaller,
- the party mode protocol, its encryption and the relay (`relay-worker/`, `crates/bullet-relay`),
- the terminal install script (`install.ps1`) and the in-app update, which verify the installer before running it,
- the build and release pipeline in `.github/`.

Out of scope:

- the LTK patcher binaries (report those to [League Toolkit](https://github.com/LeagueToolkit/ltk-manager)),
- the League of Legends client or game,
- detection or penalties by Riot's anti-cheat. Using Bullet is at your own risk, as the README's disclaimer
  explains.

## Supported versions

Only the latest release and the latest pre-release receive security fixes.

## Verifying a download

Every installer published by the release workflow comes with a `SHA256SUMS` file, its signature
(`SHA256SUMS.sig`) and a build provenance attestation. Bullet itself checks the signed checksum list before it runs
an update, and `install.ps1` checks the installer against `SHA256SUMS` and its Authenticode signature:

```powershell
Get-FileHash .\Bullet-Setup-<version>-x64.exe -Algorithm SHA256
gh attestation verify .\Bullet-Setup-<version>-x64.exe --repo Isllanrx/Bullet
```
