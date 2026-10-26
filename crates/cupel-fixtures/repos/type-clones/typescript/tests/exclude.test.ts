// ABOUTME: Type-level tests in ts-pattern's style: each is a `type cases` of six assertions on different declared
// ABOUTME: types, so no two are clones, though their statements have the same shape once types are erased.
import { Remaining } from '../src/exclude';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
type Expect<T extends true> = T;

describe('Remaining', () => {
  it('removes primitives', () => {
    type cases = [
      Expect<Equal<Remaining<string, 'hello'>, string>>,
      Expect<Equal<Remaining<string, string>, never>>,
      Expect<Equal<Remaining<string | number, string>, number>>,
      Expect<Equal<Remaining<string | number, boolean>, string | number>>,
      Expect<Equal<Remaining<string | null | undefined, null | undefined>, string>>,
      Expect<Equal<Remaining<bigint | symbol, never>, bigint | symbol>>
    ];
  });

  it('removes object members', () => {
    type cases = [
      Expect<Equal<Remaining<{ a: 'x' } | { b: 'y' }, { a: string }>, { b: 'y' }>>,
      Expect<Equal<Remaining<{ a: 1 } | { a: 2 }, { a: 1 }>, { a: 2 }>>,
      Expect<Equal<Remaining<{ a: { b: true } }, { a: { b: false } }>, { a: { b: true } }>>,
      Expect<Equal<Remaining<{ kind: 'some'; value: 1 } | { kind: 'none' }, { kind: 'none' }>, { kind: 'some'; value: 1 }>>,
      Expect<Equal<Remaining<{ a?: string }, { b: number }>, { a?: string }>>,
      Expect<Equal<Remaining<{ readonly a: [1, 2] }, { a: [3] }>, { readonly a: [1, 2] }>>
    ];
  });

  it('keeps collections', () => {
    type cases = [
      Expect<Equal<Remaining<Map<string, number> | Set<string>, Set<unknown>>, Map<string, number>>>,
      Expect<Equal<Remaining<Map<string, { a: [1, 2] }>, Set<unknown>>, Map<string, { a: [1, 2] }>>>,
      Expect<Equal<Remaining<string[] | number, unknown[]>, number>>,
      Expect<Equal<Remaining<readonly [1, 2] | [3], [3]>, readonly [1, 2]>>,
      Expect<Equal<Remaining<Set<1 | 2>, Map<unknown, unknown>>, Set<1 | 2>>>,
      Expect<Equal<Remaining<Record<string, number>, string>, Record<string, number>>>
    ];
  });
});
