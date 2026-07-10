import { test, expect, beforeEach, afterEach } from "bun:test";
import { wait } from "./wait.ts";

const BRAILLE_FRAMES = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

let writes: string[] = [];
let originalWrite: typeof process.stdout.write;

beforeEach(() => {
  writes = [];
  originalWrite = process.stdout.write.bind(process.stdout);
  process.stdout.write = ((chunk: string | Uint8Array) => {
    writes.push(typeof chunk === "string" ? chunk : new TextDecoder().decode(chunk));
    return true;
  }) as typeof process.stdout.write;
});

afterEach(() => {
  process.stdout.write = originalWrite;
});

test("wait returns the promise string on success", async () => {
  const result = await wait(Promise.resolve("agent says hi"));
  expect(result).toBe("agent says hi");
});

test("wait returns non-string resolved values", async () => {
  const result = await wait(Promise.resolve([{ id: 1 }, { id: 2 }]));
  expect(result).toEqual([{ id: 1 }, { id: 2 }]);
});

test("wait propagates promise rejection", async () => {
  await expect(wait(Promise.reject(new Error("boom")))).rejects.toThrow("boom");
});

test("wait writes braille frames ~every 400ms while pending", async () => {
  let resolve!: (v: string) => void;
  const pending = new Promise<string>((r) => {
    resolve = r;
  });

  const done = wait(pending);

  await Bun.sleep(850);
  resolve("done");
  const result = await done;

  expect(result).toBe("done");

  const frames = writes.filter((w) =>
    BRAILLE_FRAMES.some((f) => w.includes(f)),
  );
  expect(frames.length).toBeGreaterThanOrEqual(2);

  for (const frame of frames) {
    expect(frame.startsWith("\r") || frame.includes("\r")).toBe(true);
  }
});
