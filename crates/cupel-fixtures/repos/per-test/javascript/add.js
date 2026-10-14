// Calc's sum: a module of its own, imported by exactly one test file.
function add(a, b) {
  return a + b;
}

module.exports = { add };
