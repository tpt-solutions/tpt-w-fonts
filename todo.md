# tpt-fonts — Project Checklist

TPT Solutions — dual-licensed under MIT OR Apache-2.0.
Modern font manager for designers & devs. Rust core engine + Tauri UI (TS/Vite,
no framework), with a resident tray process for session-scoped font activation.
Targets Windows and Linux for v1 (user-level fonts only, no elevation). Archon
(unified page-cache, memory-mapped read-only shared font objects) is design-only
until the core is solid.

Reference: Archon platform — https://github.com/tpt-solutions/tpt-archon

Decisions locked in for this roadmap:
- UI is **Tauri** (Rust `src-tauri/` + plain TS/Vite frontend) — variable font
  preview rendering leans on CSS `font-variation-settings`/`@font-face`, not a
  hand-built rasterizer.
- "Session-based activation" means **login-session-scoped temporary loading**
  (system-wide for the session, cleaned up on exit/logout) — not project-linked
  auto-activation font sets. Requires a resident tray process to hold the
  registrations alive.
- v1 install scope is **user-level only** (no admin elevation). System-wide
  (all-users) install is a later phase.
- Duplicate detection covers **both** exact byte-hash duplicates and metadata
  duplicates (same family/subfamily/version/vendor, different file).
- **CLI ships alongside the GUI in v1**, not deferred.
- Font formats in v1: **TTF, OTF, TTC, and variable fonts (`fvar`)** only.
  WOFF/WOFF2 import is backlog.

---

## Status Summary

| Phase | Area | Progress |
|-------|------|----------|
| 0 | Project Setup & Licensing | 🟡 In Progress (repo hosting pending) |
| 1 | Core Font Data Model & Scanning Engine | 🟡 In Progress |
| 2 | Duplicate Detection | 🟡 In Progress (UI pending) |
| 3 | Variable Font Preview Engine | 🟡 Implemented, GUI not yet exercised |
| 4 | Windows Backend: User-Level Install & Session Activation | 🟡 In Progress (tray wiring pending) |
| 5 | Linux Backend: Fontconfig & Session Activation | 🟡 Implemented, not yet run on Linux |
| 6 | Resident Tray App & Session Manager | 🟡 Tray runs on Windows with a quick-activate menu; Linux and logoff cleanup pending |
| 7 | CLI | 🟡 Implemented; SVG preview only, uninstall not run from the CLI |
| 8 | Tauri GUI: Main Application | 🟡 Library, detail, preview, duplicates and settings run; grouping and thumbnails open |
| 9 | Archon Support (Design Only) | 🟡 Design draft written; open questions await the tpt-archon team |
| 10 | Performance & Polish | 🟡 Scale measured on 10,000 fonts; polish not started |
| 11 | Testing & QA | 🟡 In Progress (unit and flow tests in; manual QA and stress test open) |
| 12 | Release v1 | 🟡 In Progress (installer builds locally; signing, Linux packages and tag open) |

---

## Phase 0 — Project Setup & Licensing
- [x] Init git repo, `.gitignore` (Rust + Node/Tauri targets + Windows build artifacts)
- [x] Add `LICENSE-MIT` and `LICENSE-APACHE` (dual license, TPT Solutions copyright)
- [x] Add SPDX dual-license header/notice convention for source files
- [x] Cargo workspace: `core/` (font engine, OS backends, session manager), `cli/` (scriptable commands), `tray-app/` (resident session-activation host), `src-tauri/` (Tauri backend wrapping core), `src/` (TS/Vite frontend, no framework); shared `[workspace.package]` (`license`, `authors = ["TPT Solutions"]`, `repository`)
- [x] `rust-toolchain.toml` (MSRV pin), `rustfmt.toml`, `clippy.toml`
- [x] Configure `tauri.conf.json` (app id, window defaults, bundle identifiers for tpt-fonts)
- [x] TypeScript config (tsconfig, Vite, no framework)
- [x] `README.md` (from spec.txt), build instructions, license badges
- [x] Set up CI (GitHub Actions, build+lint+test on Windows + Linux runners) — workflow added in `.github/workflows/ci.yml`; not yet run on GitHub
- [ ] Repo hosting under `tpt-solutions` GitHub org

