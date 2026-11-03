// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::Path;

use tpt_fonts_core::{desktop_entry, write_desktop_entry};

#[test]
fn desktop_entry_launches_the_executable_quoted() {
    let entry = desktop_entry(Path::new("/opt/tpt fonts/tpt-fonts-tray"));
    assert!(entry.starts_with("[Desktop Entry]\n"));
    assert!(entry.contains("Exec=\"/opt/tpt fonts/tpt-fonts-tray\"\n"));
    assert!(entry.contains("Terminal=false"));
}

#[test]
fn enabling_writes_and_disabling_removes_the_entry() {
    let config = tempfile::tempdir().expect("temp dir");
    let exe = Path::new("/usr/bin/tpt-fonts-tray");
    let file = config
        .path()
        .join("autostart")
        .join("tpt-fonts-tray.desktop");

    write_desktop_entry(config.path(), true, exe).expect("enable");
    assert!(file.is_file());
    assert_eq!(
        std::fs::read_to_string(&file).expect("read"),
        desktop_entry(exe)
    );

    write_desktop_entry(config.path(), false, exe).expect("disable");
    assert!(!file.exists());
}

#[test]
fn disabling_when_already_disabled_is_not_an_error() {
    let config = tempfile::tempdir().expect("temp dir");
    write_desktop_entry(config.path(), false, Path::new("/x")).expect("no-op");
}
