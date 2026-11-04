// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! End-to-end session flows: `SessionManager`, the state file and the real `FontconfigBackend`,
//! with a fake `fc-cache` runner and temporary directories. The layers are tested separately
//! elsewhere; these tests check that they work together, including after a crash.

mod common;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;

use tpt_fonts_core::{
    ActivationSource, CommandOutput, CommandRunner, FontconfigBackend, LinuxPaths, Recovery,
    SessionManager, SessionStore, recover_orphans_with,
};

use common::require_font;

/// Records `fc-cache` invocations and can be told to fail.
#[derive(Default)]
struct FakeRunner {
    calls: RefCell<Vec<String>>,
    fail: bool,
}

impl CommandRunner for &FakeRunner {
    fn run(&self, program: &str, args: &[&OsStr]) -> io::Result<CommandOutput> {
        let joined: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        self.calls
            .borrow_mut()
            .push(format!("{program} {}", joined.join(" ")));
        Ok(CommandOutput {
            success: !self.fail,
            stderr: if self.fail {
                "boom".into()
            } else {
                String::new()
            },
        })
    }
}

struct Sandbox {
    _root: tempfile::TempDir,
    paths: LinuxPaths,
    store: SessionStore,
}

fn sandbox() -> Sandbox {
    let root = tempfile::tempdir().expect("temp dir");
    let base = root.path();
    let paths = LinuxPaths {
        user_fonts: base.join("data").join("fonts"),
        session_fonts: base.join("cache").join("tpt-fonts").join("session"),
        conf_dir: base.join("config").join("fontconfig").join("conf.d"),
    };
    let store = SessionStore::new(base.join("state").join("session.json"));
    Sandbox {
        _root: root,
        paths,
        store,
    }
}

fn sample() -> Option<PathBuf> {
    require_font("arial.ttf")
}

/// Deactivation empties the session directory but keeps it, so the check is "no files left".
fn session_dir_is_empty(paths: &LinuxPaths) -> bool {
    fs::read_dir(&paths.session_fonts)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true)
}

#[test]
fn quitting_removes_every_session_font_and_the_state_file() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let mut manager =
        SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");

    manager
        .activate(&font, ActivationSource::Gui)
        .expect("activate");
    assert!(sb.paths.session_fonts.is_dir(), "font copy should exist");
    assert!(sb.paths.conf_dir.join("50-tpt-fonts-session.conf").exists());
    assert!(sb.store.path().exists(), "state file should exist");

    manager.deactivate_all().expect("quit");

    assert!(session_dir_is_empty(&sb.paths));
    assert!(!sb.paths.conf_dir.join("50-tpt-fonts-session.conf").exists());
    assert!(!sb.store.path().exists());
}

#[test]
fn the_next_launch_recovers_fonts_left_by_a_crashed_session() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    {
        let backend = FontconfigBackend::new(&runner, sb.paths.clone());
        let mut manager =
            SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");
        manager
            .activate(&font, ActivationSource::Tray)
            .expect("activate");
        // The manager goes out of scope without `deactivate_all`, as after a crash.
    }
    assert!(sb.paths.session_fonts.is_dir());

    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let result = recover_orphans_with(&backend, &sb.store, |_| false, 10).expect("recovery");

    match result {
        Recovery::Recovered {
            pid,
            fonts,
            cleanup_error,
        } => {
            assert_eq!(pid, 4242);
            assert_eq!(fonts, vec![font]);
            assert_eq!(cleanup_error, None);
        }
        other => panic!("expected recovery, got {other:?}"),
    }
    assert!(session_dir_is_empty(&sb.paths));
    assert!(!sb.paths.conf_dir.join("50-tpt-fonts-session.conf").exists());
    assert!(!sb.store.path().exists());
}

#[test]
fn a_refused_cache_refresh_never_records_the_font_as_active() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner {
        fail: true,
        ..FakeRunner::default()
    };
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let mut manager =
        SessionManager::start(backend, sb.store.clone(), 4242).expect("start session");

    let result = manager.activate(&font, ActivationSource::Gui);

    assert!(result.is_err(), "a failed cache refresh should be an error");
    assert!(manager.active().is_empty());
    let saved = sb.store.load().expect("state file");
    assert!(saved.active().is_empty(), "state must not claim the font");
}
