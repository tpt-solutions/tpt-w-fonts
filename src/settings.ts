// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Settings: start at login, and the colour theme. The theme is a per-viewer preference, so it is
// kept in localStorage; if storage is unavailable the page simply follows the system theme.

import { api, errorMessage } from "./api";
import { el, statusLine } from "./dom";

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

export function createSettingsPanel(): { element: HTMLElement } {
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
  panel.append(autostartLabel, themeLabel, status.element);

  api
    .autostartGet()
    .then((enabled) => {
      autostart.checked = enabled;
    })
    .catch((err: unknown) => status.show(errorMessage(err)));

  return { element: panel };
}
