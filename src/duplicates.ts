// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// The duplicates view. Identical copies can be resolved: pick the file to keep, review the plan
// (a dry run), then confirm. Files go to the Recycle Bin or trash, never permanent deletion.
// Same-metadata groups are report-only, because their members can differ in content.

import { api, errorMessage, type DuplicateGroup, type Resolution } from "./api";
import { button, el, statusLine } from "./dom";

const KIND_LABEL: Record<DuplicateGroup["kind"], string> = {
  ExactContent: "Identical copies",
  SameMetadata: "Same family, style, version and vendor, but different files (report only)",
};

export function createDuplicatesPanel(): { element: HTMLElement } {
  const status = statusLine();
  const results = el("div", "duplicates");
  const summary = el("p", "meta", "Scan the library for duplicate fonts.");

  async function scan(): Promise<void> {
    status.clear();
    results.replaceChildren();
    summary.textContent = "Scanning…";
    try {
      const groups = await api.duplicateGroups();
      summary.textContent =
        groups.length === 0 ? "No duplicates found." : `${groups.length} group(s) found.`;
      for (const group of groups) {
        results.append(renderGroup(group));
      }
    } catch (err) {
      summary.textContent = "";
      status.show(errorMessage(err));
    }
  }

  function renderGroup(group: DuplicateGroup): HTMLElement {
    const section = el("section", "group");
    section.append(el("h3", "", KIND_LABEL[group.kind]), el("p", "meta", group.key));
    const plan = el("div", "plan");
    plan.hidden = true;
    const list = el("ul");
    for (const member of group.members) {
      const item = el("li");
      item.append(el("span", "path", member));
      if (group.kind === "ExactContent") {
        item.append(
          " ",
          button("Keep this one", () => void planKeep(group, member, plan), "small"),
        );
      }
      list.append(item);
    }
    section.append(list, plan);
    return section;
  }

  /** Shows what would be trashed if `keep` were kept, and asks for confirmation. */
  async function planKeep(group: DuplicateGroup, keep: string, box: HTMLElement): Promise<void> {
    status.clear();
    try {
      const plan = await api.duplicateResolve(group.key, keep, false);
      showPlan(box, plan, group, keep);
    } catch (err) {
      status.show(errorMessage(err));
    }
  }

  function showPlan(box: HTMLElement, plan: Resolution, group: DuplicateGroup, keep: string): void {
    box.replaceChildren();
    box.hidden = false;
    if (plan.trashed.length === 0) {
      box.append(el("p", "meta", "Nothing to trash for this choice."));
      return;
    }
    box.append(el("p", "", `Keeping ${keep}. These will be sent to the trash:`));
    const list = el("ul");
    for (const path of plan.trashed) list.append(el("li", "path", path));
    box.append(list);
    if (plan.skipped_system.length > 0) {
      box.append(el("p", "meta", `${plan.skipped_system.length} system font(s) are read-only and will be left alone.`));
    }
    const confirm = button("Trash the other copies", () => void apply(group, keep), "danger");
    box.append(confirm, " ", button("Cancel", () => {
      box.hidden = true;
      box.replaceChildren();
    }));
  }

  async function apply(group: DuplicateGroup, keep: string): Promise<void> {
    status.clear();
    try {
      await api.duplicateResolve(group.key, keep, true);
      await scan();
    } catch (err) {
      status.show(errorMessage(err));
    }
  }

  const panel = el("section", "panel");
  panel.append(button("Scan for duplicates", () => void scan()), summary, status.element, results);
  return { element: panel };
}
