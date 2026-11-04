// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Headless specimen export: renders sample text as SVG outlines, with variation axes applied.
//!
//! The output is plain vector paths, so it needs no font loading or rasteriser to view.

use std::fmt::Write as _;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use ttf_parser::{Face, GlyphId, OutlineBuilder, Tag};

use crate::error::{FontError, Result};

/// Padding around the text, in pixels.
const PADDING: f32 = 16.0;

/// Renders `text` in `face_index` of the font at `font_path` as an SVG document.
///
/// `axes` are `(tag, value)` pairs such as `("wght", 700.0)`. The font clamps values to the axis
/// range. Unknown tags are an error.
pub fn render_svg(
    font_path: &Path,
    face_index: u32,
    axes: &[(String, f32)],
    text: &str,
    size_px: f32,
) -> Result<String> {
    // Same guard as the face parser: a debug-only assertion in ttf-parser must not crash the caller.
    match catch_unwind(AssertUnwindSafe(|| {
        render_svg_unchecked(font_path, face_index, axes, text, size_px)
    })) {
        Ok(result) => result,
        Err(_) => Err(FontError::Parse {
            path: font_path.to_path_buf(),
            reason: "malformed font data".into(),
        }),
    }
}

fn render_svg_unchecked(
    font_path: &Path,
    face_index: u32,
    axes: &[(String, f32)],
    text: &str,
    size_px: f32,
) -> Result<String> {
    let data = fs::read(font_path).map_err(|source| FontError::Io {
        path: font_path.to_path_buf(),
        source,
    })?;
    let mut face = Face::parse(&data, face_index).map_err(|e| FontError::Parse {
        path: font_path.to_path_buf(),
        reason: format!("face {face_index}: {e:?}"),
    })?;

    for (tag, value) in axes {
        let tag_bytes: [u8; 4] = tag
            .as_bytes()
            .try_into()
            .map_err(|_| FontError::UnknownAxis(tag.clone()))?;
        let axis_tag = Tag::from_bytes(&tag_bytes);
        // `set_variation` reports success for axes the font lacks, so check the font's own list.
        if !face
            .variation_axes()
            .into_iter()
            .any(|axis| axis.tag == axis_tag)
        {
            return Err(FontError::UnknownAxis(tag.clone()));
        }
        face.set_variation(axis_tag, *value)
            .ok_or_else(|| FontError::UnknownAxis(tag.clone()))?;
    }

    let units = f32::from(face.units_per_em().max(1));
    let scale = size_px / units;
    let ascent = f32::from(face.ascender()) * scale;
    let descent = f32::from(face.descender()) * scale;

    let mut outline = String::new();
    let mut pen_x = PADDING;
    let baseline = PADDING + ascent;
    for ch in text.chars() {
        // Characters the font lacks fall back to glyph 0, the font's .notdef glyph.
        let glyph = face.glyph_index(ch).unwrap_or(GlyphId(0));
        let mut builder = SvgPath {
            d: &mut outline,
            x: pen_x,
            baseline,
            scale,
        };
        face.outline_glyph(glyph, &mut builder);
        let advance = face.glyph_hor_advance(glyph).unwrap_or(0);
        pen_x += f32::from(advance) * scale;
    }

    let width = pen_x + PADDING;
    let height = PADDING * 2.0 + (ascent - descent);
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.2}\" height=\"{height:.2}\" viewBox=\"0 0 {width:.2} {height:.2}\">\n  <path fill=\"#000000\" d=\"{}\"/>\n</svg>\n",
        outline.trim_end()
    ))
}

/// Converts font units into SVG coordinates. The y axis is flipped so glyphs stand upright.
struct SvgPath<'a> {
    d: &'a mut String,
    x: f32,
    baseline: f32,
    scale: f32,
}

impl SvgPath<'_> {
    fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.scale, self.baseline - y * self.scale)
    }
}

impl OutlineBuilder for SvgPath<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        let _ = write!(self.d, "M{x:.2} {y:.2} ");
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        let _ = write!(self.d, "L{x:.2} {y:.2} ");
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        let _ = write!(self.d, "Q{x1:.2} {y1:.2} {x:.2} {y:.2} ");
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        let _ = write!(self.d, "C{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2} ");
    }

    fn close(&mut self) {
        self.d.push_str("Z ");
    }
}
