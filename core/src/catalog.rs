// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::error::{FontError, Result};
use crate::format::is_font_extension;
use crate::model::{FontFile, ScanRoot};
use crate::parse::read_font_file;

/// Bumped whenever the cache layout changes, so stale caches are discarded rather than misread.
const SCHEMA_VERSION: u32 = 1;

/// The in-memory catalog of scanned font files, persisted to disk for fast cold starts.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    schema_version: u32,
    files: Vec<FontFile>,
}

/// What changed during a [`Catalog::refresh`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RefreshReport {
    /// Files newly read because they were not in the catalog.
    pub added: usize,
    /// Files re-read because their size or modification time changed.
    pub updated: usize,
    /// Catalog entries dropped because their file is gone.
    pub removed: usize,
    /// Files that could not be read or parsed, with the reason.
    pub errors: Vec<(PathBuf, String)>,
}

impl Catalog {
    /// Builds a catalog from already-parsed entries, for example in tests or after a merge.
    pub fn from_files(files: Vec<FontFile>) -> Catalog {
        Catalog {
            schema_version: SCHEMA_VERSION,
            files,
        }
    }

    pub fn files(&self) -> &[FontFile] {
        &self.files
    }

    /// Loads a cache from disk. A missing, unreadable or outdated cache yields an empty catalog,
    /// so the next refresh rebuilds it rather than failing startup.
    pub fn load(path: &Path) -> Catalog {
        let cached: Option<Catalog> = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        match cached {
            Some(catalog) if catalog.schema_version == SCHEMA_VERSION => catalog,
            _ => Catalog {
                schema_version: SCHEMA_VERSION,
                files: Vec::new(),
            },
        }
    }

    /// Writes the cache atomically: a temporary file is written, then renamed over the target.
    pub fn save(&self, path: &Path) -> Result<()> {
        let io_err = |source| FontError::Io {
            path: path.to_path_buf(),
            source,
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, serde_json::to_vec(self)?).map_err(io_err)?;
        fs::rename(&tmp, path).map_err(io_err)
    }

    /// Brings the catalog in line with the files under `roots`.
    ///
    /// Unchanged files (same size and modification time) keep their cached metadata and are not
    /// re-read. Entries whose files have disappeared are removed.
    pub fn refresh(&mut self, roots: &[ScanRoot]) -> RefreshReport {
        let mut report = RefreshReport::default();
        let mut previous: HashMap<PathBuf, FontFile> = std::mem::take(&mut self.files)
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();

        for root in roots {
            for path in candidate_paths(&root.path) {
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                let modified_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as u64);

                let was_known = previous.contains_key(&path);
                let cached = previous.remove(&path).filter(|old| {
                    old.size == meta.len()
                        && old.modified_ms == modified_ms
                        && old.source == root.source
                });
                match cached {
                    Some(file) => self.files.push(file),
                    None => match read_font_file(&path, root.source) {
                        Ok(file) => {
                            if was_known {
                                report.updated += 1;
                            } else {
                                report.added += 1;
                            }
                            self.files.push(file);
                        }
                        Err(err) => report.errors.push((path, err.to_string())),
                    },
                }
            }
        }

        report.removed = previous.len();
        self.schema_version = SCHEMA_VERSION;
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        report
    }
}

/// Font files under a root, in a stable order. Unreadable directory entries are skipped.
fn candidate_paths(root: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file() && is_font_extension(entry.path()))
        .map(|entry| entry.into_path())
}

/// Loads the per-user catalog cache, refreshes it against the OS font folders, and saves it.
///
/// Returns the refreshed catalog and the refresh report. The cache lives in the app state
/// directory, so repeated calls are cheap: unchanged files are not re-read.
pub fn load_default_catalog() -> Result<(Catalog, RefreshReport)> {
    let dir = crate::platform::app_state_dir().ok_or_else(|| FontError::Backend {
        path: PathBuf::new(),
        reason: "no per-user state directory could be determined".into(),
    })?;
    let cache = dir.join("catalog.json");
    let mut catalog = Catalog::load(&cache);
    let report = catalog.refresh(&crate::scan_dirs::scan_roots());
    catalog.save(&cache)?;
    Ok((catalog, report))
}
