# Plan: Begleit-App für iPhone und iPad

Stand 23.11.2026. Noch nichts umgesetzt, nichts auf iOS gebaut.

## Ziel

Aufträge unterwegs ansehen und anlegen, ohne dass der Mac dabei sein muss. Der Mac bleibt die **Hauptinstanz**:
- Er slict.
- Er beobachtet die Auftragsordner.
- Er gleicht mit dem Bambu-Konto ab.
- Er lädt Dateien.

Die iOS-App arbeitet auf derselben PostgreSQL-Datenbank und braucht keine lokalen Werkzeuge.

## Grundsatz

**Die Datenbank ist die Quelle für alles, was angezeigt oder für eine Entscheidung gebraucht wird. Originaldateien (3MF, STL, G-Code) bleiben auf dem Mac.**

Davon profitiert auch die Mac-App allein:
- Listen und Board lesen keine Dateien mehr.
- Verschobene Ordner führen nicht zu leeren Bildern.
- Eine Sicherung der Datenbank enthält alles, was man zum Anschauen braucht.

## Umfang der iOS-App

| Bereich | iOS | Bemerkung |
|---|---|---|
| Heute, Board, Liste, Auftrag ansehen | ja | Layout fürs iPhone neu, iPad nahe am heutigen |
| Auftrag anlegen, auch per Link | ja, auch offline | Dateien lädt der Mac (siehe B), offline über den Postausgang (siehe F) |
| Fotos an Auftrag hängen | ja | über die Datenbank zum Mac (siehe B) |
| Status, Termin, Kunde ändern | ja | |
| Filament, Drucker, Druckverlauf ansehen | ja | nur lesen, kein eigener Abgleich mit dem Konto |
| Kunden und Kontakte | ja | Contacts-Framework gibt es auch auf iOS |
| Slicen, Kalkulation, Druckvorbereitung | nein | nur Ergebnisse anzeigen, die der Mac berechnet hat |
| Ordner beobachten, `spools.json`, Bambu Studio öffnen | nein | |
| Abgleich mit dem Bambu-Konto | nein | nur der Mac, wegen Drosselung und Token |

## Schritte

### A. Vorbereitung im Kern (nützt schon der Mac-App)

1. **Vorschaubilder in die Datenbank:** neue Tabelle `blobs` (id, sha256, mime, bytes, created_at), WebP oder JPEG bei etwa 400 px, rund 20–50 KB.
   - Die Tabelle ist eigen, damit Listen die Bytes nicht mitladen.
   - Verweise kommen aus den Platten (heute `plateSrc`), aus dem Katalog (heute `thumbName`) und aus Bild-Anhängen (heute `attachments.path`).
   - Die Oberfläche lädt Bilder über einen Befehl bzw. ein eigenes Protokoll statt `convertFileSrc`.
2. **Was aus einer 3MF gelesen wird, wird gespeichert:** Platten, Objekte, Filamente, Gramm, Druckzeit, ob gesliced. Dazu Prüfsumme und Änderungszeit.
   - Der Mac liest nur neu, wenn sich die Datei geändert hat (`watch.rs`).
   - Auch das Ergebnis von `threemf::analyze` beim Hinzufügen einer Datei wird gespeichert.
3. **Einmaliger Lauf, der Vorschauen nachträgt:** für bestehende Aufträge und den Katalog.

### B. Dateiaufträge für den Mac

Eine Warteschlange `file_jobs` (Migration 0021 ff.) mit den Spalten:
- `id`
- `order_id`
- `kind`: `download_link` | `download_makerworld` | `place_blob`
- `payload` (JSON: Link, gewählte Dateien, blob_id)
- `state`: `pending` | `done` | `failed`
- `error`
- `claimed_at`, `claimed_by`
- `created_at`, `done_at`

1. **`link_import_to_order` teilen:**
   - Vorschau und Auftrag sofort anlegen, die Dateien als Job eintragen.
   - Auf dem Mac läuft der Download wie heute, aber im Hintergrund.
