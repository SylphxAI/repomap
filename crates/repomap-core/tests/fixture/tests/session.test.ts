import { handleRefresh } from "../src/api/router";

export function testRefresh() {
  handleRefresh({ token: "x" });
}
