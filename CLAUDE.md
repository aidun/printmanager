@AGENTS.md

# Arbeitsweise für Claude

## Ziel
Das beste Werkzeug für private 3D-Druck-Aufträge mit Bambu Lab. Jede Änderung beginnt beim Nutzen für den Anwender: weniger Klicks, klare Zustände, nichts Überraschendes.
- Neue Funktionen bekommen leere Zustände und Fehlermeldungen in Klartext.
- Änderungen außerhalb der App brauchen eine Bestätigung und einen Protokolleintrag.

## Antworten
- Deutsch und knapp: zuerst das Ergebnis, dann was offen oder ungeprüft ist.
- Bei klarem Standard handeln, nicht fragen. Bei echten Entscheidungen eine Frage stellen, mit Empfehlung.

## Fertig heißt
1. `npx tsc -b`
2. `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`, denn die CI bricht bei jeder Warnung ab.
3. Neuer Tauri-Befehl: Eintrag in `lib.rs` und `api.ts` (READ_ONLY prüfen), Antwort in `devmock.ts`.
4. Oberfläche geändert: in der Vorschau bei 960×600 (kleinste Fenstergröße) und 1600×1000 prüfen, hell und dunkel.
5. Doku:
   - Bedienung → `docs/BENUTZERHANDBUCH.md`
   - Technik → `docs/ARCHITEKTUR.md`
   - Änderung → `CHANGELOG.md`
6. Ist eine sichtbar geänderte Ansicht in `docs/images/` abgebildet, das Bild neu erzeugen (siehe unten).

## Schnell und verlässlich prüfen
- **Vorschau:** `npm run dev` startet sie (Preview „ui“, Port 1420).
  - Routen: `#filament`, `#board&order=o8`, `#today&neu`.
  - Nach einem Hash-Wechsel `location.reload()` aufrufen.
- **Verdeckter Browserbereich:** Screenshots sind dann veraltet. Layout per DOM messen (`getBoundingClientRect`).
- **Eingaben:** mit echten Klicks und Tippen testen. Ein per Skript ausgelöstes `blur` erreicht React nicht.
- **`devmock.ts`:** Listen als Kopie zurückgeben (`list.map((x) => ({ ...x }))`), sonst rendert React nicht neu.
- **Produktbilder:** nur mit Beispieldaten, 1600×1000, hell:
  `"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --hide-scrollbars --blink-settings=preferredColorScheme=1 --window-size=1600,1000 --virtual-time-budget=4000 --user-data-dir=<scratch>/chrome --screenshot=<scratch>/x.png "http://localhost:1420/#filament"`

## Echte App und echte Daten (nur mit Zustimmung)
- **Bauen und neu starten:**
  1. `npx tauri build --debug --bundles app`
  2. `osascript -e 'quit app "Printmanager"'`
  3. `open src-tauri/target/debug/bundle/macos/Printmanager.app`
- **Datenbank nur lesen:** `sqlite3 -readonly ~/Library/Application\ Support/de.printmanager.app/printmanager.db`
- **Computer Use:** funktioniert nur, wenn das Fenster auf einem normalen Schreibtisch liegt, ohne Vollbild-App davor. Sonst die Schritte vom Nutzer klicken lassen und das Ergebnis in der Datenbank prüfen.
- **Schreiben ins Bambu-Konto:** nur an einer Testspule, die der Nutzer angelegt hat. Danach das Protokoll (`filament_writes`) prüfen und die Testspulen entfernen lassen.

## Release
1. Version anheben in:
   - `package.json`
   - `package-lock.json` (2 Stellen)
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Abschnitt in `CHANGELOG.md` schreiben, committen, mit `git tag -a vX.Y.Z` taggen, Branch und Tag pushen.
3. Auf CI und Release-Workflow warten (`gh run list`), dann die Notizen setzen:
   `gh release edit vX.Y.Z --notes-file <Abschnitt aus CHANGELOG>`

   Den Entwurf veröffentlicht der Nutzer.

## Token sparen
- Große Dateien gezielt lesen: `store.rs` (~2100 Zeilen), `commands.rs`, `styles.css`, `OrderPanel.tsx`. Erst `grep -n`, dann den Ausschnitt.
- Bambu-Endpunkte und Felder zuerst in `docs/ARCHITEKTUR.md#bambu-konto` nachsehen, Testergebnisse in `docs/history/`. Nicht neu recherchieren.
- Pläne für größere Vorhaben stehen in `docs/PLAN-*.md`. Vor einem Umbau dort den Stand lesen.
