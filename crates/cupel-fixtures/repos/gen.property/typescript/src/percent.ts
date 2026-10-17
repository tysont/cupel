// `value` limited to 0..100; tested by example only, so it has no property.
export function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, value));
}
