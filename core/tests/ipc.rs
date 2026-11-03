// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tpt_fonts_core::{
    ActivationSource, FontBackend, FontError, InstalledFont, Request, Response, Result,
    SessionManager, SessionStore, handle_request,
};

/// Accepts every activation unless `refuse` is set. The flag is shared so a test can flip it
/// after the manager has taken ownership of the backend.
#[derive(Default)]
struct SwitchBackend {
    refuse: Rc<Cell<bool>>,
}

impl FontBackend for SwitchBackend {
    fn install(&self, _source: &Path) -> Result<InstalledFont> {
        unimplemented!("not used by IPC tests")
    }

    fn uninstall(&self, _font: &InstalledFont) -> Result<()> {
        unimplemented!("not used by IPC tests")
    }

    fn list_installed(&self) -> Result<Vec<InstalledFont>> {
        Ok(Vec::new())
    }

    fn activate_session(&self, paths: &[&Path]) -> Result<()> {
        if self.refuse.get() {
            return Err(FontError::Backend {
                path: paths[0].to_path_buf(),
                reason: "refused".into(),
            });
        }
        Ok(())
    }

    fn deactivate_session(&self, _paths: &[&Path]) -> Result<()> {
        Ok(())
    }
}

fn manager(dir: &Path, refuse: Rc<Cell<bool>>) -> SessionManager<SwitchBackend> {
    let backend = SwitchBackend { refuse };
    SessionManager::start(backend, SessionStore::new(dir.join("s.json")), 1).expect("start")
}

#[test]
fn activate_then_list_reports_the_font() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut mgr = manager(dir.path(), Rc::default());
    let path = font_file("ipc-alpha");

    let reply = handle_request(
        &mut mgr,
        Request::Activate {
            path: path.clone(),
            source: ActivationSource::Cli,
        },
    );
    assert_eq!(reply, Response::Ok);

    match handle_request(&mut mgr, Request::List) {
        Response::Active { fonts } => {
            assert_eq!(fonts.len(), 1);
            assert_eq!(fonts[0].path, path);
            assert_eq!(fonts[0].source, ActivationSource::Cli);
        }
        other => panic!("expected the active list, got {other:?}"),
    }
}

#[test]
fn a_refused_activation_becomes_an_error_reply_not_a_panic() {
    let dir = tempfile::tempdir().expect("temp dir");
    let refuse = Rc::new(Cell::new(true));
    let mut mgr = manager(dir.path(), refuse);

    let reply = handle_request(
        &mut mgr,
        Request::Activate {
            path: font_file("ipc-bad"),
            source: ActivationSource::Gui,
        },
    );
    match reply {
        Response::Error { message } => assert!(message.contains("refused")),
        other => panic!("expected an error reply, got {other:?}"),
    }
    assert_eq!(
        handle_request(&mut mgr, Request::List),
        Response::Active { fonts: Vec::new() },
        "a refused font is not recorded"
    );
}

#[test]
fn deactivate_all_clears_the_list() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut mgr = manager(dir.path(), Rc::default());
    handle_request(
        &mut mgr,
        Request::Activate {
            path: font_file("ipc-a"),
            source: ActivationSource::Tray,
        },
    );

    assert_eq!(
        handle_request(&mut mgr, Request::DeactivateAll),
        Response::Ok
    );
    assert_eq!(
        handle_request(&mut mgr, Request::List),
        Response::Active { fonts: Vec::new() }
    );
}

/// A real, if empty, `.ttf` path under the system temp directory. The session only checks that
/// the file exists and has a font extension; the backend is a fake, so the bytes do not matter.
fn font_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("tpt-fonts-session-tests");
    std::fs::create_dir_all(&dir).expect("temp subdir");
    let path = dir.join(format!("{name}.ttf"));
    std::fs::write(&path, b"placeholder").expect("write placeholder");
    path
}
