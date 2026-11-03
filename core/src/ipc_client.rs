// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Client side of the tray's IPC channel, used by the CLI.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use interprocess::local_socket::{GenericNamespaced, Stream, prelude::*};

use crate::error::{FontError, Result};
use crate::ipc::{Request, Response, SOCKET_NAME, decode_line, encode_line};

/// Sends one request to the running tray and returns its reply.
///
/// Fails with [`FontError::TrayNotRunning`] when nothing is listening on the socket.
pub fn send(request: &Request) -> Result<Response> {
    let name = SOCKET_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|_| FontError::TrayNotRunning)?;
    let stream = Stream::connect(name).map_err(|_| FontError::TrayNotRunning)?;

    let socket = PathBuf::from(SOCKET_NAME);
    let mut reader = BufReader::new(stream);
    reader
        .get_mut()
        .write_all(&encode_line(request)?)
        .map_err(|source| FontError::Io {
            path: socket.clone(),
            source,
        })?;

    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|source| FontError::Io {
            path: socket,
            source,
        })?;
    decode_line(&line)
}
