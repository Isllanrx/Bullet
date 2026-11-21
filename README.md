<p align="center">
  <img src="assets/bullet.png" width="96" alt="Bullet">
</p>

<h1 align="center">Bullet</h1>

<p align="center">
  Open-source League of Legends skin changer and custom mod manager (<code>.fantome</code> / <code>.modpkg</code>) for Windows and macOS, written in Rust.
</p>

<p align="center">
  <a href="https://github.com/Isllanrx/Bullet/releases/latest"><b>Download</b></a> ·
  <a href="https://discord.gg/e2dH2nUjd9"><b>Installation Guide (Discord)</b></a> ·
  <a href="https://github.com/Isllanrx/Bullet/wiki"><b>Technical Wiki</b></a> ·
  <a href="https://github.com/Isllanrx/Bullet/issues"><b>Issues</b></a>
</p>

> [!WARNING]
> **Educational Project & Disclaimer:** Bullet is developed strictly for **educational and research purposes**. It is provided as is, without warranty of any kind. The author takes **no responsibility** for any account penalties, bans, damages, or consequences resulting from using or misusing this software. You use it at your own risk.

---

## Overview

Bullet reads your installed League of Legends archives (`DATA/FINAL/Champions/*.wad.client`) to generate and load skins, chromas, Exalted forms (`Ctrl+5`), TFT cosmetics, and custom mods locally at match start without modifying Riot's game files.

<p align="center">
  <img src="assets/urgot-select.png" alt="Bullet in Champion Select" width="49%">
  <img src="assets/urgot-ingame.png" alt="Bullet In-Game Preview" width="49%">
</p>

## Installation

> **Step-by-step installation tutorial:** Full setup instructions and support are inside our **[Discord server](https://discord.gg/e2dH2nUjd9)**.

### Windows (10 / 11 x64)

Download [`Bullet-Setup-x64.exe`](https://github.com/Isllanrx/Bullet/releases/latest) or run in **PowerShell**:

```powershell
irm https://raw.githubusercontent.com/Isllanrx/Bullet/main/install.ps1 | iex
```

Or install with Cargo:

```powershell
cargo install --git https://github.com/Isllanrx/Bullet.git bullet-app --locked
```

### macOS (Intel `x86_64` & Apple Silicon)

Run in **Terminal**:

```bash
command -v cargo >/dev/null 2>&1 || curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env"; cargo install --git https://github.com/Isllanrx/Bullet.git bullet-app --locked && bullet
```

For in-client mode on macOS, launch League with remote debugging enabled:

```bash
open -a "League of Legends" --args --remote-debugging-port=46359
```

## Features

- **Skins, Chromas & Forms (`Ctrl+5`):** Supports 170+ champions and 9,000+ skins, chromas, Exalted, Ultimate, and Hall of Legends forms, plus localized loading-screen names.
- **Window & In-Client Modes:** Use Bullet's desktop window or the native in-client carousel without Pengu Loader.
- **Customize Identity & TFT:** Summoner Icons, Borders, Banners, Titles, Tokens, and TFT Tacticians, Booms, Portals, and Arenas.
- **Custom Mods (`.fantome`, `.modpkg`, `.zip`):** Import and repair custom skins, maps, HUDs, fonts, and announcers.
- **Party Mode:** End-to-end encrypted (`X25519` + `ChaCha20-Poly1305`) skin and identity sync with up to 5 friends.
- **All Regions & Languages:** Works across all Riot servers (including WeGame/CN), Classic Rift (JADE), and supports **English**, **Português (BR)**, and **Español**.

## FAQ

**Where is the full installation tutorial?**
Inside our Discord server: **[discord.gg/e2dH2nUjd9](https://discord.gg/e2dH2nUjd9)**.

**Does Bullet modify my League of Legends installation?**
No. Overlays are built separately and loaded in memory at match start. Your game files stay untouched.

**Does it require Administrator privileges or Pengu Loader?**
No. On Windows it runs as a standard user (`asInvoker`) and does not use Pengu Loader.

**Can other players see my skin?**
Only locally, unless you and your friends enable **Party Mode** inside Bullet.

**Where can I read how Bullet works internally?**
Check the **[Technical Wiki](https://github.com/Isllanrx/Bullet/wiki)**.

## Build from Source

```powershell
cargo xtask check
cargo xtask package
cargo xtask installer
```

## License

[MIT License](LICENSE) · [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)
