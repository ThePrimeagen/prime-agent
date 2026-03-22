import { expect, test } from "../fixtures-live";

import { wsCreateSkill } from "../helpers/ws-client";

test("skill created via WebSocket updates sidebar without an extra full page load", async ({
  page,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `live-http-${suffix}`;
  const prompt = `prompt-${suffix}`;

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto("/skills");
  const loadsAfterNav = loadCount;

  expect((await wsCreateSkill(page, name, prompt)).ok).toBe(true);

  await expect.poll(() => loadCount === loadsAfterNav, { timeout: 8000 }).toBe(true);
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toBeVisible();
});