## Phase 1 — Core Font Data Model & Scanning Engine
- [x] Font metadata model (family, subfamily/style, full name, PostScript name, version, vendor/foundry, embedding/`fsType`, file path, format, content hash)
- [x] Font file parser (`ttf-parser` or equivalent) extracting `name`, `OS/2`, `head`, and `fvar` (variable axes + named instances) tables
- [x] Font format detection: TTF / OTF / TTC / variable vs static
- [x] Recursive scan of font directories (Windows per-user + system read-only view, Linux `~/.local/share/fonts` + `/usr/share/fonts` read-only view) building an in-memory catalog
- [x] Catalog persistence for fast cold start (SQLite or serialized cache) — JSON cache with schema version and atomic write (not SQLite)
- [x] Filesystem watch on font directories (`ReadDirectoryChangesW` / `inotify`) to keep the catalog live — the GUI watches both font folders and refreshes the library on change, with bursts coalesced; verified live (a copied font raised the count 346 to 347, and removing it restored 346). The CLI and tray do not watch.
- [x] Unit tests: parsing fixtures across TTF/OTF/TTC/variable-font samples — runs against installed system fonts; skips when absent, no font binaries committed

## Phase 2 — Duplicate Detection
- [x] Exact duplicate detection: content hash (e.g. blake3) grouping across catalog
- [x] Metadata duplicate detection: same family + subfamily + version + vendor across different files/hashes
- [x] Duplicate group model + resolution actions (keep one, remove others, side-by-side metadata diff view) — core model, keep-one plan and side-by-side diff data; the view itself lands in Phase 8
- [x] Safe-delete flow (send to OS trash/recycle bin, never permanent delete) — uses the OS trash; dry-run covered by tests, live trash not exercised in CI
- [x] Unit tests: hash-based and metadata-based grouping against fixture sets (true dupes, false-positive near-matches)

## Phase 3 — Variable Font Preview Engine
- [x] `fvar` axis parsing (`wght`, `wdth`, `slnt`, `ital`, `opsz`, custom axes) + named instances
- [x] Live preview data model: selected font, per-axis current value, sample text, size, saved custom instances
- [x] Preview rendering via Tauri asset/custom protocol serving the local font file to the frontend, rendered with `@font-face` + `font-variation-settings` — implemented via asset protocol and FontFace; not yet exercised in a running window
- [x] Preview UI: per-axis sliders (min/default/max), named-instance quick-select, sample text input, size control, light/dark background toggle — implemented in plain TS; not yet exercised in a running window
- [x] Static (non-variable) font preview fallback path — same UI, no axis sliders
- [x] Specimen/waterfall view (sample text rendered at multiple sizes) — waterfall sizes shared with the core constant

## Phase 4 — Windows Backend: User-Level Install & Session Activation
- [x] Shared `trait FontBackend` (install/uninstall/activate-session/deactivate-session/list) implemented per OS
- [x] Windows user-level permanent install (per-user font install API, Windows 10 1809+, no elevation, `HKCU` registration) — copies to %LOCALAPPDATA%\Microsoft\Windows\Fonts and registers under HKCU; verified with a real round trip
- [x] Windows user-level uninstall (registry cleanup + file removal) — refuses paths outside the user font folder
- [x] Windows session activation: `AddFontResourceEx` (shared, non-private flag) for system-wide-during-session visibility, tracked by the resident tray process — shared scope (flags 0), batched per call
- [ ] Session cleanup: `RemoveFontResourceEx` + `WM_FONTCHANGE` broadcast on tray app exit; handle abnormal-exit/logoff cleanup — Quit calls `deactivate_all` and a hidden window handles `WM_ENDSESSION`; abnormal exit verified live (tray killed with a session font active, `--cleanup` deactivated it and removed the state file). Not yet exercised: menu Quit and a real logoff
- [x] Avoid unnecessary GDI font-cache rebuild churn when activating/deactivating — one WM_FONTCHANGE per batch, and none if nothing changed
- [x] Unit/integration tests against a mockable Win32 boundary (CI-safe) — 8 tests use a fake Win32 boundary; 1 opt-in real test (`--ignored`) passed

