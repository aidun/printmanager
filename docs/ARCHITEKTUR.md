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
│ threemf.rs, filament.rs, linkimport.rs, files.rs, slicers/      │
│ matching.rs, usage.rs (Fachlogik, anbieterunabhängig)           │
│ connectors/ ── Registry, HttpGateway (gedrosselt), Takt         │
│ bambu/     ── Anbindung bambu-cloud → api.bambulab.com          │
│              mod.rs (Descriptor, Anmeldung, Abgleiche)          │
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
- **`Arc<Registry>`** (`connectors/registry.rs`): alle Anbindungen mit Einstellungen, HTTP-Zugang und Geheimnissen.
- **`Bambu`** (`bambu/mod.rs`), Innenleben der Anbindung `bambu-cloud`:
  - Token im Speicher und im Schlüsselbund (`bambu-cloud:token`), HTTP über den gemeinsamen `HttpGateway`
  - Laufzeitzustand (verbunden, abgelaufen, letzte Fehler) und die zuletzt gesehenen Drucker
  - `BambuStatus` für die Oberfläche: Kontoangaben aus den Optionen, Schalter aus den Fähigkeiten
- **Einstellungen:**
  - `settings.json`: Datenordner, Datenbank-URL, Pfad zu Bambu Studio und zu `spools.json`
  - `connectors.json`: je Anbindung an/aus, Schalter je Fähigkeit und Optionen (bei Bambu: Konto-Name, uid, Ablauf). Beide liegen im Konfigurationsordner der App.
  - `bambu.json` (bis 0.3) wird beim ersten Start einmalig übernommen und bleibt liegen.

## Datenmodell (Migrationen 0001–0020)

| Tabelle | Inhalt |
|---|---|
| `people` | Kunden: fortlaufende Kundennummer (`number`, angezeigt als K-0001), Name (eindeutig ohne Groß-/Kleinschreibung), E-Mail, Telefon, Adresse (`street`, `house_no`, `zip`, `city`), optionale Rechnungsadresse (`billing_differs` + `billing_*`; ohne Haken geleert, mit Haken Straße und Ort Pflicht), Notiz. `address` ist die alte Freitext-Spalte und wird als Zeile mitgeführt. Jeder Auftrag braucht einen Kunden (geprüft in `create_order`/`update_order`) |
| `orders` | Auftrag: Nummer, Titel, Status, Priorität, Fälligkeit, Position in der Spalte, `archived_at` |
| `order_items` | Teile eines Auftrags: Menge, Farbwunsch, erledigt, Katalogbezug, Druckdatei |
| `attachments` | Anhänge: image, file, link, text; Pfad relativ zum Auftragsordner |
| `events` | Verlauf: created, status, note, attachment, edit, print |
| `catalog` | Katalogprojekte: Datei in `catalog/<id>/`, Vorschau, Filamente, Druckzeit; `origin` lokal/makerworld, `remote_id`, `cover_url`, `remote_missing` |
| `mw_collections`, `catalog_collections` | MakerWorld-Sammlungen und Zuordnung der Modelle. Die Katalogauswahl (`CatalogPicker.tsx`, `categoriesOf`) bildet daraus und aus `catalog.tags` Kategorien, nach Namen ohne Groß-/Kleinschreibung zusammengefasst. Die letzte Wahl liegt im Browser-Speicher (`pm.pickerCategory`) |
| `spools` | Spulen: `studio_grams` ist der Wert der Quelle, `remaining_grams` der Wert der App; `source` datei/konto, AMS-Slot, `rfid` |
| `external_links` | Verknüpfung lokaler Datensätze mit Anbindungen: `connector`, `entity`, `external_id`, `local_id`, `etag`, letzter gemeinsamer Stand beider Seiten (`remote_snapshot`, `local_snapshot` als JSON der zugeordneten Felder), `state` (ok, konflikt, geloescht), `conflict` (Felder). Heute: Kunden ↔ Apple Kontakte |
| `connector_state` | Zustand je Anbindung, z.B. Sync-Token (`connector`, `key`, `value`) |
| `item_estimates` | Kalkulation und Druckvorbereitung je Teil: Profilquelle, Drucker, Düse, Prozess- und Filament-Preset, Druckplatte, Basis (`slicer`, `makerworld`), Sekunden, Filamente (JSON), MakerWorld-Profil, Fehler; ab 0018 `slots` (JSON `[{slot, preset, material, color, spoolId}]`), `objects` (JSON `[{index, name, slot}]`), `setup_state` (`entwurf`, `bereit`, `uebergeben`), `project_file` |
| `connector_writes` | Protokoll der Schreibvorgänge aller Anbindungen: `connector`, `entity`, `local_id`, Aktion (`rest`, `anlegen`, `loeschen`, `slot`), alt/neu, Status (`ok`, `konflikt`, `fehler`, `wartet`, `uebersprungen`), Kopie vor dem Schreiben. Bis 0.3 `filament_writes` (Migration 0015 übernimmt die Einträge) |
| `print_history` | Druckverlauf aus dem Konto: Titel, Drucker, Start/Ende, geplante Dauer, Gramm je Slot, Vorschau, Zuordnung zum Auftrag |
| `print_progress` | Zuletzt gesehener Fortschritt laufender Drucke (für die Schätzung bei Abbrüchen) |
| `print_assignments` | Von Hand festgelegte Zuordnung Druckauftrag → Auftrag |
| `print_runs` | Drucke: gesliced Datei, Filamente (JSON), Ergebnis, Abzug; Rückmeldung des Druckers (`printer_*`, `actual_*`); ab 0019 `plate_details` (Platten einzeln) |

