// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Renders sample text in each font's own typeface. A font file is loaded when its element scrolls
// near the viewport. At most MAX_LOADED fonts stay registered; the least recently used are dropped
// and reload if they come back into view. Files that fail to load fall back to the app font, so
// one bad file never breaks the list.

import { convertFileSrc } from "@tauri-apps/api/core";

/** Upper bound on font files registered with the page at once. */
export const MAX_LOADED = 300;

interface Loaded {
  family: string | null;
  face: FontFace | null;
}

// A Map keeps insertion order, so re-inserting a path on use makes the first key the least recent.
const loaded = new Map<string, Promise<Loaded>>();
let counter = 0;

async function load(path: string): Promise<Loaded> {
  try {
    const family = `tpt-sample-${++counter}`;
    const face = new FontFace(family, `url("${convertFileSrc(path)}")`);
    await face.load();
    document.fonts.add(face);
    return { family, face };
  } catch {
    return { family: null, face: null };
  }
}

function evictIfFull(): void {
  while (loaded.size > MAX_LOADED) {
    const oldest = loaded.keys().next().value;
    if (oldest === undefined) return;
    const entry = loaded.get(oldest);
    loaded.delete(oldest);
    void entry?.then(({ face }) => {
      if (face) document.fonts.delete(face);
    });
  }
}

function familyFor(path: string): Promise<Loaded> {
  let pending = loaded.get(path);
  if (pending) {
    // Mark as most recently used.
    loaded.delete(path);
  } else {
    pending = load(path);
  }
  loaded.set(path, pending);
  evictIfFull();
  return pending;
}

function createObserver(): IntersectionObserver {
  return new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const target = entry.target as HTMLElement;
        const path = target.dataset.path;
        if (!path) continue;
        void familyFor(path).then(({ family }) => {
          if (family) {
            target.style.fontFamily = `"${family}", sans-serif`;
            target.classList.remove("unavailable");
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

/**
 * Starts rendering `element` in the typeface of the font at `path`. It is watched for as long as it
 * stays on the page, so a font that was dropped from the cache reloads when scrolled back to.
 */
export function previewIn(element: HTMLElement, path: string): void {
  element.dataset.path = path;
  observer.observe(element);
}
