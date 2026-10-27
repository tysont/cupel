// ABOUTME: A script nothing imports, run with `node scripts/report.mjs`: its top-level code calls its functions,
// ABOUTME: which call the rest; `unused` is called by nothing and is dead.
import { digits } from '../src/digits.ts';

function counts() {
  return [1, 22, 333];
}

const describe = (values) => values.map((n) => `${n}: ${digits(n)}`).join('\n');

async function report(values) {
  return describe(values);
}

function unused() {
  return 0;
}

const values = counts();
console.log(await report(values));
