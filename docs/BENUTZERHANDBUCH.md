# Printmanager – Benutzerhandbuch

Printmanager verwaltet 3D-Druckwünsche aus Familie und Freundeskreis, von der Anfrage bis zur Übergabe.
Gedruckt wird weiter in **Bambu Studio** und auf deinem Drucker. Die App organisiert die Aufträge, öffnet Dateien in Bambu Studio und liest Informationen aus deinem Bambu-Konto.
Sie startet, pausiert oder stoppt nie einen Druck.

---

## 1. Erste Schritte

1. **App starten.** Die App öffnet mit der Seite **Heute**.
2. **Bambu-Konto verbinden** (empfohlen):
   1. Öffne **Einstellungen → Anbindungen → Bambu-Konto → Anmelden …**.
   2. Ein Fenster von Bambu Lab öffnet sich. Melde dich dort an wie gewohnt.
   3. Das Fenster schließt sich von selbst, und die App zeigt „Verbunden“.
   - Dein Passwort sieht die App nicht. Das Anmelde-Token liegt im Schlüsselbund deines Systems und gilt etwa drei Monate.
   - Fragt macOS, ob „printmanager“ auf den Schlüsselbund zugreifen darf, erlaube es.
3. **Ersten Auftrag anlegen:** **⌘N** oder **Neuer Auftrag** oben links.

Ohne Bambu-Konto funktioniert alles außer Druckerstatus, Druckverlauf und MakerWorld. Das Filament kommt dann aus der Datei `spools.json` von Bambu Studio.

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
- Die Zahl an **Heute** in der Seitenleiste ist dieselbe wie bei **Fällig**. Rot wird sie, sobald ein Auftrag überfällig ist.

### Board
Eine Spalte pro Station. Karten zeigen Kunde, Fälligkeit, Teile und Anhänge.
Rot ist überfällig, Orange bald fällig. Ein farbiger Strich links bedeutet Priorität „hoch“ bzw. „dringend“.
Das Archiv-Symbol in der Spalte „Übergeben“ nimmt erledigte Aufträge vom Board.
Ist das Fenster zu schmal für alle Spalten, scrollst du waagerecht (Trackpad oder ⇧ + Mausrad).

### Alle Aufträge
Eine Tabelle mit Filter (offen, alle, Archiv, einzelne Station) und Sortierung. **⌘F** springt in die Suche, gesucht wird nach Titel, Kunde und Nummer.

### Auftrag (rechts)
Ein Klick auf einen Auftrag öffnet ihn rechts. **Esc** schließt ihn wieder.
- **Kopf:** In Bambu Studio öffnen, Auftragsordner, Archivieren, Löschen. Gelöschte Aufträge landen im Ordner `trash/`.
- **Eigenschaften:** Status, Kunde, Fällig bis, Priorität. Mit **Ändern** beim Kunden wählst du einen anderen Kunden oder legst einen neuen an. Ohne Kunden geht es nicht. Darunter die Beschreibung.
- **Teile:** Anzahl, Farbwunsch (z.B. „PETG rot“) und erledigt. Der Farbpunkt zeigt die passende Spule. Ein **!** heißt, dass keine Spule passt.
- **Kalkulation:** Verkaufspreis (Vorschlag) und Kosten. Die Kennzeichnung zeigt die Quelle, von der genauesten an:
  - **tatsächlich:** nach dem Druck
  - **geschätzt (Slicer):** du hast gesliced und im Auftragsordner gespeichert
  - **geschätzt (Slicer, vorab):** das Teil wurde im Hintergrund gesliced
  - **geschätzt (MakerWorld):** Druckzeit und Gewicht eines MakerWorld-Druckprofils
  - **geschätzt (Katalog):** Werte des Katalogprojekts

  Unter **Details** steht die Aufteilung.
- **Alle kalkulieren** (über der Teileliste) rechnet alle Teile nacheinander im Hintergrund. Teile ohne gespeichertes Profil bekommen dieselbe Vorgabe wie im Dialog unten. Teile ohne Modelldatei werden übersprungen. Am Ende steht, was gerechnet wurde.
- **Kalkulation und Druck je Teil:** Das Rechner-Symbol am Teil öffnet den gleichnamigen Dialog, siehe unten.
- **Drucke:** siehe Abschnitt 5. Mit **Druck wählen …** hängst du einen laufenden Druck oder einen aus dem Druckverlauf an den Auftrag.
- **Anhänge:** Dateien ins Fenster ziehen, mit **⌘V** Bilder, Links oder Text einfügen, oder „Datei wählen …“.
- **Verlauf:** Statuswechsel, Notizen und Druckmeldungen.

