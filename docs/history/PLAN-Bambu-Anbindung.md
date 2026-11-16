# Plan: Bambu-Konto, Drucker und MakerWorld

Stand 16.11.2026: Stufen 0–4 umgesetzt (3b entfällt vorerst). Drei Wünsche:

1. Die Filament-Bibliothek soll sich fortlaufend aktualisieren.
2. Die Bambu-Lab-Drucker sollen in der App erscheinen und ihren Status zeigen.
3. MakerWorld-Sammlungen und -Favoriten sollen in den Katalog kommen.

Für alle drei braucht die App dasselbe Fundament: eine **Anmeldung am Bambu-Konto**. Darum beginnt der Plan dort.

---

## Ausgangslage (lokal geprüft)

**Filament**
- `filament_inventory/spools.json` wurde zuletzt am **08.11.** geschrieben. Bambu Studio schreibt die Datei also nur gelegentlich, nämlich beim Abgleich im Filament-Manager.
- Die Spulen tragen `cloud_synced: true`, `bound_dev_id`, `bound_ams_id` und `tag_uid`. Die eigentliche Quelle ist das Bambu-Konto, nicht die Datei.
- Die Datei zu beobachten, wie es die App heute tut, ist deshalb richtig, reicht aber nicht für „fortlaufend“.

**Drucker**
- Bambu Studio kennt zwei Drucker, einen H2C und einen A1.
- Die Zugangscodes liegen in `BambuStudio.conf`. Die App liest sie **nicht** aus, siehe Entscheidungen.

**MakerWorld**
- Der Link-Import liest heute schon öffentliche Modelldaten.
- Dateien bekommt die App nur mit Anmeldung, das steht so auch in `linkimport.rs`.

**Was öffentlich bekannt ist**
- Bambu und MakerWorld haben **keine offizielle API**. Alles Folgende stützt sich auf Endpunkte, die die Community dokumentiert hat. Sie funktionieren, können sich aber ändern.
- Darum gilt für jede Funktion: nur lesen, optional, und die App läuft auch ohne sie weiter.

| Zweck | Endpunkt (Host `api.bambulab.com`) | Quelle |
|---|---|---|
| Anmeldung | `POST /v1/user-service/user/login` (Passwort oder E-Mail-Code), Token gilt ca. 3 Monate, Erneuern geht nicht → neu anmelden | OpenBambuAPI |
| Filament-Manager | `GET /v1/design-user-service/my/filament/v2` (Rest in g, RFID, Farbe, `updated_at`) | ha-bambulab PR #2028 |
| Drucker am Konto | `GET /v1/iot-service/api/user/bind` (Name, Modell, online, Druckstatus, Zugangscode) | OpenBambuAPI |
| Laufender Druck | `GET /v1/iot-service/api/user/print` (Auftrag, Fortschritt, Status) | OpenBambuAPI |
| Druckverlauf | `GET /v1/user-service/my/tasks` (echte Dauer, Gramm pro AMS-Slot) | OpenBambuAPI |
| Sammlungen (Liste) | `GET /v1/design-service/my/favorites/listlite` → `default` + `hits[]` mit `id`, `title`, `designCnt` | Spike ✅ |
| Alle gesammelten Modelle | `GET /v1/design-service/favorites/designs/{uid}?offset&limit` | OpenBambuAPI, Spike ✅ |
| Sammlung | `GET /v1/design-service/favorites/{id}/designs?limit=20&offset=N` (Feld `total` nutzen) oder `GET …/favorites/{id}` (alle auf einmal, inkl. `hiddenIds`) | PR #195/#196, Spike ✅ |
| Modell / 3MF | `GET /v1/design-service/design/{id}`, `…/instance/{instanceId}/f3mf?type=download` | OpenBambuAPI |
| Live im LAN | MQTT TLS Port 8883, Benutzer `bblp`, Passwort = Zugangscode, Thema `device/<Seriennr.>/report` | Bambu-Wiki, Community |

Die Liste der eigenen Sammlungen war nicht dokumentiert. Stufe 0 hat sie gefunden (siehe unten).

---

## Leitplanken

