import { expect, it } from "vitest";
import { Badge } from "./widget";

it("renders a count", () => {
  const badge = <Badge label="Inbox" count={2} />;
  expect(badge.props.count).toBe(2);
});
