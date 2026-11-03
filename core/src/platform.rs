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
