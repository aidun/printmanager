# Printmanager v3 – Plan (Desktop-App)

Stand: 16.11.2026 · Ersetzt den früheren Web-Plan v2

## Ziel

Eine **Desktop-App nur für dich** (MacBook und Windows-PC), konzipiert als **Workflow- und Auftragssystem**
für die Druckwünsche von Freunden und Familie:

- Wünsche erfassen: **Freitext**, **Links**, **Bilder** oder ein Projekt aus dem **Katalog**.
- Jeden Auftrag durch einen festen Ablauf führen, vom Eingang bis zur Übergabe.
- Zum Drucken öffnet die App das Modell **in Bambu Studio**. Profile, Filamente, AMS-Zuordnung, Slicen und Senden an den H2C
  machst du dort. Die App setzt **keine Profile oder Presets** (Entscheidung nach Phase A, siehe [PHASE-A.md](PHASE-A.md)).
- Die **Filament-Bibliothek** (deine Spulen aus Bambu Studio) ist eingebunden: als Übersicht und für den Abzug der verbrauchten Gramm.

## Entscheidungen

| Thema | Entscheidung | Folge |
|---|---|---|
| Nutzer | Nur du | Kein Login, kein Server, keine Freigabe nach außen |
| Plattform | **Tauri 2** (Mac + Windows) | Oberfläche in React/TypeScript, Kern in Rust, App ca. 10 MB |
| Daten | **SQLite**, später optional ein „richtiges“ DBMS (z.B. PostgreSQL) | Datenzugriff von Anfang an über eine Repository-Schicht mit portablem SQL |
| Slicen | **In Bambu Studio**, Datei wird unverändert geöffnet | Kein Slicer-Container, kein Bambu Connect, keine Profil-Logik |
| AMS | **Zuordnung in Bambu Studio** („AMS synchronisieren“ beim Drucken) | Drucker bleibt in der Bambu-Cloud, kein Entwicklermodus |
| Drucker | Nur H2C | Keine Druckerauswahl in der App |

## Was wir mitnehmen

**Aus dem PoC:** Ablauf und Oberfläche für Katalog und Warteschlange, die Logik des 3MF-Lesers (`threemf.js`, wird in Rust nachgebaut),
„Katalogprojekte nur ausblenden, wenn schon bestellt“, Farbwünsche als Freitext.

**Aus Phase 0** ([PHASE0.md](PHASE0.md)): In einer gesliced 3MF stehen Druckzeit und Gramm pro Filament in `slice_info.config`.

**Nicht mehr gebraucht:** Web-Server, Zugangscodes, Owner-Token, Slicer-Container, Bambu-Connect-Übergabe,
Auflösen von Profilen und Schreiben von Einstellungen in die 3MF.

## Kernfunktion „In Bambu Studio öffnen“

1. Die App legt für das Teil eine Arbeitskopie im Auftragsordner an (Katalog-3MF oder eigene STL/STEP/3MF).
2. Sie öffnet die Kopie mit Bambu Studio:
   - Mac: `open -a BambuStudio <datei>`
   - Windows: `bambu-studio.exe <datei>`
3. Daneben zeigt die App die Angaben aus dem Auftrag: Farbwunsch, Anzahl, Notizen, passende Spulen aus der Bibliothek.
4. Du wählst in Bambu Studio Profile und Filamente, slicst, druckst und **speicherst** das Projekt (es liegt im Auftragsordner).
5. Die App beobachtet den Auftragsordner. Sobald dort eine gesliced Datei liegt, liest sie Druckzeit und Gramm pro Filament
   aus und schlägt vor, die Gramm von den Spulen abzuziehen.

Hinweis: Ist in Bambu Studio schon ein Projekt offen, fragt Studio „Als Projekt öffnen?“ und eventuell „Speichern?“.

## Daten von Bambu Studio (nur lesen)

| | Mac | Windows |
|---|---|---|
| Spulen | `~/Library/Application Support/BambuStudio/filament_inventory/spools.json` | `%APPDATA%\BambuStudio\filament_inventory\spools.json` |

