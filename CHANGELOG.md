# Änderungen

## Unveröffentlicht

**Neu**
- Katalogauswahl beim Anlegen eines Auftrags und bei „+ Aus Katalog“: Filter nach Kategorie neben der Suche. Kategorien sind deine MakerWorld-Sammlungen und die Stichworte der Projekte, gleiche Namen zählen als eine Kategorie. Die App merkt sich die zuletzt gewählte.

**Behoben**
- Der Fortschritt eines laufenden Drucks stand immer auf 0 %, weil das Bambu-Konto über die Web-Schnittstelle keinen Fortschritt meldet. Die App schätzt ihn jetzt aus Startzeit und geplanter Druckdauer (Druckverlauf) und zeigt ihn mit „ca.“, dazu die Restzeit. Ohne Schätzung steht dort „druckt“ statt „0 %“.

## 0.4.0 – 23.11.2026

**Kalkulation und Druckvorbereitung**
- **Kalkulation vorab je Teil:** Teil im Hintergrund mit Bambu Studio slicen (Profile pro Teil, aus [aidun/printerprofiles](https://github.com/aidun/printerprofiles); ab Werk aus) oder Druckzeit und Gewicht eines MakerWorld-Druckprofils übernehmen. Der Preis steht damit vor dem eigenen Slicen fest.
- **Druck vorbereiten je Teil:** Im Dialog „Kalkulation und Druck“ legst du je Filament-Slot Spule, Farbe und Filament-Profil fest und ordnest die Objekte den Slots zu. Die App schlägt Spulen aus dem Bestand vor (AMS zuerst) und warnt, wenn eine Spule nicht reicht oder das Profil nicht zum Material passt.
- Zur Wahl stehen nur die eigenen Drucker aus dem Bambu-Konto, und einer ist immer gewählt. Standardprozess ist „Normal + Stützen“.
- Knopf **Alle kalkulieren** über der Teileliste: slict alle Teile nacheinander, Teile ohne Profil bekommen die Standardauswahl.
- Slicen im Hintergrund kann mehrere Objekte in einer 3MF, bemalte Modelle und Modelle, die zum Teil unter der Druckplatte liegen. Bei mehreren Slots rechnet es mit einem Filament-Profil für alle, die Mengen je Slot kommen aus dem Modell.
- Gleiches Filament in zwei Slots: getrennte Verbrauchswerte je Slot und Spule, Hinweis bei gleicher Spule mit „Zusammenlegen“.

**Platten**
- **Platten in der Teileliste:** Unter jedem Teil mit einer Projektdatei mit mehreren Platten stehen die Platten mit Bild. Ein Klick wählt sie ab oder an, und die Schätzung passt sich ohne neues Slicen an.
- **Platten unter Drucke:** Jede Platte steht einzeln da (gedruckt, offen, übersprungen), auch ohne gesliced Datei. Drucke aus dem Druckverlauf schließen ihre Platte. Die Kalkulation rechnet gedruckte Platten tatsächlich und offene geplant („teils gedruckt, teils geplant“). Abziehen geht, sobald keine Platte mehr offen ist.

**Drucke**
- Automatisch zugeordnete Drucke aus dem Druckverlauf tragen nach dem Druck Dauer und Gramm in den Auftrag, wie bei der Zuordnung von Hand. Die Kalkulation ist dann tatsächlich, und der Abzug von den Spulen wird angeboten. Liegen schon genauere Werte vor, sagt der Kalkulationsdialog, dass Slicen nicht nötig ist.

**Kunden und Anbindungen**
- **Kunden aus Kontakten (macOS):** Anbindung „Apple Kontakte“ (nur lesen). „Aus Kontakten …“ legt Kunden aus Kontakten an, „Mit Kontakt verknüpfen …“ verbindet bestehende. Verknüpfte Kunden bleiben aktuell (alle 30 Minuten); haben sich Kontakt und Kunde beide geändert, entscheidest du.
- Einstellungen → **Anbindungen**: alle Dienste und Programme an einer Stelle (Bambu-Konto, Bambu Studio, Web, Printables, Druckprofile, Kontakte) mit Anmeldung, An/Aus und Schaltern je Fähigkeit; ein gemeinsames Protokoll aller geschriebenen Änderungen. Drucker und Druckverlauf haben getrennte Schalter.

**Behoben**
- Gesliced Dateien fassten zwei Slots mit gleichem Material und gleicher Farbe zu einem Eintrag zusammen.

**Technik**
- Migrationen `0014` bis `0020`: Verknüpfungen und Zustand der Anbindungen, gemeinsames Schreibprotokoll (übernimmt `filament_writes`), Abgleich verknüpfter Kontakte, Kalkulation und Druckvorbereitung je Teil, Platten je Druck und je Schätzung.
- Gerüst für einheitliche externe Anbindungen (`src-tauri/src/connectors/`): Registry, gemeinsamer HTTP-Zugang mit Drosselung und Schreibschutz, Schlüsselbund je Anbindung, `connectors.json`, ein Takt für alle Abgleiche. Plan in `docs/PLAN-Anbindungen.md`.
- Das Bambu-Konto läuft als Anbindung auf dem Gerüst; `bambu.json` und das Token werden beim ersten Start übernommen. Bambu Studio ist eine Slicer-Anbindung, Link-Import und MakerWorld-Download laufen über die gemeinsamen Zugänge. Ein Token kann nie an einen fremden Host gehen.
- Zuordnung von Drucken (`matching.rs`) und Verbrauchsschätzung (`usage.rs`) liegen im Kern, unabhängig vom Anbieter.
- Oberflächentests mit Playwright (`npm run test:e2e`, `e2e/`) gegen die Vorschau mit Beispieldaten, auch in der CI. Tests der Anbindungen laufen jetzt auch unter Linux (CI).
- Projekt-Skills `ausliefern` und `release` (`.claude/skills/`) für gleichbleibende Abläufe mit Prüfungen und Reviews.
- Entwicklerversion „Printmanager Dev“ (`npm run app:dev`, `npm run app:dev:build`) mit eigener Kennung, eigenem Datenordner und eigenem Schlüsselbund-Dienst; `npm run app:stable:install` installiert die stabile App nach `/Applications`.

## 0.3.0 – 17.11.2026

**Filament ins Bambu-Konto schreiben (optional, ab Werk aus)**
- Neuer Schalter **Einstellungen → Bambu-Konto → Änderungen ins Bambu-Konto schreiben**, mit Erklärung der Risiken beim Einschalten.
- **Rest zurückschreiben:** Nach dem Ändern des Rests, nach „Verbrauch abziehen“ und nach „Rückgängig“ fragt die App, ob der neue Rest ins Konto soll. Vorher prüft sie, ob sich die Spule dort inzwischen geändert hat (Konflikt), und schreibt dann nicht.
- **Spulen anlegen:** „+ Spule“ legt eine oder mehrere Spulen mit Marke, Material, Produkt, Name, Farbe und Gewicht im Konto an.
- **Spulen löschen:** Papierkorb auf jeder Spule, nach Rückfrage. Nicht für Spulen im Drucker oder mit RFID.
- **Vorrat:** Mit − und + änderst du die Zahl gleicher Spulen im Konto.
- **Protokoll** aller Schreibvorgänge mit „Rückgängig“ für den Rest.
- Spulen mit RFID: Den Rest pflegt das AMS, die App schreibt ihn nicht. Spulen im druckenden Drucker: Die Änderung wartet bis nach dem Druck.
- Drucker-Symbol zum Zuordnen eines AMS-Slots im Konto. Das Konto übernimmt die Zuordnung bisher nicht, die App meldet das.

**Verbessert**
- Der Filament-Abgleich holt jetzt alle Seiten aus dem Konto. Vorher fehlten bei vielen Spulen die neuesten.
- Werkzeugleisten passen auch bei der kleinsten Fenstergröße (960 px) in eine Zeile, die Suche schrumpft mit. Nur die Statistik bricht ihre Filter um.
- Spulenkarten sind aufgeräumter: Ausblenden als Symbol, Aktionen in einer Zeile.

**Technik**
- Schreibende Anfragen sind im Kern auf die Spulenliste beschränkt (`write_allowed`); alles andere im Konto bleibt lesend.
- Gemeinsame Drosselung für alle Anfragen ans Konto, neue Tests für Schreiben, Konflikt, Vorlage und Slot-Vergleich (43 Rust-Tests).
- 3MF lesen ohne Regex-Neubau pro Platte, toter Code entfernt (`order_move`, `spools_sync`, alte CSS-Regeln), `Modal` als eigene Datei.
- Migration `0013`: Spalte `rfid` und Tabelle `filament_writes`.

## 0.2.0 – 17.11.2026

- Kundenverwaltung mit Kundennummer, Adresse und abweichender Rechnungsadresse.
- Druckverlauf aus dem Bambu-Konto mit Filtern und Zuordnung zu Aufträgen; „Druck wählen …“ im Auftrag.
- Robuste Zuordnung von Drucken (Auftragsnummer im Dateinamen, Bewertung, Zuordnung von Hand).
- Geschätzter Verbrauch bei abgebrochenen Drucken.
- Kalkulation: geschätzte und tatsächliche Kosten, Preisvorschlag.
- Statistik über alle Drucke oder nur Aufträge.
- README als Produktseite.

## 0.1.0 – 16.11.2026

- Erste Version: Aufträge mit Board, Liste und Heute-Ansicht, Katalog mit MakerWorld, Filament aus Bambu Studio oder dem Konto, Druckerstatus, Bambu Studio öffnen und gesliced Dateien einlesen.
