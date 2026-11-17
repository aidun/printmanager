# Architektur

Stand: 16.11.2026. Regeln und Befehle stehen in [AGENTS.md](../AGENTS.md).

## Überblick

```
┌──────────────── Tauri-Fenster (React) ────────────────┐
│  App.tsx · Ansichten · api.ts (invoke)                │
└──────────────▲─────────────────────┬──────────────────┘
               │ Events              │ Befehle (invoke)
               │ pm:external         ▼
┌──────────────┴──────── Rust-Kern ───────────────────────────────┐
│ commands.rs ── store.rs (sqlx Any) ── SQLite / PostgreSQL       │
│ watch.rs   ── notify: orders/<Nr>/*.3mf, spools.json            │
│ threemf.rs, filament.rs, linkimport.rs, files.rs                │
│ bambu/     ── cloud.rs (GET, gedrosselt) → api.bambulab.com     │
│              mod.rs (Anmeldung, Schlüsselbund, Takt)            │
│              printers.rs · makerworld.rs                        │
└─────────────────────────────────────────────────────────────────┘
        │ öffnet Dateien                     ▲ liest gesliced 3MF
        ▼                                    │
   Bambu Studio  ───── speichert im Auftragsordner ─────┘
```

## Zustand im Kern

- **`AppState`** (`commands.rs`):
  - `ctx: RwLock<Ctx>` mit Store, Datenordner und Einstellungen
  - `settings_file`, `default_data_dir`, `default_spools_file`
  - `watcher` (notify)
  - Ein Wechsel des Datenordners baut `Ctx` neu und startet den Beobachter neu.
- **`Bambu`** (`bambu/mod.rs`):
  - Cloud-Client und Token, beides nur im Speicher und im Schlüsselbund
  - `BambuStatus` mit `BambuConfig` aus `bambu.json`
  - die zuletzt gesehenen Drucker
- **Einstellungen:**
  - `settings.json`: Datenordner, Datenbank-URL, Pfad zu Bambu Studio und zu `spools.json`
  - `bambu.json`: Konto-Name, uid, Ablauf, Schalter. Beide liegen im Konfigurationsordner der App.

## Datenmodell (Migrationen 0001–0013)

| Tabelle | Inhalt |
|---|---|
| `people` | Kunden: fortlaufende Kundennummer (`number`, angezeigt als K-0001), Name (eindeutig ohne Groß-/Kleinschreibung), E-Mail, Telefon, Adresse (`street`, `house_no`, `zip`, `city`), optionale Rechnungsadresse (`billing_differs` + `billing_*`; ohne Haken geleert, mit Haken Straße und Ort Pflicht), Notiz. `address` ist die alte Freitext-Spalte und wird als Zeile mitgeführt. Jeder Auftrag braucht einen Kunden (geprüft in `create_order`/`update_order`) |
| `orders` | Auftrag: Nummer, Titel, Status, Priorität, Fälligkeit, Position in der Spalte, `archived_at` |
| `order_items` | Teile eines Auftrags: Menge, Farbwunsch, erledigt, Katalogbezug, Druckdatei |
| `attachments` | Anhänge: image, file, link, text; Pfad relativ zum Auftragsordner |
| `events` | Verlauf: created, status, note, attachment, edit, print |
| `catalog` | Katalogprojekte: Datei in `catalog/<id>/`, Vorschau, Filamente, Druckzeit; `origin` lokal/makerworld, `remote_id`, `cover_url`, `remote_missing` |
| `mw_collections`, `catalog_collections` | MakerWorld-Sammlungen und Zuordnung der Modelle |
| `spools` | Spulen: `studio_grams` ist der Wert der Quelle, `remaining_grams` der Wert der App; `source` datei/konto, AMS-Slot, `rfid` |
| `filament_writes` | Protokoll der Schreibvorgänge ins Konto: Spule, Aktion (`rest`, `anlegen`, `loeschen`), alt/neu, Status (`ok`, `konflikt`, `fehler`, `wartet`, `uebersprungen`), Kopie der Spule vor dem Schreiben |
| `print_history` | Druckverlauf aus dem Konto: Titel, Drucker, Start/Ende, geplante Dauer, Gramm je Slot, Vorschau, Zuordnung zum Auftrag |
| `print_progress` | Zuletzt gesehener Fortschritt laufender Drucke (für die Schätzung bei Abbrüchen) |
| `print_assignments` | Von Hand festgelegte Zuordnung Druckauftrag → Auftrag |
| `print_runs` | Drucke: gesliced Datei, Filamente (JSON), Ergebnis, Abzug; Rückmeldung des Druckers (`printer_*`, `actual_*`) |

