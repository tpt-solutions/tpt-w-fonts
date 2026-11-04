# Changelog

All notable changes to tpt-fonts are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Versioning

tpt-fonts uses [Semantic Versioning](https://semver.org/). Until 1.0.0, minor versions may change
behaviour; patch versions only fix bugs. From 1.0.0, a change to the CLI's commands or options, or
to the on-disk state files, is a major version change.

## [Unreleased]

Pre-release. The first tagged release will be 1.0.0.

### Added

- Font library with per-row samples in each font's own typeface, search, state, format,
  variable/static filters, and grouping by family.
- Live preview for variable fonts: axis sliders, named instances, saved combinations, sample text,
  size, and light or dark background.
- Duplicate detection by content hash and by metadata, with a keep-one flow that uses a dry-run
  plan and sends extra copies to the trash.
- Session activation through a resident tray process, with a single-instance check, recovery after
  a crash, cleanup on quit, and cleanup on Windows logoff.
- Per-user install and uninstall on Windows (registry plus font folder) and Linux (fontconfig).
- `tpt-fonts` command-line tool: `list`, `activate`, `deactivate`, `install`, `uninstall`,
  `scan-duplicates`, and `preview` (SVG specimens).
- Library refreshes when font folders change.
- Start at login, from the tray menu or Settings, on Windows (Run key) and Linux (autostart entry).
- First-run notice explaining session and permanent use.
- Local error log in the per-user state directory, rotated at 1 MB.
- NSIS per-user installer for Windows. Unsigned.
- `.deb` and AppImage packaging configuration for Linux, and a systemd user unit for the tray.

### Security

- The font parser is fuzzed with corrupted real fonts. Panics from malformed input are reported as
  errors in every build profile.
- Duplicate removal sends extra copies to the trash; nothing is permanently deleted that way.
  Uninstall removes the installed copy from your user font folder, and only there.
- Files outside the user font folder are never changed. System fonts are read-only.

### Known limitations

- Windows installers are not code-signed; SmartScreen will warn.
- Linux packages have not yet been built or tested on real desktops.
- Install is per-user only. Only TrueType, OpenType and `.ttc` files are supported.
- Specimens are SVG only.
