// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Starting the tray process at login.
//!
//! Windows uses the per-user `Run` key. Linux writes an XDG autostart `.desktop` entry. The
//! desktop-entry text and file handling are platform-neutral so they are tested everywhere.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{FontError, Result};

/// Name of the autostart entry, used as the Run value name and the `.desktop` file stem.
pub const AUTOSTART_NAME: &str = "tpt-fonts-tray";

/// The contents of an XDG autostart entry that launches `exe` at login.
pub fn desktop_entry(exe: &Path) -> String {
    // Desktop Entry Exec values must quote paths that contain spaces.
    let exec = exe.to_string_lossy().replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nType=Application\nName=tpt-fonts\nComment=Font manager tray process\nExec=\"{exec}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

// The desktop-entry helpers run only on Linux, but are compiled everywhere so their tests run on
// every platform. Allow them to be unused on Windows.
/// Writes or removes the autostart entry under `config_home/autostart`.
#[cfg_attr(windows, allow(dead_code))]
pub fn write_desktop_entry(config_home: &Path, enabled: bool, exe: &Path) -> Result<()> {
    let file = desktop_entry_path(config_home);
    if enabled {
        let dir = file.parent().unwrap_or(config_home);
        fs::create_dir_all(dir).map_err(|e| io_error(dir, e))?;
        fs::write(&file, desktop_entry(exe)).map_err(|e| io_error(&file, e))
    } else {
        match fs::remove_file(&file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_error(&file, e)),
        }
    }
}

/// Whether the autostart entry exists under `config_home`.
#[cfg_attr(windows, allow(dead_code))]
pub fn desktop_entry_exists(config_home: &Path) -> bool {
    desktop_entry_path(config_home).is_file()
}

#[cfg_attr(windows, allow(dead_code))]
fn desktop_entry_path(config_home: &Path) -> PathBuf {
    config_home
        .join("autostart")
        .join(format!("{AUTOSTART_NAME}.desktop"))
}

/// Enables or disables starting the tray at login for the current user.
pub fn set_autostart(enabled: bool, exe: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        windows_run_key::set(enabled, exe)
    }
    #[cfg(not(windows))]
    {
        let config = crate::platform::user_config_dir().ok_or_else(|| FontError::Backend {
            path: PathBuf::new(),
            reason: "no XDG config directory or HOME".into(),
        })?;
        write_desktop_entry(&config, enabled, exe)
    }
}

/// Whether autostart is currently enabled for the current user.
pub fn autostart_enabled() -> bool {
    #[cfg(windows)]
    {
        windows_run_key::is_set()
    }
    #[cfg(not(windows))]
    {
        crate::platform::user_config_dir().is_some_and(|config| desktop_entry_exists(&config))
    }
}

#[cfg_attr(windows, allow(dead_code))]
fn io_error(path: &Path, source: std::io::Error) -> FontError {
    FontError::Io {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(windows)]
mod windows_run_key {
    use std::path::Path;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};

    use super::AUTOSTART_NAME;
    use crate::error::{FontError, Result};

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn set(enabled: bool, exe: &Path) -> Result<()> {
        let key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_WRITE)
            .map_err(registry_error)?;
        if enabled {
            let command = format!("\"{}\"", exe.display());
            key.set_value(AUTOSTART_NAME, &command)
                .map_err(registry_error)
        } else {
            match key.delete_value(AUTOSTART_NAME) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(registry_error(e)),
            }
        }
    }

    pub fn is_set() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .and_then(|key| key.get_value::<String, _>(AUTOSTART_NAME))
            .is_ok()
    }

    fn registry_error(source: std::io::Error) -> FontError {
        FontError::Io {
            path: Path::new(RUN_KEY).to_path_buf(),
            source,
        }
    }
}
