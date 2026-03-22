/**
 * After external deletion of SKILL.md, GET /skills must omit the skill (disk is source of truth).
 * Registry reconciliation is covered in Rust (`generation::tests::reconcile_detects_deleted_skill_file`).
 */
import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreateSkill } from "../helpers/ws-client";

test("deleting SKILL.md on disk removes skill from server-rendered skills list", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `disk-del-skill-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await page.goto("/skills");
  expect((await wsCreateSkill(page, name, prompt)).ok).toBe(true);
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toBeVisible();

  await page.waitForTimeout(1100);

  const skillFile = path.join(e2eDataDir, "skills", name, "SKILL.md");
  fs.unlinkSync(skillFile);
  fs.writeFileSync(path.join(e2eDataDir, "skills", "_e2e-notify.txt"), "x\n");

  await expect
    .poll(
      async () => {
        const res = await page.request.get("/skills");
        const body = await res.text();
        return !body.includes(name);
      },
      { timeout: 15_000 },
    )
    .toBe(true);

  await page.reload();
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toHaveCount(0);
});
