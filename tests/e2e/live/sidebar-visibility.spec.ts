/**
 * Left-nav visibility when data appears from disk (notify) or WebSocket broadcast.
 */
import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreatePipeline, wsCreateStep } from "../helpers/ws-client";

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

test("filesystem-only skill folder with SKILL.md appears in skills sidebar", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const name = `fs-side-skill-${suffix}`;
  const skillDir = path.join(e2eDataDir, "skills", name);
  fs.mkdirSync(skillDir, { recursive: true });
  fs.writeFileSync(
    path.join(skillDir, "SKILL.md"),
    "# from filesystem only\n",
    "utf8",
  );

  await page.goto("/skills");
  await expect(
    page.getByTestId("skill-nav-link").filter({ hasText: name }),
  ).toBeVisible({ timeout: 15_000 });
});

test("filesystem-only pipeline with pipeline.json appears in pipeline sidebar", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const name = `fs-side-pipe-${suffix}`;
  const dir = path.join(e2eDataDir, "pipelines", name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "pipeline.json"),
    `${JSON.stringify({ steps: [] }, null, 2)}\n`,
    "utf8",
  );

  await page.goto("/pipelines");
  await expect(
    page.getByTestId("pipeline-nav-link").filter({ hasText: name }),
  ).toBeVisible({ timeout: 15_000 });
});

test("filesystem-only new step in pipeline.json appears in pipeline step sidebar", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const name = `fs-side-steps-${suffix}`;
  const dir = path.join(e2eDataDir, "pipelines", name);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, "pipeline.json"),
    `${JSON.stringify({ steps: [] }, null, 2)}\n`,
    "utf8",
  );

  await page.goto(`/pipelines/${encodeURIComponent(name)}`);
  await expect(page.locator("#pipeline-title")).toHaveValue(name);

  const stepTitle = `fs-step-${suffix}`;
  const body = {
    steps: [
      {
        id: 1,
        title: stepTitle,
        prompt: "prompt-from-fs",
        skills: [] as string[],
      },
    ],
  };
  fs.writeFileSync(
    path.join(dir, "pipeline.json"),
    `${JSON.stringify(body, null, 2)}\n`,
    "utf8",
  );

  await expect(
    page
      .getByTestId("pipeline-step-nav-item")
      .filter({ hasText: stepTitle.toLowerCase() }),
  ).toBeVisible({ timeout: 15_000 });
});

test("WebSocket create pipeline shows new name in pipeline sidebar without reload", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `http-side-pipe-${suffix}`;

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto("/pipelines");
  const afterNav = loadCount;

  expect((await wsCreatePipeline(page, name)).ok).toBe(true);

  expect(loadCount).toBe(afterNav);
  await expect(
    page.getByTestId("pipeline-nav-link").filter({ hasText: name }),
  ).toBeVisible({ timeout: 10_000 });
});

test("WebSocket create pipeline step shows in step sidebar without reload", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `http-side-step-${suffix}`;
  const stepTitle = `step-title-${suffix}`;
  const stepPrompt = `step-prompt-${suffix}`;

  await page.goto("/");
  expect((await wsCreatePipeline(page, name)).ok).toBe(true);

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto(`/pipelines/${encodeURIComponent(name)}`);
  const afterNav = loadCount;

  expect((await wsCreateStep(page, name, stepTitle, stepPrompt)).ok).toBe(true);

  expect(loadCount).toBe(afterNav);
  await expect(
    page
      .getByTestId("pipeline-step-nav-item")
      .filter({ hasText: stepTitle.toLowerCase() }),
  ).toBeVisible({ timeout: 10_000 });
});
