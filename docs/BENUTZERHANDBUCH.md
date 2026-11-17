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
Eine Spalte pro Station. Karten zeigen Kunde, Fälligkeit, Teile und Anhänge.
Rot ist überfällig, Orange bald fällig. Ein farbiger Strich links bedeutet Priorität „hoch“ bzw. „dringend“.
**Archivieren** in der Spalte „Übergeben“ nimmt erledigte Aufträge vom Board.

### Alle Aufträge
Eine Tabelle mit Filter (offen, alle, Archiv, einzelne Station) und Sortierung. **⌘F** springt in die Suche, gesucht wird nach Titel, Kunde und Nummer.

### Auftrag (rechts)
Ein Klick auf einen Auftrag öffnet ihn rechts. **Esc** schließt ihn wieder.
- **Kopf:** In Bambu Studio öffnen, Auftragsordner, Archivieren, Löschen. Gelöschte Aufträge landen im Ordner `trash/`.
- **Eigenschaften:** Status, Kunde, Fällig bis, Priorität. Mit **Ändern** beim Kunden wählst du einen anderen Kunden oder legst einen neuen an. Ohne Kunden geht es nicht. Darunter die Beschreibung.
- **Teile:** Anzahl, Farbwunsch (z.B. „PETG rot“) und erledigt. Der Farbpunkt zeigt die passende Spule. Ein **!** heißt, dass keine Spule passt.
- **Kalkulation:** Verkaufspreis (Vorschlag) und Kosten. Gerechnet wird zuerst mit den Katalogprojekten, nach dem Slicen mit den Werten aus Bambu Studio und nach dem Druck mit den tatsächlichen Werten. Die Kennzeichnung zeigt die Quelle: „geschätzt (Katalog)“, „geschätzt (Slicer)“ oder „tatsächlich“. Unter **Details** steht die Aufteilung.
- **Drucke:** siehe Abschnitt 5. Mit **Druck wählen …** hängst du einen laufenden Druck oder einen Druck aus dem Druckverlauf an diesen Auftrag. Gesucht wird mit Suchfeld, Zeitraum und „nur ohne Auftrag“.
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
- Den Rest kannst du direkt ändern. Das **Auge** (Ausblenden) versteckt leere oder verliehene Spulen.
- Mit Bambu-Konto gleicht die App alle 15 Minuten ab. Änderst du den Rest in Bambu Studio oder Bambu Handy, gilt der neue Wert.

#### Ins Bambu-Konto schreiben (optional, ab Werk aus)
Schaltest du **Einstellungen → Bambu-Konto → Änderungen ins Bambu-Konto schreiben** ein, zeigen Bambu Studio und Bambu Handy denselben Rest wie Printmanager.
- **Rest:** Nach dem Ändern des Rests, nach **Verbrauch abziehen** und nach **Rückgängig** fragt die App: „Ins Bambu-Konto übernehmen?“ Mit **Nur in der App** bleibt das Konto unverändert.
- **Konflikt:** Hat sich die Spule im Konto seit dem letzten Abgleich geändert, schreibt die App nicht. Du wählst **Konto-Wert übernehmen** oder **Trotzdem schreiben**.
- **RFID-Spulen** von Bambu Lab: Ihren Rest pflegt das AMS, die App schreibt ihn nicht.
- **Spule im druckenden Drucker:** Die Änderung wartet und wird nach dem Druck geschrieben.
- **Spule anlegen:** **+ Spule** oben legt eine oder mehrere neue Spulen im Konto an: Marke, Material, Produkt, Name, Farbe, Gesamtgewicht und Rest.
- **Spule löschen:** Der Papierkorb auf einer Spule löscht genau diese Spule im Konto, nach Rückfrage. Nicht möglich bei Spulen im Drucker oder mit RFID.
- **Drucker und Slot:** Das Drucker-Symbol auf einer Spule ordnet sie im Konto einem Drucker, AMS und Slot zu. Bisher übernimmt das Bambu-Konto diese Angabe nicht. Die App meldet das, und es ändert sich nichts.
- **Vorrat:** Mit **−** und **+** auf einer Spule änderst du die Zahl gleicher Spulen (Marke, Produkt, Farbe) im Konto. **+** legt eine volle Spule an. **−** löscht zuerst eine leere oder die mit dem kleinsten Rest, nie eine im Drucker oder mit RFID. Beides fragt vorher nach.
- **Protokoll:** Unter **Einstellungen → Bambu-Konto → Geschriebene Änderungen** steht jede Änderung mit alt → neu. **Rückgängig** schreibt den alten Rest zurück.
- Die Schnittstelle ist inoffiziell. Ändert Bambu sie, scheitert das Schreiben mit einer Meldung, und die App arbeitet lesend weiter.

