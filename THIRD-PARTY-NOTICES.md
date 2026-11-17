# Third-party notices

Bullet's own source code is licensed under the [MIT License](LICENSE). This file lists the third-party
components Bullet uses or depends on and the terms they come with.

## Rust crates

`bullet.exe` statically links open-source Rust crates from [crates.io](https://crates.io). Every crate in the
dependency graph uses a permissive license, except the Slint interface toolkit (below). The accepted set is
enforced in CI by `cargo deny` (see `deny.toml`):

| License | Crates (approx.) |
| --- | --- |
| MIT and/or Apache-2.0 (dual or single) | ~175 |
| Unicode-3.0 | 19 |
| BSD-3-Clause, BSD-2-Clause, 0BSD | 5 |
| ISC | 5 |
| Zlib | 4 |
| CDLA-Permissive-2.0 (Mozilla CA bundle via `webpki-roots`) | 2 |
| BSL-1.0 | 2 |
| Unlicense, CC0-1.0 or MIT-0 (as an option) | 4 |

For the exact list with versions and license texts, run:

```powershell
cargo deny list          # every crate and its license
cargo tree --edges normal --target x86_64-pc-windows-msvc
```

The Apache-2.0 and BSD licenses require their copyright notices to travel with binaries. Those notices are
included in each crate's source, which is available from crates.io at the versions pinned in `Cargo.lock`.

## Other components

| Component | Use | License |
| --- | --- | --- |
| [Slint](https://slint.dev) (`slint`, `i-slint-*`) | Draws Bullet's windows; linked into `bullet.exe` | Slint Royalty-free License 2.0 (chosen from `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`); attribution is the "Made with Slint" badge on the project page |
| [Inno Setup](https://jrsoftware.org/isinfo.php) | Builds the Windows installer | Inno Setup License (permissive) |
| `relay-worker` dev tooling (`wrangler`, `typescript`, `@cloudflare/workers-types`) | Build and deploy only; not shipped to users | MIT / Apache-2.0 |

## Reference projects

Bullet was built by studying the projects listed in the README's acknowledgements. No source code from them
is included, and no third-party injector is used or shipped. Where a reference is GPL-licensed, Bullet
reimplements the behavior it needs independently.

## Trademarks

League of Legends and Riot Games are trademarks or registered trademarks of Riot Games, Inc. Bullet is not
affiliated with, endorsed or sponsored by Riot Games.