2. **Worker auf dem Mac:** beim Start und danach regelmäßig.
   - Er beansprucht einen Job per `UPDATE … SET claimed_at = … WHERE id = $1 AND claimed_at IS NULL` und lädt dann mit `download_file` bzw. `makerworld::fetch_file`.
   - Danach legt er die Datei im Auftragsordner ab, und der normale Ablauf übernimmt.
3. **Zustand im Auftrag:** „Wartet auf Mac“, „Dateien da“, bei einem Fehler Klartext mit „Erneut versuchen“.
4. **Fotos vom iPhone:** Sie gehen erst als Blob in die Datenbank, dann legt der Mac sie per `place_blob` als Anhang in den Ordner.

Das nützt schon jetzt: Auch am Mac ist der Auftrag sofort angelegt, selbst wenn der Download langsam ist.

### C. Mehrere Clients auf einer Datenbank

1. **Rolle je Installation:** `primary` (Mac) oder `companion` (iOS, weitere Macs).
   - Nur `primary` gleicht mit dem Konto ab, beobachtet Ordner und arbeitet Dateiaufträge ab.
   - Die Drosselung auf 1 Anfrage pro Sekunde bleibt so pro Konto gewahrt.
2. **Migrationen:**
   - Nur `primary` migriert.
   - Ein `companion` prüft die Schemaversion und meldet „App aktualisieren“, statt mit falschem Schema zu arbeiten.
3. **Änderungen anderer Clients sehen:** Neu laden beim Fokus und in einem Takt. Später optional `LISTEN/NOTIFY` (nur Postgres).
4. **Konflikte:** Der letzte Schreiber gewinnt pro Feld. Optional prüft `updated_at` beim Speichern und meldet „Inzwischen geändert“.

### D. Verbindung zu PostgreSQL (NAS, nur über Tailscale, ohne TLS)

1. PostgreSQL läuft auf dem NAS bzw. Heimserver und wird dort gesichert (`pg_dump` im Takt).
2. Tailscale (WireGuard) verschlüsselt die Verbindung. `sqlx` bleibt ohne TLS-Feature und verbindet mit `sslmode=disable`.
3. Postgres lauscht nur auf der Tailscale-Adresse. `pg_hba.conf` lässt nur Adressen aus dem Tailnet zu (100.64.0.0/10), mit `scram-sha-256`.
4. Die App warnt, wenn der Host weder eine Tailscale-Adresse noch `localhost` ist.
5. Der Verbindungs-String liegt im Schlüsselbund, nie in Dateien oder Logs.

### Umzug SQLite → PostgreSQL

Ein Befehl kopiert die Daten Tabelle für Tabelle von SQLite nach Postgres. `store.rs` kann beide Datenbanken lesen. Danach prüft er die Zeilenzahlen. Vorher sichert er die SQLite-Datenbank.

### F. Offline-Betrieb auf iOS

Ansatz: **Lokaler Zwischenspeicher plus Postausgang.** Eine vollständige Zwei-Wege-Synchronisation gibt es nicht.

- **Lokale SQLite auf dem iPhone** mit denselben Migrationen. `store.rs` ist portabel und lässt sich lokal wiederverwenden.
- **Lesen:**
  - Bei Verbindung lädt die App die Tabellen vollständig aus Postgres in die lokale Datenbank. Bei einem Nutzer sind das wenige MB.
  - Vorschaubilder kommen nur für offene Aufträge mit.
  - Die App zeigt immer die lokale Datenbank an, mit dem Hinweis „Stand: vor 2 Std.“.
- **Schreiben:**
  - Jede Änderung wirkt sofort lokal und kommt zusätzlich in die Tabelle `outbox` (id, op, payload JSON, base_updated_at, state, error, created_at).
  - Mögliche Vorgänge: `create_order`, `create_person`, `update_order` (Status, Termin, Priorität, Text), `add_photo`, `link_import`.
