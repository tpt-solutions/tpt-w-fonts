// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Windows implementation of [`FontBackend`].
//!
//! Every OS call goes through the [`Win32Api`] trait. [`NativeWin32`] is the real implementation;
//! tests substitute a fake so the install and activation logic runs on any CI runner.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::backend::{FontBackend, InstalledFont};
use crate::error::{FontError, Result};
use crate::format::{is_font_extension, is_within, unique_destination};
use crate::model::{FontFormat, FontSource};
use crate::parse::read_font_file;

/// Registry key where Windows records per-user font registrations.
const USER_FONTS_KEY: &str = r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

/// The OS calls the backend needs. Only [`NativeWin32`] talks to the real system.
pub trait Win32Api {
    /// `AddFontResourceExW` with shared (non-private) scope. Returns `true` if a font was added.
    fn add_font_resource(&self, path: &Path) -> bool;
    /// `RemoveFontResourceExW` with the same scope. Returns `true` if a font was removed.
    fn remove_font_resource(&self, path: &Path) -> bool;
    /// Broadcasts `WM_FONTCHANGE` so running applications refresh their font lists.
    fn broadcast_font_change(&self);
    /// Writes a per-user font registration (`HKCU`).
    fn set_user_font(&self, name: &str, path: &Path) -> io::Result<()>;
    /// Removes a per-user font registration (`HKCU`).
    fn remove_user_font(&self, name: &str) -> io::Result<()>;
    /// Lists every per-user font registration as `(name, path)`.
    fn user_fonts(&self) -> io::Result<Vec<(String, PathBuf)>>;
}

/// The real Win32 and registry implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeWin32;

impl Win32Api for NativeWin32 {
    fn add_font_resource(&self, path: &Path) -> bool {
        let wide = to_wide(path);
        // SAFETY: `wide` is a NUL-terminated UTF-16 buffer that lives for the whole call.
        // A null `pdv` is allowed when no design vector is supplied.
        unsafe {
            windows_sys::Win32::Graphics::Gdi::AddFontResourceExW(
                wide.as_ptr(),
                0,
                std::ptr::null(),
            ) > 0
        }
    }

    fn remove_font_resource(&self, path: &Path) -> bool {
        let wide = to_wide(path);
        // SAFETY: same buffer contract as `add_font_resource`.
        unsafe {
            windows_sys::Win32::Graphics::Gdi::RemoveFontResourceExW(
                wide.as_ptr(),
                0,
                std::ptr::null(),
            ) > 0
        }
    }

    fn broadcast_font_change(&self) {
        use windows_sys::Win32::Foundation::{LPARAM, WPARAM};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_FONTCHANGE,
        };
        // SAFETY: a broadcast with no result buffer; the timeout keeps a hung window from
        // blocking the caller.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_FONTCHANGE,
                WPARAM::default(),
                LPARAM::default(),
                SMTO_ABORTIFHUNG,
                1000,
                std::ptr::null_mut(),
            );
        }
    }

    fn set_user_font(&self, name: &str, path: &Path) -> io::Result<()> {
        let (key, _) = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .create_subkey(USER_FONTS_KEY)?;
        key.set_value(name, &path.to_string_lossy().into_owned())
    }

    fn remove_user_font(&self, name: &str) -> io::Result<()> {
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags(USER_FONTS_KEY, winreg::enums::KEY_WRITE)?;
        key.delete_value(name)
    }

    fn user_fonts(&self) -> io::Result<Vec<(String, PathBuf)>> {
        let key = match winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(USER_FONTS_KEY)
        {
            Ok(key) => key,
            // No per-user fonts have ever been registered.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };
        let mut fonts = Vec::new();
        for entry in key.enum_values() {
            let (name, _) = entry?;
            let target: String = key.get_value(&name)?;
            fonts.push((name, PathBuf::from(target)));
        }
        Ok(fonts)
    }
}

fn to_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Installs fonts into a per-user directory and registers them under `HKCU`.
///
/// `font_dir` is the directory that counts as "user-level"; anything outside it is refused.
pub struct WindowsBackend<A: Win32Api> {
    api: A,
    font_dir: PathBuf,
}

impl WindowsBackend<NativeWin32> {
    /// The backend for the current user, using `%LOCALAPPDATA%\Microsoft\Windows\Fonts`.
    pub fn for_current_user() -> Result<Self> {
        let local = std::env::var_os("LOCALAPPDATA").ok_or_else(|| FontError::Backend {
            path: PathBuf::new(),
            reason: "LOCALAPPDATA is not set".into(),
        })?;
        let font_dir = PathBuf::from(local)
            .join("Microsoft")
            .join("Windows")
            .join("Fonts");
        fs::create_dir_all(&font_dir).map_err(|source| FontError::Io {
            path: font_dir.clone(),
            source,
        })?;
        Ok(Self::new(NativeWin32, font_dir))
    }
}

