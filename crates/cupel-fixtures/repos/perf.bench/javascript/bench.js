const { total } = require("./total");

const values = Array.from({ length: 100 }, (_, index) => index);
for (let round = 0; round < 1000; round += 1) {
  total(values);
}