### Drucker
Deine Drucker aus dem Bambu-Konto, mit Zustand, Druckauftrag, Fortschritt, Restzeit und dem passenden Auftrag.
Die Seite aktualisiert sich jede Minute, während eines Drucks alle 20 Sekunden.

**Welcher Auftrag druckt?** Modelldateien im Auftrag tragen die Auftragsnummer im Namen, z.B. `A0004 benchy.stl`. Das passiert von selbst: bei neuen Dateien sofort, bei älteren Dateien offener Aufträge beim Start der App. Bambu Studio übernimmt diesen Namen für den Druck, so ordnet die App den Druck eindeutig dem Auftrag zu. Fehlt die Nummer, z.B. weil du das Projekt umbenannt hast, vergleicht die App die Dateinamen und bevorzugt Aufträge in „Druckt“ oder „In Studio“. Liegt sie daneben, klickst du auf der Druckerkarte auf **Ändern** und wählst den richtigen Auftrag. Das merkt sich die App auch für die Rückmeldung nach dem Druck. Die Karte zeigt, wie zugeordnet wurde: „per Nummer“ oder „von Hand“.
Drucker, die nicht mit deinem Konto verbunden sind, erscheinen nicht. Verbinden kannst du sie in Bambu Handy.

### Druckverlauf
Alle Drucke aus deinem Bambu-Konto, auch solche ohne Auftrag. Pro Druck siehst du Vorschaubild, Name, Drucker, Ende, echte Dauer, Filament (Slicer-Werte, der Drucker misst nicht), das Ergebnis (Fertig oder Abgebrochen) und den Auftrag.
- **Filter:** Alle, Ohne Auftrag, Mit Auftrag, Abgebrochen, dazu Zeitraum (7, 30, 90 Tage oder alles), Material und Drucker. Die Suche findet Name, Modell, Auftrag und Kunde.
- **Automatisch zugeordnet** werden Drucke mit Auftragsnummer im Namen (`A0004 …`), sonst nach Dateiname. Ein Druck, der vor dem Anlegen des Auftrags endete, wird nie automatisch zugeordnet.
- **Abgebrochene Drucke:** Das Konto meldet auch dann den vollen geplanten Verbrauch. Die App schätzt den tatsächlichen Anteil, angezeigt als „≈ 15 g von 84 g“: aus dem Fortschritt beim Abbruch, wenn sie den Druck beobachtet hat, sonst aus der Laufzeit im Verhältnis zur geplanten Druckzeit, abzüglich 5 Minuten Aufheizen. Diesen Wert nutzen Statistik, Auftrag und der Abzug-Dialog.
- **Nachträglich zuordnen:** **Zuordnen …** bzw. **Ändern**, dann den Auftrag über die Suche finden (Nummer, Titel, Kunde; „nur offene“ abwählbar). Die echte Dauer und die Gramm kommen an den Druck im Auftrag: an einen offenen Druck ohne Rückmeldung oder, wenn es keinen gibt, an einen neuen. Danach kannst du dort wie gewohnt den Verbrauch abziehen. Mit „– kein Auftrag –“ hebst du die Zuordnung wieder auf.
- **Abgleich:** Der ganze Verlauf wird kurz nach dem Start geholt, danach alle 15 Minuten die neuesten Drucke. **Aktualisieren** holt alles neu.

