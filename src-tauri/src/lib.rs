// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/// Builds and runs the Tauri application. Shared by the desktop binary and any future mobile entry point.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tpt-fonts");
}
