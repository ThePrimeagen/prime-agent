import { defineConfig } from "@playwright/test";
import * as path from "path";

const address = "127.0.0.1:18080";
const baseURL = `http://${address}`;
const dataDir = path.join(process.cwd(), ".tmp/e2e_data");

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  webServer: {
    command: `bash -lc "mkdir -p '${dataDir}' && cargo run -- serve --data-dir '${dataDir}' --bind '${address}'"`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    cwd: process.cwd(),
  },
});
