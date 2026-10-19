#!/usr/bin/env node
// ABOUTME: The `cupel` command of the npm package (ADR-125): downloads this platform's release binary on first run,
// ABOUTME: checks it against the release's SHA256SUMS, caches it, and runs it with the caller's arguments and stdio.
"use strict";

const { execFileSync } = require("node:child_process");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

// The release this package runs: its own version.
const VERSION = require("../package.json").version;
const RELEASES = "https://github.com/tysont/cupel/releases";

// Each release target's triple and archive format, by `<os>-<arch>` in Rust's spelling, as
// dist/targets.toml lists them.
const TARGETS = {
  "linux-x86_64": ["x86_64-unknown-linux-musl", "tar.gz"],
  "linux-aarch64": ["aarch64-unknown-linux-musl", "tar.gz"],
  "macos-x86_64": ["x86_64-apple-darwin", "tar.gz"],
  "macos-aarch64": ["aarch64-apple-darwin", "tar.gz"],
  "windows-x86_64": ["x86_64-pc-windows-msvc", "zip"],
};

const OS = { linux: "linux", darwin: "macos", win32: "windows" };
const ARCH = { x64: "x86_64", arm64: "aarch64" };

function target() {
  const found = TARGETS[`${OS[process.platform]}-${ARCH[process.arch]}`];
  if (!found) {
    throw new Error(`no release for ${process.platform} ${process.arch}; see ${RELEASES}`);
  }
  return found;
}

// Where downloaded binaries are kept: CUPEL_CACHE_DIR, else the user's cache directory.
function cacheDir() {
  if (process.env.CUPEL_CACHE_DIR) {
    return process.env.CUPEL_CACHE_DIR;
  }
  if (process.platform === "win32") {
    return path.join(process.env.LOCALAPPDATA || os.homedir(), "cupel");
  }
  return path.join(process.env.XDG_CACHE_HOME || path.join(os.homedir(), ".cache"), "cupel");
}

async function download(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`could not download ${url}: HTTP ${response.status}`);
  }
  return Buffer.from(await response.arrayBuffer());
}

// The digest SHA256SUMS gives `archive`, in hex.
function listedDigest(sums, archive) {
  for (const line of sums.split("\n")) {
    const [digest, name] = line.trim().split(/\s+/);
    if (name && name.replace(/^\*/, "") === archive) {
      return digest;
    }
  }
  throw new Error(`SHA256SUMS lists no ${archive}`);
}

// Downloads the release archive for `triple`, checks it, and moves its binary to `binary`.
async function install(binary, triple, format) {
  const base = `${process.env.CUPEL_DOWNLOAD_URL || RELEASES}/download/v${VERSION}`;
  const archive = `cupel-${VERSION}-${triple}.${format}`;
  const listed = listedDigest((await download(`${base}/SHA256SUMS`)).toString("utf8"), archive);
  process.stderr.write(`cupel: downloading ${base}/${archive}\n`);
  const bytes = await download(`${base}/${archive}`);
  const actual = crypto.createHash("sha256").update(bytes).digest("hex");
  if (actual !== listed) {
    throw new Error(
      `checksum mismatch for ${archive}: SHA256SUMS has ${listed}, the download is ${actual}`,
    );
  }
  const staging = fs.mkdtempSync(path.join(path.dirname(binary), ".download-"));
  try {
    fs.writeFileSync(path.join(staging, archive), bytes);
    // tar reads both: bsdtar, which Windows ships, extracts zip archives too.
    execFileSync("tar", ["-xf", archive], { cwd: staging, stdio: "inherit" });
    fs.renameSync(path.join(staging, `cupel-${VERSION}-${triple}`, path.basename(binary)), binary);
  } finally {
    fs.rmSync(staging, { recursive: true, force: true });
  }
}

async function main() {
  const [triple, format] = target();
  const name = process.platform === "win32" ? "cupel.exe" : "cupel";
  const binary = path.join(cacheDir(), VERSION, triple, name);
  if (!fs.existsSync(binary)) {
    fs.mkdirSync(path.dirname(binary), { recursive: true });
    await install(binary, triple, format);
  }
  return binary;
}

// Runs `binary` with this process's arguments and stdio, then exits as it did.
function run(binary) {
  try {
    execFileSync(binary, process.argv.slice(2), { stdio: "inherit" });
  } catch (error) {
    if (error.signal) {
      process.kill(process.pid, error.signal);
    } else if (typeof error.status === "number") {
      process.exitCode = error.status;
    } else {
      throw error;
    }
  }
}

if (require.main === module) {
  main()
    .then(run)
    .catch((error) => {
      // Exit 3, Cupel's environment code: the binary could not run, so nothing was graded.
      process.stderr.write(`cupel: ${error.message}\n`);
      process.exitCode = 3;
    });
}

module.exports = { VERSION, TARGETS };
