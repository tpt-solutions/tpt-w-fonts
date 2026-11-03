// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! fontconfig backend tests. They use a fake `fc-cache` runner and temporary directories, so
//! they run on every platform. The real fontconfig behaviour is covered on Linux separately.

mod common;

use std::cell::RefCell;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::PathBuf;

use tpt_fonts_core::{
    CommandOutput, CommandRunner, FontBackend, FontError, FontconfigBackend, InstalledFont,
    LinuxPaths,
};

use common::require_font;

/// Records `fc-cache` invocations and can be told to fail.
#[derive(Default)]
struct FakeRunner {
    calls: RefCell<Vec<String>>,
    fail: bool,
}

impl FakeRunner {
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
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
}

fn sandbox() -> Sandbox {
    let root = tempfile::tempdir().expect("temp dir");
    let base = root.path();
    let paths = LinuxPaths {
        user_fonts: base.join("data").join("fonts"),
        session_fonts: base.join("cache").join("tpt-fonts").join("session"),
        conf_dir: base.join("config").join("fontconfig").join("conf.d"),
    };
    Sandbox { _root: root, paths }
}

fn sample() -> Option<PathBuf> {
    require_font("arial.ttf")
}

#[test]
fn install_copies_into_user_dir_and_refreshes_only_that_dir() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());

    let installed = backend.install(&font).expect("install");

    assert!(installed.path.starts_with(&sb.paths.user_fonts));
    assert!(installed.path.is_file());
    assert_eq!(installed.registration, "arial.ttf");
    assert_eq!(
        runner.calls(),
        vec![format!("fc-cache -f {}", sb.paths.user_fonts.display())],
        "a scoped refresh of the user directory only"
    );
}

#[test]
fn a_failed_cache_refresh_rolls_back_the_copy() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner {
        fail: true,
        ..Default::default()
    };
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());

    let err = backend.install(&font).expect_err("fc-cache failed");
    assert!(matches!(err, FontError::Backend { reason, .. } if reason.contains("boom")));
    assert!(backend.list_installed().expect("list").is_empty());
}

#[test]
fn uninstall_removes_the_file_and_refreshes() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let installed = backend.install(&font).expect("install");

    backend.uninstall(&installed).expect("uninstall");

    assert!(!installed.path.exists());
    assert_eq!(runner.calls().len(), 2);
}

#[test]
fn uninstall_refuses_paths_outside_the_user_directory() {
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let elsewhere = InstalledFont {
        path: PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        registration: "DejaVuSans.ttf".into(),
    };

    let err = backend.uninstall(&elsewhere).expect_err("refused");
    assert!(matches!(err, FontError::OutsideFontDirectory { .. }));
    assert!(runner.calls().is_empty());
}

#[test]
fn activation_copies_fonts_writes_include_and_refreshes_once() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());

    backend
        .activate_session(&[font.as_path()])
        .expect("activate");

    let session_files: Vec<_> = fs::read_dir(&sb.paths.session_fonts)
        .expect("session dir exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(session_files.len(), 1);
    assert!(session_files[0].ends_with("-arial.ttf"));

    let conf = fs::read_to_string(sb.paths.conf_dir.join("50-tpt-fonts-session.conf"))
        .expect("drop-in written");
    assert!(conf.contains(&format!("<dir>{}</dir>", sb.paths.session_fonts.display())));

    assert_eq!(
        runner.calls(),
        vec![format!("fc-cache -f {}", sb.paths.session_fonts.display())]
    );
}

#[test]
fn deactivation_removes_the_copy_and_the_drop_in_when_empty() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    backend
        .activate_session(&[font.as_path()])
        .expect("activate");

    backend
        .deactivate_session(&[font.as_path()])
        .expect("deactivate");

    assert!(
        fs::read_dir(&sb.paths.session_fonts)
            .expect("dir")
            .next()
            .is_none()
    );
    assert!(!sb.paths.conf_dir.join("50-tpt-fonts-session.conf").exists());
}

#[test]
fn clear_session_removes_everything_the_session_added() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    backend
        .activate_session(&[font.as_path()])
        .expect("activate");

    backend.clear_session().expect("clear");

    assert!(!sb.paths.session_fonts.exists());
    assert!(!sb.paths.conf_dir.join("50-tpt-fonts-session.conf").exists());
}

#[test]
fn clear_session_does_not_touch_permanent_installs() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    let installed = backend.install(&font).expect("install");
    backend
        .activate_session(&[font.as_path()])
        .expect("activate");

    backend.clear_session().expect("clear");

    assert!(
        installed.path.exists(),
        "permanent install survives session cleanup"
    );
}

#[test]
fn list_installed_reports_only_font_files() {
    let Some(font) = sample() else {
        return;
    };
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    backend.install(&font).expect("install");
    fs::write(sb.paths.user_fonts.join("README.txt"), b"not a font").expect("write");

    let listed = backend.list_installed().expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].registration, "arial.ttf");
}

#[test]
fn list_is_empty_before_anything_is_installed() {
    let sb = sandbox();
    let runner = FakeRunner::default();
    let backend = FontconfigBackend::new(&runner, sb.paths.clone());
    assert!(backend.list_installed().expect("list").is_empty());
    assert!(!sb.paths.user_fonts.exists());
}