- **Nur lesen.** Die App startet, pausiert und bricht keinen Druck ab. Sie ändert nichts am Konto, an MakerWorld oder in Bambu Studio. Drucken bleibt in Bambu Studio oder Bambu Handy. Den Developer Mode braucht es nicht, die Drucker bleiben in der Cloud.
- **Alles optional.** Ohne Anmeldung funktioniert die App wie heute, mit `spools.json` und lokalem Katalog.
- **Geheimnisse gehören nicht in die Datenbank.**
  - Das Token liegt im Schlüsselbund des Systems (macOS-Schlüsselbund, unter Windows die Anmeldeinformationsverwaltung, Rust-Crate `keyring`).
  - Das Passwort speichert die App nie.
  - Tokens und Zugangscodes erscheinen nicht in Logs.
- **Sparsam abfragen.** Bambus Server bekommen höchstens etwa eine Anfrage pro Sekunde, mit festen Intervallen (siehe unten), Zwischenspeicher und Abbruch nach Fehlern.
- **Ausfälle stören nicht.** Liefert ein Endpunkt Fehler, zeigt die App „Bambu-Konto: nicht erreichbar“ und arbeitet mit den letzten Daten weiter.

---

## Stufe 0 – Spike: Anmeldung und Endpunkte prüfen ✅ erledigt (16.11.2026)

**So lief es:** Ein Testabschnitt in den Einstellungen ([bambu_spike.rs](../src-tauri/src/bambu_spike.rs)) öffnet die Bambu-Anmeldeseite in einem eigenen Fenster. Dort hast du dich angemeldet. Danach hat die App das Cookie `token` gelesen (Tauri `cookies()`) und alle Endpunkte nur lesend abgefragt.
- Die Antworten liegen geschwärzt in `<Datenordner>/spike-bambu/`.
- Das Token wurde nicht gespeichert.
- Das Anmeldefenster hat keinen Zugriff auf die Befehle der App.

**Ergebnisse**

| Bereich | Ergebnis | Folge für den Plan |
|---|---|---|
| Anmeldung | Das Anmeldefenster und das Auslesen des Cookies funktionieren, ein eigenes Formular ist unnötig | Stufe 1 übernimmt diesen Weg |
| Filament (`filament/v2`) | 26 Spulen. Die 24 aus `spools.json` haben **dieselben IDs** und denselben Rest, 2 gibt es nur im Konto. Je Spule: `netWeight`/`totalNetWeight`, `inPrinter`, `deviceName`, `amsId`, `slotId`, `trayIdName`, `RFID`, `depleted`, `updatedAt` | Das Konto ist die Hauptquelle. **AMS-Belegung kommt aus dem Konto**, dafür ist keine Live-Verbindung nötig |
| Drucker (`bind`) | Nur der **H2C** ist am Konto (online, `print_status`, `print_job`, Gesamtdruckzeit). **Der A1 ist nicht verbunden** | A1 nur über die Live-Verbindung im Netzwerk (3b) oder nach Verbinden in Bambu Handy |
| Laufender Druck (`print`) | Ohne laufenden Druck nur Name, Modell, online. Den Fortschritt während eines Drucks konnten wir noch nicht sehen | Prüfen beim ersten echten Druck in Stufe 3a |
| Druckverlauf (`my/tasks`) | 61 Drucke, je Druck `status` (2 = fertig), `startTime`/`endTime`, `costTime`, `weight`, `title`, `plateIndex`, `designId`, `amsDetailMapping` mit Gramm pro Slot | Echte Werte für den Abzug und die Statistik |
| Sammlungen | `my/favorites/listlite` liefert die **Liste der Sammlungen**: Standard (9) und 23 weitere mit Name und Anzahl | Der offene Punkt ist gelöst |
| Inhalt einer Sammlung | Blättern funktioniert (164 Modelle, `total` stimmt, Seite 2 anders als Seite 1). `favorites/{id}` liefert alles auf einmal und dazu `hiddenIds` (6 ausgeblendete) | Abgleich mit einer Anfrage pro Sammlung |
| Modell (`design/{id}`) | Titel, Beschreibung, Bilder, `license`, `instances[]` (Druckprofile mit Titel, Gewicht, `needAms`), `defaultInstanceId` | Genug für die Katalogkarte |
| 3MF (`instance/{id}/f3mf`) | Liefert Dateiname und eine signierte Download-Adresse auf `makerworld.bblmw.com` | Den eigentlichen Download prüft Stufe 4, kein Cloudflare-Problem zu erwarten |
| Live im LAN | Nicht geprüft | Erst in 3b |

