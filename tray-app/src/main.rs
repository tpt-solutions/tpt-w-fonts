// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Resident tray process. It owns session activations, serves requests from the GUI and CLI,
//! and cleans up after a previous crash on startup.

// Hide the console window on Windows release builds; debug builds keep it for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Prints a message to the console and appends it to the local log.
macro_rules! report {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        eprintln!("{message}");
        tpt_fonts_core::record_error("tray", &message);
    }};
}

/// Prints a routine message and appends it to the local log at info level.
macro_rules! note {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        eprintln!("{message}");
        tpt_fonts_core::record(tpt_fonts_core::Level::Info, "tray", &message);
    }};
}

mod ipc_server;
#[cfg(windows)]
mod logoff;
mod tray;

use std::process::ExitCode;

use tpt_fonts_core::{
    PlatformBackend, Recovery, SessionManager, SessionStore, app_state_dir, platform_backend,
    recover_orphans,
};

fn main() -> ExitCode {
    // `--cleanup` runs after the tray has stopped (for example as a systemd ExecStop). It
    // deactivates whatever the last session recorded, then exits without starting the tray.
    if std::env::args().any(|arg| arg == "--cleanup") {
        return cleanup_only();
    }
    let Some(state_dir) = app_state_dir() else {
        report!("tpt-fonts-tray: no per-user state directory could be determined");
        return ExitCode::FAILURE;
    };

    // Taking the socket first makes this the only tray instance. A second launch exits here.
    let listener = match ipc_server::bind() {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            report!("tpt-fonts-tray is already running");
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            report!("tpt-fonts-tray: could not open the IPC socket: {err}");
            return ExitCode::FAILURE;
        }
    };

    let backend: PlatformBackend = match platform_backend() {
        Ok(backend) => backend,
        Err(err) => {
            report!("tpt-fonts-tray: {err}");
            return ExitCode::FAILURE;
        }
    };

    let store = SessionStore::new(state_dir.join("session.json"));
    match recover_orphans(&backend, &store) {
        Ok(Recovery::Recovered {
            pid,
            fonts,
            cleanup_error,
        }) => {
            note!(
                "tpt-fonts-tray: recovered {} font(s) left by process {pid}",
                fonts.len()
            );
            if let Some(reason) = cleanup_error {
                report!("tpt-fonts-tray: some fonts could not be deactivated: {reason}");
            }
        }
        Ok(Recovery::OwnerRunning { pid }) => {
            // The recorded owner is alive but not answering on the socket, so the PID is most
            // likely reused by an unrelated process. Starting fresh is the safe choice.
            report!("tpt-fonts-tray: state file names live process {pid}; starting fresh");
        }
        Ok(Recovery::Clean) => {}
        Err(err) => report!("tpt-fonts-tray: recovery skipped: {err}"),
    }

    let manager = match SessionManager::start(backend, store, std::process::id()) {
        Ok(manager) => manager,
        Err(err) => {
            report!("tpt-fonts-tray: could not start the session: {err}");
            return ExitCode::FAILURE;
        }
    };

    match tray::run(manager, listener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            report!("tpt-fonts-tray: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Deactivates every font recorded by a stopped tray, then removes the state file.
fn cleanup_only() -> ExitCode {
    let Some(state_dir) = app_state_dir() else {
        report!("tpt-fonts-tray: no per-user state directory could be determined");
        return ExitCode::FAILURE;
    };
    let backend = match platform_backend() {
        Ok(backend) => backend,
        Err(err) => {
            report!("tpt-fonts-tray: {err}");
            return ExitCode::FAILURE;
        }
    };
    let store = SessionStore::new(state_dir.join("session.json"));
    // No owner is alive at this point, so any state file is ours to clean up.
    match tpt_fonts_core::recover_orphans_with(&backend, &store, |_| false, 0) {
        Ok(Recovery::Recovered {
            fonts,
            cleanup_error,
            ..
        }) => {
            note!(
                "tpt-fonts-tray: cleanup deactivated {} font(s)",
                fonts.len()
            );
            if let Some(reason) = cleanup_error {
                report!("tpt-fonts-tray: cleanup reported: {reason}");
            }
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            report!("tpt-fonts-tray: cleanup failed: {err}");
            ExitCode::FAILURE
        }
    }
}
