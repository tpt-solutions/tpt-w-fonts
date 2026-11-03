// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

mod common;

use std::fs;

use tpt_fonts_core::{Catalog, FontSource, ScanRoot};

use common::require_font;

fn root(path: &std::path::Path) -> Vec<ScanRoot> {
    vec![ScanRoot {
        path: path.to_path_buf(),
        source: FontSource::User,
    }]
}

#[test]
fn refresh_adds_fonts_and_reports_unreadable_candidates() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    fs::copy(&sample, dir.path().join("Arial.ttf")).expect("copy sample");
    fs::write(dir.path().join("broken.otf"), b"nope").expect("write broken");
    fs::write(dir.path().join("readme.txt"), b"ignored").expect("write text");

    let mut catalog = Catalog::default();
    let report = catalog.refresh(&root(dir.path()));

    assert_eq!(report.added, 1);
    assert_eq!(report.updated, 0);
    assert_eq!(report.removed, 0);
    assert_eq!(report.errors.len(), 1, "only the broken .otf is reported");
    assert_eq!(catalog.files().len(), 1);
}

#[test]
fn unchanged_files_are_not_reread() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    fs::copy(&sample, dir.path().join("Arial.ttf")).expect("copy sample");

    let mut catalog = Catalog::default();
    catalog.refresh(&root(dir.path()));
    let second = catalog.refresh(&root(dir.path()));

    assert_eq!(second.added + second.updated + second.removed, 0);
    assert_eq!(catalog.files().len(), 1);
}

#[test]
fn deleted_files_are_removed_from_the_catalog() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("Arial.ttf");
    fs::copy(&sample, &file).expect("copy sample");

    let mut catalog = Catalog::default();
    catalog.refresh(&root(dir.path()));
    fs::remove_file(&file).expect("remove");
    let report = catalog.refresh(&root(dir.path()));

    assert_eq!(report.removed, 1);
    assert!(catalog.files().is_empty());
}

#[test]
fn catalog_round_trips_through_disk() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    fs::copy(&sample, dir.path().join("Arial.ttf")).expect("copy sample");
    let cache = dir.path().join("cache").join("catalog.json");

    let mut catalog = Catalog::default();
    catalog.refresh(&root(dir.path()));
    catalog.save(&cache).expect("save");

    let loaded = Catalog::load(&cache);
    assert_eq!(loaded.files(), catalog.files());
}

#[test]
fn missing_or_corrupt_cache_loads_as_empty() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(
        Catalog::load(&dir.path().join("absent.json"))
            .files()
            .is_empty()
    );

    let corrupt = dir.path().join("corrupt.json");
    fs::write(&corrupt, b"{ not json").expect("write");
    assert!(Catalog::load(&corrupt).files().is_empty());
}

#[test]
fn outdated_schema_is_discarded() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");
    fs::copy(&sample, dir.path().join("Arial.ttf")).expect("copy sample");
    let cache = dir.path().join("catalog.json");

    let mut catalog = Catalog::default();
    catalog.refresh(&root(dir.path()));
    catalog.save(&cache).expect("save");
    let stale = fs::read_to_string(&cache).expect("read").replacen(
        "\"schema_version\":1",
        "\"schema_version\":0",
        1,
    );
    fs::write(&cache, stale).expect("write stale");

    assert!(Catalog::load(&cache).files().is_empty());
}
