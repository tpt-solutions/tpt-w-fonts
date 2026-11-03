// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Container format of a font file, detected from its magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontFormat {
    /// TrueType outlines in a single-face file (`.ttf`).
    Ttf,
    /// CFF/PostScript outlines in a single-face file (`.otf`).
    Otf,
    /// Multiple faces in one file (`.ttc`).
    Ttc,
}

/// Embedding rights from the OS/2 `fsType` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmbeddingRights {
    Installable,
    Restricted,
    PreviewAndPrint,
    Editable,
    /// The `fsType` field is absent or holds a reserved value.
    Unspecified,
}

/// Where a font file was found. System fonts are shown read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontSource {
    User,
    System,
}

/// A root directory to scan for font files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanRoot {
    pub path: PathBuf,
    pub source: FontSource,
}

/// One variation axis from the `fvar` table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariationAxis {
    /// Four-character axis tag, e.g. `wght`.
    pub tag: String,
    pub name: Option<String>,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub hidden: bool,
}

/// A named instance from the `fvar` table, e.g. "Bold Condensed".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedInstance {
    pub name: Option<String>,
    pub postscript_name: Option<String>,
    /// Coordinates in the same order as the face's `axes`.
    pub coordinates: Vec<f32>,
}

/// Metadata for one face. A `.ttc` file holds several; other files hold one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontFace {
    pub index: u32,
    pub family: String,
    pub subfamily: String,
    pub full_name: String,
    pub postscript_name: Option<String>,
    pub version: String,
    /// Four-character vendor/foundry ID from the OS/2 table, e.g. `MS  `.
    pub vendor_id: Option<String>,
    pub embedding: EmbeddingRights,
    /// Design units per em, from the `head` table.
    pub units_per_em: u16,
    pub is_variable: bool,
    pub axes: Vec<VariationAxis>,
    pub named_instances: Vec<NamedInstance>,
}

/// A font file in the catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontFile {
    pub path: PathBuf,
    pub source: FontSource,
    pub format: FontFormat,
    pub size: u64,
    /// Last-modified time in milliseconds since the Unix epoch.
    pub modified_ms: u64,
    /// Hex-encoded BLAKE3 hash of the full file contents.
    pub content_hash: String,
    pub faces: Vec<FontFace>,
}