impl<A: Win32Api> WindowsBackend<A> {
    pub fn new(api: A, font_dir: PathBuf) -> Self {
        Self { api, font_dir }
    }

    pub fn font_dir(&self) -> &Path {
        &self.font_dir
    }

    fn ensure_inside_font_dir(&self, path: &Path) -> Result<()> {
        if is_within(path, &self.font_dir) {
            Ok(())
        } else {
            Err(FontError::OutsideFontDirectory {
                path: path.to_path_buf(),
            })
        }
    }

    fn io_error(path: &Path, source: io::Error) -> FontError {
        FontError::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}

impl<A: Win32Api> FontBackend for WindowsBackend<A> {
    fn install(&self, source: &Path) -> Result<InstalledFont> {
        if !is_font_extension(source) {
            return Err(FontError::Backend {
                path: source.to_path_buf(),
                reason: "not a font file extension".into(),
            });
        }
        // Parsing first means a corrupt file is rejected before anything is copied or registered.
        let file = read_font_file(source, FontSource::User)?;
        let face = file.faces.first().ok_or_else(|| FontError::Backend {
            path: source.to_path_buf(),
            reason: "font has no faces".into(),
        })?;

        let kind = match file.format {
            FontFormat::Otf => "OpenType",
            FontFormat::Ttf | FontFormat::Ttc => "TrueType",
        };
        let display_name = if face.full_name.is_empty() {
            &face.family
        } else {
            &face.full_name
        };
        let registration = format!("{display_name} ({kind})");

        let registered = self
            .api
            .user_fonts()
            .map_err(|e| Self::io_error(&self.font_dir, e))?;
        if registered.iter().any(|(name, _)| *name == registration) {
            return Err(FontError::AlreadyInstalled(registration));
        }

        let file_name = source.file_name().ok_or_else(|| FontError::Backend {
            path: source.to_path_buf(),
            reason: "path has no file name".into(),
        })?;
        let destination = unique_destination(&self.font_dir, file_name);
        fs::copy(source, &destination).map_err(|e| Self::io_error(&destination, e))?;

        if let Err(err) = self.api.set_user_font(&registration, &destination) {
            // Roll back the copy so a failed registration does not leave an orphaned file.
            let _ = fs::remove_file(&destination);
            return Err(Self::io_error(&destination, err));
        }
        self.api.broadcast_font_change();

        Ok(InstalledFont {
            path: destination,
            registration,
        })
    }

    fn uninstall(&self, font: &InstalledFont) -> Result<()> {
        self.ensure_inside_font_dir(&font.path)?;
        self.api
            .remove_user_font(&font.registration)
            .map_err(|e| Self::io_error(&font.path, e))?;
        fs::remove_file(&font.path).map_err(|e| Self::io_error(&font.path, e))?;
        self.api.broadcast_font_change();
        Ok(())
    }

    fn list_installed(&self) -> Result<Vec<InstalledFont>> {
        let registered = self
            .api
            .user_fonts()
            .map_err(|e| Self::io_error(&self.font_dir, e))?;
        Ok(registered
            .into_iter()
            .filter(|(_, path)| path.starts_with(&self.font_dir))
            .map(|(registration, path)| InstalledFont { path, registration })
            .collect())
    }

    fn activate_session(&self, paths: &[&Path]) -> Result<()> {
        let mut added_any = false;
        let mut failed: Vec<&Path> = Vec::new();
        for &path in paths {
            if self.api.add_font_resource(path) {
                added_any = true;
            } else {
                failed.push(path);
            }
        }
        // One broadcast for the whole batch, and only if something changed.
        if added_any {
            self.api.broadcast_font_change();
        }
        first_failure(&failed, "AddFontResourceEx added no font")
    }

    fn deactivate_session(&self, paths: &[&Path]) -> Result<()> {
        let mut removed_any = false;
        let mut failed: Vec<&Path> = Vec::new();
        for &path in paths {
            if self.api.remove_font_resource(path) {
                removed_any = true;
            } else {
                failed.push(path);
            }
        }
        if removed_any {
            self.api.broadcast_font_change();
        }
        first_failure(&failed, "RemoveFontResourceEx removed no font")
    }
}

/// Reports the first failing path, if any. The others were still processed.
fn first_failure(failed: &[&Path], reason: &str) -> Result<()> {
    match failed.first() {
        None => Ok(()),
        Some(path) => Err(FontError::Backend {
            path: path.to_path_buf(),
            reason: format!("{reason} ({} failed in total)", failed.len()),
        }),
    }
}
