// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Font engine, OS backends and session manager for tpt-fonts.
//!
//! Phase 1 covers the data model, the parser, the recursive scanner, the persisted
//! catalog and the filesystem watcher. Phase 2 adds duplicate detection and safe resolution,
//! and Phase 3 adds the live preview model for variable fonts.

mod backend;
mod catalog;
mod duplicates;
mod error;
mod format;
mod fvar;
mod linux;
mod model;
mod parse;
mod platform;
mod preview;
mod session;
mod watch;
#[cfg(windows)]
mod win32;

pub use backend::{FontBackend, InstalledFont};
pub use catalog::{Catalog, RefreshReport};
pub use duplicates::{
    DuplicateGroup, DuplicateKind, Resolution, find_duplicates, keep_one, metadata_diff,
};
pub use error::{FontError, Result};
pub use format::{detect_format, is_font_extension};
pub use linux::{CommandOutput, CommandRunner, FontconfigBackend, LinuxPaths, SystemRunner};
pub use model::{
    EmbeddingRights, FontFace, FontFile, FontFormat, FontSource, NamedInstance, ScanRoot,
    VariationAxis,
};
pub use parse::{parse_faces, read_font_file};
pub use platform::{default_scan_roots, process_is_running};
pub use preview::{
    AxisValue, Background, DEFAULT_SAMPLE_TEXT, PreviewState, SavedInstance, WATERFALL_SIZES,
};
pub use session::{
    ActivationSource, ActiveFont, Recovery, SessionManager, SessionRegistry, SessionStore,
    recover_orphans, recover_orphans_with,
};
pub use watch::{FontChange, FontWatcher};
#[cfg(windows)]
pub use win32::{NativeWin32, Win32Api, WindowsBackend};
