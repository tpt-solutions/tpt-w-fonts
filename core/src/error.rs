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

    #[error("catalog serialisation failed: {0}")]
    Serialise(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, FontError>;
