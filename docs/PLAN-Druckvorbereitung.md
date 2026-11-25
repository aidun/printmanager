# Plan: Druckauftrag in der App fertig vorbereiten und an Bambu übergeben

Stand 25.11.2026: D1 ist erledigt (20.11.2026), D2 bis D4 sind offen. Baut auf „Kalkulation vorab“ auf (`estimates.rs`, `profiles.rs`, `item_estimates`).

## Ziel

In der App wird jedes Teil eines Auftrags druckfertig gemacht, ohne in Bambu Studio etwas einstellen zu müssen:

- Drucker und Düse wählen
- Prozessprofil wählen
- Farben bzw. Filamente je Objekt festlegen

Die Spulen aus dem Bestand und die passenden Filament-Profile werden dabei vorgeschlagen.

Danach öffnet ein Klick das fertige Projekt in Bambu Studio. Dort sind Drucker, Prozess, Filamente, Farben und Platte schon gesetzt. Der Nutzer prüft, slict und druckt wie gewohnt. Die App startet nie selbst einen Druck.

## Was heute schon geht (geprüft 18.11.2026 mit Bambu Studio 2.9.1)

- `--slice 0` mit eigenständigen Presets aus `aidun/printerprofiles` liefert Zeit und Gramm. Auch bei laufender Oberfläche ändert das nichts im Ordner von Bambu Studio.
- **Ohne `--slice`** schreibt `--export-3mf` ein ungesliced Projekt mit Einstellungen:
  - In `project_settings.config` stehen `printer_settings_id`, `print_settings_id`, `filament_settings_id[]`, `filament_type[]`, `filament_ids[]`, `curr_bed_type` und `nozzle_diameter`, jeweils als vollständige Werte.
  - Bambu Studio zeigt diese Werte beim Öffnen als Projekt-Presets. Es muss dafür nichts in seine eigenen Dateien schreiben.
- `--load-filament-ids "2,1,…"` legt das Filament je Objekt fest (`model_settings.config` → `extruder`).
- **Lücke:** `filament_colour` übernimmt die Kommandozeile nur für den ersten Eintrag, und zwar mit der Standardfarbe des Presets. Die Farben muss die App danach selbst in `project_settings.config` setzen. Das ist erlaubt, weil es die eigene Datei im Auftragsordner ist.

## Regeln, die gelten bleiben

- Kein Steuern von Druckern. Gedruckt wird in Bambu Studio oder Bambu Handy.
- Nie in Dateien von Bambu Studio schreiben. Die Profile kommen nur über das Projekt mit.
- Das Bambu-Konto wird nur gelesen. Filament-Profile ins Konto zu legen wäre eine neue Schreib-Ausnahme und ist hier **nicht** vorgesehen (siehe Entscheidungen).

## Stufen

### D1: Druckvorbereitung je Teil (Daten und Oberfläche) – erledigt 20.11.2026

Umgesetzt mit einer Abweichung: Slots und Objekte stehen als JSON in `item_estimates` (`slots`, `objects`) statt in eigenen Tabellen, wie schon die Filamente der Schätzung.

Entschieden: **ein Projekt je Teil**. Das Profil des Teils steht schon in `item_estimates` (Drucker, Düse, Prozess, Platte) und wird weiterverwendet. Dieselbe Wahl gilt also für Kalkulation und Übergabe.

- **Migration 0018:**
  - `item_estimates` bekommt die Spalten `setup_state` (`entwurf`, `bereit`, `uebergeben`) und `project_file`.
  - Neue Tabelle **`item_slots`**: Teil, Slot-Nr., Filament-Preset, Farbe (`#RRGGBB`), Material, optional `spool_id`.
  - Neue Tabelle **`item_objects`**: Teil, Objekt bzw. Volumen im Modell, Slot-Nr.
- **Vorbelegung:** aus den Filament-Slots im Katalogprojekt bzw. in der Druckdatei (Material und Farbe) und aus `color_wish` des Teils.
- **Ansicht „Druck vorbereiten“:** Der Kalkulationsdialog wird zum Dialog „Kalkulation und Druck“ des Teils:
  1. Oben Drucker, Düse, Prozess und Platte, wie bisher.
  2. Darunter die Slots als Farbfelder. Ein Klick öffnet die Spulenauswahl mit `rankSpools` (Material → Farbabstand → Rest). Daraus folgt das Filament-Preset: Die Kennung des Presets (`filament`, z. B. `bambu-pla-basic`) wird aus Marke, Serie und Material der Spule abgeleitet. Ohne Treffer wählt der Nutzer das Preset von Hand.
  3. Darunter die Objekte mit einem Slot je Objekt, per Auswahlfeld zugeordnet.