Die App schreibt **nie** in Dateien von Bambu Studio. Restmengen und eigene Ergänzungen zu Spulen speichert sie in ihrer eigenen Datenbank.
Die Pfade sind in den Einstellungen änderbar.

## Architektur

```
┌─────────────────────────── Tauri-App ───────────────────────────┐
│ Oberfläche (React + TypeScript + Vite)                          │
│   Board · Liste · Auftrag · Katalog · Bibliothek · Einstellungen│
│        │ invoke()                                               │
│ Kern (Rust)                                                     │
│   repo/      Repository-Schicht (sqlx: SQLite, später Postgres) │
│   threemf/   lesen: Vorschau, Titel, Druckzeit, Gramm          │
│   bambu/     Spulen lesen, Bambu Studio starten               │
│   watch/     Auftragsordner beobachten (notify)                 │
└─────────────────────────────────────────────────────────────────┘
        │ Dateien                          │ öffnet
   Datenordner (DB + Dateien)          Bambu Studio ──► H2C
```

- **Datenordner** (frei wählbar): `printmanager.db`, `orders/<nr>/` (Bilder, Anhänge, Arbeitsdateien),
  `catalog/<id>/` (3MF, Vorschau). Backup = diesen Ordner sichern.
- **Datenbank:** `sqlx` mit Migrationen. Wir schreiben nur SQL, das unter SQLite und PostgreSQL gleich läuft
  (keine SQLite-Spezialitäten, IDs als UUID/Text, Zeiten als ISO-Text).
  Wenn wir später PostgreSQL wollen, kommt eine zweite Implementierung der Repository-Schicht dazu,
  und die Dateien liegen dann auf einer Freigabe.
- **Mac und Windows:** Mit SQLite arbeitet die App jeweils mit dem Datenordner auf dem Rechner.
  Den Datenordner in iCloud oder OneDrive zu legen geht, aber nur, wenn die App nie auf beiden Rechnern gleichzeitig offen ist.
  Gleichzeitiges Arbeiten auf beiden Rechnern ist genau der Fall für die PostgreSQL-Option.

## Datenmodell (Kern)

- `people` – Personen (Name, Kontakt, Notiz).
- `orders` – Auftrag: Nummer, Person, Titel, Beschreibung, Status, Priorität, Fälligkeitsdatum, Position im Board.
- `order_items` – Teile eines Auftrags: Katalogprojekt **oder** eigene Datei, Anzahl, Farbwunsch, eigener Status.
- `attachments` – Bilder, Links, Dateien, eingefügter Text (z.B. aus WhatsApp) zu Auftrag oder Teil.
- `catalog` – Projekte: 3MF, Vorschau, Titel, Beschreibung, Platten, Filament-Slots, sichtbar ja/nein.
- `spools` – Spiegel von `spools.json` (Schlüssel `spool_id`) plus eigene Felder: Restgramm, ausgeblendet.
- `print_runs` – ein Druck zu einem Teil: Arbeitsdatei, Ergebnis aus der gesliced Datei (Druckzeit, Gramm pro Filament),
  verwendete Spulen, Erfolg/Fehldruck.
- `events` – Verlauf: Statuswechsel, Notizen, Drucke, mit Zeitstempel.

**Status eines Auftrags:**
Eingang → Klären → Vorbereiten → In Studio → Druckt → Nacharbeit → Fertig → Übergeben (oder Abgelehnt / Pausiert).
Das Board zeigt die Status als Spalten. Ein Auftrag mit mehreren Teilen ist erst „Fertig“, wenn alle Teile fertig sind.

## Phasen

**Phase A – Spike „Bambu Studio öffnen“ ✅ erledigt.**
Ergebnis in [PHASE-A.md](PHASE-A.md): Öffnen per Kommandozeile funktioniert. Profile vorbelegen ginge, ist aber zu aufwendig
und versionsabhängig, deshalb öffnet die App die Datei unverändert.

