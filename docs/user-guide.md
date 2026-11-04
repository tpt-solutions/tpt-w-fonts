# tpt-fonts user guide

tpt-fonts is a font manager for designers and developers. It shows every font on your computer, lets you preview them, and lets you use a font in one of two ways:

- **Session:** the font is available to every application until you log out or quit the tray. Nothing is copied or registered permanently.
- **Install:** the font is copied into your user font folder and stays installed until you uninstall it.

Version 1 works on Windows 10 and 11, and on Linux. Everything is per user. Nothing needs administrator rights, and system fonts are read-only.

## Installing (Windows)

1. Run `tpt-fonts_<version>_x64-setup.exe`.
2. Windows may show "Windows protected your PC" because the installer is not yet code-signed. Choose **More info**, then **Run anyway**.
3. The app installs to `%LOCALAPPDATA%\Programs\tpt-fonts`. It adds no administrator entries.

To remove it, use **Apps** in Windows Settings, or run `uninstall.exe` in the install folder.

## The app window

The header shows whether the tray is running and how many fonts are active for this session. **Deactivate all** removes every session font at once.

- **Library:** every font file on your computer, with its state (*Available*, *Session* or *Installed*). Each row shows a sample in that font's own typeface. Use the search box, the state, format and variable/static filters, and **Group by family** to find fonts. Select a row to see its details and the actions for it.
- **Preview:** opens one font with live controls. Use the sliders for variable fonts, the named instances, the sample text and size, and the light or dark background. Type a file path to preview a font that is not in the library.
- **Duplicates:** finds identical copies and fonts that share the same family, style, version and vendor. For identical copies, choose **Keep this one**, review the list of what would go to the Recycle Bin, then confirm. Nothing is permanently deleted.
- **Settings:** start the tray when you log in, and choose a light, dark or system theme.

You can also drag `.ttf`, `.otf` or `.ttc` files onto the window to install them.

When a font file is added to or removed from your font folders, the library updates by itself.

## Session fonts and the tray

Session activation needs the **tray process** (`tpt-fonts-tray`) to be running. The tray:

- keeps track of the fonts you activated, so they can be removed later;
- can be started from the app with **Start tray**, or from *Start at login* in its menu;
- removes all session fonts when you quit it from its menu, and when you log out.

If the tray stops unexpectedly, for example because of a crash or a power cut, the next start cleans up the fonts it left behind. Only one tray runs at a time.

On Linux, the tray can run as a systemd user unit (`packaging/linux/tpt-fonts-tray.service`). Its stop step clears session fonts once the tray has stopped.

## Command line

The `tpt-fonts` command runs the same operations from a terminal. Session commands need the tray running.

| Command | What it does |
|---|---|
| `tpt-fonts list` | Lists fonts with their state. Add `--family TEXT`, `--format ttf\|otf\|ttc`, `--kind variable\|static`, `--state installed\|session\|available`, or `--json`. |
| `tpt-fonts activate FONT --session` | Makes a font available for this login session. `FONT` is a file path, file name, family, full name or PostScript name. |
| `tpt-fonts deactivate FONT` | Removes a font from the session. |
| `tpt-fonts install FILE [--user]` | Installs a font for the current user. `--user` is accepted for clarity; all installs are per-user in this version. |
| `tpt-fonts uninstall FONT` | Removes a font installed with `install`. |
| `tpt-fonts scan-duplicates [--dry-run \| --fix]` | Reports duplicate fonts. `--dry-run` shows what `--fix` would send to the trash. `--fix` trashes extra copies of identical fonts and keeps one. |
| `tpt-fonts preview FONT [--face N] [--axis TAG=VALUE] [--text TEXT] [--size PX] [--out FILE]` | Writes an SVG specimen of the text. `--axis` can be repeated, for example `--axis wght=700`. |

Run `tpt-fonts --help` or `tpt-fonts COMMAND --help` for the full list of options.

Exit status is `0` on success, `1` for an error such as a font that is not found or a tray that is not running, and `2` for a usage error such as a malformed `--axis`.

## Troubleshooting

- **"The tray process is not running":** start it from the app with **Start tray**, or run `tpt-fonts-tray` from the install folder.
- **A font shows as "cannot load this file":** the file may be damaged or not a font. The library reports how many files could not be read; the reasons are not logged yet.
- **Some fonts are missing from the library:** they may be in a folder the app does not scan. The app scans your user font folder and the system font folder.
- **Anything else:** the log is at `%LOCALAPPDATA%\tpt-fonts\logs\tpt-fonts.log` on Windows, or `$XDG_STATE_HOME/tpt-fonts/logs/tpt-fonts.log` on Linux. It records errors and routine tray events; nothing is sent anywhere. The file rotates at 1 MB, and the previous copy is kept as `tpt-fonts.log.1`.

## Limits in this version

- Install is per-user only. System-wide installs are planned for a later version.
- Only TrueType, OpenType and collection (`.ttc`) fonts are supported. WOFF and WOFF2 are not.
- Specimens are SVG only. PNG export is not available.
- Linux support has not yet been tested on real desktops.
- Installers are not code-signed yet.
