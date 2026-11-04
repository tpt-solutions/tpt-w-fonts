// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Command implementations. Each one loads what it needs from the core and prints the result.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tpt_fonts_core::{
    ActivationSource, Catalog, DuplicateKind, FontBackend, FontError, FontFile, FontFormat,
    FontSource, InstalledFont, Request, Response, Result, find_duplicates, keep_one,
    load_default_catalog, platform_backend, render_svg, resolve, send_request,
};

use crate::Command;
use crate::{FormatArg, KindArg, StateArg};

/// Whether a font is permanently installed, active in this session, or only scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Installed,
    Session,
    Available,
}

impl State {
    fn label(self) -> &'static str {
        match self {
            State::Installed => "installed",
            State::Session => "session",
            State::Available => "available",
        }
    }
}

pub fn run(command: Command) -> Result<()> {
    match command {
        Command::List {
            family,
            format,
            kind,
            state,
            json,
        } => list(family, format, kind, state, json),
        Command::Activate { font, session } => activate(&font, session),
        Command::Deactivate { font } => deactivate(&font),
        Command::Install { file, user: _ } => install(&file),
        Command::Uninstall { font } => uninstall(&font),
        Command::ScanDuplicates { fix, dry_run } => scan_duplicates(fix, dry_run),
        Command::Preview {
            font,
            face,
            axes,
            text,
            size,
            out,
        } => preview(&font, face, &axes, &text, size, out),
    }
}

/// Loads the catalog, and reports refresh problems on stderr without failing the command.
fn catalog() -> Result<Catalog> {
    let (catalog, report) = load_default_catalog()?;
    if !report.errors.is_empty() {
        eprintln!(
            "tpt-fonts: {} font file(s) could not be read and were skipped",
            report.errors.len()
        );
    }
    Ok(catalog)
}

/// Paths of the fonts this session has activated. Empty when the tray is not running.
fn session_paths() -> Vec<PathBuf> {
    match send_request(&Request::List) {
        Ok(Response::Active { fonts }) => fonts.into_iter().map(|f| f.path).collect(),
        _ => Vec::new(),
    }
}

fn installed_fonts() -> Vec<InstalledFont> {
    platform_backend()
        .and_then(|backend| backend.list_installed())
        .unwrap_or_default()
}

fn state_of(path: &Path, session: &[PathBuf], installed: &[InstalledFont]) -> State {
    if session.iter().any(|p| p == path) {
        State::Session
    } else if installed.iter().any(|f| f.path == path) {
        State::Installed
    } else {
        State::Available
    }
}

fn list(
    family: Option<String>,
    format: Option<FormatArg>,
    kind: Option<KindArg>,
    state: Option<StateArg>,
    json: bool,
) -> Result<()> {
    let catalog = catalog()?;
    let session = session_paths();
    let installed = installed_fonts();

    let mut rows: Vec<(&FontFile, State, usize)> = Vec::new();
    for file in catalog.files() {
        let file_state = state_of(&file.path, &session, &installed);
        if let Some(wanted) = state
            && !matches_state(wanted, file_state)
        {
            continue;
        }
        if let Some(wanted) = format
            && file.format != to_format(wanted)
        {
            continue;
        }
        for face in &file.faces {
            if let Some(text) = &family
                && !face.family.to_lowercase().contains(&text.to_lowercase())
            {
                continue;
            }
            if let Some(wanted) = kind {
                let variable = matches!(wanted, KindArg::Variable);
                if face.is_variable != variable {
                    continue;
                }
            }
            rows.push((file, file_state, face.index as usize));
        }
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    if json {
        let items: Vec<serde_json::Value> = rows
            .iter()
            .map(|(file, state, index)| {
                let face = &file.faces[*index];
                serde_json::json!({
                    "path": file.path,
                    "state": state.label(),
                    "face": face.index,
                    "family": face.family,
                    "subfamily": face.subfamily,
                    "format": format!("{:?}", file.format),
                    "variable": face.is_variable,
                    "source": match file.source {
                        FontSource::User => "user",
                        FontSource::System => "system",
                    },
                })
            })
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&items)?).map_err(io_error)?;
    } else {
        for (file, state, index) in &rows {
            let face = &file.faces[*index];
            writeln!(
                out,
                "{:<10} {:<9} {:<28} {:<20} {}",
                state.label(),
                format!("{:?}", file.format),
                truncate(&face.family, 28),
                truncate(&face.subfamily, 20),
                file.path.display()
            )
            .map_err(io_error)?;
        }
        writeln!(out, "{} face(s)", rows.len()).map_err(io_error)?;
    }
    Ok(())
}

fn matches_state(wanted: StateArg, actual: State) -> bool {
    matches!(
        (wanted, actual),
        (StateArg::Installed, State::Installed)
            | (StateArg::Session, State::Session)
            | (StateArg::Available, State::Available)
    )
}

fn to_format(arg: FormatArg) -> FontFormat {
    match arg {
        FormatArg::Ttf => FontFormat::Ttf,
        FormatArg::Otf => FontFormat::Otf,
        FormatArg::Ttc => FontFormat::Ttc,
    }
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        let kept: String = text.chars().take(width - 1).collect();
        format!("{kept}…")
    }
}

