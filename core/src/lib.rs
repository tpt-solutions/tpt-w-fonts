// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Font engine, OS backends and session manager for tpt-fonts.
//!
//! Phase 1 covers the data model, the parser, the recursive scanner, the persisted
//! catalog and the filesystem watcher. Phase 2 adds duplicate detection and safe resolution,
//! and Phase 3 adds the live preview model for variable fonts.

mod catalog;
mod duplicates;
mod error;
mod format;
mod fvar;
mod model;
mod parse;
mod platform;
mod preview;
mod watch;

pub use catalog::{Catalog, RefreshReport};
pub use duplicates::{
    DuplicateGroup, DuplicateKind, Resolution, find_duplicates, keep_one, metadata_diff,
};
pub use error::{FontError, Result};
pub use format::{detect_format, is_font_extension};
pub use model::{
    EmbeddingRights, FontFace, FontFile, FontFormat, FontSource, NamedInstance, ScanRoot,
    VariationAxis,
};
pub use parse::{parse_faces, read_font_file};
pub use platform::default_scan_roots;
pub use preview::{
    AxisValue, Background, DEFAULT_SAMPLE_TEXT, PreviewState, SavedInstance, WATERFALL_SIZES,
};
pub use watch::{FontChange, FontWatcher};