#### Kalkulation und Druck je Teil
Hier legst du fest, wie ein Teil gedruckt wird, und lässt es vorab rechnen.
- **Drucker und Profil:** Drucker, Düse, Prozess (Qualität, Normal oder Schnell, jeweils mit oder ohne Stützen) und Druckplatte. Zur Wahl stehen deine Drucker aus dem Bambu-Konto, vorgewählt ist der zuletzt benutzte. Ohne Konto stehen alle Drucker der Profilquelle zur Wahl. Neue Teile starten mit **Normal + Stützen**.
- **Farben und Filamente:** Je Filament-Slot des Modells wählst du eine Spule. Die App schlägt die passendste vor (Material, Farbe, Rest; Spulen im AMS zuerst) und wählt das Filament-Profil dazu. Ohne Spule wählst du Farbe und Profil von Hand. Hat das Modell keine eigenen Farben, gilt dein Farbwunsch am Teil.
- **Objekte:** Bei mehreren Objekten legst du je Objekt den Slot fest.
- **Dieselbe Spule in zwei Slots:** Das geht nicht, weil eine Spule nur in einem AMS-Fach steckt. Die App weist darauf hin, und **Mit Slot N zusammenlegen** verschiebt die Objekte in den ersten Slot.
- **Warnungen:** Spule reicht nicht, Profil passt nicht zum Material, mehr als vier Filamente.
- **Slicen und kalkulieren** rechnet mit Bambu Studio im Hintergrund. Bambu Studio druckt dabei nichts und legt nichts im Auftragsordner ab. Bei mehreren Slots gilt das Filament-Profil des ersten Slots für alle, Druckzeit und Gramm stimmen deshalb nur ungefähr. Einschalten unter **Einstellungen → Anbindungen → Bambu Studio → Im Hintergrund slicen** (ab Werk aus).
- **MakerWorld:** Bei MakerWorld-Projekten wählst du ein Druckprofil. Dessen Druckzeit und Gewicht gelten als Schätzung.
- Sind Prozess und alle benutzten Slots festgelegt, steht am Teil **Druck vorbereitet**.
- Gibt es schon einen echten Druck oder eine gesliced Datei, sagt der Dialog, dass Slicen nicht nötig ist.

#### Projekte mit mehreren Platten
- **Platten wählen:** Unter einem Teil mit einer 3MF mit mehreren Platten steht je Platte eine Karte mit Bild. Ein Klick wählt sie ab oder wieder an. Ist das Teil schon gerechnet, passt sich die Schätzung sofort an.
- **Unter Drucke** steht das Teil als ein Druck mit seinen Platten: gedruckt, offen oder übersprungen. Abgewählte Platten gelten als übersprungen, mit **Überspringen** markierst du weitere.
- **Nach dem Druck** schließt jeder Druck aus dem Druckverlauf seine Platte. Die Kalkulation rechnet gedruckte Platten tatsächlich und offene geplant („teils gedruckt, teils geplant“).
- **Abziehen** geht, sobald keine Platte mehr offen ist.
- Speicherst du die Datei später gesliced im Auftragsordner, übernimmt derselbe Druck die Werte. Es entsteht kein zweiter.

### Katalog
Fertige Projekte, die du immer wieder druckst.
- **Hinzufügen:** „+ Dateien“, Dateien ins Fenster ziehen oder **Von Link …** (Printables gibt Dateien direkt heraus).
- **MakerWorld:** Mit Bambu-Konto erscheinen links deine Sammlungen. Welche abgeglichen werden, legst du in den Einstellungen fest. Die 3MF lädt die App erst, wenn du sie öffnest, zu einem Auftrag hinzufügst oder auf **Datei holen** klickst.
- **Stichworte** (z.B. „Küche, Geschenk“) setzt du im Projekt. Zusammen mit den MakerWorld-Sammlungen sind sie die Kategorien, nach denen du beim Hinzufügen zu einem Auftrag filterst.
- Projekte, die schon bestellt wurden, kannst du nur ausblenden, nicht löschen.

### Filament
Deine Spulen nach Material, mit Restbalken.
- Spulen im Drucker zeigen ihren AMS-Slot, z.B. „h2c · AMS 1 · Slot 2“.
- Den Rest kannst du direkt ändern. Das **Auge** (Ausblenden) versteckt leere oder verliehene Spulen.
- Mit Bambu-Konto gleicht die App alle 15 Minuten ab. Änderst du den Rest in Bambu Studio oder Bambu Handy, gilt der neue Wert.

