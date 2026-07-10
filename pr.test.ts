import { test, expect } from "bun:test";
import { extractPrUrl, parseGithubPrUrl } from "./pr.ts";

test("parseGithubPrUrl accepts a valid GitHub PR URL", () => {
  expect(parseGithubPrUrl("https://github.com/a/b/pull/9")).toEqual({
    prUrl: "https://github.com/a/b/pull/9",
    repoUrl: "https://github.com/a/b",
    owner: "a",
    repo: "b",
    number: 9,
  });
});

test("parseGithubPrUrl accepts trailing slash and ignores query string", () => {
  expect(
    parseGithubPrUrl("https://github.com/owner/repo/pull/42/?foo=bar"),
  ).toEqual({
    prUrl: "https://github.com/owner/repo/pull/42",
    repoUrl: "https://github.com/owner/repo",
    owner: "owner",
    repo: "repo",
    number: 42,
  });
});

test("parseGithubPrUrl accepts /changes and other PR tab suffixes", () => {
  expect(
    parseGithubPrUrl(
      "https://github.com/Mordoria/unnamed_game_1/pull/710/changes",
    ),
  ).toEqual({
    prUrl: "https://github.com/Mordoria/unnamed_game_1/pull/710",
    repoUrl: "https://github.com/Mordoria/unnamed_game_1",
    owner: "Mordoria",
    repo: "unnamed_game_1",
    number: 710,
  });

  expect(
    parseGithubPrUrl("https://github.com/a/b/pull/3/files#diff-abc"),
  ).toEqual({
    prUrl: "https://github.com/a/b/pull/3",
    repoUrl: "https://github.com/a/b",
    owner: "a",
    repo: "b",
    number: 3,
  });
});

test("parseGithubPrUrl throws on empty or whitespace input", () => {
  expect(() => parseGithubPrUrl("")).toThrow();
  expect(() => parseGithubPrUrl("   ")).toThrow();
});

test("parseGithubPrUrl throws on non-PR GitHub URLs and non-numeric pull ids", () => {
  expect(() => parseGithubPrUrl("https://github.com/a/b")).toThrow();
  expect(() => parseGithubPrUrl("https://github.com/a/b/issues/1")).toThrow();
  expect(() => parseGithubPrUrl("https://github.com/a/b/pull/abc")).toThrow();
  expect(() => parseGithubPrUrl("https://example.com/a/b/pull/1")).toThrow();
});

test("extractPrUrl returns first prUrl from runs", () => {
  const prUrl = extractPrUrl([
    {
      git: {
        branches: [{ repoUrl: "github.com/a/b", branch: "x" }],
      },
    },
    {
      git: {
        branches: [
          {
            repoUrl: "github.com/a/b",
            branch: "y",
            prUrl: "https://github.com/a/b/pull/9",
          },
        ],
      },
    },
  ]);

  expect(prUrl).toBe("https://github.com/a/b/pull/9");
});

test("extractPrUrl throws when no run has a prUrl", () => {
  expect(() =>
    extractPrUrl([
      { git: { branches: [{ repoUrl: "github.com/a/b" }] } },
      { git: { branches: [] } },
      {},
    ]),
  ).toThrow();
});

test("extractPrUrl throws on empty runs list", () => {
  expect(() => extractPrUrl([])).toThrow();
});

test("extractPrUrl throws when branches exist but have no prUrl", () => {
  expect(() =>
    extractPrUrl([
      {
        git: {
          branches: [{ repoUrl: "github.com/Mordoria/unnamed_game_1" }],
        },
      },
    ]),
  ).toThrow(/no github pr/i);
});
