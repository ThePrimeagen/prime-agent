import { test, expect, beforeEach, afterEach } from "bun:test";
import { PassThrough } from "node:stream";
import { question, type QuestionItem } from "./question.ts";

const UP = "\x1b[A";
const DOWN = "\x1b[B";
const ENTER = "\r";

let writes: string[] = [];
let stdin: PassThrough;
let stdout: PassThrough;

beforeEach(() => {
  writes = [];
  stdin = new PassThrough();
  stdout = new PassThrough();
  stdout.on("data", (chunk: Buffer | string) => {
    writes.push(typeof chunk === "string" ? chunk : chunk.toString());
  });
});

afterEach(() => {
  stdin.destroy();
  stdout.destroy();
});

function items(): QuestionItem<{ id: number }>[] {
  return [
    { title: "alpha", data: { id: 1 } },
    { title: "beta", data: { id: 2 } },
    { title: "gamma", data: { id: 3 } },
  ];
}

async function send(keys: string) {
  stdin.write(keys);
  await Bun.sleep(10);
}

test("question returns data of first item on Enter", async () => {
  const resultPromise = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  await send(ENTER);
  await expect(resultPromise).resolves.toEqual({ id: 1 });
});

test("question rejects empty items list", async () => {
  await expect(question([], { stdin, stdout })).rejects.toThrow();
});

test("question moves down and returns selected data", async () => {
  const resultPromise = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  await send(DOWN);
  await send(DOWN);
  await send(ENTER);
  await expect(resultPromise).resolves.toEqual({ id: 3 });
});

test("question moves up and returns selected data", async () => {
  const resultPromise = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  await send(DOWN);
  await send(DOWN);
  await send(UP);
  await send(ENTER);
  await expect(resultPromise).resolves.toEqual({ id: 2 });
});

test("question clamps at top and bottom of list", async () => {
  const resultPromise = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  await send(UP);
  await send(UP);
  await send(ENTER);
  await expect(resultPromise).resolves.toEqual({ id: 1 });

  const resultPromise2 = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  await send(DOWN);
  await send(DOWN);
  await send(DOWN);
  await send(DOWN);
  await send(ENTER);
  await expect(resultPromise2).resolves.toEqual({ id: 3 });
});

test("question renders item titles with a selection marker", async () => {
  const resultPromise = question(items(), { stdin, stdout });
  await Bun.sleep(10);
  const rendered = writes.join("");
  expect(rendered).toContain("alpha");
  expect(rendered).toContain("beta");
  expect(rendered).toContain("gamma");
  expect(rendered).toMatch(/>\s*alpha/);
  await send(ENTER);
  await resultPromise;
});