Statusfolge: `eingang → klaeren → vorbereiten → in_studio → druckt → nacharbeit → fertig → uebergeben`, daneben `pausiert` und `abgelehnt`.

## Abläufe

### Datei in Bambu Studio öffnen
`launch_studio` öffnet die Datei in Bambu Studio, am Standardort oder am Pfad aus den Einstellungen.
Steht der Auftrag vor `in_studio`, rückt er auf `in_studio` vor.

### Rückweg aus Bambu Studio (`watch.rs`)
- **Beobachtung:** notify überwacht `orders/` rekursiv. Nach 2 s Ruhe wertet `handle_file` jede `orders/<Nr>/<name>.3mf` aus.
- **Anhänge:** Eine unbekannte Datei wird zum Anhang. Eine bekannte zählt nur, wenn sie mehr als 10 s nach dem Anhängen geändert wurde.
- **Druck anlegen:**
  - Ist die Datei gesliced, entsteht ein `print_run`.
  - Die Signatur aus Druckzeit, Platten und Filamenten verhindert Doppelte.
  - Der Status wechselt auf `druckt`.
- **Meldung:** über `pm:external`.

### Filament
- **Quelle:** Mit Konto und eingeschaltetem Schalter kommen die Spulen aus `filament/v2`, sonst aus `spools.json`.
- **Regel beim Abgleich:**
  - Neue Spule: Rest = Wert der Quelle.
  - Hat sich der Wert der Quelle geändert, gilt er.
  - Sonst bleibt der Rest der App, der aus den Abzügen kommt.
  - Eine ältere `spools.json` überschreibt keine Werte aus dem Konto.
  - Eine leere Kontoliste wird ignoriert.
  - Fehlende Spulen bekommen `missing=1`.
- **Abzug:** Den Verbrauch eines Drucks zieht die App ab (`deduct_run`), mit Rückgängig. Der Spulen-Vorschlag folgt Material, dann Farbabstand, dann ausreichendem Rest (`rankSpools` in `api.ts`).

### Ins Konto schreiben (`bambu/filament_write.rs`, `src/AccountWrite.tsx`)
Hintergrund und Testergebnisse: [history/PLAN-Filament-Schreiben.md](history/PLAN-Filament-Schreiben.md).
- **Nur Spulen:** `Bambu::send_raw` lehnt jeden Pfad außer der Spulenliste und ihrem `/batch` ab (`write_allowed`).
- **Schalter** `filament_write` in `bambu.json`, ab Werk `false`. Ist er aus, lehnt der Kern jeden Schreibbefehl ab.
- **Rest** (`spool_push_rest`), in dieser Reihenfolge:
  1. Nur Spulen mit `source = konto`. RFID-Spulen: Status `rfid`, nichts geschrieben.
  2. Spule im druckenden Drucker: Status `wartet`. Der Takt schreibt den neuesten wartenden Wert pro Spule, sobald kein Drucker druckt (`process_pending`).
  3. Spule frisch lesen (`?ids=`, sonst seitenweise).
  4. **Konflikt**, wenn `updatedAt` vom letzten Abgleich abweicht und `netWeight` anders als `studio_grams` ist (`conflict`). Mit `force` wird trotzdem geschrieben.
  5. `PUT` mit `{id, filamentName, netWeight}` (ganze Gramm), dann erneut lesen und vergleichen.
  6. `studio_grams = remaining_grams = Wert` und neuer Zeitstempel, damit der nächste Abgleich nichts zurücksetzt.
