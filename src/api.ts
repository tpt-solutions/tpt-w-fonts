// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Typed wrappers over the Tauri commands in src-tauri/src/lib.rs.
// Field names mirror the serde output of tpt-fonts-core.

import { invoke } from "@tauri-apps/api/core";

export type Background = "light" | "dark";
export type FontState = "installed" | "session" | "available";
export type FontSource = "User" | "System";
export type FontFormat = "Ttf" | "Otf" | "Ttc";
export type DuplicateKind = "ExactContent" | "SameMetadata";
export type EmbeddingRights =
  | "Installable"
  | "Restricted"
  | "PreviewAndPrint"
  | "Editable"
  | "Unspecified";

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

export interface FontRow {
  path: string;
  state: FontState;
  source: FontSource;
  format: FontFormat;
  family: string;
  subfamily: string;
  full_name: string;
  is_variable: boolean;
  faces: number;
  version: string;
  vendor_id: string | null;
  embedding: EmbeddingRights;
  units_per_em: number;
}

export interface LibraryView {
  rows: FontRow[];
  tray_running: boolean;
  unreadable: number;
}

export interface DuplicateGroup {
  kind: DuplicateKind;
  key: string;
  members: string[];
}

export interface Resolution {
  trashed: string[];
  skipped_system: string[];
}

export interface ScanDirsView {
  /** Folders the OS provides. They are fixed and cannot be removed. */
  defaults: string[];
  extra: { path: string; exists: boolean }[];
}

export const api = {
  libraryList: () => invoke<LibraryView>("library_list"),
  sessionActivate: (path: string) => invoke<void>("session_activate", { path }),
  sessionDeactivate: (path: string) => invoke<void>("session_deactivate", { path }),
  fontInstall: (path: string) => invoke<string>("font_install", { path }),
  fontUninstall: (path: string) => invoke<void>("font_uninstall", { path }),
  duplicateGroups: () => invoke<DuplicateGroup[]>("duplicate_groups"),
  duplicateResolve: (key: string, keep: string, apply: boolean) =>
    invoke<Resolution>("duplicate_resolve", { key, keep, apply }),
  sessionDeactivateAll: () => invoke<void>("session_deactivate_all"),
  autostartGet: () => invoke<boolean>("autostart_get"),
  autostartSet: (enabled: boolean) => invoke<void>("autostart_set", { enabled }),
  trayStart: () => invoke<void>("tray_start"),
  openToolsPage: () => invoke<void>("open_tools_page"),
  scanDirsGet: () => invoke<ScanDirsView>("scan_dirs_get"),
  scanDirsAdd: (path: string) => invoke<void>("scan_dirs_add", { path }),
  scanDirsRemove: (path: string) => invoke<void>("scan_dirs_remove", { path }),

  openPreview: (path: string, faceIndex: number) =>
    invoke<PreviewState>("open_preview", { path, faceIndex }),
  setAxis: (state: PreviewState, tag: string, value: number) =>
    invoke<PreviewState>("preview_set_axis", { state, tag, value }),
  resetAxes: (state: PreviewState) => invoke<PreviewState>("preview_reset_axes", { state }),
  applyNamed: (state: PreviewState, index: number) =>
    invoke<PreviewState>("preview_apply_named", { state, index }),
  save: (state: PreviewState, name: string) => invoke<PreviewState>("preview_save", { state, name }),
  applySaved: (state: PreviewState, index: number) =>
    invoke<PreviewState>("preview_apply_saved", { state, index }),
};

/** Turns whatever a rejected invoke produced into a readable line. */
export function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}
