// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Shared helpers for integration tests.
//!
//! Font binaries are never committed. Tests look for real fonts already installed on the
//! machine, and skip with a message when they are absent (for example on a bare CI image).

use std::path::PathBuf;

use tpt_fonts_core::default_scan_roots;
use walkdir::WalkDir;

/// Finds an installed font file by exact file name within the OS font directories.
pub fn installed_font(file_name: &str) -> Option<PathBuf> {
    default_scan_roots()
        .into_iter()
        .flat_map(|root| WalkDir::new(root.path).into_iter().filter_map(Result::ok))
        .find(|entry| entry.file_name().eq_ignore_ascii_case(file_name))
        .map(|entry| entry.into_path())
}

/// Returns the font path, or prints a skip notice and returns `None`.
pub fn require_font(file_name: &str) -> Option<PathBuf> {
    let found = installed_font(file_name);
    if found.is_none() {
        eprintln!("skipping: {file_name} is not installed on this machine");
    }
    found
}
