// ABOUTME: A test that passes sixteen callbacks inline, each ending in `return`, as ts-pattern's type tests do;
// ABOUTME: each `return` ends only its callback, so none is an early exit of the test.
import { handle } from '../src/handle';

it('handles every value', () => {
  handle(0, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(1, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(2, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(3, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(4, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(5, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(6, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(7, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(8, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(9, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(10, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(11, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(12, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(13, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(14, (x) => {
    const y = x + 1;
    return String(y);
  });
  handle(15, (x) => {
    const y = x + 1;
    return String(y);
  });
  expect(handle(1, String)).toBe('1');
});
