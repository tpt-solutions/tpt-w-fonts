# Release and packaging

Audience: maintainers who build and publish tpt-fonts.

## Current state

- Windows: an NSIS installer is produced by `npm run tauri build`. It installs per-user into
  `%LOCALAPPDATA%\Programs\tpt-fonts` and needs no administrator rights.
- Linux: `.deb` and AppImage bundles are configured but must be built on Linux. The `.deb` also
  installs the tray's systemd user unit (`packaging/linux/tpt-fonts-tray.service`).
- Signing: **not signed yet.** Unsigned installers trigger Windows SmartScreen warnings. See
  "Code signing" below.

## Building

Windows (from the repository root):

```sh
npm install
npm run tauri build
```

The build runs `scripts/prepare-sidecars.mjs` first. It builds `tpt-fonts-tray` and `tpt-fonts`
(the CLI) in release mode and copies them into `src-tauri/binaries/` with the target-triple names
Tauri expects. The installer then places them next to the GUI.

Linux:

```sh
npm install
npm run tauri build -- --bundles deb,appimage
```

Output lands in `target/release/bundle/`.

## Code signing

### What OV means

An **OV (Organization Validated)** code-signing certificate is issued by a certificate authority
after it has checked that the organisation legally exists. Signing with it proves who published
the installer and stops the file being altered in transit.

- It does **not** remove SmartScreen warnings immediately. Reputation builds as users install the
  signed app, so early downloads can still show a prompt.
- Since 2023, CA rules require the private key to sit on a hardware token or a cloud HSM. It
  cannot be exported as a plain file.
- An **EV (Extended Validation)** certificate has stricter checks and costs more. Microsoft no
  longer gives EV an immediate SmartScreen bypass, so OV is usually enough.
- **Azure Trusted Signing** is a cloud alternative. It uses an HSM Microsoft manages, has lower
  running costs, and needs an Azure identity check. Check eligibility before buying an OV
  certificate.

### Status and plan

- No certificate has been bought. Buying one needs a legal entity and identity checks that only
  the organisation can complete.
- Until then, release builds are unsigned. Label them as test builds on the download page.
- When a certificate exists, configure `src-tauri/tauri.conf.json` under `bundle.windows`. Either
  set `certificateThumbprint` and `timestampUrl` for a certificate in the Windows store, or set
  `signCommand` so Tauri calls `signtool` against a hardware token or cloud signing service.
  Signing both the installer and the binaries inside it is recommended.
- Use a timestamp server so signatures stay valid after the certificate expires.

## Linux packaging

- `.deb`: installs the GUI, the tray and the CLI into `/usr/bin` and the systemd user unit into
  `/usr/lib/systemd/user`. The unit's `ExecStop=... --cleanup` removes any session fonts the tray
  left behind.
- AppImage: a single file with no install step. It does not install the systemd unit, so users
  who want it should use the `.deb`, or copy the unit from `packaging/linux/` themselves.
- `.rpm` is not produced. Add it only if there is demand.

## Checklist before a release

1. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, and `npm run build` all pass.
2. The installer installs and uninstalls cleanly on Windows 10 and 11 without elevation.
3. A session activation survives the GUI closing and is removed when the tray quits.
4. The version in `Cargo.toml` (`workspace.package.version`) and `tauri.conf.json` agree.
5. The release notes list any known limitations. The current ones are in `todo.md`.
