// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Linux implementation of [`FontBackend`] using fontconfig.
//!
//! Permanent installs go to the XDG user font directory. Session activation copies fonts into a
//! dedicated session directory and adds a fontconfig drop-in that points at it. Cache refreshes
//! are scoped to the one directory that changed, never a full `fc-cache -r`.
//!
//! External commands go through [`CommandRunner`], so the logic is tested with a fake runner on
//! any platform. The real fontconfig behaviour still needs confirming on Linux (see todo.md).

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::backend::{FontBackend, InstalledFont};
use crate::error::{FontError, Result};
use crate::format::{is_font_extension, unique_destination};
use crate::model::FontSource;
use crate::parse::read_font_file;

/// File name of the fontconfig drop-in that points at the session directory.
const SESSION_CONF_NAME: &str = "50-tpt-fonts-session.conf";

/// Outcome of running an external command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stderr: String,
}

/// Runs external programs. Only [`SystemRunner`] spawns real processes.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&OsStr]) -> io::Result<CommandOutput>;
}

/// Spawns real processes.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&OsStr]) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(args).output()?;
        Ok(CommandOutput {
            success: output.status.success(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// The directories and config location the backend works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxPaths {
    /// Permanent per-user fonts (`$XDG_DATA_HOME/fonts`).
    pub user_fonts: PathBuf,
    /// Session-only copies (`$XDG_CACHE_HOME/tpt-fonts/session`).
    pub session_fonts: PathBuf,
    /// Where the session drop-in is written (`$XDG_CONFIG_HOME/fontconfig/conf.d`).
    pub conf_dir: PathBuf,
}

impl LinuxPaths {
    /// The XDG locations for the current user, following the XDG base directory rules.
    pub fn for_current_user() -> Result<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let xdg = |var: &str, fallback: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .or_else(|| home.as_ref().map(|h| h.join(fallback)))
        };
        let data = xdg("XDG_DATA_HOME", ".local/share").ok_or_else(home_missing)?;
        let cache = xdg("XDG_CACHE_HOME", ".cache").ok_or_else(home_missing)?;
        let config = xdg("XDG_CONFIG_HOME", ".config").ok_or_else(home_missing)?;
        Ok(LinuxPaths {
            user_fonts: data.join("fonts"),
            session_fonts: cache.join("tpt-fonts").join("session"),
            conf_dir: config.join("fontconfig").join("conf.d"),
        })
    }
}

fn home_missing() -> FontError {
    FontError::Backend {
        path: PathBuf::new(),
        reason: "neither XDG variables nor HOME are set".into(),
    }
}

/// fontconfig-based backend. `R` runs `fc-cache`; tests substitute a fake.
pub struct FontconfigBackend<R: CommandRunner> {
    runner: R,
    paths: LinuxPaths,
}

impl<R: CommandRunner> FontconfigBackend<R> {
    pub fn new(runner: R, paths: LinuxPaths) -> Self {
        Self { runner, paths }
    }

    pub fn paths(&self) -> &LinuxPaths {
        &self.paths
    }

    /// Removes every session copy and the session drop-in, then refreshes the session cache.
    ///
    /// Called on tray exit. A crash can skip this, so the tray also calls it on next launch.
    pub fn clear_session(&self) -> Result<()> {
        let conf = self.paths.conf_dir.join(SESSION_CONF_NAME);
        if conf.exists() {
            fs::remove_file(&conf).map_err(|e| io_error(&conf, e))?;
        }
        if self.paths.session_fonts.is_dir() {
            fs::remove_dir_all(&self.paths.session_fonts)
                .map_err(|e| io_error(&self.paths.session_fonts, e))?;
        }
        self.refresh_cache(&self.paths.session_fonts)
    }

