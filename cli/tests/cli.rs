// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! End-to-end checks of the `tpt-fonts` binary. These avoid anything that needs the tray process
//! or changes installed fonts, so they are safe to run on any machine.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tpt-fonts"))
        .args(args)
        .output()
        .expect("the binary runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_lists_every_command() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    for command in [
        "list",
        "activate",
        "deactivate",
        "install",
        "uninstall",
        "scan-duplicates",
        "preview",
    ] {
        assert!(text.contains(command), "help mentions {command}");
    }
}

#[test]
fn activation_without_session_flag_is_a_usage_error() {
    let output = run(&["activate", "anything.ttf"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("session-only"));
}

#[test]
fn malformed_axis_is_rejected_by_the_parser() {
    let output = run(&["preview", "anything.ttf", "--axis", "wg=400"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("four ASCII characters"));
}

#[test]
fn previewing_an_unknown_font_reports_no_match() {
    let output = run(&["preview", "definitely-not-an-installed-font-zz9"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no font matches"));
}

#[test]
fn list_json_is_a_json_array() {
    let output = run(&["list", "--json", "--state", "session"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON on stdout");
    assert!(parsed.is_array());
}
