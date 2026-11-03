// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::{FontError, Result};
use crate::format::is_font_extension;

/// A change to a font file under a watched directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontChange {
    /// The file was created or modified and should be (re)read.
    Upserted(PathBuf),
    /// The file was removed and should be dropped from the catalog.
    Removed(PathBuf),
}

/// Keeps filesystem watches alive. Dropping it stops the watches.
pub struct FontWatcher {
    _watcher: RecommendedWatcher,
}

impl FontWatcher {
    /// Watches each directory recursively. Changes arrive on the returned receiver.
    ///
    /// Only font files are reported; other filesystem noise is dropped here.
    pub fn start(dirs: &[&Path]) -> Result<(FontWatcher, Receiver<FontChange>)> {
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                for change in changes_from(&event) {
                    // The receiver being gone means the app is shutting down; nothing to do.
                    let _ = tx.send(change);
                }
            })
            .map_err(|e| watch_error(Path::new(""), e))?;

        for dir in dirs {
            watcher
                .watch(dir, RecursiveMode::Recursive)
                .map_err(|e| watch_error(dir, e))?;
        }
        Ok((FontWatcher { _watcher: watcher }, rx))
    }
}

fn changes_from(event: &notify::Event) -> Vec<FontChange> {
    event
        .paths
        .iter()
        .filter(|path| is_font_extension(path))
        .filter_map(|path| match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => Some(FontChange::Upserted(path.clone())),
            EventKind::Remove(_) => Some(FontChange::Removed(path.clone())),
            _ => None,
        })
        .collect()
}

fn watch_error(path: &Path, source: notify::Error) -> FontError {
    FontError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other(source),
    }
}
