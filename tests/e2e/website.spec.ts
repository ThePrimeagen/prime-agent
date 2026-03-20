import { expect, Page, test } from "@playwright/test";

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

async function openSkillsTab(page: Page) {
  await page.getByTestId("tab-skills").click();
  await expect(page).toHaveURL(/\/skills$/);
}

async function openPipelineTab(page: Page) {
  await page.getByTestId("tab-pipeline").click();
  await expect(page).toHaveURL(/\/pipelines$/);
}

async function createPipelineByRequest(request: Page["request"], name: string): Promise<string> {
  const response = await request.post("/pipelines", {
    form: { name },
    maxRedirects: 0,
  });
  expect(response.status()).toBe(303);
  const location = response.headers()["location"] ?? "";
  const id = location.split("/").pop() ?? "";
  expect(id).toMatch(/^\d+$/);
  return id;
}

async function createSkillByRequest(
  request: Page["request"],
  name: string,
  prompt: string,
): Promise<string> {
  const response = await request.post("/skills", {
    form: { name, prompt },
    maxRedirects: 0,
  });
  expect(response.status()).toBe(303);
  const location = response.headers()["location"] ?? "";
  const id = location.split("/").pop() ?? "";
  expect(id).toMatch(/^\d+$/);
  return id;
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

test("create pipeline happy path from left nav plus button", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-pipeline-${suffix}`;

  await page.goto("/");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page).toHaveURL(new RegExp(`/pipelines/\\d+$`));
  await expect(page.locator("#pipeline-title")).toHaveText(name);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: name })).toBeVisible();
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
  await expect(page).toHaveURL(new RegExp(`/pipelines/\\d+$`));
  await expect(page.locator("#pipeline-title")).toHaveText(normalizedName);
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

test("backend rejects uppercase pipeline name on insertion", async ({ request }) => {
  const suffix = uniqueSuffix();
  const uppercaseName = `BACKEND-PIPELINE-${suffix}`;
  const response = await request.post("/pipelines", {
    form: { name: uppercaseName },
  });
  expect(response.status()).toBe(400);
  await expect(response.text()).resolves.toContain(
    "name must contain only lowercase letters, digits, and dashes",
  );
});

test("create pipeline unhappy path rejects non-kebab names", async ({ request }) => {
  const invalidNames = ["bad name", "Bad-Name", "bad_name", "   "];
  for (const name of invalidNames) {
    const response = await request.post("/pipelines", {
      form: { name },
    });
    expect(response.status()).toBe(400);
    await expect(response.text()).resolves.toContain(
      "name must contain only lowercase letters, digits, and dashes",
    );
  }
});

test("pipeline list renders each pipeline as clickable nav item and routes to /pipelines/:id", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-pipeline-first-${suffix}`;
  const second = `e2e-pipeline-second-${suffix}`;

  const createOne = await request.post("/pipelines", {
    form: { name: first },
    maxRedirects: 0,
  });
  expect(createOne.status()).toBe(303);
  const createTwo = await request.post("/pipelines", {
    form: { name: second },
    maxRedirects: 0,
  });
  expect(createTwo.status()).toBe(303);
  const locationTwo = createTwo.headers()["location"] ?? "";
  const secondId = locationTwo.split("/").pop() ?? "";

  await page.goto("/pipelines");
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: first })).toBeVisible();
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: second })).toBeVisible();

  await page.getByTestId("pipeline-nav-link").filter({ hasText: second }).click();
  await expect(page).toHaveURL(new RegExp(`/pipelines/${secondId}$`));
  await expect(page.locator("#pipeline-title")).toHaveText(second);
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

  await expect(page).toHaveURL(new RegExp(`/skills/\\d+$`));
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(prompt);
});

test("create skill unhappy path rejects non-kebab names", async ({ request }) => {
  const invalidNames = ["bad name", "Bad-Name", "bad_name", ""];
  for (const name of invalidNames) {
    const response = await request.post("/skills", {
      form: { name, prompt: "some-prompt" },
    });
    expect(response.status()).toBe(400);
    await expect(response.text()).resolves.toContain(
      "name must contain only lowercase letters, digits, and dashes",
    );
  }
});

