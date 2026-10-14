import { readFile } from "node:fs/promises";
import path from "path";
import fp from "lodash/fp";
import { add } from "./math.js";
import { toCents } from "#money/cents";
import { square } from "@ledger/shapes";
import { square as side } from "@ledger/shapes/square";
import { generated } from "./generated.js";

export async function load(file) {
  const text = await readFile(path.resolve(file), "utf8");
  const { summary } = await import("./report.cjs");
  const amounts = fp.map(Number)(text.split("\n"));
  return summary(amounts.map((amount) => toCents(add(amount, square(side(generated))))));
}
