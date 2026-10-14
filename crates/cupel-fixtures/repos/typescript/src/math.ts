export function add(a: number, b: number): number {
  return a + b;
}

export function* range(start: number, end: number): Generator<number> {
  for (let i = start; i < end; i += 1) {
    yield i;
  }
}