    /// Runs `fc-cache -f` on exactly one directory. Missing directories are skipped.
    fn refresh_cache(&self, dir: &Path) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        let output = self
            .runner
            .run("fc-cache", &[OsStr::new("-f"), dir.as_os_str()])
            .map_err(|e| io_error(dir, e))?;
        if output.success {
            Ok(())
        } else {
            Err(FontError::Backend {
                path: dir.to_path_buf(),
                reason: format!("fc-cache failed: {}", output.stderr.trim()),
            })
        }
    }

    /// Writes the drop-in that adds the session directory to fontconfig's search path.
    fn write_session_conf(&self) -> Result<()> {
        fs::create_dir_all(&self.paths.conf_dir).map_err(|e| io_error(&self.paths.conf_dir, e))?;
        let conf = self.paths.conf_dir.join(SESSION_CONF_NAME);
        let body = format!(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n<fontconfig>\n  <dir>{}</dir>\n</fontconfig>\n",
            xml_escape(&self.paths.session_fonts.to_string_lossy())
        );
        fs::write(&conf, body).map_err(|e| io_error(&conf, e))
    }

    fn ensure_inside_user_dir(&self, path: &Path) -> Result<()> {
        if path.starts_with(&self.paths.user_fonts) {
            Ok(())
        } else {
            Err(FontError::OutsideFontDirectory {
                path: path.to_path_buf(),
            })
        }
    }

    /// A stable session file name for a source path: `<hash of path>-<original name>`.
    /// Using the path, not the bytes, keeps activation and deactivation in step.
    fn session_copy_for(&self, source: &Path) -> Result<PathBuf> {
        let file_name = source.file_name().ok_or_else(|| FontError::Backend {
            path: source.to_path_buf(),
            reason: "path has no file name".into(),
        })?;
        let digest = blake3::hash(source.as_os_str().as_encoded_bytes()).to_hex();
        let mut name = OsString::from(&digest.as_str()[..16]);
        name.push("-");
        name.push(file_name);
        Ok(self.paths.session_fonts.join(name))
    }
}

impl<R: CommandRunner> FontBackend for FontconfigBackend<R> {
    fn install(&self, source: &Path) -> Result<InstalledFont> {
        if !is_font_extension(source) {
            return Err(FontError::Backend {
                path: source.to_path_buf(),
                reason: "not a font file extension".into(),
            });
        }
        // Parse before copying so a corrupt file never reaches the user directory.
        read_font_file(source, FontSource::User)?;

        fs::create_dir_all(&self.paths.user_fonts)
            .map_err(|e| io_error(&self.paths.user_fonts, e))?;
        let file_name = source.file_name().ok_or_else(|| FontError::Backend {
            path: source.to_path_buf(),
            reason: "path has no file name".into(),
        })?;
        let destination = unique_destination(&self.paths.user_fonts, file_name);
        fs::copy(source, &destination).map_err(|e| io_error(&destination, e))?;

        if let Err(err) = self.refresh_cache(&self.paths.user_fonts) {
            // Undo the copy so a failed refresh does not leave a font fontconfig never indexed.
            let _ = fs::remove_file(&destination);
            return Err(err);
        }
        Ok(InstalledFont {
            registration: destination
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path: destination,
        })
    }

    fn uninstall(&self, font: &InstalledFont) -> Result<()> {
        self.ensure_inside_user_dir(&font.path)?;
        fs::remove_file(&font.path).map_err(|e| io_error(&font.path, e))?;
        self.refresh_cache(&self.paths.user_fonts)
    }

    fn list_installed(&self) -> Result<Vec<InstalledFont>> {
        if !self.paths.user_fonts.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.paths.user_fonts)
            .map_err(|e| io_error(&self.paths.user_fonts, e))?;
        let mut fonts: Vec<InstalledFont> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && is_font_extension(path))
            .map(|path| InstalledFont {
                registration: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path,
            })
            .collect();
        fonts.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(fonts)
    }

    fn activate_session(&self, paths: &[&Path]) -> Result<()> {
        fs::create_dir_all(&self.paths.session_fonts)
            .map_err(|e| io_error(&self.paths.session_fonts, e))?;
        for &source in paths {
            read_font_file(source, FontSource::User)?;
            let copy = self.session_copy_for(source)?;
            fs::copy(source, &copy).map_err(|e| io_error(&copy, e))?;
        }
        self.write_session_conf()?;
        // A single refresh of the session directory covers the whole batch.
        self.refresh_cache(&self.paths.session_fonts)
    }

    fn deactivate_session(&self, paths: &[&Path]) -> Result<()> {
        for &source in paths {
            let copy = self.session_copy_for(source)?;
            if copy.exists() {
                fs::remove_file(&copy).map_err(|e| io_error(&copy, e))?;
            }
        }
        // With nothing left active, drop the include so fontconfig stops scanning the empty dir.
        let session_is_empty = fs::read_dir(&self.paths.session_fonts)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(true);
        if session_is_empty {
            let conf = self.paths.conf_dir.join(SESSION_CONF_NAME);
            if conf.exists() {
                fs::remove_file(&conf).map_err(|e| io_error(&conf, e))?;
            }
        }
        self.refresh_cache(&self.paths.session_fonts)
    }
}

fn io_error(path: &Path, source: io::Error) -> FontError {
    FontError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
