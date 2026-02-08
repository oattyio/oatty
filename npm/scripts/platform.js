"use strict";

const PLATFORM_TARGETS = {
  darwin: {
    x64: { target: "x86_64-apple-darwin", archiveExtension: ".zip", binaryName: "oatty" },
    arm64: { target: "aarch64-apple-darwin", archiveExtension: ".zip", binaryName: "oatty" }
  },
  linux: {
    x64: { target: "x86_64-unknown-linux-gnu", archiveExtension: ".tar.gz", binaryName: "oatty" }
  },
  win32: {
    x64: { target: "x86_64-pc-windows-msvc", archiveExtension: ".zip", binaryName: "oatty.exe" }
  }
};

function resolvePlatformTarget(platform, architecture) {
  const platformEntry = PLATFORM_TARGETS[platform];
  if (!platformEntry) {
    return null;
  }

  return platformEntry[architecture] || null;
}

module.exports = {
  resolvePlatformTarget
};
