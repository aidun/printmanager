# AGENTS.md – Hinweise für Coding-Agenten und Entwickler

Printmanager ist eine Tauri-2-Desktop-App für **einen** Nutzer (macOS und Windows): Druckaufträge, Katalog, Filament, Drucker. Sie bindet Bambu Studio und das Bambu-Konto **nur lesend** an.
Die Architektur im Detail steht in [docs/ARCHITEKTUR.md](docs/ARCHITEKTUR.md), die Bedienung in [docs/BENUTZERHANDBUCH.md](docs/BENUTZERHANDBUCH.md), frühere Pläne und Entscheidungen in [docs/history/](docs/history/).

## Harte Regeln

1. **Kein Steuern von Druckern.** Die App startet, pausiert und stoppt keine Drucke und sendet keine Befehle an Drucker, auch nicht im LAN oder per MQTT. Drucken bleibt in Bambu Studio oder Bambu Handy.
2. **Bambu-Konto und MakerWorld nur lesen.** Nur `GET` auf `api.bambulab.com`, gedrosselt auf höchstens 1 Anfrage pro Sekunde (`connectors/http.rs`, `bambu::DESCRIPTOR`). Nichts am Konto, an Spulen oder an Sammlungen ändern.
   **Einzige Ausnahme:** `bambu/filament_write.rs` schreibt Spulen (Rest, Anlegen, Löschen), nur bei eingeschaltetem Schalter `filament_write` (ab Werk aus) und nur nach Bestätigung durch den Nutzer. Neue Schreibaufrufe gehören nur dorthin. Ein Agent testet sie nur an einer vom Nutzer angelegten Testspule.
3. **Nie in Dateien von Bambu Studio schreiben**, weder in `spools.json` noch in `BambuStudio.conf` oder ähnliche.
4. **Geheimnisse:**
   - Das Bambu-Token liegt nur im Schlüsselbund des Systems (`keyring`, Dienst `de.printmanager.app`).
   - Es gehört nie in Datenbank, `connectors.json`, Logs, Tests oder Commits.
   - Zugangscodes von Druckern liest und speichert die App nicht.
   - Nach Zugangsdaten fragt ein Agent nicht.
5. **Nutzerdaten:** Den Datenordner unter `~/Library/Application Support/de.printmanager.app` löscht oder ändert ein Agent nur nach ausdrücklicher Zustimmung. Zum Ausprobieren dienen `npm run dev` mit Beispieldaten oder Tests mit Temp-Ordnern.
6. **Sprache:** Oberfläche, Fehlermeldungen, Kommentare und Doku sind auf Deutsch, kurz und verständlich. Bezeichner im Code sind englisch.

## Stack

| Teil | Technik |
|---|---|
| Oberfläche | React 19, TypeScript, Vite, Icons `lucide-react`, ein Stylesheet `src/styles.css` |
| Kern | Rust, Tauri 2, sqlx 0.8 (`Any`-Treiber), tokio, reqwest (rustls), notify, keyring |
| Datenbank | SQLite im Datenordner, optional PostgreSQL (gleiche Migrationen) |

## Befehle

```bash
npm install
```

```bash
npm run app:dev
```

Startet die Entwicklerversion „Printmanager Dev“ mit Hot Reload. Sie hat eine eigene Kennung (`de.printmanager.app.dev`), einen eigenen Datenordner und eigene Einträge im Schlüsselbund, und beeinträchtigt die installierte App nicht.

```bash
npm run dev
```

Startet nur die Oberfläche unter http://localhost:1420 mit Beispieldaten (`src/devmock.ts`).

```bash
npx tsc -b
```

Typprüfung der Oberfläche.

```bash
cd src-tauri && cargo test
```

Alle Rust-Tests laufen offline.

```bash
cd src-tauri && cargo clippy
```

```bash
cd src-tauri && PM_NET_TESTS=1 cargo test
```

Dazu kommen echte Abrufe bei Printables und MakerWorld.

```bash
npm run app:dev:build
```

Baut die Entwicklerversion unter `src-tauri/target/debug/bundle/macos/Printmanager Dev.app` und startet sie.

```bash
npm run app:stable:install
```

Baut die stabile App (Release) und installiert sie nach `/Applications/Printmanager.app`. Das macht nur der Nutzer oder ein Agent auf seinen Wunsch.

**CI und Release:**
- Die CI (`.github/workflows/ci.yml`) läuft bei jedem Push mit `cargo clippy --all-targets -- -D warnings`. Jede Warnung lässt den Lauf fehlschlagen.
- Für ein Release erhöhst du die Version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` und `package.json` gleichzeitig und pushst dann einen Tag `vX.Y.Z`.
- Releases entstehen als Entwurf, veröffentlicht werden sie nur vom Nutzer.

**Vor jedem Commit:** `npx tsc -b`, `cargo test` und `cargo clippy --all-targets -- -D warnings`.

## Aufbau

```
src/                    Oberfläche
  App.tsx               Rahmen: Seitenleiste, Ansichten, Drag&Drop, Tastenkürzel, Ereignisse
  api.ts                alle Typen und invoke-Aufrufe; READ_ONLY-Liste; Hilfsfunktionen (Farben, Gramm, Spulen-Ranking)
  devmock.ts            Beispieldaten für die Browser-Vorschau (nur DEV, nicht im Build)
  styles.css            Designsystem (Tokens hell/dunkel, Bausteine)
  TodayView, Board, ListView, OrderPanel, Prints, NewOrderWizard, CatalogView, CatalogPicker,
  LinkImport, FilamentView, PrintersView, StatsView, PeopleView, SettingsView
