// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Scale check for the catalog at 10,000 font files.
//!
//! The files are hard links to the system's own fonts, so the test needs little disk space but
//! still makes the scanner read and hash 10,000 separate paths. It is ignored by default because
//! it takes a while. Run it with:
//!
//! `cargo test -p tpt-fonts-core --release --test stress -- --ignored --nocapture`

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tpt_fonts_core::{Catalog, FontSource, ScanRoot, build_rows, default_scan_roots, find_matches};

const TARGET: usize = 10_000;
const FOLDERS: usize = 10;

/// Font files in the system's read-only font folders. These are the sources the links point to.
fn system_fonts() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in default_scan_roots() {
        if root.source != FontSource::System {
            continue;
        }
        for entry in fs::read_dir(&root.path).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_file() && tpt_fonts_core::is_font_extension(&path) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
#[ignore = "takes minutes of disk work; run with --ignored"]
fn ten_thousand_fonts() {
    let sources = system_fonts();
    assert!(
        !sources.is_empty(),
        "no system fonts found to build the set from"
    );

    let dir = tempfile::tempdir().expect("temp dir");
    let started = Instant::now();
    for i in 0..TARGET {
        let source = &sources[i % sources.len()];
        let folder = dir.path().join(format!("set{:02}", i % FOLDERS));
        fs::create_dir_all(&folder).expect("create folder");
        let name = format!(
            "copy{i:05}-{}",
            source.file_name().unwrap_or_default().to_string_lossy()
        );
        let dest = folder.join(name);
        if fs::hard_link(source, &dest).is_err() {
            fs::copy(source, &dest).expect("copy font");
        }
    }
    println!(
        "built {TARGET} files from {} system fonts in {:?}",
        sources.len(),
        started.elapsed()
    );

    let roots = vec![ScanRoot {
        path: dir.path().to_path_buf(),
        source: FontSource::User,
    }];

    let mut catalog = Catalog::default();
    let started = Instant::now();
    catalog.refresh(&roots);
    println!(
        "cold scan: {:?} for {} files",
        started.elapsed(),
        catalog.files().len()
    );
    assert_eq!(
        catalog.files().len(),
        TARGET,
        "every linked font should parse"
    );

    let cache = dir.path().join("catalog.json");
    catalog.save(&cache).expect("save cache");

    let started = Instant::now();
    let mut warm = Catalog::load(&cache);
    println!("cache load: {:?}", started.elapsed());

    let started = Instant::now();
    warm.refresh(&roots);
    println!("warm refresh: {:?}", started.elapsed());

    let started = Instant::now();
    let rows = build_rows(&warm, &[], &[]);
    println!("row build: {:?} for {} rows", started.elapsed(), rows.len());

    // Each query runs several times; the slowest run is the figure that matters for typing.
    let mut slowest = Duration::ZERO;
    for query in ["arial", "bold", "italic", "no-such-family-xyz"] {
        let mut worst = Duration::ZERO;
        let mut hits = 0;
        for _ in 0..5 {
            let started = Instant::now();
            hits = find_matches(warm.files(), query).len();
            worst = worst.max(started.elapsed());
        }
        slowest = slowest.max(worst);
        println!("search {query:?}: {hits} hits, slowest of 5 runs {worst:?}");
    }
    assert!(
        slowest < Duration::from_millis(500),
        "a search over 10,000 fonts took {slowest:?}"
    );
}
