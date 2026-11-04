// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Extra folders the user adds to the library, on top of the OS font folders.
//!
//! The list is stored per user in the app state directory. Each folder is checked when it is
//! added so that no file can be scanned twice: a folder may not overlap an OS folder or another
//! extra folder, whether it sits inside one or contains one.

use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{FontError, Result};
use crate::model::{FontSource, ScanRoot};
use crate::platform::{app_state_dir, default_scan_roots};

/// Bumped whenever the file layout changes, so an outdated file is ignored rather than misread.
const SCHEMA_VERSION: u32 = 1;

const FILE_NAME: &str = "scan-dirs.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoredDirs {
    schema_version: u32,
    dirs: Vec<PathBuf>,
}

/// What the Settings view shows: the OS folders, which are fixed, and the user's folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDirSettings {
    pub defaults: Vec<PathBuf>,
    pub extra: Vec<PathBuf>,
}

/// The scan roots for this user: the OS folders followed by the user's folders.
pub fn scan_roots() -> Vec<ScanRoot> {
    let mut roots = default_scan_roots();
    if let Ok(path) = settings_file() {
        for dir in load_extra_dirs(&path)
            .into_iter()
            .filter(|dir| dir.is_dir())
        {
            roots.push(ScanRoot {
                path: dir,
                source: FontSource::User,
            });
        }
    }
    roots
}

/// The OS folders and the user's folders, for display.
pub fn scan_dir_settings() -> Result<ScanDirSettings> {
    let extra = load_extra_dirs(&settings_file()?);
    Ok(ScanDirSettings {
        defaults: default_scan_roots().into_iter().map(|r| r.path).collect(),
        extra,
    })
}

/// Adds a folder to the user's list after checking it.
pub fn add_scan_dir(dir: &Path) -> Result<()> {
    let path = settings_file()?;
    let mut extra = load_extra_dirs(&path);
    let defaults: Vec<PathBuf> = default_scan_roots().into_iter().map(|r| r.path).collect();
    check_new_dir(dir, &defaults, &extra)?;
    extra.push(dir.to_path_buf());
    save_extra_dirs(&path, &extra)
}

/// Removes a folder from the user's list. Nothing on disk is touched.
pub fn remove_scan_dir(dir: &Path) -> Result<()> {
    let path = settings_file()?;
    let mut extra = load_extra_dirs(&path);
    let before = extra.len();
    extra.retain(|known| known != dir);
    if extra.len() == before {
        return Err(FontError::Usage(format!(
            "{} is not in the scan folder list",
            dir.display()
        )));
    }
    save_extra_dirs(&path, &extra)
}

/// Checks a folder before it joins the list. `defaults` and `existing` are the roots already
/// scanned, so an overlap with either is refused.
fn check_new_dir(dir: &Path, defaults: &[PathBuf], existing: &[PathBuf]) -> Result<()> {
    let refuse = |reason: String| FontError::Usage(format!("{}: {reason}", dir.display()));
    if !dir.is_absolute() {
        return Err(refuse("give the full path to the folder".into()));
    }
    if dir.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(refuse("the path must not contain '..'".into()));
    }
    if !dir.is_dir() {
        return Err(refuse("this is not an existing folder".into()));
    }
    for root in defaults.iter().chain(existing) {
        if overlaps(dir, root) {
            return Err(refuse(format!(
                "it overlaps {}, which is already scanned",
                root.display()
            )));
        }
    }
    Ok(())
}

/// Whether one path is the other or sits inside it. Windows paths compare without case.
fn overlaps(a: &Path, b: &Path) -> bool {
    let (a, b) = (comparable(a), comparable(b));
    a.starts_with(&b) || b.starts_with(&a)
}

fn comparable(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}

/// The user's folders. A missing or outdated file means none.
fn load_extra_dirs(path: &Path) -> Vec<PathBuf> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<StoredDirs>(&bytes).ok())
        .filter(|stored| stored.schema_version == SCHEMA_VERSION)
        .map(|stored| stored.dirs)
        .unwrap_or_default()
}

/// Writes the list atomically: a temporary file is written, then renamed over the target.
fn save_extra_dirs(path: &Path, dirs: &[PathBuf]) -> Result<()> {
    let io_err = |source| FontError::Io {
        path: path.to_path_buf(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    let stored = StoredDirs {
        schema_version: SCHEMA_VERSION,
        dirs: dirs.to_vec(),
    };
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, serde_json::to_vec_pretty(&stored)?).map_err(io_err)?;
    fs::rename(&tmp, path).map_err(io_err)
}

fn settings_file() -> Result<PathBuf> {
    app_state_dir()
        .map(|dir| dir.join(FILE_NAME))
        .ok_or_else(|| FontError::Backend {
            path: PathBuf::new(),
            reason: "no per-user state directory could be determined".into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(parent: &Path, name: &str) -> PathBuf {
        let path = parent.join(name);
        fs::create_dir_all(&path).expect("creates folder");
        path
    }

    #[test]
    fn a_fresh_folder_is_accepted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fonts = dir(tmp.path(), "Fonts");
        assert!(check_new_dir(&fonts, &[], &[]).is_ok());
    }

    #[test]
    fn relative_and_missing_folders_are_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(check_new_dir(Path::new("fonts"), &[], &[]).is_err());
        let missing = tmp.path().join("missing");
        assert!(check_new_dir(&missing, &[], &[]).is_err());
    }

    #[test]
    fn parent_segments_are_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fonts = dir(tmp.path(), "Fonts");
        let sneaky = fonts.join("..").join("Fonts");
        assert!(check_new_dir(&sneaky, &[], &[]).is_err());
    }

    #[test]
    fn overlap_with_an_os_folder_is_refused_in_either_direction() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let os = dir(tmp.path(), "os");
        let inside = dir(&os, "inside");
        assert!(check_new_dir(&inside, std::slice::from_ref(&os), &[]).is_err());
        assert!(check_new_dir(&os, std::slice::from_ref(&inside), &[]).is_err());
        assert!(check_new_dir(&os, std::slice::from_ref(&os), &[]).is_err());
    }

    #[test]
    fn overlap_with_an_existing_extra_folder_is_refused() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let extra = dir(tmp.path(), "extra");
        let inside = dir(&extra, "inside");
        assert!(check_new_dir(&inside, &[], std::slice::from_ref(&extra)).is_err());
        let sibling = dir(tmp.path(), "sibling");
        assert!(check_new_dir(&sibling, &[], std::slice::from_ref(&extra)).is_ok());
    }

    #[test]
    fn the_list_round_trips_through_its_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("state").join(FILE_NAME);
        assert!(load_extra_dirs(&file).is_empty(), "missing file means none");

        let dirs = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        save_extra_dirs(&file, &dirs).expect("saves");
        assert_eq!(load_extra_dirs(&file), dirs);
    }

    #[test]
    fn an_outdated_file_is_ignored() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join(FILE_NAME);
        fs::write(&file, r#"{"schema_version":0,"dirs":["/a"]}"#).expect("writes");
        assert!(load_extra_dirs(&file).is_empty());
    }
}
