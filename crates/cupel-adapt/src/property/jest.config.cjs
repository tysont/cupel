// ABOUTME: Written by Cupel's property engine (ADR-095) and passed to jest as `--config`: the configuration of the
// ABOUTME: project jest runs in, with the setup file that sets Cupel's fast-check budget appended to its setup files.
const { existsSync, readFileSync } = require("node:fs");
const { join, resolve } = require("node:path");
const { pathToFileURL } = require("node:url");

const setup = {setup};

module.exports = async () => {
  const dir = process.cwd();
  const json = join(dir, "jest.config.json");
  const found = ["jest.config.js", "jest.config.mjs", "jest.config.cjs"]
    .map((name) => join(dir, name))
    .find((path) => existsSync(path));
  const manifest = join(dir, "package.json");
  let config = {};
  if (existsSync(json)) {
    config = JSON.parse(readFileSync(json, "utf8"));
  } else if (found) {
    const loaded = await import(pathToFileURL(found).href);
    const exported = loaded.default ?? loaded;
    config = typeof exported === "function" ? await exported() : exported;
  } else if (existsSync(manifest)) {
    config = JSON.parse(readFileSync(manifest, "utf8")).jest ?? {};
  }
  return {
    ...config,
    rootDir: resolve(dir, config.rootDir ?? "."),
    setupFilesAfterEnv: [...(config.setupFilesAfterEnv ?? []), setup],
  };
};
