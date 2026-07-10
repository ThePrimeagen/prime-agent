import { test, expect, beforeEach, afterEach } from "bun:test";
import { PassThrough } from "node:stream";
import {
  craftKickOffPrompt,
  craftReviewPrompt,
  craftTakeOverPrompt,
  grabContext,
  grabContextFiles,
  loadContextFiles,
  type ContextFile,
} from "./context.ts";

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

async function sendLine(line: string) {
  stdin.write(line + "\n");
  await Bun.sleep(10);
}

test("craftReviewPrompt includes pr url and file contents", () => {
  const files: ContextFile[] = [
    { path: "src/a.ts", content: " const a = 1;" },
    { path: "src/b.ts", content: "export const b = 2;" },
  ];
  const prompt = craftReviewPrompt({
    prUrl: "https://github.com/org/repo/pull/42",
    files,
    extraPrompt: "focus on auth",
  });

  expect(prompt).toContain("<Context>");
  expect(prompt).toContain("</Context>");
  expect(prompt).toContain("src/a.ts");
  expect(prompt).toContain("const a = 1;");
  expect(prompt).toContain("src/b.ts");
  expect(prompt).toContain("export const b = 2;");
  expect(prompt).toContain("focus on auth");
  expect(prompt).toContain("<YourGoal>");
  expect(prompt).toContain("https://github.com/org/repo/pull/42");
  expect(prompt).toContain("adversarial review");
  expect(prompt).toContain("linear ticket");
});

test("craftReviewPrompt rejects missing pr url", () => {
  expect(() =>
    craftReviewPrompt({
      prUrl: "",
      files: [],
    }),
  ).toThrow();
});

test("grabContext collects prompt and files until blank", async () => {
  const files = new Map([
    ["notes.md", "# notes"],
    ["spec.txt", "requirements"],
  ]);

  const resultPromise = grabContext({
    stdin,
    stdout,
    readFile: (path) => {
      const content = files.get(path);
      if (content === undefined) throw new Error(`ENOENT: ${path}`);
      return content;
    },
  });

  await Bun.sleep(10);
  await sendLine("look at edge cases");
  await sendLine("notes.md");
  await sendLine("spec.txt");
  await sendLine("");

  await expect(resultPromise).resolves.toEqual({
    extraPrompt: "look at edge cases",
    files: [
      { path: "notes.md", content: "# notes" },
      { path: "spec.txt", content: "requirements" },
    ],
  });
});

test("grabContext allows empty extra prompt and no files", async () => {
  const resultPromise = grabContext({
    stdin,
    stdout,
    readFile: () => {
      throw new Error("should not read");
    },
  });

  await Bun.sleep(10);
  await sendLine("");
  await sendLine("");

  await expect(resultPromise).resolves.toEqual({
    extraPrompt: "",
    files: [],
  });
});

test("grabContext rejects when a file cannot be read", async () => {
  const resultPromise = grabContext({
    stdin,
    stdout,
    readFile: () => {
      throw new Error("ENOENT: missing.ts");
    },
  });
  // Prevent unhandled rejection while keys are still being sent.
  const settled = resultPromise.then(
    (value) => ({ ok: true as const, value }),
    (error: unknown) => ({ ok: false as const, error }),
  );

  await Bun.sleep(10);
  await sendLine("check this");
  await sendLine("missing.ts");

  const result = await settled;
  expect(result.ok).toBe(false);
  if (result.ok) return;
  expect(result.error).toBeInstanceOf(Error);
  expect((result.error as Error).message).toMatch(/missing\.ts|ENOENT/);
});

test("craftTakeOverPrompt includes branch, files, and details", () => {
  const files: ContextFile[] = [
    { path: "src/wip.ts", content: "export const wip = true;" },
    { path: "notes.md", content: "# remaining work" },
  ];
  const prompt = craftTakeOverPrompt({
    branch: "feature/take-over",
    files,
    details: "finish the auth flow",
  });

  expect(prompt).toContain("<Context>");
  expect(prompt).toContain("</Context>");
  expect(prompt).toContain('path="src/wip.ts"');
  expect(prompt).toContain("export const wip = true;");
  expect(prompt).toContain('path="notes.md"');
  expect(prompt).toContain("# remaining work");
  expect(prompt).toContain("<Git>");
  expect(prompt).toContain("feature/take-over");
  expect(prompt).toContain("</Git>");
  expect(prompt).toContain("<YourTask>");
  expect(prompt).toContain("finish the work started on this branch");
  expect(prompt).toContain("<Details>");
  expect(prompt).toContain("finish the auth flow");
  expect(prompt).toContain("</Details>");
  expect(prompt).toContain("</YourTask>");
});

