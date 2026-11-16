# Printmanager

Desktop-App für macOS und Windows, mit der du private 3D-Druckwünsche von Freunden und Familie verwaltest.
Du nimmst Aufträge auf, führst sie Schritt für Schritt bis zur Übergabe und öffnest die Modelle direkt in Bambu Studio.
Über dein Bambu-Konto kommen Filament, Druckerstatus und MakerWorld-Sammlungen dazu. Die App liest dabei nur.

| Für wen | Dokument |
|---|---|
| Nutzer der App | [docs/BENUTZERHANDBUCH.md](docs/BENUTZERHANDBUCH.md) |
| Entwickler und Coding-Agenten | [AGENTS.md](AGENTS.md) (Regeln, Befehle, Konventionen) und [docs/ARCHITEKTUR.md](docs/ARCHITEKTUR.md) |
| Entscheidungen und Verlauf | [docs/history/](docs/history/) |

## Funktionen

- **Heute:** was fällig ist, was druckt, welche Drucke noch ausgewertet werden müssen, was zur Übergabe bereit liegt und welches Filament knapp wird.
- **Board und Liste:** Die Stationen sind Eingang → Klären → Vorbereiten → In Studio → Druckt → Nacharbeit → Fertig → Übergeben, dazu Pausiert und Abgelehnt. Fertige Aufträge lassen sich archivieren.
- **Auftrag:** Person, Fälligkeit, Priorität, Teile mit Farbwunsch, Anhänge (Dateien, Bilder, Links, Text) und Verlauf.
- **Neuer Auftrag (⌘N):** Ein Assistent fragt nacheinander ab: für wen, was (Katalog, Link oder eigene Dateien), Details, und zum Schluss die Prüfung.
- **Katalog:** fertige Projekte (3MF/STL/STEP), Import über Links (Printables, MakerWorld & Co.) und deine MakerWorld-Sammlungen. Die Dateien lädt die App erst, wenn du sie brauchst.
- **Bambu Studio:** Druckdateien öffnest du per Klick. Speicherst du nach dem Slicen im Auftragsordner, erkennt die App den Druck und übernimmt Druckzeit und Gramm.
- **Filament:** Spulen aus dem Bambu-Konto, alle 15 Minuten abgeglichen, mit AMS-Slot. Den Verbrauch eines Drucks ziehst du mit einem Vorschlag für die passende Spule ab.
- **Drucker:** Status, Fortschritt und Restzeit über das Bambu-Konto. Ist ein Druck fertig, schlägt die App das Ergebnis mit echter Dauer und echten Gramm vor. Die App steuert keinen Drucker.
- **Statistik:** Verbrauch, Druckzeit, Fehldrucke und Übergaben, pro Monat, Person und Material.

## Schnellstart (Entwicklung)

Voraussetzungen: Node 22 und Rust (stable). Unter macOS brauchst du außerdem die Xcode Command Line Tools.

```bash
npm install
```

```bash
npm run tauri dev
```

```bash
npm run dev
```

`npm run dev` startet nur die Oberfläche im Browser unter http://localhost:1420, mit Beispieldaten aus `src/devmock.ts`.

```bash
cd src-tauri && cargo test
```

```bash
npx tauri build --debug --bundles app
```

Der letzte Befehl baut eine lokale App unter `src-tauri/target/debug/bundle/macos/`.

## CI und Releases

- **CI** (`.github/workflows/ci.yml`): Nach jedem Push und bei Pull Requests laufen die Typprüfung und der Build der Oberfläche, dazu `cargo clippy -D warnings` und `cargo test`.
- **Release** (`.github/workflows/release.yml`): Ein Tag `vX.Y.Z` prüft zuerst, ob er zur Version in `src-tauri/tauri.conf.json` passt, und testet. Dann baut er macOS (Universal, `.dmg`/`.app`) und Windows (`.msi`/`.exe`) und legt einen **Release-Entwurf** an. Manuell gestartet entstehen nur Build-Artefakte.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

Die Builds sind nicht signiert. Unter macOS öffnest du die App beim ersten Start per Rechtsklick → Öffnen, unter Windows wählst du bei SmartScreen „Weitere Informationen → Trotzdem ausführen“.

## Daten

Die Daten liegen standardmäßig in `~/Library/Application Support/de.printmanager.app` (Windows: `%APPDATA%\de.printmanager.app`):
- `printmanager.db` (SQLite)
- `orders/<Nr>/` mit den Dateien eines Auftrags
- `catalog/<id>/` mit den Katalogprojekten
- `trash/` mit gelöschten Aufträgen

Zum Sichern kopierst du diesen Ordner. Das Anmelde-Token des Bambu-Kontos liegt im Schlüsselbund des Systems, nicht im Datenordner.
