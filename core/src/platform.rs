// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::env;
use std::path::PathBuf;

use crate::model::{FontSource, ScanRoot};

/// The font directories the OS uses, limited to those that exist on this machine.
///
/// Windows: the per-user font folder and `%WINDIR%\Fonts` (read-only system view).
/// Linux: the XDG user font folder and the system `/usr/share/fonts` and `/usr/local/share/fonts`.
pub fn default_scan_roots() -> Vec<ScanRoot> {
    candidate_roots()
        .into_iter()
        .filter(|root| root.path.is_dir())
        .collect()
}

#[cfg(windows)]
fn candidate_roots() -> Vec<ScanRoot> {
    let mut roots = Vec::new();
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        roots.push(ScanRoot {
            path: PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
            source: FontSource::User,
        });
    }
    let windir = env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    roots.push(ScanRoot {
        path: PathBuf::from(windir).join("Fonts"),
        source: FontSource::System,
    });
    roots
}

#[cfg(not(windows))]
fn candidate_roots() -> Vec<ScanRoot> {
    let user = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share"))
        })
        .map(|data| data.join("fonts"));

    let mut roots: Vec<ScanRoot> = user
        .into_iter()
        .map(|path| ScanRoot {
            path,
            source: FontSource::User,
        })
        .collect();
    for system in ["/usr/share/fonts", "/usr/local/share/fonts"] {
        roots.push(ScanRoot {
            path: PathBuf::from(system),
            source: FontSource::System,
        });
    }
    roots
}

/// Whether a process with this ID is still running. Used to detect a crashed tray process.
#[cfg(windows)]
pub fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    // SAFETY: the handle is checked for null and closed before returning; the exit code
    // pointer refers to a local that lives for the call.
    unsafe {
        let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code: u32 = 0;
        let ok = GetExitCodeProcess(handle, &mut code) != 0;
        CloseHandle(handle);
        ok && code == STILL_ACTIVE as u32
    }
}

/// Whether a process with this ID is still running. Used to detect a crashed tray process.
#[cfg(not(windows))]
pub fn process_is_running(pid: u32) -> bool {
    std::path::Path::new("/proc").join(pid.to_string()).exists()
}

/// The XDG configuration directory (`$XDG_CONFIG_HOME`, falling back to `~/.config`).
/// Only Linux uses it; Windows autostart goes through the registry.
pub fn user_config_dir() -> Option<PathBuf> {
    xdg_or_home("XDG_CONFIG_HOME", ".config")
}

/// Per-user directory for tpt-fonts state such as the session file.
/// Windows: `%LOCALAPPDATA%\tpt-fonts`. Linux: `$XDG_STATE_HOME/tpt-fonts`.
pub fn app_state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("LOCALAPPDATA").map(|local| PathBuf::from(local).join("tpt-fonts"))
    }
    #[cfg(not(windows))]
    {
        xdg_or_home("XDG_STATE_HOME", ".local/state").map(|dir| dir.join("tpt-fonts"))
    }
}

fn xdg_or_home(var: &str, fallback: &str) -> Option<PathBuf> {
    env::var_os(var)
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
}
