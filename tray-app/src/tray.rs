// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The tray icon, its menu, and the UI-thread event loop that owns the session.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use tpt_fonts_core::{
    ActivationSource, FontFile, Request, Response, SessionManager, autostart_enabled,
    cached_default_catalog, handle_request, quick_candidates, set_autostart,
};
use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use crate::ipc_server;
use tpt_fonts_core::PlatformBackend;

/// Events delivered to the UI thread.
pub enum UserEvent {
    /// A request from the IPC server, with the channel to send its reply on.
    Ipc(Request, Sender<Response>),
    /// A menu item was chosen.
    Menu(MenuId),
    /// Windows is ending the session. Deactivate everything, then reply.
    #[cfg(windows)]
    SessionEnd(Sender<()>),
}

/// What a menu item does when chosen.
enum Action {
    OpenWindow,
    Activate(PathBuf),
    Deactivate(PathBuf),
    DeactivateAll,
    ToggleAutostart,
    Quit,
}

struct App {
    manager: SessionManager<PlatformBackend>,
    exe: PathBuf,
    tray: Option<TrayIcon>,
    actions: HashMap<MenuId, Action>,
    autostart_item: Option<CheckMenuItem>,
}

pub fn run(
    manager: SessionManager<PlatformBackend>,
    listener: interprocess::local_socket::Listener,
) -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    // Menu clicks arrive on tray-icon's own channel; forward them into the event loop.
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event.id));
    }));
    #[cfg(windows)]
    {
        let session_proxy = proxy.clone();
        crate::logoff::watch(move || {
            let (reply_tx, reply_rx) = std::sync::mpsc::channel();
            if session_proxy
                .send_event(UserEvent::SessionEnd(reply_tx))
                .is_ok()
            {
                // Bounded wait: Windows gives a session-ending process only a few seconds.
                let _ = reply_rx.recv_timeout(std::time::Duration::from_secs(5));
            }
        });
    }
    ipc_server::spawn(listener, proxy);

    let exe = std::env::current_exe()?;
    let mut app = App {
        manager,
        exe,
        tray: None,
        actions: HashMap::new(),
        autostart_item: None,
    };
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut app)?;
    Ok(())
}

