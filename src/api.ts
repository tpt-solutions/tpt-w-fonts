// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs.
// Field names mirror the serde output of tpt-fonts-core's preview model.

import { invoke } from "@tauri-apps/api/core";

export type Background = "light" | "dark";

export interface AxisValue {
  tag: string;
  name: string | null;
  min: number;
  default: number;
  max: number;
  value: number;
  hidden: boolean;
}

export interface NamedInstance {
  name: string | null;
  postscript_name: string | null;
  coordinates: number[];
}

export interface SavedInstance {
  name: string;
  coordinates: number[];
}

export interface PreviewState {
  font_path: string;
  face_index: number;
  family: string;
  subfamily: string;
  is_variable: boolean;
  axes: AxisValue[];
  named_instances: NamedInstance[];
  saved_instances: SavedInstance[];
  sample_text: string;
  size_px: number;
  background: Background;
}

export const api = {
  openPreview: (path: string, faceIndex: number) =>
    invoke<PreviewState>("open_preview", { path, faceIndex }),

  setAxis: (state: PreviewState, tag: string, value: number) =>
    invoke<PreviewState>("preview_set_axis", { state, tag, value }),

  resetAxes: (state: PreviewState) =>
    invoke<PreviewState>("preview_reset_axes", { state }),

  applyNamed: (state: PreviewState, index: number) =>
    invoke<PreviewState>("preview_apply_named", { state, index }),

  save: (state: PreviewState, name: string) =>
    invoke<PreviewState>("preview_save", { state, name }),

  applySaved: (state: PreviewState, index: number) =>
    invoke<PreviewState>("preview_apply_saved", { state, index }),
};