### Statistik
Filamentverbrauch, Druckzeit, Fehldrucke und Übergaben, wählbar für die letzten 12 Monate, dieses Jahr oder alles. Mit den Schaltern **Fertig**, **Erfolgreich**, **Abgebrochen** und **Geplant** wählst du, welche Drucke zählen. „Geplant“ sind Drucke, die gesliced, aber noch nicht gedruckt sind, also Filament und Zeit, die noch anstehen. Mit **Alle Drucke (Konto)** zählt die Statistik jeden Druck aus dem Druckverlauf mit echter Dauer, Drucke ohne Auftrag erscheinen als „Ohne Auftrag“. **Nur Aufträge** zählt nur die Drucke in Aufträgen.
Dazu Filament pro Monat sowie Tabellen nach Kunde und nach Material.

### Kunden
Deine Kundenverwaltung. Jeder Auftrag gehört zu genau einem Kunden.
- **Liste:** Kundennummer (K-0001 …), Name, E-Mail, Telefon, offene und alle Aufträge, letzter Auftrag. Die Suche findet Name, Nummer, E-Mail, Telefon, Adresse und Notiz.
- **Kunde öffnen:** Ein Klick auf die Zeile öffnet ihn. Du bearbeitest Name, E-Mail, Telefon, Adresse und Notiz und siehst rechts alle Aufträge des Kunden. Ein Klick öffnet den Auftrag.
- **Adresse:** Straße, Hausnummer, PLZ und Ort in eigenen Feldern. Die Liste zeigt PLZ und Ort.
- **Rechnungsadresse:** Mit **Abweichende Rechnungsadresse** gibst du eine eigene Rechnungsanschrift ein: Name oder Firma (leer heißt Kundenname), Straße, Hausnummer, PLZ und Ort. Straße und Ort sind dann Pflicht. In der Liste steht das Kennzeichen „Rechnung“. Nimmst du den Haken wieder heraus, löscht die App die Rechnungsadresse beim Speichern.
- **Neuer Auftrag** beim Kunden startet den Assistenten mit diesem Kunden.
- **+ Kunde** legt einen Kunden an. Einen Namen gibt es nur einmal: Legst du ihn doppelt an, nennt die App die vorhandene Kundennummer.
- Einen Kunden ohne Aufträge kannst du löschen. Kunden mit Aufträgen bleiben.

### Einstellungen
- **Bambu-Konto:** anmelden oder abmelden, die Schalter für Filament, Drucker und MakerWorld, die Auswahl der Sammlungen und der Schalter „Änderungen ins Bambu-Konto schreiben“ mit Protokoll.
- **Kalkulation:** Leistung des Druckers, Strompreis, Maschinenkosten pro Stunde, Puffer für Fehldrucke, Pauschale pro Auftrag, Preisfaktor, Mindestpreis, Rundung und Filamentpreise pro Material. Ein Beispiel zeigt sofort das Ergebnis.
- **Speicherorte:** Datenordner, optional eine eigene Datenbank (PostgreSQL), der Ort von Bambu Studio und von `spools.json`.

---

## 4. Neuer Auftrag (⌘N)

Ein Assistent in vier Schritten:
1. **Kunde** (Pflicht): suchen nach Name, Kundennummer, E-Mail oder Telefon, und anklicken oder mit Enter wählen. Gibt es ihn noch nicht, wählst du **Neuen Kunden „…“ anlegen**, ergänzt E-Mail und Telefon (optional) und klickst **Kunde anlegen**. Der Assistent geht dann weiter.
2. **Was?** Aus dem Katalog wählen, einen Link einfügen (Printables, MakerWorld …) oder eigene Dateien und Bilder dazugeben. Das lässt sich auch mischen.
3. **Details:** Titel, Fälligkeit, Priorität, Beschreibung.
4. **Prüfen** und anlegen.

Ziehst du Dateien ins Fenster, während kein Auftrag offen ist, startet der Assistent mit diesen Dateien. Auch dann wählst du zuerst den Kunden.

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
- Das Bambu-Konto wird nur gelesen. Einzige Ausnahme: Mit dem Schalter „Änderungen ins Bambu-Konto schreiben“ schreibt die App nach deiner Bestätigung den Rest von Spulen und legt Spulen an oder löscht sie. An MakerWorld-Sammlungen und an Dateien von Bambu Studio ändert sie nie etwas.
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
