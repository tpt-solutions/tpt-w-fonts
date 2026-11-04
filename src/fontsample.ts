// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Renders sample text in each font's own typeface. A font file is loaded the first time its
// element scrolls near the viewport, then kept for the session. Files that fail to load fall back
// to the app font, so one bad file never breaks the list.

import { convertFileSrc } from "@tauri-apps/api/core";

interface Loaded {
  family: string | null;
}

const loaded = new Map<string, Promise<Loaded>>();
let counter = 0;

async function load(path: string): Promise<Loaded> {
  try {
    const family = `tpt-sample-${++counter}`;
    const face = new FontFace(family, `url("${convertFileSrc(path)}")`);
    await face.load();
    document.fonts.add(face);
    return { family };
  } catch {
    return { family: null };
  }
}

function familyFor(path: string): Promise<Loaded> {
  let pending = loaded.get(path);
  if (!pending) {
    pending = load(path);
    loaded.set(path, pending);
  }
  return pending;
}

function createObserver(): IntersectionObserver {
  return new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const target = entry.target as HTMLElement;
        observer.unobserve(target);
        const path = target.dataset.path;
        if (!path) continue;
        void familyFor(path).then(({ family }) => {
          if (family) {
            target.style.fontFamily = `"${family}", sans-serif`;
          } else {
            target.classList.add("unavailable");
          }
        });
      }
    },
    { rootMargin: "200px" },
  );
}

let observer = createObserver();

/**
 * Forgets every element waiting to be previewed. Call before rebuilding rows, so elements that
 * were removed from the page are not kept alive by the observer.
 */
export function resetPreviews(): void {
  observer.disconnect();
  observer = createObserver();
}

/** Starts rendering `element` in the typeface of the font at `path`, once it is near the viewport. */
export function previewIn(element: HTMLElement, path: string): void {
  element.dataset.path = path;
  observer.observe(element);
}
