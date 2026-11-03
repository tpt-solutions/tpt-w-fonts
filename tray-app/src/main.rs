// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Resident tray process. It owns session activations, serves requests from the GUI and CLI,
//! and cleans up after a previous crash on startup.

mod ipc_server;
mod tray;

use std::process::ExitCode;

use tpt_fonts_core::{Recovery, SessionManager, SessionStore, app_state_dir, recover_orphans};

#[cfg(windows)]
pub type Platform = tpt_fonts_core::WindowsBackend<tpt_fonts_core::NativeWin32>;

#[cfg(windows)]
fn platform() -> tpt_fonts_core::Result<Platform> {
    tpt_fonts_core::WindowsBackend::for_current_user()
}

#[cfg(not(windows))]
pub type Platform = tpt_fonts_core::FontconfigBackend<tpt_fonts_core::SystemRunner>;

#[cfg(not(windows))]
fn platform() -> tpt_fonts_core::Result<Platform> {
    let paths = tpt_fonts_core::LinuxPaths::for_current_user()?;
    Ok(tpt_fonts_core::FontconfigBackend::new(
        tpt_fonts_core::SystemRunner,
        paths,
    ))
}

fn main() -> ExitCode {
    let Some(state_dir) = app_state_dir() else {
        eprintln!("tpt-fonts-tray: no per-user state directory could be determined");
        return ExitCode::FAILURE;
    };

    // Taking the socket first makes this the only tray instance. A second launch exits here.
    let listener = match ipc_server::bind() {
        Ok(listener) => listener,
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("tpt-fonts-tray is already running");
            return ExitCode::SUCCESS;
        }
        Err(err) => {
            eprintln!("tpt-fonts-tray: could not open the IPC socket: {err}");
            return ExitCode::FAILURE;
        }
    };

    let backend = match platform() {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("tpt-fonts-tray: {err}");
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
            eprintln!(
                "tpt-fonts-tray: recovered {} font(s) left by process {pid}",
                fonts.len()
            );
            if let Some(reason) = cleanup_error {
                eprintln!("tpt-fonts-tray: some fonts could not be deactivated: {reason}");
            }
        }
        Ok(Recovery::OwnerRunning { pid }) => {
            // The recorded owner is alive but not answering on the socket, so the PID is most
            // likely reused by an unrelated process. Starting fresh is the safe choice.
            eprintln!("tpt-fonts-tray: state file names live process {pid}; starting fresh");
        }
        Ok(Recovery::Clean) => {}
        Err(err) => eprintln!("tpt-fonts-tray: recovery skipped: {err}"),
    }

    let manager = match SessionManager::start(backend, store, std::process::id()) {
        Ok(manager) => manager,
        Err(err) => {
            eprintln!("tpt-fonts-tray: could not start the session: {err}");
            return ExitCode::FAILURE;
        }
    };

    match tray::run(manager, listener) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tpt-fonts-tray: {err}");
            ExitCode::FAILURE
        }
    }
}
