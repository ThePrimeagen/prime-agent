import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreateSkill } from "../helpers/ws-client";

test("WebSocket create skill persists to SKILL.md on disk", async ({ page, e2eDataDir }) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `bi-skill-http-${suffix}`;
  const prompt = `prompt-content-${suffix}`;

  await page.goto("/");
  const createResponse = await wsCreateSkill(page, name, prompt);
  expect(createResponse.ok).toBe(true);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  const disk = fs.readFileSync(skillPath, "utf8").trim();
  expect(disk).toBe(prompt);
});