#### Ins Bambu-Konto schreiben (optional, ab Werk aus)
Schaltest du **Einstellungen → Anbindungen → Bambu-Konto → Änderungen an Spulen zurückschreiben** ein, zeigen Bambu Studio und Bambu Handy denselben Rest wie Printmanager.
- **Rest:** Nach dem Ändern des Rests, nach **Verbrauch abziehen** und nach **Rückgängig** fragt die App: „Ins Bambu-Konto übernehmen?“ Mit **Nur in der App** bleibt das Konto unverändert.
- **Konflikt:** Hat sich die Spule im Konto seit dem letzten Abgleich geändert, schreibt die App nicht. Du wählst **Konto-Wert übernehmen** oder **Trotzdem schreiben**.
- **RFID-Spulen** von Bambu Lab: Ihren Rest pflegt das AMS, die App schreibt ihn nicht.
- **Spule im druckenden Drucker:** Die Änderung wartet und wird nach dem Druck geschrieben.
- **Spule anlegen:** **+ Spule** oben legt eine oder mehrere neue Spulen im Konto an. Du gibst an, welches Filament es ist, welche Farbe es hat und wie viel drauf ist.
- **Spule löschen:** Der Papierkorb auf einer Spule löscht genau diese Spule im Konto, nach Rückfrage. Nicht möglich bei Spulen im Drucker oder mit RFID.
- **Drucker und Slot:** Das Drucker-Symbol auf einer Spule ordnet sie im Konto einem Drucker, AMS und Slot zu. Bisher übernimmt das Bambu-Konto diese Angabe nicht. Die App meldet das, und es ändert sich nichts.
- **Vorrat:** Mit **−** und **+** auf einer Spule änderst du die Zahl gleicher Spulen (Marke, Produkt, Farbe) im Konto. **+** legt eine volle Spule an. **−** löscht zuerst eine leere oder die mit dem kleinsten Rest, nie eine im Drucker oder mit RFID. Beides fragt vorher nach.
- **Protokoll:** Unter **Einstellungen → Anbindungen → Geschriebene Änderungen** steht jede Änderung mit alt → neu. **Rückgängig** schreibt den alten Rest zurück.
- Die Schnittstelle ist inoffiziell. Ändert Bambu sie, scheitert das Schreiben mit einer Meldung, und die App arbeitet lesend weiter.

### Drucker
Deine Drucker aus dem Bambu-Konto, mit Zustand, Druckauftrag, Fortschritt, Restzeit und dem passenden Auftrag.

Das Bambu-Konto meldet den Fortschritt meist nicht. Die App schätzt ihn dann aus Startzeit und geplanter Druckdauer und zeigt ihn mit „ca.“ an, zum Beispiel „ca. 70 %“. Pausen rechnet die Schätzung nicht heraus. Sie bleibt bis zum Ende des Drucks unter 100 %.
Die Seite aktualisiert sich jede Minute, während eines Drucks alle 20 Sekunden.

**Welcher Auftrag druckt?**
- Modelldateien im Auftrag bekommen die Auftragsnummer in den Namen, z.B. `A0004 benchy.stl`. Neue Dateien bekommen sie sofort, ältere Dateien offener Aufträge beim nächsten Start der App.
- Bambu Studio übernimmt diesen Namen für den Druck. Daran erkennt die App den Auftrag.
- Fehlt die Nummer, etwa nach dem Umbenennen, vergleicht die App die Dateinamen und bevorzugt Aufträge in „Druckt“ oder „In Studio“.
- Liegt sie daneben, klickst du auf der Druckerkarte auf **Ändern** und wählst den richtigen Auftrag. Das gilt dann auch für die Rückmeldung nach dem Druck.
- Die Karte zeigt, wie zugeordnet wurde: „per Nummer“ oder „von Hand“.

Drucker, die nicht mit deinem Konto verbunden sind, erscheinen nicht. Verbinden kannst du sie in Bambu Handy.

