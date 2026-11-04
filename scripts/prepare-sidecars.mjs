// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

// Builds the tray and CLI in release mode and copies them into src-tauri/binaries under the
// target-triple names Tauri expects for sidecars. Run as part of `beforeBuildCommand`.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";

const hostLine = execFileSync("rustc", ["-vV"]).toString().split("\n").find((l) => l.startsWith("host: "));
const triple = hostLine.slice("host: ".length).trim();
const ext = process.platform === "win32" ? ".exe" : "";

execFileSync("cargo", ["build", "--release", "-p", "tpt-fonts-tray", "-p", "tpt-fonts-cli"], {
  stdio: "inherit",
});

mkdirSync("src-tauri/binaries", { recursive: true });
for (const name of ["tpt-fonts-tray", "tpt-fonts"]) {
  copyFileSync(`target/release/${name}${ext}`, `src-tauri/binaries/${name}-${triple}${ext}`);
  console.log(`sidecar: ${name}-${triple}${ext}`);
}