**Abweichungen von den Annahmen**
- Für die AMS-Belegung ist die Live-Verbindung im Netzwerk nicht mehr nötig. 3b wird dadurch noch optionaler.
- Ob Bambu den Rest nach einem Druck selbst mindert, zeigt sich erst durch Vergleich über einige Tage. Dafür gibt es die Einstellung „Verbrauch abziehen durch“ in Stufe 2.

**Aufräumen:** Der Testabschnitt in den Einstellungen bleibt bis Stufe 1 und wird dann durch den Abschnitt „Bambu-Konto“ ersetzt. Der Ordner `spike-bambu` enthält Kontodaten und eine zeitlich begrenzte Download-Adresse. Er kann nach Stufe 1 gelöscht werden.

---

## Stufe 1 – Bambu-Konto in der App ✅ umgesetzt (16.11.2026)

**Umgesetzt** in [bambu/mod.rs](../src-tauri/src/bambu/mod.rs) und [bambu/cloud.rs](../src-tauri/src/bambu/cloud.rs). Die Einstellungen haben einen Abschnitt „Bambu-Konto“, der Testabschnitt aus Stufe 0 ist entfernt.
- **Anmeldung:** Ein privates Anmeldefenster (incognito) öffnet sich. Sobald das Cookie `token` da ist, übernimmt die App das Token, schließt das Fenster und meldet „Verbunden: Name“.
- **Speicherorte:** Das Token liegt im Schlüsselbund (`keyring`, Dienst `de.printmanager.app`). Name, uid, Ablaufdatum und Schalter liegen in `bambu.json` im Konfigurationsordner, ohne Token.
- **Cloud-Zugang:** nur GET, höchstens eine Anfrage pro Sekunde. 401/403 bedeutet „Anmeldung abgelaufen“.
- **Hintergrundprüfung:** 5 s nach dem Start und danach alle 30 Minuten. Ist die Anmeldung abgelaufen, erscheint eine Meldung.
- **Tests:** Drosselung, Fehler und Ablauf (`wiremock`), Ablaufdatum aus dem Token, `bambu.json` ohne Token.
- **Mit deinem Konto geprüft:** Anmeldung über das Fenster, Eintrag im Schlüsselbund, `bambu.json` ohne Token, Ablaufdatum aus dem Token gelesen (gültig bis 14.02.2027).

Ursprünglicher Plan:


- **Neue Module:**
  - `src-tauri/src/bambu/auth.rs`: Anmeldefenster, Token im Schlüsselbund, Abmelden, Ablauf erkennen (401 → „bitte neu anmelden“).
  - `bambu/cloud.rs`: HTTP-Client mit Drosselung, Region global/China.
- **Einstellungen**, Abschnitt „Bambu-Konto“:
  - Anmelden/Abmelden und angemeldet als (Name)
  - Gültig bis
  - drei Schalter: *Filament aus dem Konto*, *Druckerstatus*, *MakerWorld im Katalog*
- **Hintergrund-Takt:** ein gemeinsamer Planer für alle Abfragen, der auch beim Zurückkehren ins Fenster neu lädt. Neue Daten meldet er wie bisher über `pm:external`.
- **Tests:** Token-Ablauf, Fehler und Drosselung mit einem Test-HTTP-Server (`wiremock`).

---

## Stufe 2 – Filament fortlaufend aktuell ✅ umgesetzt (16.11.2026)

**Umgesetzt**
- **Abgleich:** Angemeldet und mit eingeschaltetem Schalter gleicht die App das Filament mit dem Konto ab:
  - 5 s nach dem Start, danach alle 15 Minuten
  - beim Öffnen der Filament-Seite, höchstens alle 2 Minuten
  - per Knopf „Jetzt abgleichen“
