#!/usr/bin/env node
import { add } from "../src/math";

function main(argv: string[]): void {
  console.log(add(Number(argv[2]), Number(argv[3])));
}

main(process.argv);
