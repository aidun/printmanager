---
name: ausliefern
description: Fertige Änderungen am Printmanager ausliefern – prüfen, committen, pushen, Datenbank sichern, stabile App nach /Applications installieren und den Start prüfen. Verwenden, wenn der Nutzer „committen, pushen und installieren“, „alles drei“, „ausliefern“, „in meiner App installieren“ oder Ähnliches sagt. Nicht für ein GitHub-Release (dafür der Skill release).
---

# Ausliefern

Eine Kette, jedes Mal gleich. Nur auf ausdrücklichen Wunsch des Nutzers; die stabile App enthält seine echten Daten.
Schlägt ein Schritt fehl: anhalten, den Fehler im Klartext melden, nichts überspringen.

## 1. Prüfen (alles muss grün sein)

```bash
npx tsc -b
```

```bash
cd src-tauri && cargo test 2>&1 | grep -E "^test result|FAILED|panicked" ; cargo clippy --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)" | head
```

```bash
npm run test:e2e
```

Bei sichtbaren Textänderungen in der Oberfläche die neuen Texte kurz gegen `unslop` (falls installiert) bzw. `design:ux-copy` lesen: deutsch, kurz, ohne Floskeln. Bei größeren Änderungen am Kern (grob über 200 Zeilen) vorher `/code-review` auf den Diff.

## 2. Stand prüfen und committen

- `git status --short`: keine fremden Dateien (z.B. `result.json` der Bambu-Kommandozeile, Testdateien), keine Geheimnisse (Token, Passwörter) im Diff.
- Doku nachgezogen? Bedienung → `docs/BENUTZERHANDBUCH.md`, Technik → `docs/ARCHITEKTUR.md`, Änderung → `CHANGELOG.md` (Abschnitt „Unveröffentlicht“ bzw. aktueller Abschnitt). Neue Migration → Zähler in `AGENTS.md` und `docs/ARCHITEKTUR.md`.
- Commit-Nachricht auf Deutsch: erste Zeile was und wofür, darunter Stichpunkte. Am Ende die Attributionszeile aus der Sitzung.
- Ein Commit pro zusammengehöriger Änderung, dann **einmal** pushen: `git push origin main`. Mehrere schnelle Pushes brechen sich in der CI gegenseitig ab.

## 3. Datenbank der stabilen App sichern

```bash
S=<scratchpad>/db-stabil-$(date +%Y%m%d-%H%M); mkdir -p $S && sqlite3 -readonly ~/Library/Application\ Support/de.printmanager.app/printmanager.db ".backup '$S/printmanager.db'" && sqlite3 "file:$S/printmanager.db?immutable=1" "select max(version) from _sqlx_migrations; select count(*) from orders"
```

Die Zahlen (Migration, Aufträge) für Schritt 5 merken.

## 4. Installieren

```bash
npm run app:stable:install 2>&1 | tail -1
```

Baut die Release-Version, beendet die laufende App über ihre Kennung, ersetzt `/Applications/Printmanager.app` und startet sie. Dauert einige Minuten (Timeout 600000).

## 5. Start prüfen

```bash
sleep 4; pgrep -fl "Printmanager" | cut -c1-90; ls src-tauri/migrations | tail -1; sqlite3 -readonly ~/Library/Application\ Support/de.printmanager.app/printmanager.db "select max(version) from _sqlx_migrations; select count(*) from orders"
```

- Läuft `/Applications/Printmanager.app/...` (nicht ein Build aus `target/`)?
- Migration = Nummer der neuesten Datei in `src-tauri/migrations/`?
- Aufträge mindestens so viele wie in der Sicherung?

Weicht etwas ab: nichts weiter ändern, dem Nutzer die Sicherung nennen und fragen.

## 6. Melden

Kurz auf Deutsch: Commit-Hash und Titel, gepusht, installiert mit Versionsstand (`git describe`), Migration, Anzahl Aufträge, vollständiger Pfad der Sicherung. Dann ehrlich, was nicht in der echten App geprüft wurde.