impl App {
    fn build_menu(&mut self) -> Menu {
        self.actions.clear();
        let menu = Menu::new();
        let count = self.manager.active().len();
        let open = MenuItem::new("Open tpt-fonts", true, None);
        self.actions.insert(open.id().clone(), Action::OpenWindow);
        append(&menu, &open);
        append(&menu, &PredefinedMenuItem::separator());

        let header = MenuItem::new(format!("Active session fonts: {count}"), false, None);
        append(&menu, &header);

        for font in self.manager.active() {
            let name = font.path.file_name().map_or_else(
                || font.path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let item = MenuItem::new(format!("Deactivate {name}"), true, None);
            self.actions
                .insert(item.id().clone(), Action::Deactivate(font.path.clone()));
            append(&menu, &item);
        }

        let quick = Submenu::new("Activate a recent font", true);
        self.fill_quick_list(&quick);
        append(&menu, &quick);

        append(&menu, &PredefinedMenuItem::separator());
        let deactivate_all = MenuItem::new("Deactivate all", count > 0, None);
        self.actions
            .insert(deactivate_all.id().clone(), Action::DeactivateAll);
        append(&menu, &deactivate_all);

        let autostart = CheckMenuItem::new("Start at login", true, autostart_enabled(), None);
        self.actions
            .insert(autostart.id().clone(), Action::ToggleAutostart);
        append(&menu, &autostart);
        self.autostart_item = Some(autostart.clone());

        append(&menu, &PredefinedMenuItem::separator());
        let quit = MenuItem::new("Quit", true, None);
        self.actions.insert(quit.id().clone(), Action::Quit);
        append(&menu, &quit);
        menu
    }

    /// Adds the newest library fonts that are not active yet. The list reads the catalog cache and
    /// does not scan, so it may lag a moment behind the library; the full search is in the GUI.
    fn fill_quick_list(&mut self, submenu: &Submenu) {
        let Some(catalog) = cached_default_catalog() else {
            append_sub(
                submenu,
                &MenuItem::new("No font library available", false, None),
            );
            return;
        };
        let active: Vec<PathBuf> = self
            .manager
            .active()
            .iter()
            .map(|f| f.path.clone())
            .collect();
        let (picked, total) = quick_candidates(
            catalog.files(),
            |path| active.iter().any(|a| a == path),
            QUICK_LIST_LIMIT,
        );
        if picked.is_empty() {
            append_sub(
                submenu,
                &MenuItem::new("No inactive fonts in the library", false, None),
            );
            return;
        }
        for file in picked {
            let item = MenuItem::new(quick_label(file), true, None);
            self.actions
                .insert(item.id().clone(), Action::Activate(file.path.clone()));
            append_sub(submenu, &item);
        }
        if total > QUICK_LIST_LIMIT {
            let more = total - QUICK_LIST_LIMIT;
            append_sub(
                submenu,
                &MenuItem::new(
                    format!("{more} more: open tpt-fonts to find them"),
                    false,
                    None,
                ),
            );
        }
    }

    /// Rebuilds the menu so it reflects the current session.
    fn refresh_menu(&mut self) {
        let menu = self.build_menu();
        if let Some(tray) = &self.tray {
            tray.set_menu(Some(Box::new(menu)));
        }
    }

    fn on_menu(&mut self, event_loop: &ActiveEventLoop, id: MenuId) {
        let Some(action) = self.actions.remove(&id) else {
            return;
        };
        match action {
            Action::OpenWindow => {
                if let Err(err) = open_window(&self.exe) {
                    report!("tpt-fonts-tray: {err}");
                }
            }
            Action::Activate(path) => {
                if let Err(err) = self.manager.activate(&path, ActivationSource::Tray) {
                    report!("tpt-fonts-tray: {err}");
                }
            }
            Action::Deactivate(path) => {
                if let Err(err) = self.manager.deactivate(&path) {
                    report!("tpt-fonts-tray: {err}");
                }
            }
            Action::DeactivateAll => {
                if let Err(err) = self.manager.deactivate_all() {
                    report!("tpt-fonts-tray: {err}");
                }
            }
            Action::ToggleAutostart => {
                let enable = !autostart_enabled();
                if let Err(err) = set_autostart(enable, &self.exe) {
                    report!("tpt-fonts-tray: could not change autostart: {err}");
                }
            }
            Action::Quit => {
                // Clean deactivation on quit: the session should not outlive the tray.
                if let Err(err) = self.manager.deactivate_all() {
                    report!("tpt-fonts-tray: {err}");
                }
                event_loop.exit();
                return;
            }
        }
        self.refresh_menu();
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        if self.tray.is_some() {
            return;
        }
        let menu = self.build_menu();
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("tpt-fonts")
            .with_icon(placeholder_icon())
            .build()
        {
            Ok(tray) => self.tray = Some(tray),
            Err(err) => report!("tpt-fonts-tray: could not create the tray icon: {err}"),
        }
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {
        // The tray has no windows.
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Ipc(request, reply) => {
                let response = handle_request(&mut self.manager, request);
                let _ = reply.send(response);
                self.refresh_menu();
            }
            UserEvent::Menu(id) => self.on_menu(event_loop, id),
            #[cfg(windows)]
            UserEvent::SessionEnd(reply) => {
                if let Err(err) = self.manager.deactivate_all() {
                    report!("tpt-fonts-tray: session end cleanup: {err}");
                }
                let _ = reply.send(());
            }
        }
    }
}

/// Starts the GUI from the folder the tray runs from. The GUI is not waited on: it runs on its own,
/// and the tray keeps the session either way.
fn open_window(tray_exe: &Path) -> Result<(), String> {
    let name = if cfg!(windows) {
        "tpt-fonts-gui.exe"
    } else {
        "tpt-fonts-gui"
    };
    let dir = tray_exe
        .parent()
        .ok_or_else(|| "the tray executable has no parent directory".to_string())?;
    let gui = dir.join(name);
    std::process::Command::new(&gui)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start {}: {e}", gui.display()))
}

/// Most fonts the quick list offers. Longer lists are a menu nobody scrolls.
const QUICK_LIST_LIMIT: usize = 40;

/// The label for a quick-list entry: the first face's family and style, or the file name.
fn quick_label(file: &FontFile) -> String {
    match file.faces.first() {
        Some(face) if face.subfamily.is_empty() => face.family.clone(),
        Some(face) => format!("{} {}", face.family, face.subfamily),
        None => file.path.file_name().map_or_else(
            || file.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        ),
    }
}

/// `muda` menus take items by reference; this keeps the call sites short.
fn append(menu: &Menu, item: &dyn IsMenuItem) {
    if let Err(err) = menu.append(item) {
        report!("tpt-fonts-tray: could not add a menu item: {err}");
    }
}

fn append_sub(submenu: &Submenu, item: &dyn IsMenuItem) {
    if let Err(err) = submenu.append(item) {
        report!("tpt-fonts-tray: could not add a menu item: {err}");
    }
}

/// A flat 32×32 square until real branding lands (todo.md Phase 10).
fn placeholder_icon() -> Icon {
    const SIZE: u32 = 32;
    let rgba: Vec<u8> = (0..SIZE * SIZE)
        .flat_map(|_| [0x1f, 0x3a, 0x5c, 0xff])
        .collect();
    Icon::from_rgba(rgba, SIZE, SIZE).expect("32x32 RGBA buffer is valid")
}
