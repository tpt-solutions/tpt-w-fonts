// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use tpt_fonts_core::{
    ActivationSource, FontBackend, FontError, InstalledFont, Recovery, Result, SessionManager,
    SessionStore, recover_orphans_with,
};

/// Records session calls. Activation and deactivation can be made to fail on demand.
#[derive(Default)]
struct FakeBackend {
    calls: RefCell<Vec<String>>,
    fail_activation: bool,
    fail_deactivation: bool,
}

impl FakeBackend {
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    /// One entry per backend call, listing every path in that batch.
    fn record(&self, verb: &str, paths: &[&Path]) {
        let names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        self.calls
            .borrow_mut()
            .push(format!("{verb} {}", names.join(", ")));
    }
}

impl FontBackend for FakeBackend {
    fn install(&self, _source: &Path) -> Result<InstalledFont> {
        unimplemented!("not used by session tests")
    }

    fn uninstall(&self, _font: &InstalledFont) -> Result<()> {
        unimplemented!("not used by session tests")
    }

    fn list_installed(&self) -> Result<Vec<InstalledFont>> {
        Ok(Vec::new())
    }

    fn activate_session(&self, paths: &[&Path]) -> Result<()> {
        self.record("activate", paths);
        if self.fail_activation {
            return Err(FontError::Backend {
                path: paths[0].to_path_buf(),
                reason: "refused".into(),
            });
        }
        Ok(())
    }

    fn deactivate_session(&self, paths: &[&Path]) -> Result<()> {
        self.record("deactivate", paths);
        if self.fail_deactivation {
            return Err(FontError::Backend {
                path: paths[0].to_path_buf(),
                reason: "not loaded".into(),
            });
        }
        Ok(())
    }
}

fn store_in(dir: &Path) -> SessionStore {
    SessionStore::new(dir.join("session.json"))
}

fn font(name: &str) -> PathBuf {
    PathBuf::from(format!(r"C:\fonts\{name}.ttf"))
}

#[test]
fn activation_is_recorded_and_persisted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    let mut manager =
        SessionManager::start(FakeBackend::default(), store.clone(), 4242).expect("start");

    manager
        .activate(&font("Alpha"), ActivationSource::Gui)
        .expect("activate");

    assert_eq!(manager.active().len(), 1);
    let saved = store.load().expect("state file written");
    assert_eq!(saved.owner_pid(), 4242);
    assert!(saved.contains(&font("Alpha")));
}

#[test]
fn activating_twice_only_calls_the_backend_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut manager =
        SessionManager::start(FakeBackend::default(), store_in(dir.path()), 1).expect("start");

    manager
        .activate(&font("Alpha"), ActivationSource::Cli)
        .expect("first");
    manager
        .activate(&font("Alpha"), ActivationSource::Tray)
        .expect("second");

    assert_eq!(manager.active().len(), 1);
    assert_eq!(
        manager.active()[0].source,
        ActivationSource::Cli,
        "the first source is kept"
    );
}

#[test]
fn a_refused_activation_leaves_no_record() {
    let dir = tempfile::tempdir().expect("temp dir");
    let backend = FakeBackend {
        fail_activation: true,
        ..Default::default()
    };
    let store = store_in(dir.path());
    let mut manager = SessionManager::start(backend, store.clone(), 1).expect("start");

    assert!(
        manager
            .activate(&font("Alpha"), ActivationSource::Gui)
            .is_err()
    );
    assert!(manager.active().is_empty());
    assert!(store.load().expect("file").active().is_empty());
}

#[test]
fn deactivating_a_font_that_is_not_active_does_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut manager =
        SessionManager::start(FakeBackend::default(), store_in(dir.path()), 1).expect("start");

    manager.deactivate(&font("Ghost")).expect("ignored");
    assert!(manager.active().is_empty());
}

#[test]
fn deactivate_all_clears_state_and_removes_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    let mut manager =
        SessionManager::start(FakeBackend::default(), store.clone(), 1).expect("start");
    manager
        .activate(&font("Alpha"), ActivationSource::Gui)
        .expect("a");
    manager
        .activate(&font("Beta"), ActivationSource::Gui)
        .expect("b");

    manager.deactivate_all().expect("quit cleanly");

    assert!(manager.active().is_empty());
    assert!(!store.path().exists());
}

#[test]
fn no_state_file_means_nothing_to_recover() {
    let dir = tempfile::tempdir().expect("temp dir");
    let backend = FakeBackend::default();

    let result =
        recover_orphans_with(&backend, &store_in(dir.path()), |_| false, 10).expect("recovery");

    assert_eq!(result, Recovery::Clean);
    assert!(backend.calls().is_empty());
}

#[test]
fn a_dead_owner_has_its_fonts_deactivated_and_the_file_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    {
        let mut crashed =
            SessionManager::start(FakeBackend::default(), store.clone(), 777).expect("start");
        crashed
            .activate(&font("Alpha"), ActivationSource::Gui)
            .expect("a");
        crashed
            .activate(&font("Beta"), ActivationSource::Cli)
            .expect("b");
        // Dropped without deactivate_all, as after a crash.
    }
    let backend = FakeBackend::default();

    let result = recover_orphans_with(&backend, &store, |_| false, 10).expect("recovery");

    match result {
        Recovery::Recovered {
            pid,
            fonts,
            cleanup_error,
        } => {
            assert_eq!(pid, 777);
            assert_eq!(fonts, vec![font("Alpha"), font("Beta")]);
            assert!(cleanup_error.is_none());
        }
        other => panic!("expected recovery, got {other:?}"),
    }
    assert_eq!(backend.calls().len(), 1, "one batched deactivation");
    assert!(!store.path().exists());
}

#[test]
fn a_running_owner_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    SessionManager::start(FakeBackend::default(), store.clone(), 555)
        .expect("start")
        .activate(&font("Alpha"), ActivationSource::Gui)
        .expect("activate");
    let backend = FakeBackend::default();

    let result = recover_orphans_with(&backend, &store, |pid| pid == 555, 10).expect("recovery");

    assert_eq!(result, Recovery::OwnerRunning { pid: 555 });
    assert!(backend.calls().is_empty());
    assert!(store.path().exists(), "the other instance's record is kept");
}

#[test]
fn our_own_pid_in_the_file_is_not_treated_as_an_orphan() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    SessionManager::start(FakeBackend::default(), store.clone(), 99).expect("start");

    let result =
        recover_orphans_with(&FakeBackend::default(), &store, |_| false, 99).expect("recovery");
    assert_eq!(result, Recovery::OwnerRunning { pid: 99 });
}

#[test]
fn cleanup_errors_are_reported_but_the_stale_file_is_still_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    SessionManager::start(FakeBackend::default(), store.clone(), 321)
        .expect("start")
        .activate(&font("Alpha"), ActivationSource::Gui)
        .expect("activate");
    let backend = FakeBackend {
        fail_deactivation: true,
        ..Default::default()
    };

    let result = recover_orphans_with(&backend, &store, |_| false, 10).expect("recovery");

    match result {
        Recovery::Recovered {
            cleanup_error: Some(reason),
            ..
        } => assert!(reason.contains("not loaded")),
        other => panic!("expected a reported cleanup error, got {other:?}"),
    }
    assert!(
        !store.path().exists(),
        "a stale record is not retried forever"
    );
}

#[test]
fn an_unreadable_state_file_counts_as_clean() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_in(dir.path());
    std::fs::write(store.path(), b"{ broken").expect("write");

    let result =
        recover_orphans_with(&FakeBackend::default(), &store, |_| false, 10).expect("recovery");
    assert_eq!(result, Recovery::Clean);
}
