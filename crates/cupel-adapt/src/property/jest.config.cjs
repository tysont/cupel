// ABOUTME: Written by Cupel's property engine (ADR-095) and passed to jest as `--config`: the configuration of the
// ABOUTME: project jest runs in, with the setup file that sets Cupel's fast-check budget appended to its setup files.
const { existsSync, readFileSync } = require("node:fs");
const { join, resolve } = require("node:path");
const { pathToFileURL } = require("node:url");

const setup = {setup};

async function project(dir) {
  const json = join(dir, "jest.config.json");
  if (existsSync(json)) return JSON.parse(readFileSync(json, "utf8"));
  const found = ["jest.config.js", "jest.config.mjs", "jest.config.cjs"]
    .map((name) => join(dir, name))
    .find((path) => existsSync(path));
  if (found) {
    const loaded = await import(pathToFileURL(found).href);
    const exported = loaded.default ?? loaded;
    return typeof exported === "function" ? await exported() : exported;
  }
  const manifest = join(dir, "package.json");
  return existsSync(manifest) ? (JSON.parse(readFileSync(manifest, "utf8")).jest ?? {}) : {};
}

module.exports = async () => {
  const dir = process.cwd();
  const config = await project(dir);
  return {
    ...config,
    rootDir: resolve(dir, config.rootDir ?? "."),
    setupFilesAfterEnv: [...(config.setupFilesAfterEnv ?? []), setup],
  };
};
