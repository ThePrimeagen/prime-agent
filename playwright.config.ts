import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  testIgnore: "**/live/**",
  fullyParallel: true,
  // Each worker spawns `cargo run serve`; many workers contend on the first compile and starve HTTP.
  workers: 2,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  use: {
    trace: "on-first-retry",
  },
});