test("skill update unhappy path rejects invalid rename and allows kebab rename", async ({
  request,
}) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-update-${suffix}`;
  const validRenamed = `e2e-renamed-${suffix}`;
  const prompt = `prompt-${suffix}`;

  const createResponse = await request.post("/skills", {
    form: { name: originalName, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);
  const location = createResponse.headers()["location"] ?? "";

  const invalidRename = await request.post(`${location}/update`, {
    headers: { "X-Autosave": "1" },
    form: { name: "Legacy_Name", prompt },
  });
  expect(invalidRename.status()).toBe(400);
  await expect(invalidRename.text()).resolves.toContain(
    "name must contain only lowercase letters, digits, and dashes",
  );

  const validRename = await request.post(`${location}/update`, {
    headers: { "X-Autosave": "1" },
    form: { name: validRenamed, prompt: `${prompt}-updated` },
  });
  expect(validRename.status()).toBe(204);
});

test("autosave update happy path persists edits while typing", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-update-old-${suffix}`;
  const updatedName = `e2e-update-new-${suffix}`;
  const originalPrompt = `old-prompt-${suffix}`;
  const updatedPrompt = `new-prompt-${suffix}`;

  const createResponse = await request.post("/skills", {
    form: { name: originalName, prompt: originalPrompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);

  let autosaveRequests = 0;
  page.on("request", (req) => {
    if (
      req.url().includes("/skills/") &&
      req.url().endsWith("/update") &&
      req.headers()["x-autosave"] === "1"
    ) {
      autosaveRequests += 1;
    }
  });

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  await expect(page).toHaveURL(/\/skills\/\d+$/);

  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").click();
  await editor.locator("input[name='name']").fill("");
  await editor.locator("input[name='name']").type(updatedName);
  await editor.locator("textarea[name='prompt']").click();
  await editor.locator("textarea[name='prompt']").fill("");
  await editor.locator("textarea[name='prompt']").type(updatedPrompt);
  await expect(editor.locator("[data-testid='autosave-status']")).toHaveAttribute(
    "data-save-state",
    "saved",
  );
  expect(autosaveRequests).toBeGreaterThan(1);

  await page.reload();
  await expect(page.locator(`input[name="name"][value="${updatedName}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(updatedPrompt);
});

test("autosave update unhappy path returns 404 for missing skill", async ({
  request,
}) => {
  const response = await request.post("/skills/999999/update", {
    headers: { "X-Autosave": "1" },
    form: { name: "missing", prompt: "missing" },
  });
  expect(response.status()).toBe(404);
});

test("delete skill happy path removes row from page", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-delete-${suffix}`;
  const prompt = `prompt-${suffix}`;

  const createResponse = await request.post("/skills", {
    form: { name, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page).toHaveURL(/\/skills\/\d+$/);

  const editor = page.locator("[data-skill-editor]");
  await editor.locator("[data-testid='delete-skill-trigger']").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeVisible();
  await editor.locator("[data-delete-confirm]").click();
  await expect(page).toHaveURL(/\/skills$/);
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toHaveCount(0);
});

test("delete popover closes on outside click without deleting", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const name = `e2e-delete-cancel-${suffix}`;
  const prompt = `prompt-${suffix}`;

  const createResponse = await request.post("/skills", {
    form: { name, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: name }).click();
  await expect(page).toHaveURL(/\/skills\/\d+$/);

  const editor = page.locator("[data-skill-editor]");
  await editor.locator("[data-testid='delete-skill-trigger']").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeVisible();
  await page.locator("#skills-main-panel h1").click();
  await expect(editor.locator("[data-testid='delete-skill-popover']")).toBeHidden();
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: name })).toBeVisible();
});

test("delete skill unhappy path returns 404 for missing skill", async ({ request }) => {
  const response = await request.post("/skills/999999/delete");
  expect(response.status()).toBe(404);
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
  expect(putSkillMutation.status()).toBe(405);

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
  request,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-skill-first-${suffix}`;
  const second = `e2e-skill-second-${suffix}`;

  const createOne = await request.post("/skills", {
    form: { name: first, prompt: "prompt one" },
    maxRedirects: 0,
  });
  expect(createOne.status()).toBe(303);
  const createTwo = await request.post("/skills", {
    form: { name: second, prompt: "prompt two" },
    maxRedirects: 0,
  });
  expect(createTwo.status()).toBe(303);
  const locationTwo = createTwo.headers()["location"] ?? "";
  const secondId = locationTwo.split("/").pop() ?? "";

  await page.goto("/skills");
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: first })).toBeVisible();
  await expect(page.getByTestId("skill-nav-link").filter({ hasText: second })).toBeVisible();

  await page.getByTestId("skill-nav-link").filter({ hasText: second }).click();
  await expect(page).toHaveURL(new RegExp(`/skills/${secondId}$`));
  await expect(page.locator(`input[name="name"][value="${second}"]`)).toBeVisible();
});

test("main area title updates to currently selected pipeline from /pipelines/:id", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const first = `e2e-pipeline-title-first-${suffix}`;
  const second = `e2e-pipeline-title-second-${suffix}`;

  await request.post("/pipelines", { form: { name: first }, maxRedirects: 0 });
  await request.post("/pipelines", { form: { name: second }, maxRedirects: 0 });

  await page.goto("/pipelines");
  await page.getByTestId("pipeline-nav-link").filter({ hasText: first }).click();
  await expect(page.locator("#pipeline-title")).toHaveText(first);

  await page.getByTestId("pipeline-nav-link").filter({ hasText: second }).click();
  await expect(page.locator("#pipeline-title")).toHaveText(second);
});

test("skills detail main area renders from /skills/:id", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-skill-detail-${suffix}`;
  const prompt = `prompt-${suffix}`;

  const createResponse = await request.post("/skills", {
    form: { name, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);
  const location = createResponse.headers()["location"] ?? "";

  await page.goto(location);
  await expect(page).toHaveURL(/\/skills\/\d+$/);
  await expect(page.locator(`input[name="name"][value="${name}"]`)).toBeVisible();
  await expect(page.locator("#skills-main-panel textarea[name='prompt']")).toHaveValue(prompt);
});

test("autosave retries after one second when backend fails once", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-retry-timer-${suffix}`;
  const prompt = `prompt-${suffix}`;
  const createResponse = await request.post("/skills", {
    form: { name: originalName, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);

  const requestTimes: number[] = [];
  let failedFirstAutosave = false;
  await page.route("**/skills/*/update", async (route) => {
    const headers = route.request().headers();
    if (headers["x-autosave"] === "1") {
      requestTimes.push(Date.now());
      if (!failedFirstAutosave) {
        failedFirstAutosave = true;
        await route.fulfill({ status: 500, body: "failed once" });
        return;
      }
    }
    await route.continue();
  });

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").type("x");
  await expect(editor.locator("[data-testid='autosave-status']")).toHaveAttribute(
    "data-save-state",
    "saved",
  );
  expect(requestTimes.length).toBeGreaterThanOrEqual(2);
  expect(requestTimes[1] - requestTimes[0]).toBeGreaterThanOrEqual(900);
});

test("autosave retries on next keypress before timer fires", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const originalName = `e2e-retry-key-${suffix}`;
  const prompt = `prompt-${suffix}`;
  const createResponse = await request.post("/skills", {
    form: { name: originalName, prompt },
    maxRedirects: 0,
  });
  expect(createResponse.status()).toBe(303);

  const requestTimes: number[] = [];
  let failedFirstAutosave = false;
  await page.route("**/skills/*/update", async (route) => {
    const headers = route.request().headers();
    if (headers["x-autosave"] === "1") {
      requestTimes.push(Date.now());
      if (!failedFirstAutosave) {
        failedFirstAutosave = true;
        await route.fulfill({ status: 500, body: "failed once" });
        return;
      }
    }
    await route.continue();
  });

  await page.goto("/skills");
  await page.getByTestId("skill-nav-link").filter({ hasText: originalName }).click();
  const editor = page.locator("[data-skill-editor]");
  await expect(editor).toBeVisible();

  await editor.locator("input[name='name']").type("x");
  await page.waitForTimeout(200);
  await editor.locator("input[name='name']").type("y");

  await expect(editor.locator("[data-testid='autosave-status']")).toHaveAttribute(
    "data-save-state",
    "saved",
  );
  expect(requestTimes.length).toBeGreaterThanOrEqual(2);
  expect(requestTimes[1] - requestTimes[0]).toBeLessThan(900);
});

test("create pipeline step happy path with title and prompt", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-create-${suffix}`);
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

test("pipeline step title input lowercases in real time and persists lowercase", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-lowercase-${suffix}`);
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

test("pipeline step title paste is immediately normalized to lowercase", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-lowercase-paste-${suffix}`);
  const pastedTitle = `PaStEd-STEP-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  await page.getByTestId("pipeline-step-create-open").click();
  const input = page.locator("dialog#pipeline-step-modal input[name='title']");
  await input.fill(pastedTitle);
  await expect(input).toHaveValue(pastedTitle.toLowerCase());
});

test("backend normalizes uppercase pipeline step title on insertion", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-backend-lower-${suffix}`);
  const uppercaseTitle = `BACKEND-STEP-${suffix}`;
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: uppercaseTitle, prompt: "prompt" },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);
  const normalizedTitle = uppercaseTitle.toLowerCase();

  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-nav-item").filter({ hasText: normalizedTitle })).toBeVisible();
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(normalizedTitle);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue("prompt");
});

