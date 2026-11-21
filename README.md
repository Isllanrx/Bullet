<p align="center">
  <img src="assets/bullet.png" width="96" alt="Bullet">
</p>

<h1 align="center">Bullet</h1>

<p align="center">
  A League of Legends skin changer for Windows, written in Rust.<br>
  <em>The right skin loads into the match — every mode, every time.</em>
</p>

<p align="center">
  <a href="https://github.com/Isllanrx/Bullet/actions/workflows/ci.yml"><img src="https://github.com/Isllanrx/Bullet/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/Isllanrx/Bullet"><img src="https://api.scorecard.dev/projects/github.com/Isllanrx/Bullet/badge" alt="OpenSSF Scorecard"></a>
  <a href="https://github.com/Isllanrx/Bullet/releases/latest"><img src="https://img.shields.io/github/v/release/Isllanrx/Bullet?include_prereleases&sort=semver" alt="Release"></a>
  <a href="https://github.com/Isllanrx/Bullet/releases/latest"><img src="https://img.shields.io/github/downloads/Isllanrx/Bullet/total?label=downloads" alt="Downloads"></a>
  <img src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-0078D4" alt="Platform">
  <img src="https://img.shields.io/badge/rust-stable%20%C2%B7%201.85%2B-B7410E" alt="Rust stable">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT">
  <a href="https://discord.gg/e2dH2nUjd9"><img src="https://img.shields.io/badge/Discord-join%20the%20community-5865F2?logo=discord&logoColor=white" alt="Discord"></a>
</p>

<p align="center">
  <a href="https://github.com/Isllanrx/Bullet/releases/latest"><b>Download</b></a> ·
  <a href="https://discord.gg/e2dH2nUjd9"><b>Discord community</b></a> ·
  <a href="https://github.com/Isllanrx/Bullet/issues"><b>Report a bug</b></a>
</p>

---

Bullet generates skins straight from your installed League of Legends files and applies them in-game via LTK Manager — without modifying your game files, without Pengu Loader, and without external plugins.

<p align="center">
  <img src="assets/urgot-select.png" alt="Bullet in Champion Select" width="49%">
  <img src="assets/urgot-ingame.png" alt="Bullet in Game" width="49%">
</p>

> [!IMPORTANT]
> **Educational project**, provided as is, without warranty. The author is not responsible for any damage, account penalty, or loss caused by its use. See the [Disclaimer](#disclaimer).

## How to Use Bullet

Follow these 4 simple steps to get started:

1. **Download and install Bullet**
   - https://github.com/Isllanrx/Bullet/releases
2. **Download and install LTK Manager** (`1.33.0+`)
   - https://github.com/LeagueToolkit/ltk-manager/releases
3. **Open Bullet and authenticate**
   - Launch Bullet (no administrator rights needed).
   - Log in with your GitHub account and finish authentication.
   - Visit the Bullet repository and click **Star**: https://github.com/Isllanrx/Bullet
4. **Choose your skin**
   - Open League of Legends and enter Champion Select (or Lobby in Swiftplay/Brawl).
   - Pick your champion and select your skin or chroma in Bullet's window or directly inside the client carousel.
   - That's it — your selected skin loads automatically when the match starts.

Enjoy Bullet!

## Features

- **Every skin, chroma, and form** — includes Exalted, Hall of Legends, and Elementalist forms, plus localized skin names on the loading screen.
- **Two picker modes** — switch on the Home tab between **Bullet's window** and **In-Client mode** (native Hextech chroma flyout, Swiftplay banner sync, and top-bar Mods & Panel menu — zero Pengu Loader).
- **Every server** — works across all Riot regions (BR, NA, EUW, KR, JP, SEA, TW, VN, PBE) and regional clients (including China/WeGame).
- **Custom mods & one-click repair** — `.fantome`, `.zip`, and `.modpkg` (maps, fonts, announcers, skins) verified against the current patch, plus skin and client repair in **Support**.
- **Classic Rift (JADE)** — legacy models built directly from your installed game.
- **Party Mode** — end-to-end encrypted skin sharing with up to 5 friends and automatic lobby pairing.
- **Local skin stats** — clean offline win/loss and win-rate tracking per skin, zero telemetry.
- **Safe by design** — runs without admin rights, never modifies game files, verifies signed updates, and supports English, Portuguese, and Spanish.

## Troubleshooting

- Run **Repair skins and mods** or **Repair client** in **Support** if a patch or mod breaks.
- Click **Export diagnostics** in **Support** (`%LOCALAPPDATA%\Bullet\logs`) before opening an [issue](https://github.com/Isllanrx/Bullet/issues) or asking on [Discord](https://discord.gg/e2dH2nUjd9).
- Close other skin tools while Bullet is running to avoid injector conflicts.

## Building from Source

```powershell
cargo xtask check
cargo xtask package
cargo xtask installer
```

## Acknowledgements

Inspired by [Rose](https://github.com/Alban1911/Rose) and the open-source projects in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). Maintained by **Isllan Toso** — [isllan.dev](https://isllan.dev/).

## Disclaimer

Bullet is published **for educational purposes**, **as is, without warranty of any kind**, under the MIT license. Using third-party software with League of Legends violates Riot Games' Terms of Service; you use it at your own risk. Bullet only changes visuals locally on your machine and is not affiliated with or endorsed by Riot Games. League of Legends is a trademark of Riot Games, Inc.

## License

- **Source Code:** [MIT License](LICENSE)
- **Third-Party Components:** [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
