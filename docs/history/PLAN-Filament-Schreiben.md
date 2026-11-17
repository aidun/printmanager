# Plan: Filament ins Bambu-Konto zurückschreiben (abschaltbar)

Stand 17.11.2026. Stufe 0 (Test) erledigt: Ändern (Rest, Notiz), Anlegen und Löschen funktionieren. **Stufen 1, 2 und 4 umgesetzt**, aus Stufe 3 Anlegen, Löschen und der Vorrat. Offen: Bearbeiten von Name, Marke, Farbe und Notiz. Slot-Zuordnung im Konto: vom Server ignoriert.

## Ziel

Änderungen an Spulen in Printmanager sollen auch im Bambu-Konto landen, damit Bambu Studio, Bambu Handy und Printmanager denselben Stand zeigen. Dazu gehören:
- der Rest nach dem Abziehen oder Ändern
- Notiz und Name
- später Anlegen und Löschen

Das Ganze ist ein **eigener Schalter, der ab Werk ausgeschaltet ist**. Ohne ihn bleibt die App wie heute rein lesend.

## Leitplanken

1. **Ab Werk aus.** Der Schalter „Änderungen ins Bambu-Konto schreiben“ steht in **Einstellungen → Bambu-Konto**. Beim Einschalten erklärt die App die Risiken, und du bestätigst das einmal.
2. **Nichts ohne dein OK.** Jede Änderung zeigt vorher die Vorschau „Spule X: Rest 650 g → 588 g“ mit „Ins Konto schreiben“ oder „Nur in der App“. Bei Bedarf gibt es später die Option „immer ohne Nachfrage für den Rest“.
3. **Erst prüfen, dann schreiben.** Vor jedem Schreiben holt die App die Spule frisch aus dem Konto. Hat sie sich dort seit dem letzten Abgleich geändert, gibt es einen **Konflikt**, und die App überschreibt nichts.
4. **Spulen mit RFID:** Ihren Rest pflegt das AMS. Die App schreibt dort keinen Rest, sondern nur Notiz und Name. Das AMS würde einen geschriebenen Wert sonst wieder überschreiben.
5. **Nicht während eines Drucks:** Steckt die Spule in einem Drucker, der gerade druckt, wartet der Schreibvorgang bis nach dem Druck. Während eines Drucks gleicht Bambu selbst ab.
6. **Protokoll und Rückgängig:** Jede Änderung wird mit altem und neuem Wert protokolliert. „Rückgängig“ schreibt den alten Wert zurück.
7. **Löschen** nur für Spulen, die nicht im Drucker stecken, mit doppelter Bestätigung und vorher gesicherter Kopie im Protokoll.

Die harte Regel 2 in `AGENTS.md` („Bambu-Konto nur lesen“) bekommt eine Ausnahme: Schreiben ist nur für Spulen erlaubt, nur bei eingeschaltetem Schalter und nur nach Bestätigung.

## Bekannte Endpunkte

Die Endpunkte sind inoffiziell. Host `api.bambulab.com`, Basis `/v1/design-user-service`.

| Zweck | Aufruf | Stand |
|---|---|---|
| Liste lesen | `GET /my/filament/v2?offset=&limit=` | ✅ von uns genutzt |
| Ändern (Rest, Notiz, Name …) | `PUT /my/filament/v2`, `id` im Inhalt | ✅ von uns bestätigt (Notiz und Rest, ganzes oder minimales Objekt) |
| Anlegen | `POST /my/filament/v2` (Antwort `{}`, neue ID per erneutem GET) | ✅ von uns bestätigt |
| Löschen | `DELETE /my/filament/v2/batch` mit `{"ids": [...]}` | ✅ von uns bestätigt |
| AMS-Abgleich | `POST /my/filament/v2/ams/sync` (nur Bambu Studio/Handy) | 🚫 nicht nutzen |

Felder (camelCase): `id`, `filamentVendor`, `filamentType`, `filamentName`, `filamentId`, `color`, `colors`, `netWeight` (Rest in g), `totalNetWeight`, `note`, `status`, `RFID`. Geräte- und Slotfelder (`inPrinter`, `devId`, `amsId`, `slotId`, `trayIdName`) gelten als nur lesbar.

