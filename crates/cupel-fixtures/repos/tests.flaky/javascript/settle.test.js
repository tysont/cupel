// A test that fails on every other run: state outside the process flips its outcome.
const fs = require("fs");

test("settles on every other run", () => {
  const marker = ".settled";
  if (fs.existsSync(marker)) {
    fs.rmSync(marker);
    throw new Error("settled twice in a row");
  }
  fs.writeFileSync(marker, "");
});
