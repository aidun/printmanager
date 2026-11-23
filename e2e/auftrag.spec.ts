import { expect, test } from "@playwright/test";

// Grundabläufe im Auftrag mit den Beispieldaten aus src/devmock.ts

test("Platten eines Teils mit Bild abwählen", async ({ page }) => {
  await page.goto("/#board&order=o6");
  const plates = page.locator(".item-plates").first();
  await expect(plates.locator(".plate-card")).toHaveCount(3);
  await expect(plates).toContainText("alle 3 Platten");
  await page.getByRole("checkbox", { name: "Platte 2 drucken" }).click();
  await expect(plates).toContainText("2 von 3 Platten");
  await expect(plates.locator(".plate-card.off")).toHaveCount(1);
});

test("Kalkulationsdialog: eigener Drucker, Normal + Stützen, AMS zuerst", async ({ page }) => {
  await page.goto("/#board&order=o6");
  await page.getByRole("button", { name: "Kalkulation", exact: true }).first().click();
  const dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("heading", { name: /Kalkulation und Druck/ })).toBeVisible();
  await expect(dialog.getByLabel("Drucker")).toHaveValue("h2c");
  await expect(dialog.getByLabel("Prozess").locator("option:checked")).toHaveText(/Normal \+ Stützen/);
  const firstGroup = dialog.getByLabel("Spule Slot 1").locator("optgroup").first();
  await expect(firstGroup).toHaveAttribute("label", "Im AMS");
});

test("Drucke: Platte überspringen, Abziehen erst ohne offene Platten", async ({ page }) => {
  await page.goto("/#board&order=o9");
  const plates = page.locator(".run-plates li");
  await expect(plates).toHaveCount(3);
  await expect(page.locator(".run-actions")).toContainText("Noch 2 Platten offen");
  await page.getByRole("button", { name: "Überspringen", exact: true }).last().click();
  await expect(plates.nth(2)).toContainText("übersprungen");
  await expect(page.locator(".run-actions")).toContainText("Noch 1 Platte offen");
  await expect(page.locator(".cost-box .basis")).toHaveText("teils gedruckt, teils geplant");
});

test("Alle kalkulieren: rechnet Teile mit Modell, überspringt die übrigen", async ({ page }) => {
  await page.goto("/#board&order=o6");
  await page.getByRole("button", { name: "Alle kalkulieren" }).click();
  await expect(page.getByText(/1 Teil kalkuliert\. Ohne Modelldatei übersprungen/)).toBeVisible();
  await expect(page.locator(".est-tag").first()).toContainText("Slicer");
});