- **Anlegen:** `spool_create` nimmt eine vorhandene Spule als Vorlage (gleiche Marke, Material und Produkt, sonst gleiches Material), entfernt ID und Gerätefelder und setzt die neuen Werte. Farbe als `#RRGGBBAA`. Bei anderem Material entfallen `filamentId` und `category` der Vorlage. Bis zu 10 Stück, danach werden die neuen IDs durch erneutes Lesen gefunden.
- **Slot zuordnen:** `spool_assign_slot` schickt `inPrinter`, `devId`, `amsId`, `slotId`. Das Konto antwortet 2xx, **ignoriert die Felder aber** (Test 17.11.2026).
- **Vorrat:** `spool_stock_add` legt per `POST` eine Kopie der Vorlage ohne ID und Gerätefelder an, voll (`netWeight = totalNetWeight`). `spool_stock_remove` löscht per `DELETE /batch` eine Spule, nicht im Drucker und ohne RFID. Beide gleichen danach ab.
- **Protokoll:** jeder Versuch in `filament_writes`. `filament_write_undo` schreibt den alten Rest eines `ok`-Eintrags mit `force` zurück.
- **Oberfläche:** `useAccountWrite` öffnet nach einer Rest-Änderung (Filament-Seite, Abzug, Rückgängig) die Vorschau alt → neu. Geschrieben wird erst nach Klick.

### Drucker (`bambu/printers.rs`)
- **Abfrage:** `bind` jede Minute, während eines Drucks alle 20 s. `print` nur, wenn etwas druckt.
- **Auftragskennung:** Modelldateien im Auftragsordner heißen `A0004 name.ext`. Sie bekommen die Kennung beim Ablegen (Drag&Drop, Einfügen, Katalog, Link-Import), ältere Dateien offener Aufträge beim Start der App (`tag_open_model_files`) und beim Öffnen in Bambu Studio (`tag_model_file` → `rename_attachment`; erst Datenbank, dann Datei, erkannte Drucke ziehen mit). Bambu Studio übernimmt den Namen als Projekt- und Druckauftragsnamen.
- **Zuordnung** (`find_order`), in dieser Reihenfolge:
  1. **von Hand** festgelegt (`print_assignments`, Schlüssel = vereinheitlichter Druckauftragsname)
  2. **Auftragskennung** im Druckauftrag (`A0004 …`) → genau dieser offene Auftrag
  3. **Bewertung:** Wörter von Dateiname bzw. Titel vergleichen (ohne Endung, `_plate_N` und Kennung). Gleich = 3, alle Wörter des kürzeren im längeren = 2 (mindestens zwei Wörter oder ein Wort mit 4+ Zeichen). Dateien zählen doppelt. Bei Gleichstand gewinnt der Status (Druckt > In Studio > Vorbereiten/Nacharbeit), dann der neueste erkannte Druck, dann der neueste Auftrag.
- **Anzeige:** `PrinterView.matchedBy` (manuell, nummer, datei, titel) erscheint auf der Druckerkarte. Mit „Ändern“ ordnest du von Hand zu (`printer_assign`), mit „Automatisch zuordnen“ hebst du das wieder auf.
- **Fertige Drucke** kommen aus `my/tasks`: alle 15 Minuten und sofort nach einem erkannten Druckende.
  - Der Eintrag geht an den passenden Druck ohne Rückmeldung, der vor dem Ende angelegt wurde.
  - Jeder Eintrag aus dem Verlauf wird nur einmal zugeordnet (`printer_task_id`).
  - Das Ergebnis ist nur ein Vorschlag. Der Nutzer bestätigt es.

