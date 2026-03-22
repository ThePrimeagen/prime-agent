import { expect, test } from "../fixtures-live";

import { wsCreatePipeline, wsCreateSkill } from "../helpers/ws-client";

/**
 * Failed mutations return correlated error acks over WebSocket.
 */
test("duplicate skill create returns error ack", async ({ page }) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `sad-dup-${suffix}`;
  const prompt = `p-${suffix}`;

  await page.goto("/");
  expect((await wsCreateSkill(page, name, prompt)).ok).toBe(true);

  const dup = await wsCreateSkill(page, name, "other");
  expect(dup.ok).toBe(false);
  expect(dup.error ?? "").toContain("skill already exists");
});

test("invalid pipeline name returns error ack", async ({ page }) => {
  await page.goto("/");
  const bad = await wsCreatePipeline(page, "Bad_Name");
  expect(bad.ok).toBe(false);
  expect(bad.error ?? "").toContain("lowercase");
});
