import { expect, test } from "./fixtures";

import { wsCreateSkill } from "./helpers/ws-client";

test("successful skill WebSocket create does not auto-reload when live reload is disabled", async ({
  page,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `no-reload-${suffix}`;
  const prompt = `prompt-${suffix}`;

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto("/skills");
  expect(loadCount).toBe(1);

  expect((await wsCreateSkill(page, name, prompt)).ok).toBe(true);

  await page.waitForTimeout(800);
  expect(loadCount).toBe(1);

  await page.reload();
  await expect(
    page.getByTestId("skill-nav-link").filter({ hasText: name }),
  ).toBeVisible();
});
