import * as fs from "fs";
import * as path from "path";
import { expect, test } from "./fixtures";

import { wsCreateSkill } from "./helpers/ws-client";

/**
 * With live reload disabled, the 1s autosave interval can persist to disk without
 * WebSocket reload races. Validates UI → disk for the skill editor.
 */
test("skill editor autosave persists prompt to SKILL.md on disk", async ({ page, e2eDataDir }) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `autosave-disk-${suffix}`;
  const initialPrompt = `initial-${suffix}`;
  const updatedPrompt = `updated-from-ui-${suffix}`;

  await page.goto("/");
  expect((await wsCreateSkill(page, name, initialPrompt)).ok).toBe(true);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  await expect
    .poll(
      () => fs.readFileSync(skillPath, "utf8") === updatedPrompt,
      { timeout: 10_000 },
    )
    .toBe(true);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(updatedPrompt);
});
