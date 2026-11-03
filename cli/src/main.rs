// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! `tpt-fonts`: scriptable command-line access to the font engine and the tray process.

mod commands;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "tpt-fonts",
    version,
    about = "Font manager for designers and developers"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List scanned fonts, with whether each is installed, active in this session, or available.
    List {
        /// Only fonts whose family contains this text (case-insensitive).
        #[arg(long)]
        family: Option<String>,
        /// Only fonts in this container format.
        #[arg(long, value_enum)]
        format: Option<FormatArg>,
        /// Only variable or only static fonts.
        #[arg(long, value_enum)]
        kind: Option<KindArg>,
        /// Only fonts in this state.
        #[arg(long, value_enum)]
        state: Option<StateArg>,
        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Activate a font for this login session. Needs the tray process running.
    Activate {
        /// A font file path, file name, family, full name or PostScript name.
        font: String,
        /// Required: permanent installs use `install`.
        #[arg(long)]
        session: bool,
    },
    /// Deactivate a session font.
    Deactivate {
        /// A font file path, file name, family, full name or PostScript name.
        font: String,
    },
    /// Install a font permanently for the current user.
    Install {
        /// The font file to install.
        file: PathBuf,
        /// Accepted for explicitness. Installs are per-user in v1; system-wide installs are backlog.
        #[arg(long)]
        user: bool,
    },
    /// Remove a font installed by `install`.
    Uninstall {
        /// A file name or registration name of an installed font.
        font: String,
    },
    /// Find duplicate fonts. By default this only reports them.
    ScanDuplicates {
        /// Send the extra copies of byte-identical fonts to the trash. Keeps one copy per group.
        #[arg(long, conflicts_with = "dry_run")]
        fix: bool,
        /// Show what --fix would trash, without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Render sample text as an SVG specimen, with optional variation axes applied.
    Preview {
        /// A font file path, file name, family, full name or PostScript name.
        font: String,
        /// Face index inside a `.ttc` collection.
        #[arg(long, default_value_t = 0)]
        face: u32,
        /// A variation axis setting, such as `wght=700`. Repeat for several axes.
        #[arg(long = "axis", value_parser = parse_axis)]
        axes: Vec<(String, f32)>,
        /// The text to render.
        #[arg(long, default_value = "The quick brown fox jumps over the lazy dog")]
        text: String,
        /// Font size in pixels.
        #[arg(long, default_value_t = 72.0)]
        size: f32,
        /// Write the SVG here instead of standard output.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    Ttf,
    Otf,
    Ttc,
}

#[derive(Clone, Copy, ValueEnum)]
enum KindArg {
    Variable,
    Static,
}

#[derive(Clone, Copy, ValueEnum)]
enum StateArg {
    Installed,
    Session,
    Available,
}

fn parse_axis(text: &str) -> Result<(String, f32), String> {
    let (tag, value) = text
        .split_once('=')
        .ok_or_else(|| format!("expected TAG=VALUE, got '{text}'"))?;
    if tag.len() != 4 || !tag.is_ascii() {
        return Err(format!("axis tags are four ASCII characters, got '{tag}'"));
    }
    let value: f32 = value
        .parse()
        .map_err(|_| format!("'{value}' is not a number"))?;
    Ok((tag.to_string(), value))
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match commands::run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("tpt-fonts: {err}");
            ExitCode::FAILURE
        }
    }
}
