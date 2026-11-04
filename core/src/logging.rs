// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Local error logging. Nothing is sent anywhere: entries go to a file in the per-user state
//! directory, which the user can open or delete. Logging never fails the caller.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Rotate once the log passes this size. The old log is kept as `<name>.1`.
const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// The log file used by the tray and the GUI: `<app state dir>/logs/tpt-fonts.log`.
pub fn default_log_path() -> Option<PathBuf> {
    crate::platform::app_state_dir().map(|dir| dir.join("logs").join("tpt-fonts.log"))
}

/// Appends one entry to the default log. Does nothing if no state directory is available.
pub fn record(level: Level, component: &str, message: &str) {
    if let Some(path) = default_log_path() {
        record_to(&path, level, component, message);
    }
}

/// Appends an error entry to the default log.
pub fn record_error(component: &str, message: &str) {
    record(Level::Error, component, message);
}

/// Appends one entry to `path`, creating the directory and rotating the file as needed.
/// Errors are ignored on purpose: a logging failure must not break the operation being logged.
pub fn record_to(path: &Path, level: Level, component: &str, message: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(path).is_ok_and(|m| m.len() >= MAX_LOG_BYTES) {
        let _ = fs::rename(path, rotated_name(path));
    }
    let line = format!(
        "[{}] {} {}: {}\n",
        unix_seconds(),
        level.label(),
        component,
        // Keep each entry on one line, so a multi-line error cannot forge extra entries.
        message.replace(['\r', '\n'], " ")
    );
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

fn rotated_name(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_appended_one_per_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = dir.path().join("logs").join("test.log");
        record_to(&log, Level::Error, "tray", "first");
        record_to(&log, Level::Info, "cli", "second\nforged line");

        let text = fs::read_to_string(&log).expect("log exists");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "a newline in a message cannot add an entry");
        assert!(lines[0].contains("ERROR tray: first"));
        assert!(lines[1].contains("INFO cli: second forged line"));
    }

    #[test]
    fn the_log_rotates_when_it_grows_past_the_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = dir.path().join("test.log");
        fs::write(&log, vec![b'x'; MAX_LOG_BYTES as usize]).expect("prefill");

        record_to(&log, Level::Warn, "tray", "after rotation");

        assert!(rotated_name(&log).exists(), "the full log is kept as .1");
        let fresh = fs::read_to_string(&log).expect("new log");
        assert!(fresh.contains("WARN tray: after rotation"));
        assert!(fresh.len() < MAX_LOG_BYTES as usize);
    }
}
