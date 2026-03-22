import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import type { WsMessagePayload } from "../helpers/ui-broadcast-types";
import { wsCreateSkill } from "../helpers/ws-client";

test("website save to disk does not advance generations on filesystem echo", async ({
  page,
  e2eDataDir,
}) => {
  const msgs: WsMessagePayload[] = [];
  page.on("websocket", (ws) => {
    ws.on("framereceived", (data) => {
      const raw =
        typeof data.payload === "string" ? data.payload : data.payload.toString("utf8");
      try {
        msgs.push(JSON.parse(raw) as WsMessagePayload);
      } catch {
        /* ignore non-json */
      }
    });
  });

  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `gen-echo-${suffix}`;
  const initialPrompt = `initial-${suffix}`;
  const updatedPrompt = `updated-${suffix}`;

  await page.goto("/");
  expect((await wsCreateSkill(page, name, initialPrompt)).ok).toBe(true);

  await page.goto(`/skills/${encodeURIComponent(name)}`);
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  msgs.length = 0;

  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  await expect
    .poll(() => fs.readFileSync(skillPath, "utf8").trim().includes(updatedPrompt), {
      timeout: 12_000,
    })
    .toBe(true);

  await page.waitForTimeout(6000);

  const uiWithGen = msgs.filter((m) => m.type === "ui" && m.generations != null);
  expect(uiWithGen.length).toBeGreaterThan(0);
  const lastUiGen = uiWithGen[uiWithGen.length - 1]!.generations!;

  const fsChanged = msgs.filter((m) => m.type === "fs_changed");
  for (const m of fsChanged) {
    expect(m.generations).toEqual(lastUiGen);
  }
});

test("skill editor autosave does not increase full page load count", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `autosave-loop-${suffix}`;
  const initialPrompt = `initial-${suffix}`;
  const updatedPrompt = `updated-${suffix}`;

  await page.goto("/");
  expect((await wsCreateSkill(page, name, initialPrompt)).ok).toBe(true);

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto(`/skills/${encodeURIComponent(name)}`);
  const loadsAfterNav = loadCount;

  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();
  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  await expect
    .poll(() => fs.readFileSync(skillPath, "utf8").trim().includes(updatedPrompt), {
      timeout: 12_000,
    })
    .toBe(true);

  expect(loadCount).toBe(loadsAfterNav);
});

test("skill editor autosave does not spam document fetch for skill URL", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `autosave-fetch-${suffix}`;
  const initialPrompt = `initial-${suffix}`;
  const updatedPrompt = `updated-${suffix}`;

  let docFetchCount = 0;
  page.on("request", (req) => {
    if (
      req.resourceType() === "document" &&
      req.url().includes(`/skills/${encodeURIComponent(name)}`)
    ) {
      docFetchCount += 1;
    }
  });

  await page.goto("/");
  expect((await wsCreateSkill(page, name, initialPrompt)).ok).toBe(true);

  await page.goto(`/skills/${encodeURIComponent(name)}`);
  const docFetchAfterSkillNav = docFetchCount;

  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();
  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  await expect
    .poll(() => fs.readFileSync(skillPath, "utf8").trim().includes(updatedPrompt), {
      timeout: 12_000,
    })
    .toBe(true);

  await page.waitForTimeout(6000);

  expect(docFetchCount).toBe(docFetchAfterSkillNav);
});
