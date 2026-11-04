// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import "./styles.css";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { createDuplicatesPanel } from "./duplicates";
import { createLibraryPanel } from "./library";
import { createPreviewPanel } from "./preview";
import { createSettingsPanel } from "./settings";
import { createOnboardingPanel } from "./onboarding";
import { api, errorMessage } from "./api";
import { button, el, statusLine } from "./dom";

type Tab = "library" | "preview" | "duplicates" | "settings";

const root = document.querySelector<HTMLDivElement>("#app");

if (root) {
  const preview = createPreviewPanel();
  const sessionSummary = el("span", "meta");
  sessionSummary.setAttribute("aria-live", "polite");
  const library = createLibraryPanel(
    (path) => {
      showTab("preview");
      void preview.open(path);
    },
    (view) => {
      const active = view.rows.filter((row) => row.state === "session").length;
      sessionSummary.textContent = view.tray_running
        ? `${active} font(s) active this session`
        : "Tray not running";
      deactivateAll.disabled = !view.tray_running || active === 0;
    },
  );
  const deactivateAll = button("Deactivate all", () => {
    void api
      .sessionDeactivateAll()
      .catch((err: unknown) => dropStatus.show(errorMessage(err)))
      .finally(() => void library.refresh());
  });
  deactivateAll.disabled = true;
  const duplicates = createDuplicatesPanel();
  const settings = createSettingsPanel(() => void library.refresh());

  const panels: Record<Tab, HTMLElement> = {
    library: library.element,
    preview: preview.element,
    duplicates: duplicates.element,
    settings: settings.element,
  };

  // Preview entry form: open any font file by path, for fonts not in the library.
  const pathInput = el("input");
  pathInput.type = "text";
  pathInput.placeholder = "Path to a .ttf, .otf or .ttc file";
  pathInput.setAttribute("aria-label", "Font file path");
  const faceInput = el("input");
  faceInput.type = "number";
  faceInput.min = "0";
  faceInput.value = "0";
  faceInput.setAttribute("aria-label", "Face index within a collection");
  const openForm = el("form", "open-form");
  const openButton = el("button", "", "Open");
  openButton.type = "submit";
  openForm.append(pathInput, faceInput, openButton);
  openForm.addEventListener("submit", (event) => {
    event.preventDefault();
    showTab("preview");
    void preview.open(pathInput.value.trim(), Number(faceInput.value) || 0);
  });
  const previewEntry = el("section", "panel");
  previewEntry.append(openForm);

  const tabButtons = new Map<Tab, HTMLButtonElement>();
  const tabBar = el("nav", "tabs");
  for (const [tab, label] of [
    ["library", "Library"],
    ["preview", "Preview"],
    ["duplicates", "Duplicates"],
    ["settings", "Settings"],
  ] as const) {
    const item = button(label, () => showTab(tab), "tab");
    tabButtons.set(tab, item);
    tabBar.append(item);
  }

  // Fonts added or removed outside the app, and session changes made from the CLI or the tray,
  // refresh the library automatically.
  void listen("library-changed", () => void library.refresh());
  void listen("session-changed", () => void library.refresh());

  const dropStatus = statusLine();
  const header = el("header");
  const sessionBar = el("div", "session-bar");
  sessionBar.append(sessionSummary, deactivateAll);
  header.append(el("h1", "", "tpt-fonts"), sessionBar, tabBar);

  const main = el("main");
  for (const tab of Object.keys(panels) as Tab[]) {
    main.append(panels[tab]);
  }
  main.insertBefore(previewEntry, panels.preview);

  const onboarding = createOnboardingPanel();
  onboarding.element.hidden = !onboarding.visible;
  main.prepend(onboarding.element);
  root.append(header, dropStatus.element, main, footer());

  /** A quiet footer link. It opens in the browser rather than navigating the app window. */
  function footer(): HTMLElement {
    const link = el("a", "", "TPT Solutions tools");
    link.href = "https://tptsolutions.co.nz/tools";
    link.addEventListener("click", (event) => {
      event.preventDefault();
      api.openToolsPage().catch((err: unknown) => dropStatus.show(errorMessage(err)));
    });
    const element = el("footer", "app-footer");
    element.append(link);
    return element;
  }

  function showTab(tab: Tab): void {
    for (const [name, panel] of Object.entries(panels) as [Tab, HTMLElement][]) {
      panel.hidden = name !== tab;
    }
    previewEntry.hidden = tab !== "preview";
    for (const [name, item] of tabButtons) {
      item.setAttribute("aria-current", name === tab ? "page" : "false");
    }
    if (tab === "library") void library.refresh();
  }

  showTab("library");

  // Dropping font files anywhere on the window installs them for this user.
  void getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type === "enter") {
      document.body.classList.add("dragging");
    } else if (event.payload.type === "leave") {
      document.body.classList.remove("dragging");
    } else if (event.payload.type === "drop") {
      document.body.classList.remove("dragging");
      const fonts = event.payload.paths.filter((p) => /\.(ttf|otf|ttc)$/i.test(p));
      if (fonts.length === 0) {
        dropStatus.show("Only .ttf, .otf and .ttc files can be installed.");
        return;
      }
      dropStatus.clear();
      showTab("library");
      void library.installPaths(fonts);
    }
  });
}