- **`spools.json` ruht** in dieser Zeit. Eine ältere Datei überschreibt nach dem Abmelden keine Kontowerte, erst eine neuere Datei gilt wieder.
- **Leere Antwort:** Eine leere Antwort des Kontos wird ignoriert, damit bei einer Störung nicht alle Spulen als „nicht mehr vorhanden“ gelten.
- **Migration `0005`:** `source`, `source_updated_at`, `in_printer`, `printer_name`, `ams_slot`, `depleted`.
- **Filament-Seite:**
  - Quelle und Zeit des letzten Abgleichs
  - „◉ h2c · AMS 1 · Slot 2“ an Spulen im Drucker, diese stehen in ihrer Gruppe oben
  - „laut Bambu leer“
  - „Konto: X g“, wenn der App-Rest abweicht
- **Verbrauch** zieht weiter die App ab (Entscheidung 3).
- **Mit deinem Konto geprüft:** 26 Spulen übernommen (2 neu), 4 im H2C mit Slot, Rest gesamt 16,7 kg.
- **Tests:** Einlesen der Kontoliste, Slot-Bezeichnungen, Vorrang Konto vor älterer Datei, fehlende Spulen.

Ursprünglicher Plan:


**Quellen, in dieser Reihenfolge**
1. **Bambu-Konto** (`filament/v2`): alle 15 Minuten, beim Fokus und per Knopf „Jetzt abgleichen“
2. **`spools.json`**: wie heute, als Ersatz ohne Anmeldung
3. **AMS-Belegung** kommt direkt aus dem Konto (`inPrinter`, `deviceName`, `amsId`, `slotId`). Die Live-Verbindung (3b) liefert sie nur schneller.

**Abgleich**
- Die bestehende Regel bleibt: Hat sich der Wert in der Quelle geändert, gilt die Quelle, sonst der App-Wert.
- Neue Spalten in Migration `0005`:
  - `source` (konto/datei)
  - `source_grams` (heute `studio_grams`)
  - `source_updated_at`
  - `bound_printer`
  - `bound_slot`
  - `synced_at`
- **Doppelt abziehen vermeiden:** Bambu Studio 2.6/2.7 zieht bei RFID-Spulen den Verbrauch teils selbst ab. Würde die App zusätzlich abziehen, wäre der Rest doppelt gemindert. Darum gibt es eine Einstellung „Verbrauch abziehen durch“:
  - **App:** wie heute
  - **Bambu:** Der Abzug-Dialog vermerkt nur, von welcher Spule der Druck kam. Der Rest kommt aus dem Konto.
  - Empfehlung: zunächst „App“ und nach der ersten Woche vergleichen. Die Abweichung zeigt die Filament-Seite.

**Oberfläche**
- Kopfzeile: „Zuletzt abgeglichen vor 3 Min.“ und die Quelle
- Auf jeder Spule ein Hinweis „im H2C, AMS 1 · Slot 2“
- Neue Spulen aus dem Konto erscheinen von selbst, mit Meldung

**Fertig, wenn** eine Spule, die du in Bambu Handy änderst, spätestens nach 15 Minuten ohne Neustart in der App stimmt, und wenn ein Test den Abgleich (Konto, Datei, App-Abzug) abdeckt.

---

## Stufe 3 – Drucker anzeigen (ca. 2–3 Tage)

**3a ✅ umgesetzt (16.11.2026)** in [bambu/printers.rs](../src-tauri/src/bambu/printers.rs)
- **Seite „Drucker“:** eine Karte pro Drucker mit Name, Modell, online, Zustand (Bereit, Druckt, Pausiert, Fertig, Fehler, Offline), Druckauftrag, Fortschritt, Restzeit und dem Auftrag dazu.
- **Abfrage:** jede Minute, während eines Drucks alle 20 s. Die Daten bleiben im Speicher, die Datenbank bekommt keine Druckerdaten.
- **Heute** zeigt laufende und fehlerhafte Drucker oben. Der Auftrag zeigt „◉ Druckt auf h2c · 45 % · noch 1 h 20 min“.
- **Zuordnung:** Druckauftrag und Dateiname werden ohne Endung, ohne „_plate_N“ und ohne Sonderzeichen verglichen. Zuerst gegen die 3MF-Anhänge und Drucke offener Aufträge, dann gegen den Titel.
- **Fertige Drucke** kommen aus dem Druckverlauf (`my/tasks`):
  - Wann: alle 15 Minuten, sofort nach einem erkannten Druckende und auch für Drucke, die geendet haben, während die App zu war.
  - Der Eintrag geht an den passenden Druck ohne Rückmeldung, der vor dem Ende angelegt wurde. Jeder Eintrag aus dem Verlauf wird nur einmal zugeordnet.
  - Am Druck erscheint „h2c meldet: fertig · 2 h 10 min · 84 g laut Drucker“ mit dem Knopf „Als Erfolgreich übernehmen“. Das Ergebnis setzt die App nicht selbst.
  - Der Abzug-Dialog nimmt die echten Gramm pro Material und Farbe.
  - Die Statistik nutzt die echte Druckzeit, wenn es sie gibt.
