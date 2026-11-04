// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Deactivates session fonts when Windows is shutting down or the user logs off.
//!
//! The tray's winit event loop does not see session-end messages, so a hidden message-only window
//! runs on its own thread. On `WM_ENDSESSION` it calls the handler, which blocks until the UI
//! thread has deactivated the fonts, so the work finishes before Windows ends the process.

use std::ptr::{null, null_mut};
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG,
    RegisterClassW, TranslateMessage, WM_ENDSESSION, WM_QUERYENDSESSION, WNDCLASSW,
};

type Handler = Box<dyn Fn() + Send + Sync>;

static ON_SESSION_END: OnceLock<Handler> = OnceLock::new();

const CLASS_NAME: &str = "tpt-fonts-session-end";

/// Starts the hidden window. `on_session_end` runs on the window thread when the session ends and
/// should block until the cleanup is done.
pub fn watch(on_session_end: impl Fn() + Send + Sync + 'static) {
    if ON_SESSION_END.set(Box::new(on_session_end)).is_err() {
        return;
    }
    std::thread::spawn(run_window);
}

fn run_window() {
    let class: Vec<u16> = CLASS_NAME.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `class` is a NUL-terminated UTF-16 buffer that outlives the window. The class and
    // window are created on this thread and only used here. The message loop runs until the
    // process exits.
    unsafe {
        let instance = GetModuleHandleW(null());
        let class_def = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        if RegisterClassW(&class_def) == 0 {
            report_missing_window();
            return;
        }
        let window: HWND = CreateWindowExW(
            0,
            class.as_ptr(),
            class.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            null_mut(),
            instance,
            null(),
        );
        if window.is_null() {
            report_missing_window();
            return;
        }
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

fn report_missing_window() {
    tpt_fonts_core::record_error(
        "tray",
        "could not create the session-end window; fonts are cleaned up at next launch",
    );
}

/// The window procedure. Only session-end messages are handled; everything else takes the default.
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        // Allow the session to end. Refusing would only delay the logoff for the user.
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION if wparam != 0 => {
            if let Some(handler) = ON_SESSION_END.get() {
                handler();
            }
            0
        }
        // SAFETY: forwarding unhandled messages to the default procedure, with the arguments
        // Windows gave us.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
