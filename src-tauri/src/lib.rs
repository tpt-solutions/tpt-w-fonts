// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::path::PathBuf;

use tauri::AppHandle;
use tauri::Manager;
use tpt_fonts_core::{FontSource, PreviewState, read_font_file};

/// Opens a preview for one face of a font file.
///
/// The file is also added to the asset-protocol scope so the webview can load it through the
/// URL from `convertFileSrc`. The frontend keeps the returned state and sends it back for edits.
#[tauri::command]
fn open_preview(app: AppHandle, path: String, face_index: u32) -> Result<PreviewState, String> {
    let path = PathBuf::from(path);
    let file = read_font_file(&path, FontSource::User).map_err(|e| e.to_string())?;
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|e| e.to_string())?;
    PreviewState::new(&file, face_index).map_err(|e| e.to_string())
}

#[tauri::command]
fn preview_set_axis(
    mut state: PreviewState,
    tag: String,
    value: f32,
) -> Result<PreviewState, String> {
    state.set_axis(&tag, value).map_err(|e| e.to_string())?;
    Ok(state)
}

#[tauri::command]
fn preview_reset_axes(mut state: PreviewState) -> PreviewState {
    state.reset_axes();
    state
}

#[tauri::command]
fn preview_apply_named(mut state: PreviewState, index: usize) -> Result<PreviewState, String> {
    state
        .apply_named_instance(index)
        .map_err(|e| e.to_string())?;
    Ok(state)
}

#[tauri::command]
fn preview_save(mut state: PreviewState, name: String) -> PreviewState {
    state.save_current(name);
    state
}

#[tauri::command]
fn preview_apply_saved(mut state: PreviewState, index: usize) -> Result<PreviewState, String> {
    state.apply_saved(index).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Builds and runs the Tauri application. Shared by the desktop binary and any future mobile entry point.
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
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