- **Migration `0006`:** `printer_name`, `printer_status`, `printer_finished_at`, `actual_seconds`, `actual_filaments`, `printer_task_id`.
- **Mit deinem Konto geprüft:** Der H2C wird abgerufen (Log: „Bambu-Konto: 1 Drucker“).
- **Noch nicht geprüft:**
  - Fortschritt und Restzeit während eines echten Drucks. Die Feldnamen sind tolerant geraten (`progress`/`mc_percent`, `mc_remaining_time`/`prediction`).
  - die Zuordnung eines echten Drucks zu einem Auftrag
  - die Oberfläche der Drucker-Seite, weil der Bildschirm gesperrt war

Ursprünglicher Plan 3a:
- Neue Seite **„Drucker“**: eine Karte pro Drucker mit
  - Name, Modell, online/offline
  - Status (bereit, druckt, pausiert, Fehler, fertig)
  - Name des Druckauftrags, Fortschrittsbalken, Restzeit
- Takt: alle 60 s, während etwas druckt alle 20 s.
- **Verbindung zum Auftrag:**
  - Passt der Auftragsname zu einer gesliced Datei eines Auftrags, z.B. `figuren` zu `figuren.gcode.3mf`, zeigt die Karte „#6 Figuren“.
  - Auch der Auftrag und „Heute → Druckt gerade“ zeigen dann „H2C · 45 % · noch 1:20“.
- **Druck zu Ende:**
  - Die App meldet „#6 fertig auf H2C – Ergebnis eintragen?“.
  - Bei „SUCCESS“ schlägt sie *Erfolgreich* und den Status *Nacharbeit* vor, bei Abbruch *Fehldruck*.
  - Du bestätigst. Die App ändert nichts von selbst.
- **Druckverlauf (`my/tasks`):** Er liefert die echte Dauer und die Gramm pro AMS-Slot. Damit werden die Werte im Abzug-Dialog genauer, und die Statistik bekommt die echte Druckzeit statt der Schätzung.

**3b – Live im Netzwerk (optional pro Drucker)**
- Verbindung per MQTT über TLS (Crate `rumqttc`), nur das Thema `device/<Seriennr.>/report` abonnieren. Den Zugangscode liefert `bind` aus dem Konto. Ohne Anmeldung tippst du ihn vom Druckerdisplay in die App ein, gespeichert im Schlüsselbund.
- Gewinn gegenüber 3a:
  - Sekundengenaue Daten: Schicht, Temperaturen, Lüfter
  - **AMS-Slots** mit Farbe, Material und Rest. Das versorgt Stufe 2 mit „welche Spule steckt wo“.
  - **HMS-Fehlermeldungen** im Klartext
- Ob die Anfrage „alles senden“ (`pushall`) ohne Developer Mode erlaubt ist, prüft 3b selbst (Stufe 0 hat LAN nicht getestet). Sonst wartet die App auf die nächste vollständige Meldung des Druckers.

**Später möglich:** Kamerabild (A1 Einzelbilder, H2C Videostrom), Benachrichtigung bei Fehlern.

**Fertig, wenn** beide Drucker auf der Seite erscheinen, ein echter Druck mit Fortschritt am richtigen Auftrag hängt und ein Ende-Vorschlag erscheint. Nötig dafür ist ein echter Druck, den **du** in Bambu Studio startest.

---

