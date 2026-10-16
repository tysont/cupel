export function total(entries: number[]): number {
  return entries.reduce((sum, entry) => sum + entry, 0);
}
