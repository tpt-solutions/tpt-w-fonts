// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! The live-preview model for a single face: axis values, sample text, size and saved instances.
//!
//! The frontend holds one [`PreviewState`], sends it back to the core on every change, and
//! renders it with the CSS produced by [`PreviewState::variation_settings`]. Keeping the
//! clamping and CSS formatting here means the UI cannot send out-of-range axis values.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{FontError, Result};
use crate::model::{FontFile, NamedInstance};

/// Sizes (in px) used for the specimen/waterfall view.
pub const WATERFALL_SIZES: [f32; 6] = [12.0, 16.0, 24.0, 36.0, 48.0, 72.0];

/// Default sample text when the user has not typed anything.
pub const DEFAULT_SAMPLE_TEXT: &str = "The quick brown fox jumps over the lazy dog";

const DEFAULT_SIZE_PX: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Background {
    Light,
    Dark,
}

/// One variation axis with its range and the value currently in use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisValue {
    pub tag: String,
    pub name: Option<String>,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub value: f32,
    pub hidden: bool,
}

/// A user-saved combination of axis values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedInstance {
    pub name: String,
    /// Coordinates in the same order as [`PreviewState::axes`].
    pub coordinates: Vec<f32>,
}

/// Everything needed to render one preview of one face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewState {
    pub font_path: PathBuf,
    pub face_index: u32,
    pub family: String,
    pub subfamily: String,
    /// Static fonts have no axes; the UI shows the same view without sliders.
    pub is_variable: bool,
    pub axes: Vec<AxisValue>,
    /// Named instances shipped with the font (from `fvar`).
    pub named_instances: Vec<NamedInstance>,
    pub saved_instances: Vec<SavedInstance>,
    pub sample_text: String,
    pub size_px: f32,
    pub background: Background,
}

impl PreviewState {
    /// Builds a preview for one face of `file`, with every axis at its default.
    pub fn new(file: &FontFile, face_index: u32) -> Result<Self> {
        let face = file
            .faces
            .iter()
            .find(|face| face.index == face_index)
            .ok_or(FontError::NoSuchFace(face_index))?;

        let axes = face
            .axes
            .iter()
            .map(|axis| AxisValue {
                tag: axis.tag.clone(),
                name: axis.name.clone(),
                min: axis.min,
                default: axis.default,
                max: axis.max,
                value: axis.default,
                hidden: axis.hidden,
            })
            .collect();

        Ok(PreviewState {
            font_path: file.path.clone(),
            face_index,
            family: face.family.clone(),
            subfamily: face.subfamily.clone(),
            is_variable: face.is_variable,
            axes,
            named_instances: face.named_instances.clone(),
            saved_instances: Vec::new(),
            sample_text: DEFAULT_SAMPLE_TEXT.to_string(),
            size_px: DEFAULT_SIZE_PX,
            background: Background::Light,
        })
    }

    /// Sets an axis to `value`, clamped to the axis range. Returns the value actually applied.
    pub fn set_axis(&mut self, tag: &str, value: f32) -> Result<f32> {
        let axis = self.axis_mut(tag)?;
        let applied = value.clamp(axis.min, axis.max);
        axis.value = applied;
        Ok(applied)
    }

    /// Resets every axis to its default.
    pub fn reset_axes(&mut self) {
        for axis in &mut self.axes {
            axis.value = axis.default;
        }
    }

    /// Applies a named instance from the font, setting each axis to its coordinate.
    pub fn apply_named_instance(&mut self, index: usize) -> Result<()> {
        let instance = self
            .named_instances
            .get(index)
            .ok_or(FontError::UnknownInstance(index))?
            .coordinates
            .clone();
        self.apply_coordinates(&instance);
        Ok(())
    }

    /// Saves the current axis values under `name`. Returns the index of the new entry.
    pub fn save_current(&mut self, name: impl Into<String>) -> usize {
        self.saved_instances.push(SavedInstance {
            name: name.into(),
            coordinates: self.axes.iter().map(|axis| axis.value).collect(),
        });
        self.saved_instances.len() - 1
    }

    /// Restores a saved instance into the axes.
    pub fn apply_saved(&mut self, index: usize) -> Result<()> {
        let coordinates = self
            .saved_instances
            .get(index)
            .ok_or(FontError::UnknownInstance(index))?
            .coordinates
            .clone();
        self.apply_coordinates(&coordinates);
        Ok(())
    }

