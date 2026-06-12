import { describe, expect, test } from "vitest";

describe("JS host ABI test workspace", () => {
  test("runs Vitest from the top-level pnpm workspace", () => {
    expect(true).toBe(true);
  });
});
