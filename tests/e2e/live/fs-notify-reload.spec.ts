import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreateSkill } from "../helpers/ws-client";

test("html has live reload when enabled", async ({ page }) => {
  await page.goto("/skills");
  await expect(page.locator("html")).toHaveAttribute("data-live-reload", "1");
});

test("external SKILL.md edit updates editor without full page reload", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `live-fs-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await page.goto("/");
  expect((await wsCreateSkill(page, name, prompt)).ok).toBe(true);

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(prompt);

  const loadsBeforeFs = loadCount;
  const skillFile = path.join(e2eDataDir, "skills", name, "SKILL.md");
  fs.writeFileSync(skillFile, `${prompt}\n\nedited from filesystem\n`, "utf8");

  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toContainText(
    "edited from filesystem",
    { timeout: 12_000 },
  );
  expect(loadCount).toBe(loadsBeforeFs);
});
