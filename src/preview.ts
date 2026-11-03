// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// The variable-font preview panel. The core owns axis clamping and CSS formatting; this module
// only renders the state it returns and forwards user input.

import { convertFileSrc } from "@tauri-apps/api/core";
import { api, errorMessage, type PreviewState } from "./api";
import { button, el, statusLine } from "./dom";

// Must match WATERFALL_SIZES in core/src/preview.rs.
const WATERFALL_SIZES = [12, 16, 24, 36, 48, 72];
const FONT_FAMILY = "tpt-preview";

export interface PreviewPanel {
  element: HTMLElement;
  open: (path: string, faceIndex?: number) => Promise<void>;
}

/** Formats an axis value: whole numbers as-is, fractional ones (such as slnt) to two decimals. */
function formatAxisValue(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

export function createPreviewPanel(): PreviewPanel {
  let state: PreviewState | null = null;
  let loadedFace: FontFace | null = null;
  // Only the newest request may update the state, so slow replies cannot overwrite fast ones.
  let latestRequest = 0;

  const status = statusLine();
  const panel = el("section", "panel");
  panel.hidden = true;

  const fontTitle = el("h2");
  const fontMeta = el("p", "meta");

  const sampleInput = el("input");
  sampleInput.type = "text";
  sampleInput.setAttribute("aria-label", "Sample text");
  sampleInput.addEventListener("input", () => {
    if (state) state.sample_text = sampleInput.value;
    render();
  });

  const sizeInput = el("input");
  sizeInput.type = "range";
  sizeInput.min = "8";
  sizeInput.max = "200";
  sizeInput.step = "1";
  sizeInput.setAttribute("aria-label", "Preview size in pixels");
  sizeInput.addEventListener("input", () => {
    if (state) state.size_px = Number(sizeInput.value);
    render();
  });
  const sizeLabel = el("span", "value");
  const sizeControl = el("label", "size-control", "Size ");
  sizeControl.append(sizeInput, sizeLabel);

  const backgroundSelect = el("select");
  backgroundSelect.setAttribute("aria-label", "Preview background");
  for (const [value, label] of [
    ["light", "Light background"],
    ["dark", "Dark background"],
  ] as const) {
    const option = el("option", "", label);
    option.value = value;
    backgroundSelect.append(option);
  }
  backgroundSelect.addEventListener("change", () => {
    if (state) state.background = backgroundSelect.value === "dark" ? "dark" : "light";
    render();
  });

  const specimen = el("div", "specimen");
  const waterfall = el("div", "waterfall");
  const axesSection = el("div", "axes");

  const instanceSelect = el("select");
  instanceSelect.setAttribute("aria-label", "Named instances");
  const applyInstanceButton = button("Apply instance", () => {
    const index = Number(instanceSelect.value);
    if (Number.isNaN(index)) return;
    void update((current) => api.applyNamed(current, index));
  });
  const resetButton = button("Reset axes", () => {
    void update((current) => api.resetAxes(current));
  });
  const instanceRow = el("div", "row");
  instanceRow.append(instanceSelect, applyInstanceButton, resetButton);

  const saveName = el("input");
  saveName.type = "text";
  saveName.placeholder = "Name this combination";
  saveName.setAttribute("aria-label", "Saved instance name");
  const saveButton = button("Save current", () => {
    const name = saveName.value.trim();
    if (!name) return;
    saveName.value = "";
    void update((current) => api.save(current, name));
  });
  const saveRow = el("div", "row");
  saveRow.append(saveName, saveButton);
  const savedList = el("ul", "saved");

  const controlsRow = el("div", "row");
  controlsRow.append(sampleInput, sizeControl, backgroundSelect);

  panel.append(
    fontTitle,
    fontMeta,
    status.element,
    controlsRow,
    specimen,
    axesSection,
    instanceRow,
    saveRow,
    savedList,
    waterfall,
  );

  interface AxisControl {
    tag: string;
    slider: HTMLInputElement;
    readout: HTMLSpanElement;
  }
  let axisControls: AxisControl[] = [];

  /** Applies a core command and keeps the frontend-only fields the user has just typed. */
  async function update(action: (current: PreviewState) => Promise<PreviewState>): Promise<void> {
    if (!state) return;
    const seq = ++latestRequest;
    try {
      const next = await action(state);
      if (seq !== latestRequest || !state) return;
      state = {
        ...next,
        sample_text: state.sample_text,
        size_px: state.size_px,
        background: state.background,
      };
      render();
    } catch (err) {
      status.show(errorMessage(err));
    }
  }

  /** Registers the font file under FONT_FAMILY, replacing any previous one. */
  async function loadFontFace(path: string): Promise<void> {
    const face = new FontFace(FONT_FAMILY, `url("${convertFileSrc(path)}")`);
    await face.load();
    if (loadedFace) document.fonts.delete(loadedFace);
    document.fonts.add(face);
    loadedFace = face;
  }

  /** Builds sliders once per opened font. Rebuilding on every change would drop focus mid-drag. */
  function buildAxisControls(): void {
    axesSection.replaceChildren();
    axisControls = [];
    if (!state) return;

    for (const axis of state.axes) {
      const slider = el("input");
      slider.type = "range";
      slider.min = String(axis.min);
      slider.max = String(axis.max);
      slider.step = String((axis.max - axis.min) / 200);
      slider.setAttribute("aria-label", axis.name ?? axis.tag);
      const readout = el("span", "value");
      const label = el("label", "axis", `${axis.name ?? axis.tag} (${axis.tag}) `);
      label.append(slider, readout);
      axesSection.append(label);

      slider.addEventListener("input", () => {
        const value = Number(slider.value);
        readout.textContent = formatAxisValue(value);
        void update((current) => api.setAxis(current, axis.tag, value));
      });
      axisControls.push({ tag: axis.tag, slider, readout });
    }
    if (state.axes.length === 0) {
      axesSection.append(el("p", "meta", "This is a static font, so it has no axes to adjust."));
    }
  }

  function render(): void {
    if (!state) return;
    const current = state;
    const variation =
      current.axes.length > 0
        ? current.axes.map((a) => `"${a.tag}" ${a.value}`).join(", ")
        : "normal";

    fontTitle.textContent = `${current.family} ${current.subfamily}`.trim();
    fontMeta.textContent = `${current.font_path}${current.is_variable ? " · variable" : " · static"}`;

    if (sampleInput.value !== current.sample_text) sampleInput.value = current.sample_text;
    sizeInput.value = String(current.size_px);
    sizeLabel.textContent = `${current.size_px} px`;
    backgroundSelect.value = current.background;
    specimen.className = `specimen ${current.background}`;
    specimen.style.fontFamily = `"${FONT_FAMILY}", sans-serif`;
    specimen.style.fontVariationSettings = variation;
    specimen.style.fontSize = `${current.size_px}px`;
    specimen.textContent = current.sample_text;

    for (const control of axisControls) {
      const axis = current.axes.find((a) => a.tag === control.tag);
      if (!axis) continue;
      control.slider.value = String(axis.value);
      control.readout.textContent = formatAxisValue(axis.value);
    }

    instanceSelect.replaceChildren();
    current.named_instances.forEach((instance, index) => {
      const option = el(
        "option",
        "",
        instance.name ?? instance.postscript_name ?? `Instance ${index + 1}`,
      );
      option.value = String(index);
      instanceSelect.append(option);
    });
    instanceSelect.disabled = current.named_instances.length === 0;
    applyInstanceButton.disabled = instanceSelect.disabled;

    savedList.replaceChildren();
    current.saved_instances.forEach((saved, index) => {
      const item = el("li");
      item.append(
        button(saved.name, () => {
          void update((latest) => api.applySaved(latest, index));
        }),
      );
      savedList.append(item);
    });

    waterfall.replaceChildren();
    for (const size of WATERFALL_SIZES) {
      const row = el("p", `waterfall-row ${current.background}`, current.sample_text);
      row.style.fontFamily = `"${FONT_FAMILY}", sans-serif`;
      row.style.fontVariationSettings = variation;
      row.style.fontSize = `${size}px`;
      waterfall.append(row);
    }
  }

  async function open(path: string, faceIndex = 0): Promise<void> {
    status.clear();
    try {
      const next = await api.openPreview(path, faceIndex);
      await loadFontFace(next.font_path);
      state = next;
      buildAxisControls();
      render();
      panel.hidden = false;
    } catch (err) {
      status.show(errorMessage(err));
    }
  }

  return { element: panel, open };
}
