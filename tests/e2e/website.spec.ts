import * as fs from "fs";
import * as path from "path";
import type { Page } from "@playwright/test";
import { expect, test } from "./fixtures";

import type { WsClientOpField } from "./helpers/ui-broadcast-types";
import type { PrimeTestWindow } from "./helpers/prime-window";
import {
  wsAddStepSkill,
  wsCommand,
  wsCreatePipeline,
  wsCreateSkill,
  wsCreateStep,
  wsDeleteStep,
  wsDeleteStepSkill,
  wsReorderStep,
  wsRenamePipeline,
  wsUpdateSkill,
  wsUpdateStep,
} from "./helpers/ws-client";

test.beforeEach(async ({ e2eDataDir }) => {
  try {
    fs.rmSync(path.join(e2eDataDir, "skills"), { recursive: true, force: true });
  } catch {
    /* ignore */
  }
  try {
    fs.rmSync(path.join(e2eDataDir, "pipelines"), { recursive: true, force: true });
  } catch {
    /* ignore */
  }
  try {
    fs.unlinkSync(path.join(e2eDataDir, "counter.json"));
  } catch {
    /* ignore */
  }
});

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

const SKILL_ID_FILE = ".prime-agent-skill-id";

function readSkillUuidFromDisk(e2eDataDir: string, skillName: string): string {
  const p = path.join(e2eDataDir, "skills", skillName, SKILL_ID_FILE);
  return fs.readFileSync(p, "utf8").trim();
}

async function openSkillsTab(page: Page) {
  await page.getByTestId("tab-skills").click();
  await expect(page).toHaveURL(/\/skills(\/[^/]+)?$/);
}

async function openPipelineTab(page: Page) {
  await page.getByTestId("tab-pipeline").click();
  await expect(page).toHaveURL(/\/pipelines(\/[^/]+)?$/);
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

test("GET / renders tabbed shell and defaults to Pipeline view", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.locator("#left-nav")).toBeVisible();
  await expect(page.locator("#main-content")).toBeVisible();
  await expect(page.locator("#left-nav")).toHaveAttribute("style", /width:20%/);
  await expect(page.locator("#main-content")).toHaveAttribute("style", /width:80%/);

  const skillsTab = page.getByTestId("tab-skills");
  const pipelineTab = page.getByTestId("tab-pipeline");
  await expect(skillsTab).toBeVisible();
  await expect(pipelineTab).toBeVisible();
  await expect(skillsTab).toHaveAttribute("title", "Skills");
  await expect(pipelineTab).toHaveAttribute("title", "Pipeline");
  await expect(skillsTab).toHaveAttribute("data-icon", "sword");
  await expect(pipelineTab).toHaveAttribute("data-icon", "pipe");
  await expect(pipelineTab).toHaveAttribute("aria-current", "page");
  await expect(page.locator("#pipeline-main-panel")).toBeVisible();
  await expect(page.locator("#skills-main-panel")).toBeHidden();
  await expect(page.getByTestId("pipeline-create-open")).toBeVisible();
  await expect(page.locator("#pipeline-nav-list")).toHaveCount(0);
});

test("pipeline icon navigates to /pipelines", async ({ page }) => {
  await page.goto("/skills");
  await page.getByTestId("tab-pipeline").click();
  await expect(page).toHaveURL(/\/pipelines$/);
  await expect(page.locator("#pipeline-main-panel")).toBeVisible();
});

test("skills icon navigates to /skills", async ({ page }) => {
  await page.goto("/");
  await page.getByTestId("tab-skills").click();
  await expect(page).toHaveURL(/\/skills$/);
  await expect(page.locator("#skills-main-panel")).toBeVisible();
});

test("skill name input keeps focus after rename autosave and ui broadcast", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-focus-${suffix}`;
  const renamed = `${name}-ren`;

  await createSkillByRequest(page, name, `prompt-${suffix}`);
  await page.goto(`/skills/${encodeURIComponent(name)}`);

  const nameInput = page.locator("[data-skill-editor] input[name='name']");
  await expect(nameInput).toBeVisible();
  await nameInput.click();
  await nameInput.fill(renamed);

  await expect
    .poll(
      async () =>
        nameInput.evaluate((el) => el === document.activeElement),
      { timeout: 15_000 },
    )
    .toBe(true);
});

test("skill prompt textarea keeps focus after autosave and ui broadcast", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-prompt-focus-${suffix}`;
  const initialPrompt = `initial-${suffix}`;

  await createSkillByRequest(page, name, initialPrompt);
  await page.goto(`/skills/${encodeURIComponent(name)}`);

  const promptTa = page.locator(
    "[data-skill-editor] textarea[name='prompt']",
  );
  await expect(promptTa).toBeVisible();
  await promptTa.click();
  await promptTa.fill(`${initialPrompt}\n\nextra line from e2e`);

  await expect
    .poll(
      async () =>
        promptTa.evaluate((el) => el === document.activeElement),
      { timeout: 15_000 },
    )
    .toBe(true);
});

test("skill prompt textarea keeps scroll position after autosave ui broadcast", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-prompt-scroll-${suffix}`;
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

test("pipeline step description keeps scroll position after save ui broadcast", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const pipelineName = `e2e-pipe-scroll-${suffix}`;
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

test("last viewed skill opens when clicking Skills tab after Pipeline tab", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const skillName = `e2e-last-skill-${suffix}`;
  const pipeName = `e2e-last-skill-pipe-${suffix}`;

  await createSkillByRequest(page, skillName, "p1");
  await createPipelineByRequest(page, pipeName);

  await page.goto(`/skills/${encodeURIComponent(skillName)}`);
  await expect(page.locator(`input[name="name"][value="${skillName}"]`)).toBeVisible();

  await page.getByTestId("tab-pipeline").click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${pipeName}$`));

  await page.getByTestId("tab-skills").click();
  await expect(page).toHaveURL(new RegExp(`/skills/${skillName}$`));
  await expect(page.locator(`input[name="name"][value="${skillName}"]`)).toBeVisible();
});

test("last viewed pipeline opens when clicking Pipeline tab after Skills tab", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const skillName = `e2e-last-pipe-sk-${suffix}`;
  const pipeName = `e2e-last-pipe-${suffix}`;

  await createSkillByRequest(page, skillName, "p1");
  await createPipelineByRequest(page, pipeName);

  await page.goto(`/pipelines/${encodeURIComponent(pipeName)}`);
  await expect(page.locator("#pipeline-title")).toHaveValue(pipeName);

  await page.getByTestId("tab-skills").click();
  await expect(page).toHaveURL(/\/skills(\/[^/]+)?$/);

  await page.getByTestId("tab-pipeline").click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${pipeName}$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(pipeName);
});

