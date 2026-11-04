// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Settings: start at login, and the colour theme. The theme is a per-viewer preference, so it is
// kept in localStorage; if storage is unavailable the page simply follows the system theme.

import { api, errorMessage, type ScanDirsView } from "./api";
import { button, el, statusLine } from "./dom";

const THEME_KEY = "tpt-fonts.theme";
type Theme = "system" | "light" | "dark";

function readTheme(): Theme {
  try {
    const saved = localStorage.getItem(THEME_KEY);
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // Storage can be blocked; fall through to the system theme.
  }
  return "system";
}

function applyTheme(theme: Theme): void {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

/** Trims a pasted path and drops the quotes Explorer adds when copying a folder's path. */
function cleanPath(value: string): string {
  return value.trim().replace(/^"(.*)"$/, "$1");
}

/**
 * Lists the extra scan folders and lets the user add or remove them. The OS folders are shown
 * for reference only. `onChanged` runs after a change so the library can be refreshed.
 */
function createScanFoldersSection(onChanged: () => void): HTMLElement {
  const status = statusLine();
  const section = el("div", "scan-folders");
  const heading = el("h3", "", "Scan folders");
  const intro = el(
    "p",
    "meta",
    "Fonts in these folders appear in the library, alongside the system folders.",
  );
  const systemNote = el("p", "meta");
  const list = el("ul", "scan-dirs");

  const input = el("input");
  input.type = "text";
  input.placeholder = "Full path to a folder";
  input.setAttribute("aria-label", "Folder to add");
  const addRow = el("div", "setting");

  function render(view: ScanDirsView): void {
    systemNote.textContent = `System folders: ${view.defaults.join(", ") || "none found"}`;
    list.replaceChildren();
    if (view.extra.length === 0) {
      list.append(el("li", "meta", "No extra folders added."));
      return;
    }
    for (const dir of view.extra) {
      const item = el("li");
      const label = el("span", dir.exists ? "" : "error", dir.exists ? dir.path : `${dir.path} (missing)`);
      const remove = button("Remove", () => removeFolder(dir.path));
      remove.setAttribute("aria-label", `Remove ${dir.path}`);
      item.append(label, " ", remove);
      list.append(item);
    }
  }

  function reload(): void {
    api
      .scanDirsGet()
      .then(render)
      .catch((err: unknown) => status.show(errorMessage(err)));
  }

  function addFolder(): void {
    const path = cleanPath(input.value);
    if (!path) return;
    status.clear();
    api
      .scanDirsAdd(path)
      .then(() => {
        input.value = "";
        reload();
        onChanged();
      })
      .catch((err: unknown) => status.show(errorMessage(err)));
  }

  function removeFolder(path: string): void {
    status.clear();
    api
      .scanDirsRemove(path)
      .then(() => {
        reload();
        onChanged();
      })
      .catch((err: unknown) => status.show(errorMessage(err)));
  }

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") addFolder();
  });
  addRow.append(input, button("Add folder", addFolder));

  section.append(heading, intro, list, addRow, systemNote, status.element);
  reload();
  return section;
}

export function createSettingsPanel(onFoldersChanged: () => void): { element: HTMLElement } {
  const status = statusLine();

  const autostart = el("input");
  autostart.type = "checkbox";
  autostart.id = "autostart";
  const autostartLabel = el("label", "setting");
  autostartLabel.append(autostart, " Start the tray process at login");

  autostart.addEventListener("change", () => {
    const enabled = autostart.checked;
    status.clear();
    api.autostartSet(enabled).catch((err: unknown) => {
      autostart.checked = !enabled;
      status.show(errorMessage(err));
    });
  });

  const themeSelect = el("select");
  themeSelect.setAttribute("aria-label", "Colour theme");
  for (const [value, label] of [
    ["system", "Follow the system"],
    ["light", "Light"],
    ["dark", "Dark"],
  ] as const) {
    const option = el("option", "", label);
    option.value = value;
    themeSelect.append(option);
  }
  themeSelect.value = readTheme();
  applyTheme(readTheme());
  themeSelect.addEventListener("change", () => {
    const theme = themeSelect.value as Theme;
    applyTheme(theme);
    try {
      if (theme === "system") localStorage.removeItem(THEME_KEY);
      else localStorage.setItem(THEME_KEY, theme);
    } catch {
      // Not persisted; the choice still applies for this session.
    }
  });
  const themeLabel = el("label", "setting");
  themeLabel.append("Theme ", themeSelect);

  const panel = el("section", "panel");
  panel.append(autostartLabel, themeLabel, createScanFoldersSection(onFoldersChanged), status.element);

  api
    .autostartGet()
    .then((enabled) => {
      autostart.checked = enabled;
    })
    .catch((err: unknown) => status.show(errorMessage(err)));

  return { element: panel };
}