- **Warnungen in Klartext:**
  - Spule reicht nicht (Gramm aus der Schätzung gegen den Rest).
  - Material passt nicht zum Preset.
  - Mehr Slots als das AMS des Druckers hat.
- **Leerer Zustand:** „Noch keine Druckvorbereitung. Profil und Farben wählen, dann an Bambu Studio übergeben.“

### D2: Projekt erzeugen

- **`SlicerAdapter::prepare(job) -> PathBuf`** neben `slice`, mit Rückfall auf einen Fehler bei Slicern ohne diese Fähigkeit. Neue Fähigkeit `Cap::SlicerPrepare`, ab Werk an, weil die App nur eine eigene Datei erzeugt.
- **Bambu Studio:**
  - Aufruf: `--arrange 1 --load-settings drucker;prozess --load-filaments f1;f2;… --load-filament-ids … --export-3mf <Teil>.3mf --outputdir <Auftragsordner>/druck/` mit der Datei des Teils.
  - Menge > 1: Das Modell wird mehrfach übergeben und auf Platten verteilt. Ob das klappt, ist zu prüfen. Sonst stellt der Nutzer die Kopien in Studio ein, und ein Hinweis nennt die Menge.
- **Nachbearbeitung im eigenen Zip:**
  - `filament_colour[]` je Slot setzen.
  - `printer_settings_id` auf den Systemdrucker setzen (`Bambu Lab H2C 0.4 nozzle`), damit Studio den Drucker erkennt und kein Projekt-Drucker „Printmanager …“ erscheint. Das muss im Test bestätigt werden.
- **Ablage:**
  - Das Projekt liegt im Auftragsordner, also kommt die gesliced Version über `watch.rs` zurück.
  - Damit dieses Projekt keinen zweiten Anhang erzeugt, wird es als eigene Datei markiert.
- **Offen:** Drucker mit zwei Düsen (H2C/H2D). Die Zuordnung links/rechts läuft über `filament_map`. Erst prüfen, dann für Doppeldüse freigeben. Bis dahin ordnet Bambu Studio automatisch zu (`filament_map_mode` = Auto).

### D3: Übergabe an Bambu Studio

- **Schaltfläche „An Bambu Studio übergeben“:**
  1. Projekt erzeugen (D2).
  2. Mit `SlicerAdapter::open` öffnen.
  3. Status auf `in_studio` setzen und `uebergeben` vermerken. Ein Eintrag im Verlauf nennt Drucker, Prozess und die Slots mit Spule.
- **Rückweg wie bisher:** Speichert der Nutzer die gesliced Datei im Auftragsordner, entsteht ein Druck. Die Vorbereitung liefert dabei die Spulen für den Abzug als Vorschlag, statt sie wie heute nur über Farbe und Material zu raten.
- **Prüfhinweis vor dem Drucken** (wie im Bambu-Skill): Drucker, Platte, Düse, AMS-Zuordnung, freie Platte.

### D4: Doku und Prüfung

- Doku: `BENUTZERHANDBUCH.md` (Druck vorbereiten), `ARCHITEKTUR.md` (Abläufe, Tabellen) und `CHANGELOG.md`.
- Tests:
  - Projekt aus Testwürfel und zwei Slots erzeugen und `project_settings.config` prüfen. Läuft nur, wenn Bambu Studio installiert ist, sonst wird der Test übersprungen wie bei den Fixtures.
  - Store-Tests für die neuen Tabellen.
  - Ableitung Spule → Preset.
- Mit Zustimmung in der echten App prüfen: Projekt in Studio öffnen. Dabei kontrollieren, dass Drucker, Prozess, Farben und Platte gesetzt sind und keine Meldung zu fehlenden Presets erscheint.

## Spätere Erweiterungen (nicht Teil dieses Plans)

- **Bambu Connect:** eine in der App gesliced `.gcode.3mf` über `bambu-connect://import-file?path=…&name=…&version=1.0.0` übergeben. Der Druck wird weiter in Connect gestartet. Dafür muss Harte Regel 1 um Bambu Connect erweitert werden.
- **Filament-Profile ins Bambu-Konto** (Benutzer-Presets, damit sie auf jedem Rechner in Studio erscheinen): wäre eine neue Schreib-Ausnahme in `bambu/`, mit eigenem Schalter, Bestätigung und Protokoll. Der Endpunkt ist noch nicht geprüft.
- Weitere Slicer (OrcaSlicer): Dafür reicht ein weiterer `SlicerAdapter` mit `prepare`.

## Entscheidungen

Getroffen am 18.11.2026:

1. **Einheit:** ein Projekt je Teil.
2. **Übergabeweg:** Bambu Studio. Bambu Connect bleibt eine spätere Erweiterung.
3. **Filament-Profile ins Konto:** vorerst nicht, sie kommen über das Projekt mit.
