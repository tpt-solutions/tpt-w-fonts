// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

#![cfg(windows)]

mod common;

use std::cell::RefCell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use tpt_fonts_core::{FontBackend, FontError, InstalledFont, Win32Api, WindowsBackend};

use common::require_font;

/// Records every OS call and keeps a fake per-user registry, so the logic runs without touching
/// the real system.
#[derive(Default)]
struct FakeWin32 {
    calls: RefCell<Vec<String>>,
    registry: RefCell<Vec<(String, PathBuf)>>,
    /// Paths for which `AddFontResourceEx` should report failure.
    refuse_add: Vec<PathBuf>,
}

impl FakeWin32 {
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    fn broadcasts(&self) -> usize {
        self.calls().iter().filter(|c| *c == "broadcast").count()
    }
}

impl Win32Api for &FakeWin32 {
    fn add_font_resource(&self, path: &Path) -> bool {
        self.calls
            .borrow_mut()
            .push(format!("add {}", path.display()));
        !self.refuse_add.iter().any(|p| p == path)
    }

    fn remove_font_resource(&self, path: &Path) -> bool {
        self.calls
            .borrow_mut()
            .push(format!("remove {}", path.display()));
        true
    }

    fn broadcast_font_change(&self) {
        self.calls.borrow_mut().push("broadcast".into());
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

fn fresh_font_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

#[test]
fn install_copies_the_font_and_registers_it() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = fresh_font_dir();
    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, dir.path().to_path_buf());

    let installed = backend.install(&sample).expect("install succeeds");

    assert!(installed.path.starts_with(dir.path()));
    assert!(installed.path.is_file());
    assert_eq!(installed.registry_name, "Arial (TrueType)");
    assert_eq!(fake.registry.borrow().len(), 1);
    assert_eq!(fake.broadcasts(), 1, "one broadcast per install");
}

#[test]
fn installing_the_same_font_twice_is_rejected() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = fresh_font_dir();
    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, dir.path().to_path_buf());

    backend.install(&sample).expect("first install");
    let err = backend
        .install(&sample)
        .expect_err("second install conflicts");
    assert!(matches!(err, FontError::AlreadyInstalled(name) if name == "Arial (TrueType)"));
}

#[test]
fn uninstall_removes_registration_and_file() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let dir = fresh_font_dir();
    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, dir.path().to_path_buf());

    let installed = backend.install(&sample).expect("install");
    backend.uninstall(&installed).expect("uninstall");

    assert!(!installed.path.exists());
    assert!(fake.registry.borrow().is_empty());
    assert!(backend.list_installed().expect("list").is_empty());
}

#[test]
fn uninstall_refuses_files_outside_the_font_directory() {
    let dir = fresh_font_dir();
    let outside = fresh_font_dir();
    let victim = outside.path().join("keep-me.ttf");
    fs::write(&victim, b"not ours").expect("write");

    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, dir.path().to_path_buf());
    let bogus = InstalledFont {
        path: victim.clone(),
        registry_name: "Anything".into(),
    };

    let err = backend.uninstall(&bogus).expect_err("outside path refused");
    assert!(matches!(err, FontError::OutsideFontDirectory { .. }));
    assert!(
        victim.exists(),
        "the file outside the font folder is untouched"
    );
    assert!(
        fake.calls().is_empty(),
        "no OS call is made for a refused path"
    );
}

#[test]
fn activation_adds_every_font_with_a_single_broadcast() {
    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, PathBuf::from("unused"));
    let a = PathBuf::from(r"C:\fonts\a.ttf");
    let b = PathBuf::from(r"C:\fonts\b.ttf");

    backend
        .activate_session(&[a.as_path(), b.as_path()])
        .expect("activation succeeds");

    assert_eq!(fake.calls().len(), 3);
    assert_eq!(fake.broadcasts(), 1, "batched activation broadcasts once");
}

#[test]
fn activation_failure_is_reported_but_other_fonts_still_activate() {
    let bad = PathBuf::from(r"C:\fonts\bad.ttf");
    let good = PathBuf::from(r"C:\fonts\good.ttf");
    let fake = FakeWin32 {
        refuse_add: vec![bad.clone()],
        ..Default::default()
    };
    let backend = WindowsBackend::new(&fake, PathBuf::from("unused"));

    let err = backend
        .activate_session(&[bad.as_path(), good.as_path()])
        .expect_err("one font failed");

    assert!(matches!(err, FontError::Backend { path, .. } if path == bad));
    assert!(fake.calls().contains(&format!("add {}", good.display())));
    assert_eq!(
        fake.broadcasts(),
        1,
        "the successful font still triggers a refresh"
    );
}

#[test]
fn deactivation_removes_each_font_and_broadcasts_once() {
    let fake = FakeWin32::default();
    let backend = WindowsBackend::new(&fake, PathBuf::from("unused"));
    let a = PathBuf::from(r"C:\fonts\a.ttf");

    backend
        .deactivate_session(&[a.as_path()])
        .expect("deactivate");

    assert_eq!(
        fake.calls(),
        vec![format!("remove {}", a.display()), "broadcast".into()]
    );
}

#[test]
fn list_installed_ignores_registrations_outside_the_font_directory() {
    let dir = fresh_font_dir();
    let fake = FakeWin32::default();
    fake.registry.borrow_mut().push((
        "Elsewhere (TrueType)".into(),
        PathBuf::from(r"C:\Windows\Fonts\elsewhere.ttf"),
    ));
    let backend = WindowsBackend::new(&fake, dir.path().to_path_buf());

    assert!(backend.list_installed().expect("list").is_empty());
}

/// Exercises the real registry, font folder and Win32 calls.
///
/// Ignored by default because it writes to the current user's font folder and registry. It
/// always uninstalls what it installed. Run with: `cargo test -p tpt-fonts-core -- --ignored`.
#[test]
#[ignore = "touches the real per-user font folder and HKCU registry"]
fn native_install_activate_deactivate_uninstall_round_trip() {
    let Some(sample) = require_font("arial.ttf") else {
        return;
    };
    let backend = WindowsBackend::for_current_user().expect("user font dir");
    // A renamed copy avoids clashing with the real Arial registration under HKLM.
    let staging = fresh_font_dir();
    let copy = staging.path().join("tptfonts-roundtrip-probe.ttf");
    fs::copy(&sample, &copy).expect("stage copy");

    let installed = backend.install(&copy).expect("install");
    let result = (|| {
        backend.activate_session(&[installed.path.as_path()])?;
        backend.deactivate_session(&[installed.path.as_path()])?;
        Ok::<(), FontError>(())
    })();
    let cleanup = backend.uninstall(&installed);

    result.expect("activation round trip");
    cleanup.expect("uninstall after the round trip");
    assert!(!installed.path.exists());
}
