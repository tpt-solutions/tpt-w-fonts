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
| 6 | Resident Tray App & Session Manager | ❌ Not Started |
| 7 | CLI | ❌ Not Started |
| 8 | Tauri GUI: Main Application | ❌ Not Started |
| 9 | Archon Support (Design Only) | ❌ Not Started |
| 10 | Performance & Polish | ❌ Not Started |
| 11 | Testing & QA | ❌ Not Started |
| 12 | Release v1 | ❌ Not Started |

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
- [ ] Filesystem watch on font directories (`ReadDirectoryChangesW` / `inotify`) to keep the catalog live
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
- [ ] Session cleanup: `RemoveFontResourceEx` + `WM_FONTCHANGE` broadcast on tray app exit; handle abnormal-exit/logoff cleanup
- [x] Avoid unnecessary GDI font-cache rebuild churn when activating/deactivating — one WM_FONTCHANGE per batch, and none if nothing changed
- [x] Unit/integration tests against a mockable Win32 boundary (CI-safe) — 8 tests use a fake Win32 boundary; 1 opt-in real test (`--ignored`) passed

## Phase 5 — Linux Backend: Fontconfig & Session Activation
- [x] Linux user-level permanent install (`~/.local/share/fonts`, targeted `fc-cache` refresh) — copies to $XDG_DATA_HOME/fonts with a scoped `fc-cache -f`; tested with a fake runner, not yet on a real Linux desktop
- [x] Linux user-level uninstall (file removal + fontconfig cache refresh) — refuses paths outside the user directory; fake-runner tested
- [ ] Fontconfig cache management: inspect cache files, trigger scoped `fc-cache -f` on affected dirs only (avoid full-system rebuild)
- [x] Linux session activation: dedicated session font directory + scoped fontconfig `<include>` added/removed by the resident process, cache refresh on activate/deactivate — session copies in $XDG_CACHE_HOME/tpt-fonts/session plus a fontconfig drop-in in conf.d; fake-runner tested
- [ ] Session cleanup on resident-process exit/logout (systemd user unit `ExecStop` or signal handler) — `clear_session()` exists; the systemd unit and signal handler come with Phase 6/12
- [ ] Verify behavior on X11 and Wayland

## Phase 6 — Resident Tray App & Session Manager
- [ ] Cross-platform tray-app crate (native tray icon, Windows + Linux)
- [ ] Session font registry: tracks active session fonts, OS handles, activation source
- [ ] Tray menu: quick activate/deactivate list, open main window, quit (clean deactivation on quit)
- [ ] Optional user-toggleable autostart (Windows Run key/Startup folder, Linux `.desktop` autostart)
- [ ] Orphaned-resource recovery: detect stale session registrations left by a crash/abnormal exit on next launch and clean them up
- [ ] IPC between tray-app resident process and Tauri main window (local socket/named pipe) so the GUI reflects live session state

## Phase 7 — CLI
- [ ] `tpt-fonts list` (installed + scanned fonts; filters by family/style/format/state)
- [ ] `tpt-fonts activate <font> --session` / `tpt-fonts deactivate <font>`
- [ ] `tpt-fonts install <file> [--user]` / `tpt-fonts uninstall <font>`
- [ ] `tpt-fonts scan-duplicates [--fix | --dry-run]`
- [ ] `tpt-fonts preview <font> --axis wght=700 --text "..."` (headless preview/specimen export)
- [ ] CLI talks to the same `core` engine (and resident process for session state) used by the GUI
- [ ] CLI unit/integration tests

## Phase 8 — Tauri GUI: Main Application
- [ ] Main window: font catalog list/grid (family grouping, style variants, thumbnail preview)
- [ ] Search/filter (family name, format, variable vs static, installed/session/available state)
- [ ] Font detail panel: metadata, embedding rights (`fsType`), install/activate/session actions
- [ ] Duplicate resolution view (Phase 2)
- [ ] Variable font preview view (Phase 3)
- [ ] Session status indicator: active session fonts, quick deactivate
- [ ] Settings: autostart toggle, scan-directory configuration, light/dark theme
- [ ] Drag-and-drop font file import

## Phase 9 — Archon Support (Design Only)
- [ ] `docs/archon-design.md`: design for memory-mapped, read-only, page-cache-backed shared font objects; `trait ArchonFontBackend` so apps bind to font capabilities without duplicating font data in RAM
- [ ] Open Questions (pending tpt-archon team): mmap/shared-object syscall interface, capability grant/revocation model for font handles, per-app binding performance targets, interaction with session-scoped activation model
- [ ] Post-v1 `archon` Cargo feature flag plan (`ArchonFontBackend` stub, `is_available() -> false`, `Unsupported` errors until implemented)

## Phase 10 — Performance & Polish
- [ ] Cold-start/catalog-load optimization targeting the spec's 10,000-installed-fonts scenario
- [ ] Memory profiling (catalog index + tray resident process + WebView)
- [ ] Light/dark theme support, accessibility pass (keyboard navigation, screen reader labels)
- [ ] App icon, branding assets (TPT Solutions)
- [ ] First-run onboarding (initial scan progress, explain session vs. permanent install)
- [ ] Crash reporting/local error logging (opt-in only)

## Phase 11 — Testing & QA
- [ ] Unit tests: parsing engine, duplicate detection, session activation logic (mocked per-platform backends)
- [ ] Integration tests: end-to-end install/activate/deactivate/uninstall flows
- [ ] Manual QA: Windows 10/11
- [ ] Manual QA: Linux (X11 + Wayland, at least one major distro)
- [ ] Stress test: 10,000 scanned/installed fonts — catalog load time, search responsiveness, RAM usage
- [ ] Security review: malformed/malicious font file parsing (fuzzing), path traversal in scan directories, IPC boundary (tray↔GUI, CLI↔core)

## Phase 12 — Release v1
- [ ] Finalize versioning scheme (SemVer) and changelog process
- [ ] Windows installer (MSI/NSIS via Tauri bundler), autostart registration, code-signing decision
- [ ] Linux packaging (.deb/.rpm/AppImage via Tauri bundler), systemd user unit for the tray app
- [ ] Publish LICENSE files + notices in release artifacts
- [ ] User-facing docs (install guide, CLI reference, session-vs-permanent-install explainer)
- [ ] Tag v1.0.0 release, publish to GitHub (tpt-solutions org)
- [ ] Post-release triage process for bug reports

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
