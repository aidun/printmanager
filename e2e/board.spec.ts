import { expect, test } from "@playwright/test";

// Ab 1600 × 1000 stehen alle acht Spalten ohne waagerechtes Scrollen nebeneinander

test("Board zeigt bei 1600 px alle Spalten ohne Scrollen", async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto("/#board");
  await expect(page.locator(".column")).toHaveCount(8);
  const { scroll, client } = await page.locator(".board").evaluate((b) => ({ scroll: b.scrollWidth, client: b.clientWidth }));
  expect(scroll).toBeLessThanOrEqual(client);
});
