// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Mutation fuzzing of the font parser.
//!
//! Real fonts are corrupted in random ways (byte flips, truncation, header damage, hostile values)
//! and fed to the format detector and the face parser. The parser must return an error or a value;
//! it must never panic. The run is deterministic, so a failure can be reproduced from the seed.
//!
//! Iterations default to a quick count for CI. For a longer run:
//! `TPT_FUZZ_ITERATIONS=50000 cargo test -p tpt-fonts-core --test fuzz_parse -- --nocapture`

mod common;

use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};

use tpt_fonts_core::{detect_format, parse_faces};

use common::require_font;

/// A small xorshift generator. Deterministic, so every failure can be replayed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

/// Applies one of several corruption strategies to `data`.
fn mutate(data: &mut Vec<u8>, rng: &mut Rng) {
    if data.is_empty() {
        return;
    }
    match rng.below(5) {
        // Flip a few random bytes anywhere in the file.
        0 => {
            for _ in 0..=rng.below(16) {
                let at = rng.below(data.len());
                data[at] ^= 1 << rng.below(8);
            }
        }
        // Damage the header and table directory, where offsets and counts live.
        1 => {
            let limit = data.len().min(512);
            for _ in 0..=rng.below(8) {
                let at = rng.below(limit);
                data[at] = rng.next() as u8;
            }
        }
        // Write hostile 32-bit values, such as huge offsets and counts, at aligned positions.
        2 => {
            let hostile = [0u32, 1, 0x7FFF_FFFF, 0xFFFF_FFFF, 0x0010_0000, 0xDEAD_BEEF];
            let value = hostile[rng.below(hostile.len())].to_be_bytes();
            let at = rng.below(data.len().saturating_sub(4).max(1)) & !3;
            let end = (at + 4).min(data.len());
            data[at..end].copy_from_slice(&value[..end - at]);
        }
        // Truncate at a random point.
        3 => {
            let keep = rng.below(data.len());
            data.truncate(keep);
        }
        // Zero or saturate a run of bytes.
        _ => {
            let start = rng.below(data.len());
            let len = rng.below(64).min(data.len() - start);
            let fill = if rng.below(2) == 0 { 0x00 } else { 0xFF };
            data[start..start + len].fill(fill);
        }
    }
}

fn iterations() -> usize {
    std::env::var("TPT_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(400)
}

#[test]
fn corrupted_fonts_never_panic_the_parser() {
    let iterations = iterations();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut parsed_ok = 0usize;
    let mut rejected = 0usize;

    for name in ["arial.ttf", "bahnschrift.ttf", "msgothic.ttc"] {
        let Some(path) = require_font(name) else {
            continue;
        };
        let original = fs::read(&path).expect("sample readable");

        for round in 0..iterations {
            let mut data = original.clone();
            for _ in 0..=rng.below(3) {
                mutate(&mut data, &mut rng);
            }

            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let Some(format) = detect_format(&data) else {
                    return None;
                };
                Some(parse_faces(&data, format).is_ok())
            }));
            match outcome {
                Ok(Some(true)) => parsed_ok += 1,
                Ok(_) => rejected += 1,
                Err(_) => panic!(
                    "parser panicked on {name}, round {round}; \
                     replay with seed state derived from the test's fixed seed"
                ),
            }
        }
    }

    println!("fuzz: {parsed_ok} parsed, {rejected} rejected, no panics");
}
