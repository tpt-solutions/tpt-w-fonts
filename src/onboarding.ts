// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// First-run explanation of the two ways a font can be used: session activation (temporary, needs
// the tray) and permanent install (stays until removed). Dismissal is remembered per viewer; if
// storage is blocked the notice simply shows again next time.

import { button, el } from "./dom";

const SEEN_KEY = "tpt-fonts.onboarded";

function alreadySeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "1";
  } catch {
    return false;
  }
}

function markSeen(): void {
  try {
    localStorage.setItem(SEEN_KEY, "1");
  } catch {
    // Not persisted; the notice will return on the next launch.
  }
}

export function createOnboardingPanel(): { element: HTMLElement; visible: boolean } {
  const panel = el("section", "panel onboarding");
  panel.append(
    el("h2", "", "Two ways to use a font"),
    el(
      "p",
      "",
      "Session: the font is available to every application until you log out or quit the tray. Nothing is copied or registered permanently, and it needs the tray process running.",
    ),
    el(
      "p",
      "",
      "Install: the font is copied into your user font folder and stays installed until you uninstall it. It does not need the tray.",
    ),
    el("p", "meta", "System fonts are read-only in this version; you can preview and activate them but not change them."),
  );
  const dismiss = button("Got it", () => {
    markSeen();
    panel.hidden = true;
  });
  panel.append(dismiss);
  return { element: panel, visible: !alreadySeen() };
}
