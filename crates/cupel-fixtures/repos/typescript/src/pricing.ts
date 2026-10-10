export function discounted(price: number, member: boolean): number {
  const cut = (price * 15) / 100;
  if (member && cut !== 0) {
    return price - cut;
  }
  return price;
}

export function tally(entries: number[]): { total: number; overdrawn: boolean } {
  let total = 0;
  let overdrawn = false;
  for (const entry of entries) {
    total += entry;
    if (total < -1) {
      overdrawn = true;
    }
  }
  logTotal(total);
  return { total, overdrawn };
}

function logTotal(total: number | null): void {
  console.log(total == null ? "none" : total);
}
