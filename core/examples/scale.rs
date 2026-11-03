// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Timing for the library at scale: cold scan, cache load, warm refresh, row building and
//! duplicate grouping. Run with `cargo run --release -p tpt-fonts-core --example scale -- <dir>`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tpt_fonts_core::{Catalog, FontSource, ScanRoot, build_rows, find_duplicates};

fn timed<T>(label: &str, work: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let value = work();
    let elapsed = start.elapsed();
    println!("{label:<34} {:>10.1} ms", elapsed.as_secs_f64() * 1000.0);
    (value, elapsed)
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: scale <directory of font files>"),
    );
    let roots = vec![ScanRoot {
        path: dir.clone(),
        source: FontSource::User,
    }];
    let cache = std::env::temp_dir().join("tpt-fonts-scale-cache.json");
    let _ = std::fs::remove_file(&cache);

    let mut catalog = Catalog::load(&cache);
    let (report, _) = timed("cold scan (no cache)", || catalog.refresh(&roots));
    println!(
        "  added {}, updated {}, errors {}",
        report.added,
        report.updated,
        report.errors.len()
    );
    timed("save cache", || catalog.save(&cache).expect("save"));
    let size = std::fs::metadata(&cache).map(|m| m.len()).unwrap_or(0);
    println!("  cache file: {:.1} MB", size as f64 / 1_048_576.0);

    let (mut reloaded, _) = timed("cache load", || Catalog::load(&cache));
    let (warm, _) = timed("warm refresh (stat only)", || reloaded.refresh(&roots));
    println!("  re-read {}", warm.added + warm.updated);

    let (rows, _) = timed("build library rows", || build_rows(&reloaded, &[], &[]));
    println!("  rows {}", rows.len());

    let (groups, _) = timed("find duplicates", || find_duplicates(&reloaded));
    println!("  groups {}", groups.len());

    let _ = std::fs::remove_file(&cache);
}
