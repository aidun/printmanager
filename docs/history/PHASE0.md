# Phase 0 – Ergebnis: Slicen im Linux-Container

Stand: 16.11.2026 · Code: `spike/` (Dockerfile, `resolve.py`)

## Ergebnis in Kürze

**Machbar.** Bambu Studio 02.09.01.70 (offizielles Linux-AppImage) slict im Docker-Container
ohne Bildschirm und ohne Xvfb. Getestet wurden deine H2C-Profile und Spulen aus der Bibliothek.
Die Ausgabe ist eine vollständige `.gcode.3mf`.

| Test | Ergebnis |
|---|---|
| `Switch+Wandhalter.3mf` · „Normal + Stützen H2C 0.4“ · Spule Generic PETG (GFG99) | ✅ 3 h 5 min, 85,2 g, Baumstützen aktiv, 4 s Slice-Zeit |
| `Figuren.3mf` · gleiches Profil · Spule Bambu PLA Matte (GFA01), Farbe #BB3D43 | ✅ 3 h 53 min, 185,4 g, Farbe korrekt übernommen |
| Ausgabe lesbar mit `threemf.js` aus dem PoC | ✅ Druckzeit, Gewicht, Filament, Vorschaubild |
| `tray_info_idx` in `slice_info.config` = `setting_id` der Spule | ✅ (wichtig für die AMS-Zuordnung später) |
| Übergabe an Bambu Connect | ⚠️ nicht testbar: Bambu Connect ist auf diesem Mac nicht installiert |
| Übergabe an Bambu Studio per `bambustudioopen://open?file=<URL>` | ❓ Bambu Studio hat die Datei nicht abgerufen, Ergebnis unklar |

Die Zeiten sind auf deinem Mac (Apple Silicon) gemessen, wo der x86-Container emuliert läuft.
Auf einem x86-NAS sollte es mindestens genauso schnell gehen.

## Erkenntnisse für die Umsetzung

1. **Profile selbst auflösen.** Die CLI sucht Systemprofile in `machine_full/` und `process_full/`.
   Diese Ordner gibt es im AppImage nicht. Wir lösen deshalb die `inherits`-Kette selbst gegen
   `resources/profiles/BBL` auf und übergeben vollständige JSONs. Der Prototyp `spike/resolve.py`
   macht das bereits für Drucker, Prozess (deine Änderungen + System) und Filament.
2. **Spule → Filamentprofil:** Die `setting_id` der Spule (z.B. `GFA01`) ist die `filament_id`
   im Basisprofil. Wir wählen das instanziierbare Profil, dessen aufgelöste `filament_id` passt und dessen
   `compatible_printers` den Drucker enthält. Beispiel: GFG99 → „Generic PETG @BBL H2C 0.4 nozzle“.
3. **Farbe über die 3MF setzen.** Die CLI übernimmt die Farbe nicht aus dem Filamentprofil.
   Vor dem Slicen schreiben wir `filament_colour` (und den Typ) in die `Metadata/project_settings.config`
   der Eingabe-3MF. Das ist getestet.
4. **Mehrfarbig / mehrere Slots:** Bisher ist nur Slot 1 getestet. Die Projekte haben bis zu 6 Slots;
   `--load-filaments` ersetzt sie der Reihe nach. Die Zuordnung mehrerer Slots wird in Phase 3 mit einem echten mehrfarbigen Modell getestet.
5. **Vorschaubilder** werden ohne Bildschirm nicht neu gerendert; die Bilder aus der Eingabe-3MF bleiben erhalten.
   Für den Katalog reicht das. Eine gesliced Vorschau gibt es dann nicht.
6. **CLI-Details:** `--allow-newer-file` ist ein Schalter ohne Wert.
   Das Ergebnis steht zusätzlich als `result.json` im Ausgabeordner (Fehlertext, Zeiten, Gramm pro Filament),
   was sich gut für die Statusanzeige eignet.
7. **Image:** Ubuntu 24.04 + AppImage, ca. **1,25 GB**. Das AppImage wird nicht ausgeführt, sondern direkt per
   `unsquashfs` entpackt (unter Emulation lässt sich AppImage nicht starten).
8. **Plattform:** Bambu Studio gibt es nur für **x86_64**. Das NAS muss x86 sein (Intel/AMD);
   auf einem ARM-NAS (z.B. Raspberry Pi) wäre es emuliert und langsam.

## Offene Punkte für dich

- **Bambu Connect installieren** (https://wiki.bambulab.com/en/software/bambu-connect) und anmelden.
  Danach teste ich die Übergabe per `bambu-connect://import-file`.
- **Ist dein NAS x86_64?** Modell bzw. Ausgabe von `uname -m` genügt.
- **`bambustudioopen://`:** Falls sich beim Test ein Fenster oder Hinweis in Bambu Studio geöffnet hat, sag mir kurz, was dort stand.
  Wenn das funktioniert, könnte Bambu Studio die Datei direkt vom NAS laden, ganz ohne Download-Pfad.

## Auswirkung auf den Plan

Der Ansatz „slicer-Container auf dem NAS“ bleibt; der Ausweichplan (slicen auf dem Mac) wird nicht gebraucht.
Neu in Phase 2/3: Auflösen der Profile und Spulen (aus `resolve.py`) sowie das Setzen der Farbe in der 3MF vor dem Slicen.