- **Nachsenden:**
  - Bei Verbindung werden die Vorgänge der Reihe nach gegen Postgres ausgeführt, mit denselben Store-Funktionen.
  - Die IDs entstehen als UUID schon auf dem Gerät. Gibt es eine ID schon, wird der Vorgang übersprungen. So schadet es nicht, wenn er zweimal ankommt.
- **Auftrags- und Kundennummern:**
  - Heute vergibt `store.rs` sie als `MAX(number)+1`. Offline geht das nicht.
  - Neue Aufträge zeigen „Neu“, bis der Server beim Nachsenden die Nummer vergibt.
  - Ein `UNIQUE`-Index auf `number` mit Wiederholung bei Konflikt verhindert doppelte Nummern. Das braucht auch Schritt C.
  - Den Auftragsordner legt der Mac erst an, wenn der Auftrag eine Nummer hat.
- **Konflikte:**
  - `update_order` trägt `base_updated_at` mit.
  - Hat sich der Auftrag auf dem Server inzwischen geändert, gewinnt der Server für die geänderten Felder. Die App meldet das im Klartext: „A0012 wurde am Mac geändert, dein Status wurde nicht übernommen.“
  - Neu angelegte Aufträge, Kunden und Fotos geraten nie in Konflikt.
- **Oberfläche:** Ein Symbol zeigt den Stand der Synchronisation („3 Änderungen warten“). Eine Liste zeigt die fehlgeschlagenen Vorgänge mit „Erneut“ und „Verwerfen“.
- **Mac:** Er braucht keinen Offline-Modus und schreibt direkt in Postgres.

### E. iOS-Build

1. `tauri ios init` mit eigener Kennung, zum Beispiel `de.printmanager.app.ios`, und eigener Konfiguration nach dem Muster von `tauri.dev.conf.json`.
2. **Code für den Mac abgrenzen** mit `cfg(not(target_os = "ios"))`:
   - `watch.rs` und `notify`
   - `slicers/`
   - `connectors/slicer.rs`
   - `estimates.rs` (Slicen)
   - `Command::new("open")` in `commands.rs`
3. Links und Dateien über Tauri-Plugins öffnen bzw. teilen, nicht per `Command`.
4. Kontakte: `objc2-contacts` auch für iOS, mit `NSContactsUsageDescription` in der Info.plist.
5. **Oberfläche:**
   - Die Ansichten fragen eine Fähigkeitenliste ab, etwa `canSlice`, `canWatch`, `isPrimary`, und blenden fehlende Aktionen aus, statt Fehler zu zeigen.
   - Layout fürs iPhone: Tab-Leiste statt Seitenleiste, Board als Liste nach Status, kein Drag & Drop nötig.
6. **Verteilung:** Apple-Entwicklerkonto (99 €/Jahr) und TestFlight. Ohne Konto muss die App alle 7 Tage neu signiert werden.

## Reihenfolge

1. A: Vorschauen und 3MF-Daten in der Datenbank (nur Mac, sofort nützlich)
2. B: Dateiaufträge (nur Mac, sofort nützlich)
3. C: mehrere Clients, mit UNIQUE-Nummern
4. D: Postgres auf dem NAS über Tailscale, getestet mit zwei Mac-Instanzen (stabil + Dev)
5. Umzug SQLite → PostgreSQL
6. F: Offline-Betrieb, zuerst als Store-Tests
7. E: iOS-Build, zuerst iPad, dann das Layout fürs iPhone

## Entscheidungen vom 23.11.2026

- PostgreSQL läuft auf dem NAS bzw. Heimserver, erreichbar nur über Tailscale, ohne TLS.
- Bilder liegen in Postgres (`blobs`, `bytea`). Ein S3-Speicher ist nicht nötig.
- Offline-Betrieb auf iOS ist von Anfang an dabei (Schritt F).