### Druckverlauf (`bambu/history.rs`)
- **Lokale Kopie** von `my/tasks` in `print_history`.
  - Vollständig kurz nach dem Start: Seiten zu 50, zuerst mit `offset`, liefert das nichts Neues, mit `after=<letzte ID>`.
  - Danach alle 15 Minuten die erste Seite.
  - Die Zuordnung zum Auftrag bleibt beim Abgleich erhalten.
- **Automatisch:** `find_order` (Zuordnung von Hand, Kennung, Bewertung) gegen offene Aufträge, nur wenn der Druck nach dem Anlegen des Auftrags endete.
- **Von Hand** (`assign_history`): setzt `order_id`/`matched_by='manuell'` und hängt Dauer und Gramm an den neuesten Druck ohne Rückmeldung. Gibt es keinen, entsteht ein Druck mit Signatur `verlauf:<id>`. Aufheben entfernt diesen Druck wieder, sofern nichts abgezogen ist.
- **Statistik:** `Stats.history` liefert alle Einträge. Die Oberfläche schaltet zwischen „Alle Drucke (Konto)“ und „Nur Aufträge“ um.

### Verbrauch bei Abbrüchen (`history::usage_factor`)
- **Faktor:** Fertig = 1. Bei einem Abbruch der Fortschritt beim Abbruch (`print_progress`, wird bei jeder Abfrage laufender Drucke gemerkt; gilt, wenn zwischen Start und Ende + 10 Min. gesehen), sonst (Laufzeit − 300 s) ÷ geplante Druckzeit, begrenzt auf 0–1. Ohne Angaben gilt der geplante Wert.
- **Speicherung:** `print_history.used_factor`, `used_method` und `progress_at_end`.
- **Verwendung:** Rückmeldungen an Aufträge (`match_finished`, `assign_history`) skalieren die Gramm bei Abbrüchen. Die Statistik nutzt bei Aufträgen `actual_filaments`, sonst `filaments`, und rechnet mit dem wirksamen Ergebnis (Ergebnis oder Rückmeldung des Druckers).

### Kalkulation (`pricing.rs`, `src/pricing.ts`, `CostBox.tsx`)
- **Werte:** `pricing.json` im Konfigurationsordner (`pricing_get`/`pricing_save`).
- **Rechnung** in der Oberfläche:
  - Kosten = (Material + Strom + Maschine) × (1 + Puffer) + Pauschale
  - Preis = (Kosten ohne Pauschale) × Faktor + Pauschale, aufgerundet, mindestens der Mindestpreis
  - Bei tatsächlichen Werten entfällt der Puffer.
- **Mengen** (`orderUsage`): tatsächlich (Drucke mit Rückmeldung, Ergebnis oder Abzug, auch Fehldrucke), sonst Slicer (neuester Stand je Datei), sonst Katalogprojekte der Teile × Menge.

### MakerWorld (`bambu/makerworld.rs`)
- **Abgleich:** beim Start und alle 6 Stunden.
  - Erst kommt die Sammlungsliste (`listlite`), dann pro eingeschalteter Sammlung `favorites/{id}`. Wenn nötig, blättert die App seitenweise.
  - Die Modelle landen als Katalogeinträge ohne Datei. Einträge mit derselben MakerWorld-Adresse werden verknüpft, nicht verdoppelt.
  - Modelle ohne Sammlung bekommen `remote_missing=1`. Gelöscht wird nichts.
- **3MF bei Bedarf:** `design/{id}` → Standard-Druckprofil → `instance/{id}/f3mf?type=download` → Download vom CDN → `describe_catalog_file`.

### Takt (`bambu::start`)
Ein Schritt dauert 20 s. Filament alle 45 Schritte, Drucker alle 3 Schritte (beim Drucken jeden Schritt), Druckverlauf alle 45 Schritte, MakerWorld alle 1080 Schritte, Anmeldeprüfung alle 90 Schritte. Wartende Schreibvorgänge ins Konto mit dem Druckerabgleich, wenn kein Drucker druckt.

