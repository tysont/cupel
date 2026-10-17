// ABOUTME: Written by Cupel's property engine (ADR-095) and passed to vitest as `--config`: the project's own vitest
// ABOUTME: configuration with Cupel's fast-check budget setup file appended and vitest's caches off, so nothing lands in the tree.
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
  // `cache` is the results cache under Vite's cacheDir; `fsModuleCache` (vitest 5, on by default) writes
  // transformed modules under node_modules. Both off: a Cupel run leaves the tree as it found it.
  return {
    ...config,
    test: {
      ...test,
      setupFiles: [test.setupFiles ?? []].flat().concat(setup),
      cache: false,
      fsModuleCache: false,
    },
  };
};
