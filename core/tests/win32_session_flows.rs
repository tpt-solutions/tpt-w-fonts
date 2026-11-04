// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! End-to-end session flows on the Windows backend: `SessionManager`, the state file and
//! `WindowsBackend`, with a fake Win32 API. Like the Linux flow tests, these run on every
//! platform because nothing here calls the real OS.

mod common;

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use tpt_fonts_core::{
    ActivationSource, Recovery, SessionManager, SessionStore, Win32Api, WindowsBackend,
    recover_orphans_with,
};

use common::require_font;

/// Records every OS call so the tests can check ordering and broadcast counts.
#[derive(Default)]
struct FakeWin32 {
    calls: RefCell<Vec<String>>,
    /// Paths for which `AddFontResourceEx` reports failure.
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

    fn set_user_font(&self, _name: &str, _path: &Path) -> io::Result<()> {
        Ok(())
    }

    fn remove_user_font(&self, _name: &str) -> io::Result<()> {
        Ok(())
    }

    fn user_fonts(&self) -> io::Result<Vec<(String, PathBuf)>> {
        Ok(Vec::new())
    }
}

struct Sandbox {
    _root: tempfile::TempDir,
    store: SessionStore,
}

fn sandbox() -> Sandbox {
    let root = tempfile::tempdir().expect("temp dir");
    let store = SessionStore::new(root.path().join("session.json"));
    Sandbox { _root: root, store }
}

fn sample() -> Option<PathBuf> {
    require_font("arial.ttf")
}

#[test]
fn quitting_deactivates_each_font_with_one_broadcast_and_clears_the_state() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let api = FakeWin32::default();
    let backend = WindowsBackend::new(&api, sb.store.path().with_file_name("fonts"));
    let mut manager =
        SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");

    manager
        .activate(&font, ActivationSource::Gui)
        .expect("activate");
    assert_eq!(api.broadcasts(), 1, "one broadcast per activation batch");
    assert!(sb.store.path().exists());

    manager.deactivate_all().expect("quit");

    let remove = format!("remove {}", font.display());
    assert!(api.calls().contains(&remove), "font should be removed");
    assert_eq!(api.broadcasts(), 2);
    assert!(!sb.store.path().exists());
}

#[test]
fn the_next_launch_recovers_fonts_left_by_a_crashed_session() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let api = FakeWin32::default();
    {
        let backend = WindowsBackend::new(&api, sb.store.path().with_file_name("fonts"));
        let mut manager =
            SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");
        manager
            .activate(&font, ActivationSource::Tray)
            .expect("activate");
        // The manager goes out of scope without `deactivate_all`, as after a crash.
    }

    let backend = WindowsBackend::new(&api, sb.store.path().with_file_name("fonts"));
    let result = recover_orphans_with(&backend, &sb.store, |_| false, 10).expect("recovery");

    match result {
        Recovery::Recovered {
            pid,
            fonts,
            cleanup_error,
        } => {
            assert_eq!(pid, 4242);
            assert_eq!(fonts, vec![font.clone()]);
            assert_eq!(cleanup_error, None);
        }
        other => panic!("expected recovery, got {other:?}"),
    }
    let remove = format!("remove {}", font.display());
    assert!(api.calls().contains(&remove));
    assert!(!sb.store.path().exists());
}

#[test]
fn a_refused_activation_never_records_the_font_or_broadcasts() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let api = FakeWin32 {
        refuse_add: vec![font.clone()],
        ..FakeWin32::default()
    };
    let backend = WindowsBackend::new(&api, sb.store.path().with_file_name("fonts"));
    let mut manager =
        SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");

    let result = manager.activate(&font, ActivationSource::Gui);

    assert!(result.is_err(), "a refused font should be an error");
    assert!(manager.active().is_empty());
    assert_eq!(api.broadcasts(), 0, "nothing changed, so no broadcast");
    let saved = sb.store.load().expect("state file");
    assert!(saved.active().is_empty(), "state must not claim the font");
}
