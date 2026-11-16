# Printmanager – Benutzerhandbuch

Mit Printmanager behältst du den Überblick über 3D-Druckwünsche aus Familie und Freundeskreis: von der Anfrage bis zur Übergabe.
Gedruckt wird weiter in **Bambu Studio** und auf deinem Drucker. Die App organisiert die Aufträge, öffnet Dateien in Bambu Studio und liest Informationen aus deinem Bambu-Konto.
Sie startet, pausiert oder stoppt nie einen Druck.

---

## 1. Erste Schritte

1. **App starten.** Die App öffnet mit der Seite **Heute**.
2. **Bambu-Konto verbinden** (empfohlen):
   1. Öffne **Einstellungen → Bambu-Konto → Anmelden …**.
   2. Ein Fenster von Bambu Lab öffnet sich. Melde dich dort an wie gewohnt.
   3. Das Fenster schließt sich von selbst, und die App zeigt „Verbunden“.
   - Dein Passwort sieht die App nicht. Das Anmelde-Token liegt im Schlüsselbund deines Systems und gilt etwa drei Monate.
   - Fragt macOS, ob „printmanager“ auf den Schlüsselbund zugreifen darf, erlaube es.
3. **Ersten Auftrag anlegen:** **⌘N** oder **Neuer Auftrag** oben links.

Ohne Bambu-Konto funktioniert alles außer Druckerstatus und MakerWorld. Das Filament kommt dann aus der Datei `spools.json` von Bambu Studio.

---

## 2. Der Ablauf eines Auftrags

Jeder Auftrag durchläuft diese Stationen:

| Station | Bedeutung |
|---|---|
| Eingang | neu, noch nicht angeschaut |
| Klären | Rückfragen offen, z.B. Farbe, Größe oder Datei |
| Vorbereiten | Modell suchen oder anpassen |
| In Studio | wird in Bambu Studio eingerichtet |
| Druckt | liegt auf dem Drucker |
| Nacharbeit | Stützen entfernen, schleifen, kleben |
| Fertig | bereit zur Übergabe |
| Übergeben | abgeholt oder verschickt |
| Pausiert / Abgelehnt | liegt auf Eis bzw. wird nicht gedruckt |

So wechselst du die Station:
- Auf dem **Board** ziehst du die Karte in eine andere Spalte.
- Im Auftrag drückst du **Weiter: …** oder wählst die Station im Feld **Status**.

Manches geht von selbst:
- **In Bambu Studio öffnen** setzt einen Auftrag, der noch davor steht, auf **In Studio**.
- Speicherst du eine gesliced Datei im Auftragsordner, setzt die App den Auftrag auf **Druckt** (siehe Abschnitt 5).

---

## 3. Die Seiten im Überblick

### Heute
Die Startseite zeigt, was ansteht:
- **Fällig:** überfällig oder in den nächsten drei Tagen fällig
- **Drucke auswerten:** Drucke ohne Ergebnis oder ohne abgezogenen Verbrauch
- **Neu & zu klären**, **Druckt gerade**, **Nacharbeit**, **Zur Übergabe bereit**
- **Filament knapp:** Spulen unter 100 g
- Oben siehst du die Kennzahlen und laufende Drucker.

### Board
Eine Spalte pro Station. Karten zeigen Person, Fälligkeit, Teile und Anhänge.
Rot ist überfällig, Orange bald fällig. Ein farbiger Strich links bedeutet Priorität „hoch“ bzw. „dringend“.
**Archivieren** in der Spalte „Übergeben“ nimmt erledigte Aufträge vom Board.

### Alle Aufträge
Eine Tabelle mit Filter (offen, alle, Archiv, einzelne Station) und Sortierung. **⌘F** springt in die Suche, gesucht wird nach Titel, Person und Nummer.

### Auftrag (rechts)
Ein Klick auf einen Auftrag öffnet ihn rechts. **Esc** schließt ihn wieder.
- **Kopf:** In Bambu Studio öffnen, Auftragsordner, Archivieren, Löschen. Gelöschte Aufträge landen im Ordner `trash/`.
- **Eigenschaften:** Status, Person, Fällig bis, Priorität. Darunter die Beschreibung.
- **Teile:** Anzahl, Farbwunsch (z.B. „PETG rot“) und erledigt. Der Farbpunkt zeigt die passende Spule. Ein **!** heißt, dass keine Spule passt.
- **Drucke:** siehe Abschnitt 5.
- **Anhänge:** Dateien ins Fenster ziehen, mit **⌘V** Bilder, Links oder Text einfügen, oder „Datei wählen …“.
- **Verlauf:** Statuswechsel, Notizen und Druckmeldungen.

### Katalog
Fertige Projekte, die du immer wieder druckst.
- **Hinzufügen:** „+ Dateien“, Dateien ins Fenster ziehen oder **Von Link …** (Printables gibt Dateien direkt heraus).
- **MakerWorld:** Mit Bambu-Konto erscheinen links deine Sammlungen. Welche abgeglichen werden, legst du in den Einstellungen fest. Die 3MF lädt die App erst, wenn du sie öffnest, zu einem Auftrag hinzufügst oder auf **Datei holen** klickst.
- Projekte, die schon bestellt wurden, kannst du nur ausblenden, nicht löschen.

