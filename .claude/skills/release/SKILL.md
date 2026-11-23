---
name: release
description: Neues GitHub-Release des Printmanagers erstellen – Prüfungen und Reviews, Version an allen Stellen anheben, Changelog-Abschnitt, Commit, Tag, Push, auf CI und Release-Workflow warten, Release-Notizen setzen. Verwenden bei „mach ein Release“, „neue Version“, „GitHub Release“, „taggen“. Veröffentlichen tut der Nutzer selbst.
---

# Release

Releases entstehen als Entwurf; veröffentlicht werden sie nur vom Nutzer.

## 1. Vorher prüfen

- Arbeitsbaum sauber, alles committet und gepusht (`git status --short`, `git log origin/main..HEAD`).
- Prüfungen wie im Skill ausliefern, Schritt 1 (tsc, cargo test, clippy, `npm run test:e2e`).
- Reviews seit dem letzten Tag (`git describe --tags --abbrev=0`):
  - `security-review` – besonders Schlüsselbund, Schreiben ins Bambu-Konto (`bambu/filament_write.rs`), HTTP-Zugang (`connectors/http.rs`), Dateipfade.
  - `/code-review` auf den Bereich `<letzter-tag>..HEAD` (Stufe medium).
  - Befunde mit dem Nutzer klären, bevor die Version angehoben wird.
- `gh release list --limit 5`: Liegen ältere Entwürfe herum, den Nutzer darauf hinweisen.

## 2. Version festlegen

Neue Funktionen → Minor (0.4.0 → 0.5.0), nur Korrekturen → Patch. Im Zweifel den Nutzer fragen, mit Empfehlung.

Version anheben in:
- `package.json`
- `package-lock.json` (2 Stellen: oben und unter `packages[""]`)
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.lock` aktualisiert sich über `cd src-tauri && cargo build`.

## 3. Changelog

Abschnitt „Unveröffentlicht“ in `## X.Y.Z – TT.MM.JJJJ` umbenennen und für Leser ordnen: thematische Gruppen (z.B. **Neu**, Bereiche, **Behoben**, **Technik**), keine Doppelungen, Migrationen zusammengefasst. Deutsch, kurz, ohne Floskeln (bei Bedarf `unslop`).

## 4. Commit, Tag, Push

```bash
npx tsc -b && (cd src-tauri && cargo test 2>&1 | grep -E "^test result|FAILED" && cargo clippy --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)")
```

```bash
git add -A && git commit -m "Version X.Y.Z: <Stichworte>" -m "<Attributionszeile>" && git tag -a vX.Y.Z -m "Printmanager X.Y.Z" && git push origin main && git push origin vX.Y.Z
```

## 5. Auf CI und Release-Workflow warten

`gh run list --limit 4` zeigt die IDs. Warten im Hintergrund (benachrichtigt bei Ende, nicht selbst abfragen):

```bash
gh run watch <release-id> --exit-status --interval 60 > /dev/null 2>&1; echo "release rc=$?"; gh run watch <ci-id> --exit-status --interval 60 > /dev/null 2>&1; echo "ci rc=$?"
```

Schlägt ein Lauf fehl: `gh run view <id> --log-failed | tail -50`, Ursache melden, nicht neu taggen ohne Rückfrage.

## 6. Notizen setzen

```bash
python3 -c "import re,sys;s=open('CHANGELOG.md').read();m=re.search(r'^## X\.Y\.Z.*?\n(.*?)(?=^## )',s,re.S|re.M);open(sys.argv[1],'w').write(m.group(1).strip()+'\n')" <scratchpad>/notes.md && gh release edit vX.Y.Z --notes-file <scratchpad>/notes.md && gh release view vX.Y.Z --json isDraft,assets --jq '{draft: .isDraft, assets: [.assets[].name]}'
```

## 7. Melden

Version, Tag, Link zum Entwurf (`gh release view vX.Y.Z --json url --jq .url`), Pakete (Assets), Ergebnis der Reviews. Den Nutzer bitten, den Entwurf zu veröffentlichen. Nicht selbst veröffentlichen.
