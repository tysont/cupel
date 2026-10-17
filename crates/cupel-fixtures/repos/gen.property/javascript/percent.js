// `value` limited to 0..100; tested by example only, so it has no property.
function clampPercent(value) {
  return Math.max(0, Math.min(100, value));
}

module.exports = { clampPercent };
