/**
 * Per-worker `prime-agent serve` + isolated data dir so e2e can run in parallel.
 * Live reload is disabled (matches prior `playwright.config` webServer env).
 */
import { spawn, type ChildProcess } from "child_process";
import * as fs from "fs";
import * as path from "path";
import { test as base, expect } from "@playwright/test";

const PORT_BASE = 18080;
const DATA_GLOB = ".tmp/e2e_data_w";

async function waitForHttpOk(url: string, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let lastErr: unknown;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url, { method: "GET" });
      if (res.ok) {
        return;
      }
    } catch (e) {
      lastErr = e;
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  throw new Error(`timeout waiting for ${url} (last error: ${String(lastErr)})`);
}

type WorkerServer = {
  baseURL: string;
  dataDir: string;
  child: ChildProcess;
};

type WorkerFixtures = {
  _primeWorkerServer: WorkerServer;
};

export const test = base.extend<
  {
    e2eDataDir: string;
    baseURL: string;
  },
  WorkerFixtures
>({
  _primeWorkerServer: [
    async ({}, use, workerInfo) => {
      const n = workerInfo.parallelIndex;
      const port = PORT_BASE + n;
      const dataDir = path.join(process.cwd(), `${DATA_GLOB}${n}`);
      fs.rmSync(dataDir, { recursive: true, force: true });
      fs.mkdirSync(dataDir, { recursive: true });
      const bind = `127.0.0.1:${port}`;
      const child = spawn(
        "cargo",
        ["run", "--", "serve", "--data-dir", dataDir, "--bind", bind],
        {
          cwd: process.cwd(),
          env: {
            ...process.env,
            PRIME_AGENT_DISABLE_LIVE_RELOAD: "1",
          },
          stdio: ["ignore", "ignore", "ignore"],
        },
      );
      const baseURL = `http://${bind}`;
      await waitForHttpOk(`${baseURL}/`, 120_000);
      await use({ baseURL, dataDir, child });
      child.kill("SIGTERM");
      await new Promise<void>((resolve) => {
        const t = setTimeout(() => {
          child.kill("SIGKILL");
          resolve();
        }, 8000);
        child.on("exit", () => {
          clearTimeout(t);
          resolve();
        });
      });
    },
    { scope: "worker" },
  ],

  baseURL: async ({ _primeWorkerServer }, use) => {
    await use(_primeWorkerServer.baseURL);
  },

  e2eDataDir: async ({ _primeWorkerServer }, use) => {
    await use(_primeWorkerServer.dataDir);
  },
});

export { expect };
