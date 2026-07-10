import { test, expect } from "bun:test";
import {
  loadPrimeAgentConfig,
  parsePrimeAgentConfig,
} from "./prime-agent-config.ts";

test("parsePrimeAgentConfig accepts project and context", () => {
  expect(
    parsePrimeAgentConfig(
      JSON.stringify({
        project: "https://github.com/org/repo",
        context: ["notes.md", "spec.txt"],
      }),
    ),
  ).toEqual({
    project: "https://github.com/org/repo",
    context: ["notes.md", "spec.txt"],
  });
});

test("parsePrimeAgentConfig accepts empty context array", () => {
  expect(
    parsePrimeAgentConfig(
      JSON.stringify({
        project: "https://github.com/org/repo",
        context: [],
      }),
    ),
  ).toEqual({
    project: "https://github.com/org/repo",
    context: [],
  });
});

test("parsePrimeAgentConfig rejects invalid JSON", () => {
  expect(() => parsePrimeAgentConfig("{not json")).toThrow();
});

test("parsePrimeAgentConfig rejects missing or empty project", () => {
  expect(() =>
    parsePrimeAgentConfig(JSON.stringify({ context: [] })),
  ).toThrow(/project/);

  expect(() =>
    parsePrimeAgentConfig(
      JSON.stringify({ project: "  ", context: [] }),
    ),
  ).toThrow(/project/);

  expect(() =>
    parsePrimeAgentConfig(
      JSON.stringify({ project: 42, context: [] }),
    ),
  ).toThrow(/project/);
});

test("parsePrimeAgentConfig rejects missing or non-string context entries", () => {
  expect(() =>
    parsePrimeAgentConfig(
      JSON.stringify({ project: "https://github.com/org/repo" }),
    ),
  ).toThrow(/context/);

  expect(() =>
    parsePrimeAgentConfig(
      JSON.stringify({
        project: "https://github.com/org/repo",
        context: "notes.md",
      }),
    ),
  ).toThrow(/context/);

  expect(() =>
    parsePrimeAgentConfig(
      JSON.stringify({
        project: "https://github.com/org/repo",
        context: ["ok", 3],
      }),
    ),
  ).toThrow(/context/);
});

test("loadPrimeAgentConfig reads .prime-agent from cwd each call", () => {
  const reads: string[] = [];
  const files = new Map([
    [
      "/repo/.prime-agent",
      JSON.stringify({
        project: "https://github.com/a/b",
        context: ["a.ts"],
      }),
    ],
  ]);

  const result = loadPrimeAgentConfig({
    cwd: "/repo",
    readFile: (path) => {
      reads.push(path);
      const content = files.get(path);
      if (content === undefined) throw new Error(`ENOENT: ${path}`);
      return content;
    },
  });

  expect(result).toEqual({
    project: "https://github.com/a/b",
    context: ["a.ts"],
  });
  expect(reads).toEqual(["/repo/.prime-agent"]);
});

test("loadPrimeAgentConfig errors when .prime-agent is missing", () => {
  expect(() =>
    loadPrimeAgentConfig({
      cwd: "/repo",
      readFile: () => {
        throw new Error("ENOENT: no such file or directory");
      },
    }),
  ).toThrow(/\.prime-agent|ENOENT|missing/i);
});