Statusfolge: `eingang → klaeren → vorbereiten → in_studio → druckt → nacharbeit → fertig → uebergeben`, daneben `pausiert` und `abgelehnt`.

## Abläufe

### Datei im Slicer öffnen
Der eingeschaltete Slicer (`Registry::slicer`, heute Bambu Studio) öffnet die Datei über `SlicerAdapter::open`, am Standardort oder am Pfad aus den Einstellungen. Die App startet nur das Programm, nie einen Druck.
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
- **Nur Spulen:** Der `HttpGateway` der Anbindung lässt nicht-lesende Anfragen nur auf die Spulenliste und ihr `/batch` durch (`DESCRIPTOR.write_paths`).
- **Schalter** „Spulen schreiben“ (`Cap::SpoolsWrite` in `connectors.json`), ab Werk aus. Ist er aus, blockiert der HTTP-Zugang jede schreibende Anfrage, bevor sie rausgeht.
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
- **Zuordnung** (`matching::find_order`, unabhängig vom Anbieter), in dieser Reihenfolge:
  1. **von Hand** festgelegt (`print_assignments`, Schlüssel = vereinheitlichter Druckauftragsname)
  2. **Auftragskennung** im Druckauftrag (`A0004 …`) → genau dieser offene Auftrag
  3. **Bewertung:** Wörter von Dateiname bzw. Titel vergleichen (ohne Endung, `_plate_N` und Kennung). Gleich = 3, alle Wörter des kürzeren im längeren = 2 (mindestens zwei Wörter oder ein Wort mit 4+ Zeichen). Dateien zählen doppelt. Bei Gleichstand gewinnt der Status (Druckt > In Studio > Vorbereiten/Nacharbeit), dann der neueste erkannte Druck, dann der neueste Auftrag.
- **Anzeige:** `PrinterView.matchedBy` (manuell, nummer, datei, titel) erscheint auf der Druckerkarte. Mit „Ändern“ ordnest du von Hand zu (`printer_assign`), mit „Automatisch zuordnen“ hebst du das wieder auf.
- **Fertige Drucke** kommen aus `my/tasks`: alle 15 Minuten und sofort nach einem erkannten Druckende.
  - Der Eintrag geht an den passenden Druck ohne Rückmeldung, der vor dem Ende angelegt wurde.
  - Jeder Eintrag aus dem Verlauf wird nur einmal zugeordnet (`printer_task_id`).
  - Das Ergebnis ist nur ein Vorschlag. Der Nutzer bestätigt es.