## Phase 5 — Linux Backend: Fontconfig & Session Activation
- [x] Linux user-level permanent install (`~/.local/share/fonts`, targeted `fc-cache` refresh) — copies to $XDG_DATA_HOME/fonts with a scoped `fc-cache -f`; tested with a fake runner, not yet on a real Linux desktop
- [x] Linux user-level uninstall (file removal + fontconfig cache refresh) — refuses paths outside the user directory; fake-runner tested
- [ ] Fontconfig cache management: inspect cache files, trigger scoped `fc-cache -f` on affected dirs only (avoid full-system rebuild) — the refresh already runs on the one changed directory, never `fc-cache -r`. Inspecting cache files needs fontconfig's cache naming confirmed on a Linux machine
- [x] Linux session activation: dedicated session font directory + scoped fontconfig `<include>` added/removed by the resident process, cache refresh on activate/deactivate — session copies in $XDG_CACHE_HOME/tpt-fonts/session plus a fontconfig drop-in in conf.d; fake-runner tested
- [x] Session cleanup on resident-process exit/logout (systemd user unit `ExecStop` or signal handler) — `packaging/linux/tpt-fonts-tray.service` runs `tpt-fonts-tray --cleanup` as `ExecStop`, which deactivates whatever the last session recorded, including after a crash. The unit is in the .deb. Not yet run on a Linux desktop
- [ ] Verify behavior on X11 and Wayland

