#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");
const platforms = require("../platforms.json");
const rootManifest = require("../../package.json");
const reinstall = "Reinstall with `npm install --include=optional oatty` (or add `--global` for a global installation).";

/** Select a supported native target; Linux requires the glibc build. */
function selectPlatform() {
  const platform = platforms[`${process.platform}-${process.arch}`];
  if (!platform) {
    throw new Error(`Unsupported platform: ${process.platform}-${process.arch}. Use a supported native release or build from source: https://github.com/oattyio/oatty.`);
  }
  if (platform.libc === "glibc") {
    const report = process.report && process.report.getReport();
    if (!report || !report.header.glibcVersionRuntime) {
      throw new Error("Unsupported Linux libc: this release requires glibc; musl is not supported. Use a supported glibc system or build from source: https://github.com/oattyio/oatty.");
    }
  }
  return platform;
}

/** Resolve an exact-version optional package without modifying the installation. */
function resolveBinary(platform) {
  let manifestPath;
  try {
    manifestPath = require.resolve(`${platform.package}/package.json`);
  } catch (error) {
    if (error.code === "MODULE_NOT_FOUND") {
      throw new Error(`Missing optional binary package ${platform.package}@${rootManifest.version}.`);
    }
    throw error;
  }
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const expected = {
    name: platform.package,
    version: rootManifest.version,
    os: [platform.os],
    cpu: [platform.cpu],
    oattyTarget: platform.target
  };
  if (platform.libc) expected.libc = [platform.libc];
  for (const [name, value] of Object.entries(expected)) {
    if (JSON.stringify(manifest[name]) !== JSON.stringify(value)) {
      throw new Error(`Invalid ${platform.package} metadata: ${name} must be ${JSON.stringify(value)}.`);
    }
  }
  if (!platform.libc && manifest.libc !== undefined) {
    throw new Error(`Invalid ${platform.package} metadata: unexpected libc restriction.`);
  }
  const binary = require.resolve(`${platform.package}/bin/${platform.binary}`);
  if (path.dirname(binary) !== path.join(path.dirname(manifestPath), "bin") || !fs.statSync(binary).isFile()) {
    throw new Error(`Invalid executable path for ${platform.package}.`);
  }
  const marker = path.join(path.dirname(binary), ".oatty-installed-version");
  if (fs.readFileSync(marker, "utf8").trim() !== rootManifest.version) {
    throw new Error(`Invalid installed-version marker for ${platform.package}.`);
  }
  return binary;
}

/** Preserve terminal I/O and process status, forwarding parent-directed signals. */
function launch(binary) {
  const child = spawn(binary, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
    cwd: process.cwd()
  });
  const handlers = new Map();
  for (const signal of ["SIGINT", "SIGTERM"]) {
    const handler = () => child.kill(signal);
    handlers.set(signal, handler);
    process.on(signal, handler);
  }
  function removeHandlers() {
    for (const [signal, handler] of handlers) process.removeListener(signal, handler);
  }
  child.on("error", (error) => {
    removeHandlers();
    process.stderr.write(`[oatty] Failed to launch executable: ${error.message}. ${reinstall}\n`);
    process.exitCode = 1;
  });
  child.on("exit", (code, signal) => {
    removeHandlers();
    if (signal) {
      process.kill(process.pid, signal);
    } else {
      process.exitCode = code === null ? 1 : code;
    }
  });
}

try {
  launch(resolveBinary(selectPlatform()));
} catch (error) {
  const guidance = error.message.startsWith("Unsupported") ? "" : ` ${reinstall}`;
  process.stderr.write(`[oatty] ${error.message}${guidance}\n`);
  process.exitCode = 1;
}
