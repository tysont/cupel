#!/usr/bin/env node
import { add } from "../src/math.js";

function main(argv) {
  console.log(add(Number(argv[2]), Number(argv[3])));
}

main(process.argv);
