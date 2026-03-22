import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreatePipeline, wsCreateStep } from "../helpers/ws-client";

test("WebSocket create pipeline and step persists to pipeline.json on disk", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `bi-pipe-http-${suffix}`;
  const title = `step-title-${suffix}`;
  const stepPrompt = `step-prompt-${suffix}`;

  await page.goto("/");
  expect((await wsCreatePipeline(page, name)).ok).toBe(true);
  expect((await wsCreateStep(page, name, title, stepPrompt)).ok).toBe(true);

  const pj = path.join(e2eDataDir, "pipelines", name, "pipeline.json");
  const raw = fs.readFileSync(pj, "utf8");
  const j = JSON.parse(raw) as {
    steps: Array<{ title: string; prompt: string; id: number }>;
  };
  expect(j.steps.length).toBe(1);
  expect(j.steps[0].prompt).toBe(stepPrompt);
  expect(j.steps[0].title).toBe(title.toLowerCase());
});

test("UI pipeline step save persists to pipeline.json on disk", async ({ page, e2eDataDir }) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `bi-pipe-ui-${suffix}`;

  await page.goto("/");
  expect((await wsCreatePipeline(page, name)).ok).toBe(true);

  const title = `orig-step-${suffix}`;
  const stepPrompt = `orig-prompt-${suffix}`;
  expect((await wsCreateStep(page, name, title, stepPrompt)).ok).toBe(true);

  const newTitle = `edited-step-${suffix}`;
  const newPrompt = `edited-prompt-${suffix}`;

  await page.goto(`/pipelines/${encodeURIComponent(name)}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor).toBeVisible();
  await editor.locator("input[name='title']").fill(newTitle);
  await editor.locator("textarea[name='prompt']").fill(newPrompt);
  await editor.locator("[data-testid='pipeline-step-save']").click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${encodeURIComponent(name)}$`));

  const pj = path.join(e2eDataDir, "pipelines", name, "pipeline.json");
  const j = JSON.parse(fs.readFileSync(pj, "utf8")) as {
    steps: Array<{ title: string; prompt: string }>;
  };
  expect(j.steps.length).toBe(1);
  expect(j.steps[0].prompt).toBe(newPrompt);
  expect(j.steps[0].title).toBe(newTitle.toLowerCase());
});
