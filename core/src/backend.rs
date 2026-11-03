// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The per-OS contract for installing fonts and activating them for the login session.

use std::path::{Path, PathBuf};

use crate::error::Result;

/// A font installed permanently for the current user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledFont {
    /// Where the font file lives inside the user font directory.
    pub path: PathBuf,
    /// The registry value name that makes the OS register this font (Windows).
    pub registry_name: String,
}

/// Installs fonts permanently and activates them for the login session.
///
/// Implementations must never touch system-wide font locations. Session activation is
/// temporary: it lasts until [`FontBackend::deactivate_session`] or the user logs out.
pub trait FontBackend {
    /// Copies a font into the user font directory and registers it. Permanent until uninstalled.
    fn install(&self, source: &Path) -> Result<InstalledFont>;

    /// Unregisters and removes a font previously installed by this backend.
    fn uninstall(&self, font: &InstalledFont) -> Result<()>;

    /// Lists fonts this backend installed for the current user.
    fn list_installed(&self) -> Result<Vec<InstalledFont>>;

    /// Makes fonts visible to every application for this login session.
    ///
    /// Implementations should batch the OS notification so that several fonts cost one
    /// font-cache refresh, not one per font.
    fn activate_session(&self, paths: &[&Path]) -> Result<()>;

    /// Removes session activation for fonts previously passed to [`FontBackend::activate_session`].
    fn deactivate_session(&self, paths: &[&Path]) -> Result<()>;
}
