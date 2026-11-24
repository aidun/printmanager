import { expect, test } from "@playwright/test";

// Katalogauswahl im Assistenten „Neuer Auftrag“ mit Beispieldaten aus src/devmock.ts

test("Katalog beim Hinzufügen nach Kategorie filtern", async ({ page }) => {
  await page.goto("/#today");
  await page.getByRole("button", { name: /Neuer Auftrag/ }).click();
  await page.getByRole("dialog").getByRole("button", { name: /Anna Berger/ }).click(); // springt zu „Was?“

  const picker = page.locator(".catalog-picker");
  const titles = picker.locator(".pick-title");
  await expect(titles).toHaveCount(8);

  // Sammlung „Haushalt“ und Stichwort „Haushalt“ sind eine Kategorie
  const select = picker.getByRole("combobox", { name: "Kategorie" });
  await expect(select.locator("option")).toHaveText(["Alle Kategorien (8)", "Bad (1)", "Haushalt (4)", "Spielzeug (1)", "Weihnachten (2)"]);
  await select.selectOption({ label: "Haushalt (4)" });
  await expect(titles).toHaveText(["Hogwarts Lesezeichen", "Wandhaken minimal", "Schlüsselanhänger Puzzle", "Zahnbürstenhalter"]);

  // Suche wirkt innerhalb der Kategorie; ohne Treffer zurück zu allen
  await picker.getByPlaceholder("Katalog durchsuchen …").fill("Benchy");
  await expect(titles).toHaveCount(0);
  await picker.getByRole("button", { name: "Alle Kategorien zeigen" }).click();
  await expect(titles).toHaveText(["3D Benchy"]);

  // Auswahl bleibt beim nächsten Öffnen erhalten
  await select.selectOption({ label: "Weihnachten (2)" });
  await page.reload();
  await page.getByRole("button", { name: /Neuer Auftrag/ }).click();
  await page.getByRole("dialog").getByRole("button", { name: /Anna Berger/ }).click(); // springt zu „Was?“
  await expect(page.locator(".catalog-picker").getByRole("combobox", { name: "Kategorie" })).toHaveValue("weihnachten");
  await expect(page.locator(".catalog-picker .pick-title")).toHaveCount(2);
});