test("create pipeline step unhappy path rejects empty title or prompt", async ({ request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-invalid-${suffix}`);

  const emptyTitle = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: "   ", prompt: "valid prompt" },
  });
  expect(emptyTitle.status()).toBe(400);

  const emptyPrompt = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: "valid title", prompt: "   " },
  });
  expect(emptyPrompt.status()).toBe(400);
});

test("edit pipeline step happy path persists updated title and prompt", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-edit-${suffix}`);
  const originalTitle = `step-edit-original-${suffix}`;
  const originalPrompt = `step-edit-prompt-${suffix}`;
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: originalTitle, prompt: originalPrompt },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);

  const updatedTitle = `step-edit-updated-${suffix}`;
  const updatedPrompt = `step-edit-updated-prompt-${suffix}`;

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor).toBeVisible();
  await editor.locator("input[name='title']").fill(updatedTitle);
  await editor.locator("textarea[name='prompt']").fill(updatedPrompt);
  await editor.locator("[data-testid='pipeline-step-save']").click();

  await expect(page).toHaveURL(new RegExp(`/pipelines/${pipelineID}$`));
  await expect(page.getByTestId("pipeline-step-nav-item").filter({ hasText: updatedTitle.toLowerCase() })).toBeVisible();
  await expect(editor.locator("input[name='title']")).toHaveValue(updatedTitle.toLowerCase());
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(updatedPrompt);

  await page.reload();
  const reloadedEditor = page.getByTestId("pipeline-step-editor").first();
  await expect(reloadedEditor.locator("input[name='title']")).toHaveValue(updatedTitle.toLowerCase());
  await expect(reloadedEditor.locator("textarea[name='prompt']")).toHaveValue(updatedPrompt);
});

