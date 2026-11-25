import { expect, test } from "@playwright/test";

// Beispieldaten aus src/devmock.ts: 8 offene Aufträge, einer davon pausiert; knappe Spulen mit Material im Namen

test("Heute und Seitenleiste zählen offene Aufträge gleich", async ({ page }) => {
  await page.goto("/#today");
  await expect(page.locator(".sidebar-foot")).toContainText("8 offene Aufträge");
  await expect(page.locator(".toolbar")).toContainText("8 offene Aufträge");
});

test("Knappe Spule ohne doppeltes Material im Namen", async ({ page }) => {
  await page.goto("/#today");
  const low = page.locator(".today-block", { hasText: "Filament knapp" });
  await expect(low).toContainText("Bambu Lab PLA Matte");
  await expect(low).not.toContainText("PLA Bambu Lab");
});

test("Fällig-Zahl in der Seitenleiste entspricht der Karte auf Heute", async ({ page }) => {
  await page.goto("/#today");
  const kpi = page.locator(".kpi-tile", { hasText: "Fällig" }).locator(".kpi-num");
  const badge = page.locator(".nav-count").first();
  await expect(kpi).toHaveText("3");
  await expect(badge).toHaveText("3");
  await expect(badge).toHaveClass(/alert/);
});
