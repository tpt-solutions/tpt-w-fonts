# tpt-fonts

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/tpt-solutions/tpt-fonts/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-fonts/actions/workflows/ci.yml)

**Modern font manager for designers & devs.**

OS font settings are archaic. Installing hundreds of fonts slows down app launch
times, and there is no easy way to temporarily load a font for a single project.
tpt-fonts is a font manager with **session-based activation** (fonts load for the
current login session and are cleaned up on exit or logout), **duplicate
detection**, and **variable font previewing**.

- **Platforms (v1):** Windows 10/11 and Linux. User-level installs only, no elevation.
- **Interfaces:** a Tauri desktop GUI, a scriptable CLI, and a resident tray process
  that holds session activations alive.
- **Formats (v1):** TTF, OTF, TTC, and variable fonts (`fvar`).
- **Archon:** planned for the [tpt-archon](https://github.com/tpt-solutions/tpt-archon)
  platform (memory-mapped, read-only shared font objects). Design-only for now.

Progress and the full roadmap live in [todo.md](todo.md).

## Repository layout

| Path | Purpose |
|------|---------|
| `core/` | Font engine, OS backends, session manager (`tpt-fonts-core`) |
| `cli/` | Scriptable `tpt-fonts` command (`tpt-fonts-cli`) |
| `tray-app/` | Resident session-activation host (`tpt-fonts-tray`) |
| `src-tauri/` | Tauri backend wrapping `core` (`tpt-fonts-gui`) |
| `src/` | TypeScript + Vite frontend, no framework |
| `spec.txt` | Product spec |

## Prerequisites

- [Rust](https://rustup.rs/). The toolchain is pinned in `rust-toolchain.toml`, so
  `rustup` installs it automatically.
- [Node.js](https://nodejs.org/) 20 or newer (LTS recommended).
- Windows: WebView2 runtime (preinstalled on Windows 11).
- Linux: the WebKitGTK and Tauri system packages listed in the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

## Building

```sh
npm install           # frontend and Tauri CLI
cargo check --workspace
cargo test --workspace
npm run build         # type-check and build the frontend into dist/
npm run tauri dev     # run the desktop app with hot reload
npm run tauri build   # produce platform installers
```

## Licensing

tpt-fonts is dual-licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option. Copyright (c) 2026 TPT Solutions.

Unless you state otherwise, any contribution you intentionally submit for inclusion
is licensed under the same MIT OR Apache-2.0 terms. See [CONTRIBUTING.md](CONTRIBUTING.md).
