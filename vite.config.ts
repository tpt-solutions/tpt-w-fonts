// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

import { defineConfig } from "vite";
import { resolve } from "node:path";

// The frontend lives in src/, while Tauri (src-tauri/tauri.conf.json) loads the build from dist/ at the repo root.
export default defineConfig({
  root: "src",
  publicDir: resolve(__dirname, "public"),
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: resolve(__dirname, "dist"),
    emptyOutDir: true,
    target: "es2022",
  },
});
