"use strict";

const fs = require("node:fs");
const fsp = require("node:fs/promises");
const https = require("node:https");
const path = require("node:path");
const crypto = require("node:crypto");
const os = require("node:os");
const tar = require("tar");
const AdmZip = require("adm-zip");
const { resolvePlatformTarget } = require("./platform");

const PACKAGE_ROOT = path.resolve(__dirname, "..", "..");
const BIN_DIRECTORY = path.join(PACKAGE_ROOT, "npm", "bin");
const OUTPUT_EXECUTABLE_PATH = path.join(BIN_DIRECTORY, process.platform === "win32" ? "oatty.exe" : "oatty");
const OUTPUT_MARKER_PATH = path.join(BIN_DIRECTORY, ".oatty-installed-version");
const RELEASE_REPOSITORY = process.env.OATTY_NPM_RELEASE_REPOSITORY || "oattyio/oatty";
const RELEASE_TAG_PREFIX = process.env.OATTY_NPM_RELEASE_TAG_PREFIX || "v";
const RELEASE_BASE_URL = process.env.OATTY_NPM_RELEASE_BASE_URL || `https://github.com/${RELEASE_REPOSITORY}/releases/download`;

async function main() {
  const packageJsonPath = path.join(PACKAGE_ROOT, "package.json");
  const packageJson = JSON.parse(await fsp.readFile(packageJsonPath, "utf8"));
  const packageVersion = packageJson.version;
  const releaseTag = `${RELEASE_TAG_PREFIX}${packageVersion}`;

  const platformTarget = resolvePlatformTarget(process.platform, process.arch);
  if (!platformTarget) {
    emitWarning(
      `No prebuilt oatty binary is published for platform=${process.platform} arch=${process.arch}.` +
        " Skipping binary download."
    );
    return;
  }

  const assetFileName = `oatty-${releaseTag}-${platformTarget.target}${platformTarget.archiveExtension}`;
  const checksumFileName = "SHA256SUMS";
  const assetUrl = `${RELEASE_BASE_URL}/${releaseTag}/${assetFileName}`;
  const checksumUrl = `${RELEASE_BASE_URL}/${releaseTag}/${checksumFileName}`;

  await fsp.mkdir(BIN_DIRECTORY, { recursive: true });
  if (await isAlreadyInstalled(packageVersion)) {
    return;
  }

  const temporaryDirectory = await fsp.mkdtemp(path.join(os.tmpdir(), "oatty-npm-"));
  const archivePath = path.join(temporaryDirectory, assetFileName);
  const checksumPath = path.join(temporaryDirectory, checksumFileName);
  try {
    await downloadToPath(assetUrl, archivePath);
    await downloadToPath(checksumUrl, checksumPath);
    await verifyArchiveChecksum(archivePath, checksumPath, assetFileName);

    if (platformTarget.archiveExtension === ".tar.gz") {
      await extractTarArchive(archivePath, BIN_DIRECTORY);
    } else {
      extractZipArchive(archivePath, BIN_DIRECTORY);
    }

    const installedBinaryPath = await resolveInstalledBinaryPath(OUTPUT_EXECUTABLE_PATH, platformTarget.binaryName);
    await ensureExecutablePermissions(installedBinaryPath);
    await fsp.writeFile(OUTPUT_MARKER_PATH, packageVersion, "utf8");
  } catch (error) {
    emitWarning(`Failed to install oatty binary from ${assetUrl}. ${error.message}`);
    emitWarning("You can retry later with: npm rebuild oatty");
  } finally {
    await fsp.rm(temporaryDirectory, { recursive: true, force: true });
  }
}

async function isAlreadyInstalled(packageVersion) {
  try {
    const version = (await fsp.readFile(OUTPUT_MARKER_PATH, "utf8")).trim();
    const platformTarget = resolvePlatformTarget(process.platform, process.arch);
    if (!platformTarget) {
      return false;
    }

    const binaryPath = await resolveInstalledBinaryPath(OUTPUT_EXECUTABLE_PATH, platformTarget.binaryName);
    const binaryExists = await fileExists(binaryPath);
    return version === packageVersion && binaryExists;
  } catch {
    return false;
  }
}

async function verifyArchiveChecksum(archivePath, checksumPath, assetFileName) {
  const expectedChecksum = await readExpectedChecksum(checksumPath, assetFileName);
  const actualChecksum = await hashFileSha256(archivePath);
  if (actualChecksum !== expectedChecksum) {
    throw new Error(`Checksum mismatch for ${assetFileName}. expected=${expectedChecksum} actual=${actualChecksum}`);
  }
}

async function readExpectedChecksum(checksumPath, assetFileName) {
  const checksumContent = await fsp.readFile(checksumPath, "utf8");
  const lines = checksumContent.split(/\r?\n/);
  for (const line of lines) {
    if (!line.trim()) {
      continue;
    }
    const match = line.match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (!match) {
      continue;
    }
    const [, checksum, fileName] = match;
    if (fileName.trim() === assetFileName) {
      return checksum.toLowerCase();
    }
  }
  throw new Error(`Checksum for ${assetFileName} not found in SHA256SUMS`);
}

async function hashFileSha256(filePath) {
  const hash = crypto.createHash("sha256");
  await new Promise((resolve, reject) => {
    const stream = fs.createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", resolve);
  });
  return hash.digest("hex");
}

async function downloadToPath(url, destinationPath) {
  await new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      if (response.statusCode && response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        response.resume();
        downloadToPath(response.headers.location, destinationPath).then(resolve).catch(reject);
        return;
      }

      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Request failed for ${url}. status=${response.statusCode}`));
        return;
      }

      const file = fs.createWriteStream(destinationPath);
      response.pipe(file);
      file.on("finish", () => file.close(resolve));
      file.on("error", reject);
    });

    request.on("error", reject);
  });
}

async function extractTarArchive(archivePath, destinationPath) {
  await tar.x({
    cwd: destinationPath,
    file: archivePath
  });
}

function extractZipArchive(archivePath, destinationPath) {
  const zipArchive = new AdmZip(archivePath);
  zipArchive.extractAllTo(destinationPath, true);
}

async function ensureExecutablePermissions(binaryPath) {
  if (process.platform === "win32") {
    return;
  }

  if (!(await fileExists(binaryPath))) {
    throw new Error(`Expected binary not found after extraction: ${binaryPath}`);
  }

  const fileStat = await fsp.stat(binaryPath);
  if (!fileStat.isFile()) {
    throw new Error(`Expected installed binary at ${binaryPath}, but found a non-file entry.`);
  }

  await fsp.chmod(binaryPath, 0o755);
}

async function resolveInstalledBinaryPath(primaryPath, expectedBinaryName) {
  if (await isFile(primaryPath)) {
    return primaryPath;
  }

  if (await isDirectory(primaryPath)) {
    const nestedPath = path.join(primaryPath, expectedBinaryName);
    if (await isFile(nestedPath)) {
      return nestedPath;
    }
  }

  return primaryPath;
}

async function fileExists(filePath) {
  try {
    await fsp.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function isFile(filePath) {
  try {
    const stat = await fsp.stat(filePath);
    return stat.isFile();
  } catch {
    return false;
  }
}

async function isDirectory(filePath) {
  try {
    const stat = await fsp.stat(filePath);
    return stat.isDirectory();
  } catch {
    return false;
  }
}

function emitWarning(message) {
  process.stderr.write(`[oatty npm installer] ${message}\n`);
}

main().catch((error) => {
  emitWarning(error.message);
});
