import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["js_host_abi/**/*.test.ts"],
  },
});