/// Resolves a user's font argument to a path: an existing file wins, otherwise the catalog.
fn font_path(font: &str) -> Result<PathBuf> {
    let direct = PathBuf::from(font);
    if direct.is_file() {
        return std::path::absolute(&direct).map_err(|source| FontError::Io {
            path: direct.clone(),
            source,
        });
    }
    let catalog = catalog()?;
    Ok(resolve(catalog.files(), font)?.path.clone())
}

fn activate(font: &str, session: bool) -> Result<()> {
    if !session {
        return Err(FontError::Usage(
            "activation is session-only; pass --session, or use `install` for a permanent install"
                .into(),
        ));
    }
    let path = font_path(font)?;
    match send_request(&Request::Activate {
        path: path.clone(),
        source: ActivationSource::Cli,
    })? {
        Response::Ok => {
            println!("activated {}", path.display());
            Ok(())
        }
        Response::Error { message } => Err(tray_error(message)),
        Response::Active { .. } => Ok(()),
    }
}

fn deactivate(font: &str) -> Result<()> {
    let path = font_path(font)?;
    match send_request(&Request::Deactivate { path: path.clone() })? {
        Response::Ok => {
            println!("deactivated {}", path.display());
            Ok(())
        }
        Response::Error { message } => Err(tray_error(message)),
        Response::Active { .. } => Ok(()),
    }
}

fn tray_error(message: String) -> FontError {
    FontError::Backend {
        path: PathBuf::new(),
        reason: message,
    }
}

fn install(file: &Path) -> Result<()> {
    let backend = platform_backend()?;
    let installed = backend.install(file)?;
    println!("installed {}", installed.path.display());
    Ok(())
}

fn uninstall(font: &str) -> Result<()> {
    let backend = platform_backend()?;
    let installed = backend.list_installed()?;
    let matches: Vec<&InstalledFont> = installed
        .iter()
        .filter(|f| {
            let name = f
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            name.eq_ignore_ascii_case(font) || f.registration.eq_ignore_ascii_case(font)
        })
        .collect();
    match matches.as_slice() {
        [] => Err(FontError::NotFound(font.to_string())),
        [single] => {
            backend.uninstall(single)?;
            println!("uninstalled {}", single.path.display());
            Ok(())
        }
        many => Err(FontError::Ambiguous {
            query: font.to_string(),
            paths: many.iter().map(|f| f.path.clone()).collect(),
        }),
    }
}

fn scan_duplicates(fix: bool, dry_run: bool) -> Result<()> {
    let catalog = catalog()?;
    let groups = find_duplicates(&catalog);
    if groups.is_empty() {
        println!("no duplicates found");
        return Ok(());
    }

    for group in &groups {
        let label = match group.kind {
            DuplicateKind::ExactContent => "identical copies",
            DuplicateKind::SameMetadata => "same metadata, different files",
        };
        println!("{label} ({}):", group.key);
        for member in &group.members {
            println!("  {}", member.display());
        }
    }

    if !(fix || dry_run) {
        println!("\nrun with --dry-run to see what --fix would trash");
        return Ok(());
    }

    // Only byte-identical copies are safe to remove automatically. Metadata matches can differ in
    // content, so they stay report-only.
    let apply = fix && !dry_run;
    for group in groups
        .iter()
        .filter(|g| g.kind == DuplicateKind::ExactContent)
    {
        let keep = preferred_keeper(&catalog, &group.members);
        let resolution = keep_one(&catalog, group, &keep, !apply)?;
        let verb = if apply { "trashed" } else { "would trash" };
        for path in &resolution.trashed {
            println!("{verb} {} (keeping {})", path.display(), keep.display());
        }
        for path in &resolution.skipped_system {
            println!("skipped system font {}", path.display());
        }
    }
    Ok(())
}

/// Keeps a user font over a system one, then the shortest path, so the result is stable.
fn preferred_keeper(catalog: &Catalog, members: &[PathBuf]) -> PathBuf {
    let source_of = |path: &PathBuf| {
        catalog
            .files()
            .iter()
            .find(|f| &f.path == path)
            .map(|f| f.source)
    };
    members
        .iter()
        .min_by_key(|path| {
            let system = source_of(path) == Some(FontSource::System);
            (system, path.as_os_str().len(), path.as_os_str().to_owned())
        })
        .cloned()
        .unwrap_or_default()
}

fn preview(
    font: &str,
    face: u32,
    axes: &[(String, f32)],
    text: &str,
    size: f32,
    out: Option<PathBuf>,
) -> Result<()> {
    let path = font_path(font)?;
    let svg = render_svg(&path, face, axes, text, size)?;
    match out {
        Some(file) => {
            fs::write(&file, svg).map_err(|source| FontError::Io {
                path: file.clone(),
                source,
            })?;
            println!("wrote {}", file.display());
        }
        None => {
            io::stdout().write_all(svg.as_bytes()).map_err(io_error)?;
        }
    }
    Ok(())
}

fn io_error(source: io::Error) -> FontError {
    FontError::Io {
        path: PathBuf::from("stdout"),
        source,
    }
}
