// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::PathBuf;

/// Errors produced by the font engine.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("I/O error on {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("not a recognised font file: {}", path.display())]
    UnrecognisedFormat { path: PathBuf },

    #[error("failed to parse {}: {reason}", path.display())]
    Parse { path: PathBuf, reason: String },

    #[error("font has no face at index {0}")]
    NoSuchFace(u32),

    #[error("font has no variation axis '{0}'")]
    UnknownAxis(String),

    #[error("font has no named instance {0}")]
    UnknownInstance(usize),

    #[error("{} could not be handled: {reason}", path.display())]
    Backend { path: PathBuf, reason: String },

    #[error("a font named '{0}' is already installed")]
    AlreadyInstalled(String),

    #[error("{} is outside the user font directory and cannot be changed", path.display())]
    OutsideFontDirectory { path: PathBuf },

    #[error("{0}")]
    Usage(String),

    #[error("no font matches '{0}'")]
    NotFound(String),

    #[error(
        "'{query}' matches several fonts; give a full path instead: {}",
        join_paths(paths)
    )]
    Ambiguous { query: String, paths: Vec<PathBuf> },

    #[error("the tpt-fonts tray process is not running; start tpt-fonts-tray first")]
    TrayNotRunning,

    #[error("serialisation failed: {0}")]
    Serialise(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, FontError>;

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