test("goto /skills redirects to last viewed skill when still present", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-bare-redirect-${suffix}`;

  await createSkillByRequest(page, name, "p");
  await page.goto(`/skills/${encodeURIComponent(name)}`);
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();

  await page.goto("/skills");
  await expect(page).toHaveURL(new RegExp(`/skills/${encodeURIComponent(name)}$`));
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();
});

test("goto /skills does not redirect to deleted last skill", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-stale-last-${suffix}`;

  await createSkillByRequest(page, name, "p");
  await page.goto(`/skills/${encodeURIComponent(name)}`);

  const editor = page.locator("[data-skill-editor]");
  await editor.locator("[data-testid='delete-skill-trigger']").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeVisible();
  await editor.locator("[data-delete-confirm]").click();
  await expect(page).toHaveURL(/\/skills$/);

  await page.goto("/skills");
  await expect(page).toHaveURL(/\/skills$/);
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toHaveCount(
    0,
  );
});

test("create pipeline happy path from left nav plus button", async ({ page, e2eDataDir }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-pipeline-${suffix}`;

  await page.goto("/");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page).toHaveURL(new RegExp(`/pipelines/[a-z0-9-]+$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(name);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: name })).toBeVisible();

  const pj = path.join(e2eDataDir, "pipelines", name, "pipeline.json");
  await expect.poll(() => fs.existsSync(pj)).toBe(true);
});

test("create pipeline from UI shows error and keeps modal open when name is invalid", async ({
  page,
}) => {
  await page.goto("/pipelines");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill("bad name");
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page).toHaveURL(/\/pipelines$/);
  await expect(page.locator("dialog#pipeline-modal")).toBeVisible();
  await expect(page.getByTestId("pipeline-create-error")).toBeVisible();
  await expect(page.getByTestId("pipeline-create-error")).toContainText(
    /name must contain only lowercase letters, digits, and dashes/,
  );
});

test("create pipeline from UI shows error and keeps modal open when name is duplicate", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-dup-${suffix}`;

  await page.goto("/pipelines");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${name}$`));

  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page.locator("dialog#pipeline-modal")).toBeVisible();
  await expect(page.getByTestId("pipeline-create-error")).toBeVisible();
  await expect(page.getByTestId("pipeline-create-error")).toContainText(/pipeline exists/);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: name })).toHaveCount(1);
});

test("pipeline name input lowercases in real time and persists lowercase", async ({ page }) => {
  const suffix = uniqueSuffix();
  const enteredName = `E2E-PIPELINE-MIXED-${suffix}`;
  const normalizedName = enteredName.toLowerCase();

  await page.goto("/pipelines");
  await page.getByTestId("pipeline-create-open").click();
  const input = page.locator("dialog#pipeline-modal input[name='name']");
  await input.fill(enteredName);
  await expect(input).toHaveValue(normalizedName);

  await page.locator("dialog#pipeline-modal button[type='submit']").click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/[a-z0-9-]+$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(normalizedName);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: normalizedName })).toBeVisible();
});

test("pipeline name paste is immediately normalized to lowercase", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pastedName = `PaStEd-PIPELINE-${suffix}`;

  await page.goto("/pipelines");
  await page.getByTestId("pipeline-create-open").click();
  const input = page.locator("dialog#pipeline-modal input[name='name']");
  await input.fill(pastedName);
  await expect(input).toHaveValue(pastedName.toLowerCase());
});

test("backend rejects uppercase pipeline name on insertion", async ({ page }) => {
  const suffix = uniqueSuffix();
  const uppercaseName = `BACKEND-PIPELINE-${suffix}`;
  await page.goto("/pipelines");
  const r = await wsCreatePipeline(page, uppercaseName);
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("name must contain only lowercase letters, digits, and dashes");
});

test("create pipeline unhappy path rejects non-kebab names", async ({ page }) => {
  const invalidNames = ["bad name", "Bad-Name", "bad_name", "   "];
  await page.goto("/pipelines");
  for (const name of invalidNames) {
    const r = await wsCreatePipeline(page, name);
    expect(r.ok).toBe(false);
    expect(r.error ?? "").toContain("name must contain only lowercase letters, digits, and dashes");
  }
});

