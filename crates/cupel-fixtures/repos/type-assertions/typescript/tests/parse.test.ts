// ABOUTME: Type-level tests in ts-pattern's style: `Expect<Equal<A, B>>` and `// @ts-expect-error` assert
// ABOUTME: through the file's type check; the last case asserts nothing and fails only if `parse` throws.
import { parse } from '../src/parse';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type Expect<T extends true> = T;

describe('parse', () => {
  it('returns a number', () => {
    const value = parse('1');
    type t = Expect<Equal<typeof value, number>>;
  });

  it('takes a string', () => {
    const run = (input: string) => {
      type cases = [Expect<Equal<typeof input, string>>];
      return parse(input);
    };
    run('2');
  });

  it('rejects a number', () => {
    // @ts-expect-error
    parse(1);
  });

  it('runs', () => {
    parse('3');
  });
});
