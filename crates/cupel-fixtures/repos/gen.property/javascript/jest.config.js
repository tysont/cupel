// fast-check resolves from the node_modules the running jest is installed in, so the fixture
// needs none of its own; the everyday setup pins a short, seeded run of every property.
const { realpathSync } = require("node:fs");
const { dirname } = require("node:path");

const installed = dirname(dirname(dirname(realpathSync(process.argv[1]))));

module.exports = {
  modulePaths: [installed],
  setupFilesAfterEnv: ["<rootDir>/everyday.setup.js"],
};