test("pipeline list renders each pipeline as clickable nav item and routes to /pipelines/:id", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-pipeline-first-${suffix}`;
  const second = `e2e-pipeline-second-${suffix}`;

  await createPipelineByRequest(page, first);
  const locTwo = (await wsCreatePipeline(page, second)).location ?? "";
  const secondId = locTwo.split("/").pop() ?? "";

  await page.goto("/pipelines");
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: first })).toBeVisible();
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: second })).toBeVisible();

  await page.getByTestId("pipeline-nav-link").filter({ hasText: second }).click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${secondId}$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(second);
});

test("create skill happy path from Skills tab", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-create-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await page.goto("/skills");
  await page.getByTestId("skill-create-open").click();
  await page.locator("dialog#skill-modal input[name='name']").fill(name);
  await page.locator("dialog#skill-modal textarea[name='prompt']").fill(prompt);
  await page.locator("dialog#skill-modal button[type='submit']").click();

  await expect(page).toHaveURL(new RegExp(`/skills/[a-z0-9-]+$`));
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(prompt);
});

test("create skill rejects empty name after normalization", async ({ page }) => {
  await page.goto("/skills");
  const r = await wsCreateSkill(page, "   ", "some-prompt");
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("name is required");
});

test("create skill normalizes messy names to kebab", async ({ page }) => {
  const suffix = uniqueSuffix();
  await page.goto("/skills");
  const r = await wsCreateSkill(page, `Bad_Name_${suffix}`, "some-prompt");
  expect(r.ok).toBe(true);
  expect(r.location ?? "").toContain(`bad-name-${suffix}`);
});

test("skill update normalizes rename to kebab and allows further kebab rename", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-update-${suffix}`;
  const validRenamed = `e2e-renamed-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await createSkillByRequest(page, originalName, prompt);

  const normalizedRename = await wsUpdateSkill(page, originalName, "Legacy_Name", prompt);
  expect(normalizedRename.ok).toBe(true);
  expect(normalizedRename.location ?? "").toContain("/skills/legacy-name");

  const validRename = await wsUpdateSkill(page, "legacy-name", validRenamed, `${prompt}-updated`);
  expect(validRename.ok).toBe(true);
});

test("autosave update happy path persists edits while typing", async ({ page }) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-update-old-${suffix}`;
  const updatedName = `e2e-update-new-${suffix}`;
  const originalPrompt = `old-prompt-${suffix}`;
  const updatedPrompt = `new-prompt-${suffix}`;

  await page.addInitScript(() => {
    const w = window as PrimeTestWindow;
    if (w.__primeWsSendPatched) {
      return;
    }
    w.__primeWsSendPatched = true;
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

  await createSkillByRequest(page, originalName, originalPrompt);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  await expect(page).toHaveURL(/\/skills\/[a-z0-9-]+$/);

  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").click();
  await editor.locator("input[name='name']").fill("");
  await editor.locator("input[name='name']").type(updatedName);
  await editor.locator("textarea[name='prompt']").click();
  await editor.locator("textarea[name='prompt']").fill("");
  await editor.locator("textarea[name='prompt']").type(updatedPrompt);
  await expect
    .poll(
      async () =>
        (await page.evaluate(() => {
          const win = window as PrimeTestWindow;
          return win.__primeWsUpdate ?? 0;
        })) >= 1,
      { timeout: 5000 },
    )
    .toBe(true);

  await page.reload();
  await expect(page.locator(`input[name="name"][value="${updatedName}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(updatedPrompt);
});

test("skill autosave requests are at least one second apart", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-interval-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await page.addInitScript(() => {
    const w = window as PrimeTestWindow;
    if (w.__primeWsSendPatchedInterval) {
      return;
    }
    w.__primeWsSendPatchedInterval = true;
    w.__primeWsUpdateTimes = [];
    const S = WebSocket.prototype.send;
    WebSocket.prototype.send = function (this: WebSocket, data: Parameters<WebSocket["send"]>[0]) {
      try {
        const j = JSON.parse(String(data)) as WsClientOpField;
        if (j.op === "update_skill") {
          w.__primeWsUpdateTimes!.push(Date.now());
        }
      } catch {
        /* ignore */
      }
      return S.call(this, data);
    };
  });

  await createSkillByRequest(page, name, prompt);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("textarea[name='prompt']").fill(`${prompt}-a`);
  await expect
    .poll(
      async () =>
        (await page.evaluate(() => {
          const win = window as PrimeTestWindow;
          return win.__primeWsUpdateTimes?.length ?? 0;
        })) >= 1,
      { timeout: 5000 },
    )
    .toBe(true);
  const first = await page.evaluate(() => {
    const win = window as PrimeTestWindow;
    return win.__primeWsUpdateTimes![0];
  });
  await editor.locator("textarea[name='prompt']").fill(`${prompt}-b`);
  await expect
    .poll(
      async () =>
        (await page.evaluate(() => {
          const win = window as PrimeTestWindow;
          return win.__primeWsUpdateTimes?.length ?? 0;
        })) >= 2,
      { timeout: 5000 },
    )
    .toBe(true);
  const second = await page.evaluate(() => {
    const win = window as PrimeTestWindow;
    return win.__primeWsUpdateTimes![1];
  });
  expect(second - first).toBeGreaterThanOrEqual(900);
});

test("autosave update unhappy path returns 404 for missing skill", async ({ page }) => {
  await page.goto("/skills");
  const r = await wsUpdateSkill(page, "does-not-exist-skill-zzzz", "x", "y");
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("delete skill happy path removes row from page", async ({ page, e2eDataDir }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-delete-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await createSkillByRequest(page, name, prompt);

  const skillDir = path.join(e2eDataDir, "skills", name);
  expect(fs.existsSync(path.join(skillDir, "SKILL.md"))).toBe(true);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page).toHaveURL(/\/skills\/[a-z0-9-]+$/);

  const editor = page.locator("[data-skill-editor]");
  await editor.locator("[data-testid='delete-skill-trigger']").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeVisible();
  await expect(editor.locator("[data-testid='delete-skill-warning']")).toContainText(
    "permanent",
  );
  await editor.locator("[data-delete-confirm]").click();
  await expect(page).toHaveURL(/\/skills$/);
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toHaveCount(0);
  expect(fs.existsSync(skillDir)).toBe(false);
});

test("delete popover closes on outside click without deleting", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-delete-cancel-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await createSkillByRequest(page, name, prompt);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page).toHaveURL(/\/skills\/[a-z0-9-]+$/);

  const editor = page.locator("[data-skill-editor]");
  await editor.locator("[data-testid='delete-skill-trigger']").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeVisible();
  await page.locator("#skills-main-panel h1").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeHidden();
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toBeVisible();
});

test("delete skill unhappy path returns 404 for missing skill", async ({ page }) => {
  await page.goto("/skills");
  const r = await wsCommand(page, {
    op: "delete_skill",
    id: `del-${Date.now()}`,
    name: "does-not-exist-skill-zzzz",
  });
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("method guards and unknown skill action are enforced", async ({ request }) => {
  const postRoot = await request.post("/");
  expect(postRoot.status()).toBe(405);

  const postFragment = await request.post("/fragments/counter");
  expect(postFragment.status()).toBe(405);

  const putPipelines = await request.put("/pipelines");
  expect(putPipelines.status()).toBe(405);

  const deletePipeline = await request.delete("/pipelines/1");
  expect(deletePipeline.status()).toBe(405);

  const putSkillMutation = await request.put("/skills/1/update");
  expect(putSkillMutation.status()).toBe(404);

  const unknownAction = await request.post("/skills/1/nope");
  expect(unknownAction.status()).toBe(404);
});

test("route navigation switches both left nav and main content", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("#pipeline-nav-panel")).toBeVisible();
  await expect(page.locator("#pipeline-main-panel")).toBeVisible();
  await expect(page.locator("#skills-main-panel")).toBeHidden();

  await openSkillsTab(page);
  await expect(page.locator("#skills-nav-panel")).toBeVisible();
  await expect(page.locator("#skills-main-panel")).toBeVisible();
  await expect(page.locator("#pipeline-main-panel")).toBeHidden();

  await openPipelineTab(page);
  await expect(page.locator("#pipeline-nav-panel")).toBeVisible();
  await expect(page.locator("#pipeline-main-panel")).toBeVisible();
});

test("skills list renders each skill as clickable nav item and routes to /skills/:id", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-skill-first-${suffix}`;
  const second = `e2e-skill-second-${suffix}`;

  await createSkillByRequest(page, first, "prompt one");
  const locTwo = (await wsCreateSkill(page, second, "prompt two")).location ?? "";
  const secondId = locTwo.split("/").pop() ?? "";

  await page.goto("/skills");
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: first })).toBeVisible();
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: second })).toBeVisible();

  await page.getByTestId("skill-nav-link").filter({ hasText: second }).click();
  await expect(page).toHaveURL(new RegExp(`/skills/${secondId}$`));
  await expect(page.locator(`input[name="name"][value="${second}"]`)).toBeVisible();
});