### Platten (Migration 0019)
- **Lesen:** `threemf::analyze` liefert je gesliced Platte `PlateInfo` (Index aus `<metadata key="index">`, Zeit, Gewicht, Filamente je Slot); `threemf::layout` die Platten des Projekts (`plater_id`, `plater_name`).
- **Druck:** Hat eine gesliced Datei mehrere Platten, speichert `watch.rs` sie als `print_runs.plate_details` (JSON `RunPlate`: geplant, `status` '' | ok | fehldruck | uebersprungen, tatsächlich, `taskId`). Bei einer Platte bleibt die Liste leer.
- **Zuordnen:** `link_history_run` trägt einen Druck aus dem Verlauf mit `plate_index` über `record_plate` an der offenen Platte ein (Druck, dessen Dateiname zum Titel passt, sonst der neueste mit dieser offenen Platte). Drucke mit mehreren Platten bekommen keine Rückmeldung als Ganzes; die Live-Rückmeldung (`record_printer_result`) lässt sie aus. Doppelte verhindert `plate_details LIKE '%"taskId":"<id>"%'`.
- **Gesamtstand** (`save_plates`, `plates_total`): Erst wenn keine Platte offen ist, setzt der Druck `printer_status` und die tatsächlichen Werte (Summe der gedruckten Platten). `filaments` und `print_seconds` zählen nur nicht übersprungene Platten, damit Abzug und Kalkulation stimmen. `run_plate_skip` überspringt oder öffnet eine Platte, nicht nach dem Abzug.
- **Kalkulation** (`orderUsage`): je Druck zurückgemeldet tatsächlich; bei Platten gedruckte tatsächlich, offene geplant; sonst geplant (neuester Stand je Datei, nicht wenn die Datei schon gedruckt ist). Basis `tatsaechlich`, `teilweise` oder `slicer`.
- **Vorab:** `item_estimates.plates` (gewählte Platten, leer = alle) und ab 0020 `plate_results` (Schätzung je Platte, `PlateEstimate`). `item_slice` speichert alle Platten und übernimmt die Summe der gewählten (`apply_plates`); `item_set_plates` rechnet ohne neues Slicen neu. Projekte (3MF) werden ohne `--arrange` gesliced, damit die Platten bleiben.
- **Geplanter Plattendruck je Teil:** `sync_plate_run` (beim Öffnen des Auftrags über `order_plates_sync`, nach `item_set_plates` und `item_slice`) legt für ein Teil mit 3MF und mehreren Platten einen Druck mit Signatur `platten:<teil>` an bzw. gleicht ihn ab (`upsert_plate_run`): nicht gewählte Platten `uebersprungen`, geplante Werte aus `plate_results`, gedruckte Platten bleiben. `relink_plate_history` hängt Drucke aus dem Verlauf, die vorher als eigener Druck (`verlauf:<id>`) übernommen wurden, an ihre Platte um. `watch.rs` übernimmt bei einer gesliced Datei mit gleichem Namen deren Werte je Platte (`adopt_sliced_plates`) statt einen zweiten Druck anzulegen. Ohne Projektdatei mit mehreren Platten wird ein ungenutzter Plattendruck entfernt.
- **Teileliste:** `order_plates` liefert je Teil mit 3MF und mehr als einer Platte die Platten (`model_settings.config`, sonst `plate_N.png`) und legt die Vorschaubilder unter `plates/<teil>/plate_N.png` im Datenordner ab (`threemf::plates_with_images`, liest keine Modelle). Die Oberfläche zeigt sie über das Asset-Protokoll.

### Druckverlauf (`bambu/history.rs`)
- **Lokale Kopie** von `my/tasks` in `print_history`.
  - Vollständig kurz nach dem Start: Seiten zu 50, zuerst mit `offset`, liefert das nichts Neues, mit `after=<letzte ID>`.
  - Danach alle 15 Minuten die erste Seite.
  - Die Zuordnung zum Auftrag bleibt beim Abgleich erhalten.
- **Automatisch:** `find_order` (Zuordnung von Hand, Kennung, Bewertung) gegen offene Aufträge, nur wenn der Druck nach dem Anlegen des Auftrags endete.
- **Von Hand** (`assign_history`): setzt `order_id`/`matched_by='manuell'` und hängt Dauer und Gramm an den neuesten Druck ohne Rückmeldung (`link_history_run`). Gibt es keinen, entsteht ein Druck mit Signatur `verlauf:<id>`. Aufheben entfernt diesen Druck wieder, sofern nichts abgezogen ist.
- **Automatisch zugeordnet und beendet** (Status 2 oder 3): Nach jedem Abgleich hängt `link_matched_history` Dauer und Gramm genauso an einen Druck im Auftrag, sofern noch kein Druck den Eintrag trägt (`printer_task_id`). Damit ist die Kalkulation „tatsächlich“, und der Abzug von den Spulen wird angeboten. Gilt auch für früher zugeordnete Einträge.
- **Statistik:** `Stats.history` liefert alle Einträge. Die Oberfläche schaltet zwischen „Alle Drucke (Konto)“ und „Nur Aufträge“ um.