    /// The CSS value for `font-variation-settings`, e.g. `"wght" 700, "wdth" 100`.
    /// Returns `normal` for static fonts.
    pub fn variation_settings(&self) -> String {
        if self.axes.is_empty() {
            return "normal".to_string();
        }
        self.axes
            .iter()
            .map(|axis| format!("\"{}\" {}", axis.tag, axis.value))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn apply_coordinates(&mut self, coordinates: &[f32]) {
        for (axis, &coordinate) in self.axes.iter_mut().zip(coordinates) {
            axis.value = coordinate.clamp(axis.min, axis.max);
        }
    }

    fn axis_mut(&mut self, tag: &str) -> Result<&mut AxisValue> {
        self.axes
            .iter_mut()
            .find(|axis| axis.tag == tag)
            .ok_or_else(|| FontError::UnknownAxis(tag.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EmbeddingRights, FontFace, FontFormat, FontSource, VariationAxis};

    fn axis(tag: &str, min: f32, default: f32, max: f32) -> VariationAxis {
        VariationAxis {
            tag: tag.into(),
            name: Some(tag.to_uppercase()),
            min,
            default,
            max,
            hidden: false,
        }
    }

    fn variable_file() -> FontFile {
        FontFile {
            path: PathBuf::from("Var.ttf"),
            source: FontSource::User,
            format: FontFormat::Ttf,
            size: 1,
            modified_ms: 0,
            content_hash: "h".into(),
            faces: vec![FontFace {
                index: 0,
                family: "Var".into(),
                subfamily: "Regular".into(),
                full_name: "Var Regular".into(),
                postscript_name: None,
                version: "1".into(),
                vendor_id: None,
                embedding: EmbeddingRights::Installable,
                units_per_em: 1000,
                is_variable: true,
                axes: vec![
                    axis("wght", 100.0, 400.0, 900.0),
                    axis("wdth", 75.0, 100.0, 125.0),
                ],
                named_instances: vec![NamedInstance {
                    name: Some("Black Condensed".into()),
                    postscript_name: None,
                    coordinates: vec![900.0, 75.0],
                }],
            }],
        }
    }

    fn static_file() -> FontFile {
        let mut file = variable_file();
        file.faces[0].is_variable = false;
        file.faces[0].axes.clear();
        file.faces[0].named_instances.clear();
        file
    }

    #[test]
    fn starts_at_axis_defaults() {
        let state = PreviewState::new(&variable_file(), 0).expect("face exists");
        assert_eq!(state.axes[0].value, 400.0);
        assert_eq!(state.axes[1].value, 100.0);
        assert_eq!(state.size_px, DEFAULT_SIZE_PX);
        assert_eq!(state.background, Background::Light);
    }

    #[test]
    fn set_axis_clamps_to_the_axis_range() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        assert_eq!(state.set_axis("wght", 2000.0).expect("known axis"), 900.0);
        assert_eq!(state.set_axis("wght", -5.0).expect("known axis"), 100.0);
    }

    #[test]
    fn set_axis_rejects_unknown_tag() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        let err = state.set_axis("ital", 1.0).expect_err("no such axis");
        assert!(matches!(err, FontError::UnknownAxis(tag) if tag == "ital"));
    }

    #[test]
    fn css_lists_every_axis_in_order() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        state.set_axis("wght", 700.0).expect("known axis");
        assert_eq!(state.variation_settings(), "\"wght\" 700, \"wdth\" 100");
    }

    #[test]
    fn static_fonts_use_normal_and_have_no_axes() {
        let state = PreviewState::new(&static_file(), 0).expect("face exists");
        assert!(!state.is_variable);
        assert_eq!(state.variation_settings(), "normal");
    }

    #[test]
    fn named_instance_sets_all_axes() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        state.apply_named_instance(0).expect("instance exists");
        assert_eq!(state.variation_settings(), "\"wght\" 900, \"wdth\" 75");
    }

    #[test]
    fn unknown_named_instance_is_an_error() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        assert!(matches!(
            state.apply_named_instance(3),
            Err(FontError::UnknownInstance(3))
        ));
    }

    #[test]
    fn saved_instance_round_trips() {
        let mut state = PreviewState::new(&variable_file(), 0).expect("face exists");
        state.set_axis("wght", 650.0).expect("known axis");
        let saved = state.save_current("Medium-ish");
        state.reset_axes();
        assert_eq!(state.axes[0].value, 400.0);

        state.apply_saved(saved).expect("saved exists");
        assert_eq!(state.axes[0].value, 650.0);
        assert_eq!(state.saved_instances[0].name, "Medium-ish");
    }

    #[test]
    fn missing_face_is_an_error() {
        assert!(matches!(
            PreviewState::new(&variable_file(), 4),
            Err(FontError::NoSuchFace(4))
        ));
    }

    #[test]
    fn waterfall_sizes_are_increasing() {
        assert!(WATERFALL_SIZES.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
