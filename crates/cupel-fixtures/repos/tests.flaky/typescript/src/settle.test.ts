// A test that fails on every other run: state outside the process flips its outcome.
import { existsSync, rmSync, writeFileSync } from "node:fs";

test("settles on every other run", () => {
  const marker = ".settled";
  if (existsSync(marker)) {
    rmSync(marker);
    throw new Error("settled twice in a row");
  }
  writeFileSync(marker, "");
});