### Verbrauch bei Abbrüchen (`usage::usage_factor`)
Nimmt ein neutrales Ergebnis (`JobOutcome`: fertig, abgebrochen, unbekannt); den Bambu-Status (2/3) übersetzt `bambu/history.rs`.
- **Faktor:** Fertig = 1. Bei einem Abbruch der Fortschritt beim Abbruch (`print_progress`, wird bei jeder Abfrage laufender Drucke gemerkt; gilt, wenn zwischen Start und Ende + 10 Min. gesehen), sonst (Laufzeit − 300 s) ÷ geplante Druckzeit, begrenzt auf 0–1. Ohne Angaben gilt der geplante Wert.
- **Speicherung:** `print_history.used_factor`, `used_method` und `progress_at_end`.
- **Verwendung:** Rückmeldungen an Aufträge (`match_finished`, `assign_history`) skalieren die Gramm bei Abbrüchen. Die Statistik nutzt bei Aufträgen `actual_filaments`, sonst `filaments`, und rechnet mit dem wirksamen Ergebnis (Ergebnis oder Rückmeldung des Druckers).

### Kalkulation (`pricing.rs`, `src/pricing.ts`, `CostBox.tsx`)
- **Werte:** `pricing.json` im Konfigurationsordner (`pricing_get`/`pricing_save`).
- **Rechnung** in der Oberfläche:
  - Kosten = (Material + Strom + Maschine) × (1 + Puffer) + Pauschale
  - Preis = (Kosten ohne Pauschale) × Faktor + Pauschale, aufgerundet, mindestens der Mindestpreis
  - Bei tatsächlichen Werten entfällt der Puffer.
- **Mengen** (`orderUsage`): tatsächlich (Drucke mit Rückmeldung, Ergebnis oder Abzug, auch Fehldrucke), sonst Slicer (neuester Stand je Datei), sonst je Teil (`itemsUsage`): Schätzung des Teils (`item_estimates`: Slicer im Hintergrund, sonst MakerWorld-Druckprofil), sonst Katalogprojekt, jeweils × Menge. Die Basis ist die unsicherste verwendete Quelle.
- **Kalkulation vorab** (`estimates.rs`, `profiles.rs`, `ItemCalc.tsx`): Je Teil Profilwahl in `item_estimates` (Migration 0017). `item_slice` holt die Presets aus der Profilquelle (Anbindung `printerprofiles`, Cache in `profiles/<quelle>/` im Datenordner), schreibt ein kleines Druckerprofil (`type: machine`, `inherits` = Bambu-Druckermodell aus `compatible_printers` des Prozess-Presets, `curr_bed_type`) und ruft `SlicerAdapter::slice` auf: Bambu Studio `--slice 0 --arrange 1 --ensure-on-bed --load-settings … --load-filaments … --export-3mf ergebnis.gcode.3mf` (Dateiname relativ zu `--outputdir`) im Arbeitsordner `slicing/<teil>/`, Zeitlimit 10 min, Log `bambu-studio.log`. Geprüft mit Bambu Studio 2.9.1: Ein 20-mm-Würfel auf H2C 0.4 ist in unter 1 s gesliced, und im Datenordner von Bambu Studio ändert sich nichts, auch wenn die Oberfläche läuft. Das Ergebnis liest `threemf.rs`. Nur mit eingeschalteter Fähigkeit `slicer-slice` (ab Werk aus). MakerWorld: `design/{id}` → `instances[]` (`prediction`, `weight`).
- **Druckvorbereitung je Teil** (`estimates.rs`, `threemf::layout`, `ItemCalc.tsx`): `item_setup` liefert den gespeicherten Stand und den Aufbau des Modells (Slots aus `project_settings.config`: `filament_colour`, `filament_type`, `filament_settings_id`; Objekte in der Reihenfolge von `model_settings.config` mit ihrem `extruder`, `fixed` bei Teilen mit eigenem Slot; `painted` bei `paint_color`). STL, OBJ und STEP gelten als ein Objekt. `item_set_profile` speichert Profil, Slots und Objekte (geprüft: Slots 1–16 eindeutig, Farben `#RRGGBB`, Objekte nur auf vorhandene Slots), setzt `filament` auf das Preset des ersten Slots und `setup_state` auf `bereit`, wenn Prozess und alle benutzten Slots ein Preset haben. Ändern sich Prozess, Platte, Slot-Presets oder Objekt-Slots, wird eine Slicer-Schätzung verworfen. `item_slice` lädt **ein** Filament-Preset (das des ersten Slots mit Preset): Mehrere verschiedene Presets lassen die Kommandozeile bei Druckern mit Düsenwechsel (H2C) mit `group_nozzle_info` abstürzen, und danach öffnet sich die Oberfläche von Bambu Studio. Mit einem Preset behält Bambu Studio die Slots des Projekts und liefert die Gramm je Slot (geprüft am 23.11.2026 mit 2.9.1). `--load-filament-ids` gilt je eingelesener Datei, nicht je Objekt: Bei 3MF schreibt `threemf::with_object_slots` geänderte Objekt-Slots in eine Kopie im Arbeitsordner (`modell.3mf`, Teile unverändert), bei STL geht ein Wert für die eine Datei mit. Die Schätzung übernimmt Material, Farbe und Spule der Slots (`Filament::slot` aus `slice_info.config`). Gleiche Filamente in zwei Slots bleiben getrennt: `threemf::analyze` fasst gesliced Filamente je Slot zusammen (`<filament id>`), nur ohne Slot-Nummer nach Material und Farbe; `RunFilament.slot` trägt den Slot weiter. `Layout.objects[].parts` nennt die Slots der Teile; Slots von Teilen mit eigener Farbe gelten als benutzt und lassen sich im Dialog nicht zusammenlegen. „Alle kalkulieren“ (`OrderPanel` → `sliceAll`) ruft je Teil `item_setup`, bei fehlendem Profil `defaultChoice` (`ItemCalc.tsx`, dieselben Vorgaben wie der Dialog) mit `item_set_profile`, dann `item_slice`, nacheinander. Ist Slicen ausgeschaltet oder fehlt der Slicer, bricht es ab. Spule → Preset: `presetFor` in `api.ts` (Marke und Sorte exakt, sonst gleiches Material, bevorzugt Bambu Basic).

