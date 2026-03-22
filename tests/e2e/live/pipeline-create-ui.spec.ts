import { expect, test } from "../fixtures-live";

function uniqueSuffix(): string {
  return `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
}

test("create pipeline happy path from left nav plus button (live)", async ({ page }) => {
  const suffix = uniqueSuffix();
  const name = `e2e-live-pipeline-${suffix}`;

  await page.goto("/");
  await page.getByTestId("pipeline-create-open").click();
  await page.locator("dialog#pipeline-modal input[name='name']").fill(name);
  await page.locator("dialog#pipeline-modal button[type='submit']").click();

  await expect(page).toHaveURL(new RegExp(`/pipelines/[a-z0-9-]+$`));
  await expect(page.locator("#pipeline-title")).toHaveText(name);
  await expect(page.getByTestId("pipeline-nav-link").filter({ hasText: name })).toBeVisible();
});
