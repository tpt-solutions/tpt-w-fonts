// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Turns a user's font query (a path, file name, family, full name or PostScript name) into
//! catalog entries.

use std::path::PathBuf;

use crate::error::{FontError, Result};
use crate::model::FontFile;

/// Every catalog entry matching `query`, compared case-insensitively.
///
/// A query matches a file by its full path or file name, or by any face's family, full name or
/// PostScript name.
pub fn find_matches<'a>(files: &'a [FontFile], query: &str) -> Vec<&'a FontFile> {
    let query = query.trim();
    files
        .iter()
        .filter(|file| matches_file(file, query))
        .collect()
}

/// The single catalog entry matching `query`.
///
/// Errors with [`FontError::NotFound`] when nothing matches, and [`FontError::Ambiguous`] listing
/// the candidate paths when more than one does.
pub fn resolve<'a>(files: &'a [FontFile], query: &str) -> Result<&'a FontFile> {
    let mut matches = find_matches(files, query);
    match matches.len() {
        0 => Err(FontError::NotFound(query.to_string())),
        1 => Ok(matches.remove(0)),
        _ => Err(FontError::Ambiguous {
            query: query.to_string(),
            paths: matches
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<PathBuf>>(),
        }),
    }
}

fn matches_file(file: &FontFile, query: &str) -> bool {
    let path_matches = file.path.to_string_lossy().eq_ignore_ascii_case(query)
        || file
            .path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(query));
    path_matches
        || file.faces.iter().any(|face| {
            face.family.eq_ignore_ascii_case(query)
                || face.full_name.eq_ignore_ascii_case(query)
                || face
                    .postscript_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(query))
        })
}
