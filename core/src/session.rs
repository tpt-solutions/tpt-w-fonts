// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tracks which fonts are session-activated, persists that state, and recovers after a crash.
//!
//! The tray process owns a [`SessionManager`]. Every activation is written to a state file that
//! records the owner's process ID. If the tray dies without cleaning up, the next launch finds the
//! file, sees that its owner is gone, and deactivates the orphaned fonts.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::backend::FontBackend;
use crate::error::{FontError, Result};
use crate::format::is_font_extension;
use crate::platform::process_is_running;

/// Where an activation request came from. Shown in the UI and kept for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivationSource {
    Gui,
    Cli,
    Tray,
}

/// One font the session has activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveFont {
    pub path: PathBuf,
    pub source: ActivationSource,
    /// Milliseconds since the Unix epoch.
    pub activated_ms: u64,
}

/// The persisted session state: the owner and every font it has activated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRegistry {
    owner_pid: u32,
    fonts: Vec<ActiveFont>,
}

impl SessionRegistry {
    pub fn new(owner_pid: u32) -> Self {
        Self {
            owner_pid,
            fonts: Vec::new(),
        }
    }

    pub fn owner_pid(&self) -> u32 {
        self.owner_pid
    }

    pub fn active(&self) -> &[ActiveFont] {
        &self.fonts
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.fonts.iter().any(|font| font.path == path)
    }

    /// Records an activation. Returns `false` if the font was already active.
    fn add(&mut self, path: &Path, source: ActivationSource, activated_ms: u64) -> bool {
        if self.contains(path) {
            return false;
        }
        self.fonts.push(ActiveFont {
            path: path.to_path_buf(),
            source,
            activated_ms,
        });
        true
    }

    /// Forgets an activation. Returns `false` if the font was not active.
    fn remove(&mut self, path: &Path) -> bool {
        let before = self.fonts.len();
        self.fonts.retain(|font| font.path != path);
        self.fonts.len() != before
    }
}

/// Reads and writes the session state file.
#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns `None` when there is no state file. An unreadable file is treated the same way,
    /// because it cannot describe any fonts to clean up.
    pub fn load(&self) -> Option<SessionRegistry> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Writes the state atomically, so a crash mid-write never leaves a half-written file.
    pub fn save(&self, registry: &SessionRegistry) -> Result<()> {
        let io_err = |source| FontError::Io {
            path: self.path.clone(),
            source,
        };
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(registry)?).map_err(io_err)?;
        fs::rename(&tmp, &self.path).map_err(io_err)
    }

    fn delete(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(FontError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

/// Activates and deactivates fonts through a backend while keeping the persisted state in step.
pub struct SessionManager<B: FontBackend> {
    backend: B,
    store: SessionStore,
    registry: SessionRegistry,
}

impl<B: FontBackend> SessionManager<B> {
    /// Starts a fresh session owned by `owner_pid`.
    ///
    /// Call [`recover_orphans`] first. Starting over an unrecovered state file would lose the
    /// record of fonts that are still loaded.
    pub fn start(backend: B, store: SessionStore, owner_pid: u32) -> Result<Self> {
        let registry = SessionRegistry::new(owner_pid);
        store.save(&registry)?;
        Ok(Self {
            backend,
            store,
            registry,
        })
    }

    pub fn active(&self) -> &[ActiveFont] {
        self.registry.active()
    }

    /// Activates a font for this session. A font that is already active is left alone.
    pub fn activate(&mut self, path: &Path, source: ActivationSource) -> Result<()> {
        if self.registry.contains(path) {
            return Ok(());
        }
        if !path.is_file() || !is_font_extension(path) {
            return Err(FontError::Backend {
                path: path.to_path_buf(),
                reason: "not an existing font file".into(),
            });
        }
        // Activate first: if the OS refuses, the state must not claim the font is active.
        self.backend.activate_session(&[path])?;
        self.registry.add(path, source, now_ms());
        self.store.save(&self.registry)
    }

    /// Deactivates a font. Fonts that are not active are ignored.
    pub fn deactivate(&mut self, path: &Path) -> Result<()> {
        if !self.registry.contains(path) {
            return Ok(());
        }
        self.backend.deactivate_session(&[path])?;
        self.registry.remove(path);
        self.store.save(&self.registry)
    }

    /// Deactivates everything and removes the state file. Called on a clean quit.
    pub fn deactivate_all(&mut self) -> Result<()> {
        let paths: Vec<PathBuf> = self
            .registry
            .active()
            .iter()
            .map(|f| f.path.clone())
            .collect();
        if !paths.is_empty() {
            let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
            self.backend.deactivate_session(&refs)?;
        }
        self.registry = SessionRegistry::new(self.registry.owner_pid());
        self.store.delete()
    }
}

/// What [`recover_orphans`] found and did.
#[derive(Debug, PartialEq, Eq)]
pub enum Recovery {
    /// No state file, so nothing was left behind.
    Clean,
    /// A previous owner is still running. Its state was left untouched.
    OwnerRunning { pid: u32 },
    /// The previous owner is gone. Its fonts were deactivated and the state file removed.
    Recovered {
        pid: u32,
        fonts: Vec<PathBuf>,
        /// Set when the OS refused some deactivations. The state file is removed regardless,
        /// because the fonts cannot be cleaned up by retrying forever.
        cleanup_error: Option<String>,
    },
}

/// Cleans up after a tray process that exited without deactivating its fonts.
///
/// `current_pid` is this process. A state file that names it is not an orphan, so it is treated
/// as a running owner.
pub fn recover_orphans<B: FontBackend>(backend: &B, store: &SessionStore) -> Result<Recovery> {
    recover_orphans_with(backend, store, process_is_running, std::process::id())
}

/// [`recover_orphans`] with the liveness check and current process ID supplied by the caller.
pub fn recover_orphans_with<B: FontBackend>(
    backend: &B,
    store: &SessionStore,
    is_running: impl Fn(u32) -> bool,
    current_pid: u32,
) -> Result<Recovery> {
    let Some(registry) = store.load() else {
        return Ok(Recovery::Clean);
    };
    let pid = registry.owner_pid();
    if pid == current_pid || is_running(pid) {
        return Ok(Recovery::OwnerRunning { pid });
    }

    let fonts: Vec<PathBuf> = registry.active().iter().map(|f| f.path.clone()).collect();
    let cleanup_error = if fonts.is_empty() {
        None
    } else {
        let refs: Vec<&Path> = fonts.iter().map(PathBuf::as_path).collect();
        backend
            .deactivate_session(&refs)
            .err()
            .map(|err| err.to_string())
    };
    store.delete()?;
    Ok(Recovery::Recovered {
        pid,
        fonts,
        cleanup_error,
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
