// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::fs;
use std::time::Duration;

use tpt_fonts_core::{FontChange, FontWatcher};

#[test]
fn reports_font_file_creation_and_ignores_other_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (_watcher, changes) = FontWatcher::start(&[dir.path()]).expect("watch starts");

    fs::write(dir.path().join("notes.txt"), b"not a font").expect("write text");
    let font = dir.path().join("Fresh.otf");
    fs::write(&font, b"placeholder bytes").expect("write font");

    let timeout = Duration::from_secs(10);
    let mut seen_font = false;
    while let Ok(change) = changes.recv_timeout(timeout) {
        match change {
            FontChange::Upserted(path) => {
                assert!(
                    path.extension().is_some_and(|ext| ext == "otf"),
                    "only font files are reported, got {path:?}"
                );
                if path.ends_with("Fresh.otf") {
                    seen_font = true;
                    break;
                }
            }
            FontChange::Removed(_) => {}
        }
    }
    assert!(seen_font, "creating a font file emits an Upserted event");
}