## Stufe 4 – MakerWorld im Katalog ✅ umgesetzt (16.11.2026)

**Umgesetzt** in [bambu/makerworld.rs](../src-tauri/src/bambu/makerworld.rs), Migration `0007`.
- **Abgleich:** kurz nach dem Start, alle 6 Stunden und per Knopf „MakerWorld abgleichen“ im Katalog.
- **Ablauf:** Zuerst die Sammlungsliste (`listlite`), dann pro eingeschalteter Sammlung alle Modelle (`favorites/{id}`). Ist die Liste unvollständig, blättert die App seitenweise.
- **Auswahl der Sammlungen:** in den Einstellungen unter „Bambu-Konto“. Neue Sammlungen sind eingeschaltet.
- **Doppelte Einträge:** Modelle, die schon per Link im Katalog sind (gleiche MakerWorld-Nummer), werden verknüpft, nicht verdoppelt.
- **Entfernte Modelle:** Was in keiner Sammlung mehr ist, wird markiert („nicht mehr in Sammlungen“) und ohne Datei ausgeblendet. Gelöscht wird nichts.
- **Oberfläche „Katalog“:**
  - links *Alle*, *Eigene*, *MakerWorld* und jede Sammlung mit Anzahl
  - Karten mit MakerWorld-Kennzeichen und „Datei bei Bedarf“
  - Vorschaubild direkt von MakerWorld
- **3MF bei Bedarf:** beim Öffnen in Bambu Studio, beim Hinzufügen zu einem Auftrag oder über „Datei holen“.
  - Die App liest die Modelldetails (Lizenz, Beschreibung, Standard-Druckprofil), holt die Download-Adresse und lädt die Datei in `catalog/<id>/`.
  - Danach wertet sie die 3MF aus wie bisher: Vorschau, Platten, Filamente, Druckzeit.
- **Mit deinem Konto geprüft:** 23 Sammlungen, 211 Modelle übernommen, 297 Zuordnungen von Modellen zu Sammlungen.
- **Von dir geprüft (16.11.):** „Datei holen“ lädt die 3MF (`f3mf?type=download`). Der Eintrag hat danach Datei, Druckprofil, Vorschau, Druckzeit, Filamente und Lizenz.

Ursprünglicher Plan:


**Abgleich**
- Takt: beim Start, alle 6 Stunden und per Knopf
- Was abgeglichen wird:
  - Favoriten
  - alle Sammlungen, die du in den Einstellungen auswählst
  - Einzelheiten zu jedem Modell: Titel, Bilder, Designer, Lizenz, Druckprofile

**Datenmodell** (Migration `0006`)
- `catalog` bekommt `origin` (lokal/makerworld), `remote_id` (Design), `remote_instance_id` (Druckprofil), `remote_updated_at`, `file_state` (keine/geladen) und `remote_missing`.
- Neue Tabelle `catalog_collections (catalog_id, collection_id, name)`. Ein Modell kann in mehreren Sammlungen stehen.
- Einträge dürfen **ohne Datei** existieren, nur verlinkt mit Vorschaubild. Die 3MF lädt die App erst bei Bedarf:
  - bei „In Bambu Studio öffnen“
  - bei „Zum Auftrag hinzufügen“
  - beim Schalter „Sammlung offline halten“

**Entfernte Favoriten** werden markiert („nicht mehr in MakerWorld“), wie bei Spulen. Lokale Dateien bleiben, gelöscht wird nur auf deinen Klick.

**Oberfläche „Katalog“**
- Links eine Spalte mit *Alle*, *Lokal*, *MakerWorld-Favoriten* und den einzelnen Sammlungen
- Auf jeder Karte ein MakerWorld-Kennzeichen und die Lizenz
- Knöpfe „Auf MakerWorld öffnen“ und „Datei holen“

**Wiederverwendung:** `linkimport.rs::makerworld` wird zur gemeinsamen Funktion „Modell einlesen“ für Link-Import und Abgleich.

**Lizenz:** Dateien sind nur für deinen eigenen Gebrauch. Die App zeigt die Lizenz und gibt nichts weiter.