test("main area title updates to currently selected pipeline from /pipelines/:id", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-pipeline-title-first-${suffix}`;
  const second = `e2e-pipeline-title-second-${suffix}`;

  await createPipelineByRequest(page, first);
  await wsCreatePipeline(page, second);

  await page.goto("/pipelines");
  await page.getByTestId("pipeline-nav-link").filter({ hasText: first }).click();
  await expect(page.locator("#pipeline-title")).toHaveValue(first);

  await page.getByTestId("pipeline-nav-link").filter({ hasText: second }).click();
  await expect(page.locator("#pipeline-title")).toHaveValue(second);
});

test("pipeline rename via WS updates URL, nav, and on-disk folder", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-ren-old-${suffix}`;
  const newName = `pipe-ren-new-${suffix}`;
  await createPipelineByRequest(page, oldName);
  const r = await wsRenamePipeline(page, oldName, newName);
  expect(r.ok).toBe(true);
  expect(r.location).toBe(`/pipelines/${encodeURIComponent(newName)}`);

  await expect(page).toHaveURL(new RegExp(`/pipelines/${newName}$`));
  await expect(page.locator("#pipeline-title")).toHaveValue(newName);
  await expect(
    page.getByTestId("pipeline-nav-link").filter({ hasText: newName }),
  ).toBeVisible();

  const newPath = path.join(e2eDataDir, "pipelines", newName, "pipeline.json");
  const oldPath = path.join(e2eDataDir, "pipelines", oldName, "pipeline.json");
  expect(fs.existsSync(newPath)).toBe(true);
  expect(fs.existsSync(oldPath)).toBe(false);
});

test("pipeline rename submit with empty name does not navigate or error", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-empty-submit-${suffix}`;
  await createPipelineByRequest(page, oldName);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await titleInput.fill("");
  await titleInput.press("Enter");
  await expect(page).toHaveURL(new RegExp(`/pipelines/${oldName}$`));
  await expect(titleInput).toHaveValue("");
});

test("pipeline rename unhappy path rejects invalid kebab name", async ({ page }) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-bad-old-${suffix}`;
  await createPipelineByRequest(page, oldName);
  const r = await wsRenamePipeline(page, oldName, "Bad_Name");
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain(
    "name must contain only lowercase letters, digits, and dashes",
  );
});

test("pipeline rename unhappy path rejects duplicate target name", async ({ page }) => {
  const suffix = uniqueSuffix();
  const a = `pipe-dup-a-${suffix}`;
  const b = `pipe-dup-b-${suffix}`;
  await createPipelineByRequest(page, a);
  const r2 = await wsCreatePipeline(page, b);
  expect(r2.ok).toBe(true);
  const r = await wsRenamePipeline(page, a, b);
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("pipeline exists");
});

test("pipeline name autosaves from title input without clicking Rename", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-ui-auto-${suffix}`;
  const newName = `pipe-ui-auto-ren-${suffix}`;
  await createPipelineByRequest(page, oldName);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await expect(titleInput).toBeVisible();
  await titleInput.click();
  await titleInput.fill(newName);

  await expect(page).toHaveURL(new RegExp(`/pipelines/${newName}$`), { timeout: 15_000 });
  await expect(
    page.getByTestId("pipeline-nav-link").filter({ hasText: newName }),
  ).toBeVisible();

  const newPath = path.join(e2eDataDir, "pipelines", newName, "pipeline.json");
  const oldPath = path.join(e2eDataDir, "pipelines", oldName, "pipeline.json");
  await expect.poll(() => fs.existsSync(newPath) && !fs.existsSync(oldPath)).toBe(true);
});

test("pipeline title can be cleared without saving then autosave rename", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-clear-${suffix}`;
  const newName = `pipe-clear-ren-${suffix}`;
  await createPipelineByRequest(page, oldName);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await titleInput.fill("");
  await expect(titleInput).toHaveValue("");

  const oldPath = path.join(e2eDataDir, "pipelines", oldName, "pipeline.json");
  await page.waitForTimeout(2200);
  expect(fs.existsSync(oldPath)).toBe(true);

  await titleInput.fill(newName);
  await expect(page).toHaveURL(new RegExp(`/pipelines/${newName}$`), { timeout: 15_000 });
  expect(fs.existsSync(path.join(e2eDataDir, "pipelines", newName, "pipeline.json"))).toBe(
    true,
  );
  expect(fs.existsSync(oldPath)).toBe(false);
});

test("pipeline title input keeps focus after rename autosave and ui broadcast", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `pipe-focus-${suffix}`;
  const newName = `pipe-focus-ren-${suffix}`;
  await createPipelineByRequest(page, oldName);
  await page.goto(`/pipelines/${encodeURIComponent(oldName)}`);

  const titleInput = page.locator("#pipeline-title");
  await titleInput.click();
  await titleInput.fill(newName);

  await expect
    .poll(
      async () => titleInput.evaluate((el) => el === document.activeElement),
      { timeout: 15_000 },
    )
    .toBe(true);
});

test("skills detail main area renders from /skills/:id", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-skill-detail-${suffix}`;
  const prompt = `prompt-${suffix}`;

  await createSkillByRequest(page, name, prompt);
  const loc = `/skills/${encodeURIComponent(name)}`;

  await page.goto(loc);
  await expect(page).toHaveURL(/\/skills\/[a-z0-9-]+$/);
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(prompt);
});

test("autosave retries after one second when backend fails once", async ({ page }) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-retry-timer-${suffix}`;
  const prompt = `prompt-${suffix}`;
  await createSkillByRequest(page, originalName, prompt);

  await page.addInitScript(() => {
    const Orig = WebSocket.prototype.send;
    let dropped = 0;
    WebSocket.prototype.send = function (this: WebSocket, data: Parameters<WebSocket["send"]>[0]) {
      if (typeof data === "string" && data.includes('"op":"update_skill"')) {
        const win = window as PrimeTestWindow;
        win.__primeUpdateSkillSends = win.__primeUpdateSkillSends ?? [];
        win.__primeUpdateSkillSends.push(Date.now());
        if (dropped === 0) {
          dropped += 1;
          return;
        }
      }
      return Orig.call(this, data);
    };
  });

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").type("x");
  await expect
    .poll(async () => {
      const n = await page.evaluate(() => {
        const win = window as PrimeTestWindow;
        return win.__primeUpdateSkillSends?.length ?? 0;
      });
      return n;
    }, { timeout: 8000 })
    .toBeGreaterThanOrEqual(2);
  const times = await page.evaluate(() => {
    const win = window as PrimeTestWindow;
    const sends = win.__primeUpdateSkillSends;
    if (sends === undefined) {
      throw new Error("missing __primeUpdateSkillSends");
    }
    return sends;
  });
  expect(times[1]! - times[0]!).toBeGreaterThanOrEqual(900);
});

