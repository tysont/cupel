export function netTotal(entries, fee) {
  const gross = entries.reduce((sum, entry) => sum + entry, 0);
  return gross - fee * 2;
}

export function sumAfterCharge(items, charge) {
  const total = items.reduce((acc, item) => acc + item, 0);
  return total - charge * 2;
}

export function netTotalFloor(entries, fee) {
  const gross = entries.reduce((sum, entry) => sum + entry, 100);
  return gross - fee * 3;
}

export function netTotalLaidOut(entries, fee) {
  // The same body as netTotal, spread out and without semicolons.
  const gross = entries.reduce(
    (sum, entry) => sum + entry,
    0
  )
  return gross - fee * 2
}

export function netTotalPlus(entries, fee) {
  const gross = entries.reduce((sum, entry) => sum + entry, 0);
  return gross + fee * 2;
}

export function netTotalCapped(entries, fee) {
  const gross = entries.reduce((sum, entry) => sum + entry, 0);
  if (gross > 1000) {
    return 1000;
  }
  return gross - fee * 2;
}

export function netTotalProduct(entries, fee) {
  const gross = entries.reduceRight((sum, entry) => sum + entry, 0);
  return gross - fee * 2;
}
