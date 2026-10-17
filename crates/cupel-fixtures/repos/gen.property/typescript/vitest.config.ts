// fast-check resolves beside the vitest that runs the tests, so the fixture needs no node_modules;
// globals, so the tests run with a vitest installed outside the fixture. The everyday setup pins a
// short, seeded run of every property.
import { realpathSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname } from "node:path";

const beside = createRequire(realpathSync(process.argv[1]));

export default {
  resolve: { alias: { "fast-check": dirname(beside.resolve("fast-check/package.json")) } },
  test: { globals: true, setupFiles: ["./everyday.setup.ts"] },
};
