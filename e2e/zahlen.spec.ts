import { expect, test } from "@playwright/test";

// Beispieldaten: TPU hat genau eine Spule

test("Einzahl bei genau einer Spule", async ({ page }) => {
  await page.goto("/#filament");
  await expect(page.getByText("1 Spule ·").first()).toBeVisible();
  await expect(page.getByText("1 Spulen")).toHaveCount(0);
});
