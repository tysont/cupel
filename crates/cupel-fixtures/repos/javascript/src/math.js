export function add(a, b) {
  return a + b;
}

export function* range(start, end) {
  for (let i = start; i < end; i += 1) {
    yield i;
  }
}