## Bambu-Konto

### Anmeldung
- **Fenster:** Ein privates Tauri-Fenster öffnet `bambulab.com/en/sign-in`. Es hat keine Capability, also keinen Zugriff auf die Befehle der App.
- **Token:** Die App liest das Cookie `token`, legt es in den Schlüsselbund und schließt das Fenster.
- **Ablauf:** Das Ablaufdatum steht im JWT als `exp`, sonst gilt heute + 90 Tage. 401/403 bedeutet „Anmeldung abgelaufen“.

### Endpunkte (inoffiziell, Host `https://api.bambulab.com`, mit `Authorization: Bearer`; alle `GET` außer den Schreibaufrufen für Filament)

| Zweck | Pfad | Wichtige Felder |
|---|---|---|
| Profil | `/v1/design-user-service/my/profile` | `name`, `handle`, `uid` |
| Vorlieben (uid) | `/v1/design-user-service/my/preference` | `uid` |
| Filament | `/v1/design-user-service/my/filament/v2` | `hits[]`: `id`, `filamentType`, `filamentName`, `filamentVendor`, `color`, `netWeight`, `totalNetWeight`, `inPrinter`, `deviceName`, `amsId`, `slotId`, `depleted`, `updatedAt` |
| Filament schreiben (nur mit Schalter) | `PUT /v1/design-user-service/my/filament/v2`, `POST` dito, `DELETE …/filament/v2/batch` | `PUT {id, filamentName, netWeight}` (ganze Zahl), `POST` ganzes Objekt ohne `id`, `DELETE {"ids": [n]}`; Antworten `{"filamentV2": …}` bzw. `{}` |
| Drucker | `/v1/iot-service/api/user/bind` | `devices[]`: `dev_id`, `name`, `dev_product_name`, `online`, `print_status` |
| Laufender Druck | `/v1/iot-service/api/user/print` | `devices[]`: `task_name`, `task_status`, Fortschritt und Restzeit (Feldnamen ungeprüft) |
| Druckverlauf | `/v1/user-service/my/tasks?limit=20` | `hits[]`: `id`, `status` (2 ok, 3 Fehler), `startTime`, `endTime`, `title`, `deviceName`, `amsDetailMapping[]` |
| Sammlungen | `/v1/design-service/my/favorites/listlite` | `default`, `hits[]`: `id`, `title`, `designCnt`, `isDefault` |
| Sammlung | `/v1/design-service/favorites/{id}` bzw. `…/{id}/designs?limit=20&offset=N` | `designs[]` bzw. `hits[]`, `total`, `hiddenCnt` |
| Modell | `/v1/design-service/design/{id}` | `title`, `summary`, `license`, `defaultInstanceId`, `instances[]` |
| 3MF | `/v1/design-service/instance/{id}/f3mf?type=download` | `name`, `url` (signiert, CDN `makerworld.bblmw.com`) |

Herkunft: Community-Dokumentation (OpenBambuAPI, ha-bambulab, 3d-model-manager), bestätigt im Spike vom 16.11.2026 ([history/PLAN-Bambu-Anbindung.md](history/PLAN-Bambu-Anbindung.md)).

## Oberfläche

- **`App.tsx`:**
  - lädt Aufträge bei `pm:changed`, bei Fokus und alle 20 s
  - empfängt `pm:external`
  - verteilt Drag&Drop: neuer Auftrag, Anhang oder Katalog
- **`api.ts`:** Nach jedem Befehl, der nicht auf der `READ_ONLY`-Liste steht, löst es `pm:changed` aus.
- **Designsystem:** Tokens in `styles.css`, siehe AGENTS.md. Die Vorschau ohne Tauri übernimmt `devmock.ts` (`mockIPC`), geladen nur bei `import.meta.env.DEV` außerhalb von Tauri.
