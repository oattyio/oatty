#!/usr/bin/env node
"use strict";

const path = require("node:path");
const fs = require("node:fs");
const { spawn } = require("node:child_process");

const expectedBinaryName = process.platform === "win32" ? "oatty.exe" : "oatty";
const primaryBinaryPath = path.join(__dirname, expectedBinaryName);
const nestedBinaryPath = path.join(primaryBinaryPath, expectedBinaryName);

function resolveBinaryPath() {
  if (fs.existsSync(primaryBinaryPath)) {
    const stat = fs.statSync(primaryBinaryPath);
    if (stat.isFile()) {
      return primaryBinaryPath;
    }
  }

  if (fs.existsSync(nestedBinaryPath)) {
    const stat = fs.statSync(nestedBinaryPath);
    if (stat.isFile()) {
      return nestedBinaryPath;
    }
  }

  return primaryBinaryPath;
}

const binaryPath = resolveBinaryPath();

if (process.platform !== "win32" && fs.existsSync(binaryPath)) {
  try {
    fs.chmodSync(binaryPath, 0o755);
  } catch {
    // Best-effort permission fix for already-installed packages.
  }
}

if (!fs.existsSync(binaryPath)) {
  process.stderr.write(
    "[oatty] Binary not found. Reinstall with `npm rebuild oatty` or verify release assets exist for your platform.\n"
  );
  process.exit(1);
}

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
  cwd: process.cwd()
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

child.on("error", (error) => {
  process.stderr.write(`[oatty] Failed to launch binary at ${binaryPath}: ${error.message}\n`);
  process.exit(1);
});
