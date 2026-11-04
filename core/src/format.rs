// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::model::FontFormat;

/// Detects the container format from the first four bytes of a file.
///
/// Returns `None` for anything that is not TrueType, OpenType/CFF or a collection.
pub fn detect_format(data: &[u8]) -> Option<FontFormat> {
    match data.get(0..4)? {
        [0x00, 0x01, 0x00, 0x00] | b"true" => Some(FontFormat::Ttf),
        b"OTTO" => Some(FontFormat::Otf),
        b"ttcf" => Some(FontFormat::Ttc),
        _ => None,
    }
}

/// A path in `dir` for `file_name` that does not exist yet.
///
/// Adds " (2)", " (3)" and so on before the extension when the plain name is taken.
pub(crate) fn unique_destination(dir: &Path, file_name: &OsStr) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused suffix exists")
}

/// Whether `path` resolves to a location inside `dir`.
///
/// Both sides are canonicalised, so `..` segments and symlinks that leave `dir` are caught.
/// A file that no longer exists is checked through its parent folder, so uninstalling an
/// already-deleted font still works. Returns `false` when `dir` itself cannot be resolved.
pub(crate) fn is_within(path: &Path, dir: &Path) -> bool {
    let Ok(dir) = dir.canonicalize() else {
        return false;
    };
    let resolved = match path.canonicalize() {
        Ok(resolved) => resolved,
        Err(_) => {
            let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
                return false;
            };
            match parent.canonicalize() {
                Ok(parent) => parent.join(name),
                Err(_) => return false,
            }
        }
    };
    resolved.starts_with(&dir)
}

/// Whether a path has an extension the scanner should consider a font candidate.
pub fn is_font_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["ttf", "otf", "ttc"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_truetype_by_version_tag() {
        assert_eq!(detect_format(&[0, 1, 0, 0, 9]), Some(FontFormat::Ttf));
    }

    #[test]
    fn detects_truetype_by_apple_tag() {
        assert_eq!(detect_format(b"true...."), Some(FontFormat::Ttf));
    }

    #[test]
    fn detects_cff_opentype() {
        assert_eq!(detect_format(b"OTTO...."), Some(FontFormat::Otf));
    }

    #[test]
    fn detects_collection() {
        assert_eq!(detect_format(b"ttcf...."), Some(FontFormat::Ttc));
    }

    #[test]
    fn rejects_unknown_and_short_input() {
        assert_eq!(detect_format(b"%PDF-1.7"), None);
        assert_eq!(detect_format(&[0, 1]), None);
    }

    #[test]
    fn is_within_accepts_files_inside_the_folder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let inside = dir.path().join("Inside.ttf");
        fs::write(&inside, b"x").expect("write");
        let missing = dir.path().join("gone.ttf");

        assert!(is_within(&inside, dir.path()));
        assert!(
            is_within(&missing, dir.path()),
            "deleted files check via the parent"
        );
    }

    #[test]
    fn is_within_rejects_dot_dot_escapes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let font_dir = dir.path().join("fonts");
        fs::create_dir_all(&font_dir).expect("mkdir");
        let outside = dir.path().join("outside.ttf");
        fs::write(&outside, b"x").expect("write");

        let escape = font_dir.join("..").join("outside.ttf");
        assert!(!is_within(&escape, &font_dir));
        let missing_escape = font_dir.join("..").join("gone.ttf");
        assert!(!is_within(&missing_escape, &font_dir));
    }

    #[test]
    fn matches_font_extensions_case_insensitively() {
        assert!(is_font_extension(Path::new("Arial.TTF")));
        assert!(is_font_extension(Path::new("a/b/Meiryo.ttc")));
        assert!(!is_font_extension(Path::new("notes.txt")));
        assert!(!is_font_extension(Path::new("no_extension")));
    }
}