### MakerWorld (`bambu/makerworld.rs`)
- **Abgleich:** beim Start und alle 6 Stunden.
  - Erst kommt die Sammlungsliste (`listlite`), dann pro eingeschalteter Sammlung `favorites/{id}`. Wenn nötig, blättert die App seitenweise.
  - Die Modelle landen als Katalogeinträge ohne Datei. Einträge mit derselben MakerWorld-Adresse werden verknüpft, nicht verdoppelt.
  - Modelle ohne Sammlung bekommen `remote_missing=1`. Gelöscht wird nichts.
- **3MF bei Bedarf:** `design/{id}` → Standard-Druckprofil → `instance/{id}/f3mf?type=download` → Download vom CDN → `describe_catalog_file`.

### Takt (`connectors::start`)
Ein gemeinsamer Takt für alle Anbindungen, Grundschritt 20 s. Bambu: Filament alle 15 min, Drucker jede Minute (beim Drucken alle 20 s), Druckverlauf alle 15 min (der erste vollständig), MakerWorld alle 6 h, Anmeldeprüfung alle 30 min. Wartende Schreibvorgänge ins Konto mit dem Drucker- oder Filament-Abgleich, wenn kein Drucker druckt. Fehler erscheinen in der Oberfläche nur bei abgelaufener Anmeldung.

## Externe Anbindungen (im Aufbau)

Ziel ist eine einheitliche Schicht für alle Außenschnittstellen (Anbindungen mit Fähigkeiten, gemeinsame Anmeldung, Drosselung, Schreibschutz und Abgleich), zuerst Umzug von Bambu und Co., danach Google- und Apple-Kontakte (nur lesen): [PLAN-Anbindungen.md](PLAN-Anbindungen.md).

