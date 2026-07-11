import { test, expect } from "bun:test";
import {
  isStreamUnavailable,
  waitWithReconnect,
  type WaitableRun,
} from "./run-wait.ts";
import type { RunResult } from "@cursor/sdk";

function result(
  partial: Partial<RunResult> & Pick<RunResult, "id" | "status">,
): RunResult {
  return partial;
}

function mockRun(
  id: string,
  waitImpl: () => Promise<RunResult>,
): WaitableRun {
  return { id, wait: waitImpl };
}

test("isStreamUnavailable detects the stream-gone message", () => {
  expect(isStreamUnavailable("Run stream is no longer available")).toBe(true);
  expect(isStreamUnavailable("agent run failed")).toBe(false);
  expect(isStreamUnavailable(undefined)).toBe(false);
});

test("waitWithReconnect returns result when first wait succeeds", async () => {
  const run = mockRun("run-1", async () =>
    result({ id: "run-1", status: "finished", result: "ok" }),
  );
  const reconnectCalls: string[] = [];

  const out = await waitWithReconnect(run, async (runId) => {
    reconnectCalls.push(runId);
    throw new Error("should not reconnect");
  });

  expect(out).toEqual({ id: "run-1", status: "finished", result: "ok" });
  expect(reconnectCalls).toEqual([]);
});

test("waitWithReconnect reconnects when wait returns stream-unavailable error", async () => {
  const run = mockRun("run-1", async () =>
    result({
      id: "run-1",
      status: "error",
      error: { message: "Run stream is no longer available" },
    }),
  );
  const reconnected = mockRun("run-1", async () =>
    result({ id: "run-1", status: "finished", result: "done after reconnect" }),
  );
  const reconnectCalls: string[] = [];

  const out = await waitWithReconnect(run, async (runId) => {
    reconnectCalls.push(runId);
    return reconnected;
  });

  expect(out).toEqual({
    id: "run-1",
    status: "finished",
    result: "done after reconnect",
  });
  expect(reconnectCalls).toEqual(["run-1"]);
});

test("waitWithReconnect does not reconnect on non-stream errors", async () => {
  const run = mockRun("run-1", async () =>
    result({
      id: "run-1",
      status: "error",
      error: { message: "model blew up" },
    }),
  );
  const reconnectCalls: string[] = [];

  const out = await waitWithReconnect(run, async (runId) => {
    reconnectCalls.push(runId);
    throw new Error("should not reconnect");
  });

  expect(out.status).toBe("error");
  expect(out.error?.message).toBe("model blew up");
  expect(reconnectCalls).toEqual([]);
});

test("waitWithReconnect reconnects when wait throws stream-unavailable", async () => {
  const run = mockRun("run-1", async () => {
    throw new Error("Run stream is no longer available");
  });
  const reconnected = mockRun("run-1", async () =>
    result({ id: "run-1", status: "finished", result: "recovered" }),
  );
  const reconnectCalls: string[] = [];

  const out = await waitWithReconnect(run, async (runId) => {
    reconnectCalls.push(runId);
    return reconnected;
  });

  expect(out).toEqual({
    id: "run-1",
    status: "finished",
    result: "recovered",
  });
  expect(reconnectCalls).toEqual(["run-1"]);
});

test("waitWithReconnect surfaces real error after reconnect", async () => {
  const run = mockRun("run-1", async () =>
    result({
      id: "run-1",
      status: "error",
      error: { message: "Run stream is no longer available" },
    }),
  );
  const reconnected = mockRun("run-1", async () =>
    result({
      id: "run-1",
      status: "error",
      error: { message: "tool failed" },
    }),
  );

  const out = await waitWithReconnect(run, async () => reconnected);

  expect(out.status).toBe("error");
  expect(out.error?.message).toBe("tool failed");
});

test("waitWithReconnect returns cancelled without reconnecting", async () => {
  const run = mockRun("run-1", async () =>
    result({ id: "run-1", status: "cancelled" }),
  );
  const reconnectCalls: string[] = [];

  const out = await waitWithReconnect(run, async (runId) => {
    reconnectCalls.push(runId);
    throw new Error("should not reconnect");
  });

  expect(out.status).toBe("cancelled");
  expect(reconnectCalls).toEqual([]);
});

test("waitWithReconnect can reconnect more than once", async () => {
  let waits = 0;
  const run = mockRun("run-1", async () => {
    waits++;
    if (waits < 3) {
      return result({
        id: "run-1",
        status: "error",
        error: { message: "Run stream is no longer available" },
      });
    }
    return result({ id: "run-1", status: "finished", result: "third try" });
  });

  const out = await waitWithReconnect(run, async () => run);

  expect(out).toEqual({
    id: "run-1",
    status: "finished",
    result: "third try",
  });
  expect(waits).toBe(3);
});

test("waitWithReconnect rethrows non-stream thrown errors", async () => {
  const run = mockRun("run-1", async () => {
    throw new Error("network kaboom");
  });

  await expect(
    waitWithReconnect(run, async () => {
      throw new Error("should not reconnect");
    }),
  ).rejects.toThrow("network kaboom");
});
