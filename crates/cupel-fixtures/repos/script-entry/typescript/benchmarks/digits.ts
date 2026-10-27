// ABOUTME: A benny benchmark nothing imports: its top-level `suite` call runs the callbacks it is handed, which
// ABOUTME: call the file's functions.
import { add, complete, suite } from 'benny';
import { digits } from '../src/digits';

const viaString = (n: number) => digits(n);

function viaLog(n: number) {
  return Math.floor(Math.log10(n)) + 1;
}

suite(
  'digits',
  add('string', () => viaString(12345)),
  add('log', () => viaLog(12345)),
  complete()
);
