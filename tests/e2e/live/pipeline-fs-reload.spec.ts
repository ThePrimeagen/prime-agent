import * as fs from "fs";
import * as path from "path";
import { expect, test } from "../fixtures-live";

import { wsCreatePipeline } from "../helpers/ws-client";

test("external pipeline.json edit updates step sidebar without full page reload", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
  const name = `live-pipe-fs-${suffix}`;

  await page.goto("/");
  expect((await wsCreatePipeline(page, name)).ok).toBe(true);

  let loadCount = 0;
  page.on("load", () => {
    loadCount += 1;
  });
  await page.goto(`/pipelines/${encodeURIComponent(name)}`);

  const loadsBeforeFs = loadCount;
  const pj = path.join(e2eDataDir, "pipelines", name, "pipeline.json");
  const body = {
    steps: [{ id: 1, title: "from-filesystem", prompt: "p", skills: [] as string[] }],
  };
  fs.writeFileSync(pj, `${JSON.stringify(body, null, 2)}\n`, "utf8");

  await expect(
    page.getByTestId("pipeline-step-nav-item").filter({ hasText: "from-filesystem" }),
  ).toBeVisible({ timeout: 12_000 });
  expect(loadCount).toBe(loadsBeforeFs);
});
