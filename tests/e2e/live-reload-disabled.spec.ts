import { expect, test } from "./fixtures";

test("live reload is disabled in e2e and omits data-live-reload on html", async ({
  page,
}) => {
  await page.goto("/skills");
  await expect(page.locator("html")).not.toHaveAttribute("data-live-reload");
});
