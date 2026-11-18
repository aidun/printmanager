# Änderungen

## Unveröffentlicht

**Neu**
- **Kunden aus Kontakten (macOS):** Anbindung „Apple Kontakte“ (nur lesen). „Aus Kontakten …“ auf der Kundenseite und im Assistenten legt Kunden aus Kontakten an, „Mit Kontakt verknüpfen …“ verbindet bestehende. Verknüpfte Kunden bleiben aktuell (alle 30 Minuten); haben sich Kontakt und Kunde beide geändert, entscheidest du.
- Einstellungen → **Anbindungen**: alle Dienste und Programme an einer Stelle (Bambu-Konto, Bambu Studio, Web, Printables) mit Anmeldung, An/Aus und Schaltern je Fähigkeit; ein gemeinsames Protokoll aller geschriebenen Änderungen. Drucker und Druckverlauf haben jetzt getrennte Schalter.

**Technik**
- Gerüst für einheitliche externe Anbindungen (`src-tauri/src/connectors/`): Registry, gemeinsamer HTTP-Zugang mit Drosselung und Schreibschutz, Schlüsselbund je Anbindung, Einstellungen `connectors.json`, ein Takt für alle Abgleiche. Noch ohne sichtbare Änderung; Plan in `docs/PLAN-Anbindungen.md`.
- Migrationen `0014` (`external_links`, `connector_state`), `0015` (`connector_writes`, übernimmt `filament_writes`) und `0016` (Abgleich verknüpfter Kontakte).
- Bambu-Konto läuft als erste Anbindung auf dem Gerüst: gemeinsamer HTTP-Zugang (Schreibschutz über den Schalter „Spulen schreiben“), gemeinsamer Takt, Einstellungen in `connectors.json`. `bambu.json` und das Token werden beim ersten Start übernommen; ein Zurückwechseln auf 0.3 bleibt angemeldet.
- Bambu Studio ist eine Slicer-Anbindung (Programm öffnen, `spools.json`, gesliced 3MF lesen); weitere Slicer lassen sich gleichartig ergänzen.
- Link-Import und MakerWorld-Download laufen über die gemeinsamen Zugänge (neue öffentliche Anbindungen Web und Printables, je Plattform ein Resolver). Ein Token kann nie an einen fremden Host gehen.
- Zuordnung von Drucken zu Aufträgen (`matching.rs`) und Verbrauchsschätzung (`usage.rs`) aus dem Bambu-Modul in den Kern verschoben; die Schätzung nimmt ein neutrales Druckergebnis.

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
