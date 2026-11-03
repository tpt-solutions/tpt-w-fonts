// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use ttf_parser::{Face, Permissions, Tag, name_id};

use crate::error::{FontError, Result};
use crate::format::detect_format;
use crate::fvar::parse_instances;
use crate::model::{
    EmbeddingRights, FontFace, FontFile, FontFormat, FontSource, NamedInstance, VariationAxis,
};

/// Reads a font file from disk and builds its catalog entry, including the content hash.
pub fn read_font_file(path: &Path, source: FontSource) -> Result<FontFile> {
    let io_err = |source| FontError::Io {
        path: path.to_path_buf(),
        source,
    };
    let metadata = fs::metadata(path).map_err(io_err)?;
    let data = fs::read(path).map_err(io_err)?;

    let format = detect_format(&data).ok_or_else(|| FontError::UnrecognisedFormat {
        path: path.to_path_buf(),
    })?;
    let faces = parse_faces(&data, format).map_err(|reason| FontError::Parse {
        path: path.to_path_buf(),
        reason,
    })?;

    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64);

    Ok(FontFile {
        path: path.to_path_buf(),
        source,
        format,
        size: metadata.len(),
        modified_ms,
        content_hash: blake3::hash(&data).to_hex().to_string(),
        faces,
    })
}

/// Parses every face in a font file's bytes.
///
/// Returns a human-readable reason on failure, which the caller wraps with the path.
pub fn parse_faces(data: &[u8], format: FontFormat) -> std::result::Result<Vec<FontFace>, String> {
    let count = match format {
        FontFormat::Ttc => ttf_parser::fonts_in_collection(data)
            .ok_or_else(|| "invalid font collection header".to_string())?,
        FontFormat::Ttf | FontFormat::Otf => 1,
    };

    (0..count)
        .map(|index| {
            let face = Face::parse(data, index).map_err(|e| format!("face {index}: {e:?}"))?;
            Ok(describe_face(&face, index))
        })
        .collect()
}

fn describe_face(face: &Face, index: u32) -> FontFace {
    let is_variable = face.is_variable();
    let axes: Vec<VariationAxis> = face
        .variation_axes()
        .into_iter()
        .map(|axis| VariationAxis {
            tag: axis.tag.to_string(),
            name: name_string(face, axis.name_id),
            min: axis.min_value,
            default: axis.def_value,
            max: axis.max_value,
            hidden: axis.hidden,
        })
        .collect();

    let named_instances = if is_variable {
        face.raw_face()
            .table(Tag::from_bytes(b"fvar"))
            .and_then(parse_instances)
            .unwrap_or_default()
            .into_iter()
            .map(|raw| NamedInstance {
                name: name_string(face, raw.subfamily_name_id),
                postscript_name: raw.postscript_name_id.and_then(|id| name_string(face, id)),
                coordinates: raw.coordinates,
            })
            .collect()
    } else {
        Vec::new()
    };

    FontFace {
        index,
        family: name_string(face, name_id::TYPOGRAPHIC_FAMILY)
            .or_else(|| name_string(face, name_id::FAMILY))
            .unwrap_or_default(),
        subfamily: name_string(face, name_id::TYPOGRAPHIC_SUBFAMILY)
            .or_else(|| name_string(face, name_id::SUBFAMILY))
            .unwrap_or_default(),
        full_name: name_string(face, name_id::FULL_NAME).unwrap_or_default(),
        postscript_name: name_string(face, name_id::POST_SCRIPT_NAME),
        version: name_string(face, name_id::VERSION).unwrap_or_default(),
        vendor_id: vendor_id(face),
        embedding: embedding_rights(face),
        units_per_em: face.units_per_em(),
        is_variable,
        axes,
        named_instances,
    }
}

/// Looks up the first decodable name-table string for an ID.
///
/// `ttf-parser` does not export its language enum, so this cannot yet prefer English explicitly.
/// Fonts usually list their English records first, which covers the common case.
fn name_string(face: &Face, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|name| name.name_id == id)
        .find_map(|name| name.to_string())
}

fn embedding_rights(face: &Face) -> EmbeddingRights {
    match face.permissions() {
        Some(Permissions::Installable) => EmbeddingRights::Installable,
        Some(Permissions::Restricted) => EmbeddingRights::Restricted,
        Some(Permissions::PreviewAndPrint) => EmbeddingRights::PreviewAndPrint,
        Some(Permissions::Editable) => EmbeddingRights::Editable,
        None => EmbeddingRights::Unspecified,
    }
}

/// Reads the four-byte `achVendID` at offset 58 of the face's OS/2 table.
fn vendor_id(face: &Face) -> Option<String> {
    let os2 = face.raw_face().table(Tag::from_bytes(b"OS/2"))?;
    let bytes = os2.get(58..62)?;
    let id = String::from_utf8_lossy(bytes).trim_end().to_string();
    (!id.is_empty()).then_some(id)
}