### Druckverlauf
Alle Drucke aus deinem Bambu-Konto, auch solche ohne Auftrag. Jede Zeile zeigt, wann und wie lange gedruckt wurde, wie viel Filament es war und zu welchem Auftrag der Druck gehört. Die Dauer ist gemessen, die Gramm kommen aus dem Slicer.
- **Filter:** Oben grenzt du ein, etwa auf Drucke ohne Auftrag oder auf Abbrüche, auf die letzten 7, 30 oder 90 Tage, auf ein Material oder einen Drucker. Die Suche findet Name, Modell, Auftrag und Kunde.
- **Automatisch zugeordnet** werden Drucke mit Auftragsnummer im Namen (`A0004 …`), sonst nach Dateiname. Ein Druck, der vor dem Anlegen des Auftrags endete, wird nie automatisch zugeordnet.
- **Abgebrochene Drucke:** Das Konto meldet auch dann den vollen geplanten Verbrauch. Die App schätzt deshalb, wie viel bis zum Abbruch verbraucht wurde, und zeigt es als „≈ 15 g von 84 g“. Hat sie den Druck beobachtet, nimmt sie den Fortschritt beim Abbruch. Sonst rechnet sie mit der Laufzeit im Verhältnis zur geplanten Druckzeit, abzüglich 5 Minuten Aufheizen. Statistik, Auftrag und Abzug nutzen diesen Wert.
- **Nachträglich zuordnen:** Klick auf **Zuordnen …** (oder **Ändern**) und such den Auftrag nach Nummer, Titel oder Kunde. Nimmst du den Haken bei „nur offene“ heraus, findest du auch erledigte Aufträge. Dauer und Gramm landen dann bei einem offenen Druck ohne Rückmeldung im Auftrag, sonst bei einem neuen. Dort ziehst du den Verbrauch wie gewohnt ab. „– kein Auftrag –“ hebt die Zuordnung wieder auf.
- **Abgleich:** Der ganze Verlauf wird kurz nach dem Start geholt, danach alle 15 Minuten die neuesten Drucke. **Aktualisieren** holt alles neu.

### Statistik
Filamentverbrauch, Druckzeit, Fehldrucke und Übergaben, dazu Filament pro Monat und Tabellen nach Kunde und Material.
- **Zeitraum:** letzte 12 Monate, dieses Jahr oder alles.
- **Welche Drucke zählen:** **Fertig**, **Erfolgreich**, **Abgebrochen** und **Geplant**. Geplant heißt gesliced, aber noch nicht gedruckt.
- **Alle Drucke (Konto)** zählt jeden Druck aus dem Druckverlauf. Drucke ohne Auftrag stehen unter „Ohne Auftrag“. **Nur Aufträge** zählt nur Drucke in Aufträgen.

### Kunden
Deine Kundenverwaltung. Jeder Auftrag gehört zu genau einem Kunden.
- **Liste:** Jeder Kunde hat eine Kundennummer (K-0001 …). Die Liste zeigt die Kontaktdaten und wie viele Aufträge offen sind. Die Suche findet Name, Nummer, E-Mail, Telefon, Adresse und Notiz.
- **Kunde öffnen:** Ein Klick auf die Zeile öffnet ihn. Du bearbeitest Name, E-Mail, Telefon, Adresse und Notiz und siehst rechts alle Aufträge des Kunden. Ein Klick öffnet den Auftrag.
- **Adresse:** Straße, Hausnummer, PLZ und Ort in eigenen Feldern. Die Liste zeigt PLZ und Ort.
- **Rechnungsadresse:** Mit **Abweichende Rechnungsadresse** gibst du eine eigene Rechnungsanschrift ein: Name oder Firma (leer heißt Kundenname), Straße, Hausnummer, PLZ und Ort. Straße und Ort sind dann Pflicht. In der Liste steht das Kennzeichen „Rechnung“. Nimmst du den Haken wieder heraus, löscht die App die Rechnungsadresse beim Speichern.
- **Neuer Auftrag** beim Kunden startet den Assistenten mit diesem Kunden.
- **+ Kunde** legt einen Kunden an. Einen Namen gibt es nur einmal: Legst du ihn doppelt an, nennt die App die vorhandene Kundennummer.
- Einen Kunden ohne Aufträge kannst du löschen. Kunden mit Aufträgen bleiben.

#### Kunden aus Kontakten (macOS)
Schalte unter **Einstellungen → Anbindungen → Kontakte** die Anbindung **Apple Kontakte (dieser Mac)** mit **Freigeben …** ein und erlaube den Zugriff im Dialog von macOS. Die App liest damit alle Konten der Kontakte-App (iCloud, Google, Exchange …), **ändert aber nie etwas an deinen Kontakten**.
- **Aus Kontakten …** gibt es auf der Kundenseite und im Assistenten „Neuer Auftrag“. Du suchst nach Name, Firma, E-Mail oder Telefon, und die App legt aus dem Kontakt einen Kunden an. Sie übernimmt Name, E-Mail, Telefon (Mobil zuerst) und Adresse, die Hausnummer getrennt von der Straße. Gibt es schon einen Kunden mit diesem Namen, wird er verknüpft statt doppelt angelegt.
- **Mit Kontakt verknüpfen …** im Kunden verbindet einen bestehenden Kunden mit einem Kontakt. Leere Felder werden aus dem Kontakt gefüllt, vorhandene bleiben.
- **Abgleich:** Alle 30 Minuten (oder mit **Verknüpfte Kunden abgleichen** unter Anbindungen) übernimmt die App Änderungen am Kontakt, z.B. nach einem Umzug. Hast du denselben Wert in der App geändert, gilt deiner. Haben sich beide geändert, zeigt der Kunde beide Werte, und du wählst **Kontakt übernehmen** oder **Kunde behalten**.
- Notiz und Rechnungsadresse fasst der Abgleich nie an. Gibt es den Kontakt nicht mehr, bleibt der Kunde unverändert; **Lösen** trennt die Verknüpfung.

