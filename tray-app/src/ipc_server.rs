// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Serves requests from the GUI and CLI over a local socket (a named pipe on Windows).
//!
//! Each connection carries one request line and receives one reply line. Requests are handed to
//! the UI thread, which owns the session, so this thread never touches font state directly.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::sync::mpsc;
use std::thread;

use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream, prelude::*};
use tpt_fonts_core::{Request, Response, SOCKET_NAME, decode_line, encode_line};
use winit::event_loop::EventLoopProxy;

use crate::tray::UserEvent;

/// Largest request line accepted. Requests are small; this stops a runaway client.
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

/// Takes the socket name. Fails with `AddrInUse` if another tray process holds it.
pub fn bind() -> io::Result<interprocess::local_socket::Listener> {
    let name = SOCKET_NAME.to_ns_name::<GenericNamespaced>()?;
    ListenerOptions::new().name(name).create_sync()
}

/// Accepts connections on a background thread until the process exits.
pub fn spawn(listener: interprocess::local_socket::Listener, proxy: EventLoopProxy<UserEvent>) {
    thread::spawn(move || {
        for connection in listener.incoming() {
            match connection {
                Ok(stream) => {
                    let proxy = proxy.clone();
                    thread::spawn(move || {
                        if let Err(err) = serve(stream, &proxy) {
                            report!("tpt-fonts-tray: IPC client error: {err}");
                        }
                    });
                }
                Err(err) => report!("tpt-fonts-tray: IPC accept failed: {err}"),
            }
        }
    });
}

fn serve(stream: Stream, proxy: &EventLoopProxy<UserEvent>) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    // Cap the read so a client that never sends a newline cannot exhaust memory.
    (&mut reader).take(MAX_REQUEST_BYTES).read_line(&mut line)?;

    let response = match decode_line::<Request>(&line) {
        Ok(request) => ask_ui(proxy, request),
        Err(err) => Response::Error {
            message: format!("bad request: {err}"),
        },
    };
    let bytes = encode_line(&response).map_err(|e| io::Error::other(e.to_string()))?;
    reader.get_mut().write_all(&bytes)
}

/// Sends a request to the UI thread and waits for its reply.
fn ask_ui(proxy: &EventLoopProxy<UserEvent>, request: Request) -> Response {
    let (reply_tx, reply_rx) = mpsc::channel();
    if proxy.send_event(UserEvent::Ipc(request, reply_tx)).is_err() {
        return Response::Error {
            message: "tray is shutting down".into(),
        };
    }
    reply_rx.recv().unwrap_or(Response::Error {
        message: "tray did not reply".into(),
    })
}
