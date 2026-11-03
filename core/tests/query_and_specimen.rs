// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

mod common;

use std::path::PathBuf;

use tpt_fonts_core::{
    EmbeddingRights, FontError, FontFace, FontFile, FontFormat, FontSource, find_matches,
    render_svg, resolve,
};

use common::require_font;

fn face(family: &str, subfamily: &str, postscript: &str) -> FontFace {
    FontFace {
        index: 0,
        family: family.into(),
        subfamily: subfamily.into(),
        full_name: format!("{family} {subfamily}"),
        postscript_name: Some(postscript.into()),
        version: "1.0".into(),
        vendor_id: None,
        embedding: EmbeddingRights::Installable,
        units_per_em: 1000,
        is_variable: false,
        axes: vec![],
        named_instances: vec![],
    }
}

fn file(path: &str, face: FontFace) -> FontFile {
    FontFile {
        path: PathBuf::from(path),
        source: FontSource::User,
        format: FontFormat::Ttf,
        size: 1,
        modified_ms: 0,
        content_hash: path.into(),
        faces: vec![face],
    }
}

fn catalog() -> Vec<FontFile> {
    vec![
        file(
            r"C:\fonts\Alpha-Regular.ttf",
            face("Alpha", "Regular", "Alpha-Regular"),
        ),
        file(
            r"C:\fonts\Beta.otf",
            face("Beta Sans", "Bold", "BetaSans-Bold"),
        ),
        file(
            r"C:\other\Alpha-Regular.ttf",
            face("Alpha", "Regular", "Alpha-Regular"),
        ),
    ]
}

#[test]
fn matches_by_file_name_case_insensitively() {
    let files = catalog();
    assert_eq!(find_matches(&files, "beta.OTF").len(), 1);
}

#[test]
fn matches_by_family_full_name_and_postscript_name() {
    let files = catalog();
    assert_eq!(find_matches(&files, "beta sans").len(), 1);
    assert_eq!(find_matches(&files, "Beta Sans Bold").len(), 1);
    assert_eq!(find_matches(&files, "BetaSans-Bold").len(), 1);
}

#[test]
fn an_exact_full_path_disambiguates_duplicates_by_name() {
    let files = catalog();
    let matches = find_matches(&files, r"C:\fonts\Alpha-Regular.ttf");
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].path,
        PathBuf::from(r"C:\fonts\Alpha-Regular.ttf")
    );
}

#[test]
fn resolve_reports_ambiguity_with_the_candidate_paths() {
    let files = catalog();
    match resolve(&files, "Alpha") {
        Err(FontError::Ambiguous { paths, .. }) => assert_eq!(paths.len(), 2),
        other => panic!("expected an ambiguity error, got {other:?}"),
    }
}

#[test]
fn resolve_reports_no_match() {
    let files = catalog();
    assert!(matches!(
        resolve(&files, "Nonexistent"),
        Err(FontError::NotFound(q)) if q == "Nonexistent"
    ));
}

#[test]
fn resolve_returns_the_single_match() {
    let files = catalog();
    let found = resolve(&files, "Beta Sans").expect("one match");
    assert_eq!(found.path, PathBuf::from(r"C:\fonts\Beta.otf"));
}

#[test]
fn specimen_is_svg_with_outline_paths() {
    let Some(font) = require_font("arial.ttf") else {
        return;
    };
    let svg = render_svg(&font, 0, &[], "Hi", 48.0).expect("renders");
    assert!(svg.starts_with("<svg "));
    assert!(svg.contains("<path"));
    assert!(svg.contains(" d=\"M"), "outline data present");
}

#[test]
fn variation_axes_change_the_outlines() {
    let Some(font) = require_font("bahnschrift.ttf") else {
        return;
    };
    let light = render_svg(&font, 0, &[("wght".into(), 300.0)], "Hamburg", 72.0).expect("light");
    let bold = render_svg(&font, 0, &[("wght".into(), 800.0)], "Hamburg", 72.0).expect("bold");
    assert_ne!(light, bold, "a weight change must alter the outlines");
}

#[test]
fn unknown_axis_is_an_error_not_a_silent_default() {
    let Some(font) = require_font("bahnschrift.ttf") else {
        return;
    };
    let err = render_svg(&font, 0, &[("zzzz".into(), 1.0)], "x", 48.0).expect_err("no such axis");
    assert!(matches!(err, FontError::UnknownAxis(tag) if tag == "zzzz"));
}

#[test]
fn an_axis_tag_that_is_not_four_bytes_is_rejected() {
    let Some(font) = require_font("bahnschrift.ttf") else {
        return;
    };
    let err = render_svg(&font, 0, &[("wg".into(), 400.0)], "x", 48.0).expect_err("bad tag");
    assert!(matches!(err, FontError::UnknownAxis(_)));
}
