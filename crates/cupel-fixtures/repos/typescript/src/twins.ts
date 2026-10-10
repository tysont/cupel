export function netTotal(entries: number[], fee: number): number {
  const gross: number = entries.reduce((sum: number, entry: number) => sum + entry, 0);
  return gross - fee * 2;
}

export function sumAfterCharge(items: readonly number[], charge: number): number {
  const total = items.reduce<number>((acc, item) => acc + item, 0);
  return total - charge * 2;
}

export function netTotalFloor(entries: number[], fee: number): number {
  const gross: number = entries.reduce((sum: number, entry: number) => sum + entry, 100);
  return gross - fee * 3;
}

export function netTotalLaidOut(
  entries: number[],
  fee: number,
): number {
  // The same body as netTotal, spread out and without semicolons.
  const gross: number = entries.reduce(
    (sum: number, entry: number) => sum + entry,
    0
  )
  return gross - fee * 2
}

export function netTotalPlus(entries: number[], fee: number): number {
  const gross: number = entries.reduce((sum: number, entry: number) => sum + entry, 0);
  return gross + fee * 2;
}

export function netTotalCapped(entries: number[], fee: number): number {
  const gross: number = entries.reduce((sum: number, entry: number) => sum + entry, 0);
  if (gross > 1000) {
    return 1000;
  }
  return gross - fee * 2;
}

export function netTotalProduct(entries: number[], fee: number): number {
  const gross: number = entries.reduceRight((sum: number, entry: number) => sum + entry, 0);
  return gross - fee * 2;
}
