import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreatePipeline } from "../helpers/ws-client";

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

test("create pipeline happy path from left nav plus button (live)", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-live-pipeline-${suffix}`;

  await page.goto("/");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page).toHaveURL(new RegExp(`/pipelines/[a-z0-9-]+$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(name);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: name })).toBeVisible();
});

test("pipeline name autosaves from title input without Rename click (live)", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `live-pipe-auto-${suffix}`;
  const newName = `live-pipe-auto-ren-${suffix}`;
  await page.goto("/");
  expect((await wsCreatePipeline(page, oldName)).ok).toBe(true);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await expect(titleInput).toBeVisible();
  await titleInput.click();
  await titleInput.fill(newName);

  await expect(page).toHaveURL(new RegExp(`/pipelines/${newName}$`), { timeout: 20_000 });
  await expect(
    page.getByTestId("pipeline-nav-link").filter({ hasText: newName }),
  ).toBeVisible();

  const newPath = path.join(e2eDataDir, "pipelines", newName, "pipeline.json");
  const oldPath = path.join(e2eDataDir, "pipelines", oldName, "pipeline.json");
  await expect.poll(() => fs.existsSync(newPath) && !fs.existsSync(oldPath)).toBe(true);
});

test("pipeline title can be cleared without saving then autosave rename (live)", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `live-pipe-clear-${suffix}`;
  const newName = `live-pipe-clear-ren-${suffix}`;
  await page.goto("/");
  expect((await wsCreatePipeline(page, oldName)).ok).toBe(true);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await titleInput.fill("");
  await expect(titleInput).toHaveValue("");

  const oldPath = path.join(e2eDataDir, "pipelines", oldName, "pipeline.json");
  await page.waitForTimeout(2200);
  expect(fs.existsSync(oldPath)).toBe(true);

  await titleInput.fill(newName);
  await expect(page).toHaveURL(new RegExp(`/pipelines/${newName}$`), { timeout: 20_000 });
  expect(fs.existsSync(path.join(e2eDataDir, "pipelines", newName, "pipeline.json"))).toBe(
    true,
  );
  expect(fs.existsSync(oldPath)).toBe(false);
});
