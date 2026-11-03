// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// The library: every scanned font with its state, search and filter, and the actions that apply
// to the selected font.

import { api, errorMessage, type FontRow, type FontState, type LibraryView } from "./api";
import { button, el, statusLine } from "./dom";

const STATE_LABEL: Record<FontState, string> = {
  installed: "Installed",
  session: "Session",
  available: "Available",
};

export interface LibraryPanel {
  element: HTMLElement;
  refresh: () => Promise<void>;
  /** Installs each path permanently, then refreshes. Used by drag and drop. */
  installPaths: (paths: string[]) => Promise<void>;
  /** Whether the tray answered on the last refresh. */
  trayRunning: () => boolean;
}

export function createLibraryPanel(onPreview: (path: string) => void): LibraryPanel {
  let rows: FontRow[] = [];
  let trayUp = false;
  let selected: FontRow | null = null;

  const status = statusLine();
  const notice = el("div", "notice");
  notice.hidden = true;
  const trayButton = button("Start tray", () => {
    void run(() => api.trayStart());
  });
  notice.append(
    el("span", "", "The tray process is not running, so session activation is unavailable. "),
    trayButton,
  );

  const search = el("input");
  search.type = "search";
  search.placeholder = "Search family, style or path";
  search.setAttribute("aria-label", "Search fonts");

  const stateFilter = el("select");
  stateFilter.setAttribute("aria-label", "Filter by state");
  for (const [value, label] of [
    ["", "All states"],
    ["installed", "Installed"],
    ["session", "Session"],
    ["available", "Available"],
  ] as const) {
    const option = el("option", "", label);
    option.value = value;
    stateFilter.append(option);
  }

  const refreshButton = button("Rescan", () => void refresh());
  const toolbar = el("div", "row");
  toolbar.append(search, stateFilter, refreshButton);

  const table = el("table", "library");
  const head = el("thead");
  const headRow = el("tr");
  for (const label of ["State", "Family", "Style", "Format", "Path"]) {
    headRow.append(el("th", "", label));
  }
  head.append(headRow);
  const body = el("tbody");
  table.append(head, body);

  const count = el("p", "meta");

  const actions = el("div", "row actions");
  actions.hidden = true;
  const detail = el("p", "meta");

  const panel = el("section", "panel");
  panel.append(notice, status.element, toolbar, count, table, detail, actions);

  search.addEventListener("input", renderRows);
  stateFilter.addEventListener("change", renderRows);

  function visibleRows(): FontRow[] {
    const query = search.value.trim().toLowerCase();
    const wanted = stateFilter.value as FontState | "";
    return rows.filter((row) => {
      if (wanted && row.state !== wanted) return false;
      if (!query) return true;
      return [row.family, row.subfamily, row.full_name, row.path]
        .some((text) => text.toLowerCase().includes(query));
    });
  }

  function renderRows(): void {
    const shown = visibleRows();
    body.replaceChildren();
    for (const row of shown) {
      const tr = el("tr", row === selected ? "selected" : "");
      tr.tabIndex = 0;
      tr.addEventListener("click", () => select(row));
      tr.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          select(row);
        }
      });
      tr.append(
        el("td", `state ${row.state}`, STATE_LABEL[row.state]),
        el("td", "", row.family),
        el("td", "", row.subfamily),
        el("td", "", row.faces > 1 ? `${row.format} (${row.faces} faces)` : row.format),
        el("td", "path", row.path),
      );
      body.append(tr);
    }
    count.textContent = `${shown.length} of ${rows.length} font file(s)`;
    renderActions();
  }

  function select(row: FontRow): void {
    selected = row;
    renderRows();
  }

  function renderActions(): void {
    actions.replaceChildren();
    if (!selected) {
      actions.hidden = true;
      detail.textContent = "";
      return;
    }
    const row = selected;
    actions.hidden = false;
    detail.textContent = `${row.full_name || row.family} · ${row.source} · ${row.is_variable ? "variable" : "static"}`;

    if (row.state === "session") {
      actions.append(button("Deactivate for session", () => runOn(row, api.sessionDeactivate)));
    } else {
      const activate = button("Activate for session", () => runOn(row, api.sessionActivate));
      activate.disabled = !trayUp;
      activate.title = trayUp ? "" : "Needs the tray process";
      actions.append(activate);
    }

    if (row.state === "installed") {
      actions.append(button("Uninstall", () => runOn(row, api.fontUninstall)));
    } else {
      actions.append(
        button("Install for this user", () => {
          void installPaths([row.path]);
        }),
      );
    }

    actions.append(button("Preview", () => onPreview(row.path)));
  }

  async function runOn(row: FontRow, action: (path: string) => Promise<unknown>): Promise<void> {
    await run(() => action(row.path));
  }

  /** Runs an action, reports any error, and refreshes the library afterwards. */
  async function run(action: () => Promise<unknown>): Promise<void> {
    status.clear();
    try {
      await action();
    } catch (err) {
      status.show(errorMessage(err));
    }
    await refresh();
  }

  async function refresh(): Promise<void> {
    try {
      const view: LibraryView = await api.libraryList();
      rows = view.rows;
      trayUp = view.tray_running;
      notice.hidden = trayUp;
      if (view.unreadable > 0) {
        status.show(`${view.unreadable} font file(s) could not be read and were skipped.`);
      }
      if (selected) {
        const path = selected.path;
        selected = rows.find((r) => r.path === path) ?? null;
      }
      renderRows();
    } catch (err) {
      status.show(errorMessage(err));
    }
  }

  async function installPaths(paths: string[]): Promise<void> {
    status.clear();
    const failures: string[] = [];
    for (const path of paths) {
      try {
        await api.fontInstall(path);
      } catch (err) {
        failures.push(`${path}: ${errorMessage(err)}`);
      }
    }
    if (failures.length > 0) status.show(failures.join("\n"));
    await refresh();
  }

  return {
    element: panel,
    refresh,
    installPaths,
    trayRunning: () => trayUp,
  };
}
