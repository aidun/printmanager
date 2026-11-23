import { expect, test } from "@playwright/test";

// Laufender Druck mit Beispieldaten aus src/devmock.ts: H2C druckt, Fortschritt geschätzt

test("Geschätzter Fortschritt erscheint mit „ca.“ statt 0 %", async ({ page }) => {
  await page.goto("/#printers");
  await expect(page.locator(".sidebar-foot")).toContainText("H2C · ca. 46 %");
  await expect(page.locator(".printer-meta").first()).toContainText("ca. 46 %");
  await expect(page.locator(".sidebar-foot")).not.toContainText("0 %");
});
