# Änderungen

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