## Phase 6 — Resident Tray App & Session Manager
- [x] Cross-platform tray-app crate (native tray icon, Windows + Linux) — builds and runs on Windows (IPC and session verified over the named pipe); the tray icon itself was not checked visually, and Linux is not yet built or run
- [x] Session font registry: tracks active session fonts, OS handles, activation source — `SessionManager` + `SessionRegistry` track fonts, source (GUI/CLI/tray) and timestamps; persisted atomically; 11 tests
- [x] Tray menu: quick activate/deactivate list, open main window, quit (clean deactivation on quit) — the menu has Open tpt-fonts (starts the GUI from the tray's folder), an "Activate a recent font" submenu of the 40 newest library fonts that are not active (read from the catalog cache, so it does not scan), active fonts with deactivate, deactivate-all, autostart toggle and quit. The tray starts with this menu and answers CLI requests, but the menu items have not been clicked in a running tray
- [x] Optional user-toggleable autostart (Windows Run key/Startup folder, Linux `.desktop` autostart) — toggle is in the tray menu; the desktop-entry file is tested, the Run key is not yet toggled on a real login
- [x] Orphaned-resource recovery: detect stale session registrations left by a crash/abnormal exit on next launch and clean them up — `recover_orphans` checks the owner PID and deactivates stale fonts; a live owner is left alone
- [x] IPC between tray-app resident process and Tauri main window (local socket/named pipe) so the GUI reflects live session state — tray side: named pipe (local socket on Linux) with one JSON line per request. The GUI polls the tray every two seconds and refreshes the library when the active set or the tray's presence changes (`watch_session` in `src-tauri/src/lib.rs`). Not yet seen in a running window

## Phase 7 — CLI
- [x] `tpt-fonts list` — filters by family, format, kind (variable or static) and state; `--json` output. There is no separate style filter yet; family and subfamily are both shown
- [x] `tpt-fonts activate <font> --session` / `tpt-fonts deactivate <font>` — verified against a live tray (activate, list, deactivate)
- [x] `tpt-fonts install <file> [--user]` / `tpt-fonts uninstall <font>` — per-user only; uninstall from the CLI is not yet exercised
- [x] `tpt-fonts scan-duplicates [--fix | --dry-run]` — `--fix` only trashes byte-identical copies; metadata matches are report-only
- [x] `tpt-fonts preview <font> --axis wght=700 --text ...` — writes an SVG of outlines (no rasteriser needed); PNG output is not done
- [x] CLI talks to the same `core` engine (and resident process for session state) used by the GUI
- [x] CLI unit/integration tests — 5 end-to-end tests of the binary, plus query and specimen tests in core

## Phase 8 — Tauri GUI: Main Application
- [x] Main window: font catalog list/grid — list with state, a sample in each font's own typeface, family, style, format and path; family grouping is a header row per family. Thumbnails are the sample column. Still open: a grid view
- [x] Search/filter — text search, state, format and variable/static filters in the GUI; the CLI has the same filters
- [x] Font detail panel — shows full name, version, vendor, type, units per em, source, embedding rights (fsType) and path, with the actions for the selected font; checked in a screenshot
- [x] Duplicate resolution view — identical copies can be resolved: keep one, review the dry-run plan, then confirm to trash the rest. Same-metadata groups stay report-only. The trash path is covered by a dry-run test only; it has not been run on real duplicates
- [x] Variable font preview view — opened from the library Preview action or the Preview tab (built in Phase 3)
- [x] Session status indicator — header shows the number of session fonts, a Deactivate all button, and the tray state with a Start button. A per-font list is in the library filter, not the header
- [x] Settings: autostart toggle, scan-directory configuration, light/dark theme — autostart and theme were already in; scan folders can now be added and removed in Settings, stored per user, and the library watcher rebuilds for the new set. Overlapping or relative folders are refused. Not yet run in a live window, and there is no folder picker (the path is typed or pasted)
- [ ] Drag-and-drop font file import — drop handler installs dropped fonts; verified by code review only, not by dragging a file

## Phase 9 — Archon Support (Design Only)
- [x] `docs/archon-design.md` — written as a design draft with a sketched trait. Based on the tpt-archon project page only: the raw README and source paths returned 404, so the trait names are placeholders
- [ ] Open Questions — nine questions are listed in docs/archon-design.md section 8, for the tpt-archon team to answer
- [x] Post-v1 `archon` Cargo feature flag plan — written as section 5 of the design doc; no code is added in v1

## Phase 10 — Performance & Polish
- [ ] Cold-start/catalog-load optimization — measured, not optimised. 10,000 fonts, release build: cold scan 1.5-2.3 s, cache load 23-31 ms, warm refresh 250-340 ms, row build 10 ms. Cache is 4.5 MB. Real fonts are larger, so cold scans will be slower. Not optimised until real-font data shows where the time goes
- [ ] Memory profiling (catalog index + tray resident process + WebView) — catalog scan of 10,000 fonts peaks at about 32 MB working set; tray idle about 10 MB. The GUI and WebView2 figure is not isolated yet: a process-tree measurement was attempted but blocked, and the total would include WebView2 used by other apps
- [ ] Light/dark theme support, accessibility pass — theme choice (system, light, dark) is in. A code audit fixed: control borders below 3:1 (new `--control-border` token), no visible focus on library rows, errors and session-count changes not announced, and motion without a reduced-motion guard. Text contrast in both themes meets WCAG AA. Still open: a real keyboard and screen-reader pass
- [ ] App icon, branding assets — placeholder icon in place
- [x] First-run onboarding — a dismissible notice explaining session versus permanent install, shown once per viewer
- [x] Crash reporting/local error logging (opt-in only) — local log only, in logs/tpt-fonts.log under the state directory, rotated at 1 MiB. Nothing is sent anywhere, so no opt-in is needed for v1; remote reporting is not built

## Phase 11 — Testing & QA
- [x] Unit tests: parsing engine, duplicate detection, session activation logic (mocked per-platform backends) — parsing in `core/tests/parsing.rs`, duplicates in `core/src/duplicates.rs` (9 tests), session logic with fake backends in `core/tests/session.rs`
- [x] Integration tests: end-to-end install/activate/deactivate/uninstall flows — `core/tests/install_flows.rs`, `session_flows.rs` and `win32_session_flows.rs`, with fake OS boundaries so they run on every platform
- [ ] Manual QA: Windows 10/11
- [ ] Manual QA: Linux (X11 + Wayland, at least one major distro)
- [x] Stress test: 10,000 scanned/installed fonts — catalog load time, search responsiveness, RAM usage — `core/tests/stress.rs` (ignored by default) builds 10,000 hard links to the 346 system fonts and measures them. Release build on Windows: cold scan 86 s (it hashes every file, about 10.6 GB of font data, so this is disk-bound; the 1.5–2.3 s cold figure above was measured on a different fixture and is not comparable), cache load 311 ms, warm refresh 253 ms, row build 35 ms, search 1.5–2.1 ms per query. Peak working set of the test process 55 MB, sampled every 100 ms. Not measured: the GUI and WebView2 under the same load
- [ ] Security review: malformed/malicious font file parsing (fuzzing) — mutation fuzzing done (core/tests/fuzz_parse.rs; 20,000 iterations pass in debug and release); parser panics are caught and reported as errors. Scan-directory traversal and IPC boundary reviewed by reading the code: scanning uses `follow_links(false)`; uninstall only acts on paths the backend lists itself; the tray caps request size at 64 KiB and activation rejects non-font paths. Containment checks in `win32.rs` and `linux.rs` now canonicalise both sides (`format::is_within`), with tests for `..` escapes and deleted files. The tray's named pipe now has a DACL that grants access only to the owner and SYSTEM (`PIPE_SDDL` in `tray-app/src/ipc_server.rs`). A CLI round trip works through it. PowerShell cannot read a pipe's security descriptor back, and no second account was tested. Open: confirm from a second Windows account that the pipe refuses access

## Phase 12 — Release v1
- [x] Finalize versioning scheme (SemVer) and changelog process — policy and CHANGELOG.md in place (Keep a Changelog format, 0.x rules stated)
- [ ] Windows installer (MSI/NSIS via Tauri bundler), autostart registration, code-signing decision — NSIS per-user installer configured with the tray and CLI as sidecars, and it builds locally (`npx tauri build --bundles nsis`, 2.46 MiB); OV code-signing chosen as the plan, not yet bought (see docs/release.md)
- [ ] Linux packaging (.deb/.rpm/AppImage via Tauri bundler), systemd user unit for the tray app — .deb and AppImage configured, systemd unit in packaging/linux with --cleanup as ExecStop; not built on Linux yet, no .rpm
- [x] Publish LICENSE files + notices in release artifacts — LICENSE-MIT and LICENSE-APACHE are listed under `bundle.resources` in `src-tauri/tauri.conf.json`, and the generated NSIS script installs and removes both. Not checked by running the built installer
- [x] User-facing docs (install guide, CLI reference, session-vs-permanent-install explainer) — docs/user-guide.md
- [ ] Tag v1.0.0 release, publish to GitHub (tpt-solutions org)
- [x] Post-release triage process for bug reports — `docs/triage.md`: intake, what to ask for, severity, labels, reproduction and a weekly review

---

## Backlog / Future Phases (post-v1)

- [ ] System-wide (all-users) elevated install/uninstall (`Windows\Fonts`, `/usr/share/fonts`)
- [ ] Project-linked font sets / auto-activation tied to project folders (Suitcase Fusion / FontExplorer style)
- [ ] WOFF/WOFF2 import & conversion to installable desktop formats
- [ ] Font pairing/recommendation suggestions
- [ ] Cloud font library integration (e.g. Google Fonts)
- [ ] Font licensing/embedding-rights advisory (deeper `fsType` analysis, usage warnings)
- [ ] macOS support
- [ ] Archon backend implementation (post design-only phase, pending public `tpt-archon` API)