Stand A6: Modul `src-tauri/src/connectors/` mit den Anbindungen Bambu-Konto (`bambu-cloud`), Web, Printables und Bambu Studio (`bambu-studio`).
- `mod.rs`: `Connector`-Trait, `Descriptor` mit Fähigkeiten (`Cap`), Schreib-Pfaden, Takt und Geheimnis-Namen.
- `config.rs`: `connectors.json`; eine Anbindung ist ab Werk aus, eingeschaltet sind Lesen an und Schreiben aus.
- `http.rs`: `HttpGateway` mit Drosselung und Schreibschutz (nicht-GET nur mit Schalter und auf `write_paths`).
- `secrets.rs`: Schlüsselbund, Dienst = Kennung der App (`de.printmanager.app`, Entwicklerversion `de.printmanager.app.dev`), Konto `<anbindung>:<name>`, `MemoryBackend` für Tests.
- `registry.rs`: Anbindungen der Plattform, Schalter, Verbinden/Trennen (Trennen löscht die Geheimnisse und schaltet aus).
- `scheduler.rs`: ein Takt für lesende Fähigkeiten, die an, verbunden und fällig sind.
- `commands.rs`: Befehle für Einstellungen → Anbindungen (`ConnectorsSection.tsx`): Liste, Verbinden/Trennen, An/Aus, Schalter je Fähigkeit, gemeinsames Schreibprotokoll mit Rückgängig.
- `PublicConnector`: öffentliche Dienste ohne Anmeldung, ab Werk an. Registriert: **Web** (Seitenvorschau, Bilder, CDN-Downloads; fremde Hosts nur ohne Token) und **Printables** (GraphQL über `read_post_paths`).
- Slicer (`connectors/slicer.rs`, `slicers/`): `SlicerAdapter` mit Programm finden/öffnen, Filament-Datei lesen und Projekt lesen. Anbindung **Bambu Studio** (`bambu-studio`, ab Werk an, ohne HTTP); Beobachter (`watch.rs`) und Befehle holen den Slicer über `Registry::slicer`. Ein weiterer Slicer (OrcaSlicer, PrusaSlicer) ist ein weiteres Modul in `slicers/`.
- Kontakte (`connectors/contacts.rs`, `contacts/apple.rs`, `contactsync.rs`): Fähigkeit `ContactSource` mit neutralem `ExtContact`. Anbindung **Apple Kontakte** (nur macOS, `objc2-contacts`, Freigabe über macOS, `NSContactsUsageDescription` in `src-tauri/Info.plist`). `contactsync.rs` ordnet Kontakt → Kunde zu, übernimmt und verknüpft Kunden und gleicht verknüpfte Kunden alle 30 min mit einem Drei-Wege-Vergleich je Feld ab. Nie schreibend.
- Link-Import (`linkimport.rs`): je Plattform ein `LinkResolver` (Printables, MakerWorld, Thingiverse); was keiner erkennt, bekommt die OpenGraph-Vorschau über den Web-Zugang. Eine neue Plattform ist ein weiterer Resolver.

## Bambu-Konto

### Anmeldung
- **Fenster:** Ein privates Tauri-Fenster öffnet `bambulab.com/en/sign-in`. Es hat keine Capability, also keinen Zugriff auf die Befehle der App.
- **Token:** Die App liest das Cookie `token`, legt es in den Schlüsselbund (`bambu-cloud:token`) und schließt das Fenster. Ein Token aus 0.3 (`bambu-token`) wird kopiert; „Abmelden“ löscht beide Einträge.
- **Ablauf:** Das Ablaufdatum steht im JWT als `exp`, sonst gilt heute + 90 Tage. 401/403 bedeutet „Anmeldung abgelaufen“.

### Endpunkte (inoffiziell, Host `https://api.bambulab.com`, mit `Authorization: Bearer`; alle `GET` außer den Schreibaufrufen für Filament)

| Zweck | Pfad | Wichtige Felder |
|---|---|---|
| Profil | `/v1/design-user-service/my/profile` | `name`, `handle`, `uid` |
| Vorlieben (uid) | `/v1/design-user-service/my/preference` | `uid` |
| Filament | `/v1/design-user-service/my/filament/v2` | `hits[]`: `id`, `filamentType`, `filamentName`, `filamentVendor`, `color`, `netWeight`, `totalNetWeight`, `inPrinter`, `deviceName`, `amsId`, `slotId`, `depleted`, `updatedAt` |
| Filament schreiben (nur mit Schalter) | `PUT /v1/design-user-service/my/filament/v2`, `POST` dito, `DELETE …/filament/v2/batch` | `PUT {id, filamentName, netWeight}` (ganze Zahl), `POST` ganzes Objekt ohne `id`, `DELETE {"ids": [n]}`; Antworten `{"filamentV2": …}` bzw. `{}` |
| Drucker | `/v1/iot-service/api/user/bind` | `devices[]`: `dev_id`, `name`, `dev_product_name`, `online`, `print_status` |
| Laufender Druck | `/v1/iot-service/api/user/print?force=true` | `devices[]`: `task_name`, `task_status`, `start_time`, `prediction`, `progress`. `progress` kam am 23.11.2026 beim H2C nie an (`print_progress` blieb leer). Ersatz: Schätzung aus dem Eintrag mit `status` 4 im Druckverlauf (`startTime`, `costTime`), sonst aus `start_time` und `prediction` (`printers::estimate`, höchstens 99 %, `progressEstimated`). In `print_progress` landen nur echte Werte |
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