### Einstellungen
- **Anbindungen:** Hier meldest du dich bei Diensten an und schaltest Programme ein oder aus, etwa das Bambu-Konto, Bambu Studio oder die Kontakte. Bei jeder Anbindung legst du fest, was sie darf. Beim Bambu-Konto wählst du auch die MakerWorld-Sammlungen. Darunter steht das Protokoll aller Änderungen, die die App ins Konto geschrieben hat.
- **Kalkulation:** Hier legst du fest, was ein Druck kostet: Strompreis und Leistung des Druckers, Maschinenkosten pro Stunde und Filamentpreise je Material. Auf die Kosten kommen ein Puffer für Fehldrucke, eine Pauschale pro Auftrag und der Preisfaktor, gerundet und nie unter dem Mindestpreis. Ein Rechenbeispiel zeigt sofort, was herauskommt.
- **Speicherorte:** Datenordner, optional eine eigene Datenbank (PostgreSQL), der Ort von Bambu Studio und von `spools.json`.

---

## 4. Neuer Auftrag (⌘N)

Ein Assistent in vier Schritten:
1. **Kunde** (Pflicht): suchen nach Name, Kundennummer, E-Mail oder Telefon, und anklicken oder mit Enter wählen. Gibt es ihn noch nicht, wählst du **Neuen Kunden „…“ anlegen**, ergänzt E-Mail und Telefon (optional) und klickst **Kunde anlegen**. Der Assistent geht dann weiter.
2. **Was?** Aus dem Katalog wählen, einen Link einfügen (Printables, MakerWorld …) oder eigene Dateien und Bilder dazugeben. Das lässt sich auch mischen. Im Katalog grenzt du die Auswahl neben der Suche mit **Alle Kategorien** auf eine Sammlung oder ein Stichwort ein. Die zuletzt gewählte Kategorie bleibt beim nächsten Mal eingestellt.
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
5. Ist der Druck fertig, meldet die App „h2c meldet: fertig …“ mit der gemessenen Dauer und den Gramm laut Bambu-Konto. Mit **Als Erfolgreich übernehmen** bestätigst du.
6. **Verbrauch abziehen …** schlägt pro Filament die passende Spule vor (gleiches Material, ähnlichste Farbe). Gramm und Spule kannst du ändern. Mit **Rückgängig** nimmst du den Abzug zurück.

Erkannt wird nur ein Projekt, das nach dem Slicen gespeichert wurde und im Auftragsordner liegt. **Ordner prüfen** sucht von Hand.

---

## 6. Daten, Sicherung, Datenschutz

- Alle Daten liegen lokal im Datenordner (siehe **Einstellungen → Speicherorte**). **Zum Sichern kopierst du diesen Ordner.**
- `trash/` enthält gelöschte Aufträge samt Dateien. Diesen Ordner kannst du bei Bedarf von Hand leeren.
- Das Bambu-Konto wird nur gelesen. Einzige Ausnahme: Mit dem Schalter „Änderungen an Spulen zurückschreiben“ schreibt die App nach deiner Bestätigung den Rest von Spulen und legt Spulen an oder löscht sie. An MakerWorld-Sammlungen und an Dateien von Bambu Studio ändert sie nie etwas.
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
- **„Bambu-Anmeldung abgelaufen“.** Unter **Einstellungen → Anbindungen → Bambu-Konto** auf **Neu anmelden** klicken.
- **Ein MakerWorld-Modell lässt sich nicht laden.** Kostenpflichtige oder exklusive Modelle gibt MakerWorld eventuell nicht heraus. Lade sie dann selbst herunter und ziehe sie in den Katalog.
- **Ein Druck wurde nicht erkannt.** Das Projekt muss nach dem Slicen im Auftragsordner gespeichert sein. Danach im Auftrag **Ordner prüfen** klicken.