test("autosave retries on next interval after failure without extra keypress", async ({ page }) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-retry-key-${suffix}`;
  const prompt = `prompt-${suffix}`;
  await createSkillByRequest(page, originalName, prompt);

  await page.addInitScript(() => {
    const Orig = WebSocket.prototype.send;
    let dropped = 0;
    WebSocket.prototype.send = function (this: WebSocket, data: Parameters<WebSocket["send"]>[0]) {
      if (typeof data === "string" && data.includes('"op":"update_skill"')) {
        const win = window as PrimeTestWindow;
        win.__primeUpdateSkillSends = win.__primeUpdateSkillSends ?? [];
        win.__primeUpdateSkillSends.push(Date.now());
        if (dropped === 0) {
          dropped += 1;
          return;
        }
      }
      return Orig.call(this, data);
    };
  });

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").type("x");
  await expect
    .poll(async () => {
      const n = await page.evaluate(() => {
        const win = window as PrimeTestWindow;
        return win.__primeUpdateSkillSends?.length ?? 0;
      });
      return n;
    }, { timeout: 8000 })
    .toBeGreaterThanOrEqual(2);
  const times = await page.evaluate(() => {
    const win = window as PrimeTestWindow;
    const sends = win.__primeUpdateSkillSends;
    if (sends === undefined) {
      throw new Error("missing __primeUpdateSkillSends");
    }
    return sends;
  });
  expect(times[1]! - times[0]!).toBeGreaterThanOrEqual(900);
});

test("create pipeline step happy path with title and prompt", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-create-${suffix}`);
  const title = `step-title-${suffix}`;
  const prompt = `step-prompt-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  await page.getByTestId("pipeline-step-create-open").click();
  await page.locator("dialog#pipeline-step-modal input[name='title']").fill(title);
  await page.locator("dialog#pipeline-step-modal textarea[name='prompt']").fill(prompt);
  await page.locator("dialog#pipeline-step-modal button[type='submit']").click();

  await expect(page.getByTestId("pipeline-step-nav-item").filter({ hasText: title })).toBeVisible();
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(title);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(prompt);
});

test("pipeline step title input lowercases in real time and persists lowercase", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-lowercase-${suffix}`);
  const enteredTitle = `MiXeD-STEP-${suffix}`;
  const normalizedTitle = enteredTitle.toLowerCase();
  const prompt = `step-prompt-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  await page.getByTestId("pipeline-step-create-open").click();
  const input = page.locator("dialog#pipeline-step-modal input[name='title']");
  await input.fill(enteredTitle);
  await expect(input).toHaveValue(normalizedTitle);
  await page.locator("dialog#pipeline-step-modal textarea[name='prompt']").fill(prompt);
  await page.locator("dialog#pipeline-step-modal button[type='submit']").click();

  await expect(page.getByTestId("pipeline-step-nav-item").filter({ hasText: normalizedTitle })).toBeVisible();
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(normalizedTitle);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(prompt);
});

test("pipeline step title paste is immediately normalized to lowercase", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-lowercase-paste-${suffix}`);
  const pastedTitle = `PaStEd-STEP-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  await page.getByTestId("pipeline-step-create-open").click();
  const input = page.locator("dialog#pipeline-step-modal input[name='title']");
  await input.fill(pastedTitle);
  await expect(input).toHaveValue(pastedTitle.toLowerCase());
});

test("backend normalizes uppercase pipeline step title on insertion", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-backend-lower-${suffix}`);
  const uppercaseTitle = `BACKEND-STEP-${suffix}`;
  const r = await wsCreateStep(page, pipelineID, uppercaseTitle, "prompt");
  expect(r.ok).toBe(true);
  const normalizedTitle = uppercaseTitle.toLowerCase();

  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-nav-item").filter({ hasText: normalizedTitle })).toBeVisible();
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(normalizedTitle);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue("prompt");
});

test("create pipeline step unhappy path rejects empty title", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-invalid-${suffix}`);

  const emptyTitle = await wsCreateStep(page, pipelineID, "   ", "valid prompt");
  expect(emptyTitle.ok).toBe(false);
  expect(emptyTitle.error ?? "").toContain("required");
});

test("create pipeline step persists empty description", async ({ page, e2eDataDir }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-empty-desc-${suffix}`);
  const title = `step-empty-${suffix}`;
  const r = await wsCreateStep(page, pipelineID, title, "");
  expect(r.ok).toBe(true);

  const pj = path.join(e2eDataDir, "pipelines", pipelineID, "pipeline.json");
  const j = JSON.parse(fs.readFileSync(pj, "utf8")) as { steps: { title: string; prompt: string }[] };
  expect(j.steps[0]?.prompt).toBe("");

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue("");
});

test("edit pipeline step happy path persists updated title and prompt", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-edit-${suffix}`);
  const originalTitle = `step-edit-original-${suffix}`;
  const originalPrompt = `step-edit-prompt-${suffix}`;
  const createStep = await wsCreateStep(page, pipelineID, originalTitle, originalPrompt);
  expect(createStep.ok).toBe(true);

  const updatedTitle = `step-edit-updated-${suffix}`;
  const updatedPrompt = `step-edit-updated-prompt-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor).toBeVisible();
  await editor.locator("input[name='title']").fill(updatedTitle);
  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);

  await expect(page).toHaveURL(new RegExp(`/pipelines/${pipelineID}$`));
  await expect(
    page.getByTestId("pipeline-step-nav-item").filter({ hasText: updatedTitle.toLowerCase() }),
  ).toBeVisible({ timeout: 12_000 });
  await expect(editor.locator("input[name='title']")).toHaveValue(updatedTitle.toLowerCase());
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(updatedPrompt);

  await page.reload();
  const reloadedEditor = page.getByTestId("pipeline-step-editor").first();
  await expect(reloadedEditor.locator("input[name='title']")).toHaveValue(updatedTitle.toLowerCase());
  await expect(reloadedEditor.locator("textarea[name='prompt']")).toHaveValue(updatedPrompt);
});

