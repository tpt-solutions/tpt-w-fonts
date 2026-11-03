// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

mod common;

use tpt_fonts_core::{FontFormat, FontSource, read_font_file};

use common::require_font;

#[test]
fn parses_static_truetype() {
    let Some(path) = require_font("arial.ttf") else {
        return;
    };
    let file = read_font_file(&path, FontSource::System).expect("arial.ttf parses");

    assert_eq!(file.format, FontFormat::Ttf);
    assert_eq!(file.faces.len(), 1);
    let face = &file.faces[0];
    assert_eq!(face.family, "Arial");
    assert_eq!(face.units_per_em, 2048);
    assert!(!face.is_variable);
    assert!(face.axes.is_empty());
    assert!(face.named_instances.is_empty());
    assert_eq!(file.content_hash.len(), 64, "BLAKE3 hex is 64 chars");
}

#[test]
fn parses_variable_font_axes_and_named_instances() {
    let Some(path) = require_font("bahnschrift.ttf") else {
        return;
    };
    let file = read_font_file(&path, FontSource::System).expect("bahnschrift.ttf parses");
    let face = &file.faces[0];

    assert!(face.is_variable);
    let wght = face
        .axes
        .iter()
        .find(|axis| axis.tag == "wght")
        .expect("weight axis present");
    assert!(wght.min <= wght.default && wght.default <= wght.max);
    assert!(
        !face.named_instances.is_empty(),
        "variable fonts ship named instances"
    );
    for instance in &face.named_instances {
        assert_eq!(instance.coordinates.len(), face.axes.len());
        assert!(
            instance.name.is_some(),
            "instance names resolve via the name table"
        );
    }
}

#[test]
fn parses_every_face_in_a_collection() {
    let Some(path) = require_font("msgothic.ttc") else {
        return;
    };
    let file = read_font_file(&path, FontSource::System).expect("msgothic.ttc parses");

    assert_eq!(file.format, FontFormat::Ttc);
    assert!(file.faces.len() > 1, "a collection holds several faces");
    for (expected, face) in file.faces.iter().enumerate() {
        assert_eq!(face.index as usize, expected);
    }
}

#[test]
fn rejects_non_font_file_with_a_path_in_the_error() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("fake.ttf");
    std::fs::write(&path, b"this is not a font").expect("write");

    let err = read_font_file(&path, FontSource::User).expect_err("garbage is rejected");
    assert!(err.to_string().contains("fake.ttf"));
}