**Phase 1 – Grundgerüst und Aufträge ✅ umgesetzt (16.11.2026).**
Tauri-Projekt, Repository-Schicht + Migrationen, Personen, Aufträge mit Teilen und Anhängen (Bilder, Links und Text per Drag&Drop/Einfügen),
Board + Liste, Verlauf, Einstellungen (Datenordner). Der Web-PoC wandert nach `poc/`.

**Phase 2 – Katalog und Bibliothek.** *✅ umgesetzt (16.11.2026): Katalog, Link-Import, Auftrags-Assistent, „Druckdatei in Bambu Studio öffnen“ und die Filament-Bibliothek.*
3MF-Import in den Katalog (Rust-Port des 3MF-Lesers), Katalogprojekt als Teil zu einem Auftrag hinzufügen,
Spulen von Bambu Studio einlesen, Seite für die Bibliothek (Restmenge, ausblenden).

**Iteration 1 nach Phase 2 – Rückweg aus Bambu Studio.** *✅ umgesetzt (16.11.2026).* Die App beobachtet `orders/<nr>/` und `spools.json`. Eine gesliced gespeicherte 3MF wird zum Druck (Druckzeit, Gramm pro Filament), der Auftrag rückt auf „Druckt“. Verbrauch abziehen mit Spulen-Vorschlag (Material + ähnlichste Farbe), Warnung bei zu wenig Rest, Rückgängig, Ergebnis Erfolgreich/Fehldruck („Erfolgreich“ hakt das Teil ab). Farbpunkt am Teil zeigt die passende Spule zum Farbwunsch. Restmengen führt die App selbst; ändert sich der Rest in Bambu Studio, gilt der Wert aus Studio.

**Iteration 2 nach Phase 2 – Komfort im Ablauf.** *✅ umgesetzt (16.11.2026).* Startseite „Heute“: fällig (überfällig oder in 3 Tagen), Drucke auswerten (ohne Ergebnis oder ohne Abzug), neu & zu klären, druckt, Nacharbeit, zur Übergabe bereit, Filament knapp. Archiv: übergebene/abgelehnte Aufträge archivieren (einzeln im Auftrag oder alle über die Spalte „Übergeben“); sie verschwinden von Board und Liste, bleiben in Liste „Archiv“, Suche und Statistik; ein neuer Status holt sie zurück. Statistik: Filament, Druckzeit, Fehldrucke, Übergaben; Filament pro Monat; nach Person und nach Material; Zeitraum 12 Monate / dieses Jahr / alles. Nachrichtenvorlagen und Export bewusst weggelassen.

**Nächster Block – Bambu-Konto, Drucker, MakerWorld.** *geplant:* fortlaufend aktuelles Filament aus dem Bambu-Konto, Drucker mit Status (Konto, optional LAN), MakerWorld-Sammlungen und -Favoriten im Katalog. Details, Stufen und offene Entscheidungen: [PLAN-Bambu-Anbindung.md](PLAN-Bambu-Anbindung.md).

**Phase 3 – In Bambu Studio öffnen und Ergebnis übernehmen.**
Arbeitskopie anlegen und in Bambu Studio öffnen, Auftragsangaben daneben anzeigen, Ordner beobachten,
Druckzeit und Gramm übernehmen, Gramm von der Spule abziehen.

**Phase 4 – Feinschliff und Windows.**
Windows-Build und -Pfade, Export/Backup, kleine Statistik (Drucke, Gramm, pro Person), Suche und Filter.
Signieren/Notarisieren nur bei Bedarf. Für dich allein reicht ein unsignierter Build.

**Später (optional):** PostgreSQL als zweite Implementierung der Repository-Schicht. LAN-Entwicklermodus für Live-AMS.

## Offene Punkte

- **Bambu-Studio-Versionen:** Das Format von `slice_info.config` kann sich mit Updates ändern. Die App liest nur wenige Felder
  und zeigt eine Warnung, wenn sie eine Datei nicht versteht.
- **Windows-Pfad von Bambu Studio** wird in Phase 4 ermittelt (Registry oder Standardpfad, sonst Einstellung).
- **Daten aus dem Web-PoC:** Falls darin echte Aufträge stehen, übernehmen wir sie mit einem einmaligen Import.
