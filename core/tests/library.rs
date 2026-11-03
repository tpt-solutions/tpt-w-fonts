// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::PathBuf;

use tpt_fonts_core::{
    Catalog, EmbeddingRights, FontFace, FontFile, FontFormat, FontSource, FontState, build_rows,
};

fn file(path: &str, family: &str, subfamily: &str) -> FontFile {
    FontFile {
        path: PathBuf::from(path),
        source: FontSource::User,
        format: FontFormat::Ttf,
        size: 1,
        modified_ms: 0,
        content_hash: path.into(),
        faces: vec![FontFace {
            index: 0,
            family: family.into(),
            subfamily: subfamily.into(),
            full_name: format!("{family} {subfamily}"),
            postscript_name: None,
            version: "1".into(),
            vendor_id: None,
            embedding: EmbeddingRights::Installable,
            units_per_em: 1000,
            is_variable: false,
            axes: vec![],
            named_instances: vec![],
        }],
    }
}

fn catalog() -> Catalog {
    Catalog::from_files(vec![
        file(r"C:\u\Zeta-Bold.ttf", "Zeta", "Bold"),
        file(r"C:\u\Alpha.ttf", "alpha", "Regular"),
        file(r"C:\u\Zeta-Regular.ttf", "Zeta", "Regular"),
    ])
}

#[test]
fn rows_are_sorted_by_family_then_subfamily_ignoring_case() {
    let rows = build_rows(&catalog(), &[], &[]);
    let order: Vec<&str> = rows.iter().map(|r| r.family.as_str()).collect();
    assert_eq!(order, vec!["alpha", "Zeta", "Zeta"]);
    assert_eq!(rows[1].subfamily, "Bold");
    assert_eq!(rows[2].subfamily, "Regular");
}

#[test]
fn state_reflects_session_and_installed_lists() {
    let session = vec![PathBuf::from(r"C:\u\Alpha.ttf")];
    let installed = vec![PathBuf::from(r"C:\u\Zeta-Bold.ttf")];
    let rows = build_rows(&catalog(), &session, &installed);

    let state_of = |path: &str| {
        rows.iter()
            .find(|r| r.path.as_path() == std::path::Path::new(path))
            .map(|r| r.state)
            .expect("row exists")
    };
    assert_eq!(state_of(r"C:\u\Alpha.ttf"), FontState::Session);
    assert_eq!(state_of(r"C:\u\Zeta-Bold.ttf"), FontState::Installed);
    assert_eq!(state_of(r"C:\u\Zeta-Regular.ttf"), FontState::Available);
}

#[test]
fn installed_wins_over_session_for_the_same_file() {
    let path = PathBuf::from(r"C:\u\Alpha.ttf");
    let rows = build_rows(
        &catalog(),
        std::slice::from_ref(&path),
        std::slice::from_ref(&path),
    );
    let alpha = rows.iter().find(|r| r.family == "alpha").expect("row");
    assert_eq!(alpha.state, FontState::Installed);
}
