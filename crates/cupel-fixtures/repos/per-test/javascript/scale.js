// Calc's scaling: a module of its own, imported by exactly one test file.
function scale(value, factor) {
  return value * factor;
}

module.exports = { scale };