### Filament
Deine Spulen nach Material, mit Restbalken.
- Spulen im Drucker zeigen ihren AMS-Slot, z.B. „h2c · AMS 1 · Slot 2“.
- Den Rest kannst du direkt ändern. **Ausblenden** versteckt leere oder verliehene Spulen.
- Mit Bambu-Konto gleicht die App alle 15 Minuten ab. Änderst du den Rest in Bambu Studio oder Bambu Handy, gilt der neue Wert.

### Drucker
Deine Drucker aus dem Bambu-Konto, mit Zustand, Druckauftrag, Fortschritt, Restzeit und dem passenden Auftrag.
Die Seite aktualisiert sich jede Minute, während eines Drucks alle 20 Sekunden.
Drucker, die nicht mit deinem Konto verbunden sind, erscheinen nicht. Verbinden kannst du sie in Bambu Handy.

### Statistik
Filamentverbrauch, Druckzeit, Fehldrucke und Übergaben, wählbar für die letzten 12 Monate, dieses Jahr oder alles.
Dazu Filament pro Monat sowie Tabellen nach Person und nach Material.

### Personen
Wer etwas bestellt hat. Neue Personen entstehen beim Anlegen eines Auftrags von selbst.

### Einstellungen
- **Bambu-Konto:** anmelden oder abmelden, die Schalter für Filament, Drucker und MakerWorld, und die Auswahl der Sammlungen.
- **Speicherorte:** Datenordner, optional eine eigene Datenbank (PostgreSQL), der Ort von Bambu Studio und von `spools.json`.

---

## 4. Neuer Auftrag (⌘N)

Ein Assistent in vier Schritten:
1. **Für wen?** Name eintippen oder antippen.
2. **Was?** Aus dem Katalog wählen, einen Link einfügen (Printables, MakerWorld …) oder eigene Dateien und Bilder dazugeben. Das lässt sich auch mischen.
3. **Details:** Titel, Fälligkeit, Priorität, Beschreibung.
4. **Prüfen** und anlegen.

Ziehst du Dateien ins Fenster, während kein Auftrag offen ist, startet der Assistent mit diesen Dateien.

---

## 5. Drucken und Verbrauch erfassen

1. Im Auftrag **In Bambu Studio öffnen** klicken.
2. In Bambu Studio slicen und das Projekt mit **⌘S** im Auftragsordner speichern.
3. Die App erkennt die gesliced Datei nach wenigen Sekunden:
   - Unter **Drucke** erscheint ein Eintrag mit Druckzeit und Gramm pro Filament.
   - Der Auftrag steht jetzt auf **Druckt**.
4. Den Druck startest du wie gewohnt in Bambu Studio. Mit Bambu-Konto siehst du den Fortschritt unter **Drucker**, unter **Heute** und im Auftrag.
5. Ist der Druck fertig, meldet die App „h2c meldet: fertig …“ mit echter Dauer und echten Gramm. Mit **Als Erfolgreich übernehmen** bestätigst du.
6. **Verbrauch abziehen …** schlägt pro Filament die passende Spule vor (gleiches Material, ähnlichste Farbe). Gramm und Spule kannst du ändern. Mit **Rückgängig** nimmst du den Abzug zurück.

Erkannt wird nur ein Projekt, das nach dem Slicen gespeichert wurde und im Auftragsordner liegt. **Ordner prüfen** sucht von Hand.

---

## 6. Daten, Sicherung, Datenschutz

- Alle Daten liegen lokal im Datenordner (siehe **Einstellungen → Speicherorte**). **Zum Sichern kopierst du diesen Ordner.**
- `trash/` enthält gelöschte Aufträge samt Dateien. Diesen Ordner kannst du bei Bedarf von Hand leeren.
- Das Bambu-Konto wird nur gelesen. Die App ändert nichts an deinem Konto, an deinen Spulen, an MakerWorld-Sammlungen oder an Dateien von Bambu Studio.
- Die Schnittstellen von Bambu und MakerWorld sind nicht offiziell. Ändert Bambu sie, meldet die App „nicht erreichbar“ und arbeitet mit den letzten Daten weiter.

---

## 7. Tastenkürzel

| Taste | Wirkung |
|---|---|
| ⌘N | Neuer Auftrag |
| ⌘F | Suche (Board, Alle Aufträge) |
| ⌘V | Bild, Link oder Text als Anhang einfügen (Auftrag offen) |
| Esc | Auftrag schließen |

## 8. Häufige Fragen

- **Die Drucker-Seite ist leer.** Prüfe in den Einstellungen, ob das Bambu-Konto verbunden und „Drucker und Druckstatus anzeigen“ eingeschaltet ist. Der Drucker muss mit deinem Konto verbunden sein.
- **„Bambu-Anmeldung abgelaufen“.** Gehe zu **Einstellungen → Neu anmelden**.
- **Ein MakerWorld-Modell lässt sich nicht laden.** Kostenpflichtige oder exklusive Modelle gibt MakerWorld eventuell nicht heraus. Lade sie dann selbst herunter und ziehe sie in den Katalog.
- **Ein Druck wurde nicht erkannt.** Das Projekt muss nach dem Slicen im Auftragsordner gespeichert sein. Danach im Auftrag **Ordner prüfen** klicken.
