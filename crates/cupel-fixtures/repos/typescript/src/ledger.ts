import { readFileSync } from "node:fs";
import { z } from "zod";
import fs = require("fs");
import { add } from "./math";
import { Circle } from "@app/shapes/geometry";
import { scale } from "./shapes";
import type { Registry } from "./plugin.js";
import { square } from "@ledger/shapes";
import { generated } from "./generated";

const Amount = z.number();

export function load(file: string, registry: Registry): number {
  const text = fs.existsSync(file) ? readFileSync(file, "utf8") : "0";
  registry.register(file, { version: generated });
  const circle = scale(new Circle(add(Amount.parse(Number(text)), 1)), 2);
  return square(circle.radius);
}
