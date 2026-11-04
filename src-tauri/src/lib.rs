// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tauri commands the frontend calls. Each one delegates to tpt-fonts-core; nothing here holds
//! font state of its own.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tpt_fonts_core::{
    ActivationSource, DuplicateGroup, FontBackend, FontError, FontRow, FontSource, PreviewState,
    Request, Resolution, Response, autostart_enabled, build_rows, find_duplicates,
    load_default_catalog, platform_backend, read_font_file, resolve_exact_group, send_request,
    set_autostart,
};

/// The library as the frontend shows it.
#[derive(Serialize)]
struct LibraryView {
    rows: Vec<FontRow>,
    /// Whether the tray process answered. Session actions need it.
    tray_running: bool,
    /// Files the scanner found but could not read.
    unreadable: usize,
}

fn message(err: FontError) -> String {
    err.to_string()
}

/// Paths the tray reports as session-active, and whether the tray answered at all.
fn session_state() -> (Vec<PathBuf>, bool) {
    match send_request(&Request::List) {
        Ok(Response::Active { fonts }) => (fonts.into_iter().map(|f| f.path).collect(), true),
        Ok(_) => (Vec::new(), true),
        Err(_) => (Vec::new(), false),
    }
}

fn installed_paths() -> Vec<PathBuf> {
    platform_backend()
        .and_then(|backend| backend.list_installed())
        .map(|fonts| fonts.into_iter().map(|f| f.path).collect())
        .unwrap_or_default()
}

#[tauri::command]
fn library_list(app: AppHandle) -> Result<LibraryView, String> {
    let (catalog, report) = load_default_catalog().map_err(message)?;
    let (session, tray_running) = session_state();
    let installed = installed_paths();
    let rows = build_rows(&catalog, &session, &installed);
    // Each row previews its own font, so the webview needs read access to every listed file.
    let scope = app.asset_protocol_scope();
    for row in &rows {
        let _ = scope.allow_file(&row.path);
    }
    Ok(LibraryView {
        rows,
        tray_running,
        unreadable: report.errors.len(),
    })
}

#[tauri::command]
fn session_activate(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    match send_request(&Request::Activate {
        path,
        source: ActivationSource::Gui,
    })
    .map_err(message)?
    {
        Response::Ok | Response::Active { .. } => Ok(()),
        Response::Error { message } => Err(message),
    }
}

#[tauri::command]
fn session_deactivate(path: String) -> Result<(), String> {
    match send_request(&Request::Deactivate {
        path: PathBuf::from(path),
    })
    .map_err(message)?
    {
        Response::Ok | Response::Active { .. } => Ok(()),
        Response::Error { message } => Err(message),
    }
}

#[tauri::command]
fn font_install(path: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    // Parse first so the reply names a real font, not a renamed text file.
    read_font_file(&source, FontSource::User).map_err(message)?;
    let backend = platform_backend().map_err(message)?;
    let installed = backend.install(&source).map_err(message)?;
    Ok(installed.path.display().to_string())
}

#[tauri::command]
fn font_uninstall(path: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    let backend = platform_backend().map_err(message)?;
    let installed = backend.list_installed().map_err(message)?;
    let font = installed
        .iter()
        .find(|f| f.path == target)
        .ok_or_else(|| format!("{path} is not a font installed by tpt-fonts"))?;
    backend.uninstall(font).map_err(message)
}

#[tauri::command]
fn duplicate_groups() -> Result<Vec<DuplicateGroup>, String> {
    let (catalog, _) = load_default_catalog().map_err(message)?;
    Ok(find_duplicates(&catalog))
}

/// Plans (`apply` false) or carries out (`apply` true) keeping `keep` and trashing the other
/// copies in an identical-copies group. Nothing is permanently deleted.
#[tauri::command]
fn duplicate_resolve(key: String, keep: String, apply: bool) -> Result<Resolution, String> {
    let (catalog, _) = load_default_catalog().map_err(message)?;
    resolve_exact_group(&catalog, &key, Path::new(&keep), !apply).map_err(message)
}

#[tauri::command]
fn session_deactivate_all() -> Result<(), String> {
    match send_request(&Request::DeactivateAll).map_err(message)? {
        Response::Ok | Response::Active { .. } => Ok(()),
        Response::Error { message } => Err(message),
    }
}

#[tauri::command]
fn autostart_get() -> bool {
    autostart_enabled()
}

#[tauri::command]
fn autostart_set(enabled: bool) -> Result<(), String> {
    set_autostart(enabled, &tray_exe()?).map_err(message)
}

/// Starts the tray process if it is not already answering.
#[tauri::command]
fn tray_start() -> Result<(), String> {
    if session_state().1 {
        return Ok(());
    }
    Command::new(tray_exe()?)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start the tray process: {e}"))
}

/// The tray executable is installed next to the GUI executable.
fn tray_exe() -> Result<PathBuf, String> {
    let gui = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = gui
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "the GUI executable has no parent directory".to_string())?;
    let name = if cfg!(windows) {
        "tpt-fonts-tray.exe"
    } else {
        "tpt-fonts-tray"
    };
    Ok(dir.join(name))
}

/// Opens a preview for one face of a font file.
///
/// The file is also added to the asset-protocol scope so the webview can load it through the
/// URL from `convertFileSrc`. The frontend keeps the returned state and sends it back for edits.
#[tauri::command]
fn open_preview(app: AppHandle, path: String, face_index: u32) -> Result<PreviewState, String> {
    let path = PathBuf::from(path);
    let file = read_font_file(&path, FontSource::User).map_err(message)?;
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|e| e.to_string())?;
    PreviewState::new(&file, face_index).map_err(message)
}

#[tauri::command]
fn preview_set_axis(
    mut state: PreviewState,
    tag: String,
    value: f32,
) -> Result<PreviewState, String> {
    state.set_axis(&tag, value).map_err(message)?;
    Ok(state)
}

#[tauri::command]
fn preview_reset_axes(mut state: PreviewState) -> PreviewState {
    state.reset_axes();
    state
}

#[tauri::command]
fn preview_apply_named(mut state: PreviewState, index: usize) -> Result<PreviewState, String> {
    state.apply_named_instance(index).map_err(message)?;
    Ok(state)
}

#[tauri::command]
fn preview_save(mut state: PreviewState, name: String) -> PreviewState {
    state.save_current(name);
    state
}

#[tauri::command]
fn preview_apply_saved(mut state: PreviewState, index: usize) -> Result<PreviewState, String> {
    state.apply_saved(index).map_err(message)?;
    Ok(state)
}

/// Builds and runs the Tauri application. Shared by the desktop binary and any future mobile entry point.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            library_list,
            session_activate,
            session_deactivate,
            font_install,
            font_uninstall,
            duplicate_groups,
            duplicate_resolve,
            session_deactivate_all,
            autostart_get,
            autostart_set,
            tray_start,
            open_preview,
            preview_set_axis,
            preview_reset_axes,
            preview_apply_named,
            preview_save,
            preview_apply_saved,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tpt-fonts");
}
