// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The tray icon, its menu, and the UI-thread event loop that owns the session.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use tpt_fonts_core::{
    Request, Response, SessionManager, autostart_enabled, handle_request, set_autostart,
};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
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
    SessionEnd(Sender<()>),
}

/// What a menu item does when chosen.
enum Action {
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
            let (reply_tx, reply_rx) = mpsc::channel();
            if session_proxy
                .send_event(UserEvent::SessionEnd(reply_tx))
                .is_ok()
            {
                // Bounded wait: Windows gives a session-ending process only a few seconds.
                let _ = reply_rx.recv_timeout(Duration::from_secs(5));
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
            UserEvent::SessionEnd(reply) => {
                if let Err(err) = self.manager.deactivate_all() {
                    report!("tpt-fonts-tray: session end cleanup: {err}");
                }
                let _ = reply.send(());
            }
        }
    }
}

/// `muda` menus take items by reference; this keeps the call sites short.
fn append(menu: &Menu, item: &dyn tray_icon::menu::IsMenuItem) {
    if let Err(err) = menu.append(item) {
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