test("pipeline step title and prompt autosave without clicking Save", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-ui-auto-${suffix}`);
  const createStep = await wsCreateStep(
    page,
    pipelineID,
    `step-ui-auto-a-${suffix}`,
    `prompt-a-${suffix}`,
  );
  expect(createStep.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor).toBeVisible();

  const newTitle = `step-ui-auto-b-${suffix}`;
  const newPrompt = `prompt-b-${suffix}`;
  await editor.locator("input[name='title']").fill(newTitle);
  await editor.locator("textarea[name='prompt']").fill(newPrompt);

  await expect(
    page.getByTestId("pipeline-step-nav-item").filter({ hasText: newTitle.toLowerCase() }),
  ).toBeVisible({ timeout: 12_000 });

  await page.reload();
  const reloadedEditor = page.getByTestId("pipeline-step-editor").first();
  await expect(reloadedEditor.locator("input[name='title']")).toHaveValue(newTitle.toLowerCase());
  await expect(reloadedEditor.locator("textarea[name='prompt']")).toHaveValue(newPrompt);
});

test("pipeline step title can be cleared without saving then autosave new title", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-clear-ui-${suffix}`);
  const origTitle = `step-clear-orig-${suffix}`;
  const createStep = await wsCreateStep(page, pipelineID, origTitle, `prompt-${suffix}`);
  expect(createStep.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  const titleInput = editor.locator("input[name='title']");
  await titleInput.fill("");
  await expect(titleInput).toHaveValue("");

  const pipePath = path.join(e2eDataDir, "pipelines", pipelineID, "pipeline.json");
  const readStepTitle = () => {
    const j = JSON.parse(fs.readFileSync(pipePath, "utf8")) as { steps: { title: string }[] };
    return j.steps[0]?.title ?? "";
  };

  await page.waitForTimeout(2200);
  expect(readStepTitle()).toBe(origTitle.toLowerCase());

  const newTitle = `step-clear-ren-${suffix}`;
  await titleInput.fill(newTitle);
  await expect(
    page.getByTestId("pipeline-step-nav-item").filter({ hasText: newTitle.toLowerCase() }),
  ).toBeVisible({ timeout: 12_000 });
  expect(readStepTitle()).toBe(newTitle.toLowerCase());
});

test("pipeline step title input keeps focus after autosave ui broadcast", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-title-focus-${suffix}`);
  const createStep = await wsCreateStep(
    page,
    pipelineID,
    `step-title-focus-a-${suffix}`,
    `p-${suffix}`,
  );
  expect(createStep.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  const titleInput = editor.locator("input[name='title']");
  await titleInput.click();
  const newTitle = `step-title-focus-b-${suffix}`;
  await titleInput.fill(newTitle);

  await expect
    .poll(
      async () => titleInput.evaluate((el) => el === document.activeElement),
      { timeout: 12_000 },
    )
    .toBe(true);
});

test("edit pipeline step unhappy path rejects empty title and preserves previous values", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-edit-invalid-${suffix}`);
  const originalTitle = `step-edit-keep-${suffix}`;
  const originalPrompt = `step-edit-keep-prompt-${suffix}`;
  const createStep = await wsCreateStep(page, pipelineID, originalTitle, originalPrompt);
  expect(createStep.ok).toBe(true);
  const loc = createStep.location ?? "";
  const stepID = Number(loc.split("/").pop() ?? "0");
  expect(Number.isFinite(stepID)).toBe(true);

  const emptyTitle = await wsUpdateStep(page, pipelineID, stepID, "   ", "valid prompt");
  expect(emptyTitle.ok).toBe(false);
  expect(emptyTitle.error ?? "").toContain("required");

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(originalTitle);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(originalPrompt);
});

test("edit pipeline step clears description to empty string", async ({ page, e2eDataDir }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-clear-prompt-${suffix}`);
  const originalTitle = `step-clear-prompt-title-${suffix}`;
  const originalPrompt = `step-clear-prompt-body-${suffix}`;
  const createStep = await wsCreateStep(page, pipelineID, originalTitle, originalPrompt);
  expect(createStep.ok).toBe(true);
  const loc = createStep.location ?? "";
  const stepID = Number(loc.split("/").pop() ?? "0");
  expect(Number.isFinite(stepID)).toBe(true);

  const cleared = await wsUpdateStep(page, pipelineID, stepID, originalTitle, "");
  expect(cleared.ok).toBe(true);

  const pj = path.join(e2eDataDir, "pipelines", pipelineID, "pipeline.json");
  const j = JSON.parse(fs.readFileSync(pj, "utf8")) as { steps: { title: string; prompt: string }[] };
  expect(j.steps[0]?.prompt).toBe("");

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue("");
});

test("delete pipeline step happy path removes step from main panel and left nav", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-delete-${suffix}`);
  const stepCreate = await wsCreateStep(page, pipelineID, `step-delete-title-${suffix}`, "prompt");
  expect(stepCreate.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const stepItem = page.getByTestId("pipeline-step-nav-item");
  await expect(stepItem).toHaveCount(1);
  await page.getByTestId("pipeline-step-delete").click();

  await expect(page.getByTestId("pipeline-step-nav-item")).toHaveCount(0);
  await expect(page.getByTestId("pipeline-step-editor")).toHaveCount(0);
});

test("delete pipeline step unhappy path returns 404 for missing step", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-delete-missing-${suffix}`);

  const r = await wsDeleteStep(page, pipelineID, 999999);
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("pipeline step left-nav drag swap persists after refresh", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-reorder-${suffix}`);
  expect((await wsCreateStep(page, pipelineID, `first-${suffix}`, "prompt-a")).ok).toBe(true);
  expect((await wsCreateStep(page, pipelineID, `second-${suffix}`, "prompt-b")).ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const items = page.locator(
    `[data-testid="pipeline-step-nav-item"][data-ws-pipeline="${pipelineID}"]`,
  );
  await expect(items).toHaveCount(2);
  await items.nth(1).dragTo(items.nth(0));

  await expect(items.nth(0)).toContainText(`second-${suffix}`);
  await page.reload();
  await expect(
    page.locator(
      `[data-testid="pipeline-step-nav-item"][data-ws-pipeline="${pipelineID}"]`,
    ).nth(0),
  ).toContainText(`second-${suffix}`);
});