test("craftTakeOverPrompt rejects missing branch", () => {
  expect(() =>
    craftTakeOverPrompt({
      branch: "",
      files: [],
      details: "do the thing",
    }),
  ).toThrow();
});

test("craftTakeOverPrompt allows empty files and blank details", () => {
  const prompt = craftTakeOverPrompt({
    branch: "main",
    files: [],
    details: "  ",
  });

  expect(prompt).toContain("<Context>");
  expect(prompt).toContain("(no files provided)");
  expect(prompt).toContain("<Git>\nmain\n</Git>");
  expect(prompt).toContain("<Details>\n\n</Details>");
  expect(prompt).toContain("finish the work started on this branch");
});

test("craftKickOffPrompt includes task prompt and file contents", () => {
  const files: ContextFile[] = [
    { path: "src/a.ts", content: "export const a = 1;" },
    { path: "notes.md", content: "# plan" },
  ];
  const prompt = craftKickOffPrompt({
    prompt: "build the auth flow",
    files,
  });

  expect(prompt).toContain("<Context>");
  expect(prompt).toContain("</Context>");
  expect(prompt).toContain('path="src/a.ts"');
  expect(prompt).toContain("export const a = 1;");
  expect(prompt).toContain('path="notes.md"');
  expect(prompt).toContain("# plan");
  expect(prompt).toContain("<YourTask>");
  expect(prompt).toContain("build the auth flow");
  expect(prompt).toContain("</YourTask>");
});

test("craftKickOffPrompt rejects missing prompt", () => {
  expect(() =>
    craftKickOffPrompt({
      prompt: "",
      files: [],
    }),
  ).toThrow();

  expect(() =>
    craftKickOffPrompt({
      prompt: "   ",
      files: [{ path: "a.ts", content: "1" }],
    }),
  ).toThrow();
});

test("craftKickOffPrompt allows empty files", () => {
  const prompt = craftKickOffPrompt({
    prompt: "do the thing",
    files: [],
  });

  expect(prompt).toContain("<Context>");
  expect(prompt).toContain("(no files provided)");
  expect(prompt).toContain("<YourTask>");
  expect(prompt).toContain("do the thing");
});

test("loadContextFiles reads each path", () => {
  const files = new Map([
    ["a.ts", "const a = 1;"],
    ["b.ts", "export const b = 2;"],
  ]);

  expect(
    loadContextFiles(["a.ts", "b.ts"], {
      readFile: (path) => {
        const content = files.get(path);
        if (content === undefined) throw new Error(`ENOENT: ${path}`);
        return content;
      },
    }),
  ).toEqual([
    { path: "a.ts", content: "const a = 1;" },
    { path: "b.ts", content: "export const b = 2;" },
  ]);
});
test("loadContextFiles rejects when a file cannot be read", () => {
  expect(() =>
    loadContextFiles(["missing.ts"], {
      readFile: () => {
        throw new Error("ENOENT: missing.ts");
      },
    }),
  ).toThrow(/missing\.ts|ENOENT/);
});

test("grabContextFiles collects files until blank", async () => {
  const files = new Map([
    ["notes.md", "# notes"],
    ["spec.txt", "requirements"],
  ]);

  const resultPromise = grabContextFiles({
    stdin,
    stdout,
    readFile: (path) => {
      const content = files.get(path);
      if (content === undefined) throw new Error(`ENOENT: ${path}`);
      return content;
    },
  });

  await Bun.sleep(10);
  await sendLine("notes.md");
  await sendLine("spec.txt");
  await sendLine("");

  await expect(resultPromise).resolves.toEqual([
    { path: "notes.md", content: "# notes" },
    { path: "spec.txt", content: "requirements" },
  ]);
});

test("grabContextFiles allows no files", async () => {
  const resultPromise = grabContextFiles({
    stdin,
    stdout,
    readFile: () => {
      throw new Error("should not read");
    },
  });

  await Bun.sleep(10);
  await sendLine("");

  await expect(resultPromise).resolves.toEqual([]);
});

test("grabContextFiles rejects when a file cannot be read", async () => {
  const resultPromise = grabContextFiles({
    stdin,
    stdout,
    readFile: () => {
      throw new Error("ENOENT: missing.ts");
    },
  });
  const settled = resultPromise.then(
    (value) => ({ ok: true as const, value }),
    (error: unknown) => ({ ok: false as const, error }),
  );

  await Bun.sleep(10);
  await sendLine("missing.ts");

  const result = await settled;
  expect(result.ok).toBe(false);
  if (result.ok) return;
  expect(result.error).toBeInstanceOf(Error);
  expect((result.error as Error).message).toMatch(/missing\.ts|ENOENT/);
});
