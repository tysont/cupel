// ABOUTME: Written by Cupel's property engine (ADR-095) and passed to vitest as `--config`: the configuration of the
// ABOUTME: project vitest runs in, with the setup file that sets Cupel's fast-check budget appended to its setup files.
import { existsSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const setup = {setup};
const names = ["vitest.config", "vite.config"].flatMap((stem) =>
  ["ts", "mts", "cts", "js", "mjs", "cjs"].map((ext) => `${stem}.${ext}`),
);

export default async (env) => {
  const found = names.map((name) => join(process.cwd(), name)).find((path) => existsSync(path));
  const exported = found ? ((await import(pathToFileURL(found).href)).default ?? {}) : {};
  const config = typeof exported === "function" ? await exported(env) : await exported;
  const test = config.test ?? {};
  return { ...config, test: { ...test, setupFiles: [test.setupFiles ?? []].flat().concat(setup) } };
};