test("pipeline step reorder unhappy path rejects invalid step target", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-reorder-invalid-${suffix}`);
  const response = await wsReorderStep(page, pipelineID, 999999, 888888);
  expect(response.ok).toBe(false);
  expect(response.error ?? "").toContain("not found");
});

test("pipeline step skill association happy path increments per-step count", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-skill-count-${suffix}`);
  const skillA = await createSkillByRequest(page, `skill-a-${suffix}`, "a prompt");
  const skillB = await createSkillByRequest(page, `skill-b-${suffix}`, "b prompt");
  const uuidA = readSkillUuidFromDisk(e2eDataDir, skillA);
  const uuidB = readSkillUuidFromDisk(e2eDataDir, skillB);
  const createStep = await wsCreateStep(page, pipelineID, `skill-step-${suffix}`, "prompt");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");

  const addA = await wsAddStepSkill(page, pipelineID, stepID, uuidA);
  expect(addA.ok).toBe(true);
  const addB = await wsAddStepSkill(page, pipelineID, stepID, uuidB);
  expect(addB.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-skill-count").filter({ hasText: "2" })).toBeVisible();
});

test("pipeline step skill association unhappy path rejects duplicate skill per step", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-skill-dup-${suffix}`);
  const skillName = await createSkillByRequest(page, `skill-dup-${suffix}`, "dup prompt");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, skillName);
  const createStep = await wsCreateStep(page, pipelineID, `dup-step-${suffix}`, "prompt");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");

  const first = await wsAddStepSkill(page, pipelineID, stepID, skillUuid);
  expect(first.ok).toBe(true);

  const duplicate = await wsAddStepSkill(page, pipelineID, stepID, skillUuid);
  expect(duplicate.ok).toBe(false);
  expect(duplicate.error ?? "").toContain("already attached");
});

test("pipeline step rows render accurate skill counts", async ({ page, e2eDataDir }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-count-render-${suffix}`);
  const skillName = await createSkillByRequest(page, `count-skill-${suffix}`, "count prompt");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, skillName);

  const createOne = await wsCreateStep(page, pipelineID, `count-one-${suffix}`, "prompt-1");
  expect(createOne.ok).toBe(true);
  expect((await wsCreateStep(page, pipelineID, `count-two-${suffix}`, "prompt-2")).ok).toBe(true);
  const stepOneID = Number((createOne.location ?? "").split("/").pop() ?? "0");

  const linkSkill = await wsAddStepSkill(page, pipelineID, stepOneID, skillUuid);
  expect(linkSkill.ok).toBe(true);

  await page.goto(`/pipelines/${pipelineID}`);
  const rows = page.getByTestId("pipeline-step-nav-item");
  await expect(rows.filter({ hasText: `count-one-${suffix}` }).getByTestId("pipeline-step-skill-count")).toHaveText("1");
  await expect(rows.filter({ hasText: `count-two-${suffix}` }).getByTestId("pipeline-step-skill-count")).toHaveText("0");
});

test("pipeline step skill count updates after add and remove skill", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `step-count-update-${suffix}`);
  const skillName = await createSkillByRequest(page, `update-skill-${suffix}`, "update prompt");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, skillName);
  const createStep = await wsCreateStep(page, pipelineID, `update-step-${suffix}`, "prompt");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");

  const add = await wsAddStepSkill(page, pipelineID, stepID, skillUuid);
  expect(add.ok).toBe(true);
  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-skill-count")).toContainText("1");

  const remove = await wsDeleteStepSkill(page, pipelineID, stepID, skillUuid);
  expect(remove.ok).toBe(true);
  await page.reload();
  await expect(page.getByTestId("pipeline-step-skill-count")).toContainText("0");
});

test("create skill unhappy path rejects duplicate name", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-dup-skill-${suffix}`;
  await page.goto("/");
  expect((await wsCreateSkill(page, name, "first")).ok).toBe(true);
  const second = await wsCreateSkill(page, name, "second");
  expect(second.ok).toBe(false);
  expect(second.error ?? "").toContain("skill already exists");
});

test("create skill unhappy path rejects empty prompt", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-empty-prompt-${suffix}`;
  await page.goto("/skills");
  const response = await wsCreateSkill(page, name, "");
  expect(response.ok).toBe(false);
  expect(response.error ?? "").toContain("prompt is required");
});

test("skill prompt preserves leading and trailing whitespace on disk and in UI", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-ws-prompt-${suffix}`;
  /* A lone leading newline is dropped by HTML textarea parsing; use spaces + trailing ws to exercise no-trim. */
  const promptWithWs = `  ${suffix}-body  \n`;
  await page.goto("/");
  expect((await wsCreateSkill(page, name, promptWithWs)).ok).toBe(true);

  const skillPath = path.join(e2eDataDir, "skills", name, "SKILL.md");
  await expect
    .poll(() => fs.readFileSync(skillPath, "utf8"), { timeout: 8000 })
    .toBe(promptWithWs);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(promptWithWs);
});

test("update skill unhappy path rejects rename to existing skill name", async ({ page }) => {
  const suffix = uniqueSuffix();
  const a = `e2e-collision-a-${suffix}`;
  const b = `e2e-collision-b-${suffix}`;
  await page.goto("/");
  await wsCreateSkill(page, a, "a");
  await wsCreateSkill(page, b, "b");
  const collision = await wsUpdateSkill(page, b, a, "b-updated");
  expect(collision.ok).toBe(false);
  expect(collision.error ?? "").toContain("skill already exists");
});

test("GET missing skill slug returns 404", async ({ request }) => {
  const response = await request.get("/skills/does-not-exist-skill-zzzz");
  expect(response.status()).toBe(404);
});

test("GET missing pipeline name returns 404", async ({ request }) => {
  const response = await request.get("/pipelines/does-not-exist-pipeline-zzzz");
  expect(response.status()).toBe(404);
});

test("GET missing pipeline step id returns 404", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `missing-step-${suffix}`);
  const response = await request.get(`/pipelines/${pipelineID}/steps/999999`);
  expect(response.status()).toBe(404);
});

test("create pipeline unhappy path rejects duplicate name", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-dup-pipeline-${suffix}`;
  await page.goto("/");
  expect((await wsCreatePipeline(page, name)).ok).toBe(true);
  const second = await wsCreatePipeline(page, name);
  expect(second.ok).toBe(false);
  expect(second.error ?? "").toContain("pipeline exists");
});

