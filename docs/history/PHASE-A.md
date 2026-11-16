# Phase A – Ergebnis: Bambu Studio vorbereitet öffnen

Stand: 16.11.2026 · Code: `spike/phaseA/patch3mf.py` · Testdateien: `spike/phaseA/out/`

## Entscheidung

**Die App setzt keine Profile oder Presets.** Sie öffnet die Datei des Auftrags unverändert in Bambu Studio.
Drucker, Prozess, Filamente und AMS wählst du dort wie gewohnt. Die App zeigt dir dazu die Angaben aus dem Auftrag
(Farbwunsch, Anzahl, Notizen).

Begründung: Technisch geht es (siehe unten). Aber damit Bambu Studio die Presets sauber und ohne „geändert“-Sternchen
erkennt, müsste die App das interne Format der Projekteinstellungen pro Studio-Version nachbauen:
Hotend-Varianten, `inherits_group` und Standardwerte, die in keinem Profil stehen. Der Aufwand steht in keinem Verhältnis
zum Nutzen, und jedes Update von Bambu Studio könnte es brechen.

## Was der Spike gezeigt hat (zum Nachlesen)

| Test | Ergebnis |
|---|---|
| Datei per `open -a BambuStudio <datei>` öffnen | ✅ funktioniert. Ist schon ein Projekt offen, fragt Studio „Open as project / Import geometry only“ und ggf. „Speichern?“ |
| Preset-Name + Werte in `project_settings.config` | Studio wählt das Preset mit diesem Namen, **die Werte kommen aber aus der Datei** |
| Unbekannter Preset-Name | Studio fällt auf das Systempreset aus `inherits_group` zurück, Werte bleiben |
| Werte aus dem Repo printprofiles | Sternchen, weil die Repo-Version von den installierten Presets abweicht (5 statt 4 Hotend-Varianten) |
| Werte aus den installierten Presets | Prozess ohne Sternchen; Filament nur ohne Sternchen, wenn auch die Werte außerhalb der Profilkette zurückgesetzt werden |
| Farbe pro Slot | ✅ wird aus `filament_colour` übernommen |

Weitere Erkenntnisse:
- Der H2C speichert Filamentwerte pro Slot **und** Hotend-Variante (`filament_self_index`, `filament_extruder_variant`).
- Systemprofile liegen in Unterordnern (`system/BBL/filament/SUNLU/…`).
- Für SUNLU PLA+ / PLA+ 2.0 gibt es weder bei Bambu noch in printprofiles ein H2C-Profil.

## Was für die App bleibt

- **Öffnen:** Mac `open -a BambuStudio <datei>`, Windows `bambu-studio.exe <datei>`. STL, STEP und 3MF öffnet Studio direkt,
  die App muss nichts umbauen.
- **Arbeitskopie:** Die App kopiert die Datei in den Auftragsordner und öffnet die Kopie. So bleibt die Katalog-Datei unverändert,
  und das gespeicherte Projekt landet beim Auftrag.
- **Rückweg:** Wenn du in Studio slicst und speicherst, liest die App aus der 3MF Druckzeit, Gewicht und Filamente
  (`slice_info.config`, in Phase 0 getestet). Gespeichert wird dort, wo die Datei geöffnet wurde, also im Auftragsordner.
- **Spulen:** Die Filament-Bibliothek (`spools.json`) bleibt als Anzeige und für den Abzug der verbrauchten Gramm.
  Zuordnung über Material und Farbe aus `slice_info.config`.
