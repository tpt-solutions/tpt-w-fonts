// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Permanent install round trips: install, list, activate for the session, deactivate and
//! uninstall, checking that each step is visible to `list_installed`. Both backends run with
//! fakes for the OS calls, so the tests run on every platform.

mod common;

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tpt_fonts_core::{
    CommandOutput, CommandRunner, FontBackend, FontconfigBackend, InstalledFont, LinuxPaths,
    Win32Api, WindowsBackend,
};

use common::require_font;

// ---------------------------------------------------------------------------------------------
// Linux (fontconfig)
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeRunner {
    calls: RefCell<Vec<String>>,
}

impl CommandRunner for &FakeRunner {
    fn run(&self, program: &str, args: &[&std::ffi::OsStr]) -> io::Result<CommandOutput> {
        let joined: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", joined.join(" ")));
        Ok(CommandOutput {
            success: true,
            stderr: String::new(),
        })
    }
}

fn linux_paths(base: &Path) -> LinuxPaths {
    LinuxPaths {
        user_fonts: base.join("data").join("fonts"),
        session_fonts: base.join("cache").join("tpt-fonts").join("session"),
        conf_dir: base.join("config").join("fontconfig").join("conf.d"),
    }
}

#[test]
fn linux_install_list_activate_uninstall_round_trip() {
    let Some(source) = require_font("arial.ttf") else {
        return;
    };
    let root = tempfile::tempdir().expect("temp dir");
    let paths = linux_paths(root.path());
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, paths.clone());

    assert!(backend.list_installed().expect("list").is_empty());

    let installed = backend.install(&source).expect("install");
    assert!(installed.path.is_file(), "copy should be in the user dir");
    assert_eq!(
        backend.list_installed().expect("list"),
        vec![installed.clone()]
    );

    backend
        .activate_session(&[installed.path.as_path()])
        .expect("activate");
    assert!(paths.session_fonts.is_dir());
    backend
        .deactivate_session(&[installed.path.as_path()])
        .expect("deactivate");
    // A permanent install stays listed after a session is deactivated.
    assert_eq!(
        backend.list_installed().expect("list"),
        vec![installed.clone()]
    );

    backend.uninstall(&installed).expect("uninstall");
    assert!(!installed.path.exists());
    assert!(backend.list_installed().expect("list").is_empty());
}

// ---------------------------------------------------------------------------------------------
// Windows (per-user registry)
// ---------------------------------------------------------------------------------------------

/// Keeps a fake per-user registry so install and uninstall are visible to `user_fonts`.
#[derive(Default)]
struct FakeWin32 {
    registry: RefCell<Vec<(String, PathBuf)>>,
    broadcasts: RefCell<usize>,
}

impl Win32Api for &FakeWin32 {
    fn add_font_resource(&self, _path: &Path) -> bool {
        true
    }

    fn remove_font_resource(&self, _path: &Path) -> bool {
        true
    }

    fn broadcast_font_change(&self) {
        *self.broadcasts.borrow_mut() += 1;
    }

    fn set_user_font(&self, name: &str, path: &Path) -> io::Result<()> {
        self.registry
            .borrow_mut()
            .push((name.to_string(), path.to_path_buf()));
        Ok(())
    }

    fn remove_user_font(&self, name: &str) -> io::Result<()> {
        let mut registry = self.registry.borrow_mut();
        let before = registry.len();
        registry.retain(|(n, _)| n != name);
        if registry.len() == before {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        Ok(())
    }

    fn user_fonts(&self) -> io::Result<Vec<(String, PathBuf)>> {
        Ok(self.registry.borrow().clone())
    }
}

#[test]
fn windows_install_list_activate_uninstall_round_trip() {
    let Some(source) = require_font("arial.ttf") else {
        return;
    };
    let root = tempfile::tempdir().expect("temp dir");
    let font_dir = root.path().join("Microsoft").join("Windows").join("Fonts");
    fs::create_dir_all(&font_dir).expect("font dir");
    let api = FakeWin32::default();
    let backend = WindowsBackend::new(&api, font_dir.clone());

    assert!(backend.list_installed().expect("list").is_empty());

    let installed: InstalledFont = backend.install(&source).expect("install");
    assert!(installed.path.starts_with(&font_dir));
    assert!(installed.path.is_file());
    assert_eq!(
        backend.list_installed().expect("list"),
        vec![installed.clone()]
    );

    backend
        .activate_session(&[installed.path.as_path()])
        .expect("activate");
    backend
        .deactivate_session(&[installed.path.as_path()])
        .expect("deactivate");
    assert_eq!(
        backend.list_installed().expect("list"),
        vec![installed.clone()]
    );

    backend.uninstall(&installed).expect("uninstall");
    assert!(!installed.path.exists());
    assert!(backend.list_installed().expect("list").is_empty());
    assert!(
        api.registry.borrow().is_empty(),
        "registration should be gone"
    );
}