test("add pipeline step skill unhappy path rejects empty skill_id", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `empty-skill-id-${suffix}`);
  const createStep = await wsCreateStep(page, pipelineID, `step-${suffix}`, "p");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");
  const response = await wsAddStepSkill(page, pipelineID, stepID, "   ");
  expect(response.ok).toBe(false);
  expect(response.error ?? "").toContain("skill_id is required");
});

test("add pipeline step skill unhappy path returns 404 for unknown skill", async ({ page }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `unknown-skill-${suffix}`);
  const createStep = await wsCreateStep(page, pipelineID, `step-${suffix}`, "p");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");
  const response = await wsAddStepSkill(
    page,
    pipelineID,
    stepID,
    "00000000-0000-0000-0000-000000000099",
  );
  expect(response.ok).toBe(false);
  expect(response.error ?? "").toContain("not found");
});

test("pipeline step reorder unhappy path rejects non-numeric target_step_id", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `reorder-bad-parse-${suffix}`);
  const corr = `reorder-bad-${suffix}`;
  const response = await wsCommand(page, {
    op: "reorder_step",
    id: corr,
    pipeline: pipelineID,
    step_id: 1,
    target_step_id: "not-a-number",
  });
  expect(response.ok).toBe(false);
  expect(response.error ?? "").toContain("invalid message");
});

test("create pipeline step unhappy path returns not found for missing pipeline", async ({
  page,
}) => {
  await page.goto("/pipelines");
  const r = await wsCreateStep(page, `no-such-pipeline-${uniqueSuffix()}`, "title", "prompt");
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("delete pipeline step skill unhappy path returns not found for missing step", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `del-skill-step-${suffix}`);
  const skillName = await createSkillByRequest(page, `del-skill-s-${suffix}`, "p");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, skillName);
  const r = await wsDeleteStepSkill(page, pipelineID, 999999, skillUuid);
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("delete pipeline step skill unhappy path returns not found when skill not linked", async ({
  page,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(page, `del-skill-nolink-${suffix}`);
  const createStep = await wsCreateStep(page, pipelineID, `step-${suffix}`, "p");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");
  const r = await wsDeleteStepSkill(
    page,
    pipelineID,
    stepID,
    "00000000-0000-0000-0000-000000000088",
  );
  expect(r.ok).toBe(false);
  expect(r.error ?? "").toContain("not found");
});

test("skill rename updates pipeline.json step skill references on disk", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const oldName = `rename-old-${suffix}`;
  const newName = `rename-new-${suffix}`;
  const pipelineID = await createPipelineByRequest(page, `rename-pipe-${suffix}`);
  await createSkillByRequest(page, oldName, "prompt");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, oldName);
  const createStep = await wsCreateStep(page, pipelineID, `st-${suffix}`, "p");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");
  expect((await wsAddStepSkill(page, pipelineID, stepID, skillUuid)).ok).toBe(true);

  const rename = await wsUpdateSkill(page, oldName, newName, "updated");
  expect(rename.ok).toBe(true);

  const pj = path.join(e2eDataDir, "pipelines", pipelineID, "pipeline.json");
  const raw = fs.readFileSync(pj, "utf8");
  const j = JSON.parse(raw) as { steps: Array<{ skills: Array<{ alias: string }> }> };
  const skills = j.steps[0]?.skills ?? [];
  expect(skills.some((s) => s.alias === newName)).toBe(true);
  expect(skills.some((s) => s.alias === oldName)).toBe(false);
});

test("skill delete removes skill from pipeline.json step lists on disk", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const skillName = `del-ref-skill-${suffix}`;
  const pipelineID = await createPipelineByRequest(page, `del-ref-pipe-${suffix}`);
  await createSkillByRequest(page, skillName, "prompt");
  const skillUuid = readSkillUuidFromDisk(e2eDataDir, skillName);
  const createStep = await wsCreateStep(page, pipelineID, `st-${suffix}`, "p");
  expect(createStep.ok).toBe(true);
  const stepID = Number((createStep.location ?? "").split("/").pop() ?? "0");
  expect((await wsAddStepSkill(page, pipelineID, stepID, skillUuid)).ok).toBe(true);

  const del = await wsCommand(page, {
    op: "delete_skill",
    id: `del-skill-${suffix}`,
    name: skillName,
  });
  expect(del.ok).toBe(true);

  const pj = path.join(e2eDataDir, "pipelines", pipelineID, "pipeline.json");
  const raw = fs.readFileSync(pj, "utf8");
  const j = JSON.parse(raw) as { steps: Array<{ skills: unknown[] }> };
  const skills = j.steps[0]?.skills ?? [];
  expect(skills.length).toBe(0);
});

test("broken pipeline nav link is red and marked with data-broken", async ({
  page,
  e2eDataDir,
}) => {
  const suffix = uniqueSuffix();
  const name = `broken-pipe-${suffix}`;
  fs.mkdirSync(path.join(e2eDataDir, "pipelines", name), { recursive: true });
  const pj = path.join(e2eDataDir, "pipelines", name, "pipeline.json");
  fs.writeFileSync(
    pj,
    JSON.stringify({
      steps: [
        {
          id: 1,
          title: "s",
          prompt: "p",
          skills: [{ id: "00000000-0000-0000-0000-000000000001", alias: "missing" }],
        },
      ],
    }),
    "utf8",
  );

  await page.goto("/pipelines");
  const link = page.getByTestId("pipeline-nav-link").filter({ hasText: name });
  await expect(link).toHaveAttribute("data-broken", "true");
  await expect(link).toHaveCSS("color", "rgb(185, 28, 28)");
});

test("GET /fragments/counter returns incrementing count", async ({ request }) => {
  const a = await request.get("/fragments/counter");
  const b = await request.get("/fragments/counter");
  expect(a.status()).toBe(200);
  expect(b.status()).toBe(200);
  const textA = await a.text();
  const textB = await b.text();
  expect(textA).toMatch(/hello world \d+/);
  expect(textB).toMatch(/hello world \d+/);
  const nA = Number((textA.match(/hello world (\d+)/) ?? [])[1]);
  const nB = Number((textB.match(/hello world (\d+)/) ?? [])[1]);
  expect(Number.isFinite(nA)).toBe(true);
  expect(Number.isFinite(nB)).toBe(true);
  expect(nB).toBe(nA + 1);
});
