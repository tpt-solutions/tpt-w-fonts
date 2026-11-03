// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::Path;

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
    fn matches_font_extensions_case_insensitively() {
        assert!(is_font_extension(Path::new("Arial.TTF")));
        assert!(is_font_extension(Path::new("a/b/Meiryo.ttc")));
        assert!(!is_font_extension(Path::new("notes.txt")));
        assert!(!is_font_extension(Path::new("no_extension")));
    }
}