Quellen: [ha-bambu-filaments RESEARCH.md/SPEC.md](https://github.com/Ich-h4lt/ha-bambu-filaments), [BambuStudio Issue #10766](https://github.com/bambulab/BambuStudio/issues/10766).

---

## Stufe 0 – Ergebnis (17.11.2026, Testspule „PRINTMANAGER TEST“, Fremdmarke ohne RFID)

| Test | Ergebnis |
|---|---|
| PUT ganzes Objekt mit neuer Notiz | ✅ 200, Antwort `{"filamentV2": {…}}`, Notiz geändert |
| PUT nur `{id, filamentName, note}` | ✅ Notiz geändert, **alle anderen Felder unverändert** (Teilaktualisierung möglich) |
| PUT `netWeight` als `900.0` | ❌ 400 `strconv.ParseInt` → **Gramm müssen ganze Zahlen sein** |
| PUT ganzes Objekt mit `netWeight: 900` | ✅ 1000 → 900 g |
| PUT nur `{id, filamentName, netWeight: 850}` | ✅ 900 → 850 g, Notiz unverändert |
| POST (Kopie ohne ID und Gerätefelder) | ✅ Antwort `{}`, neue Spule per GET gefunden |
| DELETE batch `{"ids": [beide]}` | ✅ Antwort `{}`, beide Testspulen aus dem Konto gelöscht |

| PUT `{id, filamentName, inPrinter: true, devId, deviceName, amsId, slotId}` (Slot zuordnen) | ⚠️ Antwort 2xx, aber **Felder ignoriert**: danach `inPrinter=false`, `devId=""`, `amsId=null`, `slotId=""` (2 Versuche, Drucker und AMS waren aus) |

Erkenntnisse:
- Der Name aus Bambu Handy steht in `displayName`.
- `filamentName` ist der Produktname (z.B. „PLA“). Er muss beim PUT mitgeschickt werden, die übrigen Felder dürfen fehlen.
- `updatedAt` steigt bei jedem Schreiben. Damit lässt sich die Konfliktprüfung bauen.

Ursprünglicher Testablauf:

Ziel: herausfinden, was wirklich funktioniert, ohne eine echte Spule zu gefährden.

1. Du legst in Bambu Handy eine Spule **„PRINTMANAGER TEST“** an: Fremdmarke, ohne RFID, 1000 g, Rest 1000 g.
2. Ein Testbefehl, nur in einer Entwicklerversion, nur für diese eine Spule:
   1. **PUT Notiz** → GET zeigt die neue Notiz? Zeigt Bambu Handy sie nach dem Aktualisieren?
   2. **PUT Rest** 1000 → 900 g → bleibt der Wert? Zeigt Bambu Studio ihn nach einem Neustart?
   3. **Minimaler Inhalt:** Reicht `{id, filamentName, netWeight}`, oder braucht es das ganze Objekt? Welche Felder werden leer, wenn sie fehlen?
   4. **POST** einer zweiten Testspule → erscheint sie in Handy? Welche Felder sind Pflicht?
   5. **DELETE batch** beider Testspulen → sind sie weg?
3. Alle Antworten werden ohne Token im Scratch-Ordner gespeichert und danach ausgewertet, wie beim Test für das Konto.

**Fertig, wenn** feststeht, welche der vier Operationen tragen und wie der Inhalt aussehen muss. Fällt PUT für den Rest durch, endet der Plan hier. Dann bleibt nur das Ändern der Notiz, oder wir verzichten ganz.

## Stufe 1 – Grundlage (ca. 1 Tag)

- **`cloud.rs`:** `put`, `post` und `delete` mit derselben Drosselung (1 Anfrage pro Sekunde). Gibt der Server einen Fehler zurück, versucht die App es nicht erneut.
- **Schalter** `filament_write` in `bambu.json`, ab Werk `false`. Ist er aus, lehnt der Kern jeden Schreibbefehl ab, auch wenn die Oberfläche ihn schickt.
- **Protokoll** in einer neuen Tabelle `filament_writes` (Migration `0013`): Zeit, Spule, Aktion, Feld, alter und neuer Wert, Status (`ok`, `konflikt`, `fehler`, `rueckgaengig`), Fehlertext.
- **Prüfung vor dem Schreiben:** frisches `GET ?ids=<id>` und Vergleich von `updatedAt` mit dem letzten Abgleich. Weicht er ab, gibt es einen Konflikt, und die App schreibt nicht.
- **Tests:** mit `wiremock` für PUT, POST und DELETE, für Konflikt, abgeschalteten Schalter, RFID-Sperre und Drucksperre.

## Stufe 2 – Rest zurückschreiben (ca. 1 Tag)

- **Auslöser:** „Verbrauch abziehen“, „Rückgängig“ beim Abzug und das Ändern des Rests auf der Filament-Seite.
- **Ablauf:**
  1. Die Vorschau zeigt alt → neu.
  2. Die App prüft auf Konflikte.
  3. `PUT` mit `netWeight`.
  4. Die App liest erneut und vergleicht. Stimmt der Wert, ist die Änderung erledigt.
- **Ausnahmen:** RFID-Spulen bekommen nur den Hinweis „Rest pflegt das AMS“. Spulen im druckenden Drucker kommen in eine Warteschlange und werden nach dem Druck geschrieben.
- **Anzeige auf der Spule:** „im Konto gespeichert“, „wartet“ oder „Konflikt“. Bei einem Konflikt wählst du „Wert aus dem Konto übernehmen“ oder „Trotzdem schreiben“.

## Stufe 3 – Bearbeiten, Anlegen, Löschen (ca. 1–1,5 Tage)

- **Bearbeiten:** Name, Marke, Material, Farbe, Gesamtgewicht und Notiz in einem Spulen-Dialog.
- **Anlegen:** „+ Spule“ legt die Spule im Konto an. Danach liest die App sie neu und übernimmt sie.
- **Löschen:** nur bei Spulen ohne RFID und nicht im Drucker, mit doppelter Bestätigung. Eine Kopie bleibt im Protokoll.
- Ausgeführt wird jeweils nur, was in Stufe 0 bestätigt wurde.

## Stufe 4 – Protokoll in der Oberfläche (ca. ½ Tag)

- **Einstellungen → Bambu-Konto → „Geschriebene Änderungen“:** Liste mit Zeit, Spule, alt → neu und Status, dazu „Rückgängig“ für die letzten Einträge.

## Risiken

| Risiko | Folge | Gegenmaßnahme |
|---|---|---|
| Bambu ändert oder sperrt die Schreib-Endpunkte | Schreiben scheitert | Fehler wird protokolliert, die App läuft lesend weiter, Schalter aus |
| Falscher Inhalt löscht Felder | Spule im Konto beschädigt | Stufe 0 klärt den Inhalt. Die App schickt immer das frisch gelesene ganze Objekt mit geänderten Feldern. Rückgängig aus dem Protokoll |
| AMS oder Bambu Studio überschreibt den Wert | Wert springt zurück | RFID-Sperre, Schreiben nicht während eines Drucks, Konfliktprüfung |
| Konto-Sperre wegen Nutzung inoffizieller Endpunkte | Ärger mit dem Konto | wenige Anfragen (nur bei Änderungen), keine Massenoperationen, Schalter ab Werk aus |
| Doppelt gezählt (App und Bambu ziehen ab) | Rest zu niedrig | Bambu zieht bei Spulen ohne RFID nichts ab, bei RFID-Spulen schreibt die App keinen Rest |

## Reihenfolge und Aufwand

| Stufe | Inhalt | Aufwand |
|---|---|---|
| 0 | Test mit Testspule | ½ Tag, mit dir |
| 1 | Grundlage, Schalter, Protokoll, Konfliktprüfung | 1 Tag |
| 2 | Rest zurückschreiben | 1 Tag |
| 3 | Bearbeiten, Anlegen, Löschen | 1–1,5 Tage |
| 4 | Protokoll in der Oberfläche | ½ Tag |

Jede Stufe ist für sich nutzbar. Nach Stufe 2 ist der Hauptnutzen erreicht: Bambu Studio und Bambu Handy zeigen den Rest aus Printmanager.

## Zu entscheiden (vor Stufe 0)

1. **Nachfrage:** vor jedem Schreiben fragen (empfohlen) oder beim Rest automatisch schreiben?
2. **Umfang:** zuerst nur den Rest (Stufe 2, empfohlen) oder gleich auch Bearbeiten, Anlegen und Löschen?
3. **RFID-Spulen:** nichts außer der Notiz schreiben (empfohlen) oder den Rest trotzdem schreiben, mit dem Risiko, dass das AMS ihn zurücksetzt?
