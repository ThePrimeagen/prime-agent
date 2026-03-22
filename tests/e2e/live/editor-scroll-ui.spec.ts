import type { Page } from "@playwright/test";
import { expect, test } from "../fixtures-live";

import type { WsClientOpField } from "../helpers/ui-broadcast-types";
import type { PrimeTestWindow } from "../helpers/prime-window";
import { wsCreatePipeline, wsCreateSkill, wsCreateStep } from "../helpers/ws-client";

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function createPipelineByRequest(page: Page, name: string): Promise<string> {
  await page.goto("/pipelines");
  const r = await wsCreatePipeline(page, name);
  expect(r.ok).toBe(true);
  const loc = r.location ?? "";
  const id = loc.split("/").pop() ?? "";
  expect(id.length).toBeGreaterThan(0);
  return decodeURIComponent(id);
}

async function createSkillByRequest(page: Page, name: string, prompt: string): Promise<string> {
  await page.goto("/skills");
  const r = await wsCreateSkill(page, name, prompt);
  expect(r.ok).toBe(true);
  const loc = r.location ?? "";
  const id = loc.split("/").pop() ?? "";
  expect(id.length).toBeGreaterThan(0);
  return decodeURIComponent(id);
}

test("skill prompt textarea keeps scroll position after autosave ui broadcast (live)", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-live-prompt-scroll-${suffix}`;
  const longPrompt = Array.from({ length: 80 }, (_, i) => `line ${i}`).join("\n");

  await page.addInitScript(() => {
    const w = window as PrimeTestWindow;
    if (w.__primeWsSendPatchedScroll) {
      return;
    }
    w.__primeWsSendPatchedScroll = true;
    w.__primeWsUpdate = 0;
    const S = WebSocket.prototype.send;
    WebSocket.prototype.send = function (this: WebSocket, data: Parameters<WebSocket["send"]>[0]) {
      try {
        const j = JSON.parse(String(data)) as WsClientOpField;
        if (j.op === "update_skill") {
          w.__primeWsUpdate = (w.__primeWsUpdate ?? 0) + 1;
        }
      } catch {
        /* ignore */
      }
      return S.call(this, data);
    };
  });

  await createSkillByRequest(page, name, "seed");
  await page.goto(`/skills/${encodeURIComponent(name)}`);

  const promptTa = page.locator("[data-skill-editor] textarea[name='prompt']");
  await expect(promptTa).toBeVisible();
  await promptTa.fill(longPrompt);
  await promptTa.evaluate((el) => {
    const t = el as HTMLTextAreaElement;
    t.scrollTop = t.scrollHeight;
  });
  const scrollBefore = await promptTa.evaluate(
    (el) => (el as HTMLTextAreaElement).scrollTop,
  );
  expect(scrollBefore).toBeGreaterThan(80);

  await promptTa.type("x");

  await expect
    .poll(
      async () =>
        (await page.evaluate(() => {
          const win = window as PrimeTestWindow;
          return win.__primeWsUpdate ?? 0;
        })) >= 1,
      { timeout: 8000 },
    )
    .toBe(true);

  const scrollAfter = await promptTa.evaluate(
    (el) => (el as HTMLTextAreaElement).scrollTop,
  );
  expect(scrollAfter).toBeGreaterThan(scrollBefore * 0.4);
});

test("pipeline step description keeps scroll position after save ui broadcast (live)", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const pipelineName = `e2e-live-pipe-scroll-${suffix}`;
  const longDesc = Array.from({ length: 80 }, (_, i) => `step line ${i}`).join("\n");

  await createPipelineByRequest(page, pipelineName);
  const stepCreate = await wsCreateStep(page, pipelineName, `step-a-${suffix}`, "short");
  expect(stepCreate.ok).toBe(true);

  await page.goto(`/pipelines/${encodeURIComponent(pipelineName)}`);
  const editor = page.locator("[data-testid='pipeline-step-editor']").first();
  await expect(editor).toBeVisible();

  const ta = editor.locator("textarea[name='prompt']");
  await ta.fill(longDesc);
  await ta.evaluate((el) => {
    const t = el as HTMLTextAreaElement;
    t.scrollTop = t.scrollHeight;
  });
  const scrollBefore = await ta.evaluate(
    (el) => (el as HTMLTextAreaElement).scrollTop,
  );
  expect(scrollBefore).toBeGreaterThan(80);

  await editor.locator("[data-testid='pipeline-step-save']").click();

  await expect
    .poll(
      async () =>
        ta.evaluate((el) => (el as HTMLTextAreaElement).scrollTop),
      { timeout: 12_000 },
    )
    .toBeGreaterThan(scrollBefore * 0.4);
});