**Fertig, wenn**
- eine Sammlung mit mehr als 20 Modellen vollständig erscheint,
- ein neuer Favorit spätestens nach dem nächsten Abgleich da ist,
- „In Bambu Studio öffnen“ die 3MF lädt und öffnet,
- Tests das Seitenblättern und das Markieren entfernter Einträge abdecken.

---

## Reihenfolge und Aufwand

| Stufe | Inhalt | Aufwand | Hängt ab von |
|---|---|---|---|
| 0 | Spike: Anmeldung und Endpunkte ✅ | ½ Tag | erledigt |
| 1 | Bambu-Konto in der App ✅ | 1 Tag | 0 |
| 2 | Filament fortlaufend ✅ | 1–1,5 Tage | 1 |
| 3 | Drucker (3a Konto ✅, 3b LAN entfällt) | 2–3 Tage | 1 |
| 4 | MakerWorld im Katalog ✅ | 2–3 Tage | 1 |

Reihenfolge: **0 ✅ → 1 ✅ → 2 ✅ → 3a ✅ → 4 ✅.** 3b entfällt vorerst (Entscheidung 5). Stufe 2 bringt sofort spürbaren Nutzen. 3b kommt zuletzt, weil es am meisten Feinarbeit braucht und 3a für den Alltag meist reicht.

## Risiken

| Risiko | Folge | Gegenmaßnahme |
|---|---|---|
| Bambu ändert inoffizielle Endpunkte | Abgleich fällt aus | Jede Quelle einzeln abschaltbar, letzte Daten bleiben, klare Meldung, Antworten tolerant lesen |
| Cloudflare oder Sperren bei MakerWorld | Downloads scheitern | Über `api.bambulab.com` statt `makerworld.com`, Drosselung, Zwischenspeicher |
| Token läuft ab (ca. 3 Monate) | Abgleich stoppt | Hinweis in „Heute“ und Einstellungen, ein Klick zum Neuanmelden |
| Bambu und App ziehen Verbrauch doppelt ab | Rest zu niedrig | Einstellung „Verbrauch abziehen durch“ (Stufe 2) |
| Authorization-Firmware schränkt LAN-MQTT weiter ein | 3b liefert nichts | 3a funktioniert unabhängig davon, 3b ist optional |

## Entscheidungen (16.11.2026)

1. **Anmeldung:** über das Anmeldefenster von Bambu (durch den Spike bestätigt).
2. **Region:** Global.
3. **Verbrauch abziehen:** weiter durch die App. Die Werte aus dem Konto gelten nur, wenn sich dort etwas geändert hat (bestehende Regel).
4. **MakerWorld-Dateien:** nur bei Bedarf laden.
5. **Drucker:** nur über das Konto (3a). **3b (LAN live) entfällt vorerst.** Der A1 erscheint erst, wenn er in Bambu Handy mit dem Konto verbunden ist.

## Quellen

- [OpenBambuAPI – Cloud HTTP](https://github.com/Doridian/OpenBambuAPI/blob/main/cloud-http.md)
- [OpenBambuAPI – MakerWorld](https://github.com/Doridian/OpenBambuAPI/blob/main/cloud-makerworld.md)
- [ha-bambulab PR #2028 – Filament-Manager aus dem Konto](https://github.com/greghesp/ha-bambulab/pull/2028)
- [3d-model-manager PR #195](https://github.com/metril/3d-model-manager/pull/195)
- [3d-model-manager PR #196](https://github.com/metril/3d-model-manager/pull/196)
- [Bambu Wiki – Third-party integration](https://wiki.bambulab.com/en/software/third-party-integration)
- [Bambu Wiki – Netzwerk-Ports](https://wiki.bambulab.com/en/general/printer-network-ports%3C)
- [All3DP – Developer Mode](https://all3dp.com/4/bambu-lab-responds-to-security-update-controversy-promises-developer-mode/)
- [Tom's Hardware – Authorization-Firmware](https://www.tomshardware.com/3d-printing/bambu-lab-removes-third-party-app-support-for-p-and-a-series-3d-printers)
- [Bambuddy – MakerWorld](https://wiki.bambuddy.cool/features/makerworld/)
- [3DBite – Filament-Manager in Bambu Studio 2.6.1](https://3dbite.com/bambu-studio-filament-manager-how-to-use/)