test("edit pipeline step unhappy path rejects empty title or prompt and preserves previous values", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-edit-invalid-${suffix}`);
  const originalTitle = `step-edit-keep-${suffix}`;
  const originalPrompt = `step-edit-keep-prompt-${suffix}`;
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: originalTitle, prompt: originalPrompt },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);
  const location = createStep.headers()["location"] ?? "";
  const stepID = location.split("/").pop() ?? "";
  expect(stepID).toMatch(/^\d+$/);

  const emptyTitle = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/update`, {
    form: { title: "   ", prompt: "valid prompt" },
  });
  expect(emptyTitle.status()).toBe(400);

  const emptyPrompt = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/update`, {
    form: { title: "valid title", prompt: "   " },
  });
  expect(emptyPrompt.status()).toBe(400);

  await page.goto(`/pipelines/${pipelineID}`);
  const editor = page.getByTestId("pipeline-step-editor").first();
  await expect(editor.locator("input[name='title']")).toHaveValue(originalTitle);
  await expect(editor.locator("textarea[name='prompt']")).toHaveValue(originalPrompt);
});

test("delete pipeline step happy path removes step from main panel and left nav", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-delete-${suffix}`);
  const stepCreate = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `step-delete-title-${suffix}`, prompt: "prompt" },
    maxRedirects: 0,
  });
  expect(stepCreate.status()).toBe(303);

  await page.goto(`/pipelines/${pipelineID}`);
  const stepItem = page.getByTestId("pipeline-step-nav-item");
  await expect(stepItem).toHaveCount(1);
  await page.getByTestId("pipeline-step-delete").click();

  await expect(page.getByTestId("pipeline-step-nav-item")).toHaveCount(0);
  await expect(page.getByTestId("pipeline-step-editor")).toHaveCount(0);
});

test("delete pipeline step unhappy path returns 404 for missing step", async ({ request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-delete-missing-${suffix}`);

  const response = await request.post(`/pipelines/${pipelineID}/steps/999999/delete`);
  expect(response.status()).toBe(404);
});

test("pipeline step left-nav drag swap persists after refresh", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-reorder-${suffix}`);
  await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `first-${suffix}`, prompt: "prompt-a" },
    maxRedirects: 0,
  });
  await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `second-${suffix}`, prompt: "prompt-b" },
    maxRedirects: 0,
  });

  await page.goto(`/pipelines/${pipelineID}`);
  const items = page.getByTestId("pipeline-step-nav-item");
  await expect(items).toHaveCount(2);
  await items.nth(1).dragTo(items.nth(0));

  await expect(items.nth(0)).toContainText(`second-${suffix}`);
  await page.reload();
  await expect(page.getByTestId("pipeline-step-nav-item").nth(0)).toContainText(`second-${suffix}`);
});

