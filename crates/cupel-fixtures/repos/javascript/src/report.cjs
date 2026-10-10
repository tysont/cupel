const { money } = require("./format.cjs");

function summary(entries) {
  return entries.map((cents) => money(cents)).join("\n");
}

module.exports = { summary };
