// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

/** Creates an element with optional class and text. Text is always set via textContent. */
export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/** A button with its type set, so it never submits a surrounding form. */
export function button(label: string, onClick: () => void, className = ""): HTMLButtonElement {
  const node = el("button", className, label);
  node.type = "button";
  node.addEventListener("click", onClick);
  return node;
}

/** A small inline status line that can show an error or hide itself. */
export function statusLine(): { element: HTMLParagraphElement; show: (text: string) => void; clear: () => void } {
  const element = el("p", "error");
  // Assistive technology announces the message when it appears.
  element.setAttribute("role", "alert");
  element.hidden = true;
  return {
    element,
    show(text: string) {
      element.textContent = text;
      element.hidden = false;
    },
    clear() {
      element.textContent = "";
      element.hidden = true;
    },
  };
}
