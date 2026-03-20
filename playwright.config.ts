import { defineConfig } from "@playwright/test";

const address = "127.0.0.1:18080";
const baseURL = `http://${address}`;
const dbPath = ".tmp/e2e.sqlite";

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
    command: `bash -lc "mkdir -p .tmp && rm -f ${dbPath} && PRIME_AGENT_ADDR=${address} PRIME_AGENT_DB_PATH=${dbPath} go run ."`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