test("pipeline step reorder unhappy path rejects invalid step target", async ({ request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-reorder-invalid-${suffix}`);
  const response = await request.post(`/pipelines/${pipelineID}/steps/999999/reorder`, {
    form: { target_step_id: "888888" },
  });
  expect(response.status()).toBe(404);
});

test("pipeline step skill association happy path increments per-step count", async ({
  page,
  request,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-skill-count-${suffix}`);
  const skillA = await createSkillByRequest(request, `skill-a-${suffix}`, "a prompt");
  const skillB = await createSkillByRequest(request, `skill-b-${suffix}`, "b prompt");
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `skill-step-${suffix}`, prompt: "prompt" },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);
  const stepID = (createStep.headers()["location"] ?? "").split("/").pop() ?? "";

  const addA = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills`, {
    form: { skill_id: skillA },
    maxRedirects: 0,
  });
  expect(addA.status()).toBe(303);
  const addB = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills`, {
    form: { skill_id: skillB },
    maxRedirects: 0,
  });
  expect(addB.status()).toBe(303);

  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-skill-count").filter({ hasText: "2" })).toBeVisible();
});

test("pipeline step skill association unhappy path rejects duplicate skill per step", async ({
  request,
}) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-skill-dup-${suffix}`);
  const skillID = await createSkillByRequest(request, `skill-dup-${suffix}`, "dup prompt");
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `dup-step-${suffix}`, prompt: "prompt" },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);
  const stepID = (createStep.headers()["location"] ?? "").split("/").pop() ?? "";

  const first = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills`, {
    form: { skill_id: skillID },
    maxRedirects: 0,
  });
  expect(first.status()).toBe(303);

  const duplicate = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills`, {
    form: { skill_id: skillID },
  });
  expect(duplicate.status()).toBe(400);
});

test("pipeline step rows render accurate skill counts", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-count-render-${suffix}`);
  const skillID = await createSkillByRequest(request, `count-skill-${suffix}`, "count prompt");

  const createOne = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `count-one-${suffix}`, prompt: "prompt-1" },
    maxRedirects: 0,
  });
  expect(createOne.status()).toBe(303);
  const createTwo = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `count-two-${suffix}`, prompt: "prompt-2" },
    maxRedirects: 0,
  });
  expect(createTwo.status()).toBe(303);
  const stepOneID = (createOne.headers()["location"] ?? "").split("/").pop() ?? "";

  const linkSkill = await request.post(`/pipelines/${pipelineID}/steps/${stepOneID}/skills`, {
    form: { skill_id: skillID },
    maxRedirects: 0,
  });
  expect(linkSkill.status()).toBe(303);

  await page.goto(`/pipelines/${pipelineID}`);
  const rows = page.getByTestId("pipeline-step-nav-item");
  await expect(rows.filter({ hasText: `count-one-${suffix}` }).getByTestId("pipeline-step-skill-count")).toHaveText("1");
  await expect(rows.filter({ hasText: `count-two-${suffix}` }).getByTestId("pipeline-step-skill-count")).toHaveText("0");
});

test("pipeline step skill count updates after add and remove skill", async ({ page, request }) => {
  const suffix = uniqueSuffix();
  const pipelineID = await createPipelineByRequest(request, `step-count-update-${suffix}`);
  const skillID = await createSkillByRequest(request, `update-skill-${suffix}`, "update prompt");
  const createStep = await request.post(`/pipelines/${pipelineID}/steps`, {
    form: { title: `update-step-${suffix}`, prompt: "prompt" },
    maxRedirects: 0,
  });
  expect(createStep.status()).toBe(303);
  const stepID = (createStep.headers()["location"] ?? "").split("/").pop() ?? "";

  const add = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills`, {
    form: { skill_id: skillID },
    maxRedirects: 0,
  });
  expect(add.status()).toBe(303);
  await page.goto(`/pipelines/${pipelineID}`);
  await expect(page.getByTestId("pipeline-step-skill-count")).toContainText("1");

  const remove = await request.post(`/pipelines/${pipelineID}/steps/${stepID}/skills/${skillID}/delete`, {
    maxRedirects: 0,
  });
  expect(remove.status()).toBe(303);
  await page.reload();
  await expect(page.getByTestId("pipeline-step-skill-count")).toContainText("0");
});