src-tauri/
  src/lib.rs            Setup, State, Befehlsliste (invoke_handler)
  src/commands.rs       Tauri-Befehle für Aufträge, Anhänge, Katalog, Filament, Drucke
  src/store.rs          gesamte Datenbank-Schicht (portables SQL) und Tests
  src/model.rs          Datentypen (serde camelCase, sqlx FromRow)
  src/watch.rs          Auftragsordner und spools.json beobachten; gesliced 3MF → Druck
  src/threemf.rs        3MF lesen (Vorschau, Platten, Filamente, Druckzeit, Gramm)
  src/filament.rs       gemeinsame Helfer für Spulen (Farbe)
  src/contacts/         Kontaktquellen: apple.rs (Contacts-Framework, nur macOS)
  src/contactsync.rs    Kunden aus Kontakten übernehmen, verknüpfen, abgleichen (nur lesen)
  src/profiles.rs       Profilquelle für den Slicer (aidun/printerprofiles), Cache im Datenordner
  src/estimates.rs      Kalkulation vorab je Teil: Profilwahl, Slicen im Hintergrund, MakerWorld-Druckprofil
  src/slicers/          Slicer-Anbindungen: bambu_studio.rs (Programm finden/öffnen, spools.json, Projekt lesen)
  src/linkimport.rs     Link-Resolver je Plattform (Printables, MakerWorld, Thingiverse, Web) über die Anbindungen Web und Printables
  src/matching.rs       Zuordnung Druckauftrag → Auftrag (Namen, Kennung A0004), anbieterunabhängig
  src/usage.rs          tatsächlicher Verbrauch bei Abbrüchen, anbieterunabhängig
  src/connectors/       Gerüst für alle externen Anbindungen (Registry, HTTP, Schlüsselbund, Takt), siehe docs/PLAN-Anbindungen.md
  src/bambu/            Bambu-Konto als Anbindung bambu-cloud: mod.rs (Descriptor, Anmeldung, Abgleiche),
                        printers.rs (Drucker, Druckverlauf), makerworld.rs (Sammlungen, 3MF bei Bedarf)
  migrations/           0001…0020, nur anhängen, nie ändern
  tests/fixtures/       lokale 3MF-Testdateien (nicht im Repo; Tests überspringen sie, wenn sie fehlen)
docs/                   Benutzerhandbuch, Architektur, history/ (Pläne, Spikes)
```

## Konventionen

**Neuer Tauri-Befehl**
1. Funktion in `commands.rs` bzw. `bambu/*.rs` schreiben.
2. In `lib.rs` im `generate_handler!` eintragen.
3. Typ und Aufruf in `src/api.ts` ergänzen.
4. Ändert der Befehl nichts, kommt er auf die `READ_ONLY`-Liste. Alle anderen lösen nach dem Aufruf `pm:changed` aus, und die Ansichten laden neu.
5. Für die Browser-Vorschau eine Antwort in `devmock.ts` ergänzen.

**Datenbank**
- Neue Migration `000N_name.sql` anhängen.
- **Portables SQL:**
  - Platzhalter `$1…`
  - `BIGINT` und `DOUBLE PRECISION`
  - Zeitpunkte als RFC-3339-Text (`store::now()`)
  - IDs als UUID-Text
  - Keine SQLite- oder Postgres-Sonderfunktionen wie `GROUP_CONCAT`. Listen werden in Rust zusammengesetzt.
- **Spalten** kommen in die `*_COLS`-Konstanten und ins passende Struct in `model.rs`.
- **Tests:** Store-Tests in `store.rs` mit Temp-Datenbank.

**Ereignisse:** Änderungen außerhalb der App (Ordner, Konto) meldet der Kern über das Event `pm:external` mit `{messages}`. `App.tsx` lädt dann neu und zeigt eine Meldung.

**Oberfläche**
- Nur Tokens aus `:root` verwenden (`--surface`, `--border`, `--text`, `--muted`, `--accent-bg` …), keine festen Farben.
- Bedienelemente sind 30 px hoch (`--h`). Icons kommen aus `lucide-react`, Größe 13–16.
- Jede Ansicht beginnt mit `<header className="toolbar">` mit `h1`, optional `.page-sub`, dann die Aktionen.
- Statusfarben (`--s-*`) nur als Punkt oder Leiste, nie als Fläche oder Textfarbe für Titel.
- Leere Zustände zeigen einen Hinweis, was zu tun ist.

**Bambu-Endpunkte**
- Die bekannten Endpunkte und ihre Felder stehen in [docs/ARCHITEKTUR.md](docs/ARCHITEKTUR.md#bambu-konto).
- JSON tolerant lesen: Felder können fehlen oder einen anderen Typ haben.
- Jede Funktion des Kontos muss ohne Anmeldung sauber ausfallen.

## Test- und Vorschau-Hinweise

- `cargo test` braucht weder Netz noch Bambu-Konto. HTTP-Tests laufen gegen `wiremock`, der Schlüsselbund wird in Tests nicht benutzt.
- Fehlen die 3MF-Testdateien in `src-tauri/tests/fixtures/`, überspringen die Tests sie stillschweigend. Sie lassen sich durch eigene gesliced Projekte ersetzen: `switch.3mf` (nicht gesliced) und `figuren.gcode.3mf` (gesliced).
- Für visuelle Prüfungen eignet sich `npm run dev` mit der Browser-Vorschau. Die echte App nur prüfen, wenn der Nutzer zustimmt.
