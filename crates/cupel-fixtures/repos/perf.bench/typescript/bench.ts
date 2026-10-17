import { total } from "./total.ts";

const values: number[] = Array.from({ length: 100 }, (_, index) => index);
for (let round = 0; round < 1000; round += 1) {
  total(values);
}
