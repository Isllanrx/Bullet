# Architecture

## Overview

Bullet is a Cargo workspace. A single tray executable (`bullet-app`) runs everything: it follows the League
client, builds the overlay and drives the injector. The skin is chosen in a window Bullet owns (a WebView2
view), never inside the client.

```text
                  ┌──────────────────────────────────────────────────┐
  League client   │  bullet-lcu      follows phases and selections    │
  (LCU REST + WS) │─────────────┐                                     │
                  │             ▼                                     │
  Bullet window   │  bullet-app      shared state + injection trigger │
  (WebView2)  ────┼─────────────┤   decides what to build and when    │
                  │             ▼                                     │
  Installed game  │  bullet-inject   builds the overlay (bullet-wad)  │
  (DATA/FINAL) ───┼─────────────►   and arms the injector host        │
                  │             ▼                                     │
  Game process    │  injector DLL    serves the overlay to the game   │
                  └──────────────────────────────────────────────────┘
```

## Crates and dependency rules

| Crate | Responsibility | Depends on |
| --- | --- | --- |
| `bullet-core` | Domain types, shared state, game phases, task supervisor, configuration names | nothing |
| `bullet-platform` | Windows: processes, discovery of the game and tools, windows, tray, translations | core |
| `bullet-wad` | WAD archives, BIN/PROP data files, `.fantome` packages, hash index | nothing |
| `bullet-lcu` | League client REST and WebSocket, champion select | core |
| `bullet-classic` | Skins and Classic Rift models generated from the installed game | wad |
| `bullet-inject` | Mod checks, overlay builder, injector host, suspension guard | core, platform, wad |
| `bullet-party` | Encrypted party rooms over a relay | core |
| `bullet-relay` | Optional self-hosted relay server | none of the above |
| `bullet-app` | The executable: composition, lifecycle, tray, catalog, injection trigger | all |

Dependencies only point one way. The pure crates (`core`, `wad`) know nothing about Windows or the client,
so most of the logic can be tested without either.

## Startup sequence

1. **Single instance.** A named mutex guarantees one running Bullet; starting it again brings the first one
   to the front.
2. **User profile.** The desktop user is resolved through the Windows API, and data goes to that user's
   `%LOCALAPPDATA%\Bullet`.
3. **Logging.** A non-blocking daily log file starts in `%LOCALAPPDATA%\Bullet\logs`.
4. **Recovery.** If a previous run died while the game was suspended, the game is resumed.
5. **Discovery.** The game install (from Riot's metadata or the running process) and the injector tools (from
   Bullet's own folder, hash-checked) are located.
6. **Warm-up.** The index of the game's archives is built on a background thread so champion select never
   waits for it.
7. **Game build.** The game executable's build timestamp is read. After a patch, cached overlays and locale
   data are discarded. If the injector DLL does not support this build, the user is told immediately.
8. **Services.** The supervised tasks start: client observer, selection window session, party mode, and the
   optional skin library sync.

## Shared state

`AppState` is published through a Tokio `watch` channel. Readers always see the latest complete value, never
a partial update. It can only be changed through named transitions in `bullet_core::state`, so every change
is easy to find and to log. The main fields are the game phase, the team roster (used to verify party
announcements), the selected custom mods, the injection status and the party members.

Every background task is started by the `Supervisor` with a cancellation token. None is spawned on its own,
so shutdown is orderly and a task that dies is noticed.

## Runtime discovery

| What | Where it comes from |
| --- | --- |
| Game folder | The running game process, otherwise `product_install_full_path` in the Riot Client metadata under `%ProgramData%\Riot Games\Metadata`; any drive, folder or region |
| Client API | Port and password from the client's lockfile |
| Game build | `TimeDateStamp` of `League of Legends.exe` |
| Language | The selection window follows the client's locale; tray and dialogs, which can open before the client, follow Windows |
| Party relay | `BULLET_RELAY_URL`, then `party.json`, then the built-in default |

## Configuration

Every environment variable Bullet reads is listed in `bullet_core::env`. Domain constants stay with the
code that owns them: audited tool hashes in `bullet_app::trigger`, the default relay in
`bullet_party::config`, and the DLL's supported build limit in `bullet_inject::ltk_host`.
