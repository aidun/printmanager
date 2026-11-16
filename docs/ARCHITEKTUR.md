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

## Datenmodell (Migrationen 0001–0007)

| Tabelle | Inhalt |
|---|---|
| `people` | Personen |
| `orders` | Auftrag: Nummer, Titel, Status, Priorität, Fälligkeit, Position in der Spalte, `archived_at` |
| `order_items` | Teile eines Auftrags: Menge, Farbwunsch, erledigt, Katalogbezug, Druckdatei |
| `attachments` | Anhänge: image, file, link, text; Pfad relativ zum Auftragsordner |
| `events` | Verlauf: created, status, note, attachment, edit, print |
| `catalog` | Katalogprojekte: Datei in `catalog/<id>/`, Vorschau, Filamente, Druckzeit; `origin` lokal/makerworld, `remote_id`, `cover_url`, `remote_missing` |
| `mw_collections`, `catalog_collections` | MakerWorld-Sammlungen und Zuordnung der Modelle |
| `spools` | Spulen: `studio_grams` ist der Wert der Quelle, `remaining_grams` der Wert der App; `source` datei/konto, AMS-Slot |
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

### Drucker (`bambu/printers.rs`)
- **Abfrage:** `bind` jede Minute, während eines Drucks alle 20 s. `print` nur, wenn etwas druckt.
- **Zuordnung:** Der Name des Druckauftrags wird normalisiert (ohne Endung, ohne `_plate_N`, nur Buchstaben und Ziffern) und mit den 3MF-Dateien und Titeln offener Aufträge verglichen.
- **Fertige Drucke** kommen aus `my/tasks`: alle 15 Minuten und sofort nach einem erkannten Druckende.
  - Der Eintrag geht an den passenden Druck ohne Rückmeldung, der vor dem Ende angelegt wurde.
  - Jeder Eintrag aus dem Verlauf wird nur einmal zugeordnet (`printer_task_id`).
  - Das Ergebnis ist nur ein Vorschlag. Der Nutzer bestätigt es.

### MakerWorld (`bambu/makerworld.rs`)
- **Abgleich:** beim Start und alle 6 Stunden.
  - Erst kommt die Sammlungsliste (`listlite`), dann pro eingeschalteter Sammlung `favorites/{id}`. Wenn nötig, blättert die App seitenweise.
  - Die Modelle landen als Katalogeinträge ohne Datei. Einträge mit derselben MakerWorld-Adresse werden verknüpft, nicht verdoppelt.
  - Modelle ohne Sammlung bekommen `remote_missing=1`. Gelöscht wird nichts.
- **3MF bei Bedarf:** `design/{id}` → Standard-Druckprofil → `instance/{id}/f3mf?type=download` → Download vom CDN → `describe_catalog_file`.

### Takt (`bambu::start`)
Ein Schritt dauert 20 s. Filament alle 45 Schritte, Drucker alle 3 Schritte (beim Drucken jeden Schritt), Druckverlauf alle 45 Schritte, MakerWorld alle 1080 Schritte, Anmeldeprüfung alle 90 Schritte.

## Bambu-Konto

### Anmeldung
- **Fenster:** Ein privates Tauri-Fenster öffnet `bambulab.com/en/sign-in`. Es hat keine Capability, also keinen Zugriff auf die Befehle der App.
- **Token:** Die App liest das Cookie `token`, legt es in den Schlüsselbund und schließt das Fenster.
- **Ablauf:** Das Ablaufdatum steht im JWT als `exp`, sonst gilt heute + 90 Tage. 401/403 bedeutet „Anmeldung abgelaufen“.

### Endpunkte (inoffiziell, Host `https://api.bambulab.com`, alle `GET` mit `Authorization: Bearer`)

| Zweck | Pfad | Wichtige Felder |
|---|---|---|
| Profil | `/v1/design-user-service/my/profile` | `name`, `handle`, `uid` |
| Vorlieben (uid) | `/v1/design-user-service/my/preference` | `uid` |
| Filament | `/v1/design-user-service/my/filament/v2` | `hits[]`: `id`, `filamentType`, `filamentName`, `filamentVendor`, `color`, `netWeight`, `totalNetWeight`, `inPrinter`, `deviceName`, `amsId`, `slotId`, `depleted`, `updatedAt` |
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
