#!/usr/bin/env node
// repomap launcher: runs the native binary for this platform.
"use strict";
const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const path = require("node:path");

const PLATFORMS = {
  "darwin-arm64": "@sylphx/repomap-darwin-arm64",
  "darwin-x64": "@sylphx/repomap-darwin-x64",
  "linux-x64": "@sylphx/repomap-linux-x64-gnu",
  "linux-arm64": "@sylphx/repomap-linux-arm64-gnu",
  "win32-x64": "@sylphx/repomap-win32-x64-msvc",
};

function resolveBinary() {
  if (process.env.REPOMAP_BIN && existsSync(process.env.REPOMAP_BIN)) return process.env.REPOMAP_BIN;
  const exe = process.platform === "win32" ? "repomap.exe" : "repomap";
  const pkg = PLATFORMS[`${process.platform}-${process.arch}`];
  if (pkg) {
    try {
      return require.resolve(`${pkg}/${exe}`);
    } catch {}
  }
  // Development checkout: packages/repomap/bin -> repo root target/.
  const root = path.resolve(__dirname, "..", "..", "..");
  for (const p of [path.join(root, "target", "release", exe), path.join(root, "target", "debug", exe)]) {
    if (existsSync(p)) return p;
  }
  return null;
}

const bin = resolveBinary();
if (!bin) {
  console.error(
    `repomap: no native binary for ${process.platform}-${process.arch}.\n` +
      "Supported: macOS (arm64, x64), Linux glibc (x64, arm64), Windows x64.\n" +
      "If optional dependencies were skipped, reinstall without --no-optional, or build from source: cargo install --git https://github.com/SylphxAI/repomap repomap"
  );
  process.exit(1);
}
const res = spawnSync(bin, process.argv.slice(2), { stdio: "inherit", windowsHide: true });
if (res.error) {
  console.error(`repomap: failed to start ${bin}: ${res.error.message}`);
  process.exit(1);
}
process.exit(res.status === null ? 1 : res.status);
