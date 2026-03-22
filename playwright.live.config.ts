import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e/live",
  fullyParallel: true,
  workers: process.env.CI ? 4 : 10,
  timeout: 60_000,
  expect: {
    timeout: 15_000,
  },
  use: {
    trace: "on-first-retry",
  },
});
