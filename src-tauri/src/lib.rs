// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Tauri commands the frontend calls. Each one delegates to tpt-fonts-core; nothing here holds
//! font state of its own.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};

use serde::Serialize;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tpt_fonts_core::{
    ActivationSource, DuplicateGroup, FontBackend, FontError, FontRow, FontSource, FontWatcher,
    PreviewState, Request, Resolution, Response, add_scan_dir, autostart_enabled, build_rows,
    find_duplicates, load_default_catalog, platform_backend, read_font_file, remove_scan_dir,
    resolve_exact_group, scan_dir_settings, scan_roots, send_request, set_autostart,
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

/// The TPT Solutions tools page, linked from the footer.
const TOOLS_URL: &str = "https://tptsolutions.co.nz/tools";

/// Opens the tools page in the default browser. The URL is fixed here, so the webview cannot
/// use this command to open anything else.
#[tauri::command]
fn open_tools_page() -> Result<(), String> {
    let spawned = if cfg!(windows) {
        Command::new("explorer").arg(TOOLS_URL).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(TOOLS_URL).spawn()
    } else {
        Command::new("xdg-open").arg(TOOLS_URL).spawn()
    };
    spawned
        .map(|_| ())
        .map_err(|e| format!("could not open the browser: {e}"))
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

/// Sends a signal to the library watcher when the set of scan folders changes.
struct WatchRestart(Sender<()>);

#[derive(Serialize)]
struct ScanDirsView {
    /// The folders the OS provides. They are shown for reference and cannot be removed.
    defaults: Vec<String>,
    extra: Vec<ExtraScanDir>,
}

#[derive(Serialize)]
struct ExtraScanDir {
    path: String,
    /// False when the folder has gone missing since it was added.
    exists: bool,
}

#[tauri::command]
fn scan_dirs_get() -> Result<ScanDirsView, String> {
    let settings = scan_dir_settings().map_err(message)?;
    Ok(ScanDirsView {
        defaults: settings
            .defaults
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        extra: settings
            .extra
            .iter()
            .map(|p| ExtraScanDir {
                path: p.display().to_string(),
                exists: p.is_dir(),
            })
            .collect(),
    })
}

#[tauri::command]
fn scan_dirs_add(path: String, restart: State<WatchRestart>) -> Result<(), String> {
    add_scan_dir(Path::new(&path)).map_err(message)?;
    // The watcher may be dead if the folder list was empty at startup; the signal is best effort.
    let _ = restart.0.send(());
    Ok(())
}

#[tauri::command]
fn scan_dirs_remove(path: String, restart: State<WatchRestart>) -> Result<(), String> {
    remove_scan_dir(Path::new(&path)).map_err(message)?;
    let _ = restart.0.send(());
    Ok(())
}

/// Polls the tray so the library follows session changes made elsewhere, such as from the CLI or
/// the tray menu. An event is sent only when the active set or the tray's presence changes, so an
/// idle poll does nothing. The first reading is not an event: the library loads on its own.
fn watch_session(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<(Vec<PathBuf>, bool)> = None;
        loop {
            let mut state = session_state();
            state.0.sort();
            if last.as_ref() != Some(&state) {
                if last.is_some() && app.emit("session-changed", ()).is_err() {
                    return;
                }
                last = Some(state);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

/// Watches the font folders and tells the frontend when they change, so the library stays current
/// without a manual rescan. A burst of changes (installing one font writes several files) becomes
/// a single event after a short quiet period. When the scan folders change, the watches are rebuilt
/// for the new set.
fn watch_library(app: AppHandle, restart: Receiver<()>) {
    std::thread::spawn(move || {
        loop {
            let roots: Vec<PathBuf> = scan_roots().into_iter().map(|r| r.path).collect();
            let refs: Vec<&Path> = roots.iter().map(PathBuf::as_path).collect();
            let Ok((watcher, changes)) = FontWatcher::start(&refs) else {
                // Nothing to watch with this folder list. Wait for it to change before retrying.
                if restart.recv().is_err() {
                    return;
                }
                continue;
            };
            // The watcher stops when dropped, which happens at the end of each pass.
            let _watcher = watcher;
            loop {
                match changes.recv_timeout(Duration::from_millis(250)) {
                    Ok(_) => {
                        // Keep draining until the folders have been quiet for a moment.
                        while changes.recv_timeout(Duration::from_millis(400)).is_ok() {}
                        if app.emit("library-changed", ()).is_err() {
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                if restart.try_recv().is_ok() {
                    while restart.try_recv().is_ok() {}
                    break;
                }
            }
        }
    });
}

/// Builds and runs the Tauri application. Shared by the desktop binary and any future mobile entry point.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let (restart_tx, restart_rx) = mpsc::channel();
            app.manage(WatchRestart(restart_tx));
            watch_library(app.handle().clone(), restart_rx);
            watch_session(app.handle().clone());
            Ok(())
        })
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
            open_tools_page,
            scan_dirs_get,
            scan_dirs_add,
            scan_dirs_remove,
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
