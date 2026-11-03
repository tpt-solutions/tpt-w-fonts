// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// The duplicates view. It reports groups found by the core; resolving them is not exposed to the
// GUI yet, so this view is read-only.

import { api, errorMessage, type DuplicateGroup } from "./api";
import { button, el, statusLine } from "./dom";

const KIND_LABEL: Record<DuplicateGroup["kind"], string> = {
  ExactContent: "Identical copies",
  SameMetadata: "Same family, style, version and vendor, but different files",
};

export function createDuplicatesPanel(): { element: HTMLElement } {
  const status = statusLine();
  const results = el("div", "duplicates");
  const summary = el("p", "meta", "Scan the library for duplicate fonts.");

  async function scan(): Promise<void> {
    status.clear();
    summary.textContent = "Scanning…";
    try {
      const groups = await api.duplicateGroups();
      results.replaceChildren();
      summary.textContent =
        groups.length === 0 ? "No duplicates found." : `${groups.length} group(s) found.`;
      for (const group of groups) {
        const section = el("section", "group");
        section.append(el("h3", "", KIND_LABEL[group.kind]), el("p", "meta", group.key));
        const list = el("ul");
        for (const member of group.members) {
          list.append(el("li", "path", member));
        }
        section.append(list);
        results.append(section);
      }
    } catch (err) {
      summary.textContent = "";
      status.show(errorMessage(err));
    }
  }

  const panel = el("section", "panel");
  panel.append(button("Scan for duplicates", () => void scan()), summary, status.element, results);
  return { element: panel };
}
