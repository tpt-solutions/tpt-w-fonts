// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Font engine, OS backends and session manager for tpt-fonts.
//!
//! Phase 1 covers the data model, the parser, the recursive scanner, the persisted
//! catalog and the filesystem watcher. Phase 2 adds duplicate detection and safe resolution,
//! and Phase 3 adds the live preview model for variable fonts.

mod autostart;
mod backend;
mod catalog;
mod duplicates;
mod error;
mod format;
mod fvar;
mod ipc;
mod ipc_client;
mod linux;
mod model;
mod parse;
mod platform;
mod preview;
mod query;
mod session;
mod specimen;
mod watch;
#[cfg(windows)]
mod win32;

pub use autostart::{
    AUTOSTART_NAME, autostart_enabled, desktop_entry, set_autostart, write_desktop_entry,
};
pub use backend::{FontBackend, InstalledFont};
pub use catalog::{Catalog, RefreshReport, load_default_catalog};
pub use duplicates::{
    DuplicateGroup, DuplicateKind, Resolution, find_duplicates, keep_one, metadata_diff,
};
pub use error::{FontError, Result};
pub use format::{detect_format, is_font_extension};
pub use ipc::{
    Request, Response, SOCKET_NAME, decode_line, encode_line, handle_request, is_activatable,
};
pub use ipc_client::send as send_request;
pub use linux::{CommandOutput, CommandRunner, FontconfigBackend, LinuxPaths, SystemRunner};
pub use model::{
    EmbeddingRights, FontFace, FontFile, FontFormat, FontSource, NamedInstance, ScanRoot,
    VariationAxis,
};
pub use parse::{parse_faces, read_font_file};
pub use platform::{
    PlatformBackend, app_state_dir, default_scan_roots, platform_backend, process_is_running,
    user_config_dir,
};
pub use preview::{
    AxisValue, Background, DEFAULT_SAMPLE_TEXT, PreviewState, SavedInstance, WATERFALL_SIZES,
};
pub use query::{find_matches, resolve};
pub use session::{
    ActivationSource, ActiveFont, Recovery, SessionManager, SessionRegistry, SessionStore,
    recover_orphans, recover_orphans_with,
};
pub use specimen::render_svg;
pub use watch::{FontChange, FontWatcher};
#[cfg(windows)]
pub use win32::{NativeWin32, Win32Api, WindowsBackend};
