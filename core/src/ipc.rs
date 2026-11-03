// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Requests the GUI and CLI send to the resident tray process, and the replies it returns.
//!
//! The wire format is one JSON object per line. The transport (a named pipe on Windows, a
//! local socket elsewhere) lives in the tray app. This module holds the message types, the
//! codec and the dispatcher, so the behaviour can be tested without any socket.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::backend::FontBackend;
use crate::error::Result;
use crate::session::{ActivationSource, ActiveFont, SessionManager};

/// Name the tray process listens on. Used as a named pipe on Windows and an abstract local
/// socket on Linux.
pub const SOCKET_NAME: &str = "tpt-fonts-tray";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Request {
    Activate {
        path: PathBuf,
        source: ActivationSource,
    },
    Deactivate {
        path: PathBuf,
    },
    DeactivateAll,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Active { fonts: Vec<ActiveFont> },
    Error { message: String },
}

/// Serialises a message as a single newline-terminated JSON line.
pub fn encode_line<T: Serialize>(message: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(message)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Parses one line (without its newline) into a message.
pub fn decode_line<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T> {
    Ok(serde_json::from_str(line.trim_end())?)
}

/// Applies a request to the session and produces the reply. Errors become [`Response::Error`]
/// so a bad request never takes the tray process down.
pub fn handle_request<B: FontBackend>(
    manager: &mut SessionManager<B>,
    request: Request,
) -> Response {
    let outcome = match request {
        Request::Activate { path, source } => manager.activate(&path, source),
        Request::Deactivate { path } => manager.deactivate(&path),
        Request::DeactivateAll => manager.deactivate_all(),
        Request::List => {
            return Response::Active {
                fonts: manager.active().to_vec(),
            };
        }
    };
    match outcome {
        Ok(()) => Response::Ok,
        Err(err) => Response::Error {
            message: err.to_string(),
        },
    }
}

/// Whether `path` names something the session may activate: an existing font file.
pub fn is_activatable(path: &Path) -> bool {
    path.is_file() && crate::format::is_font_extension(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_as_one_json_line() {
        let request = Request::Activate {
            path: PathBuf::from("C:/fonts/Alpha.ttf"),
            source: ActivationSource::Cli,
        };
        let line = encode_line(&request).expect("encodes");
        assert_eq!(line.last(), Some(&b'\n'));
        assert_eq!(
            line.iter().filter(|&&b| b == b'\n').count(),
            1,
            "exactly one newline"
        );

        let text = String::from_utf8(line).expect("utf-8");
        let decoded: Request = decode_line(&text).expect("decodes");
        assert_eq!(decoded, request);
    }

    #[test]
    fn wire_format_uses_snake_case_action_names() {
        let text = String::from_utf8(encode_line(&Request::DeactivateAll).expect("encodes"))
            .expect("utf-8");
        assert_eq!(text.trim_end(), r#"{"action":"deactivate_all"}"#);
    }

    #[test]
    fn malformed_lines_are_errors_not_panics() {
        assert!(decode_line::<Request>("{\"action\":\"explode\"}").is_err());
        assert!(decode_line::<Request>("not json").is_err());
    }

    #[test]
    fn error_replies_carry_a_message() {
        let text = serde_json::to_string(&Response::Error {
            message: "refused".into(),
        })
        .expect("encodes");
        assert_eq!(text, r#"{"status":"error","message":"refused"}"#);
    }
}
