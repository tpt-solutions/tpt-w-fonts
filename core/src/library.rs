// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The library view: one row per font file, with its state (installed, active in this session,
//! or only scanned). Shared by the GUI and anything else that lists fonts.

use std::path::PathBuf;

use serde::Serialize;

use crate::catalog::Catalog;
use crate::model::{FontFormat, FontSource};

/// Whether a font is permanently installed, active in this session, or only scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FontState {
    Installed,
    Session,
    Available,
}

/// One font file as the library shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FontRow {
    pub path: PathBuf,
    pub state: FontState,
    pub source: FontSource,
    pub format: FontFormat,
    /// Family, subfamily and full name of the first face. Collections list their other faces
    /// through `faces`.
    pub family: String,
    pub subfamily: String,
    pub full_name: String,
    pub is_variable: bool,
    pub faces: usize,
}

/// Builds the library rows, sorted by family then subfamily.
///
/// `session` and `installed` are the paths the tray and the installer report. A font in both
/// lists counts as installed, because a permanent install is the stronger state.
pub fn build_rows(catalog: &Catalog, session: &[PathBuf], installed: &[PathBuf]) -> Vec<FontRow> {
    let mut rows: Vec<FontRow> = catalog
        .files()
        .iter()
        .map(|file| {
            let state = if installed.iter().any(|p| p == &file.path) {
                FontState::Installed
            } else if session.iter().any(|p| p == &file.path) {
                FontState::Session
            } else {
                FontState::Available
            };
            let first = file.faces.first();
            FontRow {
                path: file.path.clone(),
                state,
                source: file.source,
                format: file.format,
                family: first.map(|f| f.family.clone()).unwrap_or_default(),
                subfamily: first.map(|f| f.subfamily.clone()).unwrap_or_default(),
                full_name: first.map(|f| f.full_name.clone()).unwrap_or_default(),
                is_variable: first.is_some_and(|f| f.is_variable),
                faces: file.faces.len(),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        a.family
            .to_lowercase()
            .cmp(&b.family.to_lowercase())
            .then_with(|| a.subfamily.to_lowercase().cmp(&b.subfamily.to_lowercase()))
            .then_with(|| a.path.cmp(&b.path))
    });
    rows
}
